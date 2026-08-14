import { FileJson, LoaderCircle, Upload } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent, type FormEvent } from "react";

import { importOAuthFiles } from "../api/oauth-api";
import type { OAuthImportResult } from "../api/oauth-contracts";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field, FormError } from "@/shared/ui/form-field";
import { SideDrawer } from "@/shared/ui/SideDrawer";

const MAX_FILES = 32;
const MAX_FILE_BYTES = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES = 8 * 1024 * 1024;

interface OAuthImportDrawerProps {
  onClose: () => void;
  onImported: (result: OAuthImportResult) => void | Promise<void>;
  onReconcile: () => Promise<void>;
}

export function OAuthImportDrawer({
  onClose,
  onImported,
  onReconcile,
}: OAuthImportDrawerProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [files, setFiles] = useState<File[]>([]);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const totalBytes = files.reduce((total, file) => total + file.size, 0);

  useEffect(
    () => () => {
      if (inputRef.current) {
        inputRef.current.value = "";
      }
    },
    [],
  );

  function clearFiles() {
    setFiles([]);
    if (inputRef.current) {
      inputRef.current.value = "";
    }
  }

  function selectFiles(event: ChangeEvent<HTMLInputElement>) {
    const selected = Array.from(event.target.files ?? []);
    const selectionError = validateFiles(selected);
    if (selectionError) {
      clearFiles();
      setError(new Error(selectionError));
      return;
    }
    setError(null);
    setFiles(selected);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || files.length === 0) {
      return;
    }
    const selected = files;
    clearFiles();
    setError(null);
    setPending(true);
    try {
      const result = await importOAuthFiles(selected);
      await onImported(result);
      onClose();
    } catch (nextError) {
      try {
        await onReconcile();
      } catch {
        // Keep the original import failure visible if the reconciliation read also fails.
      }
      setError(nextError);
    } finally {
      setPending(false);
    }
  }

  function close() {
    if (pending) {
      return;
    }
    clearFiles();
    setError(null);
    onClose();
  }

  return (
    <SideDrawer
      open
      title="导入 OAuth JSON"
      description="兼容 CLIProxyAPI 与 Sub2API。导入后账号会写入 SQLite 并立即参与统一路由。"
      onClose={close}
    >
      <form className="space-y-5" aria-busy={pending} onSubmit={(event) => void submit(event)}>
        <Field
          label="OAuth JSON 文件"
          htmlFor="oauth-json-files"
          hint="最多 32 个文件；单文件 2 MiB，总计 8 MiB。一个文件可以包含多个账号。"
        >
          <input
            ref={inputRef}
            id="oauth-json-files"
            type="file"
            accept=".json,application/json"
            multiple
            disabled={pending}
            className={controlClass(false, "cursor-pointer py-1 file:mr-3 file:border-0 file:bg-transparent file:text-[12px] file:font-medium")}
            onChange={selectFiles}
          />
        </Field>

        {files.length > 0 ? (
          <div className="flex items-center gap-3 rounded-[9px] bg-surface-muted px-3 py-2.5 text-[13px]">
            <FileJson size={16} className="text-secondary" aria-hidden="true" />
            <p>
              已选择 <span className="font-medium tabular-nums">{files.length}</span> 个文件
              <span className="ml-2 text-secondary">{formatBytes(totalBytes)}</span>
            </p>
          </div>
        ) : null}

        <p className="text-[12px] leading-5 text-tertiary">
          上传文件不会作为副本保留；服务器只提取受支持的 OAuth 认证信息并保存为规范化账号。
        </p>

        <FormError>{error ? getImportErrorMessage(error) : null}</FormError>

        <div className="flex justify-end">
          <Button type="submit" variant="primary" disabled={pending || files.length === 0}>
            {pending ? (
              <LoaderCircle size={14} className="animate-spin" aria-hidden="true" />
            ) : (
              <Upload size={14} aria-hidden="true" />
            )}
            {pending ? "正在导入" : "导入并启用"}
          </Button>
        </div>
      </form>
    </SideDrawer>
  );
}

function validateFiles(files: File[]): string | null {
  if (files.length === 0) {
    return "请选择至少一个 JSON 文件。";
  }
  if (files.length > MAX_FILES) {
    return "一次最多选择 32 个 JSON 文件。";
  }
  if (files.some((file) => file.size > MAX_FILE_BYTES)) {
    return "单个 JSON 文件不能超过 2 MiB。";
  }
  if (files.reduce((total, file) => total + file.size, 0) > MAX_TOTAL_BYTES) {
    return "所选 JSON 文件总大小不能超过 8 MiB。";
  }
  return null;
}

function getImportErrorMessage(error: unknown) {
  if (error instanceof Error && error.message.startsWith("请选择")) {
    return error.message;
  }
  if (error instanceof Error && error.message.includes("JSON 文件")) {
    return error.message;
  }
  return getOAuthErrorMessage(error);
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KiB`;
}
