import { RefreshCw } from "lucide-react";
import { useMemo } from "react";

import type { SettingItem, SettingValue } from "../api/settings-contracts";
import { getSettingsErrorMessage } from "../model/settings-error";
import { useSettingMutations } from "../model/use-setting-mutations";
import { useSettings } from "../model/use-settings";
import { sectionsForWebGroups, type SettingSection } from "./setting-categories";
import { SettingRow } from "./SettingRow";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export interface SettingsManagementProps {
  /** Filter items whose key starts with this prefix. */
  keyPrefix?: string;
  /** Filter items belonging to a single backend web group. */
  webGroup?: string;
  /** Filter items belonging to any of these backend web groups. */
  webGroups?: readonly string[];
  /** @deprecated Sidebar owns categories; kept for call-site compatibility. */
  categorized?: boolean;
}

export function SettingsManagement({
  keyPrefix,
  webGroup,
  webGroups,
}: SettingsManagementProps = {}) {
  const query = useSettings();
  const mutations = useSettingMutations();
  const pending = query.isFetching || mutations.isPending;
  const filteredItems = useMemo(() => {
    const allowed = webGroups ? new Set(webGroups) : null;
    return (query.data?.items ?? []).filter((item) => {
      if (keyPrefix && !item.key.startsWith(keyPrefix)) {
        return false;
      }
      if (webGroup && item.webGroup !== webGroup) {
        return false;
      }
      if (allowed && !allowed.has(item.webGroup)) {
        return false;
      }
      return true;
    });
  }, [keyPrefix, query.data, webGroup, webGroups]);

  const groups = useMemo(() => groupSettings(filteredItems), [filteredItems]);
  const sections = useMemo(
    () => sectionsForWebGroups(groups.map(([name]) => name)),
    [groups],
  );

  if (query.isPending && !query.data) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-busy="true">
        正在读取设置
      </div>
    );
  }
  if (!query.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取设置</p>
        <p className="mt-2 text-sm text-secondary">{getSettingsErrorMessage(query.error)}</p>
        <Button className="mt-5" onClick={() => void query.refetch()} disabled={query.isFetching}>
          <RefreshCw size={14} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = query.data;

  async function save(item: SettingItem, value: SettingValue) {
    mutations.update.reset();
    mutations.reset.reset();
    await mutations.update.mutateAsync({
      key: item.key,
      input: { expectedRevision: configuration.configRevision, value },
    });
  }

  async function reset(item: SettingItem) {
    mutations.update.reset();
    mutations.reset.reset();
    await mutations.reset.mutateAsync({
      key: item.key,
      expectedRevision: configuration.configRevision,
    });
  }

  return (
    <div className="space-y-4" aria-busy={pending}>
      <div className="flex justify-end">
        <Button variant="ghost" onClick={() => void query.refetch()} disabled={pending}>
          <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
          刷新
        </Button>
      </div>

      {query.isError ? (
        <Surface className="border-warning/40 p-4 text-sm text-secondary" role="status">
          配置刷新失败，当前仍显示最近一次有效数据：{getSettingsErrorMessage(query.error)}
        </Surface>
      ) : null}

      {sections.length === 0 ? (
        <p className="py-10 text-center text-sm text-secondary">没有可显示的设置项</p>
      ) : (
        <div className="space-y-6">
          {sections.map((section) => (
            <SectionPanel
              key={section.id}
              section={section}
              groups={groups}
              pending={pending}
              mutations={mutations}
              onSave={save}
              onReset={reset}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function SectionPanel({
  section,
  groups,
  pending,
  mutations,
  onSave,
  onReset,
}: {
  section: SettingSection;
  groups: [string, SettingItem[]][];
  pending: boolean;
  mutations: ReturnType<typeof useSettingMutations>;
  onSave: (item: SettingItem, value: SettingValue) => Promise<void>;
  onReset: (item: SettingItem) => Promise<void>;
}) {
  const subsections = section.webGroups
    .map((name) => {
      const items = groups.find(([group]) => group === name)?.[1] ?? [];
      return [name, items] as const;
    })
    .filter(([, items]) => items.length > 0);

  for (const [name, items] of groups) {
    if (section.webGroups.includes(name)) {
      continue;
    }
    if (section.id === `other:${name}`) {
      subsections.push([name, items]);
    }
  }

  return (
    <section aria-labelledby={`settings-section-${cssId(section.id)}`}>
      <header className="mb-2">
        <h2
          id={`settings-section-${cssId(section.id)}`}
          className="text-[15px] font-semibold tracking-tight"
        >
          {section.label}
        </h2>
      </header>

      <div className="space-y-5">
        {subsections.map(([group, items]) => (
          <div key={group}>
            {subsections.length > 1 ? (
              <h3 className="mb-1 text-[12px] font-medium text-secondary">{group}</h3>
            ) : null}
            <div>
              {items.map((item) => (
                <SettingRow
                  key={item.key}
                  item={item}
                  pending={pending}
                  mutationError={mutationErrorFor(item.key, mutations.update, mutations.reset)}
                  onSave={onSave}
                  onReset={onReset}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function cssId(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]+/g, "-");
}

function mutationErrorFor(
  key: string,
  update: { error: unknown; variables?: { key: string } },
  reset: { error: unknown; variables?: { key: string } },
) {
  if (update.variables?.key === key && update.error) {
    return update.error;
  }
  if (reset.variables?.key === key && reset.error) {
    return reset.error;
  }
  return null;
}

function groupSettings(items: SettingItem[]) {
  const grouped = new Map<string, SettingItem[]>();
  for (const item of items) {
    const group = grouped.get(item.webGroup) ?? [];
    group.push(item);
    grouped.set(item.webGroup, group);
  }
  return [...grouped.entries()];
}
