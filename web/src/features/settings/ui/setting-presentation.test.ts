import { expect, test } from "vitest";

import type { SettingItem } from "../api/settings-contracts";
import {
  enumOptionLabel,
  reloadLabel,
  settingDescription,
  settingLabel,
} from "./setting-presentation";

test("labels the restart-required inbound connection limit", () => {
  const item: SettingItem = {
    key: "network.max_connections",
    valueType: "integer",
    defaultValue: 4_096,
    overrideValue: null,
    effectiveValue: 4_096,
    minValue: 1,
    maxValue: 100_000,
    allowedValues: null,
    options: null,
    applyMode: "restart_required",
    webGroup: "入站网络",
    description: "Inbound TCP connection limit",
  };

  expect(settingLabel(item)).toBe("入站连接数上限");
  expect(reloadLabel(item)).toBe("修改后需要重启");
});

test("uses user-facing capacity language for byte settings", () => {
  const item: SettingItem = {
    key: "logs.file.max_total_size",
    valueType: "integer",
    defaultValue: 256 * 1024 * 1024,
    overrideValue: null,
    effectiveValue: 256 * 1024 * 1024,
    minValue: 1024 * 1024,
    maxValue: 64 * 1024 * 1024 * 1024,
    allowedValues: null,
    options: null,
    applyMode: "hot_reload",
    webGroup: "本地文件日志",
    description: "本地 JSONL 日志目录允许占用的最大总字节数。",
  };

  expect(settingDescription(item)).toBe("本地 JSONL 日志目录允许占用的最大总容量。");
});

test("labels inbound slowloris protection settings", () => {
  const header: SettingItem = {
    key: "network.request_header_timeout",
    valueType: "duration_secs",
    defaultValue: 30,
    overrideValue: null,
    effectiveValue: 30,
    minValue: 1,
    maxValue: 86_400,
    allowedValues: null,
    options: null,
    applyMode: "restart_required",
    webGroup: "入站网络",
    description: "header timeout",
  };
  const body = { ...header, key: "network.request_body_idle_timeout", defaultValue: 60, effectiveValue: 60, applyMode: "restart_required" as const };

  expect(settingLabel(header)).toBe("HTTP 请求头读取超时");
  expect(settingLabel(body)).toBe("请求体空闲超时");
  expect(reloadLabel(header)).toBe("修改后需要重启");
  expect(reloadLabel(body)).toBe("修改后需要重启");
});

test("keeps logging level enum values in their protocol form", () => {
  expect([
    "error",
    "warn",
    "info",
    "debug",
    "trace",
  ].map((value) => enumOptionLabel(value))).toEqual([
    "error",
    "warn",
    "info",
    "debug",
    "trace",
  ]);
});
