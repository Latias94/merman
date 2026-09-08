//! Renderer-neutral ER diagram adapter.
//!
//! ER layout keeps a separate `render_edges` projection because Dagre may use helper segments
//! for self-loops.  The canonical DrawingList consumes that projection when available, preserving
//! Mermaid's visible relationship edges without exposing internal layout helpers.

use super::{
    ErEdgeSvgMetadata, ErSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar,
    parse_font_families_for, theme_color,
};
use crate::config::{config_bool, config_f64_explicit_css_px, config_string};
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::support::{stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::er::{
    ErBoxLabel, ErEntityMeasure, ErEntityMeasurementSettings, er_box_label_metrics,
    measure_entity_box,
};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, ErDiagramLayout, LayoutCluster, LayoutEdge, LayoutNode};
use crate::render_geometry::{FlowchartCurveKind, emit_flowchart_curve_segments};
use crate::text::TextMeasurer as _;
use crate::{Error, Result};
use base64::Engine as _;
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::er::{
    ErAttributeRenderModel, ErDiagramRenderModel, ErEntityRenderModel, ErRelationshipRenderModel,
};
use merman_display_list::{
    Color, DrawingCommand, DrawingListLimits, DrawingListPolicy, FillRule, FontDescriptor,
    FontStyle, Paint, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle,
    Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

type ErPair = FamilyPair<ErDiagramRenderModel, ErDiagramLayout>;

pub(crate) fn build_er_document(
    pair: &ErPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: DrawingListLimits,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let builder = ErBuilder::new(pair, metadata, policy, limits, session)?;
    builder.build()
}

struct ErBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    model: &'a ErDiagramRenderModel,
    layout: &'a ErDiagramLayout,
    edges: &'a [LayoutEdge],
    entities_by_id: HashMap<&'a str, &'a ErEntityRenderModel>,
    relationships_by_index: HashMap<usize, &'a ErRelationshipRenderModel>,
    font: FontDescriptor,
    font_size: f64,
    relationship_font_size: f64,
    text_obligation: TextObligation,
    node_fill: Color,
    node_stroke: Color,
    node_text: Color,
    line_color: Color,
    cluster_fill: Color,
    cluster_stroke: Color,
    cluster_text: Color,
    title_text: Color,
    edge_label_background: Color,
    row_odd_fill: Color,
    row_even_fill: Color,
    label_style: crate::text::TextStyle,
    attr_style: crate::text::TextStyle,
    entity_measurement: ErEntityMeasurementSettings,
    use_max_width: bool,
    data_look: String,
    relationship_html_labels: bool,
    entity_html_labels: bool,
    output: DrawingListBuilder<'a>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
    edge_metadata: BTreeMap<String, ErEdgeSvgMetadata>,
    marker_types: std::collections::BTreeSet<String>,
}

struct TextEmitSpec {
    origin: Point,
    bounds: Rect,
    style: TextStyle,
    anchor: TextAnchor,
    baseline: TextBaseline,
}

impl TextEmitSpec {
    fn new(
        origin: Point,
        bounds: Rect,
        fill: Color,
        font: FontDescriptor,
        font_size: f64,
        anchor: TextAnchor,
        baseline: TextBaseline,
    ) -> Self {
        Self {
            origin,
            bounds,
            style: TextStyle {
                font,
                font_size,
                letter_spacing: 0.0,
                line_height: (font_size * 1.35).max(1.0),
                fill: Paint::solid(fill),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor,
            baseline,
        }
    }
}

impl<'a> ErBuilder<'a> {
    fn is_elk_layout(&self) -> bool {
        crate::er::ErConfigView::new(self.metadata.effective_config.as_value()).is_elk_layout()
    }

