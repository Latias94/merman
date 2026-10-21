use super::super::timing::RenderTimings;
use super::*;
use rustc_hash::FxHashMap;
use std::time::Duration;

#[derive(Clone, Copy)]
pub(super) struct ClassEmitCheckpoint<'a> {
    checkpoint: &'a dyn Fn() -> crate::Result<()>,
}

impl<'a> ClassEmitCheckpoint<'a> {
    pub(super) fn new(checkpoint: &'a dyn Fn() -> crate::Result<()>) -> Self {
        Self { checkpoint }
    }

    pub(super) fn checkpoint(self) -> crate::Result<()> {
        (self.checkpoint)()
    }
}

#[derive(Debug, Default, Clone)]
pub(super) struct ClassRenderDetails {
    pub clusters: Duration,
    pub edge_paths: Duration,
    pub edge_curve: Duration,
    pub edge_points_json: Duration,
    pub edge_points_b64: Duration,
    pub edge_labels: Duration,
    pub nodes: Duration,
    pub notes_sanitize: Duration,
    pub path_bounds: Duration,
    pub path_bounds_calls: usize,
}

pub(super) struct ClassRenderLookups<'a> {
    pub class_nodes_by_id: FxHashMap<&'a str, ClassSvgNode<'a>>,
    pub class_color_indices: FxHashMap<&'a str, usize>,
    pub relations_by_id: FxHashMap<&'a str, ClassSvgRelation<'a>>,
    pub relation_index_by_id: FxHashMap<&'a str, usize>,
    pub note_by_id: FxHashMap<&'a str, ClassSvgNote<'a>>,
    pub iface_by_id: FxHashMap<&'a str, ClassSvgInterface<'a>>,
}

impl<'a> ClassRenderLookups<'a> {
    pub(super) fn new(model: &'a ClassSvgModel) -> Self {
        let mut class_nodes_by_id: FxHashMap<&str, ClassSvgNode<'_>> = FxHashMap::default();
        class_nodes_by_id.reserve(model.classes().len());
        for (id, n) in model.classes() {
            class_nodes_by_id.insert(id, n);
        }

        let mut relations_by_id: FxHashMap<&str, ClassSvgRelation<'_>> = FxHashMap::default();
        relations_by_id.reserve(model.relations().len());
        for r in model.relations() {
            relations_by_id.insert(r.source.id.as_str(), r);
        }

        let mut relation_index_by_id: FxHashMap<&str, usize> = FxHashMap::default();
        relation_index_by_id.reserve(model.relations().len());
        for (idx, r) in model.relations().enumerate() {
            relation_index_by_id.insert(r.source.id.as_str(), idx + 1);
        }

        let mut note_by_id: FxHashMap<&str, ClassSvgNote<'_>> = FxHashMap::default();
        note_by_id.reserve(model.notes().len());
        for n in model.notes() {
            note_by_id.insert(n.id, n);
        }

        let mut iface_by_id: FxHashMap<&str, ClassSvgInterface<'_>> = FxHashMap::default();
        iface_by_id.reserve(model.interfaces().len());
        for i in model.interfaces() {
            iface_by_id.insert(i.id, i);
        }

        let class_color_indices = model
            .classes()
            .enumerate()
            .map(|(index, (id, _))| (id, index))
            .collect();
        Self {
            class_nodes_by_id,
            class_color_indices,
            relations_by_id,
            relation_index_by_id,
            note_by_id,
            iface_by_id,
        }
    }
}

pub(super) fn emit_class_render_timing(
    timings: &RenderTimings,
    detail: &ClassRenderDetails,
    layout: &ClassDiagramLayout,
) {
    eprintln!(
        "[render-timing] diagram=classDiagram total={:?} deserialize={:?} build_ctx={:?} viewbox={:?} render_svg={:?} finalize={:?} clusters={:?} edge_paths={:?} edge_curve={:?} edge_points_json={:?} edge_points_b64={:?} edge_labels={:?} nodes={:?} notes_sanitize={:?} path_bounds={:?} path_bounds_calls={} nodes_count={} edges_count={} clusters_count={}",
        timings.total,
        timings.deserialize_model,
        timings.build_ctx,
        timings.viewbox,
        timings.render_svg,
        timings.finalize_svg,
        detail.clusters,
        detail.edge_paths,
        detail.edge_curve,
        detail.edge_points_json,
        detail.edge_points_b64,
        detail.edge_labels,
        detail.nodes,
        detail.notes_sanitize,
        detail.path_bounds,
        detail.path_bounds_calls,
        layout.nodes.len(),
        layout.edges.len(),
        layout.clusters.len(),
    );
}
