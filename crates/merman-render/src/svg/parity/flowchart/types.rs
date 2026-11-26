//! Shared flowchart rendering types.
//!
//! This module keeps `flowchart.rs` slimmer by housing context structs and scratch buffers that
//! are used across flowchart SVG emission (rendering + viewBox computation).

use super::super::*;
use super::FlowchartEdgeStylePlan;
use super::render_input::FlowchartRenderEdgeRef;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartEmitCheckpoint<'a> {
    checkpoint: &'a dyn Fn() -> crate::Result<()>,
}

impl<'a> FlowchartEmitCheckpoint<'a> {
    pub(in crate::svg::parity::flowchart) fn new(
        checkpoint: &'a dyn Fn() -> crate::Result<()>,
    ) -> Self {
        Self { checkpoint }
    }

    pub(in crate::svg::parity::flowchart) fn checkpoint(self) -> crate::Result<()> {
        (self.checkpoint)()
    }
}

#[derive(Clone, Copy)]
pub(super) enum FlowchartLayoutEdgeRef<'a> {
    Materialized(&'a crate::model::LayoutEdge),
    #[cfg(feature = "layout-shoal")]
    Projected(&'a crate::graph_layout::FlowchartSvgEdgeGeometry),
}

impl<'a> FlowchartLayoutEdgeRef<'a> {
    pub(super) fn in_layout_order(
        layout: &'a FlowchartLayout,
    ) -> impl ExactSizeIterator<Item = (crate::flowchart::FlowchartEdgeKey, Self)> + 'a {
        #[cfg(feature = "layout-shoal")]
        let projected = layout.route_contract.flowchart_svg_edges();
        // Paint preparation validates the row count before iterating the owners.
        layout
            .edge_owners
            .iter()
            .enumerate()
            .map(move |(index, key)| {
                #[cfg(feature = "layout-shoal")]
                if let Some(edges) = projected {
                    return (key, Self::Projected(&edges[index]));
                }
                (key, Self::Materialized(&layout.edges[index]))
            })
    }

    pub(super) fn points(self) -> &'a [crate::model::LayoutPoint] {
        match self {
            Self::Materialized(edge) => &edge.points,
            #[cfg(feature = "layout-shoal")]
            Self::Projected(edge) => &edge.points,
        }
    }

    pub(super) fn label(self) -> Option<&'a crate::model::LayoutLabel> {
        match self {
            Self::Materialized(edge) => edge.label.as_ref(),
            #[cfg(feature = "layout-shoal")]
            Self::Projected(edge) => edge.label.as_ref(),
        }
    }

    pub(super) fn materialized(self) -> Option<&'a crate::model::LayoutEdge> {
        match self {
            Self::Materialized(edge) => Some(edge),
            #[cfg(feature = "layout-shoal")]
            Self::Projected(_) => None,
        }
    }
}

