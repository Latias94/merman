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
  configNamespace?: string | null,
): MermaidCanvasTone {
  let effectiveTheme = normalizeMermaidThemeName(selectedTheme);
  try {
    const config = buildMermaidConfig(configJson, selectedTheme);
    // The playground inserts config after frontmatter and before authored directives.
    // Project only theme fields; scoped values survive later global-only overrides.
    const themes: ThemeFields = {};
    mergeThemeFields(themes, frontmatterConfig(source), configNamespace);
    mergeThemeFields(themes, config, configNamespace);
    for (const directive of directiveConfigs(source)) {
      mergeThemeFields(themes, directive, configNamespace);
    }
    const resolved = [themes.scoped, themes.global].find(
      (value): value is ThemeName | "null" => value === "null" || isKnownTheme(value),
    );
    // The string sentinel skips theme recomputation and retains initialize()'s palette.
    effectiveTheme = resolved === "null"
      ? normalizeMermaidThemeName(typeof config.theme === "string" ? config.theme : undefined)
      : resolved ?? (themes.sawThemeField ? "default" : effectiveTheme);
  } catch {
    // Invalid config is rendered as an error; keep the selected-theme canvas.
  }
  return MERMAID_CANVAS_TONES[effectiveTheme];
}

function frontmatterConfig(source: string): MermaidConfigObject | null {
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
    return parsed.config;
  } catch {
    return null;
  }
}

function* directiveConfigs(source: string): Generator<MermaidConfigObject> {
  const directiveStart = /%%\{\s*(?:init|initialize)\s*:\s*/gi;
  const directiveEnd = /\}\s*%%/g;

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
      if (isPlainObject(config)) yield config;
    } catch {
      return;
    }
  }
}

function isPlainObject(value: unknown): value is MermaidConfigObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

interface ThemeFields {
  global?: unknown;
  scoped?: unknown;
  sawThemeField?: boolean;
}

function isKnownTheme(value: unknown): value is ThemeName {
  return typeof value === "string" && Object.hasOwn(MERMAID_CANVAS_TONES, value);
}

function mergeThemeFields(
  themes: ThemeFields,
  config: MermaidConfigObject | null,
  namespace: string | null | undefined,
): void {
  if (!config) return;
  // assignWithDepth ignores JSON null, but unknown strings replace previous values.
  // Validate after merging so a rejected scoped value can fall back to the global one.
  if (config.theme != null) {
    themes.sawThemeField = true;
    themes.global = config.theme;
  }
  const section = namespace ? config[namespace] : undefined;
  if (isPlainObject(section) && section.theme != null) {
    themes.sawThemeField = true;
    themes.scoped = section.theme;
  }
}
