//! Renderer-neutral Requirement diagram adapter.
//!
//! Requirement layout owns the Dagre positions and the prepared label plans.  The adapter keeps
//! those coordinates, expands relationship markers into ordinary paths, and accepts only labels
//! whose prepared Markdown is plain text. Classic Rough.js box and divider geometry is lowered to
//! ordinary path segments; browser HTML, hand-drawn styling, filters, and unsupported class
//! declarations remain explicit failures instead of becoming silent omissions.

use super::{
    RenderDocument, RequirementSvgBody, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    theme_color,
};
use crate::config::config_string_vec;
use crate::drawing_list::flowchart::{ellipse_path, rounded_rect_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, LayoutEdge, LayoutNode, LayoutPoint, RequirementDiagramLayout};
use crate::render_geometry::{FlowchartCurveKind, flowchart_curve_segments};
use crate::requirement::{
    RequirementEdgeLabelPlan, RequirementNodeLabelPlan, RequirementNodeRenderPlan,
    RequirementPreparedArtifact,
};
use crate::rough_geometry::{
    RoughRectangleSpec, display_color_to_srgba, operation_randomness, opset_to_path_segments,
    rough_line_opset, rough_rectangle_opsets,
};
use crate::text::{
    TextMeasurer as _, TextStyle as MeasurementTextStyle, mermaid_markdown_to_xhtml_label_fragment,
    mermaid_xhtml_label_plain_text,
};
use crate::{Error, Result};
use merman_core::diagrams::requirement::{
    RequirementDiagramRenderModel, RequirementRenderElement, RequirementRenderNode,
};
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle as DisplayTextStyle, Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

type RequirementPair = FamilyPair<RequirementDiagramRenderModel, RequirementPreparedArtifact>;

const RELATION_COLOR_DEFAULT: &str = "#333333";
const RELATION_LABEL_COLOR_DEFAULT: &str = "black";
const REQUIREMENT_BACKGROUND_DEFAULT: &str = "#ECECFF";
const NODE_BORDER_DEFAULT: &str = "#9370DB";
const NODE_TEXT_DEFAULT: &str = "#333333";
const RELATION_LABEL_BACKGROUND_DEFAULT: &str = "rgba(232,232,232, 0.8)";
const LABEL_PADDING_PX: f64 = 20.0;
const TITLE_FONT_WEIGHT: u16 = 700;

#[derive(Debug, Clone, Copy)]
struct NodeStyle {
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_width: f64,
    text: Color,
    weight: u16,
    font_style: FontStyle,
}

struct TextEmitSpec {
    origin: Point,
    font_size: f64,
    weight: u16,
    color: Color,
    font: FontDescriptor,
    anchor: TextAnchor,
    baseline: TextBaseline,
}

pub(crate) fn build_requirement_document(
    pair: &RequirementPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    RequirementBuilder::new(pair, metadata, policy, session)?.build()
}

struct RequirementBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a RequirementDiagramRenderModel,
    layout: &'a RequirementDiagramLayout,
    prepared_nodes: &'a HashMap<String, RequirementNodeRenderPlan>,
    prepared_edges: &'a HashMap<dugong::graphlib::EdgeKey, RequirementEdgeLabelPlan>,
    font: FontDescriptor,
    font_size: f64,
    relation_color: Color,
    relation_label_color: Color,
    relation_label_background: Color,
    default_fill: Color,
    default_stroke: Color,
    default_text: Color,
    relation_width: f64,
    rough_randomness: roughr::core::RoughRandomness,
    border_colors: Vec<String>,
    background_colors: Vec<String>,
    text_obligation: TextObligation,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    semantic_looks: BTreeMap<String, String>,
    semantic_color_ids: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
}

