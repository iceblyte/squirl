//! 外部模式进程唤起（设计 §5.3.4：fire-and-forget，INV-2 不入 Job Object）。
//!
//! 为什么不用 `std::process::Command`：std 在 Windows 下总是传
//! `STARTF_USESTDHANDLES`（父进程 stdio），`CREATE_NEW_CONSOLE` 的新控制台
//! 只接管窗口不接管 stdio——keep-open 的交互 shell 会从父进程 stdin 读到
//! EOF 立即退出（Spike A 探针实机证据）。`CreateProcessW` 不设
//! USESTDHANDLES 时，子进程 stdio = 新控制台默认句柄，交互会话正常。
//!
//! 唤起后仅短持进程句柄用于「窗口唤起是否失败」判定（DR-12 launch record
//! 由 supervisor/上层补写），绝不追踪进程树（外部进程归终端所有）。

use crate::engine::adapter::LaunchPlan;
use crate::error::{AppError, Result};

#[derive(Debug)]
pub struct ExternalProcess {
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
    pub pid: u32,
}

impl ExternalProcess {
    /// 进程是否仍存活（WaitForSingleObject(0) == WAIT_TIMEOUT）。
    #[cfg(windows)]
    pub fn is_alive(&self) -> bool {
        use windows::Win32::Foundation::WAIT_TIMEOUT;
        use windows::Win32::System::Threading::WaitForSingleObject;
        unsafe { WaitForSingleObject(self.handle, 0) == WAIT_TIMEOUT }
    }

