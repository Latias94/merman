//! Renderer-neutral Mindmap adapter.
//!
//! Mindmap SVG output contains HTML labels and a small amount of look-specific CSS.  The
//! canonical adapter therefore accepts the lossless vector subset explicitly and fails closed
//! for labels/effects that cannot be represented by DrawingList v1.  It never parses the SVG
//! output back into a second visual model.

use super::{
    MindmapSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    parse_svg_path, theme_color,
};
use crate::config::{
    config_bool, config_diagram_look, config_f64, config_font_family_css, config_string,
};
use crate::environment::{RenderSession, TextMeasurementPhase, TextMeasurementSource};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::flowchart::flowchart_label_plain_text_for_layout;
use crate::model::{Bounds, LayoutEdge, LayoutNode, LayoutPoint, MindmapDiagramLayout};
use crate::render_geometry::{FlowchartCurveKind, flowchart_curve_segments};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::mindmap::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, GradientSpread,
    GradientStop, LineCap, LineJoin, LinearGradientResource, MeasurementProvenance, Paint,
    PathResource, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, StrokeStyle, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun,
    TextStyle, Transform, Viewport,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

const DEFAULT_MINDMAP_FILLS: [&str; 12] = [
    "hsl(240, 100%, 76.2745098039%)",
    "hsl(60, 100%, 73.5294117647%)",
    "hsl(80, 100%, 76.2745098039%)",
    "hsl(270, 100%, 76.2745098039%)",
    "hsl(300, 100%, 76.2745098039%)",
    "hsl(330, 100%, 76.2745098039%)",
    "hsl(0, 100%, 76.2745098039%)",
    "hsl(30, 100%, 76.2745098039%)",
    "hsl(90, 100%, 76.2745098039%)",
    "hsl(150, 100%, 76.2745098039%)",
    "hsl(180, 100%, 76.2745098039%)",
    "hsl(210, 100%, 76.2745098039%)",
];

const DEFAULT_MINDMAP_INV_FILLS: [&str; 12] = [
    "hsl(60, 100%, 86.2745098039%)",
    "hsl(240, 100%, 83.5294117647%)",
    "hsl(260, 100%, 86.2745098039%)",
    "hsl(90, 100%, 86.2745098039%)",
    "hsl(120, 100%, 86.2745098039%)",
    "hsl(150, 100%, 86.2745098039%)",
    "hsl(180, 100%, 86.2745098039%)",
    "hsl(210, 100%, 86.2745098039%)",
    "hsl(270, 100%, 86.2745098039%)",
    "hsl(330, 100%, 86.2745098039%)",
    "hsl(0, 100%, 86.2745098039%)",
    "hsl(30, 100%, 86.2745098039%)",
];

const DEFAULT_MINDMAP_LABELS: [&str; 12] = [
    "#ffffff", "black", "black", "#ffffff", "black", "black", "black", "black", "black", "black",
    "black", "black",
];

type MindmapPair = FamilyPair<MindmapDiagramRenderModel, MindmapDiagramLayout>;

pub(crate) fn build_mindmap_document(
    pair: &MindmapPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let mut builder = MindmapBuilder::new(pair, metadata, policy, session)?;
    builder.build()
}

struct MindmapBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a MindmapDiagramRenderModel,
    layout: &'a MindmapDiagramLayout,
    layout_nodes_by_id: HashMap<&'a str, &'a LayoutNode>,
    layout_edges_by_id: HashMap<&'a str, &'a LayoutEdge>,
    font: FontDescriptor,
    text_obligation: TextObligation,
    look: String,
    theme: String,
    theme_color_limit: usize,
    root_fill: Color,
    root_text: Color,
    main_bkg: Color,
    node_border: Color,
    stroke_width: f64,
    use_gradient: bool,
    gradient_start: Option<Color>,
    gradient_stop: Option<Color>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    extensions: std::collections::BTreeMap<String, Value>,
}

impl<'a> MindmapBuilder<'a> {
    fn new(
        pair: &'a MindmapPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let model = pair.semantic();
        let layout = pair.layout();
        let bounds = layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Mindmap layout did not provide root bounds"))?;
        validate_bounds(bounds)?;

        let layout_nodes_by_id = layout
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let layout_edges_by_id = layout
            .edges
            .iter()
            .map(|edge| (edge.id.as_str(), edge))
            .collect::<HashMap<_, _>>();

        let look = effective_look(model, metadata);
        if !matches!(look.as_str(), "classic" | "neo") {
            return Err(unavailable(format!(
                "look `{look}` has no lossless DrawingList v1 adapter"
            )));
        }
        if look == "neo" {
            let drop_shadow = config_string(
                metadata.effective_config.as_value(),
                &["themeVariables", "dropShadow"],
            )
            .unwrap_or_else(|| "none".to_string());
            if !drop_shadow.trim().is_empty() && !drop_shadow.eq_ignore_ascii_case("none") {
                return Err(unavailable(
                    "Mindmap neo drop shadows require an explicit filter or raster fallback",
                ));
            }
        }

        let theme =
            config_string(metadata.effective_config.as_value(), &["theme"]).unwrap_or_default();
        let theme_color_limit = config_f64(
            metadata.effective_config.as_value(),
            &["themeVariables", "THEME_COLOR_LIMIT"],
        )
        .map(|value| value.round() as i64)
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MINDMAP_FILLS.len() as i64)
        .clamp(1, 64) as usize;

