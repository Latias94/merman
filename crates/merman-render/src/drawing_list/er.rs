//! Renderer-neutral ER diagram adapter.
//!
//! ER layout keeps a separate `render_edges` projection because Dagre may use helper segments
//! for self-loops.  The canonical DrawingList consumes that projection when available, preserving
//! Mermaid's visible relationship edges without exposing internal layout helpers.

use super::{
    ErSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    theme_color,
};
use crate::config::{config_f64_explicit_css_px, config_string};
use crate::drawing_list::support::{stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, ErDiagramLayout, LayoutCluster, LayoutEdge, LayoutNode};
use crate::render_geometry::{FlowchartCurveKind, flowchart_curve_segments};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::er::{
    ErAttributeRenderModel, ErDiagramRenderModel, ErEntityRenderModel, ErRelationshipRenderModel,
};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

type ErPair = FamilyPair<ErDiagramRenderModel, ErDiagramLayout>;

pub(crate) fn build_er_document(
    pair: &ErPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let mut builder = ErBuilder::new(pair, metadata, policy, session)?;
    builder.build()
}

struct ErBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a ErDiagramRenderModel,
    layout: &'a ErDiagramLayout,
    edges: Vec<LayoutEdge>,
    entities_by_id: HashMap<&'a str, &'a ErEntityRenderModel>,
    relationships_by_index: HashMap<usize, &'a ErRelationshipRenderModel>,
    font: FontDescriptor,
    font_size: f64,
    line_height: f64,
    text_obligation: TextObligation,
    node_fill: Color,
    node_stroke: Color,
    node_text: Color,
    line_color: Color,
    cluster_fill: Color,
    cluster_stroke: Color,
    cluster_text: Color,
    edge_label_background: Color,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> ErBuilder<'a> {
    fn new(
        pair: &'a ErPair,
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
                "hand-drawn ER output is RoughJS-owned and has no stable vector equivalent in DrawingList v1",
            ));
        }
        if config
            .get("themeCSS")
            .and_then(Value::as_str)
            .is_some_and(|css| !css.trim().is_empty())
        {
            return Err(unavailable(
                "themeCSS is an unresolved SVG cascade input for ER DrawingList output",
            ));
        }
        let bounds = layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("ER layout did not provide root bounds"))?;
        validate_bounds(bounds)?;

        let font_size = config_f64_explicit_css_px(config, &["themeVariables", "fontSize"])
            .unwrap_or(16.0)
            .max(1.0);
        let font_family = config_string(config, &["fontFamily"])
            .or_else(|| config_string(config, &["themeVariables", "fontFamily"]))
            .unwrap_or_else(|| crate::config::MERMAID_DEFAULT_FONT_FAMILY_CSS.to_string());
        let font = FontDescriptor {
            families: parse_font_families(font_family),
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        let node_fill = theme_color(config, "mainBkg", "#ECECFF")?;
        let node_stroke = theme_color(config, "nodeBorder", "#9370DB")?;
        let node_text = theme_color(config, "nodeTextColor", "#333333")?;
        let line_color = theme_color(config, "lineColor", "#333333")?;
        let cluster_fill = theme_color(config, "clusterBkg", "#ffffde")?;
        let cluster_stroke = theme_color(config, "clusterBorder", "#aaaa33")?;
        let cluster_text = theme_color(config, "titleColor", "#333333")?;
        let edge_label_background = theme_color(config, "edgeLabelBackground", "#e8e8e8")?;

        unique_layout_nodes(layout)?;
        let edges = if layout.render_edges.is_empty() {
            layout.edges.clone()
        } else {
            layout.render_edges.clone()
        };
        let entities_by_id = model
            .entities
            .values()
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<HashMap<_, _>>();
        let relationships_by_index = model
            .relationships
            .iter()
            .enumerate()
            .map(|(index, relation)| (index, relation))
            .collect::<HashMap<_, _>>();

        let builder = Self {
            metadata,
            session,
            policy,
            model,
            layout,
            edges,
            entities_by_id,
            relationships_by_index,
            font,
            font_size,
            line_height: (font_size * 1.35).max(1.0),
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            node_fill,
            node_stroke,
            node_text,
            line_color,
            cluster_fill,
            cluster_stroke,
            cluster_text,
            edge_label_background,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "er.document".to_string(),
                },
            ],
            semantics: vec![SemanticAnnotation {
                id: "er.document".to_string(),
                role: SemanticRole::Document,
                title: model
                    .acc_title
                    .clone()
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
        for (index, cluster) in self.layout.clusters.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_cluster(index, cluster)?;
        }
        for (index, edge) in self.edges.clone().iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge(index, edge)?;
        }
        for (index, node) in self.layout.nodes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if node.is_cluster || node.id.contains("---") {
                continue;
            }
            self.emit_entity(index, node)?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
        let padding = self
            .metadata
            .effective_config
            .as_value()
            .get("er")
            .and_then(|er| er.get("diagramPadding"))
            .and_then(Value::as_f64)
            .unwrap_or(20.0)
            .max(0.0);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                bounds.min_x - padding,
                bounds.min_y - padding,
                bounds.max_x - bounds.min_x + 2.0 * padding,
                bounds.max_y - bounds.min_y + 2.0 * padding,
            )),
            policy: self.policy,
            resources: std::mem::take(&mut self.resources),
            commands: std::mem::take(&mut self.commands),
            semantics: std::mem::take(&mut self.semantics),
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-er".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "direction": self.model.direction,
                    "geometry_subset": "entities-attribute-tables-relationships-cardinality-subgraphs",
                    "label_mode": "plain_host_text",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Er,
                body: SvgStructureBody::Er(ErSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                }),
            },
        })
    }

    fn preflight(&self) -> Result<()> {
        if self.model.entities.is_empty() && self.model.relationships.is_empty() {
            return Err(invalid("ER diagram has no entities or relationships"));
        }
        if !self.model.classes.is_empty() {
            return Err(unavailable(
                "ER classDef styles require resolved portable paint state",
            ));
        }
        for entity in self.model.entities.values() {
            if entity.shape != "" && entity.shape != "erBox" {
                return Err(unavailable(format!(
                    "ER entity `{}` uses unsupported shape `{}`",
                    entity.id, entity.shape
                )));
            }
            if !entity.css_styles.is_empty()
                || entity
                    .css_classes
                    .split_whitespace()
                    .any(|class| class != "default")
            {
                return Err(unavailable(format!(
                    "ER entity `{}` carries unresolved style declarations",
                    entity.id
                )));
            }
            plain_text(&entity.label)
                .map_err(|error| unavailable(format!("ER entity `{}`: {error}", entity.id)))?;
            if !entity.alias.trim().is_empty() {
                plain_text(&entity.alias).map_err(|error| {
                    unavailable(format!("ER entity `{}` alias: {error}", entity.id))
                })?;
            }
            for attribute in &entity.attributes {
                validate_attribute(attribute).map_err(|error| {
                    unavailable(format!("ER entity `{}` attribute: {error}", entity.id))
                })?;
            }
        }
        for subgraph in &self.model.subgraphs {
            if !subgraph.classes.is_empty() || !subgraph.css_styles.is_empty() {
                return Err(unavailable(format!(
                    "ER subgraph `{}` carries unresolved style declarations",
                    subgraph.id
                )));
            }
            plain_text(&subgraph.title)
                .map_err(|error| unavailable(format!("ER subgraph `{}`: {error}", subgraph.id)))?;
        }
        for relation in &self.model.relationships {
            plain_text(&relation.role_a)
                .map_err(|error| unavailable(format!("ER relationship label: {error}")))?;
            if !matches!(
                relation.rel_spec.card_a.as_str(),
                "ONLY_ONE" | "ZERO_OR_ONE" | "ONE_OR_MORE" | "ZERO_OR_MORE" | "MD_PARENT"
            ) || !matches!(
                relation.rel_spec.card_b.as_str(),
                "ONLY_ONE" | "ZERO_OR_ONE" | "ONE_OR_MORE" | "ZERO_OR_MORE" | "MD_PARENT"
            ) {
                return Err(unavailable(
                    "ER relationship uses unknown cardinality marker",
                ));
            }
        }
        for node in &self.layout.nodes {
            validate_layout_node(node)?;
        }
        for edge in &self.edges {
            if edge.points.len() < 2 {
                return Err(invalid(format!(
                    "ER edge `{}` has fewer than two points",
                    edge.id
                )));
            }
            if edge
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
            {
                return Err(invalid(format!(
                    "ER edge `{}` has non-finite points",
                    edge.id
                )));
            }
            if let Some(label) = edge.label.as_ref() {
                validate_label(label, &edge.id)?;
            }
        }
        Ok(())
    }

    fn emit_cluster(&mut self, index: usize, cluster: &LayoutCluster) -> Result<()> {
        let semantic_id = format!("er.group.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let bounds = centered_rect(cluster.x, cluster.y, cluster.width, cluster.height);
        self.add_path(
            format!("{semantic_id}.box"),
            rectangle_path(bounds),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(with_alpha(self.cluster_fill, 36))),
                stroke: Some(stroke(self.cluster_stroke, 1.0)),
            },
        )?;
        let title = plain_text(&cluster.title).map_err(unavailable)?;
        if !title.trim().is_empty() {
            self.draw_text(
                &title,
                Point::new(cluster.title_label.x, cluster.title_label.y),
                centered_rect(
                    cluster.title_label.x,
                    cluster.title_label.y,
                    cluster.title_label.width,
                    cluster.title_label.height,
                ),
                self.cluster_text,
                self.font.clone(),
                TextAnchor::Middle,
                TextBaseline::Middle,
            );
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(title),
            description: Some(format!("ER subgraph {}", cluster.id)),
            link: None,
        });
        Ok(())
    }

    fn emit_edge(&mut self, index: usize, edge: &LayoutEdge) -> Result<()> {
        let relation = relationship_for_edge(edge, &self.relationships_by_index);
        let semantic_id = format!("er.edge.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let segments =
            flowchart_curve_segments(&edge.points, FlowchartCurveKind::Basis, 0.0, false, None);
        let mut line = stroke(self.line_color, 1.0);
        if edge.stroke_dasharray.as_deref() == Some("8,8") {
            line.dash_array = vec![8.0, 8.0];
        }
        self.add_path(
            format!("{semantic_id}.route"),
            segments,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(line),
            },
        )?;
        if let Some(marker) = edge.start_marker.as_deref() {
            self.emit_cardinality_marker(&semantic_id, edge, marker, true)?;
        }
        if let Some(marker) = edge.end_marker.as_deref() {
            self.emit_cardinality_marker(&semantic_id, edge, marker, false)?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        let title = relation
            .and_then(|relation| {
                (!relation.role_a.trim().is_empty()).then(|| plain_text(&relation.role_a).ok())
            })
            .flatten();
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.clone(),
            role: SemanticRole::Edge,
            title,
            description: relation
                .map(|relation| format!("{} → {}", relation.entity_a, relation.entity_b))
                .or_else(|| Some(format!("{} → {}", edge.from, edge.to))),
            link: None,
        });
        if let (Some(relation), Some(label)) = (relation, edge.label.as_ref()) {
            if !relation.role_a.trim().is_empty() {
                self.emit_label(
                    &format!("{semantic_id}.label"),
                    &relation.role_a,
                    label,
                    "ER relationship label".to_string(),
                )?;
            }
        }
        Ok(())
    }

    fn emit_cardinality_marker(
        &mut self,
        semantic_id: &str,
        edge: &LayoutEdge,
        marker: &str,
        start: bool,
    ) -> Result<()> {
        let (tip, tangent) = if start {
            (&edge.points[0], &edge.points[1])
        } else {
            let last = edge.points.len() - 1;
            (&edge.points[last], &edge.points[last - 1])
        };
        let segments = cardinality_path(
            Point::new(tip.x, tip.y),
            Point::new(tangent.x, tangent.y),
            marker,
        )?;
        if segments.is_empty() {
            return Ok(());
        }
        self.add_path(
            format!(
                "{semantic_id}.marker.{}",
                if start { "start" } else { "end" }
            ),
            segments,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.line_color, 1.0)),
            },
        )
    }

    fn emit_label(
        &mut self,
        semantic_id: &str,
        raw_text: &str,
        label: &crate::model::LayoutLabel,
        description: String,
    ) -> Result<()> {
        let text = plain_text(raw_text).map_err(unavailable)?;
        if text.trim().is_empty() {
            return Ok(());
        }
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        });
        let bounds = centered_rect(label.x, label.y, label.width, label.height);
        self.add_path(
            format!("{semantic_id}.background"),
            rectangle_path(bounds),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(with_alpha(self.edge_label_background, 210))),
                stroke: None,
            },
        )?;
        self.draw_text(
            &text,
            Point::new(label.x, label.y),
            bounds,
            self.node_text,
            self.font.clone(),
            TextAnchor::Middle,
            TextBaseline::Middle,
        );
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Label,
            title: Some(text),
            description: Some(description),
            link: None,
        });
        Ok(())
    }

    fn emit_entity(&mut self, index: usize, layout_node: &LayoutNode) -> Result<()> {
        let entity = self
            .entities_by_id
            .get(layout_node.id.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("ER layout node `{}` has no entity", layout_node.id)))?;
        let semantic_id = format!("er.entity.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let bounds = centered_rect(
            layout_node.x,
            layout_node.y,
            layout_node.width,
            layout_node.height,
        );
        self.add_path(
            format!("{semantic_id}.box"),
            rectangle_path(bounds),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.node_fill)),
                stroke: Some(stroke(self.node_stroke, 1.3)),
            },
        )?;

        let name = if entity.alias.trim().is_empty() {
            plain_text(&entity.label).map_err(unavailable)?
        } else {
            plain_text(&entity.alias).map_err(unavailable)?
        };
        let mut rows = Vec::with_capacity(entity.attributes.len() + 1);
        rows.push(name.clone());
        for attribute in &entity.attributes {
            rows.push(attribute_text(attribute)?);
        }
        let line_height = self.line_height.max(1.0);
        let total_height = line_height * rows.len() as f64;
        let first_y = layout_node.y - total_height / 2.0 + line_height / 2.0;
        for (row_index, row) in rows.iter().enumerate() {
            self.draw_text(
                row,
                Point::new(layout_node.x, first_y + row_index as f64 * line_height),
                Rect::new(
                    bounds.x + 8.0,
                    first_y + row_index as f64 * line_height - line_height / 2.0,
                    (bounds.width - 16.0).max(1.0),
                    line_height,
                ),
                self.node_text,
                FontDescriptor {
                    weight: if row_index == 0 { 700 } else { 400 },
                    ..self.font.clone()
                },
                TextAnchor::Middle,
                TextBaseline::Middle,
            );
            if row_index == 0 && !entity.attributes.is_empty() {
                self.add_path(
                    format!("{semantic_id}.divider"),
                    line_path(
                        Point::new(bounds.x, first_y + line_height / 2.0),
                        Point::new(bounds.x + bounds.width, first_y + line_height / 2.0),
                    ),
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(stroke(self.node_stroke, 1.0)),
                    },
                )?;
            }
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(name),
            description: Some(format!("Entity {}", entity.id)),
            link: None,
        });
        Ok(())
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

