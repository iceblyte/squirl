//! 终端适配器（设计 §5.3.2）：按「模拟器 × shell」组合定义
//! 唤起模板 + 命令直传 + 转义 + 保持打开四件套。
//!
//! M0 实测定版（证据见 docs/进度与验证记录.md Spike A）：
//! | 组合 | 保持打开形态 | 验证 |
//! |---|---|---|
//! | conhost × pwsh | `-Login -NoLogo -NoExit -File <脚本>` | T8 实机 |
//! | conhost × cmd | `/k <原文>`（raw 传递） | 实机 |
//! | conhost × GitBash | `-lic "printf marker; ( <原文> ); exec bash"` | T6 语义 + 实机 |
//! | WT × 三者 | `wt -d <cwd> -- <shell 原参数>`（`--` 分隔符实机验证通过） | 实机 |
//! | macOS Terminal/iTerm2 | `do script "<cd && 原文>"`（tab 本身即交互 shell） | 代码级，未实机验证 |
//! | Linux gnome-terminal/konsole | `<emu> -- bash -lic "…( <原文> ); exec bash"` | 代码级，未实机验证 |
//!
//! pwsh 保持打开不用 `-NoExit -Command`：命令含 `exit` 时会话被杀
//! （实机 T1/T2/T7）；改用「marker + 命令原文」写入数据目录脚本文件、
//! `-File` 执行——零转义、多行安全，`exit` 只结束脚本不结束会话。

use std::path::{Path, PathBuf};

use crate::engine::emulator::EmulatorKind;
use crate::engine::quoting;
use crate::engine::shell::{ShellKind, ShellSpec};
use crate::error::{AppError, Result};

/// Windows 进程创建标志（std/tokio CommandExt::creation_flags）。
pub mod flags {
    pub const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    pub const CREATE_SUSPENDED: u32 = 0x0000_0004;
    pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
}

/// pwsh `-File` 脚本文件名前缀（启动清扫用）。
pub const SCRIPT_PREFIX: &str = "squirl-launch-";

#[derive(Debug, Clone)]
pub struct LaunchJob {
    pub shell: ShellSpec,
    /// 命令原文（事实记录，INV-6：不做二次解析）。
    pub command_line: String,
    pub cwd: PathBuf,
    /// 仅外部模式为 true（需求附录 A #4：外部一律保持打开）。
    pub keep_open: bool,
    pub with_marker: bool,
    /// pwsh `-File` 脚本落盘目录（生产 = 数据目录 tmp；Spike = 测试临时目录）。
    pub script_dir: Option<PathBuf>,
}

/// 单个 argv 元素。`raw = true` 时必须经 `raw_arg` 原样拼接
/// （cmd 不理解 MSVCRT 的 `\"` 转义）。
#[derive(Debug, Clone, PartialEq)]
pub struct PlanArg {
    pub text: String,
    pub raw: bool,
}

impl PlanArg {
    pub fn quoted(text: impl Into<String>) -> Self {
        Self { text: text.into(), raw: false }
    }
    pub fn raw(text: impl Into<String>) -> Self {
        Self { text: text.into(), raw: true }
    }
}

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<PlanArg>,
    pub cwd: PathBuf,
    pub creation_flags: u32,
    /// 适配器落盘的临时脚本（pwsh -File 形态）；启动失败时清理。
    pub temp_script: Option<PathBuf>,
}

pub trait TerminalAdapter {
    /// 适配器族 id（launch record 用；完整组合 id = `{adapter}+{shell}`）。
    fn id(&self) -> &'static str;
    fn build_launch(&self, job: &LaunchJob) -> Result<LaunchPlan>;
}

// ---------- Windows ----------

/// 控制台宿主：直接以 CREATE_NEW_CONSOLE 唤起 shell，进程句柄即终端进程。
pub struct ConsoleHostAdapter;

