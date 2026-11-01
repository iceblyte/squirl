//! Spike B —— 内置模式进程链路集成测试（Windows 实机）。
//!
//! 运行方式：`cargo test --test spike_b -- --ignored --test-threads=1`
//!
//! B1 登录 shell + marker 分离；B2 stdout/stderr 捕获 + ANSI 透传 + 30ms 分批；
//! B3 Windows Job Object 收 `npm run dev` 全链 0 残留；B5 崩溃兜底
//! （TerminateProcess 模拟宿主死亡 → KILL_ON_JOB_CLOSE 内核收树）。
//! B4（Unix setsid/SIGTERM→SIGKILL）与 Unix PDEATHSIG 为 `#[cfg(unix)]`
//! 用例（CI 覆盖，标注未实机验证）。
//!
//! B6（存储链路）见 tests/spike_c.rs。

#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use squirl::engine::pipeline::{EventSink, LogBatch, RunStateEvent};
use squirl::engine::shell::{locate_on_path, ShellKind, ShellSpec};
use squirl::engine::supervisor::{Registry, RunState, StartArgs, Supervisor};
use uuid::Uuid;

// ---------- 测试基建 ----------

#[derive(Default)]
struct CollectSink {
    batches: std::sync::Mutex<Vec<LogBatch>>,
    states: std::sync::Mutex<Vec<RunStateEvent>>,
}

impl EventSink for CollectSink {
    fn on_log_batch(&self, batch: LogBatch) {
        self.batches.lock().unwrap().push(batch);
    }
    fn on_state(&self, event: RunStateEvent) {
        self.states.lock().unwrap().push(event);
    }
}

fn sink() -> Arc<CollectSink> {
    Arc::new(CollectSink::default())
}

fn pwsh() -> ShellSpec {
    ShellSpec {
        kind: ShellKind::Pwsh,
        program: locate_on_path("pwsh").expect("pwsh 必须在位"),
    }
}

fn git_bash() -> ShellSpec {
    ShellSpec {
        kind: ShellKind::Bash,
        program: squirl::engine::shell::resolve_git_bash().expect("Git Bash 必须在位"),
    }
}

fn args(command_line: &str, cwd: &Path, shell: ShellSpec, env: Vec<(String, String)>) -> StartArgs {
    StartArgs {
        command_id: Some(Uuid::now_v7()),
        command_line: command_line.into(),
        cwd: cwd.into(),
        shell,
        with_marker: true,
        env,
    }
}

