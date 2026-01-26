use crate::model::{
    Bounds, FlowchartV2Layout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

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

fn parse_css_px_f64(v: &str) -> Option<f64> {
    let v = v.trim().trim_end_matches(';').trim();
    let v = v.trim_end_matches("px").trim();
    if v.is_empty() {
        return None;
    }
    v.parse::<f64>().ok()
}

fn normalize_css_font_family(font_family: &str) -> String {
    font_family.trim().trim_end_matches(';').trim().to_string()
}

pub(crate) fn flowchart_effective_text_style_for_classes(
    base: &TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles: &[String],
) -> TextStyle {
    let mut style = base.clone();

    for class in classes {
        let Some(decls) = class_defs.get(class) else {
            continue;
        };
        for d in decls {
            let Some((k, v)) = parse_style_decl(d) else {
                continue;
            };
            match k {
                "font-size" => {
                    if let Some(px) = parse_css_px_f64(v) {
                        style.font_size = px;
                    }
                }
                "font-family" => {
                    style.font_family = Some(normalize_css_font_family(v));
                }
                "font-weight" => {
                    style.font_weight = Some(v.trim().to_string());
                }
                _ => {}
            }
        }
    }

    for d in inline_styles {
        let Some((k, v)) = parse_style_decl(d) else {
            continue;
        };
        match k {
            "font-size" => {
                if let Some(px) = parse_css_px_f64(v) {
                    style.font_size = px;
                }
            }
            "font-family" => {
                style.font_family = Some(normalize_css_font_family(v));
            }
            "font-weight" => {
                style.font_weight = Some(v.trim().to_string());
            }
            _ => {}
        }
    }

    style
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowchartV2Model {
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "classDefs")]
    pub class_defs: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default, rename = "edgeDefaults")]
    pub edge_defaults: Option<FlowEdgeDefaults>,
    #[serde(default, rename = "vertexCalls")]
    pub vertex_calls: Vec<String>,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    #[serde(default)]
    pub subgraphs: Vec<FlowSubgraph>,
    #[serde(default)]
    pub tooltips: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowEdgeDefaults {
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub style: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowNode {
    pub id: String,
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(rename = "layoutShape")]
    pub layout_shape: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default, rename = "linkTarget")]
    pub link_target: Option<String>,
    #[serde(default, rename = "haveCallback")]
    pub have_callback: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default, rename = "type")]
    pub edge_type: Option<String>,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub style: Vec<String>,
    pub length: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowSubgraph {
    pub id: String,
    pub title: String,
    pub dir: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    pub nodes: Vec<String>,
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn rank_dir_from_flow(direction: &str) -> RankDir {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

fn normalize_dir(s: &str) -> String {
    s.trim().to_uppercase()
}

fn toggled_dir(parent: &str) -> String {
    let parent = normalize_dir(parent);
    if parent == "TB" || parent == "TD" {
        "LR".to_string()
    } else {
        "TB".to_string()
    }
}

pub(crate) fn flowchart_label_metrics_for_layout(
    measurer: &dyn TextMeasurer,
    raw_label: &str,
    label_type: &str,
    style: &TextStyle,
    max_width_px: Option<f64>,
    wrap_mode: WrapMode,
) -> crate::text::TextMetrics {
    let mut metrics = if label_type == "markdown" {
        crate::text::measure_markdown_with_flowchart_bold_deltas(
            measurer,
            raw_label,
            style,
            max_width_px,
            wrap_mode,
        )
    } else {
        let html_labels = wrap_mode == WrapMode::HtmlLike;
        if html_labels {
            fn measure_flowchart_html_images(
                measurer: &dyn TextMeasurer,
                html: &str,
                style: &TextStyle,
                max_width_px: Option<f64>,
            ) -> crate::text::TextMetrics {
                let max_width = max_width_px.unwrap_or(200.0).max(1.0);
                let lower = html.to_ascii_lowercase();
                if !lower.contains("<img") {
                    return measurer.measure_wrapped(html, style, max_width_px, WrapMode::HtmlLike);
                }

                fn has_img_src(tag: &str) -> bool {
                    let lower = tag.to_ascii_lowercase();
                    let Some(idx) = lower.find("src=") else {
                        return false;
                    };
                    let rest = tag[idx + 4..].trim_start();
                    let Some(quote) = rest.chars().next() else {
                        return false;
                    };
                    if quote != '"' && quote != '\'' {
                        return false;
                    }
                    let mut it = rest.chars();
                    let _ = it.next();
                    let mut val = String::new();
                    while let Some(ch) = it.next() {
                        if ch == quote {
                            break;
                        }
                        val.push(ch);
                    }
                    !val.trim().is_empty()
                }

                fn is_single_img_tag(html: &str) -> bool {
                    let t = html.trim();
                    let lower = t.to_ascii_lowercase();
                    if !lower.starts_with("<img") {
                        return false;
                    }
                    let Some(end) = t.find('>') else {
                        return false;
                    };
                    t[end + 1..].trim().is_empty()
                }

                let fixed_img_width = is_single_img_tag(html);
                let img_w = if fixed_img_width { 80.0 } else { max_width };

                if fixed_img_width {
                    let img_h = if has_img_src(html) { img_w } else { 0.0 };
                    return crate::text::TextMetrics {
                        width: crate::text::ceil_to_1_64_px(img_w),
                        height: crate::text::ceil_to_1_64_px(img_h),
                        line_count: if img_h > 0.0 { 1 } else { 0 },
                    };
                }

                #[derive(Debug, Clone)]
                enum Block {
                    Text(String),
                    Img { has_src: bool },
                }

                let mut blocks: Vec<Block> = Vec::new();
                let mut text_buf = String::new();

                let bytes = html.as_bytes();
                let mut i = 0usize;
                while i < bytes.len() {
                    if bytes[i] == b'<' {
                        let rest = &html[i..];
                        let rest_lower = rest.to_ascii_lowercase();
                        if rest_lower.starts_with("<img") {
                            if let Some(rel_end) = rest.find('>') {
                                if !text_buf.trim().is_empty() {
                                    blocks.push(Block::Text(std::mem::take(&mut text_buf)));
                                } else {
                                    text_buf.clear();
                                }
                                let tag = &rest[..=rel_end];
                                blocks.push(Block::Img {
                                    has_src: has_img_src(tag),
                                });
                                i += rel_end + 1;
                                continue;
                            }
                        }
                        if rest_lower.starts_with("<br") {
                            if let Some(rel_end) = rest.find('>') {
                                text_buf.push('\n');
                                i += rel_end + 1;
                                continue;
                            }
                        }
                        if let Some(rel_end) = rest.find('>') {
                            i += rel_end + 1;
                            continue;
                        }
                    }
                    let ch = html[i..].chars().next().unwrap();
                    text_buf.push(ch);
                    i += ch.len_utf8();
                }
                if !text_buf.trim().is_empty() {
                    blocks.push(Block::Text(text_buf));
                }

                fn normalize_text_block(input: &str) -> String {
                    let mut out = String::with_capacity(input.len());
                    let mut last_space = false;
                    for ch in input.chars() {
                        if ch == '\n' {
                            while out.ends_with(' ') {
                                out.pop();
                            }
                            out.push('\n');
                            last_space = false;
                            continue;
                        }
                        if ch.is_whitespace() {
                            if !last_space {
                                out.push(' ');
                            }
                            last_space = true;
                            continue;
                        }
                        out.push(ch);
                        last_space = false;
                    }
                    out.lines()
                        .map(|l| l.trim())
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_string()
                }

                let mut width: f64 = 0.0;
                let mut height: f64 = 0.0;
                let mut lines = 0usize;

                for b in blocks {
                    match b {
                        Block::Img { has_src } => {
                            width = width.max(img_w);
                            let img_h = if has_src { img_w } else { 0.0 };
                            height += img_h;
                            if img_h > 0.0 {
                                lines += 1;
                            }
                        }
                        Block::Text(t) => {
                            let t = normalize_text_block(&t);
                            if t.is_empty() {
                                continue;
                            }
                            let m = measurer.measure_wrapped(
                                &t,
                                style,
                                Some(max_width),
                                WrapMode::HtmlLike,
                            );
                            width = width.max(m.width);
                            height += m.height;
                            lines += m.line_count;
                        }
                    }
                }

                crate::text::TextMetrics {
                    width: crate::text::ceil_to_1_64_px(width),
                    height: crate::text::ceil_to_1_64_px(height),
                    line_count: lines,
                }
            }

            let mut label = raw_label.replace("\r\n", "\n");
            if label_type == "string" {
                label = label.trim().to_string();
            }
            let label = label.trim_end_matches('\n').replace('\n', "<br />");
            let fixed_img_width = {
                let t = label.trim();
                let lower = t.to_ascii_lowercase();
                lower.starts_with("<img")
                    && t.find('>')
                        .is_some_and(|end| t[end + 1..].trim().is_empty())
            };
            let html = if fixed_img_width {
                label
            } else {
                format!("<p>{}</p>", label)
            };
            let html = crate::text::replace_fontawesome_icons(&html);

            let lower = html.to_ascii_lowercase();
            let has_inline_style = crate::text::flowchart_html_has_inline_style_tags(&lower);

            if lower.contains("<img") {
                measure_flowchart_html_images(measurer, &html, style, max_width_px)
            } else if has_inline_style {
                crate::text::measure_html_with_flowchart_bold_deltas(
                    measurer,
                    &html,
                    style,
                    max_width_px,
                    wrap_mode,
                )
            } else {
                let label_for_metrics =
                    flowchart_label_plain_text_for_layout(raw_label, label_type, html_labels);
                measurer.measure_wrapped(&label_for_metrics, style, max_width_px, wrap_mode)
            }
        } else {
            let label_for_metrics =
                flowchart_label_plain_text_for_layout(raw_label, label_type, html_labels);
            measurer.measure_wrapped(&label_for_metrics, style, max_width_px, wrap_mode)
        }
    };

    if label_type == "string" {
        crate::text::flowchart_apply_mermaid_string_whitespace_height_parity(
            &mut metrics,
            raw_label,
            style,
        );
    }

    metrics
}

fn flow_dir_from_rankdir(rankdir: RankDir) -> &'static str {
    match rankdir {
        RankDir::TB => "TB",
        RankDir::BT => "BT",
        RankDir::LR => "LR",
        RankDir::RL => "RL",
    }
}

fn effective_cluster_dir(sg: &FlowSubgraph, parent_dir: &str, inherit_dir: bool) -> String {
    if let Some(dir) = sg.dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return normalize_dir(dir);
    }
    if inherit_dir {
        return normalize_dir(parent_dir);
    }
    toggled_dir(parent_dir)
}

fn compute_effective_dir_by_id(
    subgraphs_by_id: &HashMap<String, FlowSubgraph>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    diagram_dir: &str,
    inherit_dir: bool,
) -> HashMap<String, String> {
    fn compute_one(
        id: &str,
        subgraphs_by_id: &HashMap<String, FlowSubgraph>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        diagram_dir: &str,
        inherit_dir: bool,
        visiting: &mut std::collections::HashSet<String>,
        memo: &mut HashMap<String, String>,
    ) -> String {
        if let Some(dir) = memo.get(id) {
            return dir.clone();
        }
        if !visiting.insert(id.to_string()) {
            let dir = toggled_dir(diagram_dir);
            memo.insert(id.to_string(), dir.clone());
            return dir;
        }

        let parent_dir = g
            .parent(id)
            .and_then(|p| subgraphs_by_id.contains_key(p).then_some(p.to_string()))
            .map(|p| {
                compute_one(
                    &p,
                    subgraphs_by_id,
                    g,
                    diagram_dir,
                    inherit_dir,
                    visiting,
                    memo,
                )
            })
            .unwrap_or_else(|| normalize_dir(diagram_dir));

        let dir = subgraphs_by_id
            .get(id)
            .map(|sg| effective_cluster_dir(sg, &parent_dir, inherit_dir))
            .unwrap_or_else(|| toggled_dir(&parent_dir));

        memo.insert(id.to_string(), dir.clone());
        let _ = visiting.remove(id);
        dir
    }

    let mut memo: HashMap<String, String> = HashMap::new();
    let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();
    for id in subgraphs_by_id.keys() {
        let _ = compute_one(
            id,
            subgraphs_by_id,
            g,
            diagram_dir,
            inherit_dir,
            &mut visiting,
            &mut memo,
        );
    }
    memo
}

