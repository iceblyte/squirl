import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderMinus, GitBranch } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
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

/** DR-29 Git 元数据 + DR-30 休眠瘦身（项目详情区块） */
export function ProjectMetaPanel({ projectId }: { projectId: string }) {
  const { t } = useTranslation("project");
  const { t: tc } = useTranslation("common");
  const queryClient = useQueryClient();

  const meta = useQuery({
    queryKey: ["gitmeta", projectId],
    queryFn: () => api.getGitMeta(projectId),
    retry: false,
  });

  const [confirmHibernate, setConfirmHibernate] = useState(false);
  const hibernate = useMutation({
    mutationFn: () => api.hibernateProject(projectId),
    onSuccess: (removed) => {
      toast.success(
        `${t("hibernated", { defaultValue: "已休眠瘦身" })}: ${removed.join(", ") || t("nothingRemoved", { defaultValue: "无可清理目录" })}`,
      );
      setConfirmHibernate(false);
      void queryClient.invalidateQueries({ queryKey: ["gitmeta", projectId] });
    },
    onError: (e) => toast.error(String(e)),
  });

  return (
    <section className="space-y-1.5">
      <h2 className="text-sm font-medium text-muted-foreground">
        {t("gitMeta", { defaultValue: "Git" })}
      </h2>
      {meta.data ? (
        <div className="flex flex-wrap items-center gap-2 text-xs">
          {meta.data.branch && (
            <Badge variant="secondary">
              <GitBranch className="mr-0.5 size-3" />
              {meta.data.branch}
            </Badge>
          )}
          <code className="text-muted-foreground">{meta.data.commitHash}</code>
          {meta.data.remoteUrl && (
            <a
              href={meta.data.remoteUrl}
              target="_blank"
              rel="noreferrer"
              className="truncate text-primary underline-offset-2 hover:underline"
              title={meta.data.remoteUrl}
            >
              {meta.data.remoteUrl}
            </a>
          )}
        </div>
      ) : (
        <p className="text-xs text-muted-foreground">
          {t("notGitRepo", { defaultValue: "非 git 仓库" })}
        </p>
      )}

      <div className="flex items-center justify-between pt-1">
        <span className="text-xs text-muted-foreground">
          {t("hibernateHint", {
            defaultValue: "休眠瘦身：删除 node_modules / .venv / target（备注与命令保留）",
          })}
        </span>
        <Button
          size="sm"
          variant="outline"
          disabled={hibernate.isPending}
          onClick={() => setConfirmHibernate(true)}
        >
          <FolderMinus className="size-4" />
          {t("hibernate", { defaultValue: "休眠瘦身" })}
        </Button>
      </div>

      <Dialog open={confirmHibernate} onOpenChange={setConfirmHibernate}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("hibernateTitle", { defaultValue: "休眠瘦身？" })}</DialogTitle>
            <DialogDescription>
              {t("hibernateBody", {
                defaultValue:
                  "将删除项目根下的 node_modules / .venv / venv / target 目录（README 与配置文件保留）。重新执行 install/build 即可恢复。",
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setConfirmHibernate(false)}>
              {tc("cancel")}
            </Button>
            <Button
              variant="destructive"
              disabled={hibernate.isPending}
              onClick={() => hibernate.mutate()}
            >
              {t("hibernate", { defaultValue: "休眠瘦身" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </section>
  );
}
