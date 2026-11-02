//! commands 域命令（契约 §5.4.1）：命令 CRUD。
//!
//! E_CMD_RUNNING_NO_DELETE 归 S6（run 注册表落地后接查）。

use crate::db::repo;
use crate::error::IpcError;

/// `add_command` — `(project_id, name, command_line, cwd?)` → `Command`。
#[tauri::command]
pub async fn add_command(
    project_id: String,
    name: String,
    command_line: String,
    cwd: Option<String>,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<repo::projects::CommandRow, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::add_command(&conn, &project_id, &name, &command_line, cwd.as_deref())
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `update_command` — `(id, patch)` → `Command`（编辑不影响已运行进程）。
#[tauri::command]
pub async fn update_command(
    id: String,
    patch: repo::CommandPatchDto,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<repo::projects::CommandRow, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::update_command(
            &conn,
            &id,
            repo::CommandPatch {
                name: patch.name,
                command_line: patch.command_line,
                cwd: patch.cwd,
            },
        )
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `delete_command` — `id` → void。
#[tauri::command]
pub async fn delete_command(id: String, db: tauri::State<'_, crate::db::Db>) -> Result<(), IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::delete_command(&conn, &id)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `list_commands` — `project_id` → `Command[]`。
#[tauri::command]
pub async fn list_commands(
    project_id: String,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<Vec<repo::projects::CommandRow>, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::list_commands(&conn, &project_id)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}
