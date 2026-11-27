//! Scope ownership, boundary segments and importer order for the adapter graph.

use std::collections::{HashMap, HashSet, VecDeque};

use super::{
    Algorithm, ContainerMode, Direction, Error, Graph, HierarchyHandling, HierarchyIndex, NodeKind,
    Result, ScopePlan, ScopedEdge, ScopedEndpoint, WorkControl, WorkError, container, source_port,
};

type ScopeEdgePiece = (
    usize,
    ScopedEndpoint,
    ScopedEndpoint,
    source_port::CompoundEdgeSegment,
);

impl<'a> HierarchyIndex<'a> {
    pub(super) fn build(graph: &'a Graph, work_control: &mut dyn WorkControl) -> Result<Self> {
        let unique_items = graph
            .nodes
            .len()
            .checked_add(graph.edges.len())
            .ok_or(WorkError::ArithmeticOverflow)?;
        work_control.check(unique_items)?;
        work_control.charge(unique_items)?;

        let mut node_by_id = HashMap::with_capacity(graph.nodes.len());
        for (index, node) in graph.nodes.iter().enumerate() {
            if node_by_id.insert(node.id.as_str(), index).is_some() {
                return Err(source_port::ImportError::DuplicateNode {
                    id: node.id.clone(),
                }
                .into());
            }
        }

        // FlowDB normalizes every edge ID before invoking Mermaid's ELK adapter: duplicate
        // user-provided IDs receive unique generated identities. Scope metadata may key by that
        // identity, but non-canonical duplicate input must not silently alias original edges.
        work_control.check(graph.edges.len())?;
        work_control.charge(graph.edges.len())?;
        let mut edge_ids = HashSet::with_capacity(graph.edges.len());
        for edge in &graph.edges {
            if !edge_ids.insert(edge.id.as_str()) {
                return Err(source_port::ImportError::DuplicateEdge {
                    id: edge.id.clone(),
                }
                .into());
            }
        }

        let mut parent = vec![None; graph.nodes.len()];
        let mut children = vec![Vec::new(); graph.nodes.len()];
        let mut roots = Vec::new();
        for (index, node) in graph.nodes.iter().enumerate() {
            if let Some(parent_id) = node.parent.as_deref() {
                let Some(&parent_index) = node_by_id.get(parent_id) else {
                    return Err(source_port::ImportError::MissingParent {
                        node_id: node.id.clone(),
                        parent_id: parent_id.to_string(),
                    }
                    .into());
                };
                parent[index] = Some(parent_index);
                children[parent_index].push(index);
            } else {
                roots.push(index);
            }
        }
        detect_parent_cycles(graph, &parent)?;

        let (container_modes, include_children) =
            container::resolve(graph, &parent, &children, &node_by_id, work_control)?;

        work_control.check(1)?;
        work_control.charge(1)?;
        let mut scopes = vec![ScopePlan {
            algorithm: graph.options.algorithm,
            parent: None,
            anchor: None,
            depth: 0,
            seed_scope: source_port::GraphSeedScope::root(graph.id.as_str()),
            direction: graph.direction,
            handling: graph.options.layered.hierarchy_handling,
            nodes: Vec::new(),
            children: Vec::new(),
            edges: Vec::new(),
            owned_edges: Vec::new(),
        }];
        let mut node_scope = vec![0usize; graph.nodes.len()];
        let mut resolved_handling =
            vec![graph.options.layered.hierarchy_handling; graph.nodes.len()];
        let mut child_scope_by_anchor = vec![None; graph.nodes.len()];
        let mut hierarchy_preorder = Vec::with_capacity(graph.nodes.len());
        let mut preorder_position = vec![0usize; graph.nodes.len()];
        let mut search = roots
            .into_iter()
            .rev()
            .map(|node| {
                (
                    node,
                    0usize,
                    graph.options.layered.hierarchy_handling,
                    graph.direction,
                )
            })
            .collect::<Vec<_>>();
        while let Some((node_index, scope, parent_handling, parent_direction)) = search.pop() {
            let node = &graph.nodes[node_index];
            let mode = container_modes[node_index];
            let handling = if node.kind == NodeKind::Group {
                if include_children[node_index] {
                    HierarchyHandling::IncludeChildren
                } else if matches!(mode, ContainerMode::Explicit(_)) {
                    HierarchyHandling::SeparateChildren
                } else {
                    node.hierarchy_handling.unwrap_or(parent_handling)
                }
            } else {
                parent_handling
            };
            let direction = mode.direction(node).unwrap_or(parent_direction);
            // Unconfigured containers resolve to ELK's default provider, not their parent's
            // non-layered provider. A direction option explicitly selects the diagram provider.
            let algorithm = if let ContainerMode::Explicit(algorithm) = mode {
                algorithm
            } else if mode.direction(node).is_some() {
                graph.options.algorithm
            } else {
                Algorithm::Layered
            };
            resolved_handling[node_index] = handling;
            node_scope[node_index] = scope;
            preorder_position[node_index] = hierarchy_preorder.len();
            hierarchy_preorder.push(node_index);

            let separates = node.kind == NodeKind::Group
                && !children[node_index].is_empty()
                && (parent_handling == HierarchyHandling::SeparateChildren
                    || handling == HierarchyHandling::SeparateChildren
                    || scopes[scope].algorithm != Algorithm::Layered
                    || algorithm != scopes[scope].algorithm);
            let child_scope = if separates {
                work_control.check(1)?;
                work_control.charge(1)?;
                let child_scope = scopes.len();
                let seed_scope = scopes[scope].seed_scope.child(node.id.as_str());
                scopes.push(ScopePlan {
                    algorithm,
                    parent: Some(scope),
                    anchor: Some(node_index),
                    depth: scopes[scope].depth + 1,
                    seed_scope,
                    direction: mode
                        .direction(node)
                        .unwrap_or(if algorithm == Algorithm::MrTree {
                            Direction::Down
                        } else {
                            Direction::Right
                        }),
                    handling,
                    nodes: Vec::new(),
                    children: Vec::new(),
                    edges: Vec::new(),
                    owned_edges: Vec::new(),
                });
                scopes[scope].children.push(child_scope);
                child_scope_by_anchor[node_index] = Some(child_scope);
                child_scope
            } else {
                scope
            };
            search.extend(children[node_index].iter().rev().map(|child| {
                (
                    *child,
                    child_scope,
                    handling,
                    if separates {
                        scopes[child_scope].direction
                    } else {
                        direction
                    },
                )
            }));
        }
        for node in 0..graph.nodes.len() {
            scopes[node_scope[node]].nodes.push(node);
        }

        let mut edge_owner_scope = vec![0usize; graph.edges.len()];
        let mut edge_model_order = vec![0usize; graph.edges.len()];
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let Some(&source) = node_by_id.get(edge.source.as_str()) else {
                return Err(source_port::ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.source.clone(),
                }
                .into());
            };
            let Some(&target) = node_by_id.get(edge.target.as_str()) else {
                return Err(source_port::ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.target.clone(),
                }
                .into());
            };
            let source_scope = node_scope[source];
            let target_scope = node_scope[target];
            if source_scope == target_scope {
                let owner = source_scope;
                edge_owner_scope[edge_index] = owner;
                edge_model_order[edge_index] = scopes[owner].owned_edges.len();
                scopes[owner].owned_edges.push(edge_index);
                scopes[owner].edges.push(ScopedEdge {
                    original: edge_index,
                    source: ScopedEndpoint::Node(source),
                    target: ScopedEndpoint::Node(target),
                    segment: None,
                    segment_order: None,
                    segment_count: 0,
                    carries_label: true,
                });
                continue;
            }

            // Mermaid owns a cross-scope edge at the endpoint-inclusive common ancestor. Build
            // that owner and ELK's required boundary sections in one parent-chain pass.
            let (owner, mut pieces) = scope_edge_segments(
                &scopes,
                source,
                target,
                source_scope,
                target_scope,
                work_control,
            )?;
            edge_owner_scope[edge_index] = owner;
            edge_model_order[edge_index] = scopes[owner].owned_edges.len();
            scopes[owner].owned_edges.push(edge_index);
            if scopes[owner].algorithm == Algorithm::Radial {
                // Radial uses hierarchical incoming edges to select its root and may traverse
                // successors in other scopes. The flat kernel cannot represent that node domain;
                // keep this raw-adapter combination unsupported instead of silently changing roots.
                return Err(Error::UnsupportedCrossProviderEdge {
                    edge_id: edge.id.clone(),
                });
            }
            if scopes[owner].algorithm != Algorithm::Layered {
                // Force/MrTree importers exclude hierarchical edges; packing providers never
                // route them. SPOrE exports the original contained edge after node placement.
                // None of these providers imports Layered's synthetic boundary segments.
                let containing_scope = pieces
                    .iter()
                    .find_map(|(scope, source, target, _)| {
                        [source, target]
                            .into_iter()
                            .any(|endpoint| {
                                matches!(
                                    endpoint,
                                    ScopedEndpoint::ParentBoundary {
                                        connects_node: true,
                                        ..
                                    }
                                )
                            })
                            .then_some(*scope)
                    })
                    .unwrap_or(owner);
                if scopes[containing_scope].algorithm == Algorithm::Layered {
                    // ELK Layered cannot import a parent endpoint belonging to another
                    // provider's scope (as opposed to an ordinary descendant-to-peer edge).
                    return Err(Error::UnsupportedCrossProviderEdge {
                        edge_id: edge.id.clone(),
                    });
                }
                scopes[containing_scope].edges.push(ScopedEdge {
                    original: edge_index,
                    source: ScopedEndpoint::Node(source),
                    target: ScopedEndpoint::Node(target),
                    segment: None,
                    segment_order: None,
                    segment_count: 0,
                    carries_label: true,
                });
                continue;
            }
            let label_piece = edge.label.map(|_| compound_center_segment(&pieces));
            let segment_count = pieces.len();
            for (piece_index, (scope, source, target, segment)) in pieces.drain(..).enumerate() {
                scopes[scope].edges.push(ScopedEdge {
                    original: edge_index,
                    source,
                    target,
                    segment: Some(segment),
                    segment_order: Some(piece_index),
                    segment_count,
                    carries_label: label_piece == Some(piece_index),
                });
            }
        }

        let include_order_work = scopes
            .iter()
            .filter(|scope| {
                scope.handling == HierarchyHandling::IncludeChildren && scope.edges.len() >= 2
            })
            .try_fold(graph.nodes.len(), |work, scope| {
                let edge_work = scope
                    .edges
                    .len()
                    .checked_mul(3)
                    .ok_or(WorkError::ArithmeticOverflow)?;
                work.checked_add(scope.nodes.len())
                    .and_then(|work| work.checked_add(scope.owned_edges.len()))
                    .and_then(|work| work.checked_add(edge_work))
                    .ok_or(WorkError::ArithmeticOverflow)
            })?;
        if scopes.iter().any(|scope| {
            scope.handling == HierarchyHandling::IncludeChildren && scope.edges.len() >= 2
        }) {
            work_control.check(include_order_work)?;
            work_control.charge(include_order_work)?;

            let mut subtree_size = vec![1usize; graph.nodes.len()];
            for node in hierarchy_preorder.iter().rev().copied() {
                if let Some(parent) = parent[node] {
                    subtree_size[parent] = subtree_size[parent]
                        .checked_add(subtree_size[node])
                        .ok_or(WorkError::ArithmeticOverflow)?;
                }
            }
            for (scope_index, scope) in scopes.iter_mut().enumerate() {
                if scope.handling != HierarchyHandling::IncludeChildren || scope.edges.len() < 2 {
                    continue;
                }
                order_scope_edges_like_elk_importer(
                    scope,
                    scope_index,
                    &parent,
                    &children,
                    &node_scope,
                    &resolved_handling,
                    &preorder_position,
                    &subtree_size,
                );
                let previous_owned = std::mem::take(&mut scope.owned_edges);
                for &original in &previous_owned {
                    edge_model_order[original] = usize::MAX;
                }
                for scoped in &scope.edges {
                    if edge_owner_scope[scoped.original] == scope_index
                        && edge_model_order[scoped.original] == usize::MAX
                    {
                        edge_model_order[scoped.original] = scope.owned_edges.len();
                        scope.owned_edges.push(scoped.original);
                    }
                }
                for original in previous_owned {
                    if edge_model_order[original] == usize::MAX {
                        edge_model_order[original] = scope.owned_edges.len();
                        scope.owned_edges.push(original);
                    }
                }
            }
        }

        let mut postorder = Vec::with_capacity(scopes.len());
        let mut frames = vec![(0usize, false)];
        while let Some((scope, expanded)) = frames.pop() {
            if expanded {
                postorder.push(scope);
                continue;
            }
            frames.push((scope, true));
            frames.extend(
                scopes[scope]
                    .children
                    .iter()
                    .rev()
                    .map(|child| (*child, false)),
            );
        }

        Ok(Self {
            graph,
            parent,
            children,
            container_modes,
            resolved_handling,
            node_scope,
            edge_model_order,
            child_scope_by_anchor,
            scopes,
            postorder,
        })
    }

    pub(super) fn is_flat_hierarchy_edge(&self, scope: usize, edge: &ScopedEdge) -> bool {
        matches!((edge.source, edge.target), (ScopedEndpoint::Node(source), ScopedEndpoint::Node(target))
            if self.node_scope[source] != scope || self.node_scope[target] != scope)
    }
}

