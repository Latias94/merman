use merman_theme_contract::*;

use super::design::*;
use super::{ThemePreset, ThemePresetDescriptor, ThemePresetQualifiedCell};
use crate::DiagramFamilyId;
use crate::diagram_theme::{
    ThemeResourcePolicy, ThemeTarget, ThemeVariant, materialize_theme_design_with_resource_policy,
};

const ALPHA: &str = "alpha";
const PROJECT_LICENSE: &str = "MIT OR Apache-2.0";
const NO_QUALIFIED_CELLS: &[ThemePresetQualifiedCell] = &[];
// Families without a dedicated recipe use the shared palette recipe. Keep unreviewed families
// absent instead of deriving aesthetic scope from renderer support.
const SHARED_PALETTE_DESIGNS: &[(&str, &str)] = &[
    ("class", "base_only"),
    ("flowchart", "base_only"),
    ("sequence", "base_only"),
    ("xychart", "base_only"),
];
const README_STYLE_DESIGNS: &[(&str, &str)] = &[
    ("class", "base_only"),
    ("flowchart", "dedicated"),
    ("sequence", "dedicated"),
    ("xychart", "dedicated"),
];
const GHIBLI_DESIGNS: &[(&str, &str)] = &[("pie", "dedicated")];
const NO_IDS: &[&str] = &[];
type PresetRecipeBuilder = fn(PresetPalette) -> ThemeDesignV1;

#[derive(Clone, Copy)]
pub(super) struct PresetPalette {
    pub(super) canvas: &'static str,
    pub(super) surface: &'static str,
    pub(super) surface_alt: &'static str,
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
    pub(super) gantt_active_text: &'static str,
    pub(super) gantt_critical: &'static str,
    pub(super) gantt_critical_text: &'static str,
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
    recipe_builder: PresetRecipeBuilder,
    palette: PresetPalette,
    family_designs: &'static [(&'static str, &'static str)],
    qualified_cells: &'static [ThemePresetQualifiedCell],
    export_kind: &'static str,
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

    pub(super) const fn family_designs(&self) -> &'static [(&'static str, &'static str)] {
        self.family_designs
    }

    pub(super) fn preview_colors(&self) -> Vec<String> {
        let mut colors = Vec::new();
        for color in [self.palette.canvas, self.palette.surface, self.palette.text]
            .into_iter()
            .chain(self.palette.series.iter().take(5).copied())
        {
            if !colors.iter().any(|value| value == color) {
                colors.push(color.to_owned());
            }
        }
        colors
    }

    pub(super) fn application_notes(&self) -> Vec<ThemePresetApplicationNoteV1> {
        let (id, message) = match self.preset {
            ThemePreset::Brutalist => (
                "flowchart-classic-shadows",
                "Hard node shadows need Classic look and supported basic shapes (rectangles, diamonds and circles). Selecting this preset does not change the diagram's look.",
            ),
            ThemePreset::Cyberpunk => (
                "flowchart-classic-glow",
                "Node glow needs Classic look and supported basic shapes. Selecting this preset does not change the diagram's look.",
            ),
            _ => return Vec::new(),
        };
        vec![ThemePresetApplicationNoteV1 {
            id: id.to_owned(),
            family_id: "flowchart".to_owned(),
            message: message.to_owned(),
            recommended_config: Some(
                serde_json::json!({"flowchart": {"look": "classic"}})
                    .as_object()
                    .expect("catalog recommendation is a configuration object")
                    .clone(),
            ),
        }]
    }

    pub(super) const fn qualified_cells(&self) -> &'static [ThemePresetQualifiedCell] {
        self.qualified_cells
    }

    pub(super) const fn export_kind(&self) -> &'static str {
        self.export_kind
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

    fn recipe(&self) -> ThemeDesignV1 {
        (self.recipe_builder)(self.palette)
    }
}

