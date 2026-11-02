import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileWarning, Play } from "lucide-react";
import type React from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import type { CommandRow, ProjectCard } from "@/api/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useNavStore } from "@/stores/scan";

/** 已登记卡片网格（F2）：名称/路径/备注摘要/最近使用排序/失效标记。
 *  命令快启按钮归 S4（命令 CRUD 落地后接 G3 规则：单命令直启、多命令主按钮+下拉）。 */
export function RegisteredGrid({ searchQuery }: { searchQuery: string }) {
  const { t } = useTranslation("home");
  const openProject = useNavStore((s) => s.openProject);

  // 搜索走后端 SQL（四字段 LIKE）；空查询走全量（最近使用排序）
  const list = useQuery({
    queryKey: ["projects", searchQuery],
    queryFn: () => (searchQuery ? api.searchProjects(searchQuery) : api.listProjects()),
  });

  const cards = list.data ?? [];

  return (
    <section aria-label="registered" className="space-y-2">
      <h2 className="text-sm font-medium text-muted-foreground">{t("registered")}</h2>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-2">
        {cards.map((card) => (
          <button
            key={card.id}
            type="button"
            className="flex flex-col items-start gap-1 rounded-lg border bg-card p-3 text-left transition-colors hover:border-primary/50"
            onClick={() => openProject(card.id)}
          >
            <div className="flex w-full items-center justify-between gap-2">
              <span className="truncate font-medium text-foreground">{card.name}</span>
              {card.pathMissing && (
                <Badge variant="destructive">
                  <FileWarning className="mr-0.5 size-3" />
                  {t("pathMissing", { defaultValue: "路径不存在" })}
                </Badge>
              )}
            </div>
            <span className="w-full truncate text-xs text-muted-foreground" title={card.path}>
              {card.path}
            </span>
            {card.note && (
              <span className="line-clamp-2 w-full text-xs text-muted-foreground">{card.note}</span>
            )}
            <span className="text-[10px] text-muted-foreground">
              {card.lastRunAt
                ? `${t("lastRun", { defaultValue: "最近使用" })}: ${card.lastRunAt}`
                : t("neverRun", { defaultValue: "未运行过" })}
            </span>
            <QuickRun card={card} />
          </button>
        ))}
        {cards.length === 0 && !list.isLoading && (
          <div className="col-span-full rounded-md border border-dashed px-3 py-6 text-center text-sm text-muted-foreground">
            {searchQuery
              ? t("noSearchResult", { defaultValue: "没有匹配的项目" })
              : t("emptyProjects", {
                  defaultValue: "从下方「本次发现」登记项目，或手动注册任意文件夹",
                })}
          </div>
        )}
      </div>
    </section>
  );
}

/** 卡片命令快启（G3）：单命令直启；多命令主按钮 = 最近使用命令 + 下拉列全部 */
function QuickRun({ card }: { card: ProjectCard }) {
  const { t } = useTranslation("common");
  const { t: th } = useTranslation("home");
  const queryClient = useQueryClient();
  const run = useMutation({
    mutationFn: (commandId: string) => api.startCommand(commandId),
    onSuccess: (result) => {
      if (result.mode === "external") {
        toast.success(th("launchedExternal", { defaultValue: "已在外部终端打开" }));
      }
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (e) => toast.error(String(e)),
  });

  const commands = card.commands;
  if (commands.length === 0) return null;

  const lastUsed = [...commands]
    .filter((c): c is CommandRow & { lastRunAt: string } => c.lastRunAt !== null)
    .sort((a, b) => (a.lastRunAt < b.lastRunAt ? 1 : -1))[0];
  const primary = lastUsed ?? commands[0];

  const stop = (e: React.MouseEvent) => e.stopPropagation();
  return (
    <div className="flex w-full items-center gap-1">
      {commands.length === 1 ? (
        <Button
          size="sm"
          className="w-full"
          disabled={run.isPending}
          onClick={(e) => {
            stop(e);
            run.mutate(primary.id);
          }}
        >
          <Play className="size-3.5" />
          {primary.name}
        </Button>
      ) : (
        <div className="flex w-full items-center gap-1">
          <Button
            size="sm"
            className="flex-1"
            disabled={run.isPending}
            onClick={(e) => {
              stop(e);
              run.mutate(primary.id);
            }}
          >
            <Play className="size-3.5" />
            {primary.name}
          </Button>
          <details className="relative">
            <summary className="flex h-8 cursor-pointer list-none items-center rounded-md border border-input px-2 text-xs">
              ▾
            </summary>
            <ul className="absolute right-0 z-10 mt-1 w-48 rounded-md border bg-card p-1 shadow-lg">
              {commands.map((c: CommandRow) => (
                <li key={c.id}>
                  <button
                    type="button"
                    className="flex w-full items-center gap-1 rounded px-2 py-1.5 text-left text-xs hover:bg-accent"
                    onClick={(e) => {
                      e.stopPropagation();
                      run.mutate(c.id);
                    }}
                  >
                    <Play className="size-3" />
                    {c.name}
                  </button>
                </li>
              ))}
              <li className="px-2 py-1 text-[10px] text-muted-foreground">
                {t("run")} · {commands.length}
              </li>
            </ul>
          </details>
        </div>
      )}
    </div>
  );
}