        let root_fill = theme_color(
            metadata.effective_config.as_value(),
            "git0",
            "hsl(240, 100%, 46.2745098039%)",
        )?;
        let root_text = theme_color(
            metadata.effective_config.as_value(),
            "gitBranchLabel0",
            "#ffffff",
        )?;
        let main_bkg = theme_color(
            metadata.effective_config.as_value(),
            "mainBkg",
            &color_css_fallback(root_fill),
        )?;
        let node_border = theme_color(
            metadata.effective_config.as_value(),
            "nodeBorder",
            &color_css_fallback(root_text),
        )?;
        let stroke_width = config_css_number(
            metadata.effective_config.as_value(),
            &["themeVariables", "strokeWidth"],
        )
        .unwrap_or(1.0);
        if !stroke_width.is_finite() || stroke_width < 0.0 {
            return Err(invalid("Mindmap strokeWidth is invalid"));
        }

        let use_gradient = look == "neo"
            && config_bool(
                metadata.effective_config.as_value(),
                &["themeVariables", "useGradient"],
            )
            .unwrap_or(false);
        let gradient_start = if use_gradient {
            if config_string(
                metadata.effective_config.as_value(),
                &["themeVariables", "gradientStart"],
            )
            .is_none()
            {
                return Err(unavailable(
                    "Mindmap gradientStart is required when useGradient is enabled",
                ));
            }
            Some(theme_color(
                metadata.effective_config.as_value(),
                "gradientStart",
                "#ffffff",
            )?)
        } else {
            None
        };
        let gradient_stop = if use_gradient {
            if config_string(
                metadata.effective_config.as_value(),
                &["themeVariables", "gradientStop"],
            )
            .is_none()
            {
                return Err(unavailable(
                    "Mindmap gradientStop is required when useGradient is enabled",
                ));
            }
            Some(theme_color(
                metadata.effective_config.as_value(),
                "gradientStop",
                "#000000",
            )?)
        } else {
            None
        };

