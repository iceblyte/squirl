// IPC 类型（与设计 §5.4 冻结契约逐字段一致；Rust DTO camelCase 序列化）

export interface Settings {
  defaultMode: "external" | "builtin";
  defaultEmulator: string | null;
  defaultShell: string | null;
  loadShellProfile: boolean;
  scanDepth: number;
}

export interface Root {
  id: string;
  path: string;
  enabled: boolean;
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
