use merman_theme_contract::{
    DiagramThemeSpecWireV1, MermaidThemeCompatibilityWireV1, SpecifiedWireV1,
    ThemeCanvasPaintWireV1, ThemeColorTokenV1, ThemeDefinitionV1, ThemeMaterializationErrorV1,
    ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1, ThemeTokensV1,
};

use super::{
    ThemePreset, ThemePresetDescriptor, ThemePresetQualificationInvalidation,
    ThemePresetQualifiedCell,
};
use crate::diagram_theme::ThemeResourcePolicy;
use crate::diagram_theme::definition_admission::materialize_theme_with_resource_policy;

const CATALOG_SCHEMA_VERSION: u32 = 1;
const AUTHORING_SCHEMA_VERSION: u32 = 1;
const EXPANSION_VERSION: u32 = 1;
const SPEC_SCHEMA_VERSION: u32 = 1;
const ALPHA: &str = "alpha";
const PROJECT_LICENSE: &str = "MIT OR Apache-2.0";
const NO_QUALIFIED_CELLS: &[ThemePresetQualifiedCell] = &[];
const NO_IDS: &[&str] = &[];
const DEFAULT_RESOURCE_FINGERPRINT: &str =
    "c6aa7af73322aac35ce4548369140848c9a4d093700abccb063ea47a1797d0aa";
const EDITOR_LIGHT_RECIPE_V1_FINGERPRINT: &str =
    "86d97c5535b6e6e18464f3021cfb3be32a2b67da626f4ce8156fa6226b622072";
const EDITOR_DARK_RECIPE_V1_FINGERPRINT: &str =
    "cce6fc1bb059dd0b4d4bc40f8059fab979b8e490dbc3a9b1f52d09a6ade3a48e";
const ONE_DARK_RECIPE_V1_FINGERPRINT: &str =
    "d13df7df39aa540ee4afe11402956a5ec49a3de15fe09e688294170f68b641b4";
const GRUVBOX_LIGHT_RECIPE_V1_FINGERPRINT: &str =
    "3937cd427248164757d6a91709c7a375acfaa058139477cadf3a37060e6b5eef";
const GRUVBOX_DARK_RECIPE_V1_FINGERPRINT: &str =
    "33cdb285bd8524ca3dc39439036c819d4f6d36c95a3443580e552ae30d2c7046";
const AYU_LIGHT_RECIPE_V1_FINGERPRINT: &str =
    "c2959559b5ee97d1d388fdb012e2295941bfc0064d273830ddc4d79adec7a8e0";
const AYU_DARK_RECIPE_V1_FINGERPRINT: &str =
    "8d8fd3d8188071d43b37fe0e1654a45dd830696eb15e1d69f0552fa3a0324671";
const BRUTALIST_RECIPE_V1_FINGERPRINT: &str =
    "d1d03d91fe664073afb7ab8fda1d7a02779c1cbefd1ae90496a3c939132a635a";
const SPOTLESS_RECIPE_V1_FINGERPRINT: &str =
    "4fc028b522105a59ba77aa548b49ec57ce22cfb9a1b6ebd386255bff607d9fdd";
const CYBERPUNK_RECIPE_V1_FINGERPRINT: &str =
    "665cea53a07936efbaf90d5880e993191dc6e036f2e0ac2cca2750aca2cad11d";
type PresetRecipeBuilder = fn(PresetPalette) -> ThemeDefinitionV1;

#[derive(Clone, Copy)]
pub(super) struct PresetPalette {
    pub(super) canvas: &'static str,
    pub(super) surface: &'static str,
    pub(super) surface_alt: &'static str,
    pub(super) surface_muted: &'static str,
    pub(super) text: &'static str,
    pub(super) subtle_text: &'static str,
    pub(super) border: &'static str,
    pub(super) line: &'static str,
    pub(super) accent: &'static str,
    pub(super) edge_label_background: &'static str,
    pub(super) cluster_background: &'static str,
    pub(super) cluster_border: &'static str,
    pub(super) note_background: &'static str,
    pub(super) note_border: &'static str,
    pub(super) note_text: &'static str,
    pub(super) actor_background: &'static str,
    pub(super) actor_border: &'static str,
    pub(super) actor_text: &'static str,
    pub(super) activation_background: &'static str,
    pub(super) activation_border: &'static str,
    pub(super) sequence_number_text: &'static str,
    pub(super) packet_field_label_text: &'static str,
    pub(super) series: &'static [&'static str],
}

