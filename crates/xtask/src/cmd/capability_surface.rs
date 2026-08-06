//! Canonical capability-surface descriptor validation and projection.

use crate::XtaskError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const DESCRIPTOR_PATH: &str = "capabilities/feature-surface-v1.json";
const DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
const GENERATED_OUTPUTS: &[(&str, ArtifactKind)] = &[
    (
        "crates/merman-bindings-core/src/generated/capability_surface.rs",
        ArtifactKind::Rust,
    ),
    (
        "crates/merman-cli/src/generated/capability_surface.rs",
        ArtifactKind::Rust,
    ),
    (
        "capabilities/generated/capability-surface.ts",
        ArtifactKind::TypeScript,
    ),
    (
        "platforms/node/src/generated/capability-surface.mjs",
        ArtifactKind::NodeJavaScript,
    ),
    (
        "platforms/web/src/generated/capability-surface.ts",
        ArtifactKind::WebTypeScript,
    ),
    (
        "capabilities/generated/merman_capability_surface.h",
        ArtifactKind::CHeader,
    ),
    (
        "capabilities/generated/feature-surface-v1.md",
        ArtifactKind::Markdown,
    ),
];

const INCIDENTAL_DEPENDENCY_NAMES: &[&str] = &[
    "jiff",
    "krilla",
    "pulldown-cmark",
    "ratex",
    "resvg",
    "serde",
    "serde-json",
    "tokio",
    "usvg",
    "uuid",
    "wasm-bindgen",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CapabilitySurfaceDescriptor {
    schema_version: u32,
    descriptor_id: String,
    targets: Vec<TargetDescriptor>,
    capabilities: Vec<CapabilityDescriptor>,
    outputs: Vec<OutputDescriptor>,
    binding_operations: Vec<BindingOperationDescriptor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TargetDescriptor {
    id: String,
    description: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum CapabilityKind {
    Api,
    Output,
    Engine,
    Adapter,
    Tool,
}

impl CapabilityKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Output => "output",
            Self::Engine => "engine",
            Self::Adapter => "adapter",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CapabilityDescriptor {
    id: String,
    kind: CapabilityKind,
    description: String,
    targets: Vec<String>,
    implications: Vec<String>,
    absence: AbsenceContract,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AbsenceContract {
    error_id: String,
    contract: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OutputDescriptor {
    id: String,
    capability: String,
    description: String,
    media_type: String,
    targets: Vec<String>,
}

/// A required JSON field whose value may be either a stable descriptor ID or explicit `null`.
///
/// Keeping this as a transparent wrapper rather than a bare `Option<String>` makes omission a
/// descriptor-schema error. Callers must distinguish an explicit absence from an accidentally
/// omitted relationship.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
struct RequiredNullableId(Option<String>);

impl<'de> Deserialize<'de> for RequiredNullableId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RequiredNullableIdVisitor;

        impl serde::de::Visitor<'_> for RequiredNullableIdVisitor {
            type Value = RequiredNullableId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a stable descriptor ID string or explicit null")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableId(None))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableId(None))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableId(Some(value.to_owned())))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableId(Some(value)))
            }
        }

        deserializer.deserialize_any(RequiredNullableIdVisitor)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BindingOperationDescriptor {
    id: String,
    capability: RequiredNullableId,
    output: RequiredNullableId,
    compiled_prerequisites: Vec<String>,
    description: String,
    media_type: String,
    requires_uri: bool,
    targets: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum ArtifactKind {
    Rust,
    TypeScript,
    NodeJavaScript,
    WebTypeScript,
    CHeader,
    Markdown,
}

impl ArtifactKind {
    fn render(
        self,
        descriptor: &CapabilitySurfaceDescriptor,
        digest: &str,
    ) -> Result<String, String> {
        match self {
            Self::Rust => render_rust(descriptor, digest),
            Self::TypeScript => render_typescript(descriptor, digest),
            Self::NodeJavaScript => render_node_javascript(descriptor, digest),
            Self::WebTypeScript => render_web_typescript(descriptor, digest),
            Self::CHeader => render_c_header(descriptor, digest),
            Self::Markdown => render_markdown(descriptor, digest),
        }
    }
}

fn surface_error(message: impl Into<String>) -> XtaskError {
    XtaskError::CapabilitySurface(message.into())
}

fn require_non_empty(value: &str, path: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{path}: must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_kebab_id(value: &str, path: &str) -> Result<(), String> {
    require_non_empty(value, path)?;
    let bytes = value.as_bytes();
    if !bytes[0].is_ascii_lowercase()
        || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
        || bytes.windows(2).any(|pair| pair == b"--")
        || bytes
            .iter()
            .any(|byte| !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && *byte != b'-')
    {
        return Err(format!(
            "{path}: `{value}` must be a lowercase kebab-case ID"
        ));
    }
    Ok(())
}

fn validate_unique_ids<'a>(
    values: impl IntoIterator<Item = (usize, &'a str)>,
    collection_path: &str,
) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    for (index, id) in values {
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{collection_path}[{index}].id: duplicate ID `{id}`"
            ));
        }
    }
    Ok(ids)
}

fn validate_string_set(values: &[String], path: &str) -> Result<BTreeSet<String>, String> {
    let mut result = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !result.insert(value.clone()) {
            return Err(format!("{path}[{index}]: duplicate ID `{value}`"));
        }
    }
    Ok(result)
}

fn validate_sorted_string_set(values: &[String], path: &str) -> Result<BTreeSet<String>, String> {
    let result = validate_string_set(values, path)?;
    if values.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(format!("{path}: IDs must be sorted lexicographically"));
    }
    Ok(result)
}

fn is_negative_feature_name(id: &str) -> bool {
    id.starts_with("no-")
        || id.starts_with("without-")
        || id.contains("-no-")
        || id.ends_with("-disabled")
}

fn is_diagram_specific_feature_name(id: &str) -> bool {
    merman_core::diagram_family_capabilities()
        .iter()
        .any(|fact| {
            fact.diagram_type == id
                || fact.logical_family_kind == id
                || fact.metadata_id == Some(id)
                || fact.render_model_kind == Some(id)
        })
}

fn validate_targets(
    values: &[String],
    path: &str,
    known_targets: &BTreeSet<String>,
) -> Result<BTreeSet<String>, String> {
    if values.is_empty() {
        return Err(format!("{path}: must name at least one target"));
    }
    let targets = validate_string_set(values, path)?;
    for (index, target) in values.iter().enumerate() {
        if !known_targets.contains(target) {
            return Err(format!("{path}[{index}]: unknown target `{target}`"));
        }
    }
    Ok(targets)
}

