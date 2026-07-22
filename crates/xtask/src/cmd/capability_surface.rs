//! Canonical capability-surface descriptor validation and projection.

use crate::XtaskError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

const DESCRIPTOR_PATH: &str = "capabilities/feature-surface-v1.json";
const DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
const GENERATED_OUTPUTS: &[(&str, ArtifactKind)] = &[
    (
        "capabilities/generated/capability_surface.rs",
        ArtifactKind::Rust,
    ),
    (
        "capabilities/generated/capability-surface.ts",
        ArtifactKind::TypeScript,
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

// These are retirement guards, not capability authorities. They remain stable after a ledger entry
// is removed so strict mode can detect a catalog that was accidentally left live.
const LEGACY_LIVE_CATALOG_GUARDS: &[(&str, &str)] = &[
    ("native-abi", "abi/merman-v2.json"),
    (
        "typst-artifacts",
        "crates/merman-typst-plugin/wasm-profiles.json",
    ),
    ("web-packages", "platforms/web/web-surface-descriptor.json"),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CapabilitySurfaceDescriptor {
    schema_version: u32,
    descriptor_id: String,
    targets: Vec<TargetDescriptor>,
    runtime_capabilities: Vec<RuntimeCapabilityDescriptor>,
    capabilities: Vec<CapabilityDescriptor>,
    outputs: Vec<OutputDescriptor>,
    presets: Vec<PresetDescriptor>,
    surface_mappings: Vec<SurfaceMapping>,
    migration_ledger: Vec<MigrationLedgerEntry>,
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
struct RuntimeCapabilityDescriptor {
    id: String,
    kind: RuntimeCapabilityKind,
    description: String,
    targets: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum RuntimeCapabilityKind {
    Adapter,
    Transport,
}

impl RuntimeCapabilityKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Adapter => "adapter",
            Self::Transport => "transport",
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
    admission: AdmissionContract,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AbsenceContract {
    error_id: String,
    contract: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AdmissionContract {
    observable_contract: String,
    material_closure: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    evidence: Option<MeasurementEvidence>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MeasurementEvidence {
    status: EvidenceStatus,
    kind: String,
    source: String,
    gate: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum EvidenceStatus {
    MigrationRequired,
    Observed,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PresetDescriptor {
    id: String,
    description: String,
    targets: Vec<String>,
    includes: Vec<String>,
    excludes: Vec<String>,
    expected_runtime_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SurfaceMapping {
    id: String,
    surface: String,
    artifact: String,
    target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preset: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    expected_runtime_capabilities: Vec<String>,
    #[serde(default)]
    transport_only: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MigrationLedgerEntry {
    surface: String,
    migration_unit: String,
    legacy_catalogs: Vec<String>,
    replacement: String,
}

#[derive(Debug, Clone, Copy)]
enum ArtifactKind {
    Rust,
    TypeScript,
    CHeader,
    Markdown,
}

impl ArtifactKind {
    fn render(self, descriptor: &CapabilitySurfaceDescriptor) -> Result<String, String> {
        match self {
            Self::Rust => render_rust(descriptor),
            Self::TypeScript => render_typescript(descriptor),
            Self::CHeader => render_c_header(descriptor),
            Self::Markdown => render_markdown(descriptor),
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

    let runtime_ids = validate_unique_ids(
        descriptor
            .runtime_capabilities
            .iter()
            .enumerate()
            .map(|(index, capability)| (index, capability.id.as_str())),
        "runtime_capabilities",
    )?;
    for (index, capability) in descriptor.runtime_capabilities.iter().enumerate() {
        validate_kebab_id(&capability.id, &format!("runtime_capabilities[{index}].id"))?;
        require_non_empty(
            &capability.description,
            &format!("runtime_capabilities[{index}].description"),
        )?;
        validate_targets(
            &capability.targets,
            &format!("runtime_capabilities[{index}].targets"),
            &target_ids,
        )?;
    }

    let capability_ids = validate_unique_ids(
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
        if runtime_ids.contains(&capability.id) {
            return Err(format!(
                "{base}.id: public capability `{}` duplicates a runtime-only capability ID",
                capability.id
            ));
        }
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
        require_non_empty(
            &capability.admission.observable_contract,
            &format!("{base}.admission.observable_contract"),
        )?;
        require_non_empty(
            &capability.admission.material_closure,
            &format!("{base}.admission.material_closure"),
        )?;
        let evidence =
            capability.admission.evidence.as_ref().ok_or_else(|| {
                format!("{base}.admission.evidence: measured evidence is required")
            })?;
        require_non_empty(&evidence.kind, &format!("{base}.admission.evidence.kind"))?;
        require_non_empty(
            &evidence.source,
            &format!("{base}.admission.evidence.source"),
        )?;
        require_non_empty(&evidence.gate, &format!("{base}.admission.evidence.gate"))?;
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

    let preset_ids = validate_unique_ids(
        descriptor
            .presets
            .iter()
            .enumerate()
            .map(|(index, preset)| (index, preset.id.as_str())),
        "presets",
    )?;
    let preset_by_id = descriptor
        .presets
        .iter()
        .map(|preset| (preset.id.as_str(), preset))
        .collect::<BTreeMap<_, _>>();
    for (index, preset) in descriptor.presets.iter().enumerate() {
        let base = format!("presets[{index}]");
        validate_kebab_id(&preset.id, &format!("{base}.id"))?;
        if !preset.id.starts_with("preset-") {
            return Err(format!(
                "{base}.id: public preset `{}` must use the `preset-` namespace",
                preset.id
            ));
        }
        require_non_empty(&preset.description, &format!("{base}.description"))?;
        validate_targets(&preset.targets, &format!("{base}.targets"), &target_ids)?;
        validate_string_set(&preset.includes, &format!("{base}.includes"))?;
        validate_string_set(&preset.excludes, &format!("{base}.excludes"))?;
        validate_string_set(
            &preset.expected_runtime_capabilities,
            &format!("{base}.expected_runtime_capabilities"),
        )?;
        for (include_index, include) in preset.includes.iter().enumerate() {
            if !capability_ids.contains(include) && !preset_ids.contains(include) {
                return Err(format!(
                    "{base}.includes[{include_index}]: unknown capability or preset `{include}`"
                ));
            }
        }
        for (exclude_index, exclude) in preset.excludes.iter().enumerate() {
            if !capability_ids.contains(exclude) {
                return Err(format!(
                    "{base}.excludes[{exclude_index}]: unknown capability `{exclude}`"
                ));
            }
        }
    }

    let effective_presets = effective_preset_sets(descriptor, &capability_by_id, &preset_by_id)?;
    for (index, preset) in descriptor.presets.iter().enumerate() {
        validate_preset_contract(
            index,
            preset,
            effective_presets
                .get(preset.id.as_str())
                .expect("effective preset exists"),
            &capability_by_id,
            &runtime_ids,
            descriptor,
        )?;
    }

    for (capability_index, capability) in descriptor.capabilities.iter().enumerate() {
        let included = descriptor.presets.iter().any(|preset| {
            preset
                .targets
                .iter()
                .any(|target| capability.targets.contains(target))
                && effective_presets
                    .get(preset.id.as_str())
                    .is_some_and(|set| set.contains(&capability.id))
        });
        let excluded = descriptor.presets.iter().any(|preset| {
            preset
                .targets
                .iter()
                .any(|target| capability.targets.contains(target))
                && preset.excludes.contains(&capability.id)
        });
        if !included || !excluded {
            return Err(format!(
                "capabilities[{capability_index}].admission: `{}` needs at least one applicable preset include and exclude",
                capability.id
            ));
        }
    }

    validate_surface_mappings(
        descriptor,
        &target_ids,
        &capability_by_id,
        &runtime_ids,
        &preset_by_id,
    )?;
    validate_migration_ledger(descriptor)?;
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

fn expand_capability(
    id: &str,
    capability_by_id: &BTreeMap<&str, &CapabilityDescriptor>,
    result: &mut BTreeSet<String>,
) {
    if !result.insert(id.to_string()) {
        return;
    }
    for implication in &capability_by_id[id].implications {
        expand_capability(implication, capability_by_id, result);
    }
}

fn effective_preset_sets(
    descriptor: &CapabilitySurfaceDescriptor,
    capability_by_id: &BTreeMap<&str, &CapabilityDescriptor>,
    preset_by_id: &BTreeMap<&str, &PresetDescriptor>,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    fn expand_preset(
        id: &str,
        capability_by_id: &BTreeMap<&str, &CapabilityDescriptor>,
        preset_by_id: &BTreeMap<&str, &PresetDescriptor>,
        cache: &mut BTreeMap<String, BTreeSet<String>>,
        stack: &mut Vec<String>,
    ) -> Result<BTreeSet<String>, String> {
        if let Some(result) = cache.get(id) {
            return Ok(result.clone());
        }
        if let Some(start) = stack.iter().position(|candidate| candidate == id) {
            let mut cycle = stack[start..].to_vec();
            cycle.push(id.to_string());
            return Err(format!(
                "presets[id={id}].includes: preset cycle: {}",
                cycle.join(" -> ")
            ));
        }
        stack.push(id.to_string());
        let preset = preset_by_id[id];
        let mut result = BTreeSet::new();
        for include in &preset.includes {
            if capability_by_id.contains_key(include.as_str()) {
                expand_capability(include, capability_by_id, &mut result);
            } else {
                result.extend(expand_preset(
                    include,
                    capability_by_id,
                    preset_by_id,
                    cache,
                    stack,
                )?);
            }
        }
        stack.pop();
        cache.insert(id.to_string(), result.clone());
        Ok(result)
    }

    let mut result = BTreeMap::new();
    for preset in &descriptor.presets {
        expand_preset(
            &preset.id,
            capability_by_id,
            preset_by_id,
            &mut result,
            &mut Vec::new(),
        )?;
    }
    Ok(result)
}

fn validate_preset_contract(
    index: usize,
    preset: &PresetDescriptor,
    effective: &BTreeSet<String>,
    capability_by_id: &BTreeMap<&str, &CapabilityDescriptor>,
    runtime_ids: &BTreeSet<String>,
    descriptor: &CapabilitySurfaceDescriptor,
) -> Result<(), String> {
    let base = format!("presets[{index}]");
    for capability_id in effective {
        let capability = capability_by_id[capability_id.as_str()];
        for target in &preset.targets {
            if !capability.targets.contains(target) {
                return Err(format!(
                    "{base}.includes: capability `{capability_id}` is unavailable on target `{target}`"
                ));
            }
        }
    }

    let eligible = descriptor
        .capabilities
        .iter()
        .filter(|capability| {
            preset
                .targets
                .iter()
                .all(|target| capability.targets.contains(target))
        })
        .map(|capability| capability.id.clone())
        .collect::<BTreeSet<_>>();
    let expected_excludes = eligible
        .difference(effective)
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual_excludes = preset.excludes.iter().cloned().collect::<BTreeSet<_>>();
    if actual_excludes != expected_excludes {
        return Err(format!(
            "{base}.excludes: must exactly list applicable leaves outside the preset; expected [{}], found [{}]",
            join_set(&expected_excludes),
            join_set(&actual_excludes)
        ));
    }

    let mut runtime_public = BTreeSet::new();
    for (runtime_index, runtime_id) in preset.expected_runtime_capabilities.iter().enumerate() {
        if let Some(capability) = capability_by_id.get(runtime_id.as_str()) {
            for target in &preset.targets {
                if !capability.targets.contains(target) {
                    return Err(format!(
                        "{base}.expected_runtime_capabilities[{runtime_index}]: `{runtime_id}` is unavailable on target `{target}`"
                    ));
                }
            }
            runtime_public.insert(runtime_id.clone());
        } else if runtime_ids.contains(runtime_id) {
            let runtime = descriptor
                .runtime_capabilities
                .iter()
                .find(|capability| capability.id == *runtime_id)
                .expect("known runtime capability");
            for target in &preset.targets {
                if !runtime.targets.contains(target) {
                    return Err(format!(
                        "{base}.expected_runtime_capabilities[{runtime_index}]: `{runtime_id}` is unavailable on target `{target}`"
                    ));
                }
            }
        } else {
            return Err(format!(
                "{base}.expected_runtime_capabilities[{runtime_index}]: unknown runtime capability `{runtime_id}`"
            ));
        }
    }
    if runtime_public != *effective {
        return Err(format!(
            "{base}.expected_runtime_capabilities: public capability report must equal the effective preset; expected [{}], found [{}]",
            join_set(effective),
            join_set(&runtime_public)
        ));
    }
    Ok(())
}

fn validate_surface_mappings(
    descriptor: &CapabilitySurfaceDescriptor,
    target_ids: &BTreeSet<String>,
    capability_by_id: &BTreeMap<&str, &CapabilityDescriptor>,
    runtime_ids: &BTreeSet<String>,
    preset_by_id: &BTreeMap<&str, &PresetDescriptor>,
) -> Result<(), String> {
    validate_unique_ids(
        descriptor
            .surface_mappings
            .iter()
            .enumerate()
            .map(|(index, mapping)| (index, mapping.id.as_str())),
        "surface_mappings",
    )?;
    for (index, mapping) in descriptor.surface_mappings.iter().enumerate() {
        let base = format!("surface_mappings[{index}]");
        validate_kebab_id(&mapping.id, &format!("{base}.id"))?;
        validate_kebab_id(&mapping.surface, &format!("{base}.surface"))?;
        require_non_empty(&mapping.artifact, &format!("{base}.artifact"))?;
        if !target_ids.contains(&mapping.target) {
            return Err(format!(
                "{base}.target: unknown target `{}`",
                mapping.target
            ));
        }
        if mapping.preset.is_some() && !mapping.capabilities.is_empty() {
            return Err(format!(
                "{base}: select either a preset or direct capabilities, not both"
            ));
        }
        let effective = if let Some(preset_id) = &mapping.preset {
            let Some(preset) = preset_by_id.get(preset_id.as_str()) else {
                return Err(format!("{base}.preset: unknown preset `{preset_id}`"));
            };
            if !preset.targets.contains(&mapping.target) {
                return Err(format!(
                    "{base}.preset: `{preset_id}` is unavailable on target `{}`",
                    mapping.target
                ));
            }
            preset
                .expected_runtime_capabilities
                .iter()
                .filter(|id| capability_by_id.contains_key(id.as_str()))
                .cloned()
                .collect::<BTreeSet<_>>()
        } else {
            let mut effective = BTreeSet::new();
            for (capability_index, capability_id) in mapping.capabilities.iter().enumerate() {
                let Some(capability) = capability_by_id.get(capability_id.as_str()) else {
                    return Err(format!(
                        "{base}.capabilities[{capability_index}]: unknown capability `{capability_id}`"
                    ));
                };
                if !capability.targets.contains(&mapping.target) {
                    return Err(format!(
                        "{base}.capabilities[{capability_index}]: `{capability_id}` is unavailable on target `{}`",
                        mapping.target
                    ));
                }
                expand_capability(capability_id, capability_by_id, &mut effective);
            }
            effective
        };
        if mapping.transport_only && !effective.is_empty() {
            return Err(format!(
                "{base}.transport_only: transport-only mappings cannot expose public capabilities"
            ));
        }
        if !mapping.transport_only && effective.is_empty() {
            return Err(format!(
                "{base}.capabilities: non-transport mapping must expose a public capability"
            ));
        }
        if mapping.preset.is_none() {
            let mut reported_public = BTreeSet::new();
            for (runtime_index, id) in mapping.expected_runtime_capabilities.iter().enumerate() {
                if let Some(capability) = capability_by_id.get(id.as_str()) {
                    if !capability.targets.contains(&mapping.target) {
                        return Err(format!(
                            "{base}.expected_runtime_capabilities[{runtime_index}]: `{id}` is unavailable on target `{}`",
                            mapping.target
                        ));
                    }
                    reported_public.insert(id.clone());
                } else if let Some(runtime) = descriptor
                    .runtime_capabilities
                    .iter()
                    .find(|capability| capability.id == *id)
                {
                    if !runtime.targets.contains(&mapping.target) {
                        return Err(format!(
                            "{base}.expected_runtime_capabilities[{runtime_index}]: `{id}` is unavailable on target `{}`",
                            mapping.target
                        ));
                    }
                } else if !runtime_ids.contains(id) {
                    return Err(format!(
                        "{base}.expected_runtime_capabilities[{runtime_index}]: unknown runtime capability `{id}`"
                    ));
                }
            }
            if reported_public != effective {
                return Err(format!(
                    "{base}.expected_runtime_capabilities: public report must equal mapped capabilities; expected [{}], found [{}]",
                    join_set(&effective),
                    join_set(&reported_public)
                ));
            }
        } else if !mapping.expected_runtime_capabilities.is_empty() {
            return Err(format!(
                "{base}.expected_runtime_capabilities: preset mappings inherit the preset report"
            ));
        }
    }
    Ok(())
}

fn validate_migration_ledger(descriptor: &CapabilitySurfaceDescriptor) -> Result<(), String> {
    validate_unique_ids(
        descriptor
            .migration_ledger
            .iter()
            .enumerate()
            .map(|(index, entry)| (index, entry.surface.as_str())),
        "migration_ledger",
    )?;
    let mut catalog_paths = BTreeSet::new();
    for (index, entry) in descriptor.migration_ledger.iter().enumerate() {
        let base = format!("migration_ledger[{index}]");
        validate_kebab_id(&entry.surface, &format!("{base}.surface"))?;
        if !entry.migration_unit.starts_with('U')
            || entry.migration_unit.len() < 2
            || !entry.migration_unit[1..]
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(format!(
                "{base}.migration_unit: `{}` must be a single implementation unit such as `U6`",
                entry.migration_unit
            ));
        }
        if entry.legacy_catalogs.is_empty() {
            return Err(format!(
                "{base}.legacy_catalogs: pending migration must name its legacy live catalog"
            ));
        }
        for (path_index, path) in entry.legacy_catalogs.iter().enumerate() {
            if !is_owned_relative_path(path) {
                return Err(format!(
                    "{base}.legacy_catalogs[{path_index}]: `{path}` must be a normalized repository-relative path"
                ));
            }
            if !catalog_paths.insert(path) {
                return Err(format!(
                    "{base}.legacy_catalogs[{path_index}]: duplicate legacy catalog `{path}`"
                ));
            }
        }
        require_non_empty(&entry.replacement, &format!("{base}.replacement"))?;
    }
    Ok(())
}

fn is_owned_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !value.contains('\\')
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn validate_committed_ledger(
    root: &Path,
    descriptor: &CapabilitySurfaceDescriptor,
) -> Result<(), String> {
    let recorded = descriptor
        .migration_ledger
        .iter()
        .flat_map(|entry| {
            entry
                .legacy_catalogs
                .iter()
                .map(move |path| (path.as_str(), entry.surface.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    for (surface, path) in LEGACY_LIVE_CATALOG_GUARDS {
        let exists = root.join(path).is_file();
        let recorded_surface = recorded.get(path).copied();
        match (exists, recorded_surface) {
            (true, Some(actual)) if actual == *surface => {}
            (true, Some(actual)) => {
                return Err(format!(
                    "migration_ledger: legacy catalog `{path}` is recorded under `{actual}`, expected `{surface}`"
                ));
            }
            (true, None) => {
                return Err(format!(
                    "migration_ledger: live legacy catalog `{path}` is not recorded"
                ));
            }
            (false, Some(_)) => {
                return Err(format!(
                    "migration_ledger: `{path}` was removed but its ledger entry remains"
                ));
            }
            (false, None) => {}
        }
    }
    for entry in &descriptor.migration_ledger {
        for path in &entry.legacy_catalogs {
            if !root.join(path).is_file() {
                return Err(format!(
                    "migration_ledger[surface={}].legacy_catalogs: `{path}` is not a live file",
                    entry.surface
                ));
            }
        }
    }
    Ok(())
}

fn validate_strict(root: &Path, descriptor: &CapabilitySurfaceDescriptor) -> Result<(), String> {
    if !descriptor.migration_ledger.is_empty() {
        return Err(format!(
            "migration_ledger: strict mode requires an empty ledger; pending surfaces: {}",
            descriptor
                .migration_ledger
                .iter()
                .map(|entry| entry.surface.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for capability in &descriptor.capabilities {
        let status = capability
            .admission
            .evidence
            .as_ref()
            .map(|evidence| evidence.status);
        if status != Some(EvidenceStatus::Observed) {
            return Err(format!(
                "capabilities[id={}].admission.evidence.status: strict mode requires `observed` evidence",
                capability.id
            ));
        }
    }
    let remaining = LEGACY_LIVE_CATALOG_GUARDS
        .iter()
        .filter_map(|(_, path)| root.join(path).is_file().then_some(*path))
        .collect::<Vec<_>>();
    if !remaining.is_empty() {
        return Err(format!(
            "strict.legacy_catalogs: old live catalogs remain: {}",
            remaining.join(", ")
        ));
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
    canonical.migration_ledger.clear();
    canonical
        .targets
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .runtime_capabilities
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .capabilities
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .outputs
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .presets
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .surface_mappings
        .sort_by(|left, right| left.id.cmp(&right.id));
    for capability in &mut canonical.capabilities {
        capability.targets.sort();
        capability.implications.sort();
    }
    for runtime in &mut canonical.runtime_capabilities {
        runtime.targets.sort();
    }
    for output in &mut canonical.outputs {
        output.targets.sort();
    }
    for preset in &mut canonical.presets {
        preset.targets.sort();
        preset.includes.sort();
        preset.excludes.sort();
        preset.expected_runtime_capabilities.sort();
    }
    for mapping in &mut canonical.surface_mappings {
        mapping.capabilities.sort();
        mapping.expected_runtime_capabilities.sort();
    }
    let bytes = serde_json::to_vec(&canonical).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{}", crate::util::sha256_hex(&bytes)))
}

fn sorted_targets(descriptor: &CapabilitySurfaceDescriptor) -> Vec<&TargetDescriptor> {
    let mut values = descriptor.targets.iter().collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}

fn sorted_runtime_capabilities(
    descriptor: &CapabilitySurfaceDescriptor,
) -> Vec<&RuntimeCapabilityDescriptor> {
    let mut values = descriptor.runtime_capabilities.iter().collect::<Vec<_>>();
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

fn sorted_presets(descriptor: &CapabilitySurfaceDescriptor) -> Vec<&PresetDescriptor> {
    let mut values = descriptor.presets.iter().collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}

fn sorted_surface_mappings(descriptor: &CapabilitySurfaceDescriptor) -> Vec<&SurfaceMapping> {
    let mut values = descriptor.surface_mappings.iter().collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values
}

fn sorted_string_refs(values: &[String]) -> Vec<&str> {
    let mut values = values.iter().map(String::as_str).collect::<Vec<_>>();
    values.sort_unstable();
    values
}

struct ResolvedSurfaceMapping<'a> {
    source: &'a SurfaceMapping,
    capabilities: BTreeSet<String>,
    expected_runtime_capabilities: BTreeSet<String>,
}

fn resolve_surface_mappings<'a>(
    descriptor: &'a CapabilitySurfaceDescriptor,
    capability_by_id: &BTreeMap<&str, &CapabilityDescriptor>,
    preset_by_id: &BTreeMap<&str, &PresetDescriptor>,
    effective_presets: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<ResolvedSurfaceMapping<'a>>, String> {
    sorted_surface_mappings(descriptor)
        .into_iter()
        .map(|mapping| {
            let (capabilities, expected_runtime_capabilities) =
                if let Some(preset_id) = mapping.preset.as_deref() {
                    let preset = preset_by_id.get(preset_id).ok_or_else(|| {
                        format!("surface_mappings[id={}].preset: unknown preset `{preset_id}`", mapping.id)
                    })?;
                    let capabilities = effective_presets.get(preset_id).cloned().ok_or_else(|| {
                        format!("surface_mappings[id={}].preset: preset `{preset_id}` has no effective projection", mapping.id)
                    })?;
                    let expected = preset
                        .expected_runtime_capabilities
                        .iter()
                        .cloned()
                        .collect();
                    (capabilities, expected)
                } else {
                    let mut capabilities = BTreeSet::new();
                    for capability in &mapping.capabilities {
                        if !capability_by_id.contains_key(capability.as_str()) {
                            return Err(format!(
                                "surface_mappings[id={}].capabilities: unknown capability `{capability}`",
                                mapping.id
                            ));
                        }
                        expand_capability(capability, capability_by_id, &mut capabilities);
                    }
                    let expected = mapping
                        .expected_runtime_capabilities
                        .iter()
                        .cloned()
                        .collect();
                    (capabilities, expected)
                };
            Ok(ResolvedSurfaceMapping {
                source: mapping,
                capabilities,
                expected_runtime_capabilities,
            })
        })
        .collect()
}

fn render_rust(descriptor: &CapabilitySurfaceDescriptor) -> Result<String, String> {
    let digest = semantic_digest(descriptor)?;
    let capability_by_id = descriptor
        .capabilities
        .iter()
        .map(|capability| (capability.id.as_str(), capability))
        .collect::<BTreeMap<_, _>>();
    let preset_by_id = descriptor
        .presets
        .iter()
        .map(|preset| (preset.id.as_str(), preset))
        .collect::<BTreeMap<_, _>>();
    let effective = effective_preset_sets(descriptor, &capability_by_id, &preset_by_id)?;
    let mappings =
        resolve_surface_mappings(descriptor, &capability_by_id, &preset_by_id, &effective)?;
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
    out.push_str("];\n\npub const RUNTIME_CAPABILITY_IDS: &[&str] = &[\n");
    for capability in sorted_runtime_capabilities(descriptor) {
        writeln!(out, "    {:?},", capability.id).unwrap();
    }
    out.push_str("];\n\npub const CAPABILITY_IDS: &[&str] = &[\n");
    for capability in sorted_capabilities(descriptor) {
        writeln!(out, "    {:?},", capability.id).unwrap();
    }
    out.push_str("];\n\npub const OUTPUT_IDS: &[&str] = &[\n");
    for output in sorted_outputs(descriptor) {
        writeln!(out, "    {:?},", output.id).unwrap();
    }
    out.push_str("];\n\npub const PRESET_IDS: &[&str] = &[\n");
    for preset in sorted_presets(descriptor) {
        writeln!(out, "    {:?},", preset.id).unwrap();
    }
    out.push_str("];\n\npub const SURFACE_MAPPING_IDS: &[&str] = &[\n");
    for mapping in &mappings {
        writeln!(out, "    {:?},", mapping.source.id).unwrap();
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct TargetDescriptor {\n    pub id: &'static str,\n    pub description: &'static str,\n}\n\npub const TARGETS: &[TargetDescriptor] = &[\n");
    for target in sorted_targets(descriptor) {
        writeln!(
            out,
            "    TargetDescriptor {{ id: {:?}, description: {:?} }},",
            target.id, target.description
        )
        .unwrap();
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct RuntimeCapabilityDescriptor {\n    pub id: &'static str,\n    pub kind: &'static str,\n    pub description: &'static str,\n    pub targets: &'static [&'static str],\n}\n\npub const RUNTIME_CAPABILITIES: &[RuntimeCapabilityDescriptor] = &[\n");
    for capability in sorted_runtime_capabilities(descriptor) {
        out.push_str("    RuntimeCapabilityDescriptor {\n");
        writeln!(out, "        id: {:?},", capability.id).unwrap();
        writeln!(out, "        kind: {:?},", capability.kind.as_str()).unwrap();
        writeln!(out, "        description: {:?},", capability.description).unwrap();
        out.push_str("        targets: &[");
        write_quoted_list(&mut out, sorted_string_refs(&capability.targets));
        out.push_str("],\n    },\n");
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct CapabilityDescriptor {\n    pub id: &'static str,\n    pub kind: &'static str,\n    pub description: &'static str,\n    pub targets: &'static [&'static str],\n    pub implications: &'static [&'static str],\n}\n\npub const CAPABILITIES: &[CapabilityDescriptor] = &[\n");
    for capability in sorted_capabilities(descriptor) {
        out.push_str("    CapabilityDescriptor {\n");
        writeln!(out, "        id: {:?},", capability.id).unwrap();
        writeln!(out, "        kind: {:?},", capability.kind.as_str()).unwrap();
        writeln!(out, "        description: {:?},", capability.description).unwrap();
        out.push_str("        targets: &[");
        write_quoted_list(&mut out, sorted_string_refs(&capability.targets));
        out.push_str("],\n        implications: &[");
        write_quoted_list(&mut out, sorted_string_refs(&capability.implications));
        out.push_str("],\n    },\n");
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct OutputDescriptor {\n    pub id: &'static str,\n    pub capability: &'static str,\n    pub description: &'static str,\n    pub media_type: &'static str,\n    pub targets: &'static [&'static str],\n}\n\npub const OUTPUTS: &[OutputDescriptor] = &[\n");
    for output in sorted_outputs(descriptor) {
        out.push_str("    OutputDescriptor {\n");
        writeln!(out, "        id: {:?},", output.id).unwrap();
        writeln!(out, "        capability: {:?},", output.capability).unwrap();
        writeln!(out, "        description: {:?},", output.description).unwrap();
        writeln!(out, "        media_type: {:?},", output.media_type).unwrap();
        out.push_str("        targets: &[");
        write_quoted_list(&mut out, sorted_string_refs(&output.targets));
        out.push_str("],\n    },\n");
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct CapabilityPresetDescriptor {\n    pub id: &'static str,\n    pub description: &'static str,\n    pub targets: &'static [&'static str],\n    pub capabilities: &'static [&'static str],\n    pub expected_runtime_capabilities: &'static [&'static str],\n}\n\npub const CAPABILITY_PRESETS: &[CapabilityPresetDescriptor] = &[\n");
    for preset in sorted_presets(descriptor) {
        out.push_str("    CapabilityPresetDescriptor {\n");
        writeln!(out, "        id: {:?},", preset.id).unwrap();
        writeln!(out, "        description: {:?},", preset.description).unwrap();
        out.push_str("        targets: &[");
        write_quoted_list(&mut out, sorted_string_refs(&preset.targets));
        out.push_str("],\n        capabilities: &[");
        write_quoted_list(&mut out, effective[&preset.id].iter().map(String::as_str));
        out.push_str("],\n        expected_runtime_capabilities: &[");
        write_quoted_list(
            &mut out,
            sorted_string_refs(&preset.expected_runtime_capabilities),
        );
        out.push_str("],\n    },\n");
    }
    out.push_str("];\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SurfaceMappingDescriptor {\n    pub id: &'static str,\n    pub surface: &'static str,\n    pub artifact: &'static str,\n    pub target: &'static str,\n    pub preset: Option<&'static str>,\n    pub capabilities: &'static [&'static str],\n    pub expected_runtime_capabilities: &'static [&'static str],\n    pub transport_only: bool,\n}\n\npub const SURFACE_MAPPINGS: &[SurfaceMappingDescriptor] = &[\n");
    for mapping in mappings {
        out.push_str("    SurfaceMappingDescriptor {\n");
        writeln!(out, "        id: {:?},", mapping.source.id).unwrap();
        writeln!(out, "        surface: {:?},", mapping.source.surface).unwrap();
        writeln!(out, "        artifact: {:?},", mapping.source.artifact).unwrap();
        writeln!(out, "        target: {:?},", mapping.source.target).unwrap();
        match mapping.source.preset.as_deref() {
            Some(preset) => writeln!(out, "        preset: Some({preset:?}),").unwrap(),
            None => out.push_str("        preset: None,\n"),
        }
        out.push_str("        capabilities: &[");
        write_quoted_list(&mut out, mapping.capabilities.iter().map(String::as_str));
        out.push_str("],\n        expected_runtime_capabilities: &[");
        write_quoted_list(
            &mut out,
            mapping
                .expected_runtime_capabilities
                .iter()
                .map(String::as_str),
        );
        writeln!(
            out,
            "],\n        transport_only: {},\n    }},",
            mapping.source.transport_only
        )
        .unwrap();
    }
    out.push_str("];\n");
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

fn render_typescript(descriptor: &CapabilitySurfaceDescriptor) -> Result<String, String> {
    let digest = semantic_digest(descriptor)?;
    let capability_by_id = descriptor
        .capabilities
        .iter()
        .map(|capability| (capability.id.as_str(), capability))
        .collect::<BTreeMap<_, _>>();
    let preset_by_id = descriptor
        .presets
        .iter()
        .map(|preset| (preset.id.as_str(), preset))
        .collect::<BTreeMap<_, _>>();
    let effective = effective_preset_sets(descriptor, &capability_by_id, &preset_by_id)?;
    let mappings =
        resolve_surface_mappings(descriptor, &capability_by_id, &preset_by_id, &effective)?;
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
        "RUNTIME_CAPABILITIES",
        serde_json::json!(
            sorted_runtime_capabilities(descriptor)
                .into_iter()
                .map(|capability| serde_json::json!({
                    "id": capability.id,
                    "kind": capability.kind.as_str(),
                    "description": capability.description,
                    "targets": sorted_string_refs(&capability.targets),
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
        "CAPABILITY_PRESETS",
        serde_json::json!(
            sorted_presets(descriptor)
                .into_iter()
                .map(|preset| serde_json::json!({
                    "id": preset.id,
                    "description": preset.description,
                    "targets": sorted_string_refs(&preset.targets),
                    "capabilities": effective[&preset.id].iter().collect::<Vec<_>>(),
                    "expected_runtime_capabilities": sorted_string_refs(
                        &preset.expected_runtime_capabilities,
                    ),
                }))
                .collect::<Vec<_>>()
        ),
    )?;
    write_typescript_value(
        &mut out,
        "SURFACE_MAPPINGS",
        serde_json::json!(
            mappings
                .iter()
                .map(|mapping| serde_json::json!({
                    "id": mapping.source.id,
                    "surface": mapping.source.surface,
                    "artifact": mapping.source.artifact,
                    "target": mapping.source.target,
                    "preset": mapping.source.preset.as_deref(),
                    "capabilities": mapping.capabilities.iter().collect::<Vec<_>>(),
                    "expected_runtime_capabilities": mapping
                        .expected_runtime_capabilities
                        .iter()
                        .collect::<Vec<_>>(),
                    "transport_only": mapping.source.transport_only,
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
            "RUNTIME_CAPABILITY_IDS",
            sorted_runtime_capabilities(descriptor)
                .into_iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
            "RuntimeCapabilityId",
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
            "PRESET_IDS",
            sorted_presets(descriptor)
                .into_iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
            "CapabilityPresetId",
        ),
        (
            "SURFACE_MAPPING_IDS",
            mappings
                .iter()
                .map(|value| value.source.id.as_str())
                .collect::<Vec<_>>(),
            "SurfaceMappingId",
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

fn render_c_header(descriptor: &CapabilitySurfaceDescriptor) -> Result<String, String> {
    let digest = semantic_digest(descriptor)?;
    let capability_by_id = descriptor
        .capabilities
        .iter()
        .map(|capability| (capability.id.as_str(), capability))
        .collect::<BTreeMap<_, _>>();
    let preset_by_id = descriptor
        .presets
        .iter()
        .map(|preset| (preset.id.as_str(), preset))
        .collect::<BTreeMap<_, _>>();
    let effective = effective_preset_sets(descriptor, &capability_by_id, &preset_by_id)?;
    let native_presets = sorted_presets(descriptor)
        .into_iter()
        .filter(|preset| preset.targets.iter().any(|target| target == "native"))
        .collect::<Vec<_>>();
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
    for capability in sorted_runtime_capabilities(descriptor) {
        writeln!(
            out,
            "#define MERMAN_RUNTIME_CAPABILITY_{} {:?}",
            upper_snake(&capability.id),
            capability.id
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
    for preset in sorted_presets(descriptor) {
        let suffix = preset.id.strip_prefix("preset-").unwrap_or(&preset.id);
        writeln!(
            out,
            "#define MERMAN_PRESET_{} {:?}",
            upper_snake(suffix),
            preset.id
        )
        .unwrap();
    }

    out.push_str(
        "\ntypedef struct MermanRuntimeCapabilityDescriptor {\n    const char *id;\n    const char *kind;\n    const char *description;\n    const char *const *target_ids;\n    size_t target_count;\n} MermanRuntimeCapabilityDescriptor;\n\ntypedef struct MermanCapabilityDescriptor {\n    const char *id;\n    const char *kind;\n    const char *description;\n    const char *const *target_ids;\n    size_t target_count;\n    const char *const *implication_ids;\n    size_t implication_count;\n} MermanCapabilityDescriptor;\n\ntypedef struct MermanOutputDescriptor {\n    const char *id;\n    const char *capability_id;\n    const char *description;\n    const char *media_type;\n    const char *const *target_ids;\n    size_t target_count;\n} MermanOutputDescriptor;\n\ntypedef struct MermanCapabilityPresetDescriptor {\n    const char *id;\n    const char *description;\n    const char *const *target_ids;\n    size_t target_count;\n    const char *const *capability_ids;\n    size_t capability_count;\n    const char *const *expected_runtime_capability_ids;\n    size_t expected_runtime_capability_count;\n} MermanCapabilityPresetDescriptor;\n\n",
    );

    for capability in sorted_runtime_capabilities(descriptor) {
        let name = format!(
            "MERMAN_RUNTIME_CAPABILITY_{}_TARGETS",
            upper_snake(&capability.id)
        );
        write_c_string_array(&mut out, &name, &sorted_string_refs(&capability.targets));
    }
    out.push_str(
        "static const MermanRuntimeCapabilityDescriptor MERMAN_RUNTIME_CAPABILITIES[] = {\n",
    );
    for capability in sorted_runtime_capabilities(descriptor) {
        let targets = sorted_string_refs(&capability.targets);
        let targets_name = format!(
            "MERMAN_RUNTIME_CAPABILITY_{}_TARGETS",
            upper_snake(&capability.id)
        );
        writeln!(
            out,
            "    {{ {:?}, {:?}, {:?}, {}, {} }},",
            capability.id,
            capability.kind.as_str(),
            capability.description,
            c_array_name_or_null(&targets_name, &targets),
            targets.len()
        )
        .unwrap();
    }
    writeln!(
        out,
        "}};\n#define MERMAN_RUNTIME_CAPABILITY_COUNT {}u\n",
        descriptor.runtime_capabilities.len()
    )
    .unwrap();

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

    for preset in &native_presets {
        let suffix = upper_snake(preset.id.strip_prefix("preset-").unwrap_or(&preset.id));
        write_c_string_array(
            &mut out,
            &format!("MERMAN_PRESET_{suffix}_TARGETS"),
            &sorted_string_refs(&preset.targets),
        );
        let capabilities = effective[&preset.id]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        write_c_string_array(
            &mut out,
            &format!("MERMAN_PRESET_{suffix}_CAPABILITIES"),
            &capabilities,
        );
        write_c_string_array(
            &mut out,
            &format!("MERMAN_PRESET_{suffix}_EXPECTED_RUNTIME_CAPABILITIES"),
            &sorted_string_refs(&preset.expected_runtime_capabilities),
        );
    }
    out.push_str(
        "static const MermanCapabilityPresetDescriptor MERMAN_NATIVE_CAPABILITY_PRESETS[] = {\n",
    );
    for preset in &native_presets {
        let suffix = upper_snake(preset.id.strip_prefix("preset-").unwrap_or(&preset.id));
        let targets = sorted_string_refs(&preset.targets);
        let capabilities = effective[&preset.id]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let expected = sorted_string_refs(&preset.expected_runtime_capabilities);
        writeln!(
            out,
            "    {{ {:?}, {:?}, {}, {}, {}, {}, {}, {} }},",
            preset.id,
            preset.description,
            c_array_name_or_null(&format!("MERMAN_PRESET_{suffix}_TARGETS"), &targets),
            targets.len(),
            c_array_name_or_null(
                &format!("MERMAN_PRESET_{suffix}_CAPABILITIES"),
                &capabilities,
            ),
            capabilities.len(),
            c_array_name_or_null(
                &format!("MERMAN_PRESET_{suffix}_EXPECTED_RUNTIME_CAPABILITIES"),
                &expected,
            ),
            expected.len()
        )
        .unwrap();
    }
    writeln!(
        out,
        "}};\n#define MERMAN_NATIVE_CAPABILITY_PRESET_COUNT {}u\n\n#endif /* MERMAN_CAPABILITY_SURFACE_H */",
        native_presets.len()
    )
    .unwrap();
    Ok(out)
}

fn render_markdown(descriptor: &CapabilitySurfaceDescriptor) -> Result<String, String> {
    let digest = semantic_digest(descriptor)?;
    let capability_by_id = descriptor
        .capabilities
        .iter()
        .map(|capability| (capability.id.as_str(), capability))
        .collect::<BTreeMap<_, _>>();
    let preset_by_id = descriptor
        .presets
        .iter()
        .map(|preset| (preset.id.as_str(), preset))
        .collect::<BTreeMap<_, _>>();
    let effective = effective_preset_sets(descriptor, &capability_by_id, &preset_by_id)?;
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
    out.push_str("\n## Presets\n\n| ID | Targets | Effective leaves | Explicit exclusions | Expected runtime report |\n| --- | --- | --- | --- | --- |\n");
    for preset in sorted_presets(descriptor) {
        writeln!(
            out,
            "| `{}` | {} | {} | {} | {} |",
            preset.id,
            code_list(preset.targets.iter().map(String::as_str)),
            code_list(effective[&preset.id].iter().map(String::as_str)),
            code_list(preset.excludes.iter().map(String::as_str)),
            code_list(
                preset
                    .expected_runtime_capabilities
                    .iter()
                    .map(String::as_str)
            )
        )
        .unwrap();
    }
    out.push_str("\n## Surface Mappings\n\n| ID | Surface | Artifact | Selection |\n| --- | --- | --- | --- |\n");
    let mut mappings = descriptor.surface_mappings.iter().collect::<Vec<_>>();
    mappings.sort_by(|left, right| left.id.cmp(&right.id));
    for mapping in mappings {
        let selection = mapping.preset.clone().unwrap_or_else(|| {
            if mapping.transport_only {
                "transport only".to_string()
            } else {
                mapping.capabilities.join(", ")
            }
        });
        writeln!(
            out,
            "| `{}` | `{}` | `{}` | `{}` |",
            mapping.id, mapping.surface, mapping.artifact, selection
        )
        .unwrap();
    }
    out.push_str("\n## Pending Migrations\n\nThese entries are explicit transitional debt, not evidence that current consumers match this descriptor.\n\n| Surface | Unit | Legacy live catalogs | Replacement |\n| --- | --- | --- | --- |\n");
    let mut ledger = descriptor.migration_ledger.iter().collect::<Vec<_>>();
    ledger.sort_by(|left, right| left.surface.cmp(&right.surface));
    for entry in ledger {
        writeln!(
            out,
            "| `{}` | `{}` | {} | {} |",
            entry.surface,
            entry.migration_unit,
            code_list(entry.legacy_catalogs.iter().map(String::as_str)),
            entry.replacement
        )
        .unwrap();
    }
    Ok(out)
}

fn upper_snake(value: &str) -> String {
    value.replace('-', "_").to_ascii_uppercase()
}

fn write_quoted_list<'a>(out: &mut String, values: impl IntoIterator<Item = &'a str>) {
    let mut first = true;
    for value in values {
        if !first {
            out.push_str(", ");
        }
        first = false;
        write!(out, "{value:?}").unwrap();
    }
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

fn join_set(values: &BTreeSet<String>) -> String {
    values
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn generated_artifacts(
    descriptor: &CapabilitySurfaceDescriptor,
) -> Result<Vec<(PathBuf, String)>, String> {
    GENERATED_OUTPUTS
        .iter()
        .map(|(path, kind)| {
            kind.render(descriptor)
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
    validate_committed_ledger(&root, &descriptor).map_err(surface_error)?;
    for (path, contents) in generated_artifacts(&descriptor).map_err(surface_error)? {
        write_artifact(&root, &path, &contents)?;
    }
    Ok(())
}

pub(crate) fn verify_capability_surface_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    validate_committed_ledger(&root, &descriptor).map_err(surface_error)?;
    let drift = drifted_artifacts(&root, &descriptor)?;
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
    let mut custom_descriptor = false;
    let mut strict = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--descriptor" => {
                index += 1;
                descriptor_path = PathBuf::from(args.get(index).ok_or(XtaskError::Usage)?);
                custom_descriptor = true;
            }
            "--strict" => strict = true,
            "--help" | "-h" => {
                println!("usage: xtask verify-capability-surface [--descriptor <path>] [--strict]");
                return Err(XtaskError::Usage);
            }
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }

    let descriptor = read_descriptor(&descriptor_path)?;
    if !custom_descriptor {
        validate_committed_ledger(&root, &descriptor).map_err(surface_error)?;
    }
    if strict {
        validate_strict(&root, &descriptor).map_err(surface_error)?;
    }
    if !custom_descriptor && let Some(message) = verify_capability_surface_artifacts()? {
        return Err(XtaskError::VerifyFailed(message));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const KTD4_INITIAL_PUBLIC_LEAVES: &[&str] = &[
        "analysis",
        "ascii",
        "editor",
        "jpeg",
        "layout-cytoscape",
        "layout-elk",
        "math",
        "network-icons",
        "parallel-markdown",
        "pdf",
        "png",
        "shell-completions",
        "svg",
        "system-clock",
        "system-random",
        "system-timezone",
        "system-timing",
    ];

    const KTD4_INITIAL_OUTPUTS: &[&str] = &["ascii", "jpeg", "pdf", "png", "svg"];

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

    fn preset_index(value: &Value, id: &str) -> usize {
        value["presets"]
            .as_array()
            .unwrap()
            .iter()
            .position(|preset| preset["id"] == id)
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
        let ids = descriptor
            .capabilities
            .iter()
            .map(|capability| capability.id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids, KTD4_INITIAL_PUBLIC_LEAVES.iter().copied().collect());
        assert_eq!(
            descriptor
                .outputs
                .iter()
                .map(|output| output.id.as_str())
                .collect::<BTreeSet<_>>(),
            KTD4_INITIAL_OUTPUTS.iter().copied().collect()
        );
    }

    #[test]
    fn generated_projections_are_complete_for_rust_typescript_and_native_consumers() {
        let descriptor = committed_descriptor();
        let rust = render_rust(&descriptor).unwrap();
        let typescript = render_typescript(&descriptor).unwrap();
        let c = render_c_header(&descriptor).unwrap();

        assert!(typescript.ends_with('\n'));
        assert!(!typescript.ends_with("\n\n"));

        for required in [
            "TARGETS",
            "RUNTIME_CAPABILITIES",
            "CAPABILITIES",
            "implications",
            "OUTPUTS",
            "CAPABILITY_PRESETS",
            "SURFACE_MAPPINGS",
            "@mermanjs/analysis",
            "typst-publish",
        ] {
            assert!(rust.contains(required), "Rust projection missed {required}");
            assert!(
                typescript.contains(required),
                "TypeScript projection missed {required}"
            );
        }
        for required in [
            "MermanRuntimeCapabilityDescriptor",
            "MermanCapabilityDescriptor",
            "MermanOutputDescriptor",
            "MermanCapabilityPresetDescriptor",
            "MERMAN_TARGET_WEB",
            "MERMAN_RUNTIME_CAPABILITY_BROWSER_TIME",
            "MERMAN_PRESET_NATIVE_SVG",
        ] {
            assert!(c.contains(required), "C projection missed {required}");
        }
        assert!(!c.contains("@mermanjs/"));
        assert!(!c.contains("typst-publish"));
    }

    #[test]
    fn committed_migration_ledger_covers_every_live_legacy_catalog() {
        validate_committed_ledger(&crate::cmd::workspace_root(), &committed_descriptor()).unwrap();
    }

    #[test]
    fn committed_presets_and_surface_mappings_match_ktd4() {
        fn ids(values: &[&str]) -> BTreeSet<String> {
            values.iter().map(|value| (*value).to_string()).collect()
        }

        let descriptor = committed_descriptor();
        let capability_by_id = descriptor
            .capabilities
            .iter()
            .map(|capability| (capability.id.as_str(), capability))
            .collect::<BTreeMap<_, _>>();
        let preset_by_id = descriptor
            .presets
            .iter()
            .map(|preset| (preset.id.as_str(), preset))
            .collect::<BTreeMap<_, _>>();
        let effective =
            effective_preset_sets(&descriptor, &capability_by_id, &preset_by_id).unwrap();

        let expected = BTreeMap::from([
            (
                "preset-native-svg",
                ids(&[
                    "svg",
                    "layout-cytoscape",
                    "layout-elk",
                    "math",
                    "system-clock",
                    "system-timezone",
                    "system-random",
                    "system-timing",
                ]),
            ),
            (
                "preset-static-svg",
                ids(&["svg", "layout-cytoscape", "layout-elk", "math"]),
            ),
            ("preset-editor", ids(&["analysis", "editor"])),
            ("preset-ci-lint", ids(&["analysis"])),
            (
                "preset-native-sdk",
                ids(&[
                    "svg",
                    "analysis",
                    "ascii",
                    "png",
                    "jpeg",
                    "pdf",
                    "layout-cytoscape",
                    "layout-elk",
                    "math",
                    "system-clock",
                    "system-timezone",
                    "system-random",
                    "system-timing",
                ]),
            ),
            (
                "preset-mmdc",
                ids(&[
                    "svg",
                    "analysis",
                    "ascii",
                    "png",
                    "jpeg",
                    "pdf",
                    "layout-cytoscape",
                    "layout-elk",
                    "math",
                    "system-clock",
                    "system-timezone",
                    "system-random",
                    "system-timing",
                    "network-icons",
                    "parallel-markdown",
                    "shell-completions",
                ]),
            ),
            (
                "preset-all",
                ids(&[
                    "svg",
                    "analysis",
                    "editor",
                    "ascii",
                    "png",
                    "jpeg",
                    "pdf",
                    "layout-cytoscape",
                    "layout-elk",
                    "math",
                    "system-clock",
                    "system-timezone",
                    "system-random",
                    "system-timing",
                ]),
            ),
            ("preset-web-analysis", ids(&["analysis"])),
            (
                "preset-web-render",
                ids(&["svg", "layout-cytoscape", "layout-elk", "math"]),
            ),
            ("preset-web-editor", ids(&["analysis", "editor"])),
            ("preset-web-ascii", ids(&["ascii"])),
            (
                "preset-web-full",
                ids(&[
                    "svg",
                    "analysis",
                    "editor",
                    "ascii",
                    "layout-cytoscape",
                    "layout-elk",
                    "math",
                ]),
            ),
        ])
        .into_iter()
        .map(|(id, capabilities)| (id.to_string(), capabilities))
        .collect::<BTreeMap<_, _>>();
        assert_eq!(effective, expected);

        let mappings = descriptor
            .surface_mappings
            .iter()
            .map(|mapping| (mapping.id.as_str(), mapping))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            mappings.keys().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "typst-bridge",
                "typst-publish",
                "typst-svg",
                "web-analysis",
                "web-ascii",
                "web-editor",
                "web-full",
                "web-render",
            ])
        );
        for (id, artifact, preset) in [
            ("web-analysis", "@mermanjs/analysis", "preset-web-analysis"),
            ("web-render", "@mermanjs/render", "preset-web-render"),
            ("web-editor", "@mermanjs/editor", "preset-web-editor"),
            ("web-ascii", "@mermanjs/ascii", "preset-web-ascii"),
            ("web-full", "@mermanjs/web", "preset-web-full"),
        ] {
            assert_eq!(mappings[id].artifact, artifact);
            assert_eq!(mappings[id].preset.as_deref(), Some(preset));
        }
        assert_eq!(
            preset_by_id["preset-web-render"]
                .expected_runtime_capabilities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>(),
            ids(&[
                "svg",
                "layout-cytoscape",
                "layout-elk",
                "math",
                "browser-time",
                "browser-random",
                "browser-timing",
            ])
        );
        assert!(mappings["typst-bridge"].transport_only);
        assert!(mappings["typst-bridge"].capabilities.is_empty());
        assert_eq!(
            mappings["typst-bridge"].expected_runtime_capabilities,
            ["typst-transport"]
        );
        assert_eq!(mappings["typst-svg"].capabilities, ["svg"]);
        assert_eq!(
            mappings["typst-svg"]
                .expected_runtime_capabilities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>(),
            ids(&["svg", "typst-transport"])
        );
        assert_eq!(
            mappings["typst-publish"]
                .capabilities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>(),
            ids(&["svg", "analysis", "layout-cytoscape", "layout-elk", "math",])
        );
        assert_eq!(
            mappings["typst-publish"]
                .expected_runtime_capabilities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>(),
            ids(&[
                "svg",
                "analysis",
                "layout-cytoscape",
                "layout-elk",
                "math",
                "typst-transport",
            ])
        );
    }

    #[test]
    fn semantic_digest_excludes_migration_bookkeeping() {
        let descriptor = committed_descriptor();
        let mut migrated = descriptor.clone();
        migrated.migration_ledger.clear();
        assert_eq!(
            semantic_digest(&descriptor).unwrap(),
            semantic_digest(&migrated).unwrap()
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
    fn fixture_rejects_runtime_and_public_capability_id_collision() {
        let mut descriptor = committed_value();
        let index = capability_index(&descriptor, "svg");
        descriptor["capabilities"][index]["id"] =
            descriptor["runtime_capabilities"][0]["id"].clone();

        let error = validate_fixture(descriptor).expect_err("ambiguous capability ID must fail");
        assert!(
            error.contains(&format!(
                "capabilities[{index}].id: public capability `browser-time` duplicates a runtime-only capability ID"
            )),
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
    fn fixture_rejects_target_invalid_preset() {
        let mut descriptor = committed_value();
        let index = preset_index(&descriptor, "preset-native-svg");
        descriptor["presets"][index]["targets"] = json!(["typst"]);
        let error = validate_fixture(descriptor).expect_err("invalid target must fail");
        assert!(
            error.contains(&format!("presets[{index}].includes"))
                && error.contains("unavailable on target `typst`"),
            "unexpected diagnostic: {error}"
        );
    }

    #[test]
    fn fixture_rejects_leaf_without_measured_evidence() {
        let mut descriptor = committed_value();
        let index = capability_index(&descriptor, "svg");
        descriptor["capabilities"][index]["admission"]
            .as_object_mut()
            .unwrap()
            .remove("evidence");
        let error = validate_fixture(descriptor).expect_err("missing evidence must fail");
        assert!(
            error.contains(&format!("capabilities[{index}].admission.evidence")),
            "unexpected diagnostic: {error}"
        );
    }

    #[test]
    fn fixture_rejects_invalid_migration_ledger() {
        let mut descriptor = committed_value();
        descriptor["migration_ledger"][0]["legacy_catalogs"] = json!(["../outside.json"]);
        let error = validate_fixture(descriptor).expect_err("invalid ledger must fail");
        assert!(
            error.contains("migration_ledger[0].legacy_catalogs[0]"),
            "unexpected diagnostic: {error}"
        );
    }

    #[test]
    fn strict_mode_rejects_pending_ledger_and_unrecorded_old_catalog() {
        let descriptor = committed_descriptor();
        let temporary = tempfile::tempdir().unwrap();
        let error = validate_strict(temporary.path(), &descriptor).unwrap_err();
        assert!(error.contains("strict mode requires an empty ledger"));

        let mut cleared = descriptor;
        cleared.migration_ledger.clear();
        let error = validate_strict(temporary.path(), &cleared).unwrap_err();
        assert!(
            error.contains("admission.evidence.status") && error.contains("observed"),
            "unexpected diagnostic: {error}"
        );
        for capability in &mut cleared.capabilities {
            capability.admission.evidence.as_mut().unwrap().status = EvidenceStatus::Observed;
        }
        validate_strict(temporary.path(), &cleared).unwrap();

        let old_catalog = temporary.path().join(LEGACY_LIVE_CATALOG_GUARDS[0].1);
        fs::create_dir_all(old_catalog.parent().unwrap()).unwrap();
        fs::write(&old_catalog, "{}\n").unwrap();
        let error = validate_strict(temporary.path(), &cleared).unwrap_err();
        assert!(error.contains("strict.legacy_catalogs"));
    }
}
