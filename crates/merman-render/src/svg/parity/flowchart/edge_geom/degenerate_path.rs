//! Flowchart edge path overrides.
//!
//! Mermaid flowchart-v2 can emit a degenerate edge path when linking a subgraph to one of its
//! strict descendants (e.g. `Sub --> In` where `In` is declared inside `subgraph Sub`).
//! Upstream renders these as a single-point path (`M..Z`) while preserving the original
//! `data-points`.

use super::*;

pub(in crate::svg::parity::flowchart) fn maybe_override_degenerate_subgraph_edge_path_d(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    data_points: &[crate::model::LayoutPoint],
) -> Option<String> {
    let edge_is_between_subgraph_and_descendant = (ctx
        .subgraphs_by_id
        .contains_key(edge.from.as_str())
        && flowchart_is_strict_descendant(&ctx.parent, edge.to.as_str(), edge.from.as_str()))
        || (ctx.subgraphs_by_id.contains_key(edge.to.as_str())
            && flowchart_is_strict_descendant(&ctx.parent, edge.from.as_str(), edge.to.as_str()));
    if !edge_is_between_subgraph_and_descendant {
        return None;
    }

    let p = data_points.last()?;
    Some(format!(
        "M{},{}Z",
        crate::svg::parity::util::fmt_display(p.x + 4.0),
        crate::svg::parity::util::fmt_display(p.y)
    ))
}
