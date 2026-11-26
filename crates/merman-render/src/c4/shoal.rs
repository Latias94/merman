mod labels;

use super::layout::{
    C4Measurement, measure_c4_relation, measure_c4_shape, prepare_c4_boundary_layout,
};
use super::{C4_ROUNDED_RADIUS, C4ClusterThemePlan, C4ConfigView, C4TypographyThemePlan};
use crate::graph_layout::Marker;
use crate::graph_layout::canonical::CanonicalRoutes;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Rect,
    RoutingStyle, Side, Size, TitlePosition,
};
use crate::graph_layout::placement;
use crate::graph_layout::route_paint::RouteStyle;
use crate::layout_frame::GraphPlacement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, C4DiagramLayout, LayoutEdge, LayoutPoint};
use crate::{Error, LayoutAdaptation, LayoutAdaptationObservation, LayoutExecution, Result};
use kurbo::{Insets, Vec2};
use merman_layout_shoal::{WidthPolicy, card_grid};
use merman_model::families::c4::C4DiagramModel;
use merman_render_core::graph_layout::shoal::{Placement, project_input, project_output};
use merman_theme_contract::v2::EdgeStyle;
use rustc_hash::FxHashMap;
use serde_json::Value;

const ROOT: &str = "global";

// The shared markers extend one unit past their reference. The themed marker keeps that
// setback after adding a rounded stroke. Its half stroke also increases the reserved width.
const MARKER: Marker = Marker {
    trim: 1.0,
    depth: 10.0,
    width: 11.0,
};

#[derive(Debug)]
pub(crate) struct C4ShoalGeometry {
    placement: GraphPlacement,
    pub(crate) boundary_title_x: Vec<Option<f64>>,
    pub(crate) relations: Vec<C4ShoalRelation>,
}

#[derive(Debug)]
pub(crate) struct C4ShoalRelation {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) points: Vec<LayoutPoint>,
    pub(crate) label: Option<labels::PlacedRelationLabel>,
}

