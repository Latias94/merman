//! Renderer-neutral Flowchart adapter.
//!
//! This module deliberately emits typed paths and text runs from the already-authoritative
//! Flowchart layout.  It does not parse the SVG output back into a second representation: doing
//! so would make the public target depend on DOM spelling and would lose the distinction between
//! source semantics and SVG-only structure.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families, theme_color,
};
use crate::config::config_diagram_look;
use crate::environment::{RenderSession, TextMeasurementPhase, TextMeasurementSource};
use crate::family::FlowchartFamilyArtifact;
use crate::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartConfigView, FlowchartLabelMetricsRequest,
    FlowchartRenderModelRef, FlowchartShape, flowchart_effective_edge_label_text_style,
    flowchart_effective_node_class_names, flowchart_effective_text_style_for_node_classes,
    flowchart_label_is_empty_for_render, flowchart_label_metrics_for_layout,
    flowchart_label_plain_text_for_layout, flowchart_split_mermaid_style_decls,
};
use crate::model::{Bounds, FlowchartLayout, LayoutCluster, LayoutEdge, LayoutNode};
use crate::render_geometry::{
    FlowchartCurveKind, flowchart_curve_segments as shared_flowchart_curve_segments,
};
use crate::text::{TextStyle as RenderTextStyle, WrapMode};
use crate::{Error, Result};
use merman_core::diagrams::flowchart::{
    FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNodeProvenance,
};
use merman_core::svg_security::{MermaidNavigationSecurity, prepare_mermaid_navigation_uri};
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    BlendMode, Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, LineCap, LineJoin,
    MeasurementProvenance, Paint, PathResource, PathSegment, PathStyle, Point, Rect, ResourceId,
    SemanticAnnotation, SemanticRole, StrokeStyle, TextAnchor, TextBaseline, TextDirection,
    TextObligation, TextRun, TextStyle, Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;

/// SVG-only metadata retained beside the public Flowchart document.
#[derive(Debug, Clone)]
pub(crate) struct FlowchartSvgBody {
    pub(crate) diagram_type: String,
}

pub(crate) fn build_flowchart_document(
    artifact: &FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let mut builder = FlowchartBuilder::new(artifact, metadata, policy, session)?;
    builder.build()
}

struct FlowchartBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: FlowchartRenderModelRef<'a>,
    layout: &'a FlowchartLayout,
    config: FlowchartConfigView<'a>,
    node_style: RenderTextStyle,
    html_text_style: RenderTextStyle,
    font: FontDescriptor,
    node_fill: Color,
    node_border: Color,
    node_text: Color,
    line_color: Color,
    cluster_fill: Color,
    cluster_border: Color,
    cluster_text: Color,
    edge_label_background: Color,
    edge_stroke_width: f64,
    look_neo: bool,
    default_edge_styles: Vec<String>,
    default_curve: String,
    edge_corner_radius: f64,
    compact_edge_corners: bool,
    security_level_loose: bool,
    nodes_by_id: HashMap<&'a str, &'a FlowNode>,
    edges_by_id: HashMap<&'a str, &'a FlowEdge>,
    projected_edges_by_id: HashMap<String, FlowEdge>,
    subgraphs_by_id: HashMap<&'a str, (usize, &'a FlowSubgraph)>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    extensions: BTreeMap<String, Value>,
    interaction_nodes: Vec<Value>,
}

