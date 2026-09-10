//! mtrans：docker CLI 插件入口。
//!
//! docker 以 `docker-mtrans mtrans sync -s foo` 形态调用插件：argv[1] 是子命令名 `mtrans`，
//! 解析前必须剥掉；并容忍 docker 透传的全局 flag。发现协议（`docker-cli-plugin-metadata`）
//! 排在一切解析之前——发现阶段无守护进程可用，不读配置、不联网。

use clap::{CommandFactory, FromArgMatches, Parser as _};

use mtrans::ci;
use mtrans::cli::{Cli, Command};
use mtrans::config;
use mtrans::i18n;
use mtrans::ops;
use mtrans::ui;
use mtrans::wizard;

/// 插件发现元数据（按实测 compose 的最小四键形状）。
/// ShortDescription 按系统语言切换（与 help/运行时输出同口径）；Vendor 为作者英文名。
fn plugin_metadata() -> String {
    let desc = if i18n::is_zh() {
        "Docker 镜像复制"
    } else {
        "Docker image transfer"
    };
    format!(
        "{{\"SchemaVersion\":\"0.1.0\",\"Vendor\":\"Jetsung Chan\",\"Version\":\"{}\",\"ShortDescription\":\"{desc}\"}}",
        env!("CARGO_PKG_VERSION")
    )
}

/// 剥掉 docker 注入的前导 `mtrans`，并忽略子命令 token 之前的 docker 全局 flag
/// （`--debug`、`--log-level`、`--trace` 等）。子命令之后的参数保持严格解析。
fn preprocess_args(raw: &[String]) -> Vec<String> {
    let mut idx = 0usize;
    if raw.first().map(String::as_str) == Some("mtrans") {
        idx = 1;
    }
    while idx < raw.len() {
        let token = raw[idx].as_str();
        match token {
            "--debug" | "--trace" | "--tls" | "--tlsv" | "--tlsverify" => {
                idx += 1;
                continue;
            }
            "--log-level" | "--config" | "--host" | "-H" | "-l" => {
                // 带值 flag：连同其值一起跳过
                idx = (idx + 2).min(raw.len());
                continue;
            }
            _ => {}
        }
        if token.starts_with("--log-level=")
            || token.starts_with("--config=")
            || token.starts_with("--host=")
        {
            idx += 1;
            continue;
        }
        break;
    }
    raw[idx..].to_vec()
}

#[tokio::main]
async fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    // 插件发现协议：单个参数，原样打印元数据 JSON
    if raw.len() == 1 && raw[0] == "docker-cli-plugin-metadata" {
        println!("{}", plugin_metadata());
        return;
    }

    let args = preprocess_args(&raw);

    // 空参数（裸 `docker mtrans`）：按系统语言打印对应帮助并退出
    if args.is_empty() {
        if i18n::is_zh() {
            // parse_from 遇 --help 直接打印中文属性默认文案并 exit 0
            Cli::parse_from(["docker-mtrans", "--help"]);
        } else {
            print!("{}", full_help());
        }
        return;
    }

    // 若含 `help` / `man` / `version` 子命令或 `-h/--help`，按系统语言本地化后再解析
    let help_wanted = args
        .iter()
        .any(|a| a == "help" || a == "man" || a == "version" || a == "-h" || a == "--help");
    let cli = if help_wanted {
        let mut cmd = Cli::command();
        i18n::localize(&mut cmd);
        Cli::from_arg_matches_mut(
            &mut cmd
                .get_matches_from(std::iter::once("docker-mtrans".to_string()).chain(args.clone())),
        )
        .unwrap_or_else(|e| e.exit())
    } else {
        Cli::parse_from(std::iter::once("docker-mtrans".to_string()).chain(args))
    };
    std::process::exit(dispatch(cli).await);
}

/// 完整帮助：返回顶层 `--help` 的渲染输出（含全部子命令摘要）。
fn full_help() -> String {
    let mut cmd = Cli::command();
    i18n::localize(&mut cmd);
    // 用 --help 触发渲染，捕获 clap 输出
    let mut out = Vec::new();
    let _ = cmd.write_help(&mut out);
    String::from_utf8(out).unwrap_or_default()
}

