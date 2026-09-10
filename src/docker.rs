//! Docker Engine API：端点解析（context 链）、连接与版本协商、流式 pull、
//! 打标、删除、注册表探活，以及 `~/.docker/config.json` 的读取与归一化。
//!
//! 复制一律由远程 Runner 完成（ADR-0001），本地只做 `pull` 侧的拉取与改名，
//! 凭据由守护进程自身存储提供，不持久化。

use std::collections::BTreeMap;
use std::io::IsTerminal as _;
use std::io::Write as _;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use bollard::Docker;
use bollard::auth::DockerCredentials;
use bollard::query_parameters::{
    CreateImageOptionsBuilder, RemoveImageOptions, TagImageOptionsBuilder,
};
use futures_util::StreamExt as _;
use serde_json::Value;

use crate::image::split_ref;

pub enum Endpoint {
    /// unix socket 路径
    Unix(String),
    /// HTTP 基址（含 scheme，如 `tcp://127.0.0.1:2375` / `http://...`）
    Tcp(String),
}

impl Endpoint {
    fn to_bollard_host(&self) -> String {
        match self {
            Endpoint::Unix(p) => format!("unix://{p}"),
            Endpoint::Tcp(base) => base.clone(),
        }
    }
}

/// 解析 Docker 端点：`DOCKER_HOST` > `DOCKER_CONTEXT` > config.json `currentContext` > 默认 socket。
pub fn resolve_endpoint() -> Result<Endpoint, String> {
    if let Ok(h) = std::env::var("DOCKER_HOST") {
        let h = h.trim().to_string();
        if !h.is_empty() {
            return parse_host(&h).map_err(|e| crate::i18n::err_invalid_docker_host(&e));
        }
    }
    if let Ok(name) = std::env::var("DOCKER_CONTEXT") {
        let name = name.trim().to_string();
        if !name.is_empty() {
            return find_context(&name).ok_or_else(|| crate::i18n::err_context_missing(&name));
        }
    }
    if let Ok(cfg) = read_docker_config()
        && let Some(cur) = cfg.get("currentContext").and_then(|v| v.as_str())
        && !cur.is_empty()
        && let Some(ep) = find_context(cur)
    {
        return Ok(ep);
    }
    // currentContext 缺失或指向不存在的 context 时退回默认端点（与 docker CLI 容错一致）
    default_endpoint().map(Endpoint::Unix)
}

/// 平台默认 Docker 端点：Windows 命名管道，其余（Linux/macOS）unix socket。
fn default_endpoint() -> Result<String, String> {
    #[cfg(windows)]
    {
        Ok(r"\\.\pipe\docker_engine".to_string())
    }
    #[cfg(not(windows))]
    {
        // macOS 与 Linux 的默认 socket 同为 /var/run/docker.sock
        Ok("/var/run/docker.sock".to_string())
    }
}

fn parse_host(h: &str) -> Result<Endpoint, String> {
    if let Some(p) = h.strip_prefix("unix://") {
        return Ok(Endpoint::Unix(p.to_string()));
    }
    if let Some(p) = h.strip_prefix("npipe://") {
        // 命名管道仅 Windows 可用；Bollard 会按 Host 字符串自行识别
        return Ok(Endpoint::Unix(p.to_string()));
    }
    if let Some(rest) = h.strip_prefix("tcp://").or(h.strip_prefix("http://")) {
        return Ok(Endpoint::Tcp(format!("http://{rest}")));
    }
    if h.starts_with("https://") {
        return Err(format!(
            "暂不支持 TLS 端点: {h}；可用 ssh 隧道或本地代理转成 unix/tcp 明文端点"
        ));
    }
    if h.starts_with("ssh://") {
        return Err(format!(
            "暂不支持 ssh:// 端点: {h}；请先自行建立隧道后以 unix/tcp 端点连接（如 DOCKER_HOST）"
        ));
    }
    if h.starts_with("http://") {
        return Ok(Endpoint::Tcp(h.to_string()));
    }
    Err(format!(
        "无法解析端点: {h}（支持 unix://、tcp://、http://、npipe://）"
    ))
}

