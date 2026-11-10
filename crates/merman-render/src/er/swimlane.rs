//! ER's measured compound graph boundary for the registered Swimlane provider.

use super::{
    ErEntity, ErLayoutSettings, ErPreparedLabels, ErPreparedRelationshipLabel, er_layout_bounds,
    er_marker_id, er_subgraph_title_metrics, er_swimlane_title_metrics,
};
use crate::model::{
    ErDiagramLayout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, SwimlaneDirection,
    SwimlaneLaneLayout,
};
use crate::resources::OperationWorkMeter;
use crate::swimlane::{MeasuredNode, layout_compound};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::RankDir;
use merman_model::families::er::ErDiagramModel;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// The selected provider creates a fresh plain labelRect node. Its frozen terminal supplies
/// layout metrics, painted content, HTML mode and visible-style facts to every consumer.
#[derive(Debug, Clone)]
pub(crate) struct ErSwimlaneRelationshipLabel {
    metrics: TextMetrics,
    content: ErSwimlaneLabelContent,
    font_size_px: f64,
    visible_style_facts: OnceLock<super::text_facts::ErTextFacts>,
}

#[derive(Debug, Clone)]
enum ErSwimlaneLabelContent {
    Html {
        fragment: String,
        wrapper_style: String,
    },
    Svg {
        lines: Vec<Vec<String>>,
    },
}

pub(crate) fn relationship_html_source(source: &str) -> String {
    if source.is_empty() {
        String::new()
    } else {
        let label = crate::graph_label::flowchart_non_markdown_label_for_html(source, "text");
        crate::text::replace_fontawesome_icons(&format!("<p>{label}</p>"))
    }
}

impl ErSwimlaneRelationshipLabel {
    pub(super) fn retained_buffer_bytes(&self) -> usize {
        match &self.content {
            ErSwimlaneLabelContent::Html {
                fragment,
                wrapper_style,
            } => fragment.capacity().saturating_add(wrapper_style.capacity()),
            ErSwimlaneLabelContent::Svg { lines } => lines.iter().fold(
                lines
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Vec<String>>()),
                |bytes, row| {
                    row.iter().fold(
                        bytes.saturating_add(
                            row.capacity().saturating_mul(std::mem::size_of::<String>()),
                        ),
                        |bytes, word| bytes.saturating_add(word.capacity()),
                    )
                },
            ),
        }
    }

    pub(super) fn prepare(
        source: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        config: &merman_model::MermaidConfig,
    ) -> Self {
        let view = super::ErConfigView::new(config.as_value());
        let wrapping_width = view.swimlane_relationship_wrapping_width();
        let (metrics, content) = if view.swimlane_relationship_html_labels() {
            // nonMarkdownToHTML wraps literal text in one paragraph. The sanitizer retains
            // authored HTML semantics, while Markdown punctuation remains ordinary text.
            let html = relationship_html_source(source);
            let fragment = merman_markup::sanitize::sanitize_xhtml(&html, config);
            let metrics =
                if crate::graph_label::flowchart_label_text_is_empty_for_mode(source, true) {
                    TextMetrics {
                        width: 0.0,
                        height: 0.0,
                        line_count: 0,
                    }
                } else {
                    crate::text::measure_xhtml_label_fragment_with_terminal_fonts(
                        measurer,
                        &fragment,
                        style,
                        Some(wrapping_width),
                        super::HTML_DOCUMENT_ROOT_FONT_SIZE_PX,
                    )
                };
            // addHtmlSpan switches a constrained table-cell to a fixed-width break-spaces
            // table. Retain this choice with the measured terminal instead of deciding in SVG.
            let wrapped = wrapping_width.is_finite()
                && wrapping_width > 0.0
                && metrics.width >= wrapping_width;
            let wrapper_style = if wrapped {
                format!(
                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {wrapping_width}px; width: {wrapping_width}px; text-align: center;"
                )
            } else {
                format!(
                    "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {wrapping_width}px; text-align: center;"
                )
            };
            (
                metrics,
                ErSwimlaneLabelContent::Html {
                    fragment,
                    wrapper_style,
                },
            )
        } else {
            // This source/measurement utility is shared text machinery, independent of the
            // Flowchart family feature. Freeze its selected word rows for the SVG writer.
            let source = crate::graph_label::FlowchartSvgLabelSource::new(source);
            let lines = source.wrapped_lines(measurer, style, Some(wrapping_width), true);
            let metrics = source.metrics_from_wrapped(
                measurer,
                style,
                &lines,
                crate::graph_label::FlowchartSvgWidthMode::Bbox,
            );
            (metrics, ErSwimlaneLabelContent::Svg { lines })
        };
        Self {
            metrics,
            content,
            font_size_px: style.font_size,
            visible_style_facts: OnceLock::new(),
        }
    }

    pub(super) fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub(crate) fn xhtml_fragment(&self) -> Option<&str> {
        self.html().map(|(fragment, _)| fragment)
    }

    pub(crate) fn html(&self) -> Option<(&str, &str)> {
        match &self.content {
            ErSwimlaneLabelContent::Html {
                fragment,
                wrapper_style,
            } => Some((fragment, wrapper_style)),
            ErSwimlaneLabelContent::Svg { .. } => None,
        }
    }

    pub(crate) fn svg_lines(&self) -> Option<&[Vec<String>]> {
        match &self.content {
            ErSwimlaneLabelContent::Svg { lines } => Some(lines),
            ErSwimlaneLabelContent::Html { .. } => None,
        }
    }

    pub(super) fn text_facts(&self) -> &super::text_facts::ErTextFacts {
        self.visible_style_facts
            .get_or_init(|| match &self.content {
                ErSwimlaneLabelContent::Html { fragment, .. } => {
                    super::text_facts::ErTextFacts::html(fragment, Some(self.font_size_px))
                }
                ErSwimlaneLabelContent::Svg { lines } => {
                    let plain =
                        crate::graph_label::flowchart_svg_source_word_lines_plain_text(lines);
                    super::text_facts::ErTextFacts::plain(&plain, Some(self.font_size_px))
                }
            })
    }
}

