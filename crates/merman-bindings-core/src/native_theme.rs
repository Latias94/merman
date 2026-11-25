use std::borrow::Cow;

use merman::theme::{
    Appearance, BuiltinPreset, ThemeCompiler, ThemeError, ThemePrecedence, ThemeSelection,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};

use crate::common::{BindingError, internal_json_error};

// One document byte can occupy six bytes in a JSON string escape. Leave room for the envelope.
const RESOLVE_REQUEST_MAX_BYTES: usize = ThemeCompiler::new().max_bytes() * 6 + 1024;

#[derive(Deserialize)]
struct OptionsWithTheme<'a> {
    #[serde(borrow)]
    theme: Option<&'a RawValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeSelectionJson<'a> {
    #[serde(default, deserialize_with = "present")]
    preset: Option<String>,
    #[serde(default, borrow, deserialize_with = "present")]
    document: Option<&'a RawValue>,
    #[serde(default, deserialize_with = "present")]
    appearance: Option<Appearance>,
    #[serde(default)]
    precedence: ThemePrecedence,
}

fn present<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Uses the original JSON slice so duplicate keys in an embedded document reach the compiler.
pub(crate) fn binding_theme(
    options_json: &[u8],
    options: &Value,
) -> Result<Option<ThemeSelection>, BindingError> {
    let Some(value) = options.get("theme") else {
        return Ok(None);
    };
    let raw: OptionsWithTheme<'_> = serde_json::from_slice(options_json).map_err(|error| {
        BindingError::invalid_options_json(format!("invalid theme option: {error}"))
    })?;
    let Some(raw) = raw.theme else {
        return Ok(None);
    };

    if value.as_str() == Some("none") {
        return Ok(None);
    }
    if let Some(preset) = value.as_str() {
        return select_preset(preset, None, ThemePrecedence::Host).map(Some);
    }
    if value.get("schema_version").is_some() {
        return compile_document(raw, None).map(Some);
    }
    if !value.is_object() {
        return Err(BindingError::invalid_options_json(
            "theme must be a preset ID, a v2 document, or a selection object",
        ));
    }

    let selection: ThemeSelectionJson<'_> = serde_json::from_str(raw.get()).map_err(|error| {
        BindingError::invalid_options_json(format!("invalid theme selection: {error}"))
    })?;
    let theme = match (selection.preset.as_deref(), selection.document) {
        (Some(preset), None) => select_preset(preset, selection.appearance, selection.precedence)?,
        (None, Some(document)) => compile_document(document, selection.appearance)?,
        _ => {
            return Err(BindingError::invalid_options_json(
                "theme selection requires exactly one of `preset` or `document`",
            ));
        }
    };
    Ok(Some(theme.with_precedence(selection.precedence)))
}

#[cfg(feature = "svg")]
pub(crate) fn deserialize_background<'de, D>(
    deserializer: D,
) -> Result<Option<merman::theme::Color>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use merman::theme::Color;
    let Some(color) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let color = if color == "transparent" {
        Color::TRANSPARENT
    } else {
        Color::parse(&color)
            .map_err(|error| serde::de::Error::custom(format!("invalid background: {error}")))?
    };
    Ok(Some(color))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(crate) fn deserialize_export_matte<'de, D>(
    deserializer: D,
) -> Result<Option<merman::theme::Color>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(value) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let color = merman::theme::Color::parse(&value)
        .map_err(|error| serde::de::Error::custom(format!("invalid export_matte: {error}")))?;
    if color.channels()[3] != u8::MAX {
        return Err(serde::de::Error::custom("export_matte must be opaque"));
    }
    Ok(Some(color))
}

