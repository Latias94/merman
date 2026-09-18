use merman_theme_contract::{
    DiagramThemeSpecWireV1, MermaidThemeCompatibilityWireV1, SpecifiedWireV1,
    ThemeCanvasPaintWireV1, ThemeColorTokenV1, ThemeDefinitionV1, ThemeMaterializationErrorV1,
    ThemeOrdinalCycleWireV1, ThemeOrdinalSelectorWireV1, ThemeRuleSetWireV1,
    ThemeStrokePatchWireV1, ThemeStylePatchWireV1, ThemeTokensV1,
};

use super::{
    ThemePreset, ThemePresetDescriptor, ThemePresetQualificationInvalidation,
    ThemePresetQualifiedCell,
};
use crate::DiagramFamilyId;
use crate::diagram_theme::definition_admission::{
    check_complete_spec_encoded_bytes, materialize_theme_with_resource_policy,
};
use crate::diagram_theme::{ThemeResourcePolicy, ThemeTarget};

const CATALOG_SCHEMA_VERSION: u32 = 1;
const AUTHORING_SCHEMA_VERSION: u32 = 1;
const EXPANSION_VERSION: u32 = 1;
const SPEC_SCHEMA_VERSION: u32 = 1;
const ALPHA: &str = "alpha";
const PROJECT_LICENSE: &str = "MIT OR Apache-2.0";
const NO_QUALIFIED_CELLS: &[ThemePresetQualifiedCell] = &[];
// These audited families currently use the shared palette recipe. Family-specific color fixes
// do not amount to the complete dedicated visual designs of the Modern Mermaid references.
// Keep unreviewed families absent instead of deriving aesthetic scope from renderer support.
const SHARED_PALETTE_DESIGNS: &[(&str, &str)] = &[
    ("class", "base_only"),
    ("flowchart", "base_only"),
    ("sequence", "base_only"),
    ("xychart", "base_only"),
];
const NO_IDS: &[&str] = &[];
const DEFAULT_RESOURCE_FINGERPRINT: &str =
    "c6aa7af73322aac35ce4548369140848c9a4d093700abccb063ea47a1797d0aa";
const EDITOR_LIGHT_RECIPE_FINGERPRINT: &str =
    "a98b7a6e93bbbc17832c6be11ebe09008c5382a38d2a27fc5ca52f5abb2e5411";
const EDITOR_DARK_RECIPE_FINGERPRINT: &str =
    "49a46a985abe9a53194f8aa6d5c95f79383eb562f63097a684992ff001829aa0";
const ONE_DARK_RECIPE_FINGERPRINT: &str =
    "c94f5100f62f6664da322275127c4f60556c3339ef3ba03d0c9ace84a44230da";
const GRUVBOX_LIGHT_RECIPE_FINGERPRINT: &str =
    "fe8ac082387f78aeaa7ae198d57b358991b395e2b212de4eb03d0c62151c6b90";
const GRUVBOX_DARK_RECIPE_FINGERPRINT: &str =
    "d0cd109708af411f35fee3a6e8e2c8c133280942f855af97e706efaf3b05dd42";
const AYU_LIGHT_RECIPE_FINGERPRINT: &str =
    "e62e3c175ac47801ec2fe2a631fe253aeaf60dc58f00917920ce1c0ccd45ea00";
const AYU_DARK_RECIPE_FINGERPRINT: &str =
    "3ebcce80dfe68ee9fd312ddd4fdc6e0f556eb46e74d1ebd84e835bc039a64c87";
const BRUTALIST_RECIPE_FINGERPRINT: &str =
    "cff2165b390d27fe5dd03bb60cefb18489dd9375fd37f1167036e427f452c30a";
const SPOTLESS_RECIPE_FINGERPRINT: &str =
    "c177e9885bb2ad7ccaf59f83702c43a01b913b6a4964f24abdc00c1c6b2a346b";
const CYBERPUNK_RECIPE_FINGERPRINT: &str =
    "932a54d67b0bec2e8e9f3964bff5b3b7c203085c587b4b49fb13a613ca16fcb5";
