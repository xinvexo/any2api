import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import {
  APPLICATION_UPDATE_CONFIRMATION_TIMEOUT_MS,
  APPLICATION_UPDATE_PENDING_TARGET_KEY,
  ApplicationUpdateProvider,
} from "../model/ApplicationUpdateProvider";
import {
  APPLICATION_RESTART_CONFIRMATION_TIMEOUT_MS,
  APPLICATION_RESTART_PENDING_INSTANCE_KEY,
} from "../model/application-restart-flow";
import { AboutSettings } from "./AboutSettings";

const { reloadApplicationMock } = vi.hoisted(() => ({
  reloadApplicationMock: vi.fn(),
}));

vi.mock("../model/reload-application", () => ({
  reloadApplication: reloadApplicationMock,
}));

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  reloadApplicationMock.mockReset();
  window.sessionStorage.clear();
  document.getElementById("root")?.remove();
});

test("locks the page, shows progress, and reloads after the target build is healthy", async () => {
  let phase = "downloading";
  let healthVersion = "1.0.0";
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/check") && init?.method === "POST") {
      return jsonResponse(updateCheck());
    }
    if (path.endsWith("/update/install") && init?.method === "POST") {
      return jsonResponse(updateStatus({ phase: "checking" }), 202);
    }
    if (path.endsWith("/update/status")) {
      return jsonResponse(phase === "downloading"
        ? updateStatus({
            phase,
            target_version: "1.1.0",
            downloaded_bytes: 512,
            total_bytes: 1024,
          })
        : updateStatus({ phase, target_version: "1.1.0" }));
    }
    if (path.endsWith("/api/health")) {
      return jsonResponse(health(healthVersion));
    }
    return jsonResponse(about());
  });
  renderAbout();

  expect(await screen.findByText("v1.0.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("发现新版本 v1.1.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "更新到 v1.1.0" }));

  expect(await screen.findByRole("dialog", { name: "正在下载 v1.1.0" })).toBeInTheDocument();
  expect(screen.getByRole("progressbar", { name: "更新下载进度" })).toHaveAttribute(
    "aria-valuenow",
    "50",
  );
  expect(screen.queryByRole("button", { name: "返回" })).not.toBeInTheDocument();
  expect(document.getElementById("root")).toHaveAttribute("inert");
  const unload = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(unload);
  expect(unload.defaultPrevented).toBe(true);

  phase = "installing";
  expect(await screen.findByRole("dialog", { name: "正在安装" })).toBeInTheDocument();
  phase = "restarting";
  expect(await screen.findByRole("dialog", { name: "正在重新启动" })).toBeInTheDocument();
  healthVersion = "1.1.0";
  expect(await screen.findByRole("dialog", { name: "更新完成" })).toBeInTheDocument();
  await waitFor(() => expect(reloadApplicationMock).toHaveBeenCalledOnce(), { timeout: 2_000 });
});

test.each([
  {
    name: "another client already started the task",
    start: () => Promise.resolve(new Response(
      JSON.stringify({
        error: {
          code: "update_in_progress",
          message: "an application update is already in progress",
        },
      }),
      { status: 409, headers: { "Content-Type": "application/json" } },
    )),
  },
  {
    name: "the accepted install response was lost",
    start: () => Promise.reject(new TypeError("connection closed")),
  },
])("keeps the page locked when $name", async ({ start }) => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/check") && init?.method === "POST") {
      return jsonResponse(updateCheck());
    }
    if (path.endsWith("/update/install") && init?.method === "POST") {
      return start();
    }
    if (path.endsWith("/update/status")) {
      return jsonResponse(updateStatus({
        phase: "downloading",
        target_version: "1.1.0",
        downloaded_bytes: 512,
        total_bytes: 1024,
      }));
    }
    if (path.endsWith("/api/health")) {
      return jsonResponse(health("1.0.0"));
    }
    return jsonResponse(about());
  });
  renderAbout();

  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  fireEvent.click(await screen.findByRole("button", { name: "更新到 v1.1.0" }));

  expect(await screen.findByRole("dialog", { name: "正在下载 v1.1.0" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "返回" })).not.toBeInTheDocument();
  expect(document.getElementById("root")).toHaveAttribute("inert");
});

test("only unlocks the full-screen flow after the update itself fails", async () => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/check") && init?.method === "POST") {
      return jsonResponse(updateCheck());
    }
    if (path.endsWith("/update/install") && init?.method === "POST") {
      return new Response(
        JSON.stringify({
          error: {
            code: "update_unsupported",
            message: "this build cannot replace itself",
          },
        }),
        { status: 409, headers: { "Content-Type": "application/json" } },
      );
    }
    return jsonResponse(about());
  });
  renderAbout();

  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  fireEvent.click(await screen.findByRole("button", { name: "更新到 v1.1.0" }));

  expect(await screen.findByRole("dialog", { name: "更新未完成" })).toBeInTheDocument();
  expect(screen.getByText("当前运行环境不支持自动更新。")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "重新尝试" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(document.getElementById("root")).not.toHaveAttribute("inert");
});

