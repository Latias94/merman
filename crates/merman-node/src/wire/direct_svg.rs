//! Direct N-API SVG execution and its equivalent JSON wire limits.

use std::sync::LazyLock;

use merman_bindings_core::{
    BindingEngine, BindingError, BindingOperationRequest, OperationControl, OperationKey,
    OperationPhase,
};
use serde::Serialize;

use super::{
    OPERATION_OPTIONS_JSON_LABEL, SuccessEnvelope, binding_boundary, caller_options_error,
    decode_operation_request_options, ensure_field, ensure_field_bytes, node_wire_contract,
    producer_error, response_data_text, scan_bounded_json_text, validate_document_bytes,
};

#[derive(Clone, Copy)]
pub(crate) enum DirectSvgOptions<'a> {
    Json(Option<&'a str>),
    DiagramId(&'a str),
}

impl<'a> DirectSvgOptions<'a> {
    fn json(self) -> Option<&'a str> {
        match self {
            Self::Json(json) => json,
            Self::DiagramId(_) => None,
        }
    }
}

pub(crate) fn execute_svg_with_admitted_control(
    engine: &BindingEngine,
    source: &str,
    options: DirectSvgOptions<'_>,
    control: OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> Result<String, BindingError> {
    binding_boundary(|| {
        let fallback;
        let options = match options {
            DirectSvgOptions::DiagramId(id)
                if !direct_svg_id_fits(source, id, admitted_timeout_ms) =>
            {
                fallback =
                    prepare_direct_svg_id_fallback(source, id, &control, admitted_timeout_ms)?;
                DirectSvgOptions::Json(Some(fallback.as_str()))
            }
            options => {
                validate_direct_svg_framing(source, options.json(), admitted_timeout_ms)?;
                control
                    .checkpoint_at(OperationPhase::Admission)
                    .map_err(BindingError::cancelled)?;
                options
            }
        };
        let decoded = decode_operation_request_options(options.json())?;
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        let data = match options {
            DirectSvgOptions::Json(_) => {
                let request =
                    BindingOperationRequest::new(OperationKey::Svg.id(), source.as_bytes())
                        .with_control(control);
                engine.execute_data_with_decoded_options(request, decoded)?
            }
            DirectSvgOptions::DiagramId(id) => {
                engine.execute_svg_data_with_diagram_id(source.as_bytes(), id, control)?
            }
        };
        direct_svg_response(data)
    })
}

pub(crate) fn direct_svg_field_limits() -> (usize, usize) {
    let fields = node_wire_contract().fields;
    (fields.source_utf8_bytes, fields.options_json_utf8_bytes)
}

fn direct_svg_frame_bytes(has_options: bool, timeout_ms: Option<u32>) -> usize {
    const FRAME_BYTES: usize =
        r#"{"operation_id":"svg","source":"","uri":null,"include_metadata":false}"#.len();
    const OPTIONS_FRAME_BYTES: usize = r#","options_json":"""#.len();
    const TIMEOUT_FRAME_BYTES: usize = r#","operation_control":{"timeout_ms":}"#.len();

    FRAME_BYTES
        + usize::from(has_options) * OPTIONS_FRAME_BYTES
        + timeout_ms.map_or(0, |timeout| {
            TIMEOUT_FRAME_BYTES + timeout.checked_ilog10().unwrap_or(0) as usize + 1
        })
}

pub(super) fn direct_svg_id_fits(source: &str, id: &str, timeout_ms: Option<u32>) -> bool {
    const ID_OPTIONS_BYTES: usize = br#"{"svg":{"diagram_id":""}}"#.len();
    let contract = node_wire_contract();
    // The virtual option document expands by at most twice when escaped as a request field.
    // Its structure and field bytes are otherwise bounded by this compact JSON shape.
    let max_options_bytes = ID_OPTIONS_BYTES.saturating_add(id.len().saturating_mul(6));
    let max_request_bytes = direct_svg_frame_bytes(true, timeout_ms)
        .saturating_add(source.len().saturating_mul(6))
        .saturating_add(max_options_bytes.saturating_mul(2));
    max_options_bytes <= contract.fields.options_json_utf8_bytes
        && max_request_bytes <= contract.documents.request.max_utf8_bytes
}

pub(super) fn prepare_direct_svg_id_fallback(
    source: &str,
    id: &str,
    control: &OperationControl,
    timeout_ms: Option<u32>,
) -> Result<String, BindingError> {
    #[derive(Serialize)]
    struct Svg<'a> {
        diagram_id: &'a str,
    }
    #[derive(Serialize)]
    struct Options<'a> {
        svg: Svg<'a>,
    }

    let contract = node_wire_contract();
    validate_direct_svg_source(source, contract.fields.source_utf8_bytes)?;
    let options = Options {
        svg: Svg { diagram_id: id },
    };
    let mut count = CompactJsonByteCount::default();
    serde_json::to_writer(&mut count, &options).map_err(direct_svg_size_error)?;
    let frame_bytes = direct_svg_frame_bytes(true, timeout_ms)
        .saturating_add(count.json_bytes)
        .saturating_add(count.outer_escape_bytes);
    let max = contract.documents.request.max_utf8_bytes;
    if frame_bytes.saturating_add(source.len().saturating_mul(6)) > max {
        let mut source_bytes = JsonByteCount(0);
        serde_json::to_writer(&mut source_bytes, source).map_err(direct_svg_size_error)?;
        validate_document_bytes(
            frame_bytes.saturating_add(source_bytes.0 - 2),
            "operation request",
            max,
        )
        .map_err(caller_options_error)?;
    }
    control
        .checkpoint_at(OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    ensure_field_bytes(
        count.json_bytes,
        OPERATION_OPTIONS_JSON_LABEL,
        contract.fields.options_json_utf8_bytes,
    )
    .map_err(caller_options_error)?;
    serde_json::to_string(&options).map_err(|error| {
        BindingError::internal(format!("failed to serialize Node SVG options: {error}"))
    })
}