fn dir_to_rankdir(dir: &str) -> RankDir {
    match normalize_dir(dir).as_str() {
        "TB" | "TD" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

fn edge_label_is_non_empty(edge: &FlowEdge) -> bool {
    edge.label
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(feature = "flowchart_root_pack")]
fn edge_label_leaf_id(edge: &FlowEdge) -> String {
    format!("edge-label::{}", edge.id)
}

fn lowest_common_parent(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    a: &str,
    b: &str,
) -> Option<String> {
    if !g.options().compound {
        return None;
    }

    let mut ancestors: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cur = g.parent(a);
    while let Some(p) = cur {
        ancestors.insert(p.to_string());
        cur = g.parent(p);
    }

    let mut cur = g.parent(b);
    while let Some(p) = cur {
        if ancestors.contains(p) {
            return Some(p.to_string());
        }
        cur = g.parent(p);
    }

    None
}

fn extract_descendants(id: &str, g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Vec<String> {
    let children = g.children(id);
    let mut out: Vec<String> = children.iter().map(|s| s.to_string()).collect();
    for child in children {
        out.extend(extract_descendants(child, g));
    }
    out
}

fn edge_in_cluster(
    edge: &dugong::graphlib::EdgeKey,
    cluster_id: &str,
    descendants: &std::collections::HashMap<String, Vec<String>>,
) -> bool {
    if edge.v == cluster_id || edge.w == cluster_id {
        return false;
    }
    let Some(cluster_desc) = descendants.get(cluster_id) else {
        return false;
    };
    cluster_desc.contains(&edge.v) || cluster_desc.contains(&edge.w)
}

#[derive(Debug, Clone)]
struct FlowchartClusterDbEntry {
    anchor_id: String,
    external_connections: bool,
}

fn flowchart_find_common_edges(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    id1: &str,
    id2: &str,
) -> Vec<(String, String)> {
    let edges1: Vec<(String, String)> = graph
        .edge_keys()
        .into_iter()
        .filter(|edge| edge.v == id1 || edge.w == id1)
        .map(|edge| (edge.v, edge.w))
        .collect();
    let edges2: Vec<(String, String)> = graph
        .edge_keys()
        .into_iter()
        .filter(|edge| edge.v == id2 || edge.w == id2)
        .map(|edge| (edge.v, edge.w))
        .collect();

    let edges1_prim: Vec<(String, String)> = edges1
        .into_iter()
        .map(|(v, w)| {
            (
                if v == id1 { id2.to_string() } else { v },
                // Mermaid's `findCommonEdges(...)` has an asymmetry here: it maps the `w` side
                // back to `id1` rather than `id2` (Mermaid@11.12.2).
                if w == id1 { id1.to_string() } else { w },
            )
        })
        .collect();

    let mut out = Vec::new();
    for e1 in edges1_prim {
        if edges2.iter().any(|e2| *e2 == e1) {
            out.push(e1);
        }
    }
    out
}

fn flowchart_find_non_cluster_child(
    id: &str,
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    cluster_id: &str,
) -> Option<String> {
    let children = graph.children(id);
    if children.is_empty() {
        return Some(id.to_string());
    }
    let mut reserve: Option<String> = None;
    for child in children {
        let Some(child_id) = flowchart_find_non_cluster_child(child, graph, cluster_id) else {
            continue;
        };
        let common_edges = flowchart_find_common_edges(graph, cluster_id, &child_id);
        if !common_edges.is_empty() {
            reserve = Some(child_id);
        } else {
            return Some(child_id);
        }
    }
    reserve
}

fn adjust_flowchart_clusters_and_edges(graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    use serde_json::Value;

    fn is_descendant(
        node_id: &str,
        cluster_id: &str,
        descendants: &std::collections::HashMap<String, Vec<String>>,
    ) -> bool {
        descendants
            .get(cluster_id)
            .is_some_and(|ds| ds.iter().any(|n| n == node_id))
    }

    let mut descendants: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut cluster_db: std::collections::HashMap<String, FlowchartClusterDbEntry> =
        std::collections::HashMap::new();

    for id in graph.node_ids() {
        if graph.children(&id).is_empty() {
            continue;
        }
        descendants.insert(id.clone(), extract_descendants(&id, graph));
        let anchor_id =
            flowchart_find_non_cluster_child(&id, graph, &id).unwrap_or_else(|| id.clone());
        cluster_db.insert(
            id,
            FlowchartClusterDbEntry {
                anchor_id,
                external_connections: false,
            },
        );
    }

    for id in cluster_db.keys().cloned().collect::<Vec<_>>() {
        if graph.children(&id).is_empty() {
            continue;
        }
        let mut has_external = false;
        for e in graph.edges() {
            let d1 = is_descendant(e.v.as_str(), id.as_str(), &descendants);
            let d2 = is_descendant(e.w.as_str(), id.as_str(), &descendants);
            if d1 ^ d2 {
                has_external = true;
                break;
            }
        }
        if let Some(entry) = cluster_db.get_mut(&id) {
            entry.external_connections = has_external;
        }
    }

    for id in cluster_db.keys().cloned().collect::<Vec<_>>() {
        let Some(non_cluster_child) = cluster_db.get(&id).map(|e| e.anchor_id.clone()) else {
            continue;
        };
        let parent = graph.parent(&non_cluster_child);
        if parent.is_some_and(|p| p != id.as_str())
            && parent.is_some_and(|p| cluster_db.contains_key(p))
            && parent.is_some_and(|p| !cluster_db.get(p).is_some_and(|e| e.external_connections))
        {
            if let Some(p) = parent {
                if let Some(entry) = cluster_db.get_mut(&id) {
                    entry.anchor_id = p.to_string();
                }
            }
        }
    }

    fn get_anchor_id(
        id: &str,
        cluster_db: &std::collections::HashMap<String, FlowchartClusterDbEntry>,
    ) -> String {
        let Some(entry) = cluster_db.get(id) else {
            return id.to_string();
        };
        if !entry.external_connections {
            return id.to_string();
        }
        entry.anchor_id.clone()
    }

    let edge_keys = graph.edge_keys();
    for ek in edge_keys {
        if !cluster_db.contains_key(&ek.v) && !cluster_db.contains_key(&ek.w) {
            continue;
        }

        let Some(mut edge_label) = graph.edge_by_key(&ek).cloned() else {
            continue;
        };

        let v = get_anchor_id(&ek.v, &cluster_db);
        let w = get_anchor_id(&ek.w, &cluster_db);

        // Match Mermaid `adjustClustersAndEdges`: edges that touch cluster nodes are removed and
        // re-inserted even when their endpoints do not change. This affects edge iteration order
        // and therefore cycle-breaking determinism in Dagre's acyclic pass.
        let _ = graph.remove_edge_key(&ek);

        if v != ek.v {
            if let Some(parent) = graph.parent(&v) {
                if let Some(entry) = cluster_db.get_mut(parent) {
                    entry.external_connections = true;
                }
            }
            edge_label
                .extras
                .insert("fromCluster".to_string(), Value::String(ek.v.clone()));
        }

        if w != ek.w {
            if let Some(parent) = graph.parent(&w) {
                if let Some(entry) = cluster_db.get_mut(parent) {
                    entry.external_connections = true;
                }
            }
            edge_label
                .extras
                .insert("toCluster".to_string(), Value::String(ek.w.clone()));
        }

        graph.set_edge_named(v, w, ek.name, Some(edge_label));
    }
}

fn copy_cluster(
    cluster_id: &str,
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    new_graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root_id: &str,
    descendants: &std::collections::HashMap<String, Vec<String>>,
) {
    let mut nodes: Vec<String> = graph
        .children(cluster_id)
        .iter()
        .map(|s| s.to_string())
        .collect();
    if cluster_id != root_id {
        nodes.push(cluster_id.to_string());
    }

    for node in nodes {
        if !graph.has_node(&node) {
            continue;
        }

        if !graph.children(&node).is_empty() {
            copy_cluster(&node, graph, new_graph, root_id, descendants);
        } else {
            let data = graph.node(&node).cloned().unwrap_or_default();
            new_graph.set_node(node.clone(), data);

            if let Some(parent) = graph.parent(&node) {
                if parent != root_id {
                    new_graph.set_parent(node.clone(), parent.to_string());
                }
            }
            if cluster_id != root_id && node != cluster_id {
                new_graph.set_parent(node.clone(), cluster_id.to_string());
            }

            // Copy edges for this extracted cluster.
            //
            // Mermaid's implementation calls `graph.edges(node)` (note: on dagre-d3-es Graphlib,
            // `edges()` ignores the argument and returns *all* edges). Because the source graph is
            // mutated as nodes are removed, this makes edge insertion order sensitive to the leaf
            // traversal order, which in turn can affect deterministic tie-breaking in Dagre's
            // acyclic/ranking steps.
            //
            // Reference:
            // - `packages/mermaid/src/rendering-util/layout-algorithms/dagre/mermaid-graphlib.js`
            for ek in graph.edge_keys() {
                if !edge_in_cluster(&ek, root_id, descendants) {
                    continue;
                }
                if new_graph.has_edge(&ek.v, &ek.w, ek.name.as_deref()) {
                    continue;
                }
                let Some(lbl) = graph.edge_by_key(&ek).cloned() else {
                    continue;
                };
                new_graph.set_edge_named(ek.v, ek.w, ek.name, Some(lbl));
            }
        }

        let _ = graph.remove_node(&node);
    }
}

fn extract_clusters_recursively(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    subgraphs_by_id: &std::collections::HashMap<String, FlowSubgraph>,
    effective_dir_by_id: &std::collections::HashMap<String, String>,
    extracted: &mut std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
    depth: usize,
) {
    if depth > 10 {
        return;
    }

    let node_ids = graph.node_ids();
    let mut descendants: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for id in &node_ids {
        if graph.children(id).is_empty() {
            continue;
        }
        descendants.insert(id.clone(), extract_descendants(id, graph));
    }

    let mut external: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for id in descendants.keys() {
        let Some(ds) = descendants.get(id) else {
            continue;
        };
        let mut has_external = false;
        for e in graph.edges() {
            let d1 = ds.contains(&e.v);
            let d2 = ds.contains(&e.w);
            if d1 ^ d2 {
                has_external = true;
                break;
            }
        }
        external.insert(id.clone(), has_external);
    }

    let mut extracted_here: Vec<(String, Graph<NodeLabel, EdgeLabel, GraphLabel>)> = Vec::new();

    let candidates: Vec<String> = node_ids
        .into_iter()
        .filter(|id| graph.has_node(id))
        .filter(|id| !graph.children(id).is_empty())
        // Mermaid's extractor does not require clusters to be root-level; it only checks
        // `externalConnections` and `children.length > 0`, then recurses into extracted graphs.
        //
        // Reference:
        // - `packages/mermaid/src/rendering-util/layout-algorithms/dagre/mermaid-graphlib.js`
        .filter(|id| !external.get(id).copied().unwrap_or(false))
        .collect();

    for id in candidates {
        if !graph.has_node(&id) {
            continue;
        }
        if graph.children(&id).is_empty() {
            continue;
        }

        let mut cluster_graph: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            directed: true,
        });

        // Mermaid's `extractor(...)` uses:
        // - `clusterData.dir` when explicitly set for the subgraph
        // - otherwise: toggle relative to the current graph's rankdir (TB<->LR)
        let dir = subgraphs_by_id
            .get(&id)
            .and_then(|sg| sg.dir.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(normalize_dir)
            .unwrap_or_else(|| toggled_dir(flow_dir_from_rankdir(graph.graph().rankdir)));

        cluster_graph.set_graph(GraphLabel {
            rankdir: dir_to_rankdir(&dir),
            // Mermaid's cluster extractor initializes subgraphs with a fixed dagre config
            // (nodesep/ranksep=50, marginx/marginy=8). Before each recursive render Mermaid then
            // overrides `nodesep` to the parent graph value and `ranksep` to `parent.ranksep + 25`.
            //
            // We model that in headless mode by keeping the extractor defaults here, then applying
            // the per-depth override inside `layout_graph_with_recursive_clusters(...)` right
            // before laying out each extracted graph.
            //
            // Reference:
            // - `packages/mermaid/src/rendering-util/layout-algorithms/dagre/mermaid-graphlib.js`
            // - `packages/mermaid/src/rendering-util/layout-algorithms/dagre/index.js`
            nodesep: 50.0,
            ranksep: 50.0,
            marginx: 8.0,
            marginy: 8.0,
            acyclicer: None,
            ..Default::default()
        });

        copy_cluster(&id, graph, &mut cluster_graph, &id, &descendants);
        extracted_here.push((id, cluster_graph));
    }

    for (id, mut g) in extracted_here {
        extract_clusters_recursively(
            &mut g,
            subgraphs_by_id,
            effective_dir_by_id,
            extracted,
            depth + 1,
        );
        extracted.insert(id, g);
    }
}

pub fn layout_flowchart_v2(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<FlowchartV2Layout> {
    let model: FlowchartV2Model = serde_json::from_value(semantic.clone())?;

    // Mermaid's dagre adapter expands self-loop edges into a chain of two special label nodes plus
    // three edges. This avoids `v == w` edges in Dagre and is required for SVG parity (Mermaid
    // uses `*-cyclic-special-*` ids when rendering self-loops).
    let mut render_edges: Vec<FlowEdge> = Vec::new();
    let mut self_loop_label_node_ids: Vec<String> = Vec::new();
    let mut self_loop_label_node_id_set: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for e in &model.edges {
        if e.from != e.to {
            render_edges.push(e.clone());
            continue;
        }

        let node_id = e.from.clone();
        let special_id_1 = format!("{node_id}---{node_id}---1");
        let special_id_2 = format!("{node_id}---{node_id}---2");
        if self_loop_label_node_id_set.insert(special_id_1.clone()) {
            self_loop_label_node_ids.push(special_id_1.clone());
        }
        if self_loop_label_node_id_set.insert(special_id_2.clone()) {
            self_loop_label_node_ids.push(special_id_2.clone());
        }

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
        // Mermaid clears the label text on the end segments, but keeps the label (if any) on the
        // mid edge (`edgeMid` is a structuredClone of the original edge without label changes).
        edge1.label = Some(String::new());
        edge2.label = Some(String::new());

        render_edges.push(edge1);
        render_edges.push(edge_mid);
        render_edges.push(edge2);
    }

    let nodesep = config_f64(effective_config, &["flowchart", "nodeSpacing"]).unwrap_or(50.0);
    let ranksep = config_f64(effective_config, &["flowchart", "rankSpacing"]).unwrap_or(50.0);
    // Mermaid's default config sets `flowchart.padding` to 15.
    let node_padding = config_f64(effective_config, &["flowchart", "padding"]).unwrap_or(15.0);
    let wrapping_width =
        config_f64(effective_config, &["flowchart", "wrappingWidth"]).unwrap_or(200.0);
    // Mermaid@11.12.2 renders subgraph titles via the `createText(...)` path and applies a default
    // wrapping width of 200px (even when `labelType=text` and `htmlLabels=false`), which results
    // in `<tspan>`-wrapped titles for long words. Match that behavior in headless metrics.
    let cluster_title_wrapping_width = 200.0;
    // Mermaid flowchart-v2 uses the global `htmlLabels` toggle for node/subgraph labels, while
    // edge labels follow `flowchart.htmlLabels` (falling back to the global toggle when unset).
    let node_html_labels = effective_config
        .get("htmlLabels")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let edge_html_labels = effective_config
        .get("flowchart")
        .and_then(|v| v.get("htmlLabels"))
        .and_then(Value::as_bool)
        .unwrap_or(node_html_labels);
    let node_wrap_mode = if node_html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    let edge_wrap_mode = if edge_html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    // Mermaid FlowDB encodes subgraph nodes with a fixed `padding: 8` in `data4Layout.nodes`.
    // That value is separate from `flowchart.padding` (node padding) and `nodeSpacing`/`rankSpacing`.
    let cluster_padding = 8.0;
    let title_margin_top = config_f64(
        effective_config,
        &["flowchart", "subGraphTitleMargin", "top"],
    )
    .unwrap_or(0.0);
    let title_margin_bottom = config_f64(
        effective_config,
        &["flowchart", "subGraphTitleMargin", "bottom"],
    )
    .unwrap_or(0.0);
    let title_total_margin = title_margin_top + title_margin_bottom;
    let y_shift = title_total_margin / 2.0;
    let inherit_dir = effective_config
        .get("flowchart")
        .and_then(|v| v.get("inheritDir"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let font_family = config_string(effective_config, &["fontFamily"])
        .or_else(|| Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()));
    let font_size = config_f64(effective_config, &["fontSize"]).unwrap_or(16.0);
    let font_weight = config_string(effective_config, &["fontWeight"]);
    let text_style = TextStyle {
        font_family,
        font_size,
        font_weight,
    };

    let diagram_direction = normalize_dir(model.direction.as_deref().unwrap_or("TB"));
    let has_subgraphs = !model.subgraphs.is_empty();
    let mut subgraphs_by_id: std::collections::HashMap<String, FlowSubgraph> =
        std::collections::HashMap::new();
    for sg in &model.subgraphs {
        subgraphs_by_id.insert(sg.id.clone(), sg.clone());
    }
    let subgraph_ids: std::collections::HashSet<&str> =
        model.subgraphs.iter().map(|sg| sg.id.as_str()).collect();
    let subgraph_id_set: std::collections::HashSet<String> =
        model.subgraphs.iter().map(|sg| sg.id.clone()).collect();
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        // Mermaid's Dagre adapter always enables `compound: true`, even if there are no explicit
        // subgraphs. This also allows `nestingGraph.run` to connect components during ranking.
        compound: true,
        directed: true,
    });
    g.set_graph(GraphLabel {
        rankdir: rank_dir_from_flow(&diagram_direction),
        nodesep,
        ranksep,
        marginx: 8.0,
        marginy: 8.0,
        acyclicer: None,
        ..Default::default()
    });

    let mut empty_subgraph_ids: Vec<String> = Vec::new();
    let mut cluster_node_labels: std::collections::HashMap<String, NodeLabel> =
        std::collections::HashMap::new();
    for sg in &model.subgraphs {
        if sg.nodes.is_empty() {
            // Mermaid renders empty subgraphs as regular nodes. Keep the semantic `subgraph`
            // definition around for styling/title, but size + lay it out as a leaf node.
            empty_subgraph_ids.push(sg.id.clone());
            continue;
        }
        // Mermaid does not pre-size compound (subgraph) nodes based on title metrics for Dagre
        // layout. Their dimensions are computed from children (border nodes) and then adjusted at
        // render time for title width and configured margins.
        cluster_node_labels.insert(sg.id.clone(), NodeLabel::default());
    }

    let mut leaf_node_labels: std::collections::HashMap<String, NodeLabel> =
        std::collections::HashMap::new();
    for n in &model.nodes {
        // Mermaid treats the subgraph id as the "group node" id (a cluster can be referenced in
        // edges). Avoid introducing a separate leaf node that would collide with the cluster node
        // of the same id.
        if subgraph_ids.contains(n.id.as_str()) {
            continue;
        }
        let raw_label = n.label.as_deref().unwrap_or(&n.id);
        let label_type = n.label_type.as_deref().unwrap_or("text");
        let node_text_style = flowchart_effective_text_style_for_classes(
            &text_style,
            &model.class_defs,
            &n.classes,
            &n.styles,
        );
        let mut metrics = flowchart_label_metrics_for_layout(
            measurer,
            raw_label,
            label_type,
            &node_text_style,
            Some(wrapping_width),
            node_wrap_mode,
        );
        let span_css_height_parity = n.classes.iter().any(|c| {
            model.class_defs.get(c.as_str()).is_some_and(|styles| {
                styles.iter().any(|s| {
                    matches!(
                        s.split_once(':').map(|p| p.0.trim()),
                        Some("background" | "border")
                    )
                })
            })
        });
        if span_css_height_parity {
            crate::text::flowchart_apply_mermaid_styled_node_height_parity(
                &mut metrics,
                &node_text_style,
            );
        }
        let (width, height) =
            node_layout_dimensions(n.layout_shape.as_deref(), metrics, node_padding);
        leaf_node_labels.insert(
            n.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }
    for sg in &model.subgraphs {
        if !sg.nodes.is_empty() {
            continue;
        }
        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let sg_text_style = flowchart_effective_text_style_for_classes(
            &text_style,
            &model.class_defs,
            &sg.classes,
            &[],
        );
        let metrics = flowchart_label_metrics_for_layout(
            measurer,
            &sg.title,
            label_type,
            &sg_text_style,
            Some(cluster_title_wrapping_width),
            node_wrap_mode,
        );
        let (width, height) = node_layout_dimensions(Some("squareRect"), metrics, cluster_padding);
        leaf_node_labels.insert(
            sg.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }

    // Mermaid constructs the Dagre graph by:
    // 1) inserting subgraph (cluster) nodes first (in reverse `subgraphs[]` order), then
    // 2) inserting vertex nodes (in FlowDB `Map` insertion order),
    // and setting `parentId` as each node is inserted.
    //
    // Matching this order matters because Graphlib insertion order can affect compound-graph
    // child ordering, anchor selection and deterministic tie-breaking in layout.
    let mut inserted: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut parent_assigned: std::collections::HashSet<String> = std::collections::HashSet::new();
    let insert_one = |id: &str,
                      g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
                      inserted: &mut std::collections::HashSet<String>| {
        if inserted.contains(id) {
            return;
        }
        if let Some(lbl) = cluster_node_labels.get(id).cloned() {
            g.set_node(id.to_string(), lbl);
            inserted.insert(id.to_string());
            return;
        }
        if let Some(lbl) = leaf_node_labels.get(id).cloned() {
            g.set_node(id.to_string(), lbl);
            inserted.insert(id.to_string());
        }
    };

    if has_subgraphs {
        // Match Mermaid's `FlowDB.getData()` parent assignment: build `parentId` by iterating
        // subgraphs in reverse order and recording each membership.
        let mut parent_by_id: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for sg in model.subgraphs.iter().rev() {
            for child in &sg.nodes {
                parent_by_id.insert(child.clone(), sg.id.clone());
            }
        }

        let insert_with_parent =
            |id: &str,
             g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
             inserted: &mut std::collections::HashSet<String>,
             parent_assigned: &mut std::collections::HashSet<String>| {
                insert_one(id, g, inserted);
                if !parent_assigned.insert(id.to_string()) {
                    return;
                }
                if let Some(p) = parent_by_id.get(id).cloned() {
                    g.set_parent(id.to_string(), p);
                }
            };

        for sg in model.subgraphs.iter().rev() {
            insert_with_parent(sg.id.as_str(), &mut g, &mut inserted, &mut parent_assigned);
        }
        for n in &model.nodes {
            insert_with_parent(n.id.as_str(), &mut g, &mut inserted, &mut parent_assigned);
        }
        for id in &model.vertex_calls {
            insert_with_parent(id.as_str(), &mut g, &mut inserted, &mut parent_assigned);
        }
    } else {
        // No subgraphs: insertion order still matters for deterministic Dagre tie-breaking.
        for n in &model.nodes {
            insert_one(n.id.as_str(), &mut g, &mut inserted);
        }
        for id in &model.vertex_calls {
            insert_one(id.as_str(), &mut g, &mut inserted);
        }
    }

    // Materialize self-loop helper label nodes and place them in the same parent cluster as the
    // base node (if any), matching Mermaid `@11.12.2` dagre layout adapter behavior.
    for id in &self_loop_label_node_ids {
        if !g.has_node(id) {
            g.set_node(
                id.clone(),
                NodeLabel {
                    // Mermaid initializes these labelRect nodes at 10x10, but then immediately
                    // runs `insertNode(...)` + `updateNodeBounds(...)` before Dagre layout. For an
                    // empty `labelRect`, the measured bbox collapses to ~0.1x0.1 and that is what
                    // Dagre actually sees for spacing. Match that here for layout parity.
                    width: 0.1,
                    height: 0.1,
                    ..Default::default()
                },
            );
        }
        let Some((base, _)) = id.split_once("---") else {
            continue;
        };
        if let Some(p) = g.parent(base) {
            g.set_parent(id.clone(), p.to_string());
        }
    }

    let effective_dir_by_id = if has_subgraphs {
        compute_effective_dir_by_id(&subgraphs_by_id, &g, &diagram_direction, inherit_dir)
    } else {
        HashMap::new()
    };

    // Map SVG edge ids to the multigraph key used by the Dagre layout graph. Most edges use their
    // `id` as the key, but Mermaid uses distinct keys for the self-loop special edges and we also
    // want deterministic ordering under our BTree-backed graph storage.
    let mut edge_key_by_id: HashMap<String, String> = HashMap::new();
    let mut edge_id_by_key: HashMap<String, String> = HashMap::new();

    for e in &render_edges {
        // Mermaid uses distinct graphlib multigraph keys for self-loop helper edges.
        // Reference: `packages/mermaid/src/rendering-util/layout-algorithms/dagre/index.js`
        let edge_key = if let Some(prefix) = e.id.strip_suffix("-cyclic-special-1") {
            format!("{prefix}-cyclic-special-0")
        } else if let Some(prefix) = e.id.strip_suffix("-cyclic-special-mid") {
            format!("{prefix}-cyclic-special-1")
        } else if let Some(prefix) = e.id.strip_suffix("-cyclic-special-2") {
            // Mermaid contains this typo in the edge key (note the `<`):
            // `nodeId + '-cyc<lic-special-2'`
            format!("{prefix}-cyc<lic-special-2")
        } else {
            e.id.clone()
        };
        edge_key_by_id.insert(e.id.clone(), edge_key.clone());
        edge_id_by_key.insert(edge_key.clone(), e.id.clone());

        let from = e.from.clone();
        let to = e.to.clone();

        if edge_label_is_non_empty(e) {
            let label_text = e.label.as_deref().unwrap_or_default();
            let label_type = e.label_type.as_deref().unwrap_or("text");
            let edge_text_style = flowchart_effective_text_style_for_classes(
                &text_style,
                &model.class_defs,
                &e.classes,
                &e.style,
            );
            let metrics = flowchart_label_metrics_for_layout(
                measurer,
                label_text,
                label_type,
                &edge_text_style,
                Some(wrapping_width),
                edge_wrap_mode,
            );
            let (label_width, label_height) = if edge_html_labels {
                (metrics.width.max(1.0), metrics.height.max(1.0))
            } else {
                // Mermaid's SVG edge-labels include a padded background rect (+2px left/right and
                // +2px top/bottom).
                (
                    (metrics.width + 4.0).max(1.0),
                    (metrics.height + 4.0).max(1.0),
                )
            };

            let minlen = e.length.max(1);
            let el = EdgeLabel {
                width: label_width,
                height: label_height,
                labelpos: LabelPos::C,
                // Dagre layout defaults `labeloffset` to 10 when unspecified.
                labeloffset: 10.0,
                minlen,
                weight: 1.0,
                ..Default::default()
            };

            g.set_edge_named(from, to, Some(edge_key), Some(el));
        } else {
            let el = EdgeLabel {
                width: 0.0,
                height: 0.0,
                labelpos: LabelPos::C,
                // Dagre layout defaults `labeloffset` to 10 when unspecified.
                labeloffset: 10.0,
                minlen: e.length.max(1),
                weight: 1.0,
                ..Default::default()
            };
            g.set_edge_named(from, to, Some(edge_key), Some(el));
        }
    }

    if has_subgraphs {
        adjust_flowchart_clusters_and_edges(&mut g);
    }

    let mut edge_endpoints_by_id: HashMap<String, (String, String)> = HashMap::new();
    for ek in g.edge_keys() {
        let Some(edge_key) = ek.name.as_deref() else {
            continue;
        };
        let edge_id = edge_id_by_key
            .get(edge_key)
            .cloned()
            .unwrap_or_else(|| edge_key.to_string());
        edge_endpoints_by_id.insert(edge_id, (ek.v.clone(), ek.w.clone()));
    }

    let mut extracted_graphs: std::collections::HashMap<
        String,
        Graph<NodeLabel, EdgeLabel, GraphLabel>,
    > = std::collections::HashMap::new();
    if has_subgraphs {
        extract_clusters_recursively(
            &mut g,
            &subgraphs_by_id,
            &effective_dir_by_id,
            &mut extracted_graphs,
            0,
        );
    }

    #[derive(Debug, Clone, Copy)]
    struct Rect {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    impl Rect {
        fn from_center(x: f64, y: f64, width: f64, height: f64) -> Self {
            let hw = width / 2.0;
            let hh = height / 2.0;
            Self {
                min_x: x - hw,
                min_y: y - hh,
                max_x: x + hw,
                max_y: y + hh,
            }
        }

        fn width(&self) -> f64 {
            self.max_x - self.min_x
        }

        fn height(&self) -> f64 {
            self.max_y - self.min_y
        }

        fn center(&self) -> (f64, f64) {
            (
                (self.min_x + self.max_x) / 2.0,
                (self.min_y + self.max_y) / 2.0,
            )
        }

        fn union(&mut self, other: Rect) {
            self.min_x = self.min_x.min(other.min_x);
            self.min_y = self.min_y.min(other.min_y);
            self.max_x = self.max_x.max(other.max_x);
            self.max_y = self.max_y.max(other.max_y);
        }

        fn translate(&mut self, dx: f64, dy: f64) {
            self.min_x += dx;
            self.max_x += dx;
            self.min_y += dy;
            self.max_y += dy;
        }
    }

    fn extracted_graph_bbox_rect(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        title_total_margin: f64,
        extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_id_set: &std::collections::HashSet<String>,
    ) -> Option<Rect> {
        fn graph_content_rect(
            g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
            subgraph_id_set: &std::collections::HashSet<String>,
            title_total_margin: f64,
        ) -> Option<Rect> {
            let mut out: Option<Rect> = None;
            for id in g.node_ids() {
                let Some(n) = g.node(&id) else { continue };
                let (Some(x), Some(y)) = (n.x, n.y) else {
                    continue;
                };
                let mut height = n.height;
                let is_cluster_node = extracted.contains_key(&id) && g.children(&id).is_empty();
                let is_non_recursive_cluster =
                    subgraph_id_set.contains(&id) && !g.children(&id).is_empty();

                // Mermaid increases cluster node height by `subGraphTitleTotalMargin` *after* Dagre
                // layout (just before rendering), and `updateNodeBounds(...)` measures the DOM
                // bbox after that expansion. Mirror that here for non-recursive clusters.
                //
                // For leaf clusterNodes (recursively rendered clusters), the node's width/height
                // comes directly from `updateNodeBounds(...)`, so do not add margins again.
                if !is_cluster_node && is_non_recursive_cluster && title_total_margin > 0.0 {
                    height = (height + title_total_margin).max(1.0);
                }

                let r = Rect::from_center(x, y, n.width, height);
                if let Some(ref mut cur) = out {
                    cur.union(r);
                } else {
                    out = Some(r);
                }
            }
            for ek in g.edge_keys() {
                let Some(e) = g.edge_by_key(&ek) else {
                    continue;
                };
                let (Some(x), Some(y)) = (e.x, e.y) else {
                    continue;
                };
                if e.width <= 0.0 && e.height <= 0.0 {
                    continue;
                }
                let r = Rect::from_center(x, y, e.width, e.height);
                if let Some(ref mut cur) = out {
                    cur.union(r);
                } else {
                    out = Some(r);
                }
            }
            out
        }

        graph_content_rect(g, extracted, subgraph_id_set, title_total_margin)
    }

    fn apply_mermaid_subgraph_title_shifts(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_id_set: &std::collections::HashSet<String>,
        y_shift: f64,
    ) {
        if y_shift.abs() < 1e-9 {
            return;
        }

        // Mermaid v11.12.2 adjusts Y positions after Dagre layout:
        // - regular nodes: +subGraphTitleTotalMargin/2
        // - clusterNode nodes (recursively rendered clusters): +subGraphTitleTotalMargin
        // - pure cluster nodes (non-recursive clusters): no y-shift (but height grows elsewhere)
        for id in graph.node_ids() {
            // A cluster is only a Mermaid "clusterNode" placeholder if it is a leaf in the
            // current graph. Extracted graphs contain an injected parent cluster node with the
            // same id (and children), which must follow the pure-cluster path.
            let is_cluster_node = extracted.contains_key(&id) && graph.children(&id).is_empty();
            let delta_y = if is_cluster_node {
                y_shift * 2.0
            } else if subgraph_id_set.contains(&id) && !graph.children(&id).is_empty() {
                0.0
            } else {
                y_shift
            };
            if delta_y.abs() > 1e-9 {
                let Some(y) = graph.node(&id).and_then(|n| n.y) else {
                    continue;
                };
                if let Some(n) = graph.node_mut(&id) {
                    n.y = Some(y + delta_y);
                }
            }
        }

        // Mermaid shifts all edge points and the edge label position by +subGraphTitleTotalMargin/2.
        for ek in graph.edge_keys() {
            let Some(e) = graph.edge_mut_by_key(&ek) else {
                continue;
            };
            if let Some(y) = e.y {
                e.y = Some(y + y_shift);
            }
            for p in &mut e.points {
                p.y += y_shift;
            }
        }
    }

    fn layout_graph_with_recursive_clusters(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        graph_cluster_id: Option<&str>,
        extracted: &mut std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        depth: usize,
        subgraph_id_set: &std::collections::HashSet<String>,
        y_shift: f64,
        cluster_node_labels: &std::collections::HashMap<String, NodeLabel>,
        title_total_margin: f64,
    ) {
        if depth > 10 {
            dugong::layout_dagreish(graph);
            apply_mermaid_subgraph_title_shifts(graph, extracted, subgraph_id_set, y_shift);
            return;
        }

        // Layout child graphs first, then update the corresponding node sizes before laying out
        // the parent graph. This mirrors Mermaid: `recursiveRender` computes clusterNode sizes
        // before `dagreLayout(graph)`.
        let ids = graph.node_ids();
        for id in ids {
            if !extracted.contains_key(&id) {
                continue;
            }
            // Only treat leaf cluster nodes as "clusterNode" placeholders. RecursiveRender adds
            // the parent cluster node (with children) into the child graph before layout, so the
            // cluster id will exist there but should not recurse back into itself.
            if !graph.children(&id).is_empty() {
                continue;
            }
            let mut child = match extracted.remove(&id) {
                Some(g) => g,
                None => continue,
            };

            // Match Mermaid `recursiveRender` behavior: before laying out a recursively rendered
            // cluster graph, override `nodesep` to the parent graph spacing and `ranksep` to
            // `parent.ranksep + 25`. This compounds for nested recursive clusters (each recursion
            // level adds another +25).
            let parent_nodesep = graph.graph().nodesep;
            let parent_ranksep = graph.graph().ranksep;
            child.graph_mut().nodesep = parent_nodesep;
            child.graph_mut().ranksep = parent_ranksep + 25.0;

            layout_graph_with_recursive_clusters(
                &mut child,
                Some(id.as_str()),
                extracted,
                depth + 1,
                subgraph_id_set,
                y_shift,
                cluster_node_labels,
                title_total_margin,
            );

            // In Mermaid, `updateNodeBounds(...)` measures the recursively rendered `<g class="root">`
            // group. In that render path, the child graph contains a node matching the cluster id
            // (inserted via `graph.setNode(parentCluster.id, ...)`), whose computed compound bounds
            // correspond to the cluster box measured in the DOM.
            if let Some(r) =
                extracted_graph_bbox_rect(&child, title_total_margin, extracted, subgraph_id_set)
            {
                if let Some(n) = graph.node_mut(&id) {
                    n.width = r.width().max(1.0);
                    n.height = r.height().max(1.0);
                }
            } else if let Some(n_child) = child.node(&id) {
                if let Some(n) = graph.node_mut(&id) {
                    n.width = n_child.width.max(1.0);
                    n.height = n_child.height.max(1.0);
                }
            }
            extracted.insert(id, child);
        }

        // Mermaid `recursiveRender` injects the parent cluster node into the child graph and
        // assigns it as the parent of nodes without an existing parent.
        if let Some(cluster_id) = graph_cluster_id {
            if !graph.has_node(cluster_id) {
                let lbl = cluster_node_labels
                    .get(cluster_id)
                    .cloned()
                    .unwrap_or_default();
                graph.set_node(cluster_id.to_string(), lbl);
            }
            let node_ids = graph.node_ids();
            for node_id in node_ids {
                if node_id == cluster_id {
                    continue;
                }
                if graph.parent(&node_id).is_none() {
                    graph.set_parent(node_id, cluster_id.to_string());
                }
            }
        }

        dugong::layout_dagreish(graph);
        apply_mermaid_subgraph_title_shifts(graph, extracted, subgraph_id_set, y_shift);
    }

    layout_graph_with_recursive_clusters(
        &mut g,
        None,
        &mut extracted_graphs,
        0,
        &subgraph_id_set,
        y_shift,
        &cluster_node_labels,
        title_total_margin,
    );

    let mut leaf_rects: std::collections::HashMap<String, Rect> = std::collections::HashMap::new();
    let mut base_pos: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();
    let mut edge_override_points: std::collections::HashMap<String, Vec<LayoutPoint>> =
        std::collections::HashMap::new();
    let mut edge_override_label: std::collections::HashMap<String, Option<LayoutLabel>> =
        std::collections::HashMap::new();
    let mut edge_override_from_cluster: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let mut edge_override_to_cluster: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    #[cfg(feature = "flowchart_root_pack")]
    let mut edge_packed_shift: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();
    #[cfg(not(feature = "flowchart_root_pack"))]
    let edge_packed_shift: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();

    let mut leaf_node_ids: std::collections::HashSet<String> = model
        .nodes
        .iter()
        .filter(|n| !subgraph_ids.contains(n.id.as_str()))
        .map(|n| n.id.clone())
        .collect();
    for id in &self_loop_label_node_ids {
        leaf_node_ids.insert(id.clone());
    }
    for id in &empty_subgraph_ids {
        leaf_node_ids.insert(id.clone());
    }

    fn place_graph(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        offset: (f64, f64),
        is_root: bool,
        edge_id_by_key: &std::collections::HashMap<String, String>,
        extracted_graphs: &std::collections::HashMap<
            String,
            Graph<NodeLabel, EdgeLabel, GraphLabel>,
        >,
        subgraph_ids: &std::collections::HashSet<&str>,
        leaf_node_ids: &std::collections::HashSet<String>,
        title_total_margin: f64,
        base_pos: &mut std::collections::HashMap<String, (f64, f64)>,
        leaf_rects: &mut std::collections::HashMap<String, Rect>,
        cluster_rects_from_graph: &mut std::collections::HashMap<String, Rect>,
        extracted_cluster_rects: &mut std::collections::HashMap<String, Rect>,
        edge_override_points: &mut std::collections::HashMap<String, Vec<LayoutPoint>>,
        edge_override_label: &mut std::collections::HashMap<String, Option<LayoutLabel>>,
        edge_override_from_cluster: &mut std::collections::HashMap<String, Option<String>>,
        edge_override_to_cluster: &mut std::collections::HashMap<String, Option<String>>,
    ) {
        for id in graph.node_ids() {
            let Some(n) = graph.node(&id) else { continue };
            let x = n.x.unwrap_or(0.0) + offset.0;
            let y = n.y.unwrap_or(0.0) + offset.1;
            if leaf_node_ids.contains(&id) {
                base_pos.insert(id.clone(), (x, y));
                leaf_rects.insert(id, Rect::from_center(x, y, n.width, n.height));
                continue;
            }
        }

        fn subtree_rect(
            graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            id: &str,
            visiting: &mut std::collections::HashSet<String>,
        ) -> Option<Rect> {
            if !visiting.insert(id.to_string()) {
                return None;
            }
            let mut out: Option<Rect> = None;
            for child in graph.children(id) {
                if let Some(n) = graph.node(child) {
                    if let (Some(x), Some(y)) = (n.x, n.y) {
                        let r = Rect::from_center(x, y, n.width, n.height);
                        if let Some(ref mut cur) = out {
                            cur.union(r);
                        } else {
                            out = Some(r);
                        }
                    }
                }
                if !graph.children(child).is_empty() {
                    if let Some(r) = subtree_rect(graph, child, visiting) {
                        if let Some(ref mut cur) = out {
                            cur.union(r);
                        } else {
                            out = Some(r);
                        }
                    }
                }
            }
            visiting.remove(id);
            out
        }

        // Capture the layout-computed compound bounds for non-extracted clusters.
        //
        // Upstream Dagre computes compound-node geometry from border nodes and then removes the
        // border dummy nodes (`removeBorderNodes`). Our dugong parity pipeline mirrors that, so
        // prefer the compound node's own x/y/width/height when available.
        for id in graph.node_ids() {
            if !subgraph_ids.contains(id.as_str()) {
                continue;
            }
            if extracted_graphs.contains_key(&id) {
                continue;
            }
            if cluster_rects_from_graph.contains_key(&id) {
                continue;
            }
            if let Some(n) = graph.node(&id) {
                if let (Some(x), Some(y)) = (n.x, n.y) {
                    if n.width > 0.0 && n.height > 0.0 {
                        let mut r = Rect::from_center(x, y, n.width, n.height);
                        r.translate(offset.0, offset.1);
                        cluster_rects_from_graph.insert(id, r);
                        continue;
                    }
                }
            }

            let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();
            let Some(mut r) = subtree_rect(graph, &id, &mut visiting) else {
                continue;
            };
            r.translate(offset.0, offset.1);
            cluster_rects_from_graph.insert(id, r);
        }

        for ek in graph.edge_keys() {
            let Some(edge_key) = ek.name.as_deref() else {
                continue;
            };
            let edge_id = edge_id_by_key
                .get(edge_key)
                .map(String::as_str)
                .unwrap_or(edge_key);
            let Some(lbl) = graph.edge_by_key(&ek) else {
                continue;
            };

            if let (Some(x), Some(y)) = (lbl.x, lbl.y) {
                if lbl.width > 0.0 || lbl.height > 0.0 {
                    let lx = x + offset.0;
                    let ly = y + offset.1;
                    let leaf_id = format!("edge-label::{edge_id}");
                    base_pos.insert(leaf_id.clone(), (lx, ly));
                    leaf_rects.insert(leaf_id, Rect::from_center(lx, ly, lbl.width, lbl.height));
                }
            }

            if !is_root {
                let points = lbl
                    .points
                    .iter()
                    .map(|p| LayoutPoint {
                        x: p.x + offset.0,
                        y: p.y + offset.1,
                    })
                    .collect::<Vec<_>>();
                let label_pos = match (lbl.x, lbl.y) {
                    (Some(x), Some(y)) if lbl.width > 0.0 || lbl.height > 0.0 => {
                        Some(LayoutLabel {
                            x: x + offset.0,
                            y: y + offset.1,
                            width: lbl.width,
                            height: lbl.height,
                        })
                    }
                    _ => None,
                };
                edge_override_points.insert(edge_id.to_string(), points);
                edge_override_label.insert(edge_id.to_string(), label_pos);
                let from_cluster = lbl
                    .extras
                    .get("fromCluster")
                    .and_then(|v| v.as_str().map(|s| s.to_string()));
                let to_cluster = lbl
                    .extras
                    .get("toCluster")
                    .and_then(|v| v.as_str().map(|s| s.to_string()));
                edge_override_from_cluster.insert(edge_id.to_string(), from_cluster);
                edge_override_to_cluster.insert(edge_id.to_string(), to_cluster);
            }
        }

        for id in graph.node_ids() {
            // Only recurse into extracted graphs for leaf cluster nodes ("clusterNode" in Mermaid).
            // The recursively rendered graph itself also contains a node with the same id (the
            // parent cluster node injected before layout), which has children and must not recurse.
            if !graph.children(&id).is_empty() {
                continue;
            }
            let Some(child) = extracted_graphs.get(&id) else {
                continue;
            };
            let Some(n) = graph.node(&id) else {
                continue;
            };
            let (Some(px), Some(py)) = (n.x, n.y) else {
                continue;
            };
            let parent_x = px + offset.0;
            let parent_y = py + offset.1;
            let Some(cnode) = child.node(&id) else {
                continue;
            };
            let (Some(cx), Some(cy)) = (cnode.x, cnode.y) else {
                continue;
            };
            let child_offset = (parent_x - cx, parent_y - cy);
            // The extracted cluster's footprint in the parent graph is the clusterNode itself.
            // Our recursive layout step updates the parent graph's node `width/height` to match
            // Mermaid's `updateNodeBounds(...)` behavior (including any title margin). Avoid
            // adding `title_total_margin` again here.
            let r = Rect::from_center(parent_x, parent_y, n.width, n.height);
            extracted_cluster_rects.insert(id.clone(), r);
            place_graph(
                child,
                child_offset,
                false,
                edge_id_by_key,
                extracted_graphs,
                subgraph_ids,
                leaf_node_ids,
                title_total_margin,
                base_pos,
                leaf_rects,
                cluster_rects_from_graph,
                extracted_cluster_rects,
                edge_override_points,
                edge_override_label,
                edge_override_from_cluster,
                edge_override_to_cluster,
            );
        }
    }

    let mut cluster_rects_from_graph: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut extracted_cluster_rects: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    place_graph(
        &g,
        (0.0, 0.0),
        true,
        &edge_id_by_key,
        &extracted_graphs,
        &subgraph_ids,
        &leaf_node_ids,
        title_total_margin,
        &mut base_pos,
        &mut leaf_rects,
        &mut cluster_rects_from_graph,
        &mut extracted_cluster_rects,
        &mut edge_override_points,
        &mut edge_override_label,
        &mut edge_override_from_cluster,
        &mut edge_override_to_cluster,
    );

    let mut extra_children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let labeled_edges: std::collections::HashSet<&str> = render_edges
        .iter()
        .filter(|e| edge_label_is_non_empty(e))
        .map(|e| e.id.as_str())
        .collect();

    fn collect_extra_children(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        edge_id_by_key: &std::collections::HashMap<String, String>,
        labeled_edges: &std::collections::HashSet<&str>,
        implicit_root: Option<&str>,
        out: &mut std::collections::HashMap<String, Vec<String>>,
    ) {
        for ek in graph.edge_keys() {
            let Some(edge_key) = ek.name.as_deref() else {
                continue;
            };
            let edge_id = edge_id_by_key
                .get(edge_key)
                .map(String::as_str)
                .unwrap_or(edge_key);
            if !labeled_edges.contains(edge_id) {
                continue;
            }
            // Mermaid's recursive cluster extractor removes the root cluster node from the
            // extracted graph. In that case, the "lowest common parent" of edges whose endpoints
            // belong to the extracted cluster becomes `None`, even though the label should still
            // participate in the extracted cluster's bounding box. Use `implicit_root` to map
            // those labels back to the extracted cluster id.
            let parent = lowest_common_parent(graph, &ek.v, &ek.w)
                .or_else(|| implicit_root.map(|s| s.to_string()));
            let Some(parent) = parent else {
                continue;
            };
            out.entry(parent)
                .or_default()
                .push(format!("edge-label::{edge_id}"));
        }
    }

    collect_extra_children(
        &g,
        &edge_id_by_key,
        &labeled_edges,
        None,
        &mut extra_children,
    );
    for (cluster_id, cg) in &extracted_graphs {
        collect_extra_children(
            cg,
            &edge_id_by_key,
            &labeled_edges,
            Some(cluster_id.as_str()),
            &mut extra_children,
        );
    }

    // Ensure Mermaid-style self-loop helper nodes participate in cluster bounding/packing.
    // These nodes are not part of the semantic `subgraph ... end` membership list, but are
    // parented into the same clusters as their base node.
    for id in &self_loop_label_node_ids {
        if let Some(p) = g.parent(id) {
            extra_children
                .entry(p.to_string())
                .or_default()
                .push(id.clone());
        }
    }

    // Mermaid does not apply an extra post-layout packing step for disconnected subgraphs.
    // Keep the experimental packing logic behind a feature flag for debugging only.
    #[cfg(feature = "flowchart_root_pack")]
    {
        let subgraph_ids: std::collections::HashSet<&str> =
            model.subgraphs.iter().map(|s| s.id.as_str()).collect();

        let mut subgraph_has_parent: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for sg in &model.subgraphs {
            for child in &sg.nodes {
                if subgraph_ids.contains(child.as_str()) {
                    subgraph_has_parent.insert(child.as_str());
                }
            }
        }

        fn collect_descendant_leaf_nodes<'a>(
            id: &'a str,
            subgraphs_by_id: &'a std::collections::HashMap<String, FlowSubgraph>,
            subgraph_ids: &std::collections::HashSet<&'a str>,
            out: &mut std::collections::HashSet<String>,
            visiting: &mut std::collections::HashSet<&'a str>,
        ) {
            if !visiting.insert(id) {
                return;
            }
            let Some(sg) = subgraphs_by_id.get(id) else {
                visiting.remove(id);
                return;
            };
            for member in &sg.nodes {
                if subgraph_ids.contains(member.as_str()) {
                    collect_descendant_leaf_nodes(
                        member,
                        subgraphs_by_id,
                        subgraph_ids,
                        out,
                        visiting,
                    );
                } else {
                    out.insert(member.clone());
                }
            }
            visiting.remove(id);
        }

        fn collect_descendant_cluster_ids<'a>(
            id: &'a str,
            subgraphs_by_id: &'a std::collections::HashMap<String, FlowSubgraph>,
            subgraph_ids: &std::collections::HashSet<&'a str>,
            out: &mut std::collections::HashSet<String>,
            visiting: &mut std::collections::HashSet<&'a str>,
        ) {
            if !visiting.insert(id) {
                return;
            }
            let Some(sg) = subgraphs_by_id.get(id) else {
                visiting.remove(id);
                return;
            };
            out.insert(id.to_string());
            for member in &sg.nodes {
                if subgraph_ids.contains(member.as_str()) {
                    collect_descendant_cluster_ids(
                        member,
                        subgraphs_by_id,
                        subgraph_ids,
                        out,
                        visiting,
                    );
                }
            }
            visiting.remove(id);
        }

        fn has_external_edges(
            leaves: &std::collections::HashSet<String>,
            edges: &[FlowEdge],
        ) -> bool {
            for e in edges {
                let in_from = leaves.contains(&e.from);
                let in_to = leaves.contains(&e.to);
                if in_from ^ in_to {
                    return true;
                }
            }
            false
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum PackAxis {
            X,
            Y,
        }

        let pack_axis = match diagram_direction.as_str() {
            "LR" | "RL" => PackAxis::Y,
            _ => PackAxis::X,
        };

        let mut pack_rects: std::collections::HashMap<String, Rect> =
            std::collections::HashMap::new();
        let mut pack_visiting: std::collections::HashSet<String> = std::collections::HashSet::new();

        fn compute_pack_rect(
            id: &str,
            subgraphs_by_id: &std::collections::HashMap<String, FlowSubgraph>,
            leaf_rects: &std::collections::HashMap<String, Rect>,
            extra_children: &std::collections::HashMap<String, Vec<String>>,
            extracted_cluster_rects: &std::collections::HashMap<String, Rect>,
            pack_rects: &mut std::collections::HashMap<String, Rect>,
            pack_visiting: &mut std::collections::HashSet<String>,
            measurer: &dyn TextMeasurer,
            text_style: &TextStyle,
            title_wrapping_width: f64,
            wrap_mode: WrapMode,
            cluster_padding: f64,
            title_total_margin: f64,
            node_padding: f64,
        ) -> Option<Rect> {
            if let Some(r) = pack_rects.get(id).copied() {
                return Some(r);
            }
            if !pack_visiting.insert(id.to_string()) {
                return None;
            }
            if let Some(r) = extracted_cluster_rects.get(id).copied() {
                pack_visiting.remove(id);
                pack_rects.insert(id.to_string(), r);
                return Some(r);
            }
            let Some(sg) = subgraphs_by_id.get(id) else {
                pack_visiting.remove(id);
                return None;
            };

            let mut content: Option<Rect> = None;
            for member in &sg.nodes {
                let member_rect = if let Some(r) = leaf_rects.get(member).copied() {
                    Some(r)
                } else if subgraphs_by_id.contains_key(member) {
                    compute_pack_rect(
                        member,
                        subgraphs_by_id,
                        leaf_rects,
                        extra_children,
                        extracted_cluster_rects,
                        pack_rects,
                        pack_visiting,
                        measurer,
                        text_style,
                        title_wrapping_width,
                        wrap_mode,
                        cluster_padding,
                        title_total_margin,
                        node_padding,
                    )
                } else {
                    None
                };

                if let Some(r) = member_rect {
                    if let Some(ref mut cur) = content {
                        cur.union(r);
                    } else {
                        content = Some(r);
                    }
                }
            }

            if let Some(extra) = extra_children.get(id) {
                for child in extra {
                    if let Some(r) = leaf_rects.get(child).copied() {
                        if let Some(ref mut cur) = content {
                            cur.union(r);
                        } else {
                            content = Some(r);
                        }
                    }
                }
            }

            let label_type = sg.label_type.as_deref().unwrap_or("text");
            let title_width_limit = Some(title_wrapping_width);
            let title_metrics = flowchart_label_metrics_for_layout(
                measurer,
                &sg.title,
                label_type,
                text_style,
                title_width_limit,
                wrap_mode,
            );

            let mut rect = if let Some(r) = content {
                r
            } else {
                Rect::from_center(
                    0.0,
                    0.0,
                    title_metrics.width.max(1.0),
                    title_metrics.height.max(1.0),
                )
            };

            rect.min_x -= cluster_padding;
            rect.max_x += cluster_padding;
            rect.min_y -= cluster_padding;
            rect.max_y += cluster_padding;

            // Mermaid cluster "rect" rendering widens to fit the raw title bbox, plus a small
            // horizontal inset. Empirically (Mermaid@11.12.2 fixtures), this behaves like
            // `title_width + cluster_padding` when the title is wider than the content.
            let min_width = title_metrics.width.max(1.0) + cluster_padding;
            if rect.width() < min_width {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, min_width, rect.height());
            }

            if title_total_margin > 0.0 {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), rect.height() + title_total_margin);
            }

            let min_height = title_metrics.height.max(1.0) + title_total_margin;
            if rect.height() < min_height {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), min_height);
            }

            pack_visiting.remove(id);
            pack_rects.insert(id.to_string(), rect);
            Some(rect)
        }

        struct PackItem {
            rect: Rect,
            members: Vec<String>,
            internal_edge_ids: Vec<String>,
            cluster_ids: Vec<String>,
        }

        let mut items: Vec<PackItem> = Vec::new();
        for sg in &model.subgraphs {
            if subgraph_has_parent.contains(sg.id.as_str()) {
                continue;
            }

            let mut leaves: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut visiting: std::collections::HashSet<&str> = std::collections::HashSet::new();
            collect_descendant_leaf_nodes(
                &sg.id,
                &subgraphs_by_id,
                &subgraph_ids,
                &mut leaves,
                &mut visiting,
            );
            if leaves.is_empty() {
                continue;
            }

            // Treat cluster ids as part of the pack-membership boundary when detecting external
            // edges. Mermaid flowcharts allow edges to reference subgraph ids directly (cluster
            // nodes). If we only consider leaf nodes, edges like `X --> Y` would incorrectly mark
            // both top-level clusters as "isolated" and the packing step would separate them,
            // diverging from Mermaid's Dagre layout.
            let mut cluster_ids_set: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            let mut cluster_visiting: std::collections::HashSet<&str> =
                std::collections::HashSet::new();
            collect_descendant_cluster_ids(
                &sg.id,
                &subgraphs_by_id,
                &subgraph_ids,
                &mut cluster_ids_set,
                &mut cluster_visiting,
            );

            let mut membership_endpoints: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            membership_endpoints.extend(leaves.iter().cloned());
            membership_endpoints.extend(cluster_ids_set.iter().cloned());

            if has_external_edges(&membership_endpoints, &render_edges) {
                continue;
            }

            let Some(rect) = compute_pack_rect(
                &sg.id,
                &subgraphs_by_id,
                &leaf_rects,
                &extra_children,
                &extracted_cluster_rects,
                &mut pack_rects,
                &mut pack_visiting,
                measurer,
                &text_style,
                cluster_title_wrapping_width,
                node_wrap_mode,
                cluster_padding,
                title_total_margin,
                node_padding,
            ) else {
                continue;
            };

            let mut members = leaves.iter().cloned().collect::<Vec<_>>();
            if let Some(extra) = extra_children.get(&sg.id) {
                members.extend(extra.iter().cloned());
            }

            // Ensure internal labeled edge nodes participate in translation.
            let mut internal_edge_ids: Vec<String> = Vec::new();
            for e in &render_edges {
                if leaves.contains(&e.from) && leaves.contains(&e.to) {
                    internal_edge_ids.push(e.id.clone());
                    if edge_label_is_non_empty(e) {
                        members.push(edge_label_leaf_id(e));
                    }
                }
            }

            let mut cluster_ids = cluster_ids_set.into_iter().collect::<Vec<_>>();
            cluster_ids.sort();

            items.push(PackItem {
                rect,
                members,
                internal_edge_ids,
                cluster_ids,
            });
        }

        if !items.is_empty() {
            items.sort_by(|a, b| match pack_axis {
                PackAxis::X => a.rect.min_x.total_cmp(&b.rect.min_x),
                PackAxis::Y => a.rect.min_y.total_cmp(&b.rect.min_y),
            });

            let mut cursor = match pack_axis {
                PackAxis::X => items.first().unwrap().rect.min_x,
                PackAxis::Y => items.first().unwrap().rect.min_y,
            };

            for item in items {
                let (cx, cy) = item.rect.center();
                let desired_center = match pack_axis {
                    PackAxis::X => cursor + item.rect.width() / 2.0,
                    PackAxis::Y => cursor + item.rect.height() / 2.0,
                };
                let (dx, dy) = match pack_axis {
                    PackAxis::X => (desired_center - cx, 0.0),
                    PackAxis::Y => (0.0, desired_center - cy),
                };

                if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
                    for id in &item.members {
                        if let Some((x, y)) = base_pos.get_mut(id) {
                            *x += dx;
                            *y += dy;
                        }
                        if let Some(r) = leaf_rects.get_mut(id) {
                            r.translate(dx, dy);
                        }
                    }
                    for cid in &item.cluster_ids {
                        if let Some(r) = extracted_cluster_rects.get_mut(cid) {
                            r.translate(dx, dy);
                        }
                        if let Some(r) = cluster_rects_from_graph.get_mut(cid) {
                            r.translate(dx, dy);
                        }
                    }
                    for edge_id in &item.internal_edge_ids {
                        edge_packed_shift.insert(edge_id.clone(), (dx, dy));
                    }
                }

                cursor += match pack_axis {
                    PackAxis::X => item.rect.width() + nodesep,
                    PackAxis::Y => item.rect.height() + nodesep,
                };
            }
        }
    }

    let mut out_nodes: Vec<LayoutNode> = Vec::new();
    for n in &model.nodes {
        if subgraph_ids.contains(n.id.as_str()) {
            continue;
        }
        let (x, y) = base_pos
            .get(&n.id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {}", n.id),
            })?;
        let (width, height) = leaf_rects
            .get(&n.id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {}", n.id),
            })?;
        out_nodes.push(LayoutNode {
            id: n.id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
        });
    }
    for id in &empty_subgraph_ids {
        let (x, y) = base_pos
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
        });
    }
    for id in &self_loop_label_node_ids {
        let (x, y) = base_pos
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
        });
    }

    let mut clusters: Vec<LayoutCluster> = Vec::new();

    let mut cluster_rects: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut cluster_base_widths: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();

    fn compute_cluster_rect(
        id: &str,
        subgraphs_by_id: &std::collections::HashMap<String, FlowSubgraph>,
        leaf_rects: &std::collections::HashMap<String, Rect>,
        extra_children: &std::collections::HashMap<String, Vec<String>>,
        cluster_rects: &mut std::collections::HashMap<String, Rect>,
        cluster_base_widths: &mut std::collections::HashMap<String, f64>,
        visiting: &mut std::collections::HashSet<String>,
        measurer: &dyn TextMeasurer,
        text_style: &TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        cluster_padding: f64,
        title_total_margin: f64,
        node_padding: f64,
    ) -> Result<(Rect, f64)> {
        if let Some(r) = cluster_rects.get(id).copied() {
            let base_width = cluster_base_widths
                .get(id)
                .copied()
                .unwrap_or_else(|| r.width());
            return Ok((r, base_width));
        }
        if !visiting.insert(id.to_string()) {
            return Err(Error::InvalidModel {
                message: format!("cycle in subgraph membership involving {id}"),
            });
        }

        let Some(sg) = subgraphs_by_id.get(id) else {
            return Err(Error::InvalidModel {
                message: format!("missing subgraph definition for {id}"),
            });
        };

        let mut content: Option<Rect> = None;
        for member in &sg.nodes {
            let member_rect = if let Some(r) = leaf_rects.get(member).copied() {
                Some(r)
            } else if subgraphs_by_id.contains_key(member) {
                Some(
                    compute_cluster_rect(
                        member,
                        subgraphs_by_id,
                        leaf_rects,
                        extra_children,
                        cluster_rects,
                        cluster_base_widths,
                        visiting,
                        measurer,
                        text_style,
                        title_wrapping_width,
                        wrap_mode,
                        cluster_padding,
                        title_total_margin,
                        node_padding,
                    )?
                    .0,
                )
            } else {
                None
            };

            if let Some(r) = member_rect {
                if let Some(ref mut cur) = content {
                    cur.union(r);
                } else {
                    content = Some(r);
                }
            }
        }

        if let Some(extra) = extra_children.get(id) {
            for child in extra {
                if let Some(r) = leaf_rects.get(child).copied() {
                    if let Some(ref mut cur) = content {
                        cur.union(r);
                    } else {
                        content = Some(r);
                    }
                }
            }
        }

        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let title_width_limit = Some(title_wrapping_width);
        let title_metrics = flowchart_label_metrics_for_layout(
            measurer,
            &sg.title,
            label_type,
            text_style,
            title_width_limit,
            wrap_mode,
        );
        let mut rect = if let Some(r) = content {
            r
        } else {
            Rect::from_center(
                0.0,
                0.0,
                title_metrics.width.max(1.0),
                title_metrics.height.max(1.0),
            )
        };

        // Expand to provide the cluster's internal padding.
        rect.min_x -= cluster_padding;
        rect.max_x += cluster_padding;
        rect.min_y -= cluster_padding;
        rect.max_y += cluster_padding;

        // Mermaid computes `node.diff` using the pre-widened layout node width, then may widen the
        // rect to fit the label bbox during rendering.
        let base_width = rect.width();

        // Mermaid cluster "rect" rendering widens to fit the raw title bbox, plus a small
        // horizontal inset. Empirically (Mermaid@11.12.2 fixtures), this behaves like
        // `title_width + cluster_padding` when the title is wider than the content.
        let min_width = title_metrics.width.max(1.0) + cluster_padding;
        if rect.width() < min_width {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, min_width, rect.height());
        }

        // Extend height to reserve space for subgraph title margins (Mermaid does this after layout).
        if title_total_margin > 0.0 {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), rect.height() + title_total_margin);
        }

        // Keep the cluster tall enough to accommodate the title bbox if needed.
        let min_height = title_metrics.height.max(1.0) + title_total_margin;
        if rect.height() < min_height {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), min_height);
        }

        visiting.remove(id);
        cluster_rects.insert(id.to_string(), rect);
        cluster_base_widths.insert(id.to_string(), base_width);
        Ok((rect, base_width))
    }

    for sg in &model.subgraphs {
        fn adjust_cluster_rect_for_title(
            mut rect: Rect,
            title: &str,
            label_type: &str,
            measurer: &dyn TextMeasurer,
            text_style: &TextStyle,
            title_wrapping_width: f64,
            wrap_mode: WrapMode,
            title_total_margin: f64,
            cluster_padding: f64,
            add_title_total_margin: bool,
        ) -> Rect {
            let title_width_limit = Some(title_wrapping_width);
            let title_metrics = flowchart_label_metrics_for_layout(
                measurer,
                title,
                label_type,
                text_style,
                title_width_limit,
                wrap_mode,
            );
            let title_w = title_metrics.width.max(1.0);
            let title_h = title_metrics.height.max(1.0);

            // Mermaid cluster "rect" widens to fit the raw title bbox (no added padding),
            // even when the cluster bounds come from Dagre border nodes.
            let min_w = title_w + cluster_padding;
            if rect.width() < min_w {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, min_w, rect.height());
            }

            // Mermaid adds `subGraphTitleTotalMargin` to cluster height after layout.
            if add_title_total_margin && title_total_margin > 0.0 {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), rect.height() + title_total_margin);
            }

            // Keep the cluster tall enough for the title bbox (including title margins).
            let min_h = title_h + title_total_margin;
            if rect.height() < min_h {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), min_h);
            }

            rect
        }

        if sg.nodes.is_empty() {
            continue;
        }

        let (rect, base_width) = if extracted_graphs.contains_key(&sg.id) {
            // For extracted (recursive) clusters, match Mermaid's `updateNodeBounds(...)` intent by
            // taking the rendered child-graph content bbox (including border nodes) as the cluster
            // node's bounds.
            let rect = extracted_cluster_rects
                .get(&sg.id)
                .copied()
                .unwrap_or_else(|| {
                    compute_cluster_rect(
                        &sg.id,
                        &subgraphs_by_id,
                        &leaf_rects,
                        &extra_children,
                        &mut cluster_rects,
                        &mut cluster_base_widths,
                        &mut visiting,
                        measurer,
                        &text_style,
                        cluster_title_wrapping_width,
                        node_wrap_mode,
                        cluster_padding,
                        title_total_margin,
                        node_padding,
                    )
                    .map(|v| v.0)
                    .unwrap_or_else(|_| Rect::from_center(0.0, 0.0, 1.0, 1.0))
                });
            let base_width = rect.width();
            let rect = adjust_cluster_rect_for_title(
                rect,
                &sg.title,
                sg.label_type.as_deref().unwrap_or("text"),
                measurer,
                &text_style,
                cluster_title_wrapping_width,
                node_wrap_mode,
                title_total_margin,
                cluster_padding,
                false,
            );
            (rect, base_width)
        } else if let Some(r) = cluster_rects_from_graph.get(&sg.id).copied() {
            let base_width = r.width();
            let rect = adjust_cluster_rect_for_title(
                r,
                &sg.title,
                sg.label_type.as_deref().unwrap_or("text"),
                measurer,
                &text_style,
                cluster_title_wrapping_width,
                node_wrap_mode,
                title_total_margin,
                cluster_padding,
                true,
            );
            (rect, base_width)
        } else {
            compute_cluster_rect(
                &sg.id,
                &subgraphs_by_id,
                &leaf_rects,
                &extra_children,
                &mut cluster_rects,
                &mut cluster_base_widths,
                &mut visiting,
                measurer,
                &text_style,
                cluster_title_wrapping_width,
                node_wrap_mode,
                cluster_padding,
                title_total_margin,
                node_padding,
            )?
        };
        let (cx, cy) = rect.center();

        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let title_width_limit = Some(cluster_title_wrapping_width);
        let title_metrics = flowchart_label_metrics_for_layout(
            measurer,
            &sg.title,
            label_type,
            &text_style,
            title_width_limit,
            node_wrap_mode,
        );
        let title_label = LayoutLabel {
            x: cx,
            y: cy - rect.height() / 2.0 + title_margin_top + title_metrics.height / 2.0,
            width: title_metrics.width,
            height: title_metrics.height,
        };

        // `dagre-wrapper/clusters.js` (shape `rect`) sets `padding = 0 * node.padding`.
        // The cluster label is positioned at `node.x - bbox.width/2`, and `node.diff` is:
        // - `(bbox.width - node.width)/2 - node.padding/2` when the box widens to fit the title
        // - otherwise `-node.padding/2`.
        let title_w = title_metrics.width.max(1.0);
        let diff = if base_width <= title_w {
            (title_w - base_width) / 2.0 - cluster_padding / 2.0
        } else {
            -cluster_padding / 2.0
        };
        let offset_y = title_metrics.height - cluster_padding / 2.0;

        let effective_dir = effective_dir_by_id
            .get(&sg.id)
            .cloned()
            .unwrap_or_else(|| effective_cluster_dir(sg, &diagram_direction, inherit_dir));

        clusters.push(LayoutCluster {
            id: sg.id.clone(),
            x: cx,
            y: cy,
            width: rect.width(),
            height: rect.height(),
            diff,
            offset_y,
            title: sg.title.clone(),
            title_label,
            requested_dir: sg.dir.as_ref().map(|s| normalize_dir(s)),
            effective_dir,
            padding: cluster_padding,
            title_margin_top,
            title_margin_bottom,
        });

        out_nodes.push(LayoutNode {
            id: sg.id.clone(),
            x: cx,
            // Mermaid does not shift pure cluster nodes by `subGraphTitleTotalMargin / 2`.
            y: cy,
            width: rect.width(),
            height: rect.height(),
            is_cluster: true,
        });
    }
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edges: Vec<LayoutEdge> = Vec::new();
    for e in &render_edges {
        let (dx, dy) = edge_packed_shift.get(&e.id).copied().unwrap_or((0.0, 0.0));
        let (
            mut points,
            mut label_pos,
            label_pos_already_shifted,
            mut from_cluster,
            mut to_cluster,
        ) = if let Some(points) = edge_override_points.get(&e.id) {
            let from_cluster = edge_override_from_cluster
                .get(&e.id)
                .cloned()
                .unwrap_or(None);
            let to_cluster = edge_override_to_cluster.get(&e.id).cloned().unwrap_or(None);
            (
                points.clone(),
                edge_override_label.get(&e.id).cloned().unwrap_or(None),
                false,
                from_cluster,
                to_cluster,
            )
        } else {
            let (v, w) = edge_endpoints_by_id
                .get(&e.id)
                .cloned()
                .unwrap_or_else(|| (e.from.clone(), e.to.clone()));
            let edge_key = edge_key_by_id
                .get(&e.id)
                .map(String::as_str)
                .unwrap_or(e.id.as_str());
            let Some(label) = g.edge(&v, &w, Some(edge_key)) else {
                return Err(Error::InvalidModel {
                    message: format!("missing layout edge {}", e.id),
                });
            };
            let from_cluster = label
                .extras
                .get("fromCluster")
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            let to_cluster = label
                .extras
                .get("toCluster")
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            let points = label
                .points
                .iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect::<Vec<_>>();
            let label_pos = match (label.x, label.y) {
                (Some(x), Some(y)) if label.width > 0.0 || label.height > 0.0 => {
                    Some(LayoutLabel {
                        x,
                        y,
                        width: label.width,
                        height: label.height,
                    })
                }
                _ => None,
            };
            (points, label_pos, false, from_cluster, to_cluster)
        };

        // Match Mermaid's dagre adapter: self-loop special edges on group nodes are annotated with
        // `fromCluster` / `toCluster` so downstream renderers can clip routes to the cluster
        // boundary.
        if subgraph_ids.contains(e.from.as_str()) && e.id.ends_with("-cyclic-special-1") {
            from_cluster = Some(e.from.clone());
        }
        if subgraph_ids.contains(e.to.as_str()) && e.id.ends_with("-cyclic-special-2") {
            to_cluster = Some(e.to.clone());
        }

        if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
            for p in &mut points {
                p.x += dx;
                p.y += dy;
            }
            if !label_pos_already_shifted {
                if let Some(l) = label_pos.as_mut() {
                    l.x += dx;
                    l.y += dy;
                }
            }
        }
        out_edges.push(LayoutEdge {
            id: e.id.clone(),
            from: e.from.clone(),
            to: e.to.clone(),
            from_cluster,
            to_cluster,
            points,
            label: label_pos,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }

    // Mermaid's flowchart renderer uses shape-specific intersection functions for edge endpoints
    // (e.g. diamond nodes). Our Dagre-ish layout currently treats all nodes as rectangles, so the
    // first/last points can land on the bounding box rather than the actual polygon boundary.
    //
    // Adjust the first/last edge points to match Mermaid's shape intersection behavior for the
    // shapes that materially differ from rectangles.
    let mut node_shape_by_id: HashMap<&str, &str> = HashMap::new();
    for n in &model.nodes {
        if let Some(s) = n.layout_shape.as_deref() {
            node_shape_by_id.insert(n.id.as_str(), s);
        }
    }
    let mut layout_node_by_id: HashMap<&str, &LayoutNode> = HashMap::new();
    for n in &out_nodes {
        layout_node_by_id.insert(n.id.as_str(), n);
    }

    fn diamond_intersection(node: &LayoutNode, toward: &LayoutPoint) -> Option<LayoutPoint> {
        let vx = toward.x - node.x;
        let vy = toward.y - node.y;
        if !(vx.is_finite() && vy.is_finite()) {
            return None;
        }
        if vx.abs() <= 1e-12 && vy.abs() <= 1e-12 {
            return None;
        }
        let hw = (node.width / 2.0).max(1e-9);
        let hh = (node.height / 2.0).max(1e-9);
        let denom = vx.abs() / hw + vy.abs() / hh;
        if !(denom.is_finite() && denom > 0.0) {
            return None;
        }
        let t = 1.0 / denom;
        Some(LayoutPoint {
            x: node.x + vx * t,
            y: node.y + vy * t,
        })
    }

    for e in &mut out_edges {
        if e.points.len() < 2 {
            continue;
        }

        if let Some(node) = layout_node_by_id.get(e.from.as_str()) {
            if !node.is_cluster {
                let shape = node_shape_by_id
                    .get(e.from.as_str())
                    .copied()
                    .unwrap_or("squareRect");
                if matches!(shape, "diamond" | "question" | "diam") {
                    if let Some(p) = diamond_intersection(node, &e.points[1]) {
                        e.points[0] = p;
                    }
                }
            }
        }
        if let Some(node) = layout_node_by_id.get(e.to.as_str()) {
            if !node.is_cluster {
                let shape = node_shape_by_id
                    .get(e.to.as_str())
                    .copied()
                    .unwrap_or("squareRect");
                if matches!(shape, "diamond" | "question" | "diam") {
                    let n = e.points.len();
                    if let Some(p) = diamond_intersection(node, &e.points[n - 2]) {
                        e.points[n - 1] = p;
                    }
                }
            }
        }
    }

    let bounds = compute_bounds(&out_nodes, &out_edges);

    Ok(FlowchartV2Layout {
        nodes: out_nodes,
        edges: out_edges,
        clusters,
        bounds,
    })
}

