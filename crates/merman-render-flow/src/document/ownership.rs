//! Operation-local ownership of generated Flowchart-family SVG identifiers.
//!
//! Semantic Mermaid identifiers remain observable through `data-id`/`data-et`. They are not used
//! as generated DOM identifiers because nodes, clusters, lanes, edges, accessibility metadata,
//! filters, markers, root-theme resources, and prepared-text labels all share one SVG namespace.

use merman_render_core::{LayoutNode, SwimlaneLaneLayout, SwimlaneNodeLayout, svg_css::SvgId};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{FlowchartDocumentId, FlowchartDocumentScope};
use crate::projection::{FlowchartEdgeKey, FlowchartRenderEdgeRef};

#[derive(Debug, Clone, Copy)]
enum FlowchartNodeDocumentId {
    Node(usize),
    Subgraph(usize),
}

/// Assigns generated IDs to prepared layout and semantic owners.
#[derive(Debug)]
pub struct FlowchartDocumentIds {
    scope: FlowchartDocumentScope,
    edge_ids: FxHashSet<FlowchartEdgeKey>,
    node_ids: FxHashMap<String, FlowchartNodeDocumentId>,
    cluster_ids: FxHashMap<String, usize>,
    lane_ids: FxHashMap<String, usize>,
    synthetic_label_ids: FxHashMap<String, usize>,
}

pub struct FlowchartDocumentIdRequest<'plan, 'data> {
    pub diagram_id: SvgId<'plan>,
    pub edge_order: &'plan [FlowchartRenderEdgeRef<'data>],
    pub node_dom_index: &'plan FxHashMap<&'data str, usize>,
    pub subgraph_index_by_id: &'plan FxHashMap<&'data str, usize>,
    pub layout_nodes: &'data [LayoutNode],
    pub swimlane_nodes: Option<&'data [SwimlaneNodeLayout]>,
    pub swimlane_lanes: Option<&'data [SwimlaneLaneLayout]>,
}

impl FlowchartDocumentIds {
    pub fn prepare(request: FlowchartDocumentIdRequest<'_, '_>) -> Self {
        let scope = FlowchartDocumentScope::new(request.diagram_id);

        let edge_ids = request.edge_order.iter().map(|edge| edge.key).collect();

        let mut node_ids = FxHashMap::default();
        node_ids.reserve(request.node_dom_index.len() + request.subgraph_index_by_id.len());
        for (&raw_id, &dom_index) in request.node_dom_index {
            node_ids.insert(raw_id.to_owned(), FlowchartNodeDocumentId::Node(dom_index));
        }
        for (&raw_id, &subgraph_index) in request.subgraph_index_by_id {
            // Projected edge endpoints can also register a collapsed subgraph in the node
            // ordinal map. Its owner remains the subgraph, whose ordinal namespace is separate
            // from parser-assigned node ordinals (notably Agentflow connectors).
            node_ids.insert(
                raw_id.to_owned(),
                FlowchartNodeDocumentId::Subgraph(subgraph_index),
            );
        }

        let cluster_ids = request
            .subgraph_index_by_id
            .iter()
            .map(|(&raw_id, &subgraph_index)| (raw_id.to_owned(), subgraph_index))
            .collect();

        let lane_ids = request
            .swimlane_lanes
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(lane_index, lane)| (lane.id.clone(), lane_index))
            .collect();

