import type { ThemeName } from "@mermanjs/web";

const MERMAID_THEME_NAMES = {
  default: true,
  base: true,
  dark: true,
  forest: true,
  neutral: true,
  neo: true,
  "neo-dark": true,
  redux: true,
  "redux-dark": true,
  "redux-color": true,
  "redux-dark-color": true,
} as const satisfies Record<ThemeName, true>;

export type { ThemeName };

export function normalizeMermaidThemeName(
  theme: string | null | undefined,
): ThemeName {
  return typeof theme === "string" && Object.hasOwn(MERMAID_THEME_NAMES, theme)
    ? (theme as ThemeName)
    : "default";
}

export type MermaidThemeSelection = "auto" | ThemeName;

export function isMermaidThemeSelection(
  value: unknown,
): value is MermaidThemeSelection {
  return (
    value === "auto" ||
    (typeof value === "string" && Object.hasOwn(MERMAID_THEME_NAMES, value))
  );
}

export function normalizeMermaidThemeSelection(
  value: string,
): MermaidThemeSelection {
  return isMermaidThemeSelection(value) ? value : "auto";
}
