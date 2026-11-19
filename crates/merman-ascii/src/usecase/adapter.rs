use super::validation::{self, Index, reserved_vec};
use crate::error::{AsciiError, Result};
use crate::graph::{
    AsciiGraph, DeferredGraphLabelSectionPlan, DeferredGraphNodeLabelPlan, GraphDirection,
    GraphEdgeAttrs, GraphEdgeMarker, GraphEdgeStroke, GraphGroupStyle, GraphNodeSemantics,
    GraphNodeShape, GraphNodeStyle,
};
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::LabelBreakPolicy;
use merman_model::OperationPhase;
use merman_model::families::usecase::{
    UsecaseActorType, UsecaseArrowType, UsecaseDiagramModel, UsecaseNode, UsecaseNodeKind,
    UsecaseRelationship, UsecaseRelationshipType,
};
use std::fmt::Write;

const FAMILY: &str = "usecase";
const COPY_CHUNK_BYTES: usize = 8 * 1024;

struct TextPlans<'a> {
    nodes: Vec<DeferredGraphNodeLabelPlan<'a>>,
    notes: Vec<DeferredGraphNodeLabelPlan<'a>>,
    boundaries: Vec<DeferredGraphLabelSectionPlan<'a>>,
    relationships: Vec<Option<DeferredGraphLabelSectionPlan<'a>>>,
}

