//! Native theme selection and visual settings used by family renderers.

use std::sync::Arc;

use merman_model::presentation::PresentationConfig;

#[allow(unused_imports, reason = "Used by enabled diagram families.")]
pub(crate) use merman_render_core::mermaid_style::font_family_css;

pub(crate) use merman_render_core::theme_svg::canvas as canvas_svg;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "diagram-state",
    feature = "diagram-swimlane"
))]
pub(crate) use merman_render_core::theme_svg::edge_label_background as edge_label_bg;
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
pub(crate) use merman_render_core::theme_svg::elevation as elevation_svg;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "diagram-swimlane",
    feature = "diagram-block",
    feature = "diagram-state",
))]
pub(crate) use merman_render_core::theme_svg::marker as marker_svg;
#[cfg(any(feature = "diagram-treemap", feature = "diagram-xychart"))]
pub(crate) use merman_render_core::theme_svg::pattern as pattern_svg;

pub use merman_theme_contract::v2::{
    Appearance, BuiltinPreset, Color, FamilyTheme, ResolvedThemeV2, ThemeCapabilityFlags,
    ThemeCompiler, ThemeDocumentV2, ThemeError, ThemeNotice,
};

/// An operation-local projection of the configuration used by native layout and text admission.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct NativeRenderConfig(Arc<NativeRenderConfigParts>);

#[derive(Debug)]
struct NativeRenderConfigParts {
    source: PresentationConfig,
    render: PresentationConfig,
}

impl NativeRenderConfig {
    pub fn prepare(
        config: &PresentationConfig,
        control: &merman_model::OperationControl,
    ) -> merman_model::OperationControlResult<Option<Self>> {
        let render = Self::project(config, control)?;
        Ok(render.map(|render| {
            Self(Arc::new(NativeRenderConfigParts {
                source: config.clone(),
                render,
            }))
        }))
    }

    fn project(
        config: &PresentationConfig,
        control: &merman_model::OperationControl,
    ) -> merman_model::OperationControlResult<Option<PresentationConfig>> {
        let control = control.for_phase(merman_model::OperationPhase::Layout);
        // Keep structural settings and the trusted output CSS boundary. The original source
        // configuration and authored paths remain available to compatibility projections.
        config.without_root_keys_controlled(
            &[
                "theme",
                "themeVariables",
                "darkMode",
                "fontFamily",
                "fontSize",
                "fontWeight",
            ],
            &control,
        )
    }

    pub fn config(&self) -> &PresentationConfig {
        &self.0.render
    }

    fn is_for(&self, source: &PresentationConfig) -> bool {
        let original = &self.0.source;
        original.as_config().shares_storage_with(source.as_config())
            && Arc::ptr_eq(
                original.shared_explicit_config_paths(),
                source.shared_explicit_config_paths(),
            )
    }
}

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
    render_config: Option<NativeRenderConfig>,
    export_backdrop: Option<Color>,
}

impl NativeTheme {
    /// Uses the theme's resolved canvas paint and appearance.
    pub fn new(resolved: Arc<ResolvedThemeV2>) -> Self {
        Self {
            resolved,
            background: None,
            render_config: None,
            export_backdrop: None,
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

    /// Supplies an opaque destination color for automatic contrast under a translucent canvas.
    #[doc(hidden)]
    pub fn with_export_backdrop(mut self, color: Color) -> Self {
        self.export_backdrop = Some(color);
        self
    }

    #[cfg(feature = "diagram-pie")]
    pub(crate) const fn export_backdrop(&self) -> Option<Color> {
        self.export_backdrop
    }

    #[doc(hidden)]
    pub fn with_render_config(mut self, config: Option<NativeRenderConfig>) -> Self {
        self.render_config = config;
        self
    }

    pub(crate) fn render_config_for(
        &self,
        source: &PresentationConfig,
        control: &merman_model::OperationControl,
    ) -> merman_model::OperationControlResult<Option<PresentationConfig>> {
        if let Some(config) = &self.render_config
            && config.is_for(source)
        {
            return Ok(Some(config.config().clone()));
        }
        NativeRenderConfig::project(source, control)
    }
}

use ThemeCapabilityFlags as Cap;

macro_rules! caps {
    ($first:ident $(| $rest:ident)*) => {
        Cap::$first$(.union(Cap::$rest))*
    };
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "diagram-swimlane"
))]
const FLOWCHART_BASE_CAPABILITIES: Cap = caps!(
    CANVAS
        | NODE_SURFACE
        | CONTAINER
        | CALLOUT
        | EMPHASIS
        | CONNECTOR
        | DATA_FILL
        | ELEVATION
        | RADIUS
        | MARKER_STYLE
        | TYPOGRAPHY
);

