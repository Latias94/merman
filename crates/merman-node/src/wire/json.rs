//! Bounded JSON framing for Node requests and transport documents.
//!
//! Operation resolution and transport-control admission run between request framing and
//! decoding nested options.

use merman_bindings_core::{BindingError, BindingOperationRequest, DecodedOperationOptions};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{NodeDocumentLimits, node_wire_contract};

pub(super) const OPERATION_OPTIONS_JSON_LABEL: &str = "operation request options_json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeOperationRequest {
    operation_id: String,
    source: String,
    uri: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present_string")]
    pub(super) options_json: Option<String>,
    operation_control: Option<NodeOperationControlRequest>,
    // Omission preserves the result contract used by executeOperation. String helpers request
    // data before execution so metadata is neither projected nor serialized by the producer.
    #[serde(default = "include_metadata_by_default")]
    pub(super) include_metadata: bool,
}

impl NodeOperationRequest {
    pub(super) fn binding_request(&self) -> BindingOperationRequest<'_> {
        BindingOperationRequest::new(&self.operation_id, self.source.as_bytes())
            .with_optional_uri(self.uri.as_deref().map(str::as_bytes))
    }

    pub(super) fn timeout_ms(&self) -> Option<u32> {
        self.operation_control
            .as_ref()
            .map(|operation_control| operation_control.timeout_ms)
    }
}

fn include_metadata_by_default() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeOperationControlRequest {
    timeout_ms: u32,
}

pub(super) fn parse_operation_request_framing(
    request_json: &str,
) -> Result<NodeOperationRequest, BindingError> {
    let contract = node_wire_contract();
    let request = deserialize_bounded_json::<NodeOperationRequest>(
        request_json,
        "operation request",
        contract.documents.request,
    )
    .map_err(caller_options_error)?;

    if request.operation_id.is_empty() {
        return Err(BindingError::invalid_options_json(
            "invalid Node operation request: operation_id must be non-empty",
        ));
    }
    ensure_field(
        &request.operation_id,
        "operation request operation_id",
        contract.fields.operation_id_utf8_bytes,
    )
    .map_err(caller_options_error)?;
    ensure_field(
        &request.source,
        "operation request source",
        contract.fields.source_utf8_bytes,
    )
    .map_err(caller_options_error)?;
    if let Some(uri) = request.uri.as_deref() {
        ensure_field(uri, "operation request uri", contract.fields.uri_utf8_bytes)
            .map_err(caller_options_error)?;
    }
    Ok(request)
}

pub(super) fn decode_operation_request_options(
    options_json: Option<&str>,
) -> Result<Option<DecodedOperationOptions<'_>>, BindingError> {
    let Some(options_json) = options_json else {
        return Ok(None);
    };
    let contract = node_wire_contract();
    let label = OPERATION_OPTIONS_JSON_LABEL;
    ensure_field(options_json, label, contract.fields.options_json_utf8_bytes)
        .map_err(caller_options_error)?;
    validate_bounded_json_text(options_json, label, contract.documents.binding_options)
        .map_err(caller_options_error)?;
    DecodedOperationOptions::decode(options_json, |error| {
        caller_options_error(format!("{label} is not valid JSON: {error}"))
    })
    .map(Some)
}

pub(super) fn validate_binding_options(options_json: &str) -> Result<(), BindingError> {
    if options_json.is_empty() {
        return Ok(());
    }
    let contract = node_wire_contract();
    ensure_field(
        options_json,
        "binding options",
        contract.fields.options_json_utf8_bytes,
    )
    .map_err(caller_options_error)?;
    deserialize_bounded_json::<Value>(
        options_json,
        "binding options",
        contract.documents.binding_options,
    )
    .map_err(caller_options_error)?;
    Ok(())
}

fn deserialize_present_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

pub(super) fn deserialize_bounded_json<T>(
    source: &str,
    label: &str,
    limits: NodeDocumentLimits,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    validate_bounded_json_text(source, label, limits)?;
    serde_json::from_str(source).map_err(|error| format!("{label} is not valid JSON: {error}"))
}

