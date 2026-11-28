use std::{collections::hash_map::Entry, sync::Arc};

use merman_layout_elk as elk;
use merman_model::{
    families::flowchart::{FlowEdge, FlowEdgeVisibility, FlowchartModel, FlowchartRenderContext},
    presentation::PresentationConfig,
};
use merman_render_core::{
    Error, Result,
    config::config_diagram_look,
    elk_adapter::{
        arrowheads::{Arrowhead, EdgeArrowheads},
        feedback_edges::{FeedbackOrientation, orient_feedback_edges_with_arrowheads},
        hierarchy::charge_adapter_work,
    },
    layout_work::OperationLayoutWorkControl,
    math::MathRenderer,
    resources::OperationWorkMeter,
    text::TextMeasurer,
};
use rustc_hash::FxHashMap;

use super::{
    FlowchartElkProjection, build_flowchart_elk_graph_with_render_labels_and_work_control,
    project_layout,
};
use crate::{
    class_style::FlowchartClassDefinitions,
    edge_svg::arrow_types_for_edge,
    layout_config::FlowchartConfigView,
    projection::{FlowchartEdgeTransportPlan, FlowchartRenderModelRef},
    style::FlowchartEdgeStylePlan,
    svg_label::FlowchartSvgLabelSidecarBuilder,
};

/// Operation-owned resources and the selected ELK provider for this layout.
pub struct FlowchartElkLayoutExecution<'a> {
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    operation_seed: elk::ElkOperationSeed,
    algorithm: elk::Algorithm,
    svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &'a FlowchartEdgeStylePlan,
    work_meter: Arc<OperationWorkMeter>,
}

impl<'a> FlowchartElkLayoutExecution<'a> {
    pub fn new(
        measurer: &'a dyn TextMeasurer,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        operation_seed: elk::ElkOperationSeed,
        algorithm: elk::Algorithm,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        edge_style_plan: &'a FlowchartEdgeStylePlan,
        work_meter: Arc<OperationWorkMeter>,
    ) -> Self {
        Self {
            measurer,
            math_renderer,
            operation_seed,
            algorithm,
            svg_label_sidecar,
            edge_style_plan,
            work_meter,
        }
    }
}

/// Builds, solves, and projects the Flow graph under one layout work control.
pub fn layout(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    class_defs: &FlowchartClassDefinitions<'_>,
    effective_config: &PresentationConfig,
    execution: FlowchartElkLayoutExecution<'_>,
) -> Result<FlowchartElkProjection> {
    let mut work_control = OperationLayoutWorkControl::new(execution.work_meter);
    let mut graph = build_flowchart_elk_graph_with_render_labels_and_work_control(
        model,
        render_label_sources,
        class_defs,
        effective_config,
        execution.algorithm,
        execution.measurer,
        execution.math_renderer,
        execution.svg_label_sidecar,
        execution.edge_style_plan,
        Some(&mut work_control),
    )?;
    let render_model = FlowchartRenderModelRef::new(model, render_label_sources);
    let orientation = prepare_provider_graph(
        render_model,
        effective_config,
        &mut graph,
        &mut Some(&mut work_control),
    )?;
    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        elk::layout_with_operation_seed_and_work_control(
            orientation.graph(),
            execution.operation_seed,
            &mut work_control,
        )
    };
    let mut layout = solved.map_err(|error| work_control.map_elk_error(error))?;
    orientation.restore(&mut layout, &mut Some(&mut work_control))?;
    let settings = FlowchartConfigView::new(effective_config).layout_settings();
    project_layout(
        render_model,
        settings,
        &graph,
        layout,
        Some(&mut work_control),
    )
}

/// Solves an already measured diagnostic graph with the provider's raw seed policy.
#[cfg(any(test, feature = "test-support"))]
pub fn layout_diagnostic(
    model: FlowchartRenderModelRef<'_>,
    effective_config: &PresentationConfig,
    mut graph: elk::Graph,
) -> Result<FlowchartElkProjection> {
    let mut no_work = None;
    let orientation = prepare_provider_graph(model, effective_config, &mut graph, &mut no_work)?;
    let mut layout = elk::layout(orientation.graph()).map_err(|error| Error::InvalidModel {
        message: format!("ELK layout failed: {error}"),
    })?;
    orientation.restore(&mut layout, &mut no_work)?;
    let settings = FlowchartConfigView::new(effective_config).layout_settings();
    project_layout(model, settings, &graph, layout, None)
}

