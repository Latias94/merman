use super::*;

#[derive(Debug)]
struct RecordingWorkControl {
    max: usize,
    used: usize,
    attempted: Vec<usize>,
}

impl RecordingWorkControl {
    fn unbounded() -> Self {
        Self {
            max: usize::MAX,
            used: 0,
            attempted: Vec::new(),
        }
    }
}

impl WorkControl for RecordingWorkControl {
    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.attempted.push(units);
        let next = self
            .used
            .checked_add(units)
            .ok_or(WorkError::ArithmeticOverflow)?;
        if next > self.max {
            return Err(WorkError::Interrupted);
        }
        self.used = next;
        Ok(())
    }
}

fn flat_layer(originals: &[usize]) -> LayerRank {
    let mut layer = LayerRank {
        nodes: vec![LayerNode::root()],
        local_by_original: HashMap::default(),
        root: 0,
        hierarchy_link_visits: 0,
        child_sort_work: 0,
    };
    for &original_ix in originals {
        let local_ix = layer.ensure_original(original_ix);
        layer.set_parent(local_ix, layer.root);
    }
    layer
}

fn barycenterless_entries(count: usize) -> Vec<BarycenterEntryIx> {
    (1..=count)
        .map(|v_ix| BarycenterEntryIx {
            v_ix,
            barycenter: None,
            weight: None,
        })
        .collect()
}

#[test]
fn conflict_replay_preserves_per_source_order_across_interleaved_inserts() {
    let layer = flat_layer(&[10, 20, 30, 40, 50]);
    let entries = barycenterless_entries(5);
    let mut constraints = ConstraintGraph::default();
    constraints.insert(20, 40);
    constraints.insert(10, 30);
    constraints.insert(20, 30);
    constraints.insert(10, 40);
    constraints.insert(30, 50);

    assert_eq!(
        constraints.outgoing.get(&10).map(Vec::as_slice),
        Some(&[30, 40][..])
    );
    assert_eq!(
        constraints.outgoing.get(&20).map(Vec::as_slice),
        Some(&[40, 30][..])
    );

    let mut scratch = SortScratch::default();
    let mut work = RecordingWorkControl::unbounded();
    resolve_conflicts(&layer, &entries, &constraints, &mut scratch, 51, &mut work).unwrap();

    assert_eq!(work.attempted, vec![2, 2, 1]);
    assert_eq!(work.used, 5);
    assert_eq!(scratch.group_members, vec![2, 1, 4, 3, 5]);
    assert_eq!(
        scratch.resolved,
        vec![
            SortEntryIx {
                members: 0..3,
                i: 0,
                barycenter: None,
                weight: None,
            },
            SortEntryIx {
                members: 3..5,
                i: 2,
                barycenter: None,
                weight: None,
            },
        ]
    );
}

#[test]
fn conflict_replay_deduplicates_edges_and_charges_irrelevant_targets_once() {
    let layer = flat_layer(&[10, 20, 30]);
    let entries = barycenterless_entries(3);
    let mut constraints = ConstraintGraph::default();
    constraints.insert(10, 30);
    constraints.insert(10, 30);
    constraints.insert(10, 99);
    constraints.insert(99, 30);
    constraints.insert(20, 30);

    assert_eq!(
        constraints.outgoing.get(&10).map(Vec::as_slice),
        Some(&[30, 99][..])
    );

    let mut scratch = SortScratch::default();
    let mut limited = RecordingWorkControl {
        max: 2,
        used: 0,
        attempted: Vec::new(),
    };
    assert_eq!(
        resolve_conflicts(
            &layer,
            &entries,
            &constraints,
            &mut scratch,
            100,
            &mut limited,
        ),
        Err(WorkError::Interrupted)
    );
    assert_eq!(limited.attempted, vec![2, 1]);
    assert_eq!(limited.used, 2);

    let mut work = RecordingWorkControl::unbounded();
    resolve_conflicts(&layer, &entries, &constraints, &mut scratch, 100, &mut work).unwrap();

    assert_eq!(work.attempted, vec![2, 1]);
    assert_eq!(work.used, 3);
    assert_eq!(scratch.group_members, vec![2, 1, 3]);
    assert_eq!(
        scratch.resolved,
        vec![SortEntryIx {
            members: 0..3,
            i: 0,
            barycenter: None,
            weight: None,
        }]
    );
}

