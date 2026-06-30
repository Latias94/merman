export type RenderFormat = "svg" | "ascii" | "unicode" | "png" | "pdf";

export function renderMermanArgs(request: {
  format: RenderFormat;
  outputPath?: string;
  theme?: string;
  background?: string;
}): string[] {
  const args = [
    "-q",
    "-i",
    "-",
    "-o",
    request.outputPath ?? "-",
    "-e",
    request.format,
  ];
  if (request.theme && request.theme !== "source") {
    args.push("--theme", request.theme);
  }
  if (request.background) {
    args.push("--background-color", request.background);
  }
  return args;
}
