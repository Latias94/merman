use std::time::Duration;

use merman_model::{OperationPhase, families::flowchart::FlowSubgraph, runtime::OperationTimer};
use merman_render_core::{
    Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    svg_output::{SvgOutput, timing::RenderTiming},
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::plan::FlowchartHierarchyPlan;
use super::step_badges::StepBadges;
use crate::edge_labels::{write_edge_labels, write_elk_edge_labels};
use crate::markers::FlowchartMarkerEmissionPlan;
use crate::projection::{FlowchartRenderEdgeRef, FlowchartRenderModelRef};
use crate::svg::{FlowchartEdgeDataPointsScratch, FlowchartRootPaint};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowchartClusterPass {
    Complete,
    Background,
    Label,
}

/// Root traversal timings. The caller holds its separate node, label, and viewport counters.
#[derive(Debug, Default)]
pub(crate) struct FlowchartGraphTimings {
    pub root_calls: u32,
    pub clusters: Duration,
    pub edges_select: Duration,
    pub edge_paths: Duration,
    pub edge_labels: Duration,
    pub dom_order: Duration,
    pub nodes: Duration,
    pub nested_roots: Duration,
}

/// The admitted Flow graph and borrowed render facts used by its parity traversal.
///
/// Use the model, hierarchy facts, and work meter from the same prepared operation.
pub(crate) struct FlowchartGraph<'plan, 'data> {
    pub hierarchy: &'plan FlowchartHierarchyPlan<'data>,
    pub model: &'plan FlowchartRenderModelRef<'data>,
    pub work_meter: &'plan OperationWorkMeter,
    pub markers: &'plan FlowchartMarkerEmissionPlan<'plan>,
    pub step_badges: &'plan StepBadges<'plan>,
    pub struck_group_titles: Option<&'plan FxHashSet<String>>,
    pub subgraphs_by_id: &'plan FxHashMap<&'data str, &'data FlowSubgraph>,
    pub subgraph_ids_with_children: &'plan FxHashSet<&'data str>,
    pub recursive_clusters: &'plan FxHashSet<&'data str>,
    pub swimlane_label_edges: &'plan FxHashMap<&'data str, FlowchartRenderEdgeRef<'data>>,
    pub swimlane: bool,
    pub has_depth: bool,
    pub edge_html_labels: bool,
    pub dagre_merges_self_loop_segments: bool,
}

struct GraphEmission<'a, 'plan, 'data> {
    paint: &'a mut FlowchartRootPaint<'plan, 'data>,
    timing: RenderTiming,
    details: &'a mut FlowchartGraphTimings,
}

struct FlowchartRootFrame<'data, 'plan> {
    cluster_id: Option<&'data str>,
    parent_origin: (f64, f64),
    origin: (f64, f64),
    dom_order: &'plan [&'data str],
    next_dom_index: usize,
    initialized: bool,
    nested_start: Option<OperationTimer>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RootPass {
    Complete,
    Depth,
    Foreground,
}

impl<'data> FlowchartRootFrame<'data, '_> {
    fn new(
        cluster_id: Option<&'data str>,
        parent_origin: (f64, f64),
        nested_start: Option<OperationTimer>,
    ) -> Self {
        Self {
            cluster_id,
            parent_origin,
            origin: parent_origin,
            dom_order: &[],
            next_dom_index: 0,
            initialized: false,
            nested_start,
        }
    }
}

impl<'plan, 'data> FlowchartGraph<'plan, 'data> {
    /// Emits the marker and graph children of the document's outer group.
    pub fn write<S: SvgOutput>(
        &self,
        out: &mut S,
        paint: &mut FlowchartRootPaint<'_, '_>,
        timing: RenderTiming,
        details: &mut FlowchartGraphTimings,
    ) -> Result<()> {
        self.markers.push_base_markers(out)?;
        let mut emission = GraphEmission {
            paint,
            timing,
            details,
        };
        if self.hierarchy.uses_flat_adapter_dom {
            out.checkpoint()?;
            self.markers.push_extra_markers(out)?;
            out.checkpoint()?;
            return self.write_flat(out, &mut emission);
        }

        if self.has_depth {
            self.write_root_pass(out, &mut emission, RootPass::Depth)?;
            self.write_root_pass(out, &mut emission, RootPass::Foreground)?;
        } else {
            self.write_root_pass(out, &mut emission, RootPass::Complete)?;
        }
        self.markers.push_extra_markers(out)
    }