fn node_render_dimensions(
    layout_shape: Option<&str>,
    metrics: crate::text::TextMetrics,
    padding: f64,
) -> (f64, f64) {
    // This function mirrors Mermaid `@11.12.2` node shape sizing rules at the "rendering-elements"
    // layer, but uses our headless `TextMeasurer` metrics instead of DOM `getBBox()`.
    //
    // References:
    // - `packages/mermaid/src/diagrams/flowchart/flowDb.ts` (shape assignment + padding)
    // - `packages/mermaid/src/rendering-util/rendering-elements/shapes/*.ts` (shape bounds)
    // Mermaid's DOM `getBBox()` can legitimately return 0 for empty/whitespace-only labels.
    // Do not clamp to 1px here, otherwise we skew layout widths (notably `max-width`) by 1px.
    let text_w = metrics.width.max(0.0);
    let text_h = metrics.height.max(0.0);
    let p = padding.max(0.0);

    let shape = layout_shape.unwrap_or("squareRect");

    match shape {
        // Default flowchart process node.
        "squareRect" => (text_w + 4.0 * p, text_h + 2.0 * p),

        // Mermaid uses a few aliases for the same rounded-rectangle shape across layers.
        // In FlowDB output (flowchart-v2), this commonly appears as `rounded`.
        "roundedRect" | "rounded" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Diamond (decision/question).
        "diamond" | "question" | "diam" => {
            let w = text_w + p;
            let h = text_h + p;
            let s = w + h;
            (s, s)
        }

        // Hexagon.
        "hexagon" | "hex" => {
            let h = text_h + p;
            let w0 = text_w + 2.5 * p;
            // The current Mermaid implementation expands the half-width by `m = (w/2)/6`,
            // resulting in a total width of `7/6 * w`.
            (w0 * (7.0 / 6.0), h)
        }

        // Stadium/terminator.
        "stadium" => {
            let h = text_h + p;
            let w = text_w + h / 4.0 + p;
            (w, h)
        }

        // Subroutine (framed rectangle): adds an 8px "frame" on both sides.
        "subroutine" | "fr-rect" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + 16.0, h)
        }

        // Cylinder/database.
        "cylinder" | "cyl" => {
            let w = text_w + p;
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            // Mermaid's cylinder path height ends up including two extra `ry` from the ellipses.
            // See `createCylinderPathD` + `translate(..., -(h/2 + ry))`.
            let height = text_h + p + 3.0 * ry;
            (w, height)
        }

        // Circle.
        "circle" | "circ" => {
            // Mermaid uses half-padding for circles and bases radius on label width.
            let d = text_w + p;
            (d, d)
        }

        // Double circle.
        "doublecircle" | "dbl-circ" => {
            // `gap = 5` is hard-coded in Mermaid.
            let d = text_w + p + 10.0;
            (d, d)
        }

        // Lean and trapezoid variants (parallelograms/trapezoids).
        "lean_right" | "lean-r" | "lean-right" | "lean_left" | "lean-l" | "lean-left"
        | "trapezoid" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + h, h)
        }

        // Inverted trapezoid uses `2 * padding` on both axes in Mermaid.
        "inv_trapezoid" | "inv-trapezoid" => {
            let w = text_w + 2.0 * p;
            let h = text_h + 2.0 * p;
            (w + h, h)
        }

        // Odd node (`>... ]`) is rendered using `rect_left_inv_arrow`.
        "odd" | "rect_left_inv_arrow" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + h / 4.0, h)
        }

        // Ellipses are currently broken upstream but still emitted by FlowDB.
        // Keep a reasonable headless size for layout stability.
        "ellipse" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Fallback: treat unknown shapes as default rectangles.
        _ => (text_w + 4.0 * p, text_h + 2.0 * p),
    }
}

