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
use crate::c4::{C4ConfigView, C4NodeShape, c4_node_shape};
use crate::config::{config_f64_explicit_css_px, config_string};
use crate::drawing_list::support::{stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, C4BoundaryLayout, C4DiagramLayout, C4RelLayout, C4ShapeLayout};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::c4::{C4DiagramRenderModel, C4RelRenderModel, C4ShapeRenderModel};
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
    rel_layouts: Vec<&'a C4RelLayout>,
    shapes_by_alias: HashMap<&'a str, &'a C4ShapeRenderModel>,
    rels_by_pair: HashMap<(&'a str, &'a str), &'a C4RelRenderModel>,
    font: FontDescriptor,
    font_size: f64,
    line_height: f64,
    text_obligation: TextObligation,
    boundary_fill: Color,
    boundary_stroke: Color,
    boundary_text: Color,
    relation_stroke: Color,
    relation_text: Color,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
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

        let c4_config = C4ConfigView::new(config);
        let layout_settings = c4_config.layout_settings();
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
        let boundary_fill = theme_color(config, "background", "#ffffff")?;
        let boundary_stroke = theme_color(config, "nodeBorder", "#444444")?;
        let relation_stroke = theme_color(config, "lineColor", "#444444")?;
        let relation_text = theme_color(config, "textColor", "#444444")?;

        let shape_layouts = unique_shape_layouts(layout)?;
        let boundary_layouts = unique_boundary_layouts(layout)?;
        if layout.rels.len() != model.rels.len() {
            return Err(invalid(format!(
                "C4 layout has {} relationships for {} semantic relationships",
                layout.rels.len(),
                model.rels.len()
            )));
        }
        let rel_layouts = layout.rels.iter().collect::<Vec<_>>();
        let shapes_by_alias = model
            .shapes
            .iter()
            .map(|shape| (shape.alias.as_str(), shape))
            .collect::<HashMap<_, _>>();
        let rels_by_pair = model
            .rels
            .iter()
            .map(|rel| ((rel.from_alias.as_str(), rel.to_alias.as_str()), rel))
            .collect::<HashMap<_, _>>();

        let builder = Self {
            metadata,
            session,
            policy,
            model,
            layout,
            shape_layouts,
            boundary_layouts,
            rel_layouts,
            shapes_by_alias,
            rels_by_pair,
            font,
            font_size,
            line_height: layout_settings.message_font_size.max(1.0) * 1.25,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            boundary_fill,
            boundary_stroke,
            boundary_text,
            relation_stroke,
            relation_text,
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
        };
        builder.preflight()?;
        Ok(builder)
    }

    fn build(&mut self) -> Result<RenderDocument> {
        // Boundaries are painted before shapes so child nodes remain visible.
        for (index, boundary) in self.layout.boundaries.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if boundary.alias == "global" {
                continue;
            }
            self.emit_boundary(index, boundary)?;
        }
        for (index, shape) in self.layout.shapes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_shape(index, shape)?;
        }
        for (index, relation) in self.rel_layouts.clone().iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_relation(index, relation)?;
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
            .layout
            .title
            .as_deref()
            .or(self.model.title.as_deref())
            .is_some_and(|title| !title.trim().is_empty())
            .then_some(60.0)
            .unwrap_or(0.0);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                bounds.min_x - padding_x,
                bounds.min_y - padding_y - title_extra,
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
                }),
            },
        })
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
        let semantic_id = format!("c4.boundary.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let bounds = Rect::new(boundary.x, boundary.y, boundary.width, boundary.height);
        self.add_path(
            format!("{semantic_id}.box"),
            rounded_rect_path(bounds, 3.0),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(with_alpha(self.boundary_fill, 10))),
                stroke: Some(StrokeStyleWithDash::new(self.boundary_stroke, 1.0, true)),
            },
        )?;
        let title = plain_text(&boundary.label.text).map_err(unavailable)?;
        if !title.trim().is_empty() {
            let center = Point::new(
                boundary.x + boundary.width / 2.0,
                boundary.y + boundary.label.y,
            );
            self.draw_text(
                &title,
                center,
                Rect::new(
                    center.x - boundary.label.width / 2.0,
                    center.y - boundary.label.height / 2.0,
                    boundary.label.width.max(1.0),
                    boundary.label.height.max(1.0),
                ),
                self.boundary_text,
                self.boundary_font(true),
                TextAnchor::Middle,
                TextBaseline::Middle,
            );
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(title),
            description: Some(format!("C4 boundary {}", boundary.alias)),
            link: None,
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
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let node_shape = c4_node_shape(meta);
        let (default_fill, default_stroke) = if shape.type_c4_shape.starts_with("external_") {
            ("#999999", "#8A8A8A")
        } else {
            ("#08427B", "#073B6F")
        };
        let fill =
            parse_optional_color(meta.bg_color.as_deref())?.unwrap_or(parse_color(default_fill)?);
        let border = parse_optional_color(meta.border_color.as_deref())?
            .unwrap_or(parse_color(default_stroke)?);
        let text = parse_optional_color(meta.font_color.as_deref())?
            .unwrap_or(Color::rgba(255, 255, 255, 255));
        self.add_path(
            format!("{semantic_id}.shape"),
            c4_shape_path(shape, node_shape),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(fill)),
                stroke: Some(stroke(border, 2.0)),
            },
        )?;
        let mut blocks = Vec::new();
        if !shape.label.text.trim().is_empty() {
            blocks.push((&shape.label, 700_u16, self.font_size));
        }
        blocks.push((&shape.type_block, 400_u16, self.font_size * 0.75));
        if let Some(descr) = shape.descr.as_ref()
            && !descr.text.trim().is_empty()
        {
            blocks.push((descr, 400_u16, self.font_size * 0.82));
        }
        for (block, weight, size) in blocks {
            let text_value = plain_text(&block.text).map_err(unavailable)?;
            if text_value.trim().is_empty() {
                continue;
            }
            let center = Point::new(shape.x + shape.width / 2.0, shape.y + block.y);
            self.draw_text(
                &text_value,
                center,
                Rect::new(
                    center.x - block.width / 2.0,
                    center.y - block.height / 2.0,
                    block.width.max(1.0),
                    block.height.max(1.0),
                ),
                text,
                FontDescriptor {
                    weight,
                    ..self.font.clone()
                },
                TextAnchor::Middle,
                TextBaseline::Middle,
            );
            let _ = size;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(plain_text(&meta.label.as_str()).map_err(unavailable)?),
            description: Some(format!("C4 {}", shape.alias)),
            link: value_link(meta.link.as_ref())?,
        });
        Ok(())
    }

    fn emit_relation(&mut self, index: usize, relation: &C4RelLayout) -> Result<()> {
        let meta = self
            .rels_by_pair
            .get(&(relation.from.as_str(), relation.to.as_str()))
            .copied()
            .ok_or_else(|| invalid(format!("C4 relationship {} has no semantic model", index)))?;
        let semantic_id = format!("c4.relation.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let start = Point::new(relation.start_point.x, relation.start_point.y);
        let end = Point::new(relation.end_point.x, relation.end_point.y);
        let segments = if index == 0 {
            vec![
                PathSegment::MoveTo { to: start },
                PathSegment::LineTo { to: end },
            ]
        } else {
            let control = Point::new(
                (start.x + end.x) / 2.0 - (end.y - start.y) / 4.0,
                (start.y + end.y) / 2.0 + (end.x - start.x) / 4.0,
            );
            vec![
                PathSegment::MoveTo { to: start },
                PathSegment::QuadTo { control, to: end },
            ]
        };
        let color =
            parse_optional_color(meta.line_color.as_deref())?.unwrap_or(self.relation_stroke);
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
                arrow_path(end, start, false),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(color)),
                    stroke: Some(stroke(color, 1.0)),
                },
            )?;
        }
        if meta.rel_type == "rel_b" || meta.rel_type == "birel" {
            self.add_path(
                format!("{semantic_id}.marker.start"),
                arrow_path(start, end, true),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(color)),
                    stroke: Some(stroke(color, 1.0)),
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
        let mut blocks = vec![(&relation.label, 400_u16, self.font_size * 0.75)];
        if let Some(techn) = relation.techn.as_ref()
            && !techn.text.trim().is_empty()
        {
            blocks.push((techn, 400_u16, self.font_size * 0.75));
        }
        if let Some(descr) = relation.descr.as_ref()
            && !descr.text.trim().is_empty()
        {
            blocks.push((descr, 400_u16, self.font_size * 0.75));
        }
        let mut y = mid.y;
        for (block, weight, _size) in blocks {
            let text_value = plain_text(&block.text).map_err(unavailable)?;
            if text_value.trim().is_empty() {
                continue;
            }
            self.draw_text(
                &text_value,
                Point::new(mid.x, y),
                Rect::new(
                    mid.x - block.width / 2.0,
                    y - block.height / 2.0,
                    block.width.max(1.0),
                    block.height.max(1.0),
                ),
                parse_optional_color(meta.text_color.as_deref())?.unwrap_or(self.relation_text),
                FontDescriptor {
                    weight,
                    ..self.font.clone()
                },
                TextAnchor::Middle,
                TextBaseline::Middle,
            );
            y += block.height + 4.0;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: Some(plain_text(&meta.label.as_str()).map_err(unavailable)?),
            description: Some(format!("{} → {}", relation.from, relation.to)),
            link: value_link(meta.link.as_ref())?,
        });
        Ok(())
    }

    fn boundary_font(&self, bold: bool) -> FontDescriptor {
        FontDescriptor {
            families: self.font.families.clone(),
            weight: if bold { 700 } else { 400 },
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        }
    }

    fn draw_text(
        &mut self,
        text: &str,
        origin: Point,
        bounds: Rect,
        fill: Color,
        font: FontDescriptor,
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
                    font_size: self.font_size,
                    letter_spacing: 0.0,
                    line_height: self.line_height,
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

fn c4_shape_path(shape: &C4ShapeLayout, node_shape: C4NodeShape) -> Vec<PathSegment> {
    let bounds = Rect::new(shape.x, shape.y, shape.width, shape.height);
    match node_shape {
        C4NodeShape::Rounded => rounded_rect_path(bounds, 8.0),
        C4NodeShape::Framed => {
            let inset = 8.0_f64.min(bounds.width / 4.0);
            vec![
                PathSegment::MoveTo {
                    to: Point::new(bounds.x, bounds.y + bounds.height),
                },
                PathSegment::LineTo {
                    to: Point::new(bounds.x, bounds.y),
                },
                PathSegment::LineTo {
                    to: Point::new(bounds.x + bounds.width - inset, bounds.y),
                },
                PathSegment::LineTo {
                    to: Point::new(bounds.x + bounds.width, bounds.y + inset),
                },
                PathSegment::LineTo {
                    to: Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
                },
                PathSegment::LineTo {
                    to: Point::new(bounds.x + inset, bounds.y + bounds.height),
                },
                PathSegment::Close,
            ]
        }
        C4NodeShape::Person => {
            let radius = (shape.width * 0.23).clamp(16.0, 56.0);
            let center = Point::new(shape.x + shape.width / 2.0, shape.y + radius);
            let body = Rect::new(
                shape.x,
                shape.y + radius * 1.7,
                shape.width,
                (shape.height - radius * 1.7).max(1.0),
            );
            let mut path = rounded_rect_path(body, (radius / 2.0).min(12.0));
            path.extend(circle_path(center, radius));
            path
        }
        C4NodeShape::Cylinder => cylinder_path(bounds, false),
        C4NodeShape::HorizontalCylinder => cylinder_path(bounds, true),
    }
}

fn cylinder_path(bounds: Rect, horizontal: bool) -> Vec<PathSegment> {
    let radius = if horizontal {
        (bounds.height / 2.0).max(1.0)
    } else {
        (bounds.width / 6.0).max(1.0)
    };
    let mut path = rounded_rect_path(bounds, radius.min(12.0));
    if horizontal {
        path.extend(line_path(
            Point::new(bounds.x + radius, bounds.y),
            Point::new(bounds.x + radius, bounds.y + bounds.height),
        ));
        path.extend(line_path(
            Point::new(bounds.x + bounds.width - radius, bounds.y),
            Point::new(bounds.x + bounds.width - radius, bounds.y + bounds.height),
        ));
    } else {
        path.extend(line_path(
            Point::new(bounds.x, bounds.y + radius),
            Point::new(bounds.x + bounds.width, bounds.y + radius),
        ));
        path.extend(line_path(
            Point::new(bounds.x, bounds.y + bounds.height - radius),
            Point::new(bounds.x + bounds.width, bounds.y + bounds.height - radius),
        ));
    }
    path
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
            large_arc: true,
            sweep_clockwise: true,
            to: Point::new(center.x - radius, center.y),
        },
        PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: true,
            sweep_clockwise: true,
            to: Point::new(center.x + radius, center.y),
        },
        PathSegment::Close,
    ]
}

