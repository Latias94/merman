//! Renderer-neutral Block diagram adapter.
//!
//! Block layout already owns the final grid positions and source-backed shape boundaries.  This
//! adapter keeps that geometry typed, expands edge markers into ordinary paths, and accepts only
//! labels and styles that the v1 contract can represent without dropping visible information.

use super::{
    BlockSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    theme_color,
};
use crate::block::{BlockRectangleKind, BlockShapeBoundary, BlockShapeGeometry};
use crate::config::{
    config_diagram_look, config_font_family_or_first_array_css, config_theme_or_root_font_size_px,
};
use crate::drawing_list::flowchart::{ellipse_path, polygon_path, rounded_rect_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{BlockDiagramLayout, Bounds, LayoutEdge, LayoutNode, LayoutPoint};
use crate::render_geometry::{FlowchartCurveKind, flowchart_curve_segments};
use crate::text::{
    mermaid_markdown_to_xhtml_label_fragment, mermaid_xhtml_label_plain_text, split_html_br_lines,
};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::block::{
    BlockDiagramRenderModel, BlockEdgeRenderModel, BlockNodeRenderModel,
};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, LineCap, LineJoin,
    Paint, PathResource, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, StrokeStyle, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun,
    TextStyle as DisplayTextStyle, Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

type BlockPair = FamilyPair<BlockDiagramRenderModel, BlockDiagramLayout>;

const DEFAULT_BLOCK_FONT_SIZE: f64 = 16.0;
const DEFAULT_BLOCK_PADDING: f64 = 5.0;
const DEFAULT_NODE_FILL: &str = "#ECECFF";
const DEFAULT_NODE_BORDER: &str = "#9370DB";
const DEFAULT_NODE_TEXT: &str = "#333333";
const DEFAULT_LINE_COLOR: &str = "#333333";
const DEFAULT_EDGE_LABEL_BACKGROUND: &str = "rgba(232,232,232, 0.8)";

#[derive(Debug, Clone)]
struct BlockSource {
    label: String,
    block_type: String,
    classes: Vec<String>,
    styles: Vec<String>,
    directions: Vec<String>,
}

#[derive(Debug, Clone)]
struct BlockStyle {
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_width: f64,
    dash_array: Vec<f64>,
    dash_offset: f64,
    line_cap: LineCap,
    line_join: LineJoin,
    miter_limit: f64,
    fill_rule: FillRule,
    opacity: f64,
    fill_opacity: f64,
    stroke_opacity: f64,
    text: Color,
    font: FontDescriptor,
    font_family_css: String,
    font_size: f64,
    line_height: f64,
    text_anchor: TextAnchor,
    visible: bool,
}

#[derive(Debug, Clone, Copy)]
enum StyleTarget {
    Box,
    Text,
}

struct BlockBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a BlockDiagramRenderModel,
    layout: &'a BlockDiagramLayout,
    sources: HashMap<String, BlockSource>,
    geometry_by_id: HashMap<String, &'a BlockShapeGeometry>,
    edge_by_id: HashMap<String, &'a LayoutEdge>,
    base_font: FontDescriptor,
    base_font_family_css: String,
    base_font_size: f64,
    node_fill: Color,
    node_border: Color,
    node_text: Color,
    cluster_fill: Color,
    cluster_border: Color,
    line_color: Color,
    arrow_color: Color,
    edge_label_background: Color,
    stroke_width: f64,
    text_obligation: TextObligation,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
    label_max_widths: BTreeMap<String, f64>,
    label_inline_styles: BTreeMap<String, Vec<String>>,
    label_data_ids: BTreeMap<String, String>,
    path_inline_styles: BTreeMap<String, Vec<String>>,
}

pub(crate) fn build_block_document(
    pair: &BlockPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    BlockBuilder::new(pair, metadata, policy, session)?.build()
}

