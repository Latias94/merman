#[cfg(test)]
mod tests {
    use merman_model::families::state::StateTerminalView;
    use merman_model::resources::ModelComplexity;

    #[test]
    fn canonical_terminal_view_borrows_nodes_and_edges() {
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync("stateDiagram-v2\nA --> B: ready\n")
            .expect("canonical state source should parse");
        let crate::SemanticDiagramModel::State(model) = &parsed.model else {
            panic!("expected State model");
        };
        let view = StateTerminalView::from_semantic(model);
        assert_eq!(view.nodes().len(), model.nodes.len());
        assert_eq!(view.edges().len(), model.edges.len());
        assert!(!view.nodes().is_empty());
        assert!(!view.edges().is_empty());
        for (view, node) in view.nodes().iter().zip(&model.nodes) {
            assert!(std::ptr::eq(view.id.as_ptr(), node.id.as_ptr()));
            assert!(std::ptr::eq(view.shape.as_ptr(), node.shape.as_ptr()));
            assert!(view.label_style.is_empty());
        }
        for (view, edge) in view.edges().iter().zip(&model.edges) {
            assert!(std::ptr::eq(view.start.as_ptr(), edge.start.as_ptr()));
            assert!(std::ptr::eq(view.label.as_ptr(), edge.label.as_ptr()));
            assert!(view.arrow_type_end.is_empty());
        }
        assert!(view.nodes().get(model.nodes.len()).is_none());
    }