fn validate_direct_svg_source(source: &str, max_utf8_bytes: usize) -> Result<(), BindingError> {
    ensure_field(source, "operation request source", max_utf8_bytes).map_err(caller_options_error)
}

fn direct_svg_size_error(error: serde_json::Error) -> BindingError {
    BindingError::internal(format!("failed to size Node SVG request: {error}"))
}

pub(super) fn validate_direct_svg_framing(
    source: &str,
    options_json: Option<&str>,
    timeout_ms: Option<u32>,
) -> Result<(), BindingError> {
    let contract = node_wire_contract();
    validate_direct_svg_source(source, contract.fields.source_utf8_bytes)?;
    let frame_bytes = direct_svg_frame_bytes(options_json.is_some(), timeout_ms);
    let text_bytes = source
        .len()
        .saturating_add(options_json.map_or(0, str::len));
    let max = contract.documents.request.max_utf8_bytes;
    // Only the encoded string contents can expand. All fixed fields fit the structural limits.
    if frame_bytes.saturating_add(text_bytes.saturating_mul(6)) > max {
        let mut count = JsonByteCount(frame_bytes - 2 - options_json.map_or(0, |_| 2));
        serde_json::to_writer(&mut count, source)
            .and_then(|()| {
                options_json.map_or(Ok(()), |json| serde_json::to_writer(&mut count, json))
            })
            .map_err(direct_svg_size_error)?;
        validate_document_bytes(count.0, "operation request", max).map_err(caller_options_error)?;
    }
    Ok(())
}

pub(super) fn direct_svg_response(data: Vec<u8>) -> Result<String, BindingError> {
    let data = response_data_text(OperationKey::Svg, data)?;
    static EMPTY_ENVELOPE: LazyLock<(usize, Result<(), String>)> = LazyLock::new(|| {
        let envelope = serde_json::to_string(&SuccessEnvelope::new(OperationKey::Svg, "", None))
            .expect("the fixed SVG response envelope is serializable");
        let shape = scan_bounded_json_text(
            &envelope,
            "response",
            node_wire_contract().documents.response,
        );
        (envelope.len(), shape)
    });
    let (envelope_bytes, shape) = &*EMPTY_ENVELOPE;
    let limits = node_wire_contract().documents.response;
    // serde_json emits at most six bytes per UTF-8 byte (an ASCII control becomes \u00XX).
    // The raw data field already fits the decoded-string ceiling, and its escaped content
    // cannot change the fixed envelope shape.
    const MAX_JSON_BYTES_PER_INPUT_BYTE: usize = 6;
    if envelope_bytes.saturating_add(data.len().saturating_mul(MAX_JSON_BYTES_PER_INPUT_BYTE))
        > limits.max_utf8_bytes
    {
        let mut count = JsonByteCount(0);
        serde_json::to_writer(
            &mut count,
            &SuccessEnvelope::new(OperationKey::Svg, &data, None),
        )
        .map_err(|error| producer_error(format!("failed to serialize Node response: {error}")))?;
        validate_document_bytes(count.0, "response", limits.max_utf8_bytes)
            .map_err(producer_error)?;
    }
    shape
        .as_ref()
        .map_err(|reason| producer_error(reason.clone()))?;
    Ok(data)
}

struct JsonByteCount(usize);

impl std::io::Write for JsonByteCount {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct CompactJsonByteCount {
    json_bytes: usize,
    outer_escape_bytes: usize,
}

impl std::io::Write for CompactJsonByteCount {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let escapes = bytes
            .iter()
            .filter(|&&byte| matches!(byte, b'"' | b'\\'))
            .count();
        self.json_bytes = self.json_bytes.saturating_add(bytes.len());
        self.outer_escape_bytes = self.outer_escape_bytes.saturating_add(escapes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
