import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";

/** Source/configuration and host chrome are outside the theme-edit transaction. */
export function sameThemeEditOrigin(left: Readonly<WorkspaceSnapshot>, right: Readonly<WorkspaceSnapshot>): boolean {
  return left.diagramTheme === right.diagramTheme && left.themePresetId === right.themePresetId &&
    left.themeRecipeJson === right.themeRecipeJson && (left.themeAppearance ?? "default") === (right.themeAppearance ?? "default") &&
    left.themeRecipeLabel?.name === right.themeRecipeLabel?.name &&
    left.themeRecipeLabel?.source === right.themeRecipeLabel?.source &&
    left.themeRecipeLabel?.derivedFrom === right.themeRecipeLabel?.derivedFrom;
}
