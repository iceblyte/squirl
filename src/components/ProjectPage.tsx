import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, FolderInput, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import { CommandEditor, CommandListItems } from "@/components/CommandsSection";

function IdeButton({ path, ide, label }: { path: string; ide: string; label: string }) {
  const onOpen = useMutation({
    mutationFn: () => api.openInIde(path, ide),
    onError: (e) => toast.error(String(e)),
  });
  return (
    <Button size="sm" variant="outline" disabled={onOpen.isPending} onClick={() => onOpen.mutate()}>
      {label}
    </Button>
  );
}

import { ConfigFillPanel } from "@/components/ConfigFillPanel";
import { LogPanel } from "@/components/LogPanel";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useRunStore } from "@/stores/runs";
import { useNavStore } from "@/stores/scan";

/** ProjectPage（设计 §5.5.1）：备注编辑 + 重新定位（G2）+ 删除危险区（G1）+ 命令列表（S4） */
export function ProjectPage() {
  const { t } = useTranslation("project");
  const { t: tc } = useTranslation("common");
  const { t: th } = useTranslation("home");
  const navigate = useNavStore((s) => s.navigate);
  const activeProjectId = useNavStore((s) => s.activeProjectId);
  const queryClient = useQueryClient();

  const detail = useQuery({
    queryKey: ["project", activeProjectId],
    queryFn: () => api.getProject(activeProjectId ?? ""),
    enabled: activeProjectId !== null,
  });

  const [note, setNote] = useState("");
  useEffect(() => {
    if (detail.data) setNote(detail.data.note);
  }, [detail.data]);

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["project", activeProjectId] });
    void queryClient.invalidateQueries({ queryKey: ["projects"] });
  };

  const saveNote = useMutation({
    mutationFn: () => api.updateProject(activeProjectId ?? "", { note }),
    onSuccess: () => {
      toast.success(t("noteSaved", { defaultValue: "备注已保存" }));
      invalidate();
    },
    onError: (e) => toast.error(String(e)),
  });

  const relocate = useMutation({
    mutationFn: async () => {
      const picked = await pickFolder({ directory: true, multiple: false });
      if (typeof picked !== "string") return null;
      return api.relocateProject(activeProjectId ?? "", picked);
    },
    onSuccess: (card) => {
      if (card) {
        toast.success(`${t("relocated", { defaultValue: "已重新定位" })}: ${card.name}`);
        invalidate();
      }
    },
    onError: (e) => toast.error(String(e)),
  });

  const count = useQuery({
    queryKey: ["project", activeProjectId, "cmdCount"],
    queryFn: () => api.getProjectCommandCount(activeProjectId ?? ""),
    enabled: activeProjectId !== null,
  });

  const [confirmDelete, setConfirmDelete] = useState(false);
  const remove = useMutation({
    mutationFn: () => api.deleteProject(activeProjectId ?? ""),
    onSuccess: () => {
      toast.success(t("deleted", { defaultValue: "已删除" }));
      setConfirmDelete(false);
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      navigate("home");
    },
    onError: (e) => toast.error(String(e)),
  });

  // 日志面板目标：本项目运行中实例优先，否则最近触达的实例
  const runs = useRunStore((s) => s.runs);
  const byCommand = useRunStore((s) => s.byCommand);

  if (!activeProjectId || detail.isError) {
    return <div className="p-6 text-sm text-muted-foreground">{tc("errorUnexpected")}</div>;
  }
  if (!detail.data) {
    return <div className="p-6 text-sm text-muted-foreground">…</div>;
  }

  const card = detail.data;
  const panelRunId =
    card.commands.map((c) => byCommand[c.id]).find((rid) => rid !== undefined) ??
    (Object.keys(runs).length > 0 ? Object.keys(runs)[0] : null);

  return (
    <div className="mx-auto flex h-full max-w-4xl flex-col gap-4 overflow-auto p-6">
      <div className="flex items-center gap-2">
        <Button size="icon" variant="ghost" aria-label="back" onClick={() => navigate("home")}>
          <ArrowLeft className="size-4" />
        </Button>
        <h1 className="text-lg font-semibold">{card.name}</h1>
        {card.pathMissing && (
          <Badge variant="destructive">{th("pathMissing", { defaultValue: "路径不存在" })}</Badge>
        )}
      </div>
      <p className="text-xs text-muted-foreground" title={card.path}>
        {card.path}
      </p>

      {/* 备注编辑（F2） */}
      <section className="space-y-1.5">
        <label htmlFor="note" className="text-sm font-medium">
          {t("note", { defaultValue: "备注" })}
        </label>
        <textarea
          id="note"
          value={note}
          onChange={(e) => setNote(e.target.value)}
          placeholder={t("notePlaceholder")}
          rows={3}
          className="w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        />
        <div>
          <Button
            size="sm"
            disabled={saveNote.isPending || note === card.note}
            onClick={() => saveNote.mutate()}
          >
            {tc("save")}
          </Button>
        </div>
      </section>

      {/* 日志面板（F5，内置模式）：本项目运行中的实例，否则最近实例 */}
      {panelRunId !== null && <LogPanel runId={panelRunId} />}

      {/* 命令列表 + 常驻保存编辑器（F3/F4 外部半） */}
      <section className="space-y-1.5">
        <h2 className="text-sm font-medium text-muted-foreground">{t("commands")}</h2>
        <CommandListItems projectId={card.id} commands={card.commands} />
        <CommandEditor projectId={card.id} />
      </section>

      {/* P2-⑧：配置快填（DR-26 引擎 + 表单） */}
      <ConfigFillPanel projectPath={card.path} />

      {/* DR-22：IDE 一键打开 */}
      <section className="space-y-1.5">
        <h2 className="text-sm font-medium text-muted-foreground">
          {t("openInIde", { defaultValue: "在 IDE 中打开" })}
        </h2>
        <div className="flex gap-1.5">
          <IdeButton path={card.path} ide="vscode" label="VS Code" />
          <IdeButton path={card.path} ide="cursor" label="Cursor" />
          <IdeButton
            path={card.path}
            ide="explorer"
            label={t("fileManager", { defaultValue: "文件管理器" })}
          />
        </div>
      </section>

      {/* 危险区（G1） */}
      <section className="mt-auto space-y-1.5 rounded-md border border-destructive/40 p-3">
        <h2 className="text-sm font-medium text-destructive">
          {t("dangerZone", { defaultValue: "危险区" })}
        </h2>
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted-foreground">
            {t("dangerHint", { defaultValue: "删除项目将同时删除其全部命令" })}
          </span>
          <div className="flex gap-1.5">
            <Button
              size="sm"
              variant="outline"
              disabled={relocate.isPending}
              onClick={() => relocate.mutate()}
            >
              <FolderInput className="size-4" />
              {t("relocate", { defaultValue: "重新定位" })}
            </Button>
            <Button size="sm" variant="destructive" onClick={() => setConfirmDelete(true)}>
              <Trash2 className="size-4" />
              {tc("delete")}
            </Button>
          </div>
        </div>
      </section>

      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("deleteConfirmTitle", { defaultValue: "删除项目？" })}</DialogTitle>
            <DialogDescription>
              {t("deleteConfirmBody", {
                defaultValue: `将同时删除 ${count.data ?? 0} 条命令，此操作不可撤销。`,
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setConfirmDelete(false)}>
              {tc("cancel")}
            </Button>
            <Button
              variant="destructive"
              disabled={remove.isPending}
              onClick={() => remove.mutate()}
            >
              {tc("delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