pub(super) fn serialize_bounded_json(
    value: &impl Serialize,
    label: &str,
    limits: NodeDocumentLimits,
) -> Result<String, String> {
    let source = serde_json::to_string(value)
        .map_err(|error| format!("failed to serialize Node {label}: {error}"))?;
    validate_bounded_json_text(&source, label, limits)?;
    Ok(source)
}

pub(super) fn validate_bounded_json_text(
    source: &str,
    label: &str,
    limits: NodeDocumentLimits,
) -> Result<(), String> {
    validate_document_bytes(source.len(), label, limits.max_utf8_bytes)?;
    scan_bounded_json_text(source, label, limits)
}

pub(super) fn validate_document_bytes(
    bytes: usize,
    label: &str,
    max_utf8_bytes: usize,
) -> Result<(), String> {
    if bytes > max_utf8_bytes {
        return Err(format!(
            "{label} exceeds the {max_utf8_bytes}-byte wire limit"
        ));
    }
    Ok(())
}

pub(super) fn scan_bounded_json_text(
    source: &str,
    label: &str,
    limits: NodeDocumentLimits,
) -> Result<(), String> {
    let bytes = source.as_bytes();
    let mut stack = Vec::new();
    let mut depth = 0usize;
    let mut members = 0usize;
    let mut tokens = 0usize;
    let mut index = 0usize;

    'scan: while index < bytes.len() {
        match bytes[index] {
            b' ' | b'\t' | b'\n' | b'\r' => index += 1,
            b'"' => {
                let (end, string_bytes) = scan_json_string_token(source, index, label)?;
                if string_bytes > limits.max_string_utf8_bytes {
                    return Err(format!(
                        "{label} contains a string exceeding the {}-byte field limit",
                        limits.max_string_utf8_bytes
                    ));
                }
                tokens += 1;
                index = end;
            }
            opening @ (b'{' | b'[') => {
                stack.push(opening);
                depth += 1;
                tokens += 1;
                if depth > limits.max_depth {
                    return Err(format!(
                        "{label} exceeds the structural depth limit {}",
                        limits.max_depth
                    ));
                }
                index += 1;
            }
            closing @ (b'}' | b']') => {
                let expected = if closing == b'}' { b'{' } else { b'[' };
                if stack.last().copied() != Some(expected) {
                    break 'scan;
                }
                stack.pop();
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b':' => {
                members += 1;
                index += 1;
            }
            b'-' | b'0'..=b'9' => {
                let Some(end) = scan_json_number(bytes, index) else {
                    break 'scan;
                };
                index = end;
                tokens += 1;
            }
            b't' if bytes[index..].starts_with(b"true") => {
                tokens += 1;
                index += 4;
            }
            b'f' if bytes[index..].starts_with(b"false") => {
                tokens += 1;
                index += 5;
            }
            b'n' if bytes[index..].starts_with(b"null") => {
                tokens += 1;
                index += 4;
            }
            _ => index += 1,
        }

        if members > limits.max_members {
            return Err(format!(
                "{label} exceeds the member-work limit {}",
                limits.max_members
            ));
        }
        if tokens > limits.max_tokens {
            return Err(format!(
                "{label} exceeds the token-work limit {}",
                limits.max_tokens
            ));
        }
    }
    Ok(())
}

fn scan_json_string_token(
    source: &str,
    start: usize,
    label: &str,
) -> Result<(usize, usize), String> {
    let bytes = source.as_bytes();
    let mut decoded_bytes = 0usize;
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Ok((index + 1, decoded_bytes)),
            b'\\' => {
                let escaped = bytes
                    .get(index + 1)
                    .copied()
                    .ok_or_else(|| format!("{label} contains an unterminated JSON escape"))?;
                match escaped {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                        decoded_bytes += 1;
                        index += 2;
                        continue;
                    }
                    b'u' => {}
                    _ => return Err(format!("{label} contains an invalid JSON escape")),
                }
                let first = parse_json_unicode_escape(bytes, index, label)?;
                if (0xd800..=0xdbff).contains(&first) {
                    if bytes.get(index + 6) != Some(&b'\\') || bytes.get(index + 7) != Some(&b'u') {
                        return Err(format!(
                            "{label} contains an isolated JSON surrogate escape"
                        ));
                    }
                    let second = parse_json_unicode_escape(bytes, index + 6, label)?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(format!(
                            "{label} contains an isolated JSON surrogate escape"
                        ));
                    }
                    decoded_bytes += 4;
                    index += 12;
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(format!(
                        "{label} contains an isolated JSON surrogate escape"
                    ));
                } else {
                    decoded_bytes += char::from_u32(u32::from(first))
                        .expect("non-surrogate u16 is a Unicode scalar")
                        .len_utf8();
                    index += 6;
                }
            }
            byte if byte.is_ascii() => {
                decoded_bytes += 1;
                index += 1;
            }
            _ => {
                let character = source[index..]
                    .chars()
                    .next()
                    .expect("index is within a valid UTF-8 string");
                decoded_bytes += character.len_utf8();
                index += character.len_utf8();
            }
        }
    }
    Ok((source.len(), decoded_bytes))
}

