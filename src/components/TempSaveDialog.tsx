import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { api } from "@/api/client";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { type TemsStateImpl, useTempRunStore } from "@/stores/temps";

/** F3：临时命令保存命名框（需确认名称；不保存则丢弃） */
export function TempSaveDialog() {
  const { t } = useTranslation("project");
  const { t: tc } = useTranslation("common");
  const queryClient = useQueryClient();
  const pending = useTempRunStore((s: TemsStateImpl) => s.pending);
  const clearPending = useTempRunStore((s: TemsStateImpl) => s.clearPending);
  const [name, setName] = useState("");

  const save = useMutation({
    mutationFn: (name: string) =>
      api.addCommand(pending?.projectId ?? "", name, pending?.commandLine ?? ""),
    onSuccess: () => {
      toast.success(t("commandSaved", { defaultValue: "命令已保存" }));
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      if (pending?.projectId) {
        void queryClient.invalidateQueries({ queryKey: ["project", pending.projectId] });
      }
      clearPending();
      setName("");
    },
    onError: (e) => toast.error(String(e)),
  });

  return (
    <Dialog open={pending !== null}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("saveTempTitle", { defaultValue: "保存本次命令" })}</DialogTitle>
        </DialogHeader>
        <Input
          autoFocus
          placeholder={t("commandName", { defaultValue: "命令名称" })}
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <code className="block truncate text-xs text-muted-foreground" title={pending?.commandLine}>
          {pending?.commandLine}
        </code>
        <DialogFooter>
          <Button
            variant="ghost"
            onClick={() => {
              clearPending(); // 不保存则丢弃
              setName("");
            }}
          >
            {t("discard", { defaultValue: "丢弃" })}
          </Button>
          <Button
            disabled={name.trim().length === 0 || save.isPending}
            onClick={() => save.mutate(name.trim())}
          >
            {tc("save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