pub(super) struct PresetCatalogEntry {
    preset: ThemePreset,
    id: &'static str,
    display_name: &'static str,
    dark_mode: bool,
    maturity: &'static str,
    catalog_schema_version: u32,
    authoring_schema_version: u32,
    expansion_version: u32,
    spec_schema_version: u32,
    recipe_revision: u32,
    recipe_fingerprint: &'static str,
    resource_fingerprint: &'static str,
    recipe_builder: PresetRecipeBuilder,
    palette: PresetPalette,
    qualified_cells: &'static [ThemePresetQualifiedCell],
    export_kind: &'static str,
    qualification_invalidation: ThemePresetQualificationInvalidation,
    required_feature_ids: &'static [&'static str],
    bundled_resource_ids: &'static [&'static str],
    allowed_residual_ids: &'static [&'static str],
    license_expression: &'static str,
    required_attribution: Option<&'static str>,
    retained: bool,
}

impl PresetCatalogEntry {
    pub(super) const fn preset(&self) -> ThemePreset {
        self.preset
    }

    pub(super) const fn id(&self) -> &'static str {
        self.id
    }

    pub(super) const fn display_name(&self) -> &'static str {
        self.display_name
    }

    pub(super) const fn is_dark(&self) -> bool {
        self.dark_mode
    }

    pub(super) const fn maturity(&self) -> &'static str {
        self.maturity
    }

    pub(super) const fn catalog_schema_version(&self) -> u32 {
        self.catalog_schema_version
    }

    pub(super) const fn authoring_schema_version(&self) -> u32 {
        self.authoring_schema_version
    }

    pub(super) const fn expansion_version(&self) -> u32 {
        self.expansion_version
    }

    pub(super) const fn spec_schema_version(&self) -> u32 {
        self.spec_schema_version
    }

    pub(super) const fn recipe_revision(&self) -> u32 {
        self.recipe_revision
    }

    pub(super) const fn recipe_fingerprint(&self) -> &'static str {
        self.recipe_fingerprint
    }

    pub(super) const fn resource_fingerprint(&self) -> &'static str {
        self.resource_fingerprint
    }

    pub(super) const fn qualified_cells(&self) -> &'static [ThemePresetQualifiedCell] {
        self.qualified_cells
    }

    pub(super) const fn export_kind(&self) -> &'static str {
        self.export_kind
    }

    pub(super) const fn qualification_invalidation(&self) -> ThemePresetQualificationInvalidation {
        self.qualification_invalidation
    }

    pub(super) const fn license_expression(&self) -> &'static str {
        self.license_expression
    }

    pub(super) const fn required_attribution(&self) -> Option<&'static str> {
        self.required_attribution
    }

    #[cfg(test)]
    pub(super) const fn palette(&self) -> PresetPalette {
        self.palette
    }

    #[cfg(test)]
    pub(super) const fn required_feature_ids(&self) -> &'static [&'static str] {
        self.required_feature_ids
    }

    #[cfg(test)]
    pub(super) const fn bundled_resource_ids(&self) -> &'static [&'static str] {
        self.bundled_resource_ids
    }

    #[cfg(test)]
    pub(super) const fn allowed_residual_ids(&self) -> &'static [&'static str] {
        self.allowed_residual_ids
    }

    #[cfg(test)]
    pub(super) const fn retained(&self) -> bool {
        self.retained
    }

    fn definition(&self) -> ThemeDefinitionV1 {
        (self.recipe_builder)(self.palette)
    }
}

const fn entry(
    preset: ThemePreset,
    id: &'static str,
    display_name: &'static str,
    dark_mode: bool,
    recipe_fingerprint: &'static str,
    palette: PresetPalette,
) -> PresetCatalogEntry {
    PresetCatalogEntry {
        preset,
        id,
        display_name,
        dark_mode,
        maturity: ALPHA,
        catalog_schema_version: CATALOG_SCHEMA_VERSION,
        authoring_schema_version: AUTHORING_SCHEMA_VERSION,
        expansion_version: EXPANSION_VERSION,
        spec_schema_version: SPEC_SCHEMA_VERSION,
        recipe_revision: 1,
        recipe_fingerprint,
        resource_fingerprint: DEFAULT_RESOURCE_FINGERPRINT,
        recipe_builder: build_cross_family_recipe,
        palette,
        qualified_cells: NO_QUALIFIED_CELLS,
        export_kind: "complete_spec",
        qualification_invalidation: ThemePresetQualificationInvalidation::current(),
        required_feature_ids: NO_IDS,
        bundled_resource_ids: NO_IDS,
        allowed_residual_ids: NO_IDS,
        license_expression: PROJECT_LICENSE,
        required_attribution: None,
        retained: true,
    }
}

