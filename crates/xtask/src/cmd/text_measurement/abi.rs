//! Generation of the cross-platform host text-measurement ABI contract.

use crate::XtaskError;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const DESCRIPTOR_PATH: &str = "abi/merman-v2.json";
const ABI_SCHEMA_VERSION: u32 = 1;
const ABI_VERSION: u32 = 2;
const ABI_V2_OPERATION_COUNT: usize = 19;
const ABI_V2_RESULT_KIND_COUNT: usize = 4;

const GENERATED_OUTPUTS: &[(&str, ArtifactKind)] = &[
    (
        "crates/merman-render/src/generated/text_measurement_abi.rs",
        ArtifactKind::RenderRust,
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
        "crates/merman-wasm/src/generated/abi.rs",
        ArtifactKind::WasmRust,
    ),
    (
        "crates/merman-ffi/include/merman_text_measurement_abi.h",
        ArtifactKind::CHeader,
    ),
    (
        "platforms/apple/Sources/Merman/Generated/TextMeasurementAbi.swift",
        ArtifactKind::Swift,
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
        "platforms/flutter/lib/src/generated/text_measurement_abi.dart",
        ArtifactKind::Dart,
    ),
    (
        "platforms/web/src/generated/text-measurement-abi.ts",
        ArtifactKind::TypeScript,
    ),
    (
        "platforms/python/merman/src/merman/_abi.py",
        ArtifactKind::Python,
    ),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AbiDescriptor {
    schema_version: u32,
    abi_version: u32,
    text_measurement: TextMeasurementDescriptor,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TextMeasurementDescriptor {
    result_kinds: Vec<ResultKind>,
    operations: Vec<Operation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultKind {
    id: String,
    external_name: String,
    rust_variant: String,
    code: i32,
}

#[derive(Debug, Clone, Deserialize)]
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
    UniffiRust,
    FfiRust,
    WasmRust,
    CHeader,
    Swift,
    KotlinOperations,
    KotlinResultKinds,
    Dart,
    TypeScript,
    Python,
}

impl ArtifactKind {
    fn render(self, descriptor: &AbiDescriptor) -> String {
        match self {
            Self::RenderRust => render_render_rust(descriptor),
            Self::UniffiRust => render_uniffi_rust(descriptor),
            Self::FfiRust => render_ffi_rust(descriptor),
            Self::WasmRust => render_wasm_rust(descriptor),
            Self::CHeader => render_c_header(descriptor),
            Self::Swift => render_swift(descriptor),
            Self::KotlinOperations => render_kotlin_operations(descriptor),
            Self::KotlinResultKinds => render_kotlin_result_kinds(descriptor),
            Self::Dart => render_dart(descriptor),
            Self::TypeScript => render_typescript(descriptor),
            Self::Python => render_python(descriptor),
        }
    }
}

fn descriptor_error(message: impl Into<String>) -> XtaskError {
    XtaskError::TextMeasurementAbi(message.into())
}

fn read_descriptor(path: &Path) -> Result<AbiDescriptor, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let descriptor: AbiDescriptor = serde_json::from_str(&text).map_err(|error| {
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
        .map(|code| i32::try_from(code).expect("ABI descriptor count fits i32"))
        .collect::<BTreeSet<_>>();
    if codes == expected {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} codes must be the contiguous ABI range 0..{}; found {codes:?}",
            expected_count - 1
        )))
    }
}