impl<'a> RequirementBuilder<'a> {
    fn new(
        pair: &'a RequirementPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let settings = crate::requirement::RequirementConfigView::new(config).render_settings();
        if settings.look.as_str().eq_ignore_ascii_case("handDrawn") {
            return Err(unavailable(
                "hand-drawn RoughJS Requirement output has no stable vector equivalent in DrawingList v1",
            ));
        }
        if settings.look.is_neo() {
            return Err(unavailable(
                "Requirement neo look requires SVG drop-shadow filters that DrawingList v1 cannot represent",
            ));
        }

        let (layout, prepared_nodes, prepared_edges) = pair.layout().render_parts();
        validate_layout(pair.semantic(), layout, prepared_nodes, prepared_edges)?;

        let font_family = settings.font_family.clone();
        let font = FontDescriptor {
            families: parse_font_families(font_family),
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let relation_color = theme_color(config, "relationColor", RELATION_COLOR_DEFAULT)?;
        let relation_label_color =
            theme_color(config, "relationLabelColor", RELATION_LABEL_COLOR_DEFAULT)?;
        let relation_label_background = config_theme_color(
            config,
            "requirementEdgeLabelBackground",
            theme_color(
                config,
                "edgeLabelBackground",
                RELATION_LABEL_BACKGROUND_DEFAULT,
            )?,
        )?;
        let default_fill = theme_color(
            config,
            "requirementBackground",
            REQUIREMENT_BACKGROUND_DEFAULT,
        )?;
        let default_stroke = theme_color(config, "nodeBorder", NODE_BORDER_DEFAULT)?;
        let default_text = config_theme_color(
            config,
            "nodeTextColor",
            theme_color(config, "textColor", NODE_TEXT_DEFAULT)?,
        )?;
        let relation_width: f64 = 1.0;
        if !relation_width.is_finite() || relation_width < 0.0 {
            return Err(invalid("Requirement relationship stroke width is invalid"));
        }

        Ok(Self {
            metadata,
            session,
            policy,
            model: pair.semantic(),
            layout,
            prepared_nodes,
            prepared_edges,
            font,
            font_size: settings.font_size.max(1.0),
            relation_color,
            relation_label_color,
            relation_label_background,
            default_fill,
            default_stroke,
            default_text,
            relation_width,
            rough_randomness: operation_randomness(
                session,
                settings.hand_drawn_seed,
                "render.requirement.roughjs",
            ),
            border_colors: config_string_vec(config, &["themeVariables", "borderColorArray"]),
            background_colors: config_string_vec(config, &["themeVariables", "bkgColorArray"]),
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "requirement.document".to_string(),
                },
            ],
            semantics: Vec::new(),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            semantic_looks: BTreeMap::new(),
            semantic_color_ids: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.semantics.push(SemanticAnnotation {
            id: "requirement.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| self.metadata.title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        });
        self.semantic_looks
            .insert("requirement.document".to_string(), "classic".to_string());

        self.emit_edges()?;
        self.emit_edge_labels()?;
        self.emit_nodes()?;
        let title = self
            .metadata
            .title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_string);
        if let Some(title) = title.as_deref() {
            self.emit_title(title)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let viewport = self.viewport(title.as_deref())?;
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(viewport),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-requirement".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "direction": self.model.direction,
                    "text_mode": "plain_host_text",
                    "markup": "plain_only",
                    "relationship_curve": "basis",
                    "markers": "expanded_paths",
                    "look": "classic",
                    "use_max_width": crate::requirement::RequirementConfigView::new(self.metadata.effective_config.as_value()).render_settings().use_max_width,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Requirement,
                body: SvgStructureBody::Requirement(RequirementSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: crate::requirement::RequirementConfigView::new(
                        self.metadata.effective_config.as_value(),
                    )
                    .render_settings()
                    .use_max_width,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                    semantic_looks: self.semantic_looks,
                    semantic_color_ids: self.semantic_color_ids,
                    dom_ids: self.dom_ids,
                }),
            },
        })
    }

    fn emit_edges(&mut self) -> Result<()> {
        for (index, edge) in self.layout.edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let identity = edge_identity(edge);
            let prepared = self.prepared_edges.get(&identity).ok_or_else(|| {
                invalid(format!(
                    "missing prepared Requirement edge label for {} -> {} ({})",
                    edge.from, edge.to, edge.id
                ))
            })?;
            let semantic_id = format!("requirement.edge.{index}");
            self.semantic_classes
                .insert(semantic_id.clone(), "edgePath".to_string());
            self.semantic_looks
                .insert(semantic_id.clone(), "classic".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            let segments =
                flowchart_curve_segments(&edge.points, FlowchartCurveKind::Basis, 0.0, false, None);
            let dash_array = if prepared.relationship_type == "contains" {
                Vec::new()
            } else {
                vec![10.0, 7.0]
            };
            self.path_classes.insert(
                format!("{semantic_id}.route"),
                format!(
                    "edge-thickness-normal edge-pattern-{} relationshipLine",
                    if dash_array.is_empty() {
                        "solid"
                    } else {
                        "dashed"
                    }
                ),
            );
            self.add_path(
                format!("{semantic_id}.route"),
                segments,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(merman_display_list::StrokeStyle {
                        dash_array,
                        ..stroke(self.relation_color, self.relation_width)
                    }),
                },
            )?;
            if prepared.marker_start {
                self.emit_contains_marker(&semantic_id, edge)?;
            }
            if prepared.marker_end {
                self.emit_arrow_marker(&semantic_id, edge)?;
            }
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Edge,
                title: prepared
                    .has_label
                    .then(|| plain_markdown(&prepared.display_text))
                    .transpose()?,
                description: Some(format!("{} → {}", edge.from, edge.to)),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_edge_labels(&mut self) -> Result<()> {
        for (index, edge) in self.layout.edges.iter().enumerate() {
            let prepared = self
                .prepared_edges
                .get(&edge_identity(edge))
                .ok_or_else(|| invalid("missing prepared Requirement edge label"))?;
            if !prepared.has_label {
                continue;
            }
            let label = edge.label.as_ref().ok_or_else(|| {
                invalid(format!(
                    "Requirement edge {} has no label geometry",
                    edge.id
                ))
            })?;
            let text = plain_markdown(&prepared.display_text)?;
            let semantic_id = format!("requirement.edge.{index}.label");
            self.semantic_classes
                .insert(semantic_id.clone(), "edgeLabel".to_string());
            self.semantic_looks
                .insert(semantic_id.clone(), "classic".to_string());
            self.path_classes
                .insert(format!("{semantic_id}.background"), "labelBkg".to_string());
            self.text_classes
                .insert(semantic_id.clone(), "edgeLabel".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.add_path(
                format!("{semantic_id}.background"),
                rounded_rect_path(label.x, label.y, label.width, label.height, 0.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.relation_label_background)),
                    stroke: None,
                },
            )?;
            self.emit_text(
                &semantic_id,
                &text,
                TextEmitSpec {
                    origin: Point::new(label.x, label.y),
                    font_size: self.font_size,
                    weight: 400,
                    color: self.relation_label_color,
                    font: self.font.clone(),
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Label,
                title: Some(text),
                description: Some(format!(
                    "Requirement relationship label {}",
                    prepared.relationship_type
                )),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_nodes(&mut self) -> Result<()> {
        let mut source_indices = HashMap::new();
        for (index, node) in self.model.requirements.iter().enumerate() {
            source_indices.insert(
                node.name.as_str(),
                (index, RequirementRenderNodeOrElement::Requirement(node)),
            );
        }
        let requirement_count = self.model.requirements.len();
        for (index, node) in self.model.elements.iter().enumerate() {
            source_indices.entry(node.name.as_str()).or_insert_with(|| {
                (
                    requirement_count + index,
                    RequirementRenderNodeOrElement::Element(node),
                )
            });
        }

        for node in &self.layout.nodes {
            let Some(prepared) = self.prepared_nodes.get(&node.id) else {
                return Err(invalid(format!(
                    "missing prepared Requirement node plan for {}",
                    node.id
                )));
            };
            if matches!(prepared, RequirementNodeRenderPlan::EdgeLabelAnchor) {
                continue;
            }
            let RequirementNodeRenderPlan::Semantic(plan) = prepared else {
                unreachable!("edge label anchors were handled above");
            };
            let Some((source_index, source)) = source_indices.get(node.id.as_str()) else {
                return Err(invalid(format!(
                    "Requirement layout node `{}` has no semantic source",
                    node.id
                )));
            };
            let semantic_id = format!("requirement.node.{source_index}");
            self.dom_ids
                .insert(semantic_id.clone(), source.name().to_string());
            self.semantic_classes
                .insert(semantic_id.clone(), "node default".to_string());
            self.semantic_looks
                .insert(semantic_id.clone(), "classic".to_string());
            if !self.border_colors.is_empty() {
                self.semantic_color_ids.insert(
                    semantic_id.clone(),
                    format!("color-{}", source_index % self.border_colors.len()),
                );
            }
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            let style = self.node_style(*source_index, source)?;
            let rough_shape = rough_rectangle_opsets(RoughRectangleSpec {
                x: node.x,
                y: node.y,
                width: node.width,
                height: node.height,
                fill: style.fill.map(display_color_to_srgba),
                stroke: style.stroke.map(display_color_to_srgba),
                stroke_width: style.stroke_width as f32,
                randomness: &self.rough_randomness,
            })?;
            if let (Some(fill), Some(fill_geometry)) = (style.fill, rough_shape.fill.as_ref()) {
                self.add_path(
                    format!("{semantic_id}.shape.fill"),
                    opset_to_path_segments(fill_geometry)?,
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: Some(Paint::solid(fill)),
                        stroke: None,
                    },
                )?;
            }
            if let (Some(stroke_color), Some(stroke_geometry)) =
                (style.stroke, rough_shape.stroke.as_ref())
            {
                self.add_path(
                    format!("{semantic_id}.shape.stroke"),
                    opset_to_path_segments(stroke_geometry)?,
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(stroke(stroke_color, style.stroke_width)),
                    },
                )?;
            }
            self.emit_node_labels(&semantic_id, node, plan, style)?;
            if let (Some(divider_offset), Some(stroke_color)) =
                (plan.divider_y_offset, style.stroke)
            {
                self.path_classes
                    .insert(format!("{semantic_id}.divider"), "divider".to_string());
                let start = Point::new(node.x, node.y + divider_offset);
                let end = Point::new(node.x + node.width, node.y + divider_offset);
                let divider = rough_line_opset(
                    start.x,
                    start.y,
                    end.x,
                    end.y,
                    display_color_to_srgba(stroke_color),
                    style.stroke_width as f32,
                    &self.rough_randomness,
                )?;
                self.add_path(
                    format!("{semantic_id}.divider"),
                    opset_to_path_segments(&divider)?,
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(stroke(stroke_color, style.stroke_width)),
                    },
                )?;
            }
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(source.name().to_string()),
                description: Some(node_description(source)),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_node_labels(
        &mut self,
        semantic_id: &str,
        node: &LayoutNode,
        plan: &RequirementNodeLabelPlan,
        style: NodeStyle,
    ) -> Result<()> {
        let center = Point::new(node.x + node.width / 2.0, node.y + node.height / 2.0);
        for (index, line) in plan.lines.iter().enumerate() {
            let text = plain_markdown(&line.display_text)?;
            let (anchor, origin_x) = if line.keep_centered {
                (TextAnchor::Middle, center.x)
            } else {
                (TextAnchor::Start, node.x + LABEL_PADDING_PX / 2.0)
            };
            let origin_y = node.y + line.y_offset + LABEL_PADDING_PX;
            self.text_classes.insert(
                format!("{semantic_id}.label.{index}"),
                "reqLabel".to_string(),
            );
            self.emit_text(
                format!("{semantic_id}.label.{index}"),
                &text,
                TextEmitSpec {
                    origin: Point::new(origin_x, origin_y),
                    font_size: self.font_size,
                    weight: if line.bold {
                        TITLE_FONT_WEIGHT
                    } else {
                        style.weight
                    },
                    color: style.text,
                    font: FontDescriptor {
                        weight: if line.bold {
                            TITLE_FONT_WEIGHT
                        } else {
                            style.weight
                        },
                        style: style.font_style,
                        ..self.font.clone()
                    },
                    anchor,
                    baseline: TextBaseline::Middle,
                },
            )?;
        }
        Ok(())
    }

    fn node_style(&self, source_index: usize, source: NodeSource<'_>) -> Result<NodeStyle> {
        let fill = self
            .background_colors
            .get(source_index)
            .map(|value| PortableStyleResolver::new("requirement").color("bkgColorArray", value))
            .transpose()?
            .or(Some(self.default_fill));
        let stroke = self
            .border_colors
            .get(source_index)
            .map(|value| PortableStyleResolver::new("requirement").color("borderColorArray", value))
            .transpose()?
            .or(Some(self.default_stroke));
        let mut style = NodeStyle {
            fill,
            stroke,
            stroke_width: 1.3,
            text: self.default_text,
            weight: 400,
            font_style: FontStyle::Normal,
        };
        for raw in source.css_styles() {
            let raw = raw.trim().trim_end_matches(';');
            let Some((key, value)) = raw.split_once(':') else {
                return Err(unavailable(format!(
                    "Requirement node style `{raw}` is not a declaration"
                )));
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "fill" | "background-color" => {
                    style.fill =
                        PortableStyleResolver::new("requirement").optional_color("fill", value)?;
                }
                "stroke" => {
                    style.stroke = PortableStyleResolver::new("requirement")
                        .optional_color("stroke", value)?;
                }
                "stroke-width" => {
                    style.stroke_width =
                        PortableStyleResolver::new("requirement").length("stroke-width", value)?;
                }
                "color" => {
                    style.text = PortableStyleResolver::new("requirement").color("color", value)?;
                }
                "font-weight" => style.weight = parse_font_weight(value)?,
                "font-style" => style.font_style = parse_font_style(value)?,
                _ => {
                    return Err(unavailable(format!(
                        "Requirement node style `{key}` cannot be represented by DrawingList v1"
                    )));
                }
            }
        }
        if style.fill.is_none() && style.stroke.is_none() {
            return Err(unavailable(
                "Requirement node style removes both fill and stroke",
            ));
        }
        Ok(style)
    }

    fn emit_contains_marker(&mut self, semantic_id: &str, edge: &LayoutEdge) -> Result<()> {
        let start = edge
            .points
            .first()
            .ok_or_else(|| invalid("Requirement edge has no start marker point"))?;
        let direction = start_marker_direction(&edge.points)?;
        let normal = Point::new(-direction.y, direction.x);
        let scale = self.relation_width.max(0.1);
        let center = Point::new(
            start.x + direction.x * 10.0 * scale,
            start.y + direction.y * 10.0 * scale,
        );
        let mut segments = ellipse_path(center.x, center.y, 9.0 * scale, 9.0 * scale);
        segments.extend(line_segments(
            Point::new(
                center.x - direction.x * 9.0 * scale,
                center.y - direction.y * 9.0 * scale,
            ),
            Point::new(
                center.x + direction.x * 9.0 * scale,
                center.y + direction.y * 9.0 * scale,
            ),
        ));
        segments.extend(line_segments(
            Point::new(
                center.x - normal.x * 9.0 * scale,
                center.y - normal.y * 9.0 * scale,
            ),
            Point::new(
                center.x + normal.x * 9.0 * scale,
                center.y + normal.y * 9.0 * scale,
            ),
        ));
        self.path_classes
            .insert(format!("{semantic_id}.marker.start"), "marker".to_string());
        self.add_path(
            format!("{semantic_id}.marker.start"),
            segments,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.relation_color, self.relation_width)),
            },
        )
    }

    fn emit_arrow_marker(&mut self, semantic_id: &str, edge: &LayoutEdge) -> Result<()> {
        let end = edge
            .points
            .last()
            .ok_or_else(|| invalid("Requirement edge has no end marker point"))?;
        let direction = end_marker_direction(&edge.points)?;
        let normal = Point::new(-direction.y, direction.x);
        let scale = self.relation_width.max(0.1);
        let base = Point::new(
            end.x - direction.x * 20.0 * scale,
            end.y - direction.y * 20.0 * scale,
        );
        let left = Point::new(
            base.x + normal.x * 10.0 * scale,
            base.y + normal.y * 10.0 * scale,
        );
        let right = Point::new(
            base.x - normal.x * 10.0 * scale,
            base.y - normal.y * 10.0 * scale,
        );
        self.path_classes
            .insert(format!("{semantic_id}.marker.end"), "marker".to_string());
        self.add_path(
            format!("{semantic_id}.marker.end"),
            vec![
                PathSegment::MoveTo { to: left },
                PathSegment::LineTo {
                    to: Point::new(end.x, end.y),
                },
                PathSegment::MoveTo {
                    to: Point::new(end.x, end.y),
                },
                PathSegment::LineTo { to: right },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.relation_color, self.relation_width)),
            },
        )
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Requirement layout did not provide root bounds"))?;
        let origin = Point::new(
            (bounds.min_x + bounds.max_x) / 2.0,
            -self.title_top_margin(),
        );
        self.emit_text(
            "requirement.title",
            title,
            TextEmitSpec {
                origin,
                font_size: self.font_size,
                weight: TITLE_FONT_WEIGHT,
                color: self.default_text,
                font: FontDescriptor {
                    weight: TITLE_FONT_WEIGHT,
                    ..self.font.clone()
                },
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Alphabetic,
            },
        )?;
        self.text_classes.insert(
            "requirement.title".to_string(),
            "requirementDiagramTitleText".to_string(),
        );
        self.semantics.push(SemanticAnnotation {
            id: "requirement.title".to_string(),
            role: SemanticRole::Label,
            title: Some(title.to_string()),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn title_top_margin(&self) -> f64 {
        crate::requirement::RequirementConfigView::new(self.metadata.effective_config.as_value())
            .render_settings()
            .title_top_margin
    }

    fn viewport(&self, title: Option<&str>) -> Result<Rect> {
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Requirement layout did not provide root bounds"))?;
        let mut min_x = bounds.min_x;
        let mut min_y = bounds.min_y;
        let mut max_x = bounds.max_x;
        let mut max_y = bounds.max_y;
        if let Some(title) = title {
            let measurer = self
                .session
                .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
            let style = MeasurementTextStyle {
                font_family: Some(self.font.families.join(", ")),
                font_size: self.font_size,
                font_weight: Some(TITLE_FONT_WEIGHT.to_string()),
                font_style: None,
            };
            let width = measurer
                .measure_svg_raw_text_bbox_width_px(title, &style)
                .max(1.0);
            let height = measurer
                .measure_svg_simple_text_bbox_height_px(title, &style)
                .max(1.0);
            let x = (bounds.min_x + bounds.max_x) / 2.0;
            let y = -self.title_top_margin();
            min_x = min_x.min(x - width / 2.0);
            max_x = max_x.max(x + width / 2.0);
            min_y = min_y.min(y - height);
            max_y = max_y.max(y);
        }
        let padding = 8.0;
        Ok(Rect::new(
            min_x - padding,
            min_y - padding,
            (max_x - min_x + 2.0 * padding).max(1.0),
            (max_y - min_y + 2.0 * padding).max(1.0),
        ))
    }

    fn emit_text(&mut self, _id: impl Into<String>, value: &str, spec: TextEmitSpec) -> Result<()> {
        let TextEmitSpec {
            origin,
            font_size,
            weight,
            color,
            font,
            anchor,
            baseline,
        } = spec;
        let value = svg_plain_text(value);
        if value.is_empty() {
            return Ok(());
        }
        let measurement_style = MeasurementTextStyle {
            font_family: Some(font.families.join(", ")),
            font_size,
            font_weight: Some(weight.to_string()),
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer
            .measure_svg_raw_text_bbox_width_px(&value, &measurement_style)
            .max(1.0);
        let height = measurer
            .measure_svg_simple_text_bbox_height_px(&value, &measurement_style)
            .max(1.0);
        let left = match anchor {
            TextAnchor::Start => origin.x,
            TextAnchor::Middle => origin.x - width / 2.0,
            TextAnchor::End => origin.x - width,
        };
        let top = match baseline {
            TextBaseline::Alphabetic => origin.y - height,
            TextBaseline::Middle | TextBaseline::Central => origin.y - height / 2.0,
            TextBaseline::Hanging => origin.y,
            TextBaseline::Ideographic
            | TextBaseline::TextBeforeEdge
            | TextBaseline::TextAfterEdge => origin.y - height,
        };
        self.commands.push(DrawingCommand::draw_text(TextRun {
            text: value,
            origin,
            bounds: Rect::new(left, top, width, height),
            style: DisplayTextStyle {
                font: FontDescriptor { weight, ..font },
                font_size,
                letter_spacing: 0.0,
                line_height: font_size * 1.5,
                fill: Paint::solid(color),
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

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("Requirement path has no geometry"));
        }
        let id = ResourceId::new(id.into());
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        Ok(())
    }
}

type NodeSource<'a> = &'a RequirementRenderNodeOrElement<'a>;

#[derive(Debug, Clone, Copy)]
enum RequirementRenderNodeOrElement<'a> {
    Requirement(&'a RequirementRenderNode),
    Element(&'a RequirementRenderElement),
}

impl RequirementRenderNodeOrElement<'_> {
    fn name(&self) -> &str {
        match self {
            Self::Requirement(node) => &node.name,
            Self::Element(node) => &node.name,
        }
    }

    fn css_styles(&self) -> &[String] {
        match self {
            Self::Requirement(node) => &node.css_styles,
            Self::Element(node) => &node.css_styles,
        }
    }
}

fn node_description(source: NodeSource<'_>) -> String {
    match source {
        RequirementRenderNodeOrElement::Requirement(node) => format!(
            "{} requirement{}{}{}{}",
            node.node_type,
            nonempty_detail("; ID: ", &node.requirement_id),
            nonempty_detail("; Text: ", &node.text),
            nonempty_detail("; Risk: ", &node.risk),
            nonempty_detail("; Verification: ", &node.verify_method),
        ),
        RequirementRenderNodeOrElement::Element(node) => format!(
            "{} element{}{}",
            node.element_type,
            nonempty_detail("; Type: ", &node.element_type),
            nonempty_detail("; Doc Ref: ", &node.doc_ref),
        ),
    }
}

fn nonempty_detail(prefix: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        String::new()
    } else {
        format!("{prefix}{value}")
    }
}

fn plain_markdown(value: &str) -> Result<String> {
    let fragment = mermaid_markdown_to_xhtml_label_fragment(value, true);
    mermaid_xhtml_label_plain_text(&fragment).map(|text| svg_plain_text(&text)).ok_or_else(|| {
        unavailable(
            "Requirement labels contain Markdown/HTML styling that DrawingList v1 cannot preserve",
        )
    })
}

fn config_theme_color(config: &Value, key: &str, fallback: Color) -> Result<Color> {
    let Some(value) = config
        .get("themeVariables")
        .and_then(|variables| variables.get(key))
        .and_then(Value::as_str)
    else {
        return Ok(fallback);
    };
    PortableStyleResolver::new("requirement").color(key, value)
}

fn edge_identity(edge: &LayoutEdge) -> dugong::graphlib::EdgeKey {
    dugong::graphlib::EdgeKey::new(&edge.from, &edge.to, Some(&edge.id))
}

fn line_segments(from: Point, to: Point) -> Vec<PathSegment> {
    vec![PathSegment::MoveTo { to: from }, PathSegment::LineTo { to }]
}

fn unit_direction(x: f64, y: f64) -> Option<Point> {
    let length = x.hypot(y);
    if !length.is_finite() || length <= f64::EPSILON {
        None
    } else {
        Some(Point::new(x / length, y / length))
    }
}

fn start_marker_direction(points: &[LayoutPoint]) -> Result<Point> {
    let start = points
        .first()
        .ok_or_else(|| invalid("Requirement edge has no start marker point"))?;
    points
        .iter()
        .skip(1)
        .find_map(|candidate| unit_direction(candidate.x - start.x, candidate.y - start.y))
        .ok_or_else(|| {
            unavailable(
                "Requirement start marker has no non-zero tangent; DrawingList v1 cannot choose a lossless marker orientation",
            )
        })
}

fn end_marker_direction(points: &[LayoutPoint]) -> Result<Point> {
    let end = points
        .last()
        .ok_or_else(|| invalid("Requirement edge has no end marker point"))?;
    points
        .iter()
        .rev()
        .skip(1)
        .find_map(|candidate| unit_direction(end.x - candidate.x, end.y - candidate.y))
        .ok_or_else(|| {
            unavailable(
                "Requirement end marker has no non-zero tangent; DrawingList v1 cannot choose a lossless marker orientation",
            )
        })
}

fn parse_font_weight(value: &str) -> Result<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(400),
        "bold" | "bolder" => Ok(700),
        other => other
            .parse::<u16>()
            .map_err(|_| unavailable(format!("Requirement font-weight `{value}` is not portable"))),
    }
}

fn parse_font_style(value: &str) -> Result<FontStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(unavailable(format!(
            "Requirement font-style `{value}` is not portable"
        ))),
    }
}