        let font = FontDescriptor {
            families: parse_font_families(config_font_family_css(
                metadata.effective_config.as_value(),
            )),
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let text_obligation = text_obligation(session);

        let builder = Self {
            metadata,
            session,
            policy,
            model,
            layout,
            layout_nodes_by_id,
            layout_edges_by_id,
            font,
            text_obligation,
            look,
            theme,
            theme_color_limit,
            root_fill,
            root_text,
            main_bkg,
            node_border,
            stroke_width,
            use_gradient,
            gradient_start,
            gradient_stop,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "mindmap.document".to_string(),
                },
            ],
            semantics: Vec::new(),
            extensions: std::collections::BTreeMap::new(),
        };
        builder.preflight()?;
        Ok(builder)
    }

    fn build(&mut self) -> Result<RenderDocument> {
        self.session.checkpoint(OperationPhase::Emit)?;
        self.semantics.push(SemanticAnnotation {
            id: "mindmap.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .metadata
                .title
                .clone()
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: None,
            link: None,
        });

        // Match Mermaid's painter order: routes first, then node shapes and labels.
        for (index, edge) in self.model.edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge(index, edge)?;
        }
        for (index, node) in self.model.nodes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_node(index, node)?;
        }

        self.extensions.insert(
            "x-merman-mindmap".to_string(),
            json!({
                "diagram_type": self.metadata.diagram_type,
                "look": self.look,
                "theme": self.theme,
                "label_mode": "plain_host_text",
            }),
        );
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
        let padding = 10.0;
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                bounds.min_x - padding,
                bounds.min_y - padding,
                (bounds.max_x - bounds.min_x) + 2.0 * padding,
                (bounds.max_y - bounds.min_y) + 2.0 * padding,
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
                family: RenderFamilyKind::Mindmap,
                body: SvgStructureBody::Mindmap(MindmapSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                }),
            },
        })
    }

    fn preflight(&self) -> Result<()> {
        if self.model.nodes.is_empty() {
            return Err(invalid("Mindmap semantic model has no nodes"));
        }
        if self.layout.nodes.len() != self.model.nodes.len() {
            return Err(invalid(format!(
                "Mindmap layout has {} nodes for {} semantic nodes",
                self.layout.nodes.len(),
                self.model.nodes.len()
            )));
        }

        let mut node_ids = HashSet::with_capacity(self.model.nodes.len());
        for node in &self.model.nodes {
            if !node_ids.insert(node.id.as_str()) {
                return Err(invalid(format!("duplicate Mindmap node id `{}`", node.id)));
            }
            let layout_node = self
                .layout_nodes_by_id
                .get(node.id.as_str())
                .ok_or_else(|| invalid(format!("Mindmap node `{}` has no layout node", node.id)))?;
            validate_layout_node(layout_node)?;
            if node.icon.is_some() {
                return Err(unavailable(format!(
                    "Mindmap node `{}` uses an icon; DrawingList v1 needs an explicit icon resource",
                    node.id
                )));
            }
            ensure_node_classes(&node.css_classes, &node.id)?;
            if !node.css_styles.is_empty() {
                return Err(unavailable(format!(
                    "Mindmap node `{}` has CSS declarations without a portable v1 mapping",
                    node.id
                )));
            }
            if !matches!(
                node.shape.as_str(),
                "defaultMindmapNode"
                    | ""
                    | "rect"
                    | "rounded"
                    | "mindmapCircle"
                    | "cloud"
                    | "hexagon"
                    | "bang"
            ) {
                return Err(unavailable(format!(
                    "Mindmap node `{}` shape `{}` has no typed v1 adapter",
                    node.id, node.shape
                )));
            }
            self.plain_label(node)?;
        }

        let mut layout_node_ids = HashSet::with_capacity(self.layout.nodes.len());
        for node in &self.layout.nodes {
            if !layout_node_ids.insert(node.id.as_str()) || !node_ids.contains(node.id.as_str()) {
                return Err(invalid(format!(
                    "Mindmap layout contains an unknown or duplicate node `{}`",
                    node.id
                )));
            }
        }

        let mut edge_ids = HashSet::with_capacity(self.model.edges.len());
        for edge in &self.model.edges {
            if !edge_ids.insert(edge.id.as_str()) {
                return Err(invalid(format!("duplicate Mindmap edge id `{}`", edge.id)));
            }
            if !node_ids.contains(edge.start.as_str()) || !node_ids.contains(edge.end.as_str()) {
                return Err(invalid(format!(
                    "Mindmap edge `{}` references a missing endpoint",
                    edge.id
                )));
            }
            let layout_edge = self
                .layout_edges_by_id
                .get(edge.id.as_str())
                .ok_or_else(|| invalid(format!("Mindmap edge `{}` has no layout edge", edge.id)))?;
            if layout_edge
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
            {
                return Err(invalid(format!(
                    "Mindmap edge `{}` has non-finite layout points",
                    edge.id
                )));
            }
            ensure_edge_classes(&edge.classes, &edge.id)?;
            if !edge.curve.trim().is_empty()
                && FlowchartCurveKind::from_mermaid_name(&edge.curve).is_none()
            {
                return Err(unavailable(format!(
                    "Mindmap edge `{}` curve `{}` has no typed v1 adapter",
                    edge.id, edge.curve
                )));
            }
            if !edge.thickness.trim().is_empty()
                && !matches!(edge.thickness.trim(), "normal" | "thick")
            {
                return Err(unavailable(format!(
                    "Mindmap edge `{}` thickness `{}` has no typed v1 adapter",
                    edge.id, edge.thickness
                )));
            }
        }
        if self.layout.edges.len() != self.model.edges.len() {
            return Err(invalid(format!(
                "Mindmap layout has {} edges for {} semantic edges",
                self.layout.edges.len(),
                self.model.edges.len()
            )));
        }
        let mut layout_edge_ids = HashSet::with_capacity(self.layout.edges.len());
        for edge in &self.layout.edges {
            if !layout_edge_ids.insert(edge.id.as_str()) || !edge_ids.contains(edge.id.as_str()) {
                return Err(invalid(format!(
                    "Mindmap layout contains an unknown or duplicate edge `{}`",
                    edge.id
                )));
            }
        }
        Ok(())
    }

    fn plain_label(&self, node: &MindmapDiagramRenderNode) -> Result<String> {
        let label_type = if node.label_type.trim().is_empty() {
            "markdown"
        } else {
            node.label_type.trim()
        };
        if !matches!(label_type, "markdown" | "text" | "string") {
            return Err(unavailable(format!(
                "Mindmap node `{}` label type `{label_type}` has no portable text mapping",
                node.id
            )));
        }

        let normalized = normalize_mindmap_breaks(&node.label);
        let analysis = crate::text::analyze_mermaid_markdown(&normalized, true);
        if analysis.has_styled_runs
            || crate::text::mermaid_markdown_contains_raw_blocks(&normalized)
            || crate::text::mermaid_markdown_contains_html_tags(&normalized)
            || normalized.contains('`')
            || normalized.contains("](")
            || normalized.contains("![")
        {
            return Err(unavailable(format!(
                "Mindmap node `{}` label uses Markdown/HTML styling that DrawingList v1 cannot preserve as a single host text run",
                node.id
            )));
        }
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(&normalized);
        Ok(flowchart_label_plain_text_for_layout(
            decoded.as_ref(),
            label_type,
            true,
        ))
    }

    fn emit_edge(&mut self, index: usize, edge: &MindmapDiagramRenderEdge) -> Result<()> {
        let source = self
            .layout_nodes_by_id
            .get(edge.start.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("Mindmap edge `{}` start is not laid out", edge.id)))?;
        let target = self
            .layout_nodes_by_id
            .get(edge.end.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("Mindmap edge `{}` end is not laid out", edge.id)))?;

        let points = mindmap_edge_points(source, target);
        let curve = if edge.curve.trim().is_empty() {
            FlowchartCurveKind::Basis
        } else {
            FlowchartCurveKind::from_mermaid_name(&edge.curve)
                .ok_or_else(|| unavailable(format!("edge curve `{}` is unsupported", edge.curve)))?
        };
        let segments = flowchart_curve_segments(&points, curve, 0.0, false, None);
        let (stroke, width) = self.edge_paint(edge)?;
        let semantic_id = format!("mindmap.edge.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.add_path(
            format!("{semantic_id}.route"),
            segments,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(StrokeStyle {
                    paint: Paint::solid(stroke),
                    width,
                    dash_array: Vec::new(),
                    dash_offset: 0.0,
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Miter,
                    miter_limit: 4.0,
                }),
            },
        )?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: Some(edge.id.clone()),
            description: Some(format!("{} → {}", edge.start, edge.end)),
            link: None,
        });
        Ok(())
    }

    fn emit_node(&mut self, index: usize, node: &MindmapDiagramRenderNode) -> Result<()> {
        let layout_node = self
            .layout_nodes_by_id
            .get(node.id.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("Mindmap node `{}` is not laid out", node.id)))?;
        let semantic_id = format!("mindmap.node.{index}");
        let style = self.node_style(index, node, layout_node)?;
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        let shape_paths = mindmap_node_paths(node, layout_node)?;
        let stroke_paint = if let Some((gradient_id, gradient)) = style.gradient {
            self.resources
                .push(DrawingResource::LinearGradient(gradient));
            Paint::resource(gradient_id)
        } else {
            Paint::solid(style.stroke)
        };
        for (part, segments) in shape_paths {
            self.add_path(
                format!("{semantic_id}.shape.{part}"),
                segments,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(style.fill)),
                    stroke: Some(StrokeStyle {
                        paint: stroke_paint.clone(),
                        width: style.stroke_width,
                        dash_array: Vec::new(),
                        dash_offset: 0.0,
                        line_cap: LineCap::Butt,
                        line_join: LineJoin::Miter,
                        miter_limit: 4.0,
                    }),
                },
            )?;
        }
        if node.shape.is_empty() || node.shape == "defaultMindmapNode" {
            let divider = vec![
                PathSegment::MoveTo {
                    to: Point::new(
                        layout_node.x - layout_node.width / 2.0,
                        layout_node.y + layout_node.height / 2.0,
                    ),
                },
                PathSegment::LineTo {
                    to: Point::new(
                        layout_node.x + layout_node.width / 2.0,
                        layout_node.y + layout_node.height / 2.0,
                    ),
                },
            ];
            self.add_path(
                format!("{semantic_id}.divider"),
                divider,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(StrokeStyle {
                        paint: Paint::solid(style.divider),
                        width: 3.0,
                        dash_array: Vec::new(),
                        dash_offset: 0.0,
                        line_cap: LineCap::Butt,
                        line_join: LineJoin::Miter,
                        miter_limit: 4.0,
                    }),
                },
            )?;
        }

        let text = self.plain_label(node)?;
        if !text.is_empty() {
            let bounds = mindmap_label_bounds(node, layout_node);
            self.commands.push(DrawingCommand::DrawText {
                run: TextRun {
                    text,
                    origin: Point::new(layout_node.x, layout_node.y),
                    bounds,
                    style: TextStyle {
                        font: self.font.clone(),
                        font_size: 16.0,
                        letter_spacing: 0.0,
                        line_height: 24.0,
                        fill: Paint::solid(style.text),
                    },
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                    direction: TextDirection::Auto,
                    language: None,
                    obligation: self.text_obligation.clone(),
                },
            });
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(text_for_semantic(node)),
            description: (!node.node_id.is_empty()).then(|| node.node_id.clone()),
            link: None,
        });
        Ok(())
    }

    fn node_style(
        &self,
        index: usize,
        node: &MindmapDiagramRenderNode,
        layout: &LayoutNode,
    ) -> Result<NodeStyle> {
        let palette_index = mindmap_palette_index(node);
        let c_scale = self.palette_color(palette_index, false)?;
        let c_scale_inv = self.palette_color(palette_index, true)?;
        let c_scale_label = self.palette_label(palette_index)?;
        let is_root = node.level == 0
            || node
                .css_classes
                .split_whitespace()
                .any(|class| class == "section-root");
        let redux = self.theme.eq_ignore_ascii_case("redux")
            || self.theme.eq_ignore_ascii_case("redux-dark");
        let neutral = self.theme.eq_ignore_ascii_case("neutral");

        let (fill, stroke, text) = if is_root {
            let fill = if self.look == "neo" && redux {
                self.main_bkg
            } else {
                self.root_fill
            };
            let text = if self.look == "neo" && (redux || neutral) {
                self.node_border
            } else if self.look == "neo" {
                self.palette_label(if neutral { 1 } else { 0 })?
            } else {
                self.root_text
            };
            (
                fill,
                if self.look == "neo" && !redux {
                    c_scale
                } else {
                    self.node_border
                },
                text,
            )
        } else if self.look == "neo" {
            let fill = if redux || neutral {
                self.main_bkg
            } else {
                c_scale
            };
            let stroke = if redux { self.node_border } else { c_scale };
            let text = if redux {
                self.node_border
            } else {
                self.palette_label(if neutral { 1 } else { palette_index })?
            };
            (fill, stroke, text)
        } else {
            (c_scale, self.node_border, c_scale_label)
        };

        let gradient = if self.use_gradient {
            let start = self.gradient_start.ok_or_else(|| {
                unavailable("Mindmap gradientStart is required when useGradient is enabled")
            })?;
            let stop = self.gradient_stop.ok_or_else(|| {
                unavailable("Mindmap gradientStop is required when useGradient is enabled")
            })?;
            let id = ResourceId::new(format!("mindmap.gradient.{index}"));
            Some((
                id.clone(),
                LinearGradientResource {
                    id,
                    start: Point::new(layout.x - layout.width / 2.0, layout.y),
                    end: Point::new(layout.x + layout.width / 2.0, layout.y),
                    transform: Transform::IDENTITY,
                    spread: GradientSpread::Pad,
                    stops: vec![GradientStop::new(0.0, start), GradientStop::new(1.0, stop)],
                },
            ))
        } else {
            None
        };

        Ok(NodeStyle {
            fill,
            stroke,
            text,
            divider: c_scale_inv,
            stroke_width: self.stroke_width,
            gradient,
        })
    }

    fn edge_paint(&self, edge: &MindmapDiagramRenderEdge) -> Result<(Color, f64)> {
        let index = edge
            .section
            .map(|section| section.rem_euclid(11) as usize + 1)
            .unwrap_or(0);
        let c_scale = self.palette_color(index, false)?;
        let redux = self.theme.eq_ignore_ascii_case("redux")
            || self.theme.eq_ignore_ascii_case("redux-dark");
        let stroke = if self.look == "neo" && (redux || self.theme.eq_ignore_ascii_case("neo-dark"))
        {
            self.node_border
        } else {
            c_scale
        };
        let width = if index >= self.theme_color_limit {
            3.0
        } else {
            (17.0 - 3.0 * index as f64).max(0.0)
        };
        Ok((stroke, width))
    }

    fn palette_color(&self, index: usize, inverse: bool) -> Result<Color> {
        let key = if inverse {
            format!("cScaleInv{index}")
        } else {
            format!("cScale{index}")
        };
        let fallback = if inverse {
            DEFAULT_MINDMAP_INV_FILLS[index % DEFAULT_MINDMAP_INV_FILLS.len()]
        } else {
            DEFAULT_MINDMAP_FILLS[index % DEFAULT_MINDMAP_FILLS.len()]
        };
        theme_color(self.metadata.effective_config.as_value(), &key, fallback)
    }

    fn palette_label(&self, index: usize) -> Result<Color> {
        let key = format!("cScaleLabel{index}");
        let fallback = DEFAULT_MINDMAP_LABELS[index % DEFAULT_MINDMAP_LABELS.len()];
        theme_color(self.metadata.effective_config.as_value(), &key, fallback)
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!("Mindmap path `{id}` has no geometry")));
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

