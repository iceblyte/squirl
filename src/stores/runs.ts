// runStore（S6）：内置模式运行实例状态（run://state 事件增量维护 + 启动时快照）
import { create } from "zustand";

import type { RunInfo } from "@/api/types";

interface RunState {
  /** runId → 运行实例摘要 */
  runs: Record<string, RunInfo>;
  /** 命令 → runId（重复启动拦截判断用） */
  byCommand: Record<string, string>;
  setSnapshot: (runs: RunInfo[]) => void;
  upsert: (info: RunInfo) => void;
  /** 终态：移除实例（state 事件仍会推送终态给日志面板） */
  remove: (runId: string) => void;
}

export const useRunStore = create<RunState>((set) => ({
  runs: {},
  byCommand: {},
  setSnapshot: (list) =>
    set(() => {
      const runs: Record<string, RunInfo> = {};
      const byCommand: Record<string, string> = {};
      for (const r of list) {
        runs[r.runId] = r;
        if (r.commandId) byCommand[r.commandId] = r.runId;
      }
      return { runs, byCommand };
    }),
  upsert: (info) =>
    set((s) => ({
      runs: { ...s.runs, [info.runId]: info },
      byCommand: info.commandId ? { ...s.byCommand, [info.commandId]: info.runId } : s.byCommand,
    })),
  remove: (runId) =>
    set((s) => {
      const runs = { ...s.runs };
      const byCommand = { ...s.byCommand };
      const info = runs[runId];
      delete runs[runId];
      if (info?.commandId) delete byCommand[info.commandId];
      return { runs, byCommand };
    }),
}));
