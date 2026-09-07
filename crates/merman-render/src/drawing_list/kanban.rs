//! Renderer-neutral Kanban adapter.
//!
//! Kanban's layout already owns the column and card geometry.  This adapter keeps that geometry
//! and projects the ordinary markdown-label subset into host text runs.  Rich HTML, icons, and
//! arbitrary CSS remain explicit structured failures instead of being flattened into a visibly
//! different card.

use super::{
    KanbanSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
};
use crate::drawing_list::flowchart::rounded_rect_path;
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::kanban::{
    KanbanConfigView, KanbanPreparedArtifact, KanbanPreparedItem, KanbanPreparedMarkdownLabel,
};
use crate::model::{Bounds, KanbanDiagramLayout, KanbanItemLayout, KanbanSectionLayout};
use crate::svg::render_theme::KanbanTheme;
use crate::text::{
    TextMeasurer as _, TextStyle as MeasurementTextStyle, WrapMode, wrap_text_lines_measurer,
};
use crate::{Error, Result};
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle as DisplayTextStyle, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type KanbanPair = FamilyPair<KanbanDiagramRenderModel, KanbanPreparedArtifact>;

const BLACK: Color = Color::rgba(0, 0, 0, 255);
const LABEL_LINE_HEIGHT_SCALE: f64 = 1.5;

pub(crate) fn build_kanban_document(
    pair: &KanbanPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    KanbanBuilder::new(pair, metadata, policy, session)?.build()
}

#[derive(Debug, Clone)]
struct LabelPlan {
    lines: Vec<String>,
    width: f64,
    height: f64,
}

impl LabelPlan {
    fn is_empty(&self) -> bool {
        self.lines.is_empty()
            || self
                .lines
                .iter()
                .all(|line| svg_plain_text(line).is_empty())
    }
}

