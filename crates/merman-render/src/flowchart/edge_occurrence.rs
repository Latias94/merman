use super::FlowEdge;

/// Operation-local identity for one semantic Flowchart edge occurrence.
///
/// Mermaid permits distinct semantic edges to share the same public `edge.id`. Internal layout,
/// style, and SVG caches must therefore use the stable semantic model index instead of the raw
/// DOM id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FlowchartEdgeKey(usize);

impl FlowchartEdgeKey {
    pub(crate) const fn new(semantic_index: usize) -> Self {
        Self(semantic_index)
    }

    pub(crate) const fn semantic_index(self) -> usize {
        self.0
    }

    /// Unique adapter transport id. It never crosses the compatibility or SVG DOM boundary.
    pub(crate) fn adapter_id(self) -> String {
        format!("__merman_flowchart_edge_{:020}", self.0)
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
            if layout_edge.id != semantic_edge.id {
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
            is_user_defined_id: false,
            stroke: None,
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
}
