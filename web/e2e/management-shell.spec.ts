import { expect, test, type Page } from "@playwright/test";

const password = "any2api-e2e-password";

test("login preserves a direct settings link and refreshes the SPA route", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);

  await loginAt(page, "/settings", "设置");
  await expect(page).toHaveURL(/\/settings$/);
  await expect(page.getByRole("heading", { level: 1, name: "设置" })).toBeVisible();
  await expect(page.getByText("配置版本", { exact: false }).first()).toBeVisible();
  await expectNoHorizontalOverflow(page);

  await page.reload();
  await expect(page).toHaveURL(/\/settings$/);
  await expect(page.getByRole("heading", { level: 1, name: "设置" })).toBeVisible();
  expect(browserErrors).toEqual([]);
});

test("desktop core management deep links render against the real service", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);
  await loginAt(page, "/", "总览");

  for (const [path, heading, readyText] of [
    ["/", "总览", "运行正常"],
    ["/proxies", "代理", "代理列表"],
    ["/providers", "Provider", "Provider Endpoint"],
    ["/routes", "模型路由", "已发布路由"],
    ["/balancing", "负载均衡", "还没有 Provider Credential"],
    ["/keys", "网关密钥", "尚未创建网关密钥"],
    ["/logs", "请求日志", "还没有请求日志"],
  ] as const) {
    await page.goto(path);
    await expect(page.getByRole("heading", { level: 1, name: heading })).toBeVisible();
    await expect(page.getByText(readyText, { exact: false }).first()).toBeVisible();
    await expectNoHorizontalOverflow(page);
  }

  expect(browserErrors).toEqual([]);
});

test("390px navigation closes after a deep-link transition without horizontal overflow", async ({ page }) => {
  const browserErrors = watchBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await loginAt(page, "/settings", "设置");

  const menu = page.getByRole("button", { name: "打开导航" });
  await menu.click();
  const navigation = page.locator("#responsive-navigation").getByRole("navigation", {
    name: "主导航",
  });
  await expect(navigation).toBeVisible();
  await navigation.getByRole("link", { name: "请求日志" }).click();

  await expect(page).toHaveURL(/\/logs$/);
  await expect(page.getByRole("heading", { level: 1, name: "请求日志" })).toBeVisible();
  await expect(page.getByRole("button", { name: "打开导航" })).toBeVisible();
  await expect(page.getByText("还没有请求日志")).toBeVisible();
  await expectNoHorizontalOverflow(page);
  expect(browserErrors).toEqual([]);
});

async function loginAt(page: Page, path: string, heading: string) {
  await page.goto(path);
  await expect(page.getByRole("heading", { name: "管理员登录" })).toBeVisible();
  await page.getByLabel("管理员密码").fill(password);
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: heading })).toBeVisible();
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
