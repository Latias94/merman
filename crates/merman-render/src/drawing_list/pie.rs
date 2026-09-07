//! Renderer-neutral Pie diagram adapter.
//!
//! Pie already has typed layout for slice angles, labels, legend placement, and root bounds. This
//! adapter resolves the remaining CSS theme roles and emits arcs directly. Static slice
//! highlighting is portable; hover highlighting remains an explicit capability error because the
//! DrawingList v1 contract intentionally has no host interaction state machine.

use super::{
    PieSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families_for,
};
use crate::config::config_string;
use crate::drawing_list::flowchart::{ellipse_path, polygon_path};
use crate::drawing_list::support::{PortableStyleResolver, stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, PieDiagramLayout};
use crate::pie::{
    PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_SPACING_PX, PIE_TITLE_Y, PieConfigView, PieLegendPosition,
    pie_content_offset, pie_legend_text,
};
use crate::render_geometry::pie_slice_segments;
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::theme::PresentationTheme;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::pie::PieDiagramRenderModel;
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type PiePair = FamilyPair<PieDiagramRenderModel, PieDiagramLayout>;

const HIGHLIGHT_SCALE: f64 = 1.05;
const LEGEND_TEXT_Y: f64 = 14.0;

pub(crate) fn build_pie_document(
    pair: &PiePair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    PieBuilder::new(pair, metadata, policy, session)?.build()
}

