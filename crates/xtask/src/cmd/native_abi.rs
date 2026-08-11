//! Generation and verification for the native ABI 3 contract.
//!
//! `contracts/abi/merman-v3.json` owns native numeric discriminants, record layouts, function-table
//! entries, and ownership semantics. It deliberately references the capability descriptor for
//! semantic IDs rather than defining a second capability catalog.

use crate::XtaskError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const DESCRIPTOR_PATH: &str = "contracts/abi/merman-v3.json";
const CAPABILITY_DESCRIPTOR_PATH: &str = "capabilities/feature-surface-v1.json";
const PROTOCOL_ID: &str = "merman-native";
const ABI_VERSION: u32 = 3;
const SCHEMA_VERSION: u32 = 1;
const RESULT_SCHEMA_VERSION: u32 = 1;
const FFI_RUST_OUTPUT: &str = "crates/merman-ffi/src/generated/abi3.rs";
const FFI_HEADER_OUTPUT: &str = "crates/merman-ffi/include/merman.h";
const FLUTTER_OPERATION_OUTPUT: &str = "platforms/flutter/lib/src/generated/native_operations.dart";
const FLUTTER_OPERATION_FIXED_MEMBERS: &[&str] = &[
    "fromNativeCode",
    "fromOperationId",
    "hashCode",
    "knownValues",
    "nativeCode",
    "noSuchMethod",
    "operationId",
    "requiresUri",
    "runtimeType",
    "toString",
];

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct NativeAbiDescriptor {
    schema_version: u32,
    protocol_id: String,
    abi_version: u32,
    result_schema_version: u32,
    minimum_prefix: MinimumPrefixDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    minimum_semantics: Vec<SemanticRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    opaque_scalars: Vec<OpaqueScalarDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    caller_memory_rules: Vec<SemanticRule>,
    error_kinds: Vec<ErrorKindDescriptor>,
    entry_point: EntryPoint,
    status_codes: Vec<CodeDescriptor>,
    operation_codes: Vec<OperationCodeDescriptor>,
    callbacks: Vec<CallableDescriptor>,
    function_slots: Vec<CallableDescriptor>,
    records: Vec<RecordDescriptor>,
    ownership_rules: Vec<OwnershipRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MinimumPrefixDescriptor {
    status_code_count: usize,
    operation_code_count: usize,
    error_kind_count: usize,
    callback_count: usize,
    function_slot_count: usize,
    record_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SemanticRule {
    id: String,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OpaqueScalarDescriptor {
    id: String,
    c_name: Option<String>,
    rust_name: Option<String>,
    c_type: String,
    rust_type: String,
    invalid_value: u64,
    domain: TokenDomainDescriptor,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TokenDomainDescriptor {
    tag: u64,
    mask: u64,
    counter_shift: u32,
    maximum_counter: u64,
    maximum_value: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct EntryPoint {
    c_name: String,
    rust_name: String,
    calling_convention: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    return_c_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    return_rust_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    parameters: Vec<ParameterDescriptor>,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CodeDescriptor {
    id: String,
    c_name: String,
    code: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    error_kinds: Vec<String>,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ErrorKindDescriptor {
    id: String,
    c_name: String,
    json_name: String,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OperationCodeDescriptor {
    id: String,
    c_name: String,
    code: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    executable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    non_executable_failure: Option<OperationFailureDescriptor>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OperationFailureDescriptor {
    status_id: String,
    error_kind_id: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CallableDescriptor {
    id: String,
    c_name: String,
    rust_name: String,
    code: i32,
    return_c_type: String,
    return_rust_type: String,
    parameters: Vec<ParameterDescriptor>,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ParameterDescriptor {
    name: String,
    c_type: String,
    rust_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RecordDescriptor {
    id: String,
    c_name: String,
    rust_name: String,
    description: String,
    #[serde(default)]
    appends_function_slots: bool,
    fields: Vec<FieldDescriptor>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct FieldDescriptor {
    name: String,
    c_type: String,
    rust_type: String,
    ownership: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OwnershipRule {
    id: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct CapabilityDescriptor {
    binding_operations: Vec<CapabilityOperationReference>,
}

#[derive(Debug, Clone, Deserialize)]
struct CapabilityOperationReference {
    id: String,
    capability: Option<String>,
    media_type: String,
    requires_uri: bool,
    targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct ResolvedOperation {
    id: String,
    c_name: String,
    code: i32,
    executable: bool,
    non_executable_failure: Option<ResolvedOperationFailure>,
    binding_operation_id: Option<String>,
    capability_id: Option<String>,
    media_type: Option<String>,
    requires_uri: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ResolvedOperationFailure {
    status_id: String,
    status_code: i32,
    error_kind_id: String,
    error_kind_json_name: String,
}

struct PreparedNativeAbi {
    descriptor: NativeAbiDescriptor,
    operations: Vec<ResolvedOperation>,
    minimum_prefix_digest: String,
    full_descriptor_digest: String,
}

fn native_abi_error(message: impl Into<String>) -> XtaskError {
    XtaskError::NativeAbi(message.into())
}

fn descriptor_error(message: impl Into<String>) -> String {
    message.into()
}

fn read_descriptor(path: &Path) -> Result<NativeAbiDescriptor, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let descriptor = serde_json::from_str::<NativeAbiDescriptor>(&text).map_err(|error| {
        native_abi_error(format!(
            "{}: descriptor schema error: {error}",
            path.display()
        ))
    })?;
    validate_descriptor(&descriptor).map_err(native_abi_error)?;
    Ok(descriptor)
}

fn validate_identifier(value: &str, context: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| match byte {
            b'a'..=b'z' => true,
            b'0'..=b'9' => index > 0,
            b'_' => index > 0 && index + 1 < value.len(),
            _ => false,
        })
        && !value.contains("__");
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} `{value}` must be a lower_snake_case identifier"
        )))
    }
}

fn validate_kebab_identifier(value: &str, context: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| match byte {
            b'a'..=b'z' => true,
            b'0'..=b'9' => index > 0,
            b'-' => index > 0 && index + 1 < value.len(),
            _ => false,
        })
        && !value.contains("--");
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} `{value}` must be a lower-kebab-case identifier"
        )))
    }
}

fn is_ascii_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(byte) if byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

// Generated Rust and C compile tests own language grammar; this layer only rejects missing types.
fn validate_type_name(value: &str, language: &str, context: &str) -> Result<(), String> {
    if !value.trim().is_empty() {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} {language} type must not be empty"
        )))
    }
}

fn validate_unique<'a>(
    values: impl IntoIterator<Item = &'a str>,
    context: &str,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(descriptor_error(format!("duplicate {context} `{value}`")));
        }
    }
    Ok(())
}

fn validate_contiguous_codes(
    values: impl IntoIterator<Item = i32>,
    context: &str,
) -> Result<(), String> {
    let codes = values.into_iter().collect::<BTreeSet<_>>();
    let expected = (0..i32::try_from(codes.len()).expect("descriptor count fits i32"))
        .collect::<BTreeSet<_>>();
    if codes == expected {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} codes must be contiguous from zero; found {codes:?}"
        )))
    }
}

fn validate_codes_in_descriptor_order(
    values: impl IntoIterator<Item = i32>,
    context: &str,
) -> Result<(), String> {
    for (expected, actual) in values.into_iter().enumerate() {
        let expected = i32::try_from(expected).expect("descriptor count fits i32");
        if actual != expected {
            return Err(descriptor_error(format!(
                "{context} must appear in code order; descriptor index {expected} contains code {actual}"
            )));
        }
    }
    Ok(())
}

fn validate_code_descriptors(
    values: &[CodeDescriptor],
    id_context: &str,
    c_prefix: &str,
) -> Result<(), String> {
    if values.is_empty() {
        return Err(descriptor_error(format!("{id_context} must not be empty")));
    }
    validate_unique(values.iter().map(|value| value.id.as_str()), id_context)?;
    validate_unique(
        values.iter().map(|value| value.c_name.as_str()),
        &format!("{id_context} C names"),
    )?;
    validate_contiguous_codes(values.iter().map(|value| value.code), id_context)?;
    validate_codes_in_descriptor_order(values.iter().map(|value| value.code), id_context)?;
    for value in values {
        validate_identifier(&value.id, id_context)?;
        if !value.c_name.starts_with(c_prefix) || !is_ascii_identifier(&value.c_name) {
            return Err(descriptor_error(format!(
                "{id_context} `{}` must use C name prefix `{c_prefix}`",
                value.id
            )));
        }
        if value.description.trim().is_empty() {
            return Err(descriptor_error(format!(
                "{id_context} `{}` must have a description",
                value.id
            )));
        }
    }
    Ok(())
}

fn validate_callable(
    callable: &CallableDescriptor,
    context: &str,
    require_code: bool,
) -> Result<(), String> {
    validate_identifier(&callable.id, context)?;
    if !is_ascii_identifier(&callable.c_name) {
        return Err(descriptor_error(format!(
            "{context} `{}` has invalid C name `{}`",
            callable.id, callable.c_name
        )));
    }
    if !is_ascii_identifier(&callable.rust_name) {
        return Err(descriptor_error(format!(
            "{context} `{}` has invalid Rust name `{}`",
            callable.id, callable.rust_name
        )));
    }
    if require_code && callable.code < 0 {
        return Err(descriptor_error(format!(
            "{context} `{}` must have a non-negative code",
            callable.id
        )));
    }
    validate_type_name(
        &callable.return_c_type,
        "C",
        &format!("{context} `{}`", callable.id),
    )?;
    validate_type_name(
        &callable.return_rust_type,
        "Rust",
        &format!("{context} `{}`", callable.id),
    )?;
    validate_unique(
        callable
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str()),
        &format!("{context} `{}` parameters", callable.id),
    )?;
    for parameter in &callable.parameters {
        validate_identifier(
            &parameter.name,
            &format!("{context} `{}` parameter", callable.id),
        )?;
        validate_type_name(
            &parameter.c_type,
            "C",
            &format!("{context} `{}` parameter `{}`", callable.id, parameter.name),
        )?;
        validate_type_name(
            &parameter.rust_type,
            "Rust",
            &format!("{context} `{}` parameter `{}`", callable.id, parameter.name),
        )?;
    }
    if callable.description.trim().is_empty() {
        return Err(descriptor_error(format!(
            "{context} `{}` must have a description",
            callable.id
        )));
    }
    Ok(())
}

