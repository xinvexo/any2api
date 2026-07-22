import { Save, Trash2, X } from "lucide-react";
import { useState, type FormEvent } from "react";

import type { ProxyProfile } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { Button } from "@/shared/ui/Button";

interface ProxyAuthenticationPanelProps {
  profile: ProxyProfile;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSet: (
    id: string,
    expectedRevision: number,
    input: { username: string; password: string },
  ) => Promise<void>;
  onClear: (id: string, expectedRevision: number) => Promise<void>;
}

export function ProxyAuthenticationPanel({
  profile,
  configRevision,
  pending,
  error,
  onSet,
  onClear,
}: ProxyAuthenticationPanelProps) {
  const [username, setUsername] = useState(profile.username ?? "");
  const [password, setPassword] = useState("");
  const [validationError, setValidationError] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextError = validate(username, password);
    setValidationError(nextError);
    if (nextError) {
      setPassword("");
      return;
    }
    try {
      await onSet(profile.id, configRevision, { username, password });
    } catch {
      // The parent renders the structured API error.
    } finally {
      setPassword("");
    }
  }

  async function clear() {
    try {
      await onClear(profile.id, configRevision);
      setConfirmClear(false);
    } catch {
      // The parent renders the structured API error.
    } finally {
      setPassword("");
    }
  }

  return (
    <section className="mt-8 space-y-4 border-t border-subtle pt-6" aria-labelledby="proxy-auth-heading">
      <div>
        <h3 id="proxy-auth-heading" className="text-[14px] font-semibold tracking-tight">
          代理认证
        </h3>
        <p className="mt-1 text-[13px] text-secondary">
          {profile.passwordConfigured ? `已为 ${profile.username} 配置` : "当前未配置认证"}
        </p>
      </div>

      <form className="space-y-4" onSubmit={(event) => void submit(event)}>
        <Field label="用户名" htmlFor="proxy-auth-username">
          <input
            id="proxy-auth-username"
            className={inputClass}
            value={username}
            autoComplete="username"
            onChange={(event) => {
              setUsername(event.target.value);
              setValidationError(null);
            }}
          />
        </Field>
        <Field label="密码" htmlFor="proxy-auth-password">
          <input
            id="proxy-auth-password"
            type="password"
            className={inputClass}
            value={password}
            autoComplete="new-password"
            onChange={(event) => {
              setPassword(event.target.value);
              setValidationError(null);
            }}
          />
        </Field>

        {validationError ? (
          <p className="text-sm text-danger" role="alert">
            {validationError}
          </p>
        ) : null}
        {error ? (
          <p className="text-sm text-danger" role="alert">
            {getProxyErrorMessage(error)}
          </p>
        ) : null}

        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-between">
          <div className="flex gap-2">
            {profile.passwordConfigured ? (
              confirmClear ? (
                <>
                  <Button variant="danger" disabled={pending} onClick={() => void clear()}>
                    <Trash2 size={15} />
                    确认清除
                  </Button>
                  <Button disabled={pending} onClick={() => setConfirmClear(false)}>
                    <X size={15} />
                    取消
                  </Button>
                </>
              ) : (
                <Button variant="ghost" disabled={pending} onClick={() => setConfirmClear(true)}>
                  <Trash2 size={15} />
                  清除认证
                </Button>
              )
            ) : null}
          </div>
          <Button type="submit" variant="primary" disabled={pending}>
            <Save size={15} />
            {pending ? "正在保存" : profile.passwordConfigured ? "替换认证" : "保存认证"}
          </Button>
        </div>
      </form>
    </section>
  );
}

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label htmlFor={htmlFor} className="text-sm font-medium">
        {label}
      </label>
      <div className="mt-2">{children}</div>
    </div>
  );
}

function validate(username: string, password: string) {
  if (username.length === 0) {
    return "请输入代理用户名";
  }
  if (new TextEncoder().encode(username).length > 255 || [...username].some(isControlCharacter)) {
    return "用户名必须在 255 字节内且不能包含控制字符";
  }
  if (username.includes(":")) {
    return "用户名不能包含冒号";
  }
  const passwordBytes = new TextEncoder().encode(password).length;
  if (passwordBytes < 1 || passwordBytes > 255) {
    return "密码必须在 1–255 字节之间";
  }
  return null;
}

function isControlCharacter(value: string) {
  const codePoint = value.codePointAt(0) ?? 0;
  return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
}

const inputClass =
  "focus-ring h-10 w-full rounded-[10px] border-0 bg-surface-muted px-3 text-sm text-primary disabled:opacity-60";