        let mut synthetic_label_ids = FxHashMap::default();
        let swimlane_node_count = request.swimlane_nodes.map_or(0, |nodes| nodes.len());
        synthetic_label_ids.reserve(request.layout_nodes.len() + swimlane_node_count);
        for (layout_index, node) in request.layout_nodes.iter().enumerate() {
            if !node_ids.contains_key(node.id.as_str()) {
                synthetic_label_ids
                    .entry(node.id.clone())
                    .or_insert(layout_index);
            }
        }
        let swimlane_offset = request.layout_nodes.len();
        for (swimlane_index, node) in request.swimlane_nodes.into_iter().flatten().enumerate() {
            if node.is_edge_label && !node_ids.contains_key(node.id.as_str()) {
                synthetic_label_ids
                    .entry(node.id.clone())
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

    pub fn scope(&self) -> &FlowchartDocumentScope {
        &self.scope
    }

    pub fn edge(&self, key: FlowchartEdgeKey) -> Option<FlowchartDocumentId<'_>> {
        self.edge_ids.contains(&key).then(|| {
            self.scope
                .edge(key.semantic_index(), key.self_loop_segment_order())
        })
    }

    pub fn node(&self, raw_id: &str) -> Option<FlowchartDocumentId<'_>> {
        self.node_ids.get(raw_id).map(|&owner| match owner {
            FlowchartNodeDocumentId::Node(ordinal) => self.scope.node(ordinal),
            FlowchartNodeDocumentId::Subgraph(ordinal) => self.scope.subgraph_node(ordinal),
        })
    }