pub(in crate::svg::parity) struct FlowchartRenderCtx<'a> {
    pub(in crate::svg::parity::flowchart) prepared_nodes:
        &'a super::node_inventory::FlowchartPreparedNodes,
    pub(in crate::svg::parity::flowchart) model: &'a crate::flowchart::FlowchartRenderModelRef<'a>,
    pub(in crate::svg::parity::flowchart) diagram_id: SvgDiagramId<'a>,
    pub(in crate::svg::parity::flowchart) diagram_type: &'a str,
    pub(in crate::svg::parity::flowchart) tx: f64,
    pub(in crate::svg::parity::flowchart) ty: f64,
    pub(in crate::svg::parity::flowchart) measurer: &'a dyn TextMeasurer,
    pub(in crate::svg::parity::flowchart) config:
        &'a merman_model::presentation::PresentationConfig,
    pub(in crate::svg::parity::flowchart) compatibility:
        &'a crate::flowchart::FlowchartCompatibilityBinding,
    pub(in crate::svg::parity::flowchart) native:
        Option<&'a crate::flowchart::native_theme::FlowchartNativeStyle>,
    pub(in crate::svg::parity::flowchart) container_label_layers:
        std::cell::OnceCell<super::render::edge_label::ContainerLabelLayers>,
    pub(in crate::svg::parity::flowchart) palette: super::agentflow::ContainerPalette<'a>,
    pub(in crate::svg::parity::flowchart) hand_drawn_seed: roughr::core::RoughRandomness,
    pub(in crate::svg::parity::flowchart) work_meter: &'a crate::resources::OperationWorkMeter,
    pub(in crate::svg::parity::flowchart) emit: FlowchartEmitCheckpoint<'a>,
    pub(in crate::svg::parity::flowchart) math_renderer:
        Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    pub(in crate::svg::parity::flowchart) svg_label_sidecar:
        Option<&'a crate::flowchart::FlowchartSvgLabelSidecar>,
    pub(in crate::svg::parity::flowchart) icon_registry: Option<&'a crate::svg::IconRegistry>,
    pub(in crate::svg::parity::flowchart) security_level_loose: bool,
    pub(in crate::svg::parity::flowchart) node_html_labels: bool,
    pub(in crate::svg::parity::flowchart) edge_html_labels: bool,
    pub(in crate::svg::parity::flowchart) swimlane_title_html_labels: bool,
    pub(in crate::svg::parity::flowchart) uses_elk_adapter_dom: bool,
    pub(in crate::svg::parity::flowchart) class_defs: &'a IndexMap<String, Vec<String>>,
    pub(in crate::svg::parity::flowchart) edge_style_plan: &'a FlowchartEdgeStylePlan,
    pub(in crate::svg::parity::flowchart) steps: super::step::FlowchartSteps,
    pub(in crate::svg::parity::flowchart) document_ids:
        std::cell::OnceCell<super::document_ids::FlowchartDocumentIds>,
    pub(in crate::svg::parity::flowchart) layout: &'a FlowchartLayout,
    pub(in crate::svg::parity::flowchart) swimlane_layout: Option<&'a crate::model::SwimlaneLayout>,
    pub(in crate::svg::parity::flowchart) node_border_color: &'a String,
    pub(in crate::svg::parity::flowchart) node_stroke_width: f32,
    pub(in crate::svg::parity::flowchart) edge_corner_radius: f64,
    pub(in crate::svg::parity::flowchart) default_edge_interpolate: &'a String,
    pub(in crate::svg::parity::flowchart) default_edge_style: &'a Vec<String>,
    pub(in crate::svg::parity::flowchart) trace_edge_id: Option<&'a str>,
    pub(in crate::svg::parity::flowchart) trace_collector:
        Option<&'a crate::svg::FlowchartEdgeTraceCollector>,
    pub(in crate::svg::parity::flowchart) edge_order: Vec<FlowchartRenderEdgeRef<'a>>,
    pub(in crate::svg::parity::flowchart) edges_by_key:
        FxHashMap<crate::flowchart::FlowchartEdgeKey, &'a crate::flowchart::FlowEdge>,
    pub(in crate::svg::parity::flowchart) nodes_by_id:
        FxHashMap<&'a str, &'a crate::flowchart::FlowNode>,
    pub(in crate::svg::parity::flowchart) subgraphs_by_id:
        FxHashMap<&'a str, &'a crate::flowchart::FlowSubgraph>,
    pub(in crate::svg::parity::flowchart) subgraph_indices_by_id: FxHashMap<&'a str, usize>,
    pub(in crate::svg::parity::flowchart) subgraph_ids_with_children: FxHashSet<&'a str>,
    pub(in crate::svg::parity::flowchart) tooltips: &'a FxHashMap<String, String>,
    pub(in crate::svg::parity::flowchart) recursive_clusters: FxHashSet<&'a str>,
    pub(in crate::svg::parity::flowchart) parent: FxHashMap<&'a str, &'a str>,
    pub(in crate::svg::parity::flowchart) layout_nodes_by_id: FxHashMap<&'a str, &'a LayoutNode>,
    pub(in crate::svg::parity::flowchart) layout_edges_by_key:
        FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartLayoutEdgeRef<'a>>,
    pub(in crate::svg::parity::flowchart) layout_clusters_by_id:
        FxHashMap<&'a str, &'a LayoutCluster>,
    pub(in crate::svg::parity::flowchart) elk_cluster_geometry:
        FxHashMap<&'a str, super::cluster_geometry::FlowchartElkClusterGeometry>,
    pub(in crate::svg::parity::flowchart) swimlane_direction:
        Option<crate::model::SwimlaneDirection>,
    pub(in crate::svg::parity::flowchart) swimlane_lanes_by_id:
        FxHashMap<&'a str, &'a crate::model::SwimlaneLaneLayout>,
    pub(in crate::svg::parity::flowchart) swimlane_edge_label_edges_by_node_id:
        FxHashMap<&'a str, FlowchartRenderEdgeRef<'a>>,
    pub(in crate::svg::parity::flowchart) node_dom_index: FxHashMap<&'a str, usize>,
    pub(in crate::svg::parity::flowchart) node_padding: f64,
    pub(in crate::svg::parity::flowchart) wrapping_width: f64,
    pub(in crate::svg::parity::flowchart) node_wrap_mode: WrapMode,
    pub(in crate::svg::parity::flowchart) edge_wrap_mode: WrapMode,
    pub(in crate::svg::parity::flowchart) text_style: &'a TextStyle,
    pub(in crate::svg::parity::flowchart) html_label_text_style: &'a TextStyle,
}

