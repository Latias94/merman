//! Layer-order observations from the published elkjs 0.9.3 worker, captured after phase 2.
//! The worker's layerer bodies were unchanged; an observer recorded layers immediately on return.
//! Input dimensions and positions were copied before elkjs mutated its supplied graph.
//! Connected-component separation was disabled so disconnected ordering is observed together.
//! Java source: Eclipse ELK 62d5909f96fad541bc101ad52dabaece6b7eab7e (EPL-2.0).
//! Original worker SHA-256: 90ab078ad34ff826ca0ece1ee338ae98071a6b3e6cfbb00ce2eb671b32eacd88

use super::tests::{edge, graph, node};
use crate::options::{ElkDirection, LayeringStrategy};
use crate::pipeline::{LayeredPhase, execute_processors_until};

fn assert_oracle(
    strategy: LayeringStrategy,
    sizes: &[(f64, f64, f64)],
    edges: &[(usize, usize)],
    expected: &[&[usize]],
) {
    let nodes = sizes
        .iter()
        .enumerate()
        .map(|(index, &(width, height, _))| {
            let mut node = node(&index.to_string());
            node.width = width;
            node.height = height;
            node
        })
        .collect();
    let edges = edges
        .iter()
        .enumerate()
        .map(|(index, &(source, target))| {
            edge(
                &format!("e{index}"),
                &source.to_string(),
                &target.to_string(),
            )
        })
        .collect();
    let mut graph = graph(nodes, edges);
    graph.options = crate::options::LayeredOptions {
        direction: ElkDirection::Right,
        layering_strategy: strategy,
        layering_coffman_graham_layer_bound: 2,
        layering_min_width_upper_bound: -1,
        layering_min_width_upper_layer_estimation_scaling_factor: -1,
        ..Default::default()
    };
    for (node, &(_, _, x)) in graph.layerless_nodes.iter_mut().zip(sizes) {
        node.position.x = x;
    }
    execute_processors_until(&mut graph, LayeredPhase::P2Layering).unwrap();
    let actual = graph
        .layers
        .iter()
        .map(|layer| {
            layer
                .nodes
                .iter()
                .map(|&node| graph.layerless_nodes[node].id.parse::<usize>().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{strategy:?}");
}

#[test]
fn diamond_tail_longest_path() {
    assert_oracle(
        LayeringStrategy::LongestPath,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 2), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5), (1, 5)],
        &[&[0, 1], &[2], &[3, 4], &[5, 6]],
    );
}

#[test]
fn diamond_tail_longest_path_source() {
    assert_oracle(
        LayeringStrategy::LongestPathSource,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 2), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5), (1, 5)],
        &[&[0, 1, 6], &[2], &[3, 4], &[5]],
    );
}

#[test]
fn diamond_tail_coffman_graham() {
    assert_oracle(
        LayeringStrategy::CoffmanGraham,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 2), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5), (1, 5)],
        &[&[1, 0], &[2, 6], &[3, 4], &[5]],
    );
}

#[test]
fn diamond_tail_interactive() {
    assert_oracle(
        LayeringStrategy::Interactive,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 2), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5), (1, 5)],
        &[&[0, 1, 6], &[2], &[3, 4], &[5]],
    );
}

#[test]
fn diamond_tail_min_width() {
    assert_oracle(
        LayeringStrategy::MinWidth,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 2), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5), (1, 5)],
        &[&[1, 0], &[2], &[4], &[3], &[5, 6]],
    );
}

#[test]
fn diamond_tail_stretch_width() {
    assert_oracle(
        LayeringStrategy::StretchWidth,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 2), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5), (1, 5)],
        &[&[1, 0], &[2], &[3, 4], &[5, 6]],
    );
}

#[test]
fn disconnected_order_longest_path() {
    assert_oracle(
        LayeringStrategy::LongestPath,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 1), (1, 2), (3, 4)],
        &[&[0], &[1, 3], &[2, 4, 5, 6]],
    );
}

