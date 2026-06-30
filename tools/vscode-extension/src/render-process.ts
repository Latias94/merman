import * as cp from "node:child_process";

import type { BinaryInvocation } from "./binaries.js";

export interface RenderProcessRequest {
  invocation: BinaryInvocation;
  source: string;
  signal?: AbortSignal;
}

export interface RenderProcessResult {
  stdout: Buffer;
  stderr: string;
  invocation: BinaryInvocation;
}

export function runRenderProcess(request: RenderProcessRequest): Promise<RenderProcessResult> {
  return new Promise<RenderProcessResult>((resolve, reject) => {
    let settled = false;
    const child = cp.spawn(request.invocation.command, request.invocation.args, {
      cwd: request.invocation.cwd,
      env: process.env,
      stdio: "pipe",
    });
    const rejectOnce = (error: Error): void => {
      if (settled) {
        return;
      }
      settled = true;
      reject(error);
    };
    const resolveOnce = (result: RenderProcessResult): void => {
      if (settled) {
        return;
      }
      settled = true;
      resolve(result);
    };
    const abort = (): void => {
      child.kill("SIGTERM");
    };
    if (request.signal?.aborted) {
      abort();
    } else {
      request.signal?.addEventListener("abort", abort, { once: true });
    }
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    child.stdout?.on("data", (chunk: Buffer | string) => {
      stdoutChunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    });
    child.stderr?.on("data", (chunk: Buffer | string) => {
      stderrChunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    });
    child.on("error", (error) => {
      request.signal?.removeEventListener("abort", abort);
      rejectOnce(error);
    });
    child.on("close", (code, signal) => {
      request.signal?.removeEventListener("abort", abort);
      const stdout = Buffer.concat(stdoutChunks);
      const stderr = Buffer.concat(stderrChunks).toString("utf8");
      if (request.signal?.aborted || signal === "SIGTERM") {
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
