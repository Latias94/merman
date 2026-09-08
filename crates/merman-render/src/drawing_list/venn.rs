//! Renderer-neutral Venn adapter.
//!
//! Classic Venn output already owns deterministic area paths and label coordinates. This adapter
//! projects those typed values directly. Plain text nodes resolve normal line boxes through the
//! operation's text provider. Hand-drawn operations flow directly into the bounded path builder.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, VennSvgBody, parse_font_families_for,
    parse_svg_path,
};
use crate::config::config_diagram_look;
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::flowchart::{ellipse_path, polygon_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, VennAreaLayout, VennDiagramLayout};
use crate::rough_geometry::{display_color_to_srgba, op_to_path_segment, operation_randomness};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::theme::PresentationTheme;
use crate::venn::{
    VennAreaPresentation, VennStrokeWidth, venn_area_label, venn_area_presentation,
    venn_stable_sets_key, venn_style_by_key, venn_title,
};
use crate::{Error, Result};
use merman_core::diagrams::venn::VennDiagramRenderModel;
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, DrawingCommand, DrawingListPolicy, FillRule, FontDescriptor, FontStyle, Paint,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle,
    TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform,
    Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type VennPair = FamilyPair<VennDiagramRenderModel, VennDiagramLayout>;

const TITLE_FONT_SIZE_PX: f64 = 32.0;

fn rough_path_paint(token: &str, width: f64, fade: f64) -> Result<(PathStyle, f64)> {
    use merman_core::theme_color::{ColorChannel, ThemeColor, transparentize};
    let token = transparentize(token, fade).map_err(|error| unavailable(error.to_string()))?;
    let mut color = PortableStyleResolver::new("venn").color("rough paint", &token)?;
    let opacity = ThemeColor::parse(&token)
        .map_err(|error| unavailable(error.to_string()))?
        .channel(ColorChannel::Alpha);
    color.alpha = 255;
    Ok((
        PathStyle {
            fill_rule: FillRule::NonZero,
            fill: None,
            stroke: Some(stroke(color, width)),
        },
        opacity,
    ))
}

pub(crate) fn build_venn_document(
    pair: &VennPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: impl Into<super::DocumentBudget>,
    session: &RenderSession,
) -> Result<RenderDocument> {
    VennBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct VennBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    output: DrawingListBuilder<'a>,
    model: &'a VennDiagramRenderModel,
    layout: &'a VennDiagramLayout,
    font_family_css: String,
    font: FontDescriptor,
    text_obligation: TextObligation,
    theme: crate::theme::VennTheme,
    rough_randomness: Option<roughr::core::RoughRandomness>,
    semantic_classes: BTreeMap<String, String>,
    semantic_data_sets: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
}

