//! Paint ER's materialized Grid shafts and measured labelRect terminals.

use std::collections::HashMap;

use merman_render_core::{
    LayoutNode,
    graph_layout::mermaid_grid::{
        GridLineHops, GridPaint,
        routing::paint::{GridPaintedEdge, grid_line_hop_overrides},
    },
};
use rustc_hash::FxHashSet;

use super::*;

pub(super) fn paint_settings(mut settings: GridPaint, native: Option<&ErNativeStyle>) -> GridPaint {
    if native.is_some_and(|style| style.edge_style != EdgeStyle::Curved) {
        // The Grid solver has already produced orthogonal shafts. Use their vertices directly
        // when native form requests an orthogonal, straight, or sketch connector.
        settings.curve = "linear";
    }
    settings
}

pub(super) fn shaft(
    points: &[LayoutPoint],
    settings: GridPaint,
    work: &OperationWorkMeter,
) -> Result<String> {
    work.charge_emit_work(points.len())?;
    // Both erBox and its cluster shape use rectangular intersections. Their measured Grid
    // ports already lie on those rectangles, so outline projection keeps the routed points.
    Ok(match settings.curve {
        "linear" => curve::curve_linear_path_d(points),
        "rounded" => curve::curve_rounded_path_d(points, settings.corner_radius),
        interpolate => {
            let mut adjusted = points.to_vec();
            edge_path::maybe_fix_corners(&mut adjusted);
            curve::interpolated_path_d(&adjusted, interpolate)
        }
    })
}

pub(super) fn line_hop_paths<'a>(
    edges: &'a [LayoutEdge],
    settings: GridPaint,
    work: &OperationWorkMeter,
) -> Result<HashMap<&'a str, String>> {
    if settings.line_hops == GridLineHops::Disabled {
        return Ok(HashMap::new());
    }
    work.charge_emit_work(edges.len())?;
    let mut painted = Vec::new();
    painted
        .try_reserve_exact(edges.len())
        .map_err(|_| allocation_error())?;
    for edge in edges {
        if edge.points.len() >= 2 {
            painted.push(GridPaintedEdge {
                geometry: line_hops::LineHopEdge {
                    id: &edge.id,
                    points: &edge.points,
                    curve: Some(settings.curve),
                    // ER cardinality shapes use the unshifted attachment coordinates.
                    arrow_type_start: None,
                    arrow_type_end: None,
                },
                corner_radius: Some(settings.corner_radius),
                // The shared pass uses this only when no curve is supplied.
                original_path: "",
            });
        }
    }
    let overrides = grid_line_hop_overrides(settings.line_hops, &painted, work)?;
    let mut paths = HashMap::new();
    paths
        .try_reserve(overrides.len())
        .map_err(|_| allocation_error())?;
    for (index, path) in overrides {
        work.charge_emit_work(path.len())?;
        paths.insert(painted[index].geometry.id, path);
    }
    Ok(paths)
}

pub(super) fn write_labels(
    out: &mut impl SvgOutput,
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    model: &ErDiagramModel,
    labels: &ErPreparedLabels,
    diagram_id: SvgId<'_>,
    context: ErSubgraphRenderContext<'_>,
) -> Result<()> {
    let work = context.work_meter;
    work.charge_emit_work(nodes.len())?;
    work.charge_emit_work(edges.len())?;
    let mut node_ids = FxHashSet::default();
    node_ids
        .try_reserve(nodes.len().saturating_add(1))
        .map_err(|_| allocation_error())?;
    node_ids.insert(diagram_id.semantic_str());
    for node in nodes {
        work.charge_emit_work(node.id.len().max(1))?;
        node_ids.insert(node.id.as_str());
    }
    for edge in edges {
        work.checkpoint(OperationPhase::Emit)?;
        let Some(label) = &edge.label else { continue };
        let index = er_rel_idx_from_edge_id(&edge.id)
            .filter(|index| *index < model.relationships.len())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing ER relationship owner for Grid label `{}`", edge.id),
            })?;
        let terminal = labels
            .relationship(index)
            .and_then(ErPreparedRelationshipLabel::label_rect)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared ER Grid relationship terminal {index}"),
            })?;
        work.charge_emit_work(edge.from.len())?;
        work.charge_emit_work(edge.to.len())?;
        let dom_id = er_relationship_dom_id(&model.relationships[index], index);
        let base = swimlane::relationship_label_dom_id(edge, &dom_id);
        let id = available_label_id(base, &node_ids, work)?;
        swimlane::write_relationship_label_rect(out, label, &id, terminal, context)?;
    }
    Ok(())
}

fn available_label_id(
    base: String,
    nodes: &FxHashSet<&str>,
    work: &OperationWorkMeter,
) -> Result<String> {
    work.charge_emit_work(base.len())?;
    if !nodes.contains(base.as_str()) {
        return Ok(base);
    }
    // The source relationship ordinal makes the base unique among helpers. Only original node
    // IDs can occupy one of its suffixed spellings.
    for suffix in 1..=nodes.len() {
        work.charge_emit_work(base.len().saturating_add(1))?;
        let candidate = format!("{base}-{suffix}");
        if !nodes.contains(candidate.as_str()) {
            return Ok(candidate);
        }
    }
    Err(Error::InvalidModel {
        message: format!("ER Grid could not allocate a label id for `{base}`"),
    })
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}
