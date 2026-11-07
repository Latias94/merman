export type ThemeAppearance = "light" | "dark";
export type ThemeAppearancePreference = "follow-editor" | ThemeAppearance;
export type DiagramTheme =
  | "source"
  | "default"
  | "dark"
  | "forest"
  | "neutral"
  | "base"
  | "neo"
  | "neo-dark"
  | "redux"
  | "redux-dark"
  | "redux-color"
  | "redux-dark-color"
  | `mermaid:${string}`
  | `preset:${string}`
  | "file";

export interface ThemeChoice {
  id: DiagramTheme;
  label: string;
  origin: "source" | "mermaid" | "preset" | "file";
  supportedAppearances: ThemeAppearance[];
  defaultAppearance?: ThemeAppearance;
  available: boolean;
  variants?: Array<{ appearance: ThemeAppearance; themeId: string }>;
  fixedAppearance?: ThemeAppearance;
}

export interface ThemeDiscovery {
  choices: ThemeChoice[];
  legacyChoices: ThemeChoice[];
  paired: boolean;
  inspect: boolean;
  notice?: string;
}

export interface ImportedTheme {
  content: string;
  label: string;
  supportedAppearances: ThemeAppearance[];
  defaultAppearance?: ThemeAppearance;
}

export interface ResolvedTheme {
  requested: DiagramTheme;
  preference: ThemeAppearancePreference;
  effectiveAppearance?: ThemeAppearance;
  theme?: string;
  themePreset?: string;
  themeFileContent?: string;
  label: string;
  supportedAppearances: ThemeAppearance[];
  notice?: string;
}

export const SOURCE_THEME: ThemeChoice = {
  id: "source",
  label: "Source / default",
  origin: "source",
  supportedAppearances: [],
  available: true,
};

export function legacyThemeDiscovery(notice?: string): ThemeDiscovery {
  return {
    choices: [
      SOURCE_THEME,
      ...(["default", "dark", "forest", "neutral", "base"] as const).map((id) => ({
        id,
        label: `Mermaid · ${id[0]!.toUpperCase()}${id.slice(1)}`,
        origin: "mermaid" as const,
        supportedAppearances: [],
        available: true,
      })),
    ],
    legacyChoices: [],
    paired: false,
    inspect: false,
    notice: notice ?? "This CLI does not expose paired theme discovery. Source keeps its original meaning; update the CLI to select paired themes or import a theme.",
  };
}

export function isThemeAppearance(value: unknown): value is ThemeAppearance {
  return value === "light" || value === "dark";
}

export function isThemePreference(value: unknown): value is ThemeAppearancePreference {
  return value === "follow-editor" || isThemeAppearance(value);
}

export function isDiagramTheme(value: unknown): value is DiagramTheme {
  return typeof value === "string" && (
    value === "source" || value === "file" ||
    ["default", "dark", "forest", "neutral", "base", "neo", "neo-dark", "redux", "redux-dark", "redux-color", "redux-dark-color"].includes(value) ||
    /^(?:mermaid|preset):[a-z][a-z0-9-]{0,79}$/.test(value)
  );
}

export function editorThemeAppearance(kind: number): ThemeAppearance {
  return kind === 2 || kind === 3 ? "dark" : "light";
}

export function themeChoice(
  discovery: ThemeDiscovery,
  id: DiagramTheme,
  imported?: ImportedTheme,
): ThemeChoice | undefined {
  if (id === "file" && imported) {
    return {
      id,
      label: imported.label,
      origin: "file",
      available: true,
      supportedAppearances: imported.supportedAppearances,
      defaultAppearance: imported.defaultAppearance,
    };
  }
  return discovery.choices.find((choice) => choice.id === id) ??
    discovery.legacyChoices.find((choice) => choice.id === id);
}

