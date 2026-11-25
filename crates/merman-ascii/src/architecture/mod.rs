//! Architecture-owned cardinal layout and complete semantic reading records.

mod layout;
mod validate;

use crate::graph::positioned::{
    PositionedEdge, PositionedGraph, PositionedMarker, PositionedShape, PositionedSide,
    PositionedSize, PositionedStroke, PositionedTextAdmission,
};
use crate::graph::{DeferredGraphLabelSectionPlan, DeferredGraphNodeLabelPlan};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, GraphLayoutPolicy};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiProjection, AsciiSourceContext,
    AsciiViewportPolicy, Result,
};
use merman_model::OperationPhase;
use merman_model::families::architecture::{ArchitectureDiagramModel, ArchitectureNodeKind};

const FAMILY: &str = "architecture";
const LABEL_WIDTH: usize = 18;

pub(crate) fn render(
    model: &ArchitectureDiagramModel,
    options: &AsciiRenderOptions,
    source: AsciiSourceContext<'_>,
    execution: AsciiExecution<'_>,
) -> Result<crate::output::RenderedPrimary> {
    let resources = execution.new_resource_context(OperationPhase::Semantic);
    let topology = if let Some(plan) = validate::prepare(model, &resources)? {
        let scene_resources = resources.scoped();
        let scene_execution = execution
            .with_render_ledger(&scene_resources)
            .with_trimmed_canvas()
            .with_viewport(AsciiViewportPolicy::unrestricted());
        let mut layout = scene_execution.resource_context(&scene_resources, OperationPhase::Layout);
        Some(render_topology(
            model,
            &plan,
            options,
            &mut layout,
            scene_execution,
        )?)
    } else {
        None
    };
    let projection = if topology.is_some() {
        AsciiProjection::Diagrammatic
    } else {
        AsciiProjection::StructuredText
    };
    crate::output::render_reading_primary(
        &AsciiDiagram::Architecture(model),
        source,
        options,
        execution,
        topology.as_deref(),
    )
    .map(|text| crate::output::RenderedPrimary::records(text, projection))
}

