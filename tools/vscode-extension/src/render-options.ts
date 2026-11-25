import type { ResolvedTheme, ThemeAppearance } from "./theme-selection.js";

export type RenderFormat = "svg" | "ascii" | "unicode" | "png" | "pdf";

export function isFitWidth(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

export interface DiagramRenderSettings {
  theme?: string;
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
    themeFileContent: theme?.themeFileContent,
    themeAppearance: theme?.effectiveAppearance,
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
  const theme = request.themeFilePath ?? request.theme;
  if (theme && theme !== "source") {
    args.push("--theme", theme);
    if (request.themeAppearance) {
      args.push("--appearance", request.themeAppearance);
    }
    args.push("--theme-precedence", "host");
  }
  if (request.inputKind) {
    args.push("--input-kind", request.inputKind);
  }
  if (request.format === "ascii" || request.format === "unicode") {
    if (theme && theme !== "source") {
      args.push("--ascii-color", "truecolor");
    }
  } else if (request.background) {
    args.push("--background", request.background);
  }
  return args;
}
