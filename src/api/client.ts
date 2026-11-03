// invoke 封装（契约 §5.4.1）：命令名与出入参以冻结表为准，前端不做二次转换
import { invoke } from "@tauri-apps/api/core";

import type {
  CommandRow,
  ConfigFilePayload,
  DiscoveryEntry,
  EffectiveExecConfig,
  LogLineDto,
  ProjectCard,
  ProjectDetail,
  Root,
  RunInfo,
  ScanTicket,
  Settings,
  StartResult,
} from "./types";

export const api = {
  // settings
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (patch: Settings) => invoke<Settings>("update_settings", { patch }),

  // roots
  listRoots: () => invoke<Root[]>("list_roots"),
  addRoot: (path: string) => invoke<Root>("add_root", { path }),
  removeRoot: (id: string) => invoke<void>("remove_root", { id }),
  setRootEnabled: (id: string, enabled: boolean) =>
    invoke<void>("set_root_enabled", { id, enabled }),

  // scan
  startScan: () => invoke<ScanTicket>("start_scan"),
  resolveScanPause: (decision: "continue" | "reduce_depth" | "cancel") =>
    invoke<void>("resolve_scan_pause", { decision }),
  getDiscovery: () => invoke<DiscoveryEntry[]>("get_discovery"),

  // projects
  registerProject: (path: string, note?: string) =>
    invoke<ProjectCard>("register_project", { path, note }),
  listProjects: () => invoke<ProjectCard[]>("list_projects"),
  searchProjects: (query: string) => invoke<ProjectCard[]>("search_projects", { query }),
  getProject: (id: string) => invoke<ProjectDetail>("get_project", { id }),
  updateProject: (id: string, patch: { note?: string }) =>
    invoke<ProjectCard>("update_project", { id, patch }),
  relocateProject: (id: string, newPath: string) =>
    invoke<ProjectCard>("relocate_project", { id, newPath }),
  deleteProject: (id: string) => invoke<void>("delete_project", { id }),
  getProjectCommandCount: (id: string) => invoke<number>("get_project_command_count", { id }),

  // commands（S4）
  addCommand: (
    projectId: string | null,
    name: string,
    commandLine: string,
    cwd?: string,
    fingerprint?: boolean,
  ) =>
    invoke<CommandRow>("add_command", {
      projectId,
      name,
      commandLine,
      cwd,
      fingerprint,
    }),
  listGlobalCommands: () => invoke<CommandRow[]>("list_global_commands"),
  setRootDepth: (id: string, depth: number | null) => invoke<void>("set_root_depth", { id, depth }),
  listAvailableIdes: () => invoke<string[]>("list_available_ides"),
  openInIde: (path: string, ide: string) => invoke<void>("open_in_ide", { path, ide }),
  updateCommand: (id: string, patch: { name: string; commandLine: string; cwd?: string }) =>
    invoke<CommandRow>("update_command", { id, patch }),
  deleteCommand: (id: string) => invoke<void>("delete_command", { id }),

  // run（S4）
  startCommand: (commandId: string, forceNew?: boolean) =>
    invoke<StartResult>("start_command", { commandId, forceNew }),
  startTempCommand: (args: { projectId?: string; commandLine: string; cwd?: string }) =>
    invoke<StartResult>("start_temp_command", args),
  getEffectiveExecutionConfig: (projectId?: string) =>
    invoke<EffectiveExecConfig>("get_effective_execution_config", { projectId }),
  readProjectConfig: (path: string) => invoke<ConfigFilePayload>("read_project_config", { path }),
  applyProjectConfig: (path: string, values: { key: string; value: string }[]) =>
    invoke<boolean>("apply_project_config", { path, values }),
  scanReadmeCommands: (path: string) => invoke<string[]>("scan_readme_commands", { path }),
  stopRun: (runId: string) => invoke<void>("stop_run", { runId }),
  listActiveRuns: () => invoke<RunInfo[]>("list_active_runs"),
  getRunLog: (runId: string) => invoke<{ 0: LogLineDto[]; 1: number }>("get_run_log", { runId }),
};
