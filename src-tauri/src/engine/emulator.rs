//! 模拟器层（设计 §5.3.1，仅外部模式使用）。
//!
//! 探测顺序：
//! - Windows：Windows Terminal（`wt.exe`）→ 默认控制台宿主（无需探测，直接
//!   `CREATE_NEW_CONSOLE` 唤起 shell；Win11 下系统默认即 WT）
//! - macOS：Terminal（必在）→ iTerm2（`/Applications/iTerm.app` 存在性）
//! - Linux：gnome-terminal / konsole / alacritty / kitty / xterm 逐个探测
//! - 全部失败 → `E_NO_EMULATOR`，由设置页手动指定
//!
//! ⚠️ R2：`wt.exe` App Execution Alias 假阳性风险由 Spike A 实机冒烟覆盖。

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmulatorKind {
    WindowsTerminal,
    ConsoleHost,
    MacTerminal,
    ITerm2,
    GnomeTerminal,
    Konsole,
    Alacritty,
    Kitty,
    XTerm,
}

impl EmulatorKind {
    pub fn id(self) -> &'static str {
        match self {
            EmulatorKind::WindowsTerminal => "wt",
            EmulatorKind::ConsoleHost => "conhost",
            EmulatorKind::MacTerminal => "terminal.app",
            EmulatorKind::ITerm2 => "iterm2",
            EmulatorKind::GnomeTerminal => "gnome-terminal",
            EmulatorKind::Konsole => "konsole",
            EmulatorKind::Alacritty => "alacritty",
            EmulatorKind::Kitty => "kitty",
            EmulatorKind::XTerm => "xterm",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmulatorSpec {
    pub kind: EmulatorKind,
    /// 启动器可执行路径（WindowsTerminal=wt.exe；其余按平台语义）。
    pub program: Option<String>,
}

pub trait Probe {
    fn locate(&self, program: &str) -> Option<String>;
    fn path_exists(&self, path: &str) -> bool;
}

/// 生产探测：PATH + 特定路径存在性。
pub struct EnvProbe;

impl Probe for EnvProbe {
    fn locate(&self, program: &str) -> Option<String> {
        crate::engine::shell::locate_on_path(program)
    }
    fn path_exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}

pub fn resolve(probe: &dyn Probe) -> Result<EmulatorSpec> {
    match std::env::consts::OS {
        "windows" => resolve_windows(probe),
        "macos" => resolve_macos(probe),
        _ => resolve_linux(probe),
    }
}

pub fn resolve_windows(probe: &dyn Probe) -> Result<EmulatorSpec> {
    if let Some(p) = probe.locate("wt") {
        return Ok(EmulatorSpec { kind: EmulatorKind::WindowsTerminal, program: Some(p) });
    }
    Ok(EmulatorSpec { kind: EmulatorKind::ConsoleHost, program: None })
}

pub fn resolve_macos(probe: &dyn Probe) -> Result<EmulatorSpec> {
    if probe.path_exists("/Applications/iTerm.app") {
        return Ok(EmulatorSpec { kind: EmulatorKind::ITerm2, program: None });
    }
    Ok(EmulatorSpec { kind: EmulatorKind::MacTerminal, program: None })
}

pub fn resolve_linux(probe: &dyn Probe) -> Result<EmulatorSpec> {
    for (name, kind) in [
        ("gnome-terminal", EmulatorKind::GnomeTerminal),
        ("konsole", EmulatorKind::Konsole),
        ("alacritty", EmulatorKind::Alacritty),
        ("kitty", EmulatorKind::Kitty),
        ("xterm", EmulatorKind::XTerm),
    ] {
        if let Some(p) = probe.locate(name) {
            return Ok(EmulatorSpec { kind, program: Some(p) });
        }
    }
    Err(AppError::NoEmulator)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProbe {
        found: Vec<&'static str>,
        existing: Vec<&'static str>,
    }
    impl Probe for MockProbe {
        fn locate(&self, program: &str) -> Option<String> {
            self.found.iter().find(|f| **f == program).map(|f| format!(r"C:\bins\{f}.exe"))
        }
        fn path_exists(&self, path: &str) -> bool {
            self.existing.contains(&path)
        }
    }

    #[test]
    fn windows_prefers_wt_falls_back_to_conhost() {
        let s = resolve_windows(&MockProbe { found: vec!["wt"], existing: vec![] }).unwrap();
        assert_eq!(s.kind, EmulatorKind::WindowsTerminal);
        let s = resolve_windows(&MockProbe { found: vec![], existing: vec![] }).unwrap();
        assert_eq!(s.kind, EmulatorKind::ConsoleHost, "控制台宿主是兜底，不报错");
    }

    #[test]
    fn macos_iterm2_then_terminal() {
        let s = resolve_macos(&MockProbe { found: vec![], existing: vec!["/Applications/iTerm.app"] }).unwrap();
        assert_eq!(s.kind, EmulatorKind::ITerm2);
        let s = resolve_macos(&MockProbe { found: vec![], existing: vec![] }).unwrap();
        assert_eq!(s.kind, EmulatorKind::MacTerminal);
    }

    #[test]
    fn linux_priority_and_no_emulator_error() {
        let s = resolve_linux(&MockProbe { found: vec!["konsole", "gnome-terminal"], existing: vec![] }).unwrap();
        assert_eq!(s.kind, EmulatorKind::GnomeTerminal, "按探测顺序 gnome-terminal 优先");
        assert!(resolve_linux(&MockProbe { found: vec![], existing: vec![] }).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn env_probe_finds_wt_on_real_machine() {
        let s = resolve(&EnvProbe).unwrap();
        assert_eq!(s.kind, EmulatorKind::WindowsTerminal, "实机应探测到 WT");
    }
}