#[derive(Debug, Clone)]
struct NodeStyle {
    fill: Color,
    stroke: Color,
    text: Color,
    divider: Color,
    stroke_width: f64,
    gradient: Option<(ResourceId, LinearGradientResource)>,
}

fn effective_look(model: &MindmapDiagramRenderModel, metadata: &ParseMetadata) -> String {
    let config_look = config_diagram_look(metadata.effective_config.as_value())
        .as_str()
        .to_string();
    model
        .nodes
        .iter()
        .map(|node| node.look.trim())
        .find(|look| !look.is_empty() && !look.eq_ignore_ascii_case("default"))
        .map_or(config_look, str::to_string)
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("Mindmap root bounds are invalid"));
    }
    Ok(())
}

fn validate_layout_node(node: &LayoutNode) -> Result<()> {
    if ![node.x, node.y, node.width, node.height]
        .into_iter()
        .all(f64::is_finite)
        || node.width < 0.0
        || node.height < 0.0
        || node.is_cluster
    {
        return Err(invalid(format!(
            "Mindmap layout node `{}` is invalid",
            node.id
        )));
    }
    Ok(())
}

fn ensure_node_classes(classes: &str, id: &str) -> Result<()> {
    for class in classes.split_whitespace() {
        let known = class == "mindmap-node"
            || class == "section-root"
            || class == "section--1"
            || class
                .strip_prefix("section-")
                .and_then(|value| value.parse::<i32>().ok())
                .is_some();
        if !known {
            return Err(unavailable(format!(
                "Mindmap node `{id}` class `{class}` has no portable style mapping"
            )));
        }
    }
    Ok(())
}

