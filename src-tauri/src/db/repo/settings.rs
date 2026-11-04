//! settings 表仓储（单行表 CHECK(id=1)，附录 A #13）。

use rusqlite::Connection;

use crate::error::{AppError, Result};
use crate::models::Settings;

/// 读取单行设置（CHECK(id=1)；迁移时已插入默认行）。
pub fn get_settings(conn: &Connection) -> Result<Settings> {
    conn.query_row(
        "SELECT default_mode, default_emulator, default_shell, load_shell_profile, scan_depth, tray_enabled, custom_excludes, auto_watch FROM settings WHERE id=1",
        [],
        |r| {
            let excludes_json: Option<String> = r.get(6)?;
            Ok(Settings {
                default_mode: r.get(0)?,
                default_emulator: r.get(1)?,
                default_shell: r.get(2)?,
                load_shell_profile: r.get::<_, i64>(3)? != 0,
                scan_depth: r.get::<_, i64>(4)? as u32,
                tray_enabled: r.get::<_, i64>(5)? != 0,
                custom_excludes: excludes_json
                    .and_then(|j| serde_json::from_str(&j).ok()),
                auto_watch: r.get::<_, i64>(7)? != 0,
            })
        },
    )
    .map_err(AppError::Db)
}

/// 更新设置（S5 设置页接线；S1 先落仓储层）。
pub fn update_settings(conn: &Connection, patch: &Settings) -> Result<Settings> {
    let excludes_json = patch
        .custom_excludes
        .as_ref()
        .map(|list| serde_json::to_string(list).unwrap_or_else(|_| "null".into()));
    conn.execute(
        "UPDATE settings SET default_mode=?1, default_emulator=?2, default_shell=?3, load_shell_profile=?4, scan_depth=?5, tray_enabled=?6, custom_excludes=?7, auto_watch=?8 WHERE id=1",
        rusqlite::params![
            patch.default_mode,
            patch.default_emulator,
            patch.default_shell,
            patch.load_shell_profile as i64,
            patch.scan_depth as i64,
            patch.tray_enabled as i64,
            excludes_json,
            patch.auto_watch as i64,
        ],
    )
    .map_err(AppError::Db)?;
    get_settings(conn)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_with_defaults() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db::migrate::schema_migrations()
            .to_latest(&mut conn)
            .unwrap();
        let s = get_settings(&conn).unwrap();
        assert_eq!(s.default_mode, "external");
        assert_eq!(s.scan_depth, 1);
        assert!(s.load_shell_profile);
        assert_eq!(s.default_emulator, None);

        assert!(!s.tray_enabled);
        assert_eq!(s.custom_excludes, None);

        let patch = Settings {
            default_mode: "builtin".into(),
            default_emulator: Some("wt".into()),
            default_shell: Some("pwsh".into()),
            load_shell_profile: false,
            scan_depth: 3,
            tray_enabled: true,
            custom_excludes: Some(vec!["vendor".into(), ".turbo".into()]),
            auto_watch: true,
        };
        let s2 = update_settings(&conn, &patch).unwrap();
        assert_eq!(s2, patch);
    }
}
