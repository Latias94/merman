//! Generation of the cross-platform host text-measurement protocol contract.

use crate::XtaskError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const DESCRIPTOR_PATH: &str = "contracts/abi/text-measurement-v1.json";
const TEXT_MEASUREMENT_PROTOCOL_ID: &str = "merman-text-measurement";
const TEXT_MEASUREMENT_PROTOCOL_SCHEMA_VERSION: u32 = 1;
const TEXT_MEASUREMENT_PROTOCOL_VERSION: u32 = 1;
const TEXT_MEASUREMENT_OPERATION_COUNT: usize = 19;
const TEXT_MEASUREMENT_RESULT_KIND_COUNT: usize = 4;
const TEXT_MEASUREMENT_WRAP_MODE_COUNT: usize = 3;
const TEXT_MEASUREMENT_DIRECTION_COUNT: usize = 1;
const TEXT_MEASUREMENT_WHITE_SPACE_COUNT: usize = 3;
const TEXT_MEASUREMENT_PHASE_COUNT: usize = 4;
const TEXT_MEASUREMENT_V1_PROTOCOL_SHA256: &str =
    "86f824a5217b4eabe41e914e42e7ab046617b1c94ea085b398a63d8fa278098f";

const GENERATED_OUTPUTS: &[(&str, ArtifactKind)] = &[
    (
        "crates/merman-render/src/generated/text_measurement_abi.rs",
        ArtifactKind::RenderRust,
    ),
    (
        "crates/merman-bindings-core/src/generated/text_measurement_abi.rs",
        ArtifactKind::BindingsCoreRust,
    ),
    (
        "crates/merman-uniffi/src/generated/text_measurement_abi.rs",
        ArtifactKind::UniffiRust,
    ),
    (
        "crates/merman-ffi/src/generated/text_measurement_abi.rs",
        ArtifactKind::FfiRust,
    ),
    (
        "crates/merman-ffi/include/merman_text_measurement_abi.h",
        ArtifactKind::CHeader,
    ),
    (
        "platforms/android/src/main/kotlin/io/merman/MermanTextMeasurementOperation.kt",
        ArtifactKind::KotlinOperations,
    ),
    (
        "platforms/android/src/main/kotlin/io/merman/MermanTextMeasurementResultKind.kt",
        ArtifactKind::KotlinResultKinds,
    ),
    (
        "platforms/android/src/main/kotlin/io/merman/MermanTextMeasurementVocabulary.kt",
        ArtifactKind::KotlinVocabulary,
    ),
    (
        "platforms/flutter/lib/src/generated/text_measurement_protocol.dart",
        ArtifactKind::Dart,
    ),
    (
        "platforms/web/src/generated/text-measurement-abi.ts",
        ArtifactKind::TypeScript,
    ),
    (
        "platforms/python/merman/src/merman/_text_measurement_protocol.py",
        ArtifactKind::Python,
    ),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TextMeasurementProtocolDescriptor {
    schema_version: u32,
    protocol_id: String,
    protocol_version: u32,
    wrap_modes: Vec<VocabularyValue>,
    directions: Vec<VocabularyValue>,
    white_spaces: Vec<VocabularyValue>,
    phases: Vec<VocabularyValue>,
    result_kinds: Vec<ResultKind>,
    operations: Vec<Operation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VocabularyValue {
    id: String,
    external_name: String,
    rust_variant: String,
    code: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultKind {
    id: String,
    external_name: String,
    rust_variant: String,
    code: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    id: String,
    external_name: String,
    rust_variant: String,
    code: i32,
    result_kind: String,
    accepts_signed_length: bool,
}

#[derive(Debug, Clone, Copy)]
enum ArtifactKind {
    RenderRust,
    BindingsCoreRust,
    UniffiRust,
    FfiRust,
    CHeader,
    KotlinOperations,
    KotlinResultKinds,
    KotlinVocabulary,
    Dart,
    TypeScript,
    Python,
}

impl ArtifactKind {
    fn render(self, descriptor: &TextMeasurementProtocolDescriptor) -> String {
        match self {
            Self::RenderRust => render_render_rust(descriptor),
            Self::BindingsCoreRust => render_bindings_core_rust(descriptor),
            Self::UniffiRust => render_uniffi_rust(descriptor),
            Self::FfiRust => render_ffi_rust(descriptor),
            Self::CHeader => render_c_header(descriptor),
            Self::KotlinOperations => render_kotlin_operations(descriptor),
            Self::KotlinResultKinds => render_kotlin_result_kinds(descriptor),
            Self::KotlinVocabulary => render_kotlin_vocabulary(descriptor),
            Self::Dart => render_dart(descriptor),
            Self::TypeScript => render_typescript(descriptor),
            Self::Python => render_python(descriptor),
        }
    }
}

fn descriptor_error(message: impl Into<String>) -> XtaskError {
    XtaskError::TextMeasurementProtocol(message.into())
}

fn read_descriptor(path: &Path) -> Result<TextMeasurementProtocolDescriptor, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let descriptor: TextMeasurementProtocolDescriptor =
        serde_json::from_str(&text).map_err(|error| {
            descriptor_error(format!("failed to parse {}: {error}", path.display()))
        })?;
    validate_descriptor(&descriptor)?;
    Ok(descriptor)
}

fn validate_identifier(id: &str, context: &str) -> Result<(), XtaskError> {
    let valid = !id.is_empty()
        && id.bytes().enumerate().all(|(index, byte)| match byte {
            b'a'..=b'z' => true,
            b'0'..=b'9' => index > 0,
            b'_' => index > 0 && index + 1 < id.len(),
            _ => false,
        })
        && !id.contains("__");
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} id `{id}` must be lower_snake_case"
        )))
    }
}

fn validate_external_name(name: &str, context: &str) -> Result<(), XtaskError> {
    let valid = !name.is_empty()
        && name.bytes().enumerate().all(|(index, byte)| match byte {
            b'a'..=b'z' => true,
            b'0'..=b'9' => index > 0,
            b'-' => index > 0 && index + 1 < name.len(),
            _ => false,
        })
        && !name.contains("--");
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} external name `{name}` must be lower-kebab-case"
        )))
    }
}

fn validate_rust_variant(variant: &str, context: &str) -> Result<(), XtaskError> {
    let mut bytes = variant.bytes();
    let valid = bytes.next().is_some_and(|byte| byte.is_ascii_uppercase())
        && bytes.all(|byte| byte.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} Rust variant `{variant}` must be an ASCII PascalCase identifier"
        )))
    }
}

