//! Pure full-data and paired-context accounting.

use super::*;
use crate::presentation::{PresentationContext, PresentationDiagram, RenderSemanticModel};

/// Applies absolute construction limits before family depth accounting builds graph indexes.
/// The preliminary check uses the same item/text totals; it does not charge an operation ledger.
#[allow(
    unused_variables,
    reason = "Construction checkpoints inside depth indexes depend on the selected families."
)]
pub(crate) fn check_presentation_construction(
    model: &RenderSemanticModel,
    context: &PresentationContext,
    policy: &InputResourcePolicy,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<(), InputResourceLimitExceeded>> {
    let mut complexity = match model {
        #[cfg(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        ))]
        RenderSemanticModel::Flowchart(model) => {
            FlowchartComplexity::without_depth(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => {
            let mut complexity = ClassComplexity::topology_without_depth(&model.topology_view())
                .as_model_complexity();
            complexity.text_bytes = ModelComplexity::from_serializable(model.model())
                .text_bytes
                .saturating_add(model.presentation().retained_bytes());
            complexity
        }
        _ => model_complexity_from_render_model(model),
    };
    complexity.text_bytes = complexity
        .text_bytes
        .saturating_add(context.retained_text_bytes());
    if let Err(error) = policy.check_model_complexity(complexity) {
        return Ok(Err(error));
    }
    let nesting_depth: Option<usize> = match model {
        #[cfg(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        ))]
        RenderSemanticModel::Flowchart(model) => Some(
            family::flowchart_subgraph_depth_with_control(model, Some(control))?,
        ),
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => Some(family::class_namespace_depth_with_control(
            model.topology_view().effective_namespaces(),
            Some(control),
        )?),
        _ => None,
    };
    if let Some(depth) = nesting_depth {
        complexity.nesting_depth = depth;
        Ok(policy.check_model_complexity(complexity))
    } else {
        Ok(Ok(()))
    }
}

pub fn model_complexity_from_render_model(model: &RenderSemanticModel) -> ModelComplexity {
    match model {
        RenderSemanticModel::Error(model) => ModelComplexity::from_serializable(model),
        RenderSemanticModel::CustomJson(model) => {
            let mut complexity = ModelComplexity::from_json(model.value());
            complexity.text_bytes = complexity
                .text_bytes
                .saturating_add(model.model_name().len());
            complexity
        }
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => {
            MindmapComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => model.model_complexity(),
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => {
            SequenceComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-zenuml")]
        RenderSemanticModel::Zenuml(model) => {
            ZenumlComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        ))]
        RenderSemanticModel::Flowchart(model) => {
            FlowchartComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => {
            let mut complexity = ClassComplexity::topology_from_model(model).as_model_complexity();
            complexity.text_bytes = ModelComplexity::from_serializable(model.model())
                .text_bytes
                .saturating_add(model.presentation().retained_bytes());
            complexity
        }
        #[cfg(feature = "diagram-c4")]
        RenderSemanticModel::C4(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-cynefin")]
        RenderSemanticModel::Cynefin(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-railroad")]
        RenderSemanticModel::Railroad(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(model) => model_complexity_from_kanban(model),
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-pie")]
        RenderSemanticModel::Pie(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(model) => ModelComplexity::from_requirement(model),
        #[cfg(feature = "diagram-sankey")]
        RenderSemanticModel::Sankey(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-radar")]
        RenderSemanticModel::Radar(model) => model_complexity_from_radar(model),
        #[cfg(feature = "diagram-info")]
        RenderSemanticModel::Info(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-treemap")]
        RenderSemanticModel::Treemap(model) => {
            TreemapComplexity::from_render_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(model) => model.model_complexity(),
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-quadrant-chart")]
        RenderSemanticModel::QuadrantChart(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-ishikawa")]
        RenderSemanticModel::Ishikawa(model) => {
            IshikawaComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-event-modeling")]
        RenderSemanticModel::EventModeling(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-venn")]
        RenderSemanticModel::Venn(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-wardley")]
        RenderSemanticModel::Wardley(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => ModelComplexity::from_serializable(model),
    }
}

/// Counts full model data and its retained family context, without changing metadata accounting.
pub fn model_complexity_from_presentation(diagram: &PresentationDiagram) -> ModelComplexity {
    let mut complexity = model_complexity_from_render_model(diagram.model());
    complexity.text_bytes = complexity
        .text_bytes
        .saturating_add(diagram.render_context().retained_text_bytes());
    complexity
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-class")))]
mod construction_tests {
    use super::*;
    use crate::OperationControl;
    use serde_json::json;

    fn indexed_models() -> Vec<RenderSemanticModel> {
        vec![
            #[cfg(feature = "diagram-flowchart")]
            RenderSemanticModel::Flowchart(
                serde_json::from_value(json!({
                    "nodes": [], "edges": [], "subgraphs": [
                        {"id": "Outer", "title": "Outer", "nodes": ["Inner", "Inner"]},
                        {"id": "Inner", "title": "Inner", "nodes": []}
                    ]
                }))
                .unwrap(),
            ),
            #[cfg(feature = "diagram-class")]
            RenderSemanticModel::Class(crate::families::class::ClassDiagram::new(
                serde_json::from_value(json!({
                    "type": "classDiagram", "direction": "TB", "classes": {},
                    "namespaces": {
                        "Outer": {"id": "Outer", "explicit": true},
                        "Outer.Inner": {"id": "Outer.Inner", "parent": "Outer", "explicit": true}
                    },
                    "constants": {
                        "lineType": {"line": 0, "dottedLine": 1},
                        "relationType": {"none": -1, "aggregation": 0, "extension": 1,
                            "composition": 2, "dependency": 3, "lollipop": 4}
                    }
                }))
                .unwrap(),
                &crate::MermaidConfig::from_value(
                    json!({"class": {"hierarchicalNamespaces": true}}),
                ),
            )),
        ]
    }

    #[test]
    fn construction_preflight_preserves_counts_limits_and_error_priority() {
        for model in indexed_models() {
            let expected = model_complexity_from_render_model(&model);
            assert!(expected.items >= 2);
            assert!(expected.text_bytes >= 2);
            assert_eq!(expected.nesting_depth, 2, "{}", model.kind());
            let exact = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelItems, expected.items)
                .unwrap()
                .with_limit(InputResourceLimitId::MaxModelTextBytes, expected.text_bytes)
                .unwrap()
                .with_limit(
                    InputResourceLimitId::MaxModelNestingDepth,
                    expected.nesting_depth,
                )
                .unwrap();
            assert!(
                check_presentation_construction(
                    &model,
                    &PresentationContext::None,
                    &exact,
                    &OperationControl::new(),
                )
                .unwrap()
                .is_ok()
            );
            for (id, actual) in [
                (InputResourceLimitId::MaxModelItems, expected.items),
                (InputResourceLimitId::MaxModelTextBytes, expected.text_bytes),
                (
                    InputResourceLimitId::MaxModelNestingDepth,
                    expected.nesting_depth,
                ),
            ] {
                let policy = exact.with_limit(id, actual - 1).unwrap();
                let control = OperationControl::new();
                if id != InputResourceLimitId::MaxModelNestingDepth {
                    // The next depth-index checkpoint must remain unreached on cheap rejection.
                    control.cancel_after_checkpoints(0);
                }
                let rejected = check_presentation_construction(
                    &model,
                    &PresentationContext::None,
                    &policy,
                    &control,
                )
                .unwrap()
                .unwrap_err();
                assert_eq!(
                    rejected,
                    policy.check_model_complexity(expected).unwrap_err()
                );
            }
            let policy = exact
                .with_limit(InputResourceLimitId::MaxModelItems, 1)
                .unwrap()
                .with_limit(InputResourceLimitId::MaxModelTextBytes, 1)
                .unwrap()
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, 1)
                .unwrap();
            assert_eq!(
                check_presentation_construction(
                    &model,
                    &PresentationContext::None,
                    &policy,
                    &OperationControl::new(),
                )
                .unwrap()
                .unwrap_err()
                .limit,
                "max_model_items"
            );
        }
    }

    #[test]
    fn construction_depth_indexes_observe_cancellation() {
        for model in indexed_models() {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(2);
            assert!(
                check_presentation_construction(
                    &model,
                    &PresentationContext::None,
                    &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
                    &control,
                )
                .is_err(),
                "{}",
                model.kind()
            );
        }
    }
}
