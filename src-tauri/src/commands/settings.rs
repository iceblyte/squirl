//! settings 域命令（契约 §5.4.1）。

use crate::db::repo;
use crate::error::IpcError;
use crate::models::Settings;

/// `get_settings` — → `Settings`
#[tauri::command]
pub fn get_settings(db: tauri::State<'_, crate::db::Db>) -> Result<Settings, IpcError> {
    let conn = db.conn();
    repo::get_settings(&conn).map_err(IpcError::from)
}

/// `update_settings` — `SettingsPatch` → `Settings`（S1 先落命令层，S5 接 UI）
#[tauri::command]
pub fn update_settings(
    patch: Settings,
    db: tauri::State<'_, crate::db::Db>,
) -> Result<Settings, IpcError> {
    if patch.default_mode != "external" && patch.default_mode != "builtin" {
        return Err(IpcError::new("E_INVALID_INPUT", "default_mode 仅允许 external | builtin"));
    }
    if !(1..=10).contains(&patch.scan_depth) {
        return Err(IpcError::new("E_INVALID_INPUT", "scan_depth 必须在 1~10"));
    }
    let conn = db.conn();
    repo::update_settings(&conn, &patch).map_err(IpcError::from)
}
