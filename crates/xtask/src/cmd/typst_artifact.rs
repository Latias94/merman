use crate::{
    XtaskError,
    cmd::{
        MERMAID_SOURCE_COMMIT,
        artifact_profiles::WasmArtifactProfile,
        paths,
        typst_profiles::{
            TypstPackageProfile, TypstProfileCatalog, validate_typst_artifact_profiles,
        },
    },
    util::sha256_hex,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const MANIFEST_SCHEMA_VERSION: u32 = 3;
const ARTIFACT_ROOT_NAME: &str = "typst-wasm-artifacts";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const TARGET_TRIPLE: &str = "wasm32-unknown-unknown";
const CARGO_PROFILE: &str = "wasm-size";
const WASM_OPT_VERSION: &str = "wasm-opt version 131";
const WASM_OPT_FEATURES: [&str; 7] = [
    "--enable-bulk-memory",
    "--enable-bulk-memory-opt",
    "--enable-multivalue",
    "--enable-mutable-globals",
    "--enable-nontrapping-float-to-int",
    "--enable-reference-types",
    "--enable-sign-ext",
];

#[derive(Debug, Clone)]
pub(crate) struct TypstArtifactSpec {
    workspace_root: PathBuf,
    target_root: PathBuf,
    artifact_profile: String,
    profile: String,
    default_features: bool,
    features: Vec<String>,
    cargo_package: String,
    cargo_manifest_path: PathBuf,
    artifact_name: String,
    cargo_package_version: String,
    typst_package_version: String,
    plugin_abi_version: u32,
    mermaid_version: String,
    mermaid_source_commit: String,
}

#[derive(Debug)]
pub(crate) struct VerifiedTypstArtifact {
    wasm_path: PathBuf,
    manifest_path: PathBuf,
    _lock: ProfileArtifactLock,
}

#[derive(Debug, Deserialize)]
struct WorkspaceManifest {
    workspace: WorkspaceTable,
}

#[derive(Debug, Deserialize)]
struct WorkspaceTable {
    package: WorkspacePackage,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackage {
    version: String,
}

#[derive(Debug, Deserialize)]
struct TypstPackageManifest {
    package: TypstPackageTable,
}

#[derive(Debug, Deserialize)]
struct TypstPackageTable {
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactManifest {
    schema_version: u32,
    artifact_profile: String,
    profile: String,
    default_features: bool,
    features: Vec<String>,
    target_triple: String,
    cargo_profile: String,
    cargo_package: String,
    cargo_package_version: String,
    typst_package_version: String,
    plugin_abi_version: u32,
    mermaid_version: String,
    mermaid_source_commit: String,
    input: InputFingerprint,
    tools: ToolFingerprint,
    artifact: ArtifactFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InputFingerprint {
    sha256: String,
    packages: Vec<InputPackageFingerprint>,
    files: Vec<InputFileFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InputPackageFingerprint {
    name: String,
    manifest_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InputFileFingerprint {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolFingerprint {
    sha256: String,
    cargo_version: String,
    rustc_version: String,
    wasm_opt_version: String,
    wasm_tools_version: String,
    rustflags: Option<String>,
    cargo_encoded_rustflags: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolIdentity {
    cargo_version: String,
    rustc_version: String,
    wasm_opt_version: String,
    wasm_tools_version: String,
    rustflags: Option<String>,
    cargo_encoded_rustflags: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactFingerprint {
    file: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadataOutput {
    packages: Vec<MetadataPackage>,
    resolve: Option<MetadataResolve>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetadataPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    source: Option<String>,
    targets: Vec<MetadataTarget>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetadataTarget {
    kind: Vec<String>,
    src_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct MetadataResolve {
    root: Option<String>,
    nodes: Vec<MetadataNode>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetadataNode {
    id: String,
    deps: Vec<MetadataNodeDependency>,
    features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetadataNodeDependency {
    pkg: String,
    dep_kinds: Vec<MetadataDependencyKind>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetadataDependencyKind {
    kind: Option<String>,
}

#[derive(Debug)]
struct ProfileArtifactLock {
    _file: File,
}

trait ArtifactRuntime {
    fn cargo_metadata(
        &mut self,
        spec: &TypstArtifactSpec,
    ) -> Result<CargoMetadataOutput, XtaskError>;
    fn tool_identity(&mut self) -> Result<ToolIdentity, XtaskError>;
    fn optimize_wasm(&mut self, input: &Path, output: &Path) -> Result<(), XtaskError>;
    fn strip_wasm(&mut self, input: &Path, output: &Path) -> Result<(), XtaskError>;
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
}

struct SystemArtifactRuntime;

impl TypstArtifactSpec {
    pub(crate) fn for_repository_profile(
        catalog: &TypstProfileCatalog,
        profile: &TypstPackageProfile,
        artifact_profile: &WasmArtifactProfile,
    ) -> Result<Self, XtaskError> {
        if catalog.package_profile() != profile {
            return Err(artifact_error(format!(
                "profile `{}` is not the canonical descriptor entry",
                profile.name()
            )));
        }
        validate_typst_artifact_profiles(catalog, std::slice::from_ref(artifact_profile))?;

        let workspace_root = paths::workspace_root().canonicalize().map_err(|source| {
            artifact_io_error(
                "canonicalize workspace root",
                &paths::workspace_root(),
                source,
            )
        })?;
        let cargo_package_version = read_workspace_package_version(&workspace_root)?;
        let typst_package_version = read_typst_package_version(&workspace_root)?;
        Self {
            target_root: paths::target_root(),
            workspace_root,
            artifact_profile: artifact_profile.id.clone(),
            profile: profile.name().to_string(),
            default_features: artifact_profile.default_features,
            features: artifact_profile.features.clone(),
            cargo_package: artifact_profile.package.clone(),
            cargo_manifest_path: artifact_profile.manifest_path.clone(),
            artifact_name: artifact_profile.artifact_name.clone(),
            cargo_package_version,
            typst_package_version,
            plugin_abi_version: catalog.plugin_abi_version(),
            mermaid_version: merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION.to_string(),
            mermaid_source_commit: MERMAID_SOURCE_COMMIT.to_string(),
        }
        .normalized_and_validated()
    }

    fn normalized_and_validated(mut self) -> Result<Self, XtaskError> {
        self.features.sort();
        if self
            .features
            .windows(2)
            .any(|features| features[0] == features[1])
        {
            return Err(artifact_error(format!(
                "profile `{}` contains duplicate Cargo features",
                self.profile
            )));
        }

        self.validate()?;
        Ok(self)
    }

    fn validate(&self) -> Result<(), XtaskError> {
        validate_kebab_name("artifact profile", &self.artifact_profile)?;
        validate_kebab_name("canonical package profile", &self.profile)?;
        validate_artifact_name(&self.artifact_name)?;
        validate_cargo_manifest_path(&self.cargo_manifest_path)?;
        let crate_plugin_abi_version = merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION;
        if self.plugin_abi_version != crate_plugin_abi_version {
            return Err(artifact_error(format!(
                "Typst plugin ABI must match crate ABI {crate_plugin_abi_version}, found {}",
                self.plugin_abi_version
            )));
        }
        for (field, value) in [
            ("Cargo package", self.cargo_package.as_str()),
            ("Cargo package version", self.cargo_package_version.as_str()),
            ("Typst package version", self.typst_package_version.as_str()),
            ("Mermaid version", self.mermaid_version.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(artifact_error(format!("{field} must not be empty")));
            }
        }
        if self.mermaid_source_commit.len() != 40
            || !self
                .mermaid_source_commit
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(artifact_error(
                "Mermaid source commit must be a 40-character hexadecimal Git object id",
            ));
        }
        if !self.workspace_root.is_dir() {
            return Err(artifact_error(format!(
                "workspace root is not a directory: {}",
                self.workspace_root.display()
            )));
        }
        Ok(())
    }

    fn artifact_root(&self) -> PathBuf {
        self.target_root.join(ARTIFACT_ROOT_NAME)
    }

    fn output_directory(&self) -> PathBuf {
        self.artifact_root().join(&self.profile)
    }

    fn output_wasm(&self) -> PathBuf {
        self.output_directory().join(&self.artifact_name)
    }

    fn manifest(
        &self,
        input: InputFingerprint,
        tools: ToolFingerprint,
        artifact: ArtifactFingerprint,
    ) -> ArtifactManifest {
        ArtifactManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            artifact_profile: self.artifact_profile.clone(),
            profile: self.profile.clone(),
            default_features: self.default_features,
            features: self.features.clone(),
            target_triple: TARGET_TRIPLE.to_string(),
            cargo_profile: CARGO_PROFILE.to_string(),
            cargo_package: self.cargo_package.clone(),
            cargo_package_version: self.cargo_package_version.clone(),
            typst_package_version: self.typst_package_version.clone(),
            plugin_abi_version: self.plugin_abi_version,
            mermaid_version: self.mermaid_version.clone(),
            mermaid_source_commit: self.mermaid_source_commit.clone(),
            input,
            tools,
            artifact,
        }
    }
}

impl VerifiedTypstArtifact {
    pub(crate) fn wasm_path(&self) -> &Path {
        &self.wasm_path
    }

    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }
}

impl ToolIdentity {
    fn fingerprint(self) -> ToolFingerprint {
        let mut hasher = Sha256::new();
        hash_framed(&mut hasher, b"merman-typst-tool-fingerprint-v2");
        hash_framed(&mut hasher, self.cargo_version.as_bytes());
        hash_framed(&mut hasher, self.rustc_version.as_bytes());
        hash_framed(&mut hasher, self.wasm_opt_version.as_bytes());
        hash_framed(&mut hasher, self.wasm_tools_version.as_bytes());
        hash_optional(&mut hasher, self.rustflags.as_deref());
        hash_optional(&mut hasher, self.cargo_encoded_rustflags.as_deref());
        ToolFingerprint {
            sha256: format!("{:x}", hasher.finalize()),
            cargo_version: self.cargo_version,
            rustc_version: self.rustc_version,
            wasm_opt_version: self.wasm_opt_version,
            wasm_tools_version: self.wasm_tools_version,
            rustflags: self.rustflags,
            cargo_encoded_rustflags: self.cargo_encoded_rustflags,
        }
    }
}

impl ArtifactRuntime for SystemArtifactRuntime {
    fn cargo_metadata(
        &mut self,
        spec: &TypstArtifactSpec,
    ) -> Result<CargoMetadataOutput, XtaskError> {
        let mut command = Command::new("cargo");
        command.args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            TARGET_TRIPLE,
        ]);
        if !spec.default_features {
            command.arg("--no-default-features");
        }
        command
            .arg("--manifest-path")
            .arg(spec.workspace_root.join(&spec.cargo_manifest_path));
        if !spec.features.is_empty() {
            let features = spec.features.join(",");
            command.args(["--features", &features]);
        }
        let output = command
            .current_dir(&spec.workspace_root)
            .output()
            .map_err(|source| {
                artifact_io_error(
                    "resolve Cargo metadata",
                    &spec.workspace_root.join(&spec.cargo_manifest_path),
                    source,
                )
            })?;
        if !output.status.success() {
            return Err(artifact_error(format!(
                "cargo metadata failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        serde_json::from_slice(&output.stdout).map_err(|error| {
            artifact_error(format!("cargo metadata returned invalid JSON: {error}"))
        })
    }

    fn tool_identity(&mut self) -> Result<ToolIdentity, XtaskError> {
        let wasm_opt_version = verify_typst_wasm_optimizer()?;
        Ok(ToolIdentity {
            cargo_version: command_version("cargo", &["--version", "--verbose"])?,
            rustc_version: command_version("rustc", &["-vV"])?,
            wasm_opt_version,
            wasm_tools_version: command_version("wasm-tools", &["--version"])?,
            rustflags: unicode_environment_variable("RUSTFLAGS")?,
            cargo_encoded_rustflags: unicode_environment_variable("CARGO_ENCODED_RUSTFLAGS")?,
        })
    }

    fn optimize_wasm(&mut self, input: &Path, output: &Path) -> Result<(), XtaskError> {
        optimize_typst_wasm(input, output)
    }

    fn strip_wasm(&mut self, input: &Path, output: &Path) -> Result<(), XtaskError> {
        let command_output = Command::new("wasm-tools")
            .args(["strip", "--all"])
            .arg(input)
            .arg("-o")
            .arg(output)
            .output()
            .map_err(|source| artifact_io_error("run wasm-tools strip", input, source))?;
        if !command_output.status.success() {
            return Err(artifact_error(format!(
                "wasm-tools strip failed with status {}: {}",
                command_output.status,
                String::from_utf8_lossy(&command_output.stderr).trim()
            )));
        }
        Ok(())
    }

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }
}

pub(crate) fn verify_typst_wasm_optimizer() -> Result<String, XtaskError> {
    let version = command_version("wasm-opt", &["--version"])?;
    validate_typst_wasm_optimizer_version(&version)?;
    Ok(version)
}

fn validate_typst_wasm_optimizer_version(version: &str) -> Result<(), XtaskError> {
    let compatible = match version.strip_prefix(WASM_OPT_VERSION) {
        Some("") => true,
        Some(suffix) => suffix
            .strip_prefix(" (")
            .and_then(|metadata| metadata.strip_suffix(')'))
            .is_some_and(|metadata| {
                !metadata.is_empty() && !metadata.chars().any(char::is_whitespace)
            }),
        None => false,
    };
    if !compatible {
        return Err(artifact_error(format!(
            "Typst packaging requires Binaryen 131 (`{WASM_OPT_VERSION}`), found `{version}`"
        )));
    }
    Ok(())
}

pub(crate) fn optimize_typst_wasm(input: &Path, output: &Path) -> Result<(), XtaskError> {
    let command_output = Command::new("wasm-opt")
        .arg("-Oz")
        .args(WASM_OPT_FEATURES)
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|source| artifact_io_error("run wasm-opt", input, source))?;
    if !command_output.status.success() {
        return Err(artifact_error(format!(
            "wasm-opt failed with status {}: {}",
            command_output.status,
            String::from_utf8_lossy(&command_output.stderr).trim()
        )));
    }
    Ok(())
}

pub(crate) fn install_typst_artifact(
    spec: &TypstArtifactSpec,
    raw_wasm: &Path,
) -> Result<VerifiedTypstArtifact, XtaskError> {
    let mut runtime = SystemArtifactRuntime;
    install_with_runtime(spec, raw_wasm, &mut runtime)
}

pub(crate) fn verify_typst_artifact(
    spec: &TypstArtifactSpec,
) -> Result<VerifiedTypstArtifact, XtaskError> {
    let mut runtime = SystemArtifactRuntime;
    verify_with_runtime(spec, &mut runtime)
}

fn install_with_runtime<R: ArtifactRuntime>(
    spec: &TypstArtifactSpec,
    raw_wasm: &Path,
    runtime: &mut R,
) -> Result<VerifiedTypstArtifact, XtaskError> {
    spec.validate()?;
    validate_raw_wasm(raw_wasm)?;
    let artifact_root = prepare_artifact_root(spec)?;
    let lock = acquire_profile_lock(spec, &artifact_root)?;
    let metadata = runtime.cargo_metadata(spec)?;
    let input = collect_input_fingerprint(spec, &metadata)?;
    let tools = runtime.tool_identity()?.fingerprint();

    let staging = tempfile::Builder::new()
        .prefix(&format!(".{}.staging-", spec.profile))
        .tempdir_in(&artifact_root)
        .map_err(|source| {
            artifact_io_error("create artifact staging directory", &artifact_root, source)
        })?;
    let private_raw = staging.path().join(".raw.wasm");
    fs::copy(raw_wasm, &private_raw)
        .map_err(|source| artifact_io_error("copy raw WASM into staging", raw_wasm, source))?;
    let optimized_wasm = staging.path().join(".optimized.wasm");
    runtime.optimize_wasm(&private_raw, &optimized_wasm)?;
    validate_regular_file(&optimized_wasm, "optimized WASM artifact")?;
    let staged_wasm = staging.path().join(&spec.artifact_name);
    runtime.strip_wasm(&optimized_wasm, &staged_wasm)?;
    validate_regular_file(&staged_wasm, "stripped WASM artifact")?;
    fs::remove_file(&private_raw).map_err(|source| {
        artifact_io_error("remove private raw WASM copy", &private_raw, source)
    })?;
    fs::remove_file(&optimized_wasm).map_err(|source| {
        artifact_io_error(
            "remove private optimized WASM copy",
            &optimized_wasm,
            source,
        )
    })?;

    let current_metadata = runtime.cargo_metadata(spec)?;
    let current_input = collect_input_fingerprint(spec, &current_metadata)?;
    if current_input != input {
        return Err(artifact_error(format!(
            "artifact input tree changed while profile `{}` was being staged",
            spec.profile
        )));
    }
    let artifact = fingerprint_artifact(&staged_wasm, &spec.artifact_name)?;
    let manifest = spec.manifest(input.clone(), tools.clone(), artifact);
    write_manifest(&staging.path().join(MANIFEST_FILE_NAME), &manifest)?;
    validate_directory_manifest(spec, staging.path(), &input, &tools, &manifest)?;

    replace_output_directory(spec, staging, runtime)?;
    Ok(VerifiedTypstArtifact {
        wasm_path: spec.output_wasm(),
        manifest_path: spec.output_directory().join(MANIFEST_FILE_NAME),
        _lock: lock,
    })
}

fn verify_with_runtime<R: ArtifactRuntime>(
    spec: &TypstArtifactSpec,
    runtime: &mut R,
) -> Result<VerifiedTypstArtifact, XtaskError> {
    spec.validate()?;
    let artifact_root = prepare_artifact_root(spec)?;
    let lock = acquire_profile_lock(spec, &artifact_root)?;
    let metadata = runtime.cargo_metadata(spec)?;
    let input = collect_input_fingerprint(spec, &metadata)?;
    let tools = runtime.tool_identity()?.fingerprint();
    let wasm_path = verify_directory(spec, &spec.output_directory(), &input, &tools)?;
    Ok(VerifiedTypstArtifact {
        wasm_path,
        manifest_path: spec.output_directory().join(MANIFEST_FILE_NAME),
        _lock: lock,
    })
}

fn verify_directory(
    spec: &TypstArtifactSpec,
    directory: &Path,
    input: &InputFingerprint,
    tools: &ToolFingerprint,
) -> Result<PathBuf, XtaskError> {
    let manifest_path = directory.join(MANIFEST_FILE_NAME);
    validate_regular_file(&manifest_path, "artifact manifest")?;
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|source| artifact_io_error("read artifact manifest", &manifest_path, source))?;
    let manifest: ArtifactManifest = serde_json::from_slice(&manifest_bytes).map_err(|error| {
        artifact_error(format!(
            "artifact manifest {} is not valid schema-{MANIFEST_SCHEMA_VERSION} JSON: {error}",
            manifest_path.display()
        ))
    })?;
    let wasm_path = validate_directory_manifest(spec, directory, input, tools, &manifest)?;
    let artifact = fingerprint_artifact(&wasm_path, &spec.artifact_name)?;
    if manifest.artifact != artifact {
        return Err(artifact_error(format!(
            "artifact digest or byte length is stale for {}",
            wasm_path.display()
        )));
    }
    Ok(wasm_path)
}

fn validate_directory_manifest(
    spec: &TypstArtifactSpec,
    directory: &Path,
    input: &InputFingerprint,
    tools: &ToolFingerprint,
    manifest: &ArtifactManifest,
) -> Result<PathBuf, XtaskError> {
    validate_output_directory(directory)?;
    let actual_entries = directory_entries(directory)?;
    let expected_entries =
        BTreeSet::from([MANIFEST_FILE_NAME.to_string(), spec.artifact_name.clone()]);
    if actual_entries != expected_entries {
        return Err(artifact_error(format!(
            "artifact directory {} contains unexpected or missing entries; expected exactly [{}], found [{}]",
            directory.display(),
            expected_entries
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            actual_entries
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    validate_regular_file(&directory.join(MANIFEST_FILE_NAME), "artifact manifest")?;

    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(manifest_mismatch(
            "schema_version",
            MANIFEST_SCHEMA_VERSION,
            manifest.schema_version,
        ));
    }
    if manifest.artifact_profile != spec.artifact_profile {
        return Err(manifest_mismatch(
            "artifact_profile",
            &spec.artifact_profile,
            &manifest.artifact_profile,
        ));
    }
    if manifest.profile != spec.profile {
        return Err(manifest_mismatch(
            "profile",
            &spec.profile,
            &manifest.profile,
        ));
    }
    if manifest.default_features != spec.default_features {
        return Err(artifact_error(format!(
            "artifact manifest default_features is stale for profile `{}`",
            spec.profile
        )));
    }
    if manifest.features != spec.features {
        return Err(artifact_error(format!(
            "artifact manifest features are stale for profile `{}`",
            spec.profile
        )));
    }
    if manifest.target_triple != TARGET_TRIPLE {
        return Err(manifest_mismatch(
            "target_triple",
            TARGET_TRIPLE,
            &manifest.target_triple,
        ));
    }
    if manifest.cargo_profile != CARGO_PROFILE {
        return Err(manifest_mismatch(
            "cargo_profile",
            CARGO_PROFILE,
            &manifest.cargo_profile,
        ));
    }
    if manifest.cargo_package != spec.cargo_package {
        return Err(manifest_mismatch(
            "cargo_package",
            &spec.cargo_package,
            &manifest.cargo_package,
        ));
    }
    if manifest.cargo_package_version != spec.cargo_package_version {
        return Err(manifest_mismatch(
            "cargo_package_version",
            &spec.cargo_package_version,
            &manifest.cargo_package_version,
        ));
    }
    if manifest.typst_package_version != spec.typst_package_version {
        return Err(manifest_mismatch(
            "typst_package_version",
            &spec.typst_package_version,
            &manifest.typst_package_version,
        ));
    }
    if manifest.plugin_abi_version != spec.plugin_abi_version {
        return Err(manifest_mismatch(
            "plugin_abi_version",
            spec.plugin_abi_version,
            manifest.plugin_abi_version,
        ));
    }
    if manifest.mermaid_version != spec.mermaid_version {
        return Err(manifest_mismatch(
            "mermaid_version",
            &spec.mermaid_version,
            &manifest.mermaid_version,
        ));
    }
    if manifest.mermaid_source_commit != spec.mermaid_source_commit {
        return Err(manifest_mismatch(
            "mermaid_source_commit",
            &spec.mermaid_source_commit,
            &manifest.mermaid_source_commit,
        ));
    }
    if manifest.input != *input {
        return Err(artifact_error(format!(
            "artifact input tree is stale for profile `{}`",
            spec.profile
        )));
    }
    if manifest.tools != *tools {
        return Err(artifact_error(format!(
            "artifact toolchain or effective Rust flags are stale for profile `{}`",
            spec.profile
        )));
    }

    let wasm_path = directory.join(&spec.artifact_name);
    validate_regular_file(&wasm_path, "WASM artifact")?;
    Ok(wasm_path)
}

fn replace_output_directory<R: ArtifactRuntime>(
    spec: &TypstArtifactSpec,
    staging: tempfile::TempDir,
    runtime: &mut R,
) -> Result<(), XtaskError> {
    let destination = spec.output_directory();
    let artifact_root = spec.artifact_root();
    let had_original = match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => true,
        Ok(_) => {
            return Err(artifact_error(format!(
                "artifact destination is not a regular directory: {}",
                destination.display()
            )));
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(artifact_io_error(
                "inspect artifact destination",
                &destination,
                source,
            ));
        }
    };

    let backup = tempfile::Builder::new()
        .prefix(&format!(".{}.backup-", spec.profile))
        .tempdir_in(&artifact_root)
        .map_err(|source| {
            artifact_io_error("create artifact backup directory", &artifact_root, source)
        })?;
    let backup_path = backup.path().join("previous");

    if had_original {
        runtime
            .rename(&destination, &backup_path)
            .map_err(|source| {
                artifact_io_error(
                    "move existing artifact directory into rollback storage",
                    &destination,
                    source,
                )
            })?;
    }

    if let Err(install_error) = runtime.rename(staging.path(), &destination) {
        if had_original && let Err(rollback_error) = runtime.rename(&backup_path, &destination) {
            let preserved_backup = backup.keep();
            return Err(artifact_error(format!(
                "failed to install staged artifact directory at {}: {install_error}; failed to restore the previous artifact: {rollback_error}; previous artifact preserved at {}",
                destination.display(),
                preserved_backup.join("previous").display()
            )));
        }
        backup.close().map_err(|source| {
            artifact_io_error(
                "clean empty artifact rollback directory",
                &artifact_root,
                source,
            )
        })?;
        let recovery = if had_original {
            "previous artifact restored"
        } else {
            "no previous artifact existed"
        };
        return Err(artifact_error(format!(
            "failed to install staged artifact directory at {}: {install_error}; {recovery}",
            destination.display()
        )));
    }

    backup.close().map_err(|source| {
        artifact_io_error(
            "remove committed artifact rollback directory",
            &artifact_root,
            source,
        )
    })?;
    Ok(())
}

fn prepare_artifact_root(spec: &TypstArtifactSpec) -> Result<PathBuf, XtaskError> {
    let artifact_root = spec.artifact_root();
    fs::create_dir_all(&artifact_root).map_err(|source| {
        artifact_io_error("create Typst artifact root", &artifact_root, source)
    })?;
    let metadata = fs::symlink_metadata(&artifact_root).map_err(|source| {
        artifact_io_error("inspect Typst artifact root", &artifact_root, source)
    })?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(artifact_error(format!(
            "Typst artifact root must be a regular directory: {}",
            artifact_root.display()
        )));
    }
    Ok(artifact_root)
}

fn acquire_profile_lock(
    spec: &TypstArtifactSpec,
    artifact_root: &Path,
) -> Result<ProfileArtifactLock, XtaskError> {
    let lock_root = artifact_root.join(".locks");
    fs::create_dir_all(&lock_root).map_err(|source| {
        artifact_io_error("create artifact lock directory", &lock_root, source)
    })?;
    let lock_path = lock_root.join(format!("{}.lock", spec.profile));
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| artifact_io_error("open profile artifact lock", &lock_path, source))?;
    fs2::FileExt::lock_exclusive(&file)
        .map_err(|source| artifact_io_error("lock profile artifact", &lock_path, source))?;
    Ok(ProfileArtifactLock { _file: file })
}

fn collect_input_fingerprint(
    spec: &TypstArtifactSpec,
    metadata: &CargoMetadataOutput,
) -> Result<InputFingerprint, XtaskError> {
    let workspace_root = &spec.workspace_root;
    let (root_id, closure) = resolve_local_production_closure(spec, metadata)?;
    let mut paths = Vec::new();
    collect_required_file(workspace_root, Path::new("Cargo.toml"), &mut paths)?;
    collect_required_file(workspace_root, Path::new("Cargo.lock"), &mut paths)?;
    collect_optional_file(workspace_root, Path::new("rust-toolchain.toml"), &mut paths)?;
    collect_optional_file(workspace_root, Path::new("rust-toolchain"), &mut paths)?;
    collect_optional_directory(workspace_root, Path::new(".cargo"), false, &mut paths)?;
    collect_required_file(
        workspace_root,
        Path::new("distribution/typst/merman/typst.toml"),
        &mut paths,
    )?;
    let mut packages = Vec::with_capacity(closure.len());
    for package in closure {
        let manifest_path = manifest_relative_path(workspace_root, &package.manifest_path)?;
        packages.push(InputPackageFingerprint {
            name: package.name.clone(),
            manifest_path,
        });
        collect_package_inputs(workspace_root, package, package.id == root_id, &mut paths)?;
    }
    packages.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.manifest_path.cmp(&right.manifest_path))
    });
    paths.sort();
    paths.dedup();

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::read(&path)
            .map_err(|source| artifact_io_error("read artifact input", &path, source))?;
        files.push(InputFileFingerprint {
            path: manifest_relative_path(workspace_root, &path)?,
            sha256: sha256_hex(&bytes),
            bytes: u64::try_from(bytes.len())
                .map_err(|_| artifact_error(format!("input is too large: {}", path.display())))?,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let mut hasher = Sha256::new();
    hash_framed(&mut hasher, b"merman-typst-local-production-closure-v1");
    for package in &packages {
        hash_framed(&mut hasher, package.name.as_bytes());
        hash_framed(&mut hasher, package.manifest_path.as_bytes());
    }
    for file in &files {
        hash_framed(&mut hasher, file.path.as_bytes());
        hash_framed(&mut hasher, &file.bytes.to_le_bytes());
        hash_framed(&mut hasher, file.sha256.as_bytes());
    }
    Ok(InputFingerprint {
        sha256: format!("{:x}", hasher.finalize()),
        packages,
        files,
    })
}

fn resolve_local_production_closure<'a>(
    spec: &TypstArtifactSpec,
    metadata: &'a CargoMetadataOutput,
) -> Result<(String, Vec<&'a MetadataPackage>), XtaskError> {
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| artifact_error("cargo metadata did not contain a resolved graph"))?;
    let root_id = resolve.root.as_ref().ok_or_else(|| {
        artifact_error("cargo metadata did not identify the selected root package")
    })?;

    let mut packages_by_id = BTreeMap::new();
    for package in &metadata.packages {
        if packages_by_id
            .insert(package.id.as_str(), package)
            .is_some()
        {
            return Err(artifact_error(format!(
                "cargo metadata contains duplicate package id `{}`",
                package.id
            )));
        }
    }
    let mut nodes_by_id = BTreeMap::new();
    for node in &resolve.nodes {
        if nodes_by_id.insert(node.id.as_str(), node).is_some() {
            return Err(artifact_error(format!(
                "cargo metadata contains duplicate resolve node `{}`",
                node.id
            )));
        }
    }

    let root_package = packages_by_id.get(root_id.as_str()).ok_or_else(|| {
        artifact_error(format!(
            "cargo metadata root `{root_id}` has no package record"
        ))
    })?;
    let expected_manifest = spec.workspace_root.join(&spec.cargo_manifest_path);
    let manifest_matches =
        if root_package.name == spec.cargo_package && root_package.source.is_none() {
            canonicalize_path_for_comparison(
                &expected_manifest,
                "canonicalize expected Cargo manifest",
            )? == canonicalize_path_for_comparison(
                &root_package.manifest_path,
                "canonicalize Cargo metadata manifest",
            )?
        } else {
            false
        };
    if root_package.name != spec.cargo_package || root_package.source.is_some() || !manifest_matches
    {
        return Err(artifact_error(format!(
            "cargo metadata root must be local package `{}` at {}, found `{}` at {}",
            spec.cargo_package,
            expected_manifest.display(),
            root_package.name,
            root_package.manifest_path.display()
        )));
    }
    let root_node = nodes_by_id.get(root_id.as_str()).ok_or_else(|| {
        artifact_error(format!(
            "cargo metadata root `{root_id}` has no resolve node"
        ))
    })?;
    validate_resolved_root_features(spec, root_node)?;

    let mut pending = vec![root_id.clone()];
    let mut visited = BTreeSet::new();
    let mut local_packages = Vec::new();
    while let Some(package_id) = pending.pop() {
        if !visited.insert(package_id.clone()) {
            continue;
        }
        let package = packages_by_id.get(package_id.as_str()).ok_or_else(|| {
            artifact_error(format!(
                "cargo metadata resolve node references missing package `{package_id}`"
            ))
        })?;
        if package.source.is_none() {
            ensure_local_manifest_is_in_workspace(&spec.workspace_root, package)?;
            local_packages.push(*package);
        }

        let node = nodes_by_id.get(package_id.as_str()).ok_or_else(|| {
            artifact_error(format!(
                "cargo metadata package `{package_id}` has no resolve node"
            ))
        })?;
        for dependency in &node.deps {
            if is_production_dependency(&package_id, dependency)? {
                if !packages_by_id.contains_key(dependency.pkg.as_str()) {
                    return Err(artifact_error(format!(
                        "cargo metadata dependency `{}` of `{package_id}` has no package record",
                        dependency.pkg
                    )));
                }
                pending.push(dependency.pkg.clone());
            }
        }
    }

    local_packages.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.manifest_path.cmp(&right.manifest_path))
    });
    Ok((root_id.clone(), local_packages))
}

fn validate_resolved_root_features(
    spec: &TypstArtifactSpec,
    root: &MetadataNode,
) -> Result<(), XtaskError> {
    let resolved = root
        .features
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let requested = spec
        .features
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if resolved != requested {
        return Err(artifact_error(format!(
            "cargo metadata root features do not exactly match profile `{}`: expected [{}], found [{}]",
            spec.profile,
            requested.into_iter().collect::<Vec<_>>().join(", "),
            resolved.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(())
}

fn ensure_local_manifest_is_in_workspace(
    workspace_root: &Path,
    package: &MetadataPackage,
) -> Result<(), XtaskError> {
    let workspace_root =
        canonicalize_path_for_comparison(workspace_root, "canonicalize workspace root")?;
    let manifest_path = canonicalize_path_for_comparison(
        &package.manifest_path,
        "canonicalize reachable Cargo manifest",
    )?;
    manifest_path.strip_prefix(&workspace_root).map_err(|_| {
        artifact_error(format!(
            "reachable local package `{}` is outside workspace root: {}",
            package.name,
            package.manifest_path.display()
        ))
    })?;
    Ok(())
}

fn is_production_dependency(
    package_id: &str,
    dependency: &MetadataNodeDependency,
) -> Result<bool, XtaskError> {
    if dependency.dep_kinds.is_empty() {
        return Err(artifact_error(format!(
            "cargo metadata dependency `{}` of `{package_id}` has no dependency kind",
            dependency.pkg
        )));
    }
    let mut production = false;
    for dependency_kind in &dependency.dep_kinds {
        match dependency_kind.kind.as_deref() {
            None | Some("normal" | "build") => production = true,
            Some("dev") => {}
            Some(kind) => {
                return Err(artifact_error(format!(
                    "cargo metadata dependency `{}` of `{package_id}` has unknown kind `{kind}`",
                    dependency.pkg
                )));
            }
        }
    }
    Ok(production)
}

fn collect_package_inputs(
    workspace_root: &Path,
    package: &MetadataPackage,
    is_root: bool,
    output: &mut Vec<PathBuf>,
) -> Result<(), XtaskError> {
    let first_new_path = output.len();
    validate_regular_file(&package.manifest_path, "reachable Cargo manifest")?;
    let package_root = package.manifest_path.parent().ok_or_else(|| {
        artifact_error(format!(
            "reachable Cargo manifest has no parent directory: {}",
            package.manifest_path.display()
        ))
    })?;
    validate_regular_directory(package_root, "reachable Cargo package directory")?;
    collect_directory_files(package_root, true, output)?;
    for target in &package.targets {
        collect_production_target_inputs(package_root, target, output)?;
    }
    if is_root {
        let descriptor = package_root.join("wasm-profiles.json");
        validate_regular_file(&descriptor, "Typst package descriptor")?;
        output.push(descriptor);
    }
    for path in &output[first_new_path..] {
        manifest_relative_path(workspace_root, path)?;
    }
    Ok(())
}

fn collect_production_target_inputs(
    package_root: &Path,
    target: &MetadataTarget,
    output: &mut Vec<PathBuf>,
) -> Result<(), XtaskError> {
    let mut production = false;
    for kind in &target.kind {
        match kind.as_str() {
            "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro" | "bin" => {
                production = true;
            }
            "custom-build" => {
                production = true;
            }
            "test" | "bench" | "example" => {}
            unknown => {
                return Err(artifact_error(format!(
                    "cargo metadata target {} has unknown kind `{unknown}`",
                    target.src_path.display()
                )));
            }
        }
    }
    if !production {
        return Ok(());
    }
    validate_regular_file(&target.src_path, "production Cargo target source")?;
    output.push(target.src_path.clone());
    if target.src_path.starts_with(package_root) {
        return Ok(());
    }
    let source_root = target.src_path.parent().ok_or_else(|| {
        artifact_error(format!(
            "production Cargo target source has no parent: {}",
            target.src_path.display()
        ))
    })?;
    collect_directory_files(source_root, true, output)
}

fn collect_required_file(
    workspace_root: &Path,
    relative: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), XtaskError> {
    let path = workspace_root.join(relative);
    validate_regular_file(&path, "required artifact input")?;
    output.push(path);
    Ok(())
}

fn collect_optional_file(
    workspace_root: &Path,
    relative: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), XtaskError> {
    let path = workspace_root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            output.push(path);
            Ok(())
        }
        Ok(_) => Err(artifact_error(format!(
            "optional artifact input is not a regular file: {}",
            path.display()
        ))),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(artifact_io_error(
            "inspect optional artifact input",
            &path,
            source,
        )),
    }
}

fn collect_optional_directory(
    workspace_root: &Path,
    relative: &Path,
    exclude_non_production: bool,
    output: &mut Vec<PathBuf>,
) -> Result<(), XtaskError> {
    collect_optional_absolute_directory(
        &workspace_root.join(relative),
        exclude_non_production,
        output,
    )
}

fn collect_optional_absolute_directory(
    directory: &Path,
    exclude_non_production: bool,
    output: &mut Vec<PathBuf>,
) -> Result<(), XtaskError> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            collect_directory_files(directory, exclude_non_production, output)
        }
        Ok(_) => Err(artifact_error(format!(
            "optional artifact input is not a regular directory: {}",
            directory.display()
        ))),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(artifact_io_error(
            "inspect optional artifact input directory",
            directory,
            source,
        )),
    }
}

fn collect_directory_files(
    directory: &Path,
    exclude_non_production: bool,
    output: &mut Vec<PathBuf>,
) -> Result<(), XtaskError> {
    let entries = fs::read_dir(directory)
        .map_err(|source| artifact_io_error("read artifact input directory", directory, source))?;
    let mut entries = entries.collect::<Result<Vec<_>, _>>().map_err(|source| {
        artifact_io_error("read artifact input directory entry", directory, source)
    })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().into_string().map_err(|_| {
            artifact_error(format!(
                "artifact input path is not valid UTF-8: {}",
                path.display()
            ))
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| artifact_io_error("inspect artifact input", &path, source))?;
        if file_type.is_symlink() {
            return Err(artifact_error(format!(
                "artifact input tree must not contain symbolic links: {}",
                path.display()
            )));
        }
        if file_type.is_dir() {
            if exclude_non_production && is_non_production_directory(&name) {
                continue;
            }
            collect_directory_files(&path, exclude_non_production, output)?;
        } else if file_type.is_file() {
            output.push(path);
        } else {
            return Err(artifact_error(format!(
                "artifact input tree contains a non-file entry: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn is_non_production_directory(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | "tests" | "benches" | "examples" | "node_modules" | "__pycache__"
    )
}

fn manifest_relative_path(workspace_root: &Path, path: &Path) -> Result<String, XtaskError> {
    let workspace_root =
        canonicalize_path_for_comparison(workspace_root, "canonicalize workspace root")?;
    let path = canonicalize_path_for_comparison(path, "canonicalize artifact input")?;
    let relative = path.strip_prefix(&workspace_root).map_err(|_| {
        artifact_error(format!(
            "artifact input {} is outside workspace root {}",
            path.display(),
            workspace_root.display()
        ))
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(artifact_error(format!(
                "artifact input has a non-canonical relative path: {}",
                path.display()
            )));
        };
        parts.push(part.to_str().ok_or_else(|| {
            artifact_error(format!(
                "artifact input path is not valid UTF-8: {}",
                path.display()
            ))
        })?);
    }
    if parts.is_empty() {
        return Err(artifact_error("artifact input path must name a file"));
    }
    Ok(parts.join("/"))
}

fn canonicalize_path_for_comparison(path: &Path, action: &str) -> Result<PathBuf, XtaskError> {
    fs::canonicalize(path).map_err(|source| artifact_io_error(action, path, source))
}

fn fingerprint_artifact(path: &Path, file_name: &str) -> Result<ArtifactFingerprint, XtaskError> {
    let bytes =
        fs::read(path).map_err(|source| artifact_io_error("read WASM artifact", path, source))?;
    Ok(ArtifactFingerprint {
        file: file_name.to_string(),
        sha256: sha256_hex(&bytes),
        bytes: u64::try_from(bytes.len()).map_err(|_| {
            artifact_error(format!("WASM artifact is too large: {}", path.display()))
        })?,
    })
}

fn write_manifest(path: &Path, manifest: &ArtifactManifest) -> Result<(), XtaskError> {
    let mut bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| artifact_error(format!("serialize artifact manifest: {error}")))?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| artifact_io_error("create artifact manifest", path, source))?;
    file.write_all(&bytes)
        .map_err(|source| artifact_io_error("write artifact manifest", path, source))?;
    Ok(())
}

fn read_workspace_package_version(workspace_root: &Path) -> Result<String, XtaskError> {
    let path = workspace_root.join("Cargo.toml");
    let source = fs::read_to_string(&path)
        .map_err(|error| artifact_io_error("read workspace Cargo manifest", &path, error))?;
    let manifest: WorkspaceManifest = toml::from_str(&source).map_err(|error| {
        artifact_error(format!(
            "failed to parse workspace Cargo manifest {}: {error}",
            path.display()
        ))
    })?;
    Ok(manifest.workspace.package.version)
}

fn read_typst_package_version(workspace_root: &Path) -> Result<String, XtaskError> {
    let path = workspace_root.join("distribution/typst/merman/typst.toml");
    let source = fs::read_to_string(&path)
        .map_err(|error| artifact_io_error("read Typst package manifest", &path, error))?;
    let manifest: TypstPackageManifest = toml::from_str(&source).map_err(|error| {
        artifact_error(format!(
            "failed to parse Typst package manifest {}: {error}",
            path.display()
        ))
    })?;
    Ok(manifest.package.version)
}

fn command_version(program: &str, arguments: &[&str]) -> Result<String, XtaskError> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|source| artifact_io_error("query tool version", Path::new(program), source))?;
    if !output.status.success() {
        return Err(artifact_error(format!(
            "failed to query `{program}` version with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        artifact_error(format!(
            "`{program}` version output is not valid UTF-8: {error}"
        ))
    })?;
    let stdout = stdout.trim();
    if stdout.is_empty() {
        return Err(artifact_error(format!(
            "`{program}` returned an empty version"
        )));
    }
    Ok(stdout.to_string())
}

fn unicode_environment_variable(name: &str) -> Result<Option<String>, XtaskError> {
    std::env::var_os(name)
        .map(|value| {
            value.into_string().map_err(|_| {
                artifact_error(format!(
                    "environment variable `{name}` is not valid Unicode"
                ))
            })
        })
        .transpose()
}

fn validate_kebab_name(kind: &str, value: &str) -> Result<(), XtaskError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(artifact_error(format!(
            "{kind} `{value}` must use lowercase ASCII letters, digits, and hyphens"
        )));
    }
    Ok(())
}

