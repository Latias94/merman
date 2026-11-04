import type { ThemeName } from "./mermaid-theme-name.ts";

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

export function resolveMermaidCanvasTone(theme: unknown): MermaidCanvasTone {
  return typeof theme === "string" && Object.hasOwn(MERMAID_CANVAS_TONES, theme)
    ? MERMAID_CANVAS_TONES[theme as ThemeName]
    : "light";
}
