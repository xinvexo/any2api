import { CheckCircle2, KeyRound, LoaderCircle } from "lucide-react";
import { useState, type FormEvent } from "react";

import { getAdminAuthErrorMessage } from "../model/admin-auth-error";
import { useAdminAuth } from "../model/use-admin-auth";
import { Button } from "@/shared/ui/Button";

export function AdminPasswordRotation() {
  const auth = useAdminAuth();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState<unknown>(null);
  const [completed, setCompleted] = useState(false);
  const mismatch = confirmation.length > 0 && newPassword !== confirmation;
  const incomplete =
    currentPassword.length === 0 || newPassword.length === 0 || confirmation.length === 0;

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (incomplete || mismatch) {
      return;
    }
    setError(null);
    setCompleted(false);
    try {
      await auth.rotatePassword(currentPassword, newPassword);
      setCompleted(true);
    } catch (nextError) {
      setError(nextError);
    } finally {
      setCurrentPassword("");
      setNewPassword("");
      setConfirmation("");
    }
  }

  return (
    <section aria-labelledby="admin-password-heading">
      <header className="mb-4">
        <h2 id="admin-password-heading" className="text-[15px] font-semibold tracking-tight">
          管理员密码
        </h2>
        <p className="mt-1 text-[12px] leading-5 text-secondary">
          更新后，其他已登录浏览器需要重新登录。
        </p>
      </header>

      <form className="space-y-4" onSubmit={(event) => void submit(event)} aria-busy={auth.submitting}>
        <div className="grid gap-3 sm:grid-cols-3">
          <PasswordInput
            label="当前密码"
            value={currentPassword}
            autoComplete="current-password"
            onChange={setCurrentPassword}
          />
          <PasswordInput
            label="新密码"
            value={newPassword}
            autoComplete="new-password"
            onChange={setNewPassword}
          />
          <PasswordInput
            label="确认新密码"
            value={confirmation}
            autoComplete="new-password"
            onChange={setConfirmation}
          />
        </div>

        {mismatch ? (
          <p className="text-[12px] text-danger" role="alert">
            两次输入的新密码不一致。
          </p>
        ) : null}
        {error ? (
          <p className="text-[12px] text-danger" role="alert">
            {getAdminAuthErrorMessage(error)}
          </p>
        ) : null}
        {completed ? (
          <p className="flex items-center gap-2 text-[12px] text-success" role="status">
            <CheckCircle2 size={14} aria-hidden="true" />
            密码已更新，当前会话已刷新。
          </p>
        ) : null}

        <div className="flex justify-end">
          <Button type="submit" variant="primary" disabled={auth.submitting || incomplete || mismatch}>
            {auth.submitting ? <LoaderCircle size={14} className="animate-spin" /> : <KeyRound size={14} />}
            更新密码
          </Button>
        </div>
      </form>
    </section>
  );
}

function PasswordInput({
  label,
  value,
  autoComplete,
  onChange,
}: {
  label: string;
  value: string;
  autoComplete: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block min-w-0">
      <span className="text-[12px] font-medium text-primary">{label}</span>
      <input
        className="focus-ring mt-1.5 h-8 w-full rounded-[8px] border-0 bg-surface-muted px-2.5 text-[12px]"
        type="password"
        value={value}
        autoComplete={autoComplete}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
