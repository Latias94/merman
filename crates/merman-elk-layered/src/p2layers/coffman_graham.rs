//! Coffman-Graham layering translated from Eclipse ELK, EPL-2.0:
//! https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p2layers/CoffmanGrahamLayerer.java
//!
//! Priority-queue ties follow the GWT `java.util.PriorityQueue` emitted in elkjs 0.9.3.
//! Those ties are observable in node order, so a Rust heap's unspecified tie order is unsuitable.

use super::{LayeringError, LayeringInput, LayeringResult, charge};
use crate::work::{WorkControl, checked_sum};
use std::cmp::Ordering;

pub(super) fn layer(
    input: &LayeringInput<'_>,
    work: &mut dyn WorkControl,
) -> LayeringResult<Vec<Vec<usize>>> {
    if input.nodes.is_empty() {
        return Ok(Vec::new());
    }
    let mut marked_edges = vec![false; input.graph.edges.len()];
    let mut visited = vec![false; input.len()];
    let mut active = vec![false; input.len()];
    let mut stack = Vec::new();
    // Source transitive reduction marks edges; it does not remove them from the graph.
    for &start in &input.nodes {
        charge(work, input.len())?;
        visited.fill(false);
        for &edge in &input.outgoing[start] {
            let root = input.target(edge);
            if visited[root] {
                continue;
            }
            active[root] = true;
            stack.push((root, 0usize));
            while let Some((node, cursor)) = stack.last_mut() {
                charge(work, 1)?;
                if let Some(&edge) = input.outgoing[*node].get(*cursor) {
                    *cursor += 1;
                    let next = input.target(edge);
                    charge(work, input.incoming[next].len())?;
                    for &transitive in &input.incoming[next] {
                        if input.source(transitive) == start {
                            marked_edges[transitive] = true;
                        }
                    }
                    if active[next] {
                        return Err(LayeringError::CyclicGraph);
                    }
                    if !visited[next] {
                        active[next] = true;
                        stack.push((next, 0));
                    }
                } else {
                    let (node, _) = stack.pop().expect("the DFS frame exists");
                    visited[node] = true;
                    active[node] = false;
                }
            }
        }
    }

    let mut incoming = vec![0usize; input.len()];
    let mut predecessors = vec![Vec::new(); input.len()];
    let mut topological = vec![0usize; input.len()];
    let mut sources = Vec::new();
    for &node in &input.nodes {
        charge(work, checked_sum([input.incoming[node].len(), 1])?)?;
        incoming[node] = input.incoming[node]
            .iter()
            .filter(|&&edge| !marked_edges[edge])
            .count();
        if incoming[node] == 0 {
            push(&mut sources, node, |a, b| {
                compare_topology(&predecessors[a], &predecessors[b], work)
            })?;
        }
    }
    let mut count = 0;
    while let Some(node) = pop(&mut sources, |a, b| {
        compare_topology(&predecessors[a], &predecessors[b], work)
    })? {
        topological[node] = count;
        count += 1;
        for &edge in &input.outgoing[node] {
            charge(work, 1)?;
            if marked_edges[edge] {
                continue;
            }
            let next = input.target(edge);
            incoming[next] -= 1;
            predecessors[next].push(topological[node]);
            if incoming[next] == 0 {
                push(&mut sources, next, |a, b| {
                    compare_topology(&predecessors[a], &predecessors[b], work)
                })?;
            }
        }
    }
    if count != input.nodes.len() {
        return Err(LayeringError::CyclicGraph);
    }

    let mut outgoing = vec![0usize; input.len()];
    let mut sinks = Vec::new();
    for &node in &input.nodes {
        charge(work, checked_sum([input.outgoing[node].len(), 1])?)?;
        outgoing[node] = input.outgoing[node]
            .iter()
            .filter(|&&edge| !marked_edges[edge])
            .count();
        if outgoing[node] == 0 {
            push(&mut sinks, node, |a, b| {
                charge(work, 1)?;
                Ok(topological[b].cmp(&topological[a]))
            })?;
        }
    }
    let mut layers: Vec<Vec<usize>> = vec![Vec::new()];
    let mut assigned = vec![None; input.len()];
    let bound = input.graph.options.layering_coffman_graham_layer_bound;
    while let Some(node) = pop(&mut sinks, |a, b| {
        charge(work, 1)?;
        Ok(topological[b].cmp(&topological[a]))
    })? {
        charge(
            work,
            checked_sum([input.outgoing[node].len(), input.incoming[node].len(), 1])?,
        )?;
        let mut layer = layers.len() - 1;
        if layers[layer].len() as i64 >= i64::from(bound)
            || input.outgoing[node]
                .iter()
                .any(|&edge| assigned[input.target(edge)] == Some(layer))
        {
            layers.push(Vec::new());
            layer += 1;
        }
        assigned[node] = Some(layer);
        layers[layer].push(node);
        for &edge in &input.incoming[node] {
            if marked_edges[edge] {
                continue;
            }
            let source = input.source(edge);
            outgoing[source] -= 1;
            if outgoing[source] == 0 {
                push(&mut sinks, source, |a, b| {
                    charge(work, 1)?;
                    Ok(topological[b].cmp(&topological[a]))
                })?;
            }
        }
    }
    layers.reverse();
    Ok(layers)
}

fn compare_topology(
    left: &[usize],
    right: &[usize],
    work: &mut dyn WorkControl,
) -> LayeringResult<Ordering> {
    charge(work, 1)?;
    let mut a = left.len();
    let mut b = right.len();
    while a > 0 && b > 0 {
        charge(work, 1)?;
        a -= 1;
        b -= 1;
        // Upstream compares boxed Integers by identity before comparing their values. GWT caches
        // only -128..127; equal larger ranks therefore return Equal immediately.
        if left[a] != right[b] || left[a] >= 128 {
            return Ok(left[a].cmp(&right[b]));
        }
    }
    // Preserve upstream's hasNext(), rather than silently correcting it to hasPrevious().
    Ok(if a == left.len() && b == right.len() {
        Ordering::Equal
    } else if a == left.len() {
        Ordering::Less
    } else {
        Ordering::Greater
    })
}

fn push(
    heap: &mut Vec<usize>,
    value: usize,
    mut compare: impl FnMut(usize, usize) -> LayeringResult<Ordering>,
) -> LayeringResult<()> {
    let mut cursor = heap.len();
    heap.push(value);
    while cursor > 0 {
        let parent = (cursor - 1) / 2;
        if compare(heap[parent], value)? != Ordering::Greater {
            break;
        }
        heap[cursor] = heap[parent];
        cursor = parent;
    }
    heap[cursor] = value;
    Ok(())
}

fn pop(
    heap: &mut Vec<usize>,
    mut compare: impl FnMut(usize, usize) -> LayeringResult<Ordering>,
) -> LayeringResult<Option<usize>> {
    let Some(&result) = heap.first() else {
        return Ok(None);
    };
    let value = heap.pop().expect("the heap is nonempty");
    if !heap.is_empty() {
        let mut cursor = 0;
        while cursor * 2 + 1 < heap.len() {
            let mut child = cursor * 2 + 1;
            if child + 1 < heap.len() && compare(heap[child + 1], heap[child])? == Ordering::Less {
                child += 1;
            }
            if compare(value, heap[child])? == Ordering::Less {
                break;
            }
            heap[cursor] = heap[child];
            cursor = child;
        }
        heap[cursor] = value;
    }
    Ok(Some(result))
}