fn validate_descriptor(descriptor: &CapabilitySurfaceDescriptor) -> Result<(), String> {
    if descriptor.schema_version != DESCRIPTOR_SCHEMA_VERSION {
        return Err(format!(
            "schema_version: expected {DESCRIPTOR_SCHEMA_VERSION}, found {}",
            descriptor.schema_version
        ));
    }
    validate_kebab_id(&descriptor.descriptor_id, "descriptor_id")?;

    let target_ids = validate_unique_ids(
        descriptor
            .targets
            .iter()
            .enumerate()
            .map(|(index, target)| (index, target.id.as_str())),
        "targets",
    )?;
    if target_ids.is_empty() {
        return Err("targets: must not be empty".to_string());
    }
    for (index, target) in descriptor.targets.iter().enumerate() {
        validate_kebab_id(&target.id, &format!("targets[{index}].id"))?;
        require_non_empty(
            &target.description,
            &format!("targets[{index}].description"),
        )?;
    }

    validate_unique_ids(
        descriptor
            .capabilities
            .iter()
            .enumerate()
            .map(|(index, capability)| (index, capability.id.as_str())),
        "capabilities",
    )?;
    let capability_by_id = descriptor
        .capabilities
        .iter()
        .map(|capability| (capability.id.as_str(), capability))
        .collect::<BTreeMap<_, _>>();

    for (index, capability) in descriptor.capabilities.iter().enumerate() {
        let base = format!("capabilities[{index}]");
        validate_kebab_id(&capability.id, &format!("{base}.id"))?;
        if capability.id.starts_with("preset-") {
            return Err(format!(
                "{base}.id: public leaf `{}` must not use the preset namespace",
                capability.id
            ));
        }
        if is_negative_feature_name(&capability.id) {
            return Err(format!(
                "{base}.id: negative public feature `{}` is forbidden",
                capability.id
            ));
        }
        if is_diagram_specific_feature_name(&capability.id) {
            return Err(format!(
                "{base}.id: diagram-specific public feature `{}` is forbidden",
                capability.id
            ));
        }
        if INCIDENTAL_DEPENDENCY_NAMES.contains(&capability.id.as_str()) {
            return Err(format!(
                "{base}.id: incidental dependency-named public feature `{}` is forbidden",
                capability.id
            ));
        }
        require_non_empty(&capability.description, &format!("{base}.description"))?;
        validate_targets(&capability.targets, &format!("{base}.targets"), &target_ids)?;
        validate_string_set(&capability.implications, &format!("{base}.implications"))?;
        require_non_empty(
            &capability.absence.error_id,
            &format!("{base}.absence.error_id"),
        )?;
        require_non_empty(
            &capability.absence.contract,
            &format!("{base}.absence.contract"),
        )?;
    }

    for (capability_index, capability) in descriptor.capabilities.iter().enumerate() {
        for (implication_index, implication) in capability.implications.iter().enumerate() {
            let Some(implied) = capability_by_id.get(implication.as_str()) else {
                return Err(format!(
                    "capabilities[{capability_index}].implications[{implication_index}]: unknown capability `{implication}`"
                ));
            };
            for target in &capability.targets {
                if !implied.targets.contains(target) {
                    return Err(format!(
                        "capabilities[{capability_index}].implications[{implication_index}]: `{implication}` is unavailable on target `{target}`"
                    ));
                }
            }
        }
    }
    validate_implication_cycles(descriptor, &capability_by_id)?;

    validate_unique_ids(
        descriptor
            .outputs
            .iter()
            .enumerate()
            .map(|(index, output)| (index, output.id.as_str())),
        "outputs",
    )?;
    for (index, output) in descriptor.outputs.iter().enumerate() {
        let base = format!("outputs[{index}]");
        validate_kebab_id(&output.id, &format!("{base}.id"))?;
        require_non_empty(&output.description, &format!("{base}.description"))?;
        require_non_empty(&output.media_type, &format!("{base}.media_type"))?;
        let Some(capability) = capability_by_id.get(output.capability.as_str()) else {
            return Err(format!(
                "{base}.capability: unknown capability `{}`",
                output.capability
            ));
        };
        if capability.kind != CapabilityKind::Output {
            return Err(format!(
                "{base}.capability: `{}` is not an output capability",
                output.capability
            ));
        }
        if output.id != output.capability {
            return Err(format!(
                "{base}.id: output ID `{}` must equal capability ID `{}`",
                output.id, output.capability
            ));
        }
        let output_targets =
            validate_targets(&output.targets, &format!("{base}.targets"), &target_ids)?;
        let capability_targets = capability.targets.iter().cloned().collect::<BTreeSet<_>>();
        if output_targets != capability_targets {
            return Err(format!(
                "{base}.targets: must exactly equal capability `{}` targets",
                output.capability
            ));
        }
    }
    for capability in descriptor
        .capabilities
        .iter()
        .filter(|capability| capability.kind == CapabilityKind::Output)
    {
        let descriptor_count = descriptor
            .outputs
            .iter()
            .filter(|output| output.capability == capability.id)
            .count();
        if descriptor_count != 1 {
            return Err(format!(
                "capabilities[id={}]: output capability must have exactly one output descriptor; found {descriptor_count}",
                capability.id
            ));
        }
    }
    let output_by_id = descriptor
        .outputs
        .iter()
        .map(|output| (output.id.as_str(), output))
        .collect::<BTreeMap<_, _>>();

    validate_unique_ids(
        descriptor
            .binding_operations
            .iter()
            .enumerate()
            .map(|(index, operation)| (index, operation.id.as_str())),
        "binding_operations",
    )?;
    let mut output_owner_counts = BTreeMap::<&str, usize>::new();
    for (index, operation) in descriptor.binding_operations.iter().enumerate() {
        let base = format!("binding_operations[{index}]");
        validate_kebab_id(&operation.id, &format!("{base}.id"))?;
        require_non_empty(&operation.description, &format!("{base}.description"))?;
        require_non_empty(&operation.media_type, &format!("{base}.media_type"))?;
        let operation_targets =
            validate_targets(&operation.targets, &format!("{base}.targets"), &target_ids)?;
        let primary_capability = if let Some(capability_id) = operation.capability.0.as_deref() {
            let Some(capability) = capability_by_id.get(capability_id) else {
                return Err(format!(
                    "{base}.capability: unknown capability `{capability_id}`"
                ));
            };
            if !matches!(
                capability.kind,
                CapabilityKind::Api | CapabilityKind::Output
            ) {
                return Err(format!(
                    "{base}.capability: `{capability_id}` must be an API or output capability, not `{}`",
                    capability.kind.as_str()
                ));
            }
            for target in &operation_targets {
                if !capability.targets.contains(target) {
                    return Err(format!(
                        "{base}.targets: capability `{capability_id}` is unavailable on target `{target}`"
                    ));
                }
            }
            Some(*capability)
        } else {
            None
        };

        let compiled_prerequisites = validate_sorted_string_set(
            &operation.compiled_prerequisites,
            &format!("{base}.compiled_prerequisites"),
        )?;
        for (required_index, required_id) in operation.compiled_prerequisites.iter().enumerate() {
            if operation.capability.0.as_deref() == Some(required_id.as_str()) {
                return Err(format!(
                    "{base}.compiled_prerequisites[{required_index}]: availability capability `{required_id}` must not be repeated"
                ));
            }
            let Some(required) = capability_by_id.get(required_id.as_str()) else {
                return Err(format!(
                    "{base}.compiled_prerequisites[{required_index}]: unknown capability `{required_id}`"
                ));
            };
            if !matches!(
                required.kind,
                CapabilityKind::Api | CapabilityKind::Output | CapabilityKind::Engine
            ) {
                return Err(format!(
                    "{base}.compiled_prerequisites[{required_index}]: `{required_id}` must be an API, output, or engine capability, not `{}`",
                    required.kind.as_str()
                ));
            }
            for target in &operation_targets {
                if !required.targets.contains(target) {
                    return Err(format!(
                        "{base}.compiled_prerequisites[{required_index}]: capability `{required_id}` is unavailable on target `{target}`"
                    ));
                }
            }
        }
        debug_assert_eq!(
            compiled_prerequisites.len(),
            operation.compiled_prerequisites.len()
        );

        let Some(output_id) = operation.output.0.as_deref() else {
            continue;
        };
        let Some(output) = output_by_id.get(output_id) else {
            return Err(format!("{base}.output: unknown output `{output_id}`"));
        };
        *output_owner_counts.entry(output_id).or_default() += 1;
        if primary_capability.map(|capability| capability.id.as_str())
            != Some(output.capability.as_str())
        {
            return Err(format!(
                "{base}.capability: must match output `{}` capability `{}`",
                output.id, output.capability
            ));
        }
        if operation.media_type != output.media_type {
            return Err(format!(
                "{base}.media_type: must match output `{}` media type `{}`",
                output.id, output.media_type
            ));
        }
        if operation.requires_uri {
            return Err(format!(
                "{base}.requires_uri: output `{}` must not require a URI",
                output.id
            ));
        }
        let operation_targets = operation.targets.iter().cloned().collect::<BTreeSet<_>>();
        let output_targets = output.targets.iter().cloned().collect::<BTreeSet<_>>();
        if operation_targets != output_targets {
            return Err(format!(
                "{base}.targets: must match output `{}` targets",
                output.id
            ));
        }
    }

    for output in &descriptor.outputs {
        let owner_count = output_owner_counts
            .get(output.id.as_str())
            .copied()
            .unwrap_or(0);
        if owner_count != 1 {
            return Err(format!(
                "outputs[id={}]: must be referenced by exactly one binding operation; found {owner_count}",
                output.id
            ));
        }
    }

    Ok(())
}

fn validate_implication_cycles(
    descriptor: &CapabilitySurfaceDescriptor,
    capability_by_id: &BTreeMap<&str, &CapabilityDescriptor>,
) -> Result<(), String> {
    fn visit<'a>(
        id: &'a str,
        capability_by_id: &BTreeMap<&'a str, &'a CapabilityDescriptor>,
        visiting: &mut Vec<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> Result<(), String> {
        if let Some(start) = visiting.iter().position(|candidate| *candidate == id) {
            let mut cycle = visiting[start..].to_vec();
            cycle.push(id);
            return Err(format!(
                "capabilities[id={id}].implications: implication cycle: {}",
                cycle.join(" -> ")
            ));
        }
        if visited.contains(id) {
            return Ok(());
        }
        visiting.push(id);
        for implication in &capability_by_id[id].implications {
            visit(implication, capability_by_id, visiting, visited)?;
        }
        visiting.pop();
        visited.insert(id);
        Ok(())
    }

    let mut visited = BTreeSet::new();
    for capability in &descriptor.capabilities {
        visit(
            &capability.id,
            capability_by_id,
            &mut Vec::new(),
            &mut visited,
        )?;
    }
    Ok(())
}