    fn fallback_source() -> &'static str {
        concat!(
            "stateDiagram-v2\ndirection LR\naccTitle: State title\naccDescr: State description\n",
            "state Outer {\nstate \"Human label\" as A\nA --> B: transition\n",
            "note right of A : remember\n}\n",
            "classDef active fill:red,color:blue\nclass A active\nstyle A color:green\n",
            "click A href \"https://example.com\"\n",
        )
    }

    #[test]
    fn canonical_terminal_serialization_preserves_complete_authored_records() {
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(fallback_source())
            .unwrap();
        let crate::SemanticDiagramModel::State(model) = &parsed.model else {
            panic!("expected State model");
        };
        let view = StateTerminalView::from_semantic(model);
        let value = serde_json::to_value(view).unwrap();
        assert_eq!(value["direction"], model.direction);
        assert_eq!(
            value["accTitle"],
            serde_json::to_value(&model.acc_title).unwrap()
        );
        assert_eq!(
            value["accDescr"],
            serde_json::to_value(&model.acc_descr).unwrap()
        );
        assert_eq!(value["nodes"], serde_json::to_value(&model.nodes).unwrap());
        assert_eq!(value["edges"], serde_json::to_value(&model.edges).unwrap());
        assert_eq!(
            value["relations"],
            serde_json::to_value(&model.relations).unwrap()
        );
        assert_eq!(value["links"], serde_json::to_value(&model.links).unwrap());
        assert_eq!(
            value["styleClasses"],
            serde_json::to_value(&model.style_classes).unwrap()
        );
        assert!(!model.links.is_empty());
        assert!(!model.style_classes.is_empty());
        assert_eq!(
            value["document"]["root"],
            model.document.root().unwrap().index()
        );
        assert_eq!(
            value["document"]["documents"].as_array().unwrap().len(),
            model.document.len()
        );
        assert!(model.document.len() > 1);
        for (id, record) in &model.states {
            assert_eq!(value["states"][id]["id"], record.id);
            assert_eq!(value["states"][id]["type"], record.state_type);
            assert_eq!(
                value["states"][id]["descriptions"],
                serde_json::to_value(&record.descriptions).unwrap()
            );
            assert_eq!(
                value["states"][id]["classes"],
                serde_json::to_value(&record.classes).unwrap()
            );
            assert_eq!(
                value["states"][id]["styles"],
                serde_json::to_value(&record.styles).unwrap()
            );
            assert_eq!(
                value["states"][id]["textStyles"],
                serde_json::to_value(&record.text_styles).unwrap()
            );
            assert_eq!(
                value["states"][id]["note"],
                serde_json::to_value(&record.note).unwrap()
            );
            assert_eq!(
                value["states"][id]["doc"],
                serde_json::to_value(record.doc.map(|id| id.index())).unwrap()
            );
            assert_eq!(
                value["states"][id]["start"],
                serde_json::to_value(record.start).unwrap()
            );
        }
        let statements = serde_json::to_string(&value["document"]).unwrap();
        for authored in [
            "Human label",
            "transition",
            "remember",
            "classDef",
            "color:green",
            "https://example.com",
        ] {
            assert!(
                statements.contains(authored),
                "missing {authored}: {statements}"
            );
        }
        assert!(value.get("config").is_none());
        assert!(value.get("look").is_none());
        assert_eq!(view.complexity(), model.model_complexity());
        let complexity = view.complexity();
        let json_complexity = ModelComplexity::from_json(&value);
        // Typed complexity excludes fixed schema keys that arbitrary JSON counts as data.
        assert!(complexity.text_bytes <= json_complexity.text_bytes);
        assert_eq!(complexity.nesting_depth, json_complexity.nesting_depth);
        assert!(complexity.items <= json_complexity.items);
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn full_terminal_serialization_keeps_typed_fields_and_flat_documents() {
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(fallback_source(), crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::State(model) = parsed.model() else {
            panic!("expected full State model");
        };
        let view = StateTerminalView::from_render(model);
        let value = serde_json::to_value(view).unwrap();
        for (actual, node) in value["nodes"].as_array().unwrap().iter().zip(&model.nodes) {
            let typed = serde_json::to_value(node).unwrap();
            for (field, expected) in typed.as_object().unwrap() {
                assert_eq!(&actual[field], expected, "field: {field}");
            }
            assert_eq!(
                actual.get("explicitDir"),
                node.explicit_dir.map(serde_json::Value::Bool).as_ref()
            );
        }
        assert_eq!(value["edges"], serde_json::to_value(&model.edges).unwrap());
        assert_eq!(
            value["relations"],
            serde_json::to_value(&model.relations).unwrap()
        );
        assert_eq!(value["links"], serde_json::to_value(&model.links).unwrap());
        assert_eq!(
            value["styleClasses"],
            serde_json::to_value(&model.style_classes).unwrap()
        );
        assert_eq!(
            value["document"]["documents"].as_array().unwrap().len(),
            model.document.len()
        );
        assert!(value["states"]["Outer"]["doc"].is_number());
        assert!(
            value["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node.get("domId").is_some())
        );
        assert!(
            value["edges"]
                .as_array()
                .unwrap()
                .iter()
                .any(|edge| edge.get("arrowTypeEnd").is_some())
        );
        let complexity = view.complexity();
        let json_complexity = ModelComplexity::from_json(&value);
        // Typed complexity excludes fixed schema keys that arbitrary JSON counts as data.
        assert!(complexity.text_bytes <= json_complexity.text_bytes);
        assert_eq!(complexity.nesting_depth, json_complexity.nesting_depth);
        assert!(complexity.items <= json_complexity.items);
    }

    #[test]
    fn terminal_complexity_keeps_the_authored_document_nesting_bound() {
        const DEPTH: usize = 40;
        let mut source = String::from("stateDiagram-v2\n");
        for level in 0..DEPTH {
            source.push_str(&format!("state S{level} {{\n"));
        }
        source.push_str("Leaf\n");
        source.push_str(&"}\n".repeat(DEPTH));
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(&source)
            .unwrap();
        let crate::SemanticDiagramModel::State(model) = &parsed.model else {
            panic!("expected State model");
        };
        let view = StateTerminalView::from_semantic(model);
        assert!(view.complexity().nesting_depth >= DEPTH);
        assert_eq!(view.complexity(), model.model_complexity());
    }
}
