use crate::color::{AsciiColorMode, AsciiColorTheme};
use crate::error::{AsciiError, Result};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsciiCharset {
    #[default]
    Unicode,
    Ascii,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsciiDirection {
    #[default]
    LeftRight,
    TopDown,
}

/// Terminal display-width convention used by every text measurement and placement operation.
///
/// `Unicode` follows the non-CJK width table exposed by the pinned `unicode-width` dependency.
/// `Cjk` additionally treats East Asian ambiguous characters as wide. Hosts should select the
/// profile that matches the terminal in which the rendered text will be displayed.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerminalWidthProfile {
    #[default]
    Unicode,
    Cjk,
}

/// ASCII 布局密度配置。Canonical 保持现有默认几何；Compact 仅作为显式选择的候选配置。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsciiLayoutProfile {
    #[default]
    Canonical,
    Compact,
}

impl AsciiLayoutProfile {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::Compact => "compact",
        }
    }
}

impl TerminalWidthProfile {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unicode => "unicode",
            Self::Cjk => "cjk",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiRenderOptions {
    pub charset: AsciiCharset,
    pub terminal_width_profile: TerminalWidthProfile,
    pub layout_profile: AsciiLayoutProfile,
    pub default_direction: AsciiDirection,
    pub color_mode: AsciiColorMode,
    pub color_theme: AsciiColorTheme,
    pub box_border_padding: usize,
    pub graph_padding_x: usize,
    pub graph_padding_y: usize,
    pub flowchart_node_label_wrap_width: usize,
    pub sequence_participant_spacing: usize,
    pub sequence_message_spacing: usize,
    pub sequence_self_message_width: usize,
    pub sequence_mirror_actors: bool,
    pub xychart_vertical_plot_height: usize,
    pub xychart_category_band_width: usize,
    pub xychart_horizontal_plot_width: usize,
    pub relation_summary_diagnostics: bool,
    pub(crate) layout_overrides: u8,
}

/// Host facts and explicit presentation choices resolved before family rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AsciiHostPolicy {
    pub charset: AsciiCharset,
    pub structural_charset: AsciiCharset,
    pub terminal_width_profile: TerminalWidthProfile,
    pub color_mode: AsciiColorMode,
    pub color_theme: AsciiColorTheme,
}

/// Output facts shared by measurement, viewport admission, and encoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AsciiOutputPolicy {
    pub terminal_width_profile: TerminalWidthProfile,
    pub color_mode: AsciiColorMode,
}

/// Flowchart-owned projection and geometry policy. The family boundary reduces this view to a
/// graph-scene policy only after Flowchart semantics have been projected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FlowchartLayoutPolicy {
    /// Padding inside a node frame, measured in terminal cells.
    pub node_border_padding: usize,
    /// Gap between ranked graph columns and rows.
    pub rank_gap_x: usize,
    pub rank_gap_y: usize,
    pub node_label_wrap_width: usize,
    /// Horizontal and vertical clearance around compound subgraph members.
    pub group_padding_x: usize,
    pub group_padding_y: usize,
    /// Extra rows reserved for a compound group title before its members.
    pub group_title_clearance: usize,
    /// Bounded search radius used when moving edge labels away from route cells.
    pub edge_label_lane_radius: usize,
    pub default_direction: AsciiDirection,
    pub structural_charset: AsciiCharset,
    pub terminal_width_profile: TerminalWidthProfile,
}

impl FlowchartLayoutPolicy {
    pub(crate) const DEFAULT_EDGE_LABEL_LANE_RADIUS: usize =
        GraphLayoutPolicy::DEFAULT_EDGE_LABEL_LANE_RADIUS;

    pub(crate) const fn graph_policy(self) -> GraphLayoutPolicy {
        GraphLayoutPolicy {
            node_border_padding: self.node_border_padding,
            rank_gap_x: self.rank_gap_x,
            rank_gap_y: self.rank_gap_y,
            group_padding_x: self.group_padding_x,
            group_padding_y: self.group_padding_y,
            group_title_clearance: self.group_title_clearance,
            edge_label_lane_radius: self.edge_label_lane_radius,
            structural_charset: self.structural_charset,
            terminal_width_profile: self.terminal_width_profile,
        }
    }
}

