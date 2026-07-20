import { expect, test } from "vitest";

import type { SettingItem } from "../api/settings-contracts";
import { settingLabel } from "./setting-presentation";

test("labels upstream and postcommit timeout settings", () => {
  expect(settingLabel(item("upstream.read_timeout"))).toBe("上游读取超时");
  expect(settingLabel(item("stream.postcommit.idle_timeout"))).toBe("提交后流空闲超时");
});

function item(key: string): SettingItem {
  return {
    key,
    valueType: "duration_ms",
    defaultValue: 1,
    overrideValue: null,
    effectiveValue: 1,
    minValue: 1,
    maxValue: 86_400_000,
    allowedValues: null,
    applyMode: "hot_reload",
    webGroup: "Test",
    description: "Test setting",
  };
}