#[derive(Debug, Clone, Copy)]
struct PlainMetrics {
    width: f64,
    height: f64,
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

struct KanbanBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a KanbanDiagramRenderModel,
    layout: &'a KanbanDiagramLayout,
    prepared_sections: &'a [KanbanPreparedMarkdownLabel],
    prepared_items: &'a [KanbanPreparedItem],
    theme: KanbanTheme,
    look: String,
    label_style: MeasurementTextStyle,
    font: FontDescriptor,
    font_size: f64,
    text_color: Color,
    background: Color,
    node_border: Color,
    text_obligation: TextObligation,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
    ticket_links: BTreeMap<String, Option<String>>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> KanbanBuilder<'a> {
    fn new(
        pair: &'a KanbanPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let artifact = pair.layout();
        let (layout, prepared_sections, prepared_items) = artifact.render_parts();
        validate_model(model)?;
        validate_layout(layout, prepared_sections, prepared_items)?;

        let config_view = KanbanConfigView::new(config);
        let settings = config_view.layout_settings();
        let font_size = settings.text_style.font_size.max(1.0);
        let font_family = settings
            .text_style
            .font_family
            .clone()
            .filter(|family| !family.trim().is_empty())
            .unwrap_or_else(|| "sans-serif".to_string());
        let label_style = MeasurementTextStyle {
            font_family: Some(font_family.clone()),
            font_size,
            font_weight: settings.text_style.font_weight.clone(),
            font_style: settings.text_style.font_style.clone(),
        };
        let font = FontDescriptor {
            families: parse_font_families(font_family),
            weight: parse_font_weight(label_style.font_weight.as_deref()),
            style: parse_font_style(label_style.font_style.as_deref()),
            postscript_name: None,
            resource: None,
        };

        let theme = crate::svg::render_theme::PresentationTheme::new(config).kanban()?;
        let styles = PortableStyleResolver::new("kanban");
        let text_color = styles.color("textColor", &theme.text_color)?;
        let background = styles.color("background", &theme.background)?;
        let node_border = styles.color("nodeBorder", &theme.node_border)?;

        Ok(Self {
            metadata,
            session,
            policy,
            model,
            layout,
            prepared_sections,
            prepared_items,
            theme,
            look: config_view.look().as_str().to_string(),
            label_style,
            font,
            font_size,
            text_color,
            background,
            node_border,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
            ticket_links: BTreeMap::new(),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "kanban.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.semantics.push(SemanticAnnotation {
            id: "kanban.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .metadata
                .title
                .clone()
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: None,
            link: None,
        });

        self.emit_sections()?;
        self.emit_items()?;

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Kanban layout did not provide root bounds"))?;
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
                "x-merman-kanban".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "text_mode": "plain_host_text",
                    "markdown": "plain_and_br_only",
                    "icons": "unavailable",
                    "look": self.look,
                    "use_max_width": self.layout.use_max_width,
                    "ticket_links": "safe_href_only",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Kanban,
                body: SvgStructureBody::Kanban(KanbanSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    look: self.look,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                    dom_ids: self.dom_ids,
                    ticket_links: self.ticket_links,
                }),
            },
        })
    }

    fn emit_sections(&mut self) -> Result<()> {
        for (index, (section, prepared)) in self
            .layout
            .sections
            .iter()
            .zip(self.prepared_sections.iter())
            .enumerate()
        {
            self.session.checkpoint(OperationPhase::Emit)?;
            let semantic_id = format!("kanban.section.{index}");
            let (fill, border) = self.section_colors(section.index)?;
            self.semantic_classes.insert(
                semantic_id.clone(),
                format!("cluster section-{}", section.index),
            );
            self.dom_ids.insert(semantic_id.clone(), section.id.clone());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.add_path(
                format!("{semantic_id}.background"),
                rounded_rect_path(
                    section.center_x,
                    section.rect_y + section.rect_height / 2.0,
                    section.width,
                    section.rect_height,
                    section.rx,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: Some(stroke(border, 1.0)),
                },
            )?;

            let max_width = section.width.max(1.0);
            let plan = self.prepared_label_plan(prepared, max_width)?;
            if !plan.is_empty() {
                self.text_classes
                    .insert(semantic_id.clone(), "nodeLabel".to_string());
                let left = section.center_x - section.width / 2.0;
                let label_left = left + (section.width - plan.width).max(0.0) / 2.0;
                let label_height = section
                    .label_height
                    .max(plan.height)
                    .max(self.font_size * LABEL_LINE_HEIGHT_SCALE);
                self.emit_label_plan(
                    &format!("{semantic_id}.label"),
                    &plan,
                    Point::new(label_left, section.rect_y),
                    TextAnchor::Middle,
                    label_height,
                )?;
            }

            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(plan_title(prepared)?),
                description: Some("Kanban section".to_string()),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_items(&mut self) -> Result<()> {
        // Mermaid permits the same user-facing node id in different sections (the upstream
        // renderer keeps both DOM nodes).  Index sources by `(id, parent)` and consume each
        // occurrence in layout order instead of using a first-match lookup that aliases the
        // second occurrence to the first node.
        let mut sources_by_key: BTreeMap<(String, String), Vec<&KanbanRenderNode>> =
            BTreeMap::new();
        for source in self.model.nodes.iter().filter(|node| !node.is_group) {
            if let Some(parent_id) = source.parent_id.as_deref() {
                sources_by_key
                    .entry((source.id.clone(), parent_id.to_owned()))
                    .or_default()
                    .push(source);
            }
        }
        let mut source_offsets: BTreeMap<(String, String), usize> = BTreeMap::new();

        for (index, (item, prepared)) in self
            .layout
            .items
            .iter()
            .zip(self.prepared_items.iter())
            .enumerate()
        {
            self.session.checkpoint(OperationPhase::Emit)?;
            let key = (item.id.clone(), item.parent_id.clone());
            let offset = source_offsets.entry(key.clone()).or_default();
            let _source = sources_by_key
                .get(&key)
                .and_then(|sources| sources.get(*offset))
                .ok_or_else(|| {
                    invalid(format!(
                        "Kanban layout item `{}` is not paired with its semantic parent `{}`",
                        item.id, item.parent_id
                    ))
                })?;
            *offset = offset.saturating_add(1);

            let semantic_id = format!("kanban.item.{index}");
            self.semantic_classes
                .insert(semantic_id.clone(), "node".to_string());
            self.dom_ids.insert(semantic_id.clone(), item.id.clone());
            self.path_classes.insert(
                format!("{semantic_id}.background"),
                "basic label-container __APA__".to_string(),
            );
            self.text_classes
                .insert(semantic_id.clone(), "nodeLabel".to_string());
            if let Some(ticket_link) = prepared.ticket_link.as_ref() {
                self.ticket_links
                    .insert(semantic_id.clone(), ticket_link.uri.clone());
            }
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.add_path(
                format!("{semantic_id}.background"),
                rounded_rect_path(
                    item.center_x,
                    item.center_y,
                    item.width,
                    item.height,
                    item.rx,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.background)),
                    stroke: Some(stroke(self.node_border, 1.0)),
                },
            )?;

            let max_width = (item.width - crate::kanban::KANBAN_SECTION_PADDING_PX).max(1.0);
            let title_plan = self.prepared_label_plan(&prepared.title, max_width)?;
            let ticket_metrics = self.measure_plain_text(item.ticket.as_deref());
            let assigned_metrics = self.measure_plain_text(item.assigned.as_deref());
            let detail_height = ticket_metrics
                .into_iter()
                .chain(assigned_metrics)
                .map(|metrics| metrics.height)
                .fold(0.0, f64::max);
            let title_height = if title_plan.is_empty() {
                0.0
            } else {
                title_plan
                    .height
                    .max(self.line_height() * title_plan.lines.len() as f64)
            };
            let height_adjustment = detail_height / 2.0;
            let title_top = item.center_y - height_adjustment - title_height / 2.0;
            let detail_top = item.center_y - height_adjustment + title_height / 2.0;
            let left = item.center_x - item.width / 2.0;
            let left_x = left + crate::kanban::KANBAN_SECTION_PADDING_PX;

            if !title_plan.is_empty() {
                self.emit_label_plan(
                    &format!("{semantic_id}.title"),
                    &title_plan,
                    Point::new(left_x, title_top),
                    TextAnchor::Start,
                    title_height,
                )?;
            }

            if let Some(metrics) = ticket_metrics {
                let ticket_x = left_x;
                self.emit_plain_text(
                    &format!("{semantic_id}.ticket"),
                    item.ticket.as_deref().unwrap_or_default(),
                    Point::new(ticket_x, detail_top),
                    metrics,
                    TextAnchor::Start,
                )?;
            }

            if let Some(metrics) = assigned_metrics {
                let right = item.center_x + item.width / 2.0;
                let assigned_x =
                    right - crate::kanban::KANBAN_SECTION_PADDING_PX - metrics.width.max(0.0);
                self.emit_plain_text(
                    &format!("{semantic_id}.assigned"),
                    item.assigned.as_deref().unwrap_or_default(),
                    Point::new(assigned_x, detail_top),
                    metrics,
                    TextAnchor::Start,
                )?;
            }

            if let Some(priority) = item.priority.as_deref().filter(|value| !value.is_empty()) {
                self.path_classes
                    .insert(format!("{semantic_id}.priority"), "priority".to_string());
                self.emit_priority_line(&semantic_id, item, priority)?;
            }

            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id.clone(),
                role: SemanticRole::Node,
                title: Some(plan_title(&prepared.title)?),
                description: Some(format!("Kanban item in {}", item.parent_id)),
                link: prepared
                    .ticket_link
                    .as_ref()
                    .and_then(|link| link.uri.clone()),
            });
            if item
                .ticket
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                self.semantics.push(SemanticAnnotation {
                    id: format!("{semantic_id}.ticket"),
                    role: SemanticRole::Label,
                    title: item.ticket.clone(),
                    description: Some("Kanban ticket".to_string()),
                    link: prepared
                        .ticket_link
                        .as_ref()
                        .and_then(|link| link.uri.clone()),
                });
            }
            if item
                .assigned
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                self.semantics.push(SemanticAnnotation {
                    id: format!("{semantic_id}.assigned"),
                    role: SemanticRole::Label,
                    title: item.assigned.clone(),
                    description: Some("Kanban assignee".to_string()),
                    link: None,
                });
            }
            if item
                .priority
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                self.semantics.push(SemanticAnnotation {
                    id: format!("{semantic_id}.priority"),
                    role: SemanticRole::Label,
                    title: item.priority.clone(),
                    description: Some("Kanban priority".to_string()),
                    link: None,
                });
            }
        }
        Ok(())
    }

    fn emit_priority_line(
        &mut self,
        semantic_id: &str,
        item: &KanbanItemLayout,
        priority: &str,
    ) -> Result<()> {
        let left = item.center_x - item.width / 2.0;
        let top = item.center_y - item.height / 2.0;
        let y1 = top + (item.rx / 2.0).floor();
        let y2 = top + item.height - (item.rx / 2.0).floor();
        let color = priority_color(priority)?;
        self.add_path(
            format!("{semantic_id}.priority"),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(left + 2.0, y1),
                },
                PathSegment::LineTo {
                    to: Point::new(left + 2.0, y2),
                },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(color, 4.0)),
            },
        )
    }

    fn section_colors(&self, index: i64) -> Result<(Color, Color)> {
        let Some(slot) = index
            .try_into()
            .ok()
            .and_then(|index: usize| index.checked_add(1))
        else {
            return Err(invalid(
                "Kanban section index is outside the portable range",
            ));
        };
        let styles = PortableStyleResolver::new("kanban");
        let Some(section) = self.theme.sections.get(slot) else {
            // Mermaid only emits section selectors for section -1 through section 10.  Later
            // columns fall back to the generic node palette, exactly like the upstream CSS.
            return Ok((self.background, self.node_border));
        };
        Ok((
            styles.color("section.fill", &section.section_fill)?,
            styles.color("section.stroke", &section.section_fill)?,
        ))
    }

    fn prepared_label_plan(
        &self,
        prepared: &KanbanPreparedMarkdownLabel,
        max_width: f64,
    ) -> Result<LabelPlan> {
        let lines = {
            let measurer = self
                .session
                .controlled_text_measurer(TextMeasurementPhase::Wrap, OperationPhase::Emit);
            prepared_label_lines(prepared, max_width, &self.label_style, &measurer)?
        };
        let lines = lines
            .into_iter()
            .map(|line| svg_plain_text(&line))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            return Ok(LabelPlan {
                lines,
                width: 0.0,
                height: 0.0,
            });
        }

        let line_width = {
            let measurer = self
                .session
                .controlled_text_measurer(TextMeasurementPhase::Wrap, OperationPhase::Emit);
            lines
                .iter()
                .map(|line| {
                    measurer
                        .measure_wrapped(line, &self.label_style, None, WrapMode::HtmlLike)
                        .width
                })
                .fold(0.0, f64::max)
        };
        let width = if prepared.geometry.foreign_object_width.is_finite()
            && prepared.geometry.foreign_object_width > 0.0
        {
            prepared.geometry.foreign_object_width
        } else {
            line_width
        }
        .min(max_width.max(1.0));
        let height = if prepared.geometry.content_height.is_finite()
            && prepared.geometry.content_height > 0.0
        {
            prepared.geometry.content_height
        } else {
            self.line_height() * lines.len() as f64
        };
        Ok(LabelPlan {
            lines,
            width: width.max(1.0),
            height: height.max(self.line_height()),
        })
    }

    fn emit_label_plan(
        &mut self,
        prefix: &str,
        plan: &LabelPlan,
        box_origin: Point,
        anchor: TextAnchor,
        box_height: f64,
    ) -> Result<()> {
        let line_height = self.line_height();
        let total_height = line_height * plan.lines.len() as f64;
        let top = box_origin.y + (box_height - total_height) / 2.0;
        let origin_x = match anchor {
            TextAnchor::Start => box_origin.x,
            TextAnchor::Middle => box_origin.x + plan.width / 2.0,
            TextAnchor::End => box_origin.x + plan.width,
        };
        let font = self.font.clone();
        for (index, line) in plan.lines.iter().enumerate() {
            self.emit_text(
                format!("{prefix}.{index}"),
                line,
                TextEmitSpec {
                    origin: Point::new(origin_x, top + line_height * (index as f64 + 0.5)),
                    font_size: self.font_size,
                    weight: self.font.weight,
                    color: self.text_color,
                    font: font.clone(),
                    anchor,
                    baseline: TextBaseline::Middle,
                },
            )?;
        }
        Ok(())
    }

    fn measure_plain_text(&self, value: Option<&str>) -> Option<PlainMetrics> {
        let value = value.map(svg_plain_text)?;
        if value.is_empty() {
            return None;
        }
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::Wrap, OperationPhase::Emit);
        let metrics = measurer.measure_wrapped(&value, &self.label_style, None, WrapMode::HtmlLike);
        Some(PlainMetrics {
            width: metrics.width.max(1.0),
            height: metrics.height.max(self.font_size),
        })
    }

    fn emit_plain_text(
        &mut self,
        id: &str,
        value: &str,
        origin: Point,
        metrics: PlainMetrics,
        anchor: TextAnchor,
    ) -> Result<()> {
        self.emit_text(
            id,
            value,
            TextEmitSpec {
                origin,
                font_size: self.font_size,
                weight: self.font.weight,
                color: self.text_color,
                font: self.font.clone(),
                anchor,
                baseline: TextBaseline::Middle,
            },
        )?;
        let _ = metrics;
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
        let value = svg_plain_text(value);
        if value.is_empty() {
            return Ok(());
        }
        let measurement_style = MeasurementTextStyle {
            font_family: Some(font.families.join(", ")),
            font_size,
            font_weight: Some(weight.to_string()),
            font_style: Some(match font.style {
                FontStyle::Normal => "normal".to_string(),
                FontStyle::Italic => "italic".to_string(),
                FontStyle::Oblique => "oblique".to_string(),
            }),
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer
            .measure_svg_raw_text_bbox_width_px(&value, &measurement_style)
            .max(1.0);
        let height = measurer
            .measure_svg_simple_text_bbox_height_px(&value, &measurement_style)
            .max(1.0);
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
        self.commands.push(DrawingCommand::draw_text(TextRun {
            text: value,
            origin,
            bounds: Rect::new(left, top, width, height),
            style: DisplayTextStyle {
                font: FontDescriptor { weight, ..font },
                font_size,
                letter_spacing: 0.0,
                line_height: self.line_height(),
                fill: Paint::solid(color),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor,
            baseline,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        }));
        Ok(())
    }

    fn line_height(&self) -> f64 {
        (self.font_size * LABEL_LINE_HEIGHT_SCALE).max(1.0)
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("Kanban path has no geometry"));
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

fn prepared_label_lines(
    prepared: &KanbanPreparedMarkdownLabel,
    max_width: f64,
    style: &MeasurementTextStyle,
    measurer: &dyn crate::text::TextMeasurer,
) -> Result<Vec<String>> {
    let lines = parse_prepared_label(&prepared.html)?;
    if !prepared.geometry.wrapped {
        return Ok(lines);
    }
    let mut wrapped = Vec::new();
    for line in lines {
        wrapped.extend(wrap_text_lines_measurer(
            &line,
            measurer,
            style,
            Some(max_width),
        ));
    }
    Ok(wrapped)
}

fn parse_prepared_label(fragment: &str) -> Result<Vec<String>> {
    let rooted = format!("<merman-fragment>{fragment}</merman-fragment>");
    let document = roxmltree::Document::parse(&rooted).map_err(|_| {
        unavailable("Kanban markdown label is not valid XHTML supported by DrawingList v1")
    })?;
    let root = document.root_element();
    let mut paragraph = None;
    for child in root.children() {
        if child.is_element() {
            if paragraph.is_some() {
                return Err(unavailable(
                    "Kanban markdown supports only one plain paragraph in DrawingList v1",
                ));
            }
            paragraph = Some(child);
        } else if child.is_text() && !child.text().unwrap_or_default().trim().is_empty() {
            return Err(unavailable(
                "Kanban markdown label has text outside its portable paragraph",
            ));
        } else if !child.is_text() {
            return Err(unavailable(
                "Kanban markdown label contains unsupported XHTML structure",
            ));
        }
    }
    let paragraph = paragraph
        .ok_or_else(|| unavailable("Kanban markdown label has no portable paragraph content"))?;
    if paragraph.tag_name().name() != "p" || paragraph.attributes().next().is_some() {
        return Err(unavailable(
            "Kanban markdown supports only plain text and <br> labels in DrawingList v1",
        ));
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for child in paragraph.children() {
        if child.is_text() {
            current.push_str(child.text().unwrap_or_default());
        } else if child.is_element()
            && child.tag_name().name() == "br"
            && child.attributes().next().is_none()
        {
            lines.push(svg_plain_text(&current));
            current.clear();
        } else {
            return Err(unavailable(
                "Kanban markdown supports only plain text and <br> labels in DrawingList v1",
            ));
        }
    }
    lines.push(svg_plain_text(&current));
    Ok(lines)
}

fn plan_title(prepared: &KanbanPreparedMarkdownLabel) -> Result<String> {
    Ok(parse_prepared_label(&prepared.html)?
        .into_iter()
        .map(|line| svg_plain_text(&line))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" "))
}

fn priority_color(priority: &str) -> Result<Color> {
    let value = match priority.trim() {
        "Very High" => "red",
        "High" => "orange",
        "Low" => "blue",
        "Very Low" => "lightblue",
        _ => return Ok(BLACK),
    };
    PortableStyleResolver::new("kanban").color("priority.stroke", value)
}

fn parse_font_weight(value: Option<&str>) -> u16 {
    value
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|weight| *weight > 0)
        .unwrap_or(400)
}

fn parse_font_style(value: Option<&str>) -> FontStyle {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("italic") => FontStyle::Italic,
        Some("oblique") => FontStyle::Oblique,
        _ => FontStyle::Normal,
    }
}

