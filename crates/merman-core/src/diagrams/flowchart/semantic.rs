use crate::sanitize::sanitize_text;
use crate::utils::format_url;
use crate::{Error, MermaidConfig, Result};
use indexmap::IndexMap;
use std::collections::HashMap;

use super::{
    ClickAction, Edge, EdgeDefaults, FlowSubGraph, LinkStylePos, Node, Stmt, TitleKind,
    apply_shape_data_to_node, parse_shape_data_yaml, yaml_to_bool, yaml_to_string,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_semantic_statements(
    statements: &[Stmt],
    nodes: &mut Vec<Node>,
    node_index: &mut HashMap<String, usize>,
    edges: &mut Vec<Edge>,
    subgraphs: &mut Vec<FlowSubGraph>,
    subgraph_index: &mut HashMap<String, usize>,
    class_defs: &mut IndexMap<String, Vec<String>>,
    tooltips: &mut HashMap<String, String>,
    edge_defaults: &mut EdgeDefaults,
    security_level_loose: bool,
    diagram_type: &str,
    config: &MermaidConfig,
) -> Result<()> {
    for stmt in statements {
        match stmt {
            Stmt::Subgraph(sg) => {
                apply_semantic_statements(
                    &sg.statements,
                    nodes,
                    node_index,
                    edges,
                    subgraphs,
                    subgraph_index,
                    class_defs,
                    tooltips,
                    edge_defaults,
                    security_level_loose,
                    diagram_type,
                    config,
                )?;
            }
            Stmt::Style(s) => {
                if let Some(&idx) = subgraph_index.get(&s.target) {
                    subgraphs[idx].styles.extend(s.styles.iter().cloned());
                } else {
                    let idx = ensure_node(nodes, node_index, &s.target);
                    nodes[idx].styles.extend(s.styles.iter().cloned());
                }
            }
            Stmt::ClassDef(c) => {
                for id in &c.ids {
                    class_defs.insert(id.clone(), c.styles.clone());
                }
            }
            Stmt::ClassAssign(c) => {
                for target in &c.targets {
                    add_class_to_target(
                        nodes,
                        node_index,
                        edges,
                        subgraphs,
                        subgraph_index,
                        target,
                        &c.class_name,
                    );
                }
            }
            Stmt::Click(c) => {
                for id in &c.ids {
                    if let Some(tt) = &c.tooltip {
                        tooltips.insert(id.clone(), sanitize_text(tt, config));
                    }
                    add_class_to_target(
                        nodes,
                        node_index,
                        edges,
                        subgraphs,
                        subgraph_index,
                        id,
                        "clickable",
                    );

                    match &c.action {
                        ClickAction::Link { href, target } => {
                            if let Some(&idx) = node_index.get(id) {
                                nodes[idx].link = format_url(href, config);
                                nodes[idx].link_target = target.clone();
                            }
                        }
                        ClickAction::Callback { .. } => {
                            if security_level_loose {
                                if let Some(&idx) = node_index.get(id) {
                                    nodes[idx].have_callback = true;
                                }
                            }
                        }
                    }
                }
            }
            Stmt::LinkStyle(ls) => {
                if let Some(algo) = &ls.interpolate {
                    for pos in &ls.positions {
                        match pos {
                            LinkStylePos::Default => edge_defaults.interpolate = Some(algo.clone()),
                            LinkStylePos::Index(i) => {
                                if *i >= edges.len() {
                                    return Err(Error::DiagramParse {
                                        diagram_type: diagram_type.to_string(),
                                        message: format!(
                                            "The index {i} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                                            edges.len().saturating_sub(1)
                                        ),
                                    });
                                }
                                edges[*i].interpolate = Some(algo.clone());
                            }
                        }
                    }
                }

                if !ls.styles.is_empty() {
                    for pos in &ls.positions {
                        match pos {
                            LinkStylePos::Default => edge_defaults.style = ls.styles.clone(),
                            LinkStylePos::Index(i) => {
                                if *i >= edges.len() {
                                    return Err(Error::DiagramParse {
                                        diagram_type: diagram_type.to_string(),
                                        message: format!(
                                            "The index {i} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                                            edges.len().saturating_sub(1)
                                        ),
                                    });
                                }
                                edges[*i].style = ls.styles.clone();
                                if !edges[*i].style.is_empty()
                                    && !edges[*i]
                                        .style
                                        .iter()
                                        .any(|s| s.trim_start().starts_with("fill"))
                                {
                                    edges[*i].style.push("fill:none".to_string());
                                }
                            }
                        }
                    }
                }
            }
            Stmt::ShapeData { target, yaml } => {
                // Mermaid syntax uses the same `@{...}` form for both nodes and edges:
                // - if an edge with the given ID exists, it updates the edge metadata
                // - otherwise it updates (and may create) a node
                let v = parse_shape_data_yaml(yaml).map_err(|e| Error::DiagramParse {
                    diagram_type: diagram_type.to_string(),
                    message: format!("Invalid shapeData: {e}"),
                })?;

                let map = v.as_mapping();
                let is_edge_target = edges
                    .iter()
                    .any(|e| e.id.as_deref() == Some(target.as_str()));
                if is_edge_target {
                    if let Some(map) = map {
                        for e in edges.iter_mut() {
                            if e.id.as_deref() != Some(target.as_str()) {
                                continue;
                            }
                            for (k, v) in map {
                                let Some(key) = k.as_str() else { continue };
                                match key {
                                    "animate" => {
                                        if let Some(b) = yaml_to_bool(v) {
                                            e.animate = Some(b);
                                        }
                                    }
                                    "animation" => {
                                        if let Some(s) = yaml_to_string(v) {
                                            e.animation = Some(s);
                                        }
                                    }
                                    "curve" => {
                                        if let Some(s) = yaml_to_string(v) {
                                            e.interpolate = Some(s);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    continue;
                }

                let idx = ensure_node(nodes, node_index, target);
                apply_shape_data_to_node(&mut nodes[idx], yaml).map_err(|e| {
                    Error::DiagramParse {
                        diagram_type: diagram_type.to_string(),
                        message: e,
                    }
                })?;
            }
            Stmt::Chain { .. } | Stmt::Node(_) | Stmt::Direction(_) => {}
        }
    }
    Ok(())
}

fn add_class_to_target(
    nodes: &mut [Node],
    node_index: &HashMap<String, usize>,
    edges: &mut [Edge],
    subgraphs: &mut [FlowSubGraph],
    subgraph_index: &HashMap<String, usize>,
    target: &str,
    class_name: &str,
) {
    if let Some(&idx) = subgraph_index.get(target) {
        subgraphs[idx].classes.push(class_name.to_string());
    }
    if let Some(&idx) = node_index.get(target) {
        nodes[idx].classes.push(class_name.to_string());
    }
    for e in edges.iter_mut() {
        if e.id.as_deref() == Some(target) {
            e.classes.push(class_name.to_string());
        }
    }
}

fn ensure_node(nodes: &mut Vec<Node>, node_index: &mut HashMap<String, usize>, id: &str) -> usize {
    if let Some(&idx) = node_index.get(id) {
        return idx;
    }
    let idx = nodes.len();
    nodes.push(Node {
        id: id.to_string(),
        label: None,
        label_type: TitleKind::Text,
        shape: None,
        shape_data: None,
        icon: None,
        form: None,
        pos: None,
        img: None,
        constraint: None,
        asset_width: None,
        asset_height: None,
        styles: Vec::new(),
        classes: Vec::new(),
        link: None,
        link_target: None,
        have_callback: false,
    });
    node_index.insert(id.to_string(), idx);
    idx
}
