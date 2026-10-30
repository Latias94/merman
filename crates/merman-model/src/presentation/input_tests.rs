use super::*;
use crate::families::error::ErrorDiagramModel;
use crate::resources::{InputResourceLimitId, InputResourcePolicy, ResourceProfile};
use crate::{MermaidConfig, OperationControl};
use serde_json::json;

fn metadata(diagram_type: &str) -> AuthoredPresentationMetadata {
    AuthoredPresentationMetadata {
        diagram_type: diagram_type.to_string(),
        config: MermaidConfig::empty_object(),
        effective_config: MermaidConfig::empty_object(),
        title: None,
        theme_recipe: None,
    }
}

fn error_model() -> RenderSemanticModel {
    RenderSemanticModel::Error(ErrorDiagramModel::new("error"))
}

#[test]
fn checked_input_rejects_metadata_from_another_family() {
    let result = PresentationDiagram::try_new(
        metadata("sequence"),
        error_model(),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap();
    assert!(matches!(
        result,
        Err(PresentationDataError::ModelKind { .. })
    ));
}

#[test]
fn cancelled_input_observes_the_terminal_before_invalid_pairing() {
    let control = OperationControl::new();
    control.cancel();
    assert!(
        PresentationDiagram::try_new(
            metadata("sequence"),
            error_model(),
            PresentationContext::None,
            &InputResourcePolicy::default(),
            &control,
        )
        .is_err()
    );
}

#[test]
fn named_custom_data_does_not_gain_a_builtin_family_from_its_metadata() {
    let diagram = PresentationDiagram::try_new(
        metadata("flowchart-v2"),
        RenderSemanticModel::CustomJson(CustomJsonRenderModel::new(
            "custom-layout-adapter",
            json!({"label": "custom input"}),
        )),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(diagram.family_id(), None);
    assert_eq!(diagram.model().kind(), "custom-json");
}

#[test]
fn construction_bounds_model_data_independently_of_later_output_admission() {
    let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
        .with_limit(InputResourceLimitId::MaxModelTextBytes, 3)
        .unwrap();
    let result = PresentationDiagram::try_new(
        metadata("error"),
        error_model(),
        PresentationContext::None,
        &policy,
        &OperationControl::new(),
    )
    .unwrap();
    assert!(matches!(
        result,
        Err(PresentationDataError::ResourceLimit(_))
    ));
}

#[cfg(feature = "diagram-info")]
#[test]
fn family_context_is_checked_even_when_the_payload_is_empty() {
    let result = PresentationDiagram::try_new(
        metadata("error"),
        error_model(),
        PresentationContext::Info(crate::families::info::InfoOutputData::Empty),
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap();
    assert!(matches!(
        result,
        Err(PresentationDataError::ContextKind { .. })
    ));
}

#[cfg(all(feature = "diagram-info", feature = "compat-json"))]
#[test]
fn explicit_empty_facts_survive_an_independent_data_projection() {
    use crate::families::info::{InfoDiagramModel, InfoOutputData};

    let construct = |context| {
        PresentationDiagram::try_new(
            metadata("info"),
            RenderSemanticModel::Info(InfoDiagramModel::default()),
            context,
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    };
    let explicit = construct(PresentationContext::Info(InfoOutputData::Empty));
    let ordinary = construct(PresentationContext::None);
    assert_eq!(
        explicit.compatibility_json().unwrap().as_value(),
        &json!({})
    );
    assert_ne!(
        explicit.compatibility_json().unwrap().as_value(),
        ordinary.compatibility_json().unwrap().as_value(),
    );
    let (meta, model, context) = explicit.into_parts();
    let restored = PresentationDiagram::try_from_frozen(
        meta,
        model,
        context,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        restored.compatibility_json().unwrap().as_value(),
        &json!({})
    );
}

#[test]
fn family_paint_declarations_require_the_matching_model() {
    let config = PresentationConfig::try_from_frozen_parts_controlled(
        FrozenPresentationConfigParts::new(MermaidConfig::empty_object())
            .with_theme_compatibility(
                Some(
                    ThemeCompatibilityRecipe::try_new([0x41; 32], MermaidConfig::empty_object())
                        .unwrap(),
                ),
                std::sync::Arc::from([]),
            )
            .with_family_paint_inputs(Some(FamilyPaintInputs::Er(ErPaintInputs::default()))),
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    let result = PresentationDiagram::try_from_frozen(
        PresentationMetadata {
            diagram_type: "error".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: config,
            title: None,
        },
        error_model(),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap();
    assert!(matches!(
        result,
        Err(PresentationDataError::PaintFamily { .. })
    ));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn independent_flowchart_css_validation_covers_absent_context_and_keeps_error_order() {
    use crate::families::flowchart::{
        FlowSubgraphVertexStyle, FlowchartModel, FlowchartRenderContext,
        FlowchartRenderLabelSources,
    };

    let mut model: FlowchartModel = serde_json::from_value(json!({
        "nodes": [], "edges": [], "subgraphs": [{"id": "G", "title": "G", "nodes": []}]
    }))
    .unwrap();
    model
        .subgraph_vertex_styles
        .insert("G".to_string(), 1, FlowSubgraphVertexStyle::default());
    let construct = |context, policy: &InputResourcePolicy, control: &OperationControl| {
        PresentationDiagram::try_from_frozen(
            PresentationMetadata {
                diagram_type: "flowchart-v2".to_string(),
                config: MermaidConfig::empty_object(),
                effective_config: PresentationConfig::empty_object(),
                title: None,
            },
            RenderSemanticModel::Flowchart(model.clone()),
            context,
            policy,
            control,
        )
    };
    let policy = InputResourcePolicy::default();
    for context in [
        PresentationContext::None,
        PresentationContext::Flowchart(FlowchartRenderContext::default()),
    ] {
        assert!(matches!(
            construct(context, &policy, &OperationControl::new()).unwrap(),
            Err(PresentationDataError::FamilyContext {
                family: "flowchart",
                message: "flowchart subgraph CSS source does not match its declaration ordinal",
            })
        ));
    }
    let mut labels = FlowchartRenderLabelSources::default();
    labels.insert_node("unknown".to_string(), "label".to_string());
    let context = FlowchartRenderContext::new(
        labels,
        Default::default(),
        Default::default(),
        &model.subgraphs,
    );
    assert!(matches!(
        construct(
            PresentationContext::Flowchart(context),
            &policy,
            &OperationControl::new()
        )
        .unwrap(),
        Err(PresentationDataError::FamilyContext {
            message: "flowchart node label refers to an unknown node",
            ..
        })
    ));
    let limited = policy
        .with_limit(InputResourceLimitId::MaxModelTextBytes, 1)
        .unwrap();
    assert!(matches!(
        construct(
            PresentationContext::None,
            &limited,
            &OperationControl::new()
        )
        .unwrap(),
        Err(PresentationDataError::ResourceLimit(_))
    ));
    let control = OperationControl::new();
    control.cancel();
    assert!(construct(PresentationContext::None, &limited, &control).is_err());
}

#[cfg(feature = "diagram-mindmap")]
mod mindmap {
    use super::*;
    use crate::families::mindmap::{MindmapDiagramRenderModel, MindmapDiagramRenderNode};

    fn model() -> MindmapDiagramRenderModel {
        MindmapDiagramRenderModel {
            nodes: vec![
                serde_json::from_value::<MindmapDiagramRenderNode>(json!({
                    "id": "0", "domId": "node-0", "label": "Root", "shape": "mindmap",
                    "cssClasses": "mindmap-node section-root section--1", "level": 0
                }))
                .unwrap(),
            ],
            edges: Vec::new(),
        }
    }

    #[cfg(feature = "compat-json")]
    fn tree(child_count: usize) -> MindmapDiagramRenderModel {
        let mut model = model();
        model.nodes[0].node_id = "repeated-authored-name".to_string();
        for id in 1..=child_count {
            let mut child = model.nodes[0].clone();
            child.id = id.to_string();
            child.dom_id = format!("node-{id}");
            child.level = 1;
            model.nodes.push(child);
            model.edges.push(
                serde_json::from_value(json!({
                    "id": format!("edge-{id}"), "start": "0", "end": id.to_string(),
                }))
                .unwrap(),
            );
        }
        model
    }

    #[cfg(feature = "compat-json")]
    fn diagram(model: MindmapDiagramRenderModel) -> PresentationDiagram {
        PresentationDiagram::try_new(
            metadata("mindmap"),
            RenderSemanticModel::Mindmap(model),
            PresentationContext::Mindmap {
                diagram_id: "explicit-tree-id".to_string(),
            },
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn nonempty_mindmap_requires_an_explicit_output_identity() {
        let result = PresentationDiagram::try_new(
            metadata("mindmap"),
            RenderSemanticModel::Mindmap(model()),
            PresentationContext::None,
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap();
        assert!(matches!(
            result,
            Err(PresentationDataError::MissingOutputIdentity { family: "mindmap" })
        ));
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn frozen_mindmap_identity_is_reused_without_runtime_state() {
        let diagram = PresentationDiagram::try_new(
            metadata("mindmap"),
            RenderSemanticModel::Mindmap(model()),
            PresentationContext::Mindmap {
                diagram_id: "external-producer-id".to_string(),
            },
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let first = diagram.compatibility_json().unwrap();
        let second = diagram.clone().compatibility_json().unwrap();
        assert_eq!(first, second);
        assert_eq!(first["diagramId"], "external-producer-id");
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn independent_projection_rejects_a_cycle_without_recursing() {
        let mut model = model();
        model
            .edges
            .push(serde_json::from_value(json!({"id":"loop", "start":"0", "end":"0"})).unwrap());
        let diagram = PresentationDiagram::try_new(
            metadata("mindmap"),
            RenderSemanticModel::Mindmap(model),
            PresentationContext::Mindmap {
                diagram_id: "cycle".to_string(),
            },
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert!(
            diagram
                .compatibility_json()
                .unwrap_err()
                .message()
                .contains("cyclic")
        );
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn repeated_child_edges_are_rejected_for_either_parent_identity() {
        for parent in ["0", "2"] {
            let mut model = tree(2);
            model.edges.push(
                serde_json::from_value(json!({
                    "id": "duplicate-child", "start": parent, "end": "1",
                }))
                .unwrap(),
            );
            let diagram = diagram(model);
            assert!(
                diagram
                    .compatibility_json()
                    .unwrap_err()
                    .message()
                    .contains("more than one parent edge")
            );
            let control = OperationControl::new();
            control.cancel();
            assert!(diagram.compatibility_json_controlled(&control).is_err());
        }
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn distinct_internal_nodes_keep_repeated_authored_names_and_labels() {
        let value = diagram(tree(2)).compatibility_json().unwrap();
        let children = value["rootNode"]["children"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["nodeId"], "repeated-authored-name");
        assert_eq!(children[0]["nodeId"], children[1]["nodeId"]);
        assert_eq!(children[0]["descr"], children[1]["descr"]);
        assert_ne!(children[0]["id"], children[1]["id"]);
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn raw_model_missing_identity_drops_deep_projection_values_iteratively() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut value = json!("deep config");
                for _ in 0..2048 {
                    value = serde_json::Value::Array(vec![value]);
                }
                let policy =
                    InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
                let effective_config = PresentationConfig::from_config_controlled(
                    MermaidConfig::from_value(value),
                    &policy,
                    &OperationControl::new(),
                )
                .unwrap()
                .unwrap();
                let metadata = PresentationMetadata {
                    diagram_type: "mindmap".to_string(),
                    config: MermaidConfig::empty_object(),
                    effective_config,
                    title: None,
                };
                let error = RenderSemanticModel::Mindmap(model())
                    .compatibility_json(&metadata)
                    .unwrap_err();
                assert!(error.message().contains("diagram id is missing"));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
