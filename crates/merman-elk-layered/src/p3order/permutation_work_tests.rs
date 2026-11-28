use super::*;
use crate::graph::{LNode, LPort, PortType, SelfHyperLoop, SelfLoopHolder, SelfLoopPort};
use crate::options::LayeredOptions;

struct Budget {
    remaining: usize,
    charges: Vec<usize>,
}

impl WorkControl for Budget {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        if units <= self.remaining {
            Ok(())
        } else {
            Err(WorkError::Interrupted)
        }
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or(WorkError::Interrupted)?;
        self.charges.push(units);
        Ok(())
    }
}

fn graph() -> LGraph {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    let mut owner = LNode::new("owner", 10.0, 10.0, Some(0));
    owner.ports.push(LPort::new("a", 0, PortType::Output));
    owner.ports.push(LPort::new("b", 0, PortType::Output));
    graph.layerless_nodes.push(owner);
    graph
        .layerless_nodes
        .push(LNode::new("other", 10.0, 10.0, Some(1)));
    graph
}

#[test]
fn nonidentity_rewrite_admits_exact_reference_work_before_installing_the_map() {
    for available in [1, 11, 12] {
        let mut graph = graph();
        let before = graph.clone();
        let mut context = PortOrderContext::default();
        let mut work = Budget {
            remaining: available,
            charges: Vec::new(),
        };
        let result = apply_port_order(&mut graph, 0, vec![1, 0], &mut work, &mut context);
        if available == 1 {
            assert_eq!(result, Err(WorkError::Interrupted));
            assert_eq!(graph, before);
            assert!(context.census.is_none());
            assert!(work.charges.is_empty());
            assert_eq!(work.remaining, 1);
        } else if available == 11 {
            assert_eq!(result, Err(WorkError::Interrupted));
            assert_eq!(graph, before);
            assert_eq!(work.charges, [2]);
        } else {
            result.unwrap();
            assert_eq!(work.charges, [2, 10]);
            assert_eq!(graph.layerless_nodes[0].ports[0].id, "b");
            let mut second = Budget {
                remaining: 10,
                charges: Vec::new(),
            };
            apply_port_order(&mut graph, 0, vec![1, 0], &mut second, &mut context).unwrap();
            assert_eq!(second.charges, [10]);
            assert_eq!(graph, before);
        }
    }
}

#[test]
fn identity_rewrite_needs_no_reference_census_and_still_repairs_owners() {
    for count in [0, 1, 2] {
        let mut graph = graph();
        graph.layerless_nodes[0].ports.truncate(count);
        for port in &mut graph.layerless_nodes[0].ports {
            port.node = 99;
        }
        let mut work = Budget {
            remaining: 0,
            charges: Vec::new(),
        };
        let mut context = PortOrderContext::default();
        apply_port_order(&mut graph, 0, (0..count).collect(), &mut work, &mut context).unwrap();
        assert!(context.census.is_none());
        assert!(work.charges.is_empty());
        assert!(
            graph.layerless_nodes[0]
                .ports
                .iter()
                .all(|port| port.node == 0)
        );
    }
}

#[test]
fn census_interrupts_before_unadmitted_descendants_and_before_graph_mutation() {
    let mut graph = graph();
    let mut child = LGraph::new("child", LayeredOptions::default());
    child
        .layerless_nodes
        .push(LNode::new("leaf", 0.0, 0.0, None));
    graph.layerless_nodes[0].nested_graph = Some(Box::new(child));
    let before = graph.clone();
    let mut work = Budget {
        remaining: 2,
        charges: Vec::new(),
    };
    let mut context = PortOrderContext::default();
    assert_eq!(
        apply_port_order(&mut graph, 0, vec![1, 0], &mut work, &mut context),
        Err(WorkError::Interrupted)
    );
    assert_eq!(graph, before);
    assert!(context.census.is_none());
    assert_eq!(work.charges, [2]);
}

struct RejectAtBoundary {
    checks_fail: bool,
    reject_charge: usize,
    calls: usize,
}