/// 在 `$HOME/.docker/contexts/meta/<哈希>/meta.json` 中按 Name 匹配 context。
/// 路径同样尊重 `DOCKER_CONFIG`（docker context 目录位于其下）。
fn find_context(name: &str) -> Option<Endpoint> {
    let base = std::env::var_os("DOCKER_CONFIG")
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(docker_config_home)?;
    let meta_root = base.join("contexts").join("meta");
    let entries = std::fs::read_dir(meta_root).ok()?;
    for entry in entries.flatten() {
        let meta = entry.path().join("meta.json");
        let Ok(text) = std::fs::read_to_string(&meta) else {
            continue;
        };
        let Ok(doc) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if doc.get("Name").and_then(|v| v.as_str()) != Some(name) {
            continue;
        }
        let host = doc.get("Endpoints")?.get("docker")?.get("Host")?.as_str()?;
        match parse_host(host) {
            Ok(ep) => return Some(ep),
            Err(e) => {
                eprintln!("{}", crate::i18n::warn_invalid_context_endpoint(name, &e));
                return None;
            }
        }
    }
    None
}

pub struct Engine {
    pub docker: Docker,
}

/// 连接守护进程并完成版本协商（GET /version 取服务端上限调低客户端版本）。
pub async fn connect() -> Result<Engine, String> {
    let endpoint = resolve_endpoint()?;
    let host = endpoint.to_bollard_host();
    let docker = Docker::connect_with_host(&host)
        .map_err(|e| crate::i18n::err_create_docker_client(&host, &e))?;
    let docker = docker.negotiate_version().await.map_err(|e| {
        format!(
            "无法连接 Docker 守护进程（{host}）: {e}\n\
             下一步: 确认 Docker 已启动且当前用户有 socket 权限；rootless 环境检查 XDG_RUNTIME_DIR 与 DOCKER_HOST/DOCKER_CONTEXT"
        )
    })?;
    Ok(Engine { docker })
}

/// 流式进度渲染：TTY 单行覆写层数与状态；非 TTY 只留关键行（由调用方打印）。
struct Renderer {
    label: &'static str,
    tty: bool,
    layers: BTreeMap<String, String>,
    done: usize,
}

impl Renderer {
    fn new(label: &'static str) -> Self {
        Self {
            label,
            tty: std::io::stdout().is_terminal(),
            layers: BTreeMap::new(),
            done: 0,
        }
    }

    fn is_done(status: &str) -> bool {
        matches!(status.trim(), "Pull complete" | "Already exists")
    }

    fn on(&mut self, id: Option<&str>, status: Option<&str>, progress: Option<&str>) {
        let (Some(id), Some(status)) = (id, status) else {
            return;
        };
        let prev = self.layers.insert(id.to_string(), status.to_string());
        if Self::is_done(status) && !prev.as_deref().map(Self::is_done).unwrap_or(false) {
            self.done += 1;
        }
        if self.tty && prev.as_deref() != Some(status) {
            let total = self.layers.len();
            let short = &id[..id.len().min(12)];
            let extra = progress.map(|p| format!(" {p}")).unwrap_or_default();
            print!(
                "\r  {} {}/{} {} · {} {}{}",
                self.label,
                self.done,
                total,
                crate::i18n::layers_unit(),
                short,
                status,
                extra
            );
            let _ = std::io::stdout().flush();
        }
    }

    fn finish(&mut self) {
        if self.tty {
            println!();
        }
    }
}

/// progressDetail 的 current/total 渲染为 `12.3M/50.1M`。
fn human_pair(current: Option<i64>, total: Option<i64>) -> Option<String> {
    match (current, total) {
        (Some(c), Some(t)) if t > 0 => Some(format!(
            "{:.1}M/{:.1}M",
            c as f64 / 1_000_000.0,
            t as f64 / 1_000_000.0
        )),
        _ => None,
    }
}

