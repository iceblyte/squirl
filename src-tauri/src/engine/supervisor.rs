//! 内置模式进程监管（设计 §5.3.3，INV-3：一切按 run_id 归属）。
//!
//! 状态机（需求 4.3）：idle/starting/running/exited(code)/failed/killed。
//! 进程组：Windows Job Object（KILL_ON_JOB_CLOSE，子进程默认留组、不开
//! breakaway）；Unix `process_group(0)` + `kill(-pgid)`，SIGTERM→3s→SIGKILL。
//! 两条回收路径不得混同（INV-2）：本模块只管内置模式的进程；
//! 外部模式走 engine::spawn（fire-and-forget，不进 Job Object）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use uuid::Uuid;

use crate::engine::adapter::{build_builtin_launch, LaunchJob};
use crate::engine::logbuf::RingBuffer;
use crate::engine::pipeline::{spawn_pipe_with_clock, EventSink, RunStateEvent};
use crate::engine::shell::ShellSpec;
use crate::error::{AppError, Result};

/// 单命令停止后等待进程组消亡的宽限期（仅 Unix 有意义；设计 §5.3.5）。
pub const STOP_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Idle,
    Starting,
    Running,
    /// 退出码 0
    Exited,
    /// 退出码非 0
    Failed,
    /// 手动停止 / 应用退出统一回收
    Killed,
}

impl RunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunState::Idle => "idle",
            RunState::Starting => "starting",
            RunState::Running => "running",
            RunState::Exited => "exited",
            RunState::Failed => "failed",
            RunState::Killed => "killed",
        }
    }
}

/// `RunInfo`（契约 §5.4.1：StartResult.run / list_active_runs 出参）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunInfo {
    pub run_id: String,
    pub command_id: Option<String>,
    pub state: String,
    pub command_line: String,
}

impl RunHandle {
    pub fn to_info(&self) -> RunInfo {
        RunInfo {
            run_id: self.run_id.to_string(),
            command_id: self.command_id.map(|c| c.to_string()),
            state: self.current_state().as_str().to_string(),
            command_line: self.command_line.clone(),
        }
    }
}

/// 启动参数（内置模式）。
#[derive(Debug, Clone)]
pub struct StartArgs {
    pub command_id: Option<Uuid>,
    pub command_line: String,
    pub cwd: std::path::PathBuf,
    pub shell: ShellSpec,
    pub with_marker: bool,
    /// 附加环境变量（M0 供测试注入 HOME 等；生产暂为空）。
    pub env: Vec<(String, String)>,
}

pub struct RunHandle {
    pub run_id: Uuid,
    pub command_id: Option<Uuid>,
    pub command_line: String,
    pub shell: ShellSpec,
    pub cwd: std::path::PathBuf,
    pub state: Mutex<RunState>,
    /// 子进程直接 PID（Shell 本体）。
    pub child_pid: u32,
    /// 手动停止已请求（watcher 以此区分 killed 与 failed/exited）。
    stopping: std::sync::atomic::AtomicBool,
    job: Option<JobGuard>,
    #[cfg(unix)]
    pgid: Option<i32>,
    pub log: Arc<Mutex<RingBuffer>>,
    pub pipe_marker: Arc<Mutex<Option<u64>>>,
    pipe_done: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// 最后输出时刻（Unix 毫秒，DR-8 启发式时钟；由管道聚合器更新）。
    pub last_output_ms: Arc<std::sync::atomic::AtomicU64>,
}

impl RunHandle {
    pub fn is_stopping(&self) -> bool {
        self.stopping.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn current_state(&self) -> RunState {
        self.state.lock().unwrap().clone()
    }
}

/// run 注册表（会话内存，INV-3；重启即空）。
#[derive(Default)]
pub struct Registry {
    runs: Mutex<std::collections::HashMap<Uuid, Arc<RunHandle>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert(&self, h: Arc<RunHandle>) {
        self.runs.lock().unwrap().insert(h.run_id, h);
    }

    pub fn get(&self, run_id: Uuid) -> Option<Arc<RunHandle>> {
        self.runs.lock().unwrap().get(&run_id).cloned()
    }

    pub fn list(&self) -> Vec<Arc<RunHandle>> {
        self.runs.lock().unwrap().values().cloned().collect()
    }

    pub fn remove(&self, run_id: Uuid) {
        self.runs.lock().unwrap().remove(&run_id);
    }

