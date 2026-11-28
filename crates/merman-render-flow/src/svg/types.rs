//! Shared flowchart rendering types.
//!
//! This module keeps `flowchart.rs` slimmer by housing context structs and scratch buffers that
//! are used across flowchart SVG emission (rendering + viewBox computation).

use super::FlowchartEdgeStylePlan;
use super::render_input::FlowchartRenderEdgeRef;
use super::*;
pub(in crate::svg) use crate::graph::FlowchartRootOffsets;
use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Copy)]
pub enum FlowchartLayoutEdgeRef<'a> {
    Materialized(&'a merman_render_core::LayoutEdge),
    #[cfg(feature = "layout-layered")]
    Layered {
        edge: &'a merman_render_core::LayoutEdge,
        route: &'a merman_render_core::graph_layout::SolvedEdge,
    },
    #[cfg(feature = "layout-shoal")]
    Projected(&'a crate::graph::FlowchartSvgEdgeGeometry),
}

impl<'a> FlowchartLayoutEdgeRef<'a> {
    pub(super) fn in_layout_order(
        layout: FlowchartSvgLayout<'a>,
    ) -> impl ExactSizeIterator<Item = (crate::projection::FlowchartEdgeKey, Self)> + 'a {
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
                #[cfg(feature = "layout-layered")]
                if let FlowchartSvgRoute::Layered(layered) = layout.route_contract {
                    return (
                        key,
                        Self::Layered {
                            edge: &layout.edges[index],
                            route: &layered.edges[index],
                        },
                    );
                }
                (key, Self::Materialized(&layout.edges[index]))
            })
    }

    pub fn points(self) -> &'a [merman_render_core::LayoutPoint] {
        match self {
            Self::Materialized(edge) => &edge.points,
            #[cfg(feature = "layout-layered")]
            Self::Layered { edge, .. } => &edge.points,
            #[cfg(feature = "layout-shoal")]
            Self::Projected(edge) => &edge.points,
        }
    }

    pub(super) fn label(self) -> Option<&'a merman_render_core::LayoutLabel> {
        match self {
            Self::Materialized(edge) => edge.label.as_ref(),
            #[cfg(feature = "layout-layered")]
            Self::Layered { edge, .. } => edge.label.as_ref(),
            #[cfg(feature = "layout-shoal")]
            Self::Projected(edge) => edge.label.as_ref(),
        }
    }

    pub(super) fn materialized(self) -> Option<&'a merman_render_core::LayoutEdge> {
        match self {
            Self::Materialized(edge) => Some(edge),
            #[cfg(feature = "layout-layered")]
            Self::Layered { edge, .. } => Some(edge),
            #[cfg(feature = "layout-shoal")]
            Self::Projected(_) => None,
        }
    }
}

