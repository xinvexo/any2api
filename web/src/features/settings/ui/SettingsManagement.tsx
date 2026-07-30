import { ChevronDown, RefreshCw } from "lucide-react";
import { useMemo } from "react";

import type { SettingItem } from "../api/settings-contracts";
import { getSettingsErrorMessage } from "../model/settings-error";
import type { SettingsEditor } from "../model/use-settings-editor";
import { sectionsForWebGroups, type SettingSection } from "./setting-categories";
import { SettingRow } from "./SettingRow";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export interface SettingsManagementProps {
  editor: SettingsEditor;
  /** Items shown before the collapsed advanced section. */
  featuredKeys?: readonly string[];
  /** When false, section titles are omitted (page tabs already label the section). */
  showSectionHeading?: boolean;
}

export function SettingsManagement({
  editor,
  featuredKeys,
  showSectionHeading = true,
}: SettingsManagementProps) {
  const query = editor.query;

  const featured = useMemo(
    () => (featuredKeys ? new Set(featuredKeys) : null),
    [featuredKeys],
  );

  const groups = useMemo(() => groupSettings(editor.items), [editor.items]);
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
        <Button className="mt-5" onClick={() => void editor.refresh()} disabled={query.isFetching}>
          <RefreshCw size={14} />
          重试
        </Button>
      </Surface>
    );
  }

  return (
    <div className="space-y-4" aria-busy={editor.pending}>
      {query.isError ? (
        <Surface className="border-warning/40 p-4 text-sm text-secondary" role="status">
          配置刷新失败，当前仍显示最近一次有效数据：{getSettingsErrorMessage(query.error)}
        </Surface>
      ) : null}

      {editor.saveError ? (
        <Surface className="border-danger/40 p-4 text-sm text-secondary" role="alert">
          保存失败，修改仍保留：{getSettingsErrorMessage(editor.saveError)}
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
              editor={editor}
              showHeading={showSectionHeading}
              featured={featured}
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
  editor,
  showHeading,
  featured,
}: {
  section: SettingSection;
  groups: [string, SettingItem[]][];
  editor: SettingsEditor;
  showHeading: boolean;
  featured: ReadonlySet<string> | null;
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
          editor={editor}
          showGroupHeading={primary.length > 1 || !showHeading}
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
                editor={editor}
                showGroupHeading
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
  editor,
  showGroupHeading,
}: {
  subsections: readonly (readonly [string, SettingItem[]])[];
  editor: SettingsEditor;
  showGroupHeading: boolean;
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
                value={editor.draftFor(item)}
                pending={editor.pending}
                onChange={editor.setDraft}
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

function groupSettings(items: SettingItem[]) {
  const grouped = new Map<string, SettingItem[]>();
  for (const item of items) {
    const group = grouped.get(item.webGroup) ?? [];
    group.push(item);
    grouped.set(item.webGroup, group);
  }
  return [...grouped.entries()];
}
