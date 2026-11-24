import type { ThemeAppearance, ThemeCatalog } from "@mermanjs/web";
import { isThemeAppearance, parseThemeDocument, THEME_APPEARANCES } from "../themes/v2.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";
import { resolveMermaidCanvasTone } from "./mermaid-canvas-tone.ts";
import type { MermaidThemeSelection, ThemeName } from "./mermaid-theme-name.ts";

export type ThemeAppearancePreference = "default" | "follow" | ThemeAppearance;

export interface ResolvedThemeSelection {
  readonly supported_appearances: readonly ThemeAppearance[];
  readonly default_appearance: ThemeAppearance | null;
  readonly preference: ThemeAppearancePreference;
  readonly appearance: ThemeAppearance | null;
  readonly mermaidTheme: MermaidThemeSelection;
  readonly error?: string;
}

const MERMAID_PAIRS = [
  ["neo", "neo-dark"],
  ["redux", "redux-dark"],
  ["redux-color", "redux-dark-color"],
] as const satisfies readonly (readonly [ThemeName, ThemeName])[];

export function isThemeAppearancePreference(value: unknown): value is ThemeAppearancePreference {
  return value === "default" || value === "follow" || isThemeAppearance(value);
}

export function resolveThemeSelection(
  workspace: Pick<WorkspaceSnapshot, "diagramTheme" | "themePresetId" | "themeDocumentJson" | "themeAppearance">,
  hostAppearance: ThemeAppearance,
  catalog?: ThemeCatalog | null,
): ResolvedThemeSelection {
  const preference = workspace.themeAppearance ?? "default";
  const selected = workspace.themePresetId?.trim();
  let mermaidTheme = workspace.diagramTheme;
  let supported: readonly ThemeAppearance[] = [];
  let defaultAppearance: ThemeAppearance | null = null;
  let error: string | undefined;

  if (workspace.themeDocumentJson !== null) {
    try {
      const document = parseThemeDocument(workspace.themeDocumentJson);
      const base = catalog?.presets.find(({ id }) => id === document.extends);
      supported = THEME_APPEARANCES.filter((mode) => document.appearances?.[mode] || base?.appearances.includes(mode));
      defaultAppearance = isThemeAppearance(document.default_appearance) ? document.default_appearance
        : base?.default_appearance ?? (supported.length === 1 ? supported[0] : null);
    } catch { /* The binding reports invalid documents with their input locations. */ }
  } else if (selected) {
    const preset = catalog?.presets.find(({ id }) => id === selected);
    supported = preset?.appearances ?? [];
    defaultAppearance = preset?.default_appearance ?? null;
  } else if (mermaidTheme !== "auto") {
    const pair = MERMAID_PAIRS.find(([light, dark]) => mermaidTheme === light || mermaidTheme === dark);
    defaultAppearance = resolveMermaidCanvasTone(mermaidTheme);
    supported = pair ? THEME_APPEARANCES : [defaultAppearance];
    if (pair && preference !== "default") {
      mermaidTheme = (preference === "follow" ? hostAppearance : preference) === "dark" ? pair[1] : pair[0];
    }
    if (isThemeAppearance(preference) && !supported.includes(preference)) {
      error = `Mermaid theme "${mermaidTheme}" does not support the "${preference}" appearance.`;
    }
  }

  const appearance = error ? null : preference === "default" ? defaultAppearance : preference === "follow"
    ? supported.includes(hostAppearance) ? hostAppearance : defaultAppearance
    : preference;
  return {
    supported_appearances: supported,
    default_appearance: defaultAppearance,
    preference,
    appearance,
    mermaidTheme,
    ...(error ? { error } : {}),
  };
}
