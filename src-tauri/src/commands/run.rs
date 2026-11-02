//! run 域命令（契约 §5.4.1）：执行模式解析 + 外部一键启动（DR-16 判别联合）。
//!
//! 内置模式（state 机 / run 注册表）在 Phase 2 S6 落地——当前 mode=builtin
//! 返回 E_INVALID_INPUT 明示未开放（UI 侧置灰，AGENTS.md Phase 1 约束）。

use serde::Serialize;
use uuid::Uuid;

use crate::db::repo;
use crate::engine::adapter::{
    build_builtin_launch, ConsoleHostAdapter, GenericExecAdapter, LaunchJob, TerminalAdapter,
    WindowsTerminalAdapter,
};
use crate::engine::effective::{build_external_parts, resolve_cwd, EffectiveExec};
use crate::engine::emulator::EmulatorKind;
use crate::engine::spawn::spawn_external;
use crate::error::{AppError, IpcError, Result};

/// `StartResult`（契约 §5.4 运行域判别联合，DR-16③）。
#[derive(Debug, Serialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum StartResult {
    Builtin { run: crate::engine::supervisor::RunInfo },
    External { launched: bool },
}

/// `get_effective_execution_config` — `project_id?` → 有效两层配置。
#[tauri::command]
pub async fn get_effective_execution_config(
    project_id: Option<String>,
    db: tauri::State<'_, crate::db::Db>,
) -> std::result::Result<EffectiveExec, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn();
        crate::engine::effective::resolve(&conn, project_id.as_deref())
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// 外部模式启动的共同路径：组装 Job → 适配器 → spawn → launch record → 写回。
/// pub 供 AC3 集成测试复用同一条代码路径。
pub fn launch_external(
    db: &crate::db::Db,
    command_id: Option<Uuid>,
    project_id: Option<Uuid>,
    command_line: String,
    cwd_override: Option<&str>,
    project_root: &std::path::Path,
) -> Result<u32> {
    let (shell_spec, emu_spec) = {
        let conn = db.conn();
        build_external_parts(&conn, project_id.as_ref().map(std::string::ToString::to_string).as_deref())?
    };
    let cwd = resolve_cwd(cwd_override, project_root)?;

    // 适配器选择（模拟器 × shell；M0 定版表）
    let adapter: &dyn TerminalAdapter = match emu_spec.kind {
        EmulatorKind::WindowsTerminal => &WindowsTerminalAdapter {
            wt_path: emu_spec
                .program
                .clone()
                .ok_or_else(|| AppError::NoEmulator)?,
        },
        EmulatorKind::ConsoleHost => &ConsoleHostAdapter,
        EmulatorKind::GnomeTerminal | EmulatorKind::Konsole => &GenericExecAdapter {
            kind: emu_spec.kind,
            program: emu_spec
                .program
                .clone()
                .ok_or_else(|| AppError::NoEmulator)?,
        },
        EmulatorKind::MacTerminal => &crate::engine::adapter::MacTerminalAdapter,
        EmulatorKind::ITerm2 => &crate::engine::adapter::ITerm2Adapter,
        _ => &ConsoleHostAdapter,
    };

    let script_dir = db
        .path()
        .parent()
        .map(|p| p.join("tmp"))
        .ok_or_else(|| AppError::Engine("数据目录解析失败".into()))?;
    let job = LaunchJob {
        shell: shell_spec.clone(),
        command_line,
        cwd: cwd.clone(),
        keep_open: true, // 外部模式一律保持打开（附录 A #4）
        with_marker: true,
        script_dir: Some(script_dir),
    };
    let plan = adapter.build_launch(&job)?;

    // DR-12 launch record（spawn 前；run_id 对外部模式为 None）
    tracing::info!(
        target: "launch_record",
        mode = "external",
        run_id = Option::<String>::None,
        command_id = ?command_id,
        adapter_id = %adapter.id(),
        shell = %shell_spec.program,
        emulator = %emu_spec.kind.id(),
        cwd = %cwd.display(),
        program = %plan.program,
        "external launch"
    );

    let spawned = spawn_external(&plan)?;

    // DR-16①：外部模式启动成功即累计 run_count + 更新 last_run_at
    if let Some(pid_project) = project_id {
        let mut conn = db.conn();
        repo::record_external_launch(&mut conn, command_id, &pid_project.to_string())?;
    }
    Ok(spawned.pid)
}