#[test]
fn disconnected_order_longest_path_source() {
    assert_oracle(
        LayeringStrategy::LongestPathSource,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 1), (1, 2), (3, 4)],
        &[&[0, 3, 5, 6], &[1, 4], &[2]],
    );
}

#[test]
fn disconnected_order_coffman_graham() {
    assert_oracle(
        LayeringStrategy::CoffmanGraham,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 1), (1, 2), (3, 4)],
        &[&[0], &[6, 3], &[4, 1], &[2, 5]],
    );
}

#[test]
fn disconnected_order_interactive() {
    assert_oracle(
        LayeringStrategy::Interactive,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 1), (1, 2), (3, 4)],
        &[&[0, 3, 5, 6], &[1, 4], &[2]],
    );
}

#[test]
fn disconnected_order_min_width() {
    assert_oracle(
        LayeringStrategy::MinWidth,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 1), (1, 2), (3, 4)],
        &[&[6], &[5], &[3], &[4], &[0], &[1], &[2]],
    );
}

#[test]
fn disconnected_order_stretch_width() {
    assert_oracle(
        LayeringStrategy::StretchWidth,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[(0, 1), (1, 2), (3, 4)],
        &[&[3, 6], &[0, 4, 5], &[1], &[2]],
    );
}

#[test]
fn transitive_parallel_longest_path() {
    assert_oracle(
        LayeringStrategy::LongestPath,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[
            (0, 1),
            (0, 1),
            (1, 2),
            (0, 2),
            (0, 3),
            (3, 4),
            (0, 4),
            (1, 4),
            (4, 5),
        ],
        &[&[0], &[1, 3], &[4], &[2, 5]],
    );
}

#[test]
fn transitive_parallel_longest_path_source() {
    assert_oracle(
        LayeringStrategy::LongestPathSource,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[
            (0, 1),
            (0, 1),
            (1, 2),
            (0, 2),
            (0, 3),
            (3, 4),
            (0, 4),
            (1, 4),
            (4, 5),
        ],
        &[&[0], &[1, 3], &[2, 4], &[5]],
    );
}

#[test]
fn transitive_parallel_coffman_graham() {
    assert_oracle(
        LayeringStrategy::CoffmanGraham,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[
            (0, 1),
            (0, 1),
            (1, 2),
            (0, 2),
            (0, 3),
            (3, 4),
            (0, 4),
            (1, 4),
            (4, 5),
        ],
        &[&[0], &[1, 3], &[4], &[5, 2]],
    );
}

#[test]
fn transitive_parallel_interactive() {
    assert_oracle(
        LayeringStrategy::Interactive,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[
            (0, 1),
            (0, 1),
            (1, 2),
            (0, 2),
            (0, 3),
            (3, 4),
            (0, 4),
            (1, 4),
            (4, 5),
        ],
        &[&[0], &[1, 3], &[2, 4], &[5]],
    );
}

#[test]
fn transitive_parallel_min_width() {
    assert_oracle(
        LayeringStrategy::MinWidth,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[
            (0, 1),
            (0, 1),
            (1, 2),
            (0, 2),
            (0, 3),
            (3, 4),
            (0, 4),
            (1, 4),
            (4, 5),
        ],
        &[&[0], &[3], &[1], &[4], &[5], &[2]],
    );
}

#[test]
fn transitive_parallel_stretch_width() {
    assert_oracle(
        LayeringStrategy::StretchWidth,
        &[
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 20.0, 0.0),
        ],
        &[
            (0, 1),
            (0, 1),
            (1, 2),
            (0, 2),
            (0, 3),
            (3, 4),
            (0, 4),
            (1, 4),
            (4, 5),
        ],
        &[&[0], &[1, 3], &[4], &[2, 5]],
    );
}

