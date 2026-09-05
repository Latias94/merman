//! Renderer-neutral C4 diagram adapter.
//!
//! C4 layout owns the final shape/boundary/relationship rectangles and text blocks.  The
//! DrawingList projection reuses that geometry and emits only portable vector primitives; SVG
//! symbol sprites, arbitrary legends, shadows, and custom embedded graphics remain explicit
//! capability errors rather than being silently dropped.

use super::{
    C4SvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    theme_color,
};
use crate::c4::{
    C4ConfigView, C4NodeShape, C4PaintItem, c4_node_shape, c4_paint_order, c4_visible_text,
};
use crate::config::{config_f64_explicit_css_px, config_string};
use crate::drawing_list::support::{
    navigation_security, portable_navigation_uri, stroke, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, C4BoundaryLayout, C4DiagramLayout, C4RelLayout, C4ShapeLayout};
use crate::text::{TextMeasurer, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::c4::{C4DiagramRenderModel, C4ShapeRenderModel};
use merman_core::svg_security::MermaidNavigationSecurity;
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

type C4Pair = FamilyPair<C4DiagramRenderModel, C4DiagramLayout>;

pub(crate) fn build_c4_document(
    pair: &C4Pair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let mut builder = C4Builder::new(pair, metadata, policy, session)?;
    builder.build()
}

struct C4Builder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a C4DiagramRenderModel,
    layout: &'a C4DiagramLayout,
    shape_layouts: HashMap<&'a str, &'a C4ShapeLayout>,
    boundary_layouts: HashMap<&'a str, &'a C4BoundaryLayout>,
    shapes_by_alias: HashMap<&'a str, &'a C4ShapeRenderModel>,
    font: FontDescriptor,
    font_size: f64,
    text_obligation: TextObligation,
    navigation_security: MermaidNavigationSecurity,
    boundary_text: Color,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
}

impl<'a> C4Builder<'a> {
    fn new(
        pair: &'a C4Pair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let model = pair.semantic();
        let layout = pair.layout();
        let config = metadata.effective_config.as_value();
        let navigation_security = navigation_security(config);
        let look = crate::config::config_diagram_look(config);
        if look.as_str().eq_ignore_ascii_case("handDrawn") {
            return Err(unavailable(
                "hand-drawn C4 output has no stable vector equivalent in DrawingList v1",
            ));
        }
        if config
            .get("themeCSS")
            .and_then(Value::as_str)
            .is_some_and(|css| !css.trim().is_empty())
        {
            return Err(unavailable(
                "themeCSS is an unresolved SVG cascade input for C4 DrawingList output",
            ));
        }
        let bounds = layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("C4 layout did not provide root bounds"))?;
        validate_bounds(bounds)?;