fn read_descriptor(path: &Path) -> Result<CapabilitySurfaceDescriptor, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let descriptor =
        serde_json::from_str::<CapabilitySurfaceDescriptor>(&text).map_err(|error| {
            surface_error(format!(
                "{}: descriptor schema error: {error}",
                path.display()
            ))
        })?;
    validate_descriptor(&descriptor)
        .map_err(|error| surface_error(format!("{}: {error}", path.display())))?;
    Ok(descriptor)
}

fn semantic_digest(descriptor: &CapabilitySurfaceDescriptor) -> Result<String, String> {
    let mut canonical = descriptor.clone();
    canonical
        .targets
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .capabilities
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .outputs
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .binding_operations
        .sort_by(|left, right| left.id.cmp(&right.id));
    for capability in &mut canonical.capabilities {
        capability.targets.sort();
        capability.implications.sort();
    }
    for output in &mut canonical.outputs {
        output.targets.sort();
    }
    for operation in &mut canonical.binding_operations {
        operation.targets.sort();
        operation.compiled_prerequisites.sort();
    }
    let bytes = serde_json::to_vec(&canonical).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{}", crate::util::sha256_hex(&bytes)))
}

fn sorted_targets(descriptor: &CapabilitySurfaceDescriptor) -> Vec<&TargetDescriptor> {
    let mut values = descriptor.targets.iter().collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}

fn sorted_capabilities(descriptor: &CapabilitySurfaceDescriptor) -> Vec<&CapabilityDescriptor> {
    let mut values = descriptor.capabilities.iter().collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}

fn sorted_outputs(descriptor: &CapabilitySurfaceDescriptor) -> Vec<&OutputDescriptor> {
    let mut values = descriptor.outputs.iter().collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}

fn sorted_binding_operations(
    descriptor: &CapabilitySurfaceDescriptor,
) -> Vec<&BindingOperationDescriptor> {
    let mut values = descriptor.binding_operations.iter().collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}

fn sorted_string_refs(values: &[String]) -> Vec<&str> {
    let mut values = values.iter().map(String::as_str).collect::<Vec<_>>();
    values.sort_unstable();
    values
}

