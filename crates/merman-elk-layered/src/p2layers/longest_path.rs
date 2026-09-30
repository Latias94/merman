//! Longest-path-to-sink and longest-path-to-source layer assignment.
//!
//! Translated from Eclipse ELK, EPL-2.0:
//! https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p2layers
//! (`LongestPathLayerer.java`, `LongestPathSourceLayerer.java`).

use super::{LayeringError, LayeringInput, LayeringResult, charge};
use crate::work::WorkControl;

pub(super) fn layer(
    input: &LayeringInput<'_>,
    from_sources: bool,
    work: &mut dyn WorkControl,
) -> LayeringResult<Vec<Vec<usize>>> {
    let mut heights = vec![0usize; input.len()];
    let mut visiting = vec![false; input.len()];
    let mut finished = Vec::with_capacity(input.nodes.len());
    let mut stack = Vec::new();
    let adjacency = if from_sources {
        &input.incoming
    } else {
        &input.outgoing
    };
    for &root in &input.nodes {
        if heights[root] != 0 {
            continue;
        }
        visiting[root] = true;
        stack.push((root, 0usize, 1usize));
        // An explicit DFS stack preserves Java's postorder without depending on native stack depth.
        while let Some((node, cursor, height)) = stack.last_mut() {
            charge(work, 1)?;
            if let Some(&edge) = adjacency[*node].get(*cursor) {
                let next = if from_sources {
                    input.source(edge)
                } else {
                    input.target(edge)
                };
                if next == *node {
                    *cursor += 1;
                    continue;
                }
                if heights[next] != 0 {
                    *height = (*height).max(heights[next] + 1);
                    *cursor += 1;
                } else if visiting[next] {
                    return Err(LayeringError::CyclicGraph);
                } else {
                    visiting[next] = true;
                    stack.push((next, 0, 1));
                }
            } else {
                let (node, _, height) = stack.pop().expect("the DFS frame exists");
                heights[node] = height;
                visiting[node] = false;
                finished.push(node);
            }
        }
    }
    let count = heights.iter().copied().max().unwrap_or(0);
    let mut layers = vec![Vec::new(); count];
    for node in finished {
        let index = if from_sources {
            heights[node] - 1
        } else {
            count - heights[node]
        };
        layers[index].push(node);
    }
    Ok(layers)
}
