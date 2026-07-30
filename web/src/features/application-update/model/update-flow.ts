import type { UpdateStatus } from "../api/update-contracts";

export type ApplicationUpdateFlow =
  | { kind: "idle" }
  | {
      kind: "running";
      targetVersion: string;
      accepted: boolean;
      status: UpdateStatus;
    }
  | { kind: "complete"; targetVersion: string }
  | { kind: "failed"; targetVersion: string; message: string };
