//! ClassDB projection onto Mermaid 12.4.0's shared Layered solver.

use std::borrow::Cow;

use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    config::json_value_is_truthy,
    graph_layout::{
        Axis, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Size,
        TitlePosition, direction_from_rank,
        mermaid_layered::{self, EdgeRouting, LayeredError, Options},
    },
    layout_work::OperationLayoutWorkControl,
    math::MathRenderer,
    resources::OperationWorkMeter,
    text::TextMeasurer,
};
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::Value;

use super::measured::{MeasureInterfaceLabel, MeasurementTarget, RelationTerminals, measure_into};
use super::{
    ClassDiagramLayout, ClassDiagramModel, ClassPreparedLabels, ClassRenderConfig, ClassRenderItem,
    ClassRenderRoot, ClassRenderRootId, ClassRenderTree, EdgeTerminalMetrics,
    apply_class_terminal_labels, class_namespace_label, compute_bounds, normalize_dir,
};

const GROUP_PAD: f64 = 20.0;
const TITLE_GAP: f64 = 12.0;

/// Paint information paired with the source-ordered layout by Class preparation.
#[derive(Debug, Default)]
pub(super) enum Paint {
    #[default]
    None,
    Routed {
        routing: EdgeRouting,
        axes: Vec<Vec<Axis>>,
        cluster_order: Vec<usize>,
        label_y_offset: f64,
    },
    #[cfg(feature = "layout-dagre")]
    DagreFallback,
}

