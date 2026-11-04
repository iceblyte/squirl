import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { useNavStore } from "@/stores/scan";

/** DR-36 ③：Ctrl+K 命令面板（搜索项目/命令 → Enter 运行或导航） */
export function CommandPalette() {
  const { t } = useTranslation("common");
  const queryClient = useQueryClient();
  const openProject = useNavStore((s) => s.openProject);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");

  // Ctrl+K 快捷键
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "k") {
        e.preventDefault();
        setOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const projects = useQuery({
    queryKey: ["projects", query],
    queryFn: () => (query ? api.searchProjects(query) : api.listProjects()),
    enabled: open,
  });
  const globals = useQuery({
    queryKey: ["globalCommands"],
    queryFn: api.listGlobalCommands,
    enabled: open,
  });

  const run = useMutation({
    mutationFn: (commandId: string) => api.startCommand(commandId),
    onSuccess: (result) => {
      if (result.mode === "external") {
        toast.success(t("launchedExternal", { ns: "home", defaultValue: "已在外部终端打开" }));
      }
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      setOpen(false);
    },
    onError: (e) => toast.error(String(e)),
  });

  const items = useMemo(() => {
    const proj = (projects.data ?? []).map((p) => ({
      kind: "project" as const,
      id: p.id,
      label: p.name,
      hint: p.path,
    }));
    const glob = (globals.data ?? [])
      .filter((c) => !query || c.name.includes(query) || c.commandLine.includes(query))
      .map((c) => ({
        kind: "global" as const,
        id: c.id,
        label: c.name,
        hint: c.commandLine,
      }));
    return [...glob, ...proj].slice(0, 12);
  }, [projects.data, globals.data, query]);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="top-[15%] max-w-xl p-0">
        <input
          autoFocus
          placeholder={t("palettePlaceholder", { defaultValue: "搜索项目或命令…" })}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="w-full border-b bg-transparent px-4 py-3 text-sm outline-none"
          onKeyDown={(e) => {
            if (e.key === "Escape") setOpen(false);
          }}
        />
        <ul className="max-h-80 overflow-auto p-1">
          {items.map((item) => (
            <li key={item.kind + item.id}>
              {item.kind === "project" ? (
                <button
                  type="button"
                  className="w-full rounded px-3 py-2 text-left text-sm hover:bg-accent"
                  onClick={() => {
                    openProject(item.id);
                    setOpen(false);
                    setQuery("");
                  }}
                >
                  {item.label}
                </button>
              ) : (
                <button
                  type="button"
                  className="w-full rounded px-3 py-2 text-left text-sm hover:bg-accent"
                  onClick={() => run.mutate(item.id)}
                >
                  <span className="font-medium">{item.label}</span>
                  <code className="ml-2 text-xs text-muted-foreground">{item.hint}</code>
                </button>
              )}
            </li>
          ))}
          {items.length === 0 && (
            <li className="px-3 py-4 text-center text-xs text-muted-foreground">—</li>
          )}
        </ul>
        <p className="border-t px-4 py-1.5 text-[10px] text-muted-foreground">
          {t("paletteHint", { defaultValue: "Enter 运行全局命令 / 点击项目进入详情 · Esc 关闭" })}
        </p>
      </DialogContent>
    </Dialog>
  );
}