impl<'a> FlowchartBuilder<'a> {
    fn new(
        artifact: &'a FlowchartFamilyArtifact<FlowchartLayout>,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let semantic = artifact.pair().semantic();
        let model = FlowchartRenderModelRef::new(semantic, artifact.render_context());
        let layout = artifact.pair().layout();
        let config = FlowchartConfigView::new(metadata.effective_config.as_value());

        let look = config_diagram_look(metadata.effective_config.as_value());
        if look.as_str().eq_ignore_ascii_case("handDrawn") {
            return Err(unavailable(
                "hand-drawn RoughJS output has no stable vector equivalent in DrawingList v1; use the SVG target or a future explicit raster fallback",
            ));
        }
        if model.requires_math() {
            return Err(unavailable(
                "Math labels require a math renderer or an explicit raster fallback; DrawingList v1 will not replace them with plain text",
            ));
        }

        let bounds = layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Flowchart layout did not provide root bounds"))?;
        validate_bounds(bounds)?;

        let font_family = config.font_family();
        let font_size = config.render_font_size();
        let node_style = config.render_text_style(&font_family, font_size);
        let html_text_style = config.html_label_measurement_base_style(&node_style);
        let font = FontDescriptor {
            families: parse_font_families(font_family),
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        let node_fill = theme_color(metadata.effective_config.as_value(), "mainBkg", "#ECECFF")?;
        let node_border = theme_color(
            metadata.effective_config.as_value(),
            "nodeBorder",
            "#9370DB",
        )?;
        let node_text = theme_color(
            metadata.effective_config.as_value(),
            "nodeTextColor",
            "#333",
        )?;
        let line_color = theme_color(metadata.effective_config.as_value(), "lineColor", "#333")?;
        let cluster_fill = theme_color(
            metadata.effective_config.as_value(),
            "clusterBkg",
            "#ffffde",
        )?;
        let cluster_border = theme_color(
            metadata.effective_config.as_value(),
            "clusterBorder",
            "#aaaa33",
        )?;
        let cluster_text = theme_color(metadata.effective_config.as_value(), "titleColor", "#333")?;
        let edge_label_background = theme_color(
            metadata.effective_config.as_value(),
            "edgeLabelBackground",
            "rgba(232,232,232, 0.8)",
        )?;
        let edge_stroke_width = metadata
            .effective_config
            .as_value()
            .get("themeVariables")
            .and_then(|variables| variables.get("strokeWidth"))
            .and_then(|value| {
                value.as_f64().or_else(|| {
                    value
                        .as_str()?
                        .trim()
                        .strip_suffix("px")
                        .unwrap_or_else(|| value.as_str().unwrap_or_default())
                        .trim()
                        .parse()
                        .ok()
                })
            })
            .filter(|value: &f64| value.is_finite() && *value >= 0.0)
            .unwrap_or(1.0);

        let is_elk_layout = metadata.diagram_type == "flowchart-elk"
            || metadata
                .effective_config
                .as_value()
                .get("layout")
                .and_then(Value::as_str)
                .is_some_and(|layout| layout.eq_ignore_ascii_case("elk"));
        let default_curve = artifact
            .pair()
            .semantic()
            .edge_defaults
            .as_ref()
            .and_then(|defaults| defaults.interpolate.clone())
            .or_else(|| {
                is_elk_layout
                    .then_some("rounded".to_string())
                    .or_else(|| config.render_curve())
            })
            .unwrap_or_else(|| "basis".to_string());
        ensure_curve_supported(&default_curve)?;
        let presentation = artifact.policy().unwrap_or_default();

        let nodes_by_id = semantic
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let edges_by_id = semantic
            .edges
            .iter()
            .map(|edge| (edge.id.as_str(), edge))
            .collect::<HashMap<_, _>>();
        let mut projected_edges_by_id = HashMap::with_capacity(model.edges.len());
        for edge in &model.edges {
            if let Some(projected) =
                crate::flowchart::project_flowchart_edge(edge, artifact.render_context())
            {
                projected_edges_by_id.insert(projected.id.clone(), projected);
            }
        }
        let subgraphs_by_id = semantic
            .subgraphs
            .iter()
            .enumerate()
            .map(|(index, subgraph)| (subgraph.id.as_str(), (index, subgraph)))
            .collect::<HashMap<_, _>>();

        let builder = Self {
            metadata,
            session,
            policy,
            model,
            layout,
            config,
            node_style,
            html_text_style,
            font,
            node_fill,
            node_border,
            node_text,
            line_color,
            cluster_fill,
            cluster_border,
            cluster_text,
            edge_label_background,
            edge_stroke_width,
            look_neo: look.as_str().eq_ignore_ascii_case("neo"),
            default_edge_styles: model
                .edge_defaults
                .as_ref()
                .map(|defaults| defaults.style.clone())
                .unwrap_or_default(),
            default_curve,
            edge_corner_radius: presentation
                .edge_corner_radius
                .unwrap_or_else(|| {
                    metadata
                        .effective_config
                        .as_value()
                        .get("themeVariables")
                        .and_then(|variables| variables.get("radius"))
                        .and_then(Value::as_f64)
                        .unwrap_or(5.0)
                })
                .max(0.0),
            compact_edge_corners: presentation.compact_edge_corners,
            security_level_loose: metadata.effective_config.get_str("securityLevel")
                == Some("loose"),
            nodes_by_id,
            edges_by_id,
            projected_edges_by_id,
            subgraphs_by_id,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "flowchart.document".to_string(),
                },
            ],
            semantics: Vec::new(),
            extensions: BTreeMap::new(),
            interaction_nodes: Vec::new(),
        };
        builder.preflight_sources()?;
        Ok(builder)
    }

    fn build(&mut self) -> Result<RenderDocument> {
        self.session.checkpoint(OperationPhase::Emit)?;
        self.add_document_semantics();

        // Mermaid's stable Flowchart DOM partitions clusters, edge paths/labels, and nodes. The
        // same partition is useful to native consumers because node fills do not erase routes.
        for (index, cluster) in self.layout.clusters.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_cluster(index, cluster)?;
        }
        for edge_layout in &self.layout.edges {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge(edge_layout)?;
        }
        for node_layout in &self.layout.nodes {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_node(node_layout)?;
        }

        if !self.interaction_nodes.is_empty() {
            self.extensions.insert(
                "x-merman-flowchart-interactions".to_string(),
                Value::Array(std::mem::take(&mut self.interaction_nodes)),
            );
        }
        self.extensions.insert(
            "x-merman-flowchart".to_string(),
            json!({
                "diagram_type": self.metadata.diagram_type,
                "uses_elk_adapter_dom": self.layout.uses_elk_adapter_dom,
                "label_modes": {
                    "node_html": self.config.node_html_labels(),
                    "edge_html": self.config.effective_html_labels(),
                },
            }),
        );

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                bounds.min_x,
                bounds.min_y,
                bounds.max_x - bounds.min_x,
                bounds.max_y - bounds.min_y,
            )),
            policy: self.policy,
            resources: std::mem::take(&mut self.resources),
            commands: std::mem::take(&mut self.commands),
            semantics: std::mem::take(&mut self.semantics),
            fallbacks: Vec::new(),
            extensions: std::mem::take(&mut self.extensions),
        };
        document.validate().map_err(Error::DrawingListContract)?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: crate::family::RenderFamilyKind::Flowchart,
                body: SvgStructureBody::Flowchart(FlowchartSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                }),
            },
        })
    }

    fn preflight_sources(&self) -> Result<()> {
        for node in &self.model.nodes {
            if node.is_subgraph_anchor() {
                if node.icon.is_some() || node.img.is_some() || node.label.is_some() {
                    return Err(unavailable(format!(
                        "synthetic subgraph anchor `{}` carries visible node content",
                        node.id
                    )));
                }
                continue;
            }
            if node.icon.is_some() || node.img.is_some() {
                return Err(unavailable(format!(
                    "node `{}` uses an icon/image asset; DrawingList v1 requires an explicit image or raster resource",
                    node.id
                )));
            }
            let shape_name = node.layout_shape.as_deref().unwrap_or("squareRect");
            let shape = FlowchartShape::resolve(shape_name)
                .map_err(|error| unavailable(format!("node `{}`: {error}", node.id)))?;
            if !portable_shape(shape) {
                return Err(unavailable(format!(
                    "node `{}` uses shape `{shape_name}`, which has no portable DrawingList v1 geometry adapter",
                    node.id
                )));
            }
            let label = self
                .model
                .node_label_for_render(node)
                .unwrap_or(node.id.as_str());
            self.validate_label(
                label,
                node.label_type.as_deref().unwrap_or("text"),
                self.config.node_html_labels(),
                &format!("node `{}`", node.id),
            )?;
        }

        for subgraph in &self.model.subgraphs {
            if self.model.is_subgraph_collapsed(&subgraph.id) {
                return Err(unavailable(format!(
                    "collapsed subgraph `{}` needs its indicator/separator geometry before it can be emitted without loss",
                    subgraph.id
                )));
            }
        }

        for edge in &self.model.edges {
            if edge.animate == Some(true) || edge.animation.is_some() {
                return Err(unavailable(format!(
                    "edge `{}` uses animation, which DrawingList v1 does not silently discard",
                    edge.id
                )));
            }
            if let Some(interpolate) = edge.interpolate.as_deref() {
                ensure_curve_supported(interpolate)?;
            }
            self.validate_style_declarations(
                &edge.classes,
                &self.default_edge_styles,
                &edge.style,
                &format!("edge `{}`", edge.id),
            )?;
            if let Some(label) = self.model.edge_label_for_render(edge)
                && !flowchart_label_is_empty_for_render(label)
            {
                self.validate_label(
                    label,
                    edge.label_type.as_deref().unwrap_or("text"),
                    self.config.effective_html_labels(),
                    &format!("edge `{}` label", edge.id),
                )?;
            }
        }

        for (index, subgraph) in self.model.subgraphs.iter().enumerate() {
            let (classes, styles) = self.model.effective_subgraph_css(index, subgraph);
            self.validate_style_declarations(
                classes,
                &[],
                styles,
                &format!("subgraph `{}`", subgraph.id),
            )?;
            self.validate_label(
                self.model.subgraph_title_for_render(index, subgraph),
                subgraph.label_type.as_deref().unwrap_or("text"),
                self.config.effective_html_labels(),
                &format!("subgraph `{}` title", subgraph.id),
            )?;
        }

        for layout_node in &self.layout.nodes {
            if ![
                layout_node.x,
                layout_node.y,
                layout_node.width,
                layout_node.height,
            ]
            .into_iter()
            .all(f64::is_finite)
                || layout_node.width < 0.0
                || layout_node.height < 0.0
            {
                return Err(invalid(format!(
                    "layout node `{}` has invalid geometry",
                    layout_node.id
                )));
            }
            if layout_node.is_cluster {
                if !self.subgraphs_by_id.contains_key(layout_node.id.as_str()) {
                    return Err(invalid(format!(
                        "layout cluster `{}` has no semantic subgraph",
                        layout_node.id
                    )));
                }
            } else if let Some(node) = self.nodes_by_id.get(layout_node.id.as_str()) {
                if node.is_subgraph_anchor() {
                    continue;
                }
            } else {
                return Err(invalid(format!(
                    "layout node `{}` has no semantic Flowchart node",
                    layout_node.id
                )));
            }
        }
        for layout_edge in &self.layout.edges {
            if !self.edges_by_id.contains_key(layout_edge.id.as_str())
                && !self
                    .projected_edges_by_id
                    .contains_key(layout_edge.id.as_str())
            {
                return Err(invalid(format!(
                    "layout edge `{}` has no semantic Flowchart edge",
                    layout_edge.id
                )));
            }
            if layout_edge
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
            {
                return Err(invalid(format!(
                    "layout edge `{}` has non-finite points",
                    layout_edge.id
                )));
            }
            if let Some(label) = &layout_edge.label {
                let invalid_label = [label.x, label.y]
                    .into_iter()
                    .any(|value| !value.is_finite())
                    || [label.width, label.height]
                        .into_iter()
                        .any(|value| !value.is_finite() || value < 0.0);
                if invalid_label {
                    return Err(invalid(format!(
                        "layout edge `{}` has invalid label geometry",
                        layout_edge.id
                    )));
                }
            }
        }
        Ok(())
    }

    fn add_document_semantics(&mut self) {
        let title = self
            .model
            .acc_title
            .clone()
            .or_else(|| self.metadata.title.clone())
            .or_else(|| Some(self.metadata.diagram_type.clone()));
        self.semantics.push(SemanticAnnotation {
            id: "flowchart.document".to_string(),
            role: SemanticRole::Document,
            title,
            description: self.model.acc_descr.clone(),
            link: None,
        });
    }

    fn emit_cluster(&mut self, index: usize, cluster: &LayoutCluster) -> Result<()> {
        let (subgraph_index, subgraph) = self
            .subgraphs_by_id
            .get(cluster.id.as_str())
            .copied()
            .ok_or_else(|| {
                invalid(format!(
                    "layout cluster `{}` has no semantic subgraph",
                    cluster.id
                ))
            })?;
        let (classes, styles) = self.model.effective_subgraph_css(subgraph_index, subgraph);
        let style = self.resolve_style(
            classes,
            &[],
            styles,
            self.base_text_style(self.config.effective_html_labels()),
            self.cluster_fill,
            self.cluster_border,
            self.cluster_text,
            1.0,
        )?;
        let semantic_id = format!("flowchart.group.{index}");
        self.begin_group(&semantic_id, style.opacity, style.blend_mode);
        if style.has_paint()
            && let Some(path) =
                rectangle_path(cluster.x, cluster.y, cluster.width, cluster.height, 0.0)
        {
            self.add_path(format!("{semantic_id}.body"), path, style.path_style())?;
        }
        let raw_title = self
            .model
            .subgraph_title_for_render(subgraph_index, subgraph)
            .to_string();
        let label = self.label_run(
            &raw_title,
            subgraph.label_type.as_deref().unwrap_or("text"),
            self.config.effective_html_labels(),
            Point::new(cluster.title_label.x, cluster.title_label.y),
            Rect::new(
                cluster.title_label.x - cluster.title_label.width / 2.0,
                cluster.title_label.y - cluster.title_label.height / 2.0,
                cluster.title_label.width,
                cluster.title_label.height,
            ),
            &style,
        )?;
        self.commands.push(DrawingCommand::DrawText { run: label });
        self.end_group(style.opacity != 1.0 || style.blend_mode != BlendMode::Normal);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(flowchart_label_plain_text_for_layout(
                &raw_title,
                subgraph.label_type.as_deref().unwrap_or("text"),
                self.config.effective_html_labels(),
            )),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_edge(&mut self, layout_edge: &LayoutEdge) -> Result<()> {
        let edge = self
            .edges_by_id
            .get(layout_edge.id.as_str())
            .copied()
            .map(Clone::clone)
            .or_else(|| {
                self.projected_edges_by_id
                    .get(layout_edge.id.as_str())
                    .cloned()
            })
            .ok_or_else(|| {
                invalid(format!(
                    "layout edge `{}` has no semantic edge",
                    layout_edge.id
                ))
            })?;
        if edge.visibility == FlowEdgeVisibility::Invisible {
            self.semantics.push(self.edge_semantic(&edge, None));
            return Ok(());
        }
        if layout_edge.points.len() < 2 {
            return Err(unavailable(format!(
                "visible edge `{}` has fewer than two routed points",
                edge.id
            )));
        }
        let style = self.resolve_style(
            &edge.classes,
            &self.default_edge_styles,
            &edge.style,
            self.base_text_style(self.config.effective_html_labels()),
            self.line_color,
            self.line_color,
            self.node_text,
            self.edge_stroke_width,
        )?;
        let semantic_id = format!("flowchart.edge.{}", edge.id);
        self.begin_group(&semantic_id, style.opacity, style.blend_mode);
        let curve = edge.interpolate.as_deref().unwrap_or(&self.default_curve);
        let segments = curve_segments(
            &layout_edge.points,
            curve,
            self.edge_corner_radius,
            self.compact_edge_corners,
        )?;
        if let Some(stroke_color) = style.stroke_color {
            self.add_path(
                format!("{semantic_id}.route"),
                segments,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(style.stroke_style(edge.stroke_kind, stroke_color)),
                },
            )?;
            self.emit_marker(
                &format!("{semantic_id}.start-marker"),
                edge.start_marker,
                marker_point(&layout_edge.points, MarkerPosition::Start),
                stroke_color,
                self.line_color,
            )?;
            self.emit_marker(
                &format!("{semantic_id}.end-marker"),
                edge.end_marker,
                marker_point(&layout_edge.points, MarkerPosition::End),
                stroke_color,
                self.line_color,
            )?;
        }

        if let Some(raw_label) = self.model.edge_label_for_render(&edge).map(str::to_string)
            && !flowchart_label_is_empty_for_render(&raw_label)
        {
            let label_layout = layout_edge.label.as_ref().ok_or_else(|| {
                unavailable(format!(
                    "edge `{}` has a label without layout bounds",
                    edge.id
                ))
            })?;
            let label_style = self.resolve_style(
                &edge.classes,
                &self.default_edge_styles,
                &edge.style,
                self.base_text_style(self.config.effective_html_labels()),
                self.edge_label_background,
                self.edge_label_background,
                self.node_text,
                self.edge_stroke_width,
            )?;
            self.add_path(
                format!("{semantic_id}.label-background"),
                rectangle_path(
                    label_layout.x,
                    label_layout.y,
                    label_layout.width,
                    label_layout.height,
                    0.0,
                )
                .ok_or_else(|| invalid(format!("edge `{}` label bounds are invalid", edge.id)))?,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.edge_label_background)),
                    stroke: None,
                },
            )?;
            let run = self.label_run(
                &raw_label,
                edge.label_type.as_deref().unwrap_or("text"),
                self.config.effective_html_labels(),
                Point::new(label_layout.x, label_layout.y),
                Rect::new(
                    label_layout.x - label_layout.width / 2.0,
                    label_layout.y - label_layout.height / 2.0,
                    label_layout.width,
                    label_layout.height,
                ),
                &label_style,
            )?;
            self.commands.push(DrawingCommand::DrawText { run });
        }
        self.end_group(style.opacity != 1.0 || style.blend_mode != BlendMode::Normal);
        self.semantics
            .push(self.edge_semantic(&edge, Some(semantic_id)));
        Ok(())
    }

    fn emit_node(&mut self, layout_node: &LayoutNode) -> Result<()> {
        if layout_node.is_cluster {
            return Ok(());
        }
        let node = self
            .nodes_by_id
            .get(layout_node.id.as_str())
            .copied()
            .ok_or_else(|| {
                invalid(format!(
                    "layout node `{}` has no semantic node",
                    layout_node.id
                ))
            })?;
        if node.provenance == FlowNodeProvenance::SubgraphAnchor {
            return Ok(());
        }
        let semantic_id = format!("flowchart.node.{}", node.id);
        let classes = flowchart_effective_node_class_names(&self.model.class_defs, &node.classes)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let style = self.resolve_style(
            &classes,
            &[],
            &node.styles,
            self.base_text_style(self.config.node_html_labels()),
            self.node_fill,
            self.node_border,
            self.node_text,
            1.3,
        )?;
        self.begin_group(&semantic_id, style.opacity, style.blend_mode);
        let shape = FlowchartShape::resolve(node.layout_shape.as_deref().unwrap_or("squareRect"))
            .map_err(|error| unavailable(format!("node `{}`: {error}", node.id)))?;
        if shape != FlowchartShape::Text && style.has_paint() {
            for (part, path) in node_paths(shape, layout_node, self.config_diagram_look())? {
                self.add_path(
                    format!("{semantic_id}.shape.{part}"),
                    path,
                    style.path_style(),
                )?;
            }
        }
        let raw_label = self
            .model
            .node_label_for_render(node)
            .unwrap_or(node.id.as_str());
        let label_text = flowchart_label_plain_text_for_layout(
            raw_label,
            node.label_type.as_deref().unwrap_or("text"),
            self.config.node_html_labels(),
        );
        let label_bounds = node_label_bounds(
            layout_node,
            &label_text,
            node.label_type.as_deref().unwrap_or("text"),
            &self.node_style_for(node),
            self.config.node_html_labels(),
            self.config.node_wrap_mode(),
            self.config.render_wrapping_width(),
            &self.metadata.effective_config,
            self.session,
        )?;
        let run = self.label_run(
            raw_label,
            node.label_type.as_deref().unwrap_or("text"),
            self.config.node_html_labels(),
            Point::new(layout_node.x, layout_node.y),
            label_bounds,
            &style,
        )?;
        if !label_text.is_empty() {
            self.commands.push(DrawingCommand::DrawText { run });
        }
        let target = self
            .security_level_loose
            .then_some(node.link_target.as_deref())
            .flatten()
            .map(str::trim)
            .filter(|target| !target.is_empty());
        if node.have_callback || target.is_some() {
            self.interaction_nodes.push(json!({
                "semantic_id": semantic_id,
                "source_id": node.id,
                "callback": node.have_callback,
                "target": target,
            }));
        }
        self.end_group(style.opacity != 1.0 || style.blend_mode != BlendMode::Normal);
        self.semantics.push(self.node_semantic(node, semantic_id));
        Ok(())
    }

    fn config_diagram_look(&self) -> &str {
        config_diagram_look(self.metadata.effective_config.as_value()).as_str()
    }

    fn base_text_style(&self, html_labels: bool) -> &RenderTextStyle {
        if html_labels {
            &self.html_text_style
        } else {
            &self.node_style
        }
    }

    fn node_style_for(&self, node: &FlowNode) -> RenderTextStyle {
        let style = flowchart_effective_text_style_for_node_classes(
            &self.node_style,
            &self.model.class_defs,
            &node.classes,
            &node.styles,
        );
        style.into_owned()
    }

    fn label_run(
        &self,
        raw_label: &str,
        label_type: &str,
        html_labels: bool,
        origin: Point,
        bounds: Rect,
        style: &ResolvedStyle,
    ) -> Result<TextRun> {
        let text = flowchart_label_plain_text_for_layout(raw_label, label_type, html_labels);
        let render_style = &style.text_style;
        let font = font_descriptor_from_style(&self.font, render_style)?;
        Ok(TextRun {
            text,
            origin,
            bounds,
            style: TextStyle {
                font,
                font_size: render_style.font_size,
                letter_spacing: style.letter_spacing,
                line_height: style.line_height,
                fill: Paint::solid(style.text_color),
            },
            anchor: TextAnchor::Middle,
            baseline: TextBaseline::Middle,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation(),
        })
    }

    fn text_obligation(&self) -> TextObligation {
        let route = self
            .session
            .text_measurement_route(TextMeasurementPhase::Layout);
        let profile = profile_identity(&route.primary);
        let measurement = match route.primary_source {
            TextMeasurementSource::Host => MeasurementProvenance::HostCallback { profile },
            TextMeasurementSource::Profile => {
                MeasurementProvenance::DeterministicFallback { profile }
            }
        };
        TextObligation::HostText { measurement }
    }

    fn begin_group(&mut self, semantic_id: &str, opacity: f64, blend_mode: BlendMode) {
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        });
        if opacity != 1.0 || blend_mode != BlendMode::Normal {
            self.commands.push(DrawingCommand::Save);
            if opacity != 1.0 {
                self.commands.push(DrawingCommand::SetOpacity { opacity });
            }
            if blend_mode != BlendMode::Normal {
                self.commands
                    .push(DrawingCommand::SetBlendMode { blend_mode });
            }
        }
    }

    fn end_group(&mut self, had_state: bool) {
        if had_state {
            self.commands.push(DrawingCommand::Restore);
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Ok(());
        }
        let id = ResourceId::new(id);
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        Ok(())
    }

    fn emit_marker(
        &mut self,
        id: &str,
        marker: FlowEdgeMarker,
        endpoint: Option<MarkerEndpoint>,
        stroke_color: Color,
        base_fill_color: Color,
    ) -> Result<()> {
        let Some(endpoint) = endpoint else {
            return Ok(());
        };
        let Some((segments, fill, stroke)) =
            marker_segments(marker, endpoint, stroke_color, base_fill_color)
        else {
            return Ok(());
        };
        self.add_path(
            id.to_string(),
            segments,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill,
                stroke,
            },
        )
    }

    fn resolve_style(
        &self,
        classes: &[String],
        styles_a: &[String],
        styles_b: &[String],
        base_text_style: &RenderTextStyle,
        default_fill: Color,
        default_stroke: Color,
        default_text: Color,
        default_stroke_width: f64,
    ) -> Result<ResolvedStyle> {
        let text_style = base_text_style;
        let effective_text = if classes.iter().any(|class| class == "node") {
            flowchart_effective_text_style_for_node_classes(
                text_style,
                &self.model.class_defs,
                classes,
                styles_b,
            )
        } else {
            flowchart_effective_edge_label_text_style(
                text_style,
                &self.model.class_defs,
                classes,
                styles_a,
                styles_b,
            )
        };
        let mut result = ResolvedStyle::defaults(
            effective_text.into_owned(),
            default_fill,
            default_stroke,
            default_text,
            default_stroke_width,
        );
        if self.look_neo {
            result.line_cap = LineCap::Round;
            result.line_join = LineJoin::Round;
        }
        let declarations =
            collect_declarations(&self.model.class_defs, classes, styles_a, styles_b)?;
        for (key, value) in declarations {
            result.apply(&key, &value)?;
        }
        Ok(result)
    }

    fn validate_style_declarations(
        &self,
        classes: &[String],
        styles_a: &[String],
        styles_b: &[String],
        owner: &str,
    ) -> Result<()> {
        let _ = self
            .resolve_style(
                classes,
                styles_a,
                styles_b,
                &self.node_style,
                self.node_fill,
                self.node_border,
                self.node_text,
                1.3,
            )
            .map_err(|error| match error {
                Error::DrawingListUnavailable { reason, .. } => {
                    unavailable(format!("{owner}: {reason}"))
                }
                other => other,
            })?;
        Ok(())
    }

    fn validate_label(
        &self,
        label: &str,
        label_type: &str,
        _html_labels: bool,
        owner: &str,
    ) -> Result<()> {
        if label_type.eq_ignore_ascii_case("markdown") {
            return Err(unavailable(format!(
                "{owner} uses Markdown rich text; DrawingList v1 needs an explicit rich-text run contract before emitting it"
            )));
        }
        if crate::math::contains_delimited_math(label) {
            return Err(unavailable(format!("{owner} contains a math expression")));
        }
        if label.contains("fa:fa-")
            || label.contains("fas:fa-")
            || label.contains("far:fa-")
            || label.contains("fab:fa-")
        {
            return Err(unavailable(format!("{owner} contains a Font Awesome icon")));
        }
        if let Some(tag) = first_non_textual_html_tag(label) {
            return Err(unavailable(format!(
                "{owner} contains HTML tag `{tag}`; only plain text and line breaks are portable in DrawingList v1"
            )));
        }
        Ok(())
    }

    fn node_semantic(&self, node: &FlowNode, id: String) -> SemanticAnnotation {
        let link = node.link.as_deref().and_then(|raw| {
            prepare_mermaid_navigation_uri(
                raw,
                MermaidNavigationSecurity::from_security_level_loose(self.security_level_loose),
            )
        });
        SemanticAnnotation {
            id,
            role: SemanticRole::Node,
            title: Some(
                self.model
                    .node_label_for_render(node)
                    .unwrap_or(node.id.as_str())
                    .to_string(),
            ),
            description: self.model.tooltips.get(&node.id).cloned(),
            link,
        }
    }

    fn edge_semantic(&self, edge: &FlowEdge, id: Option<String>) -> SemanticAnnotation {
        SemanticAnnotation {
            id: id.unwrap_or_else(|| format!("flowchart.edge.{}", edge.id)),
            role: SemanticRole::Edge,
            title: self.model.edge_label_for_render(edge).map(str::to_string),
            description: Some(format!("{} → {}", edge.from, edge.to)),
            link: None,
        }
    }
}

