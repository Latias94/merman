import * as path from "node:path";
import * as vscode from "vscode";

import { defaultExportPath, exportFilters } from "./export-options.js";
import { renderMermanSource, type RenderFormat } from "./renderer.js";
import {
  extractPreviewInput,
  extractPreviewInputFromDocument,
  type PreviewInput,
} from "./preview-source.js";

const EXPORT_SVG_COMMAND = "merman.exportSvg";
const EXPORT_PNG_COMMAND = "merman.exportPng";
const COPY_SVG_COMMAND = "merman.copySvg";
const COPY_PNG_COMMAND = "merman.copyPng";

export function registerExport(context: vscode.ExtensionContext): void {
  const outputChannel = vscode.window.createOutputChannel("Merman Export", { log: true });
  context.subscriptions.push(outputChannel);
  context.subscriptions.push(
    vscode.commands.registerCommand(EXPORT_SVG_COMMAND, async (resource?: vscode.Uri) => {
      await exportDiagram(context, outputChannel, "svg", resource);
    }),
    vscode.commands.registerCommand(EXPORT_PNG_COMMAND, async (resource?: vscode.Uri) => {
      await exportDiagram(context, outputChannel, "png", resource);
    }),
    vscode.commands.registerCommand(COPY_SVG_COMMAND, async (resource?: vscode.Uri) => {
      await copySvg(context, outputChannel, resource);
    }),
    vscode.commands.registerCommand(COPY_PNG_COMMAND, async (resource?: vscode.Uri) => {
      void vscode.window.showInformationMessage(
        "PNG clipboard copy is not available in this VS Code build. Choose a file to save the PNG instead.",
      );
      await exportDiagram(context, outputChannel, "png", resource);
    }),
  );
}

async function exportDiagram(
  context: vscode.ExtensionContext,
  outputChannel: vscode.LogOutputChannel,
  format: RenderFormat,
  resource?: vscode.Uri,
): Promise<void> {
  const source = await resolveExportSource(resource);
  if (!source) {
    void vscode.window.showWarningMessage(
      "Focus a Mermaid file or a Markdown Mermaid fence before exporting.",
    );
    return;
  }

  const target = await vscode.window.showSaveDialog({
    defaultUri: defaultExportUri(source.document.uri, source.input, format),
    filters: exportFilters(format),
    saveLabel: `Export ${format.toUpperCase()}`,
  });
  if (!target) {
    return;
  }

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `Exporting Mermaid ${format.toUpperCase()}`,
      cancellable: false,
    },
    async () => {
      try {
        await renderMermanSource({
          context,
          source: source.input.source,
          format,
          outputPath: target.fsPath,
          outputChannel,
          signalLabel: `export-${format}`,
        });
        void vscode.window.showInformationMessage(`Exported ${path.basename(target.fsPath)}.`);
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
  resource?: vscode.Uri,
): Promise<void> {
  const source = await resolveExportSource(resource);
  if (!source) {
    void vscode.window.showWarningMessage(
      "Focus a Mermaid file or a Markdown Mermaid fence before copying SVG.",
    );
    return;
  }

  try {
    const result = await renderMermanSource({
      context,
      source: source.input.source,
      format: "svg",
      outputChannel,
      signalLabel: "copy-svg",
    });
    await vscode.env.clipboard.writeText(result.stdout.toString("utf8"));
    void vscode.window.showInformationMessage("Copied Mermaid SVG to clipboard.");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    outputChannel.error(message);
    void vscode.window.showErrorMessage(`Merman SVG copy failed: ${message}`);
  }
}

async function resolveExportSource(
  resource?: vscode.Uri,
): Promise<{ document: vscode.TextDocument; input: PreviewInput } | undefined> {
  const activeEditor = vscode.window.activeTextEditor;
  if (resource) {
    if (activeEditor?.document.uri.toString() === resource.toString()) {
      const input = extractPreviewInput(activeEditor);
      return input ? { document: activeEditor.document, input } : undefined;
    }
    const document = await vscode.workspace.openTextDocument(resource);
    const input = extractPreviewInputFromDocument(document);
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
  format: RenderFormat,
): vscode.Uri | undefined {
  if (sourceUri.scheme !== "file") {
    return undefined;
  }
  return vscode.Uri.file(defaultExportPath(sourceUri.fsPath, input.exportBaseName, format));
}
