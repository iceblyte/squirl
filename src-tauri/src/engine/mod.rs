//! 执行引擎（设计 §5.3）：Shell 层 / 模拟器层 / 终端适配器 / 外部唤起 /
//! 日志管道与环形缓冲 / 内置模式进程监管。
pub mod adapter;
pub mod effective;
pub mod emulator;
pub mod logbuf;
pub mod pipeline;
pub mod quoting;
pub mod shell;
pub mod spawn;
pub mod supervisor;
