use super::*;
use indexmap::IndexMap;

struct RowLabel {
    id: String,
    rank: usize,
    lane: Option<String>,
    before: bool,
}

struct GapLabel {
    id: String,
    start: String,
    end: String,
    gap: usize,
    crosses_lanes: bool,
}

pub(super) struct LabelPlacement {
    rows: Vec<RowLabel>,
    gaps: IndexMap<(usize, Option<String>), Vec<GapLabel>>,
    unranked: Vec<(String, String)>,
    row_nodes: HashMap<(usize, Option<String>), Vec<String>>,
}

impl LabelPlacement {
    pub fn new(layout: &WorkingLayout, layers: &[Vec<String>], columns: &[Option<String>]) -> Self {
        let ranks: HashMap<&str, usize> = layers
            .iter()
            .enumerate()
            .flat_map(|(rank, layer)| layer.iter().map(move |id| (id.as_str(), rank)))
            .collect();
        let mut row_nodes: HashMap<(usize, Option<String>), Vec<String>> = HashMap::new();
        for (rank, layer) in layers.iter().enumerate() {
            for id in layer {
                row_nodes
                    .entry((rank, layout.top_lane_of(id).map(str::to_string)))
                    .or_default()
                    .push(id.clone());
            }
        }
        let mut placement = Self {
            rows: Vec::new(),
            gaps: IndexMap::new(),
            unranked: Vec::new(),
            row_nodes,
        };
        for edge in &layout.original_edges {
            let Some(id) = edge
                .label_node_id
                .as_ref()
                .filter(|id| layout.nodes.contains_key(*id))
            else {
                continue;
            };
            let (Some(&source_rank), Some(&target_rank)) =
                (ranks.get(edge.from.as_str()), ranks.get(edge.to.as_str()))
            else {
                placement.unranked.push((
                    id.clone(),
                    if ranks.contains_key(edge.from.as_str()) {
                        edge.from.clone()
                    } else {
                        edge.to.clone()
                    },
                ));
                continue;
            };
            let lane = layout.top_lane_of(&edge.to).map(str::to_string);
            if source_rank == target_rank {
                let column = |node_id: &str| {
                    columns
                        .iter()
                        .position(|column| column.as_deref() == layout.top_lane_of(node_id))
                };
                placement.rows.push(RowLabel {
                    id: id.clone(),
                    rank: target_rank,
                    lane,
                    before: edge.from != edge.to && column(&edge.from) < column(&edge.to),
                });
            } else {
                let lower = source_rank.min(target_rank);
                let upper = source_rank.max(target_rank);
                let gap = lower + (upper - lower - 1) / 2;
                placement
                    .gaps
                    .entry((gap, lane))
                    .or_default()
                    .push(GapLabel {
                        id: id.clone(),
                        start: edge.from.clone(),
                        end: edge.to.clone(),
                        gap,
                        crosses_lanes: layout.top_lane_of(&edge.from)
                            != layout.top_lane_of(&edge.to),
                    });
            }
        }
        placement
    }

    pub fn reserve_flow(&self, layout: &WorkingLayout, heights: &mut [f64], gaps: &mut [f64]) {
        for row in &self.rows {
            heights[row.rank] = heights[row.rank].max(layout.nodes[&row.id].height);
        }
        let horizontal = matches!(
            layout.direction,
            SwimlaneDirection::Lr | SwimlaneDirection::Rl
        );
        let stretch = if horizontal {
            super::super::direction::flow_axis_stretch(layout)
        } else {
            1.0
        };
        let mut label_gaps = vec![0.0_f64; gaps.len()];
        for label in self.gaps.values().flatten() {
            let node = &layout.nodes[&label.id];
            let extent = if horizontal { node.width } else { node.height };
            label_gaps[label.gap] = label_gaps[label.gap]
                .max((extent + 2.0 * super::super::config::EDGE_LABEL_CLEARANCE) / stretch);
        }
        for (gap, label_gap) in gaps.iter_mut().zip(label_gaps) {
            *gap += label_gap;
        }
    }