/// `None` requests an independently admitted Dagre retry on the same operation meter.
#[allow(
    clippy::too_many_arguments,
    reason = "Class providers share prepared styles and operation-owned measurement inputs."
)]
pub fn layout(
    model: &ClassDiagramModel,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render: &ClassRenderConfig,
    measure_interface_label: MeasureInterfaceLabel,
    work_control: &OperationLayoutWorkControl,
    work: &OperationWorkMeter,
) -> Result<Option<(ClassDiagramLayout, ClassPreparedLabels)>> {
    let namespace_count = model.namespaces().len();
    let node_count = work_control.checked_add(model.classes().len(), model.notes().len())?;
    let node_count = work_control.checked_add(node_count, model.interfaces().len())?;
    let edge_count = work_control.checked_add(model.notes().len(), model.relations().len())?;
    work.charge(work_control.checked_add(namespace_count, node_count)?)?;
    work.charge(edge_count)?;
    let mut measured = ClassMeasurement {
        groups: buffer(namespace_count)?,
        nodes: buffer(node_count)?,
        edges: buffer(edge_count)?,
        work,
    };
    let mut labels = measure_into(
        model,
        config,
        measurer,
        math_renderer,
        render,
        false,
        measure_interface_label,
        Some(work_control),
        &mut measured,
    )?;
    work_control.checkpoint()?;

    let routing = EdgeRouting::from_name(config.get_str("layered.edgeRouting"));
    let Some(input) = measured.graph(model, config, routing)? else {
        work_control.checkpoint()?;
        return Ok(None);
    };
    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        mermaid_layered::layout(&input, Options::default(), work)
    };
    let solved = match solved {
        Ok(layout) => layout.graph,
        Err(LayeredError::Work(error)) => return Err(error.into()),
        Err(LayeredError::ArithmeticOverflow) => return Err(work.arithmetic_overflow().into()),
        Err(LayeredError::AllocationFailed) => return Err(allocation_error()),
        Err(error) if error.is_recoverable() => {
            work_control.checkpoint()?;
            return Ok(None);
        }
        Err(error) => {
            return Err(Error::InvalidModel {
                message: error.to_string(),
            });
        }
    };
    if solved.nodes.len() != measured.nodes.len()
        || solved.groups.len() != measured.groups.len()
        || solved.edges.len() != measured.edges.len()
    {
        return Err(invalid_projection(
            "output does not match ClassDB occurrences",
        ));
    }

    let settings = render.layout_settings();
    let direction = normalize_dir(&model.model().direction);
    let top_margin = title_margin(config, "top");
    let bottom_margin = title_margin(config, "bottom");
    let label_y_offset = (top_margin + bottom_margin) / 2.0;
    let mut nodes = buffer(work_control.checked_add(solved.groups.len(), solved.nodes.len())?)?;
    let mut clusters = buffer(solved.groups.len())?;
    let mut root = ClassRenderRoot {
        cluster_ids: buffer(solved.groups.len())?,
        edge_ids: buffer(solved.edges.len())?,
        items: buffer(solved.nodes.len())?,
        ..Default::default()
    };
    for (source, group) in measured.groups.iter().zip(&solved.groups) {
        let id = source.id;
        let rect = group.rect;
        let center = rect.center();
        let title = class_namespace_label(model, id);
        work.charge(id.len().max(1))?;
        work.charge(title.len().max(1))?;
        let size = source.size;
        clusters.push(LayoutCluster {
            id: id.to_owned(),
            x: center.x,
            y: center.y,
            width: rect.width,
            height: rect.height,
            diff: -settings.namespace_padding,
            offset_y: size.height - settings.namespace_padding / 2.0,
            title: title.to_owned(),
            // The engine reserves a title band. The common cluster painter places the actual
            // title at the authored top margin, separately from the engine's title center.
            title_label: LayoutLabel {
                x: center.x,
                y: rect.y + top_margin + size.height / 2.0,
                width: size.width,
                height: size.height,
            },
            requested_dir: None,
            effective_dir: direction.clone(),
            padding: settings.namespace_padding,
            title_margin_top: top_margin,
            title_margin_bottom: bottom_margin,
        });
        nodes.push(layout_node(id, rect, true, Some((size.width, size.height))));
        root.cluster_ids.push(id.to_owned());
    }
    for (source, rect) in measured.nodes.iter().zip(solved.nodes) {
        work.charge(source.id.len().max(1))?;
        let metrics = labels.node_label_metrics_by_id.get(source.id).copied();
        nodes.push(layout_node(source.id, rect, false, metrics));
        root.items.push(ClassRenderItem::Node(source.id.to_owned()));
    }

    let mut edges = buffer(solved.edges.len())?;
    let mut axes = buffer(solved.edges.len())?;
    // The common renderer paints the original LayoutData edges, including note attachments
    // before relations. The kernel's internal order is for group-to-member routing only.
    for (source, route) in measured.edges.into_iter().zip(solved.edges) {
        if route.points.len() < 2 {
            work_control.checkpoint()?;
            return Ok(None);
        }
        work.charge(source.id.len().max(1))?;
        work.charge(route.points.len())?;
        work.charge(route.points.len())?; // The final bounds scan also visits every point.
        let mut points = buffer(route.points.len())?;
        let mut tags = buffer(route.points.len())?;
        // Class boxes, notes, and interface shells have rectangular intersections. Layered's
        // measured box contacts are already on those outlines, including for self-loops.
        for point in route.points {
            points.push(LayoutPoint {
                x: point.x,
                y: point.y,
            });
            tags.push(point.axis);
        }
        let label = route.label.map(|label| LayoutLabel {
            x: label.x,
            y: label.y,
            width: source.size.width,
            height: source.size.height,
        });
        if label
            .as_ref()
            .is_some_and(|label| !(label.y + label_y_offset).is_finite())
        {
            work_control.checkpoint()?;
            return Ok(None);
        }
        root.edge_ids.push(source.id.to_string());
        let mut edge = LayoutEdge {
            id: source.id.into_owned(),
            from: source.from.to_owned(),
            to: source.to.to_owned(),
            from_cluster: matches!(route.source, Endpoint::Group(_))
                .then(|| source.from.to_owned()),
            to_cluster: matches!(route.target, Endpoint::Group(_)).then(|| source.to.to_owned()),
            points: Vec::new(),
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        };
        if let Some(terminals) = source.terminals
            && (terminals.start_right.is_some() || terminals.end_left.is_some())
        {
            work.charge(points.len())?;
            work.charge(points.len())?;
            if let Err(error) = apply_class_terminal_labels(&mut edge, &terminals, &points) {
                work_control.checkpoint()?;
                if matches!(error, Error::InvalidModel { .. }) {
                    // Mermaid's common terminal placement can reject an unsampleable route.
                    return Ok(None);
                }
                return Err(error);
            }
        }
        edge.points = points;
        edges.push(edge);
        axes.push(tags);
    }
    let cluster_order = cluster_paint_order(&input.groups, work_control, work)?;
    let bounds = compute_bounds(&nodes, &edges, &clusters);
    work_control.checkpoint()?;
    labels.prepared_labels.layered = Paint::Routed {
        routing,
        axes,
        cluster_order,
        label_y_offset,
    };
    Ok(Some((
        ClassDiagramLayout {
            nodes,
            edges,
            clusters,
            bounds,
            uses_elk_adapter_dom: false,
            class_label_plans_by_id: labels.class_label_plans_by_id,
            render_tree: ClassRenderTree {
                roots: vec![root],
                top: ClassRenderRootId(0),
            },
        },
        labels.prepared_labels,
    )))
}

