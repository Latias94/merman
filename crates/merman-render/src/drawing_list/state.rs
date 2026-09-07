//! Renderer-neutral State diagram adapter.
//!
//! This first State slice admits only shapes whose classic renderer is already deterministic
//! typed geometry. Mermaid currently sends state-end, choice, fork, and join shapes through
//! RoughJS even under the classic look, so those remain explicit capability errors until their
//! randomness and generated paths move behind the canonical document seam.

use super::{
    RenderDocument, StateSvgBody, SvgStructureBody, SvgStructureSidecar, parse_font_families_for,
};
use crate::config::{MERMAID_DEFAULT_FONT_FAMILY_CSS, config_diagram_look};
use crate::drawing_list::flowchart::{ellipse_path, polygon_path, rounded_rect_path};
use crate::drawing_list::support::{PortableStyleResolver, stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, LayoutEdge, LayoutNode, StateDiagramLayout};
use crate::render_geometry::{FlowchartCurveKind, flowchart_curve_segments, state_curve_points};
use crate::state::{StateConfigView, state_plain_text_label, state_value_to_label_text};
use crate::theme::PresentationTheme;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Viewport,
};
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};

type StatePair = FamilyPair<StateDiagramRenderModel, StateDiagramLayout>;

const VIEWPORT_PADDING: f64 = 8.0;
const CLASSIC_NODE_RADIUS: f64 = 5.0;

pub(crate) fn build_state_document(
    pair: &StatePair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    let mut builder = StateBuilder::new(pair, metadata, policy, session)?;
    builder.build()
}

struct StateBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a StateDiagramRenderModel,
    layout: &'a StateDiagramLayout,
    nodes_by_id: HashMap<&'a str, &'a LayoutNode>,
    edges_by_id: HashMap<&'a str, &'a LayoutEdge>,
    font: FontDescriptor,
    font_size: f64,
    line_height: f64,
    html_labels: bool,
    state_padding: f64,
    text_obligation: TextObligation,
    state_fill: Color,
    state_border: Color,
    state_label: Color,
    special_state: Color,
    transition: Color,
    transition_label: Color,
    marker_fill: Color,
    edge_label_background: Color,
    stroke_width: f64,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    semantic_looks: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
}

impl<'a> StateBuilder<'a> {
    fn new(
        pair: &'a StatePair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let model = pair.semantic();
        let layout = pair.layout();
        let config = metadata.effective_config.as_value();
        let look = config_diagram_look(config);
        if look.as_str() != "classic" {
            return Err(unavailable(format!(
                "look `{look}` uses effects whose paths are not yet owned by the canonical State document"
            )));
        }
        if metadata
            .title
            .as_deref()
            .is_some_and(|title| !title.trim().is_empty())
        {
            return Err(unavailable(
                "State diagram titles require canonical root-title measurement before DrawingList v1 can preserve their viewport",
            ));
        }
        let bounds = layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("State layout did not provide root bounds"))?;
        validate_bounds(bounds)?;

