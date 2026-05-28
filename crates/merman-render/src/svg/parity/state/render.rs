use super::*;
use crate::generated::state_text_overrides_11_12_2 as state_text_overrides;

pub(super) fn render_state_diagram_v2_svg_impl(
    layout: &StateDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: StateSvgModel = crate::json::from_value_ref(semantic)?;
    render_state_diagram_v2_svg_model_impl(
        layout,
        &model,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_state_diagram_v2_svg_model_impl(
    layout: &StateDiagramV2Layout,
    model: &StateSvgModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let timing_enabled = super::timing::render_timing_enabled();
    let mut timings = super::timing::RenderTimings::default();
    let total_start = std::time::Instant::now();
    fn section<'a>(
        enabled: bool,
        dst: &'a mut std::time::Duration,
    ) -> Option<super::timing::TimingGuard<'a>> {
        enabled.then(|| super::timing::TimingGuard::new(dst))
    }

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");

    let _g_build_ctx = section(timing_enabled, &mut timings.build_ctx);

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
    let title_top_margin = config_f64(effective_config, &["state", "titleTopMargin"])
        .unwrap_or(25.0)
        .max(0.0);

    let has_acc_title = model
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = model
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());

    let text_style = crate::state::state_text_style(effective_config);

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
        diagram_id: diagram_id.to_string(),
        diagram_look: config_string(effective_config, &["look"])
            .unwrap_or_else(|| "classic".to_string()),
        hand_drawn_seed: effective_config
            .get("handDrawnSeed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        html_label_wrapping_width: crate::state::state_html_label_wrapping_width(effective_config),
        state_padding: config_f64(effective_config, &["state", "padding"])
            .unwrap_or(8.0)
            .max(0.0),
        node_order,
        nodes_by_id,
        layout_nodes_by_id,
        layout_edges_by_id,
        layout_clusters_by_id,
        parent,
        nested_roots: std::collections::BTreeSet::new(),
        hidden_prefixes,
        security_level_loose: config_string(effective_config, &["securityLevel"]).as_deref()
            == Some("loose"),
        links: &model.links,
        states: &model.states,
        edges: &model.edges,
        include_edges: options.include_edges,
        include_nodes: options.include_nodes,
        measurer,
        text_style,
        rough_circle_cache: std::cell::RefCell::new(FxHashMap::default()),
        rough_paths_cache: std::cell::RefCell::new(FxHashMap::default()),
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

    let fast_viewport = matches!(
        std::env::var("MERMAN_STATE_VIEWPORT").as_deref(),
        Ok("layout") | Ok("fast") | Ok("1") | Ok("true")
    );
    if fast_viewport {
        // In fast mode we can compute the root viewport purely from layout geometry, so we do not
        // need placeholder replacement.
        let css = state_css(diagram_id, model, effective_config);

        let viewbox_svg_scan = std::time::Duration::ZERO;
        let _g_viewbox = section(timing_enabled, &mut timings.viewbox);
        let mut content_bounds = state_viewport_bounds_from_layout(layout).unwrap_or(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        });

        let mut title_svg = String::new();
        if let Some(title) = diagram_title.as_deref() {
            // Mermaid centers the title using the pre-title content bbox:
            // `x = bbox.x + bbox.width/2`, `y = -titleTopMargin`.
            let title_x = (content_bounds.min_x + content_bounds.max_x) / 2.0;
            let title_y = -title_top_margin;

            let mut title_style = crate::state::state_text_style(effective_config);
            title_style.font_size = 18.0;
            let (title_left, title_right) =
                crate::generated::state_text_overrides_11_12_2::lookup_state_diagram_title_bbox_x_px(
                    title_style.font_size,
                    title,
                )
                .unwrap_or_else(|| measurer.measure_svg_title_bbox_x(title, &title_style));

            let (ascent, descent) = crate::text::svg_title_bbox_vertical_extents_px(&title_style);

            content_bounds.min_x = content_bounds.min_x.min(title_x - title_left);
            content_bounds.max_x = content_bounds.max_x.max(title_x + title_right);
            content_bounds.min_y = content_bounds.min_y.min(title_y - ascent);
            content_bounds.max_y = content_bounds.max_y.max(title_y + descent);

            title_svg = String::with_capacity(title.len() + 128);
            let _ = write!(
                &mut title_svg,
                r#"<text text-anchor="middle" x="{}" y="{}" class="statediagramTitleText">{}</text>"#,
                fmt(title_x),
                fmt(title_y),
                escape_xml_display(title)
            );
        }

        let vb_min_x = content_bounds.min_x - viewport_padding;
        let vb_min_y = content_bounds.min_y - viewport_padding;
        let vb_w =
            ((content_bounds.max_x - content_bounds.min_x) + 2.0 * viewport_padding).max(1.0);
        let vb_h =
            ((content_bounds.max_y - content_bounds.min_y) + 2.0 * viewport_padding).max(1.0);
        // Mermaid's root viewBox widths/heights often land on a single-precision lattice.
        let vb_w = (vb_w as f32) as f64;
        let vb_h = (vb_h as f32) as f64;

        let mut max_w_attr = String::new();
        super::util::fmt_max_width_px_into(&mut max_w_attr, vb_w.max(1.0));
        let mut view_box_attr = String::with_capacity(64);
        let _ = write!(
            &mut view_box_attr,
            "{} {} {} {}",
            fmt(vb_min_x),
            fmt(vb_min_y),
            fmt(vb_w),
            fmt(vb_h)
        );
        let mut width_attr = fmt_string(vb_w);
        let mut height_attr = fmt_string(vb_h);
        apply_root_viewport_override(
            diagram_id,
            &mut view_box_attr,
            &mut width_attr,
            &mut height_attr,
            &mut max_w_attr,
            crate::generated::state_root_overrides_11_12_2::lookup_state_root_viewport_override,
        );

        drop(_g_viewbox);

        let _g_render_svg = section(timing_enabled, &mut timings.render_svg);
        let estimated_svg_bytes = 2048usize
            + css.len()
            + title_svg.len()
            + max_w_attr.len()
            + view_box_attr.len()
            + layout.nodes.len().saturating_mul(512)
            + layout.edges.len().saturating_mul(384)
            + layout.clusters.len().saturating_mul(256);
        let mut out = String::with_capacity(estimated_svg_bytes);
        let diagram_id_esc = escape_xml_display(diagram_id);
        let aria_labelledby_attr = has_acc_title.then(|| format!("chart-title-{diagram_id_esc}"));
        let aria_describedby_attr = has_acc_descr.then(|| format!("chart-desc-{diagram_id_esc}"));
        let style_attr = format!("max-width: {max_w_attr}px; background-color: white;");
        root_svg::push_svg_root_open(
            &mut out,
            root_svg::SvgRootAttrs {
                class: Some("statediagram"),
                width: root_svg::SvgRootWidth::Percent100,
                style_attr: Some(style_attr.as_str()),
                viewbox_attr: Some(view_box_attr.as_str()),
                aria_labelledby: aria_labelledby_attr.as_deref(),
                aria_describedby: aria_describedby_attr.as_deref(),
                trailing_newline: false,
                aria_attr_order: root_svg::SvgRootAriaAttrOrder::LabelledbyThenDescribedby,
                ..root_svg::SvgRootAttrs::new(diagram_id, "stateDiagram")
            },
        );

        if has_acc_title {
            let _ = write!(
                &mut out,
                r#"<title id="chart-title-{}">{}"#,
                escape_xml_display(diagram_id),
                escape_xml_display(model.acc_title.as_deref().unwrap_or_default())
            );
            out.push_str("</title>");
        }
        if has_acc_descr {
            let _ = write!(
                &mut out,
                r#"<desc id="chart-desc-{}">{}"#,
                escape_xml_display(diagram_id),
                escape_xml_display(model.acc_descr.as_deref().unwrap_or_default())
            );
            out.push_str("</desc>");
        }

        let _ = write!(&mut out, "<style>{}</style>", css);

        // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
        out.push_str("<g>");
        state_markers(&mut out, diagram_id);

        let mut detail = StateRenderDetails::default();
        render_state_root(
            &mut out,
            &ctx,
            None,
            origin_x,
            origin_y,
            timing_enabled,
            &mut detail,
        );

        out.push_str("</g>");
        out.push_str(&title_svg);
        out.push_str("</svg>\n");
        drop(_g_render_svg);

        timings.total = total_start.elapsed();
        if timing_enabled {
            eprintln!(
                "[render-timing] diagram=stateDiagram total={:?} deserialize={:?} build_ctx={:?} render_svg={:?} viewbox={:?} viewbox_svg_scan={:?} finalize={:?} fast_viewport={} root_calls={} clusters={:?} edge_paths={:?} edge_labels={:?} leaf_nodes={:?} leaf_style_parse={:?} leaf_roughjs={:?} leaf_roughjs_calls={} leaf_roughjs_unique={} leaf_measure={:?} leaf_label_html={:?} leaf_emit={:?} nested_roots={:?} self_loop_placeholders={:?}",
                timings.total,
                timings.deserialize_model,
                timings.build_ctx,
                timings.render_svg,
                timings.viewbox,
                viewbox_svg_scan,
                timings.finalize_svg,
                fast_viewport,
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
        return Ok(out);
    }

    let _g_render_svg = section(timing_enabled, &mut timings.render_svg);

    // Mermaid derives the final root viewport via `svg.getBBox()` (after rendering). We don't
    // have a browser DOM, so approximate that by parsing the SVG we just emitted and unioning
    // bboxes for the SVG elements we generate (`rect`/`path`/`circle`/`foreignObject`, etc).
    const VIEWBOX_PLACEHOLDER: &str = "__MERMAID_VIEWBOX__";
    const MAX_WIDTH_PLACEHOLDER: &str = "__MERMAID_MAX_WIDTH__";
    const TITLE_PLACEHOLDER_COMMENT: &str = "<!--__MERMAID_TITLE__-->";

    // Mermaid emits a single `<style>` element with diagram-scoped CSS.
    let css = state_css(diagram_id, model, effective_config);

    let estimated_svg_bytes = 2048usize
        + css.len()
        + layout.nodes.len().saturating_mul(512)
        + layout.edges.len().saturating_mul(384)
        + layout.clusters.len().saturating_mul(256);
    let mut out = String::with_capacity(estimated_svg_bytes);
    let diagram_id_esc = escape_xml_display(diagram_id);
    let aria_labelledby_attr = has_acc_title.then(|| format!("chart-title-{diagram_id_esc}"));
    let aria_describedby_attr = has_acc_descr.then(|| format!("chart-desc-{diagram_id_esc}"));
    let style_attr = format!("max-width: {MAX_WIDTH_PLACEHOLDER}px; background-color: white;");
    root_svg::push_svg_root_open(
        &mut out,
        root_svg::SvgRootAttrs {
            class: Some("statediagram"),
            width: root_svg::SvgRootWidth::Percent100,
            style_attr: Some(style_attr.as_str()),
            viewbox_attr: Some(VIEWBOX_PLACEHOLDER),
            aria_labelledby: aria_labelledby_attr.as_deref(),
            aria_describedby: aria_describedby_attr.as_deref(),
            trailing_newline: false,
            aria_attr_order: root_svg::SvgRootAriaAttrOrder::LabelledbyThenDescribedby,
            ..root_svg::SvgRootAttrs::new(diagram_id, "stateDiagram")
        },
    );

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}"#,
            escape_xml_display(diagram_id),
            escape_xml_display(model.acc_title.as_deref().unwrap_or_default())
        );
        out.push_str("</title>");
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}"#,
            escape_xml_display(diagram_id),
            escape_xml_display(model.acc_descr.as_deref().unwrap_or_default())
        );
        out.push_str("</desc>");
    }

    let _ = write!(&mut out, "<style>{}</style>", css);

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");
    state_markers(&mut out, diagram_id);

    // `svg.getBBox()` does not include `<style>` and typically excludes non-rendered `<defs>`
    // content from the rendered bbox. Scan only the rendered graph payload to reduce overhead
    // in our SVG bounds approximation.
    let bounds_scan_start = out.len();
    let mut detail = StateRenderDetails::default();
    render_state_root(
        &mut out,
        &ctx,
        None,
        origin_x,
        origin_y,
        timing_enabled,
        &mut detail,
    );
    let bounds_scan_end = out.len();

    out.push_str("</g>");
    out.push_str(TITLE_PLACEHOLDER_COMMENT);
    out.push_str("</svg>\n");

    drop(_g_render_svg);

    let mut viewbox_svg_scan = std::time::Duration::ZERO;
    let _g_viewbox = section(timing_enabled, &mut timings.viewbox);
    let fast_viewport = matches!(
        std::env::var("MERMAN_STATE_VIEWPORT").as_deref(),
        Ok("layout") | Ok("fast") | Ok("1") | Ok("true")
    );
    let mut content_bounds = if fast_viewport {
        state_viewport_bounds_from_layout(layout)
    } else {
        let _g_scan = section(timing_enabled, &mut viewbox_svg_scan);
        svg_emitted_bounds_from_svg(&out[bounds_scan_start..bounds_scan_end])
            .or_else(|| state_viewport_bounds_from_layout(layout))
    }
    .unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    // Note: Chromium `getBBox()` values are not always exact `f32`-lattice outputs. Some Mermaid
    // state diagram fixtures show sub-ulp deltas in `x/y` that survive into the serialized root
    // `viewBox`. Avoid forcing `f32` quantization here; we keep `max-width` stable via the
    // Mermaid-like significant-digit formatter (`fmt_max_width_px`).

    let mut title_svg = String::new();
    if let Some(title) = diagram_title.as_deref() {
        // Mermaid centers the title using the pre-title content bbox:
        // `x = bbox.x + bbox.width/2`, `y = -titleTopMargin`.
        let title_x = (content_bounds.min_x + content_bounds.max_x) / 2.0;
        let title_y = -title_top_margin;

        let mut title_style = crate::state::state_text_style(effective_config);
        title_style.font_size = 18.0;
        let (title_left, title_right) =
            crate::generated::state_text_overrides_11_12_2::lookup_state_diagram_title_bbox_x_px(
                title_style.font_size,
                title,
            )
            .unwrap_or_else(|| measurer.measure_svg_title_bbox_x(title, &title_style));

        let (ascent, descent) = crate::text::svg_title_bbox_vertical_extents_px(&title_style);

        content_bounds.min_x = content_bounds.min_x.min(title_x - title_left);
        content_bounds.max_x = content_bounds.max_x.max(title_x + title_right);
        content_bounds.min_y = content_bounds.min_y.min(title_y - ascent);
        content_bounds.max_y = content_bounds.max_y.max(title_y + descent);

        title_svg = String::with_capacity(title.len() + 128);
        let _ = write!(
            &mut title_svg,
            r#"<text text-anchor="middle" x="{}" y="{}" class="statediagramTitleText">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            escape_xml_display(title)
        );
    }

    let vb_min_x = content_bounds.min_x - viewport_padding;
    let vb_min_y = content_bounds.min_y - viewport_padding;
    let vb_w = ((content_bounds.max_x - content_bounds.min_x) + 2.0 * viewport_padding).max(1.0);
    let vb_h = ((content_bounds.max_y - content_bounds.min_y) + 2.0 * viewport_padding).max(1.0);
    // Mermaid's root viewBox widths/heights often land on a single-precision lattice.
    let vb_w = (vb_w as f32) as f64;
    let vb_h = (vb_h as f32) as f64;

    let mut max_w_attr = String::new();
    super::util::fmt_max_width_px_into(&mut max_w_attr, vb_w.max(1.0));
    let mut view_box_attr = String::with_capacity(64);
    let _ = write!(
        &mut view_box_attr,
        "{} {} {} {}",
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w),
        fmt(vb_h)
    );
    let mut width_attr = fmt_string(vb_w);
    let mut height_attr = fmt_string(vb_h);
    apply_root_viewport_override(
        diagram_id,
        &mut view_box_attr,
        &mut width_attr,
        &mut height_attr,
        &mut max_w_attr,
        crate::generated::state_root_overrides_11_12_2::lookup_state_root_viewport_override,
    );

    drop(_g_viewbox);
    let _g_finalize = section(timing_enabled, &mut timings.finalize_svg);

    out = super::util::replace_placeholders_once(
        &out,
        &[
            (MAX_WIDTH_PLACEHOLDER, max_w_attr.as_str()),
            (VIEWBOX_PLACEHOLDER, view_box_attr.as_str()),
            (TITLE_PLACEHOLDER_COMMENT, title_svg.as_str()),
        ],
    );

    drop(_g_finalize);
    timings.total = total_start.elapsed();
    if timing_enabled {
        eprintln!(
            "[render-timing] diagram=stateDiagram total={:?} deserialize={:?} build_ctx={:?} render_svg={:?} viewbox={:?} viewbox_svg_scan={:?} finalize={:?} fast_viewport={} root_calls={} clusters={:?} edge_paths={:?} edge_labels={:?} leaf_nodes={:?} leaf_style_parse={:?} leaf_roughjs={:?} leaf_roughjs_calls={} leaf_roughjs_unique={} leaf_measure={:?} leaf_label_html={:?} leaf_emit={:?} nested_roots={:?} self_loop_placeholders={:?}",
            timings.total,
            timings.deserialize_model,
            timings.build_ctx,
            timings.render_svg,
            timings.viewbox,
            viewbox_svg_scan,
            timings.finalize_svg,
            fast_viewport,
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
    Ok(out)
}

fn render_state_root(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    root: Option<&str>,
    parent_origin_x: f64,
    parent_origin_y: f64,
    timing_enabled: bool,
    details: &mut StateRenderDetails,
) {
    details.root_calls += 1;

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
    let _g_clusters = detail_guard(timing_enabled, &mut details.clusters);
    out.push_str(r#"<g class="clusters">"#);
    if let Some(root_id) = root {
        render_state_cluster(out, ctx, root_id, origin_x, origin_y);
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
        let note_owner = cluster_id.strip_suffix("----parent").unwrap_or(cluster_id);
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
        let _ = write!(
            out,
            r#"<g id="{}" class="note-cluster"><rect x="{}" y="{}" width="{}" height="{}" fill="none"/></g>"#,
            escape_xml_display(cluster_id),
            fmt_display(x),
            fmt_display(y),
            fmt_display(cluster.width.max(1.0)),
            fmt_display(cluster.height.max(1.0))
        );
    }
    out.push_str("</g>");
    drop(_g_clusters);

    // edge paths
    let _g_edge_paths = detail_guard(timing_enabled, &mut details.edge_paths);
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
        }
    }
    out.push_str("</g>");
    drop(_g_edge_paths);

    // edge labels
    let _g_edge_labels = detail_guard(timing_enabled, &mut details.edge_labels);
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
        let leaf_start = timing_enabled.then(std::time::Instant::now);
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
            render_state_node_svg(out, ctx, id, origin_x, origin_y, timing_enabled, details);
        }
        if let Some(s) = leaf_start {
            details.leaf_nodes += s.elapsed();
        }
    }

    for child_root in nested {
        let nested_start = timing_enabled.then(std::time::Instant::now);
        render_state_root(
            out,
            ctx,
            Some(child_root),
            origin_x,
            origin_y,
            timing_enabled,
            details,
        );
        if let Some(s) = nested_start {
            details.nested_roots += s.elapsed();
        }
    }

    // Mermaid adds extra edgeLabel placeholders for self-loop transitions inside `nodes`.
    if ctx.include_edges {
        let _g_placeholders = detail_guard(timing_enabled, &mut details.self_loop_placeholders);
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
                        let mut y = (n.y - n.height / 2.0) - origin_y;
                        // Mermaid's self-loop helper nodes are rendered as tiny `labelRect`
                        // placeholders (`0.1x0.1`). In upstream browser snapshots, their
                        // effective SVG bbox y-origin lands 0.05px lower than the geometric
                        // top-left computed from Dagre center/size.
                        if n.width <= 0.1 + 1e-9 && n.height <= 0.1 + 1e-9 {
                            y += 0.05;
                        }
                        (x, y)
                    })
                    .unwrap_or((0.0, 0.0));
                let _ = write!(
                    out,
                    r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#,
                    escape_xml_display(&id),
                    fmt_display(cx),
                    fmt_display(cy),
                );
            }
        }
        drop(_g_placeholders);
    }

    out.push_str("</g>");
    out.push_str("</g>");
}