/// Renderer-owned provider data. Label anchors remain on the ER relationships; lane metadata
/// carries the content boundary and parent roles that ordinary LayoutCluster does not expose.
#[derive(Debug, Clone)]
pub(crate) struct ErSwimlanePaintLayout {
    pub direction: SwimlaneDirection,
    pub lanes: Vec<SwimlaneLaneLayout>,
}

pub(crate) struct ErSwimlaneLaneGeometry {
    pub width: f64,
    pub height: f64,
    pub title_band_width: f64,
    pub title_band_height: f64,
    pub title_label: LayoutLabel,
}

impl ErSwimlanePaintLayout {
    /// Geometry from rendering-elements/clusters/swimlane.js, shared by bounds and painting.
    pub(crate) fn lane_geometry(&self, lane: &SwimlaneLaneLayout) -> ErSwimlaneLaneGeometry {
        let label_width = lane.title_label_width.max(0.0);
        let label_height = lane.title_label_height.max(0.0);
        let is_lr = self.direction == SwimlaneDirection::Lr;
        let title_size = label_height + if is_lr { 8.0 } else { 0.0 };
        let width = lane.width.max(label_width + lane.padding.max(0.0));
        let height = lane.height.max(0.0);
        let top = lane.y - height / 2.0;
        if is_lr {
            ErSwimlaneLaneGeometry {
                width,
                height,
                title_band_width: title_size,
                title_band_height: height,
                title_label: LayoutLabel {
                    x: lane.x - width / 2.0 + title_size / 2.0,
                    y: lane.y,
                    width: label_height,
                    height: label_width,
                },
            }
        } else {
            let content_top = lane.content_top.unwrap_or(top + height / 3.0);
            let title_height = title_size.min((content_top - top).max(0.0));
            ErSwimlaneLaneGeometry {
                width,
                height,
                title_band_width: width,
                title_band_height: title_height,
                title_label: LayoutLabel {
                    x: lane.x,
                    y: top + title_height / 2.0,
                    width: label_width,
                    height: label_height,
                },
            }
        }
    }
}

