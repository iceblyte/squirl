import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import type { Settings } from "@/api/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

const SHELL_OPTIONS = ["pwsh", "powershell", "cmd", "bash"] as const;
const EMULATOR_OPTIONS = ["wt", "conhost"] as const;
const SHELL_LABELS: Record<string, string> = {
  pwsh: "pwsh 7+",
  powershell: "powershell.exe 5.1",
  cmd: "cmd.exe",
  bash: "bash（Git Bash）",
};
const EMULATOR_LABELS: Record<string, string> = {
  wt: "Windows Terminal",
  conhost: "默认控制台宿主",
};

/** 设置页（F7）：默认模式（内置置灰 Phase 2）/ 模拟器 / Shell / profile / 扫描深度 */
export function SettingsPage() {
  const { t } = useTranslation("settings");
  const { t: tc } = useTranslation("common");
  const queryClient = useQueryClient();

  const settings = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const effective = useQuery({
    queryKey: ["effectiveConfig"],
    queryFn: () => api.getEffectiveExecutionConfig(),
  });

  const [draft, setDraft] = useState<Settings | null>(null);
  useEffect(() => {
    if (settings.data && draft === null) setDraft(settings.data);
  }, [settings.data, draft]);

  const save = useMutation({
    mutationFn: (patch: Settings) => api.updateSettings(patch),
    onSuccess: () => {
      toast.success(t("saved", { defaultValue: "设置已保存" }));
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
      void queryClient.invalidateQueries({ queryKey: ["effectiveConfig"] });
    },
    onError: (e) => toast.error(String(e)),
  });

  if (!draft) {
    return <div className="p-6 text-sm text-muted-foreground">…</div>;
  }

  const dirty = JSON.stringify(draft) !== JSON.stringify(settings.data);

  return (
    <div className="mx-auto flex h-full max-w-3xl flex-col gap-5 overflow-auto p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("title")}</h1>
        <Button size="sm" disabled={!dirty || save.isPending} onClick={() => save.mutate(draft)}>
          {tc("save")}
        </Button>
      </header>

      {/* 默认执行模式（附录 A #3：内置 Phase 2 解锁，置灰） */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium">{t("defaultMode")}</h2>
        <div className="flex gap-2">
          <label className="flex items-center gap-2 rounded-md border bg-card px-3 py-2 text-sm">
            <input
              type="radio"
              name="mode"
              checked={draft.defaultMode === "external"}
              onChange={() => setDraft({ ...draft, defaultMode: "external" })}
            />
            {t("modeExternal")}
          </label>
          <label
            className="flex items-center gap-2 rounded-md border bg-card px-3 py-2 text-sm opacity-50"
            title="Phase 2 解锁"
          >
            <input type="radio" name="mode" disabled checked={false} onChange={() => {}} />
            {t("modeBuiltin")}
          </label>
        </div>
      </section>

      {/* 模拟器（仅外部模式；显示探测结果） */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium">{t("emulator")}</h2>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Badge variant="secondary">
            {t("detected", { defaultValue: "探测结果" })}:{" "}
            {effective.data?.emulator
              ? (EMULATOR_LABELS[effective.data.emulator] ?? effective.data.emulator)
              : "—"}
          </Badge>
        </div>
        <div className="flex gap-2">
          <label className="flex items-center gap-2 rounded-md border bg-card px-3 py-2 text-sm">
            <input
              type="radio"
              name="emulator"
              checked={draft.defaultEmulator === null}
              onChange={() => setDraft({ ...draft, defaultEmulator: null })}
            />
            {t("autoDetect", { defaultValue: "自动探测" })}
          </label>
          {EMULATOR_OPTIONS.map((e) => (
            <label
              key={e}
              className="flex items-center gap-2 rounded-md border bg-card px-3 py-2 text-sm"
            >
              <input
                type="radio"
                name="emulator"
                checked={draft.defaultEmulator === e}
                onChange={() => setDraft({ ...draft, defaultEmulator: e })}
              />
              {EMULATOR_LABELS[e]}
            </label>
          ))}
        </div>
      </section>

      {/* Shell（解析路径 + 回退提示，R3） */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium">{t("shell")}</h2>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Badge variant="secondary">
            {effective.data?.shell} → {effective.data?.shellPath}
          </Badge>
          {(effective.data?.warnings ?? []).map((w) => (
            <Badge key={w} variant="destructive">
              {w}
            </Badge>
          ))}
        </div>
        <div className="flex flex-wrap gap-2">
          <label className="flex items-center gap-2 rounded-md border bg-card px-3 py-2 text-sm">
            <input
              type="radio"
              name="shell"
              checked={draft.defaultShell === null}
              onChange={() => setDraft({ ...draft, defaultShell: null })}
            />
            {t("autoDetect", { defaultValue: "自动解析" })}
          </label>
          {SHELL_OPTIONS.map((s) => (
            <label
              key={s}
              className="flex items-center gap-2 rounded-md border bg-card px-3 py-2 text-sm"
            >
              <input
                type="radio"
                name="shell"
                checked={draft.defaultShell === s}
                onChange={() => setDraft({ ...draft, defaultShell: s })}
              />
              {SHELL_LABELS[s]}
            </label>
          ))}
        </div>
      </section>

      {/* profile 开关 */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium">{t("loadProfile")}</h2>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={draft.loadShellProfile}
            onChange={(e) => setDraft({ ...draft, loadShellProfile: e.target.checked })}
          />
          {t("loadProfileHint", {
            defaultValue: "启动 shell 时加载 profile（nvm / conda 等依赖它）",
          })}
        </label>
      </section>

      {/* 扫描深度 */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium">{t("scanDepth")}</h2>
        <div className="flex items-center gap-2">
          <input
            type="range"
            min={1}
            max={10}
            value={draft.scanDepth}
            onChange={(e) => setDraft({ ...draft, scanDepth: Number(e.target.value) })}
            className="w-56"
          />
          <Badge variant="secondary">{draft.scanDepth}</Badge>
          <span className="text-xs text-muted-foreground">
            {t("depthHint", { defaultValue: "1 = 只列一级子文件夹" })}
          </span>
        </div>
      </section>

      {/* DR-21：自定义排除名单（与内置名单并集） */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium">
          {t("customExcludes", { defaultValue: "自定义排除名单" })}
        </h2>
        <textarea
          rows={3}
          value={(draft.customExcludes ?? []).join("\n")}
          onChange={(e) =>
            setDraft({
              ...draft,
              customExcludes: e.target.value
                .split("\n")
                .map((x) => x.trim())
                .filter(Boolean),
            })
          }
          placeholder={t("excludesHint", {
            defaultValue: "每行一个条目名（与内置名单并集生效）",
          })}
          className="w-full rounded-md border border-input bg-transparent px-3 py-2 font-mono text-xs focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        />
      </section>

      {/* DR-36 ①：导入导出 */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium">{t("dataTransfer", { defaultValue: "导入导出" })}</h2>
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={async () => {
              const p = await saveDialog({ title: "导出台账" });
              if (typeof p === "string") {
                try {
                  await api.exportData(p);
                  toast.success(t("exported", { defaultValue: "导出完成" }));
                } catch (e) {
                  toast.error(String(e));
                }
              }
            }}
          >
            {t("exportLedger", { defaultValue: "导出台账 JSON" })}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={async () => {
              const p = await openDialog({ title: "导入台账" });
              if (typeof p === "string") {
                try {
                  const r = await api.importData(p);
                  toast.success(
                    t("imported", { defaultValue: `导入 ${r.imported} 项（跳过 ${r.skipped}）` }),
                  );
                } catch (e) {
                  toast.error(String(e));
                }
              }
            }}
          >
            {t("importLedger", { defaultValue: "导入台账 JSON" })}
          </Button>
        </div>
      </section>

      {/* DR-19：托盘开关（改后重启生效） */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium">{t("tray", { defaultValue: "系统托盘" })}</h2>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={draft.trayEnabled}
            onChange={(e) => setDraft({ ...draft, trayEnabled: e.target.checked })}
          />
          {t("trayHint", {
            defaultValue: "开启后关窗 = 隐藏到托盘（改后需重启应用生效；Linux 初始化失败自动降级）",
          })}
        </label>
      </section>
    </div>
  );
}
