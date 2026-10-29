export type DiagramFont =
  | "inherit"
  | "system"
  | "trebuchet"
  | "arial"
  | "georgia"
  | "monospace";

export const DIAGRAM_FONT_VALUES: readonly DiagramFont[] = [
  "inherit",
  "trebuchet",
  "system",
  "arial",
  "georgia",
  "monospace",
];

const DIAGRAM_FONT_STACKS: Record<Exclude<DiagramFont, "inherit">, string> = {
  system:
    'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  trebuchet: '"trebuchet ms", verdana, arial, sans-serif',
  arial: "Arial, Helvetica, sans-serif",
  georgia: 'Georgia, "Times New Roman", serif',
  monospace:
    '"JetBrains Mono", "Fira Code", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
};

export function diagramFontStack(font: DiagramFont): string | undefined {
  return font === "inherit" ? undefined : DIAGRAM_FONT_STACKS[font];
}

export function isDiagramFont(value: string | undefined): value is DiagramFont {
  return Boolean(
    value && DIAGRAM_FONT_VALUES.includes(value as DiagramFont)
  );
}
