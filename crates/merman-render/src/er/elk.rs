//! ELK graph construction and projection of solved ER geometry.

use super::{
    ErEntity, ErLayoutSettings, ErPreparedLabels, ErPreparedRelationshipLabel, ErSubgraph,
    calc_label_position, er_layout_bounds, er_marker_id, er_subgraph_title_metrics,
    parse_er_rel_idx_from_edge_name,
};
use crate::elk_options::layout_options as er_elk_layout_options;
use crate::graph_layout::ir::Direction;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{ErDiagramLayout, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use merman_layout_elk as elk;
use merman_model::families::er::ErDiagramModel;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub(super) fn layout(
    model: &ErDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    settings: ErLayoutSettings,
    prepared_labels: Arc<ErPreparedLabels>,
    operation_seed: elk::ElkOperationSeed,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<ErDiagramLayout> {
    let mut elk_graph = er_elk_graph(
        model,
        effective_config,
        measurer,
        &settings,
        &prepared_labels,
        &mut Some(&mut *work_control),
    )?;
    let subgraph_by_id: HashMap<&str, &ErSubgraph> = model
        .subgraphs
        .iter()
        .map(|subgraph| (subgraph.id.as_str(), subgraph))
        .collect();
    let subgraph_ids: HashSet<&str> = subgraph_by_id.keys().copied().collect();
    let mut subgraph_title_metrics = HashMap::with_capacity(subgraph_by_id.len());
    for subgraph in model.subgraphs.iter() {
        let metrics = er_subgraph_title_metrics(
            subgraph,
            measurer,
            &settings.label_style,
            settings.relationship_html_labels,
        );
        subgraph_title_metrics.insert(
            subgraph.id.as_str(),
            (metrics.width.max(0.0), metrics.height.max(0.0)),
        );
    }
    let oriented = crate::elk_feedback_edges::orient_feedback_edges(
        &mut elk_graph,
        effective_config,
        &mut Some(&mut *work_control),
    )?;
    let mut elk_layout = elk::layout_with_operation_seed_and_work_control(
        oriented.graph(),
        operation_seed,
        work_control,
    )
    .map_err(|error| work_control.map_elk_error_with_context(error, "ER ELK"))?;
    oriented.restore(&mut elk_layout, &mut Some(&mut *work_control))?;
    let source_edge_by_id = elk_graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<HashMap<_, _>>();

    let frames = crate::elk_adapter::drawing_group_frames(
        &elk_graph,
        &elk_layout,
        |node| node.label.map_or(0.0, |label| label.width) + node.container.padding,
        &mut Some(&mut *work_control),
    )?;
    let paint_order =
        crate::elk_adapter::parent_first_order(&elk_graph.nodes, &mut Some(&mut *work_control))?;
    let paint_index_by_id: HashMap<&str, usize> = paint_order
        .iter()
        .enumerate()
        .map(|(index, &node_index)| (elk_graph.nodes[node_index].id.as_str(), index))
        .collect();
    let mut out_nodes = elk_layout
        .nodes
        .into_iter()
        .map(|node| {
            let is_cluster = subgraph_ids.contains(node.id.as_str());
            let frame = frames.get(node.id.as_str());
            LayoutNode {
                id: node.id,
                x: frame.map_or(node.x, |frame| frame.x),
                y: frame.map_or(node.y, |frame| frame.y),
                width: frame.map_or(node.width, |frame| frame.width),
                height: frame.map_or(node.height, |frame| frame.height),
                is_cluster,
                label_width: None,
                label_height: None,
            }
        })
        .collect::<Vec<_>>();

    let mut clusters = Vec::with_capacity(subgraph_by_id.len());
    for node in out_nodes.iter().filter(|node| node.is_cluster) {
        let Some(subgraph) = subgraph_by_id.get(node.id.as_str()).copied() else {
            continue;
        };
        let (title_width, title_height) = subgraph_title_metrics
            .get(node.id.as_str())
            .copied()
            .unwrap_or((0.0, 0.0));
        let padding = 8.0;
        let title_label = LayoutLabel {
            x: node.x,
            y: node.y - node.height / 2.0 + padding + title_height / 2.0,
            width: title_width,
            height: title_height,
        };
        clusters.push(crate::model::LayoutCluster {
            id: node.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            diff: (title_width - node.width) / 2.0 - padding / 2.0,
            offset_y: title_height - padding / 2.0,
            title: subgraph.title.clone(),
            title_label,
            requested_dir: subgraph.dir.clone(),
            effective_dir: subgraph
                .dir
                .clone()
                .unwrap_or_else(|| model.direction.clone()),
            padding,
            title_margin_top: 0.0,
            title_margin_bottom: 0.0,
        });
    }

    let mut out_edges = Vec::with_capacity(elk_layout.edges.len());
    for edge in elk_layout.edges {
        let Some(source) = source_edge_by_id.get(edge.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown ER edge {}", edge.id),
            });
        };
        let Some(index) = parse_er_rel_idx_from_edge_name(&edge.id) else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned malformed ER edge id {}", edge.id),
            });
        };
        let Some(relationship) = model.relationships.get(index) else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned out-of-range ER edge id {}", edge.id),
            });
        };
        let points = edge
            .points
            .into_iter()
            .map(|point| LayoutPoint {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>();
        let label = source.label.and_then(|source_label| {
            edge.labels
                .first()
                .map(|label| LayoutLabel {
                    x: label.x + label.width / 2.0,
                    y: label.y + label.height / 2.0,
                    width: label.width,
                    height: label.height,
                })
                .or_else(|| {
                    calc_label_position(&points).map(|(x, y)| LayoutLabel {
                        x,
                        y,
                        width: source_label.width,
                        height: source_label.height,
                    })
                })
        });
        let rel_type = relationship.rel_spec.rel_type.as_str();
        out_edges.push(LayoutEdge {
            id: edge.id,
            from: source.source.clone(),
            to: source.target.clone(),
            from_cluster: None,
            to_cluster: None,
            points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: er_marker_id(relationship.rel_spec.card_b.as_str(), "START"),
            end_marker: er_marker_id(relationship.rel_spec.card_a.as_str(), "END"),
            stroke_dasharray: (rel_type == "NON_IDENTIFYING").then(|| "8,8".to_string()),
        });
    }

    out_nodes.sort_by(|left, right| left.id.cmp(&right.id));
    out_edges.sort_by(|left, right| left.id.cmp(&right.id));
    // Mermaid paints groups by stable ancestor depth, then paints leaf nodes above them.
    clusters.sort_by_key(|cluster| paint_index_by_id.get(cluster.id.as_str()).copied());
    let bounds = er_layout_bounds(&out_nodes, &out_edges);
    Ok(ErDiagramLayout {
        nodes: out_nodes,
        // ELK's canonical edges already form the SVG-facing projection.
        render_edges: Vec::new(),
        swimlane: None,
        edges: out_edges,
        clusters,
        bounds,
        prepared_labels,
    })
}