fn render_topology(
    model: &ArchitectureDiagramModel,
    plan: &validate::Plan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let count = resources.checked_grid_add(model.nodes.len(), model.groups.len())?;
    // Keys replace long route captions only in the overview. Complete fields follow below.
    let key_width = count.max(model.edges.len()).max(1).ilog10() as usize + 2;
    let mut generated = resources.checked_work_add(
        resources.checked_work_mul(
            count,
            resources.checked_work_add(resources.checked_work_mul(key_width, 2)?, 16)?,
        )?,
        resources.checked_work_mul(model.edges.len(), key_width)?,
    )?;
    for index in 0..count {
        resources.charge_layout_work(1)?;
        generated = resources.checked_work_add(generated, overview_title(model, index).len())?;
    }
    resources.check(AsciiResourceLimitId::MaxOutputBytes, generated)?;
    resources.charge_layout_work(generated)?;
    let mut ids = Vec::new();
    let mut headings = Vec::new();
    reserve(&mut ids, count)?;
    reserve(&mut headings, count)?;
    for (index, node) in model.nodes.iter().enumerate() {
        let key = format!("N{}", index + 1);
        headings.push(if node.node_type == ArchitectureNodeKind::Junction {
            format!("{key} [+]")
        } else {
            heading(&key, overview_title(model, index), resources)?
        });
        ids.push(key);
    }
    for index in 0..model.groups.len() {
        let key = format!("G{}", index + 1);
        headings.push(heading(
            &key,
            overview_title(model, model.nodes.len() + index),
            resources,
        )?);
        ids.push(key);
    }
    let mut edge_keys = Vec::new();
    reserve(&mut edge_keys, model.edges.len())?;
    for index in 0..model.edges.len() {
        edge_keys.push(format!("E{}", index + 1));
    }
    let profile = options.terminal_width_profile;
    let mut admission = PositionedTextAdmission::default();
    admission.include_retained_bytes(generated, resources)?;
    let mut labels = Vec::new();
    reserve(&mut labels, count)?;
    labels.resize_with(count, || None);
    for &index in &plan.preorder {
        resources.charge_layout_work(1)?;
        if index == count {
            continue;
        }
        let section = DeferredGraphLabelSectionPlan::try_single(
            &headings[index],
            Some(LABEL_WIDTH),
            FAMILY,
            profile,
            resources,
        )?;
        let label = DeferredGraphNodeLabelPlan::single(section, None, FAMILY, profile, resources)?;
        admission.include_label(&label, resources)?;
        admission.include_copy_work(resources.checked_work_mul(ids[index].len(), 2)?, resources)?;
        if index >= model.nodes.len() {
            for &child in &plan.children[index] {
                admission.include_copy_work(ids[child].len(), resources)?;
            }
        }
        labels[index] = Some(label);
    }
    for (index, &(from, to)) in plan.endpoints.iter().enumerate() {
        admission.include_copy_work(ids[from].len(), resources)?;
        admission.include_copy_work(ids[to].len(), resources)?;
        admission.include_copy_work(edge_keys[index].len(), resources)?;
        admission.include_edge_label(&edge_keys[index], profile, resources)?;
        for anchor in [plan.anchors[index].0, plan.anchors[index].1]
            .into_iter()
            .flatten()
        {
            admission.include_copy_work(ids[anchor].len(), resources)?;
        }
    }
    admission.admit(resources)?;
    let mut graph =
        PositionedGraph::try_new(FAMILY, policy(options), count, model.edges.len(), resources)?;
    let mut elements = Vec::new();
    let mut minimums = Vec::new();
    reserve(&mut elements, count)?;
    reserve(&mut minimums, count)?;
    elements.resize(count, None);
    minimums.resize(count, PositionedSize::default());
    for &index in &plan.preorder {
        resources.charge_layout_work(1)?;
        if index == count {
            continue;
        }
        let prepared = labels[index]
            .take()
            .ok_or_else(invalid_projection)?
            .materialize_after_admission(resources)?;
        let (element, mut minimum) = if index < model.nodes.len() {
            let node = &model.nodes[index];
            let shape = match node.node_type {
                ArchitectureNodeKind::Junction => PositionedShape::Diamond,
                ArchitectureNodeKind::Service => PositionedShape::Rect,
            };
            graph.try_add_node(&ids[index], prepared, shape, resources)?
        } else {
            graph.try_add_frame(
                &ids[index],
                prepared,
                plan.children[index]
                    .iter()
                    .map(|&child| ids[child].as_str()),
                resources,
            )?
        };
        if index < model.nodes.len() {
            // Odd contours put all four center ports on actual terminal cells.
            minimum.width |= 1;
            minimum.height |= 1;
        }
        elements[index] = Some(element);
        minimums[index] = minimum;
    }
    for (index, edge) in model.edges.iter().enumerate() {
        let (from, to) = plan.endpoints[index];
        graph.try_add_port_edge(
            PositionedEdge {
                id: &edge_keys[index],
                from: &ids[from],
                to: &ids[to],
                label: &edge_keys[index],
                stroke: PositionedStroke::Normal,
                start_marker: marker(edge.lhs_into),
                end_marker: marker(edge.rhs_into),
            },
            (
                side(edge.lhs_dir)?,
                plan.anchors[index].0.map(|anchor| ids[anchor].as_str()),
            ),
            (
                side(edge.rhs_dir)?,
                plan.anchors[index].1.map(|anchor| ids[anchor].as_str()),
            ),
            resources,
        )?;
    }
    let bounds = plan.bounds(&minimums, resources)?;
    for (index, bounds) in bounds.into_iter().enumerate() {
        graph.set_bounds(
            elements[index].ok_or_else(invalid_projection)?,
            bounds,
            resources,
        )?;
    }
    graph.render(
        &options.with_color_mode(AsciiColorMode::Plain),
        resources,
        execution,
    )
}

fn overview_title(model: &ArchitectureDiagramModel, index: usize) -> &str {
    if index < model.nodes.len() {
        let node = &model.nodes[index];
        if node.node_type == ArchitectureNodeKind::Junction {
            ""
        } else {
            node.title.as_deref().unwrap_or(&node.id)
        }
    } else {
        let group = &model.groups[index - model.nodes.len()];
        group.title.as_deref().unwrap_or(&group.id)
    }
}