struct PieBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a PieDiagramRenderModel,
    layout: &'a PieDiagramLayout,
    legend_position: PieLegendPosition,
    inner_radius: f64,
    highlight_slice: Option<String>,
    font_family_css: String,
    font: FontDescriptor,
    text_obligation: TextObligation,
    styles: PortableStyleResolver,
    slice_stroke: Option<Color>,
    slice_stroke_width: f64,
    slice_opacity: f64,
    outer_stroke: Option<Color>,
    outer_stroke_width: f64,
    title_font_size: f64,
    title_color: Color,
    section_font_size: f64,
    section_color: Color,
    legend_font_size: f64,
    legend_color: Color,
    use_max_width: bool,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> PieBuilder<'a> {
    fn new(
        pair: &'a PiePair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let highlight_slice =
            config_string(config, &["pie", "highlightSlice"]).filter(|value| !value.is_empty());
        if highlight_slice.as_deref() == Some("hover") {
            return Err(unavailable(
                "pie.highlightSlice=hover requires a host interaction state machine that DrawingList v1 does not define",
            ));
        }

        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout, model)?;
        let settings = PieConfigView::new(config).render_settings();
        let theme = PresentationTheme::new(config).pie_drawing();
        let styles = PortableStyleResolver::new("pie");
        let font_family_css = crate::config::config_font_family_css_raw(config);
        let font = FontDescriptor {
            families: parse_font_families_for(&font_family_css, RenderFamilyKind::Pie)?,
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
            legend_position: settings.legend_position,
            inner_radius: settings.donut_hole * layout.radius,
            highlight_slice,
            font_family_css,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            styles,
            slice_stroke: styles.optional_color("pieStrokeColor", &theme.slice_stroke_color)?,
            slice_stroke_width: styles.length("pieStrokeWidth", &theme.slice_stroke_width)?,
            slice_opacity: styles.opacity("pieOpacity", &theme.slice_opacity)?,
            outer_stroke: styles
                .optional_color("pieOuterStrokeColor", &theme.outer_stroke_color)?,
            outer_stroke_width: styles.length("pieOuterStrokeWidth", &theme.outer_stroke_width)?,
            title_font_size: styles.positive_length("pieTitleTextSize", &theme.title_text_size)?,
            title_color: styles.color("pieTitleTextColor", &theme.title_text_color)?,
            section_font_size: styles
                .positive_length("pieSectionTextSize", &theme.section_text_size)?,
            section_color: styles.color("pieSectionTextColor", &theme.section_text_color)?,
            legend_font_size: styles
                .positive_length("pieLegendTextSize", &theme.legend_text_size)?,
            legend_color: styles.color("pieLegendTextColor", &theme.legend_text_color)?,
            use_max_width: settings.use_max_width,
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "pie.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let bounds = document_viewport_bounds(self.layout, self.model);
        // The SVG root background is a projection of this paint, not an independent default.
        self.add_path(
            "pie.background".to_string(),
            polygon_path(&[
                Point::new(bounds.x, bounds.y),
                Point::new(bounds.x + bounds.width, bounds.y),
                Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
                Point::new(bounds.x, bounds.y + bounds.height),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )?;
        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::ConcatTransform {
            transform: translate(self.layout.center_x, self.layout.center_y),
        });
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "pie.content".to_string(),
        });
        self.semantics.push(SemanticAnnotation {
            id: "pie.content".to_string(),
            role: SemanticRole::Group,
            title: None,
            description: None,
            link: None,
        });
        self.semantics.push(SemanticAnnotation {
            id: "pie.document".to_string(),
            role: SemanticRole::Document,
            title: self.model.acc_title.clone(),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_plot()?;
        self.emit_title()?;
        self.emit_legend()?;

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(bounds),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-pie".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "donut_hole": self.inner_radius / self.layout.radius,
                    "highlight_slice": self.highlight_slice,
                    "legend_position": legend_position_name(self.legend_position),
                    "show_data": self.model.show_data,
                    "text_mode": "plain_host_text",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Pie,
                body: SvgStructureBody::Pie(PieSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.use_max_width,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                }),
            },
        })
    }

    fn emit_plot(&mut self) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)?;
        self.semantics.push(SemanticAnnotation {
            id: "pie.plot".to_string(),
            role: SemanticRole::Group,
            title: Some("Pie chart".to_string()),
            description: None,
            link: None,
        });
        self.commands.push(DrawingCommand::Save);
        let (offset_x, offset_y) = pie_content_offset(self.layout, self.legend_position);
        if offset_x != 0.0 || offset_y != 0.0 {
            self.commands.push(DrawingCommand::ConcatTransform {
                transform: translate(offset_x, offset_y),
            });
        }
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "pie.plot".to_string(),
        });

        self.path_classes
            .insert("pie.outer".to_string(), "pieOuterCircle".to_string());
        self.add_path(
            "pie.outer".to_string(),
            ellipse_path(0.0, 0.0, self.layout.outer_radius, self.layout.outer_radius),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: self
                    .outer_stroke
                    .map(|color| stroke(color, self.outer_stroke_width)),
            },
        )?;

        for (index, slice) in self.layout.slices.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let semantic_id = format!("pie.slice.{index}");
            let fill = self.styles.optional_color("pie slice fill", &slice.fill)?;
            let stroke = self
                .slice_stroke
                .map(|color| stroke(color, self.slice_stroke_width));
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            {
                let highlighted = self.highlight_slice.as_deref() == Some(slice.label.as_str());
                let class = if highlighted {
                    "pieCircle highlighted"
                } else {
                    "pieCircle"
                };
                self.path_classes
                    .insert(format!("{semantic_id}.shape"), class.to_string());
                self.commands.push(DrawingCommand::Save);
                if highlighted {
                    self.commands.push(DrawingCommand::ConcatTransform {
                        transform: scale(HIGHLIGHT_SCALE),
                    });
                }
                self.commands.push(DrawingCommand::SetOpacity {
                    opacity: if highlighted { 1.0 } else { self.slice_opacity },
                });
                let segments = pie_slice_segments(
                    self.layout.radius,
                    self.inner_radius,
                    slice.start_angle,
                    slice.end_angle,
                    slice.is_full_circle,
                )
                .ok_or_else(|| {
                    invalid(format!(
                        "Pie slice `{}` has invalid arc geometry",
                        slice.label
                    ))
                })?;
                self.add_path(
                    format!("{semantic_id}.shape"),
                    segments,
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: fill.map(Paint::solid),
                        stroke,
                    },
                )?;
                self.commands.push(DrawingCommand::Restore);
            }
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(slice.label.clone()),
                description: Some(format!("{}% ({})", slice.percent, slice.value)),
                link: None,
            });
        }

        for (index, slice) in self.layout.slices.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let text = format!("{}%", slice.percent);
            let semantic_id = format!("pie.slice.{index}.percentage");
            self.text_classes
                .insert(semantic_id.clone(), "slice".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.commands.push(DrawingCommand::draw_text(self.text_run(
                text.clone(),
                Point::new(slice.text_x, slice.text_y),
                self.section_font_size,
                self.section_color,
                TextAnchor::Middle,
            )?));
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Label,
                title: Some(text),
                description: Some(format!("Percentage for {}", slice.label)),
                link: None,
            });
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        Ok(())
    }

    fn emit_title(&mut self) -> Result<()> {
        let raw_title = self.layout.title.as_deref().unwrap_or("");
        // Mermaid keeps the title text node's boundary whitespace. Do not run it through the
        // label-only whitespace normalizer used for legend content.
        let title = raw_title.to_string();
        self.session.checkpoint(OperationPhase::Emit)?;
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "pie.title".to_string(),
        });
        self.text_classes
            .insert("pie.title".to_string(), "pieTitleText".to_string());
        self.commands.push(DrawingCommand::draw_text(self.text_run(
            title.clone(),
            Point::new(0.0, PIE_TITLE_Y),
            self.title_font_size,
            self.title_color,
            TextAnchor::Middle,
        )?));
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: "pie.title".to_string(),
            role: SemanticRole::Label,
            title: Some(raw_title.to_string()),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_legend(&mut self) -> Result<()> {
        for (index, item) in self.layout.legend_items.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let semantic_id = format!("pie.legend.{index}");
            self.semantic_classes
                .insert(semantic_id.clone(), "legend".to_string());
            let text = pie_legend_text(&item.label, item.value, self.model.show_data);
            let color = self.styles.optional_color("pie legend fill", &item.fill)?;
            self.commands.push(DrawingCommand::Save);
            self.commands.push(DrawingCommand::ConcatTransform {
                transform: translate(self.layout.legend_x, item.y),
            });
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            {
                self.add_path(
                    format!("{semantic_id}.swatch"),
                    polygon_path(&[
                        Point::new(0.0, 0.0),
                        Point::new(PIE_LEGEND_RECT_SIZE_PX, 0.0),
                        Point::new(PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_RECT_SIZE_PX),
                        Point::new(0.0, PIE_LEGEND_RECT_SIZE_PX),
                    ]),
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: color.map(Paint::solid),
                        stroke: color.map(|color| stroke(color, 1.0)),
                    },
                )?;
            }
            {
                self.commands.push(DrawingCommand::draw_text(self.text_run(
                    text.clone(),
                    Point::new(
                        PIE_LEGEND_RECT_SIZE_PX + PIE_LEGEND_SPACING_PX,
                        LEGEND_TEXT_Y,
                    ),
                    self.legend_font_size,
                    self.legend_color,
                    TextAnchor::Start,
                )?));
            }
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.commands.push(DrawingCommand::Restore);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(item.label.clone()),
                description: Some(text),
                link: None,
            });
        }
        Ok(())
    }

    fn text_run(
        &self,
        text: String,
        origin: Point,
        font_size: f64,
        color: Color,
        anchor: TextAnchor,
    ) -> Result<TextRun> {
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.font_family_css.clone()),
            font_size,
            font_weight: None,
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width =
            measurer.measure_svg_text_bounding_client_rect_width_px(&text, &measurement_style);
        let height = measurer.measure_svg_simple_text_bbox_height_px(&text, &measurement_style);
        self.session.checkpoint(OperationPhase::Emit)?;
        if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
            return Err(invalid("Pie text measurement returned invalid bounds"));
        }
        let x = match anchor {
            TextAnchor::Start => origin.x,
            TextAnchor::Middle => origin.x - width / 2.0,
            TextAnchor::End => origin.x - width,
        };
        Ok(TextRun {
            text,
            origin,
            bounds: Rect::new(x, origin.y - height, width, height),
            style: TextStyle {
                font: self.font.clone(),
                font_size,
                letter_spacing: 0.0,
                line_height: font_size,
                fill: Paint::solid(color),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor,
            baseline: TextBaseline::Alphabetic,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        })
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!("Pie path `{id}` has no geometry")));
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
}

