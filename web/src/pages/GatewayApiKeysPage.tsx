import { GatewayApiKeyManagement } from "@/features/gateway-api-keys";

export function GatewayApiKeysPage() {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">客户端访问</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">网关密钥</h1>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-secondary">
          管理不同设备和客户端访问当前实例所用的密钥。
        </p>
      </header>
      <GatewayApiKeyManagement />
    </div>
  );
}
