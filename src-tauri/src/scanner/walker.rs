//! 扫描器（设计 §5.2，F1）：jwalk 并行遍历 + 排除剪枝 + 深度裁剪 +
//! symlink 防环与根外不跟随 + 500 条暂停协议 + 「本次发现」会话内存。
//!
//! - 排除按**条目名**匹配内置名单，在 jwalk `process_read_dir` 中剪枝——
//!   被排除目录整个子树不入队（AC7 的克制性来源）。MVP 名单不可编辑。
//! - 深度语义：根的一级子文件夹 = depth 1（默认 1，上限 10）。
//! - symlink（需求两条硬限制）：`follow_links(false)` 下手动解析目标——
//!   目标已访问（防环）则跳过；目标在所有根之外则只列出条目不下降；
//!   目标在根内则以下剩余深度继续遍历。
//! - 暂停协议：命中 500 → `on_pause` 回调（命令层发 `scan://paused` 事件
//!   并阻塞等 `resolve_scan_pause`）→ Continue / ReduceDepth / Cancel。
//!   Continue 后本次不再暂停；ReduceDepth = 当前根剩余部分与后续根按
//!   深度 1 续扫（已扫条目不回滚，按路径去重）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::scanner::flags::detect_flags;

/// 内置默认排除名单（需求 F1，MVP 不可编辑）。
pub const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    "target",
    "dist",
    "build",
    ".next",
    ".idea",
    ".vscode",
];

/// 全部根目录累计条目上限（需求 F1）。
pub const SCAN_LIMIT: usize = 500;

/// 扫描深度默认值（只列一级子文件夹）与上限。
pub const DEFAULT_DEPTH: u32 = 1;
pub const MAX_DEPTH: u32 = 10;

/// `DiscoveryEntry`（契约 §5.4.1：get_discovery 出参）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryEntry {
    pub root_id: String,
    /// 绝对路径（登记直接可用）。
    pub path: String,
    /// 相对根目录的平铺路径（展示用）。
    pub rel_path: String,
    pub name: String,
    /// 命中的标志文件（前端高亮）。
    pub flags: Vec<String>,
}

/// 暂停时的三选一裁决（契约 resolve_scan_pause）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseDecision {
    Continue,
    ReduceDepth,
    Cancel,
}

