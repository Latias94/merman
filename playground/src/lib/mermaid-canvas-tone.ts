import { JSON_SCHEMA, load as parseYaml } from "js-yaml";

import { buildMermaidConfig, type MermaidConfigObject } from "./mermaid-config.ts";
import {
  normalizeMermaidThemeName,
  type ThemeName,
} from "./mermaid-theme-name.ts";

export type MermaidCanvasTone = "light" | "dark";

const MERMAID_CANVAS_TONES = {
  default: "light",
  base: "light",
  dark: "dark",
  forest: "light",
  neutral: "light",
  neo: "light",
  "neo-dark": "dark",
  redux: "light",
  "redux-dark": "dark",
  "redux-color": "light",
  "redux-dark-color": "dark",
} as const satisfies Record<ThemeName, MermaidCanvasTone>;

export function resolveMermaidCanvasTone(
  configJson: string,
  selectedTheme: string,
  source = "",
): MermaidCanvasTone {
  let effectiveTheme = normalizeMermaidThemeName(selectedTheme);
  try {
    const config = buildMermaidConfig(configJson, selectedTheme);
    if (typeof config.theme === "string") {
      effectiveTheme = normalizeMermaidThemeName(config.theme);
    } else {
      effectiveTheme = frontmatterTheme(source) ?? effectiveTheme;
    }

    effectiveTheme = directiveTheme(source) ?? effectiveTheme;
  } catch {
    // Invalid config is rendered as an error; keep the selected-theme canvas.
  }
  return MERMAID_CANVAS_TONES[effectiveTheme];
}

function frontmatterTheme(source: string): ThemeName | null {
  const match = /^([^\S\n\r]*)-{3}\s*[\n\r](.*?)[\n\r]\1-{3}\s*[\n\r]+/s.exec(
    source,
  );
  if (!match) return null;

  const openingIndent = match[1] ?? "";
  const body = (match[2] ?? "")
    .split(/\r?\n/)
    .map((line) =>
      openingIndent && line.startsWith(openingIndent)
        ? line.slice(openingIndent.length)
        : line,
    )
    .join("\n");

  try {
    const parsed = parseYaml(body, { schema: JSON_SCHEMA }) as unknown;
    if (!isPlainObject(parsed) || !isPlainObject(parsed.config)) return null;
    return typeof parsed.config.theme === "string"
      ? normalizeMermaidThemeName(parsed.config.theme)
      : null;
  } catch {
    return null;
  }
}

function directiveTheme(source: string): ThemeName | null {
  const directiveStart = /%%\{\s*(?:init|initialize)\s*:\s*/gi;
  const directiveEnd = /\}\s*%%/g;
  let effectiveTheme: ThemeName | null = null;

  for (
    let start = directiveStart.exec(source);
    start;
    start = directiveStart.exec(source)
  ) {
    directiveEnd.lastIndex = directiveStart.lastIndex;
    const end = directiveEnd.exec(source);
    if (!end) break;

    const body = source.slice(directiveStart.lastIndex, end.index);
    directiveStart.lastIndex = directiveEnd.lastIndex;
    try {
      const config = JSON.parse(body.trim().replaceAll("'", '"')) as unknown;
      if (isPlainObject(config) && typeof config.theme === "string") {
        effectiveTheme = normalizeMermaidThemeName(config.theme);
      }
    } catch {
      return null;
    }
  }
  return effectiveTheme;
}

function isPlainObject(value: unknown): value is MermaidConfigObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
