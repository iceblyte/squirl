import { useTranslation } from "react-i18next";

/**
 * S1 骨架占位视图：S2 起替换为 HomePage 两分区结构（设计 §5.5.1）。
 * 此处验证 i18n + Tailwind + Provider 链路。
 */
export default function App() {
  const { t } = useTranslation("home");
  const { t: tc } = useTranslation("common");
  return (
    <main className="flex h-screen flex-col items-center justify-center gap-2 bg-neutral-50">
      <h1 className="text-3xl font-semibold text-neutral-800">{t("title")}</h1>
      <p className="text-sm text-neutral-500">{t("tagline")}</p>
      <p className="mt-6 text-xs text-neutral-400">{tc("save")}</p>
    </main>
  );
}