#[derive(Clone)]
struct ResolvedStyle {
    fill_color: Option<Color>,
    stroke_color: Option<Color>,
    stroke_width: f64,
    dash_array: Vec<f64>,
    dash_offset: f64,
    line_cap: LineCap,
    line_join: LineJoin,
    miter_limit: f64,
    opacity: f64,
    fill_opacity: f64,
    stroke_opacity: f64,
    blend_mode: BlendMode,
    text_color: Color,
    text_style: RenderTextStyle,
    letter_spacing: f64,
    line_height: f64,
    line_height_explicit: bool,
}

impl ResolvedStyle {
    fn defaults(
        text_style: RenderTextStyle,
        fill: Color,
        stroke: Color,
        text: Color,
        stroke_width: f64,
    ) -> Self {
        let line_height = if text_style.font_size > 0.0 {
            text_style.font_size * 1.5
        } else {
            24.0
        };
        Self {
            fill_color: Some(fill),
            stroke_color: Some(stroke),
            stroke_width,
            dash_array: Vec::new(),
            dash_offset: 0.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
            opacity: 1.0,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            blend_mode: BlendMode::Normal,
            text_color: text,
            text_style,
            letter_spacing: 0.0,
            line_height,
            line_height_explicit: false,
        }
    }

    fn apply(&mut self, key: &str, raw_value: &str) -> Result<()> {
        let value = raw_value
            .trim()
            .strip_suffix("!important")
            .map(str::trim)
            .unwrap_or(raw_value.trim());
        match key.trim().to_ascii_lowercase().as_str() {
            "fill" => self.fill_color = parse_css_color(value, "fill")?,
            "stroke" => self.stroke_color = parse_css_color(value, "stroke")?,
            "stroke-width" => self.stroke_width = parse_css_number(value, "stroke-width")?,
            "stroke-dasharray" => self.dash_array = parse_dash_array(value)?,
            "stroke-dashoffset" => self.dash_offset = parse_css_number(value, "stroke-dashoffset")?,
            "stroke-linecap" => self.line_cap = parse_line_cap(value)?,
            "stroke-linejoin" => self.line_join = parse_line_join(value)?,
            "stroke-miterlimit" => {
                let limit = parse_css_number(value, "stroke-miterlimit")?;
                if limit <= 0.0 {
                    return Err(unavailable("stroke-miterlimit must be greater than zero"));
                }
                self.miter_limit = limit;
            }
            "opacity" => self.opacity = parse_unit(value, "opacity")?,
            "fill-opacity" => self.fill_opacity = parse_unit(value, "fill-opacity")?,
            "stroke-opacity" => self.stroke_opacity = parse_unit(value, "stroke-opacity")?,
            "mix-blend-mode" => self.blend_mode = parse_blend_mode(value)?,
            "color" => {
                self.text_color = parse_css_color(value, "color")?
                    .ok_or_else(|| unavailable("text color cannot be none"))?
            }
            "font-family" => {
                let family = crate::config::normalize_css_font_family(value);
                if family.is_empty() {
                    return Err(unavailable("font-family is not a safe portable CSS value"));
                }
                self.text_style.font_family = Some(family);
            }
            "font-size" => {
                let size =
                    crate::mermaid_style::parse_css_font_size_px(value, self.text_style.font_size)
                        .ok_or_else(|| {
                            unavailable(format!("font-size `{value}` is not portable"))
                        })?;
                self.text_style.font_size = size;
                if !self.line_height_explicit {
                    self.line_height = size * 1.5;
                }
            }
            "font-weight" => {
                parse_font_weight(value)?;
                self.text_style.font_weight = Some(value.to_string());
            }
            "font-style" => {
                parse_font_style(value)?;
                self.text_style.font_style = Some(value.to_string());
            }
            "letter-spacing" => {
                self.letter_spacing = parse_css_number_allow_normal(value, "letter-spacing")?
            }
            "line-height" => {
                self.line_height = parse_line_height(value, self.text_style.font_size)?;
                self.line_height_explicit = true;
            }
            "text-decoration" | "text-align" | "text-transform" | "word-spacing"
            | "text-shadow" | "text-overflow" | "white-space" | "word-wrap" | "word-break"
            | "overflow-wrap" | "hyphens" => {
                return Err(unavailable(format!(
                    "CSS property `{key}` needs a richer text contract"
                )));
            }
            "background" | "background-color" | "border" | "border-color" | "border-width"
            | "padding" | "margin" | "filter" | "mask" | "clip-path" | "transform" | "display"
            | "visibility" | "animation" | "transition" | "cursor" | "pointer-events"
            | "paint-order" | "stroke-alignment" => {
                return Err(unavailable(format!(
                    "CSS property `{key}` has no lossless DrawingList v1 mapping"
                )));
            }
            _ => {
                // Mermaid's own class style compiler keeps unknown declarations in the SVG CSS.
                // A public renderer-neutral target cannot safely assume that an unknown property
                // is non-visual, so fail closed instead of dropping it.
                return Err(unavailable(format!(
                    "CSS property `{key}` is not in the portable v1 subset"
                )));
            }
        }
        Ok(())
    }