test("offers bounded recovery when the target version cannot be confirmed", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-08-03T00:00:00Z"));
  window.sessionStorage.setItem(APPLICATION_UPDATE_PENDING_TARGET_KEY, "1.1.0");
  let installRequests = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/install") && init?.method === "POST") {
      installRequests += 1;
    }
    if (path.endsWith("/update/status") || path.endsWith("/api/health")) {
      throw new TypeError("service unavailable");
    }
    return jsonResponse(about());
  });
  renderAbout();

  expect(screen.getByRole("dialog", { name: "正在准备更新" })).toBeInTheDocument();
  const lockedUnload = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(lockedUnload);
  expect(lockedUnload.defaultPrevented).toBe(true);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(APPLICATION_UPDATE_CONFIRMATION_TIMEOUT_MS - 1);
  });
  expect(screen.queryByRole("button", { name: "返回" })).not.toBeInTheDocument();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(screen.getByRole("dialog", { name: "无法确认更新结果" })).toBeInTheDocument();
  expect(screen.getByText(/连续 90 秒未能确认 v1\.1\.0/)).toBeInTheDocument();
  expect(window.sessionStorage.getItem(APPLICATION_UPDATE_PENDING_TARGET_KEY)).toBeNull();
  const unlockedUnload = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(unlockedUnload);
  expect(unlockedUnload.defaultPrevented).toBe(false);

  fireEvent.click(screen.getByRole("button", { name: "继续等待" }));
  expect(screen.getByRole("dialog", { name: "正在重新启动" })).toBeInTheDocument();
  expect(window.sessionStorage.getItem(APPLICATION_UPDATE_PENDING_TARGET_KEY)).toBe("1.1.0");
  expect(installRequests).toBe(0);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(APPLICATION_UPDATE_CONFIRMATION_TIMEOUT_MS);
  });
  expect(screen.getByRole("dialog", { name: "无法确认更新结果" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(document.getElementById("root")).not.toHaveAttribute("inert");
  expect(window.sessionStorage.getItem(APPLICATION_UPDATE_PENDING_TARGET_KEY)).toBeNull();
  expect(installRequests).toBe(0);
});

test("restarts the unavailable deadline after an authoritative active status", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-08-03T00:00:00Z"));
  window.sessionStorage.setItem(APPLICATION_UPDATE_PENDING_TARGET_KEY, "1.1.0");
  let returnActiveStatus = false;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path.endsWith("/update/status")) {
      if (returnActiveStatus) {
        returnActiveStatus = false;
        return jsonResponse(updateStatus({ phase: "restarting", target_version: "1.1.0" }));
      }
      throw new TypeError("status unavailable");
    }
    if (path.endsWith("/api/health")) {
      throw new TypeError("health unavailable");
    }
    return jsonResponse(about());
  });
  renderAbout();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });
  returnActiveStatus = true;
  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });
  expect(screen.getByRole("dialog", { name: "正在重新启动" })).toBeInTheDocument();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(40_000);
  });
  expect(screen.queryByRole("dialog", { name: "无法确认更新结果" })).not.toBeInTheDocument();
  expect(screen.getByRole("dialog", { name: "正在重新启动" })).toBeInTheDocument();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(51_000);
  });
  expect(screen.getByRole("dialog", { name: "无法确认更新结果" })).toBeInTheDocument();
});

test("treats three idle observations as a definitive stopped update", async () => {
  vi.useFakeTimers();
  window.sessionStorage.setItem(APPLICATION_UPDATE_PENDING_TARGET_KEY, "1.1.0");
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path.endsWith("/update/status")) {
      return jsonResponse(updateStatus({ phase: "idle" }));
    }
    if (path.endsWith("/api/health")) {
      return jsonResponse(health("1.0.0"));
    }
    return jsonResponse(about());
  });
  renderAbout();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(1_000);
  });
  expect(screen.getByRole("dialog", { name: "更新未完成" })).toBeInTheDocument();
  expect(screen.getByText("更新任务已中止，当前版本未发生变化。")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "重新尝试" })).toBeInTheDocument();
  expect(window.sessionStorage.getItem(APPLICATION_UPDATE_PENDING_TARGET_KEY)).toBeNull();
});

test("requires confirmation before requesting a manual restart", async () => {
  let healthRequests = 0;
  let restartRequests = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/api/health")) {
      healthRequests += 1;
      return jsonResponse(health("1.0.0"));
    }
    if (path.endsWith("/api/admin/restart") && init?.method === "POST") {
      restartRequests += 1;
      return jsonResponse({ status: "restarting" }, 202);
    }
    return jsonResponse(about());
  });
  renderAbout();

  fireEvent.click(await screen.findByRole("button", { name: "重启服务" }));
  expect(await screen.findByRole("alertdialog", { name: "重启 ANY2API？" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));

  await waitFor(() => {
    expect(screen.queryByRole("alertdialog", { name: "重启 ANY2API？" })).not.toBeInTheDocument();
  });
  expect(healthRequests).toBe(0);
  expect(restartRequests).toBe(0);
});

