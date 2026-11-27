use std::{collections::HashMap, sync::Arc};

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
        let mut copied = self.copy_without_nodes(domains.copy_domain(self.reference_domain()));
        let mut pending = Vec::new();
        self.copy_nodes(&mut copied, &mut pending, &mut domains);
        while let Some((source, tree_copy)) = pending.pop() {
            source.copy_nodes(tree_copy, &mut pending, &mut domains);
        }
        drop(pending);

        let mut pending = Vec::new();
        domains.rebind_graph(&mut copied, &mut pending);
        while let Some(tree_copy) = pending.pop() {
            domains.rebind_graph(tree_copy, &mut pending);
        }
        drop(pending);
        (copied, domains)
    }

    fn copy_nodes<'source, 'target>(
        &'source self,
        copied: &'target mut Self,
        pending: &mut Vec<(&'source Self, &'target mut Self)>,
        domains: &mut CopiedDomains,
    ) {
        copied.layerless_nodes = self
            .layerless_nodes
            .iter()
            .map(LNode::copy_without_nested_graph)
            .collect();
        for (source, node_copy) in self.layerless_nodes.iter().zip(&mut copied.layerless_nodes) {
            if let Some(nested) = source.nested_graph.as_deref() {
                let domain = domains.copy_domain(nested.reference_domain());
                let nested_copy = node_copy
                    .nested_graph
                    .insert(Box::new(nested.copy_without_nodes(domain)));
                pending.push((nested, nested_copy.as_mut()));
            }
        }
    }

    /// Copies a temporary view of this graph without allocating replacement domains.
    /// The copy must not be attached alongside its source as a separate graph.
    pub(crate) fn copy_preserving_reference_domains(&self) -> Self {
        let mut copied = self.copy_without_nodes(self.reference_domain().clone());
        copied.layerless_nodes = self
            .layerless_nodes
            .iter()
            .map(LNode::copy_preserving_reference_domains)
            .collect();
        copied
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
    pub(crate) fn copy_preserving_reference_domains(&self) -> Self {
        let mut copied = self.copy_without_nested_graph();
        copied.nested_graph = self
            .nested_graph
            .as_deref()
            .map(|graph| Box::new(graph.copy_preserving_reference_domains()));
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