fn relationship_for_edge<'a>(
    edge: &LayoutEdge,
    relationships: &HashMap<usize, &'a ErRelationshipRenderModel>,
) -> Option<&'a ErRelationshipRenderModel> {
    let rest = edge.id.strip_prefix("er-rel-")?;
    let index = rest
        .split('-')
        .next()
        .and_then(|value| value.parse::<usize>().ok())?;
    relationships.get(&index).copied()
}

fn attribute_text(attribute: &ErAttributeRenderModel) -> Result<String> {
    let mut text = format!(
        "{} {}",
        plain_text(&attribute.ty).map_err(unavailable)?,
        plain_text(&attribute.name).map_err(unavailable)?
    );
    if !attribute.keys.is_empty() {
        text.push_str(" [");
        text.push_str(
            &attribute
                .keys
                .iter()
                .map(|key| plain_text(key).map_err(unavailable))
                .collect::<Result<Vec<_>>>()?
                .join(", "),
        );
        text.push(']');
    }
    if !attribute.comment.trim().is_empty() {
        text.push_str(" : ");
        text.push_str(&plain_text(&attribute.comment).map_err(unavailable)?);
    }
    Ok(text)
}

fn validate_attribute(attribute: &ErAttributeRenderModel) -> std::result::Result<(), String> {
    for text in [
        attribute.ty.as_str(),
        attribute.name.as_str(),
        attribute.comment.as_str(),
    ] {
        plain_text(text)?;
    }
    for key in &attribute.keys {
        plain_text(key)?;
    }
    Ok(())
}

