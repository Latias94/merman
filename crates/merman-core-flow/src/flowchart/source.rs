//! Replays a parsed Flowchart or Swimlane and projects its canonical model.

use super::{
    ClickAction, EdgeAction, FlowchartAst, LinkStylePos, LinkToken, NodeAction, ShapeData,
    StatementEvent, Stmt, SubgraphBlock, TitleKind, is_ecmascript_trim_char, link,
    trim_flowdb_label_text,
};
use crate::grid::{InlineGridCoordinates, InlineGridReplay};
use indexmap::IndexMap;
use merman_markup::sanitize::{sanitize_text, sanitize_text_as_html_fragment};
#[cfg(feature = "presentation")]
use merman_model::families::flowchart::FlowchartRenderContext;
use merman_model::families::flowchart::{
    FlowDroppedSubgraphMember, FlowEdge, FlowEdgeDefaults, FlowNode, FlowNodeProvenance,
    FlowSubgraph, FlowSubgraphVertexStyle, FlowchartLabelSourceOwner, FlowchartModel,
    FlowchartRenderLabelSources, FlowchartSubgraphStyles, flowchart_svg_text_source,
};
use merman_model::warnings::{DiagramWarningFact, FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID};
use merman_model::{OperationControl, OperationControlResult, SourceSpan, config::MermaidConfig};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

mod build;
mod model;
mod semantic;
mod shape_data;
mod subgraph;

use build::FlowchartBuildState;
use model::{Edge, EdgeDefaults, Node};
use semantic::{FlowchartSemanticContext, apply_semantic_statements};
use shape_data::{
    ShapeDataDocument, apply_shape_data_value_to_node, prepare_flowchart_shape_data, value_to_bool,
    value_to_string,
};
use subgraph::SubgraphBuilder;

type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone)]
struct FlowSubGraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub dir: Option<String>,
    pub has_explicit_dir: bool,
    pub label_type: String,
    pub metadata: Option<Value>,
}

struct FlowchartSemanticSource {
    keyword: String,
    direction: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    class_defs: IndexMap<String, Vec<String>>,
    tooltips: HashMap<String, String>,
    edge_defaults: EdgeDefaults,
    vertex_calls: Vec<String>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    subgraphs: Vec<FlowSubGraph>,
    dropped_subgraph_members: Vec<FlowDroppedSubgraphMember>,
    subgraph_vertex_styles: FlowchartSubgraphStyles,
    #[cfg(feature = "presentation")]
    collapsed_subgraphs: Option<rustc_hash::FxHashSet<String>>,
    grid_coordinates: InlineGridCoordinates,
    warning_facts: Vec<DiagramWarningFact>,
}

