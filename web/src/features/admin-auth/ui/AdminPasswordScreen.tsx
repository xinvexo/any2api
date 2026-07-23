import { Eye, EyeOff, LoaderCircle, Network } from "lucide-react";
import { useState, type FormEvent, type ReactNode } from "react";

import { getAdminAuthErrorMessage } from "../model/admin-auth-error";
import { useAdminAuth } from "../model/use-admin-auth";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";

export function AdminPasswordScreen({ mode }: { mode: "setup" | "login" }) {
  const auth = useAdminAuth();
  const [setupToken, setSetupToken] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [visible, setVisible] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const setup = mode === "setup";
  const mismatch = setup && confirmation.length > 0 && password !== confirmation;

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (mismatch || password.length === 0 || (setup && setupToken.length === 0)) {
      return;
    }
    setError(null);
    try {
      if (setup) {
        await auth.setup(password, setupToken);
      } else {
        await auth.login(password);
      }
    } catch (nextError) {
      setError(nextError);
    }
  }

  return (
    <AuthCanvas>
      <section
        className="auth-card w-full max-w-[360px] px-8 pb-8 pt-9 sm:px-9 sm:pb-9 sm:pt-10"
        aria-labelledby="auth-brand-title"
      >
        <header className="flex flex-col items-center text-center">
          <span
            className="grid size-14 place-items-center rounded-[16px] bg-accent text-white shadow-[0_8px_24px_rgb(0_113_227_/_22%)]"
            aria-hidden="true"
          >
            <Network size={26} strokeWidth={1.75} />
          </span>
          <h1 id="auth-brand-title" className="mt-5 text-[17px] font-semibold tracking-tight text-primary">
            any2api
          </h1>
        </header>

        {auth.session?.plaintextHttpWarning ? (
          <div
            className="mt-6 rounded-[10px] bg-warning/10 px-3.5 py-3 text-left text-[12px] leading-5 text-warning"
            role="status"
          >
            当前连接使用明文 HTTP，密码与会话 Cookie 可能被截获。
          </div>
        ) : null}

        <form className="mt-7 space-y-3" onSubmit={(event) => void submit(event)}>
          {setup ? (
            <input
              className={authControlClass}
              type="text"
              value={setupToken}
              placeholder="Setup Token"
              aria-label="Setup Token"
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setSetupToken(event.target.value.trim())}
            />
          ) : null}

          <PasswordField
            label="管理员密码"
            placeholder={setup ? "密码" : "密码"}
            value={password}
            visible={visible}
            autoComplete={setup ? "new-password" : "current-password"}
            onChange={setPassword}
            onToggle={() => setVisible((current) => !current)}
          />

          {setup ? (
            <PasswordField
              label="确认密码"
              placeholder="确认密码"
              value={confirmation}
              visible={visible}
              autoComplete="new-password"
              onChange={setConfirmation}
              onToggle={() => setVisible((current) => !current)}
            />
          ) : null}

          {mismatch ? (
            <p className="text-[12px] text-danger" role="alert">
              两次输入的密码不一致。
            </p>
          ) : null}
          {error ? (
            <p className="text-[12px] text-danger" role="alert">
              {getAdminAuthErrorMessage(error)}
            </p>
          ) : null}

          <Button
            className="mt-1 h-9 w-full rounded-[10px] text-[14px]"
            type="submit"
            variant="primary"
            size="lg"
            disabled={
              auth.submitting ||
              mismatch ||
              password.length === 0 ||
              (setup && setupToken.length === 0)
            }
          >
            {auth.submitting ? <LoaderCircle size={15} className="animate-spin" /> : null}
            {setup ? "创建管理员" : "登录"}
          </Button>
        </form>
      </section>
    </AuthCanvas>
  );
}

function PasswordField({
  label,
  placeholder,
  value,
  visible,
  autoComplete,
  onChange,
  onToggle,
}: {
  label: string;
  placeholder: string;
  value: string;
  visible: boolean;
  autoComplete: string;
  onChange: (value: string) => void;
  onToggle: () => void;
}) {
  return (
    <span className="relative block">
      <input
        className={cn(authControlClass, "pr-11")}
        type={visible ? "text" : "password"}
        value={value}
        placeholder={placeholder}
        aria-label={label}
        autoComplete={autoComplete}
        onChange={(event) => onChange(event.target.value)}
      />
      <button
        type="button"
        className="absolute inset-y-0 right-0 grid w-11 place-items-center text-tertiary transition-colors hover:text-primary"
        aria-label={visible ? "隐藏密码" : "显示密码"}
        onClick={onToggle}
      >
        {visible ? <EyeOff size={16} strokeWidth={1.75} /> : <Eye size={16} strokeWidth={1.75} />}
      </button>
    </span>
  );
}

const authControlClass =
  "focus-ring h-10 w-full rounded-[10px] border-0 bg-surface-muted px-3 text-[14px] text-primary outline-none placeholder:text-tertiary";

export function AuthCanvas({ children }: { children: ReactNode }) {
  return (
    <main className="auth-canvas relative grid min-h-dvh place-items-center px-4 py-10 text-primary">
      {children}
    </main>
  );
}