impl WorkControl for RejectAtBoundary {
    fn check(&mut self, _units: usize) -> Result<(), WorkError> {
        if self.checks_fail {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
        self.calls += 1;
        if self.calls == self.reject_charge {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }
}

#[test]
fn failed_charge_or_identity_cancellation_never_installs_a_permutation() {
    for (identity, fail_check, reject_charge) in
        [(true, true, 0), (false, false, 1), (false, false, 2)]
    {
        let mut graph = graph();
        graph.layerless_nodes[0].ports[0].node = 99;
        let before = graph.clone();
        let mut context = PortOrderContext::default();
        let mut work = RejectAtBoundary {
            checks_fail: fail_check,
            reject_charge,
            calls: 0,
        };
        let order = if identity { vec![0, 1] } else { vec![1, 0] };
        assert_eq!(
            apply_port_order(&mut graph, 0, order, &mut work, &mut context),
            Err(WorkError::Interrupted)
        );
        assert_eq!(graph, before);
        assert_eq!(work.calls, reject_charge);
    }
}

#[test]
fn a_later_descendant_census_rejection_preserves_the_graph() {
    let mut graph = graph();
    let mut child = LGraph::new("child", LayeredOptions::default());
    let mut parent = LNode::new("parent", 0.0, 0.0, None);
    let mut grandchild = LGraph::new("grandchild", LayeredOptions::default());
    grandchild
        .layerless_nodes
        .push(LNode::new("leaf", 0.0, 0.0, None));
    parent.nested_graph = Some(Box::new(grandchild));
    child.layerless_nodes.push(parent);
    graph.layerless_nodes[0].nested_graph = Some(Box::new(child));
    let before = graph.clone();
    let mut work = Budget {
        remaining: 4,
        charges: Vec::new(),
    };
    let mut context = PortOrderContext::default();
    assert_eq!(
        apply_port_order(&mut graph, 0, vec![1, 0], &mut work, &mut context),
        Err(WorkError::Interrupted)
    );
    assert_eq!(graph, before);
    assert!(context.census.is_none());
    assert_eq!(work.charges, [2, 2]);
    let mut charge_failure = RejectAtBoundary {
        checks_fail: false,
        reject_charge: 3,
        calls: 0,
    };
    assert_eq!(
        apply_port_order(&mut graph, 0, vec![1, 0], &mut charge_failure, &mut context),
        Err(WorkError::Interrupted)
    );
    assert_eq!(graph, before);
    assert!(context.census.is_none());
    assert_eq!(charge_failure.calls, 3);
}

#[test]
fn side_range_interruption_keeps_the_admitted_south_permutation() {
    let mut graph = graph();
    graph.set_node_layer(0, 0);
    graph.layerless_nodes[0].port_constraints = PortConstraints::FixedSide;
    graph.layerless_nodes[1].long_edge_source = Some(PortRef { node: 0, port: 0 });
    graph.layerless_nodes[0].ports.clear();
    for index in 0..6 {
        let mut port = LPort::new(format!("p{index}"), 0, PortType::Output);
        port.side = if index < 3 {
            PortSide::South
        } else {
            PortSide::West
        };
        graph.layerless_nodes[0].ports.push(port);
    }
    let original = graph.clone();
    let mut work = Budget {
        remaining: 12,
        charges: Vec::new(),
    };
    assert_eq!(
        sort_port_lists_with_work_control(&mut graph, &mut work),
        Err(WorkError::Interrupted)
    );
    assert_eq!(work.charges, [2, 10]);
    assert_eq!(
        graph.layerless_nodes[1].long_edge_source,
        Some(PortRef { node: 0, port: 4 })
    );
    assert_eq!(
        graph.layerless_nodes[0]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>(),
        ["p4", "p3", "p2", "p1", "p0", "p5"]
    );
    let mut retry = Budget {
        remaining: 12,
        charges: Vec::new(),
    };
    reverse_west_and_south_side(&mut graph, 0, &mut retry, &mut PortOrderContext::default())
        .unwrap();
    assert_eq!(graph, original);
}

#[test]
fn deferred_repair_prepays_the_exit_walk_and_charges_only_affected_maps() {
    let mut original = graph();
    original
        .layerless_nodes
        .push(LNode::new("third", 10.0, 10.0, Some(2)));
    for owner in [1, 2] {
        let node = &mut original.layerless_nodes[owner];
        node.ports.push(LPort::new("a", owner, PortType::Input));
        node.ports.push(LPort::new("b", owner, PortType::Input));
    }
    let peer = LGraph::new("peer", LayeredOptions::default());
    original.layerless_nodes[0].origin_port =
        Some(original.scoped_port(PortRef { node: 2, port: 0 }));
    original.layerless_nodes[1].origin_port = Some(peer.scoped_port(PortRef { node: 2, port: 0 }));
    let mut child = LGraph::new("child", LayeredOptions::default());
    let mut dummy = LNode::new("dummy", 0.0, 0.0, None);
    dummy.origin_port = Some(original.scoped_port(PortRef { node: 1, port: 0 }));
    dummy.nested_graph = Some(Box::new(LGraph::new("empty", LayeredOptions::default())));
    child.layerless_nodes.push(dummy);
    original.layerless_nodes[0].nested_graph = Some(Box::new(child));

    for slot in [0, 1] {
        original.self_loop_holders.push(SelfLoopHolder {
            node: 1,
            hyper_loops: vec![SelfHyperLoop {
                ports: vec![SelfLoopPort {
                    port: slot,
                    had_only_self_loops: true,
                    hidden: false,
                }],
                edges: Vec::new(),
                labels: None,
                self_loop_type: None,
                leftmost_port: None,
                rightmost_port: None,
                occupied_sides: Vec::new(),
                routing_slots: [0; 5],
            }],
            ports_hidden: false,
            original_port_constraints: PortConstraints::FixedOrder,
        });
    }

    let orders = [
        (0, [1, 0]),
        (1, [1, 0]),
        (1, [1, 0]),
        (0, [1, 0]),
        (2, [1, 0]),
        (0, [1, 0]),
        (2, [0, 1]),
    ];
    let charges = [5, 2, 2, 2, 1, 16, 31, 3, 8, 1, 3];
    for (available, admitted, debits) in [(58, 1, 6), (59, 2, 7), (73, 5, 10), (74, 7, 11)] {
        let mut graph = original.clone();
        let mut work = Budget {
            remaining: available,
            charges: Vec::new(),
        };
        let result = {
            let mut pass = PortOrderPass::new(&mut graph, PortOrderDeferral::AllowDescendants);
            orders.into_iter().try_for_each(|(owner, order)| {
                apply_port_order(
                    pass.graph,
                    owner,
                    order.to_vec(),
                    &mut work,
                    &mut pass.context,
                )
            })
        };
        assert_eq!(work.charges, charges[..debits]);
        assert_eq!(result.is_ok(), available == 74);
        let mut expected = original.clone();
        for (owner, order) in orders.into_iter().take(admitted) {
            assert!(expected.reorder_node_ports(owner, order));
        }
        assert_eq!(graph, expected);
    }
}

#[test]
fn reference_context_reuses_counts_across_owners_with_nested_and_long_edge_refs() {
    let mut graph = graph();
    graph.layerless_nodes[1].ports = vec![
        LPort::new("other-a", 1, PortType::Input),
        LPort::new("other-b", 1, PortType::Input),
    ];
    graph.layerless_nodes[0].long_edge_target = Some(PortRef { node: 1, port: 1 });
    graph.layerless_nodes[1].long_edge_source = Some(PortRef { node: 0, port: 0 });
    let mut child = LGraph::new("child", LayeredOptions::default());
    for (node, port) in [(0, 0), (1, 1)] {
        let mut external = LNode::new(format!("external-{node}"), 0.0, 0.0, None);
        external.origin_port = Some(graph.scoped_port(PortRef { node, port }));
        child.layerless_nodes.push(external);
    }
    graph.layerless_nodes[0].nested_graph = Some(Box::new(child));
    let mut context = PortOrderContext::default();
    let mut first = Budget {
        remaining: 17,
        charges: Vec::new(),
    };
    apply_port_order(&mut graph, 0, vec![1, 0], &mut first, &mut context).unwrap();
    assert_eq!(first.charges, [2, 3, 12]);
    let mut second = Budget {
        remaining: 12,
        charges: Vec::new(),
    };
    apply_port_order(&mut graph, 1, vec![1, 0], &mut second, &mut context).unwrap();
    assert_eq!(second.charges, [12]);
    assert_eq!(
        graph.layerless_nodes[0].long_edge_target,
        Some(PortRef { node: 1, port: 0 })
    );
    assert_eq!(
        graph.layerless_nodes[1].long_edge_source,
        Some(PortRef { node: 0, port: 1 })
    );
    let nested = graph.layerless_nodes[0].nested_graph.as_ref().unwrap();
    assert_eq!(
        nested.layerless_nodes[0].origin_port.as_ref().unwrap().port,
        PortRef { node: 0, port: 1 }
    );
    assert_eq!(
        nested.layerless_nodes[1].origin_port.as_ref().unwrap().port,
        PortRef { node: 1, port: 0 }
    );
}

#[test]
fn later_loop_census_check_and_charge_failures_preserve_the_graph() {
    let mut graph = graph();
    let hyper_loop = |port| SelfHyperLoop {
        ports: vec![SelfLoopPort {
            port,
            had_only_self_loops: true,
            hidden: false,
        }],
        edges: Vec::new(),
        labels: None,
        self_loop_type: None,
        leftmost_port: None,
        rightmost_port: None,
        occupied_sides: Vec::new(),
        routing_slots: [0; 5],
    };
    graph.self_loop_holders.push(SelfLoopHolder {
        node: 0,
        hyper_loops: vec![hyper_loop(0), hyper_loop(1)],
        ports_hidden: false,
        original_port_constraints: PortConstraints::Free,
    });
    let before = graph.clone();
    let mut work = Budget {
        remaining: 5,
        charges: Vec::new(),
    };
    let mut context = PortOrderContext::default();
    assert_eq!(
        apply_port_order(&mut graph, 0, vec![1, 0], &mut work, &mut context),
        Err(WorkError::Interrupted)
    );
    assert_eq!(work.charges, [3, 2]);
    assert_eq!(graph, before);
    assert!(context.census.is_none());
    let mut failure = RejectAtBoundary {
        checks_fail: false,
        reject_charge: 3,
        calls: 0,
    };
    assert_eq!(
        apply_port_order(&mut graph, 0, vec![1, 0], &mut failure, &mut context),
        Err(WorkError::Interrupted)
    );
    assert_eq!(failure.calls, 3);
    assert_eq!(graph, before);
    assert!(context.census.is_none());
}

#[derive(Clone, Copy, Debug)]
enum DamagedReference {
    OppositePort,
    HolderEdge,
    LongEdge,
    DescendantOrigin,
}

fn damaged_reference_graph(damage: DamagedReference, owner: usize) -> LGraph {
    let mut graph = graph();
    graph.layerless_nodes[1].ports = vec![
        LPort::new("other-a", 1, PortType::Input),
        LPort::new("other-b", 1, PortType::Input),
    ];
    for node in 0..2 {
        graph.set_node_layer(node, 0);
        graph.layerless_nodes[node].port_constraints = PortConstraints::FixedOrder;
        for (slot, port) in graph.layerless_nodes[node].ports.iter_mut().enumerate() {
            port.port_index = Some(1 - slot as isize);
        }
    }
    let invalid = PortRef {
        node: owner,
        port: 2,
    };
    let edge = crate::graph::LayeredEdge {
        id: "edge".into(),
        source: PortRef { node: 0, port: 0 },
        target: PortRef { node: 1, port: 0 },
        source_node_id: "owner".into(),
        target_node_id: "other".into(),
        labels: Vec::new(),
        minlen: 1,
        reversed: false,
        bend_points: Vec::new(),
        model_order: None,
        priority_direction: 0,
        priority_shortness: 0,
        priority_straightness: 0,
        thickness: 0.0,
        original_opposite_port: matches!(damage, DamagedReference::OppositePort).then_some(invalid),
        compound_segment: None,
    };
    let edge_index = graph.add_edge(edge).unwrap();
    match damage {
        DamagedReference::OppositePort => {}
        DamagedReference::HolderEdge => graph.self_loop_holders.push(SelfLoopHolder {
            node: owner,
            hyper_loops: vec![SelfHyperLoop {
                ports: Vec::new(),
                edges: vec![crate::graph::SelfLoopEdge {
                    edge: edge_index,
                    source_port: 0,
                    target_port: invalid.port,
                }],
                labels: None,
                self_loop_type: None,
                leftmost_port: None,
                rightmost_port: None,
                occupied_sides: Vec::new(),
                routing_slots: [0; 5],
            }],
            ports_hidden: false,
            original_port_constraints: PortConstraints::FixedOrder,
        }),
        DamagedReference::LongEdge => graph.layerless_nodes[0].long_edge_source = Some(invalid),
        DamagedReference::DescendantOrigin => {
            let mut child = LGraph::new("child", LayeredOptions::default());
            let mut dummy = LNode::new("origin", 0.0, 0.0, None);
            dummy.origin_port = Some(graph.scoped_port(invalid));
            child.layerless_nodes.push(dummy);
            graph.layerless_nodes[0].nested_graph = Some(Box::new(child));
        }
    }
    graph
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecondOrder {
    Continue,
    Interrupt,
    Panic,
}

struct CallbackPanic;

struct SecondOrderWork {
    action: SecondOrder,
    orders: usize,
}

impl WorkControl for SecondOrderWork {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        if units == 0 {
            self.orders += 1;
            if self.orders == 2 {
                match self.action {
                    SecondOrder::Continue => {}
                    SecondOrder::Interrupt => return Err(WorkError::Interrupted),
                    SecondOrder::Panic => std::panic::panic_any(CallbackPanic),
                }
            }
        }
        Ok(())
    }

    fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
        Ok(())
    }
}

#[test]
fn malformed_references_preserve_failure_order_and_repair_the_admitted_prefix() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    for damage in [
        DamagedReference::OppositePort,
        DamagedReference::HolderEdge,
        DamagedReference::LongEdge,
        DamagedReference::DescendantOrigin,
    ] {
        for (owner, action) in [
            (0, SecondOrder::Panic),
            (1, SecondOrder::Panic),
            (1, SecondOrder::Interrupt),
            (1, SecondOrder::Continue),
        ] {
            let mut graph = damaged_reference_graph(damage, owner);
            let mut expected = graph.clone();
            let reference_panics = owner == 0 || action == SecondOrder::Continue;
            let immediate = catch_unwind(AssertUnwindSafe(|| {
                expected.reorder_node_ports(0, [1, 0]);
                if reference_panics {
                    expected.reorder_node_ports(1, [1, 0]);
                }
            }));
            assert_eq!(immediate.is_err(), reference_panics);

            let mut work = SecondOrderWork { action, orders: 0 };
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                sort_port_lists_with_work_control(&mut graph, &mut work)
            }));
            if let Err(reference_error) = immediate {
                let error = outcome.expect_err("the reference's owner must panic");
                assert_eq!(
                    error.downcast_ref::<String>(),
                    reference_error.downcast_ref::<String>()
                );
                assert!(!error.is::<CallbackPanic>());
            } else if action == SecondOrder::Panic {
                assert!(
                    outcome
                        .expect_err("the callback must panic")
                        .is::<CallbackPanic>()
                );
            } else {
                assert_eq!(outcome.unwrap(), Err(WorkError::Interrupted));
            }
            assert_eq!(work.orders, if owner == 0 { 1 } else { 2 });
            assert_eq!(graph, expected, "{damage:?} {owner} {action:?}");
        }
    }
}