impl<'a> BlockBuilder<'a> {
    fn new(
        pair: &'a BlockPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let look = config_diagram_look(config);
        match look.as_str() {
            value if value.eq_ignore_ascii_case("classic") => {}
            value if value.eq_ignore_ascii_case("handDrawn") => {
                return Err(unavailable(
                    "Block hand-drawn output uses SVG/RoughJS effects that DrawingList v1 cannot preserve",
                ));
            }
            value if value.eq_ignore_ascii_case("neo") => {
                return Err(unavailable(
                    "Block neo output uses SVG drop-shadow filters that DrawingList v1 cannot preserve",
                ));
            }
            other => {
                return Err(unavailable(format!(
                    "Block look `{other}` has no lossless DrawingList v1 mapping"
                )));
            }
        }
        if config
            .get("themeCSS")
            .and_then(Value::as_str)
            .is_some_and(|css| !css.trim().is_empty())
        {
            return Err(unavailable(
                "themeCSS is an unresolved SVG cascade input for Block DrawingList output",
            ));
        }

        let model = pair.semantic();
        let layout = pair.layout();
        let sources = collect_sources(model);
        let geometry_by_id = unique_geometries(layout)?;
        let node_by_id = unique_nodes(layout)?;
        let edge_by_id = unique_edges(layout)?;
        validate_model(
            model,
            layout,
            &sources,
            &geometry_by_id,
            &node_by_id,
            &edge_by_id,
        )?;

        let base_font_family_css = config_font_family_or_first_array_css(config);
        let base_font_size =
            config_theme_or_root_font_size_px(config, DEFAULT_BLOCK_FONT_SIZE).max(1.0);
        let base_font = FontDescriptor {
            families: parse_font_families(base_font_family_css.clone()),
            weight: theme_font_weight(config)?,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let resolver = PortableStyleResolver::new("block");
        let stroke_width = theme_length(config, "strokeWidth", "1", &resolver)?;
        let node_fill = theme_color(config, "mainBkg", DEFAULT_NODE_FILL)?;
        let node_border = theme_color(config, "nodeBorder", DEFAULT_NODE_BORDER)?;
        let node_text =
            config_theme_color(config, "nodeTextColor", "textColor", DEFAULT_NODE_TEXT)?;
        let cluster_fill = with_alpha(theme_color(config, "clusterBkg", "#ffffde")?, 0.5);
        let cluster_border = with_alpha(theme_color(config, "clusterBorder", "#aaaa33")?, 0.2);
        let line_color = theme_color(config, "lineColor", DEFAULT_LINE_COLOR)?;
        let arrow_color =
            config_theme_color(config, "arrowheadColor", "lineColor", DEFAULT_LINE_COLOR)?;
        let edge_label_background =
            theme_color(config, "edgeLabelBackground", DEFAULT_EDGE_LABEL_BACKGROUND)?;

        Ok(Self {
            metadata,
            session,
            policy,
            model,
            layout,
            sources,
            geometry_by_id,
            edge_by_id,
            base_font,
            base_font_family_css,
            base_font_size,
            node_fill,
            node_border,
            node_text,
            cluster_fill,
            cluster_border,
            line_color,
            arrow_color,
            edge_label_background,
            stroke_width,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "block.document".to_string(),
                },
            ],
            semantics: Vec::new(),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
            label_max_widths: BTreeMap::new(),
            label_inline_styles: BTreeMap::new(),
            label_data_ids: BTreeMap::new(),
            path_inline_styles: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.session.checkpoint(OperationPhase::Emit)?;
        self.semantics.push(SemanticAnnotation {
            id: "block.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .metadata
                .title
                .clone()
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: None,
            link: None,
        });
        self.semantic_classes
            .insert("block.document".to_string(), "block".to_string());

        for (index, node) in self.layout.nodes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_node(index, node)?;
        }
        for (index, edge) in self.model.edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge(index, edge)?;
        }
        for (index, edge) in self.model.edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge_label(index, edge)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let viewport = self.viewport()?;
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(viewport),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-block".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "look": config_diagram_look(self.metadata.effective_config.as_value()).as_str(),
                    "text_mode": "plain_host_text",
                    "markup": "plain_only_with_br",
                    "markers": "typed_paths",
                    "shape_geometry": "source_backed_boundaries",
                    "style_mode": "resolved_common_css",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Block,
                body: SvgStructureBody::Block(BlockSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    class_defs: self.model.class_defs.clone(),
                    semantic_classes: std::mem::take(&mut self.semantic_classes),
                    path_classes: std::mem::take(&mut self.path_classes),
                    text_classes: std::mem::take(&mut self.text_classes),
                    dom_ids: std::mem::take(&mut self.dom_ids),
                    label_max_widths: std::mem::take(&mut self.label_max_widths),
                    label_inline_styles: std::mem::take(&mut self.label_inline_styles),
                    label_data_ids: std::mem::take(&mut self.label_data_ids),
                    path_inline_styles: std::mem::take(&mut self.path_inline_styles),
                }),
            },
        })
    }

    fn emit_node(&mut self, index: usize, node: &LayoutNode) -> Result<()> {
        let source = self.sources.get(&node.id).cloned().ok_or_else(|| {
            invalid(format!(
                "Block layout node `{}` has no source model",
                node.id
            ))
        })?;
        let geometry = self.geometry_by_id.get(&node.id).cloned().ok_or_else(|| {
            invalid(format!(
                "Block layout node `{}` has no shape geometry",
                node.id
            ))
        })?;
        let style = self.node_style(&source, &geometry)?;
        let semantic_id = format!("block.node.{index}");
        let node_class = if source.classes.is_empty() {
            "node default flowchart-label".to_string()
        } else {
            format!("node {} flowchart-label", source.classes.join(" "))
        };
        self.semantic_classes
            .insert(semantic_id.clone(), node_class);
        self.dom_ids.insert(semantic_id.clone(), node.id.clone());
        self.text_classes
            .insert(semantic_id.clone(), "nodeLabel".to_string());
        self.label_max_widths
            .insert(semantic_id.clone(), geometry.allocated.width.max(0.0));
        if !source.styles.is_empty() {
            self.label_inline_styles
                .insert(semantic_id.clone(), source.styles.clone());
        }
        if !style.visible {
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(source.label.clone()),
                description: Some(format!("{} ({})", node.id, source.block_type)),
                link: None,
            });
            return Ok(());
        }

        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.emit_shape(&format!("{semantic_id}.shape"), node, &geometry, &style)?;
        let lines = plain_lines(&source.label, &format!("Block node `{}` label", node.id))?;
        self.emit_label_lines(
            node,
            &geometry,
            &lines,
            &style,
            node.label_width.unwrap_or_default().max(0.0),
            node.label_height.unwrap_or_default().max(0.0),
        )?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(if lines.iter().all(|line| line.trim().is_empty()) {
                node.id.clone()
            } else {
                lines.join(" ")
            }),
            description: Some(format!("{} ({})", node.id, source.block_type)),
            link: None,
        });
        Ok(())
    }

    fn emit_shape(
        &mut self,
        id: &str,
        node: &LayoutNode,
        geometry: &BlockShapeGeometry,
        style: &BlockStyle,
    ) -> Result<()> {
        let inline_styles = self
            .sources
            .get(&node.id)
            .map(|source| source.styles.clone())
            .unwrap_or_default();
        let fill = style
            .fill
            .map(|color| with_alpha(color, style.opacity * style.fill_opacity));
        let stroke_color = style
            .stroke
            .map(|color| with_alpha(color, style.opacity * style.stroke_opacity));
        let path_style = |fill: Option<Color>, stroke_color: Option<Color>| PathStyle {
            fill_rule: style.fill_rule,
            fill: fill.map(Paint::solid),
            stroke: stroke_color.map(|color| StrokeStyle {
                paint: Paint::solid(color),
                width: style.stroke_width,
                dash_array: style.dash_array.clone(),
                dash_offset: style.dash_offset,
                line_cap: style.line_cap,
                line_join: style.line_join,
                miter_limit: style.miter_limit,
            }),
        };

        match &geometry.boundary {
            BlockShapeBoundary::DoubleCircle {
                outer_radius,
                inner_radius,
                ..
            } => {
                let outer_id = format!("{id}.outer");
                let inner_id = format!("{id}.inner");
                self.path_classes
                    .insert(outer_id.clone(), "outer-circle".to_string());
                self.path_classes
                    .insert(inner_id.clone(), "inner-circle".to_string());
                if !inline_styles.is_empty() {
                    self.path_inline_styles
                        .insert(outer_id.clone(), inline_styles.clone());
                    self.path_inline_styles
                        .insert(inner_id.clone(), inline_styles.clone());
                }
                self.add_path(
                    outer_id,
                    ellipse_path(node.x, node.y, *outer_radius, *outer_radius),
                    path_style(fill, stroke_color),
                )?;
                self.add_path(
                    inner_id,
                    ellipse_path(node.x, node.y, *inner_radius, *inner_radius),
                    path_style(fill, stroke_color),
                )?;
            }
            boundary => {
                let class = match boundary {
                    BlockShapeBoundary::Rectangle {
                        kind: BlockRectangleKind::Composite,
                        ..
                    } => "basic cluster composite label-container",
                    BlockShapeBoundary::Rectangle { .. } | BlockShapeBoundary::Circle { .. } => {
                        "basic label-container"
                    }
                    BlockShapeBoundary::Cylinder { .. } => "basic label-container outer-path",
                    BlockShapeBoundary::Polygon { .. } => "label-container",
                    BlockShapeBoundary::Stadium { .. } => {
                        unreachable!("RoughJS stadiums fail Block preflight")
                    }
                    BlockShapeBoundary::DoubleCircle { .. } => {
                        unreachable!("double circles use the dedicated branch")
                    }
                };
                self.path_classes.insert(id.to_string(), class.to_string());
                if !inline_styles.is_empty() {
                    self.path_inline_styles
                        .insert(id.to_string(), inline_styles.clone());
                }
                self.add_path(
                    id.to_string(),
                    shape_segments(node, boundary),
                    path_style(fill, stroke_color),
                )?;
            }
        }
        Ok(())
    }

    fn emit_label_lines(
        &mut self,
        node: &LayoutNode,
        geometry: &BlockShapeGeometry,
        lines: &[String],
        style: &BlockStyle,
        layout_width: f64,
        layout_height: f64,
    ) -> Result<()> {
        if lines.iter().all(|line| line.is_empty()) {
            return Ok(());
        }
        let visually_empty = lines.iter().all(|line| line.trim().is_empty());
        let text_line_height = style
            .line_height
            .max(layout_height / lines.len().max(1) as f64)
            .max(style.font_size);
        let bounds_line_height = if visually_empty {
            0.0
        } else {
            text_line_height
        };
        let total_height = bounds_line_height * lines.len().max(1) as f64;
        let label_center = label_center(
            node,
            geometry,
            &self
                .sources
                .get(&node.id)
                .map(|source| source.block_type.as_str())
                .unwrap_or(""),
        );
        let label_width = if visually_empty {
            0.0
        } else {
            layout_width.max(0.0)
        };
        let label_left = label_center.x - label_width / 2.0;
        for (line_index, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            self.session.checkpoint(OperationPhase::Emit)?;
            let origin_x = match style.text_anchor {
                TextAnchor::Start => label_left,
                TextAnchor::Middle => label_center.x,
                TextAnchor::End => label_left + label_width,
            };
            let origin = Point::new(
                origin_x,
                label_center.y - total_height / 2.0
                    + bounds_line_height * (line_index as f64 + 0.5),
            );
            self.commands.push(DrawingCommand::DrawText {
                run: TextRun {
                    text: line.clone(),
                    origin,
                    bounds: Rect::new(
                        label_left,
                        origin.y - bounds_line_height / 2.0,
                        label_width,
                        bounds_line_height,
                    ),
                    style: DisplayTextStyle {
                        font: style.font.clone(),
                        font_size: style.font_size,
                        letter_spacing: 0.0,
                        line_height: text_line_height,
                        fill: Paint::solid(with_alpha(style.text, style.opacity)),
                    },
                    anchor: style.text_anchor,
                    baseline: TextBaseline::Middle,
                    direction: TextDirection::Auto,
                    language: None,
                    obligation: self.text_obligation.clone(),
                },
            });
        }
        Ok(())
    }

    fn emit_edge(&mut self, index: usize, edge: &BlockEdgeRenderModel) -> Result<()> {
        let layout_edge = self
            .edge_by_id
            .get(&edge.id)
            .ok_or_else(|| invalid(format!("Block edge `{}` has no layout geometry", edge.id)))?;
        let semantic_id = format!("block.edge.{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "edgePath".to_string());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let mut points = self.edge_points(edge, layout_edge)?;
        if points.len() >= 2 {
            inset_endpoint(
                &mut points,
                true,
                marker_inset(edge.arrow_type_start.as_deref(), true),
            );
            inset_endpoint(
                &mut points,
                false,
                marker_inset(edge.arrow_type_end.as_deref(), false),
            );
        }
        let style = self.edge_style(edge)?;
        let route_id = format!("{semantic_id}.route");
        self.path_classes.insert(
            route_id.clone(),
            "edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1".to_string(),
        );
        self.dom_ids.insert(route_id.clone(), edge.id.clone());
        self.add_path(
            route_id,
            flowchart_curve_segments(&points, FlowchartCurveKind::Basis, 0.0, false, None),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: style.stroke.map(|color| StrokeStyle {
                    paint: Paint::solid(with_alpha(color, style.opacity * style.stroke_opacity)),
                    width: style.stroke_width,
                    dash_array: style.dash_array.clone(),
                    dash_offset: style.dash_offset,
                    line_cap: style.line_cap,
                    line_join: style.line_join,
                    miter_limit: style.miter_limit,
                }),
            },
        )?;
        self.emit_marker(
            &format!("{semantic_id}.marker.start"),
            edge.arrow_type_start.as_deref(),
            &points,
            true,
            &style,
        )?;
        self.emit_marker(
            &format!("{semantic_id}.marker.end"),
            edge.arrow_type_end.as_deref(),
            &points,
            false,
            &style,
        )?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: (!edge.label.trim().is_empty())
                .then(|| plain_label(&edge.label))
                .transpose()?,
            description: Some(format!("{} → {}", edge.start, edge.end)),
            link: None,
        });
        Ok(())
    }

    fn emit_marker(
        &mut self,
        id: &str,
        arrow: Option<&str>,
        points: &[LayoutPoint],
        start: bool,
        style: &BlockStyle,
    ) -> Result<()> {
        let Some(marker) = marker_kind(arrow)? else {
            return Ok(());
        };
        self.path_classes
            .insert(id.to_string(), "arrowMarkerPath".to_string());
        let endpoint = if start {
            points
                .first()
                .ok_or_else(|| invalid("Block edge has no start point"))?
        } else {
            points
                .last()
                .ok_or_else(|| invalid("Block edge has no end point"))?
        };
        let adjacent = if start {
            points.iter().skip(1).find(|candidate| {
                (candidate.x - endpoint.x).hypot(candidate.y - endpoint.y) > f64::EPSILON
            })
        } else {
            points.iter().rev().skip(1).find(|candidate| {
                (candidate.x - endpoint.x).hypot(candidate.y - endpoint.y) > f64::EPSILON
            })
        }
        .ok_or_else(|| {
            unavailable(
                "Block edge marker has no non-zero tangent; DrawingList v1 cannot choose a lossless marker orientation",
            )
        })?;
        let direction = if start {
            normalize(adjacent.x - endpoint.x, adjacent.y - endpoint.y)
        } else {
            normalize(endpoint.x - adjacent.x, endpoint.y - adjacent.y)
        };
        let normal = Point::new(-direction.y, direction.x);
        let marker_point = |x: f64, y: f64, ref_x: f64, ref_y: f64, scale: f64| {
            let dx = (x - ref_x) * scale;
            let dy = (y - ref_y) * scale;
            Point::new(
                endpoint.x + direction.x * dx + normal.x * dy,
                endpoint.y + direction.y * dx + normal.y * dy,
            )
        };
        let stroke_color = style.stroke.unwrap_or(self.line_color);
        let stroke_color = with_alpha(stroke_color, style.opacity * style.stroke_opacity);
        let fill_color = with_alpha(self.arrow_color, style.opacity * style.fill_opacity);
        let segments = match marker {
            BlockMarker::Point if start => polygon_path(&[
                marker_point(0.0, 5.0, 4.5, 5.0, 0.8),
                marker_point(10.0, 10.0, 4.5, 5.0, 0.8),
                marker_point(10.0, 0.0, 4.5, 5.0, 0.8),
            ]),
            BlockMarker::Point => polygon_path(&[
                marker_point(0.0, 0.0, 5.0, 5.0, 0.8),
                marker_point(10.0, 5.0, 5.0, 5.0, 0.8),
                marker_point(0.0, 10.0, 5.0, 5.0, 0.8),
            ]),
            BlockMarker::Circle => {
                let ref_x = if start { -1.0 } else { 11.0 };
                let center = marker_point(5.0, 5.0, ref_x, 5.0, 1.1);
                ellipse_path(center.x, center.y, 5.5, 5.5)
            }
            BlockMarker::Cross => vec![
                PathSegment::MoveTo {
                    to: marker_point(1.0, 1.0, if start { -1.0 } else { 12.0 }, 5.2, 1.0),
                },
                PathSegment::LineTo {
                    to: marker_point(10.0, 10.0, if start { -1.0 } else { 12.0 }, 5.2, 1.0),
                },
                PathSegment::MoveTo {
                    to: marker_point(10.0, 1.0, if start { -1.0 } else { 12.0 }, 5.2, 1.0),
                },
                PathSegment::LineTo {
                    to: marker_point(1.0, 10.0, if start { -1.0 } else { 12.0 }, 5.2, 1.0),
                },
            ],
        };
        self.add_path(
            id.to_string(),
            segments,
            match marker {
                BlockMarker::Point => PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill_color)),
                    stroke: Some(stroke(stroke_color, 1.0)),
                },
                BlockMarker::Circle => PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill_color)),
                    stroke: Some(stroke(stroke_color, 1.0)),
                },
                BlockMarker::Cross => PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(stroke_color, 2.0)),
                },
            },
        )
    }

    fn emit_edge_label(&mut self, index: usize, edge: &BlockEdgeRenderModel) -> Result<()> {
        if edge.label.trim().is_empty() {
            return Ok(());
        }
        let layout_edge = self
            .edge_by_id
            .get(&edge.id)
            .ok_or_else(|| invalid(format!("Block edge `{}` has no layout geometry", edge.id)))?;
        let label = layout_edge
            .label
            .as_ref()
            .ok_or_else(|| invalid(format!("Block edge `{}` has no label geometry", edge.id)))?;
        let lines = plain_lines(&edge.label, &format!("Block edge `{}` label", edge.id))?;
        let style = self.edge_style(edge)?;
        let semantic_id = format!("block.edge.{index}.label");
        self.semantic_classes
            .insert(semantic_id.clone(), "edgeLabel".to_string());
        self.path_classes
            .insert(format!("{semantic_id}.background"), "label".to_string());
        self.text_classes
            .insert(semantic_id.clone(), "edgeLabel".to_string());
        self.label_max_widths.insert(semantic_id.clone(), 200.0);
        self.label_data_ids
            .insert(semantic_id.clone(), edge.id.clone());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.add_path(
            format!("{semantic_id}.background"),
            rounded_rect_path(
                label.x,
                label.y,
                label.width.max(1.0),
                label.height.max(1.0),
                0.0,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.edge_label_background)),
                stroke: None,
            },
        )?;
        let pseudo_node = LayoutNode {
            id: edge.id.clone(),
            x: label.x,
            y: label.y,
            width: label.width,
            height: label.height,
            is_cluster: false,
            label_width: Some(label.width),
            label_height: Some(label.height),
        };
        let geometry = BlockShapeGeometry {
            id: edge.id.clone(),
            allocated: crate::block::BlockAllocatedBounds {
                x: label.x,
                y: label.y,
                width: label.width,
                height: label.height,
            },
            boundary: BlockShapeBoundary::Rectangle {
                width: label.width,
                height: label.height,
                radius: 0.0,
                kind: BlockRectangleKind::Basic,
            },
        };
        self.emit_label_lines(
            &pseudo_node,
            &geometry,
            &lines,
            &style,
            label.width,
            label.height,
        )?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(lines.join(" ")),
            description: Some(format!("{} → {}", edge.start, edge.end)),
            link: None,
        });
        Ok(())
    }

    fn edge_points(
        &self,
        edge: &BlockEdgeRenderModel,
        layout_edge: &LayoutEdge,
    ) -> Result<Vec<LayoutPoint>> {
        let from = self.geometry_by_id.get(&edge.start).ok_or_else(|| {
            invalid(format!(
                "Block edge `{}` references missing start node",
                edge.id
            ))
        })?;
        let to = self.geometry_by_id.get(&edge.end).ok_or_else(|| {
            invalid(format!(
                "Block edge `{}` references missing end node",
                edge.id
            ))
        })?;
        let midpoint = layout_edge
            .points
            .get(1)
            .cloned()
            .ok_or_else(|| invalid(format!("Block edge `{}` has no midpoint", edge.id)))?;
        let mut points = layout_edge.points.clone();
        if points.len() >= 2 {
            points[0] = from.intersect(&midpoint);
            let last = points.len() - 1;
            points[last] = to.intersect(&midpoint);
        }
        Ok(points)
    }

    fn node_style(
        &self,
        source: &BlockSource,
        geometry: &BlockShapeGeometry,
    ) -> Result<BlockStyle> {
        let (default_fill, default_stroke) = match geometry.boundary {
            BlockShapeBoundary::Rectangle {
                kind: BlockRectangleKind::Composite,
                ..
            } => (self.cluster_fill, self.cluster_border),
            _ => (self.node_fill, self.node_border),
        };
        let mut style = BlockStyle::base(
            Some(default_fill),
            default_stroke,
            self.node_text,
            self.stroke_width,
            &self.base_font,
            &self.base_font_family_css,
            self.base_font_size,
        );
        for class in &source.classes {
            let class_def = self.model.class_defs.get(class).ok_or_else(|| {
                unavailable(format!(
                    "Block node `{}` references missing class `{class}`",
                    source.label
                ))
            })?;
            for declaration in &class_def.styles {
                apply_declaration(&mut style, declaration, StyleTarget::Box)?;
            }
            for declaration in &class_def.text_styles {
                apply_declaration(&mut style, declaration, StyleTarget::Text)?;
            }
        }
        for declaration in &source.styles {
            apply_declaration(&mut style, declaration, StyleTarget::Box)?;
        }
        Ok(style)
    }

    fn edge_style(&self, _edge: &BlockEdgeRenderModel) -> Result<BlockStyle> {
        Ok(BlockStyle::base(
            None,
            self.line_color,
            self.node_text,
            self.stroke_width,
            &self.base_font,
            &self.base_font_family_css,
            self.base_font_size,
        ))
    }

    fn viewport(&self) -> Result<Rect> {
        let fallback = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Block layout did not provide root bounds"))?;
        validate_bounds(fallback)?;
        let mut extents: Option<(f64, f64, f64, f64)> = None;
        for geometry in self.geometry_by_id.values() {
            let current = geometry.rendered_extents();
            extents = Some(match extents {
                None => current,
                Some((min_x, min_y, max_x, max_y)) => (
                    min_x.min(current.0),
                    min_y.min(current.1),
                    max_x.max(current.2),
                    max_y.max(current.3),
                ),
            });
        }
        let (min_x, min_y, max_x, max_y) = extents.unwrap_or((
            fallback.min_x,
            fallback.min_y,
            fallback.max_x,
            fallback.max_y,
        ));
        let padding = crate::config::config_f64(
            self.metadata.effective_config.as_value(),
            &["block", "diagramPadding"],
        )
        .unwrap_or(DEFAULT_BLOCK_PADDING)
        .max(0.0);
        Ok(Rect::new(
            min_x - padding,
            min_y - padding,
            (max_x - min_x + 2.0 * padding).max(1.0),
            (max_y - min_y + 2.0 * padding).max(1.0),
        ))
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!("Block path `{id}` has no geometry")));
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
}

