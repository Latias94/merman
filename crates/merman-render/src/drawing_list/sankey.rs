//! Renderer-neutral Sankey adapter.
//!
//! Sankey geometry, labels, palette assignment, and link paints come from the family-owned
//! visual plan shared with SVG. The adapter only resolves portable CSS tokens and projects that
//! plan into the public command/resource contract.
//! SVG's `shape-rendering: crispEdges` remains an SVG rasterization hint in the private sidecar;
//! DrawingList owns the exact node geometry and paint, while host rasterization stays backend-local.

use super::{
    RenderDocument, SankeySvgBody, SvgStructureBody, SvgStructureSidecar, parse_font_families_for,
};
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::support::{PortableStyleResolver, svg_plain_text, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::SankeyDiagramLayout;
use crate::sankey::{
    SANKEY_LABEL_ASCENT_EM, SANKEY_LABEL_DESCENT_EM, SANKEY_LABEL_FONT_SIZE_PX, SankeyLabelAnchor,
    SankeyVisualLink, SankeyVisualLinkPaint, SankeyVisualPlan, build_sankey_visual_plan,
};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::sankey::SankeyDiagramRenderModel;
use merman_display_list::{
    BlendMode, Color, DrawingCommand, DrawingListLimits, DrawingListPolicy, FillRule,
    FontDescriptor, FontStyle, GradientSpread, GradientStop, LineCap, LineJoin,
    LinearGradientResource, Paint, PathSegment, PathStyle, Point, Rect, ResourceId,
    SemanticAnnotation, SemanticRole, StrokeStyle, TextAnchor, TextBaseline, TextDirection,
    TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type SankeyPair = FamilyPair<SankeyDiagramRenderModel, SankeyDiagramLayout>;

pub(crate) fn build_sankey_document(
    pair: &SankeyPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: DrawingListLimits,
    session: &RenderSession,
) -> Result<RenderDocument> {
    SankeyBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct SankeyBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    plan: SankeyVisualPlan,
    font: FontDescriptor,
    text_color: Color,
    text_obligation: TextObligation,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    output: DrawingListBuilder<'a>,
}

impl<'a> SankeyBuilder<'a> {
    fn new(
        pair: &SankeyPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let plan = build_sankey_visual_plan(pair.layout(), config)?;
        if plan.outlined_labels {
            return Err(unavailable(
                "outlined Sankey labels require text stroke and paint-order semantics that DrawingList v1 does not yet expose",
            ));
        }
        let styles = PortableStyleResolver::new("sankey");
        let text_color = styles.color("themeVariables.textColor", &plan.theme.text_color)?;
        let font_family_css = crate::config::config_font_family_css_raw(config);
        let font = FontDescriptor {
            families: parse_font_families_for(&font_family_css, RenderFamilyKind::Sankey)?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        let mut output = DrawingListBuilder::new(policy, limits, session);
        output.push_control(DrawingCommand::Save)?;
        output.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "sankey.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            plan,
            font,
            text_color,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            output,
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.output.push_semantic(SemanticAnnotation {
            id: "sankey.document".to_string(),
            role: SemanticRole::Document,
            title: self.metadata.title.clone(),
            description: None,
            link: None,
        })?;

        for node_index in 0..self.plan.nodes.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_node(node_index)?;
        }
        for link_index in 0..self.plan.links.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_link(link_index)?;
        }

        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_control(DrawingCommand::Restore)?;
        let bounds = &self.plan.bounds;
        let viewport = Viewport::new(Rect::new(
            bounds.min_x,
            bounds.min_y,
            bounds.max_x - bounds.min_x,
            bounds.max_y - bounds.min_y,
        ));
        let extensions = BTreeMap::from([(
            "x-merman-sankey".to_string(),
            json!({
                "diagram_type": self.metadata.diagram_type,
                "label_style": "legacy",
                "link_color": self.plan.link_color,
                "show_values": self.plan.show_values,
                "text_mode": "plain_host_text",
            }),
        )]);
        let document = self.output.finish(viewport, extensions)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Sankey,
                body: SvgStructureBody::Sankey(SankeySvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.plan.use_max_width,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                }),
            },
        })
    }

    fn emit_node(&mut self, node_index: usize) -> Result<()> {
        let node = self
            .plan
            .nodes
            .get(node_index)
            .ok_or_else(|| invalid(format!("missing Sankey visual node {node_index}")))?
            .clone();
        let semantic_id = format!("sankey.node.{node_index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "node".to_string());
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        let fill = PortableStyleResolver::new("sankey").optional_color("nodeColors", &node.fill)?;
        if let Some(fill) = fill {
            let path_id = ResourceId::new(format!("{semantic_id}.shape"));
            self.path_classes
                .insert(path_id.as_str().to_string(), "node-shape".to_string());
            let style = PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(fill)),
                stroke: None,
            };
            let top = node.y;
            let bottom = node.y + node.height;
            let left = node.x;
            let right = node.x + node.width;
            self.output.draw_path_with(path_id, style, |emit| {
                emit(PathSegment::MoveTo {
                    to: Point::new(left, top),
                })?;
                emit(PathSegment::LineTo {
                    to: Point::new(right, top),
                })?;
                emit(PathSegment::LineTo {
                    to: Point::new(right, bottom),
                })?;
                emit(PathSegment::LineTo {
                    to: Point::new(left, bottom),
                })?;
                emit(PathSegment::Close)
            })?;
        }
        for label_index in 0..self.plan.labels.len() {
            if self.plan.labels[label_index].node_index != node_index {
                continue;
            }
            let label = self.plan.labels[label_index].clone();
            self.emit_label_text(&label)?;
        }
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: {
                let title = svg_plain_text(&node.id);
                (!title.is_empty()).then_some(title)
            },
            description: Some(format!("Value {}", node.value)),
            link: None,
        })?;
        Ok(())
    }

    fn emit_label_text(&mut self, label: &crate::sankey::SankeyVisualLabel) -> Result<()> {
        let text = svg_plain_text(&label.text);
        if text.is_empty() {
            return Ok(());
        }
        let baseline_y = label.baseline_y();
        let ascent = SANKEY_LABEL_FONT_SIZE_PX * SANKEY_LABEL_ASCENT_EM;
        let descent = SANKEY_LABEL_FONT_SIZE_PX * SANKEY_LABEL_DESCENT_EM;
        let bounds = &self.plan.bounds;
        let origin = Point::new(label.x, baseline_y);
        let text_bounds = Rect::new(
            bounds.min_x,
            baseline_y - ascent,
            bounds.max_x - bounds.min_x,
            ascent + descent,
        );
        let font = self.font.clone();
        let text_color = self.text_color;
        let text_obligation = self.text_obligation.clone();
        let anchor = match label.anchor {
            SankeyLabelAnchor::Start => TextAnchor::Start,
            SankeyLabelAnchor::End => TextAnchor::End,
        };
        self.output.draw_host_text(&text, |text| TextRun {
            text,
            origin,
            bounds: text_bounds,
            style: TextStyle {
                font,
                font_size: SANKEY_LABEL_FONT_SIZE_PX,
                letter_spacing: 0.0,
                line_height: SANKEY_LABEL_FONT_SIZE_PX,
                fill: Paint::solid(text_color),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor,
            baseline: TextBaseline::Alphabetic,
            direction: TextDirection::Auto,
            language: None,
            obligation: text_obligation,
        })
    }

    fn emit_link(&mut self, link_index: usize) -> Result<()> {
        let link = self
            .plan
            .links
            .get(link_index)
            .ok_or_else(|| invalid(format!("missing Sankey visual link {link_index}")))?
            .clone();
        let semantic_id = format!("sankey.link.{}", link.index);
        self.semantic_classes
            .insert(semantic_id.clone(), "link".to_string());
        let paint = self.link_paint(&semantic_id, &link)?;
        if let Some(paint) = paint {
            let path_id = ResourceId::new(format!("{semantic_id}.path"));
            self.path_classes
                .insert(path_id.as_str().to_string(), "link-path".to_string());
            self.output.push_control(DrawingCommand::Save)?;
            self.output
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.output
                .push_control(DrawingCommand::SetOpacity { opacity: 0.5 })?;
            self.output.push_control(DrawingCommand::SetBlendMode {
                blend_mode: BlendMode::Multiply,
            })?;
            let style = PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke_with_paint(paint, link.width)),
            };
            self.output.draw_path_with(path_id, style, |emit| {
                emit(PathSegment::MoveTo {
                    to: Point::new(link.start_x, link.start_y),
                })?;
                emit(PathSegment::CubicTo {
                    control1: Point::new(link.control_x, link.start_y),
                    control2: Point::new(link.control_x, link.end_y),
                    to: Point::new(link.end_x, link.end_y),
                })
            })?;
            self.output.push_control(DrawingCommand::EndSemanticGroup)?;
            self.output.push_control(DrawingCommand::Restore)?;
        }
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: Some(format!("{} → {}", link.source, link.target)),
            description: Some(format!("Value {}", link.value)),
            link: None,
        })?;
        Ok(())
    }

    fn link_paint(&mut self, semantic_id: &str, link: &SankeyVisualLink) -> Result<Option<Paint>> {
        let styles = PortableStyleResolver::new("sankey");
        match &link.paint {
            SankeyVisualLinkPaint::Solid(color) => {
                Ok(styles.optional_color("linkColor", color)?.map(Paint::solid))
            }
            SankeyVisualLinkPaint::LinearGradient {
                start_color,
                end_color,
            } => {
                let start = styles.color("gradient source node color", start_color)?;
                let end = styles.color("gradient target node color", end_color)?;
                let id = ResourceId::new(format!("{semantic_id}.paint"));
                self.output.push_linear_gradient(LinearGradientResource {
                    id: id.clone(),
                    start: Point::new(link.start_x, 0.0),
                    end: Point::new(link.end_x, 0.0),
                    transform: Transform::IDENTITY,
                    spread: GradientSpread::Pad,
                    stops: vec![GradientStop::new(0.0, start), GradientStop::new(1.0, end)],
                })?;
                Ok(Some(Paint::resource(id)))
            }
        }
    }
}

fn stroke_with_paint(paint: Paint, width: f64) -> StrokeStyle {
    StrokeStyle {
        paint,
        width,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        miter_limit: 4.0,
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "sankey".to_string(),
        reason: message.into(),
    }
}
