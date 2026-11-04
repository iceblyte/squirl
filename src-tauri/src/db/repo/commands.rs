//! commands 表仓储（F3：每项目多条命名命令）。

use rusqlite::Connection;
use uuid::Uuid;

use crate::db::repo::projects::CommandRow;
use crate::error::{AppError, Result};

/// 新增命令（add_command；cwd=None = 项目根/主目录；project_id=None = 全局命令 DR-20）。
#[allow(clippy::too_many_arguments)]
pub fn add_command(
    conn: &Connection,
    project_id: Option<&str>,
    name: &str,
    command_line: &str,
    cwd: Option<&str>,
    fingerprint: bool,
    note: Option<&str>,
    env_vars: Option<&str>,
) -> Result<CommandRow> {
    let id = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO commands (id, project_id, name, command_line, cwd, fingerprint, note, env_vars) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![id, project_id, name, command_line, cwd, fingerprint as i64, note.unwrap_or(""), env_vars.unwrap_or("[]")],
    )
    .map_err(AppError::Db)?;
    Ok(CommandRow {
        id,
        project_id: project_id.map(String::from),
        name: name.to_string(),
        command_line: command_line.to_string(),
        cwd: cwd.map(String::from),
        last_run_at: None,
        last_exit_code: None,
        fingerprint,
        note: note.unwrap_or("").to_string(),
        env_vars: env_vars.unwrap_or("[]").to_string(),
    })
}

/// 更新命令（编辑允许；运行中进程不受影响——S6 起语义生效）。
pub fn update_command(
    conn: &Connection,
    id: &str,
    patch: CommandPatch,
) -> Result<CommandRow> {
    let n = conn
        .execute(
        "UPDATE commands SET name=?1, command_line=?2, cwd=?3, note=?4, env_vars=?5 WHERE id=?6",
        rusqlite::params![
            patch.name,
            patch.command_line,
            patch.cwd,
            patch.note.as_deref().unwrap_or(""),
            patch.env_vars.as_deref().unwrap_or("[]"),
            id
        ],
        )
        .map_err(AppError::Db)?;
    if n == 0 {
        return Err(AppError::InvalidInput(format!("命令不存在: {id}")));
    }
    get_command(conn, id)
}

#[derive(Debug, Clone)]
pub struct CommandPatch {
    pub name: String,
    pub command_line: String,
    pub cwd: Option<String>,
    pub note: Option<String>,
    pub env_vars: Option<String>,
}

/// IPC 输入形态（camelCase 反序列化）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandPatchDto {
    pub name: String,
    pub command_line: String,
    pub cwd: Option<String>,
    pub note: Option<String>,
    pub env_vars: Option<String>,
}

/// 删除命令。TODO(S6)：内置模式运行中 → E_CMD_RUNNING_NO_DELETE（查 run 注册表）。
pub fn delete_command(conn: &Connection, id: &str) -> Result<()> {
    let n = conn
        .execute("DELETE FROM commands WHERE id=?1", rusqlite::params![id])
        .map_err(AppError::Db)?;
    if n == 0 {
        return Err(AppError::InvalidInput(format!("命令不存在: {id}")));
    }
    Ok(())
}

