import * as path from "node:path";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as vscode from "vscode";

import {
  EXPORT_PRESETS,
  defaultExportPath,
  exportFilters,
  exportPresetForFormat,
  pngClipboardArgs,
  pngClipboardCommand,
  type ExportFormat,
  type ExportPreset,
} from "./export-options.js";
import { runClipboardCommand } from "./clipboard-command.js";
import { renderMermanSource } from "./renderer.js";
import {
  extractPreviewInput,
  extractPreviewInputFromDocument,
  type PreviewInput,
} from "./preview-source.js";
import { assertSafePreviewSvg } from "./preview-svg-safety.js";
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

  const outputUri = await vscode.window.showSaveDialog({
    defaultUri: defaultExportUri(source.document.uri, source.input, preset.format),
    filters: exportFilters(preset.format),
    saveLabel: `Export ${preset.format.toUpperCase()}`,
  });
  if (!outputUri) {
    return;
  }

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `Exporting Mermaid ${preset.format.toUpperCase()}`,
      cancellable: false,
    },
    async () => {
      try {
        if (preset.format === "svg") {
          const svg = await renderSafeSvg(context, outputChannel, source.input.source, "export-svg");
          await fs.writeFile(outputUri.fsPath, svg, "utf8");
        } else {
          await renderMermanSource({
            context,
            source: source.input.source,
            format: preset.format,
            outputPath: outputUri.fsPath,
            outputChannel,
            signalLabel: `export-${preset.format}`,
          });
        }
        if (preset.openAfterExport) {
          await vscode.commands.executeCommand("vscode.open", outputUri);
        }
        void vscode.window.showInformationMessage(`Exported ${path.basename(outputUri.fsPath)}.`);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        outputChannel.error(message);
        void vscode.window.showErrorMessage(`Merman export failed: ${message}`);
      }
    },
  );
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

async function renderSafeSvg(
  context: vscode.ExtensionContext,
  outputChannel: vscode.LogOutputChannel,
  source: string,
  signalLabel: string,
): Promise<string> {
  const result = await renderMermanSource({
    context,
    source,
    format: "svg",
    outputChannel,
    signalLabel,
  });
  const svg = result.stdout.toString("utf8");
  assertSafePreviewSvg(svg);
  return svg;
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

function defaultExportUri(
  sourceUri: vscode.Uri,
  input: PreviewInput,
  format: ExportFormat,
): vscode.Uri | undefined {
  if (sourceUri.scheme !== "file") {
    return undefined;
  }
  return vscode.Uri.file(defaultExportPath(sourceUri.fsPath, input.exportBaseName, format));
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