fn select_preset(
    preset: &str,
    appearance: Option<Appearance>,
    precedence: ThemePrecedence,
) -> Result<ThemeSelection, BindingError> {
    let preset = preset_from_id(preset)?;
    if let Some(requested) = appearance
        && !preset.supported_appearances().contains(&requested)
    {
        return match precedence {
            ThemePrecedence::Source => Ok(ThemeSelection::preset(preset, appearance)),
            ThemePrecedence::Host => Err(BindingError::invalid_argument(format!(
                "theme.appearance `{requested}` is unavailable for preset `{preset}`"
            ))),
        };
    }
    ThemeSelection::resolved_preset(preset, appearance)
        .map_err(|error| compile_error("theme", error))
}

fn compile_document(
    document: &RawValue,
    appearance: Option<Appearance>,
) -> Result<ThemeSelection, BindingError> {
    ThemeCompiler::new()
        .compile_json(document.get().as_bytes(), appearance)
        .map(ThemeSelection::compiled)
        .map_err(|error| compile_error("theme.document", error))
}

fn preset_from_id(id: &str) -> Result<BuiltinPreset, BindingError> {
    BuiltinPreset::from_id(id)
        .ok_or_else(|| BindingError::invalid_argument(format!("unknown theme.preset `{id}`")))
}

pub(crate) fn compile_error(field: &str, error: ThemeError) -> BindingError {
    match error.path().filter(|path| !matches!(*path, "." | "")) {
        Some(path) => BindingError::invalid_argument(format!("invalid {field}.{path}: {error}")),
        None => BindingError::invalid_argument(format!("invalid {field}: {error}")),
    }
}

/// Exports a built-in preset as its editable v2 theme document.
pub fn export_theme_preset_json(preset_id: &[u8]) -> Result<Vec<u8>, BindingError> {
    let preset_id = std::str::from_utf8(preset_id)
        .map_err(|_| BindingError::invalid_argument("theme preset id must be valid UTF-8"))?;
    Ok(preset_from_id(preset_id)?.json().as_bytes().to_vec())
}

pub(crate) fn validate_operation_options(
    operation: &str,
    options_json: &[u8],
) -> Result<(), BindingError> {
    if options_json.is_empty() {
        return Ok(());
    }
    crate::common::enforce_options_json_byte_budget(options_json)?;
    let options: serde_json::Map<String, Value> =
        serde_json::from_slice(options_json).map_err(|error| {
            BindingError::invalid_options_json(format!("invalid {operation} options: {error}"))
        })?;
    if options.is_empty() {
        Ok(())
    } else {
        Err(BindingError::invalid_options_json(format!(
            "{operation} does not accept options_json fields"
        )))
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveOptions {
    #[serde(default, deserialize_with = "present")]
    appearance: Option<Appearance>,
    #[serde(default)]
    auto_fix_contrast: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveRequest<'a> {
    #[serde(borrow)]
    document: &'a RawValue,
    #[serde(default, deserialize_with = "present")]
    appearance: Option<Appearance>,
    #[serde(default)]
    auto_fix_contrast: bool,
}

/// Resolves a `resolve-theme` request with an inline document or its original JSON text.
pub fn resolve_theme_request_json(request_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    if request_json.len() > RESOLVE_REQUEST_MAX_BYTES {
        return Err(BindingError::invalid_argument(format!(
            "resolve-theme request exceeds the {RESOLVE_REQUEST_MAX_BYTES} byte limit"
        )));
    }
    if !is_object(request_json) {
        return Err(BindingError::invalid_argument(
            "resolve-theme request must be a JSON object",
        ));
    }
    let request: ResolveRequest<'_> = serde_json::from_slice(request_json).map_err(|error| {
        BindingError::invalid_argument(format!("invalid resolve-theme request: {error}"))
    })?;
    let document_json = request.document.get();
    let document = if document_json.starts_with('"') {
        let text = serde_json::from_str::<String>(document_json).map_err(|error| {
            BindingError::invalid_argument(format!(
                "invalid resolve-theme document string: {error}"
            ))
        })?;
        Cow::Owned(text)
    } else {
        Cow::Borrowed(document_json)
    };
    resolve_document(
        document.as_bytes(),
        ResolveOptions {
            appearance: request.appearance,
            auto_fix_contrast: request.auto_fix_contrast,
        },
    )
}

