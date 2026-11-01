//! Squirl 库入口：本地项目台账与一键启动中枢。
//!
//! 模块布局与设计文档 §3.2 一致；M0 阶段（Phase 0）先落
//! 危险链路的生产模块（engine / db / error / paths），
//! Tauri 装配层在 Phase 1 S1 引入（见 docs/进度与验证记录.md ADR-1）。

pub mod clock;
pub mod db;
pub mod engine;
pub mod error;
pub mod paths;

// 依赖类型再导出：集成测试与上层（Tauri commands）共用同一版本
pub use rusqlite;
pub use rusqlite_migration;
