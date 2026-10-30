//! Temporary Class data forwarding and core-owned source/resource operations.

pub use merman_model::families::class::*;

pub(crate) fn sanitize_common_db_fields(
    model: &mut ClassDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[cfg(feature = "presentation")]
pub(crate) fn sanitize_full_common_db_fields(
    model: &mut ClassDiagram,
    config: &crate::MermaidConfig,
) {
    let (acc_title, acc_descr) = model.accessibility_mut();
    crate::common_db::sanitize_optional_acc_title(acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(acc_descr, config);
}

/// Counts the requested graph projection and its retained authored and topology text.
pub fn class_topology_complexity(
    view: &ClassTopologyView<'_>,
) -> crate::resources::ModelComplexity {
    let mut complexity =
        crate::resources::ClassComplexity::topology_from_view(view).as_model_complexity();
    let authored = crate::resources::ModelComplexity::from_serializable(view.model());
    let topology = crate::resources::ModelComplexity::from_serializable(view.topology());
    complexity.text_bytes = authored.text_bytes.saturating_add(view.retained_bytes());
    complexity.nesting_depth = complexity
        .nesting_depth
        .max(authored.nesting_depth)
        .max(topology.nesting_depth.saturating_add(1));
    complexity
}

#[cfg(test)]
#[test]
fn flat_class_topology_does_not_reserve_slots_for_implicit_namespaces() {
    let namespace_id = (0..128)
        .map(|index| format!("N{index}"))
        .collect::<Vec<_>>()
        .join(".");
    let parsed = crate::SemanticParser::new()
        .parse_diagram_sync(&format!(
            "classDiagram\nnamespace {namespace_id} {{\nclass Service\n}}\n"
        ))
        .unwrap();
    let crate::SemanticDiagramModel::Class(mut model) = parsed.model else {
        panic!("expected a canonical Class model");
    };
    assert_eq!(model.namespaces.len(), 128);
    let mut config = crate::MermaidConfig::empty_object();
    config.set_value("class.hierarchicalNamespaces", false.into());
    {
        let topology = model
            .topology(&config, &crate::OperationControl::new())
            .unwrap();
        assert_eq!(topology.namespaces().len(), 1);
        assert!(topology.topology().namespaces.capacity() < model.namespaces.len());
        assert_eq!(
            topology.namespace(&namespace_id).unwrap().route_id,
            format!("classId-{namespace_id}-127")
        );
    }
    for namespace in model.namespaces.values_mut() {
        namespace.explicit = false;
    }
    let topology = model
        .topology(&config, &crate::OperationControl::new())
        .unwrap();
    assert_eq!(topology.namespaces().len(), 0);
    assert_eq!(topology.topology().namespaces.capacity(), 0);
}

#[cfg(test)]
mod topology_serialization_tests {
    use super::*;
    use crate::resources::{ClassComplexity, ModelComplexity};
    use serde_json::{Map, Value};

    fn authored_model() -> ClassDiagramModel {
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(concat!(
                "classDiagram\nnamespace A.B {\nclass Service\nnote \"namespace note\"\n}\n",
                "class note0\nclass interface0\nIService ()-- Service\n",
            ))
            .unwrap();
        let crate::SemanticDiagramModel::Class(model) = parsed.model else {
            panic!("expected a canonical Class model");
        };
        model
    }

    fn flat_config() -> crate::MermaidConfig {
        let mut config = crate::MermaidConfig::empty_object();
        config.set_value("class.hierarchicalNamespaces", false.into());
        config
    }

    #[test]
    fn class_topology_serializes_all_authored_fields_and_effective_routing() {
        let mut model = authored_model();
        model.acc_title = Some("Class title".to_owned());
        model.acc_descr = Some("Class description".to_owned());
        model
            .namespace_facade_aliases
            .insert("Facade".to_owned(), "A.B".to_owned());
        model.classes["Service"].callback = Some(
            serde_json::from_value(serde_json::json!({
                "unknownMember": {"items": ["authored value", 7, {"nested": true}]}
            }))
            .unwrap(),
        );
        model.classes["Service"].members.push(ClassMember {
            member_type: "attribute".to_owned(),
            visibility: "+".to_owned(),
            id: "field".to_owned(),
            classifier: "static".to_owned(),
            parameters: "parameter".to_owned(),
            return_type: "Result".to_owned(),
            display_text: "+field: Result".to_owned(),
            css_style: "font-weight:bold".to_owned(),
        });
        model.style_precedence_facts = ClassStylePrecedenceFacts::from_witnesses(
            None,
            None,
            Some(ClassStyleDeclarationWitness::new(
                "Service".to_owned(),
                vec!["stroke:red".to_owned()],
                7,
            )),
            None,
        );
        let authored = serde_json::to_value(&model).unwrap();
        let view = model
            .topology(&flat_config(), &crate::OperationControl::new())
            .unwrap();
        let serialized = serde_json::to_value(&view).unwrap();
        for (field, value) in authored.as_object().unwrap() {
            assert_eq!(serialized.get(field), Some(value), "authored field {field}");
        }
        assert_eq!(
            serialized.as_object().unwrap().len(),
            authored.as_object().unwrap().len() + 1
        );
        assert_eq!(serialized["type"], "classDiagram");
        assert!(serialized["classes"]["Service"].get("domId").is_none());
        assert_eq!(
            serialized["classes"]["Service"]["callback"]["unknownMember"]["items"][0],
            "authored value"
        );
        assert_eq!(serialized["namespaceFacadeAliases"]["Facade"], "A.B");
        assert_eq!(
            serialized["stylePrecedenceFacts"]["inline_paint"]["target"],
            "Service"
        );
        assert!(serialized["namespaces"].get("A").is_some());
        assert!(serialized["topology"]["namespaces"].get("A").is_none());
        assert_eq!(
            serialized["topology"]["namespaceDomIds"]["A"],
            "classId-A-0"
        );
        assert_eq!(
            serialized["topology"]["namespaceDomIds"]["A.B"],
            "classId-A.B-1"
        );
        assert_eq!(serialized["topology"]["classParents"]["Service"], "A.B");
        assert_eq!(serialized["topology"]["namespaces"]["A.B"]["label"], "A.B");
        assert_eq!(
            serialized["topology"]["namespaces"]["A.B"]["classIds"],
            serde_json::json!(["Service"])
        );
        assert_eq!(
            serialized["topology"]["namespaces"]["A.B"]["noteIds"],
            serde_json::json!(["note0$1"])
        );
        assert_eq!(serialized["notes"][0]["id"], "note0");
        assert_eq!(
            serialized["topology"]["noteIds"],
            serde_json::json!(["note0$1"])
        );
        assert_eq!(
            serialized["topology"]["noteParents"],
            serde_json::json!(["A.B"])
        );
        assert_eq!(serialized["interfaces"][0]["id"], "interface0");
        assert_eq!(
            serialized["topology"]["interfaceIds"],
            serde_json::json!(["interface0$1"])
        );
        assert_eq!(
            serialized["topology"]["interfaceIdRemaps"]["interface0"],
            "interface0$1"
        );
        assert!(std::ptr::eq(view.model(), &model));
    }

    #[test]
    fn class_topology_complexity_reuses_effective_projection_and_complete_authored_text() {
        let mut model = authored_model();
        model.classes["Service"].callback = Some(
            serde_json::from_value(serde_json::json!({
                "unknownMember": {"retained": "all nested callback text"}
            }))
            .unwrap(),
        );
        let view = model
            .topology(&flat_config(), &crate::OperationControl::new())
            .unwrap();
        let projected = ClassComplexity::from_view(&view);
        assert_eq!(projected.nodes, 6);
        assert_eq!(projected.edges, 1);
        assert_eq!(projected.namespaces, 1);
        assert_eq!(projected.namespace_depth, 1);
        let expected = ModelComplexity::new(
            projected.nodes + projected.edges,
            ModelComplexity::from_serializable(&model).text_bytes + view.retained_bytes(),
            projected
                .namespace_depth
                .max(ModelComplexity::from_serializable(&view).nesting_depth),
        );
        assert_eq!(class_topology_complexity(&view), expected);
        let policy = crate::resources::InputResourcePolicy::for_profile(
            crate::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        policy
            .with_limit(
                crate::resources::InputResourceLimitId::MaxModelTextBytes,
                expected.text_bytes,
            )
            .unwrap()
            .check_model_complexity(class_topology_complexity(&view))
            .unwrap();
        let error = policy
            .with_limit(
                crate::resources::InputResourceLimitId::MaxModelTextBytes,
                expected.text_bytes - 1,
            )
            .unwrap()
            .check_model_complexity(class_topology_complexity(&view))
            .unwrap_err();
        assert_eq!(error.actual, expected.text_bytes);
    }

    #[test]
    fn class_topology_admits_callback_json_depth_before_serialization() {
        use crate::resources::{InputResourceLimitId, InputResourcePolicy, ResourceProfile};

        for array_depth in [8, 129] {
            let mut model = authored_model();
            let mut value = Value::String("callback text".to_owned());
            for _ in 0..array_depth {
                value = Value::Array(vec![value]);
            }
            model.classes["Service"].callback =
                Some(Map::from_iter([("unknownMember".to_owned(), value)]));
            let view = model
                .topology(&flat_config(), &crate::OperationControl::new())
                .unwrap();
            let complexity = class_topology_complexity(&view);
            // Class root -> classes map -> Class node -> callback map -> authored arrays.
            let depth = array_depth + 3;
            assert_eq!(complexity.nesting_depth, depth);
            assert_eq!(complexity.items, 7);
            let policy =
                InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
            policy
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth)
                .unwrap()
                .check_model_complexity(complexity)
                .unwrap();
            let error = policy
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth - 1)
                .unwrap()
                .check_model_complexity(complexity)
                .unwrap_err();
            assert_eq!(error.limit, "max_model_nesting_depth");
            assert_eq!(error.actual, depth);
            assert_eq!(error.max, depth - 1);
            assert_eq!(
                complexity.nesting_depth,
                ModelComplexity::from_serializable(&view).nesting_depth
            );
        }
    }

    #[test]
    fn class_topology_depth_includes_the_effective_topology_envelope() {
        let model = authored_model();
        let view = model
            .topology(&flat_config(), &crate::OperationControl::new())
            .unwrap();
        let authored_depth = ModelComplexity::from_serializable(&model).nesting_depth;
        let topology_depth = ModelComplexity::from_serializable(view.topology()).nesting_depth;
        assert_eq!(authored_depth, topology_depth);
        assert_eq!(
            class_topology_complexity(&view).nesting_depth,
            topology_depth + 1
        );
        assert_eq!(
            class_topology_complexity(&view).nesting_depth,
            ModelComplexity::from_serializable(&view).nesting_depth
        );
    }
}