struct ClassMeasurement<'a, 'work> {
    groups: Vec<MeasuredItem<'a>>,
    nodes: Vec<MeasuredItem<'a>>,
    edges: Vec<MeasuredRelation<'a>>,
    work: &'work OperationWorkMeter,
}

struct MeasuredItem<'a> {
    id: &'a str,
    parent: Option<&'a str>,
    size: Size,
}

struct MeasuredRelation<'a> {
    id: Cow<'a, str>,
    from: &'a str,
    to: &'a str,
    size: Size,
    terminals: Option<EdgeTerminalMetrics>,
}

impl<'a> MeasurementTarget<'a> for ClassMeasurement<'a, '_> {
    fn uses_layered_labels(&self) -> bool {
        true
    }

    fn namespace(&mut self, id: &'a str, parent: Option<&'a str>, size: (f64, f64)) -> Result<()> {
        self.work.charge(id.len().max(1))?;
        self.groups.push(MeasuredItem {
            id,
            parent,
            size: measured_size(size),
        });
        Ok(())
    }

    fn node(&mut self, id: &'a str, parent: Option<&'a str>, size: (f64, f64)) -> Result<()> {
        self.work.charge(id.len().max(1))?;
        self.nodes.push(MeasuredItem {
            id,
            parent,
            size: measured_size(size),
        });
        Ok(())
    }

    fn edge(
        &mut self,
        from: &'a str,
        to: &'a str,
        id: Cow<'a, str>,
        size: (f64, f64),
        terminals: Option<RelationTerminals<'a>>,
    ) -> Result<()> {
        self.work.charge(id.len().max(1))?;
        self.edges.push(MeasuredRelation {
            id,
            from,
            to,
            size: measured_size(size),
            terminals: terminals.map(|terminals| EdgeTerminalMetrics {
                start_left: None,
                start_right: terminals.start_right,
                end_left: terminals.end_left,
                end_right: None,
                // ClassDB supplies a truthy arrow name even for `none`.
                start_marker: 10.0,
                end_marker: 10.0,
                #[cfg(feature = "layout-elk")]
                start_arrow_type: super::class_arrow_type_for_relation_end(
                    terminals.relation.relation.type1,
                ),
                #[cfg(feature = "layout-elk")]
                end_arrow_type: super::class_arrow_type_for_relation_end(
                    terminals.relation.relation.type2,
                ),
            }),
        });
        Ok(())
    }
}

impl ClassMeasurement<'_, '_> {
    fn graph(
        &self,
        model: &ClassDiagramModel,
        config: &PresentationConfig,
        routing: EdgeRouting,
    ) -> Result<Option<MeasuredGraph>> {
        let work = self.work;
        let mut slots = FxHashMap::default();
        slots
            .try_reserve(
                self.groups
                    .len()
                    .checked_add(self.nodes.len())
                    .ok_or_else(|| work.arithmetic_overflow())?,
            )
            .map_err(|_| allocation_error())?;
        for (items, endpoint) in [
            (&self.nodes, Endpoint::Node as fn(u32) -> Endpoint),
            (&self.groups, Endpoint::Group),
        ] {
            for (index, item) in items.iter().enumerate() {
                work.charge(item.id.len().max(1))?;
                if slots
                    .insert(item.id, endpoint(slot(index, work)?))
                    .is_some()
                {
                    // The pinned Layered engine rejects duplicate node/group IDs before solving.
                    return Ok(None);
                }
            }
        }
        let parent_slot = |parent: Option<&str>| -> Result<_> {
            let Some(parent) = parent else {
                return Ok(None);
            };
            work.charge(parent.len().max(1))?;
            Ok(match slots.get(parent) {
                Some(Endpoint::Group(index)) => Some(*index),
                _ => None,
            })
        };
        let mut nodes = buffer(self.nodes.len())?;
        for (index, source) in self.nodes.iter().enumerate() {
            work.charge(1)?;
            nodes.push(MeasuredNode::new(
                source.size,
                slot(index, work)?,
                parent_slot(source.parent)?,
            ));
        }
        let mut groups = buffer(self.groups.len())?;
        let title_top = title_margin(config, "top").max(0.0)
            + title_margin(config, "bottom").max(0.0)
            + TITLE_GAP;
        for (index, source) in self.groups.iter().enumerate() {
            work.charge(1)?;
            let title =
                (source.size.width > 0.0 && source.size.height > 0.0).then_some(source.size);
            let top = if title.is_some() {
                title_top
            } else {
                GROUP_PAD
            };
            groups.push(MeasuredGroup {
                seq: slot(index, work)?,
                parent: parent_slot(source.parent)?,
                direction: None,
                padding: kurbo::Insets::new(GROUP_PAD, top, GROUP_PAD, GROUP_PAD),
                min_size: Size::default(),
                title,
                title_position: TitlePosition::TopCenter,
            });
        }
        let mut edge_ids = FxHashSet::default();
        edge_ids
            .try_reserve(self.edges.len())
            .map_err(|_| allocation_error())?;
        let mut edges = buffer(self.edges.len())?;
        for edge in &self.edges {
            work.charge(edge.id.len().max(1))?;
            work.charge(edge.from.len().max(1))?;
            work.charge(edge.to.len().max(1))?;
            let (Some(&source), Some(&target)) = (slots.get(edge.from), slots.get(edge.to)) else {
                return Ok(None);
            };
            if !edge_ids.insert(edge.id.as_ref()) {
                return Ok(None);
            }
            edges.push(MeasuredEdge {
                source,
                target,
                min_rank_span: 1,
                weight: 1.0,
                label: (edge.size.width > 0.0).then_some(Size {
                    width: edge.size.width + 12.0,
                    height: edge.size.height + 4.0,
                }),
                start_label: None,
                end_label: None,
                start_marker: Default::default(),
                end_marker: Default::default(),
            });
        }
        Ok(Some(MeasuredGraph {
            direction: direction_from_rank(&model.model().direction),
            routing: routing.geometry(),
            node_spacing: spacing(config, "nodeSpacing"),
            rank_spacing: spacing(config, "rankSpacing"),
            component_spacing: 0.0,
            inherit_crossed_group_direction: true,
            nodes,
            groups,
            edges,
        }))
    }
}

