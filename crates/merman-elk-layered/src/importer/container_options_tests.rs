use super::*;

fn node(
    id: &str,
    parent: Option<&str>,
    hierarchy_handling: Option<HierarchyHandling>,
) -> ElkInputNode {
    ElkInputNode {
        id: id.to_string(),
        width: 80.0,
        height: 40.0,
        parent: parent.map(str::to_string),
        direction: None,
        hierarchy_handling,
        layer_constraint: None,
        port_constraints: None,
        port_alignment: None,
        node_label_placement: NodeLabelPlacement::Fixed,
        node_flexibility: crate::options::NodeFlexibility::None,
        ports_surrounding: None,
        nested_options: None,
        label: None,
    }
}

fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
    ElkInputEdge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        labels: Vec::new(),
        minlen: 1,
        inside_self_loops_yo: false,
        model_order: None,
        priority_direction: 0,
        priority_shortness: 0,
        priority_straightness: 0,
    }
}

fn inside_loop(id: &str) -> ElkInputEdge {
    ElkInputEdge {
        inside_self_loops_yo: true,
        ..edge(format!("{id}-loop").as_str(), id, id)
    }
}

fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> ElkInputGraph {
    ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Down,
            hierarchy_handling: HierarchyHandling::IncludeChildren,
            random_seed: 17,
            ..Default::default()
        },
        nodes,
        edges,
    }
}

// Retain the former all-node traversal and parent-value stack independently of InputIndex and
// its option/hierarchy helpers. The oracle deliberately computes and stores leaf options too.
fn former_full_node_options(
    input: &ElkInputGraph,
) -> (LayeredOptions, HashMap<&str, LayeredOptions>) {
    let mut root = input.options.clone();
    if root.direction == ElkDirection::Undefined {
        root.direction = ElkDirection::Right;
    }
    if root.hierarchy_handling == HierarchyHandling::Inherit {
        root.hierarchy_handling = HierarchyHandling::SeparateChildren;
    }
    let mut children: HashMap<Option<&str>, Vec<&ElkInputNode>> = HashMap::new();
    for node in &input.nodes {
        children
            .entry(node.parent.as_deref())
            .or_default()
            .push(node);
    }
    let mut pending = children
        .get(&None)
        .into_iter()
        .flat_map(|nodes| nodes.iter().rev().copied())
        .map(|node| (node, root.clone()))
        .collect::<Vec<_>>();
    let mut resolved = HashMap::with_capacity(input.nodes.len());
    while let Some((node, parent)) = pending.pop() {
        let mut own = node.nested_options.as_deref().cloned().unwrap_or_default();
        own.direction = parent.direction;
        own.hierarchy_handling = match node.hierarchy_handling {
            None | Some(HierarchyHandling::Inherit) => parent.hierarchy_handling,
            Some(handling) => handling,
        };
        own.port_constraints = node.port_constraints.unwrap_or(PortConstraints::Free);
        resolved.insert(node.id.as_str(), own.clone());
        if let Some(descendants) = children.get(&Some(node.id.as_str())) {
            pending.extend(descendants.iter().rev().map(|child| (*child, own.clone())));
        }
    }
    (root, resolved)
}

fn varied_options(seed: i32) -> LayeredOptions {
    LayeredOptions {
        direction: ElkDirection::Left,
        hierarchy_handling: HierarchyHandling::SeparateChildren,
        separate_connected_components: false,
        aspect_ratio: 2.75,
        inside_self_loops_activate: true,
        padding: ElkPadding::uniform(f64::from(seed)),
        node_labels_padding: ElkPadding::uniform(3.0),
        node_size_minimum: Some(LSize {
            width: 123.0,
            height: 67.0,
        }),
        node_size_default_minimum: false,
        spacing: crate::options::SpacingOptions::layered_base_value(f64::from(seed)),
        cycle_breaking_strategy: CycleBreakingStrategy::GreedyModelOrder,
        layering_strategy: LayeringStrategy::DepthFirstModelOrder,
        consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
        consider_model_order_crossing_counter_node_influence: 0.25,
        consider_model_order_crossing_counter_port_influence: 0.75,
        force_node_model_order: true,
        port_constraints: PortConstraints::FixedOrder,
        merge_edges: true,
        merge_hierarchy_edges: false,
        feedback_edges: true,
        random_seed: seed,
        thoroughness: 11,
        graph_has_center_labels: true,
        graph_has_external_ports: true,
        ..Default::default()
    }
}

