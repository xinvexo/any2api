import { useState } from "react";

import type {
  EditableProxyKind,
  ProxyProfile,
  ProxyWriteInput,
} from "../api/proxy-contracts";

interface EditorDraft {
  name: string;
  kind: EditableProxyKind;
  host: string;
  port: string;
  enabled: boolean;
}

type EditorField = keyof EditorDraft;
type EditorErrors = Partial<Record<EditorField, string>>;

export function useProxyEditor(profile?: ProxyProfile) {
  const [draft, setDraft] = useState<EditorDraft>(() => initialDraft(profile));
  const [errors, setErrors] = useState<EditorErrors>({});

  function update<Field extends EditorField>(field: Field, value: EditorDraft[Field]) {
    setDraft((current) => ({ ...current, [field]: value }));
    setErrors((current) => ({ ...current, [field]: undefined }));
  }

  function buildInput(expectedRevision: number): ProxyWriteInput | null {
    const nextErrors = validate(draft);
    setErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) {
      return null;
    }

    return {
      expectedRevision,
      name: draft.name,
      kind: draft.kind,
      host: draft.host,
      port: Number(draft.port),
      enabled: draft.enabled,
    };
  }

  return { draft, errors, update, buildInput };
}

function initialDraft(profile?: ProxyProfile): EditorDraft {
  return {
    name: profile?.name ?? "",
    kind: profile?.kind === "socks5" ? "socks5" : "http",
    host: profile?.host ?? "",
    port: profile?.port?.toString() ?? "",
    enabled: profile?.enabled ?? true,
  };
}

function validate(draft: EditorDraft): EditorErrors {
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
  return errors;
}
