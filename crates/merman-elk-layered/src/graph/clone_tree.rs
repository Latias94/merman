use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use super::{GraphReferenceDomain, GraphSeedScope, LGraph, LNode};

impl Clone for LGraph {
    fn clone(&self) -> Self {
        self.copy_with_rebound_domains().0
    }
}

impl Clone for LNode {
    fn clone(&self) -> Self {
        let mut copied = self.copy_without_nested_graph();
        if let Some(nested) = self.nested_graph.as_deref() {
            let (nested, domains) = nested.copy_with_rebound_domains();
            domains.rebind_node(&mut copied);
            copied.nested_graph = Some(Box::new(nested));
        }
        copied
    }
}

/// An internal validation snapshot may own a deep hierarchy even on the flat pipeline entry.
pub(crate) struct ValidationGraphCopy(LGraph);

impl Deref for ValidationGraphCopy {
    type Target = LGraph;

    fn deref(&self) -> &LGraph {
        &self.0
    }
}

impl DerefMut for ValidationGraphCopy {
    fn deref_mut(&mut self) -> &mut LGraph {
        &mut self.0
    }
}

impl Drop for ValidationGraphCopy {
    fn drop(&mut self) {
        LNode::drop_nested_graphs(&mut self.0.layerless_nodes);
    }
}

#[derive(Default)]
struct CopiedDomains(HashMap<*const GraphSeedScope, GraphReferenceDomain>);

impl CopiedDomains {
    fn copy_domain(&mut self, domain: &GraphReferenceDomain) -> GraphReferenceDomain {
        let copied = GraphReferenceDomain(Arc::new(domain.0.as_ref().clone()));
        let previous = self.0.insert(Arc::as_ptr(&domain.0), copied.clone());
        debug_assert!(previous.is_none());
        copied
    }

    fn rebind(&self, domain: &mut GraphReferenceDomain) {
        if let Some(copied) = self.0.get(&Arc::as_ptr(&domain.0)) {
            *domain = copied.clone();
        }
    }

    fn rebind_node(&self, node: &mut LNode) {
        if let Some(origin) = &mut node.origin_port {
            self.rebind(&mut origin.domain);
        }
        for port in &mut node.ports {
            if let Some(dummy) = &mut port.port_dummy {
                self.rebind(&mut dummy.domain);
            }
        }
    }

    fn rebind_graph<'a>(&self, graph: &'a mut LGraph, pending: &mut Vec<&'a mut LGraph>) {
        for node in &mut graph.layerless_nodes {
            self.rebind_node(node);
            if let Some(nested) = node.nested_graph.as_deref_mut() {
                pending.push(nested);
            }
        }
    }
}

impl LGraph {
    fn copy_with_rebound_domains(&self) -> (Self, CopiedDomains) {
        let mut domains = CopiedDomains::default();
        let mut copied = self.copy_tree(|domain| domains.copy_domain(domain));

        let mut pending = Vec::new();
        domains.rebind_graph(&mut copied, &mut pending);
        while let Some(tree_copy) = pending.pop() {
            domains.rebind_graph(tree_copy, &mut pending);
        }
        drop(pending);
        (copied, domains)
    }

    fn copy_tree(
        &self,
        mut copy_domain: impl FnMut(&GraphReferenceDomain) -> GraphReferenceDomain,
    ) -> Self {
        let mut copied = self.copy_without_nodes(copy_domain(self.reference_domain()));
        let mut pending = Vec::new();
        self.copy_nodes(&mut copied, &mut pending, &mut copy_domain);
        while let Some((source, tree_copy)) = pending.pop() {
            source.copy_nodes(tree_copy, &mut pending, &mut copy_domain);
        }
        drop(pending);
        copied
    }

