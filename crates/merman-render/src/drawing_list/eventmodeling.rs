//! Renderer-neutral EventModeling adapter.
//!
//! EventModeling's layout already owns swimlane, frame, and relation geometry.  The SVG target
//! uses an HTML label shell for frame contents; the renderer-neutral projection expands that shell
//! into explicit title and data text runs so native hosts do not need an HTML interpreter.

use super::{
    EventModelingSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar,
    parse_font_families,
};
use crate::config::config_font_family_css;
use crate::drawing_list::flowchart::{polygon_path, rounded_rect_path};
use crate::drawing_list::support::{PortableStyleResolver, stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{EventModelingBoxLayout, EventModelingDiagramLayout};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::theme::PresentationTheme;
use crate::{Error, Result};
use merman_core::diagrams::eventmodeling::EventModelingDiagramRenderModel;
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Viewport,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

type EventModelingPair = FamilyPair<EventModelingDiagramRenderModel, EventModelingDiagramLayout>;

const BOX_RADIUS: f64 = 3.0;
const BOX_TEXT_PADDING: f64 = 10.0;
const TEXT_FONT_SIZE: f64 = 16.0;
const LABEL_LINE_HEIGHT: f64 = 19.0;

pub(crate) fn build_eventmodeling_document(
    pair: &EventModelingPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    EventModelingBuilder::new(pair, metadata, policy, session)?.build()
}

struct EventModelingBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a EventModelingDiagramRenderModel,
    layout: &'a EventModelingDiagramLayout,
    font: FontDescriptor,
    code_font: FontDescriptor,
    text_obligation: TextObligation,
    swimlane_fill: Color,
    swimlane_stroke: Color,
    relation_stroke: Color,
    arrowhead_fill: Color,
    text_color: Color,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> EventModelingBuilder<'a> {
    fn new(
        pair: &'a EventModelingPair,
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
                "themeCSS is an unresolved SVG cascade input for EventModeling DrawingList output",
            ));
        }

        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout, model)?;
        let theme = PresentationTheme::new(config).eventmodeling();
        let styles = PortableStyleResolver::new("eventmodeling");
        let font_families = parse_font_families(config_font_family_css(config));
        let font = FontDescriptor {
            families: font_families,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let code_font = FontDescriptor {
            families: vec!["monospace".to_string()],
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        Ok(Self {
            metadata,
            session,
            policy,
            model,
            layout,
            swimlane_fill: styles.color("emSwimlaneBackground", &theme.swimlane_background_fill)?,
            swimlane_stroke: styles.color(
                "emSwimlaneBackgroundStroke",
                &theme.swimlane_background_stroke,
            )?,
            relation_stroke: styles.color("emRelationStroke", &theme.relation_stroke)?,
            arrowhead_fill: styles.color("emArrowhead", &theme.arrowhead_fill)?,
            text_color: styles.color("textColor", &theme.text_color)?,
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            font,
            code_font,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "eventmodeling.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.semantics.push(SemanticAnnotation {
            id: "eventmodeling.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| self.model.title.clone())
                .or_else(|| self.metadata.title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_background()?;
        for index in 0..self.layout.swimlanes.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_swimlane(index)?;
        }
        for index in 0..self.layout.boxes.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_box(index)?;
        }
        for index in 0..self.layout.relations.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_relation(index)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                self.layout.viewbox_x,
                self.layout.viewbox_y,
                self.layout.total_width,
                self.layout.total_height,
            )),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-eventmodeling".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "text_mode": "title_and_data_host_text",
                    "foreign_object": "expanded_to_text_runs",
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::EventModeling,
                body: SvgStructureBody::EventModeling(EventModelingSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        self.add_path(
            "eventmodeling.background",
            rectangle_path(
                self.layout.viewbox_x,
                self.layout.viewbox_y,
                self.layout.total_width,
                self.layout.total_height,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )
    }

    fn emit_swimlane(&mut self, index: usize) -> Result<()> {
        let swimlane = self
            .layout
            .swimlanes
            .get(index)
            .ok_or_else(|| invalid(format!("missing EventModeling swimlane {index}")))?;
        let semantic_id = format!("eventmodeling.swimlane.{}", swimlane.index);
        self.semantic_classes
            .insert(semantic_id.clone(), "em-swimlane".to_string());
        self.text_classes
            .insert(semantic_id.clone(), "em-swimlane-label".to_string());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.add_path(
            format!("{semantic_id}.shape"),
            rounded_rect_path(
                swimlane.x,
                swimlane.y,
                swimlane.width,
                swimlane.height,
                BOX_RADIUS,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.swimlane_fill)),
                stroke: Some(stroke(self.swimlane_stroke, 1.0)),
            },
        )?;

        let label_origin = Point::new(swimlane.x + 30.0, swimlane.y + 30.0);
        let label_style = MeasurementTextStyle {
            font_size: TEXT_FONT_SIZE,
            font_weight: Some("700".to_string()),
            ..Default::default()
        };
        let (label_width, label_height) = self.measure_text(&swimlane.label, &label_style);
        self.commands.push(DrawingCommand::DrawText {
            run: TextRun {
                text: swimlane.label.clone(),
                origin: label_origin,
                bounds: Rect::new(
                    label_origin.x,
                    label_origin.y - label_height,
                    label_width.max(1.0),
                    label_height.max(1.0),
                ),
                style: TextStyle {
                    font: FontDescriptor {
                        weight: 700,
                        ..self.font.clone()
                    },
                    font_size: TEXT_FONT_SIZE,
                    letter_spacing: 0.0,
                    line_height: LABEL_LINE_HEIGHT,
                    fill: Paint::solid(self.text_color),
                },
                anchor: TextAnchor::Start,
                baseline: TextBaseline::Alphabetic,
                direction: TextDirection::Auto,
                language: None,
                obligation: self.text_obligation.clone(),
            },
        });
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(swimlane.label.clone()),
            description: swimlane.namespace.clone(),
            link: None,
        });
        Ok(())
    }

    fn emit_box(&mut self, index: usize) -> Result<()> {
        let box_layout = self
            .layout
            .boxes
            .get(index)
            .ok_or_else(|| invalid(format!("missing EventModeling box {index}")))?;
        let semantic_id = format!("eventmodeling.box.{}", box_layout.index);
        self.semantic_classes
            .insert(semantic_id.clone(), "em-box".to_string());
        self.text_classes
            .insert(semantic_id.clone(), "em-box-label".to_string());
        let fill =
            PortableStyleResolver::new("eventmodeling").color("box.fill", &box_layout.fill)?;
        let stroke_color =
            PortableStyleResolver::new("eventmodeling").color("box.stroke", &box_layout.stroke)?;

        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.add_path(
            format!("{semantic_id}.shape"),
            rounded_rect_path(
                box_layout.x,
                box_layout.y,
                box_layout.width,
                box_layout.height,
                BOX_RADIUS,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(fill)),
                stroke: Some(stroke(stroke_color, 1.0)),
            },
        )?;
        self.emit_box_text(box_layout, &semantic_id)?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        let (title, data) = box_text_parts(&box_layout.text);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(title),
            description: (!data.is_empty()).then(|| data.join("\n")),
            link: None,
        });
        Ok(())
    }

    fn emit_box_text(
        &mut self,
        box_layout: &EventModelingBoxLayout,
        _semantic_id: &str,
    ) -> Result<()> {
        let (title, data_lines) = box_text_parts(&box_layout.text);
        let title_measurement = MeasurementTextStyle {
            font_size: TEXT_FONT_SIZE,
            font_weight: Some("700".to_string()),
            ..Default::default()
        };
        let code_measurement = MeasurementTextStyle {
            font_size: TEXT_FONT_SIZE,
            font_family: Some("monospace".to_string()),
            ..Default::default()
        };
        let title_metrics = self.measure_text(&title, &title_measurement);
        let code_metrics: Vec<_> = data_lines
            .iter()
            .map(|line| self.measure_text(line, &code_measurement))
            .collect();
        let has_data = !data_lines.is_empty();
        let trailing_break = has_data
            && self
                .model
                .frames
                .iter()
                .find(|frame| frame.name == box_layout.frame_name)
                .is_some_and(|frame| frame.data_reference.is_some());
        let line_count = if has_data {
            1 + 2 + data_lines.len() + usize::from(trailing_break)
        } else {
            1
        };
        let content_height = line_count as f64 * LABEL_LINE_HEIGHT;
        let content_top = box_layout.y + box_layout.height / 2.0 - content_height / 2.0;
        let center_x = box_layout.x + box_layout.width / 2.0;
        let title_y = content_top + LABEL_LINE_HEIGHT / 2.0;
        self.commands.push(DrawingCommand::DrawText {
            run: TextRun {
                text: title,
                origin: Point::new(center_x, title_y),
                bounds: Rect::new(
                    box_layout.x + BOX_TEXT_PADDING,
                    title_y - title_metrics.1 / 2.0,
                    (box_layout.width - 2.0 * BOX_TEXT_PADDING).max(1.0),
                    title_metrics.1.max(1.0),
                ),
                style: TextStyle {
                    font: FontDescriptor {
                        weight: 700,
                        ..self.font.clone()
                    },
                    font_size: TEXT_FONT_SIZE,
                    letter_spacing: 0.0,
                    line_height: LABEL_LINE_HEIGHT,
                    fill: Paint::solid(self.text_color),
                },
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Middle,
                direction: TextDirection::Auto,
                language: None,
                obligation: self.text_obligation.clone(),
            },
        });

        if has_data {
            let data_x = box_layout.x + BOX_TEXT_PADDING;
            for (line_index, (line, (_, line_height))) in
                data_lines.iter().zip(code_metrics.iter()).enumerate()
            {
                let y = content_top + (3 + line_index) as f64 * LABEL_LINE_HEIGHT
                    - LABEL_LINE_HEIGHT / 2.0;
                self.commands.push(DrawingCommand::DrawText {
                    run: TextRun {
                        text: line.clone(),
                        origin: Point::new(data_x, y),
                        bounds: Rect::new(
                            data_x,
                            y - *line_height / 2.0,
                            (box_layout.width - 2.0 * BOX_TEXT_PADDING).max(1.0),
                            (*line_height).max(1.0),
                        ),
                        style: TextStyle {
                            font: self.code_font.clone(),
                            font_size: TEXT_FONT_SIZE,
                            letter_spacing: 0.0,
                            line_height: LABEL_LINE_HEIGHT,
                            fill: Paint::solid(self.text_color),
                        },
                        anchor: TextAnchor::Start,
                        baseline: TextBaseline::Middle,
                        direction: TextDirection::Auto,
                        language: None,
                        obligation: self.text_obligation.clone(),
                    },
                });
            }
        }
        Ok(())
    }

    fn emit_relation(&mut self, index: usize) -> Result<()> {
        let relation = self
            .layout
            .relations
            .get(index)
            .ok_or_else(|| invalid(format!("missing EventModeling relation {index}")))?;
        let semantic_id = format!("eventmodeling.relation.{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "em-relation".to_string());
        self.path_classes
            .insert(format!("{semantic_id}.line"), "em-relation".to_string());
        self.path_classes.insert(
            format!("{semantic_id}.arrowhead"),
            "em-arrowhead".to_string(),
        );
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.add_path(
            format!("{semantic_id}.line"),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(relation.x1, relation.y1),
                },
                PathSegment::LineTo {
                    to: Point::new(relation.x2, relation.y2),
                },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.relation_stroke, 1.0)),
            },
        )?;
        self.add_path(
            format!("{semantic_id}.arrowhead"),
            arrowhead_path(relation.x1, relation.y1, relation.x2, relation.y2)?,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.arrowhead_fill)),
                stroke: None,
            },
        )?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: None,
            description: Some(format!(
                "{} → {}",
                relation.source_frame, relation.target_frame
            )),
            link: None,
        });
        Ok(())
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("EventModeling path has no geometry"));
        }
        let id = ResourceId::new(id.into());
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        Ok(())
    }

    fn measure_text(&self, text: &str, style: &MeasurementTextStyle) -> (f64, f64) {
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        (
            measurer
                .measure_svg_simple_text_bbox_width_px(text, style)
                .max(0.0),
            measurer
                .measure_svg_simple_text_bbox_height_px(text, style)
                .max(1.0),
        )
    }
}