/// Resolves a v2 theme document for inspection and returns its settings and contrast notices.
pub fn resolve_theme_json(document: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    crate::common::enforce_options_json_byte_budget(options_json)?;
    let options = if options_json.is_empty() {
        ResolveOptions::default()
    } else {
        if !is_object(options_json) {
            return Err(BindingError::invalid_options_json(
                "resolve-theme options must be a JSON object",
            ));
        }
        serde_json::from_slice(options_json).map_err(|error| {
            BindingError::invalid_options_json(format!("invalid resolve-theme options: {error}"))
        })?
    };
    resolve_document(document, options)
}

fn is_object(json: &[u8]) -> bool {
    json.iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b'{')
}

fn resolve_document(document: &[u8], options: ResolveOptions) -> Result<Vec<u8>, BindingError> {
    let compiler = ThemeCompiler::new().with_auto_fix_contrast(options.auto_fix_contrast);
    match compiler.compile_json(document, options.appearance) {
        Ok(resolved) => {
            #[derive(Serialize)]
            struct Resolution<'a> {
                ok: bool,
                resolved: &'a merman::theme::ResolvedThemeV2,
                notices: &'a [merman::theme::ThemeNotice],
            }
            serde_json::to_vec(&Resolution {
                ok: true,
                resolved: &resolved,
                notices: resolved.notices(),
            })
        }
        Err(error) => {
            #[derive(Serialize)]
            struct ResolutionError<'a> {
                ok: bool,
                path: Option<&'a str>,
                line: Option<usize>,
                column: Option<usize>,
                message: String,
            }
            serde_json::to_vec(&ResolutionError {
                ok: false,
                path: error.path(),
                line: error.line(),
                column: error.column(),
                message: error.to_string(),
            })
        }
    }
    .map_err(internal_json_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_bounds_the_envelope_before_decoding() {
        let mut oversized = br#"{"document":{"schema_version":2,"extends":"studio"}}"#.to_vec();
        oversized.resize(RESOLVE_REQUEST_MAX_BYTES + 1, b' ');
        let error = resolve_theme_request_json(&oversized).unwrap_err();
        assert!(error.message().contains("resolve-theme request exceeds"));

        let document = r#"{\"schema_version\":2,\"extends\":\"studio\"}"#;
        let decoded_len = br#"{"schema_version":2,"extends":"studio"}"#.len();
        let padding = "\\u0020".repeat(ThemeCompiler::new().max_bytes() - decoded_len);
        let request = format!(r#"{{"document":"{document}{padding}"}}"#);
        let result: Value =
            serde_json::from_slice(&resolve_theme_request_json(request.as_bytes()).unwrap())
                .unwrap();
        assert_eq!(result["ok"], true);
    }

    #[test]
    fn raw_resolution_keeps_the_document_syntax_location() {
        let document = "{\n  \"schema_version\": 2,\n  \"extends\": }";
        let direct = resolve_theme_json(document.as_bytes(), b"").unwrap();
        let envelope = serde_json::to_vec(&serde_json::json!({ "document": document })).unwrap();
        assert_eq!(resolve_theme_request_json(&envelope).unwrap(), direct);
        let result: Value = serde_json::from_slice(&direct).unwrap();
        assert_eq!(result["ok"], false);
        assert_eq!(result["line"], 3);
        assert!(result["column"].as_u64().is_some_and(|column| column > 0));
    }

    #[test]
    fn options_pass_embedded_document_keys_through_to_the_compiler() {
        for options in [
            br#"{"theme":{"schema_version":2,"extends":"studio","extends":"paper"}}"#.as_slice(),
            br#"{"theme":{"document":{"schema_version":2,"extends":"studio","extends":"paper"}}}"#
                .as_slice(),
        ] {
            let error = crate::BindingEngine::new(options).err().unwrap();
            assert!(error.message().contains("duplicate key"), "{error:?}");
        }
    }
}
