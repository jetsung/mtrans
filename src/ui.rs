//! 回显辅助：轻量级 ANSI 着色，让终端输出更直观。
//!
//! 仅对支持颜色的 TTY 启用；检测 `NO_COLOR` 环境变量或非 TTY 时自动降级为纯文本。
//! 不需要额外依赖，全部为内联转义序列。

use std::io::IsTerminal;

/// 当前是否启用颜色。
fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn paint(code: &str, s: &str) -> String {
    if color_enabled() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// 成功（绿色）。
pub fn ok(s: &str) -> String {
    paint("32", s)
}

/// 失败（红色）。
pub fn err(s: &str) -> String {
    paint("31", s)
}

/// 步骤/强调（青色）。
pub fn step(s: &str) -> String {
    paint("36", s)
}

/// 加粗。
pub fn bold(s: &str) -> String {
    paint("1", s)
}

/// 弱化/提示（暗色）。
pub fn dim(s: &str) -> String {
    paint("2", s)
}

/// 高亮（黄色）。
pub fn warn(s: &str) -> String {
    paint("33", s)
}
