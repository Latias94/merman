//! Operation-local ownership of generated Flowchart-family SVG identifiers.
//!
//! Semantic Mermaid identifiers remain observable through `data-id`/`data-et`. They are not used
//! as generated DOM identifiers because nodes, clusters, lanes, edges, accessibility metadata,
//! filters, markers, root-theme resources, and prepared-text labels all share one SVG namespace.

use super::render_input::FlowchartRenderEdgeRef;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt;

const DOCUMENT_NAMESPACE: &str = "merman-flowchart-document";

#[derive(Debug, Clone, Copy)]
enum FlowchartDocumentIdLocal {
    Static(&'static str),
    Indexed(&'static str, usize),
}

#[derive(Debug, Clone, Copy)]
enum FlowchartNodeDocumentId {
    Node(usize),
    Subgraph(usize),
}

/// Allocation-free view of one generated id.
///
/// `scope` is produced by `SvgRenderOptions::normalized()` and local fragments are fixed ASCII,
/// so the display form is already valid in XML id attributes and CSS URL fragments.
#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartDocumentId<'a> {
    scope: &'a str,
    local: FlowchartDocumentIdLocal,
}

impl fmt::Display for FlowchartDocumentId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.scope)?;
        f.write_str("-")?;
        match self.local {
            FlowchartDocumentIdLocal::Static(suffix) => f.write_str(suffix),
            FlowchartDocumentIdLocal::Indexed(prefix, ordinal) => {
                f.write_str(prefix)?;
                fmt::Display::fmt(&ordinal, f)
            }
        }
    }
}

#[derive(Debug)]
pub(in crate::svg::parity::flowchart) struct FlowchartDocumentIds<'a> {
    scope: String,
    edge_ids: FxHashSet<crate::flowchart::FlowchartEdgeKey>,
    node_ids: FxHashMap<&'a str, FlowchartNodeDocumentId>,
    cluster_ids: FxHashMap<&'a str, usize>,
    lane_ids: FxHashMap<&'a str, usize>,
    synthetic_label_ids: FxHashMap<&'a str, usize>,
}

pub(in crate::svg::parity::flowchart) struct FlowchartDocumentIdRequest<'plan, 'data> {
    pub(in crate::svg::parity::flowchart) diagram_id: &'plan str,
    pub(in crate::svg::parity::flowchart) edge_order: &'plan [FlowchartRenderEdgeRef<'data>],
    pub(in crate::svg::parity::flowchart) node_dom_index: &'plan FxHashMap<&'data str, usize>,
    pub(in crate::svg::parity::flowchart) subgraph_index_by_id: &'plan FxHashMap<&'data str, usize>,
    pub(in crate::svg::parity::flowchart) layout_nodes: &'data [crate::model::LayoutNode],
    pub(in crate::svg::parity::flowchart) swimlane_nodes:
        Option<&'data [crate::model::SwimlaneNodeLayout]>,
    pub(in crate::svg::parity::flowchart) swimlane_lanes:
        Option<&'data [crate::model::SwimlaneLaneLayout]>,
}

impl<'a> FlowchartDocumentIds<'a> {
    pub(in crate::svg::parity::flowchart) fn prepare(
        request: FlowchartDocumentIdRequest<'_, 'a>,
    ) -> Self {
        let mut scope =
            String::with_capacity(request.diagram_id.len() + 1 + DOCUMENT_NAMESPACE.len());
        scope.push_str(request.diagram_id);
        scope.push('-');
        scope.push_str(DOCUMENT_NAMESPACE);

        let edge_ids = request.edge_order.iter().map(|edge| edge.key).collect();

        let mut node_ids = FxHashMap::default();
        node_ids.reserve(request.node_dom_index.len() + request.subgraph_index_by_id.len());
        for (&raw_id, &dom_index) in request.node_dom_index {
            node_ids.insert(raw_id, FlowchartNodeDocumentId::Node(dom_index));
        }
        for (&raw_id, &subgraph_index) in request.subgraph_index_by_id {
            node_ids
                .entry(raw_id)
                .or_insert(FlowchartNodeDocumentId::Subgraph(subgraph_index));
        }

        let cluster_ids = request
            .subgraph_index_by_id
            .iter()
            .map(|(&raw_id, &subgraph_index)| (raw_id, subgraph_index))
            .collect();

        let lane_ids = request
            .swimlane_lanes
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(lane_index, lane)| (lane.id.as_str(), lane_index))
            .collect();

