import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Play, Square, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import type { CommandRow, IpcErrorShape } from "@/api/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useRunStore } from "@/stores/runs";
import { useTempRunStore } from "@/stores/temps";

/** 命令编辑器（F3：常驻「保存」按钮——随时可存，需名称；失败命令也允许保存） */
export function CommandEditor({ projectId }: { projectId: string }) {
  const { t } = useTranslation("project");
  const { t: tc } = useTranslation("common");
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [commandLine, setCommandLine] = useState("");
  const [cwd, setCwd] = useState("");

  // F3：临时命令——不保存直接按当前执行模式跑；内置结束会提示保存一次
  const runTemp = useMutation({
    mutationFn: async () => {
      const result = await api.startTempCommand({
        projectId,
        commandLine,
        cwd: cwd || undefined,
      });
      if (result.mode === "builtin") {
        useTempRunStore.getState().register(result.run.runId, {
          projectId,
          commandLine,
        });
      } else {
        toast.success(t("launchedExternal", { defaultValue: "已在外部终端打开" }));
      }
      return result;
    },
    onError: (e) => toast.error(String(e)),
  });

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
      {/* F3：常驻保存按钮——两种模式通用、填写后随时可存；运行 = 临时命令 */}
      <div className="flex justify-end gap-1.5">
        <Button
          size="sm"
          variant="outline"
          disabled={!canSave || runTemp.isPending}
          onClick={() => runTemp.mutate()}
        >
          <Play className="size-3.5" />
          {tc("run")}
        </Button>
        <Button size="sm" disabled={!canSave || save.isPending} onClick={() => save.mutate()}>
          {tc("save")}
        </Button>
      </div>
    </div>
  );
}

/** 命令行条目：运行（双模式）/ 停止（内置运行中）/ 删除 */
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
  const runs = useRunStore((s) => s.runs);
  const byCommand = useRunStore((s) => s.byCommand);

  const run = useMutation({
    mutationFn: ({ commandId, forceNew }: { commandId: string; forceNew: boolean }) =>
      api.startCommand(commandId, forceNew),
    onSuccess: (result) => {
      if (result.mode === "external") {
        toast.success(t("launchedExternal", { defaultValue: "已在外部终端打开" }));
      } else {
        // 内置：runStore 由 run://state 维护；这里立即置位（事件到达前的乐观更新）
        useRunStore.getState().upsert({
          runId: result.run.runId,
          commandId: result.run.commandId,
          state: "running" as const,
          commandLine: result.run.commandLine,
        });
      }
      void queryClient.invalidateQueries({ queryKey: ["project", projectId] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (e) => {
      // E_ALREADY_RUNNING{runId}：默认阻止 + 「再启动一个」（契约 §5.4）
      const err = e as unknown as IpcErrorShape;
      if (err?.code === "E_ALREADY_RUNNING") {
        const commandId = (err.data as { commandId?: string })?.commandId ?? "";
        toast.error(t("alreadyRunning", { defaultValue: "命令已在运行中" }), {
          description: t("alreadyRunningHint", { defaultValue: "重复点击已阻止" }),
          action: {
            label: t("startAnother", { defaultValue: "再启动一个" }),
            onClick: () => run.mutate({ commandId, forceNew: true }),
          },
        });
        return;
      }
      toast.error(String(e));
    },
  });

  const stop = useMutation({
    mutationFn: (runId: string) => api.stopRun(runId),
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
      {commands.map((cmd) => {
        const runningRunId = byCommand[cmd.id];
        const running = runningRunId !== undefined && runs[runningRunId]?.state === "running";
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
              {cmd.lastExitCode !== null && cmd.lastExitCode !== 0 && (
                <Badge variant="destructive">exit {cmd.lastExitCode}</Badge>
              )}
            </div>
            <div className="flex shrink-0 items-center gap-1">
              {running ? (
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={tc("stop")}
                  disabled={stop.isPending}
                  onClick={() => runningRunId && stop.mutate(runningRunId)}
                >
                  <Square className="size-4 text-destructive" />
                </Button>
              ) : (
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={tc("run")}
                  onClick={() => run.mutate({ commandId: cmd.id, forceNew: false })}
                >
                  <Play className="size-4 text-primary" />
                </Button>
              )}
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
  );
}
