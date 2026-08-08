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
    let resolved =
        resolve_conflicts(&layer, &entries, &constraints, &mut scratch, 51, &mut work).unwrap();

    assert_eq!(work.attempted, vec![2, 2, 1]);
    assert_eq!(work.used, 5);
    assert_eq!(
        resolved,
        vec![
            SortEntryIx {
                vs: vec![2, 1, 4],
                i: 0,
                barycenter: None,
                weight: None,
            },
            SortEntryIx {
                vs: vec![3, 5],
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
    let resolved =
        resolve_conflicts(&layer, &entries, &constraints, &mut scratch, 100, &mut work).unwrap();

    assert_eq!(work.attempted, vec![2, 1]);
    assert_eq!(work.used, 3);
    assert_eq!(
        resolved,
        vec![SortEntryIx {
            vs: vec![2, 1, 3],
            i: 0,
            barycenter: None,
            weight: None,
        }]
    );
}