/// `POST /images/create`：拉取镜像（流式进度），凭据由调用方传入（X-Registry-Auth）。
pub async fn pull_image(
    docker: &Docker,
    image: &str,
    creds: Option<DockerCredentials>,
) -> Result<(), String> {
    let (repo, tag) = split_ref(image);
    let mut stream = docker.create_image(
        Some(
            CreateImageOptionsBuilder::new()
                .from_image(&repo)
                .tag(&tag)
                .build(),
        ),
        None,
        creds,
    );
    let mut renderer = Renderer::new(crate::i18n::pull_label());
    while let Some(item) = stream.next().await {
        match item {
            Ok(info) => {
                let prog = info
                    .progress_detail
                    .as_ref()
                    .and_then(|d| human_pair(d.current, d.total));
                renderer.on(info.id.as_deref(), info.status.as_deref(), prog.as_deref());
            }
            Err(e) => {
                if renderer.tty {
                    println!();
                }
                return Err(crate::i18n::err_pull_failed(image, &e));
            }
        }
    }
    renderer.finish();
    Ok(())
}

/// `POST /images/<name>/tag`。
pub async fn tag_image(docker: &Docker, source: &str, target: &str) -> Result<(), String> {
    let (repo, tag) = split_ref(target);
    docker
        .tag_image(
            source,
            Some(TagImageOptionsBuilder::new().repo(&repo).tag(&tag).build()),
        )
        .await
        .map_err(|e| crate::i18n::err_tag_failed(source, target, &e))
}

/// `DELETE /images/<name>`。
pub async fn remove_image(docker: &Docker, image: &str) -> Result<(), String> {
    docker
        .remove_image(image, None::<RemoveImageOptions>, None)
        .await
        .map_err(|e| crate::i18n::err_remove_failed(image, &e))?;
    Ok(())
}

/// 注册表探活结果（口径与基准「不存在或需登录」一致，拆两档便于提示）。
pub enum Probe {
    Exists,
    Missing,
    Unauthorized,
}

/// `GET /distribution/<name>/json` 探活（不下载镜像；凭据由调用方传入）。
pub async fn probe(
    docker: &Docker,
    image: &str,
    creds: Option<DockerCredentials>,
) -> Result<Probe, String> {
    match docker.inspect_registry_image(image, creds).await {
        Ok(_) => Ok(Probe::Exists),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(Probe::Missing),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 401 | 403,
            ..
        }) => Ok(Probe::Unauthorized),
        Err(e) => Err(crate::i18n::err_probe_failed(image, &e)),
    }
}

// ---------------------------------------------------------------------------
// ~/.docker/config.json
// ---------------------------------------------------------------------------

/// 平台默认 Docker 配置目录：`$HOME/.docker`（Docker CLI 约定，各平台一致）。
fn docker_config_home() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".docker"))
}

pub fn docker_config_path() -> Option<std::path::PathBuf> {
    // 尊重 DOCKER_CONFIG 覆盖路径；否则按 `$HOME/.docker/config.json`
    if let Some(cfg) = std::env::var_os("DOCKER_CONFIG")
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
    {
        return Some(cfg.join("config.json"));
    }
    docker_config_home().map(|d| d.join("config.json"))
}

/// 读取 `~/.docker/config.json`。
pub fn read_docker_config() -> Result<Value, String> {
    let path = docker_config_path().ok_or_else(|| crate::i18n::err_no_home_dir().to_string())?;
    if !path.is_file() {
        return Err(crate::i18n::err_docker_config_not_found(&path));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| crate::i18n::err_read_file(&path, &e))?;
    serde_json::from_str(&text).map_err(|e| {
        if crate::i18n::is_zh() {
            format!("读取 {} 失败（JSON 解析错误）: {e}", path.display())
        } else {
            format!("Failed to read {} (JSON parse error): {e}", path.display())
        }
    })
}

/// 注册表的候选键：裸域名、带 scheme 前缀，docker.io 额外匹配历史 Hub 键。
pub fn registry_candidates(registry: &str) -> Vec<String> {
    let mut v = vec![
        registry.to_string(),
        format!("https://{registry}"),
        format!("http://{registry}"),
    ];
    if registry == "docker.io" || registry == "index.docker.io" {
        v.push("https://index.docker.io/v1/".to_string());
        v.push("https://index.docker.io/v1".to_string());
        v.push("index.docker.io".to_string());
    }
    v
}

