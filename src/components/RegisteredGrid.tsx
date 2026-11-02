import { useQuery } from "@tanstack/react-query";
import { FileWarning } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "@/api/client";
import { Badge } from "@/components/ui/badge";
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
                ? t("lastRun", { defaultValue: "最近使用" }) + ": " + card.lastRunAt
                : t("neverRun", { defaultValue: "未运行过" })}
            </span>
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
