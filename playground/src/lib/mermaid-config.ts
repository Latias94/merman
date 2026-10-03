import { diagramFontStack, type DiagramFont } from "./diagram-font.ts";
import { locateMermaidFrontmatter } from "./mermaid-frontmatter.ts";
import { isMermaidThemeSelection, normalizeMermaidThemeSelection, type ThemeName } from "./mermaid-theme-name.ts";

export type MermaidConfigObject = Record<string, unknown>;

export interface MermaidConfigBuildOptions {
  diagramFont?: DiagramFont;
}

export const DEFAULT_MERMAID_CONFIG = "{\n}\n";

export function parseMermaidConfigJson(configJson: string): MermaidConfigObject {
  const trimmed = configJson.trim();
  if (!trimmed) {
    return {};
  }

  const parsed = JSON.parse(trimmed) as unknown;
  if (!isPlainObject(parsed)) {
    throw new Error("Mermaid config must be a JSON object.");
  }
  return parsed;
}

export function formatMermaidConfigJson(configJson: string): string {
  return `${JSON.stringify(parseMermaidConfigJson(configJson), null, 2)}\n`;
}

export function buildMermaidConfig(
  configJson: string,
  theme: string,
  options: MermaidConfigBuildOptions = {}
): MermaidConfigObject {
  const config = { ...parseMermaidConfigJson(configJson) };
  const normalizedTheme = normalizeMermaidThemeSelection(theme);
  if (normalizedTheme !== "auto" && config.theme === undefined) {
    config.theme = normalizedTheme;
  }
  if (options.diagramFont) {
    applyDiagramFont(config, diagramFontStack(options.diagramFont));
  }
  return config;
}

// Only validated theme names and UI font choices belong to host initialization.
// Authored config stays in the source layer, including secure/CSS/resource keys,
// so each engine retains its own sanitizer and security policy.
export function buildMermaidOperationInput(
  source: string,
  theme: string,
  configJson: string,
  options: MermaidConfigBuildOptions = {},
): Readonly<{
  initializationConfig: Readonly<MermaidConfigObject>;
  configuredSource: string;
}> {
  const authoredConfig = buildMermaidConfig(configJson, theme, options);
  const initializationConfig = safeInitializationConfig(authoredConfig, options);
  const configuredSource = sourceWithMermaidConfig(source, authoredConfig);
  return Object.freeze({ initializationConfig, configuredSource });
}

function safeInitializationConfig(
  authoredConfig: MermaidConfigObject,
  options: MermaidConfigBuildOptions,
): Readonly<MermaidConfigObject> {
  const safe: MermaidConfigObject = {};
  const authoredTheme = authoredConfig.theme;
  if (isKnownThemeName(authoredTheme)) {
    safe.theme = authoredTheme;
  }
  // Font values come only from the validated UI enum. User config font/CSS fields
  // remain source directives and therefore pass through Mermaid/Core sanitization.
  if (options.diagramFont) {
    const fontFamily = diagramFontStack(options.diagramFont);
    safe.fontFamily = fontFamily;
    safe.themeVariables = Object.freeze({ fontFamily });
  }
  return Object.freeze(safe);
}

function isKnownThemeName(value: unknown): value is ThemeName {
  return typeof value === "string" && value !== "auto" && isMermaidThemeSelection(value);
}

export function sourceWithConfig(
  source: string,
  theme: string,
  configJson: string,
  options: MermaidConfigBuildOptions = {}
): string {
  const config = buildMermaidConfig(configJson, theme, options);
  return sourceWithMermaidConfig(source, config);
}

export function sourceWithMermaidConfig(
  source: string,
  config: MermaidConfigObject
): string {
  if (Object.keys(config).length === 0) {
    return source;
  }

  const directive = `%%{init: ${JSON.stringify(config)}}%%`;
  return insertDirectiveAfterFrontmatter(source, directive);
}

function insertDirectiveAfterFrontmatter(source: string, directive: string): string {
  const newline = /\r\n|[\r\n]/.exec(source)?.[0] ?? "\n";
  const insertion = locateMermaidFrontmatter(source)?.end ?? 0;
  return `${source.slice(0, insertion)}${directive}${newline}${source.slice(insertion)}`;
}

function isPlainObject(value: unknown): value is MermaidConfigObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function applyDiagramFont(config: MermaidConfigObject, fontFamily: string) {
  if (config.fontFamily === undefined) {
    config.fontFamily = fontFamily;
  }

  const themeVariables = isPlainObject(config.themeVariables)
    ? { ...config.themeVariables }
    : {};
  if (themeVariables.fontFamily === undefined) {
    themeVariables.fontFamily = fontFamily;
  }
  config.themeVariables = themeVariables;
}
