use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use merman_model::families::venn::VennDiagramModel;
use merman_render_core::{Bounds as LayoutBounds, Error, Result, resources::OperationWorkMeter};

use super::config::VennConfigView;
use super::model::{
    VennAreaLayout, VennCircleLayout, VennDiagramLayout, VennTextAreaLayout,
    VennTextDebugCellLayout, VennTextNodeLayout,
};
use super::solver::{VennArea, VennLayoutArea, VennLayoutOptions, compute_venn_layout};

const REFERENCE_WIDTH: f64 = 1600.0;

/// Pixel sizes shared by layout and the SVG labels.
#[derive(Clone, Copy)]
pub struct VennFontSizes {
    pub title: f64,
    pub area: f64,
    text_node: f64,
}

impl VennFontSizes {
    pub fn new(native_base: Option<f64>, scale: f64) -> Self {
        native_base.map_or(
            Self {
                title: 32.0 * scale,
                area: 48.0 * scale,
                text_node: 40.0 * scale,
            },
            |base| {
                let body = 2.0 * scale * base;
                Self {
                    title: body * 1.25,
                    area: body,
                    text_node: body,
                }
            },
        )
    }
}

/// Prepares Venn geometry under the caller's existing operation work budget.
pub fn layout_venn_diagram_typed_with_work_meter(
    model: &VennDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &serde_json::Value,
    native_base_size: Option<f64>,
    work_meter: &OperationWorkMeter,
) -> Result<VennDiagramLayout> {
    let cfg = VennConfigView::new(effective_config).layout_settings();
    let title = model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        });
    let scale = cfg.width / REFERENCE_WIDTH;
    let font_sizes = VennFontSizes::new(native_base_size, scale);
    let title_height = if title.is_none() {
        0.0
    } else if native_base_size.is_some() {
        font_sizes.title * 1.5
    } else {
        48.0 * scale
    };
    let diagram_height = (cfg.height - title_height).max(1.0);

    let areas = ensure_pairwise_subsets_for_layout(
        model
            .subsets
            .iter()
            .map(|subset| VennArea {
                sets: subset.sets.clone(),
                size: subset.size,
                weight: None,
                label: subset.label.clone(),
            })
            .collect::<Vec<_>>(),
        work_meter,
    )?;
    let layout_areas = if areas.is_empty() {
        Vec::new()
    } else {
        let layout_options = VennLayoutOptions {
            width: cfg.width,
            height: diagram_height,
            padding: cfg.padding,
            ..Default::default()
        };
        work_meter.charge(venn_layout_work_units(&areas, &layout_options))?;
        compute_venn_layout(&areas, &layout_options).map_err(|err| Error::InvalidModel {
            message: err.to_string(),
        })?
    };

    let areas = layout_areas
        .iter()
        .map(|area| VennAreaLayout {
            sets: area.data.sets.clone(),
            size: area.data.size,
            label: area.data.label.clone(),
            text_x: area.text.x.floor(),
            text_y: area.text.y.floor(),
            text_disjoint: area.text.disjoint,
            circles: area
                .circles
                .iter()
                .map(|circle| VennCircleLayout {
                    set: circle.set.clone(),
                    x: circle.x,
                    y: circle.y,
                    radius: circle.radius,
                })
                .collect(),
            path: area.path.clone(),
        })
        .collect::<Vec<_>>();
    let layout_by_key = layout_areas
        .iter()
        .map(|area| (stable_sets_key(&area.data.sets), area))
        .collect::<HashMap<_, _>>();
    let (text_areas, text_nodes) = layout_text_nodes(
        model,
        &layout_by_key,
        scale,
        native_base_size,
        cfg.use_debug_layout,
    );

    Ok(VennDiagramLayout {
        bounds: Some(LayoutBounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: cfg.width,
            max_y: cfg.height,
        }),
        width: cfg.width,
        height: cfg.height,
        diagram_height,
        title_height,
        scale,
        padding: cfg.padding,
        use_max_width: cfg.use_max_width,
        use_debug_layout: cfg.use_debug_layout,
        areas,
        text_areas,
        text_nodes,
    })
}