impl TerminalAdapter for ConsoleHostAdapter {
    fn id(&self) -> &'static str {
        "conhost"
    }

    fn build_launch(&self, job: &LaunchJob) -> Result<LaunchPlan> {
        let mut args = Vec::new();
        let mut temp_script = None;
        match job.shell.kind {
            ShellKind::Cmd => {
                let line = quoting::cmd_command_line(&job.command_line, job.with_marker);
                args.push(PlanArg::raw(quoting::cmd_arg(job.keep_open, &line)));
            }
            ShellKind::Pwsh | ShellKind::PowerShell5 => {
                if job.keep_open {
                    // M0 定版：-File 形态（脚本写数据目录，零转义）
                    let script = write_pwsh_script(job, job.shell.kind == ShellKind::Pwsh)?;
                    temp_script = Some(script.path.clone());
                    let mut v = vec![
                        PlanArg::quoted("-NoLogo"),
                        PlanArg::quoted("-NoExit"),
                        PlanArg::quoted("-File"),
                        PlanArg::quoted(script.path.to_string_lossy().into_owned()),
                    ];
                    if script.supports_login {
                        v.insert(0, PlanArg::quoted("-Login"));
                    }
                    args = v;
                } else {
                    let script = quoting::pwsh_command_script(&job.command_line, job.with_marker);
                    if job.shell.kind == ShellKind::Pwsh {
                        args = vec![
                            PlanArg::quoted("-Login"),
                            PlanArg::quoted("-NoLogo"),
                            PlanArg::quoted("-Command"),
                            PlanArg::quoted(script),
                        ];
                    } else {
                        args = vec![
                            PlanArg::quoted("-NoLogo"),
                            PlanArg::quoted("-Command"),
                            PlanArg::quoted(script),
                        ];
                    }
                }
            }
            ShellKind::Bash | ShellKind::Zsh => {
                let script = quoting::sh_script(
                    job.shell.sh_name(),
                    &job.command_line,
                    job.keep_open,
                    job.with_marker,
                );
                args = vec![PlanArg::quoted("-lic"), PlanArg::quoted(script)];
            }
        }
        Ok(LaunchPlan {
            program: job.shell.program.clone(),
            args,
            cwd: job.cwd.clone(),
            creation_flags: flags::CREATE_NEW_CONSOLE,
            temp_script,
        })
    }
}

/// Windows Terminal：`wt -d <cwd> -- <shell 原参数>`（实机验证 `--` 受支持）。
pub struct WindowsTerminalAdapter {
    /// wt.exe 解析路径。
    pub wt_path: String,
}

impl TerminalAdapter for WindowsTerminalAdapter {
    fn id(&self) -> &'static str {
        "wt"
    }

    fn build_launch(&self, job: &LaunchJob) -> Result<LaunchPlan> {
        let inner = ConsoleHostAdapter.build_launch(job)?;
        // wt 接管新控制台：内层不再自带 CREATE_NEW_CONSOLE
        let mut args = vec![
            PlanArg::quoted("-d"),
            PlanArg::quoted(job.cwd.to_string_lossy().into_owned()),
            PlanArg::quoted("--"),
            PlanArg::quoted(inner.program),
        ];
        args.extend(inner.args);
        Ok(LaunchPlan {
            program: self.wt_path.clone(),
            args,
            cwd: job.cwd.clone(),
            creation_flags: 0,
            temp_script: inner.temp_script,
        })
    }
}

struct PwshScript {
    path: PathBuf,
    /// pwsh 7 支持 `-Login`；PS 5.1 无该参数（需求 F4 表）。
    supports_login: bool,
}

/// 写 pwsh `-File` 脚本（UTF-8 BOM：PS 5.1 无 BOM 按 ANSI 误读）。
fn write_pwsh_script(job: &LaunchJob, is_pwsh7: bool) -> Result<PwshScript> {
    let dir = job.script_dir.as_ref().ok_or_else(|| {
        AppError::Engine("pwsh 保持打开需要 script_dir（数据目录 tmp）".into())
    })?;
    std::fs::create_dir_all(dir).map_err(|e| AppError::Path(format!("创建脚本目录失败: {e}")))?;
    let path = dir.join(format!(
        "{SCRIPT_PREFIX}{}.ps1",
        uuid::Uuid::now_v7().simple()
    ));
    let content = quoting::pwsh_file_script(&job.command_line, job.with_marker);
    // BOM 前缀：两代 PowerShell 都正确识别为 UTF-8
    std::fs::write(&path, format!("\u{feff}{content}"))
        .map_err(|e| AppError::Path(format!("写启动脚本失败: {e}")))?;
    Ok(PwshScript { path, supports_login: is_pwsh7 })
}