impl<'a> FlowchartRenderCtx<'a> {
    pub(in crate::svg::parity::flowchart) fn node_rectangle(
        &self,
        id: &str,
    ) -> Option<crate::flowchart::FlowchartRectangleGeometry> {
        match self.node_geometry(id)? {
            crate::flowchart::PreparedFlowchartNodeGeometry::Rectangle(rectangle) => {
                Some(rectangle)
            }
            _ => None,
        }
    }

    pub(in crate::svg::parity::flowchart) fn node_image(
        &self,
        id: &str,
    ) -> Option<crate::flowchart::ImageSquareGeometry> {
        match self.node_geometry(id)? {
            crate::flowchart::PreparedFlowchartNodeGeometry::ImageSquare(image) => Some(image),
            _ => None,
        }
    }

    fn node_geometry(&self, id: &str) -> Option<crate::flowchart::PreparedFlowchartNodeGeometry> {
        self.svg_label_sidecar?
            .node_geometry(id, self.swimlane_layout.is_some())
    }

    pub(in crate::svg::parity::flowchart) fn rectangle_geometry(
        &self,
        node: &LayoutNode,
    ) -> crate::flowchart::FlowchartRectangleGeometry {
        self.node_rectangle(&node.id).unwrap_or_else(|| {
            let shape = kurbo::Rect::from_center_size(
                kurbo::Point::ORIGIN,
                (node.width.max(1.0), node.height.max(1.0)),
            );
            crate::flowchart::FlowchartRectangleGeometry {
                shape,
                group: shape,
            }
        })
    }

    pub(in crate::svg::parity::flowchart) fn document_ids(
        &self,
    ) -> &super::document_ids::FlowchartDocumentIds {
        self.document_ids.get_or_init(|| {
            super::document_ids::FlowchartDocumentIds::prepare(
                super::document_ids::FlowchartDocumentIdRequest {
                    diagram_id: self.diagram_id.semantic_str(),
                    edge_order: &self.edge_order,
                    node_dom_index: &self.node_dom_index,
                    subgraph_index_by_id: &self.subgraph_indices_by_id,
                    layout_nodes: &self.layout.nodes,
                    swimlane_nodes: self.swimlane_layout.map(|layout| layout.nodes.as_slice()),
                    swimlane_lanes: self.swimlane_layout.map(|layout| layout.lanes.as_slice()),
                },
            )
        })
    }

    pub(in crate::svg::parity::flowchart) fn subgraph_has_children(&self, id: &str) -> bool {
        self.subgraph_ids_with_children.contains(id)
    }

    pub(in crate::svg::parity::flowchart) fn is_subgraph_collapsed(&self, id: &str) -> bool {
        self.model.is_subgraph_collapsed(id)
    }

    pub(in crate::svg::parity::flowchart) fn checkpoint_emit(&self) -> crate::Result<()> {
        self.emit.checkpoint()
    }
}

#[derive(Debug, Default, Clone)]
pub(in crate::svg::parity::flowchart) struct FlowchartRenderDetails {
    pub(in crate::svg::parity::flowchart) root_calls: u32,
    pub(in crate::svg::parity::flowchart) clusters: std::time::Duration,
    pub(in crate::svg::parity::flowchart) edges_select: std::time::Duration,
    pub(in crate::svg::parity::flowchart) edge_paths: std::time::Duration,
    pub(in crate::svg::parity::flowchart) edge_labels: std::time::Duration,
    pub(in crate::svg::parity::flowchart) dom_order: std::time::Duration,
    pub(in crate::svg::parity::flowchart) nodes: std::time::Duration,
    pub(in crate::svg::parity::flowchart) node_style_compile: std::time::Duration,
    pub(in crate::svg::parity::flowchart) node_roughjs: std::time::Duration,
    pub(in crate::svg::parity::flowchart) node_roughjs_calls: u32,
    pub(in crate::svg::parity::flowchart) node_label_html: std::time::Duration,
    pub(in crate::svg::parity::flowchart) node_label_html_calls: u32,
    pub(in crate::svg::parity::flowchart) nested_roots: std::time::Duration,
    pub(in crate::svg::parity::flowchart) viewbox_edge_root_lookup: std::time::Duration,
    pub(in crate::svg::parity::flowchart) viewbox_edge_curve_offsets: std::time::Duration,
    pub(in crate::svg::parity::flowchart) viewbox_edge_curve_geom: std::time::Duration,
    pub(in crate::svg::parity::flowchart) viewbox_edge_curve_bbox_union: std::time::Duration,
    pub(in crate::svg::parity::flowchart) viewbox_edge_curve_geom_calls: u32,
    pub(in crate::svg::parity::flowchart) viewbox_edge_curve_geom_skipped_bounds: u32,
}

