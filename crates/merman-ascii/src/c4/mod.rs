//! C4-owned declaration-order topology with complete keyed reading records.

mod pack;
mod validate;

use crate::graph::positioned::{
    PositionedEdge, PositionedGraph, PositionedMarker, PositionedShape, PositionedSize,
    PositionedStroke, PositionedTextAdmission,
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
use merman_model::families::c4::{C4DiagramModel, C4Text};

const FAMILY: &str = "c4";
const LABEL_WIDTH: usize = 18;

/// A one-to-one reading name for each recognized view, rather than a source-language header.
pub(crate) fn view_name(value: &str) -> Option<&'static str> {
    match value {
        "C4Context" => Some("Context"),
        "C4Container" => Some("Container"),
        "C4Component" => Some("Component"),
        "C4Dynamic" => Some("Dynamic"),
        "C4Deployment" => Some("Deployment"),
        _ => None,
    }
}

/// Only literal text has a spatial spelling. All other typed values remain exact records.
fn text(value: &C4Text) -> Option<&str> {
    match value {
        C4Text::String(value) => Some(value),
        C4Text::Wrapped { text } | C4Text::Value(text) => text.as_str(),
    }
}

fn role(value: &str) -> Option<&'static str> {
    Some(match value {
        "person" => "Person",
        "external_person" => "External person",
        "system" => "System",
        "external_system" => "External system",
        "system_db" => "System DB",
        "external_system_db" => "External system DB",
        "system_queue" => "System queue",
        "external_system_queue" => "External system queue",
        "container" => "Container",
        "external_container" => "External container",
        "container_db" => "Container DB",
        "external_container_db" => "External container DB",
        "container_queue" => "Container queue",
        "external_container_queue" => "External container queue",
        "component" => "Component",
        "external_component" => "External component",
        "component_db" => "Component DB",
        "external_component_db" => "External component DB",
        "component_queue" => "Component queue",
        "external_component_queue" => "External component queue",
        _ => return None,
    })
}

