//! Renderer-neutral Radar adapter.
//!
//! Radar layout owns the axis, graticule, curve, legend, and root geometry. The shared family
//! helpers own title selection and axis-label placement. This adapter resolves the remaining
//! theme paint and emits portable paths/text without reconstructing an SVG group tree.

use super::{
    RadarSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    parse_svg_path,
};
use crate::drawing_list::flowchart::{ellipse_path, polygon_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{LayoutPoint, RadarCurveLayout, RadarDiagramLayout};
use crate::radar::{
    RadarConfigView, RadarTextAnchor, RadarTextBaseline, radar_axis_label_placement,
    radar_curves_use_polygon, radar_title,
};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::theme::PresentationTheme;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::radar::RadarDiagramRenderModel;
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle,
    TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform,
    Viewport,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

type RadarPair = FamilyPair<RadarDiagramRenderModel, RadarDiagramLayout>;
type RadarSeriesPaint = (Option<(Color, f64)>, Option<StrokeStyle>);

const RADAR_LEGEND_BOX_SIZE_PX: f64 = 12.0;
const RADAR_LEGEND_TEXT_X_PX: f64 = 16.0;

pub(crate) fn build_radar_document(
    pair: &RadarPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    RadarBuilder::new(pair, metadata, policy, session)?.build()
}

struct RadarBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a RadarDiagramRenderModel,
    layout: &'a RadarDiagramLayout,
    use_max_width: bool,
    font_family_css: String,
    font: FontDescriptor,
    text_obligation: TextObligation,
    text_color: String,
    title_color: String,
    title_font_size: String,
    axis_color: String,
    axis_stroke_width: f64,
    axis_label_font_size: f64,
    graticule_color: String,
    graticule_opacity: f64,
    graticule_stroke_width: f64,
    legend_font_size: f64,
    curve_opacity: f64,
    curve_stroke_width: f64,
    series_colors: Vec<String>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> RadarBuilder<'a> {
    fn new(
        pair: &'a RadarPair,
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
                "themeCSS is an unresolved SVG cascade input for Radar DrawingList output",
            ));
        }

        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout, model)?;
        let theme = PresentationTheme::new(config).radar();
        let render_settings = RadarConfigView::new(config).render_settings();
        let font_family_css = theme.font_family_css.clone();

        Ok(Self {
            metadata,
            session,
            policy,
            model,
            layout,
            use_max_width: render_settings.use_max_width,
            font: FontDescriptor {
                families: parse_font_families(font_family_css.clone()),
                weight: 400,
                style: FontStyle::Normal,
                postscript_name: None,
                resource: None,
            },
            font_family_css,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            text_color: theme.text_color,
            title_color: theme.title_color,
            title_font_size: theme.title_font_size_css,
            axis_color: theme.axis_color,
            axis_stroke_width: theme.axis_stroke_width,
            axis_label_font_size: theme.axis_label_font_size,
            graticule_color: theme.graticule_color,
            graticule_opacity: theme.graticule_opacity,
            graticule_stroke_width: theme.graticule_stroke_width,
            legend_font_size: theme.legend_font_size,
            curve_opacity: theme.curve_opacity,
            curve_stroke_width: theme.curve_stroke_width,
            series_colors: theme.series_colors,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "radar.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let title = radar_title(self.model, self.metadata.title.as_deref());
        self.semantics.push(SemanticAnnotation {
            id: "radar.document".to_string(),
            role: SemanticRole::Document,
            // Preserve Mermaid's accessibility contract: a body/frontmatter chart title is visual
            // content, while only an explicit `accTitle` becomes the root accessible title.
            title: self.model.acc_title.clone(),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_background()?;
        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::ConcatTransform {
            transform: translate(self.layout.center_x, self.layout.center_y),
        });

        for index in 0..self.layout.graticules.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_graticule(index)?;
        }
        for index in 0..self.layout.axes.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_axis(index)?;
        }
        for index in 0..self.layout.curves.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_curve(index)?;
        }
        for index in 0..self.layout.legend_items.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_legend_item(index)?;
        }
        self.emit_title(title)?;

        self.commands.push(DrawingCommand::Restore);
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                0.0,
                0.0,
                self.layout.svg_width,
                self.layout.svg_height,
            )),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-radar".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "graticule": self.model.options.graticule,
                    "show_legend": self.model.options.show_legend,
                    "text_mode": "plain_host_text",
                    "use_max_width": self.use_max_width,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Radar,
                body: SvgStructureBody::Radar(RadarSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.use_max_width,
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        if self.layout.svg_width == 0.0 || self.layout.svg_height == 0.0 {
            return Ok(());
        }
        self.add_painted_path(
            "radar.background".to_string(),
            polygon_path(&[
                Point::new(0.0, 0.0),
                Point::new(self.layout.svg_width, 0.0),
                Point::new(self.layout.svg_width, self.layout.svg_height),
                Point::new(0.0, self.layout.svg_height),
            ]),
            Some((Color::rgba(255, 255, 255, 255), 1.0)),
            None,
        )?;
        Ok(())
    }

    fn emit_graticule(&mut self, index: usize) -> Result<()> {
        let graticule = self
            .layout
            .graticules
            .get(index)
            .ok_or_else(|| invalid(format!("missing Radar graticule {index}")))?;
        let semantic_id = format!("radar.graticule.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        let segments = if graticule.kind == "polygon" {
            polygon_points(&graticule.points)
        } else {
            let radius = graticule
                .r
                .ok_or_else(|| invalid(format!("Radar graticule {index} has no radius")))?;
            if radius == 0.0 {
                Vec::new()
            } else {
                ellipse_path(0.0, 0.0, radius, radius)
            }
        };
        let color = PortableStyleResolver::new("radar")
            .optional_color("graticule color", &self.graticule_color)?;
        let (fill, stroke) = if let Some(color) = color {
            let opacity = portable_opacity("graticuleOpacity", self.graticule_opacity)?;
            let stroke_width =
                portable_length("graticuleStrokeWidth", self.graticule_stroke_width)?;
            (
                Some((color, opacity)),
                (stroke_width > 0.0).then(|| stroke(color, stroke_width)),
            )
        } else {
            (None, None)
        };
        self.add_painted_path(format!("{semantic_id}.shape"), segments, fill, stroke)?;

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(format!("Radar grid {}", index + 1)),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_axis(&mut self, index: usize) -> Result<()> {
        let axis = self
            .layout
            .axes
            .get(index)
            .ok_or_else(|| invalid(format!("missing Radar axis {index}")))?
            .clone();
        let semantic_id = format!("radar.axis.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        let axis_color =
            PortableStyleResolver::new("radar").optional_color("axis color", &self.axis_color)?;
        let axis_stroke = if let Some(axis_color) = axis_color {
            let stroke_width = portable_length("axisStrokeWidth", self.axis_stroke_width)?;
            (stroke_width > 0.0).then(|| stroke(axis_color, stroke_width))
        } else {
            None
        };
        self.add_painted_path(
            format!("{semantic_id}.line"),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(0.0, 0.0),
                },
                PathSegment::LineTo {
                    to: Point::new(axis.line_x2, axis.line_y2),
                },
            ],
            None,
            axis_stroke,
        )?;

        let placement = radar_axis_label_placement(&axis);
        let font_size = portable_length("axisLabelFontSize", self.axis_label_font_size)?;
        let text_color = self.axis_color.clone();
        self.emit_text(
            &axis.label,
            Point::new(placement.x, placement.y),
            font_size,
            radar_anchor(placement.anchor),
            radar_baseline(placement.baseline),
            &text_color,
        )?;

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: visible_text(&axis.label),
            description: Some(format!("Axis {}", index + 1)),
            link: None,
        });
        Ok(())
    }

    fn emit_curve(&mut self, index: usize) -> Result<()> {
        let curve = self
            .layout
            .curves
            .get(index)
            .ok_or_else(|| invalid(format!("missing Radar curve {index}")))?
            .clone();
        let semantic_id = format!("radar.curve.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        let segments = self.curve_segments(&curve)?;
        let (fill, stroke) = self.series_paint(curve.class_index, false)?;
        self.add_painted_path(format!("{semantic_id}.shape"), segments, fill, stroke)?;

        self.commands.push(DrawingCommand::EndSemanticGroup);
        let model_curve = self.model.curves.get(index);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: visible_text(&curve.label),
            description: model_curve.map(|curve| {
                format!(
                    "Values: {}",
                    curve
                        .entries
                        .iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }),
            link: None,
        });
        Ok(())
    }

    fn emit_legend_item(&mut self, index: usize) -> Result<()> {
        let item = self
            .layout
            .legend_items
            .get(index)
            .ok_or_else(|| invalid(format!("missing Radar legend item {index}")))?
            .clone();
        let semantic_id = format!("radar.legend.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::ConcatTransform {
            transform: translate(item.x, item.y),
        });

        let (fill, stroke) = self.series_paint(item.class_index, true)?;
        self.add_painted_path(
            format!("{semantic_id}.box"),
            polygon_path(&[
                Point::new(0.0, 0.0),
                Point::new(RADAR_LEGEND_BOX_SIZE_PX, 0.0),
                Point::new(RADAR_LEGEND_BOX_SIZE_PX, RADAR_LEGEND_BOX_SIZE_PX),
                Point::new(0.0, RADAR_LEGEND_BOX_SIZE_PX),
            ]),
            fill,
            stroke,
        )?;
        let font_size = portable_length("legendFontSize", self.legend_font_size)?;
        let text_color = self.text_color.clone();
        self.emit_text(
            &item.label,
            Point::new(RADAR_LEGEND_TEXT_X_PX, 0.0),
            font_size,
            TextAnchor::Start,
            TextBaseline::Hanging,
            &text_color,
        )?;

        self.commands.push(DrawingCommand::Restore);
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: visible_text(&item.label),
            description: Some("Radar legend item".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_title(&mut self, title: Option<&str>) -> Result<()> {
        let Some(title) = title else {
            return Ok(());
        };
        let title = svg_plain_text(title);
        if title.is_empty() {
            return Ok(());
        }
        let semantic_id = "radar.title".to_string();
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let font_size =
            PortableStyleResolver::new("radar").length("title font-size", &self.title_font_size)?;
        let title_color = self.title_color.clone();
        self.emit_text(
            &title,
            Point::new(0.0, self.layout.title_y),
            font_size,
            TextAnchor::Middle,
            TextBaseline::Hanging,
            &title_color,
        )?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(title),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_text(
        &mut self,
        value: &str,
        origin: Point,
        font_size: f64,
        anchor: TextAnchor,
        baseline: TextBaseline,
        color: &str,
    ) -> Result<()> {
        let value = svg_plain_text(value);
        if value.is_empty() || font_size == 0.0 {
            return Ok(());
        }
        let Some(fill) = PortableStyleResolver::new("radar").optional_color("text fill", color)?
        else {
            return Ok(());
        };
        let bounds = self.measure_text_bounds(&value, origin, font_size, anchor, baseline)?;
        self.commands.push(DrawingCommand::DrawText {
            run: TextRun {
                text: value,
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
            },
        });
        Ok(())
    }

    fn measure_text_bounds(
        &self,
        text: &str,
        origin: Point,
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
            return Err(invalid("Radar text measurement returned invalid bounds"));
        }
        let x = match anchor {
            TextAnchor::Start => origin.x,
            TextAnchor::Middle => origin.x - width / 2.0,
            TextAnchor::End => origin.x - width,
        };
        let y = match baseline {
            TextBaseline::Hanging | TextBaseline::TextBeforeEdge => origin.y,
            TextBaseline::Middle => origin.y - height / 2.0,
            TextBaseline::Alphabetic | TextBaseline::Ideographic | TextBaseline::TextAfterEdge => {
                origin.y - height
            }
        };
        Ok(Rect::new(x, y, width, height))
    }

    fn curve_segments(&self, curve: &RadarCurveLayout) -> Result<Vec<PathSegment>> {
        if radar_curves_use_polygon(self.layout) {
            return Ok(polygon_points(&curve.points));
        }
        if curve.path_d.trim().is_empty() {
            return Ok(Vec::new());
        }
        parse_svg_path(&curve.path_d)
    }

    fn series_paint(&self, class_index: i64, legend: bool) -> Result<RadarSeriesPaint> {
        let index = usize::try_from(class_index)
            .map_err(|_| invalid(format!("Radar series class index {class_index} is invalid")))?;
        let styles = PortableStyleResolver::new("radar");
        let Some(color_token) = self.series_colors.get(index) else {
            // Mermaid only emits twelve `.radarCurve-N` rules. Higher indexes inherit the root
            // fill and retain SVG's initial `stroke:none`; they do not wrap the palette.
            let fill = styles.optional_color("inherited series fill", &self.text_color)?;
            return Ok((fill.map(|color| (color, 1.0)), None));
        };
        let color = styles.optional_color(&format!("cScale{index}"), color_token)?;
        let Some(color) = color else {
            return Ok((None, None));
        };
        let opacity = portable_opacity("curveOpacity", self.curve_opacity)?;
        let stroke_width = if legend {
            1.0
        } else {
            portable_length("curveStrokeWidth", self.curve_stroke_width)?
        };
        Ok((
            Some((color, opacity)),
            if stroke_width == 0.0 {
                None
            } else {
                Some(stroke(color, stroke_width))
            },
        ))
    }

    fn add_painted_path(
        &mut self,
        id: String,
        segments: Vec<PathSegment>,
        fill: Option<(Color, f64)>,
        stroke: Option<StrokeStyle>,
    ) -> Result<bool> {
        let visible_fill = fill.is_some_and(|(_, opacity)| opacity > 0.0);
        if segments.is_empty() || (!visible_fill && stroke.is_none()) {
            return Ok(false);
        }

        let id = ResourceId::new(id);
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        if let Some((color, opacity)) = fill.filter(|(_, opacity)| *opacity > 0.0) {
            if opacity < 1.0 {
                self.commands.push(DrawingCommand::Save);
                self.commands.push(DrawingCommand::SetOpacity { opacity });
            }
            self.commands.push(DrawingCommand::DrawPath {
                path: id.clone(),
                style: PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(color)),
                    stroke: None,
                },
            });
            if opacity < 1.0 {
                self.commands.push(DrawingCommand::Restore);
            }
        }
        if let Some(stroke) = stroke {
            self.commands.push(DrawingCommand::DrawPath {
                path: id,
                style: PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke),
                },
            });
        }
        Ok(true)
    }
}

