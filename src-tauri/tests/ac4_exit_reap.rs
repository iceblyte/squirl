//! AC4：正常退出路径零残留，10/10 用例（重点 npm run dev 孙进程，Windows 实机）。
//!
//! 与应用退出钩子走同一条代码路径：`Supervisor::reap_all()`（DR-14 第①步）
//! + watcher 实时终态写回（第②步）。10 轮独立 fixture 全链回收断言。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use squirl::engine::pipeline::EventSink;
use squirl::engine::shell::{locate_on_path, ShellKind, ShellSpec};
use squirl::engine::supervisor::{StartArgs, Supervisor};
use uuid::Uuid;

#[derive(Default)]
struct SilentSink;

impl EventSink for SilentSink {
    fn on_log_batch(&self, _: squirl::engine::pipeline::LogBatch) {}
    fn on_state(&self, _: squirl::engine::pipeline::RunStateEvent) {}
}

fn npm_fixture(label: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/npm-dev");
    let dir = std::env::temp_dir().join(format!("squirl-ac4-{label}-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    for f in ["package.json", "server.js", "child.js"] {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    dir
}

fn fixture_pids(dir: &std::path::Path) -> Vec<u32> {
    let txt = std::fs::read_to_string(dir.join("pids.txt")).expect("pids.txt");
    txt.lines()
        .filter_map(|l| l.split(':').nth(1))
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect()
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
        assert!(start.elapsed() < timeout, "{what} 后仍有残留: {remaining:?}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ac4_exit_reap_zero_residue_10_rounds() {
    for round in 1..=10u32 {
        let sink = Arc::new(SilentSink);
        let sup = Supervisor::new(sink);
        let dir = npm_fixture(&format!("r{round}"));
        let run_id = sup
            .start(StartArgs {
                command_id: Some(Uuid::now_v7()),
                command_line: "npm run dev".into(),
                cwd: dir.clone(),
                shell: ShellSpec::new(ShellKind::Pwsh, locate_on_path("pwsh").unwrap()),
                with_marker: true,
                env: vec![],
            })
            .await
            .unwrap();

        // 等 READY（三层链就绪）
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            let ready = sup
                .registry
                .get(run_id)
                .map(|h| {
                    let lines = h.log.lock().unwrap().snapshot().0;
                    lines.iter().any(|l| l.text.contains("SQUIRL_DEV_READY"))
                })
                .unwrap_or(false);
            if ready {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "第 {round} 轮 fixture 未就绪"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        let pids = fixture_pids(&dir);
        assert!(pids.len() >= 3, "第 {round} 轮应有三层 PID");

        // 退出钩子第①步：reap_all（与 lib.rs ExitRequested 同一路径）
        sup.reap_all().await;
        assert_pids_dead(&pids, Duration::from_secs(10), &format!("第 {round} 轮退出回收"));

        let _ = std::fs::remove_dir_all(&dir);
        println!("round {round}/10: 0 residue ✓");
    }
}
