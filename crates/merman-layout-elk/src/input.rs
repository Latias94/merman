//! Materialize validated adapter scopes and options for the provider importers.

use std::collections::{HashMap, HashSet};

use super::{
    Algorithm, ContainerMode, CycleBreakingStrategy, Direction, Edge, EdgeRouting, Error, Graph,
    HierarchyHandling, HierarchyIndex, Label, LayerConstraint, LayeringStrategy,
    ModelOrderStrategy, Node, NodeKind, NodePlacementAlignment, NodePlacementStrategy,
    NoopWorkControl, PortAlignment, Result, ScopeEdgeMetadata, ScopeLayout, ScopedEndpoint,
    SegmentedEdgeMetadata, SelfLoopDistributionStrategy, SelfLoopOrderingStrategy, WorkControl,
    WorkError, flat, source_port,
};
use source_port::{
    ElkDirection, ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode,
    LayeredOptions as SourceLayeredOptions, NodeLabelPlacement,
};

const MEASURED_LABEL_PLACEHOLDER: &str = "\u{fffc}";

pub(super) fn graph_to_source_input(graph: &Graph) -> Result<ElkInputGraph> {
    let index = HierarchyIndex::build(graph, &mut NoopWorkControl)?;
    if index
        .scopes
        .iter()
        .any(|scope| scope.algorithm != Algorithm::Layered)
    {
        return Err(Error::LayeredDiagnosticsRequired);
    }
    if index.scopes.len() > 1 {
        return Err(Error::SeparateScopesDiagnosticsUnsupported);
    }
    let options = layered_options_to_source(graph);
    let node_ids_with_direct_children = graph
        .nodes
        .iter()
        .filter_map(|node| node.parent.as_deref())
        .collect::<HashSet<_>>();

    Ok(ElkInputGraph {
        id: graph.id.clone(),
        options,
        nodes: graph
            .nodes
            .iter()
            .enumerate()
            .map(|(node_index, node)| ElkInputNode {
                id: node.id.clone(),
                width: if index.children[node_index].is_empty() {
                    node.width
                } else {
                    0.0
                },
                height: if index.children[node_index].is_empty() {
                    node.height
                } else {
                    0.0
                },
                parent: node.parent.clone(),
                direction: index.container_modes[node_index]
                    .direction(node)
                    .map(direction_to_source),
                hierarchy_handling: (node.kind == NodeKind::Group)
                    .then(|| hierarchy_handling_to_source(index.resolved_handling[node_index])),
                layer_constraint: node.layer_constraint.map(layer_constraint_to_source),
                port_alignment: node.port_alignment.map(port_alignment_to_source),
                port_constraints: None,
                node_label_placement: match node.kind {
                    NodeKind::Group => NodeLabelPlacement::InsideTopCenter,
                    NodeKind::Leaf => NodeLabelPlacement::Fixed,
                },
                node_flexibility: if node.kind == NodeKind::Group {
                    source_port::NodeFlexibility::PortPosition
                } else {
                    source_port::NodeFlexibility::None
                },
                ports_surrounding: (node.kind == NodeKind::Group).then_some(
                    source_port::SpacingMargin {
                        top: 12.0,
                        right: 12.0,
                        bottom: 12.0,
                        left: 12.0,
                    },
                ),
                nested_options: (node.kind == NodeKind::Group).then(|| {
                    Box::new(resolved_container_options(
                        graph,
                        node,
                        index.container_modes[node_index],
                        graph.direction,
                        graph.options.layered.hierarchy_handling,
                    ))
                }),
                // Mermaid's leaf labels are already included in the painted node dimensions.
                // Only non-empty groups expose an ELK label; retain all other measurements in
                // the adapter model for SVG rendering without creating provider label margins.
                label: if node.kind == NodeKind::Group
                    && node_ids_with_direct_children.contains(node.id.as_str())
                {
                    node.label.map(|label| {
                        ElkInputLabel::center(
                            node.label_text
                                .as_deref()
                                .unwrap_or(MEASURED_LABEL_PLACEHOLDER),
                            label.width,
                            label.height,
                        )
                    })
                } else {
                    None
                },
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .map(|edge| ElkInputEdge {
                id: edge.id.clone(),
                source: edge.source.clone(),
                target: edge.target.clone(),
                labels: source_edge_labels(edge, true, true, true),
                minlen: edge.minlen,
                inside_self_loops_yo: edge.inside_self_loops_yo,
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            })
            .collect(),
    })
}

fn source_edge_labels(edge: &Edge, center: bool, start: bool, end: bool) -> Vec<ElkInputLabel> {
    let mut labels: Vec<_> = edge
        .label
        .filter(|_| center)
        .into_iter()
        .map(|label| ElkInputLabel::center(MEASURED_LABEL_PLACEHOLDER, label.width, label.height))
        .collect();
    labels.extend(
        edge.terminal_labels
            .iter()
            .filter(|terminal| if terminal.key.at_start() { start } else { end })
            .map(|terminal| ElkInputLabel {
                source_index: Some(terminal.key.source_index()),
                text: MEASURED_LABEL_PLACEHOLDER.to_owned(),
                width: terminal.label.width,
                height: terminal.label.height,
                placement: if terminal.key.at_start() {
                    source_port::EdgeLabelPlacement::Tail
                } else {
                    source_port::EdgeLabelPlacement::Head
                },
                inline: false,
            }),
    );
    labels
}

fn apply_root_inside_top_center_label_padding(
    options: &mut SourceLayeredOptions,
    label: Label,
) -> Result<()> {
    options.include_inside_top_center_label_minimum(source_port::LSize {
        width: label.width,
        height: label.height,
    })?;
    if label.height > 0.0 {
        options.padding.top += label.height + options.node_labels_padding.top;
    }
    Ok(())
}

impl HierarchyIndex<'_> {
    pub(super) fn materialize_flat_scope(
        &self,
        scope_index: usize,
        arena: &[Option<ScopeLayout>],
        work: &mut dyn WorkControl,
    ) -> Result<(Graph, HashMap<String, ScopeEdgeMetadata>)> {
        let scope = &self.scopes[scope_index];
        // Only Layered-owned boundary segments reach this branch. Upstream Layered rejects
        // endpoints inside a scope handled by another provider; do not invent external ports.
        if let Some(edge) = scope.edges.iter().find(|edge| edge.segment.is_some()) {
            return Err(Error::UnsupportedCrossProviderEdge {
                edge_id: self.graph.edges[edge.original].id.clone(),
            });
        }
        let units = scope
            .nodes
            .len()
            .checked_add(scope.edges.len())
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.check(units)?;
        work.charge(units)?;
        let nodes = scope
            .nodes
            .iter()
            .map(|&index| {
                let mut node = self.graph.nodes[index].clone();
                node.parent = None;
                if let Some(child) = self.child_scope_by_anchor[index] {
                    let size = arena[child]
                        .as_ref()
                        .expect("children are laid out in postorder")
                        .size;
                    node.width = size.width;
                    node.height = size.height;
                } else if node.kind == NodeKind::Group {
                    // Empty groups have no ELK container options or node labels.
                    node.kind = NodeKind::Leaf;
                    node.label_text = None;
                }
                node
            })
            .collect();
        let edges = scope
            .edges
            .iter()
            .filter(|edge| !self.is_flat_hierarchy_edge(scope_index, edge))
            .map(|edge| {
                let source = &self.graph.edges[edge.original];
                // Non-layered providers never place terminal labels. Avoid cloning the
                // caller-owned terminal-label payload into a graph that cannot consume it.
                Edge {
                    id: source.id.clone(),
                    source: source.source.clone(),
                    target: source.target.clone(),
                    label: source.label,
                    terminal_labels: Vec::new(),
                    minlen: source.minlen,
                    inside_self_loops_yo: source.inside_self_loops_yo,
                }
            })
            .collect();
        let metadata = scope
            .edges
            .iter()
            .map(|edge| {
                (
                    self.graph.edges[edge.original].id.clone(),
                    ScopeEdgeMetadata {
                        original: edge.original,
                        segment: None,
                    },
                )
            })
            .collect();
        let mut options = self.graph.options.clone();
        options.algorithm = scope.algorithm;
        Ok((
            Graph {
                id: scope
                    .anchor
                    .map(|node| self.graph.nodes[node].id.clone())
                    .unwrap_or_else(|| self.graph.id.clone()),
                direction: scope.direction,
                nodes,
                edges,
                options,
                spacing: self.graph.spacing,
            },
            metadata,
        ))
    }

    pub(super) fn flat_scope_context(&self, scope: usize) -> flat::ScopeContext<'_> {
        self.scopes[scope]
            .anchor
            .map_or(flat::ScopeContext::Root, |anchor| {
                flat::ScopeContext::Container {
                    node: &self.graph.nodes[anchor],
                    mode: self.container_modes[anchor],
                }
            })
    }

    pub(super) fn flat_node_contexts(
        &self,
        scope: usize,
        arena: &[Option<ScopeLayout>],
    ) -> Vec<flat::NodeContext> {
        self.scopes[scope]
            .nodes
            .iter()
            .map(|&node| flat::NodeContext {
                mode: self.container_modes[node],
                size_constraints_active: self.child_scope_by_anchor[node]
                    .and_then(|child| arena[child].as_ref())
                    .is_some_and(|child| child.size_constraints_active),
                label_margin: self.child_scope_by_anchor[node]
                    .and_then(|child| arena[child].as_ref())
                    .map_or_else(source_port::LMargin::default, |child| child.label_margin),
            })
            .collect()
    }

    pub(super) fn materialize_scope(
        &self,
        scope_index: usize,
        arena: &[Option<ScopeLayout>],
        work_control: &mut dyn WorkControl,
    ) -> Result<(
        ElkInputGraph,
        Vec<source_port::ElkInputEdgeSegment>,
        HashMap<String, ScopeEdgeMetadata>,
    )> {
        let scope = &self.scopes[scope_index];
        let labels = scope.edges.iter().try_fold(0usize, |count, scoped| {
            count
                .checked_add(self.graph.edges[scoped.original].terminal_labels.len())
                .ok_or(WorkError::ArithmeticOverflow)
        })?;
        let materialized = scope
            .nodes
            .len()
            .checked_add(scope.edges.len())
            .and_then(|count| count.checked_add(labels))
            .ok_or(WorkError::ArithmeticOverflow)?;
        work_control.check(materialized)?;
        work_control.charge(materialized)?;

        let mut options = if let Some(anchor) = scope.anchor {
            resolved_container_options(
                self.graph,
                &self.graph.nodes[anchor],
                self.container_modes[anchor],
                scope.direction,
                scope.handling,
            )
        } else {
            layered_options_to_source_for(self.graph, scope.direction, scope.handling)
        };
        if let Some(label) = scope.anchor.and_then(|node| self.graph.nodes[node].label) {
            apply_root_inside_top_center_label_padding(&mut options, label)?;
        }

        let nodes = scope
            .nodes
            .iter()
            .map(|node_index| {
                let source = &self.graph.nodes[*node_index];
                let child_layout =
                    self.child_scope_by_anchor[*node_index].and_then(|child| arena[child].as_ref());
                let child_size = child_layout.map(|layout| layout.size);
                // Mermaid removes a non-empty group's explicit size before ELK layout. A parent
                // scope must therefore consume the completed child extent, not the input size.
                let width = child_size.map(|size| size.width).unwrap_or_else(|| {
                    if self.children[*node_index].is_empty() {
                        source.width
                    } else {
                        0.0
                    }
                });
                let height = child_size.map(|size| size.height).unwrap_or_else(|| {
                    if self.children[*node_index].is_empty() {
                        source.height
                    } else {
                        0.0
                    }
                });
                let size = if child_layout.is_some_and(|layout| layout.size_constraints_active) {
                    source_port::inside_top_center_micro_layout(
                        source_port::LSize { width, height },
                        self.container_modes[*node_index].minimum(source),
                        source.label.map(|label| source_port::LSize {
                            width: label.width,
                            height: label.height,
                        }),
                    )?
                    .0
                } else {
                    source_port::LSize { width, height }
                };
                Ok::<_, Error>(ElkInputNode {
                    id: source.id.clone(),
                    width: size.width,
                    height: size.height,
                    parent: self.parent[*node_index]
                        .filter(|parent| self.node_scope[*parent] == scope_index)
                        .map(|parent| self.graph.nodes[parent].id.clone()),
                    direction: self.container_modes[*node_index]
                        .direction(source)
                        .map(direction_to_source),
                    hierarchy_handling: (source.kind == NodeKind::Group)
                        .then(|| hierarchy_handling_to_source(self.resolved_handling[*node_index])),
                    layer_constraint: source.layer_constraint.map(layer_constraint_to_source),
                    port_alignment: source.port_alignment.map(port_alignment_to_source),
                    port_constraints: None,
                    node_label_placement: match source.kind {
                        NodeKind::Group => NodeLabelPlacement::InsideTopCenter,
                        NodeKind::Leaf => NodeLabelPlacement::Fixed,
                    },
                    node_flexibility: if source.kind == NodeKind::Group {
                        source_port::NodeFlexibility::PortPosition
                    } else {
                        source_port::NodeFlexibility::None
                    },
                    ports_surrounding: (source.kind == NodeKind::Group).then_some(
                        source_port::SpacingMargin {
                            top: 12.0,
                            right: 12.0,
                            bottom: 12.0,
                            left: 12.0,
                        },
                    ),
                    nested_options: (source.kind == NodeKind::Group).then(|| {
                        Box::new(resolved_container_options(
                            self.graph,
                            source,
                            self.container_modes[*node_index],
                            scope.direction,
                            scope.handling,
                        ))
                    }),
                    // Mermaid attaches an ELK label only when childrenById contains the group.
                    // Leaf measurements and empty-group titles are paint metadata, not margins.
                    label: if source.kind == NodeKind::Group
                        && !self.children[*node_index].is_empty()
                    {
                        source.label.map(|label| {
                            ElkInputLabel::center(
                                source
                                    .label_text
                                    .as_deref()
                                    .unwrap_or(MEASURED_LABEL_PLACEHOLDER),
                                label.width,
                                label.height,
                            )
                        })
                    } else {
                        None
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let mut edges = Vec::new();
        let mut segments = Vec::new();
        let mut edge_metadata = HashMap::new();
        let uses_edge_model_order = layout_uses_edge_model_order(self.graph);
        for (scope_edge_order, scoped) in scope.edges.iter().enumerate() {
            let source = &self.graph.edges[scoped.original];
            let model_order = uses_edge_model_order.then_some(if scoped.segment.is_some() {
                // A boundary segment inherits its original edge owner's ordinal even when this
                // particular piece is materialized in a descendant scope.
                self.edge_model_order[scoped.original]
            } else {
                // Ordinary edges follow the importer-visible order of this materialized scope.
                scope_edge_order
            });
            let input_edge = ElkInputEdge {
                id: source.id.clone(),
                source: source.source.clone(),
                target: source.target.clone(),
                labels: source_edge_labels(
                    source,
                    scoped.carries_label,
                    scoped.segment_order.is_none_or(|order| order == 0),
                    scoped
                        .segment_order
                        .is_none_or(|order| order + 1 == scoped.segment_count),
                ),
                minlen: source.minlen,
                inside_self_loops_yo: source.inside_self_loops_yo,
                model_order,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            };
            let segment_metadata = scoped.segment.map(|segment| SegmentedEdgeMetadata {
                segment,
                model_order,
                order: scoped
                    .segment_order
                    .expect("segmented edge must carry its original path order"),
                count: scoped.segment_count,
            });
            edge_metadata.insert(
                source.id.clone(),
                ScopeEdgeMetadata {
                    original: scoped.original,
                    segment: segment_metadata,
                },
            );
            if let Some(segment) = scoped.segment {
                segments.push(source_port::ElkInputEdgeSegment {
                    edge: input_edge,
                    source: self.segment_endpoint(scoped.source),
                    target: self.segment_endpoint(scoped.target),
                    segment,
                    // Model order is local to the owning scope, while external-port identity must
                    // be unique among every segment materialized in this scope. Conflating the two
                    // lets edges with different owners accidentally reuse the same hierarchy port.
                    edge_order: scope_edge_order,
                });
            } else {
                edges.push(input_edge);
            }
        }

        Ok((
            ElkInputGraph {
                id: scope
                    .anchor
                    .map(|node| self.graph.nodes[node].id.clone())
                    .unwrap_or_else(|| self.graph.id.clone()),
                options,
                nodes,
                edges,
            },
            segments,
            edge_metadata,
        ))
    }

    fn segment_endpoint(
        &self,
        endpoint: ScopedEndpoint,
    ) -> source_port::ElkInputEdgeSegmentEndpoint {
        match endpoint {
            ScopedEndpoint::Node(node) => source_port::ElkInputEdgeSegmentEndpoint::Node {
                id: self.graph.nodes[node].id.clone(),
            },
            ScopedEndpoint::ParentBoundary {
                scope,
                connects_node,
            } => {
                let anchor = self.scopes[scope]
                    .anchor
                    .expect("non-root scope boundary must have an anchor");
                source_port::ElkInputEdgeSegmentEndpoint::ParentBoundary {
                    id: self.graph.nodes[anchor].id.clone(),
                    connects_node,
                }
            }
        }
    }
}

fn layered_options_to_source(graph: &Graph) -> SourceLayeredOptions {
    layered_options_to_source_for(
        graph,
        graph.direction,
        graph.options.layered.hierarchy_handling,
    )
}

fn layered_options_to_source_for(
    graph: &Graph,
    direction: Direction,
    hierarchy_handling: HierarchyHandling,
) -> SourceLayeredOptions {
    let mut options =
        SourceLayeredOptions::mermaid_flowchart_defaults(direction_to_source(direction));
    options.random_seed = graph.options.layered.random_seed;
    options.hierarchy_handling = hierarchy_handling_to_source(hierarchy_handling);
    options.edge_routing = edge_routing_to_source(graph.options.layered.edge_routing);
    options.cycle_breaking_strategy =
        cycle_breaking_to_source(graph.options.layered.cycle_breaking);
    options.layering_strategy = match graph.options.layered.layering {
        LayeringStrategy::NetworkSimplex => source_port::LayeringStrategy::NetworkSimplex,
        LayeringStrategy::LongestPath => source_port::LayeringStrategy::LongestPath,
        LayeringStrategy::LongestPathSource => source_port::LayeringStrategy::LongestPathSource,
        LayeringStrategy::CoffmanGraham => source_port::LayeringStrategy::CoffmanGraham,
        LayeringStrategy::MinWidth => source_port::LayeringStrategy::MinWidth,
        LayeringStrategy::StretchWidth => source_port::LayeringStrategy::StretchWidth,
        LayeringStrategy::Interactive => source_port::LayeringStrategy::Interactive,
    };
    options.layering_coffman_graham_layer_bound = graph.options.layered.layering_layer_bound;
    options.node_placement_strategy =
        node_placement_to_source(graph.options.layered.node_placement);
    options.node_placement_bk_fixed_alignment =
        node_placement_alignment_to_source(graph.options.layered.node_placement_alignment);
    options.consider_model_order_strategy = if graph.options.layered.consider_model_order {
        model_order_to_source(graph.options.layered.model_order)
    } else {
        source_port::OrderingStrategy::None
    };
    options.force_node_model_order = graph.options.layered.force_node_model_order;
    options.merge_edges = graph.options.layered.merge_edges;
    options.merge_hierarchy_edges = graph.options.layered.merge_hierarchy_edges;
    options.unnecessary_bendpoints = graph.options.layered.unnecessary_bendpoints;
    options.inside_self_loops_activate = graph.options.layered.inside_self_loops_activate;
    options.self_loop_distribution =
        self_loop_distribution_to_source(graph.options.layered.self_loop_distribution);
    options.self_loop_ordering =
        self_loop_ordering_to_source(graph.options.layered.self_loop_ordering);
    set_mermaid_port_clearance(&mut options);
    options
}

fn set_mermaid_port_clearance(options: &mut SourceLayeredOptions) {
    options.spacing.ports_surrounding.top = 12.0;
    options.spacing.ports_surrounding.right = 12.0;
    options.spacing.ports_surrounding.bottom = 12.0;
    options.spacing.ports_surrounding.left = 12.0;
}

fn container_options_to_source(
    graph: &Graph,
    direction: Direction,
    handling: HierarchyHandling,
) -> SourceLayeredOptions {
    // Mermaid 12 buildSubgraphLayoutOptions. Containers get their own strategy resolution;
    // root layering, layer bounds, model-order and edge-routing settings are not inherited.
    let mut options = SourceLayeredOptions {
        direction: direction_to_source(direction),
        hierarchy_handling: hierarchy_handling_to_source(handling),
        random_seed: graph.options.layered.random_seed,
        merge_edges: graph.options.layered.merge_edges,
        cycle_breaking_strategy: cycle_breaking_to_source(graph.options.container.cycle_breaking),
        node_placement_strategy: node_placement_to_source(graph.options.container.node_placement),
        node_placement_bk_fixed_alignment: node_placement_alignment_to_source(
            graph.options.container.node_placement_alignment,
        ),
        padding: source_port::ElkPadding::uniform(24.0),
        spacing: source_port::SpacingOptions::layered_base_value(24.0),
        ..Default::default()
    };
    options.spacing.node_node = 50.0;
    options.spacing.edge_node_between_layers = 30.0;
    options.spacing.edge_edge = 20.0;
    set_mermaid_port_clearance(&mut options);
    options
}

fn resolved_container_options(
    graph: &Graph,
    node: &Node,
    mode: ContainerMode,
    direction: Direction,
    handling: HierarchyHandling,
) -> SourceLayeredOptions {
    let mut options = container_options_to_source(graph, direction, handling);
    options.padding = mode.padding(node);
    options.node_size_minimum = mode.minimum(node);
    options.node_size_include_labels = node.label.is_some();
    if matches!(mode, ContainerMode::Explicit(_)) {
        options.aspect_ratio = 2.0;
        options.horizontal_content_alignment = source_port::ContentAlignment::Center;
    }
    options
}

fn layout_uses_edge_model_order(graph: &Graph) -> bool {
    graph.options.layered.consider_model_order
        || graph.options.layered.force_node_model_order
        || matches!(
            graph.options.layered.cycle_breaking,
            CycleBreakingStrategy::ModelOrder | CycleBreakingStrategy::GreedyModelOrder
        )
}

fn direction_to_source(direction: Direction) -> ElkDirection {
    match direction {
        Direction::Left => ElkDirection::Left,
        Direction::Right => ElkDirection::Right,
        Direction::Up => ElkDirection::Up,
        Direction::Down => ElkDirection::Down,
    }
}

fn hierarchy_handling_to_source(
    hierarchy_handling: HierarchyHandling,
) -> source_port::HierarchyHandling {
    match hierarchy_handling {
        HierarchyHandling::IncludeChildren => source_port::HierarchyHandling::IncludeChildren,
        HierarchyHandling::SeparateChildren => source_port::HierarchyHandling::SeparateChildren,
    }
}

fn edge_routing_to_source(edge_routing: EdgeRouting) -> source_port::EdgeRouting {
    match edge_routing {
        EdgeRouting::Orthogonal => source_port::EdgeRouting::Orthogonal,
        EdgeRouting::Polyline => source_port::EdgeRouting::Polyline,
    }
}

fn cycle_breaking_to_source(
    cycle_breaking: CycleBreakingStrategy,
) -> source_port::CycleBreakingStrategy {
    match cycle_breaking {
        CycleBreakingStrategy::Greedy => source_port::CycleBreakingStrategy::Greedy,
        CycleBreakingStrategy::DepthFirst => source_port::CycleBreakingStrategy::DepthFirst,
        CycleBreakingStrategy::Interactive => source_port::CycleBreakingStrategy::Interactive,
        CycleBreakingStrategy::ModelOrder => source_port::CycleBreakingStrategy::ModelOrder,
        CycleBreakingStrategy::GreedyModelOrder => {
            source_port::CycleBreakingStrategy::GreedyModelOrder
        }
    }
}

fn node_placement_to_source(
    node_placement: NodePlacementStrategy,
) -> source_port::NodePlacementStrategy {
    match node_placement {
        NodePlacementStrategy::Simple => source_port::NodePlacementStrategy::Simple,
        NodePlacementStrategy::Interactive => source_port::NodePlacementStrategy::Interactive,
        NodePlacementStrategy::NetworkSimplex => source_port::NodePlacementStrategy::NetworkSimplex,
        NodePlacementStrategy::LinearSegments => source_port::NodePlacementStrategy::LinearSegments,
        NodePlacementStrategy::BrandesKoepf => source_port::NodePlacementStrategy::BrandesKoepf,
    }
}

fn node_placement_alignment_to_source(
    alignment: NodePlacementAlignment,
) -> source_port::FixedAlignment {
    match alignment {
        NodePlacementAlignment::None => source_port::FixedAlignment::None,
        NodePlacementAlignment::LeftUp => source_port::FixedAlignment::LeftUp,
        NodePlacementAlignment::RightUp => source_port::FixedAlignment::RightUp,
        NodePlacementAlignment::LeftDown => source_port::FixedAlignment::LeftDown,
        NodePlacementAlignment::RightDown => source_port::FixedAlignment::RightDown,
        NodePlacementAlignment::Balanced => source_port::FixedAlignment::Balanced,
    }
}

fn port_alignment_to_source(alignment: PortAlignment) -> source_port::PortAlignment {
    match alignment {
        PortAlignment::Distributed => source_port::PortAlignment::Distributed,
        PortAlignment::Justified => source_port::PortAlignment::Justified,
        PortAlignment::Begin => source_port::PortAlignment::Begin,
        PortAlignment::Center => source_port::PortAlignment::Center,
        PortAlignment::End => source_port::PortAlignment::End,
    }
}

fn layer_constraint_to_source(layer_constraint: LayerConstraint) -> source_port::LayerConstraint {
    match layer_constraint {
        LayerConstraint::First => source_port::LayerConstraint::First,
        LayerConstraint::FirstSeparate => source_port::LayerConstraint::FirstSeparate,
        LayerConstraint::Last => source_port::LayerConstraint::Last,
        LayerConstraint::LastSeparate => source_port::LayerConstraint::LastSeparate,
    }
}

fn model_order_to_source(model_order: ModelOrderStrategy) -> source_port::OrderingStrategy {
    match model_order {
        ModelOrderStrategy::None => source_port::OrderingStrategy::None,
        ModelOrderStrategy::NodesAndEdges => source_port::OrderingStrategy::NodesAndEdges,
        ModelOrderStrategy::PreferEdges => source_port::OrderingStrategy::PreferEdges,
        ModelOrderStrategy::PreferNodes => source_port::OrderingStrategy::PreferNodes,
    }
}

fn self_loop_distribution_to_source(
    self_loop_distribution: SelfLoopDistributionStrategy,
) -> source_port::SelfLoopDistributionStrategy {
    match self_loop_distribution {
        SelfLoopDistributionStrategy::North => source_port::SelfLoopDistributionStrategy::North,
        SelfLoopDistributionStrategy::Equally => source_port::SelfLoopDistributionStrategy::Equally,
        SelfLoopDistributionStrategy::NorthSouth => {
            source_port::SelfLoopDistributionStrategy::NorthSouth
        }
    }
}

fn self_loop_ordering_to_source(
    self_loop_ordering: SelfLoopOrderingStrategy,
) -> source_port::SelfLoopOrderingStrategy {
    match self_loop_ordering {
        SelfLoopOrderingStrategy::Stacked => source_port::SelfLoopOrderingStrategy::Stacked,
        SelfLoopOrderingStrategy::ReverseStacked => {
            source_port::SelfLoopOrderingStrategy::ReverseStacked
        }
        SelfLoopOrderingStrategy::Sequenced => source_port::SelfLoopOrderingStrategy::Sequenced,
    }
}
