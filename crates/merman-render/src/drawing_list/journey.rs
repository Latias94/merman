//! Renderer-neutral Journey adapter.
//!
//! Journey layout owns the chart's final coordinates.  This adapter makes the visible circles,
//! rounded rectangles, faces, labels, and axis line explicit so native renderers do not need to
//! replay Mermaid's HTML/SVG wrapper tree or CSS selectors.

use super::{
    JourneySvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families_for,
};
use crate::config::{config_font_family_css_raw, config_theme_font_size_css_or_root_number_px};
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::flowchart::{ellipse_path, polygon_path, rounded_rect_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::journey::{JOURNEY_FACE_RADIUS_PX, JOURNEY_TITLE_EXTRA_HEIGHT_PX, JourneyConfigView};
use crate::model::{
    Bounds, JourneyActorLegendItemLayout, JourneyDiagramLayout, JourneyLineLayout,
    JourneyMouthKind, JourneySectionLayout, JourneyTaskActorCircleLayout, JourneyTaskLayout,
};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::journey::JourneyDiagramRenderModel;
use merman_display_list::{
    Color, DrawingCommand, DrawingListLimits, DrawingListPolicy, FillRule, FontDescriptor,
    FontStyle, Paint, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, StrokeStyle, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun,
    TextStyle as DisplayTextStyle, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type JourneyPair = FamilyPair<JourneyDiagramRenderModel, JourneyDiagramLayout>;

const BORDER_COLOR: Color = Color::rgba(102, 102, 102, 255);
const FACE_STROKE_COLOR: Color = Color::rgba(153, 153, 153, 255);
const BLACK: Color = Color::rgba(0, 0, 0, 255);
const EYE_RADIUS_PX: f64 = 1.5;
const EYE_OFFSET_PX: f64 = JOURNEY_FACE_RADIUS_PX / 3.0;

pub(crate) fn build_journey_document(
    pair: &JourneyPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: DrawingListLimits,
    session: &RenderSession,
) -> Result<RenderDocument> {
    JourneyBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct JourneyBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    document: DrawingListBuilder<'a>,
    model: &'a JourneyDiagramRenderModel,
    layout: &'a JourneyDiagramLayout,
    task_font: FontDescriptor,
    task_font_size: f64,
    legend_font: FontDescriptor,
    legend_font_size: f64,
    title_font: FontDescriptor,
    title_font_size: f64,
    text_color: Color,
    line_color: Color,
    face_color: Color,
    title_color: Color,
    actor_color_overrides: Vec<Option<Color>>,
    text_obligation: TextObligation,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
}

struct TextEmitSpec {
    origin: Point,
    font_size: f64,
    weight: u16,
    color: Color,
    font: FontDescriptor,
    anchor: TextAnchor,
    baseline: TextBaseline,
}

impl<'a> JourneyBuilder<'a> {
    fn new(
        pair: &'a JourneyPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;

        let theme = crate::svg::render_theme::PresentationTheme::new(config).journey();
        let settings = JourneyConfigView::new(config).render_settings();
        let styles = PortableStyleResolver::new("journey");
        let text_color = styles.color("textColor", &theme.text_color)?;
        let line_color = text_color;
        let face_color = styles.color("faceColor", &theme.face_color)?;
        let title_color = if settings.title_color.trim().is_empty() {
            text_color
        } else {
            styles.color("titleColor", &settings.title_color)?
        };

        let task_font_family = settings
            .task_text_style
            .font_family
            .clone()
            .filter(|family| !family.trim().is_empty())
            .unwrap_or_else(|| config_font_family_css_raw(config));
        let task_font_size = settings.task_text_style.font_size.max(1.0);
        let raw_default_font_family = config_font_family_css_raw(config);
        let title_font_family = if settings.title_font_family.trim().is_empty() {
            raw_default_font_family.clone()
        } else {
            settings.title_font_family.clone()
        };
        let title_font_size =
            crate::mermaid_style::parse_css_font_size_px(&settings.title_font_size, task_font_size)
                .ok_or_else(|| {
                    unavailable(format!(
                        "Journey titleFontSize `{}` is not a portable CSS font size",
                        settings.title_font_size
                    ))
                })?;
        let legend_font_size = config_theme_font_size_css_or_root_number_px(config, 16.0).max(1.0);

        let actor_color_overrides = theme
            .actor_colors
            .iter()
            .enumerate()
            .map(|(index, color)| {
                color
                    .as_deref()
                    .map(|value| styles.color(&format!("actor{index}"), value))
                    .transpose()
            })
            .collect::<Result<Vec<_>>>()?;
        let legend_font_family = raw_default_font_family;
        let mut document = DrawingListBuilder::new(policy, limits, session);
        document.push_control(DrawingCommand::Save)?;
        document.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "journey.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            document,
            model,
            layout,
            task_font: FontDescriptor {
                families: parse_font_families_for(task_font_family, RenderFamilyKind::Journey)?,
                weight: 400,
                style: FontStyle::Normal,
                postscript_name: None,
                resource: None,
            },
            task_font_size,
            legend_font: FontDescriptor {
                families: parse_font_families_for(legend_font_family, RenderFamilyKind::Journey)?,
                weight: 400,
                style: FontStyle::Normal,
                postscript_name: None,
                resource: None,
            },
            legend_font_size,
            title_font: FontDescriptor {
                families: parse_font_families_for(title_font_family, RenderFamilyKind::Journey)?,
                weight: 700,
                style: FontStyle::Normal,
                postscript_name: None,
                resource: None,
            },
            title_font_size,
            text_color,
            line_color,
            face_color,
            title_color,
            actor_color_overrides,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let title = self
            .layout
            .title
            .as_deref()
            .or(self.metadata.title.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty());

        self.emit_actor_legend()?;
        self.emit_sections()?;
        self.emit_tasks()?;
        if let Some(title) = title {
            self.emit_title(title)?;
        }
        self.emit_activity_line()?;

        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_control(DrawingCommand::Restore)?;
        // Duplicate potentially large authored titles only after their visible text is admitted.
        self.document.push_semantic(SemanticAnnotation {
            id: "journey.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| title.map(str::to_string))
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        })?;

        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Journey layout did not provide root bounds"))?;
        let title_from_metadata = self.layout.title.is_none() && title.is_some();
        let max_y = if title_from_metadata {
            bounds.max_y + JOURNEY_TITLE_EXTRA_HEIGHT_PX
        } else {
            bounds.max_y
        };
        let viewport = Rect::new(
            bounds.min_x,
            bounds.min_y,
            bounds.max_x - bounds.min_x,
            max_y - bounds.min_y,
        );
        let document = self.document.finish(
            Viewport::new(viewport),
            BTreeMap::from([(
                "x-merman-journey".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "text_mode": "plain_host_text",
                    "markup": "br_only",
                    "use_max_width": self.layout.use_max_width,
                    "actor_count": self.layout.actor_legend.len(),
                    "title_from_metadata": title_from_metadata,
                }),
            )]),
        )?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Journey,
                body: SvgStructureBody::Journey(JourneySvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    width: self.layout.width,
                    svg_height: self.layout.svg_height
                        + if title_from_metadata {
                            JOURNEY_TITLE_EXTRA_HEIGHT_PX
                        } else {
                            0.0
                        },
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                    dom_ids: self.dom_ids,
                }),
            },
        })
    }

    fn emit_actor_legend(&mut self) -> Result<()> {
        let legend_font = self.legend_font.clone();
        for (index, item) in self.layout.actor_legend.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            validate_actor_legend(item)?;
            let semantic_id = format!("journey.actor.{index}");
            self.semantic_classes
                .insert(semantic_id.clone(), "legend".to_string());
            self.text_classes
                .insert(semantic_id.clone(), "legend".to_string());
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.path_classes.insert(
                format!("{semantic_id}.circle"),
                format!("actor-{}", item.pos),
            );
            self.add_path(
                format!("{semantic_id}.circle"),
                ellipse_path(item.circle_cx, item.circle_cy, item.circle_r, item.circle_r),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.actor_color(item.pos, &item.color)?)),
                    stroke: Some(stroke(BLACK, 1.0)),
                },
            )?;
            for (line_index, line) in item.label_lines.iter().enumerate() {
                self.session.checkpoint(OperationPhase::Emit)?;
                if [line.x, line.y, line.tspan_x, line.text_margin]
                    .iter()
                    .any(|value| !value.is_finite())
                {
                    return Err(invalid("Journey actor legend label geometry is invalid"));
                }
                self.emit_text(
                    format!("{semantic_id}.label.{line_index}"),
                    &line.text,
                    TextEmitSpec {
                        origin: Point::new(line.tspan_x, line.y),
                        font_size: self.legend_font_size,
                        weight: 400,
                        color: self.text_color,
                        font: legend_font.clone(),
                        anchor: TextAnchor::Start,
                        baseline: TextBaseline::Alphabetic,
                    },
                )?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Label,
                title: Some(item.actor.clone()),
                description: Some("Journey actor".to_string()),
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_sections(&mut self) -> Result<()> {
        for (index, section) in self.layout.sections.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            validate_section(section)?;
            let semantic_id = format!("journey.section.{index}");
            let section_class = format!("journey-section section-type-{}", section.num);
            self.semantic_classes
                .insert(semantic_id.clone(), section_class.clone());
            self.text_classes
                .insert(semantic_id.clone(), section_class.clone());
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            let fill =
                PortableStyleResolver::new("journey").color("section.fill", &section.fill)?;
            self.path_classes
                .insert(format!("{semantic_id}.background"), section_class);
            self.add_path(
                format!("{semantic_id}.background"),
                rounded_rect_path(
                    section.x + section.width / 2.0,
                    section.y + section.height / 2.0,
                    section.width,
                    section.height,
                    3.0,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: Some(stroke(BORDER_COLOR, 1.0)),
                },
            )?;
            self.emit_box_text(
                &format!("{semantic_id}.label"),
                &section.section,
                section.x,
                section.y,
                section.width,
                section.height,
            )?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(visible_journey_text(&section.section, self.session)?),
                description: Some("Journey section".to_string()),
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_tasks(&mut self) -> Result<()> {
        for (index, task) in self.layout.tasks.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            validate_task(task)?;
            let semantic_id = format!("journey.task.{index}");
            self.semantic_classes
                .insert(semantic_id.clone(), "task".to_string());
            self.text_classes
                .insert(semantic_id.clone(), "task".to_string());
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.dom_ids
                .insert(format!("{semantic_id}.line"), task.line_id.clone());
            self.path_classes
                .insert(format!("{semantic_id}.line"), "task-line".to_string());
            self.add_path(
                format!("{semantic_id}.line"),
                line_path(
                    Point::new(task.line_x1, task.line_y1),
                    Point::new(task.line_x2, task.line_y2),
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(StrokeStyle {
                        dash_array: vec![4.0, 2.0],
                        ..stroke(self.line_color, 1.0)
                    }),
                },
            )?;

            let face_y = task
                .face_cy
                .ok_or_else(|| unavailable("Journey score produced non-finite face geometry"))?;
            self.path_classes
                .insert(format!("{semantic_id}.face"), "face".to_string());
            self.add_path(
                format!("{semantic_id}.face"),
                ellipse_path(
                    task.face_cx,
                    face_y,
                    JOURNEY_FACE_RADIUS_PX,
                    JOURNEY_FACE_RADIUS_PX,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.face_color)),
                    stroke: Some(stroke(FACE_STROKE_COLOR, 1.0)),
                },
            )?;
            self.add_path(
                format!("{semantic_id}.face.left_eye"),
                ellipse_path(
                    task.face_cx - EYE_OFFSET_PX,
                    face_y - EYE_OFFSET_PX,
                    EYE_RADIUS_PX,
                    EYE_RADIUS_PX,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(BORDER_COLOR)),
                    stroke: Some(stroke(BORDER_COLOR, 2.0)),
                },
            )?;
            self.add_path(
                format!("{semantic_id}.face.right_eye"),
                ellipse_path(
                    task.face_cx + EYE_OFFSET_PX,
                    face_y - EYE_OFFSET_PX,
                    EYE_RADIUS_PX,
                    EYE_RADIUS_PX,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(BORDER_COLOR)),
                    stroke: Some(stroke(BORDER_COLOR, 2.0)),
                },
            )?;
            self.emit_mouth(
                &format!("{semantic_id}.face.mouth"),
                task.mouth.clone(),
                task.face_cx,
                face_y,
            )?;

            let fill = PortableStyleResolver::new("journey").color("task.fill", &task.fill)?;
            self.path_classes.insert(
                format!("{semantic_id}.background"),
                format!("task task-type-{}", task.num),
            );
            self.add_path(
                format!("{semantic_id}.background"),
                rounded_rect_path(
                    task.x + task.width / 2.0,
                    task.y + task.height / 2.0,
                    task.width,
                    task.height,
                    3.0,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: Some(stroke(BORDER_COLOR, 1.0)),
                },
            )?;

            for (actor_index, actor) in task.actor_circles.iter().enumerate() {
                self.emit_task_actor(&semantic_id, actor_index, actor)?;
            }
            self.emit_box_text(
                &format!("{semantic_id}.label"),
                &task.task,
                task.x,
                task.y,
                task.width,
                task.height,
            )?;

            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(visible_journey_text(&task.task, self.session)?),
                description: Some(format!(
                    "Journey task in {} with score {}",
                    task.section, task.score
                )),
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_task_actor(
        &mut self,
        task_id: &str,
        index: usize,
        actor: &JourneyTaskActorCircleLayout,
    ) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)?;
        validate_task_actor(actor)?;
        let semantic_id = format!("{task_id}.actor.{index}");
        self.path_classes.insert(
            format!("{semantic_id}.circle"),
            format!("actor-{}", actor.pos),
        );
        self.add_path(
            format!("{semantic_id}.circle"),
            ellipse_path(actor.cx, actor.cy, actor.r, actor.r),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.actor_color(actor.pos, &actor.color)?)),
                stroke: Some(stroke(BLACK, 1.0)),
            },
        )?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(actor.actor.clone()),
            description: Some("Journey task actor".to_string()),
            link: None,
        })?;
        Ok(())
    }

    fn emit_mouth(&mut self, id: &str, mouth: JourneyMouthKind, cx: f64, cy: f64) -> Result<()> {
        self.path_classes
            .insert(id.to_string(), "mouth".to_string());
        match mouth {
            JourneyMouthKind::Smile => self.add_path(
                id,
                smile_path(cx, cy + 2.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(BLACK)),
                    stroke: Some(stroke(BORDER_COLOR, 1.0)),
                },
            ),
            JourneyMouthKind::Sad => self.add_path(
                id,
                sad_path(cx, cy + 7.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(BLACK)),
                    stroke: Some(stroke(BORDER_COLOR, 1.0)),
                },
            ),
            JourneyMouthKind::Ambivalent => self.add_path(
                id,
                line_path(
                    Point::new(cx - 5.0, cy + 7.0),
                    Point::new(cx + 5.0, cy + 7.0),
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(BORDER_COLOR, 1.0)),
                },
            ),
        }
    }

    fn emit_box_text(
        &mut self,
        prefix: &str,
        text: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<()> {
        let mut line_count = 0usize;
        for line in JourneyLines::new(text) {
            self.session.checkpoint(OperationPhase::Emit)?;
            validate_journey_line(line)?;
            line_count += 1;
        }
        let line_count = line_count.max(1) as f64;
        let center = Point::new(x + width / 2.0, y + height / 2.0);
        let task_font = self.task_font.clone();
        for (index, line) in JourneyLines::new(text).enumerate() {
            let offset =
                index as f64 * self.task_font_size - self.task_font_size * (line_count - 1.0) / 2.0;
            self.emit_text(
                format!("{prefix}.{index}"),
                line,
                TextEmitSpec {
                    origin: Point::new(center.x, center.y + offset),
                    font_size: self.task_font_size,
                    weight: 400,
                    color: self.text_color,
                    font: task_font.clone(),
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
        }
        Ok(())
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        if normalized_text_parts(title).next().is_none() {
            return Ok(());
        }
        let title_font = self.title_font.clone();
        let title_origin = Point::new(self.layout.title_x, self.layout.title_y);
        self.emit_text(
            "journey.title",
            title,
            TextEmitSpec {
                origin: title_origin,
                font_size: self.title_font_size,
                weight: 700,
                color: self.title_color,
                font: title_font,
                anchor: TextAnchor::Start,
                baseline: TextBaseline::Alphabetic,
            },
        )?;
        self.document.push_semantic(SemanticAnnotation {
            id: "journey.title".to_string(),
            role: SemanticRole::Label,
            title: Some(svg_plain_text(title)),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_activity_line(&mut self) -> Result<()> {
        let line = &self.layout.activity_line;
        validate_line(line)?;
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "journey.activity".to_string(),
            })?;
        self.add_path(
            "journey.activity.line",
            line_path(Point::new(line.x1, line.y1), Point::new(line.x2, line.y2)),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.line_color, 4.0)),
            },
        )?;
        self.add_path(
            "journey.activity.arrowhead",
            arrowhead_path(line.x1, line.y1, line.x2, line.y2, 4.0)?,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(BLACK)),
                stroke: None,
            },
        )?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: "journey.activity".to_string(),
            role: SemanticRole::Edge,
            title: None,
            description: Some("Journey activity axis".to_string()),
            link: None,
        })?;
        Ok(())
    }

    fn emit_text(&mut self, _id: impl Into<String>, value: &str, spec: TextEmitSpec) -> Result<()> {
        let TextEmitSpec {
            origin,
            font_size,
            weight,
            color,
            font,
            anchor,
            baseline,
        } = spec;
        let parts = normalized_text_parts(value);
        if parts.clone().next().is_none() {
            return Ok(());
        }
        let session = self.session;
        let obligation = &self.text_obligation;
        self.document.draw_host_text_iter(parts, |text| {
            let measurement_style = MeasurementTextStyle {
                font_family: Some(font.families.join(", ")),
                font_size,
                font_weight: Some(weight.to_string()),
                font_style: None,
            };
            let measurer = session
                .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
            let width = measurer.measure_svg_raw_text_bbox_width_px(&text, &measurement_style);
            let height = measurer.measure_svg_simple_text_bbox_height_px(&text, &measurement_style);
            session.checkpoint(OperationPhase::Emit)?;
            if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
                return Err(invalid("Journey text measurement returned invalid bounds"));
            }
            let width = width.max(1.0);
            let height = height.max(1.0);
            let left = match anchor {
                TextAnchor::Start => origin.x,
                TextAnchor::Middle => origin.x - width / 2.0,
                TextAnchor::End => origin.x - width,
            };
            let top = match baseline {
                TextBaseline::Alphabetic => origin.y - height,
                TextBaseline::Middle | TextBaseline::Central => origin.y - height / 2.0,
                TextBaseline::Hanging => origin.y,
                TextBaseline::Ideographic
                | TextBaseline::TextBeforeEdge
                | TextBaseline::TextAfterEdge => origin.y - height,
            };
            Ok(TextRun {
                text,
                origin,
                bounds: Rect::new(left, top, width, height),
                style: DisplayTextStyle {
                    font: FontDescriptor { weight, ..font },
                    font_size,
                    letter_spacing: 0.0,
                    line_height: font_size,
                    fill: Paint::solid(color),
                    stroke: None,
                    paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                },
                anchor,
                baseline,
                direction: TextDirection::Auto,
                language: None,
                obligation: obligation.clone(),
            })
        })
    }

    fn actor_color(&self, pos: i64, layout_color: &str) -> Result<Color> {
        if pos < 0 {
            return Err(invalid("Journey actor position is negative"));
        }
        self.actor_color_overrides
            .get(pos as usize)
            .and_then(|color| *color)
            .map_or_else(
                || PortableStyleResolver::new("journey").color("actor.fill", layout_color),
                Ok,
            )
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("Journey path has no geometry"));
        }
        // Journey primitives have fixed segment counts; no input-sized route is collected here.
        self.document
            .draw_path(ResourceId::new(id.into()), segments, style)
    }
}

