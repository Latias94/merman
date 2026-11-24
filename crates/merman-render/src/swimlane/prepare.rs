use super::config::{DEFAULT_LANE_ID, DEFAULT_LANE_PADDING, GROUP_PADDING};
use super::lanes::LaneModel;
use super::working::{ProducerLaneRole, WorkingEdge, WorkingLayout, WorkingNode, WorkingNodeKind};
use crate::flowchart::{
    FlowchartConfigView, FlowchartEdgeKey, FlowchartEdgeTransportPlan,
    FlowchartLabelMetricsRequest, FlowchartRenderModelRef, FlowchartSvgLabelOwner,
    FlowchartSvgLabelSidecarBuilder, FlowchartSvgWidthMode, NodeLayoutDimensionsRequest,
    ProjectedFlowchartEdge, flowchart_effective_text_style_for_classes_with_provenance,
    flowchart_node_label_min_width, flowchart_node_layout_style, flowchart_node_svg_width_mode,
    measure_flowchart_svg_label_for_layout_with_source_style,
};
use crate::math::MathRenderer;
use crate::model::SwimlaneDirection;
use crate::resources::OperationWorkMeter;
use crate::text::{TextMeasurer, WrapMode};
use indexmap::IndexMap;
use merman_model::families::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel, FlowchartRenderContext,
};
use merman_model::presentation::PresentationConfig;
use std::collections::{HashMap, HashSet};

pub(super) fn measure_zero_width_lane_titles(
    layout: &mut WorkingLayout,
    model: &FlowchartModel,
    render_context: &FlowchartRenderContext,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
) {
    let config_view = FlowchartConfigView::new(config.as_value());
    if config_view.swimlane_title_html_labels() {
        return;
    }
    let settings = svg_label_sidecar.map_or_else(
        || config_view.layout_settings(),
        |sidecar| sidecar.layout_settings(config.as_value()),
    );
    let render_model = FlowchartRenderModelRef::new(model, render_context);
    for (index, subgraph) in model.subgraphs.iter().enumerate() {
        let Some(lane) = layout.nodes.get_mut(&subgraph.id) else {
            continue;
        };
        if !lane.is_group() || lane.shape != "swimlane" || lane.width != 0.0 {
            continue;
        }
        let (classes, styles) = render_model.effective_subgraph_css(index, subgraph);
        let style = flowchart_effective_text_style_for_classes_with_provenance(
            &settings.text_style,
            &model.class_defs,
            classes,
            styles,
        );
        let rows = crate::flowchart::zero_width_lane_title_rows(
            render_model.subgraph_title_for_render(index, subgraph),
        );
        let metrics = crate::text::measure_svg_markdown_word_lines(
            measurer,
            &rows,
            style.as_ref(),
            crate::text::SvgTextEntityMode::DecodedModel,
        );
        lane.label_width = metrics.width;
        lane.label_height = metrics.height;
    }
}

pub(super) fn normalize_direction(direction: Option<&str>) -> SwimlaneDirection {
    match direction
        .unwrap_or("TB")
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "LR" => SwimlaneDirection::Lr,
        "BT" => SwimlaneDirection::Bt,
        "RL" => SwimlaneDirection::Rl,
        "TB" | "TD" => SwimlaneDirection::Tb,
        _ => SwimlaneDirection::Tb,
    }
}

struct MeasureContext<'a> {
    model: &'a FlowchartRenderModelRef<'a>,
    config: &'a PresentationConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    direction: SwimlaneDirection,
    settings: &'a crate::flowchart::FlowchartLayoutSettings,
    title_html_labels: bool,
    svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &'a crate::svg::FlowchartEdgeStylePlan,
    edge_label_padding: crate::flowchart::FlowchartEdgeLabelPadding,
    work_meter: &'a OperationWorkMeter,
}

