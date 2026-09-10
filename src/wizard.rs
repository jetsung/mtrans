//! TUI 交互（dialoguer）：`registry` 与 `config` 四步向导。
//!
//! 交互约定：逐步输入，"回车"在已有值时保持原值，无原值时回退内置默认值；
//! 任意步骤按 ESC 取消（不写盘）；写回按行文本改写以保留注释与顺序（ADR-0004），
//! 配置文件不存在时用模板整体生成。

use std::collections::BTreeSet;

use dialoguer::console::{Key, Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};

use crate::config::{self, Config};
use crate::crypto;
use crate::docker;
use crate::i18n;
use crate::ops;
use crate::ui;

/// 覆盖/继续确认；Ctrl-C 或 EOF 视为否。
pub fn confirm(prompt: &str) -> bool {
    Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact_opt()
        .ok()
        .flatten()
        .unwrap_or(false)
}

fn select(prompt: &str, items: &[&str], default: usize) -> Result<usize, String> {
    // 选中行高亮为黄色，以区分选中与未选中项。
    let theme = ColorfulTheme {
        active_item_style: Style::new().for_stderr().yellow(),
        active_item_prefix: dialoguer::console::style("❯".to_string())
            .for_stderr()
            .yellow(),
        ..ColorfulTheme::default()
    };

    Select::with_theme(&theme)
        .with_prompt(prompt)
        .items(items)
        .default(default)
        .interact_opt()
        .map_err(|e| i18n::err_interactive(&e))?
        .ok_or_else(|| i18n::err_cancelled().to_string())
}

