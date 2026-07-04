import * as path from "node:path";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as vscode from "vscode";

import {
  EXPORT_PRESETS,
  exportPresetForFormat,
  pngClipboardArgs,
  pngClipboardCommand,
  type ExportPreset,
} from "./export-options.js";
import { exportRenderedDiagram, renderSafeSvg } from "./export-workflow.js";
import { runClipboardCommand } from "./clipboard-command.js";
import { renderMermanSource } from "./renderer.js";
import {
  extractPreviewInput,
  extractPreviewInputFromDocument,
  type PreviewInput,
} from "./preview-source.js";
import {
  mermaidSourceCommandSourceId,
  mermaidSourceCommandUri,
  type MermaidSourceCommandArgument,
} from "./source-actions.js";

const EXPORT_SVG_COMMAND = "merman.exportSvg";
const EXPORT_PNG_COMMAND = "merman.exportPng";
const EXPORT_COMMAND = "merman.export";
const COPY_SVG_COMMAND = "merman.copySvg";
const COPY_PNG_COMMAND = "merman.copyPng";

export function registerExport(context: vscode.ExtensionContext): void {
  const outputChannel = vscode.window.createOutputChannel("Merman Export", { log: true });
  context.subscriptions.push(outputChannel);
  context.subscriptions.push(
    vscode.commands.registerCommand(
      EXPORT_SVG_COMMAND,
      async (target?: MermaidSourceCommandArgument) => {
        await exportDiagram(context, outputChannel, exportPresetForFormat("svg"), target);
      },
    ),
    vscode.commands.registerCommand(
      EXPORT_PNG_COMMAND,
      async (target?: MermaidSourceCommandArgument) => {
        await exportDiagram(context, outputChannel, exportPresetForFormat("png"), target);
      },
    ),
    vscode.commands.registerCommand(EXPORT_COMMAND, async (target?: MermaidSourceCommandArgument) => {
      const preset = await pickExportPreset();
      if (!preset) {
        return;
      }
      await exportDiagram(context, outputChannel, preset, target);
    }),
    vscode.commands.registerCommand(
      COPY_SVG_COMMAND,
      async (target?: MermaidSourceCommandArgument) => {
        await copySvg(context, outputChannel, target);
      },
    ),
    vscode.commands.registerCommand(
      COPY_PNG_COMMAND,
      async (target?: MermaidSourceCommandArgument) => {
        await copyPng(context, outputChannel, target);
      },
    ),
  );
}

async function exportDiagram(
  context: vscode.ExtensionContext,
  outputChannel: vscode.LogOutputChannel,
  preset: ExportPreset,
  target?: MermaidSourceCommandArgument,
): Promise<void> {
  const source = await resolveExportSource(target);
  if (!source) {
    void vscode.window.showWarningMessage(
      "Focus a Mermaid file or a Markdown Mermaid fence before exporting.",
    );
    return;
  }

  await exportRenderedDiagram({
    context,
    outputChannel,
    sourceUri: source.document.uri,
    exportBaseName: source.input.exportBaseName,
    source: source.input.source,
    format: preset.format,
    openAfterExport: preset.openAfterExport,
    signalLabel: `export-${preset.format}`,
    progressTitle: `Exporting Mermaid ${preset.format.toUpperCase()}`,
    failureMessagePrefix: "Merman export failed",
  });
}

async function copySvg(
  context: vscode.ExtensionContext,
  outputChannel: vscode.LogOutputChannel,
  target?: MermaidSourceCommandArgument,
): Promise<void> {
  const source = await resolveExportSource(target);
  if (!source) {
    void vscode.window.showWarningMessage(
      "Focus a Mermaid file or a Markdown Mermaid fence before copying SVG.",
    );
    return;
  }

  try {
    const svg = await renderSafeSvg(context, outputChannel, source.input.source, "copy-svg");
    await vscode.env.clipboard.writeText(svg);
    void vscode.window.showInformationMessage("Copied Mermaid SVG to clipboard.");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    outputChannel.error(message);
    void vscode.window.showErrorMessage(`Merman SVG copy failed: ${message}`);
  }
}

async function copyPng(
  context: vscode.ExtensionContext,
  outputChannel: vscode.LogOutputChannel,
  target?: MermaidSourceCommandArgument,
): Promise<void> {
  const source = await resolveExportSource(target);
  if (!source) {
    void vscode.window.showWarningMessage(
      "Focus a Mermaid file or a Markdown Mermaid fence before copying PNG.",
    );
    return;
  }

  const command = pngClipboardCommand(process.platform);
  if (!command) {
    void vscode.window.showInformationMessage(
      "PNG clipboard copy is not available on this platform. Choose a file to save the PNG instead.",
    );
    await exportDiagram(context, outputChannel, exportPresetForFormat("png"), target);
    return;
  }

  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "merman-vscode-"));
  const tempPath = path.join(tempDir, `${source.input.exportBaseName}.png`);
  try {
    await renderMermanSource({
      context,
      source: source.input.source,
      format: "png",
      outputPath: tempPath,
      outputChannel,
      signalLabel: "copy-png",
    });
    const stdin =
      process.platform === "linux" ? await fs.readFile(tempPath) : undefined;
    await runClipboardCommand(command, pngClipboardArgs(process.platform, tempPath), stdin);
    void vscode.window.showInformationMessage("Copied Mermaid PNG to clipboard.");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    outputChannel.error(message);
    void vscode.window.showWarningMessage(
      `Merman PNG clipboard copy failed: ${message}. Choose a file to save the PNG instead.`,
    );
    await exportDiagram(context, outputChannel, exportPresetForFormat("png"), target);
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
}

async function resolveExportSource(
  target?: MermaidSourceCommandArgument,
): Promise<{ document: vscode.TextDocument; input: PreviewInput } | undefined> {
  const activeEditor = vscode.window.activeTextEditor;
  const resource = mermaidSourceCommandUri(target);
  const sourceId = mermaidSourceCommandSourceId(target);
  if (resource) {
    if (activeEditor?.document.uri.toString() === resource.toString()) {
      const input = extractPreviewInput(activeEditor, sourceId);
      return input ? { document: activeEditor.document, input } : undefined;
    }
    const document = await vscode.workspace.openTextDocument(resource);
    const input = extractPreviewInputFromDocument(document, undefined, sourceId);
    return input ? { document, input } : undefined;
  }

  if (!activeEditor) {
    return undefined;
  }
  const input = extractPreviewInput(activeEditor);
  return input ? { document: activeEditor.document, input } : undefined;
}

export async function pickExportPreset(): Promise<ExportPreset | undefined> {
  const picked = await vscode.window.showQuickPick(
    EXPORT_PRESETS.map((preset) => ({
      label: preset.label,
      description: preset.description,
      preset,
    })),
    {
      placeHolder: "Choose a Mermaid export format",
    },
  );

  return picked?.preset;
}
