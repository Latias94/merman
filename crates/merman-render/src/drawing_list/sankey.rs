//! Renderer-neutral Sankey adapter.
//!
//! Sankey geometry, labels, palette assignment, and link paints come from the family-owned
//! visual plan shared with SVG. The adapter only resolves portable CSS tokens and projects that
//! plan into the public command/resource contract.
//! SVG's `shape-rendering: crispEdges` remains an SVG rasterization hint in the private sidecar;
//! DrawingList owns the exact node geometry and paint, while host rasterization stays backend-local.

use super::{
    RenderDocument, SankeySvgBody, SvgStructureBody, SvgStructureSidecar, parse_font_families,
};
use crate::drawing_list::flowchart::polygon_path;
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
    BlendMode, Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, GradientSpread,
    GradientStop, LineCap, LineJoin, LinearGradientResource, Paint, PathResource, PathSegment,
    PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

type SankeyPair = FamilyPair<SankeyDiagramRenderModel, SankeyDiagramLayout>;

pub(crate) fn build_sankey_document(
    pair: &SankeyPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    SankeyBuilder::new(pair, metadata, policy, session)?.build()
}

struct SankeyBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    plan: SankeyVisualPlan,
    font: FontDescriptor,
    text_color: Color,
    text_obligation: TextObligation,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> SankeyBuilder<'a> {
    fn new(
        pair: &SankeyPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        if config
            .get("themeCSS")
            .and_then(Value::as_str)
            .is_some_and(|css| !css.trim().is_empty())
        {
            return Err(unavailable(
                "themeCSS is an unresolved SVG cascade input for Sankey DrawingList output",
            ));
        }

        let plan = build_sankey_visual_plan(pair.layout(), config)?;
        if plan.outlined_labels {
            return Err(unavailable(
                "outlined Sankey labels require text stroke and paint-order semantics that DrawingList v1 does not yet expose",
            ));
        }
        let styles = PortableStyleResolver::new("sankey");
        let text_color = styles.color("themeVariables.textColor", &plan.theme.text_color)?;
        let font = FontDescriptor {
            families: parse_font_families(plan.theme.font_family_css.clone()),
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        Ok(Self {
            metadata,
            session,
            policy,
            plan,
            font,
            text_color,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "sankey.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.semantics.push(SemanticAnnotation {
            id: "sankey.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .metadata
                .title
                .clone()
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: None,
            link: None,
        });

        for node_index in 0..self.plan.nodes.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_node(node_index)?;
        }
        for label_index in 0..self.plan.labels.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_label(label_index)?;
        }
        for link_index in 0..self.plan.links.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_link(link_index)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let bounds = &self.plan.bounds;
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
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-sankey".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "label_style": "legacy",
                    "link_color": self.plan.link_color,
                    "show_values": self.plan.show_values,
                    "text_mode": "plain_host_text",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Sankey,
                body: SvgStructureBody::Sankey(SankeySvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
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
        let fill = PortableStyleResolver::new("sankey").optional_color("nodeColors", &node.fill)?;
        if let Some(fill) = fill {
            let path_id = ResourceId::new(format!("{semantic_id}.shape"));
            self.resources.push(DrawingResource::Path(PathResource {
                id: path_id.clone(),
                segments: polygon_path(&[
                    Point::new(node.x, node.y),
                    Point::new(node.x + node.width, node.y),
                    Point::new(node.x + node.width, node.y + node.height),
                    Point::new(node.x, node.y + node.height),
                ]),
            }));
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.commands.push(DrawingCommand::DrawPath {
                path: path_id,
                style: PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: None,
                },
            });
            self.commands.push(DrawingCommand::EndSemanticGroup);
        }
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: {
                let title = svg_plain_text(&node.id);
                (!title.is_empty()).then_some(title)
            },
            description: Some(format!("Value {}", node.value)),
            link: None,
        });
        Ok(())
    }

    fn emit_label(&mut self, label_index: usize) -> Result<()> {
        let label = self
            .plan
            .labels
            .get(label_index)
            .ok_or_else(|| invalid(format!("missing Sankey visual label {label_index}")))?
            .clone();
        let text = svg_plain_text(&label.text);
        if text.is_empty() {
            return Ok(());
        }
        let semantic_id = format!("sankey.node.{}", label.node_index);
        let baseline_y = label.baseline_y();
        let ascent = SANKEY_LABEL_FONT_SIZE_PX * SANKEY_LABEL_ASCENT_EM;
        let descent = SANKEY_LABEL_FONT_SIZE_PX * SANKEY_LABEL_DESCENT_EM;
        let bounds = &self.plan.bounds;
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.commands.push(DrawingCommand::DrawText {
            run: TextRun {
                text,
                origin: Point::new(label.x, baseline_y),
                bounds: Rect::new(
                    bounds.min_x,
                    baseline_y - ascent,
                    bounds.max_x - bounds.min_x,
                    ascent + descent,
                ),
                style: TextStyle {
                    font: self.font.clone(),
                    font_size: SANKEY_LABEL_FONT_SIZE_PX,
                    letter_spacing: 0.0,
                    line_height: SANKEY_LABEL_FONT_SIZE_PX,
                    fill: Paint::solid(self.text_color),
                },
                anchor: match label.anchor {
                    SankeyLabelAnchor::Start => TextAnchor::Start,
                    SankeyLabelAnchor::End => TextAnchor::End,
                },
                baseline: TextBaseline::Alphabetic,
                direction: TextDirection::Auto,
                language: None,
                obligation: self.text_obligation.clone(),
            },
        });
        self.commands.push(DrawingCommand::EndSemanticGroup);
        Ok(())
    }

    fn emit_link(&mut self, link_index: usize) -> Result<()> {
        let link = self
            .plan
            .links
            .get(link_index)
            .ok_or_else(|| invalid(format!("missing Sankey visual link {link_index}")))?
            .clone();
        let semantic_id = format!("sankey.link.{}", link.index);
        let paint = self.link_paint(&semantic_id, &link)?;
        if let Some(paint) = paint {
            let path_id = ResourceId::new(format!("{semantic_id}.path"));
            self.resources.push(DrawingResource::Path(PathResource {
                id: path_id.clone(),
                segments: vec![
                    PathSegment::MoveTo {
                        to: Point::new(link.start_x, link.start_y),
                    },
                    PathSegment::CubicTo {
                        control1: Point::new(link.control_x, link.start_y),
                        control2: Point::new(link.control_x, link.end_y),
                        to: Point::new(link.end_x, link.end_y),
                    },
                ],
            }));
            self.commands.push(DrawingCommand::Save);
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.commands
                .push(DrawingCommand::SetOpacity { opacity: 0.5 });
            self.commands.push(DrawingCommand::SetBlendMode {
                blend_mode: BlendMode::Multiply,
            });
            self.commands.push(DrawingCommand::DrawPath {
                path: path_id,
                style: PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke_with_paint(paint, link.width)),
                },
            });
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.commands.push(DrawingCommand::Restore);
        }
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: Some(format!("{} → {}", link.source, link.target)),
            description: Some(format!("Value {}", link.value)),
            link: None,
        });
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
                self.resources
                    .push(DrawingResource::LinearGradient(LinearGradientResource {
                        id: id.clone(),
                        start: Point::new(link.start_x, 0.0),
                        end: Point::new(link.end_x, 0.0),
                        transform: Transform::IDENTITY,
                        spread: GradientSpread::Pad,
                        stops: vec![GradientStop::new(0.0, start), GradientStop::new(1.0, end)],
                    }));
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
