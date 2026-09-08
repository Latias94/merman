//! Renderer-neutral Treemap adapter.
//!
//! Treemap owns deterministic hierarchy geometry and a shared presentation pass for ordinal
//! colors, label fitting, clipping, and value formatting. This adapter projects that result into
//! typed paths and host text. Arbitrary CSS remains an explicit capability boundary.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, TreemapSvgBody, parse_font_families_for,
};
use crate::config::config_font_family_css_raw;
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::flowchart::polygon_path;
use crate::drawing_list::support::{PortableStyleResolver, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{TreemapDiagramLayout, TreemapLeafLayout};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::theme::PresentationTheme;
use crate::treemap::{
    TreemapCompiledStyles, TreemapLeafPresentation, TreemapPresentation,
    TreemapSectionPresentation, TreemapStyleTarget, treemap_presentation,
    treemap_value_format_is_portable,
};
use crate::{Error, Result};
use merman_core::diagrams::treemap::TreemapDiagramRenderModel;
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    BlendMode, Color, DrawingCommand, DrawingListLimits, DrawingListPolicy, FillRule,
    FontDescriptor, FontStyle, LineCap, LineJoin, Paint, PathSegment, PathStyle, Point, Rect,
    ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle, TextAnchor, TextBaseline,
    TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type TreemapPair = FamilyPair<TreemapDiagramRenderModel, TreemapDiagramLayout>;

struct TextEmitSpec<'a> {
    origin: Point,
    anchor: TextAnchor,
    baseline: TextBaseline,
    style: &'a ResolvedTextStyle,
    clip: Option<(String, Rect)>,
    class: &'a str,
}

pub(crate) fn build_treemap_document(
    pair: &TreemapPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: DrawingListLimits,
    session: &RenderSession,
) -> Result<RenderDocument> {
    TreemapBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct TreemapBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    document: DrawingListBuilder<'a>,
    model: &'a TreemapDiagramRenderModel,
    layout: &'a TreemapDiagramLayout,
    presentation: TreemapPresentation,
    base_font: FontDescriptor,
    font_family_css: String,
    title_fill: Option<Color>,
    title_font_size: f64,
    text_obligation: TextObligation,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    text_class_counts: BTreeMap<String, usize>,
}

impl<'a> TreemapBuilder<'a> {
    fn new(
        pair: &'a TreemapPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;
        if layout.show_values && !treemap_value_format_is_portable(&layout.value_format) {
            return Err(unavailable(format!(
                "Treemap valueFormat `{}` requires a D3 number format not represented by the portable v1 formatter",
                layout.value_format
            )));
        }

        let theme = PresentationTheme::new(config).treemap()?;
        let font_family_css = config_font_family_css_raw(config);
        let bbox_measurer =
            session.controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let computed_length_measurer = session
            .controlled_text_measurer(TextMeasurementPhase::ComputedLength, OperationPhase::Emit);
        let presentation = treemap_presentation(
            layout,
            &theme,
            &font_family_css,
            &bbox_measurer,
            &computed_length_measurer,
        );
        session.checkpoint(OperationPhase::Emit)?;

        let styles = PortableStyleResolver::new("treemap");
        let title_font_size =
            styles.positive_length("treemap.titleFontSize", theme.title_font_size.as_str())?;
        if (title_font_size
            - presentation
                .title
                .as_ref()
                .map_or(title_font_size, |title| title.font_size))
        .abs()
            > 1e-9
        {
            return Err(unavailable(
                "treemap.titleFontSize uses a relative CSS value whose browser sizing cannot be preserved by DrawingList v1",
            ));
        }

        let mut document = DrawingListBuilder::new(policy, limits, session);
        document.push_control(DrawingCommand::Save)?;
        document.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "treemap.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            document,
            model,
            layout,
            presentation,
            base_font: FontDescriptor {
                families: parse_font_families_for(&font_family_css, RenderFamilyKind::Treemap)?,
                weight: 400,
                style: FontStyle::Normal,
                postscript_name: None,
                resource: None,
            },
            font_family_css,
            title_fill: styles.optional_color("treemap.titleColor", &theme.title_color)?,
            title_font_size,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            text_class_counts: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.document.push_semantic(SemanticAnnotation {
            id: "treemap.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| self.layout.title.clone())
                .or_else(|| self.metadata.title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        })?;

        self.emit_background()?;
        self.emit_title()?;
        self.document.push_control(DrawingCommand::Save)?;
        self.document
            .push_control(DrawingCommand::ConcatTransform {
                transform: translate(0.0, self.layout.title_height),
            })?;
        for index in 0..self.presentation.sections.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_section(index)?;
        }
        for index in 0..self.presentation.leaves.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_leaf(index)?;
        }
        self.document.push_control(DrawingCommand::Restore)?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_control(DrawingCommand::Restore)?;