    fn path_style(&self) -> PathStyle {
        PathStyle {
            fill_rule: FillRule::NonZero,
            fill: self
                .fill_color
                .map(|color| Paint::solid(with_alpha(color, self.fill_opacity))),
            stroke: self.stroke_color.map(|color| StrokeStyle {
                paint: Paint::solid(with_alpha(color, self.stroke_opacity)),
                width: self.stroke_width,
                dash_array: self.dash_array.clone(),
                dash_offset: self.dash_offset,
                line_cap: self.line_cap,
                line_join: self.line_join,
                miter_limit: self.miter_limit,
            }),
        }
    }

    fn has_paint(&self) -> bool {
        self.fill_color.is_some() || self.stroke_color.is_some()
    }

    fn stroke_style(&self, kind: FlowEdgeStroke, color: Color) -> StrokeStyle {
        let width = match kind {
            FlowEdgeStroke::Thick => self.stroke_width.max(3.5),
            _ => self.stroke_width,
        };
        let dash_array = match kind {
            FlowEdgeStroke::Dotted if self.dash_array.is_empty() => vec![2.0],
            _ => self.dash_array.clone(),
        };
        StrokeStyle {
            paint: Paint::solid(with_alpha(color, self.stroke_opacity)),
            width,
            dash_array,
            dash_offset: self.dash_offset,
            line_cap: self.line_cap,
            line_join: self.line_join,
            miter_limit: self.miter_limit,
        }
    }
}

