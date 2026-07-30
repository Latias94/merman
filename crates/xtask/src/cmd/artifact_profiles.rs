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
const CARGO_DIST_PROFILE_IDS: [&str; 2] = ["cli-release", "lsp-stdio-release"];

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
    capabilities: Vec<String>,
    runtime_ids: Vec<String>,
    outputs: Vec<String>,
}

/// A validated WASM artifact recipe consumed by owner builds and the size matrix.
///
/// The matrix deliberately receives this projection only after the complete
/// artifact-profile descriptor has passed the same Cargo and capability checks
/// used for release recipes. It must not reconstruct a second Web feature map.
#[derive(Debug, Clone)]
pub(crate) struct WasmArtifactProfile {
    pub(crate) id: String,
    pub(crate) semantic_target: String,
    pub(crate) package: String,
    pub(crate) manifest_path: PathBuf,
    pub(crate) cargo_profile: String,
    pub(crate) default_features: bool,
    pub(crate) features: Vec<String>,
    pub(crate) target_triple: String,
    pub(crate) artifact_name: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) runtime_ids: Vec<String>,
    pub(crate) outputs: Vec<String>,
}

/// A validated exact artifact recipe compiled for the executing Rust host.
#[derive(Debug, Clone)]
pub(crate) struct HostArtifactProfile {
    pub(crate) id: String,
    pub(crate) package: String,
    pub(crate) manifest_path: PathBuf,
    pub(crate) default_features: bool,
    pub(crate) features: Vec<String>,
    pub(crate) target_name: String,
    pub(crate) target_kinds: Vec<String>,
}

impl WasmArtifactProfile {
    pub(crate) fn configure_cargo_command(&self, command: &mut Command) {
        command.arg("--manifest-path").arg(&self.manifest_path);
        if !self.default_features {
            command.arg("--no-default-features");
        }
        if !self.features.is_empty() {
            command.arg("--features").arg(self.features.join(","));
        }
    }
}