    fn checkpoint_emit(&self) -> Result<()> {
        self.work_meter.checkpoint(OperationPhase::Emit)?;
        Ok(())
    }

    fn write_root_pass<S: SvgOutput>(
        &self,
        out: &mut S,
        emission: &mut GraphEmission<'_, '_, '_>,
        pass: RootPass,
    ) -> Result<()> {
        self.checkpoint_emit()?;
        let mut stack = vec![FlowchartRootFrame::new(None, (0.0, 0.0), None)];

        while let Some(mut frame) = stack.pop() {
            if pass == RootPass::Depth {
                self.work_meter.charge(1)?;
            }
            self.checkpoint_emit()?;
            if !frame.initialized {
                self.initialize_root_frame(out, emission, &mut frame, pass)?;
            }

            let mut nested = None;
            while frame.next_dom_index < frame.dom_order.len() {
                self.checkpoint_emit()?;
                let id = frame.dom_order[frame.next_dom_index];
                frame.next_dom_index += 1;
                if pass == RootPass::Depth {
                    self.work_meter.charge(1)?;
                }

                if let Some(edge) = self.swimlane_label_edges.get(id).copied() {
                    if pass != RootPass::Depth {
                        emission
                            .paint
                            .write_swimlane_edge_label(out, id, edge, frame.origin)?;
                        self.checkpoint_emit()?;
                        out.checkpoint()?;
                    }
                    continue;
                }

                if self.subgraphs_by_id.contains_key(id)
                    && self.subgraph_ids_with_children.contains(id)
                    && !self.model.is_subgraph_collapsed(id)
                {
                    // Non-recursive clusters are already painted in `.clusters`.
                    if self.recursive_clusters.contains(id) {
                        let nested_start = emission.timing.start();
                        nested = Some(FlowchartRootFrame::new(
                            Some(id),
                            frame.origin,
                            nested_start,
                        ));
                        break;
                    }
                    continue;
                }

                let node_start = emission.timing.start();
                emission
                    .paint
                    .write_node(out, id, frame.origin, pass == RootPass::Depth)?;
                self.checkpoint_emit()?;
                out.checkpoint()?;
                if let Some(start) = node_start {
                    emission.details.nodes += start.elapsed();
                }
            }

            if let Some(child) = nested {
                stack.push(frame);
                stack.push(child);
                continue;
            }

            out.push_str("</g>");
            if pass != RootPass::Depth {
                self.step_badges.write_root(out, frame.cluster_id)?;
            }
            out.push_str("</g>");
            out.checkpoint()?;
            if let Some(start) = frame.nested_start {
                emission.details.nested_roots += start.elapsed();
            }
        }
        Ok(())
    }