        let font_size = config_f64_explicit_css_px(config, &["themeVariables", "fontSize"])
            .unwrap_or(14.0)
            .max(1.0);
        let font_family = config_string(config, &["fontFamily"])
            .or_else(|| config_string(config, &["themeVariables", "fontFamily"]))
            .unwrap_or_else(|| crate::c4::C4_DEFAULT_FONT_FAMILY.to_string());
        let font = FontDescriptor {
            families: parse_font_families(font_family),
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let boundary_text = theme_color(config, "textColor", "#444444")?;
        let shape_layouts = unique_shape_layouts(layout)?;
        let boundary_layouts = unique_boundary_layouts(layout)?;
        if layout.rels.len() != model.rels.len() {
            return Err(invalid(format!(
                "C4 layout has {} relationships for {} semantic relationships",
                layout.rels.len(),
                model.rels.len()
            )));
        }
        let shapes_by_alias = model
            .shapes
            .iter()
            .map(|shape| (shape.alias.as_str(), shape))
            .collect::<HashMap<_, _>>();
        let builder = Self {
            metadata,
            session,
            policy,
            model,
            layout,
            shape_layouts,
            boundary_layouts,
            shapes_by_alias,
            font,
            font_size,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            navigation_security,
            boundary_text,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "c4.document".to_string(),
                },
            ],
            semantics: vec![SemanticAnnotation {
                id: "c4.document".to_string(),
                role: SemanticRole::Document,
                title: model
                    .acc_title
                    .clone()
                    .or_else(|| model.title.clone())
                    .or_else(|| metadata.title.clone())
                    .or_else(|| Some(metadata.diagram_type.clone())),
                description: model.acc_descr.clone(),
                link: None,
            }],
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
        };
        builder.preflight()?;
        Ok(builder)
    }

    fn build(&mut self) -> Result<RenderDocument> {
        for item in c4_paint_order(self.layout)? {
            self.session.checkpoint(OperationPhase::Emit)?;
            match item {
                C4PaintItem::Shape(index) => self.emit_shape(index, &self.layout.shapes[index])?,
                C4PaintItem::Boundary(index) => {
                    self.emit_boundary(index, &self.layout.boundaries[index])?
                }
            }
        }
        for (index, relation) in self.layout.rels.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_relation(index, relation)?;
        }

        if let Some(title) = self.diagram_title() {
            self.emit_title(&title)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
        let config = C4ConfigView::new(self.metadata.effective_config.as_value());
        let settings = config.layout_settings();
        let padding_x = settings.diagram_margin_x.max(0.0);
        let padding_y = settings.diagram_margin_y.max(0.0);
        let title_extra = self
            .diagram_title()
            .is_some()
            .then_some(60.0)
            .unwrap_or(0.0);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                bounds.min_x - padding_x,
                -(padding_y + title_extra),
                bounds.max_x - bounds.min_x + 2.0 * padding_x,
                bounds.max_y - bounds.min_y + 2.0 * padding_y + title_extra,
            )),
            policy: self.policy,
            resources: std::mem::take(&mut self.resources),
            commands: std::mem::take(&mut self.commands),
            semantics: std::mem::take(&mut self.semantics),
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-c4".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "c4_type": self.model.c4_type,
                    "geometry_subset": "boundaries-shapes-relations-arrows",
                    "label_mode": "plain_host_text",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::C4,
                body: SvgStructureBody::C4(C4SvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    acc_title: self
                        .model
                        .acc_title
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned),
                    acc_description: self
                        .model
                        .acc_descr
                        .as_deref()
                        .map(|value| value.trim_end_matches('\n'))
                        .filter(|value| !value.trim().is_empty())
                        .map(ToOwned::to_owned),
                    semantic_classes: std::mem::take(&mut self.semantic_classes),
                    path_classes: std::mem::take(&mut self.path_classes),
                    text_classes: std::mem::take(&mut self.text_classes),
                    dom_ids: std::mem::take(&mut self.dom_ids),
                }),
            },
        })
    }

    fn diagram_title(&self) -> Option<String> {
        self.metadata
            .title
            .as_deref()
            .or(self.layout.title.as_deref())
            .or(self.model.title.as_deref())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned)
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        let title = plain_text(title).map_err(unavailable)?;
        if title.trim().is_empty() {
            return Ok(());
        }
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("C4 layout did not provide root bounds"))?;
        let config = C4ConfigView::new(self.metadata.effective_config.as_value());
        let settings = config.layout_settings();
        let padding_x = settings.diagram_margin_x.max(0.0);
        let origin = Point::new(
            (bounds.max_x - bounds.min_x).max(1.0) / 2.0 - 4.0 * padding_x,
            bounds.min_y + settings.diagram_margin_y.max(0.0),
        );
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.font.families.join(", ")),
            font_size: self.font_size,
            font_weight: Some(self.font.weight.to_string()),
            font_style: Some("normal".to_string()),
        };
        let measurement = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit)
            .measure(&title, &measurement_style);
        let text_width = measurement.width.max(1.0);
        let text_height = measurement.height.max(1.0);
        self.draw_text(
            &title,
            origin,
            Rect::new(origin.x, origin.y - text_height, text_width, text_height),
            self.boundary_text,
            self.font.clone(),
            self.font_size,
            self.font_size * 1.25,
            TextAnchor::Start,
            TextBaseline::Alphabetic,
        );
        Ok(())
    }

    fn preflight(&self) -> Result<()> {
        if self.model.shapes.is_empty() && self.model.boundaries.is_empty() {
            return Err(invalid("C4 diagram has no shapes or boundaries"));
        }
        for shape in &self.model.shapes {
            validate_c4_metadata(
                shape.alias.as_str(),
                shape.sprite.as_ref(),
                shape.tags.as_ref(),
                shape.link.as_ref(),
                shape.shadowing.as_ref(),
                shape.legend_text.as_ref(),
                shape.legend_sprite.as_ref(),
                shape.shape.as_ref(),
            )?;
            validate_text(
                &shape.label.as_str(),
                &format!("C4 shape `{}` label", shape.alias),
            )?;
            validate_text(
                &shape.type_c4_shape.as_str(),
                &format!("C4 shape `{}` type", shape.alias),
            )?;
            for (name, value) in [
                ("type", shape.ty.as_ref()),
                ("technology", shape.techn.as_ref()),
                ("description", shape.descr.as_ref()),
            ] {
                if let Some(value) = value {
                    validate_text(
                        &value.as_str(),
                        &format!("C4 shape `{}` {name}", shape.alias),
                    )?;
                }
            }
            if !self.shape_layouts.contains_key(shape.alias.as_str()) {
                return Err(invalid(format!("C4 shape `{}` has no layout", shape.alias)));
            }
        }
        for boundary in &self.model.boundaries {
            validate_c4_metadata(
                boundary.alias.as_str(),
                boundary.sprite.as_ref(),
                boundary.tags.as_ref(),
                boundary.link.as_ref(),
                boundary.shadowing.as_ref(),
                boundary.legend_text.as_ref(),
                boundary.legend_sprite.as_ref(),
                boundary.shape.as_ref(),
            )?;
            validate_text(
                &boundary.label.as_str(),
                &format!("C4 boundary `{}` label", boundary.alias),
            )?;
            if let Some(value) = boundary.ty.as_ref() {
                validate_text(
                    &value.as_str(),
                    &format!("C4 boundary `{}` type", boundary.alias),
                )?;
            }
            if let Some(value) = boundary.descr.as_ref() {
                validate_text(
                    &value.as_str(),
                    &format!("C4 boundary `{}` description", boundary.alias),
                )?;
            }
            if !self.boundary_layouts.contains_key(boundary.alias.as_str()) {
                return Err(invalid(format!(
                    "C4 boundary `{}` has no layout",
                    boundary.alias
                )));
            }
        }
        for (index, relation) in self.model.rels.iter().enumerate() {
            validate_text(
                &relation.label.as_str(),
                &format!("C4 relationship {index} label"),
            )?;
            for (name, value) in [
                ("technology", relation.techn.as_ref()),
                ("description", relation.descr.as_ref()),
            ] {
                if let Some(value) = value {
                    validate_text(&value.as_str(), &format!("C4 relationship {index} {name}"))?;
                }
            }
            if !matches!(
                relation.rel_type.as_str(),
                "rel" | "rel_a" | "rel_b" | "birel"
            ) {
                return Err(unavailable(format!(
                    "C4 relationship {} uses unsupported type `{}`",
                    index, relation.rel_type
                )));
            }
        }
        for shape in self.layout.shapes.iter() {
            validate_box(
                shape.x,
                shape.y,
                shape.width,
                shape.height,
                &format!("C4 shape `{}`", shape.alias),
            )?;
        }
        for boundary in self.layout.boundaries.iter() {
            validate_box(
                boundary.x,
                boundary.y,
                boundary.width,
                boundary.height,
                &format!("C4 boundary `{}`", boundary.alias),
            )?;
        }
        for (index, relation) in self.layout.rels.iter().enumerate() {
            if !relation.start_point.x.is_finite()
                || !relation.start_point.y.is_finite()
                || !relation.end_point.x.is_finite()
                || !relation.end_point.y.is_finite()
            {
                return Err(invalid(format!(
                    "C4 relationship {index} has invalid route points"
                )));
            }
        }
        Ok(())
    }

    fn emit_boundary(&mut self, index: usize, boundary: &C4BoundaryLayout) -> Result<()> {
        let meta = self
            .model
            .boundaries
            .iter()
            .find(|candidate| candidate.alias == boundary.alias)
            .ok_or_else(|| {
                invalid(format!(
                    "C4 boundary `{}` has no semantic model",
                    boundary.alias
                ))
            })?;
        let semantic_id = format!("c4.boundary.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let bounds = Rect::new(boundary.x, boundary.y, boundary.width, boundary.height);
        let fill = match meta
            .bg_color
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
        {
            Some(value) => Some(Paint::solid(parse_color(value)?)),
            None => None,
        };
        let border =
            parse_optional_color(meta.border_color.as_deref())?.unwrap_or(parse_color("#444444")?);
        self.add_path(
            format!("{semantic_id}.box"),
            rounded_rect_path(bounds, 2.5),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill,
                stroke: Some(StrokeStyleWithDash::new(
                    border,
                    1.0,
                    meta.node_type.is_none(),
                )),
            },
        )?;
        let config = C4ConfigView::new(self.metadata.effective_config.as_value());
        let boundary_style = config.boundary_font();
        let boundary_color = parse_color("#444444")?;
        let title = plain_text(&boundary.label.text).map_err(unavailable)?;
        if !title.trim().is_empty() {
            let mut title_font = font_descriptor(&boundary_style, 700, FontStyle::Normal);
            title_font.weight = 700;
            self.draw_centered_lines(
                &title,
                Point::new(
                    boundary.x + boundary.width / 2.0,
                    boundary.y + boundary.label.y,
                ),
                boundary.label.width,
                boundary.label.height,
                boundary_color,
                title_font,
                (boundary_style.font_size + 2.0).max(1.0),
                (boundary_style.font_size + 2.0).max(1.0),
            );
        }
        if let Some(block) = boundary
            .ty
            .as_ref()
            .filter(|block| !block.text.trim().is_empty())
        {
            let value = plain_text(&block.text).map_err(unavailable)?;
            self.draw_centered_lines(
                &value,
                Point::new(boundary.x + boundary.width / 2.0, boundary.y + block.y),
                block.width,
                block.height,
                boundary_color,
                font_descriptor(&boundary_style, 400, FontStyle::Normal),
                boundary_style.font_size.max(1.0),
                boundary_style.font_size.max(1.0),
            );
        }
        if let Some(block) = boundary
            .descr
            .as_ref()
            .filter(|block| !block.text.trim().is_empty())
        {
            let value = plain_text(&block.text).map_err(unavailable)?;
            let font_size = (boundary_style.font_size - 2.0).max(1.0);
            self.draw_centered_lines(
                &value,
                Point::new(boundary.x + boundary.width / 2.0, boundary.y + block.y),
                block.width,
                block.height,
                boundary_color,
                font_descriptor(&boundary_style, 400, FontStyle::Normal),
                font_size,
                font_size,
            );
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(title),
            description: meta
                .descr
                .as_ref()
                .map(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| Some(format!("C4 boundary {}", boundary.alias))),
            link: value_link(meta.link.as_ref(), self.navigation_security)?,
        });
        Ok(())
    }

    fn emit_shape(&mut self, index: usize, shape: &C4ShapeLayout) -> Result<()> {
        let meta = self
            .shapes_by_alias
            .get(shape.alias.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("C4 shape `{}` has no semantic model", shape.alias)))?;
        let semantic_id = format!("c4.shape.{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), c4_shape_classes(meta));
        self.dom_ids
            .insert(semantic_id.clone(), shape.alias.clone());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let node_shape = c4_node_shape(meta);
        let (default_fill, default_stroke) = if shape.type_c4_shape.starts_with("external_") {
            ("#999999", "#8A8A8A")
        } else {
            ("#08427B", "#073B6F")
        };
        let config = C4ConfigView::new(self.metadata.effective_config.as_value());
        let fill = parse_optional_color(meta.bg_color.as_deref())?.unwrap_or(parse_color(
            &config.color(&format!("{}_bg_color", shape.type_c4_shape), default_fill),
        )?);
        let border = parse_optional_color(meta.border_color.as_deref())?.unwrap_or(parse_color(
            &config.color(
                &format!("{}_border_color", shape.type_c4_shape),
                default_stroke,
            ),
        )?);
        let text = parse_optional_color(meta.font_color.as_deref())?
            .unwrap_or(Color::rgba(255, 255, 255, 255));
        self.emit_shape_geometry(&semantic_id, shape, node_shape, fill, border)?;

        let shape_style = config.shape_font(&shape.type_c4_shape);
        let base_weight = parse_font_weight(shape_style.font_weight.as_deref(), 400);
        let mut blocks = Vec::new();
        if !shape.label.text.trim().is_empty() {
            blocks.push((
                "name",
                "c4-name",
                &shape.label,
                700_u16,
                shape_style.font_size,
            ));
        }
        blocks.push((
            "type",
            "c4-type",
            &shape.type_block,
            base_weight,
            shape_style.font_size * 0.75,
        ));
        if let Some(descr) = shape.descr.as_ref()
            && !descr.text.trim().is_empty()
        {
            blocks.push((
                "description",
                "c4-descr",
                descr,
                base_weight,
                shape_style.font_size * 0.82,
            ));
        }
        let total_width = blocks
            .iter()
            .map(|(_, _, block, _, _)| block.width)
            .fold(0.0, f64::max);
        let total_height = blocks
            .iter()
            .map(|(_, _, block, _, _)| block.height)
            .sum::<f64>()
            + 3.0 * blocks.len().saturating_sub(1) as f64;
        let padding = config.layout_settings().c4_shape_padding;
        let center = Point::new(shape.x + shape.width / 2.0, shape.y + shape.height / 2.0);
        let (label_left_x, label_top_y) = shape_label_origin(
            shape,
            node_shape,
            center,
            total_width,
            total_height,
            padding,
        );
        let label_id = format!("{semantic_id}.label");
        self.semantic_classes
            .insert(label_id.clone(), "label".to_string());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: label_id.clone(),
        });
        let mut label_text = Vec::with_capacity(blocks.len());
        let mut section_y = 0.0;
        for (name, class, block, weight, font_size) in blocks {
            let text_value = c4_block_text(block)?;
            label_text.push(text_value.clone());
            let section_id = format!("{semantic_id}.{name}");
            self.semantic_classes
                .insert(section_id.clone(), class.to_string());
            self.text_classes
                .insert(section_id.clone(), class.to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: section_id.clone(),
            });
            self.draw_centered_lines(
                &text_value,
                Point::new(
                    c4_block_center_x(label_left_x, total_width, block),
                    label_top_y + section_y + block.height / 2.0,
                ),
                block.width,
                block.height,
                text,
                FontDescriptor {
                    families: parse_font_families(
                        shape_style
                            .font_family
                            .clone()
                            .unwrap_or_else(|| crate::c4::C4_DEFAULT_FONT_FAMILY.to_string()),
                    ),
                    weight,
                    style: FontStyle::Normal,
                    postscript_name: None,
                    resource: None,
                },
                font_size.max(1.0),
                (font_size * 1.1).max(1.0),
            );
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: section_id,
                role: SemanticRole::Label,
                title: None,
                description: None,
                link: None,
            });
            section_y += block.height + 3.0;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: label_id,
            role: SemanticRole::Label,
            title: Some(label_text.join("\n")),
            description: None,
            link: None,
        });
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(plain_text(&meta.label.as_str()).map_err(unavailable)?),
            description: meta
                .descr
                .as_ref()
                .map(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| Some(format!("C4 {}", shape.alias))),
            link: value_link(meta.link.as_ref(), self.navigation_security)?,
        });
        Ok(())
    }

    fn emit_relation(&mut self, index: usize, relation: &C4RelLayout) -> Result<()> {
        let meta =
            self.model.rels.get(index).ok_or_else(|| {
                invalid(format!("C4 relationship {} has no semantic model", index))
            })?;
        if meta.from_alias != relation.from || meta.to_alias != relation.to {
            return Err(invalid(format!(
                "C4 relationship {index} layout/model order diverged"
            )));
        }
        let semantic_id = format!("c4.relation.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let start = Point::new(relation.start_point.x, relation.start_point.y);
        let end = Point::new(relation.end_point.x, relation.end_point.y);
        let (segments, start_tangent, end_tangent) = if index == 0 {
            let tangent = Point::new(end.x - start.x, end.y - start.y);
            (
                vec![
                    PathSegment::MoveTo { to: start },
                    PathSegment::LineTo { to: end },
                ],
                tangent,
                tangent,
            )
        } else {
            let control = Point::new(
                start.x + (end.x - start.x) / 4.0,
                start.y + (end.y - start.y) / 2.0,
            );
            (
                vec![
                    PathSegment::MoveTo { to: start },
                    PathSegment::QuadTo { control, to: end },
                ],
                Point::new(control.x - start.x, control.y - start.y),
                Point::new(end.x - control.x, end.y - control.y),
            )
        };
        let color =
            parse_optional_color(meta.line_color.as_deref())?.unwrap_or(parse_color("#444444")?);
        self.add_path(
            format!("{semantic_id}.route"),
            segments,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(color, 1.0)),
            },
        )?;
        if meta.rel_type != "rel_b" {
            self.add_path(
                format!("{semantic_id}.marker.end"),
                marker_path(end, end_tangent, false)?,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(Color::rgba(0, 0, 0, 255))),
                    stroke: None,
                },
            )?;
        }
        if meta.rel_type == "rel_b" || meta.rel_type == "birel" {
            self.add_path(
                format!("{semantic_id}.marker.start"),
                marker_path(start, start_tangent, true)?,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(Color::rgba(0, 0, 0, 255))),
                    stroke: None,
                },
            )?;
        }
        let offset = Point::new(
            relation.offset_x.unwrap_or(0) as f64,
            relation.offset_y.unwrap_or(0) as f64,
        );
        let mid = Point::new(
            (start.x + end.x) / 2.0 + offset.x,
            (start.y + end.y) / 2.0 + offset.y,
        );
        let config = C4ConfigView::new(self.metadata.effective_config.as_value());
        let message_style = config.message_font();
        let message_size = message_style.font_size.max(1.0);
        let message_font = font_descriptor(&message_style, 400, FontStyle::Normal);
        let text_color =
            parse_optional_color(meta.text_color.as_deref())?.unwrap_or(parse_color("#444444")?);
        let label = plain_text(&relation.label.text).map_err(unavailable)?;
        self.draw_centered_lines(
            &label,
            mid,
            relation.label.width,
            relation.label.height,
            text_color,
            message_font.clone(),
            message_size,
            message_size,
        );
        if let Some(techn) = relation.techn.as_ref()
            && !techn.text.trim().is_empty()
        {
            let text_value = format!("[{}]", plain_text(&techn.text).map_err(unavailable)?);
            self.draw_centered_lines(
                &text_value,
                Point::new(mid.x, mid.y + message_size + 5.0),
                relation.label.width.max(techn.width),
                techn.height,
                text_color,
                FontDescriptor {
                    style: FontStyle::Italic,
                    ..message_font
                },
                message_size,
                message_size,
            );
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: Some(plain_text(&meta.label.as_str()).map_err(unavailable)?),
            description: meta
                .descr
                .as_ref()
                .map(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(|description| format!("{} → {}: {description}", relation.from, relation.to))
                .or_else(|| Some(format!("{} → {}", relation.from, relation.to))),
            link: value_link(meta.link.as_ref(), self.navigation_security)?,
        });
        Ok(())
    }

    fn emit_shape_geometry(
        &mut self,
        semantic_id: &str,
        shape: &C4ShapeLayout,
        node_shape: C4NodeShape,
        fill: Color,
        border: Color,
    ) -> Result<()> {
        let bounds = Rect::new(shape.x, shape.y, shape.width, shape.height);
        let style = || PathStyle {
            fill_rule: FillRule::NonZero,
            fill: Some(Paint::solid(fill)),
            stroke: Some(stroke(border, 2.0)),
        };
        let add = |builder: &mut Self, suffix: &str, class: &str, segments: Vec<PathSegment>| {
            let id = format!("{semantic_id}.{suffix}");
            builder.path_classes.insert(id.clone(), class.to_string());
            builder.add_path(id, segments, style())
        };

        match node_shape {
            C4NodeShape::Rounded => add(
                self,
                "shape",
                "basic label-container",
                rounded_rect_path(bounds, 12.0),
            ),
            C4NodeShape::Framed => add(self, "shape", "label-container", framed_rect_path(bounds)),
            C4NodeShape::Person => {
                let radius = (shape.width * 0.23).clamp(16.0, 56.0);
                let overlap = radius * 0.27;
                let body_height = (shape.height - (2.0 * radius - overlap)).max(1.0);
                let body_top = shape.y + 2.0 * radius - overlap;
                let body_radius = (shape.width * 0.177).min(body_height * 0.45);
                add(
                    self,
                    "body",
                    "basic label-container",
                    rounded_rect_path(
                        Rect::new(shape.x, body_top, shape.width, body_height),
                        body_radius,
                    ),
                )?;
                add(
                    self,
                    "head",
                    "basic label-container",
                    circle_path(
                        Point::new(shape.x + shape.width / 2.0, shape.y + radius),
                        radius,
                    ),
                )
            }
            C4NodeShape::Cylinder => add(
                self,
                "shape",
                "basic label-container outer-path",
                cylinder_shape_path(bounds),
            ),
            C4NodeShape::HorizontalCylinder => add(
                self,
                "shape",
                "basic label-container outer-path",
                horizontal_cylinder_shape_path(bounds),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_centered_lines(
        &mut self,
        text: &str,
        center: Point,
        width: f64,
        height: f64,
        fill: Color,
        font: FontDescriptor,
        font_size: f64,
        line_height: f64,
    ) {
        let lines = text.split('\n').collect::<Vec<_>>();
        let line_count = lines.len().max(1) as f64;
        let row_height = (height / line_count).max(font_size).max(1.0);
        for (index, line) in lines.into_iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let y = center.y + (index as f64 - (line_count - 1.0) / 2.0) * line_height;
            self.draw_text(
                line,
                Point::new(center.x, y),
                Rect::new(
                    center.x - width.max(1.0) / 2.0,
                    y - row_height / 2.0,
                    width.max(1.0),
                    row_height,
                ),
                fill,
                font.clone(),
                font_size,
                line_height,
                TextAnchor::Middle,
                TextBaseline::Middle,
            );
        }
    }

    fn draw_text(
        &mut self,
        text: &str,
        origin: Point,
        bounds: Rect,
        fill: Color,
        font: FontDescriptor,
        font_size: f64,
        line_height: f64,
        anchor: TextAnchor,
        baseline: TextBaseline,
    ) {
        self.commands.push(DrawingCommand::DrawText {
            run: TextRun {
                text: text.to_string(),
                origin,
                bounds,
                style: TextStyle {
                    font,
                    font_size,
                    letter_spacing: 0.0,
                    line_height,
                    fill: Paint::solid(fill),
                },
                anchor,
                baseline,
                direction: TextDirection::Auto,
                language: None,
                obligation: self.text_obligation.clone(),
            },
        });
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        self.resources.push(DrawingResource::Path(PathResource {
            id: ResourceId::new(id.clone()),
            segments,
        }));
        self.commands.push(DrawingCommand::DrawPath {
            path: ResourceId::new(id),
            style,
        });
        Ok(())
    }
}

struct StrokeStyleWithDash;

impl StrokeStyleWithDash {
    fn new(color: Color, width: f64, dashed: bool) -> merman_display_list::StrokeStyle {
        let mut result = stroke(color, width);
        if dashed {
            result.dash_array = vec![7.0, 7.0];
        }
        result
    }
}

fn framed_rect_path(bounds: Rect) -> Vec<PathSegment> {
    let left = bounds.x;
    let right = bounds.x + bounds.width;
    let top = bounds.y;
    let bottom = bounds.y + bounds.height;
    let points = [
        Point::new(left, bottom),
        Point::new(right - 16.0, bottom),
        Point::new(right - 16.0, top),
        Point::new(left, top),
        Point::new(left, bottom),
        Point::new(left - 8.0, bottom),
        Point::new(right - 8.0, bottom),
        Point::new(right - 8.0, top),
        Point::new(left - 8.0, top),
        Point::new(left - 8.0, bottom),
    ];
    polygon_path(&points)
}

fn cylinder_shape_path(bounds: Rect) -> Vec<PathSegment> {
    let rx = bounds.width / 2.0;
    let ry = rx / (2.5 + bounds.width / 50.0);
    let body_height = (bounds.height - 2.0 * ry).max(1.0);
    let left = bounds.x;
    let right = bounds.x + bounds.width;
    let top_center = bounds.y + ry;
    let bottom_center = top_center + body_height;
    vec![
        PathSegment::MoveTo {
            to: Point::new(left, top_center),
        },
        PathSegment::ArcTo {
            radius_x: rx,
            radius_y: ry,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: false,
            to: Point::new(right, top_center),
        },
        PathSegment::ArcTo {
            radius_x: rx,
            radius_y: ry,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: false,
            to: Point::new(left, top_center),
        },
        PathSegment::LineTo {
            to: Point::new(left, bottom_center),
        },
        PathSegment::ArcTo {
            radius_x: rx,
            radius_y: ry,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: false,
            to: Point::new(right, bottom_center),
        },
        PathSegment::LineTo {
            to: Point::new(right, top_center),
        },
    ]
}

fn horizontal_cylinder_shape_path(bounds: Rect) -> Vec<PathSegment> {
    let ry = bounds.height / 2.0;
    let rx = ry / (2.5 + bounds.height / 50.0);
    let left = bounds.x;
    let right = bounds.x + bounds.width;
    let top = bounds.y;
    let bottom = bounds.y + bounds.height;
    vec![
        PathSegment::MoveTo {
            to: Point::new(left, bottom),
        },
        PathSegment::ArcTo {
            radius_x: rx,
            radius_y: ry,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(left, top),
        },
        PathSegment::LineTo {
            to: Point::new(right, top),
        },
        PathSegment::ArcTo {
            radius_x: rx,
            radius_y: ry,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(right, bottom),
        },
        PathSegment::MoveTo {
            to: Point::new(right, top),
        },
        PathSegment::ArcTo {
            radius_x: rx,
            radius_y: ry,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: false,
            to: Point::new(right, bottom),
        },
        PathSegment::LineTo {
            to: Point::new(left, bottom),
        },
    ]
}

fn circle_path(center: Point, radius: f64) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo {
            to: Point::new(center.x + radius, center.y),
        },
        PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(center.x - radius, center.y),
        },
        PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(center.x + radius, center.y),
        },
        PathSegment::Close,
    ]
}

