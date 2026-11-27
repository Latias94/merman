#[cfg(any(
    test,
    feature = "diagram-agentflow",
    feature = "diagram-kanban",
    feature = "diagram-sequence"
))]
use crate::yaml_config::{YamlCollectionText, parse_yaml_with_capture_controlled};
use serde_json::Value;

#[cfg(any(test, feature = "diagram-agentflow", feature = "diagram-sequence"))]
pub(crate) fn parse_mermaid_inline_object_controlled(
    input: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Value, String>> {
    let parsed = parse_mermaid_inline_object_with_text_controlled(input, &[], control)?;
    Ok(parsed.map(|(value, _)| value))
}

#[cfg(any(
    test,
    feature = "diagram-agentflow",
    feature = "diagram-kanban",
    feature = "diagram-sequence"
))]
pub(crate) fn parse_mermaid_inline_object_with_text_controlled(
    input: &str,
    collection_text_keys: &[&'static str],
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<(Value, YamlCollectionText), String>> {
    let captured = parse_yaml_with_capture_controlled(
        &inline_yaml(input),
        crate::MAX_DIAGRAM_NESTING_DEPTH,
        collection_text_keys,
        control,
    )?;
    Ok(captured
        .value
        .map(|value| (value, captured.collection_text)))
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) fn parse_mermaid_inline_object_with_order_controlled(
    input: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<(Value, Vec<String>), String>> {
    crate::yaml_config::parse_yaml_value_with_root_order_controlled(
        &inline_yaml(input),
        crate::MAX_DIAGRAM_NESTING_DEPTH,
        control,
    )
}

fn inline_yaml(input: &str) -> String {
    if input.contains('\n') {
        format!("{input}\n")
    } else {
        format!("{{\n{input}\n}}")
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use merman_model::config::inline_config_scalar_to_string as value_to_string;

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane",))]
pub(crate) fn value_to_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inline_shape_data_uses_the_canonical_yaml_parser() {
        let value = parse_mermaid_inline_object_controlled(
            r#"shape: rounded, label: "End", flag: true"#,
            &crate::OperationControl::new(),
        )
        .expect("active parse control")
        .expect("valid Mermaid inline shape data");
        assert_eq!(
            value,
            json!({"shape": "rounded", "label": "End", "flag": true})
        );
    }

    #[test]
    fn controlled_inline_shape_data_propagates_cancellation() {
        let control = crate::OperationControl::new();
        control.cancel();

        assert!(matches!(
            parse_mermaid_inline_object_controlled("shape: rounded", &control),
            Err(crate::OperationCancelled { .. })
        ));
    }
}
