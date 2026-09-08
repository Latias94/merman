//! Renderer-neutral Sequence diagram adapter.
//!
//! Sequence layout already owns the final actor, lifeline, message, note, activation, and
//! control-block coordinates.  This adapter projects that typed result directly into the public
//! DrawingList contract.  The admitted subset is deliberately explicit: classic participant
//! shapes and plain host text are portable; HTML/math labels, custom participant shapes, lifecycle
//! rewrites, and SVG-only effects fail closed instead of being silently flattened.

use super::{
    RenderDocument, SequenceSvgBody, SvgStructureBody, SvgStructureSidecar,
    parse_font_families_for, theme_color,
};
use crate::config::{config_diagram_look, config_f64, config_string};
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::flowchart::{ellipse_path, polygon_path, rounded_rect_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, LayoutEdge, LayoutNode, SequenceBlockLayout, SequenceDiagramLayout};
use crate::sequence::{
    SequencePreparedArtifact, sequence_activation_start_x, sequence_text_dimensions_height_px,
    sequence_text_line_step_px,
};
use crate::text::{
    TextMeasurer as _, TextStyle as MeasurementTextStyle, mermaid_markdown_to_xhtml_label_fragment,
    mermaid_xhtml_label_plain_text, split_html_br_lines,
};
use crate::{Error, Result};
use merman_core::diagrams::sequence::{
    SequenceActor, SequenceControlKind, SequenceControlRole, SequenceDiagramRenderModel,
    SequenceMessage, SequenceMessageKind, SequenceMessageMarker, SequenceMessageStroke,
};
use merman_core::svg_security::{MermaidNavigationSecurity, prepare_mermaid_navigation_uri};
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, DrawingCommand, DrawingListLimits, DrawingListPolicy, FillRule, FontDescriptor,
    FontStyle, Paint, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun,
    TextStyle as DisplayTextStyle, Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

type SequencePair = FamilyPair<SequenceDiagramRenderModel, SequencePreparedArtifact>;
type ControlSection = (String, String, Option<f64>);
type ControlStackEntry = (usize, SequenceControlKind, String, Vec<ControlSection>);

const SIGNAL_WIDTH: f64 = 1.5;
const LIFELINE_WIDTH: f64 = 0.5;
const CONTROL_WIDTH: f64 = 2.0;
const CONTROL_DASH: [f64; 2] = [2.0, 2.0];
const CONTROL_SEPARATOR_DASH: [f64; 2] = [3.0, 3.0];
const LABEL_BOX_HEIGHT: f64 = 20.0;
const LABEL_BOX_ARROW_INSET: f64 = 8.4;
const CENTRAL_CONNECTION_CIRCLE_OFFSET: f64 = 16.5;
const VIEWPORT_PADDING: f64 = 8.0;

#[derive(Debug, Clone, Copy)]
struct SequenceSettings {
    mirror_actors: bool,
    right_angles: bool,
    diagram_margin_x: f64,
    box_margin: f64,
    box_text_margin: f64,
    label_box_width: f64,
    wrap_padding: f64,
    sequence_width: f64,
    activation_width: f64,
    font_size: f64,
    font_weight: u16,
    message_align: SequenceMessageAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceMessageAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy)]
struct SequencePalette {
    actor_fill: Color,
    actor_border: Color,
    actor_text: Color,
    actor_line: Color,
    signal: Color,
    signal_text: Color,
    sequence_number: Color,
    label_box_fill: Color,
    label_box_border: Color,
    label_text: Color,
    loop_text: Color,
    note_fill: Color,
    note_border: Color,
    note_text: Color,
    activation_fill: Color,
    activation_border: Color,
    node_border: Color,
    text: Color,
    stroke_width: f64,
}

struct TextLinesSpec {
    center: Point,
    font_size: f64,
    weight: u16,
    color: Color,
    font: FontDescriptor,
    anchor: TextAnchor,
}

#[derive(Debug, Clone)]
struct ActivationRect {
    start_id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    class_index: usize,
}

#[derive(Debug, Clone)]
struct ControlBlock {
    end_index: usize,
    kind: SequenceControlKind,
    start_id: String,
    layout: SequenceBlockLayout,
    sections: Vec<ControlSection>,
}

pub(crate) fn build_sequence_document(
    pair: &SequencePair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: DrawingListLimits,
    session: &RenderSession,
) -> Result<RenderDocument> {
    SequenceBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct SequenceBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    document: DrawingListBuilder<'a>,
    model: &'a SequenceDiagramRenderModel,
    layout: &'a SequenceDiagramLayout,
    settings: SequenceSettings,
    palette: SequencePalette,
    actor_font: FontDescriptor,
    message_font: FontDescriptor,
    note_font: FontDescriptor,
    text_obligation: TextObligation,
    nodes_by_id: HashMap<&'a str, &'a LayoutNode>,
    edges_by_id: HashMap<&'a str, &'a LayoutEdge>,
    activations: Vec<ActivationRect>,
    controls: Vec<ControlBlock>,
}