fn model_order_rank_graph(with_child: bool) -> LGraph {
    let mut graph = LGraph::new(
        "root",
        LayeredOptions {
            consider_model_order_strategy: OrderingStrategy::PreferEdges,
            ..LayeredOptions::default()
        },
    );
    for (node, sides) in [
        &[PortSide::West, PortSide::East][..],
        &[PortSide::East, PortSide::East],
        &[PortSide::West, PortSide::West],
        &[PortSide::West],
        &[PortSide::West],
    ]
    .into_iter()
    .enumerate()
    {
        let mut data = LNode::new(format!("n{node}"), 10.0, 10.0, Some(node));
        for &side in sides {
            let port_type = if side == PortSide::West {
                PortType::Input
            } else {
                PortType::Output
            };
            let mut port = LPort::new("shared-id", node, port_type);
            port.set_side(side);
            data.ports.push(port);
        }
        graph.layerless_nodes.push(data);
        graph.set_node_layer(node, usize::from(node >= 2));
    }
    let scoped_slot = PortRef { node: 1, port: 0 };
    for (source_port, target_node, target_port, model_order) in [
        (0, 2, 1, 1),
        (1, 2, 0, 0),
        (0, 3, 0, 2),
        (1, 4, 0, 3),
        (0, 1, 1, 4),
    ] {
        graph
            .add_edge(crate::graph::LayeredEdge {
                id: format!("e{model_order}"),
                source: PortRef {
                    node: 1,
                    port: source_port,
                },
                target: PortRef {
                    node: target_node,
                    port: target_port,
                },
                source_node_id: "n1".into(),
                target_node_id: format!("n{target_node}"),
                labels: Vec::new(),
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: Some(model_order),
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: (target_node == 1).then_some(scoped_slot),
                compound_segment: None,
            })
            .unwrap();
    }
    graph.detach_edge(4).unwrap();
    graph.self_loop_holders.push(SelfLoopHolder {
        node: 1,
        hyper_loops: vec![SelfHyperLoop {
            ports: vec![SelfLoopPort {
                port: 0,
                had_only_self_loops: false,
                hidden: false,
            }],
            edges: vec![crate::graph::SelfLoopEdge {
                edge: 4,
                source_port: 0,
                target_port: 1,
            }],
            labels: None,
            self_loop_type: None,
            leftmost_port: None,
            rightmost_port: None,
            occupied_sides: Vec::new(),
            routing_slots: [0; 5],
        }],
        ports_hidden: false,
        original_port_constraints: PortConstraints::Free,
    });
    let foreign = LGraph::new("root", LayeredOptions::default());
    graph.layerless_nodes[2].origin_port = Some(graph.scoped_port(scoped_slot));
    graph.layerless_nodes[3].origin_port = Some(foreign.scoped_port(scoped_slot));
    graph.layerless_nodes[0].long_edge_source = Some(scoped_slot);
    graph.layerless_nodes[0].long_edge_target = Some(PortRef { node: 1, port: 1 });
    graph.layerless_nodes[3].ports[0].node = usize::MAX;

    if with_child {
        let mut child = LGraph::new("root", LayeredOptions::default());
        let mut parent_origin = LNode::new("parent-origin", 0.0, 0.0, None);
        parent_origin.origin_port = Some(graph.scoped_port(scoped_slot));
        let mut child_origin = LNode::new("child-origin", 0.0, 0.0, None);
        child_origin.origin_port = Some(child.scoped_port(scoped_slot));
        child_origin
            .ports
            .push(LPort::new("shared-id", 1, PortType::Input));
        child.layerless_nodes.extend([parent_origin, child_origin]);
        graph.layerless_nodes[1].nested_graph = Some(Box::new(child));
    }
    graph
}