fn validate_layout(layout: &PieDiagramLayout, model: &PieDiagramRenderModel) -> Result<()> {
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| invalid("Pie layout did not provide root bounds"))?;
    if !model.sections.is_empty() || bounds_is_finite(bounds) {
        validate_bounds(bounds)?;
    }
    if ![
        layout.center_x,
        layout.center_y,
        layout.radius,
        layout.outer_radius,
        layout.legend_x,
        layout.legend_start_y,
        layout.legend_step_y,
    ]
    .into_iter()
    .all(f64::is_finite)
        || layout.radius <= 0.0
        || layout.outer_radius <= 0.0
        || layout.legend_step_y <= 0.0
    {
        return Err(invalid("Pie layout has invalid chart geometry"));
    }
    if layout.legend_items.len() != model.sections.len() {
        return Err(invalid(format!(
            "Pie layout has {} legend items for {} sections",
            layout.legend_items.len(),
            model.sections.len()
        )));
    }
    if layout.slices.iter().any(|slice| {
        ![
            slice.value,
            slice.start_angle,
            slice.end_angle,
            slice.text_x,
            slice.text_y,
        ]
        .into_iter()
        .all(f64::is_finite)
    }) || layout
        .legend_items
        .iter()
        .any(|item| !item.value.is_finite() || !item.y.is_finite())
    {
        return Err(invalid(
            "Pie layout contains non-finite slice or legend data",
        ));
    }
    Ok(())
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("Pie root bounds are invalid"));
    }
    Ok(())
}

