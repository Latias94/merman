//! Requirement traceability over one shared-endpoint graph and complete keyed reading records.

use crate::graph::{
    AsciiGraph, DeferredGraphLabelSectionPlan, DeferredGraphNodeLabelPlan, GraphDirection,
    GraphEdgeAttrs, GraphEdgeMarker, GraphEdgeStroke, GraphNodeSemantics, GraphNodeShape,
    GraphNodeStyle,
};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, GraphLayoutPolicy, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiProjection, AsciiSourceContext,
    AsciiViewportPolicy, Result,
};
use merman_model::OperationPhase;
use merman_model::families::requirement::RequirementDiagramModel;
use std::collections::HashMap;
use std::num::NonZeroUsize;

const FAMILY: &str = "requirement";
const NODE_LABEL_WIDTH: NonZeroUsize = NonZeroUsize::new(12).unwrap();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReadingKey {
    pub(crate) prefix: char,
    pub(crate) number: usize,
}

impl std::fmt::Display for ReadingKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}{}", self.prefix, self.number)
    }
}

/// Names in independent requirement/element collections can overlap. Keep all declarations;
/// only a unique authored name can become a spatial endpoint.
pub(crate) struct Identities<'a> {
    by_name: HashMap<&'a str, Vec<ReadingKey>>,
}

impl<'a> Identities<'a> {
    pub(crate) fn new(
        model: &'a RequirementDiagramModel,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let count = resources.checked_work_add(model.requirements.len(), model.elements.len())?;
        resources.charge_layout_work(count)?;
        resources.check(AsciiResourceLimitId::MaxDocumentCells, count)?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, count)?;
        let mut by_name: HashMap<&str, Vec<ReadingKey>> = HashMap::new();
        by_name
            .try_reserve(count)
            .map_err(|_| allocation_failed())?;
        for (key, name) in declarations(model) {
            resources.checkpoint()?;
            resources.charge_layout_work(name.len().max(1))?;
            let keys = by_name.entry(name).or_default();
            keys.try_reserve(1).map_err(|_| allocation_failed())?;
            keys.push(key);
        }
        Ok(Self { by_name })
    }

    pub(crate) fn keys(&self, name: &str, resources: &ResourceContext) -> Result<&[ReadingKey]> {
        resources.charge_layout_work(name.len().max(1))?;
        let keys = self.by_name.get(name).map_or(&[][..], Vec::as_slice);
        resources.charge_layout_work(keys.len())?;
        Ok(keys)
    }
}

fn declarations(model: &RequirementDiagramModel) -> impl Iterator<Item = (ReadingKey, &str)> {
    model
        .requirements
        .iter()
        .enumerate()
        .map(|(index, node)| {
            (
                ReadingKey {
                    prefix: 'R',
                    number: index + 1,
                },
                node.name.as_str(),
            )
        })
        .chain(model.elements.iter().enumerate().map(|(index, node)| {
            (
                ReadingKey {
                    prefix: 'N',
                    number: index + 1,
                },
                node.name.as_str(),
            )
        }))
}

