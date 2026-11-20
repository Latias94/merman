//! Agentflow roles and relationships over shared graph geometry, with complete reading records.

use crate::graph::{
    AsciiGraph, DeferredGraphLabelSectionPlan, DeferredGraphNodeLabelPlan, GraphDirection,
    GraphEdgeAttrs, GraphEdgeMarker, GraphEdgeStroke, GraphGroupStyle, GraphNodeSemantics,
    GraphNodeShape, GraphNodeStyle,
};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, GraphLayoutPolicy, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::try_clone_layout_text;
use crate::{AsciiDiagram, AsciiError, AsciiProjection, AsciiSourceContext, Result};
use merman_model::OperationPhase;
use merman_model::families::agentflow::{
    AgentflowDiagramModel, AgentflowEdgeSemantic, AgentflowVertexKind,
};
use std::collections::HashMap;

const FAMILY: &str = "agentflow";

pub(crate) fn render(
    model: &AgentflowDiagramModel,
    options: &AsciiRenderOptions,
    policy: GraphLayoutPolicy,
    source: AsciiSourceContext<'_>,
    execution: AsciiExecution<'_>,
) -> Result<crate::output::RenderedPrimary> {
    let resources = execution.new_resource_context(OperationPhase::Semantic);
    if source.title.is_some() || !spatially_eligible(model, &resources)? {
        return crate::output::render_reading_primary(
            &AsciiDiagram::Agentflow(model),
            source,
            options,
            execution,
            None,
        )
        .map(|text| {
            crate::output::RenderedPrimary::records(text, AsciiProjection::StructuredText)
        });
    }
    resources
        .transaction(|resources| {
            let result = resources.transaction_preserving_layout_work(|resources| {
                let graph = project(model, options.terminal_width_profile, resources)?;
                let mut layout_resources =
                    execution.resource_context(resources, OperationPhase::Layout);
                crate::graph::render_graph_with_resolved_policy_and_execution(
                    &graph,
                    options,
                    policy,
                    &mut layout_resources,
                    execution,
                )
            });
            match result {
                Err(error)
                    if execution
                        .preserves_graph_work_on_error(&error, options.graph_presentation) =>
                {
                    Ok(Err(error))
                }
                Ok(text) => Ok(Ok(text)),
                Err(error) => Err(error),
            }
        })?
        .map(|text| crate::output::RenderedPrimary::new(text, AsciiProjection::Diagrammatic))
}