fn measure_content_node(
    node: &FlowNode,
    render_label: &str,
    owner: Option<FlowchartSvgLabelOwner>,
    use_default_class: bool,
    ctx: &MeasureContext<'_>,
) -> crate::Result<WorkingNode> {
    let wrap_mode = ctx.settings.node_wrap_mode;
    let wrapping_width = crate::flowchart::bpmn::wrapping_width(
        node.layout_shape.as_deref(),
        ctx.settings.wrapping_width,
    );
    let base_style = if wrap_mode == WrapMode::HtmlLike {
        &ctx.settings.html_label_text_style
    } else {
        &ctx.settings.text_style
    };
    let (style, rectangle_size) = if use_default_class {
        flowchart_node_layout_style(
            base_style,
            &ctx.model.class_defs,
            &node.classes,
            &node.styles,
        )
    } else {
        (
            flowchart_effective_text_style_for_classes_with_provenance(
                base_style,
                &ctx.model.class_defs,
                &node.classes,
                &node.styles,
            ),
            [None; 2],
        )
    };
    let label = render_label;
    let semantic_label = node.label.as_deref().unwrap_or(&node.id);
    let label_type = node.label_type.as_deref().unwrap_or("text");
    let svg_width_mode = flowchart_node_svg_width_mode(
        label,
        label_type,
        wrap_mode,
        node.layout_shape.as_deref().unwrap_or("squareRect"),
    );
    let metrics = measure_flowchart_svg_label_for_layout_with_source_style(
        ctx.svg_label_sidecar,
        owner,
        owner.map(|_| node.id.as_str()),
        FlowchartLabelMetricsRequest {
            measurer: ctx.measurer,
            raw_label: label,
            label_type,
            style: style.as_ref(),
            max_width_px: Some(wrapping_width),
            wrap_mode,
            config: ctx.config,
            math_renderer: ctx.math_renderer,
        },
        Some(&style),
        svg_width_mode,
    )
    .with_label_min_width(
        label,
        flowchart_node_label_min_width(label, node.layout_shape.as_deref(), ctx.config),
        None,
    );

    let look = crate::config::mermaid_config_diagram_look(ctx.config);
    let ((width, height), _) = NodeLayoutDimensionsRequest {
        layout_shape: node.layout_shape.as_deref(),
        layout_direction: ctx.direction.as_str(),
        metrics,
        has_label: !label.is_empty(),
        wrapping_width,
        node_constraint: node.constraint.as_deref(),
        padding: ctx.settings.node_padding,
        look_is_neo: look.is_neo(),
        state_padding: ctx.settings.state_padding,
        node_icon: node.icon.as_deref(),
        node_img: node.img.as_deref(),
        node_pos: node.pos.as_deref(),
        node_asset_width: node.asset_width,
        node_asset_height: node.asset_height,
    }
    .measure(
        if owner.is_some() {
            rectangle_size
        } else {
            [None; 2]
        },
        look.is_hand_drawn(),
        ctx.svg_label_sidecar.zip(owner),
        |units| ctx.work_meter.charge(units).map_err(Into::into),
    )?;

    Ok(WorkingNode {
        id: node.id.clone(),
        // Geometry follows the parser-owned render spelling, while the public layout projection
        // remains a projection of the semantic model.
        label: semantic_label.to_string(),
        label_type: label_type.to_string(),
        shape: node
            .layout_shape
            .clone()
            .unwrap_or_else(|| "squareRect".to_string()),
        kind: WorkingNodeKind::Content,
        producer_lane_role: None,
        parent_id: None,
        top_lane_id: None,
        requested_dir: None,
        padding: ctx.settings.node_padding,
        x: 0.0,
        y: 0.0,
        width,
        height,
        label_width: metrics.width.max(0.0),
        label_height: metrics.height.max(0.0),
        layer: 0,
        order: 0,
        content_top: None,
        title_rect: None,
    })
}

