//! 系统语言检测与帮助本地化。
//!
//! 依据 `LANG` / `LC_ALL` / `LC_MESSAGES` 判断界面语言：`zh*` 用中文，其余一律英文。
//! 中文走 clap 属性里的默认文案；英文在渲染帮助前把命令对象覆盖为英文文案。
//!
//! 运行时文案（进度/成功/错误提示）在本模块下半部分：中英文成对，中文与既有文案逐字一致。

use clap::Command;

/// 界面语言。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// English（默认）
    En,
    /// 简体中文
    Zh,
}

/// 从系统环境变量推断语言。
pub fn detect() -> Lang {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default();
    if lang.to_ascii_lowercase().starts_with("zh") {
        Lang::Zh
    } else {
        Lang::En
    }
}

/// 是否中文界面。
pub fn is_zh() -> bool {
    detect() == Lang::Zh
}

/// 把命令对象本地化（在渲染帮助前调用）。
/// 中文沿用 clap 属性的默认文案；英文覆盖为英文文案。
pub fn localize(cmd: &mut Command) {
    if is_zh() {
        return;
    }
    *cmd = cmd.clone().about("Docker image transfer tool: copy source images to a target registry via GitHub Actions, or pull them back to local");
    *cmd = cmd.clone().help_template(
        r#"{about}

Usage: {usage}

Commands:

  Image transfer:
    sync <SOURCE_IMAGE>       Copy image to the target registry via remote workflow
    pull <SOURCE_IMAGE>       Pull back from the target registry and rename
    spull <SOURCE_IMAGE>      sync + pull in one go

  Credentials:
    secret [REGISTRY]         Print the encrypted credential (auth_passphrase)
    passphrase                TUI to set the auth_passphrase
    import                    Sync logged-in registries into [registries]

  Configuration:
    registry                  TUI to pick the target registry
    config                    TUI wizard: passphrase/registry/org/repo/mode/[ci]

  Workflow template:
    ci [FILE]                 Output the docker-mtrans.yml workflow YAML template

  Help:
    version                   Show version information
    help                      Show concise subcommand list
    man                       Show full manual (detailed usage)

Options:
  -h, --help     Print help
"#,
    );

    // 英文子命令 about：与中文属性文案同源的结构化多行（一行摘要 + 缩进说明块）
    let sub_abouts: [(&str, &str); 12] = [
        (
            "sync",
            "Copy image: trigger the remote GitHub Actions workflow to copy to the target registry\n
        Source image   positional, e.g. ghcr.io/jetsung/shortener:latest (no @sha256: digest)
        Prerequisite   target registry logged in (an entry under auths in ~/.docker/config.json)
        Credential     auth encrypted with auth_passphrase into target_auth_secret on trigger
        Other values   all from config.toml; -R/-o/-r/-m override once, config file untouched",
        ),
        (
            "pull",
            "Pull image: fetch from the target registry and rename to the source image name\n
        Target         derived from the source image by compose rules (no sync needed first)
        Flow           pull target image → rename to source name → remove local target tag
        Missing target error with a hint to run `sync <SOURCE_IMAGE>` first
        Prerequisite   target registry logged in; otherwise same as sync (-R/-o/-r/-m)",
        ),
        (
            "spull",
            "Sync then pull: sync + pull in one go\n
        Flow           sync (remote copy), then pull (fetch and rename) right after success
        Overrides      -R/-o/-r/-m shared by both steps via one effective config",
        ),
        (
            "secret",
            "Print the encrypted credential (auth encrypted with auth_passphrase, i.e. target_auth_secret)\n
        Registry       default target_registry; or pass one (e.g. docker.cnb.cool)
        Output         auth encrypted with auth_passphrase (random salt, ciphertext differs each run)
        Prerequisite   logged in to that registry; auth_passphrase set in config.toml (error if unset)",
        ),
        (
            "import",
            "Sync logged-in registries into [registries] entries (idempotent)\n
        Source         auths in ~/.docker/config.json (present once logged in)
        Rules          existing registries are skipped unchanged; credentials stay in place",
        ),
        (
            "registry",
            "TUI to pick the target registry (writes target_registry)\n
        Candidates     registries logged in ~/.docker/config.json
        Side effect    syncs logged-in registries into [registries] entries (existing ones skipped)",
        ),
        (
            "passphrase",
            "TUI to set the auth passphrase (writes [setting].auth_passphrase)\n
        Options        keep current / generate a random passphrase / type one (same as config step 1)
        Side effect    echoes the value after writing; sync it to GitHub Secret AUTH_PASSPHRASE
        Prerequisite   config.toml exists (run `config` first to initialize)",
        ),
        (
            "config",
            "TUI wizard to configure config.toml step by step\n
        Steps          1 passphrase → 2 target registry (sync registries) → 3 org/repo → 4 mode → 5 [ci]
        Interaction    Enter keeps the current/default value; ESC at any step cancels (nothing written)",
        ),
        (
            "ci",
            "Output the docker-mtrans.yml workflow YAML template\n
        File           prints to stdout by default; with a file name (e.g. ci test.yaml) saves to it
        Overwrite      prompts for confirmation when the file already exists
        Deploy         commit it to the [ci].repo repo (must match the deployed workflow byte-for-byte)",
        ),
        ("version", "Show version information"),
        ("help", "Show concise subcommand list"),
        ("man", "Show full manual (detailed usage)"),
    ];
    for (name, about) in sub_abouts {
        if let Some(sub) = cmd.find_subcommand_mut(name) {
            let replaced = sub.clone().about(about);
            *sub = replaced;
        }
    }

    // 参数帮助（source_image 属于 sync/pull/spull；registry 属于 secret；file 属于 ci）
    let arg_helps: [(&str, &[&str], &str); 3] = [
        (
            "source_image",
            &["sync", "pull", "spull"],
            "Source image, e.g. ghcr.io/jetsung/shortener:latest",
        ),
        (
            "registry",
            &["secret"],
            "Registry domain; default target_registry",
        ),
        ("file", &["ci"], "Output file; default stdout"),
    ];
    for (id, sub_names, help) in arg_helps {
        for sub_name in sub_names {
            if let Some(sub) = cmd.find_subcommand_mut(sub_name) {
                let replaced = sub.clone().mut_arg(id, |a| a.help(help));
                *sub = replaced;
            }
        }
    }

    // 覆盖参数帮助（sync/pull/spull 共用四个参数定义）
    let override_helps: [(&str, &str); 4] = [
        (
            "registry",
            "Override [setting] target_registry for this run only (config file untouched)",
        ),
        (
            "org",
            "Override [setting] target_org for this run (takes precedence over [registries] entries)",
        ),
        (
            "repo",
            "Override [setting] target_repo for this run (takes precedence over [registries] entries)",
        ),
        ("mode", "Override [setting] mode for this run (1/2/3)"),
    ];
    for sub_name in ["sync", "pull", "spull"] {
        if let Some(sub) = cmd.find_subcommand_mut(sub_name) {
            for (id, help) in override_helps {
                let replaced = sub.clone().mut_arg(id, |a| a.help(help));
                *sub = replaced;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 运行时文案：中英文成对，中文与既有文案逐字一致
// ---------------------------------------------------------------------------

// --- ops.rs ---

pub fn target_image_label() -> &'static str {
    if is_zh() {
        "目标镜像:"
    } else {
        "Target image:"
    }
}

pub fn err_not_logged_in(registry: &str) -> String {
    if is_zh() {
        format!("本地未登录目标注册表 {registry}，请先执行 docker login {registry}")
    } else {
        format!("Not logged in to target registry {registry}; run `docker login {registry}` first")
    }
}

pub fn err_missing_auth(registry: &str) -> String {
    if is_zh() {
        format!("注册表 {registry} 的登录条目缺少 auth 字段，请重新执行 docker login {registry}")
    } else {
        format!(
            "The login entry for registry {registry} has no auth field; run `docker login {registry}` again"
        )
    }
}

pub fn err_target_missing(target: &str, source_image: &str) -> String {
    if is_zh() {
        format!(
            "目标注册表中不存在镜像 {target}，请先执行 docker mtrans sync {source_image} 同步后再拉取"
        )
    } else {
        format!(
            "Image {target} does not exist in the target registry; run `docker mtrans sync {source_image}` first, then pull"
        )
    }
}

pub fn err_probe_unauthorized(target: &str, registry: &str) -> String {
    if is_zh() {
        format!("无法访问目标镜像 {target}（不存在或未授权，请确认已 docker login {registry}）")
    } else {
        format!(
            "Cannot access target image {target} (missing or unauthorized; confirm you ran `docker login {registry}`)"
        )
    }
}

pub fn ok_pulled() -> &'static str {
    if is_zh() { "已拉取" } else { "Pulled" }
}

pub fn ok_renamed() -> &'static str {
    if is_zh() {
        "并重命名"
    } else {
        "and renamed it as"
    }
}

pub fn deleted_target_tag_label() -> &'static str {
    if is_zh() {
        "已删除本地目标标签:"
    } else {
        "Deleted local target tag:"
    }
}

