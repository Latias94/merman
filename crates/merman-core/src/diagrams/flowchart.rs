use crate::sanitize::sanitize_text;
use crate::utils::format_url;
use crate::{Error, MermaidConfig, ParseMetadata, Result};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

lalrpop_util::lalrpop_mod!(
    #[allow(clippy::type_complexity, clippy::result_large_err)]
    flowchart_grammar,
    "/diagrams/flowchart_grammar.rs"
);

mod ast;
mod lex;
mod lexer;
mod lexer_iter;
mod model;
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

use lexer::Lexer;

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
    let _ = builder.eval_statements(&ast.statements);

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
    let _ = builder.eval_statements(&ast.statements);

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

fn extract_flowchart_accessibility_statements(
    code: &str,
) -> (String, Option<String>, Option<String>) {
    let mut acc_title: Option<String> = None;
    let mut acc_descr: Option<String> = None;
    let mut out = String::with_capacity(code.len());

    let mut lines = code.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix("accTitle") {
            let rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix(':') {
                acc_title = Some(after.trim().to_string());
                continue;
            }
        }

        if let Some(rest) = trimmed.strip_prefix("accDescr") {
            let rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix(':') {
                acc_descr = Some(after.trim().to_string());
                continue;
            }

            if let Some(after_lbrace) = rest.strip_prefix('{') {
                let mut buf = String::new();

                let mut after = after_lbrace.to_string();
                if let Some(end) = after.find('}') {
                    after.truncate(end);
                    acc_descr = Some(after.trim().to_string());
                    continue;
                }
                let after = after.trim_start();
                if !after.is_empty() {
                    buf.push_str(after);
                }

                for raw in lines.by_ref() {
                    if let Some(pos) = raw.find('}') {
                        let part = &raw[..pos];
                        if !buf.is_empty() {
                            buf.push('\n');
                        }
                        buf.push_str(part);
                        break;
                    }

                    if !buf.is_empty() {
                        buf.push('\n');
                    }
                    buf.push_str(raw);
                }

                acc_descr = Some(buf.trim().to_string());
                continue;
            }
        }

        out.push_str(line);
        out.push('\n');
    }

    (out, acc_title, acc_descr)
}

struct FlowchartBuildState {
    nodes: Vec<Node>,
    node_index: HashMap<String, usize>,
    edges: Vec<Edge>,
    used_edge_ids: HashSet<String>,
    edge_pair_counts: HashMap<(String, String), usize>,
    vertex_calls: Vec<String>,
}