fn arrow_path(tip: Point, previous: Point, start: bool) -> Vec<PathSegment> {
    let dx = tip.x - previous.x;
    let dy = tip.y - previous.y;
    let length = (dx * dx + dy * dy).sqrt().max(1.0);
    let ux = dx / length;
    let uy = dy / length;
    let nx = -uy;
    let ny = ux;
    let direction = if start { -1.0 } else { 1.0 };
    let base = Point::new(tip.x - ux * 10.0 * direction, tip.y - uy * 10.0 * direction);
    vec![
        PathSegment::MoveTo { to: tip },
        PathSegment::LineTo {
            to: Point::new(base.x + nx * 5.0, base.y + ny * 5.0),
        },
        PathSegment::LineTo {
            to: Point::new(base.x - nx * 5.0, base.y - ny * 5.0),
        },
        PathSegment::Close,
    ]
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
    let k = 0.5522847498;
    vec![
        PathSegment::MoveTo {
            to: Point::new(bounds.x + radius, bounds.y),
        },
        PathSegment::LineTo {
            to: Point::new(bounds.x + bounds.width - radius, bounds.y),
        },
        PathSegment::CubicTo {
            control1: Point::new(bounds.x + bounds.width - radius + k * radius, bounds.y),
            control2: Point::new(bounds.x + bounds.width, bounds.y + radius - k * radius),
            to: Point::new(bounds.x + bounds.width, bounds.y + radius),
        },
        PathSegment::LineTo {
            to: Point::new(bounds.x + bounds.width, bounds.y + bounds.height - radius),
        },
        PathSegment::CubicTo {
            control1: Point::new(
                bounds.x + bounds.width,
                bounds.y + bounds.height - radius + k * radius,
            ),
            control2: Point::new(
                bounds.x + bounds.width - radius + k * radius,
                bounds.y + bounds.height,
            ),
            to: Point::new(bounds.x + bounds.width - radius, bounds.y + bounds.height),
        },
        PathSegment::LineTo {
            to: Point::new(bounds.x + radius, bounds.y + bounds.height),
        },
        PathSegment::CubicTo {
            control1: Point::new(bounds.x + radius - k * radius, bounds.y + bounds.height),
            control2: Point::new(bounds.x, bounds.y + bounds.height - radius + k * radius),
            to: Point::new(bounds.x, bounds.y + bounds.height - radius),
        },
        PathSegment::LineTo {
            to: Point::new(bounds.x, bounds.y + radius),
        },
        PathSegment::CubicTo {
            control1: Point::new(bounds.x, bounds.y + radius - k * radius),
            control2: Point::new(bounds.x + radius - k * radius, bounds.y),
            to: Point::new(bounds.x + radius, bounds.y),
        },
        PathSegment::Close,
    ]
}

fn line_path(start: Point, end: Point) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo { to: start },
        PathSegment::LineTo { to: end },
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

fn value_link(value: Option<&Value>) -> Result<Option<String>> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| unavailable("C4 link value is not a portable string"))
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

fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.red, color.green, color.blue, alpha)
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