fn marker_path(origin: Point, tangent: Point, start: bool) -> Result<Vec<PathSegment>> {
    let length = tangent.x.hypot(tangent.y);
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(invalid("C4 relationship marker has no tangent"));
    }
    let ux = tangent.x / length;
    let uy = tangent.y / length;
    let (points, reference_x) = if start {
        ([(10.0, 0.0), (0.0, 5.0), (10.0, 10.0)], 1.0)
    } else {
        ([(0.0, 0.0), (10.0, 5.0), (0.0, 10.0)], 9.0)
    };
    let transformed = points.map(|(x, y)| {
        let local_x = x - reference_x;
        let local_y = y - 5.0;
        Point::new(
            origin.x + ux * local_x - uy * local_y,
            origin.y + uy * local_x + ux * local_y,
        )
    });
    Ok(polygon_path(&transformed))
}

fn polygon_path(points: &[Point]) -> Vec<PathSegment> {
    let Some(first) = points.first().copied() else {
        return Vec::new();
    };
    let mut segments = Vec::with_capacity(points.len() + 1);
    segments.push(PathSegment::MoveTo { to: first });
    segments.extend(
        points
            .iter()
            .skip(1)
            .copied()
            .map(|to| PathSegment::LineTo { to }),
    );
    segments.push(PathSegment::Close);
    segments
}

