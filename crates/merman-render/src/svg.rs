use crate::model::{
    Bounds, ClassDiagramV2Layout, ErDiagramLayout, FlowchartV2Layout, InfoDiagramLayout,
    LayoutCluster, LayoutNode, PacketDiagramLayout, PieDiagramLayout, SequenceDiagramLayout,
    StateDiagramV2Layout, TimelineDiagramLayout,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use base64::Engine as _;
use chrono::TimeZone;
use serde::Deserialize;
use std::fmt::Write as _;

const MERMAID_SEQUENCE_BASE_DEFS_11_12_2: &str =
    include_str!("../assets/sequence_base_defs_11_12_2.svgfrag");

#[derive(Debug, Clone)]
pub struct SvgRenderOptions {
    /// Adds extra space around the computed viewBox.
    pub viewbox_padding: f64,
    /// Optional diagram id used for Mermaid-like marker ids.
    pub diagram_id: Option<String>,
    /// Optional override for the root SVG `aria-roledescription` attribute.
    ///
    /// This is primarily used to reproduce Mermaid's per-header accessibility metadata quirks
    /// (e.g. `classDiagram-v2` differs from `classDiagram` at Mermaid 11.12.2).
    pub aria_roledescription: Option<String>,
    /// When true, include edge polylines.
    pub include_edges: bool,
    /// When true, include node bounding boxes and ids.
    pub include_nodes: bool,
    /// When true, include cluster bounding boxes and titles.
    pub include_clusters: bool,
    /// When true, draw markers that visualize Mermaid cluster positioning metadata.
    pub include_cluster_debug_markers: bool,
    /// When true, label edge routes with edge ids.
    pub include_edge_id_labels: bool,
}

impl Default for SvgRenderOptions {
    fn default() -> Self {
        Self {
            viewbox_padding: 8.0,
            diagram_id: None,
            aria_roledescription: None,
            include_edges: true,
            include_nodes: true,
            include_clusters: true,
            include_cluster_debug_markers: false,
            include_edge_id_labels: false,
        }
    }
}

pub fn render_flowchart_v2_debug_svg(
    layout: &FlowchartV2Layout,
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
.edge-label { fill: #111827; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 11px; text-anchor: middle; dominant-baseline: middle; }
.debug-cross { stroke: #ef4444; stroke-width: 1; }
</style>
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
                out.push_str(r#"<polyline class="edge" points=""#);
                for (idx, p) in e.points.iter().enumerate() {
                    if idx > 0 {
                        out.push(' ');
                    }
                    let _ = write!(&mut out, "{},{}", fmt(p.x), fmt(p.y));
                }
                out.push_str(r#"" />"#);
            }
            if options.include_edge_id_labels {
                if let Some(lbl) = &e.label {
                    let _ = write!(
                        &mut out,
                        r#"<text class="edge-label" x="{}" y="{}">{}</text>"#,
                        fmt(lbl.x),
                        fmt(lbl.y),
                        escape_xml(&e.id)
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

pub fn render_sequence_diagram_debug_svg(
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
                        let _ = write!(&mut out, "{},{}", fmt(p.x), fmt(p.y));
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
                        escape_xml(&e.id)
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
    links: serde_json::Map<String, serde_json::Value>,
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
    notes: Vec<SequenceSvgNote>,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceSvgBox {
    #[serde(rename = "actorKeys")]
    actor_keys: Vec<String>,
    fill: String,
    name: Option<String>,
    wrap: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceSvgNote {
    actor: serde_json::Value,
    message: String,
    placement: i32,
    wrap: bool,
}

pub fn render_sequence_diagram_svg(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: SequenceSvgModel = serde_json::from_value(semantic.clone())?;

    let seq_cfg = effective_config
        .get("sequence")
        .unwrap_or(&serde_json::Value::Null);
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
    let message_margin = seq_cfg
        .get("messageMargin")
        .and_then(|v| v.as_f64())
        .unwrap_or(35.0)
        .max(0.0);
    let bottom_margin_adj = seq_cfg
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
    let actor_label_font_size = seq_cfg
        .get("messageFontSize")
        .and_then(|v| v.as_f64())
        .or_else(|| effective_config.get("fontSize").and_then(|v| v.as_f64()))
        .unwrap_or(16.0)
        .max(1.0);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let mut nodes_by_id: std::collections::HashMap<&str, &LayoutNode> =
        std::collections::HashMap::new();
    for n in &layout.nodes {
        nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut edges_by_id: std::collections::HashMap<&str, &crate::model::LayoutEdge> =
        std::collections::HashMap::new();
    for e in &layout.edges {
        edges_by_id.insert(e.id.as_str(), e);
    }

    fn node_left_top(n: &LayoutNode) -> (f64, f64) {
        (n.x - n.width / 2.0, n.y - n.height / 2.0)
    }

    fn split_html_br_lines(text: &str) -> Vec<&str> {
        let b = text.as_bytes();
        let mut parts: Vec<&str> = Vec::new();
        let mut start = 0usize;
        let mut i = 0usize;
        while i + 3 < b.len() {
            if b[i] != b'<' {
                i += 1;
                continue;
            }
            let b1 = b[i + 1];
            let b2 = b[i + 2];
            if !matches!(b1, b'b' | b'B') || !matches!(b2, b'r' | b'R') {
                i += 1;
                continue;
            }
            let mut j = i + 3;
            while j < b.len() && matches!(b[j], b' ' | b'\t' | b'\r' | b'\n') {
                j += 1;
            }
            if j < b.len() && b[j] == b'/' {
                j += 1;
            }
            if j < b.len() && b[j] == b'>' {
                parts.push(&text[start..i]);
                start = j + 1;
                i = start;
                continue;
            }
            i += 1;
        }
        parts.push(&text[start..]);
        parts
    }

    fn write_actor_label(out: &mut String, cx: f64, cy: f64, label: &str, font_size: f64) {
        let lines = split_html_br_lines(label);
        let n = lines.len().max(1) as f64;
        for (i, line) in lines.into_iter().enumerate() {
            let dy = if n <= 1.0 {
                0.0
            } else {
                (i as f64 - (n - 1.0) / 2.0) * font_size
            };
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor actor-box" style="text-anchor: middle; font-size: {fs}px; font-weight: 400;"><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
                x = fmt(cx),
                y = fmt(cy),
                fs = fmt(font_size),
                dy = fmt(dy),
                text = escape_xml(line)
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
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{min_x} {min_y} {w} {h}" role="graphics-document document" aria-roledescription="sequence"{aria}>"#,
        diagram_id_esc = diagram_id_esc,
        max_w = fmt(vb_w),
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
        aria = aria
    );

    if let Some(title) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(title)
        );
    }
    if let Some(desc) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(desc)
        );
    }

    // Mermaid renders "box" frames as root-level `<g><rect class="rect"/>...</g>` nodes before actors.
    // Mermaid renders boxes "behind" other elements; multiple boxes end up reversed in DOM order.
    for b in model.boxes.iter().rev() {
        const PAD_X: f64 = 25.0;
        const PAD_TOP: f64 = 32.0;
        const PAD_BOTTOM: f64 = 20.0;

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

        let x = min_x - PAD_X;
        let w = (max_x - min_x) + PAD_X * 2.0;
        let y = min_top_y - PAD_TOP;
        let h = (max_bottom_y - min_top_y) + PAD_TOP + PAD_BOTTOM;

        out.push_str("<g>");
        let _ = write!(
            &mut out,
            r#"<rect x="{x}" y="{y}" fill="{fill}" stroke="rgb(0,0,0, 0.5)" width="{w}" height="{h}" class="rect"/>"#,
            x = fmt(x),
            y = fmt(y),
            w = fmt(w),
            h = fmt(h),
            fill = escape_xml(&b.fill),
        );
        if let Some(name) = b.name.as_deref() {
            let cx = x + (w / 2.0);
            let text_y = y + 18.5;
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="text" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{x}" dy="0">{text}</tspan></text>"#,
                x = fmt(cx),
                y = fmt(text_y),
                text = escape_xml(name)
            );
        }
        out.push_str("</g>");
    }

    // Mermaid renders `rect` blocks as root-level `<rect class="rect"/>` nodes before actors.
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
        let _ = write!(
            &mut out,
            r#"<rect x="{x}" y="{y}" fill="{fill}" width="{w}" height="{h}" class="rect"/>"#,
            x = fmt(x),
            y = fmt(y),
            w = fmt(n.width),
            h = fmt(n.height),
            fill = escape_xml(fill)
        );
    }

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
                    name = escape_xml(actor_id)
                );
                let _ = write!(
                    &mut out,
                    r##"<rect x="{sx}" y="{sy}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor"/>"##,
                    sx = fmt(front_x),
                    sy = fmt(front_y),
                    w = fmt(n.width),
                    h = fmt(n.height),
                    name = escape_xml(actor_id)
                );
                write_actor_label(&mut out, cx, cy, &actor.description, actor_label_font_size);
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
                    actor_label_font_size,
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
                    actor_label_font_size,
                );
                out.push_str("</g>");
            }
            _ => {
                out.push_str("<g>");
                let _ = write!(
                    &mut out,
                    r##"<rect x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" rx="3" ry="3" class="actor actor-bottom"/>"##,
                    x = fmt(x),
                    y = fmt(y),
                    w = fmt(n.width),
                    h = fmt(n.height),
                    name = escape_xml(actor_id)
                );
                write_actor_label(
                    &mut out,
                    n.x,
                    n.y,
                    &actor.description,
                    actor_label_font_size,
                );
                out.push_str("</g>");
            }
        }

        let _ = idx;
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
                write_actor_label(&mut out, cx, cy, &actor.description, actor_label_font_size);
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
                    actor_label_font_size,
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
                    actor_label_font_size,
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
                    r##"<rect x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" rx="3" ry="3" class="actor actor-top"/>"##,
                    x = fmt(top_x),
                    y = fmt(top_y),
                    w = fmt(top.width),
                    h = fmt(top.height),
                    name = escape_xml(actor_id),
                );
                write_actor_label(
                    &mut out,
                    top.x,
                    top.y,
                    &actor.description,
                    actor_label_font_size,
                );
                out.push_str("</g></g>");
            }
        }
    }

    // CSS is ignored by DOM compare in non-strict modes; keep a placeholder `<style>` node.
    let _ = write!(&mut out, r#"<style>#{}{{}}</style><g/>"#, diagram_id_esc);

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
    // For headless rendering, precompute activation rectangles from the semantic message stream and
    // render them as root-level `<g><rect .../></g>` nodes before notes/blocks/messages.
    #[derive(Debug, Clone)]
    struct SequenceActivationStart {
        startx: f64,
        starty: f64,
        start_index: usize,
    }

    #[derive(Debug, Clone)]
    struct SequenceActivationRect {
        startx: f64,
        starty: f64,
        width: f64,
        height: f64,
        class_idx: usize,
        start_index: usize,
    }

    fn actor_center_x(
        nodes_by_id: &std::collections::HashMap<&str, &LayoutNode>,
        actor_id: &str,
    ) -> Option<f64> {
        let node_id = format!("actor-top-{actor_id}");
        nodes_by_id.get(node_id.as_str()).copied().map(|n| n.x)
    }

    fn lifeline_y(
        edges_by_id: &std::collections::HashMap<&str, &crate::model::LayoutEdge>,
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
    let mut activations: Vec<SequenceActivationRect> = Vec::new();

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
                let stack = activation_stacks.entry(actor_id.to_string()).or_default();
                let stacked_size = stack.len();
                let startx = cx + (((stacked_size as f64) - 1.0) * activation_width) / 2.0;

                let starty = last_line_y
                    .or_else(|| lifeline_y(&edges_by_id, actor_id).map(|(y0, _y1)| y0))
                    .unwrap_or(0.0);

                stack.push(SequenceActivationStart {
                    startx,
                    starty,
                    start_index: activation_counter,
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
                activations.push(SequenceActivationRect {
                    startx: start.startx,
                    starty,
                    width: activation_width,
                    height: (vertical_pos - starty).max(0.0),
                    class_idx,
                    start_index: start.start_index,
                });
            }
            _ => {}
        }

        let _ = msg.activate;
    }

    // Close any dangling activations at the end of the lifeline / diagram bounds.
    if !activation_stacks.is_empty() {
        let diagram_bottom = bounds.max_y;
        for (actor_id, stack) in activation_stacks.iter_mut() {
            let lifeline_bottom = lifeline_y(&edges_by_id, actor_id)
                .map(|(_y0, y1)| y1)
                .unwrap_or(diagram_bottom);
            while let Some(start) = stack.pop() {
                let class_idx = stack.len() % 3;
                activations.push(SequenceActivationRect {
                    startx: start.startx,
                    starty: start.starty,
                    width: activation_width,
                    height: (lifeline_bottom - start.starty).max(0.0),
                    class_idx,
                    start_index: start.start_index,
                });
            }
        }
    }

    activations.sort_by_key(|a| a.start_index);
    for a in &activations {
        out.push_str("<g>");
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
        out.push_str("</g>");
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

    fn estimate_char_width_em(ch: char) -> f64 {
        if ch == ' ' {
            return 0.33;
        }
        if ch == '\t' {
            return 0.66;
        }
        if ch == '_' || ch == '-' {
            return 0.33;
        }
        if matches!(ch, '.' | ',' | ':' | ';') {
            return 0.28;
        }
        if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '/') {
            return 0.33;
        }
        if matches!(ch, '+' | '*' | '=' | '\\' | '^' | '|' | '~') {
            return 0.45;
        }
        if ch.is_ascii_digit() {
            return 0.56;
        }
        if ch.is_ascii_uppercase() {
            return match ch {
                'I' => 0.30,
                'W' => 0.85,
                _ => 0.60,
            };
        }
        if ch.is_ascii_lowercase() {
            return match ch {
                'i' | 'l' => 0.28,
                'm' | 'w' => 0.78,
                'k' | 'y' => 0.55,
                _ => 0.43,
            };
        }
        0.60
    }

    fn estimate_line_width_px(line: &str, font_size: f64) -> f64 {
        let em: f64 = line.chars().map(estimate_char_width_em).sum();
        em * font_size
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

    fn wrap_svg_text_line(line: &str, font_size: f64, max_width: f64) -> Vec<String> {
        use std::collections::VecDeque;

        if !max_width.is_finite() || max_width <= 0.0 {
            return vec![line.to_string()];
        }

        let mut tokens = VecDeque::from(split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            if estimate_line_width_px(&candidate, font_size) <= max_width {
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

            // `tok` itself does not fit on an empty line; split by characters.
            let chars = tok.chars().collect::<Vec<_>>();
            let mut cut = 1usize;
            while cut < chars.len() {
                let head: String = chars[..cut].iter().collect();
                if estimate_line_width_px(&head, font_size) > max_width {
                    break;
                }
                cut += 1;
            }
            cut = cut.saturating_sub(1).max(1);
            let head: String = chars[..cut].iter().collect();
            let tail: String = chars[cut..].iter().collect();
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

    fn wrap_svg_text_lines(text: &str, font_size: f64, max_width: Option<f64>) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for line in split_html_br_lines(text) {
            if let Some(w) = max_width {
                lines.extend(wrap_svg_text_line(line, font_size, w));
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
        x: f64,
        y0: f64,
        font_size: f64,
        max_width: Option<f64>,
        text: &str,
        use_tspan: bool,
    ) {
        let line_step = font_size * 1.1875;
        let lines = wrap_svg_text_lines(text, font_size, max_width);
        for (i, line) in lines.into_iter().enumerate() {
            let y = y0 + (i as f64) * line_step;
            if use_tspan {
                let _ = write!(
                    out,
                    r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="font-size: {fs}px; font-weight: 400;"><tspan x="{x}">{text}</tspan></text>"#,
                    x = fmt(x),
                    y = fmt(y),
                    fs = fmt(font_size),
                    text = escape_xml(&line)
                );
            } else {
                let _ = write!(
                    out,
                    r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="font-size: {fs}px; font-weight: 400;">{text}</text>"#,
                    x = fmt(x),
                    y = fmt(y),
                    fs = fmt(font_size),
                    text = escape_xml(&line)
                );
            }
        }
    }

    fn frame_x_from_actors(
        model: &SequenceSvgModel,
        nodes_by_id: &std::collections::HashMap<&str, &LayoutNode>,
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
        edges_by_id: &std::collections::HashMap<&str, &crate::model::LayoutEdge>,
        msg_id: &str,
    ) -> Option<f64> {
        let edge_id = format!("msg-{msg_id}");
        let e = edges_by_id.get(edge_id.as_str()).copied()?;
        Some(e.points.first()?.y)
    }

    // Mermaid renders block frames (`alt`, `loop`, ...) as `<g>` elements before message lines.
    // Use layout-derived message y-coordinates for separator placement to avoid visual artifacts
    // like dashed lines ending in a gap right before the frame border.
    #[derive(Debug, Clone)]
    enum SequencePreItem {
        Note { id: String, raw: String },
        Block(usize),
    }

    let mut pre_items: Vec<SequencePreItem> = Vec::new();
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
                pre_items.push(SequencePreItem::Note {
                    id: msg.id.clone(),
                    raw: raw_label.to_string(),
                });
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
                    pre_items.push(SequencePreItem::Block(idx));
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
                    pre_items.push(SequencePreItem::Block(idx));
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
                    pre_items.push(SequencePreItem::Block(idx));
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
                    pre_items.push(SequencePreItem::Block(idx));
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
                    pre_items.push(SequencePreItem::Block(idx));
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
                    pre_items.push(SequencePreItem::Block(idx));
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
            let t = raw_label.trim();
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

        let mut actor_nodes_by_id: std::collections::HashMap<&str, &LayoutNode> =
            std::collections::HashMap::new();
        for actor_id in &model.actor_order {
            let node_id = format!("actor-top-{actor_id}");
            let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
                continue;
            };
            actor_nodes_by_id.insert(actor_id.as_str(), n);
        }

        let mut msg_endpoints: std::collections::HashMap<&str, (&str, &str)> =
            std::collections::HashMap::new();
        for msg in &model.messages {
            let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
                continue;
            };
            msg_endpoints.insert(msg.id.as_str(), (from, to));
        }

        fn frame_x_from_message_ids<'a>(
            message_ids: impl IntoIterator<Item = &'a String>,
            msg_endpoints: &std::collections::HashMap<&str, (&str, &str)>,
            actor_nodes_by_id: &std::collections::HashMap<&str, &LayoutNode>,
        ) -> Option<(f64, f64, f64)> {
            const SIDE_PAD: f64 = 11.0;
            let mut min_cx = f64::INFINITY;
            let mut max_cx = f64::NEG_INFINITY;
            let mut min_left = f64::INFINITY;

            for msg_id in message_ids {
                let Some((from, to)) = msg_endpoints.get(msg_id.as_str()).copied() else {
                    continue;
                };
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
            Some((min_cx - SIDE_PAD, max_cx + SIDE_PAD, min_left))
        }

        for item in &pre_items {
            match item {
                SequencePreItem::Note { id, raw } => {
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
                    let lines = split_html_br_lines(raw);
                    for (i, line) in lines.into_iter().enumerate() {
                        let y = text_y + (i as f64) * line_step;
                        let _ = write!(
                            &mut out,
                            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="noteText" dy="1em" style="font-size: {fs}px; font-weight: 400;"><tspan x="{x}">{text}</tspan></text>"#,
                            x = fmt(cx),
                            y = fmt(y),
                            fs = fmt(actor_label_font_size),
                            text = escape_xml(line)
                        );
                    }
                    out.push_str("</g>");
                }
                SequencePreItem::Block(idx) => {
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
                                    if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                        min_y = min_y.min(y);
                                        max_y = max_y.max(y);
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
                            )
                            .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                            let frame_y1 = min_y - 79.0;
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
                                    if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                        sec_max_y = sec_max_y.max(y);
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
                            let label_cx = x1 + 25.0;
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
                                        main_text_x,
                                        y,
                                        actor_label_font_size,
                                        Some(max_w),
                                        &label_text,
                                        true,
                                    );
                                    continue;
                                }
                                let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                                write_loop_text_lines(
                                    &mut out,
                                    center_text_x,
                                    y,
                                    actor_label_font_size,
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
                                    if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                        min_y = min_y.min(y);
                                        max_y = max_y.max(y);
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
                            )
                            .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                            let frame_y1 = min_y - 79.0;
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
                                    if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                        sec_max_y = sec_max_y.max(y);
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
                            let label_cx = x1 + 25.0;
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
                                        main_text_x,
                                        y,
                                        actor_label_font_size,
                                        Some(max_w),
                                        &label_text,
                                        true,
                                    );
                                    continue;
                                }
                                let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                                write_loop_text_lines(
                                    &mut out,
                                    center_text_x,
                                    y,
                                    actor_label_font_size,
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
                                if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                    min_y = min_y.min(y);
                                    max_y = max_y.max(y);
                                }
                            }
                            if !min_y.is_finite() || !max_y.is_finite() {
                                continue;
                            }

                            let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                                message_ids.iter(),
                                &msg_endpoints,
                                &actor_nodes_by_id,
                            )
                            .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                            let frame_y1 = min_y - 40.0;
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
                            let label_cx = x1 + 25.0;
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
                                text_x,
                                text_y,
                                actor_label_font_size,
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
                                if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                    min_y = min_y.min(y);
                                    max_y = max_y.max(y);
                                }
                            }
                            if !min_y.is_finite() || !max_y.is_finite() {
                                continue;
                            }

                            let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                                message_ids.iter(),
                                &msg_endpoints,
                                &actor_nodes_by_id,
                            )
                            .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                            let frame_y1 = min_y - 40.0;
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
                            let label_cx = x1 + 25.0;
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
                                text_x,
                                text_y,
                                actor_label_font_size,
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
                                if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                    min_y = min_y.min(y);
                                    max_y = max_y.max(y);
                                }
                            }
                            if !min_y.is_finite() || !max_y.is_finite() {
                                continue;
                            }

                            let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                                message_ids.iter(),
                                &msg_endpoints,
                                &actor_nodes_by_id,
                            )
                            .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                            let frame_y1 = min_y - 79.0;
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
                            let label_cx = x1 + 25.0;
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
                                text_x,
                                text_y,
                                actor_label_font_size,
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
                                    if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                        min_y = min_y.min(y);
                                        max_y = max_y.max(y);
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
                            )
                            .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));
                            if sections.len() > 1 && min_left.is_finite() {
                                // Mermaid's `critical` w/ `option` sections widens the frame to the left.
                                frame_x1 = frame_x1.min(min_left - 9.0);
                            }

                            let frame_y1 = min_y - 79.0;
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
                                    if let Some(y) = msg_line_y(&edges_by_id, msg_id) {
                                        sec_max_y = sec_max_y.max(y);
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
                            let label_cx = x1 + 25.0;
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
                                        main_text_x,
                                        y,
                                        actor_label_font_size,
                                        Some(max_w),
                                        &label_text,
                                        true,
                                    );
                                    continue;
                                }
                                let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                                write_loop_text_lines(
                                    &mut out,
                                    center_text_x,
                                    y,
                                    actor_label_font_size,
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
            let lines = split_html_br_lines(text);
            for (i, line) in lines.into_iter().enumerate() {
                let y = lbl.y + (i as f64) * line_step;
                let _ = write!(
                    &mut out,
                    r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="messageText" dy="1em" style="font-size: {fs}px; font-weight: 400;">{text}</text>"#,
                    x = fmt(lbl.x),
                    y = fmt(y),
                    fs = fmt(actor_label_font_size),
                    text = escape_xml(line)
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
                n = escape_xml(&sequence_number.to_string()),
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

        let node_id = format!("actor-top-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        let (x, _y) = node_left_top(n);

        let mut link_y: f64 = 20.0;
        let panel_height = 20.0 + (actor.links.len() as f64) * 30.0;

        let _ = write!(
            &mut out,
            r##"<g id="actor{idx}_popup" class="actorPopupMenu" display="none">"##,
            idx = actor_cnt
        );
        let _ = write!(
            &mut out,
            r##"<rect class="actorPopupMenuPanel actor actor-bottom" x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" rx="3" ry="3"/>"##,
            x = fmt(x),
            y = fmt(actor_height),
            w = fmt(n.width),
            h = fmt(panel_height)
        );

        for (label, url) in &actor.links {
            let Some(href) = url.as_str() else {
                continue;
            };
            let href = url::Url::parse(href)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| href.to_string());
            let text_x = x + 10.0;
            let text_y = actor_height + link_y + 10.0;
            let _ = write!(
                &mut out,
                r##"<a xlink:href="{href}"><text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor" style="text-anchor: start; font-size: 16px; font-weight: 400;"><tspan x="{x}" dy="0">{label}</tspan></text></a>"##,
                href = escape_xml(&href),
                x = fmt(text_x),
                y = fmt(text_y),
                label = escape_xml(label)
            );
            link_y += 30.0;
        }

        out.push_str("</g>");
    }

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

    if let Some(title) = model.title.as_deref() {
        // Mermaid sequence titles are currently emitted as a plain `<text>` node.
        let title_x = vb_min_x + (vb_w / 2.0);
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

#[derive(Debug, Clone, Deserialize)]
struct PieSvgSection {
    label: String,
    value: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct PieSvgModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    #[serde(rename = "showData")]
    show_data: bool,
    title: Option<String>,
    sections: Vec<PieSvgSection>,
}

fn info_css(diagram_id: &str) -> String {
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
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}#{} :root{{--mermaid-font-family:{};}}"#,
        id, font, id, id, font
    );
    out
}

fn pie_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = info_css(diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} .pieCircle{{stroke:black;stroke-width:2px;opacity:0.7;}}#{} .pieOuterCircle{{stroke:black;stroke-width:2px;fill:none;}}#{} .pieTitleText{{text-anchor:middle;font-size:25px;fill:black;font-family:{};}}#{} .slice{{font-family:{};fill:#333;font-size:17px;}}#{} .legend text{{fill:black;font-family:{};font-size:17px;}}"#,
        id, id, id, font, id, font, id, font
    );
    out
}

fn packet_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let mut out = info_css(diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} .packetByte{{font-size:10px;}}#{} .packetByte.start{{fill:black;}}#{} .packetByte.end{{fill:black;}}#{} .packetLabel{{fill:black;font-size:12px;}}#{} .packetTitle{{fill:black;font-size:14px;}}#{} .packetBlock{{stroke:black;stroke-width:1;fill:#efefef;}}"#,
        id, id, id, id, id, id
    );
    out
}

fn timeline_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let mut out = info_css(diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} .edge{{stroke-width:3;}}#{} .edge{{fill:none;}}#{} .eventWrapper{{filter:brightness(120%);}}"#,
        id, id, id
    );
    out
}

fn journey_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let mut out = info_css(diagram_id);
    // Keep this intentionally small: DOM compare tooling ignores `<style>` text, but CSS helps
    // when visually inspecting rendered SVGs.
    let _ = write!(
        &mut out,
        r#"#{} .label{{font-family:"trebuchet ms",verdana,arial,sans-serif;color:#333;}}#{} .mouth{{stroke:#666;}}#{} line{{stroke:#333;}}#{} .legend{{fill:#333;font-family:"trebuchet ms",verdana,arial,sans-serif;}}#{} .label text{{fill:#333;}}#{} .label{{color:#333;}}#{} .face{{fill:#FFF8DC;stroke:#999;}}"#,
        id, id, id, id, id, id, id
    );
    out
}

fn kanban_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let mut out = info_css(diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} .edge{{stroke-width:3;}}#{} .edge{{fill:none;}}#{} .cluster-label,#{} .label{{color:#333;fill:#333;}}"#,
        id, id, id, id
    );
    out
}

fn gitgraph_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let mut out = info_css(diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} .branch{{stroke-width:1;stroke:#333333;stroke-dasharray:2;}}#{} .arrow{{stroke-width:8;stroke-linecap:round;fill:none;}}#{} .commit-label{{font-size:10px;}}#{} .commit-label-bkg{{font-size:10px;opacity:0.5;}}"#,
        id, id, id, id
    );
    out
}

fn gantt_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let root_rule = format!(r#"#{} :root{{--mermaid-font-family:{};}}"#, id, font);
    let mut out = info_css(diagram_id);
    if let Some(prefix) = out.strip_suffix(&root_rule) {
        out = prefix.to_string();
    }

    let _ = write!(
        &mut out,
        r#"#{} .mermaid-main-font{{font-family:{};}}"#,
        id, font
    );
    let _ = write!(&mut out, r#"#{} .exclude-range{{fill:#eeeeee;}}"#, id);
    let _ = write!(&mut out, r#"#{} .section{{stroke:none;opacity:0.2;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .section0{{fill:rgba(102, 102, 255, 0.49);}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .section2{{fill:#fff400;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .section1,#{} .section3{{fill:white;opacity:0.2;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle0{{fill:#333;}}#{} .sectionTitle1{{fill:#333;}}#{} .sectionTitle2{{fill:#333;}}#{} .sectionTitle3{{fill:#333;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle{{text-anchor:start;font-family:{};}}"#,
        id, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .grid .tick{{stroke:lightgrey;opacity:0.8;shape-rendering:crispEdges;}}#{} .grid .tick text{{font-family:{};fill:#333;}}#{} .grid path{{stroke-width:0;}}"#,
        id, id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .today{{fill:none;stroke:red;stroke-width:2px;}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .task{{stroke-width:2;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .taskText{{text-anchor:middle;font-family:{};}}#{} .taskTextOutsideRight{{fill:black;text-anchor:start;font-family:{};}}#{} .taskTextOutsideLeft{{fill:black;text-anchor:end;}}"#,
        id, font, id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task.clickable{{cursor:pointer;}}#{} .taskText.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}#{} .taskTextOutsideLeft.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}#{} .taskTextOutsideRight.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .taskText0,#{} .taskText1,#{} .taskText2,#{} .taskText3{{fill:white;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task0,#{} .task1,#{} .task2,#{} .task3{{fill:#8a90dd;stroke:#534fbc;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .taskTextOutside0,#{} .taskTextOutside2{{fill:black;}}#{} .taskTextOutside1,#{} .taskTextOutside3{{fill:black;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .active0,#{} .active1,#{} .active2,#{} .active3{{fill:#bfc7ff;stroke:#534fbc;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeText0,#{} .activeText1,#{} .activeText2,#{} .activeText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .done0,#{} .done1,#{} .done2,#{} .done3{{stroke:grey;fill:lightgrey;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneText0,#{} .doneText1,#{} .doneText2,#{} .doneText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .crit0,#{} .crit1,#{} .crit2,#{} .crit3{{stroke:#ff8888;fill:red;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeCrit0,#{} .activeCrit1,#{} .activeCrit2,#{} .activeCrit3{{stroke:#ff8888;fill:#bfc7ff;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneCrit0,#{} .doneCrit1,#{} .doneCrit2,#{} .doneCrit3{{stroke:#ff8888;fill:lightgrey;stroke-width:2;cursor:pointer;shape-rendering:crispEdges;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .milestone{{transform:rotate(45deg) scale(0.8,0.8);}}#{} .milestoneText{{font-style:italic;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneCritText0,#{} .doneCritText1,#{} .doneCritText2,#{} .doneCritText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .vert{{stroke:navy;}}#{} .vertText{{font-size:15px;text-anchor:middle;fill:navy!important;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeCritText0,#{} .activeCritText1,#{} .activeCritText2,#{} .activeCritText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .titleText{{text-anchor:middle;font-size:18px;fill:#333;font-family:{};}}"#,
        id, font
    );

    out.push_str(&root_rule);
    out
}

fn pie_legend_rect_style(fill: &str) -> String {
    // Mermaid emits legend colors via inline `style` in rgb() form for default themes.
    // The compare tooling ignores `style`, but we keep this for human inspection parity.
    let rgb = match fill {
        "#ECECFF" => "rgb(236, 236, 255)",
        "#ffffde" => "rgb(255, 255, 222)",
        "hsl(80, 100%, 56.2745098039%)" => "rgb(181, 255, 32)",
        other => other,
    };
    format!("fill: {rgb}; stroke: {rgb};")
}

fn pie_polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    let x = radius * angle.sin();
    let y = -radius * angle.cos();
    (x, y)
}

pub fn render_info_diagram_svg(
    layout: &InfoDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: 400px; background-color: white;" role="graphics-document document" aria-roledescription="info">"#,
    );
    let css = info_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);
    let _ = write!(
        &mut out,
        r#"<g><text x="100" y="40" class="version" font-size="32" style="text-anchor: middle;">{}</text></g>"#,
        escape_xml(&layout.version)
    );
    out.push_str("</svg>\n");
    Ok(out)
}

pub fn render_pie_diagram_svg(
    layout: &PieDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: PieSvgModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 450.0,
        max_y: 450.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

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

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="{min_x} {min_y} {w} {h}" style="max-width: {max_w}px; background-color: white;" role="graphics-document document" aria-roledescription="pie"{aria}>"#,
        diagram_id_esc = diagram_id_esc,
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
        max_w = fmt(vb_w),
        aria = aria
    );

    if let Some(t) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(t)
        );
    }
    if let Some(d) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(d)
        );
    }

    let css = pie_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);

    let _ = write!(
        &mut out,
        r#"<g transform="translate({x},{y})">"#,
        x = fmt(layout.center_x),
        y = fmt(layout.center_y)
    );
    let _ = write!(
        &mut out,
        r#"<circle cx="0" cy="0" r="{r}" class="pieOuterCircle"/>"#,
        r = fmt(layout.outer_radius)
    );

    for slice in &layout.slices {
        let r = layout.radius;
        if slice.is_full_circle {
            let d = format!(
                "M0,-{r}A{r},{r},0,1,1,0,{r}A{r},{r},0,1,1,0,-{r}Z",
                r = fmt(r)
            );
            let _ = write!(
                &mut out,
                r#"<path d="{d}" fill="{fill}" class="pieCircle"/>"#,
                d = d,
                fill = escape_xml(&slice.fill)
            );
        } else {
            let (x0, y0) = pie_polar_xy(r, slice.start_angle);
            let (x1, y1) = pie_polar_xy(r, slice.end_angle);
            let large = if (slice.end_angle - slice.start_angle) > std::f64::consts::PI {
                1
            } else {
                0
            };
            let d = format!(
                "M{x0},{y0}A{r},{r},0,{large},1,{x1},{y1}L0,0Z",
                x0 = fmt(x0),
                y0 = fmt(y0),
                r = fmt(r),
                large = large,
                x1 = fmt(x1),
                y1 = fmt(y1)
            );
            let _ = write!(
                &mut out,
                r#"<path d="{d}" fill="{fill}" class="pieCircle"/>"#,
                d = d,
                fill = escape_xml(&slice.fill)
            );
        }
    }

    for slice in &layout.slices {
        let _ = write!(
            &mut out,
            r#"<text transform="translate({x},{y})" class="slice" style="text-anchor: middle;">{text}</text>"#,
            x = fmt(slice.text_x),
            y = fmt(slice.text_y),
            text = escape_xml(&format!("{}%", slice.percent))
        );
    }

    match model.title.as_deref() {
        Some(t) => {
            let _ = write!(
                &mut out,
                r#"<text x="0" y="-200" class="pieTitleText">{text}</text>"#,
                text = escape_xml(t)
            );
        }
        None => {
            out.push_str(r#"<text x="0" y="-200" class="pieTitleText"/>"#);
        }
    }

    for item in &layout.legend_items {
        let _ = write!(
            &mut out,
            r#"<g class="legend" transform="translate({x},{y})">"#,
            x = fmt(layout.legend_x),
            y = fmt(item.y)
        );
        let style = pie_legend_rect_style(&item.fill);
        let _ = write!(
            &mut out,
            r#"<rect width="18" height="18" style="{style}"/>"#,
            style = escape_xml(&style)
        );
        let text = if model.show_data {
            format!("{} [{}]", item.label, fmt(item.value))
        } else {
            item.label.clone()
        };
        let _ = write!(
            &mut out,
            r#"<text x="22" y="14">{text}</text>"#,
            text = escape_xml(&text)
        );
        out.push_str("</g>");
    }

    out.push_str("</g></svg>\n");
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
struct PacketSvgModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    title: Option<String>,
}

pub fn render_packet_diagram_svg(
    layout: &PacketDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: PacketSvgModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: layout.width.max(1.0),
        max_y: layout.height.max(1.0),
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

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

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="{min_x} {min_y} {w} {h}" style="max-width: {max_w}px; background-color: white;" role="graphics-document document" aria-roledescription="packet"{aria}>"#,
        diagram_id_esc = diagram_id_esc,
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
        max_w = fmt(vb_w),
        aria = aria
    );

    if let Some(t) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(t)
        );
    }
    if let Some(d) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(d)
        );
    }

    let css = packet_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);

    for word in &layout.words {
        out.push_str("<g>");
        for b in &word.blocks {
            let _ = write!(
                &mut out,
                r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" class="packetBlock"/>"#,
                x = fmt(b.x),
                y = fmt(b.y),
                w = fmt(b.width),
                h = fmt(b.height)
            );
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" class="packetLabel" dominant-baseline="middle" text-anchor="middle">{text}</text>"#,
                x = fmt(b.x + b.width / 2.0),
                y = fmt(b.y + b.height / 2.0),
                text = escape_xml(&b.label)
            );

            if !layout.show_bits {
                continue;
            }
            let is_single_block = b.start == b.end;
            let bit_number_y = b.y - 2.0;
            let start_x = if is_single_block {
                b.x + b.width / 2.0
            } else {
                b.x
            };
            let start_anchor = if is_single_block { "middle" } else { "start" };
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" class="packetByte start" dominant-baseline="auto" text-anchor="{anchor}">{text}</text>"#,
                x = fmt(start_x),
                y = fmt(bit_number_y),
                anchor = start_anchor,
                text = b.start
            );
            if !is_single_block {
                let _ = write!(
                    &mut out,
                    r#"<text x="{x}" y="{y}" class="packetByte end" dominant-baseline="auto" text-anchor="end">{text}</text>"#,
                    x = fmt(b.x + b.width),
                    y = fmt(bit_number_y),
                    text = b.end
                );
            }
        }
        out.push_str("</g>");
    }

    let total_row_height = layout.row_height + layout.padding_y;
    let title_y = layout.height - total_row_height / 2.0;
    match model.title.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(title) => {
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" dominant-baseline="middle" text-anchor="middle" class="packetTitle">{text}</text>"#,
                x = fmt(layout.width / 2.0),
                y = fmt(title_y),
                text = escape_xml(title)
            );
        }
        None => {
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" dominant-baseline="middle" text-anchor="middle" class="packetTitle"/>"#,
                x = fmt(layout.width / 2.0),
                y = fmt(title_y),
            );
        }
    }

    out.push_str("</svg>\n");
    Ok(out)
}

