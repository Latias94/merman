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

const TRANSPORT_TARGETS: &[(&str, &str)] = &[
    ("merman-android-jni", "native"),
    ("merman-ffi", "native"),
    ("merman-typst-plugin", "typst"),
    ("merman-uniffi", "native"),
    ("merman-wasm", "web"),
];
const HOST_TRANSPORT_PACKAGES: &[&str] = &["merman-ffi", "merman-uniffi"];
const EMPTY_DEFAULT_PACKAGES: &[&str] = &[
    "merman-analysis",
    "merman-android-jni",
    "merman-ascii",
    "merman-bindings-core",
    "merman-core",
    "merman-editor-core",
    "merman-export",
    "merman-ffi",
    "merman-lsp",
    "merman-render",
    "merman-typst-plugin",
    "merman-uniffi",
    "merman-wasm",
    "roughr-merman",
];
const PUBLIC_FEATURE_ALLOWLIST_EXTRAS: &[(&str, &[&str])] = &[
    ("merman", &["complete-svg"]),
    ("merman-analysis", &[]),
    ("merman-android-jni", &[]),
    ("merman-ascii", &[]),
    ("merman-bindings-core", &[]),
    ("merman-cli", &[]),
    ("merman-core", &[]),
    ("merman-editor-core", &[]),
    ("merman-export", &[]),
    ("merman-ffi", &[]),
    ("merman-lsp", &["stdio"]),
    ("merman-render", &[]),
    ("merman-rustdoc", &["complete-svg"]),
    ("merman-typst-plugin", &[]),
    ("merman-uniffi", &["bindgen-smoke"]),
    ("merman-wasm", &[]),
    ("roughr-merman", &[]),
];
const SVG_ENGINE_FEATURES: &[&str] = &["layout-cytoscape", "layout-elk", "math"];
const PRODUCT_FEATURE_CONTRACTS: usize = 5;
const PAIRWISE_PACKAGES: &[&str] = &[
    "merman",
    "merman-android-jni",
    "merman-bindings-core",
    "merman-cli",
    "merman-ffi",
    "merman-typst-plugin",
    "merman-uniffi",
    "merman-wasm",
];

fn transport_packages() -> impl Iterator<Item = &'static str> {
    TRANSPORT_TARGETS.iter().map(|(package, _)| *package)
}

fn wasm_transport_packages() -> impl Iterator<Item = &'static str> {
    TRANSPORT_TARGETS
        .iter()
        .filter_map(|(package, target)| matches!(*target, "typst" | "web").then_some(*package))
}

fn complete_svg_features() -> impl Iterator<Item = &'static str> {
    SVG_ENGINE_FEATURES
        .iter()
        .copied()
        .chain(std::iter::once("svg"))
}

#[derive(Debug, Clone, Copy)]
struct FeatureForwardingContract {
    package: &'static str,
    dependency: &'static str,
    features: &'static [&'static str],
}