pub fn secret_credential_label() -> &'static str {
    if is_zh() {
        "登录凭据加密串（target_auth_secret）:"
    } else {
        "Encrypted credential (target_auth_secret):"
    }
}

pub fn secret_keep_safe() -> &'static str {
    if is_zh() {
        "解密口令须与 GitHub Secret AUTH_PASSPHRASE 一致，请妥善保管"
    } else {
        "Decrypts with the passphrase matching GitHub Secret AUTH_PASSPHRASE; keep it safe"
    }
}

pub fn err_config_not_found(path: &std::path::Path) -> String {
    if is_zh() {
        format!(
            "未找到配置文件: {}，请先运行 docker mtrans config",
            path.display()
        )
    } else {
        format!(
            "Config file not found: {}; run `docker mtrans config` first",
            path.display()
        )
    }
}

pub fn import_no_new() -> &'static str {
    if is_zh() {
        "没有新注册表条目可导入（均已存在）"
    } else {
        "No new registry entries to import (all already exist)"
    }
}

pub fn err_read_file(path: &std::path::Path, e: &std::io::Error) -> String {
    if is_zh() {
        format!("读取 {} 失败: {e}", path.display())
    } else {
        format!("Failed to read {}: {e}", path.display())
    }
}

pub fn ok_synced() -> &'static str {
    if is_zh() { "已同步" } else { "Synced" }
}