fn parse_json_unicode_escape(bytes: &[u8], slash_index: usize, label: &str) -> Result<u16, String> {
    let digits = bytes
        .get(slash_index + 2..slash_index + 6)
        .ok_or_else(|| format!("{label} contains an incomplete JSON Unicode escape"))?;
    let mut value = 0u16;
    for digit in digits {
        let digit = hex_digit(*digit)
            .map_err(|_| format!("{label} contains an invalid JSON Unicode escape"))?;
        value = value
            .checked_mul(16)
            .and_then(|value| value.checked_add(u16::from(digit)))
            .ok_or_else(|| format!("{label} contains an invalid JSON Unicode escape"))?;
    }
    Ok(value)
}

fn hex_digit(digit: u8) -> Result<u8, String> {
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        b'A'..=b'F' => Ok(digit - b'A' + 10),
        _ => Err("invalid hexadecimal digit".to_owned()),
    }
}

fn scan_json_number(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    if bytes.get(index) == Some(&b'-') {
        index += 1;
    }
    match bytes.get(index).copied()? {
        b'0' => index += 1,
        b'1'..=b'9' => {
            index += 1;
            while bytes
                .get(index)
                .copied()
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                index += 1;
            }
        }
        _ => return None,
    }
    if bytes.get(index) == Some(&b'.')
        && bytes
            .get(index + 1)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
    {
        index += 2;
        while bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            index += 1;
        }
    }
    if matches!(bytes.get(index).copied(), Some(b'e' | b'E')) {
        let exponent = index;
        index += 1;
        if matches!(bytes.get(index).copied(), Some(b'+' | b'-')) {
            index += 1;
        }
        if !bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            return Some(exponent);
        }
        index += 1;
        while bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            index += 1;
        }
    }
    Some(index)
}

pub(super) fn ensure_field(value: &str, label: &str, max_utf8_bytes: usize) -> Result<(), String> {
    ensure_field_bytes(value.len(), label, max_utf8_bytes)
}

pub(crate) fn ensure_field_bytes(
    bytes: usize,
    label: &str,
    max_utf8_bytes: usize,
) -> Result<(), String> {
    if bytes > max_utf8_bytes {
        return Err(format!(
            "{label} exceeds the {max_utf8_bytes}-byte field limit"
        ));
    }
    Ok(())
}

pub(crate) fn caller_options_error(message: String) -> BindingError {
    BindingError::invalid_options_json(format!("invalid Node transport JSON: {message}"))
}

#[cfg(test)]
mod tests {
    use super::{
        BindingError, NodeDocumentLimits, NodeOperationRequest, decode_operation_request_options,
        deserialize_bounded_json, node_wire_contract, parse_operation_request_framing,
        scan_bounded_json_text, validate_binding_options, validate_bounded_json_text,
    };
    use merman_bindings_core::BindingStatus;

    fn parse_operation_request(request_json: &str) -> Result<NodeOperationRequest, BindingError> {
        let request = parse_operation_request_framing(request_json)?;
        decode_operation_request_options(request.options_json.as_deref())?;
        Ok(request)
    }

