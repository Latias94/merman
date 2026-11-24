export type ThemeAppearance = "light" | "dark";
export type ThemeAppearancePreference = "follow-editor" | ThemeAppearance;
export type DiagramTheme = string;

export interface ThemeAppearances {
  supportedAppearances: ThemeAppearance[];
  defaultAppearance: ThemeAppearance;
}

export interface ThemeChoice {
  id: DiagramTheme;
  label: string;
  origin: "source" | "mermaid" | "preset" | "file";
  appearances?: ThemeAppearances;
}

export interface ThemeDiscovery {
  choices: ThemeChoice[];
  notice?: string;
}

export interface ImportedTheme extends ThemeAppearances {
  content: string;
  label: string;
}

export interface ResolvedTheme {
  effectiveAppearance?: ThemeAppearance;
  theme?: string;
  themeFileContent?: string;
  label: string;
  supportedAppearances: ThemeAppearance[];
  notice?: string;
}

export const SOURCE_THEME: ThemeChoice = { id: "source", label: "Source / default", origin: "source" };
export const CUSTOM_FILE_THEME: ThemeChoice = { id: "file", label: "Custom File", origin: "file" };

export function isThemeAppearance(value: unknown): value is ThemeAppearance {
  return value === "light" || value === "dark";
}

export function isThemePreference(value: unknown): value is ThemeAppearancePreference {
  return value === "follow-editor" || isThemeAppearance(value);
}

export function isDiagramTheme(value: unknown): value is DiagramTheme {
  return typeof value === "string" && /^[a-z][a-z0-9-]{0,79}$/.test(value);
}

export function editorThemeAppearance(kind: number): ThemeAppearance {
  return kind === 2 || kind === 3 ? "dark" : "light";
}

export function resolveThemeSelection(
  discovery: ThemeDiscovery,
  requested: DiagramTheme,
  preference: ThemeAppearancePreference,
  host: ThemeAppearance,
  imported?: ImportedTheme,
): ResolvedTheme {
  const choice = discovery.choices.find((entry) => entry.id === requested);
  if (!choice) {
    throw new Error(`Theme "${requested}" is unavailable in the CLI catalog. Choose another theme in Preview or update the CLI.`);
  }
  const file = choice.origin === "file" ? imported : undefined;
  if (choice.origin === "file" && !file) {
    throw new Error("Choose a file with merman.preview.themeFile or import a theme in the preview.");
  }
  const appearances = file ?? choice.appearances;
  const desired = preference === "follow-editor" ? host : preference;
  const effectiveAppearance = appearances?.supportedAppearances.includes(desired)
    ? desired
    : appearances?.defaultAppearance;
  return {
    effectiveAppearance,
    theme: choice.origin === "mermaid" || choice.origin === "preset" ? choice.id : undefined,
    themeFileContent: file?.content,
    label: file?.label ?? choice.label,
    supportedAppearances: appearances?.supportedAppearances ?? [],
    notice: effectiveAppearance && effectiveAppearance !== desired
      ? `This theme does not provide ${desired}. Using ${effectiveAppearance}.`
      : undefined,
  };
}
