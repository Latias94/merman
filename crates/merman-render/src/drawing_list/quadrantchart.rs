//! Renderer-neutral QuadrantChart adapter.
//!
//! QuadrantChart layout already owns the final rectangles, border lines, point circles, text
//! anchors, and rotations. This adapter preserves that paint order and resolves the remaining
//! family-local CSS tokens without reconstructing the SVG group tree.

use super::{
    QuadrantChartSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar,
    parse_font_families,
};
use crate::config::config_font_family_css;
use crate::drawing_list::flowchart::{ellipse_path, polygon_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{
    QuadrantChartBorderLineData, QuadrantChartDiagramLayout, QuadrantChartPointData,
    QuadrantChartQuadrantData, QuadrantChartTextData,
};
use crate::quadrantchart::{
    QUADRANT_BROWSER_POINT_FILL, QUADRANT_BROWSER_POINT_STROKE, QuadrantChartConfigView,
    QuadrantTextAnchor, QuadrantTextBaseline, is_mermaid_missing_amount_hsl, quadrant_text_anchor,
    quadrant_text_baseline,
};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::quadrant_chart::QuadrantChartRenderModel;
use merman_display_list::{
    CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument, DrawingListPolicy,
    DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource, PathSegment,
    PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor, TextBaseline,
    TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type QuadrantChartPair = FamilyPair<QuadrantChartRenderModel, QuadrantChartDiagramLayout>;

pub(crate) fn build_quadrantchart_document(
    pair: &QuadrantChartPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    QuadrantChartBuilder::new(pair, metadata, policy, session)?.build()
}

struct QuadrantChartBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a QuadrantChartRenderModel,
    layout: &'a QuadrantChartDiagramLayout,
    root_width: f64,
    root_height: f64,
    font_family_css: String,
    font: FontDescriptor,
    text_obligation: TextObligation,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    svg_semantic_classes: BTreeMap<String, String>,
    use_max_width: bool,
}

impl<'a> QuadrantChartBuilder<'a> {
    fn new(
        pair: &'a QuadrantChartPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let layout = pair.layout();
        validate_layout(layout)?;
        let font_family_css = config_font_family_css(config);
        let use_max_width = QuadrantChartConfigView::new(config)
            .render_settings()
            .use_max_width;
        Ok(Self {
            metadata,
            session,
            policy,
            model: pair.semantic(),
            layout,
            root_width: layout.width.max(1.0),
            root_height: layout.height.max(1.0),
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
            commands: vec![DrawingCommand::Save],
            semantics: Vec::new(),
            svg_semantic_classes: BTreeMap::new(),
            use_max_width,
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.begin_group("quadrantchart.document");
        self.semantics.push(SemanticAnnotation {
            id: "quadrantchart.document".to_string(),
            role: SemanticRole::Document,
            // Body/frontmatter titles are visible chart labels.  Only an explicit `accTitle`
            // belongs in the root accessibility metadata.
            title: self.model.acc_title.clone(),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.begin_group("quadrantchart.quadrants");
        for index in 0..self.layout.quadrants.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let quadrant = self.layout.quadrants[index].clone();
            self.emit_quadrant(index, &quadrant)?;
        }
        self.end_group();
        self.push_structural_semantic("quadrantchart.quadrants");

        self.begin_group("quadrantchart.border");
        for index in 0..self.layout.border_lines.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let border = self.layout.border_lines[index].clone();
            self.emit_border(index, &border)?;
        }
        self.end_group();
        self.push_structural_semantic("quadrantchart.border");

        self.begin_group("quadrantchart.data-points");
        for index in 0..self.layout.points.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let point = self.layout.points[index].clone();
            self.emit_point(index, &point)?;
        }
        self.end_group();
        self.push_structural_semantic("quadrantchart.data-points");

        self.begin_group("quadrantchart.labels");
        for index in 0..self.layout.axis_labels.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let label = self.layout.axis_labels[index].clone();
            self.emit_standalone_text(
                format!("quadrantchart.axis-label.{index}"),
                SemanticRole::Label,
                &label,
                "axis label fill",
            )?;
        }
        self.end_group();
        self.push_structural_semantic("quadrantchart.labels");

        if let Some(title) = self.layout.title.clone() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_standalone_text(
                "quadrantchart.title".to_string(),
                SemanticRole::Label,
                &title,
                "title fill",
            )?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(0.0, 0.0, self.root_width, self.root_height)),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-quadrantchart".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "text_mode": "plain_host_text",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::QuadrantChart,
                body: SvgStructureBody::QuadrantChart(QuadrantChartSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.use_max_width,
                    semantic_classes: self.svg_semantic_classes,
                }),
            },
        })
    }

    fn begin_group(&mut self, semantic_id: &str) {
        if let Some(class) = quadrantchart_svg_group_class(semantic_id) {
            self.svg_semantic_classes
                .insert(semantic_id.to_string(), class.to_string());
        }
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        });
    }

    fn end_group(&mut self) {
        self.commands.push(DrawingCommand::EndSemanticGroup);
    }

    fn push_structural_semantic(&mut self, semantic_id: &str) {
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Group,
            title: None,
            description: None,
            link: None,
        });
    }

    fn emit_quadrant(&mut self, index: usize, quadrant: &QuadrantChartQuadrantData) -> Result<()> {
        let semantic_id = format!("quadrantchart.quadrant.{index}");
        self.begin_group(&semantic_id);
        let styles = PortableStyleResolver::new("quadrantchart");
        if let Some(fill) = styles.optional_color("quadrant fill", &quadrant.fill)? {
            self.add_path(
                format!("{semantic_id}.shape"),
                polygon_path(&[
                    Point::new(quadrant.x, quadrant.y),
                    Point::new(quadrant.x + quadrant.width, quadrant.y),
                    Point::new(quadrant.x + quadrant.width, quadrant.y + quadrant.height),
                    Point::new(quadrant.x, quadrant.y + quadrant.height),
                ]),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: None,
                },
            )?;
        }
        self.emit_text(&quadrant.text, "quadrant text fill")?;
        self.end_group();
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: visible_text(&quadrant.text.text),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_border(&mut self, index: usize, border: &QuadrantChartBorderLineData) -> Result<()> {
        if border.stroke_width == 0.0 {
            return Ok(());
        }
        let Some(color) = PortableStyleResolver::new("quadrantchart")
            .optional_color("border stroke", &border.stroke_fill)?
        else {
            return Ok(());
        };
        self.add_path(
            format!("quadrantchart.border.{index}"),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(border.x1, border.y1),
                },
                PathSegment::LineTo {
                    to: Point::new(border.x2, border.y2),
                },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(color, border.stroke_width)),
            },
        )
    }

    fn emit_point(&mut self, index: usize, point: &QuadrantChartPointData) -> Result<()> {
        let semantic_id = format!("quadrantchart.point.{index}");
        self.begin_group(&semantic_id);
        let styles = PortableStyleResolver::new("quadrantchart");
        let fill_value = if is_mermaid_missing_amount_hsl(&point.fill) {
            QUADRANT_BROWSER_POINT_FILL
        } else {
            &point.fill
        };
        let fill = styles.optional_color("point fill", fill_value)?;
        let stroke_width = styles.length("point stroke-width", &point.stroke_width)?;
        let stroke_value = if is_mermaid_missing_amount_hsl(&point.stroke_color) {
            QUADRANT_BROWSER_POINT_STROKE
        } else {
            &point.stroke_color
        };
        let stroke = styles
            .optional_color("point stroke", stroke_value)?
            .map(|color| stroke(color, stroke_width));
        if point.radius > 0.0 && (fill.is_some() || stroke.is_some()) {
            self.add_path(
                format!("{semantic_id}.shape"),
                ellipse_path(point.x, point.y, point.radius, point.radius),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: fill.map(Paint::solid),
                    stroke,
                },
            )?;
        }
        self.emit_text(&point.text, "point text fill")?;
        self.end_group();
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: visible_text(&point.text.text),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_standalone_text(
        &mut self,
        semantic_id: String,
        role: SemanticRole,
        text: &QuadrantChartTextData,
        fill_property: &'static str,
    ) -> Result<()> {
        let title = visible_text(&text.text);
        self.begin_group(&semantic_id);
        self.emit_text(text, fill_property)?;
        self.end_group();
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role,
            title,
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_text(
        &mut self,
        text: &QuadrantChartTextData,
        fill_property: &'static str,
    ) -> Result<()> {
        let text_value = svg_plain_text(&text.text);
        if text_value.is_empty() {
            return Ok(());
        }
        let Some(fill) = PortableStyleResolver::new("quadrantchart")
            .optional_color(fill_property, &text.fill)?
        else {
            return Ok(());
        };
        let anchor = match quadrant_text_anchor(&text.vertical_pos) {
            QuadrantTextAnchor::Start => TextAnchor::Start,
            QuadrantTextAnchor::Middle => TextAnchor::Middle,
        };
        let baseline = match quadrant_text_baseline(&text.horizontal_pos) {
            QuadrantTextBaseline::Hanging => TextBaseline::Hanging,
            QuadrantTextBaseline::Middle => TextBaseline::Middle,
        };
        let bounds = self.measure_text_bounds(&text_value, text.font_size, anchor, baseline)?;
        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::ConcatTransform {
            transform: text_transform(text.x, text.y, text.rotation),
        });
        self.commands.push(DrawingCommand::draw_text(TextRun {
            text: text_value,
            origin: Point::new(0.0, 0.0),
            bounds,
            style: TextStyle {
                font: self.font.clone(),
                font_size: text.font_size,
                letter_spacing: 0.0,
                line_height: text.font_size,
                fill: Paint::solid(fill),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor,
            baseline,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        }));
        self.commands.push(DrawingCommand::Restore);
        Ok(())
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
            return Err(invalid(
                "QuadrantChart text measurement returned invalid bounds",
            ));
        }
        let x = match anchor {
            TextAnchor::Start => 0.0,
            TextAnchor::Middle => -width / 2.0,
            TextAnchor::End => -width,
        };
        let y = match baseline {
            TextBaseline::Hanging | TextBaseline::TextBeforeEdge => 0.0,
            TextBaseline::Middle | TextBaseline::Central => -height / 2.0,
            TextBaseline::Alphabetic | TextBaseline::Ideographic | TextBaseline::TextAfterEdge => {
                -height
            }
        };
        Ok(Rect::new(x, y, width, height))
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!(
                "QuadrantChart path `{id}` has no geometry"
            )));
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

