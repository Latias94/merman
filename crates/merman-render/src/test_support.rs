//! Source-fixture adapters used only by renderer unit tests.

use merman_model::presentation::PresentationConfig;
use merman_model::resources::{InputResourcePolicy, ResourceProfile};
use merman_model::{MermaidConfig, OperationControl};

pub(crate) fn presentation_config(value: serde_json::Value) -> PresentationConfig {
    presentation_config_from_config(MermaidConfig::from_value(value))
}

pub(crate) fn presentation_config_from_config(config: MermaidConfig) -> PresentationConfig {
    PresentationConfig::from_config_controlled(
        config,
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn prepare_flowchart_edge_style_plan(
    model: &crate::flowchart::FlowchartModel,
    config: &PresentationConfig,
    swimlane: bool,
    meter: &crate::resources::OperationWorkMeter,
) -> crate::Result<crate::svg::FlowchartEdgeStylePlan> {
    crate::svg::FlowchartEdgeStylePlan::prepare_with_class_definitions(
        model,
        config,
        swimlane,
        &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
        meter,
    )
}
