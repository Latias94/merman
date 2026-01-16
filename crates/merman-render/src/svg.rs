use crate::model::{
    Bounds, ClassDiagramV2Layout, ErDiagramLayout, FlowchartV2Layout, LayoutCluster, LayoutNode,
    StateDiagramV2Layout,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use base64::Engine as _;
use serde::Deserialize;
use std::fmt::Write as _;

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

    let mut edges_by_id: std::collections::HashMap<String, crate::flowchart::FlowEdge> =
        std::collections::HashMap::new();
    for e in model.edges.iter().cloned() {
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

    let cluster_ids: std::collections::HashSet<String> =
        model.subgraphs.iter().map(|s| s.id.clone()).collect();
    let mut recursive_clusters: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for sg in model.subgraphs.iter() {
        if sg.nodes.is_empty() {
            continue;
        }
        let mut external = false;
        for e in model.edges.iter() {
            let from_in = flowchart_is_in_cluster(&parent, &cluster_ids, &e.from, &sg.id);
            let to_in = flowchart_is_in_cluster(&parent, &cluster_ids, &e.to, &sg.id);
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

    let _ = write!(
        &mut out,
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
    );
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

    out.push_str("</g></svg>\n");
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

struct FlowchartRenderCtx<'a> {
    diagram_id: String,
    diagram_type: String,
    tx: f64,
    ty: f64,
    measurer: &'a dyn TextMeasurer,
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

    let title_html = flowchart_label_html(
        &cluster.title,
        ctx.subgraphs_by_id
            .get(&cluster.id)
            .and_then(|s| s.label_type.as_deref())
            .unwrap_or("text"),
    );

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
    let thickness_1 = match edge.stroke.as_deref() {
        Some("thick") => "edge-thickness-thick",
        Some("invisible") => "edge-thickness-invisible",
        _ => "edge-thickness-normal",
    };
    let pattern_1 = match edge.stroke.as_deref() {
        Some("dotted") => "edge-pattern-dotted",
        _ => "edge-pattern-solid",
    };

    let (thickness_2, pattern_2) = match edge.edge_type.as_deref() {
        Some("double_arrow_point") => ("edge-thickness-normal", "edge-pattern-solid"),
        _ => (thickness_1, pattern_1),
    };

    format!("{thickness_1} {pattern_1} {thickness_2} {pattern_2} flowchart-link")
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

    fn flowchart_fix_corners(
        points: &[crate::model::LayoutPoint],
    ) -> Vec<crate::model::LayoutPoint> {
        fn extract_corner_positions(points: &[crate::model::LayoutPoint]) -> Vec<usize> {
            let mut corner_positions = Vec::new();
            if points.len() < 3 {
                return corner_positions;
            }
            for i in 1..points.len().saturating_sub(1) {
                let prev = &points[i - 1];
                let curr = &points[i];
                let next = &points[i + 1];
                if prev.x == curr.x
                    && curr.y == next.y
                    && (curr.x - next.x).abs() > 5.0
                    && (curr.y - prev.y).abs() > 5.0
                {
                    corner_positions.push(i);
                    continue;
                }
                if prev.y == curr.y
                    && curr.x == next.x
                    && (curr.x - prev.x).abs() > 5.0
                    && (curr.y - next.y).abs() > 5.0
                {
                    corner_positions.push(i);
                }
            }
            corner_positions
        }

        fn find_adjacent_point(
            point_a: &crate::model::LayoutPoint,
            point_b: &crate::model::LayoutPoint,
            distance: f64,
        ) -> crate::model::LayoutPoint {
            let x_diff = point_b.x - point_a.x;
            let y_diff = point_b.y - point_a.y;
            let length = (x_diff * x_diff + y_diff * y_diff).sqrt();
            if length <= 0.0 {
                return crate::model::LayoutPoint {
                    x: point_b.x,
                    y: point_b.y,
                };
            }
            let ratio = distance / length;
            crate::model::LayoutPoint {
                x: point_b.x - ratio * x_diff,
                y: point_b.y - ratio * y_diff,
            }
        }

        let corner_positions = extract_corner_positions(points);
        if corner_positions.is_empty() {
            return points.to_vec();
        }

        let mut out = Vec::new();
        for (i, p) in points.iter().enumerate() {
            if !corner_positions.contains(&i) {
                out.push(crate::model::LayoutPoint { x: p.x, y: p.y });
                continue;
            }
            if i == 0 || i + 1 >= points.len() {
                out.push(crate::model::LayoutPoint { x: p.x, y: p.y });
                continue;
            }

            let prev_point = &points[i - 1];
            let next_point = &points[i + 1];
            let corner_point = &points[i];

            let new_prev = find_adjacent_point(prev_point, corner_point, 5.0);
            let new_next = find_adjacent_point(next_point, corner_point, 5.0);
            let x_diff = new_next.x - new_prev.x;
            let y_diff = new_next.y - new_prev.y;

            let new_prev_x = new_prev.x;
            let new_prev_y = new_prev.y;
            out.push(new_prev);

            let a = 2.0 * 2.0_f64.sqrt();
            let mut new_corner = crate::model::LayoutPoint {
                x: corner_point.x,
                y: corner_point.y,
            };
            if (next_point.x - prev_point.x).abs() > 10.0
                && (next_point.y - prev_point.y).abs() >= 10.0
            {
                let r = 5.0;
                if corner_point.x == new_prev_x {
                    new_corner = crate::model::LayoutPoint {
                        x: if x_diff < 0.0 {
                            new_prev_x - r + a
                        } else {
                            new_prev_x + r - a
                        },
                        y: if y_diff < 0.0 {
                            new_prev_y - a
                        } else {
                            new_prev_y + a
                        },
                    };
                } else {
                    new_corner = crate::model::LayoutPoint {
                        x: if x_diff < 0.0 {
                            new_prev_x - a
                        } else {
                            new_prev_x + a
                        },
                        y: if y_diff < 0.0 {
                            new_prev_y - r + a
                        } else {
                            new_prev_y + r - a
                        },
                    };
                }
            }

            out.push(new_corner);
            out.push(new_next);
        }

        out
    }

    let mut local_points: Vec<crate::model::LayoutPoint> = Vec::new();
    for p in &le.points {
        local_points.push(crate::model::LayoutPoint {
            x: p.x + ctx.tx - origin_x,
            y: p.y + ctx.ty - origin_y,
        });
    }

    let line_data: Vec<crate::model::LayoutPoint> = local_points
        .iter()
        .filter(|p| !p.y.is_nan())
        .cloned()
        .collect();
    let line_data = flowchart_fix_corners(&line_data);
    let interpolate = edge
        .interpolate
        .as_deref()
        .unwrap_or(ctx.default_edge_interpolate.as_str());
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
    let label_html = if label_text.trim().is_empty() {
        String::new()
    } else {
        flowchart_label_html(label_text, label_type)
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
    let Some(node) = ctx.nodes_by_id.get(node_id) else {
        return;
    };
    let Some(layout_node) = ctx.layout_nodes_by_id.get(node_id) else {
        return;
    };

    let x = layout_node.x + ctx.tx - origin_x;
    let y = layout_node.y + ctx.ty - origin_y;
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
            let _ = write!(
                out,
                r#"<g class="basic label-container outer-path" style="{}">"#,
                escape_attr(&style)
            );
            let _ = write!(
                out,
                r#"<path d="M0 0" stroke="none" stroke-width="0" fill="{}" style=""/>"#,
                escape_attr(&ctx.node_fill_color)
            );
            let _ = write!(
                out,
                r#"<path d="M0 0" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"#,
                escape_attr(&ctx.node_border_color)
            );
            out.push_str("</g>");
        }
        "hexagon" => {
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}">"#,
                escape_attr(&style)
            );
            let _ = write!(
                out,
                r#"<path d="M0 0" stroke="none" stroke-width="0" fill="{}" style=""/>"#,
                escape_attr(&ctx.node_fill_color)
            );
            let _ = write!(
                out,
                r#"<path d="M0 0" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"#,
                escape_attr(&ctx.node_border_color)
            );
            out.push_str("</g>");
        }
        "odd" => {
            let _ = write!(
                out,
                r#"<g class="basic label-container" transform="translate({},0)" style="{}">"#,
                fmt(0.0),
                escape_attr(&style)
            );
            let _ = write!(
                out,
                r#"<path d="M0 0" stroke="none" stroke-width="0" fill="{}" style=""/>"#,
                escape_attr(&ctx.node_fill_color)
            );
            let _ = write!(
                out,
                r#"<path d="M0 0" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"#,
                escape_attr(&ctx.node_border_color)
            );
            out.push_str("</g>");
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
    let label_html = flowchart_label_html(label_text, label_type);
    let _ = write!(
        out,
        r#"<g class="label" style="" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel">{}</span></div></foreignObject></g></g>"#,
        fmt(-metrics.width / 2.0),
        fmt(-metrics.height / 2.0),
        fmt(metrics.width),
        fmt(metrics.height),
        label_html
    );
    if wrapped_in_a {
        out.push_str("</a>");
    }
}

fn flowchart_escape_preserving_br(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let placeholder = "#br#";
    let mut t = input.to_string();
    t = t.replace("<br />", placeholder);
    t = t.replace("<br/>", placeholder);
    t = t.replace("<br>", placeholder);
    let mut out = String::with_capacity(t.len());
    for ch in t.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out.replace(placeholder, "<br />")
}

fn flowchart_label_html(label: &str, label_type: &str) -> String {
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
            merman_core::sanitize::remove_script(&html_out)
        }
        _ => {
            let label = label.replace("\r\n", "\n");
            let label = label.trim_end_matches('\n').replace('\n', "<br />");
            format!("<p>{}</p>", flowchart_escape_preserving_br(&label))
        }
    }
}
