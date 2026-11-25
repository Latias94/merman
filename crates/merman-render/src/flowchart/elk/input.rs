use crate::config::config_bool;
use crate::elk_options::layout_options as elk_layout_options;
use crate::flowchart::measured::FlowchartMeasurement;
use crate::flowchart::{
    FlowchartClassDefinitions, FlowchartLabelMetricsRequest, FlowchartRenderModelRef,
    FlowchartSvgLabelOwner, FlowchartSvgLabelSidecarBuilder, FlowchartSvgWidthMode,
    flowchart_effective_text_style_for_classes_with_provenance,
    measure_flowchart_svg_label_for_layout_with_source_style,
};
use crate::layout_backend::ElkRootAlgorithm;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use merman_layout_elk as elk;
use merman_model::families::flowchart::{FlowSubgraph, FlowchartModel, FlowchartRenderContext};
use merman_model::presentation::PresentationConfig;
use std::collections::{HashMap, HashSet, VecDeque};

use super::{charge_adapter_work, checked_adapter_add, checked_adapter_mul};

struct ProjectedFlowchartModel {
    model: FlowchartModel,
    node_owner_indices: Vec<usize>,
    edge_owner_indices: Vec<usize>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "ELK preparation keeps renderer dependencies and operation controls explicit"
)]
pub(super) fn build_flowchart_elk_graph_with_render_labels_and_work_control(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    class_defs: &FlowchartClassDefinitions<'_>,
    effective_config: &PresentationConfig,
    algorithm: ElkRootAlgorithm,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    mut work_control: Option<&mut OperationLayoutWorkControl>,
) -> Result<elk::Graph> {
    // Mermaid's FlowDB projects collapsed subgraphs before handing data to any layout backend.
    // Keep the public typed model lossless, but give ELK the same visible graph. The projected
    // model retains subgraph declaration ordinals so source-backed title bindings remain valid.
    if let Some(projected_model) =
        project_collapsed_flowchart_model(model, render_label_sources, &mut work_control)?
    {
        return build_flowchart_elk_graph_with_render_labels_and_work_control_inner(
            &projected_model.model,
            render_label_sources,
            class_defs,
            effective_config,
            algorithm,
            measurer,
            math_renderer,
            svg_label_sidecar,
            edge_style_plan,
            Some(&projected_model.node_owner_indices),
            Some(&projected_model.edge_owner_indices),
            work_control,
        );
    }

    build_flowchart_elk_graph_with_render_labels_and_work_control_inner(
        model,
        render_label_sources,
        class_defs,
        effective_config,
        algorithm,
        measurer,
        math_renderer,
        svg_label_sidecar,
        edge_style_plan,
        None,
        None,
        work_control,
    )
}

fn project_collapsed_flowchart_model(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<Option<ProjectedFlowchartModel>> {
    let has_collapsed_projection = model.subgraphs.iter().any(|subgraph| {
        render_label_sources.is_subgraph_collapsed(subgraph.id.as_str())
            || render_label_sources
                .collapsed_replacement(subgraph.id.as_str())
                .is_some()
    });
    if !has_collapsed_projection {
        return Ok(None);
    }

    let projection_work = checked_adapter_add(
        work_control,
        model.nodes.len(),
        checked_adapter_add(
            work_control,
            model.edges.len(),
            checked_adapter_add(
                work_control,
                model.subgraphs.len(),
                model.vertex_calls.len(),
            )?,
        )?,
    )?;
    charge_adapter_work(work_control, projection_work)?;

    let mut projected = model.clone();
    let mut node_owner_indices = Vec::with_capacity(model.nodes.len());
    projected.nodes = model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            render_label_sources
                .collapsed_replacement(node.id.as_str())
                .is_none()
        })
        .map(|(owner_index, node)| {
            node_owner_indices.push(owner_index);
            node.clone()
        })
        .collect();
    let mut edge_owner_indices = Vec::with_capacity(model.edges.len());
    projected.edges = model
        .edges
        .iter()
        .enumerate()
        .filter_map(|(owner_index, edge)| {
            crate::flowchart::project_flowchart_edge(edge, render_label_sources).map(|projected| {
                edge_owner_indices.push(owner_index);
                projected.into_render_edge().into_owned()
            })
        })
        .collect();
    // Keep the original subgraph vector length/order. Render-label sources are keyed by the
    // declaration ordinal, and filtering hidden descendants here would make later titles read
    // the wrong source-backed spelling (especially for nested collapsed subgraphs).
    projected.subgraphs = model
        .subgraphs
        .iter()
        .map(|subgraph| {
            let mut projected = subgraph.clone();
            projected.nodes.retain(|id| {
                render_label_sources
                    .collapsed_replacement(id.as_str())
                    .is_none()
            });
            projected
        })
        .collect();
    projected.vertex_calls.retain(|id| {
        render_label_sources
            .collapsed_replacement(id.as_str())
            .is_none()
    });

    Ok(Some(ProjectedFlowchartModel {
        model: projected,
        node_owner_indices,
        edge_owner_indices,
    }))
}