impl HostArtifactProfile {
    pub(crate) fn configure_cargo_command(&self, command: &mut Command) -> Result<(), String> {
        command
            .arg("--package")
            .arg(&self.package)
            .arg("--manifest-path")
            .arg(&self.manifest_path);
        let kinds = self
            .target_kinds
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if kinds == BTreeSet::from(["bin"]) {
            command.arg("--bin").arg(&self.target_name);
        } else if !kinds.is_disjoint(&BTreeSet::from([
            "lib",
            "proc-macro",
            "cdylib",
            "rlib",
            "staticlib",
        ])) {
            command.arg("--lib");
        } else {
            return Err(format!(
                "artifact profile `{}` has unsupported host target kinds {:?}",
                self.id, self.target_kinds
            ));
        }
        if !self.default_features {
            command.arg("--no-default-features");
        }
        if !self.features.is_empty() {
            command.arg("--features").arg(self.features.join(","));
        }
        Ok(())
    }
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
    metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CargoDistRecipe {
    default_features: bool,
    features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadataTarget {
    name: String,
    kind: Vec<String>,
    crate_types: Vec<String>,
    #[serde(rename = "required-features", default)]
    required_features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BindingOperationAuthority {
    binding_operations: Vec<CanonicalBindingOperation>,
}

#[derive(Debug, Clone, Deserialize)]
struct CanonicalBindingOperation {
    id: String,
    capability: Option<String>,
    targets: Vec<String>,
}

struct ValidationContext {
    root: PathBuf,
    capability: CapabilityContractCatalog,
    binding_operations: Vec<CanonicalBindingOperation>,
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
    let path = fs::canonicalize(path).map_err(|error| {
        format!(
            "cannot resolve Cargo manifest `{}`: {error}",
            path.display()
        )
    })?;
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
        let capability_source = fs::read_to_string(root.join(CAPABILITY_DESCRIPTOR_PATH))
            .map_err(|error| format!("cannot read capability authority: {error}"))?;
        let binding_operations =
            serde_json::from_str::<BindingOperationAuthority>(&capability_source)
                .map_err(|error| format!("cannot decode capability operations: {error}"))?
                .binding_operations;
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
            binding_operations,
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

fn validate_capability_feature_closure(
    profile: &ArtifactProfile,
    package: &CargoPackage,
    enabled: &BTreeSet<String>,
    catalog: &CapabilityContractCatalog,
    path: &str,
) -> Result<(), String> {
    let declared = profile
        .expected
        .capabilities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let fixed = package
        .metadata
        .get("merman")
        .and_then(|metadata| metadata.get("fixed-capabilities"))
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| {
                    format!(
                        "{}: package.metadata.merman.fixed-capabilities must be an array",
                        package.manifest_path.display()
                    )
                })?
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    value.as_str().ok_or_else(|| {
                        format!(
                            "{}: package.metadata.merman.fixed-capabilities[{index}] must be a string",
                            package.manifest_path.display()
                        )
                    })
                })
                .collect::<Result<BTreeSet<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    for capability in &fixed {
        if !catalog.capability_targets.contains_key(*capability) {
            return Err(format!(
                "{}: package.metadata.merman.fixed-capabilities contains unknown capability `{capability}`",
                package.manifest_path.display()
            ));
        }
    }
    let enabled_feature_capabilities = package
        .features
        .keys()
        .filter(|feature| catalog.capability_targets.contains_key(*feature))
        .filter(|feature| enabled.contains(*feature))
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual = fixed
        .union(&enabled_feature_capabilities)
        .copied()
        .collect::<BTreeSet<_>>();
    if actual != declared {
        let missing = declared.difference(&actual).copied().collect::<Vec<_>>();
        let undeclared = actual.difference(&declared).copied().collect::<Vec<_>>();
        return Err(format!(
            "{path}.expected.capabilities: must equal package fixed capabilities plus enabled same-named Cargo capability features; missing owners {missing:?}, undeclared enabled capabilities {undeclared:?}"
        ));
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct DerivedContractSurface {
    outputs: BTreeSet<String>,
    operation_ids: BTreeSet<String>,
}

fn derive_contract_surface(
    catalog: &CapabilityContractCatalog,
    binding_operations: &[CanonicalBindingOperation],
    target: &str,
    capabilities: &BTreeSet<String>,
) -> DerivedContractSurface {
    let outputs = catalog
        .output_capabilities
        .iter()
        .filter(|(output, capability)| {
            capabilities.contains(*capability) && catalog.output_targets[*output].contains(target)
        })
        .map(|(output, _)| output.clone())
        .collect();
    let operation_ids = binding_operations
        .iter()
        .filter(|operation| {
            operation
                .targets
                .iter()
                .any(|candidate| candidate == target)
                && operation
                    .capability
                    .as_ref()
                    .is_none_or(|capability| capabilities.contains(capability))
        })
        .map(|operation| operation.id.clone())
        .collect();

    DerivedContractSurface {
        outputs,
        operation_ids,
    }
}

fn validate_cargo_dist_recipe(
    profile: &ArtifactProfile,
    package: &CargoPackage,
    path: &str,
) -> Result<(), String> {
    if !CARGO_DIST_PROFILE_IDS.contains(&profile.id.as_str()) {
        return Ok(());
    }
    if profile.cargo.profile != "dist" {
        return Err(format!(
            "{path}.cargo.profile: cargo-dist artifact `{}` must use the `dist` profile",
            profile.id
        ));
    }

    let metadata_path = format!("{path}.cargo.package.metadata.dist");
    let metadata = package
        .metadata
        .get("dist")
        .ok_or_else(|| format!("{metadata_path}: missing exact cargo-dist recipe"))?;
    let recipe = serde_json::from_value::<CargoDistRecipe>(metadata.clone())
        .map_err(|error| format!("{metadata_path}: invalid cargo-dist recipe: {error}"))?;
    if recipe.default_features != profile.cargo.default_features {
        return Err(format!(
            "{metadata_path}.default-features: expected {}, found {}",
            profile.cargo.default_features, recipe.default_features
        ));
    }
    let actual_features =
        validate_sorted_unique(&recipe.features, &format!("{metadata_path}.features"))?;
    let expected_features = profile
        .cargo
        .features
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual_features != expected_features {
        return Err(format!(
            "{metadata_path}.features: exact artifact `{}` requires {expected_features:?}, found {actual_features:?}",
            profile.id
        ));
    }
    Ok(())
}

fn validate_expected(
    profile: &ArtifactProfile,
    catalog: &CapabilityContractCatalog,
    binding_operations: &[CanonicalBindingOperation],
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

    if capabilities != runtime_ids {
        return Err(format!(
            "{path}.expected.runtime_ids: runtime report must equal the compiled capability IDs"
        ));
    }
    for output in &outputs {
        let output_target = catalog
            .output_targets
            .get(output)
            .ok_or_else(|| format!("{path}.expected.outputs: unknown output `{output}`"))?;
        if !output_target.contains(target) {
            return Err(format!(
                "{path}.expected.outputs: `{output}` is unavailable on `{target}`"
            ));
        }
        let capability = &catalog.output_capabilities[output];
        if !capabilities.contains(capability) {
            return Err(format!(
                "{path}.expected.outputs: `{output}` requires capability `{capability}`"
            ));
        }
    }

    let derived = derive_contract_surface(catalog, binding_operations, target, &capabilities);
    if outputs != derived.outputs {
        return Err(format!(
            "{path}.expected.outputs: must equal outputs derived from the canonical operations for target `{target}` and capabilities {capabilities:?}; expected {:?}, found {outputs:?}",
            derived.outputs
        ));
    }
    let outputs_without_operations = outputs
        .difference(&derived.operation_ids)
        .cloned()
        .collect::<Vec<_>>();
    if !outputs_without_operations.is_empty() {
        return Err(format!(
            "{path}.expected.outputs: canonical outputs lack matching binding operations: {outputs_without_operations:?}"
        ));
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
    if profile.cargo.default_features {
        return Err(format!(
            "{path}.cargo.default_features: exact artifact recipes must disable Cargo defaults"
        ));
    }
    validate_sorted_unique(&profile.cargo.features, &format!("{path}.cargo.features"))?;
    for feature in &profile.cargo.features {
        if feature == "default" {
            return Err(format!(
                "{path}.cargo.features: exact artifact recipes must not enable the implicit `default` feature"
            ));
        }
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
    validate_capability_feature_closure(profile, package, &enabled, &context.capability, path)?;

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
    validate_cargo_dist_recipe(profile, package, path)?;
    validate_expected(
        profile,
        &context.capability,
        &context.binding_operations,
        path,
    )
}

fn package_requires_artifact_profile(
    package: &CargoPackage,
    manifest: &str,
) -> Result<bool, String> {
    let Some(merman) = package.metadata.get("merman") else {
        return Ok(false);
    };
    let metadata = merman.as_object().ok_or_else(|| {
        format!("{manifest}: package.metadata.merman must be a table when present")
    })?;
    let Some(required) = metadata.get("artifact-profile-required") else {
        return Ok(false);
    };
    match required.as_bool() {
        Some(true) => Ok(true),
        Some(false) => Err(format!(
            "{manifest}: package.metadata.merman.artifact-profile-required must be omitted or true"
        )),
        None => Err(format!(
            "{manifest}: package.metadata.merman.artifact-profile-required must be boolean true"
        )),
    }
}

fn validate_artifact_profile_coverage(
    descriptor: &ArtifactProfileDescriptor,
    packages: &BTreeMap<(String, String), CargoPackage>,
) -> Result<(), String> {
    let covered_packages = descriptor
        .profiles
        .iter()
        .map(|profile| {
            (
                profile.cargo.package.clone(),
                profile.cargo.manifest.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let required_packages = packages
        .iter()
        .filter_map(|((package, manifest), metadata)| {
            match package_requires_artifact_profile(metadata, manifest) {
                Ok(true) => Some(Ok((package.clone(), manifest.clone()))),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let missing = required_packages
        .difference(&covered_packages)
        .map(|(package, manifest)| format!("`{package}` at `{manifest}`"))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "profiles: Cargo build roots marked artifact-profile-required lack exact recipes: {}",
            missing.join(", ")
        ))
    }?;
    let unexpected = covered_packages
        .difference(&required_packages)
        .map(|(package, manifest)| format!("`{package}` at `{manifest}`"))
        .collect::<Vec<_>>();
    if unexpected.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "profiles: exact recipes reference Cargo packages without artifact-profile-required ownership: {}",
            unexpected.join(", ")
        ))
    }
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
    validate_artifact_profile_coverage(descriptor, &context.packages)?;
    for profile_id in CARGO_DIST_PROFILE_IDS {
        if !ids.contains(profile_id) {
            return Err(format!(
                "profiles: cargo-dist release requires exact artifact profile `{profile_id}`"
            ));
        }
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
    let wasm_profiles = wasm_artifact_profiles(&descriptor)?;
    validate_wasm_owner_closures(&wasm_profiles)?;
    println!(
        "validated {} exact artifact build profile(s)",
        descriptor.profiles.len()
    );
    Ok(())
}

/// Loads every exact artifact recipe that produces the canonical browser or
/// Typst WASM binary measured by `wasm-size-matrix`.
///
/// The descriptor is intentionally the only source for Cargo feature lists,
/// target triples, and expected capabilities. A profile targeting one of those
/// semantic surfaces must be a single `wasm32-unknown-unknown` cdylib recipe;
/// otherwise it cannot be measured as one stable `.wasm` artifact.
pub(crate) fn load_wasm_size_artifact_profiles() -> Result<Vec<WasmArtifactProfile>, String> {
    let root = super::workspace_root();
    let descriptor = read_descriptor(&root.join(ARTIFACT_PROFILE_DESCRIPTOR_PATH))?;
    let context = ValidationContext::load(&root)?;
    validate_descriptor(&descriptor, &context)?;

    let profiles = wasm_artifact_profiles(&descriptor)?;
    validate_wasm_owner_closures(&profiles)?;
    Ok(profiles)
}

/// Loads every validated exact recipe whose build target is the executing host.
pub(crate) fn load_host_artifact_profiles() -> Result<Vec<HostArtifactProfile>, String> {
    let root = super::workspace_root();
    let descriptor = read_descriptor(&root.join(ARTIFACT_PROFILE_DESCRIPTOR_PATH))?;
    let context = ValidationContext::load(&root)?;
    validate_descriptor(&descriptor, &context)?;

    Ok(descriptor
        .profiles
        .into_iter()
        .filter(|profile| matches!(&profile.cargo.build_target, BuildTarget::Host))
        .map(|profile| HostArtifactProfile {
            id: profile.id,
            package: profile.cargo.package,
            manifest_path: PathBuf::from(profile.cargo.manifest),
            default_features: profile.cargo.default_features,
            features: profile.cargo.features,
            target_name: profile.cargo.target.name,
            target_kinds: profile.cargo.target.kinds,
        })
        .collect())
}

/// Loads one validated exact WASM artifact recipe by descriptor ID.
///
/// Consumers use this instead of reconstructing Cargo package, target, and
/// feature arguments in workflow or release documentation.
pub(crate) fn load_exact_wasm_artifact_profile(
    profile_id: &str,
) -> Result<WasmArtifactProfile, String> {
    load_wasm_size_artifact_profiles()?
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| format!("unknown exact WASM artifact profile `{profile_id}`"))
}

fn wasm_artifact_profiles(
    descriptor: &ArtifactProfileDescriptor,
) -> Result<Vec<WasmArtifactProfile>, String> {
    let mut profiles = Vec::new();
    for profile in &descriptor.profiles {
        if !matches!(profile.semantic_target.as_str(), "web" | "typst") {
            continue;
        }

        if !profile
            .cargo
            .target
            .kinds
            .iter()
            .any(|kind| kind == "cdylib")
        {
            return Err(format!(
                "artifact profile `{}`: WASM size measurement requires a cdylib target",
                profile.id
            ));
        }
        let BuildTarget::TargetSet { triples } = &profile.cargo.build_target else {
            return Err(format!(
                "artifact profile `{}`: WASM size measurement requires a target-set recipe",
                profile.id
            ));
        };
        let [target_triple] = triples.as_slice() else {
            return Err(format!(
                "artifact profile `{}`: WASM size measurement requires exactly one target triple",
                profile.id
            ));
        };
        if target_triple != "wasm32-unknown-unknown" {
            return Err(format!(
                "artifact profile `{}`: WASM size measurement requires `wasm32-unknown-unknown`, found `{target_triple}`",
                profile.id
            ));
        }

        profiles.push(WasmArtifactProfile {
            id: profile.id.clone(),
            semantic_target: profile.semantic_target.clone(),
            package: profile.cargo.package.clone(),
            manifest_path: PathBuf::from(&profile.cargo.manifest),
            cargo_profile: profile.cargo.profile.clone(),
            default_features: profile.cargo.default_features,
            features: profile.cargo.features.clone(),
            target_triple: target_triple.clone(),
            artifact_name: format!("{}.wasm", profile.cargo.target.name),
            capabilities: profile.expected.capabilities.clone(),
            runtime_ids: profile.expected.runtime_ids.clone(),
            outputs: profile.expected.outputs.clone(),
        });
    }

    if profiles.is_empty() {
        return Err("artifact profiles: no Web or Typst WASM recipes are declared".to_string());
    }
    Ok(profiles)
}

fn validate_wasm_owner_closures(profiles: &[WasmArtifactProfile]) -> Result<(), String> {
    let typst_catalog = super::typst_profiles::load_typst_profiles()
        .map_err(|error| format!("Typst artifact profile owner: {error}"))?;
    super::typst_profiles::validate_typst_artifact_profiles(&typst_catalog, profiles)
        .map_err(|error| format!("Typst artifact profile owner: {error}"))
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

    #[test]
    fn relative_manifest_normalizes_equivalent_path_spellings() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("workspace");
        fs::create_dir_all(root.join("nested")).expect("workspace directories");
        fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("workspace manifest");

        let canonical_root = fs::canonicalize(&root).expect("canonical workspace root");
        let alternate_manifest = root.join("nested").join("..").join("Cargo.toml");

        assert_eq!(
            relative_manifest(&canonical_root, &alternate_manifest).unwrap(),
            "Cargo.toml"
        );
    }

    fn validate_fixture(value: Value) -> Result<(), String> {
        let descriptor = serde_json::from_value::<ArtifactProfileDescriptor>(value)
            .map_err(|error| format!("descriptor schema: {error}"))?;
        validate_descriptor(&descriptor, context())
    }

    fn committed_descriptor() -> ArtifactProfileDescriptor {
        serde_json::from_value(committed_value()).expect("committed descriptor schema")
    }

    fn packages_with_merman_metadata(
        package_name: &str,
        metadata: Value,
    ) -> BTreeMap<(String, String), CargoPackage> {
        let mut packages = context().packages.clone();
        let package = packages
            .iter_mut()
            .find_map(|((name, _), package)| (name == package_name).then_some(package))
            .expect("workspace package");
        package.metadata["merman"] = metadata;
        packages
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
        validate_fixture(value).unwrap();
    }

    #[test]
    fn export_profiles_report_only_the_outputs_they_expose() {
        let value = committed_value();
        for (profile_id, expected) in [
            ("rust-export-jpeg", json!(["jpeg"])),
            ("rust-export-native-sdk", json!(["jpeg", "pdf", "png"])),
            ("rust-export-pdf", json!(["pdf"])),
            ("rust-export-png", json!(["png"])),
        ] {
            let index = profile_index(&value, profile_id);
            assert_eq!(
                value["profiles"][index]["expected"]["capabilities"],
                expected
            );
            assert_eq!(
                value["profiles"][index]["expected"]["runtime_ids"],
                expected
            );
            assert_eq!(value["profiles"][index]["expected"]["outputs"], expected);
        }
    }

    #[test]
    fn rustdoc_profile_selects_renderer_engines_as_direct_leaves() {
        let value = committed_value();
        let index = profile_index(&value, "rustdoc-static-svg");
        assert_eq!(
            value["profiles"][index]["cargo"]["features"],
            json!(["layout-cytoscape", "layout-elk", "math", "svg"])
        );
        assert_eq!(
            value["profiles"][index]["expected"]["capabilities"],
            json!(["layout-cytoscape", "layout-elk", "math", "svg"])
        );
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
    fn schema_rejects_removed_preset_binding() {
        let mut value = committed_value();
        let index = profile_index(&value, "cli-release");
        value["profiles"][index]["expected"]["preset"] = json!("preset-mmdc");
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("unknown field `preset`"), "{error}");
    }

    #[test]
    fn rejects_cargo_defaults_for_every_exact_recipe() {
        let mut value = committed_value();
        let index = profile_index(&value, "lsp-library");
        value["profiles"][index]["cargo"]["default_features"] = json!(true);
        let error = validate_fixture(value).unwrap_err();
        assert!(
            error.contains("exact artifact recipes must disable Cargo defaults"),
            "{error}"
        );
    }

    #[test]
    fn rejects_explicit_default_feature_in_an_exact_recipe() {
        let mut value = committed_value();
        let index = profile_index(&value, "rust-native-svg");
        value["profiles"][index]["cargo"]["features"] = json!(["default"]);
        let error = validate_fixture(value).unwrap_err();
        assert!(
            error.contains("must not enable the implicit `default` feature"),
            "{error}"
        );
    }

    #[test]
    fn rejects_missing_capability_bearing_package_recipe() {
        let mut value = committed_value();
        let index = profile_index(&value, "rust-ascii");
        value["profiles"].as_array_mut().unwrap().remove(index);
        let error = validate_fixture(value).unwrap_err();
        assert!(
            error.contains("artifact-profile-required lack exact recipes"),
            "{error}"
        );
        assert!(error.contains("merman-ascii"), "{error}");
    }

    #[test]
    fn rejects_malformed_artifact_profile_owner_table() {
        let packages = packages_with_merman_metadata("merman-ascii", json!("required"));
        let error =
            validate_artifact_profile_coverage(&committed_descriptor(), &packages).unwrap_err();
        assert!(
            error.contains("package.metadata.merman must be a table"),
            "{error}"
        );
    }

    #[test]
    fn rejects_malformed_artifact_profile_required_marker() {
        let packages = packages_with_merman_metadata(
            "merman-ascii",
            json!({"artifact-profile-required": "yes"}),
        );
        let error =
            validate_artifact_profile_coverage(&committed_descriptor(), &packages).unwrap_err();
        assert!(error.contains("must be boolean true"), "{error}");
    }

    #[test]
    fn rejects_profile_for_package_without_owner_marker() {
        let packages = packages_with_merman_metadata("merman-ascii", json!({}));
        let error =
            validate_artifact_profile_coverage(&committed_descriptor(), &packages).unwrap_err();
        assert!(
            error.contains("without artifact-profile-required ownership"),
            "{error}"
        );
        assert!(error.contains("merman-ascii"), "{error}");
    }

    #[test]
    fn rejects_cargo_dist_feature_drift_from_exact_release_profile() {
        let mut value = committed_value();
        let index = profile_index(&value, "cli-release");
        for field in [
            &["cargo", "features"][..],
            &["expected", "capabilities"][..],
            &["expected", "runtime_ids"][..],
        ] {
            value["profiles"][index][field[0]][field[1]]
                .as_array_mut()
                .unwrap()
                .retain(|item| item != "system-timing");
        }
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("package.metadata.dist.features"), "{error}");
    }

    #[test]
    fn rejects_cargo_dist_profile_drift_from_exact_release_profile() {
        let mut value = committed_value();
        let index = profile_index(&value, "cli-release");
        value["profiles"][index]["cargo"]["profile"] = json!("release");
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("must use the `dist` profile"), "{error}");
    }

    #[test]
    fn rejects_output_without_its_capability() {
        let mut value = committed_value();
        let index = profile_index(&value, "web-ascii");
        value["profiles"][index]["cargo"]["features"]
            .as_array_mut()
            .unwrap()
            .retain(|item| item != "ascii");
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
    fn rejects_declared_capability_removed_from_the_cargo_feature_closure() {
        let mut value = committed_value();
        let index = profile_index(&value, "rust-all");
        value["profiles"][index]["cargo"]["features"]
            .as_array_mut()
            .unwrap()
            .retain(|feature| feature != "editor");
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("missing owners [\"editor\"]"), "{error}");
    }

    #[test]
    fn rejects_enabled_cargo_capability_omitted_from_the_declaration() {
        let mut value = committed_value();
        let index = profile_index(&value, "rust-editor-facade");
        value["profiles"][index]["expected"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .retain(|capability| capability != "analysis");
        value["profiles"][index]["expected"]["runtime_ids"]
            .as_array_mut()
            .unwrap()
            .retain(|capability| capability != "analysis");
        let error = validate_fixture(value).unwrap_err();
        assert!(
            error.contains("undeclared enabled capabilities [\"analysis\"]"),
            "{error}"
        );
    }

    #[test]
    fn rejects_capability_not_owned_by_the_artifact_package() {
        let mut value = committed_value();
        let index = profile_index(&value, "rust-core");
        value["profiles"][index]["expected"]["capabilities"] = json!(["analysis"]);
        value["profiles"][index]["expected"]["runtime_ids"] = json!(["analysis"]);
        let error = validate_fixture(value).unwrap_err();
        assert!(error.contains("missing owners [\"analysis\"]"), "{error}");
    }

    #[test]
    fn rejects_output_omitted_from_the_canonical_capability_surface() {
        let mut value = committed_value();
        let index = profile_index(&value, "rust-svg-basic");
        value["profiles"][index]["expected"]["outputs"] = json!([]);
        let error = validate_fixture(value).unwrap_err();
        assert!(
            error.contains("must equal outputs derived from the canonical operations"),
            "{error}"
        );
        assert!(error.contains("\"svg\""), "{error}");
    }

    #[test]
    fn derives_invariant_and_capability_gated_operations_from_one_authority() {
        let catalog = &context().capability;
        let surface = derive_contract_surface(
            catalog,
            &context().binding_operations,
            "native",
            &BTreeSet::from(["analysis".to_string(), "svg".to_string()]),
        );

        assert!(surface.operation_ids.contains("semantic-json"));
        assert!(surface.operation_ids.contains("svg"));
        assert!(surface.operation_ids.contains("analysis-json"));
        assert!(!surface.operation_ids.contains("png"));
        assert_eq!(surface.outputs, BTreeSet::from(["svg".to_string()]));
    }

    #[test]
    fn rejects_unknown_rust_target() {
        let mut value = committed_value();
        let index = profile_index(&value, "web-full");
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
