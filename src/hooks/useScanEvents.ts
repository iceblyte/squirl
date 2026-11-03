// scan://* 与 app://db-restored 事件订阅（契约 §5.4.2）
// 挂载一次（App 生命周期），写入 scanStore / Query 失效

import { useQueryClient } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import type {
  DbRestored,
  RunLogEvent,
  RunStateEvent,
  ScanCompleted,
  ScanPaused,
  ScanProgress,
} from "@/api/types";
import { useLogStore } from "@/stores/logs";
import { useRunStore } from "@/stores/runs";
import { useScanStore } from "@/stores/scan";
import { useTempRunStore } from "@/stores/temps";

export function useScanEvents() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const { setProgress, setPaused, setCompleted } = useScanStore.getState();

  useEffect(() => {
    const unlistens: UnlistenFn[] = [];
    // 非 Tauri 环境（单测 jsdom）下 listen 会 reject：兜底为空订阅
    const subscribe = async <T>(
      event: string,
      handler: (payload: T) => void,
    ): Promise<UnlistenFn> => listen<T>(event, (e) => handler(e.payload)).catch(() => () => {});

    void subscribe<ScanProgress>("scan://progress", setProgress).then((u) => unlistens.push(u));
    void subscribe<ScanPaused>("scan://paused", setPaused).then((u) => unlistens.push(u));
    void subscribe<ScanCompleted>("scan://completed", (payload) => {
      setCompleted(payload);
      // 完成后失效发现区与台账（设计 §5.4.2 消费方约定）
      void queryClient.invalidateQueries({ queryKey: ["discovery"] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    }).then((u) => unlistens.push(u));
    void subscribe<DbRestored>("app://db-restored", (payload) => {
      // AC9：恢复/重建提示 toast
      toast.success(payload.kind === "rebuilt_empty" ? t("errorDbRebuilt") : t("errorDbRestored"));
    }).then((u) => unlistens.push(u));

    // S7：run://log 直达 logStore（绕过 Query，30ms 批）
    void subscribe<RunLogEvent>("run://log", (payload) => {
      useLogStore.getState().append(payload.runId, {
        seq: payload.seq,
        trimmedLines: payload.trimmedLines,
        lines: payload.lines,
      });
    }).then((u) => unlistens.push(u));

    // S6/S8：run://state → runStore；终态处理（AC5 / F3 临时保存提示）
    void subscribe<RunStateEvent>("run://state", (payload) => {
      const runStore = useRunStore.getState();
      if (["exited", "failed", "killed"].includes(payload.state)) {
        runStore.remove(payload.runId);
        // AC5：非零退出 ≤5s 可读错误（事件即时到达）；原始输出保留在 LogPanel
        if (payload.state === "failed") {
          toast.error(t("commandFailed", { ns: "common", defaultValue: "命令失败" }), {
            description: t("exitCode", {
              ns: "common",
              defaultValue: `退出码 ${payload.exitCode ?? "?"}（原始输出见日志面板）`,
            }),
          });
        }
        // F3：内置临时命令结束 → 提示保存一次（不保存则丢弃）
        if (payload.commandId === null) {
          const temp = useTempRunStore.getState().take(payload.runId);
          if (temp) {
            toast.info(
              t("tempSavePrompt", { ns: "project", defaultValue: "本次命令未保存，是否保存？" }),
              {
                duration: 15_000,
                action: {
                  label: t("saveTemp", { ns: "project", defaultValue: "保存" }),
                  onClick: () =>
                    useTempRunStore
                      .getState()
                      .setPending({ projectId: temp.projectId, commandLine: temp.commandLine }),
                },
              },
            );
          }
        }
      } else {
        runStore.upsert({
          runId: payload.runId,
          commandId: payload.commandId,
          state: "running",
          commandLine: "",
        });
      }
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    }).then((u) => unlistens.push(u));

    // DR-8：疑似等待输入（只提示 + 可强制停止）
    void subscribe<{ runId: string }>("run://input-suspected", (payload) => {
      toast.warning(
        t("inputSuspected", {
          ns: "project",
          defaultValue: "命令可能在等待输入（MVP 不支持交互式命令）",
        }),
        {
          duration: 20_000,
          action: {
            label: t("stop", { ns: "common", defaultValue: "停止" }),
            onClick: () => void api.stopRun(payload.runId),
          },
        },
      );
    }).then((u) => unlistens.push(u));

    return () => {
      for (const u of unlistens) u();
    };
    // t 变化重挂载无意义（文案惰性求值即可）；biome 精确依赖集
  }, [queryClient, setProgress, setPaused, setCompleted, t]);
}