pub struct FlowchartRenderCtx<'a> {
    pub(in crate::svg) prepared_nodes: &'a crate::node_inventory::FlowchartPreparedNodes,
    pub(in crate::svg) model: &'a crate::projection::FlowchartRenderModelRef<'a>,
    pub(in crate::svg) diagram_id: SvgDiagramId<'a>,
    pub(in crate::svg) diagram_type: &'a str,
    pub(in crate::svg) tx: f64,
    pub(in crate::svg) ty: f64,
    pub(in crate::svg) measurer: &'a dyn TextMeasurer,
    pub(in crate::svg) config: &'a merman_model::presentation::PresentationConfig,
    pub(in crate::svg) compatibility: &'a crate::theme::FlowchartCompatibilityBinding,
    pub(in crate::svg) native: Option<&'a crate::theme::FlowchartNativeStyle>,
    /// A known canvas color, including transparent, or `None` for unsampled paint.
    pub(in crate::svg) canvas: Option<Color>,
    pub(in crate::svg) container_label_layers:
        std::cell::OnceCell<super::render::edge_label::ContainerLabelLayers>,
    pub(in crate::svg) palette: super::agentflow::ContainerPalette<'a>,
    pub hand_drawn_seed: roughr::core::RoughRandomness,
    pub work_meter: &'a merman_render_core::resources::OperationWorkMeter,
    pub(in crate::svg) work_phase: merman_model::OperationPhase,
    pub(in crate::svg) math_renderer:
        Option<&'a (dyn merman_render_core::math::MathRenderer + Send + Sync)>,
    pub(in crate::svg) svg_label_sidecar: Option<&'a crate::svg_label::FlowchartSvgLabelSidecar>,
    pub(in crate::svg) icon_registry: Option<&'a merman_render_core::icon_registry::IconRegistry>,
    pub(in crate::svg) security_level_loose: bool,
    pub(in crate::svg) node_html_labels: bool,
    pub(in crate::svg) edge_html_labels: bool,
    pub(in crate::svg) swimlane_title_html_labels: bool,
    pub(in crate::svg) uses_elk_adapter_dom: bool,
    pub(in crate::svg) class_defs: &'a IndexMap<String, Vec<String>>,
    pub edge_style_plan: &'a FlowchartEdgeStylePlan,
    pub(in crate::svg) steps: crate::edge_svg::FlowchartSteps,
    pub document_ids: std::cell::OnceCell<super::document_ids::FlowchartDocumentIds>,
    pub layout: FlowchartSvgLayout<'a>,
    pub(in crate::svg) swimlane_layout: Option<SwimlaneSvgLayout<'a>>,
    pub(in crate::svg) node_border_color: &'a String,
    pub(in crate::svg) node_stroke_width: f32,
    pub(in crate::svg) edge_corner_radius: f64,
    pub(in crate::svg) default_edge_interpolate: &'a String,
    pub(in crate::svg) default_edge_style: &'a Vec<String>,
    pub(in crate::svg) trace_edge_id: Option<&'a str>,
    pub(in crate::svg) trace_collector:
        Option<&'a merman_render_core::svg_edge_parity::trace::FlowchartEdgeTraceCollector>,
    pub(in crate::svg) edge_order: Vec<FlowchartRenderEdgeRef<'a>>,
    pub(in crate::svg) edges_by_key: FxHashMap<
        crate::projection::FlowchartEdgeKey,
        &'a merman_model::families::flowchart::FlowEdge,
    >,
    pub(in crate::svg) nodes_by_id:
        FxHashMap<&'a str, &'a merman_model::families::flowchart::FlowNode>,
    pub(in crate::svg) subgraphs_by_id:
        FxHashMap<&'a str, &'a merman_model::families::flowchart::FlowSubgraph>,
    pub(in crate::svg) subgraph_indices_by_id: FxHashMap<&'a str, usize>,
    pub(in crate::svg) subgraph_ids_with_children: FxHashSet<&'a str>,
    pub(in crate::svg) tooltips: &'a FxHashMap<String, String>,
    pub(in crate::svg) recursive_clusters: FxHashSet<&'a str>,
    pub(in crate::svg) parent: FxHashMap<&'a str, &'a str>,
    pub(in crate::svg) layout_nodes_by_id: FxHashMap<&'a str, &'a LayoutNode>,
    pub layout_edges_by_key:
        FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartLayoutEdgeRef<'a>>,
    pub(in crate::svg) layout_clusters_by_id: FxHashMap<&'a str, &'a LayoutCluster>,
    pub(in crate::svg) elk_cluster_geometry:
        FxHashMap<&'a str, super::cluster_geometry::FlowchartElkClusterGeometry>,
    pub(in crate::svg) swimlane_direction: Option<merman_render_core::SwimlaneDirection>,
    pub(in crate::svg) swimlane_lanes_by_id:
        FxHashMap<&'a str, &'a merman_render_core::SwimlaneLaneLayout>,
    pub(in crate::svg) swimlane_edge_label_edges_by_node_id:
        FxHashMap<&'a str, FlowchartRenderEdgeRef<'a>>,
    pub(in crate::svg) node_dom_index: FxHashMap<&'a str, usize>,
    pub(in crate::svg) node_padding: f64,
    pub(in crate::svg) wrapping_width: f64,
    pub(in crate::svg) node_wrap_mode: WrapMode,
    pub(in crate::svg) edge_wrap_mode: WrapMode,
    pub(in crate::svg) text_style: &'a TextStyle,
    pub(in crate::svg) html_label_text_style: &'a TextStyle,
}

impl<'a> FlowchartRenderCtx<'a> {
    pub(in crate::svg) fn node_rectangle(
        &self,
        id: &str,
    ) -> Option<crate::node_geometry::FlowchartRectangleGeometry> {
        crate::node_geometry::FlowchartRectangleGeometry::from_sidecar(
            self.svg_label_sidecar?,
            id,
            self.swimlane_layout.is_some(),
        )
    }

    pub(in crate::svg) fn node_image(
        &self,
        id: &str,
    ) -> Option<crate::node_geometry::ImageSquareGeometry> {
        crate::node_geometry::flowchart_image_square_geometry(
            self.svg_label_sidecar?,
            id,
            self.swimlane_layout.is_some(),
        )
    }

    pub(in crate::svg) fn rectangle_geometry(
        &self,
        node: &LayoutNode,
    ) -> crate::node_geometry::FlowchartRectangleGeometry {
        self.node_rectangle(&node.id).unwrap_or_else(|| {
            crate::node_geometry::FlowchartRectangleGeometry::from_layout_node(node)
        })
    }

    pub fn document_ids(&self) -> &super::document_ids::FlowchartDocumentIds {
        self.document_ids.get_or_init(|| {
            super::document_ids::FlowchartDocumentIds::prepare(
                super::document_ids::FlowchartDocumentIdRequest {
                    diagram_id: self.diagram_id,
                    edge_order: &self.edge_order,
                    node_dom_index: &self.node_dom_index,
                    subgraph_index_by_id: &self.subgraph_indices_by_id,
                    layout_nodes: self.layout.nodes,
                    swimlane_nodes: self.swimlane_layout.map(|layout| layout.nodes),
                    swimlane_lanes: self.swimlane_layout.map(|layout| layout.lanes),
                },
            )
        })
    }

