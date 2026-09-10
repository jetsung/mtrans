//! mtrans 配置（`$XDG_CONFIG_HOME/mtrans/config.toml`，未设置时回退平台标准配置目录）。
//!
//! schema 见 docs/SPEC.md：`[setting]`（`github_token`/`target_registry`/`target_org`/
//! `target_repo`/`mode`）+ `[ci]`（`repo`/`branch`/`workflow`/`keep_run`）+
//! `[registries."<域名>"]`（`target_org`/`target_repo` 逐键覆盖，未设置回退 `[setting]`）。
//! 写回一律按行文本改写以保留注释与顺序（ADR-0004），不使用 TOML 序列化。

use std::collections::BTreeMap;
use std::path::PathBuf;

pub const DEFAULT_REGISTRY: &str = "registry.cn-guangzhou.aliyuncs.com";
pub const DEFAULT_ORG: &str = "";
pub const DEFAULT_REPO: &str = "mtrans";
pub const DEFAULT_CI_REPO: &str = "jetsung/docker-build-sync";
pub const DEFAULT_BRANCH: &str = "main";
pub const DEFAULT_WORKFLOW: &str = "docker-mtrans.yml";

/// `[registries."<域名>"]` 条目：逐键覆盖 `[setting]` 的同名值，空值视为未设置。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegistryEntry {
    pub target_org: String,
    pub target_repo: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub github_token: String,
    /// 加密口令：加密登录凭据成 `target_auth_secret`，须与目标仓库 GitHub Secret `AUTH_PASSPHRASE` 一致
    pub auth_passphrase: String,
    pub target_registry: String,
    pub target_org: String,
    pub target_repo: String,
    pub mode: u8,
    pub repo: String,
    pub branch: String,
    pub workflow: String,
    pub keep_run: bool,
    pub registries: BTreeMap<String, RegistryEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            github_token: String::new(),
            auth_passphrase: String::new(),
            target_registry: DEFAULT_REGISTRY.to_string(),
            target_org: DEFAULT_ORG.to_string(),
            target_repo: DEFAULT_REPO.to_string(),
            mode: 1,
            repo: DEFAULT_CI_REPO.to_string(),
            branch: DEFAULT_BRANCH.to_string(),
            workflow: DEFAULT_WORKFLOW.to_string(),
            keep_run: false,
            registries: BTreeMap::new(),
        }
    }
}

impl Config {
    /// 生效目标组织：`[registries]` 条目逐键覆盖，未设置回退全局值（再回退内置默认）。
    pub fn effective_org(&self) -> String {
        let global = if self.target_org.is_empty() {
            DEFAULT_ORG
        } else {
            &self.target_org
        };
        self.registries
            .get(&self.target_registry)
            .map(|e| non_empty(&e.target_org).unwrap_or_else(|| global.to_string()))
            .unwrap_or_else(|| global.to_string())
    }

    /// 生效目标仓库：规则同 [`Self::effective_org`]。
    pub fn effective_repo(&self) -> String {
        let global = if self.target_repo.is_empty() {
            DEFAULT_REPO
        } else {
            &self.target_repo
        };
        self.registries
            .get(&self.target_registry)
            .map(|e| non_empty(&e.target_repo).unwrap_or_else(|| global.to_string()))
            .unwrap_or_else(|| global.to_string())
    }

    /// 生效目标注册表：返回配置里实际写下的 `target_registry`，未设置时为空（向导据此显示"未设置"）。
    pub fn effective_registry(&self) -> String {
        self.target_registry.clone()
    }

