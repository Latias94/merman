use crate::sanitize::sanitize_text;
use crate::{Error, ParseMetadata, Result};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::collections::HashMap;

lalrpop_util::lalrpop_mod!(
    #[allow(clippy::type_complexity, clippy::result_large_err)]
    flowchart_grammar,
    "/diagrams/flowchart_grammar.rs"
);

mod accessibility;
mod ast;
mod build;
mod lex;
mod lexer;
mod lexer_iter;
mod model;
mod semantic;
mod subgraph;
mod tokens;

pub use model::{FlowEdge, FlowEdgeDefaults, FlowNode, FlowSubgraph, FlowchartV2Model};

pub(crate) use model::{
    Edge, EdgeDefaults, LabeledText, LinkToken, Node, SubgraphHeader, TitleKind,
};

pub(crate) use ast::{
    ClassAssignStmt, ClassDefStmt, ClickAction, ClickStmt, FlowchartAst, LinkStylePos,
    LinkStyleStmt, Stmt, StyleStmt, SubgraphBlock,
};

pub(crate) use tokens::{LexError, NodeLabelToken, Tok};

use accessibility::extract_flowchart_accessibility_statements;
use build::FlowchartBuildState;
use lexer::Lexer;
use semantic::apply_semantic_statements;
use subgraph::SubgraphBuilder;

#[derive(Debug, Clone)]
pub(crate) struct FlowSubGraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub dir: Option<String>,
    pub label_type: String,
}

