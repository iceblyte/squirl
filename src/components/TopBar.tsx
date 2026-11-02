import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { FolderPlus, Settings } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useDebouncedValue } from "@/hooks/useDebouncedValue";
import { useNavStore } from "@/stores/scan";

/** TopBar（设计 §5.5.1）：搜索框（debounce 150ms）· 手动注册 · 设置入口 */
export function TopBar({ onSearch }: { onSearch: (q: string) => void }) {
  const { t } = useTranslation("home");
  const { t: tc } = useTranslation("common");
  const navigate = useNavStore((s) => s.navigate);
  const queryClient = useQueryClient();

  const [raw, setRaw] = useState("");
  const debounced = useDebouncedValue(raw, 150);
  useEffect(() => {
    onSearch(debounced);
  }, [debounced, onSearch]);

  const manualRegister = useMutation({
    mutationFn: async () => {
      const picked = await pickFolder({ directory: true, multiple: false });
      if (typeof picked !== "string") return null;
      return api.registerProject(picked);
    },
    onSuccess: (card) => {
      if (card) {
        toast.success(`${t("registeredOk", { defaultValue: "已登记" })}: ${card.name}`);
        void queryClient.invalidateQueries({ queryKey: ["projects"] });
        void queryClient.invalidateQueries({ queryKey: ["discovery"] });
      }
    },
    onError: (e) => toast.error(String(e)),
  });

  return (
    <header className="flex items-center gap-2">
      <Input
        aria-label={tc("search")}
        placeholder={`${tc("search")}…`}
        value={raw}
        onChange={(e) => setRaw(e.target.value)}
        className="max-w-sm"
      />
      <Button
        size="sm"
        variant="outline"
        disabled={manualRegister.isPending}
        onClick={() => manualRegister.mutate()}
      >
        <FolderPlus className="size-4" />
        {t("manualRegister")}
      </Button>
      <Button size="sm" variant="ghost" aria-label="settings" onClick={() => navigate("settings")}>
        <Settings className="size-4" />
      </Button>
    </header>
  );
}
