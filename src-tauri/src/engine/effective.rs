//! 两层配置解析（设计 §5.3.1）：Shell 层（两模式共用）+ 模拟器层（仅外部）。
//! 全局默认 + 项目级覆盖 `execution_override`（三值独立：None = 跟随全局）。

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::engine::emulator::{self, EmulatorKind, EmulatorSpec};
use crate::engine::shell::{self, ShellKind, ShellSpec};
use crate::error::{AppError, Result};

/// `EffectiveExecConfig`（契约 get_effective_execution_config 出参）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveExec {
    /// external | builtin
    pub mode: String,
    /// 解析后的 shell 名（pwsh/powershell/cmd/bash/zsh）。
    pub shell: String,
    /// 实际解析到的 shell 可执行路径（设置页展示）。
    pub shell_path: String,
    /// 仅外部模式：模拟器 id（wt/conhost/terminal.app/...）。
    pub emulator: Option<String>,
    /// R3/R4 类提示（PS5.1 无 && 等）。
    pub warnings: Vec<String>,
    pub supported: bool,
    /// 是否加载 profile（F7 设置 + 项目覆盖链路末端）。
    pub load_profile: bool,
}

/// 显式 shell 名 → 定位（设置页与项目覆盖的枚举值）。
fn locate_explicit_shell(name: &str) -> Result<ShellSpec> {
    let kind = match name {
        "pwsh" => ShellKind::Pwsh,
        "powershell" => ShellKind::PowerShell5,
        "cmd" => ShellKind::Cmd,
        "bash" => ShellKind::Bash,
        "zsh" => ShellKind::Zsh,
        other => {
            return Err(AppError::ShellUnsupported(format!(
                "不支持的 shell: {other}（支持 pwsh/powershell/cmd/bash/zsh）"
            )))
        }
    };
    let program = match kind {
        ShellKind::Cmd => r"C:\Windows\System32\cmd.exe".to_string(),
        ShellKind::Pwsh => shell::locate_on_path("pwsh")
            .ok_or_else(|| AppError::ShellUnsupported("未找到 pwsh".into()))?,
        ShellKind::PowerShell5 => shell::locate_on_path("powershell")
            .ok_or_else(|| AppError::ShellUnsupported("未找到 powershell.exe".into()))?,
        ShellKind::Bash | ShellKind::Zsh => {
            if cfg!(windows) && kind == ShellKind::Bash {
                shell::resolve_git_bash()
                    .ok_or_else(|| AppError::ShellUnsupported("未找到 Git Bash".into()))?
            } else {
                shell::locate_on_path(name)
                    .ok_or_else(|| AppError::ShellUnsupported(format!("未找到 {name}")))?
            }
        }
    };
    Ok(ShellSpec::new(kind, program))
}

/// 显式模拟器名 → EmulatorSpec（仅外部模式）。
fn locate_explicit_emulator(name: &str) -> Result<EmulatorSpec> {
    match name {
        "wt" => {
            let p = shell::locate_on_path("wt")
                .ok_or_else(|| AppError::NoEmulator)?;
            Ok(EmulatorSpec { kind: EmulatorKind::WindowsTerminal, program: Some(p) })
        }
        "conhost" => Ok(EmulatorSpec { kind: EmulatorKind::ConsoleHost, program: None }),
        "terminal.app" => Ok(EmulatorSpec { kind: EmulatorKind::MacTerminal, program: None }),
        "iterm2" => Ok(EmulatorSpec { kind: EmulatorKind::ITerm2, program: None }),
        "gnome-terminal" | "konsole" | "alacritty" | "kitty" | "xterm" => {
            let p = shell::locate_on_path(name).ok_or_else(|| AppError::NoEmulator)?;
            let kind = match name {
                "gnome-terminal" => EmulatorKind::GnomeTerminal,
                "konsole" => EmulatorKind::Konsole,
                "alacritty" => EmulatorKind::Alacritty,
                "kitty" => EmulatorKind::Kitty,
                _ => EmulatorKind::XTerm,
            };
            Ok(EmulatorSpec { kind, program: Some(p) })
        }
        other => Err(AppError::InvalidInput(format!("未知模拟器: {other}"))),
    }
}

/// 解析两层配置（合并项目覆盖）。`load_profile` 来自 settings。
pub fn resolve(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<EffectiveExec> {
    // 全局默认（settings 单行）
    let settings = crate::db::repo::get_settings(conn)?;
    let mut mode = settings.default_mode;
    let mut explicit_shell = settings.default_shell.clone();
    let mut explicit_emulator = settings.default_emulator.clone();
    let load_profile = settings.load_shell_profile;

    // 项目覆盖（三值独立，None = 跟随全局）
    if let Some(pid) = project_id {
        let (m, e, sh): (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT exec_mode, exec_emulator, exec_shell FROM projects WHERE id=?1",
                rusqlite::params![pid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::InvalidInput(format!("项目不存在: {pid}"))
                }
                other => AppError::Db(other),
            })?;
        if let Some(v) = m {
            mode = v;
        }
        explicit_emulator = e.or(explicit_emulator);
        explicit_shell = sh.or(explicit_shell);
    }

    // Shell 层
    let mut warnings: Vec<String> = Vec::new();
    let shell_spec = match &explicit_shell {
        Some(name) => match locate_explicit_shell(name) {
            Ok(s) => s,
            Err(AppError::ShellUnsupported(msg)) => {
                warnings.push(format!("指定的 shell 不可用（{msg}），已回退默认链"));
                shell::resolve(&shell::EnvProbe)?
            }
            Err(e) => return Err(e),
        },
        None => shell::resolve(&shell::EnvProbe)?,
    };
    warnings.extend(shell_spec.kind.warnings().iter().map(|s| (*s).to_string()));

    // 模拟器层（仅外部模式）
    let emu = if mode == "external" {
        match &explicit_emulator {
            Some(name) => match locate_explicit_emulator(name) {
                Ok(e) => Some(e),
                Err(AppError::NoEmulator) | Err(AppError::InvalidInput(_)) => {
                    warnings.push(format!("指定的模拟器不可用（{name}），已回退自动探测"));
                    emulator::resolve(&emulator::EnvProbe).ok()
                }
                Err(e) => return Err(e),
            },
            None => emulator::resolve(&emulator::EnvProbe).ok(),
        }
    } else {
        None
    };

    Ok(EffectiveExec {
        mode,
        shell: shell_spec.kind.display_name().to_string(),
        shell_path: shell_spec.program.clone(),
        emulator: emu.as_ref().map(|e| e.kind.id().to_string()),
        warnings,
        supported: shell_spec.supported(),
        load_profile,
    })
}

