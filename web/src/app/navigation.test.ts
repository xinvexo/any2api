import { expect, test } from "vitest";

import { getPageTitle, navigationItems } from "./navigation";

test("orders and names the primary navigation", () => {
  expect(navigationItems.map(({ label, path }) => [label, path])).toEqual([
    ["系统总览", "/"],
    ["上游提供", "/providers"],
    ["认证文件", "/oauth"],
    ["额度费率", "/quota-rates"],
    ["网关密钥", "/keys"],
    ["出口代理", "/proxies"],
    ["请求日志", "/logs"],
    ["系统日志", "/system-logs"],
    ["系统设置", "/settings"],
  ]);
  expect(getPageTitle("/")).toBe("系统总览");
  expect(getPageTitle("/quota-rates")).toBe("额度费率");
});