fn validate_opaque_scalars(descriptor: &NativeAbiDescriptor) -> Result<(), String> {
    validate_unique(
        descriptor
            .opaque_scalars
            .iter()
            .map(|scalar| scalar.id.as_str()),
        "native ABI opaque scalar ids",
    )?;
    if descriptor.opaque_scalars.len() != 2 {
        return Err(descriptor_error(
            "native ABI must define exactly the engine_token and result_allocation_token roles",
        ));
    }
    let engine = descriptor
        .opaque_scalars
        .iter()
        .find(|scalar| scalar.id == "engine_token")
        .ok_or_else(|| descriptor_error("native ABI must define the engine_token role"))?;
    let result = descriptor
        .opaque_scalars
        .iter()
        .find(|scalar| scalar.id == "result_allocation_token")
        .ok_or_else(|| {
            descriptor_error("native ABI must define the result_allocation_token role")
        })?;

    let mut tags = BTreeSet::new();
    for scalar in &descriptor.opaque_scalars {
        validate_identifier(&scalar.id, "native ABI opaque scalar")?;
        validate_type_name(
            &scalar.c_type,
            "C",
            &format!("native ABI opaque scalar `{}`", scalar.id),
        )?;
        validate_type_name(
            &scalar.rust_type,
            "Rust",
            &format!("native ABI opaque scalar `{}`", scalar.id),
        )?;
        if scalar.description.trim().is_empty() {
            return Err(descriptor_error(format!(
                "native ABI opaque scalar `{}` must have a description",
                scalar.id
            )));
        }
        let shift = scalar.domain.counter_shift;
        let expected_mask = 1_u64
            .checked_shl(shift)
            .and_then(|value| value.checked_sub(1))
            .ok_or_else(|| {
                descriptor_error(format!(
                    "native ABI opaque scalar `{}` has an invalid token counter shift",
                    scalar.id
                ))
            })?;
        let expected_counter_max = (i64::MAX as u64) >> shift;
        if scalar.invalid_value != 0
            || scalar.domain.mask != expected_mask
            || scalar.domain.maximum_counter != expected_counter_max
            || scalar.domain.tag == 0
            || scalar.domain.tag > scalar.domain.mask
            || !tags.insert(scalar.domain.tag)
        {
            return Err(descriptor_error(format!(
                "native ABI opaque scalar `{}` must use a unique nonzero low-bit domain tag and the sign-bit-preserving counter range",
                scalar.id
            )));
        }
        let maximum_value = scalar
            .domain
            .maximum_counter
            .checked_shl(scalar.domain.counter_shift)
            .and_then(|counter| counter.checked_add(scalar.domain.tag))
            .ok_or_else(|| {
                descriptor_error(format!(
                    "native ABI opaque scalar `{}` maximum token overflows",
                    scalar.id
                ))
            })?;
        if scalar.domain.maximum_value != maximum_value || maximum_value > i64::MAX as u64 {
            return Err(descriptor_error(format!(
                "native ABI opaque scalar `{}` maximum value must preserve the signed-64 sign bit",
                scalar.id
            )));
        }
    }

    if engine.domain.mask != result.domain.mask
        || engine.domain.counter_shift != result.domain.counter_shift
        || engine.domain.maximum_counter != result.domain.maximum_counter
    {
        return Err(descriptor_error(
            "native ABI opaque token roles must share one mask, shift, and counter range",
        ));
    }
    if engine.c_name.as_deref() != Some("MermanNativeEngineToken")
        || engine.rust_name.as_deref() != Some("MermanNativeEngineToken")
        || engine.c_type != "uint64_t"
        || engine.rust_type != "u64"
    {
        return Err(descriptor_error(
            "native ABI engine_token must project the MermanNativeEngineToken uint64_t/u64 alias",
        ));
    }
    if result.c_name.is_some()
        || result.rust_name.is_some()
        || result.c_type != "uint64_t"
        || result.rust_type != "u64"
    {
        return Err(descriptor_error(
            "native ABI result_allocation_token must retain its anonymous uint64_t/u64 record-field representation",
        ));
    }
    Ok(())
}

fn validate_operation_global_identifiers(
    operations: &[OperationCodeDescriptor],
) -> Result<(), String> {
    let mut identifiers = BTreeMap::from([(
        "MERMAN_NATIVE_OPERATION_DESCRIPTORS".to_string(),
        "generated Rust operation descriptor table".to_string(),
    )]);
    for operation in operations {
        let suffix = upper_snake(&operation.id);
        let mut emitted = vec![
            (operation.c_name.clone(), "operation code"),
            (
                format!("MERMAN_NATIVE_OPERATION_REQUIRES_URI_{suffix}"),
                "requires-URI flag",
            ),
            (
                format!("MERMAN_NATIVE_OPERATION_EXECUTABLE_{suffix}"),
                "executable flag",
            ),
        ];
        if operation.non_executable_failure.is_some() {
            emitted.extend([
                (
                    format!("MERMAN_NATIVE_OPERATION_NON_EXECUTABLE_STATUS_{suffix}"),
                    "non-executable status",
                ),
                (
                    format!("MERMAN_NATIVE_OPERATION_NON_EXECUTABLE_ERROR_KIND_{suffix}"),
                    "non-executable error kind",
                ),
            ]);
        }
        if operation.id != "none" {
            // Capability is optional today, but reserving every generated metadata name keeps a
            // later capability assignment from retroactively breaking the ABI projection.
            emitted.extend([
                (
                    format!("MERMAN_NATIVE_OPERATION_ID_{suffix}"),
                    "binding operation id",
                ),
                (
                    format!("MERMAN_NATIVE_OPERATION_CAPABILITY_{suffix}"),
                    "capability id",
                ),
                (
                    format!("MERMAN_NATIVE_OPERATION_MEDIA_TYPE_{suffix}"),
                    "media type",
                ),
            ]);
        }

        for (identifier, role) in emitted {
            let source = format!("operation `{}` {role}", operation.id);
            if let Some(previous) = identifiers.insert(identifier.clone(), source.clone()) {
                return Err(descriptor_error(format!(
                    "native ABI global identifier `{identifier}` is emitted by both {previous} and {source}"
                )));
            }
        }
    }
    Ok(())
}