/// TUI 选择目标注册表（`registry` 命令与 `config` 向导共用）。
/// 候选为 `~/.docker/config.json` 已登录的注册表（键归一化后）。
/// `has_config` 表示配置文件是否已存在：初次（尚无配置）时当前注册表显示"未设置"，
/// 避免把内置默认域名当成"当前使用"误导用户。
pub fn prompt_target_registry(cfg: &mut Config, has_config: bool) -> Result<(), String> {
    let docker_config = docker::read_docker_config()?;
    let names: Vec<String> = docker::auth_keys(&docker_config)
        .into_iter()
        .map(|k| docker::normalize_registry_key(&k))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if names.is_empty() {
        return Err(i18n::err_no_docker_auths().to_string());
    }
    let current = cfg.effective_registry();
    if !has_config || current.is_empty() {
        println!("{}", ui::step(i18n::current_registry_unset()));
    } else {
        println!(
            "{} {}",
            ui::step(i18n::current_registry_label()),
            ui::bold(&current)
        );
    }
    let idx = select(
        i18n::select_target_registry(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
        names.iter().position(|n| n == &current).unwrap_or(0),
    )?;
    cfg.target_registry = names[idx].clone();
    Ok(())
}

/// `registry` 命令：切换目标注册表并同步 `[registries]` 条目。
pub fn run_registry() -> Result<(), String> {
    let path = config::config_path()?;
    if !path.is_file() {
        return Err(i18n::err_config_not_found(&path));
    }
    let mut cfg = config::load();
    prompt_target_registry(&mut cfg, true)?;
    let added = ops::sync_registries(&mut cfg)?;
    let mut body = std::fs::read_to_string(&path).map_err(|e| i18n::err_read_file(&path, &e))?;
    body = config::rewrite_section_key(&body, "[setting]", "target_registry", &cfg.target_registry);
    for name in &added {
        body = config::append_registry_entry(&body, name);
    }
    config::write(&body)?;
    println!(
        "{} {}",
        ui::ok(i18n::ok_switched_registry()),
        ui::bold(&cfg.target_registry)
    );
    if added.is_empty() {
        println!("{}", ui::dim(i18n::no_new_registry_entries()));
    } else {
        println!(
            "{} {}: {}",
            ui::ok(i18n::ok_synced()),
            added.len(),
            ui::step(&added.join(i18n::list_separator()))
        );
    }
    Ok(())
}

/// `passphrase` 命令：独立设置加密口令（对齐 `run_registry` 的方式）。
/// 复用 `config` 向导第 1 步的交互（保持 / 自动生成 / 手输，ESC 取消），
/// 结果按行改写进 `[setting].auth_passphrase`；配置文件不存在时报错（先 `config` 向导初始化）。
pub fn run_passphrase() -> Result<(), String> {
    let path = config::config_path()?;
    if !path.is_file() {
        return Err(i18n::err_config_not_found(&path));
    }
    let mut cfg = config::load();
    cfg.auth_passphrase = prompt_auth_passphrase(&cfg.auth_passphrase)?;
    let body = std::fs::read_to_string(&path).map_err(|e| i18n::err_read_file(&path, &e))?;
    let body =
        config::rewrite_section_key(&body, "[setting]", "auth_passphrase", &cfg.auth_passphrase);
    config::write(&body)?;
    println!("{} {}", ui::ok(i18n::ok_wizard_written()), path.display());
    println!(
        "{} {}",
        ui::step(i18n::passphrase_value_label()),
        ui::bold(&cfg.auth_passphrase)
    );
    Ok(())
}

/// `config` 命令：四步 TUI 向导。
/// auth_passphrase -> target_registry（同步 registries）-> target_org/target_repo -> mode -> [ci]。
pub fn run_config_wizard() -> Result<(), String> {
    let path = config::config_path()?;
    let existing = path.is_file();
    let mut cfg = config::load();
    let mut body = if existing {
        std::fs::read_to_string(&path).map_err(|e| i18n::err_read_file(&path, &e))?
    } else {
        String::new()
    };

    println!("{}", ui::bold(i18n::config_wizard_title()));
    println!("{} {}", ui::step(i18n::config_file_label()), path.display());

    // 1. auth_passphrase：保持 / 自动生成 / 手输（空值且无旧值时循环；ESC 取消向导）
    cfg.auth_passphrase = prompt_auth_passphrase(&cfg.auth_passphrase)?;

    // 2. target_registry：从已登录注册表中选择，并同步 [registries]
    prompt_target_registry(&mut cfg, existing)?;
    let added = ops::sync_registries(&mut cfg)?;
    if existing {
        body = config::rewrite_section_key(
            &body,
            "[setting]",
            "target_registry",
            &cfg.target_registry,
        );
        for name in &added {
            body = config::append_registry_entry(&body, name);
        }
    }

    // 3. target_org（必填）/ target_repo：手输，回车保持；都校验合法性
    cfg.target_org = prompt_org(&cfg.target_org)?;
    cfg.target_repo = prompt_repo(&cfg.target_repo)?;

    // 4. mode：TUI 选择
    let modes = [
        i18n::mode_item_1(&cfg.target_org, &cfg.target_repo),
        i18n::mode_item_2(&cfg.target_org),
        i18n::mode_item_3(&cfg.target_org, &cfg.target_repo),
    ];
    let items: Vec<&str> = modes.iter().map(String::as_str).collect();
    let sel = select(
        i18n::prompt_mode_select(),
        &items,
        cfg.mode.saturating_sub(1).min(2) as usize,
    )?;
    cfg.mode = sel as u8 + 1;

    // 4. [ci] 各参数：逐项输入，回车保持
    cfg.repo = prompt_field("ci.repo", &cfg.repo, config::DEFAULT_CI_REPO)?;
    cfg.branch = prompt_field("ci.branch", &cfg.branch, config::DEFAULT_BRANCH)?;
    cfg.workflow = prompt_field("ci.workflow", &cfg.workflow, config::DEFAULT_WORKFLOW)?;
    cfg.keep_run = prompt_keep_run(cfg.keep_run)?;

    if existing {
        body = config::rewrite_section_key(&body, "[setting]", "target_org", &cfg.target_org);
        body = config::rewrite_section_key(&body, "[setting]", "target_repo", &cfg.target_repo);
        body = config::rewrite_section_key(
            &body,
            "[setting]",
            "auth_passphrase",
            &cfg.auth_passphrase,
        );
        body = config::rewrite_section_key(&body, "[setting]", "mode", &cfg.mode.to_string());
        body = config::rewrite_section_key(&body, "[ci]", "repo", &cfg.repo);
        body = config::rewrite_section_key(&body, "[ci]", "branch", &cfg.branch);
        body = config::rewrite_section_key(&body, "[ci]", "workflow", &cfg.workflow);
        body = config::rewrite_section_key(&body, "[ci]", "keep_run", &cfg.keep_run.to_string());
        config::write(&body)?;
    } else {
        config::write(&config::render_template(&cfg))?;
    }
    println!("{} {}", ui::ok(i18n::ok_wizard_written()), path.display());
    // 末尾回显加密口令（短标签，与写入配置同一行区块）
    println!(
        "{} {}",
        ui::step(i18n::passphrase_value_label()),
        ui::bold(&cfg.auth_passphrase)
    );
    Ok(())
}

/// 校验 org/repo 标识：4~30 个字母与数字（必填）。
fn validate_org(v: &str) -> Result<(), String> {
    if !(4..=30).contains(&v.len()) || !v.chars().all(|c| c.is_ascii_alphanumeric()) {
        Err(i18n::err_validate_org().to_string())
    } else {
        Ok(())
    }
}

/// 校验 repo 标识：字母数字，`-` 仅作单个分隔符（不可连续、不可在首尾）；
/// 空值合法，填入值须通过校验。
fn validate_repo(v: &str) -> Result<(), String> {
    if v.is_empty() {
        return Ok(());
    }
    // 按 `-` 分段：每段非空且全为字母数字 ⇒ 等价于 ^[a-zA-Z0-9]+(-[a-zA-Z0-9]+)*$
    let ok = !v.starts_with('-')
        && !v.ends_with('-')
        && !v.contains("--")
        && v.split('-')
            .all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric()));
    if ok {
        Ok(())
    } else {
        Err(i18n::err_validate_repo().to_string())
    }
}

