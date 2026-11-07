import type { WorkspaceSnapshot } from "../../lib/workspace-snapshot.ts";
import type { ThemeRecipeLabel } from "../../lib/diagram-appearance.ts";
import { utf8ByteLength } from "../../lib/utf8.ts";
import type { ThemeCatalog } from "@mermanjs/web";

interface ExplorationIdentity {
  readonly id: string;
  readonly recipeJson: string;
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
  const recipe = currentRecipeIdentity(workspace, explorations, "");
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
