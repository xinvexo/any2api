import type { LucideIcon } from "lucide-react";
import { Cpu, Gauge, HardDrive, MemoryStick } from "lucide-react";

import type { OverviewResources } from "../api/overview-resources-contracts";
import {
  formatResourceBytes,
  formatResourcePercent,
  formatSystemMemory,
} from "../model/overview-resources-presentation";

type ResourceTone = "blue" | "violet" | "green" | "orange";

const toneColors: Record<ResourceTone, string> = {
  blue: "var(--chart-1)",
  violet: "var(--chart-2)",
  green: "var(--chart-6)",
  orange: "var(--chart-5)",
};

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
      note: processMemoryShare === null ? "进程 RSS" : `RSS · 占系统 ${formatResourcePercent(processMemoryShare)}`,
      progress: processMemoryShare,
      tone: "blue",
      icon: MemoryStick,
    },
    {
      label: "any2api CPU",
      value: resources ? formatResourcePercent(resources.process.cpuUsagePercent) : "—",
      note: "占整机逻辑 CPU",
      progress: resources?.process.cpuUsagePercent ?? null,
      tone: "violet",
      icon: Cpu,
    },
    {
      label: "系统内存",
      value: systemMemory?.value ?? "—",
      note: systemMemory?.note ?? "等待采样",
      progress: systemMemoryPercent,
      tone: "green",
      icon: HardDrive,
    },
    {
      label: "系统 CPU",
      value: resources ? formatResourcePercent(resources.system.cpuUsagePercent) : "—",
      note: "全部逻辑 CPU",
      progress: resources?.system.cpuUsagePercent ?? null,
      tone: "orange",
      icon: Gauge,
    },
  ];

  return (
    <section className="min-w-0" aria-labelledby="overview-resources-title">
      <div className="flex items-start justify-between gap-3">
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
            <p className="mt-0.5 truncate text-xs text-tertiary">进程与主机的当前采样</p>
          </div>
        </div>
        {resources ? (
          <time
            className="shrink-0 pt-1 text-[11px] tabular-nums text-tertiary"
            dateTime={new Date(resources.sampledAtMs).toISOString()}
          >
            {formatSampleTime(resources.sampledAtMs)}
          </time>
        ) : null}
      </div>

      <div className="mt-3 grid min-w-0 grid-cols-1 gap-3 min-[360px]:grid-cols-2">
        {metrics.map((metric) => (
          <ResourceTile key={metric.label} {...metric} />
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
  tone: ResourceTone;
  icon: LucideIcon;
}

function ResourceTile({ label, value, note, progress, tone, icon: Icon }: ResourceMetric) {
  const color = toneColors[tone];
  return (
    <div className="min-w-0 rounded-[8px] border border-subtle bg-surface/70 px-3.5 py-3.5 transition-colors hover:border-strong sm:px-4">
      <div className="flex min-w-0 items-center gap-2">
        <span
          className="grid size-6 shrink-0 place-items-center rounded-[6px]"
          style={{
            backgroundColor: `color-mix(in srgb, ${color} 12%, transparent)`,
            color,
          }}
          aria-hidden="true"
        >
          <Icon size={14} strokeWidth={2.1} />
        </span>
        <span className="truncate text-xs font-medium text-secondary">{label}</span>
      </div>
      <div className="mt-3 flex min-w-0 items-baseline gap-1.5">
        <strong className="min-w-0 truncate text-[1.65rem] font-semibold leading-none tracking-tight tabular-nums">
          {value}
        </strong>
      </div>
      <ProgressBar value={progress} color={color} label={`${label} 使用率`} />
      <p className="mt-2 truncate text-[11px] leading-4 text-tertiary" title={note}>
        {note}
      </p>
    </div>
  );
}

export function ProgressBar({
  value,
  color = "var(--accent)",
  label,
}: {
  value: number | null;
  color?: string;
  label: string;
}) {
  const bounded = value === null ? 0 : Math.min(100, Math.max(0, value));
  return (
    <div
      className="mt-3 h-1 overflow-hidden rounded-full bg-surface-muted"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value === null ? undefined : bounded}
    >
      <span
        className="block h-full rounded-full transition-[width] duration-500"
        style={{ backgroundColor: color, width: `${bounded}%` }}
      />
    </div>
  );
}

function ratioPercent(used: number, total: number) {
  return total > 0 ? (used / total) * 100 : null;
}

function formatSampleTime(value: number) {
  return new Date(value).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}