/// `start_command` — `(command_id, { force_new? })` → `StartResult`。
#[tauri::command]
pub async fn start_command(
    command_id: String,
    force_new: Option<bool>,
    db: tauri::State<'_, crate::db::Db>,
) -> std::result::Result<StartResult, IpcError> {
    let db = db.inner().clone();
    // force_new 语义 = 内置模式重复启动拦截的「再启动一个」（S6）；
    // 外部模式 fire-and-forget，天然允许重复（记录到 launch record）
    let _force_new = force_new.unwrap_or(false);
    tokio::task::spawn_blocking(move || {
        let cmd = {
            let conn = db.conn();
            repo::get_command(&conn, &command_id)?
        };
        let project_root = {
            let conn = db.conn();
            let path: String = conn
                .query_row(
                    "SELECT path FROM projects WHERE id=?1",
                    rusqlite::params![cmd.project_id()],
                    |r| r.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        AppError::InvalidInput(format!("项目不存在: {}", cmd.project_id()))
                    }
                    other => AppError::Db(other),
                })?;
            std::path::PathBuf::from(path)
        };
        let pid = launch_external(
            &db,
            Some(Uuid::parse_str(&cmd.id).map_err(|e| AppError::InvalidInput(e.to_string()))?),
            Some(Uuid::parse_str(cmd.project_id()).map_err(|e| AppError::InvalidInput(e.to_string()))?),
            cmd.command_line.clone(),
            cmd.cwd.as_deref(),
            &project_root,
        )?;
        tracing::debug!(target: "launch_record", pid, "external spawned");
        Ok::<StartResult, AppError>(StartResult::External { launched: true })
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

/// `start_temp_command` — `({ project_id?, command_line, cwd })` → `StartResult`。
#[tauri::command]
pub async fn start_temp_command(
    project_id: Option<String>,
    command_line: String,
    cwd: Option<String>,
    db: tauri::State<'_, crate::db::Db>,
) -> std::result::Result<StartResult, IpcError> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let project_root = match &project_id {
            Some(pid) => {
                let conn = db.conn();
                let path: String = conn
                    .query_row(
                        "SELECT path FROM projects WHERE id=?1",
                        rusqlite::params![pid],
                        |r| r.get(0),
                    )
                    .map_err(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => {
                            AppError::InvalidInput(format!("项目不存在: {pid}"))
                        }
                        other => AppError::Db(other),
                    })?;
                std::path::PathBuf::from(path)
            }
            None => {
                let cwd = cwd.clone().ok_or_else(|| {
                    AppError::InvalidInput("临时命令需要 cwd 或 project_id".into())
                })?;
                std::path::PathBuf::from(cwd)
            }
        };
        // 临时命令没有命令行 cwd 字段：优先用入参 cwd，否则项目根
        let pid = launch_external(
            &db,
            None,
            project_id
                .as_ref()
                .and_then(|p| Uuid::parse_str(p).ok()),
            command_line,
            cwd.as_deref(),
            &project_root,
        )?;
        tracing::debug!(target: "launch_record", pid, "external temp spawned");
        Ok::<StartResult, AppError>(StartResult::External { launched: true })
    })
    .await
    .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
    .map_err(IpcError::from)
}

// 抑制未使用告警：build_builtin_launch 在 S6 supervisor 接线时使用
#[allow(dead_code)]
fn _builtin_not_available() -> Result<StartResult> {
    let _ = build_builtin_launch;
    Err(AppError::InvalidInput("内置模式将在 Phase 2 开放".into()))
}