fn validate_unique<'a>(
    values: impl IntoIterator<Item = &'a str>,
    context: &str,
) -> Result<(), XtaskError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(descriptor_error(format!(
                "duplicate {context} value `{value}`"
            )));
        }
    }
    Ok(())
}

fn validate_contiguous_codes(
    values: impl IntoIterator<Item = i32>,
    expected_count: usize,
    context: &str,
) -> Result<(), XtaskError> {
    let codes = values.into_iter().collect::<BTreeSet<_>>();
    let expected = (0..expected_count)
        .map(|code| i32::try_from(code).expect("protocol descriptor count fits i32"))
        .collect::<BTreeSet<_>>();
    if codes == expected {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} codes must be the contiguous protocol range 0..{}; found {codes:?}",
            expected_count - 1
        )))
    }
}

fn validate_vocabulary(
    values: &[VocabularyValue],
    expected_count: usize,
    context: &str,
) -> Result<(), XtaskError> {
    if values.len() != expected_count {
        return Err(descriptor_error(format!(
            "text-measurement protocol {TEXT_MEASUREMENT_PROTOCOL_VERSION} requires exactly {expected_count} {context} values; found {}",
            values.len()
        )));
    }
    for value in values {
        validate_identifier(&value.id, context)?;
        validate_external_name(&value.external_name, context)?;
        validate_rust_variant(&value.rust_variant, context)?;
    }
    validate_unique(
        values.iter().map(|value| value.id.as_str()),
        &format!("{context} id"),
    )?;
    validate_unique(
        values.iter().map(|value| value.external_name.as_str()),
        &format!("{context} external name"),
    )?;
    validate_unique(
        values.iter().map(|value| value.rust_variant.as_str()),
        &format!("{context} Rust variant"),
    )?;
    validate_contiguous_codes(
        values.iter().map(|value| value.code),
        expected_count,
        context,
    )
}

fn protocol_sha256(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let bytes = serde_json::to_vec(descriptor).expect("text-measurement protocol is serializable");
    crate::util::sha256_hex(&bytes)
}

fn validate_descriptor(descriptor: &TextMeasurementProtocolDescriptor) -> Result<(), XtaskError> {
    if descriptor.schema_version != TEXT_MEASUREMENT_PROTOCOL_SCHEMA_VERSION {
        return Err(descriptor_error(format!(
            "unsupported text-measurement descriptor schema {}; expected {TEXT_MEASUREMENT_PROTOCOL_SCHEMA_VERSION}",
            descriptor.schema_version
        )));
    }
    if descriptor.protocol_id != TEXT_MEASUREMENT_PROTOCOL_ID {
        return Err(descriptor_error(format!(
            "text-measurement descriptor protocol id `{}`; expected `{TEXT_MEASUREMENT_PROTOCOL_ID}`",
            descriptor.protocol_id
        )));
    }
    if descriptor.protocol_version != TEXT_MEASUREMENT_PROTOCOL_VERSION {
        return Err(descriptor_error(format!(
            "text-measurement descriptor protocol version {}; expected {TEXT_MEASUREMENT_PROTOCOL_VERSION}",
            descriptor.protocol_version
        )));
    }
    if descriptor.operations.len() != TEXT_MEASUREMENT_OPERATION_COUNT {
        return Err(descriptor_error(format!(
            "text-measurement protocol {TEXT_MEASUREMENT_PROTOCOL_VERSION} requires exactly {TEXT_MEASUREMENT_OPERATION_COUNT} operations; found {}",
            descriptor.operations.len()
        )));
    }
    if descriptor.result_kinds.len() != TEXT_MEASUREMENT_RESULT_KIND_COUNT {
        return Err(descriptor_error(format!(
            "text-measurement protocol {TEXT_MEASUREMENT_PROTOCOL_VERSION} requires exactly {TEXT_MEASUREMENT_RESULT_KIND_COUNT} result kinds; found {}",
            descriptor.result_kinds.len()
        )));
    }
    validate_vocabulary(
        &descriptor.wrap_modes,
        TEXT_MEASUREMENT_WRAP_MODE_COUNT,
        "wrap-mode",
    )?;
    validate_vocabulary(
        &descriptor.directions,
        TEXT_MEASUREMENT_DIRECTION_COUNT,
        "direction",
    )?;
    validate_vocabulary(
        &descriptor.white_spaces,
        TEXT_MEASUREMENT_WHITE_SPACE_COUNT,
        "white-space",
    )?;
    validate_vocabulary(&descriptor.phases, TEXT_MEASUREMENT_PHASE_COUNT, "phase")?;
    for kind in &descriptor.result_kinds {
        validate_identifier(&kind.id, "result kind")?;
        validate_external_name(&kind.external_name, "result kind")?;
        validate_rust_variant(&kind.rust_variant, "result kind")?;
    }
    for operation in &descriptor.operations {
        validate_identifier(&operation.id, "operation")?;
        validate_external_name(&operation.external_name, "operation")?;
        validate_rust_variant(&operation.rust_variant, "operation")?;
    }
    validate_unique(
        descriptor.result_kinds.iter().map(|kind| kind.id.as_str()),
        "result-kind id",
    )?;
    validate_unique(
        descriptor
            .result_kinds
            .iter()
            .map(|kind| kind.rust_variant.as_str()),
        "result-kind Rust variant",
    )?;
    validate_unique(
        descriptor
            .result_kinds
            .iter()
            .map(|kind| kind.external_name.as_str()),
        "result-kind external name",
    )?;
    validate_unique(
        descriptor
            .operations
            .iter()
            .map(|operation| operation.id.as_str()),
        "operation id",
    )?;
    validate_unique(
        descriptor
            .operations
            .iter()
            .map(|operation| operation.rust_variant.as_str()),
        "operation Rust variant",
    )?;
    validate_unique(
        descriptor
            .operations
            .iter()
            .map(|operation| operation.external_name.as_str()),
        "operation external name",
    )?;
    validate_contiguous_codes(
        descriptor.result_kinds.iter().map(|kind| kind.code),
        TEXT_MEASUREMENT_RESULT_KIND_COUNT,
        "result-kind",
    )?;
    validate_contiguous_codes(
        descriptor.operations.iter().map(|operation| operation.code),
        TEXT_MEASUREMENT_OPERATION_COUNT,
        "operation",
    )?;

    let result_kinds = descriptor
        .result_kinds
        .iter()
        .map(|kind| (kind.id.as_str(), kind))
        .collect::<BTreeMap<_, _>>();
    for operation in &descriptor.operations {
        let Some(result_kind) = result_kinds.get(operation.result_kind.as_str()) else {
            return Err(descriptor_error(format!(
                "operation `{}` references unknown result kind `{}`",
                operation.id, operation.result_kind
            )));
        };
        if operation.accepts_signed_length && result_kind.id != "length" {
            return Err(descriptor_error(format!(
                "operation `{}` accepts signed lengths but requires non-length result kind `{}`",
                operation.id, result_kind.id
            )));
        }
    }
    let protocol_sha256 = protocol_sha256(descriptor);
    if protocol_sha256 != TEXT_MEASUREMENT_V1_PROTOCOL_SHA256 {
        return Err(descriptor_error(format!(
            "text-measurement protocol 1 contract is immutable; expected sha256:{TEXT_MEASUREMENT_V1_PROTOCOL_SHA256}, found sha256:{protocol_sha256}; add a new protocol version before changing an emitted value or meaning"
        )));
    }
    Ok(())
}

