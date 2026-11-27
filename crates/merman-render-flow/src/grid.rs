//! FlowDB's visible, measured graph at the Mermaid Grid boundary.

use std::collections::HashMap;

use merman_model::families::flowchart::FlowSubgraph;
use merman_model::grid::{GridPlacementOwner, GridPlacementSpec};
use merman_model::presentation::PresentationConfig;
use merman_render_core::graph_layout::Size;
use merman_render_core::graph_layout::mermaid_grid::{
    GridEdgeLabel, GridGroupMeasure, GridItem, GridItemKind, GridMeasuredEdge, GridMeasuredItem,
    prepare_grid,
};
use merman_render_core::{Error, Result, resources::OperationWorkMeter, text::TextMetrics};

use crate::class_style::FlowchartClassDefinitions;
use crate::edge_svg::arrow_types_for_edge;
use crate::measured::{FlowchartMeasurement, MeasuredLabel};
use crate::node_geometry::{
    flowchart_node_render_dimensions, validate_flowchart_node_asset_dimensions,
};
use crate::projection::{FlowchartEdgeKey, FlowchartEdgeTransportPlan, FlowchartRenderModelRef};
use crate::style::FlowchartEdgeStylePlan;
use crate::svg_label::{FlowchartSvgLabelOwner, FlowchartSvgLabelSidecarBuilder};

mod layout;
pub use layout::FlowGridLayout;

struct FlowGridInput<'a> {
    items: Vec<GridMeasuredItem<'a>>,
    edges: Vec<GridMeasuredEdge<'a>>,
    edge_keys: Vec<FlowchartEdgeKey>,
    item_paint: Vec<GridItemPaint<'a>>,
}

struct GridItemPaint<'a> {
    label: MeasuredLabel,
    group: Option<&'a FlowSubgraph>,
    direction: &'a str,
}

/// Lays out the visible Flow graph with the operation's admitted label and work services.
#[allow(
    clippy::too_many_arguments,
    reason = "The layout adapter borrows the prepared configuration, labels and operation services."
)]
pub fn layout(
    view: FlowchartRenderModelRef<'_>,
    class_defs: &FlowchartClassDefinitions<'_>,
    config: &PresentationConfig,
    measurer: &dyn merman_render_core::text::TextMeasurer,
    math_renderer: Option<&(dyn merman_render_core::math::MathRenderer + Send + Sync)>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style: &FlowchartEdgeStylePlan,
    work: &OperationWorkMeter,
) -> Result<FlowGridLayout> {
    work.charge(view.edges.len())?;
    for (_, edge) in view.projected_edges() {
        work.charge(edge.semantic.id.len().max(1))?;
    }
    let edge_ids = FlowchartEdgeTransportPlan::for_keyed_edges(
        view.projected_edges()
            .map(|(key, edge)| (key, edge.semantic)),
    );
    let measurement = FlowchartMeasurement::new(
        view,
        class_defs,
        config,
        measurer,
        math_renderer,
        labels,
        edge_style,
    );
    let look = merman_render_core::config::mermaid_config_diagram_look(config);
    let input = FlowGridInput::measure(view, &measurement, &edge_ids, look.is_neo(), work)?;
    let prepared = prepare_grid(&input.items, &input.edges, config.get("grid"), work)?;
    let routes = prepared.route_edges(work)?;
    input.project_layout(view, &prepared, routes, &measurement.settings, work)
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

struct VisibleFlow<'a> {
    parents: HashMap<&'a str, &'a str>,
    groups: HashMap<&'a str, usize>,
    placements: HashMap<GridPlacementOwner, GridPlacementSpec>,
}

impl<'a> VisibleFlow<'a> {
    fn prepare(view: FlowchartRenderModelRef<'a>, work: &OperationWorkMeter) -> Result<Self> {
        let mut parents = HashMap::new();
        let mut groups = HashMap::new();
        let mut placements = HashMap::new();
        let subgraphs = &view.semantic.subgraphs;
        work.charge(subgraphs.len())?;
        groups
            .try_reserve(subgraphs.len())
            .map_err(|_| allocation_error())?;
        for (slot, group) in subgraphs.iter().enumerate() {
            work.charge(group.id.len().max(1))?;
            if view.collapsed_replacement(&group.id).is_none() {
                groups.entry(group.id.as_str()).or_insert(slot);
            }
        }
        for (id, parent) in view.visible_memberships() {
            work.charge(id.len().max(1))?;
            parents.try_reserve(1).map_err(|_| allocation_error())?;
            parents.insert(id, parent);
        }
        let entries = view.render_context.grid_placements().entries();
        placements
            .try_reserve(entries.len())
            .map_err(|_| allocation_error())?;
        for entry in entries {
            work.charge(1)?;
            placements.insert(entry.owner, entry.placement);
        }
        Ok(Self {
            parents,
            groups,
            placements,
        })
    }

