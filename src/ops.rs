//! 子命令编排：sync / pull / secret / import。
//!
//! 取值口径：**只有源镜像来自命令行**（位置参数），其余一律读 config.toml；
//! 复制一律经 GitHub Actions 远程触发（ADR-0001），
//! 凭据以加密口令加密成 `target_auth_secret` 传入、流水线内解密并打掩码（ADR-0002）。

use serde_json::Value;

use crate::config::{self, Config};
use crate::crypto;
use crate::i18n;
use crate::ui;
use crate::{docker, github, image};

/// 目标镜像解析（sync 与 pull 共用，可往返对账）：
/// 注册表取 `target_registry`（当前选定），org/repo 由 `[registries]` 条目逐键覆盖，
/// 再按 mode 组合（SPEC 第 4 节）。
fn resolve_target(source_image: &str, cfg: &Config) -> Result<String, String> {
    let registry = cfg.effective_registry();
    let org = cfg.effective_org();
    let repo = cfg.effective_repo();
    let target = image::compose_target_image(source_image, &registry, &org, &repo, cfg.mode)?;
    println!(
        "{} {}",
        ui::step(i18n::target_image_label()),
        ui::bold(&target)
    );
    Ok(target)
}

/// 登录态硬校验：目标注册表必须在 `~/.docker/config.json` 的 `auths` 下。
fn ensure_logged_in(docker_config: &Value, registry: &str) -> Result<(), String> {
    if docker::is_logged_in(docker_config, registry) {
        Ok(())
    } else {
        Err(i18n::err_not_logged_in(registry))
    }
}

/// 取目标注册表的 auth 字段（须已登录；条目缺 auth 字段时提示重新登录）。
fn lookup_auth_or_err(docker_config: &Value, registry: &str) -> Result<String, String> {
    docker::lookup_auth(docker_config, registry).ok_or_else(|| i18n::err_missing_auth(registry))
}

/// `docker mtrans sync <源镜像>`：触发远程流水线复制。
pub async fn sync(source_image: &str, cfg: &Config) -> Result<(), String> {
    let target = resolve_target(source_image, cfg)?;
    let registry = cfg.effective_registry();

    // 凭据以加密口令（[setting].auth_passphrase）加密成加密串传入，
    // 流水线内用 Secret AUTH_PASSPHRASE 解密并打掩码（ADR-0002）
    let docker_config = docker::read_docker_config()?;
    ensure_logged_in(&docker_config, &registry)?;
    let auth = lookup_auth_or_err(&docker_config, &registry)?;
    let auth_secret = crypto::encrypt_auth(&auth, &cfg.auth_passphrase)?;

    let token = if cfg.github_token.is_empty() {
        std::env::var("GITHUB_TOKEN").unwrap_or_default()
    } else {
        cfg.github_token.clone()
    };
    let gh = github::GithubClient::new(github::GITHUB_BASE, &token);
    github::trigger_workflow(
        &gh,
        config::or_default(&cfg.repo, config::DEFAULT_CI_REPO),
        config::or_default(&cfg.branch, config::DEFAULT_BRANCH),
        config::or_default(&cfg.workflow, config::DEFAULT_WORKFLOW),
        source_image,
        &target,
        &auth_secret,
        cfg.keep_run,
    )
    .await
}

/// `docker mtrans pull <源镜像>`：凭本地登录态从目标注册表直接拉取，
/// 重命名为源镜像名并删除本地目标标签（SPEC 第 4 节第 6 条）。
pub async fn pull(source_image: &str, cfg: &Config) -> Result<(), String> {
    let target = resolve_target(source_image, cfg)?;
    let registry = cfg.effective_registry();

    let docker_config = docker::read_docker_config()?;
    ensure_logged_in(&docker_config, &registry)?;
    let creds = docker::registry_credentials(&docker_config, &registry);

    let engine = docker::connect().await?;
    match docker::probe(&engine.docker, &target, creds.clone()).await? {
        docker::Probe::Exists => {}
        docker::Probe::Missing => {
            return Err(i18n::err_target_missing(&target, source_image));
        }
        docker::Probe::Unauthorized => {
            return Err(i18n::err_probe_unauthorized(&target, &registry));
        }
    }
    docker::pull_image(&engine.docker, &target, creds).await?;
    docker::tag_image(&engine.docker, &target, source_image).await?;
    docker::remove_image(&engine.docker, &target).await?;
    println!(
        "{} {}，{} {}",
        ui::ok(i18n::ok_pulled()),
        ui::bold(&target),
        ui::ok(i18n::ok_renamed()),
        ui::bold(source_image),
    );
    println!(
        "{} {}",
        ui::dim(i18n::deleted_target_tag_label()),
        ui::bold(&target)
    );
    Ok(())
}

/// `docker mtrans secret [REGISTRY]`：打印注册表登录凭据经 `auth_passphrase`
/// 加密后的加密串（即 `sync` 传给流水线的 `target_auth_secret`）。
/// 缺省用 `target_registry`，指定注册表则用指定值（须已登录）。
pub fn secret(registry: Option<&str>, cfg: &Config) -> Result<(), String> {
    let registry = registry
        .map(str::to_string)
        .unwrap_or_else(|| cfg.effective_registry());
    let docker_config = docker::read_docker_config()?;
    ensure_logged_in(&docker_config, &registry)?;
    let auth = lookup_auth_or_err(&docker_config, &registry)?;
    let auth_secret = crypto::encrypt_auth(&auth, &cfg.auth_passphrase)?;
    println!(
        "{} {}",
        ui::step(&registry),
        ui::dim(i18n::secret_credential_label())
    );
    println!("{}", ui::dim(i18n::secret_keep_safe()));
    println!("{}", ui::bold(&auth_secret));
    Ok(())
}

