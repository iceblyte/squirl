//! Spike A —— 外部终端唤起矩阵实机冒烟（Windows 实机，设计 §6.3）。
//!
//! 运行方式：`cargo test --test spike_a -- --ignored --test-threads=1`
//! （会真实弹出终端窗口；用全局互斥串行化）。
//!
//! 覆盖：A1 WT×3 shell、A2 conhost×3 shell、A3 转义矩阵（空格/引号/&/&&/中文路径/
//! PS5.1 && 失败不崩）、A4 保持打开（exit 1 不闪退）。macOS/Linux 适配器为代码级
//! 单测（adapter.rs），实机冒烟标注「待补验」。

#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use squirl::engine::adapter::{
    ConsoleHostAdapter, LaunchJob, LaunchPlan, TerminalAdapter, WindowsTerminalAdapter,
};
use squirl::engine::shell::{resolve_git_bash, ShellKind, ShellSpec};
use squirl::engine::spawn::{spawn_external, ExternalProcess};

static SERIAL: Mutex<()> = Mutex::new(());

// ---------- 基础设施 ----------

fn spawn_ext(plan: &LaunchPlan) -> ExternalProcess {
    spawn_external(plan).expect("CreateProcessW 唤起失败")
}

/// 等待进程退出并返回退出码。
fn wait_exit(proc: &ExternalProcess, timeout: Duration) -> Option<i32> {
    poll_until(timeout, || proc.try_exit_code())
}

fn poll_until<F: FnMut() -> Option<T>, T>(timeout: Duration, mut f: F) -> Option<T> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(v) = f() {
            return Some(v);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    f()
}

fn read_result(path: &Path) -> Option<String> {
    let s = std::fs::read_to_string(path).ok()?;
    Some(s.trim_start_matches('\u{feff}').trim().to_string())
}

fn kill_pids(pids: &[u32]) {
    for pid in pids {
        let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output();
    }
}

struct TempCase {
    dir: PathBuf,
}

impl TempCase {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "squirl-m0-{label}-{}",
            uuid::Uuid::now_v7().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempCase { dir }
    }
    fn result_path(&self) -> PathBuf {
        self.dir.join("result.txt")
    }
}

impl Drop for TempCase {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn pwsh_shell() -> ShellSpec {
    ShellSpec { kind: ShellKind::Pwsh, program: "pwsh".into() }
}

fn ps51_shell() -> ShellSpec {
    ShellSpec {
        kind: ShellKind::PowerShell5,
        program: r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".into(),
    }
}

fn cmd_shell() -> ShellSpec {
    ShellSpec { kind: ShellKind::Cmd, program: r"C:\Windows\System32\cmd.exe".into() }
}

fn git_bash_shell() -> ShellSpec {
    let program = resolve_git_bash().expect("Git Bash 必须可定位");
    ShellSpec { kind: ShellKind::Bash, program }
}

fn wt() -> WindowsTerminalAdapter {
    let wt_path = squirl::engine::shell::locate_on_path("wt").expect("wt.exe 必须在位");
    WindowsTerminalAdapter { wt_path }
}

fn job(shell: ShellSpec, command_line: &str, cwd: &Path, script_dir: Option<PathBuf>) -> LaunchJob {
    LaunchJob {
        shell,
        command_line: command_line.into(),
        cwd: cwd.into(),
        keep_open: true,
        with_marker: true,
        script_dir,
    }
}

// ---------- A1：WT × pwsh / cmd / GitBash ----------

#[test]
#[ignore = "Windows 实机冒烟"]
fn a1_wt_pwsh_command_delivery() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a1-pwsh");
    let res = case.result_path();
    let script_dir = case.dir.join("scripts");
    let cmdline = format!(
        "Set-Content -LiteralPath '{}' -Value 'pwsh-wt-ok'",
        res.to_string_lossy()
    );
    let plan = wt()
        .build_launch(&job(pwsh_shell(), &cmdline, &case.dir, Some(script_dir)))
        .unwrap();
    spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    assert_eq!(
        waited.as_deref(),
        Some("pwsh-wt-ok"),
        "WT×pwsh 命令直传语义保真"
    );
    // 清理 keep-open 的 pwsh（其命令行含用例目录路径，可精确匹配）
    kill_pids(&pids_matching("squirl-m0-a1-pwsh", "pwsh.exe"));
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a1_wt_cmd_command_delivery() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a1-cmd");
    let res = case.result_path();
    let cmdline = format!("echo cmd-wt-ok> \"{}\"", res.to_string_lossy());
    let plan = wt()
        .build_launch(&job(cmd_shell(), &cmdline, &case.dir, None))
        .unwrap();
    spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    assert_eq!(waited.as_deref(), Some("cmd-wt-ok"));
    kill_pids(&pids_matching("squirl-m0-a1-cmd", "cmd.exe"));
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a1_wt_gitbash_command_delivery() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a1-bash");
    let res = case.result_path();
    // bash 路径用正斜杠，避免反斜杠被 bash 当转义
    let res_fwd = res.to_string_lossy().replace('\\', "/");
    let cmdline = format!("echo bash-wt-ok > \"{res_fwd}\"");
    let plan = wt()
        .build_launch(&job(git_bash_shell(), &cmdline, &case.dir, None))
        .unwrap();
    let _proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    assert_eq!(waited.as_deref(), Some("bash-wt-ok"));
    kill_pids(&pids_matching("squirl-m0-a1-bash", "bash.exe"));
}

