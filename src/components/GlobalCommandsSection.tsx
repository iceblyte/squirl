import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Play, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import type { CommandRow } from "@/api/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useRunStore } from "@/stores/runs";

/** DR-20：全局命令分区（不绑项目；cwd 默认主目录；同一运行域） */
export function GlobalCommandsSection() {
  const { t: tc } = useTranslation("common");
  const { t } = useTranslation("home");
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [commandLine, setCommandLine] = useState("");
  const runs = useRunStore((s) => s.byCommand);

  const list = useQuery({ queryKey: ["globalCommands"], queryFn: api.listGlobalCommands });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["globalCommands"] });
    void queryClient.invalidateQueries({ queryKey: ["projects"] });
  };

  const save = useMutation({
    mutationFn: () => api.addCommand(null, name.trim() || tc("run") + Date.now(), commandLine),
    onSuccess: () => {
      toast.success(t("globalSaved", { defaultValue: "全局命令已保存" }));
      setName("");
      setCommandLine("");
      invalidate();
    },
    onError: (e) => toast.error(String(e)),
  });

  const run = useMutation({
    mutationFn: (id: string) => api.startCommand(id),
    onSuccess: (result) => {
      if (result.mode === "external") {
        toast.success(t("launchedExternal", { defaultValue: "已在外部终端打开" }));
      } else {
        useRunStore.getState().upsert({
          runId: result.run.runId,
          commandId: result.run.commandId,
          state: "running",
          commandLine: result.run.commandLine,
        });
      }
      invalidate();
    },
    onError: (e) => toast.error(String(e)),
  });

  const remove = useMutation({
    mutationFn: (id: string) => api.deleteCommand(id),
    onSuccess: invalidate,
    onError: (e) => toast.error(String(e)),
  });

  const canSave = commandLine.trim().length > 0;

  return (
    <section aria-label="global-commands" className="space-y-2">
      <h2 className="text-sm font-medium text-muted-foreground">
        {t("globalCommands", { defaultValue: "全局命令" })}
      </h2>
      <ul className="space-y-1">
        {(list.data ?? []).map((cmd: CommandRow) => {
          const running = runs[cmd.id] !== undefined;
          return (
            <li
              key={cmd.id}
              className="flex items-center justify-between rounded-md border bg-card px-3 py-2 text-sm"
            >
              <div className="flex min-w-0 items-center gap-2">
                <span className="shrink-0 font-medium">{cmd.name}</span>
                <code className="truncate text-xs text-muted-foreground" title={cmd.commandLine}>
                  {cmd.commandLine}
                </code>
                {running && <Badge variant="default">running</Badge>}
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={tc("run")}
                  onClick={() => run.mutate(cmd.id)}
                >
                  <Play className="size-4 text-primary" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={tc("delete")}
                  onClick={() => remove.mutate(cmd.id)}
                >
                  <Trash2 className="size-4 text-destructive" />
                </Button>
              </div>
            </li>
          );
        })}
      </ul>
      <div className="flex gap-1.5">
        <Input
          placeholder={t("globalName", { defaultValue: "名称（如 pi）" })}
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="h-8 w-40"
        />
        <Input
          placeholder={t("globalLine", { defaultValue: "全局命令（如 dsh deploy）" })}
          value={commandLine}
          onChange={(e) => setCommandLine(e.target.value)}
          className="h-8 flex-1"
        />
        <Button size="sm" disabled={!canSave || save.isPending} onClick={() => save.mutate()}>
          {tc("save")}
        </Button>
      </div>
    </section>
  );
}
