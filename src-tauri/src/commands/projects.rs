//! projects 域命令（契约 §5.4.1）：登记 / 台账 / 搜索 / 详情 / 更新 / 重定位 / 删除。

use crate::db::repo;
use crate::error::{AppError, IpcError};

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

/// `get_project` — `id` → `ProjectDetail`（含 commands）。
#[tauri::command]
pub async fn get_project(
    id: String,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<repo::projects::ProjectDetail, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::projects::get_project(&conn, &id)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatch {
    pub note: Option<String>,
    /// S4/S5：execution_override 覆盖（exec_mode/exec_emulator/exec_shell）
    #[serde(default)]
    pub execution_override: Option<serde_json::Value>,
}

/// `update_project` — `(id, { note? })` → `Project`
/// （execution_override 半边归 S4/S5 接线，入参形状已按契约预留）。
#[tauri::command]
pub async fn update_project(
    id: String,
    patch: ProjectPatch,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<repo::projects::ProjectCard, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        if let Some(note) = &patch.note {
            return repo::projects::update_note(&conn, &id, note);
        }
        // 仅 override 或空 patch：返回现状（S4 落 override 写入）
        repo::projects::list_projects(&conn)?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| crate::error::AppError::InvalidInput(format!("项目不存在: {id}")))
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `relocate_project` — `(id, new_path)` → `Project`（G2：name 默认取新文件夹名）。
#[tauri::command]
pub async fn relocate_project(
    id: String,
    new_path: String,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<repo::projects::ProjectCard, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::projects::relocate_project(&conn, &id, std::path::Path::new(&new_path))
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `delete_project` — `id` → void（G1；任一命令内置运行中 →
/// E_PROJECT_RUNNING_NO_DELETE；前端二次确认框明示级联命令数）。
#[tauri::command]
pub async fn delete_project(
    id: String,
    db: tauri::State<'_, crate::db::Db>,
    supervisor: tauri::State<'_, crate::engine::supervisor::Supervisor>,
) -> Result<(), IpcError> {
    let db = db.inner().clone();
    let supervisor = supervisor.inner().clone();
    tokio::task::spawn_blocking(move || {
        {
            let conn = db.conn();
            let ids: Vec<String> = conn
                .prepare("SELECT id FROM commands WHERE project_id=?1")
                .map_err(AppError::Db)?
                .query_map(rusqlite::params![id], |r| r.get(0))
                .map_err(AppError::Db)?
                .filter_map(std::result::Result::ok)
                .collect();
            for cid in ids {
                if let Ok(u) = uuid::Uuid::parse_str(&cid)
                    && supervisor.registry.active_for_command(u).is_some()
                {
                    return Err(AppError::ProjectRunningNoDelete(id));
                }
            }
        }
        let conn = db.conn();
        repo::projects::delete_project(&conn, &id)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `get_project_command_count` — 删除确认框「将同时删除 N 条命令」的数据源
/// （契约外辅助命令：纯读；S4 评审是否并入 get_project）。
#[tauri::command]
pub async fn get_project_command_count(
    id: String,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<i64, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::projects::command_count(&conn, &id)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}