    fn item(&self, id: &'a str, owner: Option<GridPlacementOwner>, ordinal: usize) -> GridItem<'a> {
        GridItem {
            id,
            authored_id: None,
            source_ordinal: Some(ordinal),
            inline: owner
                .and_then(|owner| self.placements.get(&owner).copied())
                .unwrap_or_default(),
        }
    }

    fn direction(
        &self,
        id: &str,
        groups: &'a [FlowSubgraph],
        root: &'a str,
        work: &OperationWorkMeter,
    ) -> Result<&'a str> {
        let mut current = id;
        // The measured forest reports malformed containment after measurement. Bound this walk
        // the same way createGraph's visited set bounds direction queries on malformed input.
        for _ in 0..=self.groups.len() {
            work.charge(1)?;
            if let Some(direction) = self
                .groups
                .get(current)
                .and_then(|&slot| groups[slot].dir.as_deref())
            {
                return Ok(direction);
            }
            let Some(parent) = self.parents.get(current).copied() else {
                break;
            };
            current = parent;
        }
        Ok(root)
    }
}

impl<'a> FlowGridInput<'a> {
    fn measure(
        view: FlowchartRenderModelRef<'a>,
        measure: &FlowchartMeasurement<'_>,
        edge_ids: &'a FlowchartEdgeTransportPlan<'a>,
        look_is_neo: bool,
        work: &OperationWorkMeter,
    ) -> Result<Self> {
        let visible = VisibleFlow::prepare(view, work)?;
        let item_count = view
            .nodes
            .len()
            .checked_add(visible.groups.len())
            .ok_or_else(|| work.arithmetic_overflow())?;
        let mut items = Vec::new();
        let mut item_paint = Vec::new();
        let mut edges = Vec::new();
        let mut edge_keys = Vec::new();
        let mut slots = HashMap::new();
        work.charge(item_count)?;
        work.charge(view.edges.len())?;
        items
            .try_reserve_exact(item_count)
            .map_err(|_| allocation_error())?;
        item_paint
            .try_reserve_exact(item_count)
            .map_err(|_| allocation_error())?;
        edges
            .try_reserve_exact(view.edges.len())
            .map_err(|_| allocation_error())?;
        edge_keys
            .try_reserve_exact(view.edges.len())
            .map_err(|_| allocation_error())?;
        slots
            .try_reserve(item_count)
            .map_err(|_| allocation_error())?;

        let source = view.semantic;
        let root_direction = source.direction.as_deref().unwrap_or("TB");
        let keep_collapsed_metadata = source.keyword == "agentflow-beta";
        // FlowDB adds reverse-order subgraphs before visiting ordinary vertices. Preserve the
        // actual projected order because it controls placement occurrences and paint.
        for (slot, group) in source.subgraphs.iter().enumerate().rev() {
            work.charge(1)?;
            let id = group.id.as_str();
            if visible.groups.get(id) != Some(&slot) {
                continue;
            }
            let collapsed = view.is_subgraph_collapsed(id);
            let title = view.subgraph_title_for_render(slot, group);
            let title_metrics = if !collapsed && title.is_empty() {
                TextMetrics {
                    width: 0.0,
                    height: 0.0,
                    line_count: 0,
                }
            } else {
                measure.check_measurement(
                    measure.grid_subgraph_title(group, slot, |units| Ok(work.charge(units)?)),
                )?
            };
            let size = Size {
                width: title_metrics.width,
                height: title_metrics.height,
            };
            let kind = if collapsed {
                let (width, height) = flowchart_node_render_dimensions(
                    Some("collapsedGroup"),
                    title_metrics,
                    measure.settings.cluster_padding,
                    look_is_neo,
                );
                GridItemKind::Leaf(Size { width, height })
            } else {
                GridItemKind::Group(GridGroupMeasure {
                    padding: measure.settings.cluster_padding,
                    title: (!title.is_empty()).then_some(size),
                })
            };
            // Flowchart drops collapsed group metadata before layout. Agentflow retains it.
            let owner = (!collapsed || keep_collapsed_metadata)
                .then_some(GridPlacementOwner::Subgraph(slot));
            slots.insert(id, items.len());
            items.push(GridMeasuredItem {
                item: visible.item(id, owner, items.len()),
                parent_id: visible.parents.get(id).copied(),
                kind,
            });
            item_paint.push(GridItemPaint {
                label: MeasuredLabel {
                    width: size.width,
                    height: size.height,
                },
                group: (!collapsed).then_some(group),
                direction: visible.direction(id, &source.subgraphs, root_direction, work)?,
            });
        }
        for (slot, node) in source.nodes.iter().enumerate() {
            let id = node.id.as_str();
            work.charge(id.len().max(1))?;
            if view.collapsed_replacement(id).is_some() || slots.contains_key(id) {
                continue;
            }
            validate_flowchart_node_asset_dimensions(node)?;
            let direction = visible.direction(id, &source.subgraphs, root_direction, work)?;
            let measured = measure.check_measurement(measure.node(
                node,
                FlowchartSvgLabelOwner::Node(slot),
                direction,
                |units| Ok(work.charge(units)?),
            ))?;
            let owner = view.render_context.grid_node_owner(slot);
            slots.insert(id, items.len());
            items.push(GridMeasuredItem {
                item: visible.item(id, Some(owner), items.len()),
                parent_id: visible.parents.get(id).copied(),
                kind: GridItemKind::Leaf(Size {
                    width: measured.width,
                    height: measured.height,
                }),
            });
            item_paint.push(GridItemPaint {
                label: measured.label,
                group: None,
                direction,
            });
        }
        for (key, edge) in view.projected_edges() {
            work.charge(1)?;
            let (Some(&source), Some(&target)) = (slots.get(edge.from), slots.get(edge.to)) else {
                return Err(Error::InvalidModel {
                    message: format!(
                        "GRID_MISSING_ENDPOINT: Missing endpoint for edge \"{}\"",
                        edge.semantic.id
                    ),
                });
            };
            let (start_marker, end_marker) =
                arrow_types_for_edge(edge.semantic.edge_type.as_deref());
            let visible_edge = edge.semantic.stroke.as_deref() != Some("invisible");
            let label = measure
                .check_measurement(measure.grid_edge_label(edge.semantic, key))?
                .map(|label| GridEdgeLabel {
                    size: Size {
                        width: label.width,
                        height: label.height,
                    },
                    start_marker: visible_edge && start_marker.is_some(),
                    end_marker: visible_edge && end_marker.is_some(),
                });
            let id = edge_ids.id(key).ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "Missing private Grid edge identity for `{}`",
                    edge.semantic.id
                ),
            })?;
            edges.push(GridMeasuredEdge {
                id,
                source,
                target,
                label,
            });
            edge_keys.push(key);
        }
        Ok(Self {
            items,
            edges,
            edge_keys,
            item_paint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
    use merman_model::grid::{GridCoordinate, GridInlinePlacement, GridInlinePlacements};
    use merman_render_core::{resources::RenderResourcePolicy, text::DeterministicTextMeasurer};
    use serde_json::json;

    #[test]
    fn flow_grid_measures_nodes_and_routes_their_authored_tracks() {
        let model: FlowchartModel = serde_json::from_value(json!({
            "keyword": "flowchart", "direction": "LR",
            "nodes": [{"id":"A", "label":"Request"}, {"id":"B", "label":"Answer"}],
            "edges": [{"id":"L_A_B_0", "from":"A", "to":"B", "length":1}],
        }))
        .unwrap();
        let mut context = FlowchartRenderContext::default();
        context.set_grid_placements(GridInlinePlacements::from_entries(
            [2, -1]
                .into_iter()
                .enumerate()
                .map(|(slot, column)| GridInlinePlacement {
                    owner: GridPlacementOwner::Vertex(slot),
                    id: model.nodes[slot].id.clone(),
                    placement: GridPlacementSpec {
                        row: GridCoordinate::new(0),
                        column: GridCoordinate::new(column),
                        ..GridPlacementSpec::default()
                    },
                })
                .collect(),
        ));
        let view = FlowchartRenderModelRef::new(&model, &context);
        let config = PresentationConfig::empty_object();
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let classes = FlowchartClassDefinitions::new(&model.class_defs);
        let styles = FlowchartEdgeStylePlan::prepare_with_class_definitions(
            &model, &config, true, &classes, &work,
        )
        .unwrap();
        let text = DeterministicTextMeasurer::default();
        let measure =
            FlowchartMeasurement::new(view, &classes, &config, &text, None, None, &styles);
        let edge_ids = FlowchartEdgeTransportPlan::for_keyed_edges(
            view.projected_edges()
                .map(|(key, edge)| (key, edge.semantic)),
        );
        let projected = FlowGridInput::measure(view, &measure, &edge_ids, false, &work).unwrap();
        let prepared = prepare_grid(&projected.items, &projected.edges, None, &work).unwrap();
        let a = prepared.item_bounds(0).unwrap();
        let b = prepared.item_bounds(1).unwrap();
        let routed = prepared.route_edges(&work).unwrap();
        assert_eq!(projected.edge_keys, [FlowchartEdgeKey::new(0)]);
        assert!(
            projected
                .item_paint
                .iter()
                .all(|item| item.label.width > 0.0)
        );
        assert!(a.x > b.x + b.width);
        assert_eq!(routed[0].points.first().unwrap().x, a.x);
        assert_eq!(routed[0].points.last().unwrap().x, b.x + b.width);
        let layout = projected
            .project_layout(view, &prepared, routed, &measure.settings, &work)
            .unwrap();
        assert_eq!(layout.dom_node_order_by_root[""], ["A", "B"]);
        assert_eq!(layout.nodes[0].x, a.center().x);
        assert_eq!(layout.edges[0].id, model.edges[0].id);
        assert_eq!(layout.edges[0].points[0].x, a.x);
        assert_eq!(layout.edge_keys, projected.edge_keys);
        assert_eq!(layout.paint.curve, "rounded");
    }
}