fn ensure_edge_classes(classes: &str, id: &str) -> Result<()> {
    for class in classes.split_whitespace() {
        let known = class == "edge"
            || class
                .strip_prefix("section-edge-")
                .and_then(|value| value.parse::<i32>().ok())
                .is_some()
            || class
                .strip_prefix("edge-depth-")
                .and_then(|value| value.parse::<i32>().ok())
                .is_some()
            || class.starts_with("edge-thickness-");
        if !known {
            return Err(unavailable(format!(
                "Mindmap edge `{id}` class `{class}` has no portable style mapping"
            )));
        }
    }
    Ok(())
}

fn mindmap_palette_index(node: &MindmapDiagramRenderNode) -> usize {
    if node.level == 0
        || node
            .css_classes
            .split_whitespace()
            .any(|class| class == "section-root")
    {
        0
    } else {
        node.section
            .map(|section| section.rem_euclid(11) as usize + 1)
            .unwrap_or(0)
    }
}

fn mindmap_edge_points(source: &LayoutNode, target: &LayoutNode) -> Vec<LayoutPoint> {
    let dx = target.x - source.x;
    let dy = target.y - source.y;
    let length = dx.hypot(dy);
    let (ux, uy) = if length > 0.0 {
        (dx / length, dy / length)
    } else {
        (0.0, 0.0)
    };
    let start = LayoutPoint {
        x: source.x + 15.0 * ux,
        y: source.y + 15.0 * uy,
    };
    let end = LayoutPoint {
        x: target.x - 15.0 * ux,
        y: target.y - 15.0 * uy,
    };
    vec![
        start.clone(),
        LayoutPoint {
            x: (start.x + end.x) / 2.0,
            y: (start.y + end.y) / 2.0,
        },
        end,
    ]
}

