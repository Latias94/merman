//! Renderer-neutral XYChart adapter.
//!
//! XYChart layout owns the final paint sequence: background, bars, line paths, axes, ticks,
//! legends, ordinary text transforms, and materialized bar data labels. This adapter projects that
//! visual plan without rebuilding chart geometry or relying on the SVG DOM group tree.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, XyChartSvgBody, parse_font_families,
    parse_svg_path,
};
use crate::config::config_font_family_css;
use crate::drawing_list::flowchart::polygon_path;
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{
    XyChartDiagramLayout, XyChartDrawableElem, XyChartPathData, XyChartRectData, XyChartTextData,
};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::xychart::{
    XyChartTextAnchor, XyChartTextBaseline, xychart_text_anchor, xychart_text_baseline,
};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::xychart::XyChartDiagramRenderModel;
use merman_display_list::{
    CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument, DrawingListPolicy,
    DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource, PathSegment,
    PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor, TextBaseline,
    TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

type XyChartPair = FamilyPair<XyChartDiagramRenderModel, XyChartDiagramLayout>;

pub(crate) fn build_xychart_document(
    pair: &XyChartPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    XyChartBuilder::new(pair, metadata, policy, session)?.build()
}

struct XyChartBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a XyChartDiagramRenderModel,
    layout: &'a XyChartDiagramLayout,
    font_family_css: String,
    font: FontDescriptor,
    text_obligation: TextObligation,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> XyChartBuilder<'a> {
    fn new(
        pair: &'a XyChartPair,
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
                "themeCSS is an unresolved SVG cascade input for XYChart DrawingList output",
            ));
        }

        let layout = pair.layout();
        validate_layout(layout)?;
        let font_family_css = config_font_family_css(config);
        Ok(Self {
            metadata,
            session,
            policy,
            model: pair.semantic(),
            layout,
            font: FontDescriptor {
                families: parse_font_families(font_family_css.clone()),
                weight: 400,
                style: FontStyle::Normal,
                postscript_name: None,
                resource: None,
            },
            font_family_css,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "xychart.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.semantics.push(SemanticAnnotation {
            id: "xychart.document".to_string(),
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
        for drawable_index in 0..self.layout.drawables.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let drawable = self.layout.drawables[drawable_index].clone();
            self.emit_drawable(drawable_index, &drawable)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
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
                "x-merman-xychart".to_string(),
                json!({
                    "chart_orientation": self.layout.chart_orientation,
                    "diagram_type": self.metadata.diagram_type,
                    "show_data_label": self.layout.show_data_label,
                    "show_data_label_outside_bar": self.layout.show_data_label_outside_bar,
                    "text_mode": "plain_host_text",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::XyChart,
                body: SvgStructureBody::XyChart(XyChartSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        if self.layout.width == 0.0 || self.layout.height == 0.0 {
            return Ok(());
        }
        let Some(fill) = PortableStyleResolver::new("xychart")
            .optional_color("background fill", &self.layout.background_color)?
        else {
            return Ok(());
        };
        self.add_path(
            "xychart.background".to_string(),
            polygon_path(&[
                Point::new(0.0, 0.0),
                Point::new(self.layout.width, 0.0),
                Point::new(self.layout.width, self.layout.height),
                Point::new(0.0, self.layout.height),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(fill)),
                stroke: None,
            },
        )
    }

    fn emit_drawable(
        &mut self,
        drawable_index: usize,
        drawable: &XyChartDrawableElem,
    ) -> Result<()> {
        let (group_texts, item_count) = drawable_identity(drawable);
        if item_count == 0 {
            return Ok(());
        }

        let semantic_id = format!("xychart.drawable.{drawable_index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        match drawable {
            XyChartDrawableElem::Rect { data, .. } => {
                for (item_index, rect) in data.iter().enumerate() {
                    self.emit_rect(drawable_index, item_index, group_texts, rect)?;
                }
            }
            XyChartDrawableElem::Text { data, .. } => {
                for (item_index, text) in data.iter().enumerate() {
                    self.emit_text(
                        drawable_index,
                        item_index,
                        group_texts,
                        text,
                        TextPlacement::Transformed,
                    )?;
                }
            }
            XyChartDrawableElem::BarDataLabel { data, .. } => {
                for (item_index, text) in data.iter().enumerate() {
                    self.emit_text(
                        drawable_index,
                        item_index,
                        group_texts,
                        text,
                        TextPlacement::Direct,
                    )?;
                }
            }
            XyChartDrawableElem::Path { data, .. } => {
                for (item_index, path) in data.iter().enumerate() {
                    self.emit_path(drawable_index, item_index, group_texts, path)?;
                }
            }
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: self.drawable_title(group_texts),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_rect(
        &mut self,
        drawable_index: usize,
        item_index: usize,
        group_texts: &[String],
        rect: &XyChartRectData,
    ) -> Result<()> {
        let semantic_id = format!("xychart.rect.{drawable_index}.{item_index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        let styles = PortableStyleResolver::new("xychart");
        let fill = styles.optional_color("rectangle fill", &rect.fill)?;
        let stroke = if rect.stroke_width == 0.0 {
            None
        } else {
            styles
                .optional_color("rectangle stroke", &rect.stroke_fill)?
                .map(|color| stroke(color, rect.stroke_width))
        };
        if rect.width > 0.0 && rect.height > 0.0 && (fill.is_some() || stroke.is_some()) {
            self.add_path(
                format!("{semantic_id}.shape"),
                polygon_path(&[
                    Point::new(rect.x, rect.y),
                    Point::new(rect.x + rect.width, rect.y),
                    Point::new(rect.x + rect.width, rect.y + rect.height),
                    Point::new(rect.x, rect.y + rect.height),
                ]),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: fill.map(Paint::solid),
                    stroke,
                },
            )?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        let (role, title, description) = self.rect_semantics(group_texts, item_index);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role,
            title,
            description,
            link: None,
        });
        Ok(())
    }

    fn emit_path(
        &mut self,
        drawable_index: usize,
        item_index: usize,
        group_texts: &[String],
        path: &XyChartPathData,
    ) -> Result<()> {
        let semantic_id = format!("xychart.path.{drawable_index}.{item_index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        let styles = PortableStyleResolver::new("xychart");
        let fill = path
            .fill
            .as_deref()
            .map(|fill| styles.optional_color("path fill", fill))
            .transpose()?
            .flatten();
        let stroke = if path.stroke_width == 0.0 {
            None
        } else {
            styles
                .optional_color("path stroke", &path.stroke_fill)?
                .map(|color| stroke(color, path.stroke_width))
        };
        if fill.is_some() || stroke.is_some() {
            self.add_path(
                format!("{semantic_id}.shape"),
                parse_svg_path(&path.path)?,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: fill.map(Paint::solid),
                    stroke,
                },
            )?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: self.drawable_title(group_texts),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_text(
        &mut self,
        drawable_index: usize,
        item_index: usize,
        group_texts: &[String],
        text: &XyChartTextData,
        placement: TextPlacement,
    ) -> Result<()> {
        let semantic_id = match placement {
            TextPlacement::Transformed => {
                format!("xychart.text.{drawable_index}.{item_index}")
            }
            TextPlacement::Direct => {
                format!("xychart.bar-label.{drawable_index}.{item_index}")
            }
        };
        let text_value = svg_plain_text(&text.text);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.clone(),
            role: SemanticRole::Label,
            title: (!text_value.is_empty()).then_some(text_value.clone()),
            description: group_title(group_texts),
            link: None,
        });
        // SVG font-size=0 produces no pixels, while DrawingList text runs require a positive size.
        // Keep the semantic value above and omit only the visually empty command.
        if text_value.is_empty() || text.font_size == 0.0 {
            return Ok(());
        }
        let Some(fill) =
            PortableStyleResolver::new("xychart").optional_color("text fill", &text.fill)?
        else {
            return Ok(());
        };

        let anchor = text_anchor(text);
        let baseline = text_baseline(text);
        let local_bounds =
            self.measure_text_bounds(&text_value, text.font_size, anchor, baseline)?;
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        match placement {
            TextPlacement::Transformed => {
                self.commands.push(DrawingCommand::Save);
                self.commands.push(DrawingCommand::ConcatTransform {
                    transform: text_transform(text.x, text.y, text.rotation),
                });
                self.commands.push(DrawingCommand::DrawText {
                    run: self.text_run(
                        text_value,
                        Point::new(0.0, 0.0),
                        local_bounds,
                        text.font_size,
                        fill,
                        anchor,
                        baseline,
                    ),
                });
                self.commands.push(DrawingCommand::Restore);
            }
            TextPlacement::Direct => {
                self.commands.push(DrawingCommand::DrawText {
                    run: self.text_run(
                        text_value,
                        Point::new(text.x, text.y),
                        translate_rect(local_bounds, text.x, text.y),
                        text.font_size,
                        fill,
                        anchor,
                        baseline,
                    ),
                });
            }
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        Ok(())
    }

    fn text_run(
        &self,
        text: String,
        origin: Point,
        bounds: Rect,
        font_size: f64,
        fill: merman_display_list::Color,
        anchor: TextAnchor,
        baseline: TextBaseline,
    ) -> TextRun {
        TextRun {
            text,
            origin,
            bounds,
            style: TextStyle {
                font: self.font.clone(),
                font_size,
                letter_spacing: 0.0,
                line_height: font_size,
                fill: Paint::solid(fill),
            },
            anchor,
            baseline,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        }
    }

    fn measure_text_bounds(
        &self,
        text: &str,
        font_size: f64,
        anchor: TextAnchor,
        baseline: TextBaseline,
    ) -> Result<Rect> {
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.font_family_css.clone()),
            font_size,
            font_weight: None,
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer.measure_svg_simple_text_bbox_width_px(text, &measurement_style);
        let height = measurer.measure_svg_simple_text_bbox_height_px(text, &measurement_style);
        self.session.checkpoint(OperationPhase::Emit)?;
        if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
            return Err(invalid("XYChart text measurement returned invalid bounds"));
        }
        let x = match anchor {
            TextAnchor::Start => 0.0,
            TextAnchor::Middle => -width / 2.0,
            TextAnchor::End => -width,
        };
        let y = match baseline {
            TextBaseline::Hanging | TextBaseline::TextBeforeEdge => 0.0,
            TextBaseline::Middle => -height / 2.0,
            TextBaseline::Alphabetic | TextBaseline::Ideographic | TextBaseline::TextAfterEdge => {
                -height
            }
        };
        Ok(Rect::new(x, y, width, height))
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!("XYChart path `{id}` has no geometry")));
        }
        let id = ResourceId::new(id);
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        Ok(())
    }

    fn drawable_title(&self, group_texts: &[String]) -> Option<String> {
        plot_index(group_texts)
            .and_then(|index| self.model.plots.get(index))
            .and_then(|plot| plot.title.clone())
            .filter(|title| !title.is_empty())
            .or_else(|| group_title(group_texts))
    }

    fn rect_semantics(
        &self,
        group_texts: &[String],
        item_index: usize,
    ) -> (SemanticRole, Option<String>, Option<String>) {
        let Some(plot_index) = bar_plot_index(group_texts) else {
            return (SemanticRole::Group, group_title(group_texts), None);
        };
        let Some(plot) = self.model.plots.get(plot_index) else {
            return (
                SemanticRole::Node,
                group_title(group_texts),
                Some(format!("Bar plot {plot_index}")),
            );
        };
        let title = plot.data.get(item_index).map(|(category, value)| {
            value.map_or_else(|| category.clone(), |value| format!("{category}: {value}"))
        });
        (
            SemanticRole::Node,
            title,
            plot.title.clone().filter(|title| !title.is_empty()),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextPlacement {
    Transformed,
    Direct,
}

fn drawable_identity(drawable: &XyChartDrawableElem) -> (&[String], usize) {
    match drawable {
        XyChartDrawableElem::Rect { group_texts, data } => (group_texts, data.len()),
        XyChartDrawableElem::Text { group_texts, data }
        | XyChartDrawableElem::BarDataLabel { group_texts, data } => (group_texts, data.len()),
        XyChartDrawableElem::Path { group_texts, data } => (group_texts, data.len()),
    }
}

fn plot_index(group_texts: &[String]) -> Option<usize> {
    if group_texts.first().map(String::as_str) != Some("plot") {
        return None;
    }
    let group = group_texts.get(1)?;
    group
        .strip_prefix("bar-plot-")
        .or_else(|| group.strip_prefix("line-plot-"))?
        .parse()
        .ok()
}

fn bar_plot_index(group_texts: &[String]) -> Option<usize> {
    if group_texts.first().map(String::as_str) != Some("plot") {
        return None;
    }
    group_texts.get(1)?.strip_prefix("bar-plot-")?.parse().ok()
}

fn group_title(group_texts: &[String]) -> Option<String> {
    (!group_texts.is_empty()).then(|| group_texts.join(" / "))
}

fn text_anchor(text: &XyChartTextData) -> TextAnchor {
    match xychart_text_anchor(&text.horizontal_pos) {
        XyChartTextAnchor::Start => TextAnchor::Start,
        XyChartTextAnchor::Middle => TextAnchor::Middle,
        XyChartTextAnchor::End => TextAnchor::End,
    }
}

fn text_baseline(text: &XyChartTextData) -> TextBaseline {
    match xychart_text_baseline(&text.vertical_pos) {
        XyChartTextBaseline::Alphabetic => TextBaseline::Alphabetic,
        XyChartTextBaseline::Hanging => TextBaseline::Hanging,
        XyChartTextBaseline::Middle => TextBaseline::Middle,
        XyChartTextBaseline::TextBeforeEdge => TextBaseline::TextBeforeEdge,
    }
}

fn text_transform(x: f64, y: f64, rotation_degrees: f64) -> Transform {
    let (sin, cos) = rotation_degrees.to_radians().sin_cos();
    Transform {
        a: cos,
        b: sin,
        c: -sin,
        d: cos,
        e: x,
        f: y,
    }
}

fn translate_rect(rect: Rect, x: f64, y: f64) -> Rect {
    Rect::new(rect.x + x, rect.y + y, rect.width, rect.height)
}

fn validate_layout(layout: &XyChartDiagramLayout) -> Result<()> {
    if ![layout.width, layout.height]
        .into_iter()
        .all(f64::is_finite)
        || layout.width < 0.0
        || layout.height < 0.0
    {
        return Err(invalid("XYChart root geometry is invalid"));
    }

    for drawable in &layout.drawables {
        match drawable {
            XyChartDrawableElem::Rect { data, .. } => {
                for rect in data {
                    if ![rect.x, rect.y, rect.width, rect.height, rect.stroke_width]
                        .into_iter()
                        .all(f64::is_finite)
                        || rect.width < 0.0
                        || rect.height < 0.0
                        || rect.stroke_width < 0.0
                    {
                        return Err(invalid("XYChart contains invalid rectangle geometry"));
                    }
                }
            }
            XyChartDrawableElem::Text { data, .. } => {
                for text in data {
                    validate_text(text, false)?;
                }
            }
            XyChartDrawableElem::BarDataLabel { data, .. } => {
                for text in data {
                    validate_text(text, true)?;
                }
            }
            XyChartDrawableElem::Path { data, .. } => {
                if data
                    .iter()
                    .any(|path| !path.stroke_width.is_finite() || path.stroke_width < 0.0)
                {
                    return Err(invalid("XYChart contains invalid path stroke geometry"));
                }
            }
        }
    }
    Ok(())
}

fn validate_text(text: &XyChartTextData, data_label: bool) -> Result<()> {
    if ![text.x, text.y, text.font_size, text.rotation]
        .into_iter()
        .all(f64::is_finite)
        || text.font_size < 0.0
        || (data_label && text.rotation != 0.0)
    {
        return Err(invalid("XYChart contains invalid text geometry"));
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
        family: "xychart".to_string(),
        reason: message.into(),
    }
}
