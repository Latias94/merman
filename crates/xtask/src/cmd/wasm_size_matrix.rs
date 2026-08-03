use crate::{
    XtaskError,
    cmd::{
        artifact_profiles::load_wasm_size_artifact_profiles,
        paths,
        typst_artifact::{optimize_typst_wasm, verify_typst_wasm_optimizer},
        wasm_build_lock::WorkspaceWasmBuildLock,
    },
};
use flate2::{Compression, write::GzEncoder};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const WASM_SIZE_BUDGET_SCHEMA_VERSION: u32 = 2;
const WEB_PACKAGE_PROVENANCE: &str = "provenance.json";
const WEB_PACKAGE_WASM: &str = "wasm/merman_wasm_bg.wasm";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    Web,
    Typst,
}

impl Surface {
    fn parse(raw: &str) -> Result<Self, XtaskError> {
        match raw {
            "web" => Ok(Self::Web),
            "typst" => Ok(Self::Typst),
            _ => Err(XtaskError::Usage),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Typst => "typst",
        }
    }
}

#[derive(Debug, Default)]
struct Options {
    surface: Option<Surface>,
    artifact_profile: Option<String>,
    no_strip: bool,
    budget_file: Option<PathBuf>,
    web_package_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct WasmArtifact {
    id: String,
    surface: Surface,
    package: String,
    manifest_path: PathBuf,
    artifact_name: String,
    cargo_profile: String,
    target_triple: String,
    default_features: bool,
    features: Vec<String>,
    capabilities: Vec<String>,
    runtime_ids: Vec<String>,
    outputs: Vec<String>,
}

#[derive(Debug)]
struct WasmMeasurement {
    raw_bytes: u64,
    stripped_bytes: Option<u64>,
    gzip_bytes: u64,
    brotli_bytes: u64,
    artifact_path: PathBuf,
    compressed_source: CompressionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionSource {
    Raw,
    Stripped,
}

impl CompressionSource {
    const fn label(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Stripped => "stripped",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WasmSizeBudgets {
    schema_version: u32,
    updated: String,
    compression_source: String,
    notes: String,
    artifact_profiles: BTreeMap<String, WasmArtifactBudget>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WasmArtifactBudget {
    max_raw_bytes: u64,
    max_stripped_bytes: u64,
    max_gzip_bytes: u64,
    max_brotli_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct WebPackageProvenance {
    schema_version: u32,
    artifact_profile: String,
    runtime_capability_ids: Vec<String>,
    outputs: Vec<String>,
    wasm: WebPackageWasmProvenance,
}

#[derive(Debug, Deserialize)]
struct WebPackageWasmProvenance {
    path: String,
}

fn all_artifacts() -> Result<Vec<WasmArtifact>, XtaskError> {
    let artifacts = load_wasm_size_artifact_profiles()
        .map_err(|error| {
            XtaskError::WasmSizeMatrixFailed(format!(
                "failed to load validated WASM artifact profiles: {error}"
            ))
        })?
        .into_iter()
        .map(|profile| {
            let surface = match profile.semantic_target.as_str() {
                "web" => Surface::Web,
                "typst" => Surface::Typst,
                target => {
                    return Err(XtaskError::WasmSizeMatrixFailed(format!(
                        "artifact profile `{}` has unsupported WASM semantic target `{target}`",
                        profile.id
                    )));
                }
            };
            Ok(WasmArtifact {
                id: profile.id,
                surface,
                package: profile.package,
                manifest_path: profile.manifest_path,
                artifact_name: profile.artifact_name,
                cargo_profile: profile.cargo_profile,
                target_triple: profile.target_triple,
                default_features: profile.default_features,
                features: profile.features,
                capabilities: profile.capabilities,
                runtime_ids: profile.runtime_ids,
                outputs: profile.outputs,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut names = BTreeSet::new();
    if let Some(duplicate) = artifacts
        .iter()
        .map(|artifact| artifact.id.as_str())
        .find(|name| !names.insert(*name))
    {
        return Err(XtaskError::WasmSizeMatrixFailed(format!(
            "duplicate WASM artifact profile `{duplicate}`"
        )));
    }

    Ok(artifacts)
}

pub(crate) fn wasm_size_matrix(args: Vec<String>) -> Result<(), XtaskError> {
    let all_artifacts = all_artifacts()?;
    let options = parse_options(args, &all_artifacts)?;
    let artifacts = selected_artifacts(&options, &all_artifacts)?;
    validate_measurement_source(&options, &artifacts)?;
    let budgets = options
        .budget_file
        .as_deref()
        .map(|path| load_budget_file(path, &all_artifacts))
        .transpose()?;
    let package_artifacts = options
        .web_package_root
        .as_deref()
        .map(|root| load_web_package_artifacts(root, &artifacts))
        .transpose()?;
    let _build_lock = if package_artifacts.is_none() {
        Some(WorkspaceWasmBuildLock::acquire()?)
    } else {
        None
    };
    let strip_dir = tempfile::Builder::new()
        .prefix("merman-wasm-size-")
        .tempdir()
        .map_err(|source| XtaskError::WriteFile {
            path: "temporary WASM size directory".to_string(),
            source,
        })?;

    println!(
        "wasm-size-matrix columns=surface,artifact_profile,package,manifest,cargo_profile,target,default_features,features,capabilities,runtime_ids,outputs,raw_bytes,stripped_bytes,gzip_bytes,brotli_bytes,compressed_source,artifact"
    );

    let mut budget_failures = Vec::new();

    for artifact in artifacts {
        let package_path = package_artifacts
            .as_ref()
            .and_then(|paths| paths.get(&artifact.id));
        let measurement = measure_artifact(
            artifact,
            strip_dir.path(),
            options.no_strip,
            package_path.map(PathBuf::as_path),
        )?;
        let display_artifact = measurement
            .artifact_path
            .canonicalize()
            .unwrap_or_else(|_| measurement.artifact_path.clone());

        println!(
            "wasm-size-matrix surface={} artifact_profile={} package={} manifest={} cargo_profile={} target={} default_features={} features={} capabilities={} runtime_ids={} outputs={} raw_bytes={} stripped_bytes={} gzip_bytes={} brotli_bytes={} compressed_source={} artifact={}",
            artifact.surface.label(),
            artifact.id,
            artifact.package,
            artifact.manifest_path.display(),
            artifact.cargo_profile,
            artifact.target_triple,
            artifact.default_features,
            value_label(&artifact.features),
            value_label(&artifact.capabilities),
            value_label(&artifact.runtime_ids),
            value_label(&artifact.outputs),
            measurement.raw_bytes,
            measurement
                .stripped_bytes
                .map(|bytes| bytes.to_string())
                .unwrap_or_else(|| "skipped".to_string()),
            measurement.gzip_bytes,
            measurement.brotli_bytes,
            measurement.compressed_source.label(),
            display_artifact.display()
        );

        if let Some(budgets) = budgets.as_ref() {
            budget_failures.extend(check_budget(artifact, &measurement, budgets));
        }
    }

    if budget_failures.is_empty() {
        if let Some(path) = options.budget_file.as_deref() {
            println!(
                "wasm-size-matrix budget_file={} result=ok",
                resolve_repo_path(path).display()
            );
        }
        Ok(())
    } else {
        Err(XtaskError::WasmSizeMatrixFailed(budget_failures.join("\n")))
    }
}

fn parse_options(args: Vec<String>, artifacts: &[WasmArtifact]) -> Result<Options, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage(artifacts);
        return Err(XtaskError::Usage);
    }

    let mut options = Options::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--surface" => {
                let raw = iter.next().ok_or(XtaskError::Usage)?;
                options.surface = Some(Surface::parse(&raw)?);
            }
            "--artifact-profile" => {
                options.artifact_profile = Some(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--no-strip" => {
                options.no_strip = true;
            }
            "--budget-file" => {
                options.budget_file = Some(PathBuf::from(iter.next().ok_or(XtaskError::Usage)?));
            }
            "--web-package-root" => {
                options.web_package_root =
                    Some(PathBuf::from(iter.next().ok_or(XtaskError::Usage)?));
            }
            _ => {
                print_usage(artifacts);
                return Err(XtaskError::Usage);
            }
        }
    }

    if options.no_strip && options.budget_file.is_some() {
        return Err(XtaskError::WasmSizeMatrixFailed(
            "--no-strip cannot be combined with --budget-file because committed budgets use stripped artifacts"
                .to_string(),
        ));
    }

    if options.surface.is_none() && options.artifact_profile.is_none() {
        return Err(XtaskError::WasmSizeMatrixFailed(
            "select --surface or --artifact-profile because Web packages and Typst plugins use different artifact producers"
                .to_string(),
        ));
    }

    Ok(options)
}

fn print_usage(artifacts: &[WasmArtifact]) {
    println!(
        "usage: xtask wasm-size-matrix (--surface web|typst | --artifact-profile <id>) [--web-package-root <path>] [--no-strip] [--budget-file <path>]"
    );
    println!("  --web-package-root  measure assembled Web package artifacts instead of rebuilding");
    println!();
    println!("Artifact profiles:");
    for artifact in artifacts {
        println!(
            "  {:<18} surface={} package={} manifest={} cargo_profile={} target={} default_features={} features={} capabilities={} outputs={}",
            artifact.id,
            artifact.surface.label(),
            artifact.package,
            artifact.manifest_path.display(),
            artifact.cargo_profile,
            artifact.target_triple,
            artifact.default_features,
            value_label(&artifact.features),
            value_label(&artifact.capabilities),
            value_label(&artifact.outputs)
        );
    }
}

fn measure_artifact(
    artifact: &WasmArtifact,
    strip_dir: &Path,
    no_strip: bool,
    package_path: Option<&Path>,
) -> Result<WasmMeasurement, XtaskError> {
    let artifact_path = if let Some(path) = package_path {
        path.to_path_buf()
    } else {
        build_artifact(artifact)?;
        artifact_path(artifact)
    };
    let raw_bytes = file_size(&artifact_path)?;

    let (compressed_path, stripped_bytes, compressed_source) = if no_strip {
        (artifact_path.clone(), None, CompressionSource::Raw)
    } else {
        let stripped_path = strip_copy(artifact, &artifact_path, strip_dir)?;
        let bytes = file_size(&stripped_path)?;
        (stripped_path, Some(bytes), CompressionSource::Stripped)
    };

    let gzip_bytes = gzip_size(&compressed_path)?;
    let brotli_bytes = brotli_size(&compressed_path)?;

    Ok(WasmMeasurement {
        raw_bytes,
        stripped_bytes,
        gzip_bytes,
        brotli_bytes,
        artifact_path,
        compressed_source,
    })
}

fn load_web_package_artifacts(
    root: &Path,
    artifacts: &[&WasmArtifact],
) -> Result<BTreeMap<String, PathBuf>, XtaskError> {
    if artifacts
        .iter()
        .any(|artifact| artifact.surface != Surface::Web)
    {
        return Err(XtaskError::WasmSizeMatrixFailed(
            "--web-package-root only accepts Web package artifacts; select --surface web or a Web artifact profile"
                .to_string(),
        ));
    }

    let root = resolve_repo_path(root);
    let entries = fs::read_dir(&root).map_err(|source| XtaskError::ReadFile {
        path: root.display().to_string(),
        source,
    })?;
    let mut discovered = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: root.display().to_string(),
            source,
        })?;
        let package_root = entry.path();
        if !package_root.is_dir() {
            continue;
        }
        let artifact_root = package_root.join("artifacts");
        let provenance_path = artifact_root.join(WEB_PACKAGE_PROVENANCE);
        if !provenance_path.is_file() {
            continue;
        }
        let provenance = serde_json::from_str::<WebPackageProvenance>(&crate::util::read_text(
            &provenance_path,
        )?)
        .map_err(XtaskError::Json)?;
        if provenance.schema_version != 2 {
            return Err(XtaskError::WasmSizeMatrixFailed(format!(
                "{} must use Web package provenance schema 2, found {}",
                provenance_path.display(),
                provenance.schema_version
            )));
        }
        if provenance.wasm.path != WEB_PACKAGE_WASM {
            return Err(XtaskError::WasmSizeMatrixFailed(format!(
                "{} must reference {}, found {}",
                provenance_path.display(),
                WEB_PACKAGE_WASM,
                provenance.wasm.path
            )));
        }
        let wasm_path = artifact_root.join(WEB_PACKAGE_WASM);
        if !wasm_path.is_file() {
            return Err(XtaskError::WasmSizeMatrixFailed(format!(
                "Web package artifact profile {} is missing {}",
                provenance.artifact_profile,
                wasm_path.display()
            )));
        }
        let profile_id = provenance.artifact_profile.clone();
        if discovered
            .insert(profile_id.clone(), (provenance, wasm_path))
            .is_some()
        {
            return Err(XtaskError::WasmSizeMatrixFailed(format!(
                "duplicate Web package artifact profile {} under {}",
                profile_id,
                root.display()
            )));
        }
    }

    let mut selected = BTreeMap::new();
    for artifact in artifacts {
        let (provenance, path) = discovered.get(&artifact.id).ok_or_else(|| {
            XtaskError::WasmSizeMatrixFailed(format!(
                "Web package artifact root {} is missing artifact profile {}",
                root.display(),
                artifact.id
            ))
        })?;
        if provenance.runtime_capability_ids != artifact.runtime_ids {
            return Err(XtaskError::WasmSizeMatrixFailed(format!(
                "Web package artifact profile {} runtime capability IDs drifted: expected {}, found {}",
                artifact.id,
                value_label(&artifact.runtime_ids),
                value_label(&provenance.runtime_capability_ids)
            )));
        }
        if provenance.outputs != artifact.outputs {
            return Err(XtaskError::WasmSizeMatrixFailed(format!(
                "Web package artifact profile {} output IDs drifted: expected {}, found {}",
                artifact.id,
                value_label(&artifact.outputs),
                value_label(&provenance.outputs)
            )));
        }
        selected.insert(artifact.id.clone(), path.clone());
    }
    Ok(selected)
}

fn load_budget_file(
    path: &Path,
    artifacts: &[WasmArtifact],
) -> Result<WasmSizeBudgets, XtaskError> {
    let path = resolve_repo_path(path);
    let text = crate::util::read_text(&path)?;
    let budgets = serde_json::from_str(&text).map_err(XtaskError::Json)?;
    validate_budget_coverage(&budgets, artifacts)?;
    Ok(budgets)
}

fn validate_budget_coverage(
    budgets: &WasmSizeBudgets,
    artifacts: &[WasmArtifact],
) -> Result<(), XtaskError> {
    if budgets.schema_version != WASM_SIZE_BUDGET_SCHEMA_VERSION {
        return Err(XtaskError::WasmSizeMatrixFailed(format!(
            "WASM size budget schema must be {WASM_SIZE_BUDGET_SCHEMA_VERSION}, found {}",
            budgets.schema_version
        )));
    }
    if budgets.updated.trim().is_empty() || budgets.notes.trim().is_empty() {
        return Err(XtaskError::WasmSizeMatrixFailed(
            "WASM size budget metadata must include non-empty updated and notes fields".to_string(),
        ));
    }
    if budgets.compression_source != "stripped" {
        return Err(XtaskError::WasmSizeMatrixFailed(format!(
            "WASM size budget compression_source must be `stripped`, found `{}`",
            budgets.compression_source
        )));
    }

    let expected = artifacts
        .iter()
        .map(|artifact| artifact.id.clone())
        .collect::<BTreeSet<_>>();
    let actual = budgets
        .artifact_profiles
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected != actual {
        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        let stale = actual.difference(&expected).cloned().collect::<Vec<_>>();
        return Err(XtaskError::WasmSizeMatrixFailed(format!(
            "WASM size budgets must exactly cover artifact profiles; missing=[{}] stale=[{}]",
            missing.join(","),
            stale.join(",")
        )));
    }
    for (id, budget) in &budgets.artifact_profiles {
        if [
            budget.max_raw_bytes,
            budget.max_stripped_bytes,
            budget.max_gzip_bytes,
            budget.max_brotli_bytes,
        ]
        .contains(&0)
        {
            return Err(XtaskError::WasmSizeMatrixFailed(format!(
                "WASM size budget for artifact profile {id} must define positive limits for every metric"
            )));
        }
    }
    Ok(())
}

fn resolve_repo_path(path: &Path) -> PathBuf {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        paths::workspace_root().join(path)
    };
    resolved.canonicalize().unwrap_or(resolved)
}

fn check_budget(
    artifact: &WasmArtifact,
    measurement: &WasmMeasurement,
    budgets: &WasmSizeBudgets,
) -> Vec<String> {
    let Some(budget) = budgets.artifact_profiles.get(&artifact.id) else {
        return vec![format!(
            "missing wasm size budget for artifact profile {}",
            artifact.id
        )];
    };

    let mut failures = Vec::new();
    check_metric(
        &mut failures,
        &artifact.id,
        "raw_bytes",
        measurement.raw_bytes,
        budget.max_raw_bytes,
    );
    if let Some(stripped_bytes) = measurement.stripped_bytes {
        check_metric(
            &mut failures,
            &artifact.id,
            "stripped_bytes",
            stripped_bytes,
            budget.max_stripped_bytes,
        );
    } else {
        failures.push(format!(
            "artifact profile {} skipped stripped_bytes but budget requires max_stripped_bytes={}",
            artifact.id, budget.max_stripped_bytes
        ));
    }
    check_metric(
        &mut failures,
        &artifact.id,
        "gzip_bytes",
        measurement.gzip_bytes,
        budget.max_gzip_bytes,
    );
    check_metric(
        &mut failures,
        &artifact.id,
        "brotli_bytes",
        measurement.brotli_bytes,
        budget.max_brotli_bytes,
    );

    failures
}

fn check_metric(
    failures: &mut Vec<String>,
    artifact_profile: &str,
    metric: &str,
    actual: u64,
    max: u64,
) {
    if actual > max {
        failures.push(format!(
            "artifact profile {artifact_profile} exceeds {metric}: actual={actual} max={max}"
        ));
    }
}

fn selected_artifacts<'a>(
    options: &Options,
    all_artifacts: &'a [WasmArtifact],
) -> Result<Vec<&'a WasmArtifact>, XtaskError> {
    let artifacts = all_artifacts
        .iter()
        .filter(|artifact| {
            options
                .surface
                .is_none_or(|surface| artifact.surface == surface)
        })
        .filter(|artifact| {
            options
                .artifact_profile
                .as_deref()
                .is_none_or(|id| artifact.id == id)
        })
        .collect::<Vec<_>>();

