use super::OrderEdgeWeight;
use crate::graphlib::Graph;
use crate::work::{ceil_log2, checked_add, checked_mul, checked_n_log_n};
use crate::{WorkControl, WorkError};
use rustc_hash::FxHashMap as HashMap;

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

pub(crate) struct ControlledCrossCount {
    pub value: f64,
    pub is_proven_minimum: bool,
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
    let mut cc: f64 = 0.0;
    let mut all_weights_are_non_negative_and_finite = true;
    for i in 1..layering.len() {
        let layer =
            two_layer_cross_count_ix_controlled(g, &layering[i - 1], &layering[i], work_control)?;
        cc += layer.value;
        all_weights_are_non_negative_and_finite &= layer.all_weights_are_non_negative_and_finite;
    }
    Ok(ControlledCrossCount {
        value: cc,
        is_proven_minimum: cc == 0.0 && all_weights_are_non_negative_and_finite,
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

    #[derive(Debug, Clone)]
    struct SouthEntry {
        pos: usize,
        weight: f64,
    }

    let mut south_entries: Vec<SouthEntry> = Vec::new();
    for v in north {
        let mut entries: Vec<SouthEntry> = Vec::new();
        g.for_each_out_edge(v, None, |ek, lbl| {
            let Some(&pos) = south_pos.get(ek.w.as_str()) else {
                return;
            };
            entries.push(SouthEntry {
                pos,
                weight: lbl.weight(),
            });
        });
        entries.sort_by_key(|e| e.pos);
        south_entries.extend(entries);
    }

    let mut first_index: usize = 1;
    while first_index < south.len() {
        first_index <<= 1;
    }
    let tree_size = 2 * first_index - 1;
    first_index -= 1;
    let mut tree: Vec<f64> = vec![0.0; tree_size];

    let mut cc: f64 = 0.0;
    for entry in south_entries {
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
        cc += entry.weight * weight_sum;
    }

    cc
}

fn charge_nonzero(work_control: &mut dyn WorkControl, units: usize) -> Result<(), WorkError> {
    if units == 0 {
        return Ok(());
    }
    work_control.charge(units)
}

struct ControlledTwoLayerCrossCount {
    value: f64,
    all_weights_are_non_negative_and_finite: bool,
}

fn two_layer_cross_count_ix_controlled<N, E, G>(
    g: &Graph<N, E, G>,
    north: &[usize],
    south: &[usize],
    work_control: &mut dyn WorkControl,
) -> Result<ControlledTwoLayerCrossCount, WorkError>
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    if south.is_empty() {
        return Ok(ControlledTwoLayerCrossCount {
            value: 0.0,
            all_weights_are_non_negative_and_finite: true,
        });
    }

    charge_nonzero(work_control, south.len())?;
    let mut south_pos: HashMap<usize, usize> = HashMap::default();
    for (i, &v_ix) in south.iter().enumerate() {
        south_pos.insert(v_ix, i);
    }

    #[derive(Debug, Clone)]
    struct SouthEntry {
        pos: usize,
        weight: f64,
    }

    let mut south_entries: Vec<SouthEntry> = Vec::new();
    let mut all_weights_are_non_negative_and_finite = true;
    charge_nonzero(work_control, north.len())?;
    for &v_ix in north {
        let mut entries: Vec<SouthEntry> = Vec::new();
        charge_nonzero(work_control, g.out_edge_count_ix(v_ix))?;
        g.for_each_out_edge_ix(v_ix, None, |_u_ix, w_ix, _ek, lbl| {
            let Some(&pos) = south_pos.get(&w_ix) else {
                return;
            };
            let weight = lbl.weight();
            all_weights_are_non_negative_and_finite &= weight.is_finite() && weight >= 0.0;
            entries.push(SouthEntry { pos, weight });
        });
        charge_nonzero(work_control, checked_n_log_n(entries.len())?)?;
        entries.sort_by_key(|e| e.pos);
        charge_nonzero(work_control, entries.len())?;
        south_entries.extend(entries);
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
    let entry_work = checked_mul(south_entries.len(), checked_add(tree_depth, 2)?)?;
    charge_nonzero(work_control, entry_work)?;
    let mut cc: f64 = 0.0;
    for entry in south_entries {
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
        cc += entry.weight * weight_sum;
    }

    Ok(ControlledTwoLayerCrossCount {
        value: cc,
        all_weights_are_non_negative_and_finite,
    })
}