fn sorted_result_kinds(descriptor: &TextMeasurementProtocolDescriptor) -> Vec<&ResultKind> {
    let mut values = descriptor.result_kinds.iter().collect::<Vec<_>>();
    values.sort_by_key(|kind| kind.code);
    values
}

fn sorted_operations(descriptor: &TextMeasurementProtocolDescriptor) -> Vec<&Operation> {
    let mut values = descriptor.operations.iter().collect::<Vec<_>>();
    values.sort_by_key(|operation| operation.code);
    values
}

fn sorted_vocabulary(values: &[VocabularyValue]) -> Vec<&VocabularyValue> {
    let mut values = values.iter().collect::<Vec<_>>();
    values.sort_by_key(|value| value.code);
    values
}

fn result_kind<'a>(descriptor: &'a TextMeasurementProtocolDescriptor, id: &str) -> &'a ResultKind {
    descriptor
        .result_kinds
        .iter()
        .find(|kind| kind.id == id)
        .expect("validated result-kind reference")
}

fn upper_snake(id: &str) -> String {
    id.to_ascii_uppercase()
}

fn lower_camel(id: &str) -> String {
    let mut parts = id.split('_');
    let mut output = parts.next().unwrap_or_default().to_string();
    for part in parts {
        if part == "bbox" {
            output.push_str("BBox");
            continue;
        }
        let mut bytes = part.bytes();
        if let Some(first) = bytes.next() {
            output.push(char::from(first.to_ascii_uppercase()));
            output.extend(bytes.map(char::from));
        }
    }
    output
}

fn generated_preamble(comment: &str) -> String {
    format!(
        "{comment} This file is @generated by `cargo run -p xtask -- gen-text-measurement-protocol`.\n{comment} Do not edit it directly; edit `{DESCRIPTOR_PATH}` instead.\n\n"
    )
}

fn render_render_rust(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "pub const TEXT_MEASUREMENT_PROTOCOL_VERSION: u32 = {};\n",
        descriptor.protocol_version
    )
    .unwrap();
    output.push_str("#[repr(i32)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum TextMeasurementResultKind {\n");
    for kind in &kinds {
        writeln!(output, "    {} = {},", kind.rust_variant, kind.code).unwrap();
    }
    output.push_str("}\n\nimpl TextMeasurementResultKind {\n");
    writeln!(output, "    pub const ALL: [Self; {}] = [", kinds.len()).unwrap();
    for kind in &kinds {
        writeln!(output, "        Self::{},", kind.rust_variant).unwrap();
    }
    output.push_str("    ];\n\n    pub const fn external_code(self) -> i32 {\n        self as i32\n    }\n\n    pub const fn external_name(self) -> &'static str {\n        TEXT_MEASUREMENT_RESULT_KIND_NAMES[self as usize]\n    }\n\n    pub const fn from_external_code(code: i32) -> Option<Self> {\n        if code >= 0 && (code as usize) < Self::ALL.len() {\n            Some(Self::ALL[code as usize])\n        } else {\n            None\n        }\n    }\n\n    pub fn from_external_name(name: &str) -> Option<Self> {\n        Self::ALL.into_iter().find(|kind| kind.external_name() == name)\n    }\n\n    pub const fn expected_for_operation(operation: TextMeasurementOperation) -> Self {\n        operation.required_result_kind()\n    }\n}\n\n");
    writeln!(
        output,
        "const TEXT_MEASUREMENT_RESULT_KIND_NAMES: [&str; {}] = [",
        kinds.len()
    )
    .unwrap();
    for kind in &kinds {
        writeln!(output, "    {:?},", kind.external_name).unwrap();
    }
    output.push_str("];\n\n#[repr(i32)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum TextMeasurementOperation {\n");
    for operation in &operations {
        writeln!(
            output,
            "    {} = {},",
            operation.rust_variant, operation.code
        )
        .unwrap();
    }
    output.push_str("}\n\nimpl TextMeasurementOperation {\n");
    writeln!(
        output,
        "    pub const ALL: [Self; {}] = [",
        operations.len()
    )
    .unwrap();
    for operation in &operations {
        writeln!(output, "        Self::{},", operation.rust_variant).unwrap();
    }
    output.push_str("    ];\n\n    pub(crate) const fn index(self) -> usize {\n        self as usize\n    }\n\n    pub const fn external_code(self) -> i32 {\n        self as i32\n    }\n\n    pub const fn external_name(self) -> &'static str {\n        TEXT_MEASUREMENT_OPERATION_DESCRIPTORS[self as usize].external_name\n    }\n\n    pub const fn required_result_kind(self) -> TextMeasurementResultKind {\n        TEXT_MEASUREMENT_OPERATION_DESCRIPTORS[self as usize].result_kind\n    }\n\n    pub const fn accepts_signed_length(self) -> bool {\n        TEXT_MEASUREMENT_OPERATION_DESCRIPTORS[self as usize].accepts_signed_length\n    }\n\n    pub const fn from_external_code(code: i32) -> Option<Self> {\n        if code >= 0 && (code as usize) < Self::ALL.len() {\n            Some(Self::ALL[code as usize])\n        } else {\n            None\n        }\n    }\n\n    pub fn from_external_name(name: &str) -> Option<Self> {\n        Self::ALL.into_iter().find(|operation| operation.external_name() == name)\n    }\n}\n\nstruct TextMeasurementOperationDescriptor {\n    external_name: &'static str,\n    result_kind: TextMeasurementResultKind,\n    accepts_signed_length: bool,\n}\n\n");
    writeln!(
        output,
        "const TEXT_MEASUREMENT_OPERATION_DESCRIPTORS: [TextMeasurementOperationDescriptor; {}] = [",
        operations.len()
    )
    .unwrap();
    for operation in &operations {
        let kind = result_kind(descriptor, &operation.result_kind);
        writeln!(
            output,
            "    TextMeasurementOperationDescriptor {{ external_name: {:?}, result_kind: TextMeasurementResultKind::{}, accepts_signed_length: {} }},",
            operation.external_name,
            kind.rust_variant,
            operation.accepts_signed_length
        )
        .unwrap();
    }
    output.push_str("];\n");
    output
}

