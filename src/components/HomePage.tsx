import { useState } from "react";
import { useTranslation } from "react-i18next";

import { DiscoverySection } from "@/components/DiscoverySection";
import { GlobalCommandsSection } from "@/components/GlobalCommandsSection";
import { RegisteredGrid } from "@/components/RegisteredGrid";
import { RootsManager } from "@/components/RootsManager";
import { RunsList } from "@/components/RunsList";
import { ScanPauseDialog } from "@/components/ScanPauseDialog";
import { TopBar } from "@/components/TopBar";
import { useScanEvents } from "@/hooks/useScanEvents";

/**
 * HomePage（设计 §5.5.1）：同页两分区——上方已登记网格 + 下方本次发现。
 */
export function HomePage() {
  const { t } = useTranslation("home");
  const [searchQuery, setSearchQuery] = useState("");
  useScanEvents();

  return (
    <div className="mx-auto flex h-full max-w-4xl flex-col gap-6 overflow-auto p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("title")}</h1>
        <TopBar onSearch={setSearchQuery} />
      </header>

      <RootsManager />

      <RegisteredGrid searchQuery={searchQuery} />

      <RunsList />
      <GlobalCommandsSection />

      <DiscoverySection />
      <ScanPauseDialog />
    </div>
  );
}