impl BlockStyle {
    fn base(
        fill: Option<Color>,
        stroke_color: Color,
        text: Color,
        stroke_width: f64,
        font: &FontDescriptor,
        font_family_css: &str,
        font_size: f64,
    ) -> Self {
        Self {
            fill,
            stroke: Some(stroke_color),
            stroke_width,
            dash_array: Vec::new(),
            dash_offset: 0.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
            fill_rule: FillRule::NonZero,
            opacity: 1.0,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            text,
            font: font.clone(),
            font_family_css: font_family_css.to_string(),
            font_size,
            line_height: font_size * 1.5,
            text_anchor: TextAnchor::Middle,
            visible: true,
        }
    }
}

fn collect_sources(model: &BlockDiagramRenderModel) -> HashMap<String, BlockSource> {
    let mut sources = HashMap::new();
    for root in &model.blocks_flat {
        collect_source(root, &mut sources);
    }
    sources
}

fn collect_source(node: &BlockNodeRenderModel, out: &mut HashMap<String, BlockSource>) {
    if let Some(existing) = out.get_mut(&node.id) {
        if !node.label.is_empty() {
            existing.label = node.label.clone();
        }
        if !node.block_type.is_empty() && node.block_type != "na" {
            existing.block_type = node.block_type.clone();
        }
        if !node.classes.is_empty() {
            existing.classes = node.classes.clone();
        }
        if !node.styles.is_empty() {
            existing.styles = node.styles.clone();
        }
        if !node.directions.is_empty() {
            existing.directions = node.directions.clone();
        }
    } else {
        out.insert(
            node.id.clone(),
            BlockSource {
                label: node.label.clone(),
                block_type: node.block_type.clone(),
                classes: node.classes.clone(),
                styles: node.styles.clone(),
                directions: node.directions.clone(),
            },
        );
    }
    for child in node.children.iter().rev() {
        collect_source(child, out);
    }
}

