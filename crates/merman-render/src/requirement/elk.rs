//! Keeps the shared ELK provider policy and clipping at their existing call sites.

use crate::elk_edge_geometry::{self, Outline, Shape};
use crate::layout_backend::ElkRootAlgorithm;
use crate::layout_work::OperationLayoutWorkControl;
use crate::{Result, text::TextMeasurer};
use merman_layout_elk as elk;
use merman_model::families::requirement::RequirementRelationship;
use merman_render_core::{LayoutNode, LayoutPoint};
use merman_render_graph::requirement::{
    self, RequirementMeasurements, RequirementPreparedArtifact,
};
use serde_json::Value;

pub(super) fn layout(
    relationships: &[RequirementRelationship],
    measured: RequirementMeasurements<'_>,
    config: &Value,
    text_measurer: &dyn TextMeasurer,
    algorithm: ElkRootAlgorithm,
    operation_seed: elk::ElkOperationSeed,
    work: &mut OperationLayoutWorkControl,
) -> Result<RequirementPreparedArtifact> {
    requirement::layout_elk(
        relationships,
        measured,
        config,
        text_measurer,
        work,
        |input, work| solve(input, config, algorithm, operation_seed, work),
        clip_rect_route,
    )
}

fn solve(
    input: &mut elk::Graph,
    config: &Value,
    algorithm: ElkRootAlgorithm,
    operation_seed: elk::ElkOperationSeed,
    work: &mut OperationLayoutWorkControl,
) -> Result<elk::LayoutResult> {
    input.options = crate::elk_options::layout_options(config, algorithm);
    if crate::config::config_bool(config, &["elk", "keepEntryNodeOnTop"]).unwrap_or(false) {
        crate::elk_adapter::apply_cyclic_entry_constraints(
            &mut input.nodes,
            input
                .edges
                .iter()
                .map(|edge| (edge.source.as_str(), edge.target.as_str())),
            &mut Some(&mut *work),
        )?;
    }
    work.charge_adapter(input.nodes.len().saturating_add(input.edges.len()))?;
    let oriented =
        crate::elk_feedback_edges::orient_feedback_edges(input, config, &mut Some(&mut *work))?;
    let mut placed = crate::layout_work::measure_layout_solve!(
        elk::layout_with_operation_seed_and_work_control(oriented.graph(), operation_seed, work)
    )
    .map_err(|error| work.map_elk_error_with_context(error, "Requirement ELK"))?;
    oriented.restore(&mut placed, &mut Some(&mut *work))?;
    Ok(placed)
}

fn clip_rect_route(
    points: &[LayoutPoint],
    start: &LayoutNode,
    end: &LayoutNode,
) -> Vec<LayoutPoint> {
    elk_edge_geometry::sanitize(
        points,
        Shape {
            intersection: None,
            node: start,
            outline: Outline::Rect,
        },
        Shape {
            intersection: None,
            node: end,
            outline: Outline::Rect,
        },
    )
}
