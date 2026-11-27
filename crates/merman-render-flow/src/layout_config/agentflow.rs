use std::sync::Arc;

use merman_model::json::clone_value_nonrecursive;
use merman_model::presentation::{
    FrozenPresentationConfigParts, PresentationConfig, PresentationConfigError,
};
use merman_model::resources::{InputResourcePolicy, ResourceProfile};
use merman_model::{OperationControl, OperationControlResult};
use merman_render_core::config::json_value_is_truthy;
use serde_json::{Value, json};

pub fn project_agentflow_flowchart_config(
    config: &PresentationConfig,
    uses_family_spacing: bool,
    control: &OperationControl,
) -> OperationControlResult<Result<PresentationConfig, PresentationConfigError>> {
    let mut projected = config.as_config().clone();
    let root = config.as_value();
    let agentflow = root.get("agentflow").unwrap_or(&Value::Null);
    // Agentflow's renderer owns the root viewport. Defaults here mirror its `??`
    // expressions; normal MermaidEngine configuration already includes schema defaults.
    // Overwrite even absent family values so unrelated Flowchart settings cannot leak in.
    for (key, fallback) in [
        ("diagramPadding", json!(8)),
        ("useMaxWidth", json!(true)),
        ("titleTopMargin", json!(0)),
    ] {
        let value = agentflow.get(key).filter(|value| !value.is_null());
        projected.set_value(
            &format!("flowchart.{key}"),
            value.map(clone_value_nonrecursive).unwrap_or(fallback),
        );
    }
    // Dagre takes a truthy root override before the renderer's family spacing.
    // Mermaid's ELK adapter ignores all three configuration namespaces for spacing.
    // Our shared adapter reads Flowchart settings, so keep its default spacing for ELK.
    for key in ["nodeSpacing", "rankSpacing"] {
        let value = uses_family_spacing
            .then(|| {
                root.get(key)
                    .filter(|value| json_value_is_truthy(value))
                    .or_else(|| {
                        agentflow
                            .get(key)
                            .filter(|value| json_value_is_truthy(value))
                    })
            })
            .flatten();
        projected.set_value(
            &format!("flowchart.{key}"),
            value
                .map(clone_value_nonrecursive)
                .unwrap_or_else(|| json!(50)),
        );
    }
    // AgentflowDB assigns these values to each node before the shared shape helpers
    // read them. Node-specific values precede Flowchart configuration; only a falsy
    // wrappingWidth falls through via labelHelper's JavaScript `||` expression.
    for key in ["minNodeWidth", "wrappingWidth"] {
        if let Some(value) = agentflow.get(key) {
            if key == "wrappingWidth" && !json_value_is_truthy(value) {
                continue;
            }
            projected.set_value(&format!("flowchart.{key}"), clone_value_nonrecursive(value));
        }
    }
    let parts = FrozenPresentationConfigParts::new(projected)
        .with_explicit_config_paths(Arc::clone(config.shared_explicit_config_paths()));
    // Existing render admission excludes config. This construction policy validates derived data
    // without recharging the request's model limits or declaring source authority.
    let construction_policy =
        InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    PresentationConfig::try_from_frozen_parts_controlled(parts, &construction_policy, control)
}