fn measure_edge_label(
    edge: &FlowEdge,
    key: crate::flowchart::FlowchartEdgeKey,
    render_label: &str,
    label_node_id: String,
    parent_id: Option<String>,
    owner: FlowchartSvgLabelOwner,
    ctx: &MeasureContext<'_>,
) -> crate::Result<WorkingNode> {
    // Mermaid turns Swimlane edge labels into fresh `labelRect` nodes. The conversion copies the
    // label and label style, but deliberately does not copy `edge.labelType`; `labelHelper`
    // therefore treats the new node as ordinary non-Markdown text. Its initial width is zero, so
    // createText also uses the configured Flowchart wrapping width rather than the ordinary
    // Flowchart edge-label constant.
    let wrap_mode = ctx.settings.node_wrap_mode;
    let base_style = if wrap_mode == WrapMode::HtmlLike {
        &ctx.settings.html_label_text_style
    } else {
        &ctx.settings.text_style
    };
    let style = ctx
        .edge_style_plan
        .swimlane_edge_label_text_style_for(key, base_style)?;
    let label = render_label;
    let semantic_label = edge.label.as_deref().unwrap_or_default();
    let label_type = "text";
    let metrics = measure_flowchart_svg_label_for_layout_with_source_style(
        ctx.svg_label_sidecar,
        Some(owner),
        Some(edge.id.as_str()),
        FlowchartLabelMetricsRequest {
            measurer: ctx.measurer,
            raw_label: label,
            label_type,
            style: style.as_ref(),
            max_width_px: Some(ctx.settings.wrapping_width),
            wrap_mode,
            config: ctx.config,
            math_renderer: ctx.math_renderer,
        },
        Some(&style),
        FlowchartSvgWidthMode::Bbox,
    );
    let (label_width, label_height) = ctx
        .edge_label_padding
        .padded_size(metrics.width.max(0.0), metrics.height.max(0.0));

    Ok(WorkingNode {
        id: label_node_id,
        label: semantic_label.to_string(),
        label_type: label_type.to_string(),
        shape: "labelRect".to_string(),
        kind: WorkingNodeKind::EdgeLabel,
        producer_lane_role: None,
        parent_id,
        top_lane_id: None,
        requested_dir: None,
        padding: 0.0,
        x: 0.0,
        y: 0.0,
        // createGraphWithElements overwrites labelRect's hidden 0.1 x 0.1 SVG rect with the
        // complete label group's measured bbox before the Swimlane layout core runs.
        width: label_width,
        height: label_height,
        label_width,
        label_height,
        layer: 0,
        order: 0,
        content_top: None,
        title_rect: None,
    })
}

fn working_edge(edge: &ProjectedFlowchartEdge<'_>, transport_id: &str) -> WorkingEdge {
    WorkingEdge {
        id: transport_id.to_string(),
        from: edge.from.to_owned(),
        to: edge.to.to_owned(),
        reference_id: transport_id.to_string(),
        label_node_id: None,
        reversed_for_layout: false,
        points: Vec::new(),
    }
}

pub(super) fn synthetic_node(
    id: &str,
    label: &str,
    label_type: Option<&str>,
    shape: &str,
) -> FlowNode {
    FlowNode {
        id: id.to_owned(),
        provenance: Default::default(),
        label: Some(label.to_owned()),
        label_type: label_type.map(str::to_owned),
        layout_shape: Some(shape.to_owned()),
        shape: None,
        icon: None,
        form: None,
        pos: None,
        img: None,
        constraint: None,
        asset_width: None,
        asset_height: None,
        classes: Vec::new(),
        styles: Vec::new(),
        link: None,
        link_target: None,
        have_callback: false,
    }
}

fn measure_group_title(
    declaration_ordinal: usize,
    subgraph: &FlowSubgraph,
    render_title: &str,
    ctx: &MeasureContext<'_>,
) -> (f64, f64) {
    // The dedicated Mermaid Swimlane cluster renderer reads `flowchart.htmlLabels` directly.
    // This deliberately differs from ordinary Flowchart clusters, which use the effective
    // root-first compatibility setting.
    let wrap_mode = if ctx.title_html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    let base_style = if wrap_mode == WrapMode::HtmlLike {
        &ctx.settings.html_label_text_style
    } else {
        &ctx.settings.text_style
    };
    let (classes, styles) = ctx
        .model
        .effective_subgraph_css(declaration_ordinal, subgraph);
    let style = flowchart_effective_text_style_for_classes_with_provenance(
        base_style,
        &ctx.model.class_defs,
        classes,
        styles,
    );
    // Mermaid 11.16's dedicated Swimlane renderer omits createText's `markdown` option, so the
    // default `true` applies independently of FlowDB's public subgraph labelType.
    let render_label_type = "markdown";
    let metrics = measure_flowchart_svg_label_for_layout_with_source_style(
        ctx.svg_label_sidecar,
        Some(FlowchartSvgLabelOwner::SwimlaneGroupTitle(
            declaration_ordinal,
        )),
        Some(subgraph.id.as_str()),
        FlowchartLabelMetricsRequest {
            measurer: ctx.measurer,
            raw_label: render_title,
            label_type: render_label_type,
            style: style.as_ref(),
            max_width_px: Some(ctx.settings.cluster_title_wrapping_width),
            wrap_mode,
            config: ctx.config,
            math_renderer: ctx.math_renderer,
        },
        Some(&style),
        FlowchartSvgWidthMode::Bbox,
    );
    (metrics.width.max(0.0), metrics.height.max(0.0))
}

