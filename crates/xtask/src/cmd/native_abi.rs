//! Generation and verification for the native ABI 3 contract.
//!
//! `abi/merman-v3.json` owns native numeric discriminants, record layouts, function-table
//! entries, and ownership semantics. It deliberately references the capability descriptor for
//! semantic IDs rather than defining a second capability catalog.

use crate::XtaskError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const DESCRIPTOR_PATH: &str = "abi/merman-v3.json";
const CAPABILITY_DESCRIPTOR_PATH: &str = "capabilities/feature-surface-v1.json";
const PROTOCOL_ID: &str = "merman-native";
const ABI_VERSION: u32 = 3;
const SCHEMA_VERSION: u32 = 1;
const RESULT_SCHEMA_VERSION: u32 = 1;
const FFI_RUST_OUTPUT: &str = "crates/merman-ffi/src/generated/abi3.rs";
const FFI_HEADER_OUTPUT: &str = "crates/merman-ffi/include/merman.h";
const BINDINGS_OPERATION_OUTPUT: &str =
    "crates/merman-bindings-core/src/generated/native_operations.rs";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeAbiDescriptor {
    schema_version: u32,
    protocol_id: String,
    abi_version: u32,
    result_schema_version: u32,
    error_kinds: Vec<ErrorKindDescriptor>,
    entry_point: EntryPoint,
    status_codes: Vec<CodeDescriptor>,
    operation_codes: Vec<OperationCodeDescriptor>,
    callbacks: Vec<CallableDescriptor>,
    function_slots: Vec<CallableDescriptor>,
    records: Vec<RecordDescriptor>,
    ownership_rules: Vec<OwnershipRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EntryPoint {
    c_name: String,
    rust_name: String,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CodeDescriptor {
    id: String,
    c_name: String,
    code: i32,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ErrorKindDescriptor {
    id: String,
    c_name: String,
    json_name: String,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OperationCodeDescriptor {
    id: String,
    c_name: String,
    code: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ParameterDescriptor {
    name: String,
    c_type: String,
    rust_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FieldDescriptor {
    name: String,
    c_type: String,
    rust_type: String,
    ownership: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone)]
struct ResolvedOperation {
    id: String,
    c_name: String,
    code: i32,
    binding_operation_id: Option<String>,
    capability_id: Option<String>,
    media_type: Option<String>,
    requires_uri: bool,
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

fn is_c_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(byte) if byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn is_rust_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(byte) if byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn validate_c_type(value: &str, context: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b' ' | b'*'));
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} C type `{value}` contains unsupported characters"
        )))
    }
}