impl<'a> SequenceBuilder<'a> {
    fn new(
        pair: &'a SequencePair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let mut document = DrawingListBuilder::new(policy, limits, session);
        document.push_control(DrawingCommand::Save)?;
        document.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "sequence.document".to_string(),
        })?;
        let config = metadata.effective_config.as_value();
        if config_diagram_look(config).as_str() != "classic" {
            return Err(unavailable(
                "Sequence DrawingList v1 admits only the classic look; neo and hand-drawn effects remain SVG-specific",
            ));
        }
        let model = pair.semantic();
        let prepared = pair.layout();
        let layout = prepared.layout();
        let settings = sequence_settings(config)?;
        validate_model(model, layout, &settings)?;
        let nodes_by_id = unique_nodes(layout)?;
        let edges_by_id = unique_edges(layout)?;
        validate_geometry(model, layout, &nodes_by_id, &edges_by_id, &settings)?;

        let palette = sequence_palette(config)?;
        let font_family = config_string(config, &["fontFamily"])
            .filter(|family| !family.trim().is_empty())
            .unwrap_or_else(|| "\"trebuchet ms\", verdana, arial, sans-serif;".to_string());
        let actor_font = FontDescriptor {
            families: parse_font_families_for(&font_family, RenderFamilyKind::Sequence)?,
            weight: settings.font_weight,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let message_font = actor_font.clone();
        let note_font = actor_font.clone();
        let activations =
            collect_activations(model, layout, &nodes_by_id, &edges_by_id, &settings)?;
        let controls = collect_controls(model, layout, &settings)?;

        Ok(Self {
            metadata,
            session,
            document,
            model,
            layout,
            settings,
            palette,
            actor_font,
            message_font,
            note_font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            nodes_by_id,
            edges_by_id,
            activations,
            controls,
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let navigation_security = self.navigation_security();
        let actor_links = self.portable_actor_links(navigation_security)?;
        self.document.push_semantic(SemanticAnnotation {
            id: "sequence.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        })?;

        self.emit_boxes()?;
        self.emit_rect_blocks()?;
        self.emit_actors()?;
        self.emit_controls()?;
        self.emit_activations()?;
        self.emit_notes()?;
        self.emit_messages()?;
        if let Some(title) = crate::sequence::sequence_render_title(
            self.model.title.as_deref(),
            self.metadata.title.as_deref(),
        ) {
            self.emit_title(title)?;
        }

        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_control(DrawingCommand::Restore)?;

        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Sequence layout did not provide root bounds"))?;
        let viewport = Viewport::new(Rect::new(
            bounds.min_x - VIEWPORT_PADDING,
            bounds.min_y - VIEWPORT_PADDING,
            (bounds.max_x - bounds.min_x + 2.0 * VIEWPORT_PADDING).max(1.0),
            (bounds.max_y - bounds.min_y + 2.0 * VIEWPORT_PADDING).max(1.0),
        ));
        let extensions = BTreeMap::from([(
            "x-merman-sequence".to_string(),
            json!({
                "diagram_type": self.metadata.diagram_type,
                "look": "classic",
                "text_mode": "plain_host_text",
                "markup": "plain_only_with_br",
                "markers": "typed_paths",
                "mirror_actors": self.settings.mirror_actors,
                "right_angles": self.settings.right_angles,
                "message_align": self.settings.message_align.as_str(),
                "links": actor_links,
            }),
        )]);
        let document = self.document.finish(viewport, extensions)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Sequence,
                body: SvgStructureBody::Sequence(SequenceSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                }),
            },
        })
    }

    fn emit_boxes(&mut self) -> Result<()> {
        let max_title_height = self
            .model
            .boxes
            .iter()
            .filter_map(|sequence_box| sequence_box.name.as_deref())
            .map(|name| {
                let lines = plain_lines(name, "Sequence box title")?;
                Ok(lines.len().max(1) as f64
                    * sequence_text_dimensions_height_px(self.settings.font_size))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .fold(0.0, f64::max);

        for (box_index, sequence_box) in self.model.boxes.iter().enumerate().rev() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if sequence_box.wrap {
                return Err(unavailable(
                    "wrapped Sequence box titles are not yet canonicalized",
                ));
            }
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for actor_id in &sequence_box.actor_keys {
                let top = self
                    .nodes_by_id
                    .get(format!("actor-top-{actor_id}").as_str())
                    .ok_or_else(|| {
                        invalid(format!("Sequence box references missing actor {actor_id}"))
                    })?;
                let bottom = self
                    .nodes_by_id
                    .get(format!("actor-bottom-{actor_id}").as_str())
                    .ok_or_else(|| {
                        invalid(format!(
                            "Sequence box references missing footer actor {actor_id}"
                        ))
                    })?;
                min_x = min_x.min(top.x - top.width / 2.0);
                max_x = max_x.max(top.x + top.width / 2.0);
                min_y = min_y.min(top.y - top.height / 2.0);
                max_y = max_y.max(bottom.y + bottom.height / 2.0);
            }
            if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite()
            {
                return Err(invalid("Sequence box has no actor geometry"));
            }
            let pad_x = (2.0 * self.settings.box_margin + self.settings.box_text_margin).max(0.0);
            let pad_top =
                (self.settings.box_margin + self.settings.box_text_margin + max_title_height)
                    .max(0.0);
            let pad_bottom = (2.0 * self.settings.box_margin).max(0.0);
            let x = min_x - pad_x;
            let y = min_y - pad_top;
            let width = max_x - min_x + 2.0 * pad_x;
            let height = max_y - min_y + pad_top + pad_bottom;
            let semantic_id = format!("sequence.box.{box_index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            let fill =
                PortableStyleResolver::new("sequence").color("box.fill", &sequence_box.fill)?;
            self.add_path(
                format!("{semantic_id}.shape"),
                rounded_rect_path(x + width / 2.0, y + height / 2.0, width, height, 0.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: Some(stroke(self.palette.node_border, self.palette.stroke_width)),
                },
            )?;
            if let Some(name) = sequence_box.name.as_deref() {
                let lines = plain_lines(name, "Sequence box title")?;
                let actor_font = self.actor_font.clone();
                self.emit_text_lines(
                    &format!("{semantic_id}.title"),
                    &lines,
                    TextLinesSpec {
                        center: Point::new(
                            x + width / 2.0,
                            min_y - self.settings.box_margin - max_title_height / 2.0,
                        ),
                        font_size: self.settings.font_size,
                        weight: self.settings.font_weight,
                        color: self.palette.node_border,
                        font: actor_font,
                        anchor: TextAnchor::Middle,
                    },
                )?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: sequence_box.name.clone(),
                description: Some("Sequence participant box".to_string()),
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_rect_blocks(&mut self) -> Result<()> {
        for (index, message) in self.model.messages.iter().enumerate() {
            let Some(control) = message.control_semantics() else {
                continue;
            };
            if control.kind != SequenceControlKind::Rect
                || control.role != SequenceControlRole::Start
            {
                continue;
            }
            let node_id = format!("rect-{}", message.id);
            let node = self.nodes_by_id.get(node_id.as_str()).ok_or_else(|| {
                invalid(format!("Sequence rect {} has no layout node", message.id))
            })?;
            let fill_value = if message.message_text().trim().is_empty() {
                sequence_rect_default_fill(self.metadata.effective_config.as_value())
            } else {
                message.message_text().trim().to_string()
            };
            let fill =
                PortableStyleResolver::new("sequence").optional_color("rect.fill", &fill_value)?;
            let semantic_id = format!("sequence.rect.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.add_path(
                format!("{semantic_id}.shape"),
                rounded_rect_path(node.x, node.y, node.width, node.height, 0.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: fill.map(Paint::solid),
                    stroke: Some(stroke(self.palette.node_border, self.palette.stroke_width)),
                },
            )?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(fill_value),
                description: Some("Sequence rect block".to_string()),
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_actors(&mut self) -> Result<()> {
        for (index, actor_id) in self.model.actor_order.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let actor = self.model.actors.get(actor_id).ok_or_else(|| {
                invalid(format!(
                    "Sequence actor order references missing actor {actor_id}"
                ))
            })?;
            let actor_link = portable_actor_link(actor, self.navigation_security())?;
            let top = (**self
                .nodes_by_id
                .get(format!("actor-top-{actor_id}").as_str())
                .ok_or_else(|| {
                    invalid(format!("Sequence actor {actor_id} has no top geometry"))
                })?)
            .clone();
            let actor = actor.clone();
            let semantic_id = format!("sequence.actor.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.add_path(
                format!("{semantic_id}.top.shape"),
                rounded_rect_path(top.x, top.y, top.width, top.height, 3.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.palette.actor_fill)),
                    stroke: Some(stroke(self.palette.actor_border, self.palette.stroke_width)),
                },
            )?;
            self.emit_actor_label(&format!("{semantic_id}.top.label"), &actor, &top)?;
            self.emit_lifeline(semantic_id.as_str(), actor_id, index)?;
            if self.settings.mirror_actors {
                let bottom = (**self
                    .nodes_by_id
                    .get(format!("actor-bottom-{actor_id}").as_str())
                    .ok_or_else(|| {
                        invalid(format!("Sequence actor {actor_id} has no footer geometry"))
                    })?)
                .clone();
                self.add_path(
                    format!("{semantic_id}.bottom.shape"),
                    rounded_rect_path(bottom.x, bottom.y, bottom.width, bottom.height, 3.0),
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: Some(Paint::solid(self.palette.actor_fill)),
                        stroke: Some(stroke(self.palette.actor_border, self.palette.stroke_width)),
                    },
                )?;
                self.emit_actor_label(&format!("{semantic_id}.bottom.label"), &actor, &bottom)?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(actor.description.clone()),
                description: Some(format!("Sequence participant {actor_id}")),
                link: actor_link,
            })?;
        }
        Ok(())
    }

    fn emit_actor_label(
        &mut self,
        id: &str,
        actor: &SequenceActor,
        node: &LayoutNode,
    ) -> Result<()> {
        let lines = plain_lines(&actor.description, "Sequence participant label")?;
        self.emit_text_lines(
            id,
            &lines,
            TextLinesSpec {
                center: Point::new(node.x, node.y),
                font_size: self.settings.font_size,
                weight: self.settings.font_weight,
                color: self.palette.actor_text,
                font: self.actor_font.clone(),
                anchor: TextAnchor::Middle,
            },
        )
    }

    fn emit_lifeline(&mut self, semantic_id: &str, actor_id: &str, _index: usize) -> Result<()> {
        let edge = self
            .edges_by_id
            .get(format!("lifeline-{actor_id}").as_str())
            .ok_or_else(|| {
                invalid(format!(
                    "Sequence actor {actor_id} has no lifeline geometry"
                ))
            })?;
        let [start, end] = edge.points.as_slice() else {
            return Err(invalid(format!(
                "Sequence lifeline {actor_id} has invalid geometry"
            )));
        };
        self.add_path(
            format!("{semantic_id}.lifeline"),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(start.x, start.y),
                },
                PathSegment::LineTo {
                    to: Point::new(end.x, end.y),
                },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.palette.actor_line, LIFELINE_WIDTH)),
            },
        )
    }

    fn emit_controls(&mut self) -> Result<()> {
        for block in &self.controls.clone() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let semantic_id = format!("sequence.control.{}", block.start_id);
            let (frame_x1, frame_x2) =
                block
                    .layout
                    .start_x
                    .zip(block.layout.stop_x)
                    .ok_or_else(|| {
                        invalid(format!(
                            "Sequence control {} has no horizontal frame bounds",
                            block.start_id
                        ))
                    })?;
            if frame_x2 <= frame_x1 || block.layout.stop_y <= block.layout.start_y {
                return Err(invalid(format!(
                    "Sequence control {} has invalid frame bounds",
                    block.start_id
                )));
            }
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.add_path(
                format!("{semantic_id}.frame"),
                rectangle_segments(
                    frame_x1,
                    block.layout.start_y,
                    frame_x2,
                    block.layout.stop_y,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(dashed_stroke(
                        self.palette.label_box_border,
                        CONTROL_WIDTH,
                        CONTROL_DASH.to_vec(),
                    )),
                },
            )?;
            let label = control_label(block.kind);
            self.emit_control_label_box(&semantic_id, frame_x1, block.layout.start_y, label)?;
            let label_box_right = frame_x1 + self.settings.label_box_width;
            let center_x = (label_box_right + frame_x2) / 2.0;
            if let Some((_, raw_label, _)) = block.sections.first() {
                let lines = plain_lines(raw_label, "Sequence control label")?;
                if !is_empty_lines(&lines) {
                    self.emit_text_lines(
                        &format!("{semantic_id}.label"),
                        &lines,
                        TextLinesSpec {
                            center: Point::new(center_x, block.layout.start_y + LABEL_BOX_HEIGHT),
                            font_size: self.settings.font_size,
                            weight: self.settings.font_weight,
                            color: self.palette.loop_text,
                            font: self.message_font.clone(),
                            anchor: TextAnchor::Middle,
                        },
                    )?;
                }
            }
            for (section_index, (_, raw_label, separator_y)) in
                block.sections.iter().enumerate().skip(1)
            {
                let separator_y = separator_y.ok_or_else(|| {
                    invalid(format!(
                        "Sequence control {} has a missing section separator",
                        block.start_id
                    ))
                })?;
                self.add_path(
                    format!("{semantic_id}.separator.{section_index}"),
                    vec![
                        PathSegment::MoveTo {
                            to: Point::new(frame_x1, separator_y),
                        },
                        PathSegment::LineTo {
                            to: Point::new(frame_x2, separator_y),
                        },
                    ],
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(dashed_stroke(
                            self.palette.label_box_border,
                            CONTROL_WIDTH,
                            CONTROL_SEPARATOR_DASH.to_vec(),
                        )),
                    },
                )?;
                let lines = plain_lines(raw_label, "Sequence control section label")?;
                if !is_empty_lines(&lines) {
                    self.emit_text_lines(
                        &format!("{semantic_id}.section.{section_index}"),
                        &lines,
                        TextLinesSpec {
                            center: Point::new(
                                (frame_x1 + frame_x2) / 2.0,
                                separator_y + LABEL_BOX_HEIGHT,
                            ),
                            font_size: self.settings.font_size,
                            weight: self.settings.font_weight,
                            color: self.palette.loop_text,
                            font: self.message_font.clone(),
                            anchor: TextAnchor::Middle,
                        },
                    )?;
                }
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(label.to_string()),
                description: Some(format!("Sequence {} control block", label)),
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_control_label_box(
        &mut self,
        semantic_id: &str,
        x: f64,
        y: f64,
        label: &str,
    ) -> Result<()> {
        let x2 = x + self.settings.label_box_width;
        let y2 = y + 13.0;
        let y3 = y + LABEL_BOX_HEIGHT;
        self.add_path(
            format!("{semantic_id}.label_box"),
            polygon_path(&[
                Point::new(x, y),
                Point::new(x2, y),
                Point::new(x2, y2),
                Point::new(x2 - LABEL_BOX_ARROW_INSET, y3),
                Point::new(x, y3),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.palette.label_box_fill)),
                stroke: Some(stroke(
                    self.palette.label_box_border,
                    self.palette.stroke_width,
                )),
            },
        )?;
        self.emit_text_lines(
            &format!("{semantic_id}.label_box_text"),
            &[label.to_string()],
            TextLinesSpec {
                center: Point::new(x + self.settings.label_box_width / 2.0, y + 13.0),
                font_size: self.settings.font_size,
                weight: self.settings.font_weight,
                color: self.palette.label_text,
                font: self.message_font.clone(),
                anchor: TextAnchor::Middle,
            },
        )
    }

    fn emit_activations(&mut self) -> Result<()> {
        for (index, activation) in self.activations.clone().into_iter().enumerate() {
            let semantic_id = format!("sequence.activation.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.add_path(
                format!("{semantic_id}.shape"),
                rounded_rect_path(
                    activation.x + activation.width / 2.0,
                    activation.y + activation.height / 2.0,
                    activation.width,
                    activation.height,
                    0.0,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.palette.activation_fill)),
                    stroke: Some(stroke(
                        self.palette.activation_border,
                        self.palette.stroke_width,
                    )),
                },
            )?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(activation.start_id),
                description: Some(format!(
                    "Sequence activation level {}",
                    activation.class_index
                )),
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_notes(&mut self) -> Result<()> {
        for (index, message) in self.model.messages.iter().enumerate() {
            if message.semantic_kind() != SequenceMessageKind::Note {
                continue;
            }
            let node = (**self
                .nodes_by_id
                .get(format!("note-{}", message.id).as_str())
                .ok_or_else(|| {
                    invalid(format!("Sequence note {} has no layout node", message.id))
                })?)
            .clone();
            let semantic_id = format!("sequence.note.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.add_path(
                format!("{semantic_id}.shape"),
                rounded_rect_path(node.x, node.y, node.width, node.height, 0.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.palette.note_fill)),
                    stroke: Some(stroke(self.palette.note_border, self.palette.stroke_width)),
                },
            )?;
            let lines = plain_lines(message.message_text(), "Sequence note")?;
            self.emit_text_lines(
                &format!("{semantic_id}.label"),
                &lines,
                TextLinesSpec {
                    center: Point::new(node.x, node.y),
                    font_size: self.settings.font_size,
                    weight: self.settings.font_weight,
                    color: self.palette.note_text,
                    font: self.note_font.clone(),
                    anchor: TextAnchor::Middle,
                },
            )?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(svg_plain_text(message.message_text())),
                description: Some("Sequence note".to_string()),
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_messages(&mut self) -> Result<()> {
        let mut autonumber_visible = false;
        let mut autonumber = 1.0;
        let mut autonumber_step = 1.0;
        for (index, source_message) in self.model.messages.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let message = source_message;
            match message.semantic_kind() {
                SequenceMessageKind::Autonumber => {
                    if let merman_core::diagrams::sequence::SequenceMessagePayload::Autonumber(
                        value,
                    ) = &message.message
                    {
                        autonumber_visible = value.visible;
                        if let Some(start) = value.start {
                            autonumber = start;
                        }
                        if let Some(step) = value.step {
                            autonumber_step = step;
                        }
                    }
                    continue;
                }
                SequenceMessageKind::Signal => {}
                SequenceMessageKind::ActivationStart
                | SequenceMessageKind::ActivationEnd
                | SequenceMessageKind::CentralDecorationRecord => continue,
                SequenceMessageKind::Note | SequenceMessageKind::Control => continue,
                SequenceMessageKind::Unknown => {
                    return Err(unavailable(format!(
                        "Sequence message {} has no portable semantic kind",
                        message.id
                    )));
                }
            }
            let from = message
                .from
                .as_deref()
                .ok_or_else(|| invalid("Sequence signal has no source"))?;
            let to = message
                .to
                .as_deref()
                .ok_or_else(|| invalid("Sequence signal has no target"))?;
            let edge = *self
                .edges_by_id
                .get(format!("msg-{}", message.id).as_str())
                .ok_or_else(|| {
                    invalid(format!("Sequence signal {} has no layout edge", message.id))
                })?;
            if edge.points.len() < 2 {
                return Err(invalid(format!(
                    "Sequence signal {} has too few route points",
                    message.id
                )));
            }
            let semantics = message.signal_semantics().ok_or_else(|| {
                invalid(format!(
                    "Sequence signal {} has no line semantics",
                    message.id
                ))
            })?;
            let semantic_id = format!("sequence.message.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            if let Some(label) = edge.label.as_ref() {
                let lines = plain_lines(message.message_text(), "Sequence message label")?;
                if !is_empty_lines(&lines) {
                    let (x, anchor) = self.message_label_position(edge, label);
                    let message_font = self.message_font.clone();
                    self.emit_text_lines(
                        &format!("{semantic_id}.label"),
                        &lines,
                        TextLinesSpec {
                            center: Point::new(x, label.y),
                            font_size: self.settings.font_size,
                            weight: self.settings.font_weight,
                            color: self.palette.signal_text,
                            font: message_font,
                            anchor,
                        },
                    )?;
                }
            }
            let self_route = if from == to {
                self_message_route_segments(edge, self.settings.right_angles)
            } else {
                Vec::new()
            };
            let start_marker = marker_endpoint_for(
                semantics.source_marker,
                &edge.points,
                &self_route,
                MarkerPosition::Start,
                &message.id,
            )?;
            let end_marker = marker_endpoint_for(
                semantics.target_marker,
                &edge.points,
                &self_route,
                MarkerPosition::End,
                &message.id,
            )?;
            self.session.checkpoint(OperationPhase::Emit)?;
            let route_style = PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(dashed_stroke(
                    self.palette.signal,
                    SIGNAL_WIDTH,
                    if semantics.stroke == SequenceMessageStroke::Dotted {
                        vec![3.0, 3.0]
                    } else {
                        Vec::new()
                    },
                )),
            };
            self.document.draw_path_with(
                ResourceId::new(format!("{semantic_id}.route")),
                route_style,
                |emit| {
                    if self_route.is_empty() {
                        for (index, point) in edge.points.iter().enumerate() {
                            let to = Point::new(point.x, point.y);
                            emit(if index == 0 {
                                PathSegment::MoveTo { to }
                            } else {
                                PathSegment::LineTo { to }
                            })?;
                        }
                    } else {
                        for segment in self_route {
                            emit(segment)?;
                        }
                    }
                    Ok(())
                },
            )?;
            self.emit_message_markers(&semantic_id, semantics, start_marker, end_marker)?;
            self.emit_central_connection(&semantic_id, message, from, to, edge.points[0].y)?;
            if autonumber_visible {
                self.emit_sequence_number(&semantic_id, edge, message, autonumber)?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Edge,
                title: Some(svg_plain_text(message.message_text())),
                description: Some(format!("{from} → {to}")),
                link: None,
            })?;
            if autonumber.is_finite() {
                autonumber = ((autonumber + autonumber_step) * 100.0).round() / 100.0;
            }
        }
        Ok(())
    }

    fn message_label_position(
        &self,
        edge: &LayoutEdge,
        label: &crate::model::LayoutLabel,
    ) -> (f64, TextAnchor) {
        let first = &edge.points[0];
        let second = &edge.points[1];
        let left = first.x.min(second.x);
        let width = (first.x - second.x).abs();
        match self.settings.message_align {
            SequenceMessageAlign::Left => (left + self.settings.wrap_padding, TextAnchor::Start),
            SequenceMessageAlign::Right => {
                (left + width - self.settings.wrap_padding, TextAnchor::End)
            }
            SequenceMessageAlign::Center => (label.x, TextAnchor::Middle),
        }
    }

    fn emit_message_markers(
        &mut self,
        semantic_id: &str,
        semantics: merman_core::diagrams::sequence::SequenceSignalSemantics,
        start_marker: Option<SequenceMarkerEndpoint>,
        end_marker: Option<SequenceMarkerEndpoint>,
    ) -> Result<()> {
        if let Some(endpoint) = start_marker {
            self.emit_marker(
                format!("{semantic_id}.marker.start"),
                endpoint.point,
                endpoint.direction,
                semantics.source_marker,
            )?;
        }
        if let Some(endpoint) = end_marker {
            self.emit_marker(
                format!("{semantic_id}.marker.end"),
                endpoint.point,
                endpoint.direction,
                semantics.target_marker,
            )?;
        }
        Ok(())
    }

    fn emit_marker(
        &mut self,
        id: String,
        point: Point,
        direction: Point,
        marker: SequenceMessageMarker,
    ) -> Result<()> {
        let normal = Point::new(-direction.y, direction.x);
        let base = Point::new(point.x - direction.x * 10.0, point.y - direction.y * 10.0);
        let segments = match marker {
            SequenceMessageMarker::Filled => polygon_path(&[
                point,
                Point::new(base.x + normal.x * 5.0, base.y + normal.y * 5.0),
                Point::new(base.x - normal.x * 5.0, base.y - normal.y * 5.0),
            ]),
            SequenceMessageMarker::Point => point_marker_path(point, direction),
            SequenceMessageMarker::Cross => vec![
                PathSegment::MoveTo {
                    to: Point::new(
                        point.x - normal.x * 4.0 - direction.x * 2.5,
                        point.y - normal.y * 4.0 - direction.y * 2.5,
                    ),
                },
                PathSegment::LineTo {
                    to: Point::new(
                        point.x + normal.x * 4.0 + direction.x * 2.5,
                        point.y + normal.y * 4.0 + direction.y * 2.5,
                    ),
                },
                PathSegment::MoveTo {
                    to: Point::new(
                        point.x + normal.x * 4.0 - direction.x * 2.5,
                        point.y + normal.y * 4.0 - direction.y * 2.5,
                    ),
                },
                PathSegment::LineTo {
                    to: Point::new(
                        point.x - normal.x * 4.0 + direction.x * 2.5,
                        point.y - normal.y * 4.0 + direction.y * 2.5,
                    ),
                },
            ],
            SequenceMessageMarker::FilledHalfTop | SequenceMessageMarker::FilledHalfBottom => {
                let side = if marker == SequenceMessageMarker::FilledHalfTop {
                    1.0
                } else {
                    -1.0
                };
                polygon_path(&[
                    point,
                    base,
                    Point::new(
                        base.x + normal.x * side * 7.0,
                        base.y + normal.y * side * 7.0,
                    ),
                ])
            }
            SequenceMessageMarker::OpenHalfTop | SequenceMessageMarker::OpenHalfBottom => {
                let side = if marker == SequenceMessageMarker::OpenHalfTop {
                    1.0
                } else {
                    -1.0
                };
                vec![
                    PathSegment::MoveTo { to: point },
                    PathSegment::LineTo { to: base },
                    PathSegment::LineTo {
                        to: Point::new(
                            base.x + normal.x * side * 7.0,
                            base.y + normal.y * side * 7.0,
                        ),
                    },
                ]
            }
            SequenceMessageMarker::None => return Ok(()),
        };
        let (fill, stroke_style) = match marker {
            SequenceMessageMarker::Point => (Some(Paint::solid(self.palette.text)), None),
            SequenceMessageMarker::Cross
            | SequenceMessageMarker::OpenHalfTop
            | SequenceMessageMarker::OpenHalfBottom => {
                (None, Some(stroke(self.palette.signal, 1.0)))
            }
            _ => (
                Some(Paint::solid(self.palette.signal)),
                Some(stroke(self.palette.signal, 1.0)),
            ),
        };
        self.add_path(
            id,
            segments,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill,
                stroke: stroke_style,
            },
        )
    }

    fn emit_central_connection(
        &mut self,
        semantic_id: &str,
        message: &SequenceMessage,
        from: &str,
        to: &str,
        y: f64,
    ) -> Result<()> {
        let Some(decoration) = message.central_decoration() else {
            return Ok(());
        };
        if decoration == merman_core::diagrams::sequence::SequenceCentralDecoration::None {
            return Ok(());
        }
        let from_x = self
            .nodes_by_id
            .get(format!("actor-top-{from}").as_str())
            .ok_or_else(|| {
                invalid(format!(
                    "central connection references missing actor {from}"
                ))
            })?
            .x;
        let to_x = self
            .nodes_by_id
            .get(format!("actor-top-{to}").as_str())
            .ok_or_else(|| invalid(format!("central connection references missing actor {to}")))?
            .x;
        let offset = if from_x <= to_x {
            CENTRAL_CONNECTION_CIRCLE_OFFSET
        } else {
            -CENTRAL_CONNECTION_CIRCLE_OFFSET
        };
        let mut centers = Vec::new();
        if matches!(
            decoration,
            merman_core::diagrams::sequence::SequenceCentralDecoration::Source
                | merman_core::diagrams::sequence::SequenceCentralDecoration::Both
        ) {
            centers.push(from_x + offset);
        }
        if matches!(
            decoration,
            merman_core::diagrams::sequence::SequenceCentralDecoration::Target
                | merman_core::diagrams::sequence::SequenceCentralDecoration::Both
        ) {
            centers.push(to_x - offset);
        }
        for (index, x) in centers.into_iter().enumerate() {
            self.add_path(
                format!("{semantic_id}.central.{index}"),
                ellipse_path(x, y, 5.0, 5.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.palette.signal)),
                    stroke: None,
                },
            )?;
        }
        Ok(())
    }

    fn emit_sequence_number(
        &mut self,
        semantic_id: &str,
        edge: &LayoutEdge,
        _message: &SequenceMessage,
        value: f64,
    ) -> Result<()> {
        let point = Point::new(edge.points[0].x, edge.points[0].y);
        self.add_path(
            format!("{semantic_id}.number.circle"),
            ellipse_path(point.x, point.y, 6.0, 6.0),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.palette.signal)),
                stroke: None,
            },
        )?;
        let text = if value.is_finite() {
            value.to_string()
        } else {
            String::new()
        };
        self.emit_text_lines(
            &format!("{semantic_id}.number.text"),
            &[text],
            TextLinesSpec {
                center: Point::new(point.x, point.y + 1.0),
                font_size: if value.abs() >= 1000.0 {
                    9.0
                } else if value.abs() >= 100.0 {
                    10.0
                } else {
                    12.0
                },
                weight: 400,
                color: self.palette.sequence_number,
                font: FontDescriptor {
                    families: vec!["sans-serif".to_string()],
                    weight: 400,
                    style: FontStyle::Normal,
                    postscript_name: None,
                    resource: None,
                },
                anchor: TextAnchor::Middle,
            },
        )
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        let lines = plain_lines(title, "Sequence title")?;
        if is_empty_lines(&lines) {
            return Ok(());
        }
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Sequence layout did not provide root bounds"))?;
        let width = bounds.max_x - bounds.min_x;
        let content_width = (width - 2.0 * self.settings.diagram_margin_x).max(0.0);
        let x = content_width / 2.0 - 2.0 * self.settings.diagram_margin_x;
        self.emit_text_lines(
            "sequence.title",
            &lines,
            TextLinesSpec {
                center: Point::new(x, -25.0),
                font_size: self.settings.font_size,
                weight: self.settings.font_weight,
                color: self.palette.text,
                font: self.actor_font.clone(),
                anchor: TextAnchor::Middle,
            },
        )?;
        self.document.push_semantic(SemanticAnnotation {
            id: "sequence.title".to_string(),
            role: SemanticRole::Label,
            title: Some(svg_plain_text(title)),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn navigation_security(&self) -> MermaidNavigationSecurity {
        MermaidNavigationSecurity::from_security_level_loose(
            self.metadata
                .effective_config
                .as_value()
                .get("securityLevel")
                .and_then(Value::as_str)
                == Some("loose"),
        )
    }

    fn portable_actor_links(
        &self,
        security: MermaidNavigationSecurity,
    ) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
        self.model
            .actor_order
            .iter()
            .filter_map(|actor_id| {
                self.model
                    .actors
                    .get(actor_id)
                    .map(|actor| (actor_id, actor))
            })
            .filter(|(_, actor)| !actor.links.is_empty())
            .map(|(actor_id, actor)| {
                let Some(url) = portable_actor_link(actor, security)? else {
                    return Ok(None);
                };
                let label =
                    actor.links.keys().next().cloned().ok_or_else(|| {
                        invalid("Sequence actor link map changed during rendering")
                    })?;
                Ok(Some((actor_id.clone(), BTreeMap::from([(label, url)]))))
            })
            .filter_map(|result| result.transpose())
            .collect()
    }

    fn emit_text_lines(&mut self, _id: &str, lines: &[String], spec: TextLinesSpec) -> Result<()> {
        let TextLinesSpec {
            center,
            font_size,
            weight,
            color,
            font,
            anchor,
        } = spec;
        let line_step = sequence_text_line_step_px(font_size);
        let line_count = lines.len().max(1) as f64;
        let style = MeasurementTextStyle {
            font_family: Some(font.families.join(", ")),
            font_size,
            font_weight: Some(weight.to_string()),
            font_style: None,
        };
        for (index, line) in lines.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let display = if line.is_empty() {
                "\u{200B}"
            } else {
                line.as_str()
            };
            let y = center.y + (index as f64 - (line_count - 1.0) / 2.0) * line_step;
            let session = self.session;
            let obligation = self.text_obligation.clone();
            let font = font.clone();
            self.document.draw_host_text_parts(&[display], |text| {
                let measurer = session
                    .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
                let width = measurer.measure_svg_raw_text_bbox_width_px(&text, &style);
                let height = measurer.measure_svg_simple_text_bbox_height_px(&text, &style);
                session.checkpoint(OperationPhase::Emit)?;
                if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
                    return Err(invalid("Sequence text measurement returned invalid bounds"));
                }
                let width = width.max(1.0);
                let height = height.max(1.0);
                let left = match anchor {
                    TextAnchor::Start => center.x,
                    TextAnchor::Middle => center.x - width / 2.0,
                    TextAnchor::End => center.x - width,
                };
                Ok(TextRun {
                    text,
                    origin: Point::new(center.x, y),
                    bounds: Rect::new(left, y - height / 2.0, width, height),
                    style: DisplayTextStyle {
                        font: FontDescriptor { weight, ..font },
                        font_size,
                        letter_spacing: 0.0,
                        line_height: line_step,
                        fill: Paint::solid(color),
                        stroke: None,
                        paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                    },
                    anchor,
                    baseline: TextBaseline::Middle,
                    direction: TextDirection::Auto,
                    language: None,
                    obligation,
                })
            })?;
        }
        Ok(())
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)?;
        let id = ResourceId::new(id.into());
        self.document.draw_path(id, segments, style)
    }
}

