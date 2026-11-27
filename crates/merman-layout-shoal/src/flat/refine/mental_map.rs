//! An optional, bounded cross-axis projection onto the current ordered ranks.

use super::{
    BOTTOM, Component, Direction, Geometry, Interval, LayoutError, LayoutResult, Options,
    PassScratch, RowScratch, TOP, Work, ZeroGap, capacity_bytes, clear_reserve, copy_into,
    incident, settle, vertex, weighted_pava,
};

const SWEEPS: usize = 4;
const UNCHANGED_WEIGHT: f64 = 32.;
const EDITED_WEIGHT: f64 = 2.;
const CURRENT_WEIGHT: f64 = 1.;

#[derive(Clone, Copy, Debug)]
struct Anchor {
    cross: f64,
    weight: f64,
}

#[derive(Debug, Default)]
pub(super) struct ProjectionScratch {
    x: Vec<f64>,
    dx: [Vec<f64>; 2],
    anchors: Vec<Option<Anchor>>,
    weights: Vec<f64>,
}

impl ProjectionScratch {
    pub(super) fn retained_bytes(&self) -> usize {
        capacity_bytes(&self.x)
            + self.dx.iter().map(capacity_bytes).sum::<usize>()
            + capacity_bytes(&self.anchors)
            + capacity_bytes(&self.weights)
    }
}

pub(super) fn stabilize(
    s: &mut Geometry<'_>,
    c: &Component,
    direction: Direction,
    options: &Options,
    passes: &mut PassScratch,
    trial: &mut ProjectionScratch,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let accepted = work.compare(|work| {
        match project_and_settle(s, c, direction, options, passes, trial, work) {
            Err(LayoutError::AllocationLimit) => Ok(false),
            result => result,
        }
    })?;
    if accepted == Some(true) {
        std::mem::swap(&mut s.data.x, &mut trial.x);
        std::mem::swap(&mut s.data.dx, &mut trial.dx);
        work.editor_choice = true;
    }
    Ok(())
}

fn project_and_settle(
    s: &mut Geometry<'_>,
    c: &Component,
    direction: Direction,
    options: &Options,
    passes: &mut PassScratch,
    trial: &mut ProjectionScratch,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    if !project(s, c, direction, &mut passes.row, trial, work)? {
        return Ok(false);
    }
    work.charge(s.data.dx[TOP].len())?;
    work.charge(s.data.dx[BOTTOM].len())?;
    copy_into(&mut trial.dx[TOP], &s.data.dx[TOP])?;
    copy_into(&mut trial.dx[BOTTOM], &s.data.dx[BOTTOM])?;
    // Keep the original geometry until the optional meter accepts the settled trial.
    std::mem::swap(&mut s.data.x, &mut trial.x);
    std::mem::swap(&mut s.data.dx, &mut trial.dx);
    let settled = settle(s, c.nodes.len(), options, passes, work);
    std::mem::swap(&mut s.data.x, &mut trial.x);
    std::mem::swap(&mut s.data.dx, &mut trial.dx);
    settled?;
    Ok(trial.dx.iter().flatten().all(|x| x.is_finite()))
}

fn locked(s: &Geometry<'_>, c: &Component, v: usize) -> bool {
    s.data.protected[v]
        || c.nodes
            .get(v)
            .is_some_and(|n| n.pin != 0 || n.boundary.is_some() || n.group_proxy)
}

fn origin(s: &Geometry<'_>, c: &Component, x: &[f64], work: &mut Work<'_>) -> LayoutResult<f64> {
    work.visit(c.nodes.len())?;
    Ok(c.nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.boundary.is_none() && !n.group_proxy)
        .map(|(i, _)| x[i] - s.g.width[i] * 0.5)
        .fold(f64::INFINITY, f64::min))
}

fn anchor_error(anchors: &[Option<Anchor>], x: &[f64], origin: f64) -> f64 {
    anchors
        .iter()
        .zip(x)
        .filter_map(|(anchor, &x)| anchor.map(|a| a.weight * ((x - origin) - a.cross).powi(2)))
        .sum()
}

