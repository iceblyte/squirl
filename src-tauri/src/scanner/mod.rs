//! 扫描器（设计 §5.2）：walker / flags / session 三件套。

pub mod flags;
pub mod walker;

pub use walker::{
    scan, DiscoveryEntry, DiscoverySession, PauseDecision, ScanOutcome, SharedSession,
    DEFAULT_DEPTH, MAX_DEPTH, SCAN_LIMIT,
};