    fn new(
        pair: &'a ErPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
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
        if look.is_neo() {
            return Err(unavailable(
                "neo ER output relies on SVG drop-shadow filters that are not represented by DrawingList v1",
            ));
        }
        if !look.as_str().eq_ignore_ascii_case("classic") {
            return Err(unavailable(format!(
                "ER look `{}` has no defined DrawingList v1 effect mapping",
                look.as_str()
            )));
        }
        if config
            .get("handDrawnSeed")
            .and_then(Value::as_f64)
            .is_some_and(|seed| seed != 0.0)
        {
            return Err(unavailable(
                "ER entity boxes use RoughJS even in classic look, so an explicit handDrawnSeed cannot be represented by DrawingList v1",
            ));
        }
        if config
            .get("theme")
            .and_then(Value::as_str)
            .is_some_and(|theme| matches!(theme, "redux-color" | "redux-dark-color"))
        {
            return Err(unavailable(
                "ER redux color themes require per-entity palette semantics that are not yet represented by DrawingList v1",
            ));
        }
        if config_bool(config, &["themeVariables", "useGradient"]).unwrap_or(false) {
            return Err(unavailable(
                "ER gradients are owned by the SVG cascade and are not yet represented by the portable paint contract",
            ));
        }
        let settings = crate::er::ErConfigView::new(config).render_settings();
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
            families: parse_font_families_for(font_family, RenderFamilyKind::Er)?,
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
        let title_text = theme_color(config, "titleColor", "#333333")?;
        let edge_label_background = theme_color(config, "edgeLabelBackground", "#e8e8e8")?;
        let row_odd_fill = theme_color(config, "rowOdd", "hsl(240, 100%, 100%)")?;
        let row_even_fill = theme_color(config, "rowEven", "hsl(240, 100%, 97.2745098039%)")?;

        unique_layout_nodes(layout)?;
        let edges = if layout.render_edges.is_empty() {
            layout.edges.as_slice()
        } else {
            layout.render_edges.as_slice()
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
            .collect::<HashMap<_, _>>();

        let document_semantic = SemanticAnnotation {
            id: "er.document".to_string(),
            role: SemanticRole::Document,
            title: model
                .acc_title
                .clone()
                .or_else(|| metadata.title.clone())
                .or_else(|| Some(metadata.diagram_type.clone())),
            description: model.acc_descr.clone(),
            link: None,
        };
        let mut builder = Self {
            metadata,
            session,
            model,
            layout,
            edges,
            entities_by_id,
            relationships_by_index,
            font,
            font_size,
            relationship_font_size: 14.0,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            node_fill,
            node_stroke,
            node_text,
            line_color,
            cluster_fill,
            cluster_stroke,
            cluster_text,
            title_text,
            edge_label_background,
            row_odd_fill,
            row_even_fill,
            label_style: settings.label_style.clone(),
            attr_style: settings.attr_style.clone(),
            entity_measurement: settings.entity_measurement,
            use_max_width: settings.use_max_width,
            data_look: settings.diagram_look,
            relationship_html_labels: settings.relationship_html_labels,
            // Mermaid's ER box renderer keeps entity labels in foreignObject shells even when
            // `htmlLabels` is false; that flag changes measurement/padding, not this DOM shape.
            entity_html_labels: true,
            output: DrawingListBuilder::new(policy, limits, session),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
            edge_metadata: BTreeMap::new(),
            marker_types: std::collections::BTreeSet::new(),
        };
        builder.output.push_semantic(document_semantic)?;
        builder.output.push_control(DrawingCommand::Save)?;
        builder
            .output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "er.document".to_string(),
            })?;
        builder.preflight()?;
        Ok(builder)
    }

