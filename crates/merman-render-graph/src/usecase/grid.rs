//! UsecaseDB's measured graph projected into Mermaid Grid in DB order.

mod label;

pub(super) use label::{GridLabel, GridLabels, MERMAID_EDGE_LABEL_FONT_SIZE};

use super::{UsecaseDiagramLayout, UsecaseMeasured, UsecasePrepared, layout, measure, resolve_css};
use merman_model::{
    OperationPhase, families::usecase::UsecaseDiagramModel, presentation::PresentationConfig,
};
use merman_render_core::{
    Bounds, Error, LayoutLabel, LayoutPoint, Result,
    graph_layout::{
        Size,
        mermaid_grid::{
            GridEdgeLabel, GridGroupMeasure, GridItem, GridItemKind, GridMeasuredEdge,
            GridMeasuredItem, GridPaint, prepare_grid,
        },
    },
    layout_work::OperationLayoutWorkControl,
    math::MathRenderer,
    resources::OperationWorkMeter,
    text::TextMeasurer,
};
use merman_theme_contract::v2::{Color, FamilyTheme};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
pub(super) struct UsecaseGridPaint {
    pub(super) settings: GridPaint,
    pub(super) labels: Vec<GridLabel>,
}

pub fn prepare(
    model: &UsecaseDiagramModel,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    canvas: Option<Color>,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work: &Arc<OperationWorkMeter>,
) -> Result<UsecasePrepared> {
    work.checkpoint(OperationPhase::Layout)?;
    let css = resolve_css(config, native, canvas);
    let mut control = OperationLayoutWorkControl::new(Arc::clone(work));
    let mut grid_labels = GridLabels::new(
        config,
        &css.root_font,
        native.is_some(),
        measurer,
        math_renderer,
        work,
    );
    let (node_plans, edge_plans) = measure::measure(
        model,
        config,
        &css,
        measurer,
        math_renderer,
        &mut control,
        Some(&mut grid_labels),
    )?;
    let labels = grid_labels.finish();

    work.charge(node_plans.len())?;
    let mut slots = HashMap::new();
    slots
        .try_reserve(node_plans.len())
        .map_err(|_| allocation_error())?;
    let mut items = buffer(node_plans.len())?;
    for (slot, node) in node_plans.iter().enumerate() {
        work.charge(node.id.len().max(1))?;
        slots.insert(node.id.as_str(), slot);
        let kind = if node.is_boundary {
            GridItemKind::Group(GridGroupMeasure {
                padding: 20.0,
                title: (!node.label.text.is_empty()).then_some(Size {
                    width: node.label.metrics.width,
                    height: node.label.metrics.height,
                }),
            })
        } else {
            GridItemKind::Leaf(Size {
                width: node.width,
                height: node.height,
            })
        };
        items.push(GridMeasuredItem {
            item: GridItem {
                id: &node.id,
                authored_id: None,
                source_ordinal: Some(slot),
                inline: Default::default(),
            },
            parent_id: node.parent.as_deref(),
            kind,
        });
    }

    work.charge(edge_plans.len())?;
    let mut edges = buffer(edge_plans.len())?;
    let mut label_slots = labels.iter().peekable();
    for (slot, edge) in edge_plans.iter().enumerate() {
        work.charge(edge.source.len())?;
        work.charge(edge.target.len())?;
        let (Some(&source), Some(&target)) = (
            slots.get(edge.source.as_str()),
            slots.get(edge.target.as_str()),
        ) else {
            return Err(Error::InvalidModel {
                message: format!(
                    "GRID_MISSING_ENDPOINT: Missing endpoint for edge \"{}\"",
                    edge.id
                ),
            });
        };
        let label = if label_slots
            .peek()
            .is_some_and(|label| label.edge_slot == slot)
        {
            label_slots.next().map(|label| GridEdgeLabel {
                size: label.size(),
                start_marker: edge.start_marker.is_some(),
                end_marker: edge.end_marker.is_some(),
            })
        } else {
            None
        };
        edges.push(GridMeasuredEdge {
            id: &edge.id,
            source,
            target,
            label,
        });
    }

    let grid = prepare_grid(&items, &edges, config.get("grid"), work)?;
    let routes = grid.route_edges(work)?;
    if routes.len() != edges.len() {
        return Err(invalid_projection(
            "route count differs from source edge count",
        ));
    }

    let mut nodes = buffer(items.len())?;
    for (slot, node) in node_plans.iter().enumerate() {
        work.charge(node.id.len().max(1))?;
        let rect = grid
            .item_bounds(slot)
            .ok_or_else(|| invalid_projection("missing item bounds"))?;
        let center = rect.center();
        nodes.push(layout::node(
            node,
            center.x,
            center.y,
            rect.width,
            rect.height,
        ));
    }
    let mut layout_edges = buffer(routes.len())?;
    for ((plan, measured), route) in edge_plans.iter().zip(&edges).zip(routes) {
        work.charge(plan.id.len().max(1))?;
        work.charge(route.points.len())?;
        work.charge(route.points.len())?;
        let mut points = buffer(route.points.len())?;
        points.extend(route.points.into_iter().map(|point| LayoutPoint {
            x: point.x,
            y: point.y,
        }));
        let label = match (measured.label, route.label_center) {
            (Some(label), Some(center)) => Some(LayoutLabel {
                x: center.x,
                y: center.y,
                width: label.size.width,
                height: label.size.height,
            }),
            (None, None) => None,
            _ => {
                return Err(invalid_projection(
                    "label center differs from measured edge",
                ));
            }
        };
        layout_edges.push(layout::edge(plan, points, label));
    }
    work.charge(nodes.len())?;
    work.charge(layout_edges.len())?;
    let bounds = Bounds::from_points(
        nodes
            .iter()
            .flat_map(|node| {
                [
                    (node.x - node.width / 2.0, node.y - node.height / 2.0),
                    (node.x + node.width / 2.0, node.y + node.height / 2.0),
                ]
            })
            .chain(
                layout_edges
                    .iter()
                    .flat_map(|edge| edge.points.iter().map(|point| (point.x, point.y))),
            )
            .chain(
                labels
                    .iter()
                    .filter_map(|terminal| {
                        let label = layout_edges[terminal.edge_slot].label.as_ref()?;
                        Some((label, terminal.bounds()))
                    })
                    .flat_map(|(label, rect)| {
                        [
                            (label.x + rect.x, label.y + rect.y),
                            (
                                label.x + rect.x + rect.width,
                                label.y + rect.y + rect.height,
                            ),
                        ]
                    }),
            ),
    );
    let mut settings = grid.paint();
    if !config.explicit_config_owns_path("grid.curve")
        && let Some(curve) = css.native_curve()
    {
        settings.curve = if curve == "step" { "linear" } else { curve };
    }
    work.checkpoint(OperationPhase::Layout)?;
    let mut prepared = UsecasePrepared::new(
        UsecaseMeasured {
            css,
            nodes: node_plans,
            edges: edge_plans,
        },
        UsecaseDiagramLayout {
            nodes,
            edges: layout_edges,
            bounds,
        },
        HashMap::new(),
        false,
    );
    prepared.grid = Some(UsecaseGridPaint { settings, labels });
    Ok(prepared)
}

pub(super) fn buffer<T>(capacity: usize) -> Result<Vec<T>> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_error())?;
    Ok(buffer)
}

pub(super) fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

pub(super) fn invalid_projection(detail: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid Usecase Grid projection: {detail}"),
    }
}
