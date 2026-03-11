use super::*;

pub(super) fn render_flowchart_v2_svg(
    layout: &FlowchartV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let config = merman_core::MermaidConfig::from_value(effective_config.clone());
    render_flowchart_v2_svg_with_config(layout, semantic, &config, diagram_title, measurer, options)
}

#[inline]
fn section<'a>(
    enabled: bool,
    dst: &'a mut std::time::Duration,
) -> Option<super::super::timing::TimingGuard<'a>> {
    enabled.then(|| super::super::timing::TimingGuard::new(dst))
}

fn prepare_render_edges_and_extra_nodes<'a>(
    model: &'a crate::flowchart::FlowchartV2Model,
) -> (
    Vec<std::borrow::Cow<'a, crate::flowchart::FlowEdge>>,
    Vec<crate::flowchart::FlowNode>,
) {
    // Mermaid expands self-loop edges into a chain of helper nodes plus `*-cyclic-special-*` edge
    // segments during Dagre layout. Replicate that expansion here so rendered SVG ids match.
    let self_loop_count = model.edges.iter().filter(|e| e.from == e.to).count();
    let mut render_edges: Vec<std::borrow::Cow<'a, crate::flowchart::FlowEdge>> =
        Vec::with_capacity(model.edges.len() + self_loop_count * 3);
    let mut self_loop_label_node_ids: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for e in &model.edges {
        if e.from != e.to {
            render_edges.push(std::borrow::Cow::Borrowed(e));
            continue;
        }

        let node_id = e.from.clone();
        let special_id_1 = format!("{node_id}---{node_id}---1");
        let special_id_2 = format!("{node_id}---{node_id}---2");
        self_loop_label_node_ids.insert(special_id_1.clone());
        self_loop_label_node_ids.insert(special_id_2.clone());

        let mut edge1 = e.clone();
        edge1.id = format!("{node_id}-cyclic-special-1");
        edge1.from = node_id.clone();
        edge1.to = special_id_1.clone();
        edge1.label = None;
        edge1.label_type = None;
        edge1.edge_type = Some("arrow_open".to_string());

        let mut edge_mid = e.clone();
        edge_mid.id = format!("{node_id}-cyclic-special-mid");
        edge_mid.from = special_id_1.clone();
        edge_mid.to = special_id_2.clone();
        edge_mid.edge_type = Some("arrow_open".to_string());

        let mut edge2 = e.clone();
        edge2.id = format!("{node_id}-cyclic-special-2");
        edge2.from = special_id_2.clone();
        edge2.to = node_id.clone();
        edge2.label = None;
        edge2.label_type = None;

        render_edges.push(std::borrow::Cow::Owned(edge1));
        render_edges.push(std::borrow::Cow::Owned(edge_mid));
        render_edges.push(std::borrow::Cow::Owned(edge2));
    }

    // Mermaid's `adjustClustersAndEdges(graph)` rewrites edges that connect directly to cluster
    // nodes by removing and re-adding them (after swapping endpoints to anchor nodes). This has a
    // visible side-effect: those edges end up later in `graph.edges()` insertion order, so the
    // DOM emitted under `.edgePaths` / `.edgeLabels` matches that stable partition.
    let cluster_ids_with_children: FxHashSet<&str> = model
        .subgraphs
        .iter()
        .filter(|sg| !sg.nodes.is_empty())
        .map(|sg| sg.id.as_str())
        .collect();
    if !cluster_ids_with_children.is_empty() && render_edges.len() >= 2 {
        let mut normal: Vec<std::borrow::Cow<'a, crate::flowchart::FlowEdge>> =
            Vec::with_capacity(render_edges.len());
        let mut cluster: Vec<std::borrow::Cow<'a, crate::flowchart::FlowEdge>> = Vec::new();
        for e in render_edges {
            let edge = e.as_ref();
            if cluster_ids_with_children.contains(edge.from.as_str())
                || cluster_ids_with_children.contains(edge.to.as_str())
            {
                cluster.push(e);
            } else {
                normal.push(e);
            }
        }
        normal.extend(cluster);
        render_edges = normal;
    }

    let mut extra_nodes: Vec<crate::flowchart::FlowNode> =
        Vec::with_capacity(self_loop_label_node_ids.len());
    for id in &self_loop_label_node_ids {
        extra_nodes.push(crate::flowchart::FlowNode {
            id: id.clone(),
            label: Some(String::new()),
            label_type: None,
            layout_shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            have_callback: false,
            link: None,
            link_target: None,
        });
    }

    (render_edges, extra_nodes)
}