fn c4_shape_classes(shape: &C4ShapeRenderModel) -> String {
    let mut classes = format!("node c4-shape c4-{}", shape.type_c4_shape.as_str());
    if shape.type_c4_shape.as_str().starts_with("external_") {
        classes.push_str(" c4-external");
    }
    classes
}

fn c4_block_text(block: &crate::model::C4TextBlockLayout) -> Result<String> {
    if let Some(plan) = block.render_plan.as_ref() {
        return Ok(c4_visible_text(&plan.rows));
    }
    plain_text(&block.text).map_err(unavailable)
}

fn shape_label_origin(
    shape: &C4ShapeLayout,
    node_shape: C4NodeShape,
    center: Point,
    total_width: f64,
    total_height: f64,
    padding: f64,
) -> (f64, f64) {
    match node_shape {
        C4NodeShape::Person => {
            let radius = (shape.width * 0.23).clamp(16.0, 56.0);
            let overlap = radius * 0.27;
            let body_height = (shape.height - (2.0 * radius - overlap)).max(1.0);
            let body_top = -shape.height / 2.0 + 2.0 * radius - overlap;
            (
                center.x - total_width / 2.0,
                center.y + body_top + body_height / 2.0 - total_height / 2.0,
            )
        }
        C4NodeShape::Cylinder => (
            center.x - total_width / 2.0,
            center.y - total_height / 2.0 + padding / 1.5,
        ),
        C4NodeShape::HorizontalCylinder => {
            let ry = shape.height / 2.0;
            let rx = ry / (2.5 + shape.height / 50.0);
            (
                center.x - total_width / 2.0 - rx,
                center.y - total_height / 2.0,
            )
        }
        C4NodeShape::Rounded | C4NodeShape::Framed => {
            (center.x - total_width / 2.0, center.y - total_height / 2.0)
        }
    }
}