#[allow(
    clippy::too_many_arguments,
    reason = "ELK preparation keeps renderer dependencies and operation controls explicit"
)]
fn build_flowchart_elk_graph_with_render_labels_and_work_control_inner(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    class_defs: &FlowchartClassDefinitions<'_>,
    effective_config: &PresentationConfig,
    algorithm: ElkRootAlgorithm,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    node_owner_indices: Option<&[usize]>,
    edge_owner_indices: Option<&[usize]>,
    mut work_control: Option<&mut OperationLayoutWorkControl>,
) -> Result<elk::Graph> {
    let render_model = FlowchartRenderModelRef::new(model, render_label_sources);
    let model = &render_model;
    // Shape validation only walks nodes. Charge that tranche before the validation scan; the
    // hierarchy and edge tranches are charged by their actual owners below.
    charge_adapter_work(&mut work_control, model.nodes.len())?;
    crate::flowchart::validate_flowchart_model_shapes(model)?;
    let measurement = FlowchartMeasurement::new(
        render_model,
        class_defs,
        effective_config,
        measurer,
        math_renderer,
        svg_label_sidecar,
        edge_style_plan,
    );
    let settings = &measurement.settings;

    let diagram_direction = model
        .direction
        .as_deref()
        .map(dir_to_elk_direction)
        .unwrap_or_default();

    let cluster_label_base_style = if settings.cluster_wrap_mode == WrapMode::HtmlLike {
        &settings.html_label_text_style
    } else {
        &settings.text_style
    };

    let mut graph = elk::Graph {
        id: "root".to_string(),
        direction: diagram_direction,
        spacing: elk::Spacing {
            node_node: settings.nodesep,
            layer_layer: settings.ranksep,
            group_padding_x: settings.cluster_padding,
            group_padding_y: settings.cluster_padding,
            ..Default::default()
        },
        options: elk_layout_options(effective_config, algorithm),
        ..Default::default()
    };

    charge_adapter_work(&mut work_control, model.subgraphs.len())?;
    let canonical_subgraphs = canonical_subgraphs(model);
    let subgraph_ids: HashSet<&str> = canonical_subgraphs
        .iter()
        .map(|(_, sg)| sg.id.as_str())
        .collect();
    let parent_by_id = parent_by_id(model, &mut work_control)?;
    let include_children_groups = include_children_groups(model, &parent_by_id, &mut work_control)?;
    let direction_item_count =
        checked_adapter_add(&work_control, model.nodes.len(), model.subgraphs.len())?;
    let node_directions = crate::elk_adapter::resolved_node_directions(
        model
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), None))
            .chain(model.subgraphs.iter().map(|group| {
                (
                    group.id.as_str(),
                    group.dir.as_deref().map(dir_to_elk_direction),
                )
            }))
            .map(|(id, direction)| (id, parent_by_id.get(id).map(String::as_str), direction)),
        direction_item_count,
        diagram_direction,
        &mut work_control,
    )?;

    let cluster_measure_ctx = ElkMeasureContext {
        model: render_model,
        class_defs,
        effective_config,
        measurer,
        math_renderer,
        cluster_label_base_style,
        cluster_title_wrapping_width: settings.wrapping_width,
        cluster_wrap_mode: settings.cluster_wrap_mode,
        class_html_labels: settings.edge_html_labels,
        svg_label_sidecar,
        title_padding: settings.cluster_padding,
    };
    let mut inserted_ids: HashSet<&str> = HashSet::new();
    // FlowDB emits subgraphs in reverse storage order before leaf vertices, and Mermaid derives
    // sibling lists by filtering that canonical array without sorting. Preserve that order here.
    for &(subgraph_index, sg) in canonical_subgraphs.iter().rev() {
        charge_adapter_work(&mut work_control, 1)?;
        if render_label_sources
            .collapsed_replacement(sg.id.as_str())
            .is_some()
        {
            continue;
        }
        if !inserted_ids.insert(sg.id.as_str()) {
            continue;
        }
        graph.nodes.push(subgraph_to_elk_node(
            subgraph_index,
            sg,
            parent_by_id.get(&sg.id).cloned(),
            &include_children_groups,
            &cluster_measure_ctx,
            &mut work_control,
        )?);
    }

    for (node_index, node) in model.nodes.iter().enumerate() {
        charge_adapter_work(&mut work_control, 1)?;
        if subgraph_ids.contains(node.id.as_str()) || !inserted_ids.insert(node.id.as_str()) {
            continue;
        }
        crate::flowchart::validate_flowchart_node_asset_dimensions(node)?;
        let owner_index = node_owner_indices
            .and_then(|indices| indices.get(node_index).copied())
            .unwrap_or(node_index);
        let direction = node_directions
            .get(node.id.as_str())
            .copied()
            .unwrap_or(diagram_direction);
        let measured = measurement.node(
            node,
            FlowchartSvgLabelOwner::Node(owner_index),
            match direction {
                elk::Direction::Left => "RL",
                elk::Direction::Right => "LR",
                elk::Direction::Up => "BT",
                elk::Direction::Down => "TB",
            },
            |units| charge_adapter_work(&mut work_control, units),
        )?;
        graph.nodes.push(elk::Node {
            id: node.id.clone(),
            kind: elk::NodeKind::Leaf,
            container: Default::default(),
            label_text: None,
            width: measured.width,
            height: measured.height,
            parent: parent_by_id.get(&node.id).cloned(),
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_alignment: None,
            label: Some(elk::Label {
                width: measured.label.width,
                height: measured.label.height,
            }),
        });
    }

    if config_bool(effective_config, &["elk", "keepEntryNodeOnTop"]).unwrap_or(false) {
        crate::elk_adapter::apply_cyclic_entry_constraints(
            &mut graph.nodes,
            model
                .edges
                .iter()
                .map(|edge| (edge.from.as_str(), edge.to.as_str())),
            &mut work_control,
        )?;
    }

    charge_adapter_work(&mut work_control, model.edges.len())?;
    let mut edges = Vec::with_capacity(model.edges.len());
    for (edge_index, edge) in model.edges.iter().enumerate() {
        let owner_index = edge_owner_indices
            .and_then(|indices| indices.get(edge_index).copied())
            .unwrap_or(edge_index);
        let label = measurement
            .edge_label(edge, crate::flowchart::FlowchartEdgeKey::new(owner_index))?
            .map(|label| elk::Label {
                width: label.width,
                height: label.height,
            });
        edges.push(elk::Edge {
            id: edge.id.clone(),
            source: edge.from.clone(),
            target: edge.to.clone(),
            label,
            minlen: edge.length.max(1),
            inside_self_loops_yo: false,
            terminal_labels: Vec::new(),
        });
    }
    graph.edges = edges;

    Ok(graph)
}