        let viewport = self.presentation.viewport;
        let document = self.document.finish(
            Viewport::new(Rect::new(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
            )),
            BTreeMap::from([(
                "x-merman-treemap".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "class_styles": "portable_typed_subset",
                    "text_mode": "computed_length_fitted_host_text",
                    "show_values": self.layout.show_values,
                    "value_format": self.layout.value_format,
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        )?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Treemap,
                body: SvgStructureBody::Treemap(TreemapSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        let viewport = self.presentation.viewport;
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return Ok(());
        }
        self.add_path(
            "treemap.background".to_string(),
            rectangle_path(viewport.x, viewport.y, viewport.width, viewport.height),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )
    }

    fn emit_title(&mut self) -> Result<()> {
        let Some(title) = self.presentation.title.clone() else {
            return Ok(());
        };
        let semantic_id = "treemap.title".to_string();
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        if let Some(fill) = self.title_fill {
            let style = ResolvedTextStyle {
                font: self.base_font.clone(),
                measurement_font_family: self.font_family_css.clone(),
                font_size: self.title_font_size,
                fill: Some(fill),
                letter_spacing: 0.0,
                line_height: self.title_font_size,
            };
            self.emit_text(
                &semantic_id,
                &title.text,
                TextEmitSpec {
                    origin: Point::new(title.x, title.y),
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                    style: &style,
                    clip: None,
                    class: "treemapTitle",
                },
            )?;
        }
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(title.text),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_section(&mut self, index: usize) -> Result<()> {
        let layout = self
            .layout
            .sections
            .get(index)
            .ok_or_else(|| invalid(format!("missing Treemap section layout {index}")))?
            .clone();
        let presentation = self
            .presentation
            .sections
            .get(index)
            .ok_or_else(|| invalid(format!("missing Treemap section presentation {index}")))?
            .clone();
        let semantic_id = format!("treemap.section.{index}");
        let section_class = format!("treemapNode section treemapSection section{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), section_class.clone());
        self.path_classes
            .insert(format!("{semantic_id}.shape"), section_class);
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;

        if !presentation.hidden {
            self.document.push_control(DrawingCommand::Save)?;
            self.document
                .push_control(DrawingCommand::ConcatTransform {
                    transform: translate(presentation.x, presentation.y),
                })?;
            let path_style = ResolvedPathStyle::resolve(
                &presentation.compiled,
                &presentation.fill,
                &presentation.stroke,
                2.0,
                0.6,
                0.4,
            )?;
            self.emit_styled_path(
                format!("{semantic_id}.shape"),
                rectangle_path(0.0, 0.0, presentation.width, presentation.height),
                &path_style,
            )?;
            self.emit_section_label(&semantic_id, &presentation)?;
            self.emit_section_value(&semantic_id, &presentation)?;
            self.document.push_control(DrawingCommand::Restore)?;
        }

        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(layout.name),
            description: Some(format!(
                "Treemap section depth {}; value {}",
                layout.depth, layout.value
            )),
            link: None,
        })?;
        Ok(())
    }

    fn emit_section_label(
        &mut self,
        semantic_id: &str,
        presentation: &TreemapSectionPresentation,
    ) -> Result<()> {
        if presentation.label_text.is_empty() {
            return Ok(());
        }
        let style = ResolvedTextStyle::resolve(
            &presentation.compiled,
            &self.base_font,
            &self.font_family_css,
            presentation.label_font_size,
            &presentation.label_fill,
            700,
            FontStyle::Normal,
        )?;
        let clip = Rect::new(
            0.0,
            0.0,
            presentation.clip_width,
            crate::treemap::TREEMAP_SECTION_HEADER_HEIGHT_PX,
        );
        self.emit_text(
            semantic_id,
            &presentation.label_text,
            TextEmitSpec {
                origin: Point::new(presentation.label_x, presentation.label_y),
                anchor: TextAnchor::Start,
                baseline: TextBaseline::Middle,
                style: &style,
                clip: Some((format!("{semantic_id}.label.clip"), clip)),
                class: "treemapSectionLabel",
            },
        )
    }

    fn emit_section_value(
        &mut self,
        semantic_id: &str,
        presentation: &TreemapSectionPresentation,
    ) -> Result<()> {
        let Some(value) = presentation
            .value_text
            .as_deref()
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        let style = ResolvedTextStyle::resolve(
            &presentation.compiled,
            &self.base_font,
            &self.font_family_css,
            presentation.value_font_size,
            &presentation.label_fill,
            400,
            FontStyle::Italic,
        )?;
        self.emit_text(
            semantic_id,
            value,
            TextEmitSpec {
                origin: Point::new(presentation.value_x, presentation.value_y),
                anchor: TextAnchor::End,
                baseline: TextBaseline::Middle,
                style: &style,
                clip: None,
                class: "treemapSectionValue",
            },
        )
    }

    fn emit_leaf(&mut self, index: usize) -> Result<()> {
        let layout = self
            .layout
            .leaves
            .get(index)
            .ok_or_else(|| invalid(format!("missing Treemap leaf layout {index}")))?
            .clone();
        let presentation = self
            .presentation
            .leaves
            .get(index)
            .ok_or_else(|| invalid(format!("missing Treemap leaf presentation {index}")))?
            .clone();
        let semantic_id = format!("treemap.leaf.{index}");
        let leaf_group_class = treemap_leaf_group_class(index, layout.class_selector.as_deref());
        self.semantic_classes
            .insert(semantic_id.clone(), leaf_group_class);
        self.path_classes.insert(
            format!("{semantic_id}.shape"),
            format!("treemapNode leaf treemapLeaf leaf{index}"),
        );
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        self.document.push_control(DrawingCommand::Save)?;
        self.document
            .push_control(DrawingCommand::ConcatTransform {
                transform: translate(presentation.x, presentation.y),
            })?;

        let path_style = ResolvedPathStyle::resolve(
            &presentation.compiled,
            &presentation.fill,
            &presentation.fill,
            3.0,
            0.3,
            1.0,
        )?;
        self.emit_styled_path(
            format!("{semantic_id}.shape"),
            rectangle_path(0.0, 0.0, presentation.width, presentation.height),
            &path_style,
        )?;
        self.emit_leaf_text(&semantic_id, &layout, &presentation)?;

        self.document.push_control(DrawingCommand::Restore)?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(layout.name),
            description: Some(format!("Treemap value {}", layout.value)),
            link: None,
        })?;
        Ok(())
    }

