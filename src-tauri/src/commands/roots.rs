//! roots 域命令（契约 §5.4.1）。成功后由前端自动触发 start_scan（F1）。

use crate::db::repo;
use crate::error::IpcError;

/// `list_roots` — → `Root[]`
#[tauri::command]
pub async fn list_roots(db: tauri::State<'_, crate::db::Db>) -> Result<Vec<repo::roots::Root>, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::roots::list_roots(&conn)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `add_root` — `path` → `Root`（规范化 + 唯一校验 E_DUPLICATE_PATH）
#[tauri::command]
pub async fn add_root(
    path: String,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<repo::roots::Root, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::roots::add_root(&conn, std::path::Path::new(&path))
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `remove_root` — `id` → void（删根后条目保留、无归属）
#[tauri::command]
pub async fn remove_root(id: String, db: tauri::State<'_, crate::db::Db>) -> Result<(), IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::roots::remove_root(&conn, &id)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `set_root_enabled` — `(id, enabled)` → void
#[tauri::command]
pub async fn set_root_enabled(
    id: String,
    enabled: bool,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<(), IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::roots::set_root_enabled(&conn, &id, enabled)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}