    fn build(mut self) -> Result<RenderDocument> {
        let edges = self.edges;
        self.semantic_classes
            .insert("er.root".to_string(), "root".to_string());
        self.semantic_classes
            .insert("er.clusters".to_string(), "clusters".to_string());
        self.semantic_classes.insert(
            "er.edges".to_string(),
            if self.is_elk_layout() {
                "edges edgePath"
            } else {
                "edgePaths"
            }
            .to_string(),
        );
        self.semantic_classes
            .insert("er.edgeLabels".to_string(), "edgeLabels".to_string());
        self.semantic_classes
            .insert("er.nodes".to_string(), "nodes".to_string());
        for (id, description) in [
            ("er.root", "ER diagram root"),
            ("er.clusters", "ER subgraph clusters"),
            ("er.edges", "ER relationship edges"),
            ("er.edgeLabels", "ER relationship labels"),
            ("er.nodes", "ER entities"),
        ] {
            self.push_semantic(SemanticAnnotation {
                id: id.to_string(),
                role: SemanticRole::Group,
                title: None,
                description: Some(description.to_string()),
                link: None,
            })?;
        }
        self.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "er.root".to_string(),
        })?;

        // ELK's common painter lowers edges before clusters; Dagre keeps clusters before edges.
        // The public command order follows the same source-backed z-order so SVG and native hosts
        // observe identical overlap semantics.
        if self.is_elk_layout() {
            self.push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "er.edges".to_string(),
            })?;
            for (index, edge) in edges.iter().enumerate() {
                self.session.checkpoint(OperationPhase::Emit)?;
                self.emit_edge(index, edge)?;
            }
            self.push_control(DrawingCommand::EndSemanticGroup)?;
            self.push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "er.clusters".to_string(),
            })?;
            for (index, cluster) in self.layout.clusters.iter().enumerate() {
                self.session.checkpoint(OperationPhase::Emit)?;
                self.emit_cluster(index, cluster)?;
            }
            self.push_control(DrawingCommand::EndSemanticGroup)?;
        } else {
            self.push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "er.clusters".to_string(),
            })?;
            for (index, cluster) in self.layout.clusters.iter().enumerate() {
                self.session.checkpoint(OperationPhase::Emit)?;
                self.emit_cluster(index, cluster)?;
            }
            self.push_control(DrawingCommand::EndSemanticGroup)?;
            self.push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "er.edges".to_string(),
            })?;
            for (index, edge) in edges.iter().enumerate() {
                self.session.checkpoint(OperationPhase::Emit)?;
                self.emit_edge(index, edge)?;
            }
            self.push_control(DrawingCommand::EndSemanticGroup)?;
        }
        self.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "er.edgeLabels".to_string(),
        })?;
        for (index, edge) in edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge_label(index, edge)?;
        }
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        self.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "er.nodes".to_string(),
        })?;
        for (index, node) in self.layout.nodes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if node.is_cluster || node.id.contains("---") {
                continue;
            }
            self.emit_entity(index, node)?;
        }
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        if let Some(title) = self.diagram_title() {
            self.emit_title(&title)?;
        }
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        self.push_control(DrawingCommand::Restore)?;

        let viewport = self.viewport(self.diagram_title().as_deref())?;
        let document = self.output.finish(
            Viewport::new(viewport),
            BTreeMap::from([(
                "x-merman-er".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "direction": self.model.direction,
                    "geometry_subset": "entities-attribute-tables-relationships-cardinality-subgraphs",
                    "label_mode": "plain_host_text",
                }),
            )]),
        )?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Er,
                body: SvgStructureBody::Er(ErSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.use_max_width,
                    data_look: self.data_look.clone(),
                    relationship_html_labels: self.relationship_html_labels,
                    entity_html_labels: self.entity_html_labels,
                    semantic_classes: std::mem::take(&mut self.semantic_classes),
                    path_classes: std::mem::take(&mut self.path_classes),
                    text_classes: std::mem::take(&mut self.text_classes),
                    dom_ids: std::mem::take(&mut self.dom_ids),
                    edge_metadata: std::mem::take(&mut self.edge_metadata),
                    marker_types: std::mem::take(&mut self.marker_types),
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
            if !entity.shape.is_empty() && entity.shape != "erBox" {
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
        if let Some(title) = self.diagram_title() {
            plain_text(&title).map_err(unavailable)?;
        }
        for node in &self.layout.nodes {
            validate_layout_node(node)?;
        }
        for edge in self.edges {
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
        self.semantic_classes
            .insert(semantic_id.clone(), "cluster".to_string());
        self.path_classes.insert(
            format!("{semantic_id}.box"),
            "basic label-container".to_string(),
        );
        self.text_classes
            .insert(semantic_id.clone(), "nodeLabel".to_string());
        self.dom_ids.insert(semantic_id.clone(), cluster.id.clone());
        self.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        })?;
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
                TextEmitSpec::new(
                    Point::new(cluster.title_label.x, cluster.title_label.y),
                    centered_rect(
                        cluster.title_label.x,
                        cluster.title_label.y,
                        cluster.title_label.width,
                        cluster.title_label.height,
                    ),
                    self.cluster_text,
                    self.font.clone(),
                    self.font_size,
                    TextAnchor::Middle,
                    TextBaseline::Middle,
                ),
            )?;
        }
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        self.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: Some(title),
            description: Some(format!("ER subgraph {}", cluster.id)),
            link: None,
        })?;
        Ok(())
    }

    fn emit_edge(&mut self, index: usize, edge: &LayoutEdge) -> Result<()> {
        let relation = relationship_for_edge(edge, &self.relationships_by_index);
        let semantic_id = format!("er.edge.{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "edgePath".to_string());
        let pattern = if edge.stroke_dasharray.as_deref() == Some("8,8") {
            "dashed"
        } else {
            "solid"
        };
        self.path_classes.insert(
            format!("{semantic_id}.route"),
            format!("edge-thickness-normal edge-pattern-{pattern} relationshipLine"),
        );
        self.path_classes
            .insert(format!("{semantic_id}.marker.start"), "marker".to_string());
        self.path_classes
            .insert(format!("{semantic_id}.marker.end"), "marker".to_string());
        let edge_path_id = format!("{semantic_id}.route");
        let data_points = base64::engine::general_purpose::STANDARD.encode(
            serde_json::to_vec(&edge.points)
                .map_err(|_| invalid("failed to encode ER edge points"))?,
        );
        for marker in [edge.start_marker.as_deref(), edge.end_marker.as_deref()]
            .into_iter()
            .flatten()
        {
            if let Some(marker_type) = marker_type_name(marker) {
                self.marker_types.insert(marker_type.to_string());
            }
        }
        self.edge_metadata.insert(
            edge_path_id.clone(),
            ErEdgeSvgMetadata {
                dom_id: edge_dom_id(edge, &self.model.relationships),
                data_points,
                start_marker: edge.start_marker.clone(),
                end_marker: edge.end_marker.clone(),
            },
        );
        self.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        })?;
        let mut line = stroke(self.line_color, 1.0);
        if edge.stroke_dasharray.as_deref() == Some("8,8") {
            line.dash_array = vec![8.0, 8.0];
        }
        self.add_path_with(
            edge_path_id,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(line),
            },
            |emit| {
                emit_flowchart_curve_segments(
                    &edge.points,
                    FlowchartCurveKind::Basis,
                    0.0,
                    false,
                    None,
                    emit,
                )
            },
        )?;
        if let Some(marker) = edge.start_marker.as_deref() {
            self.emit_cardinality_marker(&semantic_id, edge, marker, true)?;
        }
        if let Some(marker) = edge.end_marker.as_deref() {
            self.emit_cardinality_marker(&semantic_id, edge, marker, false)?;
        }
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        let title = relation
            .and_then(|relation| {
                (!relation.role_a.trim().is_empty()).then(|| plain_text(&relation.role_a).ok())
            })
            .flatten();
        self.push_semantic(SemanticAnnotation {
            id: semantic_id.clone(),
            role: SemanticRole::Edge,
            title,
            description: relation
                .map(|relation| format!("{} → {}", relation.entity_a, relation.entity_b))
                .or_else(|| Some(format!("{} → {}", edge.from, edge.to))),
            link: None,
        })?;
        Ok(())
    }

    fn emit_edge_label(&mut self, index: usize, edge: &LayoutEdge) -> Result<()> {
        let Some(relation) = relationship_for_edge(edge, &self.relationships_by_index) else {
            return Ok(());
        };
        let Some(label) = edge.label.as_ref() else {
            return Ok(());
        };
        if relation.role_a.trim().is_empty() {
            return Ok(());
        }
        let semantic_id = format!("er.edge.{index}.label");
        self.semantic_classes
            .insert(semantic_id.clone(), "edgeLabel".to_string());
        self.path_classes.insert(
            format!("{semantic_id}.background"),
            "background".to_string(),
        );
        self.text_classes
            .insert(semantic_id.clone(), "edgeLabel".to_string());
        self.dom_ids.insert(
            semantic_id.clone(),
            edge_dom_id(edge, &self.model.relationships),
        );
        self.emit_label(
            &semantic_id,
            &relation.role_a,
            label,
            "ER relationship label".to_string(),
        )
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
        self.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        })?;
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
            TextEmitSpec::new(
                Point::new(label.x, label.y),
                bounds,
                self.node_text,
                self.font.clone(),
                self.relationship_font_size,
                TextAnchor::Middle,
                TextBaseline::Middle,
            ),
        )?;
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        self.push_semantic(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Label,
            title: Some(text),
            description: Some(description),
            link: None,
        })?;
        Ok(())
    }

    fn diagram_title(&self) -> Option<String> {
        self.metadata
            .title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned)
    }

    fn title_top_margin(&self) -> f64 {
        crate::er::ErConfigView::new(self.metadata.effective_config.as_value())
            .render_settings()
            .title_top_margin
            .max(0.0)
    }

    fn viewport(&self, title: Option<&str>) -> Result<Rect> {
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("ER layout did not provide root bounds"))?;
        let mut min_x = bounds.min_x;
        let mut min_y = bounds.min_y;
        let mut max_x = bounds.max_x;
        let mut max_y = bounds.max_y;
        if let Some(title) = title {
            let style = crate::text::TextStyle {
                font_family: Some(self.font.families.join(", ")),
                font_size: self.font_size,
                font_weight: Some("700".to_string()),
                font_style: Some("normal".to_string()),
            };
            let measurer = self
                .session
                .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
            let width = measurer
                .measure_svg_raw_text_bbox_width_px(title, &style)
                .max(1.0);
            let height = measurer
                .measure_svg_simple_text_bbox_height_px(title, &style)
                .max(1.0);
            let x = (bounds.min_x + bounds.max_x) / 2.0;
            let y = bounds.min_y - self.title_top_margin();
            min_x = min_x.min(x - width / 2.0);
            max_x = max_x.max(x + width / 2.0);
            min_y = min_y.min(y - height);
            max_y = max_y.max(y);
        }
        let padding = self
            .metadata
            .effective_config
            .as_value()
            .get("er")
            .and_then(|er| er.get("diagramPadding"))
            .and_then(Value::as_f64)
            .unwrap_or(20.0)
            .max(0.0);
        Ok(Rect::new(
            min_x - padding,
            min_y - padding,
            (max_x - min_x + 2.0 * padding).max(1.0),
            (max_y - min_y + 2.0 * padding).max(1.0),
        ))
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("ER layout did not provide root bounds"))?;
        let origin = Point::new(
            (bounds.min_x + bounds.max_x) / 2.0,
            bounds.min_y - self.title_top_margin(),
        );
        self.semantic_classes
            .insert("er.title".to_string(), "erDiagramTitleText".to_string());
        self.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "er.title".to_string(),
        })?;
        self.draw_text(
            title,
            TextEmitSpec::new(
                origin,
                Rect::new(
                    bounds.min_x,
                    origin.y - self.font_size,
                    (bounds.max_x - bounds.min_x).max(1.0),
                    self.font_size,
                ),
                self.title_text,
                FontDescriptor {
                    weight: 700,
                    ..self.font.clone()
                },
                self.font_size,
                TextAnchor::Middle,
                TextBaseline::Alphabetic,
            ),
        )?;
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        self.text_classes
            .insert("er.title".to_string(), "erDiagramTitleText".to_string());
        self.push_semantic(SemanticAnnotation {
            id: "er.title".to_string(),
            role: SemanticRole::Label,
            title: Some(title.to_string()),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_entity(&mut self, index: usize, layout_node: &LayoutNode) -> Result<()> {
        let entity = self
            .entities_by_id
            .get(layout_node.id.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("ER layout node `{}` has no entity", layout_node.id)))?;
        let measure = self.measure_entity(entity);
        if (measure.width - layout_node.width).abs() > 1e-3
            || (measure.height - layout_node.height).abs() > 1e-3
        {
            return Err(invalid(format!(
                "ER entity measured size mismatch for {}: layout=({},{}), measure=({}, {})",
                entity.id, layout_node.width, layout_node.height, measure.width, measure.height
            )));
        }
        let semantic_id = format!("er.entity.{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "node default".to_string());
        self.dom_ids.insert(semantic_id.clone(), entity.id.clone());
        self.path_classes.insert(
            format!("{semantic_id}.box"),
            "basic label-container".to_string(),
        );
        let text_class = if entity.attributes.is_empty() {
            "nodeLabel markdown-node-label"
        } else {
            "nodeLabel"
        };
        self.text_classes
            .insert(semantic_id.clone(), text_class.to_string());
        self.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        })?;
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
        if entity.attributes.is_empty() {
            let label_width = measure.label_html_width.max(0.0);
            let label_height = measure.label_height.max(1.0);
            self.text_classes
                .insert(format!("{semantic_id}#0"), text_class.to_string());
            self.draw_text(
                &name,
                TextEmitSpec::new(
                    Point::new(layout_node.x, layout_node.y),
                    centered_rect(layout_node.x, layout_node.y, label_width, label_height),
                    self.node_text,
                    self.font.clone(),
                    self.font_size,
                    TextAnchor::Middle,
                    TextBaseline::Middle,
                ),
            )?;
        } else {
            self.emit_entity_attribute_table(&semantic_id, &measure, bounds)?;
        }
        self.push_control(DrawingCommand::EndSemanticGroup)?;
        self.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(name),
            description: Some(format!("Entity {}", entity.id)),
            link: None,
        })?;
        Ok(())
    }

    fn measure_entity(&self, entity: &ErEntityRenderModel) -> ErEntityMeasure {
        let measurer = self.session.text_measurer(TextMeasurementPhase::Layout);
        measure_entity_box(
            entity,
            &measurer,
            &self.label_style,
            &self.attr_style,
            self.entity_measurement,
        )
    }

    fn emit_entity_attribute_table(
        &mut self,
        semantic_id: &str,
        measure: &ErEntityMeasure,
        bounds: Rect,
    ) -> Result<()> {
        let line_height = (self.font_size * 1.5).max(1.0);
        let name_row_height = (measure.label_height + measure.text_padding).max(1.0);
        let separator_y = bounds.y + name_row_height;
        let mut row_top = separator_y;
        for (row_index, row) in measure.rows.iter().enumerate() {
            let row_height = row.height.max(1.0);
            let row_id = format!("{semantic_id}.row.{row_index}");
            self.path_classes.insert(
                row_id.clone(),
                if row_index % 2 == 0 {
                    "row-rect-odd"
                } else {
                    "row-rect-even"
                }
                .to_string(),
            );
            self.add_path(
                row_id,
                rectangle_path(Rect::new(bounds.x, row_top, bounds.width, row_height)),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(if row_index % 2 == 0 {
                        self.row_odd_fill
                    } else {
                        self.row_even_fill
                    })),
                    stroke: None,
                },
            )?;
            row_top += row_height;
        }

        let name_width = measure.label_html_width.max(0.0);
        let name_bounds = Rect::new(
            bounds.x + (bounds.width - name_width) / 2.0,
            bounds.y + name_row_height / 2.0 - line_height / 2.0,
            name_width,
            line_height,
        );
        self.text_classes
            .insert(format!("{semantic_id}#0"), "nodeLabel".to_string());
        self.draw_text(
            measure.label.rendered_text(),
            TextEmitSpec::new(
                Point::new(
                    bounds.x + bounds.width / 2.0,
                    name_bounds.y + line_height / 2.0,
                ),
                name_bounds,
                self.node_text,
                self.font.clone(),
                self.font_size,
                TextAnchor::Middle,
                TextBaseline::Middle,
            ),
        )?;

        let padding = if self.entity_measurement.html_labels_raw {
            self.entity_measurement.diagram_padding
        } else {
            self.entity_measurement.diagram_padding * 1.25
        };
        let left_text_x = bounds.x + padding / 2.0;
        let cell_xs = [
            left_text_x,
            left_text_x + measure.type_col_w,
            left_text_x + measure.type_col_w + measure.name_col_w,
            left_text_x + measure.type_col_w + measure.name_col_w + measure.key_col_w,
        ];
        let measurer = self.session.text_measurer(TextMeasurementPhase::Layout);
        let mut row_y = separator_y;
        for (row_index, row) in measure.rows.iter().enumerate() {
            let row_height = row.height.max(1.0);
            let cell_y = row_y + row_height / 2.0 - line_height / 2.0;
            let cells = [
                (
                    row.type_label.rendered_text(),
                    er_box_label_metrics(&row.type_label, &measurer, &self.attr_style)
                        .width
                        .max(0.0),
                ),
                (
                    row.name_label.rendered_text(),
                    er_box_label_metrics(&row.name_label, &measurer, &self.attr_style)
                        .width
                        .max(0.0),
                ),
                (
                    row.key_label.rendered_text(),
                    er_box_label_metrics(&row.key_label, &measurer, &self.attr_style)
                        .width
                        .max(0.0),
                ),
                (
                    row.comment_label.rendered_text(),
                    er_box_label_metrics(&row.comment_label, &measurer, &self.attr_style)
                        .width
                        .max(0.0),
                ),
            ];
            for (cell_index, (text, width)) in cells.into_iter().enumerate() {
                let text_index = 1 + row_index * 4 + cell_index;
                self.text_classes.insert(
                    format!("{semantic_id}#{text_index}"),
                    "nodeLabel".to_string(),
                );
                let height = if text.trim().is_empty() {
                    0.0
                } else {
                    line_height
                };
                self.draw_text(
                    text,
                    TextEmitSpec::new(
                        Point::new(cell_xs[cell_index], cell_y + line_height / 2.0),
                        Rect::new(cell_xs[cell_index], cell_y, width, height),
                        self.node_text,
                        self.font.clone(),
                        self.font_size,
                        TextAnchor::Start,
                        TextBaseline::Middle,
                    ),
                )?;
            }
            row_y += row_height;
        }

        let mut divider_xs = vec![bounds.x + measure.type_col_w];
        if measure.has_key {
            divider_xs.push(bounds.x + measure.type_col_w + measure.name_col_w);
        }
        if measure.has_comment {
            divider_xs.push(bounds.x + measure.type_col_w + measure.name_col_w + measure.key_col_w);
        }
        for (index, x) in divider_xs.into_iter().enumerate() {
            let divider_id = format!("{semantic_id}.divider.column.{index}");
            self.path_classes
                .insert(divider_id.clone(), "divider".to_string());
            self.add_path(
                divider_id,
                line_path(
                    Point::new(x, separator_y),
                    Point::new(x, bounds.y + bounds.height),
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(self.node_stroke, 1.0)),
                },
            )?;
        }

        let header_id = format!("{semantic_id}.divider.header");
        self.path_classes
            .insert(header_id.clone(), "divider".to_string());
        self.add_path(
            header_id,
            line_path(
                Point::new(bounds.x, separator_y),
                Point::new(bounds.x + bounds.width, separator_y),
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.node_stroke, 1.0)),
            },
        )?;
        let mut row_y = separator_y;
        for (row_index, row) in measure.rows.iter().enumerate() {
            if row_index + 1 == measure.rows.len() {
                continue;
            }
            let row_height = row.height.max(1.0);
            let divider_id = format!("{semantic_id}.divider.row.{row_index}");
            self.path_classes
                .insert(divider_id.clone(), "divider".to_string());
            self.add_path(
                divider_id,
                line_path(
                    Point::new(bounds.x, row_y + row_height),
                    Point::new(bounds.x + bounds.width, row_y + row_height),
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(self.node_stroke, 1.0)),
                },
            )?;
            row_y += row_height;
        }
        Ok(())
    }

    fn push_control(&mut self, command: DrawingCommand) -> Result<()> {
        self.output.push_control(command)
    }

    fn push_semantic(&mut self, semantic: SemanticAnnotation) -> Result<()> {
        self.output.push_semantic(semantic)
    }

    fn draw_text(&mut self, text: &str, spec: TextEmitSpec) -> Result<()> {
        let obligation = self.text_obligation.clone();
        self.output.draw_host_text(text, |text| TextRun {
            text,
            origin: spec.origin,
            bounds: spec.bounds,
            style: spec.style,
            anchor: spec.anchor,
            baseline: spec.baseline,
            direction: TextDirection::Auto,
            language: None,
            obligation,
        })
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        self.output.draw_path(ResourceId::new(id), segments, style)
    }

    fn add_path_with(
        &mut self,
        id: String,
        style: PathStyle,
        emit: impl FnOnce(&mut dyn FnMut(PathSegment) -> Result<()>) -> Result<()>,
    ) -> Result<()> {
        self.output.draw_path_with(ResourceId::new(id), style, emit)
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

fn edge_dom_id(edge: &LayoutEdge, relationships: &[ErRelationshipRenderModel]) -> String {
    let Some(rest) = edge.id.strip_prefix("er-rel-") else {
        return edge.id.clone();
    };
    let Some(index) = rest
        .split('-')
        .next()
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return edge.id.clone();
    };
    let Some(relation) = relationships.get(index) else {
        return edge.id.clone();
    };
    format!("id_{}_{}_{}", relation.entity_a, relation.entity_b, index)
}

