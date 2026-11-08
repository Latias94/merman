import type { ThemeCatalog, ThemePresetCatalogEntry, ThemeRecipeInspectionV1 } from "@mermanjs/web";

export function pairedPresetFixture(): ThemePresetCatalogEntry {
  return {
    id: "editor", display_name: "Editor", supported_appearances: ["light", "dark"], default_appearance: "light",
    available: true, availability_reason_ids: [], maturity: "alpha", license_expression: "MIT",
    required_attribution: null, export_kind: "design",
    appearances: (["light", "dark"] as const).map((appearance) => ({
      appearance, concrete_preset_id: `editor-${appearance}`, available: true,
      availability_reason_ids: [], preview_colors: [], family_designs: [], application_notes: [], qualified_cells: [],
    })),
  };
}

export function themeCatalogFixture(): ThemeCatalog {
  return {
    schema_version: 1, structured_spec_available: true, recipe_schema_version: 1,
    authoring_request_schema_version: 1, authoring_result_schema_version: 1,
    supported_output_ids: ["svg"], presets: [pairedPresetFixture()],
    mermaid_theme_designs: [
      { id: "neo", display_name: "Neo", default_appearance: "light",
        appearances: [{ appearance: "light", theme_id: "neo" }, { appearance: "dark", theme_id: "neo-dark" }] },
      { id: "default", display_name: "Default", default_appearance: "light", appearances: [{ appearance: "light", theme_id: "default" }] },
      { id: "dark", display_name: "Dark", default_appearance: "dark", appearances: [{ appearance: "dark", theme_id: "dark" }] },
      { id: "forest", display_name: "Forest", default_appearance: "light", appearances: [{ appearance: "light", theme_id: "forest" }] },
    ],
    known_capability_ids: [], known_text_capability_ids: [], known_font_container_ids: [], known_font_source_ids: [],
    known_semantic_target_ids: [], known_variant_ids: [], resource_limits: [],
  };
}

export function themeInspectionFixture(): ThemeRecipeInspectionV1 {
  return {
    schema_version: 1, recipe_schema_version: 1, supported_appearances: ["light", "dark"], default_appearance: "light",
    appearances: [{ appearance: "light", references: [] }, { appearance: "dark", references: [] }],
  };
}
