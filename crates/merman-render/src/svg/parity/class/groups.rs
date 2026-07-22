use super::super::timing::RenderTiming;
use super::ClassSvgRelation;
use super::context::ClassRenderDetails;
use super::edge::{
    ClassEdgeGroupsRenderContext, ClassEdgeGroupsRenderState, render_class_edge_groups,
};
use crate::model::{Bounds, LayoutEdge};
use crate::text::{TextMeasurer, TextStyle};
use rustc_hash::FxHashMap;

pub(super) struct ClassSplitEdgeGroupsRenderState<'a> {
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) detail: &'a mut ClassRenderDetails,
}

pub(super) struct ClassSplitEdgeGroupsRenderContext<'a> {
    pub(super) edges: &'a [LayoutEdge],
    pub(super) relations_by_id: &'a FxHashMap<&'a str, &'a ClassSvgRelation>,
    pub(super) relation_index_by_id: &'a FxHashMap<&'a str, usize>,
    pub(super) marker_url_prefix: &'a str,
    pub(super) diagram_id: &'a str,
    pub(super) content_tx: f64,
    pub(super) content_ty: f64,
    pub(super) edge_use_html_labels: bool,
    pub(super) text_measurer: &'a dyn TextMeasurer,
    pub(super) terminal_text_style: &'a TextStyle,
    pub(super) look: &'a str,
    pub(super) hand_drawn_seed: roughr::core::RoughRandomness,
    pub(super) timing: RenderTiming,
}

pub(super) struct ClassSplitEdgeGroups {
    pub(super) edge_paths: String,
    pub(super) edge_labels: String,
}

pub(super) fn render_class_split_edge_groups(
    state: ClassSplitEdgeGroupsRenderState<'_>,
    ctx: &ClassSplitEdgeGroupsRenderContext<'_>,
    bounds_dx: f64,
    bounds_dy: f64,
) -> ClassSplitEdgeGroups {
    let ClassSplitEdgeGroupsRenderState {
        content_bounds,
        detail,
    } = state;

    let mut edge_paths = String::new();
    let mut edge_labels = String::new();
    render_class_edge_groups(
        ClassEdgeGroupsRenderState {
            edge_paths: &mut edge_paths,
            edge_labels: &mut edge_labels,
            content_bounds,
            detail,
        },
        &ClassEdgeGroupsRenderContext {
            edges: ctx.edges,
            relations_by_id: ctx.relations_by_id,
            relation_index_by_id: ctx.relation_index_by_id,
            marker_url_prefix: ctx.marker_url_prefix,
            diagram_id: ctx.diagram_id,
            content_tx: ctx.content_tx,
            content_ty: ctx.content_ty,
            bounds_dx,
            bounds_dy,
            edge_use_html_labels: ctx.edge_use_html_labels,
            text_measurer: ctx.text_measurer,
            terminal_text_style: ctx.terminal_text_style,
            look: ctx.look,
            hand_drawn_seed: ctx.hand_drawn_seed.clone(),
            timing: ctx.timing,
        },
    );
    ClassSplitEdgeGroups {
        edge_paths,
        edge_labels,
    }
}
