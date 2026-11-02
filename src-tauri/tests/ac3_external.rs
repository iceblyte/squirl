//! AC3 外部半项：点击到终端窗口出现 ≤ 2s（Windows 实机）。
//!
//! 通过与 Tauri 命令完全相同的 `launch_external` 代码路径：
//! spawn（含窗口创建）计时 + 命令真实执行（结果文件落地）+ keep-open 清理。

#![cfg(windows)]

use std::time::{Duration, Instant};

use squirl::commands::run::launch_external;
use squirl::db::repo::{add_root, register_project};
use squirl::uuid::Uuid;

fn wait_result(path: &Path, timeout: Duration) -> Option<String> {
    let start = Instant::now();
    loop {
        if let Ok(s) = std::fs::read_to_string(path) {
            return Some(s.trim_start_matches('\u{feff}').trim().to_string());
        }
        if start.elapsed() > timeout {
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn pids_matching(marker: &str, image: &str) -> Vec<u32> {
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"Name='{img}'\" | Where-Object {{ $_.CommandLine -like '*{marker}*' }} | Select-Object -ExpandProperty ProcessId",
        img = image
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .expect("powershell");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .collect()
}

use std::path::Path;

#[test]
fn ac3_external_window_within_2s_and_command_runs() {
    // 数据目录 + DB（走生产 Db::open，含迁移与备份链路）
    let data_dir = std::env::temp_dir().join(format!("squirl-ac3-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&data_dir).unwrap();
    let (db, _) = squirl::db::Db::open(&data_dir.join("squirl.db")).unwrap();

    // 登记项目 + 命令（与 UI 操作等价的数据准备）
    let proj_dir = data_dir.join("proj");
    std::fs::create_dir_all(&proj_dir).unwrap();
    let card = register_project(&db.conn(), &proj_dir, None).unwrap();
    add_root(&db.conn(), &proj_dir).unwrap();
    let cmd = squirl::db::repo::add_command(
        &db.conn(),
        &card.id,
        "ac3",
        "Set-Content -LiteralPath 'ac3-result.txt' -Value 'ac3-ok'",
        None,
    )
    .unwrap();

    let result_path = proj_dir.join("ac3-result.txt");

    // 计时：与用户点击等价的启动路径
    let t0 = Instant::now();
    let pid = launch_external(
        &db,
        Some(Uuid::parse_str(&cmd.id).unwrap()),
        Some(Uuid::parse_str(&card.id).unwrap()),
        cmd.command_line.clone(),
        cmd.cwd.as_deref(),
        &proj_dir,
    )
    .expect("外部启动应成功");
    let spawn_ms = t0.elapsed().as_millis();
    assert!(spawn_ms <= 2000, "AC3：spawn 耗时 {spawn_ms}ms > 2000ms");

    // 命令真实执行（外部终端内 pwsh 写结果文件）
    let waited = wait_result(&result_path, Duration::from_secs(20));
    assert_eq!(waited.as_deref(), Some("ac3-ok"), "命令应在外部终端真实执行");

    // DR-16①：外部启动成功 → run_count +1、last_run_at 更新、exit 恒 NULL
    {
        let conn = db.conn();
        let rc: i64 = conn
            .query_row(
                "SELECT run_count FROM projects WHERE id=?1",
                rusqlite::params![card.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rc, 1);
        let (at, code): (Option<String>, Option<i32>) = conn
            .query_row(
                "SELECT last_run_at, last_exit_code FROM commands WHERE id=?1",
                rusqlite::params![cmd.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(at.is_some());
        assert_eq!(code, None, "外部模式 last_exit_code 恒 NULL");
    }

    // keep-open：新 pwsh 会话仍存活（其命令行含用例目录 → 精确清理）
    std::thread::sleep(Duration::from_millis(800));
    let keep = pids_matching("squirl-ac3", "pwsh.exe");
    // 注意：spawn 的是 cmd/pwsh（conhost+pwsh 路径）；即使 WT 路径下句柄已退，
    // 结果文件已证明窗口内命令执行成功。此处尽力清理。
    for p in keep {
        let _ = std::process::Command::new("taskkill").args(["/F", "/PID", &p.to_string()]).output();
    }
    let _ = pid;
    let _ = std::fs::remove_dir_all(&data_dir);
}
