import { Braces, FileText } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";

import type { SystemLogBody as Body, SystemLogHeader } from "../api/system-log-contracts";
import { formatBytes } from "../model/system-log-presentation";
import { Button } from "@/shared/ui/Button";

const MAX_FORMATTED_JSON_CHARS = 4 * 1024 * 1024;
const MAX_HIGHLIGHTED_JSON_CHARS = 256 * 1024;
const MAX_HIGHLIGHTED_JSON_TOKENS = 4_096;
const MAX_JSON_DEPTH = 256;
const JSON_TOKEN_PATTERN =
  /("(?:\\.|[^"\\])*")(?=\s*:)|("(?:\\.|[^"\\])*")|(-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)|\b(true|false)\b|\b(null)\b/g;

export function SystemLogBody({
  body,
  headers,
}: {
  body: Body;
  headers: SystemLogHeader[];
}) {
  const formattedJson = useMemo(() => formatJsonBody(body, headers), [body, headers]);
  const [sourceContent, setSourceContent] = useState<string | null>(null);
  const showingSource = formattedJson !== null && sourceContent === body.content;
  const state = body.truncated ? "已截断" : body.complete ? "完整" : "未完整";
  const toggleLabel = showingSource ? "格式化 JSON 正文" : "查看 JSON 原文";

  return (
    <div className="mt-5">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-[11px] font-medium uppercase tracking-[0.08em] text-tertiary">Body</p>
        <div className="flex flex-wrap items-center justify-end gap-1.5">
          <p className="text-[11px] tabular-nums text-secondary">
            {state} · 捕获 {formatBytes(body.capturedBytes)} / 总计 {formatBytes(body.totalBytes)}
            {body.encoding === "base64" ? " · Base64" : ""}
          </p>
          {formattedJson !== null ? (
            <Button
              variant="ghost"
              size="sm"
              className="h-6 min-h-6 px-1.5 text-[11px]"
              aria-label={toggleLabel}
              aria-pressed={showingSource}
              title={toggleLabel}
              onClick={() => setSourceContent(showingSource ? null : body.content)}
            >
              {showingSource ? (
                <Braces size={12} aria-hidden="true" />
              ) : (
                <FileText size={12} aria-hidden="true" />
              )}
              {showingSource ? "格式化" : "原文"}
            </Button>
          ) : null}
        </div>
      </div>
      <pre
        className="mt-2 max-h-[32rem] overflow-auto whitespace-pre-wrap break-words rounded-[10px] bg-surface-muted/70 p-3 font-mono text-[12px] leading-5 text-secondary [overflow-wrap:anywhere] [scrollbar-gutter:stable]"
        data-body-view={formattedJson !== null && !showingSource ? "formatted-json" : "raw"}
      >
        {formattedJson !== null && !showingSource ? (
          <HighlightedJson source={formattedJson} />
        ) : body.content.length > 0 ? (
          body.content
        ) : (
          "（空）"
        )}
      </pre>
    </div>
  );
}

function HighlightedJson({ source }: { source: string }) {
  const nodes = useMemo(
    () => source.length <= MAX_HIGHLIGHTED_JSON_CHARS ? highlightJsonNodes(source) : null,
    [source],
  );
  return (
    <code data-json-highlight={nodes === null ? "plain" : "syntax"}>
      {nodes ?? source}
    </code>
  );
}

function highlightJsonNodes(source: string): ReactNode[] | null {
  const nodes: ReactNode[] = [];
  let cursor = 0;
  let tokenIndex = 0;
  for (const match of source.matchAll(JSON_TOKEN_PATTERN)) {
    if (tokenIndex >= MAX_HIGHLIGHTED_JSON_TOKENS) {
      return null;
    }
    const index = match.index;
    if (index > cursor) {
      nodes.push(source.slice(cursor, index));
    }
    const token = jsonToken(match);
    nodes.push(
      <span key={tokenIndex} className={token.className} data-json-token={token.kind}>
        {match[0]}
      </span>,
    );
    tokenIndex += 1;
    cursor = index + match[0].length;
  }
  if (cursor < source.length) {
    nodes.push(source.slice(cursor));
  }
  return nodes;
}

function jsonToken(match: RegExpMatchArray) {
  if (match[1] !== undefined) return { kind: "key", className: "text-accent-copy" };
  if (match[2] !== undefined) return { kind: "string", className: "text-success" };
  if (match[3] !== undefined) return { kind: "number", className: "text-warning" };
  if (match[4] !== undefined) return { kind: "boolean", className: "text-accent" };
  return { kind: "null", className: "text-tertiary" };
}

function formatJsonBody(body: Body, headers: SystemLogHeader[]): string | null {
  if (
    body.encoding !== "utf8"
    || !body.complete
    || body.truncated
    || !headers.some(isJsonContentType)
  ) {
    return null;
  }
  try {
    JSON.parse(body.content);
  } catch {
    return null;
  }
  return formatJsonSource(body.content);
}

function isJsonContentType(header: SystemLogHeader): boolean {
  if (header.encoding !== "utf8" || header.name.toLowerCase() !== "content-type") {
    return false;
  }
  const mediaType = header.value.split(";", 1)[0]?.trim().toLowerCase() ?? "";
  return mediaType === "application/json" || mediaType === "text/json" || mediaType.endsWith("+json");
}

function formatJsonSource(source: string): string | null {
  let output = "";
  let depth = 0;
  let lastSignificant = "";

  function append(value: string) {
    if (output.length + value.length > MAX_FORMATTED_JSON_CHARS) {
      return false;
    }
    output += value;
    return true;
  }

  function appendLine(level: number) {
    return append(`\n${"  ".repeat(level)}`);
  }

  for (let index = 0; index < source.length; index += 1) {
    const char = source[index]!;
    if (isJsonWhitespace(char)) {
      continue;
    }
    if (char === '"') {
      const end = findStringEnd(source, index);
      if (!append(source.slice(index, end + 1))) return null;
      index = end;
      lastSignificant = '"';
      continue;
    }
    if (char === "{" || char === "[") {
      if (!append(char)) return null;
      depth += 1;
      if (depth > MAX_JSON_DEPTH) return null;
      const closing = char === "{" ? "}" : "]";
      if (nextNonWhitespace(source, index + 1) !== closing && !appendLine(depth)) return null;
      lastSignificant = char;
      continue;
    }
    if (char === "}" || char === "]") {
      depth -= 1;
      const opening = char === "}" ? "{" : "[";
      if (lastSignificant !== opening && !appendLine(depth)) return null;
      if (!append(char)) return null;
      lastSignificant = char;
      continue;
    }
    if (char === ",") {
      if (!append(char) || !appendLine(depth)) return null;
      lastSignificant = char;
      continue;
    }
    if (char === ":") {
      if (!append(": ")) return null;
      lastSignificant = char;
      continue;
    }

    const end = findScalarEnd(source, index);
    const scalar = source.slice(index, end);
    if (!append(scalar)) return null;
    index = end - 1;
    lastSignificant = scalar.at(-1) ?? "";
  }

  return output;
}

function findStringEnd(source: string, start: number): number {
  let escaped = false;
  for (let index = start + 1; index < source.length; index += 1) {
    const char = source[index]!;
    if (escaped) {
      escaped = false;
    } else if (char === "\\") {
      escaped = true;
    } else if (char === '"') {
      return index;
    }
  }
  return source.length - 1;
}

function findScalarEnd(source: string, start: number): number {
  let index = start;
  while (index < source.length) {
    const char = source[index]!;
    if (isJsonWhitespace(char) || "{}[],:\"".includes(char)) break;
    index += 1;
  }
  return index;
}

function nextNonWhitespace(source: string, start: number): string | undefined {
  for (let index = start; index < source.length; index += 1) {
    if (!isJsonWhitespace(source[index]!)) return source[index];
  }
  return undefined;
}

function isJsonWhitespace(char: string): boolean {
  return char === " " || char === "\n" || char === "\r" || char === "\t";
}
