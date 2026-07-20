import { RequestLogManagement } from "@/features/request-logs";

export function RequestLogsPage() {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">本地观测</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">请求日志</h1>
        <p className="mt-3 max-w-3xl text-sm leading-6 text-secondary">
          只保存请求元数据与 Attempt 结果，不保存 Prompt、完整请求体、响应体或任何 Secret。
        </p>
      </header>
      <RequestLogManagement />
    </div>
  );
}
