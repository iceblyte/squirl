// invoke 封装（契约 §5.4.1）：命令名与出入参以冻结表为准，前端不做二次转换
import { invoke } from "@tauri-apps/api/core";

import type { DiscoveryEntry, ProjectCard, Root, ScanTicket, Settings } from "./types";

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
};
