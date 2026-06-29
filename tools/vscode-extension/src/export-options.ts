import * as path from "node:path";

import type { RenderFormat } from "./renderer.js";

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