fn er_elk_graph(
    model: &ErDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    settings: &ErLayoutSettings,
    prepared_labels: &ErPreparedLabels,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<elk::Graph> {
    let subgraph_ids: HashSet<&str> = model
        .subgraphs
        .iter()
        .map(|subgraph| subgraph.id.as_str())
        .collect();
    let entity_id_by_name: HashMap<&str, &str> = model
        .entities
        .iter()
        .map(|(name, entity)| (name.as_str(), entity.id.as_str()))
        .collect();
    let mut parent_by_member: HashMap<&str, &str> = HashMap::new();
    for subgraph in &model.subgraphs {
        for member in &subgraph.nodes {
            let member_id = if subgraph_ids.contains(member.as_str()) {
                member.as_str()
            } else if let Some(entity_id) = entity_id_by_name.get(member.as_str()) {
                entity_id
            } else {
                continue;
            };
            parent_by_member.insert(member_id, subgraph.id.as_str());
        }
    }

    let mut nodes = Vec::with_capacity(model.subgraphs.len() + model.entities.len());
    for subgraph in model.subgraphs.iter().rev() {
        let metrics = er_subgraph_title_metrics(
            subgraph,
            measurer,
            &settings.label_style,
            settings.relationship_html_labels,
        );
        let has_children = subgraph.nodes.iter().any(|member| {
            subgraph_ids.contains(member.as_str())
                || entity_id_by_name.contains_key(member.as_str())
        });
        nodes.push(elk::Node {
            id: subgraph.id.clone(),
            kind: elk::NodeKind::Group,
            container: elk::ContainerNodeOptions {
                padding: 8.0,
                ..Default::default()
            },
            label_text: Some(subgraph.title.clone()),
            width: 0.0,
            height: 0.0,
            parent: parent_by_member
                .get(subgraph.id.as_str())
                .map(|parent| (*parent).to_string()),
            direction: subgraph.dir.as_deref().and_then(er_elk_direction),
            hierarchy_handling: Some(elk::HierarchyHandling::IncludeChildren),
            layer_constraint: None,
            port_alignment: None,
            label: has_children.then_some(elk::Label {
                width: metrics.width.max(0.0),
                height: metrics.height.max(0.0),
            }),
        });
    }

    let mut entities: Vec<&ErEntity> = model
        .entities
        .iter()
        .filter_map(|(name, entity)| (!subgraph_ids.contains(name.as_str())).then_some(entity))
        .collect();
    entities.sort_by(|left, right| {
        fn counter(id: &str) -> Option<usize> {
            id.rsplit_once('-')?.1.parse().ok()
        }
        (counter(&left.id), left.id.as_str()).cmp(&(counter(&right.id), right.id.as_str()))
    });

    let entity_nodes = entities
        .into_iter()
        .map(|entity| {
            let measure =
                prepared_labels
                    .entity(&entity.id)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("ER prepared labels are missing entity {}", entity.id),
                    })?;
            Ok(elk::Node {
                id: entity.id.clone(),
                kind: elk::NodeKind::Leaf,
                container: Default::default(),
                label_text: None,
                port_alignment: None,
                width: measure.width,
                height: measure.height,
                parent: parent_by_member
                    .get(entity.id.as_str())
                    .map(|parent| (*parent).to_string()),
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                label: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    nodes.extend(entity_nodes);

    if crate::config::config_bool(effective_config, &["elk", "keepEntryNodeOnTop"]).unwrap_or(false)
    {
        crate::elk_adapter::apply_cyclic_entry_constraints(
            &mut nodes,
            model.relationships.iter().map(|relationship| {
                (
                    relationship.entity_a.as_str(),
                    relationship.entity_b.as_str(),
                )
            }),
            work_control,
        )?;
    }

    let node_ids = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut edges = Vec::with_capacity(model.relationships.len());
    for (index, relationship) in model.relationships.iter().enumerate() {
        if !node_ids.contains(relationship.entity_a.as_str())
            || !node_ids.contains(relationship.entity_b.as_str())
        {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing ER nodes: {} -> {}",
                    relationship.entity_a, relationship.entity_b
                ),
            });
        }
        let (label_width, label_height) = prepared_labels
            .relationship(index)
            .map(ErPreparedRelationshipLabel::dimensions)
            .unwrap_or_default();
        edges.push(elk::Edge {
            id: format!("er-rel-{index}"),
            source: relationship.entity_a.clone(),
            target: relationship.entity_b.clone(),
            label: (!relationship.role_a.is_empty()).then_some(elk::Label {
                width: label_width,
                height: label_height,
            }),
            minlen: 1,
            terminal_labels: Vec::new(),
            inside_self_loops_yo: false,
        });
    }

    Ok(elk::Graph {
        id: "root".to_string(),
        direction: match settings.direction {
            Direction::Down => elk::Direction::Down,
            Direction::Up => elk::Direction::Up,
            Direction::Right => elk::Direction::Right,
            Direction::Left => elk::Direction::Left,
        },
        nodes,
        edges,
        // Mermaid's ELK adapter sets `spacing.baseValue = 40`; the source-backed adapter owns
        // that exact option projection, so family-specific Dagre spacing does not leak into ELK.
        spacing: elk::Spacing::default(),
        options: er_elk_layout_options(effective_config),
    })
}

