import type { ResolvedThemeV2, ThemeCatalog, ThemeDocumentV2, ThemePresetCatalogEntry, ThemeResolution } from "@mermanjs/web";

export function pairedPresetFixture(): ThemePresetCatalogEntry {
  return {
    id: "studio", name: "Studio", description: "Reading theme", category: "reading",
    appearances: ["light", "dark"], default_appearance: "light",
    swatches: ["#ffffff", "#f8fafc", "#e2e8f0", "#1d4ed8", "#0f172a", "#bfdbfe"],
  };
}

export function themeCatalogFixture(): ThemeCatalog {
  return {
    schema_version: 2,
    presets: [pairedPresetFixture(), { ...pairedPresetFixture(), id: "paper-cut", name: "Paper Cut", category: "art", appearances: ["light"] }],
    mermaid_themes: ["neo", "neo-dark", "default", "dark", "forest"],
    families: [],
  };
}

export const themeDocumentFixture: ThemeDocumentV2 = {
  schema_version: 2,
  default_appearance: "light",
  appearances: {
    light: { seed: { canvas: "#ffffff", ink: "#000000", accent: "#1d4ed8" } },
    dark: { seed: { canvas: "#111827", ink: "#ffffff", accent: "#bfdbfe" } },
  },
};

export const resolvedThemeFixture: ResolvedThemeV2 = {
  id: null,
  appearance: "light",
  roles: {
    canvas: "#FFFFFF",
    "surface.node": "#FFFFFF", "surface.container": "#FFFFFF", "surface.callout": "#FFFFFF", "surface.emphasis": "#FFFFFF",
    "stroke.border": "#111111", "stroke.connector": "#111111", "stroke.guide": "#CCCCCC",
    "text.primary": "#111111", "text.muted": "#111111", "text.title": "#111111", "text.callout": "#111111", "text.on_emphasis": "#111111",
    "status.info": "#2563EB", "status.success": "#15803D", "status.warn": "#B45309", "status.danger": "#B91C1C",
  },
  data: { ink: [], fill: [], fill_ink: [], pattern_encoding: false },
  form: { density: "balanced", radius: 6, container_radius: 10, stroke_width: 1, connector_width: 1.5,
    edge_style: "curved", marker_style: "stealth", edge_label_bg: "auto",
    chart: { gridlines: true, bar_radius: 4, bar_wireframe: false, line_markers: true, pie_gap_px: 1.5, pie_opacity: 1 },
  },
  typography: { sans: ["Arial", "sans-serif"], serif: ["Georgia", "serif"], mono: ["Consolas", "monospace"],
    body_family: "sans", title_family: "sans", code_family: "mono", base_size_px: 14, title_weight: 600, label_weight: 500 },
  elevation: { kind: "flat" },
  node_ordinal_palette: false,
  canvas: { textures: [], padding_px: 0 },
};

export function themeResolutionFixture(): Extract<ThemeResolution, { ok: true }> {
  return { ok: true, notices: [], resolved: resolvedThemeFixture };
}