struct ModelOrderWork {
    remaining: usize,
    events: Vec<(bool, usize)>,
    stop_at: Option<(usize, bool)>,
}

impl ModelOrderWork {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            events: Vec::new(),
            stop_at: None,
        }
    }

    fn visit(&mut self, charge: bool, units: usize) -> Result<(), WorkError> {
        let index = self.events.len();
        self.events.push((charge, units));
        if let Some((stop, panic)) = self.stop_at
            && stop == index
        {
            if panic {
                std::panic::panic_any(CallbackPanic);
            }
            return Err(WorkError::Interrupted);
        }
        if units > self.remaining {
            return Err(WorkError::Interrupted);
        }
        if charge {
            self.remaining -= units;
        }
        Ok(())
    }
}

impl WorkControl for ModelOrderWork {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        self.visit(false, units)
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.visit(true, units)
    }
}

fn sort_by_input_model_immediate(
    graph: &mut LGraph,
    work: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    sort_by_input_model_impl(graph, work, &mut PortOrderContext::default())
}

#[test]
fn model_order_rank_projection_matches_immediate_for_detached_and_scoped_ports() {
    for with_child in [false, true] {
        let original = model_order_rank_graph(with_child);
        let mut immediate = original.clone();
        let mut projected = original.clone();
        let mut old_work = ModelOrderWork::new(usize::MAX);
        let mut new_work = ModelOrderWork::new(usize::MAX);
        sort_by_input_model_immediate(&mut immediate, &mut old_work).unwrap();
        sort_by_input_model(&mut projected, &mut new_work).unwrap();
        assert_eq!(projected, immediate);
        assert_eq!(format!("{projected:?}"), format!("{immediate:?}"));
        if with_child {
            assert_eq!(old_work.events, new_work.events);
            let child = projected.layerless_nodes[1].nested_graph.as_ref().unwrap();
            let parent = child.layerless_nodes[0].origin_port.as_ref().unwrap();
            let local = child.layerless_nodes[1].origin_port.as_ref().unwrap();
            assert!(parent.belongs_to(&projected));
            assert!(local.belongs_to(child));
            assert_eq!((parent.port.port, local.port.port), (1, 0));
        } else {
            // Immediate repairs cost 35 + 38 + 35; the projected repairs cost 35 + 44 + 12.
            assert_eq!(new_work.remaining - old_work.remaining, 17);
        }
        // The sink's reversed west order must use the source's new ranks, despite duplicate IDs.
        assert_eq!(projected.layerless_nodes[2].ports[0].incoming_edges, [0]);
        let sink_order = &projected.layers[1].nodes;
        assert!(
            sink_order.iter().position(|&node| node == 4).unwrap()
                < sink_order.iter().position(|&node| node == 3).unwrap()
        );
        assert_eq!(projected.layerless_nodes[3].ports[0].node, 3);
        let local = projected.layerless_nodes[2].origin_port.as_ref().unwrap();
        let foreign = projected.layerless_nodes[3].origin_port.as_ref().unwrap();
        assert!(local.belongs_to(&projected));
        assert!(!foreign.belongs_to(&projected));
        assert_eq!((local.port.port, foreign.port.port), (1, 0));
        let detached = &projected.edges[4];
        assert!(!projected.edge_source_attached(4));
        assert!(!projected.edge_target_attached(4));
        assert_eq!((detached.source.port, detached.target.port), (1, 0));
        assert_eq!(detached.original_opposite_port.unwrap().port, 1);
        let hyper_loop = &projected.self_loop_holders[0].hyper_loops[0];
        assert_eq!(hyper_loop.ports[0].port, 1);
        assert_eq!(
            (
                hyper_loop.edges[0].source_port,
                hyper_loop.edges[0].target_port
            ),
            (1, 0)
        );
    }
}

