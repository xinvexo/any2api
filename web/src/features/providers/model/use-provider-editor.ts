import { useState } from "react";

import type {
  ProtocolDialect,
  ProviderEndpoint,
  ProviderEndpointWriteInput,
  ProviderKind,
  ProviderProtocolOptions,
} from "../api/provider-contracts";

export interface ProviderEditorDraft {
  name: string;
  providerKind: ProviderKind;
  baseUrl: string;
  protocolDialect: ProtocolDialect;
  upstreamProtocolDialect: ProtocolDialect | null;
  enabled: boolean;
}

type EditorField = keyof ProviderEditorDraft;
export type ProviderEditorErrors = Partial<Record<EditorField, string>>;

export function useProviderEditor(
  endpoint?: ProviderEndpoint,
  defaultKind: ProviderKind = "codex",
  protocolOptions: ProviderProtocolOptions[] = [],
) {
  const [draft, setDraft] = useState<ProviderEditorDraft>(() =>
    initialDraft(endpoint, defaultKind, protocolOptions),
  );
  const [expectedConfigVersion] = useState(endpoint?.configVersion);
  const [errors, setErrors] = useState<ProviderEditorErrors>({});

  function update<Field extends EditorField>(
    field: Field,
    value: ProviderEditorDraft[Field],
  ) {
    setDraft((current) => ({ ...current, [field]: value }));
    setErrors((current) => ({ ...current, [field]: undefined }));
  }

  function updateProviderKind(providerKind: ProviderKind) {
    const protocolDialect = defaultProtocol(providerKind, protocolOptions);
    setDraft((current) => ({
      ...current,
      providerKind,
      protocolDialect,
      upstreamProtocolDialect: null,
      baseUrl:
        current.baseUrl.length === 0 || current.baseUrl === defaultBaseUrl(current.providerKind)
          ? defaultBaseUrl(providerKind)
          : current.baseUrl,
    }));
    setErrors((current) => ({ ...current, providerKind: undefined }));
  }

  function updateProtocolDialect(protocolDialect: ProtocolDialect) {
    setDraft((current) => ({
      ...current,
      protocolDialect,
      upstreamProtocolDialect: null,
    }));
    setErrors((current) => ({
      ...current,
      protocolDialect: undefined,
      upstreamProtocolDialect: undefined,
    }));
  }

  function buildInput(expectedRevision: number): ProviderEndpointWriteInput | null {
    const nextErrors = validate(draft, protocolOptions);
    setErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) {
      return null;
    }
    return {
      expectedRevision,
      expectedConfigVersion,
      name: draft.name,
      providerKind: draft.providerKind,
      baseUrl: draft.baseUrl,
      protocolDialect: draft.protocolDialect,
      upstreamProtocolDialect: draft.upstreamProtocolDialect,
      enabled: draft.enabled,
    };
  }

  return {
    draft,
    errors,
    update,
    updateProviderKind,
    updateProtocolDialect,
    buildInput,
  };
}

function initialDraft(
  endpoint: ProviderEndpoint | undefined,
  defaultKind: ProviderKind,
  protocolOptions: ProviderProtocolOptions[],
): ProviderEditorDraft {
  const kind = endpoint?.providerKind ?? defaultKind;
  return {
    name: endpoint?.name ?? "",
    providerKind: kind,
    baseUrl: endpoint?.baseUrl ?? defaultBaseUrl(kind),
    protocolDialect:
      endpoint?.protocolDialect ?? defaultProtocol(kind, protocolOptions),
    upstreamProtocolDialect: endpoint?.upstreamProtocolDialect ?? null,
    enabled: endpoint?.enabled ?? true,
  };
}

function defaultBaseUrl(kind: ProviderKind) {
  return kind === "codex" ? "https://api.openai.com/v1" : "https://api.anthropic.com/v1";
}

function validate(
  draft: ProviderEditorDraft,
  protocolOptions: ProviderProtocolOptions[],
): ProviderEditorErrors {
  const errors: ProviderEditorErrors = {};
  if (draft.name.trim().length === 0) {
    errors.name = "请输入 Endpoint 名称";
  } else if (draft.name.trim() !== draft.name) {
    errors.name = "名称首尾不能包含空格";
  } else if ([...draft.name].length > 100) {
    errors.name = "名称不能超过 100 个字符";
  }

  const urlError = validateUrl(draft.baseUrl);
  if (urlError) {
    errors.baseUrl = urlError;
  }
  const option = protocolOptions.find(
    (candidate) =>
      candidate.providerKind === draft.providerKind &&
      candidate.acceptedProtocol === draft.protocolDialect,
  );
  if (!option) {
    errors.protocolDialect = "请选择当前 Provider 支持的接受协议";
  } else {
    const upstream = draft.upstreamProtocolDialect ?? draft.protocolDialect;
    if (!option.upstreamProtocols.includes(upstream)) {
      errors.upstreamProtocolDialect = "请选择已注册且上游支持的转换协议";
    }
  }
  return errors;
}

function defaultProtocol(
  kind: ProviderKind,
  options: ProviderProtocolOptions[],
): ProtocolDialect {
  return (
    options.find((option) => option.providerKind === kind)?.acceptedProtocol ??
    (kind === "codex" ? "openai_responses" : "anthropic_messages")
  );
}

function validateUrl(value: string): string | undefined {
  if (value.trim() !== value || value.length === 0) {
    return "请输入不含首尾空格的 Base URL";
  }
  if ([...value].length > 2_048) {
    return "Base URL 不能超过 2048 个字符";
  }
  try {
    const url = new URL(value);
    if (url.protocol !== "https:" && url.protocol !== "http:") {
      return "Base URL 只支持 HTTP 或 HTTPS";
    }
    if (url.username || url.password || url.search || url.hash) {
      return "Base URL 不能包含账号、查询参数或片段";
    }
  } catch {
    return "请输入有效的绝对 URL";
  }
  return undefined;
}
