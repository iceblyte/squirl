//! DR-31 文件监听自动刷新：notify watcher（去抖 2s）→ 复用 start_scan 全链路。
//!
//! - 开关：settings.auto_watch（默认关；关闭时不建 watcher）。
//! - 监听失败（inotify 上限等）降级手动刷新 + 警告日志。
//! - 单飞：spawn_scan_core 内部拒绝并发（E_SCAN_IN_PROGRESS 静默忽略）。
//! - 已知限制：会话中新添加的根不在监听列表（下次重启生效）。

use std::sync::mpsc;
use std::time::Duration;

use tauri::Manager;

use crate::commands::scan::spawn_scan_core;

const DEBOUNCE: Duration = Duration::from_secs(2);

/// setup 阶段调用：读取 auto_watch 设置，开启时为启用根建立 watcher 线程。
pub fn spawn_auto_watch(app: tauri::AppHandle, db: crate::db::Db) {
    let auto_watch = {
        let conn = db.conn();
        crate::db::repo::get_settings(&conn)
            .map(|s| s.auto_watch)
            .unwrap_or(false)
    };
    if !auto_watch {
        return;
    }
    let roots: Vec<std::path::PathBuf> = {
        let conn = db.conn();
        crate::db::repo::roots::list_roots(&conn)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.enabled)
            .map(|r| std::path::PathBuf::from(r.path))
            .collect()
    };
    if roots.is_empty() {
        return;
    }

    std::thread::spawn(move || watch_loop(app, db, roots));
}

fn watch_loop(app: tauri::AppHandle, db: crate::db::Db, roots: Vec<std::path::PathBuf>) {
    use notify::Watcher;
    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    // watcher 必须存活到线程结束（drop = 停止监听）
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!(target: "watch", error = %e, "notify 初始化失败，降级手动刷新");
            return;
        }
    };
    for root in &roots {
        if let Err(e) = watcher.watch(root, notify::RecursiveMode::Recursive) {
            tracing::warn!(target: "watch", path = %root.display(), error = %e, "根目录监听失败");
        }
    }
    tracing::info!(target: "watch", roots = roots.len(), "文件监听已启动（去抖 2s）");

    loop {
        // 首事件：阻塞等待
        if rx.recv_timeout(Duration::from_secs(u64::MAX)).is_err() {
            break;
        }
        // 去抖：清空积压后静默 2s 才触发
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(_) => continue, // 还有事件，继续等静默
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        // 经 tauri 异步运行时触发扫描（单飞拒绝 = 已在扫描，静默）
        let app2 = app.clone();
        let db2 = db.clone();
        let _ = tauri::async_runtime::spawn(async move {
            let _ = spawn_scan_core(app2, db2, app2.state::<crate::scanner::SharedSession>().inner().clone());
        });
    }
}