fn validate_model(
    model: &BlockDiagramRenderModel,
    layout: &BlockDiagramLayout,
    sources: &HashMap<String, BlockSource>,
    geometries: &HashMap<String, &BlockShapeGeometry>,
    nodes: &HashMap<String, &LayoutNode>,
    edges: &HashMap<String, &LayoutEdge>,
) -> Result<()> {
    if model
        .class_defs
        .values()
        .any(|definition| definition.id.trim().is_empty())
    {
        return Err(invalid("Block class definition has an empty id"));
    }
    if layout.nodes.len() != geometries.len() || layout.nodes.len() != nodes.len() {
        return Err(invalid(
            "Block layout contains duplicate or missing node geometry",
        ));
    }
    for node in &layout.nodes {
        if node.is_cluster {
            return Err(invalid(format!(
                "Block layout node `{}` is unexpectedly a cluster",
                node.id
            )));
        }
        if node.id.is_empty()
            || ![node.x, node.y, node.width, node.height]
                .into_iter()
                .all(f64::is_finite)
            || node.width < 0.0
            || node.height < 0.0
        {
            return Err(invalid(format!(
                "Block node `{}` has invalid geometry",
                node.id
            )));
        }
        if !sources.contains_key(&node.id) || !geometries.contains_key(&node.id) {
            return Err(invalid(format!(
                "Block node `{}` has incomplete source geometry",
                node.id
            )));
        }
        if node
            .label_width
            .into_iter()
            .chain(node.label_height)
            .any(|value| !value.is_finite() || value < 0.0)
        {
            return Err(invalid(format!(
                "Block node `{}` has invalid label metrics",
                node.id
            )));
        }
        let source = sources.get(&node.id).expect("checked above");
        match &geometries.get(&node.id).expect("checked above").boundary {
            BlockShapeBoundary::Stadium { .. } => {
                return Err(unavailable(format!(
                    "Block node `{}` uses a stadium rendered through RoughJS even in classic mode",
                    node.id
                )));
            }
            BlockShapeBoundary::Polygon { .. }
                if matches!(source.block_type.as_str(), "odd" | "rect_left_inv_arrow") =>
            {
                return Err(unavailable(format!(
                    "Block node `{}` uses an odd shape rendered through RoughJS even in classic mode",
                    node.id
                )));
            }
            _ => {}
        }
        if source
            .classes
            .iter()
            .any(|class| !model.class_defs.contains_key(class))
        {
            let class = source
                .classes
                .iter()
                .find(|class| !model.class_defs.contains_key(*class))
                .expect("one missing class");
            return Err(unavailable(format!(
                "Block node `{}` references missing class `{class}`",
                node.id
            )));
        }
        plain_lines(&source.label, &format!("Block node `{}` label", node.id))?;
        for declaration in &source.styles {
            validate_declaration(declaration)?;
        }
    }
    let mut semantic_edges = HashSet::with_capacity(model.edges.len());
    for edge in &model.edges {
        if edge.id.is_empty() || !semantic_edges.insert(edge.id.as_str()) {
            return Err(invalid(format!(
                "duplicate or empty Block edge id `{}`",
                edge.id
            )));
        }
        let layout_edge = edges
            .get(&edge.id)
            .ok_or_else(|| invalid(format!("Block edge `{}` has no layout route", edge.id)))?;
        if edge.start.is_empty()
            || edge.end.is_empty()
            || !nodes.contains_key(&edge.start)
            || !nodes.contains_key(&edge.end)
            || layout_edge.points.len() < 2
            || layout_edge
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(invalid(format!(
                "Block edge `{}` has invalid route geometry",
                edge.id
            )));
        }
        marker_kind(edge.arrow_type_start.as_deref())?;
        marker_kind(edge.arrow_type_end.as_deref())?;
        if let Some(label) = &layout_edge.label
            && [label.x, label.y, label.width, label.height]
                .into_iter()
                .any(|value| !value.is_finite() || value < 0.0)
        {
            return Err(invalid(format!(
                "Block edge `{}` has invalid label geometry",
                edge.id
            )));
        }
        plain_lines(&edge.label, &format!("Block edge `{}` label", edge.id))?;
    }
    for definition in model.class_defs.values() {
        for declaration in definition.styles.iter().chain(&definition.text_styles) {
            validate_declaration(declaration)?;
        }
    }
    Ok(())
}

