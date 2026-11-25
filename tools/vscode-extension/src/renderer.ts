import * as vscode from "vscode";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { randomUUID } from "node:crypto";

import { resolveMermanBinary, type BinaryInvocation } from "./binaries.js";
import { previewLayoutFrameCapability, readCliMetadata, supportsLayoutFitWidth } from "./cli-capabilities.js";
import { getCliSettings } from "./config.js";
import { runRenderProcess } from "./render-process.js";
import { isFitWidth, renderMermanArgs, type DiagramRenderSettings, type RenderFormat } from "./render-options.js";
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
  previewFitWidth?: number;
  previewLayoutFrame?: { previous?: Buffer };
}

export interface RenderResult {
  stdout: Buffer;
  stderr: string;
  invocation: BinaryInvocation;
  previewLayoutFrame?: Buffer;
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
  const base = resolveCliInvocation(request.context, []);
  const withArgs = (args: readonly string[]): BinaryInvocation =>
    ({ ...base, args: [...base.args, ...args] });
  const { themeFileContent } = request;
  const preview = request.format === "svg" && request.outputPath === undefined && request.inputKind === undefined;
  const fitWidth = preview && isFitWidth(request.previewFitWidth) ? request.previewFitWidth : undefined;
  const capabilities = preview && (request.previewLayoutFrame || fitWidth !== undefined)
    ? await readCliMetadata(withArgs(["capabilities", "--json"])).catch(() => undefined)
    : undefined;
  const frame = preview && request.previewLayoutFrame ? previewLayoutFrameCapability(capabilities) : undefined;
  const previous = request.previewLayoutFrame?.previous;
  const replayFrame = frame && previous && previous.byteLength <= frame.maxBytes ? previous : undefined;
  request.signal?.throwIfAborted();
  const directory = themeFileContent !== undefined || replayFrame || (frame && !frame.privateOutput)
    ? await fs.mkdtemp(path.join(os.tmpdir(), "merman-render-"))
    : undefined;
  const frameOutput = frame && (directory
    ? path.join(directory, "frame-out.json")
    : path.join(os.tmpdir(), `merman-frame-${randomUUID()}.json`));
  try {
    let themeFilePath: string | undefined;
    if (directory && themeFileContent !== undefined) {
      themeFilePath = path.join(directory, "theme.json");
      await fs.writeFile(themeFilePath, themeFileContent, { encoding: "utf8", mode: 0o600 });
    }
    const args = renderMermanArgs({ ...request, themeFilePath });
    if (fitWidth !== undefined && supportsLayoutFitWidth(capabilities)) {
      args.push("--fit-width", "--width", String(fitWidth));
    }
    if (replayFrame && directory) {
      const frameInput = path.join(directory, "frame-in.json");
      await fs.writeFile(frameInput, replayFrame, { mode: 0o600 });
      args.push("--preview-frame-in", frameInput);
    }
    if (frameOutput) {
      args.push("--preview-frame-out", frameOutput);
    }
    const invocation = withArgs(args);
    request.outputChannel.info(
      `${request.signalLabel ?? "render"}=${invocation.source} command="${invocation.command}" args="${invocation.args.join(" ")}"`,
    );
    const result = await runRenderProcess({
      invocation,
      source: request.source,
      signal: request.signal,
    });
    if (!frameOutput || !frame || request.signal?.aborted) {
      return result;
    }
    const previewLayoutFrame = await readPreviewLayoutFrame(frameOutput, frame.maxBytes);
    return previewLayoutFrame ? { ...result, previewLayoutFrame } : result;
  } finally {
    if (directory) {
      await fs.rm(directory, { recursive: true, force: true });
    } else if (frameOutput) {
      await fs.rm(frameOutput, { force: true });
    }
  }
}

async function readPreviewLayoutFrame(filePath: string, limit: number): Promise<Buffer | undefined> {
  try {
    const stat = await fs.lstat(filePath);
    if (!stat.isFile() || stat.size === 0 || stat.size > limit) {
      return undefined;
    }
    const handle = await fs.open(filePath, "r");
    try {
      const bytes = Buffer.allocUnsafe(stat.size + 1);
      let offset = 0;
      while (offset < bytes.length) {
        const { bytesRead } = await handle.read(bytes, offset, bytes.length - offset, null);
        if (bytesRead === 0) {
          break;
        }
        offset += bytesRead;
      }
      if (offset === stat.size) {
        const frame = bytes.subarray(0, offset);
        const document: unknown = JSON.parse(frame.toString("utf8"));
        if (document && typeof document === "object" && !Array.isArray(document)) {
          return frame;
        }
      }
    } finally {
      await handle.close();
    }
  } catch {
    return undefined;
  }
  return undefined;
}
