import { useParams } from "react-router-dom";

import { RequestLogDetail } from "@/features/request-logs";

export function RequestLogDetailPage() {
  const { requestId = "" } = useParams();
  return <RequestLogDetail requestId={requestId} />;
}