fn layout_node(
    id: &str,
    rect: merman_render_core::graph_layout::Rect,
    is_cluster: bool,
    label: Option<(f64, f64)>,
) -> LayoutNode {
    let center = rect.center();
    LayoutNode {
        id: id.to_owned(),
        x: center.x,
        y: center.y,
        width: rect.width,
        height: rect.height,
        is_cluster,
        label_width: label.map(|size| size.0),
        label_height: label.map(|size| size.1),
    }
}

fn cluster_paint_order(
    groups: &[MeasuredGroup],
    control: &OperationLayoutWorkControl,
    work: &OperationWorkMeter,
) -> Result<Vec<usize>> {
    let mut depths = buffer(groups.len())?;
    for group in groups {
        work.charge(1)?;
        let mut depth = 0;
        let mut parent = group.parent;
        // Class's shared measurement validates nesting before visiting namespace titles.
        while let Some(index) = parent {
            work.charge(1)?;
            depth += 1;
            parent = groups[index as usize].parent;
        }
        depths.push(depth);
    }
    let mut order = buffer(groups.len())?;
    order.extend(0..groups.len());
    let sort_passes = groups
        .len()
        .checked_ilog2()
        .map_or(0, |log| log as usize + 1);
    work.charge(control.checked_mul(groups.len(), sort_passes)?)?;
    order.sort_unstable_by_key(|&index| (depths[index], index));
    Ok(order)
}

fn spacing(config: &PresentationConfig, key: &str) -> f64 {
    // Class's unified renderer uses `state[key] || 50`. Layered then accepts only positive
    // numeric values, falling through to the flowchart section when that truthy value is invalid.
    match config
        .as_config()
        .get_path(&["state", key])
        .filter(|value| json_value_is_truthy(value))
    {
        None => 50.0,
        Some(value) => positive_number(value)
            .or_else(|| {
                config
                    .as_config()
                    .get_path(&["flowchart", key])
                    .and_then(positive_number)
            })
            .unwrap_or(50.0),
    }
}

fn title_margin(config: &PresentationConfig, side: &str) -> f64 {
    config
        .as_config()
        .get_path(&["flowchart", "subGraphTitleMargin", side])
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

fn positive_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn measured_size((width, height): (f64, f64)) -> Size {
    let size = |value: f64| {
        if value.is_finite() && value > 0.0 {
            value
        } else {
            0.0
        }
    };
    Size {
        width: size(width),
        height: size(height),
    }
}

fn slot(index: usize, work: &OperationWorkMeter) -> Result<u32> {
    u32::try_from(index).map_err(|_| work.arithmetic_overflow().into())
}

pub(super) fn buffer<T>(len: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| allocation_error())?;
    Ok(values)
}

pub(super) fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "layered" }
}

pub(super) fn invalid_projection(detail: &'static str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid Class Layered projection: {detail}"),
    }
}

#[cfg(test)]
mod tests;
