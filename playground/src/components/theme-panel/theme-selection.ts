import type { WorkspaceSnapshot } from "../../lib/workspace-snapshot.ts";
import type { DiagramThemeChoice, ThemeRecipeLabel } from "../../lib/diagram-appearance.ts";
import { selectedPresetCatalogEntry, type ThemeAppearancePreference } from "../../lib/theme-appearance.ts";
import type { ExplorationTheme } from "../../themes/catalog.ts";
import { utf8ByteLength } from "../../lib/utf8.ts";
import type { ThemeCatalog } from "@mermanjs/web";

interface ExplorationIdentity {
  readonly id: string;
  readonly presetId?: string;
  readonly recipeJson: string;
}

/** A registered preset owns the choice even when runtime policy makes it unavailable. */
export function galleryExplorations<T extends ExplorationIdentity>(
  explorations: readonly T[],
  catalog?: ThemeCatalog | null,
): readonly T[] {
  return explorations.filter((theme) => !selectedPresetCatalogEntry(catalog, theme.presetId ?? null));
}

export function explorationThemeChoice(
  theme: Pick<ExplorationTheme, "presetId" | "name" | "recipeJson" | "appearances">,
  catalog: ThemeCatalog | null,
  preference: ThemeAppearancePreference,
): DiagramThemeChoice | null {
  // A failed or pending catalog cannot authorize a mapped recipe fallback.
  if (theme.presetId && !catalog) return null;
  const preset = selectedPresetCatalogEntry(catalog, theme.presetId ?? null);
  const appearance = theme.appearances ? preference : "default";
  if (preset && theme.presetId) return preset.available ? { kind: "preset", presetId: theme.presetId, appearance } : null;
  return { kind: "recipe", recipeJson: theme.recipeJson, appearance,
    label: { name: theme.name, source: "exploration" } };
}

export function selectedExplorationTheme<T extends ExplorationIdentity>(
  workspace: Readonly<WorkspaceSnapshot>,
  explorations: readonly T[],
  catalog?: ThemeCatalog | null,
): T | null {
  if (workspace.themePresetId !== null) {
    const selectedId = selectedPresetCatalogEntry(catalog, workspace.themePresetId)?.preset.id ?? workspace.themePresetId;
    return explorations.find((theme) => theme.presetId &&
      (selectedPresetCatalogEntry(catalog, theme.presetId)?.preset.id ?? theme.presetId) === selectedId) ?? null;
  }
  const recipe = currentRecipeIdentity(workspace, explorations, "");
  return explorations.find((theme) => theme.id === recipe?.explorationId) ?? null;
}

export function currentRecipeIdentity(
  workspace: Readonly<WorkspaceSnapshot>,
  explorations: readonly ExplorationIdentity[],
  fallbackName: string,
): (ThemeRecipeLabel & { readonly explorationId: string | null; readonly recipeJson: string }) | null {
  if (workspace.themeRecipeJson === null) return null;
  const label = workspace.themeRecipeLabel;
  const exploration = label?.source === "exploration"
    ? explorations.find((theme) => theme.recipeJson === workspace.themeRecipeJson) ?? null : null;
  return {
    name: label?.name ?? fallbackName,
    source: label?.source ?? "imported",
    ...(label?.derivedFrom ? { derivedFrom: label.derivedFrom } : {}),
    explorationId: exploration?.id ?? null,
    recipeJson: workspace.themeRecipeJson,
  };
}

export function selectedThemeKey(
  workspace: Readonly<WorkspaceSnapshot>,
  explorations: readonly ExplorationIdentity[],
  catalog?: ThemeCatalog | null,
): string {
  if (workspace.themePresetId !== null) {
    const canonicalId = catalog?.presets.find((entry) =>
      entry.appearances.some((appearance) => appearance.concrete_preset_id === workspace.themePresetId))?.id;
    return `preset:${canonicalId ?? workspace.themePresetId}`;
  }
  const recipe = currentRecipeIdentity(workspace, galleryExplorations(explorations, catalog), "");
  if (recipe) return recipe.explorationId ? `exploration:${recipe.explorationId}` : "recipe:current";
  const design = catalog?.mermaid_theme_designs.find((entry) =>
    entry.appearances.some((entry) => entry.theme_id === workspace.diagramTheme));
  return `mermaid:${design?.id ?? workspace.diagramTheme}`;
}

export function importedThemeName(fileName: string, fallbackName = "Imported theme"): string {
  const name = fileName.replace(/(?:\.theme)?\.json$/i, "").trim() || fileName.trim() || fallbackName;
  let bounded = "";
  for (const character of name) {
    if (utf8ByteLength(bounded + character) > 256) break;
    bounded += character;
  }
  return bounded;
}