#[derive(Clone, Copy)]
struct ElkMeasureContext<'a> {
    model: FlowchartRenderModelRef<'a>,
    class_defs: &'a FlowchartClassDefinitions<'a>,
    effective_config: &'a PresentationConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    cluster_label_base_style: &'a TextStyle,
    cluster_title_wrapping_width: f64,
    cluster_wrap_mode: WrapMode,
    class_html_labels: bool,
    title_padding: f64,
    svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
}

pub(super) fn canonical_subgraphs(model: &FlowchartModel) -> Vec<(usize, &FlowSubgraph)> {
    let mut seen = HashSet::with_capacity(model.subgraphs.len());
    let mut canonical = Vec::with_capacity(model.subgraphs.len());
    for (semantic_index, subgraph) in model.subgraphs.iter().enumerate() {
        if seen.insert(subgraph.id.as_str()) {
            canonical.push((semantic_index, subgraph));
        }
    }
    canonical
}

fn dir_to_elk_direction(dir: &str) -> elk::Direction {
    match dir.trim().to_uppercase().as_str() {
        "LR" => elk::Direction::Right,
        "RL" => elk::Direction::Left,
        "BT" => elk::Direction::Up,
        "TB" | "TD" => elk::Direction::Down,
        _ => elk::Direction::Down,
    }
}

