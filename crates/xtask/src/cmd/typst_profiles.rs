use crate::{XtaskError, cmd::paths};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
const EXPECTED_PACKAGE: &str = "merman-typst-plugin";
const EXPECTED_ARTIFACT: &str = "merman_typst_plugin.wasm";
const REQUIRED_PUBLISH_FEATURES: &[&str] =
    &["render", "analysis", "cytoscape-layout", "elk-layout"];
const DESCRIPTOR_SOURCE: &str = include_str!("../../../merman-typst-plugin/wasm-profiles.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTypstProfileCatalog {
    schema_version: u32,
    plugin_abi_version: u32,
    package: String,
    manifest_path: String,
    artifact_name: String,
    default_profile: String,
    publish_profile: String,
    profiles: Vec<TypstWasmProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypstWasmProfile {
    name: String,
    aliases: Vec<String>,
    features: Vec<String>,
    capabilities: TypstProfileCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypstProfileCapabilities {
    pub(crate) render: bool,
    pub(crate) analysis: bool,
    pub(crate) ascii: bool,
    pub(crate) core_host: bool,
    pub(crate) elk_layout: bool,
    pub(crate) ratex_math: bool,
    pub(crate) editor_language: bool,
    pub(crate) text_measurement: TypstTextMeasurementCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypstTextMeasurementCapabilities {
    pub(crate) vendored: bool,
    pub(crate) deterministic: bool,
    pub(crate) host_callback: bool,
    pub(crate) font_assets: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TypstProfileCatalog {
    plugin_abi_version: u32,
    package: String,
    manifest_path: PathBuf,
    artifact_name: String,
    default_profile: String,
    profiles: Vec<TypstWasmProfile>,
}

#[derive(Debug, Deserialize)]
struct CargoProfileManifest {
    package: CargoPackage,
    features: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
}

impl TypstWasmProfile {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn features(&self) -> &[String] {
        &self.features
    }

    pub(crate) fn capabilities(&self) -> &TypstProfileCapabilities {
        &self.capabilities
    }

    pub(crate) fn configure_cargo_command(&self, command: &mut Command) {
        command.arg("--no-default-features");
        if !self.features.is_empty() {
            command.arg("--features").arg(self.features.join(","));
        }
    }
}

impl TypstProfileCatalog {
    pub(crate) fn plugin_abi_version(&self) -> u32 {
        self.plugin_abi_version
    }

    pub(crate) fn package(&self) -> &str {
        &self.package
    }

    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub(crate) fn artifact_name(&self) -> &str {
        &self.artifact_name
    }

    pub(crate) fn profiles(&self) -> &[TypstWasmProfile] {
        &self.profiles
    }

    pub(crate) fn resolve_package(
        &self,
        requested: Option<&str>,
    ) -> Result<&TypstWasmProfile, XtaskError> {
        if let Some(requested) = requested {
            return self
                .profiles
                .iter()
                .find(|profile| profile.aliases.iter().any(|alias| alias == requested))
                .ok_or_else(|| {
                    profile_error(format!(
                        "unknown publishable Typst WASM profile `{requested}`; expected one of: {}",
                        self.public_profile_names().join(", ")
                    ))
                });
        }

        self.profiles
            .iter()
            .find(|profile| profile.name == self.default_profile)
            .ok_or_else(|| profile_error("default_profile is not a canonical profile name"))
    }

    pub(crate) fn public_profile_names(&self) -> Vec<&str> {
        let mut names = self
            .profiles
            .iter()
            .flat_map(|profile| profile.aliases.iter().map(String::as_str))
            .collect::<Vec<_>>();
        names.sort_unstable();
        names
    }
}

pub(crate) fn load_typst_profiles() -> Result<TypstProfileCatalog, XtaskError> {
    let raw = parse_descriptor(DESCRIPTOR_SOURCE)?;
    validate_manifest_path(&raw.manifest_path)?;
    let manifest_path = paths::workspace_root().join(&raw.manifest_path);
    let metadata = fs::symlink_metadata(&manifest_path).map_err(|source| XtaskError::ReadFile {
        path: manifest_path.display().to_string(),
        source,
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(profile_error(format!(
            "manifest_path must resolve to a regular file: {}",
            manifest_path.display()
        )));
    }
    let manifest_source =
        fs::read_to_string(&manifest_path).map_err(|source| XtaskError::ReadFile {
            path: manifest_path.display().to_string(),
            source,
        })?;
    validate_and_build(raw, &manifest_source)
}

#[cfg(test)]
fn parse_and_validate(
    descriptor_source: &str,
    manifest_source: &str,
) -> Result<TypstProfileCatalog, XtaskError> {
    let raw = parse_descriptor(descriptor_source)?;
    validate_and_build(raw, manifest_source)
}

fn parse_descriptor(descriptor_source: &str) -> Result<RawTypstProfileCatalog, XtaskError> {
    serde_json::from_str(descriptor_source)
        .map_err(|error| profile_error(format!("failed to parse wasm-profiles.json: {error}")))
}

fn validate_and_build(
    raw: RawTypstProfileCatalog,
    manifest_source: &str,
) -> Result<TypstProfileCatalog, XtaskError> {
    validate_manifest_path(&raw.manifest_path)?;
    let manifest: CargoProfileManifest = toml::from_str(manifest_source).map_err(|error| {
        profile_error(format!("failed to parse {}: {error}", raw.manifest_path))
    })?;
    validate_catalog(&raw, &manifest)?;

    Ok(TypstProfileCatalog {
        plugin_abi_version: raw.plugin_abi_version,
        package: raw.package,
        manifest_path: PathBuf::from(raw.manifest_path),
        artifact_name: raw.artifact_name,
        default_profile: raw.default_profile,
        profiles: raw.profiles,
    })
}

fn validate_manifest_path(path: &str) -> Result<(), XtaskError> {
    let path_value = Path::new(path);
    if path.is_empty()
        || path.contains('\\')
        || path_value.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml")
        || path_value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(profile_error(format!(
            "manifest_path `{path}` must be a repository-relative Cargo.toml path"
        )));
    }
    Ok(())
}

fn validate_catalog(
    catalog: &RawTypstProfileCatalog,
    manifest: &CargoProfileManifest,
) -> Result<(), XtaskError> {
    if catalog.schema_version != DESCRIPTOR_SCHEMA_VERSION {
        return Err(profile_error(format!(
            "unsupported descriptor schema {}; expected {DESCRIPTOR_SCHEMA_VERSION}",
            catalog.schema_version
        )));
    }
    let crate_plugin_abi_version = merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION;
    if catalog.plugin_abi_version != crate_plugin_abi_version {
        return Err(profile_error(format!(
            "plugin_abi_version must match merman-typst-plugin ABI {crate_plugin_abi_version}, found {}",
            catalog.plugin_abi_version,
        )));
    }
    if catalog.package != EXPECTED_PACKAGE || manifest.package.name != EXPECTED_PACKAGE {
        return Err(profile_error(format!(
            "descriptor and Cargo package must both be `{EXPECTED_PACKAGE}`"
        )));
    }
    if catalog.artifact_name != EXPECTED_ARTIFACT {
        return Err(profile_error(format!(
            "descriptor artifact_name must be `{EXPECTED_ARTIFACT}`"
        )));
    }
    if catalog.profiles.is_empty() {
        return Err(profile_error("descriptor must define at least one profile"));
    }

    let mut accepted_names = BTreeSet::new();
    for profile in &catalog.profiles {
        validate_name("profile", &profile.name)?;
        if !accepted_names.insert(profile.name.as_str()) {
            return Err(profile_error(format!(
                "duplicate Typst WASM profile name `{}`",
                profile.name
            )));
        }
        for alias in &profile.aliases {
            validate_name("profile alias", alias)?;
            if !accepted_names.insert(alias.as_str()) {
                return Err(profile_error(format!(
                    "duplicate Typst WASM profile name or alias `{alias}`"
                )));
            }
        }

        let mut features = BTreeSet::new();
        for feature in &profile.features {
            if !features.insert(feature.as_str()) {
                return Err(profile_error(format!(
                    "profile `{}` repeats feature `{feature}`",
                    profile.name
                )));
            }
            if feature == "default" || !manifest.features.contains_key(feature) {
                return Err(profile_error(format!(
                    "profile `{}` references unknown or implicit Cargo feature `{feature}`",
                    profile.name
                )));
            }
        }
        let expected_capabilities = capabilities_for_features(&profile.features);
        if profile.capabilities != expected_capabilities {
            return Err(profile_error(format!(
                "profile `{}` capabilities do not match its feature set",
                profile.name
            )));
        }
    }

    if catalog.default_profile != catalog.publish_profile {
        return Err(profile_error(
            "default_profile must be the same canonical profile as publish_profile",
        ));
    }
    let publish = canonical_profile(catalog, &catalog.publish_profile, "publish_profile")?;
    let required_publish = REQUIRED_PUBLISH_FEATURES
        .iter()
        .map(|feature| (*feature).to_string())
        .collect::<Vec<_>>();
    if publish.features != required_publish {
        return Err(profile_error(format!(
            "publish profile `{}` must enable exactly {}",
            publish.name,
            REQUIRED_PUBLISH_FEATURES.join(",")
        )));
    }
    let cargo_defaults = manifest
        .features
        .get("default")
        .ok_or_else(|| profile_error("Cargo manifest must define a default feature list"))?;
    if cargo_defaults != &publish.features {
        return Err(profile_error(format!(
            "Cargo default features must exactly match publish profile `{}`: expected [{}], found [{}]",
            publish.name,
            publish.features.join(","),
            cargo_defaults.join(",")
        )));
    }

    let analysis = manifest
        .features
        .get("analysis")
        .ok_or_else(|| profile_error("Cargo manifest must define the analysis feature"))?;
    if !analysis.iter().any(|feature| feature == "dep:serde_json") {
        return Err(profile_error(
            "Cargo analysis feature must enable dep:serde_json for Typst resource options",
        ));
    }

    Ok(())
}

fn canonical_profile<'a>(
    catalog: &'a RawTypstProfileCatalog,
    name: &str,
    field: &str,
) -> Result<&'a TypstWasmProfile, XtaskError> {
    catalog
        .profiles
        .iter()
        .find(|profile| profile.name == name)
        .ok_or_else(|| profile_error(format!("{field} `{name}` is not a canonical profile name")))
}

fn capabilities_for_features(features: &[String]) -> TypstProfileCapabilities {
    let enabled = |feature: &str| features.iter().any(|value| value == feature);
    let render = enabled("render");
    TypstProfileCapabilities {
        render,
        analysis: enabled("analysis"),
        ascii: enabled("ascii"),
        core_host: enabled("core-host"),
        elk_layout: enabled("elk-layout"),
        ratex_math: enabled("ratex-math"),
        editor_language: enabled("editor-language"),
        text_measurement: TypstTextMeasurementCapabilities {
            vendored: render,
            deterministic: render,
            host_callback: false,
            font_assets: false,
        },
    }
}

fn validate_name(kind: &str, name: &str) -> Result<(), XtaskError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(profile_error(format!(
            "{kind} `{name}` must use lowercase ASCII letters, digits, and hyphens"
        )));
    }
    Ok(())
}