/// The caller owns the transaction. No graph or copied label exists before aggregate admission.
pub(super) fn project(
    model: &UsecaseDiagramModel,
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<AsciiGraph> {
    checkpoint(execution, 0)?;
    validation::preflight(model, resources, execution)?;
    let direction = match model.direction.as_str() {
        "TB" | "TD" => GraphDirection::TopDown,
        "BT" => GraphDirection::BottomTop,
        "LR" => GraphDirection::LeftRight,
        "RL" => GraphDirection::RightLeft,
        _ => return Err(unsupported("direction")),
    };
    let index = validation::validate(model, resources, execution)?;
    let tables = super::json_table::prepare(&model.json_nodes, profile, resources)?;
    let table_bytes = tables.iter().try_fold(0, |bytes, table| {
        add_metric(
            resources,
            AsciiResourceLimitId::MaxOutputBytes,
            add_metric(
                resources,
                AsciiResourceLimitId::MaxOutputBytes,
                bytes,
                table.title.len(),
            )?,
            table.body.len(),
        )
    })?;
    let resources = resources
        .clone()
        .with_additional_retained_output(table_bytes)?;
    let plans = plan_texts(model, &tables, &index, profile, &resources, execution)?;
    materialize(model, &index, plans, direction, &resources, execution)
}

fn plan_texts<'a>(
    model: &'a UsecaseDiagramModel,
    tables: &'a [super::json_table::TableText],
    index: &Index<'_>,
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<TextPlans<'a>> {
    let mut nodes = reserved_vec(resources.checked_work_add(model.nodes.len(), tables.len())?)?;
    let mut notes = reserved_vec(model.notes.len())?;
    let mut boundaries = reserved_vec(model.boundaries.len())?;
    let mut relationships = reserved_vec(model.relationships.len())?;
    let mut work = 0;
    let mut planned_cells = 0;
    let mut retained_cells = 0;
    let mut bytes = 0;
    for (node_index, node) in model.nodes.iter().enumerate() {
        checkpoint(execution, node_index)?;
        let section = section(node_fragments(node), Some(&node.id), profile, resources)?;
        let plan = DeferredGraphNodeLabelPlan::single(section, None, FAMILY, profile, resources)?;
        work = resources.checked_work_add(work, plan.source_materialization_work_units())?;
        planned_cells = add_metric(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            planned_cells,
            plan.document_cells(),
        )?;
        // Deferred node plans retain the original source until the graph consumes the plan.
        // Admit both raw source and normalized expansion, including whitespace/control escapes.
        bytes = add_metric(
            resources,
            AsciiResourceLimitId::MaxOutputBytes,
            bytes,
            plan.materialized_bytes()
                .max(plan.source_materialization_work_units()),
        )?;
        add_copy(key_len(node_index), &mut work, &mut bytes, resources)?;
        nodes.push(plan);
    }
    for (table_index, table) in tables.iter().enumerate() {
        checkpoint(execution, table_index)?;
        let primary = DeferredGraphLabelSectionPlan::try_single_with_break_policy(
            table.title.strip_suffix('\n').unwrap_or(&table.title),
            None,
            FAMILY,
            profile,
            LabelBreakPolicy::LiteralRows,
            resources,
        )?;
        let secondary = DeferredGraphLabelSectionPlan::try_single_with_break_policy(
            table.body.strip_suffix('\n').unwrap_or(&table.body),
            None,
            FAMILY,
            profile,
            LabelBreakPolicy::LiteralRows,
            resources,
        )?;
        let plan =
            DeferredGraphNodeLabelPlan::table(primary, secondary, FAMILY, profile, resources)?;
        work = resources.checked_work_add(work, plan.source_materialization_work_units())?;
        planned_cells = add_metric(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            planned_cells,
            plan.document_cells(),
        )?;
        bytes = add_metric(
            resources,
            AsciiResourceLimitId::MaxOutputBytes,
            bytes,
            plan.materialized_bytes()
                .max(plan.source_materialization_work_units()),
        )?;
        add_copy(
            key_len(model.nodes.len() + table_index),
            &mut work,
            &mut bytes,
            resources,
        )?;
        nodes.push(plan);
    }
    for (note_index, note) in model.notes.iter().enumerate() {
        checkpoint(execution, note_index)?;
        let section = section([note.label.as_str(), "[note]"], None, profile, resources)?;
        let plan = DeferredGraphNodeLabelPlan::single(section, None, FAMILY, profile, resources)?;
        work = resources.checked_work_add(work, plan.source_materialization_work_units())?;
        planned_cells = add_metric(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            planned_cells,
            plan.document_cells(),
        )?;
        bytes = add_metric(
            resources,
            AsciiResourceLimitId::MaxOutputBytes,
            bytes,
            plan.materialized_bytes()
                .max(plan.source_materialization_work_units()),
        )?;
        // A note declaration and its independent, markerless attachment relationship.
        add_copy(key_len(note_index), &mut work, &mut bytes, resources)?;
        add_copy(key_len(note_index), &mut work, &mut bytes, resources)?;
        add_copy(
            key_len(index.nodes[note.target.as_str()]),
            &mut work,
            &mut bytes,
            resources,
        )?;
        notes.push(plan);
    }
    for (boundary_index, boundary) in model.boundaries.iter().enumerate() {
        checkpoint(execution, boundary_index)?;
        let title = section(
            [
                boundary.label.as_str(),
                if boundary.package {
                    "[package]"
                } else {
                    "[boundary]"
                },
            ],
            Some(&boundary.id),
            profile,
            resources,
        )?;
        admit_section(
            &title,
            &mut work,
            &mut retained_cells,
            &mut bytes,
            resources,
        )?;
        add_copy(key_len(boundary_index), &mut work, &mut bytes, resources)?;
        for (member_index, member) in boundary.members.iter().enumerate() {
            checkpoint(execution, member_index)?;
            add_copy(
                key_len(index.nodes[member.as_str()]),
                &mut work,
                &mut bytes,
                resources,
            )?;
        }
        for &note in &index.notes_by_boundary[boundary_index] {
            resources.checkpoint()?;
            add_copy(key_len(note), &mut work, &mut bytes, resources)?;
        }
        boundaries.push(title);
    }
    for (relationship_index, relationship) in model.relationships.iter().enumerate() {
        checkpoint(execution, relationship_index)?;
        let label = DeferredGraphLabelSectionPlan::try_joined(
            relationship_fragments(relationship),
            None,
            FAMILY,
            profile,
            resources,
        )?;
        if let Some(label) = &label {
            admit_section(label, &mut work, &mut retained_cells, &mut bytes, resources)?;
        }
        add_copy(relationship.id.len(), &mut work, &mut bytes, resources)?;
        add_copy(
            key_len(index.nodes[relationship.source.as_str()]),
            &mut work,
            &mut bytes,
            resources,
        )?;
        add_copy(
            key_len(index.nodes[relationship.target.as_str()]),
            &mut work,
            &mut bytes,
            resources,
        )?;
        relationships.push(label);
    }
    resources.check_usage(
        work,
        add_metric(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            planned_cells,
            retained_cells,
        )?,
    )?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, bytes)?;
    // Node cells belong to the graph's prepared-label consumption. Group/edge strings remain
    // retained by this projection, so charge those cells here, once, after aggregate admission.
    resources.charge_usage(work, retained_cells)?;
    Ok(TextPlans {
        nodes,
        notes,
        boundaries,
        relationships,
    })
}