/// `docker mtrans import`：把已登录注册表同步为 `[registries]` 条目（幂等；
/// 已存在的注册表跳过、不作任何修改；凭据留在原地）。
pub fn import() -> Result<(), String> {
    let path = config::config_path()?;
    if !path.is_file() {
        return Err(i18n::err_config_not_found(&path));
    }
    let mut cfg = config::load();
    let added = sync_registries(&mut cfg)?;
    if added.is_empty() {
        println!("{}", ui::dim(i18n::import_no_new()));
        return Ok(());
    }
    let mut body = std::fs::read_to_string(&path).map_err(|e| i18n::err_read_file(&path, &e))?;
    for name in &added {
        body = config::append_registry_entry(&body, name);
    }
    config::write(&body)?;
    println!(
        "{} {} {}",
        ui::ok(i18n::ok_synced()),
        added.len(),
        ui::step(&added.join("、"))
    );
    Ok(())
}

/// 把 `~/.docker/config.json` 中已登录的注册表同步进 `cfg.registries`
/// （已存在跳过），返回新增列表。
pub fn sync_registries(cfg: &mut Config) -> Result<Vec<String>, String> {
    let docker_config = docker::read_docker_config()?;
    sync_registries_from(&docker_config, cfg).ok_or_else(|| i18n::err_no_docker_auths().to_string())
}

fn sync_registries_from(docker_config: &Value, cfg: &mut Config) -> Option<Vec<String>> {
    let keys = docker::auth_keys(docker_config);
    if keys.is_empty() {
        return None;
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut added = Vec::new();
    for key in keys {
        let name = docker::normalize_registry_key(&key);
        if seen.insert(name.clone()) && !cfg.registries.contains_key(&name) {
            cfg.registries
                .insert(name.clone(), config::RegistryEntry::default());
            added.push(name);
        }
    }
    Some(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cfg() -> Config {
        Config {
            target_registry: "r.example.com".into(),
            target_org: "jetsung".into(),
            target_repo: "myimage".into(),
            ..Default::default()
        }
    }

    #[test]
    fn resolve_target_mode1_and_mode2() {
        let cfg = base_cfg();
        assert_eq!(
            resolve_target("ghcr.io/idev/shortener:dev", &cfg).unwrap(),
            "r.example.com/jetsung/myimage:shortener",
            "mode1 使用 base_cfg 的 myimage"
        );
        let mut cfg = base_cfg();
        cfg.mode = 2;
        assert_eq!(
            resolve_target("ghcr.io/idev/shortener:dev", &cfg).unwrap(),
            "r.example.com/jetsung/shortener:dev"
        );
    }

    #[test]
    fn resolve_target_with_cli_overrides() {
        // 命令行覆盖仅内存生效：合并后的配置决定目标镜像，原配置不变
        let cfg = base_cfg();
        let merged = cfg
            .with_overrides(Some("new.example.com"), Some("other"), None, Some(2))
            .unwrap();
        assert_eq!(
            resolve_target("ghcr.io/idev/shortener:dev", &merged).unwrap(),
            "new.example.com/other/shortener:dev",
            "覆盖后的 registry/org/mode 生效"
        );
        // 原 Config 未被修改（覆盖不落盘）
        assert_eq!(cfg.effective_registry(), "r.example.com");
        assert_eq!(cfg.effective_org(), "jetsung");
        assert_eq!(cfg.mode, 1);
    }

    #[test]
    fn resolve_target_entry_overrides_per_key() {
        let mut cfg = base_cfg();
        cfg.mode = 2;
        cfg.registries.insert(
            "r.example.com".into(),
            config::RegistryEntry {
                target_org: "other".into(),
                target_repo: String::new(),
            },
        );
        assert_eq!(
            resolve_target("alpine:3.19", &cfg).unwrap(),
            "r.example.com/other/alpine:3.19",
            "条目 target_org 覆盖，target_repo 未设置仅影响 mode1"
        );
    }

    #[test]
    fn ensure_logged_in_and_auth_lookup() {
        let cfg = serde_json::json!({"auths": {"r.example.com": {"auth": "YTox"}}});
        assert!(ensure_logged_in(&cfg, "r.example.com").is_ok());
        assert!(ensure_logged_in(&cfg, "ghcr.io").is_err());
        assert_eq!(lookup_auth_or_err(&cfg, "r.example.com").unwrap(), "YTox");
        let empty_entry = serde_json::json!({"auths": {"r.example.com": {}}});
        assert!(lookup_auth_or_err(&empty_entry, "r.example.com").is_err());
    }

    #[test]
    fn sync_registries_is_idempotent_and_normalizes() {
        let docker_cfg = serde_json::json!({
            "auths": {
                "https://index.docker.io/v1/": {"auth": "YTox"},
                "r.example.com": {"auth": "YTox"},
                "https://r.example.com": {"auth": "YTox"}
            }
        });
        let mut cfg = base_cfg();
        cfg.registries
            .insert("r.example.com".into(), config::RegistryEntry::default());
        // r.example.com 已在条目表中；带 scheme 的重复键归一化后跳过
        let added = sync_registries_from(&docker_cfg, &mut cfg).unwrap();
        assert_eq!(added, vec!["docker.io"]);
        assert!(cfg.registries.contains_key("docker.io"));
        // 再跑一遍：全部已存在，无新增
        let added = sync_registries_from(&docker_cfg, &mut cfg).unwrap();
        assert!(added.is_empty());
    }

    #[test]
    fn sync_registries_empty_auths_is_none() {
        let mut cfg = base_cfg();
        assert!(sync_registries_from(&serde_json::json!({}), &mut cfg).is_none());
    }
}
