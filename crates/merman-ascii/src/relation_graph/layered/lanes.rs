use std::collections::HashMap;

pub(crate) fn parallel_lane_offset(index: usize, count: usize) -> isize {
    if count <= 1 {
        return 0;
    }
    (index as isize * 2 - (count as isize - 1)) * 3
}

pub(crate) fn parallel_relation_lane_offsets<'a>(
    endpoints: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<isize> {
    let endpoints = endpoints.into_iter().collect::<Vec<_>>();
    let mut counts = HashMap::<(&str, &str), usize>::new();
    for endpoint in &endpoints {
        *counts.entry(parallel_endpoint_key(*endpoint)).or_insert(0) += 1;
    }

    let mut seen = HashMap::<(&str, &str), usize>::new();
    endpoints
        .into_iter()
        .map(|endpoint| {
            let key = parallel_endpoint_key(endpoint);
            let index = seen.entry(key).or_insert(0);
            let offset = parallel_lane_offset(*index, counts[&key]);
            *index += 1;
            offset
        })
        .collect()
}

pub(crate) fn parallel_lane_margin<'a>(
    endpoints: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> usize {
    let mut counts = HashMap::<(&str, &str), usize>::new();
    for endpoint in endpoints {
        *counts.entry(parallel_endpoint_key(endpoint)).or_insert(0) += 1;
    }

    counts
        .values()
        .copied()
        .map(parallel_lane_offset_margin)
        .max()
        .unwrap_or(0)
}

fn parallel_lane_offset_margin(count: usize) -> usize {
    count.saturating_sub(1).saturating_mul(3)
}

fn parallel_endpoint_key<'a>(endpoint: (&'a str, &'a str)) -> (&'a str, &'a str) {
    if endpoint.0 <= endpoint.1 {
        endpoint
    } else {
        (endpoint.1, endpoint.0)
    }
}