    /// 同 command_id 是否已有非终态实例（重复启动拦截）。
    pub fn active_for_command(&self, command_id: Uuid) -> Option<Uuid> {
        self.runs
            .lock()
            .unwrap()
            .values()
            .find(|h| {
                h.command_id == Some(command_id)
                    && matches!(h.current_state(), RunState::Starting | RunState::Running)
            })
            .map(|h| h.run_id)
    }
}

pub struct Supervisor {
    pub registry: Arc<Registry>,
    pub sink: Arc<dyn EventSink>,
    /// DB 句柄（终态实时写回 DR-16②）；测试可传 None。
    db: Option<crate::db::Db>,
}

impl Clone for Supervisor {
    fn clone(&self) -> Self {
        Self { registry: self.registry.clone(), sink: self.sink.clone(), db: self.db.clone() }
    }
}

impl Supervisor {
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self { registry: Arc::new(Registry::new()), sink, db: None }
    }

    pub fn with_db(sink: Arc<dyn EventSink>, db: crate::db::Db) -> Self {
        Self { registry: Arc::new(Registry::new()), sink, db: Some(db) }
    }

    /// 启动一个内置模式 run。重复启动拦截由上层按 `E_ALREADY_RUNNING` 处理
    /// （契约：StartResult 判别联合，DR-16）。
    pub async fn start(&self, args: StartArgs) -> Result<Uuid> {
        let run_id = Uuid::now_v7();
        self.emit_state(&run_id, args.command_id, RunState::Starting, None);

        let plan = build_builtin_launch(&LaunchJob {
            shell: args.shell.clone(),
            command_line: args.command_line.clone(),
            cwd: args.cwd.clone(),
            keep_open: false,
            with_marker: args.with_marker,
            script_dir: None,
        })?;

        // DR-12 launch record（spawn 前）
        tracing::info!(
            target: "launch_record",
            mode = "builtin",
            run_id = %run_id,
            command_id = ?args.command_id,
            shell = %args.shell.program,
            cwd = %args.cwd.display(),
            program = %plan.program,
            "builtin launch"
        );

        let mut cmd = tokio::process::Command::new(&plan.program);
        for a in &plan.args {
            #[cfg(windows)]
            if a.raw {
                cmd.raw_arg(&a.text);
            } else {
                cmd.arg(&a.text);
            }
            #[cfg(not(windows))]
            cmd.arg(&a.text);
        }
        cmd.current_dir(&plan.cwd);
        for (k, v) in &args.env {
            cmd.env(k, v);
        }
        // 无交互（F4）：stdin 直接关闭，交互命令读 EOF 自行报错退出
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        // CREATE_SUSPENDED：入 Job Object 前不给它派生进程的窗口
        cmd.creation_flags(crate::engine::adapter::flags::CREATE_NO_WINDOW
            | crate::engine::adapter::flags::CREATE_SUSPENDED);

        #[cfg(unix)]
        {
            // 独立进程组（setsid 语义）
            cmd.process_group(0);
            // 崩溃兜底路径②：仅直接子进程（已知限制：孙进程可能遗留）
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    let r = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) };
                    if r != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::LaunchFailed(format!("内置模式 spawn 失败: {e}")))?;
        let child_pid = child
            .id()
            .ok_or_else(|| AppError::Engine("未取得子进程 PID".into()))?;

        let job = self.setup_job(&child)?;
        #[cfg(unix)]
        let pgid = get_pgid(child_pid);

        let ring = Arc::new(Mutex::new(RingBuffer::new()));
        let output_clock = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let pipe_clock = output_clock.clone();
        let pipe = spawn_pipe_with_clock(
            run_id.to_string(),
            child.stdout.take().ok_or_else(|| AppError::Engine("stdout 未接管".into()))?,
            child.stderr.take().ok_or_else(|| AppError::Engine("stderr 未接管".into()))?,
            ring.clone(),
            self.sink.clone(),
            Some(pipe_clock),
        )?;

        let handle = Arc::new(RunHandle {
            run_id,
            command_id: args.command_id,
            command_line: args.command_line.clone(),
            shell: args.shell.clone(),
            cwd: args.cwd.clone(),
            state: Mutex::new(RunState::Running),
            child_pid,
            stopping: std::sync::atomic::AtomicBool::new(false),
            job,
            #[cfg(unix)]
            pgid,
            log: ring,
            // 聚合任务写入的 marker 槽（spawn_pipe 内部 Arc，两者必须同一实例）
            pipe_marker: pipe.marker,
            pipe_done: Mutex::new(Some(pipe.done)),
            last_output_ms: output_clock.clone(),
        });
        self.registry.insert(handle.clone());

        // watcher：进程退出 → 终态（killed 优先）
        let reg = self.registry.clone();
        let reg_terminal = self.registry.clone();
        let sink = self.sink.clone();
        let sup_db = self.db.clone();
        let sup_sink = self.sink.clone();
        let watch_run_id = run_id;

        // DR-8：running 且 30s 无输出 → run://input-suspected（只提示一次，不干预）
        tokio::spawn(async move {
            const THRESHOLD_MS: u64 = 30_000;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let Some(h) = reg.get(watch_run_id) else { break };
                if !matches!(h.current_state(), RunState::Running) {
                    break;
                }
                let last = h.last_output_ms.load(std::sync::atomic::Ordering::SeqCst);
                let idle = crate::clock::now_unix_millis().saturating_sub(last);
                if idle >= THRESHOLD_MS {
                    sup_sink.on_input_suspected(watch_run_id.to_string());
                    break;
                }
            }
        });
        tokio::spawn(async move {
            let reg = reg_terminal;
            let status = child.wait().await;
            let code = status.ok().and_then(|s| s.code());
            if let Some(h) = reg.get(run_id) {
                let killed_by_stop = h.is_stopping();
                let state = if killed_by_stop {
                    RunState::Killed
                } else {
                    match code {
                        Some(0) => RunState::Exited,
                        Some(_) => RunState::Failed,
                        // 信号终止（Unix）按失败处理
                        None => RunState::Failed,
                    }
                };
                *h.state.lock().unwrap() = state.clone();
                sink.on_state(RunStateEvent {
                    run_id: run_id.to_string(),
                    command_id: h.command_id.map(|c| c.to_string()),
                    state: state.as_str().into(),
                    exit_code: if matches!(state, RunState::Killed) { None } else { code },
                    ts: now_ts(),
                });
                // 终态实时写回（DR-16②；临时命令无行跳过）
                if let Some(db) = &sup_db {
                    let mut conn = db.conn();
                    let _ = crate::db::repo::record_builtin_terminal(
                        &mut conn,
                        h.command_id,
                        state.as_str(),
                        code,
                    );
                }
                // 终发日志管道（读端已关）；guard 不得跨 await
                let done = h.pipe_done.lock().unwrap().take();
                if let Some(done) = done {
                    let _ = done.await;
                }
            }
        });

        self.emit_state(&run_id, args.command_id, RunState::Running, None);
        Ok(run_id)
    }

    #[cfg(windows)]
    fn setup_job(&self, child: &tokio::process::Child) -> Result<Option<JobGuard>> {
        let guard = JobGuard::new()?;
        let raw = child
            .raw_handle()
            .ok_or_else(|| AppError::Engine("未取得子进程句柄".into()))?;
        guard.assign(windows::Win32::Foundation::HANDLE(raw))?;
        resume_suspended(child_pid_of(child))?;
        Ok(Some(guard))
    }

    #[cfg(not(windows))]
    fn setup_job(&self, _child: &tokio::process::Child) -> Result<Option<JobGuard>> {
        Ok(None)
    }

    /// 手动停止（整组回收，§5.3.5）：不影响其他 run。
    pub async fn stop(&self, run_id: Uuid) -> Result<()> {
        let h = self
            .registry
            .get(run_id)
            .ok_or_else(|| AppError::InvalidInput(format!("run 不存在: {run_id}")))?;
        if h.is_stopping() {
            return Ok(());
        }
        h.stopping
            .store(true, std::sync::atomic::Ordering::SeqCst);

        // reap record（DR-12 可观测性三类记录之二）
        let t0 = std::time::Instant::now();
        let outcome = self.terminate_group(&h).await;
        tracing::info!(
            target: "reap_record",
            run_id = %run_id,
            pid = h.child_pid,
            elapsed_ms = t0.elapsed().as_millis() as u64,
            result = ?outcome,
            "stop_run"
        );
        outcome
    }

    async fn terminate_group(&self, h: &Arc<RunHandle>) -> Result<()> {
        #[cfg(windows)]
        if let Some(job) = &h.job {
            job.terminate()?;
            // TerminateJobObject 即时生效；等 watcher 置 killed
            return Ok(());
        }
        #[cfg(unix)]
        if let Some(pgid) = h.pgid {
            // SIGTERM → 宽限 → SIGKILL（整组）
            unsafe {
                libc::kill(-(pgid as i32), libc::SIGTERM);
            }
            let deadline = tokio::time::Instant::now() + STOP_GRACE;
            while tokio::time::Instant::now() < deadline {
                let alive = unsafe { libc::kill(-(pgid as i32), 0) == 0 };
                if !alive {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            unsafe {
                libc::kill(-(pgid as i32), libc::SIGKILL);
            }
            return Ok(());
        }
        let _ = h;
        Err(AppError::Engine("无可用的进程组句柄".into()))
    }

    fn emit_state(
        &self,
        run_id: &Uuid,
        command_id: Option<Uuid>,
        state: RunState,
        exit_code: Option<i32>,
    ) {
        self.sink.on_state(RunStateEvent {
            run_id: run_id.to_string(),
            command_id: command_id.map(|c| c.to_string()),
            state: state.as_str().into(),
            exit_code,
            ts: now_ts(),
        });
    }
}

fn now_ts() -> String {
    // MVP 不引入 chrono：诊断时间戳用 UUID v7 前缀毫秒即可；
    // UI 展示时间由前端本地生成
    Uuid::now_v7().to_string()
}

// ---------- Windows Job Object ----------

/// Windows Job Object 守卫（设计 §5.3.3）：
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` —— 句柄关闭（含宿主进程死亡）
/// 即内核收树；RAII Drop 关句柄。手动停止走 `terminate()` 显式终止。
#[cfg(windows)]
pub struct JobGuard {
    handle: SendHandle,
}

/// 裸句柄的所有权包装：句柄仅用于内核 API 调用，跨线程移动安全。
#[cfg(windows)]
struct SendHandle(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for SendHandle {}
#[cfg(windows)]
unsafe impl Sync for SendHandle {}

#[cfg(windows)]
impl JobGuard {
    pub fn new() -> Result<Self> {
        use windows::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        unsafe {
            let handle = CreateJobObjectW(None, windows::core::PCWSTR::null())
                .map_err(|e| AppError::Engine(format!("CreateJobObjectW 失败: {e}")))?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| AppError::Engine(format!("SetInformationJobObject 失败: {e}")))?;
            Ok(JobGuard { handle: SendHandle(handle) })
        }
    }

    /// 把子进程纳入作业（CREATE_SUSPENDED 窗口期内调用）。
    pub fn assign(&self, child_handle: windows::Win32::Foundation::HANDLE) -> Result<()> {
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;
        unsafe {
            AssignProcessToJobObject(self.handle.0, child_handle)
                .map_err(|e| AppError::Engine(format!("AssignProcessToJobObject 失败: {e}")))
        }
    }

    /// 显式整组终止（手动停止路径①）。
    pub fn terminate(&self) -> Result<()> {
        use windows::Win32::System::JobObjects::TerminateJobObject;
        unsafe {
            TerminateJobObject(self.handle.0, 1)
                .map_err(|e| AppError::Engine(format!("TerminateJobObject 失败: {e}")))
        }
    }

    /// 复制进程句柄（B5 崩溃兜底测试观察用）。
    pub fn raw(&self) -> windows::Win32::Foundation::HANDLE {
        self.handle.0
    }
}

#[cfg(windows)]
impl Drop for JobGuard {
    fn drop(&mut self) {
        // 不主动 Terminate：Drop = 句柄关闭 = KILL_ON_JOB_CLOSE 内核收树。
        // 这正是「异常崩溃兜底」路径②的机制（宿主死亡 → 句柄全关 → 收树）。
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle.0);
        }
    }
}

/// 恢复 CREATE_SUSPENDED 启动的进程（Toolhelp 找主线程 → ResumeThread）。
/// 公开给 B5 崩溃兜底模拟宿主（examples/job_host）复用同一套挂起-入组-恢复时序。
#[cfg(windows)]
pub fn resume_suspended(pid: u32) -> Result<()> {
    use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_FILES};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::Threading::{
        OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
    };
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
            .map_err(|e| AppError::Engine(format!("CreateToolhelp32Snapshot 失败: {e}")))?;
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut resumed = false;
        if Thread32First(snap, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid
                    && let Ok(thread) = OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID)
                {
                    ResumeThread(thread);
                    let _ = CloseHandle(thread);
                    resumed = true;
                    break; // 主线程（先创建的线程）即可
                }
                if Thread32Next(snap, &mut entry) == Err(ERROR_NO_MORE_FILES.into()) {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        if !resumed {
            return Err(AppError::Engine(format!("未找到进程 {pid} 的线程以恢复执行")));
        }
        Ok(())
    }
}

#[cfg(windows)]
fn child_pid_of(child: &tokio::process::Child) -> u32 {
    child.id().unwrap_or(0)
}

#[cfg(unix)]
fn get_pgid(pid: u32) -> Option<i32> {
    // 子进程以 process_group(0) 启动 → pgid == pid
    let pgid = unsafe { libc::getpgid(pid as i32) };
    if pgid > 0 { Some(pgid) } else { None }
}