fn unique_nodes<'a>(layout: &'a BlockDiagramLayout) -> Result<HashMap<String, &'a LayoutNode>> {
    let mut out = HashMap::with_capacity(layout.nodes.len());
    for node in &layout.nodes {
        if out.insert(node.id.clone(), node).is_some() {
            return Err(invalid(format!(
                "duplicate Block layout node `{}`",
                node.id
            )));
        }
    }
    Ok(out)
}

fn unique_edges<'a>(layout: &'a BlockDiagramLayout) -> Result<HashMap<String, &'a LayoutEdge>> {
    let mut out = HashMap::with_capacity(layout.edges.len());
    for edge in &layout.edges {
        if out.insert(edge.id.clone(), edge).is_some() {
            return Err(invalid(format!(
                "duplicate Block layout edge `{}`",
                edge.id
            )));
        }
    }
    Ok(out)
}

fn unique_geometries<'a>(
    layout: &'a BlockDiagramLayout,
) -> Result<HashMap<String, &'a BlockShapeGeometry>> {
    let mut out = HashMap::with_capacity(layout.shape_geometries.len());
    for geometry in &layout.shape_geometries {
        if out.insert(geometry.id.clone(), geometry).is_some() {
            return Err(invalid(format!(
                "duplicate Block shape geometry `{}`",
                geometry.id
            )));
        }
    }
    Ok(out)
}