        let render_settings = StateConfigView::new(config).render_settings();
        let render_text = render_settings.text_style;
        if !render_text.font_size.is_finite() || render_text.font_size <= 0.0 {
            return Err(invalid("State font size is invalid"));
        }
        let raw_family = render_text
            .font_family
            .as_deref()
            .unwrap_or(MERMAID_DEFAULT_FONT_FAMILY_CSS);
        let font = FontDescriptor {
            families: parse_font_families_for(raw_family, RenderFamilyKind::State)?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        let theme = PresentationTheme::new(config).state_drawing();
        let styles = PortableStyleResolver::new("state");
        let state_fill = styles.color("stateBkg", &theme.state_bkg)?;
        let state_border = styles.color("stateBorder", &theme.state_border)?;
        let state_label = styles.color("stateLabelColor", &theme.state_label_color)?;
        let special_state = styles.color("specialStateColor", &theme.special_state_color)?;
        let transition = styles.color("transitionColor", &theme.transition_color)?;
        let transition_label =
            styles.color("transitionLabelColor", &theme.transition_label_color)?;
        let marker_fill = styles.color("lineColor", &theme.marker_fill)?;
        let edge_label_background =
            styles.color("edgeLabelBackground", &theme.edge_label_background)?;
        let stroke_width = styles.length("strokeWidth", &theme.stroke_width)?;

        let nodes_by_id = unique_layout_nodes(layout)?;
        let edges_by_id = unique_layout_edges(layout)?;
        let builder = Self {
            metadata,
            session,
            policy,
            model,
            layout,
            nodes_by_id,
            edges_by_id,
            font,
            font_size: render_text.font_size,
            line_height: if render_settings.html_labels {
                render_text.font_size * 1.5
            } else {
                render_text.font_size * 1.1
            },
            html_labels: render_settings.html_labels,
            state_padding: render_settings.state_padding,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            state_fill,
            state_border,
            state_label,
            special_state,
            transition,
            transition_label,
            marker_fill,
            edge_label_background,
            stroke_width,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "state.document".to_string(),
                },
            ],
            semantics: Vec::new(),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            semantic_looks: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
        };
        builder.preflight()?;
        Ok(builder)
    }

    fn build(&mut self) -> Result<RenderDocument> {
        self.session.checkpoint(OperationPhase::Emit)?;
        self.semantics.push(SemanticAnnotation {
            id: "state.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        });
        self.semantic_looks
            .insert("state.document".to_string(), "classic".to_string());

        // Match Mermaid's painter order: transition paths, transition labels, then nodes.
        for (index, edge) in self.model.edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge_route(index, edge)?;
        }
        for (index, edge) in self.model.edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge_label(index, edge)?;
        }
        for (index, node) in self.model.nodes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_node(index, node)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                bounds.min_x - VIEWPORT_PADDING,
                bounds.min_y - VIEWPORT_PADDING,
                (bounds.max_x - bounds.min_x) + 2.0 * VIEWPORT_PADDING,
                (bounds.max_y - bounds.min_y) + 2.0 * VIEWPORT_PADDING,
            )),
            policy: self.policy,
            resources: std::mem::take(&mut self.resources),
            commands: std::mem::take(&mut self.commands),
            semantics: std::mem::take(&mut self.semantics),
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-state".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "direction": self.model.direction,
                    "look": "classic",
                    "label_mode": "plain_host_text",
                    "geometry_subset": "start-rect-transition",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::State,
                body: SvgStructureBody::State(StateSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    semantic_classes: std::mem::take(&mut self.semantic_classes),
                    path_classes: std::mem::take(&mut self.path_classes),
                    text_classes: std::mem::take(&mut self.text_classes),
                    semantic_looks: std::mem::take(&mut self.semantic_looks),
                    dom_ids: std::mem::take(&mut self.dom_ids),
                }),
            },
        })
    }

    fn preflight(&self) -> Result<()> {
        if !self.layout.clusters.is_empty() || self.model.nodes.iter().any(|node| node.is_group) {
            return Err(unavailable(
                "composite State roots and clusters require canonical nested-root geometry",
            ));
        }
        if !self.model.links.is_empty() {
            return Err(unavailable(
                "State links require resolved navigation metadata and SVG wrapper sidecar ownership",
            ));
        }
        if !self.model.style_classes.is_empty() {
            return Err(unavailable(
                "State classDef styles have not been resolved into portable DrawingList paint state",
            ));
        }
        for state in self.model.states.values() {
            if state.note.is_some() {
                return Err(unavailable(
                    "State notes require note clusters and note-edge geometry",
                ));
            }
            if !state.classes.is_empty()
                || !state.styles.is_empty()
                || !state.text_styles.is_empty()
            {
                return Err(unavailable(
                    "State-local classes and styles are not yet resolved into DrawingList paint state",
                ));
            }
        }
        if self.layout.nodes.len() != self.model.nodes.len() {
            return Err(unavailable(format!(
                "State layout has {} nodes for {} semantic nodes; helper geometry is outside the canonical subset",
                self.layout.nodes.len(),
                self.model.nodes.len()
            )));
        }
        if self.layout.edges.len() != self.model.edges.len() {
            return Err(unavailable(format!(
                "State layout has {} edges for {} semantic edges; helper geometry is outside the canonical subset",
                self.layout.edges.len(),
                self.model.edges.len()
            )));
        }

        let mut semantic_node_ids = HashSet::with_capacity(self.model.nodes.len());
        for node in &self.model.nodes {
            if !semantic_node_ids.insert(node.id.as_str()) {
                return Err(invalid(format!("duplicate State node id `{}`", node.id)));
            }
            self.validate_node(node)?;
        }
        let mut semantic_edge_ids = HashSet::with_capacity(self.model.edges.len());
        for edge in &self.model.edges {
            if !semantic_edge_ids.insert(edge.id.as_str()) {
                return Err(invalid(format!("duplicate State edge id `{}`", edge.id)));
            }
            self.validate_edge(edge)?;
        }
        Ok(())
    }

    fn validate_node(&self, node: &StateDiagramRenderNode) -> Result<()> {
        if node.parent_id.is_some()
            || node.position.is_some()
            || node.dir.is_some()
            || node.explicit_dir.is_some()
        {
            return Err(unavailable(format!(
                "State node `{}` carries composite or note placement metadata",
                node.id
            )));
        }
        if !node.label_style.trim().is_empty()
            || !node.css_compiled_styles.is_empty()
            || !node.css_styles.is_empty()
        {
            return Err(unavailable(format!(
                "State node `{}` uses unresolved inline style declarations",
                node.id
            )));
        }
        if node
            .css_classes
            .split_whitespace()
            .any(|class| class != "statediagram-state")
        {
            return Err(unavailable(format!(
                "State node `{}` uses custom CSS classes",
                node.id
            )));
        }
        match node.shape.as_str() {
            "stateStart" => {}
            "rect" => {
                let raw = state_node_label_text(node)?;
                state_plain_text_label(&raw).ok_or_else(|| {
                    unavailable(format!(
                        "State node `{}` label contains Markdown, HTML, an image, or multiline markup that cannot be flattened",
                        node.id
                    ))
                })?;
            }
            "stateEnd" | "choice" | "fork" | "join" => {
                return Err(unavailable(format!(
                    "State shape `{}` currently uses RoughJS path generation even in classic mode",
                    node.shape
                )));
            }
            _ => {
                return Err(unavailable(format!(
                    "State shape `{}` has no lossless DrawingList v1 adapter",
                    node.shape
                )));
            }
        }

        let layout = self
            .nodes_by_id
            .get(node.id.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("State node `{}` has no layout geometry", node.id)))?;
        if layout.is_cluster
            || ![layout.x, layout.y, layout.width, layout.height]
                .into_iter()
                .all(f64::is_finite)
            || layout.width <= 0.0
            || layout.height <= 0.0
        {
            return Err(invalid(format!(
                "State node `{}` has invalid layout geometry",
                node.id
            )));
        }
        Ok(())
    }

    fn validate_edge(&self, edge: &StateDiagramRenderEdge) -> Result<()> {
        if edge.start == edge.end {
            return Err(unavailable(format!(
                "State self-loop `{}` requires canonical self-loop clipping and placeholder ownership",
                edge.id
            )));
        }
        if edge
            .classes
            .split_whitespace()
            .any(|class| class != "transition")
        {
            return Err(unavailable(format!(
                "State edge `{}` uses an unsupported class or note-edge style",
                edge.id
            )));
        }
        if edge.arrow_type_end.trim() != "arrow_barb" {
            return Err(unavailable(format!(
                "State edge `{}` uses unsupported marker `{}`",
                edge.id, edge.arrow_type_end
            )));
        }
        if !edge.label.trim().is_empty() {
            state_plain_text_label(edge.label.trim()).ok_or_else(|| {
                unavailable(format!(
                    "State edge `{}` label contains Markdown, HTML, an image, or multiline markup that cannot be flattened",
                    edge.id
                ))
            })?;
        }

        let layout = self
            .edges_by_id
            .get(edge.id.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("State edge `{}` has no layout geometry", edge.id)))?;
        if layout.from != edge.start || layout.to != edge.end {
            return Err(invalid(format!(
                "State edge `{}` layout endpoints do not match its semantic endpoints",
                edge.id
            )));
        }
        if layout.from_cluster.is_some()
            || layout.to_cluster.is_some()
            || layout.start_label_left.is_some()
            || layout.start_label_right.is_some()
            || layout.end_label_left.is_some()
            || layout.end_label_right.is_some()
            || layout.start_marker.is_some()
            || layout.end_marker.is_some()
            || layout.stroke_dasharray.is_some()
        {
            return Err(unavailable(format!(
                "State edge `{}` carries cluster, terminal-label, marker, or dash metadata outside the admitted subset",
                edge.id
            )));
        }
        if layout.points.len() < 2
            || layout
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(invalid(format!(
                "State edge `{}` has invalid route points",
                edge.id
            )));
        }
        match (edge.label.trim().is_empty(), layout.label.as_ref()) {
            (false, Some(label)) => validate_label_bounds(label, &edge.id)?,
            (false, None) => {
                return Err(invalid(format!(
                    "State edge `{}` has text but no label layout",
                    edge.id
                )));
            }
            (true, Some(_)) => {
                return Err(invalid(format!(
                    "State edge `{}` has label layout without visible text",
                    edge.id
                )));
            }
            (true, None) => {}
        }
        Ok(())
    }

    fn emit_edge_route(&mut self, index: usize, edge: &StateDiagramRenderEdge) -> Result<()> {
        let layout = self.edges_by_id[edge.id.as_str()];
        let points = state_curve_points(&layout.points, Some(edge.arrow_type_end.as_str()));
        let route_segments =
            flowchart_curve_segments(&points, FlowchartCurveKind::Basis, 0.0, false, None);
        let semantic_id = format!("state.edge.{index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "edgePath".to_string());
        self.semantic_looks
            .insert(semantic_id.clone(), "classic".to_string());
        self.path_classes.insert(
            format!("{semantic_id}.route"),
            "edge-thickness-normal edge-pattern-solid transition".to_string(),
        );
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.add_path(
            format!("{semantic_id}.route"),
            route_segments,
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.transition, self.stroke_width)),
            },
        )?;
        self.add_classic_barb_marker(index, &points)?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        let edge_label = edge.label.trim();
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: (!edge_label.is_empty())
                .then(|| state_plain_text_label(edge_label).expect("validated in preflight")),
            description: Some(format!("{} → {}", edge.start, edge.end)),
            link: None,
        });
        Ok(())
    }

    fn add_classic_barb_marker(
        &mut self,
        index: usize,
        points: &[crate::model::LayoutPoint],
    ) -> Result<()> {
        let Some(end) = points.last() else {
            return Err(invalid(format!(
                "State edge {index} has no terminal marker tangent"
            )));
        };
        let direction = terminal_marker_direction(points, index)?;
        let normal = Point::new(-direction.y, direction.x);
        let point = |x: f64, y: f64| {
            let dx = x - 19.0;
            let dy = y - 7.0;
            Point::new(
                end.x + direction.x * dx + normal.x * dy,
                end.y + direction.y * dx + normal.y * dy,
            )
        };
        self.path_classes.insert(
            format!("state.edge.{index}.marker.end"),
            "marker".to_string(),
        );
        self.add_path(
            format!("state.edge.{index}.marker.end"),
            polygon_path(&[
                point(19.0, 7.0),
                point(9.0, 13.0),
                point(14.0, 7.0),
                point(9.0, 1.0),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.marker_fill)),
                stroke: Some(stroke(self.transition, 1.0)),
            },
        )
    }

    fn emit_edge_label(&mut self, index: usize, edge: &StateDiagramRenderEdge) -> Result<()> {
        let raw = edge.label.trim();
        if raw.is_empty() {
            return Ok(());
        }
        let text = state_plain_text_label(raw).expect("validated in preflight");
        let label = self.edges_by_id[edge.id.as_str()]
            .label
            .as_ref()
            .expect("validated in preflight");
        let bounds = Rect::new(
            label.x - label.width / 2.0,
            label.y - label.height / 2.0,
            label.width,
            label.height,
        );
        let background = if self.html_labels {
            self.edge_label_background
        } else {
            with_alpha(self.edge_label_background, 0.5)
        };
        let semantic_id = format!("state.edge.{index}.label");
        self.semantic_classes
            .insert(semantic_id.clone(), "edgeLabel".to_string());
        self.semantic_looks
            .insert(semantic_id.clone(), "classic".to_string());
        self.path_classes
            .insert(format!("{semantic_id}.background"), "label".to_string());
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
                fill: Some(Paint::solid(background)),
                stroke: None,
            },
        )?;
        self.commands.push(DrawingCommand::draw_text(self.text_run(
            text.clone(),
            Point::new(label.x, label.y),
            bounds,
            self.transition_label,
        )));
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(text),
            description: Some(format!(
                "Label for transition {} → {}",
                edge.start, edge.end
            )),
            link: None,
        });
        Ok(())
    }

    fn emit_node(&mut self, index: usize, node: &StateDiagramRenderNode) -> Result<()> {
        let layout = self.nodes_by_id[node.id.as_str()];
        let semantic_id = format!("state.node.{index}");
        let semantic_class = if node.shape == "rect" {
            "node statediagram-state"
        } else {
            "node default"
        };
        self.semantic_classes
            .insert(semantic_id.clone(), semantic_class.to_string());
        self.semantic_looks
            .insert(semantic_id.clone(), "classic".to_string());
        let dom_id = node.dom_id.trim();
        self.dom_ids.insert(
            semantic_id.clone(),
            if dom_id.is_empty() {
                node.id.clone()
            } else {
                dom_id.to_string()
            },
        );
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        let title = match node.shape.as_str() {
            "stateStart" => {
                self.path_classes
                    .insert(format!("{semantic_id}.shape"), "state-start".to_string());
                self.add_path(
                    format!("{semantic_id}.shape"),
                    ellipse_path(layout.x, layout.y, 7.0, 7.0),
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: Some(Paint::solid(self.special_state)),
                        stroke: Some(stroke(self.special_state, 1.0)),
                    },
                )?;
                "Start".to_string()
            }
            "rect" => {
                self.path_classes
                    .insert(format!("{semantic_id}.shape"), "basic".to_string());
                self.add_path(
                    format!("{semantic_id}.shape"),
                    rounded_rect_path(
                        layout.x,
                        layout.y,
                        layout.width,
                        layout.height,
                        CLASSIC_NODE_RADIUS,
                    ),
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: Some(Paint::solid(self.state_fill)),
                        stroke: Some(stroke(self.state_border, self.stroke_width)),
                    },
                )?;
                let raw = state_node_label_text(node).expect("validated in preflight");
                let text = state_plain_text_label(&raw).expect("validated in preflight");
                self.text_classes
                    .insert(semantic_id.clone(), "nodeLabel".to_string());
                let rounded = node.rx.unwrap_or(0.0) > 0.0 && node.ry.unwrap_or(0.0) > 0.0;
                let horizontal_padding = if rounded {
                    self.state_padding
                } else {
                    self.state_padding * 2.0
                };
                let label_width = (layout.width - 2.0 * horizontal_padding).max(0.0);
                let label_height = (layout.height - 2.0 * self.state_padding).max(0.0);
                self.commands.push(DrawingCommand::draw_text(self.text_run(
                    text.clone(),
                    Point::new(layout.x, layout.y),
                    Rect::new(
                        layout.x - label_width / 2.0,
                        layout.y - label_height / 2.0,
                        label_width,
                        label_height,
                    ),
                    self.state_label,
                )));
                text
            }
            _ => unreachable!("unsupported shapes fail preflight"),
        };
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(title),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn text_run(&self, text: String, origin: Point, bounds: Rect, color: Color) -> TextRun {
        TextRun {
            text,
            origin,
            bounds,
            style: TextStyle {
                font: self.font.clone(),
                font_size: self.font_size,
                letter_spacing: 0.0,
                line_height: self.line_height,
                fill: Paint::solid(color),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor: TextAnchor::Middle,
            baseline: TextBaseline::Middle,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        }
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!("State path `{id}` has no geometry")));
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