pub(super) fn layout_er_diagram_swimlane_typed(
    model: &ErDiagramModel,
    config: &Value,
    measurer: &dyn TextMeasurer,
    settings: &ErLayoutSettings,
    prepared_labels: Arc<ErPreparedLabels>,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<ErDiagramLayout> {
    work_meter.preflight(0)?;
    if let Some((index, _)) = model
        .relationships
        .iter()
        .enumerate()
        .find(|(_, relationship)| relationship.entity_a == relationship.entity_b)
    {
        return Err(Error::InvalidModel {
            message: format!(
                "ER relationship {index} is a self-loop; the selected Swimlane provider does not support self-loops"
            ),
        });
    }
    let direction = match settings.graph.rankdir {
        RankDir::TB => SwimlaneDirection::Tb,
        RankDir::BT => SwimlaneDirection::Bt,
        RankDir::LR => SwimlaneDirection::Lr,
        RankDir::RL => SwimlaneDirection::Rl,
    };
    let subgraph_by_id: HashMap<_, _> = model
        .subgraphs
        .iter()
        .map(|group| (group.id.as_str(), group))
        .collect();
    let entity_id_by_name: HashMap<_, _> = model
        .entities
        .iter()
        .map(|(name, entity)| (name.as_str(), entity.id.as_str()))
        .collect();
    let mut parent_by_id = HashMap::new();
    // ErDB.getData builds parentDB in reverse declaration order and translates entity source
    // names to renderer ids only when it projects entities. No Flowchart model is involved.
    for group in model.subgraphs.iter().rev() {
        for member in &group.nodes {
            let id = if subgraph_by_id.contains_key(member.as_str()) {
                member.as_str()
            } else if let Some(id) = entity_id_by_name.get(member.as_str()) {
                *id
            } else {
                continue;
            };
            parent_by_id.insert(id, group.id.as_str());
        }
    }

    let view = super::ErConfigView::new(config);
    let mut measured_nodes = Vec::with_capacity(model.entities.len() + model.subgraphs.len());
    for group in model.subgraphs.iter().rev() {
        // Only top-level groups use the Swimlane painter. Nested groups keep the ordinary
        // cluster label mode and label type, just as render_er_subgraph_clusters dispatches.
        let is_lane = !parent_by_id.contains_key(group.id.as_str());
        let html_labels = view.subgraph_html_labels(is_lane);
        let metrics = if is_lane {
            er_swimlane_title_metrics(
                &group.title,
                measurer,
                &settings.label_style,
                if html_labels {
                    WrapMode::HtmlLike
                } else {
                    WrapMode::SvgLike
                },
            )
        } else {
            er_subgraph_title_metrics(group, measurer, &settings.label_style, html_labels)
        };
        let has_children = group.nodes.iter().any(|member| {
            subgraph_by_id.contains_key(member.as_str())
                || entity_id_by_name.contains_key(member.as_str())
        });
        measured_nodes.push(LayoutNode {
            id: group.id.clone(),
            x: 0.0,
            y: 0.0,
            width: if has_children {
                0.0
            } else {
                metrics.width + 16.0
            },
            height: if has_children {
                0.0
            } else {
                metrics.height + 16.0
            },
            is_cluster: true,
            label_width: Some(metrics.width.max(0.0)),
            label_height: Some(metrics.height.max(0.0)),
        });
    }
    let mut entities = model
        .entities
        .iter()
        .filter(|(name, _)| !subgraph_by_id.contains_key(name.as_str()))
        .map(|(_, entity)| entity)
        .collect::<Vec<_>>();
    fn insertion_index(entity: &ErEntity) -> Option<usize> {
        entity.id.rsplit_once('-')?.1.parse().ok()
    }
    entities.sort_by(|left, right| {
        (insertion_index(left), left.id.as_str()).cmp(&(insertion_index(right), right.id.as_str()))
    });
    for entity in entities {
        let measure = prepared_labels
            .entity(&entity.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ER prepared labels are missing entity {}", entity.id),
            })?;
        measured_nodes.push(LayoutNode {
            id: entity.id.clone(),
            x: 0.0,
            y: 0.0,
            width: measure.width,
            height: measure.height,
            is_cluster: false,
            label_width: Some(measure.label_html_width),
            label_height: Some(measure.label_height),
        });
    }
    let measured = measured_nodes
        .iter()
        .map(|node| {
            let group = subgraph_by_id.get(node.id.as_str()).copied();
            MeasuredNode {
                node,
                shape: if node.is_cluster { "rect" } else { "erBox" },
                label: group.map_or("", |group| group.title.as_str()),
                label_type: group.map_or("markdown", |group| group.label_type.as_str()),
                parent_id: parent_by_id.get(node.id.as_str()).copied(),
                requested_dir: group.and_then(|group| group.dir.as_deref()),
                padding: if node.is_cluster { 8.0 } else { 0.0 },
            }
        })
        .collect::<Vec<_>>();
    let mut edges = model
        .relationships
        .iter()
        .enumerate()
        .map(|(index, relationship)| {
            let label = if relationship.role_a.is_empty() {
                None
            } else {
                let (width, height) = prepared_labels
                    .relationship(index)
                    .map(ErPreparedRelationshipLabel::dimensions)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("ER prepared labels are missing relationship {index}"),
                    })?;
                Some(LayoutLabel {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                })
            };
            Ok(LayoutEdge {
                id: format!("er-rel-{index}"),
                from: relationship.entity_a.clone(),
                to: relationship.entity_b.clone(),
                from_cluster: subgraph_by_id
                    .contains_key(relationship.entity_a.as_str())
                    .then(|| relationship.entity_a.clone()),
                to_cluster: subgraph_by_id
                    .contains_key(relationship.entity_b.as_str())
                    .then(|| relationship.entity_b.clone()),
                points: Vec::new(),
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: er_marker_id(relationship.rel_spec.card_b.as_str(), "START"),
                end_marker: er_marker_id(relationship.rel_spec.card_a.as_str(), "END"),
                stroke_dasharray: (relationship.rel_spec.rel_type == "NON_IDENTIFYING")
                    .then(|| "8,8".to_string()),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    // erRenderer-unified writes `conf.nodeSpacing || 140` / `conf.rankSpacing || 80` into
    // Flowchart before Swimlane reads them. Root Dagre spacing overrides do not participate.
    let spacing = (
        renderer_spacing(config, "nodeSpacing", 140.0),
        renderer_spacing(config, "rankSpacing", 80.0),
    );
    let placed = layout_compound(&measured, &edges, direction, config, spacing, work_meter)?;
    if placed.edges.len() != edges.len() {
        return Err(Error::InvalidModel {
            message: "Swimlane changed the ER relationship count".to_string(),
        });
    }
    let labels: HashMap<_, _> = placed
        .nodes
        .iter()
        .filter(|node| node.is_edge_label)
        .map(|node| (node.id.as_str(), node))
        .collect();
    for (edge, route) in edges.iter_mut().zip(&placed.edges) {
        if edge.id != route.id || edge.from != route.from || edge.to != route.to {
            return Err(Error::InvalidModel {
                message: format!("Swimlane changed ER relationship ownership for {}", edge.id),
            });
        }
        edge.points.clone_from(&route.points);
        if let Some(label) = &mut edge.label {
            let placed_label = route
                .label_node_id
                .as_deref()
                .and_then(|id| labels.get(id))
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("Swimlane is missing the measured ER label for {}", edge.id),
                })?;
            label.x = placed_label.x;
            label.y = placed_label.y;
        }
    }

    let mut nodes = placed
        .nodes
        .iter()
        .filter(|node| !node.is_edge_label)
        .map(|node| LayoutNode {
            id: node.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            is_cluster: false,
            label_width: Some(node.label_width),
            label_height: Some(node.label_height),
        })
        .collect::<Vec<_>>();
    let paint = ErSwimlanePaintLayout {
        direction,
        lanes: placed.lanes,
    };
    let mut clusters = Vec::with_capacity(paint.lanes.len());
    for lane in &paint.lanes {
        let geometry = lane.parent_id.is_none().then(|| paint.lane_geometry(lane));
        let width = geometry
            .as_ref()
            .map_or(lane.width, |geometry| geometry.width);
        let height = geometry
            .as_ref()
            .map_or(lane.height, |geometry| geometry.height);
        nodes.push(LayoutNode {
            id: lane.id.clone(),
            x: lane.x,
            y: lane.y,
            width,
            height,
            is_cluster: true,
            label_width: Some(lane.title_label_width),
            label_height: Some(lane.title_label_height),
        });
        let title_label = geometry.map_or_else(
            || LayoutLabel {
                x: lane.x,
                y: lane.y - height / 2.0 + lane.padding + lane.title_label_height / 2.0,
                width: lane.title_label_width,
                height: lane.title_label_height,
            },
            |geometry| geometry.title_label,
        );
        let group = subgraph_by_id.get(lane.id.as_str()).copied();
        clusters.push(LayoutCluster {
            id: lane.id.clone(),
            x: lane.x,
            y: lane.y,
            width,
            height,
            diff: (lane.title_label_width - width) / 2.0 - lane.padding / 2.0,
            offset_y: lane.title_label_height - lane.padding / 2.0,
            title: lane.title.clone(),
            title_label,
            requested_dir: group.and_then(|group| group.dir.clone()),
            effective_dir: lane
                .requested_dir
                .clone()
                .unwrap_or_else(|| direction.as_str().to_string()),
            padding: lane.padding,
            title_margin_top: 0.0,
            title_margin_bottom: 0.0,
        });
    }
    // Label waypoints are an internal Swimlane detail; ER continues to expose one relationship
    // with the original endpoints, prepared label, cardinalities, and identifying style.
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let bounds = er_layout_bounds(&nodes, &edges);
    Ok(ErDiagramLayout {
        nodes,
        render_edges: edges.clone(),
        swimlane: Some(paint),
        edges,
        clusters,
        bounds,
        prepared_labels,
    })
}