impl SequenceMessageAlign {
    fn as_str(self) -> &'static str {
        match self {
            SequenceMessageAlign::Left => "left",
            SequenceMessageAlign::Center => "center",
            SequenceMessageAlign::Right => "right",
        }
    }
}

fn dashed_stroke(
    color: Color,
    width: f64,
    dash_array: Vec<f64>,
) -> merman_display_list::StrokeStyle {
    let mut style = stroke(color, width);
    style.dash_array = dash_array;
    style
}

fn sequence_settings(config: &Value) -> Result<SequenceSettings> {
    let sequence = config.get("sequence").unwrap_or(&Value::Null);
    let number = |key: &str, default: f64, min: f64| {
        config_f64(sequence, &[key]).unwrap_or(default).max(min)
    };
    let font_size = config_f64(config, &["fontSize"])
        .or_else(|| config_f64(sequence, &["messageFontSize"]))
        .unwrap_or(16.0)
        .max(1.0);
    let font_weight = parse_font_weight(
        config_string(config, &["fontWeight"])
            .or_else(|| config_string(sequence, &["messageFontWeight"]))
            .as_deref()
            .unwrap_or("400"),
    )?;
    let message_align = match config_string(sequence, &["messageAlign"])
        .unwrap_or_else(|| "center".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "left" => SequenceMessageAlign::Left,
        "right" => SequenceMessageAlign::Right,
        _ => SequenceMessageAlign::Center,
    };
    Ok(SequenceSettings {
        mirror_actors: sequence
            .get("mirrorActors")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        right_angles: sequence
            .get("rightAngles")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        diagram_margin_x: number("diagramMarginX", 50.0, 0.0),
        box_margin: number("boxMargin", 10.0, 0.0),
        box_text_margin: number("boxTextMargin", 5.0, 0.0),
        label_box_width: number("labelBoxWidth", 50.0, 50.0),
        wrap_padding: number("wrapPadding", 10.0, 0.0),
        sequence_width: number("width", 150.0, 1.0),
        activation_width: number("activationWidth", 10.0, 1.0),
        font_size,
        font_weight,
        message_align,
    })
}