    if artifacts.is_empty() {
        return Err(XtaskError::WasmSizeMatrixFailed(
            "no WASM artifact profiles matched the requested filters".to_string(),
        ));
    }

    Ok(artifacts)
}

fn validate_measurement_source(
    options: &Options,
    artifacts: &[&WasmArtifact],
) -> Result<(), XtaskError> {
    if options.budget_file.is_some()
        && options.web_package_root.is_none()
        && artifacts
            .iter()
            .any(|artifact| artifact.surface == Surface::Web)
    {
        return Err(XtaskError::WasmSizeMatrixFailed(
            "budgeted Web measurements require --web-package-root because the committed budgets describe assembled npm package artifacts"
                .to_string(),
        ));
    }

    Ok(())
}

fn build_artifact(artifact: &WasmArtifact) -> Result<(), XtaskError> {
    let mut command = Command::new("cargo");
    command.args([
        "build",
        "-p",
        &artifact.package,
        "--profile",
        &artifact.cargo_profile,
        "--target",
        &artifact.target_triple,
    ]);
    command
        .arg("--manifest-path")
        .arg(&artifact.manifest_path)
        .args(["--locked", "--target-dir"]);
    command.arg(paths::wasm_build_target_root());

    if !artifact.default_features {
        command.arg("--no-default-features");
    }

    let features = artifact.features.join(",");
    if !features.is_empty() {
        command.arg("--features").arg(&features);
    }

    let status = command
        .current_dir(paths::workspace_root())
        .status()
        .map_err(|source| XtaskError::ReadFile {
            path: "cargo".to_string(),
            source,
        })?;

    if !status.success() {
        return Err(XtaskError::WasmSizeMatrixFailed(format!(
            "cargo build failed for artifact profile {} with status {status}",
            artifact.id
        )));
    }

    Ok(())
}