pub(super) fn render_flowchart_v2_svg_model(
    layout: &FlowchartV2Layout,
    model: &crate::flowchart::FlowchartV2Model,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let config = merman_core::MermaidConfig::from_value(effective_config.clone());
    render_flowchart_v2_svg_model_with_config(
        layout,
        model,
        &config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_flowchart_v2_svg_model_with_config(
    layout: &FlowchartV2Layout,
    model: &crate::flowchart::FlowchartV2Model,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let timing_enabled = super::super::timing::render_timing_enabled();
    let mut timings = super::super::timing::RenderTimings::default();
    let total_start = std::time::Instant::now();

    render_flowchart_v2_svg_with_config_inner(
        layout,
        model,
        effective_config,
        diagram_title,
        measurer,
        options,
        timing_enabled,
        &mut timings,
        total_start,
    )
}

pub(super) fn render_flowchart_v2_svg_with_config(
    layout: &FlowchartV2Layout,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let timing_enabled = super::super::timing::render_timing_enabled();
    let mut timings = super::super::timing::RenderTimings::default();
    let total_start = std::time::Instant::now();

    let model: crate::flowchart::FlowchartV2Model = {
        let _g = section(timing_enabled, &mut timings.deserialize_model);
        crate::json::from_value_ref(semantic)?
    };

    render_flowchart_v2_svg_with_config_inner(
        layout,
        &model,
        effective_config,
        diagram_title,
        measurer,
        options,
        timing_enabled,
        &mut timings,
        total_start,
    )
}

fn render_flowchart_v2_svg_with_config_inner(
    layout: &FlowchartV2Layout,
    model: &crate::flowchart::FlowchartV2Model,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
    timing_enabled: bool,
    timings: &mut super::super::timing::RenderTimings,
    total_start: std::time::Instant,
) -> Result<String> {
    let effective_config_value = effective_config.as_value();

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_type = "flowchart-v2";

    let _g_build_ctx = section(timing_enabled, &mut timings.build_ctx);

    let (render_edges, extra_nodes) = prepare_render_edges_and_extra_nodes(model);

    fn parse_font_size_px(v: &serde_json::Value) -> Option<f64> {
        if let Some(n) = v.as_f64() {
            return Some(n);
        }
        if let Some(n) = v.as_i64() {
            return Some(n as f64);
        }
        if let Some(n) = v.as_u64() {
            return Some(n as f64);
        }
        let s = v.as_str()?.trim();
        if s.is_empty() {
            return None;
        }
        let mut num = String::new();
        for (idx, ch) in s.chars().enumerate() {
            if ch.is_ascii_digit() {
                num.push(ch);
                continue;
            }
            if idx == 0 && (ch == '-' || ch == '+') {
                num.push(ch);
                continue;
            }
            break;
        }
        if num.trim().is_empty() {
            return None;
        }
        num.parse::<f64>().ok()
    }

    let default_theme_font_family = "\"trebuchet ms\",verdana,arial,sans-serif".to_string();
    let theme_font_family =
        config_string(effective_config_value, &["themeVariables", "fontFamily"])
            .map(|s| normalize_css_font_family(&s));
    let top_font_family = config_string(effective_config_value, &["fontFamily"])
        .map(|s| normalize_css_font_family(&s));
    let font_family = match (top_font_family, theme_font_family) {
        (Some(top), Some(theme)) if theme == default_theme_font_family => top,
        (_, Some(theme)) => theme,
        (Some(top), None) => top,
        (None, None) => default_theme_font_family,
    };
    let font_size = effective_config_value
        .get("themeVariables")
        .and_then(|tv| tv.get("fontSize"))
        .and_then(parse_font_size_px)
        .unwrap_or(16.0)
        .max(1.0);

    let wrapping_width = config_f64(effective_config_value, &["flowchart", "wrappingWidth"])
        .unwrap_or(200.0)
        .max(1.0);
    // Mermaid flowchart-v2 uses the global `htmlLabels` toggle for node/subgraph labels, while
    // edge labels follow `flowchart.htmlLabels` (falling back to the global toggle when unset).
    let node_html_labels = effective_config_value
        .get("htmlLabels")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let edge_html_labels = effective_config_value
        .get("flowchart")
        .and_then(|v| v.get("htmlLabels"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(node_html_labels);
    let node_wrap_mode = if node_html_labels {
        crate::text::WrapMode::HtmlLike
    } else {
        crate::text::WrapMode::SvgLike
    };
    let edge_wrap_mode = if edge_html_labels {
        crate::text::WrapMode::HtmlLike
    } else {
        crate::text::WrapMode::SvgLike
    };
    let diagram_padding = config_f64(effective_config_value, &["flowchart", "diagramPadding"])
        .unwrap_or(8.0)
        .max(0.0);
    let use_max_width = effective_config_value
        .get("flowchart")
        .and_then(|v| v.get("useMaxWidth"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let title_top_margin = config_f64(effective_config_value, &["flowchart", "titleTopMargin"])
        .unwrap_or(25.0)
        .max(0.0);
    let node_padding = config_f64(effective_config_value, &["flowchart", "padding"])
        .unwrap_or(15.0)
        .max(0.0);

    let text_style = crate::text::TextStyle {
        font_family: Some(font_family.clone()),
        font_size,
        font_weight: None,
    };

    let node_order: Vec<&str> = model.nodes.iter().map(|n| n.id.as_str()).collect();

    let mut nodes_by_id: FxHashMap<&str, &crate::flowchart::FlowNode> =
        FxHashMap::with_capacity_and_hasher(
            model.nodes.len() + extra_nodes.len(),
            Default::default(),
        );
    for n in &model.nodes {
        nodes_by_id.insert(n.id.as_str(), n);
    }
    for n in &extra_nodes {
        let _ = nodes_by_id.entry(n.id.as_str()).or_insert(n);
    }

    let edge_order: Vec<&str> = render_edges
        .iter()
        .map(|e| e.as_ref().id.as_str())
        .collect();
    let mut edges_by_id: FxHashMap<&str, &crate::flowchart::FlowEdge> =
        FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());
    for e in &render_edges {
        let edge = e.as_ref();
        edges_by_id.insert(edge.id.as_str(), edge);
    }

    let subgraph_order: Vec<&str> = model.subgraphs.iter().map(|s| s.id.as_str()).collect();
    let mut subgraphs_by_id: FxHashMap<&str, &crate::flowchart::FlowSubgraph> =
        FxHashMap::with_capacity_and_hasher(model.subgraphs.len(), Default::default());
    for sg in &model.subgraphs {
        subgraphs_by_id.insert(sg.id.as_str(), sg);
    }

    let mut parent: FxHashMap<&str, &str> = FxHashMap::default();
    for sg in &model.subgraphs {
        let sg_id = sg.id.as_str();
        for child in &sg.nodes {
            parent.insert(child.as_str(), sg_id);
        }
    }
    for n in &extra_nodes {
        let id = n.id.as_str();
        let Some((base, _)) = id.split_once("---") else {
            continue;
        };
        if let Some(&p) = parent.get(base) {
            parent.insert(id, p);
        }
    }

    let mut recursive_clusters: FxHashSet<&str> = FxHashSet::default();
    for sg in model.subgraphs.iter() {
        if sg.nodes.is_empty() {
            continue;
        }
        let mut external = false;
        for e in &render_edges {
            let e = e.as_ref();
            // Match Mermaid `adjustClustersAndEdges` / flowchart-v2 behavior: a cluster is
            // considered to have external connections when an edge crosses its descendant boundary.
            let from_in = flowchart_is_strict_descendant(&parent, e.from.as_str(), sg.id.as_str());
            let to_in = flowchart_is_strict_descendant(&parent, e.to.as_str(), sg.id.as_str());
            if from_in != to_in {
                external = true;
                break;
            }
        }
        if !external {
            recursive_clusters.insert(sg.id.as_str());
        }
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

    // Mermaid flowchart-v2 does not translate the root `.root` group; node/edge coordinates are
    // already in the Dagre coordinate space (including Dagre's fixed `marginx/marginy=8`).
    // `diagramPadding` is applied only when computing the final SVG viewBox.
    let tx = 0.0;
    let ty = 0.0;

    let node_dom_index = flowchart_node_dom_indices(&model);

    let cfg_curve = config_string(effective_config_value, &["flowchart", "curve"]);
    let default_edge_interpolate = model
        .edge_defaults
        .as_ref()
        .and_then(|d| d.interpolate.as_deref())
        .or(cfg_curve.as_deref())
        .unwrap_or("basis")
        .to_string();
    let default_edge_style = model
        .edge_defaults
        .as_ref()
        .map(|d| d.style.clone())
        .unwrap_or_default();

    let node_border_color = theme_color(effective_config_value, "nodeBorder", "#9370DB");
    let node_fill_color = theme_color(effective_config_value, "mainBkg", "#ECECFF");

    let ctx = FlowchartRenderCtx {
        diagram_id,
        tx,
        ty,
        diagram_type,
        measurer,
        config: effective_config,
        math_renderer: options.math_renderer.as_deref(),
        node_html_labels,
        edge_html_labels,
        class_defs: &model.class_defs,
        node_border_color,
        node_fill_color,
        default_edge_interpolate,
        default_edge_style,
        trace_edge_id: std::env::var("MERMAN_TRACE_FLOWCHART_EDGE").ok(),
        node_order,
        subgraph_order,
        edge_order,
        nodes_by_id,
        edges_by_id,
        subgraphs_by_id,
        tooltips: &model.tooltips,
        recursive_clusters,
        parent,
        layout_nodes_by_id,
        layout_edges_by_id,
        layout_clusters_by_id,
        dom_node_order_by_root: &layout.dom_node_order_by_root,
        node_dom_index,
        node_padding,
        wrapping_width,
        node_wrap_mode,
        edge_wrap_mode,
        text_style,
        diagram_title,
    };

    let mut edge_path_cache: FxHashMap<&str, FlowchartEdgePathCacheEntry> =
        FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());

    let subgraph_title_y_shift = {
        let top = config_f64(
            effective_config_value,
            &["flowchart", "subGraphTitleMargin", "top"],
        )
        .unwrap_or(0.0)
        .max(0.0);
        let bottom = config_f64(
            effective_config_value,
            &["flowchart", "subGraphTitleMargin", "bottom"],
        )
        .unwrap_or(0.0)
        .max(0.0);
        (top + bottom) / 2.0
    };

    fn self_loop_label_base_node_id(id: &str) -> Option<&str> {
        let mut parts = id.split("---");
        let a = parts.next()?;
        let b = parts.next()?;
        let n = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        if a != b {
            return None;
        }
        if n != "1" && n != "2" {
            return None;
        }
        Some(a)
    }

    drop(_g_build_ctx);

    let mut detail = FlowchartRenderDetails::default();
    let mut viewbox_edge_curve_bounds = std::time::Duration::ZERO;
    let _g_viewbox = section(timing_enabled, &mut timings.viewbox);

    let effective_parent_for_id = |id: &str| -> Option<&str> {
        let mut cur = ctx.parent.get(id).copied();
        if cur.is_none() {
            if let Some(base) = self_loop_label_base_node_id(id) {
                cur = ctx.parent.get(base).copied();
            }
        }
        while let Some(p) = cur {
            if ctx.subgraphs_by_id.contains_key(p) && !ctx.recursive_clusters.contains(p) {
                cur = ctx.parent.get(p).copied();
                continue;
            }
            return Some(p);
        }
        None
    };

    fn lca_for_ids<'a, F>(
        a: &str,
        b: &str,
        effective_parent_for_id: &F,
        scratch: &mut Vec<&'a str>,
    ) -> Option<&'a str>
    where
        F: Fn(&str) -> Option<&'a str>,
    {
        scratch.clear();
        let mut cur = effective_parent_for_id(a);
        while let Some(p) = cur {
            scratch.push(p);
            cur = effective_parent_for_id(p);
        }

        let mut cur = effective_parent_for_id(b);
        while let Some(p) = cur {
            if scratch.iter().any(|&v| v == p) {
                return Some(p);
            }
            cur = effective_parent_for_id(p);
        }
        None
    }

    let mut lca_scratch: Vec<&str> = Vec::new();

    let y_offset_for_root = |root: Option<&str>| -> f64 {
        if root.is_some() && subgraph_title_y_shift.abs() >= 1e-9 {
            -subgraph_title_y_shift
        } else {
            0.0
        }
    };

    // Mermaid's flowchart-v2 renderer draws the self-loop helper nodes (`labelRect`) as
    // `<g class="label edgeLabel" transform="translate(x, y)">` with a `0.1 x 0.1` rect anchored
    // at the translated origin (top-left). Dagre's `x/y` still represent a node center, but the
    // rendered DOM bbox that drives `setupViewPortForSVG(svg, diagramPadding)` is top-left based.
    // Account for that when approximating the final `svg.getBBox()`.
    let bounds = {
        let mut b: Option<Bounds> = None;
        let mut include_rect = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
            if let Some(ref mut cur) = b {
                cur.min_x = cur.min_x.min(min_x);
                cur.min_y = cur.min_y.min(min_y);
                cur.max_x = cur.max_x.max(max_x);
                cur.max_y = cur.max_y.max(max_y);
            } else {
                b = Some(Bounds {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                });
            }
        };

        for c in &layout.clusters {
            let root = if ctx.recursive_clusters.contains(c.id.as_str()) {
                Some(c.id.as_str())
            } else {
                effective_parent_for_id(&c.id)
            };
            let y_off = y_offset_for_root(root);
            let hw = c.width / 2.0;
            let hh = c.height / 2.0;
            include_rect(c.x - hw, c.y + y_off - hh, c.x + hw, c.y + y_off + hh);

            let lhw = c.title_label.width / 2.0;
            let lhh = c.title_label.height / 2.0;
            include_rect(
                c.title_label.x - lhw,
                c.title_label.y + y_off - lhh,
                c.title_label.x + lhw,
                c.title_label.y + y_off + lhh,
            );
        }

        for n in &layout.nodes {
            let root = if n.is_cluster && ctx.recursive_clusters.contains(n.id.as_str()) {
                Some(n.id.as_str())
            } else {
                effective_parent_for_id(&n.id)
            };
            let y_off = y_offset_for_root(root);
            if n.is_cluster || ctx.node_dom_index.contains_key(n.id.as_str()) {
                let mut left_hw = n.width / 2.0;
                let mut right_hw = left_hw;
                let mut top_hh = n.height / 2.0;
                let mut bottom_hh = top_hh;
                if !n.is_cluster {
                    if let Some(shape) = ctx
                        .nodes_by_id
                        .get(n.id.as_str())
                        .and_then(|node| node.layout_shape.as_deref())
                    {
                        // Mermaid's flowchart-v2 rhombus node renderer offsets the polygon by
                        // `(-width/2 + 0.5, height/2)` so the diamond outline stays on the same
                        // pixel lattice as other nodes. This makes the DOM bbox slightly
                        // asymmetric around the node center and affects the root `getBBox()`
                        // width (and thus `viewBox` / `max-width`) by 0.5px.
                        if shape == "diamond" || shape == "diam" || shape == "rhombus" {
                            left_hw = (left_hw - 0.5).max(0.0);
                            right_hw += 0.5;
                        }

                        // Mermaid `stateEnd.ts` renders the framed-circle using a RoughJS ellipse
                        // path with a slightly asymmetric bbox in Chromium. Model that asymmetry
                        // so root `viewBox` parity matches upstream.
                        if matches!(shape, "fr-circ" | "framed-circle" | "stop") {
                            left_hw = 7.0;
                            right_hw = (n.width - 7.0).max(0.0);
                        }

                        // Mermaid `filledCircle.ts` uses a RoughJS circle path (roughness=0) whose
                        // bbox is slightly asymmetric (it extends further to the right). Model
                        // that asymmetry so root `viewBox` parity matches upstream.
                        if matches!(shape, "f-circ") {
                            left_hw = 7.0;
                            right_hw = (n.width - 7.0).max(0.0);
                        }

                        // Mermaid `crossedCircle.ts` uses a RoughJS circle path with radius=30;
                        // its bbox is slightly asymmetric in Chromium.
                        if matches!(shape, "cross-circ") {
                            left_hw = 30.0;
                            right_hw = (n.width - 30.0).max(0.0);
                            top_hh = 30.0;
                            bottom_hh = 30.0;
                        }

                        // Mermaid `halfRoundedRectangle.ts` and `curvedTrapezoid.ts` draw their
                        // rough paths from the "theoretical" text+padding width, but Dagre uses
                        // the `updateNodeBounds(...)` bbox which can be slightly narrower. Root
                        // viewport comes from DOM `getBBox()`, so adjust the left/right extents to
                        // match the rendered path's asymmetric bbox.
                        if matches!(shape, "delay" | "curv-trap") {
                            if let Some(label_w) = n.label_width {
                                // Reuse label metrics computed during layout to avoid re-measuring
                                // HTML/markdown labels while approximating the root viewBox.
                                let pre_w = if shape == "delay" {
                                    (label_w + 2.0 * node_padding).max(80.0)
                                } else {
                                    ((label_w + 2.0 * node_padding) * 1.25).max(80.0)
                                };
                                left_hw = pre_w / 2.0;
                                right_hw = (n.width - left_hw).max(0.0);
                            } else if let Some(flow_node) = ctx.nodes_by_id.get(n.id.as_str()) {
                                // Fallback: measure if layout did not record label metrics.
                                let label = flow_node.label.as_deref().unwrap_or("");
                                let label_type = flow_node
                                    .label_type
                                    .as_deref()
                                    .unwrap_or(if ctx.node_html_labels { "html" } else { "text" });
                                let node_text_style =
                                    crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                        &ctx.text_style,
                                        ctx.class_defs,
                                        &flow_node.classes,
                                        &flow_node.styles,
                                    );
                                let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                                    ctx.measurer,
                                    label,
                                    label_type,
                                    &node_text_style,
                                    Some(ctx.wrapping_width),
                                    ctx.node_wrap_mode,
                                    ctx.config,
                                    ctx.math_renderer,
                                );
                                let pre_w = if shape == "delay" {
                                    (metrics.width + 2.0 * node_padding).max(80.0)
                                } else {
                                    ((metrics.width + 2.0 * node_padding) * 1.25).max(80.0)
                                };
                                left_hw = pre_w / 2.0;
                                right_hw = (n.width - left_hw).max(0.0);
                            }
                        }

                        // Mermaid `forkJoin.ts` inflates Dagre dimensions (via `state.padding/2`)
                        // but the rendered bar remains `70x10` (or `10x70` for LR). Root viewport
                        // comes from DOM `getBBox()`, so use the rendered dimensions here.
                        if matches!(shape, "fork" | "join") {
                            if n.width >= n.height {
                                left_hw = 35.0;
                                right_hw = 35.0;
                                top_hh = 5.0;
                                bottom_hh = 5.0;
                            } else {
                                left_hw = 5.0;
                                right_hw = 5.0;
                                top_hh = 35.0;
                                bottom_hh = 35.0;
                            }
                        }

                        // Mermaid `multiWaveEdgedRectangle.ts` (documents / stacked-document)
                        // emits a bottom sine wave and then translates the whole group upward by
                        // `waveAmplitude / 2`. The resulting DOM bbox is not vertically symmetric
                        // around the node center, so do not approximate it as `height / 2`.
                        if matches!(shape, "docs" | "documents" | "st-doc" | "stacked-document") {
                            let (label_w, label_h) = if let (Some(w), Some(h)) =
                                (n.label_width, n.label_height)
                            {
                                (w, h)
                            } else if let Some(flow_node) = ctx.nodes_by_id.get(n.id.as_str()) {
                                let label = flow_node.label.as_deref().unwrap_or("");
                                let label_type = flow_node
                                    .label_type
                                    .as_deref()
                                    .unwrap_or(if ctx.node_html_labels { "html" } else { "text" });
                                let node_text_style =
                                    crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                        &ctx.text_style,
                                        ctx.class_defs,
                                        &flow_node.classes,
                                        &flow_node.styles,
                                    );
                                let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                                    ctx.measurer,
                                    label,
                                    label_type,
                                    &node_text_style,
                                    Some(ctx.wrapping_width),
                                    ctx.node_wrap_mode,
                                    ctx.config,
                                    ctx.math_renderer,
                                );
                                (metrics.width, metrics.height)
                            } else {
                                (0.0, 0.0)
                            };

                            let w = label_w + 2.0 * node_padding;
                            let h = label_h + 2.0 * node_padding;
                            let wave_amplitude = h / 4.0;
                            let final_h = h + wave_amplitude;
                            let rect_offset = 5.0;
                            let y = -final_h / 2.0;
                            let baseline_y = y + final_h + rect_offset;

                            let mut max_wave_y = baseline_y;
                            let delta_x = w;
                            let cycle_length = if delta_x.abs() < 1e-9 {
                                delta_x
                            } else {
                                delta_x / 0.8
                            };
                            let frequency = if cycle_length.abs() < 1e-9 {
                                0.0
                            } else {
                                (2.0 * std::f64::consts::PI) / cycle_length
                            };
                            for i in 0..=50 {
                                let t = i as f64 / 50.0;
                                let x = t * delta_x;
                                let wave_y = baseline_y + wave_amplitude * (frequency * x).sin();
                                max_wave_y = max_wave_y.max(wave_y);
                            }

                            let top_y = y - rect_offset - wave_amplitude / 2.0;
                            let bottom_y = max_wave_y - wave_amplitude / 2.0;
                            top_hh = -top_y;
                            bottom_hh = bottom_y;
                            if left_hw == right_hw {
                                left_hw = w / 2.0 + rect_offset;
                                right_hw = left_hw;
                            }
                        }
                    }
                }
                include_rect(
                    n.x - left_hw,
                    n.y + y_off - top_hh,
                    n.x + right_hw,
                    n.y + y_off + bottom_hh,
                );
            } else {
                include_rect(n.x, n.y + y_off, n.x + n.width, n.y + y_off + n.height);
            }
        }

        for e in &layout.edges {
            let root = lca_for_ids(
                e.from.as_str(),
                e.to.as_str(),
                &effective_parent_for_id,
                &mut lca_scratch,
            );
            let y_off = y_offset_for_root(root);
            for lbl in [
                e.label.as_ref(),
                e.start_label_left.as_ref(),
                e.start_label_right.as_ref(),
                e.end_label_left.as_ref(),
                e.end_label_right.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                let hw = lbl.width / 2.0;
                let hh = lbl.height / 2.0;
                let svg_label_y_offset = if edge_html_labels { 0.0 } else { 1.0 };
                include_rect(
                    lbl.x - hw,
                    lbl.y + y_off - hh - svg_label_y_offset,
                    lbl.x + hw,
                    lbl.y + y_off + hh - svg_label_y_offset,
                );
            }
        }

        b.unwrap_or(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        })
    };
    // Mermaid flowchart-v2 does not translate the root `.root` group; node/edge coordinates are
    // already in the Dagre coordinate space (including Dagre's fixed `marginx/marginy=8`).
    // `diagramPadding` is applied only when computing the final SVG viewBox.

    // Mermaid computes the final viewport using `svg.getBBox()` after inserting the title, then
    // applies `setupViewPortForSVG(svg, diagramPadding)` which sets:
    //   viewBox = `${bbox.x - padding} ${bbox.y - padding} ${bbox.width + 2*padding} ${bbox.height + 2*padding}`
    //   max-width = `${bbox.width + 2*padding}px` when `useMaxWidth=true`
    //
    // In headless mode we approximate that by unioning:
    // - the layout bounds (shifted by `tx/ty`), and
    // - the flowchart title text bounding box (if present).
    const TITLE_FONT_SIZE_PX: f64 = 18.0;
    const DEFAULT_ASCENT_EM: f64 = 0.9444444444;
    const DEFAULT_DESCENT_EM: f64 = 0.262;

    let diagram_title = diagram_title.map(str::trim).filter(|t| !t.is_empty());

    let mut bbox_min_x = bounds.min_x + tx;
    let mut bbox_min_y = bounds.min_y + ty;
    let mut bbox_max_x = bounds.max_x + tx;
    let mut bbox_max_y = bounds.max_y + ty;

    // Mermaid's recursive flowchart renderer introduces additional y-offsets for some extracted
    // cluster roots (notably when an empty sibling subgraph is present). Approximate that in the
    // root viewport by expanding the max-y by the largest such extra root offset.
    let extra_recursive_root_y = {
        fn effective_parent<'a>(
            parent: &'a FxHashMap<&'a str, &'a str>,
            subgraphs_by_id: &'a FxHashMap<&'a str, &'a crate::flowchart::FlowSubgraph>,
            recursive_clusters: &FxHashSet<&'a str>,
            id: &str,
        ) -> Option<&'a str> {
            let mut cur = parent.get(id).copied();
            while let Some(p) = cur {
                if subgraphs_by_id.contains_key(p) && !recursive_clusters.contains(p) {
                    cur = parent.get(p).copied();
                    continue;
                }
                return Some(p);
            }
            None
        }

        let mut max_y: f64 = 0.0;
        for &cid in &ctx.recursive_clusters {
            let Some(cluster) = ctx.layout_clusters_by_id.get(cid) else {
                continue;
            };
            let my_parent = effective_parent(
                &ctx.parent,
                &ctx.subgraphs_by_id,
                &ctx.recursive_clusters,
                cid,
            );
            let has_empty_sibling = ctx.subgraphs_by_id.iter().any(|(&id, &sg)| {
                id != cid
                    && sg.nodes.is_empty()
                    && ctx.layout_clusters_by_id.contains_key(id)
                    && effective_parent(
                        &ctx.parent,
                        &ctx.subgraphs_by_id,
                        &ctx.recursive_clusters,
                        id,
                    ) == my_parent
            });
            if has_empty_sibling {
                max_y = max_y.max(cluster.offset_y.max(0.0) * 2.0);
            }
        }
        max_y
    };

    // Mermaid derives the final viewport using `svg.getBBox()` (after rendering). For flowcharts
    // this includes the actual curve geometry generated by D3 (which can extend beyond the routed
    // polyline points). Headlessly, approximate that by unioning a tight bbox over each rendered
    // edge path `d` into our base bbox.
    {
        let _g = section(timing_enabled, &mut viewbox_edge_curve_bounds);
        let mut scratch = FlowchartEdgeDataPointsScratch::default();
        let mut root_offsets: FxHashMap<&str, FlowchartRootOffsets> =
            FxHashMap::with_capacity_and_hasher(8, Default::default());
        root_offsets.insert(
            "",
            FlowchartRootOffsets {
                origin_x: 0.0,
                origin_y: 0.0,
                abs_top_transform: 0.0,
            },
        );
        for e in &render_edges {
            let e = e.as_ref();
            let root_id = {
                let _g = detail_guard(timing_enabled, &mut detail.viewbox_edge_curve_lca);
                lca_for_ids(
                    e.from.as_str(),
                    e.to.as_str(),
                    &effective_parent_for_id,
                    &mut lca_scratch,
                )
                .unwrap_or("")
            };
            let off = {
                let _g = detail_guard(timing_enabled, &mut detail.viewbox_edge_curve_offsets);
                *root_offsets.entry(root_id).or_insert_with(|| {
                    flowchart_cluster_root_offsets(&ctx, root_id).unwrap_or(FlowchartRootOffsets {
                        origin_x: 0.0,
                        origin_y: 0.0,
                        abs_top_transform: 0.0,
                    })
                })
            };

            let Some(geom) = ({
                detail.viewbox_edge_curve_geom_calls += 1;
                let _g = detail_guard(timing_enabled, &mut detail.viewbox_edge_curve_geom);
                flowchart_compute_edge_path_geom(
                    &ctx,
                    e,
                    off.origin_x,
                    off.origin_y,
                    off.abs_top_transform,
                    &mut scratch,
                    false,
                    Some((bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y)),
                )
            }) else {
                continue;
            };
            if geom.bounds_skipped_for_viewbox {
                detail.viewbox_edge_curve_geom_skipped_bounds += 1;
            }

            {
                let _g = detail_guard(timing_enabled, &mut detail.viewbox_edge_curve_bbox_union);
                if let Some(pb) = geom.pb {
                    bbox_min_x = bbox_min_x.min(pb.min_x + off.origin_x);
                    bbox_min_y = bbox_min_y.min(pb.min_y + off.abs_top_transform);
                    bbox_max_x = bbox_max_x.max(pb.max_x + off.origin_x);
                    bbox_max_y = bbox_max_y.max(pb.max_y + off.abs_top_transform);
                }

                edge_path_cache.insert(
                    e.id.as_str(),
                    FlowchartEdgePathCacheEntry {
                        origin_x: off.origin_x,
                        origin_y: off.origin_y,
                        abs_top_transform: off.abs_top_transform,
                        geom,
                    },
                );
            }
        }
    }

    bbox_max_y += extra_recursive_root_y;
    // Mermaid centers the title using the pre-title `getBBox()` of the rendered root group:
    //
    //   const bounds = parent.node()?.getBBox();
    //   x = bounds.x + bounds.width / 2
    //
    // Use our current content bbox (after accounting for edge curve geometry) to match that
    // behavior more closely in headless mode.
    let title_anchor_x = (bbox_min_x + bbox_max_x) / 2.0;

    if let Some(title) = diagram_title {
        let title_style = TextStyle {
            font_family: Some(font_family.clone()),
            font_size: TITLE_FONT_SIZE_PX,
            font_weight: None,
        };
        let (title_left, title_right) = measurer.measure_svg_title_bbox_x(title, &title_style);
        let baseline_y = -title_top_margin;
        // Mermaid title bbox uses SVG `getBBox()`, which varies slightly across fonts.
        // Courier in Mermaid@11.12.2 has a visibly smaller ascender than the default
        // `"trebuchet ms", verdana, arial, sans-serif` baseline; model that so viewBox parity
        // matches upstream fixtures.
        let (ascent_em, descent_em) = if font_family.to_ascii_lowercase().contains("courier") {
            (0.8333333333333334, 0.25)
        } else {
            (DEFAULT_ASCENT_EM, DEFAULT_DESCENT_EM)
        };
        let ascent = TITLE_FONT_SIZE_PX * ascent_em;
        let descent = TITLE_FONT_SIZE_PX * descent_em;

        bbox_min_x = bbox_min_x.min(title_anchor_x - title_left);
        bbox_max_x = bbox_max_x.max(title_anchor_x + title_right);
        bbox_min_y = bbox_min_y.min(baseline_y - ascent);
        bbox_max_y = bbox_max_y.max(baseline_y + descent);
    }

    // Chromium's `getBBox()` values frequently land on an `f32` lattice. Mermaid then computes the
    // root viewport in JS double space:
    // - viewBox.x/y = bbox.x/y - padding
    // - viewBox.w/h = bbox.width/height + 2*padding
    //
    // Mirror that by quantizing the content bounds to `f32` first, then applying padding in `f64`.
    let bbox_min_x_f32 = bbox_min_x as f32;
    let bbox_min_y_f32 = bbox_min_y as f32;
    let bbox_max_x_f32 = bbox_max_x as f32;
    let bbox_max_y_f32 = bbox_max_y as f32;
    let bbox_w_f32 = (bbox_max_x_f32 - bbox_min_x_f32).max(1.0);
    let bbox_h_f32 = (bbox_max_y_f32 - bbox_min_y_f32).max(1.0);

    let vb_min_x = (bbox_min_x_f32 as f64) - diagram_padding;
    let vb_min_y = (bbox_min_y_f32 as f64) - diagram_padding;
    let vb_w = (bbox_w_f32 as f64) + diagram_padding * 2.0;
    let vb_h = (bbox_h_f32 as f64) + diagram_padding * 2.0;

    drop(_g_viewbox);
    let _g_render_svg = section(timing_enabled, &mut timings.render_svg);

    let css = flowchart_css(
        diagram_id,
        effective_config_value,
        &font_family,
        font_size,
        &model.class_defs,
    );

    let estimated_svg_bytes = 2048usize
        + css.len()
        + layout.nodes.len().saturating_mul(256)
        + render_edges.len().saturating_mul(256)
        + layout.clusters.len().saturating_mul(128);
    let mut out = String::with_capacity(estimated_svg_bytes);

    let vb_w = vb_w.max(1.0);
    let vb_h = vb_h.max(1.0);

    let mut viewbox_attr = format!(
        "{} {} {} {}",
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w),
        fmt(vb_h)
    );
    let mut max_w_attr = fmt_max_width_px(vb_w);
    let mut w_attr = fmt_string(vb_w);
    let mut h_attr = fmt_string(vb_h);
    apply_root_viewport_override(
        diagram_id,
        &mut viewbox_attr,
        &mut w_attr,
        &mut h_attr,
        &mut max_w_attr,
        crate::generated::flowchart_root_overrides_11_12_2::lookup_flowchart_root_viewport_override,
    );

    let acc_title = model
        .acc_title
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(|s| s.trim_end_matches('\n'))
        .filter(|s| !s.trim().is_empty());
    let aria_labelledby_raw = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby_raw = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let aria_labelledby_attr = aria_labelledby_raw
        .as_deref()
        .map(super::super::util::escape_attr);
    let aria_describedby_attr = aria_describedby_raw
        .as_deref()
        .map(super::super::util::escape_attr);

    if use_max_width {
        let style_attr = format!("max-width: {max_w_attr}px; background-color: white;");
        root_svg::push_svg_root_open_ex(
            &mut out,
            diagram_id,
            Some("flowchart"),
            root_svg::SvgRootWidth::Percent100,
            None,
            Some(style_attr.as_str()),
            Some(viewbox_attr.as_str()),
            root_svg::SvgRootStyleViewBoxOrder::StyleThenViewBox,
            &[],
            diagram_type,
            aria_labelledby_attr.as_deref(),
            aria_describedby_attr.as_deref(),
            false,
        );
    } else {
        let after_roledescription_attrs: [(&str, &str); 1] =
            [("style", "background-color: white;")];
        root_svg::push_svg_root_open_ex3(
            &mut out,
            diagram_id,
            Some("flowchart"),
            root_svg::SvgRootWidth::Fixed(w_attr.as_str()),
            Some(h_attr.as_str()),
            None,
            Some(viewbox_attr.as_str()),
            root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
            &[],
            diagram_type,
            aria_labelledby_attr.as_deref(),
            aria_describedby_attr.as_deref(),
            &after_roledescription_attrs,
            &[],
            root_svg::SvgRootFixedHeightPlacement::AfterClass,
            false,
        );
    }

    if let (Some(id), Some(title)) = (aria_labelledby_raw.as_deref(), acc_title) {
        out.push_str(r#"<title id=""#);
        super::super::util::escape_attr_into(&mut out, id);
        out.push_str(r#"">"#);
        escape_xml_into(&mut out, title);
        out.push_str("</title>");
    }
    if let (Some(id), Some(descr)) = (aria_describedby_raw.as_deref(), acc_descr) {
        out.push_str(r#"<desc id=""#);
        super::super::util::escape_attr_into(&mut out, id);
        out.push_str(r#"">"#);
        escape_xml_into(&mut out, descr);
        out.push_str("</desc>");
    }
    out.push_str("<style>");
    out.push_str(&css);
    out.push_str("</style>");

    out.push_str("<g>");
    flowchart_markers(&mut out, diagram_id);

    let extra_marker_colors = flowchart_collect_edge_marker_colors(&ctx);
    render_flowchart_root(
        &mut out,
        &ctx,
        None,
        0.0,
        0.0,
        timing_enabled,
        &mut detail,
        Some(&edge_path_cache),
    );

    flowchart_extra_markers(&mut out, diagram_id, &extra_marker_colors);
    out.push_str("</g>");
    if let Some(title) = diagram_title {
        let title_x = title_anchor_x;
        let title_y = -title_top_margin;
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="flowchartTitleText">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            escape_xml(title)
        );
    }
    out.push_str("</svg>\n");

    drop(_g_render_svg);
    timings.total = total_start.elapsed();
    if timing_enabled {
        eprintln!(
            "[render-timing] diagram=flowchart-v2 total={:?} deserialize={:?} build_ctx={:?} viewbox={:?} viewbox_edge_curve_bounds={:?} viewbox_edge_curve_lca={:?} viewbox_edge_curve_offsets={:?} viewbox_edge_curve_geom={:?} viewbox_edge_curve_bbox_union={:?} viewbox_edge_curve_geom_calls={} viewbox_edge_curve_geom_skipped_bounds={} render_svg={:?} finalize={:?} root_calls={} clusters={:?} edges_select={:?} edge_paths={:?} edge_labels={:?} dom_order={:?} nodes={:?} node_style_compile={:?} node_roughjs={:?} node_roughjs_calls={} node_label_html={:?} node_label_html_calls={} nested_roots={:?}",
            timings.total,
            timings.deserialize_model,
            timings.build_ctx,
            timings.viewbox,
            viewbox_edge_curve_bounds,
            detail.viewbox_edge_curve_lca,
            detail.viewbox_edge_curve_offsets,
            detail.viewbox_edge_curve_geom,
            detail.viewbox_edge_curve_bbox_union,
            detail.viewbox_edge_curve_geom_calls,
            detail.viewbox_edge_curve_geom_skipped_bounds,
            timings.render_svg,
            timings.finalize_svg,
            detail.root_calls,
            detail.clusters,
            detail.edges_select,
            detail.edge_paths,
            detail.edge_labels,
            detail.dom_order,
            detail.nodes,
            detail.node_style_compile,
            detail.node_roughjs,
            detail.node_roughjs_calls,
            detail.node_label_html,
            detail.node_label_html_calls,
            detail.nested_roots,
        );
    }
    Ok(out)
}