fn c4_block_center_x(
    label_left_x: f64,
    total_width: f64,
    block: &crate::model::C4TextBlockLayout,
) -> f64 {
    let bbox_x = block.render_plan.as_ref().map_or(0.0, |plan| plan.bbox_x);
    label_left_x + total_width / 2.0 - bbox_x - block.width / 2.0
}

fn font_descriptor(
    style: &MeasurementTextStyle,
    fallback_weight: u16,
    font_style: FontStyle,
) -> FontDescriptor {
    FontDescriptor {
        families: parse_font_families(
            style
                .font_family
                .clone()
                .unwrap_or_else(|| crate::c4::C4_DEFAULT_FONT_FAMILY.to_string()),
        ),
        weight: parse_font_weight(style.font_weight.as_deref(), fallback_weight),
        style: font_style,
        postscript_name: None,
        resource: None,
    }
}

fn parse_font_weight(value: Option<&str>, fallback: u16) -> u16 {
    match value.map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("normal") => 400,
        Some(value) if value.eq_ignore_ascii_case("bold") => 700,
        Some(value) => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight))
            .unwrap_or(fallback),
        None => fallback,
    }
}

fn rounded_rect_path(bounds: Rect, radius: f64) -> Vec<PathSegment> {
    let radius = radius
        .min(bounds.width / 2.0)
        .min(bounds.height / 2.0)
        .max(0.0);
    if radius == 0.0 {
        return vec![
            PathSegment::MoveTo {
                to: Point::new(bounds.x, bounds.y),
            },
            PathSegment::LineTo {
                to: Point::new(bounds.x + bounds.width, bounds.y),
            },
            PathSegment::LineTo {
                to: Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
            },
            PathSegment::LineTo {
                to: Point::new(bounds.x, bounds.y + bounds.height),
            },
            PathSegment::Close,
        ];
    }
    vec![
        PathSegment::MoveTo {
            to: Point::new(bounds.x + radius, bounds.y),
        },
        PathSegment::LineTo {
            to: Point::new(bounds.x + bounds.width - radius, bounds.y),
        },
        PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(bounds.x + bounds.width, bounds.y + radius),
        },
        PathSegment::LineTo {
            to: Point::new(bounds.x + bounds.width, bounds.y + bounds.height - radius),
        },
        PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(bounds.x + bounds.width - radius, bounds.y + bounds.height),
        },
        PathSegment::LineTo {
            to: Point::new(bounds.x + radius, bounds.y + bounds.height),
        },
        PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(bounds.x, bounds.y + bounds.height - radius),
        },
        PathSegment::LineTo {
            to: Point::new(bounds.x, bounds.y + radius),
        },
        PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(bounds.x + radius, bounds.y),
        },
        PathSegment::Close,
    ]
}

