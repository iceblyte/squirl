// scan://* 与 app://db-restored 事件订阅（契约 §5.4.2）
// 挂载一次（App 生命周期），写入 scanStore / Query 失效

import { useQueryClient } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";

import type { DbRestored, ScanCompleted, ScanPaused, ScanProgress } from "@/api/types";
import { useScanStore } from "@/stores/scan";

export function useScanEvents() {
  const queryClient = useQueryClient();
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
      // AC9：恢复/重建提示（toast 组件在 S3 引入，先挂占位）
      console.warn("db-restored:", payload.kind);
    }).then((u) => unlistens.push(u));

    return () => {
      for (const u of unlistens) u();
    };
  }, [queryClient, setProgress, setPaused, setCompleted]);
}