/// Family-neutral graph-scene geometry consumed only after semantic projection. Flowchart and
/// State construct this policy independently so profile experiments cannot cross family bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphLayoutPolicy {
    pub node_border_padding: usize,
    pub rank_gap_x: usize,
    pub rank_gap_y: usize,
    pub group_padding_x: usize,
    pub group_padding_y: usize,
    pub group_title_clearance: usize,
    pub edge_label_lane_radius: usize,
    pub structural_charset: AsciiCharset,
    pub terminal_width_profile: TerminalWidthProfile,
}

impl GraphLayoutPolicy {
    pub(crate) const DEFAULT_EDGE_LABEL_LANE_RADIUS: usize = 4;
}

/// Sequence-owned geometry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SequenceLayoutPolicy {
    pub participant_label_wrap_width: usize,
    pub participant_spacing: usize,
    pub message_spacing: usize,
    pub message_label_left_margin: usize,
    pub message_label_overflow_buffer: usize,
    pub self_message_width: usize,
    pub note_side_gutter: usize,
    pub note_wrap_width: usize,
    pub box_content_gutter: usize,
    pub section_title_gutter: usize,
    pub section_title_wrap_width: usize,
    pub control_participant_gutter: usize,
    pub control_content_gutter: usize,
    pub control_nested_gutter: usize,
    pub control_depth_gutter: usize,
    pub title_bottom_spacing: usize,
    pub mirror_actors: bool,
    pub structural_charset: AsciiCharset,
    pub terminal_width_profile: TerminalWidthProfile,
}

/// XYChart-owned geometry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct XyChartLayoutPolicy {
    pub vertical_plot_height: usize,
    pub category_band_width: usize,
    pub horizontal_plot_width: usize,
    pub structural_charset: AsciiCharset,
    pub terminal_width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AsciiLayoutPolicies {
    pub profile: AsciiLayoutProfile,
    pub flowchart: FlowchartLayoutPolicy,
    pub state: GraphLayoutPolicy,
    pub sequence: SequenceLayoutPolicy,
    pub xychart: XyChartLayoutPolicy,
}

/// The one internal policy resolution seam. The public options record remains a compatibility
/// façade while family modules consume only the resolved view they own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedAsciiPolicies {
    pub options: AsciiRenderOptions,
    pub host: AsciiHostPolicy,
    pub layout: AsciiLayoutPolicies,
    pub output: AsciiOutputPolicy,
}

const OVERRIDE_GRAPH_PADDING_X: u8 = 1 << 0;
const OVERRIDE_GRAPH_PADDING_Y: u8 = 1 << 1;
const OVERRIDE_FLOWCHART_WRAP_WIDTH: u8 = 1 << 2;
const OVERRIDE_SEQUENCE_PARTICIPANT_SPACING: u8 = 1 << 3;

impl Default for AsciiRenderOptions {
    fn default() -> Self {
        Self {
            charset: AsciiCharset::Unicode,
            terminal_width_profile: TerminalWidthProfile::Unicode,
            layout_profile: AsciiLayoutProfile::Canonical,
            default_direction: AsciiDirection::LeftRight,
            color_mode: AsciiColorMode::Plain,
            color_theme: AsciiColorTheme::default_light(),
            box_border_padding: 1,
            graph_padding_x: 5,
            graph_padding_y: 5,
            flowchart_node_label_wrap_width: 40,
            sequence_participant_spacing: 5,
            sequence_message_spacing: 1,
            sequence_self_message_width: 4,
            sequence_mirror_actors: false,
            xychart_vertical_plot_height: 5,
            xychart_category_band_width: 3,
            xychart_horizontal_plot_width: 10,
            relation_summary_diagnostics: false,
            layout_overrides: 0,
        }
    }
}

impl AsciiRenderOptions {
    pub fn ascii() -> Self {
        Self {
            charset: AsciiCharset::Ascii,
            ..Self::default()
        }
    }

    pub fn unicode() -> Self {
        Self::default()
    }

    pub fn with_color_mode(mut self, color_mode: AsciiColorMode) -> Self {
        self.color_mode = color_mode;
        self
    }

    pub fn with_terminal_width_profile(mut self, profile: TerminalWidthProfile) -> Self {
        self.terminal_width_profile = profile;
        self
    }

    #[must_use]
    pub fn with_layout_profile(mut self, profile: AsciiLayoutProfile) -> Self {
        self.layout_profile = profile;
        self
    }