fn state_node_label_text(node: &StateDiagramRenderNode) -> Result<String> {
    match node.label.as_ref() {
        Some(value) => state_value_to_label_text(value).ok_or_else(|| {
            unavailable(format!(
                "State node `{}` label is not a string or a string array",
                node.id
            ))
        }),
        None => Ok(node.id.clone()),
    }
}

fn unique_layout_nodes(layout: &StateDiagramLayout) -> Result<HashMap<&str, &LayoutNode>> {
    let mut result = HashMap::with_capacity(layout.nodes.len());
    for node in &layout.nodes {
        if result.insert(node.id.as_str(), node).is_some() {
            return Err(invalid(format!(
                "duplicate State layout node id `{}`",
                node.id
            )));
        }
    }
    Ok(result)
}

fn unique_layout_edges(layout: &StateDiagramLayout) -> Result<HashMap<&str, &LayoutEdge>> {
    let mut result = HashMap::with_capacity(layout.edges.len());
    for edge in &layout.edges {
        if result.insert(edge.id.as_str(), edge).is_some() {
            return Err(invalid(format!(
                "duplicate State layout edge id `{}`",
                edge.id
            )));
        }
    }
    Ok(result)
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("State root bounds are invalid"));
    }
    Ok(())
}

fn validate_label_bounds(label: &crate::model::LayoutLabel, edge_id: &str) -> Result<()> {
    if ![label.x, label.y, label.width, label.height]
        .into_iter()
        .all(f64::is_finite)
        || label.width < 0.0
        || label.height < 0.0
    {
        return Err(invalid(format!(
            "State edge `{edge_id}` has invalid label geometry"
        )));
    }
    Ok(())
}