test.each([
  { name: "returns its acknowledgement", loseAcknowledgement: false },
  { name: "restarts before its acknowledgement reaches the browser", loseAcknowledgement: true },
])("waits for a new process instance when the restart request $name", async ({
  loseAcknowledgement,
}) => {
  let instanceId = INSTANCE_ONE;
  let restartRequests = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/api/health")) {
      return jsonResponse(health("1.0.0", instanceId));
    }
    if (path.endsWith("/api/admin/restart") && init?.method === "POST") {
      restartRequests += 1;
      if (loseAcknowledgement) {
        throw new TypeError("connection closed during restart");
      }
      return jsonResponse({ status: "restarting" }, 202);
    }
    return jsonResponse(about());
  });
  renderAbout();

  fireEvent.click(await screen.findByRole("button", { name: "重启服务" }));
  fireEvent.click(await screen.findByRole("button", { name: "重启" }));

  expect(await screen.findByRole("dialog", { name: "正在重新启动" })).toBeInTheDocument();
  expect(restartRequests).toBe(1);
  expect(window.sessionStorage.getItem(APPLICATION_RESTART_PENDING_INSTANCE_KEY)).toBe(
    INSTANCE_ONE,
  );
  expect(document.getElementById("root")).toHaveAttribute("inert");
  expect(screen.queryByRole("dialog", { name: "重启完成" })).not.toBeInTheDocument();

  instanceId = INSTANCE_TWO;
  expect(
    await screen.findByRole("dialog", { name: "重启完成" }, { timeout: 2_000 }),
  ).toBeInTheDocument();
  expect(window.sessionStorage.getItem(APPLICATION_RESTART_PENDING_INSTANCE_KEY)).toBeNull();
  expect(restartRequests).toBe(1);
  await waitFor(() => expect(reloadApplicationMock).toHaveBeenCalledOnce(), { timeout: 2_000 });
});

test("restores an accepted restart and offers bounded recovery without submitting it again", async () => {
  vi.useFakeTimers();
  const startedAt = new Date("2026-08-19T00:00:00Z");
  vi.setSystemTime(startedAt);
  window.sessionStorage.setItem(APPLICATION_RESTART_PENDING_INSTANCE_KEY, INSTANCE_ONE);
  let restartRequests = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/api/health")) {
      return jsonResponse(health("1.0.0", INSTANCE_ONE));
    }
    if (path.endsWith("/api/admin/restart") && init?.method === "POST") {
      restartRequests += 1;
    }
    return jsonResponse(about());
  });
  renderAbout();

  expect(screen.getByRole("dialog", { name: "正在重新启动" })).toBeInTheDocument();
  await act(async () => {
    await Promise.resolve();
  });
  vi.setSystemTime(startedAt.getTime() + APPLICATION_RESTART_CONFIRMATION_TIMEOUT_MS);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });

  expect(screen.getByRole("dialog", { name: "无法确认重启结果" })).toBeInTheDocument();
  expect(screen.getByText(/12 分钟未能确认新的服务实例/)).toBeInTheDocument();
  expect(restartRequests).toBe(0);

  fireEvent.click(screen.getByRole("button", { name: "继续等待" }));
  expect(screen.getByRole("dialog", { name: "正在重新启动" })).toBeInTheDocument();
  expect(restartRequests).toBe(0);
  expect(window.sessionStorage.getItem(APPLICATION_RESTART_PENDING_INSTANCE_KEY)).toBe(
    INSTANCE_ONE,
  );
});

function renderAbout() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const container = document.createElement("div");
  container.id = "root";
  document.body.append(container);
  return render(
    <QueryClientProvider client={client}>
      <ApplicationUpdateProvider>
        <AboutSettings />
      </ApplicationUpdateProvider>
    </QueryClientProvider>,
    { container },
  );
}

function about() {
  return {
    current_version: "1.0.0",
    repository_url: "https://github.com/xinvexo/any2api",
  };
}

const INSTANCE_ONE = "550e8400-e29b-41d4-a716-446655440000";
const INSTANCE_TWO = "550e8400-e29b-41d4-a716-446655440001";

function health(version: string, instanceId = INSTANCE_ONE) {
  return {
    status: "ok",
    application_version: version,
    instance_id: instanceId,
  };
}

function updateCheck() {
  return {
    current_version: "1.0.0",
    latest_version: "1.1.0",
    update_available: true,
    release_url: "https://github.com/xinvexo/any2api/releases/tag/v1.1.0",
    published_at: "2026-07-29T00:00:00Z",
  };
}

function updateStatus(overrides: Record<string, unknown>) {
  return {
    phase: "idle",
    target_version: null,
    downloaded_bytes: null,
    total_bytes: null,
    failure_code: null,
    ...overrides,
  };
}

function jsonResponse(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