fn layout_text_nodes(
    model: &VennDiagramModel,
    layout_by_key: &HashMap<String, &VennLayoutArea>,
    scale: f64,
    native_base_size: Option<f64>,
    use_debug_layout: bool,
) -> (Vec<VennTextAreaLayout>, Vec<VennTextNodeLayout>) {
    let mut nodes_by_area: IndexMap<String, Vec<&merman_model::families::venn::VennTextNodeModel>> =
        IndexMap::new();
    for node in &model.text_nodes {
        nodes_by_area
            .entry(stable_sets_key(&node.sets))
            .or_default()
            .push(node);
    }

    let mut text_areas = Vec::new();
    let mut text_nodes = Vec::new();
    for (key, nodes) in nodes_by_area {
        let Some(area) = layout_by_key.get(&key).copied() else {
            continue;
        };
        if area.circles.is_empty() {
            continue;
        }

        let center_x = area.text.x;
        let center_y = area.text.y;
        let min_circle_radius = area
            .circles
            .iter()
            .map(|circle| circle.radius)
            .fold(f64::INFINITY, f64::min);
        let inner_radius_raw = area
            .circles
            .iter()
            .map(|circle| {
                let dx = center_x - circle.x;
                let dy = center_y - circle.y;
                circle.radius - libm::sqrt(dx * dx + dy * dy)
            })
            .fold(f64::INFINITY, f64::min);
        let mut inner_radius = if inner_radius_raw.is_finite() {
            inner_radius_raw.max(0.0)
        } else {
            0.0
        };
        if inner_radius == 0.0 && min_circle_radius.is_finite() {
            inner_radius = min_circle_radius * 0.6;
        }

        let inner_width = (80.0 * scale).max(inner_radius * 2.0 * 0.95);
        let inner_height = (60.0 * scale).max(inner_radius * 2.0 * 0.95);
        let has_label = area
            .data
            .label
            .as_deref()
            .is_some_and(|label| !label.is_empty());
        let font_sizes = VennFontSizes::new(native_base_size, scale);
        let label_offset_base = if has_label {
            let label_offset = if native_base_size.is_some() {
                font_sizes.area * (2.0 / 3.0)
            } else {
                32.0 * scale
            };
            label_offset.min(inner_radius * 0.25)
        } else {
            0.0
        };
        let label_offset = label_offset_base + if nodes.len() <= 2 { 30.0 * scale } else { 0.0 };
        let start_x = center_x - inner_width / 2.0;
        let start_y = center_y - inner_height / 2.0 + label_offset;
        let cols = libm::sqrt(nodes.len() as f64).ceil().max(1.0) as usize;
        let rows = nodes.len().div_ceil(cols).max(1);
        let cell_width = inner_width / cols as f64;
        let cell_height = inner_height / rows as f64;

        let mut debug_cells = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            let col = index % cols;
            let row = index / cols;
            let cell_x = start_x + cell_width * col as f64;
            let cell_y = start_y + cell_height * row as f64;
            if use_debug_layout {
                debug_cells.push(VennTextDebugCellLayout {
                    x: cell_x,
                    y: cell_y,
                    width: cell_width,
                    height: cell_height,
                });
            }

            let x = start_x + cell_width * (col as f64 + 0.5);
            let y = start_y + cell_height * (row as f64 + 0.5);
            let box_width = cell_width * 0.9;
            let box_height = cell_height * 0.9;
            text_nodes.push(VennTextNodeLayout {
                sets: node.sets.clone(),
                id: node.id.clone(),
                label: node.label.clone(),
                x: x - box_width / 2.0,
                y: y - box_height / 2.0,
                width: box_width,
                height: box_height,
            });
        }

        text_areas.push(VennTextAreaLayout {
            sets: area.data.sets.clone(),
            center_x,
            center_y,
            inner_radius,
            font_size: font_sizes.text_node,
            debug_cells,
        });
    }

    (text_areas, text_nodes)
}

fn stable_sets_key(sets: &[String]) -> String {
    sets.join("|")
}

