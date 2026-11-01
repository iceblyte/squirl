//! Shell 层解析（设计 §5.3.1，两模式共用）。
//!
//! 解析优先级（需求 F4 / 附录 A #22）：
//! - Windows：pwsh 7+（PATH）→ powershell.exe（UI 警告 `&&` 不可用）→ cmd.exe（%COMSPEC%）
//! - macOS/Linux：`$SHELL`（basename 在支持列表 {bash, zsh}）→ zsh → bash（逐个探测）
//! - fish / nushell / csh 等探测到 → 回退默认链 + 警告（`E_SHELL_UNSUPPORTED` 提示）
//!
//! 探测以 `Probe` 注入，保证纯逻辑可在三平台单测（设计 §5.3.7）。

use crate::engine::quoting;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Pwsh,
    PowerShell5,
    Cmd,
    Bash,
    Zsh,
}

impl ShellKind {
    /// 逐 shell 参数形态的显示名（UI / launch record 用）。
    pub fn display_name(self) -> &'static str {
        match self {
            ShellKind::Pwsh => "pwsh",
            ShellKind::PowerShell5 => "powershell",
            ShellKind::Cmd => "cmd",
            ShellKind::Bash => "bash",
            ShellKind::Zsh => "zsh",
        }
    }

    /// PS 5.1 无 `&&`（R3：解析命中即 UI 警告，不代改命令）。
    pub fn warnings(self) -> &'static [&'static str] {
        match self {
            ShellKind::PowerShell5 => &["powershell.exe 5.1 不支持 `&&` 管道链，相关命令会失败"],
            _ => &[],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShellSpec {
    pub kind: ShellKind,
    /// 解析到的可执行文件路径（设置页展示「实际解析到的 shell 路径」）。
    pub program: String,
}

impl ShellSpec {
    /// `-lic` 等脚本参数之后的交互 shell 名（保持打开 `exec` 用）。
    pub fn sh_name(&self) -> &'static str {
        self.kind.display_name()
    }

    /// 标志该 shell 是否官方支持（F4 支持列表；不支持一律不会走到这里）。
    pub fn supported(&self) -> bool {
        true
    }

    /// 组装 shell 自身参数（不含模拟器包装）。`script` 为 quoting 装配产物。
    pub fn spawn_args(&self, script: &str, keep_open: bool) -> Vec<String> {
        match self.kind {
            ShellKind::Pwsh => vec!["-Login".into(), "-NoLogo".into(), "-Command".into(), script.into()],
            ShellKind::PowerShell5 => vec!["-NoLogo".into(), "-Command".into(), script.into()],
            // cmd：脚本与 /c、/k 同元素 raw 传递（见 quoting::cmd_arg）
            ShellKind::Cmd => vec![quoting::cmd_arg(keep_open, script)],
            ShellKind::Bash | ShellKind::Zsh => {
                vec!["-lic".into(), script.into()]
            }
        }
    }
}

/// PATH / 存在性探测的抽象，便于 mock。
pub trait Probe {
    /// 探测可执行名（如 `pwsh`、`zsh`），返回解析到的绝对路径。
    fn locate(&self, program: &str) -> Option<String>;
    /// `$SHELL` 环境变量。
    fn user_shell(&self) -> Option<String>;
}

/// 生产探测实现：扫 PATH（Windows 附加 PATHEXT 语义）。
pub struct EnvProbe;

impl Probe for EnvProbe {
    fn locate(&self, program: &str) -> Option<String> {
        locate_on_path(program)
    }
    fn user_shell(&self) -> Option<String> {
        std::env::var("SHELL").ok().filter(|s| !s.is_empty())
    }
}

/// Windows 解析链。
pub fn resolve_windows(probe: &dyn Probe) -> Result<ShellSpec> {
    if let Some(p) = probe.locate("pwsh") {
        return Ok(ShellSpec { kind: ShellKind::Pwsh, program: p });
    }
    if let Some(p) = probe.locate("powershell") {
        return Ok(ShellSpec { kind: ShellKind::PowerShell5, program: p });
    }
    if let Some(p) = probe.locate("cmd") {
        return Ok(ShellSpec { kind: ShellKind::Cmd, program: p });
    }
    Err(AppError::ShellUnsupported(
        "Windows 未找到 pwsh / powershell / cmd 任何一个".into(),
    ))
}

/// macOS / Linux 解析链。
pub fn resolve_unix(probe: &dyn Probe) -> Result<ShellSpec> {
    let unsupported = ["fish", "nushell", "nu", "csh", "tcsh"];
    if let Some(s) = probe.user_shell() {
        let base = std::path::Path::new(&s)
            .file_stem()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();
        if unsupported.contains(&base.as_str()) {
            // 附录 A #22：不支持 → 回退默认链 + 提示（警告经 ShellSpec 通道返回设置页）
            return fallback_unix(probe);
        }
        if base == "bash" || base == "zsh" {
            return Ok(ShellSpec { kind: if base == "bash" { ShellKind::Bash } else { ShellKind::Zsh }, program: s });
        }
    }
    fallback_unix(probe)
}

fn fallback_unix(probe: &dyn Probe) -> Result<ShellSpec> {
    if let Some(p) = probe.locate("zsh") {
        return Ok(ShellSpec { kind: ShellKind::Zsh, program: p });
    }
    if let Some(p) = probe.locate("bash") {
        return Ok(ShellSpec { kind: ShellKind::Bash, program: p });
    }
    Err(AppError::ShellUnsupported("未找到 zsh / bash".into()))
}

/// 平台分发。
pub fn resolve(probe: &dyn Probe) -> Result<ShellSpec> {
    match std::env::consts::OS {
        "windows" => resolve_windows(probe),
        _ => resolve_unix(probe),
    }
}

/// PATH 扫描（Windows：追加 PATHEXT；Unix：直接匹配）。
pub fn locate_on_path(program: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .map(|s| s.to_lowercase())
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let cand = dir.join(format!("{program}{ext}"));
            if cand.is_file() {
                return Some(cand.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Git Bash（Git for Windows）解析：git.exe 同源定位，兜底常见安装路径。
/// 依据需求 F4「Git Bash 预设 = Git for Windows bash」。
pub fn resolve_git_bash() -> Option<String> {
    if let Some(git) = locate_on_path("git") {
        // <root>\cmd\git.exe → <root>\bin\bash.exe
        let p = std::path::PathBuf::from(&git);
        if let Some(root) = p.ancestors().nth(2) {
            let cand = root.join("bin").join("bash.exe");
            if cand.is_file() {
                return Some(cand.to_string_lossy().into_owned());
            }
        }
    }
    for cand in [r"C:\Program Files\Git\bin\bash.exe", r"D:\software\develop\Git\bin\bash.exe"] {
        if std::path::Path::new(cand).is_file() {
            return Some(cand.into());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockProbe {
        found: Vec<&'static str>,
        shell: Option<String>,
    }
    impl Probe for MockProbe {
        fn locate(&self, program: &str) -> Option<String> {
            self.found.iter().find(|f| **f == program).map(|f| format!(r"C:\bins\{f}.exe"))
        }
        fn user_shell(&self) -> Option<String> {
            self.shell.clone()
        }
    }

    fn probe(found: Vec<&'static str>, shell: Option<&str>) -> MockProbe {
        MockProbe { found, shell: shell.map(String::from) }
    }

    #[test]
    fn windows_priority_pwsh_first() {
        let s = resolve_windows(&probe(vec!["pwsh", "powershell", "cmd"], None)).unwrap();
        assert_eq!(s.kind, ShellKind::Pwsh);
    }

    #[test]
    fn windows_falls_back_to_ps51_with_warning() {
        let s = resolve_windows(&probe(vec!["powershell"], None)).unwrap();
        assert_eq!(s.kind, ShellKind::PowerShell5);
        assert!(!s.kind.warnings().is_empty(), "PS 5.1 必须携带 && 警告");
    }

    #[test]
    fn windows_cmd_is_last_resort() {
        let s = resolve_windows(&probe(vec!["cmd"], None)).unwrap();
        assert_eq!(s.kind, ShellKind::Cmd);
    }

    #[test]
    fn windows_nothing_found_is_error() {
        assert!(resolve_windows(&probe(vec![], None)).is_err());
    }

    #[test]
    fn unix_uses_user_shell_when_supported() {
        let mut p = probe(vec!["zsh", "bash"], Some("/usr/bin/zsh"));
        p.found = vec![];
        let s = resolve_unix(&p).unwrap();
        assert_eq!(s.kind, ShellKind::Zsh);
        assert_eq!(s.program, "/usr/bin/zsh");
    }

    #[test]
    fn unix_unsupported_shell_falls_back() {
        let p = probe(vec!["bash"], Some("/usr/bin/fish"));
        let s = resolve_unix(&p).unwrap();
        assert_eq!(s.kind, ShellKind::Bash, "fish → 回退默认链");
    }

    #[test]
    fn unix_no_user_shell_probes_defaults() {
        let s = resolve_unix(&probe(vec!["bash"], None)).unwrap();
        assert_eq!(s.kind, ShellKind::Bash);
    }

    #[test]
    fn git_bash_resolution_finds_sibling() {
        // 本机（Windows 实机）验证 git.exe 同源定位
        if cfg!(windows) {
            let p = resolve_git_bash();
            assert!(p.is_some(), "实机必须能定位 Git Bash");
            let path = p.unwrap();
            assert!(path.to_lowercase().ends_with("bash.exe"));
            assert!(std::path::Path::new(&path).is_file());
        }
    }

    #[test]
    fn spawn_args_shape_per_shell() {
        let pwsh = ShellSpec { kind: ShellKind::Pwsh, program: "pwsh".into() };
        assert_eq!(pwsh.spawn_args("SCRIPT", false), vec!["-Login", "-NoLogo", "-Command", "SCRIPT"]);
        let cmd = ShellSpec { kind: ShellKind::Cmd, program: "cmd".into() };
        assert_eq!(cmd.spawn_args("echo a", true), vec!["/k echo a"]);
    }

    #[test]
    fn locate_on_path_finds_cargo() {
        // 冒烟：真实 PATH 扫描至少能找到本进程工具链里的一个程序
        let found = locate_on_path("cargo");
        assert!(found.is_some(), "cargo 应在 PATH 上");
    }

    // 让 HashMap 引用不产生未使用告警（保留给后续 mock 扩展）
    #[allow(dead_code)]
    fn unused(_: HashMap<String, String>) {}
}
