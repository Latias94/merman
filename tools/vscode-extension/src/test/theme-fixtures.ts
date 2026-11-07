import { legacyThemeDiscovery, type ThemeDiscovery } from "../theme-selection.js";

export function pairedThemeDiscovery(): ThemeDiscovery {
  return {
    ...legacyThemeDiscovery(),
    paired: true,
    inspect: true,
    notice: undefined,
    choices: [
      ...legacyThemeDiscovery().choices,
      {
        id: "preset:editor",
        label: "Merman · Editor",
        origin: "preset",
        available: true,
        supportedAppearances: ["light", "dark"],
        defaultAppearance: "light",
      },
      {
        id: "preset:ghibli",
        label: "Merman · Ghibli",
        origin: "preset",
        available: true,
        supportedAppearances: ["light"],
        defaultAppearance: "light",
      },
      {
        id: "mermaid:neo",
        label: "Mermaid · Neo",
        origin: "mermaid",
        available: true,
        supportedAppearances: ["light", "dark"],
        defaultAppearance: "light",
        variants: [
          { appearance: "light", themeId: "neo" },
          { appearance: "dark", themeId: "neo-dark" },
        ],
      },
    ],
  };
}