fn materialize(
    model: &UsecaseDiagramModel,
    index: &Index<'_>,
    plans: TextPlans<'_>,
    direction: GraphDirection,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<AsciiGraph> {
    let mut graph = AsciiGraph::new_for_diagram(FAMILY, direction);
    graph.try_reserve_projection(
        resources.checked_work_add(plans.nodes.len(), model.notes.len())?,
        resources.checked_work_add(model.relationships.len(), model.notes.len())?,
        model.boundaries.len(),
    )?;
    graph.use_incoming_edge_roots();
    for (node_index, plan) in plans.nodes.into_iter().enumerate() {
        checkpoint(execution, node_index)?;
        let shape = match model.nodes.get(node_index).map(|node| node.kind) {
            Some(UsecaseNodeKind::UseCaseEllipse) => GraphNodeShape::Stadium,
            _ => GraphNodeShape::Rect,
        };
        graph.add_node_with_prepared_text(
            key('n', node_index)?,
            plan.materialize_after_admission(resources)?,
            shape,
            GraphNodeStyle::default(),
            GraphNodeSemantics::default(),
        );
    }
    for (note_index, plan) in plans.notes.into_iter().enumerate() {
        checkpoint(execution, note_index)?;
        graph.add_node_with_prepared_text(
            key('t', note_index)?,
            plan.materialize_after_admission(resources)?,
            GraphNodeShape::Document,
            GraphNodeStyle::default(),
            GraphNodeSemantics::default(),
        );
    }
    for (boundary_index, (boundary, title)) in
        model.boundaries.iter().zip(plans.boundaries).enumerate()
    {
        checkpoint(execution, boundary_index)?;
        let attached_notes = &index.notes_by_boundary[boundary_index];
        let mut members = reserved_vec(
            resources.checked_work_add(boundary.members.len(), attached_notes.len())?,
        )?;
        for (member_index, member) in boundary.members.iter().enumerate() {
            checkpoint(execution, member_index)?;
            members.push(key('n', index.nodes[member.as_str()])?);
        }
        for &note in attached_notes {
            resources.checkpoint()?;
            members.push(key('t', note)?);
        }
        graph.add_group_with_style(
            key('g', boundary_index)?,
            title.materialize_normalized_after_admission(resources)?,
            None,
            members,
            GraphGroupStyle::default(),
        );
    }
    for (relationship_index, (relationship, label)) in model
        .relationships
        .iter()
        .zip(plans.relationships)
        .enumerate()
    {
        checkpoint(execution, relationship_index)?;
        let (start_marker, end_marker) = markers(relationship.arrow_type);
        graph.add_edge_with_attrs(
            key('n', index.nodes[relationship.source.as_str()])?,
            key('n', index.nodes[relationship.target.as_str()])?,
            GraphEdgeAttrs {
                id: Some(copy_text(&relationship.id, resources)?),
                is_user_defined_id: relationship.explicit_id,
                label: label
                    .map(|label| label.materialize_normalized_after_admission(resources))
                    .transpose()?,
                stroke: if relationship.dotted {
                    GraphEdgeStroke::Dotted
                } else {
                    GraphEdgeStroke::Normal
                },
                start_marker,
                end_marker,
                length: relationship.minlen,
                ..GraphEdgeAttrs::default()
            },
        );
    }
    for (note_index, note) in model.notes.iter().enumerate() {
        checkpoint(execution, note_index)?;
        graph.add_edge_with_attrs(
            key('n', index.nodes[note.target.as_str()])?,
            key('t', note_index)?,
            GraphEdgeAttrs {
                stroke: GraphEdgeStroke::Dotted,
                end_marker: GraphEdgeMarker::Open,
                ..GraphEdgeAttrs::default()
            },
        );
    }
    Ok(graph)
}

fn node_fragments(node: &UsecaseNode) -> impl Iterator<Item = &str> {
    let role = match node.kind {
        UsecaseNodeKind::Actor => match node.actor_type {
            UsecaseActorType::Normal => "[actor]",
            UsecaseActorType::Hollow => "[actor: hollow]",
            UsecaseActorType::Awesome => "[actor: awesome]",
            UsecaseActorType::Icon => "[actor: icon]",
        },
        UsecaseNodeKind::UseCaseEllipse => "[use case]",
        UsecaseNodeKind::UseCaseRect => "[use case: rect]",
    };
    [
        if node.label.is_empty() {
            node.id.as_str()
        } else {
            node.label.as_str()
        },
        role,
    ]
    .into_iter()
    .chain(node.business.then_some("[business]"))
    .chain(
        node.stereotype
            .as_deref()
            .into_iter()
            .flat_map(|value| ["[stereotype]", value]),
    )
    .chain(
        node.icon
            .as_deref()
            .into_iter()
            .flat_map(|value| ["[icon]", value]),
    )
}

fn relationship_fragments(relationship: &UsecaseRelationship) -> impl Iterator<Item = &str> {
    let kind = match relationship.relationship_type {
        UsecaseRelationshipType::Association => None,
        UsecaseRelationshipType::Include => Some("<<include>>"),
        UsecaseRelationshipType::Extend => Some("<<extend>>"),
        UsecaseRelationshipType::Generalization => Some("<<generalization>>"),
    };
    let label = relationship.label.as_deref().filter(|label| {
        !matches!(
            (relationship.relationship_type, *label),
            (UsecaseRelationshipType::Include, "include")
                | (UsecaseRelationshipType::Extend, "extend")
        )
    });
    kind.into_iter().chain(label)
}

pub(super) fn markers(arrow: UsecaseArrowType) -> (GraphEdgeMarker, GraphEdgeMarker) {
    use GraphEdgeMarker::{Circle, Cross, Open, Point};
    match arrow {
        UsecaseArrowType::Point => (Open, Point),
        UsecaseArrowType::ReversedPoint => (Point, Open),
        UsecaseArrowType::Line => (Open, Open),
        UsecaseArrowType::Circle => (Open, Circle),
        UsecaseArrowType::Cross => (Open, Cross),
        UsecaseArrowType::ReversedCircle => (Circle, Open),
        UsecaseArrowType::ReversedCross => (Cross, Open),
    }
}

fn section<'a>(
    fragments: impl IntoIterator<Item = &'a str>,
    fallback: Option<&'a str>,
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<DeferredGraphLabelSectionPlan<'a>> {
    DeferredGraphLabelSectionPlan::try_joined(fragments, fallback, FAMILY, profile, resources)?
        .ok_or_else(|| unsupported("missing projected label"))
}