export function preferenceForTheme(
  choice: ThemeChoice | undefined,
  preference: ThemeAppearancePreference,
): ThemeAppearancePreference {
  if (choice?.supportedAppearances.length === 1) {
    return preference === "follow-editor" ? preference : choice.defaultAppearance ?? "follow-editor";
  }
  return preference;
}

export function resolveThemeSelection(
  discovery: ThemeDiscovery,
  requested: DiagramTheme,
  preference: ThemeAppearancePreference,
  host: ThemeAppearance,
  imported?: ImportedTheme,
): ResolvedTheme {
  const choice = themeChoice(discovery, requested, imported);
  if (!choice || !choice.available) {
    throw new Error(`Theme "${requested}" is unavailable in this CLI. Choose Source / default or update the CLI.`);
  }
  const desired = preference === "follow-editor" ? host : preference;
  const effectiveAppearance = choice.fixedAppearance ??
    (choice.supportedAppearances.includes(desired) ? desired : choice.defaultAppearance);
  if (
    choice.supportedAppearances.length > 0 &&
    preference !== "follow-editor" &&
    !(choice.origin === "mermaid" && choice.fixedAppearance) &&
    effectiveAppearance !== preference
  ) {
    throw new Error(`${choice.label} does not provide a ${preference} appearance.`);
  }
  const result: ResolvedTheme = {
    requested,
    preference,
    effectiveAppearance,
    label: choice.label,
    supportedAppearances: [...choice.supportedAppearances],
  };
  if (preference === "follow-editor" && effectiveAppearance && effectiveAppearance !== host) {
    result.notice = `Following editor · ${effectiveAppearance}. This theme does not provide ${host}.`;
  }
  if (choice.origin === "mermaid") {
    result.theme = choice.variants?.find((variant) => variant.appearance === effectiveAppearance)?.themeId ??
      requested.replace(/^mermaid:/, "");
  } else if (choice.origin === "preset") {
    result.themePreset = requested.slice("preset:".length);
  } else if (choice.origin === "file") {
    result.themeFileContent = imported!.content;
  }
  return result;
}

export function sameResolvedTheme(left?: ResolvedTheme, right?: ResolvedTheme): boolean {
  return left?.theme === right?.theme &&
    left?.themePreset === right?.themePreset &&
    left?.themeFileContent === right?.themeFileContent &&
    left?.effectiveAppearance === right?.effectiveAppearance;
}

export function importedThemeInspection(
  content: string,
  label: string,
  value: unknown,
): ImportedTheme {
  const inspection = object(value);
  if (
    inspection.schema_version !== 1 ||
    !Array.isArray(inspection.supported_appearances) ||
    !inspection.supported_appearances.every(isThemeAppearance) ||
    !(inspection.default_appearance === null || isThemeAppearance(inspection.default_appearance)) ||
    !Array.isArray(inspection.appearances)
  ) {
    throw new Error("The CLI returned an unsupported theme inspection. Update the CLI and extension together.");
  }
  const supportedAppearances = inspection.supported_appearances as ThemeAppearance[];
  const inspectedAppearances = inspection.appearances.map((item: unknown) => object(item).appearance);
  if (
    new Set(supportedAppearances).size !== supportedAppearances.length ||
    (supportedAppearances.length > 0 && !supportedAppearances.includes(inspection.default_appearance as ThemeAppearance)) ||
    (supportedAppearances.length === 0 && inspection.default_appearance !== null) ||
    (supportedAppearances.length === 0
      ? inspectedAppearances.length !== 1 || inspectedAppearances[0] !== null
      : inspectedAppearances.length !== supportedAppearances.length ||
        supportedAppearances.some((appearance) => inspectedAppearances.filter((item: unknown) => item === appearance).length !== 1))
  ) {
    throw new Error("The CLI returned inconsistent theme appearance metadata.");
  }
  return {
    content,
    label,
    supportedAppearances,
    defaultAppearance: inspection.default_appearance ?? undefined,
  };
}

export function object(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}