fn portable_shape(shape: FlowchartShape) -> bool {
    matches!(
        shape,
        FlowchartShape::Process
            | FlowchartShape::RoundedRectangle
            | FlowchartShape::Diamond
            | FlowchartShape::Circle
            | FlowchartShape::DoubleCircle
            | FlowchartShape::Stadium
            | FlowchartShape::Hexagon
            | FlowchartShape::Trapezoid
            | FlowchartShape::InvertedTrapezoid
            | FlowchartShape::LeanLeft
            | FlowchartShape::LeanRight
            | FlowchartShape::Subroutine
            | FlowchartShape::Text
    )
}

fn node_paths(
    shape: FlowchartShape,
    node: &LayoutNode,
    look: &str,
) -> Result<Vec<(String, Vec<PathSegment>)>> {
    let x = node.x;
    let y = node.y;
    let w = node.width.max(1.0);
    let h = node.height.max(1.0);
    let radius = if shape == FlowchartShape::Process && look.eq_ignore_ascii_case("neo") {
        5.0
    } else if shape == FlowchartShape::RoundedRectangle {
        5.0
    } else {
        0.0
    };
    let result = match shape {
        FlowchartShape::Process | FlowchartShape::RoundedRectangle | FlowchartShape::Text => {
            vec![(
                "outer".to_string(),
                rectangle_path(x, y, w, h, radius).unwrap(),
            )]
        }
        FlowchartShape::Diamond => vec![(
            "outer".to_string(),
            polygon_path(&[
                Point::new(x - w / 2.0, y),
                Point::new(x, y - h / 2.0),
                Point::new(x + w / 2.0, y),
                Point::new(x, y + h / 2.0),
            ]),
        )],
        FlowchartShape::Circle => vec![(
            "outer".to_string(),
            ellipse_path(x, y, w.min(h) / 2.0, w.min(h) / 2.0),
        )],
        FlowchartShape::DoubleCircle => vec![
            (
                "outer".to_string(),
                ellipse_path(x, y, w.min(h) / 2.0, w.min(h) / 2.0),
            ),
            (
                "inner".to_string(),
                ellipse_path(
                    x,
                    y,
                    (w.min(h) / 2.0 - 5.0).max(0.5),
                    (w.min(h) / 2.0 - 5.0).max(0.5),
                ),
            ),
        ],
        FlowchartShape::Stadium => {
            vec![("outer".to_string(), rounded_rect_path(x, y, w, h, h / 2.0))]
        }
        FlowchartShape::Hexagon => {
            let inset = h / 4.0;
            vec![(
                "outer".to_string(),
                polygon_path(&[
                    Point::new(x - w / 2.0 + inset, y - h / 2.0),
                    Point::new(x + w / 2.0 - inset, y - h / 2.0),
                    Point::new(x + w / 2.0, y),
                    Point::new(x + w / 2.0 - inset, y + h / 2.0),
                    Point::new(x - w / 2.0 + inset, y + h / 2.0),
                    Point::new(x - w / 2.0, y),
                ]),
            )]
        }
        FlowchartShape::Trapezoid => vec![(
            "outer".to_string(),
            polygon_path(&[
                Point::new(x - w / 2.0, y + h / 2.0),
                Point::new(x + w / 2.0, y + h / 2.0),
                Point::new(x + (w - h).max(1.0) / 2.0, y - h / 2.0),
                Point::new(x - (w - h).max(1.0) / 2.0, y - h / 2.0),
            ]),
        )],
        FlowchartShape::InvertedTrapezoid => vec![(
            "outer".to_string(),
            polygon_path(&[
                Point::new(x - (w - h).max(1.0) / 2.0, y + h / 2.0),
                Point::new(x + (w - h).max(1.0) / 2.0, y + h / 2.0),
                Point::new(x + w / 2.0, y - h / 2.0),
                Point::new(x - w / 2.0, y - h / 2.0),
            ]),
        )],
        FlowchartShape::LeanLeft => vec![("outer".to_string(), lean_path(x, y, w, h, true))],
        FlowchartShape::LeanRight => vec![("outer".to_string(), lean_path(x, y, w, h, false))],
        FlowchartShape::Subroutine => vec![
            (
                "outer".to_string(),
                rectangle_path(x, y, w, h, 0.0).unwrap(),
            ),
            (
                "inner".to_string(),
                rectangle_path(x, y, (w - 16.0).max(1.0), h, 0.0).unwrap(),
            ),
        ],
        _ => return Err(unavailable(format!("shape `{shape:?}` is not portable"))),
    };
    Ok(result)
}