/// 登录态判断：config.json 的 `auths` 是否含该注册表（键可能是裸域名或带前缀）。
pub fn is_logged_in(config: &Value, registry: &str) -> bool {
    let Some(auths) = config.get("auths").and_then(|v| v.as_object()) else {
        return false;
    };
    let candidates = registry_candidates(registry);
    auths.keys().any(|k| candidates.contains(k))
}

/// 取某注册表的 `auth` 字段（base64(用户名:密码)）。
pub fn lookup_auth(config: &Value, registry: &str) -> Option<String> {
    let auths = config.get("auths").and_then(|v| v.as_object())?;
    for key in registry_candidates(registry) {
        if let Some(entry) = auths.get(&key)
            && let Some(auth) = entry.get("auth").and_then(|v| v.as_str())
            && !auth.is_empty()
        {
            return Some(auth.to_string());
        }
    }
    None
}

/// 构造交给守护进程的注册表凭据（`X-Registry-Auth`）。
///
/// 必须解出 username/password：仅填 `auth` 字段时守护进程仍以匿名访问私有注册表（实测 401），
/// 探活与拉取都会判为未授权。凭据只在内存传给本地守护进程，不落盘（ADR-0002）。
pub fn registry_credentials(config: &Value, registry: &str) -> Option<DockerCredentials> {
    let auth = lookup_auth(config, registry)?;
    let decoded = String::from_utf8(STANDARD.decode(&auth).ok()?).ok()?;
    let (username, password) = decoded.split_once(':')?;
    Some(DockerCredentials {
        username: Some(username.to_string()),
        password: Some(password.to_string()),
        serveraddress: Some(registry.to_string()),
        ..Default::default()
    })
}

/// config.json `auths` 的全部注册表键（排序）。
pub fn auth_keys(config: &Value) -> Vec<String> {
    config
        .get("auths")
        .and_then(|v| v.as_object())
        .map(|m| m.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default()
}

/// 注册表键归一化：去 scheme、去路径，Hub 历史键映射为 `docker.io`。
pub fn normalize_registry_key(key: &str) -> String {
    let without_scheme = key.split_once("://").map(|(_, rest)| rest).unwrap_or(key);
    let host = without_scheme.split('/').next().unwrap_or("");
    if host == "index.docker.io" {
        "docker.io".to_string()
    } else {
        host.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_keys() {
        assert_eq!(
            normalize_registry_key("https://index.docker.io/v1/"),
            "docker.io"
        );
        assert_eq!(normalize_registry_key("ghcr.io"), "ghcr.io");
        assert_eq!(
            normalize_registry_key("https://registry.cn-guangzhou.aliyuncs.com"),
            "registry.cn-guangzhou.aliyuncs.com"
        );
    }

    #[test]
    fn login_state_and_auth_lookup() {
        let cfg: Value = serde_json::json!({
            "auths": {
                "https://index.docker.io/v1/": {"auth": "YTox"},
                "r.example.com": {}
            }
        });
        assert!(is_logged_in(&cfg, "docker.io"));
        assert!(is_logged_in(&cfg, "r.example.com"));
        assert!(!is_logged_in(&cfg, "ghcr.io"));
        // r.example.com 条目无 auth 字段 -> 取不到
        assert_eq!(lookup_auth(&cfg, "r.example.com"), None);
        assert_eq!(lookup_auth(&cfg, "docker.io").as_deref(), Some("YTox"));
        assert_eq!(auth_keys(&cfg).len(), 2);
    }

    #[test]
    fn credentials_carry_username_password_and_server() {
        // YTox = base64("a:1")：必须解出 username/password，否则守护进程按匿名访问
        let cfg: Value = serde_json::json!({"auths": {"r.example.com": {"auth": "YTox"}}});
        let creds = registry_credentials(&cfg, "r.example.com").unwrap();
        assert_eq!(creds.username.as_deref(), Some("a"));
        assert_eq!(creds.password.as_deref(), Some("1"));
        assert_eq!(creds.serveraddress.as_deref(), Some("r.example.com"));
    }

    #[test]
    fn credentials_absent_without_auth_entry() {
        let cfg: Value = serde_json::json!({"auths": {"r.example.com": {}}});
        assert!(registry_credentials(&cfg, "r.example.com").is_none());
        assert!(registry_credentials(&cfg, "ghcr.io").is_none());
    }
}