fn validate_descriptor(descriptor: &NativeAbiDescriptor) -> Result<(), String> {
    if descriptor.schema_version != SCHEMA_VERSION {
        return Err(descriptor_error(format!(
            "unsupported native ABI descriptor schema {}; expected {SCHEMA_VERSION}",
            descriptor.schema_version
        )));
    }
    if descriptor.protocol_id != PROTOCOL_ID {
        return Err(descriptor_error(format!(
            "native ABI descriptor protocol id `{}`; expected `{PROTOCOL_ID}`",
            descriptor.protocol_id
        )));
    }
    if descriptor.abi_version != ABI_VERSION {
        return Err(descriptor_error(format!(
            "native ABI descriptor version {}; expected {ABI_VERSION}",
            descriptor.abi_version
        )));
    }
    if descriptor.result_schema_version != RESULT_SCHEMA_VERSION {
        return Err(descriptor_error(format!(
            "native ABI result schema version {}; expected {RESULT_SCHEMA_VERSION}",
            descriptor.result_schema_version
        )));
    }
    let minimum_prefix = &descriptor.minimum_prefix;
    for (name, count, available) in [
        (
            "status codes",
            minimum_prefix.status_code_count,
            descriptor.status_codes.len(),
        ),
        (
            "operation codes",
            minimum_prefix.operation_code_count,
            descriptor.operation_codes.len(),
        ),
        (
            "error kinds",
            minimum_prefix.error_kind_count,
            descriptor.error_kinds.len(),
        ),
        (
            "callbacks",
            minimum_prefix.callback_count,
            descriptor.callbacks.len(),
        ),
        (
            "function slots",
            minimum_prefix.function_slot_count,
            descriptor.function_slots.len(),
        ),
        (
            "records",
            minimum_prefix.record_count,
            descriptor.records.len(),
        ),
    ] {
        if count == 0 || count > available {
            return Err(descriptor_error(format!(
                "native ABI minimum prefix selects {count} {name}, but the descriptor defines {available}"
            )));
        }
    }
    validate_unique(
        descriptor
            .minimum_semantics
            .iter()
            .map(|semantic| semantic.id.as_str()),
        "native ABI minimum semantic ids",
    )?;
    for semantic in &descriptor.minimum_semantics {
        validate_kebab_identifier(&semantic.id, "native ABI minimum semantic id")?;
        if semantic.description.trim().is_empty() {
            return Err(descriptor_error(format!(
                "native ABI minimum semantic `{}` must have a description",
                semantic.id
            )));
        }
    }
    validate_opaque_scalars(descriptor)?;
    validate_unique(
        descriptor
            .caller_memory_rules
            .iter()
            .map(|rule| rule.id.as_str()),
        "native ABI caller-memory rule ids",
    )?;
    for rule in &descriptor.caller_memory_rules {
        validate_identifier(&rule.id, "native ABI caller-memory rule")?;
        if rule.description.trim().is_empty() {
            return Err(descriptor_error(format!(
                "native ABI caller-memory rule `{}` must have a description",
                rule.id
            )));
        }
    }
    if descriptor.error_kinds.len() != minimum_prefix.error_kind_count {
        return Err(descriptor_error(format!(
            "native ABI error-kind vocabulary is closed by the minimum prefix at {} entries, but the descriptor defines {}",
            minimum_prefix.error_kind_count,
            descriptor.error_kinds.len()
        )));
    }
    if descriptor.callbacks.len() != minimum_prefix.callback_count {
        return Err(descriptor_error(format!(
            "native ABI callback vocabulary is closed by the minimum prefix at {} entries, but the descriptor defines {}",
            minimum_prefix.callback_count,
            descriptor.callbacks.len()
        )));
    }
    validate_unique(
        descriptor.error_kinds.iter().map(|kind| kind.id.as_str()),
        "native ABI error kind ids",
    )?;
    validate_unique(
        descriptor
            .error_kinds
            .iter()
            .map(|kind| kind.c_name.as_str()),
        "native ABI error kind C names",
    )?;
    validate_unique(
        descriptor
            .error_kinds
            .iter()
            .map(|kind| kind.json_name.as_str()),
        "native ABI error kind JSON names",
    )?;
    for kind in &descriptor.error_kinds {
        validate_identifier(&kind.id, "native ABI error kind")?;
        validate_kebab_identifier(&kind.json_name, "native ABI error kind JSON name")?;
        if !kind.c_name.starts_with("MERMAN_NATIVE_ERROR_KIND_")
            || !is_ascii_identifier(&kind.c_name)
        {
            return Err(descriptor_error(format!(
                "native ABI error kind `{}` must use the MERMAN_NATIVE_ERROR_KIND_ C name prefix",
                kind.id
            )));
        }
        if kind.description.trim().is_empty() {
            return Err(descriptor_error(format!(
                "native ABI error kind `{}` must have a description",
                kind.id
            )));
        }
    }
    if descriptor.entry_point.c_name != "merman_get_native_api"
        || descriptor.entry_point.rust_name != "merman_get_native_api"
        || descriptor.entry_point.calling_convention != "C"
        || descriptor.entry_point.return_c_type != "MermanNativeStatus"
        || descriptor.entry_point.return_rust_type != "MermanNativeStatus"
        || descriptor.entry_point.parameters
            != [
                ParameterDescriptor {
                    name: "request".to_string(),
                    c_type: "const MermanNativeApiRequest *".to_string(),
                    rust_type: "*const MermanNativeApiRequest".to_string(),
                },
                ParameterDescriptor {
                    name: "out_api".to_string(),
                    c_type: "MermanNativeApi *".to_string(),
                    rust_type: "*mut MermanNativeApi".to_string(),
                },
            ]
    {
        return Err(descriptor_error(
            "native ABI must preserve the exact `merman_get_native_api` C entry-point signature",
        ));
    }
    if descriptor.entry_point.description.trim().is_empty() {
        return Err(descriptor_error(
            "native ABI entry point must have a description",
        ));
    }

    validate_code_descriptors(
        &descriptor.status_codes,
        "native ABI status codes",
        "MERMAN_NATIVE_STATUS_",
    )?;
    if descriptor.status_codes.len() != minimum_prefix.status_code_count {
        return Err(descriptor_error(format!(
            "native ABI status vocabulary is closed by the minimum prefix at {} entries, but the descriptor defines {}",
            minimum_prefix.status_code_count,
            descriptor.status_codes.len()
        )));
    }
    let known_error_kinds = descriptor
        .error_kinds
        .iter()
        .map(|kind| kind.id.as_str())
        .collect::<BTreeSet<_>>();
    for status in &descriptor.status_codes {
        validate_unique(
            status.error_kinds.iter().map(String::as_str),
            &format!("native ABI status `{}` error kinds", status.id),
        )?;
        for error_kind in &status.error_kinds {
            if !known_error_kinds.contains(error_kind.as_str()) {
                return Err(descriptor_error(format!(
                    "native ABI status `{}` references unknown error kind `{error_kind}`",
                    status.id
                )));
            }
        }
    }
    if descriptor.operation_codes.is_empty() {
        return Err(descriptor_error(
            "native ABI operation codes must not be empty",
        ));
    }
    validate_unique(
        descriptor
            .operation_codes
            .iter()
            .map(|operation| operation.id.as_str()),
        "native ABI operation code ids",
    )?;
    validate_unique(
        descriptor
            .operation_codes
            .iter()
            .map(|operation| operation.c_name.as_str()),
        "native ABI operation code C names",
    )?;
    validate_contiguous_codes(
        descriptor
            .operation_codes
            .iter()
            .map(|operation| operation.code),
        "native ABI operation codes",
    )?;
    validate_codes_in_descriptor_order(
        descriptor
            .operation_codes
            .iter()
            .map(|operation| operation.code),
        "native ABI operation codes",
    )?;
    for operation in &descriptor.operation_codes {
        validate_identifier(&operation.id, "native ABI operation code")?;
        if !operation.c_name.starts_with("MERMAN_NATIVE_OPERATION_")
            || !is_ascii_identifier(&operation.c_name)
        {
            return Err(descriptor_error(format!(
                "native ABI operation `{}` must use the MERMAN_NATIVE_OPERATION_ C name prefix",
                operation.id
            )));
        }
        if let Some(failure) = &operation.non_executable_failure {
            let status = descriptor
                .status_codes
                .iter()
                .find(|status| status.id == failure.status_id)
                .ok_or_else(|| {
                    descriptor_error(format!(
                        "native ABI operation `{}` references unknown non-executable status `{}`",
                        operation.id, failure.status_id
                    ))
                })?;
            if !known_error_kinds.contains(failure.error_kind_id.as_str()) {
                return Err(descriptor_error(format!(
                    "native ABI operation `{}` references unknown non-executable error kind `{}`",
                    operation.id, failure.error_kind_id
                )));
            }
            if !status.error_kinds.contains(&failure.error_kind_id) {
                return Err(descriptor_error(format!(
                    "native ABI operation `{}` non-executable status `{}` does not permit error kind `{}`",
                    operation.id, failure.status_id, failure.error_kind_id
                )));
            }
        }
        if operation.id != "none"
            && (!operation.executable || operation.non_executable_failure.is_some())
        {
            return Err(descriptor_error(format!(
                "native ABI operation `{}` must remain executable and have no non-executable failure",
                operation.id
            )));
        }
    }
    validate_operation_global_identifiers(&descriptor.operation_codes)?;
    validate_flutter_operation_projection(&descriptor.operation_codes)?;
    let none = descriptor
        .operation_codes
        .iter()
        .find(|operation| operation.id == "none")
        .ok_or_else(|| descriptor_error("native ABI operation codes must include `none`"))?;
    if none.code != 0 {
        return Err(descriptor_error(
            "native ABI `none` operation must use code 0",
        ));
    }
    let Some(_) = &none.non_executable_failure else {
        return Err(descriptor_error(
            "native ABI `none` operation must define its non-executable failure",
        ));
    };
    if none.executable {
        return Err(descriptor_error(
            "native ABI `none` operation must remain non-executable",
        ));
    }

    validate_unique(
        descriptor
            .callbacks
            .iter()
            .map(|callback| callback.id.as_str()),
        "native ABI callback ids",
    )?;
    validate_unique(
        descriptor
            .callbacks
            .iter()
            .map(|callback| callback.c_name.as_str()),
        "native ABI callback C names",
    )?;
    validate_contiguous_codes(
        descriptor.callbacks.iter().map(|callback| callback.code),
        "native ABI callbacks",
    )?;
    validate_codes_in_descriptor_order(
        descriptor.callbacks.iter().map(|callback| callback.code),
        "native ABI callbacks",
    )?;
    for callback in &descriptor.callbacks {
        validate_callable(callback, "native ABI callback", true)?;
    }

    validate_unique(
        descriptor
            .function_slots
            .iter()
            .map(|slot| slot.id.as_str()),
        "native ABI function slot ids",
    )?;
    validate_unique(
        descriptor
            .function_slots
            .iter()
            .map(|slot| slot.c_name.as_str()),
        "native ABI function slot C names",
    )?;
    validate_contiguous_codes(
        descriptor.function_slots.iter().map(|slot| slot.code),
        "native ABI function slots",
    )?;
    validate_codes_in_descriptor_order(
        descriptor.function_slots.iter().map(|slot| slot.code),
        "native ABI function slots",
    )?;
    for slot in &descriptor.function_slots {
        validate_callable(slot, "native ABI function slot", true)?;
    }
    if descriptor.records.is_empty() {
        return Err(descriptor_error("native ABI records must not be empty"));
    }
    validate_unique(
        descriptor.records.iter().map(|record| record.id.as_str()),
        "native ABI record ids",
    )?;
    validate_unique(
        descriptor
            .records
            .iter()
            .map(|record| record.c_name.as_str()),
        "native ABI record C names",
    )?;
    let mut function_table_records = 0usize;
    for record in &descriptor.records {
        validate_identifier(&record.id, "native ABI record")?;
        if !is_ascii_identifier(&record.c_name) || !is_ascii_identifier(&record.rust_name) {
            return Err(descriptor_error(format!(
                "native ABI record `{}` must have valid C and Rust names",
                record.id
            )));
        }
        if record.description.trim().is_empty() || record.fields.is_empty() {
            return Err(descriptor_error(format!(
                "native ABI record `{}` must have a description and fields",
                record.id
            )));
        }
        if record.fields[0].name != "struct_size" {
            return Err(descriptor_error(format!(
                "native ABI record `{}` must start with struct_size",
                record.id
            )));
        }
        validate_unique(
            record.fields.iter().map(|field| field.name.as_str()),
            &format!("native ABI record `{}` fields", record.id),
        )?;
        for field in &record.fields {
            validate_identifier(
                &field.name,
                &format!("native ABI record `{}` field", record.id),
            )?;
            validate_type_name(
                &field.c_type,
                "C",
                &format!("native ABI record `{}` field `{}`", record.id, field.name),
            )?;
            validate_type_name(
                &field.rust_type,
                "Rust",
                &format!("native ABI record `{}` field `{}`", record.id, field.name),
            )?;
            if field.ownership.trim().is_empty() {
                return Err(descriptor_error(format!(
                    "native ABI record `{}` field `{}` must describe ownership",
                    record.id, field.name
                )));
            }
        }
        if record.appends_function_slots {
            function_table_records += 1;
            if record.id != "api" {
                return Err(descriptor_error(
                    "only the native ABI api record may append function slots",
                ));
            }
        }
    }
    if function_table_records != 1 {
        return Err(descriptor_error(
            "native ABI must have exactly one api record that appends function slots",
        ));
    }
    validate_unique(
        descriptor
            .ownership_rules
            .iter()
            .map(|rule| rule.id.as_str()),
        "native ABI ownership rule ids",
    )?;
    for rule in &descriptor.ownership_rules {
        validate_identifier(&rule.id, "native ABI ownership rule")?;
        if rule.description.trim().is_empty() {
            return Err(descriptor_error(format!(
                "native ABI ownership rule `{}` must have a description",
                rule.id
            )));
        }
    }
    Ok(())
}