#[allow(clippy::too_many_arguments)]
fn order_scope_edges_like_elk_importer(
    scope: &mut ScopePlan,
    scope_index: usize,
    parent: &[Option<usize>],
    children: &[Vec<usize>],
    node_scope: &[usize],
    resolved_handling: &[HierarchyHandling],
    preorder_position: &[usize],
    subtree_size: &[usize],
) {
    if scope.edges.len() < 2 {
        return;
    }

    let local_parent =
        |node: usize| parent[node].filter(|parent| node_scope[*parent] == scope_index);
    let is_ancestor = |ancestor: usize, node: usize| {
        let start = preorder_position[ancestor];
        let end = start + subtree_size[ancestor];
        start <= preorder_position[node] && preorder_position[node] < end
    };
    let containing_parent = |edge: &ScopedEdge| {
        let (ScopedEndpoint::Node(source), ScopedEndpoint::Node(target)) =
            (edge.source, edge.target)
        else {
            return None;
        };
        let source_parent = local_parent(source);
        let target_parent = local_parent(target);
        if source_parent == target_parent {
            source_parent
        } else if is_ancestor(source, target) {
            Some(source)
        } else if is_ancestor(target, source) {
            Some(target)
        } else {
            None
        }
    };

    let mut edges_by_parent: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
    for (edge, scoped) in scope.edges.iter().enumerate() {
        edges_by_parent
            .entry(containing_parent(scoped))
            .or_default()
            .push(edge);
    }

    // ElkGraphImporter visits the root graph first, then IncludeChildren graphs in stable BFS
    // child order. Preserve that model-order stream before scope-local kernel import.
    let mut order = Vec::with_capacity(scope.edges.len());
    let mut graph_queue = VecDeque::from([None]);
    while let Some(graph_parent) = graph_queue.pop_front() {
        if let Some(edges) = edges_by_parent.remove(&graph_parent) {
            order.extend(edges);
        }
        let graph_children: &[usize] = match graph_parent {
            Some(parent) => &children[parent],
            None => &scope.nodes,
        };
        graph_queue.extend(
            graph_children
                .iter()
                .copied()
                .filter(|child| {
                    node_scope[*child] == scope_index
                        && local_parent(*child) == graph_parent
                        && resolved_handling[*child] == HierarchyHandling::IncludeChildren
                })
                .map(Some),
        );
    }

    let mut slots = std::mem::take(&mut scope.edges)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    scope.edges.reserve(slots.len());
    for edge in order {
        scope.edges.push(
            slots[edge]
                .take()
                .expect("ELK scope edge order must visit each edge at most once"),
        );
    }
    scope.edges.extend(slots.into_iter().flatten());
}

