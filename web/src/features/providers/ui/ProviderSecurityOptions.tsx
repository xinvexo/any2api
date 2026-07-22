import type { ProviderEditorDraft } from "../model/use-provider-editor";
import { Switch } from "@/shared/ui/Switch";

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
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-4">
        <p id="provider-allow-http-label" className="text-[13px] font-medium">
          允许普通 HTTP
        </p>
        <Switch
          id="provider-allow-http"
          checked={draft.allowInsecureHttp}
          disabled={pending}
          aria-labelledby="provider-allow-http-label"
          onCheckedChange={(checked) => onChange("allowInsecureHttp", checked)}
        />
      </div>
      <div className="flex items-center justify-between gap-4">
        <p id="provider-allow-private-label" className="text-[13px] font-medium">
          允许内网地址
        </p>
        <Switch
          id="provider-allow-private"
          checked={draft.allowPrivateNetwork}
          disabled={pending}
          aria-labelledby="provider-allow-private-label"
          onCheckedChange={(checked) => onChange("allowPrivateNetwork", checked)}
        />
      </div>
      {isHttp && !draft.allowInsecureHttp ? (
        <p className="text-[12px] leading-4 text-warning" role="status">
          当前地址使用 HTTP，保存前需要开启“允许普通 HTTP”。
        </p>
      ) : null}
    </div>
  );
}