/// 详细手册：各子命令 `--help` 逐节展开，节间横线分隔、带页眉编号。
fn man_help() -> String {
    let names: [&str; 10] = [
        "sync",
        "pull",
        "spull",
        "secret",
        "import",
        "registry",
        "passphrase",
        "config",
        "ci",
        "version",
    ];
    let total = names.len();
    let rule = "─".repeat(72);

    let mut out = String::new();
    out.push_str(&ui::bold(i18n::man_title()));
    out.push('\n');

    for (i, n) in names.iter().enumerate() {
        let mut cmd = Cli::command();
        i18n::localize(&mut cmd);
        let Some(sub) = cmd.find_subcommand_mut(n) else {
            continue;
        };
        let mut buf = Vec::new();
        let _ = sub.write_help(&mut buf);
        let text = String::from_utf8(buf).unwrap_or_default();

        // 页眉：docker mtrans <命令> (i/N)，首行后跟横线
        let header = format!("docker mtrans {n} ({}/{total})", i + 1);
        out.push('\n');
        out.push_str(&rule);
        out.push('\n');
        out.push_str(&ui::bold(&ui::step(&header)));
        out.push('\n');
        out.push_str(&rule);
        out.push('\n');
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out
}

/// sync/pull/spull 共用的覆盖参数（spull 两步共用同一份合并配置）。
#[derive(Clone, Copy, Default)]
struct Overrides<'a> {
    registry: Option<&'a str>,
    org: Option<&'a str>,
    repo: Option<&'a str>,
    mode: Option<u8>,
}

impl Overrides<'_> {
    fn apply(&self, cfg: &config::Config) -> Result<config::Config, String> {
        cfg.with_overrides(self.registry, self.org, self.repo, self.mode)
    }
}

async fn dispatch(cli: Cli) -> i32 {
    let result: Result<(), String> = match cli.command {
        Command::Sync {
            source_image,
            registry,
            org,
            repo,
            mode,
        } => {
            let o = Overrides {
                registry: registry.as_deref(),
                org: org.as_deref(),
                repo: repo.as_deref(),
                mode,
            };
            match o.apply(&config::load()) {
                Ok(cfg) => ops::sync(&source_image, &cfg).await,
                Err(e) => Err(e),
            }
        }
        Command::Pull {
            source_image,
            registry,
            org,
            repo,
            mode,
        } => {
            let o = Overrides {
                registry: registry.as_deref(),
                org: org.as_deref(),
                repo: repo.as_deref(),
                mode,
            };
            match o.apply(&config::load()) {
                Ok(cfg) => ops::pull(&source_image, &cfg).await,
                Err(e) => Err(e),
            }
        }
        Command::SyncPull {
            source_image,
            registry,
            org,
            repo,
            mode,
        } => {
            let o = Overrides {
                registry: registry.as_deref(),
                org: org.as_deref(),
                repo: repo.as_deref(),
                mode,
            };
            match o.apply(&config::load()) {
                Ok(cfg) => {
                    async {
                        ops::sync(&source_image, &cfg).await?;
                        ops::pull(&source_image, &cfg).await
                    }
                    .await
                }
                Err(e) => Err(e),
            }
        }
        Command::Secret { registry } => ops::secret(registry.as_deref(), &config::load()),
        Command::Import => ops::import(),
        Command::Registry => wizard::run_registry(),
        Command::Passphrase => wizard::run_passphrase(),
        Command::Config => wizard::run_config_wizard(),
        Command::Ci { file } => ci::generate(file.as_deref()),
        Command::Version => {
            // 输出形态参考 docker compose version
            println!("Docker Mtrans version v{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Help => {
            // 简洁帮助：只列出子命令一行说明（即顶层 --help 摘要）
            print!("{}", full_help());
            Ok(())
        }
        Command::Man => {
            // 完整帮助：打印各子命令详细用法（每子命令 --help 展开）
            print!("{}", man_help());
            Ok(())
        }
    };
    match result {
        Ok(()) => 0,
        Err(msg) => {
            eprintln!("{}{msg}", i18n::err_prefix());
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn strips_leading_mtrans_token() {
        assert_eq!(
            preprocess_args(&s(&["mtrans", "sync", "ghcr.io/a/b:1"])),
            s(&["sync", "ghcr.io/a/b:1"])
        );
        // 直接调用（不经 docker）不受影响
        assert_eq!(
            preprocess_args(&s(&["sync", "ghcr.io/a/b:1"])),
            s(&["sync", "ghcr.io/a/b:1"])
        );
    }

    #[test]
    fn drops_docker_global_flags_before_subcommand() {
        assert_eq!(
            preprocess_args(&s(&[
                "mtrans",
                "--debug",
                "--log-level",
                "debug",
                "sync",
                "ghcr.io/a/b:1"
            ])),
            s(&["sync", "ghcr.io/a/b:1"])
        );
        assert_eq!(
            preprocess_args(&s(&["--log-level=trace", "--trace", "sync"])),
            s(&["sync"])
        );
    }

    #[test]
    fn keeps_flags_after_subcommand() {
        // 子命令之后的 --debug 不是 docker 全局 flag，交给 clap 报错（严格）
        assert_eq!(
            preprocess_args(&s(&["mtrans", "sync", "--debug"])),
            s(&["sync", "--debug"])
        );
    }
}
