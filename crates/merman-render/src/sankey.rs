use crate::model::{Bounds, SankeyDiagramLayout, SankeyLinkLayout, SankeyNodeLayout};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
struct SankeySemanticModel {
    graph: SankeySemanticGraph,
}

#[derive(Debug, Clone, Deserialize)]
struct SankeySemanticGraph {
    #[serde(default)]
    nodes: Vec<SankeySemanticNode>,
    #[serde(default)]
    links: Vec<SankeySemanticLink>,
}

#[derive(Debug, Clone, Deserialize)]
struct SankeySemanticNode {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SankeySemanticLink {
    source: String,
    target: String,
    value: Value,
}

#[derive(Debug, Clone)]
struct Node {
    id: String,
    index: usize,
    source_links: Vec<usize>,
    target_links: Vec<usize>,
    value: f64,
    depth: usize,
    height: usize,
    layer: usize,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

#[derive(Debug, Clone)]
struct Link {
    index: usize,
    source: usize,
    target: usize,
    value: f64,
    width: f64,
    y0: f64,
    y1: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeAlign {
    Left,
    Right,
    Justify,
    Center,
}

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_f64()
}

fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn has_ref_object(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.contains_key("$ref"))
}

fn parse_align(cfg: &Value) -> NodeAlign {
    match config_string(cfg, &["sankey", "nodeAlignment"]).as_deref() {
        Some("left") => NodeAlign::Left,
        Some("right") => NodeAlign::Right,
        Some("center") => NodeAlign::Center,
        _ => NodeAlign::Justify,
    }
}

fn f64_cmp(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

pub fn layout_sankey_diagram(
    semantic: &Value,
    effective_config: &Value,
    _text_measurer: &dyn TextMeasurer,
) -> Result<SankeyDiagramLayout> {
    let model: SankeySemanticModel = serde_json::from_value(Value::clone(semantic))?;

    let width = config_f64(effective_config, &["sankey", "width"]).unwrap_or(600.0);
    let height = config_f64(effective_config, &["sankey", "height"]).unwrap_or(400.0);

    let sankey_cfg = effective_config.get("sankey");
    let sankey_cfg_missing = sankey_cfg.is_none() || sankey_cfg.is_some_and(has_ref_object);
    let show_values = if sankey_cfg_missing {
        true
    } else {
        config_bool(effective_config, &["sankey", "showValues"]).unwrap_or(true)
    };
    let align = parse_align(effective_config);

    let dx = 10.0;
    let dy: f64 = 10.0 + if show_values { 15.0 } else { 0.0 };
    let iterations = 6usize;

    let mut nodes: Vec<Node> = model
        .graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| Node {
            id: n.id.clone(),
            index: i,
            source_links: Vec::new(),
            target_links: Vec::new(),
            value: 0.0,
            depth: 0,
            height: 0,
            layer: 0,
            x0: 0.0,
            x1: 0.0,
            y0: 0.0,
            y1: 0.0,
        })
        .collect();

    let mut node_by_id: HashMap<String, usize> = HashMap::new();
    for (i, n) in model.graph.nodes.iter().enumerate() {
        node_by_id.insert(n.id.clone(), i);
    }

    let mut links: Vec<Link> = Vec::with_capacity(model.graph.links.len());
    for (i, l) in model.graph.links.iter().enumerate() {
        let source = node_by_id
            .get(&l.source)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing node id {}", l.source),
            })?;
        let target = node_by_id
            .get(&l.target)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing node id {}", l.target),
            })?;

        let value = l.value.as_f64().unwrap_or(0.0);
        links.push(Link {
            index: i,
            source,
            target,
            value,
            width: 0.0,
            y0: 0.0,
            y1: 0.0,
        });

        nodes[source].source_links.push(i);
        nodes[target].target_links.push(i);
    }

    for n in &mut nodes {
        let out_sum: f64 = n.source_links.iter().map(|&li| links[li].value).sum();
        let in_sum: f64 = n.target_links.iter().map(|&li| links[li].value).sum();
        n.value = out_sum.max(in_sum);
    }

    fn compute_node_depths(nodes: &mut [Node], links: &[Link]) -> Result<()> {
        let n = nodes.len();
        let mut current: Vec<usize> = (0..n).collect();
        let mut next: Vec<usize> = Vec::new();
        let mut next_seen = vec![false; n];
        let mut x: usize = 0;
        while !current.is_empty() {
            for &node_idx in &current {
                nodes[node_idx].depth = x;
                for &li in &nodes[node_idx].source_links {
                    let t = links[li].target;
                    if !next_seen[t] {
                        next_seen[t] = true;
                        next.push(t);
                    }
                }
            }
            x += 1;
            if x > n {
                return Err(Error::InvalidModel {
                    message: "circular link".to_string(),
                });
            }
            current = next;
            next = Vec::new();
            next_seen.fill(false);
        }
        Ok(())
    }

    fn compute_node_heights(nodes: &mut [Node], links: &[Link]) -> Result<()> {
        let n = nodes.len();
        let mut current: Vec<usize> = (0..n).collect();
        let mut next: Vec<usize> = Vec::new();
        let mut next_seen = vec![false; n];
        let mut x: usize = 0;
        while !current.is_empty() {
            for &node_idx in &current {
                nodes[node_idx].height = x;
                for &li in &nodes[node_idx].target_links {
                    let s = links[li].source;
                    if !next_seen[s] {
                        next_seen[s] = true;
                        next.push(s);
                    }
                }
            }
            x += 1;
            if x > n {
                return Err(Error::InvalidModel {
                    message: "circular link".to_string(),
                });
            }
            current = next;
            next = Vec::new();
            next_seen.fill(false);
        }
        Ok(())
    }

    compute_node_depths(&mut nodes, &links)?;
    compute_node_heights(&mut nodes, &links)?;

    let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    let column_count = max_depth + 1;
    let kx = if column_count <= 1 {
        0.0
    } else {
        (width - dx) / (column_count as f64 - 1.0)
    };

    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); column_count.max(1)];
    for i in 0..nodes.len() {
        let x = column_count.max(1);
        let raw_layer = match align {
            NodeAlign::Left => nodes[i].depth as i64,
            NodeAlign::Right => x as i64 - 1 - nodes[i].height as i64,
            NodeAlign::Justify => {
                if nodes[i].source_links.is_empty() {
                    x as i64 - 1
                } else {
                    nodes[i].depth as i64
                }
            }
            NodeAlign::Center => {
                if !nodes[i].target_links.is_empty() {
                    nodes[i].depth as i64
                } else if !nodes[i].source_links.is_empty() {
                    let min_target_depth = nodes[i]
                        .source_links
                        .iter()
                        .map(|&li| nodes[links[li].target].depth)
                        .min()
                        .unwrap_or(0);
                    min_target_depth as i64 - 1
                } else {
                    0
                }
            }
        };
        let layer = raw_layer.clamp(0, x as i64 - 1) as usize;
        nodes[i].layer = layer;
        nodes[i].x0 = layer as f64 * kx;
        nodes[i].x1 = nodes[i].x0 + dx;
        columns[layer].push(i);
    }

    let max_len = columns.iter().map(|c| c.len()).max().unwrap_or(0);
    let py = if max_len <= 1 {
        dy
    } else {
        dy.min(height / (max_len as f64 - 1.0))
    };

    let mut ky = f64::INFINITY;
    for col in &columns {
        if col.is_empty() {
            continue;
        }
        let sum_values: f64 = col.iter().map(|&ni| nodes[ni].value).sum();
        if sum_values <= 0.0 {
            continue;
        }
        let denom = height - (col.len() as f64 - 1.0) * py;
        ky = ky.min(denom / sum_values);
    }
    if !ky.is_finite() {
        ky = 0.0;
    }

    fn sort_source_links_by_target_y0(
        node_y0: &[f64],
        links: &[Link],
        link_indices: &mut [usize],
    ) {
        link_indices.sort_by(|&a, &b| {
            let ta = node_y0[links[a].target];
            let tb = node_y0[links[b].target];
            f64_cmp(ta, tb).then_with(|| links[a].index.cmp(&links[b].index))
        });
    }

    fn sort_target_links_by_source_y0(
        node_y0: &[f64],
        links: &[Link],
        link_indices: &mut [usize],
    ) {
        link_indices.sort_by(|&a, &b| {
            let sa = node_y0[links[a].source];
            let sb = node_y0[links[b].source];
            f64_cmp(sa, sb).then_with(|| links[a].index.cmp(&links[b].index))
        });
    }

    fn reorder_links(nodes: &mut [Node], links: &[Link], column: &[usize]) {
        let node_y0 = nodes.iter().map(|n| n.y0).collect::<Vec<_>>();
        for &ni in column {
            sort_source_links_by_target_y0(&node_y0, links, &mut nodes[ni].source_links);
            sort_target_links_by_source_y0(&node_y0, links, &mut nodes[ni].target_links);
        }
    }

    for col in &columns {
        let mut y = 0.0;
        for &ni in col {
            nodes[ni].y0 = y;
            nodes[ni].y1 = y + nodes[ni].value * ky;
            y = nodes[ni].y1 + py;
            for &li in &nodes[ni].source_links {
                links[li].width = links[li].value * ky;
            }
        }
        let n = col.len();
        if n > 0 {
            let offset = (height - y + py) / (n as f64 + 1.0);
            for (i, &ni) in col.iter().enumerate() {
                let adj = offset * (i as f64 + 1.0);
                nodes[ni].y0 += adj;
                nodes[ni].y1 += adj;
            }
            reorder_links(&mut nodes, &links, col);
        }
    }

    fn target_top(nodes: &[Node], links: &[Link], py: f64, source: usize, target: usize) -> f64 {
        let source_link_count = nodes[source].source_links.len() as f64;
        let mut y = nodes[source].y0 - (source_link_count - 1.0) * py / 2.0;
        for &li in &nodes[source].source_links {
            let node = links[li].target;
            if node == target {
                break;
            }
            y += links[li].width + py;
        }
        for &li in &nodes[target].target_links {
            let node = links[li].source;
            if node == source {
                break;
            }
            y -= links[li].width;
        }
        y
    }

    fn source_top(nodes: &[Node], links: &[Link], py: f64, source: usize, target: usize) -> f64 {
        let target_link_count = nodes[target].target_links.len() as f64;
        let mut y = nodes[target].y0 - (target_link_count - 1.0) * py / 2.0;
        for &li in &nodes[target].target_links {
            let node = links[li].source;
            if node == source {
                break;
            }
            y += links[li].width + py;
        }
        for &li in &nodes[source].source_links {
            let node = links[li].target;
            if node == target {
                break;
            }
            y -= links[li].width;
        }
        y
    }

    fn reorder_node_links(nodes: &mut [Node], links: &[Link], node_idx: usize) {
        let node_y0 = nodes.iter().map(|n| n.y0).collect::<Vec<_>>();

        let target_links = nodes[node_idx].target_links.clone();
        for li in target_links {
            let source = links[li].source;
            sort_source_links_by_target_y0(&node_y0, links, &mut nodes[source].source_links);
        }

        let source_links = nodes[node_idx].source_links.clone();
        for li in source_links {
            let target = links[li].target;
            sort_target_links_by_source_y0(&node_y0, links, &mut nodes[target].target_links);
        }
    }

    fn resolve_collisions_top_to_bottom(
        nodes: &mut [Node],
        column: &[usize],
        py: f64,
        mut y: f64,
        mut i: isize,
        alpha: f64,
    ) {
        while i < column.len() as isize {
            let ni = column[i as usize];
            let dy = (y - nodes[ni].y0) * alpha;
            if dy > 1e-6 {
                nodes[ni].y0 += dy;
                nodes[ni].y1 += dy;
            }
            y = nodes[ni].y1 + py;
            i += 1;
        }
    }

    fn resolve_collisions_bottom_to_top(
        nodes: &mut [Node],
        column: &[usize],
        py: f64,
        mut y: f64,
        mut i: isize,
        alpha: f64,
    ) {
        while i >= 0 {
            let ni = column[i as usize];
            let dy = (nodes[ni].y1 - y) * alpha;
            if dy > 1e-6 {
                nodes[ni].y0 -= dy;
                nodes[ni].y1 -= dy;
            }
            y = nodes[ni].y0 - py;
            i -= 1;
        }
    }

    fn resolve_collisions(
        nodes: &mut [Node],
        column: &[usize],
        py: f64,
        y0_extent: f64,
        y1_extent: f64,
        alpha: f64,
    ) {
        if column.is_empty() {
            return;
        }
        let i = column.len() >> 1;
        let subject = column[i];
        resolve_collisions_bottom_to_top(
            nodes,
            column,
            py,
            nodes[subject].y0 - py,
            i as isize - 1,
            alpha,
        );
        resolve_collisions_top_to_bottom(
            nodes,
            column,
            py,
            nodes[subject].y1 + py,
            i as isize + 1,
            alpha,
        );
        resolve_collisions_bottom_to_top(
            nodes,
            column,
            py,
            y1_extent,
            column.len() as isize - 1,
            alpha,
        );
        resolve_collisions_top_to_bottom(nodes, column, py, y0_extent, 0, alpha);
    }

    fn relax_left_to_right(
        nodes: &mut [Node],
        links: &[Link],
        columns: &mut [Vec<usize>],
        py: f64,
        alpha: f64,
        beta: f64,
        y0_extent: f64,
        y1_extent: f64,
    ) {
        for i in 1..columns.len() {
            let column = &mut columns[i];
            for &target in column.iter() {
                let mut y = 0.0;
                let mut w = 0.0;
                for &li in &nodes[target].target_links {
                    let source = links[li].source;
                    let value = links[li].value;
                    let v = value * (nodes[target].layer as f64 - nodes[source].layer as f64);
                    y += target_top(nodes, links, py, source, target) * v;
                    w += v;
                }
                if !(w > 0.0) {
                    continue;
                }
                let dy = (y / w - nodes[target].y0) * alpha;
                nodes[target].y0 += dy;
                nodes[target].y1 += dy;
                reorder_node_links(nodes, links, target);
            }
            column.sort_by(|&a, &b| f64_cmp(nodes[a].y0, nodes[b].y0).then_with(|| a.cmp(&b)));
            resolve_collisions(nodes, column, py, y0_extent, y1_extent, beta);
        }
    }

    fn relax_right_to_left(
        nodes: &mut [Node],
        links: &[Link],
        columns: &mut [Vec<usize>],
        py: f64,
        alpha: f64,
        beta: f64,
        y0_extent: f64,
        y1_extent: f64,
    ) {
        if columns.len() < 2 {
            return;
        }
        for i in (0..=(columns.len() - 2)).rev() {
            let column = &mut columns[i];
            for &source in column.iter() {
                let mut y = 0.0;
                let mut w = 0.0;
                for &li in &nodes[source].source_links {
                    let target = links[li].target;
                    let value = links[li].value;
                    let v = value * (nodes[target].layer as f64 - nodes[source].layer as f64);
                    y += source_top(nodes, links, py, source, target) * v;
                    w += v;
                }
                if !(w > 0.0) {
                    continue;
                }
                let dy = (y / w - nodes[source].y0) * alpha;
                nodes[source].y0 += dy;
                nodes[source].y1 += dy;
                reorder_node_links(nodes, links, source);
            }
            column.sort_by(|&a, &b| f64_cmp(nodes[a].y0, nodes[b].y0).then_with(|| a.cmp(&b)));
            resolve_collisions(nodes, column, py, y0_extent, y1_extent, beta);
        }
    }

    let mut columns_for_relax = columns.clone();
    for i in 0..iterations {
        let alpha = 0.99_f64.powi(i as i32);
        let beta = (1.0 - alpha).max((i as f64 + 1.0) / iterations as f64);
        relax_right_to_left(
            &mut nodes,
            &links,
            &mut columns_for_relax,
            py,
            alpha,
            beta,
            0.0,
            height,
        );
        relax_left_to_right(
            &mut nodes,
            &links,
            &mut columns_for_relax,
            py,
            alpha,
            beta,
            0.0,
            height,
        );
    }

    for node in &mut nodes {
        let mut y0 = node.y0;
        let mut y1 = node.y0;
        for &li in &node.source_links {
            links[li].y0 = y0 + links[li].width / 2.0;
            y0 += links[li].width;
        }
        for &li in &node.target_links {
            links[li].y1 = y1 + links[li].width / 2.0;
            y1 += links[li].width;
        }
    }

    let layout_nodes: Vec<SankeyNodeLayout> = nodes
        .iter()
        .map(|n| SankeyNodeLayout {
            id: n.id.clone(),
            index: n.index,
            depth: n.depth,
            height: n.height,
            layer: n.layer,
            value: n.value,
            x0: n.x0,
            x1: n.x1,
            y0: n.y0,
            y1: n.y1,
        })
        .collect();

    let layout_links: Vec<SankeyLinkLayout> = links
        .iter()
        .map(|l| SankeyLinkLayout {
            index: l.index,
            source: nodes[l.source].id.clone(),
            target: nodes[l.target].id.clone(),
            value: l.value,
            width: l.width,
            y0: l.y0,
            y1: l.y1,
        })
        .collect();

    Ok(SankeyDiagramLayout {
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: width,
            max_y: height,
        }),
        width,
        height,
        node_width: dx,
        node_padding: py,
        nodes: layout_nodes,
        links: layout_links,
    })
}
