use super::{MindmapDiagramModel, MindmapDisplayPolicy, MindmapNodeModel};
#[cfg(feature = "presentation")]
use super::{MindmapDiagramRenderModel, MindmapDiagramRenderNode};
use crate::MermaidConfig;

/// Terminal graph identity, preserving numeric canonical IDs and detached string IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MindmapTerminalNodeId<'a> {
    Canonical(i32),
    Detached(&'a str),
}

/// Borrowed authored fields used by terminal Mindmap consumers.
#[derive(Debug, Clone, Copy)]
pub struct MindmapTerminalNode<'a> {
    pub id: MindmapTerminalNodeId<'a>,
    pub node_id: &'a str,
    pub label: &'a str,
    pub shape: &'a str,
    pub icon: Option<&'a str>,
    pub section: Option<i32>,
    authored_level: Option<i32>,
}

impl<'a> MindmapTerminalNode<'a> {
    fn from_semantic(node: &'a MindmapNodeModel, display: MindmapDisplayPolicy) -> Self {
        Self {
            id: MindmapTerminalNodeId::Canonical(node.id),
            node_id: &node.node_id,
            label: &node.descr,
            shape: display.shape(node.node_type),
            icon: node.icon.as_deref(),
            section: None,
            authored_level: Some(node.level),
        }
    }

    #[cfg(feature = "presentation")]
    fn from_render(node: &'a MindmapDiagramRenderNode) -> Self {
        Self {
            id: MindmapTerminalNodeId::Detached(&node.id),
            node_id: &node.node_id,
            label: &node.label,
            shape: &node.shape,
            icon: node.icon.as_deref(),
            section: node.section,
            authored_level: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MindmapTerminalEdge<'a> {
    pub id: &'a str,
    pub start: MindmapTerminalNodeId<'a>,
    pub end: MindmapTerminalNodeId<'a>,
}

#[derive(Debug, Clone, Copy)]
enum MindmapTerminalBacking<'a> {
    Semantic(&'a MindmapDiagramModel, MindmapDisplayPolicy),
    #[cfg(feature = "presentation")]
    Render(&'a MindmapDiagramRenderModel),
}

/// Borrows terminal facts from one owning Mindmap model.
#[derive(Debug, Clone, Copy)]
pub struct MindmapTerminalView<'a> {
    backing: MindmapTerminalBacking<'a>,
}

impl<'a> MindmapTerminalView<'a> {
    pub fn from_semantic(model: &'a MindmapDiagramModel, config: &MermaidConfig) -> Self {
        Self {
            backing: MindmapTerminalBacking::Semantic(
                model,
                MindmapDisplayPolicy::from_config(config),
            ),
        }
    }

    #[cfg(feature = "presentation")]
    pub fn from_render(model: &'a MindmapDiagramRenderModel) -> Self {
        Self {
            backing: MindmapTerminalBacking::Render(model),
        }
    }

    pub fn node_count(self) -> usize {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, _) => model.nodes.len(),
            #[cfg(feature = "presentation")]
            MindmapTerminalBacking::Render(model) => model.nodes.len(),
        }
    }