    #[test]
    fn bounded_scanner_accepts_exact_limits_and_rejects_plus_one() {
        let base = NodeDocumentLimits {
            max_utf8_bytes: 256,
            max_depth: 2,
            max_members: 2,
            max_tokens: 5,
            max_string_utf8_bytes: 4,
        };
        scan_bounded_json_text(r#"{"a":0,"b":0}"#, "test", base).unwrap();

        let bytes = NodeDocumentLimits {
            max_utf8_bytes: 2,
            max_depth: 1,
            max_members: 1,
            max_tokens: 1,
            max_string_utf8_bytes: 1,
        };
        validate_bounded_json_text("{}", "test", bytes).unwrap();
        assert!(validate_bounded_json_text("{} ", "test", bytes).is_err());

        assert!(
            scan_bounded_json_text(
                r#"{"a":0,"b":0,"c":0}"#,
                "test",
                NodeDocumentLimits {
                    max_members: 2,
                    max_tokens: 7,
                    ..base
                },
            )
            .is_err()
        );
        scan_bounded_json_text("[[0]]", "test", base).unwrap();
        assert!(scan_bounded_json_text("[[[0]]]", "test", base).is_err());
        scan_bounded_json_text(r#""💩""#, "test", base).unwrap();
        assert!(scan_bounded_json_text(r#""💩a""#, "test", base).is_err());
        scan_bounded_json_text(r#""\ud83d\udca9""#, "test", base).unwrap();
        assert!(scan_bounded_json_text(r#""\ud83d""#, "test", base).is_err());
    }

    #[test]
    fn bounded_json_matches_serde_finite_number_semantics() {
        let limits = node_wire_contract().documents.binding_options;
        assert!(
            deserialize_bounded_json::<serde_json::Value>(
                r#"{"value":1e308}"#,
                "finite number",
                limits,
            )
            .is_ok()
        );
        assert!(
            deserialize_bounded_json::<serde_json::Value>(
                r#"{"value":1e309}"#,
                "non-finite number",
                limits,
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_string_escapes_return_options_errors() {
        let options = r#"{"value":"\é"}"#;
        let error = validate_binding_options(options).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("invalid JSON escape"));

        let error =
            parse_operation_request_framing(r#"{"operation_id":"svg","source":"\é"}"#).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);

        let request =
            serde_json::json!({"operation_id": "svg", "source": "info", "options_json": options});
        let error = parse_operation_request(&request.to_string()).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);

        validate_binding_options(r#"{"value":"\"\\\/\b\f\n\r\t\u00e9"}"#).unwrap();
        assert!(validate_binding_options(r#"{"value":"\x"}"#).is_err());
    }

    #[test]
    fn request_preflight_rejects_field_and_nested_options_overruns() {
        let exact_operation_id = "x".repeat(node_wire_contract().fields.operation_id_utf8_bytes);
        let exact = serde_json::json!({
            "operation_id": exact_operation_id,
            "source": "",
            "uri": null
        });
        parse_operation_request(&serde_json::to_string(&exact).unwrap()).unwrap();

        let plus_one = serde_json::json!({
            "operation_id": "x".repeat(node_wire_contract().fields.operation_id_utf8_bytes + 1),
            "source": "",
            "uri": null
        });
        assert!(parse_operation_request(&serde_json::to_string(&plus_one).unwrap()).is_err());
        assert!(
            parse_operation_request(
                r#"{"operation_id":"svg","source":"x","uri":null,"operation_control":{"timeout_ms":-1}}"#,
            )
            .is_err()
        );
        assert!(
            parse_operation_request(
                r#"{"operation_id":"svg","source":"x","uri":null,"operation_control":{"timeout_ms":1,"future":true}}"#,
            )
            .is_err()
        );
        assert!(
            parse_operation_request(
                r#"{"operation_id":"svg","source":"x","uri":null,"options_json":"{"}"}"#,
            )
            .is_err()
        );
        assert!(
            parse_operation_request(
                r#"{"operation_id":"svg","source":"x","uri":null,"future":true}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn bounded_deserialization_checks_work_before_materializing_json() {
        let limits = NodeDocumentLimits {
            max_utf8_bytes: 1024,
            max_depth: 8,
            max_members: 2,
            max_tokens: 4,
            max_string_utf8_bytes: 32,
        };
        let dense = r#"{"a":0,"b":0,"c":0}"#;
        assert!(deserialize_bounded_json::<serde_json::Value>(dense, "dense", limits).is_err());
    }
}
