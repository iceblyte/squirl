//! 双桶 VACUUM INTO 快照与恢复（设计 §5.1.2，DR-13）。
//!
//! - 常规桶 `squirl.db.bak-1..3`（bak-1 最新）：迁移成功后 + 正常退出时。
//! - 迁移前桶 `squirl.db.premigrate-1..2`：仅存在待执行迁移时、迁移动作之前。
//!   两桶独立轮换，迁移前快照不会被日常备份淘汰。
//! - 快照命令：`VACUUM INTO`（一致性快照，禁止直接复制 db 文件——附录 A #12）。
//! - 恢复 = 用通过 integrity_check 的最新快照覆盖主库（快照本身即一致点）。

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{AppError, Result};

pub const REGULAR_SUFFIX: &str = ".bak-";
pub const REGULAR_BUCKETS: u32 = 3;
pub const PREMIGRATE_SUFFIX: &str = ".premigrate-";
pub const PREMIGRATE_BUCKETS: u32 = 2;

/// 对 `db_path` 的 VACUUM INTO 快照落 `target`（target 必须不存在）。
pub fn vacuum_into(conn: &Connection, target: &Path) -> Result<()> {
    if target.exists() {
        return Err(AppError::Backup(format!("快照目标已存在: {}", target.display())));
    }
    conn.execute(
        "VACUUM INTO ?1",
        rusqlite::params![target.to_string_lossy()],
    )
    .map_err(|e| AppError::Backup(format!("VACUUM INTO 失败: {e}")))?;
    Ok(())
}

/// 滚动轮换 + 快照。
fn rotate_and_snapshot(
    conn: &Connection,
    db_path: &Path,
    prefix: &str,
    buckets: u32,
) -> Result<()> {
    // bak-2→bak-3, bak-1→bak-2（最旧被挤出删除）
    for i in (1..buckets).rev() {
        let from = bucket_path(db_path, prefix, i);
        let to = bucket_path(db_path, prefix, i + 1);
        if from.exists() {
            if to.exists() {
                std::fs::remove_file(&to)
                    .map_err(|e| AppError::Backup(format!("删除旧快照失败: {e}")))?;
            }
            std::fs::rename(&from, &to)
                .map_err(|e| AppError::Backup(format!("轮换快照失败: {e}")))?;
        }
    }
    vacuum_into(conn, &bucket_path(db_path, prefix, 1))
}

pub fn snapshot_regular(conn: &Connection) -> Result<()> {
    let db_path = main_db_path_from(conn)?;
    rotate_and_snapshot(conn, &db_path, REGULAR_SUFFIX, REGULAR_BUCKETS)
}

pub fn snapshot_premigrate(conn: &Connection) -> Result<()> {
    let db_path = main_db_path_from(conn)?;
    rotate_and_snapshot(conn, &db_path, PREMIGRATE_SUFFIX, PREMIGRATE_BUCKETS)
}

fn main_db_path_from(conn: &Connection) -> Result<PathBuf> {
    let p: String = conn
        .query_row("PRAGMA database_list", [], |r| r.get::<_, String>(2))
        .map_err(|e| AppError::Backup(format!("读取主库路径失败: {e}")))?;
    Ok(PathBuf::from(p))
}

fn bucket_path(db_path: &Path, suffix: &str, i: u32) -> PathBuf {
    // 与主库同文件名 + 后缀：squirl.db → squirl.db.bak-1
    let file = db_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    db_path.with_file_name(format!("{file}{suffix}{i}"))
}

/// 最新的、能独立打开且通过 integrity_check 的快照；无则 None。
pub fn latest_valid_snapshot(db_path: &Path, prefix: &str, buckets: u32) -> Option<PathBuf> {
    (1..=buckets).find_map(|i| {
        let p = bucket_path(db_path, prefix, i);
        if p.exists() && snapshot_integrity_ok(&p) {
            Some(p)
        } else {
            None
        }
    })
}

