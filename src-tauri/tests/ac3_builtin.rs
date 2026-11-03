//! AC3 内置半项：点击（start）到首行输出 ≤ 5s（Windows 实机）。
//! 走生产 Supervisor + 管道（M0 定版），sink 记录首批到达时间。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use squirl::engine::pipeline::{EventSink, LogBatch, RunStateEvent};
use squirl::engine::shell::{locate_on_path, ShellKind, ShellSpec};
use squirl::engine::supervisor::{RunState, StartArgs, Supervisor};
use uuid::Uuid;

#[derive(Default)]
struct TimingSink {
    first_batch_at: Mutex<Option<Instant>>,
    batches: Mutex<Vec<LogBatch>>,
    states: Mutex<Vec<RunStateEvent>>,
}

impl EventSink for TimingSink {
    fn on_log_batch(&self, batch: LogBatch) {
        let mut first = self.first_batch_at.lock().unwrap();
        if first.is_none() {
            *first = Some(Instant::now());
        }
        self.batches.lock().unwrap().push(batch);
    }
    fn on_state(&self, event: RunStateEvent) {
        self.states.lock().unwrap().push(event);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ac3_builtin_first_output_within_5s() {
    let sink = Arc::new(TimingSink::default());
    let sup = Supervisor::new(sink.clone());
    let dir = std::env::temp_dir().join(format!("squirl-ac3b-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).unwrap();

    let t0 = Instant::now();
    let run_id = sup
        .start(StartArgs {
            command_id: Some(Uuid::now_v7()),
            command_line: "Write-Output ac3-builtin-ok".into(),
            cwd: dir.clone(),
            project_root: Some(dir.clone()),
            fingerprint: false,
            shell: ShellSpec::new(ShellKind::Pwsh, locate_on_path("pwsh").unwrap()),
            with_marker: true,
            env: vec![],
        })
        .await
        .unwrap();

    // 等首批日志
    let deadline = t0 + Duration::from_secs(10);
    loop {
        if sink.first_batch_at.lock().unwrap().is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "等待首批日志超时");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let first = sink.first_batch_at.lock().unwrap().unwrap();
    let latency = first.duration_since(t0);
    assert!(
        latency.as_millis() <= 5000,
        "AC3：内置模式首行输出 {latency:?} > 5s"
    );

    // 等 run 终态（管道终发后收集全部行再断言——首批可能只有 profile 噪音）
    let deadline = t0 + Duration::from_secs(20);
    loop {
        let state = sup.registry.get(run_id).map(|h| h.current_state());
        if matches!(state, Some(RunState::Exited) | Some(RunState::Failed)) {
            break;
        }
        assert!(Instant::now() < deadline, "等待终态超时");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // marker 分离：marker 行存在且位于命令输出之前
    let all: Vec<String> = {
        let batches = sink.batches.lock().unwrap();
        batches
            .iter()
            .flat_map(|b| b.lines.iter().map(|l| l.text.clone()))
            .collect()
    };
    let marker_idx = all.iter().position(|l| l.contains("squirl command output"));
    let out_idx = all.iter().position(|l| l.contains("ac3-builtin-ok"));
    assert!(marker_idx.is_some(), "marker 行必须存在（B1 分离机制）");
    assert!(out_idx.is_some(), "命令输出必须到达");
    assert!(marker_idx.unwrap() < out_idx.unwrap(), "输出在 marker 之后");

    // 等 run 结束，终态 exited(0)
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let state = sup.registry.get(run_id).map(|h| h.current_state());
        if matches!(state, Some(RunState::Exited)) {
            break;
        }
        assert!(Instant::now() < deadline, "等待终态超时");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _ = std::fs::remove_dir_all(&dir);
}
