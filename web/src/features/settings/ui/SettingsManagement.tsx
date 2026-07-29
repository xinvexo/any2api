import { ChevronDown, RefreshCw } from "lucide-react";
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
  /** Filter items belonging to any of these backend web groups. */
  webGroups?: readonly string[];
  /** Items shown before the collapsed advanced section. */
  featuredKeys?: readonly string[];
  /** When false, section titles are omitted (page tabs already label the section). */
  showSectionHeading?: boolean;
}

export function SettingsManagement({
  webGroups,
  featuredKeys,
  showSectionHeading = true,
}: SettingsManagementProps = {}) {
  const query = useSettings();
  const mutations = useSettingMutations();
  const pending = query.isFetching || mutations.isPending;
  const filteredItems = useMemo(() => {
    const allowed = webGroups ? new Set(webGroups) : null;
    return (query.data?.items ?? []).filter((item) => {
      if (allowed && !allowed.has(item.webGroup)) {
        return false;
      }
      return true;
    });
  }, [query.data, webGroups]);

  const featured = useMemo(
    () => (featuredKeys ? new Set(featuredKeys) : null),
    [featuredKeys],
  );

  const groups = useMemo(() => groupSettings(filteredItems), [filteredItems]);
  const sections = useMemo(
    () => sectionsForWebGroups(groups.map(([name]) => name)),
    [groups],
  );

  if (query.isPending && !query.data) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-busy="true">
        正在读取系统设置
      </div>
    );
  }
  if (!query.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取系统设置</p>
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
        <p className="py-10 text-center text-sm text-secondary">没有可显示的系统设置项</p>
      ) : (
        <div className="space-y-6">
          {sections.map((section) => (
            <SectionPanel
              key={section.id}
              section={section}
              groups={groups}
              pending={pending}
              mutations={mutations}
              showHeading={showSectionHeading}
              featured={featured}
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
  showHeading,
  featured,
  onSave,
  onReset,
}: {
  section: SettingSection;
  groups: [string, SettingItem[]][];
  pending: boolean;
  mutations: ReturnType<typeof useSettingMutations>;
  showHeading: boolean;
  featured: ReadonlySet<string> | null;
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

  const headingId = `settings-section-${cssId(section.id)}`;
  const primary = featured
    ? subsections
        .map(([name, items]) => [name, items.filter((item) => featured.has(item.key))] as const)
        .filter(([, items]) => items.length > 0)
    : subsections;
  const advanced = featured
    ? subsections
        .map(([name, items]) => [name, items.filter((item) => !featured.has(item.key))] as const)
        .filter(([, items]) => items.length > 0)
    : [];
  const advancedCount = advanced.reduce((total, [, items]) => total + items.length, 0);

  return (
    <section aria-labelledby={showHeading ? headingId : undefined} aria-label={showHeading ? undefined : section.label}>
      {showHeading ? (
        <header className="mb-2">
          <h2 id={headingId} className="text-[15px] font-semibold tracking-tight">
            {section.label}
          </h2>
        </header>
      ) : null}

      <div className="space-y-6">
        <SettingGroups
          subsections={primary}
          pending={pending}
          mutations={mutations}
          showGroupHeading={primary.length > 1 || !showHeading}
          onSave={onSave}
          onReset={onReset}
        />
        {advancedCount > 0 ? (
          <details className="group rounded-[10px] bg-surface-muted">
            <summary className="focus-ring flex cursor-pointer list-none items-center justify-between gap-3 rounded-[10px] px-3 py-3 text-sm font-medium marker:hidden [&::-webkit-details-marker]:hidden">
              <span>高级设置</span>
              <span className="flex items-center gap-2 text-xs font-normal text-tertiary">
                {advancedCount} 项
                <ChevronDown size={15} className="transition-transform group-open:rotate-180" aria-hidden="true" />
              </span>
            </summary>
            <div className="space-y-5 px-3 pb-3 pt-1">
              <SettingGroups
                subsections={advanced}
                pending={pending}
                mutations={mutations}
                showGroupHeading
                onSave={onSave}
                onReset={onReset}
              />
            </div>
          </details>
        ) : null}
      </div>
    </section>
  );
}

function SettingGroups({
  subsections,
  pending,
  mutations,
  showGroupHeading,
  onSave,
  onReset,
}: {
  subsections: readonly (readonly [string, SettingItem[]])[];
  pending: boolean;
  mutations: ReturnType<typeof useSettingMutations>;
  showGroupHeading: boolean;
  onSave: (item: SettingItem, value: SettingValue) => Promise<void>;
  onReset: (item: SettingItem) => Promise<void>;
}) {
  return (
    <div className="space-y-6">
      {subsections.map(([group, items]) => (
        <div key={group}>
          {showGroupHeading ? (
            <h3 className="mb-1.5 px-1 text-[12px] font-medium text-secondary">{group}</h3>
          ) : null}
          <div className="space-y-1">
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