/// Returns the native theme capabilities implemented by this build for a logical family.
pub fn family_capabilities(family: crate::DiagramFamilyId) -> ThemeCapabilityFlags {
    match family {
        #[cfg(feature = "diagram-flowchart")]
        crate::DiagramFamilyId::FLOWCHART => FLOWCHART_BASE_CAPABILITIES.union(Cap::EDGE_STYLE),
        #[cfg(feature = "diagram-agentflow")]
        crate::DiagramFamilyId::AGENTFLOW => FLOWCHART_BASE_CAPABILITIES.union(Cap::EDGE_STYLE),
        #[cfg(feature = "diagram-swimlane")]
        crate::DiagramFamilyId::SWIMLANE => FLOWCHART_BASE_CAPABILITIES,
        #[cfg(feature = "diagram-class")]
        crate::DiagramFamilyId::CLASS => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | CALLOUT
                | EMPHASIS
                | CONNECTOR
                | RADIUS
                | EDGE_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-er")]
        crate::DiagramFamilyId::ER => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | EMPHASIS
                | CONNECTOR
                | RADIUS
                | EDGE_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-requirement")]
        crate::DiagramFamilyId::REQUIREMENT => caps!(
            CANVAS
                | NODE_SURFACE
                | EMPHASIS
                | CONNECTOR
                | STATUS_COLORS
                | RADIUS
                | EDGE_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-sequence")]
        crate::DiagramFamilyId::SEQUENCE => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | CALLOUT
                | EMPHASIS
                | CONNECTOR
                | ELEVATION
                | RADIUS
                | MARKER_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-zenuml")]
        crate::DiagramFamilyId::ZENUML => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | CALLOUT
                | EMPHASIS
                | CONNECTOR
                | ELEVATION
                | RADIUS
                | MARKER_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-kanban")]
        crate::DiagramFamilyId::KANBAN => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | DATA_INK
                | STATUS_COLORS
                | ELEVATION
                | RADIUS
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-packet")]
        crate::DiagramFamilyId::PACKET => caps!(CANVAS | NODE_SURFACE | TYPOGRAPHY),
        #[cfg(feature = "diagram-railroad")]
        crate::DiagramFamilyId::RAILROAD => {
            caps!(CANVAS | NODE_SURFACE | EMPHASIS | CONNECTOR | RADIUS | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-ishikawa")]
        crate::DiagramFamilyId::ISHIKAWA => {
            caps!(CANVAS | NODE_SURFACE | EMPHASIS | CONNECTOR | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-wardley")]
        crate::DiagramFamilyId::WARDLEY => {
            caps!(CANVAS | NODE_SURFACE | CONNECTOR | DATA_INK | CHART_POLISH | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-cynefin")]
        crate::DiagramFamilyId::CYNEFIN => {
            caps!(CANVAS | NODE_SURFACE | CONTAINER | DATA_FILL | CONNECTOR | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-quadrant-chart")]
        crate::DiagramFamilyId::QUADRANT_CHART => {
            caps!(CANVAS | NODE_SURFACE | CONTAINER | CONNECTOR | DATA_INK | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-radar")]
        crate::DiagramFamilyId::RADAR => {
            caps!(CANVAS | CONNECTOR | DATA_INK | DATA_FILL | CHART_POLISH | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-venn")]
        crate::DiagramFamilyId::VENN => caps!(CANVAS | DATA_INK | DATA_FILL | TYPOGRAPHY),
        #[cfg(feature = "diagram-git-graph")]
        crate::DiagramFamilyId::GIT_GRAPH => {
            caps!(CANVAS | NODE_SURFACE | CONNECTOR | CALLOUT | DATA_INK | DATA_FILL | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-mindmap")]
        crate::DiagramFamilyId::MINDMAP => caps!(
            CANVAS
                | NODE_SURFACE
                | EMPHASIS
                | CONNECTOR
                | DATA_INK
                | DATA_FILL
                | RADIUS
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-tree-view")]
        crate::DiagramFamilyId::TREE_VIEW => {
            caps!(CANVAS | NODE_SURFACE | EMPHASIS | CONNECTOR | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-usecase")]
        crate::DiagramFamilyId::USECASE => {
            caps!(CANVAS | NODE_SURFACE | CONTAINER | CALLOUT | CONNECTOR | ELEVATION | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-state")]
        crate::DiagramFamilyId::STATE => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | CALLOUT
                | EMPHASIS
                | CONNECTOR
                | ELEVATION
                | RADIUS
                | MARKER_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(all(
            feature = "diagram-architecture",
            any(feature = "layout-cytoscape", feature = "layout-shoal")
        ))]
        crate::DiagramFamilyId::ARCHITECTURE => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | CONNECTOR
                | ELEVATION
                | RADIUS
                | EDGE_STYLE
                | MARKER_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-c4")]
        crate::DiagramFamilyId::C4 => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | DATA_FILL
                | CONNECTOR
                | RADIUS
                | EDGE_STYLE
                | MARKER_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-block")]
        crate::DiagramFamilyId::BLOCK => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | CONNECTOR
                | ELEVATION
                | RADIUS
                | EDGE_STYLE
                | MARKER_STYLE
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-event-modeling")]
        crate::DiagramFamilyId::EVENT_MODELING => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | DATA_FILL
                | STATUS_COLORS
                | CONNECTOR
                | RADIUS
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-gantt")]
        crate::DiagramFamilyId::GANTT => caps!(
            CANVAS
                | NODE_SURFACE
                | CONTAINER
                | STATUS_COLORS
                | DATA_FILL
                | CHART_POLISH
                | RADIUS
                | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-timeline")]
        crate::DiagramFamilyId::TIMELINE => {
            caps!(CANVAS | NODE_SURFACE | CONTAINER | DATA_FILL | CONNECTOR | RADIUS | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-journey")]
        crate::DiagramFamilyId::JOURNEY => caps!(
            CANVAS | NODE_SURFACE | CONTAINER | DATA_FILL | STATUS_COLORS | RADIUS | TYPOGRAPHY
        ),
        #[cfg(feature = "diagram-info")]
        crate::DiagramFamilyId::INFO => caps!(CANVAS | TYPOGRAPHY),
        #[cfg(feature = "diagram-pie")]
        crate::DiagramFamilyId::PIE => {
            caps!(CANVAS | DATA_FILL | CHART_POLISH | PATTERN_ENCODING | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-sankey")]
        crate::DiagramFamilyId::SANKEY => caps!(CANVAS | DATA_INK | DATA_FILL | TYPOGRAPHY),
        #[cfg(feature = "diagram-treemap")]
        crate::DiagramFamilyId::TREEMAP => {
            caps!(CANVAS | CONTAINER | DATA_FILL | RADIUS | PATTERN_ENCODING | TYPOGRAPHY)
        }
        #[cfg(feature = "diagram-xychart")]
        crate::DiagramFamilyId::XY_CHART => caps!(
            CANVAS
                | CONNECTOR
                | DATA_INK
                | DATA_FILL
                | CHART_POLISH
                | ELEVATION
                | RADIUS
                | PATTERN_ENCODING
                | TYPOGRAPHY
        ),
        _ if crate::family::supports_diagram_type(family.as_str()) => caps!(CANVAS),
        _ => Cap::default(),
    }
}

/// Reports requested visual treatments that the family does not implement.
pub fn unsupported_features(
    theme: &ResolvedThemeV2,
    family: crate::DiagramFamilyId,
) -> impl Iterator<Item = &'static str> {
    use crate::DiagramFamilyId;
    use merman_theme_contract::v2::{Elevation, Form};

    let caps = family_capabilities(family);
    let uses_chart_settings = matches!(
        family,
        DiagramFamilyId::GANTT
            | DiagramFamilyId::PIE
            | DiagramFamilyId::QUADRANT_CHART
            | DiagramFamilyId::RADAR
            | DiagramFamilyId::WARDLEY
            | DiagramFamilyId::XY_CHART
    );
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
        (uses_chart_settings
            && theme.form.chart != defaults.chart
            && !caps.contains(ThemeCapabilityFlags::CHART_POLISH))
        .then_some("This diagram family does not support theme chart styling."),
    ]
    .into_iter()
    .flatten()
}