    pub(in crate::svg) fn subgraph_has_children(&self, id: &str) -> bool {
        self.subgraph_ids_with_children.contains(id)
    }

    pub(in crate::svg) fn is_subgraph_collapsed(&self, id: &str) -> bool {
        self.model.is_subgraph_collapsed(id)
    }

    pub(in crate::svg) fn checkpoint_emit(&self) -> merman_render_core::Result<()> {
        self.work_meter
            .checkpoint(merman_model::OperationPhase::Emit)
            .map_err(Into::into)
    }
}

#[derive(Debug, Default, Clone)]
pub struct FlowchartRenderDetails {
    pub(in crate::svg) node_style_compile: std::time::Duration,
    pub(in crate::svg) node_roughjs: std::time::Duration,
    pub(in crate::svg) node_roughjs_calls: u32,
    pub(in crate::svg) node_label_html: std::time::Duration,
    pub(in crate::svg) node_label_html_calls: u32,
    pub(in crate::svg) viewbox_edge_root_lookup: std::time::Duration,
    pub(in crate::svg) viewbox_edge_curve_offsets: std::time::Duration,
    pub(in crate::svg) viewbox_edge_curve_geom: std::time::Duration,
    pub(in crate::svg) viewbox_edge_curve_bbox_union: std::time::Duration,
    pub(in crate::svg) viewbox_edge_curve_geom_calls: u32,
    pub(in crate::svg) viewbox_edge_curve_geom_skipped_bounds: u32,
}

#[derive(Default)]
pub struct FlowchartEdgeDataPointsScratch {
    pub(in crate::svg) terminal: crate::edge_svg::FlowchartEdgeSvgScratch,
    pub(in crate::svg) tmp_points_a: Vec<merman_render_core::LayoutPoint>,
    pub(in crate::svg) tmp_points_b: Vec<merman_render_core::LayoutPoint>,
    pub(in crate::svg) tmp_points_c: Vec<merman_render_core::LayoutPoint>,
    #[cfg(feature = "layout-grid")]
    pub(in crate::svg) grid_points: Vec<kurbo::Point>,
    #[cfg(feature = "layout-layered")]
    pub(in crate::svg) layered_points: Vec<merman_render_core::route_types::RoutePoint>,
    pub(in crate::svg) tmp_points_rev: Vec<merman_render_core::LayoutPoint>,
    #[cfg(feature = "canonical-edge-paint")]
    pub(in crate::svg) canonical_points: Vec<merman_render_core::route_types::RoutePoint>,
    #[cfg(feature = "canonical-edge-paint")]
    pub(in crate::svg) canonical_paint: merman_render_core::route_paint::RoutePaintScratch,
}

#[derive(Debug, Clone)]
pub struct FlowchartEdgePathGeom {
    pub d: String,
    pub pb: Option<path_bounds::SvgPathBounds>,
    /// Centerline length rounded to the SVG DOM's float before any path rewrite.
    pub neo_length: Option<f64>,
    /// Exact point list serialized into `data-points` by Mermaid's `insertEdge`.
    pub data_points: Vec<merman_render_core::LayoutPoint>,
    #[cfg(feature = "layout-layered")]
    pub(in crate::svg) axes: Option<Vec<merman_render_core::route_types::Axis>>,
    /// Length of the displaced curve for an unanimated Neo dotted or dashed mask.
    pub(in crate::svg) pre_hop_dash_mask_length: Option<f64>,
    /// A crossing triggered after-paint path rewriting, even if its visible jump was suppressed.
    pub(in crate::svg) line_hop_applied: bool,
    /// The after-paint shaft, independent of the original path used by labels and Step marks.
    #[cfg(any(feature = "layout-grid", feature = "layout-layered"))]
    pub(in crate::svg) after_paint_d: Option<String>,
    /// Input to insertEdge after layout postprocessing, before paint-time clipping.
    pub(in crate::svg) original_label_path_points: Vec<merman_render_core::LayoutPoint>,
    /// Label polyline after endpoint/cluster clipping and before curve preprocessing.
    pub(in crate::svg) label_path_points: Vec<merman_render_core::LayoutPoint>,
    /// ELK main label after terminal-run projection and paired-label separation, in local coordinates.
    pub(in crate::svg) label_anchor: Option<merman_render_core::LayoutPoint>,
    pub(in crate::svg) label_path_was_explicitly_updated: bool,
    /// Actual path emitted by `insertEdge`; hand-drawn edges differ from the logical curve `d`.
    pub emitted_d_for_label: Option<String>,
    pub(in crate::svg) bounds_skipped_for_viewbox: bool,
}

impl FlowchartEdgePathGeom {
    pub(in crate::svg) fn centerline_length(&self) -> Option<f64> {
        self.neo_length.or_else(|| svg_path_length_from_d(&self.d))
    }
}

#[derive(Debug, Clone)]
pub struct FlowchartEdgePathCacheEntry {
    pub(in crate::svg) origin_x: f64,
    pub(in crate::svg) origin_y: f64,
    pub(in crate::svg) abs_top_transform: f64,
    pub geom: FlowchartEdgePathGeom,
}
