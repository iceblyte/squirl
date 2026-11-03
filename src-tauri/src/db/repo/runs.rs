//! 运行结果写回（DR-16，退出四步串行第②步）。

use rusqlite::Connection;
use uuid::Uuid;

use crate::error::{AppError, Result};

/// 运行终态（DR-16）：
/// - `exited/failed` 写 `commands.last_run_at + last_exit_code`
/// - `killed` 不写 `last_exit_code`（保留上次值）、仅更新 `last_run_at`
/// - `projects.run_count` 双模式均累计（外部模式启动成功即 +1）
#[derive(Debug, Clone)]
pub struct RunFinal {
    pub command_id: Uuid,
    pub project_id: Uuid,
    pub outcome: RunOutcome,
}

#[derive(Debug, Clone)]
pub enum RunOutcome {
    Exited(i32),
    Failed(i32),
    /// 手动停止 / 应用退出统一回收
    Killed,
    /// 外部模式启动成功（无退出码语义，`last_exit_code` 恒 NULL）
    LaunchedExternal,
}

/// 外部模式启动成功写回（DR-16①：last_run_at 更新 + run_count +1，exit 恒 NULL）。
pub fn record_external_launch(
    conn: &mut Connection,
    command_id: Option<Uuid>,
    project_id: &str,
) -> Result<()> {
    let tx = conn.transaction().map_err(AppError::Db)?;
    if let Some(cid) = command_id {
        tx.execute(
            "UPDATE commands SET last_run_at=?1 WHERE id=?2",
            rusqlite::params![crate::clock::now_rfc3339(), cid.to_string()],
        )
        .map_err(AppError::Db)?;
    }
    tx.execute(
        "UPDATE projects SET run_count = run_count + 1 WHERE id=?1",
        rusqlite::params![project_id],
    )
    .map_err(AppError::Db)?;
    tx.commit().map_err(AppError::Db)?;
    Ok(())
}

/// 内置模式启动成功写回（DR-16①：run_count +1，last_run_at 一并刷新）。
pub fn record_builtin_launch(conn: &mut Connection, command_id: Uuid) -> Result<()> {
    let now = crate::clock::now_rfc3339();
    conn.execute(
        "UPDATE commands SET last_run_at=?1 WHERE id=?2",
        rusqlite::params![now, command_id.to_string()],
    )
    .map_err(AppError::Db)?;
    conn.execute(
        "UPDATE projects SET run_count = run_count + 1 WHERE id = (SELECT project_id FROM commands WHERE id=?1)",
        rusqlite::params![command_id.to_string()],
    )
    .map_err(AppError::Db)?;
    Ok(())
}

/// 内置终态写回（watcher 实时路径，DR-16②）：exited/failed 写退出码；
/// killed 保留上次值、仅刷新 last_run_at。
pub fn record_builtin_terminal(
    conn: &mut Connection,
    command_id: Option<Uuid>,
    state: &str,
    exit_code: Option<i32>,
) -> Result<()> {
    let Some(cid) = command_id else {
        return Ok(()); // 临时命令无行可写
    };
    let now = crate::clock::now_rfc3339();
    match state {
        "exited" | "failed" => {
            conn.execute(
                "UPDATE commands SET last_run_at=?1, last_exit_code=?2 WHERE id=?3",
                rusqlite::params![now, exit_code, cid.to_string()],
            )
            .map_err(AppError::Db)?;
        }
        // killed：不写 last_exit_code（保留上次值，DR-16②）
        _ => {
            conn.execute(
                "UPDATE commands SET last_run_at=?1 WHERE id=?2",
                rusqlite::params![now, cid.to_string()],
            )
            .map_err(AppError::Db)?;
        }
    }
    Ok(())
}

