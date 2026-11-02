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
    /// ADR-12（契约 v1.1a additive）：卡片快启（G3）数据源。
    #[serde(default)]
    pub commands: Vec<CommandRow>,
}

/// 登记项目（发现区「登记」与手动注册同入口，契约 register_project）。
/// 规范化（INV-4）+ 唯一校验（E_DUPLICATE_PATH）；name 默认取文件夹名。
pub fn register_project(conn: &Connection, path: &Path, note: Option<&str>) -> Result<ProjectCard> {
    if !path.exists() {
        return Err(AppError::PathNotFound(path.to_string_lossy().into_owned()));
    }
    if !path.is_dir() {
        return Err(AppError::InvalidInput("仅支持登记文件夹".into()));
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
        commands: Vec::new(),
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
/// 全量命令一次取回按项目分组（卡片快启 G3 数据源，ADR-12）。
fn fetch_all_commands_grouped(
    conn: &Connection,
) -> Result<std::collections::HashMap<String, Vec<CommandRow>>> {
    let mut stmt = conn
        .prepare("SELECT project_id, id, name, command_line, cwd, last_run_at, last_exit_code FROM commands")
        .map_err(AppError::Db)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                CommandRow {
                    id: r.get(1)?,
                    project_id: r.get(0)?,
                    name: r.get(2)?,
                    command_line: r.get(3)?,
                    cwd: r.get(4)?,
                    last_run_at: r.get(5)?,
                    last_exit_code: r.get(6)?,
                },
            ))
        })
        .map_err(AppError::Db)?;
    let mut map: std::collections::HashMap<String, Vec<CommandRow>> =
        std::collections::HashMap::new();
    for row in rows {
        let (pid, cmd) = row.map_err(AppError::Db)?;
        map.entry(pid).or_default().push(cmd);
    }
    Ok(map)
}

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
    let mut cmds = fetch_all_commands_grouped(conn)?;

    let mut out = Vec::new();
    for row in rows {
        let (id, path, name, note, last_run_at, run_count) = row.map_err(AppError::Db)?;
        let norm = crate::paths::normalize_key(Path::new(&path))?;
        out.push(ProjectCard {
            root_id: attribution_for(&norm, conn),
            path_missing: !Path::new(&path).exists(),
            commands: cmds.remove(&id).unwrap_or_default(),
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
    let mut cmds = fetch_all_commands_grouped(conn)?;
    let mut out = Vec::new();
    for row in rows {
        let (id, path, name, note, last_run_at, run_count) = row.map_err(AppError::Db)?;
        let norm = crate::paths::normalize_key(Path::new(&path))?;
        out.push(ProjectCard {
            root_id: attribution_for(&norm, conn),
            path_missing: !Path::new(&path).exists(),
            commands: cmds.remove(&id).unwrap_or_default(),
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

/// `CommandRow`（get_project 出参内嵌；S4 扩展 CRUD）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRow {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub command_line: String,
    pub cwd: Option<String>,
    pub last_run_at: Option<String>,
    pub last_exit_code: Option<i32>,
}

impl CommandRow {
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
}

/// `ProjectDetail`（契约 get_project 出参：卡片 + 命令列表）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub card: ProjectCard,
    pub commands: Vec<CommandRow>,
}

pub fn get_project(conn: &Connection, id: &str) -> Result<ProjectDetail> {
    let cards = list_projects(conn)?;
    let card = cards
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::InvalidInput(format!("项目不存在: {id}")))?;
    let mut stmt = conn
        .prepare("SELECT id, project_id, name, command_line, cwd, last_run_at, last_exit_code FROM commands WHERE project_id=?1 ORDER BY rowid")
        .map_err(AppError::Db)?;
    let commands = stmt
        .query_map(rusqlite::params![id], |r| {
            Ok(CommandRow {
                id: r.get(0)?,
                project_id: r.get(1)?,
                name: r.get(2)?,
                command_line: r.get(3)?,
                cwd: r.get(4)?,
                last_run_at: r.get(5)?,
                last_exit_code: r.get(6)?,
            })
        })
        .map_err(AppError::Db)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(AppError::Db)?;
    Ok(ProjectDetail { card, commands })
}

/// 更新备注（契约 update_project 的 note 半；execution_override 归 S4/S5）。
pub fn update_note(conn: &Connection, id: &str, note: &str) -> Result<ProjectCard> {
    let n = conn
        .execute(
            "UPDATE projects SET note=?1 WHERE id=?2",
            rusqlite::params![note, id],
        )
        .map_err(AppError::Db)?;
    if n == 0 {
        return Err(AppError::InvalidInput(format!("项目不存在: {id}")));
    }
    list_projects(conn)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::InvalidInput(format!("项目不存在: {id}")))
}

/// 重新定位（G2）：规范化 + 唯一校验；name 默认取新文件夹名、可手改。
pub fn relocate_project(conn: &Connection, id: &str, new_path: &Path) -> Result<ProjectCard> {
    if !new_path.exists() {
        return Err(AppError::PathNotFound(new_path.to_string_lossy().into_owned()));
    }
    let display = crate::paths::canonical(new_path)?;
    let norm = crate::paths::normalize_key(new_path)?;
    let dup: i64 = conn
        .query_row(
            "SELECT count(*) FROM projects WHERE path_normalized=?1 AND id != ?2",
            rusqlite::params![norm, id],
            |r| r.get(0),
        )
        .map_err(AppError::Db)?;
    if dup > 0 {
        return Err(AppError::DuplicatePath(display.to_string_lossy().into_owned()));
    }
    let name = display
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| display.to_string_lossy().into_owned());
    conn.execute(
        "UPDATE projects SET path=?1, path_normalized=?2, name=?3 WHERE id=?4",
        rusqlite::params![display.to_string_lossy(), norm, name, id],
    )
    .map_err(AppError::Db)?;
    list_projects(conn)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::InvalidInput(format!("项目不存在: {id}")))
}

/// 删除项目（G1）：级联删除命令（FK ON DELETE CASCADE + foreign_keys=ON）。
/// TODO(S6)：任一命令内置模式运行中 → E_PROJECT_RUNNING_NO_DELETE（run 注册表在 S6 落地）。
pub fn delete_project(conn: &Connection, id: &str) -> Result<()> {
    let n = conn
        .execute("DELETE FROM projects WHERE id=?1", rusqlite::params![id])
        .map_err(AppError::Db)?;
    if n == 0 {
        return Err(AppError::InvalidInput(format!("项目不存在: {id}")));
    }
    Ok(())
}

/// 项目的命令数（删除确认框「将同时删除 N 条命令」）。
pub fn command_count(conn: &Connection, id: &str) -> Result<i64> {
    conn.query_row(
        "SELECT count(*) FROM commands WHERE project_id=?1",
        rusqlite::params![id],
        |r| r.get(0),
    )
    .map_err(AppError::Db)
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

