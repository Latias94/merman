import type { ThemeAppearance, ThemeCatalog, ThemeRecipeInspectionV1 } from "@mermanjs/web";
import type { MermanDomainFacade } from "../runtime/merman-core.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";
import type { MermaidThemeSelection } from "./mermaid-theme-name.ts";

export type ThemeAppearancePreference = "default" | "follow" | ThemeAppearance;

export interface ThemeAppearanceSupport {
  readonly supported_appearances: readonly ThemeAppearance[];
  readonly default_appearance: ThemeAppearance | null;
}

export interface ResolvedThemeSelection extends ThemeAppearanceSupport {
  readonly preference: ThemeAppearancePreference;
  readonly appearance: ThemeAppearance | null;
  readonly mermaidTheme: MermaidThemeSelection;
  readonly followsApp: boolean;
  readonly error?: string;
}

const UNCLASSIFIED: ThemeAppearanceSupport = Object.freeze({
  supported_appearances: Object.freeze([]),
  default_appearance: null,
});

export function isThemeAppearancePreference(value: unknown): value is ThemeAppearancePreference {
  return value === "default" || value === "follow" || value === "light" || value === "dark";
}

/** The runtime validates the original bytes, including duplicate fields and inactive branches. */
export function inspectThemeRecipe(
  facade: Pick<MermanDomainFacade, "materializeTheme">,
  raw: string,
): ThemeRecipeInspectionV1 {
  JSON.parse(raw);
  const result = facade.materializeTheme(`{"schema_version":1,"action":"inspect","recipe":${raw}}`);
  if (result.schema_version !== 1 || !("supported_appearances" in result) || !("appearances" in result)) {
    throw new Error("The matching Merman runtime must support complete theme inspection.");
  }
  return result;
}

export function selectedPresetCatalogEntry(catalog: ThemeCatalog | null | undefined, id: string | null) {
  if (!catalog || !id) return null;
  const preset = catalog.presets.find((entry) => entry.id === id ||
    entry.appearances.some((appearance) => appearance.concrete_preset_id === id));
  const concrete = preset?.appearances.find((entry) => entry.concrete_preset_id === id);
  return preset ? { preset, pinned: concrete?.appearance ?? null, available: preset.available } : null;
}

export function exportSelectedThemeRecipe(
  facade: Pick<MermanDomainFacade, "exportThemePreset">,
  id: string,
): string {
  return JSON.stringify(facade.exportThemePreset(id), null, 2);
}

export function themeAppearanceSupport(
  workspace: Readonly<WorkspaceSnapshot>,
  catalog: ThemeCatalog | null | undefined,
  recipe?: ThemeAppearanceSupport | null,
): ThemeAppearanceSupport {
  if (workspace.themeRecipeJson !== null) return recipe ?? UNCLASSIFIED;
  if (workspace.themePresetId !== null) {
    const selected = selectedPresetCatalogEntry(catalog, workspace.themePresetId);
    if (!selected) return UNCLASSIFIED;
    return selected.pinned
      ? { supported_appearances: [selected.pinned], default_appearance: selected.pinned }
      : selected.preset;
  }
  const design = catalog?.mermaid_theme_designs.find((entry) =>
    entry.appearances.some((appearance) => appearance.theme_id === workspace.diagramTheme));
  if (!design) return UNCLASSIFIED;
  return {
    supported_appearances: design.appearances.map((entry) => entry.appearance),
    default_appearance: design.appearances.find((entry) => entry.theme_id === workspace.diagramTheme)?.appearance ??
      design.default_appearance,
  };
}

export function resolveThemeSelection(
  workspace: Readonly<WorkspaceSnapshot>,
  hostAppearance: ThemeAppearance,
  catalog?: ThemeCatalog | null,
  recipe?: ThemeAppearanceSupport | null,
): ResolvedThemeSelection {
  const support = themeAppearanceSupport(workspace, catalog, recipe);
  const preference = workspace.themeAppearance ?? "default";
  const native = workspace.themePresetId === null && workspace.themeRecipeJson === null;
  const supportKnown = native || (workspace.themeRecipeJson !== null ? recipe != null
    : selectedPresetCatalogEntry(catalog, workspace.themePresetId) !== null);
  const unsupported = (preference === "light" || preference === "dark") &&
    supportKnown && !support.supported_appearances.includes(preference);
  const name = native ? `Mermaid theme "${workspace.diagramTheme}"`
    : workspace.themePresetId ? `Theme preset "${workspace.themePresetId}"`
    : workspace.themeRecipeLabel?.name ?? "Imported theme";
  const error = unsupported
    ? `${name} does not support the "${preference}" appearance. Supported appearances: ${support.supported_appearances.join(", ") || "none declared"}.`
    : undefined;
  const appearance = error ? null : preference === "default" ? support.default_appearance
    : preference === "follow"
      ? support.supported_appearances.includes(hostAppearance) ? hostAppearance : support.default_appearance
      : preference;
  let mermaidTheme = workspace.diagramTheme;
  if (workspace.themePresetId === null && workspace.themeRecipeJson === null && appearance !== null &&
    preference !== "default") {
    const design = catalog?.mermaid_theme_designs.find((entry) =>
      entry.appearances.some((entry) => entry.theme_id === workspace.diagramTheme));
    mermaidTheme = design?.appearances.find((entry) => entry.appearance === appearance)?.theme_id ??
      workspace.diagramTheme;
  }
  return Object.freeze({
    ...support,
    preference,
    appearance,
    mermaidTheme,
    followsApp: preference === "follow",
    ...(error ? { error } : {}),
  });
}

export function preferredAppearanceForSelection(support: ThemeAppearanceSupport): ThemeAppearancePreference {
  return support.supported_appearances.length > 1 ? "follow" : support.default_appearance ?? "default";
}

export function compatibleAppearancePreference(
  preference: ThemeAppearancePreference,
  support: ThemeAppearanceSupport,
): ThemeAppearancePreference | null {
  if (preference === "default" || preference === "follow") return preference;
  return support.supported_appearances.includes(preference) ? preference : null;
}
