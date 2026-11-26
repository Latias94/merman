use super::{Drawing, canonical_size};
use crate::geom::{Bounds, Bvh, BvhEntry};
use crate::work::{LayoutResult, Work, push};
use crate::{Graph, Point};
use std::ops::ControlFlow::{Break, Continue};

fn separation(a: Bounds, b: Bounds) -> f64 {
    let gap = |[a0, a1]: [f64; 2], [b0, b1]: [f64; 2]| (a0 - b1).max(b0 - a1);
    // The painter inflates the label box by the stroke outset on both axes.
    gap(a.x, b.x).max(gap(a.y, b.y)).max(0.0) * 0.99
}

/// Restrict the node-only certificates to the final obstacle scene. A label's assigned
/// rank says nothing about its final box after a fallback or a terminal label slide.
pub(in crate::flat) fn certify_final_clearance(
    drawing: &mut Drawing,
    graph: &Graph,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.visit(drawing.edges.len())?;
    if !drawing
        .edges
        .iter()
        .any(|(_, route)| route.corridor_clearance > 0.0)
    {
        return Ok(());
    }
    let mut labels = Vec::new();
    for (id, route) in &drawing.edges {
        work.visit(1)?;
        let spec = &graph.edges[*id as usize];
        for (center, size) in [
            (route.label_center, spec.label),
            (route.start_label_center, spec.start_label),
            (route.end_label_center, spec.end_label),
        ] {
            match (center, size) {
                (None, None) => {}
                (Some(at), Some(size)) => {
                    let bounds = Bounds::centered(at, canonical_size(graph.direction, size));
                    if !bounds.finite() {
                        return discard(drawing, work);
                    }
                    push(&mut labels, bounds)?;
                }
                _ => return discard(drawing, work),
            }
        }
    }
    if labels.is_empty() {
        return Ok(());
    }
    // Only final label boxes need indexing. A single box can qualify or veto a route directly.
    let tree = if labels.len() > 1 {
        Some(Bvh::from_bounds(std::mem::take(&mut labels), work)?)
    } else {
        None
    };
    for (_, route) in &mut drawing.edges {
        work.visit(1)?;
        let margin = &mut route.corridor_clearance;
        if *margin <= 0.0 {
            continue;
        }
        let [first, .., last] = route.points.as_slice() else {
            *margin = 0.0;
            continue;
        };
        // The certified single-gap shaft and cubic are coordinate-wise monotone, including
        // the normal marker leads. A later change to an endpoint or route drops its proof.
        let point = |p: &crate::RoutePoint| Point { x: p.x, y: p.y };
        let hull = Bounds::between(point(first), point(last));
        if let Some(tree) = &tree {
            let _ = tree.visit(|entry| -> LayoutResult<_> {
                work.visit(1)?;
                Ok(match entry {
                    BvhEntry::Node(envelope) => Continue(separation(hull, envelope) < *margin),
                    BvhEntry::Item(label) => {
                        *margin = margin.min(separation(hull, *label));
                        if *margin == 0.0 {
                            Break(())
                        } else {
                            Continue(true)
                        }
                    }
                })
            })?;
        } else {
            *margin = margin.min(separation(hull, labels[0]));
        }
    }
    Ok(())
}

fn discard(drawing: &mut Drawing, work: &mut Work<'_>) -> LayoutResult<()> {
    work.visit(drawing.edges.len())?;
    for (_, route) in &mut drawing.edges {
        route.corridor_clearance = 0.0;
    }
    Ok(())
}
