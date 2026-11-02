import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Play, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import type { CommandRow } from "@/api/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

/** 命令编辑器（F3：常驻「保存」按钮——随时可存，需名称；失败命令也允许保存） */
export function CommandEditor({ projectId }: { projectId: string }) {
  const { t } = useTranslation("project");
  const { t: tc } = useTranslation("common");
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [commandLine, setCommandLine] = useState("");
  const [cwd, setCwd] = useState("");

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["project", projectId] });
    void queryClient.invalidateQueries({ queryKey: ["projects"] });
  };

  const canSave = commandLine.trim().length > 0;
  const save = useMutation({
    mutationFn: async () => {
      if (!canSave) return null;
      return api.addCommand(
        projectId,
        name.trim() || t("unnamed", { defaultValue: "未命名命令" }),
        commandLine,
        cwd || undefined,
      );
    },
    onSuccess: () => {
      toast.success(t("commandSaved", { defaultValue: "命令已保存" }));
      setName("");
      setCommandLine("");
      setCwd("");
      invalidate();
    },
    onError: (e) => toast.error(String(e)),
  });

  return (
    <div className="space-y-1.5 rounded-md border bg-card p-3">
      <div className="flex gap-1.5">
        <Input
          placeholder={t("commandName", { defaultValue: "命令名称" })}
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="h-8 w-40"
        />
        <Input
          placeholder={t("commandLine", { defaultValue: "命令行原文（如 npm run dev）" })}
          value={commandLine}
          onChange={(e) => setCommandLine(e.target.value)}
          className="h-8 flex-1"
        />
        <Input
          placeholder={t("commandCwd", { defaultValue: "工作目录（默认项目根）" })}
          value={cwd}
          onChange={(e) => setCwd(e.target.value)}
          className="h-8 w-52"
        />
      </div>
      {/* F3：常驻保存按钮——两种模式通用、填写后随时可存 */}
      <div className="flex justify-end">
        <Button size="sm" disabled={!canSave || save.isPending} onClick={() => save.mutate()}>
          {tc("save")}
        </Button>
      </div>
    </div>
  );
}

/** 命令行条目：运行（外部）/ 编辑 / 删除（S6 接内置停止） */
export function CommandListItems({
  projectId,
  commands,
}: {
  projectId: string;
  commands: CommandRow[];
}) {
  const { t } = useTranslation("project");
  const { t: tc } = useTranslation("common");
  const queryClient = useQueryClient();

  const run = useMutation({
    mutationFn: (commandId: string) => api.startCommand(commandId),
    onSuccess: (result) => {
      if (result.mode === "external") {
        toast.success(t("launchedExternal", { defaultValue: "已在外部终端打开" }));
      }
      void queryClient.invalidateQueries({ queryKey: ["project", projectId] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (e) => toast.error(String(e)),
  });

  const remove = useMutation({
    mutationFn: (id: string) => api.deleteCommand(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["project", projectId] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (e) => toast.error(String(e)),
  });

  return (
    <ul className="space-y-1">
      {commands.map((cmd) => (
        <li
          key={cmd.id}
          className="flex items-center justify-between rounded-md border bg-card px-3 py-2 text-sm"
        >
          <div className="flex min-w-0 items-center gap-2">
            <span className="shrink-0 font-medium">{cmd.name}</span>
            <code className="truncate text-xs text-muted-foreground" title={cmd.commandLine}>
              {cmd.commandLine}
            </code>
            {cmd.lastExitCode !== null && cmd.lastExitCode !== 0 && (
              <Badge variant="destructive">exit {cmd.lastExitCode}</Badge>
            )}
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
      ))}
    </ul>
  );
}