fn project(
    s: &Geometry<'_>,
    c: &Component,
    direction: Direction,
    row: &mut RowScratch,
    ProjectionScratch {
        x,
        anchors,
        weights,
        ..
    }: &mut ProjectionScratch,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    work.charge(c.nodes.len())?;
    clear_reserve(anchors, c.nodes.len())?;
    for (v, node) in c.nodes.iter().enumerate() {
        let anchor = node.position_hint_in(direction, s.g.rank[v]);
        anchors.push(anchor.map(|(cross, unchanged)| Anchor {
            cross,
            weight: if unchanged {
                UNCHANGED_WEIGHT
            } else {
                EDITED_WEIGHT
            },
        }));
    }
    let start = origin(s, c, &s.data.x, work)?;
    work.visit(anchors.len())?;
    let before = anchor_error(anchors, &s.data.x, start);
    if !before.is_finite() || before <= 1e-12 {
        return Ok(false);
    }

    work.charge(s.data.x.len())?;
    copy_into(x, &s.data.x)?;
    let widest = s.g.layers.iter().map(Vec::len).max().unwrap_or(0);
    clear_reserve(&mut row.values, widest)?;
    clear_reserve(weights, widest)?;
    clear_reserve(&mut row.gaps, widest)?;
    clear_reserve(&mut row.pools, widest)?;
    for sweep in 0..SWEEPS {
        let origin = origin(s, c, x, work)?;
        // Alternating rank order lets each update use fresh coordinates on one side.
        let layers = &s.g.layers;
        let ascending = sweep % 2 == 0;
        for index in 0..layers.len() {
            let vertices = &layers[if ascending {
                index
            } else {
                layers.len() - 1 - index
            }];
            work.visit(vertices.len())?;
            for run in vertices
                .chunk_by(|&a, &b| s.same_lane(a as usize, b as usize))
                .flat_map(|lane| lane.split(|&v| locked(s, c, v as usize)))
                .filter(|run| !run.is_empty())
            {
                if !fit_row(s, (run, anchors), origin, x, (row, weights), work)? {
                    return Ok(false);
                }
            }
        }
    }

    let end = origin(s, c, x, work)?;
    work.visit(anchors.len())?;
    let after = anchor_error(anchors, x, end);
    // Smoothing should earn its keep in the caller's component-relative coordinates.
    Ok(after < before)
}

fn fit_row(
    s: &Geometry<'_>,
    (run, anchors): (&[u32], &[Option<Anchor>]),
    origin: f64,
    x: &mut [f64],
    (
        RowScratch {
            values,
            gaps,
            pools,
        },
        weights,
    ): (&mut RowScratch, &mut Vec<f64>),
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    values.clear();
    weights.clear();
    gaps.clear();
    gaps.push(0.);
    gaps.extend(
        run.windows(2)
            .map(|pair| s.data.extent[pair[0] as usize].1 + s.data.extent[pair[1] as usize].0),
    );
    for &v in run {
        let v = v as usize;
        let (mut mean, mut weight) = match anchors.get(v).copied().flatten() {
            Some(anchor) => (origin + anchor.cross, anchor.weight),
            None => (s.data.x[v], CURRENT_WEIGHT),
        };
        for side in [TOP, BOTTOM] {
            let segments = incident(s.g, v, side);
            work.visit(segments.len())?;
            let other = 1 - side;
            for &segment in segments {
                let e = segment as usize;
                let peer = vertex(s.g, e, other);
                let target = x[peer] + s.data.dx[other][e] - s.data.dx[side][e];
                let spring = f64::from(s.g.weight[e]).clamp(0.25, 4.);
                weight += spring;
                mean += (target - mean) * (spring / weight);
            }
        }
        if !mean.is_finite() {
            return Ok(false);
        }
        values.push(mean);
        weights.push(weight);
    }
    // The exterior neighbor of each end of a run is fixed for the whole projection.
    let bounds = Interval {
        lo: s.room(run[0] as usize, |_| false).lo,
        hi: s.room(run[run.len() - 1] as usize, |_| false).hi,
    };
    if weighted_pava(values, gaps, bounds, ZeroGap::Ordered, pools, |i| {
        weights[i]
    }) {
        for (&v, &target) in run.iter().zip(values.iter()) {
            let half = s.g.width[v as usize] * 0.5;
            if !target.is_finite()
                || half > 0. && !(target - half < target && target < target + half)
            {
                return Ok(false);
            }
            x[v as usize] = target;
        }
    }
    Ok(true)
}
