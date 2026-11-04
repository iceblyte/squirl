// IPC 类型（与设计 §5.4 冻结契约逐字段一致；Rust DTO camelCase 序列化）

export interface Settings {
  defaultMode: "external" | "builtin";
  defaultEmulator: string | null;
  defaultShell: string | null;
  loadShellProfile: boolean;
  scanDepth: number;
  /** DR-19：托盘开关（改后重启生效） */
  trayEnabled: boolean;
  /** DR-21：自定义排除名单 */
  customExcludes: string[] | null;
}

export interface Root {
  id: string;
  path: string;
  enabled: boolean;
  /** DR-21：每根独立深度；null = 跟随全局 */
  scanDepth: number | null;
}

export interface DiscoveryEntry {
  rootId: string;
  path: string;
  relPath: string;
  name: string;
  flags: string[];
}

export interface ProjectCard {
  id: string;
  path: string;
  name: string;
  note: string;
  rootId: string | null;
  pathMissing: boolean;
  lastRunAt: string | null;
  runCount: number;
  /** ADR-12：卡片快启（G3）数据源 */
  commands: CommandRow[];
}

/** StartResult 判别联合（DR-16③） */
export type StartResult =
  | {
      mode: "builtin";
      run: { runId: string; commandId: string | null; state: string; commandLine: string };
    }
  | { mode: "external"; launched: boolean };

/** get_effective_execution_config 出参 */
export interface EffectiveExecConfig {
  mode: "external" | "builtin";
  shell: string;
  shellPath: string;
  emulator: string | null;
  warnings: string[];
  supported: boolean;
  loadProfile: boolean;
}

export interface ScanTicket {
  depth: number;
  limit: number;
}

/** INV-5：IPC 错误结构 { code, message, detail?, data? } */
export interface IpcErrorShape {
  code: string;
  message: string;
  detail?: string;
  data?: unknown;
}

/** scan://progress 载荷 */
export interface ScanProgress {
  rootsDone: number;
  rootsTotal: number;
  entries: number;
}

/** scan://paused 载荷 */
export interface ScanPaused {
  entries: number;
  limit: number;
}

/** scan://completed 载荷 */
export interface ScanCompleted {
  count: number;
  durationMs: number;
  error?: string;
}

/** app://db-restored 载荷（AC9 toast） */
export interface DbRestored {
  kind: "restored_from_backup" | "rebuilt_empty";
}

export interface CommandRow {
  id: string;
  /** DR-20：null = 全局命令 */
  projectId: string | null;
  name: string;
  commandLine: string;
  cwd: string | null;
  lastRunAt: string | null;
  lastExitCode: number | null;
  /** DR-24：锁文件指纹感知开关 */
  fingerprint: boolean;
  /** DR-27：工具用途备忘 */
  note: string;
}

export interface ProjectDetail extends ProjectCard {
  commands: CommandRow[];
}

export interface RunInfo {
  runId: string;
  commandId: string | null;
  state: "idle" | "starting" | "running" | "exited" | "failed" | "killed";
  commandLine: string;
}

/** run://state 事件载荷（§5.4.2） */
export interface RunStateEvent {
  runId: string;
  commandId: string | null;
  state: string;
  exitCode?: number | null;
  ts: string;
}

/** get_run_log 出参行（§5.4.1 LogLine = {seq, text, stream}） */
export interface LogLineDto {
  seq: number;
  text: string;
  stream: "stdout" | "stderr";
}

/** run://log 事件载荷（§5.4.2，DR-7：seq 对齐 + trimmed_lines 裁剪） */
export interface RunLogEvent {
  runId: string;
  seq: number;
  trimmedLines: number;
  lines: { text: string; stream: "stdout" | "stderr" }[];
}

/** DR-26：配置文件读取载荷（read_project_config 出参） */
export interface ConfigFilePayload {
  dialect: "env" | "toml" | "yaml";
  keys: { key: string; value: string; line: number }[];
  content: string;
}

/** DR-29：项目 Git 元数据 */
export interface GitMeta {
  branch: string | null;
  commitHash: string;
  remoteUrl: string | null;
  lastCommitTime: string | null;
}
