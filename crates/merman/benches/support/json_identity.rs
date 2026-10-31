use merman::model::ManagedJson;
use serde_json::Value;

pub(crate) fn json_identity_bytes(value: &ManagedJson) -> Vec<u8> {
    let mut canonical = value.clone().into_unmanaged_value();
    let mut pending = vec![&mut canonical];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                object.sort_keys();
                pending.extend(object.values_mut());
            }
            Value::Array(array) => pending.extend(array.iter_mut()),
            _ => {}
        }
    }
    let canonical = ManagedJson::from_value(canonical);
    let mut output = Vec::new();
    canonical
        .write_json(&mut output)
        .expect("benchmark JSON output must serialize");
    output
}
