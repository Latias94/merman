//! Exact Cargo recipes for capability-bearing artifacts.

use super::capability_surface::CapabilityContractCatalog;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub(super) const ARTIFACT_PROFILE_DESCRIPTOR_PATH: &str = "capabilities/artifact-profiles-v1.json";
const ARTIFACT_PROFILE_SCHEMA_VERSION: u32 = 1;
const CAPABILITY_DESCRIPTOR_PATH: &str = "capabilities/feature-surface-v1.json";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactProfileDescriptor {
    schema_version: u32,
    descriptor_id: String,
    capability_authority: AuthorityReference,
    profiles: Vec<ArtifactProfile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityReference {
    path: String,
    schema_version: u32,
    digest: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactProfile {
    id: String,
    description: String,
    semantic_target: String,
    cargo: CargoRecipe,
    expected: ExpectedSurface,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CargoRecipe {
    package: String,
    manifest: String,
    profile: String,
    default_features: bool,
    features: Vec<String>,
    target: CargoTarget,
    build_target: BuildTarget,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CargoTarget {
    name: String,
    kinds: Vec<String>,
    crate_types: Vec<String>,
    required_features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum BuildTarget {
    Host,
    TargetSet { triples: Vec<String> },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedSurface {
    #[serde(default)]
    preset: Option<String>,
    capabilities: Vec<String>,
    runtime_ids: Vec<String>,
    outputs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoPackage {
    name: String,
    manifest_path: PathBuf,
    features: BTreeMap<String, Vec<String>>,
    targets: Vec<CargoMetadataTarget>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadataTarget {
    name: String,
    kind: Vec<String>,
    crate_types: Vec<String>,
    #[serde(rename = "required-features", default)]
    required_features: Vec<String>,
}

struct ValidationContext {
    root: PathBuf,
    capability: CapabilityContractCatalog,
    packages: BTreeMap<(String, String), CargoPackage>,
    cargo_profiles: BTreeSet<String>,
    rust_targets: BTreeSet<String>,
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

fn validate_sorted_unique(values: &[String], path: &str) -> Result<BTreeSet<String>, String> {
    let mut set = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        require_non_empty(value, &format!("{path}[{index}]"))?;
        if !set.insert(value.clone()) {
            return Err(format!("{path}[{index}]: duplicate value `{value}`"));
        }
    }
    if values.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(format!("{path}: values must be sorted"));
    }
    Ok(set)
}

fn validate_repo_file(root: &Path, relative: &str, path: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path.as_os_str().is_empty()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{path}: `{relative}` must be a normalized repository-relative file path"
        ));
    }
    let mut current = root.to_path_buf();
    for component in relative_path.components() {
        let Component::Normal(component) = component else {
            unreachable!("path components were validated")
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("{path}: cannot inspect `{relative}`: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{path}: `{relative}` must not traverse a symlink"));
        }
    }
    if !current.is_file() {
        return Err(format!("{path}: `{relative}` must name a regular file"));
    }
    Ok(current)
}

fn read_descriptor(path: &Path) -> Result<ArtifactProfileDescriptor, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
    serde_json::from_str(&source)
        .map_err(|error| format!("{}: descriptor schema error: {error}", path.display()))
}

fn relative_manifest(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map_err(|_| {
            format!(
                "Cargo manifest `{}` is outside the workspace",
                path.display()
            )
        })
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

impl ValidationContext {
    fn load(root: &Path) -> Result<Self, String> {
        let root = fs::canonicalize(root).map_err(|error| {
            format!(
                "cannot resolve workspace root `{}`: {error}",
                root.display()
            )
        })?;
        let capability = super::capability_surface::load_capability_contract_catalog(&root)
            .map_err(|error| format!("capability authority: {error}"))?;
        let output = Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .current_dir(&root)
            .output()
            .map_err(|error| format!("cannot execute `cargo metadata`: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "`cargo metadata` failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("cannot decode `cargo metadata`: {error}"))?;
        let mut packages = BTreeMap::new();
        for package in metadata.packages {
            let manifest = relative_manifest(&root, &package.manifest_path)?;
            packages.insert((package.name.clone(), manifest), package);
        }

        let root_manifest = fs::read_to_string(root.join("Cargo.toml"))
            .map_err(|error| format!("cannot read workspace Cargo.toml: {error}"))?;
        let root_manifest = toml::from_str::<toml::Table>(&root_manifest)
            .map_err(|error| format!("cannot parse workspace Cargo.toml: {error}"))?;
        let mut cargo_profiles = BTreeSet::from([
            "bench".to_string(),
            "dev".to_string(),
            "release".to_string(),
            "test".to_string(),
        ]);
        if let Some(profiles) = root_manifest.get("profile").and_then(toml::Value::as_table) {
            cargo_profiles.extend(profiles.keys().cloned());
        }

        let output = Command::new("rustc")
            .args(["--print", "target-list"])
            .current_dir(&root)
            .output()
            .map_err(|error| format!("cannot execute `rustc --print target-list`: {error}"))?;
        if !output.status.success() {
            return Err("`rustc --print target-list` failed".to_string());
        }
        let rust_targets = String::from_utf8(output.stdout)
            .map_err(|error| format!("rustc target list is not UTF-8: {error}"))?
            .lines()
            .map(str::to_string)
            .collect();

        Ok(Self {
            root,
            capability,
            packages,
            cargo_profiles,
            rust_targets,
        })
    }
}

fn expand_local_feature(
    name: &str,
    features: &BTreeMap<String, Vec<String>>,
    enabled: &mut BTreeSet<String>,
) {
    if !enabled.insert(name.to_string()) {
        return;
    }
    let Some(edges) = features.get(name) else {
        return;
    };
    for edge in edges {
        let candidate = edge.strip_suffix('?').unwrap_or(edge);
        if features.contains_key(candidate) {
            expand_local_feature(candidate, features, enabled);
        }
    }
}

fn enabled_local_features(package: &CargoPackage, recipe: &CargoRecipe) -> BTreeSet<String> {
    let mut enabled = BTreeSet::new();
    if recipe.default_features {
        expand_local_feature("default", &package.features, &mut enabled);
    }
    for feature in &recipe.features {
        expand_local_feature(feature, &package.features, &mut enabled);
    }
    enabled
}

fn validate_expected(
    profile: &ArtifactProfile,
    catalog: &CapabilityContractCatalog,
    path: &str,
) -> Result<(), String> {
    let target = &profile.semantic_target;
    if !catalog.target_ids.contains(target) {
        return Err(format!("{path}.semantic_target: unknown target `{target}`"));
    }
    let capabilities = validate_sorted_unique(
        &profile.expected.capabilities,
        &format!("{path}.expected.capabilities"),
    )?;
    let runtime_ids = validate_sorted_unique(
        &profile.expected.runtime_ids,
        &format!("{path}.expected.runtime_ids"),
    )?;
    let outputs = validate_sorted_unique(
        &profile.expected.outputs,
        &format!("{path}.expected.outputs"),
    )?;

    for capability in &capabilities {
        let targets = catalog.capability_targets.get(capability).ok_or_else(|| {
            format!("{path}.expected.capabilities: unknown capability `{capability}`")
        })?;
        if !targets.contains(target) {
            return Err(format!(
                "{path}.expected.capabilities: `{capability}` is unavailable on `{target}`"
            ));
        }
        for implied in &catalog.capability_implications[capability] {
            if !capabilities.contains(implied) {
                return Err(format!(
                    "{path}.expected.capabilities: `{capability}` requires `{implied}`"
                ));
            }
        }
    }

    if let Some(preset_id) = profile.expected.preset.as_deref() {
        let preset = catalog
            .presets
            .get(preset_id)
            .ok_or_else(|| format!("{path}.expected.preset: unknown preset `{preset_id}`"))?;
        if !preset.targets.contains(target) {
            return Err(format!(
                "{path}.expected.preset: `{preset_id}` is unavailable on `{target}`"
            ));
        }
        if preset.capabilities != capabilities {
            return Err(format!(
                "{path}.expected.capabilities: must equal preset `{preset_id}`"
            ));
        }
        if preset.expected_runtime_capabilities != runtime_ids {
            return Err(format!(
                "{path}.expected.runtime_ids: must equal preset `{preset_id}`"
            ));
        }
    }

    if !capabilities.is_subset(&runtime_ids) {
        return Err(format!(
            "{path}.expected.runtime_ids: runtime report must include every compiled capability"
        ));
    }
    for runtime_id in runtime_ids.difference(&capabilities) {
        let targets = catalog
            .runtime_capability_targets
            .get(runtime_id)
            .ok_or_else(|| {
                format!("{path}.expected.runtime_ids: unknown runtime ID `{runtime_id}`")
            })?;
        if !targets.contains(target) {
            return Err(format!(
                "{path}.expected.runtime_ids: `{runtime_id}` is unavailable on `{target}`"
            ));
        }
    }
    for output in outputs {
        let output_target = catalog
            .output_targets
            .get(&output)
            .ok_or_else(|| format!("{path}.expected.outputs: unknown output `{output}`"))?;
        if !output_target.contains(target) {
            return Err(format!(
                "{path}.expected.outputs: `{output}` is unavailable on `{target}`"
            ));
        }
        let capability = &catalog.output_capabilities[&output];
        if !capabilities.contains(capability) {
            return Err(format!(
                "{path}.expected.outputs: `{output}` requires capability `{capability}`"
            ));
        }
    }
    Ok(())
}

fn validate_profile(
    profile: &ArtifactProfile,
    context: &ValidationContext,
    path: &str,
) -> Result<(), String> {
    validate_kebab_id(&profile.id, &format!("{path}.id"))?;
    require_non_empty(&profile.description, &format!("{path}.description"))?;
    require_non_empty(&profile.cargo.package, &format!("{path}.cargo.package"))?;
    validate_repo_file(
        &context.root,
        &profile.cargo.manifest,
        &format!("{path}.cargo.manifest"),
    )?;
    let key = (
        profile.cargo.package.clone(),
        profile.cargo.manifest.clone(),
    );
    let package = context.packages.get(&key).ok_or_else(|| {
        format!(
            "{path}.cargo: no workspace package `{}` at `{}`",
            profile.cargo.package, profile.cargo.manifest
        )
    })?;
    if !context.cargo_profiles.contains(&profile.cargo.profile) {
        return Err(format!(
            "{path}.cargo.profile: unknown Cargo profile `{}`",
            profile.cargo.profile
        ));
    }
    validate_sorted_unique(&profile.cargo.features, &format!("{path}.cargo.features"))?;
    for feature in &profile.cargo.features {
        if !package.features.contains_key(feature) {
            return Err(format!(
                "{path}.cargo.features: package `{}` has no feature `{feature}`",
                package.name
            ));
        }
    }

    let target = package
        .targets
        .iter()
        .find(|candidate| candidate.name == profile.cargo.target.name)
        .ok_or_else(|| {
            format!(
                "{path}.cargo.target.name: package `{}` has no target `{}`",
                package.name, profile.cargo.target.name
            )
        })?;
    let expected_kinds = validate_sorted_unique(
        &profile.cargo.target.kinds,
        &format!("{path}.cargo.target.kinds"),
    )?;
    let actual_kinds = target.kind.iter().cloned().collect::<BTreeSet<_>>();
    if expected_kinds != actual_kinds {
        return Err(format!(
            "{path}.cargo.target.kinds: expected {expected_kinds:?}, found {actual_kinds:?}"
        ));
    }
    let expected_crate_types = validate_sorted_unique(
        &profile.cargo.target.crate_types,
        &format!("{path}.cargo.target.crate_types"),
    )?;
    let actual_crate_types = target.crate_types.iter().cloned().collect::<BTreeSet<_>>();
    if expected_crate_types != actual_crate_types {
        return Err(format!(
            "{path}.cargo.target.crate_types: expected {expected_crate_types:?}, found {actual_crate_types:?}"
        ));
    }
    let expected_required = validate_sorted_unique(
        &profile.cargo.target.required_features,
        &format!("{path}.cargo.target.required_features"),
    )?;
    let actual_required = target
        .required_features
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected_required != actual_required {
        return Err(format!(
            "{path}.cargo.target.required_features: expected {expected_required:?}, found {actual_required:?}"
        ));
    }
    let enabled = enabled_local_features(package, &profile.cargo);
    for required in expected_required {
        if !enabled.contains(&required) {
            return Err(format!(
                "{path}.cargo.target.required_features: `{required}` is not enabled"
            ));
        }
    }

    if let BuildTarget::TargetSet { triples } = &profile.cargo.build_target {
        if triples.is_empty() {
            return Err(format!(
                "{path}.cargo.build_target.triples: must not be empty"
            ));
        }
        validate_sorted_unique(triples, &format!("{path}.cargo.build_target.triples"))?;
        for triple in triples {
            if !context.rust_targets.contains(triple) {
                return Err(format!(
                    "{path}.cargo.build_target.triples: unknown rustc target `{triple}`"
                ));
            }
        }
    }
    validate_expected(profile, &context.capability, path)
}

fn validate_descriptor(
    descriptor: &ArtifactProfileDescriptor,
    context: &ValidationContext,
) -> Result<(), String> {
    if descriptor.schema_version != ARTIFACT_PROFILE_SCHEMA_VERSION {
        return Err(format!(
            "schema_version: expected {ARTIFACT_PROFILE_SCHEMA_VERSION}, found {}",
            descriptor.schema_version
        ));
    }
    validate_kebab_id(&descriptor.descriptor_id, "descriptor_id")?;
    if descriptor.capability_authority.path != CAPABILITY_DESCRIPTOR_PATH {
        return Err(format!(
            "capability_authority.path: expected `{CAPABILITY_DESCRIPTOR_PATH}`"
        ));
    }
    validate_repo_file(
        &context.root,
        &descriptor.capability_authority.path,
        "capability_authority.path",
    )?;
    if descriptor.capability_authority.schema_version != context.capability.schema_version {
        return Err(format!(
            "capability_authority.schema_version: expected {}, found {}",
            context.capability.schema_version, descriptor.capability_authority.schema_version
        ));
    }
    if descriptor.capability_authority.digest != context.capability.digest {
        return Err(format!(
            "capability_authority.digest: expected {}, found {}",
            context.capability.digest, descriptor.capability_authority.digest
        ));
    }
    if descriptor.profiles.is_empty() {
        return Err("profiles: must not be empty".to_string());
    }
    let mut ids = BTreeSet::new();
    let mut covered_targets = BTreeSet::new();
    for (index, profile) in descriptor.profiles.iter().enumerate() {
        let path = format!("profiles[{index}]");
        if !ids.insert(profile.id.as_str()) {
            return Err(format!("{path}.id: duplicate profile `{}`", profile.id));
        }
        if index > 0 && descriptor.profiles[index - 1].id > profile.id {
            return Err("profiles: entries must be sorted by ID".to_string());
        }
        validate_profile(profile, context, &path)?;
        covered_targets.insert(profile.semantic_target.as_str());
    }
    let expected_targets = context
        .capability
        .target_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if covered_targets != expected_targets {
        return Err(format!(
            "profiles: every semantic target needs at least one concrete build recipe; expected {expected_targets:?}, found {covered_targets:?}"
        ));
    }
    Ok(())
}

pub(crate) fn verify_artifact_profiles(args: Vec<String>) -> Result<(), String> {
    let root = super::workspace_root();
    let mut descriptor_path = root.join(ARTIFACT_PROFILE_DESCRIPTOR_PATH);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--descriptor" => {
                index += 1;
                descriptor_path = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "missing path after --descriptor".to_string())?,
                );
            }
            "--help" | "-h" => {
                println!("usage: xtask verify-artifact-profiles [--descriptor <path>]");
                return Ok(());
            }
            argument => return Err(format!("unknown argument `{argument}`")),
        }
        index += 1;
    }
    let descriptor = read_descriptor(&descriptor_path)?;
    let context = ValidationContext::load(&root)?;
    validate_descriptor(&descriptor, &context)?;
    println!(
        "validated {} exact artifact build profile(s)",
        descriptor.profiles.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::sync::OnceLock;

    fn committed_value() -> Value {
        let path = super::super::workspace_root().join(ARTIFACT_PROFILE_DESCRIPTOR_PATH);
        serde_json::from_str(&fs::read_to_string(path).expect("committed descriptor"))
            .expect("valid JSON")
    }

    fn context() -> &'static ValidationContext {
        static CONTEXT: OnceLock<ValidationContext> = OnceLock::new();
        CONTEXT.get_or_init(|| ValidationContext::load(&super::super::workspace_root()).unwrap())
    }

    fn validate_fixture(value: Value) -> Result<(), String> {
        let descriptor = serde_json::from_value::<ArtifactProfileDescriptor>(value)
            .map_err(|error| format!("descriptor schema: {error}"))?;
        validate_descriptor(&descriptor, context())
    }

    fn profile_index(value: &Value, id: &str) -> usize {
        value["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .position(|profile| profile["id"] == id)
            .unwrap()
    }

    #[test]
    fn committed_profiles_match_current_cargo_and_capability_authorities() {
        let value = committed_value();
        assert_eq!(value["profiles"].as_array().unwrap().len(), 12);
        validate_fixture(value).unwrap();
    }

    #[test]
    fn rejects_unknown_cargo_feature() {
        let mut value = committed_value();
        let index = profile_index(&value, "typst-wasm");
        value["profiles"][index]["cargo"]["features"] = json!(["not-a-feature"]);
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("has no feature `not-a-feature`"), "{error}");
    }

    #[test]
    fn rejects_preset_capability_drift() {
        let mut value = committed_value();
        let index = profile_index(&value, "cli-release");
        value["profiles"][index]["expected"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("must equal preset `preset-mmdc`"), "{error}");
    }

    #[test]
    fn rejects_output_without_its_capability() {
        let mut value = committed_value();
        let index = profile_index(&value, "web-wasm");
        value["profiles"][index]["expected"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .retain(|item| item != "ascii");
        value["profiles"][index]["expected"]["runtime_ids"]
            .as_array_mut()
            .unwrap()
            .retain(|item| item != "ascii");
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("requires capability `ascii`"), "{error}");
    }

    #[test]
    fn rejects_unknown_rust_target() {
        let mut value = committed_value();
        let index = profile_index(&value, "web-wasm");
        value["profiles"][index]["cargo"]["build_target"]["triples"] = json!(["not-a-rust-target"]);
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("unknown rustc target"), "{error}");
    }

    #[test]
    fn rejects_duplicate_profile() {
        let mut value = committed_value();
        let duplicate = value["profiles"][0].clone();
        value["profiles"]
            .as_array_mut()
            .unwrap()
            .insert(1, duplicate);
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("duplicate profile"), "{error}");
    }

    #[test]
    fn schema_rejects_transport_and_release_bookkeeping() {
        let mut value = committed_value();
        value["transport_authority"] = json!({"path": "README.md"});
        let error = validate_fixture(value).unwrap_err();
        assert!(
            error.contains("unknown field `transport_authority`"),
            "{error}"
        );
    }

    #[test]
    fn schema_rejects_manual_observation_state() {
        let mut value = committed_value();
        let index = profile_index(&value, "rust-core");
        value["profiles"][index]["state"] = json!("observed");
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("unknown field `state`"), "{error}");
    }
}
