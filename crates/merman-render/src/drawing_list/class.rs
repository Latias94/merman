//! Renderer-neutral Class diagram adapter.
//!
//! Class diagrams have a richer node body than ordinary graph families: namespace clusters,
//! class compartments, notes, interfaces, terminal relation labels, and relation-end markers all
//! come from the typed Class model/layout pair.  This adapter consumes those artifacts directly;
//! it never reparses or imports the SVG DOM.

use super::builder::DrawingListBuilder;
use super::{
    ClassSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families_for,
    theme_color,
};
use crate::class::class_member_create_text_input;
use crate::config::{config_f64_explicit_css_px, config_string};
use crate::drawing_list::support::{
    navigation_security, portable_navigation_uri, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, ClassDiagramLayout, LayoutEdge, LayoutLabel, LayoutNode};
use crate::render_geometry::emit_basis_segments;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::models::class_diagram::{ClassDiagram, ClassMember, ClassNode, ClassRelation};
use merman_core::svg_security::MermaidNavigationSecurity;
use merman_display_list::{
    Color, DrawingCommand, DrawingListLimits, DrawingListPolicy, FillRule, FontDescriptor,
    FontStyle, Paint, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle,
    Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

type ClassPair = FamilyPair<ClassDiagram, ClassDiagramLayout>;

const DEFAULT_FONT_FAMILY: &str = r#""trebuchet ms",verdana,arial,sans-serif"#;
const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_LINE_HEIGHT: f64 = 22.0;

pub(crate) fn build_class_document(
    pair: &ClassPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: DrawingListLimits,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let builder = ClassBuilder::new(pair, metadata, policy, limits, session)?;
    builder.build()
}

struct ClassBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    model: &'a ClassDiagram,
    layout: &'a ClassDiagramLayout,
    nodes_by_id: HashMap<&'a str, &'a LayoutNode>,
    relations_by_id: HashMap<&'a str, &'a ClassRelation>,
    notes_by_id: HashMap<&'a str, &'a merman_core::models::class_diagram::ClassNote>,
    interfaces_by_id: HashMap<&'a str, &'a merman_core::models::class_diagram::ClassInterface>,
    font: FontDescriptor,
    font_size: f64,
    line_height: f64,
    class_padding: f64,
    text_obligation: TextObligation,
    navigation_security: MermaidNavigationSecurity,
    node_fill: Color,
    node_stroke: Color,
    node_text: Color,
    line_color: Color,
    namespace_fill: Color,
    namespace_stroke: Color,
    namespace_text: Color,
    note_fill: Color,
    note_stroke: Color,
    output: DrawingListBuilder<'a>,
}

struct TextEmitSpec {
    origin: Point,
    bounds: Rect,
    fill: Color,
    font: FontDescriptor,
    anchor: TextAnchor,
    baseline: TextBaseline,
}