fn rectangle_path(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) -> Option<Vec<PathSegment>> {
    if ![x, y, width, height, radius]
        .into_iter()
        .all(f64::is_finite)
        || width < 0.0
        || height < 0.0
    {
        return None;
    }
    if radius <= 0.0 {
        return Some(polygon_path(&[
            Point::new(x - width / 2.0, y - height / 2.0),
            Point::new(x + width / 2.0, y - height / 2.0),
            Point::new(x + width / 2.0, y + height / 2.0),
            Point::new(x - width / 2.0, y + height / 2.0),
        ]));
    }
    Some(rounded_rect_path(x, y, width, height, radius))
}

fn rounded_rect_path(x: f64, y: f64, width: f64, height: f64, radius: f64) -> Vec<PathSegment> {
    let r = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    let left = x - width / 2.0;
    let right = x + width / 2.0;
    let top = y - height / 2.0;
    let bottom = y + height / 2.0;
    if r == 0.0 {
        return polygon_path(&[
            Point::new(left, top),
            Point::new(right, top),
            Point::new(right, bottom),
            Point::new(left, bottom),
        ]);
    }
    vec![
        PathSegment::MoveTo {
            to: Point::new(left + r, top),
        },
        PathSegment::LineTo {
            to: Point::new(right - r, top),
        },
        PathSegment::ArcTo {
            radius_x: r,
            radius_y: r,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(right, top + r),
        },
        PathSegment::LineTo {
            to: Point::new(right, bottom - r),
        },
        PathSegment::ArcTo {
            radius_x: r,
            radius_y: r,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(right - r, bottom),
        },
        PathSegment::LineTo {
            to: Point::new(left + r, bottom),
        },
        PathSegment::ArcTo {
            radius_x: r,
            radius_y: r,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(left, bottom - r),
        },
        PathSegment::LineTo {
            to: Point::new(left, top + r),
        },
        PathSegment::ArcTo {
            radius_x: r,
            radius_y: r,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(left + r, top),
        },
        PathSegment::Close,
    ]
}

fn polygon_path(points: &[Point]) -> Vec<PathSegment> {
    let mut segments = Vec::with_capacity(points.len() + 1);
    if let Some(first) = points.first().copied() {
        segments.push(PathSegment::MoveTo { to: first });
        segments.extend(
            points
                .iter()
                .skip(1)
                .copied()
                .map(|to| PathSegment::LineTo { to }),
        );
        segments.push(PathSegment::Close);
    }
    segments
}

fn ellipse_path(x: f64, y: f64, radius_x: f64, radius_y: f64) -> Vec<PathSegment> {
    let start = Point::new(x + radius_x, y);
    vec![
        PathSegment::MoveTo { to: start },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(x - radius_x, y),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: start,
        },
        PathSegment::Close,
    ]
}

fn lean_path(x: f64, y: f64, total_width: f64, height: f64, left: bool) -> Vec<PathSegment> {
    let w = (total_width - height).max(1.0);
    let dx = 3.0 * height / 6.0;
    let points = if left {
        [
            Point::new(x - w / 2.0, y + height / 2.0),
            Point::new(x + (w + dx) / 2.0, y + height / 2.0),
            Point::new(x + w / 2.0, y - height / 2.0),
            Point::new(x - (w + dx) / 2.0, y - height / 2.0),
        ]
    } else {
        [
            Point::new(x - (w + dx) / 2.0, y + height / 2.0),
            Point::new(x + w / 2.0, y + height / 2.0),
            Point::new(x + (w + dx) / 2.0, y - height / 2.0),
            Point::new(x - w / 2.0, y - height / 2.0),
        ]
    };
    polygon_path(&points)
}