/// Splits authored breaks without materializing all labels before their text budgets are checked.
struct JourneyLines<'a> {
    remaining: Option<&'a str>,
}

impl<'a> JourneyLines<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            remaining: Some(text),
        }
    }
}

impl<'a> Iterator for JourneyLines<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let text = self.remaining.take()?;
        for (start, _) in text.match_indices('<') {
            if let Some(end) = journey_break_tag_end(text.as_bytes(), start) {
                self.remaining = Some(&text[end..]);
                return Some(&text[..start]);
            }
        }
        Some(text)
    }
}

fn normalized_text_parts(text: &str) -> impl Iterator<Item = &str> + Clone {
    text.split(crate::text::is_html_collapsible_ascii_whitespace)
        .filter(|word| !word.is_empty())
        .enumerate()
        .flat_map(|(index, word)| {
            [(index > 0).then_some(" "), Some(word)]
                .into_iter()
                .flatten()
        })
}

fn validate_journey_line(line: &str) -> Result<()> {
    if line.contains(['<', '>']) {
        return Err(unavailable(
            "Journey labels contain HTML markup other than <br>, which DrawingList v1 cannot preserve",
        ));
    }
    Ok(())
}

fn visible_journey_text(text: &str, session: &RenderSession) -> Result<String> {
    let mut visible = String::new();
    for (index, line) in JourneyLines::new(text).enumerate() {
        session.checkpoint(OperationPhase::Emit)?;
        if index > 0 {
            visible
                .try_reserve(1)
                .map_err(|_| Error::DrawingListAllocationFailed {
                    collection: "Journey semantic text",
                })?;
            visible.push(' ');
        }
        for part in normalized_text_parts(line) {
            visible
                .try_reserve(part.len())
                .map_err(|_| Error::DrawingListAllocationFailed {
                    collection: "Journey semantic text",
                })?;
            visible.push_str(part);
        }
    }
    Ok(visible)
}

