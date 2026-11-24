use crate::work::{LayoutResult, Work, reserved};

pub(super) struct Lane {
    pub(super) target: f64,
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) gap: f64,
}

struct Pool {
    count: usize,
    mean: f64,
    min: f64,
    max: f64,
}

impl Pool {
    fn level(&self) -> f64 {
        self.mean.clamp(self.min, self.max)
    }
}

pub(super) fn project(lanes: &mut [Lane], work: &mut Work<'_>) -> LayoutResult<bool> {
    let mut pools: Vec<Pool> = reserved(lanes.len())?;
    let mut offset = 0.;
    for lane in lanes.iter() {
        work.visit(1)?;
        if !lane.min.is_finite() || !lane.max.is_finite() || lane.min > lane.max {
            return Ok(false);
        }
        offset += lane.gap;
        let mut pool = Pool {
            count: 1,
            mean: lane.target - offset,
            min: lane.min - offset,
            max: lane.max - offset,
        };
        if pool.min > pool.max || !pool.mean.is_finite() || !offset.is_finite() {
            return Ok(false);
        }
        while pools.last().is_some_and(|left| left.level() > pool.level()) {
            work.visit(1)?;
            let left = pools.pop().unwrap();
            let count = left.count + pool.count;
            let fraction = pool.count as f64 / count as f64;
            pool = Pool {
                count,
                mean: left.mean * (1. - fraction) + pool.mean * fraction,
                min: left.min.max(pool.min),
                max: left.max.min(pool.max),
            };
            if pool.min > pool.max || !pool.mean.is_finite() {
                return Ok(false);
            }
        }
        pools.push(pool);
    }
    let mut start = 0;
    offset = 0.;
    let mut previous = f64::NEG_INFINITY;
    for pool in pools {
        let level = pool.level();
        for lane in &mut lanes[start..start + pool.count] {
            work.visit(1)?;
            offset += lane.gap;
            lane.target = (level + offset)
                .clamp(lane.min, lane.max)
                .max(previous + lane.gap);
            if !lane.target.is_finite() || lane.target > lane.max {
                return Ok(false);
            }
            previous = lane.target;
        }
        start += pool.count;
    }
    Ok(true)
}
