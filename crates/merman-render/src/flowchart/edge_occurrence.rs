use super::FlowEdge;
use std::collections::{HashMap, HashSet};

/// Operation-local identity for a semantic edge or one of its retained Dagre helper segments.
///
/// Mermaid permits distinct semantic edges to share the same public `edge.id`. Internal layout,
/// style, and SVG caches must therefore use the stable semantic model index instead of the raw
/// DOM id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FlowchartEdgeKey {
    semantic_index: usize,
    self_loop_order: Option<u8>,
}

impl FlowchartEdgeKey {
    pub(crate) const fn new(semantic_index: usize) -> Self {
        Self {
            semantic_index,
            self_loop_order: None,
        }
    }

    pub(crate) const fn self_loop_segment(semantic_index: usize, order: u8) -> Self {
        Self {
            semantic_index,
            self_loop_order: Some(order),
        }
    }

    pub(crate) const fn semantic_index(self) -> usize {
        self.semantic_index
    }

    #[cfg(test)]
    pub(crate) const fn semantic_key(self) -> Self {
        Self::new(self.semantic_index)
    }

    pub(crate) const fn self_loop_order(self) -> Option<u8> {
        self.self_loop_order
    }

    pub(crate) fn label_for_render<'a>(
        self,
        model: &'a super::FlowchartRenderModelRef<'_>,
        edge: &'a FlowEdge,
    ) -> Option<&'a str> {
        match self.self_loop_order {
            Some(0 | 2) => edge.label.as_deref(),
            _ => model.edge_label_for_render(self.semantic_index, edge),
        }
    }

    fn adapter_id(self) -> String {
        let base = format!("__merman_flowchart_edge_{:020}", self.semantic_index);
        match self.self_loop_order {
            Some(order) => format!("{base}_segment_{order}"),
            None => base,
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

        let mut seen = std::collections::HashSet::with_capacity(self.len());
        for (layout_edge, key) in layout_edges.iter().zip(self.iter()) {
            if !seen.insert(key) {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Flowchart layout contains multiple edges for semantic owner {}",
                        key.semantic_index()
                    ),
                });
            }
            let Some(semantic_edge) = semantic_edges.get(key.semantic_index()) else {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Flowchart layout edge `{}` references missing semantic owner {}",
                        layout_edge.id,
                        key.semantic_index()
                    ),
                });
            };
            let expected_id = match key.self_loop_order {
                None => std::borrow::Cow::Borrowed(semantic_edge.id.as_str()),
                Some(order) if semantic_edge.from == semantic_edge.to => {
                    let suffix = match order {
                        0 => "1",
                        1 => "mid",
                        2 => "2",
                        _ => {
                            return Err(crate::Error::InvalidModel {
                                message: format!(
                                    "invalid Flowchart self-loop segment order {order}"
                                ),
                            });
                        }
                    };
                    std::borrow::Cow::Owned(format!(
                        "{}-cyclic-special-{suffix}",
                        semantic_edge.from
                    ))
                }
                Some(_) => {
                    return Err(crate::Error::InvalidModel {
                        message: format!(
                            "Flowchart helper segment references non-self-loop owner {}",
                            key.semantic_index()
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
                        semantic_edge.id
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
    }

    #[test]
    fn owner_ledger_accepts_distinct_occurrences_with_the_same_raw_id() {
        let semantic = vec![semantic_edge("same"), semantic_edge("same")];
        let layout = vec![layout_edge("same"), layout_edge("same")];
        let owners =
            FlowchartEdgeOwners::new(vec![FlowchartEdgeKey::new(0), FlowchartEdgeKey::new(1)]);

        owners.validate(&layout, &semantic).unwrap();
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

    #[test]
    fn owner_ledger_distinguishes_retained_self_loop_segments_from_their_semantic_owner() {
        let mut semantic = semantic_edge("loop");
        semantic.from = "A".to_owned();
        semantic.to = "A".to_owned();
        let layouts = [
            "A-cyclic-special-1",
            "A-cyclic-special-mid",
            "A-cyclic-special-2",
        ]
        .map(layout_edge);
        let owners = FlowchartEdgeOwners::new(
            (0..3)
                .map(|order| FlowchartEdgeKey::self_loop_segment(0, order))
                .collect(),
        );
        owners
            .validate(&layouts, std::slice::from_ref(&semantic))
            .unwrap();
        assert_eq!(owners.iter().collect::<HashSet<_>>().len(), 3);
        assert!(
            owners
                .iter()
                .all(|key| key.semantic_key() == FlowchartEdgeKey::new(0))
        );

        let mut invalid = layouts.clone();
        invalid[0].id = semantic.id.clone();
        assert!(
            owners
                .validate(&invalid, std::slice::from_ref(&semantic))
                .is_err()
        );
        semantic.to = "B".to_owned();
        assert!(owners.validate(&layouts, &[semantic]).is_err());
    }
}
