//! commands 域命令（契约 §5.4.1）：命令 CRUD。
//! 删除检查：内置模式运行中 → E_CMD_RUNNING_NO_DELETE（S8）。

use crate::db::repo;
use crate::engine::supervisor::Supervisor;
use crate::error::{AppError, IpcError};

/// `add_command` — `(project_id?, name, command_line, cwd?, fingerprint?)` → `Command`。
#[tauri::command]
pub async fn add_command(
    project_id: Option<String>,
    name: String,
    command_line: String,
    cwd: Option<String>,
    fingerprint: Option<bool>,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<repo::projects::CommandRow, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::add_command(
            &conn,
            project_id.as_deref(),
            &name,
            &command_line,
            cwd.as_deref(),
            fingerprint.unwrap_or(false),
        )
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

/// `delete_command` — `id` → void（内置运行中 → E_CMD_RUNNING_NO_DELETE）。
#[tauri::command]
pub async fn delete_command(
    id: String,
    db: tauri::State<'_, crate::db::Db>,
    supervisor: tauri::State<'_, Supervisor>,
) -> Result<(), IpcError> {
    let db = db.inner().clone();
    let supervisor = supervisor.inner().clone();
    tokio::task::spawn_blocking(move || {
        if let Ok(cid) = uuid::Uuid::parse_str(&id)
            && supervisor.registry.active_for_command(cid).is_some()
        {
            return Err(AppError::CmdRunningNoDelete(id));
        }
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

/// `list_global_commands` — → `Command[]`（DR-20：project_id IS NULL）。
#[tauri::command]
pub async fn list_global_commands(
    db: tauri::State<'_, crate::db::Db>,
) -> Result<Vec<repo::projects::CommandRow>, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        repo::list_global_commands(&conn)
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// DR-22：可用 IDE 探测（PATH 上 code / cursor）。
#[tauri::command]
pub fn list_available_ides() -> Result<Vec<String>, IpcError> {
    let mut out = Vec::new();
    for (name, probe) in [("vscode", "code"), ("cursor", "cursor")] {
        if crate::engine::shell::locate_on_path(probe).is_some() {
            out.push(name.to_string());
        }
    }
    Ok(out)
}

/// DR-22：在 IDE 中打开项目文件夹（fire-and-forget；Windows 经 cmd /c 解析 .cmd shim）。
#[tauri::command]
pub fn open_in_ide(path: String, ide: String) -> Result<(), IpcError> {
    use std::os::windows::process::CommandExt;
    let probe = match ide.as_str() {
        "vscode" => "code",
        "cursor" => "cursor",
        other => return Err(IpcError::new("E_INVALID_INPUT", format!("未知 IDE: {other}"))),
    };
    let program = crate::engine::shell::locate_on_path(probe)
        .ok_or_else(|| IpcError::new("E_NO_EMULATOR", format!("未找到 {probe}")))?;
    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/c", &program, &path]).creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd.spawn()
        .map_err(|e| IpcError::new("E_LAUNCH_FAILED", format!("IDE 启动失败: {e}")))?;
    Ok(())
}