    /// 已退出则返回退出码（STILL_ACTIVE 期间返回 None）。
    #[cfg(windows)]
    pub fn try_exit_code(&self) -> Option<i32> {
        use windows::Win32::Foundation::WAIT_TIMEOUT;
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, WaitForSingleObject,
        };
        let mut code: u32 = 0;
        unsafe {
            if WaitForSingleObject(self.handle, 0) == WAIT_TIMEOUT {
                return None;
            }
            GetExitCodeProcess(self.handle, &mut code)
                .map_err(|e| tracing::debug!("GetExitCodeProcess: {e}"))
                .ok()?;
        }
        Some(code as i32)
    }

    /// 终止进程（测试与错误兜底用；正常外部模式应用不终止用户终端）。
    #[cfg(windows)]
    pub fn terminate(&self) {
        use windows::Win32::System::Threading::TerminateProcess;
        unsafe {
            let _ = TerminateProcess(self.handle, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for ExternalProcess {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
pub fn spawn_external(plan: &LaunchPlan) -> Result<ExternalProcess> {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::System::Threading::{
        CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW,
    };

    // lpApplicationName 必须是完整路径（CreateProcessW 不做 PATH 搜索）；
    // 裸程序名在这里兜底解析为绝对路径。
    let program = resolve_program_path(&plan.program);
    let cmdline = build_command_line(&plan.program, &plan.args);
    tracing::info!(
        target: "launch_record",
        mode = "external",
        adapter_program = %plan.program,
        command_line = %cmdline,
        cwd = %plan.cwd.display(),
        "external launch"
    );

    let mut cmdline_w = to_wide(&cmdline);
    let program_w = to_wide(&program);
    let cwd_w = to_wide(&plan.cwd.to_string_lossy());

    unsafe {
        let si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();
        CreateProcessW(
            PCWSTR(program_w.as_ptr()),
            Some(PWSTR(cmdline_w.as_mut_ptr())),
            None,
            None,
            false, // bInheritHandles = false：句柄不外泄，stdio 用新控制台默认
            PROCESS_CREATION_FLAGS(plan.creation_flags),
            None,
            PCWSTR(cwd_w.as_ptr()),
            &si,
            &mut pi,
        )
        .map_err(|e| {
            AppError::LaunchFailed(format!(
                "CreateProcessW 失败: {e}; cmdline={cmdline}"
            ))
        })?;
        // 主线程句柄用完即关，进程句柄交给 ExternalProcess
        let _ = windows::Win32::Foundation::CloseHandle(pi.hThread);
        Ok(ExternalProcess { handle: pi.hProcess, pid: pi.dwProcessId })
    }
}

#[cfg(unix)]
pub fn spawn_external(plan: &LaunchPlan) -> Result<ExternalProcess> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(&plan.program);
    for a in &plan.args {
        cmd.arg(&a.text);
    }
    cmd.current_dir(&plan.cwd).stdin(Stdio::null());
    let child = cmd
        .spawn()
        .map_err(|e| AppError::LaunchFailed(format!("spawn 失败: {e}")))?;
    tracing::info!(
        target: "launch_record",
        mode = "external",
        adapter_program = %plan.program,
        cwd = %plan.cwd.display(),
        "external launch (unix)"
    );
    Ok(ExternalProcess { pid: child.id() })
}

#[cfg(unix)]
impl ExternalProcess {
    pub fn is_alive(&self) -> bool {
        unsafe { libc::kill(self.pid, 0) == 0 }
    }
    pub fn try_exit_code(&self) -> Option<i32> {
        None
    }
    pub fn terminate(&self) {
        unsafe {
            libc::kill(self.pid, libc::SIGTERM);
        }
    }
}

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// 组装 CreateProcessW 的完整命令行（MSVCRT 引号规则；raw 元素原样拼接）。
#[cfg(windows)]
fn build_command_line(program: &str, args: &[crate::engine::adapter::PlanArg]) -> String {
    let mut line = quote_msvc(program);
    for a in args {
        line.push(' ');
        if a.raw {
            line.push_str(&a.text);
        } else {
            line.push_str(&quote_msvc(&a.text));
        }
    }
    line
}

/// CreateProcessW 的 lpApplicationName 需要完整路径：
/// 绝对/相对路径（含分隔符）原样使用，裸名经 PATH 解析。
#[cfg(windows)]
fn resolve_program_path(program: &str) -> String {
    if program.contains('\\') || program.contains('/') {
        return program.to_string();
    }
    crate::engine::shell::locate_on_path(program)
        .unwrap_or_else(|| format!("{program}.exe"))
}

/// MSVCRT 参数引号规则（与 std::process 内部一致）：
/// 含空格/tab/引号时整体加引号，内部 `"` → `\"`，反斜杠在引号前成对。
#[cfg(windows)]
pub fn quote_msvc(s: &str) -> String {
    if !s.is_empty() && !s.chars().any(|c| matches!(c, ' ' | '\t' | '"' | '\n')) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for ch in s.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                out.push(ch);
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::engine::adapter::PlanArg;
    use std::path::PathBuf;

    #[test]
    fn quote_msvc_matches_std_rules() {
        assert_eq!(quote_msvc("plain"), "plain");
        assert_eq!(quote_msvc(""), "\"\"");
        assert_eq!(quote_msvc("a b"), "\"a b\"");
        assert_eq!(quote_msvc(r#"he said "hi""#), r#""he said \"hi\"""#);
        // 尾随反斜杠但无空格/引号：不加引号、不改写（与 std 一致）
        let trail: String = ['t', 'r', 'a', 'i', 'l', '\\'].iter().collect();
        assert_eq!(quote_msvc(&trail), trail);
        // 输入 a\" → "a\\\"\""（引号前反斜杠翻倍 +1）
        let input: String = ['a', '\\', '"', '"'].iter().collect();
        let expected: String = ['"', 'a', '\\', '\\', '\\', '"', '\\', '"', '"'].iter().collect();
        assert_eq!(quote_msvc(&input), expected);
    }

    #[test]
    fn command_line_raw_vs_quoted() {
        let plan = LaunchPlan {
            program: r"C:\bins\my shell.exe".into(),
            args: vec![PlanArg::quoted("-d"), PlanArg::raw("/k echo a&echo b")],
            cwd: PathBuf::from(r"C:\x"),
            creation_flags: 0,
            temp_script: None,
        };
        let line = build_command_line(&plan.program, &plan.args);
        assert_eq!(line, r#""C:\bins\my shell.exe" -d /k echo a&echo b"#);
    }
}
