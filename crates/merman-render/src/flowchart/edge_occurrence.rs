use super::FlowEdge;
use std::collections::{HashMap, HashSet};

/// Operation-local identity for one rendered Flowchart edge occurrence.
///
/// Mermaid permits distinct semantic edges to share the same public `edge.id`. Internal layout
/// and SVG caches must therefore use the semantic owner plus any generated self-loop segment.
/// Source styles and labels continue to use the stable semantic model index instead of raw DOM ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FlowchartEdgeKey {
    semantic_index: usize,
    self_loop_segment: Option<u8>,
}

impl FlowchartEdgeKey {
    pub(crate) const fn new(semantic_index: usize) -> Self {
        Self {
            semantic_index,
            self_loop_segment: None,
        }
    }

    pub(crate) const fn self_loop_segment(semantic_index: usize, order: u8) -> Self {
        Self {
            semantic_index,
            self_loop_segment: Some(order),
        }
    }

    pub(crate) const fn semantic_index(self) -> usize {
        self.semantic_index
    }

    pub(crate) const fn self_loop_segment_order(self) -> Option<u8> {
        self.self_loop_segment
    }

    fn adapter_id(self) -> String {
        match self.self_loop_segment {
            None => format!("__merman_flowchart_edge_{:020}", self.semantic_index),
            Some(order) => {
                format!(
                    "__merman_flowchart_edge_{:020}_segment_{order}",
                    self.semantic_index
                )
            }
        }
    }
}

/// Collision-safe adapter identities for one prepared set of semantic edge occurrences.
///
/// Unique Mermaid ids stay byte-identical so source-backed adapter ordering remains unchanged.
/// Only repeated ids enter the private occurrence namespace, and generated ids avoid every raw id
/// already present in the same model.
#[derive(Debug, Clone)]
pub(crate) struct FlowchartEdgeTransportPlan {
    ids: HashMap<FlowchartEdgeKey, String>,
}

impl FlowchartEdgeTransportPlan {
    pub(crate) fn for_semantic_edges(edges: &[FlowEdge]) -> Self {
        Self::for_keyed_edges(
            edges
                .iter()
                .enumerate()
                .map(|(index, edge)| (FlowchartEdgeKey::new(index), edge)),
        )
    }

    pub(crate) fn for_keyed_edges<'a>(
        edges: impl IntoIterator<Item = (FlowchartEdgeKey, &'a FlowEdge)>,
    ) -> Self {
        let entries = edges.into_iter().collect::<Vec<_>>();
        let mut counts = HashMap::<&str, usize>::with_capacity(entries.len());
        let mut reserved = HashSet::<&str>::with_capacity(entries.len());
        for (_, edge) in &entries {
            *counts.entry(edge.id.as_str()).or_default() += 1;
            reserved.insert(edge.id.as_str());
        }

        let mut ids = HashMap::with_capacity(entries.len());
        let mut used = HashSet::<String>::with_capacity(entries.len());
        for (key, edge) in entries {
            let id = if counts.get(edge.id.as_str()) == Some(&1) {
                edge.id.clone()
            } else {
                let base = key.adapter_id();
                let mut candidate = base.clone();
                let mut suffix = 0usize;
                while reserved.contains(candidate.as_str()) || used.contains(&candidate) {
                    candidate = format!("{base}_{suffix:020}");
                    suffix = suffix.saturating_add(1);
                }
                candidate
            };
            let inserted = used.insert(id.clone());
            debug_assert!(inserted, "transport ids must be unique");
            ids.insert(key, id);
        }

        Self { ids }
    }

    pub(crate) fn id(&self, key: FlowchartEdgeKey) -> Option<&str> {
        self.ids.get(&key).map(String::as_str)
    }
}

/// Atomic ownership ledger aligned with a prepared layout's renderable edges.
///
/// Keeping the typed keys behind this crate-private seam prevents public Mermaid ids and naked
/// `usize` sidecars from becoming competing sources of occurrence identity.
#[derive(Debug, Clone, Default)]
pub(crate) struct FlowchartEdgeOwners(Vec<FlowchartEdgeKey>);

impl FlowchartEdgeOwners {
    pub(crate) fn new(keys: Vec<FlowchartEdgeKey>) -> Self {
        Self(keys)
    }

    #[cfg(test)]
    pub(crate) fn from_semantic_indices(indices: impl IntoIterator<Item = usize>) -> Self {
        Self(indices.into_iter().map(FlowchartEdgeKey::new).collect())
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = FlowchartEdgeKey> + '_ {
        self.0.iter().copied()
    }

