import type {
  BridgeRequestFieldBehavior,
  ProviderUpstreamProtocolOption,
} from "../api/provider-contracts";

interface ProtocolFidelityNoticeProps {
  option: ProviderUpstreamProtocolOption | undefined;
}

export function ProtocolFidelityNotice({ option }: ProtocolFidelityNoticeProps) {
  if (!option) return null;

  if (option.fidelity === "direct") {
    return (
      <section
        className="rounded-[9px] border border-subtle bg-surface-muted px-3 py-2.5 text-[12px] text-secondary"
        aria-label="协议保真度"
      >
        <p>
          <FidelityBadge>Direct</FidelityBadge>
          不创建跨协议 Bridge；模型、stream、重复字段或 Provider 契约仍可能触发局部改写，
          不代表逐字节透明。
        </p>
        <p className="mt-1">支持操作：{option.operations.join("、")}</p>
      </section>
    );
  }

  const bridge = option.bridge;
  if (!bridge) return null;
  return (
    <section
      className="rounded-[9px] border border-warning/30 bg-warning/5 px-3 py-2.5 text-[12px] text-secondary"
      aria-label="协议保真度"
    >
      <p>
        <FidelityBadge>Translated</FidelityBadge>
        请求会按版本化 Bridge 契约重建；未登记字段会在上游 I/O 前拒绝。
      </p>
      <p className="mt-1 break-all font-mono text-[11px] text-tertiary">
        {bridge.contractId}
      </p>
      <details className="mt-2">
        <summary className="cursor-pointer select-none font-medium text-primary">
          查看转换能力与限制
        </summary>
        <dl className="mt-2 grid gap-2">
          <CapabilityRow label="操作" value={option.operations.join("、")} />
          <CapabilityRow
            label="工具类型"
            value={bridge.toolTypes.length > 0 ? bridge.toolTypes.join("、") : "无"}
          />
          <div>
            <dt className="font-medium text-primary">请求字段</dt>
            <dd className="mt-1 flex flex-wrap gap-1">
              {bridge.requestFields.map((field) => (
                <code
                  key={field.path}
                  className="rounded bg-surface px-1.5 py-0.5 text-[11px] text-secondary"
                >
                  {field.path} · {behaviorLabel(field.behavior)}
                </code>
              ))}
            </dd>
          </div>
          <div>
            <dt className="font-medium text-primary">已知限制</dt>
            <dd>
              <ul className="mt-1 list-disc space-y-1 pl-4">
                {bridge.limitations.map((limitation) => (
                  <li key={limitation.code}>{limitation.description}</li>
                ))}
              </ul>
            </dd>
          </div>
        </dl>
      </details>
    </section>
  );
}

function FidelityBadge({ children }: { children: string }) {
  return (
    <span className="mr-2 inline-flex rounded-full bg-surface px-2 py-0.5 font-semibold text-primary">
      {children}
    </span>
  );
}

function CapabilityRow({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="font-medium text-primary">{label}</dt>
      <dd className="mt-0.5">{value}</dd>
    </div>
  );
}

function behaviorLabel(behavior: BridgeRequestFieldBehavior) {
  const labels: Record<BridgeRequestFieldBehavior, string> = {
    forwarded: "原值转发",
    translated: "结构转换",
    validated_only: "仅校验",
    local_state: "本地状态",
  };
  return labels[behavior];
}
