import { useState } from "react";

import type {
  EditableProxyKind,
  ProxyProfile,
  ProxyWriteInput,
} from "../api/proxy-contracts";

export type ProxyAuthSubmit =
  | { kind: "disabled" }
  | { kind: "unchanged" }
  | { kind: "set"; username: string; password: string };

interface EditorDraft {
  name: string;
  kind: EditableProxyKind;
  host: string;
  port: string;
  enabled: boolean;
  authEnabled: boolean;
  username: string;
  password: string;
}

type EditorField = keyof EditorDraft;
type EditorErrors = Partial<Record<EditorField, string>>;

export interface ProxyEditorSubmit {
  input: ProxyWriteInput;
  auth: ProxyAuthSubmit;
}

export function useProxyEditor(profile?: ProxyProfile) {
  const [draft, setDraft] = useState<EditorDraft>(() => initialDraft(profile));
  const [errors, setErrors] = useState<EditorErrors>({});

  function update<Field extends EditorField>(field: Field, value: EditorDraft[Field]) {
    setDraft((current) => ({ ...current, [field]: value }));
    setErrors((current) => ({ ...current, [field]: undefined }));
  }

  function buildSubmit(expectedRevision: number): ProxyEditorSubmit | null {
    const nextErrors = validate(draft, profile);
    setErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) {
      return null;
    }

    return {
      input: {
        expectedRevision,
        name: draft.name,
        kind: draft.kind,
        host: draft.host,
        port: Number(draft.port),
        enabled: draft.enabled,
      },
      auth: buildAuthSubmit(draft, profile),
    };
  }

  return { draft, errors, update, buildSubmit };
}

function initialDraft(profile?: ProxyProfile): EditorDraft {
  return {
    name: profile?.name ?? "",
    kind: profile?.kind === "socks5" ? "socks5" : "http",
    host: profile?.host ?? "",
    port: profile?.port?.toString() ?? "",
    enabled: profile?.enabled ?? true,
    authEnabled: profile?.passwordConfigured ?? false,
    username: profile?.username ?? "",
    password: "",
  };
}

function validate(draft: EditorDraft, profile?: ProxyProfile): EditorErrors {
  const errors: EditorErrors = {};
  if (draft.name.trim().length === 0) {
    errors.name = "请输入代理名称";
  } else if (draft.name.trim() !== draft.name) {
    errors.name = "名称首尾不能包含空格";
  } else if ([...draft.name].length > 100) {
    errors.name = "名称不能超过 100 个字符";
  }
  if (draft.host.trim().length === 0) {
    errors.host = "请输入主机名或 IP 地址";
  } else if (draft.host.trim() !== draft.host) {
    errors.host = "主机首尾不能包含空格";
  }
  const port = Number(draft.port);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    errors.port = "端口必须在 1–65535 之间";
  }

  if (draft.authEnabled) {
    if (draft.username.length === 0) {
      errors.username = "请输入代理用户名";
    } else if (
      new TextEncoder().encode(draft.username).length > 255 ||
      [...draft.username].some(isControlCharacter)
    ) {
      errors.username = "用户名包含非法字符";
    } else if (draft.username.includes(":")) {
      errors.username = "用户名不能包含冒号";
    }

    const passwordBytes = new TextEncoder().encode(draft.password).length;
    const canKeepExisting =
      Boolean(profile?.passwordConfigured) &&
      draft.password.length === 0 &&
      draft.username === (profile?.username ?? "");

    if (!canKeepExisting) {
      if (passwordBytes < 1 || passwordBytes > 255) {
        errors.password = profile?.passwordConfigured
          ? "修改认证时请重新输入密码"
          : "请输入 1–255 字节的密码";
      }
    }
  }

  return errors;
}

function buildAuthSubmit(draft: EditorDraft, profile?: ProxyProfile): ProxyAuthSubmit {
  if (!draft.authEnabled) {
    return { kind: "disabled" };
  }

  const canKeepExisting =
    Boolean(profile?.passwordConfigured) &&
    draft.password.length === 0 &&
    draft.username === (profile?.username ?? "");

  if (canKeepExisting) {
    return { kind: "unchanged" };
  }

  return {
    kind: "set",
    username: draft.username,
    password: draft.password,
  };
}

function isControlCharacter(char: string) {
  const code = char.codePointAt(0) ?? 0;
  return code <= 0x1f || code === 0x7f;
}