fn validate_rust_type(value: &str, context: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'_' | b' ' | b'*' | b'<' | b'>' | b':' | b'(' | b')' | b','
                )
        });
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} Rust type `{value}` contains unsupported characters"
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
    for value in values {
        validate_identifier(&value.id, id_context)?;
        if !value.c_name.starts_with(c_prefix) || !is_c_identifier(&value.c_name) {
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
    if !is_c_identifier(&callable.c_name) {
        return Err(descriptor_error(format!(
            "{context} `{}` has invalid C name `{}`",
            callable.id, callable.c_name
        )));
    }
    if !is_rust_identifier(&callable.rust_name) {
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
    validate_c_type(
        &callable.return_c_type,
        &format!("{context} `{}`", callable.id),
    )?;
    validate_rust_type(
        &callable.return_rust_type,
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
        validate_c_type(
            &parameter.c_type,
            &format!("{context} `{}` parameter `{}`", callable.id, parameter.name),
        )?;
        validate_rust_type(
            &parameter.rust_type,
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
    let expected_error_kinds = [
        ("generic", "MERMAN_NATIVE_ERROR_KIND_GENERIC", "generic"),
        (
            "missing_capability",
            "MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY",
            "missing-capability",
        ),
        (
            "unknown_operation",
            "MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION",
            "unknown-operation",
        ),
        (
            "reentrant_call",
            "MERMAN_NATIVE_ERROR_KIND_REENTRANT_CALL",
            "reentrant-call",
        ),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let actual_error_kinds = descriptor
        .error_kinds
        .iter()
        .map(|kind| {
            (
                kind.id.as_str(),
                kind.c_name.as_str(),
                kind.json_name.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    if actual_error_kinds != expected_error_kinds {
        return Err(descriptor_error(format!(
            "native ABI error kind ID, C name, and JSON name mappings changed; found {actual_error_kinds:?}"
        )));
    }
    for kind in &descriptor.error_kinds {
        validate_identifier(&kind.id, "native ABI error kind")?;
        validate_kebab_identifier(&kind.json_name, "native ABI error kind JSON name")?;
        if !kind.c_name.starts_with("MERMAN_NATIVE_ERROR_KIND_") || !is_c_identifier(&kind.c_name) {
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
    {
        return Err(descriptor_error(
            "native ABI must use `merman_get_native_api` as its only direct entry point",
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
    for operation in &descriptor.operation_codes {
        validate_identifier(&operation.id, "native ABI operation code")?;
        if !operation.c_name.starts_with("MERMAN_NATIVE_OPERATION_")
            || !is_c_identifier(&operation.c_name)
        {
            return Err(descriptor_error(format!(
                "native ABI operation `{}` must use the MERMAN_NATIVE_OPERATION_ C name prefix",
                operation.id
            )));
        }
    }
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
    for slot in &descriptor.function_slots {
        validate_callable(slot, "native ABI function slot", true)?;
    }
    let expected_slots = [
        "runtime_catalog",
        "engine_new",
        "engine_free",
        "execute_collect",
        "result_free",
    ];
    let actual_slots = descriptor
        .function_slots
        .iter()
        .map(|slot| slot.id.as_str())
        .collect::<BTreeSet<_>>();
    if actual_slots != expected_slots.into_iter().collect() {
        return Err(descriptor_error(
            "native ABI 3 function slots must be the required fixed operation table",
        ));
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
        if !is_c_identifier(&record.c_name) || !is_rust_identifier(&record.rust_name) {
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
            validate_c_type(
                &field.c_type,
                &format!("native ABI record `{}` field `{}`", record.id, field.name),
            )?;
            validate_rust_type(
                &field.rust_type,
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
        if operation_code.id == "none" {
            resolved.push(ResolvedOperation {
                id: operation_code.id.clone(),
                c_name: operation_code.c_name.clone(),
                code: operation_code.code,
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
    canonical.entry_point.description.clear();
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
    for status in &mut canonical.status_codes {
        status.description.clear();
    }
    for kind in &mut canonical.error_kinds {
        kind.description.clear();
    }
    for callback in &mut canonical.callbacks {
        callback.description.clear();
    }
    for slot in &mut canonical.function_slots {
        slot.description.clear();
    }
    for record in &mut canonical.records {
        record.description.clear();
        for field in &mut record.fields {
            field.ownership.clear();
        }
    }
    for rule in &mut canonical.ownership_rules {
        rule.description.clear();
    }
    canonical
}

fn layout_descriptor_digest(descriptor: &NativeAbiDescriptor) -> Result<String, XtaskError> {
    let bytes = serde_json::to_vec(&canonical_descriptor(descriptor)).map_err(|error| {
        native_abi_error(format!(
            "failed to serialize canonical native ABI descriptor: {error}"
        ))
    })?;
    Ok(format!("sha256:{}", crate::util::sha256_hex(&bytes)))
}

fn upper_snake(value: &str) -> String {
    value.replace('-', "_").to_ascii_uppercase()
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
    digest: &str,
    operations: &[ResolvedOperation],
) -> String {
    let mut out = String::from(
        "/* @generated by `cargo run -p xtask -- gen-native-abi`. */\n/* Sources: abi/merman-v3.json and capabilities/feature-surface-v1.json. */\n/* Do not edit directly. */\n\n#ifndef MERMAN_H\n#define MERMAN_H\n\n#include <stddef.h>\n#include <stdint.h>\n\n#include \"merman_text_measurement_abi.h\"\n\n#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n",
    );
    writeln!(
        out,
        "#define MERMAN_NATIVE_ABI_VERSION {}u",
        descriptor.abi_version
    )
    .unwrap();
    writeln!(
        out,
        "#define MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST \"{digest}\""
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
        "#define MERMAN_NATIVE_STRUCT_SIZE(type) ((uint32_t)sizeof(type))\n#ifdef __cplusplus\n#define MERMAN_NATIVE_RESULT_INIT { MERMAN_NATIVE_STRUCT_SIZE(MermanNativeResult), 0, 0, {}, {}, {} }\n#else\n#define MERMAN_NATIVE_RESULT_INIT { .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeResult) }\n#endif\n\n",
    );

    render_c_code_type(
        &mut out,
        "MermanNativeStatus",
        "MERMAN_NATIVE_STATUS",
        &sorted_codes(&descriptor.status_codes),
    );
    render_c_operation_type(&mut out, operations);
    render_c_slot_type(&mut out, &sorted_slots(&descriptor.function_slots));
    out.push_str("typedef uint64_t MermanNativeEngineToken;\n\n");

    for record in &descriptor.records {
        writeln!(out, "typedef struct {} {};", record.c_name, record.c_name).unwrap();
    }
    out.push('\n');

    for callback in &descriptor.callbacks {
        writeln!(
            out,
            "typedef {} (*{})({});",
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
            "typedef {} (*{})({});",
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

    out.push_str(
        "/*\n * The descriptor digest identifies the complete declared wire layout before the function table\n * is returned. All public records are size-tagged; initialize caller-owned records with\n * MERMAN_NATIVE_STRUCT_SIZE(Type). MermanNativeResult is write-only on output: only its\n * struct_size must be initialized before a call, and a returned result must be passed to\n * result_free before that same record is reused. MERMAN_NATIVE_RESULT_INIT is the convenient\n * zeroed initializer.\n *\n * Ownership and concurrency rules from the ABI descriptor:\n",
    );
    let mut ownership_rules = descriptor.ownership_rules.iter().collect::<Vec<_>>();
    ownership_rules.sort_by(|left, right| left.id.cmp(&right.id));
    for rule in ownership_rules {
        render_c_comment_item(&mut out, &rule.id, &rule.description);
    }
    out.push_str(" */\n");
    writeln!(
        out,
        "MermanNativeStatus {}(const MermanNativeApiRequest *request, MermanNativeApi *out_api);",
        descriptor.entry_point.c_name
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
    digest: &str,
    operations: &[ResolvedOperation],
) -> String {
    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-native-abi`.\n// Sources: abi/merman-v3.json and capabilities/feature-surface-v1.json.\n// Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "pub const MERMAN_NATIVE_ABI_VERSION: u32 = {};",
        descriptor.abi_version
    )
    .unwrap();
    render_rust_string_constant(
        &mut out,
        "MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST",
        digest,
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
    render_rust_operation_type(&mut out, operations);
    render_rust_operation_catalog(&mut out, operations);
    render_rust_slot_type(&mut out, &sorted_slots(&descriptor.function_slots));
    out.push_str("pub type MermanNativeEngineToken = u64;\n\n");

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
    out.push('\n');

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
    out.push('\n');
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
         pub struct MermanNativeOperationDescriptor {\n\
         \x20   pub code: MermanNativeOperationCode,\n\
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
         }\n\n",
    );
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

fn render_bindings_operations(operations: &[ResolvedOperation]) -> String {
    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-native-abi`.\n// Sources: abi/merman-v3.json and capabilities/feature-surface-v1.json.\n// Do not edit directly.\n\n",
    );
    out.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub(crate) struct NativeOperationProjection {\n\
         \x20   pub(crate) code: i32,\n\
         \x20   pub(crate) operation_id: Option<&'static str>,\n\
         \x20   pub(crate) capability_id: Option<&'static str>,\n\
         \x20   pub(crate) media_type: Option<&'static str>,\n\
         \x20   pub(crate) requires_uri: bool,\n\
         }\n\n\
         pub(crate) const NATIVE_OPERATIONS: &[NativeOperationProjection] = &[\n",
    );
    for operation in operations {
        writeln!(out, "    NativeOperationProjection {{").unwrap();
        writeln!(out, "        code: {},", operation.code).unwrap();
        writeln!(
            out,
            "        operation_id: {:?},",
            operation.binding_operation_id
        )
        .unwrap();
        writeln!(out, "        capability_id: {:?},", operation.capability_id).unwrap();
        writeln!(out, "        media_type: {:?},", operation.media_type).unwrap();
        writeln!(out, "        requires_uri: {},", operation.requires_uri).unwrap();
        writeln!(out, "    }},").unwrap();
    }
    out.push_str(
        "];\n\n\
         pub(crate) fn native_operation_by_code(\n\
         \x20   code: i32,\n\
         ) -> Option<&'static NativeOperationProjection> {\n\
         \x20   NATIVE_OPERATIONS\n\
         \x20       .iter()\n\
         \x20       .find(|operation| operation.code == code)\n\
         }\n\n\
         pub(crate) fn native_operation_by_id(\n\
         \x20   operation_id: &str,\n\
         ) -> Option<&'static NativeOperationProjection> {\n\
         \x20   NATIVE_OPERATIONS\n\
         \x20       .iter()\n\
         \x20       .find(|operation| operation.operation_id == Some(operation_id))\n\
         }\n",
    );
    out
}

fn generated_artifacts(
    root: &Path,
    descriptor: &NativeAbiDescriptor,
) -> Result<Vec<(PathBuf, String)>, XtaskError> {
    let digest = layout_descriptor_digest(descriptor)?;
    let operations = resolve_operations(root, descriptor)?;
    Ok(vec![
        (
            PathBuf::from(FFI_RUST_OUTPUT),
            render_rust(descriptor, &digest, &operations),
        ),
        (
            PathBuf::from(FFI_HEADER_OUTPUT),
            render_c_header(descriptor, &digest, &operations),
        ),
        (
            PathBuf::from(BINDINGS_OPERATION_OUTPUT),
            render_bindings_operations(&operations),
        ),
    ])
}

fn write_artifact(root: &Path, path: &Path, contents: &str) -> Result<(), XtaskError> {
    let full = root.join(path);
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

fn load_and_validate(root: &Path) -> Result<NativeAbiDescriptor, XtaskError> {
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    resolve_operations(root, &descriptor)?;
    Ok(descriptor)
}

pub(crate) fn gen_native_abi(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    let root = crate::cmd::workspace_root();
    let descriptor = load_and_validate(&root)?;
    for (path, contents) in generated_artifacts(&root, &descriptor)? {
        write_artifact(&root, &path, &contents)?;
    }
    Ok(())
}

pub(crate) fn verify_native_abi_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let descriptor = load_and_validate(&root)?;
    let mut drift = Vec::new();
    for (path, expected) in generated_artifacts(&root, &descriptor)? {
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
    fn descriptor_has_one_abi3_entry_table_and_all_operation_routes() {
        let descriptor = committed_descriptor();
        assert_eq!(descriptor.abi_version, 3);
        assert_eq!(descriptor.result_schema_version, 1);
        assert_eq!(
            descriptor
                .error_kinds
                .iter()
                .map(|kind| kind.json_name.as_str())
                .collect::<BTreeSet<_>>(),
            [
                "generic",
                "missing-capability",
                "reentrant-call",
                "unknown-operation"
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(descriptor.entry_point.c_name, "merman_get_native_api");
        assert_eq!(
            descriptor
                .function_slots
                .iter()
                .map(|slot| slot.id.as_str())
                .collect::<BTreeSet<_>>(),
            [
                "engine_free",
                "engine_new",
                "execute_collect",
                "result_free",
                "runtime_catalog",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(descriptor.operation_codes.len(), 13);
        assert_eq!(
            descriptor
                .operation_codes
                .iter()
                .map(|operation| operation.code)
                .collect::<BTreeSet<_>>(),
            (0..13).collect()
        );
        let root = crate::cmd::workspace_root();
        let operations = resolve_operations(&root, &descriptor).unwrap();
        let uri_operations = operations
            .iter()
            .filter(|operation| operation.requires_uri)
            .map(|operation| operation.binding_operation_id.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(
            uri_operations,
            vec![
                Some("document-analysis-json"),
                Some("document-analysis-facts-json"),
            ]
        );
        assert!(
            operations
                .iter()
                .find(|operation| operation.id == "none")
                .is_some_and(|operation| !operation.requires_uri)
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
        descriptor.operation_codes[1].id = "missing_operation".to_string();
        let root = crate::cmd::workspace_root();
        assert!(resolve_operations(&root, &descriptor).is_err());
    }

    #[test]
    fn descriptor_rejects_error_kind_name_swaps() {
        let mut descriptor = committed_descriptor();
        descriptor.error_kinds[0].json_name = "unknown-operation".to_string();
        descriptor.error_kinds[1].json_name = "generic".to_string();

        assert!(validate_descriptor(&descriptor).is_err());
    }

    #[test]
    fn layout_digest_tracks_wire_structure_not_explanatory_prose() {
        let descriptor = committed_descriptor();
        let original = layout_descriptor_digest(&descriptor).unwrap();

        let mut prose_only = descriptor.clone();
        prose_only.records[0].description = "Reworded documentation only.".to_string();
        prose_only.records[0].fields[0].ownership = "Also documentation only.".to_string();
        assert_eq!(layout_descriptor_digest(&prose_only).unwrap(), original);

        let mut structural = descriptor;
        structural.records[0].fields[1].rust_type = "*mut u8".to_string();
        assert_ne!(layout_descriptor_digest(&structural).unwrap(), original);

        let mut result_contract = committed_descriptor();
        result_contract.result_schema_version += 1;
        assert_ne!(
            layout_descriptor_digest(&result_contract).unwrap(),
            original
        );

        let mut error_contract = committed_descriptor();
        error_contract.error_kinds[0].json_name = "renamed-generic".to_string();
        assert_ne!(layout_descriptor_digest(&error_contract).unwrap(), original);
    }

    #[test]
    fn generated_header_has_only_the_abi3_entry_and_pointer_callbacks() {
        let descriptor = committed_descriptor();
        let digest = layout_descriptor_digest(&descriptor).unwrap();
        let root = crate::cmd::workspace_root();
        let operations = resolve_operations(&root, &descriptor).unwrap();
        let header = render_c_header(&descriptor, &digest, &operations);
        let rust = render_rust(&descriptor, &digest, &operations);
        let operation_projection = render_bindings_operations(&operations);
        assert!(header.contains("MERMAN_NATIVE_ABI_VERSION 3u"));
        assert!(header.contains("MERMAN_NATIVE_RESULT_SCHEMA_VERSION 1u"));
        assert!(header.contains("MERMAN_NATIVE_ERROR_KIND_GENERIC \"generic\""));
        assert!(
            header.contains("MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY \"missing-capability\"")
        );
        assert!(
            header.contains("MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION \"unknown-operation\"")
        );
        assert!(header.contains("MERMAN_NATIVE_ERROR_KIND_REENTRANT_CALL \"reentrant-call\""));
        assert!(header.contains("merman_get_native_api("));
        assert!(header.contains("MermanNativeTextMeasureRequest *request"));
        assert!(header.contains("MermanNativeTextMeasureResult *out_result"));
        assert!(!header.contains("MermanNativeChunkSink"));
        assert!(!header.contains("MermanNativeLayoutProbeFn"));
        assert!(!header.contains("format_options_json"));
        assert!(header.contains("MermanNativeSlice options_json;"));
        assert!(header.contains("MERMAN_NATIVE_OPERATION_ID_ANALYSIS_FACTS_JSON"));
        assert!(header.contains("MERMAN_NATIVE_OPERATION_REQUIRES_URI_DOCUMENT_ANALYSIS_JSON 1"));
        assert!(operation_projection.contains("semantic-json"));
        assert!(operation_projection.contains("analysis-facts-json"));
        assert!(operation_projection.contains("requires_uri: true"));
        assert!(operation_projection.contains("native_operation_by_code"));
        assert!(rust.contains("#[repr(C)]\npub struct MermanNativeResult"));
        assert!(
            !rust.contains("#[repr(C)]\n#[derive(Clone, Copy)]\npub struct MermanNativeResult")
        );
        assert!(rust.contains("#[derive(Clone, Copy)]\npub struct MermanNativeSlice"));
        assert!(!header.contains("merman_render_svg("));
        assert!(!header.contains("MermanEngineResult"));
        assert!(!header.contains("MERMAN_ABI_VERSION"));
    }

    #[test]
    fn generated_artifacts_are_deterministic_and_committed() {
        let root = crate::cmd::workspace_root();
        let descriptor = load_and_validate(&root).unwrap();
        assert_eq!(
            generated_artifacts(&root, &descriptor).unwrap(),
            generated_artifacts(&root, &descriptor).unwrap()
        );
    }
}
