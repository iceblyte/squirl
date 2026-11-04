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
    /// DR-19：托盘开关（默认关；改后重启生效）
    pub tray_enabled: bool,
    /// DR-31：文件监听自动刷新（默认关）
    pub auto_watch: bool,
    /// DR-21：自定义排除名单（JSON 数组列；None = 仅内置名单）
    pub custom_excludes: Option<Vec<String>>,
}
