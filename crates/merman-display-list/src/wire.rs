use serde::{Deserialize, Deserializer};

/// Internally tagged unit variants otherwise ignore fields beyond their tag.
pub(crate) fn deserialize_empty_object<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct EmptyObject {}

    EmptyObject::deserialize(deserializer).map(|_| ())
}