    fn emit_leaf_text(
        &mut self,
        semantic_id: &str,
        layout: &TreemapLeafLayout,
        presentation: &TreemapLeafPresentation,
    ) -> Result<()> {
        let clip = Rect::new(0.0, 0.0, presentation.clip_width, presentation.clip_height);
        if !presentation.label_hidden {
            let style = ResolvedTextStyle::resolve(
                &presentation.compiled,
                &self.base_font,
                &self.font_family_css,
                presentation.label_font_size,
                &presentation.label_fill,
                400,
                FontStyle::Normal,
            )?;
            self.emit_text(
                semantic_id,
                &layout.name,
                TextEmitSpec {
                    origin: Point::new(presentation.label_x, presentation.label_y),
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                    style: &style,
                    clip: Some((format!("{semantic_id}.label.clip"), clip)),
                    class: "treemapLabel",
                },
            )?;
        }
        if !presentation.value_hidden
            && let Some(value) = presentation
                .value_text
                .as_deref()
                .filter(|value| !value.is_empty())
        {
            let style = ResolvedTextStyle::resolve(
                &presentation.compiled,
                &self.base_font,
                &self.font_family_css,
                presentation.value_font_size,
                &presentation.label_fill,
                400,
                FontStyle::Normal,
            )?;
            self.emit_text(
                semantic_id,
                value,
                TextEmitSpec {
                    origin: Point::new(presentation.value_x, presentation.value_y),
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Hanging,
                    style: &style,
                    clip: Some((format!("{semantic_id}.value.clip"), clip)),
                    class: "treemapValue",
                },
            )?;
        }
        Ok(())
    }

