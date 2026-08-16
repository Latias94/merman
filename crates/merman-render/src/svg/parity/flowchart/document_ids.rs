//! Operation-local ownership of generated Flowchart-family SVG identifiers.
//!
//! Semantic Mermaid identifiers remain observable through `data-id`/`data-et`. They are not used
//! as generated DOM identifiers because nodes, clusters, lanes, edges, accessibility metadata,
//! filters, markers, root-theme resources, and prepared-text labels all share one SVG namespace.

use super::render_input::FlowchartRenderEdgeRef;
use rustc_hash::FxHashMap;

const DOCUMENT_NAMESPACE: &str = "merman-flowchart-document";

#[derive(Debug)]
pub(in crate::svg::parity::flowchart) struct FlowchartDocumentIds {
    scope: String,
    accessibility_title: String,
    accessibility_description: String,
    drop_shadow: String,
    drop_shadow_small: String,
    root_gradient: String,
    edge_ids: FxHashMap<crate::flowchart::FlowchartEdgeKey, String>,
    node_ids: FxHashMap<String, String>,
    cluster_ids: FxHashMap<String, String>,
    lane_ids: FxHashMap<String, String>,
    synthetic_label_ids: FxHashMap<String, String>,
}

pub(in crate::svg::parity::flowchart) struct FlowchartDocumentIdRequest<'plan, 'data> {
    pub(in crate::svg::parity::flowchart) diagram_id: &'plan str,
    pub(in crate::svg::parity::flowchart) edge_order: &'plan [FlowchartRenderEdgeRef<'data>],
    pub(in crate::svg::parity::flowchart) node_dom_index: &'plan FxHashMap<&'data str, usize>,
    pub(in crate::svg::parity::flowchart) subgraph_index_by_id: &'plan FxHashMap<&'data str, usize>,
    pub(in crate::svg::parity::flowchart) layout_nodes: &'plan [crate::model::LayoutNode],
    pub(in crate::svg::parity::flowchart) swimlane_nodes:
        Option<&'plan [crate::model::SwimlaneNodeLayout]>,
    pub(in crate::svg::parity::flowchart) swimlane_lanes:
        Option<&'plan [crate::model::SwimlaneLaneLayout]>,
}

impl FlowchartDocumentIds {
    pub(in crate::svg::parity::flowchart) fn prepare(
        request: FlowchartDocumentIdRequest<'_, '_>,
    ) -> Self {
        let scope = format!("{}-{DOCUMENT_NAMESPACE}", request.diagram_id);
        let accessibility_title = format!("{scope}-a11y-title");
        let accessibility_description = format!("{scope}-a11y-description");
        let drop_shadow = format!("{scope}-filter-drop-shadow");
        let drop_shadow_small = format!("{scope}-filter-drop-shadow-small");
        let root_gradient = format!("{scope}-gradient-root");

        let edge_ids = request
            .edge_order
            .iter()
            .map(|edge| {
                (
                    edge.key,
                    format!("{scope}-edge-{}", edge.key.semantic_index()),
                )
            })
            .collect();

        let mut node_ids = FxHashMap::default();
        node_ids.reserve(request.node_dom_index.len() + request.subgraph_index_by_id.len());
        for (&raw_id, &dom_index) in request.node_dom_index {
            node_ids.insert(raw_id.to_string(), format!("{scope}-node-{dom_index}"));
        }
        for (&raw_id, &subgraph_index) in request.subgraph_index_by_id {
            node_ids
                .entry(raw_id.to_string())
                .or_insert_with(|| format!("{scope}-node-subgraph-{subgraph_index}"));
        }

        let cluster_ids = request
            .subgraph_index_by_id
            .iter()
            .map(|(&raw_id, &subgraph_index)| {
                (
                    raw_id.to_string(),
                    format!("{scope}-cluster-{subgraph_index}"),
                )
            })
            .collect();

        let lane_ids = request
            .swimlane_lanes
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(lane_index, lane)| (lane.id.clone(), format!("{scope}-lane-{lane_index}")))
            .collect();

        let mut synthetic_label_ids = FxHashMap::default();
        let swimlane_node_count = request.swimlane_nodes.map_or(0, |nodes| nodes.len());
        synthetic_label_ids.reserve(request.layout_nodes.len() + swimlane_node_count);
        for (layout_index, node) in request.layout_nodes.iter().enumerate() {
            if !node_ids.contains_key(node.id.as_str()) {
                synthetic_label_ids
                    .entry(node.id.clone())
                    .or_insert_with(|| format!("{scope}-synthetic-label-{layout_index}"));
            }
        }
        let swimlane_offset = request.layout_nodes.len();
        for (swimlane_index, node) in request.swimlane_nodes.into_iter().flatten().enumerate() {
            if node.is_edge_label && !node_ids.contains_key(node.id.as_str()) {
                synthetic_label_ids
                    .entry(node.id.clone())
                    .or_insert_with(|| {
                        format!(
                            "{scope}-synthetic-label-{}",
                            swimlane_offset + swimlane_index
                        )
                    });
            }
        }

        Self {
            scope,
            accessibility_title,
            accessibility_description,
            drop_shadow,
            drop_shadow_small,
            root_gradient,
            edge_ids,
            node_ids,
            cluster_ids,
            lane_ids,
            synthetic_label_ids,
        }
    }

    pub(in crate::svg::parity::flowchart) fn accessibility_title(&self) -> &str {
        &self.accessibility_title
    }

    pub(in crate::svg::parity::flowchart) fn accessibility_description(&self) -> &str {
        &self.accessibility_description
    }

    pub(in crate::svg::parity::flowchart) fn drop_shadow(&self) -> &str {
        &self.drop_shadow
    }

    pub(in crate::svg::parity::flowchart) fn drop_shadow_small(&self) -> &str {
        &self.drop_shadow_small
    }

    pub(in crate::svg::parity::flowchart) fn root_gradient(&self) -> &str {
        &self.root_gradient
    }

    pub(in crate::svg::parity::flowchart) fn marker_scope(&self) -> &str {
        &self.scope
    }

    pub(in crate::svg::parity::flowchart) fn edge(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> Option<&str> {
        self.edge_ids.get(&key).map(String::as_str)
    }

    pub(in crate::svg::parity::flowchart) fn node(&self, raw_id: &str) -> Option<&str> {
        self.node_ids.get(raw_id).map(String::as_str)
    }

    pub(in crate::svg::parity::flowchart) fn cluster(&self, raw_id: &str) -> Option<&str> {
        self.cluster_ids.get(raw_id).map(String::as_str)
    }

    pub(in crate::svg::parity::flowchart) fn lane(&self, raw_id: &str) -> Option<&str> {
        self.lane_ids.get(raw_id).map(String::as_str)
    }

    pub(in crate::svg::parity::flowchart) fn synthetic_label(&self, raw_id: &str) -> Option<&str> {
        self.synthetic_label_ids.get(raw_id).map(String::as_str)
    }

    pub(in crate::svg::parity::flowchart) fn icon_scope(
        &self,
        raw_node_id: &str,
    ) -> Option<String> {
        self.node(raw_node_id).map(|id| format!("{id}-icon"))
    }
}
