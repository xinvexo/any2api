import { LoaderCircle, RefreshCw, ShieldX } from "lucide-react";
import type { PropsWithChildren } from "react";

import { getAdminAuthErrorMessage } from "../model/admin-auth-error";
import { useAdminAuth } from "../model/use-admin-auth";
import { Button } from "@/shared/ui/Button";

import { AdminPasswordScreen, AuthCanvas } from "./AdminPasswordScreen";

export function AdminAuthGate({ children }: PropsWithChildren) {
  const auth = useAdminAuth();

  if (auth.loading) {
    return (
      <AuthCanvas>
        <div className="flex items-center gap-2.5 text-[13px] text-secondary" role="status">
          <LoaderCircle size={16} className="animate-spin" />
          正在检查管理员会话
        </div>
      </AuthCanvas>
    );
  }

  if (!auth.session) {
    return (
      <AuthCanvas>
        <section className="auth-card auth-panel text-center" role="alert">
          <div className="auth-panel-glow" aria-hidden="true" />
          <span className="mx-auto grid size-14 place-items-center rounded-2xl bg-warning/12 text-warning">
            <ShieldX size={24} strokeWidth={1.75} aria-hidden="true" />
          </span>
          <h1 className="auth-title mt-5">无法访问管理面</h1>
          <p className="auth-subtitle mx-auto mt-2">
            {getAdminAuthErrorMessage(auth.error)}
          </p>
          <Button className="mt-7 min-w-[6rem]" variant="secondary" onClick={() => void auth.refresh()}>
            <RefreshCw size={14} />
            重试
          </Button>
        </section>
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
