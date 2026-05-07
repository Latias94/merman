use crate::generated::class_text_overrides_11_12_2 as class_text_overrides;
use crate::model::{Bounds, ClassDiagramV2Layout, LayoutCluster, LayoutNode};
use rustc_hash::FxHashMap;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use super::super::{escape_attr, escape_xml, fmt, fmt_into};
use super::bounds::include_xywh;
use super::{ClassSvgModel, ClassSvgNode};

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ClassNamespaceSubgraphState<'a> {
    pub active_subgraph: Option<&'a str>,
    pub root_offset: Option<(f64, f64)>,
}

pub(super) fn class_order_ids_for_namespace_subgraphs<'a>(
    ordered_ids: Vec<&'a str>,
    namespace_keys: &[&'a str],
    class_nodes_by_id: &FxHashMap<&'a str, &ClassSvgNode>,
) -> Vec<&'a str> {
    let mut inner: Vec<&str> = Vec::new();
    let mut used: HashSet<&str> = HashSet::new();

    for ns_id in namespace_keys {
        for id in &ordered_ids {
            let parent = class_nodes_by_id.get(*id).and_then(|n| n.parent.as_deref());
            if parent == Some(*ns_id) && used.insert(*id) {
                inner.push(*id);
            }
        }
    }

    let mut outer: Vec<&str> = Vec::new();
    for id in &ordered_ids {
        if !used.contains(id) {
            outer.push(*id);
        }
    }
    inner.into_iter().chain(outer).collect()
}

pub(super) struct ClassNodeRenderOrder<'a> {
    pub layout_nodes_by_id: FxHashMap<&'a str, &'a LayoutNode>,
    pub ordered_ids: Vec<&'a str>,
    pub namespace_key_set: HashSet<&'a str>,
    pub clusters_by_id: HashMap<&'a str, &'a LayoutCluster>,
}

pub(super) fn build_class_node_render_order<'a>(
    layout: &'a ClassDiagramV2Layout,
    model: &'a ClassSvgModel,
    class_nodes_by_id: &FxHashMap<&'a str, &ClassSvgNode>,
    wrap_nodes_root: bool,
    single_namespace_id: Option<&'a str>,
    render_namespaces_as_subgraphs: bool,
) -> ClassNodeRenderOrder<'a> {
    let mut layout_nodes_by_id: FxHashMap<&str, &LayoutNode> = FxHashMap::default();
    layout_nodes_by_id.reserve(layout.nodes.len());
    for n in &layout.nodes {
        if n.is_cluster {
            continue;
        }
        layout_nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut ordered_ids: Vec<&str> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    seen.reserve(model.classes.len() + model.notes.len() + model.interfaces.len());
    for cls in model.classes.values() {
        let id = cls.id.as_str();
        if seen.insert(id) {
            ordered_ids.push(id);
        }
    }
    for note in &model.notes {
        let id = note.id.as_str();
        if seen.insert(id) {
            ordered_ids.push(id);
        }
    }
    for iface in &model.interfaces {
        let id = iface.id.as_str();
        if seen.insert(id) {
            ordered_ids.push(id);
        }
    }
    for n in &layout.nodes {
        if n.is_cluster {
            continue;
        }
        let id = n.id.as_str();
        if seen.insert(id) {
            ordered_ids.push(id);
        }
    }

    if wrap_nodes_root {
        let mut inner: Vec<&str> = Vec::new();
        let mut outer: Vec<&str> = Vec::new();
        for id in &ordered_ids {
            let parent = class_nodes_by_id.get(*id).and_then(|n| n.parent.as_deref());
            if single_namespace_id.is_some_and(|ns| parent == Some(ns)) {
                inner.push(*id);
            } else {
                outer.push(*id);
            }
        }
        ordered_ids = inner.into_iter().chain(outer).collect();
    }

    let namespace_keys: Vec<&str> = crate::class::class_namespace_ids_in_decl_order(model);
    let namespace_key_set = namespace_keys.iter().copied().collect();

    let mut clusters_by_id: HashMap<&str, &LayoutCluster> = HashMap::new();
    for c in &layout.clusters {
        clusters_by_id.insert(c.id.as_str(), c);
    }

    if render_namespaces_as_subgraphs {
        // Ensure namespace-contained nodes are rendered in namespace order (one nested subgraph per
        // namespace) before emitting any non-namespace nodes at the outer level.
        ordered_ids = class_order_ids_for_namespace_subgraphs(
            ordered_ids,
            &namespace_keys,
            class_nodes_by_id,
        );
    }

    ClassNodeRenderOrder {
        layout_nodes_by_id,
        ordered_ids,
        namespace_key_set,
        clusters_by_id,
    }
}

pub(super) fn transition_class_namespace_subgraph<'a>(
    out: &mut String,
    content_bounds: &mut Option<Bounds>,
    state: &mut ClassNamespaceSubgraphState<'a>,
    parent: Option<&'a str>,
    clusters_by_id: &HashMap<&str, &LayoutCluster>,
) {
    if parent == state.active_subgraph {
        return;
    }

    close_class_namespace_subgraph(out, state);
    state.active_subgraph = parent;
    if let Some(ns_id) = state.active_subgraph {
        if let Some(c) = clusters_by_id.get(ns_id).copied() {
            let w = c.width.max(1.0);
            let h = c.height.max(1.0);
            let root_dx = c.x - w / 2.0 - 8.0;
            let root_dy = c.y - h / 2.0;
            state.root_offset = Some((root_dx, root_dy));

            out.push_str(r#"<g class="root" transform="translate("#);
            fmt_into(out, root_dx);
            out.push_str(r#", "#);
            fmt_into(out, root_dy);
            out.push_str(r#")">"#);
            out.push_str(r#"<g class="clusters">"#);

            let local_left = 8.0;
            let local_top = 8.0;
            let global_left = root_dx + local_left;
            let global_top = root_dy + local_top;
            include_xywh(content_bounds, global_left, global_top, w, h);

            let label_w = c.title_label.width.max(0.0);
            let label_h = 24.0;
            let local_label_x = local_left + (w - label_w) / 2.0;
            let local_label_y = local_top + c.title_margin_top;
            let global_label_x = root_dx + local_label_x;
            let global_label_y = root_dy + local_label_y;
            include_xywh(
                content_bounds,
                global_label_x,
                global_label_y,
                label_w,
                label_h,
            );

            let _ = write!(
                out,
                r#"<g class="cluster undefined" id="{}" data-look="classic"><rect x="{}" y="{}" width="{}" height="{}" style="fill:none !important;stroke:black !important"/><g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"><span class="nodeLabel"><p>{}</p></span></div></foreignObject></g></g>"#,
                escape_attr(&c.id),
                fmt(local_left),
                fmt(local_top),
                fmt(w),
                fmt(h),
                fmt(local_label_x),
                fmt(local_label_y),
                fmt(label_w),
                class_text_overrides::class_html_label_max_width_px(),
                escape_xml(&c.title)
            );
        } else {
            state.root_offset = Some((0.0, 0.0));
            out.push_str(r#"<g class="root" transform="translate(-8, 0)"><g class="clusters">"#);
        }

        out.push_str(r#"</g><g class="edgePaths"/><g class="edgeLabels"/><g class="nodes">"#);
    }
}

pub(super) fn close_class_namespace_subgraph(
    out: &mut String,
    state: &mut ClassNamespaceSubgraphState<'_>,
) {
    if state.active_subgraph.is_some() {
        out.push_str("</g>"); // namespace subgraph nodes
        out.push_str("</g>"); // namespace subgraph root
        state.active_subgraph = None;
        state.root_offset = None;
    }
}
