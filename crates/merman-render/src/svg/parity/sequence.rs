#![allow(clippy::too_many_arguments)]

use super::*;
use rustc_hash::FxHashMap;

// Sequence SVG renderer implementation (split from parity.rs).

pub(super) fn render_sequence_diagram_debug_svg(
    layout: &SequenceDiagramLayout,
    options: &SvgRenderOptions,
) -> String {
    let mut clusters = layout.clusters.clone();
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut nodes = layout.nodes.clone();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut edges = layout.edges.clone();
    edges.sort_by(|a, b| a.id.cmp(&b.id));

    let bounds = compute_layout_bounds(&clusters, &nodes, &edges).unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let pad = options.viewbox_padding.max(0.0);
    let vb_min_x = bounds.min_x - pad;
    let vb_min_y = bounds.min_y - pad;
    let vb_w = (bounds.max_x - bounds.min_x) + pad * 2.0;
    let vb_h = (bounds.max_y - bounds.min_y) + pad * 2.0;

    let mut out = String::new();
    let _ = writeln!(
        &mut out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}">"#,
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w.max(1.0)),
        fmt(vb_h.max(1.0))
    );
    out.push_str(
        r#"<style>
 .cluster-box { fill: none; stroke: #4b5563; stroke-width: 1; }
 .cluster-title { fill: #111827; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 12px; text-anchor: middle; dominant-baseline: middle; }
 .node-box { fill: none; stroke: #2563eb; stroke-width: 1; }
 .node-label { fill: #1f2937; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 11px; text-anchor: middle; dominant-baseline: middle; }
 .edge { fill: none; stroke: #111827; stroke-width: 1; }
 .lifeline { stroke: #999; stroke-width: 0.5; }
 .message { stroke: #111827; stroke-width: 2; }
 .edge-label { fill: #111827; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 11px; text-anchor: middle; dominant-baseline: middle; }
 .debug-cross { stroke: #ef4444; stroke-width: 1; }
</style>
"#,
    );
    out.push_str(
        r#"<defs><marker id="arrowhead" refX="7.9" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M -1 0 L 10 5 L 0 10 z"/></marker></defs>
"#,
    );

    if options.include_clusters {
        out.push_str(r#"<g class="clusters">"#);
        for c in &clusters {
            render_cluster(&mut out, c, options.include_cluster_debug_markers);
        }
        out.push_str("</g>\n");
    }

    if options.include_edges {
        out.push_str(r#"<g class="edges">"#);
        for e in &edges {
            if e.points.len() >= 2 {
                if e.id.starts_with("lifeline-") && e.points.len() == 2 {
                    let p0 = &e.points[0];
                    let p1 = &e.points[1];
                    let _ = write!(
                        &mut out,
                        r#"<line class="edge lifeline" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
                        fmt(p0.x),
                        fmt(p0.y),
                        fmt(p1.x),
                        fmt(p1.y)
                    );
                } else if e.id.starts_with("msg-") && e.points.len() == 2 {
                    let p0 = &e.points[0];
                    let p1 = &e.points[1];
                    let sign = if p1.x >= p0.x { 1.0 } else { -1.0 };
                    // Layout uses Mermaid-like endpoint offsets (to make arrowheads match later).
                    // For debug output, extend the line to the lifelines so it's easier to read.
                    let x1 = p0.x - sign * 1.0;
                    let x2 = p1.x + sign * 4.0;
                    let _ = write!(
                        &mut out,
                        r#"<line class="edge message" x1="{}" y1="{}" x2="{}" y2="{}" marker-end="url(#arrowhead)" />"#,
                        fmt(x1),
                        fmt(p0.y),
                        fmt(x2),
                        fmt(p1.y)
                    );
                } else {
                    out.push_str(r#"<polyline class="edge" points=""#);
                    for (idx, p) in e.points.iter().enumerate() {
                        if idx > 0 {
                            out.push(' ');
                        }
                        let _ = write!(&mut out, "{},{}", fmt_display(p.x), fmt_display(p.y));
                    }
                    out.push_str(r#"" />"#);
                }
            }
            if options.include_edge_id_labels {
                if let Some(lbl) = &e.label {
                    let _ = write!(
                        &mut out,
                        r#"<text class="edge-label" x="{}" y="{}">{}</text>"#,
                        fmt(lbl.x),
                        fmt(lbl.y),
                        escape_xml_display(&e.id)
                    );
                }
            }
        }
        out.push_str("</g>\n");
    }

    if options.include_nodes {
        out.push_str(r#"<g class="nodes">"#);
        for n in &nodes {
            if n.is_cluster {
                continue;
            }
            render_node(&mut out, n);
        }
        out.push_str("</g>\n");
    }

    out.push_str("</svg>\n");
    out
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceSvgActor {
    description: String,
    #[serde(rename = "type")]
    actor_type: String,
    #[serde(default)]
    wrap: bool,
    #[serde(default)]
    links: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    properties: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceSvgMessage {
    id: String,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(rename = "type")]
    message_type: i32,
    message: serde_json::Value,
    #[serde(default)]
    wrap: bool,
    #[serde(default)]
    activate: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceSvgModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    title: Option<String>,
    #[serde(rename = "actorOrder")]
    actor_order: Vec<String>,
    actors: std::collections::BTreeMap<String, SequenceSvgActor>,
    #[serde(default)]
    boxes: Vec<SequenceSvgBox>,
    messages: Vec<SequenceSvgMessage>,
    #[serde(default)]
    #[allow(dead_code)]
    notes: Vec<SequenceSvgNote>,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceSvgBox {
    #[serde(rename = "actorKeys")]
    actor_keys: Vec<String>,
    fill: String,
    name: Option<String>,
    #[allow(dead_code)]
    wrap: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceSvgNote {
    #[allow(dead_code)]
    actor: serde_json::Value,
    #[allow(dead_code)]
    message: String,
    #[allow(dead_code)]
    placement: i32,
    #[allow(dead_code)]
    wrap: bool,
}

pub(super) fn render_sequence_diagram_svg(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let sanitize_config = merman_core::MermaidConfig::from_value(effective_config.clone());
    render_sequence_diagram_svg_inner(
        layout,
        semantic,
        effective_config,
        &sanitize_config,
        _diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_sequence_diagram_svg_with_config(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    _diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_sequence_diagram_svg_inner(
        layout,
        semantic,
        effective_config.as_value(),
        effective_config,
        _diagram_title,
        measurer,
        options,
    )
}

fn render_sequence_diagram_svg_inner(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    sanitize_config: &merman_core::MermaidConfig,
    _diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: SequenceSvgModel = crate::json::from_value_ref(semantic)?;

    let seq_cfg = effective_config
        .get("sequence")
        .unwrap_or(&serde_json::Value::Null);
    let force_menus = seq_cfg
        .get("forceMenus")
        .or_else(|| effective_config.get("forceMenus"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mirror_actors = seq_cfg
        .get("mirrorActors")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let diagram_margin_x = seq_cfg
        .get("diagramMarginX")
        .and_then(|v| v.as_f64())
        .unwrap_or(50.0)
        .max(0.0);
    let box_margin = seq_cfg
        .get("boxMargin")
        .and_then(|v| v.as_f64())
        .unwrap_or(10.0)
        .max(0.0);
    let actor_height = seq_cfg
        .get("height")
        .and_then(|v| v.as_f64())
        .unwrap_or(65.0)
        .max(1.0);
    let box_text_margin = seq_cfg
        .get("boxTextMargin")
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0)
        .max(0.0);
    let _message_margin = seq_cfg
        .get("messageMargin")
        .and_then(|v| v.as_f64())
        .unwrap_or(35.0)
        .max(0.0);
    let _bottom_margin_adj = seq_cfg
        .get("bottomMarginAdj")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let label_box_height = seq_cfg
        .get("labelBoxHeight")
        .and_then(|v| v.as_f64())
        .unwrap_or(20.0)
        .max(0.0);
    let right_angles = seq_cfg
        .get("rightAngles")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let wrap_padding = seq_cfg
        .get("wrapPadding")
        .and_then(|v| v.as_f64())
        .unwrap_or(10.0)
        .max(0.0);
    let sequence_width = seq_cfg
        .get("width")
        .and_then(|v| v.as_f64())
        .unwrap_or(150.0)
        .max(1.0);
    let actor_label_font_size = seq_cfg
        .get("messageFontSize")
        .and_then(|v| v.as_f64())
        .or_else(|| effective_config.get("fontSize").and_then(|v| v.as_f64()))
        .unwrap_or(16.0)
        .max(1.0);
    let loop_text_style = TextStyle {
        font_family: effective_config
            .get("fontFamily")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        font_size: actor_label_font_size,
        font_weight: Some("400".to_string()),
    };
    let note_text_style = TextStyle {
        font_family: loop_text_style.font_family.clone(),
        font_size: actor_label_font_size,
        font_weight: Some("400".to_string()),
    };
    let actor_wrap_width = (sequence_width - 2.0 * wrap_padding).max(1.0);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    // Upstream Mermaid viewports are driven by browser layout pipelines and often land on an `f32`
    // lattice (e.g. `...49998474121094`). Mirror that by quantizing the extrema to `f32` first,
    // then computing width/height in `f32` space.
    let min_x_f32 = bounds.min_x as f32;
    let min_y_f32 = bounds.min_y as f32;
    let max_x_f32 = bounds.max_x as f32;
    let max_y_f32 = bounds.max_y as f32;

    let vb_min_x = min_x_f32 as f64;
    let vb_min_y = min_y_f32 as f64;
    let vb_w = ((max_x_f32 - min_x_f32).max(1.0)) as f64;
    let vb_h = ((max_y_f32 - min_y_f32).max(1.0)) as f64;

    let mut nodes_by_id: FxHashMap<&str, &LayoutNode> =
        FxHashMap::with_capacity_and_hasher(layout.nodes.len(), Default::default());
    for n in &layout.nodes {
        nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut edges_by_id: FxHashMap<&str, &crate::model::LayoutEdge> =
        FxHashMap::with_capacity_and_hasher(layout.edges.len(), Default::default());
    for e in &layout.edges {
        edges_by_id.insert(e.id.as_str(), e);
    }

    fn node_left_top(n: &LayoutNode) -> (f64, f64) {
        (n.x - n.width / 2.0, n.y - n.height / 2.0)
    }
    fn write_actor_label(
        out: &mut String,
        cx: f64,
        cy: f64,
        label: &str,
        wrap: bool,
        wrap_width_px: f64,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) {
        // Split/wrap before decoding Mermaid entities so escaped `<br>` (`#lt;br#gt;`) remains
        // literal text rather than being treated as an actual `<br>` break.
        let raw_lines: Vec<String> = if wrap {
            crate::text::wrap_label_like_mermaid_lines(label, measurer, style, wrap_width_px)
        } else {
            crate::text::split_html_br_lines(label)
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        };
        let n = raw_lines.len().max(1) as f64;
        for (i, raw) in raw_lines.into_iter().enumerate() {
            let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(&raw);
            let dy = if n <= 1.0 {
                0.0
            } else {
                (i as f64 - (n - 1.0) / 2.0) * style.font_size
            };
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor actor-box" style="text-anchor: middle; font-size: {fs}px; font-weight: 400;"><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
                x = fmt(cx),
                y = fmt(cy),
                fs = fmt(style.font_size),
                dy = fmt(dy),
                text = escape_xml_display(decoded.as_ref())
            );
        }
    }

    let mut out = String::new();
    let aria = match (model.acc_title.as_deref(), model.acc_descr.as_deref()) {
        (Some(_), Some(_)) => format!(
            r#" aria-describedby="chart-desc-{id}" aria-labelledby="chart-title-{id}""#,
            id = diagram_id_esc
        ),
        (Some(_), None) => format!(
            r#" aria-labelledby="chart-title-{id}""#,
            id = diagram_id_esc
        ),
        (None, Some(_)) => format!(
            r#" aria-describedby="chart-desc-{id}""#,
            id = diagram_id_esc
        ),
        (None, None) => String::new(),
    };
    let mut max_w_attr = fmt_string(vb_w);
    let mut viewbox_attr = format!(
        "{} {} {} {}",
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w),
        fmt(vb_h)
    );
    if let Some((viewbox, max_w)) =
        crate::generated::sequence_root_overrides_11_12_2::lookup_sequence_root_viewport_override(
            diagram_id,
        )
    {
        viewbox_attr = viewbox.to_string();
        max_w_attr = max_w.to_string();
    }

    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{viewbox}" role="graphics-document document" aria-roledescription="sequence"{aria}>"#,
        diagram_id_esc = diagram_id_esc,
        max_w = max_w_attr,
        viewbox = viewbox_attr,
        aria = aria
    );

    if let Some(title) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml_display(title)
        );
    }
    if let Some(desc) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml_display(desc)
        );
    }

    // Mermaid renders "box" frames as root-level `<g><rect class="rect"/>...</g>` nodes before actors.
    // Mermaid renders boxes "behind" other elements; multiple boxes end up reversed in DOM order.
    let has_box_titles = model
        .boxes
        .iter()
        .any(|b| b.name.as_deref().is_some_and(|s| !s.trim().is_empty()));
    let max_box_title_height = if has_box_titles {
        // Mermaid uses `utils.calculateTextDimensions(...).height` for box titles.
        // With 16px fonts this ends up as 17px, and is used for the actor `starty` bump.
        let line_h = (actor_label_font_size * (17.0 / 16.0)).max(1.0);
        model
            .boxes
            .iter()
            .filter_map(|b| b.name.as_deref())
            .map(|s| crate::text::split_html_br_lines(s).len().max(1) as f64 * line_h)
            .fold(0.0, f64::max)
    } else {
        0.0
    };

    for b in model.boxes.iter().rev() {
        let pad_x = (box_margin * 2.0 + box_text_margin).max(0.0);
        let pad_top = (box_margin + box_text_margin + max_box_title_height).max(0.0);
        let pad_bottom = (box_margin * 2.0).max(0.0);

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_top_y = f64::INFINITY;
        let mut max_bottom_y = f64::NEG_INFINITY;

        for actor_key in &b.actor_keys {
            let top_id = format!("actor-top-{actor_key}");
            let bottom_id = format!("actor-bottom-{actor_key}");
            let Some(top) = nodes_by_id.get(top_id.as_str()).copied() else {
                continue;
            };
            let Some(bottom) = nodes_by_id.get(bottom_id.as_str()).copied() else {
                continue;
            };

            let (top_x, top_y) = node_left_top(top);
            min_x = min_x.min(top_x);
            max_x = max_x.max(top_x + top.width);
            min_top_y = min_top_y.min(top_y);

            let (_bottom_x, bottom_y) = node_left_top(bottom);
            max_bottom_y = max_bottom_y.max(bottom_y + bottom.height);
        }

        if !min_x.is_finite()
            || !max_x.is_finite()
            || !min_top_y.is_finite()
            || !max_bottom_y.is_finite()
        {
            continue;
        }

        let x = min_x - pad_x;
        let w = (max_x - min_x) + pad_x * 2.0;
        let y = min_top_y - pad_top;
        let h = (max_bottom_y - min_top_y) + pad_top + pad_bottom;

        out.push_str("<g>");
        let _ = write!(
            &mut out,
            r#"<rect x="{x}" y="{y}" fill="{fill}" stroke="rgb(0,0,0, 0.5)" width="{w}" height="{h}" class="rect"/>"#,
            x = fmt(x),
            y = fmt(y),
            w = fmt(w),
            h = fmt(h),
            fill = escape_xml_display(&b.fill),
        );
        if let Some(name) = b.name.as_deref() {
            let cx = x + (w / 2.0);
            // Mermaid's `drawBox(...)` places the title at `box.y + boxTextMargin + textMaxHeight/2`.
            // In upstream, `box.y` is the `verticalPos` passed to `addActorRenderingData`, i.e. 0.
            let box_y = min_top_y - (box_margin + max_box_title_height);
            let text_y = box_y + box_text_margin + max_box_title_height / 2.0;
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="text" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{x}" dy="0">{text}</tspan></text>"#,
                x = fmt(cx),
                y = fmt(text_y),
                text = escape_xml_display(name)
            );
        }
        out.push_str("</g>");
    }

    // Mermaid renders `rect` blocks as root-level `<rect class="rect"/>` nodes before actors.
    {
        #[derive(Debug, Clone, Copy)]
        struct RectBlock<'a> {
            fill: &'a str,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
        }

        fn contains(a: &RectBlock<'_>, b: &RectBlock<'_>) -> bool {
            const EPS: f64 = 1e-9;
            a.x <= b.x + EPS
                && a.y <= b.y + EPS
                && (a.x + a.w) >= (b.x + b.w) - EPS
                && (a.y + a.h) >= (b.y + b.h) - EPS
        }

        let mut rects: Vec<RectBlock<'_>> = Vec::new();
        for msg in &model.messages {
            if msg.message_type != 22 {
                continue;
            }
            let fill = msg.message.as_str().unwrap_or_default();
            let node_id = format!("rect-{}", msg.id);
            let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
                continue;
            };
            let (x, y) = node_left_top(n);
            rects.push(RectBlock {
                fill,
                x,
                y,
                w: n.width,
                h: n.height,
            });
        }

        // Mermaid's emitted order for nested `rect` blocks is not strictly tied to parse order.
        // Match its DOM ordering semantics by keeping parents before contained children and
        // sorting unrelated rectangles by vertical position (lower blocks first).
        rects.sort_by(|a, b| {
            if contains(a, b) && !contains(b, a) {
                return std::cmp::Ordering::Less;
            }
            if contains(b, a) && !contains(a, b) {
                return std::cmp::Ordering::Greater;
            }
            b.y.partial_cmp(&a.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });

        for r in rects {
            let _ = write!(
                &mut out,
                r#"<rect x="{x}" y="{y}" fill="{fill}" width="{w}" height="{h}" class="rect"/>"#,
                x = fmt(r.x),
                y = fmt(r.y),
                w = fmt(r.w),
                h = fmt(r.h),
                fill = escape_xml_display(r.fill)
            );
        }
    }

    if mirror_actors {
        // Mermaid draws bottom actors first (reverse DOM order).
        for (idx, actor_id) in model.actor_order.iter().enumerate().rev() {
            let Some(actor) = model.actors.get(actor_id) else {
                continue;
            };
            let actor_type = actor.actor_type.as_str();
            let node_id = format!("actor-bottom-{actor_id}");
            let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
                continue;
            };
            let (x, y) = node_left_top(n);
            let actor_custom_class = actor
                .properties
                .get("class")
                .and_then(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());
            let actor_rect_fill = if actor_custom_class.is_some() {
                "#EDF2AE"
            } else {
                "#eaeaea"
            };
            let actor_bottom_class = actor_custom_class
                .map(|c| format!("{c} actor-bottom"))
                .unwrap_or_else(|| "actor actor-bottom".to_string());
            match actor_type {
                // Actor-man variants are drawn later (after `<defs>`), but Mermaid keeps stable
                // indices by emitting empty `<g/>` placeholders here.
                "actor" | "boundary" | "control" | "entity" => {
                    out.push_str("<g/>");
                }
                "collections" => {
                    const OFFSET: f64 = 6.0;
                    let front_x = x - OFFSET;
                    let front_y = y + OFFSET;
                    let cx = front_x + (n.width / 2.0);
                    let cy = front_y + (n.height / 2.0);
                    out.push_str("<g>");
                    let _ = write!(
                        &mut out,
                        r##"<rect x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor actor-bottom"/>"##,
                        x = fmt(x),
                        y = fmt(y),
                        w = fmt(n.width),
                        h = fmt(n.height),
                        name = escape_xml_display(actor_id)
                    );
                    let _ = write!(
                        &mut out,
                        r##"<rect x="{sx}" y="{sy}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor"/>"##,
                        sx = fmt(front_x),
                        sy = fmt(front_y),
                        w = fmt(n.width),
                        h = fmt(n.height),
                        name = escape_xml_display(actor_id)
                    );
                    write_actor_label(
                        &mut out,
                        cx,
                        cy,
                        &actor.description,
                        actor.wrap,
                        actor_wrap_width,
                        measurer,
                        &loop_text_style,
                    );
                    out.push_str("</g>");
                }
                "queue" => {
                    let ry = n.height / 2.0;
                    let rx = ry / (2.5 + n.height / 50.0);
                    let body_w = n.width - 2.0 * rx;
                    let y_mid = y + ry;
                    out.push_str("<g>");
                    let _ = write!(
                        &mut out,
                        r##"<g transform="translate({tx1}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h} h {body_w} a {rx},{ry} 0 0 0 0,-{h} Z" class="actor actor-bottom"/></g>"##,
                        tx1 = fmt(rx),
                        ty = fmt(-n.height / 2.0),
                        x = fmt(x),
                        y_mid = fmt(y_mid),
                        rx = fmt(rx),
                        ry = fmt(ry),
                        h = fmt(n.height),
                        body_w = fmt(body_w)
                    );
                    let _ = write!(
                        &mut out,
                        r##"<g transform="translate({tx2}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h}" stroke="#666" stroke-width="1px" class="actor actor-bottom"/></g>"##,
                        tx2 = fmt(n.width - rx),
                        ty = fmt(-n.height / 2.0),
                        x = fmt(x),
                        y_mid = fmt(y_mid),
                        rx = fmt(rx),
                        ry = fmt(ry),
                        h = fmt(n.height)
                    );
                    write_actor_label(
                        &mut out,
                        n.x,
                        y_mid,
                        &actor.description,
                        actor.wrap,
                        actor_wrap_width,
                        measurer,
                        &loop_text_style,
                    );
                    out.push_str("</g>");
                }
                "database" => {
                    // Mermaid's database actor uses a cylinder glyph and updates the actor height after
                    // the top render; the footer render uses that updated height (≈ width/4 + labelBoxHeight).
                    let w = n.width / 4.0;
                    let h = n.width / 4.0;
                    let rx = w / 2.0;
                    let ry = rx / (2.5 + w / 50.0);
                    let footer_h = h + label_box_height;
                    let tx = w * 1.5;
                    let ty = (footer_h / 4.0) - 2.0 * ry;
                    let y_text = y + ((footer_h + h) / 4.0) + (footer_h / 2.0);
                    out.push_str("<g>");
                    let _ = write!(
                        &mut out,
                        r##"<g transform="translate({tx}, {ty})"><path d="M {x},{y1} a {rx},{ry} 0 0 0 {w},0 a {rx},{ry} 0 0 0 -{w},0 l 0,{h2} a {rx},{ry} 0 0 0 {w},0 l 0,-{h2}" fill="#eaeaea" stroke="#000" stroke-width="1" class="actor actor-bottom"/></g>"##,
                        tx = fmt(tx),
                        ty = fmt(ty),
                        x = fmt(x),
                        y1 = fmt(y + ry),
                        rx = fmt(rx),
                        ry = fmt(ry),
                        w = fmt(w),
                        h2 = fmt(h - 2.0 * ry)
                    );
                    write_actor_label(
                        &mut out,
                        n.x,
                        y_text,
                        &actor.description,
                        actor.wrap,
                        actor_wrap_width,
                        measurer,
                        &loop_text_style,
                    );
                    out.push_str("</g>");
                }
                _ => {
                    out.push_str("<g>");
                    let _ = write!(
                        &mut out,
                        r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" name="{name}" rx="3" ry="3" class="{class}"/>"##,
                        x = fmt(x),
                        y = fmt(y),
                        w = fmt(n.width),
                        h = fmt(n.height),
                        name = escape_xml(actor_id),
                        fill = escape_xml_display(actor_rect_fill),
                        class = escape_attr(&actor_bottom_class),
                    );
                    write_actor_label(
                        &mut out,
                        n.x,
                        n.y,
                        &actor.description,
                        actor.wrap,
                        actor_wrap_width,
                        measurer,
                        &loop_text_style,
                    );
                    out.push_str("</g>");
                }
            }

            let _ = idx;
        }
    }

    // Top actors + lifelines.
    for (idx, actor_id) in model.actor_order.iter().enumerate().rev() {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        let node_top_id = format!("actor-top-{actor_id}");
        let node_bottom_id = format!("actor-bottom-{actor_id}");
        let Some(top) = nodes_by_id.get(node_top_id.as_str()).copied() else {
            continue;
        };
        let Some(bottom) = nodes_by_id.get(node_bottom_id.as_str()).copied() else {
            continue;
        };
        let (top_x, top_y) = node_left_top(top);
        let (bottom_x, bottom_y) = node_left_top(bottom);
        let _ = bottom_x;

        let (y1, y2) = edges_by_id
            .get(format!("lifeline-{actor_id}").as_str())
            .and_then(|e| Some((e.points.first()?.y, e.points.get(1)?.y)))
            .unwrap_or((top_y + top.height, bottom_y));
        let actor_custom_class = actor
            .properties
            .get("class")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let actor_rect_fill = if actor_custom_class.is_some() {
            "#EDF2AE"
        } else {
            "#eaeaea"
        };
        let actor_top_class = actor_custom_class
            .map(|c| format!("{c} actor-top"))
            .unwrap_or_else(|| "actor actor-top".to_string());

        match actor_type {
            "actor" | "boundary" | "control" | "entity" => {
                let _ = write!(
                    &mut out,
                    r##"<g><line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/></g>"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id)
                );
            }
            "collections" => {
                const OFFSET: f64 = 6.0;
                let front_x = top_x - OFFSET;
                let front_y = top_y + OFFSET;
                let cx = front_x + (top.width / 2.0);
                let cy = front_y + (top.height / 2.0);
                out.push_str("<g>");
                let _ = write!(
                    &mut out,
                    r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    &mut out,
                    r##"<rect x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor actor-top"/>"##,
                    x = fmt(top_x),
                    y = fmt(top_y),
                    w = fmt(top.width),
                    h = fmt(top.height),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    &mut out,
                    r##"<rect x="{sx}" y="{sy}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor"/>"##,
                    sx = fmt(front_x),
                    sy = fmt(front_y),
                    w = fmt(top.width),
                    h = fmt(top.height),
                    name = escape_xml(actor_id),
                );
                write_actor_label(
                    &mut out,
                    cx,
                    cy,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    &loop_text_style,
                );
                out.push_str("</g></g>");
            }
            "queue" => {
                let ry = top.height / 2.0;
                let rx = ry / (2.5 + top.height / 50.0);
                let body_w = top.width - 2.0 * rx;
                let y_mid = top_y + ry;
                out.push_str("<g>");
                let _ = write!(
                    &mut out,
                    r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    &mut out,
                    r##"<g transform="translate({tx1}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h} h {body_w} a {rx},{ry} 0 0 0 0,-{h} Z" class="actor actor-top"/></g>"##,
                    tx1 = fmt(rx),
                    ty = fmt(-top.height / 2.0),
                    x = fmt(top_x),
                    y_mid = fmt(y_mid),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    h = fmt(top.height),
                    body_w = fmt(body_w),
                );
                let _ = write!(
                    &mut out,
                    r##"<g transform="translate({tx2}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h}" stroke="#666" stroke-width="1px" class="actor actor-top"/></g>"##,
                    tx2 = fmt(top.width - rx),
                    ty = fmt(-top.height / 2.0),
                    x = fmt(top_x),
                    y_mid = fmt(y_mid),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    h = fmt(top.height),
                );
                write_actor_label(
                    &mut out,
                    top.x,
                    y_mid,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    &loop_text_style,
                );
                out.push_str("</g></g>");
            }
            "database" => {
                let w = top.width / 4.0;
                let h = top.width / 4.0;
                let rx = w / 2.0;
                let ry = rx / (2.5 + w / 50.0);
                let tx = w * 1.5;
                let ty = (actor_height + ry) / 4.0;
                let y_text = top_y + actor_height + (ry / 2.0);
                out.push_str("<g>");
                let _ = write!(
                    &mut out,
                    r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    &mut out,
                    r##"<g transform="translate({tx}, {ty})"><path d="M {x},{y1p} a {rx},{ry} 0 0 0 {w},0 a {rx},{ry} 0 0 0 -{w},0 l 0,{h2} a {rx},{ry} 0 0 0 {w},0 l 0,-{h2}" fill="#eaeaea" stroke="#000" stroke-width="1" class="actor actor-top"/></g>"##,
                    tx = fmt(tx),
                    ty = fmt(ty),
                    x = fmt(top_x),
                    y1p = fmt(top_y + ry),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    w = fmt(w),
                    h2 = fmt(h - 2.0 * ry),
                );
                write_actor_label(
                    &mut out,
                    top.x,
                    y_text,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    &loop_text_style,
                );
                out.push_str("</g></g>");
            }
            _ => {
                out.push_str("<g>");
                let _ = write!(
                    &mut out,
                    r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    &mut out,
                    r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" name="{name}" rx="3" ry="3" class="{class}"/>"##,
                    x = fmt(top_x),
                    y = fmt(top_y),
                    w = fmt(top.width),
                    h = fmt(top.height),
                    name = escape_xml(actor_id),
                    fill = escape_xml_display(actor_rect_fill),
                    class = escape_attr(&actor_top_class),
                );
                write_actor_label(
                    &mut out,
                    top.x,
                    top.y,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    &loop_text_style,
                );
                out.push_str("</g></g>");
            }
        }
    }

    let _ = write!(
        &mut out,
        r#"<style>{}</style><g/>"#,
        sequence_css(diagram_id)
    );

    // Mermaid's sequence output includes a shared set of <defs> for icons/markers.
    out.push_str(MERMAID_SEQUENCE_BASE_DEFS_11_12_2);

    // Actor-man variants (actor/boundary/control/entity) are emitted after `<defs>`.
    for (actor_idx, actor_id) in model.actor_order.iter().enumerate() {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        if !matches!(actor_type, "actor" | "boundary" | "control" | "entity") {
            continue;
        }
        let node_id = format!("actor-top-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        let (_x, actor_y) = node_left_top(n);
        let cx = n.x;

        match actor_type {
            "actor" => {
                let r = 15.0;
                let cy = actor_y + 10.0;
                let torso_top = cy + r;
                let torso_bottom = torso_top + 20.0;
                let arms_y = torso_top + 8.0;
                let arms_x1 = cx - 18.0;
                let arms_x2 = cx + 18.0;
                let leg_y = torso_bottom + 15.0;
                let _ = write!(
                    &mut out,
                    r##"<g class="actor-man actor-top" name="{name}"><line id="actor-man-torso{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}"/><line id="actor-man-arms{idx}" x1="{ax1}" y1="{ay}" x2="{ax2}" y2="{ay}"/><line x1="{ax1}" y1="{ly}" x2="{cx}" y2="{y2}"/><line x1="{cx}" y1="{y2}" x2="{lx2}" y2="{ly}"/><circle cx="{cx}" cy="{cy}" r="15" width="{w}" height="{h}"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    idx = actor_idx,
                    cx = fmt(cx),
                    y1 = fmt(torso_top),
                    y2 = fmt(torso_bottom),
                    ax1 = fmt(arms_x1),
                    ax2 = fmt(arms_x2),
                    ay = fmt(arms_y),
                    ly = fmt(leg_y),
                    lx2 = fmt(cx + 16.0),
                    cy = fmt(cy),
                    w = fmt(n.width),
                    h = fmt(actor_height),
                    ty = fmt(actor_y + actor_height + 2.5),
                    label = escape_xml(&actor.description)
                );
            }
            "boundary" => {
                let radius = 30.0;
                let x_left = cx - radius * 2.5;
                let last_idx = model.actor_order.len().saturating_sub(1);
                let _ = last_idx;
                let _ = write!(
                    &mut out,
                    r##"<g class="actor-man actor-top" name="{name}" transform="translate(0,22)"><line id="actor-man-torso{idx}" x1="{x1}" y1="{y_t}" x2="{x2}" y2="{y_t}"/><line id="actor-man-arms{idx}" x1="{x1}" y1="{y0}" x2="{x1}" y2="{y20}"/><circle cx="{cx}" cy="{cy}" r="30"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    idx = actor_idx,
                    x1 = fmt(x_left),
                    x2 = fmt(cx - 15.0),
                    y_t = fmt(actor_y + 10.0),
                    y0 = fmt(actor_y + 0.0),
                    y20 = fmt(actor_y + 20.0),
                    cx = fmt(cx),
                    cy = fmt(actor_y + 10.0),
                    // drawTextCandidate adds rect.height/2. Top render uses the config height.
                    ty = fmt(actor_y + (radius / 2.0 + 3.0) + (actor_height / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            "control" => {
                let r = 18.0;
                let cy = actor_y + 30.0;
                let _ = write!(
                    &mut out,
                    r##"<g class="actor-man actor-top" name="{name}"><defs><marker id="filled-head-control" refX="11" refY="5.8" markerWidth="20" markerHeight="28" orient="172.5"><path d="M 14.4 5.6 L 7.2 10.4 L 8.8 5.6 L 7.2 0.8 Z"/></marker></defs><circle cx="{cx}" cy="{cy}" r="18" fill="#eaeaf7" stroke="#666" stroke-width="1.2"/><line marker-end="url(#filled-head-control)" transform="translate({cx}, {ly})"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    cx = fmt(cx),
                    cy = fmt(cy),
                    ly = fmt(cy - r),
                    ty = fmt(actor_y + (r + 10.0) + (actor_height / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            "entity" => {
                let r = 18.0;
                let cy = actor_y + 25.0;
                let _ = write!(
                    &mut out,
                    r##"<g class="actor-man actor-top" name="{name}" transform="translate(0, 9)"><circle cx="{cx}" cy="{cy}" r="18" width="{w}" height="{h}"/><line x1="{x1}" x2="{x2}" y1="{y}" y2="{y}" stroke="#333" stroke-width="2"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    cx = fmt(cx),
                    cy = fmt(cy),
                    w = fmt(n.width),
                    h = fmt(actor_height),
                    x1 = fmt(cx - r),
                    x2 = fmt(cx + r),
                    y = fmt(cy + r),
                    ty = fmt(actor_y + ((cy + r - actor_y) / 2.0) + (actor_height / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            _ => {}
        }
    }

    // Mermaid draws activation boxes by creating an anchored `<g>` at ACTIVE_START and inserting the
    // `<rect class="activation{0..2}">` when the corresponding ACTIVE_END is encountered.
    //
    // Important DOM detail: if an activation is started but never closed, Mermaid still creates the
    // anchored `<g/>` but never inserts a `<rect>`. Preserve that behavior for DOM parity.
    #[derive(Debug, Clone)]
    struct SequenceActivationStart {
        startx: f64,
        starty: f64,
        start_index: usize,
        group_index: usize,
    }

    #[derive(Debug, Clone)]
    struct SequenceActivationRect {
        startx: f64,
        starty: f64,
        width: f64,
        height: f64,
        class_idx: usize,
        #[allow(dead_code)]
        start_index: usize,
    }

    fn actor_center_x(nodes_by_id: &FxHashMap<&str, &LayoutNode>, actor_id: &str) -> Option<f64> {
        let node_id = format!("actor-top-{actor_id}");
        nodes_by_id.get(node_id.as_str()).copied().map(|n| n.x)
    }

    fn lifeline_y(
        edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
        actor_id: &str,
    ) -> Option<(f64, f64)> {
        let edge_id = format!("lifeline-{actor_id}");
        let e = edges_by_id.get(edge_id.as_str()).copied()?;
        let y0 = e.points.first()?.y;
        let y1 = e.points.last()?.y;
        Some((y0, y1))
    }

    let activation_width = seq_cfg
        .get("activationWidth")
        .and_then(|v| v.as_f64())
        .unwrap_or(10.0)
        .max(1.0);
    let activation_fill = effective_config
        .get("themeVariables")
        .and_then(|v| {
            v.get("activationBkgColor")
                .or_else(|| v.get("noteBkgColor"))
        })
        .and_then(|v| v.as_str())
        .unwrap_or("#EDF2AE");
    let activation_stroke = effective_config
        .get("themeVariables")
        .and_then(|v| {
            v.get("activationBorderColor")
                .or_else(|| v.get("noteBorderColor"))
        })
        .and_then(|v| v.as_str())
        .unwrap_or("#666");

    let mut last_line_y: Option<f64> = None;
    let mut activation_counter: usize = 0;
    let mut activation_stacks: std::collections::BTreeMap<String, Vec<SequenceActivationStart>> =
        std::collections::BTreeMap::new();
    let mut activation_groups: Vec<Option<SequenceActivationRect>> = Vec::new();

    // Mermaid creates activation placeholders at ACTIVE_START and inserts the `<rect>` once the
    // corresponding ACTIVE_END is encountered. We store the final rect geometry during this
    // first pass and remember which message id should emit which activation group.
    let mut activation_group_by_start_id: FxHashMap<String, usize> =
        FxHashMap::with_capacity_and_hasher(model.messages.len(), Default::default());

    for msg in &model.messages {
        if let Some(y) = msg_line_y(&edges_by_id, &msg.id) {
            last_line_y = Some(y);
        }

        match msg.message_type {
            // ACTIVE_START
            17 => {
                let Some(actor_id) = msg.from.as_deref() else {
                    continue;
                };
                let Some(cx) = actor_center_x(&nodes_by_id, actor_id) else {
                    continue;
                };
                let has_any_activation = !activation_stacks.is_empty();
                let stack = activation_stacks.entry(actor_id.to_string()).or_default();
                let stacked_size = stack.len();
                let startx = cx + (((stacked_size as f64) - 1.0) * activation_width) / 2.0;

                let starty = last_line_y
                    .or_else(|| lifeline_y(&edges_by_id, actor_id).map(|(y0, _y1)| y0))
                    .unwrap_or(0.0);
                let starty = if last_line_y.is_some() && has_any_activation {
                    starty + 2.0
                } else {
                    starty
                };

                let group_index = activation_groups.len();
                activation_groups.push(None);
                activation_group_by_start_id.insert(msg.id.clone(), group_index);
                stack.push(SequenceActivationStart {
                    startx,
                    starty,
                    start_index: activation_counter,
                    group_index,
                });
                activation_counter += 1;
            }
            // ACTIVE_END
            18 => {
                let Some(actor_id) = msg.from.as_deref() else {
                    continue;
                };
                let Some(stack) = activation_stacks.get_mut(actor_id) else {
                    continue;
                };
                let Some(start) = stack.pop() else {
                    continue;
                };

                let mut starty = start.starty;
                let mut vertical_pos = last_line_y.unwrap_or(starty);
                if starty + 18.0 > vertical_pos {
                    starty = vertical_pos - 6.0;
                    vertical_pos += 12.0;
                }

                let class_idx = stack.len() % 3;
                let rect = SequenceActivationRect {
                    startx: start.startx,
                    starty,
                    width: activation_width,
                    height: (vertical_pos - starty).max(0.0),
                    class_idx,
                    start_index: start.start_index,
                };
                if let Some(slot) = activation_groups.get_mut(start.group_index) {
                    *slot = Some(rect);
                }
            }
            _ => {}
        }

        let _ = msg.activate;
    }

    #[derive(Debug, Clone)]
    struct AltSection {
        raw_label: String,
        message_ids: Vec<String>,
    }

    #[derive(Debug, Clone)]
    enum SequenceBlock {
        Alt {
            sections: Vec<AltSection>,
        },
        Opt {
            raw_label: String,
            message_ids: Vec<String>,
        },
        Break {
            raw_label: String,
            message_ids: Vec<String>,
        },
        Par {
            sections: Vec<AltSection>,
        },
        Loop {
            raw_label: String,
            message_ids: Vec<String>,
        },
        Critical {
            sections: Vec<AltSection>,
        },
    }

    fn bracketize(s: &str) -> String {
        let t = s.trim();
        if t.is_empty() {
            return "\u{200B}".to_string();
        }
        if t.starts_with('[') && t.ends_with(']') {
            return t.to_string();
        }
        format!("[{t}]")
    }

    fn split_line_to_words(text: &str) -> Vec<String> {
        let parts = text.split(' ').collect::<Vec<_>>();
        let mut out: Vec<String> = Vec::new();
        for part in parts {
            if !part.is_empty() {
                out.push(part.to_string());
            }
            out.push(" ".to_string());
        }
        while out.last().is_some_and(|s| s == " ") {
            out.pop();
        }
        out
    }

    fn wrap_svg_text_line(
        line: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width: f64,
    ) -> Vec<String> {
        use std::collections::VecDeque;

        if !max_width.is_finite() || max_width <= 0.0 {
            return vec![line.to_string()];
        }

        // Mermaid's frame-label wrapping behaves as if the available width were slightly smaller
        // than the raw `frame_x2 - (frame_x1 + label_box_width)` span, especially for narrow
        // (single-actor-ish) frames. Apply a small pad only in that regime to avoid over-wrapping
        // wide frames like `critical` headers.
        let pad = if max_width <= 160.0 {
            15.0
        } else if max_width <= 230.0 {
            8.0
        } else {
            0.0
        };
        let max_width = (max_width - pad).max(1.0);

        fn svg_bbox_width_px(measurer: &dyn TextMeasurer, style: &TextStyle, text: &str) -> f64 {
            let (l, r) = measurer.measure_svg_text_bbox_x(text, style);
            (l + r).max(0.0)
        }

        let mut tokens = VecDeque::from(split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut force_break_after_next_non_space: bool = false;

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            if svg_bbox_width_px(measurer, style, &candidate) <= max_width {
                cur = candidate;
                if force_break_after_next_non_space && tok != " " {
                    out.push(cur.trim_end().to_string());
                    cur.clear();
                    force_break_after_next_non_space = false;
                }
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

            // `tok` itself does not fit on an empty line; split by characters.
            let chars = tok.chars().collect::<Vec<_>>();
            let mut cut = 1usize;
            while cut < chars.len() {
                let mut head: String = chars[..cut].iter().collect();
                let tail_len = chars.len().saturating_sub(cut);
                let should_hyphenate = tail_len > 0
                    && !head.ends_with('-')
                    && head
                        .chars()
                        .last()
                        .is_some_and(|ch| ch.is_ascii_alphanumeric());
                if should_hyphenate {
                    head.push('-');
                }
                if svg_bbox_width_px(measurer, style, &head) > max_width {
                    break;
                }
                cut += 1;
            }
            cut = cut.saturating_sub(1).max(1);
            let mut head: String = chars[..cut].iter().collect();
            let tail: String = chars[cut..].iter().collect();
            let mut hyphenated = false;
            if !tail.is_empty()
                && !head.ends_with('-')
                && head
                    .chars()
                    .last()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric())
                && svg_bbox_width_px(measurer, style, &(head.clone() + "-")) <= max_width
            {
                head.push('-');
                hyphenated = true;
            }
            out.push(head);
            if !tail.is_empty() {
                tokens.push_front(tail);
                if hyphenated {
                    force_break_after_next_non_space = true;
                }
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

    fn wrap_svg_text_lines(
        text: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width: Option<f64>,
    ) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for line in crate::text::split_html_br_lines(text) {
            if let Some(w) = max_width {
                lines.extend(wrap_svg_text_line(line, measurer, style, w));
            } else {
                lines.push(line.to_string());
            }
        }
        if lines.is_empty() {
            vec!["".to_string()]
        } else {
            lines
        }
    }

    fn write_loop_text_lines(
        out: &mut String,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        x: f64,
        y0: f64,
        max_width: Option<f64>,
        text: &str,
        use_tspan: bool,
    ) {
        let line_step = style.font_size * 1.1875;
        let lines = wrap_svg_text_lines(text, measurer, style, max_width);
        for (i, line) in lines.into_iter().enumerate() {
            let y = y0 + (i as f64) * line_step;
            if use_tspan {
                let _ = write!(
                    out,
                    r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="font-size: {fs}px; font-weight: 400;"><tspan x="{x}">{text}</tspan></text>"#,
                    x = fmt(x),
                    y = fmt(y),
                    fs = fmt(style.font_size),
                    text = escape_xml(&line)
                );
            } else {
                let _ = write!(
                    out,
                    r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="font-size: {fs}px; font-weight: 400;">{text}</text>"#,
                    x = fmt(x),
                    y = fmt(y),
                    fs = fmt(style.font_size),
                    text = escape_xml(&line)
                );
            }
        }
    }

    fn frame_x_from_actors(
        model: &SequenceSvgModel,
        nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    ) -> Option<(f64, f64)> {
        const SIDE_PAD: f64 = 11.0;
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        for actor_id in &model.actor_order {
            let node_id = format!("actor-top-{actor_id}");
            let n = nodes_by_id.get(node_id.as_str()).copied()?;
            min_x = min_x.min(n.x);
            max_x = max_x.max(n.x);
        }
        if !min_x.is_finite() || !max_x.is_finite() {
            return None;
        }
        Some((min_x - SIDE_PAD, max_x + SIDE_PAD))
    }

    fn msg_line_y(
        edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
        msg_id: &str,
    ) -> Option<f64> {
        let edge_id = format!("msg-{msg_id}");
        let e = edges_by_id.get(edge_id.as_str()).copied()?;
        Some(e.points.first()?.y)
    }

    fn msg_y_range_with_self_extra(
        edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
        msg_endpoints: &FxHashMap<&str, (&str, &str)>,
        msg_id: &str,
        self_extra_y: f64,
    ) -> Option<(f64, f64)> {
        let y = msg_line_y(edges_by_id, msg_id)?;
        let extra = msg_endpoints
            .get(msg_id)
            .copied()
            .filter(|(from, to)| from == to)
            .map(|_| self_extra_y)
            .unwrap_or(0.0);
        Some((y, y + extra))
    }

    fn msg_y_range_for_frame(
        edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
        msg_endpoints: &FxHashMap<&str, (&str, &str)>,
        msg_id: &str,
    ) -> Option<(f64, f64)> {
        // Mermaid's `boundMessage(...)` self-message branch expands the inserted bounds by 60px
        // below `lineStartY` (see the `+ 30 + totalOffset` bottom coordinate, where `totalOffset`
        // already includes a `+30` bump).
        const SELF_MESSAGE_EXTRA_Y: f64 = 60.0;
        msg_y_range_with_self_extra(edges_by_id, msg_endpoints, msg_id, SELF_MESSAGE_EXTRA_Y)
    }

    fn msg_y_range_for_separators(
        edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
        msg_endpoints: &FxHashMap<&str, (&str, &str)>,
        msg_id: &str,
    ) -> Option<(f64, f64)> {
        // The self-message loop curve itself extends ~30px below the message line.
        // Mermaid's dashed section separators follow the curve geometry, not the full `bounds.insert(...)`
        // envelope used for frame sizing.
        const SELF_MESSAGE_EXTRA_Y: f64 = 30.0;
        msg_y_range_with_self_extra(edges_by_id, msg_endpoints, msg_id, SELF_MESSAGE_EXTRA_Y)
    }

    // Mermaid renders block frames (`alt`, `loop`, ...) as `<g>` elements before message lines.
    // Use layout-derived message y-coordinates for separator placement to avoid visual artifacts
    // like dashed lines ending in a gap right before the frame border.
    let mut blocks_by_end_id: FxHashMap<String, Vec<usize>> =
        FxHashMap::with_capacity_and_hasher(model.messages.len(), Default::default());
    let mut blocks: Vec<SequenceBlock> = Vec::new();

    #[derive(Debug, Clone)]
    enum BlockStackEntry {
        Alt {
            raw_labels: Vec<String>,
            sections: Vec<Vec<String>>,
        },
        Loop {
            raw_label: String,
            messages: Vec<String>,
        },
        Opt {
            raw_label: String,
            messages: Vec<String>,
        },
        Break {
            raw_label: String,
            messages: Vec<String>,
        },
        Par {
            raw_labels: Vec<String>,
            sections: Vec<Vec<String>>,
        },
        Critical {
            raw_labels: Vec<String>,
            sections: Vec<Vec<String>>,
        },
    }

    let mut stack: Vec<BlockStackEntry> = Vec::new();
    for msg in &model.messages {
        let raw_label = msg.message.as_str().unwrap_or_default();
        match msg.message_type {
            // notes
            2 => {
                // Notes inside blocks must contribute to block frame bounds and section separators.
                // Track them in the active block scopes, similar to message edges.
                for entry in stack.iter_mut() {
                    match entry {
                        BlockStackEntry::Alt { sections, .. }
                        | BlockStackEntry::Par { sections, .. }
                        | BlockStackEntry::Critical { sections, .. } => {
                            if let Some(cur) = sections.last_mut() {
                                cur.push(msg.id.clone());
                            }
                        }
                        BlockStackEntry::Loop { messages, .. }
                        | BlockStackEntry::Opt { messages, .. }
                        | BlockStackEntry::Break { messages, .. } => {
                            messages.push(msg.id.clone());
                        }
                    }
                }
                continue;
            }
            // loop start/end
            10 => stack.push(BlockStackEntry::Loop {
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            11 => {
                if let Some(BlockStackEntry::Loop {
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Loop {
                        raw_label,
                        message_ids: messages,
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            // opt start/end
            15 => stack.push(BlockStackEntry::Opt {
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            16 => {
                if let Some(BlockStackEntry::Opt {
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Opt {
                        raw_label,
                        message_ids: messages,
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            // break start/end
            30 => stack.push(BlockStackEntry::Break {
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            31 => {
                if let Some(BlockStackEntry::Break {
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Break {
                        raw_label,
                        message_ids: messages,
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            // alt start/else/end
            12 => stack.push(BlockStackEntry::Alt {
                raw_labels: vec![raw_label.to_string()],
                sections: vec![Vec::new()],
            }),
            13 => {
                if let Some(BlockStackEntry::Alt {
                    raw_labels,
                    sections,
                }) = stack.last_mut()
                {
                    raw_labels.push(raw_label.to_string());
                    sections.push(Vec::new());
                }
            }
            14 => {
                if let Some(BlockStackEntry::Alt {
                    raw_labels,
                    sections,
                }) = stack.pop()
                {
                    let mut out_sections = Vec::new();
                    for (i, raw_label) in raw_labels.into_iter().enumerate() {
                        let message_ids = sections.get(i).cloned().unwrap_or_default();
                        out_sections.push(AltSection {
                            raw_label,
                            message_ids,
                        });
                    }
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Alt {
                        sections: out_sections,
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            // par start/and/end
            19 | 32 => stack.push(BlockStackEntry::Par {
                raw_labels: vec![raw_label.to_string()],
                sections: vec![Vec::new()],
            }),
            20 => {
                if let Some(BlockStackEntry::Par {
                    raw_labels,
                    sections,
                }) = stack.last_mut()
                {
                    raw_labels.push(raw_label.to_string());
                    sections.push(Vec::new());
                }
            }
            21 => {
                if let Some(BlockStackEntry::Par {
                    raw_labels,
                    sections,
                }) = stack.pop()
                {
                    let mut out_sections = Vec::new();
                    for (i, raw_label) in raw_labels.into_iter().enumerate() {
                        let message_ids = sections.get(i).cloned().unwrap_or_default();
                        out_sections.push(AltSection {
                            raw_label,
                            message_ids,
                        });
                    }
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Par {
                        sections: out_sections,
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            // critical start/option/end
            27 => stack.push(BlockStackEntry::Critical {
                raw_labels: vec![raw_label.to_string()],
                sections: vec![Vec::new()],
            }),
            28 => {
                if let Some(BlockStackEntry::Critical {
                    raw_labels,
                    sections,
                }) = stack.last_mut()
                {
                    raw_labels.push(raw_label.to_string());
                    sections.push(Vec::new());
                }
            }
            29 => {
                if let Some(BlockStackEntry::Critical {
                    raw_labels,
                    sections,
                }) = stack.pop()
                {
                    let mut out_sections = Vec::new();
                    for (i, raw_label) in raw_labels.into_iter().enumerate() {
                        let message_ids = sections.get(i).cloned().unwrap_or_default();
                        out_sections.push(AltSection {
                            raw_label,
                            message_ids,
                        });
                    }
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Critical {
                        sections: out_sections,
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            _ => {
                // If this is a "real" message edge, attach it to all active block scopes.
                if msg.from.is_some() && msg.to.is_some() {
                    for entry in stack.iter_mut() {
                        match entry {
                            BlockStackEntry::Alt { sections, .. }
                            | BlockStackEntry::Par { sections, .. }
                            | BlockStackEntry::Critical { sections, .. } => {
                                if let Some(cur) = sections.last_mut() {
                                    cur.push(msg.id.clone());
                                }
                            }
                            BlockStackEntry::Loop { messages, .. } => {
                                messages.push(msg.id.clone());
                            }
                            BlockStackEntry::Opt { messages, .. } => {
                                messages.push(msg.id.clone());
                            }
                            BlockStackEntry::Break { messages, .. } => {
                                messages.push(msg.id.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some((_frame_x1, _frame_x2)) = frame_x_from_actors(&model, &nodes_by_id) {
        fn display_block_label(raw_label: &str, always_show: bool) -> Option<String> {
            let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(raw_label);
            let t = decoded.as_ref().trim();
            if t.is_empty() {
                if always_show {
                    // Mermaid renders empty block labels as a zero-width space inside `<tspan>`.
                    Some("\u{200B}".to_string())
                } else {
                    None
                }
            } else {
                Some(bracketize(t))
            }
        }

        let mut actor_nodes_by_id: FxHashMap<&str, &LayoutNode> =
            FxHashMap::with_capacity_and_hasher(model.actors.len(), Default::default());
        for actor_id in &model.actor_order {
            let node_id = format!("actor-top-{actor_id}");
            let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
                continue;
            };
            actor_nodes_by_id.insert(actor_id.as_str(), n);
        }

        let mut msg_endpoints: FxHashMap<&str, (&str, &str)> =
            FxHashMap::with_capacity_and_hasher(model.messages.len(), Default::default());
        for msg in &model.messages {
            let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
                continue;
            };
            msg_endpoints.insert(msg.id.as_str(), (from, to));
        }

        fn frame_x_from_message_ids<'a>(
            message_ids: impl IntoIterator<Item = &'a String>,
            msg_endpoints: &FxHashMap<&str, (&str, &str)>,
            actor_nodes_by_id: &FxHashMap<&str, &LayoutNode>,
            edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
            nodes_by_id: &FxHashMap<&str, &LayoutNode>,
        ) -> Option<(f64, f64, f64)> {
            const SIDE_PAD: f64 = 11.0;
            const GEOM_PAD: f64 = 10.0;
            // For single-actor frames containing only self-messages, upstream Mermaid expands the
            // frame to cover at least the actor box width (plus a small asymmetric pad that leaves
            // room for the self-arrow loop on the right). Our deterministic layout edge points can
            // be too narrow for short self-message labels, which would over-wrap frame titles.
            const SELF_ONLY_FRAME_MIN_PAD_LEFT: f64 = 5.0;
            const SELF_ONLY_FRAME_MIN_PAD_RIGHT: f64 = 15.0;
            let mut min_left = f64::INFINITY;
            let mut geom_min_x = f64::INFINITY;
            let mut geom_max_x = f64::NEG_INFINITY;
            let mut min_cx = f64::INFINITY;
            let mut max_cx = f64::NEG_INFINITY;
            let mut self_only_actor: Option<&str> = None;

            for msg_id in message_ids {
                // Notes are nodes (not edges); include their bounding boxes in frame extents.
                let note_node_id = format!("note-{msg_id}");
                if let Some(n) = nodes_by_id.get(note_node_id.as_str()).copied() {
                    geom_min_x = geom_min_x.min(n.x - n.width / 2.0 - GEOM_PAD);
                    geom_max_x = geom_max_x.max(n.x + n.width / 2.0 + GEOM_PAD);
                }

                let Some((from, to)) = msg_endpoints.get(msg_id.as_str()).copied() else {
                    continue;
                };
                if from == to {
                    self_only_actor = match self_only_actor {
                        None => Some(from),
                        Some(prev) if prev == from => Some(prev),
                        _ => Some(""),
                    };
                } else {
                    self_only_actor = Some("");
                }

                // Expand frames to cover message geometry and label overflow (especially important
                // for single-actor blocks containing long self-message labels).
                let edge_id = format!("msg-{msg_id}");
                if let Some(e) = edges_by_id.get(edge_id.as_str()).copied() {
                    for p in &e.points {
                        geom_min_x = geom_min_x.min(p.x);
                        geom_max_x = geom_max_x.max(p.x);
                    }
                    if let Some(label) = e.label.as_ref() {
                        geom_min_x = geom_min_x.min(label.x - (label.width / 2.0) - GEOM_PAD);
                        geom_max_x = geom_max_x.max(label.x + (label.width / 2.0) + GEOM_PAD);
                    }
                }
                for actor_id in [from, to] {
                    let Some(n) = actor_nodes_by_id.get(actor_id).copied() else {
                        continue;
                    };
                    min_cx = min_cx.min(n.x);
                    max_cx = max_cx.max(n.x);
                    min_left = min_left.min(n.x - n.width / 2.0);
                }
            }

            if !min_cx.is_finite() || !max_cx.is_finite() {
                return None;
            }
            let mut x1 = min_cx - SIDE_PAD;
            let mut x2 = max_cx + SIDE_PAD;
            if geom_min_x.is_finite() {
                x1 = x1.min(geom_min_x);
            }
            if geom_max_x.is_finite() {
                x2 = x2.max(geom_max_x);
            }
            if matches!(self_only_actor, Some(a) if !a.is_empty()) {
                if let Some(n) = actor_nodes_by_id.get(self_only_actor.unwrap()).copied() {
                    let left = n.x - n.width / 2.0;
                    let right = n.x + n.width / 2.0;
                    let min_x1 = left - SELF_ONLY_FRAME_MIN_PAD_LEFT;
                    let min_x2 = right + SELF_ONLY_FRAME_MIN_PAD_RIGHT;
                    // Only widen when the computed geometry is suspiciously narrow; avoid shifting
                    // frames that already match upstream due to message label geometry.
                    if (x2 - x1) < (min_x2 - min_x1) - 1.0 {
                        x1 = x1.min(min_x1);
                        x2 = x2.max(min_x2);
                    }
                }
            }
            Some((x1, x2, min_left))
        }

        fn item_y_range(
            edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
            nodes_by_id: &FxHashMap<&str, &LayoutNode>,
            msg_endpoints: &FxHashMap<&str, (&str, &str)>,
            item_id: &str,
            is_separator: bool,
        ) -> Option<(f64, f64)> {
            let msg_range = if is_separator {
                msg_y_range_for_separators(edges_by_id, msg_endpoints, item_id)
            } else {
                msg_y_range_for_frame(edges_by_id, msg_endpoints, item_id)
            };
            if let Some((y0, y1)) = msg_range {
                return Some((y0, y1));
            }
            let note_node_id = format!("note-{item_id}");
            let n = nodes_by_id.get(note_node_id.as_str()).copied()?;
            let top = n.y - n.height / 2.0;
            let bottom = n.y + n.height / 2.0;
            Some((top, bottom))
        }

        for msg in &model.messages {
            if msg.message_type == 2 {
                let id = &msg.id;
                let raw = msg.message.as_str().unwrap_or_default();
                let node_id = format!("note-{id}");
                let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
                    continue;
                };
                let (x, y) = node_left_top(n);
                let cx = x + (n.width / 2.0);
                let text_y = y + 5.0;
                let line_step = actor_label_font_size * 1.1875;
                out.push_str(r#"<g>"#);
                let _ = write!(
                    &mut out,
                    r##"<rect x="{x}" y="{y}" fill="#EDF2AE" stroke="#666" width="{w}" height="{h}" class="note"/>"##,
                    x = fmt(x),
                    y = fmt(y),
                    w = fmt(n.width),
                    h = fmt(n.height)
                );
                let lines: Vec<String> = if msg.wrap {
                    // Mermaid@11.12.2 (Sequence) wraps notes *after* placement width is known:
                    //   noteModel.message = wrapLabel(msg.message, noteModel.width - 2*wrapPadding, noteFont)
                    //
                    // Layout already computed the note box width (`n.width`) to match Mermaid's
                    // `noteModel.width`, so wrap to `n.width - 2*wrapPadding` here.
                    let wrap_w = (n.width - 2.0 * wrap_padding).max(1.0);
                    crate::text::wrap_label_like_mermaid_lines_floored_bbox(
                        raw,
                        measurer,
                        &note_text_style,
                        wrap_w,
                    )
                } else {
                    crate::text::split_html_br_lines(raw)
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect()
                };
                for (i, line) in lines.iter().enumerate() {
                    let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(line);
                    let y = text_y + (i as f64) * line_step;
                    let _ = write!(
                        &mut out,
                        r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="noteText" dy="1em" style="font-size: {fs}px; font-weight: 400;"><tspan x="{x}">{text}</tspan></text>"#,
                        x = fmt(cx),
                        y = fmt(y),
                        fs = fmt(actor_label_font_size),
                        text = escape_xml(decoded.as_ref())
                    );
                }
                out.push_str("</g>");
            }

            if let Some(group_index) = activation_group_by_start_id.get(&msg.id).copied() {
                // Mermaid creates a `<g>` placeholder at ACTIVE_START time and inserts the
                // `<rect class="activation{0..2}">` once ACTIVE_END is encountered.
                out.push_str("<g>");
                if let Some(Some(a)) = activation_groups.get(group_index) {
                    let _ = write!(
                        &mut out,
                        r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="{stroke}" width="{w}" height="{h}" class="activation{idx}"/>"##,
                        x = fmt(a.startx),
                        y = fmt(a.starty),
                        w = fmt(a.width),
                        h = fmt(a.height),
                        idx = a.class_idx,
                        fill = escape_xml(activation_fill),
                        stroke = escape_xml(activation_stroke),
                    );
                }
                out.push_str("</g>");
            }

            let Some(idxs) = blocks_by_end_id.get(&msg.id) else {
                continue;
            };
            for idx in idxs {
                let Some(block) = blocks.get(*idx) else {
                    continue;
                };
                match block {
                    SequenceBlock::Alt { sections } => {
                        if sections.is_empty() {
                            continue;
                        }

                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for sec in sections {
                            for msg_id in &sec.message_ids {
                                if let Some((y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    false,
                                ) {
                                    min_y = min_y.min(y0);
                                    max_y = max_y.max(y1);
                                }
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            sections.iter().flat_map(|s| s.message_ids.iter()),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        let header_offset = if sections
                            .first()
                            .is_some_and(|s| s.raw_label.trim().is_empty())
                        {
                            (79.0 - label_box_height).max(0.0)
                        } else {
                            // When the critical label wraps, Mermaid increases the header height so the
                            // frame starts higher (see upstream `adjustLoopHeightForWrap(...)`).
                            let base = 79.0;
                            let label_box_right = frame_x1 + 50.0;
                            let max_w = (frame_x2 - label_box_right).max(0.0);
                            let label = display_block_label(&sections[0].raw_label, true)
                                .unwrap_or_else(|| "\u{200B}".to_string());
                            let wrapped = wrap_svg_text_lines(
                                &label,
                                measurer,
                                &loop_text_style,
                                Some(max_w),
                            );
                            let extra_lines = wrapped.len().saturating_sub(1) as f64;
                            let extra_per_line =
                                (loop_text_style.font_size * 1.1875 - box_text_margin).max(0.0);
                            base + extra_lines * extra_per_line
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);

                        // frame
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y1}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x2}" y1="{y1}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y2}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x1}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );

                        // separators (dashed)
                        // Keep separator endpoints identical to the frame endpoints to match upstream
                        // Mermaid output and avoid sub-pixel gaps at the frame border.
                        let dash_x1 = frame_x1;
                        let dash_x2 = frame_x2;
                        let mut section_max_ys: Vec<f64> = Vec::new();
                        for sec in sections {
                            let mut sec_max_y = f64::NEG_INFINITY;
                            for msg_id in &sec.message_ids {
                                if let Some((_y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    true,
                                ) {
                                    sec_max_y = sec_max_y.max(y1);
                                }
                            }
                            if !sec_max_y.is_finite() {
                                sec_max_y = min_y;
                            }
                            section_max_ys.push(sec_max_y);
                        }
                        let mut sep_ys: Vec<f64> = Vec::new();
                        for sec_max_y in section_max_ys
                            .iter()
                            .take(section_max_ys.len().saturating_sub(1))
                        {
                            sep_ys.push(*sec_max_y + 15.0);
                        }
                        for y in &sep_ys {
                            let _ = write!(
                                &mut out,
                                r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
                                x1 = fmt(dash_x1),
                                x2 = fmt(dash_x2),
                                y = fmt(*y)
                            );
                        }

                        // label box + label text
                        // This matches Mermaid's label-box shape: a 50px-wide header with a 8.4px cut.
                        let x1 = frame_x1;
                        let y1 = frame_y1;
                        let x2 = x1 + 50.0;
                        let y2 = y1 + 13.0;
                        let y3 = y1 + 20.0;
                        let x3 = x2 - 8.4;
                        let _ = write!(
                            &mut out,
                            r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
                            x1 = fmt(x1),
                            y1 = fmt(y1),
                            x2 = fmt(x2),
                            y2 = fmt(y2),
                            x3 = fmt(x3),
                            y3 = fmt(y3)
                        );
                        let label_cx = (x1 + 25.0).round();
                        let label_cy = y1 + 13.0;
                        let _ = write!(
                            &mut out,
                            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="font-size: 16px; font-weight: 400;">alt</text>"#,
                            x = fmt(label_cx),
                            y = fmt(label_cy)
                        );

                        // section labels
                        let label_box_right = frame_x1 + 50.0;
                        let main_text_x = (label_box_right + frame_x2) / 2.0;
                        let center_text_x = (frame_x1 + frame_x2) / 2.0;
                        for (i, sec) in sections.iter().enumerate() {
                            let Some(label_text) = display_block_label(&sec.raw_label, i == 0)
                            else {
                                continue;
                            };
                            if i == 0 {
                                let y = frame_y1 + 18.0;
                                let max_w = (frame_x2 - label_box_right).max(0.0);
                                write_loop_text_lines(
                                    &mut out,
                                    measurer,
                                    &loop_text_style,
                                    main_text_x,
                                    y,
                                    Some(max_w),
                                    &label_text,
                                    true,
                                );
                                continue;
                            }
                            let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                            write_loop_text_lines(
                                &mut out,
                                measurer,
                                &loop_text_style,
                                center_text_x,
                                y,
                                None,
                                &label_text,
                                false,
                            );
                        }

                        out.push_str("</g>");
                    }
                    SequenceBlock::Par { sections } => {
                        if sections.is_empty() {
                            continue;
                        }

                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for sec in sections {
                            for msg_id in &sec.message_ids {
                                if let Some((y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    false,
                                ) {
                                    min_y = min_y.min(y0);
                                    max_y = max_y.max(y1);
                                }
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            sections.iter().flat_map(|s| s.message_ids.iter()),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        let header_offset = if sections
                            .first()
                            .is_some_and(|s| s.raw_label.trim().is_empty())
                        {
                            (79.0 - label_box_height).max(0.0)
                        } else {
                            79.0
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);

                        // frame
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y1}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x2}" y1="{y1}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y2}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x1}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );

                        // separators (dashed)
                        let dash_x1 = frame_x1;
                        let dash_x2 = frame_x2;
                        let mut section_max_ys: Vec<f64> = Vec::new();
                        for sec in sections {
                            let mut sec_max_y = f64::NEG_INFINITY;
                            for msg_id in &sec.message_ids {
                                if let Some((_y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    true,
                                ) {
                                    sec_max_y = sec_max_y.max(y1);
                                }
                            }
                            if !sec_max_y.is_finite() {
                                sec_max_y = min_y;
                            }
                            section_max_ys.push(sec_max_y);
                        }
                        let mut sep_ys: Vec<f64> = Vec::new();
                        for sec_max_y in section_max_ys
                            .iter()
                            .take(section_max_ys.len().saturating_sub(1))
                        {
                            sep_ys.push(*sec_max_y + 15.0);
                        }
                        for y in &sep_ys {
                            let _ = write!(
                                &mut out,
                                r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
                                x1 = fmt(dash_x1),
                                x2 = fmt(dash_x2),
                                y = fmt(*y)
                            );
                        }

                        // label box + label text
                        let x1 = frame_x1;
                        let y1 = frame_y1;
                        let x2 = x1 + 50.0;
                        let y2 = y1 + 13.0;
                        let y3 = y1 + 20.0;
                        let x3 = x2 - 8.4;
                        let _ = write!(
                            &mut out,
                            r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
                            x1 = fmt(x1),
                            y1 = fmt(y1),
                            x2 = fmt(x2),
                            y2 = fmt(y2),
                            x3 = fmt(x3),
                            y3 = fmt(y3)
                        );
                        let label_cx = (x1 + 25.0).round();
                        let label_cy = y1 + 13.0;
                        let _ = write!(
                            &mut out,
                            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="font-size: 16px; font-weight: 400;">par</text>"#,
                            x = fmt(label_cx),
                            y = fmt(label_cy)
                        );

                        // section labels
                        let label_box_right = frame_x1 + 50.0;
                        let main_text_x = (label_box_right + frame_x2) / 2.0;
                        let center_text_x = (frame_x1 + frame_x2) / 2.0;
                        for (i, sec) in sections.iter().enumerate() {
                            let Some(label_text) = display_block_label(&sec.raw_label, i == 0)
                            else {
                                continue;
                            };
                            if i == 0 {
                                let y = frame_y1 + 18.0;
                                let max_w = (frame_x2 - label_box_right).max(0.0);
                                write_loop_text_lines(
                                    &mut out,
                                    measurer,
                                    &loop_text_style,
                                    main_text_x,
                                    y,
                                    Some(max_w),
                                    &label_text,
                                    true,
                                );
                                continue;
                            }
                            let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                            write_loop_text_lines(
                                &mut out,
                                measurer,
                                &loop_text_style,
                                center_text_x,
                                y,
                                None,
                                &label_text,
                                false,
                            );
                        }

                        out.push_str("</g>");
                    }
                    SequenceBlock::Loop {
                        raw_label,
                        message_ids,
                    } => {
                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for msg_id in message_ids {
                            if let Some((y0, y1)) = item_y_range(
                                &edges_by_id,
                                &nodes_by_id,
                                &msg_endpoints,
                                msg_id,
                                false,
                            ) {
                                min_y = min_y.min(y0);
                                max_y = max_y.max(y1);
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            message_ids.iter(),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        // Mermaid draws the loop frame far enough above the first message line to
                        // leave room for the header label box + label text.
                        let header_offset = if raw_label.trim().is_empty() {
                            (79.0 - label_box_height).max(0.0)
                        } else {
                            79.0
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y1}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x2}" y1="{y1}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y2}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x1}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let x1 = frame_x1;
                        let y1 = frame_y1;
                        let x2 = x1 + 50.0;
                        let y2 = y1 + 13.0;
                        let y3 = y1 + 20.0;
                        let x3 = x2 - 8.4;
                        let _ = write!(
                            &mut out,
                            r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
                            x1 = fmt(x1),
                            y1 = fmt(y1),
                            x2 = fmt(x2),
                            y2 = fmt(y2),
                            x3 = fmt(x3),
                            y3 = fmt(y3)
                        );
                        let label_cx = (x1 + 25.0).round();
                        let label_cy = y1 + 13.0;
                        let _ = write!(
                            &mut out,
                            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="font-size: 16px; font-weight: 400;">loop</text>"#,
                            x = fmt(label_cx),
                            y = fmt(label_cy)
                        );
                        let label_box_right = frame_x1 + 50.0;
                        let text_x = (label_box_right + frame_x2) / 2.0;
                        let text_y = frame_y1 + 18.0;
                        let label = display_block_label(raw_label, true)
                            .unwrap_or_else(|| "\u{200B}".to_string());
                        let max_w = (frame_x2 - label_box_right).max(0.0);
                        write_loop_text_lines(
                            &mut out,
                            measurer,
                            &loop_text_style,
                            text_x,
                            text_y,
                            Some(max_w),
                            &label,
                            true,
                        );
                        out.push_str("</g>");
                    }
                    SequenceBlock::Opt {
                        raw_label,
                        message_ids,
                    } => {
                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for msg_id in message_ids {
                            if let Some((y0, y1)) = item_y_range(
                                &edges_by_id,
                                &nodes_by_id,
                                &msg_endpoints,
                                msg_id,
                                false,
                            ) {
                                min_y = min_y.min(y0);
                                max_y = max_y.max(y1);
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            message_ids.iter(),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        let header_offset = if raw_label.trim().is_empty() {
                            (79.0 - label_box_height).max(0.0)
                        } else {
                            79.0
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y1}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x2}" y1="{y1}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y2}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x1}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let x1 = frame_x1;
                        let y1 = frame_y1;
                        let x2 = x1 + 50.0;
                        let y2 = y1 + 13.0;
                        let y3 = y1 + 20.0;
                        let x3 = x2 - 8.4;
                        let _ = write!(
                            &mut out,
                            r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
                            x1 = fmt(x1),
                            y1 = fmt(y1),
                            x2 = fmt(x2),
                            y2 = fmt(y2),
                            x3 = fmt(x3),
                            y3 = fmt(y3)
                        );
                        let label_cx = (x1 + 25.0).round();
                        let label_cy = y1 + 13.0;
                        let _ = write!(
                            &mut out,
                            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="font-size: 16px; font-weight: 400;">opt</text>"#,
                            x = fmt(label_cx),
                            y = fmt(label_cy)
                        );
                        let label_box_right = frame_x1 + 50.0;
                        let text_x = (label_box_right + frame_x2) / 2.0;
                        let text_y = frame_y1 + 18.0;
                        let label = display_block_label(raw_label, true)
                            .unwrap_or_else(|| "\u{200B}".to_string());
                        let max_w = (frame_x2 - label_box_right).max(0.0);
                        write_loop_text_lines(
                            &mut out,
                            measurer,
                            &loop_text_style,
                            text_x,
                            text_y,
                            Some(max_w),
                            &label,
                            true,
                        );
                        out.push_str("</g>");
                    }
                    SequenceBlock::Break {
                        raw_label,
                        message_ids,
                    } => {
                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for msg_id in message_ids {
                            if let Some((y0, y1)) = item_y_range(
                                &edges_by_id,
                                &nodes_by_id,
                                &msg_endpoints,
                                msg_id,
                                false,
                            ) {
                                min_y = min_y.min(y0);
                                max_y = max_y.max(y1);
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            message_ids.iter(),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        let frame_y1 = min_y - 93.0;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y1}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x2}" y1="{y1}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y2}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x1}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let x1 = frame_x1;
                        let y1 = frame_y1;
                        let x2 = x1 + 50.0;
                        let y2 = y1 + 13.0;
                        let y3 = y1 + 20.0;
                        let x3 = x2 - 8.4;
                        let _ = write!(
                            &mut out,
                            r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
                            x1 = fmt(x1),
                            y1 = fmt(y1),
                            x2 = fmt(x2),
                            y2 = fmt(y2),
                            x3 = fmt(x3),
                            y3 = fmt(y3)
                        );
                        let label_cx = (x1 + 25.0).round();
                        let label_cy = y1 + 13.0;
                        let _ = write!(
                            &mut out,
                            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="font-size: 16px; font-weight: 400;">break</text>"#,
                            x = fmt(label_cx),
                            y = fmt(label_cy)
                        );
                        let label_box_right = frame_x1 + 50.0;
                        let text_x = (label_box_right + frame_x2) / 2.0;
                        let text_y = frame_y1 + 18.0;
                        let label = display_block_label(raw_label, true)
                            .unwrap_or_else(|| "\u{200B}".to_string());
                        let max_w = (frame_x2 - label_box_right).max(0.0);
                        write_loop_text_lines(
                            &mut out,
                            measurer,
                            &loop_text_style,
                            text_x,
                            text_y,
                            Some(max_w),
                            &label,
                            true,
                        );
                        out.push_str("</g>");
                    }
                    SequenceBlock::Critical { sections } => {
                        if sections.is_empty() {
                            continue;
                        }

                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for sec in sections {
                            for msg_id in &sec.message_ids {
                                if let Some((y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    false,
                                ) {
                                    min_y = min_y.min(y0);
                                    max_y = max_y.max(y1);
                                }
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (mut frame_x1, frame_x2, min_left) = frame_x_from_message_ids(
                            sections.iter().flat_map(|s| s.message_ids.iter()),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));
                        if sections.len() > 1 && min_left.is_finite() {
                            // Mermaid's `critical` w/ `option` sections widens the frame to the left.
                            frame_x1 = frame_x1.min(min_left - 9.0);
                        }

                        let header_offset = if sections
                            .first()
                            .is_some_and(|s| s.raw_label.trim().is_empty())
                        {
                            (79.0 - label_box_height).max(0.0)
                        } else if sections.len() > 1 {
                            // Mermaid does not apply the wrap height adjustment for multi-section
                            // `critical` blocks (those with one or more `option` sections).
                            79.0
                        } else {
                            // Mermaid's `adjustLoopHeightForWrap(...)` expands the header height when the
                            // section label wraps to multiple lines. This affects the frame's top y.
                            let label_text = display_block_label(&sections[0].raw_label, true)
                                .unwrap_or_else(|| "\u{200B}".to_string());
                            let label_box_right = frame_x1 + 50.0;
                            let max_w = (frame_x2 - label_box_right).max(0.0);
                            let wrapped = wrap_svg_text_lines(
                                &label_text,
                                measurer,
                                &loop_text_style,
                                Some(max_w),
                            );
                            let extra_lines = wrapped.len().saturating_sub(1) as f64;
                            let extra_per_line =
                                (loop_text_style.font_size * 1.1875 - box_text_margin).max(0.0);
                            79.0 + extra_lines * extra_per_line
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);

                        // frame
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y1}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x2}" y1="{y1}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x2 = fmt(frame_x2),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y2}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            x2 = fmt(frame_x2),
                            y2 = fmt(frame_y2)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<line x1="{x1}" y1="{y1}" x2="{x1}" y2="{y2}" class="loopLine"/>"#,
                            x1 = fmt(frame_x1),
                            y1 = fmt(frame_y1),
                            y2 = fmt(frame_y2)
                        );

                        // separators (dashed)
                        let dash_x1 = frame_x1;
                        let dash_x2 = frame_x2;
                        let mut section_max_ys: Vec<f64> = Vec::new();
                        for sec in sections {
                            let mut sec_max_y = f64::NEG_INFINITY;
                            for msg_id in &sec.message_ids {
                                if let Some((_y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    true,
                                ) {
                                    sec_max_y = sec_max_y.max(y1);
                                }
                            }
                            if !sec_max_y.is_finite() {
                                sec_max_y = min_y;
                            }
                            section_max_ys.push(sec_max_y);
                        }
                        let mut sep_ys: Vec<f64> = Vec::new();
                        for sec_max_y in section_max_ys
                            .iter()
                            .take(section_max_ys.len().saturating_sub(1))
                        {
                            sep_ys.push(*sec_max_y + 15.0);
                        }
                        for y in &sep_ys {
                            let _ = write!(
                                &mut out,
                                r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
                                x1 = fmt(dash_x1),
                                x2 = fmt(dash_x2),
                                y = fmt(*y)
                            );
                        }

                        // label box + label text
                        let x1 = frame_x1;
                        let y1 = frame_y1;
                        let x2 = x1 + 50.0;
                        let y2 = y1 + 13.0;
                        let y3 = y1 + 20.0;
                        let x3 = x2 - 8.4;
                        let _ = write!(
                            &mut out,
                            r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
                            x1 = fmt(x1),
                            y1 = fmt(y1),
                            x2 = fmt(x2),
                            y2 = fmt(y2),
                            x3 = fmt(x3),
                            y3 = fmt(y3)
                        );
                        let label_cx = (x1 + 25.0).round();
                        let label_cy = y1 + 13.0;
                        let _ = write!(
                            &mut out,
                            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="font-size: 16px; font-weight: 400;">critical</text>"#,
                            x = fmt(label_cx),
                            y = fmt(label_cy)
                        );

                        // section labels
                        let label_box_right = frame_x1 + 50.0;
                        let main_text_x = (label_box_right + frame_x2) / 2.0;
                        let center_text_x = (frame_x1 + frame_x2) / 2.0;
                        for (i, sec) in sections.iter().enumerate() {
                            let Some(label_text) = display_block_label(&sec.raw_label, i == 0)
                            else {
                                continue;
                            };
                            if i == 0 {
                                let y = frame_y1 + 18.0;
                                let max_w = (frame_x2 - label_box_right).max(0.0);
                                write_loop_text_lines(
                                    &mut out,
                                    measurer,
                                    &loop_text_style,
                                    main_text_x,
                                    y,
                                    Some(max_w),
                                    &label_text,
                                    true,
                                );
                                continue;
                            }
                            let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                            write_loop_text_lines(
                                &mut out,
                                measurer,
                                &loop_text_style,
                                center_text_x,
                                y,
                                None,
                                &label_text,
                                false,
                            );
                        }

                        out.push_str("</g>");
                    }
                }
            }
        }
    }

    let mut sequence_number_visible = false;
    let mut sequence_number: i64 = 1;
    let mut sequence_number_step: i64 = 1;

    for msg in &model.messages {
        match msg.message_type {
            // AUTONUMBER
            26 => {
                let obj = msg.message.as_object();
                if let Some(visible) = obj.and_then(|o| o.get("visible")).and_then(|v| v.as_bool())
                {
                    sequence_number_visible = visible;
                } else {
                    sequence_number_visible = true;
                }
                if let Some(start) = obj
                    .and_then(|o| o.get("start"))
                    .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
                {
                    sequence_number = start;
                }
                if let Some(step) = obj
                    .and_then(|o| o.get("step"))
                    .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
                {
                    sequence_number_step = step;
                }
                continue;
            }
            // NOTE
            2 => continue,
            _ => {}
        }

        let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
            continue;
        };
        let edge_id = format!("msg-{}", msg.id);
        let Some(edge) = edges_by_id.get(edge_id.as_str()).copied() else {
            continue;
        };
        if edge.points.len() < 2 {
            continue;
        }

        let text = msg.message.as_str().unwrap_or_default();
        if let Some(lbl) = &edge.label {
            let line_step = actor_label_font_size * 1.1875;
            let bounded_width = (edge.points[0].x - edge.points[1].x).abs().max(0.0);
            let raw_lines: Vec<String> = if msg.wrap && !text.is_empty() {
                // Mermaid's `wrapLabel(...)` uses DOM-backed SVG text bbox widths. Our headless
                // vendored metrics are close but can be slightly more conservative in some edge
                // cases; give message wrapping a bit of extra horizontal slack so line breaks match
                // upstream Cypress baselines.
                let wrap_w = (bounded_width + 4.5 * wrap_padding)
                    .max(sequence_width)
                    .max(1.0);
                crate::text::wrap_label_like_mermaid_lines_floored_bbox(
                    text,
                    measurer,
                    &loop_text_style,
                    wrap_w,
                )
            } else {
                crate::text::split_html_br_lines(text)
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect()
            };
            for (i, raw) in raw_lines.into_iter().enumerate() {
                let y = lbl.y + (i as f64) * line_step;
                let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(&raw);
                let line = if decoded.as_ref().is_empty() {
                    "\u{200B}".to_string()
                } else {
                    decoded.as_ref().to_string()
                };
                let _ = write!(
                    &mut out,
                    r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="messageText" dy="1em" style="font-size: {fs}px; font-weight: 400;">{text}</text>"#,
                    x = fmt(lbl.x.round()),
                    y = fmt(y),
                    fs = fmt(actor_label_font_size),
                    text = escape_xml(&line)
                );
            }
        }

        let p0 = &edge.points[0];
        let p1 = &edge.points[1];
        let class = match msg.message_type {
            1 | 4 | 6 | 25 | 34 => "messageLine1",
            _ => "messageLine0",
        };
        let style = match msg.message_type {
            1 | 4 | 6 | 25 | 34 => r#" style="stroke-dasharray: 3, 3; fill: none;""#,
            _ => r#" style="fill: none;""#,
        };

        let marker_start = match msg.message_type {
            33 | 34 => Some(r#" marker-start="url(#arrowhead)""#),
            _ => None,
        };
        let marker_end = match msg.message_type {
            // open arrow variants: no marker.
            5 | 6 => None,
            // cross arrow variants
            3 | 4 => Some(r#" marker-end="url(#crosshead)""#),
            // filled-head variants
            24 | 25 => Some(r#" marker-end="url(#filled-head)""#),
            // default arrowhead variants
            _ => Some(r#" marker-end="url(#arrowhead)""#),
        };

        // Mermaid uses `stroke="none"` and assigns actual stroke via CSS.
        if from == to {
            let startx = p0.x;
            let y = p0.y;
            let d = if right_angles {
                let actor_w = nodes_by_id
                    .get(format!("actor-top-{from}").as_str())
                    .map(|n| n.width)
                    .unwrap_or(actor_height);
                let text_dx = edge.label.as_ref().map(|l| l.width / 2.0).unwrap_or(0.0);
                let dx = (actor_w / 2.0).max(text_dx);
                format!(
                    "M  {x},{y} H {hx} V {vy} H {x}",
                    x = fmt(startx),
                    y = fmt(y),
                    hx = fmt(startx + dx),
                    vy = fmt(y + 25.0)
                )
            } else {
                format!(
                    "M {x},{y} C {x2},{y2} {x2},{y3} {x},{y4}",
                    x = fmt(startx),
                    y = fmt(y),
                    x2 = fmt(startx + 60.0),
                    y2 = fmt(y - 10.0),
                    y3 = fmt(y + 30.0),
                    y4 = fmt(y + 20.0)
                )
            };
            let _ = write!(
                &mut out,
                r#"<path d="{d}" class="{class}" stroke-width="2" stroke="none"{marker_start}{marker_end}{style}/>"#,
                d = d,
                class = class,
                marker_start = marker_start.unwrap_or(""),
                marker_end = marker_end.unwrap_or(""),
                style = style
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" class="{class}" stroke-width="2" stroke="none"{marker_start}{marker_end}{style}/>"#,
                x1 = fmt(p0.x),
                y1 = fmt(p0.y),
                x2 = fmt(p1.x),
                y2 = fmt(p1.y),
                class = class,
                marker_start = marker_start.unwrap_or(""),
                marker_end = marker_end.unwrap_or(""),
                style = style
            );
        }

        if sequence_number_visible {
            let x = p0.x;
            let y = p0.y;
            let _ = write!(
                &mut out,
                r#"<line x1="{x}" y1="{y}" x2="{x}" y2="{y}" stroke-width="0" marker-start="url(#sequencenumber)"/>"#,
                x = fmt(x),
                y = fmt(y),
            );
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" font-family="sans-serif" font-size="12px" text-anchor="middle" class="sequenceNumber">{n}</text>"#,
                x = fmt(x),
                y = fmt(y + 4.0),
                n = sequence_number,
            );
            sequence_number = sequence_number.saturating_add(sequence_number_step);
        }

        let _ = (from, to);
    }

    // Mermaid emits actor popup menus (links/link directives) as root-level `<g class="actorPopupMenu">`
    // groups after messages.
    for (actor_cnt, actor_id) in model.actor_order.iter().enumerate() {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        if actor.links.is_empty() {
            continue;
        }
        let actor_custom_class = actor
            .properties
            .get("class")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let popup_display = if force_menus {
            "block !important"
        } else {
            "none"
        };
        let popup_fill = if actor_custom_class.is_some() {
            "#EDF2AE"
        } else {
            "#eaeaea"
        };
        let popup_actor_pos_class = if mirror_actors {
            "actor-bottom"
        } else {
            "actor-top"
        };
        let popup_panel_class = actor_custom_class
            .map(|c| format!("actorPopupMenuPanel {c} {popup_actor_pos_class}"))
            .unwrap_or_else(|| format!("actorPopupMenuPanel actor {popup_actor_pos_class}"));

        let node_id = format!("actor-top-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        let (x, _y) = node_left_top(n);

        let mut link_y: f64 = 20.0;
        let panel_height = 20.0 + (actor.links.len() as f64) * 30.0;

        let _ = write!(
            &mut out,
            r##"<g id="actor{idx}_popup" class="actorPopupMenu" display="{display}">"##,
            idx = actor_cnt,
            display = escape_attr(popup_display),
        );
        let _ = write!(
            &mut out,
            r##"<rect class="{class}" x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" rx="3" ry="3"/>"##,
            class = escape_attr(&popup_panel_class),
            x = fmt(x),
            y = fmt(actor_height),
            w = fmt(n.width),
            h = fmt(panel_height),
            fill = escape_xml_display(popup_fill),
        );

        for (label, url) in &actor.links {
            let Some(href) = url.as_str() else {
                continue;
            };
            let href = url::Url::parse(href)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| href.to_string());
            let href = merman_core::utils::format_url(&href, sanitize_config)
                .filter(|u| u.trim() != merman_core::utils::BLANK_URL);
            let text_x = x + 10.0;
            let text_y = actor_height + link_y + 10.0;
            if let Some(href) = href {
                let _ = write!(
                    &mut out,
                    r##"<a xlink:href="{href}"><text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor" style="text-anchor: start; font-size: 16px; font-weight: 400;"><tspan x="{x}" dy="0">{label}</tspan></text></a>"##,
                    href = escape_xml(&href),
                    x = fmt(text_x),
                    y = fmt(text_y),
                    label = escape_xml(label)
                );
            } else {
                let _ = write!(
                    &mut out,
                    r##"<a><text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor" style="text-anchor: start; font-size: 16px; font-weight: 400;"><tspan x="{x}" dy="0">{label}</tspan></text></a>"##,
                    x = fmt(text_x),
                    y = fmt(text_y),
                    label = escape_xml(label)
                );
            }
            link_y += 30.0;
        }

        out.push_str("</g>");
    }

    if mirror_actors {
        // Actor-man footers (actor/boundary/control/entity) are emitted after messages.
        let last_idx = model.actor_order.len().saturating_sub(1);
        for (actor_idx, actor_id) in model.actor_order.iter().enumerate() {
            let Some(actor) = model.actors.get(actor_id) else {
                continue;
            };
            let actor_type = actor.actor_type.as_str();
            if !matches!(actor_type, "actor" | "boundary" | "control" | "entity") {
                continue;
            }
            let node_id = format!("actor-bottom-{actor_id}");
            let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
                continue;
            };
            let (_x, actor_y) = node_left_top(n);
            let cx = n.x;

            match actor_type {
                "actor" => {
                    let r = 15.0;
                    let cy = actor_y + 10.0;
                    let torso_top = cy + r;
                    let torso_bottom = torso_top + 20.0;
                    let arms_y = torso_top + 8.0;
                    let arms_x1 = cx - 18.0;
                    let arms_x2 = cx + 18.0;
                    let leg_y = torso_bottom + 15.0;
                    let _ = write!(
                        &mut out,
                        r##"<g class="actor-man actor-bottom" name="{name}"><line id="actor-man-torso{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}"/><line id="actor-man-arms{idx}" x1="{ax1}" y1="{ay}" x2="{ax2}" y2="{ay}"/><line x1="{ax1}" y1="{ly}" x2="{cx}" y2="{y2}"/><line x1="{cx}" y1="{y2}" x2="{lx2}" y2="{ly}"/><circle cx="{cx}" cy="{cy}" r="15" width="{w}" height="{h}"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                        name = escape_xml(actor_id),
                        idx = last_idx,
                        cx = fmt(cx),
                        y1 = fmt(torso_top),
                        y2 = fmt(torso_bottom),
                        ax1 = fmt(arms_x1),
                        ax2 = fmt(arms_x2),
                        ay = fmt(arms_y),
                        ly = fmt(leg_y),
                        lx2 = fmt(cx + 16.0),
                        cy = fmt(cy),
                        w = fmt(n.width),
                        h = fmt(actor_height),
                        ty = fmt(actor_y + actor_height + 2.5),
                        label = escape_xml(&actor.description)
                    );
                }
                "boundary" => {
                    let radius = 30.0;
                    let x_left = cx - radius * 2.5;
                    let footer_h = 60.0 + label_box_height;
                    let _ = write!(
                        &mut out,
                        r##"<g class="actor-man actor-bottom" name="{name}" transform="translate(0,22)"><line id="actor-man-torso{idx}" x1="{x1}" y1="{y_t}" x2="{x2}" y2="{y_t}"/><line id="actor-man-arms{idx}" x1="{x1}" y1="{y0}" x2="{x1}" y2="{y20}"/><circle cx="{cx}" cy="{cy}" r="30"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                        name = escape_xml(actor_id),
                        idx = last_idx,
                        x1 = fmt(x_left),
                        x2 = fmt(cx - 15.0),
                        y_t = fmt(actor_y + 10.0),
                        y0 = fmt(actor_y + 0.0),
                        y20 = fmt(actor_y + 20.0),
                        cx = fmt(cx),
                        cy = fmt(actor_y + 10.0),
                        ty = fmt(actor_y + (radius / 2.0 - 4.0) + (footer_h / 2.0)),
                        label = escape_xml(&actor.description)
                    );
                }
                "control" => {
                    let r = 18.0;
                    let cy = actor_y + 30.0;
                    let footer_h = 36.0 + 2.0 * label_box_height;
                    let _ = write!(
                        &mut out,
                        r##"<g class="actor-man actor-bottom" name="{name}"><defs><marker id="filled-head-control" refX="11" refY="5.8" markerWidth="20" markerHeight="28" orient="172.5"><path d="M 14.4 5.6 L 7.2 10.4 L 8.8 5.6 L 7.2 0.8 Z"/></marker></defs><circle cx="{cx}" cy="{cy}" r="18" fill="#eaeaf7" stroke="#666" stroke-width="1.2"/><line marker-end="url(#filled-head-control)" transform="translate({cx}, {ly})"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                        name = escape_xml(actor_id),
                        cx = fmt(cx),
                        cy = fmt(cy),
                        ly = fmt(cy - r),
                        ty = fmt(actor_y + (r + 5.0) + (footer_h / 2.0)),
                        label = escape_xml(&actor.description)
                    );
                }
                "entity" => {
                    let r = 18.0;
                    let cy = actor_y + 10.0;
                    let footer_h = 36.0 + label_box_height;
                    let _ = write!(
                        &mut out,
                        r##"<g class="actor-man actor-bottom" name="{name}" transform="translate(0, 9)"><circle cx="{cx}" cy="{cy}" r="18" width="{w}" height="{h}"/><line x1="{x1}" x2="{x2}" y1="{y}" y2="{y}" stroke="#333" stroke-width="2"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                        name = escape_xml(actor_id),
                        cx = fmt(cx),
                        cy = fmt(cy),
                        w = fmt(n.width),
                        h = fmt(footer_h),
                        x1 = fmt(cx - r),
                        x2 = fmt(cx + r),
                        y = fmt(cy + r),
                        ty = fmt(actor_y + ((cy - actor_y + r - 5.0) / 2.0) + (footer_h / 2.0)),
                        label = escape_xml(&actor.description)
                    );
                }
                _ => {}
            }

            let _ = actor_idx;
        }
    }

    if let Some(title) = model.title.as_deref() {
        // Mermaid sequence titles are currently emitted as a plain `<text>` node.
        // Mermaid positions the title using the inner (content) box width:
        // `x = (box.stopx - box.startx) / 2 - 2 * diagramMarginX`.
        let title_x = ((vb_w - 2.0 * diagram_margin_x) / 2.0) - 2.0 * diagram_margin_x;
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="-25">{text}</text>"#,
            x = fmt(title_x),
            text = escape_xml(title)
        );
    }

    out.push_str("</svg>\n");
    Ok(out)
}

fn sequence_css(diagram_id: &str) -> String {
    // Mirrors Mermaid@11.12.2 `diagrams/sequence/styles.js` + shared base stylesheet ordering.
    // Keep `:root` last (matches upstream fixtures).
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}"#,
        id, font, id
    );

    // Sequence styles.
    let actor_border = "hsl(259.6261682243, 59.7765363128%, 87.9019607843%)";
    let actor_fill = "#ECECFF";
    let note_border = "#aaaa33";
    let note_fill = "#fff5ad";
    let _ = write!(
        &mut out,
        r#"#{} .actor{{stroke:{};fill:{};}}"#,
        id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} text.actor>tspan{{fill:black;stroke:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .actor-line{{stroke:{};}}"#,
        id, actor_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .innerArc{{stroke-width:1.5;stroke-dasharray:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine0{{stroke-width:1.5;stroke-dasharray:none;stroke:#333;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine1{{stroke-width:1.5;stroke-dasharray:2,2;stroke:#333;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} #arrowhead path{{fill:#333;stroke:#333;}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .sequenceNumber{{fill:white;}}"#, id);
    let _ = write!(&mut out, r#"#{} #sequencenumber{{fill:#333;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} #crosshead path{{fill:#333;stroke:#333;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageText{{fill:#333;stroke:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBox{{stroke:{};fill:{};}}"#,
        id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelText,#{} .labelText>tspan{{fill:black;stroke:none;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .loopText,#{} .loopText>tspan{{fill:black;stroke:none;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .loopLine{{stroke-width:2px;stroke-dasharray:2,2;stroke:{};fill:{};}}"#,
        id, actor_border, actor_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .note{{stroke:{};fill:{};}}"#,
        id, note_border, note_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .noteText,#{} .noteText>tspan{{fill:black;stroke:none;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activation0{{fill:#f4f4f4;stroke:#666;}}#{} .activation1{{fill:#f4f4f4;stroke:#666;}}#{} .activation2{{fill:#f4f4f4;stroke:#666;}}"#,
        id, id, id
    );
    let _ = write!(&mut out, r#"#{} .actorPopupMenu{{position:absolute;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .actorPopupMenuPanel{{position:absolute;fill:{};box-shadow:0px 8px 16px 0px rgba(0,0,0,0.2);filter:drop-shadow(3px 5px 2px rgb(0 0 0 / 0.4));}}"#,
        id, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .actor-man line{{stroke:{};fill:{};}}"#,
        id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .actor-man circle,#{} line{{stroke:{};fill:{};stroke-width:2px;}}"#,
        id, id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    out
}
