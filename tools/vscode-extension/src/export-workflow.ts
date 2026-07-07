import * as vscode from "vscode";

import {
  defaultExportPath,
  displayExportBasename,
  exportFilters,
  type ExportFormat,
} from "./export-options.js";
import { errorMessage } from "./error-message.js";
import { assertSafePreviewSvg } from "./preview-svg-safety.js";
import { renderMermanSource } from "./renderer.js";

export interface ExportRenderedDiagramRequest {
  context: vscode.ExtensionContext;
  outputChannel: vscode.LogOutputChannel;
  sourceUri: vscode.Uri;
  exportBaseName: string;
  source: string;
  format: ExportFormat;
  theme?: string;
  background?: string;
  openAfterExport?: boolean;
  signalLabel: string;
  progressTitle?: string;
  failureMessagePrefix: string;
}

export async function exportRenderedDiagram(
  request: ExportRenderedDiagramRequest,
): Promise<boolean> {
  const target = await vscode.window.showSaveDialog({
    defaultUri: defaultExportUri(request.sourceUri, request.exportBaseName, request.format),
    filters: exportFilters(request.format),
    saveLabel: `Export ${request.format.toUpperCase()}`,
  });
  if (!target) {
    return false;
  }

  const run = async (): Promise<boolean> => {
    try {
      await writeRenderedExport({ ...request, target });
      if (request.openAfterExport) {
        await vscode.commands.executeCommand("vscode.open", target);
      }
      void vscode.window.showInformationMessage(`Exported ${displayExportBasename(target)}.`);
      return true;
    } catch (error) {
      const message = errorMessage(error);
      request.outputChannel.error(message);
      void vscode.window.showErrorMessage(`${request.failureMessagePrefix}: ${message}`);
      return false;
    }
  };

  if (!request.progressTitle) {
    return run();
  }

  return vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: request.progressTitle,
      cancellable: false,
    },
    run,
  );
}

interface WriteRenderedExportRequest extends ExportRenderedDiagramRequest {
  target: vscode.Uri;
}

async function writeRenderedExport(request: WriteRenderedExportRequest): Promise<void> {
  if (request.format === "svg") {
    const svg = await renderSafeSvg(
      request.context,
      request.outputChannel,
      request.source,
      request.signalLabel,
      request.theme,
      request.background,
    );
    await vscode.workspace.fs.writeFile(request.target, Buffer.from(svg, "utf8"));
    return;
  }

  await renderSafeRaster({
    context: request.context,
    outputChannel: request.outputChannel,
    source: request.source,
    format: request.format,
    outputPath: request.target.fsPath,
    signalLabel: request.signalLabel,
    theme: request.theme,
    background: request.background,
  });
}

export interface RenderSafeRasterRequest {
  context: vscode.ExtensionContext;
  outputChannel: vscode.LogOutputChannel;
  source: string;
  format: Exclude<ExportFormat, "svg">;
  outputPath: string;
  signalLabel: string;
  theme?: string;
  background?: string;
}

export async function renderSafeRaster(
  request: RenderSafeRasterRequest,
): Promise<void> {
  const svg = await renderSafeSvg(
    request.context,
    request.outputChannel,
    request.source,
    request.signalLabel,
    request.theme,
    request.background,
  );
  await renderMermanSource({
    context: request.context,
    source: svg,
    format: request.format,
    outputPath: request.outputPath,
    background: request.background,
    outputChannel: request.outputChannel,
    signalLabel: request.signalLabel,
  });
}

export async function renderSafeSvg(
  context: vscode.ExtensionContext,
  outputChannel: vscode.LogOutputChannel,
  source: string,
  signalLabel: string,
  theme?: string,
  background?: string,
): Promise<string> {
  const result = await renderMermanSource({
    context,
    source,
    format: "svg",
    theme,
    background,
    outputChannel,
    signalLabel,
  });
  const svg = result.stdout.toString("utf8");
  assertSafePreviewSvg(svg);
  return svg;
}

function defaultExportUri(
  sourceUri: vscode.Uri,
  exportBaseName: string,
  format: ExportFormat,
): vscode.Uri | undefined {
  if (sourceUri.scheme !== "file") {
    return undefined;
  }
  return vscode.Uri.file(defaultExportPath(sourceUri.fsPath, exportBaseName, format));
}