impl FlowchartBuildState {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            node_index: HashMap::new(),
            edges: Vec::new(),
            used_edge_ids: HashSet::new(),
            edge_pair_counts: HashMap::new(),
            vertex_calls: Vec::new(),
        }
    }

    fn add_statements(&mut self, statements: &[Stmt]) -> std::result::Result<(), String> {
        for stmt in statements {
            match stmt {
                Stmt::Chain { nodes, edges } => {
                    let mut deferred_shape_data_vertex_calls: Vec<String> = Vec::new();
                    for mut n in nodes.iter().cloned() {
                        // Mermaid FlowDB `vertexCounter` increments on every `addVertex(...)` call.
                        // Our grammar models `shapeData` attachments in the AST, so we can replay the
                        // observable call sequence:
                        // - once for the vertex token itself
                        // - once more if a `@{ ... }` shapeData block is present
                        self.vertex_calls.push(n.id.clone());
                        if n.shape_data.is_some() {
                            // For multi-vertex statements (notably `&`-separated nodes), the upstream
                            // parser's reduction order can apply shapeData after the statement's
                            // vertices have already been introduced. Record these shapeData calls
                            // after we've visited every vertex in the statement.
                            deferred_shape_data_vertex_calls.push(n.id.clone());
                        }
                        if let Some(sd) = n.shape_data.take() {
                            apply_shape_data_to_node(&mut n, &sd)?;
                        }
                        self.upsert_node(n);
                    }
                    self.vertex_calls
                        .extend(deferred_shape_data_vertex_calls.into_iter());
                    for e in edges.iter().cloned() {
                        self.push_edge(e);
                    }
                }
                Stmt::Node(n) => {
                    let mut n = n.clone();
                    self.vertex_calls.push(n.id.clone());
                    if n.shape_data.is_some() {
                        self.vertex_calls.push(n.id.clone());
                    }
                    if let Some(sd) = n.shape_data.take() {
                        apply_shape_data_to_node(&mut n, &sd)?;
                    }
                    self.upsert_node(n);
                }
                Stmt::ShapeData { target, .. } => {
                    // Mermaid applies shapeData to edges if (and only if) an edge with that ID exists.
                    // For ordering parity we only insert a placeholder node when this currently refers to a node.
                    if !self.used_edge_ids.contains(target) {
                        // The upstream flowchart parser calls `addVertex(id)` and then
                        // `addVertex(id, ..., shapeData)` for `id@{...}` statements.
                        self.vertex_calls.push(target.clone());
                        self.vertex_calls.push(target.clone());
                    }
                    if !self.used_edge_ids.contains(target) && !self.node_index.contains_key(target)
                    {
                        let idx = self.nodes.len();
                        self.nodes.push(Node {
                            id: target.clone(),
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
                        self.node_index.insert(target.clone(), idx);
                    }
                }
                Stmt::Subgraph(sg) => self.add_statements(&sg.statements)?,
                Stmt::Direction(_)
                | Stmt::ClassDef(_)
                | Stmt::ClassAssign(_)
                | Stmt::Click(_)
                | Stmt::LinkStyle(_) => {}
                Stmt::Style(s) => {
                    // Mermaid's `style` statement routes through FlowDB `addVertex(id, ..., styles)`.
                    // This increments `vertexCounter` for nodes (but is a no-op for edges).
                    if !self.used_edge_ids.contains(&s.target) {
                        self.vertex_calls.push(s.target.clone());
                        if !self.node_index.contains_key(&s.target) {
                            let idx = self.nodes.len();
                            self.nodes.push(Node {
                                id: s.target.clone(),
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
                            self.node_index.insert(s.target.clone(), idx);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn upsert_node(&mut self, n: Node) {
        if let Some(&idx) = self.node_index.get(&n.id) {
            if n.label.is_some() {
                self.nodes[idx].label = n.label;
                self.nodes[idx].label_type = n.label_type;
            }
            if n.shape.is_some() {
                self.nodes[idx].shape = n.shape;
            }
            if n.icon.is_some() {
                self.nodes[idx].icon = n.icon;
            }
            if n.form.is_some() {
                self.nodes[idx].form = n.form;
            }
            if n.pos.is_some() {
                self.nodes[idx].pos = n.pos;
            }
            if n.img.is_some() {
                self.nodes[idx].img = n.img;
            }
            if n.constraint.is_some() {
                self.nodes[idx].constraint = n.constraint;
            }
            if n.asset_width.is_some() {
                self.nodes[idx].asset_width = n.asset_width;
            }
            if n.asset_height.is_some() {
                self.nodes[idx].asset_height = n.asset_height;
            }
            self.nodes[idx].styles.extend(n.styles);
            self.nodes[idx].classes.extend(n.classes);
            return;
        }
        let idx = self.nodes.len();
        self.node_index.insert(n.id.clone(), idx);
        self.nodes.push(n);
    }

    fn push_edge(&mut self, mut e: Edge) {
        let key = (e.from.clone(), e.to.clone());
        let existing = *self.edge_pair_counts.get(&key).unwrap_or(&0);

        let mut final_id = e.id.clone();
        let mut is_user_defined_id = false;
        if let Some(user_id) = e.id.clone() {
            if !self.used_edge_ids.contains(&user_id) {
                is_user_defined_id = true;
                self.used_edge_ids.insert(user_id);
            } else {
                final_id = None;
            }
        }

        if final_id.is_none() {
            let counter = if existing == 0 { 0 } else { existing + 1 };
            final_id = Some(format!("L_{}_{}_{}", e.from, e.to, counter));
            if let Some(id) = final_id.clone() {
                self.used_edge_ids.insert(id);
            }
        }

        self.edge_pair_counts.insert(key, existing + 1);

        e.id = final_id;
        e.is_user_defined_id = is_user_defined_id;
        e.link.length = e.link.length.min(10);
        self.edges.push(e);
    }
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

#[derive(Debug, Clone)]
enum StatementItem {
    Id(String),
    Dir(String),
}

struct SubgraphBuilder {
    sub_count: usize,
    subgraphs: Vec<FlowSubGraph>,
    inherit_dir: bool,
    global_dir: Option<String>,
}

impl SubgraphBuilder {
    fn new(inherit_dir: bool, global_dir: Option<String>) -> Self {
        Self {
            sub_count: 0,
            subgraphs: Vec::new(),
            inherit_dir,
            global_dir,
        }
    }

    fn eval_statements(&mut self, statements: &[Stmt]) -> Vec<StatementItem> {
        let mut out: Vec<StatementItem> = Vec::new();
        for stmt in statements {
            match stmt {
                Stmt::Chain { nodes, edges } => {
                    // Mermaid FlowDB's subgraph membership list is based on the Jison `vertexStatement.nodes`
                    // shape, which prepends the last node in a chain first (e.g. `a-->b` yields `[b, a]`).
                    //
                    // For node-only group statements (e.g. `A & B`), there are no edges and the list
                    // preserves the input order.
                    if edges.is_empty() {
                        for n in nodes {
                            out.push(StatementItem::Id(n.id.clone()));
                        }
                    } else {
                        for n in nodes.iter().rev() {
                            out.push(StatementItem::Id(n.id.clone()));
                        }
                    }
                }
                Stmt::Node(n) => out.push(StatementItem::Id(n.id.clone())),
                Stmt::Direction(d) => out.push(StatementItem::Dir(d.clone())),
                Stmt::Subgraph(sg) => {
                    let id = self.eval_subgraph(sg);
                    out.push(StatementItem::Id(id));
                }
                Stmt::Style(_) => {}
                Stmt::ClassDef(_) => {}
                Stmt::ClassAssign(_) => {}
                Stmt::Click(_) => {}
                Stmt::LinkStyle(_) => {}
                Stmt::ShapeData { .. } => {}
            }
        }
        out
    }

    fn eval_subgraph(&mut self, sg: &SubgraphBlock) -> String {
        let items = self.eval_statements(&sg.statements);
        let mut seen: HashSet<String> = HashSet::new();
        let mut members: Vec<String> = Vec::new();
        let mut dir: Option<String> = None;

        for item in items {
            match item {
                StatementItem::Dir(d) => dir = Some(d),
                StatementItem::Id(id) => {
                    if id.trim().is_empty() {
                        continue;
                    }
                    if seen.insert(id.clone()) {
                        members.push(id);
                    }
                }
            }
        }

        let dir = dir.or_else(|| {
            if self.inherit_dir {
                self.global_dir.clone()
            } else {
                None
            }
        });

        let raw_id = unquote(&sg.header.raw_id);
        let (title_raw, title_kind) =
            parse_subgraph_title(&sg.header.raw_title, sg.header.id_equals_title);
        let id_raw = strip_wrapping_backticks(raw_id.trim()).0;

        let mut id: Option<String> = {
            let trimmed = id_raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        };

        // Mirror Mermaid `FlowDB.addSubGraph(...)`:
        // `if (_id === _title && /\\s/.exec(_title.text)) id = undefined;`
        //
        // The important nuance is that this checks the untrimmed title token (including any
        // extra whitespace that may have been captured into the header).
        if sg.header.id_equals_title && sg.header.raw_title.chars().any(|c| c.is_whitespace()) {
            id = None;
        }

        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        let title = title_raw.trim().to_string();
        let label_type = match title_kind {
            TitleKind::Text => "text",
            TitleKind::String => "string",
            TitleKind::Markdown => "markdown",
        }
        .to_string();

        self.sub_count += 1;

        members.retain(|m| !subgraphs_exist(&self.subgraphs, m));

        self.subgraphs.push(FlowSubGraph {
            id: id.clone(),
            nodes: members,
            title,
            classes: Vec::new(),
            styles: Vec::new(),
            dir,
            label_type,
        });

        id
    }
}

fn subgraphs_exist(subgraphs: &[FlowSubGraph], node_id: &str) -> bool {
    subgraphs
        .iter()
        .any(|sg| sg.nodes.iter().any(|n| n == node_id))
}

fn parse_subgraph_title(raw_title: &str, id_equals_title: bool) -> (String, TitleKind) {
    let trimmed = raw_title.trim();
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

    if !id_equals_title && quoted {
        return (unquoted, TitleKind::String);
    }

    (unquoted, TitleKind::Text)
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

#[allow(clippy::too_many_arguments)]
fn apply_semantic_statements(
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

        assert!(subgraphs_exist(&subgraphs, "a"));
        assert!(subgraphs_exist(&subgraphs, "h"));
        assert!(subgraphs_exist(&subgraphs, "j"));
        assert!(subgraphs_exist(&subgraphs, "k"));

        assert!(!subgraphs_exist(&subgraphs, "a2"));
        assert!(!subgraphs_exist(&subgraphs, "l"));
    }
}