fn journey_break_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    if bytes.get(index) == Some(&b'/') {
        index += 1;
    }
    if !bytes
        .get(index..index + 2)
        .is_some_and(|name| name.eq_ignore_ascii_case(b"br"))
    {
        return None;
    }
    index += 2;
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    if bytes.get(index) == Some(&b'/') {
        index += 1;
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            index += 1;
        }
    }
    (bytes.get(index) == Some(&b'>')).then_some(index + 1)
}

fn line_path(from: Point, to: Point) -> Vec<PathSegment> {
    vec![PathSegment::MoveTo { to: from }, PathSegment::LineTo { to }]
}

fn smile_path(cx: f64, cy: f64) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo {
            to: Point::new(cx + 7.5, cy),
        },
        PathSegment::ArcTo {
            radius_x: 7.5,
            radius_y: 7.5,
            x_axis_rotation_degrees: 0.0,
            large_arc: true,
            sweep_clockwise: true,
            to: Point::new(cx - 7.5, cy),
        },
        PathSegment::LineTo {
            to: Point::new(cx - 6.818, cy),
        },
        PathSegment::ArcTo {
            radius_x: 6.818,
            radius_y: 6.818,
            x_axis_rotation_degrees: 0.0,
            large_arc: true,
            sweep_clockwise: false,
            to: Point::new(cx + 6.818, cy),
        },
        PathSegment::Close,
    ]
}