fn render_state_cluster(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    cluster_id: &str,
    origin_x: f64,
    origin_y: f64,
) {
    let Some(cluster) = ctx.layout_clusters_by_id.get(cluster_id).copied() else {
        return;
    };

    let data_look = ctx.diagram_look.trim();
    let data_look = if data_look.is_empty() {
        "classic"
    } else {
        data_look
    };

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

    if shape == "divider" {
        let _ = write!(
            out,
            r#"<g class="{}" id="{}" data-look="{}"><g><rect class="divider" x="{}" y="{}" width="{}" height="{}" data-look="{}"/></g></g>"#,
            escape_attr(class),
            escape_attr(cluster_id),
            escape_attr(data_look),
            fmt(x),
            fmt(y),
            fmt(cluster.width.max(1.0)),
            fmt(cluster.height.max(1.0)),
            escape_attr(data_look),
        );
        return;
    }

    let title = ctx
        .nodes_by_id
        .get(cluster_id)
        .copied()
        .map(state_node_label_text)
        .unwrap_or_else(|| cluster_id.to_string());

    let mut link_open = String::new();
    let mut link_close = String::new();
    if let Some(links) = ctx.links.get(cluster_id) {
        let mut push_link = |link: &StateSvgLink| {
            let url = link.url.trim();
            let tooltip = link.tooltip.trim();
            let title_attr = if tooltip.is_empty() {
                String::new()
            } else {
                format!(r#" title="{}""#, escape_attr(tooltip))
            };

            if !url.is_empty() && (ctx.security_level_loose || state_link_href_allowed(url)) {
                link_open.push_str(&format!(
                    r#"<a xlink:href="{}"{}>"#,
                    escape_attr(url),
                    title_attr
                ));
                link_close.push_str("</a>");
                return;
            }

            link_open.push_str(&format!(r#"<a{}>"#, title_attr));
            link_close.push_str("</a>");
        };

        match links {
            StateSvgLinks::One(link) => push_link(link),
            StateSvgLinks::Many(list) => {
                for link in list {
                    push_link(link);
                }
            }
        }
    }

    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-id="{}" data-look="{}"><g><rect class="outer" x="{}" y="{}" width="{}" height="{}" data-look="{}"/></g>{}<g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="19"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: {}px; white-space: nowrap;"><span class="nodeLabel">{}</span></div></foreignObject></g>{}<rect class="inner" x="{}" y="{}" width="{}" height="{}"/></g>"#,
        escape_attr(class),
        escape_attr(cluster_id),
        escape_attr(cluster_id),
        escape_attr(data_look),
        fmt(x),
        fmt(y),
        fmt(cluster.width.max(1.0)),
        fmt(cluster.height.max(1.0)),
        escape_attr(data_look),
        link_open,
        fmt(x + (cluster.width.max(1.0) - cluster.title_label.width.max(0.0)) / 2.0),
        fmt(y + 1.0),
        fmt(cluster.title_label.width.max(0.0)),
        fmt_display(state_text_overrides::state_html_inline_span_padding_right_px()),
        escape_xml(&title),
        link_close,
        fmt(x),
        fmt(y + 21.0),
        fmt(cluster.width.max(1.0)),
        fmt((cluster.height - 29.0).max(1.0))
    );
}

fn render_state_node_svg(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
    timing_enabled: bool,
    details: &mut StateRenderDetails,
) {
    let Some(node) = ctx.nodes_by_id.get(node_id).copied() else {
        return;
    };
    let Some(ln) = ctx.layout_nodes_by_id.get(node_id).copied() else {
        return;
    };
    if ln.is_cluster {
        return;
    }
    let cx = ln.x - origin_x;
    let cy = ln.y - origin_y;
    let w = ln.width.max(1.0);
    let h = ln.height.max(1.0);

    #[inline]
    fn cached_circle(
        ctx: &StateRenderCtx<'_>,
        key: StateRoughCacheKey,
        build: impl FnOnce() -> String,
    ) -> Arc<String> {
        let existing = { ctx.rough_circle_cache.borrow().get(&key).cloned() };
        if let Some(v) = existing {
            return v;
        }

        if let Some(v) = state_tls_get_circle(key) {
            ctx.rough_circle_cache
                .borrow_mut()
                .insert(key, Arc::clone(&v));
            return v;
        }

        if let Ok(global) = state_global_rough_circle_cache().lock() {
            if let Some(v) = global.get(&key) {
                let v = Arc::clone(v);
                state_tls_put_circle(key, Arc::clone(&v));
                ctx.rough_circle_cache
                    .borrow_mut()
                    .insert(key, Arc::clone(&v));
                return v;
            }
        }

        let built = Arc::new(build());
        let cached = if let Ok(mut global) = state_global_rough_circle_cache().lock() {
            Arc::clone(global.entry(key).or_insert_with(|| Arc::clone(&built)))
        } else {
            Arc::clone(&built)
        };
        state_tls_put_circle(key, Arc::clone(&cached));
        ctx.rough_circle_cache
            .borrow_mut()
            .insert(key, Arc::clone(&cached));
        cached
    }

    #[inline]
    fn cached_paths(
        ctx: &StateRenderCtx<'_>,
        key: StateRoughCacheKey,
        build: impl FnOnce() -> (String, String),
    ) -> (Arc<String>, Arc<String>) {
        let existing = { ctx.rough_paths_cache.borrow().get(&key).cloned() };
        if let Some(v) = existing {
            return v;
        }

        if let Some(v) = state_tls_get_paths(key) {
            ctx.rough_paths_cache
                .borrow_mut()
                .insert(key, (Arc::clone(&v.0), Arc::clone(&v.1)));
            return v;
        }

        if let Ok(global) = state_global_rough_paths_cache().lock() {
            if let Some((fill_d, stroke_d)) = global.get(&key) {
                let v = (Arc::clone(fill_d), Arc::clone(stroke_d));
                state_tls_put_paths(key, (Arc::clone(&v.0), Arc::clone(&v.1)));
                ctx.rough_paths_cache
                    .borrow_mut()
                    .insert(key, (Arc::clone(&v.0), Arc::clone(&v.1)));
                return v;
            }
        }

        let (fill_d, stroke_d) = build();
        let built = (Arc::new(fill_d), Arc::new(stroke_d));
        let cached = if let Ok(mut global) = state_global_rough_paths_cache().lock() {
            let entry = global
                .entry(key)
                .or_insert_with(|| (Arc::clone(&built.0), Arc::clone(&built.1)));
            (Arc::clone(&entry.0), Arc::clone(&entry.1))
        } else {
            (Arc::clone(&built.0), Arc::clone(&built.1))
        };
        state_tls_put_paths(key, (Arc::clone(&cached.0), Arc::clone(&cached.1)));
        ctx.rough_paths_cache
            .borrow_mut()
            .insert(key, (Arc::clone(&cached.0), Arc::clone(&cached.1)));
        cached
    }

    let node_class = if node.css_classes.trim().is_empty() {
        "node".to_string()
    } else {
        format!("node {}", node.css_classes.trim())
    };

    let style_parse_start = timing_enabled.then(std::time::Instant::now);
    let mut shape_decls: Vec<StateInlineDecl<'_>> = Vec::new();
    let mut text_decls: Vec<StateInlineDecl<'_>> = Vec::new();
    let mut fill_override: Option<&str> = None;
    let mut stroke_override: Option<&str> = None;
    let mut stroke_width_override: Option<f64> = None;
    for raw in node
        .css_compiled_styles
        .iter()
        .chain(node.css_styles.iter())
    {
        let Some(d) = state_parse_inline_decl(raw) else {
            continue;
        };
        if d.key.trim().eq_ignore_ascii_case("fill") {
            fill_override = Some(d.val.trim());
        }
        if d.key.trim().eq_ignore_ascii_case("stroke") {
            stroke_override = Some(d.val.trim());
        }
        if d.key.trim().eq_ignore_ascii_case("stroke-width") {
            let val = d.val.trim().trim_end_matches("px").trim();
            if let Ok(v) = val.parse::<f64>() {
                stroke_width_override = Some(v);
            }
        }
        if state_is_text_style_key(d.key) {
            text_decls.push(d);
        } else {
            shape_decls.push(d);
        }
    }
    let shape_style_attr = state_compact_style_attr(&shape_decls);
    let text_style_attr = state_compact_style_attr(&text_decls);
    let div_style_prefix = state_div_style_prefix(&text_decls);
    if let Some(s) = style_parse_start {
        details.leaf_nodes_style_parse += s.elapsed();
    }

    match node.shape.as_str() {
        "stateStart" => {
            let _g_emit = detail_guard(timing_enabled, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r#"<g class="node default" id="{}" transform="translate({}, {})"><circle class="state-start" r="7" width="14" height="14"/></g>"#,
                escape_xml_display(&node.dom_id),
                fmt_display(cx),
                fmt_display(cy)
            );
            drop(_g_emit);
        }
        "stateEnd" => {
            let rough_start = timing_enabled.then(std::time::Instant::now);
            if timing_enabled {
                details.leaf_roughjs_calls += 2;
                details.leaf_roughjs_unique.insert(StateRoughCacheKey {
                    tag: 1,
                    a: 14.0f64.to_bits(),
                    b: 0,
                    seed: ctx.hand_drawn_seed,
                });
                details.leaf_roughjs_unique.insert(StateRoughCacheKey {
                    tag: 2,
                    a: 5.0f64.to_bits(),
                    b: 0,
                    seed: ctx.hand_drawn_seed,
                });
            }
            let outer_key = StateRoughCacheKey {
                tag: 1,
                a: 14.0f64.to_bits(),
                b: 0,
                seed: ctx.hand_drawn_seed,
            };
            let inner_key = StateRoughCacheKey {
                tag: 2,
                a: 5.0f64.to_bits(),
                b: 0,
                seed: ctx.hand_drawn_seed,
            };

            let outer_d = cached_circle(ctx, outer_key, || {
                roughjs_circle_path_d(14.0, ctx.hand_drawn_seed)
                    .unwrap_or_else(|| "M0,0".to_string())
            });
            let inner_d = cached_circle(ctx, inner_key, || {
                roughjs_circle_path_d(5.0, ctx.hand_drawn_seed)
                    .unwrap_or_else(|| "M0,0".to_string())
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }
            let shape_style_escaped = escape_attr(&shape_style_attr);
            let fill_attr = fill_override.unwrap_or("#ECECFF");
            let _g_emit = detail_guard(timing_enabled, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r##"<g class="node default" id="{}" transform="translate({}, {})"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="#333333" stroke-width="2" fill="none" stroke-dasharray="0 0" style="{}"/><g><path d="{}" stroke="none" stroke-width="0" fill="#9370DB" style=""/><path d="{}" stroke="#9370DB" stroke-width="2" fill="none" stroke-dasharray="0 0" style=""/></g></g></g>"##,
                escape_attr(&node.dom_id),
                fmt(cx),
                fmt(cy),
                outer_d.as_str(),
                escape_attr(fill_attr),
                shape_style_escaped,
                outer_d.as_str(),
                shape_style_escaped,
                inner_d.as_str(),
                inner_d.as_str()
            );
            drop(_g_emit);
        }
        "fork" | "join" => {
            let rough_start = timing_enabled.then(std::time::Instant::now);
            let key = StateRoughCacheKey {
                tag: 3,
                a: w.to_bits(),
                b: h.to_bits(),
                seed: ctx.hand_drawn_seed,
            };
            if timing_enabled {
                details.leaf_roughjs_calls += 1;
                details.leaf_roughjs_unique.insert(key);
            }
            let (fill_d, stroke_d) = cached_paths(ctx, key, || {
                roughjs_paths_for_rect(StateRoughRectSpec {
                    x: -w / 2.0,
                    y: -h / 2.0,
                    w,
                    h,
                    fill: "#333333",
                    stroke: "#333333",
                    stroke_width: 1.3,
                    seed: ctx.hand_drawn_seed,
                })
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()))
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }
            let _g_emit = detail_guard(timing_enabled, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r##"<g class="{}" id="{}" transform="translate({}, {})"><g><path d="{}" stroke="none" stroke-width="0" fill="#333333" style=""/><path d="{}" stroke="#333333" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                escape_xml_display(&node_class),
                escape_xml_display(&node.dom_id),
                fmt_display(cx),
                fmt_display(cy),
                fill_d.as_str(),
                stroke_d.as_str()
            );
            drop(_g_emit);
        }
        "choice" => {
            let rough_start = timing_enabled.then(std::time::Instant::now);
            let key = StateRoughCacheKey {
                tag: 4,
                a: w.to_bits(),
                b: h.to_bits(),
                seed: ctx.hand_drawn_seed,
            };
            if timing_enabled {
                details.leaf_roughjs_calls += 1;
                details.leaf_roughjs_unique.insert(key);
            }
            let (fill_d, stroke_d) = cached_paths(ctx, key, || {
                roughjs_paths_for_svg_path(
                    &mermaid_choice_diamond_path_data(w, h),
                    "#ECECFF",
                    "#9370DB",
                    1.3,
                    "0 0",
                    ctx.hand_drawn_seed,
                )
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()))
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }

            let _g_emit = detail_guard(timing_enabled, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r##"<g class="{}" id="{}" transform="translate({}, {})"><g><path d="{}" stroke="none" stroke-width="0" fill="#ECECFF" style=""/><path d="{}" stroke="#9370DB" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                escape_xml_display(&node_class),
                escape_xml_display(&node.dom_id),
                fmt_display(cx),
                fmt_display(cy),
                fill_d.as_str(),
                stroke_d.as_str()
            );
            drop(_g_emit);
        }
        "note" => {
            let label = state_node_label_text(node);
            let measure_start = timing_enabled.then(std::time::Instant::now);
            let mut metrics = ctx.measurer.measure_wrapped(
                &label,
                &ctx.text_style,
                Some(ctx.html_label_wrapping_width),
                WrapMode::HtmlLike,
            );
            if let Some(s) = measure_start {
                details.leaf_nodes_measure += s.elapsed();
            }
            if let Some(w) = state_text_overrides::lookup_state_note_label_width_px(
                ctx.text_style.font_size,
                label.trim(),
            ) {
                metrics.width = w;
            }
            let lw = metrics.width.max(0.0);
            let lh = metrics.height.max(0.0);
            let rough_start = timing_enabled.then(std::time::Instant::now);
            let key = StateRoughCacheKey {
                tag: 5,
                a: w.to_bits(),
                b: h.to_bits(),
                seed: ctx.hand_drawn_seed,
            };
            if timing_enabled {
                details.leaf_roughjs_calls += 1;
                details.leaf_roughjs_unique.insert(key);
            }
            let (fill_d, stroke_d) = cached_paths(ctx, key, || {
                roughjs_paths_for_rect(StateRoughRectSpec {
                    x: -w / 2.0,
                    y: -h / 2.0,
                    w,
                    h,
                    fill: "#fff5ad",
                    stroke: "#aaaa33",
                    stroke_width: 1.3,
                    seed: ctx.hand_drawn_seed,
                })
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()))
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }
            let label_html_start = timing_enabled.then(std::time::Instant::now);
            let label_html = state_node_label_html(&label);
            if let Some(s) = label_html_start {
                details.leaf_nodes_label_html += s.elapsed();
            }
            let _g_emit = detail_guard(timing_enabled, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r##"<g class="{}" id="{}" transform="translate({}, {})"><g class="basic label-container"><path d="{}" stroke="none" stroke-width="0" fill="#fff5ad"/><path d="{}" stroke="#aaaa33" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/></g><g class="label" style="" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;">{}</div></foreignObject></g></g>"##,
                escape_xml_display(&node_class),
                escape_xml_display(&node.dom_id),
                fmt_display(cx),
                fmt_display(cy),
                fill_d.as_str(),
                stroke_d.as_str(),
                fmt_display(-lw / 2.0),
                fmt_display(-lh / 2.0),
                fmt_display(lw),
                fmt_display(lh),
                fmt_display(ctx.html_label_wrapping_width),
                label_html
            );
            drop(_g_emit);
        }
        "rectWithTitle" => {
            let title = node
                .label
                .as_ref()
                .map(state_value_to_label_text)
                .unwrap_or_else(|| node.id.clone());
            let desc = node
                .description
                .as_ref()
                .map(|v| v.join("\n"))
                .unwrap_or_default();
            // Mermaid renders `rectWithTitle` labels as HTML `<span>` (nowrap) with
            // `padding-right: 1px` and no explicit `line-height`, so their measured height matches
            // SVG `getBBox()` (19px at 16px font size) rather than the 1.5em HTML `<p>` height.
            let measure_start = timing_enabled.then(std::time::Instant::now);
            let title_metrics =
                ctx.measurer
                    .measure_wrapped(&title, &ctx.text_style, None, WrapMode::SvgLike);
            let desc_metrics =
                ctx.measurer
                    .measure_wrapped(&desc, &ctx.text_style, None, WrapMode::SvgLike);
            if let Some(s) = measure_start {
                details.leaf_nodes_measure += s.elapsed();
            }

            let padding = ctx.state_padding;
            let half_pad = (padding / 2.0).max(0.0);
            let top_pad = state_text_overrides::state_rect_with_title_top_pad_px(padding);
            let gap = state_text_overrides::state_rect_with_title_gap_px(padding);

            // Mirror `padding-right: 1px` in upstream HTML.
            let title_w = state_text_overrides::rect_with_title_span_effective_width_px(
                ctx.text_style.font_size,
                title.trim(),
                title_metrics.width,
            );
            let title_h = state_text_overrides::rect_with_title_span_effective_height_px(
                ctx.text_style.font_size,
                title.trim(),
                title_metrics.height,
            );
            let desc_w = state_text_overrides::rect_with_title_span_effective_width_px(
                ctx.text_style.font_size,
                desc.trim(),
                desc_metrics.width,
            );
            let desc_h = state_text_overrides::rect_with_title_span_effective_height_px(
                ctx.text_style.font_size,
                desc.trim(),
                desc_metrics.height,
            );
            let inner_w = (w - padding).max(0.0);
            let title_x = ((inner_w - title_w) / 2.0).max(0.0);
            let desc_x = ((inner_w - desc_w) / 2.0).max(0.0);
            let desc_y = title_h + gap;
            let divider_y = -h / 2.0 + top_pad + title_h + 1.0;
            let label_html_start = timing_enabled.then(std::time::Instant::now);
            let title_html = state_node_label_inline_html(&title);
            let desc_html = state_node_label_inline_html(&desc);
            if let Some(s) = label_html_start {
                details.leaf_nodes_label_html += s.elapsed();
            }
            let _g_emit = detail_guard(timing_enabled, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r#"<g class="{}" id="{}" transform="translate({}, {})"><g><rect class="outer title-state" style="" x="{}" y="{}" width="{}" height="{}"/><line class="divider" x1="{}" x2="{}" y1="{}" y2="{}"/></g><g class="label" style="" transform="translate({}, {})"><foreignObject width="{}" height="{}" transform="translate( {}, 0)"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: {}px; white-space: nowrap;">{}</div></foreignObject><foreignObject width="{}" height="{}" transform="translate( {}, {})"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: {}px; white-space: nowrap;">{}</div></foreignObject></g></g>"#,
                escape_xml_display(&node_class),
                escape_xml_display(&node.dom_id),
                fmt_display(cx),
                fmt_display(cy),
                fmt_display(-w / 2.0),
                fmt_display(-h / 2.0),
                fmt_display(w),
                fmt_display(h),
                fmt_display(-w / 2.0),
                fmt_display(w / 2.0),
                fmt_display(divider_y),
                fmt_display(divider_y),
                fmt_display(-w / 2.0 + half_pad),
                fmt_display(-h / 2.0 + top_pad),
                fmt_display(title_w),
                fmt_display(title_h),
                fmt_display(title_x),
                fmt_display(state_text_overrides::state_rect_with_title_span_padding_right_px()),
                title_html,
                fmt_display(desc_w),
                fmt_display(desc_h),
                fmt_display(desc_x),
                fmt_display(desc_y),
                fmt_display(state_text_overrides::state_rect_with_title_span_padding_right_px()),
                desc_html
            );
            drop(_g_emit);
        }
        _ => {
            let label = state_node_label_text(node);

            fn parse_css_px_f64(v: &str) -> Option<f64> {
                let t = v.trim();
                let t = t.trim_end_matches(';').trim();
                let t = t.trim_end_matches("!important").trim();
                let t = t.trim_end_matches("px").trim();
                t.parse::<f64>().ok()
            }

            let mut measure_style = ctx.text_style.clone();
            let mut has_metrics_style: bool = false;
            let mut italic: bool = false;

            for d in &text_decls {
                let k = d.key.trim().to_ascii_lowercase();
                let v = d.val.trim().trim_end_matches(';').trim();
                let v_no_imp = v.trim_end_matches("!important").trim();
                match k.as_str() {
                    "font-weight" => {
                        if !v_no_imp.is_empty() {
                            measure_style.font_weight = Some(v_no_imp.to_string());
                            has_metrics_style = true;
                        }
                    }
                    "font-style" => {
                        let lower = v_no_imp.to_ascii_lowercase();
                        if lower.contains("italic") || lower.contains("oblique") {
                            italic = true;
                            has_metrics_style = true;
                        }
                    }
                    "font-size" => {
                        if let Some(px) = parse_css_px_f64(v_no_imp) {
                            if px.is_finite() && px > 0.0 {
                                measure_style.font_size = px;
                                has_metrics_style = true;
                            }
                        }
                    }
                    "font-family" => {
                        if !v_no_imp.is_empty() {
                            measure_style.font_family = Some(v_no_imp.to_string());
                            has_metrics_style = true;
                        }
                    }
                    _ => {}
                }
            }

            let measure_start = timing_enabled.then(std::time::Instant::now);
            let mut metrics = ctx.measurer.measure_wrapped(
                &label,
                &measure_style,
                Some(ctx.html_label_wrapping_width),
                WrapMode::HtmlLike,
            );
            if let Some(s) = measure_start {
                details.leaf_nodes_measure += s.elapsed();
            }

            if italic {
                metrics.width +=
                    crate::text::mermaid_default_italic_width_delta_px(&label, &measure_style);
            }
            metrics.width +=
                crate::text::mermaid_default_bold_width_delta_px(&label, &measure_style);

            if metrics.width.is_finite() {
                metrics.width = metrics.width.min(ctx.html_label_wrapping_width);
            }
            metrics.width = crate::text::round_to_1_64_px(metrics.width);
            if metrics.width.is_finite() {
                metrics.width = metrics.width.min(ctx.html_label_wrapping_width);
            }

            if !has_metrics_style {
                if let Some(w) =
                    crate::generated::state_text_overrides_11_12_2::lookup_state_node_label_width_px(
                        measure_style.font_size,
                        label.trim(),
                    )
                {
                    metrics.width = w;
                }
            }

            let bold = measure_style
                .font_weight
                .as_deref()
                .is_some_and(|s| s.to_ascii_lowercase().contains("bold"));
            if let Some(w) =
                crate::generated::state_text_overrides_11_12_2::lookup_state_node_label_width_px_styled(
                    measure_style.font_size,
                    label.trim(),
                    bold,
                    italic,
                )
            {
                metrics.width = w;
            }

            let has_classdef_border_style = node
                .css_compiled_styles
                .iter()
                .any(|s| s.trim_start().to_ascii_lowercase().starts_with("border:"));

            // Mermaid@11.12.2 browser baselines show a surprising `getBoundingClientRect()` inflation
            // for `classDef`-styled border nodes: even a single-line `<p>` label can measure as `72px`
            // tall. Mirror that behavior here to avoid relying on string-keyed height overrides.
            if has_classdef_border_style && (measure_style.font_size - 16.0).abs() <= 0.01 {
                let trimmed = label.trim();
                let is_single_line = !trimmed.contains('\n')
                    && !trimmed.to_ascii_lowercase().contains("<br")
                    && !trimmed.is_empty();
                if is_single_line && (metrics.height - 24.0).abs() <= 0.01 {
                    metrics.height = metrics.height.max(72.0);
                }
            }
            let lw = metrics.width.max(0.0);
            let lh = metrics.height.max(0.0);

            let mut link_open = String::new();
            let mut link_close = String::new();
            if let Some(links) = ctx.links.get(node_id) {
                let mut push_link = |link: &StateSvgLink| {
                    let url = link.url.trim();
                    let tooltip = link.tooltip.trim();
                    let title_attr = if tooltip.is_empty() {
                        String::new()
                    } else {
                        format!(r#" title="{}""#, escape_attr(tooltip))
                    };

                    if !url.is_empty() && (ctx.security_level_loose || state_link_href_allowed(url))
                    {
                        link_open.push_str(&format!(
                            r#"<a xlink:href="{}"{}>"#,
                            escape_attr(url),
                            title_attr
                        ));
                        link_close.push_str("</a>");
                        return;
                    }

                    link_open.push_str(&format!(r#"<a{}>"#, title_attr));
                    link_close.push_str("</a>");
                };

                match links {
                    StateSvgLinks::One(link) => push_link(link),
                    StateSvgLinks::Many(list) => {
                        for link in list {
                            push_link(link);
                        }
                    }
                }
            }

            let fill_attr = fill_override.unwrap_or("#ECECFF");
            let stroke_attr = stroke_override.unwrap_or("#9370DB");
            let stroke_width_attr = stroke_width_override.unwrap_or(1.3).max(0.0);

            let rough_start = timing_enabled.then(std::time::Instant::now);
            let key = StateRoughCacheKey {
                tag: 6,
                a: w.to_bits(),
                b: h.to_bits(),
                seed: ctx.hand_drawn_seed,
            };
            if timing_enabled {
                details.leaf_roughjs_calls += 1;
                details.leaf_roughjs_unique.insert(key);
            }
            let (fill_d, stroke_d) = cached_paths(ctx, key, || {
                roughjs_paths_for_svg_path(
                    &mermaid_rounded_rect_path_data(w, h),
                    "#ECECFF",
                    "#9370DB",
                    1.3,
                    "0 0",
                    ctx.hand_drawn_seed,
                )
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()))
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }

            let label_span_style = if text_style_attr.is_empty() {
                None
            } else {
                Some(text_style_attr.as_str())
            };
            let label_html_start = timing_enabled.then(std::time::Instant::now);
            let label_html = state_node_label_html_with_style(&label, label_span_style);
            if let Some(s) = label_html_start {
                details.leaf_nodes_label_html += s.elapsed();
            }

            let div_style = if metrics.line_count > 1 {
                format!(
                    r#"{}display: table; white-space: break-spaces; line-height: 1.5; max-width: {}px; text-align: center; width: {}px;"#,
                    div_style_prefix,
                    fmt(ctx.html_label_wrapping_width),
                    fmt(lw),
                )
            } else {
                format!(
                    r#"{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"#,
                    div_style_prefix,
                    fmt(ctx.html_label_wrapping_width)
                )
            };

            let _g_emit = detail_guard(timing_enabled, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r##"<g class="{}" id="{}" transform="translate({}, {})"><g class="basic label-container outer-path"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g>{}<g class="label" style="{}" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}">{}</div></foreignObject></g>{}</g>"##,
                escape_xml_display(&node_class),
                escape_xml_display(&node.dom_id),
                fmt_display(cx),
                fmt_display(cy),
                fill_d.as_str(),
                escape_xml_display(fill_attr),
                escape_xml_display(&shape_style_attr),
                stroke_d.as_str(),
                escape_xml_display(stroke_attr),
                fmt_display(stroke_width_attr),
                escape_xml_display(&shape_style_attr),
                link_open,
                escape_xml_display(&text_style_attr),
                fmt_display(-lw / 2.0),
                fmt_display(-lh / 2.0),
                fmt_display(lw),
                fmt_display(lh),
                div_style,
                label_html,
                link_close
            );
            drop(_g_emit);
        }
    }
}