pub(super) fn detect_parent_cycles(graph: &Graph, parent: &[Option<usize>]) -> Result<()> {
    let mut state = vec![0u8; graph.nodes.len()];
    for start in 0..graph.nodes.len() {
        if state[start] == 2 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(node) = current {
            match state[node] {
                2 => break,
                1 => {
                    return Err(source_port::ImportError::ParentCycle {
                        node_id: graph.nodes[node].id.clone(),
                    }
                    .into());
                }
                _ => {
                    state[node] = 1;
                    path.push(node);
                    current = parent[node];
                }
            }
        }
        for node in path {
            state[node] = 2;
        }
    }
    Ok(())
}

fn scope_edge_segments(
    scopes: &[ScopePlan],
    source: usize,
    target: usize,
    source_scope: usize,
    target_scope: usize,
    work_control: &mut dyn WorkControl,
) -> Result<(usize, Vec<ScopeEdgePiece>)> {
    let mut pieces = Vec::new();
    let mut target_pieces = Vec::new();
    let mut current_source_scope = source_scope;
    let mut current_target_scope = target_scope;
    let mut current_source = ScopedEndpoint::Node(source);
    let mut current_target = ScopedEndpoint::Node(target);

    while scopes[current_source_scope].depth > scopes[current_target_scope].depth {
        ascend_source_scope(
            scopes,
            target,
            &mut current_source_scope,
            &mut current_source,
            &mut pieces,
            work_control,
        )?;
    }
    while scopes[current_target_scope].depth > scopes[current_source_scope].depth {
        ascend_target_scope(
            scopes,
            source,
            &mut current_target_scope,
            &mut current_target,
            &mut target_pieces,
            work_control,
        )?;
    }
    while current_source_scope != current_target_scope {
        ascend_source_scope(
            scopes,
            target,
            &mut current_source_scope,
            &mut current_source,
            &mut pieces,
            work_control,
        )?;
        ascend_target_scope(
            scopes,
            source,
            &mut current_target_scope,
            &mut current_target,
            &mut target_pieces,
            work_control,
        )?;
    }
    let owner = current_source_scope;

    if !matches!(
        (current_source, current_target),
        (ScopedEndpoint::Node(left), ScopedEndpoint::Node(right)) if left == right
    ) {
        work_control.check(1)?;
        work_control.charge(1)?;
        pieces.push((
            owner,
            current_source,
            current_target,
            if source_scope != owner {
                source_port::CompoundEdgeSegment::Output {
                    depth: scopes[owner].depth,
                }
            } else {
                source_port::CompoundEdgeSegment::Input {
                    depth: scopes[owner].depth,
                }
            },
        ));
    }
    pieces.extend(target_pieces.into_iter().rev());
    Ok((owner, pieces))
}