/// 清扫残留启动脚本（应用启动时调用；INV-1：只发生在数据目录 tmp）。
pub fn sweep_script_dir(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(SCRIPT_PREFIX) && name.ends_with(".ps1") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

// ---------- macOS（代码级实现，未实机验证 —— R1）----------

/// AppleScript 字符串转义（双层转义第一层：AppleScript 字面量）。
pub fn escape_applescript(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

/// Terminal.app：`do script` 的新 tab 本身是交互 shell，命令跑完留在提示符。
pub struct MacTerminalAdapter;

impl TerminalAdapter for MacTerminalAdapter {
    fn id(&self) -> &'static str {
        "terminal.app"
    }

    fn build_launch(&self, job: &LaunchJob) -> Result<LaunchPlan> {
        let sh = job.shell.sh_name();
        let script = quoting::sh_script(sh, &job.command_line, job.keep_open, job.with_marker);
        let inner = format!("cd {:?} && {}", job.cwd.to_string_lossy(), script);
        let apple = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            escape_applescript(&inner)
        );
        Ok(LaunchPlan {
            program: "osascript".into(),
            args: vec![PlanArg::quoted("-e"), PlanArg::quoted(apple)],
            cwd: job.cwd.clone(),
            creation_flags: 0,
            temp_script: None,
        })
    }
}

/// iTerm2：AppleScript 建新窗口 + write text（代码级，未实机验证）。
pub struct ITerm2Adapter;

impl TerminalAdapter for ITerm2Adapter {
    fn id(&self) -> &'static str {
        "iterm2"
    }

    fn build_launch(&self, job: &LaunchJob) -> Result<LaunchPlan> {
        let sh = job.shell.sh_name();
        let script = quoting::sh_script(sh, &job.command_line, job.keep_open, job.with_marker);
        let inner = format!("cd {:?}; {}", job.cwd.to_string_lossy(), script);
        let apple = format!(
            concat!(
                "tell application \"iTerm\"\n",
                "  create window with default profile\n",
                "  tell current session of current window to write text \"{}\"\n",
                "end tell"
            ),
            escape_applescript(&inner)
        );
        Ok(LaunchPlan {
            program: "osascript".into(),
            args: vec![PlanArg::quoted("-e"), PlanArg::quoted(apple)],
            cwd: job.cwd.clone(),
            creation_flags: 0,
            temp_script: None,
        })
    }
}

// ---------- Linux（代码级实现，未实机验证）----------

/// gnome-terminal / konsole 等 `-- <cmd>` 或 `-e <cmd>` 语义的通用适配。
pub struct GenericExecAdapter {
    pub kind: EmulatorKind,
    pub program: String,
}

impl TerminalAdapter for GenericExecAdapter {
    fn id(&self) -> &'static str {
        match self.kind {
            EmulatorKind::GnomeTerminal => "gnome-terminal",
            EmulatorKind::Konsole => "konsole",
            _ => "generic",
        }
    }

    fn build_launch(&self, job: &LaunchJob) -> Result<LaunchPlan> {
        let sh = job.shell.sh_name();
        let script = quoting::sh_script(sh, &job.command_line, job.keep_open, job.with_marker);
        let mut args = match self.kind {
            EmulatorKind::GnomeTerminal => vec![
                PlanArg::quoted(format!(
                    "--working-directory={}",
                    job.cwd.to_string_lossy()
                )),
                PlanArg::quoted("--"),
            ],
            EmulatorKind::Konsole => vec![
                PlanArg::quoted("--workdir"),
                PlanArg::quoted(job.cwd.to_string_lossy().into_owned()),
                PlanArg::quoted("-e"),
            ],
            _ => vec![PlanArg::quoted("-e")],
        };
        args.push(PlanArg::quoted(job.shell.program.clone()));
        args.push(PlanArg::quoted("-lic"));
        args.push(PlanArg::quoted(script));
        Ok(LaunchPlan {
            program: self.program.clone(),
            args,
            cwd: job.cwd.clone(),
            creation_flags: 0,
            temp_script: None,
        })
    }
}