/// 终态写回（退出四步串行的第②步，DR-14）。
pub fn write_back_finals(conn: &mut Connection, finals: &[RunFinal]) -> Result<()> {
    let tx = conn.transaction().map_err(AppError::Db)?;
    for f in finals {
        let now = crate::clock::now_rfc3339();
        match f.outcome {
            RunOutcome::Exited(code) | RunOutcome::Failed(code) => {
                tx.execute(
                    "UPDATE commands SET last_run_at=?1, last_exit_code=?2 WHERE id=?3",
                    rusqlite::params![now, code, f.command_id.to_string()],
                )
                .map_err(AppError::Db)?;
            }
            RunOutcome::Killed => {
                tx.execute(
                    "UPDATE commands SET last_run_at=?1 WHERE id=?2",
                    rusqlite::params![now, f.command_id.to_string()],
                )
                .map_err(AppError::Db)?;
            }
            RunOutcome::LaunchedExternal => {
                tx.execute(
                    "UPDATE commands SET last_run_at=?1 WHERE id=?2",
                    rusqlite::params![now, f.command_id.to_string()],
                )
                .map_err(AppError::Db)?;
            }
        }
        tx.execute(
            "UPDATE projects SET run_count = run_count + 1 WHERE id=?1",
            rusqlite::params![f.project_id.to_string()],
        )
        .map_err(AppError::Db)?;
    }
    tx.commit().map_err(AppError::Db)?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{insert_command, insert_project};
    use std::path::Path;

    fn setup() -> (Connection, Uuid) {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db::migrate::schema_migrations()
            .to_latest(&mut conn)
            .unwrap();
        let pid = insert_project(&conn, Path::new("D:/tmp/p"), "proj").unwrap();
        (conn, pid)
    }

    #[test]
    fn write_back_follows_dr16() {
        let (mut conn, pid) = setup();
        let c1 = insert_command(&conn, pid, "dev", "npm run dev").unwrap();
        let c2 = insert_command(&conn, pid, "build", "npm run build").unwrap();
        let c3 = insert_command(&conn, pid, "test", "npm test").unwrap();
        conn.execute(
            "UPDATE commands SET last_exit_code=9 WHERE id=?1",
            rusqlite::params![c3.to_string()],
        )
        .unwrap();

        write_back_finals(
            &mut conn,
            &[
                RunFinal { command_id: c1, project_id: pid, outcome: RunOutcome::Exited(0) },
                RunFinal { command_id: c2, project_id: pid, outcome: RunOutcome::Failed(3) },
                RunFinal { command_id: c3, project_id: pid, outcome: RunOutcome::Killed },
            ],
        )
        .unwrap();

        let get = |id: Uuid| -> Option<Option<i32>> {
            conn.query_row(
                "SELECT last_exit_code FROM commands WHERE id=?1",
                rusqlite::params![id.to_string()],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(get(c1), Some(Some(0)));
        assert_eq!(get(c2), Some(Some(3)));
        assert_eq!(get(c3), Some(Some(9)), "killed 必须保留上次值（DR-16②）");
        for c in [c1, c2, c3] {
            let at: Option<String> = conn
                .query_row(
                    "SELECT last_run_at FROM commands WHERE id=?1",
                    rusqlite::params![c.to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(at.is_some(), "last_run_at 必须更新");
        }
        let rc: i64 = conn
            .query_row(
                "SELECT run_count FROM projects WHERE id=?1",
                rusqlite::params![pid.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rc, 3);
    }

    #[test]
    fn external_launch_counts_and_keeps_exit_null() {
        let (mut conn, pid) = setup();
        let c = insert_command(&conn, pid, "open", "wt.exe").unwrap();
        write_back_finals(
            &mut conn,
            &[RunFinal { command_id: c, project_id: pid, outcome: RunOutcome::LaunchedExternal }],
        )
        .unwrap();
        let code: Option<i32> = conn
            .query_row(
                "SELECT last_exit_code FROM commands WHERE id=?1",
                rusqlite::params![c.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(code, None, "外部模式 last_exit_code 恒 NULL");
        let rc: i64 = conn
            .query_row(
                "SELECT run_count FROM projects WHERE id=?1",
                rusqlite::params![pid.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rc, 1, "外部模式启动成功即累计 run_count（DR-16①）");
    }
}