#[derive(Default)]
pub(in crate::svg::parity::flowchart) struct FlowchartEdgeDataPointsScratch {
    pub(in crate::svg::parity::flowchart) json: String,
    pub(in crate::svg::parity::flowchart) data_points_base64: String,
    pub(in crate::svg::parity::flowchart) edge_class_attr: String,
    pub(in crate::svg::parity::flowchart) edge_marker_attrs: String,
    pub(in crate::svg::parity::flowchart) ryu: ryu_js::Buffer,
    pub(in crate::svg::parity::flowchart) tmp_points_a: Vec<crate::model::LayoutPoint>,
    pub(in crate::svg::parity::flowchart) tmp_points_b: Vec<crate::model::LayoutPoint>,
    pub(in crate::svg::parity::flowchart) tmp_points_c: Vec<crate::model::LayoutPoint>,
    pub(in crate::svg::parity::flowchart) tmp_points_rev: Vec<crate::model::LayoutPoint>,
    #[cfg(any(feature = "layout-shoal", test))]
    pub(in crate::svg::parity::flowchart) canonical_points:
        Vec<crate::graph_layout::ir::RoutePoint>,
    #[cfg(any(feature = "layout-shoal", test))]
    pub(in crate::svg::parity::flowchart) canonical_paint:
        crate::graph_layout::route_paint::RoutePaintScratch,
}

#[derive(Debug, Clone)]
pub(in crate::svg::parity::flowchart) struct FlowchartEdgePathGeom {
    pub(in crate::svg::parity::flowchart) d: String,
    pub(in crate::svg::parity::flowchart) pb: Option<path_bounds::SvgPathBounds>,
    /// Centerline length rounded to the SVG DOM's float before any path rewrite.
    pub(in crate::svg::parity::flowchart) neo_length: Option<f64>,
    /// Exact point list serialized into `data-points` by Mermaid's `insertEdge`.
    pub(in crate::svg::parity::flowchart) data_points: Vec<crate::model::LayoutPoint>,
    /// Length of the displaced curve for an unanimated Neo dotted or dashed mask.
    pub(in crate::svg::parity::flowchart) pre_hop_dash_mask_length: Option<f64>,
    /// A crossing triggered after-paint path rewriting, even if its visible jump was suppressed.
    pub(in crate::svg::parity::flowchart) line_hop_applied: bool,
    /// Input to insertEdge after layout postprocessing, before paint-time clipping.
    pub(in crate::svg::parity::flowchart) original_label_path_points:
        Vec<crate::model::LayoutPoint>,
    /// Label polyline after endpoint/cluster clipping and before curve preprocessing.
    pub(in crate::svg::parity::flowchart) label_path_points: Vec<crate::model::LayoutPoint>,
    /// ELK main label after terminal-run projection and paired-label separation, in local coordinates.
    pub(in crate::svg::parity::flowchart) label_anchor: Option<crate::model::LayoutPoint>,
    pub(in crate::svg::parity::flowchart) label_path_was_explicitly_updated: bool,
    /// Actual path emitted by `insertEdge`; hand-drawn edges differ from the logical curve `d`.
    pub(in crate::svg::parity::flowchart) emitted_d_for_label: Option<String>,
    pub(in crate::svg::parity::flowchart) bounds_skipped_for_viewbox: bool,
}

impl FlowchartEdgePathGeom {
    pub(in crate::svg::parity::flowchart) fn centerline_length(&self) -> Option<f64> {
        self.neo_length.or_else(|| svg_path_length_from_d(&self.d))
    }
}

#[derive(Debug, Clone)]
pub(in crate::svg::parity) struct FlowchartEdgePathCacheEntry {
    pub(in crate::svg::parity::flowchart) origin_x: f64,
    pub(in crate::svg::parity::flowchart) origin_y: f64,
    pub(in crate::svg::parity::flowchart) abs_top_transform: f64,
    pub(in crate::svg::parity::flowchart) geom: FlowchartEdgePathGeom,
}

#[inline]
pub(in crate::svg::parity::flowchart) fn detail_guard<'a>(
    timing: timing::RenderTiming,
    dst: &'a mut std::time::Duration,
) -> Option<timing::TimingGuard<'a>> {
    timing.section(dst)
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartRootOffsets {
    pub(in crate::svg::parity::flowchart) origin_x: f64,
    pub(in crate::svg::parity::flowchart) origin_y: f64,
    pub(in crate::svg::parity::flowchart) abs_top_transform: f64,
}