impl C4ShoalGeometry {
    pub(crate) const fn placement(&self) -> &GraphPlacement {
        &self.placement
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn index(value: usize, work: &OperationLayoutWorkControl) -> Result<u32> {
    u32::try_from(value).map_err(|_| work.record_arithmetic_overflow().into())
}

fn include_ink(bounds: &mut Bounds, ink: &Bounds, stroke_outset: f64) {
    bounds.min_x = bounds.min_x.min(ink.min_x - stroke_outset);
    bounds.min_y = bounds.min_y.min(ink.min_y - stroke_outset);
    bounds.max_x = bounds.max_x.max(ink.max_x + stroke_outset);
    bounds.max_y = bounds.max_y.max(ink.max_y + stroke_outset);
}

pub(crate) fn layout_c4_shoal(
    model: &C4DiagramModel,
    effective_config: &Value,
    typography: &C4TypographyThemePlan,
    cluster_theme: &C4ClusterThemePlan,
    execution: &LayoutExecution<'_>,
) -> Result<(C4DiagramLayout, C4ShoalGeometry)> {
    let mut work = OperationLayoutWorkControl::new(execution.work_meter());
    let shape_count = model.shapes.len();
    let boundary_count = model.boundaries.len();
    let relation_count = model.rels.len();
    work.charge_adapter(work.checked_add(
        work.checked_add(shape_count, boundary_count)?,
        relation_count,
    )?)?;

    let conf = C4ConfigView::new(effective_config).layout_settings();
    let measure = C4Measurement {
        typography,
        conf: &conf,
        measurer: execution.text_measurer(),
        work: execution.work_meter_ref(),
    };
    let mut node_indices = FxHashMap::default();
    for (ordinal, shape) in model.shapes.iter().enumerate() {
        work.charge_adapter(shape.alias.len())?;
        if node_indices
            .insert(shape.alias.as_str(), index(ordinal, &work)?)
            .is_some()
        {
            return Err(invalid(format!(
                "c4: duplicate element alias `{}`",
                shape.alias
            )));
        }
    }
    let mut root = model
        .boundaries
        .iter()
        .filter(|boundary| boundary.alias == ROOT);
    if root
        .next()
        .is_none_or(|root| !root.parent_boundary.is_empty())
        || root.next().is_some()
    {
        return Err(invalid("c4: expected one implicit global boundary"));
    }
    // Normally the synthetic boundary is the unboxed root. A relation can name its border,
    // in which case the group supplies that contact without emitting a visible frame.
    let route_to_root = !node_indices.contains_key(ROOT)
        && model
            .rels
            .iter()
            .any(|relation| relation.from_alias == ROOT || relation.to_alias == ROOT);
    let mut group_ids = Vec::with_capacity(boundary_count);
    let mut group_indices = FxHashMap::default();
    for boundary in &model.boundaries {
        work.charge_adapter(boundary.alias.len())?;
        let alias = boundary.alias.as_str();
        if alias == ROOT && !route_to_root {
            continue;
        }
        if group_indices
            .insert(alias, index(group_ids.len(), &work)?)
            .is_some()
        {
            return Err(invalid(format!("c4: duplicate boundary alias `{alias}`")));
        }
        group_ids.push(alias);
    }
    let parent = |alias: &str| {
        if alias == ROOT && !route_to_root {
            return Ok(None);
        }
        group_indices
            .get(alias)
            .copied()
            .map(Some)
            .ok_or_else(|| invalid(format!("c4: missing parent boundary `{alias}`")))
    };
    let endpoint = |alias: &str, terminal: &str| {
        node_indices
            .get(alias)
            .copied()
            .map(Endpoint::Node)
            .or_else(|| group_indices.get(alias).copied().map(Endpoint::Group))
            .ok_or_else(|| {
                invalid(format!(
                    "c4: relationship references missing {terminal} element {alias}"
                ))
            })
    };

    let padding = conf.c4_shape_padding.max(C4_ROUNDED_RADIUS);
    let spacing = conf.c4_shape_margin.max(2.0 * C4_ROUNDED_RADIUS);
    let native = typography.native_style();
    let node_radius = native.map_or(C4_ROUNDED_RADIUS, |theme| theme.radius);
    let stroke_outset = native.map_or(0.5, |theme| theme.connector_width / 2.0);
    let route_style = match native.map(|theme| theme.edge_style) {
        // A short rounded elbow stays within the measured card-grid channels.
        Some(EdgeStyle::Curved) => RouteStyle::Rounded(5.0),
        Some(EdgeStyle::Straight) => RouteStyle::Linear,
        Some(EdgeStyle::Orthogonal | EdgeStyle::Sketch) | None => RouteStyle::Step,
    };
    let mut graph = MeasuredGraph {
        direction: Direction::Down,
        routing: RoutingStyle::Orthogonal,
        node_spacing: spacing,
        rank_spacing: spacing,
        component_spacing: spacing,
        inherit_crossed_group_direction: false,
        nodes: Vec::with_capacity(shape_count),
        groups: Vec::with_capacity(group_ids.len()),
        edges: Vec::with_capacity(relation_count),
    };
    let mut shapes = Vec::with_capacity(shape_count);
    let mut profiles = Vec::with_capacity(shape_count);
    let mut shape_offsets = Vec::with_capacity(shape_count);
    for (ordinal, shape) in model.shapes.iter().enumerate() {
        let text_bytes = work.checked_add(
            shape.label.as_str().len(),
            shape.descr.as_ref().map_or(0, |text| text.as_str().len()),
        )?;
        let text_bytes = work.checked_add(
            text_bytes,
            shape.techn.as_ref().map_or(0, |text| text.as_str().len()),
        )?;
        work.charge_adapter(text_bytes)?;
        let layout = measure_c4_shape(shape, &measure);
        let (profile, size, dx) = ShapePortProfile::c4_shape(
            super::c4_node_shape(shape),
            conf.look,
            Size {
                width: layout.width,
                height: layout.height,
            },
            node_radius,
        );
        graph.nodes.push(MeasuredNode {
            size,
            contour: profile.shoal_contour(size),
            seq: index(ordinal, &work)?,
            parent: parent(&shape.parent_boundary)?,
            port_span: Some(profile.port_span(Side::Top)),
            perpendicular_port_span: Some(profile.port_span(Side::Left)),
            anchor_y: profile.anchor_y(),
        });
        shapes.push(layout);
        profiles.push(profile);
        shape_offsets.push(dx);
    }

    let title_width = execution
        .screen_available_width
        .unwrap_or(execution.container_width);
    let mut boundaries = Vec::with_capacity(boundary_count);
    let mut title_centers = Vec::with_capacity(boundary_count);
    for (ordinal, boundary) in model.boundaries.iter().enumerate() {
        let (layout, _) = prepare_c4_boundary_layout(boundary, title_width, &measure)?;
        let title = if boundary.alias == ROOT {
            None
        } else {
            labels::title_ink(&layout, &measure)?
        };
        if group_indices.contains_key(boundary.alias.as_str()) {
            let height = title.map_or(0.0, |ink| ink.height());
            graph.groups.push(MeasuredGroup {
                seq: index(ordinal, &work)?,
                parent: if boundary.alias == ROOT {
                    None
                } else {
                    parent(&boundary.parent_boundary)?
                },
                direction: None,
                padding: Insets::new(padding, padding + height, padding, padding),
                min_size: Size {
                    width: 2.0 * padding,
                    height: 2.0 * padding,
                },
                title: title.map(|ink| Size {
                    width: ink.width(),
                    height,
                }),
                title_position: TitlePosition::TopCenter,
            });
        }
        title_centers.push(title.map(|ink| ink.center().y));
        boundaries.push(layout);
    }
    let mut relations = Vec::with_capacity(relation_count);
    let mut labels = Vec::with_capacity(relation_count);
    let mut ids = Vec::with_capacity(relation_count);
    for (ordinal, rel) in model.rels.iter().enumerate() {
        let mut layout = measure_c4_relation(rel, ordinal, &model.c4_type, &measure)?;
        let label = labels::RelationLabel::prepare(&mut layout, &measure)?;
        let (start, end) = super::c4_relation_markers(&rel.rel_type);
        graph.edges.push(MeasuredEdge {
            source: endpoint(&rel.from_alias, "from")?,
            target: endpoint(&rel.to_alias, "to")?,
            min_rank_span: 1,
            weight: 1.0,
            label: label.as_ref().map(labels::RelationLabel::size),
            start_label: None,
            end_label: None,
            start_marker: if start { MARKER } else { Marker::default() },
            end_marker: if end { MARKER } else { Marker::default() },
        });
        relations.push(layout);
        labels.push(label);
        ids.push(format!("c4-rel-{ordinal}"));
    }
    let input = Placement {
        width: if execution.adaptation == LayoutAdaptation::FitWidth {
            WidthPolicy::FitWidth {
                max_width: execution.container_width,
                allow_turn: false,
                turn_breakpoint: 0.0,
                fold_wide_ranks: true,
            }
        } else {
            WidthPolicy::Natural
        },
        capture_ranks: execution.capture_shoal_frame,
        ..Default::default()
    };
    let options = input.options_for(&graph);
    let measured = project_input(&graph, &input, &mut work)?;
    let geometry = crate::layout_work::measure_layout_solve!(card_grid::layout_with_control(
        &measured, &options, &mut work,
    ))
    .map_err(|error| work.map_shoal_error(error))?;
    let solved = project_output(&graph, geometry, &mut work)?;
    let mut placement = placement::placement_for_solved(
        &graph,
        &solved,
        model.shapes.iter().map(|shape| shape.alias.as_str()),
        group_ids.iter().copied(),
        execution,
        &mut work,
    )?;
    let routes = CanonicalRoutes::from_solved(
        &graph,
        &solved,
        &profiles,
        |id| {
            cluster_theme
                .boundary_tokens(group_ids[id as usize])
                .and_then(|(_, _, radius)| radius.parse().ok())
                .unwrap_or(0.0)
        },
        ids.iter().map(String::as_str),
        &mut work,
    )?;

    for ((shape, rect), dx) in shapes.iter_mut().zip(&solved.nodes).zip(shape_offsets) {
        let center = rect.center();
        shape.x = center.x - shape.width / 2.0 - dx;
        shape.y = center.y - shape.height / 2.0;
    }
    let mut boundary_title_x = Vec::with_capacity(boundary_count);
    for (boundary, title_y) in boundaries.iter_mut().zip(title_centers) {
        let group = group_indices
            .get(boundary.alias.as_str())
            .map(|&id| &solved.groups[id as usize]);
        let rect = group.map_or(
            Rect {
                width: solved.bounds.width,
                height: solved.bounds.height,
                ..Default::default()
            },
            |group| group.rect,
        );
        boundary.x = rect.x;
        boundary.y = rect.y;
        boundary.width = rect.width;
        boundary.height = rect.height;
        if boundary.alias != ROOT {
            boundary.parent_boundary = group
                .and_then(|group| group.parent)
                .map_or(ROOT, |id| group_ids[id as usize])
                .to_owned();
        }
        let title = group.and_then(|group| group.title.as_ref());
        boundary_title_x.push(title.map(|title| title.x));
        if let Some((title, original)) = title.zip(title_y) {
            let dy = title.y - rect.y - original;
            boundary.label.y += dy;
            for section in [&mut boundary.ty, &mut boundary.descr]
                .into_iter()
                .flatten()
            {
                section.y += dy;
            }
        }
    }

    let mut bounds = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: solved.bounds.width,
        max_y: solved.bounds.height,
    };
    let mut geometry = Vec::with_capacity(relation_count);
    for ((rel, id), (edge, label)) in relations
        .iter_mut()
        .zip(ids)
        .zip(solved.edges.into_iter().zip(labels))
    {
        work.charge_adapter(edge.points.len())?;
        let edge = LayoutEdge {
            id,
            from: rel.from.clone(),
            to: rel.to.clone(),
            from_cluster: None,
            to_cluster: None,
            points: edge
                .points
                .into_iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect(),
            label: edge.label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        };
        let painted = routes.paint(
            &edge,
            Vec2::ZERO,
            stroke_outset,
            route_style,
            execution.work_meter_ref(),
        )?;
        let [start, .., end] = painted.data_points.as_slice() else {
            return Err(invalid("c4: missing canonical relation contacts"));
        };
        rel.start_point = start.clone();
        rel.end_point = end.clone();
        include_ink(&mut bounds, &painted.path.bounds, stroke_outset);
        let label = label
            .as_ref()
            .zip(edge.label.as_ref())
            .map(|(label, placed)| label.place(placed, &painted, stroke_outset, &mut work))
            .transpose()?;
        geometry.push(C4ShoalRelation {
            id: edge.id,
            path: painted.path.d,
            points: painted.data_points,
            label,
        });
    }
    let ink_width = bounds.max_x - bounds.min_x;
    let ink_height = bounds.max_y - bounds.min_y;
    if placement.adaptation.is_some() {
        placement.adaptation = Some(LayoutAdaptationObservation::fit_width(
            execution.container_width,
            ink_width,
        ));
    }
    let layout = C4DiagramLayout {
        bounds: Some(bounds),
        width: ink_width + 2.0 * conf.diagram_margin_x,
        height: ink_height + 2.0 * conf.diagram_margin_y,
        container_width: execution.container_width,
        container_height: execution.container_height,
        screen_available_width: execution.screen_available_width,
        c4_type: model.c4_type.clone(),
        title: model.title.clone(),
        use_max_width: conf.use_max_width,
        boundaries,
        shapes,
        rels: relations,
    };
    Ok((
        layout,
        C4ShoalGeometry {
            placement,
            boundary_title_x,
            relations: geometry,
        },
    ))
}