pub fn get_command(conn: &Connection, id: &str) -> Result<CommandRow> {
    conn.query_row(
        "SELECT id, project_id, name, command_line, cwd, last_run_at, last_exit_code, fingerprint, note, env_vars FROM commands WHERE id=?1",
        rusqlite::params![id],
        |r| {
            Ok(CommandRow {
                id: r.get(0)?,
                project_id: r.get(1)?,
                name: r.get(2)?,
                command_line: r.get(3)?,
                cwd: r.get(4)?,
                last_run_at: r.get(5)?,
                last_exit_code: r.get(6)?,
                fingerprint: r.get::<_, i64>(7)? != 0,
                note: r.get(8)?,
                env_vars: r.get(9)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::InvalidInput(format!("命令不存在: {id}"))
        }
        other => AppError::Db(other),
    })
}

pub fn list_commands(conn: &Connection, project_id: &str) -> Result<Vec<CommandRow>> {
    let mut stmt = conn
        .prepare("SELECT id, project_id, name, command_line, cwd, last_run_at, last_exit_code, fingerprint, note, env_vars FROM commands WHERE project_id=?1 ORDER BY rowid")
        .map_err(AppError::Db)?;
    let rows = stmt
        .query_map(rusqlite::params![project_id], |r| {
            Ok(CommandRow {
                id: r.get(0)?,
                project_id: r.get(1)?,
                name: r.get(2)?,
                command_line: r.get(3)?,
                cwd: r.get(4)?,
                last_run_at: r.get(5)?,
                last_exit_code: r.get(6)?,
                fingerprint: r.get::<_, i64>(7)? != 0,
                note: r.get(8)?,
                env_vars: r.get(9)?,
            })
        })
        .map_err(AppError::Db)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(AppError::Db)
}

/// 全局命令列表（DR-20：project_id IS NULL）。
pub fn list_global_commands(conn: &Connection) -> Result<Vec<CommandRow>> {
    let mut stmt = conn
        .prepare("SELECT id, project_id, name, command_line, cwd, last_run_at, last_exit_code, fingerprint, note, env_vars FROM commands WHERE project_id IS NULL ORDER BY rowid")
        .map_err(AppError::Db)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CommandRow {
                id: r.get(0)?,
                project_id: r.get(1)?,
                name: r.get(2)?,
                command_line: r.get(3)?,
                cwd: r.get(4)?,
                last_run_at: r.get(5)?,
                last_exit_code: r.get(6)?,
                fingerprint: r.get::<_, i64>(7)? != 0,
                note: r.get(8)?,
                env_vars: r.get(9)?,
            })
        })
        .map_err(AppError::Db)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(AppError::Db)
}

/// 项目最近使用命令 id（卡片快启 G3：主按钮 = 最近使用命令）。
pub fn last_used_command(conn: &Connection, project_id: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT id FROM commands WHERE project_id=?1 AND last_run_at IS NOT NULL ORDER BY last_run_at DESC LIMIT 1",
        rusqlite::params![project_id],
        |r| r.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::Db(other)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{list_projects, register_project};

    fn setup() -> (Connection, String) {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db::migrate::schema_migrations()
            .to_latest(&mut conn)
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("proj");
        std::fs::create_dir_all(&p).unwrap();
        let card = register_project(&conn, &p, None).unwrap();
        (conn, card.id)
    }

    #[test]
    fn crud_roundtrip() {
        let (conn, pid) = setup();
        let c = add_command(&conn, Some(&pid), "dev", "npm run dev", None, false, None, None).unwrap();
        assert_eq!(c.name, "dev");
        assert_eq!(c.cwd, None);
        assert_eq!(c.env_vars, "[]");

        update_command(
            &conn,
            &c.id,
            CommandPatch {
                name: "serve".into(),
                command_line: "pnpm dev".into(),
                cwd: Some("packages/app".into()),
                note: None,
                env_vars: Some(r#"{"KEY":"val"}"#.into()),
            },
        )
        .unwrap();
        let got = get_command(&conn, &c.id).unwrap();
        assert_eq!(got.name, "serve");
        assert_eq!(got.cwd.as_deref(), Some("packages/app"));
        assert_eq!(got.env_vars, r#"{"KEY":"val"}"#);

        assert_eq!(list_commands(&conn, &pid).unwrap().len(), 1);
        delete_command(&conn, &c.id).unwrap();
        assert!(list_commands(&conn, &pid).unwrap().is_empty());
        assert!(delete_command(&conn, &c.id).is_err());
        assert!(list_projects(&conn).unwrap().len() == 1);
    }

    #[test]
    fn last_used_prefers_recent() {
        let (conn, pid) = setup();
        assert_eq!(last_used_command(&conn, &pid).unwrap(), None);
        let a = add_command(&conn, Some(&pid), "a", "a", None, false, None, None).unwrap();
        let _b = add_command(&conn, Some(&pid), "b", "b", None, false, None, None).unwrap();
        conn.execute(
            "UPDATE commands SET last_run_at='2026-11-01T00:00:00Z' WHERE id=?1",
            rusqlite::params![a.id],
        )
        .unwrap();
        assert_eq!(last_used_command(&conn, &pid).unwrap().as_deref(), Some(a.id.as_str()));
    }

    #[test]
    fn insert_command_helper_compat() {
        let (conn, pid) = setup();
        let id = crate::db::repo::insert_command(
            &conn,
            Uuid::parse_str(&pid).unwrap(),
            "dev",
            "npm run dev",
        )
        .unwrap();
        assert_eq!(get_command(&conn, &id.to_string()).unwrap().name, "dev");
    }

}
