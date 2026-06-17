export type DiagramFont =
  | "system"
  | "trebuchet"
  | "arial"
  | "georgia"
  | "monospace";

export const DIAGRAM_FONT_VALUES: readonly DiagramFont[] = [
  "trebuchet",
  "system",
  "arial",
  "georgia",
  "monospace",
];

const DIAGRAM_FONT_STACKS: Record<DiagramFont, string> = {
  system:
    'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  trebuchet: '"trebuchet ms", verdana, arial, sans-serif',
  arial: "Arial, Helvetica, sans-serif",
  georgia: 'Georgia, "Times New Roman", serif',
  monospace:
    '"JetBrains Mono", "Fira Code", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
};

export function diagramFontStack(font: DiagramFont): string {
  return DIAGRAM_FONT_STACKS[font] ?? DIAGRAM_FONT_STACKS.trebuchet;
}

export function isDiagramFont(value: string | undefined): value is DiagramFont {
  return Boolean(
    value && DIAGRAM_FONT_VALUES.includes(value as DiagramFont)
  );
}