pub(super) fn parent_by_id(
    model: &FlowchartModel,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<HashMap<String, String>> {
    let mut parent_by_id = HashMap::new();
    for sg in model.subgraphs.iter().rev() {
        charge_adapter_work(work_control, sg.nodes.len())?;
        for child in &sg.nodes {
            parent_by_id.insert(child.clone(), sg.id.clone());
        }
    }

    if let Some((child, parent)) =
        first_elk_parent_cycle_assignment(model, &parent_by_id, work_control)?
    {
        return Err(Error::InvalidModel {
            message: format!("Setting {parent} as parent of {child} would create a cycle"),
        });
    }

    Ok(parent_by_id)
}

fn first_elk_parent_cycle_assignment(
    model: &FlowchartModel,
    parent_by_id: &HashMap<String, String>,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<Option<(String, String)>> {
    // ELK does not materialize a Graphlib compound graph, so it must validate the final FlowDB
    // parent map itself. Preserve Mermaid's reverse-subgraph assignment order, but keep this check
    // local to the ELK adapter instead of restoring Dagre's obsolete duplicate preflight.
    charge_adapter_work(work_control, model.subgraphs.len())?;
    let mut assigned = HashSet::with_capacity(model.subgraphs.len());
    let mut assignments = Vec::with_capacity(model.subgraphs.len());
    for child in model
        .subgraphs
        .iter()
        .rev()
        .map(|subgraph| subgraph.id.as_str())
    {
        let Some(parent) = parent_by_id.get(child) else {
            continue;
        };
        if assigned.insert(child) {
            assignments.push((child, parent.as_str()));
        }
    }
    if assignments.is_empty() {
        return Ok(None);
    }

    let node_capacity = checked_adapter_mul(work_control, assignments.len(), 2)?;
    // Each retained assignment child has at most one final parent. In that functional graph, an
    // assignment closes a Graphlib parent cycle exactly when it is the latest Mermaid-order edge
    // on one directed cycle. Peel every acyclic node with Kahn's algorithm, then select the
    // earliest such closing edge across the remaining cycles. This preserves Graphlib's
    // observable error order without adding a logarithmic union-find term to otherwise linear ELK
    // hierarchy preparation.
    let node_work = checked_adapter_mul(work_control, node_capacity, 12)?;
    let assignment_work = checked_adapter_mul(work_control, assignments.len(), 4)?;
    charge_adapter_work(
        work_control,
        checked_adapter_add(work_control, node_work, assignment_work)?,
    )?;

    fn intern<'a>(
        id: &'a str,
        index_by_id: &mut HashMap<&'a str, usize>,
        next_by_index: &mut Vec<Option<usize>>,
        assignment_by_index: &mut Vec<Option<usize>>,
        indegree: &mut Vec<usize>,
    ) -> usize {
        if let Some(&index) = index_by_id.get(id) {
            return index;
        }
        let index = next_by_index.len();
        next_by_index.push(None);
        assignment_by_index.push(None);
        indegree.push(0);
        index_by_id.insert(id, index);
        index
    }

    let mut index_by_id = HashMap::with_capacity(node_capacity);
    let mut next_by_index = Vec::with_capacity(node_capacity);
    let mut assignment_by_index = Vec::with_capacity(node_capacity);
    let mut indegree = Vec::with_capacity(node_capacity);
    for (assignment_index, &(child, parent)) in assignments.iter().enumerate() {
        let child_index = intern(
            child,
            &mut index_by_id,
            &mut next_by_index,
            &mut assignment_by_index,
            &mut indegree,
        );
        let parent_index = intern(
            parent,
            &mut index_by_id,
            &mut next_by_index,
            &mut assignment_by_index,
            &mut indegree,
        );
        next_by_index[child_index] = Some(parent_index);
        assignment_by_index[child_index] = Some(assignment_index);
        indegree[parent_index] += 1;
    }

    let mut queue = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, &degree)| (degree == 0).then_some(index))
        .collect::<VecDeque<_>>();
    while let Some(index) = queue.pop_front() {
        let Some(parent_index) = next_by_index[index] else {
            continue;
        };
        indegree[parent_index] -= 1;
        if indegree[parent_index] == 0 {
            queue.push_back(parent_index);
        }
    }

    let mut visited = vec![false; next_by_index.len()];
    let mut first_closing_assignment = None;
    for start in 0..next_by_index.len() {
        if indegree[start] == 0 || visited[start] {
            continue;
        }
        let mut current = start;
        let mut closing_assignment = 0;
        loop {
            if visited[current] {
                break;
            }
            visited[current] = true;
            let assignment_index = assignment_by_index[current]
                .expect("a node retained by functional-graph peeling must own a parent assignment");
            closing_assignment = closing_assignment.max(assignment_index);
            current = next_by_index[current]
                .expect("a node retained by functional-graph peeling must have a parent");
        }
        if first_closing_assignment.is_none_or(|first| closing_assignment < first) {
            first_closing_assignment = Some(closing_assignment);
        }
    }

    Ok(first_closing_assignment.map(|assignment_index| {
        let (child, parent) = assignments[assignment_index];
        (child.to_string(), parent.to_string())
    }))
}

