/** Merged settings navigation sections (coarser than backend web_group). */
export interface SettingSection {
  id: string;
  label: string;
  webGroups: readonly string[];
  featuredKeys: readonly string[];
}

/**
 * High-level categories for the settings UI.
 * Backend web_group values stay as subsection titles inside a section.
 */
export const SETTING_SECTIONS: readonly SettingSection[] = [
  {
    id: "basic",
    label: "基础",
    webGroups: ["远程管理"],
    featuredKeys: ["admin.remote_enabled"],
  },
  {
    id: "routing",
    label: "路由策略",
    webGroups: ["排队策略", "软会话粘性", "硬会话粘性", "固定会话等待"],
    featuredKeys: [
      "scheduler.on_rate_limited",
      "affinity.soft.enabled",
      "affinity.soft.mode",
    ],
  },
  {
    id: "protection",
    label: "运行保护",
    webGroups: [
      "重试预算",
      "重试退避",
      "冷却",
      "Endpoint 熔断",
      "代理熔断",
      "熔断探测",
      "上游网络",
      "OAuth 刷新",
      "流式预提交",
      "流式响应",
      "优雅停机",
    ],
    featuredKeys: [
      "retry.max_total_attempts",
      "upstream.read_timeout",
      "upstream.strict_ssrf",
    ],
  },
  {
    id: "logging",
    label: "日志",
    webGroups: ["请求日志", "本地文件日志"],
    featuredKeys: ["logs.request.enabled", "logs.file.level"],
  },
] as const;

const webGroupToSection = new Map<string, SettingSection>();
for (const section of SETTING_SECTIONS) {
  for (const group of section.webGroups) {
    webGroupToSection.set(group, section);
  }
}

export function sectionForWebGroup(webGroup: string): SettingSection {
  return (
    webGroupToSection.get(webGroup) ?? {
      id: `other:${webGroup}`,
      label: webGroup,
      webGroups: [webGroup],
      featuredKeys: [],
    }
  );
}

/** Build ordered sections that actually contain at least one of the given web groups. */
export function sectionsForWebGroups(webGroups: Iterable<string>): SettingSection[] {
  const present = new Set(webGroups);
  const seen = new Set<string>();
  const sections: SettingSection[] = [];

  for (const section of SETTING_SECTIONS) {
    if (section.webGroups.some((group) => present.has(group))) {
      sections.push(section);
      seen.add(section.id);
    }
  }

  for (const group of present) {
    const section = sectionForWebGroup(group);
    if (!seen.has(section.id)) {
      sections.push(section);
      seen.add(section.id);
    }
  }

  return sections;
}