fn sequence_palette(config: &Value) -> Result<SequencePalette> {
    let styles = PortableStyleResolver::new("sequence");
    let actor_border = theme_color(config, "actorBorder", "#9370DB")?;
    let actor_fill = theme_color(config, "actorBkg", "#ECECFF")?;
    let actor_text = theme_color(config, "actorTextColor", "black")?;
    let stroke_width_value = config
        .get("themeVariables")
        .and_then(|variables| variables.get("strokeWidth"))
        .and_then(Value::as_str)
        .unwrap_or("1");
    Ok(SequencePalette {
        actor_fill,
        actor_border,
        actor_text,
        actor_line: theme_color(config, "actorLineColor", "#9370DB")?,
        signal: theme_color(config, "signalColor", "#333")?,
        signal_text: theme_color(config, "signalTextColor", "#333")?,
        sequence_number: theme_color(config, "sequenceNumberColor", "white")?,
        label_box_fill: theme_color(config, "labelBoxBkgColor", "#ECECFF")?,
        label_box_border: theme_color(config, "labelBoxBorderColor", "#9370DB")?,
        label_text: theme_color(config, "labelTextColor", "black")?,
        loop_text: theme_color(config, "loopTextColor", "black")?,
        note_fill: theme_color(config, "noteBkgColor", "#fff5ad")?,
        note_border: theme_color(config, "noteBorderColor", "#aaaa33")?,
        note_text: theme_color(config, "noteTextColor", "black")?,
        activation_fill: theme_color(config, "activationBkgColor", "#f4f4f4")?,
        activation_border: theme_color(config, "activationBorderColor", "#666")?,
        node_border: theme_color(config, "nodeBorder", "#9370DB")?,
        text: theme_color(config, "textColor", "#333")?,
        stroke_width: styles.length("strokeWidth", stroke_width_value)?.max(0.0),
    })
}