fn render_rust_code_enum(output: &mut String, enum_name: &str, values: &[VocabularyValue]) {
    let values = sorted_vocabulary(values);
    writeln!(
        output,
        "#[repr(i32)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum {enum_name} {{"
    )
    .unwrap();
    for value in &values {
        writeln!(output, "    {} = {},", value.rust_variant, value.code).unwrap();
    }
    writeln!(output, "}}\n\nimpl {enum_name} {{").unwrap();
    output.push_str(
        "    pub const fn external_code(self) -> i32 {\n        self as i32\n    }\n}\n\n",
    );
}

fn render_bindings_core_rust(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let mut output = generated_preamble("//");
    render_rust_code_enum(&mut output, "HostTextWrapModeCode", &descriptor.wrap_modes);
    render_rust_code_enum(&mut output, "HostTextDirectionCode", &descriptor.directions);
    render_rust_code_enum(
        &mut output,
        "HostTextWhiteSpaceCode",
        &descriptor.white_spaces,
    );
    render_rust_code_enum(
        &mut output,
        "HostTextMeasurementPhaseCode",
        &descriptor.phases,
    );
    output.pop();
    output
}

fn render_uniffi_rust(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "pub const MERMAN_UNIFFI_TEXT_MEASUREMENT_PROTOCOL_VERSION: u32 = {};\n",
        descriptor.protocol_version
    )
    .unwrap();
    writeln!(
        output,
        "#[doc(hidden)]\npub const MERMAN_UNIFFI_PYTHON_TEXT_MEASUREMENT_PROTOCOL_MODULE: &str = {:?};\n",
        render_python(descriptor)
    )
    .unwrap();
    for (enum_name, values) in [
        ("MermanTextWrapMode", descriptor.wrap_modes.as_slice()),
        ("MermanTextDirection", descriptor.directions.as_slice()),
        ("MermanTextWhiteSpace", descriptor.white_spaces.as_slice()),
        ("MermanTextMeasurementPhase", descriptor.phases.as_slice()),
    ] {
        writeln!(
            output,
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]\npub enum {enum_name} {{"
        )
        .unwrap();
        for value in sorted_vocabulary(values) {
            writeln!(output, "    {},", value.rust_variant).unwrap();
        }
        output.push_str("}\n\n");
    }
    output.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]\npub enum MermanTextMeasurementOperation {\n");
    for operation in &operations {
        writeln!(output, "    {},", operation.rust_variant).unwrap();
    }
    output.push_str("}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]\npub enum MermanTextMeasurementResultKind {\n");
    for kind in &kinds {
        writeln!(output, "    {},", kind.rust_variant).unwrap();
    }
    output.push_str("}\n\n#[cfg(feature = \"svg\")]\nfn uniffi_measurement_operation(\n    operation: merman_bindings_core::TextMeasurementOperation,\n) -> MermanTextMeasurementOperation {\n    match operation {\n");
    for operation in &operations {
        writeln!(
            output,
            "        merman_bindings_core::TextMeasurementOperation::{} => MermanTextMeasurementOperation::{},",
            operation.rust_variant, operation.rust_variant
        )
        .unwrap();
    }
    output.push_str("    }\n}\n\n#[cfg(feature = \"svg\")]\nfn uniffi_result_kind(\n    kind: MermanTextMeasurementResultKind,\n) -> merman_bindings_core::HostTextMeasurementResultKind {\n    match kind {\n");
    for kind in &kinds {
        writeln!(
            output,
            "        MermanTextMeasurementResultKind::{} => merman_bindings_core::HostTextMeasurementResultKind::{},",
            kind.rust_variant, kind.rust_variant
        )
        .unwrap();
    }
    output.push_str("    }\n}\n\n");
    output.push_str("#[cfg(feature = \"svg\")]\nfn uniffi_measurement_phase(\n    phase: merman_bindings_core::TextMeasurementPhase,\n) -> MermanTextMeasurementPhase {\n    match phase {\n");
    for value in sorted_vocabulary(&descriptor.phases) {
        writeln!(
            output,
            "        merman_bindings_core::TextMeasurementPhase::{} => MermanTextMeasurementPhase::{},",
            value.rust_variant, value.rust_variant
        )
        .unwrap();
    }
    output.push_str("    }\n}\n\n");
    output.push_str("#[cfg(feature = \"svg\")]\nfn uniffi_wrap_mode(\n    wrap_mode: merman_bindings_core::WrapMode,\n) -> MermanTextWrapMode {\n    match wrap_mode {\n");
    for value in sorted_vocabulary(&descriptor.wrap_modes) {
        writeln!(
            output,
            "        merman_bindings_core::WrapMode::{} => MermanTextWrapMode::{},",
            value.rust_variant, value.rust_variant
        )
        .unwrap();
    }
    output.push_str("    }\n}\n");
    output
}

