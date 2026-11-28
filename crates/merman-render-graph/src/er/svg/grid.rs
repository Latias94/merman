//! Paint ER's materialized Grid shafts and measured labelRect terminals.

use merman_render_core::{LayoutNode, graph_layout::mermaid_grid::GridPaint};
use rustc_hash::FxHashSet;

use super::*;
pub(super) use crate::grid_edges::{line_hop_paths, shaft};

pub(super) fn paint_settings(mut settings: GridPaint, native: Option<&ErNativeStyle>) -> GridPaint {
    if native.is_some_and(|style| style.edge_style != EdgeStyle::Curved) {
        // The Grid solver has already produced orthogonal shafts. Use their vertices directly
        // when native form requests an orthogonal, straight, or sketch connector.
        settings.curve = "linear";
    }
    settings
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