fn validate_descriptor(descriptor: &AbiDescriptor) -> Result<(), XtaskError> {
    if descriptor.schema_version != ABI_SCHEMA_VERSION {
        return Err(descriptor_error(format!(
            "unsupported text-measurement descriptor schema {}; expected {ABI_SCHEMA_VERSION}",
            descriptor.schema_version
        )));
    }
    if descriptor.abi_version != ABI_VERSION {
        return Err(descriptor_error(format!(
            "text-measurement descriptor targets ABI {}; this branch must remain on ABI {ABI_VERSION}",
            descriptor.abi_version
        )));
    }
    if descriptor.text_measurement.operations.len() != ABI_V2_OPERATION_COUNT {
        return Err(descriptor_error(format!(
            "ABI {ABI_VERSION} requires exactly {ABI_V2_OPERATION_COUNT} text-measurement operations; found {}",
            descriptor.text_measurement.operations.len()
        )));
    }
    if descriptor.text_measurement.result_kinds.len() != ABI_V2_RESULT_KIND_COUNT {
        return Err(descriptor_error(format!(
            "ABI {ABI_VERSION} requires exactly {ABI_V2_RESULT_KIND_COUNT} text-measurement result kinds; found {}",
            descriptor.text_measurement.result_kinds.len()
        )));
    }

    for kind in &descriptor.text_measurement.result_kinds {
        validate_identifier(&kind.id, "result kind")?;
        validate_external_name(&kind.external_name, "result kind")?;
        validate_rust_variant(&kind.rust_variant, "result kind")?;
    }
    for operation in &descriptor.text_measurement.operations {
        validate_identifier(&operation.id, "operation")?;
        validate_external_name(&operation.external_name, "operation")?;
        validate_rust_variant(&operation.rust_variant, "operation")?;
    }
    validate_unique(
        descriptor
            .text_measurement
            .result_kinds
            .iter()
            .map(|kind| kind.id.as_str()),
        "result-kind id",
    )?;
    validate_unique(
        descriptor
            .text_measurement
            .result_kinds
            .iter()
            .map(|kind| kind.rust_variant.as_str()),
        "result-kind Rust variant",
    )?;
    validate_unique(
        descriptor
            .text_measurement
            .result_kinds
            .iter()
            .map(|kind| kind.external_name.as_str()),
        "result-kind external name",
    )?;
    validate_unique(
        descriptor
            .text_measurement
            .operations
            .iter()
            .map(|operation| operation.id.as_str()),
        "operation id",
    )?;
    validate_unique(
        descriptor
            .text_measurement
            .operations
            .iter()
            .map(|operation| operation.rust_variant.as_str()),
        "operation Rust variant",
    )?;
    validate_unique(
        descriptor
            .text_measurement
            .operations
            .iter()
            .map(|operation| operation.external_name.as_str()),
        "operation external name",
    )?;
    validate_contiguous_codes(
        descriptor
            .text_measurement
            .result_kinds
            .iter()
            .map(|kind| kind.code),
        ABI_V2_RESULT_KIND_COUNT,
        "result-kind",
    )?;
    validate_contiguous_codes(
        descriptor
            .text_measurement
            .operations
            .iter()
            .map(|operation| operation.code),
        ABI_V2_OPERATION_COUNT,
        "operation",
    )?;

    let result_kinds = descriptor
        .text_measurement
        .result_kinds
        .iter()
        .map(|kind| (kind.id.as_str(), kind))
        .collect::<BTreeMap<_, _>>();
    for operation in &descriptor.text_measurement.operations {
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
    Ok(())
}

fn sorted_result_kinds(descriptor: &AbiDescriptor) -> Vec<&ResultKind> {
    let mut values = descriptor
        .text_measurement
        .result_kinds
        .iter()
        .collect::<Vec<_>>();
    values.sort_by_key(|kind| kind.code);
    values
}

fn sorted_operations(descriptor: &AbiDescriptor) -> Vec<&Operation> {
    let mut values = descriptor
        .text_measurement
        .operations
        .iter()
        .collect::<Vec<_>>();
    values.sort_by_key(|operation| operation.code);
    values
}

fn result_kind<'a>(descriptor: &'a AbiDescriptor, id: &str) -> &'a ResultKind {
    descriptor
        .text_measurement
        .result_kinds
        .iter()
        .find(|kind| kind.id == id)
        .expect("validated result-kind reference")
}

fn upper_snake(id: &str) -> String {
    id.to_ascii_uppercase()
}

fn rust_lower_camel(variant: &str) -> String {
    if let Some(suffix) = variant.strip_prefix("BBox") {
        return format!("bbox{suffix}");
    }
    let mut chars = variant.chars();
    let mut output = String::new();
    if let Some(first) = chars.next() {
        output.push(first.to_ascii_lowercase());
        output.extend(chars);
    }
    output
}

fn generated_preamble(comment: &str) -> String {
    format!(
        "{comment} This file is @generated by `cargo run -p xtask -- gen-text-measurement-abi`.\n{comment} Do not edit it directly; edit `{DESCRIPTOR_PATH}` instead.\n\n"
    )
}

