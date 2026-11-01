//! 最小仓储层（M0 范围）：运行结果终态写回（DR-16）+ 测试用插入助手。
//! 完整 CRUD（settings/roots/projects/commands）在 Phase 1 按契约补齐。

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

// ---------- 测试/初始化助手（Phase 1 扩展为完整仓储） ----------

pub fn insert_project(
    conn: &Connection,
    path: &std::path::Path,
    name: &str,
) -> Result<Uuid> {
    let id = Uuid::now_v7();
    let norm = crate::paths::normalize_key(path)?;
    let display = crate::paths::canonical(path)?.to_string_lossy().into_owned();
    conn.execute(
        "INSERT INTO projects (id, path, path_normalized, name, created_at) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![
            id.to_string(),
            display,
            norm,
            name,
            crate::clock::now_rfc3339()
        ],
    )
    .map_err(AppError::Db)?;
    Ok(id)
}

pub fn insert_command(
    conn: &Connection,
    project_id: Uuid,
    name: &str,
    command_line: &str,
) -> Result<Uuid> {
    let id = Uuid::now_v7();
    conn.execute(
        "INSERT INTO commands (id, project_id, name, command_line) VALUES (?1,?2,?3,?4)",
        rusqlite::params![id.to_string(), project_id.to_string(), name, command_line],
    )
    .map_err(AppError::Db)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Connection, Uuid) {
        let mut conn = Connection::open_in_memory().unwrap();
        super::super::migrate::schema_v1_migrations()
            .to_latest(&mut conn)
            .unwrap();
        let pid = insert_project(&conn, std::path::Path::new("D:/tmp/p"), "proj").unwrap();
        (conn, pid)
    }

    #[test]
    fn write_back_follows_dr16() {
        let (mut conn, pid) = setup();
        let c1 = insert_command(&conn, pid, "dev", "npm run dev").unwrap();
        let c2 = insert_command(&conn, pid, "build", "npm run build").unwrap();
        let c3 = insert_command(&conn, pid, "test", "npm test").unwrap();
        // c3 预置上次失败值，验证 killed 不覆盖
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

        let get = |id: Uuid, col: &str| -> Option<Option<i32>> {
            conn.query_row(
                &format!("SELECT {col} FROM commands WHERE id=?1"),
                rusqlite::params![id.to_string()],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(get(c1, "last_exit_code"), Some(Some(0)));
        assert_eq!(get(c2, "last_exit_code"), Some(Some(3)));
        assert_eq!(
            get(c3, "last_exit_code"),
            Some(Some(9)),
            "killed 必须保留上次值（DR-16②）"
        );
        // last_run_at 均更新
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
        // run_count 累计 3（内置 exited/failed/killed 均计）
        let rc: i64 = conn
            .query_row("SELECT run_count FROM projects WHERE id=?1", rusqlite::params![pid.to_string()], |r| r.get(0))
            .unwrap();
        assert_eq!(rc, 3);
    }

    #[test]
    fn external_launch_counts_and_keeps_exit_null() {
        let (mut conn, pid) = setup();
        let c = insert_command(&conn, pid, "open", "wt.exe").unwrap();
        // 预置历史退出码——外部模式不得写入新值（恒 NULL 语义指不产生新记录）
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
            .query_row("SELECT run_count FROM projects WHERE id=?1", rusqlite::params![pid.to_string()], |r| r.get(0))
            .unwrap();
        assert_eq!(rc, 1, "外部模式启动成功即累计 run_count（DR-16①）");
    }
}
