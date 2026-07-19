import { LoaderCircle, RefreshCw, ShieldX } from "lucide-react";
import type { PropsWithChildren } from "react";

import { getAdminAuthErrorMessage } from "../model/admin-auth-error";
import { useAdminAuth } from "../model/use-admin-auth";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

import { AdminPasswordScreen, AuthCanvas } from "./AdminPasswordScreen";

export function AdminAuthGate({ children }: PropsWithChildren) {
  const auth = useAdminAuth();

  if (auth.loading) {
    return (
      <AuthCanvas>
        <div className="flex items-center gap-3 text-sm text-secondary" role="status">
          <LoaderCircle size={18} className="animate-spin" />
          正在检查管理员会话
        </div>
      </AuthCanvas>
    );
  }

  if (!auth.session) {
    return (
      <AuthCanvas>
        <Surface className="w-full max-w-md p-7 text-center" role="alert">
          <ShieldX size={24} className="mx-auto text-warning" aria-hidden="true" />
          <h1 className="mt-4 text-xl font-semibold">无法访问管理面</h1>
          <p className="mt-2 text-sm leading-6 text-secondary">
            {getAdminAuthErrorMessage(auth.error)}
          </p>
          <Button className="mt-6" onClick={() => void auth.refresh()}>
            <RefreshCw size={15} />
            重试
          </Button>
        </Surface>
      </AuthCanvas>
    );
  }

  if (!auth.session.initialized) {
    return <AdminPasswordScreen mode="setup" />;
  }
  if (!auth.session.authenticated) {
    return <AdminPasswordScreen mode="login" />;
  }
  return children;
}
