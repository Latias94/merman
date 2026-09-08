//! Renderer-neutral Flowchart adapter.
//!
//! This module deliberately emits typed paths and text runs from the already-authoritative
//! Flowchart layout.  It does not parse the SVG output back into a second representation: doing
//! so would make the public target depend on DOM spelling and would lose the distinction between
//! source semantics and SVG-only structure.

use super::builder::DrawingListBuilder;
use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families_for, theme_color,
};
use crate::config::{config_bool, config_css_number_or_string, config_diagram_look};
use crate::environment::{RenderSession, TextMeasurementPhase, TextMeasurementSource};
use crate::family::{FlowchartFamilyArtifact, RenderFamilyKind};
use crate::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartConfigView, FlowchartLabelMetricsRequest,
    FlowchartModel, FlowchartRenderContext, FlowchartRenderModelRef, FlowchartShape,
    flowchart_effective_edge_label_text_style, flowchart_effective_node_class_names,
    flowchart_effective_text_style_for_node_classes, flowchart_label_is_empty_for_render,
    flowchart_label_metrics_for_layout, flowchart_label_plain_text_for_layout,
    flowchart_split_mermaid_style_decls,
};
use crate::model::{
    Bounds, FlowchartLayout, LayoutCluster, LayoutLabel, LayoutNode, LayoutPoint,
    SwimlaneDirection, SwimlaneLayout, SwimlaneNodeLayout,
};
use crate::presentation::FlowchartPresentationPolicy;
use crate::render_geometry::{FlowchartCurveKind, emit_flowchart_curve_segments};
use crate::text::{TextMeasurer as _, TextStyle as RenderTextStyle, WrapMode};
use crate::{Error, Result};
use merman_core::diagrams::flowchart::{
    FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNodeProvenance,
};
use merman_core::svg_security::{MermaidNavigationSecurity, prepare_mermaid_navigation_uri};
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    BlendMode, Color, DrawingCommand, DrawingListPolicy, FillRule, FontDescriptor, FontStyle,
    LineCap, LineJoin, MeasurementProvenance, Paint, PathSegment, PathStyle, Point, Rect,
    ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle, TextAnchor, TextBaseline,
    TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::{Value, json};
use std::borrow::Cow;
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
    limits: impl Into<super::DocumentBudget>,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let builder = FlowchartBuilder::new(
        FlowchartBuilderInputs {
            semantic: artifact.pair().semantic(),
            layout: FlowchartLayoutSource::Flowchart(artifact.pair().layout()),
            render_context: artifact.render_context(),
            presentation: artifact.policy(),
            family_kind: RenderFamilyKind::Flowchart,
            swimlane_layout: None,
        },
        metadata,
        policy,
        limits,
        session,
    )?;
    builder.build()
}

pub(crate) fn build_swimlane_document(
    artifact: &FlowchartFamilyArtifact<SwimlaneLayout>,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: impl Into<super::DocumentBudget>,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let builder = FlowchartBuilder::new(
        FlowchartBuilderInputs {
            semantic: artifact.pair().semantic(),
            layout: FlowchartLayoutSource::Swimlane(artifact.pair().layout()),
            render_context: artifact.render_context(),
            presentation: artifact.policy(),
            family_kind: RenderFamilyKind::Swimlane,
            swimlane_layout: Some(artifact.pair().layout()),
        },
        metadata,
        policy,
        limits,
        session,
    )?;
    builder.build()
}

struct FlowchartBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    family_kind: RenderFamilyKind,
    model: FlowchartRenderModelRef<'a>,
    layout: FlowchartLayoutSource<'a>,
    swimlane_layout: Option<&'a SwimlaneLayout>,
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
    output: DrawingListBuilder<'a>,
    label_nodes: HashMap<&'a str, &'a SwimlaneNodeLayout>,
    extensions: BTreeMap<String, Value>,
    interaction_nodes: Vec<Value>,
}

struct FlowchartBuilderInputs<'a> {
    semantic: &'a FlowchartModel,
    layout: FlowchartLayoutSource<'a>,
    render_context: &'a FlowchartRenderContext,
    presentation: Option<FlowchartPresentationPolicy>,
    family_kind: RenderFamilyKind,
    swimlane_layout: Option<&'a SwimlaneLayout>,
}

struct StyleInputs<'a> {
    classes: &'a [String],
    styles_a: &'a [String],
    styles_b: &'a [String],
    base_text_style: &'a RenderTextStyle,
    default_fill: Color,
    default_stroke: Color,
    default_text: Color,
    default_stroke_width: f64,
}

struct NodeLabelBoundsInputs<'a> {
    node: &'a LayoutNode,
    label: &'a str,
    label_type: &'a str,
    style: &'a RenderTextStyle,
    html_labels: bool,
    wrap_mode: WrapMode,
    wrapping_width: f64,
    config: &'a merman_core::MermaidConfig,
    session: &'a RenderSession,
}