/// 文本输入：`Enter` 确认、`ESC` 取消（返回 `Ok(None)`）。
///
/// 空输入且 `default` 非空时返回 `default`（对齐 dialoguer `Input` 的默认值语义）；
/// `default` 为空则返回空串，由调用方决定是否接受。
/// dialoguer 0.12 的 `Input` 无 `interact_opt` 变体且按键循环吞掉 ESC，故自绘：
/// 复用 `Term` 读键，单行行编辑（插入/退格/左右移动），每次按键先清整行再全量重绘
/// （`clear_line` → 重写提示与文本 → 左移 `len - pos` 归位），光标永不错位。
fn input_text_opt(prompt: &str, default: &str) -> Result<Option<String>, String> {
    use std::io;

    let term = Term::stderr();
    if !term.is_term() {
        return Err(i18n::err_needs_tty().to_string());
    }
    let prefix = ui::bold(&format!("? {prompt}"));

    // 清行后重写整行，光标定位到 pos 处。
    let redraw = |term: &Term, chars: &[char], pos: usize| -> io::Result<()> {
        let s: String = chars.iter().collect();
        term.clear_line()?;
        term.write_str(&prefix)?;
        term.write_str(": ")?;
        term.write_str(&s)?;
        // 输入内容限字母数字（ASCII），字符数即列数
        term.move_cursor_left(s.chars().count() - pos)?;
        term.flush()
    };

    let run = || -> io::Result<Option<String>> {
        redraw(&term, &[], 0)?;

        let mut chars: Vec<char> = Vec::new();
        let mut pos = 0usize;
        loop {
            match term.read_key()? {
                Key::Escape => {
                    // 清行后换行，避免与后续步骤输出混在同一行
                    term.clear_line()?;
                    term.write_line("")?;
                    term.flush()?;
                    return Ok(None);
                }
                Key::Enter => {
                    // 对齐 dialoguer Input：行内保留「提示: 值」并换行，
                    // 后续输出另起新行（不再清行导致与下一步输出混行）
                    term.write_line("")?;
                    term.flush()?;
                    if chars.is_empty() && !default.is_empty() {
                        return Ok(Some(default.to_string()));
                    }
                    return Ok(Some(chars.into_iter().collect()));
                }
                Key::Backspace if pos > 0 => {
                    chars.remove(pos - 1);
                    pos -= 1;
                    redraw(&term, &chars, pos)?;
                }
                Key::ArrowLeft if pos > 0 => {
                    pos -= 1;
                    term.move_cursor_left(1)?;
                    term.flush()?;
                }
                Key::ArrowRight if pos < chars.len() => {
                    pos += 1;
                    term.move_cursor_right(1)?;
                    term.flush()?;
                }
                Key::Char(chr) if !chr.is_ascii_control() => {
                    chars.insert(pos, chr);
                    pos += 1;
                    redraw(&term, &chars, pos)?;
                }
                _ => {}
            }
        }
    };
    run().map_err(|e| i18n::err_interactive(&e))
}

/// 必填文本字段：输入值须通过 `validate_org`，不合法时循环要求重输；ESC 取消。
/// 回车空输入时保持 `current`（若 `current` 为空则视为未输入，继续校验报错）。
fn prompt_org(current: &str) -> Result<String, String> {
    let prompt = i18n::prompt_org_field(current);
    loop {
        let Some(v) = input_text_opt(&prompt, current)? else {
            return Err(i18n::err_cancelled().to_string());
        };
        let v = v.trim().to_string();
        if let Err(e) = validate_org(&v) {
            println!("{}", ui::warn(&e));
            continue;
        }
        return Ok(v);
    }
}

/// 文本字段：回车保持原值；空值合法，填入值须通过 `validate_repo`，不合法时循环要求重输；ESC 取消。
fn prompt_repo(current: &str) -> Result<String, String> {
    let show = if current.is_empty() {
        config::DEFAULT_REPO
    } else {
        current
    };
    let prompt = i18n::prompt_repo_field(show);
    loop {
        let Some(v) = input_text_opt(&prompt, show)? else {
            return Err(i18n::err_cancelled().to_string());
        };
        let v = v.trim().to_string();
        if let Err(e) = validate_repo(&v) {
            println!("{}", ui::warn(&e));
            continue;
        }
        return Ok(v);
    }
}

