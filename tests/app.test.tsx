import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import "../src/i18n";
import { HomePage } from "../src/components/HomePage";

function renderHome() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <HomePage />
    </QueryClientProvider>,
  );
}

describe("S2 主页骨架", () => {
  it("以中文渲染标题与分区标题（F8：UI 文案中文）", () => {
    renderHome();
    expect(screen.getByText("Squirl")).toBeTruthy();
    expect(screen.getAllByText("已登记").length).toBeGreaterThan(0);
    expect(screen.getByText("本次发现")).toBeTruthy();
  });

  it("空状态引导：添加根目录提示（F1 首启引导）", async () => {
    renderHome();
    // 非 Tauri 环境下 roots 查询失败 → 空列表渲染引导文案（已登记 S3 占位外的发现区）
    expect(screen.getByText("添加根目录")).toBeTruthy();
  });
});
