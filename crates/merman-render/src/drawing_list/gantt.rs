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
use crate::drawing_list::flowchart::{elliptical_rounded_rect_path, rounded_rect_path};
use crate::drawing_list::support::{
    PortableStyleResolver, navigation_security, portable_navigation_uri, stroke, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::gantt::{GanttSvgTransformOrigins, scale_time};
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

mod today_marker;

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
    path_transform_bases: BTreeMap<String, Point>,
    task_radius_attributes: BTreeMap<String, Point>,
    transform_origins: GanttSvgTransformOrigins<'a>,
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
            path_transform_bases: BTreeMap::new(),
            task_radius_attributes: BTreeMap::new(),
            transform_origins: GanttSvgTransformOrigins::new(layout, session.local_time_zone()),
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
        let mut root_semantic = self.document.resolve_mermaid_semantic(SemanticAnnotation {
            id: "gantt.document".to_string(),
            role: SemanticRole::Document,
            title: acc_title.map(str::to_owned),
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
        self.emit_axis(&self.layout.bottom_ticks, self.layout.bottom_axis_y(), true)?;
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
        // Mermaid keeps the title element even when its text is empty.
        let default_name =
            self.emit_title(title.as_deref().unwrap_or_default(), acc_title.is_none())?;
        if let Some(name) = default_name {
            root_semantic.title = Some(if name.is_empty() {
                self.metadata.diagram_type.clone()
            } else {
                name
            });
        }
        self.document.push_semantic(root_semantic)?;

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
                    task_text_height_attribute: matches!(
                        self.navigation_security,
                        MermaidNavigationSecurity::Loose
                    )
                    .then_some(self.layout.bar_height),
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    path_transform_bases: self.path_transform_bases,
                    task_radius_attributes: self.task_radius_attributes,
                    text_classes: self.text_classes,
                    dom_ids: self.dom_ids,
                }),
            },
        })
    }

    fn begin_collection(&mut self, id: &str) -> Result<()> {
        self.document.push_mermaid_semantic(SemanticAnnotation {
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
            let (x, y) = self.transform_origins.exclude(index, exclude);
            self.path_transform_bases
                .insert(format!("gantt.exclude.{index}"), Point::new(x, y));
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
            let label = self.document.resolve_mermaid_text(&tick.label)?;
            let text_y = if bottom { 13.0 } else { -3.0 };
            let text_spec = TextEmitSpec {
                origin: Point::new(0.0, text_y),
                font_size: 10.0,
                weight: 400,
                // Mermaid assigns axis labels `fill="#000"` after D3 builds the axis;
                // this is independent of the surrounding Gantt text theme.
                color: AXIS_CURRENT_COLOR,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Alphabetic,
                italic: false,
            };
            let text_bounds = self.measure_text_bounds(&label, &text_spec)?;
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
                &label,
                text_spec,
                text_bounds,
            )?;
            self.document.push_control(DrawingCommand::EndLayer)?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_control(DrawingCommand::Restore)?;
            self.document.push_mermaid_semantic(SemanticAnnotation {
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
        self.document.push_mermaid_semantic(SemanticAnnotation {
            id: axis_semantic_id,
            role: SemanticRole::Group,
            // The source names ticks, not the axis collection itself.
            title: None,
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
            let style = if task.bar.width == 0.0 || task.bar.height == 0.0 {
                // SVG suppresses zero-extent rectangles, including their stroke. A closed
                // zero-width path would otherwise paint a line in renderer-neutral hosts.
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: None,
                }
            } else {
                self.task_style(&task.bar.class)
            };
            let bar_id = format!("{semantic_id}.bar");
            self.task_radius_attributes
                .insert(bar_id.clone(), Point::new(task.bar.rx, task.bar.ry));
            let (x, y) = self.transform_origins.task(task);
            self.path_transform_bases
                .insert(bar_id.clone(), Point::new(x, y));
            self.dom_ids.insert(bar_id.clone(), task.bar.id.clone());
            self.path_classes
                .insert(bar_id.clone(), task.bar.class.clone());
            if source.milestone {
                // CSS transforms the complete painted rect, including its stroke. The origin
                // follows the task row even when `vert` changes the rectangle's y and height.
                let x = task.bar.x + task.bar.width / 2.0;
                let y = task.order as f64 * (self.layout.bar_height + self.layout.bar_gap)
                    + self.layout.top_padding
                    + self.layout.bar_height / 2.0;
                let component = std::f64::consts::FRAC_1_SQRT_2 * 0.8;
                self.document.push_control(DrawingCommand::Save)?;
                self.document
                    .push_control(DrawingCommand::ConcatTransform {
                        transform: Transform {
                            a: component,
                            b: component,
                            c: -component,
                            d: component,
                            e: x - component * x + component * y,
                            f: y - component * x - component * y,
                        },
                    })?;
            }
            self.add_path(
                bar_id,
                elliptical_rounded_rect_path(
                    task.bar.x + task.bar.width / 2.0,
                    task.bar.y + task.bar.height / 2.0,
                    task.bar.width,
                    task.bar.height,
                    task.bar.rx,
                    task.bar.ry,
                ),
                style,
            )?;
            if source.milestone {
                self.document.push_control(DrawingCommand::Restore)?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
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
            let label_class = task_label_classes(task, source, self.layout);
            let label_color = self.task_text_style(&label_class);
            let label_size = if source.vert {
                // Mermaid's `.vertText` CSS overrides the source font-size attribute.
                15.0
            } else {
                task.label.font_size
            };
            let anchor = label_anchor(&label_class);
            let clickable = source.classes.iter().any(|class| class == "clickable");
            self.text_classes.insert(semantic_id.clone(), label_class);
            self.dom_ids
                .insert(semantic_id.clone(), task.label.id.clone());
            let resolved = self
                .document
                .resolve_mermaid_layout_text(&task.label.text)?;
            let text = self.document.normalize_normal_text(&resolved)?;
            let spec = TextEmitSpec {
                origin: Point::new(task.label.x, task.label.y),
                font_size: label_size,
                weight: if clickable { 700 } else { 400 },
                color: label_color,
                anchor,
                baseline: TextBaseline::Alphabetic,
                italic: source.milestone,
            };
            let text_start = self.document.command_count();
            if !text.is_empty() {
                let bounds = self.measure_text_bounds(&text, &spec)?;
                self.emit_text_in_bounds(&semantic_id, &text, spec, bounds)?;
            }
            // Both hit targets use the final label text as their default name. Delaying only
            // metadata preserves all-bars-before-labels paint order and avoids decoding the
            // resolved name again; each owned metadata copy is charged by the builder.
            for (id, role) in [
                (format!("gantt.task.{index}"), SemanticRole::Node),
                (semantic_id, SemanticRole::Label),
            ] {
                let title = self.document.resolved_text_name_since(text_start)?;
                self.document.push_semantic(SemanticAnnotation {
                    id,
                    role,
                    title: Some(title),
                    // Section membership is layout context, not an authored description.
                    description: None,
                    link: portable_navigation_uri(
                        self.model.links.get(&source.id).map(String::as_str),
                        self.navigation_security,
                    ),
                })?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
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
            let mut last_text = None;
            for (index, line) in section.lines.iter().enumerate().rev() {
                let line = self.document.resolve_mermaid_layout_text(line)?;
                if crate::gantt::section_line_has_text(&line) {
                    last_text = Some(index);
                    break;
                }
            }
            let mut cursor = crate::gantt::GanttSectionLineCursor::new(
                section.y,
                self.layout.section_font_size,
                section.lines.len(),
            );
            let text_start = self.document.command_count();
            for (line_index, line) in section.lines.iter().enumerate() {
                let resolved = self.document.resolve_mermaid_layout_text(line)?;
                let line = resolved.as_ref();
                let has_later_text = last_text.is_some_and(|last| line_index < last);
                let parts = cursor.normalized_parts(line, has_later_text);
                let y = cursor.next_line(line, has_later_text);
                let spec = TextEmitSpec {
                    origin: Point::new(section.x, y),
                    font_size: self.layout.section_font_size,
                    weight: 400,
                    color: self.title_fill,
                    anchor: TextAnchor::Start,
                    baseline: TextBaseline::Central,
                    italic: false,
                };
                let session = self.session;
                let family = &self.theme.font_family;
                let font = self.font.clone();
                let obligation = self.text_obligation.clone();
                self.document.draw_host_text_iter(parts, move |text| {
                    let bounds = measure_text_bounds(session, family, &text, &spec)?;
                    Ok(spec.into_run(text, bounds, font, obligation))
                })?;
            }
            let name = self.document.resolved_text_name_since(text_start)?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Label,
                title: Some(name),
                description: None,
                link: None,
            })?;
        }
        Ok(())
    }

    fn emit_today_marker(&mut self) -> Result<()> {
        let marker = self.layout.today_marker.trim();
        if marker.eq_ignore_ascii_case("off") {
            return Ok(());
        }
        let (color, width, opacity) = self.resolve_today_marker(marker)?;
        let x = if self.layout.tasks.is_empty() {
            // An empty D3 time domain emits x1/x2="NaN". SVG's invalid length fallback is zero,
            // so the pinned browser output paints the marker at the viewport's left edge.
            // Resolve that used coordinate here; public geometry must stay finite.
            0.0
        } else {
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
            scale_time(self.session.unix_millis(), min_ms, max_ms, range) + self.layout.left_padding
        };
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
        self.document.push_mermaid_semantic(SemanticAnnotation {
            id: "gantt.today".to_string(),
            role: SemanticRole::Label,
            // Mermaid renders an unnamed marker; hosts may supply an explicit name.
            title: None,
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_title(&mut self, title: &str, name_document: bool) -> Result<Option<String>> {
        self.text_classes
            .insert("gantt.title".to_string(), "titleText".to_string());
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "gantt.title".to_string(),
            })?;
        let spec = TextEmitSpec {
            origin: Point::new(self.layout.title_x, self.layout.title_y),
            font_size: 18.0,
            weight: 400,
            color: self.title_text_fill,
            anchor: TextAnchor::Middle,
            baseline: TextBaseline::Alphabetic,
            italic: false,
        };
        // Source titleText is an ordinary SVG text node: collapse its source whitespace before
        // measurement. Public run edits remain literal and are never normalized by the encoder.
        let resolved = self.document.resolve_mermaid_layout_text(title)?;
        let text = self.document.normalize_normal_text(&resolved)?;
        let bounds = self.measure_text_bounds(&text, &spec)?;
        let text_start = self.document.command_count();
        self.emit_text_in_bounds("gantt.title", &text, spec, bounds)?;
        let title_name = self.document.resolved_text_name_since(text_start)?;
        let document_name = name_document
            .then(|| self.document.resolved_text_name_since(text_start))
            .transpose()?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: "gantt.title".to_string(),
            role: SemanticRole::Label,
            title: Some(title_name),
            description: None,
            link: None,
        })?;
        Ok(document_name)
    }

    fn measure_text_bounds(&self, text: &str, spec: &TextEmitSpec) -> Result<Rect> {
        measure_text_bounds(self.session, &self.theme.font_family, text, spec)
    }

    fn emit_text_in_bounds(
        &mut self,
        _semantic_id: &str,
        text: &str,
        spec: TextEmitSpec,
        bounds: Rect,
    ) -> Result<()> {
        let font = self.font.clone();
        let obligation = self.text_obligation.clone();
        self.document.draw_host_text(text, move |owned| {
            spec.into_run(owned, bounds, font, obligation)
        })
    }
}

fn measure_text_bounds(
    session: &RenderSession,
    family: &str,
    text: &str,
    spec: &TextEmitSpec,
) -> Result<Rect> {
    if text.is_empty() {
        return Ok(Rect::new(spec.origin.x, spec.origin.y, 0.0, 0.0));
    }
    let measurement_style = MeasurementTextStyle {
        font_family: Some(family.to_string()),
        font_size: spec.font_size,
        font_weight: Some(spec.weight.to_string()),
        font_style: spec.italic.then(|| "italic".to_string()),
    };
    let measurer =
        session.controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
    let width = measurer
        .measure_svg_raw_text_bbox_width_px(text, &measurement_style)
        .max(1.0);
    let height = measurer
        .measure_svg_simple_text_bbox_height_px(text, &measurement_style)
        .max(1.0);
    session.checkpoint(OperationPhase::Emit)?;
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

impl TextEmitSpec {
    fn into_run(
        self,
        text: String,
        bounds: Rect,
        font: FontDescriptor,
        obligation: TextObligation,
    ) -> TextRun {
        let TextEmitSpec {
            origin,
            font_size,
            weight,
            color,
            anchor,
            baseline,
            italic,
        } = self;
        TextRun {
            text,
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
        }
    }
}

impl GanttBuilder<'_> {
    fn row_fill(&self, row: &GanttRowLayout) -> Color {
        match class_suffix(&row.class) {
            Some(0) => self.section_fill,
            Some(2) => self.section_fill_alt2,
            Some(1 | 3) => self.section_fill_alt,
            _ => self.text_fill,
        }
    }

    fn task_style(&self, class: &str) -> PathStyle {
        // Apply the source stylesheet's numbered rules in cascade order. Unmatched sections
        // inherit root fill and no stroke; the unnumbered .vert rule still applies.
        let (fill, border) = if numbered_class(class, "doneCrit") {
            (self.done_task_fill, Some(self.crit_border))
        } else if numbered_class(class, "activeCrit") {
            (self.active_task_fill, Some(self.crit_border))
        } else if numbered_class(class, "crit") {
            (self.crit_fill, Some(self.crit_border))
        } else if numbered_class(class, "done") {
            (self.done_task_fill, Some(self.done_task_border))
        } else if numbered_class(class, "active") {
            (self.active_task_fill, Some(self.active_task_border))
        } else if numbered_class(class, "task") {
            (self.task_fill, Some(self.task_border))
        } else {
            (self.text_fill, None)
        };
        let border = if class.split_whitespace().any(|token| token == "vert") {
            Some(self.vert_line)
        } else {
            border
        };
        PathStyle {
            fill_rule: FillRule::NonZero,
            fill: Some(Paint::solid(fill)),
            stroke: border.map(|color| stroke(color, 2.0)),
        }
    }

    fn task_text_style(&self, class: &str) -> Color {
        let has = |name| class.split_whitespace().any(|token| token == name);
        // Mermaid defines numbered paint rules only for sections 0..3. Resolve those exact
        // rules here, rather than inferring a style from flags that may not have a CSS rule.
        let numbered = |prefix| numbered_class(class, prefix);
        let outside = has("taskTextOutsideLeft") || has("taskTextOutsideRight");
        let done = numbered("doneText") || numbered("doneCritText");
        // Both selectors have two classes and !important; the done-outside rule is later.
        if outside && done {
            return self.task_text_outside_fill;
        }
        if has("clickable") && (outside || has("taskText")) {
            return self.task_text_clickable_fill;
        }
        // Of the one-class !important rules, activeCritText follows vertText, which follows
        // activeText/doneText/doneCritText. Preserve that order even for combined task tags.
        if numbered("activeCritText") {
            return self.task_text_dark_fill;
        }
        if has("vertText") {
            return self.vert_line;
        }
        if done || numbered("activeText") {
            return self.task_text_dark_fill;
        }
        if numbered("taskTextOutside") {
            return self.task_text_outside_fill;
        }
        if numbered("taskText") {
            return self.task_text_fill;
        }
        if outside {
            return self.task_text_dark_fill;
        }
        self.text_fill
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

fn numbered_class(class: &str, prefix: &str) -> bool {
    class.split_whitespace().any(|token| {
        token
            .strip_prefix(prefix)
            .is_some_and(|suffix| matches!(suffix, "0" | "1" | "2" | "3"))
    })
}

fn class_suffix(class: &str) -> Option<usize> {
    class
        .split_whitespace()
        .find_map(|value| value.strip_prefix("section")?.parse::<usize>().ok())
}

fn label_anchor(class: &str) -> TextAnchor {
    if class.split_whitespace().any(|token| token == "vertText") {
        TextAnchor::Middle
    } else if class.contains("taskTextOutsideLeft") {
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
                "gantt": { "displayMode": "compact", "useWidth": 800, "topAxis": true, "topPadding": 70 }
            })))
            .parse_diagram_for_render_model_sync(COMPACT_TASKS, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_control(OperationControl::new())
            .unwrap();
        crate::family::prepare(parsed, &crate::LayoutOptions::default(), session).unwrap()
    }

    fn render_task_fixture(
        source: &str,
        config: serde_json::Value,
    ) -> crate::family::RenderedDrawingList {
        prepare_task_fixture(source, config)
            .render_drawing_list(DrawingListPolicy::VectorOnly, Default::default())
            .unwrap()
    }

    fn prepare_task_fixture(source: &str, config: serde_json::Value) -> FamilyRenderArtifact {
        let parsed = Engine::new()
            .with_site_config(MermaidConfig::from_value(config))
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        crate::family::prepare(parsed, &crate::LayoutOptions::default(), session).unwrap()
    }

    #[test]
    fn today_marker_rejects_unresolved_paint_instead_of_ignoring_valid_css() {
        for value in ["var(--marker)", "currentColor", "color(display-p3 1 0 0)"] {
            let source = format!(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker stroke:{value}\nTask :a, 2023-12-31, 3d\n"
            );
            let error = prepare_task_fixture(&source, json!({}))
                .render_drawing_list(DrawingListPolicy::VectorOnly, Default::default())
                .err()
                .expect("unresolved paint must not produce a partial drawing");
            assert!(
                matches!(error, Error::DrawingListUnavailable { family, reason }
                    if family == "gantt" && reason.contains("todayMarker.stroke")),
                "{value} must retain a structured unsupported-paint result"
            );
        }
    }

    #[test]
    fn today_marker_resolves_source_order_before_publishing_paint() {
        let theme = Color::rgba(0x12, 0x34, 0x56, 255);
        let blue = Color::rgba(0, 0, 255, 255);
        let red = Color::rgba(255, 0, 0, 255);
        for (marker, expected, expected_width, expected_opacity) in [
            ("stroke:rgb(0,0,255),opacity:0.5", theme, 2.0, 0.5),
            ("stroke:#00f;opacity:0.5", theme, 2.0, 0.5),
            ("stroke:#00f, opacity:0.5", blue, 2.0, 0.5),
            ("stroke:rgb(0#44;0#44;255),opacity:0.5", blue, 2.0, 0.5),
            ("stroke:red,stroke:rgb(0,0,255),opacity:0.5", red, 2.0, 0.5),
            ("stroke:red !important,stroke:blue", red, 2.0, 1.0),
            (
                "stroke:red !important,stroke:blue !important",
                blue,
                2.0,
                1.0,
            ),
            (
                "stroke:blue, stroke-width:4px, opacity:25%",
                blue,
                4.0,
                0.25,
            ),
            ("opacity:0.5!important,opacity:0.2", theme, 2.0, 0.5),
        ] {
            let source = format!(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker {marker}\nTask :a, 2023-12-31, 3d\n"
            );
            let rendered = render_task_fixture(
                &source,
                json!({"themeVariables": {"todayLineColor": "#123456"}}),
            );
            let mut found = false;
            let mut opacity = None;
            for command in &rendered.document().commands {
                match command {
                    DrawingCommand::SetOpacity { opacity: value } => opacity = Some(*value),
                    DrawingCommand::DrawPath { path, style }
                        if path.as_str() == "gantt.today.line" =>
                    {
                        let stroke = style.stroke.as_ref().unwrap();
                        assert_eq!(stroke.paint, Paint::solid(expected), "{marker}");
                        assert_eq!(stroke.width, expected_width, "{marker}");
                        assert_eq!(opacity, Some(expected_opacity), "{marker}");
                        found = true;
                    }
                    _ => {}
                }
            }
            assert!(found, "{marker}");
        }
    }

    #[test]
    fn vertical_marker_labels_use_cascaded_font_size() {
        let rendered = render_task_fixture(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nMarker: vert,marker,2026-01-01,1d\n",
            json!({}),
        );
        let run = rendered
            .document()
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawText { run } if run.text == "Marker" => Some(run),
                _ => None,
            })
            .expect("vertical marker label");
        assert_eq!(run.style.font_size, 15.0);
    }

    #[test]
    fn task_labels_collapse_svg_whitespace_before_measurement() {
        for (source_label, expected) in [
            ("Alpha     Beta", "Alpha Beta"),
            ("Alpha\t Beta ", "Alpha Beta"),
            ("Alpha #32; Beta", "Alpha Beta"),
            ("Alpha\u{a0}Beta", "Alpha\u{a0}Beta"),
        ] {
            let source = format!(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\n{source_label} :a, 2026-01-01, 1d\n"
            );
            let rendered = render_task_fixture(&source, json!({}));
            let run = rendered
                .document()
                .commands
                .iter()
                .find_map(|command| match command {
                    DrawingCommand::DrawText { run } if run.text.starts_with("Alpha") => Some(run),
                    _ => None,
                })
                .unwrap();
            assert_eq!(run.text, expected);
            let spec = TextEmitSpec {
                origin: run.origin,
                font_size: run.style.font_size,
                weight: 400,
                color: Color::rgba(0, 0, 0, 255),
                anchor: run.anchor,
                baseline: run.baseline,
                italic: false,
            };
            assert_eq!(
                run.bounds,
                measure_text_bounds(
                    rendered.session(),
                    &run.style.font.families.join(", "),
                    expected,
                    &spec
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn titles_collapse_source_whitespace_before_measurement_and_default_naming() {
        for (source_title, expected) in [
            ("   Alpha     Beta  ", "Alpha Beta"),
            ("Alpha #32; Beta", "Alpha Beta"),
            ("Alpha\u{a0}Beta", "Alpha\u{a0}Beta"),
            ("#32;#32;", ""),
        ] {
            for accessibility in ["", "accTitle: Independent name\n"] {
                let source = format!(
                    "gantt\ntitle {source_title}\n{accessibility}dateFormat YYYY-MM-DD\ntodayMarker off\nTask :a, 2026-01-01, 1d\n"
                );
                let rendered = render_task_fixture(&source, json!({}));
                let document = rendered.document();
                let title_start = document.commands.iter().position(|command| matches!(command,
                    DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "gantt.title"
                )).unwrap();
                let DrawingCommand::DrawText { run } = &document.commands[title_start + 1] else {
                    panic!("even an empty source title retains its text command")
                };
                assert_eq!(run.text, expected);
                let spec = TextEmitSpec {
                    origin: run.origin,
                    font_size: run.style.font_size,
                    weight: 400,
                    color: Color::rgba(0, 0, 0, 255),
                    anchor: run.anchor,
                    baseline: run.baseline,
                    italic: false,
                };
                assert_eq!(
                    run.bounds,
                    measure_text_bounds(
                        rendered.session(),
                        &run.style.font.families.join(", "),
                        expected,
                        &spec,
                    )
                    .unwrap()
                );
                let name = |id| {
                    document
                        .semantics
                        .iter()
                        .find(|s| s.id == id)
                        .unwrap()
                        .title
                        .as_deref()
                };
                assert_eq!(name("gantt.title"), Some(expected));
                assert_eq!(
                    name("gantt.document"),
                    Some(if !accessibility.is_empty() {
                        "Independent name"
                    } else if expected.is_empty() {
                        "gantt"
                    } else {
                        expected
                    })
                );
            }
        }
    }

    #[test]
    fn task_styles_outside_the_four_numbered_rules_inherit_root_paint() {
        for count in [4, 5] {
            for state in ["", "active,", "done,", "crit,", "milestone,", "vert,"] {
                let mut source = String::from("gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\n");
                for index in 0..5 {
                    source.push_str(&format!("section S{index}\n"));
                    if state == "vert," {
                        // Vertical markers alone do not register categories in Mermaid.
                        source.push_str(&format!("Anchor{index} :anchor{index}, 2026-01-01, 1d\n"));
                    }
                    source.push_str(&format!("Task{index} :{state}t{index}, 2026-01-01, 1d\n"));
                }
                let rendered = render_task_fixture(
                    &source,
                    json!({
                        "gantt": { "numberSectionStyles": count },
                        "themeVariables": { "textColor": "#123456", "vertLineColor": "#abcdef" }
                    }),
                );
                let styles = rendered
                    .document()
                    .commands
                    .iter()
                    .filter_map(|command| match command {
                        DrawingCommand::DrawPath { path, style }
                            if path.as_str().ends_with(".bar")
                                && (state != "vert,"
                                    || path
                                        .as_str()
                                        .split('.')
                                        .nth(2)
                                        .unwrap()
                                        .parse::<usize>()
                                        .unwrap()
                                        % 2
                                        == 1) =>
                        {
                            Some(style)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(styles.len(), 5);
                if count == 4 {
                    assert_eq!(styles[4], styles[0], "default four-style modulo: {state}");
                } else {
                    assert_eq!(
                        styles[4].fill,
                        Some(Paint::solid(Color::rgba(0x12, 0x34, 0x56, 255))),
                        "{state}"
                    );
                    if state == "vert," {
                        assert_eq!(
                            styles[4].stroke.as_ref().unwrap().paint,
                            Paint::solid(Color::rgba(0xab, 0xcd, 0xef, 255))
                        );
                    } else {
                        assert!(
                            styles[4].stroke.is_none(),
                            "unmatched numbered rule: {state}"
                        );
                    }
                }
            }
        }
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
                        (&layout.bottom_ticks, layout.height - 50.0, 13.0)
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
                    // Default accessible names use the same final text as the visible label.
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
            assert_eq!(node.title.as_deref(), Some(task.task.trim_end_matches(' ')));
            assert_eq!(node.title, label.title);
            assert_eq!(node.description, label.description);
            assert_eq!(node.link, label.link);
            if task.id == "a" {
                assert_eq!(node.link.as_deref(), Some("https://example.com/task"));
                assert_eq!(node.description, None);
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
