use super::{DiagramThemeSpec, MermaidThemeCompatibility, ThemeTokens};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ThemePreset {
    #[default]
    EditorLight,
    EditorDark,
    OneDark,
    GruvboxLight,
    GruvboxDark,
    AyuLight,
    AyuDark,
}

impl ThemePreset {
    pub const ALL: [Self; 7] = [
        Self::EditorLight,
        Self::EditorDark,
        Self::OneDark,
        Self::GruvboxLight,
        Self::GruvboxDark,
        Self::AyuLight,
        Self::AyuDark,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::EditorLight => "editor-light",
            Self::EditorDark => "editor-dark",
            Self::OneDark => "one-dark",
            Self::GruvboxLight => "gruvbox-light",
            Self::GruvboxDark => "gruvbox-dark",
            Self::AyuLight => "ayu-light",
            Self::AyuDark => "ayu-dark",
        }
    }

    pub const fn is_dark(self) -> bool {
        matches!(
            self,
            Self::EditorDark | Self::OneDark | Self::GruvboxDark | Self::AyuDark
        )
    }

    pub fn from_id(id: &str) -> Result<Self, ThemePresetParseError> {
        Self::ALL
            .into_iter()
            .find(|preset| preset.id() == id)
            .ok_or_else(|| ThemePresetParseError { id: id.to_string() })
    }

    pub(crate) fn spec(self) -> DiagramThemeSpec {
        let palette = preset_palette(self);
        let tokens = ThemeTokens::default()
            .with_canvas(palette.canvas)
            .and_then(|tokens| tokens.with_surface(palette.surface))
            .and_then(|tokens| tokens.with_surface_alt(palette.surface_alt))
            .and_then(|tokens| tokens.with_surface_muted(palette.surface_muted))
            .and_then(|tokens| tokens.with_text(palette.text))
            .and_then(|tokens| tokens.with_subtle_text(palette.subtle_text))
            .and_then(|tokens| tokens.with_border(palette.border))
            .and_then(|tokens| tokens.with_line(palette.line))
            .and_then(|tokens| tokens.with_accent(palette.accent))
            .and_then(|tokens| tokens.with_edge_label_background(palette.edge_label_background))
            .and_then(|tokens| tokens.with_cluster_background(palette.cluster_background))
            .and_then(|tokens| tokens.with_cluster_border(palette.cluster_border))
            .and_then(|tokens| tokens.with_note_background(palette.note_background))
            .and_then(|tokens| tokens.with_note_border(palette.note_border))
            .and_then(|tokens| tokens.with_note_text(palette.note_text))
            .and_then(|tokens| tokens.with_actor_background(palette.actor_background))
            .and_then(|tokens| tokens.with_actor_border(palette.actor_border))
            .and_then(|tokens| tokens.with_actor_text(palette.actor_text))
            .and_then(|tokens| tokens.with_activation_background(palette.activation_background))
            .and_then(|tokens| tokens.with_activation_border(palette.activation_border))
            .and_then(|tokens| tokens.with_error(palette.error))
            .and_then(|tokens| tokens.with_warning(palette.warning))
            .and_then(|tokens| tokens.with_success(palette.success))
            .and_then(|tokens| tokens.with_series(palette.series))
            .expect("built-in theme preset values are statically valid");
        let compatibility = MermaidThemeCompatibility::default()
            .with_theme("base")
            .expect("built-in Mermaid compatibility values are statically valid")
            .with_dark_mode(self.is_dark())
            .expect("built-in Mermaid dark mode is not conflicting");
        tokens
            .into_theme_spec()
            .with_mermaid_compatibility(compatibility)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown diagram theme preset `{id}`")]
pub struct ThemePresetParseError {
    id: String,
}

impl ThemePresetParseError {
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePresetDescriptor {
    preset: ThemePreset,
}

impl ThemePresetDescriptor {
    pub const fn id(self) -> &'static str {
        self.preset.id()
    }

    pub const fn is_dark(self) -> bool {
        self.preset.is_dark()
    }

    pub const fn preset(self) -> ThemePreset {
        self.preset
    }
}

const THEME_PRESET_DESCRIPTORS: [ThemePresetDescriptor; 7] = [
    ThemePresetDescriptor {
        preset: ThemePreset::EditorLight,
    },
    ThemePresetDescriptor {
        preset: ThemePreset::EditorDark,
    },
    ThemePresetDescriptor {
        preset: ThemePreset::OneDark,
    },
    ThemePresetDescriptor {
        preset: ThemePreset::GruvboxLight,
    },
    ThemePresetDescriptor {
        preset: ThemePreset::GruvboxDark,
    },
    ThemePresetDescriptor {
        preset: ThemePreset::AyuLight,
    },
    ThemePresetDescriptor {
        preset: ThemePreset::AyuDark,
    },
];

pub const fn theme_preset_descriptors() -> &'static [ThemePresetDescriptor] {
    &THEME_PRESET_DESCRIPTORS
}

struct PresetPalette {
    canvas: &'static str,
    surface: &'static str,
    surface_alt: &'static str,
    surface_muted: &'static str,
    text: &'static str,
    subtle_text: &'static str,
    border: &'static str,
    line: &'static str,
    accent: &'static str,
    edge_label_background: &'static str,
    cluster_background: &'static str,
    cluster_border: &'static str,
    note_background: &'static str,
    note_border: &'static str,
    note_text: &'static str,
    actor_background: &'static str,
    actor_border: &'static str,
    actor_text: &'static str,
    activation_background: &'static str,
    activation_border: &'static str,
    error: &'static str,
    warning: &'static str,
    success: &'static str,
    series: &'static [&'static str],
}

