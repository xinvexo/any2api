/** Merged settings navigation sections (coarser than backend web_group). */
export interface SettingSection {
  id: string;
  label: string;
  webGroups: readonly string[];
}

/**
 * High-level categories for the settings UI.
 * Backend web_group values stay as subsection titles inside a section.
 */
export const SETTING_SECTIONS: readonly SettingSection[] = [
  {
    id: "admin",
    label: "远程管理",
    webGroups: ["远程管理"],
  },
  {
    id: "scheduler",
    label: "调度",
    webGroups: ["排队策略", "辅助请求"],
  },
  {
    id: "reliability",
    label: "重试与熔断",
    webGroups: ["重试预算", "重试退避", "冷却", "Endpoint 熔断", "代理熔断", "熔断探测"],
  },
  {
    id: "upstream",
    label: "上游与运行",
    webGroups: ["上游网络", "流式预提交", "流式响应", "优雅停机"],
  },
  {
    id: "affinity",
    label: "会话粘性",
    webGroups: ["软会话粘性", "硬会话粘性", "固定会话等待"],
  },
  {
    id: "logging",
    label: "日志",
    webGroups: ["请求日志", "本地文件日志"],
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
