import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import "../src/i18n";
import App from "../src/App";

describe("S1 骨架", () => {
  it("以中文渲染标题与标语（F8：UI 文案中文）", () => {
    render(<App />);
    expect(screen.getByText("Squirl")).toBeTruthy();
    expect(screen.getByText("本地项目台账与一键启动中枢")).toBeTruthy();
  });

  it("common 命名空间文案可用（类型化 key 链路）", () => {
    render(<App />);
    expect(screen.getByText("保存")).toBeTruthy();
  });
});