fn rust_operation_variant(id: &str) -> String {
    id.split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn write_rust_key_enum(
    out: &mut String,
    enum_name: &str,
    ids: &[&str],
    spec_type: &str,
    specs: &str,
) {
    writeln!(
        out,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\n#[non_exhaustive]\npub enum {enum_name} {{"
    )
    .unwrap();
    for id in ids {
        writeln!(out, "    {},", rust_operation_variant(id)).unwrap();
    }
    out.push_str("}\n\n");

    writeln!(
        out,
        "impl {enum_name} {{\n    pub const ALL: &'static [Self] = &["
    )
    .unwrap();
    for id in ids {
        writeln!(out, "        Self::{},", rust_operation_variant(id)).unwrap();
    }
    out.push_str("    ];\n\n    pub fn from_id(id: &str) -> Option<Self> {\n        match id {\n");
    for id in ids {
        writeln!(
            out,
            "            {id:?} => Some(Self::{}),",
            rust_operation_variant(id)
        )
        .unwrap();
    }
    out.push_str("            _ => None,\n        }\n    }\n\n");
    writeln!(
        out,
        "    pub const fn id(self) -> &'static str {{\n        self.spec().id\n    }}\n\n    pub const fn spec(self) -> &'static {spec_type} {{\n        match self {{"
    )
    .unwrap();
    for (index, id) in ids.iter().enumerate() {
        writeln!(
            out,
            "            Self::{} => &{specs}[{index}],",
            rust_operation_variant(id)
        )
        .unwrap();
    }
    out.push_str("        }\n    }\n}\n\n");
}

fn render_rust(descriptor: &CapabilitySurfaceDescriptor, digest: &str) -> Result<String, String> {
    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-capability-surface`.\n// Source: capabilities/feature-surface-v1.json. Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "pub const CAPABILITY_DESCRIPTOR_SCHEMA_VERSION: u32 = {};",
        descriptor.schema_version
    )
    .unwrap();
    writeln!(
        out,
        "pub const CAPABILITY_DESCRIPTOR_DIGEST: &str = {digest:?};\n"
    )
    .unwrap();

    out.push_str("pub const TARGET_IDS: &[&str] = &[\n");
    for target in sorted_targets(descriptor) {
        writeln!(out, "    {:?},", target.id).unwrap();
    }
    out.push_str("];\n\npub const CAPABILITY_IDS: &[&str] = &[\n");
    for capability in sorted_capabilities(descriptor) {
        writeln!(out, "    {:?},", capability.id).unwrap();
    }
    out.push_str("];\n\npub const OUTPUT_IDS: &[&str] = &[\n");
    for output in sorted_outputs(descriptor) {
        writeln!(out, "    {:?},", output.id).unwrap();
    }
    out.push_str("];\n\npub const BINDING_OPERATION_IDS: &[&str] = &[\n");
    for operation in sorted_binding_operations(descriptor) {
        writeln!(out, "    {:?},", operation.id).unwrap();
    }
    out.push_str("];\n\n");

    let targets = sorted_targets(descriptor);
    let capabilities = sorted_capabilities(descriptor);
    let outputs = sorted_outputs(descriptor);
    let operations = sorted_binding_operations(descriptor);
    let target_ids = targets
        .iter()
        .map(|target| target.id.as_str())
        .collect::<Vec<_>>();
    let capability_ids = capabilities
        .iter()
        .map(|capability| capability.id.as_str())
        .collect::<Vec<_>>();
    let output_ids = outputs
        .iter()
        .map(|output| output.id.as_str())
        .collect::<Vec<_>>();
    let operation_ids = operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect::<Vec<_>>();
    write_rust_key_enum(
        &mut out,
        "TargetKey",
        &target_ids,
        "TargetDescriptor",
        "TARGETS",
    );
    write_rust_key_enum(
        &mut out,
        "CapabilityKey",
        &capability_ids,
        "CapabilityDescriptor",
        "CAPABILITIES",
    );
    write_rust_key_enum(
        &mut out,
        "OutputKey",
        &output_ids,
        "OutputDescriptor",
        "OUTPUTS",
    );
    write_rust_key_enum(
        &mut out,
        "OperationKey",
        &operation_ids,
        "OperationSpec",
        "OPERATION_SPECS",
    );

    out.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         #[non_exhaustive]\n\
         pub struct OperationSpec {\n\
         \x20   pub key: OperationKey,\n\
         \x20   pub id: &'static str,\n\
         \x20   pub capability: Option<CapabilityKey>,\n\
         \x20   pub output: Option<OutputKey>,\n\
         \x20   pub compiled_prerequisites: &'static [CapabilityKey],\n\
         \x20   pub description: &'static str,\n\
         \x20   pub media_type: &'static str,\n\
         \x20   pub requires_uri: bool,\n\
         \x20   pub targets: &'static [TargetKey],\n\
         }\n\n",
    );

    out.push_str("pub const OPERATION_SPECS: &[OperationSpec] = &[\n");
    for operation in &operations {
        out.push_str("    OperationSpec {\n");
        writeln!(
            out,
            "        key: OperationKey::{},",
            rust_operation_variant(&operation.id)
        )
        .unwrap();
        writeln!(out, "        id: {:?},", operation.id).unwrap();
        match operation.capability.0.as_deref() {
            Some(capability) => writeln!(
                out,
                "        capability: Some(CapabilityKey::{}),",
                rust_operation_variant(capability)
            )
            .unwrap(),
            None => out.push_str("        capability: None,\n"),
        }
        match operation.output.0.as_deref() {
            Some(output) => writeln!(
                out,
                "        output: Some(OutputKey::{}),",
                rust_operation_variant(output)
            )
            .unwrap(),
            None => out.push_str("        output: None,\n"),
        }
        out.push_str("        compiled_prerequisites: &[");
        for capability in sorted_string_refs(&operation.compiled_prerequisites) {
            write!(
                out,
                "CapabilityKey::{}, ",
                rust_operation_variant(capability)
            )
            .unwrap();
        }
        out.push_str("],\n");
        writeln!(out, "        description: {:?},", operation.description).unwrap();
        writeln!(out, "        media_type: {:?},", operation.media_type).unwrap();
        writeln!(out, "        requires_uri: {},", operation.requires_uri).unwrap();
        out.push_str("        targets: &[");
        for target in sorted_string_refs(&operation.targets) {
            write!(out, "TargetKey::{}, ", rust_operation_variant(target)).unwrap();
        }
        out.push_str("],\n    },\n");
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[non_exhaustive]\npub struct TargetDescriptor {\n    pub key: TargetKey,\n    pub id: &'static str,\n    pub description: &'static str,\n}\n\npub const TARGETS: &[TargetDescriptor] = &[\n");
    for target in targets {
        writeln!(
            out,
            "    TargetDescriptor {{ key: TargetKey::{}, id: {:?}, description: {:?} }},",
            rust_operation_variant(&target.id),
            target.id,
            target.description
        )
        .unwrap();
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[non_exhaustive]\npub struct CapabilityDescriptor {\n    pub key: CapabilityKey,\n    pub id: &'static str,\n    pub kind: &'static str,\n    pub description: &'static str,\n    pub targets: &'static [TargetKey],\n    pub implications: &'static [CapabilityKey],\n}\n\npub const CAPABILITIES: &[CapabilityDescriptor] = &[\n");
    for capability in capabilities {
        out.push_str("    CapabilityDescriptor {\n");
        writeln!(
            out,
            "        key: CapabilityKey::{},",
            rust_operation_variant(&capability.id)
        )
        .unwrap();
        writeln!(out, "        id: {:?},", capability.id).unwrap();
        writeln!(out, "        kind: {:?},", capability.kind.as_str()).unwrap();
        writeln!(out, "        description: {:?},", capability.description).unwrap();
        out.push_str("        targets: &[");
        for target in sorted_string_refs(&capability.targets) {
            write!(out, "TargetKey::{}, ", rust_operation_variant(target)).unwrap();
        }
        out.push_str("],\n        implications: &[");
        for implication in sorted_string_refs(&capability.implications) {
            write!(
                out,
                "CapabilityKey::{}, ",
                rust_operation_variant(implication)
            )
            .unwrap();
        }
        out.push_str("],\n    },\n");
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[non_exhaustive]\npub struct OutputDescriptor {\n    pub key: OutputKey,\n    pub id: &'static str,\n    pub capability: CapabilityKey,\n    pub description: &'static str,\n    pub media_type: &'static str,\n    pub targets: &'static [TargetKey],\n}\n\npub const OUTPUTS: &[OutputDescriptor] = &[\n");
    for output in outputs {
        out.push_str("    OutputDescriptor {\n");
        writeln!(
            out,
            "        key: OutputKey::{},",
            rust_operation_variant(&output.id)
        )
        .unwrap();
        writeln!(out, "        id: {:?},", output.id).unwrap();
        writeln!(
            out,
            "        capability: CapabilityKey::{},",
            rust_operation_variant(&output.capability)
        )
        .unwrap();
        writeln!(out, "        description: {:?},", output.description).unwrap();
        writeln!(out, "        media_type: {:?},", output.media_type).unwrap();
        out.push_str("        targets: &[");
        for target in sorted_string_refs(&output.targets) {
            write!(out, "TargetKey::{}, ", rust_operation_variant(target)).unwrap();
        }
        out.push_str("],\n    },\n");
    }
    out.push_str("];\n\n");

    out.push_str(
        "pub type BindingOperationDescriptor = OperationSpec;\n\
         pub const BINDING_OPERATIONS: &[BindingOperationDescriptor] = OPERATION_SPECS;\n\n",
    );

    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    Ok(out)
}

fn write_typescript_value(
    out: &mut String,
    name: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let value = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    writeln!(out, "export const {name} = {value} as const;\n").unwrap();
    Ok(())
}

fn render_typescript(
    descriptor: &CapabilitySurfaceDescriptor,
    digest: &str,
) -> Result<String, String> {
    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-capability-surface`.\n// Source: capabilities/feature-surface-v1.json. Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "export const CAPABILITY_DESCRIPTOR_SCHEMA_VERSION = {} as const;",
        descriptor.schema_version
    )
    .unwrap();
    writeln!(
        out,
        "export const CAPABILITY_DESCRIPTOR_DIGEST = {digest:?} as const;\n"
    )
    .unwrap();

    write_typescript_value(
        &mut out,
        "TARGETS",
        serde_json::json!(
            sorted_targets(descriptor)
                .into_iter()
                .map(|target| serde_json::json!({
                    "id": target.id,
                    "description": target.description,
                }))
                .collect::<Vec<_>>()
        ),
    )?;
    write_typescript_value(
        &mut out,
        "CAPABILITIES",
        serde_json::json!(
            sorted_capabilities(descriptor)
                .into_iter()
                .map(|capability| serde_json::json!({
                    "id": capability.id,
                    "kind": capability.kind.as_str(),
                    "description": capability.description,
                    "targets": sorted_string_refs(&capability.targets),
                    "implications": sorted_string_refs(&capability.implications),
                }))
                .collect::<Vec<_>>()
        ),
    )?;
    write_typescript_value(
        &mut out,
        "OUTPUTS",
        serde_json::json!(
            sorted_outputs(descriptor)
                .into_iter()
                .map(|output| serde_json::json!({
                    "id": output.id,
                    "capability": output.capability,
                    "description": output.description,
                    "media_type": output.media_type,
                    "targets": sorted_string_refs(&output.targets),
                }))
                .collect::<Vec<_>>()
        ),
    )?;
    write_typescript_value(
        &mut out,
        "BINDING_OPERATIONS",
        serde_json::json!(
            sorted_binding_operations(descriptor)
                .into_iter()
                .map(|operation| serde_json::json!({
                    "id": operation.id,
                    "capability": operation.capability.0.as_deref(),
                    "output": operation.output.0.as_deref(),
                    "compiled_prerequisites": sorted_string_refs(&operation.compiled_prerequisites),
                    "description": operation.description,
                    "media_type": operation.media_type,
                    "requires_uri": operation.requires_uri,
                    "targets": sorted_string_refs(&operation.targets),
                }))
                .collect::<Vec<_>>()
        ),
    )?;
    for (name, ids, type_name) in [
        (
            "TARGET_IDS",
            sorted_targets(descriptor)
                .into_iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
            "TargetId",
        ),
        (
            "CAPABILITY_IDS",
            sorted_capabilities(descriptor)
                .into_iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
            "CapabilityId",
        ),
        (
            "OUTPUT_IDS",
            sorted_outputs(descriptor)
                .into_iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
            "OutputId",
        ),
        (
            "BINDING_OPERATION_IDS",
            sorted_binding_operations(descriptor)
                .into_iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
            "BindingOperationId",
        ),
    ] {
        write_typescript_value(&mut out, name, serde_json::json!(ids))?;
        writeln!(out, "export type {type_name} = (typeof {name})[number];\n").unwrap();
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    Ok(out)
}

/// Emit the private Node facade projection used to validate compiled operation prerequisites.
///
/// Keep this projection narrow so the loader package does not ship the complete descriptor solely
/// to recover one relationship. The canonical descriptor remains the sole owner of the operation
/// IDs and their compiled capability closure.
fn render_node_javascript(
    descriptor: &CapabilitySurfaceDescriptor,
    digest: &str,
) -> Result<String, String> {
    let operations = serde_json::json!(
        sorted_binding_operations(descriptor)
            .into_iter()
            .map(|operation| serde_json::json!({
                "id": operation.id,
                "compiled_prerequisites": sorted_string_refs(&operation.compiled_prerequisites),
            }))
            .collect::<Vec<_>>()
    );
    let operations =
        serde_json::to_string_pretty(&operations).map_err(|error| error.to_string())?;
    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-capability-surface`.\n// Source: capabilities/feature-surface-v1.json. Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "export const CAPABILITY_DESCRIPTOR_SCHEMA_VERSION = {};",
        descriptor.schema_version
    )
    .unwrap();
    writeln!(
        out,
        "export const CAPABILITY_DESCRIPTOR_DIGEST = {digest:?};\n"
    )
    .unwrap();
    writeln!(out, "export const NODE_BINDING_OPERATIONS = {operations};").unwrap();
    Ok(out)
}

/// Emit the browser-local projection used by the published Web package.
///
/// The package has a separate TypeScript compilation root, so it cannot import the workspace-wide
/// projection directly without publishing unrelated repository paths. Keep this projection narrow:
/// it contains only the vocabulary needed to validate browser runtime reports, while the canonical
/// descriptor remains the sole source of those IDs and relationships.
fn render_web_typescript(
    descriptor: &CapabilitySurfaceDescriptor,
    digest: &str,
) -> Result<String, String> {
    let web_capabilities = sorted_capabilities(descriptor)
        .into_iter()
        .filter(|capability| capability.targets.iter().any(|target| target == "web"))
        .collect::<Vec<_>>();
    let web_outputs = sorted_outputs(descriptor)
        .into_iter()
        .filter(|output| output.targets.iter().any(|target| target == "web"))
        .collect::<Vec<_>>();
    let web_binding_operations = sorted_binding_operations(descriptor)
        .into_iter()
        .filter(|operation| operation.targets.iter().any(|target| target == "web"))
        .collect::<Vec<_>>();
    let system_adapter_ids = sorted_capabilities(descriptor)
        .into_iter()
        .filter(|capability| capability.kind == CapabilityKind::Adapter)
        .map(|capability| capability.id.as_str())
        .collect::<Vec<_>>();

    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-capability-surface`.\n// Source: capabilities/feature-surface-v1.json. Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "export const CAPABILITY_DESCRIPTOR_SCHEMA_VERSION = {} as const;",
        descriptor.schema_version
    )
    .unwrap();
    writeln!(
        out,
        "export const CAPABILITY_DESCRIPTOR_DIGEST = {digest:?} as const;\n"
    )
    .unwrap();

    write_typescript_value(
        &mut out,
        "WEB_CAPABILITIES",
        serde_json::json!(
            web_capabilities
                .iter()
                .map(|capability| serde_json::json!({
                    "id": capability.id,
                    "kind": capability.kind.as_str(),
                    "implications": sorted_string_refs(&capability.implications),
                }))
                .collect::<Vec<_>>()
        ),
    )?;
    write_typescript_value(
        &mut out,
        "WEB_OUTPUTS",
        serde_json::json!(
            web_outputs
                .iter()
                .map(|output| serde_json::json!({
                    "id": output.id,
                    "capability": output.capability,
                }))
                .collect::<Vec<_>>()
        ),
    )?;
    write_typescript_value(
        &mut out,
        "WEB_BINDING_OPERATIONS",
        serde_json::json!(
            web_binding_operations
                .iter()
                .map(|operation| serde_json::json!({
                    "id": operation.id,
                    "capability": operation.capability.0.as_deref(),
                    "output": operation.output.0.as_deref(),
                    "compiled_prerequisites": sorted_string_refs(&operation.compiled_prerequisites),
                    "media_type": operation.media_type,
                    "requires_uri": operation.requires_uri,
                }))
                .collect::<Vec<_>>()
        ),
    )?;
    for (name, ids, type_name) in [
        (
            "WEB_CAPABILITY_IDS",
            web_capabilities
                .iter()
                .map(|capability| capability.id.as_str())
                .collect::<Vec<_>>(),
            "WebCapabilityId",
        ),
        (
            "WEB_OUTPUT_IDS",
            web_outputs
                .iter()
                .map(|output| output.id.as_str())
                .collect::<Vec<_>>(),
            "WebOutputId",
        ),
        (
            "WEB_BINDING_OPERATION_IDS",
            web_binding_operations
                .iter()
                .map(|operation| operation.id.as_str())
                .collect::<Vec<_>>(),
            "WebBindingOperationId",
        ),
        ("SYSTEM_ADAPTER_IDS", system_adapter_ids, "SystemAdapterId"),
    ] {
        write_typescript_value(&mut out, name, serde_json::json!(ids))?;
        writeln!(out, "export type {type_name} = (typeof {name})[number];\n").unwrap();
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    Ok(out)
}

fn write_c_string_array(out: &mut String, name: &str, values: &[&str]) {
    if values.is_empty() {
        return;
    }
    writeln!(out, "static const char *const {name}[] = {{").unwrap();
    for value in values {
        writeln!(out, "    {value:?},").unwrap();
    }
    out.push_str("};\n\n");
}

fn c_array_name_or_null(name: &str, values: &[&str]) -> String {
    if values.is_empty() {
        "NULL".to_string()
    } else {
        name.to_string()
    }
}

fn c_nullable_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "NULL".to_string())
}

