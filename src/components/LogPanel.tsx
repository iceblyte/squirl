import { useVirtualizer } from "@tanstack/react-virtual";
import Anser from "anser";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { useLogStore } from "@/stores/logs";

/** ANSI → 渲染片段（anser：纯函数，无 xterm 依赖——设计 §4.3） */
function ansiToSpans(text: string): Anser.AnserJsonEntry[] {
  return Anser.ansiToJson(text, { use_classes: true });
}

/**
 * LogPanel（F5，设计 §5.5.1）：虚拟滚动 + ANSI 渲染 + profile marker 折叠
 * + 自动滚动开关。数据源 logStore（run://log 事件 + get_run_log 快照补发）。
 */
export function LogPanel({ runId }: { runId: string }) {
  const { t } = useTranslation("project");
  const log = useLogStore((s) => s.logs[runId]);
  const hydrate = useLogStore((s) => s.hydrate);
  const parentRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const [showProfile, setShowProfile] = useState(false);

  // 面板挂载补发快照（契约：get_run_log；已有时跳过）
  useEffect(() => {
    void import("@/api/client").then(({ api }) =>
      api
        .getRunLog(runId)
        .then((tuple) => hydrate(runId, tuple[0], tuple[1]))
        .catch(() => {}),
    );
  }, [runId, hydrate]);

  const lines = log?.lines ?? [];
  const markerSeq = log?.markerSeq ?? null;

  // marker 之前 = profile 输出（可折叠）；marker 之后 = 命令输出
  const profileCount = useMemo(
    () => (markerSeq === null ? 0 : lines.filter((l) => l.seq < markerSeq).length),
    [lines, markerSeq],
  );
  const visible = useMemo(
    () => (markerSeq === null || showProfile ? lines : lines.filter((l) => l.seq >= markerSeq)),
    [lines, markerSeq, showProfile],
  );

  const virtualizer = useVirtualizer({
    count: visible.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 20,
    overscan: 30,
  });

  // 自动滚动到底（新批次到达时；visible 由 log 派生，依赖 log 即可）
  const lastSeq = log?.lines.at(-1)?.seq;
  useEffect(() => {
    if (autoScroll && parentRef.current && lastSeq !== undefined) {
      parentRef.current.scrollTop = parentRef.current.scrollHeight;
    }
  }, [lastSeq, autoScroll]);

  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">{t("log")}</h2>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Badge variant="secondary">{visible.length} 行</Badge>
          {log && log.trimmed > 0 && <Badge variant="outline">已截断 {log.trimmed} 行</Badge>}
          {profileCount > 0 && (
            <button
              type="button"
              className="underline-offset-2 hover:underline"
              onClick={() => setShowProfile((v) => !v)}
            >
              {showProfile
                ? t("hideProfile", { defaultValue: "收起 profile 输出" })
                : t("showProfile", { defaultValue: `展开 profile 输出（${profileCount} 行）` })}
            </button>
          )}
          <label className="flex items-center gap-1">
            <input
              type="checkbox"
              checked={autoScroll}
              onChange={(e) => setAutoScroll(e.target.checked)}
            />
            {t("autoScroll", { defaultValue: "自动滚动" })}
          </label>
        </div>
      </div>
      <div
        ref={parentRef}
        className="h-72 overflow-auto rounded-md border bg-neutral-950 p-2 font-mono text-xs leading-5 text-neutral-200"
      >
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((v) => {
            const line = visible[v.index];
            const spans = ansiToSpans(line.text);
            return (
              <div
                key={line.seq}
                style={{
                  position: "absolute",
                  top: v.start,
                  left: 0,
                  width: "100%",
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-all",
                }}
              >
                {spans.length === 0 ? (
                  <>&nbsp;</>
                ) : (
                  spans.map((sp: Anser.AnserJsonEntry) => (
                    <span
                      key={`${line.seq}:${sp.fg}:${sp.bg}:${sp.content}`}
                      className={sp.fg ? `${sp.fg}-fg ${sp.bg ? `${sp.bg}-bg` : ""}` : undefined}
                    >
                      {sp.content}
                    </span>
                  ))
                )}
              </div>
            );
          })}
        </div>
        {visible.length === 0 && (
          <div className="p-4 text-center text-neutral-500">
            {t("logEmpty", { defaultValue: "暂无输出" })}
          </div>
        )}
      </div>
    </div>
  );
}