    /// 命令行覆盖（sync/pull/spull 的 --registry/--org/--repo/--mode）：
    /// 仅内存生效、绝不写回配置文件；None 表示未提供、沿用配置值。
    ///
    /// 优先级：命令行 > `[registries]` 条目 > `[setting]` > 内置默认。
    /// `--registry` 覆盖后条目查找随之使用新域名（effective_* 按 target_registry 查表）；
    /// `--org`/`--repo` 写入全局字段并压制条目对应键，保证命令行值盖过条目。
    pub fn with_overrides(
        &self,
        registry: Option<&str>,
        org: Option<&str>,
        repo: Option<&str>,
        mode: Option<u8>,
    ) -> Result<Config, String> {
        let mut out = self.clone();
        if let Some(r) = registry {
            out.target_registry = r.to_string();
        }
        if let Some(o) = org {
            out.target_org = o.to_string();
            if let Some(e) = out.registries.get_mut(&out.target_registry) {
                e.target_org.clear();
            }
        }
        if let Some(r) = repo {
            out.target_repo = r.to_string();
            if let Some(e) = out.registries.get_mut(&out.target_registry) {
                e.target_repo.clear();
            }
        }
        if let Some(m) = mode {
            if !matches!(m, 1..=3) {
                return Err(crate::i18n::err_unknown_mode(m));
            }
            out.mode = m;
        }
        Ok(out)
    }
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// 取值口径：配置文件值非空则用之，否则回退内置默认值。
pub fn or_default<'a>(cfg_value: &'a str, default: &'a str) -> &'a str {
    if cfg_value.is_empty() {
        default
    } else {
        cfg_value
    }
}

/// 判断配置值是否为真（1/true/yes/on）。
pub fn truthy(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// 配置文件路径：`XDG_CONFIG_HOME` 优先（Windows 亦可设置），
/// 否则用 `dirs::config_dir()`（各平台标准目录，Unix `~/.config`，Windows `%APPDATA%`）。
pub fn config_path() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::config_dir)
        .ok_or_else(|| crate::i18n::err_no_config_dir().to_string())?;
    Ok(base.join("mtrans").join("config.toml"))
}

/// 读取配置文件；不存在返回默认值，解析失败仅告警并按默认值继续。
pub fn load() -> Config {
    let fallback = Config::default();
    let Ok(path) = config_path() else {
        return fallback;
    };
    if !path.is_file() {
        return fallback;
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", crate::i18n::warn_read_failed(&path, &e));
            return fallback;
        }
    };
    match text.parse::<toml::Table>() {
        Ok(doc) => from_table(&doc),
        Err(e) => {
            eprintln!("{}", crate::i18n::warn_toml_parse_failed(&path, &e));
            fallback
        }
    }
}

