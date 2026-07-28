use serde_json::{Map, Value};

pub const FIXTURE_JSON: &str = include_str!("assets/semantic-operations-v1.json");

#[derive(Debug)]
pub struct SemanticOperationFixtures {
    pub cases: Vec<SemanticOperationCase>,
}

#[derive(Debug)]
pub struct SemanticOperationCase {
    pub operation_id: String,
    pub source: String,
    pub uri: Option<String>,
    pub options: Option<Value>,
    pub expected_media_type: Option<String>,
    pub expected_error_kind: Option<String>,
    pub payload_invariants: Vec<String>,
}

pub fn load() -> SemanticOperationFixtures {
    let root: Value = serde_json::from_str(FIXTURE_JSON)
        .expect("semantic operation fixtures must contain valid JSON");
    let root = object(&root, "fixture root");
    assert_eq!(
        root.get("schema_version").and_then(Value::as_u64),
        Some(1),
        "unsupported semantic operation fixture schema"
    );
    let cases = root
        .get("cases")
        .and_then(Value::as_array)
        .expect("fixture root.cases must be an array")
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let label = format!("fixture case {index}");
            let value = object(value, &label);
            SemanticOperationCase {
                operation_id: string(value, "operation_id", &label),
                source: string(value, "source", &label),
                uri: optional_string(value, "uri", &label),
                options: value.get("options").cloned(),
                expected_media_type: optional_string(value, "expected_media_type", &label),
                expected_error_kind: optional_string(value, "expected_error_kind", &label),
                payload_invariants: value
                    .get("payload_invariants")
                    .and_then(Value::as_array)
                    .unwrap_or_else(|| panic!("{label}.payload_invariants must be an array"))
                    .iter()
                    .map(|invariant| {
                        invariant
                            .as_str()
                            .unwrap_or_else(|| {
                                panic!("{label}.payload_invariants entries must be strings")
                            })
                            .to_owned()
                    })
                    .collect(),
            }
        })
        .collect();
    SemanticOperationFixtures { cases }
}

fn object<'a>(value: &'a Value, label: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{label} must be an object"))
}

fn string(value: &Map<String, Value>, key: &str, label: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{label}.{key} must be a string"))
        .to_owned()
}

fn optional_string(value: &Map<String, Value>, key: &str, label: &str) -> Option<String> {
    value.get(key).map(|_| string(value, key, label))
}
