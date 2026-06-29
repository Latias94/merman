import * as cp from "node:child_process";
import * as vscode from "vscode";

import { resolveMermanBinary, type BinaryInvocation } from "./binaries.js";
import { getCliSettings } from "./config.js";
import { renderMermanArgs, type RenderFormat } from "./render-options.js";
import { workspaceRoots } from "./workspace.js";

export type { RenderFormat } from "./render-options.js";

export interface RenderRequest {
  context: vscode.ExtensionContext;
  source: string;
  format: RenderFormat;
  outputPath?: string;
  theme?: string;
  outputChannel: vscode.LogOutputChannel;
  signalLabel?: string;
}

export interface RenderResult {
  stdout: Buffer;
  stderr: string;
  invocation: BinaryInvocation;
}

export function resolveCliInvocation(
  context: vscode.ExtensionContext,
  args: readonly string[],
): BinaryInvocation {
  const settings = getCliSettings();
  return resolveMermanBinary({
    binaryName: "merman-cli",
    packageName: "merman-cli",
    extensionPath: context.extensionUri.fsPath,
    workspaceRoots: workspaceRoots(),
    directArgs: args,
    explicitPath: settings.path,
    useCargoRun: settings.useCargoRun,
    cargoArgs: settings.cargoArgs,
  });
}

export async function renderMermanSource(request: RenderRequest): Promise<RenderResult> {
  const args = renderMermanArgs(request);
  const invocation = resolveCliInvocation(request.context, args);
  request.outputChannel.info(
    `${request.signalLabel ?? "render"}=${invocation.source} command="${invocation.command}" args="${invocation.args.join(" ")}"`,
  );

  return new Promise<RenderResult>((resolve, reject) => {
    const child = cp.spawn(invocation.command, invocation.args, {
      cwd: invocation.cwd,
      env: process.env,
      stdio: "pipe",
    });
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    child.stdout?.on("data", (chunk: Buffer | string) => {
      stdoutChunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    });
    child.stderr?.on("data", (chunk: Buffer | string) => {
      stderrChunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    });
    child.on("error", (error) => {
      reject(error);
    });
    child.on("close", (code, signal) => {
      const stdout = Buffer.concat(stdoutChunks);
      const stderr = Buffer.concat(stderrChunks).toString("utf8");
      if (signal === "SIGTERM") {
        return reject(new Error("Render was superseded by a newer update."));
      }
      if (code !== 0) {
        return reject(
          new Error(stderr.trim() || `merman-cli exited with status ${code ?? "unknown"}`),
        );
      }
      resolve({
        stdout,
        stderr,
        invocation,
      });
    });
    child.stdin?.end(request.source, "utf8");
  });
}