const fn entry(
    preset: ThemePreset,
    id: &'static str,
    display_name: &'static str,
    dark_mode: bool,
    profile: PresetRecipeProfile,
    palette: PresetPalette,
) -> PresetCatalogEntry {
    PresetCatalogEntry {
        preset,
        id,
        display_name,
        dark_mode,
        profile,
        maturity: ALPHA,
        recipe_builder: match preset {
            ThemePreset::Brutalist => super::brutalist::build_recipe,
            ThemePreset::Spotless => super::spotless::build_recipe,
            ThemePreset::Cyberpunk => super::cyberpunk::build_recipe,
            ThemePreset::Ghibli => super::ghibli::build_recipe,
            _ => build_cross_family_recipe,
        },
        palette,
        family_designs: match preset {
            ThemePreset::Brutalist | ThemePreset::Spotless => README_STYLE_DESIGNS,
            ThemePreset::Ghibli => GHIBLI_DESIGNS,
            _ => SHARED_PALETTE_DESIGNS,
        },
        qualified_cells: NO_QUALIFIED_CELLS,
        export_kind: "design",
        required_feature_ids: NO_IDS,
        bundled_resource_ids: NO_IDS,
        allowed_residual_ids: NO_IDS,
        license_expression: PROJECT_LICENSE,
        required_attribution: None,
        retained: true,
    }
}

