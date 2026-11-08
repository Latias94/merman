import type { MermaidThemeSelection } from "./mermaid-theme-name.ts";
import type { ThemeRecipeLabel, WorkspaceSnapshot } from "./workspace-snapshot.ts";
import type { ThemeAppearancePreference } from "./theme-appearance.ts";

export type { ThemeRecipeLabel } from "./workspace-snapshot.ts";

export type DiagramThemeChoice =
  | { readonly kind: "mermaid"; readonly theme: MermaidThemeSelection; readonly appearance?: ThemeAppearancePreference }
  | { readonly kind: "preset"; readonly presetId: string; readonly appearance?: ThemeAppearancePreference; readonly compose?: boolean }
  | { readonly kind: "recipe"; readonly recipeJson: string; readonly label?: ThemeRecipeLabel | null; readonly appearance?: ThemeAppearancePreference; readonly compose?: boolean }
  | { readonly kind: "none" };

const APPEARANCE_KEYS = [
  "diagramTheme", "diagramFont", "themePresetId", "themeRecipeJson", "themeRecipeLabel", "themeAppearance", "mermaidConfig",
] as const;

export type DiagramAppearance = Pick<WorkspaceSnapshot, typeof APPEARANCE_KEYS[number]>;

export interface AppearanceChange {
  readonly before: DiagramAppearance;
  readonly after: DiagramAppearance;
}

export function selectDiagramAppearance(workspace: WorkspaceSnapshot): DiagramAppearance {
  return {
    diagramTheme: workspace.diagramTheme,
    diagramFont: workspace.diagramFont,
    themePresetId: workspace.themePresetId,
    themeRecipeJson: workspace.themeRecipeJson,
    themeRecipeLabel: workspace.themeRecipeLabel ?? null,
    themeAppearance: workspace.themeAppearance ?? "default",
    mermaidConfig: workspace.mermaidConfig,
  };
}

export function themeChoiceFields(choice: DiagramThemeChoice): Partial<DiagramAppearance> {
  switch (choice.kind) {
    case "mermaid": return { diagramTheme: choice.theme, themePresetId: null, themeRecipeJson: null, themeRecipeLabel: null, themeAppearance: choice.appearance ?? "default" };
    case "preset": return { ...(choice.compose ? {} : { diagramTheme: "auto" }), themePresetId: choice.presetId, themeRecipeJson: null, themeRecipeLabel: null, themeAppearance: choice.appearance ?? "default" };
    case "recipe": return { ...(choice.compose ? {} : { diagramTheme: "auto" }), themePresetId: null, themeRecipeJson: choice.recipeJson, themeRecipeLabel: choice.label ?? null, themeAppearance: choice.appearance ?? "default" };
    case "none": return { themePresetId: null, themeRecipeJson: null, themeRecipeLabel: null, themeAppearance: "default" };
  }
}

function sameValue(key: typeof APPEARANCE_KEYS[number], left: DiagramAppearance, right: DiagramAppearance): boolean {
  if (key === "themeRecipeLabel") {
    return left.themeRecipeLabel?.name === right.themeRecipeLabel?.name &&
      left.themeRecipeLabel?.source === right.themeRecipeLabel?.source &&
      left.themeRecipeLabel?.derivedFrom === right.themeRecipeLabel?.derivedFrom;
  }
  if (key === "themeAppearance") return (left[key] ?? "default") === (right[key] ?? "default");
  return left[key] === right[key];
}

export function appearanceChanged(before: DiagramAppearance, after: DiagramAppearance): boolean {
  return APPEARANCE_KEYS.some((key) => !sameValue(key, before, after));
}

/** Undo only the fields this action still owns; later source/config edits remain authored. */
export function undoAppearanceFields(current: DiagramAppearance, change: AppearanceChange): Partial<DiagramAppearance> {
  let fields: Partial<DiagramAppearance> = {};
  const themeKeys = ["diagramTheme", "themePresetId", "themeRecipeJson", "themeRecipeLabel", "themeAppearance"] as const;
  // A theme selection is atomic: never restore a recipe without its matching identity.
  if (themeKeys.every((key) => sameValue(key, current, change.after))) {
    for (const key of themeKeys) {
      if (!sameValue(key, change.before, change.after)) fields = { ...fields, [key]: change.before[key] };
    }
  }
  for (const key of ["diagramFont", "mermaidConfig"] as const) {
    if (!sameValue(key, change.before, change.after) && sameValue(key, current, change.after)) {
      fields = { ...fields, [key]: change.before[key] };
    }
  }
  return fields;
}
