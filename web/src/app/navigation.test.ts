import { FileKey2 } from "lucide-react";
import { expect, test } from "vitest";

import { getPageTitle, navigationItems } from "./navigation";

test("orders and names the primary navigation", () => {
  expect(navigationItems.map(({ label, path }) => [label, path])).toEqual([
    ["系统总览", "/"],
    ["上游提供", "/providers"],
    ["认证文件", "/oauth"],
    ["网关密钥", "/keys"],
    ["出口代理", "/proxies"],
    ["路由检查", "/routes"],
    ["额度费率", "/quota-rates"],
    ["请求日志", "/logs"],
    ["系统日志", "/system-logs"],
    ["系统设置", "/settings"],
  ]);
  expect(getPageTitle("/")).toBe("系统总览");
  expect(getPageTitle("/routes")).toBe("路由检查");
  expect(getPageTitle("/quota-rates")).toBe("额度费率");
  expect(navigationItems.find(({ path }) => path === "/oauth")?.icon).toBe(FileKey2);
});
