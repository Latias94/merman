//! Renderer-neutral Ishikawa adapter.
//!
//! The typed Ishikawa layout already owns the classic fishbone geometry, paint order, wrapped
//! labels, and root viewport. This adapter expands SVG markers into ordinary paths and resolves
//! the family theme without importing SVG or exposing DOM concepts to native hosts.

use super::{
    IshikawaSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    parse_svg_path,
};
use crate::config::config_diagram_look;
use crate::drawing_list::flowchart::polygon_path;
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::ishikawa::{
    ISHIKAWA_ARROW_MARKER_HEIGHT, ISHIKAWA_ARROW_MARKER_WIDTH, ISHIKAWA_ARROW_REF_X,
    ISHIKAWA_ARROW_REF_Y, ISHIKAWA_ARROW_VIEWBOX_HEIGHT, ISHIKAWA_ARROW_VIEWBOX_WIDTH,
    ISHIKAWA_LINE_STROKE_WIDTH, IshikawaTextAnchor, IshikawaTextBaseline, IshikawaTextPresentation,
    ishikawa_line_stroke_width, ishikawa_text_presentation,
};
use crate::model::{
    Bounds, IshikawaBranchLayout, IshikawaDiagramLayout, IshikawaHeadLayout,
    IshikawaLabelBoxLayout, IshikawaLineLayout, IshikawaSubGroupLayout, IshikawaTextLayout,
};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::theme::PresentationTheme;
use crate::{Error, Result};
use merman_core::diagrams::ishikawa::IshikawaDiagramRenderModel;
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

type IshikawaPair = FamilyPair<IshikawaDiagramRenderModel, IshikawaDiagramLayout>;

pub(crate) fn build_ishikawa_document(
    pair: &IshikawaPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    IshikawaBuilder::new(pair, metadata, policy, session)?.build()
}

