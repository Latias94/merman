mod labels;

use super::{
    C4_ROUNDED_RADIUS, C4ClusterThemePlan, C4ConfigView, C4Measurement, C4TypographyThemePlan,
    measure_c4_relation, measure_c4_shape, prepare_c4_boundary_layout, title_ink,
};
use crate::graph_layout::Marker;
use crate::graph_layout::canonical::{
    CanonicalPaintScratch, CanonicalPaintedEdge, CanonicalRoutes, CaptionClearance,
};
use crate::graph_layout::contour::{c4_shape, shoal_contour};
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Rect, Side,
    Size, TitlePosition,
};
use crate::graph_layout::placement;
use crate::graph_layout::route_paint::RouteStyle;
use crate::layout_frame::GraphPlacement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, C4DiagramLayout};
use crate::{Error, LayoutAdaptation, LayoutAdaptationObservation, LayoutExecution, Result};
use kurbo::{Insets, Shape, Vec2};
use merman_layout_shoal::{WidthPolicy, card_grid};
use merman_model::families::c4::C4DiagramModel;
use merman_render_core::graph_layout::shoal::{Placement, project_input, project_output};
use merman_render_core::rough_svg::{constrained_rough_edge_with_geometry, operation_randomness};
use merman_render_graph::c4::{C4RoutedGeometry, C4RoutedRelation};
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
    routed: C4RoutedGeometry,
}

struct PaintedRelation {
    shaft: CanonicalPaintedEdge,
    label: Option<labels::PlacedRelationLabel>,
    seated: bool,
    shaft_cutout: Option<kurbo::Rect>,
}

impl C4ShoalGeometry {
    pub(crate) const fn placement(&self) -> &GraphPlacement {
        &self.placement
    }

