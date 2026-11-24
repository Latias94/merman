//! An optional, bounded cross-axis projection onto the current ordered ranks.

use super::{
    BOTTOM, Component, Direction, Geometry, Interval, LayoutError, LayoutResult, Options, Pool,
    TOP, Work, ZeroGap, copy, incident, reserved, settle, vertex, weighted_pava,
};

const SWEEPS: usize = 4;
const UNCHANGED_WEIGHT: f64 = 32.;
const EDITED_WEIGHT: f64 = 2.;
const CURRENT_WEIGHT: f64 = 1.;

#[derive(Clone, Copy)]
struct Anchor {
    cross: f64,
    weight: f64,
}

struct Projection {
    x: Vec<f64>,
    dx: [Vec<f64>; 2],
}

pub(super) fn stabilize(
    s: &mut Geometry<'_>,
    c: &Component,
    direction: Direction,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let trial = work.compare(
        |work| match project_and_settle(s, c, direction, options, work) {
            Err(LayoutError::AllocationLimit) => Ok(None),
            result => result,
        },
    )?;
    if let Some(Some(Projection { x, dx })) = trial {
        s.x = x;
        s.dx = dx;
        work.editor_choice = true;
    }
    Ok(())
}

fn project_and_settle(
    s: &mut Geometry<'_>,
    c: &Component,
    direction: Direction,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Projection>> {
    let Some(x) = project(s, c, direction, work)? else {
        return Ok(None);
    };
    work.charge(s.dx[TOP].len())?;
    work.charge(s.dx[BOTTOM].len())?;
    let mut trial = Projection {
        x,
        dx: [copy(&s.dx[TOP])?, copy(&s.dx[BOTTOM])?],
    };
    // Keep the original geometry until the optional meter accepts the settled trial.
    std::mem::swap(&mut s.x, &mut trial.x);
    std::mem::swap(&mut s.dx, &mut trial.dx);
    let settled = settle(s, c.nodes.len(), options, work);
    std::mem::swap(&mut s.x, &mut trial.x);
    std::mem::swap(&mut s.dx, &mut trial.dx);
    settled?;
    Ok(trial
        .dx
        .iter()
        .flatten()
        .all(|x| x.is_finite())
        .then_some(trial))
}

fn locked(s: &Geometry<'_>, c: &Component, v: usize) -> bool {
    s.protected[v]
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
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<f64>>> {
    work.charge(c.nodes.len())?;
    let mut anchors = reserved(c.nodes.len())?;
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
    let start = origin(s, c, &s.x, work)?;
    work.visit(anchors.len())?;
    let before = anchor_error(&anchors, &s.x, start);
    if !before.is_finite() || before <= 1e-12 {
        return Ok(None);
    }

    work.charge(s.x.len())?;
    let mut x = copy(&s.x)?;
    let widest = s.g.layers.iter().map(Vec::len).max().unwrap_or(0);
    let mut row = RowProjection {
        targets: reserved(widest)?,
        weights: reserved(widest)?,
        gaps: reserved(widest)?,
        pools: reserved(widest)?,
    };
    for sweep in 0..SWEEPS {
        let origin = origin(s, c, &x, work)?;
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
                if !row.fit(s, run, &anchors, origin, &mut x, work)? {
                    return Ok(None);
                }
            }
        }
    }

    let end = origin(s, c, &x, work)?;
    work.visit(anchors.len())?;
    let after = anchor_error(&anchors, &x, end);
    // Smoothing should earn its keep in the caller's component-relative coordinates.
    Ok((after < before).then_some(x))
}

struct RowProjection {
    targets: Vec<f64>,
    weights: Vec<f64>,
    gaps: Vec<f64>,
    pools: Vec<Pool>,
}

impl RowProjection {
    fn fit(
        &mut self,
        s: &Geometry<'_>,
        run: &[u32],
        anchors: &[Option<Anchor>],
        origin: f64,
        x: &mut [f64],
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        self.targets.clear();
        self.weights.clear();
        self.gaps.clear();
        self.gaps.push(0.);
        self.gaps.extend(
            run.windows(2)
                .map(|pair| s.extent[pair[0] as usize].1 + s.extent[pair[1] as usize].0),
        );
        for &v in run {
            let v = v as usize;
            let (mut mean, mut weight) = match anchors.get(v).copied().flatten() {
                Some(anchor) => (origin + anchor.cross, anchor.weight),
                None => (s.x[v], CURRENT_WEIGHT),
            };
            for side in [TOP, BOTTOM] {
                let segments = incident(s.g, v, side);
                work.visit(segments.len())?;
                let other = 1 - side;
                for &segment in segments {
                    let e = segment as usize;
                    let peer = vertex(s.g, e, other);
                    let target = x[peer] + s.dx[other][e] - s.dx[side][e];
                    let spring = f64::from(s.g.weight[e]).clamp(0.25, 4.);
                    weight += spring;
                    mean += (target - mean) * (spring / weight);
                }
            }
            if !mean.is_finite() {
                return Ok(false);
            }
            self.targets.push(mean);
            self.weights.push(weight);
        }
        // The exterior neighbor of each end of a run is fixed for the whole projection.
        let bounds = Interval {
            lo: s.room(run[0] as usize, |_| false).lo,
            hi: s.room(run[run.len() - 1] as usize, |_| false).hi,
        };
        if weighted_pava(
            &mut self.targets,
            &self.gaps,
            bounds,
            ZeroGap::Ordered,
            &mut self.pools,
            |i| self.weights[i],
        ) {
            for (&v, &target) in run.iter().zip(&self.targets) {
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
}
