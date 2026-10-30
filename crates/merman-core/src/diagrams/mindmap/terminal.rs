//! Source-backed Mindmap terminal integration tests.

use super::*;
use crate::MermaidConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_fallback_retains_all_authored_fields_and_explicit_display_policy() {
        let mut model = semantic("mindmap\n root\n  left\n   leaf\n  right\n");
        model.nodes[1].class = Some("highlight".to_string());
        model.nodes[1].icon = Some("star".to_string());
        let config = MermaidConfig::from_value(
            serde_json::json!({"theme": "redux", "mindmap": {"colorBy": "depth"}}),
        );
        let view = MindmapTerminalView::from_semantic(&model, &config);
        let wire = serde_json::to_value(view).unwrap();
        assert_eq!(
            wire["display"],
            serde_json::json!({"defaultShape": "rounded", "colorByDepth": true})
        );
        assert_eq!(wire["nodes"].as_array().unwrap().len(), model.nodes.len());
        for (index, authored) in model.nodes.iter().enumerate() {
            let mut expected = serde_json::to_value(authored).unwrap();
            expected["shape"] = "rounded".into();
            assert_eq!(wire["nodes"][index], expected);
        }
        let base =
            crate::resources::MindmapComplexity::from_semantic_model(&model).as_model_complexity();
        assert_eq!(
            view.complexity(),
            crate::resources::ModelComplexity::new(
                base.items,
                base.text_bytes + "rounded".len() * (model.nodes.len() + 1),
                base.nesting_depth
            )
        );
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn detached_fallback_keeps_full_custom_shapes_sections_and_dimensions() {
        let mut model = MindmapDiagramRenderModel::default();
        model.nodes.push(MindmapDiagramRenderNode {
            id: "host-node".to_string(),
            dom_id: "host-dom".to_string(),
            label: "Host".to_string(),
            label_type: "text".to_string(),
            is_group: true,
            shape: "custom-host-shape".to_string(),
            width: 73.0,
            height: 29.0,
            padding: 3.0,
            css_classes: "host-class".to_string(),
            css_styles: vec!["fill:red".to_string()],
            look: "host-look".to_string(),
            icon: Some("star".to_string()),
            x: Some(17.0),
            y: Some(19.0),
            level: 4,
            node_id: "authored-host".to_string(),
            node_type: 0,
            section: Some(9),
        });
        let view = MindmapTerminalView::from_render(&model);
        assert_eq!(
            serde_json::to_value(view).unwrap(),
            serde_json::to_value(&model).unwrap()
        );
        assert_eq!(
            view.complexity(),
            crate::resources::MindmapComplexity::from_model(&model).as_model_complexity()
        );
    }

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
        let parsed = crate::MermaidEngine::new()
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
