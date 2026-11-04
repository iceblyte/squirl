//! P3 工具集（DR-33/34/35）：Shell Doctor + AI 速查导出 + 端口雷达。

use serde::Serialize;

use crate::db::repo;
use crate::error::{AppError, Result};

// ---------- DR-33 Shell Doctor ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub cwd_ok: bool,
    /// 命令首词是否可解析（PATH 定位 / 已知内建 / shell 关键字）。
    pub command_visible: bool,
    /// 指纹命令的锁文件在位（非指纹命令恒 true）。
    pub lockfile_ok: bool,
    pub warnings: Vec<String>,
}

const SHELL_BUILTINS: &[&str] = &[
    "echo", "cd", "set", "dir", "type", "copy", "del", "md", "rd", "cls", "exit", "call",
    "for", "if", "goto", "setlocal", "endlocal", "pause", "rem", "start", "where", "npm",
    "pnpm", "yarn", "node", "cargo", "rustc", "python", "pip", "git", "dotnet", "go",
];

/// Shell Doctor（DR-33）：纯读预检，亚毫秒级（无进程启动）。
pub fn doctor_check(
    conn: &rusqlite::Connection,
    command_id: &str,
    project_root: &std::path::Path,
) -> Result<DoctorReport> {
    let cmd = repo::get_command(conn, command_id)?;
    let mut warnings = Vec::new();

    // ① cwd
    let cwd_ok = project_root.is_dir();
    if !cwd_ok {
        warnings.push(format!("工作目录不存在: {}", project_root.display()));
    }

    // ② 命令首词可解析
    let first_word = cmd
        .command_line
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();
    let command_visible = !first_word.is_empty()
        && (SHELL_BUILTINS.contains(&first_word.as_str())
            || crate::engine::shell::locate_on_path(&first_word).is_some()
            || project_root.join(format!("{first_word}.exe")).is_file()
            || project_root.join(format!("{first_word}.cmd")).is_file()
            || project_root.join(format!("{first_word}.ps1")).is_file()
            || project_root.join(&first_word).is_file());
    if !command_visible {
        warnings.push(format!(
            "命令 {first_word:?} 在 PATH 与项目根均未找到——运行可能失败"
        ));
    }

    // ③ 指纹命令锁文件
    let lockfile_ok = if cmd.fingerprint {
        let has_lock = crate::engine::fingerprint::detect_lockfile(project_root).is_some();
        if !has_lock {
            warnings.push("指纹命令未检测到锁文件——流水线将跳过初始化段".into());
        }
        has_lock
    } else {
        true
    };

    Ok(DoctorReport { cwd_ok, command_visible, lockfile_ok, warnings })
}

// ---------- DR-34 AI 速查导出 ----------

/// 导出台账 JSON（DR-15 导出契约子集）到数据目录 `ledger.json`。
pub fn export_ledger_json(db: &crate::db::Db) -> Result<std::path::PathBuf> {
    let conn = db.conn();
    let projects = repo::list_projects(&conn)?;
    let globals = repo::list_global_commands(&conn)?;
    let settings = repo::get_settings(&conn)?;

    let doc = serde_json::json!({
        "format": "squirl.ledger",
        "format_version": 1,
        "exported_at": crate::clock::now_rfc3339(),
        "settings": &settings,
        "projects": &projects,
        "global_commands": &globals,
    });
    let path = db
        .path()
        .parent()
        .ok_or_else(|| AppError::Engine("数据目录解析失败".into()))?
        .join("ledger.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&doc).map_err(|e| AppError::Engine(e.to_string()))?,
    )
    .map_err(|e| AppError::Path(format!("写 ledger.json 失败: {e}")))?;
    Ok(path)
}

// ---------- DR-35 端口雷达 ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningPort {
    pub port: u16,
    pub pid: u32,
}

/// 解析 `netstat -ano`（Windows）→ LISTENING 端口 + PID。
#[cfg(windows)]
pub fn list_listening_ports() -> Result<Vec<ListeningPort>> {
    use std::process::Command;
    let out = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|e| AppError::Engine(format!("netstat 启动失败: {e}")))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut out = Vec::new();
    for line in text.lines().skip(4) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Proto Local-Address Foreign-Address State PID
        if cols.len() == 5
            && cols[3].eq_ignore_ascii_case("LISTENING")
            && let Some(port) = cols[1].rsplit(':').next().and_then(|p| p.parse::<u16>().ok())
            && let Ok(pid) = cols[4].parse::<u32>()
        {
            out.push(ListeningPort { port, pid });
        }
    }
    Ok(out)
}

/// 解析 `ss -tlnp`（Linux）→ LISTENING 端口 + PID。
#[cfg(unix)]
pub fn list_listening_ports() -> Result<Vec<ListeningPort>> {
    use std::process::Command;
    let out = Command::new("ss")
        .args(["-tlnp"])
        .output()
        .map_err(|e| AppError::Engine(format!("ss 启动失败: {e}")))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut ports = Vec::new();
    for line in text.lines().skip(1) {
        // Local Address:Port 形如 *:3000 或 0.0.0.0:3000；pid=(pid=X)
        let port = line
            .split_whitespace()
            .nth(3)
            .and_then(|a| a.rsplit(':').next())
            .and_then(|p| p.parse::<u16>().ok());
        let pid = line
            .find("pid=")
            .and_then(|i| line[i + 4..].split(|c| c == ',' || c == ')').next())
            .and_then(|s| s.parse::<u32>().ok());
        if let (Some(port), Some(pid)) = (port, pid) {
            ports.push(ListeningPort { port, pid });
        }
    }
    Ok(ports)
}

/// 运行实例的监听端口（PID 树匹配——shell 本体 + 全部后裔由 OS 侧归入作业；
/// Windows 上 Job Object 内 PID 可通过 netstat 匹配 shell PID + 已知子 PID）。
pub fn run_ports(shell_pid: u32, child_pids: &[u32]) -> Result<Vec<ListeningPort>> {
    let all = list_listening_ports()?;
    let mut known = vec![shell_pid];
    known.extend_from_slice(child_pids);
    Ok(all.into_iter().filter(|p| known.contains(&p.pid)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_flags_missing_command() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrate::schema_migrations().to_latest(&mut conn).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let card = crate::db::repo::register_project(&conn, tmp.path(), None).unwrap();
        let cmd = crate::db::repo::add_command(
            &conn,
            Some(&card.id),
            "ghost",
            "surely-not-a-real-tool-xyz --flag",
            None,
            false,
            None,
        )
        .unwrap();
        let report = doctor_check(&conn, &cmd.id, tmp.path()).unwrap();
        assert!(!report.command_visible, "不存在的命令必须被标记");
        assert!(!report.warnings.is_empty());
    }

    #[test]
    fn doctor_passes_known_command() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrate::schema_migrations().to_latest(&mut conn).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let card = crate::db::repo::register_project(&conn, tmp.path(), None).unwrap();
        let cmd = crate::db::repo::add_command(
            &conn,
            Some(&card.id),
            "dev",
            "npm run dev",
            None,
            false,
            None,
        )
        .unwrap();
        let report = doctor_check(&conn, &cmd.id, tmp.path()).unwrap();
        assert!(report.command_visible, "npm 在 PATH 上");
    }

    #[cfg(windows)]
    #[test]
    fn netstat_parses_listening() {
        let ports = list_listening_ports().unwrap();
        // 系统上总有监听端口（如 135 RPC）
        assert!(!ports.is_empty(), "netstat 应解析出至少一个 LISTENING 端口");
        assert!(ports.iter().all(|p| p.port > 0 && p.pid > 0));
    }
}