fn render_c_header(
    descriptor: &CapabilitySurfaceDescriptor,
    digest: &str,
) -> Result<String, String> {
    let mut out = String::from(
        "/* @generated by `cargo run -p xtask -- gen-capability-surface`. */\n/* Source: capabilities/feature-surface-v1.json. Do not edit directly. */\n\n#ifndef MERMAN_CAPABILITY_SURFACE_H\n#define MERMAN_CAPABILITY_SURFACE_H\n\n#include <stddef.h>\n\n",
    );
    writeln!(
        out,
        "#define MERMAN_CAPABILITY_DESCRIPTOR_SCHEMA_VERSION {}",
        descriptor.schema_version
    )
    .unwrap();
    writeln!(
        out,
        "#define MERMAN_CAPABILITY_DESCRIPTOR_DIGEST {:?}\n",
        digest
    )
    .unwrap();
    for target in sorted_targets(descriptor) {
        writeln!(
            out,
            "#define MERMAN_TARGET_{} {:?}",
            upper_snake(&target.id),
            target.id
        )
        .unwrap();
    }
    out.push('\n');
    for capability in sorted_capabilities(descriptor) {
        writeln!(
            out,
            "#define MERMAN_CAPABILITY_{} {:?}",
            upper_snake(&capability.id),
            capability.id
        )
        .unwrap();
    }
    out.push('\n');
    for output in sorted_outputs(descriptor) {
        writeln!(
            out,
            "#define MERMAN_OUTPUT_{} {:?}",
            upper_snake(&output.id),
            output.id
        )
        .unwrap();
    }
    out.push('\n');
    for operation in sorted_binding_operations(descriptor) {
        writeln!(
            out,
            "#define MERMAN_BINDING_OPERATION_{} {:?}",
            upper_snake(&operation.id),
            operation.id
        )
        .unwrap();
    }
    out.push_str(
        "\ntypedef struct MermanCapabilityDescriptor {\n    const char *id;\n    const char *kind;\n    const char *description;\n    const char *const *target_ids;\n    size_t target_count;\n    const char *const *implication_ids;\n    size_t implication_count;\n} MermanCapabilityDescriptor;\n\ntypedef struct MermanOutputDescriptor {\n    const char *id;\n    const char *capability_id;\n    const char *description;\n    const char *media_type;\n    const char *const *target_ids;\n    size_t target_count;\n} MermanOutputDescriptor;\n\ntypedef struct MermanBindingOperationDescriptor {\n    const char *id;\n    const char *capability_id;\n    const char *description;\n    const char *media_type;\n    int requires_uri;\n    const char *const *target_ids;\n    size_t target_count;\n    const char *output_id;\n    const char *const *compiled_prerequisite_ids;\n    size_t compiled_prerequisite_count;\n} MermanBindingOperationDescriptor;\n\n",
    );

    for capability in sorted_capabilities(descriptor) {
        let prefix = format!("MERMAN_CAPABILITY_{}", upper_snake(&capability.id));
        write_c_string_array(
            &mut out,
            &format!("{prefix}_TARGETS"),
            &sorted_string_refs(&capability.targets),
        );
        write_c_string_array(
            &mut out,
            &format!("{prefix}_IMPLICATIONS"),
            &sorted_string_refs(&capability.implications),
        );
    }
    out.push_str("static const MermanCapabilityDescriptor MERMAN_CAPABILITIES[] = {\n");
    for capability in sorted_capabilities(descriptor) {
        let prefix = format!("MERMAN_CAPABILITY_{}", upper_snake(&capability.id));
        let targets = sorted_string_refs(&capability.targets);
        let implications = sorted_string_refs(&capability.implications);
        writeln!(
            out,
            "    {{ {:?}, {:?}, {:?}, {}, {}, {}, {} }},",
            capability.id,
            capability.kind.as_str(),
            capability.description,
            c_array_name_or_null(&format!("{prefix}_TARGETS"), &targets),
            targets.len(),
            c_array_name_or_null(&format!("{prefix}_IMPLICATIONS"), &implications),
            implications.len()
        )
        .unwrap();
    }
    writeln!(
        out,
        "}};\n#define MERMAN_CAPABILITY_COUNT {}u\n",
        descriptor.capabilities.len()
    )
    .unwrap();

    for output in sorted_outputs(descriptor) {
        write_c_string_array(
            &mut out,
            &format!("MERMAN_OUTPUT_{}_TARGETS", upper_snake(&output.id)),
            &sorted_string_refs(&output.targets),
        );
    }
    out.push_str("static const MermanOutputDescriptor MERMAN_OUTPUTS[] = {\n");
    for output in sorted_outputs(descriptor) {
        let targets = sorted_string_refs(&output.targets);
        let targets_name = format!("MERMAN_OUTPUT_{}_TARGETS", upper_snake(&output.id));
        writeln!(
            out,
            "    {{ {:?}, {:?}, {:?}, {:?}, {}, {} }},",
            output.id,
            output.capability,
            output.description,
            output.media_type,
            c_array_name_or_null(&targets_name, &targets),
            targets.len()
        )
        .unwrap();
    }
    writeln!(
        out,
        "}};\n#define MERMAN_OUTPUT_COUNT {}u\n",
        descriptor.outputs.len()
    )
    .unwrap();

    for operation in sorted_binding_operations(descriptor) {
        write_c_string_array(
            &mut out,
            &format!(
                "MERMAN_BINDING_OPERATION_{}_TARGETS",
                upper_snake(&operation.id)
            ),
            &sorted_string_refs(&operation.targets),
        );
        write_c_string_array(
            &mut out,
            &format!(
                "MERMAN_BINDING_OPERATION_{}_COMPILED_PREREQUISITES",
                upper_snake(&operation.id)
            ),
            &sorted_string_refs(&operation.compiled_prerequisites),
        );
    }
    out.push_str("static const MermanBindingOperationDescriptor MERMAN_BINDING_OPERATIONS[] = {\n");
    for operation in sorted_binding_operations(descriptor) {
        let targets = sorted_string_refs(&operation.targets);
        let targets_name = format!(
            "MERMAN_BINDING_OPERATION_{}_TARGETS",
            upper_snake(&operation.id)
        );
        let compiled_prerequisites = sorted_string_refs(&operation.compiled_prerequisites);
        let compiled_prerequisites_name = format!(
            "MERMAN_BINDING_OPERATION_{}_COMPILED_PREREQUISITES",
            upper_snake(&operation.id)
        );
        writeln!(
            out,
            "    {{ {:?}, {}, {:?}, {:?}, {}, {}, {}, {}, {}, {} }},",
            operation.id,
            c_nullable_string(operation.capability.0.as_deref()),
            operation.description,
            operation.media_type,
            if operation.requires_uri { 1 } else { 0 },
            c_array_name_or_null(&targets_name, &targets),
            targets.len(),
            c_nullable_string(operation.output.0.as_deref()),
            c_array_name_or_null(&compiled_prerequisites_name, &compiled_prerequisites),
            compiled_prerequisites.len()
        )
        .unwrap();
    }
    writeln!(
        out,
        "}};\n#define MERMAN_BINDING_OPERATION_COUNT {}u\n",
        descriptor.binding_operations.len()
    )
    .unwrap();

    writeln!(out, "\n#endif /* MERMAN_CAPABILITY_SURFACE_H */").unwrap();
    Ok(out)
}