fn shape_segments(node: &LayoutNode, boundary: &BlockShapeBoundary) -> Vec<PathSegment> {
    match boundary {
        BlockShapeBoundary::Rectangle {
            width,
            height,
            radius,
            ..
        } => rounded_rect_path(node.x, node.y, *width, *height, *radius),
        BlockShapeBoundary::Circle { radius, .. }
        | BlockShapeBoundary::DoubleCircle {
            outer_radius: radius,
            ..
        } => ellipse_path(node.x, node.y, *radius, *radius),
        BlockShapeBoundary::Stadium { width, height } => {
            rounded_rect_path(node.x, node.y, *width, *height, *height / 2.0)
        }
        BlockShapeBoundary::Cylinder {
            width,
            body_height,
            radius_x,
            radius_y,
        } => cylinder_path(node.x, node.y, *width, *body_height, *radius_x, *radius_y),
        BlockShapeBoundary::Polygon {
            points,
            translation,
        } => polygon_path(
            &points
                .iter()
                .map(|point| {
                    Point::new(
                        node.x + point.x + translation.x,
                        node.y + point.y + translation.y,
                    )
                })
                .collect::<Vec<_>>(),
        ),
    }
}

fn cylinder_path(
    x: f64,
    y: f64,
    width: f64,
    body_height: f64,
    radius_x: f64,
    radius_y: f64,
) -> Vec<PathSegment> {
    let left = x - width / 2.0;
    let right = x + width / 2.0;
    let top = y - body_height / 2.0;
    let bottom = y + body_height / 2.0;
    vec![
        PathSegment::MoveTo {
            to: Point::new(left, top),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: false,
            to: Point::new(right, top),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: false,
            to: Point::new(left, top),
        },
        PathSegment::LineTo {
            to: Point::new(left, bottom),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: false,
            to: Point::new(right, bottom),
        },
        PathSegment::LineTo {
            to: Point::new(right, top),
        },
        PathSegment::Close,
    ]
}