fn render_ffi_rust(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "pub const MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION: u32 = {};\n",
        descriptor.protocol_version
    )
    .unwrap();
    for (prefix, values) in [
        ("MERMAN_TEXT_WRAP_MODE", descriptor.wrap_modes.as_slice()),
        ("MERMAN_TEXT_DIRECTION", descriptor.directions.as_slice()),
        (
            "MERMAN_TEXT_WHITE_SPACE",
            descriptor.white_spaces.as_slice(),
        ),
        (
            "MERMAN_TEXT_MEASUREMENT_PHASE",
            descriptor.phases.as_slice(),
        ),
    ] {
        for value in sorted_vocabulary(values) {
            writeln!(
                output,
                "pub const {prefix}_{}: i32 = {};",
                upper_snake(&value.id),
                value.code
            )
            .unwrap();
        }
        output.push('\n');
    }
    for operation in &operations {
        writeln!(
            output,
            "pub const MERMAN_TEXT_MEASUREMENT_OPERATION_{}: i32 = {};",
            upper_snake(&operation.id),
            operation.code
        )
        .unwrap();
    }
    writeln!(
        output,
        "\npub const MERMAN_TEXT_MEASUREMENT_OPERATIONS: [i32; {}] = [",
        operations.len()
    )
    .unwrap();
    for operation in &operations {
        writeln!(
            output,
            "    MERMAN_TEXT_MEASUREMENT_OPERATION_{},",
            upper_snake(&operation.id)
        )
        .unwrap();
    }
    output.push_str("];\n\n");
    for kind in &kinds {
        writeln!(
            output,
            "pub const MERMAN_TEXT_MEASUREMENT_RESULT_KIND_{}: i32 = {};",
            upper_snake(&kind.id),
            kind.code
        )
        .unwrap();
    }
    writeln!(
        output,
        "\npub const MERMAN_TEXT_MEASUREMENT_OPERATION_RESULT_KINDS: [i32; {}] = [",
        operations.len()
    )
    .unwrap();
    for operation in &operations {
        writeln!(
            output,
            "    MERMAN_TEXT_MEASUREMENT_RESULT_KIND_{},",
            upper_snake(&result_kind(descriptor, &operation.result_kind).id)
        )
        .unwrap();
    }
    output.push_str("];\n");
    output
}

fn render_c_header(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    output.push_str(
        "#ifndef MERMAN_TEXT_MEASUREMENT_ABI_H\n#define MERMAN_TEXT_MEASUREMENT_ABI_H\n\n",
    );
    writeln!(
        output,
        "#define MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION {}\n",
        descriptor.protocol_version
    )
    .unwrap();
    for (prefix, values) in [
        ("MERMAN_TEXT_WRAP_MODE", descriptor.wrap_modes.as_slice()),
        ("MERMAN_TEXT_DIRECTION", descriptor.directions.as_slice()),
        (
            "MERMAN_TEXT_WHITE_SPACE",
            descriptor.white_spaces.as_slice(),
        ),
        (
            "MERMAN_TEXT_MEASUREMENT_PHASE",
            descriptor.phases.as_slice(),
        ),
    ] {
        output.push_str("enum {\n");
        let values = sorted_vocabulary(values);
        for (index, value) in values.iter().enumerate() {
            let suffix = if index + 1 == values.len() { "" } else { "," };
            writeln!(
                output,
                "    {prefix}_{} = {}{}",
                upper_snake(&value.id),
                value.code,
                suffix
            )
            .unwrap();
        }
        output.push_str("};\n\n");
    }
    output.push_str("enum {\n");
    for (index, operation) in operations.iter().enumerate() {
        let suffix = if index + 1 == operations.len() {
            ""
        } else {
            ","
        };
        writeln!(
            output,
            "    MERMAN_TEXT_MEASUREMENT_OPERATION_{} = {}{}",
            upper_snake(&operation.id),
            operation.code,
            suffix
        )
        .unwrap();
    }
    output.push_str("};\n\nenum {\n");
    for (index, kind) in kinds.iter().enumerate() {
        let suffix = if index + 1 == kinds.len() { "" } else { "," };
        writeln!(
            output,
            "    MERMAN_TEXT_MEASUREMENT_RESULT_KIND_{} = {}{}",
            upper_snake(&kind.id),
            kind.code,
            suffix
        )
        .unwrap();
    }
    output.push_str("};\n\n#endif\n");
    output
}

fn render_kotlin_operations(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    output.push_str("package io.merman\n\nobject MermanTextMeasurementOperation {\n");
    writeln!(
        output,
        "    const val PROTOCOL_VERSION: Int = {}",
        descriptor.protocol_version
    )
    .unwrap();
    for operation in &operations {
        writeln!(
            output,
            "    const val {}: Int = {}",
            upper_snake(&operation.id),
            operation.code
        )
        .unwrap();
    }
    output.push_str("\n    val ALL: IntArray = intArrayOf(\n");
    for operation in &operations {
        writeln!(output, "        {},", upper_snake(&operation.id)).unwrap();
    }
    output.push_str("    )\n\n    fun externalName(code: Int): String? = when (code) {\n");
    for operation in &operations {
        writeln!(
            output,
            "        {} -> {:?}",
            upper_snake(&operation.id),
            operation.external_name
        )
        .unwrap();
    }
    output.push_str("        else -> null\n    }\n\n    fun requiredResultKind(code: Int): Int? = when (code) {\n");
    for operation in &operations {
        writeln!(
            output,
            "        {} -> MermanTextMeasurementResultKind.{}",
            upper_snake(&operation.id),
            upper_snake(&operation.result_kind)
        )
        .unwrap();
    }
    output.push_str("        else -> null\n    }\n\n    fun acceptsSignedLength(code: Int): Boolean = when (code) {\n");
    let signed = operations
        .iter()
        .filter(|operation| operation.accepts_signed_length)
        .collect::<Vec<_>>();
    if !signed.is_empty() {
        for operation in signed {
            writeln!(output, "        {} -> true", upper_snake(&operation.id)).unwrap();
        }
    }
    output.push_str("        else -> false\n    }\n}\n");
    output
}

fn render_kotlin_result_kinds(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let mut output = generated_preamble("//");
    output.push_str("package io.merman\n\nobject MermanTextMeasurementResultKind {\n");
    for kind in &kinds {
        writeln!(
            output,
            "    const val {}: Int = {}",
            upper_snake(&kind.id),
            kind.code
        )
        .unwrap();
    }
    output.push_str("\n    val ALL: IntArray = intArrayOf(\n");
    for kind in &kinds {
        writeln!(output, "        {},", upper_snake(&kind.id)).unwrap();
    }
    output.push_str("    )\n\n    fun externalName(code: Int): String? = when (code) {\n");
    for kind in &kinds {
        writeln!(
            output,
            "        {} -> {:?}",
            upper_snake(&kind.id),
            kind.external_name
        )
        .unwrap();
    }
    output.push_str("        else -> null\n    }\n}\n");
    output
}