impl FlowchartAst {
    /// Consumes the parsed statements and constructs the canonical model.
    ///
    /// `acc_title` and `acc_descr` come from the same lexer that produced this AST.
    /// A model error carries the message for the caller's fallback parse diagnostic.
    #[doc(hidden)]
    pub fn into_model(
        self,
        diagram_type: &str,
        config: &MermaidConfig,
        acc_title: Option<String>,
        acc_descr: Option<String>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<FlowchartModel>> {
        Ok(self
            .into_model_with_grid_coordinates(diagram_type, config, acc_title, acc_descr, control)?
            .map(|(model, _)| model))
    }

    /// Retains the final inline Grid coordinates for post-parse admission in Core.
    #[doc(hidden)]
    pub fn into_model_with_grid_coordinates(
        self,
        diagram_type: &str,
        config: &MermaidConfig,
        acc_title: Option<String>,
        acc_descr: Option<String>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(FlowchartModel, InlineGridCoordinates)>> {
        Ok(self
            .build_model(diagram_type, config, acc_title, acc_descr, false, control)?
            .map(|projection| (projection.model, projection.grid_coordinates)))
    }

    /// Consumes the same parsed statements and collects render-only context during replay.
    #[cfg(feature = "presentation")]
    #[doc(hidden)]
    pub fn into_model_with_render_context(
        self,
        diagram_type: &str,
        config: &MermaidConfig,
        acc_title: Option<String>,
        acc_descr: Option<String>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(FlowchartModel, FlowchartRenderContext)>> {
        Ok(self
            .into_model_with_render_context_and_grid_coordinates(
                diagram_type,
                config,
                acc_title,
                acc_descr,
                control,
            )?
            .map(|(model, render_context, _)| (model, render_context)))
    }

    /// Collects render context and inline Grid coordinates during the same replay.
    #[cfg(feature = "presentation")]
    #[doc(hidden)]
    pub fn into_model_with_render_context_and_grid_coordinates(
        self,
        diagram_type: &str,
        config: &MermaidConfig,
        acc_title: Option<String>,
        acc_descr: Option<String>,
        control: &OperationControl,
    ) -> OperationControlResult<
        Result<(
            FlowchartModel,
            FlowchartRenderContext,
            InlineGridCoordinates,
        )>,
    > {
        Ok(self
            .build_model(diagram_type, config, acc_title, acc_descr, true, control)?
            .map(|projection| {
                (
                    projection.model,
                    projection
                        .render_context
                        .expect("render context is collected for render-model consumers"),
                    projection.grid_coordinates,
                )
            }))
    }

    fn build_model(
        self,
        diagram_type: &str,
        config: &MermaidConfig,
        acc_title: Option<String>,
        acc_descr: Option<String>,
        collect_render_context: bool,
        control: &OperationControl,
    ) -> OperationControlResult<Result<FlowchartModelProjection>> {
        let source = match build_flowchart_source(
            self,
            acc_title,
            acc_descr,
            config,
            collect_render_context,
            control,
        )? {
            Ok(source) => source,
            Err(error) => return Ok(Err(error)),
        };
        source.into_model_parts_controlled(diagram_type, config, collect_render_context, control)
    }
}

fn build_flowchart_source(
    ast: FlowchartAst,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    config: &MermaidConfig,
    _collect_render_context: bool,
    control: &OperationControl,
) -> OperationControlResult<Result<FlowchartSemanticSource>> {
    let shape_data_documents = prepare_flowchart_shape_data(&ast, control)?;
    control.checkpoint()?;
    let inherit_dir = config
        .get_path(&["flowchart", "inheritDir"])
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut builder = SubgraphBuilder::new(inherit_dir, ast.direction.clone());
    builder.visit_statements(&ast, control)?;

    let subgraph_ids: HashSet<String> = builder
        .subgraphs
        .iter()
        .map(|subgraph| subgraph.id.clone())
        .collect();

    let mut build = FlowchartBuildState::new(subgraph_ids);
    build.add_statements(&ast, control)?;
    let FlowchartBuildState {
        mut nodes,
        mut node_index,
        mut edges,
        ..
    } = build;

    let mut class_defs: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut subgraph_vertex_styles = FlowchartSubgraphStyles::default();
    #[cfg(feature = "presentation")]
    let mut collapsed_subgraphs = _collect_render_context.then(rustc_hash::FxHashSet::default);
    let mut vertex_calls = Vec::new();
    let mut warning_facts = Vec::new();
    let mut grid_replay =
        (config.get_str("layout") == Some("grid")).then(InlineGridReplay::default);
    let mut tooltips: HashMap<String, String> = HashMap::new();
    let mut edge_defaults = EdgeDefaults {
        style: Vec::new(),
        interpolate: None,
    };

    // Preserve the checkpoint cadence of the former index-rebuild pass.
    for _ in (0..nodes.len()).step_by(128) {
        control.checkpoint()?;
    }
    let security_level_loose = config.get_str("securityLevel") == Some("loose");
    {
        let mut semantic_ctx = FlowchartSemanticContext {
            nodes: &mut nodes,
            node_index: &mut node_index,
            edges: &mut edges,
            subgraphs: &mut builder.subgraphs,
            subgraph_declaration_owners: &builder.declaration_owners,
            subgraph_vertex_styles: &mut subgraph_vertex_styles,
            #[cfg(feature = "presentation")]
            collapsed_subgraphs: collapsed_subgraphs.as_mut(),
            grid_replay: grid_replay.as_mut(),
            vertex_calls: &mut vertex_calls,
            warning_facts: &mut warning_facts,
            class_defs: &mut class_defs,
            tooltips: &mut tooltips,
            edge_defaults: &mut edge_defaults,
            security_level_loose,
            config,
            shape_data_documents: &shape_data_documents,
            control,
        };
        if let Err(error) = apply_semantic_statements(&ast, &mut semantic_ctx)? {
            return Ok(Err(error));
        }
    }

    let grid_coordinates = match grid_replay {
        Some(replay) => replay.finish(
            |slot| &nodes[slot].id,
            |slot| &builder.subgraphs[slot].id,
            control,
        )?,
        None => InlineGridCoordinates::default(),
    };
    let direction = ast.direction;
    warning_facts.extend(flowchart_warning_facts(&direction, ast.header_span));
    control.checkpoint()?;
    Ok(Ok(FlowchartSemanticSource {
        keyword: ast.keyword,
        direction,
        acc_descr,
        acc_title,
        class_defs,
        tooltips,
        edge_defaults,
        vertex_calls,
        nodes,
        edges,
        subgraphs: builder.subgraphs,
        dropped_subgraph_members: builder.dropped_subgraph_members,
        subgraph_vertex_styles,
        #[cfg(feature = "presentation")]
        collapsed_subgraphs,
        grid_coordinates,
        warning_facts,
    }))
}

fn flowchart_warning_facts(
    direction: &Option<String>,
    header_span: SourceSpan,
) -> Vec<DiagramWarningFact> {
    if direction.is_some() {
        return Vec::new();
    }

    vec![
        DiagramWarningFact::new(
            FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
            "flowchart headers should declare an explicit direction such as `TB`, `TD`, `BT`, `LR`, or `RL`",
        )
        .with_span(header_span)
        .with_fix_span(SourceSpan::new(header_span.end, header_span.end)),
    ]
}

struct FlowchartModelProjection {
    model: FlowchartModel,
    grid_coordinates: InlineGridCoordinates,
    #[cfg(feature = "presentation")]
    render_context: Option<FlowchartRenderContext>,
}

impl FlowchartSemanticSource {
    fn into_model_parts_controlled(
        self,
        diagram_type: &str,
        config: &MermaidConfig,
        _collect_render_context: bool,
        control: &OperationControl,
    ) -> OperationControlResult<Result<FlowchartModelProjection>> {
        control.checkpoint()?;
        let FlowchartSemanticSource {
            acc_descr,
            acc_title,
            class_defs,
            edge_defaults,
            vertex_calls,
            mut nodes,
            edges,
            subgraphs,
            dropped_subgraph_members,
            subgraph_vertex_styles,
            #[cfg(feature = "presentation")]
            collapsed_subgraphs,
            grid_coordinates,
            warning_facts,
            tooltips,
            keyword,
            direction,
        } = self;

        if diagram_type == "flowchart-elk" {
            append_missing_subgraph_nodes(&mut nodes, &subgraphs, control)?;
        }

        let mut render_nodes = Vec::with_capacity(nodes.len());
        let mut label_sources = FlowchartRenderLabelSources::default();
        let preserve_svg_text = matches!(
            diagram_type,
            "flowchart" | "flowchart-v2" | "flowchart-elk" | "swimlane"
        );
        for (index, node) in nodes.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let node = flow_node_to_model(node, config, preserve_svg_text, &mut label_sources);
            render_nodes.push(node);
        }
        let mut render_edges = Vec::with_capacity(edges.len());
        for (index, edge) in edges.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            match flow_edge_to_model(edge, config, index, preserve_svg_text, &mut label_sources) {
                Ok(edge) => {
                    render_edges.push(edge);
                }
                Err(error) => return Ok(Err(error)),
            }
        }
        let mut render_subgraphs = Vec::with_capacity(subgraphs.len());
        for (index, subgraph) in subgraphs.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let subgraph = flow_subgraph_to_model(
                subgraph,
                config,
                index,
                preserve_svg_text,
                &mut label_sources,
            );
            render_subgraphs.push(subgraph);
        }
        let mut render_tooltips = rustc_hash::FxHashMap::default();
        render_tooltips.reserve(tooltips.len());
        for (index, (id, tooltip)) in tooltips.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            render_tooltips.insert(id, tooltip);
        }

