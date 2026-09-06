//! Renderer-neutral Gantt adapter.
//!
//! Gantt layout already owns the final time scale, row placement, task state, and label
//! placement.  This adapter makes the SVG stylesheet's built-in state matrix explicit so a host
//! renderer can draw the same visible chart without evaluating CSS selectors.

use super::{
    GanttSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
};
use crate::config::config_font_family_css;
use crate::drawing_list::flowchart::rounded_rect_path;
use crate::drawing_list::support::{
    PortableStyleResolver, navigation_security, portable_navigation_uri, stroke, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{GanttAxisTickLayout, GanttDiagramLayout, GanttRowLayout};
use crate::svg::render_theme::GanttTheme;
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::diagrams::gantt::{GanttDiagramRenderModel, GanttRenderTask};
use merman_core::svg_security::MermaidNavigationSecurity;
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type GanttPair = FamilyPair<GanttDiagramRenderModel, GanttDiagramLayout>;

pub(crate) fn build_gantt_document(
    pair: &GanttPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    GanttBuilder::new(pair, metadata, policy, session)?.build()
}

struct GanttBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a GanttDiagramRenderModel,
    layout: &'a GanttDiagramLayout,
    theme: GanttTheme,
    font: FontDescriptor,
    text_obligation: TextObligation,
    navigation_security: MermaidNavigationSecurity,
    exclude_fill: Color,
    section_fill: Color,
    section_fill_alt: Color,
    section_fill_alt2: Color,
    title_fill: Color,
    title_text_fill: Color,
    text_fill: Color,
    grid_fill: Color,
    today_fill: Color,
    task_text_dark_fill: Color,
    task_text_clickable_fill: Color,
    task_text_fill: Color,
    task_fill: Color,
    task_border: Color,
    task_text_outside_fill: Color,
    active_task_fill: Color,
    active_task_border: Color,
    done_task_border: Color,
    done_task_fill: Color,
    crit_border: Color,
    crit_fill: Color,
    vert_line: Color,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

#[derive(Clone, Copy)]
struct TextEmitSpec {
    origin: Point,
    font_size: f64,
    weight: u16,
    color: Color,
    anchor: TextAnchor,
    baseline: TextBaseline,
    italic: bool,
}

impl<'a> GanttBuilder<'a> {
    fn new(
        pair: &'a GanttPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let navigation_security = navigation_security(config);
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;
        validate_task_classes(model)?;

        let presentation = crate::theme::PresentationTheme::new(config);
        let theme = presentation.gantt();
        let styles = PortableStyleResolver::new("gantt");
        let color = |property: &str, value: &str| styles.color(property, value);
        let font_css = if theme.font_family.trim().is_empty() {
            config_font_family_css(config)
        } else {
            theme.font_family.clone()
        };
        let font = FontDescriptor {
            families: parse_font_families(font_css),
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
            exclude_fill: color("excludeBkgColor", &theme.exclude_bkg_color)?,
            section_fill: color("sectionBkgColor", &theme.section_bkg_color)?,
            section_fill_alt: color("altSectionBkgColor", &theme.alt_section_bkg_color)?,
            section_fill_alt2: color("sectionBkgColor2", &theme.section_bkg_color2)?,
            title_fill: color("titleColor", &theme.title_color)?,
            title_text_fill: color("titleColor", &theme.title_text_color)?,
            text_fill: color("textColor", &theme.text_color)?,
            grid_fill: color("gridColor", &theme.grid_color)?,
            today_fill: color("todayLineColor", &theme.today_line_color)?,
            task_text_dark_fill: color("taskTextDarkColor", &theme.task_text_dark_color)?,
            task_text_clickable_fill: color(
                "taskTextClickableColor",
                &theme.task_text_clickable_color,
            )?,
            task_text_fill: color("taskTextColor", &theme.task_text_color)?,
            task_fill: color("taskBkgColor", &theme.task_bkg_color)?,
            task_border: color("taskBorderColor", &theme.task_border_color)?,
            task_text_outside_fill: color("taskTextOutsideColor", &theme.task_text_outside_color)?,
            active_task_fill: color("activeTaskBkgColor", &theme.active_task_bkg_color)?,
            active_task_border: color("activeTaskBorderColor", &theme.active_task_border_color)?,
            done_task_border: color("doneTaskBorderColor", &theme.done_task_border_color)?,
            done_task_fill: color("doneTaskBkgColor", &theme.done_task_bkg_color)?,
            crit_border: color("critBorderColor", &theme.crit_border_color)?,
            crit_fill: color("critBkgColor", &theme.crit_bkg_color)?,
            vert_line: color("vertLineColor", &theme.vert_line_color)?,
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
            theme,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            navigation_security,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "gantt.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let title = self
            .model
            .title
            .clone()
            .or_else(|| self.metadata.title.clone());
        self.semantics.push(SemanticAnnotation {
            id: "gantt.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_background()?;
        self.emit_excludes()?;
        self.emit_axis(
            &self.layout.bottom_ticks,
            self.layout.height - self.layout.top_padding,
            true,
        )?;
        if self.layout.top_axis {
            self.emit_axis(&self.layout.top_ticks, self.layout.top_padding, false)?;
        }
        self.emit_rows()?;
        self.emit_tasks()?;
        self.emit_section_titles()?;
        self.emit_today_marker()?;
        if let Some(title) = title.as_deref().filter(|title| !title.is_empty()) {
            self.emit_title(title)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let links = self
            .model
            .links
            .iter()
            .map(|(id, link)| (id.clone(), link.clone()))
            .collect::<BTreeMap<_, _>>();
        let click_events = self
            .model
            .click_events
            .iter()
            .map(|(id, event)| (id.clone(), event.clone()))
            .collect::<BTreeMap<_, _>>();

        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(0.0, 0.0, self.layout.width, self.layout.height)),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-gantt".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "text_mode": "plain_host_text",
                    "display_mode": self.layout.display_mode,
                    "date_format": self.layout.date_format,
                    "axis_format": self.layout.axis_format,
                    "today_marker": self.layout.today_marker,
                    "links": links,
                    "click_events": click_events,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Gantt,
                body: SvgStructureBody::Gantt(GanttSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    bar_height: self.layout.bar_height,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                    dom_ids: self.dom_ids,
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        self.add_path(
            "gantt.background",
            rect_path(0.0, 0.0, self.layout.width, self.layout.height, 0.0),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )
    }

    fn emit_excludes(&mut self) -> Result<()> {
        for (index, exclude) in self.layout.excludes.iter().enumerate() {
            self.dom_ids
                .insert(format!("gantt.exclude.{index}"), exclude.id.clone());
            self.path_classes.insert(
                format!("gantt.exclude.{index}"),
                "exclude-range".to_string(),
            );
            self.add_path(
                format!("gantt.exclude.{index}"),
                rect_path(exclude.x, exclude.y, exclude.width, exclude.height, 0.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.exclude_fill)),
                    stroke: None,
                },
            )?;
        }
        Ok(())
    }

    fn emit_axis(&mut self, ticks: &[GanttAxisTickLayout], y: f64, bottom: bool) -> Result<()> {
        let axis_name = if bottom { "bottom" } else { "top" };
        let axis_semantic_id = format!("gantt.axis.{axis_name}");
        self.semantic_classes
            .insert(axis_semantic_id.clone(), "grid".to_string());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: axis_semantic_id.clone(),
        });
        let range =
            (self.layout.width - self.layout.left_padding - self.layout.right_padding).max(1.0);
        let tick_size = if bottom {
            -self.layout.height + self.layout.top_padding + self.layout.grid_line_start_padding
        } else {
            self.layout.height - self.layout.top_padding - self.layout.grid_line_start_padding
        };

        let domain_id = format!("gantt.axis.{axis_name}.domain");
        self.path_classes
            .insert(domain_id.clone(), "domain".to_string());
        self.add_path(
            domain_id,
            vec![
                PathSegment::MoveTo {
                    to: Point::new(self.layout.left_padding + 0.5, y + tick_size),
                },
                PathSegment::LineTo {
                    to: Point::new(self.layout.left_padding + 0.5, y + 0.5),
                },
                PathSegment::LineTo {
                    to: Point::new(self.layout.left_padding + range + 0.5, y + 0.5),
                },
                PathSegment::LineTo {
                    to: Point::new(self.layout.left_padding + range + 0.5, y + tick_size),
                },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.grid_fill, 0.0)),
            },
        )?;

        for (index, tick) in ticks.iter().enumerate() {
            let x = tick.x + 0.5;
            let tick_semantic_id = format!("{axis_semantic_id}.tick.{index}");
            self.semantic_classes
                .insert(tick_semantic_id.clone(), "tick".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: tick_semantic_id.clone(),
            });
            self.commands.push(DrawingCommand::Save);
            self.commands
                .push(DrawingCommand::SetOpacity { opacity: 0.8 });
            let tick_path_id = format!("gantt.axis.{axis_name}.tick.{index}");
            self.add_path(
                tick_path_id,
                vec![
                    PathSegment::MoveTo {
                        to: Point::new(x, y),
                    },
                    PathSegment::LineTo {
                        to: Point::new(x, y + tick_size),
                    },
                ],
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(self.grid_fill, 1.0)),
                },
            )?;
            self.commands.push(DrawingCommand::Restore);

            let text_y = if bottom { y + 13.0 } else { y - 3.0 };
            self.emit_text(
                &format!("{tick_semantic_id}.label"),
                &tick.label,
                TextEmitSpec {
                    origin: Point::new(x, text_y),
                    font_size: 10.0,
                    weight: 400,
                    color: self.text_fill,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Alphabetic,
                    italic: false,
                },
            )?;
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: tick_semantic_id,
                role: SemanticRole::Label,
                title: Some(tick.label.clone()),
                description: None,
                link: None,
            });
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: axis_semantic_id,
            role: SemanticRole::Group,
            title: Some(format!("{} axis", if bottom { "Bottom" } else { "Top" })),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_rows(&mut self) -> Result<()> {
        for (index, row) in self.layout.rows.iter().enumerate() {
            let fill = self.row_fill(row);
            self.path_classes
                .insert(format!("gantt.row.{index}"), row.class.clone());
            self.commands.push(DrawingCommand::Save);
            self.commands
                .push(DrawingCommand::SetOpacity { opacity: 0.2 });
            self.add_path(
                format!("gantt.row.{index}"),
                rect_path(row.x, row.y, row.width, row.height, 0.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: None,
                },
            )?;
            self.commands.push(DrawingCommand::Restore);
        }
        Ok(())
    }

    fn emit_tasks(&mut self) -> Result<()> {
        for (index, task) in self.layout.tasks.iter().enumerate() {
            let source = find_task(self.model, &task.id).ok_or_else(|| {
                invalid(format!(
                    "Gantt layout task `{}` has no semantic source",
                    task.id
                ))
            })?;
            let semantic_id = format!("gantt.task.{index}");
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            let (fill, border, stroke_width) = self.task_style(source);
            let bar_id = format!("{semantic_id}.bar");
            self.dom_ids.insert(bar_id.clone(), task.bar.id.clone());
            self.path_classes
                .insert(bar_id.clone(), task.bar.class.clone());
            let path = if source.milestone {
                transformed_rect_path(
                    task.bar.x,
                    task.bar.y,
                    task.bar.width,
                    task.bar.height,
                    task.bar.rx,
                    task.bar.x + task.bar.width / 2.0,
                    task.bar.y + task.bar.height / 2.0,
                )
            } else {
                rect_path(
                    task.bar.x,
                    task.bar.y,
                    task.bar.width,
                    task.bar.height,
                    task.bar.rx,
                )
            };
            self.add_path(
                bar_id,
                path,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: Some(stroke(border, stroke_width)),
                },
            )?;

            let label_color = self.task_text_style(source, &task.label.class);
            let label_size = if source.vert {
                15.0
            } else {
                task.label.font_size
            };
            let anchor = label_anchor(&task.label.class);
            let clickable = source.classes.iter().any(|class| class == "clickable");
            self.text_classes.insert(
                semantic_id.clone(),
                task_label_classes(task, source, self.layout),
            );
            self.dom_ids
                .insert(semantic_id.clone(), task.label.id.clone());
            self.emit_text(
                &format!("{semantic_id}.label"),
                &task.label.text,
                TextEmitSpec {
                    origin: Point::new(task.label.x, task.label.y),
                    font_size: label_size,
                    weight: if clickable { 700 } else { 400 },
                    color: label_color,
                    anchor,
                    baseline: TextBaseline::Alphabetic,
                    italic: source.milestone,
                },
            )?;
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(source.task.clone()),
                description: (!source.section.is_empty())
                    .then(|| format!("{} section", source.section)),
                link: portable_navigation_uri(
                    self.model.links.get(&source.id).map(String::as_str),
                    self.navigation_security,
                ),
            });
        }
        Ok(())
    }

    fn emit_section_titles(&mut self) -> Result<()> {
        for (index, section) in self.layout.section_titles.iter().enumerate() {
            let semantic_id = format!("gantt.section.{index}");
            self.text_classes
                .insert(semantic_id.clone(), section.class.clone());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            for (line_index, line) in section.lines.iter().enumerate() {
                let y = section.y
                    + section.dy_em * self.layout.section_font_size
                    + line_index as f64 * self.layout.section_font_size;
                self.emit_text(
                    &format!("{semantic_id}.line.{line_index}"),
                    line,
                    TextEmitSpec {
                        origin: Point::new(section.x, y),
                        font_size: self.layout.section_font_size,
                        weight: 400,
                        color: self.title_fill,
                        anchor: TextAnchor::Start,
                        baseline: TextBaseline::Middle,
                        italic: false,
                    },
                )?;
            }
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Label,
                title: Some(section.section.clone()),
                description: None,
                link: None,
            });
        }
        Ok(())
    }

    fn emit_today_marker(&mut self) -> Result<()> {
        let marker = self.layout.today_marker.trim();
        if marker.eq_ignore_ascii_case("off") || self.layout.tasks.is_empty() {
            return Ok(());
        }
        let (color, width, opacity) = self.resolve_today_marker(marker)?;
        let min_ms = self
            .layout
            .tasks
            .iter()
            .map(|task| task.start_ms)
            .min()
            .ok_or_else(|| invalid("Gantt today marker has no minimum task time"))?;
        let max_ms = self
            .layout
            .tasks
            .iter()
            .map(|task| task.end_ms)
            .max()
            .ok_or_else(|| invalid("Gantt today marker has no maximum task time"))?;
        let range =
            (self.layout.width - self.layout.left_padding - self.layout.right_padding).max(1.0);
        let x = scale_time(self.session.unix_millis(), min_ms, max_ms, range)
            + self.layout.left_padding;
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "gantt.today".to_string(),
        });
        self.semantic_classes
            .insert("gantt.today".to_string(), "today".to_string());
        self.path_classes
            .insert("gantt.today.line".to_string(), "today".to_string());
        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::SetOpacity { opacity });
        self.add_path(
            "gantt.today.line",
            vec![
                PathSegment::MoveTo {
                    to: Point::new(x, self.layout.title_top_margin),
                },
                PathSegment::LineTo {
                    to: Point::new(x, self.layout.height - self.layout.title_top_margin),
                },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(color, width)),
            },
        )?;
        self.commands.push(DrawingCommand::Restore);
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: "gantt.today".to_string(),
            role: SemanticRole::Label,
            title: Some("Today".to_string()),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        self.text_classes
            .insert("gantt.title".to_string(), "titleText".to_string());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "gantt.title".to_string(),
        });
        self.emit_text(
            "gantt.title",
            title,
            TextEmitSpec {
                origin: Point::new(self.layout.title_x, self.layout.title_y),
                font_size: 18.0,
                weight: 400,
                color: self.title_text_fill,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Alphabetic,
                italic: false,
            },
        )?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: "gantt.title".to_string(),
            role: SemanticRole::Label,
            title: Some(title.to_string()),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_text(&mut self, semantic_id: &str, text: &str, spec: TextEmitSpec) -> Result<()> {
        let TextEmitSpec {
            origin,
            font_size,
            weight,
            color,
            anchor,
            baseline,
            italic,
        } = spec;
        if text.is_empty() {
            return Ok(());
        }
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.theme.font_family.clone()),
            font_size,
            font_weight: Some(weight.to_string()),
            font_style: italic.then(|| "italic".to_string()),
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer
            .measure_svg_raw_text_bbox_width_px(text, &measurement_style)
            .max(1.0);
        let height = measurer
            .measure_svg_simple_text_bbox_height_px(text, &measurement_style)
            .max(1.0);
        let bounds = Rect::new(
            match anchor {
                TextAnchor::Start => origin.x,
                TextAnchor::Middle => origin.x - width / 2.0,
                TextAnchor::End => origin.x - width,
            },
            origin.y - height / 2.0,
            width,
            height,
        );
        self.commands.push(DrawingCommand::DrawText {
            run: TextRun {
                text: text.to_string(),
                origin,
                bounds,
                style: TextStyle {
                    font: FontDescriptor {
                        weight,
                        style: if italic {
                            FontStyle::Italic
                        } else {
                            FontStyle::Normal
                        },
                        ..self.font.clone()
                    },
                    font_size,
                    letter_spacing: 0.0,
                    line_height: font_size,
                    fill: Paint::solid(color),
                },
                anchor,
                baseline,
                direction: TextDirection::Auto,
                language: None,
                obligation: self.text_obligation.clone(),
            },
        });
        let _ = semantic_id;
        Ok(())
    }

    fn row_fill(&self, row: &GanttRowLayout) -> Color {
        match class_suffix(&row.class) {
            Some(0) => self.section_fill,
            Some(2) => self.section_fill_alt2,
            Some(1 | 3) => self.section_fill_alt,
            _ => self.text_fill,
        }
    }

    fn task_style(&self, task: &GanttRenderTask) -> (Color, Color, f64) {
        let (fill, border) = if task.active && task.crit {
            (self.active_task_fill, self.crit_border)
        } else if task.active {
            (self.active_task_fill, self.active_task_border)
        } else if task.done && task.crit {
            (self.done_task_fill, self.crit_border)
        } else if task.done {
            (self.done_task_fill, self.done_task_border)
        } else if task.crit {
            (self.crit_fill, self.crit_border)
        } else {
            (self.task_fill, self.task_border)
        };
        let border = if task.vert { self.vert_line } else { border };
        (fill, border, 2.0)
    }

    fn task_text_style(&self, task: &GanttRenderTask, class: &str) -> Color {
        if task.vert {
            return self.vert_line;
        }
        let outside =
            class.contains("taskTextOutsideLeft") || class.contains("taskTextOutsideRight");
        if task.done {
            return if outside {
                self.task_text_outside_fill
            } else {
                self.task_text_dark_fill
            };
        }
        if task.active {
            return self.task_text_dark_fill;
        }
        if task.classes.iter().any(|value| value == "clickable") {
            return self.task_text_clickable_fill;
        }
        if outside {
            self.task_text_outside_fill
        } else {
            self.task_text_fill
        }
    }

    fn resolve_today_marker(&self, marker: &str) -> Result<(Color, f64, f64)> {
        let mut color = self.today_fill;
        let mut width = 2.0;
        let mut opacity = 1.0;
        let normalized = marker
            .replace(",opacity:", ";opacity:")
            .replace(",stroke:", ";stroke:")
            .replace(",stroke-width:", ";stroke-width:");
        for declaration in normalized.split(';') {
            let declaration = declaration.trim();
            if declaration.is_empty() {
                continue;
            }
            let Some((property, value)) = declaration.split_once(':') else {
                return Err(unavailable(format!(
                    "Gantt todayMarker declaration `{declaration}` is not portable"
                )));
            };
            match property.trim().to_ascii_lowercase().as_str() {
                "stroke" => {
                    color =
                        PortableStyleResolver::new("gantt").color("todayMarker.stroke", value)?
                }
                "stroke-width" => {
                    width = PortableStyleResolver::new("gantt")
                        .positive_length("todayMarker.stroke-width", value)?;
                }
                "opacity" => {
                    opacity = PortableStyleResolver::new("gantt")
                        .opacity("todayMarker.opacity", value)?;
                }
                "fill" if value.trim().eq_ignore_ascii_case("none") => {}
                _ => {
                    return Err(unavailable(format!(
                        "Gantt todayMarker property `{}` is not portable",
                        property.trim()
                    )));
                }
            }
        }
        Ok((color, width, opacity))
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("Gantt path has no geometry"));
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
}

