import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, RefreshCw, Star } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "@/api/client";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useScanStore } from "@/stores/scan";

/** 本次发现（F2）：瞬时、相对路径平铺、本地名称过滤、登记按钮、标志高亮、不可运行 */
export function DiscoverySection() {
  const { t } = useTranslation("home");
  const { t: tc } = useTranslation("common");
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(true);
  const [filter, setFilter] = useState("");
  const scanning = useScanStore((s) => s.scanning);

  const discovery = useQuery({ queryKey: ["discovery"], queryFn: api.getDiscovery });

  const register = useMutation({
    mutationFn: (path: string) => api.registerProject(path),
    onSuccess: () => {
      // 登记后：台账 +1、发现区由下次扫描重算（瞬时语义）
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      void queryClient.invalidateQueries({ queryKey: ["discovery"] });
    },
  });

  const entries = useMemo(() => {
    const all = discovery.data ?? [];
    if (!filter) return all;
    const f = filter.toLowerCase();
    return all.filter((e) => e.relPath.toLowerCase().includes(f));
  }, [discovery.data, filter]);

  return (
    <section aria-label="discovery" className="space-y-2">
      <div className="flex items-center justify-between">
        <button
          type="button"
          className="flex items-center gap-1 text-sm font-medium text-muted-foreground"
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
          {t("discovery")}
          {discovery.data && discovery.data.length > 0 && (
            <Badge variant="secondary">{discovery.data.length}</Badge>
          )}
        </button>
        <Button
          size="sm"
          variant="ghost"
          disabled={scanning}
          onClick={async () => {
            await api.startScan();
            useScanStore.getState().setScanning(true);
          }}
        >
          <RefreshCw className={`size-4 ${scanning ? "animate-spin" : ""}`} />
          {tc("refresh")}
        </Button>
      </div>

      {open && (
        <>
          <Input
            placeholder={`${t("discovery")} · ${tc("search")}`}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="h-8"
          />
          <ul className="max-h-72 space-y-1 overflow-auto">
            {entries.map((e) => (
              <li
                key={e.path}
                className="flex items-center justify-between rounded-md border bg-card px-3 py-1.5 text-sm"
              >
                <div className="flex min-w-0 items-center gap-2">
                  <span className="truncate" title={e.relPath}>
                    {e.relPath}
                  </span>
                  {e.flags.map((f) => (
                    <Badge key={f} variant="default">
                      <Star className="mr-0.5 size-3" />
                      {f}
                    </Badge>
                  ))}
                </div>
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={register.isPending}
                  onClick={() => register.mutate(e.path)}
                >
                  {t("register", { defaultValue: "登记" })}
                </Button>
              </li>
            ))}
            {entries.length === 0 && (
              <li className="rounded-md border border-dashed px-3 py-6 text-center text-sm text-muted-foreground">
                {/* G5：重启后发现区为空 → 引导刷新扫描 */}
                {scanning
                  ? t("scanning", { defaultValue: "扫描中…" })
                  : t("refreshHint", { defaultValue: "点击刷新扫描" })}
              </li>
            )}
          </ul>
        </>
      )}
    </section>
  );
}
