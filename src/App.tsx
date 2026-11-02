import { HomePage } from "@/components/HomePage";
import { useNavStore } from "@/stores/scan";

/** 视图切换（DR-4：无路由，navStore）；project/settings 视图随 S3/S5 接入 */
export default function App() {
  const view = useNavStore((s) => s.view);
  return (
    <main className="h-screen">
      {view === "home" && <HomePage />}
      {view === "project" && <div className="p-6 text-sm text-muted-foreground">S3</div>}
      {view === "settings" && <div className="p-6 text-sm text-muted-foreground">S5</div>}
    </main>
  );
}