fn validate_task_classes(model: &GanttDiagramRenderModel) -> Result<()> {
    for task in &model.tasks {
        for class in &task.classes {
            if class != "clickable" {
                return Err(unavailable(format!(
                    "Gantt task class `{class}` requires an unresolved SVG cascade"
                )));
            }
        }
    }
    Ok(())
}

fn validate_layout(layout: &GanttDiagramLayout) -> Result<()> {
    let finite = |value: f64| value.is_finite();
    if [
        layout.width,
        layout.height,
        layout.left_padding,
        layout.right_padding,
        layout.top_padding,
        layout.grid_line_start_padding,
        layout.bar_height,
        layout.bar_gap,
        layout.title_top_margin,
        layout.font_size,
        layout.section_font_size,
        layout.title_x,
        layout.title_y,
    ]
    .iter()
    .any(|value| !finite(*value))
        || layout.width <= 0.0
        || layout.height <= 0.0
    {
        return Err(invalid("Gantt layout dimensions are invalid"));
    }
    for row in &layout.rows {
        validate_rect(row.x, row.y, row.width, row.height, "row")?;
    }
    for exclude in &layout.excludes {
        validate_rect(
            exclude.x,
            exclude.y,
            exclude.width,
            exclude.height,
            "exclude",
        )?;
    }
    for task in &layout.tasks {
        validate_rect(
            task.bar.x,
            task.bar.y,
            task.bar.width,
            task.bar.height,
            "task bar",
        )?;
        if [
            task.start_ms as f64,
            task.end_ms as f64,
            task.label.font_size,
            task.label.width,
            task.label.x,
            task.label.y,
        ]
        .iter()
        .any(|value| !finite(*value))
        {
            return Err(invalid("Gantt task geometry is invalid"));
        }
    }
    for title in &layout.section_titles {
        if [title.x, title.y, title.dy_em]
            .iter()
            .any(|value| !finite(*value))
        {
            return Err(invalid("Gantt section title geometry is invalid"));
        }
    }
    for tick in layout.bottom_ticks.iter().chain(layout.top_ticks.iter()) {
        if !tick.x.is_finite() {
            return Err(invalid("Gantt axis tick geometry is invalid"));
        }
    }
    Ok(())
}