fn unit_direction(x: f64, y: f64) -> Option<Point> {
    let length = x.hypot(y);
    if !length.is_finite() || length < 1e-9 {
        None
    } else {
        Some(Point::new(x / length, y / length))
    }
}

fn terminal_marker_direction(
    points: &[crate::model::LayoutPoint],
    edge_index: usize,
) -> Result<Point> {
    let end = points.last().ok_or_else(|| {
        invalid(format!(
            "State edge {edge_index} has no terminal marker point"
        ))
    })?;
    points
        .iter()
        .rev()
        .skip(1)
        .find_map(|candidate| unit_direction(end.x - candidate.x, end.y - candidate.y))
        .ok_or_else(|| {
            unavailable(format!(
                "State edge {edge_index} marker has no non-zero tangent; DrawingList v1 cannot choose a lossless marker orientation"
            ))
        })
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

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "state".to_string(),
        reason: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> crate::model::LayoutPoint {
        crate::model::LayoutPoint { x, y }
    }

    #[test]
    fn terminal_marker_direction_skips_coincident_endpoint_points() {
        let points = [point(1.0, 2.0), point(4.0, 6.0), point(4.0, 6.0)];

        assert_eq!(
            terminal_marker_direction(&points, 7).expect("terminal marker direction"),
            Point::new(0.6, 0.8)
        );
    }

    #[test]
    fn terminal_marker_direction_rejects_a_fully_degenerate_route() {
        let points = [point(1.0, 2.0), point(1.0, 2.0), point(1.0, 2.0)];

        let error = terminal_marker_direction(&points, 7)
            .expect_err("a fully degenerate State edge must fail closed");
        assert!(matches!(
            error,
            Error::DrawingListUnavailable { ref family, .. } if family == "state"
        ));
        assert!(error.to_string().contains("no non-zero tangent"));
    }
}
