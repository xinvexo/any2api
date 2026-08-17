import { expect, test } from "vitest";

import { SETTING_SECTIONS } from "./setting-categories";

test("running protection exposes only the configurable reliability budget", () => {
  const protection = SETTING_SECTIONS.find((section) => section.id === "protection");

  expect(protection).toEqual({
    id: "protection",
    label: "运行保护",
    webGroups: [
      "重试预算",
      "上游网络",
      "OAuth 刷新",
      "流式预提交",
      "流式响应",
      "优雅停机",
    ],
    featuredKeys: [
      "retry.precommit_total_budget",
      "upstream.read_timeout",
      "upstream.strict_ssrf",
    ],
  });
});
