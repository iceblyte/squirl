// logStore（设计 §5.5.2）：高频 run://log 通道专用（绕过 Query，直达 store）。
// 镜像后端环形缓冲（DR-7）：2MB 字节硬顶近似 + 5000 行软约束，
// 按事件携带的 trimmedLines 对齐裁剪本地镜像；面板卸载不清理（可回看）。
import { create } from "zustand";

import type { LogLineDto } from "@/api/types";

export interface RunLog {
  lines: LogLineDto[];
  /** 后端累计截断行数（事件 trimmedLines 对齐） */
  trimmed: number;
  /** marker 行 seq（profile 折叠分界；null = 尚未出现） */
  markerSeq: number | null;
}

const LINE_CAP = 5000;
const BYTE_CAP = 2 * 1024 * 1024;

function approxBytes(lines: LogLineDto[]): number {
  let n = 0;
  for (const l of lines) n += l.text.length + 16;
  return n;
}

function trim(log: RunLog): RunLog {
  let { lines } = log;
  while (approxBytes(lines) > BYTE_CAP && lines.length > 0) {
    lines = lines.slice(1);
  }
  while (lines.length > LINE_CAP) {
    lines = lines.slice(1);
  }
  // 后端已截断的行按 seq 对齐丢弃（DR-7：环形缓冲头行 seq = trimmed + 1）
  if (log.trimmed > 0) {
    lines = lines.filter((l) => l.seq > log.trimmed);
  }
  return { ...log, lines };
}

interface LogsState {
  logs: Record<string, RunLog>;
  /** 追加一批（run://log 事件；seq 连续性由后端保证） */
  append: (
    runId: string,
    batch: { seq: number; trimmedLines: number; lines: { text: string; stream: string }[] },
  ) => void;
  /** 面板挂载补快照（get_run_log） */
  hydrate: (runId: string, lines: LogLineDto[], trimmed: number) => void;
  clear: (runId: string) => void;
}

export const useLogStore = create<LogsState>((set) => ({
  logs: {},
  append: (runId, batch) =>
    set((s) => {
      const prev: RunLog = s.logs[runId] ?? { lines: [], trimmed: 0, markerSeq: null };
      // seq 对齐：乱序/迟到批次直接丢弃（后端保证有序无缺口）
      const lastSeq = prev.lines.length > 0 ? prev.lines[prev.lines.length - 1].seq : 0;
      if (batch.seq <= lastSeq) return s;
      const markerText = "squirl command output";
      const added: LogLineDto[] = batch.lines.map((l, i) => ({
        seq: batch.seq + i,
        text: l.text,
        stream: l.stream as LogLineDto["stream"],
      }));
      const markerSeq =
        prev.markerSeq ?? added.find((l) => l.text.includes(markerText))?.seq ?? null;
      const next = trim({
        lines: [...prev.lines, ...added],
        trimmed: Math.max(prev.trimmed, batch.trimmedLines),
        markerSeq,
      });
      return { logs: { ...s.logs, [runId]: next } };
    }),
  hydrate: (runId, lines, trimmed) =>
    set((s) => {
      if (s.logs[runId] && s.logs[runId].lines.length > 0) return s;
      const markerText = "squirl command output";
      const markerSeq = lines.find((l) => l.text.includes(markerText))?.seq ?? null;
      return {
        logs: { ...s.logs, [runId]: { lines, trimmed, markerSeq } },
      };
    }),
  clear: (runId) =>
    set((s) => {
      const logs = { ...s.logs };
      delete logs[runId];
      return { logs };
    }),
}));