fn polygon_points(points: &[LayoutPoint]) -> Vec<PathSegment> {
    polygon_path(
        &points
            .iter()
            .map(|point| Point::new(point.x, point.y))
            .collect::<Vec<_>>(),
    )
}

fn visible_text(value: &str) -> Option<String> {
    let value = svg_plain_text(value);
    (!value.is_empty()).then_some(value)
}

fn radar_anchor(anchor: RadarTextAnchor) -> TextAnchor {
    match anchor {
        RadarTextAnchor::Start => TextAnchor::Start,
        RadarTextAnchor::Middle => TextAnchor::Middle,
        RadarTextAnchor::End => TextAnchor::End,
    }
}

fn radar_baseline(baseline: RadarTextBaseline) -> TextBaseline {
    match baseline {
        RadarTextBaseline::Alphabetic => TextBaseline::Alphabetic,
        RadarTextBaseline::Hanging => TextBaseline::Hanging,
        RadarTextBaseline::Middle => TextBaseline::Middle,
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

fn portable_length(property: &str, value: f64) -> Result<f64> {
    if !value.is_finite() || value < 0.0 {
        return Err(unavailable(format!(
            "theme property `{property}` is outside the portable length range"
        )));
    }
    Ok(value)
}

fn portable_opacity(property: &str, value: f64) -> Result<f64> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(unavailable(format!(
            "theme property `{property}` is outside 0..=1"
        )));
    }
    Ok(value)
}