fn validate_model(model: &KanbanDiagramRenderModel) -> Result<()> {
    for node in &model.nodes {
        if node
            .icon
            .as_deref()
            .is_some_and(|icon| !icon.trim().is_empty())
        {
            return Err(unavailable(
                "Kanban icons require an explicit icon resource contract and cannot be dropped from DrawingList v1",
            ));
        }
    }
    Ok(())
}

fn validate_layout(
    layout: &KanbanDiagramLayout,
    prepared_sections: &[KanbanPreparedMarkdownLabel],
    prepared_items: &[KanbanPreparedItem],
) -> Result<()> {
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| invalid("Kanban layout did not provide root bounds"))?;
    validate_bounds(bounds)?;
    if [
        layout.section_width,
        layout.padding,
        layout.max_label_height,
        layout.viewbox_padding,
    ]
    .iter()
    .any(|value| !value.is_finite() || *value < 0.0)
        || layout.section_width <= 0.0
    {
        return Err(invalid("Kanban root metrics are invalid"));
    }
    if layout.sections.len() != prepared_sections.len()
        || layout.items.len() != prepared_items.len()
    {
        return Err(invalid(
            "Kanban layout and prepared markdown labels are out of sync",
        ));
    }
    for section in &layout.sections {
        validate_section(section)?;
    }
    for item in &layout.items {
        validate_item(item)?;
    }
    for prepared in prepared_sections
        .iter()
        .chain(prepared_items.iter().map(|item| &item.title))
    {
        if [
            prepared.geometry.content_height,
            prepared.geometry.foreign_object_width,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(invalid("Kanban prepared label geometry is invalid"));
        }
    }
    Ok(())
}

