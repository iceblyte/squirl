import { Toaster } from "sonner";
import { CommandPalette } from "@/components/CommandPalette";
import { HomePage } from "@/components/HomePage";
import { ProjectPage } from "@/components/ProjectPage";
import { SettingsPage } from "@/components/SettingsPage";
import { TempSaveDialog } from "@/components/TempSaveDialog";
import { useDragDropRegister } from "@/hooks/useDragDropRegister";
import { useNavStore } from "@/stores/scan";

/** 视图切换（DR-4：无路由，navStore） */
export default function App() {
  const view = useNavStore((s) => s.view);
  useDragDropRegister();
  return (
    <main className="h-screen">
      {view === "home" && <HomePage />}
      {view === "project" && <ProjectPage />}
      {view === "settings" && <SettingsPage />}
      <TempSaveDialog />
      <CommandPalette />
      <Toaster position="top-center" richColors />
    </main>
  );
}