fn admit_section(
    plan: &DeferredGraphLabelSectionPlan<'_>,
    work: &mut usize,
    cells: &mut usize,
    bytes: &mut usize,
    resources: &ResourceContext,
) -> Result<()> {
    *work = resources.checked_work_add(
        *work,
        plan.normalized_materialization_work_units(resources)?,
    )?;
    *cells = add_metric(
        resources,
        AsciiResourceLimitId::MaxDocumentCells,
        *cells,
        plan.document_cells(),
    )?;
    *bytes = add_metric(
        resources,
        AsciiResourceLimitId::MaxOutputBytes,
        *bytes,
        plan.normalized_joined_bytes(resources)?,
    )?;
    Ok(())
}

fn add_copy(
    length: usize,
    work: &mut usize,
    bytes: &mut usize,
    resources: &ResourceContext,
) -> Result<()> {
    *work = resources.checked_work_add(*work, length)?;
    *bytes = add_metric(
        resources,
        AsciiResourceLimitId::MaxOutputBytes,
        *bytes,
        length,
    )?;
    Ok(())
}

fn add_metric(
    resources: &ResourceContext,
    limit: AsciiResourceLimitId,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.overflow(limit))
}

fn key_len(index: usize) -> usize {
    index.checked_ilog10().unwrap_or(0) as usize + 2
}

fn key(prefix: char, index: usize) -> Result<String> {
    let mut text = String::new();
    text.try_reserve_exact(key_len(index))
        .map_err(|_| allocation_failed())?;
    write!(text, "{prefix}{index}").map_err(|_| allocation_failed())?;
    Ok(text)
}

fn copy_text(value: &str, resources: &ResourceContext) -> Result<String> {
    let mut text = String::new();
    text.try_reserve_exact(value.len())
        .map_err(|_| allocation_failed())?;
    let mut start = 0;
    while start < value.len() {
        resources.checkpoint()?;
        let mut end = start.saturating_add(COPY_CHUNK_BYTES).min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        text.push_str(&value[start..end]);
        start = end;
    }
    Ok(text)
}

pub(super) fn checkpoint(execution: AsciiExecution<'_>, iteration: usize) -> Result<()> {
    execution.checkpoint_loop(OperationPhase::Semantic, iteration)
}

pub(super) fn unsupported(feature: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: FAMILY,
        feature,
    }
}

pub(super) fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_layout_work")
}