pub fn err_no_docker_auths() -> &'static str {
    if is_zh() {
        "~/.docker/config.json 中没有任何注册表登录信息，请先执行 docker login"
    } else {
        "No registry logins found in ~/.docker/config.json; run `docker login` first"
    }
}

// --- main.rs ---

pub fn man_title() -> &'static str {
    if is_zh() {
        "docker mtrans 完整手册（各子命令详细用法；中英随系统语言）"
    } else {
        "docker mtrans full manual (detailed usage per subcommand)"
    }
}

pub fn trigger_params_label() -> &'static str {
    if is_zh() {
        "触发参数:"
    } else {
        "Trigger parameters:"
    }
}

pub fn ok_workflow_triggered() -> &'static str {
    if is_zh() {
        "已成功触发远程工作流"
    } else {
        "Remote workflow triggered successfully"
    }
}

pub fn err_github_api_unreachable(raw: &str) -> String {
    if is_zh() {
        format!("无法连接 GitHub API: {raw}")
    } else {
        format!("Cannot reach the GitHub API: {raw}")
    }
}

pub fn err_trigger_failed_http(status: u16, raw: &str) -> String {
    if is_zh() {
        format!("触发工作流失败（HTTP {status}）\n{raw}")
    } else {
        format!("Failed to trigger the workflow (HTTP {status})\n{raw}")
    }
}

pub fn err_trigger_failed_hint(hint: &str, raw: &str) -> String {
    if is_zh() {
        format!("触发工作流失败（{hint}）\n{raw}")
    } else {
        format!("Failed to trigger the workflow ({hint})\n{raw}")
    }
}

pub fn trigger_hint(status: u16) -> &'static str {
    match status {
        401 => {
            if is_zh() {
                "Token 无效或已过期，请检查 config.toml 的 github_token"
            } else {
                "Token invalid or expired; check github_token in config.toml"
            }
        }
        403 => {
            if is_zh() {
                "Token 权限不足，需要对该仓库具备 actions: write 权限"
            } else {
                "Insufficient token permissions; actions:write on the repo is required"
            }
        }
        404 => {
            if is_zh() {
                "仓库不存在或不可见、工作流文件不存在，请检查 config.toml 的 repo / workflow"
            } else {
                "Repo or workflow file not found/invisible; check repo / workflow in config.toml"
            }
        }
        422 => {
            if is_zh() {
                "请求不合法：分支不存在，或 inputs 与工作流定义不匹配"
            } else {
                "Invalid request: branch missing, or inputs do not match the workflow definition"
            }
        }
        _ => "",
    }
}

pub fn err_repo_format(repo: &str) -> String {
    if is_zh() {
        format!("配置项 repo 格式应为 owner/repo，例如 jetsung/docker-build-sync，当前值: {repo}")
    } else {
        format!(
            "Config `repo` must be owner/repo, e.g. jetsung/docker-build-sync; current value: {repo}"
        )
    }
}