    pub(crate) const fn routed(&self) -> &C4RoutedGeometry {
        &self.routed
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

fn marker_ink(edge: &CanonicalPaintedEdge) -> [Option<Bounds>; 2] {
    let ends = &edge.marker_points;
    [
        (edge.data_points.first(), &ends[1], edge.markers.0),
        (edge.data_points.last(), &ends[2], edge.markers.1),
    ]
    .map(|(tip, lead, marker)| {
        if let Some(tip) = tip
            && marker.width > 0.0
        {
            let toward = Vec2::new(lead.x - tip.x, lead.y - tip.y);
            let length = toward.hypot();
            let normal = if length > 0.0 {
                toward / length
            } else {
                Vec2::ZERO
            };
            let wing = Vec2::new(-normal.y, normal.x) * (marker.width / 2.0);
            let tail = normal * (marker.trim + marker.depth);
            let min_x = tip.x + tail.x.min(0.0) - wing.x.abs();
            let max_x = tip.x + tail.x.max(0.0) + wing.x.abs();
            let min_y = tip.y + tail.y.min(0.0) - wing.y.abs();
            let max_y = tip.y + tail.y.max(0.0) + wing.y.abs();
            Some(Bounds {
                min_x,
                min_y,
                max_x,
                max_y,
            })
        } else {
            None
        }
    })
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
    let stroke_outset = native.map_or(0.5, |theme| theme.routed_shaft_outline_width() / 2.0);
    let route_style = native.map_or(RouteStyle::Step, |theme| {
        RouteStyle::from_theme(theme.edge_style)
    });
    let sketch = (native.is_some_and(|theme| theme.edge_style == EdgeStyle::Sketch)
        || conf.look.is_hand_drawn())
    .then(|| {
        operation_randomness(
            execution.family.session().operation_context(),
            effective_config
                .get("handDrawnSeed")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            "render.c4.shafts",
        )
    });
    let mut graph = MeasuredGraph {
        direction: Direction::Down,
        routing: route_style.routing_style(),
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
        let (profile, size, dx) = c4_shape(
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
            contour: shoal_contour(&profile, size),
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
            title_ink(&layout, &measure)?
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
                title_position: TitlePosition::TopAuto,
            });
        }
        title_centers.push(title.map(|ink| ink.center().y));
        boundaries.push(layout);
    }
    let mut relations = Vec::with_capacity(relation_count);
    let mut labels = Vec::with_capacity(relation_count);
    let mut ids = Vec::with_capacity(relation_count);
    let fit_width = execution.adaptation == LayoutAdaptation::FitWidth;
    let paint_style = match route_style {
        // Bound the bend within the card grid's right-angle channels.
        RouteStyle::Curved => RouteStyle::Rounded(5.0),
        style => style,
    };
    // Reserve the maximum inward movement of a rounded bend and the sketch pen's wobble.
    let shaft_outset = stroke_outset
        + match paint_style {
            RouteStyle::Rounded(radius) => radius,
            _ => 0.0,
        }
        + if sketch.is_some() { 1.0 } else { 0.0 };
    let measure_ink_later = merman_render_core::text::measurement::measurement_reuse_key(
        measure.measurer,
        crate::environment::TextMeasurementOperation::MermaidCalculateTextDimensions,
    )
    .is_some();
    for (ordinal, rel) in model.rels.iter().enumerate() {
        let mut layout =
            measure_c4_relation(rel, ordinal, &model.c4_type, &measure, measure_ink_later)?;
        let label = labels::RelationLabel::prepare(&mut layout, &measure)?;
        let label_size = label.as_ref().map(|label| label.size(shaft_outset));
        let source = endpoint(&rel.from_alias, "from")?;
        let target = endpoint(&rel.to_alias, "to")?;
        let (start, end) = super::c4_relation_markers(&rel.rel_type);
        graph.edges.push(MeasuredEdge {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label: label_size,
            start_label: None,
            end_label: None,
            start_marker: if start { MARKER } else { Marker::default() },
            end_marker: if end { MARKER } else { Marker::default() },
        });
        relations.push(layout);
        labels.push(label);
        ids.push(format!("c4-rel-{ordinal}"));
    }
    let node_ids = || model.shapes.iter().map(|shape| shape.alias.as_str());
    let hints = placement::previous_order_hints(
        &graph,
        node_ids(),
        group_ids.iter().copied(),
        execution,
        &mut work,
    )?;
    let input = Placement {
        width: if fit_width {
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
        hints: &hints,
        ..Default::default()
    };
    let options = input.options_for(&graph);
    let measured = project_input(&graph, &input, &mut work)?;
    let geometry = crate::layout_work::measure_layout_solve!(card_grid::layout_with_control(
        &measured, &options, &mut work,
    ))
    .map_err(|error| work.map_shoal_error(error))?;
    // Captions without a grid seat remain in `labels` for the painted-shaft search.
    work.charge_adapter(graph.edges.len())?;
    for ((edge, original), route) in graph
        .edges
        .iter_mut()
        .zip(&measured.edges)
        .zip(&geometry.edges)
    {
        edge.label = route.label_center.and(original.label);
    }
    let mut solved = project_output(&graph, geometry, &mut work)?;
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
        CaptionClearance::Cutout,
        &mut work,
    )?;