fn marker_type_name(marker: &str) -> Option<&'static str> {
    let base = marker
        .trim()
        .strip_suffix("_START")
        .or_else(|| marker.trim().strip_suffix("_END"))?;
    match base {
        "ONLY_ONE" => Some("onlyOne"),
        "ZERO_OR_ONE" => Some("zeroOrOne"),
        "ONE_OR_MORE" => Some("oneOrMore"),
        "ZERO_OR_MORE" => Some("zeroOrMore"),
        "MD_PARENT" => Some("mdParent"),
        _ => None,
    }
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
    let label = ErBoxLabel::from_source(raw);
    let rooted = format!(
        "<merman-fragment>{}</merman-fragment>",
        label.xhtml_fragment()
    );
    let document = roxmltree::Document::parse(&rooted)
        .map_err(|_| "contains invalid XHTML label markup".to_string())?;
    let root = document.root_element();
    let mut output = String::new();
    let mut paragraph_seen = false;
    for child in root.children() {
        if child.is_text() {
            let text = child.text().unwrap_or_default();
            if paragraph_seen && !text.trim().is_empty() {
                return Err("contains text outside its portable paragraph".to_string());
            }
            if !paragraph_seen {
                output.push_str(text);
            }
            continue;
        }
        if !child.is_element()
            || paragraph_seen
            || child.tag_name().name() != "p"
            || child.attributes().next().is_some()
        {
            return Err("contains styled Markdown or HTML markup".to_string());
        }
        paragraph_seen = true;
        for inline in child.children() {
            if inline.is_text() {
                output.push_str(inline.text().unwrap_or_default());
            } else if inline.is_element()
                && inline.tag_name().name() == "br"
                && inline.attributes().next().is_none()
                && inline.children().next().is_none()
            {
                output.push('\n');
            } else {
                return Err("contains styled Markdown or HTML markup".to_string());
            }
        }
    }
    Ok(output.lines().map(str::trim).collect::<Vec<_>>().join("\n"))
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

fn unique_layout_nodes(layout: &ErDiagramLayout) -> Result<HashMap<&str, &LayoutNode>> {
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
    let combined = (u16::from(color.alpha) * u16::from(alpha) / 255) as u8;
    Color::rgba(color.red, color.green, color.blue, combined)
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