const PRESET_CATALOG: [PresetCatalogEntry; 10] = [
    entry(
        ThemePreset::EditorLight,
        "editor-light",
        "Editor Light",
        false,
        EDITOR_LIGHT_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#ffffff",
            surface: "#f8fafc",
            surface_alt: "#e2e8f0",
            surface_muted: "#f1f5f9",
            text: "#0f172a",
            subtle_text: "#475569",
            border: "#94a3b8",
            line: "#64748b",
            accent: "#2563eb",
            edge_label_background: "#ffffff",
            cluster_background: "#f1f5f9",
            cluster_border: "#cbd5e1",
            note_background: "#fff7ed",
            note_border: "#fdba74",
            note_text: "#7c2d12",
            actor_background: "#f8fafc",
            actor_border: "#94a3b8",
            actor_text: "#0f172a",
            activation_background: "#e2e8f0",
            activation_border: "#94a3b8",
            sequence_number_text: "#ffffff",
            packet_field_label_text: "#0f172a",
            series: &[
                "#2563eb", "#059669", "#d97706", "#7c3aed", "#0891b2", "#be123c", "#a16207",
                "#65a30d",
            ],
        },
    ),
    entry(
        ThemePreset::EditorDark,
        "editor-dark",
        "Editor Dark",
        true,
        EDITOR_DARK_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#0f172a",
            surface: "#111827",
            surface_alt: "#1f2937",
            surface_muted: "#334155",
            text: "#e5e7eb",
            subtle_text: "#cbd5e1",
            border: "#475569",
            line: "#94a3b8",
            accent: "#60a5fa",
            edge_label_background: "#0f172a",
            cluster_background: "#1e293b",
            cluster_border: "#475569",
            note_background: "#422006",
            note_border: "#f59e0b",
            note_text: "#fef3c7",
            actor_background: "#1f2937",
            actor_border: "#475569",
            actor_text: "#e5e7eb",
            activation_background: "#334155",
            activation_border: "#64748b",
            sequence_number_text: "#0f172a",
            packet_field_label_text: "#0f172a",
            series: &[
                "#60a5fa", "#34d399", "#f59e0b", "#c084fc", "#22d3ee", "#fb7185", "#facc15",
                "#a3e635",
            ],
        },
    ),
    entry(
        ThemePreset::OneDark,
        "one-dark",
        "One Dark",
        true,
        ONE_DARK_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#282c34",
            surface: "#21252b",
            surface_alt: "#2c313a",
            surface_muted: "#3e4451",
            text: "#abb2bf",
            subtle_text: "#abb2bf",
            border: "#3e4451",
            line: "#61afef",
            accent: "#61afef",
            edge_label_background: "#282c34",
            cluster_background: "#2c313a",
            cluster_border: "#3e4451",
            note_background: "#3a2f1b",
            note_border: "#e5c07b",
            note_text: "#f0dca4",
            actor_background: "#2c313a",
            actor_border: "#3e4451",
            actor_text: "#abb2bf",
            activation_background: "#3e4451",
            activation_border: "#5c6370",
            sequence_number_text: "#282c34",
            packet_field_label_text: "#282c34",
            series: &[
                "#61afef", "#98c379", "#e5c07b", "#c678dd", "#56b6c2", "#e06c75", "#d19a66",
                "#be5046",
            ],
        },
    ),
    entry(
        ThemePreset::GruvboxLight,
        "gruvbox-light",
        "Gruvbox Light",
        false,
        GRUVBOX_LIGHT_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#fbf1c7",
            surface: "#f2e5bc",
            surface_alt: "#ebdbb2",
            surface_muted: "#d5c4a1",
            text: "#3c3836",
            subtle_text: "#665c54",
            border: "#d5c4a1",
            line: "#7c6f64",
            accent: "#458588",
            edge_label_background: "#fbf1c7",
            cluster_background: "#ebdbb2",
            cluster_border: "#d5c4a1",
            note_background: "#f2e5bc",
            note_border: "#d79921",
            note_text: "#3c3836",
            actor_background: "#ebdbb2",
            actor_border: "#d5c4a1",
            actor_text: "#3c3836",
            activation_background: "#d5c4a1",
            activation_border: "#bdae93",
            sequence_number_text: "#ffffff",
            packet_field_label_text: "#3c3836",
            series: &[
                "#458588", "#98971a", "#d79921", "#b16286", "#689d6a", "#cc241d", "#d65d0e",
                "#427b58",
            ],
        },
    ),
    entry(
        ThemePreset::GruvboxDark,
        "gruvbox-dark",
        "Gruvbox Dark",
        true,
        GRUVBOX_DARK_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#282828",
            surface: "#3c3836",
            surface_alt: "#504945",
            surface_muted: "#665c54",
            text: "#ebdbb2",
            subtle_text: "#d5c4a1",
            border: "#665c54",
            line: "#d5c4a1",
            accent: "#83a598",
            edge_label_background: "#282828",
            cluster_background: "#3c3836",
            cluster_border: "#665c54",
            note_background: "#3c3836",
            note_border: "#fabd2f",
            note_text: "#fbf1c7",
            actor_background: "#3c3836",
            actor_border: "#665c54",
            actor_text: "#ebdbb2",
            activation_background: "#504945",
            activation_border: "#7c6f64",
            sequence_number_text: "#282828",
            packet_field_label_text: "#282828",
            series: &[
                "#83a598", "#b8bb26", "#fabd2f", "#d3869b", "#8ec07c", "#fb4934", "#fe8019",
                "#689d6a",
            ],
        },
    ),
    entry(
        ThemePreset::AyuLight,
        "ayu-light",
        "Ayu Light",
        false,
        AYU_LIGHT_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#fcfcfc",
            surface: "#f3f4f5",
            surface_alt: "#e6e8eb",
            surface_muted: "#d9d7ce",
            text: "#5c6166",
            subtle_text: "#5c6166",
            border: "#8a9199",
            line: "#5c6166",
            accent: "#55b4d4",
            edge_label_background: "#fcfcfc",
            cluster_background: "#f3f4f5",
            cluster_border: "#8a9199",
            note_background: "#fff3bf",
            note_border: "#ffaa33",
            note_text: "#5c6166",
            actor_background: "#f3f4f5",
            actor_border: "#8a9199",
            actor_text: "#5c6166",
            activation_background: "#e6e8eb",
            activation_border: "#8a9199",
            sequence_number_text: "#ffffff",
            packet_field_label_text: "#5c6166",
            series: &[
                "#55b4d4", "#86b300", "#ffaa33", "#a37acc", "#4cbf99", "#f07171", "#f2ae49",
                "#399ee6",
            ],
        },
    ),
    entry(
        ThemePreset::AyuDark,
        "ayu-dark",
        "Ayu Dark",
        true,
        AYU_DARK_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#0b0e14",
            surface: "#11151c",
            surface_alt: "#1f2430",
            surface_muted: "#343b48",
            text: "#bfbdb6",
            subtle_text: "#8a9199",
            border: "#343b48",
            line: "#59c2ff",
            accent: "#59c2ff",
            edge_label_background: "#0b0e14",
            cluster_background: "#1f2430",
            cluster_border: "#343b48",
            note_background: "#332a14",
            note_border: "#ffb454",
            note_text: "#ffdf99",
            actor_background: "#1f2430",
            actor_border: "#343b48",
            actor_text: "#bfbdb6",
            activation_background: "#343b48",
            activation_border: "#4f5866",
            sequence_number_text: "#0b0e14",
            packet_field_label_text: "#0b0e14",
            series: &[
                "#59c2ff", "#aad94c", "#ffb454", "#d2a6ff", "#95e6cb", "#f07178", "#ff8f40",
                "#e6b673",
            ],
        },
    ),
    entry(
        ThemePreset::Brutalist,
        "brutalist",
        "Brutalist",
        false,
        BRUTALIST_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#f4f0e6",
            surface: "#fffdf5",
            surface_alt: "#ffd84d",
            surface_muted: "#d9d2c3",
            text: "#111111",
            subtle_text: "#3d3d3d",
            border: "#111111",
            line: "#111111",
            accent: "#ff4f00",
            edge_label_background: "#fffdf5",
            cluster_background: "#ffe88a",
            cluster_border: "#111111",
            note_background: "#ffd84d",
            note_border: "#111111",
            note_text: "#111111",
            actor_background: "#fffdf5",
            actor_border: "#111111",
            actor_text: "#111111",
            activation_background: "#ff6b35",
            activation_border: "#111111",
            sequence_number_text: "#ffffff",
            packet_field_label_text: "#111111",
            series: &[
                "#ff4f00", "#006d77", "#ffba08", "#8338ec", "#3a86ff", "#d00000", "#2a9d8f",
                "#6a4c93",
            ],
        },
    ),
    entry(
        ThemePreset::Spotless,
        "spotless",
        "Spotless",
        false,
        SPOTLESS_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#f7f5ef",
            surface: "#ffffff",
            surface_alt: "#eeeae0",
            surface_muted: "#e2ded2",
            text: "#1b1b1b",
            subtle_text: "#57534e",
            border: "#b8b2a7",
            line: "#2c2416",
            accent: "#8b5e34",
            edge_label_background: "#f7f5ef",
            cluster_background: "#f0ece2",
            cluster_border: "#8c867b",
            note_background: "#fff8e7",
            note_border: "#b89245",
            note_text: "#4a3712",
            actor_background: "#ffffff",
            actor_border: "#8c867b",
            actor_text: "#1b1b1b",
            activation_background: "#eeeae0",
            activation_border: "#8c867b",
            sequence_number_text: "#ffffff",
            packet_field_label_text: "#1b1b1b",
            series: &[
                "#8b5e34", "#557a46", "#9a6aa8", "#b2604b", "#4f748d", "#987b2f", "#6f6a91",
                "#5c7c76",
            ],
        },
    ),
    entry(
        ThemePreset::Cyberpunk,
        "cyberpunk",
        "Cyberpunk",
        true,
        CYBERPUNK_RECIPE_V1_FINGERPRINT,
        PresetPalette {
            canvas: "#020617",
            surface: "#0f172a",
            surface_alt: "#111827",
            surface_muted: "#1e293b",
            text: "#e0f2fe",
            subtle_text: "#a5f3fc",
            border: "#22d3ee",
            line: "#22d3ee",
            accent: "#f0abfc",
            edge_label_background: "#020617",
            cluster_background: "#111827",
            cluster_border: "#22d3ee",
            note_background: "#312e81",
            note_border: "#f0abfc",
            note_text: "#f5f3ff",
            actor_background: "#0f172a",
            actor_border: "#22d3ee",
            actor_text: "#e0f2fe",
            activation_background: "#164e63",
            activation_border: "#22d3ee",
            sequence_number_text: "#020617",
            packet_field_label_text: "#020617",
            series: &[
                "#22d3ee", "#f472b6", "#a3e635", "#facc15", "#c084fc", "#38bdf8", "#fb7185",
                "#2dd4bf",
            ],
        },
    ),
];