fn visible_text(text: &str) -> Option<String> {
    let text = svg_plain_text(text);
    (!text.is_empty()).then_some(text)
}

fn quadrantchart_svg_group_class(semantic_id: &str) -> Option<&'static str> {
    match semantic_id {
        "quadrantchart.document" => Some("main"),
        "quadrantchart.quadrants" => Some("quadrants"),
        "quadrantchart.border" => Some("border"),
        "quadrantchart.data-points" => Some("data-points"),
        "quadrantchart.labels" => Some("labels"),
        "quadrantchart.title" => Some("title"),
        id if id.starts_with("quadrantchart.quadrant.") => Some("quadrant"),
        id if id.starts_with("quadrantchart.point.") => Some("data-point"),
        id if id.starts_with("quadrantchart.axis-label.") => Some("label"),
        _ => None,
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

fn validate_layout(layout: &QuadrantChartDiagramLayout) -> Result<()> {
    if ![layout.width, layout.height]
        .into_iter()
        .all(f64::is_finite)
    {
        return Err(invalid("QuadrantChart root geometry is non-finite"));
    }
    for quadrant in &layout.quadrants {
        if ![quadrant.x, quadrant.y, quadrant.width, quadrant.height]
            .into_iter()
            .all(f64::is_finite)
            || quadrant.width < 0.0
            || quadrant.height < 0.0
        {
            return Err(invalid("QuadrantChart contains invalid quadrant geometry"));
        }
        validate_text(&quadrant.text)?;
    }
    for border in &layout.border_lines {
        if ![
            border.stroke_width,
            border.x1,
            border.y1,
            border.x2,
            border.y2,
        ]
        .into_iter()
        .all(f64::is_finite)
            || border.stroke_width < 0.0
        {
            return Err(invalid("QuadrantChart contains invalid border geometry"));
        }
    }
    for point in &layout.points {
        if ![point.x, point.y, point.radius]
            .into_iter()
            .all(f64::is_finite)
            || point.radius < 0.0
        {
            return Err(invalid("QuadrantChart contains invalid point geometry"));
        }
        validate_text(&point.text)?;
    }
    for label in &layout.axis_labels {
        validate_text(label)?;
    }
    if let Some(title) = &layout.title {
        validate_text(title)?;
    }
    Ok(())
}

fn validate_text(text: &QuadrantChartTextData) -> Result<()> {
    if ![text.x, text.y, text.font_size, text.rotation]
        .into_iter()
        .all(f64::is_finite)
        || text.font_size <= 0.0
    {
        return Err(invalid("QuadrantChart contains invalid text geometry"));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}