/// The complete intermediate batch was admitted before copying any authored title.
fn heading(key: &str, title: &str, resources: &ResourceContext) -> Result<String> {
    let capacity =
        resources.checked_work_add(resources.checked_work_add(key.len(), 1)?, title.len())?;
    let mut text = String::new();
    text.try_reserve_exact(capacity)
        .map_err(|_| AsciiError::allocation_failed("ascii_architecture"))?;
    text.push_str(key);
    text.push(' ');
    let mut start = 0;
    while start < title.len() {
        resources.checkpoint()?;
        let mut end = start.saturating_add(8 * 1024).min(title.len());
        while !title.is_char_boundary(end) {
            end -= 1;
        }
        text.push_str(&title[start..end]);
        start = end;
    }
    Ok(text)
}

fn marker(into: Option<bool>) -> PositionedMarker {
    if into == Some(true) {
        PositionedMarker::Point
    } else {
        PositionedMarker::Open
    }
}

fn side(value: char) -> Result<PositionedSide> {
    match value {
        'L' => Ok(PositionedSide::Left),
        'R' => Ok(PositionedSide::Right),
        'T' => Ok(PositionedSide::Top),
        'B' => Ok(PositionedSide::Bottom),
        _ => Err(invalid_projection()),
    }
}

fn policy(options: &AsciiRenderOptions) -> GraphLayoutPolicy {
    GraphLayoutPolicy {
        routing_space: Default::default(),
        node_padding_x: options.node_padding_x.unwrap_or(options.box_border_padding),
        node_padding_y: options.node_padding_y.unwrap_or(0),
        rank_gap_x: 9,
        rank_gap_y: 5,
        group_padding_x: 3,
        group_padding_y: 3,
        group_title_clearance: 3,
        edge_label_lane_radius: GraphLayoutPolicy::DEFAULT_EDGE_LABEL_LANE_RADIUS,
        structural_charset: options.structural_charset(),
        terminal_width_profile: options.terminal_width_profile,
    }
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve(additional)
        .map_err(|_| AsciiError::allocation_failed("ascii_architecture"))
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: FAMILY,
        feature: "invalid Architecture spatial projection",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiResourcePolicy;
    use merman_model::MermaidConfig;

    #[test]
    fn topology_and_reading_records_cannot_each_spend_a_fresh_work_allowance() {
        let model: ArchitectureDiagramModel = serde_json::from_str(
            r#"{
            "nodes":[{"id":"a","type":"service","title":"Gateway"},
                     {"id":"b","type":"service","title":"Storage"}],
            "edges":[{"lhsId":"a","lhsDir":"R","rhsId":"b","rhsDir":"L","rhsInto":true}]
        }"#,
        )
        .unwrap();
        let options = AsciiRenderOptions::ascii();
        let config = MermaidConfig::empty_object();
        let source = AsciiSourceContext {
            diagram_type: FAMILY,
            config: &config,
            semantic_config: &config,
            title: None,
        };
        let policy = AsciiResourcePolicy::unbounded();
        let scene_ledger = ResourceContext::new(policy);
        let scene_execution = AsciiExecution::for_test(&policy)
            .with_render_ledger(&scene_ledger)
            .with_trimmed_canvas();
        let plan = validate::prepare(&model, &scene_ledger).unwrap().unwrap();
        let mut scene_resources =
            scene_execution.resource_context(&scene_ledger, OperationPhase::Layout);
        let topology = render_topology(
            &model,
            &plan,
            &options,
            &mut scene_resources,
            scene_execution,
        )
        .unwrap();
        let scene_work = scene_ledger.layout_work_used();

        let record_ledger = ResourceContext::new(policy);
        crate::output::render_reading_primary(
            &AsciiDiagram::Architecture(&model),
            source,
            &options,
            AsciiExecution::for_test(&policy).with_render_ledger(&record_ledger),
            Some(&topology),
        )
        .unwrap();
        let record_work = record_ledger.layout_work_used();
        assert!(scene_work > 0 && record_work > 0);

        // Each phase fits this allowance separately. The complete operation must account
        // for both, even though its speculative canvas and final document have local cells.
        let limited = policy
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                scene_work.max(record_work),
            )
            .unwrap();
        let ledger = ResourceContext::new(limited);
        let result = render(
            &model,
            &options,
            source,
            AsciiExecution::for_test(&limited).with_render_ledger(&ledger),
        );
        assert!(
            matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
        );
    }
}