fn sequence_rect_default_fill(config: &Value) -> String {
    config_string(config, &["themeVariables", "rectBkgColor"])
        .or_else(|| config_string(config, &["themeVariables", "actorBkg"]))
        .unwrap_or_else(|| "rgba(128, 128, 128, 0.5)".to_string())
}

fn validate_model(
    model: &SequenceDiagramRenderModel,
    layout: &SequenceDiagramLayout,
    settings: &SequenceSettings,
) -> Result<()> {
    if model.actor_order.is_empty() {
        return Err(invalid("Sequence diagram has no participants"));
    }
    if !model.created_actors.is_empty()
        || !model.destroyed_actors.is_empty()
        || model.actor_lifecycles.as_ref().is_some_and(|lifecycles| {
            lifecycles
                .iter()
                .any(|lifecycle| *lifecycle != Default::default())
        })
    {
        return Err(unavailable(
            "Sequence create/destroy lifecycle rewrites require a canonical visibility timeline",
        ));
    }
    if settings.sequence_width <= 0.0 || settings.activation_width <= 0.0 {
        return Err(invalid("Sequence numeric settings are invalid"));
    }
    validate_bounds(
        layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Sequence layout did not provide root bounds"))?,
    )?;
    for actor_id in &model.actor_order {
        let actor = model
            .actors
            .get(actor_id)
            .ok_or_else(|| invalid(format!("missing Sequence actor {actor_id}")))?;
        if actor.actor_type != "participant" {
            return Err(unavailable(format!(
                "Sequence actor type `{}` requires a shape-specific canonical adapter",
                actor.actor_type
            )));
        }
        if actor.wrap {
            return Err(unavailable(
                "wrapped Sequence participant labels are not yet canonicalized",
            ));
        }
        if !actor.properties.is_empty() {
            return Err(unavailable(
                "custom Sequence participant properties are not resolved into portable paint state",
            ));
        }
    }
    for sequence_box in &model.boxes {
        if sequence_box.wrap {
            return Err(unavailable(
                "wrapped Sequence box labels are not yet canonicalized",
            ));
        }
        for actor_id in &sequence_box.actor_keys {
            if !model.actors.contains_key(actor_id) {
                return Err(invalid(format!(
                    "Sequence box references missing actor {actor_id}"
                )));
            }
        }
        plain_lines(
            sequence_box.name.as_deref().unwrap_or(""),
            "Sequence box title",
        )?;
        PortableStyleResolver::new("sequence").color("box.fill", &sequence_box.fill)?;
    }
    for message in &model.messages {
        if message.wrap {
            return Err(unavailable(
                "wrapped Sequence labels are not yet canonicalized",
            ));
        }
        match message.semantic_kind() {
            SequenceMessageKind::Signal => {
                if message.from.is_none() || message.to.is_none() {
                    return Err(invalid(format!(
                        "Sequence signal {} has incomplete endpoints",
                        message.id
                    )));
                }
                plain_lines(message.message_text(), "Sequence message label")?;
            }
            SequenceMessageKind::Note => {
                plain_lines(message.message_text(), "Sequence note")?;
                if message.note_placement().is_none() {
                    return Err(invalid(format!(
                        "Sequence note {} has invalid placement",
                        message.id
                    )));
                }
            }
            SequenceMessageKind::Control => {
                if message
                    .control_semantics()
                    .is_some_and(|control| control.consumes_text())
                {
                    plain_lines(message.message_text(), "Sequence control label")?;
                }
            }
            SequenceMessageKind::Autonumber
            | SequenceMessageKind::ActivationStart
            | SequenceMessageKind::ActivationEnd
            | SequenceMessageKind::CentralDecorationRecord => {}
            SequenceMessageKind::Unknown => {
                return Err(unavailable(format!(
                    "Sequence message {} is unsupported",
                    message.id
                )));
            }
        }
    }
    Ok(())
}

