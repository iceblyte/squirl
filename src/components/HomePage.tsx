import { useTranslation } from "react-i18next";

import { DiscoverySection } from "@/components/DiscoverySection";
import { RootsManager } from "@/components/RootsManager";
import { ScanPauseDialog } from "@/components/ScanPauseDialog";
import { useScanEvents } from "@/hooks/useScanEvents";

/**
 * HomePage（设计 §5.5.1）：同页两分区——上方已登记网格（S3 接入）+
 * 下方本次发现。S2 交付根目录管理 + 发现区 + 扫描事件链路。
 */
export function HomePage() {
  const { t } = useTranslation("home");
  useScanEvents();

  return (
    <div className="mx-auto flex h-full max-w-4xl flex-col gap-6 overflow-auto p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("title")}</h1>
        <span className="text-xs text-muted-foreground">S2 · roots + discovery</span>
      </header>

      <RootsManager />

      {/* 已登记网格：S3（搜索 / 快启 / 最近使用排序） */}
      <section
        aria-label="registered"
        className="rounded-md border border-dashed px-3 py-6 text-center text-sm text-muted-foreground"
      >
        {t("registered")} — S3
      </section>

      <DiscoverySection />
      <ScanPauseDialog />
    </div>
  );
}
