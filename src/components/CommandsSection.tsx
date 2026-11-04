import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Play, Send, Square, Trash2 } from "lucide-react";
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
  const [note, setNote] = useState("");
  const [fingerprint, setFingerprint] = useState(false);

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

  const canSave = commandLine.trim().length > 0;
  const save = useMutation({
    mutationFn: async () => {
      if (!canSave) return null;
      return api.addCommand(
        projectId,
        name.trim() || t("unnamed", { defaultValue: "未命名命令" }),
        commandLine,
        cwd || undefined,
        fingerprint,
        note || undefined,
      );
    },
    onSuccess: () => {
      toast.success(t("commandSaved", { defaultValue: "命令已保存" }));
      setName("");
      setCommandLine("");
      setCwd("");
      setNote("");
      setFingerprint(false);
      void queryClient.invalidateQueries({ queryKey: ["project", projectId] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
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
      <div className="flex items-center gap-3">
        <Input
          placeholder={t("commandNote", { defaultValue: "用途备注（词典）" })}
          value={note}
          onChange={(e) => setNote(e.target.value)}
          className="h-7 w-56 text-xs"
        />
        {/* DR-24：指纹感知开关 */}
        <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <input
            type="checkbox"
            checked={fingerprint}
            onChange={(e) => setFingerprint(e.target.checked)}
          />
          {t("fingerprintHint", {
            defaultValue: "锁文件指纹感知（变化时自动先执行 install/build）",
          })}
        </label>
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

/** 命令行条目：运行（双模式）/ 停止（内置运行中，含 stdin 输入）/ 删除 */
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
        useRunStore.getState().upsert({
          runId: result.run.runId,
          commandId: result.run.commandId,
          state: "running",
          commandLine: result.run.commandLine,
        });
      }
      void queryClient.invalidateQueries({ queryKey: ["project", projectId] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (e) => {
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
          <li key={cmd.id} className="rounded-md border bg-card px-3 py-2 text-sm">
            <div className="flex items-center justify-between">
              <div className="flex min-w-0 items-center gap-2">
                <span className="shrink-0 font-medium">{cmd.name}</span>
                {cmd.note && <Badge variant="outline">{cmd.note}</Badge>}
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
            </div>
            {running && runningRunId && <StdinInput runId={runningRunId} />}
          </li>
        );
      })}
    </ul>
  );
}

/** DR-32：运行中实例的 stdin 单行输入（Enter 发送行） */
function StdinInput({ runId }: { runId: string }) {
  const { t } = useTranslation("project");
  const [value, setValue] = useState("");
  const send = useMutation({
    mutationFn: (data: string) => api.writeStdin(runId, `${data}\n`),
    onSuccess: () => setValue(""),
    onError: (e) => toast.error(String(e)),
  });
  return (
    <div className="mt-1.5 flex items-center gap-1">
      <Input
        aria-label="stdin"
        placeholder={t("stdinPlaceholder", { defaultValue: "向进程发送输入…" })}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && value.trim()) send.mutate(value);
        }}
        className="h-7 font-mono text-xs"
      />
      <Button
        size="icon"
        variant="ghost"
        disabled={!value.trim() || send.isPending}
        onClick={() => send.mutate(value)}
      >
        <Send className="size-3.5" />
      </Button>
    </div>
  );
}
