use serde_json::Value;

/// Parses Mermaid's inline shape-data object with the same YAML-compatible path used by the
/// public configuration pipeline. This is language behavior, not an optional fallback.
pub(crate) fn parse_mermaid_inline_object(input: &str) -> Result<Value, String> {
    let yaml_data = if input.contains('\n') {
        format!("{input}\n")
    } else {
        format!("{{\n{input}\n}}")
    };
    crate::yaml_config::parse_yaml_value(&yaml_data, crate::MAX_DIAGRAM_NESTING_DEPTH)
}

pub(crate) fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

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

pub(crate) fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inline_shape_data_uses_the_canonical_yaml_parser() {
        let value = parse_mermaid_inline_object(r#"shape: rounded, label: "End", flag: true"#)
            .expect("valid Mermaid inline shape data");
        assert_eq!(
            value,
            json!({"shape": "rounded", "label": "End", "flag": true})
        );
    }
}