impl<'a> ClassBuilder<'a> {
    fn new(
        pair: &'a ClassPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let mut output = DrawingListBuilder::new(policy, limits, session);
        output.push_control(DrawingCommand::Save)?;
        output.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "class.document".to_string(),
        })?;
        let model = pair.semantic();
        let layout = pair.layout();
        let config = metadata.effective_config.as_value();
        let navigation_security = navigation_security(config);
        let look = crate::config::config_diagram_look(config);
        if look.as_str().eq_ignore_ascii_case("handDrawn") {
            return Err(unavailable(
                "hand-drawn Class output is RoughJS-owned and has no stable vector equivalent in DrawingList v1",
            ));
        }
        if crate::class::class_requires_math(model) {
            return Err(unavailable(
                "Class Math labels require a math renderer or explicit raster fallback; DrawingList v1 will not replace them with plain text",
            ));
        }
        let bounds = layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Class layout did not provide root bounds"))?;
        validate_bounds(bounds)?;

        let class_config = config.get("class").unwrap_or(&Value::Null);
        let class_padding = class_config
            .get("padding")
            .and_then(Value::as_f64)
            .or_else(|| {
                class_config
                    .get("padding")
                    .and_then(Value::as_str)
                    .and_then(|value| value.trim_end_matches("px").parse().ok())
            })
            .unwrap_or(12.0)
            .max(0.0);
        let font_size = config_f64_explicit_css_px(config, &["themeVariables", "fontSize"])
            .unwrap_or(DEFAULT_FONT_SIZE)
            .max(1.0);
        let font_family = config_string(config, &["fontFamily"])
            .or_else(|| config_string(config, &["themeVariables", "fontFamily"]))
            .unwrap_or_else(|| DEFAULT_FONT_FAMILY.to_string());
        let font = FontDescriptor {
            families: parse_font_families_for(font_family, RenderFamilyKind::Class)?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        let node_fill = theme_color(config, "mainBkg", "#ECECFF")?;
        let node_stroke = theme_color(config, "nodeBorder", "#9370DB")?;
        let node_text = theme_color(config, "nodeTextColor", "#333333")?;
        let line_color = theme_color(config, "lineColor", "#333333")?;
        let namespace_fill = theme_color(config, "clusterBkg", "#ffffde")?;
        let namespace_stroke = theme_color(config, "clusterBorder", "#aaaa33")?;
        let namespace_text = theme_color(config, "titleColor", "#333333")?;
        let note_fill = theme_color(config, "noteBkgColor", "#fff5ad")?;
        let note_stroke = theme_color(config, "noteBorderColor", "#aaaa33")?;

        let nodes_by_id = unique_layout_nodes(layout)?;
        unique_layout_edges(layout)?;
        let relations_by_id = model
            .relations
            .iter()
            .map(|relation| (relation.id.as_str(), relation))
            .collect::<HashMap<_, _>>();
        let notes_by_id = model
            .notes
            .iter()
            .map(|note| (note.id.as_str(), note))
            .collect::<HashMap<_, _>>();
        let interfaces_by_id = model
            .interfaces
            .iter()
            .map(|interface| (interface.id.as_str(), interface))
            .collect::<HashMap<_, _>>();

        output.push_semantic(SemanticAnnotation {
            id: "class.document".to_string(),
            role: SemanticRole::Document,
            title: model
                .acc_title
                .clone()
                .or_else(|| metadata.title.clone())
                .or_else(|| Some(metadata.diagram_type.clone())),
            description: model.acc_descr.clone(),
            link: None,
        })?;
        let builder = Self {
            metadata,
            session,
            model,
            layout,
            nodes_by_id,
            relations_by_id,
            notes_by_id,
            interfaces_by_id,
            font,
            font_size,
            line_height: (font_size * 1.35).max(DEFAULT_LINE_HEIGHT.min(font_size * 1.35)),
            class_padding,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            navigation_security,
            node_fill,
            node_stroke,
            node_text,
            line_color,
            namespace_fill,
            namespace_stroke,
            namespace_text,
            note_fill,
            note_stroke,
            output,
        };
        builder.preflight()?;
        Ok(builder)
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.session.checkpoint(OperationPhase::Emit)?;

        // Mermaid paints namespace clusters first, then relation geometry/labels, then nodes.
        for (index, cluster) in self.layout.clusters.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_cluster(index, cluster)?;
        }
        for (index, edge) in self.layout.edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge(index, edge)?;
        }
        for (index, node) in self.layout.nodes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if node.is_cluster {
                continue;
            }
            self.emit_node(index, node)?;
        }

        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_control(DrawingCommand::Restore)?;
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
        let padding = self
            .metadata
            .effective_config
            .as_value()
            .get("class")
            .and_then(|class| class.get("diagramPadding"))
            .and_then(Value::as_f64)
            .or_else(|| {
                self.metadata
                    .effective_config
                    .as_value()
                    .get("diagramPadding")
                    .and_then(Value::as_f64)
            })
            .unwrap_or(8.0)
            .max(0.0);
        let document = self.output.finish(
            Viewport::new(Rect::new(
                bounds.min_x - padding,
                bounds.min_y - padding,
                (bounds.max_x - bounds.min_x) + 2.0 * padding,
                (bounds.max_y - bounds.min_y) + 2.0 * padding,
            )),
            BTreeMap::from([(
                "x-merman-class".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "direction": self.model.direction,
                    "geometry_subset": "namespaces-compartments-relations-notes-interfaces",
                    "label_mode": "plain_host_text",
                }),
            )]),
        )?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Class,
                body: SvgStructureBody::Class(ClassSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                }),
            },
        })
    }

    fn preflight(&self) -> Result<()> {
        if self.layout.nodes.is_empty()
            && self.model.classes.is_empty()
            && self.model.notes.is_empty()
        {
            return Err(invalid("Class diagram has no layout or semantic nodes"));
        }
        for node in self.model.classes.values() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if !node.styles.is_empty() {
                return Err(unavailable(format!(
                    "Class node `{}` carries unresolved style class references",
                    node.id
                )));
            }
            if node.callback.is_some() || node.have_callback || node.callback_effective {
                return Err(unavailable(format!(
                    "Class node `{}` uses callback interaction metadata",
                    node.id
                )));
            }
            for text in class_node_texts(node) {
                self.session.checkpoint(OperationPhase::Emit)?;
                plain_text(text)
                    .map_err(|error| unavailable(format!("Class node `{}`: {error}", node.id)))?;
            }
            for member in node.members.iter().chain(&node.methods) {
                if member
                    .css_style
                    .split(';')
                    .filter_map(crate::mermaid_style::parse_safe_style_decl)
                    .any(|(key, _)| key == "text-decoration")
                {
                    return Err(unavailable(format!(
                        "Class member `{}` uses text decoration, which DrawingList v1 cannot represent",
                        member.display_text
                    )));
                }
            }
        }
        for note in &self.model.notes {
            self.session.checkpoint(OperationPhase::Emit)?;
            plain_text(&note.text)
                .map_err(|error| unavailable(format!("Class note `{}`: {error}", note.id)))?;
        }
        for interface in &self.model.interfaces {
            self.session.checkpoint(OperationPhase::Emit)?;
            plain_text(&interface.label).map_err(|error| {
                unavailable(format!("Class interface `{}`: {error}", interface.id))
            })?;
        }
        for namespace in self.model.namespaces.values() {
            self.session.checkpoint(OperationPhase::Emit)?;
            plain_text(&namespace.label).map_err(|error| {
                unavailable(format!("Class namespace `{}`: {error}", namespace.id))
            })?;
        }
        for relation in &self.model.relations {
            self.session.checkpoint(OperationPhase::Emit)?;
            for text in [
                relation.title.as_str(),
                relation.relation_title_1.as_deref().unwrap_or(""),
                relation.relation_title_2.as_deref().unwrap_or(""),
            ] {
                plain_text(text).map_err(|error| {
                    unavailable(format!("Class relation `{}`: {error}", relation.id))
                })?;
            }
            if !matches!(relation.relation.type1, -1..=4)
                || !matches!(relation.relation.type2, -1..=4)
            {
                return Err(unavailable(format!(
                    "Class relation `{}` uses an unknown relation-end marker",
                    relation.id
                )));
            }
        }
        for node in &self.layout.nodes {
            self.session.checkpoint(OperationPhase::Emit)?;
            if !node.is_cluster && !self.nodes_by_id.contains_key(node.id.as_str()) {
                return Err(invalid(format!("Class node `{}` is not unique", node.id)));
            }
            validate_layout_node(node)?;
        }
        for edge in &self.layout.edges {
            self.session.checkpoint(OperationPhase::Emit)?;
            if edge.points.len() < 2 {
                return Err(invalid(format!(
                    "Class edge `{}` has fewer than two route points",
                    edge.id
                )));
            }
            for point in &edge.points {
                self.session.checkpoint(OperationPhase::Emit)?;
                if !point.x.is_finite() || !point.y.is_finite() {
                    return Err(invalid(format!(
                        "Class edge `{}` has non-finite route points",
                        edge.id
                    )));
                }
            }
            for label in edge_labels(edge).into_iter().flatten() {
                validate_label(label, &edge.id)?;
            }
        }
        Ok(())
    }

    fn emit_cluster(&mut self, index: usize, cluster: &crate::model::LayoutCluster) -> Result<()> {
        let bounds = centered_rect(cluster.x, cluster.y, cluster.width, cluster.height);
        let semantic_id = format!("class.namespace.{index}");
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        self.add_path(
            format!("{semantic_id}.box"),
            rectangle_path(bounds),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.namespace_fill)),
                stroke: Some(stroke(self.namespace_stroke, 1.0)),
            },
        )?;
        let title = plain_text(&cluster.title)
            .map_err(|error| unavailable(format!("Class namespace `{}`: {error}", cluster.id)))?;
        if !title.trim().is_empty() {
            self.draw_text(
                &title,
                TextEmitSpec {
                    origin: Point::new(cluster.title_label.x, cluster.title_label.y),
                    bounds: label_rect(&cluster.title_label),
                    fill: self.namespace_text,
                    font: self.font.clone(),
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
        }
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(title),
            description: Some(format!("Namespace {}", cluster.id)),
            link: None,
        })?;
        Ok(())
    }

    fn emit_edge(&mut self, index: usize, edge: &LayoutEdge) -> Result<()> {
        let relation = self.relations_by_id.get(edge.id.as_str()).copied();
        let semantic_id = format!("class.edge.{index}");
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        let mut style = stroke(self.line_color, 1.0);
        if relation.is_some_and(|relation| relation.relation.line_type != 0) {
            style.dash_array = vec![5.0, 5.0];
        }
        self.output.draw_path_with(
            ResourceId::new(format!("{semantic_id}.route")),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(style),
            },
            |emit| emit_basis_segments(&edge.points, emit),
        )?;
        if let Some(relation) = relation {
            self.emit_relation_marker(&semantic_id, edge, relation, true)?;
            self.emit_relation_marker(&semantic_id, edge, relation, false)?;
        }
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        let title = relation
            .and_then(|relation| {
                (!relation.title.trim().is_empty()).then(|| plain_text(&relation.title).ok())
            })
            .flatten();
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id.clone(),
            role: SemanticRole::Edge,
            title,
            description: Some(format!("{} → {}", edge.from, edge.to)),
            link: None,
        })?;

        if let Some(relation) = relation {
            if let Some(label) = edge.label.as_ref()
                && !relation.title.trim().is_empty()
            {
                self.emit_label(
                    &format!("{semantic_id}.label"),
                    &relation.title,
                    label,
                    format!("Label for {}", relation.id),
                )?;
            }
            self.emit_terminal_label(
                &semantic_id,
                relation.relation_title_1.as_deref(),
                edge.start_label_right
                    .as_ref()
                    .or(edge.start_label_left.as_ref()),
                format!("Start label for {}", relation.id),
            )?;
            self.emit_terminal_label(
                &semantic_id,
                relation.relation_title_2.as_deref(),
                edge.end_label_left
                    .as_ref()
                    .or(edge.end_label_right.as_ref()),
                format!("End label for {}", relation.id),
            )?;
        }
        Ok(())
    }

    fn emit_relation_marker(
        &mut self,
        semantic_id: &str,
        edge: &LayoutEdge,
        relation: &ClassRelation,
        start: bool,
    ) -> Result<()> {
        let marker_type = if start {
            relation.relation.type1
        } else {
            relation.relation.type2
        };
        if marker_type < 0 {
            return Ok(());
        }
        let (tip, previous) = if start {
            (edge.points[0].clone(), edge.points[1].clone())
        } else {
            let last = edge.points.len() - 1;
            (edge.points[last].clone(), edge.points[last - 1].clone())
        };
        let marker = marker_path(
            Point::new(tip.x, tip.y),
            Point::new(previous.x, previous.y),
            marker_type,
            start,
        )?;
        if marker.is_empty() {
            // Unknown marker types are rejected during preflight. Keep this guard so a malformed
            // model cannot create an empty path resource.
            return Ok(());
        }
        let fill = if marker_type == 2 {
            Some(Paint::solid(self.line_color))
        } else {
            None
        };
        self.add_path(
            format!(
                "{semantic_id}.marker.{}",
                if start { "start" } else { "end" }
            ),
            marker,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill,
                stroke: Some(stroke(self.line_color, 1.0)),
            },
        )?;
        Ok(())
    }

    fn emit_terminal_label(
        &mut self,
        semantic_id: &str,
        text: Option<&str>,
        label: Option<&LayoutLabel>,
        description: String,
    ) -> Result<()> {
        let Some(text) = text.filter(|text| !text.trim().is_empty()) else {
            return Ok(());
        };
        let Some(label) = label else {
            return Err(invalid(format!(
                "Class terminal label `{text}` has no layout geometry"
            )));
        };
        self.emit_label(
            &format!("{semantic_id}.terminal.{}", self.output.command_count()),
            text,
            label,
            description,
        )
    }

    fn emit_label(
        &mut self,
        semantic_id: &str,
        raw_text: &str,
        label: &LayoutLabel,
        description: String,
    ) -> Result<()> {
        let text = plain_text(raw_text).map_err(unavailable)?;
        if text.trim().is_empty() {
            return Ok(());
        }
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.to_string(),
            })?;
        let bounds = label_rect(label);
        self.add_path(
            format!("{semantic_id}.background"),
            rectangle_path(bounds),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(with_alpha(
                    theme_color(
                        self.metadata.effective_config.as_value(),
                        "edgeLabelBackground",
                        "#e8e8e8",
                    )?,
                    210,
                ))),
                stroke: None,
            },
        )?;
        self.draw_text(
            &text,
            TextEmitSpec {
                origin: Point::new(label.x, label.y),
                bounds,
                fill: self.node_text,
                font: self.font.clone(),
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Middle,
            },
        )?;
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Label,
            title: Some(text),
            description: Some(description),
            link: None,
        })?;
        Ok(())
    }

    fn emit_node(&mut self, index: usize, layout_node: &LayoutNode) -> Result<()> {
        let semantic_id = format!("class.node.{index}");
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        if let Some(note) = self.notes_by_id.get(layout_node.id.as_str()).copied() {
            let text = plain_text(&note.text).map_err(unavailable)?;
            self.add_path(
                format!("{semantic_id}.box"),
                rectangle_path(centered_rect(
                    layout_node.x,
                    layout_node.y,
                    layout_node.width,
                    layout_node.height,
                )),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.note_fill)),
                    stroke: Some(stroke(self.note_stroke, 1.0)),
                },
            )?;
            self.draw_text(
                &text,
                TextEmitSpec {
                    origin: Point::new(layout_node.x, layout_node.y),
                    bounds: centered_rect(
                        layout_node.x,
                        layout_node.y,
                        layout_node.width,
                        layout_node.height,
                    ),
                    fill: self.node_text,
                    font: self.font.clone(),
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
            self.output.push_semantic(SemanticAnnotation {
                id: semantic_id.clone(),
                role: SemanticRole::Node,
                title: Some(text),
                description: Some(format!("Note {}", note.id)),
                link: None,
            })?;
        } else if let Some(interface) = self.interfaces_by_id.get(layout_node.id.as_str()).copied()
        {
            let text = plain_text(&interface.label).map_err(unavailable)?;
            let bounds = centered_rect(
                layout_node.x,
                layout_node.y,
                layout_node.width,
                layout_node.height,
            );
            self.add_path(
                format!("{semantic_id}.box"),
                rounded_rect_path(bounds, (bounds.height / 2.0).min(8.0)),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(with_alpha(self.node_fill, 0))),
                    stroke: Some(stroke(self.node_stroke, 1.0)),
                },
            )?;
            self.draw_text(
                &text,
                TextEmitSpec {
                    origin: Point::new(layout_node.x, layout_node.y),
                    bounds,
                    fill: self.node_text,
                    font: self.font.clone(),
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
            self.output.push_semantic(SemanticAnnotation {
                id: semantic_id.clone(),
                role: SemanticRole::Node,
                title: Some(text),
                description: Some(format!("Interface {}", interface.id)),
                link: None,
            })?;
        } else {
            let node = self
                .model
                .classes
                .get(layout_node.id.as_str())
                .ok_or_else(|| {
                    invalid(format!(
                        "Class layout node `{}` has no semantic class",
                        layout_node.id
                    ))
                })?;
            self.emit_class_node(&semantic_id, node, layout_node)?;
        }
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        Ok(())
    }

    fn emit_class_node(
        &mut self,
        semantic_id: &str,
        node: &ClassNode,
        layout_node: &LayoutNode,
    ) -> Result<()> {
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

        let title = format!("{}{}", node.text.trim(), node.type_param.trim());
        let has_title = !title.trim().is_empty();
        let members_start = usize::from(has_title) + node.annotations.len();
        let methods_start = members_start + node.members.len();
        let body_lines = methods_start + node.methods.len();
        let line_count = body_lines.max(1);
        // Normalize and admit one line at a time rather than retaining a second copy of every
        // member and method before the caller's text budget can stop the build.
        let lines = has_title
            .then_some(title.as_str())
            .into_iter()
            .map(|text| {
                plain_text(text)
                    .map(|text| (text, FontStyle::Normal, 700_u16))
                    .map_err(unavailable)
            })
            .chain(node.annotations.iter().map(|annotation| {
                plain_text(annotation)
                    .map(|text| (format!("«{text}»"), FontStyle::Italic, 400))
                    .map_err(unavailable)
            }))
            .chain(node.members.iter().chain(&node.methods).map(|member| {
                plain_member_text(member).map(|text| {
                    let (style, weight) = class_member_font_style(member);
                    (text, style, weight)
                })
            }))
            .chain((body_lines == 0).then(|| Ok((node.id.clone(), FontStyle::Normal, 400))));

        let line_height = self.line_height.max(1.0);
        let total_height = line_height * line_count as f64;
        let first_y = layout_node.y - total_height / 2.0 + line_height / 2.0;
        for (line_index, line) in lines.enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let (text, style, weight) = line?;
            let x = layout_node.x;
            let y = first_y + line_index as f64 * line_height;
            self.draw_text(
                &text,
                TextEmitSpec {
                    origin: Point::new(x, y),
                    bounds: Rect::new(
                        bounds.x + self.class_padding,
                        y - line_height / 2.0,
                        (bounds.width - 2.0 * self.class_padding).max(1.0),
                        line_height,
                    ),
                    fill: self.node_text,
                    font: FontDescriptor {
                        weight,
                        style,
                        ..self.font.clone()
                    },
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
        }

        let first_divider = if members_start > 0 && members_start < line_count {
            Some(first_y + (members_start as f64 - 0.5) * line_height)
        } else {
            None
        };
        let second_divider = if methods_start > members_start && methods_start < line_count {
            Some(first_y + (methods_start as f64 - 0.5) * line_height)
        } else {
            None
        };
        for (divider_index, y) in [first_divider, second_divider]
            .into_iter()
            .flatten()
            .enumerate()
        {
            self.add_path(
                format!("{semantic_id}.divider.{divider_index}"),
                line_path(
                    Point::new(bounds.x, y),
                    Point::new(bounds.x + bounds.width, y),
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(self.node_stroke, 1.0)),
                },
            )?;
        }
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Node,
            title: Some(svg_plain_text(&title)),
            description: Some(format!("Class {}", node.id)),
            link: portable_navigation_uri(node.link.as_deref(), self.navigation_security),
        })?;
        Ok(())
    }

    fn draw_text(&mut self, text: &str, spec: TextEmitSpec) -> Result<()> {
        self.output.draw_host_text(text, |text| TextRun {
            text,
            origin: spec.origin,
            bounds: spec.bounds,
            style: TextStyle {
                font: spec.font,
                font_size: self.font_size,
                letter_spacing: 0.0,
                line_height: self.line_height,
                fill: Paint::solid(spec.fill),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor: spec.anchor,
            baseline: spec.baseline,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        })
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        self.output.draw_path(ResourceId::new(id), segments, style)
    }
}

