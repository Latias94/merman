import { JSON_SCHEMA, load as parseYaml } from "js-yaml";
import { locateMermaidFrontmatter } from "./mermaid-frontmatter.ts";
import { parseMermaidConfigJson, type MermaidConfigObject } from "./mermaid-config.ts";

export interface MermaidAppearanceOverride {
  readonly origin: "config" | "source";
  readonly value: string;
}

/** Authored declarations only; this does not certify engine support or computed SVG styling. */
export function readMermaidAppearanceOverrides(source: string, configJson: string, namespace?: string | null): {
  theme?: MermaidAppearanceOverride;
  font?: MermaidAppearanceOverride;
  look?: MermaidAppearanceOverride;
} {
  const global: { theme?: MermaidAppearanceOverride; font?: MermaidAppearanceOverride; look?: MermaidAppearanceOverride } = {};
  const scoped: { theme?: MermaidAppearanceOverride; look?: MermaidAppearanceOverride } = {};
  try {
    for (const { config, origin } of authoredMermaidConfigLayers(source, configJson)) {
      function declaration(value: unknown): MermaidAppearanceOverride | undefined {
        return typeof value === "string" ? { origin, value } : undefined;
      }
      if (config.theme != null) global.theme = declaration(config.theme);
      if (config.look != null) global.look = declaration(config.look);
      if (config.fontFamily != null) global.font = declaration(config.fontFamily);
      if (isObject(config.themeVariables)) {
        if (config.themeVariables.fontFamily != null) global.font = declaration(config.themeVariables.fontFamily);
      }
      const section = namespace ? config[namespace] : undefined;
      if (isObject(section)) {
        if (section.theme != null) scoped.theme = declaration(section.theme);
        if (section.look != null) scoped.look = declaration(section.look);
      }
    }
  } catch {
    // Invalid authored JSON has its own renderer/editor error; do not guess its ownership.
    return {};
  }
  return { theme: scoped.theme ?? global.theme, font: global.font, look: scoped.look ?? global.look };
}

/** Same insertion order used by sourceWithMermaidConfig: frontmatter, workspace, source directives. */
export function* authoredMermaidConfigLayers(source: string, configJson: string): Generator<{
  readonly config: MermaidConfigObject;
  readonly origin: MermaidAppearanceOverride["origin"];
}> {
  const frontmatter = frontmatterConfig(source);
  if (frontmatter) yield { config: frontmatter, origin: "source" };
  yield { config: parseMermaidConfigJson(configJson), origin: "config" };
  for (const config of directiveConfigs(source)) yield { config, origin: "source" };
}

function frontmatterConfig(source: string): MermaidConfigObject | null {
  const frontmatter = locateMermaidFrontmatter(source);
  if (!frontmatter) return null;
  const { indent, bodyStart, bodyEnd } = frontmatter;
  const body = source.slice(bodyStart, bodyEnd)
    .split(/\r\n|[\r\n]/)
    .map((line) => indent && line.startsWith(indent) ? line.slice(indent.length) : line)
    .join("\n");
  try {
    const parsed = parseYaml(body, { schema: JSON_SCHEMA }) as unknown;
    return isObject(parsed) && isObject(parsed.config) ? parsed.config : null;
  } catch {
    return null;
  }
}

function* directiveConfigs(source: string): Generator<MermaidConfigObject> {
  const directiveStart = /%%\{\s*(?:init|initialize)\s*:\s*/gi;
  const directiveEnd = /\}\s*%%/g;
  for (let start = directiveStart.exec(source); start; start = directiveStart.exec(source)) {
    directiveEnd.lastIndex = directiveStart.lastIndex;
    const end = directiveEnd.exec(source);
    if (!end) break;
    const body = source.slice(directiveStart.lastIndex, end.index);
    directiveStart.lastIndex = directiveEnd.lastIndex;
    try {
      const config = JSON.parse(body.trim().replaceAll("'", '"')) as unknown;
      if (isObject(config)) yield config;
    } catch {
      return;
    }
  }
}

function isObject(value: unknown): value is MermaidConfigObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
