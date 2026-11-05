import type { WorkspaceSnapshot } from "../../lib/workspace-snapshot.ts";
import type { ThemeRecipeLabel } from "../../lib/diagram-appearance.ts";
import { utf8ByteLength } from "../../lib/utf8.ts";

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
    explorationId: exploration?.id ?? null,
    recipeJson: workspace.themeRecipeJson,
  };
}

export function selectedThemeKey(
  workspace: Readonly<WorkspaceSnapshot>,
  explorations: readonly ExplorationIdentity[],
): string {
  if (workspace.themePresetId !== null) return `preset:${workspace.themePresetId}`;
  const recipe = currentRecipeIdentity(workspace, explorations, "");
  if (recipe) return recipe.explorationId ? `exploration:${recipe.explorationId}` : "recipe:current";
  return `mermaid:${workspace.diagramTheme}`;
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
