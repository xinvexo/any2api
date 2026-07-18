import { ProviderManagement } from "@/features/providers";

export function ProvidersPage() {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">上游连接</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">Provider</h1>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-secondary">
          管理 Codex 与 Claude 的上游 Endpoint，以及每个地址下独立的 API Key。
        </p>
      </header>
      <ProviderManagement />
    </div>
  );
}