fn validate_section(section: &KanbanSectionLayout) -> Result<()> {
    if section.id.is_empty()
        || section.index <= 0
        || [section.center_x, section.center_y, section.rect_y]
            .iter()
            .any(|value| !value.is_finite())
        || [
            section.width,
            section.rect_height,
            section.rx,
            section.ry,
            section.label_width,
            section.label_height,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || section.width <= 0.0
        || section.rect_height <= 0.0
    {
        return Err(invalid("Kanban section geometry is invalid"));
    }
    Ok(())
}

fn validate_item(item: &KanbanItemLayout) -> Result<()> {
    if item.id.is_empty()
        || item.parent_id.is_empty()
        || [item.center_x, item.center_y]
            .iter()
            .any(|value| !value.is_finite())
        || [item.width, item.height, item.rx, item.ry]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        || item.width <= 0.0
        || item.height <= 0.0
    {
        return Err(invalid("Kanban item geometry is invalid"));
    }
    if item
        .icon
        .as_deref()
        .is_some_and(|icon| !icon.trim().is_empty())
    {
        return Err(unavailable(
            "Kanban icons require an explicit icon resource contract and cannot be dropped from DrawingList v1",
        ));
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
        return Err(invalid("Kanban bounds are invalid"));
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
        family: "kanban".to_string(),
        reason: message.into(),
    }
}