pub fn render_timeline_diagram_svg(
    layout: &TimelineDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let _ = (semantic, effective_config);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    fn node_line_class(section_class: &str) -> String {
        let rest = section_class
            .strip_prefix("section-")
            .unwrap_or(section_class);
        format!("node-line-{rest}")
    }

    fn render_node(out: &mut String, n: &crate::model::TimelineNodeLayout) {
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        let rd = 5.0;
        let d = format!(
            "M0 {y0} v{v1} q0,-5 5,-5 h{hw} q5,0 5,5 v{v2} H0 Z",
            y0 = fmt(h - rd),
            v1 = fmt(-h + 2.0 * rd),
            hw = fmt(w - 2.0 * rd),
            v2 = fmt(h - rd),
        );

        let _ = write!(
            out,
            r#"<g class="timeline-node {section_class}">"#,
            section_class = escape_attr(&n.section_class)
        );
        out.push_str("<g>");
        let _ = write!(
            out,
            r#"<path id="node-undefined" class="node-bkg node-undefined" d="{d}"/>"#,
            d = escape_attr(&d)
        );
        let _ = write!(
            out,
            r#"<line class="{line_class}" x1="0" y1="{y}" x2="{x2}" y2="{y}"/>"#,
            line_class = escape_attr(&node_line_class(&n.section_class)),
            y = fmt(h),
            x2 = fmt(w)
        );
        out.push_str("</g>");

        let tx = w / 2.0;
        let ty = n.padding / 2.0;
        let _ = write!(
            out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(tx),
            y = fmt(ty)
        );
        out.push_str(r#"<text dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle">"#);
        for (idx, line) in n.label_lines.iter().enumerate() {
            let dy = if idx == 0 { "1em" } else { "1.1em" };
            let _ = write!(
                out,
                r#"<tspan x="0" dy="{dy}">{text}</tspan>"#,
                dy = dy,
                text = escape_xml(line)
            );
        }
        out.push_str("</text></g></g>");
    }

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{min_x} {min_y} {w} {h}" role="graphics-document document" aria-roledescription="timeline">"#,
        diagram_id_esc = diagram_id_esc,
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
        max_w = fmt(vb_w),
    );
    let css = timeline_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);
    out.push_str(r#"<g/>"#);
    out.push_str(
        r#"<defs><marker id="arrowhead" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto"><path d="M 0,0 V 4 L6,2 Z"/></marker></defs>"#,
    );

    for section in &layout.sections {
        let node = &section.node;
        let _ = write!(
            &mut out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(node.x),
            y = fmt(node.y)
        );
        render_node(&mut out, node);
        out.push_str("</g>");

        for task in &section.tasks {
            let task_node = &task.node;
            let _ = write!(
                &mut out,
                r#"<g class="taskWrapper" transform="translate({x}, {y})">"#,
                x = fmt(task_node.x),
                y = fmt(task_node.y)
            );
            render_node(&mut out, task_node);
            out.push_str("</g>");

            let _ = write!(
                &mut out,
                r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="2" stroke="black" marker-end="url(#arrowhead)" stroke-dasharray="5,5"/></g>"#,
                x1 = fmt(task.connector.x1),
                y1 = fmt(task.connector.y1),
                x2 = fmt(task.connector.x2),
                y2 = fmt(task.connector.y2),
            );

            for ev in &task.events {
                let _ = write!(
                    &mut out,
                    r#"<g class="eventWrapper" transform="translate({x}, {y})">"#,
                    x = fmt(ev.x),
                    y = fmt(ev.y)
                );
                render_node(&mut out, ev);
                out.push_str("</g>");
            }
        }
    }

    for task in &layout.orphan_tasks {
        let task_node = &task.node;
        let _ = write!(
            &mut out,
            r#"<g class="taskWrapper" transform="translate({x}, {y})">"#,
            x = fmt(task_node.x),
            y = fmt(task_node.y)
        );
        render_node(&mut out, task_node);
        out.push_str("</g>");

        let _ = write!(
            &mut out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="2" stroke="black" marker-end="url(#arrowhead)" stroke-dasharray="5,5"/></g>"#,
            x1 = fmt(task.connector.x1),
            y1 = fmt(task.connector.y1),
            x2 = fmt(task.connector.x2),
            y2 = fmt(task.connector.y2),
        );

        for ev in &task.events {
            let _ = write!(
                &mut out,
                r#"<g class="eventWrapper" transform="translate({x}, {y})">"#,
                x = fmt(ev.x),
                y = fmt(ev.y)
            );
            render_node(&mut out, ev);
            out.push_str("</g>");
        }
    }

    if let Some(title) = layout.title.as_deref().filter(|t| !t.trim().is_empty()) {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="4ex" font-weight="bold" y="{y}">{text}</text>"#,
            x = fmt(layout.title_x),
            y = fmt(layout.title_y),
            text = escape_xml(title)
        );
    }

    let _ = write!(
        &mut out,
        r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="url(#arrowhead)"/></g>"#,
        x1 = fmt(layout.activity_line.x1),
        y1 = fmt(layout.activity_line.y1),
        x2 = fmt(layout.activity_line.x2),
        y2 = fmt(layout.activity_line.y2),
    );

    out.push_str("</svg>\n");
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
struct JourneySvgModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
}

pub fn render_journey_diagram_svg(
    layout: &crate::model::JourneyDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: JourneySvgModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: -25.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let task_font_size = effective_config
        .get("journey")
        .and_then(|j| j.get("taskFontSize"))
        .and_then(|v| v.as_f64())
        .unwrap_or(14.0)
        .max(1.0);
    let task_font_family = effective_config
        .get("journey")
        .and_then(|j| j.get("taskFontFamily"))
        .and_then(|v| v.as_str())
        .unwrap_or("\"Open Sans\", sans-serif");

    let title_font_size = effective_config
        .get("journey")
        .and_then(|j| j.get("titleFontSize"))
        .and_then(|v| v.as_str())
        .unwrap_or("4ex");
    let title_font_family = effective_config
        .get("journey")
        .and_then(|j| j.get("titleFontFamily"))
        .and_then(|v| v.as_str())
        .unwrap_or("\"trebuchet ms\", verdana, arial, sans-serif");
    let title_color = effective_config
        .get("journey")
        .and_then(|j| j.get("titleColor"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    fn split_html_br_lines(text: &str) -> Vec<String> {
        let b = text.as_bytes();
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut i = 0usize;
        while i < b.len() {
            if b[i] != b'<' {
                let ch = text[i..].chars().next().unwrap();
                cur.push(ch);
                i += ch.len_utf8();
                continue;
            }
            if i + 3 >= b.len() {
                cur.push('<');
                i += 1;
                continue;
            }
            if b[i + 1] == b'/' {
                cur.push('<');
                i += 1;
                continue;
            }
            let b1 = b[i + 1];
            let b2 = b[i + 2];
            if !matches!(b1, b'b' | b'B') || !matches!(b2, b'r' | b'R') {
                cur.push('<');
                i += 1;
                continue;
            }
            let mut j = i + 3;
            while j < b.len() && matches!(b[j], b' ' | b'\t' | b'\r' | b'\n') {
                j += 1;
            }
            if j < b.len() && b[j] == b'/' {
                j += 1;
            }
            if j < b.len() && b[j] == b'>' {
                out.push(std::mem::take(&mut cur));
                i = j + 1;
                continue;
            }
            cur.push('<');
            i += 1;
        }
        out.push(cur);
        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    fn write_text_candidate(
        out: &mut String,
        content: &str,
        class: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        task_font_size: f64,
        task_font_family: &str,
    ) {
        let content_esc = escape_xml(content);
        let class_esc = escape_attr(class);
        let font_family_esc = escape_attr(task_font_family);
        let cx = x + width / 2.0;
        let cy = y + height / 2.0;

        out.push_str("<switch>");
        let _ = write!(
            out,
            r#"<foreignObject x="{x}" y="{y}" width="{w}" height="{h}">"#,
            x = fmt(x),
            y = fmt(y),
            w = fmt(width),
            h = fmt(height),
        );
        let _ = write!(
            out,
            r#"<div class="{class}" xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><div class="label" style="display: table-cell; text-align: center; vertical-align: middle;">{text}</div></div>"#,
            class = class_esc,
            text = content_esc
        );
        out.push_str("</foreignObject>");

        let lines = split_html_br_lines(content);
        let n = lines.len().max(1) as f64;
        for (i, line) in lines.into_iter().enumerate() {
            let dy = (i as f64) * task_font_size - (task_font_size * (n - 1.0)) / 2.0;
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="{class}" style="text-anchor: middle; font-size: {fs}px; font-family: {ff};"><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
                x = fmt(cx),
                y = fmt(cy),
                class = class_esc,
                fs = fmt(task_font_size),
                ff = font_family_esc,
                dy = fmt(dy),
                text = escape_xml(&line)
            );
        }

        out.push_str("</switch>");
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
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{min_x} {min_y} {w} {h}" preserveAspectRatio="xMinYMin meet" height="{svg_h}" role="graphics-document document" aria-roledescription="journey"{aria}>"#,
        diagram_id_esc = diagram_id_esc,
        max_w = fmt(layout.width),
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
        svg_h = fmt(layout.svg_height),
        aria = aria,
    );

    if let Some(title) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(title)
        );
    }
    if let Some(desc) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(desc)
        );
    }

    let css = journey_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);
    out.push_str(
        r#"<defs><marker id="arrowhead" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto"><path d="M 0,0 V 4 L6,2 Z"/></marker></defs>"#,
    );

    for item in &layout.actor_legend {
        let _ = write!(
            &mut out,
            r##"<circle cx="{cx}" cy="{cy}" class="actor-{pos}" fill="{fill}" stroke="#000" r="{r}"/>"##,
            cx = fmt(item.circle_cx),
            cy = fmt(item.circle_cy),
            pos = item.pos,
            fill = escape_attr(&item.color),
            r = fmt(item.circle_r),
        );
        for line in &item.label_lines {
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" class="legend"><tspan x="{tx}">{text}</tspan></text>"#,
                x = fmt(line.x),
                y = fmt(line.y),
                tx = fmt(line.tspan_x),
                text = escape_xml(&line.text),
            );
        }
    }

    let mut section_iter = layout.sections.iter();
    let mut last_section: Option<&str> = None;
    for task in &layout.tasks {
        if last_section != Some(task.section.as_str()) {
            let Some(section) = section_iter.next() else {
                break;
            };
            let section_class = format!("journey-section section-type-{}", section.num);
            let _ = write!(
                &mut out,
                r##"<g><rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" rx="3" ry="3" class="{class}"/>"##,
                x = fmt(section.x),
                y = fmt(section.y),
                fill = escape_attr(&section.fill),
                w = fmt(section.width),
                h = fmt(section.height),
                class = escape_attr(&section_class),
            );
            write_text_candidate(
                &mut out,
                &section.section,
                &section_class,
                section.x,
                section.y,
                section.width,
                section.height,
                task_font_size,
                task_font_family,
            );
            out.push_str("</g>");
        }

        last_section = Some(task.section.as_str());

        let _ = write!(
            &mut out,
            r##"<g><line id="{id}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" class="task-line" stroke-width="1px" stroke-dasharray="4 2" stroke="#666"/>"##,
            id = escape_attr(&task.line_id),
            x1 = fmt(task.line_x1),
            y1 = fmt(task.line_y1),
            x2 = fmt(task.line_x2),
            y2 = fmt(task.line_y2),
        );

        let _ = write!(
            &mut out,
            r#"<circle cx="{cx}" cy="{cy}" class="face" r="15" stroke-width="2" overflow="visible"/>"#,
            cx = fmt(task.face_cx),
            cy = fmt(task.face_cy),
        );
        out.push_str("<g>");
        let eye_dx = 15.0 / 3.0;
        let eye_r = 1.5;
        let _ = write!(
            &mut out,
            r##"<circle cx="{cx}" cy="{cy}" r="{r}" stroke-width="2" fill="#666" stroke="#666"/>"##,
            cx = fmt(task.face_cx - eye_dx),
            cy = fmt(task.face_cy - eye_dx),
            r = fmt(eye_r),
        );
        let _ = write!(
            &mut out,
            r##"<circle cx="{cx}" cy="{cy}" r="{r}" stroke-width="2" fill="#666" stroke="#666"/>"##,
            cx = fmt(task.face_cx + eye_dx),
            cy = fmt(task.face_cy - eye_dx),
            r = fmt(eye_r),
        );

        match task.mouth {
            crate::model::JourneyMouthKind::Smile => {
                let _ = write!(
                    &mut out,
                    r#"<path class="mouth" d="M7.5,0A7.5,7.5,0,1,1,-7.5,0L-6.818,0A6.818,6.818,0,1,0,6.818,0Z" transform="translate({x},{y})"/>"#,
                    x = fmt(task.face_cx),
                    y = fmt(task.face_cy + 2.0),
                );
            }
            crate::model::JourneyMouthKind::Sad => {
                let _ = write!(
                    &mut out,
                    r#"<path class="mouth" d="M-7.5,0A7.5,7.5,0,1,1,7.5,0L6.818,0A6.818,6.818,0,1,0,-6.818,0Z" transform="translate({x},{y})"/>"#,
                    x = fmt(task.face_cx),
                    y = fmt(task.face_cy + 7.0),
                );
            }
            crate::model::JourneyMouthKind::Ambivalent => {
                let _ = write!(
                    &mut out,
                    r##"<line class="mouth" stroke="#666" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="1px"/>"##,
                    x1 = fmt(task.face_cx - 5.0),
                    y1 = fmt(task.face_cy + 7.0),
                    x2 = fmt(task.face_cx + 5.0),
                    y2 = fmt(task.face_cy + 7.0),
                );
            }
        }

        out.push_str("</g>");

        let _ = write!(
            &mut out,
            r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" rx="3" ry="3" class="task task-type-{num}"/>"##,
            x = fmt(task.x),
            y = fmt(task.y),
            fill = escape_attr(&task.fill),
            w = fmt(task.width),
            h = fmt(task.height),
            num = task.num,
        );

        for c in &task.actor_circles {
            let _ = write!(
                &mut out,
                r##"<circle cx="{cx}" cy="{cy}" class="actor-{pos}" fill="{fill}" stroke="#000" r="{r}"><title>{title}</title></circle>"##,
                cx = fmt(c.cx),
                cy = fmt(c.cy),
                pos = c.pos,
                fill = escape_attr(&c.color),
                r = fmt(c.r),
                title = escape_xml(&c.actor),
            );
        }

        write_text_candidate(
            &mut out,
            &task.task,
            "task",
            task.x,
            task.y,
            task.width,
            task.height,
            task_font_size,
            task_font_family,
        );

        out.push_str("</g>");
    }

    if let Some(title) = layout.title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="{fs}" font-weight="bold" y="{y}" fill="{fill}" font-family="{ff}">{text}</text>"#,
            x = fmt(layout.title_x),
            fs = escape_attr(title_font_size),
            y = fmt(layout.title_y),
            fill = escape_attr(title_color),
            ff = escape_attr(title_font_family),
            text = escape_xml(title),
        );
    }

    let _ = write!(
        &mut out,
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="url(#arrowhead)"/>"#,
        x1 = fmt(layout.activity_line.x1),
        y1 = fmt(layout.activity_line.y1),
        x2 = fmt(layout.activity_line.x2),
        y2 = fmt(layout.activity_line.y2),
    );

    out.push_str("</svg>\n");
    Ok(out)
}

