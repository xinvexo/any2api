import { expect, test, type Page } from "@playwright/test";

const password = "any2api-e2e-password";

test("settings expose the five current sections", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);

  await loginAt(page, "/settings", "允许远程管理");
  await expect(page).toHaveURL(/\/settings\/basic$/);
  await expect(page.getByText("允许远程管理", { exact: false }).first()).toBeVisible();
  await expectNoHorizontalOverflow(page);
  await expect(page.getByRole("navigation", { name: "系统设置分类" }).getByRole("link"))
    .toHaveText(["基础", "路由策略", "运行保护", "日志", "关于"]);
  expect(browserErrors).toEqual([]);
});

test("desktop core management deep links render against the real service", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);
  await loginAt(page, "/", "运行正常");
  await expect(
    page.locator("#desktop-navigation").getByRole("navigation", { name: "主导航" }).getByRole("link"),
  ).toHaveText([
    "系统总览",
    "上游提供",
    "认证文件",
    "网关密钥",
    "出口代理",
    "请求日志",
    "系统日志",
    "系统设置",
  ]);

  for (const [path, readyText] of [
    ["/", "运行正常"],
    ["/oauth", "还没有 Codex OAuth 账号"],
    ["/proxies", "代理列表"],
    ["/providers?kind=codex", "还没有 Codex Endpoint"],
    ["/settings/routing", "RPM 用尽行为"],
    ["/keys", "尚未创建网关密钥"],
    ["/logs", "还没有请求日志"],
    ["/system-logs", "自动刷新"],
  ] as const) {
    await page.goto(path);
    await expect(page.getByText(readyText, { exact: false }).first()).toBeVisible();
    await expectNoHorizontalOverflow(page);
  }

  expect(browserErrors).toEqual([]);
});

test("gateway key usage is a fixed time axis with hover and keyboard details", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);
  await loginAt(page, "/keys", "尚未创建网关密钥");

  await page.getByRole("button", { name: "新增", exact: true }).click();
  await page.getByLabel("名称").fill("E2E 时间轴");
  await page.getByRole("button", { name: "保存", exact: true }).click();

  const timeline = page.getByRole("group", {
    name: /E2E 时间轴 近 1 小时，每格 2 分钟/,
  });
  await expect(timeline).toBeVisible();
  const slots = timeline.getByRole("button");
  await expect(slots).toHaveCount(30);

  await slots.last().hover();
  let tooltip = page.getByRole("tooltip");
  await expect(tooltip).toContainText("成功 0");
  await expect(tooltip).toContainText("失败 0");
  await expect(tooltip.locator("p").first()).toHaveText(
    /^\d{2}:\d{2}–\d{2}:\d{2}·无调用$/,
  );

  await slots.nth(10).focus();
  tooltip = page.getByRole("tooltip");
  await expect(tooltip).toBeVisible();
  await expect(tooltip).toContainText("成功 0");

  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  const mobileTable = page.getByRole("table", { name: "网关密钥列表" });
  const mobileRow = mobileTable.locator("tbody > tr");
  await expect(mobileRow).toHaveCount(1);
  await expect(mobileRow).toHaveCSS("display", "grid");
  await expect(mobileRow).toHaveCSS("border-radius", "8px");
  await expect(mobileTable.getByRole("columnheader", { name: "名称" })).toBeHidden();
  await expect(mobileRow.getByText("调用统计", { exact: true })).toBeVisible();
  await expect(mobileRow.getByText("最后使用", { exact: true })).toBeVisible();
  await expect(mobileRow.getByText("创建时间", { exact: true })).toBeVisible();
  await expect(mobileRow.getByRole("switch", { name: "禁用 E2E 时间轴" })).toHaveAttribute(
    "aria-checked",
    "true",
  );
  await expect(mobileRow.getByRole("button", { name: "复制 E2E 时间轴 的密钥" })).toBeVisible();
  await expect(page.getByRole("group", { name: /E2E 时间轴 近 1 小时，每格 2 分钟/ })).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.setViewportSize({ width: 1280, height: 720 });
  await page.reload();
  const desktopTable = page.getByRole("table", { name: "网关密钥列表" });
  await expect(desktopTable.locator("tbody > tr")).toHaveCSS("display", "table-row");
  await expect(desktopTable.getByRole("columnheader", { name: "名称" })).toBeVisible();
  await page.getByRole("button", { name: "删除 E2E 时间轴" }).click();
  const dialog = page.getByRole("alertdialog", { name: "删除「E2E 时间轴」？" });
  await dialog.getByRole("button", { name: "确认删除" }).click();
  await expect(page.getByText("尚未创建网关密钥")).toBeVisible();
  expect(browserErrors).toEqual([]);
});

test("proxy rows reflow into single mobile cards and return to desktop rows", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await loginAt(page, "/proxies", "代理列表");

  const table = page.getByRole("table", { name: "出口代理列表" });
  const row = table.locator("tbody > tr");
  await expect(row).toHaveCount(1);
  await expect(row).toHaveCSS("display", "grid");
  await expect(row).toHaveCSS("border-radius", "8px");
  await expect(table.getByRole("columnheader", { name: "名称" })).toBeHidden();
  await expect(row.getByText("地址", { exact: true })).toBeVisible();
  await expect(row.getByText("认证", { exact: true })).toBeVisible();
  await expect(row.getByText("连通性", { exact: true })).toBeVisible();
  await expect(row.getByRole("button", { name: "测试 DIRECT" })).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.setViewportSize({ width: 1280, height: 720 });
  await page.reload();
  await expect(table.locator("tbody > tr")).toHaveCount(1);
  await expect(table.locator("tbody > tr")).toHaveCSS("display", "table-row");
  await expect(table.getByRole("columnheader", { name: "名称" })).toBeVisible();
  expect(browserErrors).toEqual([]);
});