fn sad_path(cx: f64, cy: f64) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo {
            to: Point::new(cx - 7.5, cy),
        },
        PathSegment::ArcTo {
            radius_x: 7.5,
            radius_y: 7.5,
            x_axis_rotation_degrees: 0.0,
            large_arc: true,
            sweep_clockwise: true,
            to: Point::new(cx + 7.5, cy),
        },
        PathSegment::LineTo {
            to: Point::new(cx + 6.818, cy),
        },
        PathSegment::ArcTo {
            radius_x: 6.818,
            radius_y: 6.818,
            x_axis_rotation_degrees: 0.0,
            large_arc: true,
            sweep_clockwise: false,
            to: Point::new(cx - 6.818, cy),
        },
        PathSegment::Close,
    ]
}

fn arrowhead_path(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    stroke_width: f64,
) -> Result<Vec<PathSegment>> {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = dx.hypot(dy);
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(invalid("Journey activity line has no arrowhead tangent"));
    }
    let ux = dx / length;
    let uy = dy / length;
    let nx = -uy;
    let ny = ux;
    let scale = stroke_width.max(0.1);
    let base_x = x2 - ux * 6.0 * scale;
    let base_y = y2 - uy * 6.0 * scale;
    let half_width = 2.0 * scale;
    Ok(polygon_path(&[
        Point::new(x2, y2),
        Point::new(base_x + nx * half_width, base_y + ny * half_width),
        Point::new(base_x - nx * half_width, base_y - ny * half_width),
    ]))
}