    fn initialize_root_frame<S: SvgOutput>(
        &self,
        out: &mut S,
        emission: &mut GraphEmission<'_, '_, '_>,
        frame: &mut FlowchartRootFrame<'data, 'plan>,
        pass: RootPass,
    ) -> Result<()> {
        self.checkpoint_emit()?;
        emission.details.root_calls += 1;
        let root_emission = self.hierarchy.root(frame.cluster_id)?;

        let (origin, transform_attr) = if let Some(cid) = frame.cluster_id {
            if let Some(off) = self.hierarchy.root_offsets(cid) {
                let rel_x = off.origin_x - frame.parent_origin.0;
                let rel_y = off.abs_top_transform - frame.parent_origin.1;
                (
                    (off.origin_x, off.origin_y),
                    format!(r#" transform="translate({},{})""#, fmt(rel_x), fmt(rel_y)),
                )
            } else {
                (
                    frame.parent_origin,
                    r#" transform="translate(0,0)""#.to_string(),
                )
            }
        } else {
            ((0.0, 0.0), String::new())
        };
        frame.origin = origin;
        let _ = write!(out, r#"<g class="root"{}>"#, transform_attr);

        let clusters_timer = emission.timing.section(&mut emission.details.clusters);
        let clusters_to_draw = root_emission.clusters();
        let mut struck_titles = Vec::new();
        if clusters_to_draw.is_empty() {
            out.push_str(r#"<g class="clusters"/>"#);
        } else {
            out.push_str(r#"<g class="clusters">"#);
            for &cluster in clusters_to_draw {
                self.checkpoint_emit()?;
                let cluster_pass = match pass {
                    RootPass::Depth => FlowchartClusterPass::Background,
                    _ if self
                        .struck_group_titles
                        .is_some_and(|titles| titles.contains(cluster.id.as_str())) =>
                    {
                        struck_titles.push(cluster);
                        if pass == RootPass::Foreground {
                            continue;
                        }
                        FlowchartClusterPass::Background
                    }
                    RootPass::Complete => FlowchartClusterPass::Complete,
                    RootPass::Foreground => FlowchartClusterPass::Label,
                };
                emission
                    .paint
                    .write_cluster(out, cluster, origin, cluster_pass)?;
                out.checkpoint()?;
            }
            out.push_str("</g>");
        }
        drop(clusters_timer);

        if pass == RootPass::Depth {
            out.push_str(r#"<g class="nodes" data-merman-theme-depth="layer">"#);
            frame.dom_order = root_emission.dom_order();
            frame.initialized = true;
            return out.checkpoint();
        }

        let edges_select = emission.timing.section(&mut emission.details.edges_select);
        let edges = root_emission.edges();
        drop(edges_select);

        self.write_edge_paths(out, emission, edges, origin)?;

        if !struck_titles.is_empty() {
            let _clusters_timer = emission.timing.section(&mut emission.details.clusters);
            out.push_str(r#"<g class="merman-struck-cluster-titles">"#);
            for cluster in struck_titles {
                emission
                    .paint
                    .write_cluster(out, cluster, origin, FlowchartClusterPass::Label)?;
                out.checkpoint()?;
            }
            out.push_str("</g>");
        }

        let edge_labels_timer = emission.timing.section(&mut emission.details.edge_labels);
        let label_edges = if self.swimlane { &[] } else { edges };
        let merge_self_loop_segments =
            self.edge_html_labels || label_edges.is_empty() || self.dagre_merges_self_loop_segments;
        write_edge_labels(
            out,
            label_edges,
            self.edge_html_labels,
            merge_self_loop_segments,
            || self.checkpoint_emit(),
            |edge| self.edge_label_is_empty(edge),
            |out, edge| emission.paint.write_edge_label(out, edge, origin),
        )?;
        drop(edge_labels_timer);

        out.push_str(r#"<g class="nodes">"#);
        let dom_order_timer = emission.timing.section(&mut emission.details.dom_order);
        let dom_order = root_emission.dom_order();
        drop(dom_order_timer);
        frame.dom_order = dom_order;
        frame.initialized = true;
        out.checkpoint()
    }

    fn edge_label_is_empty(&self, edge: FlowchartRenderEdgeRef<'_>) -> bool {
        // The FlowDB label is already trimmed. Markup-only labels can still have visible DOM.
        self.model
            .edge_label_for_occurrence(edge.key, edge.edge)
            .unwrap_or_default()
            .is_empty()
    }

    fn write_edge_paths<S: SvgOutput>(
        &self,
        out: &mut S,
        emission: &mut GraphEmission<'_, '_, '_>,
        edges: &[FlowchartRenderEdgeRef<'_>],
        origin: (f64, f64),
    ) -> Result<()> {
        let flat = self.hierarchy.uses_flat_adapter_dom;
        if flat {
            self.checkpoint_emit()?;
        }
        emission.paint.prepare_edges(edges)?;
        let _edge_paths_timer = emission.timing.section(&mut emission.details.edge_paths);
        if flat {
            if edges.is_empty() {
                out.push_str(r#"<g class="edges edgePaths"/>"#);
                return out.checkpoint();
            }
            out.push_str(r#"<g class="edges edgePaths">"#);
        } else {
            let class = if self.swimlane {
                "edges edgePaths"
            } else {
                "edgePaths"
            };
            if edges.is_empty() {
                let _ = write!(out, r#"<g class="{}"/>"#, class);
                return Ok(());
            }
            let _ = write!(out, r#"<g class="{}">"#, class);
        }

        let mut scratch = FlowchartEdgeDataPointsScratch::default();
        for &edge in edges {
            self.checkpoint_emit()?;
            emission.paint.write_edge(out, edge, origin, &mut scratch)?;
            self.checkpoint_emit()?;
            out.checkpoint()?;
        }
        out.push_str("</g>");
        if flat {
            out.checkpoint()?;
        }
        Ok(())
    }

    fn write_flat<S: SvgOutput>(
        &self,
        out: &mut S,
        emission: &mut GraphEmission<'_, '_, '_>,
    ) -> Result<()> {
        self.checkpoint_emit()?;
        emission.details.root_calls += 1;

        // The common Mermaid layout renderer groups the flat layout under one `.root` wrapper.
        out.push_str(r#"<g class="root">"#);
        self.write_flat_subgraphs(
            out,
            emission,
            if self.has_depth {
                FlowchartClusterPass::Background
            } else {
                FlowchartClusterPass::Complete
            },
        )?;

        if self.has_depth {
            out.push_str(r#"<g class="nodes" data-merman-theme-depth="layer">"#);
            for &id in self.hierarchy.root(None)?.dom_order() {
                self.checkpoint_emit()?;
                emission.paint.write_node(out, id, (0.0, 0.0), true)?;
            }
            out.push_str("</g>");
            out.checkpoint()?;
            self.write_flat_subgraphs(out, emission, FlowchartClusterPass::Label)?;
        }

        let edges_select = emission.timing.section(&mut emission.details.edges_select);
        let edges = self.hierarchy.ordered_edges();
        drop(edges_select);

        self.write_edge_paths(out, emission, edges, (0.0, 0.0))?;

        self.checkpoint_emit()?;
        let edge_labels_timer = emission.timing.section(&mut emission.details.edge_labels);
        #[cfg(feature = "layout-grid")]
        let edges = if emission.paint.paints_grid_edge_label_nodes() {
            &[]
        } else {
            edges
        };
        write_elk_edge_labels(
            out,
            edges,
            || self.checkpoint_emit(),
            |edge| self.edge_label_is_empty(edge),
            |out, edge| emission.paint.write_edge_label(out, edge, (0.0, 0.0)),
        )?;
        drop(edge_labels_timer);

        self.write_flat_nodes(out, emission)?;
        self.step_badges.write_root(out, None)?;
        out.push_str("</g>");
        out.checkpoint()
    }

    fn write_flat_subgraphs<S: SvgOutput>(
        &self,
        out: &mut S,
        emission: &mut GraphEmission<'_, '_, '_>,
        pass: FlowchartClusterPass,
    ) -> Result<()> {
        self.checkpoint_emit()?;
        let _clusters_timer = emission.timing.section(&mut emission.details.clusters);
        let clusters_to_draw = self.hierarchy.root(None)?.clusters();
        if clusters_to_draw.is_empty() {
            out.push_str(r#"<g class="clusters"/>"#);
            return out.checkpoint();
        }

        out.push_str(r#"<g class="clusters">"#);
        for &cluster in clusters_to_draw {
            self.checkpoint_emit()?;
            emission
                .paint
                .write_cluster(out, cluster, (0.0, 0.0), pass)?;
            out.checkpoint()?;
        }
        out.push_str("</g>");
        out.checkpoint()
    }

    fn write_flat_nodes<S: SvgOutput>(
        &self,
        out: &mut S,
        emission: &mut GraphEmission<'_, '_, '_>,
    ) -> Result<()> {
        self.checkpoint_emit()?;
        out.push_str(r#"<g class="nodes">"#);

        let dom_order_timer = emission.timing.section(&mut emission.details.dom_order);
        let dom_order = self.hierarchy.root(None)?.dom_order();
        drop(dom_order_timer);

        for &id in dom_order {
            self.checkpoint_emit()?;
            // Flat adapters paint subgraphs as clusters, including empty ones.
            if self.subgraphs_by_id.contains_key(id) && !self.model.is_subgraph_collapsed(id) {
                continue;
            }

            let node_start = emission.timing.start();
            emission.paint.write_node(out, id, (0.0, 0.0), false)?;
            self.checkpoint_emit()?;
            out.checkpoint()?;
            if let Some(start) = node_start {
                emission.details.nodes += start.elapsed();
            }
        }

        #[cfg(feature = "layout-grid")]
        if emission.paint.paints_grid_edge_label_nodes() {
            let _timer = emission.timing.section(&mut emission.details.edge_labels);
            emission
                .paint
                .write_grid_edge_label_nodes(out, self.hierarchy.ordered_edges())?;
        }

        out.push_str("</g>");
        out.checkpoint()
    }
}