impl<'a> VennBuilder<'a> {
    fn new(
        pair: &'a VennPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: impl Into<super::DocumentBudget>,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let rough_randomness = (config_diagram_look(config).as_str() == "handDrawn").then(|| {
            operation_randomness(
                session,
                config
                    .get("handDrawnSeed")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(session.render_seed().get() as f64),
                "render.venn.roughjs",
            )
        });

        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;

        let theme = PresentationTheme::new(config).venn()?;
        let font_family_css = crate::config::config_font_family_css_root_first_raw(config);
        let font = FontDescriptor {
            families: parse_font_families_for(&font_family_css, RenderFamilyKind::Venn)?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        let mut output = DrawingListBuilder::new(policy, limits, session);
        output.push_control(DrawingCommand::Save)?;
        output.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "venn.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            output,
            model,
            layout,
            font_family_css,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            theme,
            rough_randomness,
            semantic_classes: BTreeMap::new(),
            semantic_data_sets: BTreeMap::new(),
            text_classes: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let title = venn_title(self.model, self.metadata.title.as_deref());
        self.output.push_semantic(SemanticAnnotation {
            id: "venn.document".to_string(),
            role: SemanticRole::Document,
            title: self.model.acc_title.clone(),
            description: self.model.acc_descr.clone(),
            link: None,
        })?;

        self.emit_background()?;
        self.emit_title(title)?;

        self.output.push_control(DrawingCommand::Save)?;
        self.output.push_control(DrawingCommand::ConcatTransform {
            transform: translate(0.0, self.layout.title_height),
        })?;
        self.output.push_semantic(SemanticAnnotation {
            id: "venn.content".to_string(),
            role: SemanticRole::Group,
            title: None,
            description: None,
            link: None,
        })?;
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "venn.content".to_string(),
            })?;

        let style_by_key = venn_style_by_key(self.model);
        let mut circle_index = 0usize;
        for area_index in 0..self.layout.areas.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let area = self.layout.areas[area_index].clone();
            let styles = style_by_key.get(&venn_stable_sets_key(&area.sets));
            let presentation = venn_area_presentation(
                &area,
                circle_index,
                styles,
                &self.theme,
                self.layout.scale,
            )?;
            self.emit_area(area_index, circle_index, &area, &presentation)?;
            if area.sets.len() == 1 {
                circle_index += 1;
            }
        }

        self.emit_text_nodes()?;

        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_control(DrawingCommand::Restore)?;
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_control(DrawingCommand::Restore)?;

        let document = self.output.finish(
            Viewport::new(Rect::new(0.0, 0.0, self.layout.width, self.layout.height)),
            BTreeMap::from([(
                "x-merman-venn".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "text_mode": "plain_host_text",
                    "text_nodes": "resolved_normal_lines",
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        )?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Venn,
                body: SvgStructureBody::Venn(VennSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    semantic_classes: self.semantic_classes,
                    semantic_data_sets: self.semantic_data_sets,
                    text_classes: self.text_classes,
                }),
            },
        })
    }

    fn begin_text_group(&mut self, id: String, class: &str, title: Option<String>) -> Result<()> {
        self.semantic_classes.insert(id.clone(), class.to_owned());
        self.output.push_semantic(SemanticAnnotation {
            id: id.clone(),
            role: SemanticRole::Group,
            title,
            description: None,
            link: None,
        })?;
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup { semantic_id: id })
    }

    fn emit_text_nodes(&mut self) -> Result<()> {
        if self.model.text_nodes.is_empty() {
            return Ok(());
        }
        self.begin_text_group("venn.text-nodes".to_owned(), "venn-text-nodes", None)?;
        let layout = self.layout;
        let style_by_key = venn_style_by_key(self.model);
        let styles = PortableStyleResolver::new("venn");
        let mut node_index = 0usize;
        for (area_index, area) in layout.text_areas.iter().enumerate() {
            self.begin_text_group(
                format!("venn.text-area.{area_index}"),
                "venn-text-area",
                None,
            )?;
            if layout.use_debug_layout {
                let mut line = stroke(Color::rgba(128, 0, 128, 255), 1.5 * layout.scale);
                line.dash_array = vec![6.0 * layout.scale, 4.0 * layout.scale];
                self.output.draw_path(
                    ResourceId::new(format!("venn.text-area.{area_index}.debug-circle")),
                    ellipse_path(
                        area.center_x,
                        area.center_y,
                        area.inner_radius,
                        area.inner_radius,
                    ),
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(line),
                    },
                )?;
            }
            let mut cell_index = 0;
            while let Some(node) = layout
                .text_nodes
                .get(node_index)
                .filter(|node| node.sets == area.sets)
            {
                self.session.checkpoint(OperationPhase::Emit)?;
                // Upstream interleaves each debug cell and its node, rather than painting all
                // debug cells over or under all labels as a separate pass.
                if let Some(cell) = area
                    .debug_cells
                    .get(cell_index)
                    .filter(|_| layout.use_debug_layout)
                {
                    let mut line = stroke(Color::rgba(0, 128, 128, 255), layout.scale);
                    line.dash_array = vec![4.0 * layout.scale, 3.0 * layout.scale];
                    self.output.draw_path(
                        ResourceId::new(format!("venn.text.{node_index}.debug-cell")),
                        polygon_path(&[
                            Point::new(cell.x, cell.y),
                            Point::new(cell.x + cell.width, cell.y),
                            Point::new(cell.x + cell.width, cell.y + cell.height),
                            Point::new(cell.x, cell.y + cell.height),
                        ]),
                        PathStyle {
                            fill_rule: FillRule::NonZero,
                            fill: None,
                            stroke: Some(line),
                        },
                    )?;
                }
                let label = node.label.as_deref().unwrap_or(&node.id);
                if label.contains(['\u{00ad}', '\u{0085}', '\u{2028}', '\u{2029}']) {
                    return Err(unavailable(
                        "Venn normal text with discretionary hyphens or Unicode hard line separators requires explicit line-break glyph projection",
                    ));
                }
                let id = format!("venn.text.{node_index}");
                self.begin_text_group(id.clone(), "venn-text-node-fo", Some(label.to_owned()))?;
                // The unpainted box retains the source container independently of overflowing
                // line bounds. SVG shell geometry must not be copied into the private sidecar.
                self.output.draw_path(
                    ResourceId::new(format!("{id}.container")),
                    polygon_path(&[
                        Point::new(node.x, node.y),
                        Point::new(node.x + node.width, node.y),
                        Point::new(node.x + node.width, node.y + node.height),
                        Point::new(node.x, node.y + node.height),
                    ]),
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: None,
                    },
                )?;
                self.text_classes.insert(id, "venn-text-node".to_owned());
                let color = style_by_key
                    .get(&node.id)
                    .and_then(|style| style.get("color"))
                    .map(String::as_str)
                    .unwrap_or(&self.theme.set_text_color);
                let fill = styles.color("text node color", color)?;
                let measurement = MeasurementTextStyle {
                    font_family: Some(self.font_family_css.clone()),
                    font_size: area.font_size,
                    font_weight: None,
                    font_style: None,
                };
                self.output.draw_normal_text(
                    label,
                    Rect::new(node.x, node.y, node.width, node.height),
                    &measurement,
                    &TextStyle {
                        font: self.font.clone(),
                        font_size: area.font_size,
                        letter_spacing: 0.0,
                        line_height: 0.0,
                        fill: Paint::solid(fill),
                        stroke: None,
                        paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                    },
                    &text_obligation(self.session, TextMeasurementPhase::Wrap),
                )?;
                self.output.push_control(DrawingCommand::EndSemanticGroup)?;
                node_index += 1;
                cell_index += 1;
            }
            self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        }
        if node_index != layout.text_nodes.len() {
            return Err(invalid(
                "Venn text nodes do not follow their text-area layout groups",
            ));
        }
        self.output.push_control(DrawingCommand::EndSemanticGroup)
    }

    fn emit_background(&mut self) -> Result<()> {
        if self.layout.width == 0.0 || self.layout.height == 0.0 {
            return Ok(());
        }
        self.add_path(
            "venn.background".to_string(),
            polygon_path(&[
                Point::new(0.0, 0.0),
                Point::new(self.layout.width, 0.0),
                Point::new(self.layout.width, self.layout.height),
                Point::new(0.0, self.layout.height),
            ]),
            Some((Color::rgba(255, 255, 255, 255), 1.0)),
            None,
        )?;
        Ok(())
    }

    fn emit_title(&mut self, title: Option<&str>) -> Result<()> {
        let Some(title) = title.map(svg_plain_text).filter(|title| !title.is_empty()) else {
            return Ok(());
        };
        let styles = PortableStyleResolver::new("venn");
        let semantic_id = "venn.title".to_string();
        self.text_classes
            .insert(semantic_id.clone(), "venn-title".to_string());
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        if let Some(fill) = styles.optional_color("title color", &self.theme.title_color)? {
            // The `.venn-title` author rule wins over the scaled SVG presentation attribute, so
            // the browser-visible font size is always 32px even though its y position still scales.
            self.emit_text(
                std::iter::once(title.as_str()),
                Point::new(self.layout.width / 2.0, 32.0 * self.layout.scale),
                TITLE_FONT_SIZE_PX,
                fill,
                TextAnchor::Middle,
                TextBaseline::Middle,
            )?;
        }
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(title),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_area(
        &mut self,
        area_index: usize,
        circle_index: usize,
        area: &VennAreaLayout,
        presentation: &VennAreaPresentation,
    ) -> Result<()> {
        let semantic_id = format!("venn.area.{area_index}");
        let area_class = if area.sets.len() == 1 {
            format!("venn-area venn-circle venn-set-{}", circle_index % 8)
        } else {
            "venn-area venn-intersection".to_string()
        };
        self.semantic_classes
            .insert(semantic_id.clone(), area_class);
        self.semantic_data_sets
            .insert(semantic_id.clone(), area.sets.join("_"));
        self.text_classes
            .insert(semantic_id.clone(), "label".to_string());
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;

        let styles = PortableStyleResolver::new("venn");
        if self.rough_randomness.is_some() {
            self.emit_rough_area(&semantic_id, circle_index, area, presentation)?;
        } else {
            let fill_opacity = styles.opacity("fill-opacity", &presentation.fill_opacity)?;
            let fill = if fill_opacity == 0.0 {
                None
            } else {
                styles
                    .optional_color("fill", &presentation.fill_color)?
                    .map(|color| (color, fill_opacity))
            };
            let area_stroke = self.resolve_stroke(presentation, &styles)?;
            let segments = parse_svg_path(&area.path)?;
            self.add_path(format!("{semantic_id}.shape"), segments, fill, area_stroke)?;
        }

        let label = venn_area_label(area);
        if let Some(fill) = styles.optional_color("label color", &presentation.text_color)? {
            let font_size = 48.0 * self.layout.scale;
            // The child tspan resets y and applies Mermaid's single `.35em` centering offset.
            self.emit_text(
                area_label_parts(label),
                Point::new(area.text_x, area.text_y + 0.35 * font_size),
                font_size,
                fill,
                TextAnchor::Middle,
                TextBaseline::Alphabetic,
            )?;
        }

        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: visible_area_title(area),
            description: Some(format!(
                "Sets: {}; size: {}",
                area.sets.join(", "),
                area.size
            )),
            link: None,
        })?;
        Ok(())
    }

    fn emit_rough_area(
        &mut self,
        id: &str,
        circle_index: usize,
        area: &VennAreaLayout,
        presentation: &VennAreaPresentation,
    ) -> Result<()> {
        use crate::venn::rough;
        if area.sets.len() != 1 && !presentation.has_custom_fill {
            let path = parse_svg_path(&area.path)?;
            self.add_path(format!("{id}.shape"), path, None, None)?;
            return Ok(());
        }
        let randomness = self
            .rough_randomness
            .as_ref()
            .ok_or_else(|| invalid("Venn rough output has no randomness owner"))?;
        let meter = self.session.work_meter();
        let fill =
            PortableStyleResolver::new("venn").color("rough fill", &presentation.fill_color)?;
        let fill_id = ResourceId::new(format!("{id}.rough-fill"));
        if area.sets.len() == 1 {
            let circle = area
                .circles
                .first()
                .ok_or_else(|| invalid("Venn set has no circle geometry"))?;
            let stroke_token = presentation
                .stroke_color
                .as_deref()
                .ok_or_else(|| invalid("Venn set has no stroke color"))?;
            let outline_color =
                PortableStyleResolver::new("venn").color("rough stroke", stroke_token)?;
            let width = rough::stroke_width(
                presentation
                    .stroke_width
                    .as_ref()
                    .ok_or_else(|| invalid("Venn set has no stroke width"))?,
            )?;
            let mut options = rough::circle_options(
                display_color_to_srgba(fill),
                display_color_to_srgba(outline_color),
                width,
                -41.0 + circle_index as f32 * 60.0,
                randomness,
            )?;
            let outline_id = ResourceId::new(format!("{id}.rough-outline"));
            // Generate outline first for Rough.js RNG order, but defer painting until after fill.
            let points = self.output.create_path_with(outline_id.clone(), |emit| {
                rough::emit_circle_outline(circle, &mut options, meter, |op| {
                    emit(op_to_path_segment(&op)?)
                })
            })?;
            let (fill_style, fill_alpha) = rough_path_paint(&presentation.fill_color, 2.0, 0.7)?;
            self.output.push_control(DrawingCommand::Save)?;
            self.output.push_control(DrawingCommand::SetOpacity {
                opacity: fill_alpha,
            })?;
            self.output
                .draw_optional_path_with(fill_id, fill_style, |emit| {
                    rough::emit_hachure(&mut [points], &mut options, meter, |op| {
                        emit(op_to_path_segment(&op)?)
                    })
                })?;
            self.output.push_control(DrawingCommand::Restore)?;
            let (outline_style, outline_alpha) =
                rough_path_paint(stroke_token, f64::from(width), 0.0)?;
            self.output.push_control(DrawingCommand::Save)?;
            self.output.push_control(DrawingCommand::SetOpacity {
                opacity: outline_alpha,
            })?;
            self.output.draw_path_reference(outline_id, outline_style)?;
            self.output.push_control(DrawingCommand::Restore)?;
        } else {
            let (style, opacity) = rough_path_paint(&presentation.fill_color, 2.0, 0.3)?;
            self.output.push_control(DrawingCommand::Save)?;
            self.output
                .push_control(DrawingCommand::SetOpacity { opacity })?;
            self.output
                .draw_optional_path_with(fill_id, style, |emit| {
                    rough::emit_intersection_fill(
                        &area.path,
                        display_color_to_srgba(fill),
                        randomness,
                        meter,
                        |op| emit(op_to_path_segment(&op)?),
                    )
                })?;
            self.output.push_control(DrawingCommand::Restore)?;
        }
        Ok(())
    }

    fn resolve_stroke(
        &self,
        presentation: &VennAreaPresentation,
        styles: &PortableStyleResolver,
    ) -> Result<Option<(StrokeStyle, f64)>> {
        let Some(color_token) = presentation.stroke_color.as_deref() else {
            return Ok(None);
        };
        let Some(color) = styles.optional_color("stroke", color_token)? else {
            return Ok(None);
        };
        let width = match presentation.stroke_width.as_ref() {
            Some(VennStrokeWidth::Css(value)) => styles.length("stroke-width", value)?,
            Some(VennStrokeWidth::LogicalPixels(value)) => {
                if !value.is_finite() || *value < 0.0 {
                    return Err(invalid("Venn stroke width is invalid"));
                }
                *value
            }
            None => return Ok(None),
        };
        let opacity = presentation.stroke_opacity.unwrap_or(1.0);
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(invalid("Venn stroke opacity is invalid"));
        }
        Ok(Some((stroke(color, width), opacity)))
    }

    fn add_path(
        &mut self,
        id: String,
        segments: Vec<PathSegment>,
        fill: Option<(Color, f64)>,
        area_stroke: Option<(StrokeStyle, f64)>,
    ) -> Result<bool> {
        if segments.is_empty() {
            return Ok(false);
        }

        let id = ResourceId::new(id);
        let fill = fill
            .filter(|(_, opacity)| *opacity > 0.0)
            .map(|(color, opacity)| {
                (
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: Some(Paint::solid(color)),
                        stroke: None,
                    },
                    opacity,
                )
            });
        let stroke = area_stroke
            .filter(|(stroke, opacity)| stroke.width > 0.0 && *opacity > 0.0)
            .map(|(stroke, opacity)| {
                (
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(stroke),
                    },
                    opacity,
                )
            });
        let mut segments = Some(segments);
        for (style, opacity) in [fill, stroke].into_iter().flatten() {
            // Keep independent CSS paint opacity exact, without quantizing it into color alpha.
            if opacity != 1.0 {
                self.output.push_control(DrawingCommand::Save)?;
                self.output
                    .push_control(DrawingCommand::SetOpacity { opacity })?;
            }
            if let Some(segments) = segments.take() {
                self.output.draw_path(id.clone(), segments, style)?;
            } else {
                self.output.draw_path_reference(id.clone(), style)?;
            }
            if opacity != 1.0 {
                self.output.push_control(DrawingCommand::Restore)?;
            }
        }
        if let Some(segments) = segments {
            // Transparent intersections still have source geometry and semantic ownership.
            self.output.draw_path(
                id,
                segments,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: None,
                },
            )?;
        }
        Ok(true)
    }

    fn emit_text<'s>(
        &mut self,
        parts: impl Iterator<Item = &'s str> + Clone,
        origin: Point,
        font_size: f64,
        fill: Color,
        anchor: TextAnchor,
        baseline: TextBaseline,
    ) -> Result<()> {
        if font_size == 0.0 {
            return Ok(());
        }
        let session = self.session;
        let font_family_css = &self.font_family_css;
        let font = &self.font;
        let obligation = &self.text_obligation;
        self.output.draw_host_text_iter(parts, |text| {
            let measurement_style = MeasurementTextStyle {
                font_family: Some(font_family_css.clone()),
                font_size,
                font_weight: None,
                font_style: None,
            };
            let measurer = session
                .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
            let width = measurer.measure_svg_simple_text_bbox_width_px(&text, &measurement_style);
            let height = measurer.measure_svg_simple_text_bbox_height_px(&text, &measurement_style);
            session.checkpoint(OperationPhase::Emit)?;
            if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
                return Err(invalid("Venn text measurement returned invalid bounds"));
            }
            let x = match anchor {
                TextAnchor::Start => origin.x,
                TextAnchor::Middle => origin.x - width / 2.0,
                TextAnchor::End => origin.x - width,
            };
            let y = match baseline {
                TextBaseline::Hanging | TextBaseline::TextBeforeEdge => origin.y,
                TextBaseline::Middle | TextBaseline::Central => origin.y - height / 2.0,
                TextBaseline::Alphabetic
                | TextBaseline::Ideographic
                | TextBaseline::TextAfterEdge => origin.y - height,
            };
            Ok(TextRun {
                text,
                origin,
                bounds: Rect::new(x, y, width, height),
                style: TextStyle {
                    font: font.clone(),
                    font_size,
                    letter_spacing: 0.0,
                    line_height: font_size,
                    fill: Paint::solid(fill),
                    stroke: None,
                    paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                },
                anchor,
                baseline,
                direction: TextDirection::Auto,
                language: None,
                obligation: obligation.clone(),
            })
        })
    }
}

