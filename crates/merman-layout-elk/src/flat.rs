//! Projection of already measured flat graphs into the additional ELK providers.
//!
//! Mermaid 12 `createRootElkGraph` only changes non-layered provider defaults for
//! Rectpacking. In particular, Layered's base spacing of 40 is not a Force/Tree option.
//! Hierarchy and renderer-side missing-section handling remain separate responsibilities.

use super::*;
use source_port::algorithms::{
    self, box_layout, force, mrtree, radial, rectpacking, spore_overlap, stress,
};

#[cfg(test)]
mod tests;

pub(super) fn layout(
    graph: &Graph,
    operation_seed: Option<source_port::OperationSeed>,
    work: &mut dyn WorkControl,
) -> Result<LayoutResult> {
    let units = graph
        .nodes
        .len()
        .checked_add(graph.edges.len())
        .and_then(|n| n.checked_mul(4))
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.check(units)?;
    work.charge(units)?;
    let node_indices: HashMap<_, _> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();
    // The hierarchy index has already checked duplicate IDs and missing endpoints.
    let endpoints: Vec<_> = graph
        .edges
        .iter()
        .map(|edge| {
            (
                node_indices[edge.source.as_str()],
                node_indices[edge.target.as_str()],
            )
        })
        .collect();
    let mut result = LayoutResult {
        nodes: graph
            .nodes
            .iter()
            .map(|node| NodeLayout {
                id: node.id.clone(),
                x: 0.0,
                y: 0.0,
                width: node.width,
                height: node.height,
            })
            .collect(),
        // Empty point chains mean that the provider emitted no section. Do not replace them
        // with fake routes: Mermaid treats this case differently for clipping and labels.
        edges: graph
            .edges
            .iter()
            .map(|edge| EdgeLayout {
                id: edge.id.clone(),
                points: Vec::new(),
                labels: edge
                    .label
                    .into_iter()
                    .map(|label| EdgeLabelLayout {
                        x: 0.0,
                        y: 0.0,
                        width: label.width,
                        height: label.height,
                    })
                    .collect(),
            })
            .collect(),
    };
    match graph.options.algorithm {
        Algorithm::Layered => unreachable!("layered graphs use the compound-aware pipeline"),
        Algorithm::Box | Algorithm::Rectpacking => {
            let rectangles: Vec<_> = graph
                .nodes
                .iter()
                .map(|node| box_layout::Rectangle {
                    width: node.width,
                    height: node.height,
                    ..Default::default()
                })
                .collect();
            let packed = if graph.options.algorithm == Algorithm::Box {
                box_layout::layout(&rectangles, &box_layout::Options::default(), work)?
            } else {
                // render.ts RECTPACKING_OPTIONS. SCANLINE is absent from elkjs 0.9.3's
                // enum, so it resolves to GREEDY, the kernel's implemented default.
                rectpacking::layout(
                    &rectangles,
                    &rectpacking::Options {
                        aspect_ratio: 1.6,
                        try_box: true,
                        expand_nodes: true,
                        row_height_reevaluation: true,
                        compaction_iterations: 10,
                        eliminate_whitespace: true,
                        horizontal_content_alignment: box_layout::ContentAlignment::Center,
                        ..Default::default()
                    },
                    work,
                )?
            };
            for (node, position) in result.nodes.iter_mut().zip(packed.rectangles) {
                node.width = position.width;
                node.height = position.height;
                node.x = position.x + position.width / 2.0;
                node.y = position.y + position.height / 2.0;
            }
        }
        Algorithm::Force | Algorithm::Stress => {
            let nodes: Vec<_> = graph
                .nodes
                .iter()
                .map(|node| force::Node {
                    width: node.width,
                    height: node.height,
                    ..Default::default()
                })
                .collect();
            let edges: Vec<_> = graph
                .edges
                .iter()
                .zip(&endpoints)
                .map(|(edge, &(source, target))| {
                    let mut projected = force::Edge::new(source, target);
                    // Mermaid creates one inline ELK label even for an unlabeled edge. Force
                    // imports that zero-size label as a particle, so omitting it changes layout.
                    let label = edge.label.unwrap_or(Label {
                        width: 0.0,
                        height: 0.0,
                    });
                    projected.labels = vec![force::Label {
                        width: label.width,
                        height: label.height,
                        inline: true,
                        ..Default::default()
                    }];
                    projected
                })
                .collect();
            let options = force::Options {
                resolved_seed: algorithms::resolve_seed(
                    graph.options.layered.random_seed,
                    operation_seed,
                    &source_port::GraphSeedScope::root(graph.id.as_str()),
                    algorithms::RandomDomain::Force,
                )?,
                ..Default::default()
            };
            let placed = if graph.options.algorithm == Algorithm::Force {
                force::layout(&nodes, &edges, &options, work)?
            } else {
                let nodes: Vec<_> = nodes
                    .into_iter()
                    .map(|geometry| stress::Node {
                        geometry,
                        fixed: false,
                    })
                    .collect();
                let edges: Vec<_> = edges
                    .into_iter()
                    .map(|geometry| stress::Edge {
                        geometry,
                        desired_length: None,
                    })
                    .collect();
                stress::layout(
                    &nodes,
                    &edges,
                    &stress::Options {
                        force: options,
                        ..Default::default()
                    },
                    work,
                )?
            };
            for (node, position) in result.nodes.iter_mut().zip(placed.nodes) {
                node.x = position.x + node.width / 2.0;
                node.y = position.y + node.height / 2.0;
            }
            for (edge, route) in result.edges.iter_mut().zip(placed.edges) {
                if let Some(route) = route {
                    edge.points = vec![
                        Point {
                            x: route.start.x,
                            y: route.start.y,
                        },
                        Point {
                            x: route.end.x,
                            y: route.end.y,
                        },
                    ];
                    for (label, position) in edge.labels.iter_mut().zip(route.labels) {
                        label.x = position.x;
                        label.y = position.y;
                    }
                }
            }
        }
        Algorithm::MrTree => {
            let nodes: Vec<_> = graph
                .nodes
                .iter()
                .map(|node| mrtree::Node {
                    width: node.width,
                    height: node.height,
                    // Mermaid leaf nodes have no ELK node-label array. MrTree derives identity
                    // from its local ordinal when the first label text is absent.
                    label: String::new(),
                    ..Default::default()
                })
                .collect();
            let edges: Vec<_> = endpoints
                .iter()
                .map(|&(source, target)| mrtree::Edge { source, target })
                .collect();
            let placed = mrtree::layout(
                &nodes,
                &edges,
                &mrtree::Options {
                    direction: match graph.direction {
                        Direction::Down => mrtree::Direction::Down,
                        Direction::Up => mrtree::Direction::Up,
                        Direction::Left => mrtree::Direction::Left,
                        Direction::Right => mrtree::Direction::Right,
                    },
                    ..Default::default()
                },
                work,
            )?;
            for (node, position) in result.nodes.iter_mut().zip(placed.nodes) {
                node.x = position.x + node.width / 2.0;
                node.y = position.y + node.height / 2.0;
            }
            for (edge, route) in result.edges.iter_mut().zip(placed.edges) {
                if let Some(points) = route {
                    edge.points = points
                        .into_iter()
                        .map(|p| Point { x: p.x, y: p.y })
                        .collect();
                }
            }
        }
        Algorithm::Radial => {
            let nodes: Vec<_> = graph
                .nodes
                .iter()
                .map(|node| radial::Node {
                    width: node.width,
                    height: node.height,
                    ..Default::default()
                })
                .collect();
            let edges: Vec<_> = endpoints
                .iter()
                .map(|&(source, target)| radial::Edge::new(source, target))
                .collect();
            let placed = radial::layout(&nodes, &edges, &radial::Options::default(), work)?;
            for (node, position) in result.nodes.iter_mut().zip(placed.nodes) {
                node.x = position.x + node.width / 2.0;
                node.y = position.y + node.height / 2.0;
            }
            for (edge, route) in result.edges.iter_mut().zip(placed.edges) {
                if let Some(route) = route {
                    edge.points = vec![
                        Point {
                            x: route.start.x,
                            y: route.start.y,
                        },
                        Point {
                            x: route.end.x,
                            y: route.end.y,
                        },
                    ];
                }
            }
        }
        Algorithm::SporeOverlap => {
            // Upstream jitters coincident centers through Math.random, independently of
            // randomSeed. Use operation entropy when supplied, keeping raw calls replayable.
            let seed = algorithms::resolve_seed(
                if operation_seed.is_some() {
                    0
                } else {
                    graph.options.layered.random_seed
                },
                operation_seed,
                &source_port::GraphSeedScope::root(graph.id.as_str()),
                algorithms::RandomDomain::SporeOverlap,
            )?;
            let mut random = algorithms::random_stream(seed);
            let nodes: Vec<_> = graph
                .nodes
                .iter()
                .map(|node| spore_overlap::Node {
                    width: node.width,
                    height: node.height,
                    ..Default::default()
                })
                .collect();
            let edges: Vec<_> = endpoints
                .iter()
                .map(|&(source, target)| spore_overlap::Edge { source, target })
                .collect();
            let placed = spore_overlap::layout(
                &nodes,
                &edges,
                &spore_overlap::Options::default(),
                &mut random,
                work,
            )?;
            for (node, position) in result.nodes.iter_mut().zip(placed.nodes) {
                node.x = position.x + node.width / 2.0;
                node.y = position.y + node.height / 2.0;
            }
            for (edge, route) in result.edges.iter_mut().zip(placed.edges) {
                edge.points = route
                    .into_iter()
                    .map(|p| Point { x: p.x, y: p.y })
                    .collect();
                for label in &mut edge.labels {
                    label.x += placed.translation.x;
                    label.y += placed.translation.y;
                }
            }
        }
    }
    Ok(result)
}
