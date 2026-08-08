use super::EMPTY_LAYER_SLOT;
use super::OrderEdgeWeight;
use crate::graphlib::Graph;
use crate::work::{ceil_log2, checked_add, checked_mul, checked_n_log_n};
use crate::{WorkControl, WorkError};
use rustc_hash::FxHashMap as HashMap;

#[derive(Debug, Clone, Copy)]
struct SouthEntry {
    pos: usize,
    weight: f64,
}

fn accumulate_crossings(
    entries: &[SouthEntry],
    tree: &mut [f64],
    first_index: usize,
    cc: &mut f64,
) {
    for entry in entries {
        let mut index = entry.pos + first_index;
        tree[index] += entry.weight;
        let mut weight_sum: f64 = 0.0;
        while index > 0 {
            if index % 2 == 1 {
                weight_sum += tree[index + 1];
            }
            index = (index - 1) >> 1;
            tree[index] += entry.weight;
        }
        *cc += entry.weight * weight_sum;
    }
}

pub fn cross_count<N, E, G>(g: &Graph<N, E, G>, layering: &[Vec<String>]) -> f64
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut cc: f64 = 0.0;
    for i in 1..layering.len() {
        cc += two_layer_cross_count(g, &layering[i - 1], &layering[i]);
    }
    cc
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ControlledCrossCount {
    pub value: f64,
    pub is_proven_minimum: bool,
}

#[derive(Debug, Clone, Copy)]
struct ZeroCrossingDomain {
    all_weights_are_non_negative_and_finite: bool,
    all_layer_weight_sums_are_finite: bool,
    minimum_positive_weight: Option<f64>,
}

impl Default for ZeroCrossingDomain {
    fn default() -> Self {
        Self {
            all_weights_are_non_negative_and_finite: true,
            all_layer_weight_sums_are_finite: true,
            minimum_positive_weight: None,
        }
    }
}

impl ZeroCrossingDomain {
    fn observe_weight(&mut self, weight: f64) {
        self.all_weights_are_non_negative_and_finite &= weight.is_finite() && weight >= 0.0;
        if weight > 0.0 {
            self.minimum_positive_weight = Some(
                self.minimum_positive_weight
                    .map_or(weight, |minimum| minimum.min(weight)),
            );
        }
    }

    fn finish_layer(&mut self, edge_count: Option<usize>, maximum_weight: f64) {
        let Some(edge_count) = edge_count else {
            self.all_layer_weight_sums_are_finite = false;
            return;
        };
        if edge_count == 0 || maximum_weight == 0.0 {
            return;
        }

        // A finite total weight keeps every non-negative accumulator-tree cell and every
        // `weight_sum` finite in every ordering. Bound the total conservatively with the next
        // power of two so a zero-weight edge can never observe JavaScript's `0 * Infinity = NaN`.
        let Some(capacity) = edge_count.checked_next_power_of_two() else {
            self.all_layer_weight_sums_are_finite = false;
            return;
        };
        self.all_layer_weight_sums_are_finite &= maximum_weight <= f64::MAX / capacity as f64;
    }

    fn proves_zero_is_global_minimum(self, value: f64) -> bool {
        value == 0.0
            && self.all_weights_are_non_negative_and_finite
            && self.all_layer_weight_sums_are_finite
            && self
                .minimum_positive_weight
                .is_none_or(|minimum| minimum * minimum > 0.0)
    }
}

pub(crate) fn cross_count_ix_controlled<N, E, G>(
    g: &Graph<N, E, G>,
    layering: &[Vec<usize>],
    work_control: &mut dyn WorkControl,
) -> Result<ControlledCrossCount, WorkError>
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    charge_nonzero(work_control, layering.len().saturating_sub(1))?;
    let mut cc: f64 = 0.0;
    let mut zero_domain = ZeroCrossingDomain::default();
    for i in 1..layering.len() {
        cc += two_layer_cross_count_ix_controlled(
            g,
            &layering[i - 1],
            &layering[i],
            &mut zero_domain,
            work_control,
        )?;
    }
    Ok(ControlledCrossCount {
        value: cc,
        is_proven_minimum: zero_domain.proves_zero_is_global_minimum(cc),
    })
}

fn two_layer_cross_count<N, E, G>(g: &Graph<N, E, G>, north: &[String], south: &[String]) -> f64
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    if south.is_empty() {
        return 0.0;
    }

    let mut south_pos: HashMap<&str, usize> = HashMap::default();
    for (i, v) in south.iter().enumerate() {
        south_pos.insert(v.as_str(), i);
    }

    let mut first_index: usize = 1;
    while first_index < south.len() {
        first_index <<= 1;
    }
    let tree_size = 2 * first_index - 1;
    first_index -= 1;
    let mut tree: Vec<f64> = vec![0.0; tree_size];

    let mut cc: f64 = 0.0;
    let mut entries: Vec<SouthEntry> = Vec::new();
    for v in north {
        entries.clear();
        g.for_each_out_edge(v, None, |ek, lbl| {
            let target = if g.is_directed() || ek.v.as_str() == v.as_str() {
                ek.w.as_str()
            } else {
                ek.v.as_str()
            };
            let Some(&pos) = south_pos.get(target) else {
                return;
            };
            entries.push(SouthEntry {
                pos,
                weight: lbl.weight(),
            });
        });
        if entries.len() > 1 {
            // Dagre's per-north stable sort preserves the graph's edge order for equal positions.
            entries.sort_by_key(|entry| entry.pos);
        }
        accumulate_crossings(&entries, &mut tree, first_index, &mut cc);
    }

    cc
}