fn include_children_groups<'a>(
    model: &'a FlowchartModel,
    parent_by_id: &'a HashMap<String, String>,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<HashSet<&'a str>> {
    if model.subgraphs.is_empty() || model.edges.is_empty() {
        return Ok(HashSet::new());
    }
    let item_count = checked_adapter_add(work_control, model.nodes.len(), model.subgraphs.len())?;
    crate::elk_hierarchy::include_children(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .chain(model.subgraphs.iter().map(|subgraph| subgraph.id.as_str()))
            .map(|id| (id, parent_by_id.get(id).map(String::as_str))),
        item_count,
        model
            .edges
            .iter()
            .map(|edge| (edge.from.as_str(), edge.to.as_str())),
        work_control,
    )
}

fn subgraph_label(
    declaration_ordinal: usize,
    sg: &FlowSubgraph,
    ctx: &ElkMeasureContext<'_>,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<Option<elk::Label>> {
    let label_type = sg.label_type.as_deref().unwrap_or("text");
    let title = ctx.model.subgraph_title_for_render(declaration_ordinal, sg);
    let owner = FlowchartSvgLabelOwner::SubgraphTitle(declaration_ordinal);
    let (classes, styles) = ctx.model.effective_subgraph_css(declaration_ordinal, sg);
    let collapsed = ctx.model.is_subgraph_collapsed(sg.id.as_str());
    let structural = if collapsed {
        Some(
            crate::flowchart::FlowchartStructuralLabelTypography::prepare(
                ctx.class_defs,
                ctx.class_html_labels,
                crate::flowchart::FlowchartLabelDom::node(
                    "collapsedGroup",
                    title,
                    label_type,
                    ctx.cluster_wrap_mode == WrapMode::HtmlLike,
                    crate::config::config_diagram_look(ctx.effective_config).is_hand_drawn(),
                ),
                |units| charge_adapter_work(work_control, units),
            )?,
        )
    } else {
        None
    };
    let base_style = if let Some(structural) = &structural {
        structural.apply_to_parent(std::borrow::Cow::Borrowed(ctx.cluster_label_base_style))
    } else {
        std::borrow::Cow::Borrowed(ctx.cluster_label_base_style)
    };
    let mut text_style = flowchart_effective_text_style_for_classes_with_provenance(
        base_style.as_ref(),
        &ctx.model.class_defs,
        classes,
        styles,
    );
    // Collapsed groups become node terminals. Prepare their actual source font before the
    // occurrence sidecar seals the shared measurement and writer artifact.
    if let Some(structural) = &structural {
        structural.apply_to_content(&mut text_style.style);
    }
    // createGraph's unwrapGroupLabels measures ordinary groups at the same unbounded width
    // used by the cluster painter. Markdown and collapsed leaf labels retain their wrapping.
    let max_width_px =
        (label_type == "markdown" || collapsed).then_some(ctx.cluster_title_wrapping_width);
    // The layout and writer share this occurrence-scoped artifact. Re-entering a width-sensitive
    // backend for emission would allow the terminal XHTML to diverge from the measured geometry.
    let metrics = measure_flowchart_svg_label_for_layout_with_source_style(
        ctx.svg_label_sidecar,
        Some(owner),
        Some(sg.id.as_str()),
        FlowchartLabelMetricsRequest {
            measurer: ctx.measurer,
            raw_label: title,
            label_type,
            style: text_style.as_ref(),
            max_width_px,
            wrap_mode: ctx.cluster_wrap_mode,
            config: ctx.effective_config,
            math_renderer: ctx.math_renderer,
        },
        Some(&text_style),
        FlowchartSvgWidthMode::Bbox,
    );
    let height = if ctx
        .svg_label_sidecar
        .is_some_and(|sidecar| sidecar.has_prepared_math(owner, title))
    {
        metrics.height.max(1.0)
    } else {
        (metrics.height - 2.0).max(1.0)
    };
    Ok(Some(elk::Label {
        width: metrics.width.max(1.0),
        height,
    }))
}