fn spatially_eligible(
    model: &RequirementDiagramModel,
    identities: &Identities<'_>,
    resources: &ResourceContext,
) -> Result<bool> {
    if direction(&model.direction).is_none() {
        return Ok(false);
    }
    if model.requirements.is_empty() && model.elements.is_empty() {
        return Ok(false);
    }
    for (_, name) in declarations(model) {
        if identities.keys(name, resources)?.len() != 1 {
            return Ok(false);
        }
    }
    for relation in &model.relationships {
        resources.checkpoint()?;
        resources.charge_layout_work(relation.rel_type.len().max(1))?;
        if !matches!(
            relation.rel_type.as_str(),
            "contains" | "copies" | "derives" | "satisfies" | "verifies" | "refines" | "traces"
        ) || identities.keys(&relation.src, resources)?.len() != 1
            || identities.keys(&relation.dst, resources)?.len() != 1
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn render(
    model: &RequirementDiagramModel,
    options: &AsciiRenderOptions,
    policy: GraphLayoutPolicy,
    source: AsciiSourceContext<'_>,
    execution: AsciiExecution<'_>,
) -> Result<crate::output::RenderedPrimary> {
    let resources = execution.new_resource_context(OperationPhase::Semantic);
    let identities = Identities::new(model, &resources)?;
    let topology = if spatially_eligible(model, &identities, &resources)? {
        // Geometry is one component of the document. It shares cumulative work with the records,
        // but the selected combined document owns the final retained-cell admission.
        let scene_resources = resources.scoped();
        let scene_execution = execution
            .with_render_ledger(&scene_resources)
            .with_trimmed_canvas()
            .with_viewport(AsciiViewportPolicy::unrestricted());
        let graph = project(model, options.terminal_width_profile, &scene_resources)?;
        let mut layout = scene_execution.resource_context(&scene_resources, OperationPhase::Layout);
        Some(
            crate::graph::render_graph_with_resolved_policy_and_execution(
                &graph,
                &options.with_color_mode(AsciiColorMode::Plain),
                policy,
                &mut layout,
                scene_execution,
            )?,
        )
    } else {
        None
    };
    let projection = if topology.is_some() {
        AsciiProjection::Diagrammatic
    } else {
        AsciiProjection::StructuredText
    };
    crate::output::render_reading_primary(
        &AsciiDiagram::Requirement(model),
        source,
        options,
        execution,
        topology.as_deref(),
    )
    .map(|text| crate::output::RenderedPrimary::records(text, projection))
}

pub(crate) fn project(
    model: &RequirementDiagramModel,
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<AsciiGraph> {
    let count = resources.checked_work_add(model.requirements.len(), model.elements.len())?;
    resources.charge_layout_work_product(count, 24)?;
    let identities = Identities::new(model, resources)?;
    let mut keys = reserve_vec(count)?;
    for (key, _) in declarations(model) {
        keys.push(key.to_string());
    }
    let mut plans = reserve_vec(count)?;
    let mut work = 0;
    let mut cells = 0;
    let mut bytes = 0;
    for ((_, name), key) in declarations(model).zip(&keys) {
        let title = DeferredGraphLabelSectionPlan::try_single(
            key,
            Some(NODE_LABEL_WIDTH.get()),
            FAMILY,
            profile,
            resources,
        )?;
        let body = DeferredGraphLabelSectionPlan::try_single(
            name,
            Some(NODE_LABEL_WIDTH.get()),
            FAMILY,
            profile,
            resources,
        )?;
        let plan = DeferredGraphNodeLabelPlan::compartmented(
            title,
            body,
            Some(NODE_LABEL_WIDTH.get()),
            FAMILY,
            profile,
            resources,
        )?;
        work = resources.checked_work_add(work, plan.source_materialization_work_units())?;
        cells = resources.checked_grid_add(cells, plan.document_cells())?;
        bytes = resources.checked_grid_add(
            bytes,
            plan.materialized_bytes()
                .max(plan.source_materialization_work_units()),
        )?;
        plans.push(plan);
    }
    resources.check_usage(work, cells)?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, bytes)?;
    resources.charge_layout_work(work)?;
    let root = direction(&model.direction).ok_or_else(invalid_projection)?;
    let mut graph = AsciiGraph::new_for_diagram(FAMILY, root);
    graph.try_reserve_projection(count, model.relationships.len(), 0)?;
    graph.wrap_node_labels_at(NODE_LABEL_WIDTH);
    graph.use_incoming_edge_roots();
    for (key, plan) in keys.iter().zip(plans) {
        graph.add_node_with_prepared_text(
            key.clone(),
            plan.materialize_after_admission(resources)?,
            GraphNodeShape::Rect,
            GraphNodeStyle::default(),
            GraphNodeSemantics::default(),
        );
    }
    for relation in &model.relationships {
        resources.checkpoint()?;
        let from = unique_key(&identities, &relation.src, resources)?;
        let to = unique_key(&identities, &relation.dst, resources)?;
        resources.charge_layout_work(relation.rel_type.len())?;
        let contains = relation.rel_type == "contains";
        graph.add_edge_with_attrs(
            from.to_string(),
            to.to_string(),
            GraphEdgeAttrs {
                label: Some(relation.rel_type.clone()),
                stroke: if contains {
                    GraphEdgeStroke::Normal
                } else {
                    GraphEdgeStroke::Dotted
                },
                start_marker: if contains {
                    GraphEdgeMarker::Circle
                } else {
                    GraphEdgeMarker::Open
                },
                end_marker: if contains {
                    GraphEdgeMarker::Open
                } else {
                    GraphEdgeMarker::Point
                },
                ..Default::default()
            },
        );
    }
    Ok(graph)
}

fn unique_key(
    identities: &Identities<'_>,
    name: &str,
    resources: &ResourceContext,
) -> Result<ReadingKey> {
    match identities.keys(name, resources)? {
        [key] => Ok(*key),
        _ => Err(invalid_projection()),
    }
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

fn reserve_vec<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve(count).map_err(|_| allocation_failed())?;
    Ok(values)
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: FAMILY,
        feature: "ambiguous traceability graph",
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_requirement")
}
