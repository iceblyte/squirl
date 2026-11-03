//! 锁文件指纹感知流水线（P2-⑦，DR-24）：三段式「环境注入 → 幂等初始化 → 主命令」。
//!
//! 组合式实现（DR-24 核心决策）：不新增执行路径——流水线在适配器之前把
//! 三段组装为**一条新命令行原文**（launch record 记录最终形态），随后照常
//! 走内置/外部启动。INV-6 不破坏：组装产物本身就是新的「命令原文事实」。
//!
//! - ① 环境注入：检测项目内 `.venv`，按 shell 方言生成激活前缀。
//! - ② 幂等初始化：锁文件（package-lock/pnpm-lock/yarn.lock/Cargo.lock/
//!   poetry.lock/uv.lock）SHA-256 指纹存 `fingerprints` 表；指纹未变且
//!   构建产物目录存在 → 跳过；否则生成 `init && ` 前缀（失败即不跑主命令）。
//! - ③ 主命令原文直传。
//!
//! 组装失败（无锁文件且无 .venv 等）→ 降级为原文直跑，绝不阻断。

use std::path::Path;

use rusqlite::Connection;

use rusqlite;
use sha2::{Digest, Sha256};

use crate::engine::shell::ShellKind;
use crate::error::{AppError, Result};

/// 支持的锁文件 →（初始化命令，构建产物目录）。
const LOCKFILES: &[(&str, &str, &str)] = &[
    ("package-lock.json", "npm install", "node_modules"),
    ("pnpm-lock.yaml", "pnpm install", "node_modules"),
    ("yarn.lock", "yarn install", "node_modules"),
    ("Cargo.lock", "cargo build", "target"),
    ("poetry.lock", "poetry install", ".venv"),
    ("uv.lock", "uv sync", ".venv"),
];

pub struct LockInfo {
    pub file: &'static str,
    pub init_command: &'static str,
    pub artifact_dir: &'static str,
    pub hash: String,
}

/// 探测项目根的锁文件并计算 SHA-256（首个命中）。
pub fn detect_lockfile(project_root: &Path) -> Option<LockInfo> {
    for (file, init, artifact) in LOCKFILES {
        let p = project_root.join(file);
        if p.is_file() {
            let bytes = std::fs::read(&p).ok()?;
            let hash = format!("{:x}", Sha256::digest(&bytes));
            return Some(LockInfo {
                file,
                init_command: init,
                artifact_dir: artifact,
                hash,
            });
        }
    }
    None
}

/// `.venv` 激活前缀（按 shell 方言；无 .venv → None）。
pub fn venv_activation_prefix(project_root: &Path, kind: ShellKind) -> Option<String> {
    if !project_root.join(".venv").is_dir() {
        return None;
    }
    match kind {
        ShellKind::Pwsh => {
            if project_root.join(".venv/Scripts/Activate.ps1").is_file() {
                Some(". .venv\\Scripts\\Activate.ps1; ".into())
            } else {
                None
            }
        }
        ShellKind::Cmd => {
            if project_root.join(".venv/Scripts/activate.bat").is_file() {
                Some("call .venv\\Scripts\\activate.bat && ".into())
            } else {
                None
            }
        }
        ShellKind::Bash | ShellKind::Zsh => {
            let activate = if cfg!(windows) {
                project_root.join(".venv/Scripts/activate")
            } else {
                project_root.join(".venv/bin/activate")
            };
            if activate.is_file() {
                Some("source .venv/Scripts/activate 2>/dev/null || source .venv/bin/activate; ".into())
            } else {
                None
            }
        }
        ShellKind::PowerShell5 => {
            if project_root.join(".venv/Scripts/Activate.ps1").is_file() {
                Some(". .venv\\Scripts\\Activate.ps1; ".into())
            } else {
                None
            }
        }
    }
}

/// 指纹是否需要初始化（锁文件哈希 != 存储值，或产物目录缺失）。
pub fn init_needed(conn: &Connection, command_id: &str, lock: &LockInfo, project_root: &Path) -> bool {
    if !project_root.join(lock.artifact_dir).is_dir() {
        return true;
    }
    let stored: Option<String> = conn
        .query_row(
            "SELECT lock_hash FROM fingerprints WHERE command_id=?1",
            rusqlite::params![command_id],
            |r| r.get(0),
        )
        .unwrap_or(None);
    stored.as_deref() != Some(lock.hash.as_str())
}

