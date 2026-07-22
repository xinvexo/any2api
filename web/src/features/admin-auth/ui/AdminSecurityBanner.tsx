import { TriangleAlert } from "lucide-react";

import { useAdminAuth } from "../model/use-admin-auth";

export function AdminSecurityBanner() {
  const { session } = useAdminAuth();
  if (!session?.plaintextHttpWarning) {
    return null;
  }
  return (
    <div
      className="rounded-panel border border-warning/30 bg-warning/10 px-4 py-3 text-warning shadow-hairline"
      role="status"
    >
      <div className="flex items-start gap-3 text-sm leading-5">
        <TriangleAlert size={17} className="mt-0.5 shrink-0" aria-hidden="true" />
        <p>
          当前远程管理使用明文 HTTP。管理员密码、会话 Cookie，以及未来上传的 OAuth2 JSON
          可能被同网络中的攻击者截获。
        </p>
      </div>
    </div>
  );
}