fn plain_text(raw: &str) -> std::result::Result<String, String> {
    if raw.contains("**") || raw.contains("__") || contains_html_tag(raw) {
        return Err("contains styled Markdown or HTML markup".to_string());
    }
    Ok(crate::entities::decode_entities_minimal(raw)
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n"))
}

fn contains_html_tag(text: &str) -> bool {
    const TAGS: [&str; 14] = [
        "<a", "</a", "<b", "</b", "<div", "</div", "<em", "</em", "<i", "</i", "<p", "</p",
        "<span", "</span",
    ];
    let lower = text.to_ascii_lowercase();
    TAGS.iter().any(|tag| lower.contains(tag))
}

fn validate_text(text: &str, label: &str) -> Result<()> {
    plain_text(text)
        .map(|_| ())
        .map_err(|error| unavailable(format!("{label}: {error}")))
}

fn validate_c4_metadata(
    alias: &str,
    sprite: Option<&Value>,
    tags: Option<&Value>,
    link: Option<&Value>,
    shadowing: Option<&Value>,
    legend_text: Option<&Value>,
    legend_sprite: Option<&Value>,
    shape: Option<&Value>,
) -> Result<()> {
    if [sprite, tags, shadowing, legend_text, legend_sprite]
        .into_iter()
        .flatten()
        .any(|value| !value.is_null() && value.as_str().is_none_or(|text| !text.trim().is_empty()))
    {
        return Err(unavailable(format!(
            "C4 element `{alias}` uses sprite, tags, legend, or shadow effects"
        )));
    }
    if let Some(link) = link
        && !link.is_null()
        && link.as_str().is_none()
    {
        return Err(unavailable(format!(
            "C4 element `{alias}` uses a non-text link value"
        )));
    }
    if let Some(shape) = shape
        && !shape.is_null()
        && shape.as_str().is_none()
    {
        return Err(unavailable(format!(
            "C4 element `{alias}` uses a non-text custom shape"
        )));
    }
    Ok(())
}

fn value_link(
    value: Option<&Value>,
    security: MermaidNavigationSecurity,
) -> Result<Option<String>> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| unavailable("C4 link value is not a portable string"))?;
    Ok(portable_navigation_uri(Some(value), security))
}