const fn descriptor_projection() -> [ThemePresetDescriptor; PRESET_CATALOG.len()] {
    let mut descriptors = [ThemePresetDescriptor::from_catalog_index(0); PRESET_CATALOG.len()];
    let mut index = 0;
    while index < PRESET_CATALOG.len() {
        descriptors[index] = ThemePresetDescriptor::from_catalog_index(index as u8);
        index += 1;
    }
    descriptors
}

const DISCOVERABLE_DESCRIPTORS: [ThemePresetDescriptor; PRESET_CATALOG.len()] =
    descriptor_projection();

pub(super) const fn descriptors() -> &'static [ThemePresetDescriptor] {
    &DISCOVERABLE_DESCRIPTORS
}

pub(super) const fn entry_at(index: usize) -> &'static PresetCatalogEntry {
    &PRESET_CATALOG[index]
}

pub(super) fn entry_for_preset(preset: ThemePreset) -> &'static PresetCatalogEntry {
    PRESET_CATALOG
        .iter()
        .find(|entry| entry.preset == preset && is_discoverable(entry))
        .expect("every ThemePreset projection must resolve through the catalog")
}

pub(super) fn entry_for_id(id: &str) -> Option<&'static PresetCatalogEntry> {
    PRESET_CATALOG
        .iter()
        .find(|entry| entry.id == id && is_discoverable(entry))
}

