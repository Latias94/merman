//! Native theme selection and visual settings used by family renderers.

use std::sync::Arc;

use merman_model::presentation::PresentationConfig;

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

/// Projects the configuration used by native layout, paint, and text admission.
#[doc(hidden)]
pub fn native_render_config(
    config: &PresentationConfig,
    control: &merman_model::OperationControl,
) -> crate::Result<Option<PresentationConfig>> {
    use merman_model::MermaidConfig;
    use merman_model::presentation::FrozenPresentationConfigParts;
    use merman_model::resources::{InputResourcePolicy, ResourceProfile};

    const MERMAID_VISUAL_KEYS: [&str; 6] = [
        "theme",
        "themeVariables",
        "darkMode",
        "fontFamily",
        "fontSize",
        "fontWeight",
    ];
    if MERMAID_VISUAL_KEYS
        .iter()
        .all(|key| config.get(key).is_none())
    {
        return Ok(None);
    }
    let Some(values) = config.as_value().as_object() else {
        return Ok(None);
    };

    // Keep structural and family configuration and the trusted output CSS boundary. The source
    // configuration and its authored paths remain available to compatibility projections.
    let control = control.for_phase(merman_model::OperationPhase::Layout);
    let mut retained = MermaidConfig::empty_object();
    let retained_keys = retained
        .as_value_mut()
        .as_object_mut()
        .expect("empty config object");
    for (key, value) in values
        .iter()
        .filter(|(key, _)| !MERMAID_VISUAL_KEYS.contains(&key.as_str()))
    {
        retained_keys.insert(
            key.clone(),
            merman_model::json::clone_value_nonrecursive_with_control(value, &control)?,
        );
    }
    let mut paths = std::collections::BTreeSet::new();
    for path in config.explicit_config_paths() {
        control.checkpoint()?;
        if path.is_empty() {
            paths.extend(retained_keys.keys().map(|key| Arc::from(key.as_str())));
        } else if !MERMAID_VISUAL_KEYS
            .contains(&path.split_once('.').map_or(path, |(root, _)| root))
        {
            paths.insert(Arc::from(path));
        }
    }
    let parts =
        FrozenPresentationConfigParts::new(retained).with_explicit_config_paths(Arc::new(paths));
    // The projection only removes admitted data and preserves ownership of the retained keys.
    let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    Ok(Some(PresentationConfig::try_from_frozen_parts_controlled(
        parts, &policy, &control,
    )??))
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