/// Admit only scenes whose visible geometry can carry their domain facts. Rich metadata and
/// collapsed/ambiguous containment remain explicit reading records, not a Flowchart conversion.
fn spatially_eligible(model: &AgentflowDiagramModel, resources: &ResourceContext) -> Result<bool> {
    resources.checkpoint()?;
    if direction(&model.direction).is_none()
        || !model.diagnostics.is_empty()
        || !model.warning_facts.is_empty()
        || model.title.is_some()
        || model.acc_title.is_some()
        || model.acc_descr.is_some()
        || !model.authored.class_defs.is_empty()
        || model.authored.edge_defaults.is_some()
        || !model.authored.tooltips.is_empty()
    {
        return Ok(false);
    }
    let count = resources.checked_work_add(model.vertices.len(), model.connectors.len())?;
    let count = resources.checked_work_add(count, model.sub_graphs.len())?;
    resources.charge_layout_work_product(count, 4)?;
    let mut ids = HashMap::new();
    ids.try_reserve(count).map_err(|_| allocation_failed())?;
    for node in &model.vertices {
        resources.checkpoint()?;
        resources.charge_layout_work(node.id.len().max(1))?;
        if ids.insert(node.id.as_str(), false).is_some()
            || node
                .shape
                .as_deref()
                .is_some_and(|shape| !shape_matches_role(shape, node.vertex_kind))
            || rich_metadata(&node.metadata, resources)?
        {
            return Ok(false);
        }
    }
    for connector in &model.connectors {
        resources.checkpoint()?;
        resources.charge_layout_work(connector.id.len().max(1))?;
        if ids.insert(connector.id.as_str(), false).is_some() || !connector.metadata.is_empty() {
            return Ok(false);
        }
    }
    for group in &model.sub_graphs {
        resources.checkpoint()?;
        resources.charge_layout_work(group.id.len().max(1))?;
        if ids.insert(group.id.as_str(), true).is_some()
            || !group.metadata.is_empty()
            || group.sub_graph_type != "flow"
            || group
                .direction
                .as_deref()
                .is_some_and(|value| direction(value).is_none())
        {
            return Ok(false);
        }
    }
    let mut parents = HashMap::new();
    parents
        .try_reserve(count)
        .map_err(|_| allocation_failed())?;
    for group in &model.sub_graphs {
        for member in &group.nodes {
            resources.checkpoint()?;
            resources.charge_layout_work(member.len().max(1))?;
            if !ids.contains_key(member.as_str())
                || parents.insert(member.as_str(), group.id.as_str()).is_some()
            {
                return Ok(false);
            }
        }
    }
    for group in &model.sub_graphs {
        let mut current = group.id.as_str();
        let mut depth = 0;
        while let Some(&parent) = parents.get(current) {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            depth += 1;
            resources.check_nesting_depth(depth)?;
            if depth > model.sub_graphs.len() {
                return Ok(false);
            }
            current = parent;
        }
    }
    for node in &model.vertices {
        resources.checkpoint()?;
        resources.charge_layout_work(node.id.len().max(1))?;
        if node.parent_id.as_deref() != parents.get(node.id.as_str()).copied() {
            return Ok(false);
        }
    }
    for edge in &model.edges {
        resources.checkpoint()?;
        resources
            .charge_layout_work(resources.checked_work_add(edge.start.len(), edge.end.len())?)?;
        if !ids.contains_key(edge.start.as_str())
            || !ids.contains_key(edge.end.as_str())
            || !edge.metadata.is_empty()
            || !matches!(
                (
                    edge.edge_semantic,
                    edge.edge_type.as_str(),
                    edge.stroke.as_str()
                ),
                (AgentflowEdgeSemantic::Sequence, "arrow_point", "normal")
                    | (AgentflowEdgeSemantic::Reference, "arrow_open", "dotted")
                    | (AgentflowEdgeSemantic::Failure, "arrow_cross", "normal")
            )
        {
            return Ok(false);
        }
    }
    // The canonical authored maps are hash-backed. Visit every entry before deciding so their
    // iteration order cannot change the charged work or a resource-limit outcome.
    let mut authored_details = false;
    for attributes in model.authored.nodes.values() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        authored_details |= !attributes.styles.is_empty()
            || !attributes.classes.is_empty()
            || attributes.link.is_some()
            || attributes.link_target.is_some()
            || attributes.have_callback
            || attributes
                .label_type
                .as_deref()
                .is_some_and(|kind| !matches!(kind, "text" | "string"));
    }
    for attributes in model.authored.edges.values() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        authored_details |= !attributes.styles.is_empty()
            || !attributes.classes.is_empty()
            || attributes.animate.is_some()
            || attributes.animation.is_some()
            || attributes.interpolate.is_some()
            || attributes
                .label_type
                .as_deref()
                .is_some_and(|kind| !matches!(kind, "text" | "string"));
    }
    for classes in model.authored.subgraph_classes.values() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        authored_details |= !classes.is_empty();
    }
    for kind in model.authored.subgraph_label_types.values() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        authored_details |= !matches!(kind.as_str(), "text" | "string");
    }
    Ok(!authored_details)
}

