import { expect, test } from "vitest";

import type { SettingItem } from "../api/settings-contracts";
import { reloadLabel, settingLabel } from "./setting-presentation";

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