fn render_kotlin_vocabulary(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let mut output = generated_preamble("//");
    output.push_str("package io.merman\n\n");
    for (enum_name, values) in [
        ("MermanTextWrapMode", descriptor.wrap_modes.as_slice()),
        ("MermanTextDirection", descriptor.directions.as_slice()),
        ("MermanTextWhiteSpace", descriptor.white_spaces.as_slice()),
        ("MermanTextMeasurementPhase", descriptor.phases.as_slice()),
    ] {
        writeln!(
            output,
            "enum class {enum_name}(val code: Int, val externalName: String) {{"
        )
        .unwrap();
        let values = sorted_vocabulary(values);
        for (index, value) in values.iter().enumerate() {
            let suffix = if index + 1 == values.len() { ";" } else { "," };
            writeln!(
                output,
                "    {}({}, {:?}){}",
                upper_snake(&value.id),
                value.code,
                value.external_name,
                suffix
            )
            .unwrap();
        }
        output.push_str(
            "\n    companion object {\n        @JvmStatic\n        fun fromCode(code: Int): ",
        );
        output.push_str(enum_name);
        output.push_str("? = entries.firstOrNull { it.code == code }\n    }\n}\n\n");
    }
    output.pop();
    output
}

fn render_dart_enum(output: &mut String, enum_name: &str, values: &[VocabularyValue]) {
    writeln!(output, "enum {enum_name} {{").unwrap();
    let values = sorted_vocabulary(values);
    for (index, value) in values.iter().enumerate() {
        let suffix = if index + 1 == values.len() { ";" } else { "," };
        writeln!(
            output,
            "  {}({}, {:?}){}",
            lower_camel(&value.id),
            value.code,
            value.external_name,
            suffix
        )
        .unwrap();
    }
    writeln!(
        output,
        "\n  const {enum_name}(this.code, this.externalName);\n\n  final int code;\n  final String externalName;\n\n  static {enum_name}? fromCode(int code) {{\n    for (final value in values) {{\n      if (value.code == code) return value;\n    }}\n    return null;\n  }}\n\n  static {enum_name} requireCode(int code) {{\n    final value = fromCode(code);\n    if (value == null) {{\n      throw ArgumentError.value(\n          code, 'code', 'unknown {enum_name} code');\n    }}\n    return value;\n  }}\n}}\n"
    )
    .unwrap();
}

fn render_dart(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let mut output = generated_preamble("//");
    render_dart_enum(&mut output, "MermanTextWrapMode", &descriptor.wrap_modes);
    render_dart_enum(&mut output, "MermanTextDirection", &descriptor.directions);
    render_dart_enum(
        &mut output,
        "MermanTextWhiteSpace",
        &descriptor.white_spaces,
    );
    render_dart_enum(
        &mut output,
        "MermanTextMeasurementPhase",
        &descriptor.phases,
    );

    output.push_str("enum MermanTextMeasurementOperation {\n");
    let operations = sorted_operations(descriptor);
    for (index, operation) in operations.iter().enumerate() {
        let suffix = if index + 1 == operations.len() {
            ";"
        } else {
            ","
        };
        writeln!(
            output,
            "  {}({}){}",
            lower_camel(&operation.id),
            operation.code,
            suffix
        )
        .unwrap();
    }
    output.push_str(
        "\n  const MermanTextMeasurementOperation(this.code);\n\n  final int code;\n\n  static MermanTextMeasurementOperation? fromCode(int code) {\n    for (final operation in values) {\n      if (operation.code == code) return operation;\n    }\n    return null;\n  }\n}\n\n",
    );

    output.push_str("enum MermanTextMeasurementResultKind {\n");
    let kinds = sorted_result_kinds(descriptor);
    for (index, kind) in kinds.iter().enumerate() {
        let suffix = if index + 1 == kinds.len() { ";" } else { "," };
        writeln!(
            output,
            "  {}({}){}",
            lower_camel(&kind.id),
            kind.code,
            suffix
        )
        .unwrap();
    }
    output.push_str(
        "\n  const MermanTextMeasurementResultKind(this.code);\n\n  final int code;\n}\n",
    );
    output
}

fn render_typescript(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "export const MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION = {} as const;\n",
        descriptor.protocol_version
    )
    .unwrap();
    for (constant_name, type_name, values) in [
        (
            "HOST_TEXT_WRAP_MODES",
            "HostTextWrapMode",
            descriptor.wrap_modes.as_slice(),
        ),
        (
            "HOST_TEXT_DIRECTIONS",
            "HostTextDirection",
            descriptor.directions.as_slice(),
        ),
        (
            "HOST_TEXT_WHITE_SPACES",
            "HostTextWhiteSpace",
            descriptor.white_spaces.as_slice(),
        ),
        (
            "HOST_TEXT_MEASUREMENT_PHASES",
            "HostTextMeasurementPhase",
            descriptor.phases.as_slice(),
        ),
    ] {
        writeln!(output, "export const {constant_name} = [").unwrap();
        for value in sorted_vocabulary(values) {
            writeln!(
                output,
                "  {{ code: {}, name: {:?} }},",
                value.code, value.external_name
            )
            .unwrap();
        }
        writeln!(
            output,
            "] as const;\n\nexport type {type_name} =\n  (typeof {constant_name})[number][\"name\"];\n"
        )
        .unwrap();
    }
    output.push_str("export const HOST_TEXT_MEASUREMENT_RESULT_KINDS = [\n");
    for kind in &kinds {
        writeln!(
            output,
            "  {{ code: {}, name: {:?} }},",
            kind.code, kind.external_name
        )
        .unwrap();
    }
    output.push_str("] as const;\n\nexport type HostTextMeasurementResultKind =\n  (typeof HOST_TEXT_MEASUREMENT_RESULT_KINDS)[number][\"name\"];\n\nexport const HOST_TEXT_MEASUREMENT_OPERATIONS = [\n");
    for operation in &operations {
        writeln!(
            output,
            "  {{ code: {}, name: {:?}, resultKind: {:?}, acceptsSignedLength: {} }},",
            operation.code,
            operation.external_name,
            result_kind(descriptor, &operation.result_kind).external_name,
            operation.accepts_signed_length
        )
        .unwrap();
    }
    output.push_str("] as const;\n\nexport type HostTextMeasurementOperation =\n  (typeof HOST_TEXT_MEASUREMENT_OPERATIONS)[number][\"name\"];\n");
    output
}

