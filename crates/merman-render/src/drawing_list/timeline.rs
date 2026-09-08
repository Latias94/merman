//! Renderer-neutral Timeline adapter.
//!
//! Timeline layout owns the final node and connector geometry.  The adapter expands Mermaid's
//! family CSS into typed paints, turns marker arrows into ordinary paths, and keeps wrapped label
//! rows as separate host-text runs so a non-SVG renderer never has to interpret the SVG DOM.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, TimelineSvgBody, parse_font_families_for,
};
use crate::config::{config_bool, config_diagram_look, config_font_family_css_raw};
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::flowchart::polygon_path;
use crate::drawing_list::support::{PortableStyleResolver, stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, TimelineDiagramLayout, TimelineLineLayout, TimelineNodeLayout};
use crate::svg::render_theme::{TimelineSectionTheme, TimelineTheme};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineDirection};
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, DrawingCommand, DrawingListPolicy, FillRule, FontDescriptor, FontStyle, Paint,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle,
    TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun,
    TextStyle as DisplayTextStyle, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type TimelinePair = FamilyPair<TimelineDiagramRenderModel, TimelineDiagramLayout>;

const EVENT_BRIGHTNESS: f64 = 1.2;
const TITLE_FONT_SCALE: f64 = 1.9375;

pub(crate) fn build_timeline_document(
    pair: &TimelinePair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: impl Into<super::DocumentBudget>,
    session: &RenderSession,
) -> Result<RenderDocument> {
    TimelineBuilder::new(pair, metadata, policy, limits, session)?.build()
}

#[derive(Debug, Clone)]
struct NodeVisualStyle {
    fill: Color,
    stroke: Option<StrokeStyle>,
    label: Color,
    weight: u16,
    divider: Option<Color>,
}

struct TimelineBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    document: DrawingListBuilder<'a>,
    model: &'a TimelineDiagramRenderModel,
    layout: &'a TimelineDiagramLayout,
    theme: TimelineTheme,
    font: FontDescriptor,
    font_size: f64,
    title_font_size: f64,
    text_obligation: TextObligation,
    text_color: Color,
    connector_color: Color,
    connector_width: f64,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
}

impl<'a> TimelineBuilder<'a> {
    fn new(
        pair: &'a TimelinePair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: impl Into<super::DocumentBudget>,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;

        let theme = crate::svg::render_theme::PresentationTheme::new(config).timeline();
        // svgDraw.js creates a real Redux shadow; styles.js applies the neo gradient to
        // non-neutral nodes. Neither effect may disappear from a successful DrawingList.
        if config_diagram_look(config).is_neo()
            && (theme.is_redux_theme
                || (config.get("theme").and_then(serde_json::Value::as_str) != Some("neutral")
                    && config_bool(config, &["themeVariables", "useGradient"]).unwrap_or(false)))
        {
            return Err(unavailable(
                "Timeline neo gradient or drop-shadow effects are not yet represented by portable commands or a bounded raster subtree",
            ));
        }
        let styles = PortableStyleResolver::new("timeline");
        let text_color = styles.color("textColor", &theme.text_color)?;
        let line_color = styles.color("lineColor", &theme.line_color)?;
        let connector_width = styles.positive_length("strokeWidth", &theme.stroke_width)?;
        let connector_color = if theme.is_redux_theme {
            styles.color("nodeBorder", &theme.node_border)?
        } else {
            theme
                .sections
                .last()
                // Equal-specificity .lineWrapper line rules use the final section label color.
                .map(|section| styles.color("cScaleLabel", &section.c_scale_label))
                .transpose()?
                .unwrap_or(line_color)
        };
        let font_css = config_font_family_css_raw(config);
        let font = FontDescriptor {
            families: parse_font_families_for(font_css, RenderFamilyKind::Timeline)?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let font_size = theme.font_size_px.max(1.0);

        let mut document = DrawingListBuilder::new(policy, limits, session);
        document.push_control(DrawingCommand::Save)?;
        document.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "timeline.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            document,
            model,
            layout,
            theme,
            font,
            font_size,
            title_font_size: font_size * TITLE_FONT_SCALE,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            text_color,
            connector_color,
            connector_width,
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let layout = self.layout;
        let title = layout.title.as_deref();
        self.document.push_semantic(SemanticAnnotation {
            id: "timeline.document".to_string(),
            role: SemanticRole::Document,
            title: self.model.acc_title.clone(),
            description: self.model.acc_descr.clone(),
            link: None,
        })?;

        if self.layout.direction == TimelineDirection::TopDown {
            self.emit_activity_line(true)?;
        }

        let mut task_index = 0usize;
        for (section_index, section) in self.layout.sections.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let semantic_id = format!("timeline.section.{section_index}");
            self.semantic_classes.insert(
                semantic_id.clone(),
                format!("timeline-node {}", section.node.section_class),
            );
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.emit_node_visual(&format!("{semantic_id}.node"), &section.node, false)?;
            for task in &section.tasks {
                self.emit_task(task_index, task)?;
                task_index += 1;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: non_empty(&section.node.label),
                description: Some("Timeline section".to_string()),
                link: None,
            })?;
        }