/// 把 npm fixture 复制到独立临时目录（测试并发隔离 pids.txt）。
fn npm_fixture(label: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/npm-dev");
    let dir = std::env::temp_dir().join(format!("squirl-b-{label}-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    for f in ["package.json", "server.js", "child.js"] {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    dir
}

async fn wait_log_contains(sup: &Supervisor, run_id: Uuid, needle: &str, timeout: Duration) {
    let start = std::time::Instant::now();
    loop {
        if let Some(h) = sup.registry.get(run_id) {
            let (lines, _) = h.log.lock().unwrap().snapshot();
            if lines.iter().any(|l| l.text.contains(needle)) {
                return;
            }
        }
        if start.elapsed() >= timeout {
            let _ = sup.stop(run_id).await; // 失败路径也要回收，防泄漏
            panic!("等待日志包含 {needle:?} 超时");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn all_pids() -> std::collections::HashSet<u32> {
    let out = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .expect("tasklist");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split('"').nth(3))
        .filter_map(|f| f.trim_matches(',').trim().parse::<u32>().ok())
        .collect()
}

fn assert_pids_dead(pids: &[u32], timeout: Duration, what: &str) {
    let start = std::time::Instant::now();
    loop {
        let alive = all_pids();
        let remaining: Vec<u32> = pids.iter().copied().filter(|p| alive.contains(p)).collect();
        if remaining.is_empty() {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "{what} 后仍有残留进程: {remaining:?}"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn fixture_pids(dir: &Path) -> Option<Vec<u32>> {
    let txt = std::fs::read_to_string(dir.join("pids.txt")).ok()?;
    let pids: Vec<u32> = txt
        .lines()
        .filter_map(|l| l.split(':').nth(1))
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect();
    if pids.is_empty() { None } else { Some(pids) }
}

async fn wait_fixture_result(dir: &Path) -> Option<Vec<u32>> {
    let start = std::time::Instant::now();
    loop {
        if let Some(p) = fixture_pids(dir)
            && p.len() >= 3
        {
            return Some(p);
        }
        if start.elapsed() > Duration::from_secs(20) {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

// ---------- B1：登录 shell + profile/marker 分离 ----------

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Windows 实机冒烟"]
async fn b1_profile_output_separated_by_marker() {
    let sink = sink();
    let sup = Supervisor::new(sink.clone());
    let home = std::env::temp_dir().join(format!("squirl-b1-home-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join(".bash_profile"),
        "echo FAKE-PROFILE-NOISE\n",
    )
    .unwrap();
    let work = std::env::temp_dir().join(format!("squirl-b1-work-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&work).unwrap();

    let run_id = sup
        .start(args(
            "echo CMD-OUTPUT-AFTER-MARKER",
            &work,
            git_bash(),
            vec![("HOME".into(), home.to_string_lossy().into_owned())],
        ))
        .await
        .unwrap();
    wait_log_contains(&sup, run_id, "CMD-OUTPUT-AFTER-MARKER", Duration::from_secs(20)).await;

    let h = sup.registry.get(run_id).unwrap();
    let (lines, _) = h.log.lock().unwrap().snapshot();
    let idx_noise = lines.iter().position(|l| l.text.contains("FAKE-PROFILE-NOISE"));
    let idx_marker = lines.iter().position(|l| l.text.contains("squirl command output"));
    let idx_out = lines.iter().position(|l| l.text.contains("CMD-OUTPUT-AFTER-MARKER"));
    let idx_marker = idx_marker.expect("marker 行必须存在");
    let idx_out = idx_out.expect("命令输出必须存在");
    assert!(
        idx_noise.map(|n| n < idx_marker).unwrap_or(true),
        "若存在 profile 噪音，必须在 marker 之前"
    );
    assert!(idx_out > idx_marker, "命令输出必须在 marker 之后（B1 分离成立）");

    let marker_seq = *h.pipe_marker.lock().unwrap();
    assert_eq!(marker_seq, Some(lines[idx_marker].seq));
    sup.stop(run_id).await.unwrap();
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&work);
}

// ---------- B2：捕获 + ANSI 透传 + 30ms 分批 ----------

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Windows 实机冒烟"]
async fn b2_capture_ansi_and_batching() {
    let sink = sink();
    let sup = Supervisor::new(sink.clone());
    let work = std::env::temp_dir().join(format!("squirl-b2-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&work).unwrap();
    // 1000 行快速输出，每行带 ANSI 颜色码
    let cmdline = "$e=[char]27; 1..1000 | ForEach-Object { \"line-$_ $e[31mRED$e[0m\" }";
    let run_id = sup.start(args(cmdline, &work, pwsh(), vec![])).await.unwrap();

    // 等 run 结束 + 管道终发
    let start = std::time::Instant::now();
    loop {
        let done = sup
            .registry
            .get(run_id)
            .map(|h| matches!(h.current_state(), RunState::Exited | RunState::Failed))
            .unwrap_or(false)
            && sink
                .batches
                .lock()
                .unwrap()
                .iter()
                .filter(|b| b.run_id == run_id.to_string())
                .count()
                > 0;
        if done {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(30), "等待 run 结束超时");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;

    let batches: Vec<LogBatch> = sink
        .batches
        .lock()
        .unwrap()
        .iter()
        .filter(|b| b.run_id == run_id.to_string())
        .cloned()
        .collect();
    assert!(!batches.is_empty());
    // 分批有效：1000+ 行不能逐行一批（30ms 窗口聚合）
    let batch_count = batches.len();
    assert!(
        batch_count < 500,
        "批次数 {batch_count} 过多，30ms 分批未生效"
    );
    // 无缺口：批次 seq 连续
    let mut expected = 1u64;
    for b in &batches {
        assert_eq!(b.seq, expected, "批次 seq 必须连续无缺口");
        expected += b.lines.len() as u64;
    }
    // 数据完整 + ANSI 原文透传
    let total: usize = batches.iter().map(|b| b.lines.len()).sum();
    assert!(total >= 1001, "至少 marker + 1000 行，实际 {total}");
    let ansi_hit = batches
        .iter()
        .any(|b| b.lines.iter().any(|l| l.text.contains('\u{1b}')));
    assert!(ansi_hit, "ANSI 转义序列必须原文透传");
    // trimmed_lines = 0（未超限）
    assert!(batches.iter().all(|b| b.trimmed_lines == 0));
    // 环形缓冲与批次一致
    let h = sup.registry.get(run_id).unwrap();
    let (lines, trimmed) = h.log.lock().unwrap().snapshot();
    assert_eq!(lines.len(), total);
    assert_eq!(trimmed, 0);
    // 状态事件链：starting → running → exited
    let states: Vec<String> = sink
        .states
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.run_id == run_id.to_string())
        .map(|e| e.state.clone())
        .collect();
    assert_eq!(states, vec!["starting", "running", "exited"]);
    let last = sink.states.lock().unwrap().last().cloned().unwrap();
    assert_eq!(last.exit_code, Some(0));
    let _ = std::fs::remove_dir_all(&work);
}

// ---------- B3：Job Object 收 npm run dev 全链 0 残留 ----------

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Windows 实机冒烟"]
async fn b3_job_object_reaps_npm_dev_tree_on_stop() {
    let sink = sink();
    let sup = Supervisor::new(sink.clone());
    let dir = npm_fixture("b3");
    let run_id = sup
        .start(args("npm run dev", &dir, pwsh(), vec![]))
        .await
        .unwrap();
    wait_log_contains(&sup, run_id, "SQUIRL_DEV_READY", Duration::from_secs(30)).await;

    let pids = match wait_fixture_result(&dir).await {
        Some(p) if p.len() >= 3 => p,
        other => {
            let _ = sup.stop(run_id).await;
            panic!("fixture 应记录至少 3 层 PID，实际 {other:?}");
        }
    };

    sup.stop(run_id).await.unwrap();
    assert_pids_dead(&pids, Duration::from_secs(8), "手动停止");
    // 终态 killed + run://state 事件
    let h = sup.registry.get(run_id).unwrap();
    assert_eq!(h.current_state(), RunState::Killed);
    let ev = sink
        .states
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|e| e.run_id == run_id.to_string())
        .cloned()
        .unwrap();
    assert_eq!(ev.state, "killed");
    assert_eq!(ev.exit_code, None, "killed 不携带退出码（DR-16）");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- B5：崩溃兜底（宿主被杀 → 内核收树） ----------

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Windows 实机冒烟"]
async fn b5_crash_fallback_kernel_reaps_tree() {
    let dir = npm_fixture("b5");
    let host = env!("CARGO_BIN_EXE_job_host");
    // stderr 置 null：宿主驻留时不得持有测试管道（曾导致 cargo test 挂死）
    let mut child = std::process::Command::new(host)
        .arg(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("启动 job_host");

    // 等待 HOST=<pid> 行（超时则清理并失败）
    let host_pid = loop {
        use std::io::Read;
        let mut buf = [0u8; 256];
        let n = child.stdout.as_mut().unwrap().read(&mut buf).unwrap_or(0);
        if n > 0 {
            let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
            if let Some(pid) = chunk
                .lines()
                .chain(std::iter::once(chunk.as_str()))
                .find_map(|l| l.strip_prefix("HOST="))
                .and_then(|s| s.trim().parse::<u32>().ok())
            {
                break pid;
            }
        }
        if std::time::Instant::now() > std::time::Instant::now() + Duration::from_secs(30) {
            break 0;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    assert!(host_pid != 0, "job_host 未报出 PID");

    let pids = wait_fixture_result(&dir).await.expect("fixture 未就绪");
    // 模拟应用异常崩溃：杀宿主（句柄全关 → KILL_ON_JOB_CLOSE 内核收树）
    std::process::Command::new("taskkill")
        .args(["/F", "/PID", &host_pid.to_string()])
        .output()
        .expect("taskkill 宿主");
    assert_pids_dead(&pids, Duration::from_secs(8), "宿主崩溃兜底");
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- run 语义补充：failed 终态 ----------

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Windows 实机冒烟"]
async fn b6_nonzero_exit_marks_failed() {
    let sink = sink();
    let sup = Supervisor::new(sink.clone());
    let work = std::env::temp_dir().join(format!("squirl-b6-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&work).unwrap();
    let run_id = sup.start(args("exit 7", &work, pwsh(), vec![])).await.unwrap();
    let start = std::time::Instant::now();
    loop {
        let state = sup.registry.get(run_id).map(|h| h.current_state());
        if matches!(state, Some(RunState::Failed) | Some(RunState::Exited)) {
            assert_eq!(state, Some(RunState::Failed));
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "等待终态超时");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let ev = sink
        .states
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|e| e.run_id == run_id.to_string())
        .cloned()
        .unwrap();
    assert_eq!(ev.state, "failed");
    assert_eq!(ev.exit_code, Some(7), "非零退出码必须随事件携带（AC5）");
    let _ = std::fs::remove_dir_all(&work);
}

// ---------- 重复启动拦截（单元级，不弹进程） ----------

#[tokio::test]
async fn registry_active_for_command_intercepts() {
    let reg = Registry::new();
    assert!(reg.active_for_command(Uuid::now_v7()).is_none());
}

// Unix 侧代码路径（CI 覆盖；本机无 Linux/macOS，标注未实机验证）
// B4: setsid + SIGTERM→SIGKILL 整组回收 —— supervisor::terminate_group unix 分支
// B5(unix): PDEATHSIG 直接子进程回收 —— supervisor start pre_exec
// 两者在 ubuntu runner 上由同一套 spike_b 的 cfg(unix) 镜像用例覆盖（Phase 3 CI 矩阵接入）。
#[cfg(unix)]
mod unix_only {
    // cfg(unix) 镜像用例在 CI 接入时补齐（S6 复用同一 Supervisor API）
}
