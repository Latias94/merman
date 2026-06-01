import { normalizeThemeName } from "@merman/web";

export type MermaidConfigObject = Record<string, unknown>;

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
  theme: string
): MermaidConfigObject {
  const config = { ...parseMermaidConfigJson(configJson) };
  const normalizedTheme = normalizeThemeName(theme);
  if (normalizedTheme !== "default" && config.theme === undefined) {
    config.theme = normalizedTheme;
  }
  return config;
}

export function sourceWithConfig(
  source: string,
  theme: string,
  configJson: string
): string {
  const config = buildMermaidConfig(configJson, theme);
  if (Object.keys(config).length === 0) {
    return source;
  }

  const directive = `%%{init: ${JSON.stringify(config)}}%%`;
  return insertDirectiveAfterFrontmatter(source, directive);
}

function insertDirectiveAfterFrontmatter(source: string, directive: string): string {
  const newline = source.includes("\r\n") ? "\r\n" : "\n";
  const lines = source.split(/\r?\n/);

  if (lines[0]?.trim() === "---") {
    const frontmatterEnd = lines.findIndex(
      (line, index) => index > 0 && line.trim() === "---"
    );
    if (frontmatterEnd > 0) {
      return [
        ...lines.slice(0, frontmatterEnd + 1),
        directive,
        ...lines.slice(frontmatterEnd + 1),
      ].join(newline);
    }
  }

  return `${directive}${newline}${source}`;
}

function isPlainObject(value: unknown): value is MermaidConfigObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