fn ensure_pairwise_subsets_for_layout(
    mut areas: Vec<VennArea>,
    work_meter: &OperationWorkMeter,
) -> Result<Vec<VennArea>> {
    work_meter.preflight(areas.len())?;
    let mut existing_pairs = areas
        .iter()
        .filter(|area| area.sets.len() == 2)
        .map(|area| canonical_pair(&area.sets[0], &area.sets[1]))
        .collect::<HashSet<_>>();
    let singleton_sizes = areas
        .iter()
        .filter(|area| area.sets.len() == 1)
        .map(|area| (area.sets[0].clone(), area.size))
        .collect::<HashMap<_, _>>();
    let mut synthetic = Vec::new();

    for area in areas.iter().filter(|area| area.sets.len() >= 3) {
        let mut members = area.sets.clone();
        members.sort();
        let mut counted_members = Vec::<(String, usize)>::new();
        for member in members {
            if let Some((previous, count)) = counted_members.last_mut()
                && previous == &member
            {
                *count = count.saturating_add(1);
            } else {
                counted_members.push((member, 1));
            }
        }
        if counted_members.is_empty() {
            continue;
        }
        for left_index in 0..counted_members.len() {
            let right_start = if counted_members[left_index].1 >= 2 {
                left_index
            } else {
                left_index + 1
            };
            for right_index in right_start..counted_members.len() {
                let left = &counted_members[left_index].0;
                let right = &counted_members[right_index].0;
                let pair = canonical_pair(left, right);
                if existing_pairs.contains(&pair) {
                    continue;
                }
                work_meter.preflight(
                    areas
                        .len()
                        .saturating_add(synthetic.len())
                        .saturating_add(1),
                )?;
                existing_pairs.insert(pair);

                let size = singleton_sizes
                    .get(left)
                    .zip(singleton_sizes.get(right))
                    .map_or(2.5, |(left_size, right_size)| {
                        left_size.min(*right_size) / 4.0
                    });
                synthetic.push(VennArea {
                    sets: vec![left.clone(), right.clone()],
                    size,
                    weight: None,
                    label: Some(String::new()),
                });
            }
        }
    }

    areas.extend(synthetic);

    // `venn.js` also creates zero-sized pairwise areas for every pair of singleton sets inside
    // its layout kernel. Prove that expansion fits before entering the ported algorithm, without
    // allocating those private helper areas here or changing the public/rendered area list.
    let mut singleton_ids = singleton_sizes.keys().collect::<Vec<_>>();
    singleton_ids.sort();
    let mut expanded_area_count = areas.len();
    for left_index in 0..singleton_ids.len() {
        for right_index in left_index + 1..singleton_ids.len() {
            if existing_pairs.contains(&canonical_pair(
                singleton_ids[left_index],
                singleton_ids[right_index],
            )) {
                continue;
            }
            expanded_area_count = expanded_area_count.saturating_add(1);
            work_meter.preflight(expanded_area_count)?;
        }
    }

    Ok(areas)
}

/// Estimates the typed Venn optimizer and geometry work before entering the
/// source-backed layout kernel.
///
/// Constrained MDS runs when the expanded list has at least eight areas. Its gradient and the
/// final Nelder-Mead objective repeatedly inspect set pairs or areas. The estimate accounts for
/// both passes with saturating arithmetic. Trusted callers of the raw kernel supply their own
/// work policy.
fn venn_layout_work_units(areas: &[VennArea], options: &VennLayoutOptions) -> usize {
    let singleton_ids = areas
        .iter()
        .filter(|area| area.sets.len() == 1)
        .map(|area| area.sets[0].as_str())
        .collect::<HashSet<_>>();
    let set_count = singleton_ids.len();
    let pair_count = set_count.saturating_mul(set_count.saturating_sub(1)) / 2;

    // `add_missing_areas` may append every missing singleton pair. This is an upper bound, but
    // avoids allocating a second expanded vector solely for the preflight estimate.
    let area_count = areas.len().saturating_add(pair_count);

    // Constrained MDS evaluates every unordered set pair once per gradient iteration and restart.
    let mds_work = pair_count
        .saturating_mul(options.max_iterations)
        .saturating_mul(options.restarts);

    // Nelder-Mead evaluates the loss several times per iteration and once for the initial
    // simplex. The factor four covers the reflected/expanded/contracted/reduced branches while
    // the simplex term covers the 2N+1 initial points.
    let simplex_evaluations = set_count.saturating_mul(2).saturating_add(1);
    let objective_evaluations = options
        .max_iterations
        .saturating_mul(4)
        .saturating_add(simplex_evaluations);
    let optimizer_loss_work = area_count.saturating_mul(objective_evaluations);

    // Geometry and intersection-area construction run once after optimization. A set area with
    // many members can inspect all member pairs, so include the sum of squared area arities and
    // the pairwise cluster pass.
    let intersection_work = areas.iter().fold(0usize, |total, area| {
        total.saturating_add(area.sets.len().saturating_mul(area.sets.len()))
    });
    let geometry_work = intersection_work
        .saturating_add(pair_count.saturating_mul(4))
        .saturating_add(set_count.saturating_mul(set_count));

    area_count
        .saturating_add(pair_count)
        .saturating_add(mds_work)
        .saturating_add(optimizer_loss_work)
        .saturating_add(geometry_work)
}