/// TOML 值统一转字符串（bool -> "true"/"false"，其余 Display）。
fn value_to_string(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// 从 TOML 表构造 `Config`：空值视为未配置，回退内置默认值。
fn from_table(doc: &toml::Table) -> Config {
    let mut cfg = Config::default();
    let setting = doc.get("setting").and_then(|v| v.as_table());
    let ci = doc.get("ci").and_then(|v| v.as_table());
    let get_in = |t: Option<&toml::Table>, k: &str| -> String {
        t.and_then(|t| t.get(k))
            .map(value_to_string)
            .unwrap_or_default()
    };

    cfg.github_token = get_in(setting, "github_token");
    cfg.auth_passphrase = get_in(setting, "auth_passphrase");
    cfg.target_registry = get_in(setting, "target_registry");
    cfg.target_org = get_in(setting, "target_org");
    cfg.target_repo = get_in(setting, "target_repo");
    cfg.mode = match get_in(setting, "mode").as_str() {
        "2" => 2,
        "3" => 3,
        _ => 1,
    };
    cfg.repo = get_in(ci, "repo");
    cfg.branch = get_in(ci, "branch");
    cfg.workflow = get_in(ci, "workflow");
    cfg.keep_run = truthy(&get_in(ci, "keep_run"));
    if let Some(t) = doc.get("registries").and_then(|v| v.as_table()) {
        for (name, entry) in t {
            let Some(entry) = entry.as_table() else {
                continue;
            };
            let get = |k: &str| entry.get(k).map(value_to_string).unwrap_or_default();
            cfg.registries.insert(
                name.clone(),
                RegistryEntry {
                    target_org: get("target_org"),
                    target_repo: get("target_repo"),
                },
            );
        }
    }
    cfg
}

/// 把 `Config` 渲染为完整配置文件（首次向导生成用；注释与手工排版对齐仓库样例）。
pub fn render_template(cfg: &Config) -> String {
    let mut out = String::new();
    out.push_str("# mtrans 配置文件\n");
    out.push_str(
        "# 取值口径：本文件为唯一来源，只有源镜像走命令行；键缺省或留空时回退内置默认值\n",
    );
    out.push_str("\n[setting]\n\n");
    if cfg.github_token.is_empty() {
        out.push_str("# GitHub Token：触发远程流水线用；未设置时回退环境变量 GITHUB_TOKEN\n");
        out.push_str("# github_token = \"ghp_xxx\"\n\n");
    } else {
        out.push_str("# GitHub Token：触发远程流水线用；未设置时回退环境变量 GITHUB_TOKEN\n");
        out.push_str(&format!("github_token = {}\n\n", quote(&cfg.github_token)));
    }
    out.push_str("# 加密口令：sync 时把登录凭据加密成加密串传给流水线，流水线内用\n");
    out.push_str("# GitHub Secret AUTH_PASSPHRASE（值须与本项一致）解密。可用\n");
    out.push_str("#   openssl rand -base64 24\n");
    out.push_str("# 生成（去掉换行后填入）。\n");
    if cfg.auth_passphrase.is_empty() {
        out.push_str("# auth_passphrase = \"\"\n\n");
    } else {
        out.push_str(&format!(
            "auth_passphrase = {}\n\n",
            quote(&cfg.auth_passphrase)
        ));
    }
    out.push_str("# 目标注册表（注册表域名）：当前使用的注册表，即目标镜像推送的目的地\n");
    out.push_str(&format!(
        "target_registry = {}\n\n",
        quote(&cfg.target_registry)
    ));
    out.push_str("# 默认目标路径的两段：target_org/target_repo\n");
    out.push_str("# 目标镜像基础地址 = target_registry/target_org/target_repo\n");
    out.push_str("# 作为回退值：[registries] 目标注册表条目逐键覆盖，条目未设置的键回退到此\n");
    out.push_str(&format!("target_org = {}\n", quote(&cfg.target_org)));
    out.push_str(&format!("target_repo = {}\n\n", quote(&cfg.target_repo)));
    out.push_str("# 目标镜像组合模式（以源镜像 ghcr.io/idev/shortener:dev 为例）\n");
    out.push_str(
        "# 1 = 单仓库汇聚：源镜像都推进 target_org/target_repo，源镜像名作 tag，源 tag 丢弃\n",
    );
    out.push_str("#     target_registry/target_org/target_repo:<源镜像名>\n");
    out.push_str("#     -> registry.cn-guangzhou.aliyuncs.com/jetsung/myimage:shortener\n");
    out.push_str("# 2 = 仓库映射：target_org 下按源镜像名建仓库，tag 沿用源 tag\n");
    out.push_str(
        "# 3 = 固定中转：一律推到 target_org/target_repo:mtrans，target_repo 缺省时用 mtrans\n",
    );
    out.push_str("#     target_registry/target_org/<源镜像名>:<源 tag>\n");
    out.push_str("#     -> registry.cn-guangzhou.aliyuncs.com/jetsung/shortener:dev\n");
    out.push_str("#     （源无 tag 视为 latest；目标注册表不支持子组织，源组织段不保留）\n");
    out.push_str(&format!("mode = {}\n\n", cfg.mode));
    out.push_str("[ci]\n\n");
    out.push_str("# GitHub 仓库（owner/name）：远程流水线所在仓库（复制一律经其工作流触发）\n");
    out.push_str(&format!("repo = {}\n\n", quote(&cfg.repo)));
    out.push_str("# 触发工作流的分支\n");
    out.push_str(&format!("branch = {}\n\n", quote(&cfg.branch)));
    out.push_str("# 工作流文件名（可用 docker mtrans ci 生成模板）\n");
    out.push_str(&format!("workflow = {}\n\n", quote(&cfg.workflow)));
    out.push_str("# 复制成功后是否保留该次流水线运行记录（连同日志）；false = 成功即删除\n");
    out.push_str(&format!("keep_run = {}\n", cfg.keep_run));
    for (name, entry) in &cfg.registries {
        out.push_str(&format!("\n[registries.\"{name}\"]\n"));
        out.push_str("# 覆盖 [setting] 的 target_org / target_repo\n");
        if !entry.target_org.is_empty() {
            out.push_str(&format!("target_org = {}\n", quote(&entry.target_org)));
        }
        if !entry.target_repo.is_empty() {
            out.push_str(&format!("target_repo = {}\n", quote(&entry.target_repo)));
        }
    }
    out
}

fn quote(value: &str) -> String {
    format!("\"{value}\"")
}

/// 把文本写回配置文件（父目录不存在则创建）。
pub fn write(text: &str) -> Result<(), String> {
    let path = config_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| crate::i18n::err_create_dir_failed(dir, &e))?;
    }
    std::fs::write(&path, text).map_err(|e| crate::i18n::err_write_file(&path, &e))?;
    Ok(())
}

// ---------- 行文本改写（ADR-0004） ----------