fn validate_layout(layout: &RadarDiagramLayout, model: &RadarDiagramRenderModel) -> Result<()> {
    if ![
        layout.svg_width,
        layout.svg_height,
        layout.center_x,
        layout.center_y,
        layout.radius,
        layout.axis_label_factor,
        layout.title_y,
    ]
    .into_iter()
    .all(f64::is_finite)
        || layout.svg_width < 0.0
        || layout.svg_height < 0.0
        || layout.radius < 0.0
    {
        return Err(invalid("Radar root geometry is invalid"));
    }
    if layout.axes.len() != model.axes.len() || layout.curves.len() != model.curves.len() {
        return Err(invalid(
            "Radar semantic and visual axis/curve counts do not agree",
        ));
    }
    for axis in &layout.axes {
        if ![
            axis.angle,
            axis.line_x2,
            axis.line_y2,
            axis.label_x,
            axis.label_y,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(invalid("Radar contains invalid axis geometry"));
        }
    }
    for graticule in &layout.graticules {
        match graticule.kind.as_str() {
            "polygon" => validate_points(&graticule.points)?,
            "circle" => {
                let radius = graticule
                    .r
                    .ok_or_else(|| invalid("Radar circle graticule has no radius"))?;
                if !radius.is_finite() || radius < 0.0 {
                    return Err(invalid("Radar circle graticule has invalid geometry"));
                }
            }
            kind => {
                return Err(invalid(format!(
                    "Radar graticule kind `{kind}` is unsupported"
                )));
            }
        }
    }
    for curve in &layout.curves {
        if curve.class_index < 0 {
            return Err(invalid("Radar curve class index is negative"));
        }
        validate_points(&curve.points)?;
    }
    for item in &layout.legend_items {
        if item.class_index < 0 || !item.x.is_finite() || !item.y.is_finite() {
            return Err(invalid("Radar legend item geometry is invalid"));
        }
        if usize::try_from(item.class_index)
            .ok()
            .is_none_or(|index| index >= layout.curves.len())
        {
            return Err(invalid("Radar legend item references a missing curve"));
        }
    }
    Ok(())
}

fn validate_points(points: &[LayoutPoint]) -> Result<()> {
    if points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(invalid("Radar contains non-finite point geometry"));
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
        family: "radar".to_string(),
        reason: message.into(),
    }
}
