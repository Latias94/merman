import * as cp from "node:child_process";

import type { BinaryInvocation } from "./binaries.js";

export interface RenderProcessRequest {
  invocation: BinaryInvocation;
  source: string;
  signal?: AbortSignal;
  timeoutMs?: number;
  killGraceMs?: number;
  maxStdoutBytes?: number;
  maxStderrBytes?: number;
}

export interface RenderProcessResult {
  stdout: Buffer;
  stderr: string;
  invocation: BinaryInvocation;
}

const DEFAULT_MAX_STDOUT_BYTES = 32 * 1024 * 1024;
const DEFAULT_MAX_STDERR_BYTES = 1024 * 1024;

export function runRenderProcess(request: RenderProcessRequest): Promise<RenderProcessResult> {
  return new Promise<RenderProcessResult>((resolve, reject) => {
    let settled = false;
    let terminationReason: "abort" | "timeout" | "output-limit" | undefined;
    let timeoutTimer: NodeJS.Timeout | undefined;
    let killTimer: NodeJS.Timeout | undefined;
    let stdoutBytes = 0;
    let stderrBytes = 0;
    const maxStdoutBytes = request.maxStdoutBytes ?? DEFAULT_MAX_STDOUT_BYTES;
    const maxStderrBytes = request.maxStderrBytes ?? DEFAULT_MAX_STDERR_BYTES;
    const child = cp.spawn(request.invocation.command, request.invocation.args, {
      cwd: request.invocation.cwd,
      env: process.env,
      stdio: "pipe",
    });
    const clearTimers = (): void => {
      if (timeoutTimer) {
        clearTimeout(timeoutTimer);
        timeoutTimer = undefined;
      }
      if (killTimer) {
        clearTimeout(killTimer);
        killTimer = undefined;
      }
    };
    const rejectOnce = (error: Error): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimers();
      reject(error);
    };
    const resolveOnce = (result: RenderProcessResult): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimers();
      resolve(result);
    };
    const terminate = (reason: "abort" | "timeout" | "output-limit"): void => {
      if (terminationReason) {
        return;
      }
      terminationReason = reason;
      child.kill("SIGTERM");
      killTimer = setTimeout(() => {
        if (!settled && child.exitCode === null && child.signalCode === null) {
          child.kill("SIGKILL");
        }
      }, request.killGraceMs ?? 1000);
    };
    const abort = (): void => {
      terminate("abort");
    };
    if (request.signal?.aborted) {
      abort();
    } else {
      request.signal?.addEventListener("abort", abort, { once: true });
    }
    timeoutTimer = setTimeout(() => {
      terminate("timeout");
    }, request.timeoutMs ?? 30000);
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    child.stdout?.on("data", (chunk: Buffer | string) => {
      const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      stdoutBytes += buffer.byteLength;
      if (stdoutBytes > maxStdoutBytes) {
        terminate("output-limit");
        return;
      }
      stdoutChunks.push(buffer);
    });
    child.stderr?.on("data", (chunk: Buffer | string) => {
      const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      stderrBytes += buffer.byteLength;
      if (stderrBytes > maxStderrBytes) {
        terminate("output-limit");
        return;
      }
      stderrChunks.push(buffer);
    });
    child.on("error", (error) => {
      request.signal?.removeEventListener("abort", abort);
      rejectOnce(error);
    });
    child.on("close", (code, signal) => {
      request.signal?.removeEventListener("abort", abort);
      clearTimers();
      const stdout = Buffer.concat(stdoutChunks);
      const stderr = Buffer.concat(stderrChunks).toString("utf8");
      if (terminationReason === "timeout") {
        return rejectOnce(new Error("Merman render timed out."));
      }
      if (terminationReason === "output-limit") {
        return rejectOnce(new Error("Merman render output exceeded the size limit."));
      }
      if (terminationReason === "abort" || request.signal?.aborted || signal === "SIGTERM") {
        return rejectOnce(new Error("Render was superseded by a newer update."));
      }
      if (code !== 0) {
        return rejectOnce(
          new Error(stderr.trim() || `merman-cli exited with status ${code ?? "unknown"}`),
        );
      }
      resolveOnce({
        stdout,
        stderr,
        invocation: request.invocation,
      });
    });
    child.stdin?.end(request.source, "utf8");
  });
}
