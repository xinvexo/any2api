import { expect, test, type Page } from "@playwright/test";

const password = "any2api-e2e-password";
const fixtureSecrets = [
  "e2e-codex-access-token",
  "e2e-claude-access-token",
  "e2e-grok-access-token",
] as const;

test("OAuth JSON accounts remain server-side and support editing, model selection, and deletion", async ({
  page,
}) => {
  const browserErrors = watchBrowserErrors(page);
  await loginAt(page, "/oauth", "还没有 Codex OAuth 账号");

  await page.getByRole("button", { name: "导入 JSON" }).click();
  const importDrawer = page.getByRole("dialog", { name: "导入 OAuth JSON" });
  await importDrawer.getByLabel("OAuth JSON 文件").setInputFiles({
    name: "oauth-e2e-fixture.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(oauthFixture())),
  });
  await expect(importDrawer.getByText("已选择 1 个文件")).toBeVisible();

  const importedResponse = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/admin/oauth/import") && response.request().method() === "POST",
  );
  await importDrawer.getByRole("button", { name: "导入并启用" }).click();
  const importBody = await (await importedResponse).text();
  expect(importBody).not.toContain("access_token");
  for (const secret of fixtureSecrets) expect(importBody).not.toContain(secret);

  await expect(page.getByText("已导入并启用 3 个 OAuth 账号。")).toBeVisible();
  await expect(page.getByText("E2E Codex", { exact: true })).toBeVisible();
  await assertSecretsAbsentFromPageState(page);

  await page.getByRole("button", { name: "编辑 E2E Codex" }).click();
  const editDrawer = page.getByRole("dialog", { name: "编辑 OAuth 账号" });
  await editDrawer.getByLabel("账号名称").fill("E2E Codex Updated");
  await editDrawer.getByLabel("RPM 限制").fill("17");
  await editDrawer.getByRole("button", { name: "保存" }).click();
  await expect(page.getByText("E2E Codex Updated", { exact: true })).toBeVisible();
  await expect(page.getByText("17", { exact: true }).first()).toBeVisible();

  await page
    .getByRole("button", { name: "查看 E2E Codex Updated 的可用模型" })
    .click();
  await expect(page).toHaveURL(/account=[^&]+&oauth_action=models/);
  await page.reload();

  const modelDrawer = page.getByRole("dialog", { name: "可用模型" });
  await expect(modelDrawer).toBeVisible();
  const model = modelDrawer.getByRole("checkbox").first();
  const modelName = await model.getAttribute("aria-label");
  expect(modelName).toBeTruthy();
  const initiallyChecked = await model.isChecked();
  if (initiallyChecked) {
    await model.uncheck();
  } else {
    await model.check();
  }
  await modelDrawer.getByRole("button", { name: "保存" }).click();
  await expect(modelDrawer).toBeHidden();

  await page
    .getByRole("button", { name: "查看 E2E Codex Updated 的可用模型" })
    .click();
  const persistedModel = page.getByRole("dialog", { name: "可用模型" }).getByRole("checkbox", {
    name: modelName!,
  });
  await expect(persistedModel).toBeChecked({ checked: !initiallyChecked });
  await page
    .getByRole("dialog", { name: "可用模型" })
    .getByRole("button", { name: "关闭", exact: true })
    .last()
    .click();

  await page.setViewportSize({ width: 390, height: 844 });
  await selectProvider(page, "Claude");
  await expect(page.getByText("E2E Claude", { exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "Claude 额度" })).toBeVisible();
  await expect(page.getByRole("button", { name: "重置额度" })).toHaveCount(0);
  await page.getByRole("button", { name: "编辑 E2E Claude" }).click();
  await expect(page.getByRole("dialog", { name: "编辑 OAuth 账号" })).toBeVisible();
  await expectNoHorizontalOverflow(page);
  await page.getByRole("dialog", { name: "编辑 OAuth 账号" }).getByRole("button", { name: "取消" }).click();

  await selectProvider(page, "Grok");
  await expect(page.getByText("E2E Grok", { exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "Grok 额度" })).toBeVisible();
  await expect(page.getByRole("button", { name: "重置额度" })).toHaveCount(0);
  await expectNoHorizontalOverflow(page);

  await deleteAccount(page, "E2E Grok");
  await selectProvider(page, "Claude");
  await deleteAccount(page, "E2E Claude");
  await selectProvider(page, "Codex");
  await deleteAccount(page, "E2E Codex Updated");
  await expect(page.getByText("还没有 Codex OAuth 账号")).toBeVisible();
  await assertSecretsAbsentFromPageState(page);
  expect(browserErrors).toEqual([]);
});

function oauthFixture() {
  return [
    {
      type: "codex",
      name: "E2E Codex",
      access_token: fixtureSecrets[0],
      email: "codex-e2e@example.invalid",
    },
    {
      type: "claude",
      name: "E2E Claude",
      access_token: fixtureSecrets[1],
      email: "claude-e2e@example.invalid",
    },
    {
      type: "xai",
      name: "E2E Grok",
      access_token: fixtureSecrets[2],
      email: "grok-e2e@example.invalid",
    },
  ];
}

async function selectProvider(page: Page, label: "Codex" | "Claude" | "Grok") {
  await page
    .getByRole("navigation", { name: "OAuth2 类型" })
    .getByRole("button", { name: new RegExp(`^${label}\\s`) })
    .click();
}

async function deleteAccount(page: Page, label: string) {
  await page.getByRole("button", { name: `删除 ${label}` }).click();
  const dialog = page.getByRole("alertdialog", { name: "删除 OAuth 账号" });
  await expect(dialog).toContainText(label);
  await dialog.getByRole("button", { name: "删除", exact: true }).click();
  await expect(page.getByText(label, { exact: true })).toHaveCount(0);
}

async function loginAt(page: Page, path: string, readyText: string) {
  await page.goto(path);
  await expect(page.getByRole("heading", { name: "any2api" })).toBeVisible();
  await page.getByLabel("管理员密码").fill(password);
  await page.getByRole("button", { name: "进入控制台", exact: true }).click();
  await expect(page.getByText(readyText, { exact: false }).first()).toBeVisible();
}

async function assertSecretsAbsentFromPageState(page: Page) {
  const state = await page.evaluate(() => ({
    html: document.documentElement.outerHTML,
    localStorage: { ...window.localStorage },
    sessionStorage: { ...window.sessionStorage },
  }));
  const serialized = JSON.stringify(state);
  for (const secret of fixtureSecrets) expect(serialized).not.toContain(secret);
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
