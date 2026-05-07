use super::ClassSvgRelation;
use super::context::ClassRenderDetails;
use super::edge::{
    ClassEdgeGroupsRenderContext, ClassEdgeGroupsRenderState, render_class_edge_groups,
};
use super::namespace::{ClassNamespaceClusterGroupContext, render_class_namespace_cluster_group};
use crate::model::{Bounds, LayoutCluster, LayoutEdge};
use rustc_hash::FxHashMap;

pub(super) struct ClassClusterEdgeGroupsRenderState<'a> {
    pub(super) out: &'a mut String,
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) detail: &'a mut ClassRenderDetails,
}

pub(super) struct ClassClusterEdgeGroupsRenderContext<'a> {
    pub(super) clusters: &'a [LayoutCluster],
    pub(super) edges: &'a [LayoutEdge],
    pub(super) relations_by_id: &'a FxHashMap<&'a str, &'a ClassSvgRelation>,
    pub(super) relation_index_by_id: &'a FxHashMap<&'a str, usize>,
    pub(super) marker_url_prefix: &'a str,
    pub(super) content_tx: f64,
    pub(super) content_ty: f64,
    pub(super) edge_use_html_labels: bool,
    pub(super) timing_enabled: bool,
}

pub(super) fn render_class_cluster_edge_groups(
    state: ClassClusterEdgeGroupsRenderState<'_>,
    ctx: &ClassClusterEdgeGroupsRenderContext<'_>,
    bounds_dx: f64,
    bounds_dy: f64,
    emit_clusters: bool,
) {
    let ClassClusterEdgeGroupsRenderState {
        out,
        content_bounds,
        detail,
    } = state;

    if emit_clusters {
        detail.clusters += render_class_namespace_cluster_group(
            out,
            content_bounds,
            ctx.clusters,
            ClassNamespaceClusterGroupContext {
                content_tx: ctx.content_tx,
                content_ty: ctx.content_ty,
                bounds_dx,
                bounds_dy,
                timing_enabled: ctx.timing_enabled,
            },
        );
    }

    render_class_edge_groups(
        ClassEdgeGroupsRenderState {
            out,
            content_bounds,
            detail,
        },
        &ClassEdgeGroupsRenderContext {
            edges: ctx.edges,
            relations_by_id: ctx.relations_by_id,
            relation_index_by_id: ctx.relation_index_by_id,
            marker_url_prefix: ctx.marker_url_prefix,
            content_tx: ctx.content_tx,
            content_ty: ctx.content_ty,
            bounds_dx,
            bounds_dy,
            edge_use_html_labels: ctx.edge_use_html_labels,
            timing_enabled: ctx.timing_enabled,
        },
    );
}