    pub fn reserve_lanes(
        &self,
        layout: &WorkingLayout,
        node_gap: f64,
        widths: &mut HashMap<Option<String>, f64>,
    ) {
        for ((_, lane), labels) in &self.gaps {
            if lane.is_none() {
                continue;
            }
            let span = labels
                .iter()
                .map(|label| layout.nodes[&label.id].width)
                .sum::<f64>()
                + node_gap * labels.len().saturating_sub(1) as f64;
            let width = widths.entry(lane.clone()).or_default();
            *width = width.max(span);
        }
        let mut spans: HashMap<(usize, Option<String>, bool), f64> = HashMap::new();
        for row in &self.rows {
            *spans
                .entry((row.rank, row.lane.clone(), row.before))
                .or_default() += node_gap + layout.nodes[&row.id].width;
        }
        for ((rank, lane), nodes) in &self.row_nodes {
            let before = spans
                .get(&(*rank, lane.clone(), true))
                .copied()
                .unwrap_or(0.0);
            let after = spans
                .get(&(*rank, lane.clone(), false))
                .copied()
                .unwrap_or(0.0);
            if before == 0.0 && after == 0.0 {
                continue;
            }
            let span = nodes.iter().map(|id| layout.nodes[id].width).sum::<f64>()
                + node_gap * nodes.len().saturating_sub(1) as f64;
            let width = widths.entry(lane.clone()).or_default();
            *width = width.max(span + 2.0 * before.max(after));
        }
    }

    pub fn place(&self, layout: &mut WorkingLayout, gap_centers: &[f64], node_gap: f64) {
        let mut row_edges: HashMap<(usize, Option<String>, bool), f64> = HashMap::new();
        for row in &self.rows {
            let Some(ids) = self.row_nodes.get(&(row.rank, row.lane.clone())) else {
                continue;
            };
            let Some(first) = ids.first() else {
                continue;
            };
            let y = layout.nodes[first].y;
            let row_edge = row_edges
                .entry((row.rank, row.lane.clone(), row.before))
                .or_insert_with(|| {
                    ids.iter()
                        .map(|id| {
                            let node = &layout.nodes[id];
                            if row.before {
                                node.x - node.width / 2.0
                            } else {
                                node.x + node.width / 2.0
                            }
                        })
                        .reduce(|left, right| {
                            if row.before {
                                left.min(right)
                            } else {
                                left.max(right)
                            }
                        })
                        .unwrap_or(0.0)
                });
            let node = layout.nodes.get_mut(&row.id).expect("measured edge label");
            let sign = if row.before { -1.0 } else { 1.0 };
            node.x = *row_edge + sign * (node_gap + node.width / 2.0);
            node.y = y;
            *row_edge = node.x + sign * node.width / 2.0;
        }
        for labels in self.gaps.values() {
            let wanted: Vec<_> = labels
                .iter()
                .map(|label| {
                    let source = &layout.nodes[&label.start];
                    let target = &layout.nodes[&label.end];
                    if label.crosses_lanes {
                        target.x
                    } else {
                        (source.x + target.x) / 2.0
                    }
                })
                .collect();
            let mut order: Vec<_> = (0..labels.len()).collect();
            order.sort_by(|left, right| {
                wanted[*left]
                    .total_cmp(&wanted[*right])
                    .then_with(|| left.cmp(right))
            });
            let mut placed = vec![0.0; labels.len()];
            let mut right = f64::NEG_INFINITY;
            for index in order {
                let width = layout.nodes[&labels[index].id].width;
                placed[index] = wanted[index].max(right + node_gap + width / 2.0);
                right = placed[index] + width / 2.0;
            }
            let shift =
                (wanted.iter().sum::<f64>() - placed.iter().sum::<f64>()) / labels.len() as f64;
            for (index, label) in labels.iter().enumerate() {
                let node = layout
                    .nodes
                    .get_mut(&label.id)
                    .expect("measured edge label");
                node.x = placed[index] + shift;
                node.y = gap_centers[label.gap];
            }
        }
        for (id, near) in &self.unranked {
            let (x, y) = layout
                .nodes
                .get(near)
                .map_or((0.0, 0.0), |node| (node.x, node.y));
            if let Some(label) = layout.nodes.get_mut(id) {
                label.x = x;
                label.y = y;
            }
        }
    }
}