fn class_node_texts(node: &ClassNode) -> impl Iterator<Item = &str> {
    std::iter::once(node.text.as_str())
        .chain(std::iter::once(node.type_param.as_str()))
        .chain(node.annotations.iter().map(String::as_str))
        .chain(
            node.members
                .iter()
                .map(|member| member.display_text.as_str()),
        )
        .chain(
            node.methods
                .iter()
                .map(|member| member.display_text.as_str()),
        )
}

fn plain_member_text(member: &ClassMember) -> Result<String> {
    plain_text(&class_member_create_text_input(member)).map_err(unavailable)
}

fn class_member_font_style(member: &ClassMember) -> (FontStyle, u16) {
    let mut style = FontStyle::Normal;
    let mut weight = 400;
    for declaration in member.css_style.split(';') {
        let Some((key, value)) = crate::mermaid_style::parse_safe_style_decl(declaration) else {
            continue;
        };
        match key {
            "font-style" => {
                style = match value.trim().to_ascii_lowercase().as_str() {
                    "italic" => FontStyle::Italic,
                    "oblique" => FontStyle::Oblique,
                    _ => FontStyle::Normal,
                };
            }
            "font-weight" => {
                weight = match value.trim().to_ascii_lowercase().as_str() {
                    "bold" | "bolder" => 700,
                    "lighter" => 300,
                    value => value.parse::<u16>().unwrap_or(400).clamp(1, 1000),
                };
            }
            _ => {}
        }
    }
    (style, weight)
}