fn validate_rect(x: f64, y: f64, width: f64, height: f64, kind: &str) -> Result<()> {
    if [x, y, width, height].iter().any(|value| !value.is_finite()) || width < 0.0 || height < 0.0 {
        return Err(invalid(format!("Gantt {kind} geometry is invalid")));
    }
    Ok(())
}

fn find_task<'a>(model: &'a GanttDiagramRenderModel, id: &str) -> Option<&'a GanttRenderTask> {
    model.tasks.iter().find(|task| task.id == id)
}

fn task_label_classes(
    layout_task: &crate::model::GanttTaskLayout,
    source: &GanttRenderTask,
    layout: &GanttDiagramLayout,
) -> String {
    let section = crate::gantt::gantt_section_class_suffix(
        &source.task_type,
        &layout.categories,
        layout.number_section_styles,
    );
    let mut state_classes = String::new();
    let mut push_class = |class: String| {
        if !state_classes.is_empty() {
            state_classes.push(' ');
        }
        state_classes.push_str(&class);
    };
    if source.active {
        if source.crit {
            push_class(format!("activeCritText{section}"));
        } else {
            push_class(format!("activeText{section}"));
        }
    }
    if source.done {
        if source.crit {
            push_class(format!("doneCritText{section}"));
        } else {
            push_class(format!("doneText{section}"));
        }
    } else if source.crit {
        push_class(format!("critText{section}"));
    }
    if source.milestone {
        push_class("milestoneText".to_string());
    }
    if source.vert {
        push_class("vertText".to_string());
    }
    gantt_insert_before_width(&layout_task.label.class, state_classes.as_str())
}