fn validate_layout(
    model: &RequirementDiagramRenderModel,
    layout: &RequirementDiagramLayout,
    prepared_nodes: &HashMap<String, RequirementNodeRenderPlan>,
    prepared_edges: &HashMap<dugong::graphlib::EdgeKey, RequirementEdgeLabelPlan>,
) -> Result<()> {
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| invalid("Requirement layout did not provide root bounds"))?;
    validate_bounds(bounds)?;
    let mut layout_node_ids = HashSet::with_capacity(layout.nodes.len());
    for node in &layout.nodes {
        if !layout_node_ids.insert(node.id.as_str()) {
            return Err(invalid(format!(
                "duplicate Requirement layout node id `{}`",
                node.id
            )));
        }
        if node.id.is_empty()
            || [node.x, node.y].iter().any(|value| !value.is_finite())
            || [node.width, node.height]
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
            || node.width <= 0.0
            || node.height <= 0.0
        {
            return Err(invalid("Requirement node geometry is invalid"));
        }
        if !prepared_nodes.contains_key(&node.id) {
            return Err(invalid(format!(
                "Requirement layout node `{}` has no prepared plan",
                node.id
            )));
        }
    }
    let mut edge_ids = HashSet::with_capacity(layout.edges.len());
    let mut edge_keys = HashSet::with_capacity(layout.edges.len());
    let mut rendered_ids = HashSet::with_capacity(layout.edges.len());
    for edge in &layout.edges {
        if edge.id.is_empty() || !edge_ids.insert(edge.id.as_str()) {
            return Err(invalid(format!(
                "duplicate or empty Requirement layout edge id `{}`",
                edge.id
            )));
        }
        if edge.from.is_empty()
            || edge.to.is_empty()
            || edge.points.len() < 2
            || edge
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(invalid(format!(
                "Requirement edge `{}` has invalid route geometry",
                edge.id
            )));
        }
        if !layout_node_ids.contains(edge.from.as_str())
            || !layout_node_ids.contains(edge.to.as_str())
        {
            return Err(invalid(format!(
                "Requirement edge `{}` references a missing layout node",
                edge.id
            )));
        }
        let identity = edge_identity(edge);
        if !edge_keys.insert(identity.clone()) {
            return Err(invalid(format!(
                "duplicate Requirement edge identity for {} -> {} ({})",
                edge.from, edge.to, edge.id
            )));
        }
        let plan = prepared_edges.get(&edge_identity(edge)).ok_or_else(|| {
            invalid(format!(
                "Requirement edge `{}` has no prepared label plan",
                edge.id
            ))
        })?;
        if plan.relationship_type.trim().is_empty() {
            return Err(invalid(format!(
                "Requirement edge `{}` has no relationship type",
                edge.id
            )));
        }
        if plan.rendered_id.trim().is_empty() || !rendered_ids.insert(plan.rendered_id.as_str()) {
            return Err(invalid(format!(
                "duplicate or empty rendered Requirement edge id `{}`",
                plan.rendered_id
            )));
        }
        if plan.has_label {
            let label = edge.label.as_ref().ok_or_else(|| {
                invalid(format!(
                    "Requirement edge `{}` has text but no label geometry",
                    edge.id
                ))
            })?;
            if [label.x, label.y, label.width, label.height]
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
                || label.width <= 0.0
                || label.height <= 0.0
            {
                return Err(invalid(format!(
                    "Requirement edge `{}` label geometry is invalid",
                    edge.id
                )));
            }
        }
    }

    for node in model
        .requirements
        .iter()
        .map(|node| node.name.as_str())
        .chain(model.elements.iter().map(|node| node.name.as_str()))
        .filter(|node| *node != "__proto__")
    {
        if !layout_node_ids.contains(node) {
            return Err(invalid(format!(
                "Requirement semantic node `{node}` has no layout geometry"
            )));
        }
    }
    Ok(())
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .iter()
        .any(|value| !value.is_finite())
        || bounds.max_x <= bounds.min_x
        || bounds.max_y <= bounds.min_y
    {
        return Err(invalid("Requirement bounds are invalid"));
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
        family: "requirement".to_string(),
        reason: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_point(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }

    #[test]
    fn start_marker_direction_skips_coincident_endpoint_points() {
        let points = [
            layout_point(1.0, 2.0),
            layout_point(1.0, 2.0),
            layout_point(4.0, 6.0),
        ];

        assert_eq!(
            start_marker_direction(&points).expect("start marker direction"),
            Point::new(0.6, 0.8)
        );
    }

    #[test]
    fn end_marker_direction_skips_coincident_endpoint_points() {
        let points = [
            layout_point(1.0, 2.0),
            layout_point(4.0, 6.0),
            layout_point(4.0, 6.0),
        ];

        assert_eq!(
            end_marker_direction(&points).expect("end marker direction"),
            Point::new(0.6, 0.8)
        );
    }

    #[test]
    fn marker_directions_reject_a_fully_degenerate_route() {
        let points = [layout_point(1.0, 2.0), layout_point(1.0, 2.0)];

        for error in [
            start_marker_direction(&points).expect_err("start marker must fail closed"),
            end_marker_direction(&points).expect_err("end marker must fail closed"),
        ] {
            assert!(matches!(
                error,
                Error::DrawingListUnavailable { ref family, .. } if family == "requirement"
            ));
            assert!(error.to_string().contains("no non-zero tangent"));
        }
    }
}
