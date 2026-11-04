//! scan 域命令（契约 §5.4.1 + 事件 §5.4.2）：单飞、暂停协议、会话内存发现区。
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use serde::Serialize;
use tauri::Emitter;
use crate::db::repo;
use crate::error::{AppError, IpcError};
use crate::scanner::{
    scan, DiscoveryEntry, PauseDecision, SharedSession, DEFAULT_DEPTH, MAX_DEPTH, SCAN_LIMIT,
};
/// `ScanTicket`（start_scan 出参；结果走事件）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanTicket {
    pub depth: u32,
    pub limit: usize,
}
/// `start_scan` — → `ScanTicket`（单飞；E_SCAN_IN_PROGRESS 携带现有进度）。
#[tauri::command]
pub async fn start_scan(
    app: tauri::AppHandle,
    db: tauri::State<'_, crate::db::Db>,
    session: tauri::State<'_, SharedSession>,
) -> Result<ScanTicket, IpcError> {
    spawn_scan_core(app, db.inner().clone(), session.inner().clone())
        .await
        .map_err(IpcError::from)
}
/// 扫描启动核心（命令层与 DR-31 文件监听共用；单飞检查在此）。
pub async fn spawn_scan_core(
    app: tauri::AppHandle,
    db: crate::db::Db,
    session: SharedSession,
) -> std::result::Result<ScanTicket, AppError> {
    // 单飞：已在扫描则拒绝并携带现有进度
    {
        let s = session.lock().unwrap();
        if s.running.is_some() {
            return Err(AppError::ScanInProgress);
        }
    }
    // 读取配置与启用根（阻塞线程）
    let (depth, roots, custom_excludes) = tokio::task::spawn_blocking(
        move || -> std::result::Result<(u32, Vec<crate::scanner::RootScanInput>, Vec<String>), AppError> {
            let conn = db.conn();
            let settings = repo::get_settings(&conn)?;
            let roots = repo::roots::list_roots(&conn)?
                .into_iter()
                .filter(|r| r.enabled)
                .map(|r| (r.id, std::path::PathBuf::from(r.path), r.scan_depth))
                .collect();
            Ok((
                settings.scan_depth,
                roots,
                settings.custom_excludes.unwrap_or_default(),
            ))
        },
    )
    .await
    .map_err(|e| AppError::Engine(format!("线程失败: {e}")))
    .and_then(std::convert::identity)?;
    let (decision_tx, decision_rx) = std::sync::mpsc::channel::<PauseDecision>();
    let token = Arc::new(tokio_util::sync::CancellationToken::new());
    {
        let mut s = session.lock().unwrap();
        s.running = Some(token.clone());
        s.decision_tx = Some(decision_tx);
        s.entries.clear(); // 瞬时语义：重扫重算
        s.last_count = 0;
    }
    let session2 = session.clone();
    let app_progress = app.clone();
    let app_pause = app.clone();
    let entry_count = Arc::new(AtomicUsize::new(0));
    let progress_count = entry_count.clone();
    let pause_count = entry_count.clone();
    let scan_token = (*token).clone();
    tokio::task::spawn_blocking(move || {
        let result = scan(
            &session2,
            &roots,
            depth.clamp(1, MAX_DEPTH).max(1),
            scan_token,
            move |entries, roots_done, roots_total| {
                progress_count.store(entries, Ordering::SeqCst);
                let _ = app_progress.emit(
                    "scan://progress",
                    serde_json::json!({ "rootsDone": roots_done, "rootsTotal": roots_total, "entries": entries }),
                );
            },
            move || {
                // 暂停：发事件，阻塞等前端 resolve_scan_pause
                let _ = app_pause.emit(
                    "scan://paused",
                    serde_json::json!({ "entries": pause_count.load(Ordering::SeqCst), "limit": SCAN_LIMIT }),
                );
                decision_rx.recv().unwrap_or(PauseDecision::Cancel)
            },
            &custom_excludes,
        );
        let app_done = app.clone();
        match result {
            Ok(outcome) => {
                let _ = app_done.emit(
                    "scan://completed",
                    serde_json::json!({ "count": outcome.count, "durationMs": outcome.duration_ms as u64 }),
                );
            }
            Err(e) => {
                tracing::warn!(target: "scan", error = %e, "扫描失败");
                let _ = app_done
                    .emit("scan://completed", serde_json::json!({ "count": 0, "durationMs": 0, "error": e.to_string() }));
            }
        }
    });
    Ok(ScanTicket { depth: depth.clamp(DEFAULT_DEPTH, MAX_DEPTH), limit: SCAN_LIMIT })
}
/// `resolve_scan_pause` — `'continue' | 'reduce_depth' | 'cancel'` → void。
#[tauri::command]
pub fn resolve_scan_pause(
    decision: String,
    session: tauri::State<'_, SharedSession>,
) -> Result<(), IpcError> {
    let d = PauseDecision::parse_decision(&decision).map_err(IpcError::from)?;
    let tx = session
        .lock()
        .unwrap()
        .decision_tx
        .clone();
    match tx {
        Some(tx) => {
            tx.send(d)
                .map_err(|e| IpcError::new("E_ENGINE", format!("裁决通道已关闭: {e}")))
        }
        None => Err(IpcError::new("E_INVALID_INPUT", "当前没有等待裁决的扫描")),
    }
}
/// `get_discovery` — → `DiscoveryEntry[]`（会话内存；重启为空，G5）。
#[tauri::command]
pub fn get_discovery(session: tauri::State<'_, SharedSession>) -> Result<Vec<DiscoveryEntry>, IpcError> {
    Ok(session.lock().unwrap().entries.clone())
}