fn subgraph_to_elk_node(
    declaration_ordinal: usize,
    sg: &FlowSubgraph,
    parent: Option<String>,
    include_children_groups: &HashSet<&str>,
    ctx: &ElkMeasureContext<'_>,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<elk::Node> {
    let label = subgraph_label(declaration_ordinal, sg, ctx, work_control)?;
    if ctx.model.is_subgraph_collapsed(sg.id.as_str()) {
        let (width, height) = label
            .map(|label| ((label.width + 16.0).max(80.0), label.height + 46.0))
            .unwrap_or((80.0, 44.0));
        return Ok(elk::Node {
            id: sg.id.clone(),
            kind: elk::NodeKind::Leaf,
            container: Default::default(),
            label_text: None,
            width,
            height,
            parent,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_alignment: None,
            label,
        });
    }

    Ok(elk::Node {
        id: sg.id.clone(),
        kind: elk::NodeKind::Group,
        container: elk::ContainerNodeOptions {
            algorithm: sg
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("algorithm"))
                .and_then(serde_json::Value::as_str)
                .and_then(elk::Algorithm::from_container_name),
            padding: ctx.title_padding,
        },
        label_text: Some(
            ctx.model
                .subgraph_title_for_render(declaration_ordinal, sg)
                .to_owned(),
        ),
        width: 0.0,
        height: 0.0,
        parent,
        // Use the resolved dir, not has_explicit_dir: FlowDB may populate it through inheritDir,
        // and Mermaid applies SeparateChildren whenever that resolved direction exists.
        direction: sg.dir.as_deref().map(dir_to_elk_direction),
        hierarchy_handling: if include_children_groups.contains(sg.id.as_str()) {
            Some(elk::HierarchyHandling::IncludeChildren)
        } else if sg.dir.is_some() {
            Some(elk::HierarchyHandling::SeparateChildren)
        } else {
            None
        },
        layer_constraint: None,
        port_alignment: None,
        label,
    })
}
