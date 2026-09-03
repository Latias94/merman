//! Renderer-neutral Pie diagram adapter.
//!
//! Pie already has typed layout for slice angles, labels, legend placement, and root bounds. This
//! adapter resolves the remaining CSS theme roles and emits arcs directly. Static slice
//! highlighting is portable; hover highlighting remains an explicit capability error because the
//! DrawingList v1 contract intentionally has no host interaction state machine.

use super::{
    PieSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
};
use crate::config::config_string;
use crate::drawing_list::flowchart::{ellipse_path, polygon_path};
use crate::environment::{RenderSession, TextMeasurementPhase, TextMeasurementSource};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, PieDiagramLayout};
use crate::pie::{
    PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_SPACING_PX, PIE_TITLE_Y, PieConfigView, PieLegendPosition,
    pie_content_offset, pie_css_px, pie_legend_text,
};
use crate::render_geometry::pie_slice_segments;
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::theme::PresentationTheme;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::pie::PieDiagramRenderModel;
use merman_core::theme_color::{ColorChannel, ThemeColor};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, MeasurementProvenance,
    Paint, PathResource, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, StrokeStyle, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun,
    TextStyle, Transform, Viewport,
};
use serde_json::{Value, json};
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
        if config
            .get("themeCSS")
            .and_then(Value::as_str)
            .is_some_and(|css| !css.trim().is_empty())
        {
            return Err(unavailable(
                "themeCSS is an unresolved SVG cascade input for Pie DrawingList output",
            ));
        }

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
        let font_family_css = theme.font_family_css.clone();
        let font = FontDescriptor {
            families: parse_font_families(theme.font_family_css),
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
            text_obligation: text_obligation(session),
            slice_stroke: parse_optional_color(&theme.slice_stroke_color, "pieStrokeColor")?,
            slice_stroke_width: required_css_px(
                &theme.slice_stroke_width,
                "pieStrokeWidth",
                false,
            )?,
            slice_opacity: parse_opacity(&theme.slice_opacity, "pieOpacity")?,
            outer_stroke: parse_optional_color(&theme.outer_stroke_color, "pieOuterStrokeColor")?,
            outer_stroke_width: required_css_px(
                &theme.outer_stroke_width,
                "pieOuterStrokeWidth",
                false,
            )?,
            title_font_size: required_css_px(&theme.title_text_size, "pieTitleTextSize", true)?,
            title_color: required_text_color(&theme.title_text_color, "pieTitleTextColor")?,
            section_font_size: required_css_px(
                &theme.section_text_size,
                "pieSectionTextSize",
                true,
            )?,
            section_color: required_text_color(&theme.section_text_color, "pieSectionTextColor")?,
            legend_font_size: required_css_px(&theme.legend_text_size, "pieLegendTextSize", true)?,
            legend_color: required_text_color(&theme.legend_text_color, "pieLegendTextColor")?,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "pie.document".to_string(),
                },
                DrawingCommand::Save,
                DrawingCommand::ConcatTransform {
                    transform: translate(layout.center_x, layout.center_y),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.semantics.push(SemanticAnnotation {
            id: "pie.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| self.layout.title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_plot()?;
        self.emit_title()?;
        self.emit_legend()?;

        self.commands.push(DrawingCommand::Restore);
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
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
                }),
            },
        })
    }

    fn emit_plot(&mut self) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)?;
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "pie.plot".to_string(),
        });
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

        if let Some(color) = self.outer_stroke {
            self.add_path(
                "pie.outer".to_string(),
                ellipse_path(0.0, 0.0, self.layout.outer_radius, self.layout.outer_radius),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(color, self.outer_stroke_width)),
                },
            )?;
        }

        for (index, slice) in self.layout.slices.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let semantic_id = format!("pie.slice.{index}");
            let fill = parse_optional_color(&slice.fill, "pie slice fill")?;
            let stroke = self
                .slice_stroke
                .map(|color| stroke(color, self.slice_stroke_width));
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            if fill.is_some() || stroke.is_some() {
                let highlighted = self.highlight_slice.as_deref() == Some(slice.label.as_str());
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
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.commands.push(DrawingCommand::DrawText {
                run: self.text_run(
                    text.clone(),
                    Point::new(slice.text_x, slice.text_y),
                    self.section_font_size,
                    self.section_color,
                    TextAnchor::Middle,
                )?,
            });
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Label,
                title: Some(text),
                description: Some(format!("Percentage for {}", slice.label)),
                link: None,
            });
        }

        self.commands.push(DrawingCommand::Restore);
        self.commands.push(DrawingCommand::EndSemanticGroup);
        Ok(())
    }

    fn emit_title(&mut self) -> Result<()> {
        let Some(raw_title) = self.layout.title.as_deref() else {
            return Ok(());
        };
        let title = svg_plain_text(raw_title);
        if title.is_empty() {
            return Ok(());
        }
        self.session.checkpoint(OperationPhase::Emit)?;
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "pie.title".to_string(),
        });
        self.commands.push(DrawingCommand::DrawText {
            run: self.text_run(
                title.clone(),
                Point::new(0.0, PIE_TITLE_Y),
                self.title_font_size,
                self.title_color,
                TextAnchor::Middle,
            )?,
        });
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
            let text = svg_plain_text(&pie_legend_text(
                &item.label,
                item.value,
                self.model.show_data,
            ));
            let color = parse_optional_color(&item.fill, "pie legend fill")?;
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.commands.push(DrawingCommand::Save);
            self.commands.push(DrawingCommand::ConcatTransform {
                transform: translate(self.layout.legend_x, item.y),
            });
            if let Some(color) = color {
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
                        fill: Some(Paint::solid(color)),
                        stroke: Some(stroke(color, 1.0)),
                    },
                )?;
            }
            if !text.is_empty() {
                self.commands.push(DrawingCommand::DrawText {
                    run: self.text_run(
                        text.clone(),
                        Point::new(
                            PIE_LEGEND_RECT_SIZE_PX + PIE_LEGEND_SPACING_PX,
                            LEGEND_TEXT_Y,
                        ),
                        self.legend_font_size,
                        self.legend_color,
                        TextAnchor::Start,
                    )?,
                });
            }
            self.commands.push(DrawingCommand::Restore);
            self.commands.push(DrawingCommand::EndSemanticGroup);
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
    validate_bounds(bounds)?;
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