fn validate_artifact_name(name: &str) -> Result<(), XtaskError> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || Path::new(name).extension().and_then(|value| value.to_str()) != Some("wasm")
        || name == MANIFEST_FILE_NAME
    {
        return Err(artifact_error(format!(
            "artifact name `{name}` must be a single .wasm file name"
        )));
    }
    Ok(())
}

fn validate_cargo_manifest_path(path: &Path) -> Result<(), XtaskError> {
    if path.as_os_str().is_empty()
        || path.file_name().and_then(|value| value.to_str()) != Some("Cargo.toml")
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(artifact_error(format!(
            "Cargo manifest path must be a repository-relative Cargo.toml path: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_raw_wasm(path: &Path) -> Result<(), XtaskError> {
    validate_regular_file(path, "raw WASM artifact")
}

fn validate_output_directory(path: &Path) -> Result<(), XtaskError> {
    validate_regular_directory(path, "profile artifact directory")
}

fn validate_regular_file(path: &Path, kind: &str) -> Result<(), XtaskError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| artifact_io_error(&format!("inspect {kind}"), path, source))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(artifact_error(format!(
            "{kind} must be a regular file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_regular_directory(path: &Path, kind: &str) -> Result<(), XtaskError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| artifact_io_error(&format!("inspect {kind}"), path, source))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(artifact_error(format!(
            "{kind} must be a regular directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn directory_entries(directory: &Path) -> Result<BTreeSet<String>, XtaskError> {
    let entries = fs::read_dir(directory)
        .map_err(|source| artifact_io_error("read artifact directory", directory, source))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|source| {
            artifact_io_error("read artifact directory entry", directory, source)
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| {
            artifact_io_error("inspect artifact directory entry", &path, source)
        })?;
        if !file_type.is_file() || file_type.is_symlink() {
            return Err(artifact_error(format!(
                "artifact directory contains a non-regular file: {}",
                path.display()
            )));
        }
        let name = entry.file_name().into_string().map_err(|_| {
            artifact_error(format!(
                "artifact directory entry is not valid UTF-8: {}",
                path.display()
            ))
        })?;
        names.insert(name);
    }
    Ok(names)
}

fn hash_framed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn hash_optional(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_framed(hasher, value.as_bytes());
        }
        None => hasher.update([0]),
    }
}

fn manifest_mismatch(
    field: &str,
    expected: impl std::fmt::Display,
    actual: impl std::fmt::Display,
) -> XtaskError {
    artifact_error(format!(
        "artifact manifest `{field}` is stale: expected `{expected}`, found `{actual}`"
    ))
}

fn artifact_io_error(action: &str, path: &Path, source: io::Error) -> XtaskError {
    artifact_error(format!("failed to {action} {}: {source}", path.display()))
}

fn artifact_error(message: impl Into<String>) -> XtaskError {
    XtaskError::TypstPackageFailed(format!("Typst WASM artifact: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_optimizer_version_accepts_release_build_suffix() {
        assert!(validate_typst_wasm_optimizer_version(WASM_OPT_VERSION).is_ok());
        assert!(
            validate_typst_wasm_optimizer_version("wasm-opt version 131 (version_131)").is_ok()
        );
        assert!(validate_typst_wasm_optimizer_version("wasm-opt version 130").is_err());
        assert!(validate_typst_wasm_optimizer_version("wasm-opt version 1310").is_err());
    }

    #[test]
    fn collects_fingerprint_with_canonical_workspace_and_metadata_paths() {
        let fixture = Fixture::new();
        let mut spec = fixture.spec("publish");
        spec.workspace_root = fixture.workspace.canonicalize().unwrap();

        let fingerprint = collect_input_fingerprint(&spec, &fixture.metadata()).unwrap();

        assert!(
            fingerprint
                .packages
                .iter()
                .any(|package| package.manifest_path == "crates/plugin/Cargo.toml")
        );
    }

    struct Fixture {
        _temporary: tempfile::TempDir,
        workspace: PathBuf,
        target: PathBuf,
    }

    struct FakeRuntime {
        metadata: CargoMetadataOutput,
        metadata_error: Option<String>,
        identity: ToolIdentity,
        rename_count: usize,
        fail_rename_at: Option<usize>,
    }

    impl ArtifactRuntime for FakeRuntime {
        fn cargo_metadata(
            &mut self,
            spec: &TypstArtifactSpec,
        ) -> Result<CargoMetadataOutput, XtaskError> {
            if let Some(error) = &self.metadata_error {
                return Err(artifact_error(format!("cargo metadata failed: {error}")));
            }
            let mut metadata = self.metadata.clone();
            let root_id = metadata
                .resolve
                .as_ref()
                .and_then(|resolve| resolve.root.as_ref())
                .cloned()
                .unwrap();
            let root = metadata
                .resolve
                .as_mut()
                .unwrap()
                .nodes
                .iter_mut()
                .find(|node| node.id == root_id)
                .unwrap();
            root.features = spec.features.clone();
            Ok(metadata)
        }

        fn tool_identity(&mut self) -> Result<ToolIdentity, XtaskError> {
            Ok(self.identity.clone())
        }

        fn optimize_wasm(&mut self, input: &Path, output: &Path) -> Result<(), XtaskError> {
            let bytes = fs::read(input)
                .map_err(|source| artifact_io_error("read fake optimize input", input, source))?;
            let mut optimized = b"optimized:".to_vec();
            optimized.extend_from_slice(&bytes);
            fs::write(output, optimized).map_err(|source| {
                artifact_io_error("write fake optimize output", output, source)
            })?;
            fs::write(input, b"mutated private raw copy")
                .map_err(|source| artifact_io_error("mutate fake optimize input", input, source))
        }

        fn strip_wasm(&mut self, input: &Path, output: &Path) -> Result<(), XtaskError> {
            let bytes = fs::read(input)
                .map_err(|source| artifact_io_error("read fake strip input", input, source))?;
            let mut stripped = b"stripped:".to_vec();
            stripped.extend_from_slice(&bytes);
            fs::write(output, stripped)
                .map_err(|source| artifact_io_error("write fake strip output", output, source))?;
            fs::write(input, b"mutated private copy")
                .map_err(|source| artifact_io_error("mutate fake strip input", input, source))
        }

        fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
            self.rename_count += 1;
            if self.fail_rename_at == Some(self.rename_count) {
                return Err(io::Error::other("injected rename failure"));
            }
            fs::rename(from, to)
        }
    }

    impl Fixture {
        fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let workspace = temporary.path().join("workspace");
            let target = temporary.path().join("target");
            write(
                &workspace.join("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/plugin\", \"crates/reachable\", \"crates/build-helper\", \"crates/dev-helper\", \"crates/xtask\"]\n\n[workspace.package]\nversion = \"0.8.0-alpha.3\"\n",
            );
            write(&workspace.join("Cargo.lock"), "version = 4\n");
            write(
                &workspace.join("distribution/typst/merman/typst.toml"),
                "[package]\nname = \"merman\"\nversion = \"0.3.0\"\n",
            );
            write(
                &workspace.join("crates/plugin/Cargo.toml"),
                "[package]\nname = \"merman-typst-plugin\"\nversion.workspace = true\n",
            );
            write(
                &workspace.join("crates/plugin/src/lib.rs"),
                "pub fn render() {}\n",
            );
            write(&workspace.join("crates/plugin/build.rs"), "fn main() {}\n");
            write(
                &workspace.join("crates/plugin/assets/font.bin"),
                "font-data",
            );
            write(
                &workspace.join("crates/plugin/wasm-profiles.json"),
                "{\"schema_version\":1}\n",
            );
            write(
                &workspace.join("crates/reachable/Cargo.toml"),
                "[package]\nname = \"reachable\"\nversion.workspace = true\n",
            );
            write(
                &workspace.join("crates/reachable/src/lib.rs"),
                "pub fn reachable() {}\n",
            );
            write(
                &workspace.join("crates/build-helper/Cargo.toml"),
                "[package]\nname = \"build-helper\"\nversion.workspace = true\n",
            );
            write(
                &workspace.join("crates/build-helper/src/lib.rs"),
                "pub fn build_helper() {}\n",
            );
            write(
                &workspace.join("crates/dev-helper/Cargo.toml"),
                "[package]\nname = \"dev-helper\"\nversion.workspace = true\n",
            );
            write(
                &workspace.join("crates/dev-helper/src/lib.rs"),
                "pub fn dev_only() {}\n",
            );
            write(
                &workspace.join("crates/xtask/Cargo.toml"),
                "[package]\nname = \"xtask\"\nversion.workspace = true\n",
            );
            write(
                &workspace.join("crates/xtask/src/lib.rs"),
                "pub fn xtask() {}\n",
            );
            Self {
                _temporary: temporary,
                workspace,
                target,
            }
        }

        fn spec(&self, profile: &str) -> TypstArtifactSpec {
            TypstArtifactSpec {
                workspace_root: self.workspace.clone(),
                target_root: self.target.clone(),
                artifact_profile: "typst-wasm".to_string(),
                profile: profile.to_string(),
                default_features: false,
                features: vec!["svg".to_string(), "analysis".to_string()],
                cargo_package: "merman-typst-plugin".to_string(),
                cargo_manifest_path: PathBuf::from("crates/plugin/Cargo.toml"),
                artifact_name: "merman_typst_plugin.wasm".to_string(),
                cargo_package_version: "0.8.0-alpha.3".to_string(),
                typst_package_version: "0.3.0".to_string(),
                plugin_abi_version: 2,
                mermaid_version: "11.16.0".to_string(),
                mermaid_source_commit: "7c0cafcf42e76bfaf79d0cbbd12edb986612f014".to_string(),
            }
            .normalized_and_validated()
            .unwrap()
        }

        fn raw_wasm(&self) -> PathBuf {
            let path = self.target.join("raw/merman_typst_plugin.wasm");
            write(&path, "raw-wasm");
            path
        }

        fn runtime(&self) -> FakeRuntime {
            FakeRuntime {
                metadata: self.metadata(),
                metadata_error: None,
                identity: ToolIdentity {
                    cargo_version: "cargo 1.90.0".to_string(),
                    rustc_version: "rustc 1.90.0\nhost: test".to_string(),
                    wasm_opt_version: WASM_OPT_VERSION.to_string(),
                    wasm_tools_version: "wasm-tools 1.240.0".to_string(),
                    rustflags: Some("-C target-feature=+bulk-memory".to_string()),
                    cargo_encoded_rustflags: None,
                },
                rename_count: 0,
                fail_rename_at: None,
            }
        }

        fn metadata(&self) -> CargoMetadataOutput {
            let package = |id: &str, name: &str, directory: &str, targets| MetadataPackage {
                id: id.to_string(),
                name: name.to_string(),
                manifest_path: self.workspace.join(directory).join("Cargo.toml"),
                source: None,
                targets,
            };
            let library_target = |directory: &str| MetadataTarget {
                kind: vec!["lib".to_string()],
                src_path: self.workspace.join(directory).join("src/lib.rs"),
            };
            let root_id = "path+file:///workspace/crates/plugin#merman-typst-plugin";
            let reachable_id = "path+file:///workspace/crates/reachable#reachable";
            let build_id = "path+file:///workspace/crates/build-helper#build-helper";
            let dev_id = "path+file:///workspace/crates/dev-helper#dev-helper";
            let xtask_id = "path+file:///workspace/crates/xtask#xtask";
            CargoMetadataOutput {
                packages: vec![
                    package(
                        root_id,
                        "merman-typst-plugin",
                        "crates/plugin",
                        vec![
                            library_target("crates/plugin"),
                            MetadataTarget {
                                kind: vec!["custom-build".to_string()],
                                src_path: self.workspace.join("crates/plugin/build.rs"),
                            },
                        ],
                    ),
                    package(
                        reachable_id,
                        "reachable",
                        "crates/reachable",
                        vec![library_target("crates/reachable")],
                    ),
                    package(
                        build_id,
                        "build-helper",
                        "crates/build-helper",
                        vec![library_target("crates/build-helper")],
                    ),
                    package(
                        dev_id,
                        "dev-helper",
                        "crates/dev-helper",
                        vec![library_target("crates/dev-helper")],
                    ),
                    package(
                        xtask_id,
                        "xtask",
                        "crates/xtask",
                        vec![library_target("crates/xtask")],
                    ),
                ],
                resolve: Some(MetadataResolve {
                    root: Some(root_id.to_string()),
                    nodes: vec![
                        MetadataNode {
                            id: root_id.to_string(),
                            deps: vec![
                                MetadataNodeDependency {
                                    pkg: reachable_id.to_string(),
                                    dep_kinds: vec![MetadataDependencyKind { kind: None }],
                                },
                                MetadataNodeDependency {
                                    pkg: dev_id.to_string(),
                                    dep_kinds: vec![MetadataDependencyKind {
                                        kind: Some("dev".to_string()),
                                    }],
                                },
                                MetadataNodeDependency {
                                    pkg: build_id.to_string(),
                                    dep_kinds: vec![MetadataDependencyKind {
                                        kind: Some("build".to_string()),
                                    }],
                                },
                            ],
                            features: vec!["analysis".to_string(), "svg".to_string()],
                        },
                        MetadataNode {
                            id: reachable_id.to_string(),
                            deps: Vec::new(),
                            features: Vec::new(),
                        },
                        MetadataNode {
                            id: build_id.to_string(),
                            deps: Vec::new(),
                            features: Vec::new(),
                        },
                        MetadataNode {
                            id: dev_id.to_string(),
                            deps: Vec::new(),
                            features: Vec::new(),
                        },
                        MetadataNode {
                            id: xtask_id.to_string(),
                            deps: Vec::new(),
                            features: Vec::new(),
                        },
                    ],
                }),
            }
        }
    }

    #[test]
    fn repository_spec_uses_the_canonical_workspace_identity() {
        let catalog = crate::cmd::typst_profiles::load_typst_profiles().unwrap();
        let profile = catalog.resolve_package(Some("publish")).unwrap();
        let artifact_profiles =
            crate::cmd::artifact_profiles::load_wasm_size_artifact_profiles().unwrap();
        let artifact_profile = artifact_profiles
            .iter()
            .find(|artifact| artifact.semantic_target == "typst")
            .unwrap();
        let spec =
            TypstArtifactSpec::for_repository_profile(&catalog, profile, artifact_profile).unwrap();

        assert_eq!(
            spec.workspace_root,
            paths::workspace_root().canonicalize().unwrap()
        );
        assert!(
            !spec
                .workspace_root
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        );
    }

    #[test]
    fn installs_a_profile_owned_artifact_without_mutating_the_raw_wasm() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        let mut runtime = fixture.runtime();

        let installed = install_with_runtime(&spec, &raw_wasm, &mut runtime).unwrap();

        assert_eq!(fs::read(&raw_wasm).unwrap(), b"raw-wasm");
        assert_eq!(
            fs::read(installed.wasm_path()).unwrap(),
            b"stripped:optimized:raw-wasm".as_slice()
        );
        let manifest: ArtifactManifest = serde_json::from_slice(
            &fs::read(spec.output_directory().join(MANIFEST_FILE_NAME)).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.schema_version, 3);
        assert_eq!(manifest.artifact_profile, "typst-wasm");
        assert_eq!(manifest.profile, "publish");
        assert!(!manifest.default_features);
        assert_eq!(manifest.features, ["analysis", "svg"]);
        assert_eq!(manifest.plugin_abi_version, 2);
        assert_eq!(
            manifest
                .input
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            ["build-helper", "merman-typst-plugin", "reachable"]
        );
        assert!(
            manifest
                .input
                .packages
                .iter()
                .all(|package| package.name != "dev-helper" && package.name != "xtask")
        );
        assert!(
            manifest
                .input
                .files
                .iter()
                .any(|file| file.path == "crates/plugin/build.rs")
        );
        assert!(
            manifest
                .input
                .files
                .iter()
                .any(|file| file.path == "crates/plugin/assets/font.bin")
        );
        assert_eq!(
            directory_entries(&spec.output_directory()).unwrap(),
            BTreeSet::from([
                MANIFEST_FILE_NAME.to_string(),
                "merman_typst_plugin.wasm".to_string()
            ])
        );
    }

    #[test]
    fn skip_rejects_a_stale_local_input_tree() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        write(
            &fixture.workspace.join("crates/plugin/src/lib.rs"),
            "pub fn render() { println!(\"changed\"); }\n",
        );

        let error = verify_with_runtime(&spec, &mut fixture.runtime()).unwrap_err();

        assert!(error.to_string().contains("input tree is stale"));
    }

    #[test]
    fn skip_rejects_a_change_in_a_reachable_production_dependency() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        write(
            &fixture.workspace.join("crates/reachable/src/lib.rs"),
            "pub fn reachable() { println!(\"changed\"); }\n",
        );

        let error = verify_with_runtime(&spec, &mut fixture.runtime()).unwrap_err();

        assert!(error.to_string().contains("input tree is stale"));
    }

    #[test]
    fn skip_ignores_unreachable_dev_and_test_inputs() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        write(
            &fixture.workspace.join("crates/xtask/src/lib.rs"),
            "pub fn xtask() { println!(\"changed\"); }\n",
        );
        write(
            &fixture.workspace.join("crates/dev-helper/src/lib.rs"),
            "pub fn dev_only() { println!(\"changed\"); }\n",
        );
        write(
            &fixture
                .workspace
                .join("crates/reachable/tests/integration.rs"),
            "#[test]\nfn test_only() {}\n",
        );

        verify_with_runtime(&spec, &mut fixture.runtime()).unwrap();
    }

    #[test]
    fn skip_rejects_a_changed_feature_set() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        let mut changed = spec.clone();
        changed.features.retain(|feature| feature != "analysis");

        let error = verify_with_runtime(&changed, &mut fixture.runtime()).unwrap_err();

        assert!(error.to_string().contains("features are stale"));
    }

    #[test]
    fn skip_rejects_a_changed_exact_artifact_profile() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        let mut changed = spec.clone();
        changed.artifact_profile = "typst-other".to_string();

        let error = verify_with_runtime(&changed, &mut fixture.runtime()).unwrap_err();

        assert!(error.to_string().contains("artifact_profile"));
    }

    #[test]
    fn skip_rejects_a_changed_default_feature_policy() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        let mut changed = spec.clone();
        changed.default_features = true;

        let error = verify_with_runtime(&changed, &mut fixture.runtime()).unwrap_err();

        assert!(error.to_string().contains("default_features is stale"));
    }

    #[test]
    fn cargo_metadata_failure_is_fail_closed() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        let mut runtime = fixture.runtime();
        runtime.metadata_error = Some("injected failure".to_string());

        let error = install_with_runtime(&spec, &raw_wasm, &mut runtime).unwrap_err();

        assert!(error.to_string().contains("cargo metadata failed"));
        assert!(!spec.output_directory().exists());
    }

    #[test]
    fn cargo_metadata_rejects_unrequested_root_features() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let mut metadata = fixture.metadata();
        let root_id = metadata.resolve.as_ref().unwrap().root.clone().unwrap();
        metadata
            .resolve
            .as_mut()
            .unwrap()
            .nodes
            .iter_mut()
            .find(|node| node.id == root_id)
            .unwrap()
            .features
            .push("unexpected".to_string());

        let error = collect_input_fingerprint(&spec, &metadata).unwrap_err();

        assert!(error.to_string().contains("do not exactly match"));
    }

    #[test]
    fn skip_rejects_a_stale_wasm_digest() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        let wasm = install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        let wasm_path = wasm.wasm_path().to_path_buf();
        drop(wasm);
        write(&wasm_path, "tampered");

        let error = verify_with_runtime(&spec, &mut fixture.runtime()).unwrap_err();

        assert!(error.to_string().contains("artifact digest or byte length"));
    }

    #[test]
    fn skip_rejects_an_artifact_relocated_to_another_profile() {
        let fixture = Fixture::new();
        let original = fixture.spec("publish");
        let other = fixture.spec("other-profile");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&original, &raw_wasm, &mut fixture.runtime()).unwrap();
        fs::rename(original.output_directory(), other.output_directory()).unwrap();

        let error = verify_with_runtime(&other, &mut fixture.runtime()).unwrap_err();

        assert!(error.to_string().contains("manifest `profile` is stale"));
    }

    #[test]
    fn installation_replaces_the_complete_profile_directory() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        write(&spec.output_directory().join("obsolete.txt"), "obsolete");

        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();

        assert!(!spec.output_directory().join("obsolete.txt").exists());
        verify_with_runtime(&spec, &mut fixture.runtime()).unwrap();
    }

    #[test]
    fn failed_atomic_install_restores_the_previous_directory() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        write(&spec.output_directory().join("previous.txt"), "previous");
        let mut runtime = fixture.runtime();
        runtime.fail_rename_at = Some(2);

        let error = install_with_runtime(&spec, &raw_wasm, &mut runtime).unwrap_err();

        assert!(error.to_string().contains("previous artifact restored"));
        assert_eq!(
            fs::read(spec.output_directory().join("previous.txt")).unwrap(),
            b"previous"
        );
        assert!(!spec.output_directory().join(MANIFEST_FILE_NAME).exists());
        let root_entries = fs::read_dir(spec.artifact_root())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            root_entries,
            BTreeSet::from([".locks".to_string(), "publish".to_string()])
        );
    }

    #[test]
    fn skip_rejects_stale_tool_versions_and_rustflags() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        let mut changed = fixture.runtime();
        changed.identity.rustflags = Some("-C opt-level=s".to_string());

        let error = verify_with_runtime(&spec, &mut changed).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("toolchain or effective Rust flags")
        );
    }

    #[test]
    fn manifest_parsing_fails_closed_on_unknown_fields() {
        let fixture = Fixture::new();
        let spec = fixture.spec("publish");
        let raw_wasm = fixture.raw_wasm();
        install_with_runtime(&spec, &raw_wasm, &mut fixture.runtime()).unwrap();
        let manifest_path = spec.output_directory().join(MANIFEST_FILE_NAME);
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest
            .as_object_mut()
            .unwrap()
            .insert("future_field".to_string(), serde_json::Value::Bool(true));
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let error = verify_with_runtime(&spec, &mut fixture.runtime()).unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}
