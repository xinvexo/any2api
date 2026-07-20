import { useParams } from "react-router-dom";

import { RequestLogDetail } from "@/features/request-logs";

export function RequestLogDetailPage() {
  const { requestId = "" } = useParams();
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">本地观测</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">请求详情</h1>
      </header>
      <RequestLogDetail requestId={requestId} />
    </div>
  );
}