/// 组装流水线命令行（三段；无注入点/无需初始化时逐段退化）。
pub fn compose_pipeline(
    conn: &Connection,
    command_id: &str,
    command_line: &str,
    project_root: &Path,
    kind: ShellKind,
) -> Result<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(prefix) = venv_activation_prefix(project_root, kind) {
        parts.push(prefix);
    }
    if let Some(lock) = detect_lockfile(project_root)
        && init_needed(conn, command_id, &lock, project_root)
    {
        parts.push(format!("{} && ", lock.init_command));
    }
    parts.push(command_line.to_string());
    Ok(parts.concat())
}

/// 指纹更新（内置 exited(0) 后；外部启动 best-effort 直接记当前哈希）。
pub fn record_fingerprint(conn: &Connection, command_id: &str, project_root: &Path) -> Result<()> {
    let Some(lock) = detect_lockfile(project_root) else {
        return Ok(());
    };
    conn.execute(
        "INSERT INTO fingerprints (command_id, lock_hash, updated_at) VALUES (?1,?2,?3)
         ON CONFLICT(command_id) DO UPDATE SET lock_hash=?2, updated_at=?3",
        rusqlite::params![command_id, lock.hash, crate::clock::now_rfc3339()],
    )
    .map_err(AppError::Db)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(kind: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(kind), "{}").unwrap();
        tmp
    }

    #[test]
    fn detects_lockfile_and_hash_stable() {
        let tmp = fixture("package-lock.json");
        let a = detect_lockfile(tmp.path()).unwrap();
        let b = detect_lockfile(tmp.path()).unwrap();
        assert_eq!(a.init_command, "npm install");
        assert_eq!(a.hash, b.hash, "同内容哈希稳定");
        assert_eq!(a.artifact_dir, "node_modules");
    }

    #[test]
    fn content_change_changes_hash() {
        let tmp = fixture("Cargo.lock");
        let a = detect_lockfile(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("Cargo.lock"), "[package]").unwrap();
        let b = detect_lockfile(tmp.path()).unwrap();
        assert_ne!(a.hash, b.hash);
        assert_eq!(b.init_command, "cargo build");
    }

    #[test]
    fn no_lockfile_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(detect_lockfile(tmp.path()).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn venv_prefix_requires_venv_dir() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(venv_activation_prefix(tmp.path(), ShellKind::Pwsh).is_none());
        std::fs::create_dir_all(tmp.path().join(".venv/Scripts")).unwrap();
        std::fs::write(tmp.path().join(".venv/Scripts/Activate.ps1"), "").unwrap();
        let prefix = venv_activation_prefix(tmp.path(), ShellKind::Pwsh).unwrap();
        assert!(prefix.contains("Activate.ps1"));
    }

    #[test]
    fn compose_prepends_init_when_stale() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE fingerprints (command_id TEXT PRIMARY KEY, lock_hash TEXT, updated_at TEXT);")
            .unwrap();
        let tmp = fixture("package-lock.json");
        let out = compose_pipeline(&conn, "cmd-1", "npm run dev", tmp.path(), ShellKind::Pwsh).unwrap();
        assert!(
            out.starts_with("npm install && ") && out.ends_with("npm run dev"),
            "指纹未初始化 → init && 主命令"
        );
    }

    #[test]
    fn compose_skips_init_when_fingerprint_matches() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE fingerprints (command_id TEXT PRIMARY KEY, lock_hash TEXT, updated_at TEXT);")
            .unwrap();
        let tmp = fixture("package-lock.json");
        std::fs::create_dir_all(tmp.path().join("node_modules")).unwrap();
        let lock = detect_lockfile(tmp.path()).unwrap();
        conn.execute(
            "INSERT INTO fingerprints (command_id, lock_hash) VALUES ('cmd-1', ?1)",
            rusqlite::params![lock.hash],
        )
        .unwrap();
        let out = compose_pipeline(&conn, "cmd-1", "npm run dev", tmp.path(), ShellKind::Pwsh).unwrap();
        assert_eq!(out, "npm run dev", "指纹一致且产物存在 → 跳过初始化");
    }
}
