//! Keep Swimlane caption backplates clear of foreign painted routes.

use crate::projection::FlowchartEdgeKey;
use kurbo::{BezPath, Line, ParamCurveExtrema, PathSeg, Point, Rect, Vec2};
use merman_model::families::flowchart::FlowEdgeVisibility;
use merman_render_core::LayoutPoint;
use merman_render_core::graph_layout::spatial::{Bvh, BvhEntry};
use merman_render_core::resources::OperationWorkMeter;
use merman_render_core::theme_svg::edge_label_background::{backplate_footprint, literal_fill};
use merman_theme_contract::v2::EdgeLabelBackground;
use std::ops::ControlFlow::{Break, Continue};

use crate::svg::edge_fill::{EdgeInk, segment_enters_surface, spatial_bounds};
use crate::svg::{FlowchartEdgePathCacheEntry, FlowchartRenderCtx};
use merman_render_core::{Error, Result};
use rustc_hash::FxHashMap;

pub(in crate::svg) fn applies(ctx: &FlowchartRenderCtx<'_>) -> bool {
    ctx.swimlane_direction.is_some()
        && ctx.edge_order.len() > 1
        && !ctx.swimlane_edge_label_edges_by_node_id.is_empty()
        && ctx.layout.route_contract.is_canonical_axis()
        && ctx.recursive_clusters.is_empty()
}

#[derive(Clone, Copy)]
enum Obstacle {
    Shaft(FlowchartEdgeKey, PathSeg, f64),
    Label(FlowchartEdgeKey),
    Solid,
}

struct Ink {
    bounds: Rect,
    kind: Obstacle,
}

fn overlaps_ink(
    scene: &Bvh<Ink>,
    rect: Rect,
    owner: FlowchartEdgeKey,
    foreign_shaft_only: bool,
    work: &OperationWorkMeter,
) -> Result<bool> {
    let hit = scene.visit(|entry| -> Result<_> {
        work.charge(1)?;
        let ink = match entry {
            BvhEntry::Node(bounds) => {
                return Ok(Continue(rect.overlaps(Rect::new(
                    bounds.x[0],
                    bounds.y[0],
                    bounds.x[1],
                    bounds.y[1],
                ))));
            }
            BvhEntry::Item(ink) if ink.bounds.overlaps(rect) => ink,
            BvhEntry::Item(_) => return Ok(Continue(true)),
        };
        let overlaps = match ink.kind {
            Obstacle::Shaft(key, segment, outset) => {
                key != owner
                    && segment_enters_surface(
                        segment,
                        rect.inflate(outset, outset).to_rounded_rect(0.0),
                        0,
                        work,
                    )?
            }
            Obstacle::Label(key) => !foreign_shaft_only && key != owner,
            Obstacle::Solid => !foreign_shaft_only,
        };
        Ok(if overlaps { Break(()) } else { Continue(true) })
    })?;
    Ok(hit.is_break())
}

fn collect_seats(line: Line, anchor: Point, label: Rect, seats: &mut Vec<Point>) {
    let delta = line.p1 - line.p0;
    let along = if delta.y == 0.0 && delta.x != 0.0 {
        Vec2::new(delta.x.signum(), 0.0)
    } else if delta.x == 0.0 && delta.y != 0.0 {
        Vec2::new(0.0, delta.y.signum())
    } else {
        return;
    };
    let offset = anchor - line.p0;
    let length = delta.dot(along);
    let at = offset.dot(along);
    if offset.cross(along).abs() > 1e-6 || !(0.0..=length).contains(&at) {
        return;
    }
    let span = if delta.y == 0.0 {
        label.width()
    } else {
        label.height()
    };
    // Keep at least half the label's extent on this uninterrupted run. Sample
    // within one label span of its current position to retain its association and lane.
    let low = span * 0.25;
    let high = length - low;
    if low >= high {
        return;
    }
    for distance in [0.25, 0.5, 0.75, 1.0] {
        for sign in [-1.0, 1.0] {
            let candidate = (at + sign * distance * span).clamp(low, high);
            seats.push(line.p0 + along * candidate);
        }
    }
}