        control.checkpoint()?;
        let model = FlowchartModel {
            keyword,
            acc_descr,
            acc_title,
            class_defs,
            direction: direction.or_else(|| Some("TB".to_string())),
            edge_defaults: Some(FlowEdgeDefaults {
                style: edge_defaults.style,
                interpolate: edge_defaults.interpolate,
            }),
            vertex_calls,
            nodes: render_nodes,
            edges: render_edges,
            subgraphs: render_subgraphs,
            label_sources,
            dropped_subgraph_members,
            subgraph_vertex_styles,
            tooltips: render_tooltips,
            warning_facts,
        };
        #[cfg(feature = "presentation")]
        let render_context = if _collect_render_context {
            Some(FlowchartRenderContext::new(
                model.label_sources.clone(),
                model.subgraph_vertex_styles.clone(),
                collapsed_subgraphs
                    .expect("collapsed subgraphs are collected for render consumers"),
                &model.subgraphs,
            ))
        } else {
            None
        };
        Ok(Ok(FlowchartModelProjection {
            model,
            grid_coordinates,
            #[cfg(feature = "presentation")]
            render_context,
        }))
    }
}

fn append_missing_subgraph_nodes(
    nodes: &mut Vec<Node>,
    subgraphs: &[FlowSubGraph],
    control: &OperationControl,
) -> OperationControlResult<()> {
    let mut existing_ids = HashSet::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        existing_ids.insert(node.id.clone());
    }
    for (index, subgraph) in subgraphs.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if existing_ids.insert(subgraph.id.clone()) {
            nodes.push(Node {
                id: subgraph.id.clone(),
                provenance: FlowNodeProvenance::SubgraphAnchor,
                label: None,
                label_type: TitleKind::Text,
                shape: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                styles: Vec::new(),
                classes: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            });
        }
    }
    control.checkpoint()
}