fn mindmap_label_bounds(node: &MindmapDiagramRenderNode, layout: &LayoutNode) -> Rect {
    let padding = if node.shape == "rounded" {
        15.0
    } else {
        node.padding.max(0.0)
    };
    let half_padding = padding / 2.0;
    let (width, height) = match node.shape.as_str() {
        "defaultMindmapNode" | "" => (
            (layout.width - 8.0 * half_padding).max(1.0),
            (layout.height - 2.0 * half_padding).max(1.0),
        ),
        "rect" => (
            (layout.width - 2.0 * padding).max(1.0),
            (layout.height - padding).max(1.0),
        ),
        "rounded" | "mindmapCircle" => (
            layout
                .label_width
                .unwrap_or_else(|| (layout.width - 2.0 * padding).max(1.0))
                .max(1.0),
            layout
                .label_height
                .unwrap_or_else(|| (layout.height - 2.0 * padding).max(1.0))
                .max(1.0),
        ),
        "cloud" => (
            layout
                .label_width
                .unwrap_or_else(|| (layout.width - 2.0 * half_padding).max(1.0))
                .max(1.0),
            layout
                .label_height
                .unwrap_or_else(|| (layout.height - 2.0 * half_padding).max(1.0))
                .max(1.0),
        ),
        "bang" => (
            (layout.width - 10.0 * half_padding).max(1.0),
            (layout.height - 8.0 * half_padding).max(1.0),
        ),
        "hexagon" => (
            layout.label_width.unwrap_or(layout.width).max(1.0),
            layout.label_height.unwrap_or(layout.height).max(1.0),
        ),
        _ => (layout.width.max(1.0), layout.height.max(1.0)),
    };
    Rect::new(
        layout.x - width / 2.0,
        layout.y - height / 2.0,
        width,
        height,
    )
}