fn rich_metadata(
    metadata: &serde_json::Map<String, serde_json::Value>,
    resources: &ResourceContext,
) -> Result<bool> {
    for (key, value) in metadata {
        resources.checkpoint()?;
        resources.charge_layout_work(key.len().max(1))?;
        if key != "shape"
            || !value.as_str().is_some_and(|value| {
                merman_model::families::agentflow::ALLOWED_SHAPES.contains(&value)
            })
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn project(
    model: &AgentflowDiagramModel,
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<AsciiGraph> {
    let count = resources.checked_work_add(model.vertices.len(), model.connectors.len())?;
    let mut nodes = reserve_vec(count)?;
    let mut groups = reserve_vec(model.sub_graphs.len())?;
    let mut edges = reserve_vec(model.edges.len())?;
    let mut work = 0usize;
    let mut node_cells = 0usize;
    let mut retained_cells = 0usize;
    let mut bytes = 0usize;
    for (id, label, kind) in model
        .vertices
        .iter()
        .map(|node| (node.id.as_str(), node.label.as_deref(), node.vertex_kind))
        .chain(model.connectors.iter().map(|node| {
            (
                node.id.as_str(),
                node.title.as_deref(),
                AgentflowVertexKind::Connector,
            )
        }))
    {
        resources.checkpoint()?;
        let title = label.unwrap_or(id);
        let section = joined([title, role(kind)], profile, resources)?;
        let plan = DeferredGraphNodeLabelPlan::single(section, None, FAMILY, profile, resources)?;
        work = resources.checked_work_add(work, plan.source_materialization_work_units())?;
        node_cells = resources.checked_grid_add(node_cells, plan.document_cells())?;
        bytes = resources.checked_grid_add(
            bytes,
            plan.materialized_bytes()
                .max(plan.source_materialization_work_units()),
        )?;
        bytes = resources.checked_grid_add(bytes, id.len())?;
        nodes.push((id, kind, plan));
    }
    for group in &model.sub_graphs {
        let title = joined(
            [group.title.as_deref().unwrap_or(&group.id), "[flow]"],
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
        bytes = resources.checked_grid_add(bytes, group.id.len())?;
        for member in &group.nodes {
            resources.checkpoint()?;
            bytes = resources.checked_grid_add(bytes, member.len())?;
        }
        groups.push(title);
    }
    for edge in &model.edges {
        let label = match edge.label.as_deref() {
            Some(label) => DeferredGraphLabelSectionPlan::try_joined(
                [label],
                None,
                FAMILY,
                profile,
                resources,
            )?,
            None => None,
        };
        if let Some(label) = &label {
            admit_section(label, &mut work, &mut retained_cells, &mut bytes, resources)?;
        }
        bytes = resources.checked_grid_add(
            bytes,
            resources.checked_grid_add(edge.start.len(), edge.end.len())?,
        )?;
        bytes = resources.checked_grid_add(bytes, edge.id.as_deref().map_or(0, str::len))?;
        edges.push(label);
    }
    resources.check_usage(
        work,
        resources.checked_grid_add(node_cells, retained_cells)?,
    )?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, bytes)?;
    resources.charge_usage(work, retained_cells)?;
    let root = direction(&model.direction).ok_or(AsciiError::UnsupportedFeature {
        diagram_type: FAMILY,
        feature: "direction",
    })?;
    let mut graph = AsciiGraph::new_for_diagram(FAMILY, root);
    graph.try_reserve_projection(count, edges.len(), groups.len())?;
    graph.use_incoming_edge_roots();
    for (id, kind, plan) in nodes {
        graph.add_node_with_prepared_text(
            try_clone_layout_text(id, resources)?,
            plan.materialize_after_admission(resources)?,
            shape(kind),
            GraphNodeStyle::default(),
            GraphNodeSemantics::default(),
        );
    }
    for (group, title) in model.sub_graphs.iter().zip(groups) {
        let mut members = reserve_vec(group.nodes.len())?;
        for member in &group.nodes {
            members.push(try_clone_layout_text(member, resources)?);
        }
        graph.add_group_with_style(
            try_clone_layout_text(&group.id, resources)?,
            title.materialize_normalized_after_admission(resources)?,
            group.direction.as_deref().and_then(direction),
            members,
            GraphGroupStyle::default(),
        );
    }
    for (edge, label) in model.edges.iter().zip(edges) {
        let (stroke, end_marker) = match edge.edge_semantic {
            AgentflowEdgeSemantic::Sequence => (GraphEdgeStroke::Normal, GraphEdgeMarker::Point),
            AgentflowEdgeSemantic::Reference => (GraphEdgeStroke::Dotted, GraphEdgeMarker::Open),
            AgentflowEdgeSemantic::Failure => (GraphEdgeStroke::Normal, GraphEdgeMarker::Cross),
        };
        graph.add_edge_with_attrs(
            try_clone_layout_text(&edge.start, resources)?,
            try_clone_layout_text(&edge.end, resources)?,
            GraphEdgeAttrs {
                id: edge
                    .id
                    .as_deref()
                    .map(|id| try_clone_layout_text(id, resources))
                    .transpose()?,
                is_user_defined_id: edge.is_user_defined_id,
                label: label
                    .map(|label| label.materialize_normalized_after_admission(resources))
                    .transpose()?,
                stroke,
                end_marker,
                length: edge.length,
                ..Default::default()
            },
        );
    }
    Ok(graph)
}

fn joined<'a>(
    parts: impl IntoIterator<Item = &'a str>,
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<DeferredGraphLabelSectionPlan<'a>> {
    DeferredGraphLabelSectionPlan::try_joined(parts, None, FAMILY, profile, resources)?.ok_or(
        AsciiError::UnsupportedFeature {
            diagram_type: FAMILY,
            feature: "empty label",
        },
    )
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
    *cells = resources.checked_grid_add(*cells, plan.document_cells())?;
    *bytes = resources.checked_grid_add(*bytes, plan.normalized_joined_bytes(resources)?)?;
    Ok(())
}

fn direction(value: &str) -> Option<GraphDirection> {
    Some(match value {
        "TD" | "TB" => GraphDirection::TopDown,
        "BT" => GraphDirection::BottomTop,
        "LR" => GraphDirection::LeftRight,
        "RL" => GraphDirection::RightLeft,
        _ => return None,
    })
}

fn role(kind: AgentflowVertexKind) -> &'static str {
    match kind {
        AgentflowVertexKind::Tool => "[tool]",
        AgentflowVertexKind::Action => "[action]",
        AgentflowVertexKind::Input => "[input]",
        AgentflowVertexKind::Refdoc => "[reference]",
        AgentflowVertexKind::Decision => "[decision]",
        AgentflowVertexKind::Connector => "[connector]",
        AgentflowVertexKind::Task => "[task]",
    }
}

fn shape(kind: AgentflowVertexKind) -> GraphNodeShape {
    match kind {
        AgentflowVertexKind::Tool => GraphNodeShape::Subroutine,
        AgentflowVertexKind::Action => GraphNodeShape::Hexagon,
        AgentflowVertexKind::Input => GraphNodeShape::LeanRight,
        AgentflowVertexKind::Refdoc => GraphNodeShape::LinedDocument,
        AgentflowVertexKind::Decision => GraphNodeShape::Diamond,
        AgentflowVertexKind::Connector => GraphNodeShape::Rounded,
        AgentflowVertexKind::Task => GraphNodeShape::Rounded,
    }
}

fn shape_matches_role(shape: &str, kind: AgentflowVertexKind) -> bool {
    match kind {
        AgentflowVertexKind::Task => matches!(shape, "roundedRect" | "square"),
        AgentflowVertexKind::Tool => matches!(
            shape,
            "subroutine" | "subprocess" | "subproc" | "framed-rectangle"
        ),
        AgentflowVertexKind::Action => matches!(shape, "hexagon" | "hex"),
        AgentflowVertexKind::Input => shape == "lean-right",
        AgentflowVertexKind::Refdoc => matches!(shape, "lin-doc" | "lined-document"),
        AgentflowVertexKind::Decision => shape == "diamond",
        AgentflowVertexKind::Connector => shape == "connector",
    }
}

fn reserve_vec<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve(count).map_err(|_| allocation_failed())?;
    Ok(values)
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_agentflow")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiResourcePolicy;

    #[test]
    fn hash_iteration_does_not_change_spatial_admission_work_at_the_exact_limit() {
        let mut expected = None;
        for _ in 0..24 {
            let parsed = merman_core::SemanticParser::new()
                .parse_diagram_sync("agentflow-beta\nA[Alpha]\nB[Beta]\nstyle A fill:red\n")
                .unwrap();
            let merman_model::SemanticDiagramModel::Agentflow(model) = &parsed.model else {
                panic!("expected Agentflow")
            };
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            assert!(!spatially_eligible(model, &resources).unwrap());
            let work = resources.layout_work_used();
            assert_eq!(*expected.get_or_insert(work), work);
            for (limit, admitted) in [(work, true), (work - 1, false)] {
                let resources = ResourceContext::new(
                    AsciiResourcePolicy::unbounded()
                        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                        .unwrap(),
                );
                assert_eq!(spatially_eligible(model, &resources).is_ok(), admitted);
            }
        }
    }
}
