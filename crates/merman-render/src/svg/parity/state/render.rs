use super::*;

pub(in crate::svg::parity) fn render_state_diagram_svg_model(
    layout: &StateDiagramLayout,
    model: &StateSvgModel,
    label_sidecar: &crate::state::StateLabelSidecar,
    effect_evidence: &crate::state::StateSvgEffectEvidenceRecorder,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    label_sidecar.validate_for_render(model, layout)?;
    let timing = options.timing();
    let mut timings = super::timing::RenderTimings::default();
    let total_timer = timing.start();

    let diagram_id = options.diagram_id_or("merman");
    let style_plan = options
        .state_style_plan()
        .ok_or_else(|| Error::InvalidModel {
            message: "state SVG rendering requires the pre-layout family style plan".to_string(),
        })?;

    let _g_build_ctx = timing.section(&mut timings.build_ctx);

    let mut hidden_prefixes: Vec<String> = Vec::new();
    for (id, st) in &model.states {
        let Some(note) = st.note.as_ref() else {
            continue;
        };
        if note.text.trim().is_empty() {
            continue;
        }
        if note.position.is_none() {
            hidden_prefixes.push(id.clone());
        }
    }

    // Mermaid computes the final root viewport from DOM `svg.getBBox()` plus a fixed padding
    // (`setupViewPortForSVG(svg, padding=8)`). It does *not* pre-normalize the coordinate space by
    // shifting the entire rendered graph to start at (0,0).
    //
    // Keep the top-level origin at (0,0) and derive `viewBox` / `max-width` later from the emitted
    // SVG bounds approximation (see below).
    let viewport_padding = 8.0;
    let origin_x = 0.0;
    let origin_y = 0.0;

    let diagram_title = diagram_title
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let state_render_settings = crate::state::StateConfigView::new(effective_config)
        .render_settings(style_plan.compatibility());
    let serialized_diagram_look = crate::config::config_diagram_look(effective_config)
        .serialized()
        .to_string();
    let title_top_margin = state_render_settings.title_top_margin;
    let hand_drawn_seed = options.rough_randomness(
        state_render_settings.hand_drawn_seed,
        "render.state.roughjs",
    );
    let rough_cache = StateRoughCache::default();

    let has_acc_title = model
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = model
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());

    let mut nodes_by_id: FxHashMap<&str, &StateSvgNode> =
        FxHashMap::with_capacity_and_hasher(model.nodes.len(), Default::default());
    for n in &model.nodes {
        nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut layout_nodes_by_id: FxHashMap<&str, &LayoutNode> =
        FxHashMap::with_capacity_and_hasher(layout.nodes.len(), Default::default());
    for n in &layout.nodes {
        layout_nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut layout_edges_by_id: FxHashMap<&str, &crate::model::LayoutEdge> =
        FxHashMap::with_capacity_and_hasher(layout.edges.len(), Default::default());
    for e in &layout.edges {
        layout_edges_by_id.insert(e.id.as_str(), e);
    }

    let mut layout_clusters_by_id: FxHashMap<&str, &LayoutCluster> =
        FxHashMap::with_capacity_and_hasher(layout.clusters.len(), Default::default());
    for c in &layout.clusters {
        layout_clusters_by_id.insert(c.id.as_str(), c);
    }

    let mut parent: FxHashMap<&str, &str> =
        FxHashMap::with_capacity_and_hasher(model.nodes.len(), Default::default());
    for n in &model.nodes {
        if let Some(p) = n.parent_id.as_deref() {
            parent.insert(n.id.as_str(), p);
        }
    }

    // Mermaid's state diagram DOM insertion order follows the order of `StateDB.getData().nodes`
    // (see `dataFetcher.ts` + dagre renderer `graph.nodes()` iteration). Our semantic model's
    // `nodes` already preserves that first-seen insertion order, so use it directly.
    let node_order: Vec<&str> = model.nodes.iter().map(|n| n.id.as_str()).collect();

    let mut ctx = StateRenderCtx {
        diagram_id,
        diagram_look: state_render_settings.diagram_look,
        serialized_diagram_look,
        hand_drawn_seed,
        html_labels: state_render_settings.html_labels,
        html_label_wrapping_width: state_render_settings.html_label_wrapping_width,
        state_padding: state_render_settings.state_padding,
        node_order,
        nodes_by_id,
        layout_nodes_by_id,
        layout_edges_by_id,
        layout_clusters_by_id,
        parent,
        nested_roots: std::collections::BTreeSet::new(),
        hidden_prefixes,
        security_level_loose: state_render_settings.security_level_loose,
        links: &model.links,
        states: &model.states,
        edges: &model.edges,
        include_edges: options.debug.include_edges,
        include_nodes: options.debug.include_nodes,
        measurer,
        label_sidecar,
        effect_evidence,
        style_plan,
        theme_receipt: std::cell::RefCell::new(style_plan.begin_terminal_theme_receipt(
            options.debug.include_nodes,
            options.debug.include_edges,
        )),
        rough_cache,
    };

    fn compute_state_nested_roots(ctx: &StateRenderCtx<'_>) -> std::collections::BTreeSet<String> {
        let mut out: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        let mut composite_self_loops: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for e in ctx.edges {
            if state_is_hidden(ctx, e.start.as_str())
                || state_is_hidden(ctx, e.end.as_str())
                || state_is_hidden(ctx, e.id.as_str())
            {
                continue;
            }
            if e.start != e.end {
                continue;
            }
            let id = e.start.as_str();
            let Some(n) = ctx.nodes_by_id.get(id).copied() else {
                continue;
            };
            if n.is_group && n.shape != "noteGroup" {
                composite_self_loops.insert(id);
            }
        }

        let mut composite_externals: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for e in ctx.edges {
            if state_is_hidden(ctx, e.start.as_str())
                || state_is_hidden(ctx, e.end.as_str())
                || state_is_hidden(ctx, e.id.as_str())
            {
                continue;
            }
            let a = state_endpoint_context_raw(ctx, e.start.as_str());
            let b = state_endpoint_context_raw(ctx, e.end.as_str());
            let ca = state_context_chain_raw(ctx, a);
            let cb = state_context_chain_raw(ctx, b);

            for anc in &ca {
                let Some(id) = *anc else {
                    continue;
                };
                if cb.contains(anc) {
                    continue;
                }
                let Some(n) = ctx.nodes_by_id.get(id).copied() else {
                    continue;
                };
                if n.is_group && n.shape != "noteGroup" {
                    composite_externals.insert(id);
                }
            }
            for anc in &cb {
                let Some(id) = *anc else {
                    continue;
                };
                if ca.contains(anc) {
                    continue;
                }
                let Some(n) = ctx.nodes_by_id.get(id).copied() else {
                    continue;
                };
                if n.is_group && n.shape != "noteGroup" {
                    composite_externals.insert(id);
                }
            }
        }

        for e in ctx.edges {
            if state_is_hidden(ctx, e.start.as_str())
                || state_is_hidden(ctx, e.end.as_str())
                || state_is_hidden(ctx, e.id.as_str())
            {
                continue;
            }
            // Mermaid avoids creating a nested root for composites that have a self-loop edge on
            // the composite itself (e.g. `Active --> Active`).
            if composite_self_loops.contains(e.start.as_str()) && e.start == e.end {
                continue;
            }
            let Some(c) = state_edge_context_raw(ctx, e) else {
                continue;
            };
            if composite_externals.contains(c) {
                continue;
            }
            out.insert(c.to_string());
        }

        // Mermaid usually renders composite states in a nested root even when they don't contain
        // internal transitions, but it avoids doing so when the composite has a self-loop edge.
        for (child_id, parent_id) in &ctx.parent {
            if state_is_hidden(ctx, child_id) || state_is_hidden(ctx, parent_id) {
                continue;
            }
            if composite_self_loops.contains(parent_id) {
                continue;
            }
            if composite_externals.contains(parent_id) {
                continue;
            }
            let Some(pn) = ctx.nodes_by_id.get(parent_id).copied() else {
                continue;
            };
            if pn.is_group && pn.shape != "noteGroup" {
                out.insert((*parent_id).to_string());
            }
        }

        // If a nested graph is needed for a descendant composite state, Mermaid also nests
        // its composite state ancestors.
        let seeds: Vec<String> = out.iter().cloned().collect();
        for cid in seeds {
            let mut cur: Option<&str> = Some(cid.as_str());
            while let Some(id) = cur {
                let Some(pid) = ctx.parent.get(id).copied() else {
                    break;
                };
                let Some(pn) = ctx.nodes_by_id.get(pid).copied() else {
                    cur = Some(pid);
                    continue;
                };
                if pn.is_group && pn.shape != "noteGroup" {
                    if composite_self_loops.contains(pid) || composite_externals.contains(pid) {
                        cur = Some(pid);
                        continue;
                    }
                    out.insert(pid.to_string());
                }
                cur = Some(pid);
            }
        }

        out
    }

    ctx.nested_roots = compute_state_nested_roots(&ctx);

    drop(_g_build_ctx);

    let _g_render_svg = timing.section(&mut timings.render_svg);

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let root_context =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::STATE, diagram_id)
            .with_resource_policy(options.resource_policy());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "stateDiagram");
    root_chrome.class = Some("statediagram");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = root_svg::RootDomProfile {
        aria_attr_order: root_svg::SvgRootAriaAttrOrder::LabelledbyThenDescribedby,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    let root_document = root_context.begin_document(
        &mut out,
        root_svg::DeferredRootSpec::responsive(),
        root_chrome,
    )?;
    options.checkpoint_emit()?;

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}"#,
            diagram_id,
            escape_xml_display(model.acc_title.as_deref().unwrap_or_default())
        );
        out.push_str("</title>");
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}"#,
            diagram_id,
            escape_xml_display(model.acc_descr.as_deref().unwrap_or_default())
        );
        out.push_str("</desc>");
    }
    out.checkpoint()?;

    // Mermaid emits a single `<style>` element with diagram-scoped CSS. Stream it directly into
    // the bounded document so a large public diagram id or class catalog cannot allocate an
    // unbounded temporary stylesheet before `MaxSvgBytes` admission.
    out.push_str("<style>");
    write_state_css(&mut out, diagram_id, style_plan)?;
    out.push_str("</style>");
    out.checkpoint()?;

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");
    state_markers(&mut out, diagram_id, &ctx);

    // `svg.getBBox()` does not include `<style>` and typically excludes non-rendered `<defs>`
    // content from the rendered bbox. Scan only the rendered graph payload to reduce overhead
    // in our SVG bounds approximation.
    let bounds_scan_start = out.len();
    let mut detail = StateRenderDetails::default();
    let mut effect_outsets = crate::diagram_theme::EffectOutsets::default();
    render_state_root(
        &mut out,
        &ctx,
        None,
        (origin_x, origin_y),
        options,
        timing,
        &mut detail,
        &mut effect_outsets,
    )?;
    let bounds_scan_end = out.len();

    out.push_str("</g>");
    state_root_defs(&mut out, diagram_id, style_plan);
    out.checkpoint()?;

    drop(_g_render_svg);

    let mut viewbox_svg_scan = std::time::Duration::ZERO;
    let _g_viewbox = timing.section(&mut timings.viewbox);
    let _g_scan = timing.section(&mut viewbox_svg_scan);
    let geometry_bounds =
        svg_emitted_bounds_from_svg(&out.as_str()[bounds_scan_start..bounds_scan_end])
            .or_else(|| state_viewport_bounds_from_layout(layout))
            .unwrap_or(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            });
    let mut paint_bounds = geometry_bounds.clone();
    paint_bounds.min_x -= effect_outsets.left;
    paint_bounds.max_x += effect_outsets.right;
    paint_bounds.min_y -= effect_outsets.top;
    paint_bounds.max_y += effect_outsets.bottom;
    drop(_g_scan);

    let title_emission = if let Some(title) = diagram_title.as_deref() {
        // Mermaid centers the title using the pre-title geometry bbox. Paint-only effect outsets
        // expand the viewport, but they do not move diagram geometry or its title anchor:
        // `x = bbox.x + bbox.width/2`, `y = -titleTopMargin`.
        let title_x = (geometry_bounds.min_x + geometry_bounds.max_x) / 2.0;
        let title_y = -title_top_margin;

        let title_style = style_plan.title_text_style();
        let (title_left, title_right) = measurer.measure_svg_title_bbox_x(title, title_style);

        let (ascent, descent) = crate::text::svg_title_bbox_vertical_extents_px(title_style);

        paint_bounds.min_x = paint_bounds.min_x.min(title_x - title_left);
        paint_bounds.max_x = paint_bounds.max_x.max(title_x + title_right);
        paint_bounds.min_y = paint_bounds.min_y.min(title_y - ascent);
        paint_bounds.max_y = paint_bounds.max_y.max(title_y + descent);

        Some((title, title_x, title_y))
    } else {
        None
    };

    let root_bounds = root_svg::DiagramBounds::from_extents(
        paint_bounds.min_x,
        paint_bounds.min_y,
        paint_bounds.max_x,
        paint_bounds.max_y,
        viewport_padding,
    );

    let document_len_before_root_finalize = out.len();
    let root_document = root_context.finish_document(
        &mut out,
        root_document,
        root_svg::RootViewportSpec::responsive(root_bounds)
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width)),
    )?;
    ctx.theme_receipt
        .borrow_mut()
        .shift_emissions_after_prefix_rewrite(document_len_before_root_finalize, out.len());

    drop(_g_viewbox);
    let _g_finalize = timing.section(&mut timings.finalize_svg);

    if let Some((title, title_x, title_y)) = title_emission {
        let terminal_start = out.len();
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="statediagramTitleText""#,
            fmt(title_x),
            fmt(title_y),
        );
        if !style_plan.title_style_attr().is_empty() {
            let _ = write!(
                &mut out,
                r#" style="{}""#,
                escape_attr_display(style_plan.title_style_attr())
            );
        }
        let _ = write!(&mut out, ">{}</text>", escape_xml_display(title));
        out.checkpoint()?;
        style_plan.record_title_terminal_emission(
            &mut ctx.theme_receipt.borrow_mut(),
            terminal_start..out.len(),
        );
    }
    out.push_str("</svg>\n");
    let out = out.finish()?;

    ctx.theme_receipt
        .borrow_mut()
        .observe_svg(&out, options.work_meter())?;
    let terminal_theme_receipt = ctx.theme_receipt.borrow().clone();
    let _ = style_plan.record_terminal_theme_receipt(terminal_theme_receipt);

    drop(_g_finalize);
    timings.total = total_timer
        .map(merman_core::runtime::OperationTimer::elapsed)
        .unwrap_or_default();
    if timing.is_enabled() {
        eprintln!(
            "[render-timing] diagram=stateDiagram total={:?} deserialize={:?} build_ctx={:?} render_svg={:?} viewbox={:?} viewbox_svg_scan={:?} finalize={:?} root_calls={} clusters={:?} edge_paths={:?} edge_labels={:?} leaf_nodes={:?} leaf_style_parse={:?} leaf_roughjs={:?} leaf_roughjs_calls={} leaf_roughjs_unique={} leaf_measure={:?} leaf_label_html={:?} leaf_emit={:?} nested_roots={:?} self_loop_placeholders={:?}",
            timings.total,
            timings.deserialize_model,
            timings.build_ctx,
            timings.render_svg,
            timings.viewbox,
            viewbox_svg_scan,
            timings.finalize_svg,
            detail.root_calls,
            detail.clusters,
            detail.edge_paths,
            detail.edge_labels,
            detail.leaf_nodes,
            detail.leaf_nodes_style_parse,
            detail.leaf_nodes_roughjs,
            detail.leaf_roughjs_calls,
            detail.leaf_roughjs_unique.len(),
            detail.leaf_nodes_measure,
            detail.leaf_nodes_label_html,
            detail.leaf_nodes_emit,
            detail.nested_roots,
            detail.self_loop_placeholders,
        );
    }
    root_document.complete(out)
}

