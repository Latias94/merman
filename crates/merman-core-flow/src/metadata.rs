use merman_model::{OperationControl, OperationControlResult};
use serde_json::Value;

/// Applies Mermaid's inline metadata key policy before a family stores the decoded tree.
pub(crate) fn strip_prototype_keys(
    value: &mut Value,
    control: &OperationControl,
) -> OperationControlResult<()> {
    // The inline YAML reader bounds materialized size and depth and rejects cyclic aliases.
    let mut pending = vec![value];
    let mut visited = 0usize;
    while let Some(value) = pending.pop() {
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited += 1;
        match value {
            Value::Object(object) => {
                object.retain(|key, _| {
                    !matches!(key.as_str(), "__proto__" | "constructor" | "prototype")
                });
                pending.extend(object.values_mut());
            }
            Value::Array(items) => pending.extend(items.iter_mut()),
            _ => {}
        }
    }
    control.checkpoint()
}