fn artifact_path(artifact: &WasmArtifact) -> PathBuf {
    paths::wasm_build_target_root()
        .join(&artifact.target_triple)
        .join(&artifact.cargo_profile)
        .join(&artifact.artifact_name)
}

fn strip_copy(
    artifact: &WasmArtifact,
    wasm_path: &Path,
    strip_dir: &Path,
) -> Result<PathBuf, XtaskError> {
    let optimized_path = if artifact.surface == Surface::Typst {
        verify_typst_wasm_optimizer()?;
        let path = strip_dir.join(format!("{}.optimized.wasm", artifact.id));
        optimize_typst_wasm(wasm_path, &path)?;
        Some(path)
    } else {
        None
    };
    let strip_input = optimized_path.as_deref().unwrap_or(wasm_path);
    let stripped_path = strip_dir.join(format!("{}.stripped.wasm", artifact.id));
    let status = Command::new("wasm-tools")
        .args(["strip", "--all"])
        .arg(strip_input)
        .arg("-o")
        .arg(&stripped_path)
        .current_dir(paths::workspace_root())
        .status()
        .map_err(|source| XtaskError::ReadFile {
            path: "wasm-tools".to_string(),
            source,
        })?;

    if !status.success() {
        return Err(XtaskError::WasmSizeMatrixFailed(format!(
            "wasm-tools strip failed for artifact profile {} with status {status}",
            artifact.id
        )));
    }

    Ok(stripped_path)
}