/// 解析非注释行 `KEY = VALUE`，返回 (key, value)。
fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let stripped = line.trim();
    if stripped.is_empty() || stripped.starts_with('#') || !stripped.contains('=') {
        return None;
    }
    // 表头行（[registries."x"]）没有裸 KEY，天然被排除
    if stripped.starts_with('[') {
        return None;
    }
    let (k, v) = stripped.split_once('=')?;
    Some((k.trim(), v.trim()))
}

/// 值转 TOML 右值：true/false -> 布尔，纯数字 -> 数值，其余 -> 带引号字符串。
fn toml_rvalue(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "true" | "false" => return value.trim().to_lowercase(),
        _ => {}
    }
    if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        return value.to_string();
    }
    format!("\"{value}\"")
}

/// 在指定段（如 `[setting]`）内替换 `key` 的值；缺失则在该段末尾插入；
/// 段不存在则追加到文件末尾。保留注释与顺序。
pub fn rewrite_section_key(text: &str, section: &str, key: &str, value: &str) -> String {
    let new_line = format!("{key} = {}", toml_rvalue(value));
    let mut lines: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut section_seen = false;
    let mut replaced = false;
    let mut insert_at: Option<usize> = None;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            if in_section && insert_at.is_none() {
                insert_at = Some(lines.len());
            }
            in_section = trimmed == section;
            if in_section {
                section_seen = true;
            }
            lines.push(line.to_string());
            continue;
        }
        if in_section
            && !replaced
            && split_assignment(line)
                .map(|(k, _)| k == key)
                .unwrap_or(false)
        {
            lines.push(new_line.clone());
            replaced = true;
            continue;
        }
        lines.push(line.to_string());
    }

    if !replaced {
        if !section_seen {
            while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                lines.pop();
            }
            lines.push(String::new());
            lines.push(section.to_string());
            lines.push(new_line);
        } else {
            let at = insert_at.unwrap_or_else(|| {
                while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                    lines.pop();
                }
                lines.len()
            });
            let mut block = Vec::new();
            if at > 0 && !lines[at - 1].trim().is_empty() {
                block.push(String::new());
            }
            block.push(new_line);
            lines.splice(at..at, block);
        }
    }

    let mut body = lines.join("\n");
    body.push('\n');
    body
}

