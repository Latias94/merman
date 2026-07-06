import * as path from "node:path";

import type { RenderFormat } from "./renderer.js";

export type ExportFormat = Extract<RenderFormat, "svg" | "png">;

export interface ExportPreset {
  label: string;
  description: string;
  format: ExportFormat;
  openAfterExport: boolean;
}

export const EXPORT_PRESETS: readonly ExportPreset[] = [
  {
    label: "SVG",
    description: "Vector image",
    format: "svg",
    openAfterExport: false,
  },
  {
    label: "PNG",
    description: "Bitmap image",
    format: "png",
    openAfterExport: false,
  },
  {
    label: "SVG and Open",
    description: "Export, then open the file",
    format: "svg",
    openAfterExport: true,
  },
  {
    label: "PNG and Open",
    description: "Export, then open the file",
    format: "png",
    openAfterExport: true,
  },
];

export function defaultExportPath(
  sourceFilePath: string,
  exportBaseName: string,
  format: ExportFormat,
): string {
  return path.join(path.dirname(sourceFilePath), `${exportBaseName}.${format}`);
}

export function exportFilters(format: ExportFormat): Record<string, string[]> {
  switch (format) {
    case "png":
      return { "PNG image": ["png"] };
    case "svg":
    default:
      return { "SVG image": ["svg"] };
  }
}

export function exportPresetForFormat(format: ExportFormat): ExportPreset {
  return EXPORT_PRESETS.find((preset) => preset.format === format && !preset.openAfterExport) ?? {
    label: format.toUpperCase(),
    description: "Export diagram",
    format,
    openAfterExport: false,
  };
}

export function displayExportBasename(uri: vscodeUriLike): string {
  if (uri.path) {
    return path.posix.basename(uri.path);
  }
  return path.basename(path.win32.basename(uri.fsPath));
}

export function pngClipboardCommand(platform: NodeJS.Platform): string | undefined {
  switch (platform) {
    case "darwin":
      return "osascript";
    case "win32":
      return "powershell.exe";
    case "linux":
      return "wl-copy";
    default:
      return undefined;
  }
}

export const PNG_CLIPBOARD_AVAILABLE_CONTEXT = "merman.pngClipboardAvailable";

export function pngClipboardAvailable(
  platform: NodeJS.Platform,
  remoteName: string | undefined,
): boolean {
  return remoteName === undefined && pngClipboardCommand(platform) !== undefined;
}

export function pngClipboardArgs(platform: NodeJS.Platform, imagePath: string): string[] {
  switch (platform) {
    case "darwin":
      return [
        "-e",
        `set the clipboard to (read (POSIX file ${JSON.stringify(imagePath)}) as ${appleScriptPngClass()})`,
      ];
    case "win32":
      return [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "param([string]$imagePath) $ErrorActionPreference = 'Stop'; Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $img = [Drawing.Image]::FromFile($imagePath); try { [Windows.Forms.Clipboard]::SetImage($img) } finally { $img.Dispose() }",
        imagePath,
      ];
    case "linux":
      return ["--type", "image/png"];
    default:
      return [];
  }
}

interface vscodeUriLike {
  readonly fsPath: string;
  readonly path?: string;
}

function appleScriptPngClass(): string {
  return `${String.fromCharCode(0x00ab)}class PNGf${String.fromCharCode(0x00bb)}`;
}
