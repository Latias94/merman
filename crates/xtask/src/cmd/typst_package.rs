use crate::{
    XtaskError,
    cmd::{
        artifact_profiles::{WasmArtifactProfile, load_wasm_size_artifact_profiles},
        paths,
        typst_artifact::{TypstArtifactSpec, install_typst_artifact, verify_typst_artifact},
        typst_plugin_smoke::validate_typst_plugin,
        typst_profiles::{load_typst_profiles, validate_typst_artifact_profiles},
        wasm_build_lock::WorkspaceWasmBuildLock,
    },
    util::sha256_hex,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const PACKAGE_ARTIFACT_MANIFEST_FILE_NAME: &str = "merman_typst_plugin.manifest.json";
const PACKAGE_MANIFEST_FILE_NAME: &str = "merman_package.manifest.json";
const PACKAGE_MANIFEST_SCHEMA_VERSION: u32 = 1;
const EXPECTED_TYPST_PACKAGE_NAME: &str = "merman";
const COMPILE_FAIL_FIXTURE_DIRECTORY: &str = "compile-fail";
const RUNTIME_PACKAGE_EXCLUDES: [&str; 2] = ["examples/**", "tests/**"];

#[derive(Debug)]
struct Options {
    profile: Option<String>,
    out_dir: PathBuf,
    skip_wasm_build: bool,
}

#[derive(Debug)]
struct TypstPackageBuild {
    profile_name: String,
    package_dir: PathBuf,
}

#[derive(Debug)]
struct PackageVersionLock {
    _file: File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePackageSourceSnapshot {
    package_source: PathBuf,
    artifact_name: String,
    package_version: String,
    sha256: String,
    files: Vec<SnapshotFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotFile {
    path: String,
    bytes: Vec<u8>,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PackageManifest {
    schema_version: u32,
    package_name: String,
    package_version: String,
    profile: String,
    wrapper: PackageFileFingerprint,
    source: PackageSourceFingerprint,
    plugin: PackagePluginFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PackageSourceFingerprint {
    sha256: String,
    files: Vec<PackageFileFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PackagePluginFingerprint {
    wasm: PackageFileFingerprint,
    artifact_manifest: PackageArtifactManifestFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PackageArtifactManifestFingerprint {
    file: String,
    sha256: String,
    bytes: u64,
    profile: String,
    cargo_package_version: String,
    typst_package_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PackageFileFingerprint {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug)]
struct PluginArtifactSnapshot {
    wasm_file_name: String,
    wasm_bytes: Vec<u8>,
    artifact_manifest_bytes: Vec<u8>,
    package_manifest: PackagePluginFingerprint,
}

#[derive(Debug, serde::Deserialize)]
struct ArtifactManifestBinding {
    profile: String,
    cargo_package_version: String,
    typst_package_version: String,
    artifact: ArtifactManifestFileBinding,
}

#[derive(Debug, serde::Deserialize)]
struct ArtifactManifestFileBinding {
    file: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug)]
struct SmokeOptions {
    build: Options,
    compile_examples: bool,
    compile_tests: bool,
    keep_artifacts: bool,
    typst: Option<PathBuf>,
}

#[derive(Debug)]
struct TypstFixture {
    input: PathBuf,
    output: PathBuf,
    expected_error: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct TypstManifest {
    package: TypstManifestPackage,
}

#[derive(Debug, serde::Deserialize)]
struct TypstManifestPackage {
    name: String,
    version: String,
    exclude: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            profile: None,
            out_dir: paths::workspace_root().join("dist").join("typst"),
            skip_wasm_build: false,
        }
    }
}

impl RuntimePackageSourceSnapshot {
    fn capture(package_source: &Path, artifact_name: &str) -> Result<Self, XtaskError> {
        validate_package_directory(package_source, "Typst package source root")?;
        let reserved = BTreeSet::from([
            artifact_name.to_string(),
            PACKAGE_ARTIFACT_MANIFEST_FILE_NAME.to_string(),
            PACKAGE_MANIFEST_FILE_NAME.to_string(),
        ]);
        let mut files = Vec::new();
        collect_runtime_source_snapshot(package_source, package_source, &reserved, &mut files)?;
        files.sort_by(|left, right| left.path.cmp(&right.path));
        if files.windows(2).any(|pair| pair[0].path == pair[1].path) {
            return Err(package_transaction_error(
                "runtime package source contains colliding portable paths",
            ));
        }

        for required in [
            "typst.toml",
            "lib.typ",
            "README.md",
            "LICENSE",
            "THIRD_PARTY_NOTICES.md",
        ] {
            if !files.iter().any(|file| file.path == required) {
                return Err(package_transaction_error(format!(
                    "runtime package source is missing required file `{required}`"
                )));
            }
        }
        let manifest = files
            .iter()
            .find(|file| file.path == "typst.toml")
            .expect("required manifest was checked");
        let package_version =
            read_typst_package_version_bytes(&manifest.bytes, &package_source.join("typst.toml"))?;
        let sha256 = source_snapshot_digest(&files)?;
        Ok(Self {
            package_source: package_source.to_path_buf(),
            artifact_name: artifact_name.to_string(),
            package_version,
            sha256,
            files,
        })
    }

    fn required_file(&self, path: &str) -> Result<&SnapshotFile, XtaskError> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .ok_or_else(|| {
                package_transaction_error(format!(
                    "runtime package source snapshot is missing `{path}`"
                ))
            })
    }

    fn verify_live_identity(&self) -> Result<(), XtaskError> {
        let current = Self::capture(&self.package_source, &self.artifact_name)?;
        if current.sha256 != self.sha256 || current.files != self.files {
            return Err(package_transaction_error(format!(
                "runtime package source changed after snapshot; expected {}, found {}",
                self.sha256, current.sha256
            )));
        }
        Ok(())
    }

    fn fingerprints(&self) -> Result<Vec<PackageFileFingerprint>, XtaskError> {
        self.files
            .iter()
            .map(|file| file.fingerprint(&file.path))
            .collect()
    }
}

impl SnapshotFile {
    fn fingerprint(&self, path: &str) -> Result<PackageFileFingerprint, XtaskError> {
        Ok(PackageFileFingerprint {
            path: path.to_string(),
            sha256: self.sha256.clone(),
            bytes: byte_length(&self.bytes, path)?,
        })
    }
}

fn collect_runtime_source_snapshot(
    package_source: &Path,
    directory: &Path,
    reserved: &BTreeSet<String>,
    output: &mut Vec<SnapshotFile>,
) -> Result<(), XtaskError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|source| {
            package_transaction_io("read package source directory", directory, source)
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| package_transaction_io("read package source entry", directory, source))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(package_source).map_err(|_| {
            package_transaction_error(format!(
                "package source escaped its root: {}",
                path.display()
            ))
        })?;
        if runtime_path_is_excluded(relative) {
            continue;
        }
        let relative_path = portable_relative_path(relative)?;
        let file_type = entry.file_type().map_err(|source| {
            package_transaction_io("inspect package source entry", &path, source)
        })?;
        if file_type.is_symlink() {
            return Err(package_transaction_error(format!(
                "Typst package source must not contain symbolic links: {}",
                path.display()
            )));
        }
        if file_type.is_dir() {
            collect_runtime_source_snapshot(package_source, &path, reserved, output)?;
        } else if file_type.is_file() {
            if reserved.contains(&relative_path) {
                return Err(package_transaction_error(format!(
                    "Typst package source collides with generated file `{relative_path}`"
                )));
            }
            output.push(snapshot_file(relative_path, &path)?);
        } else {
            return Err(package_transaction_error(format!(
                "Typst package source contains a non-file entry: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn runtime_path_is_excluded(relative: &Path) -> bool {
    matches!(
        relative.components().next(),
        Some(Component::Normal(name)) if name == "examples" || name == "tests"
    )
}

fn portable_relative_path(path: &Path) -> Result<String, XtaskError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(package_transaction_error(format!(
                "package path is not canonical: {}",
                path.display()
            )));
        };
        let part = part.to_str().ok_or_else(|| {
            package_transaction_error(format!(
                "package path is not valid UTF-8: {}",
                path.display()
            ))
        })?;
        if part.contains('/') || part.contains('\\') {
            return Err(package_transaction_error(format!(
                "package path is not portable: {}",
                path.display()
            )));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(package_transaction_error("package path must name a file"));
    }
    Ok(parts.join("/"))
}

fn snapshot_file(relative_path: String, source: &Path) -> Result<SnapshotFile, XtaskError> {
    validate_package_file(source, "runtime package source file")?;
    let bytes = fs::read(source)
        .map_err(|error| package_transaction_io("read runtime package source", source, error))?;
    validate_package_file(source, "runtime package source file")?;
    Ok(SnapshotFile {
        path: relative_path,
        sha256: sha256_hex(&bytes),
        bytes,
    })
}

fn source_snapshot_digest(files: &[SnapshotFile]) -> Result<String, XtaskError> {
    let mut hasher = Sha256::new();
    hash_package_frame(&mut hasher, b"merman-typst-package-source-v1")?;
    for file in files {
        hash_package_frame(&mut hasher, file.path.as_bytes())?;
        hash_package_frame(
            &mut hasher,
            &byte_length(&file.bytes, &file.path)?.to_le_bytes(),
        )?;
        hash_package_frame(&mut hasher, file.sha256.as_bytes())?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_package_frame(hasher: &mut Sha256, bytes: &[u8]) -> Result<(), XtaskError> {
    let length = u64::try_from(bytes.len())
        .map_err(|_| package_transaction_error("package fingerprint input is too large"))?;
    hasher.update(length.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

fn byte_length(bytes: &[u8], path: &str) -> Result<u64, XtaskError> {
    u64::try_from(bytes.len())
        .map_err(|_| package_transaction_error(format!("package file is too large: {path}")))
}

pub(crate) fn build_typst_package(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let build = build_typst_package_with_options(&options)?;

    println_package_build_summary(&build.profile_name, &build.package_dir);
    Ok(())
}

pub(crate) fn typst_package_smoke(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_smoke_options(args)?;
    let build = build_typst_package_with_options(&options.build)?;
    println_package_build_summary(&build.profile_name, &build.package_dir);
    let package_dir = build.package_dir;

    let typst = find_typst_command(options.typst.as_deref())?;
    let root = paths::workspace_root();
    let package_source = root.join("distribution").join("typst").join("merman");
    let manifest_path = package_source.join("typst.toml");
    let package_version = read_typst_package_version(&manifest_path)?;
    let smoke_run = create_smoke_run_root(&paths::target_root())?;
    let smoke_root = smoke_run.path().to_path_buf();
    let package_path = smoke_root.join("packages");
    let preview_dir = package_path
        .join("preview")
        .join("merman")
        .join(&package_version);
    let output_dir = smoke_root.join("out");

    copy_dir_recursive(&package_dir, &preview_dir)?;
    fs::create_dir_all(&output_dir).map_err(|source| XtaskError::WriteFile {
        path: output_dir.display().to_string(),
        source,
    })?;

    let mut fixtures = Vec::new();
    if options.compile_examples {
        collect_typst_fixtures(
            &package_source.join("examples"),
            &output_dir.join("examples"),
            &mut fixtures,
        )?;
    }
    if options.compile_tests {
        collect_typst_fixtures(
            &package_source.join("tests"),
            &output_dir.join("tests"),
            &mut fixtures,
        )?;
    }
    fixtures.sort_by(|left, right| left.input.cmp(&right.input));

    if fixtures.is_empty() {
        return Err(XtaskError::TypstPackageSmokeFailed(
            "no Typst examples or tests were found to compile".to_string(),
        ));
    }

    let mut compiled = 0usize;
    let mut expected_failures = 0usize;
    for fixture in fixtures {
        if let Err(error) = compile_typst_fixture(&typst, &root, &package_path, &fixture) {
            let kept_root = smoke_run.keep();
            println!(
                "typst-package-smoke artifacts kept at {} after failure",
                kept_root.display()
            );
            return Err(error);
        }
        if fixture.expected_error.is_some() {
            expected_failures += 1;
        } else {
            compiled += 1;
        }
    }

    println!(
        "typst-package-smoke OK package={} compiled={compiled} expected_failures={expected_failures} package_path={}",
        package_dir.display(),
        package_path.display()
    );

    if options.keep_artifacts {
        let kept_root = smoke_run.keep();
        println!(
            "typst-package-smoke artifacts kept at {}",
            kept_root.display()
        );
    } else {
        smoke_run.close().map_err(|source| XtaskError::WriteFile {
            path: smoke_root.display().to_string(),
            source,
        })?;
    }

    Ok(())
}

fn create_smoke_run_root(target_root: &Path) -> Result<tempfile::TempDir, XtaskError> {
    let smoke_root = target_root.join("typst-package-smoke");
    fs::create_dir_all(&smoke_root).map_err(|source| XtaskError::WriteFile {
        path: smoke_root.display().to_string(),
        source,
    })?;
    tempfile::Builder::new()
        .prefix("run-")
        .tempdir_in(&smoke_root)
        .map_err(|source| XtaskError::WriteFile {
            path: smoke_root.display().to_string(),
            source,
        })
}

fn build_typst_package_with_options(options: &Options) -> Result<TypstPackageBuild, XtaskError> {
    let root = paths::workspace_root();
    let profile_catalog = load_typst_profiles()?;
    let profile = profile_catalog.resolve_package(options.profile.as_deref())?;
    let artifact_profiles = load_wasm_size_artifact_profiles().map_err(|error| {
        XtaskError::TypstPackageFailed(format!(
            "failed to load the exact Typst artifact recipe: {error}"
        ))
    })?;
    validate_typst_artifact_profiles(&profile_catalog, &artifact_profiles)?;
    let artifact_profile = artifact_profiles
        .iter()
        .find(|artifact| artifact.semantic_target == "typst")
        .ok_or_else(|| {
            XtaskError::TypstPackageFailed("the exact Typst artifact recipe is missing".to_string())
        })?;
    let package_source = root.join("distribution").join("typst").join("merman");
    let source_snapshot =
        RuntimePackageSourceSnapshot::capture(&package_source, profile_catalog.artifact_name())?;
    let artifact_spec =
        TypstArtifactSpec::for_repository_profile(&profile_catalog, profile, artifact_profile)?;
    let artifact = if options.skip_wasm_build {
        verify_typst_artifact(&artifact_spec)?
    } else {
        let _build_lock = WorkspaceWasmBuildLock::acquire()?;
        let raw_wasm = build_wasm(artifact_profile)?;
        install_typst_artifact(&artifact_spec, &raw_wasm)?
    };
    validate_typst_plugin(artifact.wasm_path(), &profile_catalog, profile)?;

    let package_dir = stage_and_install_package(
        &source_snapshot,
        &options.out_dir,
        profile.name(),
        artifact.wasm_path(),
        artifact.manifest_path(),
    )?;
    let package_dir = package_dir
        .canonicalize()
        .unwrap_or_else(|_| package_dir.clone());
    Ok(TypstPackageBuild {
        profile_name: profile.name().to_string(),
        package_dir,
    })
}

fn println_package_build_summary(profile_name: &str, package_dir: &Path) {
    println!(
        "Typst package built profile={} path={}",
        profile_name,
        package_dir.display()
    );
    println!(
        "Local install target: <typst package path>/local/merman/{}",
        package_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<version>")
    );
    println!(
        "Preview smoke target: <typst package path>/preview/merman/{}",
        package_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<version>")
    );
    println!("Tip: run `typst info` to find your Typst package path.");
}

fn read_typst_package_version(manifest_path: &Path) -> Result<String, XtaskError> {
    let manifest_bytes = fs::read(manifest_path).map_err(|source| XtaskError::ReadFile {
        path: manifest_path.display().to_string(),
        source,
    })?;
    read_typst_package_version_bytes(&manifest_bytes, manifest_path)
}

fn read_typst_package_version_bytes(
    manifest_bytes: &[u8],
    manifest_path: &Path,
) -> Result<String, XtaskError> {
    let manifest_text = std::str::from_utf8(manifest_bytes).map_err(|error| {
        XtaskError::TypstPackageFailed(format!(
            "{} must be valid UTF-8: {error}",
            manifest_path.display()
        ))
    })?;
    let manifest: TypstManifest = toml::from_str(manifest_text).map_err(|source| {
        XtaskError::TypstPackageFailed(format!(
            "failed to parse {}: {source}",
            manifest_path.display()
        ))
    })?;
    if manifest.package.name != EXPECTED_TYPST_PACKAGE_NAME {
        return Err(XtaskError::TypstPackageFailed(format!(
            "{} must declare package name `{EXPECTED_TYPST_PACKAGE_NAME}`, found `{}`",
            manifest_path.display(),
            manifest.package.name
        )));
    }
    let version = manifest.package.version.trim();
    if !is_typst_package_version(version) {
        return Err(XtaskError::TypstPackageFailed(format!(
            "{} has unsupported Typst package version `{}`; Typst imports require an x.y.z numeric version",
            manifest_path.display(),
            manifest.package.version
        )));
    }
    let excludes = manifest
        .package
        .exclude
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_excludes = RUNTIME_PACKAGE_EXCLUDES
        .into_iter()
        .collect::<BTreeSet<_>>();
    if excludes != expected_excludes || manifest.package.exclude.len() != expected_excludes.len() {
        return Err(XtaskError::TypstPackageFailed(format!(
            "{} must exclude exactly [{}] from the runtime bundle",
            manifest_path.display(),
            RUNTIME_PACKAGE_EXCLUDES.join(", ")
        )));
    }
    Ok(version.to_string())
}

fn is_typst_package_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }

    [major, minor, patch]
        .into_iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn parse_options(args: Vec<String>) -> Result<Options, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage();
        return Err(XtaskError::Usage);
    }

    let mut options = Options::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--profile" => {
                options.profile = Some(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--out" => {
                options.out_dir = PathBuf::from(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--skip-wasm-build" => {
                options.skip_wasm_build = true;
            }
            _ => {
                print_usage();
                return Err(XtaskError::Usage);
            }
        }
    }

    Ok(options)
}

fn print_usage() {
    println!(
        "usage: xtask build-typst-package [--profile <name>] [--out <dir>] [--skip-wasm-build]"
    );
    print_profile_names();
}

fn parse_smoke_options(args: Vec<String>) -> Result<SmokeOptions, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_smoke_usage();
        return Err(XtaskError::Usage);
    }

    let mut options = SmokeOptions {
        build: Options::default(),
        compile_examples: true,
        compile_tests: true,
        keep_artifacts: false,
        typst: None,
    };

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--profile" => {
                options.build.profile = Some(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--out" => {
                options.build.out_dir = PathBuf::from(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--skip-wasm-build" => {
                options.build.skip_wasm_build = true;
            }
            "--examples-only" => {
                options.compile_examples = true;
                options.compile_tests = false;
            }
            "--tests-only" => {
                options.compile_examples = false;
                options.compile_tests = true;
            }
            "--keep-artifacts" => {
                options.keep_artifacts = true;
            }
            "--typst" => {
                options.typst = Some(PathBuf::from(iter.next().ok_or(XtaskError::Usage)?));
            }
            _ => {
                print_smoke_usage();
                return Err(XtaskError::Usage);
            }
        }
    }

    Ok(options)
}

fn print_smoke_usage() {
    println!(
        "usage: xtask typst-package-smoke [--profile <name>] [--out <dir>] [--skip-wasm-build] [--examples-only|--tests-only] [--keep-artifacts] [--typst <path>]"
    );
    print_profile_names();
}

fn print_profile_names() {
    if let Ok(catalog) = load_typst_profiles() {
        println!(
            "Typst package profile: {}",
            catalog.package_profile().name()
        );
    }
}

fn find_typst_command(explicit: Option<&Path>) -> Result<PathBuf, XtaskError> {
    let typst = explicit.map_or_else(|| PathBuf::from("typst"), PathBuf::from);
    let status = Command::new(&typst)
        .arg("--version")
        .status()
        .map_err(|source| {
            XtaskError::TypstPackageSmokeFailed(format!(
                "failed to execute `{}` --version: {source}",
                typst.display()
            ))
        })?;
    if status.success() {
        Ok(typst)
    } else {
        Err(XtaskError::TypstPackageSmokeFailed(format!(
            "`{}` --version failed with status {status}",
            typst.display()
        )))
    }
}

fn collect_typst_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), XtaskError> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|source| XtaskError::ReadFile {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_typst_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("typ") {
            out.push(path);
        }
    }

    Ok(())
}