fn validate_layout(layout: &JourneyDiagramLayout) -> Result<()> {
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| invalid("Journey layout did not provide root bounds"))?;
    validate_bounds(bounds)?;
    if [
        layout.left_margin,
        layout.max_actor_label_width,
        layout.width,
        layout.height,
        layout.svg_height,
        layout.title_x,
        layout.title_y,
    ]
    .iter()
    .any(|value| !value.is_finite())
        || layout.width <= 0.0
        || layout.height <= 0.0
        || layout.svg_height <= 0.0
    {
        return Err(invalid("Journey root metrics are invalid"));
    }
    // Variable-sized collections are validated as they are emitted under operation checkpoints.
    Ok(())
}

fn validate_actor_legend(actor: &JourneyActorLegendItemLayout) -> Result<()> {
    if actor.pos < 0
        || [actor.circle_cx, actor.circle_cy, actor.circle_r]
            .iter()
            .any(|value| !value.is_finite())
        || actor.circle_r <= 0.0
    {
        return Err(invalid("Journey actor legend geometry is invalid"));
    }
    Ok(())
}

fn validate_section(section: &JourneySectionLayout) -> Result<()> {
    if [
        section.x,
        section.y,
        section.width,
        section.height,
        section.num as f64,
        section.task_count as f64,
    ]
    .iter()
    .any(|value| !value.is_finite())
        || section.width <= 0.0
        || section.height <= 0.0
        || section.num < 0
        || section.task_count < 0
    {
        return Err(invalid("Journey section geometry is invalid"));
    }
    Ok(())
}

