//! 应用错误层（设计 §5.8）：域错误（thiserror）→ IPC 结构化错误（INV-5）。
//!
//! INV-5：IPC 错误一律是结构化 `{code, message, detail?, data?}`，
//! 禁止裸字符串错误穿透到前端。错误码以 `E_` 前缀枚举，
//! 与设计 §5.4 冻结契约的错误码清单对齐。

use serde::Serialize;

/// IPC 结构化错误负载（INV-5）。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct IpcError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl IpcError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            detail: None,
            data: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// 域错误。`map_code()` 决定其 IPC 错误码。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("数据库损坏: {0}")]
    DbCorrupted(String),

    #[error("数据库备份/恢复失败: {0}")]
    Backup(String),

    /// 迁移提交成功但语义校验失败——经 premigrate 快照回滚后阻断启动（DR-13）。
    #[error("迁移失败，已回滚并阻断启动: {0}")]
    MigrationBlocked(String),

    #[error("路径错误: {0}")]
    Path(String),

    #[error("路径不存在: {0}")]
    PathNotFound(String),

    #[error("路径已登记: {0}")]
    DuplicatePath(String),

    #[error("引擎错误: {0}")]
    Engine(String),

    #[error("进程启动失败: {0}")]
    LaunchFailed(String),

    #[error("不支持的 shell: {0}")]
    ShellUnsupported(String),

    #[error("未找到可用的终端模拟器")]
    NoEmulator,

    #[error("扫描已在进行中")]
    ScanInProgress,

    #[error("参数错误: {0}")]
    InvalidInput(String),

    #[error("命令已在运行中")]
    AlreadyRunning { run_id: String, command_id: String },

    #[error("命令运行中，禁止删除")]
    CmdRunningNoDelete(String),

    #[error("项目有运行中的命令，禁止删除")]
    ProjectRunningNoDelete(String),
}

impl AppError {
    /// 映射到 INV-5 结构化错误码（设计 §5.4 错误码枚举）。
    pub fn map_code(&self) -> &'static str {
        match self {
            AppError::Db(_) => "E_DB",
            AppError::DbCorrupted(_) => "E_DB_CORRUPTED_RESTORED",
            AppError::Backup(_) => "E_DB_BACKUP",
            AppError::MigrationBlocked(_) => "E_MIGRATION_BLOCKED",
            AppError::Path(_) => "E_PATH",
            AppError::PathNotFound(_) => "E_PATH_NOT_FOUND",
            AppError::DuplicatePath(_) => "E_DUPLICATE_PATH",
            AppError::Engine(_) => "E_ENGINE",
            AppError::LaunchFailed(_) => "E_LAUNCH_FAILED",
            AppError::ShellUnsupported(_) => "E_SHELL_UNSUPPORTED",
            AppError::NoEmulator => "E_NO_EMULATOR",
            AppError::ScanInProgress => "E_SCAN_IN_PROGRESS",
            AppError::InvalidInput(_) => "E_INVALID_INPUT",
            AppError::AlreadyRunning { .. } => "E_ALREADY_RUNNING",
            AppError::CmdRunningNoDelete(_) => "E_CMD_RUNNING_NO_DELETE",
            AppError::ProjectRunningNoDelete(_) => "E_PROJECT_RUNNING_NO_DELETE",
        }
    }

    /// 附加 detail（错误结构化负载的第三段）。
    pub fn detail(&self) -> Option<String> {
        match self {
            AppError::LaunchFailed(d) | AppError::MigrationBlocked(d) => Some(d.clone()),
            AppError::DbCorrupted(d) | AppError::Backup(d) => Some(d.clone()),
            AppError::AlreadyRunning { run_id, .. } => Some(run_id.clone()),
            _ => None,
        }
    }

    /// AlreadyRunning 的 data 负载（契约：E_ALREADY_RUNNING{run_id}，前端「再启动一个」入口）。
    pub fn data(&self) -> Option<serde_json::Value> {
        match self {
            AppError::AlreadyRunning { run_id, command_id } => Some(serde_json::json!({
                "runId": run_id,
                "commandId": command_id,
            })),
            _ => None,
        }
    }
}

impl From<AppError> for IpcError {
    fn from(e: AppError) -> Self {
        let mut ipc = IpcError::new(e.map_code(), e.to_string());
        if let Some(detail) = e.detail() {
            ipc = ipc.with_detail(detail);
        }
        if let Some(data) = e.data() {
            ipc = ipc.with_data(data);
        }
        ipc
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_error_is_structured() {
        let e = AppError::LaunchFailed("wt.exe 退出码 1".to_string());
        let ipc: IpcError = e.into();
        assert_eq!(ipc.code, "E_LAUNCH_FAILED");
        assert_eq!(ipc.detail.as_deref(), Some("wt.exe 退出码 1"));
        assert!(ipc.message.contains("进程启动失败"));

        // 序列化后必须是 {code, message, detail}，无裸字符串形态
        let json = serde_json::to_value(&ipc).unwrap();
        assert_eq!(json["code"], "E_LAUNCH_FAILED");
        assert!(json["message"].is_string());
        assert!(json.get("data").is_none());
    }

    #[test]
    fn db_error_maps_to_code() {
        let e = AppError::Db(rusqlite::Error::InvalidColumnName("x".into()));
        assert_eq!(e.map_code(), "E_DB");
    }
}
