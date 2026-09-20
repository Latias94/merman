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

pub(super) struct FlatLayout {
    pub layout: LayoutResult,
    pub size: source_port::LSize,
    pub content_shifts: HashMap<String, Point>,
    pub size_constraints_active: bool,
}

pub(super) enum ScopeContext<'a> {
    Root,
    Container { node: &'a Node, mode: ContainerMode },
}

pub(super) struct NodeContext {
    pub mode: ContainerMode,
    pub size_constraints_active: bool,
}

pub(super) fn layout(
    graph: &Graph,
    scope: &source_port::GraphSeedScope,
    context: ScopeContext<'_>,
    node_contexts: &[NodeContext],
    operation_seed: Option<source_port::OperationSeed>,
    work: &mut dyn WorkControl,
) -> Result<FlatLayout> {
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
    let micro_layout = graph
        .nodes
        .iter()
        .zip(node_contexts)
        .map(|(node, context)| {
            source_port::inside_top_center_micro_layout(
                source_port::LSize {
                    width: node.width,
                    height: node.height,
                },
                context
                    .size_constraints_active
                    .then(|| context.mode.minimum(node))
                    .flatten(),
                (node.kind == NodeKind::Group)
                    .then_some(node.label)
                    .flatten()
                    .map(|label| source_port::LSize {
                        width: label.width,
                        height: label.height,
                    }),
            )
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let uses_micro_layout = !matches!(
        graph.options.algorithm,
        Algorithm::Box | Algorithm::SporeOverlap
    );
    let mut result = LayoutResult {
        nodes: graph
            .nodes
            .iter()
            .zip(&micro_layout)
            .map(|(node, (size, _))| NodeLayout {
                id: node.id.clone(),
                x: 0.0,
                y: 0.0,
                width: if uses_micro_layout {
                    size.width
                } else {
                    node.width
                },
                height: if uses_micro_layout {
                    size.height
                } else {
                    node.height
                },
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
    let mut size;
    let mut content_shifts = HashMap::new();
    // Ordinary directional containers retain buildSubgraphLayoutOptions' padding and node
    // spacing. Their algorithms do not inherit the root-only Rectpacking preset.
    let (container_padding, minimum, explicit) = match context {
        ScopeContext::Root => (None, None, false),
        ScopeContext::Container { node, mode } => (
            Some(provider_padding(mode.padding(node))),
            mode.minimum(node),
            matches!(mode, ContainerMode::Explicit(_)),
        ),
    };
    let minimum = effective_minimum(minimum);
    let minimum_width = minimum.width;
    let minimum_height = minimum.height;
    match graph.options.algorithm {
        Algorithm::Layered => unreachable!("layered graphs use the compound-aware pipeline"),
        Algorithm::Box | Algorithm::Rectpacking => {
            let rectangles: Vec<_> = graph
                .nodes
                .iter()
                .zip(node_contexts)
                .zip(&result.nodes)
                .map(|((node, context), measured)| box_layout::Rectangle {
                    width: measured.width,
                    height: measured.height,
                    horizontal_content_alignment: if matches!(
                        context.mode,
                        ContainerMode::Explicit(_)
                    ) {
                        box_layout::ContentAlignment::Center
                    } else {
                        box_layout::ContentAlignment::Start
                    },
                    minimum_size: context
                        .size_constraints_active
                        .then(|| context.mode.minimum(node))
                        .flatten()
                        .map(|minimum| effective_minimum(Some(minimum))),
                    micro_layout_size: (context.size_constraints_active
                        && context.mode.minimum(node).is_some())
                    .then_some(source_port::LSize {
                        width: measured.width,
                        height: measured.height,
                    }),
                    ..Default::default()
                })
                .collect();
            let packed = if graph.options.algorithm == Algorithm::Box {
                let mut options = box_layout::Options::default();
                if let Some(padding) = container_padding {
                    options.padding = padding;
                    options.spacing = 50.0;
                    options.minimum_width = minimum_width;
                    options.minimum_height = minimum_height;
                    if explicit {
                        options.aspect_ratio = 2.0;
                        options.expand_nodes = true;
                    }
                }
                box_layout::layout(&rectangles, &options, work)?
            } else {
                // render.ts RECTPACKING_OPTIONS. SCANLINE is absent from elkjs 0.9.3's
                // enum, so it resolves to GREEDY, the kernel's implemented default.
                let options = if !explicit && let Some(padding) = container_padding {
                    rectpacking::Options {
                        padding,
                        spacing: 50.0,
                        minimum_width,
                        minimum_height,
                        ..Default::default()
                    }
                } else {
                    rectpacking::Options {
                        aspect_ratio: 1.6,
                        try_box: true,
                        expand_nodes: true,
                        row_height_reevaluation: true,
                        compaction_iterations: 10,
                        eliminate_whitespace: true,
                        horizontal_content_alignment: box_layout::ContentAlignment::Center,
                        padding: container_padding.unwrap_or_default(),
                        minimum_width,
                        minimum_height,
                        ..Default::default()
                    }
                };
                rectpacking::layout(&rectangles, &options, work)?
            };
            size = source_port::LSize {
                width: packed.width,
                height: packed.height,
            };
            for (node, position) in result.nodes.iter_mut().zip(packed.rectangles) {
                if position.content_shift_x != 0.0 || position.content_shift_y != 0.0 {
                    content_shifts.insert(
                        node.id.clone(),
                        Point {
                            x: position.content_shift_x,
                            y: position.content_shift_y,
                        },
                    );
                }
                node.width = position.width;
                node.height = position.height;
                node.x = position.x + position.width / 2.0;
                node.y = position.y + position.height / 2.0;
            }
        }
        Algorithm::Force | Algorithm::Stress => {
            let nodes: Vec<_> = result
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
            let mut options = force::Options {
                resolved_seed: algorithms::resolve_seed(
                    graph.options.layered.random_seed,
                    operation_seed,
                    scope,
                    algorithms::RandomDomain::Force,
                )?,
                ..Default::default()
            };
            if let Some(padding) = container_padding {
                options.padding = padding;
                options.spacing = 50.0;
                if explicit {
                    options.aspect_ratio = 2.0;
                }
            }
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
            size = source_port::LSize {
                width: placed.width,
                height: placed.height,
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
                .zip(node_contexts)
                .zip(&result.nodes)
                .map(|((node, context), measured)| mrtree::Node {
                    width: measured.width,
                    height: measured.height,
                    // Mermaid leaf nodes have no ELK node-label array. MrTree derives identity
                    // from its local ordinal when the first label text is absent.
                    label: if node.kind == NodeKind::Group {
                        node.label_text.clone().unwrap_or_default()
                    } else {
                        String::new()
                    },
                    padding: if node.kind == NodeKind::Group {
                        provider_padding(context.mode.padding(node))
                    } else {
                        mrtree::Node::default().padding
                    },
                    ..Default::default()
                })
                .collect();
            let edges: Vec<_> = endpoints
                .iter()
                .map(|&(source, target)| mrtree::Edge { source, target })
                .collect();
            let mut options = mrtree::Options {
                ..Default::default()
            };
            if let Some(padding) = container_padding {
                options.padding = padding;
                options.spacing = 50.0;
                if explicit {
                    options.aspect_ratio = 2.0;
                }
            }
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
                    ..options
                },
                work,
            )?;
            size = source_port::LSize {
                width: placed.width,
                height: placed.height,
            };
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
            let nodes: Vec<_> = result
                .nodes
                .iter()
                .zip(&micro_layout)
                .map(|(node, (_, margin))| radial::Node {
                    width: node.width,
                    height: node.height,
                    margins: radial::Margins {
                        top: margin.top,
                        right: margin.right,
                        bottom: margin.bottom,
                        left: margin.left,
                    },
                    ..Default::default()
                })
                .collect();
            let edges: Vec<_> = endpoints
                .iter()
                .map(|&(source, target)| radial::Edge::new(source, target))
                .collect();
            let mut options = radial::Options::default();
            if let Some(padding) = container_padding {
                options.padding = padding;
                options.spacing = 50.0;
            }
            let placed = radial::layout(&nodes, &edges, &options, work)?;
            size = source_port::LSize {
                width: placed.width,
                height: placed.height,
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
                scope,
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
            let mut options = spore_overlap::Options::default();
            if let Some(padding) = container_padding {
                options.padding = padding;
                options.spacing = 50.0;
            }
            let placed = spore_overlap::layout(&nodes, &edges, &options, &mut random, work)?;
            size = source_port::LSize {
                width: placed.width,
                height: placed.height,
            };
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
    // Force's initial export clears NODE_SIZE_CONSTRAINTS before Stress imports the graph
    // again, so the subsequent majorization export must not restore its old title minimum.
    if !matches!(
        graph.options.algorithm,
        Algorithm::Stress | Algorithm::Radial
    ) {
        size.width = size.width.max(minimum_width);
        size.height = size.height.max(minimum_height);
    }
    Ok(FlatLayout {
        layout: result,
        size,
        content_shifts,
        // CalculateGraphSize writes dimensions directly. Other providers export through
        // ElkUtil.resizeNode, which fixes the owner's size for its parent's invocation.
        size_constraints_active: graph.options.algorithm == Algorithm::Radial,
    })
}

fn provider_padding(padding: source_port::ElkPadding) -> box_layout::Padding {
    box_layout::Padding {
        top: padding.top,
        right: padding.right,
        bottom: padding.bottom,
        left: padding.left,
    }
}

fn effective_minimum(minimum: Option<source_port::LSize>) -> source_port::LSize {
    minimum.map_or(source_port::LSize::default(), |size| source_port::LSize {
        width: if size.width <= 0.0 { 20.0 } else { size.width },
        height: if size.height <= 0.0 {
            20.0
        } else {
            size.height
        },
    })
}
