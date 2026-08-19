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
      label: "ANY2API 内存",
      value: resources ? formatResourceBytes(resources.process.residentMemoryBytes) : "—",
      note:
        processMemoryShare === null
          ? "仅统计 ANY2API"
          : `占整机内存 ${formatResourcePercent(processMemoryShare)}`,
      progress: processMemoryShare,
      tone: "blue",
      icon: MemoryStick,
    },
    {
      label: "ANY2API CPU",
      value: resources ? formatResourcePercent(resources.process.cpuUsagePercent) : "—",
      note: "仅统计 ANY2API",
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
            <p className="mt-0.5 truncate text-xs text-tertiary">ANY2API 与整机资源占用</p>
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

export function MemoryOwnershipDetails({
  ownership,
}: {
  ownership: OverviewResources["ownership"] | undefined;
}) {
  const details = [
    {
      label: "正文堆内存",
      value: ownership ? formatResourceBytes(ownership.payloadBuffers.heapCurrentBytes) : "—",
      note: ownership
        ? `峰值 ${formatResourceBytes(ownership.payloadBuffers.heapPeakBytes)}`
        : "等待采样",
    },
    {
      label: "正文映射内存",
      value: ownership ? formatResourceBytes(ownership.payloadBuffers.mappedCurrentBytes) : "—",
      note: ownership
        ? `峰值 ${formatResourceBytes(ownership.payloadBuffers.mappedPeakBytes)}`
        : "等待采样",
    },
    {
      label: "HTTP 捕获",
      value: ownership
        ? formatResourceBytes(ownership.payloadBuffers.httpBodyCaptureCurrentBytes)
        : "—",
      note: ownership
        ? `峰值 ${formatResourceBytes(ownership.payloadBuffers.httpBodyCapturePeakBytes)}`
        : "等待采样",
    },
    {
      label: "遥测待写",
      value: ownership ? formatResourceBytes(ownership.telemetry.queuedOwnedBytes) : "—",
      note: ownership
        ? `总保留 ${formatResourceBytes(ownership.telemetry.reservedOwnedBytes)}`
        : "等待采样",
    },
    {
      label: "遥测写入中",
      value: ownership ? formatResourceBytes(ownership.telemetry.inFlightOwnedBytes) : "—",
      note: "当前 Writer 批次",
    },
    {
      label: "内存回收阻塞",
      value: ownership ? ownership.reclamation.blockers.toLocaleString("zh-CN") : "—",
      note: ownership
        ? `已完成 ${ownership.reclamation.completedRuns.toLocaleString("zh-CN")} 次 · 最近 ${formatMicros(ownership.reclamation.lastDurationMicros)}`
        : "等待采样",
    },
  ];

  return (
    <dl className="grid min-w-0 grid-cols-2 gap-x-4 gap-y-3 rounded-[14px] bg-surface-muted/35 px-3.5 py-3 shadow-hairline sm:grid-cols-3 lg:col-span-2 lg:grid-cols-6">
      {details.map((detail) => (
        <div key={detail.label} className="min-w-0">
          <dt className="truncate text-[11px] leading-4 text-tertiary">{detail.label}</dt>
          <dd className="mt-0.5 truncate text-sm font-semibold tabular-nums text-primary">
            {detail.value}
          </dd>
          <dd
            className="mt-0.5 truncate text-[10px] leading-4 text-tertiary"
            title={detail.note}
          >
            {detail.note}
          </dd>
        </div>
      ))}
    </dl>
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

function formatMicros(value: number) {
  if (value === 0) return "尚无";
  if (value < 1_000) return `${value.toLocaleString("zh-CN")} µs`;
  return `${(value / 1_000).toLocaleString("zh-CN", { maximumFractionDigits: 1 })} ms`;
}