fn resolve_operations(
    root: &Path,
    descriptor: &NativeAbiDescriptor,
) -> Result<Vec<ResolvedOperation>, XtaskError> {
    let path = root.join(CAPABILITY_DESCRIPTOR_PATH);
    let text = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let capability_descriptor =
        serde_json::from_str::<CapabilityDescriptor>(&text).map_err(|error| {
            native_abi_error(format!(
                "{}: capability descriptor schema error: {error}",
                path.display()
            ))
        })?;
    let mut operations = BTreeMap::new();
    for operation in &capability_descriptor.binding_operations {
        if operations
            .insert(operation.id.as_str(), operation)
            .is_some()
        {
            return Err(native_abi_error(format!(
                "capability descriptor has duplicate binding operation `{}`",
                operation.id
            )));
        }
    }

    let native_operation_ids = capability_descriptor
        .binding_operations
        .iter()
        .filter(|operation| operation.targets.iter().any(|target| target == "native"))
        .map(|operation| operation.id.as_str())
        .collect::<BTreeSet<_>>();
    let abi_operation_ids = descriptor
        .operation_codes
        .iter()
        .filter(|operation| operation.id != "none")
        .map(|operation| operation.id.replace('_', "-"))
        .collect::<BTreeSet<_>>();
    let native_operation_ids_owned = native_operation_ids
        .iter()
        .map(|id| (*id).to_string())
        .collect::<BTreeSet<_>>();
    if abi_operation_ids != native_operation_ids_owned {
        return Err(native_abi_error(format!(
            "native ABI operation codes must exactly cover canonical native binding operations; ABI={abi_operation_ids:?}, canonical={native_operation_ids:?}"
        )));
    }

    let mut resolved = Vec::with_capacity(descriptor.operation_codes.len());
    for operation_code in sorted_operation_codes(&descriptor.operation_codes) {
        let non_executable_failure = operation_code
            .non_executable_failure
            .as_ref()
            .map(|failure| -> Result<ResolvedOperationFailure, XtaskError> {
                let status = descriptor
                    .status_codes
                    .iter()
                    .find(|status| status.id == failure.status_id)
                    .ok_or_else(|| {
                        native_abi_error(format!(
                            "native ABI operation `{}` references unknown status `{}`",
                            operation_code.id, failure.status_id
                        ))
                    })?;
                let error_kind = descriptor
                    .error_kinds
                    .iter()
                    .find(|kind| kind.id == failure.error_kind_id)
                    .ok_or_else(|| {
                        native_abi_error(format!(
                            "native ABI operation `{}` references unknown error kind `{}`",
                            operation_code.id, failure.error_kind_id
                        ))
                    })?;
                Ok(ResolvedOperationFailure {
                    status_id: status.id.clone(),
                    status_code: status.code,
                    error_kind_id: error_kind.id.clone(),
                    error_kind_json_name: error_kind.json_name.clone(),
                })
            })
            .transpose()?;
        if operation_code.id == "none" {
            resolved.push(ResolvedOperation {
                id: operation_code.id.clone(),
                c_name: operation_code.c_name.clone(),
                code: operation_code.code,
                executable: operation_code.executable,
                non_executable_failure,
                binding_operation_id: None,
                capability_id: None,
                media_type: None,
                requires_uri: false,
            });
            continue;
        }

        let binding_operation_id = operation_code.id.replace('_', "-");
        let operation = operations
            .get(binding_operation_id.as_str())
            .ok_or_else(|| {
            native_abi_error(format!(
                    "native ABI operation `{}` has no canonical binding operation `{binding_operation_id}`",
                    operation_code.id
            ))
        })?;
        resolved.push(ResolvedOperation {
            id: operation_code.id.clone(),
            c_name: operation_code.c_name.clone(),
            code: operation_code.code,
            executable: operation_code.executable,
            non_executable_failure,
            binding_operation_id: Some(operation.id.clone()),
            capability_id: operation.capability.clone(),
            media_type: Some(operation.media_type.clone()),
            requires_uri: operation.requires_uri,
        });
    }
    Ok(resolved)
}

fn canonical_descriptor(descriptor: &NativeAbiDescriptor) -> NativeAbiDescriptor {
    let mut canonical = descriptor.clone();
    canonical
        .opaque_scalars
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .caller_memory_rules
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .error_kinds
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical.status_codes.sort_by_key(|code| code.code);
    canonical
        .operation_codes
        .sort_by_key(|operation| operation.code);
    canonical
        .callbacks
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical.function_slots.sort_by_key(|slot| slot.code);
    canonical
        .ownership_rules
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .records
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
}

fn descriptor_digest(
    descriptor: &NativeAbiDescriptor,
    context: &str,
) -> Result<String, XtaskError> {
    let bytes = serde_json::to_vec(&canonical_descriptor(descriptor)).map_err(|error| {
        native_abi_error(format!(
            "failed to serialize canonical native ABI {context}: {error}"
        ))
    })?;
    Ok(format!("sha256:{}", crate::util::sha256_hex(&bytes)))
}

fn full_descriptor_digest(
    descriptor: &NativeAbiDescriptor,
    resolved_operations: &[ResolvedOperation],
) -> Result<String, XtaskError> {
    #[derive(Serialize)]
    struct FullDescriptorProvenance {
        descriptor: NativeAbiDescriptor,
        resolved_operations: Vec<ResolvedOperation>,
    }

    let mut operations = resolved_operations.to_vec();
    operations.sort_by_key(|operation| operation.code);
    let provenance = FullDescriptorProvenance {
        descriptor: canonical_descriptor(descriptor),
        resolved_operations: operations,
    };
    let bytes = serde_json::to_vec(&provenance).map_err(|error| {
        native_abi_error(format!(
            "failed to serialize canonical native ABI full descriptor provenance: {error}"
        ))
    })?;
    Ok(format!("sha256:{}", crate::util::sha256_hex(&bytes)))
}

fn strip_layout_irrelevant_metadata(descriptor: &mut NativeAbiDescriptor) {
    descriptor.entry_point.description.clear();
    descriptor.entry_point.return_c_type.clear();
    descriptor.entry_point.return_rust_type.clear();
    descriptor.entry_point.parameters.clear();
    descriptor.opaque_scalars.clear();
    descriptor.caller_memory_rules.clear();
    for status in &mut descriptor.status_codes {
        status.description.clear();
        status.error_kinds.clear();
    }
    for operation in &mut descriptor.operation_codes {
        operation.executable = false;
        operation.non_executable_failure = None;
    }
    for kind in &mut descriptor.error_kinds {
        kind.description.clear();
    }
    for callback in &mut descriptor.callbacks {
        callback.description.clear();
    }
    for slot in &mut descriptor.function_slots {
        slot.description.clear();
    }
    for record in &mut descriptor.records {
        record.description.clear();
        for field in &mut record.fields {
            field.ownership.clear();
        }
    }
    for rule in &mut descriptor.ownership_rules {
        rule.description.clear();
    }
}

fn prefix_layout_digest(
    descriptor: &NativeAbiDescriptor,
    function_slot_count: usize,
    context: &str,
) -> Result<String, XtaskError> {
    let minimum = &descriptor.minimum_prefix;
    let mut prefix = descriptor.clone();
    prefix.status_codes.sort_by_key(|status| status.code);
    prefix.status_codes.truncate(minimum.status_code_count);
    prefix
        .operation_codes
        .sort_by_key(|operation| operation.code);
    prefix
        .operation_codes
        .truncate(minimum.operation_code_count);
    prefix.error_kinds.truncate(minimum.error_kind_count);
    prefix.callbacks.sort_by_key(|callback| callback.code);
    prefix.callbacks.truncate(minimum.callback_count);
    prefix.function_slots.sort_by_key(|slot| slot.code);
    prefix.function_slots.truncate(function_slot_count);
    prefix.minimum_prefix.function_slot_count = function_slot_count;
    prefix.records.truncate(minimum.record_count);
    prefix.minimum_semantics.clear();
    prefix.ownership_rules.clear();
    strip_layout_irrelevant_metadata(&mut prefix);
    descriptor_digest(&prefix, context)
}

fn minimum_prefix_layout_digest(descriptor: &NativeAbiDescriptor) -> Result<String, XtaskError> {
    prefix_layout_digest(
        descriptor,
        descriptor.minimum_prefix.function_slot_count,
        "minimum-prefix layout",
    )
}

fn upper_snake(value: &str) -> String {
    value.replace('-', "_").to_ascii_uppercase()
}

fn upper_camel(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn lower_camel(value: &str) -> String {
    let upper = upper_camel(value);
    let mut characters = upper.chars();
    match characters.next() {
        Some(first) => format!("{}{}", first.to_ascii_lowercase(), characters.as_str()),
        None => String::new(),
    }
}

fn validate_dart_lower_camel_members<'a>(
    values: impl IntoIterator<Item = &'a str>,
    context: &str,
    fixed_members: &[&str],
) -> Result<(), String> {
    let mut projected = BTreeMap::new();
    for value in values {
        let member = lower_camel(value);
        if !is_ascii_identifier(&member) {
            return Err(descriptor_error(format!(
                "{context} `{value}` projects to invalid Dart identifier `{member}`"
            )));
        }
        if fixed_members.contains(&member.as_str()) {
            return Err(descriptor_error(format!(
                "{context} `{value}` projects to fixed Dart MermanOperation member `{member}`"
            )));
        }
        if let Some(previous) = projected.insert(member.clone(), value) {
            return Err(descriptor_error(format!(
                "{context}s `{previous}` and `{value}` both project to Dart member `{member}`"
            )));
        }
    }
    Ok(())
}

fn validate_flutter_operation_projection(
    operations: &[OperationCodeDescriptor],
) -> Result<(), String> {
    validate_dart_lower_camel_members(
        operations
            .iter()
            .filter(|operation| operation.executable)
            .map(|operation| operation.id.as_str()),
        "native ABI executable operation id",
        FLUTTER_OPERATION_FIXED_MEMBERS,
    )
}

fn sorted_codes(values: &[CodeDescriptor]) -> Vec<&CodeDescriptor> {
    let mut values = values.iter().collect::<Vec<_>>();
    values.sort_by_key(|value| value.code);
    values
}

fn sorted_error_kinds(values: &[ErrorKindDescriptor]) -> Vec<&ErrorKindDescriptor> {
    let mut values = values.iter().collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}

fn sorted_operation_codes(values: &[OperationCodeDescriptor]) -> Vec<&OperationCodeDescriptor> {
    let mut values = values.iter().collect::<Vec<_>>();
    values.sort_by_key(|value| value.code);
    values
}

fn sorted_slots(values: &[CallableDescriptor]) -> Vec<&CallableDescriptor> {
    let mut values = values.iter().collect::<Vec<_>>();
    values.sort_by_key(|value| value.code);
    values
}

fn minimum_prefix_terminal_slot(descriptor: &NativeAbiDescriptor) -> &CallableDescriptor {
    descriptor
        .function_slots
        .get(descriptor.minimum_prefix.function_slot_count - 1)
        .expect("validated minimum prefix selects a function slot")
}

fn opaque_scalar<'a>(descriptor: &'a NativeAbiDescriptor, id: &str) -> &'a OpaqueScalarDescriptor {
    descriptor
        .opaque_scalars
        .iter()
        .find(|scalar| scalar.id == id)
        .unwrap_or_else(|| panic!("validated native ABI descriptor must define `{id}`"))
}