// ---------- A2：conhost × 三 shell ----------

#[test]
#[ignore = "Windows 实机冒烟"]
fn a2_conhost_pwsh() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a2-pwsh");
    let res = case.result_path();
    let script_dir = case.dir.join("scripts");
    let cmdline = format!(
        "Set-Content -LiteralPath '{}' -Value 'pwsh-conhost-ok'",
        res.to_string_lossy()
    );
    let plan = ConsoleHostAdapter
        .build_launch(&job(pwsh_shell(), &cmdline, &case.dir, Some(script_dir)))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    assert_eq!(waited.as_deref(), Some("pwsh-conhost-ok"));
    proc.terminate();
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a2_conhost_cmd() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a2-cmd");
    let res = case.result_path();
    let cmdline = format!("echo cmd-conhost-ok> \"{}\"", res.to_string_lossy());
    let plan = ConsoleHostAdapter
        .build_launch(&job(cmd_shell(), &cmdline, &case.dir, None))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    assert_eq!(waited.as_deref(), Some("cmd-conhost-ok"));
    proc.terminate();
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a2_conhost_gitbash() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a2-bash");
    let res = case.result_path();
    let res_fwd = res.to_string_lossy().replace('\\', "/");
    let cmdline = format!("echo bash-conhost-ok > \"{res_fwd}\"");
    let plan = ConsoleHostAdapter
        .build_launch(&job(git_bash_shell(), &cmdline, &case.dir, None))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    assert_eq!(waited.as_deref(), Some("bash-conhost-ok"));
    proc.terminate();
}

// ---------- A3：转义矩阵 ----------