impl PauseDecision {
    pub fn parse_decision(s: &str) -> crate::error::Result<Self> {
        match s {
            "continue" => Ok(Self::Continue),
            "reduce_depth" => Ok(Self::ReduceDepth),
            "cancel" => Ok(Self::Cancel),
            other => Err(crate::error::AppError::InvalidInput(format!(
                "未知裁决: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
struct WalkCfg {
    depth: u32,
    /// DR-21：自定义排除（与内置并集；随 cfg 传入闭包）。
    excludes: Vec<String>,
}

enum RootFlow {
    Completed,
    /// 命中上限后前端选择「降低深度」：当前根按深度 1 续扫
    ReducedDepth,
    Aborted,
}

pub struct Walker {
    cancel: CancellationToken,
    /// 已访问真实路径（symlink 防环，需求硬限制①）。
    visited: HashSet<String>,
    /// 已列出条目路径（ReduceDepth 续扫去重，已扫部分不回滚）。
    seen: HashSet<String>,
    entries: Vec<DiscoveryEntry>,
    /// 根前缀规范键（symlink 目标「根外不下降」判定）。
    root_norms: Vec<String>,
    limit: usize,
}

impl Walker {
    fn new(cancel: CancellationToken, root_norms: Vec<String>) -> Self {
        Self {
            cancel,
            visited: HashSet::new(),
            seen: HashSet::new(),
            entries: Vec::new(),
            root_norms,
            limit: SCAN_LIMIT,
        }
    }

    fn in_any_root(&self, norm: &str) -> bool {
        self.root_norms.iter().any(|r| {
            norm == r
                || norm.starts_with(&format!("{r}{}", std::path::MAIN_SEPARATOR))
                || (std::path::MAIN_SEPARATOR != '/' && norm.starts_with(&format!("{r}/")))
        })
    }

    /// 列出条目；返回是否为新增（ReduceDepth 续扫 / symlink 重复触达时
    /// 路径去重跳过——已扫部分不回滚）。
    fn list_dir(&mut self, root_id: &str, root_len: usize, path: &Path) -> bool {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let display = path.to_string_lossy().into_owned();
        if !self.seen.insert(display.clone()) {
            return false;
        }
        let rel = display[root_len..].trim_start_matches(['\\', '/']).to_string();
        self.entries.push(DiscoveryEntry {
            root_id: root_id.to_string(),
            rel_path: rel,
            name,
            path: display,
            flags: detect_flags(path),
        });
        true
    }

    /// 遍历一个根（或链接目标）。返回终态流向。
    #[allow(clippy::too_many_arguments)] // 遍历上下文：根身份/深度/回调/裁决
    fn walk_one(
        &mut self,
        root_id: &str,
        root: &Path,
        cfg: WalkCfg,
        on_progress: &mut dyn FnMut(usize, u32, u32),
        roots_done: u32,
        roots_total: u32,
        on_pause: &mut dyn FnMut() -> PauseDecision,
    ) -> RootFlow {
        let root_display = root.to_string_lossy().into_owned();
        let root_len = root_display.len();
        let depth = cfg.depth as usize;

        let cfg_excludes = cfg.excludes.clone();
        let walk = jwalk::WalkDir::new(root)
            .follow_links(false)
            .sort(false)
            .min_depth(1)
            .max_depth(depth)
            .process_read_dir(move |_depth, _path, _state, children| {
                children.retain(|e| match e {
                    Ok(entry) => {
                        let ft = entry.file_type();
                        // 只保留目录与符号链接（链接目标在迭代时手动解析）；
                        // 排除 = 内置名单 ∪ 自定义名单（DR-21），整个子树不入队
                        let name = entry.file_name().to_string_lossy().into_owned();
                        (ft.is_dir() || ft.is_symlink())
                            && !EXCLUDED_DIRS.contains(&name.as_str())
                            && !cfg_excludes.iter().any(|x| x == &name)
                    }
                    Err(_) => false,
                });
            });

        for entry in walk.into_iter().flatten() {
            if self.cancel.is_cancelled() {
                return RootFlow::Aborted;
            }
            let ft = entry.file_type();
            let path = entry.path();
            let entry_depth = entry.depth() as u32;

            if ft.is_symlink() {
                // 硬限制②：解析真实目标
                let Ok(target) = std::fs::canonicalize(&path) else {
                    continue; // 死链接：不列出也不跟随
                };
                let norm = crate::paths::normalize_key(&target).unwrap_or_default();
                if self.visited.contains(&norm) {
                    continue; // 防环：已访问真实路径跳过
                }
                let outside = !self.in_any_root(&norm);
                // 列出链接条目本身（两个分支都列出）
                let added = self.list_dir(root_id, root_len, &path);
                self.visited.insert(norm.clone());
                if !outside {
                    // 根内目标：以下剩余深度继续遍历
                    let remain = cfg.depth.saturating_sub(entry_depth).max(1);
                    let flow = self.walk_one(
                        root_id,
                        &target,
                        WalkCfg { depth: remain, excludes: cfg.excludes.clone() },
                        on_progress,
                        roots_done,
                        roots_total,
                        on_pause,
                    );
                    if matches!(flow, RootFlow::Aborted) {
                        return RootFlow::Aborted;
                    }
                }
                // 根外目标：只列出条目不下降（硬限制②）
                // 上限检查只在新增条目后触发（否则 ReduceDepth 续扫会零进度死循环）
                if added && self.entries.len() >= self.limit {
                    on_progress(self.entries.len(), roots_done, roots_total);
                    match on_pause() {
                        PauseDecision::Continue => {
                            self.limit = usize::MAX; // 本次扫描不再暂停
                        }
                        PauseDecision::ReduceDepth => return RootFlow::ReducedDepth,
                        PauseDecision::Cancel => return RootFlow::Aborted,
                    }
                }
            } else {
                let added = self.list_dir(root_id, root_len, &path);
                if added && self.entries.len() >= self.limit {
                    on_progress(self.entries.len(), roots_done, roots_total);
                    match on_pause() {
                        PauseDecision::Continue => {
                            self.limit = usize::MAX; // 本次扫描不再暂停
                        }
                        PauseDecision::ReduceDepth => return RootFlow::ReducedDepth,
                        PauseDecision::Cancel => return RootFlow::Aborted,
                    }
                }
            }
        }
        on_progress(self.entries.len(), roots_done, roots_total);
        RootFlow::Completed
    }

}

/// 扫描会话（「本次发现」瞬时语义：不入库、重扫重算、重启即空，G5）。
#[derive(Default)]
pub struct DiscoverySession {
    pub entries: Vec<DiscoveryEntry>,
    /// 单飞令牌：Some = 正在扫描（start_scan 单飞，E_SCAN_IN_PROGRESS）。
    pub running: Option<Arc<CancellationToken>>,
    /// 裁决通道（暂停协议；扫描结束即清理）。
    pub decision_tx: Option<std::sync::mpsc::Sender<PauseDecision>>,
    /// 最近一次扫描条目数（进度摘要）。
    pub last_count: usize,
}

pub type SharedSession = Arc<Mutex<DiscoverySession>>;

pub struct ScanOutcome {
    pub count: usize,
    pub duration_ms: u128,
    pub cancelled: bool,
}

/// `RootScanInput`：DR-21 逐根深度（None = 跟随全局）。
pub type RootScanInput = (String, PathBuf, Option<u32>);

/// 执行一次全量扫描（阻塞，须在 spawn_blocking 中调用）。
/// `custom_excludes` 与内置名单并集生效（DR-21）。
pub fn scan(
    session: &SharedSession,
    roots: &[RootScanInput],
    depth: u32,
    cancel: CancellationToken,
    on_progress: impl FnMut(usize, u32, u32) + Send,
    on_pause: impl FnMut() -> PauseDecision + Send,
    custom_excludes: &[String],
) -> Result<ScanOutcome> {
    let started = Instant::now();
    let root_norms = roots
        .iter()
        .map(|(_, p, _)| crate::paths::normalize_key(p).unwrap_or_default())
        .collect();
    let mut walker = Walker::new(cancel, root_norms);
    let total = roots.len() as u32;

    let finish = |walker: Walker, cancelled: bool| {
        let outcome = ScanOutcome {
            count: walker.entries.len(),
            duration_ms: started.elapsed().as_millis(),
            cancelled,
        };
        let mut s = session.lock().unwrap();
        s.entries = walker.entries;
        s.last_count = outcome.count;
        s.running = None;
        outcome
    };

    let mut progress = on_progress;
    let mut pause = on_pause;
    let mut reduced = false;
    for (i, (root_id, root, root_depth)) in roots.iter().enumerate() {
        let roots_done = i as u32;
        loop {
            let cfg = WalkCfg {
                depth: if reduced {
                    1
                } else {
                    root_depth.unwrap_or(depth).min(MAX_DEPTH)
                },
                excludes: custom_excludes.to_vec(),
            };
            match walker.walk_one(root_id, root, cfg, &mut progress, roots_done, total, &mut pause) {
                RootFlow::Aborted => {
                    let o = finish(walker, true);
                    return Ok(o);
                }
                // 当前根剩余部分按深度 1 续扫（list_dir 按 seen 去重，
                // 已扫条目不回滚）；re-walk 再次命中上限时继续按同一裁决循环
                RootFlow::ReducedDepth => {
                    reduced = true;
                }
                RootFlow::Completed => break,
            }
        }
    }

    let o = finish(walker, false);
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn always_continue() -> impl FnMut() -> PauseDecision {
        || PauseDecision::Continue
    }

    #[test]
    fn depth_exclusion_and_flags() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(root.join("proj-a/node_modules/junk")).unwrap();
        std::fs::write(root.join("proj-a/package.json"), "{}").unwrap();
        std::fs::create_dir_all(root.join("proj-b/.git")).unwrap();
        std::fs::create_dir_all(root.join("nested/deep/proj-c")).unwrap();

        let session: SharedSession = Arc::default();
        let outcome = scan(
            &session,
            &[("r1".to_string(), root.clone(), None)],
            1, // 默认深度：只列一级
            CancellationToken::new(),
            |_, _, _| {},
            always_continue(),
            &[],
        )
        .unwrap();

        let (entries, names) = {
            let locked = session.lock().unwrap();
            let names: Vec<String> = locked.entries.iter().map(|e| e.name.clone()).collect();
            (locked.entries.clone(), names)
        };
        assert!(names.contains(&"proj-a".to_string()) && names.contains(&"proj-b".to_string()) && names.contains(&"nested".to_string()));
        // 排除名单子树剪枝：node_modules 与 .git 不出现
        assert!(!names.iter().any(|n| n.as_str() == "junk" || n == ".git"));
        // 深度 1 不下钻到二级
        assert!(!names.iter().any(|n| n.as_str() == "proj-c" || n == "deep"));
        let a = entries.iter().find(|e| e.name == "proj-a").unwrap();
        assert_eq!(a.flags, vec!["package.json"]);
        assert_eq!(a.rel_path.trim_start_matches(['\\', '/']), "proj-a");
        assert_eq!(outcome.count, entries.len());
        assert!(!outcome.cancelled);
    }

    #[test]
    fn depth_two_reaches_second_level() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(root.join("nested/deep/proj-c")).unwrap();

        let session: SharedSession = Arc::default();
        scan(
            &session,
            &[("r1".to_string(), root, None)],
            2,
            CancellationToken::new(),
            |_, _, _| {},
            always_continue(),
            &[],
        )
        .unwrap();
        let names: Vec<String> = {
            let locked = session.lock().unwrap();
            locked.entries.iter().map(|e| e.name.clone()).collect()
        };
        assert!(names.contains(&"deep".to_string()), "深度 2 应到二级");
        assert!(!names.contains(&"proj-c".to_string()), "proj-c 在三级，depth 2 不可见");
    }

    #[test]
    fn symlink_loop_and_outside_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(root.join("proj")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(outside.join("secret")).unwrap();
        let link_outside = root.join("to-outside");
        let link_loop = root.join("self-loop");
        let mut links_ok = true;
        let r1 = make_symlink(&outside, &link_outside);
        let r2 = make_symlink(&root, &link_loop);
        if r1.is_err() || r2.is_err() {
            // Windows 无符号链接特权（需开发者模式）：降级跳过链接断言
            links_ok = false;
        }

        let session: SharedSession = Arc::default();
        scan(
            &session,
            &[("r1".to_string(), root.clone(), None)],
            3,
            CancellationToken::new(),
            |_, _, _| {},
            always_continue(),
            &[],
        )
        .unwrap();
        let entries = session.lock().unwrap();
        if links_ok {
            let names: Vec<String> = entries.entries.iter().map(|e| e.name.clone()).collect();
            assert!(names.contains(&"to-outside".to_string()), "根外链接条目必须列出");
            assert!(
                !names.contains(&"secret".to_string()),
                "根外目标只列出条目不下降（硬限制②）"
            );
            // 防环：self-loop 目标只列一次
            let proj_listed = entries.entries.iter().filter(|e| e.name == "proj").count();
            assert_eq!(proj_listed, 1, "self-loop 目标只列一次（硬限制①）");
        }
        assert!(!outcome_duration_exploded(&entries.entries.len()));
    }

    #[cfg(windows)]
    fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(unix)]
    fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    fn outcome_duration_exploded(count: &usize) -> bool {
        // 防环退化的粗略哨兵：条目数不应因子链接重复爆炸
        *count > 1000
    }

    #[test]
    fn limit_pause_continue_protocol() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        for i in 0..600 {
            std::fs::create_dir_all(root.join(format!("proj-{i:03}"))).unwrap();
        }
        let session: SharedSession = Arc::default();
        let mut pauses = 0;
        let outcome = scan(
            &session,
            &[("r1".to_string(), root, None)],
            1,
            CancellationToken::new(),
            |_, _, _| {},
            || {
                pauses += 1;
                PauseDecision::Continue
            },
            &[],
        )
        .unwrap();
        assert_eq!(pauses, 1, "命中上限暂停一次，Continue 后不再暂停");
        assert_eq!(outcome.count, 600);
        assert_eq!(session.lock().unwrap().entries.len(), 600);
    }

    #[test]
    fn limit_pause_cancel_protocol() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        for i in 0..600 {
            std::fs::create_dir_all(root.join(format!("proj-{i:03}"))).unwrap();
        }
        let session: SharedSession = Arc::default();
        let outcome = scan(
            &session,
            &[("r1".to_string(), root, None)],
            1,
            CancellationToken::new(),
            |_, _, _| {},
            || PauseDecision::Cancel,
            &[],
        )
        .unwrap();
        assert!(outcome.cancelled);
        assert_eq!(outcome.count, SCAN_LIMIT, "取消时保留已达条目");
    }

    #[test]
    fn limit_pause_reduce_depth() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        for i in 0..600 {
            std::fs::create_dir_all(root.join(format!("proj-{i:03}"))).unwrap();
        }
        let session: SharedSession = Arc::default();
        let outcome = scan(
            &session,
            &[("r1".to_string(), root, None)],
            2,
            CancellationToken::new(),
            |_, _, _| {},
            || PauseDecision::ReduceDepth,
            &[],
        )
        .unwrap();
        // 已扫 500 条不回滚；ReduceDepth 只影响后续遍历（本 fixture 一级即全部）
        assert_eq!(outcome.count, 600, "一级条目在 ReduceDepth 前已收集，不回滚");
        assert!(!outcome.cancelled);
    }
}
