import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Power, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "@/api/client";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useScanStore } from "@/stores/scan";

/** 根目录管理（F1：添加/删除/启用禁用；改路径不做——G4） */
export function RootsManager() {
  const { t } = useTranslation("home");
  const queryClient = useQueryClient();
  const setScanning = useScanStore((s) => s.setScanning);

  const roots = useQuery({ queryKey: ["roots"], queryFn: api.listRoots });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["roots"] });
  };

  const addRoot = useMutation({
    mutationFn: async () => {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked !== "string") return null;
      const root = await api.addRoot(picked);
      // F1：添加根目录时全量扫一次
      await api.startScan();
      setScanning(true);
      return root;
    },
    onSuccess: invalidate,
  });

  const removeRoot = useMutation({
    mutationFn: (id: string) => api.removeRoot(id),
    onSuccess: invalidate,
  });

  const toggleEnabled = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      api.setRootEnabled(id, enabled),
    onSuccess: invalidate,
  });

  const setDepth = useMutation({
    mutationFn: ({ id, depth }: { id: string; depth: number | null }) =>
      api.setRootDepth(id, depth),
    onSuccess: invalidate,
  });

  return (
    <section aria-label="roots" className="space-y-2">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">{t("registered")}</h2>
        <Button
          size="sm"
          variant="outline"
          onClick={() => addRoot.mutate()}
          disabled={addRoot.isPending}
        >
          <FolderOpen className="size-4" />
          {t("addRoot")}
        </Button>
      </div>
      <ul className="space-y-1.5">
        {(roots.data ?? []).map((root) => (
          <li
            key={root.id}
            className="flex items-center justify-between rounded-md border bg-card px-3 py-2 text-sm"
          >
            <div className="flex min-w-0 items-center gap-2">
              <Badge variant={root.enabled ? "default" : "outline"}>
                {root.enabled ? "ON" : "OFF"}
              </Badge>
              <span className="truncate text-foreground" title={root.path}>
                {root.path}
              </span>
              <select
                aria-label="depth"
                className="h-6 rounded border border-input bg-transparent px-1 text-xs"
                value={root.scanDepth ?? ""}
                onChange={(e) =>
                  setDepth.mutate({
                    id: root.id,
                    depth: e.target.value === "" ? null : Number(e.target.value),
                  })
                }
              >
                <option value="">跟随全局</option>
                {[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((d) => (
                  <option key={d} value={d}>
                    {d}
                  </option>
                ))}
              </select>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <Button
                size="icon"
                variant="ghost"
                aria-label={root.enabled ? "disable" : "enable"}
                onClick={() => toggleEnabled.mutate({ id: root.id, enabled: !root.enabled })}
              >
                <Power className="size-4" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                aria-label="remove"
                onClick={() => removeRoot.mutate(root.id)}
              >
                <Trash2 className="size-4 text-destructive" />
              </Button>
            </div>
          </li>
        ))}
        {(roots.data ?? []).length === 0 && (
          <li className="rounded-md border border-dashed px-3 py-6 text-center text-sm text-muted-foreground">
            {t("emptyGuide")}
          </li>
        )}
      </ul>
    </section>
  );
}
