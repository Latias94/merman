//! Flowchart edge class attributes and class-definition stylesheet access.

use merman_render_flow::style::FlowchartEdgeAnimationResolution;

use super::SvgOutput;

#[inline]
pub(super) fn write_flowchart_edge_class_attr(
    out: &mut impl SvgOutput,
    edge: &crate::flowchart::FlowEdge,
    animation: FlowchartEdgeAnimationResolution,
) {
    // Mermaid includes a 2-part class tuple (thickness/pattern) for flowchart edge paths. The
    // second tuple is `edge-thickness-normal edge-pattern-solid` in Mermaid@11.12.2 baselines,
    // even for dotted/thick strokes.
    let (thickness_1, pattern_1) = match edge.stroke.as_deref() {
        Some("thick") => ("edge-thickness-thick", "edge-pattern-solid"),
        Some("invisible") => ("edge-thickness-invisible", "edge-pattern-solid"),
        Some("dotted") => ("edge-thickness-normal", "edge-pattern-dotted"),
        _ => ("edge-thickness-normal", "edge-pattern-solid"),
    };

    if thickness_1 == "edge-thickness-invisible" {
        // Mermaid@11.12.2 does *not* include the second tuple nor `flowchart-link` for invisible
        // edges.
        out.push_str(thickness_1);
        out.push(' ');
        out.push_str(pattern_1);
        return;
    }

    out.push_str(thickness_1);
    out.push(' ');
    out.push_str(pattern_1);
    out.push_str(" edge-thickness-normal edge-pattern-solid flowchart-link");

    if let Some(cls) = animation.class() {
        out.push(' ');
        out.push_str(cls);
    }
}
