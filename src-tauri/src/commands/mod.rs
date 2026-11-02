//! IPC 层（设计 §3.2）：参数校验 → 调用领域层 → 统一错误映射。
//! 薄封装，禁止业务逻辑；所有命令返回 `Result<T, IpcError>`（INV-5）。

pub mod projects;
pub mod roots;
pub mod scan;
pub mod settings;