fn mindmap_node_paths(
    node: &MindmapDiagramRenderNode,
    layout: &LayoutNode,
) -> Result<Vec<(String, Vec<PathSegment>)>> {
    let width = layout.width.max(1.0);
    let height = layout.height.max(1.0);
    let shape = if node.shape.is_empty() {
        "defaultMindmapNode"
    } else {
        node.shape.as_str()
    };
    let paths = match shape {
        "defaultMindmapNode" => vec![(
            "outer".to_string(),
            translate_path(
                parse_svg_path(&default_mindmap_path_d(width, height))?,
                layout.x,
                layout.y,
            ),
        )],
        "rect" => vec![(
            "outer".to_string(),
            super::flowchart::rectangle_path(layout.x, layout.y, width, height, 0.0)
                .expect("validated rectangle geometry"),
        )],
        "rounded" => vec![(
            "outer".to_string(),
            super::flowchart::rounded_rect_path(layout.x, layout.y, width, height, 5.0),
        )],
        "mindmapCircle" => vec![(
            "outer".to_string(),
            super::flowchart::ellipse_path(
                layout.x,
                layout.y,
                (width.max(height) / 2.0).max(1.0),
                (width.max(height) / 2.0).max(1.0),
            ),
        )],
        "cloud" => vec![("outer".to_string(), {
            let padding = node.padding.max(0.0);
            let label_width = layout
                .label_width
                .unwrap_or_else(|| (width - padding).max(1.0));
            let label_height = layout
                .label_height
                .unwrap_or_else(|| (height - padding).max(1.0));
            let shape_width = (label_width + padding).max(1.0);
            let shape_height = (label_height + padding).max(1.0);
            translate_path(
                parse_svg_path(&mindmap_cloud_path_d(shape_width, shape_height))?,
                layout.x - shape_width / 2.0,
                layout.y - shape_height / 2.0,
            )
        })],
        "bang" => vec![("outer".to_string(), {
            let padding = node.padding.max(0.0);
            let label_width = layout
                .label_width
                .unwrap_or_else(|| (width - 5.0 * padding).max(1.0));
            let w_base = label_width + 5.0 * padding;
            translate_path(
                parse_svg_path(&mindmap_bang_path_d(w_base, width, height))?,
                layout.x - width / 2.0,
                layout.y - height / 2.0,
            )
        })],
        "hexagon" => {
            let inset = height / 4.0;
            vec![(
                "outer".to_string(),
                super::flowchart::polygon_path(&[
                    Point::new(layout.x - width / 2.0 + inset, layout.y - height / 2.0),
                    Point::new(layout.x + width / 2.0 - inset, layout.y - height / 2.0),
                    Point::new(layout.x + width / 2.0, layout.y),
                    Point::new(layout.x + width / 2.0 - inset, layout.y + height / 2.0),
                    Point::new(layout.x - width / 2.0 + inset, layout.y + height / 2.0),
                    Point::new(layout.x - width / 2.0, layout.y),
                ]),
            )]
        }
        other => {
            return Err(unavailable(format!(
                "Mindmap shape `{other}` has no typed v1 adapter"
            )));
        }
    };
    Ok(paths)
}

fn default_mindmap_path_d(width: f64, height: f64) -> String {
    let radius = 5.0_f64.min(width / 2.0).min(height / 2.0);
    let left = -width / 2.0;
    let bottom = height / 2.0;
    format!(
        "M{left} {} v{} q0,-{radius} {radius},-{radius} h{} q{radius},0 {radius},{radius} v{} q0,{radius} -{radius},{radius} h{} q-{radius},0 -{radius},-{radius} Z",
        bottom - radius,
        -height + 2.0 * radius,
        width - 2.0 * radius,
        height - 2.0 * radius,
        -width + 2.0 * radius,
    )
}

fn mindmap_cloud_path_d(width: f64, height: f64) -> String {
    let r1 = 0.15 * width;
    let r2 = 0.25 * width;
    let r3 = 0.35 * width;
    let r4 = 0.2 * width;
    format!(
        "M0 0 a{r1},{r1} 0 0,1 {w25},{wn10} a{r3},{r3} 1 0,1 {w40},{wn10} a{r2},{r2} 1 0,1 {w35},{w20} a{r1},{r1} 1 0,1 {w15},{h35} a{r4},{r4} 1 0,1 {wn15},{h65} a{r2},{r1} 1 0,1 {wn25},{w15} a{r3},{r3} 1 0,1 {wn50},0 a{r1},{r1} 1 0,1 {wn25},{wn15} a{r1},{r1} 1 0,1 {wn10},{hn35} a{r4},{r4} 1 0,1 {w10},{hn65} H0 V0 Z",
        w25 = width * 0.25,
        w40 = width * 0.4,
        w35 = width * 0.35,
        w20 = width * 0.2,
        w15 = width * 0.15,
        w10 = width * 0.1,
        wn10 = -width * 0.1,
        wn15 = -width * 0.15,
        wn25 = -width * 0.25,
        wn50 = -width * 0.5,
        h35 = height * 0.35,
        h65 = height * 0.65,
        hn35 = -height * 0.35,
        hn65 = -height * 0.65,
    )
}

fn mindmap_bang_path_d(w_base: f64, effective_width: f64, effective_height: f64) -> String {
    let radius = 0.15 * w_base;
    format!(
        "M0 0 a{radius},{radius} 1 0,0 {w25},{hn10} a{radius},{radius} 1 0,0 {w25},0 a{radius},{radius} 1 0,0 {w25},0 a{radius},{radius} 1 0,0 {w25},{h10} a{radius},{radius} 1 0,0 {w15},{h33} a{r08},{r08} 1 0,0 0,{h34} a{radius},{radius} 1 0,0 {wn15},{h33} a{radius},{radius} 1 0,0 {wn25},{h15} a{radius},{radius} 1 0,0 {wn25},0 a{radius},{radius} 1 0,0 {wn25},0 a{radius},{radius} 1 0,0 {wn25},{hn15} a{radius},{radius} 1 0,0 {wn10},{hn33} a{r08},{r08} 1 0,0 0,{hn34} a{radius},{radius} 1 0,0 {w10},{hn33} H0 V0 Z",
        r08 = radius * 0.8,
        w25 = effective_width * 0.25,
        w15 = effective_width * 0.15,
        w10 = effective_width * 0.1,
        wn10 = -effective_width * 0.1,
        wn15 = -effective_width * 0.15,
        wn25 = -effective_width * 0.25,
        h10 = effective_height * 0.1,
        hn10 = -effective_height * 0.1,
        h15 = effective_height * 0.15,
        hn15 = -effective_height * 0.15,
        h33 = effective_height * 0.33,
        hn33 = -effective_height * 0.33,
        h34 = effective_height * 0.34,
        hn34 = -effective_height * 0.34,
    )
}