        for task in &self.layout.orphan_tasks {
            self.emit_task(task_index, task)?;
            task_index += 1;
        }

        if let Some(title) = title.filter(|title| !title.trim().is_empty()) {
            self.emit_title(title)?;
        }
        if self.layout.direction == TimelineDirection::LeftToRight {
            self.emit_activity_line(true)?;
        }

        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_control(DrawingCommand::Restore)?;

        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Timeline layout did not provide root bounds"))?;
        let document = self.document.finish(
            Viewport::new(Rect::new(
                bounds.min_x,
                bounds.min_y,
                bounds.max_x - bounds.min_x,
                bounds.max_y - bounds.min_y,
            )),
            BTreeMap::from([(
                "x-merman-timeline".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "direction": self.layout.direction,
                    "text_mode": "plain_host_text",
                    "use_max_width": self.layout.use_max_width,
                    "theme": if self.theme.is_redux_theme { "redux" } else { "classic" },
                }),
            )]),
        )?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Timeline,
                body: SvgStructureBody::Timeline(TimelineSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                }),
            },
        })
    }

    fn emit_task(&mut self, index: usize, task: &crate::model::TimelineTaskLayout) -> Result<()> {
        let semantic_id = format!("timeline.task.{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "taskWrapper".to_string());
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        self.emit_node_visual(&format!("{semantic_id}.node"), &task.node, false)?;

        match self.layout.direction {
            TimelineDirection::LeftToRight => {
                for (line_index, connector) in task.connectors.iter().enumerate() {
                    self.emit_connector(
                        &format!("{semantic_id}.connector.{line_index}"),
                        connector,
                    )?;
                }
                for (event_index, event) in task.events.iter().enumerate() {
                    self.emit_event(&semantic_id, event_index, event)?;
                }
            }
            TimelineDirection::TopDown => {
                for (event_index, event) in task.events.iter().enumerate() {
                    self.emit_event(&semantic_id, event_index, event)?;
                    if let Some(connector) = task.connectors.get(event_index) {
                        self.emit_connector(
                            &format!("{semantic_id}.connector.{event_index}"),
                            connector,
                        )?;
                    }
                }
                for (line_index, connector) in
                    task.connectors.iter().enumerate().skip(task.events.len())
                {
                    self.emit_connector(
                        &format!("{semantic_id}.connector.{line_index}"),
                        connector,
                    )?;
                }
            }
        }

        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: non_empty(&task.node.label),
            description: Some("Timeline task".to_string()),
            link: None,
        })?;
        Ok(())
    }

    fn emit_event(
        &mut self,
        task_id: &str,
        index: usize,
        event: &TimelineNodeLayout,
    ) -> Result<()> {
        let semantic_id = format!("{task_id}.event.{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "eventWrapper".to_string());
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        self.emit_node_visual(&format!("{semantic_id}.node"), event, true)?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: non_empty(&event.label),
            description: Some("Timeline event".to_string()),
            link: None,
        })?;
        Ok(())
    }

    fn emit_node_visual(
        &mut self,
        prefix: &str,
        node: &TimelineNodeLayout,
        is_event: bool,
    ) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)?;
        validate_node(node)?;
        let style = self.node_style(node, is_event)?;
        self.path_classes.insert(
            format!("{prefix}.background"),
            "node-bkg node-undefined".to_string(),
        );
        self.add_path(
            format!("{prefix}.background"),
            timeline_node_path(
                node.x,
                node.y,
                node.width,
                node.height,
                self.theme.is_redux_theme,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(style.fill)),
                stroke: style.stroke,
            },
        )?;

        if let Some(divider) = style.divider {
            self.path_classes.insert(
                format!("{prefix}.divider"),
                format!(
                    "node-line-{}",
                    node.section_class.trim_start_matches("section-")
                ),
            );
            self.add_path(
                format!("{prefix}.divider"),
                line_path(
                    Point::new(node.x, node.y + node.height),
                    Point::new(node.x + node.width, node.y + node.height),
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(divider, 3.0)),
                },
            )?;
        }

        self.emit_node_text(node, style.label, style.weight)
    }

    fn emit_node_text(
        &mut self,
        node: &TimelineNodeLayout,
        color: Color,
        weight: u16,
    ) -> Result<()> {
        let lines = if node.label_lines.is_empty() {
            std::slice::from_ref(&node.label)
        } else {
            node.label_lines.as_slice()
        };
        let ty = if self.theme.is_redux_theme {
            if node.kind == "event" {
                node.padding / 2.0 + 3.0
            } else {
                node.padding
            }
        } else {
            node.padding / 2.0
        };
        let x = node.x + node.width / 2.0;
        let session = self.session;
        let font_family = self.theme.font_family.clone();
        let font = self.font.clone();
        let obligation = self.text_obligation.clone();
        for (line_index, line) in lines.iter().enumerate() {
            session.checkpoint(OperationPhase::Emit)?;
            if line.trim().is_empty() {
                continue;
            }
            let y = node.y + ty + self.font_size + line_index as f64 * self.font_size * 1.1;
            let font_size = self.font_size;
            self.document.draw_host_text_parts(&[line], |text| {
                let measurement_style = MeasurementTextStyle {
                    font_family: Some(font_family.clone()),
                    font_size,
                    font_weight: Some(weight.to_string()),
                    font_style: None,
                };
                let measurer = session
                    .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
                let width =
                    measurer.measure_svg_tspan_text_bbox_width_px(&text, &measurement_style);
                let height =
                    measurer.measure_svg_tspan_text_bbox_height_px(&text, &measurement_style);
                session.checkpoint(OperationPhase::Emit)?;
                if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
                    return Err(invalid("Timeline text measurement returned invalid bounds"));
                }
                let width = width.max(1.0);
                let height = height.max(1.0);
                Ok(TextRun {
                    text,
                    origin: Point::new(x, y),
                    bounds: Rect::new(x - width / 2.0, y - height / 2.0, width, height),
                    style: DisplayTextStyle {
                        font: FontDescriptor {
                            weight,
                            ..font.clone()
                        },
                        font_size,
                        letter_spacing: 0.0,
                        line_height: font_size * 1.1,
                        fill: Paint::solid(color),
                        stroke: None,
                        paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                    },
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                    direction: TextDirection::Auto,
                    language: None,
                    obligation: obligation.clone(),
                })
            })?;
        }
        Ok(())
    }

    fn emit_connector(&mut self, prefix: &str, line: &TimelineLineLayout) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)?;
        validate_line(line)?;
        let width = if self.theme.is_redux_theme {
            self.connector_width
        } else {
            2.0
        };
        let mut line_style = stroke(self.connector_color, width);
        line_style.dash_array = vec![5.0, 5.0];
        self.path_classes
            .insert(format!("{prefix}.line"), "line".to_string());
        self.add_path(
            format!("{prefix}.line"),
            line_path(Point::new(line.x1, line.y1), Point::new(line.x2, line.y2)),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(line_style),
            },
        )?;
        self.emit_arrowhead(&format!("{prefix}.arrowhead"), line, width)?;
        self.document.push_semantic(SemanticAnnotation {
            id: prefix.to_string(),
            role: SemanticRole::Edge,
            title: None,
            description: Some("Timeline task-event connector".to_string()),
            link: None,
        })?;
        Ok(())
    }

    fn emit_activity_line(&mut self, with_arrow: bool) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)?;
        let line = &self.layout.activity_line;
        validate_line(line)?;
        let width = if self.theme.is_redux_theme {
            self.connector_width
        } else {
            4.0
        };
        let mut line_style = stroke(self.connector_color, width);
        line_style.dash_array.clear();
        let semantic_id = "timeline.activity";
        self.semantic_classes
            .insert(semantic_id.to_string(), "lineWrapper".to_string());
        self.path_classes
            .insert("timeline.activity.line".to_string(), "line".to_string());
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.to_string(),
            })?;
        self.add_path(
            "timeline.activity.line",
            line_path(Point::new(line.x1, line.y1), Point::new(line.x2, line.y2)),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(line_style),
            },
        )?;
        if with_arrow {
            self.emit_arrowhead("timeline.activity.arrowhead", line, width)?;
        }
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Edge,
            title: None,
            description: Some("Timeline activity axis".to_string()),
            link: None,
        })?;
        Ok(())
    }

    fn emit_arrowhead(
        &mut self,
        prefix: &str,
        line: &TimelineLineLayout,
        stroke_width: f64,
    ) -> Result<()> {
        self.path_classes
            .insert(prefix.to_string(), "arrowhead".to_string());
        self.add_path(
            prefix,
            arrowhead_path(line.x1, line.y1, line.x2, line.y2, stroke_width)?,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(0, 0, 0, 255))),
                stroke: None,
            },
        )
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        let style = MeasurementTextStyle {
            font_family: Some(self.theme.font_family.clone()),
            font_size: self.title_font_size,
            font_weight: Some("700".to_string()),
            font_style: None,
        };
        let semantic_id = "timeline.title";
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.to_string(),
            })?;
        let session = self.session;
        let font = self.font.clone();
        let obligation = self.text_obligation.clone();
        let title_x = self.layout.title_x;
        let title_y = self.layout.title_y;
        let title_font_size = self.title_font_size;
        let text_color = self.text_color;
        self.document.draw_host_text_parts(&[title], |text| {
            let measurer = session
                .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
            let width = measurer.measure_svg_raw_text_bbox_width_px(&text, &style);
            let height = measurer.measure_svg_raw_text_bbox_height_px(&text, &style);
            session.checkpoint(OperationPhase::Emit)?;
            if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
                return Err(invalid("Timeline text measurement returned invalid bounds"));
            }
            let width = width.max(1.0);
            let height = height.max(1.0);
            Ok(TextRun {
                text,
                origin: Point::new(title_x, title_y),
                bounds: Rect::new(title_x, title_y - height, width, height),
                style: DisplayTextStyle {
                    font: FontDescriptor {
                        weight: 700,
                        ..font
                    },
                    font_size: title_font_size,
                    letter_spacing: 0.0,
                    line_height: title_font_size,
                    fill: Paint::solid(text_color),
                    stroke: None,
                    paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                },
                anchor: TextAnchor::Start,
                baseline: TextBaseline::Alphabetic,
                direction: TextDirection::Auto,
                language: None,
                obligation,
            })
        })?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Label,
            title: Some(title.to_string()),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn node_style(&self, node: &TimelineNodeLayout, is_event: bool) -> Result<NodeVisualStyle> {
        let is_root = node.section_class == "section-root";
        let (fill_token, label_token, stroke_style, divider_token, weight) = if is_root {
            (
                self.theme.root_fill.clone(),
                self.theme.root_label.clone(),
                None,
                None,
                400,
            )
        } else if self.theme.is_redux_theme {
            let border_color = self
                .theme
                .border_colors
                .get(self.section_slot(node)?)
                .cloned()
                .unwrap_or_else(|| self.theme.node_border.clone());
            let redux_fill = if self.theme.is_color_theme && !self.theme.is_dark_theme {
                border_color.clone()
            } else {
                self.theme.main_bkg.clone()
            };
            let redux_stroke = if self.theme.is_color_theme {
                border_color
            } else {
                self.theme.node_border.clone()
            };
            (
                redux_fill,
                self.theme.node_border.clone(),
                Some(stroke(
                    PortableStyleResolver::new("timeline").color("nodeBorder", &redux_stroke)?,
                    PortableStyleResolver::new("timeline")
                        .positive_length("strokeWidth", &self.theme.stroke_width)?,
                )),
                None,
                parse_font_weight(&self.theme.font_weight),
            )
        } else {
            let section = self.section_theme(node)?;
            (
                section.c_scale.clone(),
                section.c_scale_label.clone(),
                None,
                Some(section.c_scale_inv.clone()),
                400,
            )
        };

        let styles = PortableStyleResolver::new("timeline");
        let mut fill = styles.color("node.fill", &fill_token)?;
        let mut label = styles.color("node.label", &label_token)?;
        let divider = divider_token
            .map(|color| styles.color("node.divider", &color))
            .transpose()?;
        if is_event {
            fill = brighten(fill);
            label = brighten(label);
        }
        let stroke = stroke_style.map(|mut style| {
            if is_event && let Paint::Solid { color } = style.paint {
                style.paint = Paint::solid(brighten(color));
            }
            style
        });
        Ok(NodeVisualStyle {
            fill,
            stroke,
            label,
            weight,
            divider: divider.map(|color| if is_event { brighten(color) } else { color }),
        })
    }

    fn section_slot(&self, node: &TimelineNodeLayout) -> Result<usize> {
        let suffix = node
            .section_class
            .strip_prefix("section-")
            .ok_or_else(|| invalid("Timeline node has an invalid section class"))?;
        let section = suffix
            .parse::<i64>()
            .map_err(|_| invalid("Timeline node has an invalid section class"))?;
        let slot = section
            .checked_add(1)
            .ok_or_else(|| invalid("Timeline section class overflowed"))?;
        if slot < 0 {
            return Err(invalid(
                "Timeline section class is below the supported range",
            ));
        }
        Ok(slot as usize)
    }

    fn section_theme(&self, node: &TimelineNodeLayout) -> Result<&TimelineSectionTheme> {
        let slot = self.section_slot(node)?;
        self.theme
            .sections
            .get(slot)
            .or_else(|| self.theme.sections.last())
            .ok_or_else(|| unavailable("Timeline theme did not resolve any section colors"))
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("Timeline path has no geometry"));
        }
        let id = ResourceId::new(id.into());
        self.document.draw_path(id, segments, style)
    }
}

