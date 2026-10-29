import { authoredMermaidConfigLayers } from "./mermaid-appearance.ts";
import { buildMermaidOperationInput, type MermaidConfigObject } from "./mermaid-config.ts";
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
    // The playground inserts config after frontmatter and before authored directives.
    // Project only theme fields; scoped values survive later global-only overrides.
    const initialized: ThemeFields = {};
    mergeThemeFields(initialized, buildMermaidOperationInput(source, selectedTheme, configJson).initializationConfig, configNamespace);
    const themes: ThemeFields = {};
    for (const { config } of authoredMermaidConfigLayers(source, configJson)) {
      mergeThemeFields(themes, config, configNamespace);
    }
    const resolved = [themes.scoped, themes.global, initialized.scoped, initialized.global].find(
      (value): value is ThemeName | "null" => value === "null" || isKnownTheme(value),
    );
    // The string sentinel skips theme recomputation and retains initialize()'s palette.
    effectiveTheme = resolved === "null"
      ? normalizeMermaidThemeName(typeof initialized.global === "string" ? initialized.global : undefined)
      : resolved ?? "default";
  } catch {
    // Invalid config is rendered as an error; keep the selected-theme canvas.
  }
  return MERMAID_CANVAS_TONES[effectiveTheme];
}

function isPlainObject(value: unknown): value is MermaidConfigObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

interface ThemeFields {
  global?: unknown;
  scoped?: unknown;
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
    themes.global = config.theme;
  }
  const section = namespace ? config[namespace] : undefined;
  if (isPlainObject(section) && section.theme != null) {
    themes.scoped = section.theme;
  }
}