#[test]
#[ignore = "Windows 实机冒烟"]
fn a3_pwsh_spaces_quotes_amp_chain() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a3-pwsh");
    let res = case.result_path();
    let res_s = res.to_string_lossy().into_owned();
    let script_dir = case.dir.join("scripts");

    // 空格 + 引号 + & 字面量（pwsh 语法，验证传输保真）
    let cmdline = format!(
        r#"Set-Content -LiteralPath '{res_s}' -Value ("a  b c | it's `"quoted`" a&b")"#,
    );
    let plan = ConsoleHostAdapter
        .build_launch(&job(pwsh_shell(), &cmdline, &case.dir, Some(script_dir.clone())))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    proc.terminate();
    assert_eq!(
        waited.as_deref(),
        Some("a  b c | it's \"quoted\" a&b"),
        "pwsh 空格/引号/& 传输保真"
    );

    // && 管道链（pwsh7 支持）
    std::fs::remove_file(&res).ok();
    let cmdline = format!(
        "Write-Output left && Write-Output right && Set-Content -LiteralPath '{res_s}' -Value 'and-chain-ok'"
    );
    let plan = ConsoleHostAdapter
        .build_launch(&job(pwsh_shell(), &cmdline, &case.dir, Some(script_dir)))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    proc.terminate();
    assert_eq!(waited.as_deref(), Some("and-chain-ok"), "pwsh7 && 链应成功");
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a3_ps51_and_chain_fails_but_launcher_survives() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a3-ps51");
    let res = case.result_path();
    let cmdline = format!(
        "Write-Output left && Set-Content -LiteralPath '{}' -Value 'never'",
        res.to_string_lossy()
    );
    // -Command 形态（无 -NoExit）：`&&` 解析失败 → 进程非零退出，
    // 错误留在终端窗口；「应用不崩」= 启动器（测试进程）正常走完全程。
    let mut j = job(ps51_shell(), &cmdline, &case.dir, Some(case.dir.join("s")));
    j.keep_open = false;
    let plan = ConsoleHostAdapter.build_launch(&j).unwrap();
    let proc = spawn_ext(&plan);
    let code = wait_exit(&proc, Duration::from_secs(30)).expect("PS5.1 应自行退出");
    assert_ne!(code, 0, "PS5.1 的 && 应解析失败（非零退出）");
    assert!(read_result(&res).is_none(), "&& 后段不应被执行");
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a3_cmd_ampersand_and_quotes() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a3-cmd");
    let res = case.result_path();
    let res_s = res.to_string_lossy().into_owned();
    // & 作为 cmd 分隔符必须原样抵达
    let cmdline = format!("echo a & echo b> \"{res_s}\"");
    let plan = ConsoleHostAdapter
        .build_launch(&job(cmd_shell(), &cmdline, &case.dir, None))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    proc.terminate();
    assert_eq!(waited.as_deref(), Some("b"), "cmd & 分隔符语义保真（b 写入结果）");

    // 引号原样传递
    std::fs::remove_file(&res).ok();
    let cmdline = format!("echo \"quoted text\" > \"{res_s}\"");
    let plan = ConsoleHostAdapter
        .build_launch(&job(cmd_shell(), &cmdline, &case.dir, None))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    proc.terminate();
    assert_eq!(
        waited.as_deref(),
        Some("\"quoted text\""),
        "cmd 引号原样保真（cmd echo 不剥引号）"
    );
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a3_bash_quotes_and_chain() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a3-bash");
    let res = case.result_path();
    let res_fwd = res.to_string_lossy().replace('\\', "/");
    let cmdline = format!("echo \"it's ok  a&b\" > \"{res_fwd}\" && echo chained >> \"{res_fwd}\"");
    let plan = ConsoleHostAdapter
        .build_launch(&job(git_bash_shell(), &cmdline, &case.dir, None))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || {
        read_result(&res).filter(|s| s.contains("chained"))
    });
    proc.terminate();
    assert_eq!(
        waited.as_deref(),
        Some("it's ok  a&b\nchained"),
        "bash 引号/&/&& 传输保真"
    );
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a3_chinese_path_cwd_all_shells() {
    let _g = SERIAL.lock().unwrap();
    // 中文 + 空格目录
    let case = TempCase::new("中文 路径 冒烟");
    let res = case.result_path();

    // pwsh：相对写入（cwd = 中文目录）
    let script_dir = case.dir.join("scripts");
    let plan = ConsoleHostAdapter
        .build_launch(&job(
            pwsh_shell(),
            "Set-Content -LiteralPath 'result.txt' -Value 'pwsh-cn-ok'",
            &case.dir,
            Some(script_dir),
        ))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    proc.terminate();
    assert_eq!(waited.as_deref(), Some("pwsh-cn-ok"), "pwsh 中文路径 cwd");

    std::fs::remove_file(&res).ok();
    // cmd：相对写入
    let plan = ConsoleHostAdapter
        .build_launch(&job(cmd_shell(), "echo cmd-cn-ok> result.txt", &case.dir, None))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    proc.terminate();
    assert_eq!(waited.as_deref(), Some("cmd-cn-ok"), "cmd 中文路径 cwd");

    std::fs::remove_file(&res).ok();
    // bash：相对写入
    let plan = ConsoleHostAdapter
        .build_launch(&job(git_bash_shell(), "echo bash-cn-ok > result.txt", &case.dir, None))
        .unwrap();
    let proc = spawn_ext(&plan);
    let waited = poll_until(Duration::from_secs(20), || read_result(&res));
    proc.terminate();
    assert_eq!(waited.as_deref(), Some("bash-cn-ok"), "bash 中文路径 cwd");
}