fn validate_geometry(
    model: &SequenceDiagramRenderModel,
    layout: &SequenceDiagramLayout,
    nodes: &HashMap<&str, &LayoutNode>,
    edges: &HashMap<&str, &LayoutEdge>,
    settings: &SequenceSettings,
) -> Result<()> {
    for actor_id in &model.actor_order {
        for prefix in ["actor-top-", "actor-bottom-"] {
            let id = format!("{prefix}{actor_id}");
            let node = nodes
                .get(id.as_str())
                .ok_or_else(|| invalid(format!("missing Sequence layout node {id}")))?;
            validate_node(node)?;
        }
        let edge = edges
            .get(format!("lifeline-{actor_id}").as_str())
            .ok_or_else(|| invalid(format!("missing Sequence lifeline {actor_id}")))?;
        validate_edge(edge, 2)?;
    }
    for message in &model.messages {
        match message.semantic_kind() {
            SequenceMessageKind::Signal => {
                validate_edge(
                    edges
                        .get(format!("msg-{}", message.id).as_str())
                        .ok_or_else(|| {
                            invalid(format!("missing Sequence message edge {}", message.id))
                        })?,
                    2,
                )?;
            }
            SequenceMessageKind::Note => validate_node(
                nodes
                    .get(format!("note-{}", message.id).as_str())
                    .ok_or_else(|| invalid(format!("missing Sequence note node {}", message.id)))?,
            )?,
            _ => {}
        }
    }
    let mut ids = HashSet::new();
    for node in &layout.nodes {
        if !ids.insert(node.id.as_str()) {
            return Err(invalid(format!(
                "duplicate Sequence layout node {}",
                node.id
            )));
        }
    }
    let mut edge_ids = HashSet::new();
    for edge in &layout.edges {
        if !edge_ids.insert(edge.id.as_str()) {
            return Err(invalid(format!(
                "duplicate Sequence layout edge {}",
                edge.id
            )));
        }
    }
    let _ = settings;
    Ok(())
}

fn collect_activations(
    model: &SequenceDiagramRenderModel,
    layout: &SequenceDiagramLayout,
    nodes: &HashMap<&str, &LayoutNode>,
    edges: &HashMap<&str, &LayoutEdge>,
    settings: &SequenceSettings,
) -> Result<Vec<ActivationRect>> {
    let mut stacks: HashMap<&str, Vec<(String, f64, f64)>> = HashMap::new();
    let mut result = Vec::new();
    let mut last_line_y = None;
    for message in &model.messages {
        if let Some(edge) = edges.get(format!("msg-{}", message.id).as_str()) {
            last_line_y = edge.points.first().map(|point| point.y);
        }
        match message.semantic_kind() {
            SequenceMessageKind::ActivationStart => {
                let actor_id = message
                    .from
                    .as_deref()
                    .ok_or_else(|| invalid("Sequence activation start has no actor"))?;
                let node = nodes
                    .get(format!("actor-top-{actor_id}").as_str())
                    .ok_or_else(|| invalid(format!("missing activation actor {actor_id}")))?;
                let stack = stacks.entry(actor_id).or_default();
                let x = sequence_activation_start_x(node.x, stack.len(), settings.activation_width);
                let y = last_line_y
                    .or_else(|| {
                        edges
                            .get(format!("lifeline-{actor_id}").as_str())
                            .and_then(|edge| edge.points.first().map(|point| point.y))
                    })
                    .unwrap_or(node.y + node.height / 2.0);
                stack.push((message.id.clone(), x, y));
            }
            SequenceMessageKind::ActivationEnd => {
                let actor_id = message
                    .from
                    .as_deref()
                    .ok_or_else(|| invalid("Sequence activation end has no actor"))?;
                let stack = stacks.get_mut(actor_id).ok_or_else(|| {
                    invalid(format!(
                        "Sequence activation end {} has no matching start",
                        message.id
                    ))
                })?;
                let (start_id, x, y) = stack.pop().ok_or_else(|| {
                    invalid(format!(
                        "Sequence activation end {} has no matching start",
                        message.id
                    ))
                })?;
                let end_y = last_line_y.unwrap_or(y);
                if end_y <= y {
                    return Err(invalid(format!(
                        "Sequence activation {start_id} has invalid vertical extent"
                    )));
                }
                result.push(ActivationRect {
                    start_id,
                    x,
                    y,
                    width: settings.activation_width,
                    height: end_y - y,
                    class_index: stack.len() % 3,
                });
            }
            _ => {}
        }
    }
    if stacks.values().any(|stack| !stack.is_empty()) {
        return Err(unavailable(
            "unclosed Sequence activations cannot be represented without dropping geometry",
        ));
    }
    let _ = layout;
    Ok(result)
}

fn collect_controls(
    model: &SequenceDiagramRenderModel,
    layout: &SequenceDiagramLayout,
    _settings: &SequenceSettings,
) -> Result<Vec<ControlBlock>> {
    let mut stack: Vec<ControlStackEntry> = Vec::new();
    let mut blocks = Vec::new();
    for (index, message) in model.messages.iter().enumerate() {
        let Some(control) = message.control_semantics() else {
            continue;
        };
        if control.kind == SequenceControlKind::Rect {
            continue;
        }
        match control.role {
            SequenceControlRole::Start => stack.push((
                index,
                control.kind,
                message.id.clone(),
                vec![(message.id.clone(), message.message_text().to_string(), None)],
            )),
            SequenceControlRole::Separator => {
                let Some((_, kind, _, sections)) = stack.last_mut() else {
                    return Err(invalid(format!(
                        "Sequence section {} has no open control block",
                        message.id
                    )));
                };
                if kind.separator_keyword().is_none() {
                    return Err(invalid(format!(
                        "Sequence control {:?} does not accept sections",
                        kind
                    )));
                }
                sections.push((message.id.clone(), message.message_text().to_string(), None));
            }
            SequenceControlRole::End => {
                let (_start_index, kind, start_id, mut sections) =
                    stack.pop().ok_or_else(|| {
                        invalid(format!("Sequence control {} has no open start", message.id))
                    })?;
                if !kind.accepts_end(control.kind) {
                    return Err(invalid(format!(
                        "Sequence control {} closes incompatible block {:?}",
                        message.id, kind
                    )));
                }
                let layout_data = layout.block_layouts_by_id.get(&start_id).ok_or_else(|| {
                    invalid(format!("Sequence control {start_id} has no layout data"))
                })?;
                for section in sections.iter_mut().skip(1) {
                    section.2 = layout_data.section_ys_by_id.get(&section.0).copied();
                }
                blocks.push(ControlBlock {
                    end_index: index,
                    kind,
                    start_id,
                    layout: layout_data.clone(),
                    sections,
                });
            }
        }
    }
    if !stack.is_empty() {
        return Err(unavailable(
            "unclosed Sequence control blocks cannot be represented without dropping frames",
        ));
    }
    blocks.sort_by_key(|block| block.end_index);
    Ok(blocks)
}