fn timeline_node_path(x: f64, y: f64, width: f64, height: f64, redux: bool) -> Vec<PathSegment> {
    let width = width.max(10.0);
    let height = height.max(5.0);
    if redux {
        vec![
            PathSegment::MoveTo {
                to: Point::new(x, y + height - 5.0),
            },
            PathSegment::LineTo {
                to: Point::new(x, y),
            },
            PathSegment::LineTo {
                to: Point::new(x + width, y),
            },
            PathSegment::LineTo {
                to: Point::new(x + width, y + height),
            },
            PathSegment::LineTo {
                to: Point::new(x, y + height),
            },
            PathSegment::Close,
        ]
    } else {
        vec![
            PathSegment::MoveTo {
                to: Point::new(x, y + height - 5.0),
            },
            PathSegment::LineTo {
                to: Point::new(x, y + 5.0),
            },
            PathSegment::QuadTo {
                control: Point::new(x, y),
                to: Point::new(x + 5.0, y),
            },
            PathSegment::LineTo {
                to: Point::new(x + width - 5.0, y),
            },
            PathSegment::QuadTo {
                control: Point::new(x + width, y),
                to: Point::new(x + width, y + 5.0),
            },
            PathSegment::LineTo {
                to: Point::new(x + width, y + height),
            },
            PathSegment::LineTo {
                to: Point::new(x, y + height),
            },
            PathSegment::Close,
        ]
    }
}

