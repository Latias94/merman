import * as path from "node:path";
import * as vscode from "vscode";

import { getPreviewSettings } from "./config.js";
import { previewCliBackground } from "./preview-background.js";
import { resolvedRenderSettings, type DiagramRenderSettings } from "./render-options.js";
import { discoverThemes, inspectThemeFile } from "./theme-runtime.js";
import { editorThemeAppearance, resolveThemeSelection } from "./theme-selection.js";

export async function configuredRenderSettings(
  context: vscode.ExtensionContext,
  resource: vscode.Uri,
): Promise<DiagramRenderSettings> {
  const settings = getPreviewSettings(resource);
  const discovery = await discoverThemes(context);
  const imported = settings.diagramTheme === "file"
    ? await inspectThemeFile(context, configuredThemeFileUri(settings.themeFile, resource), discovery)
    : undefined;
  const resolved = resolveThemeSelection(
    discovery,
    settings.diagramTheme,
    settings.appearance,
    editorThemeAppearance(vscode.window.activeColorTheme.kind),
    imported,
  );
  return resolvedRenderSettings(resolved, previewCliBackground(settings.background));
}

export function configuredThemeFileUri(file: string, resource?: vscode.Uri): vscode.Uri {
  if (!file) {
    throw new Error("Set merman.preview.themeFile to a theme recipe JSON file, or import a theme in the preview.");
  }
  if (path.isAbsolute(file)) {
    return vscode.Uri.file(file);
  }
  const workspace = resource ? vscode.workspace.getWorkspaceFolder(resource) : vscode.workspace.workspaceFolders?.[0];
  if (!workspace) {
    throw new Error("Use an absolute theme recipe path outside a workspace.");
  }
  return vscode.Uri.joinPath(workspace.uri, file);
}
