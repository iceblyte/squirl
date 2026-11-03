// scan://* 与 app://db-restored 事件订阅（契约 §5.4.2）
// 挂载一次（App 生命周期），写入 scanStore / Query 失效

import { useQueryClient } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import type {
  DbRestored,
  RunStateEvent,
  ScanCompleted,
  ScanPaused,
  ScanProgress,
} from "@/api/types";
import { useRunStore } from "@/stores/runs";
import { useScanStore } from "@/stores/scan";

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

    // S6：run://state → runStore（状态点/停止按钮数据源；终态移除实例）
    void subscribe<RunStateEvent>("run://state", (payload) => {
      const runStore = useRunStore.getState();
      if (["exited", "failed", "killed"].includes(payload.state)) {
        runStore.remove(payload.runId);
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

    return () => {
      for (const u of unlistens) u();
    };
    // t 变化重挂载无意义（文案惰性求值即可）；biome 精确依赖集
    // eslint-disable-next-line -- biome: t / setX 为稳定引用或惰性使用
  }, [queryClient, setProgress, setPaused, setCompleted, t]);
}
