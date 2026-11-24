import { statSync } from "node:fs";
import { isAbsolute } from "node:path";

import type { BinaryInvocation } from "./binaries.js";
import { runRenderProcess } from "./render-process.js";

const metadataByInvocation = new Map<string, {
  identity: string | undefined;
  pending: Promise<unknown>;
}>();
const MAX_PREVIEW_FRAME_BYTES = 4 * 1024 * 1024;

export function readCliMetadata(invocation: BinaryInvocation): Promise<unknown> {
  const key = cliMetadataKey(invocation);
  const identity = cliExecutableIdentity(invocation);
  const cached = metadataByInvocation.get(key);
  if (cached && cached.identity === identity) {
    return cached.pending;
  }

  const pending = runRenderProcess({
    invocation,
    source: "",
    timeoutMs: 15000,
    maxStdoutBytes: 8 * 1024 * 1024,
  }).then((result): unknown => JSON.parse(result.stdout.toString("utf8")));
  metadataByInvocation.set(key, { identity, pending });
  void pending.then(
    () => {
      if (identity === undefined) {
        discardCliMetadata(invocation, pending);
      }
    },
    () => discardCliMetadata(invocation, pending),
  );
  return pending;
}

function cliMetadataKey(invocation: BinaryInvocation): string {
  return JSON.stringify([invocation.command, invocation.cwd, invocation.args]);
}

export function discardCliMetadata(invocation: BinaryInvocation, pending: Promise<unknown>): void {
  const key = cliMetadataKey(invocation);
  if (metadataByInvocation.get(key)?.pending === pending) {
    metadataByInvocation.delete(key);
  }
}

function cliExecutableIdentity(invocation: BinaryInvocation): string | undefined {
  if (invocation.source === "cargo" || !isAbsolute(invocation.command)) {
    return undefined;
  }
  try {
    const stat = statSync(invocation.command, { bigint: true });
    return stat.isFile()
      ? `${stat.dev}:${stat.ino}:${stat.size}:${stat.mtimeNs}:${stat.ctimeNs}`
      : undefined;
  } catch {
    return undefined;
  }
}

export function previewLayoutFrameLimit(capabilities: unknown): number | undefined {
  const frame = (capabilities as {
    preview_layout_frame?: { protocol_version?: unknown; max_bytes?: unknown };
  } | null)?.preview_layout_frame;
  if (frame?.protocol_version !== 1 ||
    typeof frame.max_bytes !== "number" || !Number.isSafeInteger(frame.max_bytes) ||
    frame.max_bytes <= 0) {
    return undefined;
  }
  return Math.min(frame.max_bytes, MAX_PREVIEW_FRAME_BYTES);
}