pub(super) fn prepare(
    model: &FlowchartRenderModelRef<'_>,
    (visible_edges, transport_plan): (
        &[(FlowchartEdgeKey, ProjectedFlowchartEdge<'_>)],
        &FlowchartEdgeTransportPlan<'_>,
    ),
    effective_config: &PresentationConfig,
    #[cfg(feature = "layout-shoal")] canonical: bool,
    (measurer, math_renderer): (&dyn TextMeasurer, Option<&(dyn MathRenderer + Send + Sync)>),
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    work_meter: &OperationWorkMeter,
) -> crate::Result<WorkingLayout> {
    let direction = normalize_direction(model.direction.as_deref());
    let config_view = FlowchartConfigView::new(effective_config.as_value());
    let swimlane_title_html_labels = config_view.swimlane_title_html_labels();
    let settings = svg_label_sidecar.map_or_else(
        || config_view.layout_settings(),
        |sidecar| sidecar.layout_settings(effective_config.as_value()),
    );
    let edge_label_padding = svg_label_sidecar.map_or_else(
        crate::flowchart::FlowchartEdgeLabelPadding::default,
        FlowchartSvgLabelSidecarBuilder::edge_label_padding,
    );
    let measure_ctx = MeasureContext {
        model,
        config: effective_config,
        measurer,
        math_renderer,
        direction,
        settings: &settings,
        title_html_labels: swimlane_title_html_labels,
        svg_label_sidecar,
        edge_style_plan,
        edge_label_padding,
        work_meter,
    };

    // FlowDB builds parentDB by walking subgraphs from last to first.
    let parent_by_id: HashMap<&str, &str> = model.visible_memberships().collect();
    let parent_id = |id: &str| parent_by_id.get(id).map(|parent| (*parent).to_owned());
    let group_indices: HashMap<&str, usize> = model
        .subgraphs
        .iter()
        .enumerate()
        .rev()
        .map(|(index, subgraph)| (subgraph.id.as_str(), index))
        .collect();

    let mut nodes = IndexMap::new();
    for subgraph_index in (0..model.subgraphs.len()).rev() {
        let subgraph = &model.subgraphs[subgraph_index];
        if model.collapsed_replacement(&subgraph.id).is_some() {
            continue;
        }
        let render_title = model.subgraph_title_for_render(subgraph_index, subgraph);
        if model.is_subgraph_collapsed(&subgraph.id) {
            if group_indices.get(subgraph.id.as_str()) != Some(&subgraph_index) {
                continue;
            }
            let mut node = synthetic_node(
                &subgraph.id,
                &subgraph.title,
                subgraph.label_type.as_deref(),
                "collapsedGroup",
            );
            let (classes, styles) = model.effective_subgraph_css(subgraph_index, subgraph);
            node.classes.extend_from_slice(classes);
            node.styles.extend_from_slice(styles);
            // The node owner range for collapsed groups follows the authored node range.
            let owner = FlowchartSvgLabelOwner::SwimlaneNode(model.nodes.len() + subgraph_index);
            let mut measured =
                measure_content_node(&node, render_title, Some(owner), false, &measure_ctx)?;
            measured.parent_id = parent_id(&subgraph.id);
            nodes.insert(node.id, measured);
            continue;
        }
        let (label_width, label_height) =
            measure_group_title(subgraph_index, subgraph, render_title, &measure_ctx);
        let producer_lane_role = match subgraph
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("laneRole"))
            .and_then(serde_json::Value::as_str)
        {
            Some("pool") => Some(ProducerLaneRole::Pool),
            Some("lane") => Some(ProducerLaneRole::Lane),
            _ => None,
        };
        nodes.insert(
            subgraph.id.clone(),
            WorkingNode {
                id: subgraph.id.clone(),
                label: subgraph.title.clone(),
                label_type: subgraph
                    .label_type
                    .clone()
                    .unwrap_or_else(|| "text".to_string()),
                shape: "rect".to_string(),
                producer_lane_role,
                kind: WorkingNodeKind::Group,
                parent_id: parent_id(&subgraph.id),
                top_lane_id: None,
                requested_dir: subgraph.dir.as_ref().map(|dir| {
                    if dir.eq_ignore_ascii_case("TD") {
                        "TB".to_string()
                    } else {
                        dir.trim().to_ascii_uppercase()
                    }
                }),
                padding: GROUP_PADDING,
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
                label_width,
                label_height,
                layer: 0,
                order: 0,
                content_top: None,
                title_rect: None,
            },
        );
    }

    for (node_index, node) in model.nodes.iter().enumerate() {
        if group_indices.contains_key(node.id.as_str())
            || model.collapsed_replacement(&node.id).is_some()
        {
            continue;
        }
        crate::flowchart::validate_flowchart_node_asset_dimensions(node)?;
        let render_label = model.node_label_for_render(node).unwrap_or(&node.id);
        let mut measured = measure_content_node(
            node,
            render_label,
            Some(FlowchartSvgLabelOwner::SwimlaneNode(node_index)),
            true,
            &measure_ctx,
        )?;
        measured.parent_id = parent_id(&node.id);
        nodes.insert(node.id.clone(), measured);
    }
    for id in &model.vertex_calls {
        if nodes.contains_key(id) || model.collapsed_replacement(id).is_some() {
            continue;
        }
        let synthetic = synthetic_node(id, id, Some("text"), "squareRect");
        let mut measured = measure_content_node(&synthetic, id, None, true, &measure_ctx)?;
        measured.parent_id = parent_id(id);
        nodes.insert(id.clone(), measured);
    }

    let lane_model = LaneModel::new(&nodes);
    let mut emptied = HashSet::new();
    for member in &model.dropped_subgraph_members {
        let kept_by = if lane_model.is_lane(&member.kept_by) {
            Some(member.kept_by.as_str())
        } else {
            lane_model.lane_of(&member.kept_by)
        };
        if kept_by.is_some_and(|lane| lane != member.dropped_from)
            && lane_model.is_lane(&member.dropped_from)
        {
            emptied.insert(member.dropped_from.as_str());
        }
    }
    let used_groups: HashSet<&str> = nodes
        .values()
        .filter_map(|node| node.parent_id.as_deref())
        .chain(
            visible_edges
                .iter()
                .flat_map(|(_, edge)| [edge.from, edge.to]),
        )
        .collect();
    let removed: HashSet<String> = emptied
        .into_iter()
        .filter(|id| !used_groups.contains(id))
        .map(str::to_string)
        .collect();
    nodes.retain(|id, _| !removed.contains(id));

    let lane_model = LaneModel::new(&nodes);
    for node in nodes.values_mut() {
        if lane_model.is_lane(&node.id) || lane_model.is_pool(&node.id) {
            node.shape = if lane_model.is_lane(&node.id) {
                "swimlane"
            } else {
                "pool"
            }
            .to_string();
            node.requested_dir = Some(direction.as_str().to_string());
        }
    }

    let loose_ids: Vec<String> = nodes
        .values()
        .filter(|node| !node.is_group() && node.parent_id.is_none())
        .map(|node| node.id.clone())
        .collect();
    if !loose_ids.is_empty() {
        let lane_id: std::borrow::Cow<'_, str> = std::borrow::Cow::Borrowed(DEFAULT_LANE_ID);
        #[cfg(feature = "layout-shoal")]
        let lane_id = if canonical
            && nodes
                .get(DEFAULT_LANE_ID)
                .is_some_and(|node| !node.is_group())
        {
            let mut suffix = 1usize;
            loop {
                work_meter.charge(1)?;
                let candidate = format!("{DEFAULT_LANE_ID}-{suffix}");
                if !nodes.contains_key(&candidate) {
                    break std::borrow::Cow::Owned(candidate);
                }
                suffix += 1;
            }
        } else {
            lane_id
        };
        let lane_id = lane_id.as_ref();
        if let Some(default_lane) = nodes.get_mut(lane_id) {
            // Mermaid reuses an explicit group with the reserved id. Preserve
            // its title, classes, styles, and measured geometry; only apply
            // the swimlane shape/direction required by the layout adapter.
            if default_lane.is_group() {
                default_lane.shape = "swimlane".to_string();
                default_lane.requested_dir = Some(direction.as_str().to_string());
            }
        } else {
            nodes.insert(
                lane_id.to_owned(),
                WorkingNode {
                    id: lane_id.to_owned(),
                    label: String::new(),
                    label_type: "text".to_string(),
                    shape: "swimlane".to_string(),
                    kind: WorkingNodeKind::Group,
                    producer_lane_role: None,
                    parent_id: None,
                    top_lane_id: None,
                    requested_dir: Some(direction.as_str().to_string()),
                    padding: DEFAULT_LANE_PADDING,
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                    label_width: 0.0,
                    label_height: 0.0,
                    layer: 0,
                    order: 0,
                    content_top: None,
                    title_rect: None,
                },
            );
        }
        for id in loose_ids {
            if let Some(node) = nodes.get_mut(&id) {
                node.parent_id = Some(lane_id.to_owned());
            }
        }
    }

    let mut original_edges = Vec::with_capacity(visible_edges.len());
    for &(key, ref edge) in visible_edges {
        let edge_index = key.semantic_index();
        let transport_id = transport_plan
            .id(key)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!(
                    "missing Swimlane transport id for semantic edge owner {edge_index}"
                ),
            })?;
        let mut original = working_edge(edge, transport_id);
        let render_label = model.edge_label_for_render(edge_index, edge.semantic);
        let has_label = render_label.is_some_and(|label| !label.is_empty());
        if has_label && nodes.contains_key(edge.from) && nodes.contains_key(edge.to) {
            let label_node_id = if transport_id != edge.semantic.id {
                format!("edge-label-occurrence-{edge_index:020}")
            } else {
                format!("edge-label-{}-{}-{}", edge.from, edge.to, edge.semantic.id)
            };
            let mut unique_label_node_id = label_node_id.clone();
            let mut suffix = 1usize;
            while nodes.contains_key(&unique_label_node_id) {
                unique_label_node_id = format!("{label_node_id}-{suffix}");
                suffix += 1;
            }
            let label_node_id = unique_label_node_id;
            let source_parent = nodes.get(edge.from).and_then(|node| node.parent_id.clone());
            let target_parent = nodes.get(edge.to).and_then(|node| node.parent_id.clone());
            let label_parent = if source_parent != target_parent {
                target_parent
            } else {
                source_parent
            };
            let label_node = measure_edge_label(
                edge.semantic,
                key,
                render_label.unwrap_or_default(),
                label_node_id.clone(),
                label_parent,
                FlowchartSvgLabelOwner::SwimlaneEdgeLabel(edge_index),
                &measure_ctx,
            )?;
            nodes.insert(label_node_id.clone(), label_node);
            original.label_node_id = Some(label_node_id.clone());
        }
        original_edges.push(original);
    }

    let lane_model = LaneModel::new(&nodes);
    let lane_indices: HashMap<&str, f64> = model
        .subgraphs
        .iter()
        .filter_map(|subgraph| {
            let index = subgraph.metadata.as_ref()?.get("laneIndex")?.as_f64()?;
            index.is_finite().then_some((subgraph.id.as_str(), index))
        })
        .collect();
    let top_lane_order = lane_model.order(&nodes, &lane_indices);

    let mut layout = WorkingLayout {
        direction,
        nodes,
        graph_edges: original_edges.clone(),
        original_edges,
        top_lane_order,
    };
    layout.refresh_top_lane_ids();
    Ok(layout)
}
