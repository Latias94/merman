//! Native theme selection and visual settings used by family renderers.

use std::sync::Arc;

pub(crate) mod canvas_svg;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "diagram-state",
    feature = "diagram-swimlane"
))]
pub(crate) mod edge_label_bg;
#[cfg(any(
    all(
        feature = "diagram-architecture",
        any(feature = "layout-cytoscape", feature = "layout-shoal")
    ),
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "diagram-swimlane",
    feature = "diagram-sequence",
    feature = "diagram-zenuml",
    feature = "diagram-kanban",
    feature = "diagram-block",
    feature = "diagram-xychart",
    feature = "diagram-usecase",
    feature = "diagram-state",
))]
pub(crate) mod elevation_svg;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "diagram-swimlane",
    feature = "diagram-block",
    feature = "diagram-state",
))]
pub(crate) mod marker_svg;
#[cfg(any(
    feature = "diagram-pie",
    feature = "diagram-treemap",
    feature = "diagram-xychart"
))]
pub(crate) mod pattern_svg;

pub use merman_theme_contract::v2::{
    Appearance, BuiltinPreset, Color, FamilyTheme, ResolvedThemeV2, ThemeCapabilityFlags,
    ThemeCompiler, ThemeDocumentV2, ThemeError, ThemeNotice,
};

/// Whether rendering requires portable output for the selected target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemePortabilityRequirement {
    #[default]
    BestEffort,
    RequirePortable,
}

/// Immutable native theme and an optional host canvas color.
#[derive(Clone, Debug)]
pub struct NativeTheme {
    resolved: Arc<ResolvedThemeV2>,
    background: Option<Color>,
}

impl NativeTheme {
    /// Uses the theme's resolved canvas paint and appearance.
    pub fn new(resolved: Arc<ResolvedThemeV2>) -> Self {
        Self {
            resolved,
            background: None,
        }
    }

    /// Replaces the canvas base paint while retaining its textures.
    pub fn with_background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    /// Returns the shared theme data.
    pub fn resolved(&self) -> &ResolvedThemeV2 {
        &self.resolved
    }

    /// Returns the explicitly selected base color, if supplied.
    pub const fn background(&self) -> Option<Color> {
        self.background
    }
}

/// Connects a diagram family's measurement and rendering to its native theme values.
pub trait FamilyThemeAdapter {
    /// Logical family that consumes the values.
    const FAMILY: crate::DiagramFamilyId;
    /// Visual capabilities implemented by this family.
    const CAPS: ThemeCapabilityFlags;

    /// Returns the family-specific settings or shared theme defaults.
    fn resolve(theme: &ResolvedThemeV2) -> &FamilyTheme {
        theme.family(Self::FAMILY.as_str())
    }
}