pub fn err_missing_target_auth() -> &'static str {
    if is_zh() {
        "缺少目标注册表凭据：请确认已 docker login 目标注册表（~/.docker/config.json 的 auths 下存在该注册表条目且含 auth 字段）"
    } else {
        "Missing target registry credential: confirm you ran `docker login` on the target registry (an entry with an auth field exists under auths in ~/.docker/config.json)"
    }
}

pub fn err_missing_auth_passphrase() -> &'static str {
    if is_zh() {
        "缺少加密口令：请在 config.toml 的 [setting] 下设置 auth_passphrase（须与目标仓库的 GitHub Secret AUTH_PASSPHRASE 一致）"
    } else {
        "Missing auth passphrase: set auth_passphrase under [setting] in config.toml (must match the GitHub Secret AUTH_PASSPHRASE on the target repo)"
    }
}

pub fn err_encrypt(e: impl std::fmt::Display) -> String {
    if is_zh() {
        format!("凭据加密失败：{e}")
    } else {
        format!("Failed to encrypt the credential: {e}")
    }
}

pub fn err_decrypt(e: impl std::fmt::Display) -> String {
    if is_zh() {
        format!("凭据解密失败（密钥或密文不匹配）：{e}")
    } else {
        format!("Failed to decrypt the credential (key or ciphertext mismatch): {e}")
    }
}

pub fn err_missing_github_token() -> &'static str {
    if is_zh() {
        "缺少 GitHub Token（config.toml 的 github_token 项），Token 需具备目标仓库 actions: write 权限"
    } else {
        "Missing GitHub Token (config.toml `github_token`); the token needs actions:write on the target repo"
    }
}

pub fn run_status_page_label() -> &'static str {
    if is_zh() {
        "运行状态页:"
    } else {
        "Run status page:"
    }
}

pub fn wait_run_complete_hint() -> &'static str {
    if is_zh() {
        "等待运行完成，复制成功后自动删除该次流水线..."
    } else {
        "Waiting for the run to finish; the pipeline run is deleted automatically after a successful copy..."
    }
}

pub fn err_wait_new_run_timeout() -> &'static str {
    if is_zh() {
        "等待运行启动超时（可稍后到运行状态页确认，或把 config.toml 的 keep_run 设为 true）"
    } else {
        "Timed out waiting for the run to start (check the run status page later, or set keep_run = true in config.toml)"
    }
}

pub fn err_wait_run_complete_timeout() -> &'static str {
    if is_zh() {
        "等待运行完成超时，可稍后手动处理"
    } else {
        "Timed out waiting for the run to finish; handle it manually later"
    }
}

pub fn err_run_unsuccessful(conclusion: &str) -> String {
    if is_zh() {
        format!("运行未成功（conclusion: {conclusion}），保留流水线记录供排查")
    } else {
        format!(
            "Run did not succeed (conclusion: {conclusion}); the pipeline record is kept for troubleshooting"
        )
    }
}

pub fn ok_copy_deleted() -> &'static str {
    if is_zh() {
        "复制成功，已删除该次流水线运行"
    } else {
        "Copy succeeded; the pipeline run has been deleted"
    }
}

pub fn ok_copy_done() -> &'static str {
    if is_zh() {
        "复制成功"
    } else {
        "Copy succeeded"
    }
}

pub fn err_delete_run_failed() -> &'static str {
    if is_zh() {
        "删除流水线运行失败（Token 需具备 actions: write 权限）"
    } else {
        "Failed to delete the pipeline run (the token needs actions:write)"
    }
}

// --- docker.rs ---

pub fn err_invalid_docker_host(e: &impl std::fmt::Display) -> String {
    if is_zh() {
        format!("DOCKER_HOST 无效: {e}")
    } else {
        format!("Invalid DOCKER_HOST: {e}")
    }
}

pub fn err_context_missing(name: &str) -> String {
    if is_zh() {
        format!("DOCKER_CONTEXT 指定的 context 不存在: {name}")
    } else {
        format!("Context from DOCKER_CONTEXT does not exist: {name}")
    }
}

pub fn warn_invalid_context_endpoint(name: &str, e: &impl std::fmt::Display) -> String {
    if is_zh() {
        format!("警告: context {name} 的端点无效: {e}")
    } else {
        format!("Warning: invalid endpoint for context {name}: {e}")
    }
}