fn preset_palette(preset: ThemePreset) -> PresetPalette {
    match preset {
        ThemePreset::EditorLight => PresetPalette {
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
            error: "#dc2626",
            warning: "#d97706",
            success: "#059669",
            series: &[
                "#2563eb", "#059669", "#d97706", "#7c3aed", "#0891b2", "#be123c", "#a16207",
                "#65a30d",
            ],
        },
        ThemePreset::EditorDark => PresetPalette {
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
            error: "#f87171",
            warning: "#fbbf24",
            success: "#34d399",
            series: &[
                "#60a5fa", "#34d399", "#f59e0b", "#c084fc", "#22d3ee", "#fb7185", "#facc15",
                "#a3e635",
            ],
        },
        ThemePreset::OneDark => PresetPalette {
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
            error: "#e06c75",
            warning: "#e5c07b",
            success: "#98c379",
            series: &[
                "#61afef", "#98c379", "#e5c07b", "#c678dd", "#56b6c2", "#e06c75", "#d19a66",
                "#be5046",
            ],
        },
        ThemePreset::GruvboxLight => PresetPalette {
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
            error: "#cc241d",
            warning: "#d79921",
            success: "#98971a",
            series: &[
                "#458588", "#98971a", "#d79921", "#b16286", "#689d6a", "#cc241d", "#d65d0e",
                "#427b58",
            ],
        },
        ThemePreset::GruvboxDark => PresetPalette {
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
            error: "#fb4934",
            warning: "#fabd2f",
            success: "#b8bb26",
            series: &[
                "#83a598", "#b8bb26", "#fabd2f", "#d3869b", "#8ec07c", "#fb4934", "#fe8019",
                "#689d6a",
            ],
        },
        ThemePreset::AyuLight => PresetPalette {
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
            error: "#f07171",
            warning: "#ffaa33",
            success: "#86b300",
            series: &[
                "#55b4d4", "#86b300", "#ffaa33", "#a37acc", "#4cbf99", "#f07171", "#f2ae49",
                "#399ee6",
            ],
        },
        ThemePreset::AyuDark => PresetPalette {
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
            error: "#f07178",
            warning: "#ffb454",
            success: "#aad94c",
            series: &[
                "#59c2ff", "#aad94c", "#ffb454", "#d2a6ff", "#95e6cb", "#f07178", "#ff8f40",
                "#e6b673",
            ],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, ResolvedDiagramTheme, ThemeColorValue, ThemeTarget,
        ThemeVariant,
    };
    use crate::render_family::RenderFamilyKind;

    fn solid_color(paint: &CanvasPaint) -> Option<String> {
        match paint {
            CanvasPaint::Solid(color) => Some(color.as_css()),
            _ => None,
        }
    }

    fn compiled(preset: ThemePreset) -> (crate::diagram_theme::DiagramTheme, ResolvedDiagramTheme) {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset)
            .expect("built-in theme preset should compile");
        let resolved = theme.resolve(RenderFamilyKind::Flowchart);
        (theme, resolved)
    }

    #[test]
    fn built_in_presets_keep_mermaid_compatibility_without_layout_or_look() {
        for preset in ThemePreset::ALL {
            let (theme, _) = compiled(preset);
            let config = theme.spec().mermaid().to_mermaid_config();

            assert_eq!(config.get_str("theme"), Some("base"));
            assert_eq!(
                config.get_bool("darkMode"),
                Some(preset.is_dark()),
                "{} dark-mode compatibility value",
                preset.id()
            );
            assert_eq!(config.get_str("look"), None);
            assert_eq!(config.get_str("flowchart.defaultRenderer"), None);
        }
    }

    #[test]
    fn built_in_presets_publish_stable_semantic_representatives() {
        let expected = [
            (ThemePreset::EditorLight, "#ffffff", "#64748b", "#2563eb"),
            (ThemePreset::EditorDark, "#0f172a", "#94a3b8", "#60a5fa"),
            (ThemePreset::OneDark, "#282c34", "#61afef", "#61afef"),
            (ThemePreset::GruvboxLight, "#fbf1c7", "#7c6f64", "#458588"),
            (ThemePreset::GruvboxDark, "#282828", "#d5c4a1", "#83a598"),
            (ThemePreset::AyuLight, "#fcfcfc", "#5c6166", "#55b4d4"),
            (ThemePreset::AyuDark, "#0b0e14", "#59c2ff", "#59c2ff"),
        ];

        for (preset, canvas, line, first_series) in expected {
            let (theme, flowchart) = compiled(preset);
            assert_eq!(
                solid_color(theme.spec().canvas().base()).as_deref(),
                Some(canvas)
            );
            assert_eq!(
                flowchart
                    .style(ThemeTarget::Edge, ThemeVariant::Default, None)
                    .stroke()
                    .and_then(solid_color)
                    .as_deref(),
                Some(line)
            );
            let chart = theme.resolve(RenderFamilyKind::XyChart);
            assert_eq!(
                chart
                    .series_color(ThemeTarget::ChartSeries, 1)
                    .map(ThemeColorValue::as_css)
                    .as_deref(),
                Some(first_series)
            );
            let mindmap = theme.resolve(RenderFamilyKind::Mindmap);
            assert_eq!(
                mindmap
                    .series_color(ThemeTarget::Node, 1)
                    .map(ThemeColorValue::as_css)
                    .as_deref(),
                Some(first_series)
            );
            let kanban = theme.resolve(RenderFamilyKind::Kanban);
            assert_eq!(
                kanban
                    .series_color(ThemeTarget::Task, 1)
                    .map(ThemeColorValue::as_css)
                    .as_deref(),
                Some(first_series)
            );
        }
    }
}
