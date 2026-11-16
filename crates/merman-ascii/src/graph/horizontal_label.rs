use super::model::GraphEdgeMarker;

/// The direct horizontal route puts both endpoint markers in the inter-node gap.
/// Structural markers occupy one cell in both terminal width profiles; Open occupies none.
#[derive(Debug, Clone, Copy)]
pub(super) struct HorizontalLabelPadding {
    source_marker_cells: usize,
    target_marker_cells: usize,
}

impl HorizontalLabelPadding {
    pub(super) fn new(source: GraphEdgeMarker, target: GraphEdgeMarker) -> Self {
        Self {
            source_marker_cells: usize::from(source != GraphEdgeMarker::Open),
            target_marker_cells: usize::from(target != GraphEdgeMarker::Open),
        }
    }

    pub(super) fn gap_cells(self) -> usize {
        // Keep two visible stroke cells on each side of an inline label.
        2 * 2 + self.source_marker_cells + self.target_marker_cells
    }
}