fn validate_task(task: &JourneyTaskLayout) -> Result<()> {
    if [
        task.x,
        task.y,
        task.width,
        task.height,
        task.num as f64,
        task.line_x1,
        task.line_y1,
        task.line_x2,
        task.line_y2,
        task.face_cx,
    ]
    .iter()
    .any(|value| !value.is_finite())
        || task.width <= 0.0
        || task.height <= 0.0
        || task.num < 0
    {
        return Err(invalid("Journey task geometry is invalid"));
    }
    let face_y = task
        .face_cy
        .ok_or_else(|| unavailable("Journey score produced non-finite face geometry"))?;
    if !face_y.is_finite() {
        return Err(invalid("Journey task face geometry is invalid"));
    }
    Ok(())
}

fn validate_task_actor(actor: &JourneyTaskActorCircleLayout) -> Result<()> {
    if actor.pos < 0
        || [actor.cx, actor.cy, actor.r]
            .iter()
            .any(|value| !value.is_finite())
        || actor.r <= 0.0
    {
        return Err(invalid("Journey task actor geometry is invalid"));
    }
    Ok(())
}

fn validate_line(line: &JourneyLineLayout) -> Result<()> {
    if [line.x1, line.y1, line.x2, line.y2]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(invalid("Journey line geometry is invalid"));
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
        return Err(invalid("Journey bounds are invalid"));
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
        family: "journey".to_string(),
        reason: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_lines_and_parts_preserve_journey_text_rules() {
        let lines = JourneyLines::new("  A\tB <BR />\n C\u{00a0}D </br><br> ")
            .map(|line| normalized_text_parts(line).collect::<String>())
            .collect::<Vec<_>>();
        assert_eq!(lines, ["A B", "C\u{00a0}D", "", ""]);
        assert!(JourneyLines::new("A<br/>B").all(|line| validate_journey_line(line).is_ok()));
        assert!(JourneyLines::new("A<b>B</b>").any(|line| validate_journey_line(line).is_err()));
        assert_eq!(JourneyLines::new("").collect::<Vec<_>>(), [""]);
    }
}