fn plain_text(raw: &str) -> std::result::Result<String, String> {
    let decoded = crate::entities::decode_entities_minimal(raw);
    if decoded.contains("**") || decoded.contains("__") || contains_html_tag(&decoded) {
        return Err("contains styled Markdown or HTML markup".to_string());
    }
    Ok(decoded
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n")
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n"))
}

fn contains_html_tag(text: &str) -> bool {
    const TAGS: [&str; 14] = [
        "<a", "</a", "<b", "</b", "<div", "</div", "<em", "</em", "<i", "</i", "<p", "</p",
        "<span", "</span",
    ];
    let lower = text.to_ascii_lowercase();
    TAGS.iter().any(|tag| lower.contains(tag))
}

fn centered_rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect::new(
        x - width / 2.0,
        y - height / 2.0,
        width.max(0.0),
        height.max(0.0),
    )
}

fn rectangle_path(bounds: Rect) -> Vec<PathSegment> {
    vec![
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
    ]
}

fn line_path(start: Point, end: Point) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo { to: start },
        PathSegment::LineTo { to: end },
    ]
}

fn cardinality_path(tip: Point, tangent: Point, marker: &str) -> Result<Vec<PathSegment>> {
    let dx = tip.x - tangent.x;
    let dy = tip.y - tangent.y;
    let length = (dx * dx + dy * dy).sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(invalid("ER cardinality marker has a degenerate tangent"));
    }
    let ux = dx / length;
    let uy = dy / length;
    let nx = -uy;
    let ny = ux;
    let local = |along: f64, across: f64| {
        Point::new(
            tip.x + ux * along + nx * across,
            tip.y + uy * along + ny * across,
        )
    };
    let mut path = Vec::new();
    let marker = marker.to_ascii_uppercase();
    let has_circle = marker.contains("ZERO_OR_ONE") || marker.contains("ZERO_OR_MORE");
    let has_bar = marker.contains("ONLY_ONE") || marker.contains("ZERO_OR_ONE");
    let has_crow = marker.contains("ONE_OR_MORE") || marker.contains("ZERO_OR_MORE");
    if has_circle {
        let center = local(7.0, 0.0);
        let radius = 5.5;
        path.push(PathSegment::MoveTo {
            to: Point::new(center.x + radius, center.y),
        });
        path.push(PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: true,
            sweep_clockwise: true,
            to: Point::new(center.x - radius, center.y),
        });
        path.push(PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc: true,
            sweep_clockwise: true,
            to: Point::new(center.x + radius, center.y),
        });
    }
    if has_bar {
        for along in [0.0, 7.0] {
            path.push(PathSegment::MoveTo {
                to: local(along, -7.0),
            });
            path.push(PathSegment::LineTo {
                to: local(along, 7.0),
            });
        }
    }
    if has_crow {
        let root = local(0.0, 0.0);
        let left = local(14.0, 8.0);
        let right = local(14.0, -8.0);
        path.extend([
            PathSegment::MoveTo { to: root },
            PathSegment::LineTo { to: left },
            PathSegment::MoveTo { to: root },
            PathSegment::LineTo {
                to: local(16.0, 0.0),
            },
            PathSegment::MoveTo { to: root },
            PathSegment::LineTo { to: right },
        ]);
    }
    Ok(path)
}