/// 文件末尾追加 `[registries."<name>"]` 空条目（条目已存在则原样返回）。
pub fn append_registry_entry(text: &str, name: &str) -> String {
    let header = format!("[registries.\"{name}\"]");
    if text.lines().any(|l| l.trim() == header) {
        return text.to_string();
    }
    let mut body = text.to_string();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    if !text.trim().is_empty() {
        body.push('\n');
    }
    body.push_str(&header);
    body.push('\n');
    body.push_str("# 覆盖 [setting] 的 target_org / target_repo\n");
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# mtrans 配置文件\n\n[setting]\n\n# 注册表注释\ntarget_registry = \"r.example.com\"\n\ntarget_org = \"jetsung\"\ntarget_repo = \"myimage\"\n\nmode = 1\n\n[ci]\n\nrepo = \"me/repo\"\nkeep_run = false\n\n[registries.\"r.example.com\"]\n# 覆盖 [setting] 的 target_org / target_repo\ntarget_org = \"jetsung\"\n";

    #[test]
    fn from_table_reads_all_keys() {
        let cfg = load_from(SAMPLE);
        assert_eq!(cfg.target_registry, "r.example.com");
        assert_eq!(cfg.mode, 1);
        assert_eq!(cfg.repo, "me/repo");
        assert!(!cfg.keep_run);
        let entry = cfg.registries.get("r.example.com").unwrap();
        assert_eq!(entry.target_org, "jetsung");
        assert_eq!(entry.target_repo, "", "未设置的键应为空");
    }

    fn load_from(text: &str) -> Config {
        from_table(&text.parse::<toml::Table>().unwrap())
    }

    #[test]
    fn empty_values_fall_back_to_defaults() {
        let cfg = load_from("[setting]\nmode = \"2\"\n\n[ci]\n");
        // 字段保留原值（空 = 未配置），回退发生在消费口径（effective_org/repo 与 or_default）
        assert_eq!(
            cfg.effective_registry(),
            "",
            "target_registry 未设置时为空（向导显示\"未设置\"）"
        );
        assert_eq!(cfg.effective_org(), DEFAULT_ORG);
        assert_eq!(cfg.mode, 2, "字符串 \"2\" 也应解析");
        assert_eq!(or_default(&cfg.repo, DEFAULT_CI_REPO), DEFAULT_CI_REPO);
        assert!(!cfg.keep_run);
    }

    #[test]
    fn effective_values_follow_entry_override_per_key() {
        let mut cfg = Config {
            target_registry: "r.example.com".into(),
            ..Default::default()
        };
        cfg.registries.insert(
            "r.example.com".into(),
            RegistryEntry {
                target_org: "other".into(),
                target_repo: String::new(),
            },
        );
        assert_eq!(cfg.effective_org(), "other", "条目设置的键覆盖");
        assert_eq!(cfg.effective_repo(), DEFAULT_REPO, "条目未设置的键回退全局");
        // 全局值为空时 effective_org 回退内置默认；effective_registry 未设置时为空（向导显示"未设置"）
        let cfg = load_from("[setting]\n");
        assert_eq!(cfg.effective_org(), DEFAULT_ORG);
        assert_eq!(cfg.effective_registry(), "");
    }

    #[test]
    fn effective_registry_empty_when_unset() {
        // 未配置 target_registry 时 effective_registry 为空（向导据此显示"未设置"）
        let cfg = load_from("[setting]\ntarget_org = \"x\"\n");
        assert_eq!(cfg.effective_registry(), "");
        // 已配置时返回配置值
        let cfg = load_from("[setting]\ntarget_registry = \"r.example.com\"\n");
        assert_eq!(cfg.effective_registry(), "r.example.com");
    }

    #[test]
    fn rewrite_replaces_in_place_and_keeps_comments() {
        let out = rewrite_section_key(SAMPLE, "[setting]", "target_org", "other");
        assert!(out.contains("# 注册表注释"), "注释须保留");
        assert!(
            out.contains("target_org = \"other\""),
            "段内键被替换: {out}"
        );
        // 其他段（[registries] 条目）中的同名键不受影响
        assert_eq!(
            out.matches("target_org = \"jetsung\"").count(),
            1,
            "registries 条目内的 target_org 未动: {out}"
        );
        // 段内其他键不受影响
        assert!(out.contains("target_registry = \"r.example.com\""));
    }

    #[test]
    fn rewrite_inserts_into_existing_section_before_next_header() {
        let out = rewrite_section_key(SAMPLE, "[setting]", "github_token", "ghp_x");
        let token_pos = out.find("github_token = \"ghp_x\"").unwrap();
        let mode_pos = out.find("mode = 1").unwrap();
        let ci_pos = out.find("[ci]").unwrap();
        assert!(
            token_pos > mode_pos && token_pos < ci_pos,
            "新键应插入 [setting] 段末、下一个表头之前: {out}"
        );
        let parsed = load_from(&out);
        assert_eq!(parsed.github_token, "ghp_x");
    }

    #[test]
    fn rewrite_appends_missing_section() {
        let out = rewrite_section_key(
            "[setting]\ntarget_org = \"jetsung\"\n",
            "[ci]",
            "branch",
            "main",
        );
        assert!(out.contains("[ci]\nbranch = \"main\""));
        let parsed = load_from(&out);
        assert_eq!(parsed.branch, "main");
    }

    #[test]
    fn rewrite_value_type_inference() {
        let out = rewrite_section_key(SAMPLE, "[ci]", "keep_run", "true");
        assert!(out.contains("keep_run = true"), "布尔不带引号: {out}");
        let out = rewrite_section_key(SAMPLE, "[setting]", "mode", "2");
        assert!(out.contains("mode = 2"), "数字不带引号: {out}");
    }

    #[test]
    fn append_entry_is_idempotent() {
        let once = append_registry_entry(SAMPLE, "ghcr.io");
        assert!(once.contains("[registries.\"ghcr.io\"]"));
        let twice = append_registry_entry(&once, "ghcr.io");
        assert_eq!(once, twice, "重复追加应原样返回");
        // 既有条目不动
        assert!(append_registry_entry(SAMPLE, "r.example.com") == SAMPLE);
    }

    #[test]
    fn render_template_is_valid_and_round_trips() {
        let cfg = Config {
            registries: BTreeMap::from([("ghcr.io".to_string(), RegistryEntry::default())]),
            ..Default::default()
        };
        let text = render_template(&cfg);
        let parsed = load_from(&text);
        assert_eq!(parsed.mode, 1);
        assert!(parsed.registries.contains_key("ghcr.io"));
        assert_eq!(parsed.effective_org(), DEFAULT_ORG);
    }

    #[test]
    fn truthy_accepts_baseline_spellings() {
        for v in ["1", "true", "YES", " on "] {
            assert!(truthy(v), "{v}");
        }
        for v in ["", "0", "false", "no"] {
            assert!(!truthy(v), "{v}");
        }
    }

    #[test]
    fn with_overrides_none_keeps_everything() {
        let cfg = Config {
            target_registry: "r.example.com".into(),
            target_org: "jetsung".into(),
            target_repo: "myimage".into(),
            mode: 1,
            ..Default::default()
        };
        let out = cfg.with_overrides(None, None, None, None).unwrap();
        assert_eq!(out.target_registry, "r.example.com");
        assert_eq!(out.effective_org(), "jetsung");
        assert_eq!(out.effective_repo(), "myimage");
        assert_eq!(out.mode, 1);
    }

    #[test]
    fn with_overrides_org_repo_beat_entry_and_setting() {
        // 条目与 [setting] 均已设置：命令行 --org/--repo 必须盖过条目
        let cfg = Config {
            target_registry: "r.example.com".into(),
            target_org: "global".into(),
            target_repo: "globalrepo".into(),
            registries: BTreeMap::from([(
                "r.example.com".into(),
                RegistryEntry {
                    target_org: "entry".into(),
                    target_repo: "entryrepo".into(),
                },
            )]),
            ..Default::default()
        };
        let out = cfg
            .with_overrides(None, Some("cli-org"), Some("cli-repo"), None)
            .unwrap();
        assert_eq!(out.effective_org(), "cli-org");
        assert_eq!(out.effective_repo(), "cli-repo");
    }

    #[test]
    fn with_overrides_registry_switches_entry_lookup() {
        // --registry 换注册表后，条目查找用新域名；未覆盖的 org/repo 回退该条目
        let cfg = Config {
            target_registry: "old.example.com".into(),
            target_org: "global".into(),
            registries: BTreeMap::from([
                (
                    "old.example.com".into(),
                    RegistryEntry {
                        target_org: "old-entry".into(),
                        target_repo: String::new(),
                    },
                ),
                (
                    "new.example.com".into(),
                    RegistryEntry {
                        target_org: "new-entry".into(),
                        target_repo: "new-repo".into(),
                    },
                ),
            ]),
            ..Default::default()
        };
        let out = cfg
            .with_overrides(Some("new.example.com"), None, None, None)
            .unwrap();
        assert_eq!(out.effective_registry(), "new.example.com");
        assert_eq!(out.effective_org(), "new-entry", "条目查找应随新注册表");
        assert_eq!(out.effective_repo(), "new-repo");
    }

    #[test]
    fn with_overrides_registry_combined_with_org() {
        // --registry + --org 组合：org 覆盖新注册表的条目值
        let cfg = Config {
            target_registry: "old.example.com".into(),
            registries: BTreeMap::from([(
                "new.example.com".into(),
                RegistryEntry {
                    target_org: "new-entry".into(),
                    target_repo: String::new(),
                },
            )]),
            ..Default::default()
        };
        let out = cfg
            .with_overrides(Some("new.example.com"), Some("cli-org"), None, None)
            .unwrap();
        assert_eq!(out.effective_org(), "cli-org");
    }

    #[test]
    fn with_overrides_mode_validates_and_applies() {
        let cfg = Config::default();
        let out = cfg.with_overrides(None, None, None, Some(2)).unwrap();
        assert_eq!(out.mode, 2);
        let out = cfg.with_overrides(None, None, None, Some(3)).unwrap();
        assert_eq!(out.mode, 3);
        assert!(cfg.with_overrides(None, None, None, Some(4)).is_err());
        assert!(cfg.with_overrides(None, None, None, Some(0)).is_err());
    }

    #[test]
    fn with_overrides_does_not_mutate_source() {
        // 覆盖仅内存生效：原 Config（含条目键）不受影响
        let mut cfg = Config {
            target_registry: "r.example.com".into(),
            target_org: "global".into(),
            registries: BTreeMap::from([(
                "r.example.com".into(),
                RegistryEntry {
                    target_org: "entry".into(),
                    target_repo: String::new(),
                },
            )]),
            ..Default::default()
        };
        let _ = cfg
            .with_overrides(None, Some("cli-org"), None, None)
            .unwrap();
        assert_eq!(cfg.effective_org(), "entry", "原配置条目值不变");
        assert_eq!(cfg.target_org, "global");
        cfg.registries.get_mut("r.example.com").unwrap();
    }
}
