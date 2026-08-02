import { KeyRound, LoaderCircle } from "lucide-react";
import { useState, type FormEvent } from "react";

import { getAdminAuthErrorMessage } from "../model/admin-auth-error";
import { useAdminAuth } from "../model/use-admin-auth";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";

export function AdminPasswordRotation({
  onCancel,
  onCompleted,
}: {
  onCancel?: () => void;
  onCompleted?: () => void;
} = {}) {
  const auth = useAdminAuth();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState<unknown>(null);
  const mismatch = confirmation.length > 0 && newPassword !== confirmation;
  const incomplete =
    currentPassword.length === 0 || newPassword.length === 0 || confirmation.length === 0;

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (incomplete || mismatch) {
      return;
    }
    setError(null);
    try {
      await auth.rotatePassword(currentPassword, newPassword);
      notify.success("密码已更新，当前会话已刷新。");
      onCompleted?.();
    } catch (nextError) {
      setError(nextError);
    } finally {
      setCurrentPassword("");
      setNewPassword("");
      setConfirmation("");
    }
  }

  return (
    <form className="flex flex-col gap-4" onSubmit={(event) => void submit(event)} aria-busy={auth.submitting}>
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
      <div className="mt-2 flex items-center justify-end gap-2 border-t border-subtle/70 pt-4">
        {onCancel ? (
          <Button
            type="button"
            variant="secondary"
            className="min-w-[4.5rem]"
            disabled={auth.submitting}
            onClick={onCancel}
          >
            取消
          </Button>
        ) : null}
        <Button type="submit" variant="primary" disabled={auth.submitting || incomplete || mismatch}>
          {auth.submitting ? <LoaderCircle size={14} className="animate-spin" /> : <KeyRound size={14} />}
          更新密码
        </Button>
      </div>
    </form>
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
        className="focus-ring mt-1.5 h-9 w-full rounded-[8px] border-0 bg-surface-muted px-3 text-[13px]"
        type="password"
        value={value}
        autoComplete={autoComplete}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
