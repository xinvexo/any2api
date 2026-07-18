import type { ProviderEditorDraft } from "../model/use-provider-editor";

interface ProviderSecurityOptionsProps {
  draft: ProviderEditorDraft;
  pending: boolean;
  onChange: (field: "allowInsecureHttp" | "allowPrivateNetwork", value: boolean) => void;
}

export function ProviderSecurityOptions({
  draft,
  pending,
  onChange,
}: ProviderSecurityOptionsProps) {
  const isHttp = draft.baseUrl.toLowerCase().startsWith("http://");

  return (
    <fieldset className="space-y-3 rounded-control border border-subtle bg-surface-muted p-4">
      <legend className="px-1 text-sm font-semibold">网络安全授权</legend>
      <Toggle
        id="provider-allow-http"
        checked={draft.allowInsecureHttp}
        disabled={pending}
        onChange={(value) => onChange("allowInsecureHttp", value)}
        title="允许普通 HTTP"
        description="仅影响这个 Endpoint；开启后上游请求可能明文传输。"
      />
      <Toggle
        id="provider-allow-private"
        checked={draft.allowPrivateNetwork}
        disabled={pending}
        onChange={(value) => onChange("allowPrivateNetwork", value)}
        title="允许内网地址"
        description="允许 loopback、私网、link-local 或本地命名空间；DNS 最终校验仍由网络层执行。"
      />
      {isHttp && !draft.allowInsecureHttp ? (
        <p className="text-xs leading-5 text-warning" role="status">
          当前地址使用 HTTP，保存前需要显式开启“允许普通 HTTP”。
        </p>
      ) : null}
    </fieldset>
  );
}

function Toggle({
  id,
  checked,
  disabled,
  onChange,
  title,
  description,
}: {
  id: string;
  checked: boolean;
  disabled: boolean;
  onChange: (value: boolean) => void;
  title: string;
  description: string;
}) {
  return (
    <div className="flex items-start gap-3">
      <input
        id={id}
        type="checkbox"
        className="mt-0.5 size-4 accent-accent"
        checked={checked}
        disabled={disabled}
        aria-describedby={`${id}-description`}
        onChange={(event) => onChange(event.target.checked)}
      />
      <div>
        <label htmlFor={id} className="block text-sm font-medium">
          {title}
        </label>
        <p id={`${id}-description`} className="mt-1 text-xs leading-5 text-secondary">{description}</p>
      </div>
    </div>
  );
}
