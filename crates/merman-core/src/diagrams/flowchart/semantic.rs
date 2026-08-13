use crate::sanitize::sanitize_text;
use crate::utils::format_url;
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result};
use indexmap::IndexMap;
use std::collections::HashMap;

use super::{
    ClickAction, Edge, EdgeDefaults, FlowSubGraph, LinkStylePos, Node, Stmt, TitleKind,
    apply_shape_data_value_to_node, value_to_bool, value_to_string,
};

pub(super) struct FlowchartSemanticContext<'a> {
    pub(super) nodes: &'a mut Vec<Node>,
    pub(super) node_index: &'a mut HashMap<String, usize>,
    pub(super) edges: &'a mut Vec<Edge>,
    pub(super) subgraphs: &'a mut Vec<FlowSubGraph>,
    pub(super) subgraph_index: &'a mut HashMap<String, usize>,
    pub(super) class_defs: &'a mut IndexMap<String, Vec<String>>,
    pub(super) tooltips: &'a mut HashMap<String, String>,
    pub(super) edge_defaults: &'a mut EdgeDefaults,
    pub(super) security_level_loose: bool,
    pub(super) diagram_type: &'a str,
    pub(super) config: &'a MermaidConfig,
    pub(super) shape_data_documents:
        &'a HashMap<String, std::result::Result<serde_json::Value, String>>,
    pub(super) control: &'a OperationControl,
}

pub(super) fn apply_semantic_statements(
    statements: &[Stmt],
    ctx: &mut FlowchartSemanticContext<'_>,
) -> OperationControlResult<Result<()>> {
    ctx.apply_statements(statements)
}