fn translate_path(segments: Vec<PathSegment>, dx: f64, dy: f64) -> Vec<PathSegment> {
    segments
        .into_iter()
        .map(|segment| match segment {
            PathSegment::MoveTo { to } => PathSegment::MoveTo {
                to: Point::new(to.x + dx, to.y + dy),
            },
            PathSegment::LineTo { to } => PathSegment::LineTo {
                to: Point::new(to.x + dx, to.y + dy),
            },
            PathSegment::QuadTo { control, to } => PathSegment::QuadTo {
                control: Point::new(control.x + dx, control.y + dy),
                to: Point::new(to.x + dx, to.y + dy),
            },
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => PathSegment::CubicTo {
                control1: Point::new(control1.x + dx, control1.y + dy),
                control2: Point::new(control2.x + dx, control2.y + dy),
                to: Point::new(to.x + dx, to.y + dy),
            },
            PathSegment::ArcTo {
                radius_x,
                radius_y,
                x_axis_rotation_degrees,
                large_arc,
                sweep_clockwise,
                to,
            } => PathSegment::ArcTo {
                radius_x,
                radius_y,
                x_axis_rotation_degrees,
                large_arc,
                sweep_clockwise,
                to: Point::new(to.x + dx, to.y + dy),
            },
            PathSegment::Close => PathSegment::Close,
        })
        .collect()
}

fn normalize_mindmap_breaks(value: &str) -> String {
    value
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n")
        .replace("</br>", "\n")
}

fn text_for_semantic(node: &MindmapDiagramRenderNode) -> String {
    let normalized = normalize_mindmap_breaks(&node.label);
    let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(&normalized);
    flowchart_label_plain_text_for_layout(
        decoded.as_ref(),
        if node.label_type.is_empty() {
            "markdown"
        } else {
            node.label_type.as_str()
        },
        true,
    )
}

fn text_obligation(session: &RenderSession) -> TextObligation {
    let route = session.text_measurement_route(TextMeasurementPhase::Layout);
    let profile = profile_identity(&route.primary);
    let measurement = match route.primary_source {
        TextMeasurementSource::Host => MeasurementProvenance::HostCallback { profile },
        TextMeasurementSource::Profile => MeasurementProvenance::DeterministicFallback { profile },
    };
    TextObligation::HostText { measurement }
}

fn profile_identity(identity: &crate::environment::TextMeasurementProfileIdentity) -> String {
    let mut value = format!("{}@{}", identity.profile().as_str(), identity.version());
    for decorator in identity.decorators() {
        value.push('+');
        value.push_str(decorator);
    }
    value
}

fn config_css_number(config: &Value, path: &[&str]) -> Option<f64> {
    let value = crate::config::value_at(config, path)?;
    let raw = value.as_f64().or_else(|| {
        let text = value.as_str()?.trim();
        text.strip_suffix("px").unwrap_or(text).trim().parse().ok()
    })?;
    raw.is_finite().then_some(raw)
}

fn color_css_fallback(color: Color) -> String {
    format!(
        "rgba({}, {}, {}, {})",
        color.red,
        color.green,
        color.blue,
        f64::from(color.alpha) / 255.0
    )
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "mindmap".to_string(),
        reason: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mindmap_edge_points_match_svg_endpoint_offset() {
        let source = LayoutNode {
            id: "a".to_string(),
            x: 10.0,
            y: 20.0,
            width: 20.0,
            height: 10.0,
            is_cluster: false,
            label_width: None,
            label_height: None,
        };
        let target = LayoutNode {
            id: "b".to_string(),
            x: 110.0,
            y: 20.0,
            width: 20.0,
            height: 10.0,
            is_cluster: false,
            label_width: None,
            label_height: None,
        };
        let points = mindmap_edge_points(&source, &target);
        assert_eq!(points[0].x, 25.0);
        assert_eq!(points[1].x, 60.0);
        assert_eq!(points[2].x, 95.0);
    }

    #[test]
    fn default_mindmap_path_is_closed() {
        let segments = parse_svg_path(&default_mindmap_path_d(100.0, 40.0)).unwrap();
        assert!(matches!(segments.first(), Some(PathSegment::MoveTo { .. })));
        assert!(matches!(segments.last(), Some(PathSegment::Close)));
    }
}