/// Venn.js tokenizes with JavaScript `\s+` before the temporary SVG is attached. Its detached
/// length probes return zero, so this normalizes words but does not perform width-based wrapping.
fn area_label_parts(value: &str) -> impl Iterator<Item = &str> + Clone {
    value
        .split(crate::text::is_ecmascript_whitespace)
        .filter(|word| !word.is_empty())
        .enumerate()
        .flat_map(|(index, word)| [if index == 0 { "" } else { " " }, word])
}

fn visible_area_title(area: &VennAreaLayout) -> Option<String> {
    let raw_label = venn_area_label(area);
    let mut label = String::with_capacity(raw_label.len());
    label.extend(area_label_parts(raw_label));
    if !label.is_empty() {
        Some(label)
    } else if area.sets.is_empty() {
        None
    } else {
        Some(area.sets.join(" ∩ "))
    }
}

fn validate_layout(layout: &VennDiagramLayout) -> Result<()> {
    if ![
        layout.width,
        layout.height,
        layout.diagram_height,
        layout.title_height,
        layout.scale,
        layout.padding,
    ]
    .into_iter()
    .all(f64::is_finite)
        || layout.width < 0.0
        || layout.height < 0.0
        || layout.diagram_height < 0.0
        || layout.title_height < 0.0
        || layout.scale < 0.0
    {
        return Err(invalid("Venn root geometry is invalid"));
    }
    if let Some(bounds) = &layout.bounds {
        validate_bounds(bounds)?;
    }
    for area in &layout.areas {
        if area.sets.is_empty()
            || !area.size.is_finite()
            || !area.text_x.is_finite()
            || !area.text_y.is_finite()
            || area.path.trim().is_empty()
        {
            return Err(invalid("Venn area geometry is invalid"));
        }
    }
    for area in &layout.text_areas {
        if ![
            area.center_x,
            area.center_y,
            area.inner_radius,
            area.font_size,
        ]
        .into_iter()
        .all(f64::is_finite)
            || area.inner_radius < 0.0
            || area.font_size <= 0.0
        {
            return Err(invalid("Venn text area geometry is invalid"));
        }
        for cell in &area.debug_cells {
            validate_bounds(&Bounds {
                min_x: cell.x,
                min_y: cell.y,
                max_x: cell.x + cell.width,
                max_y: cell.y + cell.height,
            })?;
        }
    }
    for node in &layout.text_nodes {
        validate_bounds(&Bounds {
            min_x: node.x,
            min_y: node.y,
            max_x: node.x + node.width,
            max_y: node.y + node.height,
        })?;
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
        return Err(invalid("Venn root bounds are invalid"));
    }
    Ok(())
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

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "venn".to_string(),
        reason: message.into(),
    }
}
