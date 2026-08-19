import { requestJson } from "@/shared/api/http-client";

export class InvalidApplicationRestartResponseError extends Error {
  constructor() {
    super("invalid application restart response");
    this.name = "InvalidApplicationRestartResponseError";
  }
}

export function startApplicationRestart() {
  return requestJson<unknown>("/api/admin/restart", {
    method: "POST",
  }).then(parseRestartAccepted);
}

function parseRestartAccepted(value: unknown) {
  if (
    typeof value !== "object"
    || value === null
    || !("status" in value)
    || value.status !== "restarting"
  ) {
    throw new InvalidApplicationRestartResponseError();
  }
}