fn renderer_spacing(config: &Value, key: &str, fallback: f64) -> f64 {
    config
        .get("er")
        .and_then(|er| er.get(key))
        .filter(|value| crate::config::json_value_is_truthy(value))
        .and_then(crate::config::json_f64)
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use crate::text::DeterministicTextMeasurer;
    use merman_model::families::er::{ErEntity, ErRelationship};
    use serde_json::json;

    #[test]
    fn swimlane_plain_terminal_owns_literal_text_metrics_and_style_facts() {
        let measurer = DeterministicTextMeasurer::default();
        let model = ErDiagramModel {
            direction: "TB".into(),
            relationships: vec![ErRelationship {
                entity_a: "A".into(),
                entity_b: "B".into(),
                role_a: "**owns**".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        for html in [true, false] {
            let config = json!({"layout": "swimlane", "htmlLabels": html,
                "flowchart": {"wrappingWidth": 80}});
            let labels = super::super::prepare_er_labels(&model, &config, &measurer);
            let relationship = labels.relationship(0).unwrap();
            let terminal = relationship
                .swimlane()
                .expect("provider-owned plain terminal");
            assert_eq!(
                relationship.dimensions(),
                (terminal.metrics.width, terminal.metrics.height)
            );
            assert_eq!(
                relationship.visible_style_facts(),
                &crate::text::VisibleTextStyleFacts::plain_text("**owns**"),
                "Markdown punctuation cannot acquire emphasis or style ownership"
            );
            if html {
                let fragment = terminal.xhtml_fragment().unwrap();
                assert!(fragment.contains("**owns**") && !fragment.contains("<strong>"));
            } else {
                assert_eq!(
                    terminal.svg_lines().unwrap(),
                    &[vec!["**owns**".to_string()]]
                );
            }
        }
    }

    #[test]
    fn er_swimlane_keeps_the_operations_previously_consumed_work() {
        let config = json!({"layout": "swimlane"});
        let model = ErDiagramModel {
            direction: "TB".into(),
            entities: ["A", "B"]
                .into_iter()
                .map(|id| {
                    (
                        id.to_string(),
                        ErEntity {
                            id: id.to_string(),
                            label: id.to_string(),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            relationships: vec![ErRelationship {
                entity_a: "A".into(),
                entity_b: "B".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let measurer = DeterministicTextMeasurer::default();
        let settings = super::super::ErConfigView::new(&config)
            .layout_settings_with_font_family(&model.direction, None);
        let labels = super::super::prepare_er_labels(&model, &config, &measurer);
        let policy = RenderResourcePolicy::interactive()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 100_000)
            .unwrap();
        layout_er_diagram_swimlane_typed(
            &model,
            &config,
            &measurer,
            &settings,
            Arc::clone(&labels),
            Arc::new(OperationWorkMeter::new(policy)),
        )
        .expect("the small graph fits an unused operation budget");
        let meter = Arc::new(OperationWorkMeter::new(policy));
        meter.charge(99_999).unwrap();
        let error =
            layout_er_diagram_swimlane_typed(&model, &config, &measurer, &settings, labels, meter)
                .unwrap_err();
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected cumulative Swimlane resource rejection, got {error:?}");
        };
        assert_eq!(limit.limit, "max_layout_work_units");
        assert!(limit.actual > limit.max);
    }
}