fn point_marker_path(point: Point, direction: Point) -> Vec<PathSegment> {
    // Mermaid's filled-head is M18,7 L9,13 L14,7 L9,1 Z, ref=(15.5,7),
    // with default markerUnits=strokeWidth and orient=auto.
    let normal = Point::new(-direction.y, direction.x);
    let points = [(18.0, 7.0), (9.0, 13.0), (14.0, 7.0), (9.0, 1.0)].map(|(x, y)| {
        let along = (x - 15.5) * SIGNAL_WIDTH;
        let across = (y - 7.0) * SIGNAL_WIDTH;
        Point::new(
            point.x + direction.x * along + normal.x * across,
            point.y + direction.y * along + normal.y * across,
        )
    });
    polygon_path(&points)
}

fn self_message_route_segments(edge: &LayoutEdge, right_angles: bool) -> Vec<PathSegment> {
    let start = Point::new(edge.points[0].x, edge.points[0].y);
    if right_angles {
        let dx = edge
            .label
            .as_ref()
            .map(|label| label.width / 2.0)
            .unwrap_or(30.0)
            .max(30.0);
        return vec![
            PathSegment::MoveTo { to: start },
            PathSegment::LineTo {
                to: Point::new(start.x + dx, start.y),
            },
            PathSegment::LineTo {
                to: Point::new(start.x + dx, start.y + 25.0),
            },
            PathSegment::LineTo {
                to: Point::new(start.x, start.y + 25.0),
            },
        ];
    }
    vec![
        PathSegment::MoveTo { to: start },
        PathSegment::CubicTo {
            control1: Point::new(start.x + 60.0, start.y - 10.0),
            control2: Point::new(start.x + 60.0, start.y + 30.0),
            to: Point::new(start.x, start.y + 20.0),
        },
    ]
}

fn control_label(kind: SequenceControlKind) -> &'static str {
    match kind {
        SequenceControlKind::ParOver => "par",
        _ => kind.keyword(),
    }
}

fn rectangle_segments(x1: f64, y1: f64, x2: f64, y2: f64) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo {
            to: Point::new(x1, y1),
        },
        PathSegment::LineTo {
            to: Point::new(x2, y1),
        },
        PathSegment::LineTo {
            to: Point::new(x2, y2),
        },
        PathSegment::LineTo {
            to: Point::new(x1, y2),
        },
        PathSegment::Close,
    ]
}

