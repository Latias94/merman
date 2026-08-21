use super::super::RelationResourceCheckpointCursor;
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

pub(crate) fn parallel_relation_lane_offsets<'a, I>(
    endpoints: I,
    resources: &mut ResourceContext,
) -> Result<Vec<isize>>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
    I::IntoIter: ExactSizeIterator,
{
    let endpoints = endpoints.into_iter();
    resources.transaction(move |resources| {
        let endpoint_count = endpoints.len();
        resources.charge_layout_work_product(endpoint_count, 3)?;
        let mut collected = Vec::new();
        collected
            .try_reserve_exact(endpoint_count)
            .map_err(|_| layout_allocation_failed())?;
        let mut checkpoints = RelationResourceCheckpointCursor::new();
        for endpoint in endpoints {
            checkpoints.tick(resources)?;
            collected.push(endpoint);
        }
        let mut counts = HashMap::<(&str, &str), usize>::new();
        counts
            .try_reserve(collected.len())
            .map_err(|_| layout_allocation_failed())?;
        for endpoint in &collected {
            checkpoints.tick(resources)?;
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
            checkpoints.tick(resources)?;
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
    })
}

pub(crate) fn parallel_lane_margin<'a, I>(
    endpoints: I,
    resources: &mut ResourceContext,
) -> Result<usize>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
    I::IntoIter: ExactSizeIterator,
{
    let endpoints = endpoints.into_iter();
    resources.transaction(move |resources| {
        let endpoint_count = endpoints.len();
        resources.charge_layout_work_product(endpoint_count, 2)?;
        let mut counts = HashMap::<(&str, &str), usize>::new();
        counts
            .try_reserve(endpoint_count)
            .map_err(|_| layout_allocation_failed())?;
        let mut checkpoints = RelationResourceCheckpointCursor::new();
        for endpoint in endpoints {
            checkpoints.tick(resources)?;
            let count = counts.entry(parallel_endpoint_key(endpoint)).or_insert(0);
            *count = count
                .checked_add(1)
                .ok_or_else(|| grid_overflow(resources))?;
        }
        let mut margin = 0usize;
        for count in counts.values().copied() {
            checkpoints.tick(resources)?;
            margin = margin.max(parallel_lane_offset_margin(count, resources)?);
        }
        Ok(margin)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy};

    #[test]
    fn parallel_lane_offsets_admit_all_passes_before_allocation() {
        const ENDPOINT_COUNT: usize = 4;
        const EXPECTED_WORK: usize = ENDPOINT_COUNT * 3;
        let endpoints = [("A", "B"), ("A", "B"), ("A", "C"), ("B", "A")];

        let exact_policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_WORK)
            .expect("exact lane work limit should be valid");
        let mut exact = ResourceContext::new(exact_policy);
        let offsets = parallel_relation_lane_offsets(endpoints, &mut exact)
            .expect("exact lane work should succeed");
        assert_eq!(offsets, vec![-6, 0, 0, 6]);
        assert_eq!(exact.layout_work_used(), EXPECTED_WORK);

        let below_policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_WORK - 1)
            .expect("below lane work limit should be valid");
        let mut below = ResourceContext::new(below_policy);
        let error = parallel_relation_lane_offsets(endpoints, &mut below)
            .expect_err("below lane work must fail before allocation");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == EXPECTED_WORK
                    && details.max == EXPECTED_WORK - 1
        ));
        assert_eq!(below.layout_work_used(), 0);
    }
}
