//! roots 表仓储（F1：添加/删除/启用禁用）。

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::error::{AppError, Result};

/// `Root`（契约 §5.4.1：list_roots / add_root 出参）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub id: String,
    pub path: String,
    pub enabled: bool,
}

pub fn list_roots(conn: &Connection) -> Result<Vec<Root>> {
    let mut stmt = conn
        .prepare("SELECT id, path, enabled FROM roots ORDER BY rowid")
        .map_err(AppError::Db)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Root {
                id: r.get(0)?,
                path: r.get(1)?,
                enabled: r.get::<_, i64>(2)? != 0,
            })
        })
        .map_err(AppError::Db)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(AppError::Db)
}

/// 添加根目录：规范化（INV-4）+ 唯一校验（E_DUPLICATE_PATH）。
pub fn add_root(conn: &Connection, path: &Path) -> Result<Root> {
    let display = crate::paths::canonical(path)?;
    let norm = crate::paths::normalize_key(path)?;
    let exists: i64 = conn
        .query_row(
            "SELECT count(*) FROM roots WHERE path_normalized=?1",
            rusqlite::params![norm],
            |r| r.get(0),
        )
        .map_err(AppError::Db)?;
    if exists > 0 {
        return Err(AppError::DuplicatePath(display.to_string_lossy().into_owned()));
    }
    let id = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO roots (id, path, path_normalized, enabled) VALUES (?1,?2,?3,1)",
        rusqlite::params![id, display.to_string_lossy(), norm],
    )
    .map_err(AppError::Db)?;
    Ok(Root { id, path: display.to_string_lossy().into_owned(), enabled: true })
}

/// 删根：条目保留（无归属，附录 A #21），仅根记录移除。
pub fn remove_root(conn: &Connection, id: &str) -> Result<()> {
    let n = conn
        .execute("DELETE FROM roots WHERE id=?1", rusqlite::params![id])
        .map_err(AppError::Db)?;
    if n == 0 {
        return Err(AppError::InvalidInput(format!("根目录不存在: {id}")));
    }
    Ok(())
}

pub fn set_root_enabled(conn: &Connection, id: &str, enabled: bool) -> Result<()> {
    let n = conn
        .execute(
            "UPDATE roots SET enabled=?1 WHERE id=?2",
            rusqlite::params![enabled as i64, id],
        )
        .map_err(AppError::Db)?;
    if n == 0 {
        return Err(AppError::InvalidInput(format!("根目录不存在: {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db::migrate::schema_v1_migrations()
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    #[test]
    fn add_list_remove_roundtrip() {
        let conn = setup();
        let tmp = tempfile::tempdir().unwrap();
        let root = add_root(&conn, tmp.path()).unwrap();
        assert!(root.enabled);
        assert_eq!(list_roots(&conn).unwrap().len(), 1);

        // 大小写不同路径 = 重复（Windows 键折叠）
        let upper = tmp.path().to_string_lossy().to_uppercase();
        let dup = add_root(&conn, Path::new(&upper));
        assert!(
            matches!(dup, Err(AppError::DuplicatePath(_))),
            "path_normalized UNIQUE 必须拦截重复"
        );

        set_root_enabled(&conn, &root.id, false).unwrap();
        assert!(!list_roots(&conn).unwrap()[0].enabled);

        remove_root(&conn, &root.id).unwrap();
        assert!(list_roots(&conn).unwrap().is_empty());
        assert!(remove_root(&conn, &root.id).is_err(), "重复删除报错");
    }
}