const PRESET_CATALOG: [PresetCatalogEntry; 11] = [
    entry(
        ThemePreset::EditorLight,
        "editor-light",
        "Editor Light",
        false,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        PresetPalette {
            canvas: "#ffffff",
            surface: "#f8fafc",
            surface_alt: "#e2e8f0",
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
            gantt_active_text: "#ffffff",
            gantt_critical: "#be123c",
            gantt_critical_text: "#ffffff",
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
        PresetPalette {
            canvas: "#0f172a",
            surface: "#111827",
            surface_alt: "#1f2937",
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
            gantt_active_text: "#0f172a",
            gantt_critical: "#fb7185",
            gantt_critical_text: "#0f172a",
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
        PresetPalette {
            canvas: "#282c34",
            surface: "#21252b",
            surface_alt: "#2c313a",
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
            gantt_active_text: "#21252b",
            gantt_critical: "#e06c75",
            gantt_critical_text: "#21252b",
            series: &[
                "#61afef", "#98c379", "#e5c07b", "#c678dd", "#56b6c2", "#e06c75", "#d19a66",
                "#be5046",
            ],
            kanban_task_labels: &[
                "#000000", "#000000", "#000000", "#000000", "#000000", "#000000", "#000000",
                "#000000",
            ],
        },
    ),
    entry(
        ThemePreset::GruvboxLight,
        "gruvbox-light",
        "Gruvbox Light",
        false,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        PresetPalette {
            canvas: "#fbf1c7",
            surface: "#f2e5bc",
            surface_alt: "#ebdbb2",
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
            gantt_active_text: "#ffffff",
            gantt_critical: "#cc241d",
            gantt_critical_text: "#ffffff",
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
        PresetPalette {
            canvas: "#282828",
            surface: "#3c3836",
            surface_alt: "#504945",
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
            gantt_active_text: "#282828",
            gantt_critical: "#fb4934",
            gantt_critical_text: "#282828",
            series: &[
                "#83a598", "#b8bb26", "#fabd2f", "#d3869b", "#8ec07c", "#fb4934", "#fe8019",
                "#689d6a",
            ],
            kanban_task_labels: &[
                "#000000", "#000000", "#000000", "#000000", "#000000", "#000000", "#000000",
                "#000000",
            ],
        },
    ),
    entry(
        ThemePreset::AyuLight,
        "ayu-light",
        "Ayu Light",
        false,
        PresetRecipeProfile::RetainedMermaidCompatibility,
        PresetPalette {
            canvas: "#fcfcfc",
            surface: "#f3f4f5",
            surface_alt: "#e6e8eb",
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
            gantt_active_text: "#11151c",
            gantt_critical: "#f07171",
            gantt_critical_text: "#11151c",
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
        PresetPalette {
            canvas: "#0b0e14",
            surface: "#11151c",
            surface_alt: "#1f2430",
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
            gantt_active_text: "#0b0e14",
            gantt_critical: "#f07178",
            gantt_critical_text: "#0b0e14",
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
        PresetPalette {
            canvas: "#f6f3e9",
            surface: "#ffffff",
            surface_alt: "#FFE66D",
            text: "#000000",
            subtle_text: "#3d3d3d",
            border: "#000000",
            line: "#000000",
            accent: "#FF6B35",
            edge_label_background: "#ffffff",
            cluster_background: "#ffe88a",
            cluster_border: "#000000",
            note_background: "#FFE66D",
            note_border: "#000000",
            note_text: "#000000",
            actor_background: "#ffffff",
            actor_border: "#000000",
            actor_text: "#000000",
            activation_background: "#ff6b35",
            activation_border: "#000000",
            sequence_number_text: "#ffffff",
            packet_field_label_text: "#000000",
            gantt_active_text: "#000000",
            gantt_critical: "#d00000",
            gantt_critical_text: "#ffffff",
            series: &[
                "#FF6B35", "#4ECDC4", "#FFE66D", "#8338ec", "#3a86ff", "#d00000", "#2a9d8f",
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
        PresetPalette {
            canvas: "#EDE8DC",
            surface: "#F5F1E8",
            surface_alt: "#eeeae0",
            text: "#1a1a1a",
            subtle_text: "#57534e",
            border: "#2C2416",
            line: "#2c2416",
            accent: "#8b5e34",
            edge_label_background: "#EDE8DC",
            cluster_background: "#f0ece2",
            cluster_border: "#2C2416",
            note_background: "#fff8e7",
            note_border: "#b89245",
            note_text: "#4a3712",
            actor_background: "#F5F1E8",
            actor_border: "#2C2416",
            actor_text: "#1a1a1a",
            activation_background: "#eeeae0",
            activation_border: "#2C2416",
            sequence_number_text: "#F5F1E8",
            packet_field_label_text: "#1a1a1a",
            gantt_active_text: "#ffffff",
            gantt_critical: "#b2604b",
            gantt_critical_text: "#ffffff",
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
        PresetPalette {
            canvas: "#051423",
            surface: "#051423",
            surface_alt: "#051423",
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
            gantt_active_text: "#051423",
            gantt_critical: "#fb7185",
            gantt_critical_text: "#051423",
            series: &[
                "#22d3ee", "#f472b6", "#a3e635", "#facc15", "#c084fc", "#38bdf8", "#fb7185",
                "#2dd4bf",
            ],
            kanban_task_labels: &["#000000"; 8],
        },
    ),
    entry(
        ThemePreset::Ghibli,
        "ghibli",
        "Ghibli",
        false,
        PresetRecipeProfile::Native,
        PresetPalette {
            canvas: "#FDF6E3",
            surface: "#FFFFFF",
            surface_alt: "#FFF3E0",
            text: "#3A2E2C",
            subtle_text: "#6B5B4F",
            border: "#000000",
            line: "#6B5B4F",
            accent: "#FFDFAD",
            edge_label_background: "#FDF6E3",
            cluster_background: "#FFF3E0",
            cluster_border: "#6B5B4F",
            note_background: "#FFF3E0",
            note_border: "#6B5B4F",
            note_text: "#3A2E2C",
            actor_background: "#FFFFFF",
            actor_border: "#6B5B4F",
            actor_text: "#3A2E2C",
            activation_background: "#E8F5E9",
            activation_border: "#6B5B4F",
            sequence_number_text: "#FDF6E3",
            packet_field_label_text: "#3A2E2C",
            gantt_active_text: "#3A2E2C",
            gantt_critical: "#FFDFAD",
            gantt_critical_text: "#3A2E2C",
            series: &["#FFFFFF", "#FFF3E0", "#E8F5E9", "#E6E6E6", "#FFDFAD"],
            kanban_task_labels: &["#000000"; 5],
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
    materialize_theme_design_with_resource_policy(&build_design(preset), resources)
}

/// Builds the editable source; its caller admits the design before export or compilation.
pub(crate) fn build_design(preset: ThemePreset) -> ThemeDesignV1 {
    let entry = entry_for_preset(preset);
    debug_assert!(
        entry.allowed_residual_ids.is_empty(),
        "unqualified alpha presets must not inherit qualification residual allowances"
    );
    let mut design = entry.recipe();
    if entry.profile == PresetRecipeProfile::RetainedMermaidCompatibility {
        // Choose a fallback without claiming derived variables that would shadow typed paint.
        design.mermaid = Some(MermaidThemeCompatibilityWireV1 {
            theme: Some(if entry.dark_mode { "dark" } else { "base" }.to_owned()),
            dark_mode: None,
            variables: None,
        });
    }
    design
}

fn is_discoverable(entry: &PresetCatalogEntry) -> bool {
    entry.retained && entry.required_feature_ids.is_empty() && entry.bundled_resource_ids.is_empty()
}

pub(super) fn build_cross_family_recipe(palette: PresetPalette) -> ThemeDesignV1 {
    let mut design = ThemeDesignV1::default();
    // These are distinct authored roles even when a preset gives them equal initial colors.
    for (id, label, description, value) in [
        (
            "canvas",
            "Canvas",
            "Exported canvas paint and any object fills explicitly bound to it.",
            palette.canvas,
        ),
        (
            "surface",
            "Surface",
            "Main fill for diagram bodies such as nodes, states, and tasks.",
            palette.surface,
        ),
        (
            "surface-alt",
            "Alternate surface",
            "Secondary fill for composite headers and sequence loop labels.",
            palette.surface_alt,
        ),
        (
            "text",
            "Text",
            "Shared text and title color; dedicated label colors remain separate.",
            palette.text,
        ),
        (
            "subtle-text",
            "Subtle text",
            "Secondary text color used by chart legends.",
            palette.subtle_text,
        ),
        (
            "border",
            "Border",
            "Shared outline color for diagram bodies.",
            palette.border,
        ),
        (
            "line",
            "Line",
            "Shared color for connectors, transitions, and chart axes.",
            palette.line,
        ),
        (
            "accent",
            "Accent",
            "Highlight color for special state markers.",
            palette.accent,
        ),
    ] {
        color_token(&mut design, id, label, "Core colors", description, value);
    }
    for (id, label, description, value) in [
        (
            "edge-label-background",
            "Edge label background",
            "Fill behind connector and transition labels.",
            palette.edge_label_background,
        ),
        (
            "cluster-background",
            "Group background",
            "Fill inside grouped diagram bodies.",
            palette.cluster_background,
        ),
        (
            "cluster-border",
            "Group border",
            "Outline around grouped diagram bodies.",
            palette.cluster_border,
        ),
        (
            "note-background",
            "Note background",
            "Fill inside note bodies.",
            palette.note_background,
        ),
        (
            "note-border",
            "Note border",
            "Outline around note bodies.",
            palette.note_border,
        ),
        (
            "note-text",
            "Note text",
            "Text inside note bodies.",
            palette.note_text,
        ),
        (
            "actor-background",
            "Actor background",
            "Fill inside sequence actor bodies.",
            palette.actor_background,
        ),
        (
            "actor-border",
            "Actor border",
            "Sequence actor outlines and lifelines.",
            palette.actor_border,
        ),
        (
            "actor-text",
            "Actor text",
            "Sequence actor labels.",
            palette.actor_text,
        ),
        (
            "activation-background",
            "Activation background",
            "Fill inside sequence activation bars.",
            palette.activation_background,
        ),
        (
            "activation-border",
            "Activation border",
            "Outline around sequence activation bars.",
            palette.activation_border,
        ),
        (
            "sequence-number-text",
            "Sequence number",
            "Text inside sequence message-number markers.",
            palette.sequence_number_text,
        ),
        (
            "packet-field-label-text",
            "Packet field label",
            "Text inside packet fields.",
            palette.packet_field_label_text,
        ),
    ] {
        color_token(
            &mut design,
            id,
            label,
            "Diagram details",
            description,
            value,
        );
    }
    for (index, color) in palette.series.iter().enumerate() {
        color_token(
            &mut design,
            &format!("series.{index}"),
            &format!("Series {}", index + 1),
            "Series palette",
            "One color in the shared ordinal palette; dedicated chart rules may override it.",
            color,
        );
    }
    token(
        &mut design,
        "series",
        "Series palette",
        "Series palette",
        "Ordered colors shared by diagram categories and series.",
        ThemeDesignTokenValueV1::Palette(literal(
            (0..palette.series.len())
                .map(|index| reference(&format!("series.{index}")))
                .collect(),
        )),
    );
    token(
        &mut design,
        "body-font",
        "Font family",
        "Typography",
        "Default font stack, in fallback order; individual styles can override it.",
        ThemeDesignTokenValueV1::FontStack(literal(
            ["Inter", "ui-sans-serif", "system-ui", "sans-serif"]
                .map(str::to_owned)
                .to_vec(),
        )),
    );
    design.typography = Some(ThemeDesignTypographyV1 {
        default: Some(literal(ThemeDesignTextStyleV1 {
            font_stack: SpecifiedWireV1::Value(reference("body-font")),
            ..Default::default()
        })),
        ..Default::default()
    });
    design.canvas = Some(ThemeDesignCanvasV1 {
        base: Some(paint("canvas")),
        ..Default::default()
    });

    // Author the effective defaults directly. Superseded expansion rules and their unused
    // tokens are deliberately absent: every displayed reusable value must have an actual use.
    for (target, fill, stroke) in [
        (ThemeTarget::Text, Some("text"), None),
        (ThemeTarget::Node, None, Some("border")),
        (ThemeTarget::Edge, None, Some("line")),
        (
            ThemeTarget::Cluster,
            Some("cluster-background"),
            Some("cluster-border"),
        ),
        (
            ThemeTarget::Actor,
            Some("actor-background"),
            Some("actor-border"),
        ),
        (
            ThemeTarget::Lifeline,
            Some("actor-border"),
            Some("actor-border"),
        ),
        (ThemeTarget::Message, None, Some("line")),
        (ThemeTarget::State, Some("surface"), Some("border")),
        (ThemeTarget::Transition, None, Some("line")),
        (ThemeTarget::TransitionMarker, Some("line"), Some("line")),
        (
            ThemeTarget::TransitionLabelBackground,
            Some("edge-label-background"),
            None,
        ),
        (ThemeTarget::Composite, Some("canvas"), Some("border")),
        (
            ThemeTarget::CompositeHeader,
            Some("surface-alt"),
            Some("border"),
        ),
        (
            ThemeTarget::Note,
            Some("note-background"),
            Some("note-border"),
        ),
        (
            ThemeTarget::Activation,
            Some("activation-background"),
            Some("activation-border"),
        ),
        (ThemeTarget::Entity, Some("surface"), Some("border")),
        (
            ThemeTarget::EdgeLabelBackground,
            Some("edge-label-background"),
            None,
        ),
        (
            ThemeTarget::LoopLabelBackground,
            Some("surface-alt"),
            Some("border"),
        ),
    ] {
        design
            .styles
            .insert(target.id().to_owned(), style(fill, stroke));
    }
    // Text is a fallback for every label. Role paints must follow it in authored order,
    // independent of the target-name ordering used by the defaults map.
    for (target, fill, stroke) in [
        (ThemeTarget::Title, Some("text"), None),
        (ThemeTarget::ActorLabel, Some("actor-text"), None),
        (ThemeTarget::StateLabel, Some("text"), None),
        (ThemeTarget::TransitionLabel, Some("text"), None),
        (ThemeTarget::CompositeLabel, Some("text"), None),
        (ThemeTarget::NoteLabel, Some("note-text"), None),
        (
            ThemeTarget::SequenceNumberLabel,
            Some("sequence-number-text"),
            None,
        ),
        (ThemeTarget::MessageLabel, Some("text"), None),
        (ThemeTarget::LoopLabel, Some("text"), None),
        (ThemeTarget::PacketByteLabel, Some("text"), None),
        (
            ThemeTarget::PacketFieldLabel,
            Some("packet-field-label-text"),
            None,
        ),
        (ThemeTarget::Axis, Some("text"), Some("line")),
        (ThemeTarget::Legend, Some("subtle-text"), None),
    ] {
        design
            .rules
            .push(rule(None, target, None, style(fill, stroke)));
    }
    design.rules.extend([
        rule(
            None,
            ThemeTarget::SpecialState,
            Some(ThemeVariant::Special),
            style(Some("accent"), Some("accent")),
        ),
        rule(
            None,
            ThemeTarget::SpecialStateInner,
            Some(ThemeVariant::End),
            style(Some("canvas"), Some("canvas")),
        ),
    ]);

    for (target, fill, stroke) in [
        (ThemeTarget::Loop, None, Some("border")),
        (ThemeTarget::SequenceNumberLabel, Some("subtle-text"), None),
    ] {
        design.rules.push(rule(
            Some(DiagramFamilyId::ZENUML),
            target,
            None,
            style(fill, stroke),
        ));
    }
    // Mindmap consumes the ordinal palette; a cross-family Node fill would suppress it.
    for family in [
        DiagramFamilyId::FLOWCHART,
        DiagramFamilyId::C4,
        DiagramFamilyId::SWIMLANE,
        DiagramFamilyId::CLASS,
        DiagramFamilyId::TREE_VIEW,
        DiagramFamilyId::BLOCK,
    ] {
        design.rules.push(rule(
            Some(family),
            ThemeTarget::Node,
            None,
            style(Some("surface"), None),
        ));
    }
    for (family, target, fill, stroke) in [
        (
            DiagramFamilyId::C4,
            ThemeTarget::NodeLabel,
            Some("text"),
            None,
        ),
        (
            DiagramFamilyId::C4,
            ThemeTarget::EdgeLabel,
            Some("text"),
            None,
        ),
        (
            DiagramFamilyId::C4,
            ThemeTarget::ClusterLabel,
            Some("text"),
            None,
        ),
        (
            DiagramFamilyId::ER,
            ThemeTarget::Relation,
            None,
            Some("line"),
        ),
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeTarget::Requirement,
            Some("surface"),
            None,
        ),
        (
            DiagramFamilyId::PIE,
            ThemeTarget::PieSlice,
            None,
            Some("border"),
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::ChartSeries,
            Some("series.0"),
            None,
        ),
    ] {
        design
            .rules
            .push(rule(Some(family), target, None, style(fill, stroke)));
    }
    for (id, value) in [
        ("gantt-active-text", palette.gantt_active_text),
        ("gantt-critical", palette.gantt_critical),
        ("gantt-critical-text", palette.gantt_critical_text),
    ] {
        color_token(
            &mut design,
            id,
            id,
            "Gantt",
            "Authored Gantt task-state paint.",
            value,
        );
    }
    for (variant, fill, stroke, label) in [
        (ThemeVariant::Default, "surface", "border", "text"),
        (
            ThemeVariant::Active,
            "accent",
            "accent",
            "gantt-active-text",
        ),
        (ThemeVariant::Success, "surface-alt", "border", "text"),
        (
            ThemeVariant::Error,
            "gantt-critical",
            "gantt-critical",
            "gantt-critical-text",
        ),
    ] {
        design.rules.push(rule(
            Some(DiagramFamilyId::GANTT),
            ThemeTarget::Task,
            Some(variant),
            style(Some(fill), Some(stroke)),
        ));
        design.rules.push(rule(
            Some(DiagramFamilyId::GANTT),
            ThemeTarget::TaskLabel,
            Some(variant),
            style(Some(label), None),
        ));
    }
    for target in [
        ThemeTarget::Node,
        ThemeTarget::PieSlice,
        ThemeTarget::Task,
        ThemeTarget::ChartSeries,
        ThemeTarget::TimelineEvent,
        ThemeTarget::JourneyTask,
    ] {
        design
            .palettes
            .insert(target.id().to_owned(), reference("series"));
    }
    assert_eq!(
        palette.kanban_task_labels.len(),
        palette.series.len(),
        "each preset series slot must own one Kanban task-label foreground"
    );
    for (index, color) in palette.kanban_task_labels.iter().enumerate() {
        color_token(
            &mut design,
            &format!("task-label.{index}"),
            &format!("Task label {}", index + 1),
            "Diagram details",
            "Kanban task-label text for the corresponding repeating category color.",
            color,
        );
    }
    for terminal_slot in 0usize..12 {
        let mut entry = rule(
            Some(DiagramFamilyId::KANBAN),
            ThemeTarget::TaskLabel,
            None,
            style(
                Some(&format!(
                    "task-label.{}",
                    terminal_slot % palette.kanban_task_labels.len()
                )),
                None,
            ),
        );
        entry.ordinal = Some(ThemeOrdinalSelectorWireV1::Cycle {
            cycle: ThemeOrdinalCycleWireV1 {
                period: 12,
                offset: terminal_slot as u32,
            },
        });
        design.rules.push(entry);
    }
    design
}