type PresetRecipeBuilder = fn(
    PresetPalette,
    &ThemeResourcePolicy,
) -> Result<DiagramThemeSpecWireV1, ThemeMaterializationErrorV1>;

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
    pub(super) kanban_task_labels: &'static [&'static str],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PresetRecipeProfile {
    RetainedMermaidCompatibility,
    Native,
}

pub(super) struct PresetCatalogEntry {
    preset: ThemePreset,
    id: &'static str,
    display_name: &'static str,
    dark_mode: bool,
    profile: PresetRecipeProfile,
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
    family_designs: &'static [(&'static str, &'static str)],
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

    pub(super) const fn family_designs(&self) -> &'static [(&'static str, &'static str)] {
        self.family_designs
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

    fn recipe(
        &self,
        resources: &ThemeResourcePolicy,
    ) -> Result<DiagramThemeSpecWireV1, ThemeMaterializationErrorV1> {
        (self.recipe_builder)(self.palette, resources)
    }
}

const fn entry(
    preset: ThemePreset,
    id: &'static str,
    display_name: &'static str,
    dark_mode: bool,
    profile: PresetRecipeProfile,
    recipe_fingerprint: &'static str,
    palette: PresetPalette,
) -> PresetCatalogEntry {
    PresetCatalogEntry {
        preset,
        id,
        display_name,
        dark_mode,
        profile,
        maturity: ALPHA,
        catalog_schema_version: CATALOG_SCHEMA_VERSION,
        authoring_schema_version: AUTHORING_SCHEMA_VERSION,
        expansion_version: EXPANSION_VERSION,
        spec_schema_version: SPEC_SCHEMA_VERSION,
        // Unpublished recipes share revision 1; fingerprints identify content changes.
        recipe_revision: 1,
        recipe_fingerprint,
        resource_fingerprint: DEFAULT_RESOURCE_FINGERPRINT,
        recipe_builder: match preset {
            ThemePreset::Cyberpunk => super::cyberpunk::build_recipe,
            _ => build_cross_family_recipe,
        },
        palette,
        family_designs: SHARED_PALETTE_DESIGNS,
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
        PresetRecipeProfile::RetainedMermaidCompatibility,
        EDITOR_LIGHT_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &["#000000"; 8],
        },
    ),
    entry(
        ThemePreset::EditorDark,
        "editor-dark",
        "Editor Dark",
        true,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        EDITOR_DARK_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &["#000000"; 8],
        },
    ),
    entry(
        ThemePreset::OneDark,
        "one-dark",
        "One Dark",
        true,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        ONE_DARK_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &[
                "#000000", "#000000", "#000000", "#000000", "#000000", "#000000", "#000000",
                "#ffffff",
            ],
        },
    ),
    entry(
        ThemePreset::GruvboxLight,
        "gruvbox-light",
        "Gruvbox Light",
        false,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        GRUVBOX_LIGHT_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &["#000000"; 8],
        },
    ),
    entry(
        ThemePreset::GruvboxDark,
        "gruvbox-dark",
        "Gruvbox Dark",
        true,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        GRUVBOX_DARK_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &[
                "#000000", "#000000", "#000000", "#000000", "#000000", "#000000", "#000000",
                "#ffffff",
            ],
        },
    ),
    entry(
        ThemePreset::AyuLight,
        "ayu-light",
        "Ayu Light",
        false,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        AYU_LIGHT_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &["#000000"; 8],
        },
    ),
    entry(
        ThemePreset::AyuDark,
        "ayu-dark",
        "Ayu Dark",
        true,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        AYU_DARK_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &["#000000"; 8],
        },
    ),
    entry(
        ThemePreset::Brutalist,
        "brutalist",
        "Brutalist",
        false,
        PresetRecipeProfile::Native,
        BRUTALIST_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &[
                "#000000", "#000000", "#000000", "#000000", "#000000", "#000000", "#000000",
                "#ffffff",
            ],
        },
    ),
    entry(
        ThemePreset::Spotless,
        "spotless",
        "Spotless",
        false,
        PresetRecipeProfile::Native,
        SPOTLESS_RECIPE_FINGERPRINT,
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
            kanban_task_labels: &["#000000"; 8],
        },
    ),
    entry(
        ThemePreset::Cyberpunk,
        "cyberpunk",
        "Cyberpunk",
        true,
        PresetRecipeProfile::Native,
        CYBERPUNK_RECIPE_FINGERPRINT,
        PresetPalette {
            canvas: "#051423",
            surface: "#051423",
            surface_alt: "#051423",
            surface_muted: "#051423",
            text: "#00f2ff",
            subtle_text: "#00f2ff",
            border: "#00f2ff",
            line: "#00f2ff",
            accent: "#ff00ff",
            edge_label_background: "#051423",
            cluster_background: "#051423",
            cluster_border: "#00f2ff",
            note_background: "#051423",
            note_border: "#ff00ff",
            note_text: "#ff00ff",
            actor_background: "#051423",
            actor_border: "#00f2ff",
            actor_text: "#00f2ff",
            activation_background: "rgba(0, 242, 255, 0.1)",
            activation_border: "#00f2ff",
            sequence_number_text: "#051423",
            packet_field_label_text: "#051423",
            series: &[
                "#22d3ee", "#f472b6", "#a3e635", "#facc15", "#c084fc", "#38bdf8", "#fb7185",
                "#2dd4bf",
            ],
            kanban_task_labels: &["#000000"; 8],
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
    let mut spec = entry.recipe(resources)?;
    if entry.profile == PresetRecipeProfile::RetainedMermaidCompatibility {
        spec.mermaid = Some(MermaidThemeCompatibilityWireV1 {
            theme: Some("base".to_owned()),
            dark_mode: Some(entry.dark_mode),
            variables: None,
        });
    }
    check_complete_spec_encoded_bytes(resources, &spec)?;
    Ok(spec)
}

fn is_discoverable(entry: &PresetCatalogEntry) -> bool {
    entry.retained && entry.required_feature_ids.is_empty() && entry.bundled_resource_ids.is_empty()
}

pub(super) fn build_cross_family_recipe(
    palette: PresetPalette,
    resources: &ThemeResourcePolicy,
) -> Result<DiagramThemeSpecWireV1, ThemeMaterializationErrorV1> {
    materialize_theme_with_resource_policy(&build_cross_family_definition(palette), resources)
        .map(|materialized| materialized.into_spec())
}

fn build_cross_family_definition(palette: PresetPalette) -> ThemeDefinitionV1 {
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
    let first_series = palette
        .series
        .first()
        .copied()
        .expect("preset palettes must contain at least one series color");
    let mut styles = Vec::new();

    // Mindmap's direct palette needs the absence of a static fill winner. Keep the shared surface
    // recipe on the other Node families instead of installing one cross-family rule that silently
    // suppresses the Mindmap ordinal colors.
    for family in [
        DiagramFamilyId::FLOWCHART,
        DiagramFamilyId::SWIMLANE,
        DiagramFamilyId::CLASS,
        DiagramFamilyId::TREE_VIEW,
        DiagramFamilyId::BLOCK,
    ] {
        styles.push(preset_family_rule(
            family,
            ThemeTarget::Node,
            Some(palette.surface),
            None,
        ));
    }

    styles.extend([
        preset_rule(
            ThemeTarget::EdgeLabelBackground,
            Some(palette.edge_label_background),
            None,
        ),
        preset_rule(
            ThemeTarget::Cluster,
            Some(palette.cluster_background),
            Some(palette.cluster_border),
        ),
        preset_rule(
            ThemeTarget::Actor,
            Some(palette.actor_background),
            Some(palette.actor_border),
        ),
        preset_rule(ThemeTarget::ActorLabel, Some(palette.actor_text), None),
        preset_rule(
            ThemeTarget::Lifeline,
            Some(palette.actor_border),
            Some(palette.actor_border),
        ),
        preset_rule(
            ThemeTarget::SequenceNumberLabel,
            Some(palette.sequence_number_text),
            None,
        ),
        preset_rule(ThemeTarget::MessageLabel, Some(palette.text), None),
        preset_rule(
            ThemeTarget::Loop,
            Some(palette.surface_alt),
            Some(palette.border),
        ),
        preset_rule(ThemeTarget::LoopLabel, Some(palette.text), None),
        preset_rule(
            ThemeTarget::TransitionLabelBackground,
            Some(palette.edge_label_background),
            None,
        ),
        preset_rule(
            ThemeTarget::Note,
            Some(palette.note_background),
            Some(palette.note_border),
        ),
        preset_rule(ThemeTarget::NoteLabel, Some(palette.note_text), None),
        preset_rule(ThemeTarget::PacketByteLabel, Some(palette.text), None),
        preset_rule(
            ThemeTarget::PacketFieldLabel,
            Some(palette.packet_field_label_text),
            None,
        ),
        preset_rule(
            ThemeTarget::Activation,
            Some(palette.activation_background),
            Some(palette.activation_border),
        ),
        preset_rule(ThemeTarget::Axis, Some(palette.text), Some(palette.line)),
        preset_rule(ThemeTarget::Legend, Some(palette.subtle_text), None),
        preset_family_rule(
            DiagramFamilyId::ER,
            ThemeTarget::Relation,
            None,
            Some(palette.line),
        ),
        preset_family_rule(
            DiagramFamilyId::REQUIREMENT,
            ThemeTarget::Requirement,
            Some(palette.surface),
            None,
        ),
        preset_family_rule(
            DiagramFamilyId::PIE,
            ThemeTarget::PieSlice,
            None,
            Some(palette.border),
        ),
        preset_family_rule(
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::ChartSeries,
            Some(first_series),
            None,
        ),
        preset_family_rule(
            DiagramFamilyId::GANTT,
            ThemeTarget::Task,
            Some(palette.surface),
            None,
        ),
        preset_palette_rule(ThemeTarget::Task, palette.series),
        preset_palette_rule(ThemeTarget::ChartSeries, palette.series),
        preset_palette_rule(ThemeTarget::TimelineEvent, palette.series),
        preset_palette_rule(ThemeTarget::JourneyTask, palette.series),
    ]);

    assert_eq!(
        palette.kanban_task_labels.len(),
        palette.series.len(),
        "each preset series slot must own one Kanban task-label foreground"
    );
    for terminal_slot in 0usize..12 {
        styles.push(preset_ordinal_fill_rule(
            DiagramFamilyId::KANBAN,
            ThemeTarget::TaskLabel,
            12,
            terminal_slot as u32,
            palette.kanban_task_labels[terminal_slot % palette.kanban_task_labels.len()],
        ));
    }

    ThemeDefinitionV1::new(tokens).with_styles(styles)
}

fn preset_rule(
    target: ThemeTarget,
    fill: Option<&str>,
    stroke: Option<&str>,
) -> ThemeRuleSetWireV1 {
    preset_rule_for_family(None, target, fill, stroke)
}

fn preset_family_rule(
    family: DiagramFamilyId,
    target: ThemeTarget,
    fill: Option<&str>,
    stroke: Option<&str>,
) -> ThemeRuleSetWireV1 {
    preset_rule_for_family(Some(family), target, fill, stroke)
}

fn preset_rule_for_family(
    family: Option<DiagramFamilyId>,
    target: ThemeTarget,
    fill: Option<&str>,
    stroke: Option<&str>,
) -> ThemeRuleSetWireV1 {
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
        target: target.id().to_owned(),
        family: family.map(|family| family.as_str().to_owned()),
        variant: None,
        ordinal: None,
        style,
    }
}

fn preset_palette_rule(target: ThemeTarget, colors: &[&str]) -> ThemeRuleSetWireV1 {
    ThemeRuleSetWireV1::OrdinalPalette {
        target: target.id().to_owned(),
        colors: colors.iter().map(|color| (*color).to_owned()).collect(),
    }
}

fn preset_ordinal_fill_rule(
    family: DiagramFamilyId,
    target: ThemeTarget,
    period: u32,
    offset: u32,
    fill: &str,
) -> ThemeRuleSetWireV1 {
    let style = ThemeStylePatchWireV1 {
        fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(fill.to_owned())),
        ..ThemeStylePatchWireV1::default()
    };
    ThemeRuleSetWireV1::Rule {
        target: target.id().to_owned(),
        family: Some(family.as_str().to_owned()),
        variant: None,
        ordinal: Some(ThemeOrdinalSelectorWireV1::Cycle {
            cycle: ThemeOrdinalCycleWireV1 { period, offset },
        }),
        style,
    }
}