fn ascend_source_scope(
    scopes: &[ScopePlan],
    target: usize,
    current_scope: &mut usize,
    current_source: &mut ScopedEndpoint,
    pieces: &mut Vec<ScopeEdgePiece>,
    work_control: &mut dyn WorkControl,
) -> Result<()> {
    let anchor = scopes[*current_scope]
        .anchor
        .expect("non-root scope must have an anchor");
    work_control.check(1)?;
    work_control.charge(1)?;
    pieces.push((
        *current_scope,
        *current_source,
        ScopedEndpoint::ParentBoundary {
            scope: *current_scope,
            connects_node: target == anchor,
        },
        source_port::CompoundEdgeSegment::Output {
            depth: scopes[*current_scope].depth,
        },
    ));
    *current_source = ScopedEndpoint::Node(anchor);
    *current_scope = scopes[*current_scope]
        .parent
        .expect("source scope path must reach owner");
    Ok(())
}

fn ascend_target_scope(
    scopes: &[ScopePlan],
    source: usize,
    current_scope: &mut usize,
    current_target: &mut ScopedEndpoint,
    pieces: &mut Vec<ScopeEdgePiece>,
    work_control: &mut dyn WorkControl,
) -> Result<()> {
    let anchor = scopes[*current_scope]
        .anchor
        .expect("non-root scope must have an anchor");
    work_control.check(1)?;
    work_control.charge(1)?;
    pieces.push((
        *current_scope,
        ScopedEndpoint::ParentBoundary {
            scope: *current_scope,
            connects_node: source == anchor,
        },
        *current_target,
        source_port::CompoundEdgeSegment::Input {
            depth: scopes[*current_scope].depth,
        },
    ));
    *current_target = ScopedEndpoint::Node(anchor);
    *current_scope = scopes[*current_scope]
        .parent
        .expect("target scope path must reach owner");
    Ok(())
}

fn compound_center_segment(pieces: &[ScopeEdgePiece]) -> usize {
    pieces
        .iter()
        .position(|(_, _, _, segment)| {
            matches!(segment, source_port::CompoundEdgeSegment::Input { .. })
        })
        .map(|index| index.saturating_sub(1))
        .or_else(|| pieces.len().checked_sub(1))
        .unwrap_or(0)
}