fn plain_lines(raw: &str, context: &str) -> Result<Vec<String>> {
    if raw.trim().is_empty() {
        return Ok(vec![String::new()]);
    }
    split_html_br_lines(raw)
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

fn is_empty_lines(lines: &[String]) -> bool {
    lines.iter().all(|line| line.trim().is_empty())
}

fn unique_nodes(layout: &SequenceDiagramLayout) -> Result<HashMap<&str, &LayoutNode>> {
    let mut result = HashMap::with_capacity(layout.nodes.len());
    for node in &layout.nodes {
        if result.insert(node.id.as_str(), node).is_some() {
            return Err(invalid(format!(
                "duplicate Sequence layout node {}",
                node.id
            )));
        }
    }
    Ok(result)
}

fn unique_edges(layout: &SequenceDiagramLayout) -> Result<HashMap<&str, &LayoutEdge>> {
    let mut result = HashMap::with_capacity(layout.edges.len());
    for edge in &layout.edges {
        if result.insert(edge.id.as_str(), edge).is_some() {
            return Err(invalid(format!(
                "duplicate Sequence layout edge {}",
                edge.id
            )));
        }
    }
    Ok(result)
}

fn validate_node(node: &LayoutNode) -> Result<()> {
    if node.id.is_empty()
        || [node.x, node.y, node.width, node.height]
            .iter()
            .any(|value| !value.is_finite())
        || node.width <= 0.0
        || node.height <= 0.0
    {
        return Err(invalid(format!(
            "Sequence node {} has invalid geometry",
            node.id
        )));
    }
    Ok(())
}

fn validate_edge(edge: &LayoutEdge, min_points: usize) -> Result<()> {
    if edge.id.is_empty()
        || edge.points.len() < min_points
        || edge
            .points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(invalid(format!(
            "Sequence edge {} has invalid geometry",
            edge.id
        )));
    }
    Ok(())
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .iter()
        .any(|value| !value.is_finite())
        || bounds.max_x <= bounds.min_x
        || bounds.max_y <= bounds.min_y
    {
        return Err(invalid("Sequence bounds are invalid"));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SequenceMarkerEndpoint {
    point: Point,
    direction: Point,
}

#[derive(Debug, Clone, Copy)]
enum MarkerPosition {
    Start,
    End,
}

fn required_marker_endpoint(
    layout_points: &[crate::model::LayoutPoint],
    route_segments: &[PathSegment],
    position: MarkerPosition,
    message_id: &str,
) -> Result<SequenceMarkerEndpoint> {
    layout_marker_endpoint(layout_points, position)
        .or_else(|| route_marker_endpoint(route_segments, position))
        .ok_or_else(|| {
            unavailable(format!(
                "Sequence message `{message_id}` {position:?} marker has no non-zero tangent; DrawingList v1 cannot choose a lossless marker orientation"
            ))
        })
}

fn marker_endpoint_for(
    marker: SequenceMessageMarker,
    layout_points: &[crate::model::LayoutPoint],
    route_segments: &[PathSegment],
    position: MarkerPosition,
    message_id: &str,
) -> Result<Option<SequenceMarkerEndpoint>> {
    if marker == SequenceMessageMarker::None {
        return Ok(None);
    }
    required_marker_endpoint(layout_points, route_segments, position, message_id).map(Some)
}

fn layout_marker_endpoint(
    points: &[crate::model::LayoutPoint],
    position: MarkerPosition,
) -> Option<SequenceMarkerEndpoint> {
    let (point, direction) = match position {
        MarkerPosition::Start => {
            let start = points.first()?;
            let point = Point::new(start.x, start.y);
            let direction = points.iter().skip(1).find_map(|candidate| {
                unit_direction(point.x - candidate.x, point.y - candidate.y)
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
    Some(SequenceMarkerEndpoint { point, direction })
}

fn route_marker_endpoint(
    segments: &[PathSegment],
    position: MarkerPosition,
) -> Option<SequenceMarkerEndpoint> {
    let PathSegment::MoveTo { to: start } = segments.first()? else {
        return None;
    };
    let mut current = *start;
    let mut last_endpoint = None;

    for segment in &segments[1..] {
        let to = route_segment_endpoint(segment)?;
        if let Some(direction) = route_segment_direction(segment, current, to, position) {
            let endpoint = SequenceMarkerEndpoint {
                point: if matches!(position, MarkerPosition::Start) {
                    *start
                } else {
                    to
                },
                direction,
            };
            if matches!(position, MarkerPosition::Start) {
                return Some(endpoint);
            }
            last_endpoint = Some(endpoint);
        }
        current = to;
    }

    last_endpoint
}

fn route_segment_endpoint(segment: &PathSegment) -> Option<Point> {
    match segment {
        PathSegment::LineTo { to } | PathSegment::CubicTo { to, .. } => Some(*to),
        _ => None,
    }
}

fn route_segment_direction(
    segment: &PathSegment,
    from: Point,
    to: Point,
    position: MarkerPosition,
) -> Option<Point> {
    match (segment, position) {
        (PathSegment::LineTo { .. }, MarkerPosition::Start) => {
            unit_direction(from.x - to.x, from.y - to.y)
        }
        (PathSegment::LineTo { .. }, MarkerPosition::End) => {
            unit_direction(to.x - from.x, to.y - from.y)
        }
        (
            PathSegment::CubicTo {
                control1, control2, ..
            },
            MarkerPosition::Start,
        ) => [*control1, *control2, to]
            .into_iter()
            .find_map(|candidate| unit_direction(from.x - candidate.x, from.y - candidate.y)),
        (
            PathSegment::CubicTo {
                control1, control2, ..
            },
            MarkerPosition::End,
        ) => [*control2, *control1, from]
            .into_iter()
            .find_map(|candidate| unit_direction(to.x - candidate.x, to.y - candidate.y)),
        _ => None,
    }
}

fn unit_direction(x: f64, y: f64) -> Option<Point> {
    let length = x.hypot(y);
    if !length.is_finite() || length <= f64::EPSILON {
        None
    } else {
        Some(Point::new(x / length, y / length))
    }
}

fn parse_font_weight(value: &str) -> Result<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(400),
        "bold" | "bolder" => Ok(700),
        other => other
            .parse::<u16>()
            .map_err(|_| unavailable(format!("Sequence font-weight `{value}` is not portable"))),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "sequence".to_string(),
        reason: message.into(),
    }
}

fn portable_actor_link(
    actor: &SequenceActor,
    security: MermaidNavigationSecurity,
) -> Result<Option<String>> {
    let mut links = actor
        .links
        .iter()
        .map(|(label, value)| {
            let url = value
                .as_str()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .ok_or_else(|| {
                    unavailable(format!(
                        "Sequence participant `{}` has a non-string or empty link for `{label}`",
                        actor.name
                    ))
                })?;
            Ok((label.as_str(), url.to_string()))
        })
        .collect::<Result<Vec<_>>>()?;
    if links.len() > 1 {
        links.sort_by(|left, right| left.0.cmp(right.0));
        return Err(unavailable(format!(
            "Sequence participant `{}` has {} links; DrawingList v1 supports one semantic link",
            actor.name,
            links.len()
        )));
    }
    Ok(links.pop().and_then(|(_, url)| {
        let sanitized = merman_core::utils::sanitize_url(&url);
        prepare_mermaid_navigation_uri(&sanitized, security)
    }))
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MarkerPosition, layout_marker_endpoint, marker_endpoint_for, portable_actor_link,
        required_marker_endpoint,
    };
    use crate::{Error, model::LayoutPoint};
    use merman_core::diagrams::sequence::SequenceActor;
    use merman_core::svg_security::MermaidNavigationSecurity;
    use merman_display_list::{PathSegment, Point};
    use serde_json::{Value, json, map::Map};

    fn point(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }

    fn actor(links: Map<String, Value>) -> SequenceActor {
        SequenceActor {
            name: "alice".into(),
            description: "Alice".into(),
            actor_type: "participant".into(),
            wrap: false,
            links,
            properties: Map::new(),
        }
    }

    #[test]
    fn point_message_public_markers_preserve_concave_geometry_and_direction() {
        use merman_display_list::{
            Color, DrawingCommand, DrawingListLimits, DrawingListPolicy, DrawingResource, Paint,
        };

        for (message, right_angles) in [
            ("A-)B: forward", false),
            ("B-)A: reverse", false),
            ("A--)B: dotted", false),
            ("B--)A: dotted reverse", false),
            ("A-)A: self", false),
            ("A--)A: self right angles", true),
        ] {
            let source = format!("sequenceDiagram\nparticipant A\nparticipant B\n{message}\n");
            let parsed = merman_core::Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "theme": "base",
                    "themeVariables": {
                        "textColor": "#123456",
                        "signalColor": "#654321"
                    },
                    "sequence": { "rightAngles": right_angles }
                })))
                .parse_diagram_for_render_model_sync(&source, merman_core::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let rendered =
                crate::family::prepare(parsed, &crate::LayoutOptions::default(), session)
                    .unwrap()
                    .render_drawing_list(
                        DrawingListPolicy::VectorOnly,
                        DrawingListLimits::default(),
                    )
                    .unwrap();
            let document = rendered.document();
            let path_segments = |suffix: &str| {
                document
                    .resources
                    .iter()
                    .find_map(|resource| match resource {
                        DrawingResource::Path(path)
                            if path.id.as_str().starts_with("sequence.message.")
                                && path.id.as_str().ends_with(suffix) =>
                        {
                            Some(path.segments.as_slice())
                        }
                        _ => None,
                    })
                    .unwrap()
            };
            let route = path_segments(".route");
            let (end, previous) = match route.last().unwrap() {
                PathSegment::CubicTo { control2, to, .. } => (*to, *control2),
                PathSegment::LineTo { to } => {
                    let previous = match route[route.len() - 2] {
                        PathSegment::MoveTo { to } | PathSegment::LineTo { to } => to,
                        ref segment => panic!("unexpected previous route segment: {segment:?}"),
                    };
                    (*to, previous)
                }
                segment => panic!("unexpected route endpoint: {segment:?}"),
            };
            let length = (end.x - previous.x).hypot(end.y - previous.y);
            let direction =
                Point::new((end.x - previous.x) / length, (end.y - previous.y) / length);
            let marker = path_segments(".marker.end");
            assert_eq!(marker.len(), 5, "{message}");
            for (segment, (along, across)) in
                marker[..4]
                    .iter()
                    .zip([(3.75, 0.0), (-9.75, 9.0), (-2.25, 0.0), (-9.75, -9.0)])
            {
                let (PathSegment::MoveTo { to } | PathSegment::LineTo { to }) = segment else {
                    panic!("Point markers must be a concave polygon: {message}");
                };
                let expected = Point::new(
                    end.x + direction.x * along - direction.y * across,
                    end.y + direction.y * along + direction.x * across,
                );
                assert!((to.x - expected.x).abs() < 1e-9, "{message}: {segment:?}");
                assert!((to.y - expected.y).abs() < 1e-9, "{message}: {segment:?}");
            }
            assert!(matches!(marker.last(), Some(PathSegment::Close)));
            let marker_style = document
                .commands
                .iter()
                .find_map(|command| match command {
                    DrawingCommand::DrawPath { path, style }
                        if path.as_str().ends_with(".marker.end") =>
                    {
                        Some(style)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                marker_style.fill,
                Some(Paint::solid(Color::rgba(0x12, 0x34, 0x56, 255)))
            );
            assert!(marker_style.stroke.is_none(), "{message}");
        }
    }

    #[test]
    fn one_sequence_actor_link_becomes_normative_semantic_link() {
        let mut links = Map::new();
        links.insert("Docs".into(), json!("https://example.invalid/docs"));
        assert_eq!(
            portable_actor_link(&actor(links), MermaidNavigationSecurity::Sanitized)
                .unwrap()
                .as_deref(),
            Some("https://example.invalid/docs")
        );
    }

    #[test]
    fn unsafe_sequence_actor_link_is_omitted_like_strict_svg_navigation() {
        let mut links = Map::new();
        links.insert("Script".into(), json!("javascript:alert(1)"));
        assert_eq!(
            portable_actor_link(&actor(links), MermaidNavigationSecurity::Sanitized)
                .unwrap()
                .as_deref(),
            None
        );
    }

    #[test]
    fn multiple_or_malformed_sequence_actor_links_fail_closed() {
        let mut links = Map::new();
        links.insert("Docs".into(), json!("https://example.invalid/docs"));
        links.insert("Issues".into(), json!("https://example.invalid/issues"));
        assert!(portable_actor_link(&actor(links), MermaidNavigationSecurity::Sanitized).is_err());

        let mut links = Map::new();
        links.insert("Docs".into(), json!(42));
        assert!(portable_actor_link(&actor(links), MermaidNavigationSecurity::Sanitized).is_err());
    }

    #[test]
    fn message_marker_endpoints_skip_coincident_layout_points() {
        let start_points = [point(1.0, 2.0), point(1.0, 2.0), point(4.0, 6.0)];
        let end_points = [point(1.0, 2.0), point(4.0, 6.0), point(4.0, 6.0)];

        assert_eq!(
            layout_marker_endpoint(&start_points, MarkerPosition::Start)
                .expect("start marker endpoint")
                .direction,
            Point::new(-0.6, -0.8)
        );
        assert_eq!(
            layout_marker_endpoint(&end_points, MarkerPosition::End)
                .expect("end marker endpoint")
                .direction,
            Point::new(0.6, 0.8)
        );
    }

    #[test]
    fn self_message_markers_use_the_non_degenerate_render_route() {
        let layout_points = [point(5.0, 10.0), point(5.0, 10.0)];
        let route = [
            PathSegment::MoveTo {
                to: Point::new(5.0, 10.0),
            },
            PathSegment::LineTo {
                to: Point::new(35.0, 10.0),
            },
            PathSegment::LineTo {
                to: Point::new(35.0, 35.0),
            },
            PathSegment::LineTo {
                to: Point::new(5.0, 35.0),
            },
        ];

        assert_eq!(
            required_marker_endpoint(&layout_points, &route, MarkerPosition::Start, "self")
                .expect("self-message start marker endpoint"),
            super::SequenceMarkerEndpoint {
                point: Point::new(5.0, 10.0),
                direction: Point::new(-1.0, 0.0),
            }
        );
        assert_eq!(
            required_marker_endpoint(&layout_points, &route, MarkerPosition::End, "self")
                .expect("self-message end marker endpoint"),
            super::SequenceMarkerEndpoint {
                point: Point::new(5.0, 35.0),
                direction: Point::new(-1.0, 0.0),
            }
        );
    }

    #[test]
    fn message_markers_reject_a_fully_degenerate_route() {
        let layout_points = [point(1.0, 2.0), point(1.0, 2.0)];
        let route = [
            PathSegment::MoveTo {
                to: Point::new(1.0, 2.0),
            },
            PathSegment::LineTo {
                to: Point::new(1.0, 2.0),
            },
        ];

        assert!(
            marker_endpoint_for(
                merman_core::diagrams::sequence::SequenceMessageMarker::None,
                &layout_points,
                &route,
                MarkerPosition::End,
                "7",
            )
            .expect("a marker-free message does not need a tangent")
            .is_none()
        );
        for position in [MarkerPosition::Start, MarkerPosition::End] {
            let error = required_marker_endpoint(&layout_points, &route, position, "7")
                .expect_err("a fully degenerate Sequence marker must fail closed");
            assert!(matches!(
                error,
                Error::DrawingListUnavailable { ref family, .. } if family == "sequence"
            ));
            assert!(error.to_string().contains("no non-zero tangent"));
        }
    }
}
