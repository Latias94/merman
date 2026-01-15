use crate::model::{
    Bounds, ClassDiagramV2Layout, ErDiagramLayout, FlowchartV2Layout, LayoutCluster, LayoutNode,
    StateDiagramV2Layout,
};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use std::fmt::Write as _;

#[derive(Debug, Clone)]
pub struct SvgRenderOptions {
    /// Adds extra space around the computed viewBox.
    pub viewbox_padding: f64,
    /// Optional diagram id used for Mermaid-like marker ids.
    pub diagram_id: Option<String>,
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

fn theme_color(effective_config: &serde_json::Value, key: &str, fallback: &str) -> String {
    config_string(effective_config, &["themeVariables", key])
        .unwrap_or_else(|| fallback.to_string())
}

fn split_br_like_mermaid(text: &str) -> Vec<String> {
    let t = text
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n");
    t.split('\n').map(|s| s.to_string()).collect()
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
    classes: &std::collections::HashMap<String, crate::er::ErClassDef>,
) -> (Vec<String>, Vec<String>) {
    let mut compiled_box: Vec<String> = Vec::new();
    let mut compiled_text: Vec<String> = Vec::new();
    for class_name in entity.css_classes.split_whitespace() {
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

    let mut rect_decls: Vec<String> = Vec::new();
    let mut text_decls: Vec<String> = Vec::new();

    // Box styles: classDef styles + `style` statements.
    for s in compiled_box.iter().chain(entity.css_styles.iter()) {
        let Some((k, v)) = parse_style_decl(s) else {
            continue;
        };
        if is_rect_style_key(k) {
            rect_decls.push(format!("{k}:{v}"));
        }
        // Mermaid treats `color:` as the text color (even if it comes from the style list).
        if k == "color" {
            text_decls.push(format!("fill:{v}"));
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
            text_decls.push(format!("fill:{v}"));
        } else {
            text_decls.push(format!("{k}:{v}"));
        }
    }

    (rect_decls, text_decls)
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
    let diagram_type = "erDiagram";

    let stroke = theme_color(effective_config, "lineColor", "#333333");
    let node_border = theme_color(effective_config, "nodeBorder", "#333333");
    let main_bkg = theme_color(effective_config, "mainBkg", "#ffffff");
    let tertiary = theme_color(effective_config, "tertiaryColor", "#e5e7eb");
    let text_color = theme_color(effective_config, "textColor", "#111827");
    let node_text_color = theme_color(effective_config, "nodeTextColor", &text_color);
    let font_family = config_string(effective_config, &["fontFamily"])
        .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
        .unwrap_or_else(|| "Arial, Helvetica, sans-serif".to_string());
    let font_size = effective_config
        .get("er")
        .and_then(|v| v.get("fontSize"))
        .and_then(|v| v.as_f64())
        .or_else(|| effective_config.get("fontSize").and_then(|v| v.as_f64()))
        .unwrap_or(12.0)
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
        font_size: (font_size * 0.85).max(1.0),
        font_weight: None,
    };

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
    let vb_min_x = content_bounds.min_x - pad;
    let vb_min_y = content_bounds.min_y - pad;
    let vb_w = (content_bounds.max_x - content_bounds.min_x) + pad * 2.0;
    let vb_h = (content_bounds.max_y - content_bounds.min_y) + pad * 2.0;

    let mut out = String::new();
    let w_attr = fmt(vb_w.max(1.0));
    let h_attr = fmt(vb_h.max(1.0));
    if use_max_width {
        let _ = writeln!(
            &mut out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" class="erDiagram" width="100%" style="max-width: {}px;" viewBox="{} {} {} {}">"#,
            w_attr,
            fmt(vb_min_x),
            fmt(vb_min_y),
            w_attr,
            h_attr
        );
    } else {
        let _ = writeln!(
            &mut out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" class="erDiagram" width="{}" height="{}" viewBox="{} {} {} {}">"#,
            w_attr,
            h_attr,
            fmt(vb_min_x),
            fmt(vb_min_y),
            w_attr,
            h_attr
        );
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
        node_border,
        escape_xml(&font_family),
        node_text_color,
        escape_xml(&font_family),
        node_text_color,
        escape_xml(&font_family),
        node_border,
        node_border
    );

    // Markers ported from Mermaid `@11.12.2` `erMarkers.js`.
    // Note: ids follow Mermaid unified renderer rules: `${diagramId}_${diagramType}-${markerType}{Start|End}`.
    let defs = format!(
        r##"<defs>
  <marker id="{diagram_id}_{diagram_type}-mdParentStart" refX="0" refY="7" markerWidth="190" markerHeight="240" orient="auto">
    <path d="M 18,7 L9,13 L1,7 L9,1 Z" fill="{stroke}" stroke="{stroke}" />
  </marker>
  <marker id="{diagram_id}_{diagram_type}-mdParentEnd" refX="19" refY="7" markerWidth="20" markerHeight="28" orient="auto">
    <path d="M 18,7 L9,13 L1,7 L9,1 Z" fill="{stroke}" stroke="{stroke}" />
  </marker>

  <marker id="{diagram_id}_{diagram_type}-onlyOneStart" refX="0" refY="9" markerWidth="18" markerHeight="18" orient="auto">
    <path stroke="{stroke}" fill="none" d="M9,0 L9,18 M15,0 L15,18" />
  </marker>
  <marker id="{diagram_id}_{diagram_type}-onlyOneEnd" refX="18" refY="9" markerWidth="18" markerHeight="18" orient="auto">
    <path stroke="{stroke}" fill="none" d="M3,0 L3,18 M9,0 L9,18" />
  </marker>

  <marker id="{diagram_id}_{diagram_type}-zeroOrOneStart" refX="0" refY="9" markerWidth="30" markerHeight="18" orient="auto">
    <circle stroke="{stroke}" fill="{main_bkg}" cx="21" cy="9" r="6" />
    <path stroke="{stroke}" fill="none" d="M9,0 L9,18" />
  </marker>
  <marker id="{diagram_id}_{diagram_type}-zeroOrOneEnd" refX="30" refY="9" markerWidth="30" markerHeight="18" orient="auto">
    <circle stroke="{stroke}" fill="{main_bkg}" cx="9" cy="9" r="6" />
    <path stroke="{stroke}" fill="none" d="M21,0 L21,18" />
  </marker>

  <marker id="{diagram_id}_{diagram_type}-oneOrMoreStart" refX="18" refY="18" markerWidth="45" markerHeight="36" orient="auto">
    <path stroke="{stroke}" fill="none" d="M0,18 Q 18,0 36,18 Q 18,36 0,18 M42,9 L42,27" />
  </marker>
  <marker id="{diagram_id}_{diagram_type}-oneOrMoreEnd" refX="27" refY="18" markerWidth="45" markerHeight="36" orient="auto">
    <path stroke="{stroke}" fill="none" d="M3,9 L3,27 M9,18 Q27,0 45,18 Q27,36 9,18" />
  </marker>

  <marker id="{diagram_id}_{diagram_type}-zeroOrMoreStart" refX="18" refY="18" markerWidth="57" markerHeight="36" orient="auto">
    <circle stroke="{stroke}" fill="{main_bkg}" cx="48" cy="18" r="6" />
    <path stroke="{stroke}" fill="none" d="M0,18 Q18,0 36,18 Q18,36 0,18" />
  </marker>
  <marker id="{diagram_id}_{diagram_type}-zeroOrMoreEnd" refX="39" refY="18" markerWidth="57" markerHeight="36" orient="auto">
    <circle stroke="{stroke}" fill="{main_bkg}" cx="9" cy="18" r="6" />
    <path stroke="{stroke}" fill="none" d="M21,18 Q39,0 57,18 Q39,36 21,18" />
  </marker>
</defs>"##,
        diagram_id = escape_xml(diagram_id),
        diagram_type = escape_xml(diagram_type),
        stroke = escape_xml(&stroke),
        main_bkg = escape_xml(&main_bkg)
    );
    out.push_str(&defs);
    out.push('\n');

    if let Some(title) = diagram_title {
        let _ = writeln!(
            &mut out,
            r#"<text class="erDiagramTitleText" x="{}" y="{}">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            escape_xml(title)
        );
    }

    let mut entity_by_id: std::collections::HashMap<&str, &crate::er::ErEntity> =
        std::collections::HashMap::new();
    for e in model.entities.values() {
        entity_by_id.insert(e.id.as_str(), e);
    }

    if options.include_edges {
        out.push_str(r#"<g class="relationships">"#);
        for e in &edges {
            if e.points.len() >= 2 {
                let mut line_classes = String::from("er relationshipLine");
                if e.stroke_dasharray.as_deref() == Some("8,8") {
                    line_classes.push_str(" edge-pattern-dashed");
                }
                let _ = write!(&mut out, r#"<path class="{}""#, escape_xml(&line_classes));
                if let Some(dash) = &e.stroke_dasharray {
                    let _ = write!(&mut out, r#" stroke-dasharray="{}""#, escape_xml(dash));
                }
                if let Some(m) = &e.start_marker {
                    let marker = er_unified_marker_id(diagram_id, diagram_type, m);
                    let _ = write!(&mut out, r#" marker-start="url(#{})""#, escape_xml(&marker));
                }
                if let Some(m) = &e.end_marker {
                    let marker = er_unified_marker_id(diagram_id, diagram_type, m);
                    let _ = write!(&mut out, r#" marker-end="url(#{})""#, escape_xml(&marker));
                }
                let d = curve_basis_path_d(&e.points);
                let _ = write!(&mut out, r#" d="{}" />"#, escape_xml(&d));
            }

            // Role label + opaque box.
            if let Some(lbl) = &e.label {
                let x = lbl.x - lbl.width / 2.0;
                let y = lbl.y - lbl.height / 2.0;
                let _ = write!(
                    &mut out,
                    r#"<rect class="er relationshipLabelBox" x="{}" y="{}" width="{}" height="{}" />"#,
                    fmt(x),
                    fmt(y),
                    fmt(lbl.width.max(1.0)),
                    fmt(lbl.height.max(1.0))
                );

                let rel_text =
                    e.id.strip_prefix("er-rel-")
                        .and_then(|s| s.parse::<usize>().ok())
                        .and_then(|idx| model.relationships.get(idx))
                        .map(|r| r.role_a.as_str())
                        .unwrap_or("");
                let lines = split_br_like_mermaid(rel_text);
                let _ = write!(
                    &mut out,
                    r#"<text class="er relationshipLabel" x="{}" y="{}" font-size="{}">"#,
                    fmt(lbl.x),
                    fmt(lbl.y),
                    fmt(font_size)
                );
                if lines.len() <= 1 {
                    out.push_str(&escape_xml(rel_text));
                } else {
                    let first_shift = -((lines.len() as f64 - 1.0) * 0.5);
                    for (i, line) in lines.iter().enumerate() {
                        let dy = if i == 0 { first_shift } else { 1.0 };
                        let _ = write!(
                            &mut out,
                            r#"<tspan x="{}" dy="{}em">{}</tspan>"#,
                            fmt(lbl.x),
                            fmt(dy),
                            escape_xml(line)
                        );
                    }
                }
                out.push_str("</text>");
            }
        }
        out.push_str("</g>\n");
    }

    // Entities drawn after relationships so they cover markers when overlapping.
    out.push_str(r#"<g class="entities">"#);
    for n in &nodes {
        let Some(entity) = entity_by_id.get(n.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("missing ER entity for node id {}", n.id),
            });
        };

        let (rect_style_decls, text_style_decls) = compile_er_entity_styles(entity, &model.classes);
        let rect_style_attr = if rect_style_decls.is_empty() {
            String::new()
        } else {
            format!(r#" style="{}""#, escape_xml(&rect_style_decls.join(";")))
        };
        let text_style_attr = if text_style_decls.is_empty() {
            String::new()
        } else {
            format!(r#" style="{}""#, escape_xml(&text_style_decls.join(";")))
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

        let tx = n.x - w / 2.0;
        let ty = n.y - h / 2.0;

        let group_class = if entity.css_classes.trim().is_empty() {
            "er".to_string()
        } else {
            format!("er {}", entity.css_classes.trim())
        };
        let _ = write!(
            &mut out,
            r#"<g id="{}" class="{}" transform="translate({}, {})">"#,
            escape_xml(&entity.id),
            escape_xml(&group_class),
            fmt(tx),
            fmt(ty)
        );

        let _ = write!(
            &mut out,
            r#"<rect class="er entityBox" x="0" y="0" width="{}" height="{}"{} />"#,
            fmt(w),
            fmt(h),
            rect_style_attr
        );

        if entity.attributes.is_empty() {
            let _ = write!(
                &mut out,
                r#"<text class="er entityLabel" x="{}" y="{}" font-size="{}"{}>{}</text>"#,
                fmt(w / 2.0),
                fmt(h / 2.0),
                fmt(font_size),
                text_style_attr,
                escape_xml(&measure.label_text)
            );
            out.push_str("</g>");
            continue;
        }

        // Title near top.
        let title_y = measure.height_padding + measure.label_height / 2.0;
        let _ = write!(
            &mut out,
            r#"<text class="er entityLabel" x="{}" y="{}" font-size="{}"{}>{}</text>"#,
            fmt(w / 2.0),
            fmt(title_y),
            fmt(font_size),
            text_style_attr,
            escape_xml(&measure.label_text)
        );

        let width_padding_factor = 4.0
            + if measure.has_key { 2.0 } else { 0.0 }
            + if measure.has_comment { 2.0 } else { 0.0 };
        let max_width =
            measure.max_type_w + measure.max_name_w + measure.max_key_w + measure.max_comment_w;
        let spare_column_width = ((w - max_width - measure.width_padding * width_padding_factor)
            / (width_padding_factor / 2.0))
            .max(0.0);

        let type_col_w = measure.max_type_w + measure.width_padding * 2.0 + spare_column_width;
        let name_col_w = measure.max_name_w + measure.width_padding * 2.0 + spare_column_width;
        let key_col_w = measure.max_key_w + measure.width_padding * 2.0 + spare_column_width;
        let comment_col_w =
            measure.max_comment_w + measure.width_padding * 2.0 + spare_column_width;

        let mut y_off = measure.label_height + measure.height_padding * 2.0;
        let mut odd = true;
        for row in &measure.rows {
            let align_y = y_off + measure.height_padding + row.height / 2.0;
            let row_h = row.height + measure.height_padding * 2.0;
            let row_class = if odd {
                "attributeBoxOdd"
            } else {
                "attributeBoxEven"
            };

            // type
            let _ = write!(
                &mut out,
                r#"<rect class="er {}" x="0" y="{}" width="{}" height="{}" />"#,
                row_class,
                fmt(y_off),
                fmt(type_col_w.max(1.0)),
                fmt(row_h.max(1.0))
            );
            let _ = write!(
                &mut out,
                r#"<text class="er attributeText" x="{}" y="{}" font-size="{}"{}>{}</text>"#,
                fmt(measure.width_padding),
                fmt(align_y),
                fmt(attr_style.font_size),
                text_style_attr,
                escape_xml(&row.type_text)
            );

            // name
            let name_x = type_col_w;
            let _ = write!(
                &mut out,
                r#"<rect class="er {}" x="{}" y="{}" width="{}" height="{}" />"#,
                row_class,
                fmt(name_x),
                fmt(y_off),
                fmt(name_col_w.max(1.0)),
                fmt(row_h.max(1.0))
            );
            let _ = write!(
                &mut out,
                r#"<text class="er attributeText" x="{}" y="{}" font-size="{}"{}>{}</text>"#,
                fmt(name_x + measure.width_padding),
                fmt(align_y),
                fmt(attr_style.font_size),
                text_style_attr,
                escape_xml(&row.name_text)
            );

            let mut x_off = name_x + name_col_w;
            if measure.has_key {
                let _ = write!(
                    &mut out,
                    r#"<rect class="er {}" x="{}" y="{}" width="{}" height="{}" />"#,
                    row_class,
                    fmt(x_off),
                    fmt(y_off),
                    fmt(key_col_w.max(1.0)),
                    fmt(row_h.max(1.0))
                );
                let _ = write!(
                    &mut out,
                    r#"<text class="er attributeText" x="{}" y="{}" font-size="{}"{}>{}</text>"#,
                    fmt(x_off + measure.width_padding),
                    fmt(align_y),
                    fmt(attr_style.font_size),
                    text_style_attr,
                    escape_xml(&row.key_text)
                );
                x_off += key_col_w;
            }
            if measure.has_comment {
                let _ = write!(
                    &mut out,
                    r#"<rect class="er {}" x="{}" y="{}" width="{}" height="{}" />"#,
                    row_class,
                    fmt(x_off),
                    fmt(y_off),
                    fmt(comment_col_w.max(1.0)),
                    fmt(row_h.max(1.0))
                );
                let _ = write!(
                    &mut out,
                    r#"<text class="er attributeText" x="{}" y="{}" font-size="{}"{}>{}</text>"#,
                    fmt(x_off + measure.width_padding),
                    fmt(align_y),
                    fmt(attr_style.font_size),
                    text_style_attr,
                    escape_xml(&row.comment_text)
                );
            }

            y_off += row_h;
            odd = !odd;
        }

        out.push_str("</g>");
    }
    out.push_str("</g>\n");

    out.push_str("</svg>\n");
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
    format!("{v:.3}")
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