fn render_state_root(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    root: Option<&str>,
    parent_origin: (f64, f64),
    options: &SvgExecution<'_>,
    timing: super::timing::RenderTiming,
    details: &mut StateRenderDetails,
    effect_outsets: &mut crate::diagram_theme::EffectOutsets,
) -> Result<()> {
    details.root_calls += 1;
    options.checkpoint_emit()?;
    let (parent_origin_x, parent_origin_y) = parent_origin;

    // Mermaid's dagre-wrapper uses a fixed graph margin (`marginx/marginy=8`). For nested state
    // roots (extracted cluster graphs), Mermaid keeps the root cluster frame at x/y=8 in the
    // nested coordinate space and compensates via the root group's `translate(...)`.
    //
    // If we anchor the nested origin at the cluster's top-left, the emitted cluster rect starts at
    // (0,0) and the root group's transform drifts from upstream DOM. Shift the origin by the fixed
    // margin so nested roots start at (8,8), matching Mermaid's SVG structure more closely.
    const GRAPH_MARGIN_PX: f64 = 8.0;

    let (origin_x, origin_y, transform_attr) = if let Some(root_id) = root {
        if let Some(c) = ctx.layout_clusters_by_id.get(root_id).copied() {
            let left = c.x - c.width / 2.0;
            let top = c.y - c.height / 2.0;
            let origin_x = left - GRAPH_MARGIN_PX;
            let origin_y = top - GRAPH_MARGIN_PX;
            let tx = origin_x - parent_origin_x;
            let ty = origin_y - parent_origin_y;
            (
                origin_x,
                origin_y,
                format!(r#" transform="translate({}, {})""#, fmt(tx), fmt(ty)),
            )
        } else {
            (
                parent_origin_x,
                parent_origin_y,
                r#" transform="translate(0, 0)""#.to_string(),
            )
        }
    } else {
        (parent_origin_x, parent_origin_y, String::new())
    };

    let _ = write!(out, r#"<g class="root"{}>"#, transform_attr);

    // clusters
    let _g_clusters = detail_guard(timing, &mut details.clusters);
    out.push_str(r#"<g class="clusters">"#);
    if let Some(root_id) = root {
        render_state_cluster(out, ctx, root_id, origin_x, origin_y);
        out.checkpoint()?;
    }

    for &cluster_id in &ctx.node_order {
        if root == Some(cluster_id) {
            continue;
        }
        if !ctx.layout_clusters_by_id.contains_key(cluster_id) {
            continue;
        }
        if state_is_hidden(ctx, cluster_id) {
            continue;
        }
        if ctx.nested_roots.contains(cluster_id) {
            continue;
        }
        let Some(node) = ctx.nodes_by_id.get(cluster_id).copied() else {
            continue;
        };
        if !node.is_group || node.shape == "noteGroup" {
            continue;
        }
        if state_insertion_context(ctx, cluster_id) != root {
            continue;
        }
        render_state_cluster(out, ctx, cluster_id, origin_x, origin_y);
        out.checkpoint()?;
    }

    for &cluster_id in &ctx.node_order {
        if !ctx.layout_clusters_by_id.contains_key(cluster_id) {
            continue;
        }
        let Some(cluster) = ctx.layout_clusters_by_id.get(cluster_id).copied() else {
            continue;
        };
        if state_is_hidden(ctx, cluster_id) {
            continue;
        }
        let Some(node) = ctx.nodes_by_id.get(cluster_id).copied() else {
            continue;
        };
        if node.shape != "noteGroup" {
            continue;
        }
        let note_owner = state_note_owner_id(cluster_id);
        if ctx.hidden_prefixes.iter().any(|p| p == note_owner) {
            continue;
        }
        let has_position = ctx
            .states
            .get(note_owner)
            .and_then(|s| s.note.as_ref())
            .and_then(|n| n.position.as_ref())
            .is_some();
        if !has_position {
            continue;
        }

        let target_root = state_insertion_context(ctx, note_owner);
        if target_root != root {
            continue;
        }

        let left = cluster.x - cluster.width / 2.0;
        let top = cluster.y - cluster.height / 2.0;
        let x = left - origin_x;
        let y = top - origin_y;
        let dom_id = state_node_scoped_dom_id(ctx, cluster_id);
        let _ = write!(
            out,
            r#"<g id="{}" class="note-cluster"><rect x="{}" y="{}" width="{}" height="{}" fill="none"/></g>"#,
            dom_id.attr(),
            fmt_display(x),
            fmt_display(y),
            fmt_display(cluster.width.max(1.0)),
            fmt_display(cluster.height.max(1.0))
        );
        out.checkpoint()?;
    }
    out.push_str("</g>");
    drop(_g_clusters);

    // edge paths
    let _g_edge_paths = detail_guard(timing, &mut details.edge_paths);
    out.push_str(r#"<g class="edgePaths">"#);
    if ctx.include_edges {
        for (edge_index, edge) in ctx.edges.iter().enumerate() {
            if state_is_hidden(ctx, edge.start.as_str())
                || state_is_hidden(ctx, edge.end.as_str())
                || state_is_hidden(ctx, edge.id.as_str())
            {
                continue;
            }
            if state_edge_context(ctx, edge) != root {
                continue;
            }
            if state_is_shadowed_self_loop_edge(ctx, edge_index, edge, root) {
                continue;
            }
            render_state_edge_path(out, ctx, edge, origin_x, origin_y);
            out.checkpoint()?;
        }
    }
    out.push_str("</g>");
    drop(_g_edge_paths);

    // edge labels
    let _g_edge_labels = detail_guard(timing, &mut details.edge_labels);
    out.push_str(r#"<g class="edgeLabels">"#);
    if ctx.include_edges {
        for (edge_index, edge) in ctx.edges.iter().enumerate() {
            if state_is_hidden(ctx, edge.start.as_str())
                || state_is_hidden(ctx, edge.end.as_str())
                || state_is_hidden(ctx, edge.id.as_str())
            {
                continue;
            }
            if state_edge_context(ctx, edge) != root {
                continue;
            }
            if state_is_shadowed_self_loop_edge(ctx, edge_index, edge, root) {
                continue;
            }
            render_state_edge_label(out, ctx, edge, origin_x, origin_y);
            out.checkpoint()?;
        }
    }
    out.push_str("</g>");
    drop(_g_edge_labels);

    // nodes (leaf nodes + nested roots)
    out.push_str(r#"<g class="nodes">"#);
    let mut nested: Vec<&str> = Vec::new();
    for &id in &ctx.node_order {
        let Some(n) = ctx.nodes_by_id.get(id).copied() else {
            continue;
        };
        if state_is_hidden(ctx, id) {
            continue;
        }
        if n.is_group
            && n.shape != "noteGroup"
            && ctx.nested_roots.contains(id)
            && state_insertion_context(ctx, id) == root
        {
            nested.push(id);
        }
    }

    if ctx.include_nodes {
        let leaf_start = timing.start();
        for &id in &ctx.node_order {
            let Some(n) = ctx.layout_nodes_by_id.get(id).copied() else {
                continue;
            };
            if state_is_hidden(ctx, id) {
                continue;
            }
            if n.is_cluster {
                continue;
            }
            if state_leaf_context(ctx, id) != root {
                continue;
            }
            render_state_node_svg(
                out,
                ctx,
                id,
                origin_x,
                origin_y,
                timing,
                details,
                effect_outsets,
            )?;
            out.checkpoint()?;
        }
        if let Some(s) = leaf_start {
            details.leaf_nodes += s.elapsed();
        }
    }

    for child_root in nested {
        let nested_start = timing.start();
        render_state_root(
            out,
            ctx,
            Some(child_root),
            (origin_x, origin_y),
            options,
            timing,
            details,
            effect_outsets,
        )?;
        out.checkpoint()?;
        if let Some(s) = nested_start {
            details.nested_roots += s.elapsed();
        }
    }

    // Mermaid adds extra edgeLabel placeholders for self-loop transitions inside `nodes`.
    if ctx.include_edges {
        let _g_placeholders = detail_guard(timing, &mut details.self_loop_placeholders);
        for (edge_index, edge) in ctx.edges.iter().enumerate() {
            if state_is_hidden(ctx, edge.start.as_str())
                || state_is_hidden(ctx, edge.end.as_str())
                || state_is_hidden(ctx, edge.id.as_str())
            {
                continue;
            }
            if edge.start != edge.end {
                continue;
            }
            if state_edge_context(ctx, edge) != root {
                continue;
            }
            if state_is_shadowed_self_loop_edge(ctx, edge_index, edge, root) {
                continue;
            }

            let start = edge.start.as_str();
            let id1 = format!("{start}---{start}---1");
            let id2 = format!("{start}---{start}---2");

            for id in [id1, id2] {
                let (cx, cy) = ctx
                    .layout_nodes_by_id
                    .get(id.as_str())
                    .map(|n| {
                        let x = (n.x - n.width / 2.0) - origin_x;
                        let y = (n.y - n.height / 2.0) - origin_y;
                        (x, y)
                    })
                    .unwrap_or((0.0, 0.0));
                if ctx.html_labels {
                    let _ = write!(
                        out,
                        r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#,
                        escape_xml_display(&id),
                        fmt_display(cx),
                        fmt_display(cy),
                    );
                } else {
                    let _ = write!(
                        out,
                        r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/></g></g>"#,
                        escape_xml_display(&id),
                        fmt_display(cx),
                        fmt_display(cy),
                    );
                }
                out.checkpoint()?;
            }
        }
        drop(_g_placeholders);
    }

    out.push_str("</g>");
    out.push_str("</g>");
    out.checkpoint()
}

fn state_hand_drawn_rect_paths(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    stroke_width: f64,
    stroke_dasharray: &str,
    randomness: &roughr::core::RoughRandomness,
) -> (String, String) {
    let path = format!(
        "M{x},{y} L{},{} L{},{} L{},{} Z",
        x + width,
        y,
        x + width,
        y + height,
        x,
        y + height,
    );
    roughjs_paths_for_svg_path(
        &path,
        "#ECECFF",
        "#9370DB",
        stroke_width as f32,
        stroke_dasharray,
        randomness,
    )
    .unwrap_or_else(|| (path.clone(), path))
}

fn state_hand_drawn_terminal_style(property: &str, compatibility: &str, direct: &str) -> String {
    format!("{property}:{compatibility};{direct}")
}

fn render_state_cluster(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    cluster_id: &str,
    origin_x: f64,
    origin_y: f64,
) {
    let Some(cluster) = ctx.layout_clusters_by_id.get(cluster_id).copied() else {
        return;
    };
    let terminal_start = out.len();

    let data_look = state_data_look(ctx);
    let effective_look = state_effective_look(ctx);

    let shape = ctx
        .nodes_by_id
        .get(cluster_id)
        .copied()
        .map(|n| n.shape.as_str())
        .unwrap_or("");

    let class = ctx
        .nodes_by_id
        .get(cluster_id)
        .copied()
        .map(|n| n.css_classes.trim())
        .filter(|c| !c.is_empty())
        .unwrap_or("statediagram-state statediagram-cluster");

    let left = cluster.x - cluster.width / 2.0;
    let top = cluster.y - cluster.height / 2.0;
    let x = left - origin_x;
    let y = top - origin_y;
    let dom_id = state_node_scoped_dom_id(ctx, cluster_id);
    let cluster_node = ctx.nodes_by_id.get(cluster_id).copied();
    let node_style = ctx.style_plan.node(cluster_id);
    let body_style = node_style
        .map(crate::state::StateNodeStylePlan::shape_style_attr)
        .unwrap_or_default();
    let header_style = node_style
        .map(crate::state::StateNodeStylePlan::composite_header_style_attr)
        .unwrap_or_default();
    let body_fill_path_style = node_style
        .map(crate::state::StateNodeStylePlan::fill_path_style_attr)
        .unwrap_or_default();
    let body_stroke_path_style = node_style
        .map(crate::state::StateNodeStylePlan::stroke_path_style_attr)
        .unwrap_or_default();
    let header_fill_path_style = node_style
        .map(crate::state::StateNodeStylePlan::composite_header_fill_path_style_attr)
        .unwrap_or_default();
    let header_stroke_path_style = node_style
        .map(crate::state::StateNodeStylePlan::composite_header_stroke_path_style_attr)
        .unwrap_or_default();
    let label_style = node_style
        .map(crate::state::StateNodeStylePlan::label_style_attr)
        .unwrap_or_default();
    let label_div_prefix = node_style
        .map(crate::state::StateNodeStylePlan::div_style_prefix)
        .unwrap_or_default();
    let title_style = node_style
        .map(|style| {
            let header_text_style = style.composite_header_text_style_attr();
            if header_text_style.is_empty() {
                style.label_style_attr()
            } else {
                header_text_style
            }
        })
        .unwrap_or(label_style);
    let title_div_prefix = node_style
        .map(|style| {
            let header_text_prefix = style.composite_header_text_div_style_prefix();
            if header_text_prefix.is_empty() {
                style.div_style_prefix()
            } else {
                header_text_prefix
            }
        })
        .unwrap_or(label_div_prefix);
    let radius_attrs = node_style
        .and_then(crate::state::StateNodeStylePlan::radius_override)
        .map(|radius| {
            format!(
                r#" rx="{}" ry="{}""#,
                fmt(radius.max(0.0)),
                fmt(radius.max(0.0))
            )
        })
        .unwrap_or_default();

    if shape == "divider" {
        if effective_look == "handDrawn" {
            let compatibility = ctx.style_plan.compatibility();
            let surface = cluster_node
                .and_then(|node| {
                    compatibility
                        .node_shape_surface(node, crate::diagram_theme::ThemeTarget::Composite)
                })
                .expect("State divider cluster must have a compatibility surface");
            let fill = compatibility.terminal_paint_css(
                surface,
                crate::state::StateTerminalPaintProperty::Fill,
                ctx.diagram_id,
            );
            let stroke = compatibility.terminal_paint_css(
                surface,
                crate::state::StateTerminalPaintProperty::Stroke,
                ctx.diagram_id,
            );
            let stroke_width = compatibility.terminal_stroke_width_value(surface);
            let (fill_d, stroke_d) = state_hand_drawn_rect_paths(
                x,
                y,
                cluster.width.max(1.0),
                cluster.height.max(1.0),
                stroke_width,
                "5",
                &ctx.hand_drawn_seed,
            );
            let fill_style = state_hand_drawn_terminal_style("fill", &fill, body_fill_path_style);
            let stroke_style =
                state_hand_drawn_terminal_style("stroke", &stroke, body_stroke_path_style);
            let _ = write!(
                out,
                r#"<g class="{}" id="{}" data-look="{}"><g class="divider"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="5" style="{}"/></g></g>"#,
                escape_attr(class),
                dom_id.attr(),
                escape_attr(data_look),
                escape_attr(&fill_d),
                escape_attr(&fill),
                escape_attr(&fill_style),
                escape_attr(&stroke_d),
                escape_attr(&stroke),
                fmt(stroke_width),
                escape_attr(&stroke_style),
            );
            ctx.style_plan.record_node_terminal_emission(
                &mut ctx.theme_receipt.borrow_mut(),
                cluster_id,
                terminal_start..out.len(),
            );
            return;
        }
        let _ = write!(
            out,
            r#"<g class="{}" id="{}" data-look="{}"><g><rect class="divider" style="{}"{} x="{}" y="{}" width="{}" height="{}" data-look="{}"/></g></g>"#,
            escape_attr(class),
            dom_id.attr(),
            escape_attr(data_look),
            escape_attr(body_style),
            radius_attrs,
            fmt(x),
            fmt(y),
            fmt(cluster.width.max(1.0)),
            fmt(cluster.height.max(1.0)),
            escape_attr(data_look),
        );
        ctx.style_plan.record_node_terminal_emission(
            &mut ctx.theme_receipt.borrow_mut(),
            cluster_id,
            terminal_start..out.len(),
        );
        return;
    }

    let title = ctx
        .nodes_by_id
        .get(cluster_id)
        .copied()
        .map(state_node_label_text)
        .unwrap_or_else(|| cluster_id.to_string());
    let prepared_title = ctx.label_sidecar.cluster_title(cluster_id);
    let measured_title = ctx
        .label_sidecar
        .measured_native_geometry(crate::state::StateLabelOwner::ClusterTitle(cluster_id));
    let title_height = cluster.title_label.height.max(0.0);
    let inner_y = y + title_height + 2.0;
    let inner_height = (cluster.height - title_height - 6.0).max(1.0);
    let (outer_shape, inner_shape) = if effective_look == "handDrawn" {
        let compatibility = ctx.style_plan.compatibility();
        let body_surface = cluster_node
            .and_then(|node| {
                compatibility.node_shape_surface(node, crate::diagram_theme::ThemeTarget::Composite)
            })
            .expect("State composite body must have a compatibility surface");
        let header_surface = compatibility.composite_header_surface();
        let body_fill = compatibility.terminal_paint_css(
            body_surface,
            crate::state::StateTerminalPaintProperty::Fill,
            ctx.diagram_id,
        );
        let body_stroke = compatibility.terminal_paint_css(
            body_surface,
            crate::state::StateTerminalPaintProperty::Stroke,
            ctx.diagram_id,
        );
        let body_stroke_width = compatibility.terminal_stroke_width_value(body_surface);
        let header_fill = compatibility.terminal_paint_css(
            header_surface,
            crate::state::StateTerminalPaintProperty::Fill,
            ctx.diagram_id,
        );
        let header_stroke = compatibility.terminal_paint_css(
            header_surface,
            crate::state::StateTerminalPaintProperty::Stroke,
            ctx.diagram_id,
        );
        let header_stroke_width = compatibility.terminal_stroke_width_value(header_surface);
        let (outer_fill_d, outer_stroke_d) = state_hand_drawn_rect_paths(
            x,
            y,
            cluster.width.max(1.0),
            cluster.height.max(1.0),
            header_stroke_width,
            "0 0",
            &ctx.hand_drawn_seed,
        );
        let (inner_fill_d, inner_stroke_d) = state_hand_drawn_rect_paths(
            x,
            inner_y,
            cluster.width.max(1.0),
            inner_height,
            body_stroke_width,
            "0 0",
            &ctx.hand_drawn_seed,
        );
        let header_fill_style =
            state_hand_drawn_terminal_style("fill", &header_fill, header_fill_path_style);
        let header_stroke_style =
            state_hand_drawn_terminal_style("stroke", &header_stroke, header_stroke_path_style);
        let body_fill_style =
            state_hand_drawn_terminal_style("fill", &body_fill, body_fill_path_style);
        let body_stroke_style =
            state_hand_drawn_terminal_style("stroke", &body_stroke, body_stroke_path_style);
        (
            format!(
                r#"<g class="outer"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g>"#,
                escape_attr(&outer_fill_d),
                escape_attr(&header_fill),
                escape_attr(&header_fill_style),
                escape_attr(&outer_stroke_d),
                escape_attr(&header_stroke),
                fmt(header_stroke_width),
                escape_attr(&header_stroke_style),
            ),
            format!(
                r#"<g class="inner"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g>"#,
                escape_attr(&inner_fill_d),
                escape_attr(&body_fill),
                escape_attr(&body_fill_style),
                escape_attr(&inner_stroke_d),
                escape_attr(&body_stroke),
                fmt(body_stroke_width),
                escape_attr(&body_stroke_style),
            ),
        )
    } else {
        (
            format!(
                r#"<rect class="outer" style="{}"{} x="{}" y="{}" width="{}" height="{}" data-look="{}"/>"#,
                escape_attr(header_style),
                radius_attrs,
                fmt(x),
                fmt(y),
                fmt(cluster.width.max(1.0)),
                fmt(cluster.height.max(1.0)),
                escape_attr(data_look),
            ),
            format!(
                r#"<rect class="inner" style="{}"{} x="{}" y="{}" width="{}" height="{}"/>"#,
                escape_attr(body_style),
                radius_attrs,
                fmt(x),
                fmt(inner_y),
                fmt(cluster.width.max(1.0)),
                fmt(inner_height),
            ),
        )
    };

    if ctx.html_labels {
        let prepared_token_attr = state_prepared_html_label_token_attr(prepared_title);
        let label_div_style = format!(
            "{}display: table-cell; white-space: nowrap; line-height: 1.5;",
            title_div_prefix
        );
        let _ = write!(
            out,
            r#"<g class="{}" id="{}" data-id="{}" data-look="{}"><g>{}</g><g class="cluster-label" style="{}" transform="translate({}, {})"><foreignObject{} width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"><p>{}</p></span></div></foreignObject></g>{}</g>"#,
            escape_attr(class),
            dom_id.attr(),
            escape_attr(cluster_id),
            escape_attr(data_look),
            outer_shape,
            escape_attr(title_style),
            fmt(x + (cluster.width.max(1.0) - cluster.title_label.width.max(0.0)) / 2.0),
            fmt(y + 1.0),
            prepared_token_attr,
            fmt(cluster.title_label.width.max(0.0)),
            fmt(title_height),
            escape_attr(&label_div_style),
            prepared_title.map_or_else(|| escape_xml(&title), state_prepared_html_lines,),
            inner_shape,
        );
    } else {
        let title_dom = state_native_svg_text_label(
            &title,
            prepared_title,
            measured_title,
            false,
            (!title_style.is_empty()).then_some(title_style),
        );
        let _ = write!(
            out,
            r#"<g class="{}" id="{}" data-id="{}" data-look="{}"><g>{}</g><g class="cluster-label" style="{}" transform="translate({}, {})">{}</g>{}</g>"#,
            escape_attr(class),
            dom_id.attr(),
            escape_attr(cluster_id),
            escape_attr(data_look),
            outer_shape,
            escape_attr(title_style),
            fmt(x + (cluster.width.max(1.0) - cluster.title_label.width.max(0.0)) / 2.0),
            fmt(y - 2.0),
            title_dom,
            inner_shape,
        );
    }
    ctx.style_plan.record_node_terminal_emission(
        &mut ctx.theme_receipt.borrow_mut(),
        cluster_id,
        terminal_start..out.len(),
    );
}