    pub(crate) fn validate(
        &self,
        layout_edges: &[crate::model::LayoutEdge],
        semantic_edges: &[FlowEdge],
    ) -> crate::Result<()> {
        if self.len() != layout_edges.len() {
            return Err(crate::Error::InvalidModel {
                message: format!(
                    "Flowchart layout edge-owner count {} does not match edge count {}",
                    self.len(),
                    layout_edges.len()
                ),
            });
        }

        let mut owner_occurrences = HashMap::<usize, u8>::with_capacity(self.len());
        for (layout_edge, key) in layout_edges.iter().zip(self.iter()) {
            let Some(semantic_edge) = semantic_edges.get(key.semantic_index()) else {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Flowchart layout edge `{}` references missing semantic owner {}",
                        layout_edge.id,
                        key.semantic_index()
                    ),
                });
            };
            let (expected_id, occurrence_bit) = match key.self_loop_segment_order() {
                None => (
                    std::borrow::Cow::Borrowed(semantic_edge.id.as_str()),
                    1_u8 << 3,
                ),
                Some(order @ 0..=2) if semantic_edge.from == semantic_edge.to => {
                    let suffix = match order {
                        0 => "1",
                        1 => "mid",
                        _ => "2",
                    };
                    (
                        std::borrow::Cow::Owned(format!(
                            "{}-cyclic-special-{suffix}",
                            semantic_edge.from
                        )),
                        1_u8 << order,
                    )
                }
                Some(_) => {
                    return Err(crate::Error::InvalidModel {
                        message: format!(
                            "Flowchart layout has an invalid self-loop segment occurrence {key:?}"
                        ),
                    });
                }
            };
            if layout_edge.id != expected_id.as_ref() {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Flowchart layout edge `{}` is not bound to semantic owner {} (`{}`)",
                        layout_edge.id,
                        key.semantic_index(),
                        expected_id
                    ),
                });
            }
            let occurrences = owner_occurrences.entry(key.semantic_index()).or_default();
            if *occurrences & occurrence_bit != 0 {
                return Err(crate::Error::InvalidModel {
                    message: format!("Flowchart layout contains duplicate edge occurrence {key:?}"),
                });
            }
            *occurrences |= occurrence_bit;
        }
        for (semantic_index, occurrences) in owner_occurrences {
            if !matches!(occurrences, 0b0111 | 0b1000) {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Flowchart semantic owner {semantic_index} has incomplete or mixed render occurrences"
                    ),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LayoutEdge;

    fn semantic_edge(id: &str) -> FlowEdge {
        FlowEdge {
            id: id.to_string(),
            from: format!("{id}-from"),
            to: format!("{id}-to"),
            label: None,
            label_type: None,
            edge_type: None,
            arrow: String::new(),
            start_marker: Default::default(),
            end_marker: Default::default(),
            is_user_defined_id: false,
            stroke: None,
            stroke_kind: Default::default(),
            visibility: Default::default(),
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    fn layout_edge(id: &str) -> LayoutEdge {
        LayoutEdge {
            id: id.to_string(),
            from: format!("{id}-from"),
            to: format!("{id}-to"),
            from_cluster: None,
            to_cluster: None,
            points: Vec::new(),
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        }
    }

    #[test]
    fn owner_ledger_rejects_missing_out_of_range_and_duplicate_bindings() {
        let semantic = vec![semantic_edge("same"), semantic_edge("same")];
        let layout = vec![layout_edge("same"), layout_edge("same")];

        let missing = FlowchartEdgeOwners::new(vec![FlowchartEdgeKey::new(0)]);
        assert!(missing.validate(&layout, &semantic).is_err());

        let out_of_range =
            FlowchartEdgeOwners::new(vec![FlowchartEdgeKey::new(0), FlowchartEdgeKey::new(2)]);
        assert!(out_of_range.validate(&layout, &semantic).is_err());

        let duplicate =
            FlowchartEdgeOwners::new(vec![FlowchartEdgeKey::new(0), FlowchartEdgeKey::new(0)]);
        assert!(duplicate.validate(&layout, &semantic).is_err());

        let wrong_binding = FlowchartEdgeOwners::from_semantic_indices([0]);
        assert!(
            wrong_binding
                .validate(&[layout_edge("other")], &semantic)
                .is_err()
        );
    }

    #[test]
    fn owner_ledger_accepts_distinct_occurrences_with_the_same_raw_id() {
        let semantic = vec![semantic_edge("same"), semantic_edge("same")];
        let layout = vec![layout_edge("same"), layout_edge("same")];
        let owners =
            FlowchartEdgeOwners::new(vec![FlowchartEdgeKey::new(0), FlowchartEdgeKey::new(1)]);

        owners.validate(&layout, &semantic).unwrap();
    }

    fn self_loop_layout() -> (FlowEdge, Vec<LayoutEdge>) {
        let mut semantic = semantic_edge("loop");
        semantic.from = "A".to_owned();
        semantic.to = "A".to_owned();
        let helpers = super::super::flowchart_self_loop_helper_edges(&semantic);
        let layout = [helpers.edge1, helpers.edge_mid, helpers.edge2]
            .into_iter()
            .map(|helper| {
                let mut layout = layout_edge(&helper.id);
                layout.from = helper.from;
                layout.to = helper.to;
                layout
            })
            .collect();
        (semantic, layout)
    }

    #[test]
    fn self_loop_segments_have_distinct_occurrences_and_one_semantic_owner() {
        let (semantic, layout) = self_loop_layout();
        let keys = (0..3)
            .map(|order| FlowchartEdgeKey::self_loop_segment(0, order))
            .collect::<Vec<_>>();
        assert_eq!(keys.iter().copied().collect::<HashSet<_>>().len(), 3);
        assert!(keys.iter().all(|key| key.semantic_index() == 0));
        assert!(!keys.contains(&FlowchartEdgeKey::new(0)));
        FlowchartEdgeOwners::new(keys)
            .validate(&layout, &[semantic])
            .unwrap();
    }

    #[test]
    fn self_loop_owner_ledger_rejects_partial_mixed_duplicate_and_wrong_segments() {
        let (semantic, layout) = self_loop_layout();
        let keys = (0..3)
            .map(|order| FlowchartEdgeKey::self_loop_segment(0, order))
            .collect::<Vec<_>>();
        let semantic_edges = [semantic];
        for partial_count in [1, 2] {
            assert!(
                FlowchartEdgeOwners::new(keys[..partial_count].to_vec())
                    .validate(&layout[..partial_count], &semantic_edges)
                    .is_err()
            );
        }

        let mut mixed_layout = layout.clone();
        mixed_layout.push(layout_edge("loop"));
        let mut mixed_keys = keys.clone();
        mixed_keys.push(FlowchartEdgeKey::new(0));
        assert!(
            FlowchartEdgeOwners::new(mixed_keys)
                .validate(&mixed_layout, &semantic_edges)
                .is_err()
        );

        let mut duplicate_layout = layout.clone();
        duplicate_layout.push(layout[0].clone());
        let mut duplicate_keys = keys.clone();
        duplicate_keys.push(keys[0]);
        assert!(
            FlowchartEdgeOwners::new(duplicate_keys)
                .validate(&duplicate_layout, &semantic_edges)
                .is_err()
        );

        let mut invalid_keys = keys.clone();
        invalid_keys[0] = FlowchartEdgeKey::self_loop_segment(0, 3);
        assert!(
            FlowchartEdgeOwners::new(invalid_keys)
                .validate(&layout, &semantic_edges)
                .is_err()
        );

        let mut swapped_layout = layout.clone();
        swapped_layout.swap(0, 1);
        assert!(
            FlowchartEdgeOwners::new(keys)
                .validate(&swapped_layout, &semantic_edges)
                .is_err()
        );
    }

    #[test]
    fn authored_edges_cannot_claim_generated_self_loop_segment_identity() {
        let semantic = semantic_edge("A-cyclic-special-1");
        let layout = [layout_edge(&semantic.id)];
        assert!(
            FlowchartEdgeOwners::new(vec![FlowchartEdgeKey::self_loop_segment(0, 0)])
                .validate(&layout, &[semantic])
                .is_err()
        );
    }

    #[test]
    fn transport_plan_preserves_unique_ids_and_avoids_raw_id_collisions() {
        let edges = vec![
            semantic_edge("duplicate"),
            semantic_edge("duplicate"),
            semantic_edge("__merman_flowchart_edge_00000000000000000000"),
            semantic_edge("unique"),
        ];
        let plan = FlowchartEdgeTransportPlan::for_semantic_edges(&edges);
        let ids = (0..edges.len())
            .map(|index| plan.id(FlowchartEdgeKey::new(index)).expect("transport id"))
            .collect::<Vec<_>>();

        assert_eq!(ids[2], edges[2].id);
        assert_eq!(ids[3], edges[3].id);
        assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), ids.len());
        assert_ne!(ids[0], edges[2].id);
        assert_ne!(ids[1], edges[2].id);
    }
}