fn render_render_rust(descriptor: &AbiDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "pub const TEXT_MEASUREMENT_ABI_VERSION: u32 = {};\n",
        descriptor.abi_version
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

fn render_uniffi_rust(descriptor: &AbiDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "pub const MERMAN_UNIFFI_ABI_VERSION: u32 = {};\n",
        descriptor.abi_version
    )
    .unwrap();
    writeln!(
        output,
        "#[doc(hidden)]\npub const MERMAN_UNIFFI_PYTHON_ABI_MODULE: &str = {:?};\n",
        render_python(descriptor)
    )
    .unwrap();
    output.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]\npub enum MermanTextMeasurementOperation {\n");
    for operation in &operations {
        writeln!(output, "    {},", operation.rust_variant).unwrap();
    }
    output.push_str("}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]\npub enum MermanTextMeasurementResultKind {\n");
    for kind in &kinds {
        writeln!(output, "    {},", kind.rust_variant).unwrap();
    }
    output.push_str("}\n\n#[cfg(feature = \"render\")]\nfn uniffi_measurement_operation(\n    operation: merman_bindings_core::TextMeasurementOperation,\n) -> MermanTextMeasurementOperation {\n    match operation {\n");
    for operation in &operations {
        writeln!(
            output,
            "        merman_bindings_core::TextMeasurementOperation::{} => MermanTextMeasurementOperation::{},",
            operation.rust_variant, operation.rust_variant
        )
        .unwrap();
    }
    output.push_str("    }\n}\n\n#[cfg(feature = \"render\")]\nfn uniffi_result_kind(\n    kind: MermanTextMeasurementResultKind,\n) -> merman_bindings_core::HostTextMeasurementResultKind {\n    match kind {\n");
    for kind in &kinds {
        writeln!(
            output,
            "        MermanTextMeasurementResultKind::{} => merman_bindings_core::HostTextMeasurementResultKind::{},",
            kind.rust_variant, kind.rust_variant
        )
        .unwrap();
    }
    output.push_str("    }\n}\n");
    output
}

fn render_ffi_rust(descriptor: &AbiDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "pub const MERMAN_ABI_VERSION: u32 = {};\n",
        descriptor.abi_version
    )
    .unwrap();
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

fn render_wasm_rust(descriptor: &AbiDescriptor) -> String {
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "const WASM_ABI_VERSION: u32 = {};",
        descriptor.abi_version
    )
    .unwrap();
    output
}

