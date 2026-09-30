//! Mermaid 12 container option provenance and cross-hierarchy normalization.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContainerMode {
    Ordinary,
    Explicit(Algorithm),
    Cleared,
}

impl ContainerMode {
    pub(super) fn direction(self, node: &Node) -> Option<Direction> {
        (self == Self::Ordinary).then_some(node.direction).flatten()
    }

    pub(super) fn padding(self, node: &Node) -> source_port::ElkPadding {
        match self {
            Self::Ordinary => source_port::ElkPadding::uniform(24.0),
            Self::Cleared => source_port::ElkPadding::uniform(12.0),
            Self::Explicit(algorithm) => {
                let padding = if algorithm == Algorithm::Rectpacking {
                    10.0
                } else {
                    15.0
                };
                source_port::ElkPadding {
                    top: node.label.map_or(0.0, |label| label.height) + padding,
                    ..source_port::ElkPadding::uniform(padding)
                }
            }
        }
    }

    pub(super) fn minimum(self, node: &Node) -> Option<source_port::LSize> {
        let label = match (self, node.label) {
            (_, Some(label)) => label,
            (Self::Explicit(_), None) => Label {
                width: 0.0,
                height: 0.0,
            },
            _ => return None,
        };
        Some(match self {
            Self::Ordinary | Self::Cleared => source_port::LSize {
                width: label.width + node.container.padding,
                height: 0.0,
            },
            Self::Explicit(algorithm) => source_port::LSize {
                width: label.width + 2.0 * node.container.padding,
                height: label.height
                    + if algorithm == Algorithm::Rectpacking {
                        20.0
                    } else {
                        30.0
                    },
            },
        })
    }
}