fn snapshot_integrity_ok(path: &Path) -> bool {
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .and_then(|c| {
        c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
    })
    .map(|s| s == "ok")
    .unwrap_or(false)
}

/// 用快照回填主库（复制 + 清 WAL/SHM 残留，避免旧 WAL 混入）。
pub fn restore_from(db_path: &Path, snapshot: &Path) -> Result<()> {
    std::fs::copy(snapshot, db_path)
        .map_err(|e| AppError::Backup(format!("回填快照失败: {e}")))?;
    let _ = std::fs::remove_file(crate::db::wal_path(db_path));
    let _ = std::fs::remove_file(crate::db::shm_path(db_path));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db(dir: &Path) -> PathBuf {
        let db = dir.join("squirl.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('hello');",
        )
        .unwrap();
        db
    }

    #[test]
    fn regular_bucket_rolls_and_keeps_latest() {
        let dir = tempfile::tempdir().unwrap();
        let db = setup_db(dir.path());
        let conn = Connection::open(&db).unwrap();

        for round in 1..=5u32 {
            conn.execute("INSERT INTO t VALUES (?)", rusqlite::params![format!("r{round}")])
                .unwrap();
            snapshot_regular(&conn).unwrap();
        }
        // 滚动 3 份
        assert!(bucket_path(&db, REGULAR_SUFFIX, 1).exists());
        assert!(bucket_path(&db, REGULAR_SUFFIX, 2).exists());
        assert!(bucket_path(&db, REGULAR_SUFFIX, 3).exists());
        assert!(!bucket_path(&db, REGULAR_SUFFIX, 4).exists());
        // 最新快照可独立打开且内容正确
        let snap = bucket_path(&db, REGULAR_SUFFIX, 1);
        let c = Connection::open_with_flags(&snap, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
        let n: i64 = c.query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 6);
        assert!(snapshot_integrity_ok(&snap));
    }

    #[test]
    fn premigrate_bucket_separate_from_regular() {
        let dir = tempfile::tempdir().unwrap();
        let db = setup_db(dir.path());
        let conn = Connection::open(&db).unwrap();
        snapshot_premigrate(&conn).unwrap();
        assert!(bucket_path(&db, PREMIGRATE_SUFFIX, 1).exists());
        assert!(!bucket_path(&db, REGULAR_SUFFIX, 1).exists(), "两桶必须独立");
    }

    #[test]
    fn latest_valid_snapshot_skips_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let db = setup_db(dir.path());
        let conn = Connection::open(&db).unwrap();
        snapshot_regular(&conn).unwrap(); // bak-1（好）
        conn.execute("INSERT INTO t VALUES ('x2')", []).unwrap();
        snapshot_regular(&conn).unwrap(); // bak-1 新、bak-1 旧→bak-2
        // 注入损坏到最新
        let newest = bucket_path(&db, REGULAR_SUFFIX, 1);
        std::fs::write(&newest, b"corrupted garbage header").unwrap();
        let got = latest_valid_snapshot(&db, REGULAR_SUFFIX, REGULAR_BUCKETS).unwrap();
        assert_eq!(got, bucket_path(&db, REGULAR_SUFFIX, 2), "跳过损坏，取次新");
    }

    #[test]
    fn restore_recovers_data_and_clears_wal() {
        let dir = tempfile::tempdir().unwrap();
        let db = setup_db(dir.path());
        let conn = Connection::open(&db).unwrap();
        snapshot_regular(&conn).unwrap();
        let snap = bucket_path(&db, REGULAR_SUFFIX, 1);
        // 破坏主库
        drop(conn);
        std::fs::write(&db, b"broken").unwrap();
        std::fs::write(crate::db::wal_path(&db), b"stale wal").unwrap();
        restore_from(&db, &snap).unwrap();
        assert!(!crate::db::wal_path(&db).exists(), "WAL 残留必须清除");
        let c = Connection::open(&db).unwrap();
        let v: String = c.query_row("SELECT v FROM t LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "hello");
    }
}