fn mixed_hierarchy() -> ElkInputGraph {
    let mut outer = node("outer", None, Some(HierarchyHandling::IncludeChildren));
    outer.direction = Some(ElkDirection::Up);
    outer.port_constraints = Some(PortConstraints::FixedSide);
    outer.nested_options = Some(Box::new(varied_options(23)));
    let middle = node(
        "middle",
        Some("outer"),
        Some(HierarchyHandling::SeparateChildren),
    );
    let mut reopened = node(
        "reopened",
        Some("middle"),
        Some(HierarchyHandling::IncludeChildren),
    );
    reopened.nested_options = Some(Box::new(varied_options(37)));
    let deep = node("deep", Some("reopened"), Some(HierarchyHandling::Inherit));
    let mut second = node("second", None, None);
    second.nested_options = Some(Box::new(varied_options(41)));
    let mut empty = node("empty", None, Some(HierarchyHandling::IncludeChildren));
    empty.nested_options = Some(Box::new(varied_options(53)));
    // Children precede their parents in the input, and an explicitly included subtree is hidden
    // behind a separate ancestor. Container ownership must follow physical membership in both.
    graph(
        vec![
            node(
                "deep-leaf",
                Some("deep"),
                Some(HierarchyHandling::SeparateChildren),
            ),
            node("second-leaf", Some("second"), None),
            deep,
            empty,
            middle,
            second,
            reopened,
            node("outer-leaf", Some("outer"), None),
            outer,
            node("loose", None, None),
        ],
        vec![
            inside_loop("empty"),
            inside_loop("middle"),
            inside_loop("outer-leaf"),
        ],
    )
}

fn former_needs_model_order(options: &LayeredOptions) -> bool {
    options.consider_model_order_strategy != OrderingStrategy::None
        || matches!(
            options.cycle_breaking_strategy,
            CycleBreakingStrategy::ModelOrder | CycleBreakingStrategy::GreedyModelOrder
        )
        || options.force_node_model_order
        || options.consider_model_order_crossing_counter_node_influence != 0.0
        || options.consider_model_order_crossing_counter_port_influence != 0.0
        || matches!(
            options.layering_strategy,
            LayeringStrategy::BreadthFirstModelOrder | LayeringStrategy::DepthFirstModelOrder
        )
}

#[test]
fn complete_container_options_match_the_former_all_node_algorithm() {
    for handling in [
        HierarchyHandling::Inherit,
        HierarchyHandling::IncludeChildren,
        HierarchyHandling::SeparateChildren,
    ] {
        for direction in [
            ElkDirection::Undefined,
            ElkDirection::Down,
            ElkDirection::Up,
        ] {
            for permutation in 0..3 {
                let mut input = mixed_hierarchy();
                input.options = varied_options(101);
                input.options.hierarchy_handling = handling;
                input.options.direction = direction;
                match permutation {
                    1 => input.nodes.reverse(),
                    2 => input.nodes.rotate_left(3),
                    _ => {}
                }
                let (root, former) = former_full_node_options(&input);
                let index = InputIndex::new(&input).unwrap();
                assert_eq!(former.len(), input.nodes.len());
                assert_eq!(index.root_options, root);
                assert_eq!(index.container_options.len(), 5);
                for node in &input.nodes {
                    let has_children = input
                        .nodes
                        .iter()
                        .any(|child| child.parent.as_deref() == Some(node.id.as_str()));
                    let expected = if has_children {
                        former.get(node.id.as_str())
                    } else {
                        None
                    };
                    assert_eq!(
                        index.container_options.get(node.id.as_str()),
                        expected,
                        "complete options for {}",
                        node.id
                    );
                    let parent = node
                        .parent
                        .as_deref()
                        .map(|id| &former[id])
                        .unwrap_or(&root);
                    assert_eq!(index.options_for_parent(node.parent.as_deref()), parent);
                    assert_eq!(
                        needs_model_order_for_child(&index, node),
                        former_needs_model_order(parent)
                    );
                }
                let middle = &index.container_options["middle"];
                assert_eq!(middle.spacing, LayeredOptions::default().spacing);
                assert_eq!(middle.random_seed, LayeredOptions::default().random_seed);
                assert!(!middle.inside_self_loops_activate);
                assert_eq!(
                    index.container_options["outer"].port_constraints,
                    PortConstraints::FixedSide
                );
                assert_eq!(
                    index.container_options["reopened"].port_constraints,
                    PortConstraints::Free
                );
                assert_eq!(
                    index.container_options["deep"].hierarchy_handling,
                    HierarchyHandling::IncludeChildren
                );
            }
        }
    }
}