fn c_parameter_list(parameters: &[ParameterDescriptor]) -> String {
    if parameters.is_empty() {
        "void".to_string()
    } else {
        parameters
            .iter()
            .map(|parameter| c_declaration(&parameter.c_type, &parameter.name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn c_declaration(c_type: &str, name: &str) -> String {
    if c_type.ends_with('*') {
        format!("{c_type}{name}")
    } else {
        format!("{c_type} {name}")
    }
}

fn rust_parameter_list(parameters: &[ParameterDescriptor]) -> String {
    parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, parameter.rust_type))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_rust_string_constant(out: &mut String, name: &str, value: &str) {
    let single_line = format!("pub const {name}: &str = {value:?};");
    if single_line.len() <= 100 {
        writeln!(out, "{single_line}").unwrap();
    } else {
        writeln!(out, "pub const {name}: &str =").unwrap();
        writeln!(out, "    {value:?};").unwrap();
    }
}

fn render_rust_callable_type(out: &mut String, callable: &CallableDescriptor) {
    let parameters = rust_parameter_list(&callable.parameters);
    let right_hand_side = format!(
        "unsafe extern \"C\" fn({parameters}) -> {}",
        callable.return_rust_type
    );
    let single_line = format!("pub type {} = {right_hand_side};", callable.rust_name);
    if single_line.len() <= 100 {
        writeln!(out, "{single_line}").unwrap();
    } else if right_hand_side.len() + 4 <= 100 {
        writeln!(out, "pub type {} =", callable.rust_name).unwrap();
        writeln!(out, "    {right_hand_side};").unwrap();
    } else {
        writeln!(
            out,
            "pub type {} = unsafe extern \"C\" fn(",
            callable.rust_name
        )
        .unwrap();
        for parameter in &callable.parameters {
            writeln!(out, "    {}: {},", parameter.name, parameter.rust_type).unwrap();
        }
        writeln!(out, ") -> {};", callable.return_rust_type).unwrap();
    }
}

fn render_c_header(
    descriptor: &NativeAbiDescriptor,
    minimum_prefix_digest: &str,
    full_descriptor_digest: &str,
    operations: &[ResolvedOperation],
) -> String {
    let mut out = String::from(
        "/* @generated by `cargo run -p xtask -- gen-native-abi`. */\n/* Sources: contracts/abi/merman-v3.json and capabilities/feature-surface-v1.json. */\n/* Do not edit directly. */\n\n#ifndef MERMAN_H\n#define MERMAN_H\n\n#include <stddef.h>\n#include <stdint.h>\n\n#include \"merman_text_measurement_abi.h\"\n\n#if defined(__cplusplus) && (__cplusplus >= 201703L || (defined(_MSVC_LANG) && _MSVC_LANG >= 201703L))\n#define MERMAN_NATIVE_NOEXCEPT noexcept\n#else\n#define MERMAN_NATIVE_NOEXCEPT\n#endif\n\n#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n",
    );
    writeln!(
        out,
        "#define MERMAN_NATIVE_ABI_VERSION {}u",
        descriptor.abi_version
    )
    .unwrap();
    writeln!(
        out,
        "#define MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST \"{minimum_prefix_digest}\""
    )
    .unwrap();
    writeln!(
        out,
        "#define MERMAN_NATIVE_ABI_FULL_DESCRIPTOR_DIGEST \"{full_descriptor_digest}\""
    )
    .unwrap();
    writeln!(
        out,
        "#define MERMAN_NATIVE_RESULT_SCHEMA_VERSION {}u",
        descriptor.result_schema_version
    )
    .unwrap();
    for kind in sorted_error_kinds(&descriptor.error_kinds) {
        writeln!(out, "#define {} {:?}", kind.c_name, kind.json_name).unwrap();
    }
    out.push_str(
        "#define MERMAN_NATIVE_STRUCT_SIZE(type) ((uint32_t)sizeof(type))\n#ifdef __cplusplus\n#define MERMAN_NATIVE_RESULT_INIT { MERMAN_NATIVE_STRUCT_SIZE(MermanNativeResult), 0, 0, 0, {}, {}, {} }\n#else\n#define MERMAN_NATIVE_RESULT_INIT { .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeResult) }\n#endif\n\n",
    );

    render_c_code_type(
        &mut out,
        "MermanNativeStatus",
        "MERMAN_NATIVE_STATUS",
        &sorted_codes(&descriptor.status_codes),
    );
    render_c_operation_type(&mut out, operations);
    render_c_slot_type(&mut out, &sorted_slots(&descriptor.function_slots));
    let engine_token = opaque_scalar(descriptor, "engine_token");
    writeln!(
        out,
        "typedef {} {};\n",
        engine_token.c_type,
        engine_token
            .c_name
            .as_deref()
            .expect("validated engine token has a C name")
    )
    .unwrap();

    for record in &descriptor.records {
        writeln!(out, "typedef struct {} {};", record.c_name, record.c_name).unwrap();
    }
    out.push('\n');

    for callback in &descriptor.callbacks {
        writeln!(
            out,
            "typedef {} (*{})({}) MERMAN_NATIVE_NOEXCEPT;",
            callback.return_c_type,
            callback.c_name,
            c_parameter_list(&callback.parameters)
        )
        .unwrap();
    }
    out.push('\n');

    for slot in sorted_slots(&descriptor.function_slots) {
        writeln!(
            out,
            "typedef {} (*{})({}) MERMAN_NATIVE_NOEXCEPT;",
            slot.return_c_type,
            slot.c_name,
            c_parameter_list(&slot.parameters)
        )
        .unwrap();
    }
    out.push('\n');

    for record in &descriptor.records {
        writeln!(out, "struct {} {{", record.c_name).unwrap();
        for field in &record.fields {
            writeln!(out, "    {};", c_declaration(&field.c_type, &field.name)).unwrap();
        }
        if record.appends_function_slots {
            for slot in sorted_slots(&descriptor.function_slots) {
                writeln!(out, "    {} {};", slot.c_name, slot.id).unwrap();
            }
        }
        out.push_str("};\n\n");
    }
    let minimum_slot = minimum_prefix_terminal_slot(descriptor);
    writeln!(
        out,
        "#define MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE ((uint32_t)(offsetof(MermanNativeApi, {}) + sizeof(((MermanNativeApi *)0)->{})))\n",
        minimum_slot.id,
        minimum_slot.id,
    )
    .unwrap();
    for slot in sorted_slots(&descriptor.function_slots)
        .into_iter()
        .skip(descriptor.minimum_prefix.function_slot_count)
    {
        writeln!(
            out,
            "#define MERMAN_NATIVE_API_{}_PREFIX_SIZE ((uint32_t)(offsetof(MermanNativeApi, {}) + sizeof(((MermanNativeApi *)0)->{})))",
            upper_snake(&slot.id),
            slot.id,
            slot.id,
        )
        .unwrap();
    }
    out.push('\n');

    out.push_str(
        "/*\n * The minimum-prefix digest negotiates layout compatibility. The full descriptor and capability\n * digests report provenance and do not reject a compatible prefix. Except for MermanNativeApi,\n * public records require exact struct_size. The caller supplies MermanNativeApi capacity and\n * receives the largest complete producer prefix it can safely read. MermanNativeResult must be\n * fully zero-initialized with MERMAN_NATIVE_RESULT_INIT before every producing call.\n *\n * ABI 3 semantics from the descriptor:\n",
    );
    for semantic in &descriptor.minimum_semantics {
        render_c_comment_item(&mut out, &semantic.id, &semantic.description);
    }
    out.push_str(" *\n * Ownership and concurrency rules from the ABI descriptor:\n");
    let mut ownership_rules = descriptor.ownership_rules.iter().collect::<Vec<_>>();
    ownership_rules.sort_by(|left, right| left.id.cmp(&right.id));
    for rule in ownership_rules {
        render_c_comment_item(&mut out, &rule.id, &rule.description);
    }
    out.push_str(" *\n * Opaque scalar rules from the ABI descriptor:\n");
    for scalar in &descriptor.opaque_scalars {
        render_c_comment_item(&mut out, &scalar.id, &scalar.description);
    }
    out.push_str(" *\n * Unsafe caller-memory preconditions from the ABI descriptor:\n");
    for rule in &descriptor.caller_memory_rules {
        render_c_comment_item(&mut out, &rule.id, &rule.description);
    }
    out.push_str(" */\n");
    writeln!(
        out,
        "{} {}({}) MERMAN_NATIVE_NOEXCEPT;",
        descriptor.entry_point.return_c_type,
        descriptor.entry_point.c_name,
        c_parameter_list(&descriptor.entry_point.parameters)
    )
    .unwrap();
    out.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n#endif\n");
    out
}

fn render_c_comment_item(out: &mut String, id: &str, description: &str) {
    const LINE_WIDTH: usize = 100;
    let continuation = " *   ";
    let mut line = format!(" * - {id}: ");
    for word in description.split_whitespace() {
        let separator = usize::from(!line.ends_with(' '));
        if line.len() + separator + word.len() > LINE_WIDTH && line.len() > continuation.len() {
            writeln!(out, "{line}").unwrap();
            line = continuation.to_string();
        }
        if !line.ends_with(' ') {
            line.push(' ');
        }
        line.push_str(word);
    }
    writeln!(out, "{line}").unwrap();
}

fn render_c_code_type(
    out: &mut String,
    type_name: &str,
    _prefix: &str,
    values: &[&CodeDescriptor],
) {
    writeln!(out, "typedef int32_t {type_name};").unwrap();
    out.push_str("enum {\n");
    for (index, value) in values.iter().enumerate() {
        writeln!(
            out,
            "    {} = {}{}",
            value.c_name,
            value.code,
            if index + 1 == values.len() { "" } else { "," }
        )
        .unwrap();
    }
    out.push_str("};\n\n");
}

fn render_c_operation_type(out: &mut String, values: &[ResolvedOperation]) {
    out.push_str("typedef int32_t MermanNativeOperationCode;\nenum {\n");
    for (index, value) in values.iter().enumerate() {
        writeln!(
            out,
            "    {} = {}{}",
            value.c_name,
            value.code,
            if index + 1 == values.len() { "" } else { "," }
        )
        .unwrap();
    }
    out.push_str("};\n");
    for value in values {
        let suffix = upper_snake(&value.id);
        writeln!(
            out,
            "#define MERMAN_NATIVE_OPERATION_REQUIRES_URI_{suffix} {}",
            if value.requires_uri { "1" } else { "0" }
        )
        .unwrap();
        writeln!(
            out,
            "#define MERMAN_NATIVE_OPERATION_EXECUTABLE_{suffix} {}",
            if value.executable { "1" } else { "0" }
        )
        .unwrap();
        if let Some(failure) = &value.non_executable_failure {
            writeln!(
                out,
                "#define MERMAN_NATIVE_OPERATION_NON_EXECUTABLE_STATUS_{suffix} {}",
                failure.status_code
            )
            .unwrap();
            writeln!(
                out,
                "#define MERMAN_NATIVE_OPERATION_NON_EXECUTABLE_ERROR_KIND_{suffix} \"{}\"",
                failure.error_kind_json_name
            )
            .unwrap();
        }
        if let Some(media_type) = &value.media_type {
            if let Some(binding_operation_id) = &value.binding_operation_id {
                writeln!(
                    out,
                    "#define MERMAN_NATIVE_OPERATION_ID_{suffix} \"{binding_operation_id}\""
                )
                .unwrap();
            }
            if let Some(capability_id) = &value.capability_id {
                writeln!(
                    out,
                    "#define MERMAN_NATIVE_OPERATION_CAPABILITY_{suffix} \"{capability_id}\""
                )
                .unwrap();
            }
            writeln!(
                out,
                "#define MERMAN_NATIVE_OPERATION_MEDIA_TYPE_{suffix} \"{media_type}\""
            )
            .unwrap();
        }
    }
    out.push('\n');
}

fn render_c_slot_type(out: &mut String, values: &[&CallableDescriptor]) {
    out.push_str("typedef int32_t MermanNativeFunctionSlot;\nenum {\n");
    for (index, value) in values.iter().enumerate() {
        writeln!(
            out,
            "    MERMAN_NATIVE_FUNCTION_{} = {}{}",
            upper_snake(&value.id),
            value.code,
            if index + 1 == values.len() { "" } else { "," }
        )
        .unwrap();
    }
    out.push_str("};\n\n");
}

fn render_rust(
    descriptor: &NativeAbiDescriptor,
    minimum_prefix_digest: &str,
    full_descriptor_digest: &str,
    operations: &[ResolvedOperation],
) -> String {
    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-native-abi`.\n// Sources: contracts/abi/merman-v3.json and capabilities/feature-surface-v1.json.\n// Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "pub const MERMAN_NATIVE_ABI_VERSION: u32 = {};",
        descriptor.abi_version
    )
    .unwrap();
    render_rust_string_constant(
        &mut out,
        "MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST",
        minimum_prefix_digest,
    );
    render_rust_string_constant(
        &mut out,
        "MERMAN_NATIVE_ABI_FULL_DESCRIPTOR_DIGEST",
        full_descriptor_digest,
    );
    writeln!(
        out,
        "pub const MERMAN_NATIVE_RESULT_SCHEMA_VERSION: u32 = {};",
        descriptor.result_schema_version
    )
    .unwrap();
    for kind in sorted_error_kinds(&descriptor.error_kinds) {
        render_rust_string_constant(&mut out, &kind.c_name, &kind.json_name);
    }
    writeln!(
        out,
        "pub const MERMAN_NATIVE_ABI_ENTRY_POINT: &str = {:?};\n",
        descriptor.entry_point.rust_name
    )
    .unwrap();

    render_rust_code_type(
        &mut out,
        "MermanNativeStatus",
        &sorted_codes(&descriptor.status_codes),
    );
    render_rust_error_contract(&mut out, descriptor);
    render_rust_operation_type(&mut out, operations);
    render_rust_operation_catalog(&mut out, operations);
    render_rust_slot_type(&mut out, &sorted_slots(&descriptor.function_slots));
    let engine_token = opaque_scalar(descriptor, "engine_token");
    let result_token = opaque_scalar(descriptor, "result_allocation_token");
    writeln!(
        out,
        "pub type {} = {};",
        engine_token
            .rust_name
            .as_deref()
            .expect("validated engine token has a Rust name"),
        engine_token.rust_type
    )
    .unwrap();
    writeln!(
        out,
        "pub(crate) const MERMAN_NATIVE_TOKEN_DOMAIN_MASK: u64 = {};",
        engine_token.domain.mask
    )
    .unwrap();
    writeln!(
        out,
        "pub(crate) const MERMAN_NATIVE_TOKEN_COUNTER_SHIFT: u32 = {};",
        engine_token.domain.counter_shift
    )
    .unwrap();
    writeln!(
        out,
        "pub(crate) const MERMAN_NATIVE_TOKEN_COUNTER_MAX: u64 = {};",
        engine_token.domain.maximum_counter
    )
    .unwrap();
    writeln!(
        out,
        "pub(crate) const MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG: u64 = {};",
        engine_token.domain.tag
    )
    .unwrap();
    writeln!(
        out,
        "pub(crate) const MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG: u64 = {};\n",
        result_token.domain.tag
    )
    .unwrap();

    for record in &descriptor.records {
        render_rust_doc_comment(&mut out, &record.description);
        out.push_str("#[repr(C)]\n");
        if record.id != "result" {
            out.push_str("#[derive(Clone, Copy)]\n");
        }
        writeln!(out, "pub struct {} {{", record.rust_name).unwrap();
        for field in &record.fields {
            writeln!(out, "    pub {}: {},", field.name, field.rust_type).unwrap();
        }
        if record.appends_function_slots {
            for slot in sorted_slots(&descriptor.function_slots) {
                writeln!(out, "    pub {}: Option<{}>,", slot.id, slot.rust_name).unwrap();
            }
        }
        out.push_str("}\n\n");
    }

    for callback in &descriptor.callbacks {
        render_rust_callable_type(&mut out, callback);
    }
    out.push('\n');
    for slot in sorted_slots(&descriptor.function_slots) {
        render_rust_callable_type(&mut out, slot);
    }
    let minimum_slot = minimum_prefix_terminal_slot(descriptor);
    writeln!(
        out,
        "\npub const MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE: u32 =\n    (std::mem::offset_of!(MermanNativeApi, {})\n        + std::mem::size_of::<Option<{}>>()) as u32;\n",
        minimum_slot.id,
        minimum_slot.rust_name,
    )
    .unwrap();
    let appended_slots = sorted_slots(&descriptor.function_slots)
        .into_iter()
        .skip(descriptor.minimum_prefix.function_slot_count)
        .collect::<Vec<_>>();
    for slot in &appended_slots {
        writeln!(
            out,
            "pub const MERMAN_NATIVE_API_{}_PREFIX_SIZE: u32 =\n    (std::mem::offset_of!(MermanNativeApi, {})\n        + std::mem::size_of::<Option<{}>>()) as u32;\n",
            upper_snake(&slot.id),
            slot.id,
            slot.rust_name,
        )
        .unwrap();
    }
    out.push_str("pub const MERMAN_NATIVE_API_COMPLETE_PREFIX_SIZES: &[u32] = &[\n");
    out.push_str("    MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE,\n");
    for slot in appended_slots {
        writeln!(
            out,
            "    MERMAN_NATIVE_API_{}_PREFIX_SIZE,",
            upper_snake(&slot.id),
        )
        .unwrap();
    }
    out.push_str("];\n\n");

    out.push_str("pub const MERMAN_NATIVE_ABI_OWNERSHIP_RULES: &[(&str, &str)] = &[\n");
    let mut rules = descriptor.ownership_rules.iter().collect::<Vec<_>>();
    rules.sort_by(|left, right| left.id.cmp(&right.id));
    for rule in rules {
        writeln!(out, "    (").unwrap();
        writeln!(out, "        {:?},", rule.id).unwrap();
        writeln!(out, "        {:?},", rule.description).unwrap();
        writeln!(out, "    ),").unwrap();
    }
    out.push_str("];\n");
    out
}

fn render_rust_doc_comment(out: &mut String, description: &str) {
    const LINE_WIDTH: usize = 100;
    const PREFIX: &str = "/// ";
    let mut line = PREFIX.to_string();
    for word in description.split_whitespace() {
        let separator = usize::from(line.len() > PREFIX.len());
        if line.len() + separator + word.len() > LINE_WIDTH && line.len() > PREFIX.len() {
            writeln!(out, "{line}").unwrap();
            line = PREFIX.to_string();
        }
        if line.len() > PREFIX.len() {
            line.push(' ');
        }
        line.push_str(word);
    }
    writeln!(out, "{line}").unwrap();
}

fn render_rust_code_type(out: &mut String, type_name: &str, values: &[&CodeDescriptor]) {
    writeln!(out, "pub type {type_name} = i32;").unwrap();
    for value in values {
        writeln!(
            out,
            "pub const {}: {type_name} = {};",
            value.c_name, value.code
        )
        .unwrap();
    }
    out.push_str("\npub const MERMAN_NATIVE_STATUSES: &[MermanNativeStatus] = &[\n");
    for value in values {
        writeln!(out, "    {},", value.c_name).unwrap();
    }
    out.push_str(
        "];\n\n\
         pub fn merman_native_status_is_known(status: MermanNativeStatus) -> bool {\n\
         \x20   MERMAN_NATIVE_STATUSES.contains(&status)\n\
         }\n\n",
    );
}

fn render_rust_error_contract(out: &mut String, descriptor: &NativeAbiDescriptor) {
    out.push_str(
        "pub(crate) fn merman_native_status_name(status: MermanNativeStatus) -> &'static str {\n    match status {\n",
    );
    for status in sorted_codes(&descriptor.status_codes) {
        writeln!(
            out,
            "        {} => {:?},",
            status.c_name,
            status.id.replace('_', "-")
        )
        .unwrap();
    }
    out.push_str("        _ => \"unknown-status\",\n    }\n}\n\n");

    out.push_str(
        "pub(crate) fn merman_native_error_kind_name(\n    kind: merman_bindings_core::BindingErrorKind,\n) -> &'static str {\n    match kind {\n",
    );
    for kind in sorted_error_kinds(&descriptor.error_kinds) {
        writeln!(
            out,
            "        merman_bindings_core::BindingErrorKind::{} => {},",
            upper_camel(&kind.id),
            kind.c_name,
        )
        .unwrap();
    }
    out.push_str("    }\n}\n\n");

    out.push_str(
        "pub(crate) fn merman_binding_error_kind_from_native_name(\n    kind: &str,\n) -> Option<merman_bindings_core::BindingErrorKind> {\n    match kind {\n",
    );
    for kind in sorted_error_kinds(&descriptor.error_kinds) {
        writeln!(
            out,
            "        {} => Some(merman_bindings_core::BindingErrorKind::{}),",
            kind.c_name,
            upper_camel(&kind.id),
        )
        .unwrap();
    }
    out.push_str("        _ => None,\n    }\n}\n\n");

    out.push_str(
        "pub(crate) fn merman_native_normalize_error_kind(\n    status: MermanNativeStatus,\n    requested: merman_bindings_core::BindingErrorKind,\n) -> merman_bindings_core::BindingErrorKind {\n    match status {\n",
    );
    for status in sorted_codes(&descriptor.status_codes) {
        if status.error_kinds.is_empty() {
            writeln!(
                out,
                "        {} => merman_bindings_core::BindingErrorKind::Generic,",
                status.c_name,
            )
            .unwrap();
            continue;
        }
        writeln!(out, "        {} => match requested {{", status.c_name).unwrap();
        for error_kind in &status.error_kinds {
            writeln!(
                out,
                "            merman_bindings_core::BindingErrorKind::{} => requested,",
                upper_camel(error_kind),
            )
            .unwrap();
        }
        writeln!(
            out,
            "            _ => merman_bindings_core::BindingErrorKind::{},",
            upper_camel(&status.error_kinds[0]),
        )
        .unwrap();
        out.push_str("        },\n");
    }
    out.push_str("        _ => merman_bindings_core::BindingErrorKind::Generic,\n    }\n}\n\n");
}

fn render_rust_operation_type(out: &mut String, values: &[ResolvedOperation]) {
    out.push_str("pub type MermanNativeOperationCode = i32;\n");
    for value in values {
        writeln!(
            out,
            "pub const {}: MermanNativeOperationCode = {};",
            value.c_name, value.code
        )
        .unwrap();
        let suffix = upper_snake(&value.id);
        writeln!(
            out,
            "pub const MERMAN_NATIVE_OPERATION_REQUIRES_URI_{suffix}: bool = {};",
            value.requires_uri
        )
        .unwrap();
        writeln!(
            out,
            "pub const MERMAN_NATIVE_OPERATION_EXECUTABLE_{suffix}: bool = {};",
            value.executable
        )
        .unwrap();
        if let Some(failure) = &value.non_executable_failure {
            writeln!(
                out,
                "pub const MERMAN_NATIVE_OPERATION_NON_EXECUTABLE_STATUS_{suffix}: MermanNativeStatus = {};",
                failure.status_code
            )
            .unwrap();
            render_rust_string_constant(
                out,
                &format!("MERMAN_NATIVE_OPERATION_NON_EXECUTABLE_ERROR_KIND_{suffix}"),
                &failure.error_kind_json_name,
            );
        }
        if let Some(media_type) = &value.media_type {
            if let Some(binding_operation_id) = &value.binding_operation_id {
                render_rust_string_constant(
                    out,
                    &format!("MERMAN_NATIVE_OPERATION_ID_{suffix}"),
                    binding_operation_id,
                );
            }
            if let Some(capability_id) = &value.capability_id {
                render_rust_string_constant(
                    out,
                    &format!("MERMAN_NATIVE_OPERATION_CAPABILITY_{suffix}"),
                    capability_id,
                );
            }
            render_rust_string_constant(
                out,
                &format!("MERMAN_NATIVE_OPERATION_MEDIA_TYPE_{suffix}"),
                media_type,
            );
        }
    }
    out.push('\n');
}

fn render_rust_operation_catalog(out: &mut String, values: &[ResolvedOperation]) {
    out.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub struct MermanNativeOperationFailureDescriptor {\n\
         \x20   pub status: MermanNativeStatus,\n\
         \x20   pub error_kind: &'static str,\n\
         }\n\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub struct MermanNativeOperationDescriptor {\n\
         \x20   pub code: MermanNativeOperationCode,\n\
         \x20   pub executable: bool,\n\
         \x20   pub non_executable_failure: Option<MermanNativeOperationFailureDescriptor>,\n\
         \x20   pub operation_id: Option<&'static str>,\n\
         \x20   pub capability_id: Option<&'static str>,\n\
         \x20   pub media_type: Option<&'static str>,\n\
         \x20   pub requires_uri: bool,\n\
         }\n\n\
         pub const MERMAN_NATIVE_OPERATION_DESCRIPTORS: &[MermanNativeOperationDescriptor] = &[\n",
    );
    for value in values {
        writeln!(out, "    MermanNativeOperationDescriptor {{").unwrap();
        writeln!(out, "        code: {},", value.c_name).unwrap();
        writeln!(out, "        executable: {},", value.executable).unwrap();
        match &value.non_executable_failure {
            Some(failure) => {
                writeln!(
                    out,
                    "        non_executable_failure: Some(MermanNativeOperationFailureDescriptor {{"
                )
                .unwrap();
                writeln!(out, "            status: {},", failure.status_code).unwrap();
                writeln!(
                    out,
                    "            error_kind: {:?},",
                    failure.error_kind_json_name
                )
                .unwrap();
                writeln!(out, "        }}),").unwrap();
            }
            None => writeln!(out, "        non_executable_failure: None,").unwrap(),
        }
        writeln!(
            out,
            "        operation_id: {:?},",
            value.binding_operation_id
        )
        .unwrap();
        writeln!(out, "        capability_id: {:?},", value.capability_id).unwrap();
        writeln!(out, "        media_type: {:?},", value.media_type).unwrap();
        writeln!(out, "        requires_uri: {},", value.requires_uri).unwrap();
        writeln!(out, "    }},").unwrap();
    }
    out.push_str(
        "];\n\n\
         pub fn merman_native_operation_descriptor(\n\
         \x20   code: MermanNativeOperationCode,\n\
         ) -> Option<&'static MermanNativeOperationDescriptor> {\n\
         \x20   MERMAN_NATIVE_OPERATION_DESCRIPTORS\n\
         \x20       .iter()\n\
         \x20       .find(|descriptor| descriptor.code == code)\n\
         }\n\n\
         pub fn merman_native_operation_key(\n\
         \x20   code: MermanNativeOperationCode,\n\
         ) -> Option<merman_bindings_core::OperationKey> {\n\
         \x20   match code {\n",
    );
    for value in values.iter().filter(|value| value.id != "none") {
        writeln!(
            out,
            "        {} => Some(merman_bindings_core::OperationKey::{}),",
            value.c_name,
            upper_camel(&value.id)
        )
        .unwrap();
    }
    out.push_str(
        "        _ => None,\n\
         \x20   }\n\
         }\n\n\
         pub const fn merman_native_operation_code(\n\
         \x20   key: merman_bindings_core::OperationKey,\n\
         ) -> Option<MermanNativeOperationCode> {\n\
         \x20   match key {\n",
    );
    for value in values.iter().filter(|value| value.id != "none") {
        writeln!(
            out,
            "        merman_bindings_core::OperationKey::{} => Some({}),",
            upper_camel(&value.id),
            value.c_name
        )
        .unwrap();
    }
    out.push_str("        _ => None,\n    }\n}\n\n");
}

fn render_flutter_operations(values: &[ResolvedOperation]) -> String {
    let executable = values
        .iter()
        .filter(|value| value.executable)
        .collect::<Vec<_>>();
    let mut out = String::from(
        "// This file is @generated by `cargo run -p xtask -- gen-native-abi`.\n\
         // Sources: contracts/abi/merman-v3.json and capabilities/feature-surface-v1.json.\n\
         // Do not edit directly.\n\n\
         import 'native_abi.dart' as native;\n\n\
         /// One executable operation in the generated native ABI projection.\n\
         ///\n\
         /// Runtime catalogs may contain newer operation IDs. Convert an ID through\n\
         /// [MermanOperation.fromOperationId] before invocation so an older SDK fails\n\
         /// explicitly instead of guessing a numeric code.\n\
         final class MermanOperation {\n\
         \x20 const MermanOperation._(\n\
         \x20   this.nativeCode,\n\
         \x20   this.operationId,\n\
         \x20   this.requiresUri,\n\
         \x20 );\n\n",
    );

    for value in &executable {
        let suffix = upper_snake(&value.id);
        let name = lower_camel(&value.id);
        writeln!(
            out,
            "  static const {name} = MermanOperation._(\n    native.{},\n    native.MERMAN_NATIVE_OPERATION_ID_{suffix},\n    {},\n  );",
            value.c_name, value.requires_uri
        )
        .unwrap();
    }

    out.push_str("\n  static const List<MermanOperation> knownValues = <MermanOperation>[\n");
    for value in &executable {
        writeln!(out, "    {},", lower_camel(&value.id)).unwrap();
    }
    out.push_str(
        "  ];\n\n\
         \x20 factory MermanOperation.fromNativeCode(int nativeCode) {\n\
         \x20   for (final operation in knownValues) {\n\
         \x20     if (operation.nativeCode == nativeCode) {\n\
         \x20       return operation;\n\
         \x20     }\n\
         \x20   }\n\
         \x20   throw ArgumentError.value(\n\
         \x20     nativeCode,\n\
         \x20     'nativeCode',\n\
         \x20     'No executable operation mapping exists in this generated ABI projection',\n\
         \x20   );\n\
         \x20 }\n\n\
         \x20 factory MermanOperation.fromOperationId(String operationId) {\n\
         \x20   for (final operation in knownValues) {\n\
         \x20     if (operation.operationId == operationId) {\n\
         \x20       return operation;\n\
         \x20     }\n\
         \x20   }\n\
         \x20   throw UnsupportedError(\n\
         \x20     'Operation `$operationId` requires an updated Merman SDK/header before invocation',\n\
         \x20   );\n\
         \x20 }\n\n\
         \x20 final int nativeCode;\n\
         \x20 final String operationId;\n\
         \x20 final bool requiresUri;\n\n\
         \x20 @override\n\
         \x20 bool operator ==(Object other) =>\n\
         \x20     other is MermanOperation &&\n\
         \x20     nativeCode == other.nativeCode &&\n\
         \x20     operationId == other.operationId;\n\n\
         \x20 @override\n\
         \x20 int get hashCode => Object.hash(nativeCode, operationId);\n\n\
         \x20 @override\n\
         \x20 String toString() => operationId;\n\
         }\n",
    );
    out
}

fn render_rust_slot_type(out: &mut String, values: &[&CallableDescriptor]) {
    out.push_str("pub type MermanNativeFunctionSlot = i32;\n");
    for value in values {
        writeln!(
            out,
            "pub const MERMAN_NATIVE_FUNCTION_{}: MermanNativeFunctionSlot = {};",
            upper_snake(&value.id),
            value.code
        )
        .unwrap();
    }
    out.push('\n');
}

fn generated_artifacts(prepared: &PreparedNativeAbi) -> Result<Vec<(PathBuf, String)>, XtaskError> {
    Ok(vec![
        (
            PathBuf::from(FFI_RUST_OUTPUT),
            render_rust(
                &prepared.descriptor,
                &prepared.minimum_prefix_digest,
                &prepared.full_descriptor_digest,
                &prepared.operations,
            ),
        ),
        (
            PathBuf::from(FFI_HEADER_OUTPUT),
            render_c_header(
                &prepared.descriptor,
                &prepared.minimum_prefix_digest,
                &prepared.full_descriptor_digest,
                &prepared.operations,
            ),
        ),
        (
            PathBuf::from(FLUTTER_OPERATION_OUTPUT),
            render_flutter_operations(&prepared.operations),
        ),
    ])
}

fn write_artifact(root: &Path, path: &Path, contents: &str) -> Result<(), XtaskError> {
    let full = root.join(path);
    if fs::read(&full).is_ok_and(|existing| existing == contents.as_bytes()) {
        return Ok(());
    }
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&full, contents).map_err(|source| XtaskError::WriteFile {
        path: full.display().to_string(),
        source,
    })
}

fn load_and_validate(root: &Path) -> Result<PreparedNativeAbi, XtaskError> {
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    let operations = resolve_operations(root, &descriptor)?;
    let minimum_prefix_digest = minimum_prefix_layout_digest(&descriptor)?;
    let full_descriptor_digest = full_descriptor_digest(&descriptor, &operations)?;
    Ok(PreparedNativeAbi {
        descriptor,
        operations,
        minimum_prefix_digest,
        full_descriptor_digest,
    })
}

pub(crate) fn gen_native_abi(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    let root = crate::cmd::workspace_root();
    let prepared = load_and_validate(&root)?;
    for (path, contents) in generated_artifacts(&prepared)? {
        write_artifact(&root, &path, &contents)?;
    }
    Ok(())
}

pub(crate) fn verify_native_abi_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let prepared = load_and_validate(&root)?;
    let mut drift = Vec::new();
    for (path, expected) in generated_artifacts(&prepared)? {
        let full = root.join(&path);
        let actual = fs::read_to_string(&full).map_err(|source| XtaskError::ReadFile {
            path: full.display().to_string(),
            source,
        })?;
        if actual.replace("\r\n", "\n") != expected {
            drift.push(path.display().to_string());
        }
    }
    if drift.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "native ABI projections drifted: {}; regenerate with `cargo run -p xtask -- gen-native-abi`",
            drift.join(", ")
        )))
    }
}

pub(crate) fn verify_native_abi(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    match verify_native_abi_artifacts()? {
        Some(message) => Err(XtaskError::VerifyFailed(message)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed_descriptor() -> NativeAbiDescriptor {
        read_descriptor(&crate::cmd::workspace_root().join(DESCRIPTOR_PATH))
            .expect("committed native ABI descriptor")
    }

    #[test]
    fn descriptor_rejects_unsafe_flutter_operation_member_projections() {
        let mut reserved_member = committed_descriptor();
        let operation = reserved_member
            .operation_codes
            .iter_mut()
            .find(|operation| operation.id == "svg")
            .expect("committed descriptor has svg operation");
        operation.id = "from_native_code".to_string();
        operation.c_name = "MERMAN_NATIVE_OPERATION_FROM_NATIVE_CODE".to_string();
        let error = validate_descriptor(&reserved_member)
            .expect_err("fixed Flutter members must not be shadowed");
        assert!(error.contains("fromNativeCode"), "{error}");
        assert!(
            error.contains("fixed Dart MermanOperation member"),
            "{error}"
        );

        let mut collision = committed_descriptor();
        let first = collision
            .operation_codes
            .iter_mut()
            .find(|operation| operation.id == "analysis_json")
            .expect("committed descriptor has analysis-json operation");
        first.id = "foo_1".to_string();
        first.c_name = "MERMAN_NATIVE_OPERATION_FOO_1".to_string();
        let second = collision
            .operation_codes
            .iter_mut()
            .find(|operation| operation.id == "analysis_facts_json")
            .expect("committed descriptor has analysis-facts-json operation");
        second.id = "foo1".to_string();
        second.c_name = "MERMAN_NATIVE_OPERATION_FOO1".to_string();
        let error = validate_descriptor(&collision)
            .expect_err("normalized Flutter operation members must remain unique");
        assert!(error.contains("`foo_1` and `foo1`"), "{error}");
        assert!(error.contains("Dart member `foo1`"), "{error}");
    }

    #[test]
    fn descriptor_rejects_colliding_derived_operation_globals() {
        let mut descriptor = committed_descriptor();
        let first = descriptor
            .operation_codes
            .iter_mut()
            .find(|operation| operation.id == "analysis_json")
            .expect("committed descriptor has analysis-json operation");
        first.id = "foo".to_string();
        first.c_name = "MERMAN_NATIVE_OPERATION_FOO".to_string();
        let second = descriptor
            .operation_codes
            .iter_mut()
            .find(|operation| operation.id == "analysis_facts_json")
            .expect("committed descriptor has analysis-facts-json operation");
        second.id = "id_foo".to_string();
        second.c_name = "MERMAN_NATIVE_OPERATION_ID_FOO".to_string();

        let error = validate_descriptor(&descriptor)
            .expect_err("operation codes must not collide with derived metadata globals");
        assert!(
            error.contains("global identifier `MERMAN_NATIVE_OPERATION_ID_FOO`"),
            "{error}"
        );
        assert!(
            error.contains("operation `foo` binding operation id"),
            "{error}"
        );
        assert!(
            error.contains("operation `id_foo` operation code"),
            "{error}"
        );

        let mut fixed_global = committed_descriptor();
        let operation = fixed_global
            .operation_codes
            .iter_mut()
            .find(|operation| operation.id == "analysis_json")
            .expect("committed descriptor has analysis-json operation");
        operation.id = "descriptors".to_string();
        operation.c_name = "MERMAN_NATIVE_OPERATION_DESCRIPTORS".to_string();
        let error = validate_descriptor(&fixed_global)
            .expect_err("operation codes must not shadow fixed generated globals");
        assert!(
            error.contains("generated Rust operation descriptor table"),
            "{error}"
        );
    }

    #[test]
    fn descriptor_rejects_unsafe_layout_and_slot_changes() {
        let mut descriptor = committed_descriptor();
        descriptor.records[0].fields.remove(0);
        assert!(validate_descriptor(&descriptor).is_err());

        let mut descriptor = committed_descriptor();
        descriptor.function_slots[0].code = 9;
        assert!(validate_descriptor(&descriptor).is_err());

        let mut descriptor = committed_descriptor();
        descriptor.function_slots.truncate(5);
        assert!(validate_descriptor(&descriptor).is_err());

        let mut descriptor = committed_descriptor();
        descriptor.function_slots.swap(5, 6);
        assert!(validate_descriptor(&descriptor).is_err());

        let mut descriptor = committed_descriptor();
        let mut duplicate = descriptor
            .operation_codes
            .last()
            .cloned()
            .expect("operation vocabulary is non-empty");
        duplicate.id = "duplicate_terminal_code".to_string();
        duplicate.c_name = "MERMAN_NATIVE_OPERATION_DUPLICATE_TERMINAL_CODE".to_string();
        descriptor.operation_codes.push(duplicate);
        assert!(validate_descriptor(&descriptor).is_err());

        let mut descriptor = committed_descriptor();
        descriptor.operation_codes.swap(12, 13);
        assert!(validate_descriptor(&descriptor).is_err());

        let mut descriptor = committed_descriptor();
        descriptor.operation_codes[1].id = "missing_operation".to_string();
        let root = crate::cmd::workspace_root();
        assert!(resolve_operations(&root, &descriptor).is_err());

        let mut descriptor = committed_descriptor();
        descriptor.operation_codes[0].executable = true;
        assert!(validate_descriptor(&descriptor).is_err());

        let mut descriptor = committed_descriptor();
        descriptor.operation_codes[1].executable = false;
        assert!(validate_descriptor(&descriptor).is_err());
    }

    #[test]
    fn prefix_and_full_digests_separate_compatibility_from_provenance() {
        let descriptor = committed_descriptor();
        let root = crate::cmd::workspace_root();
        let operations = resolve_operations(&root, &descriptor).unwrap();
        let original_prefix = minimum_prefix_layout_digest(&descriptor).unwrap();
        let original_full = full_descriptor_digest(&descriptor, &operations).unwrap();

        let mut semantic_provenance = descriptor.clone();
        semantic_provenance.records[0].description =
            "Reworded complete descriptor semantics.".to_string();
        semantic_provenance.records[0].fields[0].ownership =
            "Changed ownership semantics.".to_string();
        assert_eq!(
            minimum_prefix_layout_digest(&semantic_provenance).unwrap(),
            original_prefix
        );
        assert_ne!(
            full_descriptor_digest(&semantic_provenance, &operations).unwrap(),
            original_full
        );

        let mut resolved_operation_provenance = operations.clone();
        let operation = resolved_operation_provenance
            .iter_mut()
            .find(|operation| operation.capability_id.is_some())
            .expect("at least one native operation has a capability");
        operation.capability_id = Some("changed-capability".to_string());
        operation.media_type = Some("application/changed".to_string());
        operation.requires_uri = !operation.requires_uri;
        assert_ne!(
            full_descriptor_digest(&descriptor, &resolved_operation_provenance).unwrap(),
            original_full,
            "the full provenance digest must cover resolved capability behavior"
        );

        let mut structural = descriptor.clone();
        structural.records[0].fields[1].rust_type = "*mut u8".to_string();
        assert_ne!(
            minimum_prefix_layout_digest(&structural).unwrap(),
            original_prefix
        );

        let mut result_contract = committed_descriptor();
        result_contract.result_schema_version += 1;
        assert_ne!(
            minimum_prefix_layout_digest(&result_contract).unwrap(),
            original_prefix
        );

        let mut error_contract = committed_descriptor();
        error_contract.error_kinds[0].json_name = "renamed-generic".to_string();
        assert_ne!(
            minimum_prefix_layout_digest(&error_contract).unwrap(),
            original_prefix
        );

        let mut appended = descriptor;
        let mut future_slot = appended.function_slots.last().unwrap().clone();
        future_slot.id = "future_slot".to_string();
        future_slot.c_name = "MermanNativeFutureSlotFn".to_string();
        future_slot.rust_name = "MermanNativeFutureSlotFn".to_string();
        future_slot.code = appended
            .function_slots
            .last()
            .expect("committed ABI has at least one function slot")
            .code
            + 1;
        appended.function_slots.push(future_slot);
        assert_eq!(
            minimum_prefix_layout_digest(&appended).unwrap(),
            original_prefix
        );
        assert_ne!(
            full_descriptor_digest(&appended, &operations).unwrap(),
            original_full
        );
    }
}
