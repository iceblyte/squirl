// i18n 架构（需求 F8 / 附录 A #11）：UI 文案中文，第一日接入 react-i18next，
// 为后续英文包留位。注：设计 §5.5.2 写「en 为默认」与需求附录 A #11「中文文案」
// 冲突，按冲突优先级（需求附录 A > 设计正文）以中文为默认语言包（ADR-6）。
import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import common from "./resources/zh/common.json";
import home from "./resources/zh/home.json";
import project from "./resources/zh/project.json";
import settings from "./resources/zh/settings.json";

export const defaultNS = "common";
export const namespaces = ["common", "home", "project", "settings"] as const;

export const resources = {
  zh: { common, home, project, settings },
} as const;

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: typeof defaultNS;
    resources: {
      common: typeof common;
      home: typeof home;
      project: typeof project;
      settings: typeof settings;
    };
  }
}

void i18n.use(initReactI18next).init({
  lng: "zh",
  fallbackLng: "zh",
  defaultNS,
  // i18next 26 起必须显式注册命名空间（否则只有默认 translation）
  ns: namespaces,
  resources,
  interpolation: { escapeValue: false },
  returnNull: false,
});

export default i18n;
