// F2 手动注册（拖拽半边）：拖入文件夹 → 登记（Toast 反馈成败）
// 非 Tauri 环境（浏览器测试）安全降级为 no-op。

import { useQueryClient } from "@tanstack/react-query";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { toast } from "sonner";

import { api } from "@/api/client";

/** Tauri 运行时是否可用（浏览器/SSR 场景为 false）。 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function useDragDropRegister() {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!isTauri()) return; // 非 Tauri 环境安全跳过
    const unlisten = getCurrentWebviewWindow().onDragDropEvent(async (event) => {
      if (event.payload.type !== "drop") return;
      let ok = 0;
      let failed = 0;
      for (const path of event.payload.paths) {
        try {
          await api.registerProject(path);
          ok += 1;
        } catch {
          failed += 1;
        }
      }
      if (ok > 0) {
        toast.success(`已登记 ${ok} 个项目`);
        void queryClient.invalidateQueries({ queryKey: ["projects"] });
        void queryClient.invalidateQueries({ queryKey: ["discovery"] });
      }
      if (failed > 0) {
        toast.error(`${failed} 个路径登记失败（重复或非文件夹）`);
      }
    });
    return () => {
      void unlisten.then((u) => u());
    };
  }, [queryClient]);
}