fn file_size(path: &Path) -> Result<u64, XtaskError> {
    fs::metadata(path)
        .map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })
        .map(|metadata| metadata.len())
}

fn gzip_size(path: &Path) -> Result<u64, XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&bytes)
        .map_err(|source| XtaskError::CompressFile {
            path: path.display().to_string(),
            source,
        })?;
    let compressed = encoder
        .finish()
        .map_err(|source| XtaskError::CompressFile {
            path: path.display().to_string(),
            source,
        })?;
    Ok(compressed.len() as u64)
}

fn brotli_size(path: &Path) -> Result<u64, XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let mut compressed = Vec::new();
    let mut reader = brotli::CompressorReader::new(&bytes[..], 4096, 11, 22);
    reader
        .read_to_end(&mut compressed)
        .map_err(|source| XtaskError::CompressFile {
            path: path.display().to_string(),
            source,
        })?;
    Ok(compressed.len() as u64)
}

fn value_label(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn artifacts() -> &'static Vec<WasmArtifact> {
        static ARTIFACTS: OnceLock<Vec<WasmArtifact>> = OnceLock::new();
        ARTIFACTS.get_or_init(|| all_artifacts().unwrap())
    }

    #[test]
    fn selection_requires_an_explicit_surface_or_artifact_profile() {
        let error = parse_options(Vec::new(), artifacts()).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("select --surface or --artifact-profile"),
            "{error}"
        );
    }

    #[test]
    fn surface_filter_selects_only_that_surface() {
        let all_artifacts = artifacts();
        let options = Options {
            surface: Some(Surface::Typst),
            ..Options::default()
        };
        let selected = selected_artifacts(&options, all_artifacts).unwrap();

        assert!(!selected.is_empty());
        assert!(
            selected
                .iter()
                .all(|artifact| artifact.surface == Surface::Typst)
        );
    }

    #[test]
    fn artifact_profile_filter_selects_one_exact_recipe() {
        let all_artifacts = artifacts();
        let options = Options {
            artifact_profile: Some("web-render".to_string()),
            ..Options::default()
        };
        let selected = selected_artifacts(&options, all_artifacts).unwrap();

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "web-render");
        assert_eq!(
            selected[0].manifest_path,
            Path::new("crates/merman-wasm/Cargo.toml")
        );
        assert_eq!(
            string_values(&selected[0].features),
            vec!["layout-cytoscape", "layout-elk", "math", "svg"]
        );
        assert!(!selected[0].default_features);
    }

    #[test]
    fn budgeted_web_measurements_require_assembled_package_artifacts() {
        let all_artifacts = artifacts();
        let options = Options {
            surface: Some(Surface::Web),
            budget_file: Some(PathBuf::from("docs/release/WASM_SIZE_BUDGETS.json")),
            ..Options::default()
        };
        let selected = selected_artifacts(&options, all_artifacts).unwrap();

        let error = validate_measurement_source(&options, &selected).unwrap_err();

        assert!(error.to_string().contains("require --web-package-root"));
    }

    #[test]
    fn web_editor_recipe_reports_its_full_callable_surface() {
        let artifact = artifacts()
            .iter()
            .find(|artifact| artifact.id == "web-editor")
            .unwrap();

        assert_eq!(artifact.surface, Surface::Web);
        assert_eq!(string_values(&artifact.features), vec!["editor"]);
        assert_eq!(
            string_values(&artifact.capabilities),
            vec!["analysis", "editor"]
        );
        assert!(artifact.outputs.is_empty());
        assert!(!artifact.default_features);
    }

    #[test]
    fn web_renderer_recipes_match_their_product_boundaries() {
        let web = artifacts()
            .iter()
            .filter(|artifact| artifact.surface == Surface::Web)
            .collect::<Vec<_>>();
        assert_eq!(
            web.iter()
                .map(|artifact| artifact.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "web-analysis",
                "web-ascii",
                "web-editor",
                "web-full",
                "web-render"
            ]
        );

        let full = web
            .iter()
            .find(|artifact| artifact.id == "web-full")
            .unwrap();
        assert_eq!(
            string_values(&full.features),
            vec![
                "analysis",
                "ascii",
                "editor",
                "layout-cytoscape",
                "layout-elk",
                "math",
                "svg"
            ]
        );
        assert_eq!(
            string_values(&full.capabilities),
            string_values(&full.features)
        );
        assert_eq!(
            string_values(&full.runtime_ids),
            string_values(&full.features)
        );
        assert_eq!(string_values(&full.outputs), vec!["ascii", "svg"]);

        let render = web
            .iter()
            .find(|artifact| artifact.id == "web-render")
            .unwrap();
        let complete_svg = vec!["layout-cytoscape", "layout-elk", "math", "svg"];
        assert_eq!(string_values(&render.features), complete_svg);
        assert_eq!(string_values(&render.capabilities), complete_svg);
        assert_eq!(string_values(&render.runtime_ids), complete_svg);
        assert_eq!(string_values(&render.outputs), vec!["svg"]);
    }

    #[test]
    fn typst_recipe_matches_the_publishable_math_free_surface() {
        let artifact = artifacts()
            .iter()
            .find(|artifact| artifact.id == "typst-wasm")
            .unwrap();

        assert_eq!(artifact.surface, Surface::Typst);
        assert_eq!(
            string_values(&artifact.features),
            vec!["analysis", "layout-cytoscape", "layout-elk", "svg"]
        );
        assert!(
            !artifact
                .capabilities
                .iter()
                .any(|capability| capability == "math")
        );
        assert!(!artifact.default_features);
    }

    #[test]
    fn unmatched_filters_are_errors() {
        let all_artifacts = artifacts();
        let options = Options {
            surface: Some(Surface::Typst),
            artifact_profile: Some("web-render".to_string()),
            ..Options::default()
        };

        assert!(selected_artifacts(&options, all_artifacts).is_err());
    }

    #[test]
    fn option_parser_accepts_surface_artifact_profile_and_budget() {
        let all_artifacts = artifacts();
        let options = parse_options(
            vec![
                "--surface".to_string(),
                "web".to_string(),
                "--artifact-profile".to_string(),
                "web-analysis".to_string(),
                "--budget-file".to_string(),
                "docs/release/WASM_SIZE_BUDGETS.json".to_string(),
                "--web-package-root".to_string(),
                "platforms/web/packages".to_string(),
            ],
            all_artifacts,
        )
        .unwrap();

        assert_eq!(options.surface, Some(Surface::Web));
        assert_eq!(options.artifact_profile.as_deref(), Some("web-analysis"));
        assert!(!options.no_strip);
        assert_eq!(
            options.budget_file.as_deref(),
            Some(Path::new("docs/release/WASM_SIZE_BUDGETS.json"))
        );
        assert_eq!(
            options.web_package_root.as_deref(),
            Some(Path::new("platforms/web/packages"))
        );
    }

    #[test]
    fn web_package_artifacts_are_selected_by_final_package_provenance() {
        let temporary = tempfile::tempdir().unwrap();
        let all_artifacts = artifacts();
        let analysis = all_artifacts
            .iter()
            .find(|artifact| artifact.id == "web-analysis")
            .unwrap();
        let render = all_artifacts
            .iter()
            .find(|artifact| artifact.id == "web-render")
            .unwrap();
        write_web_package_artifact(temporary.path(), "slim", analysis);
        write_web_package_artifact(temporary.path(), "viewer", render);
        let selected = vec![analysis, render];

        let paths = load_web_package_artifacts(temporary.path(), &selected).unwrap();
        let root = temporary.path().canonicalize().unwrap();

        assert_eq!(
            paths["web-analysis"],
            root.join("slim").join("artifacts").join(WEB_PACKAGE_WASM)
        );
        assert_eq!(
            paths["web-render"],
            root.join("viewer").join("artifacts").join(WEB_PACKAGE_WASM)
        );
    }

    #[test]
    fn web_package_artifacts_reject_non_web_profiles() {
        let typst = artifacts()
            .iter()
            .find(|artifact| artifact.id == "typst-wasm")
            .unwrap();
        let error =
            load_web_package_artifacts(Path::new("platforms/web/packages"), &[typst]).unwrap_err();

        assert!(error.to_string().contains("only accepts Web"), "{error}");
    }

    #[test]
    fn web_package_artifacts_reject_runtime_identity_drift() {
        let temporary = tempfile::tempdir().unwrap();
        let all_artifacts = artifacts();
        let analysis = all_artifacts
            .iter()
            .find(|artifact| artifact.id == "web-analysis")
            .unwrap();
        write_web_package_artifact(temporary.path(), "slim", analysis);
        let provenance_path = temporary
            .path()
            .join("slim/artifacts")
            .join(WEB_PACKAGE_PROVENANCE);
        let mut provenance: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&provenance_path).unwrap()).unwrap();
        provenance["runtime_capability_ids"] = serde_json::json!(["svg"]);
        fs::write(provenance_path, serde_json::to_string(&provenance).unwrap()).unwrap();

        let error = load_web_package_artifacts(temporary.path(), &[analysis]).unwrap_err();

        assert!(
            error.to_string().contains("runtime capability IDs drifted"),
            "{error}"
        );
    }

    #[test]
    fn option_parser_rejects_budget_checks_without_stripping() {
        let error = parse_options(
            vec![
                "--no-strip".to_string(),
                "--budget-file".to_string(),
                "docs/release/WASM_SIZE_BUDGETS.json".to_string(),
            ],
            artifacts(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("cannot be combined"), "{error}");
    }

    #[test]
    fn option_parser_rejects_retired_surface_aliases() {
        let all_artifacts = artifacts();
        for retired in ["browser", "wasm"] {
            assert!(
                parse_options(
                    vec!["--surface".to_string(), retired.to_string()],
                    all_artifacts
                )
                .is_err(),
                "retired surface alias `{retired}` must not remain accepted"
            );
        }
    }

    #[test]
    fn budget_check_reports_missing_artifact_profile_budget() {
        let artifact = artifacts()
            .iter()
            .find(|artifact| artifact.id == "web-analysis")
            .unwrap();
        let budgets = WasmSizeBudgets {
            schema_version: WASM_SIZE_BUDGET_SCHEMA_VERSION,
            updated: "2026-07-23".to_string(),
            compression_source: "stripped".to_string(),
            notes: "test".to_string(),
            artifact_profiles: BTreeMap::new(),
        };
        let measurement = measurement_for_test();

        let failures = check_budget(artifact, &measurement, &budgets);

        assert_eq!(
            failures,
            vec!["missing wasm size budget for artifact profile web-analysis"]
        );
    }

    #[test]
    fn budget_check_reports_only_exceeded_metrics() {
        let artifact = artifacts()
            .iter()
            .find(|artifact| artifact.id == "web-analysis")
            .unwrap();
        let mut budgets = WasmSizeBudgets {
            schema_version: WASM_SIZE_BUDGET_SCHEMA_VERSION,
            updated: "2026-07-23".to_string(),
            compression_source: "stripped".to_string(),
            notes: "test".to_string(),
            artifact_profiles: BTreeMap::new(),
        };
        budgets.artifact_profiles.insert(
            "web-analysis".to_string(),
            WasmArtifactBudget {
                max_raw_bytes: 9,
                max_stripped_bytes: 7,
                max_gzip_bytes: 5,
                max_brotli_bytes: 3,
            },
        );
        let measurement = measurement_for_test();

        let failures = check_budget(artifact, &measurement, &budgets);

        assert_eq!(
            failures,
            vec![
                "artifact profile web-analysis exceeds raw_bytes: actual=10 max=9",
                "artifact profile web-analysis exceeds brotli_bytes: actual=4 max=3",
            ]
        );
    }

    #[test]
    fn committed_budget_exactly_covers_every_wasm_artifact_profile() {
        load_budget_file(
            Path::new("docs/release/WASM_SIZE_BUDGETS.json"),
            artifacts(),
        )
        .unwrap();
    }

    #[test]
    fn budget_coverage_rejects_missing_and_stale_artifact_profiles() {
        let mut budgets = load_budget_file(
            Path::new("docs/release/WASM_SIZE_BUDGETS.json"),
            artifacts(),
        )
        .unwrap();
        budgets.artifact_profiles.remove("web-full");
        budgets.artifact_profiles.insert(
            "web-math".to_string(),
            WasmArtifactBudget {
                max_raw_bytes: 1,
                max_stripped_bytes: 1,
                max_gzip_bytes: 1,
                max_brotli_bytes: 1,
            },
        );

        let error = validate_budget_coverage(&budgets, artifacts()).unwrap_err();
        assert!(error.to_string().contains("missing=[web-full]"), "{error}");
        assert!(error.to_string().contains("stale=[web-math]"), "{error}");
    }

    #[test]
    fn budget_schema_rejects_legacy_preset_keys() {
        let error = serde_json::from_str::<WasmSizeBudgets>(
            r#"{
              "schema_version": 1,
              "updated": "2026-07-22",
              "compression_source": "stripped",
              "notes": "legacy",
              "presets": {}
            }"#,
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("unknown field `presets`"),
            "{error}"
        );
    }

    #[test]
    fn value_label_uses_none_for_empty_values() {
        assert_eq!(value_label(&Vec::new()), "none");
        assert_eq!(
            value_label(&["svg".to_string(), "analysis".to_string()]),
            "svg+analysis"
        );
    }

    fn string_values(values: &[String]) -> Vec<&str> {
        values.iter().map(String::as_str).collect()
    }

    fn write_web_package_artifact(root: &Path, directory: &str, artifact: &WasmArtifact) {
        let artifact_root = root.join(directory).join("artifacts");
        fs::create_dir_all(artifact_root.join("wasm")).unwrap();
        fs::write(
            artifact_root.join(WEB_PACKAGE_PROVENANCE),
            serde_json::to_string(&serde_json::json!({
                "schema_version": 2,
                "artifact_profile": artifact.id.as_str(),
                "runtime_capability_ids": artifact.runtime_ids.as_slice(),
                "outputs": artifact.outputs.as_slice(),
                "wasm": { "path": WEB_PACKAGE_WASM }
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(artifact_root.join(WEB_PACKAGE_WASM), b"wasm").unwrap();
    }

    fn measurement_for_test() -> WasmMeasurement {
        WasmMeasurement {
            raw_bytes: 10,
            stripped_bytes: Some(7),
            gzip_bytes: 5,
            brotli_bytes: 4,
            artifact_path: PathBuf::from("target/test.wasm"),
            compressed_source: CompressionSource::Stripped,
        }
    }
}