#[test]
fn wide_flat_rank_uses_source_order_and_resets_the_shared_assembly() {
    let graph: Graph<crate::NodeLabel, crate::EdgeLabel, ()> =
        Graph::new(crate::graphlib::GraphOptions::default());
    let neighbors = vec![Vec::new(); 4096];
    let constraints = ConstraintGraph::default();
    let mut scratch = SortScratch::default();
    let mut work = RecordingWorkControl::unbounded();

    for width in [4096, 7, 4096] {
        let layer = flat_layer(&(0..width).collect::<Vec<_>>());
        let sorted = sort_subgraph(
            SortSubgraphContext {
                graph: &graph,
                layer: &layer,
                neighbors: &neighbors,
                tracked_orders: &[],
                constraints: &constraints,
            },
            &mut scratch,
            false,
            &mut work,
        )
        .unwrap();
        assert!(sorted.into_iter().eq(1..=width));
    }
    assert!(work.attempted.is_empty());
}

#[test]
fn nested_rank_keeps_raw_border_occurrences_and_conflict_charge_order() {
    let mut layer = flat_layer(&[0, 1, 2, 3, 4, 5, 6]);
    layer.set_parent(2, 1);
    layer.set_parent(3, 2);
    layer.set_parent(4, 3);
    layer.set_parent(5, 1);
    layer.set_parent(6, 2);
    layer.nodes[1].border_left = Some(2);
    layer.nodes[1].border_right = Some(2);

    let graph: Graph<crate::NodeLabel, crate::EdgeLabel, ()> =
        Graph::new(crate::graphlib::GraphOptions::default());
    let neighbors = vec![Vec::new(); 7];
    let mut constraints = ConstraintGraph::default();
    constraints.insert(1, 4);
    constraints.insert(4, 20);
    let context = || SortSubgraphContext {
        graph: &graph,
        layer: &layer,
        neighbors: &neighbors,
        tracked_orders: &[],
        constraints: &constraints,
    };

    let mut scratch = SortScratch::default();
    let mut short = RecordingWorkControl {
        max: 1,
        used: 0,
        attempted: Vec::new(),
    };
    assert_eq!(
        sort_subgraph(context(), &mut scratch, false, &mut short),
        Err(WorkError::Interrupted)
    );
    assert_eq!(short.attempted, [1, 1]);
    assert_eq!(short.used, 1);

    let mut exact = RecordingWorkControl {
        max: 2,
        used: 0,
        attempted: Vec::new(),
    };
    let expected = [3, 4, 6, 5, 3, 7];
    assert_eq!(
        sort_subgraph(context(), &mut scratch, false, &mut exact).unwrap(),
        expected
    );
    assert_eq!(exact.attempted, [1, 1]);
    assert_eq!(exact.used, 2);
}

#[test]
fn indexed_sort_uses_one_slot_for_unsortable_groups_and_expanded_width_for_sortable_groups() {
    let entries = (0..4)
        .map(|index| SortEntryIx {
            members: index..index + 1,
            i: [0, 1, 3, 4][index],
            barycenter: (index % 2 == 1).then_some(2.0),
            weight: (index % 2 == 1).then_some(1.0),
        })
        .collect::<Vec<_>>();
    let mut sortable = Vec::new();
    let mut unsortable = Vec::new();

    for (bias_right, expected) in [(false, [0, 1, 2, 3]), (true, [0, 3, 2, 1])] {
        let mut emitted = Vec::new();
        let metadata = sort_ix(
            &entries,
            bias_right,
            &mut sortable,
            &mut unsortable,
            |entry| {
                emitted.push(entry.members.start);
                [4, 2, 1, 2][entry.members.start]
            },
        );
        assert_eq!(emitted, expected);
        assert_eq!(metadata, (Some(2.0), Some(2.0)));
    }
}