fn er_elk_direction(direction: &str) -> Option<elk::Direction> {
    match direction.trim().to_ascii_uppercase().as_str() {
        "LR" => Some(elk::Direction::Right),
        "RL" => Some(elk::Direction::Left),
        "BT" => Some(elk::Direction::Up),
        "TB" => Some(elk::Direction::Down),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::er::{ErConfigView, ErEntity, ErRelationship, ErSubgraph, prepare_er_labels};
    use crate::text::DeterministicTextMeasurer;

    #[test]
    fn er_elk_keeps_the_source_default_nonzero_seed() {
        assert_eq!(
            super::er_elk_layout_options(&serde_json::Value::Null)
                .layered
                .random_seed,
            1
        );
    }

    #[test]
    fn er_elk_zero_seed_adapter_graph_requires_an_operation_seed() {
        use std::num::NonZeroU64;

        let mut model = merman_model::families::er::ErDiagramModel {
            direction: "TB".to_string(),
            ..Default::default()
        };
        for id in ["CUSTOMER", "ORDER"] {
            model.entities.insert(
                id.to_string(),
                merman_model::families::er::ErEntity {
                    id: id.to_string(),
                    label: id.to_string(),
                    ..Default::default()
                },
            );
        }
        model
            .relationships
            .push(merman_model::families::er::ErRelationship {
                entity_a: "CUSTOMER".to_string(),
                role_a: "places".to_string(),
                entity_b: "ORDER".to_string(),
                ..Default::default()
            });

        let effective_config = serde_json::json!({ "layout": "elk" });
        let settings = ErConfigView::new(&effective_config)
            .layout_settings_with_font_family(&model.direction, None);
        let measurer = DeterministicTextMeasurer::default();
        let prepared_labels = prepare_er_labels(&model, &effective_config, &measurer);
        let mut graph = super::er_elk_graph(
            &model,
            &effective_config,
            &measurer,
            &settings,
            &prepared_labels,
            &mut None,
        )
        .expect("ER ELK adapter graph");
        graph.options.layered.random_seed = 0;

        assert!(super::elk::layout(&graph).is_err());

        let operation_seed = super::elk::ElkOperationSeed::from_operation_seed(
            NonZeroU64::new(0x6572_2d73_6565_6421).expect("nonzero operation seed"),
        );
        let first = super::elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("seeded ER layout");
        let replayed = super::elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("replayed seeded ER layout");

        assert_eq!(first, replayed);
    }

    #[test]
    fn er_elk_cyclic_entry_follows_edge_order_in_root_and_nested_scopes() {
        for declared in [["B", "A", "C"], ["C", "B", "A"]] {
            for nested in [false, true] {
                let mut model = merman_model::families::er::ErDiagramModel {
                    direction: "TB".to_string(),
                    ..Default::default()
                };
                for (index, name) in declared.iter().enumerate() {
                    model.entities.insert(
                        (*name).to_string(),
                        ErEntity {
                            id: format!("entity-{name}-{index}"),
                            label: (*name).to_string(),
                            ..Default::default()
                        },
                    );
                }
                for (source, target) in [("A", "B"), ("B", "C"), ("C", "A")] {
                    model.relationships.push(ErRelationship {
                        entity_a: model.entities[source].id.clone(),
                        entity_b: model.entities[target].id.clone(),
                        ..Default::default()
                    });
                }
                if nested {
                    model.subgraphs.push(ErSubgraph {
                        id: "Scope".to_string(),
                        title: "Scope".to_string(),
                        nodes: declared.iter().map(|name| (*name).to_string()).collect(),
                        ..Default::default()
                    });
                }
                for enabled in [false, true] {
                    let config = serde_json::json!({ "layout": "elk", "elk": { "keepEntryNodeOnTop": enabled } });
                    let settings = ErConfigView::new(&config)
                        .layout_settings_with_font_family(&model.direction, None);
                    let graph = super::er_elk_graph(
                        &model,
                        &config,
                        &DeterministicTextMeasurer::default(),
                        &settings,
                        &prepare_er_labels(&model, &config, &DeterministicTextMeasurer::default()),
                        &mut None,
                    )
                    .expect("ER adapter graph");
                    let constrained: Vec<_> = graph
                        .nodes
                        .iter()
                        .filter(|node| {
                            node.layer_constraint == Some(super::elk::LayerConstraint::First)
                        })
                        .map(|node| node.id.as_str())
                        .collect();
                    assert_eq!(
                        constrained,
                        if enabled {
                            vec![model.entities["A"].id.as_str()]
                        } else {
                            vec![]
                        }
                    );
                }
            }
        }
    }
}