    fn emit_styled_path(
        &mut self,
        id: String,
        segments: Vec<PathSegment>,
        style: &ResolvedPathStyle,
    ) -> Result<()> {
        if !style.visible || style.opacity <= 0.0 || segments.is_empty() {
            return Ok(());
        }
        let fill = style
            .fill
            .map(|color| with_alpha(color, style.fill_opacity));
        let stroke_color = style
            .stroke
            .filter(|_| style.stroke_width > 0.0)
            .map(|color| with_alpha(color, style.stroke_opacity));
        if fill.is_none() && stroke_color.is_none() {
            return Ok(());
        }

        let resource_id = ResourceId::new(id);
        let has_state = style.opacity != 1.0 || style.blend_mode != BlendMode::Normal;
        if has_state {
            self.document.push_control(DrawingCommand::Save)?;
            if style.opacity != 1.0 {
                self.document.push_control(DrawingCommand::SetOpacity {
                    opacity: style.opacity,
                })?;
            }
            if style.blend_mode != BlendMode::Normal {
                self.document.push_control(DrawingCommand::SetBlendMode {
                    blend_mode: style.blend_mode,
                })?;
            }
        }
        // Mermaid paints each cell as one rect: object opacity/blend applies after its
        // fill and stroke are composed, independently of either paint's own alpha.
        self.document.draw_path(
            resource_id,
            segments,
            PathStyle {
                fill_rule: style.fill_rule,
                fill: fill.map(Paint::solid),
                stroke: stroke_color.map(|color| StrokeStyle {
                    paint: Paint::solid(color),
                    width: style.stroke_width,
                    dash_array: style.dash_array.clone(),
                    dash_offset: style.dash_offset,
                    line_cap: style.line_cap,
                    line_join: style.line_join,
                    miter_limit: style.miter_limit,
                }),
            },
        )?;
        if has_state {
            self.document.push_control(DrawingCommand::Restore)?;
        }
        Ok(())
    }

