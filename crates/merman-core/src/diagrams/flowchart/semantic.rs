use crate::sanitize::sanitize_text;
use crate::utils::format_url;
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result};
use indexmap::IndexMap;
use merman_model::warnings::{DiagramWarningFact, FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID};
use std::collections::{HashMap, HashSet};

use super::{
    ClickAction, Edge, EdgeDefaults, FlowNodeProvenance, FlowNodeSyntax, FlowSubGraph,
    FlowchartAst, LinkStylePos, Node, StatementEvent, Stmt, TitleKind,
    apply_shape_data_value_to_node, value_to_bool, value_to_string,
};
use super::{FlowSubgraphVertexStyle, FlowchartSubgraphStyles};

pub(super) struct FlowchartSemanticContext<'a> {
    pub(super) nodes: &'a mut Vec<Node>,
    pub(super) node_index: &'a mut HashMap<String, usize>,
    pub(super) edges: &'a mut Vec<Edge>,
    pub(super) subgraphs: &'a mut Vec<FlowSubGraph>,
    pub(super) subgraph_declaration_owners: &'a [usize],
    pub(super) subgraph_vertex_styles: &'a mut FlowchartSubgraphStyles,
    #[cfg(feature = "presentation")]
    pub(super) collapsed_subgraphs: Option<&'a mut rustc_hash::FxHashSet<String>>,
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
    ast: &FlowchartAst,
    ctx: &mut FlowchartSemanticContext<'_>,
) -> OperationControlResult<Result<()>> {
    ctx.apply_statements(ast)
}