// ---------- A4：保持打开（exit 1 不闪退）----------

fn assert_keep_open(proc: &ExternalProcess) {
    // 故意失败命令执行完后，shell 进程必须仍存活（窗口 = 交互会话）
    let alive = poll_until(Duration::from_secs(8), || {
        if proc.is_alive() { None } else { Some(false) }
    });
    if alive == Some(false) {
        let code = proc.try_exit_code();
        panic!("窗口进程提前退出：exit_code={code:?}");
    }
    // poll 超时返回 None = 全程存活 ✓
    proc.terminate();
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a4_keep_open_cmd_failing_command() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a4-cmd");
    // cmd 的 `exit` 内建命令会连 /k 一起终结（cmd 语义所限，见定版表已知限制）；
    // A4 的验收意图是「失败命令的错误在窗口可见、窗口不闪退」，用
    // type 不存在文件制造非零失败。
    let plan = ConsoleHostAdapter
        .build_launch(&job(
            cmd_shell(),
            "type C:\\surely-missing-squirl-a4.txt",
            &case.dir,
            None,
        ))
        .unwrap();
    let proc = spawn_ext(&plan);
    assert_keep_open(&proc);
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a4_keep_open_pwsh_exit1_file_form() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a4-pwsh");
    let script_dir = case.dir.join("scripts");
    let plan = ConsoleHostAdapter
        .build_launch(&job(pwsh_shell(), "exit 1", &case.dir, Some(script_dir)))
        .unwrap();
    let proc = spawn_ext(&plan);
    assert_keep_open(&proc);
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a4_keep_open_bash_exit1_subshell() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a4-bash");
    let plan = ConsoleHostAdapter
        .build_launch(&job(git_bash_shell(), "exit 1", &case.dir, None))
        .unwrap();
    let proc = spawn_ext(&plan);
    assert_keep_open(&proc);
}

#[test]
#[ignore = "Windows 实机冒烟"]
fn a4_keep_open_wt_pwsh() {
    let _g = SERIAL.lock().unwrap();
    let case = TempCase::new("a4-wt");
    let script_dir = case.dir.join("scripts");
    let plan = wt()
        .build_launch(&job(pwsh_shell(), "exit 1", &case.dir, Some(script_dir)))
        .unwrap();
    // wt.exe 本体很快退出；keep-open 的证据 = 新 pwsh 进程存活
    // （pwsh 冷启动 + wt 建窗需要时间，轮询而非固定 sleep）
    let _proc = spawn_ext(&plan);
    let marker = "squirl-m0-a4-wt";
    let new_pids = poll_until(Duration::from_secs(15), || {
        let p = pids_matching(marker, "pwsh.exe");
        if p.is_empty() { None } else { Some(p) }
    })
    .expect("WT 窗口内应出现新的 keep-open pwsh");
    std::thread::sleep(Duration::from_secs(3));
    let still = pids_matching(marker, "pwsh.exe");
    assert!(
        new_pids.iter().any(|p| still.contains(p)),
        "exit 1 后 keep-open pwsh 仍应存活"
    );
    kill_pids(&new_pids.to_vec());
}

// ---------- 辅助 ----------

/// 按「命令行包含标记」匹配进程 PID（tasklist 不含完整命令行，
/// 用 PowerShell CIM 查询）。
fn pids_matching(marker: &str, image: &str) -> Vec<u32> {
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"Name='{img}'\" | Where-Object {{ $_.CommandLine -like '*{marker}*' }} | Select-Object -ExpandProperty ProcessId",
        img = image
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .expect("powershell CIM 查询");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .collect()
}
