import { ProviderManagement } from "@/features/providers";

export function ProvidersPage() {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">上游连接</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">Provider</h1>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-secondary">
          管理 Codex 与 Claude 的上游 Endpoint。URL、协议方言和网络授权独立保存，Credential 会在后续功能中单独聚合。
        </p>
      </header>
      <ProviderManagement />
    </div>
  );
}
