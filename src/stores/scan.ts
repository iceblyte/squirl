// scanStore（设计 §5.5.2）：扫描进度/暂停态；亦可并入 uiStore，实现独立便于测试
import { create } from "zustand";

import type { ScanCompleted, ScanPaused, ScanProgress } from "@/api/types";

interface ScanState {
  scanning: boolean;
  progress: ScanProgress | null;
  /** 非空 = 正在等待三选一裁决（scan://paused） */
  paused: ScanPaused | null;
  lastResult: ScanCompleted | null;
  setScanning: (v: boolean) => void;
  setProgress: (p: ScanProgress) => void;
  setPaused: (p: ScanPaused | null) => void;
  setCompleted: (c: ScanCompleted) => void;
}

export const useScanStore = create<ScanState>((set) => ({
  scanning: false,
  progress: null,
  paused: null,
  lastResult: null,
  setScanning: (v) => set({ scanning: v }),
  setProgress: (p) => set({ progress: p, scanning: true }),
  setPaused: (p) => set({ paused: p }),
  setCompleted: (c) => set({ lastResult: c, scanning: false, paused: null, progress: null }),
}));

/** navStore（DR-4）：无路由，视图切换（home/project/settings） */
import { create as zcreate } from "zustand";

interface NavState {
  view: "home" | "project" | "settings";
  activeProjectId: string | null;
  navigate: (v: NavState["view"]) => void;
  openProject: (id: string) => void;
}

export const useNavStore = zcreate<NavState>((set) => ({
  view: "home",
  activeProjectId: null,
  navigate: (v) => set({ view: v, activeProjectId: v === "project" ? undefined : null }),
  openProject: (id) => set({ view: "project", activeProjectId: id }),
}));
