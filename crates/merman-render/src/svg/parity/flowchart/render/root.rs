//! Flowchart root renderer.

use super::super::*;

pub(in crate::svg::parity::flowchart) fn render_flowchart_root(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    cluster_id: Option<&str>,
    parent_origin_x: f64,
    parent_origin_y: f64,
    timing_enabled: bool,
    details: &mut FlowchartRenderDetails,
    edge_cache: Option<&FxHashMap<&str, FlowchartEdgePathCacheEntry>>,
) {
    details.root_calls += 1;

    let (origin_x, origin_y, transform_attr) = if let Some(cid) = cluster_id {
        if let Some(off) = flowchart_cluster_root_offsets(ctx, cid) {
            let rel_x = off.origin_x - parent_origin_x;
            let rel_y = off.abs_top_transform - parent_origin_y;
            (
                off.origin_x,
                off.origin_y,
                format!(
                    r#" transform="translate({}, {})""#,
                    fmt_display(rel_x),
                    fmt_display(rel_y)
                ),
            )
        } else {
            // Fallback: keep the group in the parent's coordinate space.
            (
                parent_origin_x,
                parent_origin_y,
                r#" transform="translate(0, 0)""#.to_string(),
            )
        }
    } else {
        (0.0, 0.0, String::new())
    };

    let _ = write!(out, r#"<g class="root"{}>"#, transform_attr);
    let content_origin_y = origin_y;

    let _g_clusters = detail_guard(timing_enabled, &mut details.clusters);
    let mut clusters_to_draw: Vec<&LayoutCluster> = Vec::new();
    if let Some(cid) = cluster_id {
        if ctx
            .subgraphs_by_id
            .get(cid)
            .is_some_and(|sg| sg.nodes.is_empty())
        {
            // Empty subgraphs are rendered as plain nodes in Mermaid (see flowchart-v2.spec.js
            // outgoing-links-4 baseline), so they should not emit cluster boxes.
        } else if let Some(cluster) = ctx.layout_clusters_by_id.get(cid) {
            clusters_to_draw.push(cluster);
        }
    }
    for id in ctx.subgraphs_by_id.keys() {
        if cluster_id.is_some_and(|cid| cid == *id) {
            continue;
        }
        if ctx
            .subgraphs_by_id
            .get(id)
            .is_some_and(|sg| sg.nodes.is_empty())
        {
            continue;
        }
        if ctx.recursive_clusters.contains(id) {
            continue;
        }
        if flowchart_effective_parent(ctx, *id) == cluster_id {
            if let Some(cluster) = ctx.layout_clusters_by_id.get(*id) {
                clusters_to_draw.push(cluster);
            }
        }
    }
    if clusters_to_draw.is_empty() {
        out.push_str(r#"<g class="clusters"/>"#);
    } else {
        // Mermaid emits clusters by traversing the Dagre graph hierarchy (pre-order over
        // `graph.children()`), which in practice matches a stable bottom-to-top ordering in the
        // upstream SVG baselines (see `flowchart-v2 outgoing-links-*` fixtures).
        fn is_ancestor(parent: &FxHashMap<&str, &str>, ancestor: &str, node: &str) -> bool {
            let mut cur: Option<&str> = Some(node);
            while let Some(id) = cur {
                let Some(p) = parent.get(id).copied() else {
                    break;
                };
                if p == ancestor {
                    return true;
                }
                cur = Some(p);
            }
            false
        }

        clusters_to_draw.sort_by(|a, b| {
            if a.id != b.id {
                if is_ancestor(&ctx.parent, &a.id, &b.id) {
                    return std::cmp::Ordering::Less;
                }
                if is_ancestor(&ctx.parent, &b.id, &a.id) {
                    return std::cmp::Ordering::Greater;
                }
            }

            let a_top_y = a.y - a.height / 2.0;
            let b_top_y = b.y - b.height / 2.0;
            let a_top_x = a.x - a.width / 2.0;
            let b_top_x = b.x - b.width / 2.0;
            let a_idx = ctx
                .subgraph_order
                .iter()
                .position(|id| *id == a.id.as_str());
            let b_idx = ctx
                .subgraph_order
                .iter()
                .position(|id| *id == b.id.as_str());
            if let (Some(ai), Some(bi)) = (a_idx, b_idx) {
                // Mermaid's cluster insertion order tracks the order in which subgraphs are
                // defined/registered, but for SVG output the baselines match a reverse (last
                // defined first) ordering for sibling cluster boxes.
                bi.cmp(&ai)
                    .then_with(|| b_top_y.total_cmp(&a_top_y))
                    .then_with(|| b_top_x.total_cmp(&a_top_x))
                    .then_with(|| a.id.cmp(&b.id))
            } else {
                b_top_y
                    .total_cmp(&a_top_y)
                    .then_with(|| b_top_x.total_cmp(&a_top_x))
                    .then_with(|| a.id.cmp(&b.id))
            }
        });
        out.push_str(r#"<g class="clusters">"#);
        for cluster in clusters_to_draw {
            render_flowchart_cluster(out, ctx, cluster, origin_x, content_origin_y);
        }
        out.push_str("</g>");
    }
    drop(_g_clusters);

    let _g_edges_select = detail_guard(timing_enabled, &mut details.edges_select);
    let edges = flowchart_edges_for_root(ctx, cluster_id);
    drop(_g_edges_select);

    let _g_edge_paths = detail_guard(timing_enabled, &mut details.edge_paths);
    if edges.is_empty() {
        out.push_str(r#"<g class="edgePaths"/>"#);
    } else {
        out.push_str(r#"<g class="edgePaths">"#);
        let mut scratch = FlowchartEdgeDataPointsScratch::default();
        for e in &edges {
            render_flowchart_edge_path(
                out,
                ctx,
                e,
                origin_x,
                content_origin_y,
                &mut scratch,
                edge_cache,
            );
        }
        out.push_str("</g>");
    }
    drop(_g_edge_paths);

    let _g_edge_labels = detail_guard(timing_enabled, &mut details.edge_labels);
    if edges.is_empty() {
        out.push_str(r#"<g class="edgeLabels"/>"#);
    } else {
        out.push_str(r#"<g class="edgeLabels">"#);
        if !ctx.edge_html_labels {
            // Mermaid's `createText(..., useHtmlLabels=false)` always creates a background `<rect>`,
            // but for empty labels it returns the `<text>` element instead of the wrapper `<g>`.
            // The unused wrapper `<g>` (with the `background` rect) remains as a direct child
            // under `.edgeLabels`. Mirror this by emitting one rect-group per empty label.
            for e in &edges {
                let label_text = e.label.as_deref().unwrap_or_default();
                let label_type = e.label_type.as_deref().unwrap_or("text");
                let label_plain = flowchart_label_plain_text(label_text, label_type, false);
                if label_plain.trim().is_empty() {
                    out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
                }
            }
        }
        for e in &edges {
            render_flowchart_edge_label(out, ctx, e, origin_x, content_origin_y);
        }
        out.push_str("</g>");
    }
    drop(_g_edge_labels);

    out.push_str(r#"<g class="nodes">"#);

    // Mermaid inserts node DOM elements in `graph.nodes()` insertion order while recursively
    // rendering extracted cluster graphs. Our layout captures that order per extracted root.
    let _g_dom_order = detail_guard(timing_enabled, &mut details.dom_order);
    let mut dom_order: Vec<&str> = ctx
        .dom_node_order_by_root
        .get(cluster_id.unwrap_or(""))
        .map(|ids| ids.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();
    if !dom_order.is_empty() {
        // Some upstream flowchart-v2 configurations can produce a DOM registration order that
        // only includes non-recursive clusters (clusters with external edges). These clusters do
        // not emit a node DOM element, so relying on the raw order would produce an empty
        // `.nodes` group. Fall back to our effective-parent ordering in that case.
        let mut emits_anything = false;
        for id in &dom_order {
            if ctx
                .subgraphs_by_id
                .get(id)
                .is_some_and(|sg| !sg.nodes.is_empty())
            {
                if ctx.recursive_clusters.contains(id) {
                    emits_anything = true;
                    break;
                }
                continue;
            }
            emits_anything = true;
            break;
        }
        if !emits_anything {
            dom_order.clear();
        }
    }

    if dom_order.is_empty() {
        // Fallback for v1 layouts: approximate by appending extracted cluster roots after
        // regular nodes.
        dom_order = flowchart_root_children_nodes(ctx, cluster_id);
        dom_order.extend(flowchart_root_children_clusters(ctx, cluster_id));
    }
    drop(_g_dom_order);

    for id in dom_order {
        if ctx
            .subgraphs_by_id
            .get(id)
            .is_some_and(|sg| !sg.nodes.is_empty())
        {
            // Non-recursive clusters render as cluster boxes (in `.clusters`) and do not emit a
            // node DOM element. Recursive clusters render as nested `.root` groups.
            if ctx.recursive_clusters.contains(id) {
                let nested_start = timing_enabled.then(std::time::Instant::now);
                render_flowchart_root(
                    out,
                    ctx,
                    Some(id),
                    origin_x,
                    origin_y,
                    timing_enabled,
                    details,
                    edge_cache,
                );
                if let Some(s) = nested_start {
                    details.nested_roots += s.elapsed();
                }
            }
            continue;
        }

        let node_start = timing_enabled.then(std::time::Instant::now);
        render_flowchart_node(
            out,
            ctx,
            id,
            origin_x,
            content_origin_y,
            timing_enabled,
            details,
        );
        if let Some(s) = node_start {
            details.nodes += s.elapsed();
        }
    }

    out.push_str("</g></g>");
}

pub(super) fn flowchart_wrap_svg_text_lines(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
    break_long_words: bool,
) -> Vec<String> {
    const EPS_PX: f64 = 0.125;
    let max_width_px = max_width_px.filter(|w| w.is_finite() && *w > 0.0);

    fn measure_w_px(measurer: &dyn TextMeasurer, style: &crate::text::TextStyle, s: &str) -> f64 {
        measurer.measure(s, style).width
    }

    fn split_token_to_width_px(
        measurer: &dyn TextMeasurer,
        style: &crate::text::TextStyle,
        tok: &str,
        max_width_px: f64,
    ) -> (String, String) {
        if max_width_px <= 0.0 {
            return (tok.to_string(), String::new());
        }
        let chars = tok.chars().collect::<Vec<_>>();
        if chars.is_empty() {
            return (String::new(), String::new());
        }

        let mut split_at = 1usize;
        for i in 1..=chars.len() {
            let head = chars[..i].iter().collect::<String>();
            let w = measure_w_px(measurer, style, &head);
            if w.is_finite() && w <= max_width_px + EPS_PX {
                split_at = i;
            } else {
                break;
            }
        }
        let head = chars[..split_at].iter().collect::<String>();
        let tail = chars[split_at..].iter().collect::<String>();
        (head, tail)
    }

    fn wrap_line_to_width_px(
        measurer: &dyn TextMeasurer,
        style: &crate::text::TextStyle,
        line: &str,
        max_width_px: f64,
        break_long_words: bool,
    ) -> Vec<String> {
        let mut tokens = std::collections::VecDeque::from(
            crate::text::DeterministicTextMeasurer::split_line_to_words(line),
        );
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            let candidate_trimmed = candidate.trim_end();
            if measure_w_px(measurer, style, candidate_trimmed) <= max_width_px + EPS_PX {
                cur = candidate;
                continue;
            }

            if !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
                cur.clear();
                tokens.push_front(tok);
                continue;
            }

            if tok == " " {
                continue;
            }

            if measure_w_px(measurer, style, tok.as_str()) <= max_width_px + EPS_PX {
                cur = tok;
                continue;
            }

            if !break_long_words {
                out.push(tok);
                continue;
            }

            let (head, tail) = split_token_to_width_px(measurer, style, &tok, max_width_px);
            out.push(head);
            if !tail.is_empty() {
                tokens.push_front(tail);
            }
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    let mut lines = Vec::new();
    for line in crate::text::DeterministicTextMeasurer::normalized_text_lines(text) {
        if let Some(w) = max_width_px {
            lines.extend(wrap_line_to_width_px(
                measurer,
                style,
                &line,
                w,
                break_long_words,
            ));
        } else {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        vec!["".to_string()]
    } else {
        lines
    }
}