    fn emit_text(&mut self, semantic_id: &str, text: &str, spec: TextEmitSpec<'_>) -> Result<()> {
        let TextEmitSpec {
            origin,
            anchor,
            baseline,
            style,
            clip,
            class,
        } = spec;
        let Some(fill) = style.fill.filter(|fill| fill.alpha > 0) else {
            return Ok(());
        };
        if text.is_empty() || style.font_size <= 0.0 {
            return Ok(());
        }
        let font = style.font.clone();
        let measurement_font_family = style.measurement_font_family.clone();
        let font_size = style.font_size;
        let letter_spacing = style.letter_spacing;
        let line_height = style.line_height;
        let obligation = self.text_obligation.clone();
        let session = self.session;
        let make_run = |text: String| {
            let bounds = measure_text_bounds(
                session,
                &text,
                origin,
                anchor,
                baseline,
                &font,
                &measurement_font_family,
                font_size,
            )?;
            Ok(TextRun {
                text,
                origin,
                bounds,
                style: TextStyle {
                    font: font.clone(),
                    font_size,
                    letter_spacing,
                    line_height,
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
        };
        if let Some((clip_id, clip_bounds)) = clip {
            if clip_bounds.width <= 0.0 || clip_bounds.height <= 0.0 {
                return Ok(());
            }
            let clip_id = ResourceId::new(clip_id);
            self.document.push_control(DrawingCommand::Save)?;
            self.document.draw_clip_path(
                clip_id,
                rectangle_path(
                    clip_bounds.x,
                    clip_bounds.y,
                    clip_bounds.width,
                    clip_bounds.height,
                ),
                FillRule::NonZero,
            )?;
            self.record_text_class(semantic_id, class);
            self.document.draw_host_text_parts(&[text], make_run)?;
            self.document.push_control(DrawingCommand::Restore)?;
        } else {
            self.record_text_class(semantic_id, class);
            self.document.draw_host_text_parts(&[text], make_run)?;
        }
        Ok(())
    }

    fn record_text_class(&mut self, semantic_id: &str, class: &str) {
        let text_index = self
            .text_class_counts
            .entry(semantic_id.to_string())
            .or_default();
        let index = *text_index;
        *text_index = text_index.saturating_add(1);
        self.text_classes
            .insert(format!("{semantic_id}#{index}"), class.to_string());
    }
}

#[allow(clippy::too_many_arguments)]
fn measure_text_bounds(
    session: &RenderSession,
    text: &str,
    origin: Point,
    anchor: TextAnchor,
    baseline: TextBaseline,
    font: &FontDescriptor,
    measurement_font_family: &str,
    font_size: f64,
) -> Result<Rect> {
    let measurement_style = MeasurementTextStyle {
        font_family: Some(measurement_font_family.to_string()),
        font_size,
        font_weight: Some(font.weight.to_string()),
        font_style: match font.style {
            FontStyle::Normal => None,
            FontStyle::Italic => Some("italic".to_string()),
            FontStyle::Oblique => Some("oblique".to_string()),
        },
    };
    let measurer =
        session.controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
    let width = measurer.measure_svg_simple_text_bbox_width_px(text, &measurement_style);
    let height = measurer.measure_svg_simple_text_bbox_height_px(text, &measurement_style);
    session.checkpoint(OperationPhase::Emit)?;
    if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
        return Err(invalid("Treemap text measurement returned invalid bounds"));
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

impl<'a> TreemapBuilder<'a> {
    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() || (style.fill.is_none() && style.stroke.is_none()) {
            return Ok(());
        }
        self.document
            .draw_path(ResourceId::new(id), segments, style)
    }
}

#[derive(Debug, Clone)]
struct ResolvedPathStyle {
    fill_rule: FillRule,
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_width: f64,
    dash_array: Vec<f64>,
    dash_offset: f64,
    line_cap: LineCap,
    line_join: LineJoin,
    miter_limit: f64,
    opacity: f64,
    fill_opacity: f64,
    stroke_opacity: f64,
    blend_mode: BlendMode,
    visible: bool,
}

impl ResolvedPathStyle {
    fn resolve(
        compiled: &TreemapCompiledStyles,
        fill: &str,
        stroke: &str,
        stroke_width: f64,
        fill_opacity: f64,
        stroke_opacity: f64,
    ) -> Result<Self> {
        let resolver = PortableStyleResolver::new("treemap");
        let mut style = Self {
            fill_rule: FillRule::NonZero,
            fill: resolver.optional_color("fill", fill)?,
            stroke: resolver.optional_color("stroke", stroke)?,
            stroke_width,
            dash_array: Vec::new(),
            dash_offset: 0.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
            opacity: 1.0,
            fill_opacity,
            stroke_opacity,
            blend_mode: BlendMode::Normal,
            visible: true,
        };
        for declaration in compiled
            .declarations
            .iter()
            .filter(|declaration| declaration.target == TreemapStyleTarget::Node)
        {
            let key = declaration.key.trim().to_ascii_lowercase();
            let value = declaration.value.trim();
            match key.as_str() {
                "fill" => style.fill = resolver.optional_color("fill", value)?,
                "stroke" => style.stroke = resolver.optional_color("stroke", value)?,
                "stroke-width" => style.stroke_width = resolver.length("stroke-width", value)?,
                "stroke-dasharray" => style.dash_array = parse_dash_array(value)?,
                "stroke-dashoffset" => {
                    style.dash_offset = parse_signed_length(value, "stroke-dashoffset")?
                }
                "stroke-linecap" => style.line_cap = parse_line_cap(value)?,
                "stroke-linejoin" => style.line_join = parse_line_join(value)?,
                "stroke-miterlimit" => {
                    style.miter_limit = parse_positive_number(value, "stroke-miterlimit")?
                }
                "fill-rule" => style.fill_rule = parse_fill_rule(value)?,
                "opacity" => style.opacity = resolver.opacity("opacity", value)?,
                "fill-opacity" => style.fill_opacity = resolver.opacity("fill-opacity", value)?,
                "stroke-opacity" => {
                    style.stroke_opacity = resolver.opacity("stroke-opacity", value)?
                }
                "mix-blend-mode" => style.blend_mode = parse_blend_mode(value)?,
                "display" => match value.to_ascii_lowercase().as_str() {
                    "none" => style.visible = false,
                    "inline" | "block" => style.visible = true,
                    _ => {
                        return Err(unavailable(format!(
                            "CSS display value `{value}` is unsupported"
                        )));
                    }
                },
                "visibility" => match value.to_ascii_lowercase().as_str() {
                    "hidden" | "collapse" => style.visible = false,
                    "visible" => style.visible = true,
                    _ => {
                        return Err(unavailable(format!(
                            "CSS visibility value `{value}` is unsupported"
                        )));
                    }
                },
                _ => {
                    return Err(unavailable(format!(
                        "Treemap classDef property `{key}` has no lossless DrawingList v1 path mapping"
                    )));
                }
            }
        }
        Ok(style)
    }
}

#[derive(Debug, Clone)]
struct ResolvedTextStyle {
    font: FontDescriptor,
    measurement_font_family: String,
    font_size: f64,
    fill: Option<Color>,
    letter_spacing: f64,
    line_height: f64,
}

impl ResolvedTextStyle {
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        compiled: &TreemapCompiledStyles,
        base_font: &FontDescriptor,
        base_font_family_css: &str,
        font_size: f64,
        fill: &str,
        weight: u16,
        font_style: FontStyle,
    ) -> Result<Self> {
        let resolver = PortableStyleResolver::new("treemap");
        let mut style = Self {
            font: FontDescriptor {
                weight,
                style: font_style,
                ..base_font.clone()
            },
            measurement_font_family: base_font_family_css.to_string(),
            font_size,
            fill: resolver.optional_color("text color", fill)?,
            letter_spacing: 0.0,
            line_height: font_size,
        };
        let mut line_height = None;
        for declaration in compiled
            .declarations
            .iter()
            .filter(|declaration| declaration.target == TreemapStyleTarget::Label)
        {
            let key = declaration.key.trim().to_ascii_lowercase();
            let value = declaration.value.trim();
            match key.as_str() {
                "color" => style.fill = resolver.optional_color("text color", value)?,
                "font-family" => {
                    if !crate::mermaid_style::is_safe_css_font_family_value(value) {
                        return Err(unavailable(format!(
                            "font-family `{value}` is not a safe portable CSS value"
                        )));
                    }
                    let family = crate::config::normalize_css_font_family(value);
                    if family.is_empty() {
                        return Err(unavailable("font-family resolves to an empty family list"));
                    }
                    style.font.families =
                        parse_font_families_for(&family, RenderFamilyKind::Treemap)?;
                    style.measurement_font_family = family;
                }
                "font-size" => {
                    style.font_size = parse_absolute_length(value, "font-size")?;
                }
                "font-weight" => style.font.weight = parse_font_weight(value)?,
                "font-style" => style.font.style = parse_font_style(value)?,
                "letter-spacing" => {
                    style.letter_spacing = if value.eq_ignore_ascii_case("normal") {
                        0.0
                    } else {
                        parse_signed_length(value, "letter-spacing")?
                    };
                }
                "line-height" => line_height = Some(value.to_string()),
                _ => {
                    return Err(unavailable(format!(
                        "Treemap classDef property `{key}` needs a richer DrawingList v1 text contract"
                    )));
                }
            }
        }
        style.line_height = line_height
            .as_deref()
            .map(|value| parse_line_height(value, style.font_size))
            .transpose()?
            .unwrap_or(style.font_size);
        Ok(style)
    }
}

fn validate_layout(layout: &TreemapDiagramLayout) -> Result<()> {
    if ![
        layout.title_height,
        layout.width,
        layout.height,
        layout.diagram_padding,
    ]
    .into_iter()
    .all(f64::is_finite)
        || layout.title_height < 0.0
        || layout.width < 0.0
        || layout.height < 0.0
        || layout.diagram_padding < 0.0
    {
        return Err(invalid("Treemap root geometry is invalid"));
    }
    for section in &layout.sections {
        if ![
            section.value,
            section.x0,
            section.y0,
            section.x1,
            section.y1,
        ]
        .into_iter()
        .all(f64::is_finite)
            || section.x1 < section.x0
            || section.y1 < section.y0
        {
            return Err(invalid("Treemap section geometry is invalid"));
        }
    }
    for leaf in &layout.leaves {
        if ![leaf.value, leaf.x0, leaf.y0, leaf.x1, leaf.y1]
            .into_iter()
            .all(f64::is_finite)
            || leaf.x1 < leaf.x0
            || leaf.y1 < leaf.y0
        {
            return Err(invalid("Treemap leaf geometry is invalid"));
        }
    }
    Ok(())
}

fn rectangle_path(x: f64, y: f64, width: f64, height: f64) -> Vec<PathSegment> {
    polygon_path(&[
        Point::new(x, y),
        Point::new(x + width, y),
        Point::new(x + width, y + height),
        Point::new(x, y + height),
    ])
}

fn parse_absolute_length(value: &str, property: &str) -> Result<f64> {
    let lower = value.trim().to_ascii_lowercase();
    if lower.ends_with('%')
        || lower.ends_with("em")
        || lower.ends_with("rem")
        || matches!(
            lower.as_str(),
            "xx-small"
                | "x-small"
                | "small"
                | "medium"
                | "large"
                | "x-large"
                | "xx-large"
                | "smaller"
                | "larger"
        )
    {
        return Err(unavailable(format!(
            "{property} `{value}` is relative to browser CSS state"
        )));
    }
    PortableStyleResolver::new("treemap").length(property, value)
}

fn parse_signed_length(value: &str, property: &str) -> Result<f64> {
    let value = value.trim();
    let (number, scale) = if let Some(number) = value.strip_suffix("px") {
        (number.trim(), 1.0)
    } else if let Some(number) = value.strip_suffix("pt") {
        (number.trim(), 4.0 / 3.0)
    } else {
        (value, 1.0)
    };
    let result = number
        .parse::<f64>()
        .map_err(|_| unavailable(format!("{property} `{value}` is not a portable length")))?
        * scale;
    if !result.is_finite() {
        return Err(unavailable(format!(
            "{property} `{value}` is outside the portable range"
        )));
    }
    Ok(result)
}

fn parse_positive_number(value: &str, property: &str) -> Result<f64> {
    let value = parse_signed_length(value, property)?;
    if value <= 0.0 {
        return Err(unavailable(format!("{property} must be greater than zero")));
    }
    Ok(value)
}

fn parse_dash_array(value: &str) -> Result<Vec<f64>> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    value
        .split([',', ' ', '\t'])
        .filter(|part| !part.is_empty())
        .map(|part| PortableStyleResolver::new("treemap").length("stroke-dasharray", part))
        .collect()
}

fn parse_line_cap(value: &str) -> Result<LineCap> {
    match value.trim().to_ascii_lowercase().as_str() {
        "butt" => Ok(LineCap::Butt),
        "round" => Ok(LineCap::Round),
        "square" => Ok(LineCap::Square),
        _ => Err(unavailable(format!(
            "stroke-linecap `{value}` is unsupported"
        ))),
    }
}

fn parse_line_join(value: &str) -> Result<LineJoin> {
    match value.trim().to_ascii_lowercase().as_str() {
        "miter" => Ok(LineJoin::Miter),
        "round" => Ok(LineJoin::Round),
        "bevel" => Ok(LineJoin::Bevel),
        _ => Err(unavailable(format!(
            "stroke-linejoin `{value}` is unsupported"
        ))),
    }
}

fn parse_fill_rule(value: &str) -> Result<FillRule> {
    match value.trim().to_ascii_lowercase().as_str() {
        "nonzero" => Ok(FillRule::NonZero),
        "evenodd" => Ok(FillRule::EvenOdd),
        _ => Err(unavailable(format!("fill-rule `{value}` is unsupported"))),
    }
}

fn parse_blend_mode(value: &str) -> Result<BlendMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(BlendMode::Normal),
        "multiply" => Ok(BlendMode::Multiply),
        "screen" => Ok(BlendMode::Screen),
        "overlay" => Ok(BlendMode::Overlay),
        "darken" => Ok(BlendMode::Darken),
        "lighten" => Ok(BlendMode::Lighten),
        "color-dodge" => Ok(BlendMode::ColorDodge),
        "color-burn" => Ok(BlendMode::ColorBurn),
        "hard-light" => Ok(BlendMode::HardLight),
        "soft-light" => Ok(BlendMode::SoftLight),
        "difference" => Ok(BlendMode::Difference),
        "exclusion" => Ok(BlendMode::Exclusion),
        _ => Err(unavailable(format!(
            "mix-blend-mode `{value}` is unsupported"
        ))),
    }
}