fn node_label_bounds(
    node: &LayoutNode,
    label: &str,
    label_type: &str,
    style: &RenderTextStyle,
    html_labels: bool,
    wrap_mode: WrapMode,
    wrapping_width: f64,
    config: &merman_core::MermaidConfig,
    session: &RenderSession,
) -> Result<Rect> {
    match (node.label_width, node.label_height) {
        (Some(width), Some(height)) => {
            if [width, height].into_iter().all(f64::is_finite) && width >= 0.0 && height >= 0.0 {
                Ok(Rect::new(
                    node.x - width / 2.0,
                    node.y - height / 2.0,
                    width,
                    height,
                ))
            } else {
                Err(invalid(format!(
                    "node `{}` has invalid label dimensions",
                    node.id
                )))
            }
        }
        (None, None) => {
            let measurer = session
                .controlled_text_measurer(TextMeasurementPhase::Layout, OperationPhase::Emit);
            let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: label,
                label_type,
                style,
                max_width_px: Some(wrapping_width),
                wrap_mode: if html_labels {
                    WrapMode::HtmlLike
                } else {
                    wrap_mode
                },
                config,
                math_renderer: None,
            });
            if !metrics.width.is_finite()
                || !metrics.height.is_finite()
                || metrics.width < 0.0
                || metrics.height < 0.0
            {
                return Err(invalid(format!(
                    "node `{}` label metrics are invalid",
                    node.id
                )));
            }
            Ok(Rect::new(
                node.x - metrics.width / 2.0,
                node.y - metrics.height / 2.0,
                metrics.width,
                metrics.height,
            ))
        }
        _ => Err(invalid(format!(
            "node `{}` has only one label dimension",
            node.id
        ))),
    }
}

fn curve_segments(
    points: &[crate::model::LayoutPoint],
    curve: &str,
    radius: f64,
    compact: bool,
) -> Result<Vec<PathSegment>> {
    let kind = FlowchartCurveKind::from_mermaid_name(curve).ok_or_else(|| {
        unavailable(format!(
            "edge curve `{}` has no typed v1 adapter",
            curve.trim()
        ))
    })?;
    Ok(shared_flowchart_curve_segments(
        points, kind, radius, compact, None,
    ))
}

fn ensure_curve_supported(curve: &str) -> Result<()> {
    match curve.trim().to_ascii_lowercase().as_str() {
        "linear" | "step" | "stepbefore" | "stepafter" | "basis" | "rounded" => Ok(()),
        other => Err(unavailable(format!(
            "edge curve `{other}` has no typed v1 adapter"
        ))),
    }
}

#[derive(Clone, Copy)]
struct MarkerEndpoint {
    point: Point,
    direction: Point,
    position: MarkerPosition,
}

#[derive(Clone, Copy)]
enum MarkerPosition {
    Start,
    End,
}

fn marker_point(
    points: &[crate::model::LayoutPoint],
    position: MarkerPosition,
) -> Option<MarkerEndpoint> {
    if points.len() < 2 {
        return None;
    }
    let (point, direction) = match position {
        MarkerPosition::Start => {
            let point = Point::new(points[0].x, points[0].y);
            let next = Point::new(points[1].x, points[1].y);
            (point, normalize(next.x - point.x, next.y - point.y))
        }
        MarkerPosition::End => {
            let point = Point::new(points[points.len() - 1].x, points[points.len() - 1].y);
            let previous = Point::new(points[points.len() - 2].x, points[points.len() - 2].y);
            (point, normalize(point.x - previous.x, point.y - previous.y))
        }
    };
    Some(MarkerEndpoint {
        point,
        direction,
        position,
    })
}

fn marker_segments(
    marker: FlowEdgeMarker,
    endpoint: MarkerEndpoint,
    stroke_color: Color,
    base_fill_color: Color,
) -> Option<(Vec<PathSegment>, Option<Paint>, Option<StrokeStyle>)> {
    let marker_point = |x: f64, y: f64, ref_x: f64, ref_y: f64, scale: f64| {
        let normal = Point::new(-endpoint.direction.y, endpoint.direction.x);
        let dx = (x - ref_x) * scale;
        let dy = (y - ref_y) * scale;
        Point::new(
            endpoint.point.x + endpoint.direction.x * dx + normal.x * dy,
            endpoint.point.y + endpoint.direction.y * dx + normal.y * dy,
        )
    };
    match marker {
        FlowEdgeMarker::None => None,
        FlowEdgeMarker::Point => {
            let points = match endpoint.position {
                MarkerPosition::Start => [
                    marker_point(0.0, 5.0, 4.5, 5.0, 0.8),
                    marker_point(10.0, 10.0, 4.5, 5.0, 0.8),
                    marker_point(10.0, 0.0, 4.5, 5.0, 0.8),
                ],
                MarkerPosition::End => [
                    marker_point(0.0, 0.0, 5.0, 5.0, 0.8),
                    marker_point(10.0, 5.0, 5.0, 5.0, 0.8),
                    marker_point(0.0, 10.0, 5.0, 5.0, 0.8),
                ],
            };
            Some((
                polygon_path(&points),
                Some(Paint::solid(stroke_color)),
                Some(marker_stroke(stroke_color, 1.0)),
            ))
        }
        FlowEdgeMarker::Circle => {
            let ref_x = match endpoint.position {
                MarkerPosition::Start => -1.0,
                MarkerPosition::End => 11.0,
            };
            let center = marker_point(5.0, 5.0, ref_x, 5.0, 1.1);
            Some((
                ellipse_path(center.x, center.y, 5.5, 5.5),
                Some(Paint::solid(base_fill_color)),
                Some(marker_stroke(stroke_color, 1.0)),
            ))
        }
        FlowEdgeMarker::Cross => {
            let ref_x = match endpoint.position {
                MarkerPosition::Start => -1.0,
                MarkerPosition::End => 12.0,
            };
            Some((
                vec![
                    PathSegment::MoveTo {
                        to: marker_point(1.0, 1.0, ref_x, 5.2, 1.0),
                    },
                    PathSegment::LineTo {
                        to: marker_point(10.0, 10.0, ref_x, 5.2, 1.0),
                    },
                    PathSegment::MoveTo {
                        to: marker_point(10.0, 1.0, ref_x, 5.2, 1.0),
                    },
                    PathSegment::LineTo {
                        to: marker_point(1.0, 10.0, ref_x, 5.2, 1.0),
                    },
                ],
                None,
                Some(marker_stroke(stroke_color, 2.0)),
            ))
        }
    }
}

fn marker_stroke(color: Color, width: f64) -> StrokeStyle {
    StrokeStyle {
        paint: Paint::solid(color),
        width,
        dash_array: vec![1.0, 0.0],
        dash_offset: 0.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        miter_limit: 4.0,
    }
}

fn normalize(x: f64, y: f64) -> Point {
    let length = x.hypot(y);
    if length < 1e-9 {
        Point::new(0.0, 1.0)
    } else {
        Point::new(x / length, y / length)
    }
}

fn collect_declarations(
    class_defs: &indexmap::IndexMap<String, Vec<String>>,
    classes: &[String],
    styles_a: &[String],
    styles_b: &[String],
) -> Result<Vec<(String, String)>> {
    let mut ordered = Vec::<(String, String)>::new();
    for class in classes {
        if let Some(declarations) = class_defs.get(class) {
            for declaration in declarations {
                collect_declaration_parts(declaration, &mut ordered)?;
            }
        }
    }
    for declaration in styles_a.iter().chain(styles_b) {
        collect_declaration_parts(declaration, &mut ordered)?;
    }
    Ok(ordered)
}

fn collect_declaration_parts(raw: &str, ordered: &mut Vec<(String, String)>) -> Result<()> {
    for part in flowchart_split_mermaid_style_decls(raw).flat_map(|part| part.split(';')) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = crate::mermaid_style::parse_safe_style_decl(part).ok_or_else(|| {
            unavailable(format!("malformed or unsafe style declaration `{part}`"))
        })?;
        let key = key.trim().to_ascii_lowercase();
        if let Some(existing) = ordered.iter_mut().find(|(old, _)| old == &key) {
            existing.1 = value.trim().to_string();
        } else {
            ordered.push((key, value.trim().to_string()));
        }
    }
    Ok(())
}

fn parse_css_color(value: &str, property: &str) -> Result<Option<Color>> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    if value.eq_ignore_ascii_case("transparent") {
        return Ok(Some(Color::rgba(0, 0, 0, 0)));
    }
    let parsed = merman_core::theme_color::ThemeColor::parse(value).map_err(|error| {
        unavailable(format!(
            "{property} `{value}` is not a portable color: {error}"
        ))
    })?;
    let channel = |kind| parsed.channel(kind).round().clamp(0.0, 255.0) as u8;
    Ok(Some(Color::rgba(
        channel(merman_core::theme_color::ColorChannel::Red),
        channel(merman_core::theme_color::ColorChannel::Green),
        channel(merman_core::theme_color::ColorChannel::Blue),
        (parsed.channel(merman_core::theme_color::ColorChannel::Alpha) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
    )))
}