#[test]
fn leaf_include_predicate_matches_former_options_for_every_override() {
    let overrides = [
        None,
        Some(HierarchyHandling::Inherit),
        Some(HierarchyHandling::IncludeChildren),
        Some(HierarchyHandling::SeparateChildren),
    ];
    for root_handling in [
        HierarchyHandling::Inherit,
        HierarchyHandling::IncludeChildren,
        HierarchyHandling::SeparateChildren,
    ] {
        for parent_handling in overrides {
            for leaf_handling in overrides {
                let mut leaf = node("leaf", Some("parent"), leaf_handling);
                // A nested-options hierarchy value is not a node hierarchy override.
                leaf.nested_options = Some(Box::new(LayeredOptions {
                    hierarchy_handling: HierarchyHandling::IncludeChildren,
                    ..Default::default()
                }));
                let mut root_leaf = node("root-leaf", None, leaf_handling);
                root_leaf.nested_options = Some(Box::new(LayeredOptions {
                    hierarchy_handling: HierarchyHandling::SeparateChildren,
                    ..Default::default()
                }));
                let mut input = graph(
                    vec![leaf, root_leaf, node("parent", None, parent_handling)],
                    Vec::new(),
                );
                input.options.hierarchy_handling = root_handling;
                let (_, former) = former_full_node_options(&input);
                let index = InputIndex::new(&input).unwrap();
                for node in &input.nodes {
                    assert_eq!(
                        input_node_uses_include_children_layout(&index, node),
                        former[node.id.as_str()].hierarchy_handling
                            == HierarchyHandling::IncludeChildren,
                        "root {root_handling:?}, parent {parent_handling:?}, leaf {leaf_handling:?}, node {}",
                        node.id
                    );
                }
                assert!(!index.has_children("leaf"));
                assert!(!index.has_children("root-leaf"));
                assert_eq!(index.container_options.len(), 1);
            }
        }
    }
}

#[test]
fn flat_inputs_retain_no_node_options_even_with_leaf_self_loops() {
    for count in [0, 1, 64, 257] {
        let nodes = (0..count)
            .map(|ordinal| {
                let mut leaf = node(
                    format!("leaf-{ordinal}").as_str(),
                    None,
                    Some(HierarchyHandling::IncludeChildren),
                );
                leaf.nested_options = Some(Box::new(varied_options(31)));
                leaf
            })
            .collect();
        let edges = if count == 0 {
            Vec::new()
        } else {
            vec![inside_loop("leaf-0")]
        };
        let mut input = graph(nodes, edges);
        input.options.inside_self_loops_activate = true;
        let index = InputIndex::new(&input).unwrap();
        assert_eq!(index.nodes.len(), count);
        assert!(index.container_options.is_empty());
        assert_eq!(index.container_options.capacity(), 0);
        if count > 0 {
            assert_eq!(index.materialized_graph_depth("leaf-0"), Some(1));
        }
    }
}

#[test]
fn nested_inputs_retain_exactly_the_actual_parent_count() {
    for depth in [1, 2, 9] {
        for width in [1, 7, 33] {
            let mut nodes = Vec::new();
            for level in 0..depth {
                let id = format!("container-{level}");
                let parent = (level > 0).then(|| format!("container-{}", level - 1));
                nodes.push(node(id.as_str(), parent.as_deref(), None));
                for sibling in 0..width {
                    nodes.push(node(
                        format!("leaf-{level}-{sibling}").as_str(),
                        Some(id.as_str()),
                        None,
                    ));
                }
            }
            nodes.reverse();
            let input = graph(nodes, Vec::new());
            let index = InputIndex::new(&input).unwrap();
            let actual_parents = input
                .nodes
                .iter()
                .filter_map(|node| node.parent.as_deref())
                .collect::<HashSet<_>>();
            assert_eq!(actual_parents.len(), depth);
            assert_eq!(index.container_options.len(), depth);
            assert!(
                index
                    .container_options
                    .keys()
                    .all(|id| actual_parents.contains(id))
            );
        }
    }
}