const FEATURE_FORWARDING_CONTRACTS: &[FeatureForwardingContract] = &[
    FeatureForwardingContract {
        package: "merman-android-jni",
        dependency: "merman-bindings-core",
        features: &[
            "analysis",
            "ascii",
            "jpeg",
            "layout-cytoscape",
            "layout-elk",
            "math",
            "pdf",
            "png",
            "svg",
            "system-clock",
            "system-random",
            "system-timezone",
        ],
    },
    FeatureForwardingContract {
        package: "merman-bindings-core",
        dependency: "merman",
        features: &[
            "ascii",
            "jpeg",
            "layout-cytoscape",
            "layout-elk",
            "math",
            "pdf",
            "png",
            "svg",
            "system-clock",
            "system-random",
            "system-timezone",
        ],
    },
    FeatureForwardingContract {
        package: "merman-cli",
        dependency: "merman",
        features: &[
            "ascii",
            "jpeg",
            "layout-cytoscape",
            "layout-elk",
            "math",
            "pdf",
            "png",
            "svg",
            "system-clock",
            "system-random",
            "system-timezone",
            "system-timing",
        ],
    },
    FeatureForwardingContract {
        package: "merman-ffi",
        dependency: "merman-bindings-core",
        features: &[
            "analysis",
            "ascii",
            "jpeg",
            "layout-cytoscape",
            "layout-elk",
            "math",
            "pdf",
            "png",
            "svg",
            "system-clock",
            "system-random",
            "system-timezone",
        ],
    },
    FeatureForwardingContract {
        package: "merman-uniffi",
        dependency: "merman-bindings-core",
        features: &[
            "analysis",
            "ascii",
            "jpeg",
            "layout-cytoscape",
            "layout-elk",
            "math",
            "pdf",
            "png",
            "svg",
            "system-clock",
            "system-random",
            "system-timezone",
        ],
    },
    FeatureForwardingContract {
        package: "merman-wasm",
        dependency: "merman-bindings-core",
        features: &[
            "analysis",
            "ascii",
            "layout-cytoscape",
            "layout-elk",
            "math",
            "svg",
        ],
    },
    FeatureForwardingContract {
        package: "merman-typst-plugin",
        dependency: "merman-bindings-core",
        features: &["analysis", "layout-cytoscape", "layout-elk", "svg"],
    },
    FeatureForwardingContract {
        package: "merman-rustdoc",
        dependency: "merman",
        features: &["layout-cytoscape", "layout-elk", "math", "svg"],
    },
];

#[derive(Debug, Clone, Copy)]
struct DependencyFeatureContract {
    package: &'static str,
    dependency: &'static str,
    expected_features: &'static [&'static str],
}