fn label_center(node: &LayoutNode, geometry: &BlockShapeGeometry, block_type: &str) -> Point {
    let x = if matches!(block_type, "rect_left_inv_arrow" | "odd") {
        match &geometry.boundary {
            BlockShapeBoundary::Polygon { translation, .. } => node.x + translation.x,
            _ => node.x,
        }
    } else {
        node.x
    };
    Point::new(x, node.y)
}

fn plain_lines(raw: &str, context: &str) -> Result<Vec<String>> {
    let decoded = raw.replace("&nbsp;", "\u{00A0}");
    if decoded.trim().is_empty() {
        return Ok(vec![decoded]);
    }
    split_html_br_lines(&decoded)
        .into_iter()
        .map(|line| {
            let fragment = mermaid_markdown_to_xhtml_label_fragment(line, true);
            let Some(text) = mermaid_xhtml_label_plain_text(&fragment) else {
                return Err(unavailable(format!(
                    "{context} contains Markdown/HTML styling that DrawingList v1 cannot preserve"
                )));
            };
            Ok(svg_plain_text(
                merman_core::entities::decode_mermaid_entities_to_unicode(&text).as_ref(),
            ))
        })
        .collect()
}

fn plain_label(raw: &str) -> Result<String> {
    Ok(plain_lines(raw, "Block label")?.join(" "))
}

fn validate_declaration(raw: &str) -> Result<()> {
    crate::mermaid_style::parse_safe_style_decl(raw).ok_or_else(|| {
        unavailable(format!(
            "Block style declaration `{raw}` is malformed or unsafe"
        ))
    })?;
    Ok(())
}

fn apply_declaration(style: &mut BlockStyle, raw: &str, target: StyleTarget) -> Result<()> {
    let (key, value) = crate::mermaid_style::parse_safe_style_decl(raw).ok_or_else(|| {
        unavailable(format!(
            "Block style declaration `{raw}` is malformed or unsafe"
        ))
    })?;
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim();
    let resolver = PortableStyleResolver::new("block");
    match key.as_str() {
        "fill" => match target {
            StyleTarget::Box => style.fill = resolver.optional_color("fill", value)?,
            StyleTarget::Text => style.text = resolver.color("fill", value)?,
        },
        "background-color" => {
            style.fill = resolver.optional_color("background-color", value)?;
        }
        "stroke" => {
            if matches!(target, StyleTarget::Text) {
                return Err(unavailable(
                    "Block text stroke has no v1 text-paint contract",
                ));
            }
            style.stroke = resolver.optional_color("stroke", value)?;
        }
        "stroke-width" => style.stroke_width = resolver.length("stroke-width", value)?,
        "stroke-dasharray" => style.dash_array = parse_dash_array(value)?,
        "stroke-dashoffset" => style.dash_offset = parse_signed_length(value)?,
        "stroke-linecap" => style.line_cap = parse_line_cap(value)?,
        "stroke-linejoin" => style.line_join = parse_line_join(value)?,
        "stroke-miterlimit" => style.miter_limit = parse_positive_number(value)?,
        "fill-rule" => style.fill_rule = parse_fill_rule(value)?,
        "opacity" => style.opacity = resolver.opacity("opacity", value)?,
        "fill-opacity" => style.fill_opacity = resolver.opacity("fill-opacity", value)?,
        "stroke-opacity" => style.stroke_opacity = resolver.opacity("stroke-opacity", value)?,
        "color" => style.text = resolver.color("color", value)?,
        "font-family" => {
            if !crate::mermaid_style::is_safe_css_font_family_value(value) {
                return Err(unavailable(format!(
                    "Block font-family `{value}` is not portable"
                )));
            }
            let family = crate::config::normalize_css_font_family(value);
            if family.is_empty() {
                return Err(unavailable(
                    "Block font-family resolves to an empty family list",
                ));
            }
            style.font_family_css = family.clone();
            style.font.families = parse_font_families(family);
        }
        "font-size" => style.font_size = parse_font_size(value, style.font_size)?,
        "font-weight" => style.font.weight = parse_font_weight(value)?,
        "font-style" => style.font.style = parse_font_style(value)?,
        "line-height" => style.line_height = parse_line_height(value, style.font_size)?,
        "text-align" => {
            style.text_anchor = match value.to_ascii_lowercase().as_str() {
                "left" | "start" => TextAnchor::Start,
                "center" | "middle" => TextAnchor::Middle,
                "right" | "end" => TextAnchor::End,
                _ => {
                    return Err(unavailable(format!(
                        "Block text-align `{value}` is unsupported"
                    )));
                }
            }
        }
        "display" => {
            style.visible = match value.to_ascii_lowercase().as_str() {
                "none" => false,
                "inline" | "block" => true,
                _ => {
                    return Err(unavailable(format!(
                        "Block display `{value}` is unsupported"
                    )));
                }
            }
        }
        "visibility" => {
            style.visible = match value.to_ascii_lowercase().as_str() {
                "hidden" | "collapse" => false,
                "visible" => true,
                _ => {
                    return Err(unavailable(format!(
                        "Block visibility `{value}` is unsupported"
                    )));
                }
            }
        }
        _ => {
            return Err(unavailable(format!(
                "Block style property `{key}` has no lossless DrawingList v1 mapping"
            )));
        }
    }
    Ok(())
}

fn parse_font_size(value: &str, inherited: f64) -> Result<f64> {
    let value = value.trim();
    let (number, factor) = if let Some(value) = value.strip_suffix("px") {
        (value, 1.0)
    } else if let Some(value) = value.strip_suffix("pt") {
        (value, 4.0 / 3.0)
    } else if let Some(value) = value.strip_suffix("em") {
        (value, inherited)
    } else if let Some(value) = value.strip_suffix("rem") {
        (value, inherited)
    } else {
        (value, 1.0)
    };
    let parsed = number
        .trim()
        .parse::<f64>()
        .map_err(|_| unavailable(format!("Block font-size `{value}` is not portable")))?
        * factor;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(unavailable(format!(
            "Block font-size `{value}` is outside the portable range"
        )));
    }
    Ok(parsed)
}