impl<'a> FlowchartBuilder<'a> {
    fn new(
        inputs: FlowchartBuilderInputs<'a>,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: impl Into<super::DocumentBudget>,
        session: &'a RenderSession,
    ) -> Result<Self> {
        let FlowchartBuilderInputs {
            semantic,
            layout,
            render_context,
            presentation,
            family_kind,
            swimlane_layout,
        } = inputs;
        session.checkpoint(OperationPhase::Emit)?;
        let mut output = DrawingListBuilder::new(policy, limits, session);
        output.push_control(DrawingCommand::Save)?;
        output.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: format!("{}.document", family_kind.as_str()),
        })?;
        let model = FlowchartRenderModelRef::new(semantic, render_context);
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
        if look.as_str().eq_ignore_ascii_case("neo") {
            reject_unportable_neo_effects(metadata.effective_config.as_value(), family_kind)?;
        }

        let bounds = layout
            .bounds()
            .ok_or_else(|| invalid("Flowchart layout did not provide root bounds"))?;
        validate_bounds(bounds)?;

        let font_family = config.raw_font_family();
        let font_size = config.render_font_size();
        let node_style = config.render_text_style(&font_family, font_size);
        let html_text_style = config.html_label_measurement_base_style(&node_style);
        let font = FontDescriptor {
            families: parse_font_families_for(&font_family, family_kind)?,
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
        let default_curve = semantic
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
        let presentation = presentation.unwrap_or_default();

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
            if let Some(projected) = crate::flowchart::project_flowchart_edge(edge, render_context)
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
            family_kind,
            model,
            layout,
            swimlane_layout,
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
            output,
            label_nodes: swimlane_layout
                .into_iter()
                .flat_map(|layout| &layout.nodes)
                .filter(|node| node.is_edge_label)
                .map(|node| (node.id.as_str(), node))
                .collect(),
            extensions: BTreeMap::new(),
            interaction_nodes: Vec::new(),
        };
        builder.preflight_sources()?;
        Ok(builder)
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.session.checkpoint(OperationPhase::Emit)?;
        self.add_document_semantics()?;

        // Mermaid's stable Flowchart DOM partitions clusters, edge paths/labels, and nodes. The
        // same partition is useful to native consumers because node fills do not erase routes.
        if let Some(swimlane_layout) = self.swimlane_layout {
            self.emit_swimlane_lanes(swimlane_layout)?;
        } else {
            for (index, cluster) in self.layout.clusters().iter().enumerate() {
                self.session.checkpoint(OperationPhase::Emit)?;
                self.emit_cluster(index, cluster)?;
            }
        }
        for edge_layout in self.layout.edges() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge(edge_layout)?;
        }
        for node_layout in self.layout.nodes() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_node(&node_layout)?;
        }

        let mut bounds = self
            .layout
            .bounds()
            .expect("validated in constructor")
            .clone();
        if let Some(title_bounds) = self.emit_document_title(&bounds)? {
            bounds.min_x = bounds.min_x.min(title_bounds.min_x);
            bounds.min_y = bounds.min_y.min(title_bounds.min_y);
            bounds.max_x = bounds.max_x.max(title_bounds.max_x);
            bounds.max_y = bounds.max_y.max(title_bounds.max_y);
        }

        if !self.interaction_nodes.is_empty() {
            self.extensions.insert(
                format!("x-merman-{}-interactions", self.family_prefix()),
                Value::Array(std::mem::take(&mut self.interaction_nodes)),
            );
        }
        self.extensions.insert(
            format!("x-merman-{}", self.family_prefix()),
            json!({
                "diagram_type": self.metadata.diagram_type,
                "uses_elk_adapter_dom": self.layout.uses_elk_adapter_dom(),
                "label_modes": {
                    "node_html": self.config.node_html_labels(),
                    "edge_html": self.config.effective_html_labels(),
                },
            }),
        );

        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_control(DrawingCommand::Restore)?;

        let document = self.output.finish(
            Viewport::new(Rect::new(
                bounds.min_x,
                bounds.min_y,
                bounds.max_x - bounds.min_x,
                bounds.max_y - bounds.min_y,
            )),
            self.extensions,
        )?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: self.family_kind,
                body: if self.family_kind == RenderFamilyKind::Flowchart {
                    SvgStructureBody::Flowchart(FlowchartSvgBody {
                        diagram_type: self.metadata.diagram_type.clone(),
                    })
                } else {
                    SvgStructureBody::Swimlane(super::SwimlaneSvgBody {
                        diagram_type: self.metadata.diagram_type.clone(),
                    })
                },
            },
        })
    }

    /// Emits Mermaid's visible frontmatter title.
    ///
    /// Flowchart accessibility metadata (`accTitle`) is intentionally kept separate from the
    /// visible title. Mermaid's flowchart-v2 renderer paints only the YAML `title` at the root,
    /// centered over the pre-title graph bounds, with an 18px font and the configured
    /// `titleTopMargin` as its baseline offset.
    fn emit_document_title(&mut self, graph_bounds: &Bounds) -> Result<Option<Bounds>> {
        let Some(title) = self
            .metadata
            .title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
        else {
            return Ok(None);
        };

        const TITLE_FONT_SIZE: f64 = 18.0;
        let title_x = (graph_bounds.min_x + graph_bounds.max_x) / 2.0;
        let title_y = -self.config.render_title_top_margin();
        let measurement_style = RenderTextStyle {
            font_family: self.node_style.font_family.clone(),
            font_size: TITLE_FONT_SIZE,
            font_weight: None,
            font_style: None,
        };
        let font = self.font.clone();
        let obligation =
            super::support::text_obligation(self.session, TextMeasurementPhase::SvgBBox);
        let text_color = self.cluster_text;
        let session = self.session;
        let mut title_bounds = None;
        self.output.draw_host_text_parts(&[title], |text| {
            let measurer = session
                .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
            let (left, right) = measurer.measure_svg_title_bbox_x(&text, &measurement_style);
            let (ascent, descent) =
                crate::text::svg_title_bbox_vertical_extents_px(&measurement_style);
            session.checkpoint(OperationPhase::Emit)?;
            if ![left, right, ascent, descent]
                .into_iter()
                .all(f64::is_finite)
                || left < 0.0
                || right < 0.0
                || ascent < 0.0
                || descent < 0.0
            {
                return Err(invalid(
                    "Flowchart title measurement returned invalid bounds",
                ));
            }
            title_bounds = Some(Bounds {
                min_x: title_x - left,
                min_y: title_y - ascent,
                max_x: title_x + right,
                max_y: title_y + descent,
            });
            Ok(TextRun {
                text,
                origin: Point::new(title_x, title_y),
                bounds: Rect::new(
                    title_x - left,
                    title_y - ascent,
                    left + right,
                    ascent + descent,
                ),
                style: TextStyle {
                    font,
                    font_size: TITLE_FONT_SIZE,
                    letter_spacing: 0.0,
                    line_height: TITLE_FONT_SIZE,
                    fill: Paint::solid(text_color),
                    stroke: None,
                    paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                },
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Alphabetic,
                direction: TextDirection::Auto,
                language: None,
                obligation,
            })
        })?;
        Ok(title_bounds)
    }

    fn family_prefix(&self) -> &'static str {
        self.family_kind.as_str()
    }

    fn emit_swimlane_lanes(&mut self, layout: &SwimlaneLayout) -> Result<()> {
        for (index, lane) in layout.lanes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let (subgraph_index, subgraph) = self
                .subgraphs_by_id
                .get(lane.id.as_str())
                .copied()
                .map_or((None, None), |(index, subgraph)| {
                    (Some(index), Some(subgraph))
                });
            let (classes, styles) = subgraph
                .zip(subgraph_index)
                .map(|(subgraph, subgraph_index)| {
                    self.model.effective_subgraph_css(subgraph_index, subgraph)
                })
                .unwrap_or_default();
            let style = self.resolve_style(StyleInputs {
                classes,
                styles_a: &[],
                styles_b: styles,
                base_text_style: self.base_text_style(self.config.effective_html_labels()),
                default_fill: self.cluster_fill,
                default_stroke: self.cluster_border,
                default_text: self.cluster_text,
                default_stroke_width: self.edge_stroke_width,
            })?;
            let semantic_id = format!("{}.group.{index}", self.family_prefix());
            self.begin_group(&semantic_id, style.opacity, style.blend_mode)?;

            let lane_width = lane.width.max(0.0);
            let lane_height = lane.height.max(0.0);
            let lane_left = lane.x - lane_width / 2.0;
            let lane_top = lane.y - lane_height / 2.0;
            let full_bounds = Rect::new(lane_left, lane_top, lane_width, lane_height);
            let (title_bounds, body_bounds) = swimlane_band_bounds(
                lane,
                self.config.render_font_size(),
                matches!(
                    layout.direction,
                    SwimlaneDirection::Lr | SwimlaneDirection::Rl
                ),
            );

            let body_style = PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: style
                    .stroke_color
                    .map(|color| style.stroke_style(FlowEdgeStroke::Normal, color)),
            };
            self.add_path(
                format!("{semantic_id}.body"),
                rectangle_path_from_bounds(body_bounds)?,
                body_style,
            )?;
            self.add_path(
                format!("{semantic_id}.title"),
                rectangle_path_from_bounds(title_bounds)?,
                style.path_style(),
            )?;

            let render_title = subgraph.zip(subgraph_index).map_or_else(
                || lane.title.clone(),
                |(subgraph, subgraph_index)| {
                    self.model
                        .subgraph_title_for_render(subgraph_index, subgraph)
                        .to_string()
                },
            );
            if !flowchart_label_is_empty_for_render(&render_title) {
                let label_type = subgraph
                    .and_then(|subgraph| subgraph.label_type.as_deref())
                    .unwrap_or("text");
                let title_center = Point::new(
                    title_bounds.x + title_bounds.width / 2.0,
                    title_bounds.y + title_bounds.height / 2.0,
                );
                let label_bounds = Rect::new(
                    title_center.x - title_bounds.width / 2.0,
                    title_center.y - title_bounds.height / 2.0,
                    title_bounds.width,
                    title_bounds.height,
                );
                let rotated = matches!(
                    layout.direction,
                    SwimlaneDirection::Lr | SwimlaneDirection::Rl
                );
                if rotated {
                    self.output.push_control(DrawingCommand::Save)?;
                    self.output.push_control(DrawingCommand::ConcatTransform {
                        transform: rotate_about(title_center, -std::f64::consts::FRAC_PI_2),
                    })?;
                }
                self.emit_text(
                    &flowchart_label_plain_text_for_layout(
                        &render_title,
                        label_type,
                        self.config.effective_html_labels(),
                    ),
                    title_center,
                    label_bounds,
                    &style,
                )?;
                if rotated {
                    self.output.push_control(DrawingCommand::Restore)?;
                }
            }
            self.end_group(style.opacity != 1.0 || style.blend_mode != BlendMode::Normal)?;
            self.output.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(flowchart_label_plain_text_for_layout(
                    &render_title,
                    subgraph
                        .and_then(|subgraph| subgraph.label_type.as_deref())
                        .unwrap_or("text"),
                    self.config.effective_html_labels(),
                )),
                description: Some(format!(
                    "{} swimlane ({})",
                    lane.id,
                    layout.direction.as_str()
                )),
                link: None,
            })?;

            if !full_bounds.width.is_finite() || !full_bounds.height.is_finite() {
                return Err(invalid(format!(
                    "swimlane `{}` has invalid bounds",
                    lane.id
                )));
            }
        }
        Ok(())
    }

    fn preflight_sources(&self) -> Result<()> {
        for node in &self.model.nodes {
            self.session.checkpoint(OperationPhase::Emit)?;
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
            self.session.checkpoint(OperationPhase::Emit)?;
            if self.model.is_subgraph_collapsed(&subgraph.id) {
                return Err(unavailable(format!(
                    "collapsed subgraph `{}` needs its indicator/separator geometry before it can be emitted without loss",
                    subgraph.id
                )));
            }
        }

        for edge in &self.model.edges {
            self.session.checkpoint(OperationPhase::Emit)?;
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
            self.session.checkpoint(OperationPhase::Emit)?;
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

        for layout_node in self.layout.nodes() {
            self.session.checkpoint(OperationPhase::Emit)?;
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
            } else if is_self_loop_helper_node_id(layout_node.id.as_str()) {
                // Dagre materializes two zero-sized helper label nodes for every self-loop.
                // They are layout-only implementation details; the merged logical edge below is
                // the canonical visual owner and must be the only public node/edge projection.
                continue;
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
        for layout_edge in self.layout.edges() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if !self.edges_by_id.contains_key(layout_edge.id)
                && !self.projected_edges_by_id.contains_key(layout_edge.id)
            {
                return Err(invalid(format!(
                    "layout edge `{}` has no semantic Flowchart edge",
                    layout_edge.id
                )));
            }
            for points in layout_edge.points.chunks(256) {
                self.session.checkpoint(OperationPhase::Emit)?;
                if points
                    .iter()
                    .any(|point| !point.x.is_finite() || !point.y.is_finite())
                {
                    return Err(invalid(format!(
                        "layout edge `{}` has non-finite points",
                        layout_edge.id
                    )));
                }
            }
            if let Some(label) = self.edge_label_bounds(&layout_edge) {
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

    fn add_document_semantics(&mut self) -> Result<()> {
        let title = self
            .model
            .acc_title
            .clone()
            .or_else(|| self.metadata.title.clone())
            .or_else(|| Some(self.metadata.diagram_type.clone()));
        self.output.push_semantic(SemanticAnnotation {
            id: format!("{}.document", self.family_prefix()),
            role: SemanticRole::Document,
            title,
            description: self.model.acc_descr.clone(),
            link: None,
        })
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
        let style = self.resolve_style(StyleInputs {
            classes,
            styles_a: &[],
            styles_b: styles,
            base_text_style: self.base_text_style(self.config.effective_html_labels()),
            default_fill: self.cluster_fill,
            default_stroke: self.cluster_border,
            default_text: self.cluster_text,
            default_stroke_width: 1.0,
        })?;
        let semantic_id = format!("{}.group.{index}", self.family_prefix());
        self.begin_group(&semantic_id, style.opacity, style.blend_mode)?;
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
        self.emit_text(
            &flowchart_label_plain_text_for_layout(
                &raw_title,
                subgraph.label_type.as_deref().unwrap_or("text"),
                self.config.effective_html_labels(),
            ),
            Point::new(cluster.title_label.x, cluster.title_label.y),
            Rect::new(
                cluster.title_label.x - cluster.title_label.width / 2.0,
                cluster.title_label.y - cluster.title_label.height / 2.0,
                cluster.title_label.width,
                cluster.title_label.height,
            ),
            &style,
        )?;
        self.end_group(style.opacity != 1.0 || style.blend_mode != BlendMode::Normal)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(flowchart_label_plain_text_for_layout(
                &raw_title,
                subgraph.label_type.as_deref().unwrap_or("text"),
                self.config.effective_html_labels(),
            )),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn edge_label_bounds(&self, edge: &DrawingEdge<'_>) -> Option<LayoutLabel> {
        edge.label.cloned().or_else(|| {
            edge.label_node_id
                .and_then(|id| self.label_nodes.get(id))
                .map(|node| LayoutLabel {
                    x: node.x,
                    y: node.y,
                    width: node.label_width,
                    height: node.label_height,
                })
        })
    }

    fn emit_edge(&mut self, layout_edge: DrawingEdge<'_>) -> Result<()> {
        let edge = self
            .edges_by_id
            .get(layout_edge.id)
            .copied()
            .cloned()
            .or_else(|| self.projected_edges_by_id.get(layout_edge.id).cloned())
            .ok_or_else(|| {
                invalid(format!(
                    "layout edge `{}` has no semantic edge",
                    layout_edge.id
                ))
            })?;
        if edge.visibility == FlowEdgeVisibility::Invisible {
            self.output.push_semantic(self.edge_semantic(&edge, None))?;
            return Ok(());
        }
        if layout_edge.points.len() < 2 {
            return Err(unavailable(format!(
                "visible edge `{}` has fewer than two routed points",
                edge.id
            )));
        }
        let style = self.resolve_style(StyleInputs {
            classes: &edge.classes,
            styles_a: &self.default_edge_styles,
            styles_b: &edge.style,
            base_text_style: self.base_text_style(self.config.effective_html_labels()),
            default_fill: self.line_color,
            default_stroke: self.line_color,
            default_text: self.node_text,
            default_stroke_width: self.edge_stroke_width,
        })?;
        let semantic_id = format!("{}.edge.{}", self.family_prefix(), edge.id);
        self.begin_group(&semantic_id, style.opacity, style.blend_mode)?;
        let curve = edge.interpolate.as_deref().unwrap_or(&self.default_curve);
        let curve = curve_kind(curve)?;
        if let Some(stroke_color) = style.stroke_color {
            self.output.draw_path_with(
                ResourceId::new(format!("{semantic_id}.route")),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(style.stroke_style(edge.stroke_kind, stroke_color)),
                },
                |emit| {
                    emit_flowchart_curve_segments(
                        layout_edge.points,
                        curve,
                        self.edge_corner_radius,
                        self.compact_edge_corners,
                        None,
                        emit,
                    )
                },
            )?;
            self.emit_marker(
                &format!("{semantic_id}.start-marker"),
                edge.start_marker,
                layout_edge.points,
                MarkerPosition::Start,
                stroke_color,
                self.line_color,
            )?;
            self.emit_marker(
                &format!("{semantic_id}.end-marker"),
                edge.end_marker,
                layout_edge.points,
                MarkerPosition::End,
                stroke_color,
                self.line_color,
            )?;
        }

        // Mermaid applies edge styles to the route and its markers, while the label is a
        // separate sibling with only label styles. Keep the shared semantic identity without
        // allowing route opacity or blending to hide the label and its background.
        if style.opacity != 1.0 || style.blend_mode != BlendMode::Normal {
            self.output.push_control(DrawingCommand::Restore)?;
        }

        if let Some(raw_label) = self.model.edge_label_for_render(&edge).map(str::to_string)
            && !flowchart_label_is_empty_for_render(&raw_label)
        {
            let label_layout = self.edge_label_bounds(&layout_edge).ok_or_else(|| {
                unavailable(format!(
                    "edge `{}` has a label without layout bounds",
                    edge.id
                ))
            })?;
            let label_style = self.resolve_style(StyleInputs {
                classes: &edge.classes,
                styles_a: &self.default_edge_styles,
                styles_b: &edge.style,
                base_text_style: self.base_text_style(self.config.effective_html_labels()),
                default_fill: self.edge_label_background,
                default_stroke: self.edge_label_background,
                default_text: self.node_text,
                default_stroke_width: self.edge_stroke_width,
            })?;
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
            self.emit_text(
                &flowchart_label_plain_text_for_layout(
                    &raw_label,
                    edge.label_type.as_deref().unwrap_or("text"),
                    self.config.effective_html_labels(),
                ),
                Point::new(label_layout.x, label_layout.y),
                Rect::new(
                    label_layout.x - label_layout.width / 2.0,
                    label_layout.y - label_layout.height / 2.0,
                    label_layout.width,
                    label_layout.height,
                ),
                &label_style,
            )?;
        }
        self.end_group(false)?;
        self.output
            .push_semantic(self.edge_semantic(&edge, Some(semantic_id)))?;
        Ok(())
    }

    fn emit_node(&mut self, layout_node: &LayoutNode) -> Result<()> {
        if layout_node.is_cluster || is_self_loop_helper_node_id(layout_node.id.as_str()) {
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
        let semantic_id = format!("{}.node.{}", self.family_prefix(), node.id);
        let classes = flowchart_effective_node_class_names(&self.model.class_defs, &node.classes)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let style = self.resolve_style(StyleInputs {
            classes: &classes,
            styles_a: &[],
            styles_b: &node.styles,
            base_text_style: self.base_text_style(self.config.node_html_labels()),
            default_fill: self.node_fill,
            default_stroke: self.node_border,
            default_text: self.node_text,
            default_stroke_width: 1.3,
        })?;
        self.begin_group(&semantic_id, style.opacity, style.blend_mode)?;
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
        // Mermaid's styles2String keeps opacity and blending on the shape, not on its
        // sibling HTML/SVG label. Keep both inside the same link/accessibility group.
        if style.opacity != 1.0 || style.blend_mode != BlendMode::Normal {
            self.output.push_control(DrawingCommand::Restore)?;
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
        let node_style = self.node_style_for(node);
        let label_bounds = node_label_bounds(NodeLabelBoundsInputs {
            node: layout_node,
            label: &label_text,
            label_type: node.label_type.as_deref().unwrap_or("text"),
            style: &node_style,
            html_labels: self.config.node_html_labels(),
            wrap_mode: self.config.node_wrap_mode(),
            wrapping_width: self.config.render_wrapping_width(),
            config: &self.metadata.effective_config,
            session: self.session,
        })?;
        if !label_text.is_empty() {
            self.emit_text(
                &label_text,
                Point::new(layout_node.x, layout_node.y),
                label_bounds,
                &style,
            )?;
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
        self.end_group(false)?;
        self.output
            .push_semantic(self.node_semantic(node, semantic_id))?;
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

    fn emit_text(
        &mut self,
        text: &str,
        origin: Point,
        bounds: Rect,
        style: &ResolvedStyle,
    ) -> Result<()> {
        let render_style = &style.text_style;
        let font = font_descriptor_from_style(&self.font, render_style, self.family_kind)?;
        let obligation = self.text_obligation();
        self.output.draw_host_text(text, |text| TextRun {
            text,
            origin,
            bounds,
            style: TextStyle {
                font,
                font_size: render_style.font_size,
                letter_spacing: style.letter_spacing,
                line_height: style.line_height,
                fill: Paint::solid(style.text_color),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor: TextAnchor::Middle,
            baseline: TextBaseline::Middle,
            direction: TextDirection::Auto,
            language: None,
            obligation,
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

    fn begin_group(
        &mut self,
        semantic_id: &str,
        opacity: f64,
        blend_mode: BlendMode,
    ) -> Result<()> {
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.to_string(),
            })?;
        if opacity != 1.0 || blend_mode != BlendMode::Normal {
            self.output.push_control(DrawingCommand::Save)?;
            if opacity != 1.0 {
                self.output
                    .push_control(DrawingCommand::SetOpacity { opacity })?;
            }
            if blend_mode != BlendMode::Normal {
                self.output
                    .push_control(DrawingCommand::SetBlendMode { blend_mode })?;
            }
        }
        Ok(())
    }

    fn end_group(&mut self, had_state: bool) -> Result<()> {
        if had_state {
            self.output.push_control(DrawingCommand::Restore)?;
        }
        self.output.push_control(DrawingCommand::EndSemanticGroup)
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Ok(());
        }
        self.output.draw_path(ResourceId::new(id), segments, style)
    }

    fn emit_marker(
        &mut self,
        id: &str,
        marker: FlowEdgeMarker,
        points: &[crate::model::LayoutPoint],
        position: MarkerPosition,
        stroke_color: Color,
        base_fill_color: Color,
    ) -> Result<()> {
        let Some(endpoint) = marker_point_for(marker, points, position, self.family_prefix(), id)?
        else {
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

    fn resolve_style(&self, inputs: StyleInputs<'_>) -> Result<ResolvedStyle> {
        let StyleInputs {
            classes,
            styles_a,
            styles_b,
            base_text_style,
            default_fill,
            default_stroke,
            default_text,
            default_stroke_width,
        } = inputs;
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
            .resolve_style(StyleInputs {
                classes,
                styles_a,
                styles_b,
                base_text_style: &self.node_style,
                default_fill: self.node_fill,
                default_stroke: self.node_border,
                default_text: self.node_text,
                default_stroke_width: 1.3,
            })
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
            id: id.unwrap_or_else(|| format!("{}.edge.{}", self.family_prefix(), edge.id)),
            role: SemanticRole::Edge,
            title: self.model.edge_label_for_render(edge).map(str::to_string),
            description: Some(format!("{} → {}", edge.from, edge.to)),
            link: None,
        }
    }
}

fn is_self_loop_helper_node_id(id: &str) -> bool {
    let Some((pair, suffix)) = id.rsplit_once("---") else {
        return false;
    };
    if !matches!(suffix, "1" | "2") {
        return false;
    }
    let Some((from, to)) = pair.split_once("---") else {
        return false;
    };
    from == to
}

/// Borrows the authoritative layout; Swimlane conversion never duplicates all routed points.
#[derive(Clone, Copy)]
enum FlowchartLayoutSource<'a> {
    Flowchart(&'a FlowchartLayout),
    Swimlane(&'a SwimlaneLayout),
}

struct DrawingEdge<'a> {
    id: &'a str,
    points: &'a [LayoutPoint],
    label: Option<&'a LayoutLabel>,
    label_node_id: Option<&'a str>,
}

impl<'a> FlowchartLayoutSource<'a> {
    fn flowchart(self) -> Option<&'a FlowchartLayout> {
        match self {
            Self::Flowchart(layout) => Some(layout),
            Self::Swimlane(_) => None,
        }
    }

    fn swimlane(self) -> Option<&'a SwimlaneLayout> {
        match self {
            Self::Swimlane(layout) => Some(layout),
            Self::Flowchart(_) => None,
        }
    }

    fn bounds(self) -> Option<&'a Bounds> {
        match self {
            Self::Flowchart(layout) => layout.bounds.as_ref(),
            Self::Swimlane(layout) => layout.bounds.as_ref(),
        }
    }

    fn clusters(self) -> &'a [LayoutCluster] {
        self.flowchart()
            .map_or(&[], |layout| layout.clusters.as_slice())
    }

    fn uses_elk_adapter_dom(self) -> bool {
        self.flowchart()
            .is_some_and(|layout| layout.uses_elk_adapter_dom)
    }

    fn nodes(self) -> impl Iterator<Item = Cow<'a, LayoutNode>> {
        self.flowchart()
            .into_iter()
            .flat_map(|layout| &layout.nodes)
            .map(Cow::Borrowed)
            .chain(
                self.swimlane()
                    .into_iter()
                    .flat_map(|layout| &layout.nodes)
                    .filter(|node| !node.is_edge_label)
                    .map(|node| {
                        Cow::Owned(LayoutNode {
                            id: node.id.clone(),
                            x: node.x,
                            y: node.y,
                            width: node.width,
                            height: node.height,
                            is_cluster: false,
                            label_width: Some(node.label_width),
                            label_height: Some(node.label_height),
                        })
                    }),
            )
    }

    fn edges(self) -> impl Iterator<Item = DrawingEdge<'a>> {
        self.flowchart()
            .into_iter()
            .flat_map(|layout| &layout.edges)
            .map(|edge| DrawingEdge {
                id: &edge.id,
                points: &edge.points,
                label: edge.label.as_ref(),
                label_node_id: None,
            })
            .chain(
                self.swimlane()
                    .into_iter()
                    .flat_map(|layout| &layout.edges)
                    .map(|edge| DrawingEdge {
                        id: &edge.id,
                        points: &edge.points,
                        label: None,
                        label_node_id: edge.label_node_id.as_deref(),
                    }),
            )
    }
}

fn swimlane_band_bounds(
    lane: &crate::model::SwimlaneLaneLayout,
    font_size: f64,
    horizontal_title: bool,
) -> (Rect, Rect) {
    let lane_left = lane.x - lane.width.max(0.0) / 2.0;
    let lane_top = lane.y - lane.height.max(0.0) / 2.0;
    let lane_right = lane_left + lane.width.max(0.0);
    let lane_bottom = lane_top + lane.height.max(0.0);
    if let Some(title) = &lane.title_rect {
        let title_bounds = Rect::new(
            title.left,
            title.top,
            (title.right - title.left).max(0.0),
            (title.bottom - title.top).max(0.0),
        );
        let body_bounds = if horizontal_title {
            if title.left <= lane_left + lane.width.max(0.0) / 2.0 {
                Rect::new(
                    title.right,
                    lane_top,
                    (lane_right - title.right).max(0.0),
                    (lane_bottom - lane_top).max(0.0),
                )
            } else {
                Rect::new(
                    lane_left,
                    lane_top,
                    (title.left - lane_left).max(0.0),
                    (lane_bottom - lane_top).max(0.0),
                )
            }
        } else {
            Rect::new(
                lane_left,
                title.bottom,
                (lane_right - lane_left).max(0.0),
                (lane_bottom - title.bottom).max(0.0),
            )
        };
        return (title_bounds, body_bounds);
    }

    let title_extent = (lane.title_label_height + 8.0).max(font_size * 1.5);
    if horizontal_title {
        let title_width = title_extent.min((lane_right - lane_left).max(0.0));
        (
            Rect::new(lane_left, lane_top, title_width, lane_bottom - lane_top),
            Rect::new(
                lane_left + title_width,
                lane_top,
                (lane_right - lane_left - title_width).max(0.0),
                lane_bottom - lane_top,
            ),
        )
    } else {
        let title_height = title_extent.min((lane_bottom - lane_top).max(0.0));
        (
            Rect::new(lane_left, lane_top, lane_right - lane_left, title_height),
            Rect::new(
                lane_left,
                lane_top + title_height,
                lane_right - lane_left,
                (lane_bottom - lane_top - title_height).max(0.0),
            ),
        )
    }
}

fn rectangle_path_from_bounds(bounds: Rect) -> Result<Vec<PathSegment>> {
    rectangle_path(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
        bounds.width,
        bounds.height,
        0.0,
    )
    .ok_or_else(|| invalid("swimlane band bounds are invalid"))
}

fn rotate_about(center: Point, angle: f64) -> Transform {
    let cosine = angle.cos();
    let sine = angle.sin();
    Transform {
        a: cosine,
        b: sine,
        c: -sine,
        d: cosine,
        e: center.x - cosine * center.x + sine * center.y,
        f: center.y - sine * center.x - cosine * center.y,
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
    let radius = if shape == FlowchartShape::RoundedRectangle
        || shape == FlowchartShape::Process && look.eq_ignore_ascii_case("neo")
    {
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

pub(crate) fn rectangle_path(
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

pub(crate) fn rounded_rect_path(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) -> Vec<PathSegment> {
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

pub(crate) fn polygon_path(points: &[Point]) -> Vec<PathSegment> {
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

pub(crate) fn ellipse_path(x: f64, y: f64, radius_x: f64, radius_y: f64) -> Vec<PathSegment> {
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
    let half_w = w / 2.0;
    let points = if left {
        [
            Point::new(x - half_w, y + height / 2.0),
            Point::new(x + half_w + dx, y + height / 2.0),
            Point::new(x + half_w, y - height / 2.0),
            Point::new(x - half_w - dx, y - height / 2.0),
        ]
    } else {
        [
            Point::new(x - half_w - dx, y + height / 2.0),
            Point::new(x + half_w, y + height / 2.0),
            Point::new(x + half_w + dx, y - height / 2.0),
            Point::new(x - half_w, y - height / 2.0),
        ]
    };
    polygon_path(&points)
}

fn node_label_bounds(inputs: NodeLabelBoundsInputs<'_>) -> Result<Rect> {
    let NodeLabelBoundsInputs {
        node,
        label,
        label_type,
        style,
        html_labels,
        wrap_mode,
        wrapping_width,
        config,
        session,
    } = inputs;
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

fn curve_kind(curve: &str) -> Result<FlowchartCurveKind> {
    FlowchartCurveKind::from_mermaid_name(curve).ok_or_else(|| {
        unavailable(format!(
            "edge curve `{}` has no typed v1 adapter",
            curve.trim()
        ))
    })
}

fn ensure_curve_supported(curve: &str) -> Result<()> {
    match curve.trim().to_ascii_lowercase().as_str() {
        "linear" | "step" | "stepbefore" | "stepafter" | "basis" | "rounded" => Ok(()),
        other => Err(unavailable(format!(
            "edge curve `{other}` has no typed v1 adapter"
        ))),
    }
}

fn reject_unportable_neo_effects(config: &Value, family: RenderFamilyKind) -> Result<()> {
    if config_bool(config, &["themeVariables", "useGradient"]).unwrap_or(false) {
        return Err(Error::DrawingListUnavailable {
            family: family.as_str().to_string(),
            reason: "neo Flowchart output uses a CSS gradient that DrawingList v1 cannot preserve"
                .to_string(),
        });
    }

    let drop_shadow = config_css_number_or_string(config, &["themeVariables", "dropShadow"])
        .unwrap_or_else(|| "none".to_string());
    let normalized = drop_shadow.trim().trim_end_matches(';').trim();
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("none") {
        return Ok(());
    }

    Err(Error::DrawingListUnavailable {
        family: family.as_str().to_string(),
        reason: format!(
            "neo {} output uses a CSS drop-shadow filter that DrawingList v1 cannot preserve",
            family.as_str()
        ),
    })
}

#[derive(Debug, Clone, Copy)]
struct MarkerEndpoint {
    point: Point,
    direction: Point,
    position: MarkerPosition,
}

#[derive(Debug, Clone, Copy)]
enum MarkerPosition {
    Start,
    End,
}

fn marker_point(
    points: &[crate::model::LayoutPoint],
    position: MarkerPosition,
) -> Option<MarkerEndpoint> {
    let (point, direction) = match position {
        MarkerPosition::Start => {
            let start = points.first()?;
            let point = Point::new(start.x, start.y);
            let direction = points.iter().skip(1).find_map(|candidate| {
                unit_direction(candidate.x - point.x, candidate.y - point.y)
            })?;
            (point, direction)
        }
        MarkerPosition::End => {
            let end = points.last()?;
            let point = Point::new(end.x, end.y);
            let direction = points.iter().rev().skip(1).find_map(|candidate| {
                unit_direction(point.x - candidate.x, point.y - candidate.y)
            })?;
            (point, direction)
        }
    };
    Some(MarkerEndpoint {
        point,
        direction,
        position,
    })
}

fn marker_point_for(
    marker: FlowEdgeMarker,
    points: &[crate::model::LayoutPoint],
    position: MarkerPosition,
    family: &str,
    marker_id: &str,
) -> Result<Option<MarkerEndpoint>> {
    if marker == FlowEdgeMarker::None {
        return Ok(None);
    }
    marker_point(points, position).map(Some).ok_or_else(|| Error::DrawingListUnavailable {
        family: family.to_string(),
        reason: format!(
            "{marker_id} has no non-zero tangent; DrawingList v1 cannot choose a lossless marker orientation"
        ),
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

fn unit_direction(x: f64, y: f64) -> Option<Point> {
    let length = x.hypot(y);
    if !length.is_finite() || length < 1e-9 {
        None
    } else {
        Some(Point::new(x / length, y / length))
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
    family_kind: RenderFamilyKind,
) -> Result<FontDescriptor> {
    let mut font = base.clone();
    if let Some(font_family) = style.font_family.as_deref() {
        font.families = parse_font_families_for(font_family, family_kind)?;
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
            return Some(str::to_string("<>"));
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
    use serde_json::json;

    #[test]
    fn lean_paths_keep_the_full_mermaid_skew_width() {
        // Mermaid defines the polygon in label coordinates, then translates it by (-w/2, h/2).
        // For total width 120 and height 40, w=80 and dx=20; the outer side reaches +/-60.
        assert_eq!(
            lean_path(10.0, 20.0, 120.0, 40.0, true),
            polygon_path(&[
                Point::new(-30.0, 40.0),
                Point::new(70.0, 40.0),
                Point::new(50.0, 0.0),
                Point::new(-50.0, 0.0),
            ])
        );
        assert_eq!(
            lean_path(10.0, 20.0, 120.0, 40.0, false),
            polygon_path(&[
                Point::new(-50.0, 40.0),
                Point::new(50.0, 40.0),
                Point::new(70.0, 0.0),
                Point::new(-30.0, 0.0),
            ])
        );
    }

    #[test]
    fn cancellation_during_a_flowchart_route_does_not_commit_the_path() {
        use merman_core::{Engine, OperationControl, ParseOptions, RenderSemanticModel};

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA --> B --> C\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let (metadata, model, context) = parsed.into_render_parts();
        let RenderSemanticModel::Flowchart(semantic) = model else {
            panic!("expected Flowchart");
        };
        let context = context.into_flowchart_render_context();
        let control = OperationControl::new();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_control(control.clone())
            .unwrap();
        let options = crate::LayoutOptions::default();
        let execution = crate::LayoutExecution::new(&options, &session);
        let layout = crate::layout_flowchart_typed_with_render_labels_by_engine(
            &metadata.diagram_type,
            &semantic,
            &context,
            &metadata.effective_config,
            &execution,
            None,
        )
        .unwrap();
        let layout_source = FlowchartLayoutSource::Flowchart(&layout);
        let mut builder = FlowchartBuilder::new(
            FlowchartBuilderInputs {
                semantic: &semantic,
                layout: layout_source,
                render_context: &context,
                presentation: None,
                family_kind: RenderFamilyKind::Flowchart,
                swimlane_layout: None,
            },
            &metadata,
            DrawingListPolicy::VectorOnly,
            merman_display_list::DrawingListLimits::default(),
            &session,
        )
        .unwrap();
        let mut edges = layout_source.edges();
        builder.emit_edge(edges.next().unwrap()).unwrap();
        let committed = builder.output.command_count();

        // Admit the group, path header and first segment, then cancel inside the route sink.
        control.cancel_after_checkpoints(3);
        let error = builder.emit_edge(edges.next().unwrap()).unwrap_err();
        assert!(
            matches!(error, Error::Cancelled(ref cancelled) if cancelled.phase == OperationPhase::Emit)
        );
        assert_eq!(builder.output.command_count(), committed + 1);
        assert!(matches!(builder.build(), Err(Error::Cancelled(_))));
    }

    #[test]
    fn edge_opacity_does_not_leak_into_its_label_or_background() {
        use merman_core::{Engine, ParseOptions, RenderSemanticModel};

        for opacity in [0.0, 0.4, 1.0] {
            let source = format!("flowchart TD\nA -->|Visible| B\nlinkStyle 0 opacity:{opacity}\n");
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let (metadata, model, context) = parsed.into_render_parts();
            let RenderSemanticModel::Flowchart(semantic) = model else {
                panic!("expected Flowchart");
            };
            let context = context.into_flowchart_render_context();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let options = crate::LayoutOptions::default();
            let execution = crate::LayoutExecution::new(&options, &session);
            let layout = crate::layout_flowchart_typed_with_render_labels_by_engine(
                &metadata.diagram_type,
                &semantic,
                &context,
                &metadata.effective_config,
                &execution,
                None,
            )
            .unwrap();
            let document = FlowchartBuilder::new(
                FlowchartBuilderInputs {
                    semantic: &semantic,
                    layout: FlowchartLayoutSource::Flowchart(&layout),
                    render_context: &context,
                    presentation: None,
                    family_kind: RenderFamilyKind::Flowchart,
                    swimlane_layout: None,
                },
                &metadata,
                DrawingListPolicy::VectorOnly,
                merman_display_list::DrawingListLimits::default(),
                &session,
            )
            .unwrap()
            .build()
            .unwrap()
            .into_public();

            // Replay public graphics state rather than just looking for a Restore command:
            // the path and marker must keep edge opacity, but the label must remain visible.
            let mut current_opacity = 1.0;
            let mut saved_opacities = Vec::new();
            let mut route_count = 0;
            let mut marker_count = 0;
            let mut background_count = 0;
            let mut label_count = 0;
            for command in &document.commands {
                match command {
                    DrawingCommand::Save => saved_opacities.push(current_opacity),
                    DrawingCommand::Restore => {
                        current_opacity = saved_opacities.pop().expect("balanced save");
                    }
                    DrawingCommand::SetOpacity { opacity } => current_opacity = *opacity,
                    DrawingCommand::DrawPath { path, .. } if path.as_str().ends_with(".route") => {
                        assert_eq!(current_opacity, opacity);
                        route_count += 1;
                    }
                    DrawingCommand::DrawPath { path, .. }
                        if path.as_str().ends_with(".end-marker") =>
                    {
                        assert_eq!(current_opacity, opacity);
                        marker_count += 1;
                    }
                    DrawingCommand::DrawPath { path, .. }
                        if path.as_str().ends_with(".label-background") =>
                    {
                        assert_eq!(current_opacity, 1.0);
                        background_count += 1;
                    }
                    DrawingCommand::DrawText { run } if run.text == "Visible" => {
                        assert_eq!(current_opacity, 1.0);
                        label_count += 1;
                    }
                    _ => {}
                }
            }
            assert!(saved_opacities.is_empty());
            assert_eq!(
                (route_count, marker_count, background_count, label_count),
                (1, 1, 1, 1)
            );
        }
    }

    #[test]
    fn node_opacity_and_blending_do_not_leak_into_labels_or_split_semantics() {
        use merman_core::{Engine, ParseOptions, RenderSemanticModel};

        for (html_labels, opacity, blend_mode) in [false, true].into_iter().flat_map(|html| {
            [
                (0.0, BlendMode::Normal),
                (0.4, BlendMode::Multiply),
                (1.0, BlendMode::Multiply),
                (1.0, BlendMode::Normal),
            ]
            .map(|(opacity, blend_mode)| (html, opacity, blend_mode))
        }) {
            let blend = if blend_mode == BlendMode::Normal {
                "normal"
            } else {
                "multiply"
            };
            let source = format!(
                "%%{{init: {{\"htmlLabels\": {html_labels}, \"flowchart\": {{\"htmlLabels\": {html_labels}}}}}}}%%\n\
                 flowchart TD\nA[Visible]\n\
                 style A opacity:{opacity},mix-blend-mode:{blend}\n\
                 click A href \"https://example.com/\" \"Node tooltip\"\n"
            );
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let (metadata, model, context) = parsed.into_render_parts();
            let RenderSemanticModel::Flowchart(semantic) = model else {
                panic!("expected Flowchart");
            };
            let context = context.into_flowchart_render_context();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let options = crate::LayoutOptions::default();
            let execution = crate::LayoutExecution::new(&options, &session);
            let layout = crate::layout_flowchart_typed_with_render_labels_by_engine(
                &metadata.diagram_type,
                &semantic,
                &context,
                &metadata.effective_config,
                &execution,
                None,
            )
            .unwrap();
            let document = FlowchartBuilder::new(
                FlowchartBuilderInputs {
                    semantic: &semantic,
                    layout: FlowchartLayoutSource::Flowchart(&layout),
                    render_context: &context,
                    presentation: None,
                    family_kind: RenderFamilyKind::Flowchart,
                    swimlane_layout: None,
                },
                &metadata,
                DrawingListPolicy::VectorOnly,
                merman_display_list::DrawingListLimits::default(),
                &session,
            )
            .unwrap()
            .build()
            .unwrap()
            .into_public();

            let mut state = (1.0, BlendMode::Normal);
            let mut saved_states = Vec::new();
            let mut groups = Vec::new();
            let mut node_groups = 0;
            let mut shape_count = 0;
            let mut label_count = 0;
            for command in &document.commands {
                match command {
                    DrawingCommand::BeginSemanticGroup { semantic_id } => {
                        if semantic_id.starts_with("flowchart.node.") {
                            node_groups += 1;
                        }
                        groups.push(semantic_id.as_str());
                    }
                    DrawingCommand::EndSemanticGroup => {
                        groups.pop().expect("balanced semantic group");
                    }
                    DrawingCommand::Save => saved_states.push(state),
                    DrawingCommand::Restore => {
                        state = saved_states.pop().expect("balanced save");
                    }
                    DrawingCommand::SetOpacity { opacity } => state.0 = *opacity,
                    DrawingCommand::SetBlendMode { blend_mode } => state.1 = *blend_mode,
                    DrawingCommand::DrawPath { path, .. }
                        if path.as_str().starts_with("flowchart.node.A.shape.") =>
                    {
                        assert_eq!(state, (opacity, blend_mode));
                        assert_eq!(groups.last().copied(), Some("flowchart.node.A"));
                        shape_count += 1;
                    }
                    DrawingCommand::DrawText { run } => {
                        assert_eq!(state, (1.0, BlendMode::Normal));
                        assert_eq!(run.text, "Visible");
                        assert_eq!(groups.last().copied(), Some("flowchart.node.A"));
                        label_count += 1;
                    }
                    _ => {}
                }
            }
            assert!(saved_states.is_empty());
            assert!(groups.is_empty());
            assert_eq!((node_groups, shape_count, label_count), (1, 1, 1));
            let node = document
                .semantics
                .iter()
                .find(|annotation| annotation.id == "flowchart.node.A")
                .expect("node semantics");
            assert_eq!(node.role, SemanticRole::Node);
            assert_eq!(node.title.as_deref(), Some("Visible"));
            assert_eq!(node.description.as_deref(), Some("Node tooltip"));
            assert_eq!(node.link.as_deref(), Some("https://example.com/"));
        }
    }

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
    fn marker_points_skip_coincident_endpoint_points() {
        let start_points = [point(1.0, 2.0), point(1.0, 2.0), point(4.0, 6.0)];
        let end_points = [point(1.0, 2.0), point(4.0, 6.0), point(4.0, 6.0)];

        assert_eq!(
            marker_point(&start_points, MarkerPosition::Start)
                .expect("start marker tangent")
                .direction,
            Point::new(0.6, 0.8)
        );
        assert_eq!(
            marker_point(&end_points, MarkerPosition::End)
                .expect("end marker tangent")
                .direction,
            Point::new(0.6, 0.8)
        );
    }

    #[test]
    fn marker_points_reject_a_fully_degenerate_route() {
        let points = [point(1.0, 2.0), point(1.0, 2.0), point(1.0, 2.0)];

        assert!(
            marker_point_for(
                FlowEdgeMarker::None,
                &points,
                MarkerPosition::End,
                "flowchart",
                "flowchart.edge.test.marker",
            )
            .expect("a marker-free edge does not need a tangent")
            .is_none()
        );
        for position in [MarkerPosition::Start, MarkerPosition::End] {
            let error = marker_point_for(
                FlowEdgeMarker::Point,
                &points,
                position,
                "swimlane",
                "swimlane.edge.test.marker",
            )
            .expect_err("a fully degenerate Flowchart-family marker must fail closed");
            assert!(matches!(
                error,
                Error::DrawingListUnavailable { ref family, .. } if family == "swimlane"
            ));
            assert!(error.to_string().contains("no non-zero tangent"));
        }
    }

    #[test]
    fn portable_css_units_and_font_values_fail_or_convert_explicitly() {
        assert_eq!(parse_css_number("3pt", "stroke-width").unwrap(), 4.0);
        assert_eq!(parse_font_weight("600").unwrap(), 600);
        assert!(parse_font_weight("heavy").is_err());
        assert!(parse_font_style("oblique 12deg").is_err());
    }

    #[test]
    fn neo_gradient_and_drop_shadow_fail_closed_for_the_actual_family() {
        let gradient_error = reject_unportable_neo_effects(
            &json!({
                "themeVariables": {
                    "useGradient": true,
                    "dropShadow": "none"
                }
            }),
            RenderFamilyKind::Flowchart,
        )
        .expect_err("a neo gradient needs an explicit fallback");
        assert!(matches!(
            gradient_error,
            Error::DrawingListUnavailable { ref family, ref reason }
                if family == "flowchart" && reason.contains("gradient")
        ));

        let shadow_error = reject_unportable_neo_effects(
            &json!({
                "themeVariables": {
                    "useGradient": false,
                    "dropShadow": "drop-shadow(1px 2px 2px #000)"
                }
            }),
            RenderFamilyKind::Swimlane,
        )
        .expect_err("a neo shadow needs an explicit fallback");
        assert!(matches!(
            shadow_error,
            Error::DrawingListUnavailable { ref family, ref reason }
                if family == "swimlane" && reason.contains("drop-shadow")
        ));

        reject_unportable_neo_effects(
            &json!({
                "themeVariables": {
                    "useGradient": false,
                    "dropShadow": "none;"
                }
            }),
            RenderFamilyKind::Flowchart,
        )
        .expect("an explicitly disabled neo effect is renderer-neutral");
    }
}