impl<'a> FlowchartSemanticContext<'a> {
    fn apply_statements(&mut self, statements: &[Stmt]) -> OperationControlResult<Result<()>> {
        // Preserve the recursive preorder semantics while avoiding stack growth on nested
        // subgraphs.
        let mut stack = vec![statements.iter()];
        let mut visited = 0usize;
        while let Some(iter) = stack.last_mut() {
            let Some(stmt) = iter.next() else {
                stack.pop();
                continue;
            };
            if visited.is_multiple_of(128) {
                self.control.checkpoint()?;
            }
            visited = visited.saturating_add(1);

            match stmt {
                Stmt::Subgraph(sg) => stack.push(sg.statements.iter()),
                Stmt::Style(s) => {
                    if let Some(&idx) = self.subgraph_index.get(&s.target) {
                        self.subgraphs[idx].styles.extend(s.styles.iter().cloned());
                    } else {
                        let idx = self.ensure_node(&s.target);
                        self.nodes[idx].styles.extend(s.styles.iter().cloned());
                    }
                }
                Stmt::ClassDef(c) => {
                    for (index, id) in c.ids.iter().enumerate() {
                        if index % 128 == 0 {
                            self.control.checkpoint()?;
                        }
                        self.class_defs.insert(id.clone(), c.styles.clone());
                    }
                }
                Stmt::ClassAssign(c) => {
                    for (index, target) in c.targets.iter().enumerate() {
                        if index % 128 == 0 {
                            self.control.checkpoint()?;
                        }
                        self.add_class_to_target(target, &c.class_name)?;
                    }
                }
                Stmt::Click(c) => {
                    for (index, id) in c.ids.iter().enumerate() {
                        if index % 128 == 0 {
                            self.control.checkpoint()?;
                        }
                        if let Some(tt) = &c.tooltip {
                            self.tooltips
                                .insert(id.clone(), sanitize_text(tt, self.config));
                        }
                        self.add_class_to_target(id, "clickable")?;

                        match &c.action {
                            ClickAction::Link { href, target } => {
                                if let Some(&idx) = self.node_index.get(id) {
                                    self.nodes[idx].link = format_url(href, self.config);
                                    self.nodes[idx].link_target = target.clone();
                                }
                            }
                            ClickAction::Callback => {
                                if self.security_level_loose
                                    && let Some(&idx) = self.node_index.get(id)
                                {
                                    self.nodes[idx].have_callback = true;
                                }
                            }
                        }
                    }
                }
                Stmt::LinkStyle(ls) => {
                    if let Some(algo) = &ls.interpolate {
                        for (index, pos) in ls.positions.iter().enumerate() {
                            if index % 128 == 0 {
                                self.control.checkpoint()?;
                            }
                            match pos {
                                LinkStylePos::Default => {
                                    self.edge_defaults.interpolate = Some(algo.clone())
                                }
                                LinkStylePos::Index(i) => {
                                    if *i >= self.edges.len() {
                                        return Ok(Err(Error::diagram_parse_fallback(
                                            self.diagram_type.to_string(),
                                            format!(
                                                "The index {i} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                                                self.edges.len().saturating_sub(1)
                                            ),
                                        )));
                                    }
                                    self.edges[*i].interpolate = Some(algo.clone());
                                }
                            }
                        }
                    }

                    if !ls.styles.is_empty() {
                        for (index, pos) in ls.positions.iter().enumerate() {
                            if index % 128 == 0 {
                                self.control.checkpoint()?;
                            }
                            match pos {
                                LinkStylePos::Default => {
                                    self.edge_defaults.style = ls.styles.clone()
                                }
                                LinkStylePos::Index(i) => {
                                    if *i >= self.edges.len() {
                                        return Ok(Err(Error::diagram_parse_fallback(
                                            self.diagram_type.to_string(),
                                            format!(
                                                "The index {i} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                                                self.edges.len().saturating_sub(1)
                                            ),
                                        )));
                                    }
                                    self.edges[*i].style = ls.styles.clone();
                                    if !self.edges[*i].style.is_empty()
                                        && !self.edges[*i]
                                            .style
                                            .iter()
                                            .any(|s| s.trim_start().starts_with("fill"))
                                    {
                                        self.edges[*i].style.push("fill:none".to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                Stmt::ShapeData { target, yaml, .. } => {
                    // Mermaid syntax uses the same `@{...}` form for both nodes and edges:
                    // - if an edge with the given ID exists, it updates the edge metadata
                    // - otherwise it updates (and may create) a node
                    let v = match self.shape_data_documents.get(yaml).expect(
                        "flowchart shape data must be prepared before semantic construction",
                    ) {
                        Ok(document) => document,
                        Err(error) => {
                            return Ok(Err(Error::diagram_parse_fallback(
                                self.diagram_type.to_string(),
                                format!("Invalid shapeData: {error}"),
                            )));
                        }
                    };

                    let map = v.as_object();
                    let mut is_edge_target = false;
                    for (index, edge) in self.edges.iter_mut().enumerate() {
                        if index % 128 == 0 {
                            self.control.checkpoint()?;
                        }
                        if edge.id.as_deref() != Some(target.as_str()) {
                            continue;
                        }
                        is_edge_target = true;
                        let Some(map) = map else {
                            continue;
                        };
                        for (key, value) in map {
                            match key.as_str() {
                                "animate" => {
                                    if let Some(value) = value_to_bool(value) {
                                        edge.animate = Some(value);
                                    }
                                }
                                "animation" => {
                                    if let Some(value) = value_to_string(value) {
                                        edge.animation = Some(value);
                                    }
                                }
                                "curve" => {
                                    if let Some(value) = value_to_string(value) {
                                        edge.interpolate = Some(value);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    if is_edge_target {
                        continue;
                    }

                    let idx = self.ensure_node(target);
                    if let Err(error) = apply_shape_data_value_to_node(&mut self.nodes[idx], v) {
                        return Ok(Err(Error::diagram_parse_fallback(
                            self.diagram_type.to_string(),
                            error,
                        )));
                    }
                }
                Stmt::Chain { .. } | Stmt::Node(_) | Stmt::Direction(_) => {}
            }
        }
        self.control.checkpoint()?;
        Ok(Ok(()))
    }

    fn add_class_to_target(
        &mut self,
        target: &str,
        class_name: &str,
    ) -> OperationControlResult<()> {
        if let Some(&idx) = self.subgraph_index.get(target) {
            self.subgraphs[idx].classes.push(class_name.to_string());
        }
        if let Some(&idx) = self.node_index.get(target) {
            self.nodes[idx].classes.push(class_name.to_string());
        }
        for (index, edge) in self.edges.iter_mut().enumerate() {
            if index % 128 == 0 {
                self.control.checkpoint()?;
            }
            if edge.id.as_deref() == Some(target) {
                edge.classes.push(class_name.to_string());
            }
        }
        Ok(())
    }

    fn ensure_node(&mut self, id: &str) -> usize {
        if let Some(&idx) = self.node_index.get(id) {
            return idx;
        }
        let idx = self.nodes.len();
        self.nodes.push(Node {
            id: id.to_string(),
            id_span: None,
            label: None,
            label_type: TitleKind::Text,
            label_span: None,
            label_selection: None,
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
        self.node_index.insert(id.to_string(), idx);
        idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::flowchart::{
        ClassAssignStmt, FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, LinkToken,
    };

    #[test]
    fn class_assignment_can_cancel_during_edge_scanning() {
        let mut nodes = Vec::new();
        let mut node_index = HashMap::new();
        let mut edges = (0..512)
            .map(|index| Edge {
                from: format!("n{index}"),
                to: format!("n{}", index + 1),
                id: Some(format!("edge-{index}")),
                link: LinkToken {
                    end: "arrow_point".to_string(),
                    start_marker: FlowEdgeMarker::None,
                    end_marker: FlowEdgeMarker::Point,
                    stroke_kind: FlowEdgeStroke::Normal,
                    visibility: FlowEdgeVisibility::Visible,
                    length: 1,
                },
                label: None,
                label_type: TitleKind::Text,
                label_span: None,
                label_selection: None,
                style: Vec::new(),
                classes: Vec::new(),
                interpolate: None,
                is_user_defined_id: true,
                animate: None,
                animation: None,
            })
            .collect::<Vec<_>>();
        let mut subgraphs = Vec::new();
        let mut subgraph_index = HashMap::new();
        let mut class_defs = IndexMap::new();
        let mut tooltips = HashMap::new();
        let mut edge_defaults = EdgeDefaults {
            style: Vec::new(),
            interpolate: None,
        };
        let config = MermaidConfig::empty_object();
        let shape_data_documents = HashMap::new();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let mut context = FlowchartSemanticContext {
            nodes: &mut nodes,
            node_index: &mut node_index,
            edges: &mut edges,
            subgraphs: &mut subgraphs,
            subgraph_index: &mut subgraph_index,
            class_defs: &mut class_defs,
            tooltips: &mut tooltips,
            edge_defaults: &mut edge_defaults,
            security_level_loose: false,
            diagram_type: "flowchart-v2",
            config: &config,
            shape_data_documents: &shape_data_documents,
            control: &control,
        };
        let statements = [Stmt::ClassAssign(ClassAssignStmt {
            targets: vec!["missing-edge".to_string()],
            target_spans: Vec::new(),
            class_name: "hot".to_string(),
            class_name_span: None,
            editor_evidence: Default::default(),
            lexeme_components: Vec::new(),
        })];

        assert!(matches!(
            apply_semantic_statements(&statements, &mut context),
            Err(crate::OperationCancelled { .. })
        ));
    }
}