pub(crate) fn flowchart_node_render_dimensions(
    layout_shape: Option<&str>,
    metrics: crate::text::TextMetrics,
    padding: f64,
) -> (f64, f64) {
    node_render_dimensions(layout_shape, metrics, padding)
}

fn node_layout_dimensions(
    layout_shape: Option<&str>,
    metrics: crate::text::TextMetrics,
    padding: f64,
) -> (f64, f64) {
    let shape = layout_shape.unwrap_or("squareRect");
    let (render_w, render_h) = node_render_dimensions(Some(shape), metrics, padding);

    // Mermaid flowchart-v2 renders nodes using the "rendering-elements" layer:
    // 1) it generates SVG paths (roughjs-based even for non-handDrawn look),
    // 2) calls `updateNodeBounds(node, shapeElem)` which sets `node.width/height` from `getBBox()`,
    // 3) then feeds those updated dimensions into Dagre for layout.
    //
    // For stadium shapes the rough path is built from sampled arc points (`generateCirclePoints`,
    // 50 points over 180deg) and the resulting path bbox is slightly narrower than the theoretical
    // `w = bbox.width + h/4 + padding` used to generate the points. That bbox width is what Dagre
    // uses for spacing, which affects node x-positions and ultimately the root `viewBox`.
    if shape == "stadium" {
        fn generate_circle_points(
            center_x: f64,
            center_y: f64,
            radius: f64,
            num_points: usize,
            start_angle_deg: f64,
            end_angle_deg: f64,
        ) -> impl Iterator<Item = (f64, f64)> {
            let start = start_angle_deg.to_radians();
            let end = end_angle_deg.to_radians();
            let step = (end - start) / (num_points.saturating_sub(1).max(1) as f64);
            (0..num_points).map(move |i| {
                let angle = start + (i as f64) * step;
                let x = center_x + radius * angle.cos();
                let y = center_y + radius * angle.sin();
                (-x, -y)
            })
        }

        let w = render_w.max(0.0);
        let h = render_h.max(0.0);
        if w > 0.0 && h > 0.0 {
            let radius = h / 2.0;
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            let mut include = |x: f64, y: f64| {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            };

            include(-w / 2.0 + radius, -h / 2.0);
            include(w / 2.0 - radius, -h / 2.0);
            for (x, y) in generate_circle_points(-w / 2.0 + radius, 0.0, radius, 50, 90.0, 270.0) {
                include(x, y);
            }
            include(w / 2.0 - radius, h / 2.0);
            for (x, y) in generate_circle_points(w / 2.0 - radius, 0.0, radius, 50, 270.0, 450.0) {
                include(x, y);
            }

            if min_x.is_finite() && max_x.is_finite() && min_y.is_finite() && max_y.is_finite() {
                return ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0));
            }
        }
    }

    // Mermaid flowchart-v2 cylinder layout dimensions are derived from `updateNodeBounds(...)`
    // over an SVG `<path>` with arc commands. On Chromium this tends to round the bbox height to
    // the next representable f32 value above the theoretical height, which affects Dagre spacing
    // and therefore edge points (`data-points`) in strict parity mode.
    if matches!(shape, "cylinder" | "cyl") {
        let h_f32 = render_h as f32;
        if h_f32.is_finite() && h_f32.is_sign_positive() {
            let bits = h_f32.to_bits();
            if bits < u32::MAX {
                let bumped = f32::from_bits(bits + 1) as f64;
                return (render_w, bumped);
            }
        }
    }

    (render_w, render_h)
}