fn line_path(from: Point, to: Point) -> Vec<PathSegment> {
    vec![PathSegment::MoveTo { to: from }, PathSegment::LineTo { to }]
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
        return Err(invalid("Timeline line has no arrowhead tangent"));
    }
    if !stroke_width.is_finite() || stroke_width <= 0.0 {
        return Err(invalid(
            "Timeline arrowhead stroke width must be positive and finite",
        ));
    }
    let ux = dx / length;
    let uy = dy / length;
    let nx = -uy;
    let ny = ux;
    let scale = stroke_width;
    let base_x = x2 - ux * 5.0 * scale;
    let base_y = y2 - uy * 5.0 * scale;
    let half_width = 2.0 * scale;
    Ok(polygon_path(&[
        Point::new(x2 + ux * scale, y2 + uy * scale),
        Point::new(base_x + nx * half_width, base_y + ny * half_width),
        Point::new(base_x - nx * half_width, base_y - ny * half_width),
    ]))
}

fn brighten(color: Color) -> Color {
    let scale = |channel: u8| ((channel as f64 * EVENT_BRIGHTNESS).round()).clamp(0.0, 255.0) as u8;
    Color::rgba(
        scale(color.red),
        scale(color.green),
        scale(color.blue),
        color.alpha,
    )
}

