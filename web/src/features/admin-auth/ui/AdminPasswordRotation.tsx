import { CheckCircle2, KeyRound, LoaderCircle } from "lucide-react";
import { useState, type FormEvent } from "react";

import { getAdminAuthErrorMessage } from "../model/admin-auth-error";
import { useAdminAuth } from "../model/use-admin-auth";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

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
    <Surface className="overflow-hidden">
      <div className="border-b border-subtle px-5 py-4">
        <div className="flex items-center gap-3">
          <span className="grid size-9 place-items-center rounded-control bg-surface-hover text-secondary">
            <KeyRound size={17} aria-hidden="true" />
          </span>
          <div>
            <h2 className="font-semibold">管理员密码</h2>
            <p className="mt-1 text-sm text-secondary">更新后，其他已登录浏览器需要重新登录。</p>
          </div>
        </div>
      </div>

      <form className="space-y-5 p-5" onSubmit={(event) => void submit(event)} aria-busy={auth.submitting}>
        <div className="grid gap-4 lg:grid-cols-3">
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
          <p className="text-sm text-danger" role="alert">
            两次输入的新密码不一致。
          </p>
        ) : null}
        {error ? (
          <p className="text-sm text-danger" role="alert">
            {getAdminAuthErrorMessage(error)}
          </p>
        ) : null}
        {completed ? (
          <p className="flex items-center gap-2 text-sm text-success" role="status">
            <CheckCircle2 size={16} aria-hidden="true" />
            密码已更新，当前会话已刷新。
          </p>
        ) : null}

        <div className="flex justify-end">
          <Button type="submit" variant="primary" disabled={auth.submitting || incomplete || mismatch}>
            {auth.submitting ? <LoaderCircle size={16} className="animate-spin" /> : <KeyRound size={16} />}
            更新密码
          </Button>
        </div>
      </form>
    </Surface>
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
    <label className="block">
      <span className="text-sm font-medium">{label}</span>
      <input
        className="focus-ring mt-2 h-11 w-full rounded-control border border-subtle bg-canvas px-3"
        type="password"
        value={value}
        autoComplete={autoComplete}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