fn render_python(descriptor: &TextMeasurementProtocolDescriptor) -> String {
    let mut output = generated_preamble("#");
    writeln!(
        output,
        "TEXT_MEASUREMENT_PROTOCOL_VERSION = {}\n",
        descriptor.protocol_version
    )
    .unwrap();
    for (constant_name, values) in [
        (
            "TEXT_MEASUREMENT_WRAP_MODES",
            descriptor.wrap_modes.as_slice(),
        ),
        (
            "TEXT_MEASUREMENT_DIRECTIONS",
            descriptor.directions.as_slice(),
        ),
        (
            "TEXT_MEASUREMENT_WHITE_SPACES",
            descriptor.white_spaces.as_slice(),
        ),
        ("TEXT_MEASUREMENT_PHASES", descriptor.phases.as_slice()),
    ] {
        writeln!(output, "{constant_name} = (").unwrap();
        for value in sorted_vocabulary(values) {
            writeln!(output, "    ({}, {:?}),", value.code, value.external_name).unwrap();
        }
        output.push_str(")\n\n");
    }
    output.push_str("TEXT_MEASUREMENT_RESULT_KINDS = (\n");
    for kind in sorted_result_kinds(descriptor) {
        writeln!(output, "    ({}, {:?}),", kind.code, kind.external_name).unwrap();
    }
    output.push_str(")\n\nTEXT_MEASUREMENT_OPERATIONS = (\n");
    for operation in sorted_operations(descriptor) {
        let kind = result_kind(descriptor, &operation.result_kind);
        writeln!(
            output,
            "    ({}, {:?}, {:?}, {}),",
            operation.code,
            operation.external_name,
            kind.external_name,
            if operation.accepts_signed_length {
                "True"
            } else {
                "False"
            }
        )
        .unwrap();
    }
    output.push_str(")\n\n");
    output.push_str(
        "class TextMeasurementProtocolVersionMismatch(RuntimeError):\n\
         \x20   \"\"\"Raised when a host text-measurement protocol version is incompatible.\"\"\"\n\
         \n\
         \x20   def __init__(self, actual: int) -> None:\n\
         \x20       self.expected = TEXT_MEASUREMENT_PROTOCOL_VERSION\n\
         \x20       self.actual = actual\n\
         \x20       super().__init__(f\"expected text-measurement protocol {self.expected}, got {actual}\")\n\
         \n\
         \n\
         def require_text_measurement_protocol_version(actual: int) -> None:\n\
         \x20   \"\"\"Reject an incompatible host text-measurement protocol.\"\"\"\n\
         \x20   if actual != TEXT_MEASUREMENT_PROTOCOL_VERSION:\n\
         \x20       raise TextMeasurementProtocolVersionMismatch(actual)\n",
    );
    output
}

fn generated_artifacts(descriptor: &TextMeasurementProtocolDescriptor) -> Vec<(PathBuf, String)> {
    GENERATED_OUTPUTS
        .iter()
        .map(|(path, kind)| (PathBuf::from(path), kind.render(descriptor)))
        .collect()
}

fn write_generated_artifact(path: &Path, contents: &str) -> Result<(), XtaskError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, contents).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn gen_text_measurement_protocol(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    for (relative_path, contents) in generated_artifacts(&descriptor) {
        write_generated_artifact(&root.join(relative_path), &contents)?;
    }
    Ok(())
}

pub(crate) fn verify_text_measurement_protocol_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    let mut drift = Vec::new();
    for (relative_path, expected) in generated_artifacts(&descriptor) {
        let path = root.join(&relative_path);
        let actual = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        if actual.replace("\r\n", "\n") != expected {
            drift.push(relative_path.display().to_string());
        }
    }
    if drift.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "text-measurement protocol projections drifted: {}; regenerate with `cargo run -p xtask -- gen-text-measurement-protocol`",
            drift.join(", ")
        )))
    }
}