fn parse_font_weight(value: &str) -> u16 {
    match value.trim().to_ascii_lowercase().as_str() {
        "bold" => 700,
        "normal" => 400,
        raw => raw
            .parse::<u16>()
            .ok()
            .map(|value| value.clamp(100, 900))
            .unwrap_or(400),
    }
}

fn non_empty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

fn validate_layout(layout: &TimelineDiagramLayout) -> Result<()> {
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| invalid("Timeline layout did not provide root bounds"))?;
    validate_bounds(bounds)?;
    if [
        layout.left_margin,
        layout.base_x,
        layout.base_y,
        layout.pre_title_box_width,
        layout.title_x,
        layout.title_y,
    ]
    .iter()
    .any(|value| !value.is_finite())
        || layout.left_margin < 0.0
        || layout.pre_title_box_width < 0.0
    {
        return Err(invalid("Timeline root metrics are invalid"));
    }
    Ok(())
}

fn validate_node(node: &TimelineNodeLayout) -> Result<()> {
    if [
        node.x,
        node.y,
        node.width,
        node.height,
        node.content_width,
        node.padding,
    ]
    .iter()
    .any(|value| !value.is_finite())
        || node.width <= 0.0
        || node.height <= 0.0
        || node.content_width <= 0.0
        || node.padding < 0.0
    {
        return Err(invalid("Timeline node geometry is invalid"));
    }
    if node.section_class.trim().is_empty() {
        return Err(invalid("Timeline node section class is empty"));
    }
    Ok(())
}