fn plain_text(raw: &str) -> std::result::Result<String, String> {
    let decoded = crate::entities::decode_entities_minimal(raw);
    if decoded.contains("**") || decoded.contains("__") || decoded.contains("<img") {
        return Err("contains styled Markdown or an embedded image".to_string());
    }
    let mut normalized = decoded
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n");
    if contains_html_tag(&normalized) {
        return Err("contains HTML markup".to_string());
    }
    normalized = normalized
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(normalized)
}

fn contains_html_tag(text: &str) -> bool {
    const TAGS: [&str; 14] = [
        "<a", "</a", "<b", "</b", "<div", "</div", "<em", "</em", "<i", "</i", "<p", "</p",
        "<span", "</span",
    ];
    TAGS.iter()
        .any(|tag| text.to_ascii_lowercase().contains(tag))
}

fn centered_rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect::new(
        x - width / 2.0,
        y - height / 2.0,
        width.max(0.0),
        height.max(0.0),
    )
}

fn label_rect(label: &LayoutLabel) -> Rect {
    centered_rect(label.x, label.y, label.width, label.height)
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

fn rounded_rect_path(bounds: Rect, radius: f64) -> Vec<PathSegment> {
    let radius = radius
        .min(bounds.width / 2.0)
        .min(bounds.height / 2.0)
        .max(0.0);
    if radius == 0.0 {
        return rectangle_path(bounds);
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

fn marker_path(
    tip: Point,
    previous: Point,
    marker_type: i32,
    start: bool,
) -> Result<Vec<PathSegment>> {
    let dx = tip.x - previous.x;
    let dy = tip.y - previous.y;
    let length = (dx * dx + dy * dy).sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(invalid("Class relation marker has a degenerate tangent"));
    }
    let ux = dx / length;
    let uy = dy / length;
    let nx = -uy;
    let ny = ux;
    let direction = if start { -1.0 } else { 1.0 };
    let base = Point::new(tip.x - ux * 18.0 * direction, tip.y - uy * 18.0 * direction);
    let left = Point::new(base.x + nx * 8.0, base.y + ny * 8.0);
    let right = Point::new(base.x - nx * 8.0, base.y - ny * 8.0);
    match marker_type {
        // 0 = aggregation (hollow diamond), 2 = composition (filled diamond).
        0 | 2 => Ok(vec![
            PathSegment::MoveTo { to: tip },
            PathSegment::LineTo { to: left },
            PathSegment::LineTo {
                to: Point::new(tip.x - ux * 36.0 * direction, tip.y - uy * 36.0 * direction),
            },
            PathSegment::LineTo { to: right },
            PathSegment::Close,
        ]),
        1 => Ok(vec![
            PathSegment::MoveTo { to: tip },
            PathSegment::LineTo { to: left },
            PathSegment::LineTo { to: right },
            PathSegment::Close,
        ]),
        3 => Ok(vec![
            PathSegment::MoveTo { to: tip },
            PathSegment::LineTo { to: left },
            PathSegment::LineTo { to: right },
        ]),
        4 => {
            let center = Point::new(tip.x - ux * 10.0 * direction, tip.y - uy * 10.0 * direction);
            let radius = 6.0;
            Ok(vec![
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
            ])
        }
        _ => Ok(Vec::new()),
    }
}

fn edge_labels(edge: &LayoutEdge) -> [Option<&LayoutLabel>; 5] {
    [
        edge.label.as_ref(),
        edge.start_label_left.as_ref(),
        edge.start_label_right.as_ref(),
        edge.end_label_left.as_ref(),
        edge.end_label_right.as_ref(),
    ]
}

fn validate_layout_node(node: &LayoutNode) -> Result<()> {
    if ![node.x, node.y, node.width, node.height]
        .into_iter()
        .all(f64::is_finite)
        || node.width < 0.0
        || node.height < 0.0
    {
        return Err(invalid(format!(
            "Class layout node `{}` has invalid geometry",
            node.id
        )));
    }
    Ok(())
}

fn validate_label(label: &LayoutLabel, edge_id: &str) -> Result<()> {
    if ![label.x, label.y, label.width, label.height]
        .into_iter()
        .all(f64::is_finite)
        || label.width < 0.0
        || label.height < 0.0
    {
        return Err(invalid(format!(
            "Class edge `{edge_id}` has invalid label geometry"
        )));
    }
    Ok(())
}

fn unique_layout_nodes(layout: &ClassDiagramLayout) -> Result<HashMap<&str, &LayoutNode>> {
    let mut nodes = HashMap::with_capacity(layout.nodes.len());
    for node in &layout.nodes {
        if nodes.insert(node.id.as_str(), node).is_some() {
            return Err(invalid(format!(
                "duplicate Class layout node `{}`",
                node.id
            )));
        }
    }
    Ok(nodes)
}

fn unique_layout_edges(layout: &ClassDiagramLayout) -> Result<HashMap<&str, &LayoutEdge>> {
    let mut edges = HashMap::with_capacity(layout.edges.len());
    for edge in &layout.edges {
        if edges.insert(edge.id.as_str(), edge).is_some() {
            return Err(invalid(format!(
                "duplicate Class layout edge `{}`",
                edge.id
            )));
        }
    }
    Ok(edges)
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("Class layout bounds are invalid"));
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
        family: RenderFamilyKind::Class.as_str().to_string(),
        reason: message.into(),
    }
}
