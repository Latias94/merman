//! Renderer-neutral Gantt adapter.
//!
//! Gantt layout already owns the final time scale, row placement, task state, and label
//! placement.  This adapter makes the SVG stylesheet's built-in state matrix explicit so a host
//! renderer can draw the same visible chart without evaluating CSS selectors.

use super::{
    GanttSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families_for,
};
use crate::config::config_font_family_css_raw;
use crate::drawing_list::builder::DrawingListBuilder;
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
    Color, DrawingCommand, DrawingListPolicy, FillRule, FontDescriptor, FontStyle, Paint,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type GanttPair = FamilyPair<GanttDiagramRenderModel, GanttDiagramLayout>;

// D3's domain and tick lines specify currentColor themselves; the parent tick's gridColor
// stroke is not inherited by them. The isolated source SVG has no authored CSS color override.
const AXIS_CURRENT_COLOR: Color = Color::rgba(0, 0, 0, 255);

pub(crate) fn build_gantt_document(
    pair: &GanttPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: impl Into<super::DocumentBudget>,
    session: &RenderSession,
) -> Result<RenderDocument> {
    GanttBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct GanttBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    document: DrawingListBuilder<'a>,
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
        limits: impl Into<super::DocumentBudget>,
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
        let font_css = config_font_family_css_raw(config);
        let font = FontDescriptor {
            families: parse_font_families_for(font_css, RenderFamilyKind::Gantt)?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        let mut document = DrawingListBuilder::new(policy, limits, session);
        document.push_control(DrawingCommand::Save)?;
        document.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "gantt.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            document,
            model,
            layout,
            exclude_fill: color("excludeBkgColor", &theme.exclude_bkg_color)?,
            section_fill: color("sectionBkgColor", &theme.section_bkg_color)?,
            section_fill_alt: color("altSectionBkgColor", &theme.alt_section_bkg_color)?,
            section_fill_alt2: color("sectionBkgColor2", &theme.section_bkg_color2)?,
            title_fill: color("titleColor", &theme.title_color)?,
            title_text_fill: color("titleColor", &theme.title_text_color)?,
            text_fill: color("textColor", &theme.text_color)?,
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
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let acc_title = self
            .model
            .acc_title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty());
        let title = self
            .model
            .title
            .clone()
            .or_else(|| self.metadata.title.clone());
        self.document.push_semantic(SemanticAnnotation {
            id: "gantt.document".to_string(),
            role: SemanticRole::Document,
            title: acc_title
                .map(str::to_owned)
                .or_else(|| title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self
                .model
                .acc_descr
                .as_deref()
                .map(|description| description.trim_end_matches('\n'))
                .filter(|description| !description.trim().is_empty())
                .map(str::to_owned),
            link: None,
        })?;

        self.emit_background()?;
        if self.layout.has_excludes_layer {
            self.begin_collection("gantt.excludes")?;
            self.emit_excludes()?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
        }
        self.emit_axis(
            &self.layout.bottom_ticks,
            self.layout.height - self.layout.top_padding,
            true,
        )?;
        if self.layout.top_axis {
            self.emit_axis(&self.layout.top_ticks, self.layout.top_padding, false)?;
        }
        self.begin_collection("gantt.rows")?;
        self.emit_rows()?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.begin_collection("gantt.tasks")?;
        self.emit_tasks()?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.begin_collection("gantt.sections")?;
        self.emit_section_titles()?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.emit_today_marker()?;
        if let Some(title) = title.as_deref().filter(|title| !title.is_empty()) {
            self.emit_title(title)?;
        }

        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_control(DrawingCommand::Restore)?;

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

        let document = self.document.finish(
            Viewport::new(Rect::new(0.0, 0.0, self.layout.width, self.layout.height)),
            BTreeMap::from([(
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
        )?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Gantt,
                body: SvgStructureBody::Gantt(GanttSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    expose_accessibility_title: acc_title.is_some(),
                    bar_height: self.layout.bar_height,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                    dom_ids: self.dom_ids,
                }),
            },
        })
    }

    fn begin_collection(&mut self, id: &str) -> Result<()> {
        self.document.push_semantic(SemanticAnnotation {
            id: id.to_owned(),
            role: SemanticRole::Group,
            title: None,
            description: None,
            link: None,
        })?;
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: id.to_owned(),
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
        self.document.push_control(DrawingCommand::Save)?;
        self.document
            .push_control(DrawingCommand::ConcatTransform {
                transform: Transform {
                    e: self.layout.left_padding,
                    f: y,
                    ..Transform::IDENTITY
                },
            })?;
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: axis_semantic_id.clone(),
            })?;
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
                    to: Point::new(0.5, tick_size),
                },
                PathSegment::LineTo {
                    to: Point::new(0.5, 0.5),
                },
                PathSegment::LineTo {
                    to: Point::new(range + 0.5, 0.5),
                },
                PathSegment::LineTo {
                    to: Point::new(range + 0.5, tick_size),
                },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(AXIS_CURRENT_COLOR, 0.0)),
            },
        )?;

        for (index, tick) in ticks.iter().enumerate() {
            let text_y = if bottom { 13.0 } else { -3.0 };
            let text_spec = TextEmitSpec {
                origin: Point::new(0.0, text_y),
                font_size: 10.0,
                weight: 400,
                color: self.text_fill,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Alphabetic,
                italic: false,
            };
            let text_bounds = self.measure_text_bounds(&tick.label, &text_spec)?;
            // The source applies opacity to the tick group after painting both children.
            // A layer retains that compositing order rather than fading each child separately.
            let left = text_bounds.x.min(-0.5);
            let top = text_bounds.y.min(0.0_f64.min(tick_size) - 0.5);
            let right = (text_bounds.x + text_bounds.width).max(0.5);
            let bottom_edge =
                (text_bounds.y + text_bounds.height).max(0.0_f64.max(tick_size) + 0.5);
            let tick_semantic_id = format!("{axis_semantic_id}.tick.{index}");
            self.semantic_classes
                .insert(tick_semantic_id.clone(), "tick".to_string());
            self.document.push_control(DrawingCommand::Save)?;
            self.document
                .push_control(DrawingCommand::ConcatTransform {
                    transform: Transform {
                        e: tick.x - self.layout.left_padding + 0.5,
                        ..Transform::IDENTITY
                    },
                })?;
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: tick_semantic_id.clone(),
                })?;
            self.document.push_control(DrawingCommand::BeginLayer {
                bounds: Rect::new(left, top, right - left, bottom_edge - top),
                opacity: 0.8,
                blend_mode: merman_display_list::BlendMode::Normal,
            })?;
            let tick_path_id = format!("gantt.axis.{axis_name}.tick.{index}");
            self.add_path(
                tick_path_id,
                vec![
                    PathSegment::MoveTo {
                        to: Point::new(0.0, 0.0),
                    },
                    PathSegment::LineTo {
                        to: Point::new(0.0, tick_size),
                    },
                ],
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(AXIS_CURRENT_COLOR, 1.0)),
                },
            )?;
            self.emit_text_in_bounds(
                &format!("{tick_semantic_id}.label"),
                &tick.label,
                text_spec,
                text_bounds,
            )?;
            self.document.push_control(DrawingCommand::EndLayer)?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_control(DrawingCommand::Restore)?;
            self.document.push_semantic(SemanticAnnotation {
                id: tick_semantic_id,
                role: SemanticRole::Label,
                title: Some(tick.label.clone()),
                description: None,
                link: None,
            })?;
        }
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_control(DrawingCommand::Restore)?;
        self.document.push_semantic(SemanticAnnotation {
            id: axis_semantic_id,
            role: SemanticRole::Group,
            title: Some(format!("{} axis", if bottom { "Bottom" } else { "Top" })),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_rows(&mut self) -> Result<()> {
        for (index, row) in self.layout.rows.iter().enumerate() {
            let fill = self.row_fill(row);
            self.path_classes
                .insert(format!("gantt.row.{index}"), row.class.clone());
            self.document.push_control(DrawingCommand::Save)?;
            self.document
                .push_control(DrawingCommand::SetOpacity { opacity: 0.2 })?;
            self.add_path(
                format!("gantt.row.{index}"),
                rect_path(row.x, row.y, row.width, row.height, 0.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: None,
                },
            )?;
            self.document.push_control(DrawingCommand::Restore)?;
        }
        Ok(())
    }

    fn emit_tasks(&mut self) -> Result<()> {
        // Mermaid paints every bar before any label, with vertical markers last in each pass.
        // Compact rows can share a lane, so interleaving bars and labels hides earlier labels.
        let tasks = self
            .layout
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| !task.vert)
            .chain(
                self.layout
                    .tasks
                    .iter()
                    .enumerate()
                    .filter(|(_, task)| task.vert),
            );
        let mut labels = Vec::new();
        for (index, task) in tasks {
            let source = find_task(self.model, &task.id).ok_or_else(|| {
                invalid(format!(
                    "Gantt layout task `{}` has no semantic source",
                    task.id
                ))
            })?;
            let semantic_id = format!("gantt.task.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
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
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(source.task.clone()),
                description: (!source.section.is_empty())
                    .then(|| format!("{} section", source.section)),
                link: portable_navigation_uri(
                    self.model.links.get(&source.id).map(String::as_str),
                    self.navigation_security,
                ),
            })?;
            labels
                .try_reserve(1)
                .map_err(|_| Error::DrawingListAllocationFailed {
                    collection: "Gantt task label references",
                })?;
            labels.push((index, task, source));
        }

        for (index, task, source) in labels {
            // Separate scopes retain both link hit targets without reusing an SVG group id.
            let semantic_id = format!("gantt.task.{index}.label");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
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
                &semantic_id,
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
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Label,
                title: Some(source.task.clone()),
                description: (!source.section.is_empty())
                    .then(|| format!("{} section", source.section)),
                link: portable_navigation_uri(
                    self.model.links.get(&source.id).map(String::as_str),
                    self.navigation_security,
                ),
            })?;
        }
        Ok(())
    }

    fn emit_section_titles(&mut self) -> Result<()> {
        for (index, section) in self.layout.section_titles.iter().enumerate() {
            let semantic_id = format!("gantt.section.{index}");
            self.text_classes
                .insert(semantic_id.clone(), section.class.clone());
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
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
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Label,
                title: Some(section.section.clone()),
                description: None,
                link: None,
            })?;
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
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "gantt.today".to_string(),
            })?;
        self.semantic_classes
            .insert("gantt.today".to_string(), "today".to_string());
        self.path_classes
            .insert("gantt.today.line".to_string(), "today".to_string());
        self.document.push_control(DrawingCommand::Save)?;
        self.document
            .push_control(DrawingCommand::SetOpacity { opacity })?;
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
        self.document.push_control(DrawingCommand::Restore)?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: "gantt.today".to_string(),
            role: SemanticRole::Label,
            title: Some("Today".to_string()),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        self.text_classes
            .insert("gantt.title".to_string(), "titleText".to_string());
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "gantt.title".to_string(),
            })?;
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
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: "gantt.title".to_string(),
            role: SemanticRole::Label,
            title: Some(title.to_string()),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_text(&mut self, semantic_id: &str, text: &str, spec: TextEmitSpec) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let bounds = self.measure_text_bounds(text, &spec)?;
        self.emit_text_in_bounds(semantic_id, text, spec, bounds)
    }

    fn measure_text_bounds(&self, text: &str, spec: &TextEmitSpec) -> Result<Rect> {
        if text.is_empty() {
            return Ok(Rect::new(spec.origin.x, spec.origin.y, 0.0, 0.0));
        }
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.theme.font_family.clone()),
            font_size: spec.font_size,
            font_weight: Some(spec.weight.to_string()),
            font_style: spec.italic.then(|| "italic".to_string()),
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
        self.session.checkpoint(OperationPhase::Emit)?;
        Ok(Rect::new(
            match spec.anchor {
                TextAnchor::Start => spec.origin.x,
                TextAnchor::Middle => spec.origin.x - width / 2.0,
                TextAnchor::End => spec.origin.x - width,
            },
            spec.origin.y - height / 2.0,
            width,
            height,
        ))
    }

    fn emit_text_in_bounds(
        &mut self,
        semantic_id: &str,
        text: &str,
        spec: TextEmitSpec,
        bounds: Rect,
    ) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let TextEmitSpec {
            origin,
            font_size,
            weight,
            color,
            anchor,
            baseline,
            italic,
        } = spec;
        let font = self.font.clone();
        let obligation = self.text_obligation.clone();
        self.document.draw_host_text(text, move |owned| TextRun {
            text: owned,
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
                    ..font
                },
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
            obligation,
        })?;
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
        self.document.draw_path(id, segments, style)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::FamilyRenderArtifact;
    use merman_core::{Engine, MermaidConfig, OperationControl, ParseOptions};

    const COMPACT_TASKS: &str = "gantt\n\
        dateFormat YYYY-MM-DD\n\
        todayMarker off\n\
        accTitle: Compact release\n\
        accDescr: Linked compact tasks\n\
        section Core\n\
        Marker :vert, marker, 2026-01-01, 1d\n\
        A long task label extending across the following task :a, 2026-01-02, 1d\n\
        Next :b, 2026-01-03, 1d\n\
        Horizon :c, 2026-01-20, 1d\n\
        click a href \"https://example.com/task\"\n";

    fn compact_artifact() -> FamilyRenderArtifact {
        let parsed = Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "securityLevel": "loose",
                "themeVariables": { "gridColor": "#ff0000" },
                "gantt": { "displayMode": "compact", "useWidth": 800, "topAxis": true }
            })))
            .parse_diagram_for_render_model_sync(COMPACT_TASKS, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_control(OperationControl::new())
            .unwrap();
        crate::family::prepare(parsed, &crate::LayoutOptions::default(), session).unwrap()
    }

    #[test]
    fn axis_ticks_composite_lines_and_labels_in_one_public_layer() {
        let artifact = compact_artifact();
        let projection = artifact.layout_json().unwrap();
        let layout: GanttDiagramLayout =
            serde_json::from_value(projection["layout"]["GanttDiagram"].clone()).unwrap();
        let rendered = artifact
            .render_drawing_list(
                DrawingListPolicy::VectorOnly,
                merman_display_list::DrawingListLimits::default(),
            )
            .unwrap();
        let mut in_tick = false;
        let mut layer = None;
        let mut tick_texts = 0;
        let mut semantic_id = "";
        let mut translation = Point::new(0.0, 0.0);
        let mut saves = Vec::new();
        for command in &rendered.document().commands {
            match command {
                DrawingCommand::Save => saves.push(translation),
                DrawingCommand::Restore => translation = saves.pop().unwrap(),
                DrawingCommand::ConcatTransform { transform } => {
                    assert_eq!(
                        (transform.a, transform.b, transform.c, transform.d),
                        (1.0, 0.0, 0.0, 1.0)
                    );
                    translation.x += transform.e;
                    translation.y += transform.f;
                }
                DrawingCommand::BeginSemanticGroup { semantic_id: id } => {
                    semantic_id = id;
                    in_tick = id.contains(".tick.");
                }
                DrawingCommand::EndSemanticGroup => in_tick = false,
                DrawingCommand::BeginLayer {
                    bounds,
                    opacity,
                    blend_mode,
                } if in_tick => {
                    assert_eq!(*opacity, 0.8);
                    assert_eq!(*blend_mode, merman_display_list::BlendMode::Normal);
                    layer = Some(*bounds);
                }
                DrawingCommand::EndLayer => {
                    layer.take().unwrap();
                }
                DrawingCommand::DrawPath { path, style } if in_tick => {
                    assert!(
                        layer.is_some(),
                        "tick lines require group opacity, not per-path opacity"
                    );
                    assert_eq!(
                        style.stroke.as_ref().unwrap().paint,
                        Paint::solid(Color::rgba(0, 0, 0, 255)),
                        "D3 line currentColor does not inherit the tick group's gridColor stroke"
                    );
                    let bounds = layer.unwrap();
                    let resource = rendered
                        .document()
                        .resources
                        .iter()
                        .find_map(|resource| match resource {
                            merman_display_list::DrawingResource::Path(resource)
                                if resource.id == *path =>
                            {
                                Some(resource)
                            }
                            _ => None,
                        })
                        .unwrap();
                    let [
                        PathSegment::MoveTo { to: start },
                        PathSegment::LineTo { to: end },
                    ] = resource.segments.as_slice()
                    else {
                        panic!("tick line")
                    };
                    assert_eq!(*start, Point::new(0.0, 0.0));
                    assert_eq!(end.x, 0.0);
                    assert!(bounds.x <= -0.5 && bounds.x + bounds.width >= 0.5);
                    assert!(bounds.y <= start.y.min(end.y) - 0.5);
                    assert!(bounds.y + bounds.height >= start.y.max(end.y) + 0.5);
                }
                DrawingCommand::DrawText { run } if in_tick => {
                    let bounds = layer.expect("tick text must share the line's opacity layer");
                    assert!(bounds.x <= run.bounds.x && bounds.y <= run.bounds.y);
                    assert!(bounds.x + bounds.width >= run.bounds.x + run.bounds.width);
                    assert!(bounds.y + bounds.height >= run.bounds.y + run.bounds.height);
                    let index: usize = semantic_id.rsplit('.').next().unwrap().parse().unwrap();
                    let bottom = semantic_id.starts_with("gantt.axis.bottom.");
                    let (ticks, y, baseline) = if bottom {
                        (
                            &layout.bottom_ticks,
                            layout.height - layout.top_padding,
                            13.0,
                        )
                    } else {
                        (&layout.top_ticks, layout.top_padding, -3.0)
                    };
                    assert_eq!(run.origin, Point::new(0.0, baseline));
                    assert!((translation.x + run.origin.x - (ticks[index].x + 0.5)).abs() < 1e-9);
                    assert!((translation.y + run.origin.y - (y + baseline)).abs() < 1e-9);
                    tick_texts += 1;
                }
                DrawingCommand::DrawPath { path, .. }
                    if path.as_str().starts_with("gantt.task.") =>
                {
                    assert_eq!(
                        translation,
                        Point::new(0.0, 0.0),
                        "axis state must not leak into tasks"
                    );
                }
                _ => {}
            }
        }
        assert!(!layout.top_ticks.is_empty() && !layout.bottom_ticks.is_empty());
        assert_eq!(
            tick_texts,
            layout.top_ticks.len() + layout.bottom_ticks.len()
        );
        assert!(saves.is_empty());
        assert!(layer.is_none());
    }

    #[test]
    fn compact_tasks_paint_all_bars_before_labels_and_vertical_markers_last() {
        let artifact = compact_artifact();
        let projection = artifact.layout_json().unwrap();
        let layout: GanttDiagramLayout =
            serde_json::from_value(projection["layout"]["GanttDiagram"].clone()).unwrap();
        let first = layout.tasks.iter().find(|task| task.id == "a").unwrap();
        let next = layout.tasks.iter().find(|task| task.id == "b").unwrap();
        assert_eq!(first.bar.y, next.bar.y, "tasks must share a compact lane");
        assert_eq!(label_anchor(&first.label.class), TextAnchor::Start);
        assert!(first.label.x < next.bar.x + next.bar.width);
        assert!(first.label.x + first.label.width > next.bar.x);

        let rendered = artifact
            .render_drawing_list(
                DrawingListPolicy::VectorOnly,
                merman_display_list::DrawingListLimits::default(),
            )
            .unwrap();
        let document = rendered.document();
        let mut scopes = Vec::new();
        let mut task_paints = Vec::new();
        for command in &document.commands {
            match command {
                DrawingCommand::BeginSemanticGroup { semantic_id } => {
                    scopes.push(semantic_id.as_str());
                }
                DrawingCommand::EndSemanticGroup => {
                    scopes.pop().unwrap();
                }
                DrawingCommand::DrawPath { path, .. }
                    if path.as_str().starts_with("gantt.task.") =>
                {
                    let scope = *scopes.last().unwrap();
                    assert_eq!(path.as_str(), format!("{scope}.bar"));
                    task_paints.push(("bar", scope.to_owned()));
                }
                DrawingCommand::DrawText { run }
                    if scopes
                        .last()
                        .is_some_and(|id| id.starts_with("gantt.task.")) =>
                {
                    let scope = *scopes.last().unwrap();
                    let semantic = document.semantics.iter().find(|s| s.id == scope).unwrap();
                    assert_eq!(semantic.role, SemanticRole::Label);
                    assert_eq!(semantic.title.as_deref(), Some(run.text.as_str()));
                    task_paints.push(("label", scope.to_owned()));
                }
                _ => {}
            }
        }
        assert!(scopes.is_empty());
        let order = layout
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| !task.vert)
            .chain(
                layout
                    .tasks
                    .iter()
                    .enumerate()
                    .filter(|(_, task)| task.vert),
            )
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let expected = order
            .iter()
            .map(|index| ("bar", format!("gantt.task.{index}")))
            .chain(
                order
                    .iter()
                    .map(|index| ("label", format!("gantt.task.{index}.label"))),
            )
            .collect::<Vec<_>>();
        assert_eq!(task_paints, expected);
        for (index, task) in layout.tasks.iter().enumerate() {
            let node = document
                .semantics
                .iter()
                .find(|s| s.id == format!("gantt.task.{index}"))
                .unwrap();
            let label = document
                .semantics
                .iter()
                .find(|s| s.id == format!("gantt.task.{index}.label"))
                .unwrap();
            assert_eq!(node.role, SemanticRole::Node);
            assert_eq!(node.title.as_deref(), Some(task.task.as_str()));
            assert_eq!(node.title, label.title);
            assert_eq!(node.description, label.description);
            assert_eq!(node.link, label.link);
            if task.id == "a" {
                assert_eq!(node.link.as_deref(), Some("https://example.com/task"));
                assert_eq!(node.description.as_deref(), Some("Core section"));
            }
        }
        let root = document
            .semantics
            .iter()
            .find(|s| s.id == "gantt.document")
            .unwrap();
        assert_eq!(root.title.as_deref(), Some("Compact release"));
        assert_eq!(root.description.as_deref(), Some("Linked compact tasks"));
    }
}
