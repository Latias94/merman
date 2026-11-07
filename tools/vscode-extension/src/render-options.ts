import type { ResolvedTheme, ThemeAppearance } from "./theme-selection.js";

export type RenderFormat = "svg" | "ascii" | "unicode" | "png" | "pdf";

export interface DiagramRenderSettings {
  theme?: string;
  themePreset?: string;
  themeFileContent?: string;
  themeAppearance?: ThemeAppearance;
  background?: string;
}

export function resolvedRenderSettings(
  theme: ResolvedTheme | undefined,
  background?: string,
): DiagramRenderSettings {
  return {
    theme: theme?.theme,
    themePreset: theme?.themePreset,
    themeFileContent: theme?.themeFileContent,
    themeAppearance: theme?.themePreset || theme?.themeFileContent
      ? theme.effectiveAppearance
      : undefined,
    background,
  };
}

export function renderMermanArgs(request: DiagramRenderSettings & {
  format: RenderFormat;
  outputPath?: string;
  themeFilePath?: string;
  inputKind?: "svg";
}): string[] {
  const args = [
    "render",
    "-",
    "--output",
    request.outputPath ?? "-",
    "--format",
    request.format,
    "--quiet",
  ];
  if (request.theme && request.theme !== "source") {
    args.push("--theme", request.theme);
  }
  if (request.inputKind) {
    args.push("--input-kind", request.inputKind);
  }
  const graphical = request.format !== "ascii" && request.format !== "unicode";
  if (graphical && request.themePreset) {
    args.push("--theme-preset", request.themePreset);
  }
  if (graphical && request.themeFilePath) {
    args.push("--theme-file", request.themeFilePath);
  }
  if (graphical && request.themeAppearance) {
    args.push("--theme-appearance", request.themeAppearance);
  }
  if (
    request.background && graphical
  ) {
    args.push("--background", request.background);
  }
  return args;
}
