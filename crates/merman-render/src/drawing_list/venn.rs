//! Renderer-neutral Venn adapter.
//!
//! Classic Venn output already owns deterministic area paths and label coordinates. This adapter
//! projects those typed values directly. Browser-wrapped text nodes and RoughJS remain explicit
//! capability boundaries until the canonical document can describe them without SVG semantics.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, VennSvgBody, parse_font_families,
    parse_svg_path,
};
use crate::config::config_diagram_look;
use crate::drawing_list::flowchart::polygon_path;
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, VennAreaLayout, VennDiagramLayout};
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
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle,
    TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform,
    Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type VennPair = FamilyPair<VennDiagramRenderModel, VennDiagramLayout>;

const TITLE_FONT_SIZE_PX: f64 = 32.0;

pub(crate) fn build_venn_document(
    pair: &VennPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    VennBuilder::new(pair, metadata, policy, session)?.build()
}

struct VennBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a VennDiagramRenderModel,
    layout: &'a VennDiagramLayout,
    font_family_css: String,
    font: FontDescriptor,
    text_obligation: TextObligation,
    theme: crate::theme::VennTheme,
    semantic_classes: BTreeMap<String, String>,
    semantic_data_sets: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> VennBuilder<'a> {
    fn new(
        pair: &'a VennPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        if config_diagram_look(config).as_str() == "handDrawn" {
            return Err(unavailable(
                "hand-drawn Venn output requires RoughJS paths that are not canonical DrawingList geometry",
            ));
        }

        let model = pair.semantic();
        let layout = pair.layout();
        if !model.text_nodes.is_empty()
            || !layout.text_nodes.is_empty()
            || !layout.text_areas.is_empty()
        {
            return Err(unavailable(
                "Venn text nodes require foreignObject and browser wrapping that DrawingList v1 cannot preserve",
            ));
        }
        validate_layout(layout)?;

        let theme = PresentationTheme::new(config).venn()?;
        let font_family_css = theme.font_family_css.clone();
        let font = FontDescriptor {
            families: parse_font_families(font_family_css.clone()),
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
            font_family_css,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            theme,
            semantic_classes: BTreeMap::new(),
            semantic_data_sets: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "venn.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let title = venn_title(self.model, self.metadata.title.as_deref());
        self.semantics.push(SemanticAnnotation {
            id: "venn.document".to_string(),
            role: SemanticRole::Document,
            title: self.model.acc_title.clone(),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_background()?;
        self.emit_title(title)?;

        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::ConcatTransform {
            transform: translate(0.0, self.layout.title_height),
        });

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

        self.commands.push(DrawingCommand::Restore);
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
                "x-merman-venn".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "text_mode": "plain_host_text",
                    "text_nodes": "rejected_browser_wrapping",
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

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
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        if let Some(fill) = styles.optional_color("title color", &self.theme.title_color)? {
            // The `.venn-title` author rule wins over the scaled SVG presentation attribute, so
            // the browser-visible font size is always 32px even though its y position still scales.
            self.emit_text(
                &title,
                Point::new(self.layout.width / 2.0, 32.0 * self.layout.scale),
                TITLE_FONT_SIZE_PX,
                fill,
                TextAnchor::Middle,
                TextBaseline::Middle,
            )?;
        }
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
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        let styles = PortableStyleResolver::new("venn");
        let fill_opacity = styles.opacity("fill-opacity", &presentation.fill_opacity)?;
        let fill = if fill_opacity == 0.0 {
            None
        } else {
            styles
                .optional_color("fill", &presentation.fill_color)?
                .map(|color| (color, fill_opacity))
        };
        let area_stroke = self.resolve_stroke(presentation, &styles)?;
        if fill.is_some() || area_stroke.is_some() {
            let segments = parse_svg_path(&area.path)?;
            self.add_path(format!("{semantic_id}.shape"), segments, fill, area_stroke)?;
        }

        let label = svg_plain_text(venn_area_label(area));
        if !label.is_empty()
            && let Some(fill) = styles.optional_color("label color", &presentation.text_color)?
        {
            let font_size = 48.0 * self.layout.scale;
            // The child tspan resets y and applies Mermaid's single `.35em` centering offset.
            self.emit_text(
                &label,
                Point::new(area.text_x, area.text_y + 0.35 * font_size),
                font_size,
                fill,
                TextAnchor::Middle,
                TextBaseline::Alphabetic,
            )?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: visible_area_title(area),
            description: Some(format!(
                "Sets: {}; size: {}",
                area.sets.join(", "),
                area.size
            )),
            link: None,
        });
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
        let visible_fill = fill.is_some_and(|(_, opacity)| opacity > 0.0);
        let visible_stroke = area_stroke
            .as_ref()
            .is_some_and(|(stroke, opacity)| stroke.width > 0.0 && *opacity > 0.0);
        if segments.is_empty() || (!visible_fill && !visible_stroke) {
            return Ok(false);
        }

        let id = ResourceId::new(id);
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        if let Some((color, opacity)) = fill.filter(|(_, opacity)| *opacity > 0.0) {
            self.with_opacity(opacity, |commands| {
                commands.push(DrawingCommand::DrawPath {
                    path: id.clone(),
                    style: PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: Some(Paint::solid(color)),
                        stroke: None,
                    },
                });
            });
        }
        if let Some((area_stroke, opacity)) =
            area_stroke.filter(|(stroke, opacity)| stroke.width > 0.0 && *opacity > 0.0)
        {
            self.with_opacity(opacity, |commands| {
                commands.push(DrawingCommand::DrawPath {
                    path: id,
                    style: PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(area_stroke),
                    },
                });
            });
        }
        Ok(true)
    }

    fn with_opacity(&mut self, opacity: f64, emit: impl FnOnce(&mut Vec<DrawingCommand>)) {
        if opacity < 1.0 {
            self.commands.push(DrawingCommand::Save);
            self.commands.push(DrawingCommand::SetOpacity { opacity });
        }
        emit(&mut self.commands);
        if opacity < 1.0 {
            self.commands.push(DrawingCommand::Restore);
        }
    }

    fn emit_text(
        &mut self,
        value: &str,
        origin: Point,
        font_size: f64,
        fill: Color,
        anchor: TextAnchor,
        baseline: TextBaseline,
    ) -> Result<()> {
        if font_size == 0.0 {
            return Ok(());
        }
        let bounds = self.measure_text_bounds(value, origin, font_size, anchor, baseline)?;
        self.commands.push(DrawingCommand::draw_text(TextRun {
            text: value.to_string(),
            origin,
            bounds,
            style: TextStyle {
                font: self.font.clone(),
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
            obligation: self.text_obligation.clone(),
        }));
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
            TextBaseline::Alphabetic | TextBaseline::Ideographic | TextBaseline::TextAfterEdge => {
                origin.y - height
            }
        };
        Ok(Rect::new(x, y, width, height))
    }
}

fn visible_area_title(area: &VennAreaLayout) -> Option<String> {
    let label = svg_plain_text(venn_area_label(area));
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
