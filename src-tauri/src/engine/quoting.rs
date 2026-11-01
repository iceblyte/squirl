//! 逐 shell 脚本装配（设计 §5.3.2「转义只发生在适配器模板层」，INV-6）。
//!
//! 原则：`command_line` 原文作为事实记录直传，本模块只负责把它
//! 安全地「递进」目标 shell 的脚本参数里，不做任何二次解析或改写。
//! marker 输出插在脚本最前，用于把 profile 噪音与命令输出分离（F4）。
//!
//! M0 实测定版（docs/进度与验证记录.md Spike A 冒烟结论）：
//! - pwsh 保持打开 = `-NoExit -File <脚本>`：`exit 1` 不闪退（实机 T5 验证）；
//!   `-NoExit -Command` 会被命令中的 `exit` 关键字杀掉（T1/T2/T7），不可用。
//! - bash/zsh 保持打开 = `( <命令> ); exec <shell>` 子壳形态（T6 验证序列存活）。
//! - cmd 保持打开 = `/k`（天然返回提示符）。

/// marker 行文本（pwsh / bash / zsh）。前端日志面板按「包含
/// `squirl command output`」识别 marker，与其之前的 profile 段折叠分离。
pub const MARKER_TEXT: &str = "──── squirl command output ────";

/// cmd 用 ASCII marker：默认控制台代码页下 Unicode 破折号会乱码。
pub const MARKER_TEXT_ASCII: &str = "---- squirl command output ----";

/// pwsh / PowerShell 5.1 的 `-Command` 脚本（内置模式用）。
/// 脚本以 marker 起始，命令原文随后（由目标 shell 当作代码执行）。
pub fn pwsh_command_script(command_line: &str, with_marker: bool) -> String {
    let marker = format!("Write-Output '{}'", MARKER_TEXT);
    if with_marker {
        format!("{marker}; {command_line}")
    } else {
        command_line.to_string()
    }
}

/// pwsh / PowerShell 5.1 的 `-File` 脚本内容（外部模式保持打开用，M0 定版）。
/// 文件内容零转义——命令原文逐字节写入。调用方需以 UTF-8 BOM 编码落盘
/// （PS 5.1 无 BOM 会按 ANSI 误读；pwsh 7 兼容 BOM）。
pub fn pwsh_file_script(command_line: &str, with_marker: bool) -> String {
    let mut s = String::new();
    if with_marker {
        s.push_str(&format!("Write-Output '{MARKER_TEXT}'\n"));
    }
    s.push_str(command_line);
    s.push('\n');
    s
}

/// cmd 的 `/c`（内置）或 `/k`（外部保持打开）参数。
/// cmd 解析「余下整行」，命令原文必须 raw 传递（`raw_arg`），
/// 不能经过 MSVCRT 引号包装——cmd 不理解 `\"`。
pub fn cmd_arg(keep_open: bool, command_line: &str) -> String {
    let flag = if keep_open { "/k" } else { "/c" };
    format!("{flag} {command_line}")
}

/// cmd 的 marker echo（ASCII，代码页安全）。
pub fn cmd_command_line(command_line: &str, with_marker: bool) -> String {
    if with_marker {
        format!("echo {MARKER_TEXT_ASCII}& {command_line}")
    } else {
        command_line.to_string()
    }
}

/// bash / zsh 的脚本串（`-lic` 后的单一参数）。
///
/// - 内置模式：`printf <marker>; <命令>`。
/// - 外部保持打开（M0 定版子壳形态）：命令包在 `( )` 子壳里——
///   用户命令中的 `exit` 只退出子壳，随后的 `exec <shell>` 把外层
///   替换为交互式 shell，窗口不闪退。
pub fn sh_script(shell: &str, command_line: &str, keep_open: bool, with_marker: bool) -> String {
    let marker = format!("printf '\\n{MARKER_TEXT}\\n'\n");
    let mut s = String::new();
    if with_marker {
        s.push_str(&marker);
    }
    if keep_open {
        s.push_str(&format!("( {command_line} ); exec {shell}"));
    } else {
        s.push_str(command_line);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pwsh_script_keeps_command_line_verbatim() {
        let cmd = r#"Write-Output "a  b"; Write-Output 'it''s ok'"#;
        let s = pwsh_command_script(cmd, true);
        assert!(s.starts_with(&format!("Write-Output '{MARKER_TEXT}'; ")));
        assert!(s.contains(cmd), "命令原文必须原样保留");
    }

    #[test]
    fn pwsh_file_script_multiline_safe() {
        let cmd = "if ($true) {\n  Write-Output multi\n}";
        let s = pwsh_file_script(cmd, true);
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines[0], format!("Write-Output '{MARKER_TEXT}'"));
        assert!(s.contains(cmd));
    }

    #[test]
    fn cmd_arg_uses_k_for_keep_open() {
        assert_eq!(cmd_arg(true, "echo a&echo b"), "/k echo a&echo b");
        assert_eq!(cmd_arg(false, "echo a"), "/c echo a");
    }

    #[test]
    fn cmd_command_line_marker_is_ascii() {
        let s = cmd_command_line("echo ok", true);
        assert!(s.starts_with(&format!("echo {MARKER_TEXT_ASCII}& ")));
        assert!(s.ends_with("echo ok"));
        assert!(s.is_char_boundary(0));
        assert!(!s.contains('─'), "cmd marker 不含 Unicode 破折号");
    }

    #[test]
    fn sh_script_subshell_for_keep_open() {
        let s = sh_script("bash", "exit 1", true, true);
        assert!(s.starts_with(&format!("printf '\\n{MARKER_TEXT}\\n'\n")));
        assert!(s.contains("( exit 1 ); exec bash"), "exit 1 必须被关进子壳");
    }

    #[test]
    fn sh_script_plain_for_builtin() {
        let s = sh_script("zsh", "echo hi", false, true);
        assert!(s.contains("echo hi"));
        assert!(!s.contains("exec"), "内置模式不做保持打开");
    }
}
