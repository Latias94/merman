//! Order and origins for the decoded source placement dictionary.

use std::ops::Range;

/// Source evidence for one `grid.placements` member. Values remain in the decoded JSON.
#[derive(Debug, Clone, Default)]
pub struct GridPlacementsSource {
    pub key_span: Option<Range<usize>>,
    pub entries: Vec<GridPlacementSource>,
}

#[derive(Debug, Clone)]
pub struct GridPlacementSource {
    pub id: String,
    pub key_span: Option<Range<usize>>,
    pub occurrences: Vec<GridPlacementOccurrenceSource>,
}

#[derive(Debug, Clone, Default)]
pub struct GridPlacementOccurrenceSource {
    /// A single object has no index. An array object retains its original index.
    pub index: Option<usize>,
    pub fields: Vec<GridPlacementFieldSource>,
}

#[derive(Debug, Clone)]
pub struct GridPlacementFieldSource {
    pub name: String,
    pub key_span: Option<Range<usize>>,
    /// A nonfinite JSON5 coordinate whose decoded JSON value is null.
    pub nonfinite: Option<f64>,
}

impl GridPlacementsSource {
    pub(crate) fn key_spans_mut(&mut self) -> impl Iterator<Item = &mut Option<Range<usize>>> {
        std::iter::once(&mut self.key_span).chain(self.entries.iter_mut().flat_map(|entry| {
            std::iter::once(&mut entry.key_span).chain(
                entry
                    .occurrences
                    .iter_mut()
                    .flat_map(|occurrence| occurrence.fields.iter_mut())
                    .map(|field| &mut field.key_span),
            )
        }))
    }
}