fn gantt_insert_before_width(base: &str, insert: &str) -> String {
    let insert = insert.trim();
    if insert.is_empty() {
        return base.to_string();
    }
    let mut parts: Vec<&str> = base.split_whitespace().collect();
    let insert_parts: Vec<&str> = insert.split_whitespace().collect();
    if let Some(index) = parts.iter().position(|part| part.starts_with("width-")) {
        for (offset, part) in insert_parts.iter().enumerate() {
            parts.insert(index + offset, part);
        }
    } else {
        parts.extend(insert_parts);
    }
    parts.join(" ")
}

fn class_suffix(class: &str) -> Option<usize> {
    class
        .split_whitespace()
        .find_map(|value| value.strip_prefix("section"))
        .and_then(|value| value.parse::<usize>().ok())
}

fn label_anchor(class: &str) -> TextAnchor {
    if class.contains("taskTextOutsideLeft") {
        TextAnchor::End
    } else if class.contains("taskTextOutsideRight") {
        TextAnchor::Start
    } else {
        TextAnchor::Middle
    }
}

fn rect_path(x: f64, y: f64, width: f64, height: f64, radius: f64) -> Vec<PathSegment> {
    rounded_rect_path(x + width / 2.0, y + height / 2.0, width, height, radius)
}

fn transformed_rect_path(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    origin_x: f64,
    origin_y: f64,
) -> Vec<PathSegment> {
    let scale = 0.8;
    let angle = std::f64::consts::FRAC_PI_4;
    let cos = angle.cos() * scale;
    let sin = angle.sin() * scale;
    rect_path(x, y, width, height, radius)
        .into_iter()
        .map(|segment| transform_segment(segment, origin_x, origin_y, cos, sin))
        .collect()
}