fn parse_line_height(value: &str, font_size: f64) -> Result<f64> {
    if value.eq_ignore_ascii_case("normal") {
        return Ok(font_size * 1.2);
    }
    if let Some(percent) = value.strip_suffix('%') {
        let percent = percent
            .trim()
            .parse::<f64>()
            .map_err(|_| unavailable(format!("Block line-height `{value}` is not portable")))?;
        if !percent.is_finite() || percent < 0.0 {
            return Err(unavailable("Block line-height must be non-negative"));
        }
        return Ok(font_size * percent / 100.0);
    }
    if let Ok(multiplier) = value.trim().parse::<f64>()
        && multiplier.is_finite()
        && multiplier >= 0.0
    {
        return Ok(font_size * multiplier);
    }
    parse_font_size(value, font_size)
}

fn parse_dash_array(value: &str) -> Result<Vec<f64>> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    value
        .split([',', ' ', '\t'])
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            let value = value.trim().strip_suffix("px").unwrap_or(value.trim());
            let parsed = value.parse::<f64>().map_err(|_| {
                unavailable(format!("Block stroke-dasharray `{value}` is not portable"))
            })?;
            if !parsed.is_finite() || parsed < 0.0 {
                return Err(unavailable("Block stroke-dasharray must be non-negative"));
            }
            Ok(parsed)
        })
        .collect()
}

fn parse_signed_length(value: &str) -> Result<f64> {
    let value = value.trim();
    let value = value.strip_suffix("px").unwrap_or(value);
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| unavailable(format!("Block signed length `{value}` is not portable")))
}

fn parse_positive_number(value: &str) -> Result<f64> {
    let parsed = value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0);
    parsed.ok_or_else(|| unavailable(format!("Block positive number `{value}` is invalid")))
}

fn parse_line_cap(value: &str) -> Result<LineCap> {
    match value.trim().to_ascii_lowercase().as_str() {
        "butt" => Ok(LineCap::Butt),
        "round" => Ok(LineCap::Round),
        "square" => Ok(LineCap::Square),
        _ => Err(unavailable(format!(
            "Block stroke-linecap `{value}` is unsupported"
        ))),
    }
}

fn parse_line_join(value: &str) -> Result<LineJoin> {
    match value.trim().to_ascii_lowercase().as_str() {
        "miter" => Ok(LineJoin::Miter),
        "round" => Ok(LineJoin::Round),
        "bevel" => Ok(LineJoin::Bevel),
        _ => Err(unavailable(format!(
            "Block stroke-linejoin `{value}` is unsupported"
        ))),
    }
}

fn parse_fill_rule(value: &str) -> Result<FillRule> {
    match value.trim().to_ascii_lowercase().as_str() {
        "nonzero" | "non-zero" => Ok(FillRule::NonZero),
        "evenodd" | "even-odd" => Ok(FillRule::EvenOdd),
        _ => Err(unavailable(format!(
            "Block fill-rule `{value}` is unsupported"
        ))),
    }
}

fn parse_font_weight(value: &str) -> Result<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(400),
        "bold" | "bolder" => Ok(700),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight))
            .ok_or_else(|| unavailable(format!("Block font-weight `{value}` is unsupported"))),
    }
}

fn parse_font_style(value: &str) -> Result<FontStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(unavailable(format!(
            "Block font-style `{value}` is unsupported"
        ))),
    }
}

fn theme_font_weight(config: &Value) -> Result<u16> {
    let value = config
        .get("themeVariables")
        .and_then(|variables| variables.get("fontWeight"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_u64().map(|n| n.to_string()))
        })
        .unwrap_or_else(|| "400".to_string());
    parse_font_weight(&value)
}

fn theme_length(
    config: &Value,
    key: &str,
    fallback: &str,
    resolver: &PortableStyleResolver,
) -> Result<f64> {
    let value = config
        .get("themeVariables")
        .and_then(|variables| variables.get(key))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_f64().map(|n| n.to_string()))
        })
        .unwrap_or_else(|| fallback.to_string());
    resolver.length(key, &value)
}

fn config_theme_color(
    config: &Value,
    primary: &str,
    secondary: &str,
    fallback: &str,
) -> Result<Color> {
    if config
        .get("themeVariables")
        .and_then(|variables| variables.get(primary))
        .and_then(Value::as_str)
        .is_some()
    {
        return theme_color(config, primary, fallback);
    }
    theme_color(config, secondary, fallback)
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

#[derive(Debug, Clone, Copy)]
enum BlockMarker {
    Point,
    Circle,
    Cross,
}

fn marker_kind(value: Option<&str>) -> Result<Option<BlockMarker>> {
    match value.unwrap_or("").trim() {
        "" | "arrow_open" => Ok(None),
        "arrow_point" => Ok(Some(BlockMarker::Point)),
        "arrow_circle" => Ok(Some(BlockMarker::Circle)),
        "arrow_cross" => Ok(Some(BlockMarker::Cross)),
        other => Err(unavailable(format!(
            "Block marker `{other}` has no lossless DrawingList v1 mapping"
        ))),
    }
}

fn marker_inset(value: Option<&str>, start: bool) -> f64 {
    match (value.unwrap_or("").trim(), start) {
        ("arrow_point", true) => 4.5,
        ("arrow_point", false) => 4.0,
        _ => 0.0,
    }
}

fn inset_endpoint(points: &mut [LayoutPoint], start: bool, distance: f64) {
    if distance <= 0.0 || points.len() < 2 {
        return;
    }
    let (index, adjacent_index) = if start {
        (0, 1)
    } else {
        (points.len() - 1, points.len() - 2)
    };
    let point = points[index].clone();
    let target = points[adjacent_index].clone();
    let dx = target.x - point.x;
    let dy = target.y - point.y;
    let length = dx.hypot(dy);
    if length <= f64::EPSILON {
        return;
    }
    points[index].x += dx / length * distance;
    points[index].y += dy / length * distance;
}

fn normalize(x: f64, y: f64) -> Point {
    let length = x.hypot(y);
    if !length.is_finite() || length <= f64::EPSILON {
        Point::new(0.0, 1.0)
    } else {
        Point::new(x / length, y / length)
    }
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("Block root bounds are invalid"));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: RenderFamilyKind::Block.as_str().to_string(),
        reason: message.into(),
    }
}
