// F2 手动注册（拖拽半边）：拖入文件夹 → 登记（Toast 反馈成败）

import { useQueryClient } from "@tanstack/react-query";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { toast } from "sonner";

import { api } from "@/api/client";

export function useDragDropRegister() {
  const queryClient = useQueryClient();

  useEffect(() => {
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