pub(crate) fn verify_text_measurement_protocol(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    match verify_text_measurement_protocol_artifacts()? {
        Some(message) => Err(XtaskError::VerifyFailed(message)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed_descriptor() -> TextMeasurementProtocolDescriptor {
        read_descriptor(&crate::cmd::workspace_root().join(DESCRIPTOR_PATH))
            .expect("committed text-measurement protocol descriptor")
    }

    fn assert_protocol_v1_contract_is_immutable(descriptor: &TextMeasurementProtocolDescriptor) {
        let error = validate_descriptor(descriptor).expect_err("protocol v1 mutation must fail");
        assert!(
            error
                .to_string()
                .contains("text-measurement protocol 1 contract is immutable"),
            "unexpected validation error: {error}"
        );
    }

    #[test]
    fn descriptor_is_complete_unique_and_protocol_v1_stable() {
        let descriptor = committed_descriptor();
        assert_eq!(descriptor.schema_version, 1);
        assert_eq!(descriptor.protocol_id, TEXT_MEASUREMENT_PROTOCOL_ID);
        assert_eq!(descriptor.protocol_version, 1);
        assert_eq!(descriptor.operations.len(), 19);
        assert_eq!(descriptor.result_kinds.len(), 4);
        assert_eq!(descriptor.wrap_modes.len(), 3);
        assert_eq!(descriptor.directions.len(), 1);
        assert_eq!(descriptor.white_spaces.len(), 3);
        assert_eq!(descriptor.phases.len(), 4);
        assert_eq!(
            protocol_sha256(&descriptor),
            TEXT_MEASUREMENT_V1_PROTOCOL_SHA256
        );
        assert_eq!(
            sorted_operations(&descriptor)
                .into_iter()
                .map(|operation| operation.code)
                .collect::<Vec<_>>(),
            (0..19).collect::<Vec<_>>()
        );
        assert_eq!(
            sorted_result_kinds(&descriptor)
                .into_iter()
                .map(|result_kind| result_kind.code)
                .collect::<Vec<_>>(),
            (0..4).collect::<Vec<_>>()
        );
    }

    #[test]
    fn validator_rejects_duplicates_gaps_and_unknown_result_shapes() {
        let mut wrong_protocol_id = committed_descriptor();
        wrong_protocol_id.protocol_id = "other".to_string();
        assert!(validate_descriptor(&wrong_protocol_id).is_err());

        let mut wrong_protocol_version = committed_descriptor();
        wrong_protocol_version.protocol_version = 2;
        assert!(validate_descriptor(&wrong_protocol_version).is_err());

        let mut duplicate = committed_descriptor();
        duplicate.operations[1].id = duplicate.operations[0].id.clone();
        assert!(validate_descriptor(&duplicate).is_err());

        let mut duplicate_external_name = committed_descriptor();
        duplicate_external_name.operations[1].external_name =
            duplicate_external_name.operations[0].external_name.clone();
        assert!(validate_descriptor(&duplicate_external_name).is_err());

        let mut gap = committed_descriptor();
        gap.operations[18].code = 19;
        assert!(validate_descriptor(&gap).is_err());

        let mut unknown = committed_descriptor();
        unknown.operations[0].result_kind = "missing".to_string();
        assert!(validate_descriptor(&unknown).is_err());

        let mut invalid_signed = committed_descriptor();
        invalid_signed.operations[0].accepts_signed_length = true;
        assert!(validate_descriptor(&invalid_signed).is_err());

        let mut changed_vocabulary = committed_descriptor();
        changed_vocabulary.directions[0].external_name = "ltr".to_string();
        assert!(validate_descriptor(&changed_vocabulary).is_err());

        let mut vocabulary_gap = committed_descriptor();
        vocabulary_gap.phases[3].code = 4;
        assert!(validate_descriptor(&vocabulary_gap).is_err());
    }

    #[test]
    fn validator_rejects_semantic_protocol_v1_mutations() {
        let mut swapped_operation_codes = committed_descriptor();
        let first_code = swapped_operation_codes.operations[0].code;
        swapped_operation_codes.operations[0].code = swapped_operation_codes.operations[1].code;
        swapped_operation_codes.operations[1].code = first_code;
        assert_protocol_v1_contract_is_immutable(&swapped_operation_codes);

        let mut changed_result_shape = committed_descriptor();
        changed_result_shape.operations[0].result_kind = "length".to_string();
        assert_protocol_v1_contract_is_immutable(&changed_result_shape);

        let mut changed_signed_length_semantics = committed_descriptor();
        changed_signed_length_semantics.operations[1].accepts_signed_length = true;
        assert_protocol_v1_contract_is_immutable(&changed_signed_length_semantics);

        let mut changed_result_kind_name = committed_descriptor();
        changed_result_kind_name.result_kinds[0].external_name = "metric-values".to_string();
        assert_protocol_v1_contract_is_immutable(&changed_result_kind_name);

        let mut swapped_result_kind_codes = committed_descriptor();
        let first_code = swapped_result_kind_codes.result_kinds[0].code;
        swapped_result_kind_codes.result_kinds[0].code =
            swapped_result_kind_codes.result_kinds[1].code;
        swapped_result_kind_codes.result_kinds[1].code = first_code;
        assert_protocol_v1_contract_is_immutable(&swapped_result_kind_codes);
    }

    #[test]
    fn every_platform_projection_is_derived_from_the_same_ordered_contract() {
        let descriptor = committed_descriptor();
        for (path, kind) in GENERATED_OUTPUTS {
            let rendered = kind.render(&descriptor);
            assert!(rendered.contains("@generated"), "{path}");
            for operation in sorted_operations(&descriptor) {
                let expected_name = &operation.external_name;
                if matches!(
                    kind,
                    ArtifactKind::RenderRust
                        | ArtifactKind::KotlinOperations
                        | ArtifactKind::TypeScript
                        | ArtifactKind::Python
                ) {
                    assert!(
                        rendered.contains(expected_name.as_str()),
                        "{path}: {expected_name}"
                    );
                }
            }
        }

        let ffi = ArtifactKind::FfiRust.render(&descriptor);
        assert!(ffi.contains("MERMAN_TEXT_MEASUREMENT_OPERATION_RESULT_KINDS"));
        assert!(ffi.contains("MERMAN_TEXT_WRAP_MODE_HTML_LIKE"));
        assert!(ffi.contains("MERMAN_TEXT_DIRECTION_AUTO"));
        assert!(ffi.contains("MERMAN_TEXT_WHITE_SPACE_BREAK_SPACES"));
        assert!(ffi.contains("MERMAN_TEXT_MEASUREMENT_PHASE_COMPUTED_LENGTH"));
        for operation in sorted_operations(&descriptor) {
            let expected = format!(
                "MERMAN_TEXT_MEASUREMENT_RESULT_KIND_{}",
                upper_snake(&result_kind(&descriptor, &operation.result_kind).id)
            );
            assert!(
                ffi.contains(&expected),
                "FFI projection omitted `{expected}`"
            );
        }

        let uniffi = ArtifactKind::UniffiRust.render(&descriptor);
        assert!(uniffi.contains("pub enum MermanTextDirection {\n    Auto,\n}"));
        assert!(!uniffi.contains("    Ltr,"));
        assert!(!uniffi.contains("    Rtl,"));
        assert!(!uniffi.contains("    PreWrap,"));

        let dart = ArtifactKind::Dart.render(&descriptor);
        assert!(dart.contains("enum MermanTextWrapMode"));
        assert!(dart.contains("enum MermanTextMeasurementPhase"));
        assert!(dart.contains("svgBBox(2,"));
        assert!(dart.contains("titleBBoxX(4)"));
    }

    #[test]
    fn text_measurement_projections_do_not_define_a_native_abi() {
        let descriptor = committed_descriptor();
        let ffi = ArtifactKind::FfiRust.render(&descriptor);
        let c_header = ArtifactKind::CHeader.render(&descriptor);
        let typescript = ArtifactKind::TypeScript.render(&descriptor);
        let python = ArtifactKind::Python.render(&descriptor);

        for projection in [&ffi, &c_header, &typescript, &python] {
            assert!(projection.contains("TEXT_MEASUREMENT_PROTOCOL_VERSION"));
            assert!(!projection.contains("MERMAN_ABI_VERSION"));
            assert!(!projection.contains("ABI_VERSION ="));
        }
        assert!(ffi.contains("MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION"));
        assert!(c_header.contains("MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION"));
        assert!(typescript.contains("MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION"));
        assert!(python.contains("require_text_measurement_protocol_version"));
        assert!(
            !GENERATED_OUTPUTS
                .iter()
                .any(|(path, _)| path.ends_with("merman-wasm/src/generated/abi.rs")),
            "the native WASM ABI must have its own descriptor and generator"
        );
    }

    #[test]
    fn committed_generated_artifacts_have_no_drift() {
        assert_eq!(verify_text_measurement_protocol_artifacts().unwrap(), None);
    }
}
