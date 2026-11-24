import { statSync } from "node:fs";
import { isAbsolute } from "node:path";

import type { BinaryInvocation } from "./binaries.js";
import { runRenderProcess } from "./render-process.js";

const capabilitiesByInvocation = new Map<string, {
  identity: string | undefined;
  pending: Promise<unknown>;
}>();
const MAX_PREVIEW_FRAME_BYTES = 4 * 1024 * 1024;

export function readCliCapabilities(invocation: BinaryInvocation): Promise<unknown> {
  const key = cliCapabilitiesKey(invocation);
  const identity = cliExecutableIdentity(invocation);
  const cached = capabilitiesByInvocation.get(key);
  if (cached && cached.identity === identity) {
    return cached.pending;
  }

  const pending = runRenderProcess({
    invocation,
    source: "",
    timeoutMs: 15000,
    maxStdoutBytes: 8 * 1024 * 1024,
  }).then((result): unknown => JSON.parse(result.stdout.toString("utf8")));
  capabilitiesByInvocation.set(key, { identity, pending });
  void pending.then(
    () => {
      if (identity === undefined) {
        discardCliCapabilities(invocation, pending);
      }
    },
    () => discardCliCapabilities(invocation, pending),
  );
  return pending;
}

function cliCapabilitiesKey(invocation: BinaryInvocation): string {
  return JSON.stringify([invocation.command, invocation.cwd, invocation.args]);
}

export function discardCliCapabilities(invocation: BinaryInvocation, pending: Promise<unknown>): void {
  const key = cliCapabilitiesKey(invocation);
  if (capabilitiesByInvocation.get(key)?.pending === pending) {
    capabilitiesByInvocation.delete(key);
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

export function previewLayoutFrameCapability(capabilities: unknown): {
  maxBytes: number;
  privateOutput: boolean;
} | undefined {
  const frame = (capabilities as {
    preview_layout_frame?: { protocol_version?: unknown; max_bytes?: unknown; private_output?: unknown };
  } | null)?.preview_layout_frame;
  if (frame?.protocol_version !== 1 ||
    typeof frame.max_bytes !== "number" || !Number.isSafeInteger(frame.max_bytes) ||
    frame.max_bytes <= 0) {
    return undefined;
  }
  return {
    maxBytes: Math.min(frame.max_bytes, MAX_PREVIEW_FRAME_BYTES),
    privateOutput: frame.private_output === true,
  };
}

export function supportsLayoutFitWidth(capabilities: unknown): boolean {
  return (capabilities as { layout_fit_width?: unknown } | null)?.layout_fit_width === true;
}