/// Returns the native theme capabilities implemented by this build for a logical family.
pub fn family_capabilities(family: crate::DiagramFamilyId) -> ThemeCapabilityFlags {
    match family {
        #[cfg(feature = "diagram-flowchart")]
        crate::DiagramFamilyId::FLOWCHART => {
            crate::flowchart::native_theme::FlowchartThemeAdapter::CAPS
        }
        #[cfg(feature = "diagram-agentflow")]
        crate::DiagramFamilyId::AGENTFLOW => {
            crate::flowchart::native_theme::AgentflowThemeAdapter::CAPS
        }
        #[cfg(feature = "diagram-swimlane")]
        crate::DiagramFamilyId::SWIMLANE => {
            crate::flowchart::native_theme::SwimlaneThemeAdapter::CAPS
        }
        #[cfg(feature = "diagram-class")]
        crate::DiagramFamilyId::CLASS => crate::class::ClassThemeAdapter::CAPS,
        #[cfg(feature = "diagram-er")]
        crate::DiagramFamilyId::ER => crate::er::ErThemeAdapter::CAPS,
        #[cfg(feature = "diagram-requirement")]
        crate::DiagramFamilyId::REQUIREMENT => crate::requirement::RequirementThemeAdapter::CAPS,
        #[cfg(feature = "diagram-sequence")]
        crate::DiagramFamilyId::SEQUENCE => crate::sequence::SequenceThemeAdapter::CAPS,
        #[cfg(feature = "diagram-zenuml")]
        crate::DiagramFamilyId::ZENUML => crate::zenuml::ZenumlThemeAdapter::CAPS,
        #[cfg(feature = "diagram-kanban")]
        crate::DiagramFamilyId::KANBAN => crate::kanban::KanbanThemeAdapter::CAPS,
        #[cfg(feature = "diagram-packet")]
        crate::DiagramFamilyId::PACKET => crate::packet::PacketThemeAdapter::CAPS,
        #[cfg(feature = "diagram-railroad")]
        crate::DiagramFamilyId::RAILROAD => crate::railroad::RailroadThemeAdapter::CAPS,
        #[cfg(feature = "diagram-ishikawa")]
        crate::DiagramFamilyId::ISHIKAWA => crate::ishikawa::IshikawaThemeAdapter::CAPS,
        #[cfg(feature = "diagram-wardley")]
        crate::DiagramFamilyId::WARDLEY => crate::wardley::WardleyThemeAdapter::CAPS,
        #[cfg(feature = "diagram-cynefin")]
        crate::DiagramFamilyId::CYNEFIN => crate::cynefin::CynefinThemeAdapter::CAPS,
        #[cfg(feature = "diagram-quadrant-chart")]
        crate::DiagramFamilyId::QUADRANT_CHART => {
            crate::quadrantchart::QuadrantChartThemeAdapter::CAPS
        }
        #[cfg(feature = "diagram-radar")]
        crate::DiagramFamilyId::RADAR => crate::radar::RadarThemeAdapter::CAPS,
        #[cfg(feature = "diagram-venn")]
        crate::DiagramFamilyId::VENN => crate::venn::VennThemeAdapter::CAPS,
        #[cfg(feature = "diagram-git-graph")]
        crate::DiagramFamilyId::GIT_GRAPH => crate::gitgraph::GitGraphThemeAdapter::CAPS,
        #[cfg(feature = "diagram-mindmap")]
        crate::DiagramFamilyId::MINDMAP => crate::mindmap::MindmapThemeAdapter::CAPS,
        #[cfg(feature = "diagram-tree-view")]
        crate::DiagramFamilyId::TREE_VIEW => crate::tree_view::TreeViewThemeAdapter::CAPS,
        #[cfg(feature = "diagram-usecase")]
        crate::DiagramFamilyId::USECASE => crate::usecase::UsecaseThemeAdapter::CAPS,
        #[cfg(feature = "diagram-state")]
        crate::DiagramFamilyId::STATE => crate::state::StateThemeAdapter::CAPS,
        #[cfg(all(
            feature = "diagram-architecture",
            any(feature = "layout-cytoscape", feature = "layout-shoal")
        ))]
        crate::DiagramFamilyId::ARCHITECTURE => crate::architecture::ArchitectureThemeAdapter::CAPS,
        #[cfg(feature = "diagram-c4")]
        crate::DiagramFamilyId::C4 => crate::c4::C4ThemeAdapter::CAPS,
        #[cfg(feature = "diagram-block")]
        crate::DiagramFamilyId::BLOCK => crate::block::BlockThemeAdapter::CAPS,
        #[cfg(feature = "diagram-event-modeling")]
        crate::DiagramFamilyId::EVENT_MODELING => {
            crate::eventmodeling::EventModelingThemeAdapter::CAPS
        }
        #[cfg(feature = "diagram-gantt")]
        crate::DiagramFamilyId::GANTT => crate::gantt::GanttThemeAdapter::CAPS,
        #[cfg(feature = "diagram-timeline")]
        crate::DiagramFamilyId::TIMELINE => crate::timeline::TimelineThemeAdapter::CAPS,
        #[cfg(feature = "diagram-journey")]
        crate::DiagramFamilyId::JOURNEY => crate::journey::JourneyThemeAdapter::CAPS,
        #[cfg(feature = "diagram-info")]
        crate::DiagramFamilyId::INFO => crate::info::InfoThemeAdapter::CAPS,
        #[cfg(feature = "diagram-pie")]
        crate::DiagramFamilyId::PIE => crate::pie::PieThemeAdapter::CAPS,
        #[cfg(feature = "diagram-sankey")]
        crate::DiagramFamilyId::SANKEY => crate::sankey::SankeyThemeAdapter::CAPS,
        #[cfg(feature = "diagram-treemap")]
        crate::DiagramFamilyId::TREEMAP => crate::treemap::TreemapThemeAdapter::CAPS,
        #[cfg(feature = "diagram-xychart")]
        crate::DiagramFamilyId::XY_CHART => crate::xychart::XyChartThemeAdapter::CAPS,
        _ if crate::family::supports_diagram_type(family.as_str()) => ThemeCapabilityFlags::CANVAS,
        _ => ThemeCapabilityFlags::default(),
    }
}

/// Reports requested visual treatments that the family does not implement.
pub fn unsupported_features(
    theme: &ResolvedThemeV2,
    family: crate::DiagramFamilyId,
) -> impl Iterator<Item = &'static str> {
    use merman_theme_contract::v2::{Elevation, Form};

    let caps = family_capabilities(family);
    let theme = theme.family(family.as_str());
    let defaults = Form::default();
    [
        (!matches!(theme.elevation, Elevation::Flat)
            && !caps.contains(ThemeCapabilityFlags::ELEVATION))
        .then_some("This diagram family does not support theme elevation."),
        (theme.data.pattern_encoding && !caps.contains(ThemeCapabilityFlags::PATTERN_ENCODING))
            .then_some("This diagram family does not support categorical theme patterns."),
        ((theme.form.radius != defaults.radius
            || theme.form.container_radius != defaults.container_radius)
            && !caps.contains(ThemeCapabilityFlags::RADIUS))
        .then_some("This diagram family does not support theme corner radii."),
        (theme.form.edge_style != defaults.edge_style
            && !caps.contains(ThemeCapabilityFlags::EDGE_STYLE))
        .then_some("This diagram family does not support theme connector styles."),
        (theme.form.marker_style != defaults.marker_style
            && !caps.contains(ThemeCapabilityFlags::MARKER_STYLE))
        .then_some("This diagram family does not support theme marker styles."),
        (theme.form.chart != defaults.chart && !caps.contains(ThemeCapabilityFlags::CHART_POLISH))
            .then_some("This diagram family does not support theme chart styling."),
    ]
    .into_iter()
    .flatten()
}

#[allow(dead_code, reason = "Used by enabled diagram families.")]
pub(crate) fn font_family_css(families: &[String]) -> String {
    let mut css = String::new();
    for (index, family) in families.iter().enumerate() {
        if index != 0 {
            css.push_str(", ");
        }
        if matches!(
            family.as_str(),
            "serif" | "sans-serif" | "monospace" | "system-ui" | "cursive" | "fantasy"
        ) {
            css.push_str(family);
        } else {
            cssparser::serialize_string(family, &mut css)
                .expect("writing a CSS string to String cannot fail");
        }
    }
    css
}
