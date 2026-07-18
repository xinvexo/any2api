import { ModelRouteManagement } from "@/features/model-routes";

export function ModelRoutesPage() {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">模型映射</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">模型路由</h1>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-secondary">
          将客户端模型名映射到一个或多个同协议上游目标。
        </p>
      </header>
      <ModelRouteManagement />
    </div>
  );
}