    let mut bounds = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: solved.bounds.width,
        max_y: solved.bounds.height,
    };
    let mut painted_routes = Vec::with_capacity(relation_count);
    let mut shafts = Vec::new();
    let check_labels = labels.iter().any(Option::is_some);
    let check_titles = solved.groups.iter().any(|group| group.title.is_some());
    let record_shafts = check_labels || check_titles;
    if record_shafts {
        work.charge_adapter(relation_count)?;
        shafts
            .try_reserve_exact(relation_count)
            .map_err(|_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit))?;
    }
    let mut paint_scratch = CanonicalPaintScratch::default();
    for ((id, edge), label) in ids.iter().zip(&solved.edges).zip(&labels) {
        work.charge_adapter(edge.points.len())?;
        if record_shafts && sketch.is_none() {
            paint_scratch.route.record_segments();
        }
        let mut painted = routes.paint_points_with_scratch(
            id,
            edge.points.iter().map(|p| (p.x, p.y)),
            Vec2::ZERO,
            stroke_outset,
            paint_style,
            execution.work_meter_ref(),
            &mut paint_scratch,
        )?;
        let rough = if let Some(randomness) = &sketch {
            let (d, geometry) = constrained_rough_edge_with_geometry(
                &painted.path.d,
                painted.markers,
                execution.work_meter_ref(),
                randomness,
            )?;
            let ink = geometry.bounding_box();
            painted.path.d = d;
            painted.path.bounds = Bounds {
                min_x: ink.x0,
                min_y: ink.y0,
                max_x: ink.x1,
                max_y: ink.y1,
            };
            Some(geometry)
        } else {
            None
        };
        let markers = marker_ink(&painted);
        if record_shafts {
            shafts.push(labels::PaintedShaft::from_segments(
                &painted,
                paint_scratch.route.segments(),
                rough.as_ref(),
                &markers,
                &mut work,
            )?);
        }
        let [start, .., end] = painted.data_points.as_slice() else {
            return Err(invalid("c4: missing canonical relation contacts"));
        };
        include_ink(&mut bounds, &painted.path.bounds, stroke_outset);
        for marker in markers.into_iter().flatten() {
            include_ink(&mut bounds, &marker, 0.0);
        }
        let seated = label.is_none() || edge.label.is_some();
        let placed_label = label.as_ref().map(|prep| {
            let center = edge.label.as_ref().map_or_else(
                || kurbo::Point::new(start.x.midpoint(end.x), start.y.midpoint(end.y)),
                |label| kurbo::Point::new(label.x, label.y),
            );
            prep.place(center)
        });
        painted_routes.push(PaintedRelation {
            shaft: painted,
            label: placed_label,
            seated,
            shaft_cutout: None,
        });
    }
    if record_shafts {
        let shafts = labels::ShaftIndex::new(&shafts, stroke_outset, &mut work)?;
        // TopAuto may choose a side even when its top face is uncrowded. Restore the
        // center when the *painted* route leaves room for the full title and marker wings.
        work.charge_adapter(solved.groups.len())?;
        for group in &mut solved.groups {
            let Some(title) = &mut group.title else {
                continue;
            };
            let center = group.rect.center().x;
            if title.x != center {
                let ink =
                    kurbo::Rect::from_center_size((center, title.y), (title.width, title.height));
                if !shafts.intersects(ink, &mut work)? {
                    title.x = center;
                }
            }
        }
        if check_labels {
            let obstacle_count = work.checked_add(solved.nodes.len(), solved.groups.len())?;
            work.charge_adapter(obstacle_count)?;
            let mut obstacles = Vec::new();
            obstacles.try_reserve_exact(obstacle_count).map_err(|_| {
                work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit)
            })?;
            let card_outset = execution
                .family
                .native_theme()
                .map_or(1.0, |theme| theme.form.stroke_width / 2.0);
            obstacles.extend(solved.nodes.iter().map(|node| {
                let ink =
                    kurbo::Rect::new(node.x, node.y, node.x + node.width, node.y + node.height)
                        .inflate(card_outset, card_outset);
                bounds.max_y = bounds.max_y.max(ink.y1);
                ink
            }));
            obstacles.extend(
                solved
                    .groups
                    .iter()
                    .filter_map(|group| group.title.as_ref())
                    .map(|title| {
                        kurbo::Rect::from_center_size(
                            (title.x, title.y),
                            (title.width, title.height),
                        )
                    }),
            );
            if labels::reseat(&mut painted_routes, &shafts, obstacles, &mut work)? {
                labels::place_exterior(
                    &mut painted_routes,
                    &relations,
                    &bounds,
                    &measure,
                    &mut work,
                )?;
            }
        }
    }
    let mut placement = placement::placement_for_solved(
        &graph,
        &solved,
        node_ids(),
        group_ids.iter().copied(),
        execution,
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

    let mut geometry = Vec::with_capacity(relation_count);
    work.charge_adapter(relation_count)?;
    for ((rel, id), route) in relations.iter_mut().zip(ids).zip(painted_routes) {
        let [start, .., end] = route.shaft.data_points.as_slice() else {
            return Err(invalid("c4: missing canonical relation contacts"));
        };
        rel.start_point = start.clone();
        rel.end_point = end.clone();
        if let Some(label) = &route.label {
            let ink = label.ink;
            bounds.min_x = bounds.min_x.min(ink.x0);
            bounds.min_y = bounds.min_y.min(ink.y0);
            bounds.max_x = bounds.max_x.max(ink.x1);
            bounds.max_y = bounds.max_y.max(ink.y1);
        }
        let (label_origin, label_reference) = route
            .label
            .map_or((None, None), |label| (Some(label.origin), label.reference));
        geometry.push(C4RoutedRelation {
            id,
            path: route.shaft.path.d,
            points: route.shaft.data_points,
            label_origin,
            label_reference,
            shaft_cutout: route.shaft_cutout,
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
    let layout = C4DiagramLayout::from_routed(
        model,
        &conf,
        bounds,
        crate::geom::Size::new(ink_width, ink_height),
        crate::geom::Size::new(execution.container_width, execution.container_height),
        execution.screen_available_width,
        boundaries,
        shapes,
        relations,
    );
    Ok((
        layout,
        C4ShoalGeometry {
            placement,
            routed: C4RoutedGeometry {
                boundary_title_x,
                relations: geometry,
            },
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::environment::RenderEnvironment;
    use crate::family::FamilyExecutionView;
    use crate::layout_backend::GraphLayoutBackend;
    use crate::model::LayoutPoint;
    use kurbo::{BezPath, ParamCurveNearest};
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::presentation::RenderSemanticModel;
    use merman_render_graph::c4::measure_c4_relation_label;

    #[test]
    fn exterior_c4_marker_wings_expand_the_layout_beyond_the_shaft() {
        let xy = |x, y| LayoutPoint { x, y };
        let mut bounds = Bounds::from_points([(10.0, 0.0), (90.0, 0.0)]).unwrap();
        let edge = CanonicalPaintedEdge {
            path: crate::graph_layout::route_paint::PaintedRoute {
                d: "M11 0 L89 0".into(),
                bounds: bounds.clone(),
            },
            marker_points: [xy(11.0, 0.0), xy(21.0, 0.0), xy(79.0, 0.0), xy(89.0, 0.0)],
            data_points: vec![xy(10.0, 0.0), xy(90.0, 0.0)],
            markers: (MARKER, MARKER),
        };
        for marker in marker_ink(&edge).into_iter().flatten() {
            include_ink(&mut bounds, &marker, 0.0);
        }
        assert_eq!(bounds.min_y, -MARKER.width / 2.0);
        assert_eq!(bounds.max_y, MARKER.width / 2.0);
    }

    #[test]
    fn c4_shoal_crossing_caption_avoids_foreign_shafts_and_keeps_its_owner() {
        let source = r#"---
config:
  layout: shoal
  htmlLabels: false
---
C4Context
  title Overlapping Multi-Cutout Shaft Stress
  UpdateLayoutConfig($c4ShapeInRow="3")
  System(a, "Left Top", "Source")
  System(b, "Mid Top", "Relay")
  System(c, "Right Top", "Target")
  System(d, "Left Bot", "Peer 1")
  System(e, "Mid Bot", "Peer 2")
  System(f, "Right Bot", "Peer 3")
  Rel(a, c, "Long top span edge across center", "HTTPS")
  Rel(a, f, "Long cross diagonal alpha alpha alpha", "gRPC")
  Rel(c, d, "Long cross diagonal beta beta beta", "gRPC")
  Rel(b, e, "Center vertical edge with wide caption text", "SQL")
  Rel(a, e, "Offset crossover one with wide caption", "REST")
  Rel(c, e, "Offset crossover two with wide caption", "REST")
  Rel(d, f, "Bottom span edge across center", "Kafka")
  UpdateRelStyle(a, f, $offsetX="-10", $offsetY="-5")
  UpdateRelStyle(c, d, $offsetX="10", $offsetY="5")
"#;
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::C4(model) = parsed.model() else {
            panic!("expected C4 presentation");
        };
        let config = &parsed.metadata().effective_config;
        let theme = crate::theme::ThemeCompiler::new()
            .compile_preset(
                crate::theme::BuiltinPreset::Studio,
                Some(crate::theme::Appearance::Dark),
            )
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(crate::resources::RenderResourcePolicy::constrained())
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap();
        let options = crate::LayoutOptions::headless_svg_defaults();
        let family =
            FamilyExecutionView::for_test(&session, DiagramFamilyId::C4, GraphLayoutBackend::Shoal);
        let execution = LayoutExecution::new(&options, family, Some(config));
        let typography = C4TypographyThemePlan::resolve(family.native_theme(), config, model);
        let cluster_theme =
            C4ClusterThemePlan::resolve(family.native_theme(), &model.boundaries, config, true);
        let (mut layout, geometry) = layout_c4_shoal(
            model,
            config.as_value(),
            &typography,
            &cluster_theme,
            &execution,
        )
        .unwrap();
        let routes = &geometry.routed().relations;
        let settings = C4ConfigView::new(config.as_value()).layout_settings();
        let measure = C4Measurement {
            typography: &typography,
            conf: &settings,
            measurer: execution.text_measurer(),
            work: execution.work_meter_ref(),
        };
        let stroke_outset = typography
            .native_style()
            .unwrap()
            .routed_shaft_outline_width()
            / 2.0;
        let paths: Vec<_> = routes
            .iter()
            .map(|route| BezPath::from_svg(&route.path).unwrap())
            .collect();
        for (owner, (route, relation)) in routes.iter().zip(&mut layout.rels).enumerate() {
            let origin = route.label_origin.as_ref().unwrap();
            let (ink, _) = measure_c4_relation_label(relation, &measure)
                .unwrap()
                .unwrap();
            let ink = typography.routed_caption_footprint(ink) + Vec2::new(origin.x, origin.y);
            let guard = ink.inflate(stroke_outset, stroke_outset);
            for card in &layout.shapes {
                let card_ink =
                    kurbo::Rect::from_origin_size((card.x, card.y), (card.width, card.height));
                assert_eq!(
                    ink.intersect(card_ink).area(),
                    0.0,
                    "{} overlaps {}",
                    route.id,
                    card.alias
                );
            }
            let center = ink.center();
            let distance = |path: &BezPath| {
                path.segments()
                    .map(|segment| segment.nearest(center, 1e-3).distance_sq)
                    .fold(f64::INFINITY, f64::min)
            };
            for (other, shaft) in paths
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != owner)
            {
                assert!(
                    !shaft.segments().any(|segment| {
                        use kurbo::ParamCurve;
                        guard.contains(segment.start())
                            || guard.contains(segment.end())
                            || guard.path_segments(0.0).any(|side| {
                                let kurbo::PathSeg::Line(side) = side else {
                                    unreachable!()
                                };
                                !segment.intersect_line(side).is_empty()
                            })
                    }),
                    "caption {} is crossed by {}",
                    route.id,
                    routes[other].id,
                );
                assert!(
                    distance(&paths[owner]) < distance(shaft),
                    "caption {} is nearer foreign shaft {}",
                    route.id,
                    routes[other].id
                );
            }
        }
    }

    fn assert_simple_orthogonal_shaft(id: &str, path: &BezPath) {
        let legs: Vec<_> = path
            .segments()
            .filter_map(|segment| {
                let kurbo::PathSeg::Line(line) = segment else {
                    panic!("{id} has a non-linear shaft");
                };
                assert!(line.p0.x == line.p1.x || line.p0.y == line.p1.y, "{id}");
                (line.p0 != line.p1).then(|| line.bounding_box())
            })
            .collect();
        for (i, leg) in legs.iter().enumerate() {
            for (j, previous) in legs[..i].iter().enumerate() {
                let dx = leg.x1.min(previous.x1) - leg.x0.max(previous.x0);
                let dy = leg.y1.min(previous.y1) - leg.y0.max(previous.y0);
                let overlaps = dx >= 0.0 && dy >= 0.0 && (dx > 0.0 || dy > 0.0);
                let touches = dx >= 0.0 && dy >= 0.0;
                assert!(
                    !overlaps && (i == j + 1 || !touches),
                    "{id} intersects itself at shaft legs {j} and {i}"
                );
            }
        }
    }

    #[test]
    fn c4_shoal_reserves_caption_corridors_in_dense_boundaries() {
        use std::fmt::Write;

        for (boundaries, cards) in [(5, 6), (8, 8)] {
            let mut source = String::from(
                "---\nconfig:\n  layout: shoal\n---\nC4Container\n\
                 Person(user1, \"Customer\", \"Web user\")\n\
                 Person(admin1, \"Operator\", \"Ops admin\")\n",
            );
            for b in 0..boundaries {
                writeln!(source, "Boundary(b{b}, \"Domain {b}\") {{").unwrap();
                for k in 0..cards {
                    writeln!(source, "  Container(c_{b}_{k}, \"Service {b}.{k}\", \"Rust\", \"Handles flow {b}.{k}\")").unwrap();
                }
                source.push_str("}\n");
            }
            source.push_str(
                "Rel(user1, c_0_0, \"Uses\", \"HTTPS\")\n\
                 Rel(admin1, c_0_1, \"Monitors\", \"gRPC\")\n",
            );
            for b in 0..boundaries {
                for k in 0..cards {
                    if k + 1 < cards {
                        writeln!(
                            source,
                            "Rel(c_{b}_{k}, c_{b}_{}, \"Calls\", \"gRPC\")",
                            k + 1
                        )
                        .unwrap();
                    }
                    if b + 1 < boundaries && k % 2 == 0 {
                        writeln!(
                            source,
                            "Rel(c_{b}_{k}, c_{}_{k}, \"Publishes\", \"Kafka\")",
                            b + 1
                        )
                        .unwrap();
                    }
                }
            }
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let RenderSemanticModel::C4(model) = parsed.model() else {
                panic!("expected C4 presentation");
            };
            let config = &parsed.metadata().effective_config;
            let session = RenderEnvironment::deterministic()
                .with_resource_policy(crate::resources::RenderResourcePolicy::constrained())
                .begin_session()
                .unwrap();
            let options = crate::LayoutOptions::headless_svg_defaults();
            let family = FamilyExecutionView::for_test(
                &session,
                DiagramFamilyId::C4,
                GraphLayoutBackend::Shoal,
            );
            let execution = LayoutExecution::new(&options, family, Some(config));
            let typography = C4TypographyThemePlan::resolve(None, config, model);
            let cluster_theme = C4ClusterThemePlan::resolve(None, &model.boundaries, config, true);
            let (mut layout, geometry) = layout_c4_shoal(
                model,
                config.as_value(),
                &typography,
                &cluster_theme,
                &execution,
            )
            .unwrap();
            if (boundaries, cards) == (8, 8) {
                let units = execution.work_meter_ref().used();
                assert!(
                    units <= 122_000,
                    "64-card C4 used {units} layout work units"
                );
            }
            let routes = &geometry.routed().relations;
            assert_eq!(
                routes.len(),
                2 + boundaries * (cards - 1) + (boundaries - 1) * (cards / 2)
            );
            let paths: Vec<_> = routes
                .iter()
                .map(|route| {
                    let path = BezPath::from_svg(&route.path).unwrap();
                    assert_simple_orthogonal_shaft(&route.id, &path);
                    path
                })
                .collect();
            let settings = C4ConfigView::new(config.as_value()).layout_settings();
            // Verify caption ink with a fresh meter after the constrained layout has completed.
            let verification_work = crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::constrained(),
            );
            let measure = C4Measurement {
                typography: &typography,
                conf: &settings,
                measurer: execution.text_measurer(),
                work: &verification_work,
            };
            for (idx, (route, relation)) in routes.iter().zip(&mut layout.rels).enumerate() {
                assert!(route.label_reference.is_none(), "{} is exterior", route.id);
                let origin = route.label_origin.as_ref().unwrap();
                let (ink, _) = measure_c4_relation_label(relation, &measure)
                    .unwrap()
                    .unwrap();
                let center = ink.center() + Vec2::new(origin.x, origin.y);
                let clearance = merman_render_graph::c4::SHAFT_CLEARANCE;
                for displaced in [
                    center,
                    center + (0.0, clearance),
                    center - (0.0, clearance),
                    center + (clearance, 0.0),
                    center - (clearance, 0.0),
                ] {
                    let distance = |path: &BezPath| {
                        path.segments()
                            .map(|segment| segment.nearest(displaced, 1e-3).distance_sq)
                            .fold(f64::INFINITY, f64::min)
                    };
                    let own = distance(&paths[idx]);
                    assert!(own.is_finite());
                    for (other, shaft) in
                        paths.iter().enumerate().filter(|(other, _)| *other != idx)
                    {
                        let foreign = distance(shaft);
                        assert!(
                            own < foreign,
                            "{boundaries}×{cards} caption {} at {displaced:?} is {:.2}px from its shaft and {:.2}px from shaft {other}",
                            route.id,
                            own.sqrt(),
                            foreign.sqrt(),
                        );
                    }
                }
            }
        }
    }
}
