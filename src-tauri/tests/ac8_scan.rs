//! AC8（扫描路径）：扫描全程对被管理目录零写入。
//! 项目根快照（文件清单 + 内容哈希）前后比对，任何差异即失败。

#![cfg(windows)]

use std::collections::BTreeMap;
use std::path::Path;

use squirl::scanner::{scan, DEFAULT_DEPTH};
use tokio_util::sync::CancellationToken;

/// 递归快照：相对路径 → 内容哈希（简单 FNV-1a，足以检测任何变化）。
fn snapshot(dir: &Path, base: &Path, out: &mut BTreeMap<String, u64>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            let rel = p
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let meta = e.metadata().unwrap();
            if meta.is_dir() {
                out.insert(format!("{rel}/"), 0);
                snapshot(&p, base, out);
            } else {
                let bytes = std::fs::read(&p).unwrap_or_default();
                let mut hash: u64 = 0xcbf29ce484222325;
                for b in &bytes {
                    hash ^= *b as u64;
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                out.insert(rel, hash);
            }
        }
    }
}

#[test]
fn ac8_scan_leaves_project_dirs_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    // 含扫描会触碰的内容：标志文件、node_modules（walker 会 read_dir 剪枝）、深层文件
    std::fs::create_dir_all(root.join("proj/node_modules/esbuild/bin")).unwrap();
    std::fs::write(root.join("proj/package.json"), "{\"name\":\"x\"}").unwrap();
    std::fs::write(root.join("proj/node_modules/esbuild/bin/esbuild"), "binary").unwrap();
    std::fs::create_dir_all(root.join("deep/a/b/c")).unwrap();
    std::fs::write(root.join("deep/a/b/c/file.txt"), "content").unwrap();
    // 元数据也要比对：记录 mtime
    fn mtimes(dir: &Path, base: &Path, out: &mut BTreeMap<String, u64>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let meta = e.metadata().unwrap();
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0);
                out.insert(format!("{rel}#m"), mtime);
                if meta.is_dir() {
                    mtimes(&p, base, out);
                }
            }
        }
    }

    let mut before_files = BTreeMap::new();
    let mut before_mtimes = BTreeMap::new();
    snapshot(&root, &root, &mut before_files);
    mtimes(&root, &root, &mut before_mtimes);

    // 扫描（含排除子树遍历、标志探测——全部只读路径）
    let session = std::sync::Arc::new(std::sync::Mutex::new(
        squirl::scanner::DiscoverySession::default(),
    ));
    let outcome = scan(
        &session,
        &[("r1".to_string(), root.clone())],
        DEFAULT_DEPTH.max(3),
        CancellationToken::new(),
        |_, _, _| {},
        || squirl::scanner::PauseDecision::Continue,
    )
    .unwrap();
    assert!(outcome.count >= 2, "扫描应发现 proj 与 deep");

    // 前后快照比对：文件内容与 mtime 均不得变化
    let mut after_files = BTreeMap::new();
    let mut after_mtimes = BTreeMap::new();
    snapshot(&root, &root, &mut after_files);
    mtimes(&root, &root, &mut after_mtimes);
    assert_eq!(before_files, after_files, "扫描不得创建/修改/删除任何文件");
    assert_eq!(before_mtimes, after_mtimes, "扫描不得触碰任何 mtime");
}