fn collect_typst_fixtures(
    input_root: &Path,
    output_root: &Path,
    out: &mut Vec<TypstFixture>,
) -> Result<(), XtaskError> {
    let mut inputs = Vec::new();
    collect_typst_files(input_root, &mut inputs)?;
    for input in inputs {
        let output = typst_fixture_output_path(input_root, output_root, &input)?;
        let expected_error = expected_typst_fixture_error(input_root, &input)?;
        out.push(TypstFixture {
            input,
            output,
            expected_error,
        });
    }
    Ok(())
}

fn expected_typst_fixture_error(
    input_root: &Path,
    input: &Path,
) -> Result<Option<String>, XtaskError> {
    let relative = input.strip_prefix(input_root).map_err(|_| {
        XtaskError::TypstPackageSmokeFailed(format!(
            "fixture {} is outside input root {}",
            input.display(),
            input_root.display()
        ))
    })?;
    let is_compile_fail = matches!(
        relative.components().next(),
        Some(Component::Normal(name)) if name == COMPILE_FAIL_FIXTURE_DIRECTORY
    );
    if !is_compile_fail {
        return Ok(None);
    }

    let expectation_path = input.with_extension("error.txt");
    let expected = fs::read_to_string(&expectation_path).map_err(|source| {
        XtaskError::TypstPackageSmokeFailed(format!(
            "expected-failure fixture {} requires readable diagnostic file {}: {source}",
            input.display(),
            expectation_path.display()
        ))
    })?;
    let expected = expected.trim().to_string();
    if expected.is_empty() {
        return Err(XtaskError::TypstPackageSmokeFailed(format!(
            "expected-failure diagnostic file {} must not be empty",
            expectation_path.display()
        )));
    }
    Ok(Some(expected))
}

