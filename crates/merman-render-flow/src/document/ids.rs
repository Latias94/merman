//! Flow SVG namespace and allocation-free generated identifiers.

use std::fmt;

use merman_render_core::svg_css::SvgId;

const DOCUMENT_NAMESPACE: &str = "merman-flowchart-document";
const SCOPE_SEPARATOR: &str = "-";
const EDGE_PREFIX: &str = "edge-";
const EDGE_SEGMENT_SEPARATOR: &str = "-segment-";

#[derive(Debug, Clone, Copy)]
enum FlowchartDocumentIdLocal {
    Static(&'static str),
    Indexed(&'static str, usize),
    EdgeSegment(usize, u8),
}

/// Allocation-free view of one generated ID under a normalized SVG diagram ID.
#[derive(Debug, Clone, Copy)]
pub struct FlowchartDocumentId<'a> {
    scope: &'a str,
    local: FlowchartDocumentIdLocal,
}

impl fmt::Display for FlowchartDocumentId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.scope)?;
        f.write_str(SCOPE_SEPARATOR)?;
        match self.local {
            FlowchartDocumentIdLocal::Static(suffix) => f.write_str(suffix),
            FlowchartDocumentIdLocal::Indexed(prefix, ordinal) => {
                f.write_str(prefix)?;
                fmt::Display::fmt(&ordinal, f)
            }
            FlowchartDocumentIdLocal::EdgeSegment(semantic_index, order) => {
                write!(
                    f,
                    "{EDGE_PREFIX}{semantic_index}{EDGE_SEGMENT_SEPARATOR}{order}"
                )
            }
        }
    }
}

impl FlowchartDocumentId<'_> {
    pub fn serialized_len(self) -> usize {
        fn decimal_len(value: usize) -> usize {
            value.checked_ilog10().unwrap_or(0) as usize + 1
        }

        let local_bytes = match self.local {
            FlowchartDocumentIdLocal::Static(suffix) => suffix.len(),
            FlowchartDocumentIdLocal::Indexed(prefix, ordinal) => {
                prefix.len() + decimal_len(ordinal)
            }
            FlowchartDocumentIdLocal::EdgeSegment(semantic_index, order) => {
                EDGE_PREFIX.len()
                    + decimal_len(semantic_index)
                    + EDGE_SEGMENT_SEPARATOR.len()
                    + decimal_len(usize::from(order))
            }
        };
        self.scope.len() + SCOPE_SEPARATOR.len() + local_bytes
    }
}

/// The one stored namespace for the graph, its document resources, and its references.
#[derive(Debug)]
pub struct FlowchartDocumentScope(String);

impl FlowchartDocumentScope {
    pub fn new(diagram_id: SvgId<'_>) -> Self {
        // Growing a namespace uses the logical ID, like the original graph inventory.
        // The original SvgId is still used when formatting projected root and CSS IDs.
        let diagram_id = diagram_id.semantic_str();
        let mut scope = String::with_capacity(diagram_id.len() + 1 + DOCUMENT_NAMESPACE.len());
        scope.push_str(diagram_id);
        scope.push('-');
        scope.push_str(DOCUMENT_NAMESPACE);
        Self(scope)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn id(&self, local: FlowchartDocumentIdLocal) -> FlowchartDocumentId<'_> {
        FlowchartDocumentId {
            scope: &self.0,
            local,
        }
    }

    pub(super) fn accessibility_title(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("a11y-title"))
    }

    pub(super) fn accessibility_description(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("a11y-description"))
    }

    pub fn drop_shadow(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("filter-drop-shadow"))
    }

    pub fn drop_shadow_small(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("filter-drop-shadow-small"))
    }

    pub fn root_gradient(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("gradient-root"))
    }

    pub fn edge(
        &self,
        semantic_index: usize,
        segment_order: Option<u8>,
    ) -> FlowchartDocumentId<'_> {
        self.id(match segment_order {
            None => FlowchartDocumentIdLocal::Indexed(EDGE_PREFIX, semantic_index),
            Some(order) => FlowchartDocumentIdLocal::EdgeSegment(semantic_index, order),
        })
    }

    pub fn node(&self, ordinal: usize) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Indexed("node-", ordinal))
    }

    pub fn subgraph_node(&self, ordinal: usize) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Indexed("node-subgraph-", ordinal))
    }

    pub fn cluster(&self, ordinal: usize) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Indexed("cluster-", ordinal))
    }

    pub fn lane(&self, ordinal: usize) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Indexed("lane-", ordinal))
    }

    pub fn synthetic_label(&self, ordinal: usize) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Indexed(
            "synthetic-label-",
            ordinal,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_id_serialized_len_matches_display_across_decimal_widths() {
        let scope = FlowchartDocumentScope::new(SvgId::new("diagram").unwrap());
        for ordinal in [0, 9, 10, 99, 100, usize::MAX] {
            for id in [
                scope.accessibility_title(),
                scope.edge(ordinal, None),
                scope.edge(ordinal, Some(0)),
                scope.edge(ordinal, Some(u8::MAX)),
            ] {
                assert_eq!(id.serialized_len(), id.to_string().len(), "{id}");
            }
        }
    }

    #[test]
    fn long_document_scope_backs_resource_and_graph_ids() {
        let diagram_id = format!("scope-v1_{}", "d".repeat(16 * 1024));
        let scope = FlowchartDocumentScope::new(SvgId::new(&diagram_id).unwrap());
        assert!(
            scope.0.capacity()
                <= (diagram_id.len() + DOCUMENT_NAMESPACE.len() + 1).next_power_of_two()
        );
        assert!(std::ptr::eq(scope.as_str(), scope.drop_shadow().scope));
        assert!(std::ptr::eq(scope.as_str(), scope.edge(12, Some(0)).scope));
    }
}
