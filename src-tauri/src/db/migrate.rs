//! 迁移链（设计 §5.1.1 Schema v1 + DR-13）。
//!
//! `PRAGMA user_version` 对应需求的 `schema_version` 语义；
//! 每条迁移在事务内执行（rusqlite_migration），SQL 失败自动回滚。

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

use crate::error::{AppError, Result};

/// Schema v1 SQL（与设计 §5.1.1 逐字段一致；settings 单行 CHECK(id=1)，
/// roots/projects 以 path_normalized UNIQUE 防重复登记——附录 A #13）。
/// 常量为 C3 迁移失败演练提供可组合的基线。
pub const SCHEMA_V1_SQL: &str = r#"
        CREATE TABLE settings (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          default_mode TEXT NOT NULL DEFAULT 'external',
          default_emulator TEXT,
          default_shell TEXT,
          load_shell_profile INTEGER NOT NULL DEFAULT 1,
          scan_depth INTEGER NOT NULL DEFAULT 1 CHECK (scan_depth BETWEEN 1 AND 10)
        );
        INSERT INTO settings (id) VALUES (1);

        CREATE TABLE roots (
          id TEXT PRIMARY KEY,
          path TEXT NOT NULL,
          path_normalized TEXT NOT NULL UNIQUE,
          enabled INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE projects (
          id TEXT PRIMARY KEY,
          path TEXT NOT NULL,
          path_normalized TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          note TEXT NOT NULL DEFAULT '',
          exec_mode TEXT,
          exec_emulator TEXT,
          exec_shell TEXT,
          created_at TEXT NOT NULL,
          last_run_at TEXT,
          run_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE commands (
          id TEXT PRIMARY KEY,
          project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
          name TEXT NOT NULL,
          command_line TEXT NOT NULL,
          cwd TEXT,
          last_run_at TEXT,
          last_exit_code INTEGER
        );
        CREATE INDEX idx_commands_project ON commands(project_id);
        "#;

/// Schema v1 迁移链（兼容旧调用与测试）。
pub fn schema_v1_migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(SCHEMA_V1_SQL)])
}

/// Schema v2（P1 扩展，DR-19/20/21）：settings 增 tray_enabled / custom_excludes；
/// roots 增 scan_depth（NULL = 跟随全局）；commands 表重建使 project_id 可空
/// （全局命令，DR-20）。子表重建且存量行均有合法外键，FK 开启下安全。
pub const SCHEMA_V2_SQL: &str = r#"
ALTER TABLE settings ADD COLUMN tray_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN custom_excludes TEXT;
ALTER TABLE roots ADD COLUMN scan_depth INTEGER;
CREATE TABLE commands_v2 (
  id TEXT PRIMARY KEY,
  project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  command_line TEXT NOT NULL,
  cwd TEXT,
  last_run_at TEXT,
  last_exit_code INTEGER
);
INSERT INTO commands_v2 SELECT id, project_id, name, command_line, cwd, last_run_at, last_exit_code FROM commands;
DROP TABLE commands;
ALTER TABLE commands_v2 RENAME TO commands;
CREATE INDEX idx_commands_project ON commands(project_id);
"#;

/// Schema v3（P2-⑦ DR-24）：commands.fingerprint 开关 + fingerprints 指纹表。
pub const SCHEMA_V3_SQL: &str = r#"
ALTER TABLE commands ADD COLUMN fingerprint INTEGER NOT NULL DEFAULT 0;
CREATE TABLE fingerprints (
  command_id TEXT PRIMARY KEY REFERENCES commands(id) ON DELETE CASCADE,
  lock_hash TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
"#;

/// Schema v4（P2 扩展 DR-27/31）：commands.note 用途备忘 + settings.auto_watch 监听开关。
pub const SCHEMA_V4_SQL: &str = r#"
ALTER TABLE commands ADD COLUMN note TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN auto_watch INTEGER NOT NULL DEFAULT 0;
"#;

/// Schema v5（P3-⑳ DR-36）：commands.env_vars JSON 列（命令级环境变量）。
pub const SCHEMA_V5_SQL: &str = r#"
ALTER TABLE commands ADD COLUMN env_vars TEXT NOT NULL DEFAULT '[]';
"#;

/// 全量迁移链（生产打开入口使用）。
pub fn schema_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(SCHEMA_V1_SQL),
        M::up(SCHEMA_V2_SQL),
        M::up(SCHEMA_V3_SQL),
        M::up(SCHEMA_V4_SQL),
        M::up(SCHEMA_V5_SQL),
    ])
}

pub fn user_version(conn: &Connection) -> u32 {
    conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) as u32
}

/// 迁移后 sanity 检查（DR-13 语义错误防线）：
/// 四张表 + 索引在位、settings 默认行存在。
pub fn sanity_check(conn: &Connection) -> Result<()> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .map_err(AppError::Db)?;
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(AppError::Db)?
        .filter_map(|r| r.ok())
        .collect();
    for expected in ["settings", "roots", "projects", "commands"] {
        if !tables.iter().any(|t| t == expected) {
            return Err(AppError::MigrationBlocked(format!(
                "sanity 失败：缺少表 {expected}"
            )));
        }
    }
    let idx: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_commands_project'",
            [],
            |r| r.get(0),
        )
        .map_err(AppError::Db)?;
    if idx != 1 {
        return Err(AppError::MigrationBlocked("sanity 失败：缺少命令索引".into()));
    }
    let settings_rows: i64 = conn
        .query_row("SELECT count(*) FROM settings", [], |r| r.get(0))
        .map_err(AppError::Db)?;
    if settings_rows != 1 {
        return Err(AppError::MigrationBlocked(
            "sanity 失败：settings 单行表必须恰有 1 行".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_migration_and_sanity_pass() {
        let mut conn = Connection::open_in_memory().unwrap();
        schema_v1_migrations().to_latest(&mut conn).unwrap();
        assert_eq!(user_version(&conn), 1);
        sanity_check(&conn).unwrap();
    }

    #[test]
    fn sanity_detects_dropped_table() {
        let mut conn = Connection::open_in_memory().unwrap();
        schema_v1_migrations().to_latest(&mut conn).unwrap();
        conn.execute("DROP TABLE commands", []).unwrap();
        assert!(sanity_check(&conn).is_err());
    }
}
