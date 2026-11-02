//! projects 表仓储（登记 / 查询 / 更新 / 删除）。

use std::path::Path;

use rusqlite::Connection;
use uuid::Uuid;

use crate::error::{AppError, Result};

/// `ProjectCard`（契约 §5.4.1：list_projects / search_projects 出参）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCard {
    pub id: String,
    pub path: String,
    pub name: String,
    pub note: String,
    /// 归属根 id（最长根前缀派生，不落库，附录 A #21）；None = 无归属。
    pub root_id: Option<String>,
    pub path_missing: bool,
    pub last_run_at: Option<String>,
    pub run_count: i64,
}

/// 登记项目（发现区「登记」与手动注册同入口，契约 register_project）。
/// 规范化（INV-4）+ 唯一校验（E_DUPLICATE_PATH）；name 默认取文件夹名。
pub fn register_project(conn: &Connection, path: &Path, note: Option<&str>) -> Result<ProjectCard> {
    if !path.exists() {
        return Err(AppError::PathNotFound(path.to_string_lossy().into_owned()));
    }
    let display = crate::paths::canonical(path)?;
    let norm = crate::paths::normalize_key(path)?;
    let dup: i64 = conn
        .query_row(
            "SELECT count(*) FROM projects WHERE path_normalized=?1",
            rusqlite::params![norm],
            |r| r.get(0),
        )
        .map_err(AppError::Db)?;
    if dup > 0 {
        return Err(AppError::DuplicatePath(display.to_string_lossy().into_owned()));
    }
    let id = Uuid::now_v7().to_string();
    let name = display
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| display.to_string_lossy().into_owned());
    conn.execute(
        "INSERT INTO projects (id, path, path_normalized, name, note, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![
            id,
            display.to_string_lossy(),
            norm,
            name,
            note.unwrap_or(""),
            crate::clock::now_rfc3339()
        ],
    )
    .map_err(AppError::Db)?;
    Ok(ProjectCard {
        id,
        path: display.to_string_lossy().into_owned(),
        name,
        note: note.unwrap_or("").to_string(),
        root_id: attribution_for(&norm, conn),
        path_missing: false,
        last_run_at: None,
        run_count: 0,
    })
}

/// 归属派生（§5.1.4）：所有启用根前缀中取最长者（分隔符边界匹配）。
fn attribution_for(project_norm: &str, conn: &Connection) -> Option<String> {
    let mut stmt = conn
        .prepare("SELECT id, path_normalized FROM roots WHERE enabled=1")
        .ok()?;
    // attribution 期望 (path_normalized, root_id)——注意与 SELECT 列序解耦
    let roots: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(0)?)))
        .ok()?
        .filter_map(std::result::Result::ok)
        .collect();
    crate::paths::attribution(project_norm, &roots).map(str::to_string)
}

/// 台账列表（含归属、失效标记；每次调用内存派生，≤500 项目 × ≤10 根）。
pub fn list_projects(conn: &Connection) -> Result<Vec<ProjectCard>> {
    let mut stmt = conn
        .prepare("SELECT id, path, name, note, last_run_at, run_count FROM projects ORDER BY last_run_at IS NULL, last_run_at DESC")
        .map_err(AppError::Db)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })
        .map_err(AppError::Db)?;
    let mut out = Vec::new();
    for row in rows {
        let (id, path, name, note, last_run_at, run_count) = row.map_err(AppError::Db)?;
        let norm = crate::paths::normalize_key(Path::new(&path))?;
        out.push(ProjectCard {
            root_id: attribution_for(&norm, conn),
            path_missing: !Path::new(&path).exists(),
            id,
            path,
            name,
            note,
            last_run_at,
            run_count,
        });
    }
    Ok(out)
}

/// 全局搜索（契约 search_projects）：name/path/note/命令名 四字段 LIKE。
pub fn search_projects(conn: &Connection, query: &str) -> Result<Vec<ProjectCard>> {
    let like = format!("%{}%", query.to_lowercase());
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT p.id, p.path, p.name, p.note, p.last_run_at, p.run_count
             FROM projects p
             LEFT JOIN commands c ON c.project_id = p.id
             WHERE lower(p.name) LIKE ?1 OR lower(p.path) LIKE ?1
                OR lower(p.note) LIKE ?1 OR lower(c.name) LIKE ?1
             ORDER BY p.last_run_at IS NULL, p.last_run_at DESC",
        )
        .map_err(AppError::Db)?;
    let rows = stmt
        .query_map(rusqlite::params![like], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })
        .map_err(AppError::Db)?;
    let mut out = Vec::new();
    for row in rows {
        let (id, path, name, note, last_run_at, run_count) = row.map_err(AppError::Db)?;
        let norm = crate::paths::normalize_key(Path::new(&path))?;
        out.push(ProjectCard {
            root_id: attribution_for(&norm, conn),
            path_missing: !Path::new(&path).exists(),
            id,
            path,
            name,
            note,
            last_run_at,
            run_count,
        });
    }
    Ok(out)
}

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

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db::migrate::schema_v1_migrations()
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    #[test]
    fn register_and_list_with_search() {
        let conn = setup();
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join("demo");
        std::fs::create_dir_all(&proj).unwrap();
        let card = register_project(&conn, &proj, Some("测试项目")).unwrap();
        assert_eq!(card.name, "demo");
        assert_eq!(card.note, "测试项目");
        assert!(!card.path_missing);
        assert_eq!(list_projects(&conn).unwrap().len(), 1);

        let dup = register_project(&conn, &proj, None);
        assert!(matches!(dup, Err(AppError::DuplicatePath(_))));
        let upper = proj.to_string_lossy().to_uppercase();
        let dup2 = register_project(&conn, std::path::Path::new(&upper), None);
        assert!(matches!(dup2, Err(AppError::DuplicatePath(_))));

        insert_command(&conn, Uuid::parse_str(&card.id).unwrap(), "dev", "npm run dev").unwrap();
        assert_eq!(search_projects(&conn, "dev").unwrap().len(), 1, "命令名命中");
        assert_eq!(search_projects(&conn, "测试").unwrap().len(), 1, "备注命中");
        assert_eq!(search_projects(&conn, "xxx-nothing").unwrap().len(), 0);
    }

    #[test]
    fn path_missing_flag_and_no_attribution() {
        let conn = setup();
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join("gone");
        std::fs::create_dir_all(&proj).unwrap();
        register_project(&conn, &proj, None).unwrap();
        std::fs::remove_dir_all(&proj).unwrap();
        let cards = list_projects(&conn).unwrap();
        assert!(cards[0].path_missing, "失效标记");
        assert_eq!(cards[0].root_id, None, "无启用根 → 无归属");
    }

    #[test]
    fn attribution_longest_root_wins() {
        let conn = setup();
        let tmp = tempfile::tempdir().unwrap();
        let r1 = add_root_for_test(&conn, tmp.path());
        let deep = tmp.path().join("sub");
        std::fs::create_dir_all(&deep).unwrap();
        let r2 = add_root_for_test(&conn, &deep);
        let proj = deep.join("app");
        std::fs::create_dir_all(&proj).unwrap();
        let card = register_project(&conn, &proj, None).unwrap();
        assert_eq!(card.root_id.as_deref(), Some(r2.id.as_str()), "嵌套根归更深者");
        let _ = r1;
    }

    fn add_root_for_test(conn: &Connection, path: &std::path::Path) -> crate::db::repo::roots::Root {
        crate::db::repo::roots::add_root(conn, path).unwrap()
    }
}