#[test]
fn width_pressure_longest_path() {
    assert_oracle(
        LayeringStrategy::LongestPath,
        &[
            (40.0, 10.0, 0.0),
            (40.0, 70.0, 0.0),
            (40.0, 15.0, 0.0),
            (40.0, 40.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 90.0, 0.0),
            (40.0, 30.0, 0.0),
            (40.0, 10.0, 0.0),
            (40.0, 25.0, 0.0),
        ],
        &[
            (0, 3),
            (0, 4),
            (1, 3),
            (1, 5),
            (2, 4),
            (2, 5),
            (3, 6),
            (4, 7),
            (5, 8),
            (0, 8),
        ],
        &[&[0, 1, 2], &[3, 4, 5], &[6, 7, 8]],
    );
}

#[test]
fn width_pressure_longest_path_source() {
    assert_oracle(
        LayeringStrategy::LongestPathSource,
        &[
            (40.0, 10.0, 0.0),
            (40.0, 70.0, 0.0),
            (40.0, 15.0, 0.0),
            (40.0, 40.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 90.0, 0.0),
            (40.0, 30.0, 0.0),
            (40.0, 10.0, 0.0),
            (40.0, 25.0, 0.0),
        ],
        &[
            (0, 3),
            (0, 4),
            (1, 3),
            (1, 5),
            (2, 4),
            (2, 5),
            (3, 6),
            (4, 7),
            (5, 8),
            (0, 8),
        ],
        &[&[0, 1, 2], &[3, 4, 5], &[6, 7, 8]],
    );
}

#[test]
fn width_pressure_coffman_graham() {
    assert_oracle(
        LayeringStrategy::CoffmanGraham,
        &[
            (40.0, 10.0, 0.0),
            (40.0, 70.0, 0.0),
            (40.0, 15.0, 0.0),
            (40.0, 40.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 90.0, 0.0),
            (40.0, 30.0, 0.0),
            (40.0, 10.0, 0.0),
            (40.0, 25.0, 0.0),
        ],
        &[
            (0, 3),
            (0, 4),
            (1, 3),
            (1, 5),
            (2, 4),
            (2, 5),
            (3, 6),
            (4, 7),
            (5, 8),
            (0, 8),
        ],
        &[&[0], &[2, 1], &[4, 3], &[6, 5], &[8, 7]],
    );
}

#[test]
fn width_pressure_interactive() {
    assert_oracle(
        LayeringStrategy::Interactive,
        &[
            (40.0, 10.0, 0.0),
            (40.0, 70.0, 0.0),
            (40.0, 15.0, 0.0),
            (40.0, 40.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 90.0, 0.0),
            (40.0, 30.0, 0.0),
            (40.0, 10.0, 0.0),
            (40.0, 25.0, 0.0),
        ],
        &[
            (0, 3),
            (0, 4),
            (1, 3),
            (1, 5),
            (2, 4),
            (2, 5),
            (3, 6),
            (4, 7),
            (5, 8),
            (0, 8),
        ],
        &[&[0, 1, 2], &[3, 4, 5], &[6, 7, 8]],
    );
}

#[test]
fn width_pressure_min_width() {
    assert_oracle(
        LayeringStrategy::MinWidth,
        &[
            (40.0, 10.0, 0.0),
            (40.0, 70.0, 0.0),
            (40.0, 15.0, 0.0),
            (40.0, 40.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 90.0, 0.0),
            (40.0, 30.0, 0.0),
            (40.0, 10.0, 0.0),
            (40.0, 25.0, 0.0),
        ],
        &[
            (0, 3),
            (0, 4),
            (1, 3),
            (1, 5),
            (2, 4),
            (2, 5),
            (3, 6),
            (4, 7),
            (5, 8),
            (0, 8),
        ],
        &[&[2], &[1], &[0, 5], &[4, 8], &[3], &[6, 7]],
    );
}

#[test]
fn width_pressure_stretch_width() {
    assert_oracle(
        LayeringStrategy::StretchWidth,
        &[
            (40.0, 10.0, 0.0),
            (40.0, 70.0, 0.0),
            (40.0, 15.0, 0.0),
            (40.0, 40.0, 0.0),
            (40.0, 20.0, 0.0),
            (40.0, 90.0, 0.0),
            (40.0, 30.0, 0.0),
            (40.0, 10.0, 0.0),
            (40.0, 25.0, 0.0),
        ],
        &[
            (0, 3),
            (0, 4),
            (1, 3),
            (1, 5),
            (2, 4),
            (2, 5),
            (3, 6),
            (4, 7),
            (5, 8),
            (0, 8),
        ],
        &[&[1, 2], &[5], &[0], &[3, 4], &[8, 6, 7]],
    );
}