fn typst_fixture_output_path(
    input_root: &Path,
    output_root: &Path,
    input: &Path,
) -> Result<PathBuf, XtaskError> {
    let relative = input.strip_prefix(input_root).map_err(|_| {
        XtaskError::TypstPackageSmokeFailed(format!(
            "fixture {} is outside input root {}",
            input.display(),
            input_root.display()
        ))
    })?;
    Ok(output_root.join(relative).with_extension("pdf"))
}

fn compile_typst_fixture(
    typst: &Path,
    project_root: &Path,
    package_path: &Path,
    fixture: &TypstFixture,
) -> Result<(), XtaskError> {
    if let Some(parent) = fixture.output.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let output = Command::new(typst)
        .args(["compile", "--root"])
        .arg(project_root)
        .arg("--package-path")
        .arg(package_path)
        .arg(&fixture.input)
        .arg(&fixture.output)
        .env("NO_COLOR", "1")
        .output()
        .map_err(|source| {
            XtaskError::TypstPackageSmokeFailed(format!(
                "failed to compile {}: {source}",
                fixture.input.display()
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(expected_error) = fixture.expected_error.as_deref() {
        if output.status.success() {
            return Err(XtaskError::TypstPackageSmokeFailed(format!(
                "expected Typst fixture {} to fail with `{expected_error}`, but it compiled successfully",
                fixture.input.display()
            )));
        }
        if !stderr.contains(expected_error) {
            return Err(XtaskError::TypstPackageSmokeFailed(format!(
                "Typst fixture {} failed without expected diagnostic `{expected_error}`; stdout: {}; stderr: {}",
                fixture.input.display(),
                stdout.trim(),
                stderr.trim()
            )));
        }
        println!(
            "verified expected Typst failure {} diagnostic={expected_error:?}",
            fixture.input.display()
        );
        return Ok(());
    }

    if output.status.success() {
        println!(
            "compiled Typst fixture {} -> {}",
            fixture.input.display(),
            fixture.output.display()
        );
        return Ok(());
    }

    Err(XtaskError::TypstPackageSmokeFailed(format!(
        "typst compile failed for {} with status {}; stdout: {}; stderr: {}",
        fixture.input.display(),
        output.status,
        stdout.trim(),
        stderr.trim()
    )))
}

fn build_wasm(profile: &WasmArtifactProfile) -> Result<PathBuf, XtaskError> {
    let target_root = paths::wasm_build_target_root();
    let mut command = Command::new("cargo");
    command.args([
        "build",
        "--locked",
        "-p",
        &profile.package,
        "--profile",
        &profile.cargo_profile,
        "--target",
        &profile.target_triple,
    ]);
    command.arg("--target-dir").arg(&target_root);
    command.current_dir(paths::workspace_root());
    profile.configure_cargo_command(&mut command);

    let status = command.status().map_err(|source| XtaskError::ReadFile {
        path: "cargo".to_string(),
        source,
    })?;
    if !status.success() {
        return Err(XtaskError::TypstPackageFailed(format!(
            "cargo build failed with status {status}"
        )));
    }

    Ok(target_root
        .join(&profile.target_triple)
        .join(&profile.cargo_profile)
        .join(&profile.artifact_name))
}

impl PluginArtifactSnapshot {
    fn capture(
        artifact_wasm: &Path,
        artifact_manifest: &Path,
        expected_profile: &str,
        expected_typst_version: &str,
    ) -> Result<Self, XtaskError> {
        validate_package_file(artifact_wasm, "Typst plugin WASM artifact")?;
        validate_package_file(artifact_manifest, "Typst plugin artifact manifest")?;
        let wasm_file_name = artifact_wasm
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                package_transaction_error(format!(
                    "WASM artifact has no UTF-8 file name: {}",
                    artifact_wasm.display()
                ))
            })?
            .to_string();
        let wasm_bytes = fs::read(artifact_wasm).map_err(|error| {
            package_transaction_io("read Typst plugin WASM artifact", artifact_wasm, error)
        })?;
        let artifact_manifest_bytes = fs::read(artifact_manifest).map_err(|error| {
            package_transaction_io(
                "read Typst plugin artifact manifest",
                artifact_manifest,
                error,
            )
        })?;
        let binding: ArtifactManifestBinding = serde_json::from_slice(&artifact_manifest_bytes)
            .map_err(|error| {
                package_transaction_error(format!(
                    "Typst plugin artifact manifest is not valid JSON: {error}"
                ))
            })?;
        let wasm_sha256 = sha256_hex(&wasm_bytes);
        let wasm_bytes_length = byte_length(&wasm_bytes, &wasm_file_name)?;
        if binding.profile != expected_profile
            || binding.typst_package_version != expected_typst_version
            || binding.artifact.file != wasm_file_name
            || binding.artifact.sha256 != wasm_sha256
            || binding.artifact.bytes != wasm_bytes_length
            || binding.cargo_package_version.trim().is_empty()
        {
            return Err(package_transaction_error(format!(
                "Typst plugin artifact manifest does not bind profile `{expected_profile}`, Typst package `{expected_typst_version}`, and WASM `{wasm_file_name}`"
            )));
        }
        let package_manifest = PackagePluginFingerprint {
            wasm: PackageFileFingerprint {
                path: wasm_file_name.clone(),
                sha256: wasm_sha256,
                bytes: wasm_bytes_length,
            },
            artifact_manifest: PackageArtifactManifestFingerprint {
                file: PACKAGE_ARTIFACT_MANIFEST_FILE_NAME.to_string(),
                sha256: sha256_hex(&artifact_manifest_bytes),
                bytes: byte_length(
                    &artifact_manifest_bytes,
                    PACKAGE_ARTIFACT_MANIFEST_FILE_NAME,
                )?,
                profile: binding.profile,
                cargo_package_version: binding.cargo_package_version,
                typst_package_version: binding.typst_package_version,
            },
        };
        Ok(Self {
            wasm_file_name,
            wasm_bytes,
            artifact_manifest_bytes,
            package_manifest,
        })
    }
}

fn package_manifest(
    source: &RuntimePackageSourceSnapshot,
    profile: &str,
    plugin: &PluginArtifactSnapshot,
) -> Result<PackageManifest, XtaskError> {
    let wrapper = source.required_file("lib.typ")?.fingerprint("lib.typ")?;
    Ok(PackageManifest {
        schema_version: PACKAGE_MANIFEST_SCHEMA_VERSION,
        package_name: EXPECTED_TYPST_PACKAGE_NAME.to_string(),
        package_version: source.package_version.clone(),
        profile: profile.to_string(),
        wrapper,
        source: PackageSourceFingerprint {
            sha256: source.sha256.clone(),
            files: source.fingerprints()?,
        },
        plugin: plugin.package_manifest.clone(),
    })
}

fn serialize_package_manifest(manifest: &PackageManifest) -> Result<Vec<u8>, XtaskError> {
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|error| {
        package_transaction_error(format!("serialize package manifest: {error}"))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_staged_file(staged: &Path, relative: &str, bytes: &[u8]) -> Result<(), XtaskError> {
    let relative_path = Path::new(relative);
    if portable_relative_path(relative_path)? != relative {
        return Err(package_transaction_error(format!(
            "staged package path is not canonical: `{relative}`"
        )));
    }
    let destination = staged.join(relative_path);
    let parent = destination.parent().ok_or_else(|| {
        package_transaction_error(format!(
            "staged package file has no parent: {}",
            destination.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        package_transaction_io("create staged package directory", parent, error)
    })?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&destination)
        .map_err(|error| {
            package_transaction_io("create staged package file", &destination, error)
        })?;
    file.write_all(bytes)
        .map_err(|error| package_transaction_io("write staged package file", &destination, error))
}

fn expected_package_directories(files: &BTreeSet<String>) -> BTreeSet<String> {
    let mut directories = BTreeSet::new();
    for file in files {
        let mut parent = Path::new(file).parent();
        while let Some(directory) = parent {
            if directory.as_os_str().is_empty() {
                break;
            }
            directories.insert(
                directory
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
            parent = directory.parent();
        }
    }
    directories
}

fn staged_package_shape(staged: &Path) -> Result<(BTreeSet<String>, BTreeSet<String>), XtaskError> {
    validate_package_directory(staged, "staged Typst package")?;
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    collect_staged_package_shape(staged, staged, &mut files, &mut directories)?;
    Ok((files, directories))
}

fn collect_staged_package_shape(
    root: &Path,
    directory: &Path,
    files: &mut BTreeSet<String>,
    directories: &mut BTreeSet<String>,
) -> Result<(), XtaskError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| package_transaction_io("read staged package directory", directory, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| package_transaction_io("read staged package entry", directory, error))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = portable_relative_path(path.strip_prefix(root).map_err(|_| {
            package_transaction_error(format!(
                "staged package entry escaped its root: {}",
                path.display()
            ))
        })?)?;
        let file_type = entry.file_type().map_err(|error| {
            package_transaction_io("inspect staged package entry", &path, error)
        })?;
        if file_type.is_symlink() {
            return Err(package_transaction_error(format!(
                "staged package must not contain symbolic links: {}",
                path.display()
            )));
        }
        if file_type.is_dir() {
            directories.insert(relative);
            collect_staged_package_shape(root, &path, files, directories)?;
        } else if file_type.is_file() {
            files.insert(relative);
        } else {
            return Err(package_transaction_error(format!(
                "staged package contains a non-file entry: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn stage_and_install_package(
    source: &RuntimePackageSourceSnapshot,
    out_dir: &Path,
    profile: &str,
    artifact_wasm: &Path,
    artifact_manifest: &Path,
) -> Result<PathBuf, XtaskError> {
    stage_and_install_package_with_hook(
        source,
        out_dir,
        profile,
        artifact_wasm,
        artifact_manifest,
        || {},
    )
}

fn stage_and_install_package_with_hook<F>(
    source: &RuntimePackageSourceSnapshot,
    out_dir: &Path,
    profile: &str,
    artifact_wasm: &Path,
    artifact_manifest: &Path,
    before_source_revalidation: F,
) -> Result<PathBuf, XtaskError>
where
    F: FnOnce(),
{
    let plugin = PluginArtifactSnapshot::capture(
        artifact_wasm,
        artifact_manifest,
        profile,
        &source.package_version,
    )?;
    if plugin.wasm_file_name != source.artifact_name {
        return Err(package_transaction_error(format!(
            "artifact file name must match source snapshot descriptor `{}`, found `{}`",
            source.artifact_name, plugin.wasm_file_name
        )));
    }
    let package_manifest = package_manifest(source, profile, &plugin)?;
    let package_manifest_bytes = serialize_package_manifest(&package_manifest)?;
    let package_version = &source.package_version;
    let package_parent = out_dir.join("merman");
    fs::create_dir_all(&package_parent).map_err(|source| XtaskError::WriteFile {
        path: package_parent.display().to_string(),
        source,
    })?;
    validate_package_directory(&package_parent, "Typst package output root")?;
    let _package_lock = acquire_package_version_lock(&package_parent, package_version)?;

    let staging = tempfile::Builder::new()
        .prefix(&format!(".{package_version}.staging-"))
        .tempdir_in(&package_parent)
        .map_err(|source| {
            package_transaction_io("create package staging directory", &package_parent, source)
        })?;
    let staged = staging.path();
    for file in &source.files {
        write_staged_file(staged, &file.path, &file.bytes)?;
    }
    write_staged_file(staged, &plugin.wasm_file_name, &plugin.wasm_bytes)?;
    write_staged_file(
        staged,
        PACKAGE_ARTIFACT_MANIFEST_FILE_NAME,
        &plugin.artifact_manifest_bytes,
    )?;
    write_staged_file(staged, PACKAGE_MANIFEST_FILE_NAME, &package_manifest_bytes)?;
    validate_staged_package(
        staged,
        source,
        &plugin,
        &package_manifest,
        &package_manifest_bytes,
    )?;
    before_source_revalidation();
    source.verify_live_identity()?;
    validate_staged_package(
        staged,
        source,
        &plugin,
        &package_manifest,
        &package_manifest_bytes,
    )?;

    let package_dir = package_parent.join(package_version);
    install_staged_package(&package_dir, staging, |from, to| fs::rename(from, to))?;
    Ok(package_dir)
}

fn validate_staged_package(
    staged: &Path,
    source: &RuntimePackageSourceSnapshot,
    plugin: &PluginArtifactSnapshot,
    package_manifest: &PackageManifest,
    package_manifest_bytes: &[u8],
) -> Result<(), XtaskError> {
    let mut expected_files = source
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    expected_files.extend([
        plugin.wasm_file_name.clone(),
        PACKAGE_ARTIFACT_MANIFEST_FILE_NAME.to_string(),
        PACKAGE_MANIFEST_FILE_NAME.to_string(),
    ]);
    let expected_directories = expected_package_directories(&expected_files);
    let (actual_files, actual_directories) = staged_package_shape(staged)?;
    if actual_files != expected_files || actual_directories != expected_directories {
        return Err(package_transaction_error(format!(
            "staged package shape does not match its snapshot; expected files [{}] and directories [{}], found files [{}] and directories [{}]",
            expected_files
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            expected_directories
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            actual_files.iter().cloned().collect::<Vec<_>>().join(", "),
            actual_directories
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    for file in &source.files {
        ensure_staged_bytes(staged, &file.path, &file.bytes)?;
    }
    ensure_staged_bytes(staged, &plugin.wasm_file_name, &plugin.wasm_bytes)?;
    ensure_staged_bytes(
        staged,
        PACKAGE_ARTIFACT_MANIFEST_FILE_NAME,
        &plugin.artifact_manifest_bytes,
    )?;
    ensure_staged_bytes(staged, PACKAGE_MANIFEST_FILE_NAME, package_manifest_bytes)?;
    let parsed: PackageManifest =
        serde_json::from_slice(package_manifest_bytes).map_err(|error| {
            package_transaction_error(format!(
                "package manifest is not valid schema-1 JSON: {error}"
            ))
        })?;
    if &parsed != package_manifest {
        return Err(package_transaction_error(
            "staged package manifest does not match the frozen package inputs",
        ));
    }
    Ok(())
}

fn ensure_staged_bytes(staged: &Path, relative: &str, expected: &[u8]) -> Result<(), XtaskError> {
    let destination = staged.join(relative);
    validate_package_file(&destination, "staged Typst package file")?;
    let actual = fs::read(&destination)
        .map_err(|error| package_transaction_io("read staged package file", &destination, error))?;
    if actual != expected {
        return Err(package_transaction_error(format!(
            "staged package file `{relative}` does not match its frozen input"
        )));
    }
    Ok(())
}

fn install_staged_package<R>(
    package_dir: &Path,
    staging: tempfile::TempDir,
    mut rename: R,
) -> Result<(), XtaskError>
where
    R: FnMut(&Path, &Path) -> io::Result<()>,
{
    let package_parent = package_dir.parent().ok_or_else(|| {
        package_transaction_error(format!(
            "package directory has no parent: {}",
            package_dir.display()
        ))
    })?;
    let had_original = match fs::symlink_metadata(package_dir) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => true,
        Ok(_) => {
            return Err(package_transaction_error(format!(
                "package destination is not a regular directory: {}",
                package_dir.display()
            )));
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(package_transaction_io(
                "inspect package destination",
                package_dir,
                source,
            ));
        }
    };

    let backup = tempfile::Builder::new()
        .prefix(".package-backup-")
        .tempdir_in(package_parent)
        .map_err(|source| {
            package_transaction_io("create package rollback directory", package_parent, source)
        })?;
    let backup_path = backup.path().join("previous");
    if had_original {
        rename(package_dir, &backup_path).map_err(|source| {
            package_transaction_io(
                "move existing package into rollback storage",
                package_dir,
                source,
            )
        })?;
    }

    if let Err(install_error) = rename(staging.path(), package_dir) {
        if had_original && let Err(rollback_error) = rename(&backup_path, package_dir) {
            let preserved = backup.keep().join("previous");
            return Err(package_transaction_error(format!(
                "failed to install staged package at {}: {install_error}; failed to restore the previous package: {rollback_error}; previous package preserved at {}",
                package_dir.display(),
                preserved.display()
            )));
        }
        backup.close().map_err(|source| {
            package_transaction_io(
                "clean empty package rollback directory",
                package_parent,
                source,
            )
        })?;
        let recovery = if had_original {
            "previous package restored"
        } else {
            "no previous package existed"
        };
        return Err(package_transaction_error(format!(
            "failed to install staged package at {}: {install_error}; {recovery}",
            package_dir.display()
        )));
    }

    backup.close().map_err(|source| {
        package_transaction_io(
            "remove committed package rollback directory",
            package_parent,
            source,
        )
    })?;
    Ok(())
}

fn validate_package_directory(path: &Path, kind: &str) -> Result<(), XtaskError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| package_transaction_io(&format!("inspect {kind}"), path, source))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(package_transaction_error(format!(
            "{kind} must be a regular directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_package_file(path: &Path, kind: &str) -> Result<(), XtaskError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| package_transaction_io(&format!("inspect {kind}"), path, source))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(package_transaction_error(format!(
            "{kind} must be a regular file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn acquire_package_version_lock(
    package_parent: &Path,
    package_version: &str,
) -> Result<PackageVersionLock, XtaskError> {
    if !is_typst_package_version(package_version) {
        return Err(package_transaction_error(format!(
            "cannot lock invalid Typst package version `{package_version}`"
        )));
    }
    let lock_root = package_parent.join(".locks");
    fs::create_dir_all(&lock_root).map_err(|source| {
        package_transaction_io("create package lock directory", &lock_root, source)
    })?;
    validate_package_directory(&lock_root, "Typst package lock root")?;
    let lock_path = lock_root.join(format!("{package_version}.lock"));
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| {
            package_transaction_io("open package version lock", &lock_path, source)
        })?;
    fs2::FileExt::lock_exclusive(&file).map_err(|source| {
        package_transaction_io("lock Typst package version", &lock_path, source)
    })?;
    Ok(PackageVersionLock { _file: file })
}

fn package_transaction_io(action: &str, path: &Path, source: io::Error) -> XtaskError {
    package_transaction_error(format!("failed to {action} {}: {source}", path.display()))
}

fn package_transaction_error(message: impl Into<String>) -> XtaskError {
    XtaskError::TypstPackageFailed(format!("Typst package transaction: {}", message.into()))
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), XtaskError> {
    validate_package_file(source, "Typst package source file")?;
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|source_err| XtaskError::WriteFile {
            path: destination.display().to_string(),
            source: source_err,
        })
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), XtaskError> {
    validate_package_directory(source, "Typst package source directory")?;
    fs::create_dir_all(destination).map_err(|source_err| XtaskError::WriteFile {
        path: destination.display().to_string(),
        source: source_err,
    })?;

    let mut entries = fs::read_dir(source)
        .map_err(|source_err| XtaskError::ReadFile {
            path: source.display().to_string(),
            source: source_err,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source_err| XtaskError::ReadFile {
            path: source.display().to_string(),
            source: source_err,
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|source_err| XtaskError::ReadFile {
                path: source_path.display().to_string(),
                source: source_err,
            })?;
        if file_type.is_symlink() {
            return Err(package_transaction_error(format!(
                "Typst package source must not contain symbolic links: {}",
                source_path.display()
            )));
        }
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            copy_file(&source_path, &destination_path)?;
        } else {
            return Err(package_transaction_error(format!(
                "Typst package source contains a non-file entry: {}",
                source_path.display()
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PACKAGE_ARTIFACT_MANIFEST_FILE_NAME, PACKAGE_MANIFEST_FILE_NAME, PackageManifest,
        PluginArtifactSnapshot, RuntimePackageSourceSnapshot, collect_typst_files,
        collect_typst_fixtures, copy_dir_recursive, copy_file, create_smoke_run_root,
        install_staged_package, is_typst_package_version, package_manifest, parse_smoke_options,
        serialize_package_manifest, stage_and_install_package, stage_and_install_package_with_hook,
        typst_fixture_output_path, validate_staged_package, write_staged_file,
    };
    use crate::util::sha256_hex;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn typst_package_version_accepts_numeric_triplets() {
        assert!(is_typst_package_version("0.8.0"));
        assert!(is_typst_package_version("10.20.30"));
    }

    #[test]
    fn typst_package_version_rejects_prerelease_forms() {
        assert!(!is_typst_package_version("0.8.0-alpha.1"));
        assert!(!is_typst_package_version("0.8.0a1"));
        assert!(!is_typst_package_version("0.8"));
        assert!(!is_typst_package_version("0.8.0.1"));
    }

    #[test]
    fn collect_typst_files_ignores_missing_directories() {
        let mut out = Vec::new();
        collect_typst_files(Path::new("target/definitely-missing-typst-dir"), &mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn smoke_options_accept_explicit_typst_binary() {
        let options = parse_smoke_options(vec![
            "--skip-wasm-build".to_string(),
            "--typst".to_string(),
            "target/typst-local/typst.exe".to_string(),
        ])
        .unwrap();

        assert_eq!(
            options.typst.as_deref(),
            Some(Path::new("target/typst-local/typst.exe"))
        );
        assert!(options.build.skip_wasm_build);
    }

    #[test]
    fn smoke_runs_use_independent_owned_directories() {
        let temporary = tempfile::tempdir().unwrap();
        let first = create_smoke_run_root(temporary.path()).unwrap();
        let second = create_smoke_run_root(temporary.path()).unwrap();
        assert_ne!(first.path(), second.path());
        fs::write(first.path().join("first"), "first").unwrap();
        fs::write(second.path().join("second"), "second").unwrap();

        first.close().unwrap();

        assert!(second.path().join("second").exists());
        second.close().unwrap();
    }

    #[test]
    fn typst_fixture_output_path_preserves_relative_directories() {
        let root = Path::new("tests");
        let out = Path::new("out");

        let api = typst_fixture_output_path(root, out, Path::new("tests/api/test.typ")).unwrap();
        let context =
            typst_fixture_output_path(root, out, Path::new("tests/context/test.typ")).unwrap();

        assert_eq!(api, PathBuf::from("out/api/test.pdf"));
        assert_eq!(context, PathBuf::from("out/context/test.pdf"));
        assert_ne!(api, context);
    }

    #[test]
    fn collect_typst_fixtures_keeps_nested_outputs_distinct() {
        let root = unique_test_dir("typst-fixtures");
        let input_root = root.join("tests");
        let output_root = root.join("out");
        fs::create_dir_all(input_root.join("api")).unwrap();
        fs::create_dir_all(input_root.join("context")).unwrap();
        fs::write(input_root.join("api").join("test.typ"), "").unwrap();
        fs::write(input_root.join("context").join("test.typ"), "").unwrap();

        let mut fixtures = Vec::new();
        collect_typst_fixtures(&input_root, &output_root, &mut fixtures).unwrap();
        fixtures.sort_by(|left, right| left.output.cmp(&right.output));

        assert_eq!(fixtures.len(), 2);
        assert_eq!(fixtures[0].output, output_root.join("api").join("test.pdf"));
        assert_eq!(
            fixtures[1].output,
            output_root.join("context").join("test.pdf")
        );
        assert!(
            fixtures
                .iter()
                .all(|fixture| fixture.expected_error.is_none())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn collect_typst_fixtures_requires_and_loads_compile_fail_diagnostics() {
        let root = unique_test_dir("typst-compile-fail-fixtures");
        let input_root = root.join("tests");
        let output_root = root.join("out");
        let compile_fail = input_root.join("compile-fail");
        fs::create_dir_all(&compile_fail).unwrap();
        let input = compile_fail.join("invalid.typ");
        fs::write(&input, "#panic(\"expected\")").unwrap();

        let mut fixtures = Vec::new();
        let error = collect_typst_fixtures(&input_root, &output_root, &mut fixtures).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("requires readable diagnostic file")
        );

        fs::write(
            compile_fail.join("invalid.error.txt"),
            "expected diagnostic\n",
        )
        .unwrap();
        collect_typst_fixtures(&input_root, &output_root, &mut fixtures).unwrap();

        assert_eq!(fixtures.len(), 1);
        assert_eq!(fixtures[0].input, input);
        assert_eq!(
            fixtures[0].expected_error.as_deref(),
            Some("expected diagnostic")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn copy_dir_recursive_copies_source_modules() {
        let root = unique_test_dir("typst-copy-src");
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(source.join("src").join("nested")).unwrap();
        fs::write(source.join("src").join("exports.typ"), "#let ok = true").unwrap();
        fs::write(
            source.join("src").join("nested").join("module.typ"),
            "#let nested = true",
        )
        .unwrap();

        copy_dir_recursive(&source.join("src"), &destination.join("src")).unwrap();

        assert!(destination.join("src").join("exports.typ").exists());
        assert!(
            destination
                .join("src")
                .join("nested")
                .join("module.typ")
                .exists()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn copy_file_rejects_a_symbolic_link_source() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let outside = temporary.path().join("outside");
        let source = temporary.path().join("source");
        let destination = temporary.path().join("destination");
        fs::write(&outside, "outside").unwrap();
        symlink(&outside, &source).unwrap();

        let error = copy_file(&source, &destination).unwrap_err();

        assert!(error.to_string().contains("must be a regular file"));
        assert!(!destination.exists());
    }

    #[test]
    fn package_transaction_replaces_the_complete_version_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let parent = temporary.path().join("merman");
        let package = parent.join("0.2.0");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("obsolete.txt"), "obsolete").unwrap();
        let staging = tempfile::Builder::new()
            .prefix(".staging-")
            .tempdir_in(&parent)
            .unwrap();
        fs::write(staging.path().join("fresh.txt"), "fresh").unwrap();

        install_staged_package(&package, staging, |from, to| fs::rename(from, to)).unwrap();

        assert_eq!(fs::read(package.join("fresh.txt")).unwrap(), b"fresh");
        assert!(!package.join("obsolete.txt").exists());
    }

    #[test]
    fn package_transaction_restores_the_previous_version_on_install_failure() {
        let temporary = tempfile::tempdir().unwrap();
        let parent = temporary.path().join("merman");
        let package = parent.join("0.2.0");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("previous.txt"), "previous").unwrap();
        let staging = tempfile::Builder::new()
            .prefix(".staging-")
            .tempdir_in(&parent)
            .unwrap();
        fs::write(staging.path().join("fresh.txt"), "fresh").unwrap();
        let mut rename_count = 0;

        let error = install_staged_package(&package, staging, |from, to| {
            rename_count += 1;
            if rename_count == 2 {
                return Err(std::io::Error::other("injected install failure"));
            }
            fs::rename(from, to)
        })
        .unwrap_err();

        assert!(error.to_string().contains("previous package restored"));
        assert_eq!(fs::read(package.join("previous.txt")).unwrap(), b"previous");
        assert!(!package.join("fresh.txt").exists());
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
    }

    #[test]
    fn staged_runtime_package_excludes_source_examples_and_tests() {
        let temporary = tempfile::tempdir().unwrap();
        let fixture = package_fixture(&temporary);

        let package = stage_and_install_package(
            &fixture.source_snapshot,
            &fixture.out_dir,
            "publish",
            &fixture.artifact_wasm,
            &fixture.artifact_manifest,
        )
        .unwrap();

        assert!(package.join("src/plugin.typ").exists());
        assert!(package.join("merman_typst_plugin.wasm").exists());
        assert!(package.join("merman_typst_plugin.manifest.json").exists());
        assert!(package.join(PACKAGE_MANIFEST_FILE_NAME).exists());
        assert!(!package.join("examples").exists());
        assert!(!package.join("tests").exists());
        assert!(package.join("LICENSE").exists());
        assert!(package.join("THIRD_PARTY_NOTICES.md").exists());
        assert!(package.join("THIRD_PARTY_LICENSES").is_dir());
        let manifest: PackageManifest =
            serde_json::from_slice(&fs::read(package.join(PACKAGE_MANIFEST_FILE_NAME)).unwrap())
                .unwrap();
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.package_name, "merman");
        assert_eq!(manifest.package_version, "0.2.0");
        assert_eq!(manifest.profile, "publish");
        assert_eq!(manifest.wrapper.path, "lib.typ");
        assert_eq!(manifest.plugin.wasm.sha256, sha256_hex(b"wasm"));
        assert!(
            manifest
                .source
                .files
                .iter()
                .any(|file| file.path == "THIRD_PARTY_LICENSES/dependency.txt")
        );
        let bundle_entries = fs::read_dir(&package)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            bundle_entries,
            BTreeSet::from([
                "LICENSE".to_string(),
                "README.md".to_string(),
                "THIRD_PARTY_LICENSES".to_string(),
                "THIRD_PARTY_NOTICES.md".to_string(),
                "lib.typ".to_string(),
                PACKAGE_MANIFEST_FILE_NAME.to_string(),
                "merman_typst_plugin.manifest.json".to_string(),
                "merman_typst_plugin.wasm".to_string(),
                "src".to_string(),
                "typst.toml".to_string(),
            ])
        );
        assert!(fixture.out_dir.join("merman/.locks/0.2.0.lock").exists());
    }

    #[test]
    fn source_drift_after_snapshot_aborts_and_preserves_the_previous_package() {
        let temporary = tempfile::tempdir().unwrap();
        let fixture = package_fixture(&temporary);
        let package = fixture.out_dir.join("merman/0.2.0");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("previous.txt"), "previous").unwrap();

        let error = stage_and_install_package_with_hook(
            &fixture.source_snapshot,
            &fixture.out_dir,
            "publish",
            &fixture.artifact_wasm,
            &fixture.artifact_manifest,
            || {
                fs::write(
                    fixture.package_source.join("lib.typ"),
                    "#let render = () => panic(\"changed\")",
                )
                .unwrap();
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("changed after snapshot"));
        assert_eq!(fs::read(package.join("previous.txt")).unwrap(), b"previous");
        assert!(!package.join(PACKAGE_MANIFEST_FILE_NAME).exists());
    }

    #[test]
    fn staged_validation_rejects_extra_missing_and_tampered_files() {
        let temporary = tempfile::tempdir().unwrap();
        let fixture = package_fixture(&temporary);
        let plugin = PluginArtifactSnapshot::capture(
            &fixture.artifact_wasm,
            &fixture.artifact_manifest,
            "publish",
            "0.2.0",
        )
        .unwrap();
        let manifest = package_manifest(&fixture.source_snapshot, "publish", &plugin).unwrap();
        let manifest_bytes = serialize_package_manifest(&manifest).unwrap();
        let staged = temporary.path().join("staged");
        fs::create_dir_all(&staged).unwrap();
        for file in &fixture.source_snapshot.files {
            write_staged_file(&staged, &file.path, &file.bytes).unwrap();
        }
        write_staged_file(&staged, &plugin.wasm_file_name, &plugin.wasm_bytes).unwrap();
        write_staged_file(
            &staged,
            PACKAGE_ARTIFACT_MANIFEST_FILE_NAME,
            &plugin.artifact_manifest_bytes,
        )
        .unwrap();
        write_staged_file(&staged, PACKAGE_MANIFEST_FILE_NAME, &manifest_bytes).unwrap();
        validate_staged_package(
            &staged,
            &fixture.source_snapshot,
            &plugin,
            &manifest,
            &manifest_bytes,
        )
        .unwrap();

        fs::write(staged.join("extra.txt"), "extra").unwrap();
        let extra = validate_staged_package(
            &staged,
            &fixture.source_snapshot,
            &plugin,
            &manifest,
            &manifest_bytes,
        )
        .unwrap_err();
        assert!(extra.to_string().contains("shape does not match"));
        fs::remove_file(staged.join("extra.txt")).unwrap();

        fs::write(staged.join("lib.typ"), "tampered").unwrap();
        let tampered = validate_staged_package(
            &staged,
            &fixture.source_snapshot,
            &plugin,
            &manifest,
            &manifest_bytes,
        )
        .unwrap_err();
        assert!(tampered.to_string().contains("frozen input"));
        fs::write(
            staged.join("lib.typ"),
            &fixture
                .source_snapshot
                .required_file("lib.typ")
                .unwrap()
                .bytes,
        )
        .unwrap();

        fs::remove_file(staged.join("README.md")).unwrap();
        let missing = validate_staged_package(
            &staged,
            &fixture.source_snapshot,
            &plugin,
            &manifest,
            &manifest_bytes,
        )
        .unwrap_err();
        assert!(missing.to_string().contains("shape does not match"));
    }

    struct PackageFixture {
        package_source: PathBuf,
        out_dir: PathBuf,
        artifact_wasm: PathBuf,
        artifact_manifest: PathBuf,
        source_snapshot: RuntimePackageSourceSnapshot,
    }

    fn package_fixture(temporary: &tempfile::TempDir) -> PackageFixture {
        let workspace = temporary.path().join("workspace");
        let package_source = workspace.join("distribution/typst/merman");
        let out_dir = temporary.path().join("dist/typst");
        fs::create_dir_all(package_source.join("src")).unwrap();
        fs::create_dir_all(package_source.join("examples")).unwrap();
        fs::create_dir_all(package_source.join("tests/api")).unwrap();
        fs::write(
            package_source.join("typst.toml"),
            "[package]\nname = \"merman\"\nversion = \"0.2.0\"\nentrypoint = \"lib.typ\"\nexclude = [\"examples/**\", \"tests/**\"]\n",
        )
        .unwrap();
        fs::write(package_source.join("lib.typ"), "#let render = () => none").unwrap();
        fs::write(package_source.join("README.md"), "runtime package").unwrap();
        fs::write(package_source.join("src/plugin.typ"), "#let plugin = none").unwrap();
        fs::write(
            package_source.join("examples/basic.typ"),
            "#import \"../lib.typ\"",
        )
        .unwrap();
        fs::write(package_source.join("tests/api/test.typ"), "#test").unwrap();
        fs::write(package_source.join("LICENSE"), "project license").unwrap();
        fs::write(
            package_source.join("THIRD_PARTY_NOTICES.md"),
            "third-party notices",
        )
        .unwrap();
        fs::create_dir_all(package_source.join("THIRD_PARTY_LICENSES")).unwrap();
        fs::write(
            package_source.join("THIRD_PARTY_LICENSES/dependency.txt"),
            "dependency license",
        )
        .unwrap();
        let artifact_wasm = temporary.path().join("artifact/merman_typst_plugin.wasm");
        let artifact_manifest = temporary.path().join("artifact/manifest.json");
        fs::create_dir_all(artifact_wasm.parent().unwrap()).unwrap();
        fs::write(&artifact_wasm, "wasm").unwrap();
        let artifact_manifest_value = serde_json::json!({
            "profile": "publish",
            "cargo_package_version": "0.8.0-alpha.3",
            "typst_package_version": "0.2.0",
            "artifact": {
                "file": "merman_typst_plugin.wasm",
                "sha256": sha256_hex(b"wasm"),
                "bytes": 4
            }
        });
        fs::write(
            &artifact_manifest,
            serde_json::to_vec_pretty(&artifact_manifest_value).unwrap(),
        )
        .unwrap();
        let source_snapshot =
            RuntimePackageSourceSnapshot::capture(&package_source, "merman_typst_plugin.wasm")
                .unwrap();
        PackageFixture {
            package_source,
            out_dir,
            artifact_wasm,
            artifact_manifest,
            source_snapshot,
        }
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("merman-{name}-{pid}-{nanos}"))
    }
}
