//! Stretch-width layering translated from Eclipse ELK, EPL-2.0:
//! https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p2layers/StretchWidthLayerer.java

use super::{LayeringError, LayeringInput, LayeringResult, charge};
use crate::graph::LNodeKind;
use crate::work::{WorkControl, checked_n_log_n, checked_sum};
use std::cmp::Reverse;

pub(super) fn layer(
    input: &LayeringInput<'_>,
    work: &mut dyn WorkControl,
) -> LayeringResult<Vec<Vec<usize>>> {
    if input.nodes.is_empty() {
        return Ok(Vec::new());
    }
    let mut ranks = vec![0; input.len()];
    let mut incoming = vec![0; input.len()];
    let mut outgoing = vec![0; input.len()];
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    let mut total_outgoing = 0.0;
    for &node in &input.nodes {
        charge(work, checked_sum([input.incoming[node].len(), 1])?)?;
        outgoing[node] = input.outgoing[node].len();
        incoming[node] = input.incoming[node].len();
        ranks[node] = input.incoming[node]
            .iter()
            .map(|&edge| input.outgoing[input.source(edge)].len())
            .fold(outgoing[node], usize::max);
        total_outgoing += outgoing[node] as f64;
        let data = &input.graph.layerless_nodes[node];
        if data.kind == LNodeKind::Normal {
            minimum = minimum.min(data.size.height);
            maximum = maximum.max(data.size.height);
        }
    }
    let mut nodes = input.nodes.clone();
    charge(work, checked_n_log_n(nodes.len())?)?;
    nodes.sort_by_key(|&node| Reverse(ranks[node]));
    // ELK normalizes nodes before clamping the minimum to one. A zero minimum together with a
    // positive height makes the adaptive retry loop infinite. Keep the all-zero case's NaN
    // comparisons (both Java and JavaScript treat them as false), but reject infinite sizes.
    let mut sizes = vec![0.0; input.len()];
    for &node in &nodes {
        sizes[node] = input.graph.layerless_nodes[node].size.height / minimum;
        if sizes[node].is_infinite() {
            return Err(LayeringError::InvalidStretchWidthDimensions);
        }
    }
    minimum = minimum.max(1.0);
    maximum = maximum.max(1.0);
    let dummy_size = input.graph.options.spacing.edge_edge / minimum;
    let mut max_width = maximum / minimum;
    let influence = total_outgoing / nodes.len() as f64;
    let mut remaining = nodes.clone();
    let mut remaining_outgoing = outgoing.clone();
    let mut layers: Vec<Vec<usize>> = vec![Vec::new()];
    let mut width_current = 0.0;
    let mut width_up = 0.0;
    while !remaining.is_empty() {
        charge(work, checked_sum([remaining.len(), 1])?)?;
        let selected = remaining
            .iter()
            .position(|&node| remaining_outgoing[node] == 0);
        let go_up = selected.is_some_and(|index| {
            let node = remaining[index];
            width_current - outgoing[node] as f64 * dummy_size + sizes[node] > max_width
                || width_up + incoming[node] as f64 * dummy_size
                    > max_width * influence * dummy_size
        });
        let current = layers.last_mut().expect("the initial layer exists");
        if selected.is_none() || (go_up && !current.is_empty()) {
            if current.is_empty() {
                return Err(LayeringError::CyclicGraph);
            }
            for &node in current.iter() {
                charge(work, checked_sum([input.incoming[node].len(), 1])?)?;
                for &edge in &input.incoming[node] {
                    remaining_outgoing[input.source(edge)] -= 1;
                }
            }
            layers.push(Vec::new());
            width_current = width_up;
            width_up = 0.0;
        } else if go_up {
            charge(work, checked_sum([input.len(), nodes.len(), layers.len()])?)?;
            layers.clear();
            layers.push(Vec::new());
            width_current = 0.0;
            width_up = 0.0;
            let next = max_width + 1.0;
            if !next.is_finite() || next == max_width {
                return Err(LayeringError::InvalidStretchWidthDimensions);
            }
            max_width = next;
            remaining.clone_from(&nodes);
            remaining_outgoing.clone_from(&outgoing);
        } else {
            let Some(selected) = selected else {
                return Err(LayeringError::CyclicGraph);
            };
            let node = remaining.remove(selected);
            current.push(node);
            width_current = width_current - outgoing[node] as f64 * dummy_size + sizes[node];
            width_up += incoming[node] as f64 * dummy_size;
        }
    }
    layers.reverse();
    Ok(layers)
}
