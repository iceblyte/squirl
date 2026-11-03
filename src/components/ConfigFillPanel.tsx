import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Save } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import type { ConfigFilePayload } from "@/api/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

/** P2-⑧ 右栏：配置键值表单（DR-26 引擎 + IPC），保格式写回 + 备份 */
export function ConfigFillPanel({ projectPath }: { projectPath: string }) {
  const { t } = useTranslation("project");
  const { t: tc } = useTranslation("common");
  const queryClient = useQueryClient();

  // 找项目根下第一个支持的配置文件
  const candidates = useMemo(
    () => [
      `${projectPath}\\.env`,
      `${projectPath}\\.env.example`,
      `${projectPath}\\config.toml`,
      `${projectPath}\\config.yml`,
    ],
    [projectPath],
  );
  const [configPath, setConfigPath] = useState<string | null>(null);
  const config = useQuery({
    queryKey: ["config", configPath],
    queryFn: () => api.readProjectConfig(configPath ?? ""),
    enabled: configPath !== null,
    retry: false,
  });

  // 逐个探测候选文件（404 = 不存在，静默切下一个）
  useEffect(() => {
    let idx = 0;
    let cancelled = false;
    const probe = async () => {
      while (idx < candidates.length && !cancelled) {
        try {
          await api.readProjectConfig(candidates[idx]);
          if (!cancelled) setConfigPath(candidates[idx]);
          return;
        } catch {
          idx += 1;
        }
      }
    };
    void probe();
    return () => {
      cancelled = true;
    };
  }, [candidates]);

  const payload: ConfigFilePayload | undefined = config.data;
  const [values, setValues] = useState<Record<string, string>>({});
  useEffect(() => {
    if (payload) {
      const v: Record<string, string> = {};
      for (const k of payload.keys) v[k.key] = k.value;
      setValues(v);
    }
  }, [payload]);

  const apply = useMutation({
    mutationFn: () => {
      const list = Object.entries(values).map(([key, value]) => ({ key, value }));
      return api.applyProjectConfig(configPath ?? "", list);
    },
    onSuccess: (changed) => {
      if (changed) {
        toast.success(t("configSaved", { defaultValue: "配置已保存（原文件已备份）" }));
      } else {
        toast.info(t("configNoChange", { defaultValue: "无变更" }));
      }
      void queryClient.invalidateQueries({ queryKey: ["config", configPath] });
    },
    onError: (e) => toast.error(String(e)),
  });

  if (configPath === null) {
    return (
      <p className="rounded-md border border-dashed px-3 py-4 text-center text-xs text-muted-foreground">
        {t("noConfigFile", { defaultValue: "项目根未找到 .env / config.toml / config.yml" })}
      </p>
    );
  }
  if (config.isError) {
    return (
      <p className="text-xs text-muted-foreground">
        {t("noConfigFile", { defaultValue: "项目根未找到 .env / config.toml / config.yml" })}
      </p>
    );
  }

  const dirty = payload?.keys.some((k) => values[k.key] !== undefined && values[k.key] !== k.value);

  return (
    <section className="space-y-1.5">
      <div className="flex items-center justify-between">
        <h2 className="flex items-center gap-2 text-sm font-medium text-muted-foreground">
          {t("configFill", { defaultValue: "配置快填" })}
          <Badge variant="outline">{payload?.dialect}</Badge>
        </h2>
        <Button size="sm" disabled={!dirty || apply.isPending} onClick={() => apply.mutate()}>
          <Save className="size-3.5" />
          {tc("save")}
        </Button>
      </div>
      <ul className="space-y-1">
        {(payload?.keys ?? []).map((k) => (
          <li key={k.key} className="flex items-center gap-2 text-sm">
            <label htmlFor={`cfg-${k.key}`} className="w-40 shrink-0 truncate font-mono text-xs">
              {k.key}
            </label>
            <input
              id={`cfg-${k.key}`}
              value={values[k.key] ?? ""}
              onChange={(e) => setValues((v) => ({ ...v, [k.key]: e.target.value }))}
              className="h-8 flex-1 rounded-md border border-input bg-transparent px-2 font-mono text-xs focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            />
          </li>
        ))}
      </ul>
    </section>
  );
}
