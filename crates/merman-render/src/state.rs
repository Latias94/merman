//! State diagram (stateDiagram-v2) layout.
//!
//! Source semantics: Mermaid 11.16.

use merman_theme_contract::{ThemeRuleFacetV1, ThemeSupportFacetV1};

use crate::diagram_theme::ThemeTarget;

type StateDiagramModel = merman_core::diagrams::state::StateDiagramRenderModel;
type StateNode = merman_core::diagrams::state::StateDiagramRenderNode;

mod label;
pub(crate) use label::{
    StateLabelMeasurement, measure_state_markdown_label, state_edge_label_xhtml,
    state_markdown_label_plain_text, state_node_label_xhtml, state_value_to_label_text,
};

mod edge_label_geometry;
pub(crate) use edge_label_geometry::StateNativeLabelGeometry;

mod label_artifact;
pub(crate) use label_artifact::{
    PreparedStateLabel, StateLabelMetricsRequest, StateLabelOwner, StateLabelSidecar,
    StateLabelSidecarBuilder, StateLabelSourceKind,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RectWithTitleGeometry {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) label_x: f64,
    pub(crate) label_y: f64,
    pub(crate) title_x: f64,
    pub(crate) description_x: f64,
    pub(crate) description_y: f64,
    pub(crate) divider_y: f64,
}

impl RectWithTitleGeometry {
    pub(crate) fn from_metrics(
        title_width: f64,
        title_height: f64,
        description_width: f64,
        description_height: f64,
        padding: f64,
    ) -> Self {
        let title_width = title_width.max(0.0);
        let title_height = title_height.max(0.0);
        let description_width = description_width.max(0.0);
        let description_height = description_height.max(0.0);
        let padding = padding.max(0.0);
        let half_padding = padding / 2.0;
        let label_width = title_width.max(description_width);
        let description_y = title_height + half_padding + 5.0;
        let label_height = description_y + description_height;

        Self {
            width: (label_width + padding).max(1.0),
            height: (label_height + padding).max(1.0),
            label_x: -label_width / 2.0,
            label_y: -label_height / 2.0 - half_padding + 3.0,
            title_x: ((label_width - title_width) / 2.0).max(0.0),
            description_x: ((label_width - description_width) / 2.0).max(0.0),
            description_y,
            divider_y: -label_height / 2.0 + title_height,
        }
    }
}

mod compatibility;
mod config;
mod effect_evidence;
mod effect_plan;
mod layout;
mod style_plan;

pub(crate) use compatibility::{
    StateCompatibilityPlan, StateTerminalPaintProperty, StateTerminalSurface,
};
pub(crate) use config::StateConfigView;
pub(crate) use effect_evidence::StateSvgEffectEvidenceRecorder;
pub(crate) use effect_plan::{
    StateEffectOutsets, StateEffectPlan, StateNodeEffectPlan, StateSvgEffect, StateSvgFilterRegion,
};
pub(crate) use style_plan::{
    ResolvedLabelTypography, StateEdgeStylePlan, StateNodeStylePlan, StateStylePlan,
};

/// Coarse static support projected from State's existing terminal consumers.
///
/// Route ownership remains broader because the State adapter must also report honest residuals for
/// unsupported values. Public support discovery uses this narrower projection for positive claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateStaticThemeSupport {
    Partial,
    SurfaceDependent,
    Unsupported,
}

pub(crate) fn static_theme_support(
    target: ThemeTarget,
    facet: ThemeSupportFacetV1,
) -> StateStaticThemeSupport {
    use StateStaticThemeSupport::{Partial, SurfaceDependent, Unsupported};

    match facet {
        ThemeSupportFacetV1::OrdinalPalette => {
            if matches!(
                target,
                ThemeTarget::State
                    | ThemeTarget::StateLabel
                    | ThemeTarget::Transition
                    | ThemeTarget::TransitionLabel
                    | ThemeTarget::Composite
                    | ThemeTarget::CompositeLabel
                    | ThemeTarget::SpecialState
                    | ThemeTarget::Note
                    | ThemeTarget::NoteLabel
            ) {
                SurfaceDependent
            } else {
                Unsupported
            }
        }
        ThemeSupportFacetV1::Rule(facet) => {
            let text_surface = matches!(
                target,
                ThemeTarget::Text
                    | ThemeTarget::Title
                    | ThemeTarget::StateLabel
                    | ThemeTarget::TransitionLabel
                    | ThemeTarget::CompositeLabel
                    | ThemeTarget::NoteLabel
            );
            let shape_surface = matches!(
                target,
                ThemeTarget::State
                    | ThemeTarget::Transition
                    | ThemeTarget::TransitionMarker
                    | ThemeTarget::TransitionLabelBackground
                    | ThemeTarget::Composite
                    | ThemeTarget::CompositeHeader
                    | ThemeTarget::SpecialState
                    | ThemeTarget::SpecialStateInner
                    | ThemeTarget::Note
            );
            let geometry_surface = matches!(
                target,
                ThemeTarget::State | ThemeTarget::Composite | ThemeTarget::Note
            );

            let supported = match facet {
                ThemeRuleFacetV1::Fill => text_surface || shape_surface,
                ThemeRuleFacetV1::StrokePaint
                | ThemeRuleFacetV1::StrokeWidth
                | ThemeRuleFacetV1::StrokeDasharray
                | ThemeRuleFacetV1::StrokeLineCap
                | ThemeRuleFacetV1::StrokeLineJoin
                | ThemeRuleFacetV1::Opacity
                | ThemeRuleFacetV1::FillOpacity
                | ThemeRuleFacetV1::StrokeOpacity => shape_surface,
                ThemeRuleFacetV1::Radius | ThemeRuleFacetV1::Padding => geometry_surface,
                ThemeRuleFacetV1::FontStack
                | ThemeRuleFacetV1::FontSize
                | ThemeRuleFacetV1::FontWeight
                | ThemeRuleFacetV1::FontStyle
                | ThemeRuleFacetV1::LetterSpacing
                | ThemeRuleFacetV1::WordSpacing
                | ThemeRuleFacetV1::TextTransform => text_surface,
                ThemeRuleFacetV1::Effect => target == ThemeTarget::State,
                ThemeRuleFacetV1::LineHeight
                | ThemeRuleFacetV1::TextDecoration
                | ThemeRuleFacetV1::TextAlign
                | ThemeRuleFacetV1::WhiteSpace
                | ThemeRuleFacetV1::Wrap => false,
            };

            if supported { Partial } else { Unsupported }
        }
        _ => Unsupported,
    }
}

pub(crate) use layout::layout_state_diagram_typed_with_work_meter;
pub use layout::{
    debug_build_state_diagram_dagre_graph, debug_extract_state_diagram_cluster_graph,
};

#[cfg(test)]
mod tests {
    use super::RectWithTitleGeometry;

    #[test]
    fn rect_with_title_geometry_matches_upstream_equations_for_non_default_padding() {
        let geometry = RectWithTitleGeometry::from_metrics(80.0, 24.0, 60.0, 18.0, 12.0);

        assert_eq!(geometry.width, 92.0);
        assert_eq!(geometry.height, 65.0);
        assert_eq!(geometry.label_x, -40.0);
        assert_eq!(geometry.label_y, -29.5);
        assert_eq!(geometry.title_x, 0.0);
        assert_eq!(geometry.description_x, 10.0);
        assert_eq!(geometry.description_y, 35.0);
        assert_eq!(geometry.divider_y, -2.5);
    }
}