fn charge_nonzero(work_control: &mut dyn WorkControl, units: usize) -> Result<(), WorkError> {
    if units == 0 {
        return Ok(());
    }
    work_control.charge(units)
}

fn two_layer_cross_count_ix_controlled<N, E, G>(
    g: &Graph<N, E, G>,
    north: &[usize],
    south: &[usize],
    zero_domain: &mut ZeroCrossingDomain,
    work_control: &mut dyn WorkControl,
) -> Result<f64, WorkError>
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    if south.is_empty() {
        return Ok(0.0);
    }

    charge_nonzero(work_control, south.len())?;
    let mut south_pos: HashMap<usize, usize> = HashMap::default();
    for (i, &v_ix) in south.iter().enumerate() {
        if v_ix == EMPTY_LAYER_SLOT {
            continue;
        }
        south_pos.insert(v_ix, i);
    }

    let mut first_index: usize = 1;
    while first_index < south.len() {
        first_index <<= 1;
    }
    let tree_size = checked_mul(first_index, 2)?
        .checked_sub(1)
        .ok_or(WorkError::ArithmeticOverflow)?;
    charge_nonzero(work_control, tree_size)?;
    first_index -= 1;
    let mut tree: Vec<f64> = vec![0.0; tree_size];

    let tree_depth = ceil_log2(south.len());
    let mut cc: f64 = 0.0;
    let mut entries: Vec<SouthEntry> = Vec::new();
    let mut participating_edge_count = Some(0usize);
    let mut maximum_weight = 0.0f64;
    charge_nonzero(work_control, north.len())?;
    for &v_ix in north {
        if v_ix == EMPTY_LAYER_SLOT {
            continue;
        }
        entries.clear();
        let mut push_entry = |target_ix: usize, lbl: &E| {
            let Some(&pos) = south_pos.get(&target_ix) else {
                return;
            };
            let weight = lbl.weight();
            zero_domain.observe_weight(weight);
            participating_edge_count =
                participating_edge_count.and_then(|count| count.checked_add(1));
            if weight.is_finite() && weight >= 0.0 {
                maximum_weight = maximum_weight.max(weight);
            }
            entries.push(SouthEntry { pos, weight });
        };
        if g.is_directed() {
            charge_nonzero(work_control, g.out_edge_count_ix(v_ix))?;
            g.for_each_out_edge_ix(v_ix, None, |_u_ix, target_ix, _ek, lbl| {
                push_entry(target_ix, lbl);
            });
        } else {
            charge_nonzero(work_control, g.undirected_edge_count_ix(v_ix))?;
            g.for_each_undirected_edge_ix(v_ix, |_node_ix, target_ix, _ek, lbl| {
                push_entry(target_ix, lbl);
            });
        }
        if entries.len() > 1 {
            charge_nonzero(work_control, checked_n_log_n(entries.len())?)?;
            // Dagre's per-north stable sort preserves the graph's edge order for equal positions.
            entries.sort_by_key(|entry| entry.pos);
        }
        let entry_work = checked_mul(entries.len(), checked_add(tree_depth, 2)?)?;
        charge_nonzero(work_control, entry_work)?;
        accumulate_crossings(&entries, &mut tree, first_index, &mut cc);
    }
    zero_domain.finish_layer(participating_edge_count, maximum_weight);

    Ok(cc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::GraphOptions;
    use crate::order::WeightLabel;

    #[derive(Default)]
    struct RecordingWorkControl {
        used: usize,
        max: usize,
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
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

    fn weighted_edge(weight: f64) -> WeightLabel {
        WeightLabel { weight }
    }

    #[test]
    fn streaming_entries_preserve_dagre_edge_and_float_order() {
        let mut graph: Graph<(), WeightLabel, ()> = Graph::new(GraphOptions {
            multigraph: true,
            ..GraphOptions::default()
        });
        for (name, weight) in [("first", 1e16), ("second", 1.0), ("third", 1.0)] {
            graph.set_edge_named(
                "north-a",
                "south-b",
                Some(name),
                Some(weighted_edge(weight)),
            );
        }
        graph.set_edge_with_label("north-b", "south-a", weighted_edge(1.0));

        let north = vec!["north-a".to_string(), "north-b".to_string()];
        let south = vec!["south-a".to_string(), "south-b".to_string()];
        let public = cross_count(&graph, &[north.clone(), south.clone()]);
        let layering = vec![
            north
                .iter()
                .map(|node| graph.node_ix(node).unwrap())
                .collect(),
            south
                .iter()
                .map(|node| graph.node_ix(node).unwrap())
                .collect(),
        ];
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        let actual = cross_count_ix_controlled(&graph, &layering, &mut work_control).unwrap();

        // Moving the large equal-position edge after both unit edges changes the rounded sum.
        assert_eq!(public.to_bits(), 1e16f64.to_bits());
        assert_eq!(actual.value.to_bits(), public.to_bits());
    }

    #[test]
    fn streaming_work_excludes_flatten_copy_and_trivial_sorts() {
        let mut graph: Graph<(), WeightLabel, ()> = Graph::new(GraphOptions::default());
        graph.set_edge_with_label("north-a", "south-c", weighted_edge(3.0));
        graph.set_edge_with_label("north-a", "south-a", weighted_edge(2.0));
        graph.set_edge_with_label("north-b", "outside", weighted_edge(7.0));
        graph.set_edge_with_label("north-c", "south-b", weighted_edge(5.0));
        let layering = vec![
            ["north-a", "north-b", "north-c"]
                .map(|node| graph.node_ix(node).unwrap())
                .to_vec(),
            ["south-a", "south-b", "south-c"]
                .map(|node| graph.node_ix(node).unwrap())
                .to_vec(),
        ];
        let mut exact = RecordingWorkControl {
            max: 32,
            ..Default::default()
        };

        let crossing_count = cross_count_ix_controlled(&graph, &layering, &mut exact).unwrap();

        assert_eq!(crossing_count.value, 15.0);
        assert_eq!(exact.used, 32);

        let mut below = RecordingWorkControl {
            max: 31,
            ..Default::default()
        };
        assert_eq!(
            cross_count_ix_controlled(&graph, &layering, &mut below),
            Err(WorkError::Interrupted)
        );
        assert_eq!(below.used, 28);
    }

    #[test]
    fn zero_minimum_proof_rejects_underflowing_positive_weights() {
        let mut graph: Graph<(), WeightLabel, ()> = Graph::new(GraphOptions::default());
        graph.set_edge_with_label("north-a", "south-b", weighted_edge(f64::MIN_POSITIVE));
        graph.set_edge_with_label("north-b", "south-a", weighted_edge(f64::MIN_POSITIVE));
        let layering = vec![
            ["north-a", "north-b"]
                .map(|node| graph.node_ix(node).unwrap())
                .to_vec(),
            ["south-a", "south-b"]
                .map(|node| graph.node_ix(node).unwrap())
                .to_vec(),
        ];
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        let result = cross_count_ix_controlled(&graph, &layering, &mut work_control).unwrap();

        assert_eq!(result.value, 0.0);
        assert!(!result.is_proven_minimum);
    }

    #[test]
    fn overflowing_layer_disables_the_shortcut_before_a_nan_reordering() {
        let mut graph: Graph<(), WeightLabel, ()> = Graph::new(GraphOptions::default());
        graph.set_edge_with_label("zero", "south-a", weighted_edge(0.0));
        graph.set_edge_with_label("huge-a", "south-b", weighted_edge(f64::MAX));
        graph.set_edge_with_label("huge-b", "south-b", weighted_edge(f64::MAX));
        let south = ["south-a", "south-b"]
            .map(|node| graph.node_ix(node).unwrap())
            .to_vec();
        let non_crossing = ["zero", "huge-a", "huge-b"]
            .map(|node| graph.node_ix(node).unwrap())
            .to_vec();
        let crossing = ["huge-a", "huge-b", "zero"]
            .map(|node| graph.node_ix(node).unwrap())
            .to_vec();
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        let initial =
            cross_count_ix_controlled(&graph, &[non_crossing, south.clone()], &mut work_control)
                .unwrap();
        let reordered =
            cross_count_ix_controlled(&graph, &[crossing, south], &mut work_control).unwrap();

        assert_eq!(initial.value, 0.0);
        assert!(!initial.is_proven_minimum);
        assert!(reordered.value.is_nan());
    }

    #[test]
    fn undirected_crossings_use_the_incident_endpoint_in_both_scorers() {
        let mut graph: Graph<(), WeightLabel, ()> = Graph::new(GraphOptions {
            directed: false,
            ..GraphOptions::default()
        });
        // Both queried north nodes are the canonical `w` endpoint of their undirected edge.
        graph.set_edge_with_label("b", "z", weighted_edge(1.0));
        graph.set_edge_with_label("a", "y", weighted_edge(1.0));
        let north = ["z", "y"];
        let south = ["a", "b"];
        let public_layering = vec![
            north.map(str::to_string).to_vec(),
            south.map(str::to_string).to_vec(),
        ];
        let indexed_layering = vec![
            north.map(|node| graph.node_ix(node).unwrap()).to_vec(),
            south.map(|node| graph.node_ix(node).unwrap()).to_vec(),
        ];
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        let controlled =
            cross_count_ix_controlled(&graph, &indexed_layering, &mut work_control).unwrap();

        assert_eq!(cross_count(&graph, &public_layering), 1.0);
        assert_eq!(controlled.value, 1.0);
        assert!(!controlled.is_proven_minimum);
    }
}