pub fn parse_flowchart(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let (code, acc_title, acc_descr) = extract_flowchart_accessibility_statements(code);
    let ast = flowchart_grammar::FlowchartAstParser::new()
        .parse(Lexer::new(&code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut build = FlowchartBuildState::new();
    build
        .add_statements(&ast.statements)
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: e,
        })?;
    let FlowchartBuildState {
        nodes,
        edges,
        vertex_calls,
        ..
    } = build;
    let mut nodes = nodes;
    let mut edges = edges;

    let inherit_dir = meta
        .effective_config
        .as_value()
        .get("flowchart")
        .and_then(|v| v.get("inheritDir"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut builder = SubgraphBuilder::new(inherit_dir, ast.direction.clone());
    builder.visit_statements(&ast.statements);

    let mut class_defs: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut tooltips: HashMap<String, String> = HashMap::new();
    let mut edge_defaults = EdgeDefaults {
        style: Vec::new(),
        interpolate: None,
    };

    let mut node_index: HashMap<String, usize> = HashMap::new();
    for (idx, n) in nodes.iter().enumerate() {
        node_index.insert(n.id.clone(), idx);
    }
    let mut subgraph_index: HashMap<String, usize> = HashMap::new();
    for (idx, sg) in builder.subgraphs.iter().enumerate() {
        subgraph_index.insert(sg.id.clone(), idx);
    }

    let security_level_loose = meta.effective_config.get_str("securityLevel") == Some("loose");
    apply_semantic_statements(
        &ast.statements,
        &mut nodes,
        &mut node_index,
        &mut edges,
        &mut builder.subgraphs,
        &mut subgraph_index,
        &mut class_defs,
        &mut tooltips,
        &mut edge_defaults,
        security_level_loose,
        &meta.diagram_type,
        &meta.effective_config,
    )?;

    fn get_layout_shape(n: &Node) -> String {
        // Mirrors Mermaid FlowDB `getTypeFromVertex` logic at 11.12.2.
        if n.img.is_some() {
            return "imageSquare".to_string();
        }
        if n.icon.is_some() {
            match n.form.as_deref() {
                Some("circle") => return "iconCircle".to_string(),
                Some("square") => return "iconSquare".to_string(),
                Some("rounded") => return "iconRounded".to_string(),
                _ => return "icon".to_string(),
            }
        }
        match n.shape.as_deref() {
            Some("square") | None => "squareRect".to_string(),
            Some("round") => "roundedRect".to_string(),
            Some("ellipse") => "ellipse".to_string(),
            Some(other) => other.to_string(),
        }
    }

    fn decode_mermaid_hash_entities(input: &str) -> std::borrow::Cow<'_, str> {
        // Mermaid runs `encodeEntities(...)` before parsing and later decodes with browser
        // `entityDecode(...)`. In our headless pipeline we decode into Unicode during parsing so
        // layout + SVG output match upstream.
        crate::entities::decode_mermaid_entities_to_unicode(input)
    }

    Ok(json!({
        "type": meta.diagram_type,
        "keyword": ast.keyword,
        "direction": ast.direction,
        "accTitle": acc_title,
        "accDescr": acc_descr,
        "classDefs": class_defs,
        "tooltips": tooltips.into_iter().collect::<HashMap<_, _>>(),
        "edgeDefaults": {
            "style": edge_defaults.style,
            "interpolate": edge_defaults.interpolate,
        },
        "vertexCalls": vertex_calls,
        "nodes": nodes.into_iter().map(|n| {
            let layout_shape = get_layout_shape(&n);
            let label_raw = n.label.clone().unwrap_or_else(|| n.id.clone());
            let label_raw = decode_mermaid_hash_entities(&label_raw);
            let mut label = sanitize_text(&label_raw, &meta.effective_config);
            if label.len() >= 2 && label.starts_with('\"') && label.ends_with('\"') {
                label = label[1..label.len() - 1].to_string();
            }
            json!({
                "id": n.id,
                "label": label,
                "labelType": title_kind_str(&n.label_type),
                "shape": n.shape,
                "layoutShape": layout_shape,
                "icon": n.icon,
                "form": n.form,
                "pos": n.pos,
                "img": n.img,
                "constraint": n.constraint,
                "assetWidth": n.asset_width,
                "assetHeight": n.asset_height,
                "styles": n.styles,
                "classes": n.classes,
                "link": n.link,
                "linkTarget": n.link_target,
                "haveCallback": n.have_callback,
            })
        }).collect::<Vec<_>>(),
        "edges": edges.into_iter().map(|e| {
            let label = e
                .label
                .as_ref()
                .map(|s| {
                    let decoded = decode_mermaid_hash_entities(s);
                    sanitize_text(&decoded, &meta.effective_config)
                });
            json!({
                "from": e.from,
                "to": e.to,
                "id": e.id,
                "isUserDefinedId": e.is_user_defined_id,
                "arrow": e.link.end,
                "type": e.link.edge_type,
                "stroke": e.link.stroke,
                "length": e.link.length,
                "label": label,
                "labelType": title_kind_str(&e.label_type),
                "style": e.style,
                "classes": e.classes,
                "interpolate": e.interpolate,
                "animate": e.animate,
                "animation": e.animation,
            })
        }).collect::<Vec<_>>(),
        "subgraphs": builder.subgraphs.into_iter().map(flow_subgraph_to_json).collect::<Vec<_>>(),
    }))
}

pub fn parse_flowchart_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<FlowchartV2Model> {
    let (code, acc_title, acc_descr) = extract_flowchart_accessibility_statements(code);
    let ast = flowchart_grammar::FlowchartAstParser::new()
        .parse(Lexer::new(&code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut build = FlowchartBuildState::new();
    build
        .add_statements(&ast.statements)
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: e,
        })?;
    let FlowchartBuildState {
        nodes,
        edges,
        vertex_calls,
        ..
    } = build;
    let mut nodes = nodes;
    let mut edges = edges;

    let inherit_dir = meta
        .effective_config
        .as_value()
        .get("flowchart")
        .and_then(|v| v.get("inheritDir"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut builder = SubgraphBuilder::new(inherit_dir, ast.direction.clone());
    builder.visit_statements(&ast.statements);

    let mut class_defs: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut tooltips: HashMap<String, String> = HashMap::new();
    let mut edge_defaults = EdgeDefaults {
        style: Vec::new(),
        interpolate: None,
    };

    let mut node_index: HashMap<String, usize> = HashMap::new();
    for (idx, n) in nodes.iter().enumerate() {
        node_index.insert(n.id.clone(), idx);
    }
    let mut subgraph_index: HashMap<String, usize> = HashMap::new();
    for (idx, sg) in builder.subgraphs.iter().enumerate() {
        subgraph_index.insert(sg.id.clone(), idx);
    }

    let security_level_loose = meta.effective_config.get_str("securityLevel") == Some("loose");
    apply_semantic_statements(
        &ast.statements,
        &mut nodes,
        &mut node_index,
        &mut edges,
        &mut builder.subgraphs,
        &mut subgraph_index,
        &mut class_defs,
        &mut tooltips,
        &mut edge_defaults,
        security_level_loose,
        &meta.diagram_type,
        &meta.effective_config,
    )?;

    fn get_layout_shape(n: &Node) -> String {
        // Mirrors Mermaid FlowDB `getTypeFromVertex` logic at 11.12.2.
        if n.img.is_some() {
            return "imageSquare".to_string();
        }
        if n.icon.is_some() {
            match n.form.as_deref() {
                Some("circle") => return "iconCircle".to_string(),
                Some("square") => return "iconSquare".to_string(),
                Some("rounded") => return "iconRounded".to_string(),
                _ => return "icon".to_string(),
            }
        }
        match n.shape.as_deref() {
            Some("square") | None => "squareRect".to_string(),
            Some("round") => "roundedRect".to_string(),
            Some("ellipse") => "ellipse".to_string(),
            Some(other) => other.to_string(),
        }
    }

    fn decode_mermaid_hash_entities(input: &str) -> std::borrow::Cow<'_, str> {
        // Mermaid runs `encodeEntities(...)` before parsing and later decodes with browser
        // `entityDecode(...)`. In our headless pipeline we decode into Unicode during parsing so
        // layout + SVG output match upstream.
        crate::entities::decode_mermaid_entities_to_unicode(input)
    }

    let nodes = nodes
        .into_iter()
        .map(|n| {
            let layout_shape = get_layout_shape(&n);
            let label_raw = n.label.clone().unwrap_or_else(|| n.id.clone());
            let label_raw = decode_mermaid_hash_entities(&label_raw);
            let mut label = sanitize_text(&label_raw, &meta.effective_config);
            if label.len() >= 2 && label.starts_with('\"') && label.ends_with('\"') {
                label = label[1..label.len() - 1].to_string();
            }

            FlowNode {
                id: n.id,
                label: Some(label),
                label_type: Some(title_kind_str(&n.label_type).to_string()),
                layout_shape: Some(layout_shape),
                icon: n.icon,
                form: n.form,
                pos: n.pos,
                img: n.img,
                constraint: n.constraint,
                asset_width: n.asset_width,
                asset_height: n.asset_height,
                classes: n.classes,
                styles: n.styles,
                link: n.link,
                link_target: n.link_target,
                have_callback: n.have_callback,
            }
        })
        .collect::<Vec<_>>();

    let edges = edges
        .into_iter()
        .map(|e| {
            let label = e.label.as_ref().map(|s| {
                let decoded = decode_mermaid_hash_entities(s);
                sanitize_text(&decoded, &meta.effective_config)
            });
            let id = e.id.ok_or_else(|| Error::DiagramParse {
                diagram_type: meta.diagram_type.clone(),
                message: "flowchart edge id missing".to_string(),
            })?;
            Ok(FlowEdge {
                id,
                from: e.from,
                to: e.to,
                label,
                label_type: Some(title_kind_str(&e.label_type).to_string()),
                edge_type: Some(e.link.edge_type),
                stroke: Some(e.link.stroke),
                length: e.link.length,
                style: e.style,
                classes: e.classes,
                interpolate: e.interpolate,
                animate: e.animate,
                animation: e.animation,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(FlowchartV2Model {
        acc_descr,
        acc_title,
        class_defs,
        direction: ast.direction,
        edge_defaults: Some(FlowEdgeDefaults {
            style: edge_defaults.style,
            interpolate: edge_defaults.interpolate,
        }),
        vertex_calls,
        nodes,
        edges,
        subgraphs: builder
            .subgraphs
            .into_iter()
            .map(flow_subgraph_to_model)
            .collect::<Vec<_>>(),
        tooltips: tooltips.into_iter().collect(),
    })
}

fn parse_shape_data_yaml(yaml_body: &str) -> std::result::Result<serde_yaml::Value, String> {
    let yaml_data = if yaml_body.contains('\n') {
        format!("{yaml_body}\n")
    } else {
        format!("{{\n{yaml_body}\n}}")
    };
    serde_yaml::from_str(&yaml_data).map_err(|e| format!("{e}"))
}

const MERMAID_SHAPES_11_12_2: &[&str] = &[
    "anchor",
    "bang",
    "bolt",
    "bow-rect",
    "bow-tie-rectangle",
    "brace",
    "brace-l",
    "brace-r",
    "braces",
    "card",
    "choice",
    "circ",
    "circle",
    "classBox",
    "cloud",
    "collate",
    "com-link",
    "comment",
    "cross-circ",
    "crossed-circle",
    "curv-trap",
    "curved-trapezoid",
    "cyl",
    "cylinder",
    "das",
    "database",
    "db",
    "dbl-circ",
    "decision",
    "defaultMindmapNode",
    "delay",
    "diam",
    "diamond",
    "disk",
    "display",
    "div-proc",
    "div-rect",
    "divided-process",
    "divided-rectangle",
    "doc",
    "docs",
    "document",
    "documents",
    "double-circle",
    "doublecircle",
    "erBox",
    "event",
    "extract",
    "f-circ",
    "filled-circle",
    "flag",
    "flip-tri",
    "flipped-triangle",
    "fork",
    "forkJoin",
    "fr-circ",
    "fr-rect",
    "framed-circle",
    "framed-rectangle",
    "h-cyl",
    "half-rounded-rectangle",
    "hex",
    "hexagon",
    "horizontal-cylinder",
    "hourglass",
    "icon",
    "iconCircle",
    "iconRounded",
    "iconSquare",
    "imageSquare",
    "in-out",
    "internal-storage",
    "inv-trapezoid",
    "inv_trapezoid",
    "join",
    "junction",
    "kanbanItem",
    "labelRect",
    "lean-l",
    "lean-left",
    "lean-r",
    "lean-right",
    "lean_left",
    "lean_right",
    "lightning-bolt",
    "lin-cyl",
    "lin-doc",
    "lin-proc",
    "lin-rect",
    "lined-cylinder",
    "lined-document",
    "lined-process",
    "lined-rectangle",
    "loop-limit",
    "manual",
    "manual-file",
    "manual-input",
    "mindmapCircle",
    "notch-pent",
    "notch-rect",
    "notched-pentagon",
    "notched-rectangle",
    "note",
    "odd",
    "out-in",
    "paper-tape",
    "pill",
    "prepare",
    "priority",
    "proc",
    "process",
    "processes",
    "procs",
    "question",
    "rect",
    "rectWithTitle",
    "rect_left_inv_arrow",
    "rectangle",
    "requirementBox",
    "rounded",
    "roundedRect",
    "shaded-process",
    "sl-rect",
    "sloped-rectangle",
    "sm-circ",
    "small-circle",
    "squareRect",
    "st-doc",
    "st-rect",
    "stacked-document",
    "stacked-rectangle",
    "stadium",
    "start",
    "state",
    "stateEnd",
    "stateStart",
    "stop",
    "stored-data",
    "subproc",
    "subprocess",
    "subroutine",
    "summary",
    "tag-doc",
    "tag-proc",
    "tag-rect",
    "tagged-document",
    "tagged-process",
    "tagged-rectangle",
    "terminal",
    "text",
    "trap-b",
    "trap-t",
    "trapezoid",
    "trapezoid-bottom",
    "trapezoid-top",
    "tri",
    "triangle",
    "win-pane",
    "window-pane",
];

fn is_valid_shape_11_12_2(shape: &str) -> bool {
    MERMAID_SHAPES_11_12_2.binary_search(&shape).is_ok()
}

fn yaml_to_string(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn yaml_to_bool(v: &serde_yaml::Value) -> Option<bool> {
    match v {
        serde_yaml::Value::Bool(b) => Some(*b),
        serde_yaml::Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn yaml_to_f64(v: &serde_yaml::Value) -> Option<f64> {
    match v {
        serde_yaml::Value::Number(n) => n.as_f64(),
        serde_yaml::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn apply_shape_data_to_node(node: &mut Node, yaml_body: &str) -> std::result::Result<(), String> {
    // If shapeData is attached to a node reference, Mermaid has already decided this is a node.
    let v = parse_shape_data_yaml(yaml_body)?;
    let map = match v.as_mapping() {
        Some(m) => m,
        None => return Ok(()),
    };

    let mut provided_label: Option<String> = None;
    for (k, v) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "shape" => {
                let Some(shape) = v.as_str() else { continue };
                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(format!(
                        "No such shape: {shape}. Shape names should be lowercase."
                    ));
                }
                if !is_valid_shape_11_12_2(shape) {
                    return Err(format!("No such shape: {shape}."));
                }
                node.shape = Some(shape.to_string());
            }
            "label" => {
                if let Some(label) = yaml_to_string(v) {
                    provided_label = Some(label.clone());
                    node.label = Some(label);
                    node.label_type = TitleKind::Text;
                }
            }
            "icon" => {
                if let Some(icon) = yaml_to_string(v) {
                    node.icon = Some(icon);
                }
            }
            "form" => {
                if let Some(form) = yaml_to_string(v) {
                    node.form = Some(form);
                }
            }
            "pos" => {
                if let Some(pos) = yaml_to_string(v) {
                    node.pos = Some(pos);
                }
            }
            "img" => {
                if let Some(img) = yaml_to_string(v) {
                    node.img = Some(img);
                }
            }
            "constraint" => {
                if let Some(constraint) = yaml_to_string(v) {
                    node.constraint = Some(constraint);
                }
            }
            "w" => {
                if let Some(w) = yaml_to_f64(v) {
                    node.asset_width = Some(w);
                }
            }
            "h" => {
                if let Some(h) = yaml_to_f64(v) {
                    node.asset_height = Some(h);
                }
            }
            _ => {}
        }
    }

    // Mermaid clears the default label when an icon or img is set without an explicit label.
    let has_visual = node.icon.is_some() || node.img.is_some();
    let label_is_empty_or_missing = provided_label
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if has_visual && label_is_empty_or_missing {
        let current_text = node.label.as_deref().unwrap_or(node.id.as_str());
        if current_text == node.id {
            node.label = Some(String::new());
            node.label_type = TitleKind::Text;
        }
    }

    Ok(())
}

fn count_char(ch: char, s: &str) -> usize {
    s.chars().filter(|&c| c == ch).count()
}

fn destruct_start_link(s: &str) -> (&'static str, &'static str) {
    let mut str = s.trim();
    let mut edge_type = "arrow_open";
    if let Some(first) = str.as_bytes().first().copied() {
        match first {
            b'<' => {
                edge_type = "arrow_point";
                str = &str[1..];
            }
            b'x' => {
                edge_type = "arrow_cross";
                str = &str[1..];
            }
            b'o' => {
                edge_type = "arrow_circle";
                str = &str[1..];
            }
            _ => {}
        }
    }

    let mut stroke = "normal";
    if str.contains('=') {
        stroke = "thick";
    }
    if str.contains('.') {
        stroke = "dotted";
    }
    (edge_type, stroke)
}

fn destruct_end_link(s: &str) -> (String, String, usize) {
    let str = s.trim();
    if str.len() < 2 {
        return ("arrow_open".to_string(), "normal".to_string(), 1);
    }
    let mut line = &str[..str.len() - 1];
    let mut edge_type = "arrow_open".to_string();

    match str.as_bytes()[str.len() - 1] {
        b'x' => {
            edge_type = "arrow_cross".to_string();
            if str.as_bytes().first().copied() == Some(b'x') {
                edge_type = format!("double_{edge_type}");
                line = &line[1..];
            }
        }
        b'>' => {
            edge_type = "arrow_point".to_string();
            if str.as_bytes().first().copied() == Some(b'<') {
                edge_type = format!("double_{edge_type}");
                line = &line[1..];
            }
        }
        b'o' => {
            edge_type = "arrow_circle".to_string();
            if str.as_bytes().first().copied() == Some(b'o') {
                edge_type = format!("double_{edge_type}");
                line = &line[1..];
            }
        }
        _ => {}
    }

    let mut stroke = "normal".to_string();
    let mut length = line.len().saturating_sub(1);

    if line.starts_with('=') {
        stroke = "thick".to_string();
    }
    if line.starts_with('~') {
        stroke = "invisible".to_string();
    }

    let dots = count_char('.', line);
    if dots > 0 {
        stroke = "dotted".to_string();
        length = dots;
    }

    (edge_type, stroke, length)
}

fn flow_subgraph_to_json(sg: FlowSubGraph) -> Value {
    json!({
        "id": sg.id,
        "nodes": sg.nodes,
        "title": sg.title,
        "classes": sg.classes,
        "styles": sg.styles,
        "dir": sg.dir,
        "labelType": sg.label_type,
    })
}

fn flow_subgraph_to_model(sg: FlowSubGraph) -> FlowSubgraph {
    FlowSubgraph {
        id: sg.id,
        nodes: sg.nodes,
        title: sg.title,
        classes: sg.classes,
        styles: sg.styles,
        dir: sg.dir,
        label_type: Some(sg.label_type),
    }
}

#[allow(dead_code)]
fn collect_nodes_and_edges(statements: &[Stmt], nodes: &mut Vec<Node>, edges: &mut Vec<Edge>) {
    for stmt in statements {
        match stmt {
            Stmt::Chain {
                nodes: chain_nodes,
                edges: chain_edges,
            } => {
                nodes.extend(chain_nodes.iter().cloned());
                edges.extend(chain_edges.iter().cloned());
            }
            Stmt::Node(n) => nodes.push(n.clone()),
            Stmt::Subgraph(sg) => collect_nodes_and_edges(&sg.statements, nodes, edges),
            Stmt::Direction(_) => {}
            Stmt::Style(_) => {}
            Stmt::ClassDef(_) => {}
            Stmt::ClassAssign(_) => {}
            Stmt::Click(_) => {}
            Stmt::LinkStyle(_) => {}
            Stmt::ShapeData { .. } => {}
        }
    }
}

#[allow(dead_code)]
fn merge_nodes_and_edges(nodes: Vec<Node>, edges: Vec<Edge>) -> (Vec<Node>, Vec<Edge>) {
    let mut nodes_by_id: HashMap<String, usize> = HashMap::new();
    let mut merged: Vec<Node> = Vec::new();
    for n in nodes {
        if let Some(&idx) = nodes_by_id.get(&n.id) {
            if n.label.is_some() {
                merged[idx].label = n.label;
                merged[idx].label_type = n.label_type.clone();
            }
            if n.shape.is_some() {
                merged[idx].shape = n.shape;
            }
            merged[idx].styles.extend(n.styles);
            merged[idx].classes.extend(n.classes);
            continue;
        }
        let idx = merged.len();
        nodes_by_id.insert(n.id.clone(), idx);
        merged.push(n);
    }
    (merged, edges)
}

fn title_kind_str(kind: &TitleKind) -> &'static str {
    match kind {
        TitleKind::Text => "text",
        TitleKind::String => "string",
        TitleKind::Markdown => "markdown",
    }
}

fn unquote(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        return s[1..s.len() - 1].to_string();
    }
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

fn parse_label_text(raw: &str) -> (String, TitleKind) {
    let trimmed = raw.trim();
    let quoted = (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''));
    let unquoted = if quoted {
        unquote(trimmed)
    } else {
        trimmed.to_string()
    };

    let (no_backticks, is_markdown) = strip_wrapping_backticks(unquoted.trim());
    if is_markdown {
        return (no_backticks, TitleKind::Markdown);
    }
    if quoted {
        return (unquoted, TitleKind::String);
    }
    (unquoted, TitleKind::Text)
}

fn strip_wrapping_backticks(s: &str) -> (String, bool) {
    let trimmed = s.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('`') && trimmed.ends_with('`') {
        return (trimmed[1..trimmed.len() - 1].to_string(), true);
    }
    (trimmed.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flowchart_subgraphs_exist_matches_mermaid_flowdb_spec() {
        let subgraphs = vec![
            FlowSubGraph {
                id: "sg0".to_string(),
                nodes: vec![
                    "a".to_string(),
                    "b".to_string(),
                    "c".to_string(),
                    "e".to_string(),
                ],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg1".to_string(),
                nodes: vec!["f".to_string(), "g".to_string(), "h".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg2".to_string(),
                nodes: vec!["i".to_string(), "j".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg3".to_string(),
                nodes: vec!["k".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
        ];

        assert!(super::subgraph::subgraphs_exist(&subgraphs, "a"));
        assert!(super::subgraph::subgraphs_exist(&subgraphs, "h"));
        assert!(super::subgraph::subgraphs_exist(&subgraphs, "j"));
        assert!(super::subgraph::subgraphs_exist(&subgraphs, "k"));

        assert!(!super::subgraph::subgraphs_exist(&subgraphs, "a2"));
        assert!(!super::subgraph::subgraphs_exist(&subgraphs, "l"));
    }
}
