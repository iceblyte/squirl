//! 存储层（设计 §5.1，DR-6）：单连接 `Arc<Mutex<Connection>>` + WAL。
//!
//! 打开流程（§5.1.2 流程图）：integrity_check → 失败则最近备份回填 →
//! 再校验（仍失败尝试更早备份，全部失败重建空库并提示）→
//! 存在待执行迁移则先强制 premigrate 快照（DR-13）→ 迁移 →
//! 迁移后校验失败回滚快照并阻断启动 → 成功则常规备份一次。

pub mod backup;
pub mod migrate;
pub mod repo;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{AppError, Result};

/// 启动恢复事件（契约 `app://db-restored`，AC9 提示 toast）。
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryEvent {
    None,
    RestoredFromBackup,
    RebuiltEmpty,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Db {
    /// 打开（含恢复与迁移）。阻塞调用，上层须在 spawn_blocking 中执行（DR-6）。
    pub fn open(path: &Path) -> Result<(Self, RecoveryEvent)> {
        Self::open_with_migrations(path, &migrate::schema_v1_migrations())
    }

    /// 测试钩子：注入自定义迁移链（C3 演练用）。
    pub fn open_with_migrations(
        path: &Path,
        migrations: &rusqlite_migration::Migrations<'static>,
    ) -> Result<(Self, RecoveryEvent)> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Path(format!("创建数据目录失败: {e}")))?;
        }
        let (mut conn, event) = open_with_recovery(path)?;

        // 迁移（存在待执行迁移时先强制 premigrate 快照，DR-13）
        if pending_migrations(&conn, migrations) > 0 {
            backup::snapshot_premigrate(&conn).map_err(|e| {
                AppError::Backup(format!("premigrate 快照失败，绝不带险迁移: {e}"))
            })?;
            if let Err(e) = migrations.to_latest(&mut conn) {
                // SQL 层失败：迁移事务自动回滚、user_version 不动（「迁一半」防线）。
                // 库文件保持迁移前状态，阻断启动。
                drop(conn);
                return Err(AppError::MigrationBlocked(format!("迁移执行失败: {e}")));
            }
            // 迁移后校验：integrity + 关键表 sanity
            let ok = integrity_ok(&conn) && migrate::sanity_check(&conn).is_ok();
            if !ok {
                // 语义错误防线：先关闭陈旧连接（其 WAL 状态会覆盖回填结果），
                // 再用 premigrate 快照回滚 schema（数据无损），阻断启动（DR-13）。
                drop(conn);
                if let Some(snap) = backup::latest_valid_snapshot(
                    path,
                    backup::PREMIGRATE_SUFFIX,
                    backup::PREMIGRATE_BUCKETS,
                ) {
                    backup::restore_from(path, &snap)?;
                }
                return Err(AppError::MigrationBlocked(
                    "迁移后校验失败，已回滚至迁移前快照；数据无损，可用配套旧版本应用打开或反馈修复"
                        .into(),
                ));
            }
            // 迁移成功 → 常规备份一次
            backup::snapshot_regular(&conn)?;
        }

        Ok((
            Db { conn: Arc::new(Mutex::new(conn)), path: path.to_path_buf() },
            event,
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 连接句柄（单连接互斥，DR-6；耗时调用放 spawn_blocking）。
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}

/// 待执行迁移数（rusqlite_migration 2.6 API）。
fn pending_migrations(conn: &Connection, migrations: &rusqlite_migration::Migrations<'static>) -> i32 {
    migrations.pending_migrations(conn).unwrap_or(0)
}

/// 打开 + 损坏恢复（§5.1.2）：打开失败或 integrity_check 失败 →
/// 依次尝试常规备份回填（bak-1 最新）→ 全部失败重建空库。
/// 回填动作在无活动连接时执行（陈旧连接的 WAL 状态会覆盖回填结果）。
fn open_with_recovery(path: &Path) -> Result<(Connection, RecoveryEvent)> {
    match open_raw(path) {
        Ok(conn) if integrity_ok(&conn) => Ok((conn, RecoveryEvent::None)),
        Ok(conn) => {
            drop(conn);
            recover(path)
        }
        Err(_) => recover(path),
    }
}

fn recover(path: &Path) -> Result<(Connection, RecoveryEvent)> {
    if let Some(snap) =
        backup::latest_valid_snapshot(path, backup::REGULAR_SUFFIX, backup::REGULAR_BUCKETS)
    {
        backup::restore_from(path, &snap)?;
        if let Ok(conn) = open_raw(path) {
            if integrity_ok(&conn) {
                return Ok((conn, RecoveryEvent::RestoredFromBackup));
            }
            drop(conn);
        }
    }
    // 全部备份不可用：重建空库（AC9 rebuilt_empty，前端提示走
    // app://db-restored { kind: 'rebuilt_empty' }）
    rebuild_empty(path);
    let conn = open_raw(path)?;
    Ok((conn, RecoveryEvent::RebuiltEmpty))
}

fn open_raw(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).map_err(AppError::Db)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(AppError::Db)?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(AppError::Db)?;
    conn.busy_timeout(std::time::Duration::from_millis(2000))
        .map_err(AppError::Db)?;
    Ok(conn)
}

pub fn integrity_ok(conn: &Connection) -> bool {
    conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
        .map(|s| s == "ok")
        .unwrap_or(false)
}

/// 重建空库：删除主库与 WAL 残留（AC9 的 rebuilt_empty）。
fn rebuild_empty(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(wal_path(path));
    let _ = std::fs::remove_file(shm_path(path));
}

pub fn wal_path(db: &Path) -> PathBuf {
    append_ext(db, "-wal")
}

pub fn shm_path(db: &Path) -> PathBuf {
    append_ext(db, "-shm")
}

fn append_ext(db: &Path, suffix: &str) -> PathBuf {
    let mut s = db.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

/// 退出四步串行（DR-14）：
/// ① 回收全部内置 run（`reap` 由上层注入）→ ② 终态写回（DR-16）→
/// ③ VACUUM INTO 常规备份（失败仅记日志、不阻断退出）→ ④ 放行退出。
/// 单连接 Mutex 互斥 + WAL 快照读保证「备份包含最终写回」。
pub fn shutdown_sequence(
    db: &Db,
    finals: &[repo::RunFinal],
    reap: impl FnOnce(),
) -> Result<()> {
    reap(); // ①
    repo::write_back_finals(&mut db.conn(), finals)?; // ②
    if let Err(e) = backup::snapshot_regular(&db.conn()) {
        // ③ 失败不阻断退出（设计 §5.1.2）
        tracing::warn!(target: "db_record", error = %e, "退出备份失败（不阻断退出）");
    }
    Ok(()) // ④
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("squirl-c-{label}-{}", uuid::Uuid::now_v7().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("squirl.db")
    }

    #[test]
    fn open_creates_schema_and_defaults() {
        let path = temp_db("create");
        let (db, event) = Db::open(&path).unwrap();
        assert_eq!(event, RecoveryEvent::None);
        let conn = db.conn();
        // settings 单行默认
        let mode: String = conn
            .query_row("SELECT default_mode FROM settings WHERE id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "external");
        // user_version 到位
        assert_eq!(migrate::user_version(&conn), 1);
        // WAL 生效
        let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }
}
