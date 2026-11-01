//! 执行引擎（设计 §5.3）：Shell 层 / 模拟器层 / 终端适配器 / 外部唤起。
//! supervisor（进程监管）在 Spike B 落地。
pub mod adapter;
pub mod emulator;
pub mod quoting;
pub mod shell;
pub mod spawn;