fn validate_layout_node(node: &LayoutNode) -> Result<()> {
    if ![node.x, node.y, node.width, node.height]
        .into_iter()
        .all(f64::is_finite)
        || node.width < 0.0
        || node.height < 0.0
    {
        return Err(invalid(format!(
            "ER layout node `{}` has invalid geometry",
            node.id
        )));
    }
    Ok(())
}

fn validate_label(label: &crate::model::LayoutLabel, edge_id: &str) -> Result<()> {
    if ![label.x, label.y, label.width, label.height]
        .into_iter()
        .all(f64::is_finite)
        || label.width < 0.0
        || label.height < 0.0
    {
        return Err(invalid(format!(
            "ER edge `{edge_id}` has invalid label geometry"
        )));
    }
    Ok(())
}

fn unique_layout_nodes<'a>(
    layout: &'a ErDiagramLayout,
) -> Result<HashMap<&'a str, &'a LayoutNode>> {
    let mut nodes = HashMap::with_capacity(layout.nodes.len());
    for node in &layout.nodes {
        if nodes.insert(node.id.as_str(), node).is_some() {
            return Err(invalid(format!("duplicate ER layout node `{}`", node.id)));
        }
    }
    Ok(nodes)
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("ER layout bounds are invalid"));
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
        family: RenderFamilyKind::Er.as_str().to_string(),
        reason: message.into(),
    }
}
