// 临时命令（F3）：内置运行结束 → 提示保存一次（不保存则丢弃）。
// 命名确认由 TempSaveDialog 收集（Tauri 无 window.prompt）。
import { create } from "zustand";

interface TempRunMeta {
  projectId: string | null;
  commandLine: string;
}

export interface TemsStateImpl {
  /** runId → 元数据（运行结束事件时取出） */
  byRun: Record<string, TempRunMeta>;
  /** 待确认保存的请求（对话框数据源） */
  pending: { projectId: string | null; commandLine: string } | null;
  register: (runId: string, meta: TempRunMeta) => void;
  take: (runId: string) => TempRunMeta | null;
  setPending: (p: { projectId: string | null; commandLine: string }) => void;
  clearPending: () => void;
}

export const useTempRunStore = create<TemsStateImpl>((set) => ({
  byRun: {},
  pending: null,
  register: (runId, meta) => set((s) => ({ byRun: { ...s.byRun, [runId]: meta } })),
  take: (runId) => {
    let meta: TempRunMeta | null = null;
    set((s) => {
      meta = s.byRun[runId] ?? null;
      if (!meta) return {};
      const byRun = { ...s.byRun };
      delete byRun[runId];
      return { byRun };
    });
    return meta;
  },
  setPending: (p) => set({ pending: p }),
  clearPending: () => set({ pending: null }),
}));