fn render_markdown(
    descriptor: &CapabilitySurfaceDescriptor,
    digest: &str,
) -> Result<String, String> {
    let mut out = format!(
        "<!-- @generated by `cargo run -p xtask -- gen-capability-surface`; do not edit. -->\n\n# Capability Surface v1\n\nSemantic digest: `{digest}`\n\n## Public Leaves\n\n| ID | Kind | Targets | Implies | Description |\n| --- | --- | --- | --- | --- |\n"
    );
    for capability in sorted_capabilities(descriptor) {
        writeln!(
            out,
            "| `{}` | `{:?}` | {} | {} | {} |",
            capability.id,
            capability.kind,
            code_list(capability.targets.iter().map(String::as_str)),
            code_list(capability.implications.iter().map(String::as_str)),
            capability.description
        )
        .unwrap();
    }
    out.push_str(
        "\n## Outputs\n\n| ID | Capability | Media type | Targets |\n| --- | --- | --- | --- |\n",
    );
    for output in sorted_outputs(descriptor) {
        writeln!(
            out,
            "| `{}` | `{}` | `{}` | {} |",
            output.id,
            output.capability,
            output.media_type,
            code_list(output.targets.iter().map(String::as_str))
        )
        .unwrap();
    }
    out.push_str(
        "\n## Binding Operations\n\n| ID | Capability | Output | Compiled prerequisites | Media type | Requires URI | Targets |\n| --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for operation in sorted_binding_operations(descriptor) {
        let capability = operation
            .capability
            .0
            .as_deref()
            .map(|capability| format!("`{capability}`"))
            .unwrap_or_else(|| "none".to_string());
        let output = operation
            .output
            .0
            .as_deref()
            .map(|output| format!("`{output}`"))
            .unwrap_or_else(|| "none".to_string());
        writeln!(
            out,
            "| `{}` | {} | {} | {} | `{}` | {} | {} |",
            operation.id,
            capability,
            output,
            code_list(operation.compiled_prerequisites.iter().map(String::as_str)),
            operation.media_type,
            if operation.requires_uri { "yes" } else { "no" },
            code_list(operation.targets.iter().map(String::as_str))
        )
        .unwrap();
    }
    Ok(out)
}

fn upper_snake(value: &str) -> String {
    value.replace('-', "_").to_ascii_uppercase()
}