impl<'a> FlowchartSemanticContext<'a> {
    fn apply_statements(&mut self, ast: &FlowchartAst) -> OperationControlResult<Result<()>> {
        let mut active_subgraphs = HashMap::new();
        let mut seen_vertex_ids = HashSet::new();
        let mut vertex_css: HashMap<String, Option<FlowSubgraphVertexStyle>> =
            HashMap::with_capacity(self.subgraphs.len());
        for (index, subgraph) in self.subgraphs.iter().enumerate() {
            if index % 128 == 0 {
                self.control.checkpoint()?;
            }
            vertex_css.insert(subgraph.id.clone(), None);
        }
        let mut seen_edge_indices: HashMap<String, usize> = HashMap::new();
        let mut next_built_edge_index = 0usize;
        let mut next_built_subgraph_index = 0usize;
        for (visited, event) in ast.walk().enumerate() {
            if visited.is_multiple_of(128) {
                self.control.checkpoint()?;
            }
            let StatementEvent::Enter(stmt) = event else {
                let owner = self
                    .subgraph_declaration_owners
                    .get(next_built_subgraph_index)
                    .copied();
                let Some((owner, subgraph)) = owner
                    .and_then(|owner| self.subgraphs.get(owner).map(|subgraph| (owner, subgraph)))
                else {
                    return Ok(Err(Error::diagram_parse_fallback(
                        self.diagram_type.to_string(),
                        "flowchart subgraph replay diverged from the built model",
                    )));
                };
                active_subgraphs.insert(subgraph.id.clone(), owner);
                next_built_subgraph_index = next_built_subgraph_index.saturating_add(1);
                continue;
            };

            match stmt {
                Stmt::Subgraph(_) => {}
                Stmt::Style(s) => {
                    if seen_edge_indices.contains_key(&s.target) {
                        continue;
                    }
                    self.vertex_calls.push(s.target.clone());
                    let is_new_vertex = seen_vertex_ids.insert(s.target.clone());
                    if let Some(style) = vertex_css.get_mut(&s.target) {
                        style
                            .get_or_insert_default()
                            .styles
                            .extend(s.styles.iter().cloned());
                    }
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
                    // Without metadata, addVertex also creates/updates the vertex whose ID
                    // belongs to a completed subgraph. Its separate CSS owner remains intact.
                    let idx = self.ensure_authored_node(&s.target);
                    self.nodes[idx].styles.extend(s.styles.iter().cloned());
                }
                Stmt::ClassDef(c) => {
                    for (index, id) in c.ids.iter().enumerate() {
                        if index % 128 == 0 {
                            self.control.checkpoint()?;
                        }
                        self.class_defs
                            .entry(id.clone())
                            .or_default()
                            .extend(c.styles.iter().cloned());
                    }
                }
                Stmt::ClassAssign(c) => {
                    for (index, target) in c.targets.iter().enumerate() {
                        if index % 128 == 0 {
                            self.control.checkpoint()?;
                        }
                        if let Some(style) = vertex_css.get_mut(target).and_then(Option::as_mut) {
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
                        if let Some(style) = vertex_css.get_mut(id).and_then(Option::as_mut) {
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
                                            .any(|s| s.starts_with("fill"))
                                    {
                                        self.edges[*i].style.push("fill:none".to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                Stmt::ShapeData { target, yaml, .. } => {
                    // The grammar first adds the bare vertex, then applies its metadata.
                    if !seen_edge_indices.contains_key(target) {
                        self.vertex_calls.push(target.clone());
                        seen_vertex_ids.insert(target.clone());
                        if let Some(style) = vertex_css.get_mut(target) {
                            style.get_or_insert_default();
                        }
                    }
                    if let Err(error) =
                        self.apply_shape_data(target, yaml, &active_subgraphs, &seen_edge_indices)?
                    {
                        return Ok(Err(error));
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
            || next_built_subgraph_index != self.subgraph_declaration_owners.len()
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
            if let Some(style) = style
                && let Some(&declaration_ordinal) = active_subgraphs.get(&id)
            {
                // Every completed declaration with this id resolves to the same canonical
                // FlowDB group, including CSS authored between repeated declarations.
                self.subgraph_vertex_styles
                    .insert(id, declaration_ordinal, style);
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
        vertex_css: &mut HashMap<String, Option<FlowSubgraphVertexStyle>>,
        seen_edge_indices: &HashMap<String, usize>,
    ) -> OperationControlResult<Result<()>> {
        let mut pending_shape_data: Option<(&str, &str)> = None;
        for (index, node) in nodes.iter().enumerate() {
            if index % 128 == 0 {
                self.control.checkpoint()?;
            }
            if !seen_edge_indices.contains_key(&node.id) {
                self.vertex_calls.push(node.id.clone());
                seen_vertex_ids.insert(node.id.clone());
                if let Some(style) = vertex_css.get_mut(&node.id) {
                    let style = style.get_or_insert_default();
                    style.styles.extend(node.styles.iter().cloned());
                }

                let target = &mut self.nodes[self.node_index[&node.id]];
                if node.label.is_some() {
                    target.label.clone_from(&node.label);
                    target.label_type = node.label_type.clone();
                    target.label_span = node.label_span;
                    target.label_selection = node.label_selection;
                }
                if node.shape.is_some() {
                    target.shape.clone_from(&node.shape);
                }
                target.styles.extend(node.styles.iter().cloned());
            }

            // A styledVertex calls setClass after addVertex, even when addVertex routed
            // to an edge. Each comma-separated ID can also own a vertex and subgraph.
            for class_name in &node.classes {
                for target in node.id.split(',') {
                    if let Some(style) = vertex_css.get_mut(target).and_then(Option::as_mut) {
                        style.classes.push(class_name.clone());
                    }
                    self.add_class_to_target(
                        target,
                        class_name,
                        active_subgraphs,
                        seen_vertex_ids,
                        seen_edge_indices,
                    )?;
                }
            }

            // Jison reduces `node shapeData & styledVertex` after adding styledVertex.
            // Keep only that pending metadata call, including when both IDs are the same.
            if let Some((target, yaml)) = pending_shape_data
                && let Err(error) =
                    self.apply_shape_data(target, yaml, active_subgraphs, seen_edge_indices)?
            {
                return Ok(Err(error));
            }
            pending_shape_data = node
                .shape_data
                .as_deref()
                .map(|yaml| (node.id.as_str(), yaml));
        }
        if let Some((target, yaml)) = pending_shape_data
            && let Err(error) =
                self.apply_shape_data(target, yaml, active_subgraphs, seen_edge_indices)?
        {
            return Ok(Err(error));
        }
        Ok(Ok(()))
    }

    fn apply_shape_data(
        &mut self,
        target: &str,
        yaml: &str,
        active_subgraphs: &HashMap<String, usize>,
        seen_edge_indices: &HashMap<String, usize>,
    ) -> OperationControlResult<Result<()>> {
        let value = match Self::shape_data_value(self.shape_data_documents, self.diagram_type, yaml)
        {
            Ok(value) => value,
            Err(error) => return Ok(Err(error)),
        };
        // FlowDB routes metadata to an already declared subgraph before checking edge IDs.
        if active_subgraphs.get(target).is_some_and(|&subgraph_index| {
            self.apply_shape_data_to_subgraph(subgraph_index, target, value)
        }) {
            return Ok(Ok(()));
        }
        if let Some(&edge_index) = seen_edge_indices.get(target) {
            // FlowDB's addVertex updates only the first edge with this raw ID.
            self.apply_shape_data_to_edge(edge_index, value)?;
            return Ok(Ok(()));
        }

        self.vertex_calls.push(target.to_string());
        let idx = self.ensure_authored_node(target);
        Ok(apply_shape_data_value_to_node(&mut self.nodes[idx], value)
            .map_err(|error| Error::diagram_parse_fallback(self.diagram_type.to_string(), error)))
    }

    fn add_class_to_target(
        &mut self,
        target: &str,
        class_name: &str,
        active_subgraphs: &HashMap<String, usize>,
        seen_vertex_ids: &HashSet<String>,
        seen_edge_indices: &HashMap<String, usize>,
    ) -> OperationControlResult<()> {
        self.control.checkpoint()?;
        if let Some(&idx) = active_subgraphs.get(target) {
            self.subgraphs[idx].classes.push(class_name.to_string());
        }
        if seen_vertex_ids.contains(target)
            && let Some(&idx) = self.node_index.get(target)
        {
            self.nodes[idx].classes.push(class_name.to_string());
        }
        if let Some(&edge_index) = seen_edge_indices.get(target)
            && let Some(edge) = self.edges.get_mut(edge_index)
        {
            edge.classes.push(class_name.to_string());
        }
        Ok(())
    }

    fn activate_edges(
        &mut self,
        count: usize,
        next_edge_index: &mut usize,
        seen_edge_indices: &mut HashMap<String, usize>,
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
            // FlowDB captures the default at addSingleLink; later explicit overrides still win.
            self.edges[edge_index]
                .interpolate
                .clone_from(&self.edge_defaults.interpolate);
            if let Some(id) = self.edges[edge_index].id.as_deref() {
                // FlowDB's addVertex and setClass both find the first matching raw ID.
                seen_edge_indices
                    .entry(id.to_string())
                    .or_insert(edge_index);
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

    fn apply_shape_data_to_subgraph(
        &mut self,
        subgraph_index: usize,
        target: &str,
        value: &serde_json::Value,
    ) -> bool {
        let Some(map) = value.as_object() else {
            return false;
        };
        let Some(subgraph) = self.subgraphs.get_mut(subgraph_index) else {
            return false;
        };
        if subgraph.id != target {
            return false;
        }

        if map.is_empty() {
            return true;
        }
        let metadata = subgraph
            .metadata
            .get_or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        let Some(metadata_map) = metadata.as_object_mut() else {
            *metadata = serde_json::Value::Object(serde_json::Map::new());
            unreachable!("subgraph metadata was replaced with an object");
        };
        for (key, item) in map {
            metadata_map.insert(key.clone(), item.clone());
        }

        #[cfg(feature = "presentation")]
        if map.contains_key("view") {
            if metadata_map.get("view").and_then(serde_json::Value::as_str) == Some("collapsed") {
                if let Some(collapsed_subgraphs) = self.collapsed_subgraphs.as_deref_mut() {
                    collapsed_subgraphs.insert(target.to_string());
                }
            } else {
                if let Some(collapsed_subgraphs) = self.collapsed_subgraphs.as_deref_mut() {
                    collapsed_subgraphs.remove(target);
                }
            }
        }
        true
    }

    fn apply_shape_data_to_edge(
        &mut self,
        edge_index: usize,
        value: &serde_json::Value,
    ) -> OperationControlResult<()> {
        let Some(map) = value.as_object() else {
            return Ok(());
        };
        self.control.checkpoint()?;
        let Some(edge) = self.edges.get_mut(edge_index) else {
            return Ok(());
        };
        for (key, value) in map {
            match key.as_str() {
                "animate" => {
                    if value.is_null() {
                        edge.animate = None;
                    } else if let Some(value) = value_to_bool(value) {
                        edge.animate = Some(value);
                    }
                }
                "animation" => {
                    if value.is_null() {
                        edge.animation = None;
                    } else if let Some(value) = value_to_string(value) {
                        edge.animation = Some(value);
                    }
                }
                "curve" => {
                    if value.is_null() {
                        edge.interpolate = None;
                    } else if let Some(value) = value_to_string(value) {
                        edge.interpolate = Some(value);
                    }
                }
                "step" => {
                    if let Some(step) = value
                        .as_f64()
                        .filter(|step| super::FlowEdge::is_valid_step(*step))
                    {
                        edge.step = Some(step);
                    }
                }
                _ => {}
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
    use crate::diagrams::flowchart::{FlowchartBuildState, parse_flowchart_ast};

    #[test]
    fn class_and_click_lists_and_inline_classes_cancel_during_target_projection() {
        let ids = (0..256)
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>();
        let declarations = ids.join(" & ");
        let targets = ids.join(",");
        let inline = ids
            .iter()
            .map(|id| format!("{id}:::hot"))
            .collect::<Vec<_>>()
            .join(" & ");
        let meta = crate::SemanticMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        for body in [
            format!("{declarations}\nclass {targets} hot"),
            format!("{declarations}\nclick {targets} call cb()"),
            inline,
        ] {
            let source = format!("flowchart TD\n{body}\n");
            let ast = parse_flowchart_ast(&source, &meta).unwrap();
            let mut build = FlowchartBuildState::new(HashSet::new());
            build
                .add_statements(&ast, &OperationControl::new())
                .unwrap();
            let FlowchartBuildState {
                mut nodes,
                mut node_index,
                mut edges,
                ..
            } = build;
            let mut subgraphs = Vec::new();
            let mut subgraph_vertex_styles = FlowchartSubgraphStyles::default();
            let mut vertex_calls = Vec::new();
            let mut warning_facts = Vec::new();
            let mut class_defs = IndexMap::new();
            let mut tooltips = HashMap::new();
            let mut edge_defaults = EdgeDefaults {
                style: Vec::new(),
                interpolate: None,
            };
            let shape_data_documents = HashMap::new();
            let control = OperationControl::new();
            control.cancel_after_checkpoints(8);
            let mut context = FlowchartSemanticContext {
                nodes: &mut nodes,
                node_index: &mut node_index,
                edges: &mut edges,
                subgraphs: &mut subgraphs,
                subgraph_declaration_owners: &[],
                subgraph_vertex_styles: &mut subgraph_vertex_styles,
                #[cfg(feature = "presentation")]
                collapsed_subgraphs: None,
                vertex_calls: &mut vertex_calls,
                warning_facts: &mut warning_facts,
                class_defs: &mut class_defs,
                tooltips: &mut tooltips,
                edge_defaults: &mut edge_defaults,
                security_level_loose: true,
                diagram_type: "flowchart-v2",
                config: &meta.semantic_config,
                shape_data_documents: &shape_data_documents,
                control: &control,
            };
            assert!(matches!(
                apply_semantic_statements(&ast, &mut context),
                Err(crate::OperationCancelled { .. })
            ));
            let styled = nodes.iter().filter(|node| !node.classes.is_empty()).count();
            assert!(styled > 0 && styled < ids.len(), "{styled}");
        }
    }
}
