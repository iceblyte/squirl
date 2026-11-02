import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api/client";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useScanStore } from "@/stores/scan";

/** 500 条暂停三选一对话框（R6：继续 / 降低深度 / 取消） */
export function ScanPauseDialog() {
  const queryClient = useQueryClient();
  const paused = useScanStore((s) => s.paused);
  const setPaused = useScanStore((s) => s.setPaused);

  const resolve = useMutation({
    mutationFn: (decision: "continue" | "reduce_depth" | "cancel") =>
      api.resolveScanPause(decision),
    onSettled: () => {
      setPaused(null);
      void queryClient.invalidateQueries({ queryKey: ["discovery"] });
    },
  });

  return (
    <Dialog open={paused !== null}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>发现条目已达上限</DialogTitle>
          <DialogDescription>
            {paused
              ? `本次扫描已收集 ${paused.entries} 条（上限 ${paused.limit}）。继续全量扫描、降低剩余深度，还是取消？`
              : ""}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            variant="ghost"
            disabled={resolve.isPending}
            onClick={() => resolve.mutate("cancel")}
          >
            取消
          </Button>
          <Button
            variant="outline"
            disabled={resolve.isPending}
            onClick={() => resolve.mutate("reduce_depth")}
          >
            降低深度
          </Button>
          <Button disabled={resolve.isPending} onClick={() => resolve.mutate("continue")}>
            继续扫描
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