struct IshikawaBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a IshikawaDiagramRenderModel,
    layout: &'a IshikawaDiagramLayout,
    font_family_css: String,
    font: FontDescriptor,
    text_obligation: TextObligation,
    line_color: Option<Color>,
    main_background: Option<Color>,
    text_color: Option<Color>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> IshikawaBuilder<'a> {
    fn new(
        pair: &'a IshikawaPair,
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
                "themeCSS is an unresolved SVG cascade input for Ishikawa DrawingList output",
            ));
        }
        if config_diagram_look(config).as_str() == "handDrawn" {
            return Err(unavailable(
                "hand-drawn Ishikawa uses RoughJS paths and has no lossless portable adapter or bounded raster subtree",
            ));
        }

        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout, model)?;
        let theme = PresentationTheme::new(config).ishikawa();
        let styles = PortableStyleResolver::new("ishikawa");
        let font_family_css = theme.font_family;

        Ok(Self {
            metadata,
            session,
            policy,
            model,
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
            line_color: styles.optional_color("lineColor", &theme.line_color)?,
            main_background: styles.optional_color("mainBkg", &theme.main_bkg)?,
            text_color: styles.optional_color("textColor", &theme.text_color)?,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "ishikawa.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let document_title = self
            .model
            .acc_title
            .clone()
            .or_else(|| self.model.title.clone())
            .or_else(|| self.metadata.title.clone())
            .or_else(|| {
                self.model
                    .root
                    .as_ref()
                    .and_then(|root| visible_text(&root.text))
            })
            .or_else(|| Some(self.metadata.diagram_type.clone()));
        self.semantics.push(SemanticAnnotation {
            id: "ishikawa.document".to_string(),
            role: SemanticRole::Document,
            title: document_title,
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_background()?;
        if let Some(spine) = self.layout.spine.clone() {
            self.emit_line(
                "ishikawa.spine",
                &spine,
                Some("Ishikawa spine".to_string()),
                Some("Main fishbone axis".to_string()),
            )?;
        }
        if let Some(head) = self.layout.head.clone() {
            self.emit_head(&head)?;
        }
        for pair_index in 0..self.layout.pairs.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let pair = self.layout.pairs[pair_index].clone();
            self.emit_branch(pair_index, "upper", &pair.upper)?;
            if let Some(lower) = pair.lower {
                self.emit_branch(pair_index, "lower", &lower)?;
            }
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                self.layout.viewbox_x,
                self.layout.viewbox_y,
                self.layout.total_width,
                self.layout.total_height,
            )),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-ishikawa".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "look": "classic",
                    "text_mode": "plain_host_text",
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Ishikawa,
                body: SvgStructureBody::Ishikawa(IshikawaSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        if self.layout.total_width == 0.0 || self.layout.total_height == 0.0 {
            return Ok(());
        }
        self.add_path(
            "ishikawa.background".to_string(),
            polygon_path(&[
                Point::new(self.layout.viewbox_x, self.layout.viewbox_y),
                Point::new(
                    self.layout.viewbox_x + self.layout.total_width,
                    self.layout.viewbox_y,
                ),
                Point::new(
                    self.layout.viewbox_x + self.layout.total_width,
                    self.layout.viewbox_y + self.layout.total_height,
                ),
                Point::new(
                    self.layout.viewbox_x,
                    self.layout.viewbox_y + self.layout.total_height,
                ),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )
    }

    fn emit_head(&mut self, head: &IshikawaHeadLayout) -> Result<()> {
        let semantic_id = "ishikawa.head".to_string();
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        let segments = parse_svg_path(&head.path_d)?;
        let fill = self.main_background.map(Paint::solid);
        let stroke = self
            .line_color
            .map(|color| stroke(color, ISHIKAWA_LINE_STROKE_WIDTH));
        if fill.is_some() || stroke.is_some() {
            self.commands.push(DrawingCommand::Save);
            self.commands.push(DrawingCommand::ConcatTransform {
                transform: translate(head.x, head.y),
            });
            self.add_path(
                format!("{semantic_id}.shape"),
                segments,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill,
                    stroke,
                },
            )?;
            self.commands.push(DrawingCommand::Restore);
        }
        self.emit_text(&head.label)?;

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: visible_text(&head.label.text),
            description: Some("Ishikawa effect".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_branch(
        &mut self,
        pair_index: usize,
        side: &str,
        branch: &IshikawaBranchLayout,
    ) -> Result<()> {
        let semantic_id = format!("ishikawa.branch.{pair_index}.{side}");
        let title = visible_text(&branch.label_group.label.text);
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });

        self.emit_line(
            &format!("{semantic_id}.line"),
            &branch.line,
            title.clone(),
            Some(format!("{side} cause branch")),
        )?;
        self.emit_label_box(
            &format!("{semantic_id}.label-box"),
            &branch.label_group.label_box,
        )?;
        self.emit_text(&branch.label_group.label)?;
        for subgroup_index in 0..branch.sub_groups.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_subgroup(
                &semantic_id,
                subgroup_index,
                &branch.sub_groups[subgroup_index],
            )?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title,
            description: Some(format!("Ishikawa {side} cause")),
            link: None,
        });
        Ok(())
    }

    fn emit_subgroup(
        &mut self,
        branch_id: &str,
        subgroup_index: usize,
        subgroup: &IshikawaSubGroupLayout,
    ) -> Result<()> {
        let semantic_id = format!("{branch_id}.cause.{subgroup_index}");
        let title = visible_text(&subgroup.label.text);
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.emit_line(
            &format!("{semantic_id}.line"),
            &subgroup.line,
            title.clone(),
            Some("Contributing cause connection".to_string()),
        )?;
        self.emit_text(&subgroup.label)?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title,
            description: Some("Ishikawa contributing cause".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_line(
        &mut self,
        semantic_id: &str,
        line: &IshikawaLineLayout,
        title: Option<String>,
        description: Option<String>,
    ) -> Result<()> {
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        });
        if let Some(color) = self.line_color {
            let stroke_width = ishikawa_line_stroke_width(&line.class_name);
            self.add_path(
                format!("{semantic_id}.path"),
                vec![
                    PathSegment::MoveTo {
                        to: Point::new(line.x1, line.y1),
                    },
                    PathSegment::LineTo {
                        to: Point::new(line.x2, line.y2),
                    },
                ],
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(color, stroke_width)),
                },
            )?;
            if line.marker_start
                && let Some(marker) = marker_start_path(line, stroke_width)?
            {
                self.add_path(
                    format!("{semantic_id}.marker.start"),
                    marker,
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: Some(Paint::solid(color)),
                        stroke: None,
                    },
                )?;
            }
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Edge,
            title,
            description,
            link: None,
        });
        Ok(())
    }

    fn emit_label_box(&mut self, id: &str, label_box: &IshikawaLabelBoxLayout) -> Result<()> {
        if label_box.width == 0.0 || label_box.height == 0.0 {
            return Ok(());
        }
        let fill = self.main_background.map(Paint::solid);
        let stroke = self
            .line_color
            .map(|color| stroke(color, ISHIKAWA_LINE_STROKE_WIDTH));
        if fill.is_none() && stroke.is_none() {
            return Ok(());
        }
        self.add_path(
            id.to_string(),
            polygon_path(&[
                Point::new(label_box.x, label_box.y),
                Point::new(label_box.x + label_box.width, label_box.y),
                Point::new(
                    label_box.x + label_box.width,
                    label_box.y + label_box.height,
                ),
                Point::new(label_box.x, label_box.y + label_box.height),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill,
                stroke,
            },
        )
    }

    fn emit_text(&mut self, text: &IshikawaTextLayout) -> Result<()> {
        let Some(fill) = self.text_color else {
            return Ok(());
        };
        let presentation =
            ishikawa_text_presentation(&text.class_name, &text.anchor, text.font_size);
        let first_y = text.y - (text.lines.len().saturating_sub(1) as f64 * text.line_height) / 2.0;
        for (line_index, line) in text.lines.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let line = svg_plain_text(line);
            if line.is_empty() {
                continue;
            }
            let origin = Point::new(text.x, first_y + line_index as f64 * text.line_height);
            let bounds = self.measure_text_bounds(&line, origin, presentation)?;
            let mut font = self.font.clone();
            font.weight = presentation.font_weight;
            self.commands.push(DrawingCommand::DrawText {
                run: TextRun {
                    text: line,
                    origin,
                    bounds,
                    style: TextStyle {
                        font,
                        font_size: presentation.font_size,
                        letter_spacing: 0.0,
                        line_height: text.line_height,
                        fill: Paint::solid(fill),
                    },
                    anchor: text_anchor(presentation.anchor),
                    baseline: text_baseline(presentation.baseline),
                    direction: TextDirection::Auto,
                    language: None,
                    obligation: self.text_obligation.clone(),
                },
            });
        }
        Ok(())
    }

    fn measure_text_bounds(
        &self,
        text: &str,
        origin: Point,
        presentation: IshikawaTextPresentation,
    ) -> Result<Rect> {
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.font_family_css.clone()),
            font_size: presentation.font_size,
            font_weight: (presentation.font_weight != 400)
                .then(|| presentation.font_weight.to_string()),
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer.measure_svg_tspan_text_bbox_width_px(text, &measurement_style);
        let height = measurer.measure_svg_tspan_text_bbox_height_px(text, &measurement_style);
        self.session.checkpoint(OperationPhase::Emit)?;
        if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
            return Err(invalid("Ishikawa text measurement returned invalid bounds"));
        }
        let x = match presentation.anchor {
            IshikawaTextAnchor::Start => origin.x,
            IshikawaTextAnchor::Middle => origin.x - width / 2.0,
            IshikawaTextAnchor::End => origin.x - width,
        };
        let y = match presentation.baseline {
            IshikawaTextBaseline::Hanging => origin.y,
            IshikawaTextBaseline::Middle => origin.y - height / 2.0,
            IshikawaTextBaseline::Alphabetic => origin.y - height,
        };
        Ok(Rect::new(x, y, width, height))
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!("Ishikawa path `{id}` has no geometry")));
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

