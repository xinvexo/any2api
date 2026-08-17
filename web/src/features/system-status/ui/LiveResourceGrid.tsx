import { Cpu, Gauge, HardDrive, MemoryStick, type LucideIcon } from "lucide-react";

import type { OverviewResources } from "../api/overview-resources-contracts";
import {
  formatResourceBytes,
  formatResourcePercent,
  formatSystemMemory,
} from "../model/overview-resources-presentation";
import { OverviewMetricTile, type OverviewMetricTone } from "./OverviewMetricTile";

export function LiveResourceGrid({ resources }: { resources: OverviewResources | undefined }) {
  const systemMemory = resources
    ? formatSystemMemory(resources.system.usedMemoryBytes, resources.system.totalMemoryBytes)
    : null;
  const processMemoryShare = resources
    ? ratioPercent(resources.process.residentMemoryBytes, resources.system.totalMemoryBytes)
    : null;
  const systemMemoryPercent = resources
    ? ratioPercent(resources.system.usedMemoryBytes, resources.system.totalMemoryBytes)
    : null;

  const metrics: ResourceMetric[] = [
    {
      label: "any2api 内存",
      value: resources ? formatResourceBytes(resources.process.residentMemoryBytes) : "—",
      note:
        processMemoryShare === null
          ? "仅统计 any2api"
          : `占整机内存 ${formatResourcePercent(processMemoryShare)}`,
      progress: processMemoryShare,
      tone: "blue",
      icon: MemoryStick,
    },
    {
      label: "any2api CPU",
      value: resources ? formatResourcePercent(resources.process.cpuUsagePercent) : "—",
      note: "仅统计 any2api",
      progress: resources?.process.cpuUsagePercent ?? null,
      tone: "violet",
      icon: Cpu,
    },
    {
      label: "整机内存",
      value: systemMemory?.value ?? "—",
      note: systemMemory?.note ?? "等待采样",
      progress: systemMemoryPercent,
      tone: "green",
      icon: HardDrive,
    },
    {
      label: "整机 CPU",
      value: resources ? formatResourcePercent(resources.system.cpuUsagePercent) : "—",
      note: "包含所有运行中的程序",
      progress: resources?.system.cpuUsagePercent ?? null,
      tone: "orange",
      icon: Gauge,
    },
  ];

  return (
    <section className="min-w-0" aria-labelledby="overview-resources-title">
      <div className="flex items-start gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          <span
            className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[color-mix(in_srgb,var(--chart-1)_12%,transparent)] text-[var(--chart-1)]"
            aria-hidden="true"
          >
            <Gauge size={17} strokeWidth={2.2} />
          </span>
          <div className="min-w-0">
            <h2 id="overview-resources-title" className="text-sm font-semibold tracking-tight">
              资源状态
            </h2>
            <p className="mt-0.5 truncate text-xs text-tertiary">any2api 与整机资源占用</p>
          </div>
        </div>
      </div>

      <div className="mt-3 grid min-w-0 grid-cols-1 gap-2.5 min-[360px]:grid-cols-2">
        {metrics.map((metric) => (
          <OverviewMetricTile key={metric.label} {...metric} />
        ))}
      </div>
    </section>
  );
}

interface ResourceMetric {
  label: string;
  value: string;
  note: string;
  progress: number | null;
  tone: OverviewMetricTone;
  icon: LucideIcon;
}

function ratioPercent(used: number, total: number) {
  return total > 0 ? (used / total) * 100 : null;
}