/// 文本字段：回车保持原值，无原值时回退默认值；ESC 取消。
fn prompt_field(name: &str, current: &str, default: &str) -> Result<String, String> {
    let show = if current.is_empty() { default } else { current };
    let prompt = i18n::prompt_named_field(name, show);
    let Some(v) = input_text_opt(&prompt, show)? else {
        return Err(i18n::err_cancelled().to_string());
    };
    Ok(v)
}

/// 加密口令 `auth_passphrase` 向导步骤：TUI 三选一——保持原值（仅已设置时）/ 自动生成随机口令 / 手动输入。
///
/// - 「保持原值」或手输回车空输入且已有旧值 ⇒ 沿用旧值（情况 1：配置文件已存在则保存旧值）；
/// - 「自动生成」⇒ 调 `crypto::generate_passphrase` 生成并回显；
/// - 手输回车空输入且无旧值 ⇒ 告警后循环（情况 2：对齐 `prompt_org`，直至输入合法值）；
/// - 任意一步按 ESC ⇒ 取消整个向导（不写盘）。
fn prompt_auth_passphrase(current: &str) -> Result<String, String> {
    let value = loop {
        // 已有旧值时才有「保持原值」项；无旧值时默认选中「自动生成」
        let has_keep = !current.is_empty();
        let mut items: Vec<String> = Vec::new();
        if has_keep {
            items.push(i18n::passphrase_keep().to_string());
        }
        items.push(i18n::passphrase_generate().to_string());
        items.push(i18n::passphrase_input().to_string());
        let idx_keep = 0;
        let idx_generate = usize::from(has_keep);
        let idx_input = idx_generate + 1;
        let labels: Vec<&str> = items.iter().map(String::as_str).collect();
        let default = if has_keep { idx_keep } else { idx_generate };
        let idx = select(&i18n::prompt_auth_passphrase(current), &labels, default)?;

        if idx == idx_keep && has_keep {
            break current.to_string();
        }
        if idx == idx_input {
            // 手动输入：空输入时有旧值则保持，无旧值则告警循环
            let Some(v) = input_text_opt(&i18n::prompt_passphrase_manual(), current)? else {
                return Err(i18n::err_cancelled().to_string());
            };
            let v = v.trim().to_string();
            if v.is_empty() {
                if has_keep {
                    break current.to_string();
                }
                println!("{}", ui::warn(i18n::err_passphrase_required()));
                continue;
            }
            break v;
        }
        // 自动生成（idx == idx_generate）
        break crypto::generate_passphrase()?;
    };
    // 三条路径（保持 / 生成 / 手输）统一在此回显最终值
    println!("{} {}", ui::ok(i18n::passphrase_set_ok()), ui::bold(&value));
    Ok(value)
}

fn prompt_keep_run(current: bool) -> Result<bool, String> {
    let choices = [
        i18n::keep_run_keep(),
        i18n::keep_run_false(),
        i18n::keep_run_true(),
    ];
    match select(&i18n::prompt_named_field("ci.keep_run", ""), &choices, 0)? {
        1 => Ok(false),
        2 => Ok(true),
        _ => Ok(current),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_org_requires_4_to_30_alnum() {
        assert!(validate_org("ab").is_err(), "少于 4 位不合法");
        assert!(validate_org("abc").is_err(), "少于 4 位不合法");
        assert!(validate_org("abcd").is_ok(), "恰好 4 位合法");
        assert!(validate_org(&"a".repeat(30)).is_ok(), "恰好 30 位合法");
        assert!(validate_org(&"a".repeat(31)).is_err(), "超过 30 位不合法");
        assert!(validate_org("has-hyphen").is_err(), "连字符不合法");
        assert!(validate_org("has_under").is_err(), "下划线不合法");
        assert!(validate_org("").is_err(), "空值必填不合法");
        assert!(validate_org("Org123").is_ok(), "大小写混合合法");
    }

    #[test]
    fn validate_repo_allows_empty_but_checks_alnum() {
        assert!(validate_repo("").is_ok(), "空值合法");
        assert!(validate_repo("my-repo").is_ok(), "中间连字符合法");
        assert!(validate_repo("myRepo").is_ok(), "字母数字合法");
        assert!(validate_repo("mtrans").is_ok(), "纯字母合法");
        assert!(validate_repo("123").is_ok(), "纯数字合法");
        assert!(validate_repo("-myrepo").is_err(), "开头连字符不合法");
        assert!(validate_repo("myrepo-").is_err(), "结尾连字符不合法");
        assert!(validate_repo("my_repo").is_err(), "下划线不合法");
        assert!(validate_repo("my repo").is_err(), "空格不合法");
        assert!(validate_repo("my--repo").is_err(), "连续连字符不合法");
        assert!(validate_repo("a-b-c").is_ok(), "多个单个连字符分隔合法");
    }
}