pub(crate) fn flowchart_label_plain_text_for_layout(
    label: &str,
    label_type: &str,
    html_labels: bool,
) -> String {
    fn decode_html_entity(entity: &str) -> Option<char> {
        match entity {
            "nbsp" => Some(' '),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "#39" => Some('\''),
            _ => {
                if let Some(hex) = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    None
                }
            }
        }
    }

    fn strip_html_for_layout(input: &str) -> String {
        // A lightweight, deterministic HTML text extractor for Mermaid htmlLabels layout.
        // We intentionally do not attempt full HTML parsing/sanitization here; we only need a
        // best-effort approximation of the rendered textContent for sizing.
        fn trim_trailing_break_whitespace(out: &mut String) {
            loop {
                let Some(ch) = out.chars().last() else {
                    return;
                };
                if ch == '\n' {
                    return;
                }
                if ch.is_whitespace() {
                    out.pop();
                    continue;
                }
                return;
            }
        }

        let mut out = String::with_capacity(input.len());
        let mut it = input.chars().peekable();
        while let Some(ch) = it.next() {
            if ch == '<' {
                let mut tag = String::new();
                while let Some(c) = it.next() {
                    if c == '>' {
                        break;
                    }
                    tag.push(c);
                }
                let tag = tag.trim();
                let tag_lower = tag.to_ascii_lowercase();
                let tag_trim = tag_lower.trim();
                if tag_trim.starts_with('!') || tag_trim.starts_with('?') {
                    continue;
                }
                let is_closing = tag_trim.starts_with('/');
                let name = tag_trim
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if name == "br" {
                    trim_trailing_break_whitespace(&mut out);
                    out.push('\n');
                } else if is_closing && matches!(name, "p" | "div" | "li" | "tr" | "ul" | "ol") {
                    trim_trailing_break_whitespace(&mut out);
                    out.push('\n');
                }
                continue;
            }

            if ch == '&' {
                let mut entity = String::new();
                let mut saw_semicolon = false;
                while let Some(&c) = it.peek() {
                    if c == ';' {
                        it.next();
                        saw_semicolon = true;
                        break;
                    }
                    if c == '<' || c == '&' || c.is_whitespace() || entity.len() > 32 {
                        break;
                    }
                    entity.push(c);
                    it.next();
                }
                if saw_semicolon {
                    if let Some(decoded) = decode_html_entity(entity.as_str()) {
                        out.push(decoded);
                    } else {
                        out.push('&');
                        out.push_str(&entity);
                        out.push(';');
                    }
                } else {
                    out.push('&');
                    out.push_str(&entity);
                }
                continue;
            }

            out.push(ch);
        }

        // Collapse whitespace runs similar to HTML layout defaults, while preserving explicit
        // line breaks introduced by tags like `<br>` and `</p>`.
        let mut normalized = String::with_capacity(out.len());
        let mut last_space = false;
        let mut last_nl = false;
        for ch in out.chars() {
            if ch == '\u{00A0}' {
                if !last_space && !last_nl {
                    normalized.push(' ');
                }
                last_space = true;
                continue;
            }
            if ch == '\n' {
                if !last_nl {
                    normalized.push('\n');
                }
                last_space = false;
                last_nl = true;
                continue;
            }
            if ch.is_whitespace() {
                if !last_space && !last_nl {
                    normalized.push(' ');
                    last_space = true;
                }
                continue;
            }
            normalized.push(ch);
            last_space = false;
            last_nl = false;
        }

        normalized
    }

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
            if html_labels || label_type == "html" {
                // Mermaid converts `fa:fa-...` / `fas:fa-...` tokens into `<i class="..."></i>`
                // during HTML label rendering. In exported SVG fixtures the FontAwesome CSS is not
                // embedded, so these `<i/>` elements contribute `0` width and `0` textContent.
                // Mirror that by stripping the icon token before we compute a headless text run.
                t = crate::text::replace_fontawesome_icons(&t);
                t = strip_html_for_layout(&t);
            } else {
                t = t.replace("<br />", "\n");
                t = t.replace("<br/>", "\n");
                t = t.replace("<br>", "\n");

                // In SVG-label mode (htmlLabels=false), Mermaid renders `<tag>text</tag>` as
                // escaped literal tag tokens with whitespace separation (see
                // `upstream_flowchart_v2_escaped_without_html_labels_spec`).
                //
                // For layout measurement we approximate that by inserting spaces between
                // adjacent tag/text tokens when the source omits them.
                fn space_separate_html_like_tags_for_svg_labels(input: &str) -> String {
                    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                    enum TokKind {
                        Text,
                        Tag,
                        Newline,
                    }

                    fn is_tag_start(s: &str) -> bool {
                        let mut it = s.chars();
                        if it.next() != Some('<') {
                            return false;
                        }
                        let Some(next) = it.next() else {
                            return false;
                        };
                        next.is_ascii_alphabetic() || matches!(next, '/' | '!' | '?')
                    }

                    let mut out = String::with_capacity(input.len());
                    let mut prev_kind: Option<TokKind> = None;

                    let mut i = 0usize;
                    while i < input.len() {
                        let rest = &input[i..];
                        if rest.starts_with('\n') {
                            out.push('\n');
                            prev_kind = Some(TokKind::Newline);
                            i += 1;
                            continue;
                        }

                        if is_tag_start(rest) {
                            let Some(rel_end) = rest.find('>') else {
                                // Malformed tag; treat as text.
                                let ch = rest.chars().next().unwrap();
                                out.push(ch);
                                prev_kind = Some(TokKind::Text);
                                i += ch.len_utf8();
                                continue;
                            };

                            let tag = &rest[..=rel_end];
                            if matches!(prev_kind, Some(TokKind::Text))
                                && !out.ends_with(|ch: char| ch.is_whitespace())
                            {
                                out.push(' ');
                            }
                            out.push_str(tag);
                            prev_kind = Some(TokKind::Tag);
                            i += rel_end + 1;
                            continue;
                        }

                        // Text run until next newline or tag start.
                        let mut run_end = input.len();
                        if let Some(nl) = rest.find('\n') {
                            run_end = run_end.min(i + nl);
                        }
                        if let Some(lt) = rest.find('<') {
                            run_end = run_end.min(i + lt);
                        }
                        let run = &input[i..run_end];
                        if matches!(prev_kind, Some(TokKind::Tag))
                            && !run.starts_with(|ch: char| ch.is_whitespace())
                        {
                            out.push(' ');
                        }
                        out.push_str(run);
                        prev_kind = Some(TokKind::Text);
                        i = run_end;
                    }

                    out
                }

                t = space_separate_html_like_tags_for_svg_labels(&t);
            }
            t.trim().trim_end_matches('\n').to_string()
        }
    }
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for n in nodes {
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        pts.push((n.x - hw, n.y - hh));
        pts.push((n.x + hw, n.y + hh));
    }
    for e in edges {
        for p in &e.points {
            pts.push((p.x, p.y));
        }
        if let Some(l) = &e.label {
            let hw = l.width / 2.0;
            let hh = l.height / 2.0;
            pts.push((l.x - hw, l.y - hh));
            pts.push((l.x + hw, l.y + hh));
        }
    }
    Bounds::from_points(pts)
}
