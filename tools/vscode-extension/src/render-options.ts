export type RenderFormat = "svg" | "png" | "pdf";

export function renderMermanArgs(request: {
  format: RenderFormat;
  outputPath?: string;
  theme?: string;
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
  return args;
}