fn profile_error(message: impl Into<String>) -> XtaskError {
    XtaskError::TypstPackageFailed(format!(
        "Typst WASM profile descriptor is invalid: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_descriptor_matches_cargo_defaults_and_release_contract() {
        let catalog = load_typst_profiles().unwrap();
        let publish = catalog.resolve_package(Some("publish")).unwrap();

        assert_eq!(
            catalog.plugin_abi_version(),
            merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION
        );
        assert_eq!(
            catalog.manifest_path(),
            Path::new("crates/merman-typst-plugin/Cargo.toml")
        );
        assert_eq!(catalog.resolve_package(None).unwrap(), publish);
        assert_eq!(
            catalog.resolve_package(Some("minimal")).unwrap().name(),
            "typst-render-analysis-no-elk"
        );
        assert_eq!(
            publish.features(),
            &["render", "analysis", "cytoscape-layout", "elk-layout"]
        );
        assert!(catalog.resolve_package(Some("default")).is_err());
        assert!(catalog.resolve_package(Some("full")).is_err());
        assert!(catalog.resolve_package(Some("full-elk")).is_err());
        assert!(catalog.resolve_package(Some("full-no-elk")).is_err());
        assert!(catalog.resolve_package(Some("ratex-math")).is_err());
        assert!(catalog.resolve_package(Some("typst-bridge")).is_err());
        assert!(catalog.resolve_package(Some("typst-full-elk")).is_err());
        assert_eq!(catalog.public_profile_names(), vec!["minimal", "publish"]);
        assert!(publish.capabilities().render);
        assert!(publish.capabilities().analysis);
        assert!(publish.capabilities().elk_layout);
        assert!(!publish.capabilities().text_measurement.host_callback);
    }

    #[test]
    fn descriptor_rejects_unknown_fields() {
        let descriptor = valid_descriptor().replace(
            "\"schema_version\": 1,",
            "\"schema_version\": 1, \"unexpected\": true,",
        );
        let error = parse_and_validate(&descriptor, valid_manifest()).unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn descriptor_rejects_a_manifest_path_outside_the_repository() {
        let descriptor = valid_descriptor().replace(
            "crates/merman-typst-plugin/Cargo.toml",
            "../outside/Cargo.toml",
        );
        let error = parse_and_validate(&descriptor, valid_manifest()).unwrap_err();

        assert!(error.to_string().contains("repository-relative"));
    }

    #[test]
    fn descriptor_rejects_cargo_default_drift() {
        let manifest = valid_manifest().replace(
            "default = [\"render\", \"analysis\", \"cytoscape-layout\", \"elk-layout\"]",
            "default = [\"render\", \"analysis\"]",
        );
        let descriptor = valid_descriptor();
        let error = parse_and_validate(&descriptor, &manifest).unwrap_err();

        assert!(error.to_string().contains("must exactly match publish"));
    }

    #[test]
    fn descriptor_rejects_duplicate_aliases() {
        let descriptor = valid_descriptor().replace(
            "\"aliases\": [\"publish\"]",
            "\"aliases\": [\"typst-full-elk\"]",
        );
        let error = parse_and_validate(&descriptor, valid_manifest()).unwrap_err();

        assert!(error.to_string().contains("duplicate Typst WASM profile"));
    }

    #[test]
    fn descriptor_rejects_capabilities_that_do_not_match_features() {
        let descriptor =
            valid_descriptor().replace("\"elk_layout\": true", "\"elk_layout\": false");
        let error = parse_and_validate(&descriptor, valid_manifest()).unwrap_err();

        assert!(error.to_string().contains("do not match its feature set"));
    }

    fn valid_descriptor() -> String {
        r#"{
          "schema_version": 1,
          "plugin_abi_version": 0,
          "package": "merman-typst-plugin",
          "manifest_path": "crates/merman-typst-plugin/Cargo.toml",
          "artifact_name": "merman_typst_plugin.wasm",
          "default_profile": "typst-full-elk",
          "publish_profile": "typst-full-elk",
          "profiles": [
            {
              "name": "typst-full-elk",
              "aliases": ["publish"],
              "features": ["render", "analysis", "cytoscape-layout", "elk-layout"],
              "capabilities": {
                "render": true,
                "analysis": true,
                "ascii": false,
                "core_host": false,
                "elk_layout": true,
                "ratex_math": false,
                "editor_language": false,
                "text_measurement": {
                  "vendored": true,
                  "deterministic": true,
                  "host_callback": false,
                  "font_assets": false
                }
              }
            }
          ]
        }"#
        .replace(
            "\"plugin_abi_version\": 0",
            &format!(
                "\"plugin_abi_version\": {}",
                merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION
            ),
        )
    }

    fn valid_manifest() -> &'static str {
        r#"
          [package]
          name = "merman-typst-plugin"

          [features]
          default = ["render", "analysis", "cytoscape-layout", "elk-layout"]
          analysis = ["dep:serde_json"]
          render = ["dep:serde_json"]
          cytoscape-layout = []
          elk-layout = []
        "#
    }
}
