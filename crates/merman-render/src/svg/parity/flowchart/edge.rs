//! Flowchart edge marker and class helpers.

use super::FlowchartCompiledStyles;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::svg::parity::flowchart) enum FlowchartMarkerBase {
    PointStart,
    PointEnd,
    CircleStart,
    CircleEnd,
    CrossStart,
    CrossEnd,
}

impl FlowchartMarkerBase {
    pub(in crate::svg::parity::flowchart) const fn id_suffix(self) -> &'static str {
        match self {
            Self::PointStart => "pointStart",
            Self::PointEnd => "pointEnd",
            Self::CircleStart => "circleStart",
            Self::CircleEnd => "circleEnd",
            Self::CrossStart => "crossStart",
            Self::CrossEnd => "crossEnd",
        }
    }
}

pub(super) fn flowchart_edge_marker_end_base(
    edge: &crate::flowchart::FlowEdge,
) -> Option<FlowchartMarkerBase> {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => Some(FlowchartMarkerBase::PointEnd),
        Some("double_arrow_circle") => Some(FlowchartMarkerBase::CircleEnd),
        Some("double_arrow_cross") => Some(FlowchartMarkerBase::CrossEnd),
        Some("arrow_point") => Some(FlowchartMarkerBase::PointEnd),
        Some("arrow_cross") => Some(FlowchartMarkerBase::CrossEnd),
        Some("arrow_circle") => Some(FlowchartMarkerBase::CircleEnd),
        Some("arrow_open") => None,
        _ => Some(FlowchartMarkerBase::PointEnd),
    }
}

pub(super) fn flowchart_edge_marker_start_base(
    edge: &crate::flowchart::FlowEdge,
) -> Option<FlowchartMarkerBase> {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => Some(FlowchartMarkerBase::PointStart),
        Some("double_arrow_circle") => Some(FlowchartMarkerBase::CircleStart),
        Some("double_arrow_cross") => Some(FlowchartMarkerBase::CrossStart),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::svg::parity::flowchart) struct FlowchartEdgeAnimationResolution {
    generated_class: Option<&'static str>,
    active: bool,
}

impl FlowchartEdgeAnimationResolution {
    pub(in crate::svg::parity::flowchart) fn resolve(
        edge: &crate::flowchart::FlowEdge,
        styles: &FlowchartCompiledStyles,
    ) -> Self {
        // Mermaid 11.16.1 `edges.js` derives a fast class from `animate` and then lets the
        // explicit `animation` speed replace it. In particular, `animate: false, animation: fast`
        // is animated. Source declarations are resolved separately because their inline CSS can
        // replace the computed animation name after the generated class is attached.
        let explicit_animation = edge
            .animation
            .as_deref()
            .map(str::trim)
            .filter(|animation| !animation.is_empty());
        let generated_class = match explicit_animation {
            Some(animation) if animation.eq_ignore_ascii_case("slow") => {
                Some("edge-animation-slow")
            }
            Some(_) => Some("edge-animation-fast"),
            None if edge.animate == Some(true) => Some("edge-animation-fast"),
            None => None,
        };
        Self {
            active: styles
                .edge_animation_active()
                .unwrap_or(edge.animate == Some(true) || explicit_animation.is_some()),
            generated_class,
        }
    }

    pub(in crate::svg::parity::flowchart) const fn class(self) -> Option<&'static str> {
        self.generated_class
    }

    pub(in crate::svg::parity::flowchart) const fn is_active(self) -> bool {
        self.active
    }
}
