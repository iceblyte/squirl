//! IPC DTO 与领域模型（serde 结构体，设计 §3.2）。

/// `Settings`（契约 §5.4.1：get_settings / update_settings 的出参）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// external | builtin
    pub default_mode: String,
    /// None = 自动探测
    pub default_emulator: Option<String>,
    /// None = 自动解析
    pub default_shell: Option<String>,
    pub load_shell_profile: bool,
    pub scan_depth: u32,
}