pub fn err_create_docker_client(host: &str, e: &impl std::fmt::Display) -> String {
    if is_zh() {
        format!("无法创建 Docker 客户端（{host}）: {e}")
    } else {
        format!("Cannot create Docker client ({host}): {e}")
    }
}

pub fn err_no_home_dir() -> &'static str {
    if is_zh() {
        "无法确定 HOME 目录"
    } else {
        "Cannot determine the HOME directory"
    }
}

pub fn err_docker_config_not_found(path: &std::path::Path) -> String {
    if is_zh() {
        format!(
            "未找到 Docker 配置文件: {}（请先执行 docker login）",
            path.display()
        )
    } else {
        format!(
            "Docker config file not found: {} (run `docker login` first)",
            path.display()
        )
    }
}

pub fn pull_label() -> &'static str {
    if is_zh() { "拉取" } else { "Pulling" }
}

pub fn layers_unit() -> &'static str {
    if is_zh() { "层" } else { "layers" }
}

pub fn err_pull_failed(image: &str, e: &impl std::fmt::Display) -> String {
    if is_zh() {
        format!("拉取 {image} 失败: {e}")
    } else {
        format!("Failed to pull {image}: {e}")
    }
}

pub fn err_tag_failed(source: &str, target: &str, e: &impl std::fmt::Display) -> String {
    if is_zh() {
        format!("打标 {source} -> {target} 失败: {e}")
    } else {
        format!("Failed to tag {source} -> {target}: {e}")
    }
}

pub fn err_remove_failed(image: &str, e: &impl std::fmt::Display) -> String {
    if is_zh() {
        format!("删除 {image} 失败: {e}")
    } else {
        format!("Failed to remove {image}: {e}")
    }
}

pub fn err_probe_failed(image: &str, e: &impl std::fmt::Display) -> String {
    if is_zh() {
        format!("查询注册表镜像 {image} 失败: {e}")
    } else {
        format!("Failed to query registry image {image}: {e}")
    }
}

// --- image.rs ---

pub fn err_digest_unsupported(image: &str) -> String {
    if is_zh() {
        format!("暂不支持 digest 形式的镜像引用: {image}，请改用 name:tag 形式")
    } else {
        format!("Digest image references are not supported yet: {image}; use name:tag form")
    }
}

pub fn err_empty_image_name(source_image: &str) -> String {
    if is_zh() {
        format!("无法从 source_image 提取镜像名，当前值: {source_image}")
    } else {
        format!("Cannot extract an image name from source_image; current value: {source_image}")
    }
}

pub fn err_unknown_mode(mode: u8) -> String {
    if is_zh() {
        format!("未知的组合模式 mode = {mode}（仅支持 1、2、3）")
    } else {
        format!("Unknown compose mode: mode = {mode} (only 1, 2, 3 are supported)")
    }
}

// --- config.rs ---

pub fn err_no_config_dir() -> &'static str {
    if is_zh() {
        "无法确定配置目录（缺少 XDG_CONFIG_HOME，且当前平台无标准配置目录）"
    } else {
        "Cannot determine the config directory (XDG_CONFIG_HOME missing and no platform config directory)"
    }
}

pub fn warn_read_failed(path: &std::path::Path, e: &std::io::Error) -> String {
    if is_zh() {
        format!("警告: 读取 {} 失败: {e}", path.display())
    } else {
        format!("Warning: failed to read {}: {e}", path.display())
    }
}

pub fn warn_toml_parse_failed(path: &std::path::Path, e: &toml::de::Error) -> String {
    if is_zh() {
        format!("警告: 读取 {} 失败（TOML 解析错误）: {e}", path.display())
    } else {
        format!(
            "Warning: failed to read {} (TOML parse error): {e}",
            path.display()
        )
    }
}

pub fn err_create_dir_failed(dir: &std::path::Path, e: &std::io::Error) -> String {
    if is_zh() {
        format!("创建 {} 失败: {e}", dir.display())
    } else {
        format!("Failed to create {}: {e}", dir.display())
    }
}

pub fn err_write_file(path: &std::path::Path, e: &std::io::Error) -> String {
    if is_zh() {
        format!("写入 {} 失败: {e}", path.display())
    } else {
        format!("Failed to write {}: {e}", path.display())
    }
}

// --- main.rs ---

pub fn err_prefix() -> &'static str {
    if is_zh() { "错误: " } else { "Error: " }
}

// --- ci.rs ---

