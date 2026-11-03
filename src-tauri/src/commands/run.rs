//! run 域命令（契约 §5.4.1）：执行模式解析 + 双模式启动（DR-16 判别联合）。
//!
//! 内置模式：supervisor state 机 + run 注册表（INV-3，S6 接线）；
//! 外部模式：M0 定版适配器 + fire-and-forget（INV-2 不入 Job Object）。

use serde::Serialize;
use uuid::Uuid;

use crate::db::repo;
use crate::engine::adapter::{
    ConsoleHostAdapter, GenericExecAdapter, LaunchJob, TerminalAdapter, WindowsTerminalAdapter,
};
use crate::engine::effective::{
    build_builtin_parts, build_external_parts, resolve_cwd, EffectiveExec,
};
use crate::engine::emulator::EmulatorKind;
use crate::engine::spawn::spawn_external;
use crate::engine::supervisor::{StartArgs, Supervisor};
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
        build_external_parts(&conn, project_id.map(|u| u.to_string()).as_deref())?
    };
    let cwd = resolve_cwd(cwd_override, project_root)?;

    // 适配器选择（模拟器 × shell；M0 定版表）
    let adapter: &dyn TerminalAdapter = match emu_spec.kind {
        EmulatorKind::WindowsTerminal => &WindowsTerminalAdapter {
            wt_path: emu_spec.program.clone().ok_or_else(|| AppError::NoEmulator)?,
        },
        EmulatorKind::ConsoleHost => &ConsoleHostAdapter,
        EmulatorKind::GnomeTerminal | EmulatorKind::Konsole => &GenericExecAdapter {
            kind: emu_spec.kind,
            program: emu_spec.program.clone().ok_or_else(|| AppError::NoEmulator)?,
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

/// 内置模式启动：supervisor state 机 + run 注册表（INV-3）。
fn launch_builtin(
    db: &crate::db::Db,
    supervisor: &Supervisor,
    command_id: Option<Uuid>,
    command_line: String,
    cwd_override: Option<&str>,
    project_root: &std::path::Path,
    force_new: bool,
) -> Result<crate::engine::supervisor::RunInfo> {
    // 重复启动拦截（设计 §5.3.3）：默认拒绝，force_new = 「再启动一个」
    if !force_new
        && let Some(cid) = command_id
        && let Some(existing) = supervisor.registry.active_for_command(cid)
    {
        return Err(AppError::AlreadyRunning {
            run_id: existing.to_string(),
            command_id: cid.to_string(),
        });
    }
    let shell_spec = {
        let conn = db.conn();
        build_builtin_parts(&conn, None)?
    };
    let cwd = resolve_cwd(cwd_override, project_root)?;
    let run_id = tokio::runtime::Handle::current().block_on(async {
        supervisor
            .start(StartArgs {
                command_id,
                command_line,
                cwd,
                shell: shell_spec,
                with_marker: true,
                env: vec![],
            })
            .await
    })?;
    // DR-16①：内置启动成功即累计 run_count（临时命令无行，repo 跳过）
    if let Some(cid) = command_id {
        let mut conn = db.conn();
        repo::record_builtin_launch(&mut conn, cid)?;
    }
    supervisor
        .registry
        .get(run_id)
        .map(|h| h.to_info())
        .ok_or_else(|| AppError::Engine("run 注册表缺少刚启动的实例".into()))
}

/// `start_command` — `(command_id, { force_new? })` → `StartResult`。
#[tauri::command]
pub async fn start_command(
    command_id: String,
    force_new: Option<bool>,
    db: tauri::State<'_, crate::db::Db>,
    supervisor: tauri::State<'_, Supervisor>,
) -> std::result::Result<StartResult, IpcError> {
    let db = db.inner().clone();
    let supervisor = supervisor.inner().clone();
    let force_new = force_new.unwrap_or(false);
    let cmd_id =
        Uuid::parse_str(&command_id).map_err(|e| IpcError::new("E_INVALID_INPUT", e.to_string()))?;

    // 解析命令与模式（阻塞查询）
    let (cmd, mode) = {
        let conn = db.conn();
        let cmd = repo::get_command(&conn, &command_id)?;
        let eff = crate::engine::effective::resolve(&conn, Some(cmd.project_id()))?;
        (cmd, eff.mode)
    };
    let project_root: std::path::PathBuf = {
        let conn = db.conn();
        let path: String = conn
            .query_row(
                "SELECT path FROM projects WHERE id=?1",
                rusqlite::params![cmd.project_id()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    IpcError::new("E_PATH_NOT_FOUND", format!("项目不存在: {}", cmd.project_id()))
                }
                other => IpcError::from(AppError::Db(other)),
            })?;
        std::path::PathBuf::from(path)
    };

    match mode.as_str() {
        "builtin" => {
            let run = launch_builtin(
                &db,
                &supervisor,
                Some(cmd_id),
                cmd.command_line.clone(),
                cmd.cwd.as_deref(),
                &project_root,
                force_new,
            )
            .map_err(IpcError::from)?;
            Ok(StartResult::Builtin { run })
        }
        _ => {
            let pid = tokio::task::spawn_blocking(move || {
                launch_external(
                    &db,
                    Some(cmd_id),
                    Some(
                        Uuid::parse_str(cmd.project_id())
                            .map_err(|e| AppError::InvalidInput(e.to_string()))?,
                    ),
                    cmd.command_line.clone(),
                    cmd.cwd.as_deref(),
                    &project_root,
                )
            })
            .await
            .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
            .map_err(IpcError::from)?;
            tracing::debug!(target: "launch_record", pid, "external spawned");
            Ok(StartResult::External { launched: true })
        }
    }
}

/// `start_temp_command` — `({ project_id?, command_line, cwd })` → `StartResult`。
#[tauri::command]
pub async fn start_temp_command(
    project_id: Option<String>,
    command_line: String,
    cwd: Option<String>,
    db: tauri::State<'_, crate::db::Db>,
    supervisor: tauri::State<'_, Supervisor>,
) -> std::result::Result<StartResult, IpcError> {
    let db = db.inner().clone();
    let supervisor = supervisor.inner().clone();
    let mode = {
        let conn = db.conn();
        crate::engine::effective::resolve(&conn, project_id.as_deref())?.mode
    };
    let project_root: Option<std::path::PathBuf> = match &project_id {
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
                        IpcError::new("E_PATH_NOT_FOUND", format!("项目不存在: {pid}"))
                    }
                    other => IpcError::from(AppError::Db(other)),
                })?;
            Some(std::path::PathBuf::from(path))
        }
        None => None,
    };

    match mode.as_str() {
        "builtin" => {
            let root = project_root
                .clone()
                .ok_or_else(|| IpcError::new("E_INVALID_INPUT", "临时命令需要 cwd 或 project_id"))?;
            let run = launch_builtin(
                &db,
                &supervisor,
                None, // 临时命令：无 command_id（INV-3 允许 None）
                command_line,
                cwd.as_deref(),
                &root,
                false,
            )
            .map_err(IpcError::from)?;
            Ok(StartResult::Builtin { run })
        }
        _ => {
            let root = project_root
                .ok_or_else(|| IpcError::new("E_INVALID_INPUT", "临时命令需要 cwd 或 project_id"))?;
            let pid = tokio::task::spawn_blocking(move || {
                launch_external(&db, None, None, command_line, cwd.as_deref(), &root)
            })
            .await
            .map_err(|e| IpcError::new("E_ENGINE", format!("线程失败: {e}")))?
            .map_err(IpcError::from)?;
            tracing::debug!(target: "launch_record", pid, "external temp spawned");
            Ok(StartResult::External { launched: true })
        }
    }
}