    pub fn cluster(&self, raw_id: &str) -> Option<FlowchartDocumentId<'_>> {
        self.cluster_ids
            .get(raw_id)
            .map(|&ordinal| self.scope.cluster(ordinal))
    }

    pub fn lane(&self, raw_id: &str) -> Option<FlowchartDocumentId<'_>> {
        self.lane_ids
            .get(raw_id)
            .map(|&ordinal| self.scope.lane(ordinal))
    }

    pub fn synthetic_label(&self, raw_id: &str) -> Option<FlowchartDocumentId<'_>> {
        self.synthetic_label_ids
            .get(raw_id)
            .map(|&ordinal| self.scope.synthetic_label(ordinal))
    }

    pub fn icon_scope(&self, raw_node_id: &str) -> Option<String> {
        // Iconify rewrites fragment ids from this unescaped logical prefix. Materialize only this
        // one node-local boundary; XML/CSS escaping happens later inside the icon renderer.
        self.node(raw_node_id).map(|id| format!("{id}-icon"))
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
        self.edge_ids.len() * std::mem::size_of::<FlowchartEdgeKey>()
            + self.node_ids.len() * std::mem::size_of::<(String, FlowchartNodeDocumentId)>()
            + (self.cluster_ids.len() + self.lane_ids.len() + self.synthetic_label_ids.len())
                * std::mem::size_of::<(String, usize)>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::families::flowchart::FlowEdge;

    const DOCUMENT_NAMESPACE: &str = "merman-flowchart-document";

    #[test]
    fn collapsed_subgraph_endpoint_does_not_alias_a_parser_owned_node_ordinal() {
        let node_dom_index = FxHashMap::from_iter([("outer", 0), ("api", 0)]);
        let subgraph_index_by_id = FxHashMap::from_iter([("outer", 0)]);
        let ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: SvgId::new("agentflow").unwrap(),
            edge_order: &[],
            node_dom_index: &node_dom_index,
            subgraph_index_by_id: &subgraph_index_by_id,
            layout_nodes: &[],
            swimlane_nodes: None,
            swimlane_lanes: None,
        });

        assert_eq!(
            ids.node("outer").unwrap().to_string(),
            "agentflow-merman-flowchart-document-node-subgraph-0"
        );
        assert_eq!(
            ids.node("api").unwrap().to_string(),
            "agentflow-merman-flowchart-document-node-0"
        );
        assert_ne!(
            ids.node("outer").unwrap().to_string(),
            ids.cluster("outer").unwrap().to_string()
        );
    }

    #[test]
    fn synthetic_label_ordinals_preserve_layout_priority_and_swimlane_row_offsets() {
        let layout_nodes = ["semantic", "shared", "shared"].map(|id| LayoutNode {
            id: id.to_owned(),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            is_cluster: false,
            label_width: None,
            label_height: None,
        });
        let swimlane_nodes =
            [("plain", false), ("shared", true), ("generated", true)].map(|(id, is_edge_label)| {
                SwimlaneNodeLayout {
                    id: id.to_owned(),
                    label: String::new(),
                    label_type: String::new(),
                    shape: String::new(),
                    parent_id: None,
                    top_lane_id: None,
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                    label_width: 0.0,
                    label_height: 0.0,
                    layer: 0,
                    order: 0,
                    is_edge_label,
                }
            });
        let ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: SvgId::new("diagram").unwrap(),
            edge_order: &[],
            node_dom_index: &FxHashMap::from_iter([("semantic", 0)]),
            subgraph_index_by_id: &FxHashMap::default(),
            layout_nodes: &layout_nodes,
            swimlane_nodes: Some(&swimlane_nodes),
            swimlane_lanes: None,
        });

        assert!(ids.synthetic_label("plain").is_none());
        assert!(ids.synthetic_label("semantic").is_none());
        assert_eq!(
            ids.synthetic_label("shared").unwrap().to_string(),
            "diagram-merman-flowchart-document-synthetic-label-1"
        );
        assert_eq!(
            ids.synthetic_label("generated").unwrap().to_string(),
            "diagram-merman-flowchart-document-synthetic-label-5"
        );
    }

    fn edge(id: String) -> FlowEdge {
        FlowEdge {
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
            step: None,
            length: 1,
        }
    }

    #[test]
    fn long_scope_is_retained_once_independently_of_owner_count() {
        const EDGE_COUNT: usize = 128;
        const NODE_COUNT: usize = 128;
        const CLUSTER_COUNT: usize = 32;
        const SYNTHETIC_COUNT: usize = 64;

        let diagram_id = format!("scope-v1_{}", "d".repeat(16 * 1024));
        let edges = (0..EDGE_COUNT)
            .map(|index| edge(format!("edge-{index}")))
            .collect::<Vec<_>>();
        let edge_order = edges
            .iter()
            .enumerate()
            .map(|(index, edge)| FlowchartRenderEdgeRef {
                key: FlowchartEdgeKey::new(index),
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
            .map(|index| LayoutNode {
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
            diagram_id: SvgId::new(&diagram_id).unwrap(),
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
        assert_eq!(
            ids.scope().as_str().len(),
            diagram_id.len() + DOCUMENT_NAMESPACE.len() + 1
        );
        let largest_compact_entry = std::mem::size_of::<(String, FlowchartNodeDocumentId)>()
            .max(std::mem::size_of::<(String, usize)>())
            .max(std::mem::size_of::<FlowchartEdgeKey>());
        assert!(
            ids.compact_owner_payload_bytes() <= ids.compact_owner_count() * largest_compact_entry,
            "the inline owner payload is independent of the document namespace length"
        );
        assert_eq!(
            ids.edge(FlowchartEdgeKey::new(EDGE_COUNT - 1))
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

    #[test]
    fn self_loop_segments_have_distinct_document_ids_under_one_semantic_owner() {
        let source_edge = edge("authored".to_owned());
        let edge_order = [
            FlowchartEdgeKey::new(0),
            FlowchartEdgeKey::self_loop_segment(0, 0),
            FlowchartEdgeKey::self_loop_segment(0, 1),
            FlowchartEdgeKey::self_loop_segment(0, 2),
        ]
        .map(|key| FlowchartRenderEdgeRef {
            key,
            edge: &source_edge,
        });
        let ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: SvgId::new("diagram").unwrap(),
            edge_order: &edge_order,
            node_dom_index: &FxHashMap::default(),
            subgraph_index_by_id: &FxHashMap::default(),
            layout_nodes: &[],
            swimlane_nodes: None,
            swimlane_lanes: None,
        });
        let document_ids = edge_order
            .iter()
            .map(|edge| ids.edge(edge.key).unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            document_ids.iter().collect::<FxHashSet<_>>().len(),
            edge_order.len()
        );
        assert!(document_ids[0].ends_with("-edge-0"));
        for (order, id) in document_ids[1..].iter().enumerate() {
            assert!(id.ends_with(&format!("-edge-0-segment-{order}")));
        }
        assert!(ids.edge(FlowchartEdgeKey::new(1)).is_none());
        assert!(
            ids.edge(FlowchartEdgeKey::self_loop_segment(0, 3))
                .is_none()
        );
    }
}