pub fn confirm_overwrite(path: &std::path::Path) -> String {
    if is_zh() {
        format!("文件 {} 已存在，是否覆盖？", path.display())
    } else {
        format!("File {} already exists. Overwrite?", path.display())
    }
}

pub fn cancelled_no_write() -> &'static str {
    if is_zh() {
        "已取消，未写入文件"
    } else {
        "Cancelled; nothing was written"
    }
}

pub fn err_current_dir(e: &std::io::Error) -> String {
    if is_zh() {
        format!("无法获取当前目录: {e}")
    } else {
        format!("Cannot determine the current directory: {e}")
    }
}

pub fn err_write_failed(path: &std::path::Path, e: &std::io::Error) -> String {
    if is_zh() {
        format!("写入 {} 失败: {e}", path.display())
    } else {
        format!("Failed to write {}: {e}", path.display())
    }
}

pub fn ok_written(path: &std::path::Path) -> String {
    if is_zh() {
        format!("已写入: {}", path.display())
    } else {
        format!("Written: {}", path.display())
    }
}

// --- wizard.rs ---

pub fn err_interactive(e: &impl std::fmt::Display) -> String {
    if is_zh() {
        format!("交互失败: {e}")
    } else {
        format!("Interactive session failed: {e}")
    }
}

pub fn err_cancelled() -> &'static str {
    if is_zh() { "已取消" } else { "Cancelled" }
}

pub fn err_needs_tty() -> &'static str {
    if is_zh() {
        "需要交互式终端"
    } else {
        "An interactive terminal is required"
    }
}

pub fn current_registry_label() -> &'static str {
    if is_zh() {
        "当前使用注册表:"
    } else {
        "Current registry:"
    }
}

pub fn current_registry_unset() -> &'static str {
    if is_zh() {
        "当前使用注册表: 未设置（请从已登录注册表中选择）"
    } else {
        "Current registry: not set (pick one from the logged-in registries)"
    }
}

pub fn select_target_registry() -> &'static str {
    if is_zh() {
        "选择目标注册表"
    } else {
        "Select the target registry"
    }
}

pub fn config_wizard_title() -> &'static str {
    if is_zh() {
        "mtrans 配置向导：逐步输入，回车保持原值或默认值"
    } else {
        "mtrans config wizard: answer step by step; press Enter to keep the current or default value"
    }
}

pub fn config_file_label() -> &'static str {
    if is_zh() {
        "配置文件:"
    } else {
        "Config file:"
    }
}

pub fn ok_switched_registry() -> &'static str {
    if is_zh() {
        "已切换目标注册表:"
    } else {
        "Target registry switched:"
    }
}

pub fn no_new_registry_entries() -> &'static str {
    if is_zh() {
        "注册表条目无新增"
    } else {
        "No new registry entries"
    }
}

pub fn list_separator() -> &'static str {
    if is_zh() { "、" } else { ", " }
}

pub fn ok_wizard_written() -> &'static str {
    if is_zh() {
        "已写入配置:"
    } else {
        "Config written:"
    }
}

pub fn err_validate_org() -> &'static str {
    if is_zh() {
        "org 需为 4~30 个字母与数字（Use 4 to 30 letters & digits only.）"
    } else {
        "org must be 4 to 30 letters and digits only"
    }
}

pub fn err_validate_repo() -> &'static str {
    if is_zh() {
        "repo 只能含字母、数字与单个连字符分隔（如 my-repo；- 不可连续、不可在首尾）"
    } else {
        "repo may contain letters and digits separated by single hyphens (e.g. my-repo; no leading/trailing or consecutive hyphens)"
    }
}

pub fn prompt_org_field(current: &str) -> String {
    if is_zh() {
        format!("target_org（必填，4~30 个字母与数字；回车保持 \"{current}\"）")
    } else {
        format!(
            "target_org (required, 4 to 30 letters & digits; press Enter to keep \"{current}\")"
        )
    }
}

pub fn prompt_repo_field(show: &str) -> String {
    if is_zh() {
        format!("target_repo（回车保持 \"{show}\"；留空合法，填入仅限字母数字）")
    } else {
        format!(
            "target_repo (press Enter to keep \"{show}\"; may be left empty, letters and digits only)"
        )
    }
}

pub fn prompt_named_field(name: &str, show: &str) -> String {
    if is_zh() {
        format!("{name}（回车保持 {show}）")
    } else {
        format!("{name} (press Enter to keep {show})")
    }
}