fn render_c_header(descriptor: &AbiDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    output.push_str(
        "#ifndef MERMAN_TEXT_MEASUREMENT_ABI_H\n#define MERMAN_TEXT_MEASUREMENT_ABI_H\n\n",
    );
    writeln!(
        output,
        "#define MERMAN_ABI_VERSION {}\n",
        descriptor.abi_version
    )
    .unwrap();
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

fn render_swift(descriptor: &AbiDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    output.push_str("import MermanFFI\n\n");
    writeln!(
        output,
        "let mermanGeneratedAbiVersion: UInt32 = {}\n",
        descriptor.abi_version
    )
    .unwrap();
    output.push_str("/// Shape required by a handled host text-measurement result.\npublic enum MermanTextMeasurementResultKind: Int32, CaseIterable, Sendable {\n");
    for kind in &kinds {
        writeln!(
            output,
            "    case {} = {}",
            rust_lower_camel(&kind.rust_variant),
            kind.code
        )
        .unwrap();
    }
    output.push_str("\n    public var externalName: String {\n        switch self {\n");
    for kind in &kinds {
        writeln!(
            output,
            "        case .{}: {:?}",
            rust_lower_camel(&kind.rust_variant),
            kind.external_name
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n\n    var cAbiRawValue: Int {\n        switch self {\n");
    for kind in &kinds {
        writeln!(
            output,
            "        case .{}: MERMAN_TEXT_MEASUREMENT_RESULT_KIND_{}",
            rust_lower_camel(&kind.rust_variant),
            upper_snake(&kind.id)
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n}\n\n/// Stable C ABI operation code for one host text-measurement primitive.\npublic enum MermanTextMeasurementOperation: Int32, CaseIterable, Sendable {\n");
    for operation in &operations {
        writeln!(
            output,
            "    case {} = {}",
            rust_lower_camel(&operation.rust_variant),
            operation.code
        )
        .unwrap();
    }
    output.push_str("\n    public var externalName: String {\n        switch self {\n");
    for operation in &operations {
        writeln!(
            output,
            "        case .{}: {:?}",
            rust_lower_camel(&operation.rust_variant),
            operation.external_name
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n\n    /// Result shape accepted by this operation.\n    public var requiredResultKind: MermanTextMeasurementResultKind {\n        switch self {\n");
    for operation in &operations {
        writeln!(
            output,
            "        case .{}: .{}",
            rust_lower_camel(&operation.rust_variant),
            rust_lower_camel(&result_kind(descriptor, &operation.result_kind).rust_variant)
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n\n    /// Whether this operation accepts a finite negative length.\n    public var acceptsSignedLength: Bool {\n        switch self {\n");
    for operation in &operations {
        writeln!(
            output,
            "        case .{}: {}",
            rust_lower_camel(&operation.rust_variant),
            operation.accepts_signed_length
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n\n    var cAbiRawValue: Int {\n        switch self {\n");
    for operation in &operations {
        writeln!(
            output,
            "        case .{}: MERMAN_TEXT_MEASUREMENT_OPERATION_{}",
            rust_lower_camel(&operation.rust_variant),
            upper_snake(&operation.id)
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n}\n");
    output
}

fn render_kotlin_operations(descriptor: &AbiDescriptor) -> String {
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    output.push_str("package io.merman\n\nobject MermanTextMeasurementOperation {\n");
    writeln!(
        output,
        "    const val ABI_VERSION: Int = {}",
        descriptor.abi_version
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

fn render_kotlin_result_kinds(descriptor: &AbiDescriptor) -> String {
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

fn render_dart(descriptor: &AbiDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "const int mermanAbiVersion = {};\n",
        descriptor.abi_version
    )
    .unwrap();
    output.push_str("/// Shape of a handled host text-measurement result.\nenum MermanTextMeasurementResultKind {\n");
    for kind in &kinds {
        writeln!(
            output,
            "  {}({}, {:?}),",
            rust_lower_camel(&kind.rust_variant),
            kind.code,
            kind.external_name
        )
        .unwrap();
    }
    output.push_str("  ;\n\n  const MermanTextMeasurementResultKind(this.code, this.externalName);\n\n  final int code;\n  final String externalName;\n}\n\n/// Exact text-measurement primitive requested by the native renderer.\nenum MermanTextMeasurementOperation {\n");
    for operation in &operations {
        writeln!(
            output,
            "  {}(\n    {},\n    {:?},\n    MermanTextMeasurementResultKind.{},\n    {},\n  ),",
            rust_lower_camel(&operation.rust_variant),
            operation.code,
            operation.external_name,
            rust_lower_camel(&result_kind(descriptor, &operation.result_kind).rust_variant),
            operation.accepts_signed_length
        )
        .unwrap();
    }
    output.push_str("  ;\n\n  const MermanTextMeasurementOperation(\n    this.code,\n    this.externalName,\n    this.requiredResultKind,\n    this.acceptsSignedLength,\n  );\n\n  final int code;\n  final String externalName;\n  final MermanTextMeasurementResultKind requiredResultKind;\n  final bool acceptsSignedLength;\n\n  static MermanTextMeasurementOperation? fromCode(int code) {\n    for (final value in values) {\n      if (value.code == code) {\n        return value;\n      }\n    }\n    return null;\n  }\n}\n");
    output
}

fn render_typescript(descriptor: &AbiDescriptor) -> String {
    let kinds = sorted_result_kinds(descriptor);
    let operations = sorted_operations(descriptor);
    let mut output = generated_preamble("//");
    writeln!(
        output,
        "export const MERMAN_ABI_VERSION = {} as const;\n",
        descriptor.abi_version
    )
    .unwrap();
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

fn render_python(descriptor: &AbiDescriptor) -> String {
    let mut output = generated_preamble("#");
    writeln!(output, "ABI_VERSION = {}\n", descriptor.abi_version).unwrap();
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
        "class AbiVersionMismatch(RuntimeError):\n\
         \x20   \"\"\"Raised when a loaded native library does not implement the packaged ABI.\"\"\"\n\
         \n\
         \x20   def __init__(self, actual: int) -> None:\n\
         \x20       self.expected = ABI_VERSION\n\
         \x20       self.actual = actual\n\
         \x20       super().__init__(f\"expected ABI {self.expected}, got {actual}\")\n\
         \n\
         \n\
         def require_abi_version(actual: int) -> None:\n\
         \x20   \"\"\"Reject a native library whose ABI does not match this package.\"\"\"\n\
         \x20   if actual != ABI_VERSION:\n\
         \x20       raise AbiVersionMismatch(actual)\n",
    );
    output
}

fn generated_artifacts(descriptor: &AbiDescriptor) -> Vec<(PathBuf, String)> {
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

pub(crate) fn gen_text_measurement_abi(args: Vec<String>) -> Result<(), XtaskError> {
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

pub(crate) fn verify_text_measurement_abi_artifacts() -> Result<Option<String>, XtaskError> {
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
            "text-measurement ABI projections drifted: {}; regenerate with `cargo run -p xtask -- gen-text-measurement-abi`",
            drift.join(", ")
        )))
    }
}

pub(crate) fn verify_text_measurement_abi(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    match verify_text_measurement_abi_artifacts()? {
        Some(message) => Err(XtaskError::VerifyFailed(message)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed_descriptor() -> AbiDescriptor {
        read_descriptor(&crate::cmd::workspace_root().join(DESCRIPTOR_PATH))
            .expect("committed ABI descriptor")
    }

    #[test]
    fn descriptor_is_complete_unique_and_abi_v2_stable() {
        let descriptor = committed_descriptor();
        assert_eq!(descriptor.schema_version, 1);
        assert_eq!(descriptor.abi_version, 2);
        assert_eq!(descriptor.text_measurement.operations.len(), 19);
        assert_eq!(descriptor.text_measurement.result_kinds.len(), 4);
        assert_eq!(
            sorted_operations(&descriptor)
                .into_iter()
                .map(|operation| operation.code)
                .collect::<Vec<_>>(),
            (0..19).collect::<Vec<_>>()
        );
    }

    #[test]
    fn validator_rejects_duplicates_gaps_and_unknown_result_shapes() {
        let mut duplicate = committed_descriptor();
        duplicate.text_measurement.operations[1].id =
            duplicate.text_measurement.operations[0].id.clone();
        assert!(validate_descriptor(&duplicate).is_err());

        let mut duplicate_external_name = committed_descriptor();
        duplicate_external_name.text_measurement.operations[1].external_name =
            duplicate_external_name.text_measurement.operations[0]
                .external_name
                .clone();
        assert!(validate_descriptor(&duplicate_external_name).is_err());

        let mut gap = committed_descriptor();
        gap.text_measurement.operations[18].code = 19;
        assert!(validate_descriptor(&gap).is_err());

        let mut unknown = committed_descriptor();
        unknown.text_measurement.operations[0].result_kind = "missing".to_string();
        assert!(validate_descriptor(&unknown).is_err());

        let mut invalid_signed = committed_descriptor();
        invalid_signed.text_measurement.operations[0].accepts_signed_length = true;
        assert!(validate_descriptor(&invalid_signed).is_err());
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
                        | ArtifactKind::Swift
                        | ArtifactKind::KotlinOperations
                        | ArtifactKind::Dart
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
    }

    #[test]
    fn platform_identifier_projection_preserves_existing_bbox_casing() {
        let descriptor = committed_descriptor();
        let swift = ArtifactKind::Swift.render(&descriptor);
        let dart = ArtifactKind::Dart.render(&descriptor);
        for identifier in ["bboxX", "titleBBoxX", "createTextBBoxYOffset"] {
            assert!(swift.contains(identifier), "Swift omitted `{identifier}`");
            assert!(dart.contains(identifier), "Dart omitted `{identifier}`");
        }
        assert!(!swift.contains("titleBboxX"));
        assert!(!dart.contains("titleBboxX"));
    }

    #[test]
    fn committed_generated_artifacts_have_no_drift() {
        assert_eq!(verify_text_measurement_abi_artifacts().unwrap(), None);
    }
}