fn marker_start_path(
    line: &IshikawaLineLayout,
    stroke_width: f64,
) -> Result<Option<Vec<PathSegment>>> {
    let dx = line.x2 - line.x1;
    let dy = line.y2 - line.y1;
    let length = dx.hypot(dy);
    if length == 0.0 {
        return Ok(None);
    }
    if !length.is_finite() || !stroke_width.is_finite() || stroke_width < 0.0 {
        return Err(invalid("Ishikawa marker geometry is invalid"));
    }

    // SVG markers default to `markerUnits="strokeWidth"`. The marker viewport is therefore
    // scaled once by its viewBox ratio and again by the owning line's resolved stroke width.
    let scale = (ISHIKAWA_ARROW_MARKER_WIDTH / ISHIKAWA_ARROW_VIEWBOX_WIDTH)
        .min(ISHIKAWA_ARROW_MARKER_HEIGHT / ISHIKAWA_ARROW_VIEWBOX_HEIGHT)
        * stroke_width;
    let unit_x = dx / length;
    let unit_y = dy / length;
    let transform = |x: f64, y: f64| {
        let local_x = (x - ISHIKAWA_ARROW_REF_X) * scale;
        let local_y = (y - ISHIKAWA_ARROW_REF_Y) * scale;
        Point::new(
            line.x1 + local_x * unit_x - local_y * unit_y,
            line.y1 + local_x * unit_y + local_y * unit_x,
        )
    };
    Ok(Some(polygon_path(&[
        transform(ISHIKAWA_ARROW_VIEWBOX_WIDTH, 0.0),
        transform(ISHIKAWA_ARROW_REF_X, ISHIKAWA_ARROW_REF_Y),
        transform(ISHIKAWA_ARROW_VIEWBOX_WIDTH, ISHIKAWA_ARROW_VIEWBOX_HEIGHT),
    ])))
}

fn text_anchor(anchor: IshikawaTextAnchor) -> TextAnchor {
    match anchor {
        IshikawaTextAnchor::Start => TextAnchor::Start,
        IshikawaTextAnchor::Middle => TextAnchor::Middle,
        IshikawaTextAnchor::End => TextAnchor::End,
    }
}