pub fn mode_item_1(org: &str, repo: &str) -> String {
    if is_zh() {
        format!("1 单仓库汇聚：源镜像都推进 {org}/{repo}，源镜像名作 tag，源 tag 丢弃")
    } else {
        format!(
            "1 Single repo sink: all source images are pushed to {org}/{repo}; the source image name becomes the tag and the source tag is dropped"
        )
    }
}

pub fn mode_item_2(org: &str) -> String {
    if is_zh() {
        format!("2 仓库映射：{org} 下按源镜像名建仓库，tag 沿用源 tag")
    } else {
        format!(
            "2 Repo mapping: one repo per source image name under {org}; the source tag is kept"
        )
    }
}

pub fn mode_item_3(org: &str, repo: &str) -> String {
    if is_zh() {
        format!("3 固定中转：一律推到 {org}/{repo}:mtrans（固定 tag），target_repo 缺省时用 mtrans")
    } else {
        format!(
            "3 Fixed transit: everything is pushed to {org}/{repo}:mtrans (fixed tag); mtrans is used when target_repo is empty"
        )
    }
}

pub fn prompt_mode_select() -> &'static str {
    if is_zh() {
        "组合模式 mode"
    } else {
        "Compose mode"
    }
}

pub fn prompt_auth_passphrase(current: &str) -> String {
    // 不嵌入口令值：长值会撑爆单行导致 Select 重绘残留（内容重复）；
    // 值本身在选项确认后由「加密口令已确认」回显。
    if current.is_empty() {
        if is_zh() {
            "加密口令 auth_passphrase（未设置；加密登录凭据用，须与 GitHub Secret AUTH_PASSPHRASE 一致）".to_string()
        } else {
            "auth_passphrase (not set; encrypts the login credential, must match GitHub Secret AUTH_PASSPHRASE)".to_string()
        }
    } else if is_zh() {
        "加密口令 auth_passphrase（已设置；回车保持，选项可修改）".to_string()
    } else {
        "auth_passphrase (set; press Enter to keep, see options below)".to_string()
    }
}

pub fn passphrase_keep() -> &'static str {
    if is_zh() {
        "保持原值"
    } else {
        "Keep current value"
    }
}

pub fn passphrase_generate() -> &'static str {
    if is_zh() {
        "自动生成随机口令"
    } else {
        "Generate a random passphrase"
    }
}

pub fn passphrase_input() -> &'static str {
    if is_zh() {
        "手动输入"
    } else {
        "Enter manually"
    }
}

pub fn passphrase_set_ok() -> &'static str {
    if is_zh() {
        "加密口令已确认:"
    } else {
        "Auth passphrase confirmed:"
    }
}

pub fn passphrase_value_label() -> &'static str {
    if is_zh() {
        "加密口令（同步到 GitHub Secret AUTH_PASSPHRASE）:"
    } else {
        "Auth passphrase (sync to GitHub Secret AUTH_PASSPHRASE):"
    }
}

pub fn prompt_passphrase_manual() -> String {
    // 同样不嵌入口令值，避免行编辑重绘超宽
    if is_zh() {
        "auth_passphrase（输入加密口令，必填；ESC 取消向导）".to_string()
    } else {
        "auth_passphrase (type the passphrase, required; ESC to cancel)".to_string()
    }
}

pub fn err_passphrase_required() -> &'static str {
    if is_zh() {
        "加密口令不能为空：请输入一个值，或回到上级选择「自动生成随机口令」"
    } else {
        "Passphrase must not be empty: type a value, or go back and choose \"Generate a random passphrase\""
    }
}

pub fn keep_run_keep() -> &'static str {
    if is_zh() {
        "保持原值"
    } else {
        "Keep current value"
    }
}

pub fn keep_run_false() -> &'static str {
    if is_zh() {
        "false（复制成功后删除流水线记录，默认）"
    } else {
        "false (delete the pipeline record after a successful copy; default)"
    }
}