pub(crate) fn materialize_spec_wire(
    preset: ThemePreset,
    resources: &ThemeResourcePolicy,
) -> Result<DiagramThemeSpecWireV1, ThemeMaterializationErrorV1> {
    let entry = entry_for_preset(preset);
    debug_assert!(
        entry.allowed_residual_ids.is_empty(),
        "unqualified alpha presets must not inherit qualification residual allowances"
    );
    let mut spec =
        materialize_theme_with_resource_policy(&entry.definition(), resources)?.into_spec();
    spec.mermaid = Some(MermaidThemeCompatibilityWireV1 {
        theme: Some("base".to_owned()),
        dark_mode: Some(entry.dark_mode),
        variables: None,
    });
    Ok(spec)
}

fn is_discoverable(entry: &PresetCatalogEntry) -> bool {
    entry.retained && entry.required_feature_ids.is_empty() && entry.bundled_resource_ids.is_empty()
}

fn build_cross_family_recipe(palette: PresetPalette) -> ThemeDefinitionV1 {
    let tokens = ThemeTokensV1::default()
        .with_color(ThemeColorTokenV1::Canvas, palette.canvas)
        .with_color(ThemeColorTokenV1::Surface, palette.surface)
        .with_color(ThemeColorTokenV1::SurfaceAlt, palette.surface_alt)
        .with_color(ThemeColorTokenV1::SurfaceMuted, palette.surface_muted)
        .with_color(ThemeColorTokenV1::Text, palette.text)
        .with_color(ThemeColorTokenV1::Border, palette.border)
        .with_color(ThemeColorTokenV1::Line, palette.line)
        .with_color(ThemeColorTokenV1::Accent, palette.accent)
        .with_series(
            palette
                .series
                .iter()
                .map(|color| (*color).to_owned())
                .collect(),
        );
    ThemeDefinitionV1::new(tokens).with_styles(vec![
        preset_rule("node", Some(palette.surface), None),
        preset_rule(
            "edge-label-background",
            Some(palette.edge_label_background),
            None,
        ),
        preset_rule(
            "cluster",
            Some(palette.cluster_background),
            Some(palette.cluster_border),
        ),
        preset_rule(
            "actor",
            Some(palette.actor_background),
            Some(palette.actor_border),
        ),
        preset_rule("actor-label", Some(palette.actor_text), None),
        preset_rule(
            "lifeline",
            Some(palette.actor_border),
            Some(palette.actor_border),
        ),
        preset_rule("sequence-number", Some(palette.sequence_number_text), None),
        preset_rule("message-label", Some(palette.text), None),
        preset_rule("loop", Some(palette.surface_alt), Some(palette.border)),
        preset_rule("loop-label", Some(palette.text), None),
        preset_rule(
            "transition-label-background",
            Some(palette.edge_label_background),
            None,
        ),
        preset_rule(
            "note",
            Some(palette.note_background),
            Some(palette.note_border),
        ),
        preset_rule("note-label", Some(palette.note_text), None),
        preset_rule("packet-byte-label", Some(palette.text), None),
        preset_rule(
            "packet-field-label",
            Some(palette.packet_field_label_text),
            None,
        ),
        preset_rule(
            "activation",
            Some(palette.activation_background),
            Some(palette.activation_border),
        ),
        preset_rule("axis", Some(palette.text), Some(palette.line)),
        preset_rule("legend", Some(palette.subtle_text), None),
        preset_palette_rule("task", palette.series),
        preset_palette_rule("chart-series", palette.series),
        preset_palette_rule("timeline-event", palette.series),
        preset_palette_rule("journey-task", palette.series),
    ])
}

fn preset_rule(target: &str, fill: Option<&str>, stroke: Option<&str>) -> ThemeRuleSetWireV1 {
    let mut style = ThemeStylePatchWireV1::default();
    if let Some(fill) = fill {
        style.fill = SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(fill.to_owned()));
    }
    if let Some(stroke) = stroke {
        style.stroke = Some(ThemeStrokePatchWireV1 {
            paint: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(stroke.to_owned())),
            ..ThemeStrokePatchWireV1::default()
        });
    }
    ThemeRuleSetWireV1::Rule {
        target: target.to_owned(),
        family: None,
        variant: None,
        ordinal: None,
        style,
    }
}

fn preset_palette_rule(target: &str, colors: &[&str]) -> ThemeRuleSetWireV1 {
    ThemeRuleSetWireV1::OrdinalPalette {
        target: target.to_owned(),
        colors: colors.iter().map(|color| (*color).to_owned()).collect(),
    }
}
