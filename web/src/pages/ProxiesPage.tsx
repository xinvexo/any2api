import { ProxyManagement } from "@/features/proxies";

export function ProxiesPage() {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">网络出口</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">代理</h1>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-secondary">
          管理本机直连与 HTTP、SOCKS5 出口。绑定 DIRECT 的 Provider Credential 会继承这里的全局代理。
        </p>
      </header>
      <ProxyManagement />
    </div>
  );
}