#[test]
fn position_spans_longest_path() {
    assert_oracle(
        LayeringStrategy::LongestPath,
        &[
            (30.0, 20.0, 200.0),
            (40.0, 20.0, 0.0),
            (20.0, 20.0, 0.0),
            (25.0, 20.0, 100.0),
            (50.0, 20.0, 100.0),
            (30.0, 20.0, 50.0),
            (20.0, 20.0, 300.0),
        ],
        &[(0, 2), (1, 3), (2, 4), (3, 4), (4, 5)],
        &[&[0, 1], &[2, 3], &[4], &[5, 6]],
    );
}

#[test]
fn position_spans_longest_path_source() {
    assert_oracle(
        LayeringStrategy::LongestPathSource,
        &[
            (30.0, 20.0, 200.0),
            (40.0, 20.0, 0.0),
            (20.0, 20.0, 0.0),
            (25.0, 20.0, 100.0),
            (50.0, 20.0, 100.0),
            (30.0, 20.0, 50.0),
            (20.0, 20.0, 300.0),
        ],
        &[(0, 2), (1, 3), (2, 4), (3, 4), (4, 5)],
        &[&[0, 1, 6], &[2, 3], &[4], &[5]],
    );
}

#[test]
fn position_spans_coffman_graham() {
    assert_oracle(
        LayeringStrategy::CoffmanGraham,
        &[
            (30.0, 20.0, 200.0),
            (40.0, 20.0, 0.0),
            (20.0, 20.0, 0.0),
            (25.0, 20.0, 100.0),
            (50.0, 20.0, 100.0),
            (30.0, 20.0, 50.0),
            (20.0, 20.0, 300.0),
        ],
        &[(0, 2), (1, 3), (2, 4), (3, 4), (4, 5)],
        &[&[0], &[6, 1], &[3, 2], &[4], &[5]],
    );
}

#[test]
fn position_spans_interactive() {
    assert_oracle(
        LayeringStrategy::Interactive,
        &[
            (30.0, 20.0, 200.0),
            (40.0, 20.0, 0.0),
            (20.0, 20.0, 0.0),
            (25.0, 20.0, 100.0),
            (50.0, 20.0, 100.0),
            (30.0, 20.0, 50.0),
            (20.0, 20.0, 300.0),
        ],
        &[(0, 2), (1, 3), (2, 4), (3, 4), (4, 5)],
        &[&[1], &[3], &[0], &[6, 2], &[4], &[5]],
    );
}

#[test]
fn position_spans_min_width() {
    assert_oracle(
        LayeringStrategy::MinWidth,
        &[
            (30.0, 20.0, 200.0),
            (40.0, 20.0, 0.0),
            (20.0, 20.0, 0.0),
            (25.0, 20.0, 100.0),
            (50.0, 20.0, 100.0),
            (30.0, 20.0, 50.0),
            (20.0, 20.0, 300.0),
        ],
        &[(0, 2), (1, 3), (2, 4), (3, 4), (4, 5)],
        &[&[0, 1], &[2, 3], &[4], &[5, 6]],
    );
}

#[test]
fn position_spans_stretch_width() {
    assert_oracle(
        LayeringStrategy::StretchWidth,
        &[
            (30.0, 20.0, 200.0),
            (40.0, 20.0, 0.0),
            (20.0, 20.0, 0.0),
            (25.0, 20.0, 100.0),
            (50.0, 20.0, 100.0),
            (30.0, 20.0, 50.0),
            (20.0, 20.0, 300.0),
        ],
        &[(0, 2), (1, 3), (2, 4), (3, 4), (4, 5)],
        &[&[0, 1], &[2, 3], &[4], &[5, 6]],
    );
}
