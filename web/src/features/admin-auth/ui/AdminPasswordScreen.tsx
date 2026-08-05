import { Eye, EyeOff, LoaderCircle, Shield } from "lucide-react";
import { useState, type FormEvent, type ReactNode } from "react";

import { getAdminAuthErrorMessage } from "../model/admin-auth-error";
import { useAdminAuth } from "../model/use-admin-auth";
import { AuthMouseParticles } from "./AuthMouseParticles";
import { cn } from "@/shared/lib/cn";
import { AppBrandIcon } from "@/shared/ui/AppBrandIcon";
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
      <section className="auth-card auth-panel" aria-labelledby="auth-brand-title">
        <div className="auth-panel-glow" aria-hidden="true" />

        <header className="auth-panel-header">
          <AppBrandIcon className="auth-mark" />
          <div className="auth-panel-brand">
            <h1 id="auth-brand-title" className="auth-title">
              any2api
            </h1>
            <p className="auth-subtitle">
              {setup ? "初始化单管理员控制台" : "AI API 聚合代理 · 管理控制台"}
            </p>
          </div>
        </header>

        {auth.session?.plaintextHttpWarning ? (
          <div className="auth-alert" role="status">
            <Shield size={15} strokeWidth={1.9} className="mt-0.5 shrink-0" aria-hidden="true" />
            <p>当前连接使用明文 HTTP，密码与会话 Cookie 可能被截获。</p>
          </div>
        ) : null}

        <form className="auth-form" onSubmit={(event) => void submit(event)}>
          {setup ? (
            <FieldShell label="Setup Token" htmlFor="auth-setup-token">
              <input
                id="auth-setup-token"
                className={authControlClass}
                type="text"
                value={setupToken}
                placeholder="启动终端中的一次性 Token"
                aria-label="Setup Token"
                autoComplete="off"
                spellCheck={false}
                onChange={(event) => setSetupToken(event.target.value.trim())}
              />
            </FieldShell>
          ) : null}

          <PasswordField
            id="auth-password"
            label="管理员密码"
            placeholder={setup ? "设置管理员密码" : "输入管理员密码"}
            value={password}
            visible={visible}
            autoComplete={setup ? "new-password" : "current-password"}
            onChange={setPassword}
            onToggle={() => setVisible((current) => !current)}
          />

          {setup ? (
            <PasswordField
              id="auth-password-confirm"
              label="确认密码"
              placeholder="再次输入密码"
              value={confirmation}
              visible={visible}
              autoComplete="new-password"
              onChange={setConfirmation}
              onToggle={() => setVisible((current) => !current)}
            />
          ) : null}

          {mismatch ? (
            <p className="auth-error" role="alert">
              两次输入的密码不一致。
            </p>
          ) : null}
          {error ? (
            <p className="auth-error" role="alert">
              {getAdminAuthErrorMessage(error)}
            </p>
          ) : null}

          <Button
            className="auth-submit"
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
            {auth.submitting ? <LoaderCircle size={16} className="animate-spin" /> : null}
            {setup ? "创建管理员" : "进入控制台"}
          </Button>
        </form>
      </section>
    </AuthCanvas>
  );
}

function FieldShell({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: ReactNode;
}) {
  return (
    <div className="auth-field">
      <label htmlFor={htmlFor} className="auth-field-label">
        {label}
      </label>
      {children}
    </div>
  );
}

function PasswordField({
  id,
  label,
  placeholder,
  value,
  visible,
  autoComplete,
  onChange,
  onToggle,
}: {
  id: string;
  label: string;
  placeholder: string;
  value: string;
  visible: boolean;
  autoComplete: string;
  onChange: (value: string) => void;
  onToggle: () => void;
}) {
  return (
    <FieldShell label={label} htmlFor={id}>
      <span className="relative block">
        <input
          id={id}
          className={cn(authControlClass, "pr-12")}
          type={visible ? "text" : "password"}
          value={value}
          placeholder={placeholder}
          aria-label={label}
          autoComplete={autoComplete}
          onChange={(event) => onChange(event.target.value)}
        />
        <button
          type="button"
          className="auth-eye"
          aria-label={visible ? "隐藏密码" : "显示密码"}
          onClick={onToggle}
        >
          {visible ? <EyeOff size={17} strokeWidth={1.75} /> : <Eye size={17} strokeWidth={1.75} />}
        </button>
      </span>
    </FieldShell>
  );
}

const authControlClass = "auth-control focus-ring";

export function AuthCanvas({ children }: { children: ReactNode }) {
  return (
    <main className="auth-canvas relative grid min-h-dvh place-items-center px-4 py-10 text-primary">
      <div className="auth-fx auth-fx-aurora" aria-hidden="true" />
      <div className="auth-fx auth-fx-grid" aria-hidden="true" />
      <AuthMouseParticles />
      <div className="auth-orb auth-orb-a" aria-hidden="true" />
      <div className="auth-orb auth-orb-b" aria-hidden="true" />
      <div className="auth-orb auth-orb-c" aria-hidden="true" />
      {children}
    </main>
  );
}
