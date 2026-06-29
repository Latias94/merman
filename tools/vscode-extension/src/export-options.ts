import * as path from "node:path";

import type { RenderFormat } from "./renderer.js";

export interface ExportPreset {
  label: string;
  description: string;
  format: RenderFormat;
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
  format: RenderFormat,
): string {
  return path.join(path.dirname(sourceFilePath), `${exportBaseName}.${format}`);
}

export function exportFilters(format: RenderFormat): Record<string, string[]> {
  switch (format) {
    case "png":
      return { "PNG image": ["png"] };
    case "pdf":
      return { "PDF document": ["pdf"] };
    case "svg":
    default:
      return { "SVG image": ["svg"] };
  }
}

export function exportPresetForFormat(format: RenderFormat): ExportPreset {
  return EXPORT_PRESETS.find((preset) => preset.format === format && !preset.openAfterExport) ?? {
    label: format.toUpperCase(),
    description: "Export diagram",
    format,
    openAfterExport: false,
  };
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
        `[Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms') | Out-Null; [Reflection.Assembly]::LoadWithPartialName('System.Drawing') | Out-Null; $img=[Drawing.Image]::FromFile(${JSON.stringify(imagePath)}); [Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()`,
      ];
    case "linux":
      return ["--type", "image/png"];
    default:
      return [];
  }
}

function appleScriptPngClass(): string {
  return `${String.fromCharCode(0x00ab)}class PNGf${String.fromCharCode(0x00bb)}`;
}