fn text_baseline(baseline: IshikawaTextBaseline) -> TextBaseline {
    match baseline {
        IshikawaTextBaseline::Alphabetic => TextBaseline::Alphabetic,
        IshikawaTextBaseline::Hanging => TextBaseline::Hanging,
        IshikawaTextBaseline::Middle => TextBaseline::Middle,
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

fn visible_text(value: &str) -> Option<String> {
    let value = svg_plain_text(value);
    (!value.is_empty()).then_some(value)
}

fn validate_layout(
    layout: &IshikawaDiagramLayout,
    model: &IshikawaDiagramRenderModel,
) -> Result<()> {
    if ![
        layout.total_width,
        layout.total_height,
        layout.viewbox_x,
        layout.viewbox_y,
        layout.padding,
        layout.font_size,
    ]
    .into_iter()
    .all(f64::is_finite)
        || layout.total_width < 0.0
        || layout.total_height < 0.0
        || layout.padding < 0.0
        || layout.font_size <= 0.0
    {
        return Err(invalid("Ishikawa root geometry is invalid"));
    }
    if let Some(bounds) = &layout.bounds {
        validate_bounds(bounds, "root bounds")?;
    }

    if model.root.is_some() != layout.head.is_some()
        || layout.head.is_some() != layout.spine.is_some()
    {
        return Err(invalid(
            "Ishikawa semantic root, head, and spine do not agree",
        ));
    }
    let expected_pairs = model
        .root
        .as_ref()
        .map_or(0, |root| root.children.len().div_ceil(2));
    if layout.pairs.len() != expected_pairs {
        return Err(invalid(
            "Ishikawa semantic causes and visual branch pairs do not agree",
        ));
    }

    if let Some(spine) = &layout.spine {
        validate_line(spine, "ishikawa-spine")?;
    }
    if let Some(head) = &layout.head {
        if ![head.x, head.y, head.width, head.height]
            .into_iter()
            .all(f64::is_finite)
            || head.width < 0.0
            || head.height < 0.0
            || head.path_d.trim().is_empty()
        {
            return Err(invalid("Ishikawa head geometry is invalid"));
        }
        validate_text(&head.label, &["ishikawa-head-label"])?;
    }
    for pair in &layout.pairs {
        validate_branch(&pair.upper)?;
        if let Some(lower) = &pair.lower {
            validate_branch(lower)?;
        }
    }
    Ok(())
}

fn validate_branch(branch: &IshikawaBranchLayout) -> Result<()> {
    validate_line(&branch.line, "ishikawa-branch")?;
    validate_label_box(&branch.label_group.label_box)?;
    validate_text(&branch.label_group.label, &["ishikawa-label cause"])?;
    for subgroup in &branch.sub_groups {
        validate_line(&subgroup.line, "ishikawa-sub-branch")?;
        validate_text(
            &subgroup.label,
            &[
                "ishikawa-label align",
                "ishikawa-label up",
                "ishikawa-label down",
            ],
        )?;
    }
    Ok(())
}

fn validate_line(line: &IshikawaLineLayout, expected_class: &str) -> Result<()> {
    if ![line.x1, line.y1, line.x2, line.y2]
        .into_iter()
        .all(f64::is_finite)
        || line.class_name != expected_class
    {
        return Err(invalid(format!(
            "Ishikawa `{expected_class}` line geometry or presentation is invalid"
        )));
    }
    Ok(())
}

fn validate_label_box(label_box: &IshikawaLabelBoxLayout) -> Result<()> {
    if ![label_box.x, label_box.y, label_box.width, label_box.height]
        .into_iter()
        .all(f64::is_finite)
        || label_box.width < 0.0
        || label_box.height < 0.0
    {
        return Err(invalid("Ishikawa label box geometry is invalid"));
    }
    Ok(())
}

fn validate_text(text: &IshikawaTextLayout, expected_classes: &[&str]) -> Result<()> {
    if ![
        text.x,
        text.y,
        text.line_height,
        text.font_size,
        text.bbox.min_x,
        text.bbox.min_y,
        text.bbox.max_x,
        text.bbox.max_y,
    ]
    .into_iter()
    .all(f64::is_finite)
        || text.line_height <= 0.0
        || text.font_size <= 0.0
        || text.bbox.max_x < text.bbox.min_x
        || text.bbox.max_y < text.bbox.min_y
        || !matches!(text.anchor.as_str(), "start" | "middle" | "end")
        || !expected_classes.contains(&text.class_name.as_str())
    {
        return Err(invalid("Ishikawa text geometry or presentation is invalid"));
    }
    Ok(())
}

fn validate_bounds(bounds: &Bounds, label: &str) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid(format!("Ishikawa {label} are invalid")));
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
        family: "ishikawa".to_string(),
        reason: message.into(),
    }
}