test("system logs refresh and clear on desktop and mobile", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);
  await loginAt(page, "/system-logs", "自动刷新");

  const autoRefresh = page.getByRole("switch", { name: "自动刷新" });
  await expect(autoRefresh).toHaveAttribute("aria-checked", "true");
  const automaticRefresh = page.waitForResponse(
    (response) =>
      response.url().includes("/api/admin/system-logs?limit=")
      && response.request().headers()["x-any2api-system-log-refresh"] === "automatic",
  );
  await automaticRefresh;
  await autoRefresh.click();
  await expect(autoRefresh).toHaveAttribute("aria-checked", "false");
  const table = page.getByRole("table", { name: "系统日志表格" });
  await expect(table).toBeVisible();

  for (let index = 0; index < 80; index += 1) {
    await page.request.get(`/api/e2e-virtual-row/${index}`);
  }
  await expect.poll(async () => {
    const response = await page.request.get("/api/admin/system-logs?limit=200");
    const payload = await response.json() as { items: Array<{ path: string }> };
    return payload.items.filter((item) => item.path.startsWith("/api/e2e-virtual-row/")).length;
  }).toBe(80);
  const refreshed = page.waitForResponse(
    (response) =>
      response.url().includes("/api/admin/system-logs?limit=") &&
      response.request().method() === "GET" &&
      response.request().headers()["x-any2api-system-log-refresh"] === undefined,
  );
  await page.getByRole("button", { name: "刷新", exact: true }).click();
  const refreshedPayload = await (await refreshed).json() as { items: Array<{ path: string }> };
  const targetPath = "/api/e2e-virtual-row/0";
  const targetIndex = refreshedPayload.items.findIndex((item) => item.path === targetPath);
  expect(targetIndex).toBeGreaterThanOrEqual(0);

  const header = page.getByRole("rowgroup", { name: "系统日志表头" });
  const rows = page.getByRole("rowgroup", { name: "系统日志表格数据" });
  await expect(rows.getByText(targetPath)).toHaveCount(0);
  expect(await rows.getByRole("row").count()).toBeLessThan(40);
  const headerBox = await header.boundingBox();
  const rowsBox = await rows.boundingBox();
  expect(headerBox).not.toBeNull();
  expect(rowsBox).not.toBeNull();
  expect(rowsBox!.y).toBeGreaterThanOrEqual(headerBox!.y + headerBox!.height - 1);

  await rows.evaluate((element, index) => {
    element.scrollTop = Math.max(0, (index - 3) * 41);
    element.dispatchEvent(new Event("scroll"));
  }, targetIndex);
  await expect(rows.getByText(targetPath)).toBeVisible();

  await page.getByRole("button", { name: "清理历史日志" }).click();
  const dialog = page.getByRole("alertdialog", { name: "清理历史系统日志？" });
  await expect(dialog).toBeVisible();
  const cleared = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/admin/system-logs") &&
      response.request().method() === "DELETE",
  );
  await dialog.getByRole("button", { name: "清理", exact: true }).click();
  await cleared;
  await expect(dialog).toBeHidden();

  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(autoRefresh).toHaveAttribute("aria-checked", "false");
  await expect(page.getByRole("list", { name: "系统日志列表" })).toBeVisible();
  await expectNoHorizontalOverflow(page);
  expect(browserErrors).toEqual([]);
});

test("390px OAuth navigation closes after a deep-link transition without horizontal overflow", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await loginAt(page, "/settings", "允许远程管理");

  const menu = page.getByRole("button", { name: "打开导航" });
  await menu.click();
  const navigation = page.locator("#responsive-navigation").getByRole("navigation", {
    name: "主导航",
  });
  await expect(navigation).toBeVisible();
  const panelBounds = await page.locator("#responsive-navigation").boundingBox();
  expect(panelBounds).not.toBeNull();
  expect(panelBounds!.x).toBeGreaterThan(0);
  expect(panelBounds!.y).toBeGreaterThan(56);
  expect(panelBounds!.width).toBeLessThan(300);
  expect(panelBounds!.height).toBeLessThan(500);
  await expect(
    page.locator("#responsive-navigation").getByRole("button", { name: "关闭导航" }),
  ).toHaveCount(0);
  await navigation.getByRole("link", { name: "认证文件" }).click();

  await expect(page).toHaveURL(/\/oauth$/);
  await expect(page.getByRole("button", { name: "打开导航" })).toBeVisible();
  await expect(page.getByText("还没有 Codex OAuth 账号")).toBeVisible();
  await expectNoHorizontalOverflow(page);
  expect(browserErrors).toEqual([]);
});

async function loginAt(page: Page, path: string, readyText: string) {
  await page.goto(path);
  await expect(page.getByRole("heading", { name: "any2api" })).toBeVisible();
  await page.getByLabel("管理员密码").fill(password);
  await page.getByRole("button", { name: "进入控制台", exact: true }).click();
  await expect(page.getByText(readyText, { exact: false }).first()).toBeVisible();
}

async function expectNoHorizontalOverflow(page: Page) {
  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))
    .toBe(true);
}

function watchBrowserErrors(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console: ${message.text()}`);
  });
  return errors;
}