fn bounds_is_finite(bounds: &Bounds) -> bool {
    [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        && bounds.max_x >= bounds.min_x
        && bounds.max_y >= bounds.min_y
}

fn document_viewport_bounds(layout: &PieDiagramLayout, model: &PieDiagramRenderModel) -> Rect {
    let Some(bounds) = layout.bounds.as_ref() else {
        return Rect::new(0.0, 0.0, 450.0, 450.0);
    };
    if bounds_is_finite(bounds) {
        Rect::new(
            bounds.min_x,
            bounds.min_y,
            bounds.max_x - bounds.min_x,
            bounds.max_y - bounds.min_y,
        )
    } else if model.sections.is_empty() {
        // Mermaid's empty right-legend layout can expose -Infinity as the computed width. The
        // SVG renderer repairs that root to its finite empty-chart viewport; keep the same
        // bounded result in the canonical document instead of leaking non-finite JSON.
        Rect::new(0.0, 0.0, 225.0, 450.0)
    } else {
        // Construction has already rejected non-finite bounds for non-empty charts.
        Rect::new(0.0, 0.0, 1.0, 1.0)
    }
}

fn translate(x: f64, y: f64) -> Transform {
    Transform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: x,
        f: y,
    }
}

fn scale(value: f64) -> Transform {
    Transform {
        a: value,
        b: 0.0,
        c: 0.0,
        d: value,
        e: 0.0,
        f: 0.0,
    }
}

fn legend_position_name(position: PieLegendPosition) -> &'static str {
    match position {
        PieLegendPosition::Top => "top",
        PieLegendPosition::Bottom => "bottom",
        PieLegendPosition::Left => "left",
        PieLegendPosition::Right => "right",
        PieLegendPosition::Center => "center",
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "pie".to_string(),
        reason: message.into(),
    }
}
