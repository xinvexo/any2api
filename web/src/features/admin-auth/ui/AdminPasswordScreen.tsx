import { Eye, EyeOff, LoaderCircle, LockKeyhole, Network } from "lucide-react";
import { useState, type FormEvent } from "react";

import { getAdminAuthErrorMessage } from "../model/admin-auth-error";
import { useAdminAuth } from "../model/use-admin-auth";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

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
      <Surface className="w-full max-w-md p-6 sm:p-8">
        <div className="flex items-center gap-3">
          <span className="grid size-10 place-items-center rounded-control bg-accent text-on-accent shadow-accent">
            <Network size={20} aria-hidden="true" />
          </span>
          <div>
            <p className="text-lg font-semibold">any2api</p>
            <p className="text-sm text-secondary">管理员控制台</p>
          </div>
        </div>

        <div className="mt-8">
          <LockKeyhole size={22} className="text-secondary" aria-hidden="true" />
          <h1 className="mt-3 text-2xl font-semibold">
            {setup ? "初始化管理员" : "管理员登录"}
          </h1>
          <p className="mt-2 text-sm leading-6 text-secondary">
            {setup
              ? "首次初始化只能从本机完成。请先从服务启动终端复制一次性 Setup Token。"
              : "输入单管理员密码继续访问配置与运行状态。"}
          </p>
        </div>

        {auth.session?.plaintextHttpWarning ? (
          <div className="mt-5 rounded-control border border-warning/35 bg-warning/10 px-4 py-3 text-sm leading-5 text-warning" role="status">
            当前连接使用明文 HTTP。管理员密码和会话 Cookie 可能被同网络中的攻击者截获。
          </div>
        ) : null}

        <form className="mt-7 space-y-5" onSubmit={(event) => void submit(event)}>
          {setup ? (
            <label className="block">
              <span className="text-sm font-medium">Setup Token</span>
              <input
                className="focus-ring mt-2 h-11 w-full rounded-control border border-subtle bg-surface px-3 font-mono text-sm"
                type="text"
                value={setupToken}
                autoComplete="off"
                spellCheck={false}
                onChange={(event) => setSetupToken(event.target.value.trim())}
              />
            </label>
          ) : null}
          <PasswordField
            label="管理员密码"
            value={password}
            visible={visible}
            autoComplete={setup ? "new-password" : "current-password"}
            onChange={setPassword}
            onToggle={() => setVisible((current) => !current)}
          />
          {setup ? (
            <PasswordField
              label="确认密码"
              value={confirmation}
              visible={visible}
              autoComplete="new-password"
              onChange={setConfirmation}
              onToggle={() => setVisible((current) => !current)}
            />
          ) : null}

          {mismatch ? (
            <p className="text-sm text-danger" role="alert">
              两次输入的密码不一致。
            </p>
          ) : null}
          {error ? (
            <p className="text-sm text-danger" role="alert">
              {getAdminAuthErrorMessage(error)}
            </p>
          ) : null}

          <Button
            className="w-full"
            type="submit"
            variant="primary"
            disabled={
              auth.submitting ||
              mismatch ||
              password.length === 0 ||
              (setup && setupToken.length === 0)
            }
          >
            {auth.submitting ? <LoaderCircle size={16} className="animate-spin" /> : null}
            {setup ? "创建管理员" : "登录"}
          </Button>
        </form>
      </Surface>
    </AuthCanvas>
  );
}

function PasswordField({
  label,
  value,
  visible,
  autoComplete,
  onChange,
  onToggle,
}: {
  label: string;
  value: string;
  visible: boolean;
  autoComplete: string;
  onChange: (value: string) => void;
  onToggle: () => void;
}) {
  return (
    <label className="block">
      <span className="text-sm font-medium">{label}</span>
      <span className="mt-2 flex rounded-control border border-subtle bg-surface focus-within:ring-2 focus-within:ring-accent">
        <input
          className="h-11 min-w-0 flex-1 rounded-l-control bg-transparent px-3 outline-none"
          type={visible ? "text" : "password"}
          value={value}
          autoComplete={autoComplete}
          onChange={(event) => onChange(event.target.value)}
        />
        <button
          type="button"
          className="grid size-11 place-items-center rounded-r-control text-secondary hover:bg-surface-hover hover:text-primary"
          aria-label={visible ? "隐藏密码" : "显示密码"}
          onClick={onToggle}
        >
          {visible ? <EyeOff size={17} /> : <Eye size={17} />}
        </button>
      </span>
    </label>
  );
}

export function AuthCanvas({ children }: { children: import("react").ReactNode }) {
  return (
    <main className="grid min-h-dvh place-items-center bg-canvas px-4 py-10 text-primary">
      {children}
    </main>
  );
}
