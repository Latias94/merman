//! Flowchart edge marker and class helpers.

#[cfg(any(
    feature = "layout-shoal",
    test,
    all(
        feature = "native-compact",
        feature = "diagram-flowchart",
        feature = "layout-dagre"
    )
))]
pub(in crate::svg::parity::flowchart) use merman_render_flow::markers::{
    FlowchartMarkerBase, flowchart_edge_marker_end_base, flowchart_edge_marker_start_base,
};

use super::FlowchartCompiledStyles;

pub(super) use merman_render_flow::edge_svg::flowchart_neo_mask_uses_dash_pairs;

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