fn canonical_pair(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_owned(), right.to_owned())
    } else {
        (right.to_owned(), left.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::families::venn::VennSubsetModel;
    use merman_render_core::resources::{RenderResourcePolicy, ResourceLimitId};

    fn subset(sets: &[&str], size: f64) -> VennSubsetModel {
        VennSubsetModel {
            sets: sets.iter().map(|set| (*set).to_string()).collect(),
            size,
            label: None,
        }
    }

    #[test]
    fn typed_layout_adds_render_only_pairwise_subsets_for_three_set_union() {
        let model = VennDiagramModel {
            subsets: vec![
                subset(&["A"], 12.0),
                subset(&["B"], 8.0),
                subset(&["A", "B", "C"], 1.0),
            ],
            ..Default::default()
        };
        let original_model = model.clone();

        let layout = layout_venn_diagram_typed_with_work_meter(
            &model,
            None,
            &serde_json::json!({}),
            None,
            &OperationWorkMeter::new(RenderResourcePolicy::interactive()),
        )
        .expect("three-set union layout");
        let pairwise = layout
            .areas
            .iter()
            .filter(|area| area.sets.len() == 2)
            .map(|area| (area.sets.join("|"), area.size, area.label.as_deref()))
            .collect::<Vec<_>>();

        assert_eq!(
            pairwise,
            vec![
                ("A|B".to_string(), 2.0, Some("")),
                ("A|C".to_string(), 2.5, Some("")),
                ("B|C".to_string(), 2.5, Some("")),
            ]
        );
        assert_eq!(model, original_model);
    }

    #[test]
    fn typed_layout_rejects_mds_optimizer_work_before_matrix_allocation() {
        let model = VennDiagramModel {
            subsets: (0..700)
                .map(|index| VennSubsetModel {
                    sets: vec![format!("S{index}")],
                    size: 1.0,
                    label: None,
                })
                .collect(),
            ..Default::default()
        };

        let error = layout_venn_diagram_typed_with_work_meter(
            &model,
            None,
            &serde_json::json!({}),
            None,
            &OperationWorkMeter::new(RenderResourcePolicy::interactive()),
        )
        .expect_err("large Venn optimizer must be rejected by layout work budget");
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected Venn layout resource limit");
        };

        assert_eq!(error.limit, "max_layout_work_units");
        assert!(error.actual > error.max);
    }

    #[test]
    fn pairwise_expansion_is_rejected_before_it_can_exceed_the_area_budget() {
        let members = (0..1_000).map(|index| format!("S{index}")).collect();
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 32)
            .unwrap();
        let work_meter = OperationWorkMeter::new(limits);
        let error = ensure_pairwise_subsets_for_layout(
            vec![VennArea {
                sets: members,
                size: 1.0,
                weight: None,
                label: None,
            }],
            &work_meter,
        )
        .unwrap_err();
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected Venn area resource limit");
        };

        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 33);
        assert_eq!(error.max, 32);
    }

    #[test]
    fn private_singleton_pair_expansion_is_included_in_the_area_budget() {
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 6)
            .unwrap();
        let work_meter = OperationWorkMeter::new(limits);
        let areas = ["A", "B", "C", "D"]
            .into_iter()
            .map(|set| VennArea::new([set], 10.0))
            .collect();
        let error = ensure_pairwise_subsets_for_layout(areas, &work_meter).unwrap_err();
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected Venn area resource limit");
        };

        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 7);
        assert_eq!(error.max, 6);
    }

    #[test]
    fn private_singleton_pair_expansion_accepts_the_exact_area_budget() {
        let areas = ["A", "B", "C", "D"]
            .into_iter()
            .map(|set| VennArea::new([set], 10.0))
            .collect::<Vec<_>>();
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 10)
            .unwrap();
        let work_meter = OperationWorkMeter::new(limits);

        let checked = ensure_pairwise_subsets_for_layout(areas.clone(), &work_meter)
            .expect("four source areas plus six private pairs fit exactly");
        assert_eq!(checked, areas);
        assert_eq!(work_meter.used(), 0);
    }

    #[test]
    fn duplicate_union_members_preserve_upstream_self_pairs_without_multiplying_work() {
        let work_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 6)
                .unwrap(),
        );
        let expanded = ensure_pairwise_subsets_for_layout(
            vec![VennArea::new(["A", "A", "B", "B", "C"], 1.0)],
            &work_meter,
        )
        .expect("five upstream pair constraints plus the source area fit exactly");
        assert_eq!(
            expanded
                .iter()
                .filter(|area| area.sets.len() == 2)
                .map(|area| stable_sets_key(&area.sets))
                .collect::<Vec<_>>(),
            ["A|A", "A|B", "A|C", "B|B", "B|C"]
        );
    }
}
