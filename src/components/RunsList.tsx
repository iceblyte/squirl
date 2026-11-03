import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useRunStore } from "@/stores/runs";

/** DR-23：多会话管理（运行中列表 + 行内停止；引擎并行能力已存在） */
export function RunsList() {
  const { t } = useTranslation("home");
  const queryClient = useQueryClient();
  const runs = useRunStore((s) => s.runs);
  const active = Object.values(runs);

  const stop = useMutation({
    mutationFn: (runId: string) => api.stopRun(runId),
    onError: (e) => toast.error(String(e)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["projects"] }),
  });

  if (active.length === 0) return null;

  return (
    <section aria-label="active-runs" className="space-y-1.5">
      <h2 className="text-sm font-medium text-muted-foreground">
        {t("activeRuns", { defaultValue: "运行中" })}
        <Badge variant="default" className="ml-2">
          {active.length}
        </Badge>
      </h2>
      <ul className="space-y-1">
        {active.map((r) => (
          <li
            key={r.runId}
            className="flex items-center justify-between rounded-md border bg-card px-3 py-2 text-sm"
          >
            <code className="truncate text-xs" title={r.commandLine}>
              {r.commandLine}
            </code>
            <Button
              size="sm"
              variant="ghost"
              disabled={stop.isPending}
              onClick={() => stop.mutate(r.runId)}
            >
              <Square className="size-3.5 text-destructive" />
              {t("stop", { ns: "common", defaultValue: "停止" })}
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}