fn box_text_parts(text: &str) -> (String, Vec<String>) {
    let mut lines = text.lines();
    let title = lines.next().unwrap_or(text).to_string();
    let rest = lines.collect::<Vec<_>>().join("\n");
    let data = normalize_data_text(&rest);
    let data_lines = if data.is_empty() {
        Vec::new()
    } else {
        data.lines().map(str::to_string).collect()
    };
    (title, data_lines)
}

fn normalize_data_text(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_outer_braces = trimmed
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(trimmed);
    without_outer_braces.trim().to_string()
}

fn rectangle_path(x: f64, y: f64, width: f64, height: f64) -> Vec<PathSegment> {
    polygon_path(&[
        Point::new(x, y),
        Point::new(x + width, y),
        Point::new(x + width, y + height),
        Point::new(x, y + height),
    ])
}

fn arrowhead_path(x1: f64, y1: f64, x2: f64, y2: f64) -> Result<Vec<PathSegment>> {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = dx.hypot(dy);
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(invalid("EventModeling relation has no arrowhead tangent"));
    }
    let ux = dx / length;
    let uy = dy / length;
    let nx = -uy;
    let ny = ux;
    let base_x = x2 - ux * 10.0;
    let base_y = y2 - uy * 10.0;
    Ok(polygon_path(&[
        Point::new(x2, y2),
        Point::new(base_x + nx * 3.5, base_y + ny * 3.5),
        Point::new(base_x - nx * 3.5, base_y - ny * 3.5),
    ]))
}

