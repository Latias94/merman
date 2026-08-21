use crate::diagram::{DiagramWarningFact, FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID};
use crate::sanitize::sanitize_text;
use crate::utils::format_url;
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

use super::{
    ClickAction, Edge, EdgeDefaults, FlowNodeProvenance, FlowNodeSyntax, FlowSubGraph,
    FlowSubgraphVertexStyle, FlowchartRenderStyleSources, LinkStylePos, Node, Stmt, TitleKind,
    apply_shape_data_value_to_node, value_to_bool, value_to_string,
};

pub(super) struct FlowchartSemanticContext<'a> {
    pub(super) nodes: &'a mut Vec<Node>,
    pub(super) node_index: &'a mut HashMap<String, usize>,
    pub(super) edges: &'a mut Vec<Edge>,
    pub(super) subgraphs: &'a mut Vec<FlowSubGraph>,
    pub(super) subgraph_vertex_styles: &'a mut FlowchartRenderStyleSources,
    pub(super) vertex_calls: &'a mut Vec<String>,
    pub(super) warning_facts: &'a mut Vec<DiagramWarningFact>,
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
        enum ReplayItem<'a> {
            Statement(&'a Stmt),
            FinishSubgraph,
        }

        let mut stack = statements
            .iter()
            .rev()
            .map(ReplayItem::Statement)
            .collect::<Vec<_>>();
        let mut visited = 0usize;
        let mut active_subgraphs = HashMap::new();
        let mut seen_vertex_ids = HashSet::new();
        let mut vertex_css = HashMap::new();
        let mut seen_edge_indices: HashMap<String, Vec<usize>> = HashMap::new();
        let mut next_built_edge_index = 0usize;
        let mut next_built_subgraph_index = 0usize;
        while let Some(item) = stack.pop() {
            if visited.is_multiple_of(128) {
                self.control.checkpoint()?;
            }
            visited = visited.saturating_add(1);

            let ReplayItem::Statement(stmt) = item else {
                let Some(subgraph) = self.subgraphs.get(next_built_subgraph_index) else {
                    return Ok(Err(Error::diagram_parse_fallback(
                        self.diagram_type.to_string(),
                        "flowchart subgraph replay diverged from the built model",
                    )));
                };
                active_subgraphs.insert(subgraph.id.clone(), next_built_subgraph_index);
                next_built_subgraph_index = next_built_subgraph_index.saturating_add(1);
                continue;
            };

            match stmt {
                Stmt::Subgraph(sg) => {
                    stack.push(ReplayItem::FinishSubgraph);
                    stack.extend(sg.statements.iter().rev().map(ReplayItem::Statement));
                }
                Stmt::Style(s) => {
                    if seen_edge_indices.contains_key(&s.target) {
                        continue;
                    }
                    self.vertex_calls.push(s.target.clone());
                    let is_new_vertex = seen_vertex_ids.insert(s.target.clone());
                    vertex_css
                        .entry(s.target.clone())
                        .or_insert_with(FlowSubgraphVertexStyle::default)
                        .styles
                        .extend(s.styles.iter().cloned());
                    if let Some(&idx) = active_subgraphs.get(&s.target) {
                        self.subgraphs[idx].styles.extend(s.styles.iter().cloned());
                    } else {
                        if is_new_vertex {
                            let mut warning = DiagramWarningFact::new(
                                FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID,
                                format!(
                                    "Style applied to unknown node \"{}\". This may indicate a typo. The node will be created automatically.",
                                    s.target
                                ),
                            );
                            if let Some(span) = s.target_span {
                                warning = warning.with_span(span);
                            }
                            self.warning_facts.push(warning);
                        }
                        let idx = self.ensure_authored_node(&s.target);
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
                        if let Some(style) = vertex_css.get_mut(target) {
                            style.classes.push(c.class_name.clone());
                        }
                        self.add_class_to_target(
                            target,
                            &c.class_name,
                            &active_subgraphs,
                            &seen_vertex_ids,
                            &seen_edge_indices,
                        )?;
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
                        if let Some(style) = vertex_css.get_mut(id) {
                            style.classes.push("clickable".to_string());
                        }
                        self.add_class_to_target(
                            id,
                            "clickable",
                            &active_subgraphs,
                            &seen_vertex_ids,
                            &seen_edge_indices,
                        )?;

                        match &c.action {
                            ClickAction::Link { href, target } => {
                                if seen_vertex_ids.contains(id)
                                    && let Some(&idx) = self.node_index.get(id)
                                {
                                    self.nodes[idx].link = format_url(href, self.config);
                                    self.nodes[idx].link_target = target.clone();
                                }
                            }
                            ClickAction::Callback => {
                                if self.security_level_loose
                                    && seen_vertex_ids.contains(id)
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
                                    if *i >= next_built_edge_index {
                                        return Ok(Err(Error::diagram_parse_fallback(
                                            self.diagram_type.to_string(),
                                            format!(
                                                "The index {i} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                                                next_built_edge_index.saturating_sub(1)
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
                                    if *i >= next_built_edge_index {
                                        return Ok(Err(Error::diagram_parse_fallback(
                                            self.diagram_type.to_string(),
                                            format!(
                                                "The index {i} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                                                next_built_edge_index.saturating_sub(1)
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
                    let value = match Self::shape_data_value(
                        self.shape_data_documents,
                        self.diagram_type,
                        yaml,
                    ) {
                        Ok(value) => value,
                        Err(error) => return Ok(Err(error)),
                    };
                    if let Some(indices) = seen_edge_indices.get(target) {
                        Self::apply_shape_data_to_edges(self.edges, self.control, indices, value)?;
                        continue;
                    }

                    self.vertex_calls.push(target.clone());
                    self.vertex_calls.push(target.clone());
                    seen_vertex_ids.insert(target.clone());
                    vertex_css.entry(target.clone()).or_default();
                    if active_subgraphs.contains_key(target) {
                        // An explicit shapeData statement upgrades an earlier routing anchor to
                        // an authored declaration. Keep the authored fields in the typed model;
                        // the renderer decides whether the visible projection remains
                        // group-first.
                        let idx = self.ensure_authored_node(target);
                        if let Err(error) =
                            apply_shape_data_value_to_node(&mut self.nodes[idx], value)
                        {
                            return Ok(Err(Error::diagram_parse_fallback(
                                self.diagram_type.to_string(),
                                error,
                            )));
                        }
                        continue;
                    }
                    let idx = self.ensure_authored_node(target);
                    if let Err(error) = apply_shape_data_value_to_node(&mut self.nodes[idx], value)
                    {
                        return Ok(Err(Error::diagram_parse_fallback(
                            self.diagram_type.to_string(),
                            error,
                        )));
                    }
                }
                Stmt::Chain {
                    node_groups,
                    edge_groups,
                } => {
                    if let Some(first_group) = node_groups.first()
                        && let Err(error) = self.observe_node_group(
                            first_group,
                            &active_subgraphs,
                            &mut seen_vertex_ids,
                            &mut vertex_css,
                            &seen_edge_indices,
                        )?
                    {
                        return Ok(Err(error));
                    }
                    for (segment_index, edge_group) in edge_groups.iter().enumerate() {
                        if let Some(next_group) = node_groups.get(segment_index + 1)
                            && let Err(error) = self.observe_node_group(
                                next_group,
                                &active_subgraphs,
                                &mut seen_vertex_ids,
                                &mut vertex_css,
                                &seen_edge_indices,
                            )?
                        {
                            return Ok(Err(error));
                        }
                        if let Err(error) = self.activate_edges(
                            edge_group.len(),
                            &mut next_built_edge_index,
                            &mut seen_edge_indices,
                        )? {
                            return Ok(Err(error));
                        }
                    }
                }
                Stmt::Node(node) => {
                    if let Err(error) = self.observe_node_group(
                        std::slice::from_ref(node.as_ref()),
                        &active_subgraphs,
                        &mut seen_vertex_ids,
                        &mut vertex_css,
                        &seen_edge_indices,
                    )? {
                        return Ok(Err(error));
                    }
                }
                Stmt::Direction(_) => {}
            }
        }

        if next_built_edge_index != self.edges.len()
            || next_built_subgraph_index != self.subgraphs.len()
        {
            return Ok(Err(Error::diagram_parse_fallback(
                self.diagram_type.to_string(),
                "flowchart semantic replay did not consume the built model",
            )));
        }
        for (index, (id, style)) in vertex_css.into_iter().enumerate() {
            if index % 128 == 0 {
                self.control.checkpoint()?;
            }
            if active_subgraphs.contains_key(&id) {
                *self.subgraph_vertex_styles.entry_mut(id) = style;
            }
        }
        self.control.checkpoint()?;
        Ok(Ok(()))
    }

    fn observe_node_group(
        &mut self,
        nodes: &[Node],
        active_subgraphs: &HashMap<String, usize>,
        seen_vertex_ids: &mut HashSet<String>,
        vertex_css: &mut HashMap<String, FlowSubgraphVertexStyle>,
        seen_edge_indices: &HashMap<String, Vec<usize>>,
    ) -> OperationControlResult<Result<()>> {
        let mut deferred_shape_data_calls = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            if index % 128 == 0 {
                self.control.checkpoint()?;
            }
            if let Some(indices) = seen_edge_indices.get(&node.id) {
                if let Some(yaml) = node.shape_data.as_deref() {
                    let value = match Self::shape_data_value(
                        self.shape_data_documents,
                        self.diagram_type,
                        yaml,
                    ) {
                        Ok(value) => value,
                        Err(error) => return Ok(Err(error)),
                    };
                    Self::apply_shape_data_to_edges(self.edges, self.control, indices, value)?;
                }
                continue;
            }

            self.vertex_calls.push(node.id.clone());
            seen_vertex_ids.insert(node.id.clone());
            let style = vertex_css.entry(node.id.clone()).or_default();
            style.classes.extend(node.classes.iter().cloned());
            style.styles.extend(node.styles.iter().cloned());
            if let Some(yaml) = node.shape_data.as_deref() {
                deferred_shape_data_calls.push(node.id.clone());
                let value = match Self::shape_data_value(
                    self.shape_data_documents,
                    self.diagram_type,
                    yaml,
                ) {
                    Ok(value) => value,
                    Err(error) => return Ok(Err(error)),
                };
                if !active_subgraphs.contains_key(&node.id) {
                    let idx = self.ensure_authored_node(&node.id);
                    if let Err(error) = apply_shape_data_value_to_node(&mut self.nodes[idx], value)
                    {
                        return Ok(Err(Error::diagram_parse_fallback(
                            self.diagram_type.to_string(),
                            error,
                        )));
                    }
                } else {
                    // Preserve the authored shapeData in the model while leaving visibility to
                    // the group-first projection layer.
                    let idx = self.ensure_authored_node(&node.id);
                    if let Err(error) = apply_shape_data_value_to_node(&mut self.nodes[idx], value)
                    {
                        return Ok(Err(Error::diagram_parse_fallback(
                            self.diagram_type.to_string(),
                            error,
                        )));
                    }
                }
            }
        }
        for (index, id) in deferred_shape_data_calls.into_iter().enumerate() {
            if index % 128 == 0 {
                self.control.checkpoint()?;
            }
            self.vertex_calls.push(id);
        }
        Ok(Ok(()))
    }

    fn add_class_to_target(
        &mut self,
        target: &str,
        class_name: &str,
        active_subgraphs: &HashMap<String, usize>,
        seen_vertex_ids: &HashSet<String>,
        seen_edge_indices: &HashMap<String, Vec<usize>>,
    ) -> OperationControlResult<()> {
        if let Some(&idx) = active_subgraphs.get(target) {
            self.subgraphs[idx].classes.push(class_name.to_string());
        }
        if seen_vertex_ids.contains(target)
            && let Some(&idx) = self.node_index.get(target)
        {
            self.nodes[idx].classes.push(class_name.to_string());
        }
        if let Some(edge_indices) = seen_edge_indices.get(target) {
            for (index, edge_index) in edge_indices.iter().copied().enumerate() {
                if index % 128 == 0 {
                    self.control.checkpoint()?;
                }
                if let Some(edge) = self.edges.get_mut(edge_index) {
                    edge.classes.push(class_name.to_string());
                }
            }
        } else {
            // Preserve the canonical cancellation cadence even when the requested edge id is
            // absent. The previous implementation scanned every built edge in this case.
            for index in 0..self.edges.len() {
                if index % 128 == 0 {
                    self.control.checkpoint()?;
                }
            }
        }
        Ok(())
    }

    fn activate_edges(
        &self,
        count: usize,
        next_edge_index: &mut usize,
        seen_edge_indices: &mut HashMap<String, Vec<usize>>,
    ) -> OperationControlResult<Result<()>> {
        let Some(edge_end) = next_edge_index.checked_add(count) else {
            return Ok(Err(Error::diagram_parse_fallback(
                self.diagram_type.to_string(),
                "flowchart edge replay index overflow",
            )));
        };
        if edge_end > self.edges.len() {
            return Ok(Err(Error::diagram_parse_fallback(
                self.diagram_type.to_string(),
                "flowchart edge replay diverged from the built model",
            )));
        }
        for (index, edge_index) in (*next_edge_index..edge_end).enumerate() {
            if index % 128 == 0 {
                self.control.checkpoint()?;
            }
            if let Some(id) = self.edges[edge_index].id.as_deref() {
                seen_edge_indices
                    .entry(id.to_string())
                    .or_default()
                    .push(edge_index);
            }
        }
        *next_edge_index = edge_end;
        Ok(Ok(()))
    }

    fn shape_data_value<'b>(
        documents: &'b HashMap<String, std::result::Result<serde_json::Value, String>>,
        diagram_type: &str,
        yaml: &str,
    ) -> Result<&'b serde_json::Value> {
        match documents
            .get(yaml)
            .expect("flowchart shape data must be prepared before semantic construction")
        {
            Ok(document) => Ok(document),
            Err(error) => Err(Error::diagram_parse_fallback(
                diagram_type.to_string(),
                format!("Invalid shapeData: {error}"),
            )),
        }
    }

    fn apply_shape_data_to_edges(
        edges: &mut [Edge],
        control: &OperationControl,
        indices: &[usize],
        value: &serde_json::Value,
    ) -> OperationControlResult<()> {
        let Some(map) = value.as_object() else {
            return Ok(());
        };
        for (index, edge_index) in indices.iter().copied().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let Some(edge) = edges.get_mut(edge_index) else {
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
        Ok(())
    }

    fn ensure_authored_node(&mut self, id: &str) -> usize {
        if let Some(&idx) = self.node_index.get(id) {
            self.nodes[idx].provenance = FlowNodeProvenance::Authored;
            self.nodes[idx].syntax = FlowNodeSyntax::ExplicitDefinition;
            return idx;
        }
        let idx = self.nodes.len();
        self.nodes.push(Node {
            id: id.to_string(),
            provenance: FlowNodeProvenance::Authored,
            syntax: FlowNodeSyntax::ExplicitDefinition,
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
        let mut subgraph_vertex_styles = FlowchartRenderStyleSources::default();
        let mut vertex_calls = Vec::new();
        let mut warning_facts = Vec::new();
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
            subgraph_vertex_styles: &mut subgraph_vertex_styles,
            vertex_calls: &mut vertex_calls,
            warning_facts: &mut warning_facts,
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
        })];

        assert!(matches!(
            apply_semantic_statements(&statements, &mut context),
            Err(crate::OperationCancelled { .. })
        ));
    }
}