fn parse_css_number(value: &str, property: &str) -> Result<f64> {
    let trimmed = value.trim();
    let (numeric, scale) = if let Some(numeric) = trimmed.strip_suffix("px") {
        (numeric.trim(), 1.0)
    } else if let Some(numeric) = trimmed.strip_suffix("pt") {
        (numeric.trim(), 4.0 / 3.0)
    } else {
        (trimmed, 1.0)
    };
    let parsed = numeric
        .parse::<f64>()
        .map_err(|_| unavailable(format!("{property} `{value}` is not a finite number")))?;
    let parsed = parsed * scale;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(unavailable(format!(
            "{property} `{value}` is outside the portable range"
        )));
    }
    Ok(parsed)
}

fn parse_css_number_allow_normal(value: &str, property: &str) -> Result<f64> {
    if value.eq_ignore_ascii_case("normal") {
        return Ok(0.0);
    }
    parse_css_number(value, property)
}

fn parse_unit(value: &str, property: &str) -> Result<f64> {
    let number = value
        .trim()
        .parse::<f64>()
        .map_err(|_| unavailable(format!("{property} `{value}` is not a unit interval")))?;
    if !number.is_finite() || !(0.0..=1.0).contains(&number) {
        return Err(unavailable(format!(
            "{property} `{value}` is outside [0,1]"
        )));
    }
    Ok(number)
}

fn parse_line_height(value: &str, font_size: f64) -> Result<f64> {
    if value.eq_ignore_ascii_case("normal") {
        return Ok(font_size * 1.5);
    }
    if let Some(percent) = value.trim().strip_suffix('%') {
        let value = percent
            .trim()
            .parse::<f64>()
            .map_err(|_| unavailable(format!("line-height `{value}` is invalid")))?;
        if !value.is_finite() || value < 0.0 {
            return Err(unavailable("line-height must be non-negative"));
        }
        return Ok(font_size * value / 100.0);
    }
    if let Ok(multiplier) = value.trim().parse::<f64>()
        && multiplier.is_finite()
        && multiplier >= 0.0
    {
        return Ok(font_size * multiplier);
    }
    parse_css_number(value, "line-height")
}

fn parse_dash_array(value: &str) -> Result<Vec<f64>> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    value
        .split([',', ' ', '\t'])
        .filter(|part| !part.is_empty())
        .map(|part| parse_css_number(part, "stroke-dasharray"))
        .collect()
}

fn parse_line_cap(value: &str) -> Result<LineCap> {
    match value.trim().to_ascii_lowercase().as_str() {
        "butt" => Ok(LineCap::Butt),
        "round" => Ok(LineCap::Round),
        "square" => Ok(LineCap::Square),
        _ => Err(unavailable(format!(
            "stroke-linecap `{value}` is unsupported"
        ))),
    }
}

fn parse_line_join(value: &str) -> Result<LineJoin> {
    match value.trim().to_ascii_lowercase().as_str() {
        "miter" => Ok(LineJoin::Miter),
        "round" => Ok(LineJoin::Round),
        "bevel" => Ok(LineJoin::Bevel),
        _ => Err(unavailable(format!(
            "stroke-linejoin `{value}` is unsupported"
        ))),
    }
}

fn parse_blend_mode(value: &str) -> Result<BlendMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(BlendMode::Normal),
        "multiply" => Ok(BlendMode::Multiply),
        "screen" => Ok(BlendMode::Screen),
        "overlay" => Ok(BlendMode::Overlay),
        "darken" => Ok(BlendMode::Darken),
        "lighten" => Ok(BlendMode::Lighten),
        "color-dodge" => Ok(BlendMode::ColorDodge),
        "color-burn" => Ok(BlendMode::ColorBurn),
        "hard-light" => Ok(BlendMode::HardLight),
        "soft-light" => Ok(BlendMode::SoftLight),
        "difference" => Ok(BlendMode::Difference),
        "exclusion" => Ok(BlendMode::Exclusion),
        _ => Err(unavailable(format!(
            "mix-blend-mode `{value}` is unsupported"
        ))),
    }
}

fn parse_font_weight(value: &str) -> Result<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(400),
        "bold" => Ok(700),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight))
            .ok_or_else(|| unavailable(format!("font-weight `{value}` is unsupported"))),
    }
}

fn parse_font_style(value: &str) -> Result<FontStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(unavailable(format!("font-style `{value}` is unsupported"))),
    }
}

fn font_descriptor_from_style(
    base: &FontDescriptor,
    style: &RenderTextStyle,
) -> Result<FontDescriptor> {
    let mut font = base.clone();
    if let Some(family) = style.font_family.as_deref() {
        font.families = parse_font_families(family.to_string());
    }
    if let Some(weight) = style.font_weight.as_deref() {
        font.weight = parse_font_weight(weight)?;
    }
    if let Some(font_style) = style.font_style.as_deref() {
        font.style = parse_font_style(font_style)?;
    }
    Ok(font)
}

fn with_alpha(color: Color, opacity: f64) -> Color {
    Color::rgba(
        color.red,
        color.green,
        color.blue,
        (f64::from(color.alpha) * opacity.clamp(0.0, 1.0))
            .round()
            .clamp(0.0, 255.0) as u8,
    )
}

fn profile_identity(identity: &crate::environment::TextMeasurementProfileIdentity) -> String {
    let mut value = format!("{}@{}", identity.profile().as_str(), identity.version());
    for decorator in identity.decorators() {
        value.push('+');
        value.push_str(decorator);
    }
    value
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("Flowchart root bounds are invalid"));
    }
    Ok(())
}

fn first_non_textual_html_tag(label: &str) -> Option<String> {
    let mut remainder = label;
    while let Some(start) = remainder.find('<') {
        remainder = &remainder[start + 1..];
        let end = remainder.find('>')?;
        let raw = remainder[..end].trim();
        remainder = &remainder[end + 1..];
        if raw.is_empty() {
            return Some("<>").map(str::to_string);
        }
        let normalized = raw.trim_end_matches('/').trim();
        let name = normalized
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('/');
        if !matches!(name.to_ascii_lowercase().as_str(), "br" | "/br") {
            return Some(name.to_string());
        }
        if normalized != name && !normalized.eq_ignore_ascii_case("br") {
            return Some(name.to_string());
        }
    }
    None
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Display) -> Error {
    Error::DrawingListUnavailable {
        family: "flowchart".to_string(),
        reason: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> crate::model::LayoutPoint {
        crate::model::LayoutPoint { x, y }
    }

    #[test]
    fn marker_geometry_uses_mermaid_user_space_dimensions() {
        let points = [point(0.0, 0.0), point(20.0, 0.0)];
        let endpoint = marker_point(&points, MarkerPosition::End).unwrap();
        let (segments, _, _) = marker_segments(
            FlowEdgeMarker::Point,
            endpoint,
            Color::rgba(0, 0, 0, 255),
            Color::rgba(0, 0, 0, 255),
        )
        .expect("point marker");
        assert_eq!(
            segments,
            polygon_path(&[
                Point::new(16.0, -4.0),
                Point::new(24.0, 0.0),
                Point::new(16.0, 4.0),
            ])
        );
    }

    #[test]
    fn portable_css_units_and_font_values_fail_or_convert_explicitly() {
        assert_eq!(parse_css_number("3pt", "stroke-width").unwrap(), 4.0);
        assert_eq!(parse_font_weight("600").unwrap(), 600);
        assert!(parse_font_weight("heavy").is_err());
        assert!(parse_font_style("oblique 12deg").is_err());
    }
}