fn parse_font_weight(value: &str) -> Result<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(400),
        "bold" => Ok(700),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight))
            .ok_or_else(|| unavailable(format!("font-weight `{value}` is unsupported"))),
    }
}

fn parse_font_style(value: &str) -> Result<FontStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(unavailable(format!("font-style `{value}` is unsupported"))),
    }
}

fn parse_line_height(value: &str, font_size: f64) -> Result<f64> {
    if value.eq_ignore_ascii_case("normal") {
        return Ok(font_size * 1.2);
    }
    if let Some(percent) = value.trim().strip_suffix('%') {
        let percent = percent
            .trim()
            .parse::<f64>()
            .map_err(|_| unavailable(format!("line-height `{value}` is invalid")))?;
        if !percent.is_finite() || percent < 0.0 {
            return Err(unavailable("line-height must be non-negative"));
        }
        return Ok(font_size * percent / 100.0);
    }
    if let Ok(multiplier) = value.trim().parse::<f64>()
        && multiplier.is_finite()
        && multiplier >= 0.0
    {
        return Ok(font_size * multiplier);
    }
    let result = parse_absolute_length(value, "line-height")?;
    if result < 0.0 {
        return Err(unavailable("line-height must be non-negative"));
    }
    Ok(result)
}

fn with_alpha(color: Color, opacity: f64) -> Color {
    Color::rgba(
        color.red,
        color.green,
        color.blue,
        (f64::from(color.alpha) * opacity.clamp(0.0, 1.0))
            .round()
            .clamp(0.0, 255.0) as u8,
    )
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

fn treemap_leaf_group_class(index: usize, class_selector: Option<&str>) -> String {
    let leaf_index_class = class_selector
        .filter(|class| !class.trim().is_empty())
        .map_or_else(
            || format!("leaf{index}x"),
            |class| format!("leaf{index} {class}x"),
        );
    format!("treemapNode leaf treemapLeafGroup {leaf_index_class}")
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "treemap".to_string(),
        reason: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::{LayoutOptions, family};
    use merman_core::{Engine, ParseOptions};
    use merman_display_list::DrawingListDocument;

    fn styled_cells(declarations: &str) -> DrawingListDocument {
        let source = format!(
            "treemap\n\"Main\"\n    \"Group\":::cell\n        \"Item\":10:::cell\nclassDef cell {declarations};\n"
        );
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .expect("Treemap source parses")
            .expect("Treemap source is detected");
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("render session starts");
        family::prepare(parsed, &LayoutOptions::default(), session)
            .expect("Treemap layout succeeds")
            .render_drawing_list(DrawingListPolicy::VectorOnly, DrawingListLimits::default())
            .expect("Treemap DrawingList succeeds")
            .document()
            .clone()
    }

    fn cell_path<'a>(document: &'a DrawingListDocument, title: &str) -> (usize, &'a PathStyle) {
        let semantic = document
            .semantics
            .iter()
            .find(|semantic| semantic.title.as_deref() == Some(title))
            .expect("cell semantic exists");
        let shape_id = format!("{}.shape", semantic.id);
        let paths = document
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| match command {
                DrawingCommand::DrawPath { path, style } if path.as_str() == shape_id => {
                    Some((index, style))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1, "each source rect is one compositing object");
        paths[0]
    }

    #[test]
    fn cell_fill_and_stroke_share_object_opacity_and_blend() {
        let document = styled_cells(
            "fill:#ff0000,stroke:#0000ff,fill-opacity:1,stroke-opacity:1,opacity:0.5,mix-blend-mode:multiply,stroke-width:4,stroke-dasharray:5 2,stroke-dashoffset:1,stroke-linecap:round,stroke-linejoin:bevel,stroke-miterlimit:6,fill-rule:evenodd",
        );
        for title in ["Group", "Item"] {
            let (index, style) = cell_path(&document, title);
            assert_eq!(style.fill, Some(Paint::solid(Color::rgba(255, 0, 0, 255))));
            assert_eq!(style.fill_rule, FillRule::EvenOdd);
            assert_eq!(
                style.stroke,
                Some(StrokeStyle {
                    paint: Paint::solid(Color::rgba(0, 0, 255, 255)),
                    width: 4.0,
                    dash_array: vec![5.0, 2.0],
                    dash_offset: 1.0,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Bevel,
                    miter_limit: 6.0,
                })
            );
            assert!(matches!(
                &document.commands[index - 3..=index + 1],
                [
                    DrawingCommand::Save,
                    DrawingCommand::SetOpacity { opacity: 0.5 },
                    DrawingCommand::SetBlendMode {
                        blend_mode: BlendMode::Multiply
                    },
                    DrawingCommand::DrawPath { .. },
                    DrawingCommand::Restore
                ]
            ));
        }
    }

    #[test]
    fn cell_paint_alpha_is_independent_of_object_opacity() {
        let document = styled_cells(
            "fill:#ff000080,stroke:#0000ff80,fill-opacity:0.5,stroke-opacity:0.25,opacity:0.5",
        );
        for title in ["Group", "Item"] {
            let (index, style) = cell_path(&document, title);
            assert_eq!(style.fill, Some(Paint::solid(Color::rgba(255, 0, 0, 64))));
            assert_eq!(
                style.stroke.as_ref().expect("stroke exists").paint,
                Paint::solid(Color::rgba(0, 0, 255, 32))
            );
            assert!(matches!(
                &document.commands[index - 2..=index + 1],
                [
                    DrawingCommand::Save,
                    DrawingCommand::SetOpacity { opacity: 0.5 },
                    DrawingCommand::DrawPath { .. },
                    DrawingCommand::Restore
                ]
            ));
        }
    }

    #[test]
    fn cells_keep_source_default_paint_opacities() {
        let document = styled_cells("fill:#ff0000,stroke:#0000ff");
        // Mermaid's renderer.ts gives sections fill/stroke opacity 0.6/0.4 and leaves 0.3/1.
        for (title, fill_alpha, stroke_alpha, width) in
            [("Group", 153, 102, 2.0), ("Item", 77, 255, 3.0)]
        {
            let (_, style) = cell_path(&document, title);
            assert_eq!(
                style.fill,
                Some(Paint::solid(Color::rgba(255, 0, 0, fill_alpha)))
            );
            let stroke = style.stroke.as_ref().expect("stroke exists");
            assert_eq!(
                stroke.paint,
                Paint::solid(Color::rgba(0, 0, 255, stroke_alpha))
            );
            assert_eq!(stroke.width, width);
        }
        assert!(!document.commands.iter().any(|command| matches!(
            command,
            DrawingCommand::SetOpacity { .. } | DrawingCommand::SetBlendMode { .. }
        )));
    }

    #[test]
    fn leaf_group_class_preserves_upstream_suffix_placement() {
        assert_eq!(
            treemap_leaf_group_class(0, Some("redClass")),
            "treemapNode leaf treemapLeafGroup leaf0 redClassx"
        );
        assert_eq!(
            treemap_leaf_group_class(2, None),
            "treemapNode leaf treemapLeafGroup leaf2x"
        );
    }
}