fn code_list<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .into_iter()
            .map(|value| format!("`{value}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn generated_artifacts(
    descriptor: &CapabilitySurfaceDescriptor,
) -> Result<Vec<(PathBuf, String)>, String> {
    let digest = semantic_digest(descriptor)?;
    GENERATED_OUTPUTS
        .iter()
        .map(|(path, kind)| {
            kind.render(descriptor, &digest)
                .map(|contents| (PathBuf::from(path), contents))
        })
        .collect()
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

fn drifted_artifacts(
    root: &Path,
    descriptor: &CapabilitySurfaceDescriptor,
) -> Result<Vec<PathBuf>, XtaskError> {
    let mut drift = Vec::new();
    for (path, expected) in generated_artifacts(descriptor).map_err(surface_error)? {
        let full = root.join(&path);
        let actual = fs::read_to_string(&full).map_err(|source| XtaskError::ReadFile {
            path: full.display().to_string(),
            source,
        })?;
        if actual.replace("\r\n", "\n") != expected {
            drift.push(path);
        }
    }
    Ok(drift)
}

pub(crate) fn gen_capability_surface(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    for (path, contents) in generated_artifacts(&descriptor).map_err(surface_error)? {
        write_artifact(&root, &path, &contents)?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(super) struct CapabilityContractCatalog {
    pub(super) schema_version: u32,
    pub(super) digest: String,
    pub(super) target_ids: BTreeSet<String>,
    pub(super) capability_targets: BTreeMap<String, BTreeSet<String>>,
    pub(super) capability_implications: BTreeMap<String, BTreeSet<String>>,
    pub(super) output_capabilities: BTreeMap<String, String>,
    pub(super) output_targets: BTreeMap<String, BTreeSet<String>>,
}

pub(super) fn load_capability_contract_catalog(
    root: &Path,
) -> Result<CapabilityContractCatalog, XtaskError> {
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    let digest = semantic_digest(&descriptor).map_err(surface_error)?;

    Ok(CapabilityContractCatalog {
        schema_version: descriptor.schema_version,
        digest,
        target_ids: descriptor
            .targets
            .iter()
            .map(|target| target.id.clone())
            .collect(),
        capability_targets: descriptor
            .capabilities
            .iter()
            .map(|capability| {
                (
                    capability.id.clone(),
                    capability.targets.iter().cloned().collect(),
                )
            })
            .collect(),
        capability_implications: descriptor
            .capabilities
            .iter()
            .map(|capability| {
                (
                    capability.id.clone(),
                    capability.implications.iter().cloned().collect(),
                )
            })
            .collect(),
        output_capabilities: descriptor
            .outputs
            .iter()
            .map(|output| (output.id.clone(), output.capability.clone()))
            .collect(),
        output_targets: descriptor
            .outputs
            .iter()
            .map(|output| (output.id.clone(), output.targets.iter().cloned().collect()))
            .collect(),
    })
}

pub(crate) fn verify_capability_surface_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    verify_capability_surface_artifacts_with(&root, &descriptor)
}

fn verify_capability_surface_artifacts_with(
    root: &Path,
    descriptor: &CapabilitySurfaceDescriptor,
) -> Result<Option<String>, XtaskError> {
    let drift = drifted_artifacts(root, descriptor)?;
    if drift.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "capability surface projections drifted: {}; regenerate with `cargo run -p xtask -- gen-capability-surface`",
            drift
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }
}

pub(crate) fn verify_capability_surface(args: Vec<String>) -> Result<(), XtaskError> {
    let root = crate::cmd::workspace_root();
    let mut descriptor_path = root.join(DESCRIPTOR_PATH);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--descriptor" => {
                index += 1;
                descriptor_path = PathBuf::from(args.get(index).ok_or(XtaskError::Usage)?);
            }
            "--help" | "-h" => {
                println!("usage: xtask verify-capability-surface [--descriptor <path>]");
                return Err(XtaskError::Usage);
            }
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }

    let descriptor = read_descriptor(&descriptor_path)?;
    if descriptor_path == root.join(DESCRIPTOR_PATH)
        && let Some(message) = verify_capability_surface_artifacts_with(&root, &descriptor)?
    {
        return Err(XtaskError::VerifyFailed(message));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn committed_value() -> Value {
        let path = crate::cmd::workspace_root().join(DESCRIPTOR_PATH);
        serde_json::from_str(&fs::read_to_string(path).expect("committed descriptor"))
            .expect("valid JSON")
    }

    fn committed_descriptor() -> CapabilitySurfaceDescriptor {
        let value = committed_value();
        serde_json::from_value(value).expect("typed committed descriptor")
    }

    fn validate_fixture(descriptor: Value) -> Result<(), String> {
        let descriptor = serde_json::from_value::<CapabilitySurfaceDescriptor>(descriptor)
            .map_err(|error| format!("descriptor schema: {error}"))?;
        validate_descriptor(&descriptor)
    }

    fn capability_index(value: &Value, id: &str) -> usize {
        value["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .position(|capability| capability["id"] == id)
            .unwrap()
    }

    fn binding_operation_index(value: &Value, id: &str) -> usize {
        value["binding_operations"]
            .as_array()
            .unwrap()
            .iter()
            .position(|operation| operation["id"] == id)
            .unwrap()
    }

    #[test]
    fn committed_descriptor_and_generated_artifacts_are_deterministic() {
        let descriptor = committed_descriptor();
        validate_descriptor(&descriptor).unwrap();
        assert_eq!(
            generated_artifacts(&descriptor).unwrap(),
            generated_artifacts(&descriptor).unwrap()
        );
        let semantic_operation = descriptor
            .binding_operations
            .iter()
            .find(|operation| operation.id == "semantic-json")
            .expect("semantic JSON operation must be declared");
        assert_eq!(semantic_operation.capability.0.as_deref(), None);
        assert_eq!(semantic_operation.output.0.as_deref(), None);
        assert!(semantic_operation.compiled_prerequisites.is_empty());
        assert!(!semantic_operation.requires_uri);

        for (operation_id, output_id, compiled_prerequisites) in [
            ("svg", "svg", &[][..]),
            ("ascii", "ascii", &[][..]),
            ("png", "png", &["svg"][..]),
            ("jpeg", "jpeg", &["svg"][..]),
            ("pdf", "pdf", &["svg"][..]),
        ] {
            let operation = descriptor
                .binding_operations
                .iter()
                .find(|operation| operation.id == operation_id)
                .expect("output operation must be declared");
            assert_eq!(operation.output.0.as_deref(), Some(output_id));
            assert_eq!(
                operation
                    .compiled_prerequisites
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                compiled_prerequisites
            );
        }

        let header = render_c_header(&descriptor, &semantic_digest(&descriptor).unwrap()).unwrap();
        assert!(header.contains("const char *output_id;"));
        assert!(header.contains("const char *const *compiled_prerequisite_ids;"));
        assert!(header.contains("size_t compiled_prerequisite_count;"));
        assert!(header.contains("MERMAN_BINDING_OPERATION_PNG_COMPILED_PREREQUISITES"));
    }

    #[test]
    fn semantic_digest_covers_output_and_compilation_relationships() {
        let descriptor = committed_descriptor();
        let baseline = semantic_digest(&descriptor).unwrap();

        let mut changed = descriptor.clone();
        let png = changed
            .binding_operations
            .iter_mut()
            .find(|operation| operation.id == "png")
            .unwrap();
        png.compiled_prerequisites.clear();

        assert_ne!(semantic_digest(&changed).unwrap(), baseline);
    }

    #[test]
    fn rust_operation_variants_are_stable_pascal_case() {
        assert_eq!(rust_operation_variant("svg"), "Svg");
        assert_eq!(rust_operation_variant("semantic-json"), "SemanticJson");
        assert_eq!(
            rust_operation_variant("document-analysis-facts-json"),
            "DocumentAnalysisFactsJson"
        );
    }

    #[test]
    fn output_relationship_allows_an_independent_operation_id() {
        let mut descriptor = committed_value();
        let svg = binding_operation_index(&descriptor, "svg");
        descriptor["binding_operations"][svg]["id"] = json!("render-svg");

        validate_fixture(descriptor).unwrap();
    }

    #[test]
    fn fixture_rejects_invalid_binding_operation_descriptors() {
        let committed = committed_value();
        let semantic = binding_operation_index(&committed, "semantic-json");
        let png = binding_operation_index(&committed, "png");
        let svg = binding_operation_index(&committed, "svg");

        let mut missing_capability = committed.clone();
        missing_capability["binding_operations"][semantic]
            .as_object_mut()
            .unwrap()
            .remove("capability");
        let error = validate_fixture(missing_capability).unwrap_err();
        assert!(
            error.contains("missing field `capability`"),
            "unexpected diagnostic: {error}"
        );

        let mut unknown_capability = committed.clone();
        unknown_capability["binding_operations"][semantic]["capability"] =
            json!("unknown-capability");
        let error = validate_fixture(unknown_capability).unwrap_err();
        assert!(
            error.contains(&format!("binding_operations[{semantic}].capability"))
                && error.contains("unknown capability"),
            "unexpected diagnostic: {error}"
        );

        let mut missing_output = committed.clone();
        missing_output["binding_operations"][semantic]
            .as_object_mut()
            .unwrap()
            .remove("output");
        let error = validate_fixture(missing_output).unwrap_err();
        assert!(
            error.contains("missing field `output`"),
            "unexpected diagnostic: {error}"
        );

        let mut missing_compiled_prerequisites = committed.clone();
        missing_compiled_prerequisites["binding_operations"][semantic]
            .as_object_mut()
            .unwrap()
            .remove("compiled_prerequisites");
        let error = validate_fixture(missing_compiled_prerequisites).unwrap_err();
        assert!(
            error.contains("missing field `compiled_prerequisites`"),
            "unexpected diagnostic: {error}"
        );

        let mut unknown_output = committed.clone();
        unknown_output["binding_operations"][semantic]["output"] = json!("unknown-output");
        let error = validate_fixture(unknown_output).unwrap_err();
        assert!(
            error.contains(&format!("binding_operations[{semantic}].output"))
                && error.contains("unknown output"),
            "unexpected diagnostic: {error}"
        );

        let mut unknown_compiled_prerequisite = committed.clone();
        unknown_compiled_prerequisite["binding_operations"][semantic]["compiled_prerequisites"] =
            json!(["unknown-capability"]);
        let error = validate_fixture(unknown_compiled_prerequisite).unwrap_err();
        assert!(
            error.contains(&format!(
                "binding_operations[{semantic}].compiled_prerequisites[0]"
            )) && error.contains("unknown capability"),
            "unexpected diagnostic: {error}"
        );

        let mut duplicate_compiled_prerequisite = committed.clone();
        duplicate_compiled_prerequisite["binding_operations"][png]["compiled_prerequisites"] =
            json!(["svg", "svg"]);
        let error = validate_fixture(duplicate_compiled_prerequisite).unwrap_err();
        assert!(
            error.contains(&format!(
                "binding_operations[{png}].compiled_prerequisites[1]"
            )) && error.contains("duplicate ID"),
            "unexpected diagnostic: {error}"
        );

        let mut unsorted_compiled_prerequisites = committed.clone();
        unsorted_compiled_prerequisites["binding_operations"][semantic]["compiled_prerequisites"] =
            json!(["system-fonts", "analysis"]);
        let error = validate_fixture(unsorted_compiled_prerequisites).unwrap_err();
        assert!(
            error.contains(&format!(
                "binding_operations[{semantic}].compiled_prerequisites"
            )) && error.contains("sorted lexicographically"),
            "unexpected diagnostic: {error}"
        );

        let mut repeated_primary_capability = committed.clone();
        repeated_primary_capability["binding_operations"][png]["compiled_prerequisites"] =
            json!(["png"]);
        let error = validate_fixture(repeated_primary_capability).unwrap_err();
        assert!(
            error.contains(&format!(
                "binding_operations[{png}].compiled_prerequisites[0]"
            )) && error.contains("availability capability"),
            "unexpected diagnostic: {error}"
        );

        let analysis = binding_operation_index(&committed, "analysis-json");
        let mut target_invalid_compiled_prerequisite = committed.clone();
        target_invalid_compiled_prerequisite["binding_operations"][analysis]["compiled_prerequisites"] =
            json!(["math"]);
        let error = validate_fixture(target_invalid_compiled_prerequisite).unwrap_err();
        assert!(
            error.contains(&format!(
                "binding_operations[{analysis}].compiled_prerequisites[0]"
            )) && error.contains("unavailable on target `typst`"),
            "unexpected diagnostic: {error}"
        );

        for (capability_id, kind) in [("system-clock", "adapter"), ("icons", "tool")] {
            let mut implementation_invalid_compiled_prerequisite = committed.clone();
            implementation_invalid_compiled_prerequisite["binding_operations"][png]["compiled_prerequisites"] =
                json!([capability_id]);
            let error = validate_fixture(implementation_invalid_compiled_prerequisite).unwrap_err();
            assert!(
                error.contains(&format!(
                    "binding_operations[{png}].compiled_prerequisites[0]"
                )) && error.contains("must be an API, output, or engine capability")
                    && error.contains(&format!("not `{kind}`")),
                "unexpected diagnostic: {error}"
            );
        }

        let mut non_operation_capability = committed.clone();
        non_operation_capability["binding_operations"][semantic]["capability"] =
            json!("system-clock");
        let error = validate_fixture(non_operation_capability).unwrap_err();
        assert!(
            error.contains(&format!("binding_operations[{semantic}].capability"))
                && error.contains("must be an API or output capability"),
            "unexpected diagnostic: {error}"
        );

        let mut invalid_target = committed.clone();
        invalid_target["binding_operations"][png]["targets"] = json!(["web"]);
        let error = validate_fixture(invalid_target).unwrap_err();
        assert!(
            error.contains(&format!("binding_operations[{png}].targets"))
                && error.contains("unavailable on target `web`"),
            "unexpected diagnostic: {error}"
        );

        let mut mismatched_media_type = committed.clone();
        mismatched_media_type["binding_operations"][svg]["media_type"] = json!("text/plain");
        let error = validate_fixture(mismatched_media_type).unwrap_err();
        assert!(
            error.contains(&format!("binding_operations[{svg}].media_type"))
                && error.contains("must match output"),
            "unexpected diagnostic: {error}"
        );

        let mut mismatched_capability = committed.clone();
        mismatched_capability["binding_operations"][svg]["capability"] = json!("analysis");
        let error = validate_fixture(mismatched_capability).unwrap_err();
        assert!(
            error.contains(&format!("binding_operations[{svg}].capability"))
                && error.contains("must match output"),
            "unexpected diagnostic: {error}"
        );

        let mut output_requires_uri = committed.clone();
        output_requires_uri["binding_operations"][svg]["requires_uri"] = json!(true);
        let error = validate_fixture(output_requires_uri).unwrap_err();
        assert!(
            error.contains(&format!("binding_operations[{svg}].requires_uri"))
                && error.contains("must not require a URI"),
            "unexpected diagnostic: {error}"
        );

        let mut mismatched_targets = committed.clone();
        mismatched_targets["binding_operations"][svg]["targets"] = json!(["native", "web"]);
        let error = validate_fixture(mismatched_targets).unwrap_err();
        assert!(
            error.contains(&format!("binding_operations[{svg}].targets"))
                && error.contains("must match output"),
            "unexpected diagnostic: {error}"
        );

        let mut missing_output_owner = committed;
        missing_output_owner["binding_operations"][svg]["output"] = Value::Null;
        let error = validate_fixture(missing_output_owner).unwrap_err();
        assert!(
            error.contains("outputs[id=svg]")
                && error.contains("referenced by exactly one binding operation; found 0"),
            "unexpected diagnostic: {error}"
        );
    }

    #[test]
    fn fixture_rejects_unknown_implication_with_a_path() {
        let mut descriptor = committed_value();
        let index = capability_index(&descriptor, "svg");
        descriptor["capabilities"][index]["implications"] = json!(["unknown-capability"]);

        let error = validate_fixture(descriptor).expect_err("unknown implication must fail");
        assert!(
            error.contains(&format!("capabilities[{index}].implications[0]")),
            "unexpected diagnostic: {error}"
        );
    }

    #[test]
    fn fixture_rejects_duplicate_capability_id_with_a_path() {
        let mut descriptor = committed_value();
        descriptor["capabilities"][1]["id"] = descriptor["capabilities"][0]["id"].clone();
        let error = validate_fixture(descriptor).expect_err("duplicate ID must fail");
        assert!(error.contains("capabilities[1].id: duplicate ID"));
    }

    #[test]
    fn fixture_rejects_non_bijective_output_descriptors() {
        let mut mismatched_id = committed_value();
        let svg_output = mismatched_id["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .position(|output| output["id"] == "svg")
            .unwrap();
        mismatched_id["outputs"][svg_output]["id"] = json!("svg-alias");
        let error = validate_fixture(mismatched_id).unwrap_err();
        assert!(
            error.contains(&format!("outputs[{svg_output}].id"))
                && error.contains("must equal capability ID"),
            "unexpected diagnostic: {error}"
        );

        let mut mismatched_targets = committed_value();
        mismatched_targets["outputs"][svg_output]["targets"] = json!(["native", "web"]);
        let error = validate_fixture(mismatched_targets).unwrap_err();
        assert!(
            error.contains(&format!("outputs[{svg_output}].targets"))
                && error.contains("must exactly equal"),
            "unexpected diagnostic: {error}"
        );

        let mut missing_descriptor = committed_value();
        missing_descriptor["outputs"]
            .as_array_mut()
            .unwrap()
            .remove(svg_output);
        let error = validate_fixture(missing_descriptor).unwrap_err();
        assert!(
            error.contains("capabilities[id=svg]")
                && error.contains("exactly one output descriptor"),
            "unexpected diagnostic: {error}"
        );
    }

    #[test]
    fn fixture_rejects_implication_cycle() {
        let mut descriptor = committed_value();
        let svg = capability_index(&descriptor, "svg");
        let analysis = capability_index(&descriptor, "analysis");
        descriptor["capabilities"][svg]["implications"] = json!(["analysis"]);
        descriptor["capabilities"][analysis]["implications"] = json!(["svg"]);
        let error = validate_fixture(descriptor).expect_err("cycle must fail");
        assert!(
            error.contains("implication cycle"),
            "unexpected diagnostic: {error}"
        );
    }

    #[test]
    fn fixture_rejects_negative_diagram_and_dependency_named_leaves() {
        for (invalid, expected) in [
            ("svg-no-elk", "negative public feature"),
            ("flowchart", "diagram-specific public feature"),
            ("ratex", "incidental dependency-named public feature"),
        ] {
            let mut descriptor = committed_value();
            let index = capability_index(&descriptor, "svg");
            descriptor["capabilities"][index]["id"] = json!(invalid);
            let error = validate_fixture(descriptor).unwrap_err();
            assert!(error.contains(expected), "unexpected diagnostic: {error}");
        }
    }

    #[test]
    fn fixture_rejects_nonsemantic_build_and_exclusion_fields() {
        let mut with_surface_mappings = committed_value();
        with_surface_mappings["surface_mappings"] = json!([]);
        let error = validate_fixture(with_surface_mappings).unwrap_err();
        assert!(
            error.contains("unknown field `surface_mappings`"),
            "unexpected diagnostic: {error}"
        );
    }

    #[test]
    fn fixture_rejects_manual_admission_bookkeeping() {
        let mut descriptor = committed_value();
        let index = capability_index(&descriptor, "svg");
        descriptor["capabilities"][index]["admission"] = json!({
            "evidence": {"status": "observed"}
        });
        let error = validate_fixture(descriptor).expect_err("manual admission state must fail");
        assert!(
            error.contains("unknown field `admission`"),
            "unexpected diagnostic: {error}"
        );
    }
}
