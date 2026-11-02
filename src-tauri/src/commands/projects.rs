//! projects 域命令（契约 §5.4.1）：登记 / 台账 / 搜索（MVP 范围子集）。

use crate::db::repo;
use crate::error::IpcError;

/// `register_project` — `(path, note?)` → `Project`
/// （发现区「登记」与手动注册同入口；E_DUPLICATE_PATH 防重复）。
#[tauri::command]
pub async fn register_project(
    path: String,
    note: Option<String>,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<repo::projects::ProjectCard, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::projects::register_project(&conn, std::path::Path::new(&path), note.as_deref())
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `list_projects` — → `ProjectCard[]`（含归属派生、失效标记，最近使用排序）。
#[tauri::command]
pub async fn list_projects(
    db: tauri::State<'_, crate::db::Db>,
) -> Result<Vec<repo::projects::ProjectCard>, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::projects::list_projects(&conn)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `search_projects` — `query` → `ProjectCard[]`（name/path/note/命令名，后端 SQL）。
#[tauri::command]
pub async fn search_projects(
    query: String,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<Vec<repo::projects::ProjectCard>, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::projects::search_projects(&conn, &query)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}