fn required_css_px(value: &str, property: &str, positive: bool) -> Result<f64> {
    let parsed = pie_css_px(value).ok_or_else(|| {
        unavailable(format!(
            "Pie theme property `{property}` uses non-portable length `{value}`"
        ))
    })?;
    if positive && parsed <= 0.0 {
        return Err(unavailable(format!(
            "Pie theme property `{property}` must be greater than zero"
        )));
    }
    Ok(parsed)
}

fn parse_opacity(value: &str, property: &str) -> Result<f64> {
    let value = css_token(value);
    let parsed = if let Some(percent) = value.strip_suffix('%') {
        percent
            .trim()
            .parse::<f64>()
            .ok()
            .map(|value| value / 100.0)
    } else {
        value.parse::<f64>().ok()
    }
    .ok_or_else(|| {
        unavailable(format!(
            "Pie theme property `{property}` uses non-portable opacity `{value}`"
        ))
    })?;
    if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
        return Err(unavailable(format!(
            "Pie theme property `{property}` is outside 0..=1"
        )));
    }
    Ok(parsed)
}

fn parse_optional_color(value: &str, property: &str) -> Result<Option<Color>> {
    let value = css_token(value);
    if value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    if value.eq_ignore_ascii_case("transparent") {
        return Ok(Some(Color::rgba(0, 0, 0, 0)));
    }
    let parsed = ThemeColor::parse(value).map_err(|error| {
        unavailable(format!(
            "Pie theme property `{property}` uses non-portable color `{value}`: {error}"
        ))
    })?;
    let channel = |kind| parsed.channel(kind).round().clamp(0.0, 255.0) as u8;
    Ok(Some(Color::rgba(
        channel(ColorChannel::Red),
        channel(ColorChannel::Green),
        channel(ColorChannel::Blue),
        (parsed.channel(ColorChannel::Alpha) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
    )))
}

fn required_text_color(value: &str, property: &str) -> Result<Color> {
    parse_optional_color(value, property)?.ok_or_else(|| {
        unavailable(format!(
            "Pie text property `{property}` cannot be `none` in DrawingList v1"
        ))
    })
}

fn css_token(value: &str) -> &str {
    let value = value.trim().trim_end_matches(';').trim();
    value
        .strip_suffix("!important")
        .map(str::trim)
        .unwrap_or(value)
}

fn stroke(color: Color, width: f64) -> StrokeStyle {
    StrokeStyle {
        paint: Paint::solid(color),
        width,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        line_cap: merman_display_list::LineCap::Butt,
        line_join: merman_display_list::LineJoin::Miter,
        miter_limit: 4.0,
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

fn svg_plain_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if crate::text::is_html_collapsible_ascii_whitespace(character) {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(character);
    }
    output
}

fn text_obligation(session: &RenderSession) -> TextObligation {
    let route = session.text_measurement_route(TextMeasurementPhase::SvgBBox);
    let profile = profile_identity(&route.primary);
    let measurement = match route.primary_source {
        TextMeasurementSource::Host => MeasurementProvenance::HostCallback { profile },
        TextMeasurementSource::Profile => MeasurementProvenance::DeterministicFallback { profile },
    };
    TextObligation::HostText { measurement }
}

fn profile_identity(identity: &crate::environment::TextMeasurementProfileIdentity) -> String {
    let mut value = format!("{}@{}", identity.profile().as_str(), identity.version());
    for decorator in identity.decorators() {
        value.push('+');
        value.push_str(decorator);
    }
    value
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