    #[must_use]
    pub fn with_graph_padding_x(mut self, padding: usize) -> Self {
        self.graph_padding_x = padding;
        self.layout_overrides |= OVERRIDE_GRAPH_PADDING_X;
        self
    }

    #[must_use]
    pub fn with_graph_padding_y(mut self, padding: usize) -> Self {
        self.graph_padding_y = padding;
        self.layout_overrides |= OVERRIDE_GRAPH_PADDING_Y;
        self
    }

    pub fn with_color_theme(mut self, color_theme: AsciiColorTheme) -> Self {
        self.color_theme = color_theme;
        self
    }

    pub fn with_sequence_mirror_actors(mut self, mirror_actors: bool) -> Self {
        self.sequence_mirror_actors = mirror_actors;
        self
    }

    pub fn with_flowchart_node_label_wrap_width(mut self, width: usize) -> Self {
        self.flowchart_node_label_wrap_width = width;
        self.layout_overrides |= OVERRIDE_FLOWCHART_WRAP_WIDTH;
        self
    }

    #[must_use]
    pub fn with_sequence_participant_spacing(mut self, spacing: usize) -> Self {
        self.sequence_participant_spacing = spacing;
        self.layout_overrides |= OVERRIDE_SEQUENCE_PARTICIPANT_SPACING;
        self
    }

    pub fn with_xychart_vertical_plot_height(mut self, height: usize) -> Self {
        self.xychart_vertical_plot_height = height;
        self
    }

    pub fn with_xychart_category_band_width(mut self, width: usize) -> Self {
        self.xychart_category_band_width = width;
        self
    }

    pub fn with_xychart_horizontal_plot_width(mut self, width: usize) -> Self {
        self.xychart_horizontal_plot_width = width;
        self
    }

    pub fn with_relation_summary_diagnostics(mut self, enabled: bool) -> Self {
        self.relation_summary_diagnostics = enabled;
        self
    }

    /// Applies the selected layout profile while preserving explicit caller overrides.
    pub(crate) fn effective_layout(self) -> Self {
        if self.layout_profile != AsciiLayoutProfile::Compact {
            return self;
        }
        Self {
            flowchart_node_label_wrap_width: if self.layout_overrides
                & OVERRIDE_FLOWCHART_WRAP_WIDTH
                == 0
            {
                24
            } else {
                self.flowchart_node_label_wrap_width
            },
            sequence_participant_spacing: if self.layout_overrides
                & OVERRIDE_SEQUENCE_PARTICIPANT_SPACING
                == 0
            {
                3
            } else {
                self.sequence_participant_spacing
            },
            layout_overrides: self.layout_overrides,
            ..self
        }
    }

