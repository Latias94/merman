import type { ThemeAppearance, ThemeDocumentV2 } from "@mermanjs/web";

export const THEME_APPEARANCES = ["light", "dark"] as const;
export const THEME_DOCUMENT_BYTE_LIMIT = 256 * 1024;

export function isThemeAppearance(value: unknown): value is ThemeAppearance {
  return value === "light" || value === "dark";
}

export function parseThemeDocument(raw: string): ThemeDocumentV2 {
  const value: unknown = JSON.parse(raw);
  if (!value || typeof value !== "object" || !("schema_version" in value) || value.schema_version !== 2) {
    throw new Error("Expected a theme document with schema_version 2.");
  }
  return value as ThemeDocumentV2;
}

export function formatThemeDocument(document: ThemeDocumentV2): string {
  return `${JSON.stringify(document, null, 2)}\n`;
}