pub(crate) fn render(
    model: &C4DiagramModel,
    options: &AsciiRenderOptions,
    source: AsciiSourceContext<'_>,
    execution: AsciiExecution<'_>,
) -> Result<crate::output::RenderedPrimary> {
    let resources = execution.new_resource_context(OperationPhase::Semantic);
    let topology = if let Some(hierarchy) = validate::prepare(model, &resources)? {
        let scene_resources = resources.scoped();
        let scene_execution = execution
            .with_render_ledger(&scene_resources)
            .with_trimmed_canvas()
            .with_viewport(AsciiViewportPolicy::unrestricted());
        let mut layout = scene_execution.resource_context(&scene_resources, OperationPhase::Layout);
        Some(render_topology(
            model,
            &hierarchy,
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
        &AsciiDiagram::C4(model),
        source,
        options,
        execution,
        topology.as_deref(),
    )
    .map(|text| crate::output::RenderedPrimary::records(text, projection))
}

fn render_topology(
    model: &C4DiagramModel,
    hierarchy: &validate::Hierarchy,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let count = resources.checked_grid_add(model.shapes.len(), model.boundaries.len())?;
    let mut ids = Vec::new();
    let mut headings = Vec::new();
    // Admit generated keys and role headings before allocating the intermediate source batch.
    // A key's decimal width is bounded by its collection count, independently of alias length.
    let key_width = count.max(model.rels.len()).max(1).ilog10() as usize + 2;
    let node_bytes = resources.checked_work_mul(
        count,
        resources.checked_work_add(resources.checked_work_mul(key_width, 2)?, 32)?,
    )?;
    let edge_width = if model.c4_type == "C4Dynamic" {
        resources.checked_work_add(resources.checked_work_mul(key_width, 2)?, 1)?
    } else {
        key_width
    };
    let generated_bytes = resources.checked_work_add(
        node_bytes,
        resources.checked_work_mul(model.rels.len(), edge_width)?,
    )?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, generated_bytes)?;
    resources.charge_layout_work(generated_bytes)?;
    reserve(&mut ids, count)?;
    reserve(&mut headings, count)?;
    for (index, shape) in model.shapes.iter().enumerate() {
        let id = format!("S{}", index + 1);
        headings.push(format!(
            "{id} [{}]",
            text(&shape.type_c4_shape)
                .and_then(role)
                .ok_or_else(invalid_projection)?
        ));
        ids.push(id);
    }
    for index in 0..model.boundaries.len() {
        let id = format!("B{}", index + 1);
        headings.push(id.clone());
        ids.push(id);
    }
    let mut edge_labels = Vec::new();
    reserve(&mut edge_labels, model.rels.len())?;
    for index in 0..model.rels.len() {
        // Complete captions, technologies and offsets are keyed below. Route keys keep dense
        // row layouts readable without truncating, conflating, or dropping a relationship.
        edge_labels.push(if model.c4_type == "C4Dynamic" {
            format!("{}: E{}", index + 1, index + 1)
        } else {
            format!("E{}", index + 1)
        });
    }
    let profile = options.terminal_width_profile;
    let mut plans = Vec::new();
    reserve(&mut plans, count)?;
    plans.resize_with(count, || None);
    let mut admission = PositionedTextAdmission::default();
    admission.include_retained_bytes(generated_bytes, resources)?;
    for &index in &hierarchy.preorder {
        resources.charge_layout_work(1)?;
        if hierarchy.hidden[index] {
            continue;
        }
        let fragments = if index < model.shapes.len() {
            let shape = &model.shapes[index];
            [
                &headings[index][..],
                text(&shape.label).ok_or_else(invalid_projection)?,
            ]
        } else {
            let boundary = &model.boundaries[index - model.shapes.len()];
            [
                &headings[index][..],
                text(&boundary.label).ok_or_else(invalid_projection)?,
            ]
        };
        let section = DeferredGraphLabelSectionPlan::try_joined_with_wrap(
            fragments,
            None,
            Some(LABEL_WIDTH),
            FAMILY,
            profile,
            resources,
        )?
        .ok_or_else(invalid_projection)?;
        // PositionedGraph replays these prepared rows without a graph-wide wrapping policy.
        let plan = DeferredGraphNodeLabelPlan::single(section, None, FAMILY, profile, resources)?;
        admission.include_label(&plan, resources)?;
        admission.include_copy_work(resources.checked_work_mul(ids[index].len(), 2)?, resources)?;
        if index >= model.shapes.len() {
            for &child in &hierarchy.children[index] {
                resources.charge_layout_work(1)?;
                admission.include_copy_work(ids[child].len(), resources)?;
            }
        }
        plans[index] = Some(plan);
    }
    for (&(from, to), label) in hierarchy.endpoints.iter().zip(&edge_labels) {
        resources.charge_layout_work(1)?;
        admission.include_copy_work(ids[from].len(), resources)?;
        admission.include_copy_work(ids[to].len(), resources)?;
        admission.include_copy_work(label.len(), resources)?;
        admission.include_edge_label(label, profile, resources)?;
    }
    admission.admit(resources)?;
    let mut graph =
        PositionedGraph::try_new(FAMILY, policy(options), count, model.rels.len(), resources)?;
    let mut items = pack::items(resources.checked_grid_add(count, 1)?)?;
    for &index in &hierarchy.preorder {
        resources.charge_layout_work(1)?;
        if hierarchy.hidden[index] {
            continue;
        }
        let prepared = plans[index]
            .take()
            .ok_or_else(invalid_projection)?
            .materialize_after_admission(resources)?;
        let (element, minimum) = if index < model.shapes.len() {
            graph.try_add_node(&ids[index], prepared, PositionedShape::Rect, resources)?
        } else {
            graph.try_add_frame(
                &ids[index],
                prepared,
                hierarchy.children[index]
                    .iter()
                    .map(|&child| ids[child].as_str()),
                resources,
            )?
        };
        items[index].element = Some(element);
        items[index].minimum = minimum;
        if index >= model.shapes.len() {
            items[index].inset = PositionedSize {
                width: 2,
                height: minimum.height - 2,
            };
        }
    }
    for (index, (&(from, to), relation)) in hierarchy.endpoints.iter().zip(&model.rels).enumerate()
    {
        let (start_marker, end_marker) = match relation.rel_type.as_str() {
            "rel_b" => (PositionedMarker::Point, PositionedMarker::Open),
            "birel" => (PositionedMarker::Point, PositionedMarker::Point),
            _ => (PositionedMarker::Open, PositionedMarker::Point),
        };
        graph.try_add_edge(
            PositionedEdge {
                id: &edge_labels[index],
                from: &ids[from],
                to: &ids[to],
                label: &edge_labels[index],
                stroke: PositionedStroke::Normal,
                start_marker,
                end_marker,
            },
            resources,
        )?;
    }
    pack::pack(hierarchy, model.shapes.len(), &mut items, resources)?;
    pack::place(hierarchy, &mut items, &mut graph, resources)?;
    graph.render(
        &options.with_color_mode(AsciiColorMode::Plain),
        resources,
        execution,
    )
}

fn policy(options: &AsciiRenderOptions) -> GraphLayoutPolicy {
    GraphLayoutPolicy {
        routing_space: Default::default(),
        node_padding_x: options.node_padding_x.unwrap_or(options.box_border_padding),
        node_padding_y: options.node_padding_y.unwrap_or(0),
        rank_gap_x: pack::COLUMN_GAP,
        rank_gap_y: pack::ROW_GAP,
        group_padding_x: 2,
        group_padding_y: 2,
        group_title_clearance: 3,
        edge_label_lane_radius: GraphLayoutPolicy::DEFAULT_EDGE_LABEL_LANE_RADIUS,
        structural_charset: options.structural_charset(),
        terminal_width_profile: options.terminal_width_profile,
    }
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve(additional)
        .map_err(|_| allocation_failed())
}
fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_c4")
}
fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: FAMILY,
        feature: "invalid C4 spatial projection",
    }
}