/// Only valid metadata algorithms trigger this additional normalization. Ordinary callers can
/// still request explicit raw hierarchy scopes; Flowchart already supplies their include policy.
pub(super) fn resolve(
    graph: &Graph,
    parent: &[Option<usize>],
    children: &[Vec<usize>],
    node_by_id: &HashMap<&str, usize>,
    work: &mut dyn WorkControl,
) -> Result<(Vec<ContainerMode>, Vec<bool>)> {
    let mut modes: Vec<_> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            if node.kind == NodeKind::Group && !children[index].is_empty() {
                node.container
                    .algorithm
                    .map_or(ContainerMode::Ordinary, ContainerMode::Explicit)
            } else {
                ContainerMode::Ordinary
            }
        })
        .collect();
    let mut include = vec![false; graph.nodes.len()];
    for (&mode, node) in modes.iter().zip(&graph.nodes) {
        if mode
            .minimum(node)
            .is_some_and(|size| !size.width.is_finite() || !size.height.is_finite())
        {
            return Err(source_port::NodeSizeError.into());
        }
    }
    if !modes
        .iter()
        .any(|mode| matches!(mode, ContainerMode::Explicit(_)))
    {
        return Ok((modes, include));
    }
    let mut source_chain = HashSet::new();
    for edge in &graph.edges {
        let endpoint = |id: &str| {
            node_by_id.get(id).copied().ok_or_else(|| {
                Error::SourceImport(source_port::ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: id.to_owned(),
                })
            })
        };
        let source = endpoint(&edge.source)?;
        let target = endpoint(&edge.target)?;
        if parent[source] == parent[target] {
            continue;
        }
        source_chain.clear();
        let mut current = Some(source);
        while let Some(node) = current {
            work.check(1)?;
            work.charge(1)?;
            source_chain.insert(node);
            current = parent[node];
        }
        let mut ancestor = Some(target);
        while let Some(node) = ancestor {
            work.check(1)?;
            work.charge(1)?;
            if source_chain.contains(&node) {
                break;
            }
            ancestor = parent[node];
        }
        for endpoint in [source, target] {
            let mut current = Some(endpoint);
            while let Some(node) = current {
                work.check(1)?;
                work.charge(1)?;
                include[node] = true;
                if matches!(modes[node], ContainerMode::Explicit(_)) {
                    modes[node] = ContainerMode::Cleared;
                }
                if Some(node) == ancestor {
                    break;
                }
                current = parent[node];
            }
        }
    }
    Ok((modes, include))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> Graph {
        Graph {
            id: "root".into(),
            direction: Direction::Down,
            nodes: [
                ("g", None),
                ("a", Some("g")),
                ("b", Some("g")),
                ("outside", None),
            ]
            .into_iter()
            .map(|(id, parent)| Node {
                id: id.into(),
                kind: if id == "g" {
                    NodeKind::Group
                } else {
                    NodeKind::Leaf
                },
                container: if id == "g" {
                    ContainerNodeOptions {
                        algorithm: Some(Algorithm::Rectpacking),
                        padding: 15.0,
                    }
                } else {
                    Default::default()
                },
                label_text: None,
                width: 40.0,
                height: 20.0,
                parent: parent.map(str::to_owned),
                direction: (id == "g").then_some(Direction::Up),
                hierarchy_handling: None,
                layer_constraint: None,
                port_alignment: None,
                label: (id == "g").then_some(Label {
                    width: 30.0,
                    height: 10.0,
                }),
            })
            .collect(),
            edges: vec![Edge {
                id: "cross".into(),
                source: "a".into(),
                target: "outside".into(),
                label: None,
                minlen: 1,
                inside_self_loops_yo: false,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn cross_boundary_clears_metadata_without_reactivating_direction_or_padding() {
        let graph = graph();
        let index = HierarchyIndex::build(&graph, &mut NoopWorkControl).unwrap();
        assert_eq!(index.scopes.len(), 1);
        assert_eq!(index.container_modes[0], ContainerMode::Cleared);
        let (input, _, _) = index
            .materialize_scope(0, &[None], &mut NoopWorkControl)
            .unwrap();
        let group = &input.nodes[0];
        assert_eq!(group.direction, None);
        assert_eq!(group.width, 0.0);
        assert_eq!(group.height, 0.0);
        let options = group.nested_options.as_ref().unwrap();
        assert_eq!(options.padding, source_port::ElkPadding::uniform(12.0));
        assert_eq!(options.spacing.node_node, 50.0);
        assert_eq!(
            options.node_size_minimum,
            Some(source_port::LSize {
                width: 45.0,
                height: 0.0
            })
        );
        assert_eq!(
            options.horizontal_content_alignment,
            source_port::ContentAlignment::Start
        );
        let output = layout(&graph).unwrap();
        assert_eq!(output.edges.len(), 1);
        assert!(!output.edges[0].points.is_empty());
        // The diagnostic path uses the same normalization, so a cleared non-layered choice
        // can be inspected as the resulting Layered graph.
        assert!(SourcePhaseDiagnostics::from_graph(&graph).is_ok());
    }

    #[test]
    fn internal_edges_preserve_metadata_and_same_parent_group_edges_do_not_clear_it() {
        let mut graph = graph();
        for (source, target) in [("a", "b"), ("g", "outside")] {
            graph.edges[0].source = source.into();
            graph.edges[0].target = target.into();
            let index = HierarchyIndex::build(&graph, &mut NoopWorkControl).unwrap();
            assert_eq!(index.scopes.len(), 2);
            assert_eq!(index.scopes[1].algorithm, Algorithm::Rectpacking);
            assert_eq!(index.scopes[1].direction, Direction::Right);
            assert!(matches!(
                SourcePhaseDiagnostics::from_graph(&graph),
                Err(Error::LayeredDiagnosticsRequired)
            ));
        }
        graph.edges[0].source = "g".into();
        graph.edges[0].target = "a".into();
        let index = HierarchyIndex::build(&graph, &mut NoopWorkControl).unwrap();
        assert_eq!(index.container_modes[0], ContainerMode::Cleared);

        graph.edges[0].source = "a".into();
        graph.edges[0].target = "b".into();
        graph.nodes[0].container.algorithm = Some(Algorithm::Layered);
        assert!(matches!(
            SourcePhaseDiagnostics::from_graph(&graph),
            Err(Error::SeparateScopesDiagnosticsUnsupported)
        ));
        assert_eq!(layout(&graph).unwrap().nodes.len(), 4);
    }

    #[test]
    fn direction_only_container_keeps_its_direction_when_crossed() {
        let mut graph = graph();
        graph.nodes[0].container.algorithm = None;
        graph.nodes[0].hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let index = HierarchyIndex::build(&graph, &mut NoopWorkControl).unwrap();
        assert_eq!(
            index.container_modes[0].direction(&graph.nodes[0]),
            Some(Direction::Up)
        );
        assert_eq!(index.scopes.len(), 1);
    }
}