#[test]
fn model_order_rank_projection_preserves_the_admitted_prefix_on_exit() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let original = model_order_rank_graph(false);
    let mut expected = original.clone();
    let mut unlimited = ModelOrderWork::new(usize::MAX);
    sort_by_input_model(&mut expected, &mut unlimited).unwrap();
    let exact_units = usize::MAX - unlimited.remaining;
    let mut exact = original.clone();
    let mut exact_work = ModelOrderWork::new(exact_units);
    sort_by_input_model(&mut exact, &mut exact_work).unwrap();
    assert_eq!(exact_work.remaining, 0);
    assert_eq!(exact, expected);

    let mut below = original.clone();
    let mut below_work = ModelOrderWork::new(exact_units - 1);
    assert_eq!(
        sort_by_input_model(&mut below, &mut below_work),
        Err(WorkError::Interrupted)
    );

    for index in 0..unlimited.events.len() {
        for panics in [false, true] {
            let mut immediate = original.clone();
            let mut projected = original.clone();
            let mut old_work = ModelOrderWork::new(usize::MAX);
            let mut new_work = ModelOrderWork::new(usize::MAX);
            old_work.stop_at = Some((index, panics));
            new_work.stop_at = old_work.stop_at;
            let old = catch_unwind(AssertUnwindSafe(|| {
                sort_by_input_model_immediate(&mut immediate, &mut old_work)
            }));
            let new = catch_unwind(AssertUnwindSafe(|| {
                sort_by_input_model(&mut projected, &mut new_work)
            }));
            if panics {
                assert!(old.unwrap_err().is::<CallbackPanic>());
                assert!(new.unwrap_err().is::<CallbackPanic>());
            } else {
                assert_eq!(old.unwrap(), Err(WorkError::Interrupted));
                assert_eq!(new.unwrap(), Err(WorkError::Interrupted));
                if index + 1 == below_work.events.len() {
                    assert_eq!(below, immediate, "W - 1 must leave the same prefix");
                }
            }
            assert_eq!(old_work.events.len(), index + 1);
            assert_eq!(new_work.events.len(), index + 1);
            assert_eq!(old_work.events[index].0, new_work.events[index].0);
            assert_eq!(projected, immediate, "callback {index}, panics {panics}");
        }
    }
}

#[test]
fn model_order_rank_projection_retains_malformed_detached_reference_failure() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut original = model_order_rank_graph(false);
    original.edges[4]
        .original_opposite_port
        .as_mut()
        .unwrap()
        .port = 2;
    let mut immediate = original.clone();
    let mut projected = original;
    let mut old_work = ModelOrderWork::new(usize::MAX);
    let mut new_work = ModelOrderWork::new(usize::MAX);
    let old = catch_unwind(AssertUnwindSafe(|| {
        sort_by_input_model_immediate(&mut immediate, &mut old_work)
    }))
    .unwrap_err();
    let new = catch_unwind(AssertUnwindSafe(|| {
        sort_by_input_model(&mut projected, &mut new_work)
    }))
    .unwrap_err();
    assert_eq!(old.downcast_ref::<String>(), new.downcast_ref::<String>());
    assert_eq!(old_work.events, new_work.events);
    assert_eq!(projected, immediate);
}