fn validate_layout(
    layout: &EventModelingDiagramLayout,
    model: &EventModelingDiagramRenderModel,
) -> Result<()> {
    if layout.boxes.len() != model.frames.len() {
        return Err(invalid(format!(
            "EventModeling layout has {} boxes for {} frames",
            layout.boxes.len(),
            model.frames.len()
        )));
    }
    let values = [
        layout.total_width,
        layout.total_height,
        layout.viewbox_x,
        layout.viewbox_y,
        layout.padding,
    ];
    if values.iter().any(|value| !value.is_finite())
        || layout.total_width <= 0.0
        || layout.total_height <= 0.0
        || layout.padding < 0.0
    {
        return Err(invalid("EventModeling root viewport is invalid"));
    }
    for swimlane in &layout.swimlanes {
        validate_rect(
            swimlane.x,
            swimlane.y,
            swimlane.width,
            swimlane.height,
            "swimlane",
        )?;
    }
    for box_layout in &layout.boxes {
        validate_rect(
            box_layout.x,
            box_layout.y,
            box_layout.width,
            box_layout.height,
            "box",
        )?;
    }
    for relation in &layout.relations {
        let values = [relation.x1, relation.y1, relation.x2, relation.y2];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(invalid("EventModeling relation geometry is invalid"));
        }
    }
    Ok(())
}

fn validate_rect(x: f64, y: f64, width: f64, height: f64, kind: &str) -> Result<()> {
    if [x, y, width, height].iter().any(|value| !value.is_finite()) || width <= 0.0 || height <= 0.0
    {
        return Err(invalid(format!("EventModeling {kind} geometry is invalid")));
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
        family: "eventmodeling".to_string(),
        reason: message.into(),
    }
}
