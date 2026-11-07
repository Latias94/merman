import * as vscode from "vscode";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";

import { resolveMermanBinary, type BinaryInvocation } from "./binaries.js";
import { getCliSettings } from "./config.js";
import { runRenderProcess } from "./render-process.js";
import { renderMermanArgs, type DiagramRenderSettings, type RenderFormat } from "./render-options.js";
import { workspaceRoots } from "./workspace.js";

export type { RenderFormat } from "./render-options.js";

export interface RenderRequest extends DiagramRenderSettings {
  context: vscode.ExtensionContext;
  source: string;
  format: RenderFormat;
  outputPath?: string;
  inputKind?: "svg";
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
    workspaceTrusted: vscode.workspace.isTrusted,
  });
}

export async function renderMermanSource(request: RenderRequest): Promise<RenderResult> {
  const directory = request.themeFileContent !== undefined &&
    request.format !== "ascii" && request.format !== "unicode"
    ? await fs.mkdtemp(path.join(os.tmpdir(), "merman-theme-"))
    : undefined;
  try {
    const themeFilePath = directory ? path.join(directory, "theme.json") : undefined;
    if (themeFilePath) {
      await fs.writeFile(themeFilePath, request.themeFileContent!, { encoding: "utf8", mode: 0o600 });
    }
    const args = renderMermanArgs({ ...request, themeFilePath });
    const invocation = resolveCliInvocation(request.context, args);
    request.outputChannel.info(
      `${request.signalLabel ?? "render"}=${invocation.source} command="${invocation.command}" args="${invocation.args.join(" ")}"`,
    );
    return await runRenderProcess({
      invocation,
      source: request.source,
      signal: request.signal,
    });
  } finally {
    if (directory) {
      await fs.rm(directory, { recursive: true, force: true });
    }
  }
}
