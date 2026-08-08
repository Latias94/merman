use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::{AsciiError, Result};
use std::collections::HashMap;

pub(crate) fn parallel_lane_offset(
    index: usize,
    count: usize,
    resources: &ResourceContext,
) -> Result<isize> {
    if count <= 1 {
        return Ok(0);
    }
    let index = isize::try_from(index).map_err(|_| grid_overflow(resources))?;
    let count = isize::try_from(count).map_err(|_| grid_overflow(resources))?;
    index
        .checked_mul(2)
        .and_then(|value| value.checked_sub(count.checked_sub(1)?))
        .and_then(|value| value.checked_mul(3))
        .ok_or_else(|| grid_overflow(resources))
}

pub(crate) fn parallel_relation_lane_offsets<'a>(
    endpoints: impl IntoIterator<Item = (&'a str, &'a str)>,
    resources: &mut ResourceContext,
) -> Result<Vec<isize>> {
    let endpoints = endpoints.into_iter();
    let mut collected = Vec::new();
    collected
        .try_reserve(endpoints.size_hint().0)
        .map_err(|_| layout_allocation_failed())?;
    for endpoint in endpoints {
        resources.charge_layout_work(1)?;
        collected.push(endpoint);
    }
    let mut counts = HashMap::<(&str, &str), usize>::new();
    counts
        .try_reserve(collected.len())
        .map_err(|_| layout_allocation_failed())?;
    for endpoint in &collected {
        let count = counts.entry(parallel_endpoint_key(*endpoint)).or_insert(0);
        *count = count
            .checked_add(1)
            .ok_or_else(|| grid_overflow(resources))?;
    }

    let mut seen = HashMap::<(&str, &str), usize>::new();
    seen.try_reserve(collected.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(collected.len())
        .map_err(|_| layout_allocation_failed())?;
    for endpoint in collected {
        resources.charge_layout_work(1)?;
        let key = parallel_endpoint_key(endpoint);
        let index = seen.entry(key).or_insert(0);
        let count = counts
            .get(&key)
            .copied()
            .ok_or_else(|| grid_overflow(resources))?;
        offsets.push(parallel_lane_offset(*index, count, resources)?);
        *index = index
            .checked_add(1)
            .ok_or_else(|| grid_overflow(resources))?;
    }
    Ok(offsets)
}

pub(crate) fn parallel_lane_margin<'a>(
    endpoints: impl IntoIterator<Item = (&'a str, &'a str)>,
    resources: &mut ResourceContext,
) -> Result<usize> {
    let mut counts = HashMap::<(&str, &str), usize>::new();
    for endpoint in endpoints {
        resources.charge_layout_work(1)?;
        let count = counts.entry(parallel_endpoint_key(endpoint)).or_insert(0);
        *count = count
            .checked_add(1)
            .ok_or_else(|| grid_overflow(resources))?;
    }

    counts.values().copied().try_fold(0usize, |margin, count| {
        Ok::<usize, AsciiError>(margin.max(parallel_lane_offset_margin(count, resources)?))
    })
}

fn parallel_lane_offset_margin(count: usize, resources: &ResourceContext) -> Result<usize> {
    count
        .saturating_sub(1)
        .checked_mul(3)
        .ok_or_else(|| grid_overflow(resources))
}

fn parallel_endpoint_key<'a>(endpoint: (&'a str, &'a str)) -> (&'a str, &'a str) {
    if endpoint.0 <= endpoint.1 {
        endpoint
    } else {
        (endpoint.1, endpoint.0)
    }
}

fn grid_overflow(resources: &ResourceContext) -> AsciiError {
    resources.grid_overflow()
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}