fn flow_node_to_model(
    n: Node,
    config: &MermaidConfig,
    preserve_svg_text: bool,
    label_sources: &mut FlowchartRenderLabelSources,
) -> FlowNode {
    let layout_shape = layout_shape_for_node(&n);
    let label = sanitized_node_label(&n, config);
    let label_raw = n.label.as_deref().unwrap_or(&n.id);
    if let Some(source) = render_label_source_needs_provenance(label_raw)
        .then(|| sanitized_node_render_label_source(&n, config))
        .filter(|source| *source != label)
    {
        label_sources.insert_node(n.id.clone(), source);
    }
    if preserve_svg_text
        && n.label_type != TitleKind::Markdown
        && let Some(source) = flowchart_svg_text_source(
            strip_label_quotes(label_raw),
            label_sources.node_source(&n.id).unwrap_or(&label),
        )
    {
        label_sources.insert_svg_text(FlowchartLabelSourceOwner::Node(&n.id), source);
    }

    FlowNode {
        id: n.id,
        provenance: n.provenance,
        label: Some(label),
        label_type: Some(n.label_type.as_str().to_string()),
        layout_shape: Some(layout_shape),
        shape: n.shape,
        icon: n.icon,
        form: n.form,
        pos: n.pos,
        img: n.img,
        constraint: n.constraint,
        asset_width: n.asset_width,
        asset_height: n.asset_height,
        classes: n.classes,
        styles: n.styles,
        link: n.link,
        link_target: n.link_target,
        have_callback: n.have_callback,
    }
}

fn flow_edge_to_model(
    e: Edge,
    config: &MermaidConfig,
    semantic_index: usize,
    preserve_svg_text: bool,
    label_sources: &mut FlowchartRenderLabelSources,
) -> Result<FlowEdge> {
    let label = sanitized_optional_label(e.label.as_deref(), config);
    let render_label_source = e
        .label
        .as_deref()
        .filter(|raw| render_label_source_needs_provenance(raw))
        .map(|raw| sanitized_render_label_source(raw, config));
    if let (Some(label), Some(source)) = (&label, render_label_source)
        && *label != source
    {
        label_sources.insert_edge(semantic_index, source);
    }
    if preserve_svg_text
        && e.label_type != TitleKind::Markdown
        && let (Some(raw), Some(label)) = (e.label.as_deref(), label.as_deref())
        && let Some(source) = flowchart_svg_text_source(
            raw,
            label_sources.edge_source(semantic_index).unwrap_or(label),
        )
    {
        label_sources.insert_svg_text(FlowchartLabelSourceOwner::Edge(semantic_index), source);
    }
    let id =
        e.id.ok_or_else(|| "flowchart edge id missing".to_string())?;
    let edge_type = link::compatibility_edge_type(&e.link).to_string();
    let stroke = link::compatibility_stroke(&e.link).to_string();

    Ok(FlowEdge {
        id,
        from: e.from,
        to: e.to,
        label,
        label_type: Some(e.label_type.as_str().to_string()),
        edge_type: Some(edge_type),
        arrow: e.link.end,
        start_marker: e.link.start_marker,
        end_marker: e.link.end_marker,
        is_user_defined_id: e.is_user_defined_id,
        stroke: Some(stroke),
        stroke_kind: e.link.stroke_kind,
        visibility: e.link.visibility,
        length: e.link.length,
        style: e.style,
        classes: e.classes,
        interpolate: e.interpolate,
        animate: e.animate,
        animation: e.animation,
        step: e.step,
    })
}

