import * as vscode from "vscode";

import { resolveMermanBinary, type BinaryInvocation } from "./binaries.js";
import { getCliSettings } from "./config.js";
import { runRenderProcess } from "./render-process.js";
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
  signal?: AbortSignal;
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

  return runRenderProcess({
    invocation,
    source: request.source,
    signal: request.signal,
  });
}
