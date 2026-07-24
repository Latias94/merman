//! Structured Cargo feature-matrix verification.

use crate::XtaskError;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

#[allow(dead_code)]
mod capability_contract {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../capabilities/generated/capability_surface.rs"
    ));
}

const TRANSPORT_PACKAGES: &[&str] = &[
    "merman-ffi",
    "merman-typst-plugin",
    "merman-uniffi",
    "merman-wasm",
];
const TRANSPORT_TARGETS: &[(&str, &str)] = &[
    ("merman-ffi", "native"),
    ("merman-typst-plugin", "typst"),
    ("merman-uniffi", "native"),
    ("merman-wasm", "web"),
];
const HOST_TRANSPORT_PACKAGES: &[&str] = &["merman-ffi", "merman-uniffi"];
const WASM_TRANSPORT_PACKAGES: &[&str] = &["merman-typst-plugin", "merman-wasm"];
const SVG_ENGINE_FEATURES: &[&str] = &["layout-cytoscape", "layout-elk", "math"];
const PAIRWISE_PACKAGES: &[&str] = &[
    "merman",
    "merman-cli",
    "merman-ffi",
    "merman-typst-plugin",
    "merman-uniffi",
    "merman-wasm",
];

#[derive(Debug, Default, PartialEq, Eq)]
struct FeatureMatrixOptions {
    strict: bool,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
struct FeatureGraph {
    packages: BTreeMap<String, CargoPackage>,
}

#[derive(Debug, Clone)]
struct BuildCase {
    package: String,
    features: Vec<String>,
    target: Option<String>,
    reason: &'static str,
}

pub(crate) fn verify_feature_matrix(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let root = super::workspace_root();

    if let Some(message) = super::capability_surface::verify_capability_surface_artifacts()? {
        return Err(matrix_error(message));
    }
    super::artifact_profiles::verify_artifact_profiles(Vec::new()).map_err(matrix_error)?;

    let graph = FeatureGraph::load()?;
    let report = graph.validate()?;
    println!(
        "feature-matrix structure packages={} capability_leaves={} transport_engines={}",
        report.capability_bearing_packages, report.capability_leaves, report.transport_engines
    );

    let wasm_artifacts = if options.strict {
        super::artifact_profiles::load_wasm_size_artifact_profiles().map_err(matrix_error)?
    } else {
        Vec::new()
    };
    let cases = graph.build_cases(options.strict, &wasm_artifacts)?;
    for (index, case) in cases.iter().enumerate() {
        println!(
            "feature-matrix build {}/{} package={} features={} target={} reason={}",
            index + 1,
            cases.len(),
            case.package,
            display_features(&case.features),
            case.target.as_deref().unwrap_or("host"),
            case.reason
        );
        run_build_case(&root, case)?;
    }
    for profile in &wasm_artifacts {
        println!(
            "feature-matrix artifact package={} profile={} target={} features={}",
            profile.package,
            profile.id,
            profile.target_triple,
            display_features(&profile.features)
        );
        run_wasm_artifact_case(&root, profile)?;
    }

    Ok(())
}

fn parse_options(args: Vec<String>) -> Result<FeatureMatrixOptions, XtaskError> {
    let mut options = FeatureMatrixOptions::default();
    for arg in args {
        match arg.as_str() {
            "--strict" => options.strict = true,
            "--help" | "-h" => {
                println!("usage: xtask verify-feature-matrix [--strict]");
                println!();
                println!(
                    "Always validates Cargo feature implications, leaf builds, and exact artifact recipes."
                );
                println!("  --strict  build every public capability leaf, a bounded pairwise set,");
                println!("            and every exact Web/Typst WASM artifact recipe");
                return Err(XtaskError::Usage);
            }
            _ => return Err(XtaskError::Usage),
        }
    }
    Ok(options)
}

fn matrix_error(message: impl Into<String>) -> XtaskError {
    XtaskError::FeatureMatrix(message.into())
}

impl FeatureGraph {
    fn load() -> Result<Self, XtaskError> {
        let root = super::workspace_root();
        let output = Command::new("cargo")
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
            .current_dir(&root)
            .output()
            .map_err(|error| matrix_error(format!("cannot execute `cargo metadata`: {error}")))?;
        if !output.status.success() {
            return Err(matrix_error(format!(
                "`cargo metadata` failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let metadata = serde_json::from_slice::<CargoMetadata>(&output.stdout)
            .map_err(|error| matrix_error(format!("cannot decode `cargo metadata`: {error}")))?;
        Self::from_metadata(metadata).map_err(matrix_error)
    }

    fn from_metadata(metadata: CargoMetadata) -> Result<Self, String> {
        let workspace_members = metadata
            .workspace_members
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut packages = BTreeMap::new();
        for package in metadata
            .packages
            .into_iter()
            .filter(|package| workspace_members.contains(&package.id))
        {
            let name = package.name.clone();
            if packages.insert(name.clone(), package).is_some() {
                return Err(format!(
                    "Cargo metadata contains duplicate workspace package `{name}`"
                ));
            }
        }
        if packages.is_empty() {
            return Err("Cargo metadata contains no workspace packages".to_string());
        }
        Ok(Self { packages })
    }

    fn validate(&self) -> Result<ValidationReport, XtaskError> {
        let mut report = ValidationReport::default();
        for package in self.packages.values() {
            let capability_features = self.capability_features(package);
            if !capability_features.is_empty() {
                report.capability_bearing_packages += 1;
            }
            report.capability_leaves += capability_features.len();
            self.validate_capability_implications(package)?;
        }
        report.transport_engines = self.validate_transport_contracts(TRANSPORT_PACKAGES)?;
        Ok(report)
    }

    fn package(&self, name: &str) -> Result<&CargoPackage, XtaskError> {
        self.packages
            .get(name)
            .ok_or_else(|| matrix_error(format!("required workspace package `{name}` is missing")))
    }

    fn capability_features(&self, package: &CargoPackage) -> Vec<String> {
        capability_contract::CAPABILITY_IDS
            .iter()
            .filter(|feature| package.features.contains_key(**feature))
            .map(|feature| (*feature).to_string())
            .collect()
    }

    fn validate_capability_implications(&self, package: &CargoPackage) -> Result<(), XtaskError> {
        for capability in capability_contract::CAPABILITIES {
            if !package.features.contains_key(capability.id) {
                continue;
            }
            let closure = self.local_closure(&package.name, [capability.id])?;
            for implication in capability.implications {
                if package.features.contains_key(*implication) && !closure.contains(*implication) {
                    return Err(matrix_error(format!(
                        "{}: feature `{}` must imply this crate's `{}` feature",
                        package.manifest_path.display(),
                        capability.id,
                        implication
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_transport_contracts(&self, packages: &[&str]) -> Result<usize, XtaskError> {
        let mut checked = 0;
        for package_name in packages {
            let package = self.package(package_name)?;
            let target = transport_target(package_name)?;
            let defaults = package.features.get("default").ok_or_else(|| {
                matrix_error(format!(
                    "{}: transport package must declare an explicit empty `default` feature",
                    package.manifest_path.display()
                ))
            })?;
            if !defaults.is_empty() {
                return Err(matrix_error(format!(
                    "{}: transport package defaults must be empty; exact artifact profiles select release features",
                    package.manifest_path.display()
                )));
            }
            if !package.features.contains_key("svg") {
                return Err(matrix_error(format!(
                    "{}: transport package must expose its local `svg` capability leaf",
                    package.manifest_path.display()
                )));
            }
            for capability in capability_contract::CAPABILITIES {
                if package.features.contains_key(capability.id)
                    && !capability.targets.contains(&target)
                {
                    return Err(matrix_error(format!(
                        "{}: transport target `{target}` cannot expose capability `{}`",
                        package.manifest_path.display(),
                        capability.id
                    )));
                }
            }
            for feature in SVG_ENGINE_FEATURES {
                if !package.features.contains_key(*feature) {
                    continue;
                }
                checked += 1;
                let closure = self.local_closure(package_name, [*feature])?;
                if !closure.contains("svg") {
                    return Err(matrix_error(format!(
                        "{}: transport feature `{feature}` must imply this crate's `svg` feature",
                        package.manifest_path.display()
                    )));
                }
            }
        }
        Ok(checked)
    }

    fn local_closure<'a>(
        &self,
        package_name: &str,
        roots: impl IntoIterator<Item = &'a str>,
    ) -> Result<BTreeSet<String>, XtaskError> {
        let package = self.package(package_name)?;
        let mut visited = BTreeSet::new();
        let mut pending = roots.into_iter().map(str::to_string).collect::<Vec<_>>();
        while let Some(feature) = pending.pop() {
            if !visited.insert(feature.clone()) {
                continue;
            }
            let edges = package.features.get(&feature).ok_or_else(|| {
                matrix_error(format!(
                    "{}: unknown feature `{feature}`",
                    package.manifest_path.display()
                ))
            })?;
            for edge in edges {
                if !edge.starts_with("dep:")
                    && !edge.contains('/')
                    && package.features.contains_key(edge)
                {
                    pending.push(edge.clone());
                }
            }
        }
        Ok(visited)
    }

    fn build_cases(
        &self,
        strict: bool,
        wasm_artifacts: &[super::artifact_profiles::WasmArtifactProfile],
    ) -> Result<Vec<BuildCase>, XtaskError> {
        let mut cases = BTreeSet::new();
        for (package, features, reason) in [
            ("merman-core", Vec::new(), "core-base"),
            ("merman", Vec::new(), "facade-base"),
            ("merman", vec!["svg".to_string()], "facade-svg"),
            ("merman-lsp", Vec::new(), "lsp-library"),
            ("merman-lsp", vec!["stdio".to_string()], "lsp-stdio"),
        ] {
            self.package(package)?;
            cases.insert(BuildCase::new(package, features, None, reason));
        }
        let transport_packages = if strict {
            TRANSPORT_PACKAGES
        } else {
            HOST_TRANSPORT_PACKAGES
        };
        for package_name in transport_packages {
            let package = self.package(package_name)?;
            let target = if strict {
                self.build_target_for(package_name)
            } else {
                None
            };
            cases.insert(BuildCase::new(
                package_name,
                Vec::new(),
                target.clone(),
                "transport-base",
            ));
            for feature in std::iter::once("svg").chain(SVG_ENGINE_FEATURES.iter().copied()) {
                if package.features.contains_key(feature) {
                    cases.insert(BuildCase::new(
                        package_name,
                        vec![feature.to_string()],
                        target.clone(),
                        "transport-leaf",
                    ));
                }
            }
        }

        if strict {
            for package in self.packages.values() {
                for feature in self.capability_features(package) {
                    cases.insert(BuildCase::new(
                        &package.name,
                        vec![feature],
                        self.build_target_for(&package.name),
                        "capability-leaf",
                    ));
                }
            }
            self.add_pairwise_cases_for(&mut cases, PAIRWISE_PACKAGES)?;
            for profile in wasm_artifacts {
                for feature in &profile.features {
                    cases.insert(BuildCase::new(
                        &profile.package,
                        vec![feature.clone()],
                        Some(profile.target_triple.clone()),
                        "wasm-artifact-leaf",
                    ));
                }
            }
        }

        Ok(cases.into_iter().collect())
    }

    fn add_pairwise_cases_for(
        &self,
        cases: &mut BTreeSet<BuildCase>,
        package_names: &[&str],
    ) -> Result<(), XtaskError> {
        for package_name in package_names {
            let package = self.package(package_name)?;
            let leaves = self.capability_features(package);
            if leaves.len() < 2 {
                continue;
            }
            let offsets = [1, leaves.len() / 2]
                .into_iter()
                .filter(|offset| *offset > 0)
                .collect::<BTreeSet<_>>();
            for offset in offsets {
                for index in 0..leaves.len() {
                    let mut pair = vec![
                        leaves[index].clone(),
                        leaves[(index + offset) % leaves.len()].clone(),
                    ];
                    pair.sort();
                    pair.dedup();
                    if pair.len() == 2 {
                        cases.insert(BuildCase::new(
                            package_name,
                            pair,
                            self.build_target_for(package_name),
                            "bounded-pairwise",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn build_target_for(&self, package_name: &str) -> Option<String> {
        WASM_TRANSPORT_PACKAGES
            .contains(&package_name)
            .then(|| "wasm32-unknown-unknown".to_string())
    }
}

fn transport_target(package: &str) -> Result<&'static str, XtaskError> {
    TRANSPORT_TARGETS
        .iter()
        .find_map(|(candidate, target)| (*candidate == package).then_some(*target))
        .ok_or_else(|| matrix_error(format!("unknown transport package `{package}`")))
}

impl BuildCase {
    fn new(
        package: &str,
        mut features: Vec<String>,
        target: Option<String>,
        reason: &'static str,
    ) -> Self {
        features.sort();
        features.dedup();
        Self {
            package: package.to_string(),
            features,
            target,
            reason,
        }
    }

    fn comparison_key(&self) -> (&str, &[String], Option<&str>) {
        (&self.package, &self.features, self.target.as_deref())
    }
}

impl PartialEq for BuildCase {
    fn eq(&self, other: &Self) -> bool {
        self.comparison_key() == other.comparison_key()
    }
}

impl Eq for BuildCase {}

impl PartialOrd for BuildCase {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BuildCase {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.comparison_key().cmp(&other.comparison_key())
    }
}

#[derive(Debug, Default)]
struct ValidationReport {
    capability_bearing_packages: usize,
    capability_leaves: usize,
    transport_engines: usize,
}

fn display_features(features: &[String]) -> String {
    if features.is_empty() {
        "<none>".to_string()
    } else {
        features.join(",")
    }
}

fn run_build_case(root: &std::path::Path, case: &BuildCase) -> Result<(), XtaskError> {
    let mut command = Command::new("cargo");
    command.args([
        "check",
        "--locked",
        "-p",
        &case.package,
        "--no-default-features",
    ]);
    if !case.features.is_empty() {
        let features = case.features.join(",");
        command.args(["--features", features.as_str()]);
    }
    if let Some(target) = &case.target {
        command.args(["--target", target]);
    }
    let status = command.current_dir(root).status().map_err(|error| {
        matrix_error(format!(
            "cannot build package `{}` with features `{}`: {error}",
            case.package,
            display_features(&case.features)
        ))
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(matrix_error(format!(
            "package `{}` with features `{}` for target `{}` failed with {status}",
            case.package,
            display_features(&case.features),
            case.target.as_deref().unwrap_or("host")
        )))
    }
}

fn run_wasm_artifact_case(
    root: &std::path::Path,
    profile: &super::artifact_profiles::WasmArtifactProfile,
) -> Result<(), XtaskError> {
    let mut command = Command::new("cargo");
    command
        .args(["check", "--locked", "--target", &profile.target_triple])
        .current_dir(root);
    profile.configure_cargo_command(&mut command);
    let status = command.status().map_err(|error| {
        matrix_error(format!(
            "cannot build exact WASM artifact profile `{}`: {error}",
            profile.id
        ))
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(matrix_error(format!(
            "exact WASM artifact profile `{}` failed with {status}",
            profile.id
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, features: &[(&str, &[&str])]) -> CargoPackage {
        CargoPackage {
            id: format!("{name} 0.0.0 (path+file:///workspace/{name})"),
            name: name.to_string(),
            manifest_path: PathBuf::from(format!("/workspace/{name}/Cargo.toml")),
            features: features
                .iter()
                .map(|(feature, edges)| {
                    (
                        (*feature).to_string(),
                        edges.iter().map(|edge| (*edge).to_string()).collect(),
                    )
                })
                .collect(),
        }
    }

    fn graph(packages: Vec<CargoPackage>) -> FeatureGraph {
        FeatureGraph {
            packages: packages
                .into_iter()
                .map(|package| (package.name.clone(), package))
                .collect(),
        }
    }

    #[test]
    fn options_accept_only_the_explicit_strict_mode() {
        assert_eq!(
            parse_options(Vec::new()).unwrap(),
            FeatureMatrixOptions::default()
        );
        assert_eq!(
            parse_options(vec!["--strict".to_string()]).unwrap(),
            FeatureMatrixOptions { strict: true }
        );
        assert!(parse_options(vec!["--unknown".to_string()]).is_err());
    }

    #[test]
    fn transport_engine_must_imply_the_local_svg_leaf() {
        let graph = graph(vec![package(
            "merman-wasm",
            &[
                ("default", &[]),
                ("svg", &["binding/svg"]),
                ("layout-elk", &["binding/layout-elk"]),
            ],
        )]);
        let error = graph
            .validate_transport_contracts(&["merman-wasm"])
            .unwrap_err();
        assert!(
            error.to_string().contains("must imply this crate's `svg`"),
            "{error}"
        );
    }

    #[test]
    fn pairwise_matrix_is_bounded_and_deduplicated() {
        let graph = graph(vec![package(
            "merman",
            &[
                ("analysis", &[]),
                ("ascii", &[]),
                ("svg", &[]),
                ("layout-elk", &["svg"]),
            ],
        )]);
        let mut cases = BTreeSet::new();
        graph
            .add_pairwise_cases_for(&mut cases, &["merman"])
            .unwrap();
        assert!(cases.len() <= 2 * 4);
        assert!(cases.iter().all(|case| case.features.len() == 2));
    }

    #[test]
    fn browser_and_typst_transport_cases_use_the_wasm_target_only_in_strict_mode() {
        let graph = graph(Vec::new());
        assert_eq!(
            graph.build_target_for("merman-wasm").as_deref(),
            Some("wasm32-unknown-unknown")
        );
        assert_eq!(
            graph.build_target_for("merman-typst-plugin").as_deref(),
            Some("wasm32-unknown-unknown")
        );
        assert_eq!(graph.build_target_for("merman-ffi"), None);
    }
}