/// `stop_run` — `run_id` → void（单命令整组回收，§5.3.5）。
#[tauri::command]
pub async fn stop_run(
    run_id: String,
    supervisor: tauri::State<'_, Supervisor>,
) -> std::result::Result<(), IpcError> {
    let rid = Uuid::parse_str(&run_id).map_err(|e| IpcError::new("E_INVALID_INPUT", e.to_string()))?;
    supervisor
        .stop(rid)
        .await
        .map_err(IpcError::from)
}

/// `list_active_runs` — → `RunInfo[]`（重启后为空，会话内存）。
#[tauri::command]
pub fn list_active_runs(
    supervisor: tauri::State<'_, Supervisor>,
) -> std::result::Result<Vec<crate::engine::supervisor::RunInfo>, IpcError> {
    Ok(supervisor
        .registry
        .list()
        .iter()
        .map(|h| h.to_info())
        .collect())
}

/// `get_run_log` — `run_id` → `LogLine[]`（面板晚挂载补发环形缓冲快照）。
#[tauri::command]
pub fn get_run_log(
    run_id: String,
    supervisor: tauri::State<'_, Supervisor>,
) -> std::result::Result<(Vec<crate::engine::logbuf::LogLineDto>, u64), IpcError> {
    let rid = Uuid::parse_str(&run_id).map_err(|e| IpcError::new("E_INVALID_INPUT", e.to_string()))?;
    let h = supervisor
        .registry
        .get(rid)
        .ok_or_else(|| IpcError::new("E_INVALID_INPUT", format!("run 不存在: {run_id}")))?;
    let (lines, trimmed) = h.log.lock().unwrap().snapshot_dto();
    Ok((lines, trimmed))
}