fn validate_line(line: &TimelineLineLayout) -> Result<()> {
    if [line.x1, line.y1, line.x2, line.y2]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(invalid("Timeline line geometry is invalid"));
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
        return Err(invalid("Timeline bounds are invalid"));
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
        family: "timeline".to_string(),
        reason: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_arrowhead_preserves_mermaid_marker_reference_point() {
        // svgDraw.js: refX=5, refY=2, markerUnits=strokeWidth; local triangle
        // (0,0), (0,4), (6,2). The tip lies one stroke width past the line endpoint.
        assert_eq!(
            arrowhead_path(0.0, 10.0, 20.0, 10.0, 2.0).unwrap(),
            polygon_path(&[
                Point::new(22.0, 10.0),
                Point::new(10.0, 14.0),
                Point::new(10.0, 6.0)
            ])
        );
        assert_eq!(
            arrowhead_path(10.0, 0.0, 10.0, 20.0, 2.0).unwrap(),
            polygon_path(&[
                Point::new(10.0, 22.0),
                Point::new(6.0, 10.0),
                Point::new(14.0, 10.0)
            ])
        );
        assert_eq!(
            arrowhead_path(0.0, 10.0, 20.0, 10.0, 0.05).unwrap()[0],
            PathSegment::MoveTo {
                to: Point::new(20.05, 10.0)
            }
        );
    }
}
