use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) fn from_value_ref<T: DeserializeOwned>(value: &Value) -> Result<T, serde_json::Error> {
    T::deserialize(value)
}
