export type RenderFormat = "svg" | "ascii" | "unicode" | "png" | "pdf";

export function renderMermanArgs(request: {
  format: RenderFormat;
  outputPath?: string;
  theme?: string;
  background?: string;
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
  if (
    request.background &&
    request.format !== "ascii" &&
    request.format !== "unicode"
  ) {
    args.push("--background", request.background);
  }
  return args;
}