    pub fn node(self, index: usize) -> Option<MindmapTerminalNode<'a>> {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, display) => model
                .nodes
                .get(index)
                .map(|node| MindmapTerminalNode::from_semantic(node, display)),
            #[cfg(feature = "presentation")]
            MindmapTerminalBacking::Render(model) => {
                model.nodes.get(index).map(MindmapTerminalNode::from_render)
            }
        }
    }

    pub fn nodes(self) -> impl Iterator<Item = MindmapTerminalNode<'a>> + Clone {
        let (semantic, display) = match self.backing {
            MindmapTerminalBacking::Semantic(model, display) => {
                (model.nodes.as_slice(), Some(display))
            }
            #[cfg(feature = "presentation")]
            MindmapTerminalBacking::Render(_) => (&[][..], None),
        };
        let semantic_nodes = semantic.iter().filter_map(move |node| {
            display.map(|display| MindmapTerminalNode::from_semantic(node, display))
        });
        #[cfg(feature = "presentation")]
        {
            let render = match self.backing {
                MindmapTerminalBacking::Render(model) => model.nodes.as_slice(),
                MindmapTerminalBacking::Semantic(_, _) => &[],
            };
            semantic_nodes.chain(render.iter().map(MindmapTerminalNode::from_render))
        }
        #[cfg(not(feature = "presentation"))]
        {
            semantic_nodes
        }
    }

    /// Returns a ready edge count only when the backing owner already stores flat edges.
    pub fn edge_count_hint(self) -> Option<usize> {
        match self.backing {
            MindmapTerminalBacking::Semantic(_, _) => None,
            #[cfg(feature = "presentation")]
            MindmapTerminalBacking::Render(model) => Some(model.edges.len()),
        }
    }

    /// Calls the visitor once per canonical node, including empty child lists.
    ///
    /// The visitor can checkpoint, charge the scan, check the sum, or stop before the next node.
    /// Flat render edges require no node scan; their count is available from `edge_count_hint`.
    pub fn try_visit_child_counts<E>(
        self,
        mut visit: impl FnMut(usize) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, _) => {
                for node in &model.nodes {
                    visit(node.children.len())?;
                }
            }
            #[cfg(feature = "presentation")]
            MindmapTerminalBacking::Render(_) => {}
        }
        Ok(())
    }

    /// Visits edges in owner order without materializing a flat edge vector.
    ///
    /// `scan_node` runs before each canonical node's child scan, including empty lists.
    /// `visit_edge` runs for every edge. Either callback can stop traversal immediately;
    /// consumers own cancellation and resource admission in these callbacks.
    /// Flat render owners invoke only `visit_edge`.
    pub fn try_visit_edges<E>(
        self,
        mut scan_node: impl FnMut() -> std::result::Result<(), E>,
        mut visit_edge: impl FnMut(MindmapTerminalEdge<'a>) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, _) => {
                for node in &model.nodes {
                    scan_node()?;
                    for child in &node.children {
                        visit_edge(MindmapTerminalEdge {
                            id: "",
                            start: MindmapTerminalNodeId::Canonical(node.id),
                            end: MindmapTerminalNodeId::Canonical(*child),
                        })?;
                    }
                }
            }
            #[cfg(feature = "presentation")]
            MindmapTerminalBacking::Render(model) => {
                for edge in &model.edges {
                    visit_edge(MindmapTerminalEdge {
                        id: &edge.id,
                        start: MindmapTerminalNodeId::Detached(&edge.start),
                        end: MindmapTerminalNodeId::Detached(&edge.end),
                    })?;
                }
            }
        }
        Ok(())
    }

    pub fn section(
        self,
        node: MindmapTerminalNode<'_>,
        inherited: Option<i32>,
        depth: usize,
    ) -> Option<i32> {
        match (self.backing, node.authored_level) {
            (MindmapTerminalBacking::Semantic(_, display), Some(level)) => {
                display.section(level, inherited, depth)
            }
            _ => node.section,
        }
    }

    pub fn child_section(
        self,
        node: MindmapTerminalNode<'_>,
        child_index: usize,
        inherited: Option<i32>,
    ) -> Option<i32> {
        match (self.backing, node.authored_level) {
            (MindmapTerminalBacking::Semantic(_, display), Some(level)) => {
                display.child_section(level, child_index, inherited)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn semantic(source: &str) -> MindmapDiagramModel {
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let crate::SemanticDiagramModel::Mindmap(model) = parsed.model else {
            panic!("expected canonical Mindmap");
        };
        model
    }

    #[test]
    fn terminal_view_borrows_canonical_fields_and_keeps_edge_order() {
        let mut model = semantic("mindmap\n root\n  left\n   leaf\n  right\n");
        model.nodes[0].icon = Some("root-icon".to_string());
        let view = MindmapTerminalView::from_semantic(&model, &MermaidConfig::empty_object());
        assert_eq!(view.node_count(), model.nodes.len());
        assert!(view.node(model.nodes.len()).is_none());
        assert_eq!(view.nodes().count(), model.nodes.len());
        for (view, node) in view.nodes().zip(&model.nodes) {
            assert_eq!(view.id, MindmapTerminalNodeId::Canonical(node.id));
            assert!(std::ptr::eq(view.node_id.as_ptr(), node.node_id.as_ptr()));
            assert!(std::ptr::eq(view.label.as_ptr(), node.descr.as_ptr()));
            assert_eq!(
                view.icon.map(str::as_ptr),
                node.icon.as_deref().map(str::as_ptr)
            );
        }
        assert_eq!(view.edge_count_hint(), None);
        let mut counts = Vec::new();
        view.try_visit_child_counts(|count| {
            counts.push(count);
            Ok::<(), ()>(())
        })
        .unwrap();
        assert_eq!(counts, [2, 1, 0, 0]);
        let mut scanned = 0;
        let mut edges = Vec::new();
        view.try_visit_edges(
            || {
                scanned += 1;
                Ok::<(), ()>(())
            },
            |edge| {
                assert!(edge.id.is_empty());
                edges.push((edge.start, edge.end));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(scanned, model.nodes.len());
        assert_eq!(
            edges,
            [
                (
                    MindmapTerminalNodeId::Canonical(0),
                    MindmapTerminalNodeId::Canonical(1)
                ),
                (
                    MindmapTerminalNodeId::Canonical(0),
                    MindmapTerminalNodeId::Canonical(3)
                ),
                (
                    MindmapTerminalNodeId::Canonical(1),
                    MindmapTerminalNodeId::Canonical(2)
                ),
            ]
        );
    }

    #[test]
    fn terminal_visitors_stop_before_unadmitted_empty_nodes_or_edges() {
        let mut model = semantic("mindmap\n root\n  left\n  right\n");
        for node in &mut model.nodes {
            node.children.clear();
        }
        let view = MindmapTerminalView::from_semantic(&model, &MermaidConfig::empty_object());
        let mut scanned = 0;
        let result = view.try_visit_child_counts(|count| {
            assert_eq!(count, 0);
            scanned += 1;
            if scanned == 2 { Err("stop") } else { Ok(()) }
        });
        assert_eq!(result, Err("stop"));
        assert_eq!(scanned, 2);

        let mut scanned = 0;
        let mut edges = 0;
        let result = view.try_visit_edges(
            || {
                scanned += 1;
                if scanned == 2 { Err("stop") } else { Ok(()) }
            },
            |_| {
                edges += 1;
                Ok(())
            },
        );
        assert_eq!(result, Err("stop"));
        assert_eq!(scanned, 2);
        assert_eq!(edges, 0);

        model.nodes[0].children.extend([1, 2]);
        let view = MindmapTerminalView::from_semantic(&model, &MermaidConfig::empty_object());
        let mut scanned = 0;
        let mut edges = 0;
        let result = view.try_visit_edges(
            || {
                scanned += 1;
                Ok(())
            },
            |_| {
                edges += 1;
                Err("stop edge")
            },
        );
        assert_eq!(result, Err("stop edge"));
        assert_eq!(scanned, 1);
        assert_eq!(edges, 1);
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn terminal_render_view_borrows_detached_fields_without_node_scan_callbacks() {
        let parsed = crate::Engine::new()
            .parse_diagram_for_render_model_sync(
                "mindmap\n root((Root))\n  left\n  right\n",
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::Mindmap(model) = parsed.model() else {
            panic!("expected full Mindmap");
        };
        let view = MindmapTerminalView::from_render(model);
        assert_eq!(view.edge_count_hint(), Some(model.edges.len()));
        for (view, node) in view.nodes().zip(&model.nodes) {
            assert_eq!(view.id, MindmapTerminalNodeId::Detached(&node.id));
            assert!(std::ptr::eq(view.shape.as_ptr(), node.shape.as_ptr()));
            assert!(std::ptr::eq(view.label.as_ptr(), node.label.as_ptr()));
            assert_eq!(view.section, node.section);
        }
        let mut scanned = 0;
        view.try_visit_child_counts(|_| {
            scanned += 1;
            Ok::<(), ()>(())
        })
        .unwrap();
        let mut edges = 0;
        view.try_visit_edges(
            || {
                scanned += 1;
                Ok::<(), ()>(())
            },
            |edge| {
                assert!(std::ptr::eq(
                    edge.id.as_ptr(),
                    model.edges[edges].id.as_ptr()
                ));
                edges += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(scanned, 0);
        assert_eq!(edges, model.edges.len());
    }
}
