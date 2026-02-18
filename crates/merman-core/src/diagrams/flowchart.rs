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
mod link;
mod model;
mod semantic;
mod shape_data;
mod subgraph;
mod text;
mod tokens;

use text::{parse_label_text, strip_wrapping_backticks, title_kind_str, unquote};

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
use link::{destruct_end_link, destruct_start_link};
use semantic::apply_semantic_statements;
use shape_data::{apply_shape_data_to_node, parse_shape_data_yaml, yaml_to_bool, yaml_to_string};
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