fn layout_shape_for_node(n: &Node) -> String {
    // Mirrors Mermaid FlowDB `getTypeFromVertex` logic at the pinned Mermaid baseline.
    if n.img.is_some() {
        return "imageSquare".to_string();
    }
    if n.icon.is_some() {
        match n.form.as_deref() {
            Some("circle") => return "iconCircle".to_string(),
            Some("square") => return "iconSquare".to_string(),
            Some("rounded") => return "iconRounded".to_string(),
            _ => return "icon".to_string(),
        }
    }
    match n.shape.as_deref() {
        Some("square") | None => "squareRect".to_string(),
        Some("round") => "roundedRect".to_string(),
        Some("ellipse") => "ellipse".to_string(),
        Some(other) => other.to_string(),
    }
}

fn sanitized_node_label(n: &Node, config: &MermaidConfig) -> String {
    let label_raw = n.label.as_ref().unwrap_or(&n.id);
    let mut label = sanitized_label(label_raw, config);
    if label.len() >= 2 && label.starts_with('\"') && label.ends_with('\"') {
        label = label[1..label.len() - 1].to_string();
    }
    label
}

fn sanitized_node_render_label_source(n: &Node, config: &MermaidConfig) -> String {
    let label_raw = n.label.as_ref().unwrap_or(&n.id);
    let mut label = sanitized_render_label_source(label_raw, config);
    if label.len() >= 2 && label.starts_with('"') && label.ends_with('"') {
        label = label[1..label.len() - 1].to_string();
    }
    label
}

fn strip_label_quotes(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|body| body.strip_suffix('"'))
        .unwrap_or(text)
}

fn sanitized_optional_label(label: Option<&str>, config: &MermaidConfig) -> Option<String> {
    label.map(|s| sanitized_label(s, config))
}

fn sanitized_label(raw: &str, config: &MermaidConfig) -> String {
    let decoded = decode_mermaid_hash_entities(raw);
    sanitize_text(&decoded, config)
}

fn sanitized_render_label_source(raw: &str, config: &MermaidConfig) -> String {
    let flow_db_label = sanitize_text(raw, config);
    let decoded = merman_model::text::restore_mermaid_entity_spelling(&flow_db_label);
    sanitize_text_as_html_fragment(decoded.as_ref(), config)
}

fn render_label_source_needs_provenance(raw: &str) -> bool {
    raw.contains(['&', '#', 'ﬂ', '¶', '<', '>'])
}

fn decode_mermaid_hash_entities(input: &str) -> std::borrow::Cow<'_, str> {
    // Mermaid runs `encodeEntities(...)` before parsing and later decodes with browser
    // `entityDecode(...)`. In our headless pipeline we decode into Unicode during parsing so
    // layout + SVG output match upstream.
    merman_model::text::decode_mermaid_entities_to_unicode(input)
}

fn flow_subgraph_to_model(
    sg: FlowSubGraph,
    config: &MermaidConfig,
    declaration_ordinal: usize,
    preserve_svg_text: bool,
    label_sources: &mut FlowchartRenderLabelSources,
) -> FlowSubgraph {
    let sanitized_title = sanitize_text(&sg.title, config);
    let title = decode_mermaid_hash_entities(&sanitized_title).into_owned();
    let render_title_source = render_label_source_needs_provenance(&sg.title)
        .then(|| sanitized_render_label_source(&sg.title, config))
        .filter(|source| *source != title);
    if let Some(source) = render_title_source {
        label_sources.insert_subgraph(sg.id.clone(), declaration_ordinal, Some(source));
    }
    if preserve_svg_text
        && sg.label_type != "markdown"
        && let Some(source) = flowchart_svg_text_source(
            &sg.title,
            label_sources
                .subgraph_source(declaration_ordinal, &sg.id)
                .unwrap_or(&title),
        )
    {
        label_sources.insert_svg_text(
            FlowchartLabelSourceOwner::Subgraph(declaration_ordinal, &sg.id),
            source,
        );
    }
    FlowSubgraph {
        id: sg.id,
        nodes: sg.nodes,
        title,
        classes: sg.classes,
        styles: sg.styles,
        dir: sg.dir,
        has_explicit_dir: sg.has_explicit_dir,
        label_type: Some(sg.label_type),
        metadata: sg.metadata,
    }
}

#[cfg(test)]
mod tests;