pub fn keep_run_true() -> &'static str {
    if is_zh() {
        "true（保留流水线记录）"
    } else {
        "true (keep the pipeline record)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 是否含 CJK 统一表意文字（运行时英文文案不应出现）。
    fn contains_cjk(s: &str) -> bool {
        s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
    }

    /// env 修改需串行；本组测试是唯一改 LANG 的地方。
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_lang(lang: &str, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var("LANG").ok();
        // edition 2024：set_var 为 unsafe；测试进程内单线程持锁修改
        unsafe { std::env::set_var("LANG", lang) };
        f();
        match prev {
            Some(v) => unsafe { std::env::set_var("LANG", v) },
            None => unsafe { std::env::set_var("LANG", "") },
        }
    }

    #[test]
    fn english_runtime_messages_have_no_cjk() {
        with_lang("en_US.UTF-8", || {
            assert!(!contains_cjk(&err_not_logged_in("r.example.com")));
            assert!(!contains_cjk(&err_missing_auth("r.example.com")));
            assert!(!contains_cjk(&err_target_missing("t:1", "s:1")));
            assert!(!contains_cjk(&err_probe_unauthorized(
                "t:1",
                "r.example.com"
            )));
            assert!(!contains_cjk(&err_config_not_found(std::path::Path::new(
                "/x"
            ))));
            assert!(!contains_cjk(&err_read_file(
                std::path::Path::new("/x"),
                &std::io::Error::other("boom")
            )));
            assert!(!contains_cjk(err_no_docker_auths()));
            assert!(!contains_cjk(&err_repo_format("bad")));
            assert!(!contains_cjk(err_missing_target_auth()));
            assert!(!contains_cjk(err_missing_github_token()));
            assert!(!contains_cjk(&err_run_unsuccessful("failure")));
            assert!(!contains_cjk(err_delete_run_failed()));
            assert!(!contains_cjk(&err_pull_failed("img", &"x")));
            assert!(!contains_cjk(&err_tag_failed("a", "b", &"x")));
            assert!(!contains_cjk(&err_remove_failed("img", &"x")));
            assert!(!contains_cjk(&err_probe_failed("img", &"x")));
            assert!(!contains_cjk(&err_digest_unsupported("a@sha256:1")));
            assert!(!contains_cjk(&err_empty_image_name("x")));
            assert!(!contains_cjk(&err_unknown_mode(9)));
            assert!(!contains_cjk(err_prefix()));
            assert!(!contains_cjk(&confirm_overwrite(std::path::Path::new(
                "/x"
            ))));
            assert!(!contains_cjk(cancelled_no_write()));
            assert!(!contains_cjk(&err_current_dir(&std::io::Error::other(
                "boom"
            ))));
            assert!(!contains_cjk(&err_write_failed(
                std::path::Path::new("/x"),
                &std::io::Error::other("boom")
            )));
            assert!(!contains_cjk(&ok_written(std::path::Path::new("/x"))));
            assert!(!contains_cjk(&err_interactive(&"boom")));
            assert!(!contains_cjk(err_cancelled()));
            assert!(!contains_cjk(err_needs_tty()));
            assert!(!contains_cjk(current_registry_label()));
            assert!(!contains_cjk(current_registry_unset()));
            assert!(!contains_cjk(select_target_registry()));
            assert!(!contains_cjk(config_wizard_title()));
            assert!(!contains_cjk(config_file_label()));
            assert!(!contains_cjk(ok_switched_registry()));
            assert!(!contains_cjk(no_new_registry_entries()));
            assert!(!contains_cjk(list_separator()));
            assert!(!contains_cjk(ok_wizard_written()));
            assert!(!contains_cjk(err_validate_org()));
            assert!(!contains_cjk(err_validate_repo()));
            assert!(!contains_cjk(&prompt_org_field("old")));
            assert!(!contains_cjk(&prompt_repo_field("old")));
            assert!(!contains_cjk(&prompt_named_field("ci.repo", "old")));
            assert!(!contains_cjk(&mode_item_1("org", "repo")));
            assert!(!contains_cjk(&mode_item_2("org")));
            assert!(!contains_cjk(&mode_item_3("org", "repo")));
            assert!(!contains_cjk(prompt_mode_select()));
            assert!(!contains_cjk(keep_run_keep()));
            assert!(!contains_cjk(keep_run_false()));
            assert!(!contains_cjk(keep_run_true()));
        });
    }

    #[test]
    fn chinese_runtime_messages_unchanged() {
        with_lang("zh_CN.UTF-8", || {
            assert_eq!(
                err_not_logged_in("r.example.com"),
                "本地未登录目标注册表 r.example.com，请先执行 docker login r.example.com"
            );
            assert_eq!(
                err_target_missing("t:1", "s:1"),
                "目标注册表中不存在镜像 t:1，请先执行 docker mtrans sync s:1 同步后再拉取"
            );
            assert_eq!(err_prefix(), "错误: ");
            assert_eq!(ok_pulled(), "已拉取");
            assert_eq!(target_image_label(), "目标镜像:");
        });
    }
}
