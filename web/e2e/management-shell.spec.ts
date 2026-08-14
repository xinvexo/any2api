import { expect, test, type Page } from "@playwright/test";

const password = "any2api-e2e-password";

test("settings expose the five current sections", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);

  await page.setViewportSize({ width: 390, height: 844 });
  await loginAt(page, "/settings", "允许远程管理");
  await expect(page).toHaveURL(/\/settings\/basic$/);
  await expect(page.getByText("允许远程管理", { exact: false }).first()).toBeVisible();
  await expectNoHorizontalOverflow(page);
  const settingsTabs = page.getByRole("navigation", { name: "系统设置分类" });
  await expect(settingsTabs.getByRole("link"))
    .toHaveText(["基础", "路由策略", "运行保护", "日志", "关于"]);
  await expect(settingsTabs).toHaveCSS("scrollbar-width", "none");
  await expect(page.locator(".management-scroll-viewport")).toHaveCSS("padding-right", "8px");
  const tabWidths = await settingsTabs.evaluate((element) => ({
    client: element.clientWidth,
    scroll: element.scrollWidth,
  }));
  expect(tabWidths.scroll).toBeGreaterThan(tabWidths.client);
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
    "额度费率",
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
    ["/quota-rates", "Codex 额度费率"],
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
  await expect(mobileRow).toHaveCSS("border-radius", "14px");
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
  await expect(row).toHaveCSS("border-radius", "14px");
  await expect(table.getByRole("columnheader", { name: "名称" })).toBeHidden();
  await expect(row.getByText("本机网络", { exact: true })).toBeVisible();
  await expect(row.locator("td").nth(4)).toContainText(/无需认证|无认证|—/);
  await expect(row.getByText("未测试", { exact: true })).toBeVisible();
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
  await autoRefresh.click();
  await expect(autoRefresh).toHaveAttribute("aria-checked", "false");

  for (let index = 0; index < 40; index += 1) {
    await page.request.get(`/api/e2e-virtual-row/${index}`);
  }
  await expect.poll(async () => {
    const response = await page.request.get("/api/admin/system-logs?page_size=50");
    const payload = await response.json() as { items: Array<{ path: string }> };
    return payload.items.filter((item) => item.path.startsWith("/api/e2e-virtual-row/")).length;
  }).toBe(40);

  const pageSizeChanged = page.waitForResponse((response) =>
    response.url().includes("/api/admin/system-logs?page_size=50"),
  );
  await page.getByRole("combobox", { name: "每页条数" }).click();
  await page.getByRole("option", { name: "50 条/页" }).click();
  await pageSizeChanged;
  await expect(page.getByRole("table", { name: "系统日志表格" })).toBeVisible();

  const refreshed = page.waitForResponse(
    (response) =>
      response.url().includes("/api/admin/system-logs?page_size=50") &&
      response.request().method() === "GET",
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

  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(autoRefresh).toHaveAttribute("aria-checked", "false");
  await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute("content", "#ffffff");
  await expect(page.locator('meta[name="viewport"]')).toHaveAttribute(
    "content",
    /viewport-fit=cover.*interactive-widget=resizes-content/,
  );
  const mobileList = page.getByRole("list", { name: "系统日志列表" });
  await expect(mobileList).toBeVisible();
  await expect(mobileList).toHaveCSS("padding-right", "8px");
  const mobileCard = mobileList.getByRole("listitem").first();
  await expect(mobileCard).toHaveCSS("border-radius", "14px");
  await expect(mobileCard).toHaveCSS("border-top-width", "0px");
  await expect(page.getByRole("button", { name: "刷新", exact: true })).toHaveCSS("width", "36px");
  await expect(page.getByRole("button", { name: "清理历史日志" })).toHaveCSS("width", "36px");
  await expectNoHorizontalOverflow(page);
  const contentGap = await mobileList.evaluate((element) => {
    if (!(element instanceof HTMLElement)) return 0;
    const card = element.querySelector('[role="listitem"]');
    if (!card) return 0;
    const scrollbarStart = element.getBoundingClientRect().right
      - (element.offsetWidth - element.clientWidth);
    return Math.round(scrollbarStart - card.getBoundingClientRect().right);
  });
  expect(contentGap).toBe(8);

  const main = page.locator("#main-content");
  const scrollMetrics = await page.evaluate(() => ({
    documentClientHeight: document.scrollingElement?.clientHeight ?? 0,
    documentScrollHeight: document.scrollingElement?.scrollHeight ?? 0,
    scrollingElement: document.scrollingElement?.tagName ?? "",
    shellOverflowY: getComputedStyle(document.querySelector(".app-shell")!).overflowY,
    mainOverflowY: getComputedStyle(document.querySelector("#main-content")!).overflowY,
  }));
  expect(scrollMetrics.scrollingElement).toBe("HTML");
  expect(scrollMetrics.documentScrollHeight).toBeGreaterThan(scrollMetrics.documentClientHeight);
  expect(scrollMetrics.shellOverflowY).toBe("visible");
  expect(scrollMetrics.mainOverflowY).toBe("visible");
  await expect(mobileList).toHaveCSS("overflow-y", "visible");
  await page.evaluate(() => window.scrollTo(0, 400));
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
  await expect.poll(() => main.evaluate((element) => element.scrollTop)).toBe(0);

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
