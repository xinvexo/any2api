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
        <section className="auth-card w-full max-w-[380px] px-8 py-9 text-center" role="alert">
          <span className="mx-auto grid size-12 place-items-center rounded-full bg-warning/12 text-warning">
            <ShieldX size={22} strokeWidth={1.75} aria-hidden="true" />
          </span>
          <h1 className="mt-4 text-[18px] font-semibold tracking-tight">无法访问管理面</h1>
          <p className="mt-2 text-[13px] leading-5 text-secondary">
            {getAdminAuthErrorMessage(auth.error)}
          </p>
          <Button className="mt-6 min-w-[5.5rem]" variant="secondary" onClick={() => void auth.refresh()}>
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
