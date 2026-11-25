import { CUSTOM_FILE_THEME, SOURCE_THEME, type ThemeDiscovery } from "../theme-selection.js";

export function v2ThemeDiscovery(): ThemeDiscovery {
  return {
    choices: [
      SOURCE_THEME,
      ...["default", "dark", "forest", "neutral", "base", "neo"].map((id) => ({
        id,
        label: id,
        origin: "mermaid" as const,
      })),
      {
        id: "studio",
        label: "Studio",
        origin: "preset",
        appearances: { supportedAppearances: ["light", "dark"], defaultAppearance: "light" },
      },
      {
        id: "paper-cut",
        label: "Paper Cut",
        origin: "preset",
        appearances: { supportedAppearances: ["light"], defaultAppearance: "light" },
      },
      CUSTOM_FILE_THEME,
    ],
  };
}