    pub(crate) fn resolve_policies(self) -> ResolvedAsciiPolicies {
        let requested_options = self;
        let options = self.effective_layout();
        let structural_charset = options.structural_charset();
        ResolvedAsciiPolicies {
            options,
            host: AsciiHostPolicy {
                charset: options.charset,
                structural_charset,
                terminal_width_profile: options.terminal_width_profile,
                color_mode: options.color_mode,
                color_theme: options.color_theme,
            },
            layout: AsciiLayoutPolicies {
                profile: options.layout_profile,
                flowchart: FlowchartLayoutPolicy {
                    node_border_padding: options.box_border_padding,
                    rank_gap_x: options.graph_padding_x,
                    rank_gap_y: options.graph_padding_y,
                    node_label_wrap_width: options.flowchart_node_label_wrap_width,
                    group_padding_x: 2,
                    group_padding_y: 2,
                    group_title_clearance: 3,
                    edge_label_lane_radius: FlowchartLayoutPolicy::DEFAULT_EDGE_LABEL_LANE_RADIUS,
                    default_direction: options.default_direction,
                    structural_charset,
                    terminal_width_profile: options.terminal_width_profile,
                },
                state: graph_layout_policy(requested_options),
                sequence: SequenceLayoutPolicy {
                    participant_label_wrap_width: 12,
                    participant_spacing: options.sequence_participant_spacing,
                    message_spacing: options.sequence_message_spacing,
                    message_label_left_margin: 2,
                    message_label_overflow_buffer: 10,
                    self_message_width: options.sequence_self_message_width,
                    note_side_gutter: 2,
                    note_wrap_width: 24,
                    box_content_gutter: 2,
                    section_title_gutter: 2,
                    section_title_wrap_width: 12,
                    control_participant_gutter: 2,
                    control_content_gutter: 1,
                    control_nested_gutter: 2,
                    control_depth_gutter: 2,
                    title_bottom_spacing: 0,
                    mirror_actors: options.sequence_mirror_actors,
                    structural_charset,
                    terminal_width_profile: options.terminal_width_profile,
                },
                xychart: XyChartLayoutPolicy {
                    vertical_plot_height: options.xychart_vertical_plot_height,
                    category_band_width: options.xychart_category_band_width,
                    horizontal_plot_width: options.xychart_horizontal_plot_width,
                    structural_charset,
                    terminal_width_profile: options.terminal_width_profile,
                },
            },
            output: AsciiOutputPolicy {
                terminal_width_profile: options.terminal_width_profile,
                color_mode: options.color_mode,
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn flowchart_layout(self) -> FlowchartLayoutPolicy {
        self.resolve_policies().layout.flowchart
    }

    #[cfg(test)]
    pub(crate) fn sequence_layout(self) -> SequenceLayoutPolicy {
        self.resolve_policies().layout.sequence
    }

    pub(crate) fn xychart_layout(self) -> XyChartLayoutPolicy {
        self.resolve_policies().layout.xychart
    }

    /// Returns the structural glyph set that can preserve one-cell grid topology.
    ///
    /// Unicode box-drawing and marker characters are East Asian Ambiguous. The CJK width table
    /// assigns them two cells, while every current planner models a structural token as one cell.
    /// Falling back to ASCII structure keeps authored CJK/ambiguous text profile-correct without
    /// corrupting borders, routes, or alignment.
    pub(crate) fn structural_charset(&self) -> AsciiCharset {
        match self.terminal_width_profile {
            TerminalWidthProfile::Unicode => self.charset,
            TerminalWidthProfile::Cjk => AsciiCharset::Ascii,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.flowchart_node_label_wrap_width == 0 {
            return Err(AsciiError::InvalidOption {
                field: "flowchart_node_label_wrap_width",
                message: "must be greater than 0",
            });
        }

        if self.sequence_self_message_width < 2 {
            return Err(AsciiError::InvalidOption {
                field: "sequence_self_message_width",
                message: "must be at least 2",
            });
        }

        if self.xychart_vertical_plot_height < 2 {
            return Err(AsciiError::InvalidOption {
                field: "xychart_vertical_plot_height",
                message: "must be at least 2",
            });
        }

        if self.xychart_category_band_width == 0 {
            return Err(AsciiError::InvalidOption {
                field: "xychart_category_band_width",
                message: "must be greater than 0",
            });
        }

        if self.xychart_horizontal_plot_width < 2 {
            return Err(AsciiError::InvalidOption {
                field: "xychart_horizontal_plot_width",
                message: "must be at least 2",
            });
        }

        Ok(())
    }
}

fn graph_layout_policy(options: AsciiRenderOptions) -> GraphLayoutPolicy {
    GraphLayoutPolicy {
        node_border_padding: options.box_border_padding,
        rank_gap_x: options.graph_padding_x,
        rank_gap_y: options.graph_padding_y,
        group_padding_x: 2,
        group_padding_y: 2,
        group_title_clearance: 3,
        edge_label_lane_radius: GraphLayoutPolicy::DEFAULT_EDGE_LABEL_LANE_RADIUS,
        structural_charset: options.structural_charset(),
        terminal_width_profile: options.terminal_width_profile,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_profile_uses_single_cell_ascii_structure() {
        let options =
            AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk);

        assert_eq!(options.structural_charset(), AsciiCharset::Ascii);
    }

    #[test]
    fn resolved_compact_policy_changes_only_admitted_family_fields() {
        let canonical = AsciiRenderOptions::unicode().resolve_policies();
        let compact = AsciiRenderOptions::unicode()
            .with_layout_profile(AsciiLayoutProfile::Compact)
            .resolve_policies();

        assert_eq!(canonical.host, compact.host);
        assert_eq!(canonical.output, compact.output);
        assert_eq!(
            canonical.layout.flowchart.rank_gap_x,
            compact.layout.flowchart.rank_gap_x
        );
        assert_eq!(
            canonical.layout.flowchart.rank_gap_y,
            compact.layout.flowchart.rank_gap_y
        );
        assert_ne!(
            canonical.layout.flowchart.node_label_wrap_width,
            compact.layout.flowchart.node_label_wrap_width
        );
        assert_eq!(
            canonical.layout.flowchart.node_border_padding,
            compact.layout.flowchart.node_border_padding
        );
        assert_eq!(
            canonical.layout.flowchart.group_padding_x,
            compact.layout.flowchart.group_padding_x
        );
        assert_eq!(
            canonical.layout.flowchart.group_padding_y,
            compact.layout.flowchart.group_padding_y
        );
        assert_eq!(
            canonical.layout.flowchart.group_title_clearance,
            compact.layout.flowchart.group_title_clearance
        );
        assert_eq!(
            canonical.layout.flowchart.edge_label_lane_radius,
            compact.layout.flowchart.edge_label_lane_radius
        );
        assert_ne!(
            canonical.layout.sequence.participant_spacing,
            compact.layout.sequence.participant_spacing
        );
        assert_eq!(
            canonical.layout.sequence.message_spacing,
            compact.layout.sequence.message_spacing
        );
        assert_eq!(
            canonical.layout.sequence.participant_label_wrap_width,
            compact.layout.sequence.participant_label_wrap_width
        );
        assert_eq!(
            canonical.layout.sequence.message_label_left_margin,
            compact.layout.sequence.message_label_left_margin
        );
        assert_eq!(
            canonical.layout.sequence.message_label_overflow_buffer,
            compact.layout.sequence.message_label_overflow_buffer
        );
        assert_eq!(
            canonical.layout.sequence.self_message_width,
            compact.layout.sequence.self_message_width
        );
        assert_eq!(
            canonical.layout.sequence.note_side_gutter,
            compact.layout.sequence.note_side_gutter
        );
        assert_eq!(
            canonical.layout.sequence.note_wrap_width,
            compact.layout.sequence.note_wrap_width
        );
        assert_eq!(
            canonical.layout.sequence.box_content_gutter,
            compact.layout.sequence.box_content_gutter
        );
        assert_eq!(
            canonical.layout.sequence.section_title_gutter,
            compact.layout.sequence.section_title_gutter
        );
        assert_eq!(
            canonical.layout.sequence.section_title_wrap_width,
            compact.layout.sequence.section_title_wrap_width
        );
        assert_eq!(
            canonical.layout.sequence.control_participant_gutter,
            compact.layout.sequence.control_participant_gutter
        );
        assert_eq!(
            canonical.layout.sequence.control_content_gutter,
            compact.layout.sequence.control_content_gutter
        );
        assert_eq!(
            canonical.layout.sequence.control_nested_gutter,
            compact.layout.sequence.control_nested_gutter
        );
        assert_eq!(
            canonical.layout.sequence.control_depth_gutter,
            compact.layout.sequence.control_depth_gutter
        );
        assert_eq!(
            canonical.layout.sequence.title_bottom_spacing,
            compact.layout.sequence.title_bottom_spacing
        );
        assert_eq!(canonical.layout.state, compact.layout.state);
        assert_eq!(canonical.layout.xychart, compact.layout.xychart);
    }

    #[test]
    fn explicit_sequence_spacing_override_wins_over_compact_profile() {
        let policies = AsciiRenderOptions::unicode()
            .with_layout_profile(AsciiLayoutProfile::Compact)
            .with_sequence_participant_spacing(7)
            .resolve_policies();

        assert_eq!(policies.layout.sequence.participant_spacing, 7);
    }

    #[test]
    fn resolved_policy_carries_one_structural_charset_to_every_family() {
        let policies = AsciiRenderOptions::unicode()
            .with_terminal_width_profile(TerminalWidthProfile::Cjk)
            .resolve_policies();

        assert_eq!(policies.host.structural_charset, AsciiCharset::Ascii);
        assert_eq!(
            policies.layout.flowchart.structural_charset,
            AsciiCharset::Ascii
        );
        assert_eq!(
            policies.layout.sequence.structural_charset,
            AsciiCharset::Ascii
        );
        assert_eq!(
            policies.layout.state.structural_charset,
            AsciiCharset::Ascii
        );
        assert_eq!(
            policies.layout.xychart.structural_charset,
            AsciiCharset::Ascii
        );
    }
}