#[test]
fn materialized_reachability_matches_physical_paths_and_former_options() {
    for handling in [
        HierarchyHandling::Inherit,
        HierarchyHandling::IncludeChildren,
        HierarchyHandling::SeparateChildren,
    ] {
        let mut input = mixed_hierarchy();
        input.options.inside_self_loops_activate = true;
        input.options.hierarchy_handling = handling;
        let (root, former) = former_full_node_options(&input);
        let index = InputIndex::new(&input).unwrap();
        for node in &input.nodes {
            let mut ancestors = Vec::new();
            let mut parent = node.parent.as_deref();
            while let Some(id) = parent {
                let ancestor = input
                    .nodes
                    .iter()
                    .find(|candidate| candidate.id == id)
                    .unwrap();
                ancestors.push(ancestor);
                parent = ancestor.parent.as_deref();
            }
            ancestors.reverse();
            let first_unmaterialized = ancestors
                .iter()
                .find(|ancestor| {
                    root.hierarchy_handling != HierarchyHandling::IncludeChildren
                        || former[ancestor.id.as_str()].hierarchy_handling
                            != HierarchyHandling::IncludeChildren
                })
                .map(|ancestor| ancestor.id.as_str());
            let container_depth = first_unmaterialized.is_none().then_some(ancestors.len());
            let parent_options = node
                .parent
                .as_deref()
                .map(|id| &former[id])
                .unwrap_or(&root);
            let has_children = input
                .nodes
                .iter()
                .any(|child| child.parent.as_deref() == Some(node.id.as_str()));
            let has_inside_loop = input.edges.iter().any(|edge| {
                edge.inside_self_loops_yo && edge.source == node.id && edge.target == node.id
            });
            let has_graph = root.hierarchy_handling == HierarchyHandling::IncludeChildren
                && ((has_children
                    && former[node.id.as_str()].hierarchy_handling
                        == HierarchyHandling::IncludeChildren)
                    || (parent_options.inside_self_loops_activate && has_inside_loop));
            let graph_depth = container_depth.filter(|_| has_graph).map(|depth| depth + 1);
            assert_eq!(
                index.container_depth(node.id.as_str()),
                container_depth,
                "{}",
                node.id
            );
            assert_eq!(
                index.materialized_graph_depth(node.id.as_str()),
                graph_depth,
                "{}",
                node.id
            );
            assert_eq!(
                index.first_unmaterialized_graph_parent(node.id.as_str()),
                first_unmaterialized,
                "{}",
                node.id
            );
            assert_eq!(
                index.materialized_graph_path(node.id.as_str()),
                container_depth.map(|_| ancestors
                    .iter()
                    .map(|ancestor| ancestor.id.as_str())
                    .collect()),
                "{}",
                node.id
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkCall {
    Check(usize),
    Charge(usize),
}

struct WorkTrace {
    remaining: usize,
    stop_at: Option<usize>,
    calls: Vec<WorkCall>,
}

impl WorkTrace {
    fn observe(&mut self, call: WorkCall, units: usize) -> Result<(), WorkError> {
        self.calls.push(call);
        if self.stop_at == Some(self.calls.len()) || units > self.remaining {
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }
}

impl WorkControl for WorkTrace {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        self.observe(WorkCall::Check(units), units)
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.observe(WorkCall::Charge(units), units)?;
        self.remaining -= units;
        Ok(())
    }
}

#[test]
fn validation_errors_keep_their_original_precedence_before_option_resolution() {
    let mut input = graph(
        vec![
            node("stem", Some("A"), None),
            node("A", Some("B"), None),
            node("B", Some("A"), None),
            node("orphan", Some("missing-parent"), None),
            node("A", None, None),
        ],
        vec![
            edge("duplicate", "missing-source", "missing-target"),
            edge("duplicate", "B", "stem"),
        ],
    );
    let mut cases = vec![(
        input.clone(),
        ImportError::DuplicateNode {
            id: "A".to_string(),
        },
    )];
    input.nodes.pop();
    cases.push((
        input.clone(),
        ImportError::MissingParent {
            node_id: "orphan".to_string(),
            parent_id: "missing-parent".to_string(),
        },
    ));
    input.nodes[3].parent = None;
    cases.push((
        input.clone(),
        ImportError::DuplicateEdge {
            id: "duplicate".to_string(),
        },
    ));
    input.edges[1].id = "other".to_string();
    cases.push((
        input.clone(),
        ImportError::MissingEndpoint {
            edge_id: "duplicate".to_string(),
            node_id: "missing-source".to_string(),
        },
    ));
    input.edges[0].source = "A".to_string();
    cases.push((
        input.clone(),
        ImportError::MissingEndpoint {
            edge_id: "duplicate".to_string(),
            node_id: "missing-target".to_string(),
        },
    ));
    input.edges[0].target = "B".to_string();
    cases.push((
        input,
        ImportError::ParentCycle {
            node_id: "A".to_string(),
        },
    ));
    for (input, expected) in cases {
        assert_eq!(InputIndex::new(&input).err().as_ref(), Some(&expected));
        let mut control = WorkTrace {
            remaining: usize::MAX,
            stop_at: None,
            calls: Vec::new(),
        };
        assert_eq!(
            import_graph_with_work_control(&input, &mut control),
            Err(expected)
        );
        let planning = input.nodes.len() + 3 * input.edges.len();
        assert_eq!(
            control.calls,
            vec![WorkCall::Check(planning), WorkCall::Charge(planning)]
        );
    }
}

#[test]
fn parent_options_preserve_self_loops_model_order_and_every_work_checkpoint() {
    let mut group = node("group", None, None);
    group.nested_options = Some(Box::new(LayeredOptions {
        inside_self_loops_activate: true,
        force_node_model_order: true,
        ..Default::default()
    }));
    let mut leaf_loop = inside_loop("leaf");
    leaf_loop
        .labels
        .push(ElkInputLabel::center("inside", 30.0, 12.0));
    let mut root_edge = edge("group-tail", "group", "tail");
    root_edge
        .labels
        .push(ElkInputLabel::center("root", 20.0, 12.0));
    let mut input = graph(
        vec![
            node("hidden", Some("separate"), None),
            node("leaf", Some("group"), None),
            node(
                "separate",
                Some("group"),
                Some(HierarchyHandling::SeparateChildren),
            ),
            group,
            node("tail", None, None),
            node("empty", None, Some(HierarchyHandling::IncludeChildren)),
        ],
        vec![
            leaf_loop,
            inside_loop("separate"),
            root_edge,
            edge("leaf-tail", "leaf", "tail"),
        ],
    );
    input.options.inside_self_loops_activate = true;
    input.options.consider_model_order_strategy = OrderingStrategy::NodesAndEdges;
    let imported = import_graph(&input).unwrap();
    let orders = |graph: &LGraph| {
        graph
            .layerless_nodes
            .iter()
            .map(|node| (node.id.clone(), node.model_order))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        orders(&imported),
        vec![
            ("group".to_string(), Some(0)),
            ("tail".to_string(), Some(1)),
            ("empty".to_string(), Some(2)),
        ]
    );
    assert_eq!(imported.max_model_order_nodes, 5);
    let nested = imported.layerless_nodes[0].nested_graph.as_deref().unwrap();
    assert_eq!(
        orders(nested),
        vec![
            ("leaf".to_string(), Some(3)),
            ("separate".to_string(), Some(4))
        ]
    );
    for owner in &nested.layerless_nodes {
        let loop_graph = owner.nested_graph.as_deref().unwrap();
        assert!(loop_graph.graph_properties.self_loops);
        assert_eq!(loop_graph.edges.len(), 1);
        assert!(
            loop_graph
                .layerless_nodes
                .iter()
                .all(|node| node.id != "hidden")
        );
    }
    assert!(imported.layerless_nodes[2].nested_graph.is_none());
    let index = InputIndex::new(&input).unwrap();
    assert_eq!(index.container_options.len(), 2);
    assert!(!index.container_options.contains_key("leaf"));

    // Planning visits six nodes and four edges. Materialization owns two labels, the one-hop
    // leaf-to-tail path, and four reference domains (root, group, and the two loop graphs).
    let planning = 6 + 3 * 4;
    let materialization = 2 + 1 + 4;
    let required = planning + materialization;
    let expected = [
        WorkCall::Check(planning),
        WorkCall::Charge(planning),
        WorkCall::Check(materialization),
        WorkCall::Charge(materialization),
    ];
    for budget in [
        0,
        planning - 1,
        planning,
        required - 1,
        required,
        required + 1,
    ] {
        let mut control = WorkTrace {
            remaining: budget,
            stop_at: None,
            calls: Vec::new(),
        };
        let result = import_graph_with_work_control(&input, &mut control);
        let prefix = if budget < planning {
            1
        } else if budget < required {
            3
        } else {
            4
        };
        assert_eq!(control.calls.as_slice(), &expected[..prefix]);
        if budget >= required {
            assert_eq!(result.unwrap(), imported);
            assert_eq!(control.remaining, budget - required);
        } else {
            assert_eq!(result, Err(ImportError::Work(WorkError::Interrupted)));
            assert_eq!(
                control.remaining,
                if budget < planning {
                    budget
                } else {
                    budget - planning
                }
            );
        }
    }
    for stop_at in 1..=expected.len() {
        let mut control = WorkTrace {
            remaining: usize::MAX,
            stop_at: Some(stop_at),
            calls: Vec::new(),
        };
        assert_eq!(
            import_graph_with_work_control(&input, &mut control),
            Err(ImportError::Work(WorkError::Interrupted))
        );
        assert_eq!(control.calls.as_slice(), &expected[..stop_at]);
    }
}