/// 组装外部启动的 ShellSpec（应用 load_profile 开关）与模拟器适配器选择。
pub fn build_external_parts(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<(ShellSpec, EmulatorSpec)> {
    let eff = resolve(conn, project_id)?;
    if eff.mode != "external" {
        return Err(AppError::InvalidInput("内置模式将在 Phase 2 开放".into()));
    }
    let mut shell_spec = locate_explicit_shell(&eff.shell)?;
    shell_spec.load_profile = eff.load_profile;
    let emu = match &eff.emulator {
        Some(name) => locate_explicit_emulator(name)?,
        None => emulator::resolve(&emulator::EnvProbe)?,
    };
    Ok((shell_spec, emu))
}

/// 内置模式的 Shell 解析（无模拟器层；AD-13 门禁解除后由 start 分发调用）。
pub fn build_builtin_parts(conn: &Connection, project_id: Option<&str>) -> Result<ShellSpec> {
    let eff = resolve(conn, project_id)?;
    let mut shell_spec = locate_explicit_shell(&eff.shell)?;
    shell_spec.load_profile = eff.load_profile;
    Ok(shell_spec)
}

/// 命令工作目录解析（附录 A #7：cwd 默认项目根、可改，两模式统一）。
/// 相对 cwd 相对项目根拼接；绝对 cwd 直接 canonical。
pub fn resolve_cwd(cwd: Option<&str>, project_root: &Path) -> Result<std::path::PathBuf> {
    match cwd {
        Some(c) if !c.trim().is_empty() => {
            let p = Path::new(c);
            if p.is_absolute() {
                crate::paths::canonical(p)
            } else {
                crate::paths::canonical(&project_root.join(p))
            }
        }
        _ => crate::paths::canonical(project_root),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db_with_project() -> (Connection, String, std::path::PathBuf) {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db::migrate::schema_migrations()
            .to_latest(&mut conn)
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("proj");
        std::fs::create_dir_all(&p).unwrap();
        let card = crate::db::repo::register_project(&conn, &p, None).unwrap();
        (conn, card.id, p)
    }

    #[cfg(windows)]
    #[test]
    fn resolve_defaults_to_external_with_auto_shell() {
        let (conn, pid, _) = setup_db_with_project();
        let eff = resolve(&conn, Some(&pid)).unwrap();
        assert_eq!(eff.mode, "external");
        assert!(matches!(eff.shell.as_str(), "pwsh" | "powershell" | "cmd"));
        assert!(!eff.shell_path.is_empty());
        assert!(eff.emulator.is_some());
    }

    #[cfg(windows)]
    #[test]
    fn project_override_overrides_mode_and_emulator() {
        let (conn, pid, _) = setup_db_with_project();
        conn.execute(
            "UPDATE projects SET exec_mode='external', exec_emulator='conhost' WHERE id=?1",
            rusqlite::params![pid],
        )
        .unwrap();
        let eff = resolve(&conn, Some(&pid)).unwrap();
        assert_eq!(eff.emulator.as_deref(), Some("conhost"));
    }

    #[test]
    fn unknown_shell_name_is_rejected() {
        assert!(locate_explicit_shell("fish").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn build_external_parts_respects_load_profile() {
        let (conn, pid, _) = setup_db_with_project();
        conn.execute(
            "UPDATE settings SET load_shell_profile=0 WHERE id=1",
            [],
        )
        .unwrap();
        let (spec, _emu) = build_external_parts(&conn, Some(&pid)).unwrap();
        assert!(!spec.load_profile);
        let args = spec.spawn_args("SCRIPT", false);
        assert!(args.contains(&"-NoProfile".to_string()), "关 profile → -NoProfile");
        assert!(!args.contains(&"-Login".to_string()));
    }

    #[test]
    fn cwd_resolution_relative_and_absolute() {
        let root = std::path::PathBuf::from("D:/proj");
        let abs = resolve_cwd(Some("D:/other"), &root).unwrap();
        assert!(abs.to_string_lossy().to_lowercase().starts_with("d:\\other"));
        let rel = resolve_cwd(Some("packages/app"), &root).unwrap();
        assert!(rel.to_string_lossy().contains("proj"));
        let default = resolve_cwd(None, &root).unwrap();
        assert_eq!(
            normalize_key_of(&default),
            normalize_key_of(&root)
        );
    }

    fn normalize_key_of(p: &Path) -> String {
        crate::paths::normalize_key(p).unwrap_or_default()
    }
}