// ---------- 内置模式（无模拟器层）----------

/// 内置模式启动计划：直接 spawn shell，捕获管道（无窗口）。
/// 创建标志：`CREATE_NO_WINDOW | CREATE_SUSPENDED`
/// （挂起以便先入 Job Object 再恢复，见 supervisor）。
pub fn build_builtin_launch(job: &LaunchJob) -> Result<LaunchPlan> {
    let args = match job.shell.kind {
        ShellKind::Cmd => {
            let line = quoting::cmd_command_line(&job.command_line, job.with_marker);
            vec![PlanArg::raw(quoting::cmd_arg(false, &line))]
        }
        ShellKind::Pwsh | ShellKind::PowerShell5 => {
            let script = quoting::pwsh_command_script(&job.command_line, job.with_marker);
            let mut v = vec![PlanArg::quoted("-NoLogo"), PlanArg::quoted("-Command"), PlanArg::quoted(script)];
            if job.shell.kind == ShellKind::Pwsh {
                v.insert(0, PlanArg::quoted("-Login"));
            }
            v
        }
        ShellKind::Bash | ShellKind::Zsh => {
            let script = quoting::sh_script(job.shell.sh_name(), &job.command_line, false, job.with_marker);
            vec![PlanArg::quoted("-lic"), PlanArg::quoted(script)]
        }
    };
    Ok(LaunchPlan {
        program: job.shell.program.clone(),
        args,
        cwd: job.cwd.clone(),
        creation_flags: flags::CREATE_NO_WINDOW | flags::CREATE_SUSPENDED,
        temp_script: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::quoting::{MARKER_TEXT, MARKER_TEXT_ASCII};
    use std::path::PathBuf;

    fn job(kind: ShellKind) -> LaunchJob {
        LaunchJob {
            shell: ShellSpec { kind, program: format!("{kind:?}.exe").to_lowercase() },
            command_line: "echo hi".into(),
            cwd: PathBuf::from(r"C:\proj"),
            keep_open: true,
            with_marker: true,
            script_dir: None,
        }
    }

    #[test]
    fn conhost_cmd_is_raw_single_arg() {
        let p = ConsoleHostAdapter.build_launch(&job(ShellKind::Cmd)).unwrap();
        assert_eq!(p.creation_flags, flags::CREATE_NEW_CONSOLE);
        assert_eq!(p.args.len(), 1);
        assert!(p.args[0].raw, "cmd 参数必须 raw 传递");
        assert!(p.args[0].text.starts_with("/k "));
        assert!(p.args[0].text.contains(MARKER_TEXT_ASCII));
    }

    #[test]
    fn conhost_pwsh_keep_open_uses_file_form() {
        let mut j = job(ShellKind::Pwsh);
        let tmp = tempfile::tempdir().unwrap();
        j.script_dir = Some(tmp.path().to_path_buf());
        let p = ConsoleHostAdapter.build_launch(&j).unwrap();
        let texts: Vec<&str> = p.args.iter().map(|a| a.text.as_str()).collect();
        assert_eq!(&texts[..4], &["-Login", "-NoLogo", "-NoExit", "-File"]);
        let script_path = &p.args[4];
        assert!(Path::new(&script_path.text).is_file(), "脚本文件必须已落盘");
        assert!(p.temp_script.is_some());
        // 内容：BOM + marker + 原文
        let content = std::fs::read_to_string(&script_path.text).unwrap();
        assert!(content.starts_with('\u{feff}'));
        assert!(content.contains(MARKER_TEXT));
        assert!(content.contains("echo hi"));
    }

    #[test]
    fn conhost_bash_keep_open_uses_subshell() {
        let p = ConsoleHostAdapter.build_launch(&job(ShellKind::Bash)).unwrap();
        let script = &p.args[1].text;
        assert!(script.contains("( echo hi ); exec bash"));
        assert!(script.contains(MARKER_TEXT));
    }

    #[test]
    fn wt_wraps_inner_plan_with_d_flag() {
        let mut j = job(ShellKind::Cmd);
        j.script_dir = Some(PathBuf::from("."));
        let wt = WindowsTerminalAdapter { wt_path: r"C:\wt.exe".into() };
        let p = wt.build_launch(&j).unwrap();
        let texts: Vec<&str> = p.args.iter().map(|a| a.text.as_str()).collect();
        assert_eq!(texts[0], "-d");
        assert_eq!(texts[2], "--");
        assert_eq!(p.program, r"C:\wt.exe");
        assert_eq!(p.creation_flags, 0, "wt 自管窗口，不再加 CREATE_NEW_CONSOLE");
    }

    #[test]
    fn mac_apple_script_escapes_quotes_and_backslash() {
        assert_eq!(escape_applescript(r"a\b"), r"a\\b");
        assert_eq!(escape_applescript("s\"q"), "s\\\"q");
        let p = MacTerminalAdapter.build_launch(&job(ShellKind::Zsh)).unwrap();
        assert_eq!(p.program, "osascript");
        assert!(p.args[1].text.starts_with("tell application \"Terminal\""));
        // 命令原文里的引号必须被转义保真
        let mut j = job(ShellKind::Zsh);
        j.command_line = r#"echo "a b""#.into();
        let p = MacTerminalAdapter.build_launch(&j).unwrap();
        let apple = &p.args[1].text;
        assert!(apple.contains(r#"echo \"a b\""#));
    }

    #[test]
    fn iterm_builds_write_text() {
        let p = ITerm2Adapter.build_launch(&job(ShellKind::Bash)).unwrap();
        let apple = &p.args[1].text;
        assert!(apple.contains("create window with default profile"));
        assert!(apple.contains("write text"));
    }

    #[test]
    fn gnome_terminal_args_shape() {
        let g = GenericExecAdapter {
            kind: EmulatorKind::GnomeTerminal,
            program: "/usr/bin/gnome-terminal".into(),
        };
        let p = g.build_launch(&job(ShellKind::Bash)).unwrap();
        assert!(p.args[0].text.starts_with("--working-directory="));
        assert!(p.args.iter().any(|a| a.text == "--"));
        assert!(p.args.iter().any(|a| a.text == "-lic"));
    }

    #[test]
    fn builtin_plan_uses_no_window_and_command_form() {
        let mut j = job(ShellKind::Pwsh);
        j.keep_open = false;
        let p = build_builtin_launch(&j).unwrap();
        assert_eq!(p.creation_flags, flags::CREATE_NO_WINDOW | flags::CREATE_SUSPENDED);
        let texts: Vec<&str> = p.args.iter().map(|a| a.text.as_str()).collect();
        assert_eq!(&texts[..3], &["-Login", "-NoLogo", "-Command"]);
        assert!(p.args[3].text.contains(MARKER_TEXT));
        assert!(p.args[3].text.contains("echo hi"));
    }

    #[test]
    fn builtin_cmd_raw() {
        let mut j = job(ShellKind::Cmd);
        j.keep_open = false;
        let p = build_builtin_launch(&j).unwrap();
        assert!(p.args[0].raw);
        assert!(p.args[0].text.starts_with("/c "));
    }

    #[test]
    fn sweep_removes_only_launch_scripts() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join(format!("{SCRIPT_PREFIX}a.ps1"));
        let b = tmp.path().join("other.ps1");
        std::fs::write(&a, "x").unwrap();
        std::fs::write(&b, "x").unwrap();
        sweep_script_dir(tmp.path());
        assert!(!a.exists());
        assert!(b.exists());
    }
}