fn parse_color(value: &str) -> Result<Color> {
    let parsed = merman_core::theme_color::ThemeColor::parse(value)
        .map_err(|error| unavailable(format!("C4 color `{value}` is not portable: {error}")))?;
    let channel = |kind| parsed.channel(kind).round().clamp(0.0, 255.0) as u8;
    Ok(Color::rgba(
        channel(merman_core::theme_color::ColorChannel::Red),
        channel(merman_core::theme_color::ColorChannel::Green),
        channel(merman_core::theme_color::ColorChannel::Blue),
        (parsed.channel(merman_core::theme_color::ColorChannel::Alpha) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
    ))
}

fn parse_optional_color(value: Option<&str>) -> Result<Option<Color>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(parse_color)
        .transpose()
}

fn validate_box(x: f64, y: f64, width: f64, height: f64, label: &str) -> Result<()> {
    if ![x, y, width, height].into_iter().all(f64::is_finite) || width < 0.0 || height < 0.0 {
        return Err(invalid(format!("{label} has invalid geometry")));
    }
    Ok(())
}

fn unique_shape_layouts<'a>(
    layout: &'a C4DiagramLayout,
) -> Result<HashMap<&'a str, &'a C4ShapeLayout>> {
    let mut map = HashMap::with_capacity(layout.shapes.len());
    for shape in &layout.shapes {
        if map.insert(shape.alias.as_str(), shape).is_some() {
            return Err(invalid(format!(
                "duplicate C4 shape layout `{}`",
                shape.alias
            )));
        }
    }
    Ok(map)
}

fn unique_boundary_layouts<'a>(
    layout: &'a C4DiagramLayout,
) -> Result<HashMap<&'a str, &'a C4BoundaryLayout>> {
    let mut map = HashMap::with_capacity(layout.boundaries.len());
    for boundary in &layout.boundaries {
        if map.insert(boundary.alias.as_str(), boundary).is_some() {
            return Err(invalid(format!(
                "duplicate C4 boundary layout `{}`",
                boundary.alias
            )));
        }
    }
    Ok(map)
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("C4 layout bounds are invalid"));
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
        family: RenderFamilyKind::C4.as_str().to_string(),
        reason: message.into(),
    }
}
