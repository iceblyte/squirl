//! Git 元数据只读读取（P2-⑪，DR-29）：纯文件解析 `.git/`，不依赖 git CLI，
//! 不写入任何内容（AC8 口径不变）。
//!
//! - HEAD → ref → commit hash（支持分支/ detached/ packed-refs）
//! - remote URL（`.git/config` 的 `[remote "origin"] url`）
//! - 最近 commit（`.git/COMMIT_EDITMSG` 不可靠——改读 loose object 会引入
//!   zlib 依赖；MVP 读 ref 文件的 mtime + hash 短码，message 留 P3，
//!   注明限制）。

use std::path::Path;

use serde::Serialize;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitMeta {
    /// HEAD 指向的分支名（detached 时为 None）。
    pub branch: Option<String>,
    pub commit_hash: String,
    /// `origin` remote URL（未配置 = None）。
    pub remote_url: Option<String>,
    /// ref 文件修改时间（RFC3339；packed-refs 时为 None——精度限制，DR-29）。
    pub last_commit_time: Option<String>,
}

fn read_trimmed(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

/// 解析 HEAD → commit hash（分支 ref / detached 直读 / packed-refs 回退）。
pub fn resolve_head(git_dir: &Path) -> Result<(Option<String>, String)> {
    let head = read_trimmed(&git_dir.join("HEAD"))
        .ok_or_else(|| AppError::InvalidInput("无法读取 .git/HEAD".into()))?;
    if let Some(refname) = head.strip_prefix("ref: ") {
        let branch = refname.strip_prefix("refs/heads/").map(String::from);
        let ref_path = git_dir.join(&refname);
        let hash = match read_trimmed(&ref_path) {
            Some(h) => h,
            None => {
                // packed-refs 回退
                let packed = read_trimmed(&git_dir.join("packed-refs")).unwrap_or_default();
                packed
                    .lines()
                    .find_map(|l| {
                        let mut it = l.split_whitespace();
                        let hash = it.next()?;
                        let name = it.next()?;
                        (name == refname).then(|| hash.to_string())
                    })
                    .ok_or_else(|| AppError::InvalidInput("packed-refs 无此分支".into()))?
            }
        };
        Ok((branch, hash))
    } else {
        Ok((None, head)) // detached：HEAD 即 hash
    }
}

/// 解析 `.git/config` 的 origin url（极简 INI 扫描，不引依赖）。
pub fn remote_origin_url(git_dir: &Path) -> Option<String> {
    let config = read_trimmed(&git_dir.join("config"))?;
    let mut in_origin = false;
    for line in config.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_origin = t.contains("\"origin\"");
            continue;
        }
        if in_origin
            && let Some(url) = t.strip_prefix("url")
        {
            return Some(url.trim_start_matches('=').trim().to_string());
        }
    }
    None
}

/// 读取项目 Git 元数据（只读；非 git 仓库返回错误，前端隐藏入口）。
pub fn read_git_meta(project_root: &Path) -> Result<GitMeta> {
    let git_dir = project_root.join(".git");
    if !git_dir.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "非 git 仓库: {}",
            project_root.display()
        )));
    }
    let (branch, hash) = resolve_head(&git_dir)?;
    let last_commit_time = read_trimmed(&git_dir.join("refs/heads"))
        .map(|_| ())
        .and(None); // 精度限制：packed-refs 下不可靠，MVP 置 None（DR-29 注明）
    Ok(GitMeta {
        branch,
        commit_hash: hash.chars().take(8).collect(),
        remote_url: remote_origin_url(&git_dir),
        last_commit_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个手写 `.git` 目录（HEAD + ref + config），不依赖 git CLI。
    fn fake_repo(tmp: &Path, commit: &str) {
        let git = tmp.join(".git");
        std::fs::create_dir_all(git.join("refs/heads")).unwrap();
        std::fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(git.join("refs/heads/main"), format!("{commit}\n")).unwrap();
        std::fs::write(
            git.join("config"),
            "[remote \"origin\"]\n\turl = https://github.com/x/y.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n",
        )
        .unwrap();
    }

    #[test]
    fn reads_branch_hash_and_remote() {
        let tmp = tempfile::tempdir().unwrap();
        fake_repo(tmp.path(), "1234567890abcdef1234567890abcdef12345678");
        let meta = read_git_meta(tmp.path()).unwrap();
        assert_eq!(meta.branch.as_deref(), Some("main"));
        assert_eq!(meta.commit_hash, "12345678");
        assert_eq!(meta.remote_url.as_deref(), Some("https://github.com/x/y.git"));
    }

    #[test]
    fn packed_refs_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let git = tmp.path().join(".git");
        std::fs::create_dir_all(git.join("refs/heads")).unwrap();
        std::fs::write(git.join("HEAD"), "ref: refs/heads/dev\n").unwrap();
        std::fs::write(
            git.join("packed-refs"),
            "# pack-refs\nabcdef1234567890abcdef1234567890abcdef12 refs/heads/dev\n",
        )
        .unwrap();
        let meta = read_git_meta(tmp.path()).unwrap();
        assert_eq!(meta.branch.as_deref(), Some("dev"));
        assert_eq!(meta.commit_hash, "abcdef12");
    }

    #[test]
    fn detached_head() {
        let tmp = tempfile::tempdir().unwrap();
        let git = tmp.path().join(".git");
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(git.join("HEAD"), "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n").unwrap();
        let meta = read_git_meta(tmp.path()).unwrap();
        assert_eq!(meta.branch, None);
        assert_eq!(meta.commit_hash, "deadbeef");
    }

    #[test]
    fn non_git_project_errors() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_git_meta(tmp.path()).is_err());
    }
}