fn transform_segment(
    segment: PathSegment,
    origin_x: f64,
    origin_y: f64,
    cos: f64,
    sin: f64,
) -> PathSegment {
    let transform = |point: Point| {
        let dx = point.x - origin_x;
        let dy = point.y - origin_y;
        Point::new(
            origin_x + cos * dx - sin * dy,
            origin_y + sin * dx + cos * dy,
        )
    };
    match segment {
        PathSegment::MoveTo { to } => PathSegment::MoveTo { to: transform(to) },
        PathSegment::LineTo { to } => PathSegment::LineTo { to: transform(to) },
        PathSegment::QuadTo { control, to } => PathSegment::QuadTo {
            control: transform(control),
            to: transform(to),
        },
        PathSegment::CubicTo {
            control1,
            control2,
            to,
        } => PathSegment::CubicTo {
            control1: transform(control1),
            control2: transform(control2),
            to: transform(to),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees,
            large_arc,
            sweep_clockwise,
            to,
        } => PathSegment::ArcTo {
            radius_x: radius_x * 0.8,
            radius_y: radius_y * 0.8,
            x_axis_rotation_degrees: x_axis_rotation_degrees + 45.0,
            large_arc,
            sweep_clockwise,
            to: transform(to),
        },
        PathSegment::Close => PathSegment::Close,
    }
}

fn scale_time(ms: i64, min_ms: i64, max_ms: i64, range: f64) -> f64 {
    if max_ms <= min_ms {
        return (range / 2.0).round();
    }
    let ratio = (ms - min_ms) as f64 / (max_ms - min_ms) as f64;
    (ratio * range).round()
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "gantt".to_string(),
        reason: message.into(),
    }
}