    fn copy_nodes<'source, 'target>(
        &'source self,
        copied: &'target mut Self,
        pending: &mut Vec<(&'source Self, &'target mut Self)>,
        copy_domain: &mut impl FnMut(&GraphReferenceDomain) -> GraphReferenceDomain,
    ) {
        copied.layerless_nodes = self
            .layerless_nodes
            .iter()
            .map(LNode::copy_without_nested_graph)
            .collect();
        for (source, node_copy) in self.layerless_nodes.iter().zip(&mut copied.layerless_nodes) {
            if let Some(nested) = source.nested_graph.as_deref() {
                let domain = copy_domain(nested.reference_domain());
                let nested_copy = node_copy
                    .nested_graph
                    .insert(Box::new(nested.copy_without_nodes(domain)));
                pending.push((nested, nested_copy.as_mut()));
            }
        }
    }

    /// Copies a detached validation snapshot whose scoped references still target the source domains.
    pub(crate) fn copy_preserving_reference_domains(&self) -> ValidationGraphCopy {
        ValidationGraphCopy(self.copy_tree(GraphReferenceDomain::clone))
    }

    fn copy_without_nodes(&self, domain: GraphReferenceDomain) -> Self {
        let random_seed_context = super::GraphRandomSeedContext {
            authority: self.random_seed_context.authority,
            domain,
            configuration_invocations: self.random_seed_context.configuration_invocations,
        };
        Self {
            id: self.id.clone(),
            options: self.options.clone(),
            size: self.size,
            padding: self.padding,
            offset: self.offset,
            layerless_nodes: Vec::new(),
            layers: self.layers.clone(),
            edges: self.edges.clone(),
            graph_properties: self.graph_properties.clone(),
            cyclic: self.cyclic,
            max_model_order_nodes: self.max_model_order_nodes,
            random: self.random.clone(),
            parent_node_id: self.parent_node_id.clone(),
            hidden_nodes: self.hidden_nodes.clone(),
            replaced_external_port_dummies: self.replaced_external_port_dummies.clone(),
            hierarchy_edges: self.hierarchy_edges.clone(),
            cross_hierarchy_edges: self.cross_hierarchy_edges.clone(),
            self_loop_holders: self.self_loop_holders.clone(),
            in_layer_successor_constraints_between_non_dummies: self
                .in_layer_successor_constraints_between_non_dummies,
            random_seed_context,
        }
    }
}

impl LNode {
    /// Release owned subgraphs without recursing through their nodes on the native stack.
    pub(crate) fn drop_nested_graphs(nodes: &mut [Self]) {
        fn take_children(nodes: &mut [LNode]) -> impl Iterator<Item = Box<LGraph>> {
            nodes
                .iter_mut()
                .rev()
                .filter_map(|node| node.nested_graph.take())
        }

        let mut pending = take_children(nodes).collect::<Vec<_>>();
        while let Some(mut child) = pending.pop() {
            pending.extend(take_children(&mut child.layerless_nodes));
        }
    }

    pub(crate) fn copy_preserving_reference_domains(&self) -> Self {
        let mut copied = self.copy_without_nested_graph();
        copied.nested_graph = self
            .nested_graph
            .as_deref()
            .map(|graph| Box::new(graph.copy_tree(GraphReferenceDomain::clone)));
        copied
    }