pub fn render_kanban_diagram_svg(
    layout: &crate::model::KanbanDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{min_x} {min_y} {w} {h}" role="graphics-document document" aria-roledescription="kanban">"#,
        diagram_id_esc = diagram_id_esc,
        max_w = fmt(vb_w),
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
    );

    let css = kanban_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);

    // Mermaid emits a single empty <g/> before the diagram content for kanban.
    out.push_str(r#"<g/>"#);

    out.push_str(r#"<g class="sections">"#);
    for s in &layout.sections {
        let left = s.center_x - s.width / 2.0;
        let label_x = left + (s.width - s.label_width.max(0.0)) / 2.0;

        let _ = write!(
            &mut out,
            r##"<g class="cluster undefined section-{idx}" id="{id}" data-look="classic"><rect style="" rx="{rx}" ry="{ry}" x="{x}" y="{y}" width="{w}" height="{h}"/><g class="cluster-label" transform="translate({lx}, {ly})"><foreignObject width="{lw}" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel"><p>{label}</p></span></div></foreignObject></g></g>"##,
            idx = s.index,
            id = escape_attr(&s.id),
            rx = fmt(s.rx),
            ry = fmt(s.ry),
            x = fmt(left),
            y = fmt(s.rect_y),
            w = fmt(s.width),
            h = fmt(s.rect_height),
            lx = fmt(label_x),
            ly = fmt(s.rect_y),
            lw = fmt(s.label_width.max(0.0)),
            label = escape_xml(&s.label),
        );
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="items">"#);

    fn measure_text_width(text: &str) -> f64 {
        // This width is used for positioning only; DOM parity mode masks numeric drift.
        // Keep it deterministic and stable across platforms.
        let style = crate::text::TextStyle::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        measurer.measure(text, &style).width
    }

    for n in &layout.items {
        let max_w = (n.width - 10.0).max(0.0);
        let rect_x = -n.width / 2.0;
        let rect_y = -n.height / 2.0;

        let has_details_row = n.ticket.is_some() || n.assigned.is_some();
        let top_pad = if has_details_row { 4.0 } else { 10.0 };
        let row1_y = rect_y + top_pad;
        let row2_y = if has_details_row {
            rect_y + top_pad + 24.0
        } else {
            rect_y + 34.0
        };

        let left_x = rect_x + 10.0;
        let assigned_w = n.assigned.as_deref().map(measure_text_width).unwrap_or(0.0);
        let right_x = if assigned_w > 0.0 {
            n.width / 2.0 - 10.0 - assigned_w
        } else {
            n.width / 2.0 - 10.0
        };

        let _ = write!(
            &mut out,
            r##"<g class="node undefined" id="{id}" transform="translate({x}, {y})">"##,
            id = escape_attr(&n.id),
            x = fmt(n.center_x),
            y = fmt(n.center_y),
        );
        let _ = write!(
            &mut out,
            r##"<rect class="basic label-container __APA__" style="" rx="{rx}" ry="{ry}" x="{x}" y="{y}" width="{w}" height="{h}"/>"##,
            rx = fmt(n.rx),
            ry = fmt(n.ry),
            x = fmt(rect_x),
            y = fmt(rect_y),
            w = fmt(n.width),
            h = fmt(n.height),
        );

        fn write_label_group(
            out: &mut String,
            x: f64,
            y: f64,
            max_w: f64,
            text: Option<&str>,
            div_class: Option<&str>,
        ) {
            let (fo_w, fo_h) = match text {
                Some(t) if !t.is_empty() => (measure_text_width(t), 24.0),
                _ => (0.0, 0.0),
            };
            let class_attr = div_class
                .map(|c| format!(r#" class="{}""#, escape_attr(c)))
                .unwrap_or_default();
            let _ = write!(
                out,
                r##"<g class="label" style="text-align:left !important" transform="translate({x}, {y})"><rect/><foreignObject width="{w}" height="{h}"><div style="text-align: center; display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px;" xmlns="http://www.w3.org/1999/xhtml"{class_attr}><span style="text-align:left !important" class="nodeLabel">"##,
                x = fmt(x),
                y = fmt(y),
                w = fmt(fo_w),
                h = fmt(fo_h),
                mw = fmt(max_w),
                class_attr = class_attr
            );
            if let Some(t) = text.filter(|t| !t.is_empty()) {
                let _ = write!(out, r#"<p>{}</p>"#, escape_xml(t));
            }
            out.push_str("</span></div></foreignObject></g>");
        }

        write_label_group(
            &mut out,
            left_x,
            row1_y,
            max_w,
            Some(n.label.as_str()),
            n.icon.as_deref().map(|_| "labelBkg"),
        );
        write_label_group(&mut out, left_x, row2_y, max_w, n.ticket.as_deref(), None);
        write_label_group(
            &mut out,
            right_x,
            row2_y,
            max_w,
            n.assigned.as_deref(),
            None,
        );

        if n.priority.is_some() {
            let _ = write!(
                &mut out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4"/>"#,
                x1 = fmt(rect_x + 2.0),
                y1 = fmt(rect_y + 2.0),
                x2 = fmt(rect_x + 2.0),
                y2 = fmt(rect_y + n.height - 2.0),
            );
        }

        out.push_str("</g>");
    }

    out.push_str("</g>");
    out.push_str("</svg>\n");
    Ok(out)
}

pub fn render_gitgraph_diagram_svg(
    layout: &crate::model::GitGraphDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    const THEME_COLOR_LIMIT: i64 = 8;
    const PX: f64 = 4.0;
    const PY: f64 = 2.0;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let acc_title = semantic
        .get("accTitle")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let acc_descr = semantic
        .get("accDescr")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_end_matches('\n'))
        .filter(|s| !s.is_empty());

    let aria_title_id = format!("chart-title-{diagram_id}");
    let aria_desc_id = format!("chart-desc-{diagram_id}");

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{min_x} {min_y} {w} {h}" role="graphics-document document" aria-roledescription="gitGraph""#,
        diagram_id_esc = diagram_id_esc,
        max_w = fmt(vb_w),
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
    );

    if acc_descr.is_some() {
        let _ = write!(
            &mut out,
            r#" aria-describedby="{}""#,
            escape_attr(&aria_desc_id)
        );
    }
    if acc_title.is_some() {
        let _ = write!(
            &mut out,
            r#" aria-labelledby="{}""#,
            escape_attr(&aria_title_id)
        );
    }
    out.push('>');

    if let Some(t) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="{}">{}</title>"#,
            escape_attr(&aria_title_id),
            escape_xml(t)
        );
    }
    if let Some(d) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="{}">{}</desc>"#,
            escape_attr(&aria_desc_id),
            escape_xml(d)
        );
    }

    let css = gitgraph_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);

    out.push_str(r#"<g/>"#);
    out.push_str(r#"<g class="commit-bullets"/>"#);
    out.push_str(r#"<g class="commit-labels"/>"#);

    let mut branch_idx: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for b in &layout.branches {
        branch_idx.insert(b.name.as_str(), b.index);
    }

    let direction = layout.direction.as_str();

    if layout.show_branches {
        out.push_str("<g>");
        for b in &layout.branches {
            let idx = b.index % THEME_COLOR_LIMIT;
            let pos = b.pos;

            if direction == "TB" {
                let _ = write!(
                    &mut out,
                    r#"<line x1="{x1}" y1="30" x2="{x2}" y2="{y2}" class="branch branch{idx}"/>"#,
                    x1 = fmt(pos),
                    x2 = fmt(pos),
                    y2 = fmt(layout.max_pos),
                    idx = idx
                );
            } else if direction == "BT" {
                let _ = write!(
                    &mut out,
                    r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="30" class="branch branch{idx}"/>"#,
                    x1 = fmt(pos),
                    y1 = fmt(layout.max_pos),
                    x2 = fmt(pos),
                    idx = idx
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<line x1="0" y1="{y1}" x2="{x2}" y2="{y2}" class="branch branch{idx}"/>"#,
                    y1 = fmt(pos),
                    x2 = fmt(layout.max_pos),
                    y2 = fmt(pos),
                    idx = idx
                );
            }

            let name = escape_xml(&b.name);
            let bbox_w = b.bbox_width.max(0.0);
            let bbox_h = b.bbox_height.max(0.0);

            let bkg_class = format!(r#"branchLabelBkg label{idx}"#);
            let label_class = format!(r#"label branch-label{idx}"#);

            if direction == "TB" {
                let x = pos - bbox_w / 2.0 - 10.0;
                let _ = write!(
                    &mut out,
                    r#"<rect class="{cls}" rx="4" ry="4" x="{x}" y="0" width="{w}" height="{h}"/>"#,
                    cls = bkg_class,
                    x = fmt(x),
                    w = fmt(bbox_w + 18.0),
                    h = fmt(bbox_h + 4.0),
                );
                let tx = pos - bbox_w / 2.0 - 5.0;
                let _ = write!(
                    &mut out,
                    r#"<g class="branchLabel"><g class="{cls}" transform="translate({x}, 0)"><text><tspan xml:space="preserve" dy="1em" x="0" class="row">{name}</tspan></text></g></g>"#,
                    cls = label_class,
                    x = fmt(tx),
                    name = name
                );
            } else if direction == "BT" {
                let x = pos - bbox_w / 2.0 - 10.0;
                let _ = write!(
                    &mut out,
                    r#"<rect class="{cls}" rx="4" ry="4" x="{x}" y="{y}" width="{w}" height="{h}"/>"#,
                    cls = bkg_class,
                    x = fmt(x),
                    y = fmt(layout.max_pos),
                    w = fmt(bbox_w + 18.0),
                    h = fmt(bbox_h + 4.0),
                );
                let tx = pos - bbox_w / 2.0 - 5.0;
                let _ = write!(
                    &mut out,
                    r#"<g class="branchLabel"><g class="{cls}" transform="translate({x}, {y})"><text><tspan xml:space="preserve" dy="1em" x="0" class="row">{name}</tspan></text></g></g>"#,
                    cls = label_class,
                    x = fmt(tx),
                    y = fmt(layout.max_pos),
                    name = name
                );
            } else {
                let rotate_pad = if layout.rotate_commit_label {
                    30.0
                } else {
                    0.0
                };
                let x = -bbox_w - 4.0 - rotate_pad;
                let y = -bbox_h / 2.0 + 8.0;
                let _ = write!(
                    &mut out,
                    r#"<rect class="{cls}" rx="4" ry="4" x="{x}" y="{y}" width="{w}" height="{h}" transform="translate(-19, {ty})"/>"#,
                    cls = bkg_class,
                    x = fmt(x),
                    y = fmt(y),
                    w = fmt(bbox_w + 18.0),
                    h = fmt(bbox_h + 4.0),
                    ty = fmt(pos - bbox_h / 2.0),
                );
                let tx = -bbox_w - 14.0 - rotate_pad;
                let _ = write!(
                    &mut out,
                    r#"<g class="branchLabel"><g class="{cls}" transform="translate({x}, {y})"><text><tspan xml:space="preserve" dy="1em" x="0" class="row">{name}</tspan></text></g></g>"#,
                    cls = label_class,
                    x = fmt(tx),
                    y = fmt(pos - bbox_h / 2.0 - 1.0),
                    name = name
                );
            }
        }
        out.push_str("</g>");
    }

    out.push_str(r#"<g class="commit-arrows">"#);
    for a in &layout.arrows {
        let _ = write!(
            &mut out,
            r#"<path d="{d}" class="arrow arrow{idx}"/>"#,
            d = escape_attr(&a.d),
            idx = a.class_index % THEME_COLOR_LIMIT
        );
    }
    out.push_str("</g>");

    fn commit_class_type(symbol_type: i64) -> &'static str {
        match symbol_type {
            0 => "commit-normal",
            1 => "commit-reverse",
            2 => "commit-highlight",
            3 => "commit-merge",
            4 => "commit-cherry-pick",
            _ => "commit-normal",
        }
    }

    fn commit_symbol_type(commit: &crate::model::GitGraphCommitLayout) -> i64 {
        commit.custom_type.unwrap_or(commit.commit_type)
    }

    out.push_str(r#"<g class="commit-bullets">"#);
    for c in &layout.commits {
        let branch_i = branch_idx.get(c.branch.as_str()).copied().unwrap_or(0);
        let symbol_type = commit_symbol_type(c);
        let type_class = commit_class_type(symbol_type);
        let idx = branch_i % THEME_COLOR_LIMIT;
        let id = escape_attr(&c.id);

        if symbol_type == 2 {
            let _ = write!(
                &mut out,
                r#"<rect x="{x}" y="{y}" width="20" height="20" class="commit {id} commit-highlight{idx} {type_class}-outer"/>"#,
                x = fmt(c.x - 10.0),
                y = fmt(c.y - 10.0),
                id = id,
                idx = idx,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r#"<rect x="{x}" y="{y}" width="12" height="12" class="commit {id} commit{idx} {type_class}-inner"/>"#,
                x = fmt(c.x - 6.0),
                y = fmt(c.y - 6.0),
                id = id,
                idx = idx,
                type_class = type_class
            );
        } else if symbol_type == 4 {
            let _ = write!(
                &mut out,
                r#"<circle cx="{x}" cy="{y}" r="10" class="commit {id} {type_class}"/>"#,
                x = fmt(c.x),
                y = fmt(c.y),
                id = id,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r##"<circle cx="{x}" cy="{y}" r="2.75" fill="#fff" class="commit {id} {type_class}"/>"##,
                x = fmt(c.x - 3.0),
                y = fmt(c.y + 2.0),
                id = id,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r##"<circle cx="{x}" cy="{y}" r="2.75" fill="#fff" class="commit {id} {type_class}"/>"##,
                x = fmt(c.x + 3.0),
                y = fmt(c.y + 2.0),
                id = id,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r##"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#fff" class="commit {id} {type_class}"/>"##,
                x1 = fmt(c.x + 3.0),
                y1 = fmt(c.y + 1.0),
                x2 = fmt(c.x),
                y2 = fmt(c.y - 5.0),
                id = id,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r##"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#fff" class="commit {id} {type_class}"/>"##,
                x1 = fmt(c.x - 3.0),
                y1 = fmt(c.y + 1.0),
                x2 = fmt(c.x),
                y2 = fmt(c.y - 5.0),
                id = id,
                type_class = type_class
            );
        } else {
            let r = if c.commit_type == 3 { 9.0 } else { 10.0 };
            let _ = write!(
                &mut out,
                r#"<circle cx="{x}" cy="{y}" r="{r}" class="commit {id} commit{idx}"/>"#,
                x = fmt(c.x),
                y = fmt(c.y),
                r = fmt(r),
                id = id,
                idx = idx
            );
            if symbol_type == 3 {
                let _ = write!(
                    &mut out,
                    r#"<circle cx="{x}" cy="{y}" r="6" class="commit {type_class} {id} commit{idx}"/>"#,
                    x = fmt(c.x),
                    y = fmt(c.y),
                    type_class = type_class,
                    id = id,
                    idx = idx
                );
            }
            if symbol_type == 1 {
                let d = format!(
                    "M {},{}L{},{}M {},{}L{},{}",
                    fmt(c.x - 5.0),
                    fmt(c.y - 5.0),
                    fmt(c.x + 5.0),
                    fmt(c.y + 5.0),
                    fmt(c.x - 5.0),
                    fmt(c.y + 5.0),
                    fmt(c.x + 5.0),
                    fmt(c.y - 5.0)
                );
                let _ = write!(
                    &mut out,
                    r#"<path d="{d}" class="commit {type_class} {id} commit{idx}"/>"#,
                    d = escape_attr(&d),
                    type_class = type_class,
                    id = id,
                    idx = idx
                );
            }
        }
    }
    out.push_str("</g>");

    let measurer = crate::text::DeterministicTextMeasurer::default();
    let commit_label_style = crate::text::TextStyle {
        font_family: None,
        font_size: 10.0,
        font_weight: None,
    };

    out.push_str(r#"<g class="commit-labels">"#);
    for c in &layout.commits {
        let show = layout.show_commit_label
            && c.commit_type != 4
            && ((c.custom_id.unwrap_or(false) && c.commit_type == 3) || c.commit_type != 3);
        if show {
            let bbox = measurer.measure(&c.id, &commit_label_style);
            let bbox_w = bbox.width.max(0.0);
            let bbox_h = bbox.height.max(0.0);

            let mut wrapper_transform: Option<String> = None;
            let mut rect_transform: Option<String> = None;
            let mut text_transform: Option<String> = None;

            let mut rect_x = c.pos_with_offset - bbox_w / 2.0 - PY;
            let mut rect_y = c.y + 13.5;
            let rect_w = bbox_w + 2.0 * PY;
            let rect_h = bbox_h + 2.0 * PY;
            let mut text_x = c.pos_with_offset - bbox_w / 2.0;
            let mut text_y = c.y + 25.0;

            if direction == "TB" || direction == "BT" {
                rect_x = c.x - (bbox_w + 4.0 * PX + 5.0);
                rect_y = c.y - 12.0;
                text_x = c.x - (bbox_w + 4.0 * PX);
                text_y = c.y + bbox_h - 12.0;
            }

            if layout.rotate_commit_label {
                if direction == "TB" || direction == "BT" {
                    let t = format!("rotate(-45, {}, {})", fmt(c.x), fmt(c.y));
                    rect_transform = Some(t.clone());
                    text_transform = Some(t);
                } else {
                    let r_x = -7.5 - ((bbox_w + 10.0) / 25.0) * 9.5;
                    let r_y = 10.0 + (bbox_w / 25.0) * 8.5;
                    wrapper_transform = Some(format!(
                        "translate({}, {}) rotate(-45, {}, {})",
                        fmt(r_x),
                        fmt(r_y),
                        fmt(c.pos),
                        fmt(c.y)
                    ));
                }
            }

            out.push_str("<g");
            if let Some(t) = &wrapper_transform {
                let _ = write!(&mut out, r#" transform="{}""#, escape_attr(t));
            }
            out.push('>');

            out.push_str(r#"<rect class="commit-label-bkg""#);
            let _ = write!(
                &mut out,
                r#" x="{}" y="{}" width="{}" height="{}""#,
                fmt(rect_x),
                fmt(rect_y),
                fmt(rect_w),
                fmt(rect_h)
            );
            if let Some(t) = &rect_transform {
                let _ = write!(&mut out, r#" transform="{}""#, escape_attr(t));
            }
            out.push_str("/>");

            out.push_str(r#"<text class="commit-label""#);
            let _ = write!(&mut out, r#" x="{}" y="{}""#, fmt(text_x), fmt(text_y));
            if let Some(t) = &text_transform {
                let _ = write!(&mut out, r#" transform="{}""#, escape_attr(t));
            }
            let _ = write!(&mut out, ">{}</text>", escape_xml(&c.id));
            out.push_str("</g>");
        }

        if !c.tags.is_empty() {
            let mut y_offset = 0.0;
            let mut max_w: f64 = 0.0;
            let mut max_h: f64 = 0.0;
            let mut tag_values = c.tags.clone();
            tag_values.reverse();

            struct TagGeom {
                y_offset: f64,
            }
            let mut elems: Vec<TagGeom> = Vec::new();
            for tag_value in &tag_values {
                let bbox = measurer.measure(tag_value, &commit_label_style);
                max_w = max_w.max(bbox.width.max(0.0));
                max_h = max_h.max(bbox.height.max(0.0));
                elems.push(TagGeom { y_offset });
                y_offset += 20.0;
            }

            for (i, tag_value) in tag_values.iter().enumerate() {
                let y_off = elems.get(i).map(|e| e.y_offset).unwrap_or(0.0);
                let h2 = max_h / 2.0;
                let ly = c.y - 19.2 - y_off;

                if direction == "TB" || direction == "BT" {
                    let y_origin = c.pos + y_off;
                    let points = format!(
                        "{} {} {} {} {} {} {} {} {} {} {} {}",
                        fmt(c.x),
                        fmt(y_origin + 2.0),
                        fmt(c.x),
                        fmt(y_origin - 2.0),
                        fmt(c.x + 10.0),
                        fmt(y_origin - h2 - 2.0),
                        fmt(c.x + 10.0 + max_w + 4.0),
                        fmt(y_origin - h2 - 2.0),
                        fmt(c.x + 10.0 + max_w + 4.0),
                        fmt(y_origin + h2 + 2.0),
                        fmt(c.x + 10.0),
                        fmt(y_origin + h2 + 2.0)
                    );
                    let poly_t =
                        format!("translate(12,12) rotate(45, {},{})", fmt(c.x), fmt(c.pos));
                    let hole_t =
                        format!("translate(12,12) rotate(45, {},{})", fmt(c.x), fmt(c.pos));
                    let text_t =
                        format!("translate(14,14) rotate(45, {},{})", fmt(c.x), fmt(c.pos));

                    let _ = write!(
                        &mut out,
                        r#"<polygon class="tag-label-bkg" points="{pts}" transform="{t}"/>"#,
                        pts = escape_attr(&points),
                        t = escape_attr(&poly_t)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<circle cy="{cy}" cx="{cx}" r="1.5" class="tag-hole" transform="{t}"/>"#,
                        cy = fmt(y_origin),
                        cx = fmt(c.x + PX / 2.0),
                        t = escape_attr(&hole_t)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<text y="{y}" class="tag-label" x="{x}" transform="{t}">{txt}</text>"#,
                        y = fmt(y_origin + 3.0),
                        x = fmt(c.x + 5.0),
                        t = escape_attr(&text_t),
                        txt = escape_xml(tag_value)
                    );
                } else {
                    let points = format!(
                        "{} {} {} {} {} {} {} {} {} {} {} {}",
                        fmt(c.pos - max_w / 2.0 - PX / 2.0),
                        fmt(ly + PY),
                        fmt(c.pos - max_w / 2.0 - PX / 2.0),
                        fmt(ly - PY),
                        fmt(c.pos_with_offset - max_w / 2.0 - PX),
                        fmt(ly - h2 - PY),
                        fmt(c.pos_with_offset + max_w / 2.0 + PX),
                        fmt(ly - h2 - PY),
                        fmt(c.pos_with_offset + max_w / 2.0 + PX),
                        fmt(ly + h2 + PY),
                        fmt(c.pos_with_offset - max_w / 2.0 - PX),
                        fmt(ly + h2 + PY)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<polygon class="tag-label-bkg" points="{pts}"/>"#,
                        pts = escape_attr(&points)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<circle cy="{cy}" cx="{cx}" r="1.5" class="tag-hole"/>"#,
                        cy = fmt(ly),
                        cx = fmt(c.pos - max_w / 2.0 + PX / 2.0)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<text y="{y}" class="tag-label" x="{x}">{txt}</text>"#,
                        y = fmt(c.y - 16.0 - y_off),
                        x = fmt(c.pos_with_offset - max_w / 2.0),
                        txt = escape_xml(tag_value)
                    );
                }
            }
        }
    }
    out.push_str("</g>");

    out.push_str("</svg>\n");
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
struct GanttSemanticTask {
    id: String,
    #[serde(rename = "type")]
    task_type: String,
    #[serde(default)]
    classes: Vec<String>,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    crit: bool,
    #[serde(default)]
    milestone: bool,
    #[serde(default)]
    vert: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct GanttSemanticModel {
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    acc_descr: Option<String>,
    #[serde(default, rename = "todayMarker")]
    today_marker: Option<String>,
    #[serde(default)]
    tasks: Vec<GanttSemanticTask>,
}

fn gantt_section_num(task_type: &str, categories: &[String], number_section_styles: i64) -> i64 {
    if number_section_styles <= 0 {
        return 0;
    }
    for (idx, c) in categories.iter().enumerate() {
        if c == task_type {
            return (idx as i64) % number_section_styles;
        }
    }
    0
}

fn gantt_scale_time_round(ms: i64, min_ms: i64, max_ms: i64, range: f64) -> f64 {
    if max_ms <= min_ms {
        // D3 scaleTime returns the midpoint of the range for degenerate domains.
        return (range / 2.0).round();
    }
    let t = (ms - min_ms) as f64 / (max_ms - min_ms) as f64;
    (t * range).round()
}

fn gantt_start_of_day_ms(ms: i64) -> Option<i64> {
    let dt_utc = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
    let dt = dt_utc.with_timezone(&chrono::Local);
    let d = dt.date_naive();
    let local = chrono::Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
        .single()?;
    Some(local.with_timezone(&chrono::Utc).timestamp_millis())
}

fn fmt_allow_nan(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    fmt(v)
}

fn gantt_is_unsafe_rect_id(id: &str) -> bool {
    matches!(id, "__proto__" | "constructor" | "prototype")
}

fn gantt_insert_before_width(base: &str, insert: &str) -> String {
    let insert = insert.trim();
    if insert.is_empty() {
        return base.to_string();
    }
    let mut parts: Vec<&str> = base.split_whitespace().collect();
    let insert_parts: Vec<&str> = insert.split_whitespace().collect();
    let idx = parts.iter().position(|p| p.starts_with("width-"));
    match idx {
        Some(i) => {
            for (off, p) in insert_parts.iter().enumerate() {
                parts.insert(i + off, p);
            }
        }
        None => parts.extend(insert_parts),
    }
    parts.join(" ")
}

fn render_gantt_axis_group(
    out: &mut String,
    layout: &crate::model::GanttDiagramLayout,
    ticks: &[crate::model::GanttAxisTickLayout],
    y: f64,
    with_dy: bool,
) {
    let range = (layout.width - layout.left_padding - layout.right_padding).max(1.0);
    let tick_size = -layout.height + layout.top_padding + layout.grid_line_start_padding;

    let _ = write!(
        out,
        r#"<g class="grid" transform="translate({}, {})" fill="none" font-size="10" font-family="sans-serif" text-anchor="middle">"#,
        fmt(layout.left_padding),
        fmt(y)
    );

    let d = format!(
        "M0.5,{}V0.5H{}V{}",
        fmt(tick_size),
        fmt(range + 0.5),
        fmt(tick_size)
    );
    let _ = write!(
        out,
        r#"<path class="domain" stroke="currentColor" d="{}"/>"#,
        escape_attr(&d)
    );

    for t in ticks {
        let tx = (t.x - layout.left_padding) + 0.5;
        let _ = write!(
            out,
            r#"<g class="tick" opacity="1" transform="translate({},0)">"#,
            fmt(tx)
        );
        let _ = write!(
            out,
            r#"<line stroke="currentColor" y2="{}"/>"#,
            fmt(tick_size)
        );
        if with_dy {
            let _ = write!(
                out,
                r##"<text fill="#000" y="3" dy="1em" stroke="none" font-size="10" style="text-anchor: middle;">{}</text>"##,
                escape_xml(&t.label)
            );
        } else {
            let _ = write!(
                out,
                r##"<text fill="#000" y="3" stroke="none" font-size="10" style="text-anchor: middle;">{}</text>"##,
                escape_xml(&t.label)
            );
        }
        out.push_str("</g>");
    }

    out.push_str("</g>");
}

pub fn render_gantt_diagram_svg(
    layout: &crate::model::GanttDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: GanttSemanticModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let w = layout.width.max(1.0);
    let h = layout.height.max(1.0);

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

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {w} {h}" style="max-width: {max_w}px; background-color: white;" role="graphics-document document" aria-roledescription="gantt"{aria_describedby}{aria_labelledby}>"#,
        diagram_id_esc = diagram_id_esc,
        w = fmt(w),
        h = fmt(h),
        max_w = fmt(w),
        aria_describedby = acc_descr
            .as_ref()
            .map(|_| format!(r#" aria-describedby="chart-desc-{diagram_id_esc}""#))
            .unwrap_or_default(),
        aria_labelledby = acc_title
            .as_ref()
            .map(|_| format!(r#" aria-labelledby="chart-title-{diagram_id_esc}""#))
            .unwrap_or_default(),
    );

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(title)
        );
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(descr)
        );
    }

    let css = gantt_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);

    let (min_ms, max_ms) = match (
        layout.tasks.iter().map(|t| t.start_ms).min(),
        layout.tasks.iter().map(|t| t.end_ms).max(),
    ) {
        (Some(a), Some(b)) => (a, b),
        _ => (0, 0),
    };
    let range = (w - layout.left_padding - layout.right_padding).max(1.0);
    let gap = layout.bar_height + layout.bar_gap;

    // Exclude layer (drawn before the grid in Mermaid).
    if layout.has_excludes_layer {
        if layout.excludes.is_empty() {
            out.push_str("<g/>");
        } else {
            out.push_str("<g>");
            for (i, r) in layout.excludes.iter().enumerate() {
                let end_start_ms = gantt_start_of_day_ms(r.end_ms).unwrap_or(r.end_ms);
                let start_x = gantt_scale_time_round(r.start_ms, min_ms, max_ms, range);
                let end_x = gantt_scale_time_round(end_start_ms, min_ms, max_ms, range);
                let cx = start_x + layout.left_padding + 0.5 * (end_x - start_x);
                let cy = (i as f64) * gap + 0.5 * h;

                let _ = write!(
                    &mut out,
                    r#"<rect id="{id}" x="{x}" y="{y}" width="{w}" height="{h}" transform-origin="{cx}px {cy}px" class="exclude-range"/>"#,
                    id = escape_attr(&r.id),
                    x = fmt(r.x),
                    y = fmt(r.y),
                    w = fmt(r.width),
                    h = fmt(r.height),
                    cx = fmt_allow_nan(cx),
                    cy = fmt_allow_nan(cy),
                );
            }
            out.push_str("</g>");
        }
    }

    let bottom_axis_y = h - layout.top_padding;
    render_gantt_axis_group(&mut out, layout, &layout.bottom_ticks, bottom_axis_y, true);

    if layout.top_axis {
        render_gantt_axis_group(
            &mut out,
            layout,
            &layout.top_ticks,
            layout.top_padding,
            false,
        );
    }

    if layout.rows.is_empty() {
        out.push_str("<g/>");
    } else {
        out.push_str("<g>");
        for r in &layout.rows {
            let _ = write!(
                &mut out,
                r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" class="{cls}"/>"#,
                x = fmt(r.x),
                y = fmt(r.y),
                w = fmt(r.width),
                h = fmt(r.height),
                cls = escape_attr(&r.class),
            );
        }
        out.push_str("</g>");
    }

    let mut tasks_in_draw_order: Vec<(usize, &crate::model::GanttTaskLayout)> =
        layout.tasks.iter().enumerate().collect();
    tasks_in_draw_order.sort_by(|(ai, a), (bi, b)| a.vert.cmp(&b.vert).then(ai.cmp(bi)));

    let mut semantic_task_by_id: std::collections::HashMap<&str, &GanttSemanticTask> =
        std::collections::HashMap::new();
    for t in &model.tasks {
        semantic_task_by_id.insert(t.id.as_str(), t);
    }

    if layout.tasks.is_empty() {
        out.push_str("<g/>");
    } else {
        out.push_str("<g>");

        for (_idx, t) in &tasks_in_draw_order {
            let start_x = gantt_scale_time_round(t.start_ms, min_ms, max_ms, range);
            let end_x = gantt_scale_time_round(t.end_ms, min_ms, max_ms, range);
            let center_x = start_x + layout.left_padding + 0.5 * (end_x - start_x);
            let center_y = (t.order as f64) * gap + layout.top_padding + 0.5 * layout.bar_height;
            let origin = format!(
                "{}px {}px",
                fmt_allow_nan(center_x),
                fmt_allow_nan(center_y)
            );

            let _ = write!(&mut out, r#"<rect"#);
            if !gantt_is_unsafe_rect_id(&t.id) {
                let _ = write!(&mut out, r#" id="{}""#, escape_attr(&t.bar.id));
            }
            let _ = write!(
                &mut out,
                r#" rx="{rx}" ry="{ry}" x="{x}" y="{y}" width="{w}" height="{h}" transform-origin="{origin}" class="{cls}"/>"#,
                rx = fmt(t.bar.rx),
                ry = fmt(t.bar.ry),
                x = fmt(t.bar.x),
                y = fmt(t.bar.y),
                w = fmt(t.bar.width),
                h = fmt(t.bar.height),
                origin = escape_attr(&origin),
                cls = escape_attr(&t.bar.class),
            );
        }

        for (_idx, t) in &tasks_in_draw_order {
            let base_class = &t.label.class;
            let mut task_type_class = String::new();
            if let Some(st) = semantic_task_by_id.get(t.id.as_str()) {
                let sec_num = gantt_section_num(
                    &st.task_type,
                    &layout.categories,
                    layout.number_section_styles,
                );
                if st.active {
                    if st.crit {
                        task_type_class = format!("activeCritText{sec_num}");
                    } else {
                        task_type_class = format!("activeText{sec_num}");
                    }
                }
                if st.done {
                    if st.crit {
                        if !task_type_class.is_empty() {
                            task_type_class.push(' ');
                        }
                        task_type_class.push_str(&format!("doneCritText{sec_num}"));
                    } else {
                        if !task_type_class.is_empty() {
                            task_type_class.push(' ');
                        }
                        task_type_class.push_str(&format!("doneText{sec_num}"));
                    }
                } else if st.crit {
                    if !task_type_class.is_empty() {
                        task_type_class.push(' ');
                    }
                    task_type_class.push_str(&format!("critText{sec_num}"));
                }

                if st.milestone {
                    if !task_type_class.is_empty() {
                        task_type_class.push(' ');
                    }
                    task_type_class.push_str("milestoneText");
                }

                if st.vert {
                    if !task_type_class.is_empty() {
                        task_type_class.push(' ');
                    }
                    task_type_class.push_str("vertText");
                }
            }

            let class = gantt_insert_before_width(base_class, &task_type_class);
            let _ = write!(
                &mut out,
                r#"<text id="{id}" font-size="{fs}" x="{x}" y="{y}" class="{cls}">{txt}</text>"#,
                id = escape_attr(&t.label.id),
                fs = fmt(t.label.font_size),
                x = fmt(t.label.x),
                y = fmt(t.label.y),
                cls = escape_attr(&class),
                txt = escape_xml(&t.label.text),
            );
        }

        out.push_str("</g>");
    }

    if layout.section_titles.is_empty() {
        out.push_str("<g/>");
    } else {
        out.push_str("<g>");
        for st in &layout.section_titles {
            let _ = write!(
                &mut out,
                r#"<text dy="{dy}em" x="{x}" y="{y}" font-size="{fs}" class="{cls}">"#,
                dy = fmt(st.dy_em),
                x = fmt(st.x),
                y = fmt(st.y),
                fs = fmt(layout.section_font_size),
                cls = escape_attr(&st.class),
            );
            for (j, line) in st.lines.iter().enumerate() {
                if j == 0 {
                    let _ = write!(
                        &mut out,
                        r#"<tspan alignment-baseline="central" x="{x}">{txt}</tspan>"#,
                        x = fmt(st.x),
                        txt = escape_xml(line)
                    );
                } else {
                    let _ = write!(
                        &mut out,
                        r#"<tspan alignment-baseline="central" x="{x}" dy="1em">{txt}</tspan>"#,
                        x = fmt(st.x),
                        txt = escape_xml(line)
                    );
                }
            }
            out.push_str("</text>");
        }
        out.push_str("</g>");
    }

    if model.today_marker.as_deref().unwrap_or("").trim() != "off" {
        let today_x = if layout.tasks.is_empty() {
            f64::NAN
        } else {
            let now_ms = chrono::Local::now().timestamp_millis();
            gantt_scale_time_round(now_ms, min_ms, max_ms, range) + layout.left_padding
        };
        let y1 = layout.title_top_margin;
        let y2 = h - layout.title_top_margin;
        out.push_str(r#"<g class="today">"#);
        let _ = write!(
            &mut out,
            r#"<line x1="{x}" x2="{x}" y1="{y1}" y2="{y2}" class="today""#,
            x = fmt_allow_nan(today_x),
            y1 = fmt(y1),
            y2 = fmt(y2),
        );
        let style = model.today_marker.as_deref().unwrap_or("").trim();
        if !style.is_empty() && style != "off" {
            let style = style.replace(',', ";");
            let _ = write!(&mut out, r#" style="{}""#, escape_attr(&style));
        }
        out.push_str("/></g>");
    }

    let title = model.title.unwrap_or_default();
    let _ = write!(
        &mut out,
        r#"<text x="{x}" y="{y}" class="titleText">{txt}</text>"#,
        x = fmt(layout.title_x),
        y = fmt(layout.title_y),
        txt = escape_xml(&title),
    );

    out.push_str("</svg>\n");
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModelText {
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModelShape {
    alias: String,
    #[serde(default, rename = "bgColor")]
    bg_color: Option<String>,
    #[serde(default, rename = "borderColor")]
    border_color: Option<String>,
    #[serde(default, rename = "fontColor")]
    font_color: Option<String>,
    #[serde(default)]
    sprite: Option<serde_json::Value>,
    #[serde(default, rename = "typeC4Shape")]
    type_c4_shape: Option<C4SvgModelText>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModelBoundary {
    alias: String,
    #[serde(default, rename = "nodeType")]
    node_type: Option<String>,
    #[serde(default, rename = "bgColor")]
    bg_color: Option<String>,
    #[serde(default, rename = "borderColor")]
    border_color: Option<String>,
    #[serde(default, rename = "fontColor")]
    font_color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModelRel {
    #[serde(rename = "from")]
    from_alias: String,
    #[serde(rename = "to")]
    to_alias: String,
    #[serde(default, rename = "lineColor")]
    line_color: Option<String>,
    #[serde(default, rename = "textColor")]
    text_color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModel {
    #[serde(default, rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    acc_descr: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    shapes: Vec<C4SvgModelShape>,
    #[serde(default)]
    boundaries: Vec<C4SvgModelBoundary>,
    #[serde(default)]
    rels: Vec<C4SvgModelRel>,
}

fn c4_css(diagram_id: &str) -> String {
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
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}#{} .person{{stroke:hsl(240, 60%, 86.2745098039%);fill:#ECECFF;}}#{} :root{{--mermaid-font-family:{};}}"#,
        id, font, id, id, id, font
    );
    out
}

fn c4_config_string(cfg: &serde_json::Value, key: &str) -> Option<String> {
    config_string(cfg, &["c4", key])
}

fn c4_config_color(cfg: &serde_json::Value, key: &str, fallback: &str) -> String {
    c4_config_string(cfg, key).unwrap_or_else(|| fallback.to_string())
}

fn c4_config_font_family(cfg: &serde_json::Value, type_key: &str) -> String {
    c4_config_string(cfg, &format!("{type_key}FontFamily"))
        .map(|s| normalize_css_font_family(&s))
        .unwrap_or_else(|| r#""Open Sans", sans-serif"#.to_string())
}

fn c4_config_font_size(cfg: &serde_json::Value, type_key: &str, fallback: f64) -> f64 {
    config_f64(cfg, &["c4", &format!("{type_key}FontSize")]).unwrap_or(fallback)
}

fn c4_config_font_weight(cfg: &serde_json::Value, type_key: &str) -> String {
    c4_config_string(cfg, &format!("{type_key}FontWeight")).unwrap_or_else(|| "normal".to_string())
}

fn c4_write_text_by_tspan(
    out: &mut String,
    content: &str,
    x: f64,
    y: f64,
    width: f64,
    font_family: &str,
    font_size: f64,
    font_weight: &str,
    attrs: &[(&str, &str)],
) {
    let x = x + width / 2.0;
    let mut style = String::new();
    let _ = write!(
        &mut style,
        "text-anchor: middle; font-size: {}px; font-weight: {}; font-family: {};",
        fmt(font_size.max(1.0)),
        font_weight,
        font_family
    );

    let _ = write!(
        out,
        r#"<text x="{}" y="{}" dominant-baseline="middle""#,
        fmt(x),
        fmt(y)
    );
    for (k, v) in attrs {
        let _ = write!(out, r#" {k}="{v}""#);
    }
    let _ = write!(out, r#" style="{}">"#, escape_attr(&style));

    let lines: Vec<&str> = content.split('\n').collect();
    let n = lines.len().max(1) as f64;
    for (i, line) in lines.iter().enumerate() {
        let dy = (i as f64) * font_size - (font_size * (n - 1.0)) / 2.0;
        let dy_s = fmt(dy);
        let _ = write!(
            out,
            r#"<tspan dy="{}" alignment-baseline="mathematical">{}</tspan>"#,
            dy_s,
            escape_xml(line)
        );
    }
    out.push_str("</text>");
}

pub fn render_c4_diagram_svg(
    layout: &crate::model::C4DiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: C4SvgModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let diagram_margin_x = config_f64(effective_config, &["c4", "diagramMarginX"]).unwrap_or(50.0);
    let diagram_margin_y = config_f64(effective_config, &["c4", "diagramMarginY"]).unwrap_or(10.0);
    let use_max_width = effective_config
        .get("c4")
        .and_then(|v| v.get("useMaxWidth"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: diagram_margin_x,
        min_y: diagram_margin_y,
        max_x: diagram_margin_x + layout.width.max(1.0),
        max_y: diagram_margin_y + layout.height.max(1.0),
    });
    let box_w = (bounds.max_x - bounds.min_x).max(1.0);
    let box_h = (bounds.max_y - bounds.min_y).max(1.0);
    let width = (box_w + 2.0 * diagram_margin_x).max(1.0);
    let height = (box_h + 2.0 * diagram_margin_y).max(1.0);

    let title = diagram_title
        .map(|s| s.to_string())
        .or_else(|| layout.title.clone())
        .or_else(|| model.title.clone())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let extra_vert_for_title = if title.is_some() { 60.0 } else { 0.0 };

    let viewbox_x = bounds.min_x - diagram_margin_x;
    let viewbox_y = -(diagram_margin_y + extra_vert_for_title);

    let aria_roledescription = options.aria_roledescription.as_deref().unwrap_or("c4");

    let mut out = String::new();
    if use_max_width {
        let _ = write!(
            &mut out,
            r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{vb_x} {vb_y} {vb_w} {vb_h}" role="graphics-document document" aria-roledescription="{aria}"{aria_describedby}{aria_labelledby}>"#,
            diagram_id_esc = diagram_id_esc,
            max_w = fmt(width),
            vb_x = fmt(viewbox_x),
            vb_y = fmt(viewbox_y),
            vb_w = fmt(width),
            vb_h = fmt(height + extra_vert_for_title),
            aria = escape_attr(aria_roledescription),
            aria_describedby = model
                .acc_descr
                .as_ref()
                .map(|s| s.trim_end_matches('\n'))
                .filter(|s| !s.trim().is_empty())
                .map(|_| format!(r#" aria-describedby="chart-desc-{diagram_id_esc}""#))
                .unwrap_or_default(),
            aria_labelledby = model
                .acc_title
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|_| format!(r#" aria-labelledby="chart-title-{diagram_id_esc}""#))
                .unwrap_or_default(),
        );
    } else {
        let _ = write!(
            &mut out,
            r#"<svg id="{diagram_id_esc}" width="{w}" height="{h}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="background-color: white;" viewBox="{vb_x} {vb_y} {vb_w} {vb_h}" role="graphics-document document" aria-roledescription="{aria}"{aria_describedby}{aria_labelledby}>"#,
            diagram_id_esc = diagram_id_esc,
            w = fmt(width),
            h = fmt(height + extra_vert_for_title),
            vb_x = fmt(viewbox_x),
            vb_y = fmt(viewbox_y),
            vb_w = fmt(width),
            vb_h = fmt(height + extra_vert_for_title),
            aria = escape_attr(aria_roledescription),
            aria_describedby = model
                .acc_descr
                .as_ref()
                .map(|s| s.trim_end_matches('\n'))
                .filter(|s| !s.trim().is_empty())
                .map(|_| format!(r#" aria-describedby="chart-desc-{diagram_id_esc}""#))
                .unwrap_or_default(),
            aria_labelledby = model
                .acc_title
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|_| format!(r#" aria-labelledby="chart-title-{diagram_id_esc}""#))
                .unwrap_or_default(),
        );
    }

    if let Some(title) = model
        .acc_title
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(title)
        );
    }
    if let Some(descr) = model
        .acc_descr
        .as_deref()
        .map(|s| s.trim_end_matches('\n'))
        .filter(|s| !s.trim().is_empty())
    {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(descr)
        );
    }

    let css = c4_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str("<g/>");

    const C4_DATABASE_SYMBOL_D_11_12_2: &str = include_str!("../assets/c4_database_d_11_12_2.txt");

    out.push_str(
        r#"<defs><symbol id="computer" width="24" height="24"><path transform="scale(.5)" d="M2 2v13h20v-13h-20zm18 11h-16v-9h16v9zm-10.228 6l.466-1h3.524l.467 1h-4.457zm14.228 3h-24l2-6h2.104l-1.33 4h18.45l-1.297-4h2.073l2 6zm-5-10h-14v-7h14v7z"/></symbol></defs>"#,
    );
    out.push_str(
        &format!(
            r#"<defs><symbol id="database" fill-rule="evenodd" clip-rule="evenodd"><path transform="scale(.5)" d="{}"/></symbol></defs>"#,
            escape_attr(C4_DATABASE_SYMBOL_D_11_12_2.trim())
        ),
    );
    out.push_str(
        r#"<defs><symbol id="clock" width="24" height="24"><path transform="scale(.5)" d="M12 2c5.514 0 10 4.486 10 10s-4.486 10-10 10-10-4.486-10-10 4.486-10 10-10zm0-2c-6.627 0-12 5.373-12 12s5.373 12 12 12 12-5.373 12-12-5.373-12-12-12zm5.848 12.459c.202.038.202.333.001.372-1.907.361-6.045 1.111-6.547 1.111-.719 0-1.301-.582-1.301-1.301 0-.512.77-5.447 1.125-7.445.034-.192.312-.181.343.014l.985 6.238 5.394 1.011z"/></symbol></defs>"#,
    );

    let mut shape_meta: std::collections::HashMap<&str, &C4SvgModelShape> =
        std::collections::HashMap::new();
    for s in &model.shapes {
        shape_meta.insert(s.alias.as_str(), s);
    }
    let mut boundary_meta: std::collections::HashMap<&str, &C4SvgModelBoundary> =
        std::collections::HashMap::new();
    for b in &model.boundaries {
        boundary_meta.insert(b.alias.as_str(), b);
    }
    let mut rel_meta: std::collections::HashMap<(&str, &str), &C4SvgModelRel> =
        std::collections::HashMap::new();
    for r in &model.rels {
        rel_meta.insert((r.from_alias.as_str(), r.to_alias.as_str()), r);
    }

    const PERSON_IMG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADAAAAAwCAIAAADYYG7QAAACD0lEQVR4Xu2YoU4EMRCGT+4j8Ai8AhaH4QHgAUjQuFMECUgMIUgwJAgMhgQsAYUiJCiQIBBY+EITsjfTdme6V24v4c8vyGbb+ZjOtN0bNcvjQXmkH83WvYBWto6PLm6v7p7uH1/w2fXD+PBycX1Pv2l3IdDm/vn7x+dXQiAubRzoURa7gRZWd0iGRIiJbOnhnfYBQZNJjNbuyY2eJG8fkDE3bbG4ep6MHUAsgYxmE3nVs6VsBWJSGccsOlFPmLIViMzLOB7pCVO2AtHJMohH7Fh6zqitQK7m0rJvAVYgGcEpe//PLdDz65sM4pF9N7ICcXDKIB5Nv6j7tD0NoSdM2QrU9Gg0ewE1LqBhHR3BBdvj2vapnidjHxD/q6vd7Pvhr31AwcY8eXMTXAKECZZJFXuEq27aLgQK5uLMohCenGGuGewOxSjBvYBqeG6B+Nqiblggdjnc+ZXDy+FNFpFzw76O3UBAROuXh6FoiAcf5g9eTvUgzy0nWg6I8cXHRUpg5bOVBCo+KDpFajOf23GgPme7RSQ+lacIENUgJ6gg1k6HjgOlqnLqip4tEuhv0hNEMXUD0clyXE3p6pZA0S2nnvTlXwLJEZWlb7cTQH1+USgTN4VhAenm/wea1OCAOmqo6fE1WCb9WSKBah+rbUWPWAmE2Rvk0ApiB45eOyNAzU8xcTvj8KvkKEoOaIYeHNA3ZuygAvFMUO0AAAAASUVORK5CYII=";
    const EXTERNAL_PERSON_IMG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADAAAAAwCAIAAADYYG7QAAAB6ElEQVR4Xu2YLY+EMBCG9+dWr0aj0Wg0Go1Go0+j8Xdv2uTCvv1gpt0ebHKPuhDaeW4605Z9mJvx4AdXUyTUdd08z+u6flmWZRnHsWkafk9DptAwDPu+f0eAYtu2PEaGWuj5fCIZrBAC2eLBAnRCsEkkxmeaJp7iDJ2QMDdHsLg8SxKFEJaAo8lAXnmuOFIhTMpxxKATebo4UiFknuNo4OniSIXQyRxEA3YsnjGCVEjVXD7yLUAqxBGUyPv/Y4W2beMgGuS7kVQIBycH0fD+oi5pezQETxdHKmQKGk1eQEYldK+jw5GxPfZ9z7Mk0Qnhf1W1m3w//EUn5BDmSZsbR44QQLBEqrBHqOrmSKaQAxdnLArCrxZcM7A7ZKs4ioRq8LFC+NpC3WCBJsvpVw5edm9iEXFuyNfxXAgSwfrFQ1c0iNda8AdejvUgnktOtJQQxmcfFzGglc5WVCj7oDgFqU18boeFSs52CUh8LE8BIVQDT1ABrB0HtgSEYlX5doJnCwv9TXocKCaKbnwhdDKPq4lf3SwU3HLq4V/+WYhHVMa/3b4IlfyikAduCkcBc7mQ3/z/Qq/cTuikhkzB12Ae/mcJC9U+Vo8Ej1gWAtgbeGgFsAMHr50BIWOLCbezvhpBFUdY6EJuJ/QDW0XoMX60zZ0AAAAASUVORK5CYII=";

    for s in &layout.shapes {
        let meta = shape_meta.get(s.alias.as_str()).copied();
        let bg_color = meta.and_then(|m| m.bg_color.clone()).unwrap_or_else(|| {
            c4_config_color(
                effective_config,
                &format!("{}_bg_color", s.type_c4_shape),
                "#08427B",
            )
        });
        let border_color = meta
            .and_then(|m| m.border_color.clone())
            .unwrap_or_else(|| {
                c4_config_color(
                    effective_config,
                    &format!("{}_border_color", s.type_c4_shape),
                    "#073B6F",
                )
            });
        let font_color = meta
            .and_then(|m| m.font_color.clone())
            .unwrap_or_else(|| "#FFFFFF".to_string());

        out.push_str(r#"<g class="person-man">"#);

        match s.type_c4_shape.as_str() {
            "system_db"
            | "external_system_db"
            | "container_db"
            | "external_container_db"
            | "component_db"
            | "external_component_db" => {
                let half = s.width / 2.0;
                let d1 = format!(
                    "M{},{}c0,-10 {},-10 {},-10c0,0 {},0 {},10l0,{}c0,10 -{},10 -{},10c0,0 -{},0 -{},-10l0,-{}",
                    fmt(s.x),
                    fmt(s.y),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(s.height),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(s.height)
                );
                let d2 = format!(
                    "M{},{}c0,10 {},10 {},10c0,0 {},0 {},-10",
                    fmt(s.x),
                    fmt(s.y),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(half)
                );
                let _ = write!(
                    &mut out,
                    r#"<path fill="{}" stroke-width="0.5" stroke="{}" d="{}"/>"#,
                    escape_attr(&bg_color),
                    escape_attr(&border_color),
                    escape_attr(&d1)
                );
                let _ = write!(
                    &mut out,
                    r#"<path fill="none" stroke-width="0.5" stroke="{}" d="{}"/>"#,
                    escape_attr(&border_color),
                    escape_attr(&d2)
                );
            }
            "system_queue"
            | "external_system_queue"
            | "container_queue"
            | "external_container_queue"
            | "component_queue"
            | "external_component_queue" => {
                let half = s.height / 2.0;
                let d1 = format!(
                    "M{},{}l{},0c5,0 5,{} 5,{}c0,0 0,{} -5,{}l-{},0c-5,0 -5,-{} -5,-{}c0,0 0,-{} 5,-{}",
                    fmt(s.x),
                    fmt(s.y),
                    fmt(s.width),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(s.width),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                );
                let d2 = format!(
                    "M{},{}c-5,0 -5,{} -5,{}c0,{} 5,{} 5,{}",
                    fmt(s.x + s.width),
                    fmt(s.y),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(half),
                    fmt(half)
                );
                let _ = write!(
                    &mut out,
                    r#"<path fill="{}" stroke-width="0.5" stroke="{}" d="{}"/>"#,
                    escape_attr(&bg_color),
                    escape_attr(&border_color),
                    escape_attr(&d1)
                );
                let _ = write!(
                    &mut out,
                    r#"<path fill="none" stroke-width="0.5" stroke="{}" d="{}"/>"#,
                    escape_attr(&border_color),
                    escape_attr(&d2)
                );
            }
            _ => {
                let _ = write!(
                    &mut out,
                    r#"<rect x="{}" y="{}" fill="{}" stroke="{}" width="{}" height="{}" rx="2.5" ry="2.5" stroke-width="0.5"/>"#,
                    fmt(s.x),
                    fmt(s.y),
                    escape_attr(&bg_color),
                    escape_attr(&border_color),
                    fmt(s.width),
                    fmt(s.height)
                );
            }
        }

        let type_family = c4_config_font_family(effective_config, &s.type_c4_shape);
        let type_size = c4_config_font_size(effective_config, &s.type_c4_shape, 14.0) - 2.0;
        let type_text_length =
            crate::generated::c4_type_textlength_11_12_2::c4_type_text_length_px_11_12_2(
                &s.type_c4_shape,
            )
            .unwrap_or_else(|| s.type_block.width.round().max(0.0));
        let _ = write!(
            &mut out,
            r#"<text fill="{}" font-family="{}" font-size="{}" font-style="italic" lengthAdjust="spacing" textLength="{}" x="{}" y="{}">{}</text>"#,
            escape_attr(&font_color),
            escape_attr(&type_family),
            fmt(type_size.max(1.0)),
            fmt(type_text_length),
            fmt(s.x + s.width / 2.0 - type_text_length / 2.0),
            fmt(s.y + s.type_block.y),
            escape_xml(&format!("<<{}>>", s.type_c4_shape))
        );

        if matches!(s.type_c4_shape.as_str(), "person" | "external_person") {
            let href = if s.type_c4_shape == "external_person" {
                EXTERNAL_PERSON_IMG
            } else {
                PERSON_IMG
            };
            let _ = write!(
                &mut out,
                r#"<image width="48" height="48" x="{}" y="{}" xlink:href="{}"/>"#,
                fmt(s.x + s.width / 2.0 - 24.0),
                fmt(s.y + s.image.y),
                escape_attr(href)
            );
        } else if meta.is_some_and(|m| m.sprite.is_some()) {
            let _ = write!(
                &mut out,
                r#"<image width="48" height="48" x="{}" y="{}" xlink:href="{}"/>"#,
                fmt(s.x + s.width / 2.0 - 24.0),
                fmt(s.y + s.image.y),
                escape_attr(PERSON_IMG)
            );
        }

        let label_family = c4_config_font_family(effective_config, &s.type_c4_shape);
        let label_weight = "bold";
        let label_size = c4_config_font_size(effective_config, &s.type_c4_shape, 14.0) + 2.0;
        c4_write_text_by_tspan(
            &mut out,
            &s.label.text,
            s.x,
            s.y + s.label.y,
            s.width,
            &label_family,
            label_size,
            label_weight,
            &[("fill", &font_color)],
        );

        let body_family = c4_config_font_family(effective_config, &s.type_c4_shape);
        let body_weight = c4_config_font_weight(effective_config, &s.type_c4_shape);
        let body_size = c4_config_font_size(effective_config, &s.type_c4_shape, 14.0);

        if let Some(techn) = &s.techn {
            if !techn.text.trim().is_empty() {
                c4_write_text_by_tspan(
                    &mut out,
                    &techn.text,
                    s.x,
                    s.y + techn.y,
                    s.width,
                    &body_family,
                    body_size,
                    &body_weight,
                    &[("fill", &font_color), ("font-style", "italic")],
                );
            }
        } else if let Some(ty) = &s.ty {
            if !ty.text.trim().is_empty() {
                c4_write_text_by_tspan(
                    &mut out,
                    &ty.text,
                    s.x,
                    s.y + ty.y,
                    s.width,
                    &body_family,
                    body_size,
                    &body_weight,
                    &[("fill", &font_color), ("font-style", "italic")],
                );
            }
        }

        if let Some(descr) = &s.descr {
            if !descr.text.trim().is_empty() {
                let descr_family = c4_config_font_family(effective_config, "person");
                let descr_weight = c4_config_font_weight(effective_config, "person");
                let descr_size = c4_config_font_size(effective_config, "person", 14.0);
                c4_write_text_by_tspan(
                    &mut out,
                    &descr.text,
                    s.x,
                    s.y + descr.y,
                    s.width,
                    &descr_family,
                    descr_size,
                    &descr_weight,
                    &[("fill", &font_color)],
                );
            }
        }

        out.push_str("</g>");
    }

    for b in &layout.boundaries {
        if b.alias == "global" {
            continue;
        }
        let meta = boundary_meta.get(b.alias.as_str()).copied();
        let fill_color = meta
            .and_then(|m| m.bg_color.clone())
            .unwrap_or_else(|| "none".to_string());
        let stroke_color = meta
            .and_then(|m| m.border_color.clone())
            .unwrap_or_else(|| "#444444".to_string());
        let is_node_type = meta.and_then(|m| m.node_type.as_deref()).is_some();

        out.push_str("<g>");
        if is_node_type {
            let _ = write!(
                &mut out,
                r#"<rect x="{}" y="{}" fill="{}" stroke="{}" width="{}" height="{}" rx="2.5" ry="2.5" stroke-width="1"/>"#,
                fmt(b.x),
                fmt(b.y),
                escape_attr(&fill_color),
                escape_attr(&stroke_color),
                fmt(b.width),
                fmt(b.height)
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<rect x="{}" y="{}" fill="{}" stroke="{}" width="{}" height="{}" rx="2.5" ry="2.5" stroke-width="1" stroke-dasharray="7.0,7.0"/>"#,
                fmt(b.x),
                fmt(b.y),
                escape_attr(&fill_color),
                escape_attr(&stroke_color),
                fmt(b.width),
                fmt(b.height)
            );
        }

        let boundary_family = c4_config_font_family(effective_config, "boundary");
        let boundary_weight = "bold";
        let boundary_size = c4_config_font_size(effective_config, "boundary", 14.0) + 2.0;
        c4_write_text_by_tspan(
            &mut out,
            &b.label.text,
            b.x,
            b.y + b.label.y,
            b.width,
            &boundary_family,
            boundary_size,
            boundary_weight,
            &[("fill", "#444444")],
        );
        if let Some(ty) = &b.ty {
            if !ty.text.trim().is_empty() {
                let boundary_type_weight = c4_config_font_weight(effective_config, "boundary");
                let boundary_type_size = c4_config_font_size(effective_config, "boundary", 14.0);
                c4_write_text_by_tspan(
                    &mut out,
                    &ty.text,
                    b.x,
                    b.y + ty.y,
                    b.width,
                    &boundary_family,
                    boundary_type_size,
                    &boundary_type_weight,
                    &[("fill", "#444444")],
                );
            }
        }
        if let Some(descr) = &b.descr {
            if !descr.text.trim().is_empty() {
                let descr_weight = c4_config_font_weight(effective_config, "boundary");
                let descr_size =
                    (c4_config_font_size(effective_config, "boundary", 14.0) - 2.0).max(1.0);
                c4_write_text_by_tspan(
                    &mut out,
                    &descr.text,
                    b.x,
                    b.y + descr.y,
                    b.width,
                    &boundary_family,
                    descr_size,
                    &descr_weight,
                    &[("fill", "#444444")],
                );
            }
        }

        out.push_str("</g>");
    }

    out.push_str(r#"<defs><marker id="arrowhead" refX="9" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z"/></marker></defs>"#);
    out.push_str(r#"<defs><marker id="arrowend" refX="1" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 10 0 L 0 5 L 10 10 z"/></marker></defs>"#);
    out.push_str(r##"<defs><marker id="crosshead" markerWidth="15" markerHeight="8" orient="auto" refX="16" refY="4"><path fill="black" stroke="#000000" stroke-width="1px" d="M 9,2 V 6 L16,4 Z" style="stroke-dasharray: 0, 0;"/><path fill="none" stroke="#000000" stroke-width="1px" d="M 0,1 L 6,7 M 6,1 L 0,7" style="stroke-dasharray: 0, 0;"/></marker></defs>"##);
    out.push_str(r#"<defs><marker id="filled-head" refX="18" refY="7" markerWidth="20" markerHeight="28" orient="auto"><path d="M 18,7 L9,13 L14,7 L9,1 Z"/></marker></defs>"#);

    out.push_str("<g>");
    for (idx, rel) in layout.rels.iter().enumerate() {
        let meta = rel_meta.get(&(rel.from.as_str(), rel.to.as_str())).copied();
        let text_color = meta
            .and_then(|m| m.text_color.clone())
            .unwrap_or_else(|| "#444444".to_string());
        let stroke_color = meta
            .and_then(|m| m.line_color.clone())
            .unwrap_or_else(|| "#444444".to_string());
        let offset_x = rel.offset_x.unwrap_or(0) as f64;
        let offset_y = rel.offset_y.unwrap_or(0) as f64;

        if idx == 0 {
            let _ = write!(
                &mut out,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="1" stroke="{}""#,
                fmt(rel.start_point.x),
                fmt(rel.start_point.y),
                fmt(rel.end_point.x),
                fmt(rel.end_point.y),
                escape_attr(&stroke_color)
            );
            if rel.rel_type != "rel_b" {
                out.push_str(r#" marker-end="url(#arrowhead)""#);
            }
            if rel.rel_type == "birel" || rel.rel_type == "rel_b" {
                out.push_str(r#" marker-start="url(#arrowend)""#);
            }
            out.push_str(r#" style="fill: none;"/>"#);
        } else {
            let cx = rel.start_point.x + (rel.end_point.x - rel.start_point.x) / 2.0
                - (rel.end_point.x - rel.start_point.x) / 4.0;
            let cy = rel.start_point.y + (rel.end_point.y - rel.start_point.y) / 2.0;
            let d = format!(
                "M{} {} Q{} {} {} {}",
                fmt(rel.start_point.x),
                fmt(rel.start_point.y),
                fmt(cx),
                fmt(cy),
                fmt(rel.end_point.x),
                fmt(rel.end_point.y)
            );
            let _ = write!(
                &mut out,
                r#"<path fill="none" stroke-width="1" stroke="{}" d="{}""#,
                escape_attr(&stroke_color),
                escape_attr(&d)
            );
            if rel.rel_type != "rel_b" {
                out.push_str(r#" marker-end="url(#arrowhead)""#);
            }
            if rel.rel_type == "birel" || rel.rel_type == "rel_b" {
                out.push_str(r#" marker-start="url(#arrowend)""#);
            }
            out.push_str("/>");
        }

        let midx = rel.start_point.x.min(rel.end_point.x)
            + (rel.end_point.x - rel.start_point.x).abs() / 2.0
            + offset_x;
        let midy = rel.start_point.y.min(rel.end_point.y)
            + (rel.end_point.y - rel.start_point.y).abs() / 2.0
            + offset_y;

        let message_family = c4_config_font_family(effective_config, "message");
        let message_weight = c4_config_font_weight(effective_config, "message");
        let message_size = c4_config_font_size(effective_config, "message", 12.0);
        c4_write_text_by_tspan(
            &mut out,
            &rel.label.text,
            midx,
            midy,
            rel.label.width,
            &message_family,
            message_size,
            &message_weight,
            &[("fill", &text_color)],
        );

        if let Some(techn) = &rel.techn {
            if !techn.text.trim().is_empty() {
                c4_write_text_by_tspan(
                    &mut out,
                    &format!("[{}]", techn.text),
                    midx,
                    midy + message_size + 5.0,
                    rel.label.width.max(techn.width),
                    &message_family,
                    message_size,
                    &message_weight,
                    &[("fill", &text_color), ("font-style", "italic")],
                );
            }
        }
    }
    out.push_str("</g>");

    if let Some(title) = title {
        let title_x = (width - 2.0 * diagram_margin_x) / 2.0 - 4.0 * diagram_margin_x;
        let title_y = bounds.min_y + diagram_margin_y;
        let _ = write!(
            &mut out,
            r#"<text x="{}" y="{}">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            escape_xml(&title)
        );
    }

    out.push_str("</svg>");
    Ok(out)
}

pub fn render_flowchart_v2_svg(
    layout: &FlowchartV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: crate::flowchart::FlowchartV2Model = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_type = "flowchart-v2";

    // Mermaid expands self-loop edges into a chain of helper nodes plus `*-cyclic-special-*` edge
    // segments during Dagre layout. Replicate that expansion here so rendered SVG ids match.
    let mut render_edges: Vec<crate::flowchart::FlowEdge> = Vec::new();
    let mut self_loop_label_node_ids: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for e in &model.edges {
        if e.from != e.to {
            render_edges.push(e.clone());
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
        edge_mid.label = None;
        edge_mid.label_type = None;
        edge_mid.edge_type = Some("arrow_open".to_string());

        let mut edge2 = e.clone();
        edge2.id = format!("{node_id}-cyclic-special-2");
        edge2.from = special_id_2.clone();
        edge2.to = node_id.clone();
        edge2.label = None;
        edge2.label_type = None;

        render_edges.push(edge1);
        render_edges.push(edge_mid);
        render_edges.push(edge2);
    }

    let font_family = config_string(effective_config, &["fontFamily"])
        .map(|s| normalize_css_font_family(&s))
        .unwrap_or_else(|| "\"trebuchet ms\", verdana, arial, sans-serif".to_string());
    let font_size = effective_config
        .get("fontSize")
        .and_then(|v| v.as_f64())
        .unwrap_or(16.0)
        .max(1.0);

    let wrapping_width = config_f64(effective_config, &["flowchart", "wrappingWidth"])
        .unwrap_or(200.0)
        .max(1.0);
    let html_labels = effective_config
        .get("flowchart")
        .and_then(|v| v.get("htmlLabels"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let wrap_mode = if html_labels {
        crate::text::WrapMode::HtmlLike
    } else {
        crate::text::WrapMode::SvgLike
    };
    let diagram_padding = config_f64(effective_config, &["flowchart", "diagramPadding"])
        .unwrap_or(8.0)
        .max(0.0);
    let use_max_width = effective_config
        .get("flowchart")
        .and_then(|v| v.get("useMaxWidth"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let title_top_margin = config_f64(effective_config, &["flowchart", "titleTopMargin"])
        .unwrap_or(25.0)
        .max(0.0);
    let node_padding = config_f64(effective_config, &["flowchart", "padding"])
        .unwrap_or(15.0)
        .max(0.0);

    let text_style = crate::text::TextStyle {
        font_family: Some(font_family.clone()),
        font_size,
        font_weight: None,
    };

    let mut nodes_by_id: std::collections::HashMap<String, crate::flowchart::FlowNode> =
        std::collections::HashMap::new();
    let node_order: Vec<String> = model.nodes.iter().map(|n| n.id.clone()).collect();
    for n in model.nodes.iter().cloned() {
        nodes_by_id.insert(n.id.clone(), n);
    }
    for id in &self_loop_label_node_ids {
        nodes_by_id
            .entry(id.clone())
            .or_insert(crate::flowchart::FlowNode {
                id: id.clone(),
                label: Some(String::new()),
                label_type: None,
                layout_shape: None,
                classes: Vec::new(),
                styles: Vec::new(),
                have_callback: false,
                link: None,
                link_target: None,
            });
    }

    let mut edges_by_id: std::collections::HashMap<String, crate::flowchart::FlowEdge> =
        std::collections::HashMap::new();
    for e in render_edges.iter().cloned() {
        edges_by_id.insert(e.id.clone(), e);
    }

    let mut subgraphs_by_id: std::collections::HashMap<String, crate::flowchart::FlowSubgraph> =
        std::collections::HashMap::new();
    let subgraph_order: Vec<String> = model.subgraphs.iter().map(|s| s.id.clone()).collect();
    for sg in model.subgraphs.iter().cloned() {
        subgraphs_by_id.insert(sg.id.clone(), sg);
    }

    let mut parent: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for sg in model.subgraphs.iter() {
        for child in &sg.nodes {
            parent.insert(child.clone(), sg.id.clone());
        }
    }
    for id in &self_loop_label_node_ids {
        let Some((base, _)) = id.split_once("---") else {
            continue;
        };
        if let Some(p) = parent.get(base).cloned() {
            parent.insert(id.clone(), p);
        }
    }

    let mut recursive_clusters: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for sg in model.subgraphs.iter() {
        if sg.nodes.is_empty() {
            continue;
        }
        let mut external = false;
        for e in render_edges.iter() {
            // Match Mermaid `adjustClustersAndEdges`: a cluster is considered to have external
            // connections if an edge crosses its descendant boundary. Edges that connect directly
            // to the cluster node itself do not count.
            let from_in = flowchart_is_strict_descendant(&parent, &e.from, &sg.id);
            let to_in = flowchart_is_strict_descendant(&parent, &e.to, &sg.id);
            if from_in != to_in {
                external = true;
                break;
            }
        }
        if !external {
            recursive_clusters.insert(sg.id.clone());
        }
    }

    let mut layout_nodes_by_id: std::collections::HashMap<String, LayoutNode> =
        std::collections::HashMap::new();
    for n in layout.nodes.iter().cloned() {
        layout_nodes_by_id.insert(n.id.clone(), n);
    }

    let mut layout_edges_by_id: std::collections::HashMap<String, crate::model::LayoutEdge> =
        std::collections::HashMap::new();
    for e in layout.edges.iter().cloned() {
        layout_edges_by_id.insert(e.id.clone(), e);
    }

    let mut layout_clusters_by_id: std::collections::HashMap<String, LayoutCluster> =
        std::collections::HashMap::new();
    for c in layout.clusters.iter().cloned() {
        layout_clusters_by_id.insert(c.id.clone(), c);
    }

    let bounds =
        compute_layout_bounds(&layout.clusters, &layout.nodes, &layout.edges).unwrap_or(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        });
    let content_w = (bounds.max_x - bounds.min_x).max(1.0);
    let content_h = (bounds.max_y - bounds.min_y).max(1.0);
    let vb_w = content_w + diagram_padding * 2.0;
    let vb_h = content_h + diagram_padding * 2.0;
    let tx = diagram_padding - bounds.min_x;
    let ty = diagram_padding - bounds.min_y;

    let node_dom_index = flowchart_node_dom_indices(&model);

    let css = flowchart_css(
        diagram_id,
        effective_config,
        &font_family,
        font_size,
        &model.class_defs,
    );

    let node_border_color = theme_color(effective_config, "nodeBorder", "#9370DB");
    let node_fill_color = theme_color(effective_config, "mainBkg", "#ECECFF");

    let mut out = String::new();
    let w_attr = fmt(vb_w.max(1.0));
    let h_attr = fmt(vb_h.max(1.0));

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
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));

    let svg_open = if use_max_width {
        format!(
            r#"<svg id="{}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="flowchart" style="max-width: {}px; background-color: white;" viewBox="0 0 {} {}" role="graphics-document document" aria-roledescription="{}"{}{}>"#,
            escape_xml(diagram_id),
            w_attr,
            w_attr,
            h_attr,
            diagram_type,
            aria_describedby
                .as_deref()
                .map(|id| format!(r#" aria-describedby="{}""#, escape_attr(id)))
                .unwrap_or_default(),
            aria_labelledby
                .as_deref()
                .map(|id| format!(r#" aria-labelledby="{}""#, escape_attr(id)))
                .unwrap_or_default(),
        )
    } else {
        format!(
            r#"<svg id="{}" width="{}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="flowchart" height="{}" viewBox="0 0 {} {}" role="graphics-document document" aria-roledescription="{}"{}{}>"#,
            escape_xml(diagram_id),
            w_attr,
            h_attr,
            w_attr,
            h_attr,
            diagram_type,
            aria_describedby
                .as_deref()
                .map(|id| format!(r#" aria-describedby="{}""#, escape_attr(id)))
                .unwrap_or_default(),
            aria_labelledby
                .as_deref()
                .map(|id| format!(r#" aria-labelledby="{}""#, escape_attr(id)))
                .unwrap_or_default(),
        )
    };
    out.push_str(&svg_open);
    if let (Some(id), Some(title)) = (aria_labelledby.as_deref(), acc_title) {
        let _ = write!(
            &mut out,
            r#"<title id="{}">{}</title>"#,
            escape_attr(id),
            escape_xml(title)
        );
    }
    if let (Some(id), Some(descr)) = (aria_describedby.as_deref(), acc_descr) {
        let _ = write!(
            &mut out,
            r#"<desc id="{}">{}</desc>"#,
            escape_attr(id),
            escape_xml(descr)
        );
    }
    let _ = write!(&mut out, "<style>{}</style>", css);

    out.push_str("<g>");
    flowchart_markers(&mut out, diagram_id);

    let default_edge_interpolate = model
        .edge_defaults
        .as_ref()
        .and_then(|d| d.interpolate.as_deref())
        .unwrap_or("basis")
        .to_string();

    let ctx = FlowchartRenderCtx {
        diagram_id: diagram_id.to_string(),
        tx,
        ty,
        diagram_type: diagram_type.to_string(),
        measurer,
        config: merman_core::MermaidConfig::from_value(effective_config.clone()),
        html_labels,
        class_defs: model.class_defs.clone(),
        node_border_color,
        node_fill_color,
        default_edge_interpolate,
        node_order,
        subgraph_order,
        nodes_by_id,
        edges_by_id,
        subgraphs_by_id,
        tooltips: model.tooltips.clone(),
        recursive_clusters,
        parent,
        layout_nodes_by_id,
        layout_edges_by_id,
        layout_clusters_by_id,
        node_dom_index,
        node_padding,
        wrapping_width,
        wrap_mode,
        text_style,
        diagram_title: diagram_title.map(|s| s.to_string()),
    };

    render_flowchart_root(&mut out, &ctx, None, 0.0, 0.0);

    out.push_str("</g>");
    if let Some(title) = diagram_title.map(str::trim).filter(|t| !t.is_empty()) {
        let title_x = vb_w / 2.0;
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
    Ok(out)
}

pub fn render_state_diagram_v2_svg(
    layout: &StateDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: StateSvgModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");

    let bounds =
        compute_layout_bounds(&layout.clusters, &layout.nodes, &layout.edges).unwrap_or(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        });
    let diagram_padding = config_f64(effective_config, &["state", "diagramPadding"])
        .unwrap_or(0.0)
        .max(0.0);
    let vb_min_x = (bounds.min_x - diagram_padding).min(bounds.max_x);
    let vb_min_y = (bounds.min_y - diagram_padding).min(bounds.max_y);
    let vb_w = (bounds.max_x - bounds.min_x + diagram_padding * 2.0).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y + diagram_padding * 2.0).max(1.0);

    let has_acc_title = model
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = model
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="statediagram" style="max-width: {}px; background-color: white;" viewBox="{} {} {} {}" role="graphics-document document" aria-roledescription="stateDiagram""#,
        escape_xml(diagram_id),
        fmt(vb_w),
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w),
        fmt(vb_h)
    );
    if has_acc_title {
        let _ = write!(
            &mut out,
            r#" aria-labelledby="chart-title-{}""#,
            escape_xml(diagram_id)
        );
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#" aria-describedby="chart-desc-{}""#,
            escape_xml(diagram_id)
        );
    }
    out.push('>');

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}"#,
            escape_xml(diagram_id),
            escape_xml(model.acc_title.as_deref().unwrap_or_default())
        );
        out.push_str("</title>");
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}"#,
            escape_xml(diagram_id),
            escape_xml(model.acc_descr.as_deref().unwrap_or_default())
        );
        out.push_str("</desc>");
    }

    // Mermaid emits a single `<style>` element with diagram-scoped CSS.
    let _ = write!(&mut out, "<style>{}</style>", "");

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");
    state_markers(&mut out, diagram_id);

    let text_style = crate::state::state_text_style(effective_config);

    let mut nodes_by_id: std::collections::HashMap<&str, &StateSvgNode> =
        std::collections::HashMap::new();
    for n in &model.nodes {
        nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut layout_nodes_by_id: std::collections::HashMap<&str, &LayoutNode> =
        std::collections::HashMap::new();
    for n in &layout.nodes {
        layout_nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut layout_edges_by_id: std::collections::HashMap<&str, &crate::model::LayoutEdge> =
        std::collections::HashMap::new();
    for e in &layout.edges {
        layout_edges_by_id.insert(e.id.as_str(), e);
    }

    let mut layout_clusters_by_id: std::collections::HashMap<&str, &LayoutCluster> =
        std::collections::HashMap::new();
    for c in &layout.clusters {
        layout_clusters_by_id.insert(c.id.as_str(), c);
    }

    let mut parent: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for n in &model.nodes {
        if let Some(p) = n.parent_id.as_deref() {
            parent.insert(n.id.as_str(), p);
        }
    }

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

    let mut ctx = StateRenderCtx {
        diagram_id: diagram_id.to_string(),
        diagram_title: diagram_title.map(|s| s.to_string()),
        nodes_by_id,
        layout_nodes_by_id,
        layout_edges_by_id,
        layout_clusters_by_id,
        parent,
        nested_roots: std::collections::BTreeSet::new(),
        hidden_prefixes,
        links: &model.links,
        states: &model.states,
        edges: &model.edges,
        include_edges: options.include_edges,
        include_nodes: options.include_nodes,
        measurer,
        text_style,
    };

    fn compute_state_nested_roots(ctx: &StateRenderCtx<'_>) -> std::collections::BTreeSet<String> {
        let mut out: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for e in ctx.edges {
            if state_is_hidden(ctx, e.start.as_str())
                || state_is_hidden(ctx, e.end.as_str())
                || state_is_hidden(ctx, e.id.as_str())
            {
                continue;
            }
            let Some(c) = state_edge_context_raw(ctx, e) else {
                continue;
            };
            out.insert(c.to_string());
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
                    out.insert(pid.to_string());
                }
                cur = Some(pid);
            }
        }

        out
    }

    ctx.nested_roots = compute_state_nested_roots(&ctx);

    render_state_root(&mut out, &ctx, None, 0.0, 0.0);

    out.push_str("</g></svg>\n");
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
struct StateSvgModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub nodes: Vec<StateSvgNode>,
    #[serde(default)]
    pub edges: Vec<StateSvgEdge>,
    #[serde(default)]
    pub links: std::collections::HashMap<String, StateSvgLink>,
    #[serde(default)]
    pub states: std::collections::HashMap<String, StateSvgState>,
}

#[derive(Debug, Clone, Deserialize)]
struct StateSvgState {
    #[serde(default)]
    pub note: Option<StateSvgNote>,
}

#[derive(Debug, Clone, Deserialize)]
struct StateSvgNote {
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct StateSvgLink {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub tooltip: String,
}

#[derive(Debug, Clone, Deserialize)]
struct StateSvgNode {
    pub id: String,
    #[serde(default)]
    pub label: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<Vec<String>>,
    #[serde(default, rename = "domId")]
    pub dom_id: String,
    #[serde(default, rename = "isGroup")]
    pub is_group: bool,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default, rename = "cssClasses")]
    pub css_classes: String,
    pub shape: String,
}

#[derive(Debug, Clone, Deserialize)]
struct StateSvgEdge {
    pub id: String,
    #[serde(rename = "start")]
    pub start: String,
    #[serde(rename = "end")]
    pub end: String,
    #[serde(default)]
    pub classes: String,
    #[serde(default, rename = "arrowTypeEnd")]
    pub arrow_type_end: String,
    #[serde(default)]
    pub label: String,
}

struct StateRenderCtx<'a> {
    diagram_id: String,
    #[allow(dead_code)]
    diagram_title: Option<String>,
    nodes_by_id: std::collections::HashMap<&'a str, &'a StateSvgNode>,
    layout_nodes_by_id: std::collections::HashMap<&'a str, &'a LayoutNode>,
    layout_edges_by_id: std::collections::HashMap<&'a str, &'a crate::model::LayoutEdge>,
    layout_clusters_by_id: std::collections::HashMap<&'a str, &'a LayoutCluster>,
    parent: std::collections::HashMap<&'a str, &'a str>,
    nested_roots: std::collections::BTreeSet<String>,
    hidden_prefixes: Vec<String>,
    links: &'a std::collections::HashMap<String, StateSvgLink>,
    states: &'a std::collections::HashMap<String, StateSvgState>,
    edges: &'a [StateSvgEdge],
    include_edges: bool,
    include_nodes: bool,
    measurer: &'a dyn TextMeasurer,
    text_style: crate::text::TextStyle,
}

fn state_markers(out: &mut String, diagram_id: &str) {
    let diagram_id = escape_xml(diagram_id);
    let _ = write!(
        out,
        r#"<defs><marker id="{diagram_id}_stateDiagram-barbEnd" refX="19" refY="7" markerWidth="20" markerHeight="14" markerUnits="userSpaceOnUse" orient="auto"><path d="M 19,7 L9,13 L14,7 L9,1 Z"/></marker></defs>"#
    );
}

fn state_value_to_label_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => {
            let mut parts: Vec<&str> = Vec::new();
            for item in a {
                if let Some(s) = item.as_str() {
                    parts.push(s);
                }
            }
            if parts.is_empty() {
                return "".to_string();
            }
            parts.join("\n")
        }
        _ => "".to_string(),
    }
}

fn state_node_label_text(n: &StateSvgNode) -> String {
    n.label
        .as_ref()
        .map(state_value_to_label_text)
        .unwrap_or_else(|| n.id.clone())
}

fn html_paragraph_with_br(raw: &str) -> String {
    fn normalize_br_tags(raw: &str) -> String {
        let bytes = raw.as_bytes();
        let mut out = String::with_capacity(raw.len());
        let mut cur = 0usize;
        let mut i = 0usize;
        while i + 2 < bytes.len() {
            if bytes[i] != b'<' {
                i += 1;
                continue;
            }
            let b1 = bytes[i + 1];
            let b2 = bytes[i + 2];
            if !matches!(b1, b'b' | b'B') || !matches!(b2, b'r' | b'R') {
                i += 1;
                continue;
            }
            let next = bytes.get(i + 3).copied();
            if let Some(n) = next {
                if !matches!(n, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n') {
                    i += 1;
                    continue;
                }
            }
            if i > cur {
                out.push_str(&raw[cur..i]);
            }
            let Some(end_rel) = bytes[i..].iter().position(|&c| c == b'>') else {
                cur = i;
                break;
            };
            out.push('\n');
            i = i + end_rel + 1;
            cur = i;
        }
        if cur < raw.len() {
            out.push_str(&raw[cur..]);
        }
        out
    }

    let normalized = normalize_br_tags(raw);
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut out = String::new();
    out.push_str("<p>");
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push_str("<br />");
        }
        out.push_str(&escape_xml(line));
    }
    out.push_str("</p>");
    out
}

fn html_inline_with_br(raw: &str) -> String {
    fn normalize_br_tags(raw: &str) -> String {
        let bytes = raw.as_bytes();
        let mut out = String::with_capacity(raw.len());
        let mut cur = 0usize;
        let mut i = 0usize;
        while i + 2 < bytes.len() {
            if bytes[i] != b'<' {
                i += 1;
                continue;
            }
            let b1 = bytes[i + 1];
            let b2 = bytes[i + 2];
            if !matches!(b1, b'b' | b'B') || !matches!(b2, b'r' | b'R') {
                i += 1;
                continue;
            }
            let next = bytes.get(i + 3).copied();
            if let Some(n) = next {
                if !matches!(n, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n') {
                    i += 1;
                    continue;
                }
            }
            if i > cur {
                out.push_str(&raw[cur..i]);
            }
            let Some(end_rel) = bytes[i..].iter().position(|&c| c == b'>') else {
                cur = i;
                break;
            };
            out.push('\n');
            i = i + end_rel + 1;
            cur = i;
        }
        if cur < raw.len() {
            out.push_str(&raw[cur..]);
        }
        out
    }

    let normalized = normalize_br_tags(raw);
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push_str("<br />");
        }
        out.push_str(&escape_xml(line));
    }
    out
}

fn state_node_label_html(raw: &str) -> String {
    format!(
        r#"<span class="nodeLabel">{}</span>"#,
        html_paragraph_with_br(raw)
    )
}

fn state_node_label_inline_html(raw: &str) -> String {
    format!(
        r#"<span class="nodeLabel">{}</span>"#,
        html_inline_with_br(raw)
    )
}

fn state_edge_label_html(raw: &str) -> String {
    html_paragraph_with_br(raw)
}

fn state_is_hidden(ctx: &StateRenderCtx<'_>, id: &str) -> bool {
    ctx.hidden_prefixes
        .iter()
        .any(|p| id == p || id.starts_with(&format!("{p}----")))
}

fn state_strip_note_group<'a>(
    ctx: &'a StateRenderCtx<'_>,
    mut parent: Option<&'a str>,
) -> Option<&'a str> {
    while let Some(pid) = parent {
        let Some(pn) = ctx.nodes_by_id.get(pid).copied() else {
            return Some(pid);
        };
        if pn.shape == "noteGroup" {
            parent = ctx.parent.get(pid).copied();
            continue;
        }
        return Some(pid);
    }
    None
}

fn state_leaf_context_raw<'a>(ctx: &'a StateRenderCtx<'_>, id: &str) -> Option<&'a str> {
    let mut p = ctx.parent.get(id).copied();
    loop {
        let Some(pid) = state_strip_note_group(ctx, p) else {
            return None;
        };
        let Some(pn) = ctx.nodes_by_id.get(pid).copied() else {
            return Some(pid);
        };
        if pn.is_group && pn.shape != "noteGroup" {
            return Some(pid);
        }
        p = ctx.parent.get(pid).copied();
    }
}

fn state_insertion_context_raw<'a>(
    ctx: &'a StateRenderCtx<'_>,
    cluster_id: &str,
) -> Option<&'a str> {
    state_leaf_context_raw(ctx, cluster_id)
}

fn state_endpoint_context_raw<'a>(ctx: &'a StateRenderCtx<'_>, id: &str) -> Option<&'a str> {
    if let Some(n) = ctx.nodes_by_id.get(id).copied() {
        if n.is_group && n.shape != "noteGroup" {
            return state_insertion_context_raw(ctx, id);
        }
    }
    state_leaf_context_raw(ctx, id)
}

fn state_context_chain_raw<'a>(
    ctx: &'a StateRenderCtx<'_>,
    mut c: Option<&'a str>,
) -> Vec<Option<&'a str>> {
    let mut out = Vec::new();
    loop {
        out.push(c);
        let Some(id) = c else {
            break;
        };
        c = state_insertion_context_raw(ctx, id);
    }
    out
}

fn state_edge_context_raw<'a>(ctx: &'a StateRenderCtx<'_>, edge: &StateSvgEdge) -> Option<&'a str> {
    let a = state_endpoint_context_raw(ctx, edge.start.as_str());
    let b = state_endpoint_context_raw(ctx, edge.end.as_str());
    let ca = state_context_chain_raw(ctx, a);
    let cb = state_context_chain_raw(ctx, b);
    for anc in cb {
        if ca.contains(&anc) {
            return anc;
        }
    }
    None
}

fn state_leaf_context<'a>(ctx: &'a StateRenderCtx<'_>, id: &str) -> Option<&'a str> {
    let mut p = ctx.parent.get(id).copied();
    loop {
        let Some(pid) = state_strip_note_group(ctx, p) else {
            return None;
        };
        let Some(pn) = ctx.nodes_by_id.get(pid).copied() else {
            return Some(pid);
        };
        if pn.is_group && pn.shape != "noteGroup" {
            if ctx.nested_roots.contains(pid) {
                return Some(pid);
            }
            p = ctx.parent.get(pid).copied();
            continue;
        }
        p = ctx.parent.get(pid).copied();
    }
}

fn state_insertion_context<'a>(ctx: &'a StateRenderCtx<'_>, cluster_id: &str) -> Option<&'a str> {
    state_leaf_context(ctx, cluster_id)
}

fn state_endpoint_context<'a>(ctx: &'a StateRenderCtx<'_>, id: &str) -> Option<&'a str> {
    if let Some(n) = ctx.nodes_by_id.get(id).copied() {
        if n.is_group && n.shape != "noteGroup" {
            return state_insertion_context(ctx, id);
        }
    }
    state_leaf_context(ctx, id)
}

fn state_context_chain<'a>(
    ctx: &'a StateRenderCtx<'_>,
    mut c: Option<&'a str>,
) -> Vec<Option<&'a str>> {
    let mut out = Vec::new();
    loop {
        out.push(c);
        let Some(id) = c else {
            break;
        };
        c = state_insertion_context(ctx, id);
    }
    out
}

fn state_edge_context<'a>(ctx: &'a StateRenderCtx<'_>, edge: &StateSvgEdge) -> Option<&'a str> {
    let a = state_endpoint_context(ctx, edge.start.as_str());
    let b = state_endpoint_context(ctx, edge.end.as_str());
    let ca = state_context_chain(ctx, a);
    let cb = state_context_chain(ctx, b);
    for anc in cb {
        if ca.contains(&anc) {
            return anc;
        }
    }
    None
}

fn render_state_root(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    root: Option<&str>,
    parent_origin_x: f64,
    parent_origin_y: f64,
) {
    let (origin_x, origin_y, transform_attr) = if let Some(root_id) = root {
        if let Some(c) = ctx.layout_clusters_by_id.get(root_id).copied() {
            let left = c.x - c.width / 2.0;
            let top = c.y - c.height / 2.0;
            let tx = left - parent_origin_x;
            let ty = top - parent_origin_y;
            (
                left,
                top,
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
    out.push_str(r#"<g class="clusters">"#);
    if let Some(root_id) = root {
        render_state_cluster(out, ctx, root_id, origin_x, origin_y);
    }

    let mut cluster_ids: Vec<&str> = ctx.layout_clusters_by_id.keys().copied().collect();
    cluster_ids.sort_unstable();
    for &cluster_id in &cluster_ids {
        if root == Some(cluster_id) {
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

    for cluster_id in cluster_ids {
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
            escape_attr(cluster_id),
            fmt(x),
            fmt(y),
            fmt(cluster.width.max(1.0)),
            fmt(cluster.height.max(1.0))
        );
    }
    out.push_str("</g>");

    // edge paths
    out.push_str(r#"<g class="edgePaths">"#);
    if ctx.include_edges {
        for edge in ctx.edges {
            if state_is_hidden(ctx, edge.start.as_str())
                || state_is_hidden(ctx, edge.end.as_str())
                || state_is_hidden(ctx, edge.id.as_str())
            {
                continue;
            }
            if state_edge_context(ctx, edge) != root {
                continue;
            }
            render_state_edge_path(out, ctx, edge, origin_x, origin_y);
        }
    }
    out.push_str("</g>");

    // edge labels
    out.push_str(r#"<g class="edgeLabels">"#);
    if ctx.include_edges {
        for edge in ctx.edges {
            if state_is_hidden(ctx, edge.start.as_str())
                || state_is_hidden(ctx, edge.end.as_str())
                || state_is_hidden(ctx, edge.id.as_str())
            {
                continue;
            }
            if state_edge_context(ctx, edge) != root {
                continue;
            }
            render_state_edge_label(out, ctx, edge, origin_x, origin_y);
        }
    }
    out.push_str("</g>");

    // nodes (leaf nodes + nested roots)
    out.push_str(r#"<g class="nodes">"#);
    let mut nested: Vec<&str> = Vec::new();
    for (id, n) in ctx.nodes_by_id.iter() {
        if state_is_hidden(ctx, id) {
            continue;
        }
        if n.is_group && n.shape != "noteGroup" {
            if ctx.nested_roots.contains(*id) && state_insertion_context(ctx, id) == root {
                nested.push(*id);
            }
        }
    }

    if ctx.include_nodes {
        let mut leaf_ids: Vec<&str> = ctx
            .layout_nodes_by_id
            .iter()
            .filter_map(|(id, n)| {
                if state_is_hidden(ctx, id) {
                    return None;
                }
                if n.is_cluster {
                    return None;
                }
                if state_leaf_context(ctx, id) != root {
                    return None;
                }
                Some(*id)
            })
            .collect();
        leaf_ids.sort_unstable();
        for id in leaf_ids {
            render_state_node_svg(out, ctx, id, origin_x, origin_y);
        }
    }

    nested.sort_unstable();
    for child_root in nested {
        render_state_root(out, ctx, Some(child_root), origin_x, origin_y);
    }

    // Mermaid adds extra edgeLabel placeholders for self-loop transitions inside `nodes`.
    if ctx.include_edges {
        for edge in ctx.edges {
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

            let start = edge.start.as_str();
            let id1 = format!("{start}---{start}---1");
            let id2 = format!("{start}---{start}---2");

            let (cx, cy) = ctx
                .layout_edges_by_id
                .get(edge.id.as_str())
                .and_then(|e| e.label.as_ref())
                .map(|lbl| (lbl.x - origin_x, lbl.y - origin_y))
                .unwrap_or((0.0, 0.0));

            for id in [id1, id2] {
                let _ = write!(
                    out,
                    r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#,
                    escape_attr(&id),
                    fmt(cx),
                    fmt(cy),
                );
            }
        }
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
    let x = left - origin_x + 8.0;
    let y = top - origin_y + 8.0;

    if shape == "divider" {
        let _ = write!(
            out,
            r#"<g class="{}" id="{}" data-look="classic"><g><rect class="divider" x="{}" y="{}" width="{}" height="{}" data-look="classic"/></g></g>"#,
            escape_attr(class),
            escape_attr(cluster_id),
            fmt(x),
            fmt(y),
            fmt(cluster.width.max(1.0)),
            fmt(cluster.height.max(1.0))
        );
        return;
    }

    let title = ctx
        .nodes_by_id
        .get(cluster_id)
        .copied()
        .map(state_node_label_text)
        .unwrap_or_else(|| cluster_id.to_string());

    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-id="{}" data-look="classic"><g><rect class="outer" x="{}" y="{}" width="{}" height="{}" data-look="classic"/></g><g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="19"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: 1px; white-space: nowrap;"><span class="nodeLabel">{}</span></div></foreignObject></g><rect class="inner" x="{}" y="{}" width="{}" height="{}"/></g>"#,
        escape_attr(class),
        escape_attr(cluster_id),
        escape_attr(cluster_id),
        fmt(x),
        fmt(y),
        fmt(cluster.width.max(1.0)),
        fmt(cluster.height.max(1.0)),
        fmt(x + (cluster.width.max(1.0) - cluster.title_label.width.max(0.0)) / 2.0),
        fmt(y + 1.0),
        fmt(cluster.title_label.width.max(0.0)),
        escape_xml(&title),
        fmt(x),
        fmt(y + 21.0),
        fmt(cluster.width.max(1.0)),
        fmt((cluster.height - 29.0).max(1.0))
    );
}

fn render_state_edge_path(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    origin_x: f64,
    origin_y: f64,
) {
    let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() else {
        return;
    };
    if le.points.len() < 2 {
        return;
    }

    fn encode_path(
        points: &[crate::model::LayoutPoint],
        origin_x: f64,
        origin_y: f64,
    ) -> (String, String) {
        let mut local_points: Vec<crate::model::LayoutPoint> = Vec::new();
        for p in points {
            local_points.push(crate::model::LayoutPoint {
                x: p.x - origin_x,
                y: p.y - origin_y,
            });
        }
        let data_points = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&local_points).unwrap_or_default());
        let d = curve_basis_path_d(&local_points);
        (d, data_points)
    }

    let mut local_points: Vec<crate::model::LayoutPoint> = Vec::new();
    for p in &le.points {
        local_points.push(crate::model::LayoutPoint {
            x: p.x - origin_x,
            y: p.y - origin_y,
        });
    }
    let data_points = base64::engine::general_purpose::STANDARD
        .encode(serde_json::to_vec(&local_points).unwrap_or_default());
    let d = curve_basis_path_d(&local_points);

    let mut classes = "edge-thickness-normal edge-pattern-solid".to_string();
    for c in edge.classes.split_whitespace() {
        if c.trim().is_empty() {
            continue;
        }
        classes.push(' ');
        classes.push_str(c.trim());
    }

    let marker_end = if edge.arrow_type_end.trim() == "arrow_barb" {
        Some(format!("url(#{}_stateDiagram-barbEnd)", ctx.diagram_id))
    } else {
        None
    };

    if edge.start == edge.end {
        let start = edge.start.as_str();
        let id1 = format!("{start}-cyclic-special-1");
        let idm = format!("{start}-cyclic-special-mid");
        let id2 = format!("{start}-cyclic-special-2");

        let pts = &le.points;
        let seg1 = if pts.len() >= 3 {
            &pts[0..3]
        } else {
            &pts[0..2]
        };
        let segm = if pts.len() >= 5 {
            &pts[2..5]
        } else {
            &pts[0..2]
        };
        let seg2 = if pts.len() >= 3 {
            &pts[pts.len().saturating_sub(3)..]
        } else {
            &pts[pts.len().saturating_sub(2)..]
        };

        let segments = [
            (&id1, seg1, None),
            (&idm, segm, None),
            (&id2, seg2, marker_end.as_ref()),
        ];
        for (sid, pts, marker) in segments {
            if pts.len() < 2 {
                continue;
            }
            let (d, data_points) = encode_path(pts, origin_x, origin_y);
            let _ = write!(
                out,
                r#"<path d="{}" id="{}" class="{}" style="fill:none;;;fill:none" data-edge="true" data-et="edge" data-id="{}" data-points="{}""#,
                escape_attr(&d),
                escape_attr(sid),
                escape_attr(&classes),
                escape_attr(sid),
                escape_attr(&data_points)
            );
            if let Some(m) = marker {
                let _ = write!(out, r#" marker-end="{}""#, escape_attr(m));
            }
            out.push_str("/>");
        }
        return;
    }

    let _ = write!(
        out,
        r#"<path d="{}" id="{}" class="{}" style="fill:none;;;fill:none" data-edge="true" data-et="edge" data-id="{}" data-points="{}""#,
        escape_attr(&d),
        escape_attr(&edge.id),
        escape_attr(&classes),
        escape_attr(&edge.id),
        escape_attr(&data_points)
    );
    if let Some(m) = marker_end {
        let _ = write!(out, r#" marker-end="{}""#, escape_attr(&m));
    }
    out.push_str("/>");
}

fn render_state_edge_label(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    origin_x: f64,
    origin_y: f64,
) {
    let label_text = edge.label.trim();
    if edge.start == edge.end {
        let start = edge.start.as_str();
        let id1 = format!("{start}-cyclic-special-1");
        let idm = format!("{start}-cyclic-special-mid");
        let id2 = format!("{start}-cyclic-special-2");

        // Mermaid ties the visible self-loop label to the `*-mid` segment.
        if !label_text.is_empty() {
            if let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() {
                if let Some(lbl) = le.label.as_ref() {
                    let cx = lbl.x - origin_x;
                    let cy = lbl.y - origin_y;
                    let w = lbl.width.max(0.0);
                    let h = lbl.height.max(0.0);
                    let _ = write!(
                        out,
                        r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
                        fmt(cx),
                        fmt(cy),
                        escape_attr(&idm),
                        fmt(-w / 2.0),
                        fmt(-h / 2.0),
                        fmt(w),
                        fmt(h),
                        state_edge_label_html(label_text)
                    );
                }
            }
        } else {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr(&idm)
            );
        }

        for sid in [id1, id2] {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr(&sid)
            );
        }
        return;
    }

    if label_text.is_empty() {
        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
            escape_attr(&edge.id)
        );
        return;
    }

    let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() else {
        return;
    };
    let Some(lbl) = le.label.as_ref() else {
        return;
    };

    let cx = lbl.x - origin_x;
    let cy = lbl.y - origin_y;
    let w = lbl.width.max(0.0);
    let h = lbl.height.max(0.0);

    let _ = write!(
        out,
        r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
        fmt(cx),
        fmt(cy),
        escape_attr(&edge.id),
        fmt(-w / 2.0),
        fmt(-h / 2.0),
        fmt(w),
        fmt(h),
        state_edge_label_html(label_text)
    );
}

fn render_state_node_svg(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
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

    let node_class = if node.css_classes.trim().is_empty() {
        "node".to_string()
    } else {
        format!("node {}", node.css_classes.trim())
    };

    match node.shape.as_str() {
        "stateStart" => {
            let _ = write!(
                out,
                r#"<g class="node default" id="{}" transform="translate({}, {})"><circle class="state-start" r="7" width="14" height="14"/></g>"#,
                escape_attr(&node.dom_id),
                fmt(cx),
                fmt(cy)
            );
        }
        "stateEnd" => {
            let _ = write!(
                out,
                r##"<g class="node default" id="{}" transform="translate({}, {})"><g><path d="M0,0" stroke="none" stroke-width="0" fill="#ECECFF" style=""/><path d="M0,0" stroke="#333333" stroke-width="2" fill="none" stroke-dasharray="0 0" style=""/><g><path d="M0,0" stroke="none" stroke-width="0" fill="#9370DB" style=""/><path d="M0,0" stroke="#9370DB" stroke-width="2" fill="none" stroke-dasharray="0 0" style=""/></g></g></g>"##,
                escape_attr(&node.dom_id),
                fmt(cx),
                fmt(cy)
            );
        }
        "fork" | "join" => {
            let _ = write!(
                out,
                r##"<g class="{}" id="{}" transform="translate({}, {})"><g><path d="M0,0" stroke="none" stroke-width="0" fill="#333333" style=""/><path d="M0,0" stroke="#333333" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                escape_attr(&node_class),
                escape_attr(&node.dom_id),
                fmt(cx),
                fmt(cy)
            );
        }
        "note" => {
            let label = state_node_label_text(node);
            let metrics = ctx.measurer.measure_wrapped(
                &label,
                &ctx.text_style,
                Some(200.0),
                WrapMode::HtmlLike,
            );
            let lw = metrics.width.max(0.0);
            let lh = metrics.height.max(0.0);
            let _ = write!(
                out,
                r##"<g class="{}" id="{}" transform="translate({}, {})"><g class="basic label-container"><path d="M0,0" stroke="none" stroke-width="0" fill="#fff5ad"/><path d="M0,0" stroke="#aaaa33" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/></g><g class="label" style="" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;">{}</div></foreignObject></g></g>"##,
                escape_attr(&node_class),
                escape_attr(&node.dom_id),
                fmt(cx),
                fmt(cy),
                fmt(-lw / 2.0),
                fmt(-lh / 2.0),
                fmt(lw),
                fmt(lh),
                state_node_label_html(&label)
            );
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
            let title_metrics =
                ctx.measurer
                    .measure_wrapped(&title, &ctx.text_style, None, WrapMode::HtmlLike);
            let desc_metrics =
                ctx.measurer
                    .measure_wrapped(&desc, &ctx.text_style, None, WrapMode::HtmlLike);
            let _ = write!(
                out,
                r#"<g class="{}" id="{}" transform="translate({}, {})"><g><rect class="outer title-state" style="" x="{}" y="{}" width="{}" height="{}"/><line class="divider" x1="{}" x2="{}" y1="{}" y2="{}"/></g><g class="label" style="" transform="translate({}, {})"><foreignObject width="{}" height="{}" transform="translate( {}, 0)"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: 1px; white-space: nowrap;">{}</div></foreignObject><foreignObject width="{}" height="{}" transform="translate( 0, {})"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: 1px; white-space: nowrap;">{}</div></foreignObject></g></g>"#,
                escape_attr(&node_class),
                escape_attr(&node.dom_id),
                fmt(cx),
                fmt(cy),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w),
                fmt(h),
                fmt(-w / 2.0),
                fmt(w / 2.0),
                fmt(0.0),
                fmt(0.0),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(title_metrics.width.max(0.0)),
                fmt(title_metrics.height.max(0.0)),
                fmt((w - title_metrics.width.max(0.0)) / 2.0),
                state_node_label_inline_html(&title),
                fmt(desc_metrics.width.max(0.0)),
                fmt(desc_metrics.height.max(0.0)),
                fmt(title_metrics.height.max(0.0) + 9.0),
                state_node_label_inline_html(&desc)
            );
        }
        _ => {
            let label = state_node_label_text(node);
            let metrics = ctx.measurer.measure_wrapped(
                &label,
                &ctx.text_style,
                Some(200.0),
                WrapMode::HtmlLike,
            );
            let lw = metrics.width.max(0.0);
            let lh = metrics.height.max(0.0);

            let link = ctx.links.get(node_id);
            let link_open = if let Some(link) = link {
                let url = link.url.trim();
                if url.is_empty() {
                    String::new()
                } else {
                    let title_attr = if !link.tooltip.trim().is_empty() {
                        format!(r#" title="{}""#, escape_attr(link.tooltip.trim()))
                    } else {
                        String::new()
                    };
                    format!(r#"<a xlink:href="{}"{}>"#, escape_attr(url), title_attr)
                }
            } else {
                String::new()
            };
            let link_close = if link_open.is_empty() { "" } else { "</a>" };

            out.push_str(&format!(
                r##"<g class="{}" id="{}" transform="translate({}, {})"><g class="basic label-container outer-path"><path d="M0,0" stroke="none" stroke-width="0" fill="#ECECFF" style=""/><path d="M0,0" stroke="#9370DB" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>{}<g class="label" style="" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;">{}</div></foreignObject></g>{}</g>"##,
                escape_attr(&node_class),
                escape_attr(&node.dom_id),
                fmt(cx),
                fmt(cy),
                link_open,
                fmt(-lw / 2.0),
                fmt(-lh / 2.0),
                fmt(lw),
                fmt(lh),
                state_node_label_html(&label),
                link_close
            ));
        }
    }
}

pub fn render_state_diagram_v2_debug_svg(
    layout: &StateDiagramV2Layout,
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
.node-circle { fill: none; stroke: #2563eb; stroke-width: 1; }
.node-label { fill: #1f2937; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 11px; text-anchor: middle; dominant-baseline: middle; }
.edge { fill: none; stroke: #111827; stroke-width: 1; }
.edge-label-box { fill: #fef3c7; stroke: #92400e; stroke-width: 1; opacity: 0.6; }
.edge-label { fill: #111827; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 11px; text-anchor: middle; dominant-baseline: middle; }
.debug-cross { stroke: #ef4444; stroke-width: 1; }
</style>
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
                out.push_str(r#"<polyline class="edge" points=""#);
                for (idx, p) in e.points.iter().enumerate() {
                    if idx > 0 {
                        out.push(' ');
                    }
                    let _ = write!(&mut out, "{},{}", fmt(p.x), fmt(p.y));
                }
                let _ = write!(
                    &mut out,
                    r#"" data-from-cluster="{}" data-to-cluster="{}" />"#,
                    escape_attr(e.from_cluster.as_deref().unwrap_or_default()),
                    escape_attr(e.to_cluster.as_deref().unwrap_or_default())
                );
            }

            if let Some(lbl) = &e.label {
                let x = lbl.x - lbl.width / 2.0;
                let y = lbl.y - lbl.height / 2.0;
                let _ = write!(
                    &mut out,
                    r#"<rect class="edge-label-box" x="{}" y="{}" width="{}" height="{}" />"#,
                    fmt(x),
                    fmt(y),
                    fmt(lbl.width.max(1.0)),
                    fmt(lbl.height.max(1.0))
                );
            }

            if options.include_edge_id_labels {
                if let Some(lbl) = &e.label {
                    let _ = write!(
                        &mut out,
                        r#"<text class="edge-label" x="{}" y="{}">{}</text>"#,
                        fmt(lbl.x),
                        fmt(lbl.y),
                        escape_xml(&e.id)
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
            render_state_node(&mut out, n);
        }
        out.push_str("</g>\n");
    }

    out.push_str("</svg>\n");
    out
}

pub fn render_class_diagram_v2_debug_svg(
    layout: &ClassDiagramV2Layout,
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
.edge-label-box { fill: #fef3c7; stroke: #92400e; stroke-width: 1; opacity: 0.6; }
.terminal-label-box { fill: #e0f2fe; stroke: #0369a1; stroke-width: 1; opacity: 0.6; }
.terminal-label { fill: #0f172a; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 10px; text-anchor: middle; dominant-baseline: middle; }
.debug-cross { stroke: #ef4444; stroke-width: 1; }
</style>
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
                out.push_str(r#"<polyline class="edge" points=""#);
                for (idx, p) in e.points.iter().enumerate() {
                    if idx > 0 {
                        out.push(' ');
                    }
                    let _ = write!(&mut out, "{},{}", fmt(p.x), fmt(p.y));
                }
                out.push_str(r#"" />"#);
            }

            if let Some(lbl) = &e.label {
                let x = lbl.x - lbl.width / 2.0;
                let y = lbl.y - lbl.height / 2.0;
                let _ = write!(
                    &mut out,
                    r#"<rect class="edge-label-box" x="{}" y="{}" width="{}" height="{}" />"#,
                    fmt(x),
                    fmt(y),
                    fmt(lbl.width.max(1.0)),
                    fmt(lbl.height.max(1.0))
                );
            }

            for (slot, name) in [
                (e.start_label_left.as_ref(), "SL"),
                (e.start_label_right.as_ref(), "SR"),
                (e.end_label_left.as_ref(), "EL"),
                (e.end_label_right.as_ref(), "ER"),
            ] {
                let Some(lbl) = slot else {
                    continue;
                };
                let x = lbl.x - lbl.width / 2.0;
                let y = lbl.y - lbl.height / 2.0;
                let _ = write!(
                    &mut out,
                    r#"<rect class="terminal-label-box" x="{}" y="{}" width="{}" height="{}" />"#,
                    fmt(x),
                    fmt(y),
                    fmt(lbl.width.max(1.0)),
                    fmt(lbl.height.max(1.0))
                );
                let _ = write!(
                    &mut out,
                    r#"<text class="terminal-label" x="{}" y="{}">{}</text>"#,
                    fmt(lbl.x),
                    fmt(lbl.y),
                    escape_xml(name)
                );
            }

            if options.include_edge_id_labels {
                if let Some(lbl) = &e.label {
                    let _ = write!(
                        &mut out,
                        r#"<text class="node-label" x="{}" y="{}">{}</text>"#,
                        fmt(lbl.x),
                        fmt(lbl.y),
                        escape_xml(&e.id)
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
struct ClassSvgModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    direction: String,
    classes: std::collections::BTreeMap<String, ClassSvgNode>,
    #[serde(default)]
    relations: Vec<ClassSvgRelation>,
    #[serde(default)]
    notes: Vec<ClassSvgNote>,
    #[serde(default)]
    namespaces: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassSvgNode {
    id: String,
    #[serde(rename = "domId")]
    dom_id: String,
    #[serde(rename = "cssClasses")]
    css_classes: String,
    label: String,
    text: String,
    #[serde(default)]
    annotations: Vec<String>,
    #[serde(default)]
    members: Vec<ClassSvgMember>,
    #[serde(default)]
    methods: Vec<ClassSvgMember>,
    #[serde(default)]
    styles: Vec<String>,
    #[serde(default)]
    link: Option<String>,
    #[serde(rename = "linkTarget")]
    #[serde(default)]
    link_target: Option<String>,
    #[serde(default)]
    tooltip: Option<String>,
    #[serde(rename = "haveCallback")]
    #[serde(default)]
    have_callback: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassSvgMember {
    #[serde(rename = "displayText")]
    display_text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassSvgRelation {
    id: String,
    id1: String,
    id2: String,
    #[serde(rename = "relationTitle1")]
    relation_title_1: String,
    #[serde(rename = "relationTitle2")]
    relation_title_2: String,
    title: String,
    relation: ClassSvgRelationShape,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassSvgRelationShape {
    type1: i32,
    type2: i32,
    #[serde(rename = "lineType")]
    line_type: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassSvgNote {
    id: String,
    text: String,
    #[serde(rename = "class")]
    class_id: Option<String>,
}

fn class_marker_name(ty: i32, is_start: bool) -> Option<&'static str> {
    // Mermaid class diagram relationType constants.
    // -1 = none, 0 = aggregation, 1 = extension, 2 = composition, 3 = dependency, 4 = lollipop
    match ty {
        0 => Some(if is_start {
            "aggregationStart"
        } else {
            "aggregationEnd"
        }),
        1 => Some(if is_start {
            "extensionStart"
        } else {
            "extensionEnd"
        }),
        2 => Some(if is_start {
            "compositionStart"
        } else {
            "compositionEnd"
        }),
        3 => Some(if is_start {
            "dependencyStart"
        } else {
            "dependencyEnd"
        }),
        4 => Some(if is_start {
            "lollipopStart"
        } else {
            "lollipopEnd"
        }),
        _ => None,
    }
}

fn class_markers(out: &mut String, diagram_id: &str, diagram_marker_class: &str) {
    // Match Mermaid unified output: multiple <defs> wrappers, one marker each.
    fn marker_path(
        out: &mut String,
        diagram_id: &str,
        diagram_marker_class: &str,
        name: &str,
        class: &str,
        ref_x: &str,
        ref_y: &str,
        marker_w: &str,
        marker_h: &str,
        d: &str,
    ) {
        let _ = write!(
            out,
            r#"<defs><marker id="{}_{}-{}" class="{}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto"><path d="{}"/></marker></defs>"#,
            escape_xml(diagram_id),
            escape_xml(diagram_marker_class),
            escape_xml(name),
            escape_xml(class),
            ref_x,
            ref_y,
            marker_w,
            marker_h,
            escape_xml(d)
        );
    }

    fn marker_circle(
        out: &mut String,
        diagram_id: &str,
        diagram_marker_class: &str,
        name: &str,
        class: &str,
        ref_x: &str,
        ref_y: &str,
        marker_w: &str,
        marker_h: &str,
    ) {
        let _ = write!(
            out,
            r#"<defs><marker id="{}_{}-{}" class="{}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto"><circle stroke="black" fill="transparent" cx="7" cy="7" r="6"/></marker></defs>"#,
            escape_xml(diagram_id),
            escape_xml(diagram_marker_class),
            escape_xml(name),
            escape_xml(class),
            ref_x,
            ref_y,
            marker_w,
            marker_h
        );
    }

    let aggregation = format!("marker aggregation {diagram_marker_class}");
    let extension = format!("marker extension {diagram_marker_class}");
    let composition = format!("marker composition {diagram_marker_class}");
    let dependency = format!("marker dependency {diagram_marker_class}");
    let lollipop = format!("marker lollipop {diagram_marker_class}");

    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "aggregationStart",
        &aggregation,
        "18",
        "7",
        "190",
        "240",
        "M 18,7 L9,13 L1,7 L9,1 Z",
    );
    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "aggregationEnd",
        &aggregation,
        "1",
        "7",
        "20",
        "28",
        "M 18,7 L9,13 L1,7 L9,1 Z",
    );

    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "extensionStart",
        &extension,
        "18",
        "7",
        "190",
        "240",
        "M 1,7 L18,13 V 1 Z",
    );
    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "extensionEnd",
        &extension,
        "1",
        "7",
        "20",
        "28",
        "M 1,1 V 13 L18,7 Z",
    );

    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "compositionStart",
        &composition,
        "18",
        "7",
        "190",
        "240",
        "M 18,7 L9,13 L1,7 L9,1 Z",
    );
    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "compositionEnd",
        &composition,
        "1",
        "7",
        "20",
        "28",
        "M 18,7 L9,13 L1,7 L9,1 Z",
    );

    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "dependencyStart",
        &dependency,
        "6",
        "7",
        "190",
        "240",
        "M 5,7 L9,13 L1,7 L9,1 Z",
    );
    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "dependencyEnd",
        &dependency,
        "13",
        "7",
        "20",
        "28",
        "M 18,7 L9,13 L14,7 L9,1 Z",
    );

    marker_circle(
        out,
        diagram_id,
        diagram_marker_class,
        "lollipopStart",
        &lollipop,
        "13",
        "7",
        "190",
        "240",
    );
    marker_circle(
        out,
        diagram_id,
        diagram_marker_class,
        "lollipopEnd",
        &lollipop,
        "1",
        "7",
        "190",
        "240",
    );
}

fn class_edge_dom_id(
    edge: &crate::model::LayoutEdge,
    relation_index_by_id: &std::collections::HashMap<&str, usize>,
) -> String {
    if edge.id.starts_with("edgeNote") {
        return edge.id.clone();
    }
    // Mermaid uses `getEdgeId` with prefix `id`.
    let idx = relation_index_by_id
        .get(edge.id.as_str())
        .copied()
        .unwrap_or(1);
    format!("id_{}_{}_{}", edge.from, edge.to, idx)
}

fn class_edge_pattern(line_type: i32) -> &'static str {
    // Mermaid class diagram `lineType` uses "dottedLine" for `..` which maps to the dashed pattern.
    if line_type == 1 {
        "edge-pattern-dashed"
    } else {
        "edge-pattern-solid"
    }
}

fn class_note_edge_pattern() -> &'static str {
    "edge-pattern-dotted"
}

fn render_class_html_label(
    out: &mut String,
    span_class: &str,
    text: &str,
    include_p: bool,
    extra_span_class: Option<&str>,
) {
    let mut class = span_class.to_string();
    if let Some(extra) = extra_span_class {
        if !extra.trim().is_empty() {
            class.push(' ');
            class.push_str(extra.trim());
        }
    }
    if include_p {
        let _ = write!(
            out,
            r#"<span class="{}"><p>{}</p></span>"#,
            escape_xml(&class),
            escape_xml(text)
        );
    } else {
        let _ = write!(
            out,
            r#"<span class="{}">{}</span>"#,
            escape_xml(&class),
            escape_xml(text)
        );
    }
}

fn class_apply_inline_styles(node: &ClassSvgNode) -> (Option<&str>, Option<&str>, Option<&str>) {
    let mut fill: Option<&str> = None;
    let mut stroke: Option<&str> = None;
    let mut stroke_width: Option<&str> = None;
    for raw in &node.styles {
        let Some((k, v)) = raw.split_once(':') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();
        if key.eq_ignore_ascii_case("fill") && !val.is_empty() {
            fill = Some(val);
        }
        if key.eq_ignore_ascii_case("stroke") && !val.is_empty() {
            stroke = Some(val);
        }
        if key.eq_ignore_ascii_case("stroke-width") && !val.is_empty() {
            stroke_width = Some(val);
        }
    }
    (fill, stroke, stroke_width)
}

fn class_decode_entities_minimal(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn splitmix64_next(state: &mut u64) -> u64 {
    // Deterministic PRNG for "rough-like" stroke paths.
    // (We do not use OS randomness to keep SVG output stable.)
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn splitmix64_f64(state: &mut u64) -> f64 {
    let v = splitmix64_next(state);
    // Convert to [0,1).
    (v as f64) / ((u64::MAX as f64) + 1.0)
}

fn class_rough_seed(diagram_id: &str, dom_id: &str) -> u64 {
    // FNV-1a 64-bit.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in diagram_id.as_bytes().iter().chain(dom_id.as_bytes().iter()) {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn class_rough_line_double_path(x1: f64, y1: f64, x2: f64, y2: f64, mut seed: u64) -> String {
    let dx = x2 - x1;
    let dy = y2 - y1;

    fn make_pair(seed: &mut u64, a0: f64, a1: f64, b0: f64, b1: f64) -> (f64, f64) {
        let mut a = a0 + (a1 - a0) * splitmix64_f64(seed);
        let mut b = b0 + (b1 - b0) * splitmix64_f64(seed);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        (a, b)
    }

    let (t1, t2) = make_pair(&mut seed, 0.20, 0.50, 0.55, 0.90);
    let (t3, t4) = make_pair(&mut seed, 0.15, 0.55, 0.40, 0.95);

    let c1x = x1 + dx * t1;
    let c1y = y1 + dy * t1;
    let c2x = x1 + dx * t2;
    let c2y = y1 + dy * t2;

    let c3x = x1 + dx * t3;
    let c3y = y1 + dy * t3;
    let c4x = x1 + dx * t4;
    let c4y = y1 + dy * t4;

    format!(
        "M{} {} C{} {}, {} {}, {} {} M{} {} C{} {}, {} {}, {} {}",
        fmt(x1),
        fmt(y1),
        fmt(c1x),
        fmt(c1y),
        fmt(c2x),
        fmt(c2y),
        fmt(x2),
        fmt(y2),
        fmt(x1),
        fmt(y1),
        fmt(c3x),
        fmt(c3y),
        fmt(c4x),
        fmt(c4y),
        fmt(x2),
        fmt(y2),
    )
}

fn class_rough_rect_stroke_path(left: f64, top: f64, width: f64, height: f64, seed: u64) -> String {
    let right = left + width;
    let bottom = top + height;

    let mut out = String::new();
    out.push_str(&class_rough_line_double_path(
        left,
        top,
        right,
        top,
        seed ^ 0x01,
    ));
    out.push_str(&class_rough_line_double_path(
        right,
        top,
        right,
        bottom,
        seed ^ 0x02,
    ));
    out.push_str(&class_rough_line_double_path(
        right,
        bottom,
        left,
        bottom,
        seed ^ 0x03,
    ));
    out.push_str(&class_rough_line_double_path(
        left,
        bottom,
        left,
        top,
        seed ^ 0x04,
    ));
    out
}

pub fn render_class_diagram_v2_svg(
    layout: &ClassDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: ClassSvgModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let aria_roledescription = options.aria_roledescription.as_deref().unwrap_or("class");

    let font_size = effective_config
        .get("fontSize")
        .and_then(|v| v.as_f64())
        .unwrap_or(16.0)
        .max(1.0);
    let line_height = font_size * 1.5;
    let _class_padding = effective_config
        .get("class")
        .and_then(|v| v.get("padding"))
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0)
        .max(0.0);
    let text_style = TextStyle {
        font_family: None,
        font_size,
        font_weight: None,
    };

    let has_acc_title = model
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = model
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="classDiagram" role="graphics-document document" aria-roledescription="{}""#,
        escape_xml(diagram_id),
        escape_attr(aria_roledescription)
    );
    if has_acc_title {
        let _ = write!(
            &mut out,
            r#" aria-labelledby="chart-title-{}""#,
            escape_xml(diagram_id)
        );
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#" aria-describedby="chart-desc-{}""#,
            escape_xml(diagram_id)
        );
    }
    out.push('>');

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}"#,
            escape_xml(diagram_id),
            escape_xml(model.acc_title.as_deref().unwrap_or_default())
        );
        out.push_str("</title>");
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}"#,
            escape_xml(diagram_id),
            escape_xml(model.acc_descr.as_deref().unwrap_or_default())
        );
        out.push_str("</desc>");
    }

    // Mermaid emits a single `<style>` element with diagram-scoped CSS.
    out.push_str("<style></style>");

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");
    class_markers(&mut out, diagram_id, aria_roledescription);

    let mut class_nodes_by_id: std::collections::HashMap<&str, &ClassSvgNode> =
        std::collections::HashMap::new();
    for (id, n) in &model.classes {
        class_nodes_by_id.insert(id.as_str(), n);
    }

    let mut relations_by_id: std::collections::HashMap<&str, &ClassSvgRelation> =
        std::collections::HashMap::new();
    for r in &model.relations {
        relations_by_id.insert(r.id.as_str(), r);
    }
    let mut relation_index_by_id: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for (idx, r) in model.relations.iter().enumerate() {
        relation_index_by_id.insert(r.id.as_str(), idx + 1);
    }

    let mut note_by_id: std::collections::HashMap<&str, &ClassSvgNote> =
        std::collections::HashMap::new();
    for n in &model.notes {
        note_by_id.insert(n.id.as_str(), n);
    }

    out.push_str(r#"<g class="root">"#);

    // Clusters (namespaces).
    out.push_str(r#"<g class="clusters">"#);
    let mut clusters = layout.clusters.clone();
    clusters.sort_by(|a, b| a.id.cmp(&b.id));
    for c in &clusters {
        let left = c.x - c.width / 2.0;
        let top = c.y - c.height / 2.0;
        let _ = write!(
            &mut out,
            r#"<g class="cluster undefined" id="{}" data-look="classic"><rect x="{}" y="{}" width="{}" height="{}"/><g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel"><p>{}</p></span></div></foreignObject></g></g>"#,
            escape_attr(&c.id),
            fmt(left),
            fmt(top),
            fmt(c.width.max(1.0)),
            fmt(c.height.max(1.0)),
            fmt(left + (c.width.max(1.0) - c.title_label.width.max(0.0)) / 2.0),
            fmt(top),
            fmt(c.title_label.width.max(0.0)),
            escape_xml(&c.title)
        );
    }
    out.push_str("</g>");

    // Edge paths.
    out.push_str(r#"<g class="edgePaths">"#);
    let mut edges = layout.edges.clone();
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    for e in &edges {
        if e.points.len() < 2 {
            continue;
        }

        let dom_id = class_edge_dom_id(e, &relation_index_by_id);
        let d = curve_basis_path_d(&e.points);
        let points_b64 = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&e.points).unwrap_or_default());

        let mut class = String::from("edge-thickness-normal ");
        if e.id.starts_with("edgeNote") {
            class.push_str(class_note_edge_pattern());
        } else if let Some(rel) = relations_by_id.get(e.id.as_str()) {
            class.push_str(class_edge_pattern(rel.relation.line_type));
        } else {
            class.push_str("edge-pattern-solid");
        }
        class.push_str(" relation");

        let mut marker_start: Option<String> = None;
        let mut marker_end: Option<String> = None;
        if !e.id.starts_with("edgeNote") {
            if let Some(rel) = relations_by_id.get(e.id.as_str()) {
                if let Some(name) = class_marker_name(rel.relation.type1, true) {
                    marker_start = Some(format!(
                        "url(#{}_{aria_roledescription}-{name})",
                        diagram_id
                    ));
                }
                if let Some(name) = class_marker_name(rel.relation.type2, false) {
                    marker_end = Some(format!(
                        "url(#{}_{aria_roledescription}-{name})",
                        diagram_id
                    ));
                }
            }
        }

        let _ = write!(
            &mut out,
            r#"<path d="{}" id="{}" class="{}" data-edge="true" data-et="edge" data-id="{}" data-points="{}""#,
            escape_attr(&d),
            escape_attr(&dom_id),
            escape_attr(&class),
            escape_attr(&dom_id),
            escape_attr(&points_b64),
        );
        if let Some(url) = marker_start {
            let _ = write!(&mut out, r#" marker-start="{}""#, escape_attr(&url));
        }
        if let Some(url) = marker_end {
            let _ = write!(&mut out, r#" marker-end="{}""#, escape_attr(&url));
        }
        out.push_str("/>");
    }
    out.push_str("</g>");

    // Edge labels + terminals.
    out.push_str(r#"<g class="edgeLabels">"#);
    for e in &edges {
        let dom_id = class_edge_dom_id(e, &relation_index_by_id);
        let label_text = if e.id.starts_with("edgeNote") {
            String::new()
        } else {
            relations_by_id
                .get(e.id.as_str())
                .map(|r| r.title.clone())
                .unwrap_or_default()
        };

        if label_text.trim().is_empty() {
            let _ = write!(
                &mut out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr(&dom_id)
            );
        } else if let Some(lbl) = e.label.as_ref() {
            let _ = write!(
                &mut out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;">"#,
                fmt(lbl.x),
                fmt(lbl.y),
                escape_attr(&dom_id),
                fmt(-lbl.width / 2.0),
                fmt(-lbl.height / 2.0),
                fmt(lbl.width.max(0.0)),
                fmt(lbl.height.max(0.0)),
            );
            render_class_html_label(&mut out, "edgeLabel", label_text.trim(), true, None);
            out.push_str("</div></foreignObject></g></g>");
        } else {
            let _ = write!(
                &mut out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr(&dom_id)
            );
        }

        let Some(rel) = relations_by_id.get(e.id.as_str()).copied() else {
            continue;
        };

        let start_text = if rel.relation_title_1 == "none" {
            ""
        } else {
            rel.relation_title_1.as_str()
        };
        let end_text = if rel.relation_title_2 == "none" {
            ""
        } else {
            rel.relation_title_2.as_str()
        };

        if let Some(lbl) = e.start_label_left.as_ref() {
            if !start_text.trim().is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<g class="edgeTerminals" transform="translate({}, {})"><g class="inner" transform="translate(0, 0)"><foreignObject style="width: 9px; height: 12px;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: 1px; white-space: nowrap;"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
                    fmt(lbl.x),
                    fmt(lbl.y),
                    escape_xml(start_text.trim())
                );
            }
        }
        if let Some(lbl) = e.start_label_right.as_ref() {
            if !start_text.trim().is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<g class="edgeTerminals" transform="translate({}, {})"><g class="inner" transform="translate(0, 0)"><foreignObject style="width: 9px; height: 12px;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: 1px; white-space: nowrap;"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
                    fmt(lbl.x),
                    fmt(lbl.y),
                    escape_xml(start_text.trim())
                );
            }
        }
        if let Some(lbl) = e.end_label_left.as_ref() {
            if !end_text.trim().is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<g class="edgeTerminals" transform="translate({}, {})"><g class="inner" transform="translate(0, 0)"/><foreignObject style="width: 9px; height: 12px;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: 1px; white-space: nowrap;"><span class="edgeLabel">{}</span></div></foreignObject></g>"#,
                    fmt(lbl.x),
                    fmt(lbl.y),
                    escape_xml(end_text.trim())
                );
            }
        }
        if let Some(lbl) = e.end_label_right.as_ref() {
            if !end_text.trim().is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<g class="edgeTerminals" transform="translate({}, {})"><g class="inner" transform="translate(0, 0)"/><foreignObject style="width: 9px; height: 12px;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; padding-right: 1px; white-space: nowrap;"><span class="edgeLabel">{}</span></div></foreignObject></g>"#,
                    fmt(lbl.x),
                    fmt(lbl.y),
                    escape_xml(end_text.trim())
                );
            }
        }
    }
    out.push_str("</g>");

    // Nodes.
    out.push_str(r#"<g class="nodes">"#);

    // Render all non-cluster nodes, using the semantic model to decide node type/labels.
    let mut nodes = layout.nodes.clone();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    for n in &nodes {
        if n.is_cluster {
            continue;
        }

        if let Some(note) = note_by_id.get(n.id.as_str()).copied() {
            let note_text = class_decode_entities_minimal(note.text.trim());
            let metrics =
                measurer.measure_wrapped(&note_text, &text_style, None, WrapMode::HtmlLike);
            let fo_w = metrics.width.max(1.0);
            let fo_h = metrics.height.max(line_height).max(1.0);
            let w = n.width.max(1.0);
            let h = n.height.max(1.0);
            let left = -w / 2.0;
            let top = -h / 2.0;
            let label_x = -fo_w / 2.0;
            let label_y = -fo_h / 2.0;
            let note_stroke_d = class_rough_rect_stroke_path(
                left,
                top,
                w,
                h,
                class_rough_seed(diagram_id, &note.id),
            );
            let _ = write!(
                &mut out,
                r##"<g class="node undefined" id="{}" transform="translate({}, {})"><g class="basic label-container"><path d="M{} {} L{} {} L{} {} L{} {}" stroke="none" stroke-width="0" fill="#fff5ad" style="fill:#fff5ad !important;stroke:#aaaa33 !important"/><path d="{}" stroke="#aaaa33" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style="fill:#fff5ad !important;stroke:#aaaa33 !important"/></g><g class="label" style="text-align:left !important;white-space:nowrap !important" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div style="text-align: center; white-space: nowrap; display: table-cell; line-height: 1.5; max-width: 200px;" xmlns="http://www.w3.org/1999/xhtml"><span style="text-align:left !important;white-space:nowrap !important" class="nodeLabel"><p>{}</p></span></div></foreignObject></g></g>"##,
                escape_attr(&note.id),
                fmt(n.x),
                fmt(n.y),
                fmt(left),
                fmt(top),
                fmt(left + w),
                fmt(top),
                fmt(left + w),
                fmt(top + h),
                fmt(left),
                fmt(top + h),
                escape_attr(&note_stroke_d),
                fmt(label_x),
                fmt(label_y),
                fmt(fo_w),
                fmt(fo_h),
                escape_xml(&note_text)
            );
            continue;
        }

        let Some(node) = class_nodes_by_id.get(n.id.as_str()).copied() else {
            continue;
        };

        let (style_fill, style_stroke, style_stroke_width) = class_apply_inline_styles(node);
        let node_fill = style_fill.unwrap_or("#ECECFF");
        let node_stroke = style_stroke.unwrap_or("#9370DB");
        let node_stroke_width = style_stroke_width
            .unwrap_or("1.3")
            .trim_end_matches("px")
            .trim();

        let node_classes = format!("node {}", node.css_classes.trim());
        let tooltip = node.tooltip.as_deref().unwrap_or("").trim();
        let has_tooltip = !tooltip.is_empty();

        let link = node
            .link
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let include_href = link.is_some_and(|s| !s.to_ascii_lowercase().starts_with("javascript:"));
        let have_callback = node.have_callback;

        if let Some(link) = link {
            let _ = write!(
                &mut out,
                r#"<a{}{} transform="translate({}, {})">"#,
                if include_href {
                    format!(r#" xlink:href="{}""#, escape_attr(link))
                } else {
                    String::new()
                },
                if have_callback {
                    r#" class="null clickable""#.to_string()
                } else {
                    String::new()
                },
                fmt(n.x),
                fmt(n.y)
            );
        }

        let _ = write!(
            &mut out,
            r#"<g class="{}" id="{}""#,
            escape_attr(&node_classes),
            escape_attr(&node.dom_id),
        );
        if has_tooltip {
            let _ = write!(&mut out, r#" title="{}""#, escape_attr(tooltip));
        }
        if link.is_none() {
            let _ = write!(
                &mut out,
                r#" transform="translate({}, {})""#,
                fmt(n.x),
                fmt(n.y)
            );
        }
        out.push('>');

        out.push_str(r#"<g class="basic label-container">"#);
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        let left = -w / 2.0;
        let top = -h / 2.0;
        let rough_seed = class_rough_seed(diagram_id, &node.dom_id);
        let _ = write!(
            &mut out,
            r#"<path d="M{} {} L{} {} L{} {} L{} {}" stroke="none" stroke-width="0" fill="{}" style=""/>"#,
            fmt(left),
            fmt(top),
            fmt(left + w),
            fmt(top),
            fmt(left + w),
            fmt(top + h),
            fmt(left),
            fmt(top + h),
            escape_attr(node_fill)
        );
        let stroke_d = class_rough_rect_stroke_path(left, top, w, h, rough_seed);
        let _ = write!(
            &mut out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style=""/>"#,
            escape_attr(&stroke_d),
            escape_attr(node_stroke),
            escape_attr(node_stroke_width),
        );
        out.push_str("</g>");

        let title_text = class_decode_entities_minimal(node.text.trim());
        let title_metrics =
            measurer.measure_wrapped(&title_text, &text_style, None, WrapMode::HtmlLike);
        let ann_rows = node.annotations.len();
        let members_rows = node.members.len();
        let methods_rows = node.methods.len();
        let half_lh = line_height / 2.0;

        let title_y = top + (ann_rows as f64 + 1.0) * line_height;
        let annotation_group_y = if ann_rows == 0 {
            title_y
        } else {
            top + line_height
        };
        let divider1_y = top + (ann_rows as f64 + 2.0) * line_height;
        let members_group_y = top + (ann_rows as f64 + 3.0) * line_height;
        let divider2_y = members_group_y + (members_rows as f64) * line_height;
        let bottom = h / 2.0;
        let methods_group_y = if methods_rows > 0 {
            bottom - (methods_rows as f64) * line_height
        } else {
            // Upstream still emits a `methods-group` even when empty; keep it deterministic.
            divider2_y + line_height
        };

        let title_x = -title_metrics.width.max(0.0) / 2.0;

        let mut ann_max_w: f64 = 0.0;
        for a in &node.annotations {
            let t = format!(
                "\u{00AB}{}\u{00BB}",
                class_decode_entities_minimal(a.trim())
            );
            let m = measurer.measure_wrapped(&t, &text_style, None, WrapMode::HtmlLike);
            ann_max_w = ann_max_w.max(m.width);
        }
        let ann_x = -ann_max_w.max(0.0) / 2.0;
        let members_x = left + half_lh;

        // Annotation group.
        if node.annotations.is_empty() {
            let _ = write!(
                &mut out,
                r#"<g class="annotation-group text" transform="translate(0, {})"/>"#,
                fmt(annotation_group_y)
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<g class="annotation-group text" transform="translate({}, {})">"#,
                fmt(ann_x),
                fmt(annotation_group_y)
            );
            for (idx, a) in node.annotations.iter().enumerate() {
                let t = format!(
                    "\u{00AB}{}\u{00BB}",
                    class_decode_entities_minimal(a.trim())
                );
                let y = (idx as f64) * line_height - half_lh;
                let _ = write!(
                    &mut out,
                    r#"<g class="label" style="" transform="translate(0,{})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;">"#,
                    fmt(y),
                    fmt(ann_max_w.max(1.0)),
                    fmt(line_height.max(1.0))
                );
                render_class_html_label(
                    &mut out,
                    "nodeLabel",
                    t.as_str(),
                    true,
                    Some("markdown-node-label"),
                );
                out.push_str("</div></foreignObject></g>");
            }
            out.push_str("</g>");
        }

        // Label group (class name).
        let _ = write!(
            &mut out,
            r#"<g class="label-group text" transform="translate({}, {})"><g class="label" style="font-weight: bolder" transform="translate(0,-12)"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;">"#,
            fmt(title_x),
            fmt(title_y),
            fmt(title_metrics.width.max(1.0)),
            fmt(title_metrics.height.max(line_height).max(1.0))
        );
        render_class_html_label(
            &mut out,
            "nodeLabel",
            title_text.as_str(),
            true,
            Some("markdown-node-label"),
        );
        out.push_str("</div></foreignObject></g></g>");

        // Members.
        if node.members.is_empty() {
            let _ = write!(
                &mut out,
                r#"<g class="members-group text" transform="translate({}, {})"/>"#,
                fmt(members_x),
                fmt(members_group_y)
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<g class="members-group text" transform="translate({}, {})">"#,
                fmt(members_x),
                fmt(members_group_y)
            );
            for (idx, m) in node.members.iter().enumerate() {
                let t = class_decode_entities_minimal(m.display_text.trim());
                let mm = measurer.measure_wrapped(&t, &text_style, None, WrapMode::HtmlLike);
                let y = (idx as f64) * line_height - half_lh;
                let _ = write!(
                    &mut out,
                    r#"<g class="label" style="" transform="translate(0,{})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;">"#,
                    fmt(y),
                    fmt(mm.width.max(1.0)),
                    fmt(mm.height.max(line_height).max(1.0))
                );
                render_class_html_label(
                    &mut out,
                    "nodeLabel",
                    t.as_str(),
                    true,
                    Some("markdown-node-label"),
                );
                out.push_str("</div></foreignObject></g>");
            }
            out.push_str("</g>");
        }

        // Methods.
        if node.methods.is_empty() {
            let _ = write!(
                &mut out,
                r#"<g class="methods-group text" transform="translate({}, {})"/>"#,
                fmt(members_x),
                fmt(methods_group_y)
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<g class="methods-group text" transform="translate({}, {})">"#,
                fmt(members_x),
                fmt(methods_group_y)
            );
            for (idx, m) in node.methods.iter().enumerate() {
                let t = class_decode_entities_minimal(m.display_text.trim());
                let mm = measurer.measure_wrapped(&t, &text_style, None, WrapMode::HtmlLike);
                let y = (idx as f64) * line_height - half_lh;
                let _ = write!(
                    &mut out,
                    r#"<g class="label" style="" transform="translate(0,{})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;">"#,
                    fmt(y),
                    fmt(mm.width.max(1.0)),
                    fmt(mm.height.max(line_height).max(1.0))
                );
                render_class_html_label(
                    &mut out,
                    "nodeLabel",
                    t.as_str(),
                    true,
                    Some("markdown-node-label"),
                );
                out.push_str("</div></foreignObject></g>");
            }
            out.push_str("</g>");
        }

        // Dividers (always present in Mermaid output).
        for y in [divider1_y, divider2_y] {
            out.push_str(r#"<g class="divider" style="">"#);
            let d = class_rough_line_double_path(left, y, left + w, y, rough_seed ^ 0x55);
            let _ = write!(
                &mut out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style=""/>"#,
                escape_attr(&d),
                escape_attr(node_stroke),
                escape_attr(node_stroke_width),
            );
            out.push_str("</g>");
        }

        out.push_str("</g>");
        if link.is_some() {
            out.push_str("</a>");
        }
    }

    out.push_str("</g>"); // nodes
    out.push_str("</g>"); // root
    out.push_str("</g>"); // wrapper
    out.push_str("</svg>");

    Ok(out)
}

pub fn render_er_diagram_debug_svg(layout: &ErDiagramLayout, options: &SvgRenderOptions) -> String {
    let mut nodes = layout.nodes.clone();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut edges = layout.edges.clone();
    edges.sort_by(|a, b| a.id.cmp(&b.id));

    let bounds = compute_layout_bounds(&[], &nodes, &edges).unwrap_or(Bounds {
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
 .node-box { fill: none; stroke: #2563eb; stroke-width: 1; }
 .node-label { fill: #1f2937; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 11px; text-anchor: middle; dominant-baseline: middle; }
 .edge { fill: none; stroke: #111827; stroke-width: 1; }
 .edge-label-box { fill: #fef3c7; stroke: #92400e; stroke-width: 1; opacity: 0.6; }
 .debug-cross { stroke: #ef4444; stroke-width: 1; }
 </style>
 "#,
    );

    // Ported from Mermaid `@11.12.2` `erMarkers.js` (debug-only for now).
    out.push_str(
        r##"<defs>
  <marker id="MD_PARENT_START" refX="0" refY="7" markerWidth="190" markerHeight="240" orient="auto">
    <path d="M 18,7 L9,13 L1,7 L9,1 Z" fill="#111827" />
  </marker>
  <marker id="MD_PARENT_END" refX="19" refY="7" markerWidth="20" markerHeight="28" orient="auto">
    <path d="M 18,7 L9,13 L1,7 L9,1 Z" fill="#111827" />
  </marker>

  <marker id="ONLY_ONE_START" refX="0" refY="9" markerWidth="18" markerHeight="18" orient="auto">
    <path stroke="#111827" fill="none" d="M9,0 L9,18 M15,0 L15,18" />
  </marker>
  <marker id="ONLY_ONE_END" refX="18" refY="9" markerWidth="18" markerHeight="18" orient="auto">
    <path stroke="#111827" fill="none" d="M3,0 L3,18 M9,0 L9,18" />
  </marker>

  <marker id="ZERO_OR_ONE_START" refX="0" refY="9" markerWidth="30" markerHeight="18" orient="auto">
    <circle stroke="#111827" fill="white" cx="21" cy="9" r="6" />
    <path stroke="#111827" fill="none" d="M9,0 L9,18" />
  </marker>
  <marker id="ZERO_OR_ONE_END" refX="30" refY="9" markerWidth="30" markerHeight="18" orient="auto">
    <circle stroke="#111827" fill="white" cx="9" cy="9" r="6" />
    <path stroke="#111827" fill="none" d="M21,0 L21,18" />
  </marker>

  <marker id="ONE_OR_MORE_START" refX="18" refY="18" markerWidth="45" markerHeight="36" orient="auto">
    <path stroke="#111827" fill="none" d="M0,18 Q 18,0 36,18 Q 18,36 0,18 M42,9 L42,27" />
  </marker>
  <marker id="ONE_OR_MORE_END" refX="27" refY="18" markerWidth="45" markerHeight="36" orient="auto">
    <path stroke="#111827" fill="none" d="M3,9 L3,27 M9,18 Q27,0 45,18 Q27,36 9,18" />
  </marker>

  <marker id="ZERO_OR_MORE_START" refX="18" refY="18" markerWidth="57" markerHeight="36" orient="auto">
    <circle stroke="#111827" fill="white" cx="48" cy="18" r="6" />
    <path stroke="#111827" fill="none" d="M0,18 Q18,0 36,18 Q18,36 0,18" />
  </marker>
  <marker id="ZERO_OR_MORE_END" refX="39" refY="18" markerWidth="57" markerHeight="36" orient="auto">
    <circle stroke="#111827" fill="white" cx="9" cy="18" r="6" />
    <path stroke="#111827" fill="none" d="M21,18 Q39,0 57,18 Q39,36 21,18" />
  </marker>
</defs>
"##,
    );

    if options.include_edges {
        out.push_str(r#"<g class="edges">"#);
        for e in &edges {
            if e.points.len() >= 2 {
                let _ = write!(&mut out, r#"<polyline class="edge""#);
                if let Some(dash) = &e.stroke_dasharray {
                    let _ = write!(&mut out, r#" stroke-dasharray="{}""#, escape_xml(dash));
                }
                if let Some(m) = &e.start_marker {
                    let _ = write!(&mut out, r#" marker-start="url(#{})""#, escape_xml(m));
                }
                if let Some(m) = &e.end_marker {
                    let _ = write!(&mut out, r#" marker-end="url(#{})""#, escape_xml(m));
                }
                out.push_str(r#" points=""#);
                for (idx, p) in e.points.iter().enumerate() {
                    if idx > 0 {
                        out.push(' ');
                    }
                    let _ = write!(&mut out, "{},{}", fmt(p.x), fmt(p.y));
                }
                out.push_str(r#"" />"#);
            }

            if let Some(lbl) = &e.label {
                let x = lbl.x - lbl.width / 2.0;
                let y = lbl.y - lbl.height / 2.0;
                let _ = write!(
                    &mut out,
                    r#"<rect class="edge-label-box" x="{}" y="{}" width="{}" height="{}" />"#,
                    fmt(x),
                    fmt(y),
                    fmt(lbl.width.max(1.0)),
                    fmt(lbl.height.max(1.0))
                );
                if options.include_edge_id_labels {
                    let _ = write!(
                        &mut out,
                        r#"<text class="node-label" x="{}" y="{}">{}</text>"#,
                        fmt(lbl.x),
                        fmt(lbl.y),
                        escape_xml(&e.id)
                    );
                }
            }
        }
        out.push_str("</g>\n");
    }

    if options.include_nodes {
        out.push_str(r#"<g class="nodes">"#);
        for n in &nodes {
            render_node(&mut out, n);
        }
        out.push_str("</g>\n");
    }

    out.push_str("</svg>\n");
    out
}

fn config_string(cfg: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn json_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn config_f64(cfg: &serde_json::Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

fn normalize_css_font_family(font_family: &str) -> String {
    font_family.trim().trim_end_matches(';').trim().to_string()
}

fn theme_color(effective_config: &serde_json::Value, key: &str, fallback: &str) -> String {
    config_string(effective_config, &["themeVariables", key])
        .unwrap_or_else(|| fallback.to_string())
}

fn parse_style_decl(s: &str) -> Option<(&str, &str)> {
    let s = s.trim().trim_end_matches(';').trim();
    if s.is_empty() {
        return None;
    }
    let (k, v) = s.split_once(':')?;
    let k = k.trim();
    let v = v.trim();
    if k.is_empty() || v.is_empty() {
        return None;
    }
    Some((k, v))
}

fn is_rect_style_key(key: &str) -> bool {
    matches!(
        key,
        "fill"
            | "stroke"
            | "stroke-width"
            | "stroke-dasharray"
            | "opacity"
            | "fill-opacity"
            | "stroke-opacity"
    )
}

fn is_text_style_key(key: &str) -> bool {
    matches!(
        key,
        "color" | "font-family" | "font-size" | "font-weight" | "opacity"
    )
}

fn compile_er_entity_styles(
    entity: &crate::er::ErEntity,
    classes: &std::collections::BTreeMap<String, crate::er::ErClassDef>,
) -> (Vec<String>, Vec<String>) {
    let mut compiled_box: Vec<String> = Vec::new();
    let mut compiled_text: Vec<String> = Vec::new();
    let mut seen_classes: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for class_name in entity.css_classes.split_whitespace() {
        if !seen_classes.insert(class_name) {
            continue;
        }
        let Some(def) = classes.get(class_name) else {
            continue;
        };
        for s in &def.styles {
            let t = s.trim();
            if t.is_empty() {
                continue;
            }
            compiled_box.push(t.to_string());
        }
        for s in &def.text_styles {
            let t = s.trim();
            if t.is_empty() {
                continue;
            }
            compiled_text.push(t.to_string());
        }
    }

    let mut rect_map: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    let mut text_map: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();

    // Box styles: classDef styles + `style` statements.
    for s in compiled_box.iter().chain(entity.css_styles.iter()) {
        let Some((k, v)) = parse_style_decl(s) else {
            continue;
        };
        if is_rect_style_key(k) {
            rect_map.insert(k.to_string(), v.to_string());
        }
        // Mermaid treats `color:` as the HTML label text color (even if it comes from the style list).
        if k == "color" {
            text_map.insert("color".to_string(), v.to_string());
        }
    }

    // Text styles: classDef textStyles + `style` statements (only text-related keys).
    for s in compiled_text.iter().chain(entity.css_styles.iter()) {
        let Some((k, v)) = parse_style_decl(s) else {
            continue;
        };
        if !is_text_style_key(k) {
            continue;
        }
        if k == "color" {
            text_map.insert("color".to_string(), v.to_string());
        } else {
            text_map.insert(k.to_string(), v.to_string());
        }
    }

    let mut rect_decls: Vec<String> = Vec::new();
    for k in [
        "fill",
        "stroke",
        "stroke-width",
        "stroke-dasharray",
        "opacity",
        "fill-opacity",
        "stroke-opacity",
    ] {
        if let Some(v) = rect_map.get(k) {
            rect_decls.push(format!("{k}:{v}"));
        }
    }

    let mut text_decls: Vec<String> = Vec::new();
    for k in [
        "color",
        "font-family",
        "font-size",
        "font-weight",
        "opacity",
    ] {
        if let Some(v) = text_map.get(k) {
            text_decls.push(format!("{k}:{v}"));
        }
    }

    (rect_decls, text_decls)
}

fn style_decls_with_important_join(decls: &[String], join: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for d in decls {
        let Some((k, v)) = parse_style_decl(d) else {
            continue;
        };
        out.push(format!("{k}:{v} !important"));
    }
    out.join(join)
}

fn style_decls_with_important(decls: &[String]) -> String {
    style_decls_with_important_join(decls, "; ")
}

fn last_style_value(decls: &[String], key: &str) -> Option<String> {
    for d in decls.iter().rev() {
        let Some((k, v)) = parse_style_decl(d) else {
            continue;
        };
        if k == key {
            return Some(v.to_string());
        }
    }
    None
}

fn concat_style_keys(decls: &[String], keys: &[&str]) -> String {
    let mut out = String::new();
    for k in keys {
        if let Some(v) = last_style_value(decls, k) {
            out.push_str(k);
            out.push(':');
            out.push_str(&v);
        }
    }
    out
}

fn parse_px_f64(v: &str) -> Option<f64> {
    let raw = v.trim().trim_end_matches(';').trim();
    let raw = raw.trim_end_matches("px").trim();
    if raw.is_empty() {
        return None;
    }
    raw.parse::<f64>().ok()
}

pub fn render_er_diagram_svg(
    layout: &ErDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: crate::er::ErModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    // Mermaid's internal diagram type for ER is `er` (not `erDiagram`), and marker ids are derived
    // from this type (e.g. `<diagramId>_er-zeroOrMoreEnd`).
    let diagram_type = "er";

    // Mermaid's computed theme variables are not currently present in `effective_config`.
    // Use Mermaid default theme fallbacks so Stage-B SVGs match upstream defaults more closely.
    let stroke = theme_color(effective_config, "lineColor", "#333333");
    let node_border = theme_color(effective_config, "nodeBorder", "#9370DB");
    let main_bkg = theme_color(effective_config, "mainBkg", "#ECECFF");
    let tertiary = theme_color(
        effective_config,
        "tertiaryColor",
        "hsl(80, 100%, 96.2745098039%)",
    );
    let text_color = theme_color(effective_config, "textColor", "#333333");
    let node_text_color = theme_color(effective_config, "nodeTextColor", &text_color);
    let font_family = config_string(effective_config, &["fontFamily"])
        .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
        .map(|s| normalize_css_font_family(&s))
        .unwrap_or_else(|| "Arial, Helvetica, sans-serif".to_string());
    // Mermaid ER unified output defaults to the global Mermaid fontSize (16px) via `#id{font-size:...}`.
    let font_size = effective_config
        .get("fontSize")
        .and_then(|v| v.as_f64())
        .or_else(|| {
            effective_config
                .get("er")
                .and_then(|v| v.get("fontSize"))
                .and_then(|v| v.as_f64())
        })
        .unwrap_or(16.0)
        .max(1.0);
    let title_top_margin = effective_config
        .get("er")
        .and_then(|v| v.get("titleTopMargin"))
        .and_then(|v| v.as_f64())
        .or_else(|| {
            effective_config
                .get("titleTopMargin")
                .and_then(|v| v.as_f64())
        })
        .unwrap_or(25.0)
        .max(0.0);
    let use_max_width = effective_config
        .get("er")
        .and_then(|v| v.get("useMaxWidth"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let label_style = crate::text::TextStyle {
        font_family: Some(font_family.clone()),
        font_size,
        font_weight: None,
    };
    let attr_style = crate::text::TextStyle {
        font_family: Some(font_family.clone()),
        font_size: font_size.max(1.0),
        font_weight: None,
    };
    fn parse_trailing_index(id: &str) -> Option<i64> {
        let (_, tail) = id.rsplit_once('-')?;
        tail.parse::<i64>().ok()
    }
    fn er_node_sort_key(id: &str) -> (i64, i64) {
        if id.contains("---") {
            return (1, parse_trailing_index(id).unwrap_or(i64::MAX));
        }
        (0, parse_trailing_index(id).unwrap_or(i64::MAX))
    }

    let mut nodes = layout.nodes.clone();
    nodes.sort_by_key(|n| er_node_sort_key(&n.id));

    let mut edges = layout.edges.clone();
    fn er_edge_sort_key(id: &str) -> (i64, i64) {
        let Some(rest) = id.strip_prefix("er-rel-") else {
            return (i64::MAX, i64::MAX);
        };
        let mut digits_len = 0usize;
        for ch in rest.chars() {
            if !ch.is_ascii_digit() {
                break;
            }
            digits_len += ch.len_utf8();
        }
        if digits_len == 0 {
            return (i64::MAX, i64::MAX);
        }
        let Ok(idx) = rest[..digits_len].parse::<i64>() else {
            return (i64::MAX, i64::MAX);
        };
        let suffix = &rest[digits_len..];
        let variant = match suffix {
            "-cyclic-0" => 0,
            "" => 1,
            "-cyclic-2" => 2,
            _ => 99,
        };
        (idx, variant)
    }
    edges.sort_by_key(|e| er_edge_sort_key(&e.id));

    let include_md_parent = edges.iter().any(|e| {
        matches!(
            e.start_marker.as_deref(),
            Some("MD_PARENT_START") | Some("MD_PARENT_END")
        ) || matches!(
            e.end_marker.as_deref(),
            Some("MD_PARENT_START") | Some("MD_PARENT_END")
        )
    });

    let bounds = compute_layout_bounds(&[], &nodes, &edges).unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });

    let diagram_title = diagram_title.map(str::trim).filter(|t| !t.is_empty());

    let mut content_bounds = bounds.clone();
    let mut title_x = 0.0;
    let mut title_y = 0.0;
    if let Some(title) = diagram_title {
        let title_style = crate::text::TextStyle {
            font_family: Some(font_family.clone()),
            font_size: 18.0,
            font_weight: None,
        };
        let measure = measurer.measure(title, &title_style);
        let w = (content_bounds.max_x - content_bounds.min_x).max(1.0);
        title_x = content_bounds.min_x + w / 2.0;
        title_y = -title_top_margin;
        let title_min_x = title_x - measure.width / 2.0;
        let title_max_x = title_x + measure.width / 2.0;
        // Approximate the SVG text bbox using the measured height above the baseline.
        let title_min_y = title_y - measure.height;
        let title_max_y = title_y;
        content_bounds.min_x = content_bounds.min_x.min(title_min_x);
        content_bounds.max_x = content_bounds.max_x.max(title_max_x);
        content_bounds.min_y = content_bounds.min_y.min(title_min_y);
        content_bounds.max_y = content_bounds.max_y.max(title_max_y);
    }

    let pad = options.viewbox_padding.max(0.0);
    let content_w = (content_bounds.max_x - content_bounds.min_x).max(1.0);
    let content_h = (content_bounds.max_y - content_bounds.min_y).max(1.0);
    let vb_w = content_w + pad * 2.0;
    let vb_h = content_h + pad * 2.0;
    let translate_x = pad - content_bounds.min_x;
    let translate_y = pad - content_bounds.min_y;

    let mut out = String::new();
    let w_attr = fmt(vb_w.max(1.0));
    let h_attr = fmt(vb_h.max(1.0));
    if use_max_width {
        let _ = write!(
            &mut out,
            r#"<svg id="{}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="erDiagram" style="max-width: {}px; background-color: white;" viewBox="0 0 {} {}" role="graphics-document document" aria-roledescription="{}""#,
            escape_xml(diagram_id),
            w_attr,
            w_attr,
            h_attr,
            diagram_type
        );
    } else {
        let _ = write!(
            &mut out,
            r#"<svg id="{}" width="{}" height="{}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="erDiagram" style="background-color: white;" viewBox="0 0 {} {}" role="graphics-document document" aria-roledescription="{}""#,
            escape_xml(diagram_id),
            w_attr,
            h_attr,
            w_attr,
            h_attr,
            diagram_type
        );
    }

    let has_acc_title = model.acc_title.as_ref().is_some_and(|s| !s.is_empty());
    let has_acc_descr = model.acc_descr.as_ref().is_some_and(|s| !s.is_empty());
    if has_acc_title {
        let _ = write!(
            &mut out,
            r#" aria-labelledby="chart-title-{}""#,
            escape_xml(diagram_id)
        );
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#" aria-describedby="chart-desc-{}""#,
            escape_xml(diagram_id)
        );
    }
    out.push('>');
    out.push('\n');

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}"#,
            escape_xml(diagram_id),
            escape_xml(model.acc_title.as_deref().unwrap_or_default())
        );
        out.push_str("</title>");
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}"#,
            escape_xml(diagram_id),
            escape_xml(model.acc_descr.as_deref().unwrap_or_default())
        );
        out.push_str("</desc>");
    }

    let _ = writeln!(
        &mut out,
        r#"<style>
  .erDiagramTitleText {{ text-anchor: middle; font-size: 18px; fill: {}; font-family: {}; }}
  .entityBox {{ fill: {}; stroke: {}; stroke-width: 1px; }}
  .relationshipLine {{ stroke: {}; stroke-width: 1; fill: none; }}
  .relationshipLabelBox {{ fill: {}; opacity: 0.7; }}
  .edge-pattern-dashed {{ stroke-dasharray: 8,8; }}
  .relationshipLabel {{ fill: {}; font-family: {}; dominant-baseline: middle; text-anchor: middle; }}
  .entityLabel {{ fill: {}; font-family: {}; dominant-baseline: middle; text-anchor: middle; }}
  .attributeText {{ fill: {}; font-family: {}; dominant-baseline: middle; text-anchor: left; }}
  .attributeBoxOdd {{ fill: rgba(0,0,0,0.03); stroke: {}; stroke-width: 0; }}
  .attributeBoxEven {{ fill: rgba(0,0,0,0.06); stroke: {}; stroke-width: 0; }}
</style>"#,
        text_color,
        escape_xml(&font_family),
        main_bkg,
        node_border,
        stroke,
        tertiary,
        node_text_color,
        escape_xml(&font_family),
        node_text_color,
        escape_xml(&font_family),
        node_text_color,
        escape_xml(&font_family),
        node_border,
        node_border
    );

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");

    // Markers ported from Mermaid `@11.12.2` `erMarkers.js`.
    // Note: ids follow Mermaid marker rules: `${diagramId}_${diagramType}-${markerType}{Start|End}`.
    // Mermaid's ER unified renderer enables four marker types by default; include MD_PARENT only if used.
    let diagram_id_esc = escape_xml(diagram_id);
    let diagram_type_esc = escape_xml(diagram_type);

    // Mermaid emits one `<defs>` wrapper per marker.
    if include_md_parent {
        let _ = writeln!(
            &mut out,
            r#"<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-mdParentStart" class="marker mdParent er" refX="0" refY="7" markerWidth="190" markerHeight="240" orient="auto"><path d="M 18,7 L9,13 L1,7 L9,1 Z"/></marker></defs>
<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-mdParentEnd" class="marker mdParent er" refX="19" refY="7" markerWidth="20" markerHeight="28" orient="auto"><path d="M 18,7 L9,13 L1,7 L9,1 Z"/></marker></defs>"#
        );
    }

    let _ = writeln!(
        &mut out,
        r#"<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-onlyOneStart" class="marker onlyOne er" refX="0" refY="9" markerWidth="18" markerHeight="18" orient="auto"><path d="M9,0 L9,18 M15,0 L15,18"/></marker></defs>
<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-onlyOneEnd" class="marker onlyOne er" refX="18" refY="9" markerWidth="18" markerHeight="18" orient="auto"><path d="M3,0 L3,18 M9,0 L9,18"/></marker></defs>
<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-zeroOrOneStart" class="marker zeroOrOne er" refX="0" refY="9" markerWidth="30" markerHeight="18" orient="auto"><circle fill="white" cx="21" cy="9" r="6"/><path d="M9,0 L9,18"/></marker></defs>
<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-zeroOrOneEnd" class="marker zeroOrOne er" refX="30" refY="9" markerWidth="30" markerHeight="18" orient="auto"><circle fill="white" cx="9" cy="9" r="6"/><path d="M21,0 L21,18"/></marker></defs>
<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-oneOrMoreStart" class="marker oneOrMore er" refX="18" refY="18" markerWidth="45" markerHeight="36" orient="auto"><path d="M0,18 Q 18,0 36,18 Q 18,36 0,18 M42,9 L42,27"/></marker></defs>
<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-oneOrMoreEnd" class="marker oneOrMore er" refX="27" refY="18" markerWidth="45" markerHeight="36" orient="auto"><path d="M3,9 L3,27 M9,18 Q27,0 45,18 Q27,36 9,18"/></marker></defs>
<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-zeroOrMoreStart" class="marker zeroOrMore er" refX="18" refY="18" markerWidth="57" markerHeight="36" orient="auto"><circle fill="white" cx="48" cy="18" r="6"/><path d="M0,18 Q18,0 36,18 Q18,36 0,18"/></marker></defs>
<defs><marker id="{diagram_id_esc}_{diagram_type_esc}-zeroOrMoreEnd" class="marker zeroOrMore er" refX="39" refY="18" markerWidth="57" markerHeight="36" orient="auto"><circle fill="white" cx="9" cy="18" r="6"/><path d="M21,18 Q39,0 57,18 Q39,36 21,18"/></marker></defs>"#
    );

    let _ = writeln!(&mut out, r#"<g class="root">"#);

    if let Some(title) = diagram_title {
        let _ = writeln!(
            &mut out,
            r#"<text class="erDiagramTitleText" x="{}" y="{}">{}</text>"#,
            fmt(title_x + translate_x),
            fmt(title_y + translate_y),
            escape_xml(title)
        );
    }

    let mut entity_by_id: std::collections::HashMap<&str, &crate::er::ErEntity> =
        std::collections::HashMap::new();
    for e in model.entities.values() {
        entity_by_id.insert(e.id.as_str(), e);
    }

    out.push_str(r#"<g class="clusters"/>"#);

    fn er_rel_idx_from_edge_id(edge_id: &str) -> Option<usize> {
        let rest = edge_id.strip_prefix("er-rel-")?;
        let mut digits_len = 0usize;
        for ch in rest.chars() {
            if !ch.is_ascii_digit() {
                break;
            }
            digits_len += ch.len_utf8();
        }
        if digits_len == 0 {
            return None;
        }
        rest[..digits_len].parse::<usize>().ok()
    }

    fn er_edge_dom_id(edge_id: &str, relationships: &[crate::er::ErRelationship]) -> String {
        let Some(idx) = er_rel_idx_from_edge_id(edge_id) else {
            return edge_id.to_string();
        };
        let Some(rel) = relationships.get(idx) else {
            return edge_id.to_string();
        };
        let rest = edge_id.strip_prefix("er-rel-").unwrap_or("");
        let idx_prefix = idx.to_string();
        let suffix = rest.strip_prefix(&idx_prefix).unwrap_or("");
        if rel.entity_a == rel.entity_b {
            return match suffix {
                "-cyclic-0" => format!("{}-cyclic-special-1", rel.entity_a),
                "" => format!("{}-cyclic-special-mid", rel.entity_a),
                "-cyclic-2" => format!("{}-cyclic-special-2", rel.entity_a),
                _ => format!("{}-cyclic-special-mid", rel.entity_a),
            };
        }
        format!("id_{}_{}_{}", rel.entity_a, rel.entity_b, idx)
    }

    out.push_str(r#"<g class="edgePaths">"#);
    if options.include_edges {
        for e in &edges {
            if e.points.len() < 2 {
                continue;
            }
            let edge_dom_id = er_edge_dom_id(&e.id, &model.relationships);
            let is_dashed = e.stroke_dasharray.as_deref() == Some("8,8");
            let pattern_class = if is_dashed {
                "edge-pattern-dashed"
            } else {
                "edge-pattern-solid"
            };
            let line_classes = format!("edge-thickness-normal {pattern_class} relationshipLine");
            let shifted: Vec<crate::model::LayoutPoint> = e
                .points
                .iter()
                .map(|p| crate::model::LayoutPoint {
                    x: p.x + translate_x,
                    y: p.y + translate_y,
                })
                .collect();
            let data_points = base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_vec(&shifted).unwrap_or_default());
            let d = curve_basis_path_d(&shifted);

            let _ = write!(
                &mut out,
                r#"<path d="{}" id="{}" class="{}" style="undefined;;;undefined" data-edge="true" data-et="edge" data-id="{}" data-points="{}""#,
                escape_xml(&d),
                escape_xml(&edge_dom_id),
                escape_xml(&line_classes),
                escape_xml(&edge_dom_id),
                escape_xml(&data_points)
            );
            if let Some(m) = &e.start_marker {
                let marker = er_unified_marker_id(diagram_id, diagram_type, m);
                let _ = write!(&mut out, r#" marker-start="url(#{})""#, escape_xml(&marker));
            }
            if let Some(m) = &e.end_marker {
                let marker = er_unified_marker_id(diagram_id, diagram_type, m);
                let _ = write!(&mut out, r#" marker-end="url(#{})""#, escape_xml(&marker));
            }
            out.push_str(" />");
        }
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="edgeLabels">"#);
    if options.include_edges {
        for e in &edges {
            let rel_idx = er_rel_idx_from_edge_id(&e.id)
                .and_then(|idx| model.relationships.get(idx).map(|r| (idx, r)));

            let rel_text = rel_idx.map(|(_, r)| r.role_a.as_str()).unwrap_or("").trim();
            let edge_dom_id = er_edge_dom_id(&e.id, &model.relationships);

            let has_label_text = !rel_text.is_empty();
            let (w, h, cx, cy) = if has_label_text {
                if let Some(lbl) = &e.label {
                    (
                        lbl.width.max(0.0),
                        lbl.height.max(0.0),
                        lbl.x + translate_x,
                        lbl.y + translate_y,
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                }
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            if has_label_text && w > 0.0 && h > 0.0 {
                let _ = write!(
                    &mut out,
                    r#"<g class="edgeLabel" transform="translate({}, {})">"#,
                    fmt(cx),
                    fmt(cy)
                );
                let _ = write!(
                    &mut out,
                    r#"<g class="label" data-id="{}" transform="translate({}, {})">"#,
                    escape_xml(&edge_dom_id),
                    fmt(-w / 2.0),
                    fmt(-h / 2.0)
                );
                let _ = write!(
                    &mut out,
                    r#"<foreignObject width="{}" height="{}">"#,
                    fmt(w),
                    fmt(h)
                );
                out.push_str(r#"<div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"><p>"#);
                out.push_str(&escape_xml(rel_text));
                out.push_str(r#"</p></span></div></foreignObject></g></g>"#);
            } else {
                out.push_str(r#"<g class="edgeLabel"><g class="label""#);
                let _ = write!(&mut out, r#" data-id="{}""#, escape_xml(&edge_dom_id));
                out.push_str(r#" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#);
            }
        }
    }
    out.push_str("</g>\n");

    // Entities drawn after relationships so they cover markers when overlapping.
    out.push_str(r#"<g class="nodes">"#);
    for n in &nodes {
        let Some(entity) = entity_by_id.get(n.id.as_str()).copied() else {
            if n.id.contains("---") {
                let cx = n.x + translate_x;
                let cy = n.y + translate_y;
                let _ = write!(
                    &mut out,
                    r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})">"#,
                    escape_xml(&n.id),
                    fmt(cx),
                    fmt(cy)
                );
                out.push_str(r#"<rect width="0.1" height="0.1"/>"#);
                out.push_str(r#"<g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#);
            }
            continue;
        };

        let (rect_style_decls, text_style_decls) = compile_er_entity_styles(entity, &model.classes);
        let rect_style_attr = if rect_style_decls.is_empty() {
            r#"style="""#.to_string()
        } else {
            format!(
                r#"style="{}""#,
                escape_xml(&style_decls_with_important(&rect_style_decls))
            )
        };
        let label_style_attr = if text_style_decls.is_empty() {
            r#"style="""#.to_string()
        } else {
            format!(
                r#"style="{}""#,
                escape_xml(&style_decls_with_important(&text_style_decls))
            )
        };

        let measure = crate::er::measure_entity_box(
            entity,
            measurer,
            &label_style,
            &attr_style,
            effective_config,
        );
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        if (measure.width - w).abs() > 1e-3 || (measure.height - h).abs() > 1e-3 {
            return Err(Error::InvalidModel {
                message: format!(
                    "ER entity measured size mismatch for {}: layout=({},{}), measure=({}, {})",
                    n.id, w, h, measure.width, measure.height
                ),
            });
        }

        let cx = n.x + translate_x;
        let cy = n.y + translate_y;
        let ox = -w / 2.0;
        let oy = -h / 2.0;

        let group_class = if entity.css_classes.trim().is_empty() {
            "node".to_string()
        } else {
            format!("node {}", entity.css_classes.trim())
        };
        let _ = write!(
            &mut out,
            r#"<g id="{}" class="{}" transform="translate({}, {})">"#,
            escape_xml(&entity.id),
            escape_xml(&group_class),
            fmt(cx),
            fmt(cy)
        );

        if entity.attributes.is_empty() {
            let _ = write!(
                &mut out,
                r#"<rect class="basic label-container" {} x="{}" y="{}" width="{}" height="{}"/>"#,
                rect_style_attr,
                fmt(ox),
                fmt(oy),
                fmt(w),
                fmt(h)
            );
            let html_labels = effective_config
                .get("htmlLabels")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let wrap_mode = if html_labels {
                crate::text::WrapMode::HtmlLike
            } else {
                crate::text::WrapMode::SvgLike
            };
            let label_metrics =
                measurer.measure_wrapped(&measure.label_text, &label_style, None, wrap_mode);
            let lw = label_metrics.width.max(0.0);
            let lh = label_metrics.height.max(0.0);

            let _ = write!(
                &mut out,
                r#"<g class="label" transform="translate({}, {})" {}><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel"><p>{}</p></span></div></foreignObject></g>"#,
                fmt(-lw / 2.0),
                fmt(-lh / 2.0),
                label_style_attr,
                fmt(lw),
                fmt(lh),
                escape_xml(&measure.label_text)
            );
            out.push_str("</g>");
            continue;
        }

        fn rect_fill_path_d(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
            format!(
                "M{} {} L{} {} L{} {} L{} {}",
                fmt(x0),
                fmt(y0),
                fmt(x1),
                fmt(y0),
                fmt(x1),
                fmt(y1),
                fmt(x0),
                fmt(y1)
            )
        }

        fn rough_line_path_d(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
            let c1x = x0 + (x1 - x0) * 0.25;
            let c1y = y0 + (y1 - y0) * 0.25;
            let c2x = x0 + (x1 - x0) * 0.75;
            let c2y = y0 + (y1 - y0) * 0.75;
            let d1 = format!(
                "M{} {} C{} {}, {} {}, {} {}",
                fmt(x0),
                fmt(y0),
                fmt(c1x),
                fmt(c1y),
                fmt(c2x),
                fmt(c2y),
                fmt(x1),
                fmt(y1)
            );
            let c1x2 = x0 + (x1 - x0) * 0.35;
            let c1y2 = y0 + (y1 - y0) * 0.15;
            let c2x2 = x0 + (x1 - x0) * 0.65;
            let c2y2 = y0 + (y1 - y0) * 0.85;
            let d2 = format!(
                "M{} {} C{} {}, {} {}, {} {}",
                fmt(x0),
                fmt(y0),
                fmt(c1x2),
                fmt(c1y2),
                fmt(c2x2),
                fmt(c2y2),
                fmt(x1),
                fmt(y1)
            );
            format!("{d1} {d2}")
        }

        fn rough_rect_border_path_d(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
            let top = rough_line_path_d(x0, y0, x1, y0);
            let right = rough_line_path_d(x1, y0, x1, y1);
            let bottom = rough_line_path_d(x1, y1, x0, y1);
            let left = rough_line_path_d(x0, y1, x0, y0);
            format!("{top} {right} {bottom} {left}")
        }

        fn html_label_content(text: &str, span_style_attr: &str) -> String {
            let text = text.trim();
            if text.is_empty() {
                return format!(r#"<span class="nodeLabel"{}></span>"#, span_style_attr);
            }
            // Mermaid's DOM serialization for generics (`type<T>`) avoids nested HTML tags.
            if text.contains('<') || text.contains('>') {
                return escape_xml(text);
            }
            format!(
                r#"<span class="nodeLabel"{}><p>{}</p></span>"#,
                span_style_attr,
                escape_xml(text)
            )
        }

        fn parse_hex_color_rgb(s: &str) -> Option<(u8, u8, u8)> {
            let s = s.trim();
            let Some(hex) = s.strip_prefix('#') else {
                return None;
            };
            if hex.len() == 3 {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                return Some((r, g, b));
            }
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                return Some((r, g, b));
            }
            None
        }

        let label_div_color_prefix = last_style_value(&text_style_decls, "color")
            .and_then(|v| parse_hex_color_rgb(&v))
            .map(|(r, g, b)| format!("color: rgb({r}, {g}, {b}) !important; "))
            .unwrap_or_default();
        let span_style_attr = if text_style_decls.is_empty() {
            String::new()
        } else {
            format!(
                r#" style="{}""#,
                escape_xml(&style_decls_with_important(&text_style_decls))
            )
        };

        // Mermaid ER attribute tables (erBox.ts) use HTML labels (`foreignObject`) and paths for the table rows.
        let name_row_h = (measure.label_height + measure.text_padding).max(1.0);
        let box_x0 = ox;
        let box_y0 = oy;
        let box_x1 = ox + w;
        let box_y1 = oy + h;
        let sep_y = oy + name_row_h;

        let box_fill =
            last_style_value(&rect_style_decls, "fill").unwrap_or_else(|| main_bkg.clone());
        let box_stroke =
            last_style_value(&rect_style_decls, "stroke").unwrap_or_else(|| node_border.clone());
        let box_stroke_width = last_style_value(&rect_style_decls, "stroke-width")
            .and_then(|v| parse_px_f64(&v))
            .unwrap_or(1.3)
            .max(0.0);

        let stroke_width_attr = fmt(box_stroke_width);

        let group_style = concat_style_keys(&rect_style_decls, &["fill", "stroke", "stroke-width"]);
        let group_style_attr = if group_style.is_empty() {
            r#"style="""#.to_string()
        } else {
            format!(r#"style="{}""#, escape_xml(&group_style))
        };

        let mut override_decls: Vec<String> = Vec::new();
        if let Some(v) = last_style_value(&rect_style_decls, "stroke") {
            override_decls.push(format!("stroke:{v}"));
        }
        if let Some(v) = last_style_value(&rect_style_decls, "stroke-width") {
            override_decls.push(format!("stroke-width:{v}"));
        }
        let override_style = if override_decls.is_empty() {
            None
        } else {
            Some(style_decls_with_important(&override_decls))
        };
        let override_style_attr = override_style
            .as_deref()
            .map(|s| format!(r#" style="{}""#, escape_xml(s)))
            .unwrap_or_default();

        // Base box (fill + border)
        let _ = write!(&mut out, r#"<g {}>"#, group_style_attr);
        let _ = write!(
            &mut out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"{} />"#,
            rect_fill_path_d(box_x0, box_y0, box_x1, box_y1),
            escape_xml(&box_fill),
            override_style_attr
        );
        let _ = write!(
            &mut out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
            rough_rect_border_path_d(box_x0, box_y0, box_x1, box_y1),
            escape_xml(&box_stroke),
            stroke_width_attr,
            override_style_attr
        );
        out.push_str("</g>");

        // Row rectangles
        let odd_fill = "hsl(240, 100%, 100%)";
        let even_fill = "hsl(240, 100%, 97.2745098039%)";
        let mut y = sep_y;
        for (idx, row) in measure.rows.iter().enumerate() {
            let row_h = row.height.max(1.0);
            let y0 = y;
            let y1 = y + row_h;
            y = y1;
            let is_odd = idx % 2 == 0;
            let row_class = if is_odd {
                "row-rect-odd"
            } else {
                "row-rect-even"
            };
            let row_fill = if is_odd { odd_fill } else { even_fill };
            let _ = write!(
                &mut out,
                r#"<g {} class="{}">"#,
                group_style_attr, row_class
            );
            let row_override_style_attr =
                if !is_odd && last_style_value(&rect_style_decls, "fill").is_some() {
                    let mut decls: Vec<String> = Vec::new();
                    if let Some(v) = last_style_value(&rect_style_decls, "fill") {
                        decls.push(format!("fill:{v}"));
                    }
                    if let Some(v) = last_style_value(&rect_style_decls, "stroke") {
                        decls.push(format!("stroke:{v}"));
                    }
                    if let Some(v) = last_style_value(&rect_style_decls, "stroke-width") {
                        decls.push(format!("stroke-width:{v}"));
                    }
                    if decls.is_empty() {
                        override_style_attr.clone()
                    } else {
                        let s = style_decls_with_important_join(&decls, ";");
                        format!(r#" style="{}""#, escape_xml(&s))
                    }
                } else {
                    override_style_attr.clone()
                };
            let _ = write!(
                &mut out,
                r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"{} />"#,
                rect_fill_path_d(box_x0, y0, box_x1, y1),
                row_fill,
                row_override_style_attr
            );
            let _ = write!(
                &mut out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                rough_rect_border_path_d(box_x0, y0, box_x1, y1),
                escape_xml(&node_border),
                stroke_width_attr,
                row_override_style_attr
            );
            out.push_str("</g>");
        }

        // HTML labels
        let line_h = (font_size * 1.5).max(1.0);
        let name_w = measurer
            .measure_wrapped(
                &measure.label_text,
                &label_style,
                None,
                crate::text::WrapMode::HtmlLike,
            )
            .width
            .max(0.0);
        let name_x = -name_w / 2.0;
        let name_y = oy + name_row_h / 2.0 - line_h / 2.0;
        let _ = write!(
            &mut out,
            r#"<g class="label name" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: start;">{}"#,
            fmt(name_x),
            fmt(name_y),
            label_style_attr,
            fmt(name_w),
            fmt(line_h),
            escape_xml(&label_div_color_prefix),
            html_label_content(&measure.label_text, &span_style_attr)
        );
        out.push_str("</div></foreignObject></g>");

        let type_col_w = measure.type_col_w.max(0.0);
        let name_col_w = measure.name_col_w.max(0.0);
        let key_col_w = measure.key_col_w.max(0.0);
        let comment_col_w = measure.comment_col_w.max(0.0);

        let type_center = ox + type_col_w / 2.0;
        let name_center = ox + type_col_w + name_col_w / 2.0;
        let key_center = ox + type_col_w + name_col_w + key_col_w / 2.0;
        let comment_center = ox + type_col_w + name_col_w + key_col_w + comment_col_w / 2.0;

        let mut row_top = sep_y;
        for row in &measure.rows {
            let row_h = row.height.max(1.0);
            let cell_y = row_top + row_h / 2.0 - line_h / 2.0;

            let type_w = measurer
                .measure_wrapped(
                    &row.type_text,
                    &attr_style,
                    None,
                    crate::text::WrapMode::HtmlLike,
                )
                .width
                .max(0.0);
            let name_w = measurer
                .measure_wrapped(
                    &row.name_text,
                    &attr_style,
                    None,
                    crate::text::WrapMode::HtmlLike,
                )
                .width
                .max(0.0);
            let keys_w = measurer
                .measure_wrapped(
                    &row.key_text,
                    &attr_style,
                    None,
                    crate::text::WrapMode::HtmlLike,
                )
                .width
                .max(0.0);
            let comment_w = measurer
                .measure_wrapped(
                    &row.comment_text,
                    &attr_style,
                    None,
                    crate::text::WrapMode::HtmlLike,
                )
                .width
                .max(0.0);

            let _ = write!(
                &mut out,
                r#"<g class="label attribute-type" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: start;">{}"#,
                fmt(type_center - type_w / 2.0),
                fmt(cell_y),
                label_style_attr,
                fmt(type_w),
                fmt(line_h),
                escape_xml(&label_div_color_prefix),
                html_label_content(&row.type_text, &span_style_attr)
            );
            out.push_str("</div></foreignObject></g>");

            let _ = write!(
                &mut out,
                r#"<g class="label attribute-name" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: start;">{}"#,
                fmt(name_center - name_w / 2.0),
                fmt(cell_y),
                label_style_attr,
                fmt(name_w),
                fmt(line_h),
                escape_xml(&label_div_color_prefix),
                html_label_content(&row.name_text, &span_style_attr)
            );
            out.push_str("</div></foreignObject></g>");

            let _ = write!(
                &mut out,
                r#"<g class="label attribute-keys" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: start;">{}"#,
                fmt(key_center - keys_w / 2.0),
                fmt(cell_y),
                label_style_attr,
                fmt(keys_w),
                fmt(if row.key_text.trim().is_empty() {
                    0.0
                } else {
                    line_h
                }),
                escape_xml(&label_div_color_prefix),
                html_label_content(&row.key_text, &span_style_attr)
            );
            out.push_str("</div></foreignObject></g>");

            let _ = write!(
                &mut out,
                r#"<g class="label attribute-comment" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: start;">{}"#,
                fmt(comment_center - comment_w / 2.0),
                fmt(cell_y),
                label_style_attr,
                fmt(comment_w),
                fmt(if row.comment_text.trim().is_empty() {
                    0.0
                } else {
                    line_h
                }),
                escape_xml(&label_div_color_prefix),
                html_label_content(&row.comment_text, &span_style_attr)
            );
            out.push_str("</div></foreignObject></g>");

            row_top += row_h;
        }

        // Dividers (header separator + column boundaries)
        let divider_style = override_style_attr.clone();
        let divider_path_attrs = format!(
            r#" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{}"#,
            escape_xml(&box_stroke),
            stroke_width_attr,
            divider_style
        );
        // Two rough strokes for the header separator.
        let d_h1 = rough_line_path_d(box_x0, sep_y, box_x1, sep_y);
        let d_h2 = rough_line_path_d(box_x0, sep_y, box_x1, sep_y);
        let _ = write!(
            &mut out,
            r#"<g class="divider"><path d="{}"{} /></g>"#,
            d_h1, divider_path_attrs
        );

        let mut divider_xs: Vec<f64> = Vec::new();
        divider_xs.push(ox + type_col_w);
        if measure.has_key || measure.has_comment {
            divider_xs.push(ox + type_col_w + name_col_w);
        }
        if measure.has_comment {
            divider_xs.push(ox + type_col_w + name_col_w + key_col_w);
        }
        for x in divider_xs {
            let dv = rough_line_path_d(x, sep_y, x, box_y1);
            let _ = write!(
                &mut out,
                r#"<g class="divider"><path d="{}"{} /></g>"#,
                dv, divider_path_attrs
            );
        }

        let _ = write!(
            &mut out,
            r#"<g class="divider"><path d="{}"{} /></g>"#,
            d_h2, divider_path_attrs
        );

        out.push_str("</g>");
    }
    out.push_str("</g>\n");

    out.push_str("</g>\n</g>\n</svg>\n");
    Ok(out)
}

fn er_unified_marker_id(diagram_id: &str, diagram_type: &str, legacy_marker: &str) -> String {
    let legacy_marker = legacy_marker.trim();
    let (base, suffix) = if let Some(v) = legacy_marker.strip_suffix("_START") {
        (v, "Start")
    } else if let Some(v) = legacy_marker.strip_suffix("_END") {
        (v, "End")
    } else {
        return legacy_marker.to_string();
    };

    let marker_type = match base {
        "ONLY_ONE" => "onlyOne",
        "ZERO_OR_ONE" => "zeroOrOne",
        "ONE_OR_MORE" => "oneOrMore",
        "ZERO_OR_MORE" => "zeroOrMore",
        "MD_PARENT" => "mdParent",
        _ => return legacy_marker.to_string(),
    };

    format!("{diagram_id}_{diagram_type}-{marker_type}{suffix}")
}

// Ported from D3 `curveLinear` (d3-shape v3.x).
fn curve_linear_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let _ = write!(&mut out, "M {},{}", fmt(first.x), fmt(first.y));
    for p in points.iter().skip(1) {
        let _ = write!(&mut out, " L {},{}", fmt(p.x), fmt(p.y));
    }
    out
}

// Ported from D3 `curveStepAfter` (d3-shape v3.x).
fn curve_step_after_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let mut prev_y = first.y;
    let _ = write!(&mut out, "M {},{}", fmt(first.x), fmt(first.y));
    for p in points.iter().skip(1) {
        let _ = write!(&mut out, " L {},{}", fmt(p.x), fmt(prev_y));
        let _ = write!(&mut out, " L {},{}", fmt(p.x), fmt(p.y));
        prev_y = p.y;
    }
    out
}

// Ported from D3 `curveCardinal` (d3-shape v3.x).
fn curve_cardinal_path_d(points: &[crate::model::LayoutPoint], tension: f64) -> String {
    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let k = (1.0 - tension) / 6.0;

    let mut p = 0u8;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;
    let mut x2 = f64::NAN;
    let mut y2 = f64::NAN;

    fn cardinal_point(
        out: &mut String,
        k: f64,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    ) {
        let c1x = x1 + k * (x2 - x0);
        let c1y = y1 + k * (y2 - y0);
        let c2x = x2 + k * (x1 - x);
        let c2y = y2 + k * (y1 - y);
        let _ = write!(
            out,
            " C {},{} {},{} {},{}",
            fmt(c1x),
            fmt(c1y),
            fmt(c2x),
            fmt(c2y),
            fmt(x2),
            fmt(y2)
        );
    }

    for pt in points {
        let x = pt.x;
        let y = pt.y;
        match p {
            0 => {
                p = 1;
                let _ = write!(&mut out, "M {},{}", fmt(x), fmt(y));
            }
            1 => {
                p = 2;
                x1 = x;
                y1 = y;
            }
            2 => {
                p = 3;
                cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x, y);
            }
            _ => {
                cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x, y);
            }
        }

        x0 = x1;
        x1 = x2;
        x2 = x;
        y0 = y1;
        y1 = y2;
        y2 = y;
    }

    match p {
        2 => {
            let _ = write!(&mut out, " L {},{}", fmt(x2), fmt(y2));
        }
        3 => {
            cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x1, y1);
        }
        _ => {}
    }

    out
}

// Ported from D3 `curveBasis` (d3-shape v3.x), used by Mermaid ER renderer `@11.12.2`.
fn curve_basis_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let mut p = 0u8;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;

    fn basis_point(out: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, x: f64, y: f64) {
        let c1x = (2.0 * x0 + x1) / 3.0;
        let c1y = (2.0 * y0 + y1) / 3.0;
        let c2x = (x0 + 2.0 * x1) / 3.0;
        let c2y = (y0 + 2.0 * y1) / 3.0;
        let ex = (x0 + 4.0 * x1 + x) / 6.0;
        let ey = (y0 + 4.0 * y1 + y) / 6.0;
        let _ = write!(
            out,
            " C {},{} {},{} {},{}",
            fmt(c1x),
            fmt(c1y),
            fmt(c2x),
            fmt(c2y),
            fmt(ex),
            fmt(ey)
        );
    }

    for pt in points {
        let x = pt.x;
        let y = pt.y;
        match p {
            0 => {
                p = 1;
                let _ = write!(&mut out, "M {},{}", fmt(x), fmt(y));
            }
            1 => {
                p = 2;
            }
            2 => {
                p = 3;
                let lx = (5.0 * x0 + x1) / 6.0;
                let ly = (5.0 * y0 + y1) / 6.0;
                let _ = write!(&mut out, " L {},{}", fmt(lx), fmt(ly));
                basis_point(&mut out, x0, y0, x1, y1, x, y);
            }
            _ => {
                basis_point(&mut out, x0, y0, x1, y1, x, y);
            }
        }
        x0 = x1;
        x1 = x;
        y0 = y1;
        y1 = y;
    }

    match p {
        3 => {
            basis_point(&mut out, x0, y0, x1, y1, x1, y1);
            let _ = write!(&mut out, " L {},{}", fmt(x1), fmt(y1));
        }
        2 => {
            let _ = write!(&mut out, " L {},{}", fmt(x1), fmt(y1));
        }
        _ => {}
    }

    out
}

fn render_node(out: &mut String, n: &LayoutNode) {
    let x = n.x - n.width / 2.0;
    let y = n.y - n.height / 2.0;
    let _ = write!(
        out,
        r#"<rect class="node-box" x="{}" y="{}" width="{}" height="{}" />"#,
        fmt(x),
        fmt(y),
        fmt(n.width.max(1.0)),
        fmt(n.height.max(1.0))
    );
    let _ = write!(
        out,
        r#"<text class="node-label" x="{}" y="{}">{}</text>"#,
        fmt(n.x),
        fmt(n.y),
        escape_xml(&n.id)
    );
}

fn render_state_node(out: &mut String, n: &LayoutNode) {
    let is_small_circle = (n.width - n.height).abs() < 1e-6 && n.width <= 20.0 && n.height <= 20.0;
    if is_small_circle {
        let r = (n.width / 2.0).max(1.0);
        let _ = write!(
            out,
            r#"<circle class="node-circle" cx="{}" cy="{}" r="{}" />"#,
            fmt(n.x),
            fmt(n.y),
            fmt(r)
        );
    } else {
        let x = n.x - n.width / 2.0;
        let y = n.y - n.height / 2.0;
        let _ = write!(
            out,
            r#"<rect class="node-box" x="{}" y="{}" width="{}" height="{}" />"#,
            fmt(x),
            fmt(y),
            fmt(n.width.max(1.0)),
            fmt(n.height.max(1.0))
        );
    }

    let _ = write!(
        out,
        r#"<text class="node-label" x="{}" y="{}">{}</text>"#,
        fmt(n.x),
        fmt(n.y),
        escape_xml(&n.id)
    );
}

fn render_cluster(out: &mut String, c: &LayoutCluster, include_markers: bool) {
    let x = c.x - c.width / 2.0;
    let y = c.y - c.height / 2.0;

    let _ = write!(
        out,
        r#"<g id="cluster-{}" data-diff="{}" data-offset-y="{}">"#,
        escape_attr(&c.id),
        fmt(c.diff),
        fmt(c.offset_y)
    );
    let _ = write!(
        out,
        r#"<rect class="cluster-box" x="{}" y="{}" width="{}" height="{}" />"#,
        fmt(x),
        fmt(y),
        fmt(c.width.max(1.0)),
        fmt(c.height.max(1.0))
    );
    let _ = write!(
        out,
        r#"<text class="cluster-title" x="{}" y="{}">{}</text>"#,
        fmt(c.title_label.x),
        fmt(c.title_label.y),
        escape_xml(&c.title)
    );

    if include_markers {
        // Visualize Mermaid's clusterNode translation origin used by `positionNode(...)`:
        // translate(node.x + diff - node.width/2, node.y - node.height/2 - padding).
        let ox = c.x + c.diff - c.width / 2.0;
        let oy = c.y - c.height / 2.0 - c.padding;
        debug_cross(out, ox, oy, 6.0);
    }

    out.push_str("</g>\n");
}

fn debug_cross(out: &mut String, x: f64, y: f64, size: f64) {
    let s = size.abs();
    let _ = write!(
        out,
        r#"<line class="debug-cross" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
        fmt(x - s),
        fmt(y),
        fmt(x + s),
        fmt(y)
    );
    let _ = write!(
        out,
        r#"<line class="debug-cross" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
        fmt(x),
        fmt(y - s),
        fmt(x),
        fmt(y + s)
    );
}

fn compute_layout_bounds(
    clusters: &[LayoutCluster],
    nodes: &[LayoutNode],
    edges: &[crate::model::LayoutEdge],
) -> Option<Bounds> {
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

    for c in clusters {
        let hw = c.width / 2.0;
        let hh = c.height / 2.0;
        include_rect(c.x - hw, c.y - hh, c.x + hw, c.y + hh);
        let lhw = c.title_label.width / 2.0;
        let lhh = c.title_label.height / 2.0;
        include_rect(
            c.title_label.x - lhw,
            c.title_label.y - lhh,
            c.title_label.x + lhw,
            c.title_label.y + lhh,
        );
    }

    for n in nodes {
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        include_rect(n.x - hw, n.y - hh, n.x + hw, n.y + hh);
    }

    for e in edges {
        for p in &e.points {
            include_rect(p.x, p.y, p.x, p.y);
        }
        for lbl in [
            e.label.as_ref(),
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ] {
            if let Some(lbl) = lbl {
                let hw = lbl.width / 2.0;
                let hh = lbl.height / 2.0;
                include_rect(lbl.x - hw, lbl.y - hh, lbl.x + hw, lbl.y + hh);
            }
        }
    }

    b
}

fn fmt(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    if v.abs() < 0.0005 {
        return "0".to_string();
    }
    // Mermaid's upstream SVG tends to use integers when possible, and only includes decimals when
    // needed (e.g. `32.5`). Match that style to make DOM/XML diffs smaller.
    let mut r = (v * 1000.0).round() / 1000.0;
    if r.abs() < 0.0005 {
        r = 0.0;
    }
    let mut s = format!("{r:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr(text: &str) -> String {
    // Attributes in our debug SVG only use escaped XML. No URL encoding here.
    escape_xml(text)
}

struct FlowchartRenderCtx<'a> {
    diagram_id: String,
    diagram_type: String,
    tx: f64,
    ty: f64,
    measurer: &'a dyn TextMeasurer,
    config: merman_core::MermaidConfig,
    html_labels: bool,
    class_defs: std::collections::HashMap<String, Vec<String>>,
    node_border_color: String,
    node_fill_color: String,
    default_edge_interpolate: String,
    node_order: Vec<String>,
    subgraph_order: Vec<String>,
    nodes_by_id: std::collections::HashMap<String, crate::flowchart::FlowNode>,
    edges_by_id: std::collections::HashMap<String, crate::flowchart::FlowEdge>,
    subgraphs_by_id: std::collections::HashMap<String, crate::flowchart::FlowSubgraph>,
    tooltips: std::collections::HashMap<String, String>,
    recursive_clusters: std::collections::HashSet<String>,
    parent: std::collections::HashMap<String, String>,
    layout_nodes_by_id: std::collections::HashMap<String, LayoutNode>,
    layout_edges_by_id: std::collections::HashMap<String, crate::model::LayoutEdge>,
    layout_clusters_by_id: std::collections::HashMap<String, LayoutCluster>,
    node_dom_index: std::collections::HashMap<String, usize>,
    node_padding: f64,
    wrapping_width: f64,
    wrap_mode: crate::text::WrapMode,
    text_style: crate::text::TextStyle,
    diagram_title: Option<String>,
}

fn flowchart_node_dom_indices(
    model: &crate::flowchart::FlowchartV2Model,
) -> std::collections::HashMap<String, usize> {
    if !model.vertex_calls.is_empty() {
        let mut out: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut vertex_counter: usize = 0;
        for id in &model.vertex_calls {
            if !out.contains_key(id) {
                out.insert(id.clone(), vertex_counter);
            }
            vertex_counter += 1;
        }
        return out;
    }

    let mut out: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut vertex_counter: usize = 0;

    // Mermaid FlowDB assigns `domId` when a vertex is first created, but increments the internal
    // `vertexCounter` on every `addVertex(...)` call (even for repeated references). This means the
    // domId suffix depends on the full "first-use" order + repeat uses.
    let touch = |id: &str, out: &mut std::collections::HashMap<String, usize>, c: &mut usize| {
        if !out.contains_key(id) {
            out.insert(id.to_string(), *c);
        }
        *c += 1;
    };

    for e in &model.edges {
        touch(&e.from, &mut out, &mut vertex_counter);
        touch(&e.to, &mut out, &mut vertex_counter);
    }

    for n in &model.nodes {
        touch(&n.id, &mut out, &mut vertex_counter);
    }

    out
}

fn flowchart_css(
    diagram_id: &str,
    effective_config: &serde_json::Value,
    font_family: &str,
    font_size: f64,
    class_defs: &std::collections::HashMap<String, Vec<String>>,
) -> String {
    let id = escape_xml(diagram_id);
    let stroke = theme_color(effective_config, "lineColor", "#333333");
    let node_border = theme_color(effective_config, "nodeBorder", "#9370DB");
    let main_bkg = theme_color(effective_config, "mainBkg", "#ECECFF");
    let text_color = theme_color(effective_config, "textColor", "#333");
    let tertiary = theme_color(
        effective_config,
        "tertiaryColor",
        "hsl(80, 100%, 96.2745098039%)",
    );
    let cluster_bkg = theme_color(effective_config, "clusterBkg", "#ffffde");
    let cluster_border = theme_color(effective_config, "clusterBorder", "#aaaa33");

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:{}px;fill:{};}}"#,
        escape_xml(diagram_id),
        font_family,
        fmt(font_size),
        text_color
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        escape_xml(diagram_id),
        escape_xml(diagram_id)
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        escape_xml(diagram_id),
        escape_xml(diagram_id)
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id)
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:{};stroke:{};}}#{} .marker.cross{{stroke:{};}}"#,
        escape_xml(diagram_id),
        stroke,
        stroke,
        escape_xml(diagram_id),
        stroke
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:{}px;}}#{} p{{margin:0;}}#{} .label{{font-family:{};color:{};}}"#,
        escape_xml(diagram_id),
        font_family,
        fmt(font_size),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        font_family,
        text_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .cluster-label text{{fill:{};}}#{} .cluster-label span{{color:{};}}#{} .cluster-label span p{{background-color:transparent;}}#{} .label text,#{} span{{fill:{};color:{};}}"#,
        escape_xml(diagram_id),
        text_color,
        escape_xml(diagram_id),
        text_color,
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        text_color,
        text_color
    );
    let _ = write!(
        &mut out,
        r#"#{id} .node rect,#{id} .node circle,#{id} .node ellipse,#{id} .node polygon,#{id} .node path{{fill:{main_bkg};stroke:{node_border};stroke-width:1px;}}#{id} .rough-node .label text,#{id} .node .label text,#{id} .image-shape .label,#{id} .icon-shape .label{{text-anchor:middle;}}#{id} .node .katex path{{fill:#000;stroke:#000;stroke-width:1px;}}#{id} .rough-node .label,#{id} .node .label,#{id} .image-shape .label,#{id} .icon-shape .label{{text-align:center;}}#{id} .node.clickable{{cursor:pointer;}}"#
    );
    let _ = write!(
        &mut out,
        r#"#{} .root .anchor path{{fill:{}!important;stroke-width:0;stroke:{};}}#{} .arrowheadPath{{fill:{};}}#{} .edgePath .path{{stroke:{};stroke-width:2.0px;}}#{} .flowchart-link{{stroke:{};fill:none;}}"#,
        escape_xml(diagram_id),
        stroke,
        stroke,
        escape_xml(diagram_id),
        stroke,
        escape_xml(diagram_id),
        stroke,
        escape_xml(diagram_id),
        stroke
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel{{background-color:rgba(232,232,232, 0.8);text-align:center;}}#{} .edgeLabel p{{background-color:rgba(232,232,232, 0.8);}}#{} .edgeLabel rect{{opacity:0.5;background-color:rgba(232,232,232, 0.8);fill:rgba(232,232,232, 0.8);}}#{} .labelBkg{{background-color:rgba(232, 232, 232, 0.5);}}"#,
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id)
    );
    let _ = write!(
        &mut out,
        r#"#{} .cluster rect{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster span{{color:{};}}#{} div.mermaidTooltip{{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:{};font-size:12px;background:{};border:1px solid {};border-radius:2px;pointer-events:none;z-index:100;}}#{} .flowchartTitleText{{text-anchor:middle;font-size:18px;fill:{};}}#{} rect.text{{fill:none;stroke-width:0;}}"#,
        escape_xml(diagram_id),
        cluster_bkg,
        cluster_border,
        escape_xml(diagram_id),
        text_color,
        escape_xml(diagram_id),
        text_color,
        escape_xml(diagram_id),
        font_family,
        tertiary,
        cluster_border,
        escape_xml(diagram_id),
        text_color,
        escape_xml(diagram_id)
    );
    let _ = write!(
        &mut out,
        r#"#{} .icon-shape,#{} .image-shape{{background-color:rgba(232,232,232, 0.8);text-align:center;}}#{} .icon-shape p,#{} .image-shape p{{background-color:rgba(232,232,232, 0.8);padding:2px;}}#{} .icon-shape rect,#{} .image-shape rect{{opacity:0.5;background-color:rgba(232,232,232, 0.8);fill:rgba(232,232,232, 0.8);}}#{} .label-icon{{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}}#{} .node .label-icon path{{fill:currentColor;stroke:revert;stroke-width:revert;}}#{} :root{{--mermaid-font-family:{};}}"#,
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        escape_xml(diagram_id),
        font_family
    );

    for (class, decls) in class_defs {
        if decls.is_empty() {
            continue;
        }
        let mut style = String::new();
        for d in decls {
            let Some((k, v)) = parse_style_decl(d) else {
                continue;
            };
            let _ = write!(&mut style, "{}:{}!important;", k, v);
        }
        if style.is_empty() {
            continue;
        }
        let _ = write!(
            &mut out,
            r#"#{} .{}&gt;*{{{}}}#{} .{} span{{{}}}"#,
            escape_xml(diagram_id),
            escape_xml(class),
            style,
            escape_xml(diagram_id),
            escape_xml(class),
            style
        );
    }

    out
}

fn flowchart_markers(out: &mut String, diagram_id: &str) {
    let _ = write!(
        out,
        r#"<marker id="{}_flowchart-v2-pointEnd" class="marker flowchart-v2" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(diagram_id)
    );
    let _ = write!(
        out,
        r#"<marker id="{}_flowchart-v2-pointStart" class="marker flowchart-v2" viewBox="0 0 10 10" refX="4.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 5 L 10 10 L 10 0 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(diagram_id)
    );
    let _ = write!(
        out,
        r#"<marker id="{}_flowchart-v2-circleEnd" class="marker flowchart-v2" viewBox="0 0 10 10" refX="11" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(diagram_id)
    );
    let _ = write!(
        out,
        r#"<marker id="{}_flowchart-v2-circleStart" class="marker flowchart-v2" viewBox="0 0 10 10" refX="-1" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(diagram_id)
    );
    let _ = write!(
        out,
        r#"<marker id="{}_flowchart-v2-crossEnd" class="marker cross flowchart-v2" viewBox="0 0 11 11" refX="12" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(diagram_id)
    );
    let _ = write!(
        out,
        r#"<marker id="{}_flowchart-v2-crossStart" class="marker cross flowchart-v2" viewBox="0 0 11 11" refX="-1" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(diagram_id)
    );
}

fn flowchart_is_in_cluster(
    parent: &std::collections::HashMap<String, String>,
    _cluster_ids: &std::collections::HashSet<String>,
    node_id: &str,
    cluster_id: &str,
) -> bool {
    if node_id == cluster_id {
        return true;
    }
    let mut cur: Option<&str> = Some(node_id);
    while let Some(id) = cur {
        if id == cluster_id {
            return true;
        }
        cur = parent.get(id).map(|s| s.as_str());
    }
    false
}

fn flowchart_is_strict_descendant(
    parent: &std::collections::HashMap<String, String>,
    node_id: &str,
    cluster_id: &str,
) -> bool {
    if node_id == cluster_id {
        return false;
    }
    let mut cur: Option<&str> = Some(node_id);
    while let Some(id) = cur {
        if id == cluster_id {
            return true;
        }
        cur = parent.get(id).map(|s| s.as_str());
    }
    false
}

fn flowchart_effective_parent<'a>(ctx: &'a FlowchartRenderCtx<'_>, id: &str) -> Option<&'a str> {
    let mut cur = ctx.parent.get(id).map(|s| s.as_str());
    while let Some(p) = cur {
        if ctx.subgraphs_by_id.contains_key(p) && !ctx.recursive_clusters.contains(p) {
            cur = ctx.parent.get(p).map(|s| s.as_str());
            continue;
        }
        return Some(p);
    }
    None
}

fn flowchart_root_children_clusters(
    ctx: &FlowchartRenderCtx<'_>,
    parent_cluster: Option<&str>,
) -> Vec<String> {
    let mut out = Vec::new();
    for (id, _) in &ctx.subgraphs_by_id {
        if !ctx.recursive_clusters.contains(id) {
            continue;
        }
        let parent = flowchart_effective_parent(ctx, id.as_str());
        if parent == parent_cluster {
            out.push(id.clone());
        }
    }
    out.sort_by(|a, b| {
        let aa = ctx.layout_clusters_by_id.get(a);
        let bb = ctx.layout_clusters_by_id.get(b);
        let (al, at) = aa
            .map(|c| (c.x - c.width / 2.0, c.y - c.height / 2.0))
            .unwrap_or((0.0, 0.0));
        let (bl, bt) = bb
            .map(|c| (c.x - c.width / 2.0, c.y - c.height / 2.0))
            .unwrap_or((0.0, 0.0));
        al.total_cmp(&bl)
            .then_with(|| at.total_cmp(&bt))
            .then_with(|| a.cmp(b))
    });
    out
}

fn flowchart_root_children_nodes(
    ctx: &FlowchartRenderCtx<'_>,
    parent_cluster: Option<&str>,
) -> Vec<String> {
    let cluster_ids: std::collections::HashSet<&str> =
        ctx.subgraphs_by_id.keys().map(|k| k.as_str()).collect();
    let mut out = Vec::new();
    for (id, n) in &ctx.nodes_by_id {
        if cluster_ids.contains(id.as_str()) {
            continue;
        }
        let parent = flowchart_effective_parent(ctx, id.as_str());
        if parent == parent_cluster {
            out.push(n.id.clone());
        }
    }
    out.sort_by(|a, b| {
        let aa = ctx.layout_nodes_by_id.get(a);
        let bb = ctx.layout_nodes_by_id.get(b);
        let (ax, ay) = aa.map(|n| (n.x, n.y)).unwrap_or((0.0, 0.0));
        let (bx, by) = bb.map(|n| (n.x, n.y)).unwrap_or((0.0, 0.0));
        ay.total_cmp(&by)
            .then_with(|| ax.total_cmp(&bx))
            .then_with(|| a.cmp(b))
    });
    out
}

fn flowchart_lca(ctx: &FlowchartRenderCtx<'_>, a: &str, b: &str) -> Option<String> {
    let mut ancestors: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cur = flowchart_effective_parent(ctx, a).map(|s| s.to_string());
    while let Some(p) = cur {
        ancestors.insert(p.clone());
        cur = flowchart_effective_parent(ctx, &p).map(|s| s.to_string());
    }

    let mut cur = flowchart_effective_parent(ctx, b).map(|s| s.to_string());
    while let Some(p) = cur {
        if ancestors.contains(&p) {
            return Some(p);
        }
        cur = flowchart_effective_parent(ctx, &p).map(|s| s.to_string());
    }
    None
}

fn flowchart_edges_for_root(
    ctx: &FlowchartRenderCtx<'_>,
    cluster_id: Option<&str>,
) -> Vec<crate::flowchart::FlowEdge> {
    let mut out = Vec::new();
    for e in ctx.edges_by_id.values() {
        let lca = flowchart_lca(ctx, &e.from, &e.to);
        if lca.as_deref() == cluster_id {
            out.push(e.clone());
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn render_flowchart_root(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    cluster_id: Option<&str>,
    parent_origin_x: f64,
    parent_origin_y: f64,
) {
    let (origin_x, origin_y, transform_attr) = if let Some(cid) = cluster_id {
        let Some(c) = ctx.layout_clusters_by_id.get(cid) else {
            return;
        };
        let ox = (c.x - c.width / 2.0) + ctx.tx;
        let oy = (c.y - c.height / 2.0) + ctx.ty;
        let dx = ox - parent_origin_x;
        let dy = oy - parent_origin_y;
        (
            ox,
            oy,
            format!(r#" transform="translate({}, {})""#, fmt(dx), fmt(dy)),
        )
    } else {
        (0.0, 0.0, String::new())
    };

    let _ = write!(out, r#"<g class="root"{}>"#, transform_attr);

    let mut clusters_to_draw: Vec<&LayoutCluster> = Vec::new();
    if let Some(cid) = cluster_id {
        if let Some(cluster) = ctx.layout_clusters_by_id.get(cid) {
            clusters_to_draw.push(cluster);
        }
    }
    for id in ctx.subgraphs_by_id.keys() {
        if cluster_id.is_some_and(|cid| cid == id.as_str()) {
            continue;
        }
        if ctx.recursive_clusters.contains(id) {
            continue;
        }
        if flowchart_effective_parent(ctx, id.as_str()) == cluster_id {
            if let Some(cluster) = ctx.layout_clusters_by_id.get(id.as_str()) {
                clusters_to_draw.push(cluster);
            }
        }
    }
    if clusters_to_draw.is_empty() {
        out.push_str(r#"<g class="clusters"/>"#);
    } else {
        out.push_str(r#"<g class="clusters">"#);
        for cluster in clusters_to_draw {
            render_flowchart_cluster(out, ctx, cluster, origin_x, origin_y);
        }
        out.push_str("</g>");
    }

    let edges = flowchart_edges_for_root(ctx, cluster_id);
    if edges.is_empty() {
        out.push_str(r#"<g class="edgePaths"/>"#);
    } else {
        out.push_str(r#"<g class="edgePaths">"#);
        for e in &edges {
            render_flowchart_edge_path(out, ctx, e, origin_x, origin_y);
        }
        out.push_str("</g>");
    }

    if edges.is_empty() {
        out.push_str(r#"<g class="edgeLabels"/>"#);
    } else {
        out.push_str(r#"<g class="edgeLabels">"#);
        if !ctx.html_labels {
            for _ in &edges {
                out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
            }
        }
        for e in &edges {
            render_flowchart_edge_label(out, ctx, e, origin_x, origin_y);
        }
        out.push_str("</g>");
    }

    out.push_str(r#"<g class="nodes">"#);

    let child_clusters = flowchart_root_children_clusters(ctx, cluster_id);
    for child in &child_clusters {
        render_flowchart_root(out, ctx, Some(child.as_str()), origin_x, origin_y);
    }

    let child_nodes = flowchart_root_children_nodes(ctx, cluster_id);
    for node_id in &child_nodes {
        render_flowchart_node(out, ctx, node_id, origin_x, origin_y);
    }

    out.push_str("</g></g>");
}

fn render_flowchart_cluster(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) {
    let left = (cluster.x - cluster.width / 2.0) + ctx.tx - origin_x;
    let top = (cluster.y - cluster.height / 2.0) + ctx.ty - origin_y;
    let pad = cluster.padding.max(0.0);
    let rect_w = (cluster.width - pad * 2.0).max(1.0);
    let rect_h = (cluster.height - pad * 2.0).max(1.0);
    let label_w = cluster.title_label.width.max(0.0);
    let label_h = cluster.title_label.height.max(0.0);
    let label_x = pad + rect_w / 2.0 - label_w / 2.0;
    let label_y = pad;

    let label_type = ctx
        .subgraphs_by_id
        .get(&cluster.id)
        .and_then(|s| s.label_type.as_deref())
        .unwrap_or("text");

    if !ctx.html_labels {
        let title_text = flowchart_label_plain_text(&cluster.title, label_type);
        let title_lines = crate::text::wrap_text_lines_px(
            &title_text,
            &ctx.text_style,
            Some(200.0),
            crate::text::WrapMode::SvgLike,
        );
        let wrapped_title_text = title_lines.join("\n");
        let label_left = left + label_x;
        let label_top = top + pad + cluster.title_margin_top.max(0.0);
        let _ = write!(
            out,
            r#"<g class="cluster" id="{}" data-look="classic"><rect style="" x="{}" y="{}" width="{}" height="{}"/><g class="cluster-label" transform="translate({}, {})"><g><rect class="background" style="stroke: none"/>"#,
            escape_attr(&cluster.id),
            fmt(left + pad),
            fmt(top + pad),
            fmt(rect_w),
            fmt(rect_h),
            fmt(label_left),
            fmt(label_top)
        );
        write_flowchart_svg_text(out, &wrapped_title_text, true);
        out.push_str("</g></g></g>");
        return;
    }

    let title_html = flowchart_label_html(&cluster.title, label_type, &ctx.config);

    let _ = write!(
        out,
        r#"<g class="cluster" id="{}" data-look="classic"><rect style="" x="{}" y="{}" width="{}" height="{}"/><g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel">{}</span></div></foreignObject></g></g>"#,
        escape_attr(&cluster.id),
        fmt(left + pad),
        fmt(top + pad),
        fmt(rect_w),
        fmt(rect_h),
        fmt(left + label_x),
        fmt(top + label_y),
        fmt(label_w),
        fmt(label_h),
        title_html
    );
}

fn flowchart_edge_marker_end(
    diagram_id: &str,
    edge: &crate::flowchart::FlowEdge,
) -> Option<String> {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => Some(format!(r#"url(#{diagram_id}_flowchart-v2-pointEnd)"#)),
        Some("arrow_point") => Some(format!(r#"url(#{diagram_id}_flowchart-v2-pointEnd)"#)),
        Some("arrow_cross") => Some(format!(r#"url(#{diagram_id}_flowchart-v2-crossEnd)"#)),
        Some("arrow_circle") => Some(format!(r#"url(#{diagram_id}_flowchart-v2-circleEnd)"#)),
        Some("arrow_open") => None,
        _ => Some(format!(r#"url(#{diagram_id}_flowchart-v2-pointEnd)"#)),
    }
}

fn flowchart_edge_marker_start(
    diagram_id: &str,
    edge: &crate::flowchart::FlowEdge,
) -> Option<String> {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => {
            Some(format!(r#"url(#{diagram_id}_flowchart-v2-pointStart)"#))
        }
        _ => None,
    }
}

fn flowchart_edge_class_attr(edge: &crate::flowchart::FlowEdge) -> String {
    // Mermaid includes a 2-part class tuple (thickness/pattern) for flowchart edge paths. The
    // second tuple is `edge-thickness-normal edge-pattern-solid` in Mermaid@11.12.2 baselines,
    // even for dotted/thick strokes.
    let (thickness_1, pattern_1) = match edge.stroke.as_deref() {
        Some("thick") => ("edge-thickness-thick", "edge-pattern-solid"),
        Some("invisible") => ("edge-thickness-invisible", "edge-pattern-solid"),
        Some("dotted") => ("edge-thickness-normal", "edge-pattern-dotted"),
        _ => ("edge-thickness-normal", "edge-pattern-solid"),
    };

    format!("{thickness_1} {pattern_1} edge-thickness-normal edge-pattern-solid flowchart-link")
}

fn render_flowchart_edge_path(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
) {
    let Some(le) = ctx.layout_edges_by_id.get(&edge.id) else {
        return;
    };
    if le.points.len() < 2 {
        return;
    }

    let mut local_points: Vec<crate::model::LayoutPoint> = Vec::new();
    for p in &le.points {
        local_points.push(crate::model::LayoutPoint {
            x: p.x + ctx.tx - origin_x,
            y: p.y + ctx.ty - origin_y,
        });
    }

    #[derive(Debug, Clone, Copy)]
    struct BoundaryNode {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }

    fn outside_node(node: &BoundaryNode, point: &crate::model::LayoutPoint) -> bool {
        let dx = (point.x - node.x).abs();
        let dy = (point.y - node.y).abs();
        let w = node.width / 2.0;
        let h = node.height / 2.0;
        dx >= w || dy >= h
    }

    fn rect_intersection(
        node: &BoundaryNode,
        outside_point: &crate::model::LayoutPoint,
        inside_point: &crate::model::LayoutPoint,
    ) -> crate::model::LayoutPoint {
        let x = node.x;
        let y = node.y;

        let w = node.width / 2.0;
        let h = node.height / 2.0;

        let q_abs = (outside_point.y - inside_point.y).abs();
        let r_abs = (outside_point.x - inside_point.x).abs();

        if (y - outside_point.y).abs() * w > (x - outside_point.x).abs() * h {
            let q = if inside_point.y < outside_point.y {
                outside_point.y - h - y
            } else {
                y - h - outside_point.y
            };
            let r = if q_abs == 0.0 {
                0.0
            } else {
                (r_abs * q) / q_abs
            };
            let mut res = crate::model::LayoutPoint {
                x: if inside_point.x < outside_point.x {
                    inside_point.x + r
                } else {
                    inside_point.x - r_abs + r
                },
                y: if inside_point.y < outside_point.y {
                    inside_point.y + q_abs - q
                } else {
                    inside_point.y - q_abs + q
                },
            };

            if r == 0.0 {
                res.x = outside_point.x;
                res.y = outside_point.y;
            }
            if r_abs == 0.0 {
                res.x = outside_point.x;
            }
            if q_abs == 0.0 {
                res.y = outside_point.y;
            }
            return res;
        }

        let r = if inside_point.x < outside_point.x {
            outside_point.x - w - x
        } else {
            x - w - outside_point.x
        };
        let q = if r_abs == 0.0 {
            0.0
        } else {
            (q_abs * r) / r_abs
        };
        let mut ix = if inside_point.x < outside_point.x {
            inside_point.x + r_abs - r
        } else {
            inside_point.x - r_abs + r
        };
        let mut iy = if inside_point.y < outside_point.y {
            inside_point.y + q
        } else {
            inside_point.y - q
        };

        if r == 0.0 {
            ix = outside_point.x;
            iy = outside_point.y;
        }
        if r_abs == 0.0 {
            ix = outside_point.x;
        }
        if q_abs == 0.0 {
            iy = outside_point.y;
        }

        crate::model::LayoutPoint { x: ix, y: iy }
    }

    fn cut_path_at_intersect(
        input: &[crate::model::LayoutPoint],
        boundary: &BoundaryNode,
    ) -> Vec<crate::model::LayoutPoint> {
        if input.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<crate::model::LayoutPoint> = Vec::new();
        let mut last_point_outside = input[0].clone();
        let mut is_inside = false;

        for point in input {
            if !outside_node(boundary, point) && !is_inside {
                let inter = rect_intersection(boundary, &last_point_outside, point);
                if !out.iter().any(|p| p.x == inter.x && p.y == inter.y) {
                    out.push(inter);
                }
                is_inside = true;
            } else {
                last_point_outside = point.clone();
                if !is_inside {
                    out.push(point.clone());
                }
            }
        }
        out
    }

    fn dedup_consecutive_points(
        input: &[crate::model::LayoutPoint],
    ) -> Vec<crate::model::LayoutPoint> {
        if input.len() <= 1 {
            return input.to_vec();
        }
        const EPS: f64 = 1e-9;
        let mut out: Vec<crate::model::LayoutPoint> = Vec::with_capacity(input.len());
        for p in input {
            if out
                .last()
                .is_some_and(|prev| (prev.x - p.x).abs() <= EPS && (prev.y - p.y).abs() <= EPS)
            {
                continue;
            }
            out.push(p.clone());
        }
        out
    }

    fn boundary_for_cluster(
        ctx: &FlowchartRenderCtx<'_>,
        cluster_id: &str,
        origin_x: f64,
        origin_y: f64,
    ) -> Option<BoundaryNode> {
        let n = ctx.layout_clusters_by_id.get(cluster_id)?;
        Some(BoundaryNode {
            x: n.x + ctx.tx - origin_x,
            y: n.y + ctx.ty - origin_y,
            width: n.width,
            height: n.height,
        })
    }

    let is_cyclic_special = edge.id.contains("-cyclic-special-");
    let local_points = dedup_consecutive_points(&local_points);
    let mut points_for_render = local_points.clone();
    if let Some(tc) = le.to_cluster.as_deref() {
        if let Some(boundary) = boundary_for_cluster(ctx, tc, origin_x, origin_y) {
            points_for_render = cut_path_at_intersect(&points_for_render, &boundary);
        }
    }
    if let Some(fc) = le.from_cluster.as_deref() {
        if let Some(boundary) = boundary_for_cluster(ctx, fc, origin_x, origin_y) {
            let mut rev = points_for_render.clone();
            rev.reverse();
            rev = cut_path_at_intersect(&rev, &boundary);
            rev.reverse();
            points_for_render = rev;
        }
    }

    let interpolate = edge
        .interpolate
        .as_deref()
        .unwrap_or(ctx.default_edge_interpolate.as_str());
    let is_basis = !matches!(interpolate, "linear" | "stepAfter" | "cardinal");
    let is_cluster_edge = le.to_cluster.is_some() || le.from_cluster.is_some();

    fn all_triples_collinear(input: &[crate::model::LayoutPoint]) -> bool {
        if input.len() <= 2 {
            return true;
        }
        const EPS: f64 = 1e-9;
        for i in 1..input.len().saturating_sub(1) {
            let a = &input[i - 1];
            let b = &input[i];
            let c = &input[i + 1];
            let abx = b.x - a.x;
            let aby = b.y - a.y;
            let bcx = c.x - b.x;
            let bcy = c.y - b.y;
            if (abx * bcy - aby * bcx).abs() > EPS {
                return false;
            }
        }
        true
    }

    // Mermaid sometimes routes non-cluster edges with a 3-point basis polyline even when our
    // layout produces a 4-point path where the first run is perfectly horizontal. Collapse the
    // redundant middle point so the `curveBasis` command sequence matches (2 `C` segments instead
    // of 3).
    if is_basis && !is_cluster_edge && !is_cyclic_special && points_for_render.len() == 4 {
        const EPS: f64 = 1e-9;
        let p0 = &points_for_render[0];
        let p1 = &points_for_render[1];
        let p2 = &points_for_render[2];
        if (p0.y - p1.y).abs() <= EPS && (p1.y - p2.y).abs() <= EPS {
            points_for_render.remove(1);
        }
    }

    // Mermaid's cluster-edge clipping can reduce the routed polyline to 3 points (and therefore
    // `curveBasis` emits 2 `C` segments). When our route retains an extra collinear point, the SVG
    // command sequence diverges (extra `C`).
    if is_basis && is_cluster_edge && points_for_render.len() == 4 {
        const EPS: f64 = 1e-9;
        fn collinear(
            a: &crate::model::LayoutPoint,
            b: &crate::model::LayoutPoint,
            c: &crate::model::LayoutPoint,
        ) -> bool {
            let abx = b.x - a.x;
            let aby = b.y - a.y;
            let bcx = c.x - b.x;
            let bcy = c.y - b.y;
            (abx * bcy - aby * bcx).abs() <= EPS
        }

        let p0 = &points_for_render[0];
        let p1 = &points_for_render[1];
        let p2 = &points_for_render[2];
        let p3 = &points_for_render[3];
        let c012 = collinear(p0, p1, p2);
        let c123 = collinear(p1, p2, p3);
        // Mermaid keeps a 4-point straight route (3 `C` segments) for some cluster-adjacent edges,
        // so only collapse the variant where exactly one end contains a redundant collinear point.
        if c123 && !c012 {
            points_for_render.remove(2);
        } else if c012 && !c123 {
            points_for_render.remove(1);
        }
    }

    if is_basis && is_cluster_edge && points_for_render.len() > 4 {
        let n = points_for_render.len();
        let collinear = all_triples_collinear(&points_for_render)
            || (n > 4
                && (all_triples_collinear(&points_for_render[1..])
                    || all_triples_collinear(&points_for_render[..n.saturating_sub(1)])));
        if collinear {
            points_for_render = vec![
                points_for_render[0].clone(),
                points_for_render[points_for_render.len() / 2].clone(),
                points_for_render[points_for_render.len() - 1].clone(),
            ];
        }
    }

    if is_basis && is_cluster_edge && points_for_render.len() == 8 {
        const EPS: f64 = 1e-9;
        let len = points_for_render.len();
        let mut best_run: Option<(usize, usize)> = None;

        // Find the longest axis-aligned run (same x or same y) of consecutive points.
        for axis in 0..2 {
            let mut i = 0usize;
            while i + 1 < len {
                let base = if axis == 0 {
                    points_for_render[i].x
                } else {
                    points_for_render[i].y
                };
                if (if axis == 0 {
                    points_for_render[i + 1].x
                } else {
                    points_for_render[i + 1].y
                } - base)
                    .abs()
                    > EPS
                {
                    i += 1;
                    continue;
                }

                let start = i;
                while i + 1 < len {
                    let v = if axis == 0 {
                        points_for_render[i + 1].x
                    } else {
                        points_for_render[i + 1].y
                    };
                    if (v - base).abs() > EPS {
                        break;
                    }
                    i += 1;
                }
                let end = i;
                if end + 1 - start >= 6 {
                    best_run = match best_run {
                        Some((bs, be)) if (be + 1 - bs) >= (end + 1 - start) => Some((bs, be)),
                        _ => Some((start, end)),
                    };
                }
                i += 1;
            }
        }

        if let Some((start, end)) = best_run {
            let idx = end.saturating_sub(1);
            if idx > start && idx > 0 && idx + 1 < len {
                points_for_render.remove(idx);
            }
        }
    }

    if is_basis
        && is_cyclic_special
        && edge.id.contains("-cyclic-special-mid")
        && points_for_render.len() > 3
    {
        points_for_render = vec![
            points_for_render[0].clone(),
            points_for_render[points_for_render.len() / 2].clone(),
            points_for_render[points_for_render.len() - 1].clone(),
        ];
    }

    // D3's `curveBasis` emits only a straight `M ... L ...` when there are exactly two points.
    // Mermaid's Dagre pipeline typically provides at least one intermediate point even for
    // straight-looking edges, resulting in `C` segments in the SVG `d`. To keep our output closer
    // to Mermaid's command sequence, re-insert a midpoint when our route collapses to two points
    // after normalization.
    if points_for_render.len() == 1 {
        // Avoid emitting a degenerate `M x,y` path for clipped cluster-adjacent edges.
        points_for_render = local_points.clone();
    }

    if points_for_render.len() == 2
        && interpolate != "linear"
        && (!is_cyclic_special || le.to_cluster.is_some() || le.from_cluster.is_some())
    {
        let a = &points_for_render[0];
        let b = &points_for_render[1];
        points_for_render.insert(
            1,
            crate::model::LayoutPoint {
                x: (a.x + b.x) / 2.0,
                y: (a.y + b.y) / 2.0,
            },
        );
    }

    // Mermaid's cyclic self-loop helper edges (`*-cyclic-special-{1,2}`) sometimes use longer
    // routed point lists. When our layout collapses these helper edges to a short polyline, D3's
    // `basis` interpolation produces fewer cubic segments than Mermaid (`C` command count
    // mismatch in SVG `d`).
    //
    // Mermaid's behavior differs depending on whether the base node is a cluster and on the
    // cluster's effective direction. Recreate the command sequence by padding the polyline to at
    // least 5 points (so `curveBasis` emits 4 `C` segments) only for the variants that Mermaid
    // expands.
    if is_basis && is_cyclic_special {
        fn ensure_min_points(points: &mut Vec<crate::model::LayoutPoint>, min_len: usize) {
            if points.len() >= min_len || points.len() < 2 {
                return;
            }
            while points.len() < min_len {
                let mut best_i = 0usize;
                let mut best_d2 = -1.0f64;
                for i in 0..points.len().saturating_sub(1) {
                    let a = &points[i];
                    let b = &points[i + 1];
                    let dx = b.x - a.x;
                    let dy = b.y - a.y;
                    let d2 = dx * dx + dy * dy;
                    if d2 > best_d2 {
                        best_d2 = d2;
                        best_i = i;
                    }
                }
                let a = points[best_i].clone();
                let b = points[best_i + 1].clone();
                points.insert(
                    best_i + 1,
                    crate::model::LayoutPoint {
                        x: (a.x + b.x) / 2.0,
                        y: (a.y + b.y) / 2.0,
                    },
                );
            }
        }

        let cyclic_variant = if edge.id.ends_with("-cyclic-special-1") {
            Some(1u8)
        } else if edge.id.ends_with("-cyclic-special-2") {
            Some(2u8)
        } else {
            None
        };

        if let Some(variant) = cyclic_variant {
            let base_id = edge
                .id
                .split("-cyclic-special-")
                .next()
                .unwrap_or(edge.id.as_str());

            let should_expand = match ctx.layout_clusters_by_id.get(base_id) {
                Some(cluster) if cluster.effective_dir == "TB" || cluster.effective_dir == "TD" => {
                    variant == 1
                }
                Some(_) => variant == 2,
                None => variant == 2,
            };

            if should_expand {
                ensure_min_points(&mut points_for_render, 5);
            } else if points_for_render.len() == 4 {
                // For non-expanded cyclic helper edges, Mermaid's command sequence matches the
                // 3-point `curveBasis` case (`C` count = 2). Avoid emitting the intermediate
                // 4-point variant (`C` count = 3).
                points_for_render.remove(1);
            }
        }
    }

    let line_data: Vec<crate::model::LayoutPoint> = points_for_render
        .iter()
        .filter(|p| !p.y.is_nan())
        .cloned()
        .collect();
    let d = match interpolate {
        "linear" => curve_linear_path_d(&line_data),
        "stepAfter" => curve_step_after_path_d(&line_data),
        "cardinal" => curve_cardinal_path_d(&line_data, 0.0),
        // Mermaid defaults to `basis` for flowchart edges.
        _ => curve_basis_path_d(&line_data),
    };

    let points_json = serde_json::to_string(&local_points).unwrap_or_else(|_| "[]".to_string());
    let points_b64 = base64::engine::general_purpose::STANDARD.encode(points_json);

    let class_attr = flowchart_edge_class_attr(edge);
    let marker_start = flowchart_edge_marker_start(&ctx.diagram_id, edge);
    let marker_end = flowchart_edge_marker_end(&ctx.diagram_id, edge);

    let marker_start_attr = marker_start
        .as_deref()
        .map(|m| format!(r#" marker-start="{}""#, escape_attr(m)))
        .unwrap_or_default();
    let marker_end_attr = marker_end
        .as_deref()
        .map(|m| format!(r#" marker-end="{}""#, escape_attr(m)))
        .unwrap_or_default();

    let _ = write!(
        out,
        r#"<path d="{}" id="{}" class="{}" style=";" data-edge="true" data-et="edge" data-id="{}" data-points="{}"{}{} />"#,
        d,
        escape_attr(&edge.id),
        escape_attr(&class_attr),
        escape_attr(&edge.id),
        escape_attr(&points_b64),
        marker_start_attr,
        marker_end_attr
    );
}

fn render_flowchart_edge_label(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
) {
    let label_text = edge.label.as_deref().unwrap_or_default();
    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let label_text_plain = flowchart_label_plain_text(label_text, label_type);

    fn fallback_midpoint(
        le: &crate::model::LayoutEdge,
        ctx: &FlowchartRenderCtx<'_>,
        origin_x: f64,
        origin_y: f64,
    ) -> (f64, f64) {
        let Some(p) = le.points.get(le.points.len() / 2) else {
            return (ctx.tx - origin_x, ctx.ty - origin_y);
        };
        (p.x + ctx.tx - origin_x, p.y + ctx.ty - origin_y)
    }

    if !ctx.html_labels {
        if let Some(le) = ctx.layout_edges_by_id.get(&edge.id) {
            if let Some(lbl) = le.label.as_ref() {
                if !label_text_plain.trim().is_empty() {
                    let x = lbl.x + ctx.tx - origin_x;
                    let y = lbl.y + ctx.ty - origin_y;
                    let _ = write!(
                        out,
                        r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate(0, 0)">"#,
                        fmt(x),
                        fmt(y),
                        escape_attr(&edge.id)
                    );
                    write_flowchart_svg_text(out, &label_text_plain, false);
                    out.push_str("</g></g>");
                    return;
                }
            }

            if !label_text_plain.trim().is_empty() {
                let (x, y) = fallback_midpoint(le, ctx, origin_x, origin_y);
                let _ = write!(
                    out,
                    r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate(0, 0)">"#,
                    fmt(x),
                    fmt(y),
                    escape_attr(&edge.id)
                );
                write_flowchart_svg_text(out, &label_text_plain, false);
                out.push_str("</g></g>");
                return;
            }
        }

        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)">"#,
            escape_attr(&edge.id)
        );
        write_flowchart_svg_text(out, "", false);
        out.push_str("</g></g>");
        return;
    }

    let label_html = if label_text.trim().is_empty() {
        String::new()
    } else {
        flowchart_label_html(label_text, label_type, &ctx.config)
    };

    if let Some(le) = ctx.layout_edges_by_id.get(&edge.id) {
        if let Some(lbl) = le.label.as_ref() {
            let x = lbl.x + ctx.tx - origin_x;
            let y = lbl.y + ctx.ty - origin_y;
            let w = lbl.width.max(0.0);
            let h = lbl.height.max(0.0);
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
                fmt(x),
                fmt(y),
                escape_attr(&edge.id),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w),
                fmt(h),
                label_html
            );
            return;
        }

        if !label_text_plain.trim().is_empty() {
            let (x, y) = fallback_midpoint(le, ctx, origin_x, origin_y);
            let metrics = ctx.measurer.measure_wrapped(
                &label_text_plain,
                &ctx.text_style,
                Some(ctx.wrapping_width),
                ctx.wrap_mode,
            );
            let w = metrics.width.max(1.0);
            let h = metrics.height.max(1.0);
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
                fmt(x),
                fmt(y),
                escape_attr(&edge.id),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w.max(0.0)),
                fmt(h.max(0.0)),
                label_html
            );
            return;
        }
    }

    let _ = write!(
        out,
        r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
        escape_attr(&edge.id)
    );
}

fn flowchart_inline_style_for_classes(
    class_defs: &std::collections::HashMap<String, Vec<String>>,
    classes: &[String],
) -> String {
    let mut out = String::new();
    for c in classes {
        let Some(decls) = class_defs.get(c) else {
            continue;
        };
        for d in decls {
            let Some((k, v)) = parse_style_decl(d) else {
                continue;
            };
            let _ = write!(&mut out, "{k}:{v} !important;");
        }
    }
    out.trim_end_matches(';').to_string()
}

fn render_flowchart_node(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
) {
    let Some(layout_node) = ctx.layout_nodes_by_id.get(node_id) else {
        return;
    };

    let x = layout_node.x + ctx.tx - origin_x;
    let y = layout_node.y + ctx.ty - origin_y;

    fn is_self_loop_label_node_id(id: &str) -> bool {
        let mut parts = id.split("---");
        let Some(a) = parts.next() else {
            return false;
        };
        let Some(b) = parts.next() else {
            return false;
        };
        let Some(n) = parts.next() else {
            return false;
        };
        parts.next().is_none() && a == b && (n == "1" || n == "2")
    }

    if is_self_loop_label_node_id(node_id) {
        let _ = write!(
            out,
            r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#,
            escape_attr(node_id),
            fmt(x),
            fmt(y)
        );
        return;
    }

    let Some(node) = ctx.nodes_by_id.get(node_id) else {
        return;
    };

    let dom_idx = ctx.node_dom_index.get(node_id).copied().unwrap_or(0);

    let mut class_attr = "node default".to_string();
    for c in &node.classes {
        if !c.trim().is_empty() {
            class_attr.push(' ');
            class_attr.push_str(c.trim());
        }
    }

    let tooltip = ctx.tooltips.get(node_id).map(|s| s.as_str()).unwrap_or("");
    let tooltip_attr = if tooltip.trim().is_empty() {
        String::new()
    } else {
        format!(r#" title="{}""#, escape_attr(tooltip))
    };

    let is_clickable = node
        .classes
        .iter()
        .any(|c| c.trim().eq_ignore_ascii_case("clickable"));
    let href = node.link.as_deref().and_then(|u| {
        let u = u.trim();
        if u.starts_with("http://") || u.starts_with("https://") {
            Some(u)
        } else {
            None
        }
    });
    let wrapped_in_a = is_clickable || node.have_callback || node.link.is_some();

    if wrapped_in_a {
        if let Some(href) = href {
            let _ = write!(
                out,
                r#"<a xlink:href="{}" transform="translate({}, {})">"#,
                escape_attr(href),
                fmt(x),
                fmt(y)
            );
        } else {
            let _ = write!(out, r#"<a transform="translate({}, {})">"#, fmt(x), fmt(y));
        }
        let _ = write!(
            out,
            r#"<g class="{}" id="flowchart-{}-{}"{}>"#,
            escape_attr(&class_attr),
            escape_attr(node_id),
            dom_idx,
            tooltip_attr
        );
    } else {
        let _ = write!(
            out,
            r#"<g class="{}" id="flowchart-{}-{}" transform="translate({}, {})"{}>"#,
            escape_attr(&class_attr),
            escape_attr(node_id),
            dom_idx,
            fmt(x),
            fmt(y),
            tooltip_attr
        );
    }

    let shape = node.layout_shape.as_deref().unwrap_or("squareRect");
    let style = flowchart_inline_style_for_classes(&ctx.class_defs, &node.classes);
    let mut label_dx: f64 = 0.0;

    fn parse_hex_color_to_srgba(s: &str) -> Option<roughr::Srgba> {
        let s = s.trim();
        let hex = s.strip_prefix('#')?;
        let (r, g, b) = match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r, g, b)
            }
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                (r, g, b)
            }
            _ => return None,
        };
        Some(roughr::Srgba::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            1.0,
        ))
    }

    fn path_from_points(points: &[(f64, f64)]) -> String {
        let mut out = String::new();
        for (i, (x, y)) in points.iter().copied().enumerate() {
            let cmd = if i == 0 { 'M' } else { 'L' };
            let _ = write!(&mut out, "{cmd}{x},{y} ");
        }
        out.push_str("Z");
        out
    }

    fn arc_points(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        rx: f64,
        ry: f64,
        clockwise: bool,
    ) -> Vec<(f64, f64)> {
        // Port of Mermaid `@11.12.2` `generateArcPoints(...)` in
        // `packages/mermaid/src/rendering-util/rendering-elements/shapes/roundedRect.ts`.
        let num_points: usize = 20;

        let mid_x = (x1 + x2) / 2.0;
        let mid_y = (y1 + y2) / 2.0;
        let angle = (y2 - y1).atan2(x2 - x1);

        let dx = (x2 - x1) / 2.0;
        let dy = (y2 - y1) / 2.0;
        let transformed_x = dx / rx;
        let transformed_y = dy / ry;
        let distance = (transformed_x * transformed_x + transformed_y * transformed_y).sqrt();
        if distance > 1.0 {
            return vec![(x1, y1), (x2, y2)];
        }

        let scaled_center_distance = (1.0 - distance * distance).sqrt();
        let sign = if clockwise { -1.0 } else { 1.0 };
        let center_x = mid_x + scaled_center_distance * ry * angle.sin() * sign;
        let center_y = mid_y - scaled_center_distance * rx * angle.cos() * sign;

        let start_angle = ((y1 - center_y) / ry).atan2((x1 - center_x) / rx);
        let end_angle = ((y2 - center_y) / ry).atan2((x2 - center_x) / rx);

        let mut angle_range = end_angle - start_angle;
        if clockwise && angle_range < 0.0 {
            angle_range += 2.0 * std::f64::consts::PI;
        }
        if !clockwise && angle_range > 0.0 {
            angle_range -= 2.0 * std::f64::consts::PI;
        }

        let mut points: Vec<(f64, f64)> = Vec::with_capacity(num_points);
        for i in 0..num_points {
            let t = i as f64 / (num_points - 1) as f64;
            let a = start_angle + t * angle_range;
            let x = center_x + rx * a.cos();
            let y = center_y + ry * a.sin();
            points.push((x, y));
        }
        points
    }

    fn roughjs_paths_for_svg_path(
        svg_path_data: &str,
        fill: &str,
        stroke: &str,
    ) -> Option<(String, String, f32)> {
        let fill = parse_hex_color_to_srgba(fill)?;
        let stroke = parse_hex_color_to_srgba(stroke)?;
        let base_options = roughr::core::OptionsBuilder::default()
            .seed(0_u64)
            .roughness(0.0)
            .bowing(1.0)
            .fill(fill)
            .fill_style(roughr::core::FillStyle::Solid)
            .stroke(stroke)
            .stroke_width(1.3)
            .stroke_line_dash(vec![0.0, 0.0])
            .stroke_line_dash_offset(0.0)
            .fill_line_dash(vec![0.0, 0.0])
            .fill_line_dash_offset(0.0)
            .disable_multi_stroke(false)
            .disable_multi_stroke_fill(false)
            .build()
            .ok()?;

        fn ops_to_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
            let mut out = String::new();
            for op in &opset.ops {
                match op.op {
                    roughr::core::OpType::Move => {
                        let _ = write!(&mut out, "M{} {} ", fmt(op.data[0]), fmt(op.data[1]));
                    }
                    roughr::core::OpType::BCurveTo => {
                        let _ = write!(
                            &mut out,
                            "C{} {}, {} {}, {} {} ",
                            fmt(op.data[0]),
                            fmt(op.data[1]),
                            fmt(op.data[2]),
                            fmt(op.data[3]),
                            fmt(op.data[4]),
                            fmt(op.data[5])
                        );
                    }
                    roughr::core::OpType::LineTo => {
                        let _ = write!(&mut out, "L{} {} ", fmt(op.data[0]), fmt(op.data[1]));
                    }
                }
            }
            out.trim_end().to_string()
        }

        // Rough.js `generator.path(...)` has a special-case for solid fill:
        // if `pointsOnPath` yields a single point-set, it builds the fill path from `svgPath`
        // with `disableMultiStroke: true` and then removes subsequent `move` ops.
        let sets = roughr::points_on_path::points_on_path::<f64>(
            svg_path_data.to_string(),
            Some(1.0),
            Some(0.5),
        );

        let mut fill_opts = base_options.clone();
        fill_opts.disable_multi_stroke = Some(true);

        let fill_opset = if sets.len() == 1 {
            let mut opset =
                roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut fill_opts);
            let mut merged: Vec<roughr::core::Op<f64>> = Vec::new();
            for (idx, op) in opset.ops.iter().cloned().enumerate() {
                if idx != 0 && op.op == roughr::core::OpType::Move {
                    continue;
                }
                merged.push(op);
            }
            opset.ops = merged;
            opset
        } else {
            roughr::renderer::solid_fill_polygon(&sets, &mut fill_opts)
        };

        let mut stroke_opts = base_options.clone();
        let mut stroke_opset =
            roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut stroke_opts);
        // Roughr emits a `Move` op for SVG `M` segments, while Rough.js treats `M` as cursor state
        // and does not emit an output op. Drop those by removing `Move` ops that are immediately
        // followed by another `Move` (the real stroke for a segment starts with its own `Move`).
        stroke_opset.ops = stroke_opset
            .ops
            .iter()
            .cloned()
            .enumerate()
            .filter_map(|(idx, op)| {
                if op.op == roughr::core::OpType::Move {
                    if let Some(next) = stroke_opset.ops.get(idx + 1) {
                        if next.op == roughr::core::OpType::Move {
                            return None;
                        }
                    }
                }
                Some(op)
            })
            .collect();

        Some((
            ops_to_svg_path_d(&fill_opset),
            ops_to_svg_path_d(&stroke_opset),
            1.3,
        ))
    }

    match shape {
        "diamond" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let _ = write!(
                out,
                r#"<polygon points="{},0 {},{} {},{} 0,{}" class="label-container" transform="translate({}, {})"{} />"#,
                fmt(w / 2.0),
                fmt(w),
                fmt(-h / 2.0),
                fmt(w / 2.0),
                fmt(-h),
                fmt(-h / 2.0),
                fmt(-w / 2.0 + 0.5),
                fmt(h / 2.0),
                if style.is_empty() {
                    String::new()
                } else {
                    format!(r#" style="{}""#, escape_attr(&style))
                }
            );
        }
        "circle" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let r = (w.min(h) / 2.0).max(0.5);
            let _ = write!(
                out,
                r#"<circle class="basic label-container" style="{}" r="{}" cx="0" cy="0"/>"#,
                escape_attr(&style),
                fmt(r),
            );
        }
        "doublecircle" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let r = (w.min(h) / 2.0).max(0.5);
            let inner = (r - 5.0).max(0.5);
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}"><circle class="outer-circle" style="" r="{}" cx="0" cy="0"/><circle class="inner-circle" style="" r="{}" cx="0" cy="0"/></g>"#,
                escape_attr(&style),
                fmt(r),
                fmt(inner),
            );
        }
        "roundedRect" | "rounded" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let radius = 5.0;
            let taper = 5.0;

            let mut pts: Vec<(f64, f64)> = Vec::new();
            pts.push((-w / 2.0 + taper, -h / 2.0));
            pts.push((w / 2.0 - taper, -h / 2.0));
            pts.extend(arc_points(
                w / 2.0 - taper,
                -h / 2.0,
                w / 2.0,
                -h / 2.0 + taper,
                radius,
                radius,
                true,
            ));
            pts.push((w / 2.0, -h / 2.0 + taper));
            pts.push((w / 2.0, h / 2.0 - taper));
            pts.extend(arc_points(
                w / 2.0,
                h / 2.0 - taper,
                w / 2.0 - taper,
                h / 2.0,
                radius,
                radius,
                true,
            ));
            pts.push((w / 2.0 - taper, h / 2.0));
            pts.push((-w / 2.0 + taper, h / 2.0));
            pts.extend(arc_points(
                -w / 2.0 + taper,
                h / 2.0,
                -w / 2.0,
                h / 2.0 - taper,
                radius,
                radius,
                true,
            ));
            pts.push((-w / 2.0, h / 2.0 - taper));
            pts.push((-w / 2.0, -h / 2.0 + taper));
            pts.extend(arc_points(
                -w / 2.0,
                -h / 2.0 + taper,
                -w / 2.0 + taper,
                -h / 2.0,
                radius,
                radius,
                true,
            ));
            let path_data = path_from_points(&pts);

            if let Some((fill_d, stroke_d, stroke_w)) =
                roughjs_paths_for_svg_path(&path_data, &ctx.node_fill_color, &ctx.node_border_color)
            {
                out.push_str(r#"<g class="basic label-container outer-path">"#);
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/>"#,
                    escape_attr(&fill_d),
                    escape_attr(&ctx.node_fill_color)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style=""/>"#,
                    escape_attr(&stroke_d),
                    escape_attr(&ctx.node_border_color),
                    stroke_w
                );
                out.push_str("</g>");
            } else {
                let _ = write!(
                    out,
                    r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}" rx="5" ry="5"/>"#,
                    escape_attr(&style),
                    fmt(-w / 2.0),
                    fmt(-h / 2.0),
                    fmt(w),
                    fmt(h)
                );
            }
        }
        "hexagon" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let half_width = w / 2.0;
            let half_height = h / 2.0;
            let fixed_length = half_height / 2.0;
            let deduced_width = half_width - fixed_length;

            let pts: Vec<(f64, f64)> = vec![
                (-deduced_width, -half_height),
                (0.0, -half_height),
                (deduced_width, -half_height),
                (half_width, 0.0),
                (deduced_width, half_height),
                (0.0, half_height),
                (-deduced_width, half_height),
                (-half_width, 0.0),
            ];
            let path_data = path_from_points(&pts);

            if let Some((fill_d, stroke_d, stroke_w)) =
                roughjs_paths_for_svg_path(&path_data, &ctx.node_fill_color, &ctx.node_border_color)
            {
                out.push_str(r#"<g class="basic label-container">"#);
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/>"#,
                    escape_attr(&fill_d),
                    escape_attr(&ctx.node_fill_color)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style=""/>"#,
                    escape_attr(&stroke_d),
                    escape_attr(&ctx.node_border_color),
                    stroke_w
                );
                out.push_str("</g>");
            } else {
                let _ = write!(
                    out,
                    r#"<polygon points="{},{} {},{} {},{} {},{} {},{} {},{} {},{} {},{}" class="label-container" transform="translate({}, {})"{} />"#,
                    fmt(-deduced_width),
                    fmt(-half_height),
                    fmt(0.0),
                    fmt(-half_height),
                    fmt(deduced_width),
                    fmt(-half_height),
                    fmt(half_width),
                    fmt(0.0),
                    fmt(deduced_width),
                    fmt(half_height),
                    fmt(0.0),
                    fmt(half_height),
                    fmt(-deduced_width),
                    fmt(half_height),
                    fmt(-half_width),
                    fmt(0.0),
                    fmt(0.0),
                    fmt(0.0),
                    if style.is_empty() {
                        String::new()
                    } else {
                        format!(r#" style="{}""#, escape_attr(&style))
                    }
                );
            }
        }
        "odd" => {
            let total_w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let w = (total_w - h / 4.0).max(1.0);
            let x = -w / 2.0;
            let y = -h / 2.0;
            let notch = y / 2.0;
            let dx = -notch / 2.0;
            label_dx = dx;

            let pts: Vec<(f64, f64)> =
                vec![(x + notch, y), (x, 0.0), (x + notch, -y), (-x, -y), (-x, y)];
            let path_data = path_from_points(&pts);

            if let Some((fill_d, stroke_d, stroke_w)) =
                roughjs_paths_for_svg_path(&path_data, &ctx.node_fill_color, &ctx.node_border_color)
            {
                let _ = write!(
                    out,
                    r#"<g class="basic label-container" transform="translate({},0)">"#,
                    fmt(dx)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/>"#,
                    escape_attr(&fill_d),
                    escape_attr(&ctx.node_fill_color)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style=""/>"#,
                    escape_attr(&stroke_d),
                    escape_attr(&ctx.node_border_color),
                    stroke_w
                );
                out.push_str("</g>");
            } else {
                let _ = write!(
                    out,
                    r#"<polygon points="{},{} {},{} {},{} {},{} {},{}" class="label-container" transform="translate({}, {})"{} />"#,
                    fmt(x + notch),
                    fmt(y),
                    fmt(x),
                    fmt(0.0),
                    fmt(x + notch),
                    fmt(-y),
                    fmt(-x),
                    fmt(-y),
                    fmt(-x),
                    fmt(y),
                    fmt(dx),
                    fmt(0.0),
                    if style.is_empty() {
                        String::new()
                    } else {
                        format!(r#" style="{}""#, escape_attr(&style))
                    }
                );
            }
        }
        _ => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let _ = write!(
                out,
                r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
                escape_attr(&style),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w),
                fmt(h)
            );
        }
    }

    let label_text = node.label.as_deref().unwrap_or(node_id);
    let metrics = ctx.measurer.measure_wrapped(
        label_text,
        &ctx.text_style,
        Some(ctx.wrapping_width),
        ctx.wrap_mode,
    );
    let label_type = node.label_type.as_deref().unwrap_or("text");
    if !ctx.html_labels {
        let label_text_plain = flowchart_label_plain_text(label_text, label_type);
        let _ = write!(
            out,
            r#"<g class="label" style="" transform="translate({}, {})"><rect/><g><rect class="background" style="stroke: none"/>"#,
            fmt(label_dx),
            fmt(-metrics.height / 2.0)
        );
        write_flowchart_svg_text(out, &label_text_plain, true);
        out.push_str("</g></g></g>");
    } else {
        let label_html = flowchart_label_html(label_text, label_type, &ctx.config);
        let _ = write!(
            out,
            r#"<g class="label" style="" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel">{}</span></div></foreignObject></g></g>"#,
            fmt(-metrics.width / 2.0 + label_dx),
            fmt(-metrics.height / 2.0),
            fmt(metrics.width),
            fmt(metrics.height),
            label_html
        );
    }
    if wrapped_in_a {
        out.push_str("</a>");
    }
}

fn flowchart_label_html(
    label: &str,
    label_type: &str,
    config: &merman_core::MermaidConfig,
) -> String {
    fn xhtml_fix_fragment(input: &str) -> String {
        // `foreignObject` content lives in an XML document, so:
        // - void tags must be self-closed (`<br />`, not `<br>`)
        // - stray `<` / `>` in text must be entity-escaped (`&lt;`, `&gt;`)
        //
        // Mermaid's SVG baselines follow these rules.
        let input = input
            .replace("<br>", "<br />")
            .replace("<br/>", "<br />")
            .replace("<br >", "<br />");

        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();
        let mut in_tag = false;

        while let Some(ch) = chars.next() {
            if in_tag {
                out.push(ch);
                if ch == '>' {
                    in_tag = false;
                }
                continue;
            }

            match ch {
                '<' => {
                    let next = chars.peek().copied();
                    if matches!(
                        next,
                        Some(n) if n.is_ascii_alphabetic() || matches!(n, '/' | '!' | '?')
                    ) {
                        in_tag = true;
                        out.push('<');
                    } else {
                        out.push_str("&lt;");
                    }
                }
                '>' => out.push_str("&gt;"),
                '&' => {
                    // Preserve entities already encoded by the sanitizer.
                    let mut tail = String::new();
                    let mut ok = false;
                    for _ in 0..32 {
                        match chars.peek().copied() {
                            Some(';') => {
                                chars.next();
                                tail.push(';');
                                ok = true;
                                break;
                            }
                            Some(c)
                                if c.is_ascii_alphanumeric() || matches!(c, '#' | 'x' | 'X') =>
                            {
                                chars.next();
                                tail.push(c);
                            }
                            _ => break,
                        }
                    }
                    if ok {
                        out.push('&');
                        out.push_str(&tail);
                    } else {
                        out.push_str("&amp;");
                        out.push_str(&tail);
                    }
                }
                _ => out.push(ch),
            }
        }

        out
    }

    match label_type {
        "markdown" => {
            let mut html_out = String::new();
            let parser = pulldown_cmark::Parser::new_ext(
                label,
                pulldown_cmark::Options::ENABLE_TABLES
                    | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                    | pulldown_cmark::Options::ENABLE_TASKLISTS,
            )
            .map(|ev| match ev {
                pulldown_cmark::Event::SoftBreak => pulldown_cmark::Event::HardBreak,
                other => other,
            });
            pulldown_cmark::html::push_html(&mut html_out, parser);
            let html_out = html_out.trim().to_string();
            xhtml_fix_fragment(&merman_core::sanitize::sanitize_text(&html_out, config))
        }
        _ => {
            let label = label.replace("\r\n", "\n");
            let label = label.trim_end_matches('\n').replace('\n', "<br />");
            let wrapped = format!("<p>{}</p>", label);
            xhtml_fix_fragment(&merman_core::sanitize::sanitize_text(&wrapped, config))
        }
    }
}

fn flowchart_label_plain_text(label: &str, label_type: &str) -> String {
    match label_type {
        "markdown" => {
            let mut out = String::new();
            let parser = pulldown_cmark::Parser::new_ext(
                label,
                pulldown_cmark::Options::ENABLE_TABLES
                    | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                    | pulldown_cmark::Options::ENABLE_TASKLISTS,
            );
            for ev in parser {
                match ev {
                    pulldown_cmark::Event::Text(t) => out.push_str(&t),
                    pulldown_cmark::Event::Code(t) => out.push_str(&t),
                    pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                        out.push('\n');
                    }
                    _ => {}
                }
            }
            out.trim().to_string()
        }
        _ => {
            let mut t = label.replace("\r\n", "\n");
            t = t.replace("<br />", "\n");
            t = t.replace("<br/>", "\n");
            t = t.replace("<br>", "\n");
            t.trim_end_matches('\n').to_string()
        }
    }
}

fn write_flowchart_svg_text(out: &mut String, text: &str, include_style: bool) {
    // Mirrors Mermaid's SVG text structure when `flowchart.htmlLabels=false`.
    if include_style {
        out.push_str(r#"<text y="-10.1" style="">"#);
    } else {
        out.push_str(r#"<text y="-10.1">"#);
    }

    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines(text);
    if lines.len() == 1 && lines[0].is_empty() {
        out.push_str(r#"<tspan class="text-outer-tspan" x="0" y="-0.1em" dy="1.1em"/>"#);
        out.push_str("</text>");
        return;
    }

    fn split_mermaid_escaped_tag_runs(line: &str) -> Option<Vec<String>> {
        // Mermaid’s SVG text renderer tokenizes a simple HTML-tag wrapper even when htmlLabels are
        // disabled, resulting in 3 inner <tspan> runs like:
        //   `<strong>Haiya</strong>` -> `<strong>` + ` Haiya` + ` </strong>`
        // (all still rendered as escaped text).
        let line = line.trim_end();
        if !line.starts_with('<') || !line.ends_with('>') {
            return None;
        }
        let open_end = line.find('>')?;
        let open_tag = &line[..=open_end];
        if open_tag.starts_with("</") {
            return None;
        }
        let open_inner = open_tag.trim_start_matches('<').trim_end_matches('>');
        let tag_name = open_inner
            .split_whitespace()
            .next()
            .filter(|s| !s.is_empty())?;
        let close_tag = format!("</{tag_name}>");
        if !line.ends_with(&close_tag) {
            return None;
        }
        let inner = &line[open_end + 1..line.len() - close_tag.len()];
        Some(vec![
            open_tag.to_string(),
            format!(" {inner}"),
            format!(" {close_tag}"),
        ])
    }

    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            out.push_str(r#"<tspan class="text-outer-tspan" x="0" y="-0.1em" dy="1.1em">"#);
        } else {
            // Mermaid sets an absolute `y` for each subsequent line, then uses `dy="1.1em"` as
            // the line-height increment. This yields `y="1em"` for the 2nd line and `y="2.1em"`
            // for the 3rd line, etc.
            let y_em = if idx == 1 {
                "1em".to_string()
            } else {
                format!("{:.1}em", 1.0 + (idx as f64 - 1.0) * 1.1)
            };
            let _ = write!(
                out,
                r#"<tspan class="text-outer-tspan" x="0" y="{}" dy="1.1em">"#,
                y_em
            );
        }
        let runs = split_mermaid_escaped_tag_runs(line).unwrap_or_else(|| vec![line.to_string()]);
        for run in &runs {
            out.push_str(
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">"#,
            );
            out.push_str(&escape_xml(run));
            out.push_str("</tspan>");
        }
        out.push_str("</tspan>");
    }

    out.push_str("</text>");
}
