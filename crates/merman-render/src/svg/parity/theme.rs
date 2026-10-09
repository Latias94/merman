use super::util::SvgTheme;
use serde_json::Value;

mod families;
mod helpers;

use helpers::*;

#[derive(Debug, Clone)]
pub(super) struct CommonCssTheme {
    pub(super) theme_name: String,
    pub(super) dark_mode: bool,
    pub(super) look: String,
    pub(super) font_family_css: String,
    pub(super) text_color: String,
    pub(super) line_color: String,
    pub(super) error_bkg: String,
    pub(super) error_text: String,
}

impl CommonCssTheme {
    pub(super) fn is_dark_theme(&self) -> bool {
        self.dark_mode
    }

    pub(super) fn is_neo(&self) -> bool {
        self.look == "neo"
    }
}

#[derive(Debug, Clone)]
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-mindmap",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-block"
))]
pub(super) struct NodeDiagramTheme {
    pub(super) common: CommonCssTheme,
    pub(super) node_text_color: String,
    pub(super) title_color: String,
    pub(super) main_bkg: String,
    pub(super) node_border: String,
    pub(super) arrowhead_color: String,
    pub(super) stroke_width: String,
    pub(super) edge_label_background: String,
    pub(super) tertiary: String,
    pub(super) cluster_bkg: String,
    pub(super) cluster_border: String,
}

#[derive(Debug, Clone)]
#[cfg(feature = "diagram-sequence")]
pub(super) struct SequenceDiagramTheme {
    pub(super) common: CommonCssTheme,
    pub(super) actor_border: String,
    pub(super) actor_fill: String,
    pub(super) stroke_width: String,
    pub(super) drop_shadow: String,
    pub(super) note_border: String,
    pub(super) note_fill: String,
    pub(super) actor_text: String,
    pub(super) actor_line: String,
    pub(super) signal_color: String,
    pub(super) sequence_number: String,
    pub(super) signal_text: String,
    pub(super) label_box_border: String,
    pub(super) label_box_fill: String,
    pub(super) label_text: String,
    pub(super) loop_text: String,
    pub(super) note_text: String,
    pub(super) activation_fill: String,
    pub(super) activation_border: String,
    pub(super) node_border: String,
    pub(super) label_box_filter: String,
}

#[cfg(feature = "diagram-sequence")]
pub(crate) fn sequence_text_surface_fills(
    effective_config: &serde_json::Value,
) -> [String; crate::sequence::SequenceTextSurface::COUNT] {
    let theme = MermaidThemeAdapter::new(effective_config).sequence_diagram();
    [
        theme.actor_text,
        theme.common.text_color,
        theme.signal_text,
        theme.note_text,
        theme.label_text,
        theme.loop_text.clone(),
        theme.loop_text,
    ]
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn flowchart_text_surface_fills(effective_config: &Value) -> [String; 2] {
    let theme = MermaidThemeAdapter::new(effective_config).node_diagram();
    [theme.node_text_color, theme.title_color]
}

#[derive(Debug, Clone)]
#[cfg(feature = "diagram-state")]
pub(super) struct StateDiagramTheme {
    pub(super) common: CommonCssTheme,
    pub(super) transition_color: String,
    pub(super) node_border: String,
    pub(super) background: String,
    pub(super) main_bkg: String,
    pub(super) alt_background: String,
    pub(super) stroke_width: String,
    pub(super) stroke_width_px: String,
    pub(super) rough_stroke_width_value: f64,
    pub(super) note_border: String,
    pub(super) note_bkg: String,
    pub(super) note_text: String,
    pub(super) label_background: String,
    pub(super) edge_label_background: String,
    pub(super) transition_label_color: String,
    pub(super) special_state_color: String,
    pub(super) inner_end_background: String,
    pub(super) composite_background: String,
    pub(super) state_bkg: String,
    pub(super) state_border: String,
    pub(super) composite_title_background: String,
    pub(super) state_label_color: String,
    pub(super) drop_shadow: String,
}

#[cfg(feature = "diagram-radar")]
pub(crate) fn radar_default_series_color(index: usize) -> &'static str {
    default_c_scale(index)
}

#[cfg(feature = "diagram-timeline")]
pub(crate) fn timeline_default_section_colors(index: usize) -> [&'static str; 3] {
    [
        default_c_scale(index),
        default_c_scale_label(index),
        default_c_scale_inv(index),
    ]
}

#[cfg(feature = "diagram-kanban")]
pub(crate) fn kanban_section_defaults(index: usize) -> [&'static str; 3] {
    [
        default_c_scale(index),
        default_c_scale_label(index),
        default_c_scale_inv(index),
    ]
}

pub(crate) struct MermaidThemeAdapter<'a> {
    raw: SvgTheme<'a>,
    common: CommonCssTheme,
}

impl<'a> MermaidThemeAdapter<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        let raw = SvgTheme::new(effective_config);
        let theme_name = raw.theme_name();
        let common = CommonCssTheme {
            dark_mode: raw
                .bool_root_or_theme("darkMode")
                .unwrap_or_else(|| theme_name.contains("dark")),
            theme_name,
            look: raw.look(),
            font_family_css: raw.font_family_css(),
            text_color: raw.color("textColor", "#333"),
            line_color: raw.color("lineColor", "#333333"),
            error_bkg: raw.color("errorBkgColor", "#552222"),
            error_text: raw.color("errorTextColor", "#552222"),
        };

        Self { raw, common }
    }
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