    fn copy_without_nested_graph(&self) -> Self {
        Self {
            id: self.id.clone(),
            kind: self.kind,
            size: self.size,
            position: self.position,
            margin: self.margin,
            padding: self.padding,
            labels: self.labels.clone(),
            represented_labels: self.represented_labels.clone(),
            node_label_placement: self.node_label_placement,
            ports: self.ports.clone(),
            nested_graph: None,
            model_order: self.model_order,
            component_priority: self.component_priority,
            layer_index: self.layer_index,
            layer_constraint: self.layer_constraint,
            node_alignment: self.node_alignment,
            port_constraints: self.port_constraints,
            port_alignment: self.port_alignment,
            node_flexibility: self.node_flexibility,
            ports_surrounding: self.ports_surrounding,
            external_port_side: self.external_port_side,
            external_port_size: self.external_port_size,
            port_ratio_or_position: self.port_ratio_or_position,
            replaced_external_port_dummy: self.replaced_external_port_dummy,
            in_layer_successor_constraints: self.in_layer_successor_constraints.clone(),
            in_layer_layout_unit: self.in_layer_layout_unit,
            barycenter_associates: self.barycenter_associates.clone(),
            origin_port: self.origin_port.clone(),
            origin_edge: self.origin_edge,
            parent_port_key: self.parent_port_key.clone(),
            parent_port_type: self.parent_port_type,
            long_edge_source: self.long_edge_source,
            long_edge_target: self.long_edge_target,
            long_edge_has_label_dummies: self.long_edge_has_label_dummies,
            label_side: self.label_side,
            in_layer_constraint: self.in_layer_constraint,
            layer_constraint_explicit: self.layer_constraint_explicit,
            hidden: self.hidden,
            compound: self.compound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LNodeKind, LPort, PortRef, PortType};
    use crate::{LayeredOptions, PipelineError, RandomSeedError, execute_ported_processors};

    #[test]
    fn deep_internal_copies_preserve_domains_on_a_small_stack() {
        const DEPTH: usize = 1_024;

        fn assert_chain(mut source: &LGraph, mut copy: &LGraph, root: &LGraph, first: usize) {
            for level in first..DEPTH {
                assert!(
                    source
                        .reference_domain()
                        .same_arena(copy.reference_domain())
                );
                let owner = &copy.layerless_nodes[0];
                assert_eq!(owner.model_order, Some(level));
                copy = owner.nested_graph.as_deref().unwrap();
                let backlink = owner.ports[0].port_dummy.as_ref().unwrap();
                assert!(backlink.belongs_to(copy));
                assert_eq!(backlink.node, 0);
                source = source.layerless_nodes[0].nested_graph.as_deref().unwrap();
            }
            assert!(
                source
                    .reference_domain()
                    .same_arena(copy.reference_domain())
            );
            let origin = copy.layerless_nodes[0].origin_port.as_ref().unwrap();
            assert!(origin.belongs_to(root));
            assert_eq!(origin.port, PortRef { node: 0, port: 0 });
        }

        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut root = ValidationGraphCopy(LGraph::new("same", LayeredOptions::default()));
                let origin = root.scoped_port(PortRef { node: 0, port: 0 });
                let mut graph: &mut LGraph = &mut root;
                for level in 0..DEPTH {
                    let child = LGraph::new("same", LayeredOptions::default());
                    let mut owner = LNode::new("owner", 1.0, 1.0, Some(level));
                    let mut port = LPort::new("out", 0, PortType::Output);
                    port.port_dummy = Some(child.scoped_node(0));
                    owner.ports.push(port);
                    owner.nested_graph = Some(Box::new(child));
                    graph.layerless_nodes.push(owner);
                    graph = graph.layerless_nodes[0]
                        .nested_graph
                        .as_deref_mut()
                        .unwrap();
                }
                graph.options.random_seed = 0;
                let mut dummy = LNode::new("deep-dummy", 1.0, 1.0, None);
                dummy.kind = LNodeKind::ExternalPort;
                dummy.origin_port = Some(origin);
                graph.layerless_nodes.push(dummy);

                let source_refs = Arc::strong_count(&root.reference_domain().0);
                let copy = root.copy_preserving_reference_domains();
                assert_chain(&root, &copy, &root, 0);
                drop(copy);

                let owner = &root.layerless_nodes[0];
                let mut node_copy = owner.copy_preserving_reference_domains();
                let nested_copy = ValidationGraphCopy(*node_copy.nested_graph.take().unwrap());
                assert!(
                    node_copy.ports[0]
                        .port_dummy
                        .as_ref()
                        .unwrap()
                        .belongs_to(&nested_copy)
                );
                assert_chain(
                    owner.nested_graph.as_deref().unwrap(),
                    &nested_copy,
                    &root,
                    1,
                );
                drop(nested_copy);

                // The full runner drops its validation snapshot on this early return.
                assert!(matches!(
                    execute_ported_processors(&mut root),
                    Err(PipelineError::RandomSeed(RandomSeedError::Unresolved { graph_path }))
                        if graph_path == "same"
                ));
                assert_eq!(Arc::strong_count(&root.reference_domain().0), source_refs);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