pub(in crate::svg) fn reseat(
    ctx: &FlowchartRenderCtx<'_>,
    cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    positions: &mut FxHashMap<FlowchartEdgeKey, LayoutPoint>,
) -> Result<()> {
    let Some(swimlane) = ctx.swimlane_layout else {
        return Ok(());
    };
    let work = ctx.work_meter;
    let background = match ctx.native {
        Some(theme)
            if theme.form.edge_label_bg == EdgeLabelBackground::Canvas
                && theme.canvas.channels()[3] == 0 =>
        {
            EdgeLabelBackground::Transparent
        }
        Some(theme) => theme.form.edge_label_bg,
        None if !ctx.edge_html_labels
            && literal_fill(&ctx.compatibility.edge_label_background)
                .is_some_and(|color| color.channels()[3] > 0) =>
        {
            EdgeLabelBackground::Canvas
        }
        None => EdgeLabelBackground::Transparent,
    };
    let mut inks = Vec::new();
    let mut labels = Vec::new();
    let mut shafts = FxHashMap::default();
    for edge in &ctx.edge_order {
        work.charge(1)?;
        let key = edge.key;
        if let Some((position, label)) = positions.get(&key).zip(
            ctx.layout_edges_by_key
                .get(&key)
                .and_then(|edge| edge.label()),
        ) && label.width > 0.0
            && label.height > 0.0
        {
            let center = Point::new(position.x, position.y);
            let rect = backplate_footprint(
                Rect::from_center_size(center, (label.width, label.height)),
                background,
            );
            labels.push((key, rect));
            inks.push(Ink {
                bounds: rect,
                kind: Obstacle::Label(key),
            });
        }
        if edge.edge.visibility == FlowEdgeVisibility::Invisible {
            continue;
        }
        let Some(entry) = cache.get(&key) else {
            continue;
        };
        let geom = &entry.geom;
        let d = geom.emitted_d_for_label.as_deref().unwrap_or(&geom.d);
        work.charge(d.len().div_ceil(16))?;
        let shaft = BezPath::from_svg(d).map_err(|error| Error::InvalidModel {
            message: format!("could not read painted Swimlane edge path: {error}"),
        })?;
        let outset = ctx
            .edge_style_plan
            .stroke_outset_for(key)?
            .unwrap_or_default();
        let offset = Vec2::new(entry.origin_x - ctx.tx, entry.origin_y - ctx.ty);
        let to_layout = kurbo::Affine::translate(offset);
        let has_label = positions.contains_key(&key);
        let rough = geom.emitted_d_for_label.is_some();
        let mut register_run = |segment| {
            if let PathSeg::Line(line) = segment {
                shafts.entry(key).or_insert_with(Vec::new).push(line);
            }
        };
        let ink = EdgeInk::new(&shaft, edge.edge, geom, offset, outset);
        inks.extend(ink.marker_bounds().map(|bounds| Ink {
            bounds: bounds.inflate(outset, outset),
            kind: Obstacle::Solid,
        }));
        for segment in shaft.segments() {
            work.charge(1)?;
            let segment = to_layout * segment;
            inks.push(Ink {
                bounds: segment.bounding_box().inflate(outset, outset),
                kind: Obstacle::Shaft(key, segment, outset),
            });
            if has_label && !rough {
                register_run(segment);
            }
        }
        if has_label && rough {
            // RoughJS replaces straight runs with cubic subpaths. Seat labels on the logical
            // centerline, while testing clearance against the painted paths above.
            work.charge(geom.d.len().div_ceil(16))?;
            let logical = BezPath::from_svg(&geom.d).map_err(|error| Error::InvalidModel {
                message: format!("could not read logical Swimlane edge path: {error}"),
            })?;
            for segment in logical.segments() {
                work.charge(1)?;
                register_run(to_layout * segment);
            }
        }
    }
    work.charge(ctx.layout.nodes.len())?;
    inks.extend(ctx.layout.nodes.iter().map(|node| Ink {
        bounds: Rect::from_center_size((node.x, node.y), (node.width, node.height)),
        kind: Obstacle::Solid,
    }));
    work.charge(swimlane.lanes.len())?;
    let lanes: Vec<_> = swimlane
        .lanes
        .iter()
        .filter(|lane| super::is_lane_or_pool_cluster(ctx, &lane.id))
        .map(|lane| {
            let geometry = super::swimlane_terminal_geometry(lane, swimlane.direction, true);
            let (body, title) = geometry.frame_parts((lane.x, lane.y), swimlane.direction, true);
            inks.push(Ink {
                bounds: title,
                kind: Obstacle::Solid,
            });
            body
        })
        .collect();
    // The Flow projection carries geometry instead of the facade's optional layout bounds.
    // Include existing routes and label extents so a seat cannot expand the viewport.
    work.charge(inks.len().saturating_add(ctx.layout.clusters.len()))?;
    let viewport = inks
        .iter()
        .map(|ink| ink.bounds)
        .chain(ctx.layout.clusters.iter().map(|cluster| {
            Rect::from_center_size((cluster.x, cluster.y), (cluster.width, cluster.height))
        }))
        .reduce(|bounds, rect| bounds.union(rect));
    work.charge(
        inks.len()
            .saturating_mul(inks.len().checked_ilog2().unwrap_or(0) as usize + 1),
    )?;
    let scene = Bvh::build(inks, |ink| spatial_bounds(ink.bounds)).map_err(|error| {
        Error::InvalidModel {
            message: format!("could not index painted Swimlane ink: {error}"),
        }
    })?;
    let mut moved: Vec<Rect> = Vec::new();
    let mut seats = Vec::new();
    for (key, original) in labels {
        let Some(runs) = shafts.get(&key) else {
            continue;
        };
        if !overlaps_ink(&scene, original, key, true, work)? {
            continue;
        }
        let anchor = original.center();
        seats.clear();
        work.charge(runs.len())?;
        for &line in runs {
            collect_seats(line, anchor, original, &mut seats);
        }
        work.charge(
            seats
                .len()
                .saturating_mul(seats.len().checked_ilog2().unwrap_or(0) as usize + 1),
        )?;
        seats.sort_by(|a, b| (*a - anchor).hypot2().total_cmp(&(*b - anchor).hypot2()));
        work.charge(lanes.len())?;
        let room = lanes
            .iter()
            .filter(|body| body.contains_rect(original))
            .fold(
                viewport.map_or(original, |bounds| bounds.union(original)),
                |room, body| room.intersect(*body),
            );
        for &at in &seats {
            work.charge(1)?;
            let candidate = original + (at - anchor);
            if !room.contains_rect(candidate) || overlaps_ink(&scene, candidate, key, false, work)?
            {
                continue;
            }
            work.charge(moved.len())?;
            if moved.iter().any(|bounds| bounds.overlaps(candidate)) {
                continue;
            }
            moved.push(candidate);
            positions.insert(key, LayoutPoint { x: at.x, y: at.y });
            break;
        }
    }
    Ok(())
}