        let mut synthetic_label_ids = FxHashMap::default();
        let swimlane_node_count = request.swimlane_nodes.map_or(0, |nodes| nodes.len());
        synthetic_label_ids.reserve(request.layout_nodes.len() + swimlane_node_count);
        for (layout_index, node) in request.layout_nodes.iter().enumerate() {
            if !node_ids.contains_key(node.id.as_str()) {
                synthetic_label_ids
                    .entry(node.id.as_str())
                    .or_insert(layout_index);
            }
        }
        let swimlane_offset = request.layout_nodes.len();
        for (swimlane_index, node) in request.swimlane_nodes.into_iter().flatten().enumerate() {
            if node.is_edge_label && !node_ids.contains_key(node.id.as_str()) {
                synthetic_label_ids
                    .entry(node.id.as_str())
                    .or_insert(swimlane_offset + swimlane_index);
            }
        }

        Self {
            scope,
            edge_ids,
            node_ids,
            cluster_ids,
            lane_ids,
            synthetic_label_ids,
        }
    }

    fn id(&self, local: FlowchartDocumentIdLocal) -> FlowchartDocumentId<'_> {
        FlowchartDocumentId {
            scope: &self.scope,
            local,
        }
    }

    pub(in crate::svg::parity::flowchart) fn accessibility_title(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("a11y-title"))
    }

    pub(in crate::svg::parity::flowchart) fn accessibility_description(
        &self,
    ) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("a11y-description"))
    }

    pub(in crate::svg::parity::flowchart) fn drop_shadow(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("filter-drop-shadow"))
    }

    pub(in crate::svg::parity::flowchart) fn drop_shadow_small(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("filter-drop-shadow-small"))
    }

    pub(in crate::svg::parity::flowchart) fn root_gradient(&self) -> FlowchartDocumentId<'_> {
        self.id(FlowchartDocumentIdLocal::Static("gradient-root"))
    }

    pub(in crate::svg::parity::flowchart) fn marker_scope(&self) -> &str {
        &self.scope
    }

    pub(in crate::svg::parity::flowchart) fn edge(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> Option<FlowchartDocumentId<'_>> {
        self.edge_ids.contains(&key).then(|| {
            self.id(FlowchartDocumentIdLocal::Indexed(
                "edge-",
                key.semantic_index(),
            ))
        })
    }

    pub(in crate::svg::parity::flowchart) fn node(
        &self,
        raw_id: &str,
    ) -> Option<FlowchartDocumentId<'_>> {
        self.node_ids.get(raw_id).map(|&owner| match owner {
            FlowchartNodeDocumentId::Node(ordinal) => {
                self.id(FlowchartDocumentIdLocal::Indexed("node-", ordinal))
            }
            FlowchartNodeDocumentId::Subgraph(ordinal) => {
                self.id(FlowchartDocumentIdLocal::Indexed("node-subgraph-", ordinal))
            }
        })
    }

    pub(in crate::svg::parity::flowchart) fn cluster(
        &self,
        raw_id: &str,
    ) -> Option<FlowchartDocumentId<'_>> {
        self.cluster_ids
            .get(raw_id)
            .map(|&ordinal| self.id(FlowchartDocumentIdLocal::Indexed("cluster-", ordinal)))
    }

    pub(in crate::svg::parity::flowchart) fn lane(
        &self,
        raw_id: &str,
    ) -> Option<FlowchartDocumentId<'_>> {
        self.lane_ids
            .get(raw_id)
            .map(|&ordinal| self.id(FlowchartDocumentIdLocal::Indexed("lane-", ordinal)))
    }

    pub(in crate::svg::parity::flowchart) fn synthetic_label(
        &self,
        raw_id: &str,
    ) -> Option<FlowchartDocumentId<'_>> {
        self.synthetic_label_ids.get(raw_id).map(|&ordinal| {
            self.id(FlowchartDocumentIdLocal::Indexed(
                "synthetic-label-",
                ordinal,
            ))
        })
    }

    pub(in crate::svg::parity::flowchart) fn icon_scope(
        &self,
        raw_node_id: &str,
    ) -> Option<String> {
        // Iconify rewrites fragment ids from this unescaped logical prefix. Materialize only this
        // one node-local boundary; XML/CSS escaping happens later inside the icon renderer.
        self.node(raw_node_id).map(|id| format!("{id}-icon"))
    }

    #[cfg(test)]
    fn retained_generated_id_bytes(&self) -> usize {
        self.scope.capacity()
    }

    #[cfg(test)]
    fn compact_owner_count(&self) -> usize {
        self.edge_ids.len()
            + self.node_ids.len()
            + self.cluster_ids.len()
            + self.lane_ids.len()
            + self.synthetic_label_ids.len()
    }

    #[cfg(test)]
    fn compact_owner_payload_bytes(&self) -> usize {
        self.edge_ids.len() * std::mem::size_of::<crate::flowchart::FlowchartEdgeKey>()
            + self.node_ids.len() * std::mem::size_of::<(&str, FlowchartNodeDocumentId)>()
            + (self.cluster_ids.len() + self.lane_ids.len() + self.synthetic_label_ids.len())
                * std::mem::size_of::<(&str, usize)>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(id: String) -> crate::flowchart::FlowEdge {
        crate::flowchart::FlowEdge {
            id,
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            label_type: None,
            edge_type: None,
            arrow: "arrow_point".to_string(),
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

    #[test]
    fn long_scope_is_retained_once_independently_of_owner_count() {
        const EDGE_COUNT: usize = 128;
        const NODE_COUNT: usize = 128;
        const CLUSTER_COUNT: usize = 32;
        const SYNTHETIC_COUNT: usize = 64;

        let diagram_id = format!("scope:v1.{}", "d".repeat(16 * 1024));
        let edges = (0..EDGE_COUNT)
            .map(|index| edge(format!("edge-{index}")))
            .collect::<Vec<_>>();
        let edge_order = edges
            .iter()
            .enumerate()
            .map(|(index, edge)| FlowchartRenderEdgeRef {
                key: crate::flowchart::FlowchartEdgeKey::new(index),
                edge,
            })
            .collect::<Vec<_>>();
        let node_names = (0..NODE_COUNT)
            .map(|index| format!("node-{index}"))
            .collect::<Vec<_>>();
        let node_dom_index = node_names
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect::<FxHashMap<_, _>>();
        let cluster_names = (0..CLUSTER_COUNT)
            .map(|index| format!("cluster-{index}"))
            .collect::<Vec<_>>();
        let subgraph_index_by_id = cluster_names
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect::<FxHashMap<_, _>>();
        let layout_nodes = (0..SYNTHETIC_COUNT)
            .map(|index| crate::model::LayoutNode {
                id: format!("synthetic-{index}"),
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
                is_cluster: false,
                label_width: None,
                label_height: None,
            })
            .collect::<Vec<_>>();

        let ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: &diagram_id,
            edge_order: &edge_order,
            node_dom_index: &node_dom_index,
            subgraph_index_by_id: &subgraph_index_by_id,
            layout_nodes: &layout_nodes,
            swimlane_nodes: None,
            swimlane_lanes: None,
        });

        assert_eq!(
            ids.compact_owner_count(),
            EDGE_COUNT + NODE_COUNT + 2 * CLUSTER_COUNT + SYNTHETIC_COUNT
        );
        assert!(
            ids.retained_generated_id_bytes()
                <= (diagram_id.len() + DOCUMENT_NAMESPACE.len() + 1).next_power_of_two(),
            "the operation plan must retain the user-controlled scope once, not once per owner"
        );
        let largest_compact_entry = std::mem::size_of::<(&str, FlowchartNodeDocumentId)>()
            .max(std::mem::size_of::<(&str, usize)>())
            .max(std::mem::size_of::<crate::flowchart::FlowchartEdgeKey>());
        assert!(
            ids.compact_owner_payload_bytes() <= ids.compact_owner_count() * largest_compact_entry,
            "owner payload must contain only borrowed identities and fixed-size ordinals"
        );
        assert_eq!(
            ids.edge(crate::flowchart::FlowchartEdgeKey::new(EDGE_COUNT - 1))
                .expect("prepared edge id")
                .to_string(),
            format!("{diagram_id}-{DOCUMENT_NAMESPACE}-edge-{}", EDGE_COUNT - 1)
        );
        assert_eq!(
            ids.synthetic_label(&format!("synthetic-{}", SYNTHETIC_COUNT - 1))
                .expect("prepared synthetic label id")
                .to_string(),
            format!(
                "{diagram_id}-{DOCUMENT_NAMESPACE}-synthetic-label-{}",
                SYNTHETIC_COUNT - 1
            )
        );
        let expected_icon_scope = format!("{diagram_id}-{DOCUMENT_NAMESPACE}-node-0-icon");
        assert_eq!(
            ids.icon_scope("node-0").as_deref(),
            Some(expected_icon_scope.as_str()),
            "Iconify must receive the unescaped logical document id bytes"
        );
    }
}