const DEPENDENCY_FEATURE_CONTRACTS: &[DependencyFeatureContract] = &[
    DependencyFeatureContract {
        package: "dugong",
        dependency: "serde_json",
        expected_features: &[],
    },
    DependencyFeatureContract {
        package: "dugong-graphlib",
        dependency: "serde_json",
        expected_features: &[],
    },
    DependencyFeatureContract {
        package: "manatee",
        dependency: "indexmap",
        expected_features: &[],
    },
    DependencyFeatureContract {
        package: "merman-core",
        dependency: "indexmap",
        expected_features: &["serde"],
    },
    DependencyFeatureContract {
        package: "merman-core",
        dependency: "serde_json",
        expected_features: &["preserve_order"],
    },
    DependencyFeatureContract {
        package: "merman-render",
        dependency: "indexmap",
        expected_features: &["serde"],
    },
    DependencyFeatureContract {
        package: "merman-render",
        dependency: "serde_json",
        expected_features: &[],
    },
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
    #[serde(default)]
    metadata: serde_json::Value,
    #[serde(default)]
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoDependency {
    name: String,
    kind: Option<String>,
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoDistRecipe {
    features: Vec<String>,
}

#[derive(Debug)]
struct FeatureGraph {
    packages: BTreeMap<String, CargoPackage>,
}

#[derive(Debug, Clone)]
struct BuildCase {
    package: String,
    features: Vec<String>,
    default_features: bool,
    target: Option<String>,
    reason: &'static str,
}

pub(crate) fn verify_feature_matrix(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let root = super::workspace_root();

    if let Some(message) = super::capability_surface::verify_capability_surface_artifacts()? {
        return Err(matrix_error(message));
    }
    let artifact_profiles =
        super::artifact_profiles::load_artifact_profile_catalog().map_err(matrix_error)?;
    println!(
        "validated {} exact artifact build profile(s)",
        artifact_profiles.profile_count()
    );

    let graph = FeatureGraph::load()?;
    let report = graph.validate()?;
    println!(
        "feature-matrix structure packages={} capability_leaves={} empty_defaults={} feature_allowlists={} forwarding_edges={} dependency_feature_boundaries={} transport_engines={} product_contracts={}",
        report.capability_bearing_packages,
        report.capability_leaves,
        report.empty_defaults,
        report.feature_allowlists,
        report.forwarding_edges,
        report.dependency_feature_boundaries,
        report.transport_engines,
        report.product_contracts
    );

    let (host_artifacts, wasm_artifacts) = if options.strict {
        artifact_profiles.into_profiles()
    } else {
        (Vec::new(), Vec::new())
    };
    let cases = graph.build_cases(options.strict, &wasm_artifacts)?;
    for (index, case) in cases.iter().enumerate() {
        println!(
            "feature-matrix build {}/{} package={} default_features={} features={} target={} reason={}",
            index + 1,
            cases.len(),
            case.package,
            case.default_features,
            display_features(&case.features),
            case.target.as_deref().unwrap_or("host"),
            case.reason
        );
        run_build_case(&root, case)?;
    }
    for profile in &host_artifacts {
        println!(
            "feature-matrix artifact package={} profile={} target=host features={}",
            profile.package,
            profile.id,
            display_features(&profile.features)
        );
        run_host_artifact_case(&root, profile)?;
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
                    "Always validates Cargo feature implications and a curated set of product and transport builds."
                );
                println!("  --strict  build every public capability leaf, a bounded pairwise set,");
                println!("            and every exact host plus Web/Typst WASM artifact recipe");
                println!("            finite native/release target sets remain with owner CI");
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
        report.empty_defaults = self.validate_empty_default_contracts(EMPTY_DEFAULT_PACKAGES)?;
        report.feature_allowlists = self.validate_public_feature_allowlists()?;
        report.forwarding_edges = self.validate_feature_forwarding(FEATURE_FORWARDING_CONTRACTS)?;
        report.dependency_feature_boundaries =
            self.validate_dependency_feature_contracts(DEPENDENCY_FEATURE_CONTRACTS)?;
        let transport_package_names = transport_packages().collect::<Vec<_>>();
        report.transport_engines = self.validate_transport_contracts(&transport_package_names)?;
        report.product_contracts = self.validate_product_feature_contracts()?;
        Ok(report)
    }

    fn validate_empty_default_contracts(&self, packages: &[&str]) -> Result<usize, XtaskError> {
        for package_name in packages {
            let package = self.package(package_name)?;
            let defaults = package.features.get("default").ok_or_else(|| {
                matrix_error(format!(
                    "{}: low-level package must declare an explicit empty `default` feature",
                    package.manifest_path.display()
                ))
            })?;
            if !defaults.is_empty() {
                return Err(matrix_error(format!(
                    "{}: low-level package default must be empty; exact product and artifact profiles select capabilities",
                    package.manifest_path.display()
                )));
            }
        }
        Ok(packages.len())
    }

    fn validate_public_feature_allowlists(&self) -> Result<usize, XtaskError> {
        let mut checked = 0;
        for (package_name, extras) in PUBLIC_FEATURE_ALLOWLIST_EXTRAS {
            self.validate_public_feature_allowlist(package_name, extras)?;
            checked += 1;
        }
        Ok(checked)
    }

    fn validate_public_feature_allowlist(
        &self,
        package_name: &str,
        extras: &[&str],
    ) -> Result<(), XtaskError> {
        let package = self.package(package_name)?;
        let mut allowed = self
            .capability_features(package)
            .into_iter()
            .collect::<BTreeSet<_>>();
        allowed.insert("default".to_string());
        allowed.extend(extras.iter().map(|feature| (*feature).to_string()));
        let unexpected = package
            .features
            .keys()
            .filter(|feature| !allowed.contains(*feature))
            .cloned()
            .collect::<Vec<_>>();
        if !unexpected.is_empty() {
            return Err(matrix_error(format!(
                "{}: unexpected public Cargo features outside the canonical allowlist: {}",
                package.manifest_path.display(),
                unexpected.join(", ")
            )));
        }
        Ok(())
    }

    fn validate_feature_forwarding(
        &self,
        contracts: &[FeatureForwardingContract],
    ) -> Result<usize, XtaskError> {
        let mut checked = 0;
        for contract in contracts {
            let package = self.package(contract.package)?;
            for feature in contract.features {
                let edges = self.transitive_feature_edges(contract.package, feature)?;
                let forwards = edges.iter().any(|edge| {
                    dependency_feature(edge).is_some_and(|(dependency, forwarded)| {
                        dependency == contract.dependency && forwarded == *feature
                    })
                });
                if !forwards {
                    return Err(matrix_error(format!(
                        "{}: feature `{feature}` must forward `{feature}` to dependency `{}`",
                        package.manifest_path.display(),
                        contract.dependency
                    )));
                }
                checked += 1;
            }
        }
        Ok(checked)
    }

    fn validate_dependency_feature_contracts(
        &self,
        contracts: &[DependencyFeatureContract],
    ) -> Result<usize, XtaskError> {
        for contract in contracts {
            let package = self.package(contract.package)?;
            let matching = package
                .dependencies
                .iter()
                .filter(|dependency| {
                    dependency.name == contract.dependency && dependency.kind.is_none()
                })
                .collect::<Vec<_>>();
            if matching.is_empty() {
                return Err(matrix_error(format!(
                    "{}: required direct dependency `{}` is missing",
                    package.manifest_path.display(),
                    contract.dependency
                )));
            }
            let actual = matching
                .into_iter()
                .flat_map(|dependency| dependency.features.iter().cloned())
                .collect::<BTreeSet<_>>();
            let expected = contract
                .expected_features
                .iter()
                .map(|feature| (*feature).to_string())
                .collect::<BTreeSet<_>>();
            if actual != expected {
                return Err(matrix_error(format!(
                    "{}: direct dependency `{}` feature boundary expected {}, found {}",
                    package.manifest_path.display(),
                    contract.dependency,
                    display_feature_set(&expected),
                    display_feature_set(&actual)
                )));
            }
        }
        Ok(contracts.len())
    }

    fn validate_product_feature_contracts(&self) -> Result<usize, XtaskError> {
        let facade = self.package("merman")?;
        let facade_defaults = direct_feature_members(facade, "default")?;
        let expected_defaults = BTreeSet::from(["complete-svg".to_string()]);
        if facade_defaults != expected_defaults {
            return Err(matrix_error(format!(
                "{}: merman default must equal `complete-svg`; expected {expected_defaults:?}, found {facade_defaults:?}",
                facade.manifest_path.display()
            )));
        }

        let complete_svg = direct_feature_members(facade, "complete-svg")?;
        let expected_complete_svg = complete_svg_features()
            .map(|feature| feature.to_string())
            .collect::<BTreeSet<_>>();
        if complete_svg != expected_complete_svg {
            return Err(matrix_error(format!(
                "{}: `complete-svg` must contain exactly {expected_complete_svg:?}; found {complete_svg:?}",
                facade.manifest_path.display()
            )));
        }

        let rustdoc = self.package("merman-rustdoc")?;
        let rustdoc_defaults = direct_feature_members(rustdoc, "default")?;
        if rustdoc_defaults != expected_defaults {
            return Err(matrix_error(format!(
                "{}: merman-rustdoc default must equal `complete-svg`; expected {expected_defaults:?}, found {rustdoc_defaults:?}",
                rustdoc.manifest_path.display()
            )));
        }

        let rustdoc_complete_svg = direct_feature_members(rustdoc, "complete-svg")?;
        if rustdoc_complete_svg != expected_complete_svg {
            return Err(matrix_error(format!(
                "{}: merman-rustdoc `complete-svg` must contain exactly {expected_complete_svg:?}; found {rustdoc_complete_svg:?}",
                rustdoc.manifest_path.display()
            )));
        }

        let cli = self.package("merman-cli")?;
        let dist_metadata = cli.metadata.get("dist").ok_or_else(|| {
            matrix_error(format!(
                "{}: missing published cargo-dist recipe in package metadata",
                cli.manifest_path.display()
            ))
        })?;
        let dist_recipe = serde_json::from_value::<CargoDistRecipe>(dist_metadata.clone())
            .map_err(|error| {
                matrix_error(format!(
                    "{}: invalid published cargo-dist recipe: {error}",
                    cli.manifest_path.display()
                ))
            })?;
        let published_features = dist_recipe.features.into_iter().collect::<BTreeSet<_>>();
        let cli_defaults = direct_feature_members(cli, "default")?;
        if cli_defaults != published_features {
            return Err(matrix_error(format!(
                "{}: CLI default must match the published cargo-dist recipe; expected {published_features:?}, found {cli_defaults:?}",
                cli.manifest_path.display()
            )));
        }

        Ok(PRODUCT_FEATURE_CONTRACTS)
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

    fn transitive_feature_edges(
        &self,
        package_name: &str,
        root: &str,
    ) -> Result<BTreeSet<String>, XtaskError> {
        let package = self.package(package_name)?;
        let mut edges_seen = BTreeSet::new();
        let mut local_seen = BTreeSet::new();
        let mut pending = vec![root.to_string()];
        while let Some(feature) = pending.pop() {
            if !local_seen.insert(feature.clone()) {
                continue;
            }
            let edges = package.features.get(&feature).ok_or_else(|| {
                matrix_error(format!(
                    "{}: forwarding contract references unknown feature `{feature}`",
                    package.manifest_path.display()
                ))
            })?;
            for edge in edges {
                edges_seen.insert(edge.clone());
                if !edge.starts_with("dep:")
                    && !edge.contains('/')
                    && package.features.contains_key(edge)
                {
                    pending.push(edge.clone());
                }
            }
        }
        Ok(edges_seen)
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
        cases.insert(BuildCase::with_defaults("merman", None, "facade-default"));
        self.package("merman-rustdoc")?;
        cases.insert(BuildCase::with_defaults(
            "merman-rustdoc",
            None,
            "rustdoc-default",
        ));
        let transport_package_names = if strict {
            transport_packages().collect::<Vec<_>>()
        } else {
            HOST_TRANSPORT_PACKAGES.to_vec()
        };
        for package_name in transport_package_names {
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
            for feature in complete_svg_features() {
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
        match package_name {
            "merman-android-jni" => Some("aarch64-linux-android".to_string()),
            package if wasm_transport_packages().any(|candidate| candidate == package) => {
                Some("wasm32-unknown-unknown".to_string())
            }
            _ => None,
        }
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
            default_features: false,
            target,
            reason,
        }
    }

    fn with_defaults(package: &str, target: Option<String>, reason: &'static str) -> Self {
        Self {
            package: package.to_string(),
            features: Vec::new(),
            default_features: true,
            target,
            reason,
        }
    }

    fn comparison_key(&self) -> (&str, bool, &[String], Option<&str>) {
        (
            &self.package,
            self.default_features,
            &self.features,
            self.target.as_deref(),
        )
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
    empty_defaults: usize,
    feature_allowlists: usize,
    forwarding_edges: usize,
    dependency_feature_boundaries: usize,
    transport_engines: usize,
    product_contracts: usize,
}

fn dependency_feature(edge: &str) -> Option<(&str, &str)> {
    let (dependency, feature) = edge.split_once('/')?;
    Some((dependency.trim_end_matches('?'), feature))
}

fn direct_feature_members(
    package: &CargoPackage,
    feature: &str,
) -> Result<BTreeSet<String>, XtaskError> {
    package
        .features
        .get(feature)
        .map(|members| members.iter().cloned().collect())
        .ok_or_else(|| {
            matrix_error(format!(
                "{}: required feature `{feature}` is missing",
                package.manifest_path.display()
            ))
        })
}

fn display_features(features: &[String]) -> String {
    if features.is_empty() {
        "<none>".to_string()
    } else {
        features.join(",")
    }
}

fn display_feature_set(features: &BTreeSet<String>) -> String {
    format!(
        "{{{}}}",
        features.iter().cloned().collect::<Vec<_>>().join(", ")
    )
}

fn run_build_case(root: &std::path::Path, case: &BuildCase) -> Result<(), XtaskError> {
    let mut command = Command::new("cargo");
    command.args(["check", "--locked", "-p", &case.package]);
    if !case.default_features {
        command.arg("--no-default-features");
    }
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

fn run_host_artifact_case(
    root: &std::path::Path,
    profile: &super::artifact_profiles::HostArtifactProfile,
) -> Result<(), XtaskError> {
    let mut command = Command::new("cargo");
    command.args(["check", "--locked"]).current_dir(root);
    profile
        .configure_cargo_command(&mut command)
        .map_err(matrix_error)?;
    let status = command.status().map_err(|error| {
        matrix_error(format!(
            "cannot build exact host artifact profile `{}`: {error}",
            profile.id
        ))
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(matrix_error(format!(
            "exact host artifact profile `{}` failed with {status}",
            profile.id
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLI_DEFAULT_FEATURES: &[&str] = &[
        "analysis",
        "ascii",
        "icons",
        "jpeg",
        "layout-cytoscape",
        "layout-elk",
        "markdown",
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

    fn package(name: &str, features: &[(&str, &[&str])]) -> CargoPackage {
        package_with_metadata(name, features, serde_json::json!({}))
    }

    fn package_with_metadata(
        name: &str,
        features: &[(&str, &[&str])],
        metadata: serde_json::Value,
    ) -> CargoPackage {
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
            metadata,
            dependencies: Vec::new(),
        }
    }

    fn package_with_dependency_features(
        name: &str,
        dependencies: &[(&str, &[&str])],
    ) -> CargoPackage {
        let mut package = package(name, &[]);
        package.dependencies = dependencies
            .iter()
            .map(|(dependency, features)| CargoDependency {
                name: (*dependency).to_string(),
                kind: None,
                features: features
                    .iter()
                    .map(|feature| (*feature).to_string())
                    .collect(),
            })
            .collect();
        package
    }

    fn graph(packages: Vec<CargoPackage>) -> FeatureGraph {
        FeatureGraph {
            packages: packages
                .into_iter()
                .map(|package| (package.name.clone(), package))
                .collect(),
        }
    }

    fn product_contract_graph() -> FeatureGraph {
        graph(vec![
            package(
                "merman",
                &[
                    ("default", &["complete-svg"]),
                    (
                        "complete-svg",
                        &["svg", "layout-cytoscape", "layout-elk", "math"],
                    ),
                ],
            ),
            package(
                "merman-rustdoc",
                &[
                    ("default", &["complete-svg"]),
                    (
                        "complete-svg",
                        &["svg", "layout-cytoscape", "layout-elk", "math"],
                    ),
                ],
            ),
            package_with_metadata(
                "merman-cli",
                &[("default", CLI_DEFAULT_FEATURES)],
                serde_json::json!({
                    "dist": {
                        "default-features": false,
                        "features": CLI_DEFAULT_FEATURES
                    }
                }),
            ),
        ])
    }

    #[test]
    fn product_feature_contracts_accept_exact_defaults_and_aggregate() {
        let graph = product_contract_graph();
        assert_eq!(
            graph.validate_product_feature_contracts().unwrap(),
            PRODUCT_FEATURE_CONTRACTS
        );
    }

    #[test]
    fn facade_default_must_be_exactly_complete_svg() {
        let mut graph = product_contract_graph();
        graph
            .packages
            .get_mut("merman")
            .unwrap()
            .features
            .insert("default".to_string(), vec!["svg".to_string()]);

        let error = graph.validate_product_feature_contracts().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("default must equal `complete-svg`"),
            "{error}"
        );
    }

    #[test]
    fn complete_svg_aggregate_must_have_exact_leaf_members() {
        let mut graph = product_contract_graph();
        graph
            .packages
            .get_mut("merman")
            .unwrap()
            .features
            .get_mut("complete-svg")
            .unwrap()
            .retain(|feature| feature != "math");

        let error = graph.validate_product_feature_contracts().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("`complete-svg` must contain exactly"),
            "{error}"
        );
    }

    #[test]
    fn rustdoc_default_must_be_exactly_complete_svg() {
        let mut graph = product_contract_graph();
        graph
            .packages
            .get_mut("merman-rustdoc")
            .unwrap()
            .features
            .insert("default".to_string(), vec!["svg".to_string()]);

        let error = graph.validate_product_feature_contracts().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("merman-rustdoc default must equal `complete-svg`"),
            "{error}"
        );
    }

    #[test]
    fn rustdoc_complete_svg_aggregate_must_have_exact_leaf_members() {
        let mut graph = product_contract_graph();
        graph
            .packages
            .get_mut("merman-rustdoc")
            .unwrap()
            .features
            .get_mut("complete-svg")
            .unwrap()
            .retain(|feature| feature != "math");

        let error = graph.validate_product_feature_contracts().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("merman-rustdoc `complete-svg` must contain exactly"),
            "{error}"
        );
    }

    #[test]
    fn cli_default_must_match_the_published_cargo_dist_recipe() {
        let mut graph = product_contract_graph();
        graph.packages.get_mut("merman-cli").unwrap().metadata["dist"]["features"] =
            serde_json::json!(["analysis"]);

        let error = graph.validate_product_feature_contracts().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("CLI default must match the published cargo-dist recipe"),
            "{error}"
        );
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
    fn derived_matrix_groups_preserve_expected_members() {
        let transports = transport_packages().collect::<Vec<_>>();
        let wasm_transports = wasm_transport_packages().collect::<Vec<_>>();

        assert_eq!(
            transports,
            vec![
                "merman-android-jni",
                "merman-ffi",
                "merman-typst-plugin",
                "merman-uniffi",
                "merman-wasm",
            ]
        );
        assert_eq!(wasm_transports, vec!["merman-typst-plugin", "merman-wasm"]);
        assert!(
            HOST_TRANSPORT_PACKAGES.iter().all(|package| {
                transports.contains(package) && !wasm_transports.contains(package)
            })
        );
        assert_eq!(
            complete_svg_features().collect::<Vec<_>>(),
            vec!["layout-cytoscape", "layout-elk", "math", "svg"]
        );
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
    fn low_level_packages_must_keep_explicit_empty_defaults() {
        let graph = graph(vec![package(
            "merman-bindings-core",
            &[("default", &["svg"]), ("svg", &["merman/svg"])],
        )]);

        let error = graph
            .validate_empty_default_contracts(&["merman-bindings-core"])
            .unwrap_err();
        assert!(
            error.to_string().contains("default must be empty"),
            "{error}"
        );
    }

    #[test]
    fn dependency_feature_boundary_rejects_workspace_feature_leakage() {
        let graph = graph(vec![package_with_dependency_features(
            "manatee",
            &[("indexmap", &["serde"])],
        )]);
        let contract = DependencyFeatureContract {
            package: "manatee",
            dependency: "indexmap",
            expected_features: &[],
        };

        let error = graph
            .validate_dependency_feature_contracts(&[contract])
            .unwrap_err();
        assert!(error.to_string().contains("indexmap"), "{error}");
        assert!(error.to_string().contains("expected {}"), "{error}");
        assert!(error.to_string().contains("found {serde}"), "{error}");
    }

    #[test]
    fn public_feature_allowlist_rejects_retired_presets() {
        let graph = graph(vec![package(
            "merman-wasm",
            &[("default", &[]), ("svg", &[]), ("full", &["svg"])],
        )]);

        let error = graph
            .validate_public_feature_allowlist("merman-wasm", &[])
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unexpected public Cargo features"),
            "{error}"
        );
        assert!(error.to_string().contains("full"), "{error}");
    }

    #[test]
    fn forwarding_contract_requires_the_dependency_capability_edge() {
        let graph = graph(vec![package(
            "merman-wasm",
            &[("default", &[]), ("svg", &["dep:js-sys"])],
        )]);
        let contract = FeatureForwardingContract {
            package: "merman-wasm",
            dependency: "merman-bindings-core",
            features: &["svg"],
        };

        let error = graph.validate_feature_forwarding(&[contract]).unwrap_err();
        assert!(error.to_string().contains("must forward `svg`"), "{error}");
    }

    #[test]
    fn pairwise_matrix_includes_bindings_core() {
        assert!(PAIRWISE_PACKAGES.contains(&"merman-bindings-core"));
    }

    #[test]
    fn default_build_cases_remain_distinct_from_empty_feature_builds() {
        let no_defaults = BuildCase::new("merman", Vec::new(), None, "facade-base");
        let defaults = BuildCase::with_defaults("merman", None, "facade-default");
        let cases = BTreeSet::from([no_defaults, defaults]);

        assert_eq!(cases.len(), 2);
        assert!(cases.iter().any(|case| case.default_features));
        assert!(cases.iter().any(|case| !case.default_features));
    }

    #[test]
    fn transport_cases_use_their_real_compile_targets_in_strict_mode() {
        let graph = graph(Vec::new());
        assert_eq!(
            graph.build_target_for("merman-android-jni").as_deref(),
            Some("aarch64-linux-android")
        );
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
