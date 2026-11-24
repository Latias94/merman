use super::{ThemeDocumentV2, ThemeError};

pub(super) fn parse(bytes: &[u8]) -> Result<ThemeDocumentV2, ThemeError> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let document = serde_path_to_error::deserialize(&mut decoder).map_err(|error| {
        let path = error.path().to_string();
        ThemeError::json(path, normalize_duplicate_field(error.into_inner()))
    })?;
    decoder
        .end()
        .map_err(|error| ThemeError::json(".".into(), error))?;
    Ok(document)
}

fn normalize_duplicate_field(error: serde_json::Error) -> serde_json::Error {
    if error.is_data() {
        let message = error.to_string();
        if let Some(field_and_location) = message.strip_prefix("duplicate field ") {
            // Keep the public diagnostic wording. serde_json's custom error constructor
            // recovers the line and column from its own display suffix.
            return serde::de::Error::custom(format_args!("duplicate key {field_and_location}"));
        }
    }
    error
}
