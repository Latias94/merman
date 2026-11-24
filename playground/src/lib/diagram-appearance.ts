import type { MermaidThemeSelection } from "./mermaid-theme-name.ts";
import type { ThemeDocumentLabel, WorkspaceSnapshot } from "./workspace-snapshot.ts";
import type { ThemeAppearancePreference } from "./theme-appearance.ts";

export type { ThemeDocumentLabel } from "./workspace-snapshot.ts";

export type DiagramThemeChoice =
  | { readonly kind: "mermaid"; readonly theme: MermaidThemeSelection; readonly appearance?: ThemeAppearancePreference }
  | { readonly kind: "preset"; readonly presetId: string; readonly appearance?: ThemeAppearancePreference }
  | { readonly kind: "document"; readonly documentJson: string; readonly label?: ThemeDocumentLabel | null; readonly appearance?: ThemeAppearancePreference };

const APPEARANCE_KEYS = [
  "diagramTheme", "diagramFont", "themePresetId", "themeDocumentJson", "themeDocumentLabel", "themeAppearance",
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
    themeDocumentJson: workspace.themeDocumentJson,
    themeDocumentLabel: workspace.themeDocumentLabel ?? null,
    themeAppearance: workspace.themeAppearance ?? "default",
  };
}

export function themeChoiceFields(choice: DiagramThemeChoice): Partial<DiagramAppearance> {
  switch (choice.kind) {
    case "mermaid": return { diagramTheme: choice.theme, themePresetId: null, themeDocumentJson: null, themeDocumentLabel: null, themeAppearance: choice.appearance ?? "default" };
    case "preset": return { diagramTheme: "auto", themePresetId: choice.presetId, themeDocumentJson: null, themeDocumentLabel: null, themeAppearance: choice.appearance ?? "default" };
    case "document": return { diagramTheme: "auto", themePresetId: null, themeDocumentJson: choice.documentJson, themeDocumentLabel: choice.label ?? null, themeAppearance: choice.appearance ?? "default" };
  }
}

function sameValue(key: typeof APPEARANCE_KEYS[number], left: DiagramAppearance, right: DiagramAppearance): boolean {
  if (key === "themeDocumentLabel") {
    return left.themeDocumentLabel?.name === right.themeDocumentLabel?.name &&
      left.themeDocumentLabel?.source === right.themeDocumentLabel?.source &&
      left.themeDocumentLabel?.derivedFrom === right.themeDocumentLabel?.derivedFrom;
  }
  if (key === "themeAppearance") return (left[key] ?? "default") === (right[key] ?? "default");
  return left[key] === right[key];
}

export function appearanceChanged(before: DiagramAppearance, after: DiagramAppearance): boolean {
  return APPEARANCE_KEYS.some((key) => !sameValue(key, before, after));
}

/** Undo only the appearance fields this action still owns. */
export function undoAppearanceFields(current: DiagramAppearance, change: AppearanceChange): Partial<DiagramAppearance> {
  let fields: Partial<DiagramAppearance> = {};
  const themeKeys = ["diagramTheme", "themePresetId", "themeDocumentJson", "themeDocumentLabel", "themeAppearance"] as const;
  // Restore the selected theme and its identity together.
  if (themeKeys.every((key) => sameValue(key, current, change.after))) {
    for (const key of themeKeys) {
      if (!sameValue(key, change.before, change.after)) fields = { ...fields, [key]: change.before[key] };
    }
  }
  if (current.diagramFont === change.after.diagramFont && change.before.diagramFont !== change.after.diagramFont) {
    fields = { ...fields, diagramFont: change.before.diagramFont };
  }
  return fields;
}