fn prepare_provider_graph<'a>(
    model: FlowchartRenderModelRef<'_>,
    config: &PresentationConfig,
    graph: &'a mut elk::Graph,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<FeedbackOrientation<'a>> {
    // Graph diagnostics expose Mermaid's raw edge ids. Bind transport ids after preparation,
    // reusing its already charged work and the same occurrence order for terminal metadata.
    let projected_edges = model.projected_edges().collect::<Vec<_>>();
    if graph.edges.len() != projected_edges.len() {
        return Err(Error::InvalidModel {
            message: format!(
                "ELK graph edge count {} does not match projected edge count {}",
                graph.edges.len(),
                projected_edges.len()
            ),
        });
    }

    let transport_plan = FlowchartEdgeTransportPlan::for_keyed_edges(
        projected_edges
            .iter()
            .map(|(key, edge)| (*key, edge.semantic)),
    );
    for (transport, (key, projected)) in graph.edges.iter_mut().zip(projected_edges.iter()) {
        if transport.id != projected.semantic.id
            || transport.source != projected.from
            || transport.target != projected.to
        {
            let semantic_index = key.semantic_index();
            return Err(Error::InvalidModel {
                message: format!(
                    "ELK graph edge `{}` is not bound to semantic owner {} (`{}`)",
                    transport.id,
                    semantic_index,
                    model
                        .edges
                        .get(semantic_index)
                        .map(|edge| edge.id.as_str())
                        .unwrap_or("<missing>")
                ),
            });
        }
        let id = transport_plan.id(*key).ok_or_else(|| Error::InvalidModel {
            message: format!(
                "missing ELK transport id for semantic owner {}",
                key.semantic_index()
            ),
        })?;
        if transport.id != id {
            id.clone_into(&mut transport.id);
        }
    }

    let neo = config_diagram_look(config).is_neo();
    let mut animation_by_class = FxHashMap::default();
    orient_feedback_edges_with_arrowheads(graph, config, work, |index, work| {
        let edge = projected_edges[index].1.semantic;
        let mut markers = flowchart_arrowheads(edge);
        if neo
            && [markers.start, markers.end]
                .into_iter()
                .flatten()
                .any(|marker| marker != Arrowhead::Other)
        {
            markers.margin =
                !mermaid_elk_edge_animates(edge, model, &mut animation_by_class, work)?;
        }
        Ok(markers)
    })
}

fn flowchart_arrowheads(edge: &FlowEdge) -> EdgeArrowheads {
    if edge.visibility == FlowEdgeVisibility::Invisible
        || edge.stroke.as_deref() == Some("invisible")
    {
        return EdgeArrowheads::default();
    }
    if edge.edge_type.as_deref() == Some("arrow_hierarchy") {
        return EdgeArrowheads {
            end: Some(Arrowhead::Other),
            ..Default::default()
        };
    }
    let (start, end) = arrow_types_for_edge(edge.edge_type.as_deref());
    let marker = |arrow| match arrow {
        "arrow_point" => Arrowhead::Point,
        "arrow_circle" => Arrowhead::Circle,
        "arrow_cross" => Arrowhead::Cross,
        _ => Arrowhead::Other,
    };
    EdgeArrowheads {
        start: start.map(marker),
        end: end.map(marker),
        margin: false,
    }
}

fn mermaid_elk_edge_animates<'a>(
    edge: &'a FlowEdge,
    model: FlowchartRenderModelRef<'a>,
    animation_by_class: &mut FxHashMap<&'a str, bool>,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<bool> {
    if edge.animate == Some(true) || edge.animation.as_ref().is_some_and(|name| !name.is_empty()) {
        return Ok(true);
    }
    // The ELK source observes raw cssCompiledStyles from classDef. The SVG painter separately
    // resolves actual animation properties and their cascade, including direct link styles.
    for class in &edge.classes {
        charge_adapter_work(work, 1 + class.len().div_ceil(64))?;
        let animated = match animation_by_class.entry(class.as_str()) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let mut animated = false;
                if let Some(styles) = model.class_defs.get(class) {
                    for style in styles {
                        charge_adapter_work(work, 1 + style.len().div_ceil(64))?;
                        if style.contains("animation") {
                            animated = true;
                            break;
                        }
                    }
                }
                *entry.insert(animated)
            }
        };
        if animated {
            return Ok(true);
        }
    }
    Ok(false)
}
