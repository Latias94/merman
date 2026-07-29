use crate::XtaskError;
use base64::Engine as _;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const BUNDLE_RELATIVE_PATH: &str = "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json";
const REPOS_LOCK_RELATIVE_PATH: &str = "tools/upstreams/REPOS.lock.json";
const EXPECTED_SCHEMA_VERSION: u32 = 3;
const CORE_PROJECTION_RELATIVE_PATH: &str = "crates/merman-core/src/generated/mermaid_reference.rs";
const XTASK_PROJECTION_RELATIVE_PATH: &str = "crates/xtask/src/generated/mermaid_reference.rs";
const TYPESCRIPT_PROJECTION_RELATIVE_PATH: &str = "playground/src/generated/mermaid-reference.ts";
const UPSTREAM_SVG_PROVENANCE_RELATIVE_PATH: &str = "fixtures/upstream-svgs";
const OFFICIAL_NPM_REGISTRY_PREFIX: &str = "https://registry.npmjs.org/";
const PUBLISH_ATTESTATION_PREDICATE: &str =
    "https://github.com/npm/attestation/tree/main/specs/publish/v0.1";
const SLSA_ATTESTATION_PREDICATE: &str = "https://slsa.dev/provenance/v1";
const ATTESTATION_ARTIFACT_SCHEMA_VERSION: u32 = 1;
const MAX_ATTESTATION_ARTIFACT_BYTES: u64 = 64 * 1024;
const MAX_ATTESTATION_PAYLOAD_BYTES: usize = 32 * 1024;
const ZENUML_ADMISSION_GATES: [&str; 9] = [
    "plugin-contract",
    "corpus",
    "semantic",
    "render",
    "strict-inline-artifact",
    "execution-isolation",
    "security",
    "dependency-isolation",
    "resource",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MermaidReferenceBundle {
    schema_version: u32,
    projection_schema_version: u32,
    release: PackageReference,
    parser: PackageReference,
    sanitizer: PackageReference,
    builtin_registry: BuiltinRegistryInventory,
    external_diagrams: Vec<ExternalDiagramReference>,
    external_layouts: Vec<PackageReference>,
    playground: WorkspaceGraph,
    reference_cli: ReferenceCli,
    install_policy: InstallPolicy,
    feature_decision: FeatureDecision,
    generated_outputs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageReference {
    id: String,
    role: String,
    package: String,
    version: String,
    declared_range: Option<String>,
    latest_stable: String,
    integrity: String,
    tarball_url: String,
    attestation_url: String,
    publish_git_head: Option<String>,
    source: SourceReference,
    required_surfaces: Vec<String>,
    installed_content_sha256: Option<String>,
    #[serde(default)]
    runtime_registration: Option<RuntimeRegistration>,
    #[serde(default)]
    publish_provenance: Option<PublishProvenance>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishProvenance {
    workflow: PublishWorkflow,
    attestation_artifact: AttestationArtifactReference,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishWorkflow {
    repository: String,
    path: String,
    r#ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttestationArtifactReference {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttestationArtifact {
    schema_version: u32,
    package: String,
    version: String,
    attestations: Vec<AttestationRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttestationRecord {
    predicate_type: String,
    bundle: JsonValue,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DsseEnvelope {
    payload: String,
    payload_type: String,
    signatures: Vec<DsseSignature>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DsseSignature {
    keyid: String,
    sig: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeRegistration {
    module_id: String,
    diagram_aliases: Vec<String>,
    layout_ids: Vec<String>,
    source_path: String,
    source_sha256: String,
}

/// A source-backed inventory of Mermaid registrations owned by the core package.
///
/// External packages have their own runtime registrations. Built-ins must be listed here so an
/// upstream upgrade cannot silently add, remove, or rename a family or default layout while the
/// local admission inventory remains self-consistent.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuiltinRegistryInventory {
    diagrams: RegistryInventory,
    layouts: RegistryInventory,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryInventory {
    source_path: String,
    source_sha256: String,
    ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceReference {
    repository: String,
    reference: String,
    commit: String,
    checkout_path: Option<String>,
    package_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExternalDiagramReference {
    id: String,
    plugin: PackageReference,
    behavior_source: BehaviorSource,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BehaviorSource {
    declared_range: String,
    workspace_range: String,
    oracle: PackageReference,
    candidate: PackageReference,
    selected_version: String,
    decision: String,
    admission_evidence: String,
    delta_owner: String,
    latest_stable: PackageReference,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReferenceCli {
    package: PackageReference,
    workspace: String,
    package_json_sha256: String,
    package_lock_sha256: String,
    config_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceGraph {
    workspace: String,
    package_json_sha256: String,
    package_lock_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallPolicy {
    registry: String,
    default_ignore_scripts: bool,
    allowed_actions: Vec<String>,
    known_install_scripts: Vec<InstallScript>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallScript {
    workspace: String,
    package: String,
    action: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FeatureDecision {
    add_cargo_feature: bool,
    reason: String,
    evidence: Vec<String>,
}

fn load_bundle(path: &Path) -> Result<MermaidReferenceBundle, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&text).map_err(XtaskError::from)
}

fn package_references(bundle: &MermaidReferenceBundle) -> Vec<&PackageReference> {
    let mut references = vec![
        &bundle.release,
        &bundle.parser,
        &bundle.sanitizer,
        &bundle.reference_cli.package,
    ];
    references.extend(bundle.external_layouts.iter());
    for diagram in &bundle.external_diagrams {
        references.push(&diagram.plugin);
        references.push(&diagram.behavior_source.oracle);
        references.push(&diagram.behavior_source.candidate);
        references.push(&diagram.behavior_source.latest_stable);
    }
    references
}

fn selected_behavior_source(behavior: &BehaviorSource) -> Option<&PackageReference> {
    [&behavior.oracle, &behavior.candidate]
        .into_iter()
        .find(|reference| reference.version == behavior.selected_version)
}

fn materialized_runtime_references(
    bundle: &MermaidReferenceBundle,
) -> Result<Vec<&PackageReference>, XtaskError> {
    let mut references = vec![
        &bundle.release,
        &bundle.parser,
        &bundle.sanitizer,
        &bundle.reference_cli.package,
    ];
    references.extend(bundle.external_layouts.iter());
    for diagram in &bundle.external_diagrams {
        let selected = selected_behavior_source(&diagram.behavior_source).ok_or_else(|| {
            XtaskError::MermaidReference(format!(
                "{} selectedVersion must identify a materialized behavior package",
                diagram.id
            ))
        })?;
        references.extend([&diagram.plugin, selected]);
    }

    let mut packages = BTreeMap::new();
    for reference in references {
        if let Some(previous) = packages.insert(reference.package.as_str(), reference)
            && previous.version != reference.version
        {
            return Err(XtaskError::MermaidReference(format!(
                "materialized runtime package {} has conflicting versions {} and {}",
                reference.package, previous.version, reference.version
            )));
        }
    }
    Ok(packages.into_values().collect())
}

fn validate_bundle(bundle: &MermaidReferenceBundle) -> Result<(), XtaskError> {
    let mut failures = Vec::new();
    if bundle.schema_version != EXPECTED_SCHEMA_VERSION {
        failures.push(format!(
            "schemaVersion must be {EXPECTED_SCHEMA_VERSION}, found {}",
            bundle.schema_version
        ));
    }
    if bundle.projection_schema_version != EXPECTED_SCHEMA_VERSION {
        failures.push(format!(
            "projectionSchemaVersion must be {EXPECTED_SCHEMA_VERSION}, found {}",
            bundle.projection_schema_version
        ));
    }
    if bundle.release.package != "mermaid" || bundle.release.role != "core" {
        failures.push("release must describe the Mermaid core package".to_string());
    }
    if bundle.parser.package != "@mermaid-js/parser" || bundle.parser.role != "parser" {
        failures.push("parser must describe @mermaid-js/parser".to_string());
    }
    if bundle.sanitizer.package != "dompurify" || bundle.sanitizer.role != "sanitizer" {
        failures.push("sanitizer must describe the shared DOMPurify package".to_string());
    }
    let required_sanitizer_surfaces =
        BTreeSet::from(["playground", "reference-cli", "rust-sanitizer"]);
    let sanitizer_surfaces = bundle
        .sanitizer
        .required_surfaces
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if !required_sanitizer_surfaces.is_subset(&sanitizer_surfaces) {
        failures.push(
            "sanitizer must cover the Playground, reference CLI, and Rust sanitizer surfaces"
                .to_string(),
        );
    }
    let Some(zenuml) = bundle
        .external_diagrams
        .iter()
        .find(|diagram| diagram.id == "zenuml")
    else {
        return Err(XtaskError::MermaidReference(
            "externalDiagrams must contain zenuml".to_string(),
        ));
    };
    if zenuml.plugin.package != "@mermaid-js/mermaid-zenuml"
        || zenuml.plugin.role != "external-diagram"
    {
        failures.push("ZenUML must own an external-diagram plugin reference".to_string());
    }
    let Some(selected_behavior) = selected_behavior_source(&zenuml.behavior_source) else {
        failures.push(
            "ZenUML selectedVersion must identify either the recorded oracle or candidate"
                .to_string(),
        );
        return Err(XtaskError::MermaidReference(failures.join("\n")));
    };
    let expected_decision = if selected_behavior.version == zenuml.behavior_source.candidate.version
    {
        "candidate-selected"
    } else {
        "oracle-retained"
    };
    if zenuml.behavior_source.decision != expected_decision
        || zenuml.behavior_source.declared_range.trim().is_empty()
        || zenuml.behavior_source.workspace_range.trim().is_empty()
        || zenuml.behavior_source.delta_owner.trim().is_empty()
    {
        failures.push("ZenUML behavior selection has incomplete decision evidence".to_string());
    }
    if zenuml.behavior_source.oracle.publish_provenance.is_none()
        || zenuml
            .behavior_source
            .candidate
            .publish_provenance
            .is_none()
    {
        failures.push(
            "ZenUML oracle and candidate must own their publish provenance descriptors".to_string(),
        );
    }
    if bundle.external_layouts.is_empty()
        || bundle
            .external_layouts
            .iter()
            .any(|reference| reference.role != "external-layout")
    {
        failures
            .push("externalLayouts must contain only owned external-layout packages".to_string());
    }
    for reference in materialized_runtime_references(bundle)? {
        if reference.installed_content_sha256.is_none() {
            failures.push(format!(
                "materialized runtime package {}@{} must record installedContentSha256",
                reference.package, reference.version
            ));
        }
    }
    validate_builtin_registry_inventory(&bundle.builtin_registry, &mut failures);
    validate_runtime_registrations(bundle, zenuml, &mut failures);
    if bundle.reference_cli.package.package != "@mermaid-js/mermaid-cli"
        || bundle.reference_cli.package.role != "reference-cli"
    {
        failures.push("referenceCli must describe @mermaid-js/mermaid-cli".to_string());
    }
    if bundle.install_policy.registry != OFFICIAL_NPM_REGISTRY_PREFIX
        || !bundle.install_policy.default_ignore_scripts
    {
        failures.push(
            "installPolicy must require the official npm registry and disabled lifecycle scripts"
                .to_string(),
        );
    }
    if bundle.feature_decision.add_cargo_feature {
        failures.push(
            "featureDecision must keep browser/reference-only companions out of Cargo features"
                .to_string(),
        );
    }
    if bundle.feature_decision.reason.trim().is_empty()
        || bundle.feature_decision.evidence.is_empty()
    {
        failures.push("featureDecision must retain measured evidence".to_string());
    }
    let expected_outputs = BTreeSet::from([
        CORE_PROJECTION_RELATIVE_PATH.to_string(),
        XTASK_PROJECTION_RELATIVE_PATH.to_string(),
        TYPESCRIPT_PROJECTION_RELATIVE_PATH.to_string(),
    ]);
    if bundle
        .generated_outputs
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        != expected_outputs
    {
        failures.push(
            "generatedOutputs must name the Rust core, xtask, and TypeScript projections"
                .to_string(),
        );
    }
    let mut identities = BTreeSet::new();
    let mut attestation_artifact_paths = BTreeSet::new();
    for reference in package_references(bundle) {
        if reference.id.trim().is_empty()
            || reference.role.trim().is_empty()
            || reference.latest_stable.trim().is_empty()
            || reference.version.trim().is_empty()
            || reference
                .declared_range
                .as_deref()
                .is_some_and(str::is_empty)
            || reference.required_surfaces.is_empty()
            || reference.source.commit.len() != 40
            || !reference.source.repository.starts_with("https://")
            || reference.source.reference.trim().is_empty()
            || reference
                .source
                .package_path
                .as_deref()
                .is_some_and(str::is_empty)
        {
            failures.push(format!(
                "{}@{} has incomplete identity/source provenance",
                reference.package, reference.version
            ));
        }
        if !reference.integrity.starts_with("sha512-") {
            failures.push(format!(
                "{}@{} must record sha512 npm integrity",
                reference.package, reference.version
            ));
        }
        if !reference
            .tarball_url
            .starts_with(OFFICIAL_NPM_REGISTRY_PREFIX)
            || !reference
                .attestation_url
                .starts_with("https://registry.npmjs.org/-/npm/v1/attestations/")
        {
            failures.push(format!(
                "{}@{} must record official npm tarball and attestation URLs",
                reference.package, reference.version
            ));
        }
        if reference
            .publish_git_head
            .as_deref()
            .is_some_and(|commit| commit.len() != 40 || commit != reference.source.commit)
        {
            failures.push(format!(
                "{}@{} publishGitHead must match its source commit",
                reference.package, reference.version
            ));
        }
        if reference
            .installed_content_sha256
            .as_deref()
            .is_some_and(|digest| !crate::util::is_canonical_sha256(digest))
        {
            failures.push(format!(
                "{}@{} has invalid installedContentSha256",
                reference.package, reference.version
            ));
        }
        if let Some(provenance) = &reference.publish_provenance {
            let expected_repository = reference.source.repository.trim_end_matches(".git");
            if provenance.workflow.repository != expected_repository
                || !provenance.workflow.repository.starts_with("https://")
                || !is_owned_relative_path(&provenance.workflow.path)
                || provenance.workflow.r#ref.trim().is_empty()
                || !is_owned_relative_path(&provenance.attestation_artifact.path)
                || !crate::util::is_canonical_sha256(&provenance.attestation_artifact.sha256)
                || provenance
                    .attestation_artifact
                    .sha256
                    .bytes()
                    .all(|byte| byte == b'0')
                || !attestation_artifact_paths.insert(provenance.attestation_artifact.path.as_str())
            {
                failures.push(format!(
                    "{}@{} has invalid publishProvenance ownership",
                    reference.package, reference.version
                ));
            }
        }
        if !identities.insert(reference.id.as_str()) {
            failures.push(format!("duplicate package reference id {}", reference.id));
        }
    }
    let known_scripts = &bundle.install_policy.known_install_scripts;
    if known_scripts.iter().any(|script| {
        script.workspace.trim().is_empty()
            || script.package.trim().is_empty()
            || script.action != "ignored"
            || script.reason.trim().is_empty()
    }) || !bundle.install_policy.allowed_actions.is_empty()
    {
        failures.push(
            "installPolicy must explicitly ignore known hooks and allow no implicit action"
                .to_string(),
        );
    }
    for digest in [
        &bundle.playground.package_json_sha256,
        &bundle.playground.package_lock_sha256,
        &bundle.reference_cli.package_json_sha256,
        &bundle.reference_cli.package_lock_sha256,
        &bundle.reference_cli.config_sha256,
    ] {
        if !crate::util::is_canonical_sha256(digest) {
            failures.push("workspace file digests must be canonical SHA-256".to_string());
        }
    }
    if bundle.reference_cli.workspace != "tools/mermaid-cli" {
        failures.push("referenceCli workspace must be tools/mermaid-cli".to_string());
    }
    if bundle.playground.workspace != "playground" {
        failures.push("playground workspace must be playground".to_string());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::MermaidReference(failures.join("\n")))
    }
}

fn validate_runtime_registrations(
    bundle: &MermaidReferenceBundle,
    zenuml: &ExternalDiagramReference,
    failures: &mut Vec<String>,
) {
    let mut module_ids = BTreeSet::new();
    let mut diagram_aliases = BTreeSet::new();
    let mut layout_ids = BTreeSet::new();
    let registrations = std::iter::once((&zenuml.plugin, true)).chain(
        bundle
            .external_layouts
            .iter()
            .map(|package| (package, false)),
    );
    for (package, is_diagram) in registrations {
        let Some(registration) = package.runtime_registration.as_ref() else {
            failures.push(format!(
                "{} must declare its runtimeRegistration inventory",
                package.package
            ));
            continue;
        };
        if registration.module_id.trim().is_empty()
            || !module_ids.insert(registration.module_id.as_str())
            || !crate::util::is_canonical_sha256(&registration.source_sha256)
            || !is_owned_relative_path(&registration.source_path)
        {
            failures.push(format!(
                "{} has an invalid runtimeRegistration identity/source",
                package.package
            ));
        }
        if !is_sorted_unique(&registration.diagram_aliases)
            || !is_sorted_unique(&registration.layout_ids)
        {
            failures.push(format!(
                "{} runtimeRegistration ids must be sorted and unique",
                package.package
            ));
        }
        if is_diagram {
            if registration.module_id != zenuml.id
                || registration.diagram_aliases.is_empty()
                || !registration.layout_ids.is_empty()
                || !registration
                    .diagram_aliases
                    .iter()
                    .any(|alias| alias == &zenuml.id)
            {
                failures.push(
                    "ZenUML runtimeRegistration must own its diagram aliases only".to_string(),
                );
            }
        } else if registration.layout_ids.is_empty() || !registration.diagram_aliases.is_empty() {
            failures.push(format!(
                "{} runtimeRegistration must own layout ids only",
                package.package
            ));
        }
        for alias in &registration.diagram_aliases {
            if alias.trim().is_empty() || !diagram_aliases.insert(alias.as_str()) {
                failures.push(format!(
                    "duplicate or empty external diagram alias {alias:?}"
                ));
            }
        }
        for layout_id in &registration.layout_ids {
            if layout_id.trim().is_empty() || !layout_ids.insert(layout_id.as_str()) {
                failures.push(format!(
                    "duplicate or empty external layout id {layout_id:?}"
                ));
            }
        }
    }
}

fn validate_builtin_registry_inventory(
    inventory: &BuiltinRegistryInventory,
    failures: &mut Vec<String>,
) {
    for (kind, registry) in [
        ("built-in diagram", &inventory.diagrams),
        ("built-in layout", &inventory.layouts),
    ] {
        if registry.ids.is_empty()
            || !is_owned_relative_path(&registry.source_path)
            || !registry.source_path.starts_with("packages/mermaid/")
            || !crate::util::is_canonical_sha256(&registry.source_sha256)
            || registry.source_sha256.bytes().all(|byte| byte == b'0')
        {
            failures.push(format!(
                "{kind} registry inventory has an invalid source or is empty"
            ));
        }

        let mut ids = BTreeSet::new();
        for id in &registry.ids {
            if id.trim().is_empty() || id.trim() != id || !ids.insert(id.as_str()) {
                failures.push(format!(
                    "{kind} registry inventory contains a duplicate, blank, or untrimmed id {id:?}"
                ));
            }
        }
    }
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn is_owned_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn rust_string(value: &str) -> Result<String, XtaskError> {
    serde_json::to_string(value).map_err(XtaskError::from)
}

fn render_core_projection(bundle: &MermaidReferenceBundle) -> Result<String, XtaskError> {
    let suffix = bundle.release.version.replace(['.', '-'], "_");
    Ok(format!(
        "// This file is @generated by `cargo run -p xtask -- gen-mermaid-reference`.\n\
         // Do not edit it directly; edit tools/upstreams/MERMAID_REFERENCE_BUNDLE.json.\n\n\
         /// Upstream Mermaid tag pinned by this repository.\n\
         pub const PINNED_MERMAID_BASELINE_TAG: &str = {};\n\n\
         /// Upstream Mermaid semver pinned by this repository.\n\
         pub const PINNED_MERMAID_BASELINE_VERSION: &str = {};\n\n\
         /// Upstream `@mermaid-js/mermaid-cli` semver used by compatibility surfaces.\n\
         pub const PINNED_MERMAID_CLI_VERSION: &str = {};\n\n\
         /// Filesystem/module-name-safe baseline version.\n\
         pub const PINNED_MERMAID_BASELINE_VERSION_SUFFIX: &str = {};\n",
        rust_string(&bundle.release.source.reference)?,
        rust_string(&bundle.release.version)?,
        rust_string(&bundle.reference_cli.package.version)?,
        rust_string(&suffix)?,
    ))
}

fn render_xtask_projection(bundle: &MermaidReferenceBundle) -> Result<String, XtaskError> {
    let mermaid_content = bundle
        .release
        .installed_content_sha256
        .as_deref()
        .ok_or_else(|| {
            XtaskError::MermaidReference("release installedContentSha256 is required".to_string())
        })?;
    let cli_content = bundle
        .reference_cli
        .package
        .installed_content_sha256
        .as_deref()
        .ok_or_else(|| {
            XtaskError::MermaidReference(
                "reference CLI installedContentSha256 is required".to_string(),
            )
        })?;
    let mut output = String::from(
        "// This file is @generated by `cargo run -p xtask -- gen-mermaid-reference`.\n\
         // Do not edit it directly; edit tools/upstreams/MERMAID_REFERENCE_BUNDLE.json.\n\n",
    );
    for (name, value) in [
        ("PINNED_MERMAID_PACKAGE_SHA256", mermaid_content),
        ("PINNED_MERMAID_CLI_PACKAGE_SHA256", cli_content),
        ("PINNED_MERMAID_VERSION", bundle.release.version.as_str()),
        (
            "PINNED_DOMPURIFY_VERSION",
            bundle.sanitizer.version.as_str(),
        ),
        (
            "PINNED_MERMAID_CLI_VERSION",
            bundle.reference_cli.package.version.as_str(),
        ),
        (
            "MERMAID_SOURCE_TAG",
            bundle.release.source.reference.as_str(),
        ),
        (
            "MERMAID_SOURCE_COMMIT",
            bundle.release.source.commit.as_str(),
        ),
        (
            "REFERENCE_CLI_PACKAGE_JSON_SHA256",
            bundle.reference_cli.package_json_sha256.as_str(),
        ),
        (
            "REFERENCE_CLI_PACKAGE_LOCK_SHA256",
            bundle.reference_cli.package_lock_sha256.as_str(),
        ),
        (
            "REFERENCE_CLI_CONFIG_SHA256",
            bundle.reference_cli.config_sha256.as_str(),
        ),
    ] {
        let value = rust_string(value)?;
        let line = format!("pub(crate) const {name}: &str = {value};");
        if line.len() <= 100 {
            writeln!(output, "{line}").expect("writing to a String cannot fail");
        } else {
            writeln!(output, "pub(crate) const {name}: &str =\n    {value};")
                .expect("writing to a String cannot fail");
        }
    }
    Ok(output)
}

fn render_typescript_projection(bundle: &MermaidReferenceBundle) -> Result<String, XtaskError> {
    let zenuml = bundle
        .external_diagrams
        .iter()
        .find(|diagram| diagram.id == "zenuml")
        .ok_or_else(|| XtaskError::MermaidReference("missing ZenUML reference".to_string()))?;
    let selected = selected_behavior_source(&zenuml.behavior_source)
        .ok_or_else(|| XtaskError::MermaidReference("invalid ZenUML selection".to_string()))?;
    let elk = bundle
        .external_layouts
        .iter()
        .find(|reference| reference.id == "layout-elk")
        .ok_or_else(|| XtaskError::MermaidReference("missing ELK layout reference".to_string()))?;
    let tidy_tree = bundle
        .external_layouts
        .iter()
        .find(|reference| reference.id == "layout-tidy-tree")
        .ok_or_else(|| {
            XtaskError::MermaidReference("missing tidy-tree layout reference".to_string())
        })?;

    let mut output = String::from(
        "// This file is @generated by `cargo run -p xtask -- gen-mermaid-reference`.\n\
         // Do not edit it directly; edit tools/upstreams/MERMAID_REFERENCE_BUNDLE.json.\n\n",
    );
    writeln!(
        output,
        "export const MERMAID_REFERENCE_BUNDLE_SCHEMA_VERSION = {} as const;",
        bundle.schema_version
    )
    .expect("writing to a String cannot fail");
    for (name, value) in [
        ("MERMAID_JS_VERSION", bundle.release.version.as_str()),
        ("MERMAID_PARSER_VERSION", bundle.parser.version.as_str()),
        ("MERMAID_ZENUML_VERSION", zenuml.plugin.version.as_str()),
        ("ZENUML_CORE_VERSION", selected.version.as_str()),
        ("MERMAID_LAYOUT_ELK_VERSION", elk.version.as_str()),
        (
            "MERMAID_LAYOUT_TIDY_TREE_VERSION",
            tidy_tree.version.as_str(),
        ),
        (
            "MERMAID_REFERENCE_CLI_VERSION",
            bundle.reference_cli.package.version.as_str(),
        ),
    ] {
        writeln!(
            output,
            "export const {name} = {} as const;",
            rust_string(value)?
        )
        .expect("writing to a String cannot fail");
    }
    writeln!(
        output,
        "export const MERMAID_SOURCE_COMMIT =\n  {} as const;",
        rust_string(&bundle.release.source.commit)?
    )
    .expect("writing to a String cannot fail");
    let diagram_registration =
        zenuml.plugin.runtime_registration.as_ref().ok_or_else(|| {
            XtaskError::MermaidReference("missing ZenUML registration".to_string())
        })?;
    let mut external_diagram_modules = vec![diagram_registration.module_id.as_str()];
    external_diagram_modules.sort_unstable();
    let mut external_layout_modules = bundle
        .external_layouts
        .iter()
        .filter_map(|package| package.runtime_registration.as_ref())
        .map(|registration| registration.module_id.as_str())
        .collect::<Vec<_>>();
    external_layout_modules.sort_unstable();
    let diagram_alias_to_module = diagram_registration
        .diagram_aliases
        .iter()
        .map(|alias| (alias.as_str(), diagram_registration.module_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let layout_id_to_module = bundle
        .external_layouts
        .iter()
        .filter_map(|package| package.runtime_registration.as_ref())
        .flat_map(|registration| {
            registration
                .layout_ids
                .iter()
                .map(move |layout| (layout.as_str(), registration.module_id.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    for (name, value) in [
        (
            "MERMAID_EXTERNAL_DIAGRAM_MODULE_IDS",
            serde_json::to_string_pretty(&external_diagram_modules)?,
        ),
        (
            "MERMAID_LAYOUT_MODULE_IDS",
            serde_json::to_string_pretty(&external_layout_modules)?,
        ),
        (
            "MERMAID_EXTERNAL_DIAGRAM_ALIAS_TO_MODULE",
            serde_json::to_string_pretty(&diagram_alias_to_module)?,
        ),
        (
            "MERMAID_LAYOUT_ID_TO_MODULE",
            serde_json::to_string_pretty(&layout_id_to_module)?,
        ),
    ] {
        writeln!(output, "export const {name} = {value} as const;")
            .expect("writing to a String cannot fail");
    }
    writeln!(
        output,
        "export type MermaidExternalDiagramModuleId =\n  (typeof MERMAID_EXTERNAL_DIAGRAM_MODULE_IDS)[number];\n\
         export type MermaidLayoutModuleId =\n  (typeof MERMAID_LAYOUT_MODULE_IDS)[number];"
    )
    .expect("writing to a String cannot fail");
    Ok(output)
}

fn expected_projections(
    bundle: &MermaidReferenceBundle,
) -> Result<BTreeMap<&'static str, String>, XtaskError> {
    Ok(BTreeMap::from([
        (
            CORE_PROJECTION_RELATIVE_PATH,
            render_core_projection(bundle)?,
        ),
        (
            XTASK_PROJECTION_RELATIVE_PATH,
            render_xtask_projection(bundle)?,
        ),
        (
            TYPESCRIPT_PROJECTION_RELATIVE_PATH,
            render_typescript_projection(bundle)?,
        ),
    ]))
}

fn write_projection(path: &Path, contents: &str) -> Result<(), XtaskError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, contents).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn read_json(path: &Path) -> Result<JsonValue, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&text).map_err(XtaskError::from)
}

fn file_sha256(path: &Path) -> Result<String, XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    Ok(crate::util::sha256_hex(&bytes))
}

fn verify_reference_cli_files(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    for (relative, expected) in [
        (
            "package.json",
            bundle.reference_cli.package_json_sha256.as_str(),
        ),
        (
            "package-lock.json",
            bundle.reference_cli.package_lock_sha256.as_str(),
        ),
        (
            "mermaid-config.json",
            bundle.reference_cli.config_sha256.as_str(),
        ),
    ] {
        let path = root.join(&bundle.reference_cli.workspace).join(relative);
        let actual = file_sha256(&path)?;
        if actual != expected {
            failures.push(format!(
                "{} digest drift: expected {expected}, found {actual}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn verify_playground_files(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    for (relative, expected) in [
        (
            "package.json",
            bundle.playground.package_json_sha256.as_str(),
        ),
        (
            "package-lock.json",
            bundle.playground.package_lock_sha256.as_str(),
        ),
    ] {
        let path = root.join(&bundle.playground.workspace).join(relative);
        let actual = file_sha256(&path)?;
        if actual != expected {
            failures.push(format!(
                "{} digest drift: expected {expected}, found {actual}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn expected_provenance_source(bundle: &MermaidReferenceBundle) -> BTreeMap<&'static str, &str> {
    BTreeMap::from([
        ("mermaid_version", bundle.release.version.as_str()),
        (
            "mermaid_cli_version",
            bundle.reference_cli.package.version.as_str(),
        ),
        (
            "mermaid_source_tag",
            bundle.release.source.reference.as_str(),
        ),
        (
            "mermaid_source_commit",
            bundle.release.source.commit.as_str(),
        ),
        (
            "package_json_sha256",
            bundle.reference_cli.package_json_sha256.as_str(),
        ),
        (
            "package_lock_sha256",
            bundle.reference_cli.package_lock_sha256.as_str(),
        ),
        (
            "mermaid_config_sha256",
            bundle.reference_cli.config_sha256.as_str(),
        ),
    ])
}

fn upstream_provenance_manifests(root: &Path) -> Result<Vec<PathBuf>, XtaskError> {
    upstream_provenance_manifests_for(root, crate::cmd::UPSTREAM_SVG_DIAGRAMS)
}

fn upstream_provenance_manifests_for(
    root: &Path,
    primary_families: &[&str],
) -> Result<Vec<PathBuf>, XtaskError> {
    let provenance_root = root.join(UPSTREAM_SVG_PROVENANCE_RELATIVE_PATH);
    let manifests = primary_families
        .iter()
        .map(|family| provenance_root.join(family).join("_baseline-manifest.json"))
        .collect::<Vec<_>>();
    let missing = primary_families
        .iter()
        .zip(&manifests)
        .filter(|(_, manifest)| !manifest.is_file())
        .map(|(family, manifest)| format!("{family}: {}", manifest.display()))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(XtaskError::MermaidReference(format!(
            "missing primary upstream SVG provenance manifests: {}",
            missing.join(", ")
        )));
    }
    if manifests.is_empty() {
        return Err(XtaskError::MermaidReference(
            "primary upstream SVG family inventory is empty".to_string(),
        ));
    }
    Ok(manifests)
}

fn regenerate_upstream_provenance_with<F>(regenerate: F) -> Result<(), XtaskError>
where
    F: FnOnce(Vec<String>) -> Result<(), XtaskError>,
{
    regenerate(vec!["--diagram".to_string(), "all".to_string()])
}

fn regenerate_upstream_provenance() -> Result<(), XtaskError> {
    regenerate_upstream_provenance_with(crate::cmd::gen_upstream_svgs)
}

fn verify_upstream_provenance_sources(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let expected = expected_provenance_source(bundle);
    for manifest_path in upstream_provenance_manifests(root)? {
        let manifest = read_json(&manifest_path)?;
        let source = manifest.get("source");
        for (field, expected) in &expected {
            if source
                .and_then(|source| source.get(field))
                .and_then(JsonValue::as_str)
                != Some(*expected)
            {
                failures.push(format!(
                    "{} source.{field} does not match the reference bundle",
                    manifest_path.display()
                ));
            }
        }
    }
    Ok(())
}

fn verify_source_checkouts(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    materialized: bool,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let lock = read_json(&root.join(REPOS_LOCK_RELATIVE_PATH))?;
    let repos = lock
        .get("repos")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| {
            XtaskError::MermaidReference("REPOS.lock.json has no repos object".to_string())
        })?;
    let mut seen = BTreeSet::new();
    for reference in package_references(bundle) {
        let Some(checkout_path) = reference.source.checkout_path.as_deref() else {
            continue;
        };
        let matching = repos
            .values()
            .find(|repo| repo.get("path").and_then(JsonValue::as_str) == Some(checkout_path));
        let Some(repo) = matching else {
            failures.push(format!(
                "source checkout {checkout_path} is absent from REPOS.lock.json"
            ));
            continue;
        };
        for (field, expected) in [
            ("url", reference.source.repository.as_str()),
            ("ref", reference.source.reference.as_str()),
            ("commit", reference.source.commit.as_str()),
        ] {
            if repo.get(field).and_then(JsonValue::as_str) != Some(expected) {
                failures.push(format!(
                    "source checkout {checkout_path} {field} does not match the reference bundle"
                ));
            }
        }
        if materialized && let Some(registration) = &reference.runtime_registration {
            let source_path = root.join(checkout_path).join(&registration.source_path);
            if !source_path.is_file() {
                failures.push(format!(
                    "runtime registration source is missing: {}",
                    source_path.display()
                ));
            } else {
                let actual = file_sha256(&source_path)?;
                if actual != registration.source_sha256 {
                    failures.push(format!(
                        "runtime registration source digest drift for {}: expected {}, found {actual}",
                        source_path.display(),
                        registration.source_sha256
                    ));
                }
            }
        }
        if !materialized || !seen.insert(checkout_path) {
            continue;
        }
        let checkout = root.join(checkout_path);
        if !checkout.is_dir() {
            failures.push(format!("missing reference checkout {}", checkout.display()));
            continue;
        }
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&checkout)
            .output()
            .map_err(|source| XtaskError::ReadFile {
                path: checkout.display().to_string(),
                source,
            })?;
        let actual = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !output.status.success() || actual != reference.source.commit {
            failures.push(format!(
                "reference checkout {} must be at {}, found {actual}",
                checkout.display(),
                reference.source.commit
            ));
        }
        if let Some(package_path) = &reference.source.package_path
            && !checkout.join(package_path).exists()
        {
            failures.push(format!(
                "source package path is missing: {}",
                checkout.join(package_path).display()
            ));
        }
    }
    Ok(())
}

fn verify_builtin_registry_inventory(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    materialized: bool,
    failures: &mut Vec<String>,
) {
    if !materialized {
        return;
    }

    let Some(checkout_path) = bundle.release.source.checkout_path.as_deref() else {
        failures.push(
            "Mermaid release must materialize a source checkout for registry inventory".to_string(),
        );
        return;
    };
    let checkout = root.join(checkout_path);
    if !checkout.is_dir() {
        // `verify_source_checkouts` reports the missing checkout with the pinned source identity.
        return;
    }

    for (kind, registry) in [
        ("built-in diagram", &bundle.builtin_registry.diagrams),
        ("built-in layout", &bundle.builtin_registry.layouts),
    ] {
        let source_path = checkout.join(&registry.source_path);
        if !source_path.is_file() {
            failures.push(format!(
                "{kind} registry source is missing: {}",
                source_path.display()
            ));
            continue;
        }
        let source = match fs::read_to_string(&source_path) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!(
                    "cannot read {kind} registry source {}: {error}",
                    source_path.display()
                ));
                continue;
            }
        };
        let actual_sha256 = crate::util::sha256_hex(source.as_bytes());
        if actual_sha256 != registry.source_sha256 {
            failures.push(format!(
                "{kind} registry source digest drift for {}: expected {}, found {actual_sha256}",
                source_path.display(),
                registry.source_sha256
            ));
        }

        let actual_ids = if kind == "built-in diagram" {
            extract_builtin_diagram_ids(
                &source,
                Path::new(&registry.source_path),
                |relative_source_path| {
                    let path = checkout.join(relative_source_path);
                    fs::read_to_string(&path)
                        .map_err(|error| format!("{}: {error}", path.display()))
                },
            )
        } else {
            extract_builtin_layout_ids(&source)
        };
        match actual_ids {
            Ok(actual_ids) if actual_ids != registry.ids => failures.push(format!(
                "{kind} registry inventory drift at {}: expected {:?}, found {actual_ids:?}",
                source_path.display(),
                registry.ids
            )),
            Ok(_) => {}
            Err(reason) => failures.push(format!(
                "cannot extract {kind} registry inventory from {}: {reason}",
                source_path.display()
            )),
        }
    }
}

fn extract_builtin_diagram_ids(
    orchestration_source: &str,
    orchestration_path: &Path,
    mut read_detector_source: impl FnMut(&Path) -> Result<String, String>,
) -> Result<Vec<String>, String> {
    let imports = diagram_detector_imports(orchestration_source, orchestration_path)?;
    let direct_re =
        Regex::new(r#"(?s)registerDiagram\s*\(\s*(?:\"(?P<double>[^\"]+)\"|'(?P<single>[^']+)')"#)
            .expect("built-in direct registry regex is valid");
    let lazy_re = Regex::new(r"(?s)registerLazyLoadedDiagrams\s*\((?P<args>.*?)\)\s*;")
        .expect("built-in lazy registry regex is valid");
    let direct_call_re =
        Regex::new(r"registerDiagram\s*\(").expect("direct registry call regex is valid");
    let lazy_call_re =
        Regex::new(r"registerLazyLoadedDiagrams\s*\(").expect("lazy registry call regex is valid");
    let mut registrations = Vec::new();
    let mut direct_registration_count = 0usize;
    let mut lazy_registration_count = 0usize;

    for captures in direct_re.captures_iter(orchestration_source) {
        direct_registration_count += 1;
        let id = captures
            .name("double")
            .or_else(|| captures.name("single"))
            .expect("static direct diagram id is captured")
            .as_str()
            .to_string();
        registrations.push((
            captures
                .get(0)
                .expect("direct registration is captured")
                .start(),
            vec![id],
        ));
    }
    for captures in lazy_re.captures_iter(orchestration_source) {
        lazy_registration_count += 1;
        let arguments = captures
            .name("args")
            .expect("lazy registration arguments are captured")
            .as_str();
        let mut ids = Vec::new();
        for binding in arguments
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if !is_typescript_identifier(binding) {
                return Err(format!(
                    "lazy diagram registration argument {binding:?} is not a static imported identifier"
                ));
            }
            let source_path = imports.get(binding).ok_or_else(|| {
                format!("lazy diagram registration {binding:?} has no supported static import")
            })?;
            let detector_source = read_detector_source(source_path)?;
            ids.push(detector_id_from_source(&detector_source, source_path)?);
        }
        registrations.push((
            captures
                .get(0)
                .expect("lazy registration is captured")
                .start(),
            ids,
        ));
    }
    if direct_registration_count != direct_call_re.find_iter(orchestration_source).count() {
        return Err("a registerDiagram call does not use a static string id".to_string());
    }
    if lazy_registration_count != lazy_call_re.find_iter(orchestration_source).count() {
        return Err("a registerLazyLoadedDiagrams call has unsupported syntax".to_string());
    }
    registrations.sort_by_key(|(offset, _)| *offset);
    let ids = registrations
        .into_iter()
        .flat_map(|(_, ids)| ids)
        .collect::<Vec<_>>();

    ensure_unique_registry_ids(&ids, "diagram")?;
    if ids.is_empty() {
        return Err(
            "no registerDiagram or registerLazyLoadedDiagrams calls were found".to_string(),
        );
    }
    Ok(ids)
}

fn diagram_detector_imports(
    source: &str,
    orchestration_path: &Path,
) -> Result<BTreeMap<String, PathBuf>, String> {
    let import_re = Regex::new(
        r#"(?m)^import\s+(?:(?P<default>[A-Za-z_$][A-Za-z0-9_$]*)|\{\s*(?P<named>[A-Za-z_$][A-Za-z0-9_$]*)\s*\})\s+from\s+['\"](?P<path>\.[^'\"]+)['\"];\s*$"#,
    )
    .expect("built-in registry import regex is valid");
    let mut imports = BTreeMap::new();
    for captures in import_re.captures_iter(source) {
        let binding = captures
            .name("default")
            .or_else(|| captures.name("named"))
            .expect("supported import captures one binding")
            .as_str();
        let import_path = captures
            .name("path")
            .expect("supported import captures a path")
            .as_str();
        if !import_path.starts_with("../diagrams/") {
            continue;
        }
        let source_path = resolve_typescript_import(orchestration_path, import_path)?;
        if imports.insert(binding.to_string(), source_path).is_some() {
            return Err(format!("duplicate detector import binding {binding:?}"));
        }
    }
    Ok(imports)
}

fn resolve_typescript_import(base: &Path, import_path: &str) -> Result<PathBuf, String> {
    if !import_path.starts_with("../diagrams/") {
        return Err(format!(
            "detector import {import_path:?} is outside Mermaid's built-in diagram tree"
        ));
    }
    let parent = base
        .parent()
        .ok_or_else(|| format!("registry source path {} has no parent", base.display()))?;
    let mut normalized = PathBuf::new();
    for component in parent.join(import_path).components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "detector import {import_path:?} escapes its checkout"
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "detector import {import_path:?} is not a relative path"
                ));
            }
        }
    }
    if normalized
        .extension()
        .is_some_and(|extension| extension == "js")
    {
        normalized.set_extension("ts");
    }
    Ok(normalized)
}

fn is_typescript_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some('_' | '$' | 'a'..='z' | 'A'..='Z'))
        && characters
            .all(|character| matches!(character, '_' | '$' | 'a'..='z' | 'A'..='Z' | '0'..='9'))
}

fn detector_id_from_source(source: &str, source_path: &Path) -> Result<String, String> {
    let id_re = Regex::new(
        r#"(?m)^\s*(?:export\s+)?const\s+id\s*=\s*(?:\"(?P<double>[^\"]+)\"|'(?P<single>[^']+)')\s*;"#,
    )
    .expect("detector id regex is valid");
    let mut ids = id_re.captures_iter(source).filter_map(|captures| {
        captures
            .name("double")
            .or_else(|| captures.name("single"))
            .map(|id| id.as_str().to_string())
    });
    let id = ids.next().ok_or_else(|| {
        format!(
            "detector {} does not declare a static `const id = ...`",
            source_path.display()
        )
    })?;
    if ids.next().is_some() {
        return Err(format!(
            "detector {} declares more than one static `id`",
            source_path.display()
        ));
    }
    Ok(id)
}

fn extract_builtin_layout_ids(source: &str) -> Result<Vec<String>, String> {
    let start = source
        .find("const registerDefaultLayoutLoaders")
        .ok_or_else(|| "missing registerDefaultLayoutLoaders declaration".to_string())?;
    let tail = &source[start..];
    let end = tail
        .find("registerDefaultLayoutLoaders();")
        .ok_or_else(|| "missing registerDefaultLayoutLoaders invocation".to_string())?;
    let registry = &tail[..end];
    let name_re =
        Regex::new(r#"(?m)^\s*name\s*:\s*(?:\"(?P<double>[^\"]+)\"|'(?P<single>[^']+)')\s*,?\s*$"#)
            .expect("layout name regex is valid");
    let name_property_re =
        Regex::new(r"(?m)^\s*name\s*:").expect("layout name property regex is valid");
    let ids = name_re
        .captures_iter(registry)
        .filter_map(|captures| {
            captures
                .name("double")
                .or_else(|| captures.name("single"))
                .map(|id| id.as_str().to_string())
        })
        .collect::<Vec<_>>();
    if ids.len() != name_property_re.find_iter(registry).count() {
        return Err("a default layout name is not a static string literal".to_string());
    }
    ensure_unique_registry_ids(&ids, "layout")?;
    if ids.is_empty() {
        return Err("no static layout names were found".to_string());
    }
    Ok(ids)
}

fn ensure_unique_registry_ids(ids: &[String], kind: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.trim().is_empty() || id.trim() != id || !seen.insert(id.as_str()) {
            return Err(format!(
                "{kind} registry has a duplicate, blank, or untrimmed id {id:?}"
            ));
        }
    }
    Ok(())
}

fn package_lock_entries<'a>(lock: &'a JsonValue, package: &str) -> Vec<(&'a str, &'a JsonValue)> {
    let marker = format!("node_modules/{package}");
    lock.get("packages")
        .and_then(JsonValue::as_object)
        .into_iter()
        .flat_map(|packages| packages.iter())
        .filter_map(|(path, value)| {
            (path == &marker || path.ends_with(&format!("/{marker}")))
                .then_some((path.as_str(), value))
        })
        .collect()
}

fn verify_lock_registry_sources(lock: &JsonValue, workspace: &str, failures: &mut Vec<String>) {
    for (path, package) in lock
        .get("packages")
        .and_then(JsonValue::as_object)
        .into_iter()
        .flat_map(|packages| packages.iter())
    {
        let Some(resolved) = package.get("resolved").and_then(JsonValue::as_str) else {
            continue;
        };
        let is_http_source = resolved.starts_with("https://") || resolved.starts_with("http://");
        if is_http_source && !resolved.starts_with(OFFICIAL_NPM_REGISTRY_PREFIX) {
            failures.push(format!(
                "{workspace}/{path} resolves an HTTP package outside the official registry: {resolved}"
            ));
        }
    }
}

fn verify_manifest_requirement(
    manifest: &JsonValue,
    workspace: &str,
    section: &str,
    package: &PackageReference,
    failures: &mut Vec<String>,
) {
    let actual = manifest
        .get(section)
        .and_then(|value| value.get(&package.package))
        .and_then(JsonValue::as_str);
    if actual != Some(package.version.as_str()) {
        failures.push(format!(
            "{workspace}/package.json {section}.{} must be {}, found {:?}",
            package.package, package.version, actual
        ));
    }
}

fn verify_manifest_overrides(
    manifest: &JsonValue,
    workspace: &str,
    required_overrides: &[&PackageReference],
    failures: &mut Vec<String>,
) {
    for package in required_overrides {
        let actual = manifest
            .get("overrides")
            .and_then(|value| value.get(&package.package))
            .and_then(JsonValue::as_str);
        if actual != Some(package.version.as_str()) {
            failures.push(format!(
                "{workspace}/package.json must override {} to {}, found {:?}",
                package.package, package.version, actual
            ));
        }
    }
}

struct WorkspaceGraphExpectation<'a> {
    workspace: &'a str,
    direct_dependencies: Vec<&'a PackageReference>,
    expected_packages: Vec<&'a PackageReference>,
    required_overrides: Vec<&'a PackageReference>,
}

fn workspace_graph_expectations<'a>(
    bundle: &'a MermaidReferenceBundle,
    zenuml: &'a ExternalDiagramReference,
    selected_behavior: &'a PackageReference,
) -> Vec<WorkspaceGraphExpectation<'a>> {
    let layout_refs = bundle.external_layouts.iter().collect::<Vec<_>>();

    let mut playground_direct = vec![&bundle.release, &zenuml.plugin];
    playground_direct.extend(layout_refs.iter().copied());
    let mut playground_expected = vec![
        &bundle.release,
        &bundle.parser,
        &zenuml.plugin,
        selected_behavior,
        &bundle.sanitizer,
    ];
    playground_expected.extend(layout_refs.iter().copied());

    let mut cli_direct = vec![&bundle.reference_cli.package, &zenuml.plugin];
    cli_direct.extend(layout_refs.iter().copied());
    let mut cli_expected = vec![
        &bundle.reference_cli.package,
        &bundle.release,
        &bundle.parser,
        &zenuml.plugin,
        selected_behavior,
        &bundle.sanitizer,
    ];
    cli_expected.extend(layout_refs.iter().copied());

    vec![
        WorkspaceGraphExpectation {
            workspace: "playground",
            direct_dependencies: playground_direct,
            expected_packages: playground_expected,
            required_overrides: vec![selected_behavior, &bundle.sanitizer],
        },
        WorkspaceGraphExpectation {
            workspace: &bundle.reference_cli.workspace,
            direct_dependencies: cli_direct,
            expected_packages: cli_expected,
            required_overrides: vec![selected_behavior, &bundle.sanitizer],
        },
    ]
}

fn verify_workspace_graph(
    root: &Path,
    expectation: &WorkspaceGraphExpectation<'_>,
    bundle: &MermaidReferenceBundle,
    materialized: bool,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let workspace = expectation.workspace;
    let workspace_root = root.join(workspace);
    let manifest = read_json(&workspace_root.join("package.json"))?;
    let direct_section = if workspace == "playground" {
        "dependencies"
    } else {
        "devDependencies"
    };
    for package in &expectation.direct_dependencies {
        verify_manifest_requirement(&manifest, workspace, direct_section, package, failures);
    }
    verify_manifest_overrides(
        &manifest,
        workspace,
        &expectation.required_overrides,
        failures,
    );
    let npmrc = fs::read_to_string(workspace_root.join(".npmrc")).map_err(|source| {
        XtaskError::ReadFile {
            path: workspace_root.join(".npmrc").display().to_string(),
            source,
        }
    })?;
    let npmrc_settings = npmrc.lines().map(str::trim).collect::<BTreeSet<_>>();
    if npmrc_settings
        != BTreeSet::from([
            "ignore-scripts=true",
            "registry=https://registry.npmjs.org/",
        ])
    {
        failures.push(format!(
            "{workspace}/.npmrc must require the official registry and ignore scripts"
        ));
    }

    let lock = read_json(&workspace_root.join("package-lock.json"))?;
    verify_lock_registry_sources(&lock, workspace, failures);
    for package in &expectation.expected_packages {
        let entries = package_lock_entries(&lock, &package.package);
        if entries.is_empty() {
            failures.push(format!(
                "{workspace}/package-lock.json does not resolve {}",
                package.package
            ));
            continue;
        }
        for (lock_path, entry) in entries {
            let version = entry.get("version").and_then(JsonValue::as_str);
            let integrity = entry.get("integrity").and_then(JsonValue::as_str);
            let resolved = entry.get("resolved").and_then(JsonValue::as_str);
            if version != Some(package.version.as_str())
                || integrity != Some(package.integrity.as_str())
            {
                failures.push(format!(
                    "{workspace}/{lock_path} must resolve {}@{} with recorded integrity",
                    package.package, package.version
                ));
            }
            if resolved != Some(package.tarball_url.as_str()) {
                failures.push(format!(
                    "{workspace}/{lock_path} must resolve {} from its recorded npm tarball",
                    package.package
                ));
            }
            if materialized {
                let installed_manifest = workspace_root.join(lock_path).join("package.json");
                let installed = read_json(&installed_manifest)?;
                if installed.get("version").and_then(JsonValue::as_str)
                    != Some(package.version.as_str())
                {
                    failures.push(format!(
                        "installed package {} does not match the lock/reference bundle",
                        installed_manifest.display()
                    ));
                }
            }
        }
    }

    let actual_scripts = lock
        .get("packages")
        .and_then(JsonValue::as_object)
        .into_iter()
        .flat_map(|packages| packages.iter())
        .filter(|(_, package)| {
            package.get("hasInstallScript").and_then(JsonValue::as_bool) == Some(true)
        })
        .filter_map(|(path, package)| {
            let name = path.rsplit("node_modules/").next()?;
            let version = package.get("version")?.as_str()?;
            Some(format!("{workspace}:{name}@{version}"))
        })
        .collect::<BTreeSet<_>>();
    let expected_scripts = bundle
        .install_policy
        .known_install_scripts
        .iter()
        .filter(|script| script.workspace == workspace)
        .map(|script| format!("{}:{}", script.workspace, script.package))
        .collect::<BTreeSet<_>>();
    if actual_scripts != expected_scripts {
        failures.push(format!(
            "{workspace} install-script inventory drift: expected {expected_scripts:?}, found {actual_scripts:?}"
        ));
    }
    Ok(())
}

fn verify_admission(
    root: &Path,
    mermaid_release: &PackageReference,
    plugin: &PackageReference,
    behavior: &BehaviorSource,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let evidence = read_json(&root.join(&behavior.admission_evidence))?;
    let expected_plugin = format!("{}@{}", plugin.package, plugin.version);
    if evidence.get("mermaidRelease").and_then(JsonValue::as_str)
        != Some(mermaid_release.version.as_str())
        || evidence.get("plugin").and_then(JsonValue::as_str) != Some(expected_plugin.as_str())
    {
        failures
            .push("ZenUML admission host graph does not match the reference bundle".to_string());
    }
    if evidence.get("decision").and_then(JsonValue::as_str) != Some(behavior.decision.as_str()) {
        failures.push("ZenUML admission decision does not match the reference bundle".to_string());
    }
    validate_candidate_status(&evidence, &behavior.decision, failures);
    for (field, reference) in [
        ("oracle", &behavior.oracle),
        ("candidate", &behavior.candidate),
    ] {
        let value = evidence.get(field);
        if value
            .and_then(|value| value.get("version"))
            .and_then(JsonValue::as_str)
            != Some(reference.version.as_str())
            || value
                .and_then(|value| value.get("commit"))
                .and_then(JsonValue::as_str)
                != Some(reference.source.commit.as_str())
        {
            failures.push(format!("ZenUML admission {field} provenance drift"));
        }
    }
    validate_admission_matrix(&evidence, &behavior.decision, failures);
    validate_admission_deltas(&evidence, failures);
    verify_deferred_major_admission(root, mermaid_release, plugin, behavior, &evidence, failures)?;
    verify_candidate_evidence(root, behavior, &evidence, failures)?;
    Ok(())
}

fn verify_deferred_major_admission(
    root: &Path,
    mermaid_release: &PackageReference,
    plugin: &PackageReference,
    behavior: &BehaviorSource,
    admission: &JsonValue,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let excluded = admission.get("excludedLatestMajor");
    let artifact_path = excluded
        .and_then(|value| value.get("artifact"))
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let expected_sha256 = excluded
        .and_then(|value| value.get("artifactSha256"))
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    if !is_owned_relative_path(artifact_path) || !crate::util::is_canonical_sha256(expected_sha256)
    {
        failures.push(
            "ZenUML latest stable major must name a digest-bound separate admission artifact"
                .to_string(),
        );
        return Ok(());
    }

    let artifact_file = root.join(artifact_path);
    let artifact_text =
        fs::read_to_string(&artifact_file).map_err(|source| XtaskError::ReadFile {
            path: artifact_file.display().to_string(),
            source,
        })?;
    if crate::util::sha256_hex(artifact_text.as_bytes()) != expected_sha256 {
        failures.push("ZenUML deferred-major admission artifact digest drift".to_string());
    }
    let artifact: JsonValue = serde_json::from_str(&artifact_text)?;
    validate_deferred_major_admission(
        mermaid_release,
        plugin,
        behavior,
        admission,
        &artifact,
        failures,
    );
    Ok(())
}

fn validate_deferred_major_admission(
    mermaid_release: &PackageReference,
    plugin: &PackageReference,
    behavior: &BehaviorSource,
    admission: &JsonValue,
    artifact: &JsonValue,
    failures: &mut Vec<String>,
) {
    let excluded = admission.get("excludedLatestMajor");
    let latest = &behavior.latest_stable;
    if excluded
        .and_then(|value| value.get("version"))
        .and_then(JsonValue::as_str)
        != Some(latest.version.as_str())
        || excluded
            .and_then(|value| value.get("commit"))
            .and_then(JsonValue::as_str)
            != Some(latest.source.commit.as_str())
        || excluded
            .and_then(|value| value.get("reason"))
            .and_then(JsonValue::as_str)
            .is_none_or(|value| value.trim().is_empty())
        || excluded
            .and_then(|value| value.get("nextAdmission"))
            .and_then(JsonValue::as_str)
            != Some("separate-major-zenuml-v4")
    {
        failures.push(
            "ZenUML latest stable major is not bound to the separate admission decision"
                .to_string(),
        );
    }

    if artifact.get("schemaVersion").and_then(JsonValue::as_u64) != Some(1)
        || artifact.get("artifactKind").and_then(JsonValue::as_str)
            != Some("zenuml-core-major-admission")
        || artifact.get("status").and_then(JsonValue::as_str) != Some("deferred")
        || artifact.get("decision").and_then(JsonValue::as_str)
            != Some("deferred-incompatible-major")
        || artifact.get("generatedFrom").and_then(JsonValue::as_str) != Some(BUNDLE_RELATIVE_PATH)
    {
        failures.push(
            "ZenUML deferred-major artifact has an invalid schema, status, or decision".to_string(),
        );
    }

    let package = artifact.get("latestStable");
    for (field, expected) in [
        ("package", latest.package.as_str()),
        ("version", latest.version.as_str()),
        ("integrity", latest.integrity.as_str()),
        ("tarballUrl", latest.tarball_url.as_str()),
        ("attestationUrl", latest.attestation_url.as_str()),
    ] {
        if package
            .and_then(|value| value.get(field))
            .and_then(JsonValue::as_str)
            != Some(expected)
        {
            failures.push(format!(
                "ZenUML deferred-major artifact latestStable.{field} provenance drift"
            ));
        }
    }
    if package
        .and_then(|value| value.get("publishGitHead"))
        .and_then(JsonValue::as_str)
        != latest.publish_git_head.as_deref()
    {
        failures.push(
            "ZenUML deferred-major artifact latestStable.publishGitHead provenance drift"
                .to_string(),
        );
    }
    let source = package.and_then(|value| value.get("source"));
    for (field, expected) in [
        ("repository", latest.source.repository.as_str()),
        ("reference", latest.source.reference.as_str()),
        ("commit", latest.source.commit.as_str()),
    ] {
        if source
            .and_then(|value| value.get(field))
            .and_then(JsonValue::as_str)
            != Some(expected)
        {
            failures.push(format!(
                "ZenUML deferred-major artifact latestStable.source.{field} provenance drift"
            ));
        }
    }
    let checkout_matches = match latest.source.checkout_path.as_deref() {
        Some(expected) => {
            source
                .and_then(|value| value.get("checkoutPath"))
                .and_then(JsonValue::as_str)
                == Some(expected)
        }
        None => source.and_then(|value| value.get("checkoutPath")) == Some(&JsonValue::Null),
    };
    let package_path_matches = match latest.source.package_path.as_deref() {
        Some(expected) => {
            source
                .and_then(|value| value.get("packagePath"))
                .and_then(JsonValue::as_str)
                == Some(expected)
        }
        None => source.and_then(|value| value.get("packagePath")) == Some(&JsonValue::Null),
    };
    let surfaces = package
        .and_then(|value| value.get("requiredSurfaces"))
        .and_then(JsonValue::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(JsonValue::as_str)
                .collect::<Vec<_>>()
        });
    let expected_surfaces = latest
        .required_surfaces
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if !checkout_matches
        || !package_path_matches
        || surfaces.as_deref() != Some(expected_surfaces.as_slice())
    {
        failures.push(
            "ZenUML deferred-major artifact source materialization/surface contract drift"
                .to_string(),
        );
    }

    let selected = selected_behavior_source(behavior);
    let host = artifact.get("hostGraph");
    let expected_plugin = format!("{}@{}", plugin.package, plugin.version);
    if host
        .and_then(|value| value.get("mermaidRelease"))
        .and_then(JsonValue::as_str)
        != Some(mermaid_release.version.as_str())
        || host
            .and_then(|value| value.get("plugin"))
            .and_then(JsonValue::as_str)
            != Some(expected_plugin.as_str())
        || host
            .and_then(|value| value.get("pluginDeclaredRange"))
            .and_then(JsonValue::as_str)
            != Some(behavior.declared_range.as_str())
        || host
            .and_then(|value| value.get("mermaidWorkspaceRange"))
            .and_then(JsonValue::as_str)
            != Some(behavior.workspace_range.as_str())
        || host
            .and_then(|value| value.get("selectedVersion"))
            .and_then(JsonValue::as_str)
            != Some(behavior.selected_version.as_str())
        || host
            .and_then(|value| value.get("selectedCommit"))
            .and_then(JsonValue::as_str)
            != selected.map(|value| value.source.commit.as_str())
        || host
            .and_then(|value| value.get("latestStableSatisfiesPluginRange"))
            .and_then(JsonValue::as_bool)
            != Some(false)
        || host
            .and_then(|value| value.get("latestStableSatisfiesWorkspaceRange"))
            .and_then(JsonValue::as_bool)
            != Some(false)
    {
        failures.push(
            "ZenUML deferred-major host graph does not derive from the selected reference graph"
                .to_string(),
        );
    }

    let selected_major = selected
        .and_then(|value| parse_numeric_semver(&value.version))
        .map(|version| version.0);
    let latest_major = parse_numeric_semver(&latest.version).map(|version| version.0);
    let plugin_range_major = caret_range_major(&behavior.declared_range);
    let workspace_range_major = caret_range_major(&behavior.workspace_range);
    if selected_major.is_none()
        || latest_major.is_none()
        || latest_major <= selected_major
        || plugin_range_major != selected_major
        || workspace_range_major != selected_major
    {
        failures.push(
            "ZenUML deferred-major decision must be backed by an incompatible numeric major"
                .to_string(),
        );
    }

    let impact = artifact.get("selectionImpact");
    for field in [
        "selectedGraphChanged",
        "parserPorted",
        "semanticPorted",
        "editorLspPorted",
        "rendererPorted",
        "playgroundRuntimeChanged",
    ] {
        if impact
            .and_then(|value| value.get(field))
            .and_then(JsonValue::as_bool)
            != Some(false)
        {
            failures.push(format!(
                "ZenUML deferred-major artifact must not claim completed {field} work"
            ));
        }
    }
    if impact
        .and_then(|value| value.get("cargoFeatureDecision"))
        .and_then(JsonValue::as_str)
        != Some("not-evaluated-until-admission")
        || artifact
            .get("reason")
            .and_then(JsonValue::as_str)
            .is_none_or(|value| {
                value.trim().is_empty()
                    || !value.contains(&behavior.declared_range)
                    || !value.contains(&behavior.workspace_range)
                    || !value.contains(&latest.version)
            })
    {
        failures.push(
            "ZenUML deferred-major artifact must retain a no-feature/no-port rationale".to_string(),
        );
    }

    validate_deferred_major_delta_inventory(artifact, failures);
}

fn caret_range_major(value: &str) -> Option<u64> {
    parse_numeric_semver(value.strip_prefix('^')?).map(|version| version.0)
}

fn validate_deferred_major_delta_inventory(artifact: &JsonValue, failures: &mut Vec<String>) {
    let expected_areas = BTreeSet::from([
        "grammar-and-recovery",
        "semantic-and-model",
        "editor-and-lsp",
        "render-and-geometry",
        "browser-runtime-and-security",
        "feature-and-release-surface",
    ]);
    let Some(deltas) = artifact
        .get("behaviorDeltaInventory")
        .and_then(JsonValue::as_array)
    else {
        failures.push("ZenUML deferred-major artifact has no behavior delta inventory".to_string());
        return;
    };
    let mut actual_areas = BTreeSet::new();
    for delta in deltas {
        let area = delta
            .get("area")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();
        if !actual_areas.insert(area) {
            failures.push(format!(
                "ZenUML deferred-major behavior delta area {area:?} is empty or duplicated"
            ));
        }
        for field in ["source", "classification", "owner", "summary"] {
            if delta
                .get(field)
                .and_then(JsonValue::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                failures.push(format!(
                    "ZenUML deferred-major behavior delta {area:?} is missing {field}"
                ));
            }
        }
        if delta
            .get("owner")
            .and_then(JsonValue::as_str)
            .is_none_or(|owner| !owner.starts_with("future-major-admission/"))
        {
            failures.push(format!(
                "ZenUML deferred-major behavior delta {area:?} has no future admission owner"
            ));
        }
        let required = delta.get("evidenceRequired").and_then(JsonValue::as_array);
        let evidence = required
            .into_iter()
            .flatten()
            .filter_map(JsonValue::as_str)
            .collect::<Vec<_>>();
        let unique = evidence.iter().copied().collect::<BTreeSet<_>>();
        if evidence.len() < 2
            || evidence.len() != unique.len()
            || evidence.iter().any(|value| value.trim().is_empty())
        {
            failures.push(format!(
                "ZenUML deferred-major behavior delta {area:?} has incomplete evidence requirements"
            ));
        }
    }
    if actual_areas != expected_areas {
        failures.push(format!(
            "ZenUML deferred-major behavior areas must be exactly {expected_areas:?}, found {actual_areas:?}"
        ));
    }
}

fn validate_admission_deltas(admission: &JsonValue, failures: &mut Vec<String>) {
    let Some(deltas) = admission
        .get("classifiedDeltas")
        .and_then(JsonValue::as_array)
        .filter(|deltas| !deltas.is_empty())
    else {
        failures.push("ZenUML candidate admission has no classified behavior deltas".to_string());
        return;
    };
    let mut areas = BTreeSet::new();
    for delta in deltas {
        let Some(area) = delta.get("area").and_then(JsonValue::as_str) else {
            failures.push("ZenUML admission delta is missing area".to_string());
            continue;
        };
        if area.trim().is_empty() || !areas.insert(area) {
            failures.push(format!(
                "ZenUML admission delta area {area:?} is empty or duplicated"
            ));
        }
        for field in ["source", "classification", "owner", "summary"] {
            if delta
                .get(field)
                .and_then(JsonValue::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                failures.push(format!("ZenUML admission delta {area} is missing {field}"));
            }
        }
    }
}

fn validate_candidate_status(admission: &JsonValue, decision: &str, failures: &mut Vec<String>) {
    let status = admission.get("candidateStatus").and_then(JsonValue::as_str);
    let valid = match decision {
        "candidate-selected" => status == Some("pass"),
        "oracle-retained" => matches!(status, Some("pending" | "failed")),
        _ => false,
    };
    if !valid {
        failures.push(format!(
            "ZenUML candidateStatus {status:?} is inconsistent with decision {decision}"
        ));
    }
}

fn validate_admission_matrix(admission: &JsonValue, decision: &str, failures: &mut Vec<String>) {
    let required_gates = ZENUML_ADMISSION_GATES.into_iter().collect::<BTreeSet<_>>();
    let mut observed_gates = BTreeSet::new();
    let mut all_pass = true;
    if let Some(matrix) = admission.get("matrix").and_then(JsonValue::as_array) {
        for gate in matrix {
            let Some(name) = gate.get("gate").and_then(JsonValue::as_str) else {
                failures.push("ZenUML admission gate is missing its name".to_string());
                continue;
            };
            if !observed_gates.insert(name) {
                failures.push(format!("duplicate ZenUML admission gate {name}"));
            }
            let status = gate.get("status").and_then(JsonValue::as_str);
            if !matches!(status, Some("pass" | "pending" | "failed")) {
                failures.push(format!("ZenUML admission gate {name} has invalid status"));
            }
            all_pass &= status == Some("pass");
            let gate_evidence = gate.get("evidence");
            for field in ["kind", "reference", "summary"] {
                if gate_evidence
                    .and_then(|value| value.get(field))
                    .and_then(JsonValue::as_str)
                    .is_none_or(|value| value.trim().is_empty())
                {
                    failures.push(format!(
                        "ZenUML admission gate {name} has incomplete evidence.{field}"
                    ));
                }
            }
        }
    }
    if observed_gates != required_gates {
        failures.push(format!(
            "ZenUML admission gates must be exactly {required_gates:?}, found {observed_gates:?}"
        ));
    }
    match decision {
        "candidate-selected" if !all_pass => failures.push(
            "ZenUML candidate cannot be selected until every required gate passes".to_string(),
        ),
        "oracle-retained" if all_pass => failures.push(
            "ZenUML oracle cannot remain selected after every candidate gate passes".to_string(),
        ),
        "candidate-selected" | "oracle-retained" => {}
        decision => failures.push(format!("unsupported ZenUML admission decision {decision}")),
    }
}

fn verify_candidate_evidence(
    root: &Path,
    behavior: &BehaviorSource,
    admission: &JsonValue,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let Some(relative) = admission
        .get("executableEvidence")
        .and_then(JsonValue::as_str)
    else {
        failures.push("ZenUML admission must name executableEvidence".to_string());
        return Ok(());
    };
    let evidence = read_json(&root.join(relative))?;
    let harness = evidence.get("harness").and_then(JsonValue::as_str);
    if !harness.is_some_and(|path| root.join(path).is_file()) {
        failures.push("ZenUML candidate evidence references a missing harness".to_string());
    } else if let Some(path) = harness {
        let actual = file_sha256(&root.join(path))?;
        if evidence.get("harnessSha256").and_then(JsonValue::as_str) != Some(actual.as_str()) {
            failures.push(
                "ZenUML candidate evidence harnessSha256 does not match its executable harness"
                    .to_string(),
            );
        }
    }
    if evidence.get("schemaVersion").and_then(JsonValue::as_u64) != Some(5) {
        failures.push("ZenUML candidate evidence schemaVersion must be 5".to_string());
    }
    if evidence
        .get("command")
        .and_then(JsonValue::as_str)
        .is_none_or(|command| command.trim().is_empty())
    {
        failures.push("ZenUML candidate evidence has no executable command".to_string());
    }
    if evidence.get("onlineCommand").and_then(JsonValue::as_str)
        != Some("npm run verify:zenuml-candidate:online")
    {
        failures.push(
            "ZenUML candidate evidence must name its explicit online verification command"
                .to_string(),
        );
    }
    for (field, reference) in [
        ("oracle", &behavior.oracle),
        ("candidate", &behavior.candidate),
    ] {
        let package = evidence.get(field);
        for (name, expected) in [
            ("version", reference.version.as_str()),
            ("commit", reference.source.commit.as_str()),
            ("integrity", reference.integrity.as_str()),
            ("tarballUrl", reference.tarball_url.as_str()),
            ("attestationUrl", reference.attestation_url.as_str()),
        ] {
            if package
                .and_then(|value| value.get(name))
                .and_then(JsonValue::as_str)
                != Some(expected)
            {
                failures.push(format!("ZenUML executable evidence {field}.{name} drift"));
            }
        }
        verify_candidate_supply_chain_evidence(root, field, package, reference, failures)?;
    }
    let fixture_count = evidence
        .pointer("/corpus/fixtureCount")
        .and_then(JsonValue::as_u64);
    let corpus_sources = evidence
        .pointer("/corpus/sources")
        .and_then(JsonValue::as_array);
    if fixture_count != corpus_sources.and_then(|sources| u64::try_from(sources.len()).ok())
        || corpus_sources.is_none_or(Vec::is_empty)
    {
        failures.push(
            "ZenUML corpus fixtureCount must be derived from its actual source array".to_string(),
        );
    }
    verify_admission_fixture_counts(admission, fixture_count, failures);
    for pointer in [
        "/corpus/parseAgreementCount",
        "/semantic/agreementCount",
        "/render/svgAgreementCount",
    ] {
        if evidence.pointer(pointer).and_then(JsonValue::as_u64) != fixture_count
            || fixture_count == Some(0)
        {
            failures.push(format!(
                "ZenUML executable evidence {pointer} must cover the complete non-empty corpus"
            ));
        }
    }
    for pointer in [
        "/semantic/totals/participants",
        "/semantic/totals/messages",
        "/semantic/totals/fragments",
        "/semantic/totals/groups",
        "/semantic/totals/returns",
        "/semantic/totals/creations",
    ] {
        if evidence
            .pointer(pointer)
            .and_then(JsonValue::as_u64)
            .is_none_or(|count| count == 0)
        {
            failures.push(format!(
                "ZenUML executable evidence must exercise {pointer}"
            ));
        }
    }
    verify_candidate_topology_and_deltas(&evidence, failures);
    verify_candidate_strict_inline_artifact(root, &evidence, fixture_count, failures)?;
    verify_candidate_browser_admission(root, admission, &evidence, failures)?;
    for pointer in [
        "/pluginContract/candidateSatisfiesDeclaredRange",
        "/pluginContract/candidateSatisfiesWorkspaceRange",
    ] {
        if evidence.pointer(pointer).and_then(JsonValue::as_bool) != Some(true) {
            failures.push(format!("ZenUML executable evidence failed {pointer}"));
        }
    }
    verify_candidate_resource_evidence(&evidence, failures);
    Ok(())
}

fn verify_admission_fixture_counts(
    admission: &JsonValue,
    fixture_count: Option<u64>,
    failures: &mut Vec<String>,
) {
    let matrix = admission.get("matrix").and_then(JsonValue::as_array);
    for gate_name in ["corpus", "semantic", "render"] {
        let gate_count = matrix
            .into_iter()
            .flatten()
            .find(|gate| gate.get("gate").and_then(JsonValue::as_str) == Some(gate_name))
            .and_then(|gate| gate.pointer("/evidence/fixtureCount"))
            .and_then(JsonValue::as_u64);
        if fixture_count == Some(0) || gate_count != fixture_count {
            failures.push(format!(
                "ZenUML admission gate {gate_name} fixtureCount must match executable evidence"
            ));
        }
    }
}

fn verify_candidate_supply_chain_evidence(
    root: &Path,
    field: &str,
    package: Option<&JsonValue>,
    reference: &PackageReference,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let Some(supply) = package.and_then(|value| value.get("supplyChain")) else {
        failures.push(format!(
            "ZenUML executable evidence {field} has no supplyChain proof"
        ));
        return Ok(());
    };
    let Some(provenance) = reference.publish_provenance.as_ref() else {
        failures.push(format!(
            "ZenUML reference {field} has no publishProvenance descriptor"
        ));
        return Ok(());
    };
    let expected_integrity = reference.integrity.strip_prefix("sha512-");
    let tarball_base64 = supply
        .get("tarballSha512Base64")
        .and_then(JsonValue::as_str);
    let tarball_hex = supply.get("tarballSha512Hex").and_then(JsonValue::as_str);
    let decoded_integrity = tarball_base64.and_then(|value| decode_bounded_base64(value, 64));
    let derived_tarball_hex = decoded_integrity.as_deref().map(hex_lower);
    if tarball_base64 != expected_integrity
        || decoded_integrity
            .as_deref()
            .is_none_or(|bytes| bytes.len() != 64)
        || tarball_hex != derived_tarball_hex.as_deref()
        || supply
            .get("tarballBytes")
            .and_then(JsonValue::as_u64)
            .is_none_or(|bytes| bytes == 0)
    {
        failures.push(format!(
            "ZenUML executable evidence {field} has invalid independent tarball digest evidence"
        ));
    }
    let artifact_path = supply
        .pointer("/attestationArtifact/path")
        .and_then(JsonValue::as_str);
    let artifact_sha256 = supply
        .pointer("/attestationArtifact/sha256")
        .and_then(JsonValue::as_str);
    if artifact_path != Some(provenance.attestation_artifact.path.as_str())
        || artifact_sha256 != Some(provenance.attestation_artifact.sha256.as_str())
    {
        failures.push(format!(
            "ZenUML executable evidence {field} does not bind its declared attestation artifact"
        ));
        return Ok(());
    }
    let artifact_file = root.join(&provenance.attestation_artifact.path);
    let artifact_bytes = fs::read(&artifact_file).map_err(|source| XtaskError::ReadFile {
        path: artifact_file.display().to_string(),
        source,
    })?;
    if artifact_bytes.is_empty()
        || u64::try_from(artifact_bytes.len()).unwrap_or(u64::MAX) > MAX_ATTESTATION_ARTIFACT_BYTES
    {
        failures.push(format!(
            "ZenUML executable evidence {field} attestation artifact exceeds its byte budget"
        ));
        return Ok(());
    }
    let actual_artifact_sha256 = crate::util::sha256_hex(&artifact_bytes);
    if actual_artifact_sha256 != provenance.attestation_artifact.sha256 {
        failures.push(format!(
            "ZenUML executable evidence {field} attestation artifact digest drift"
        ));
        return Ok(());
    }
    let raw_artifact: JsonValue = serde_json::from_slice(&artifact_bytes)?;
    if canonical_pretty_json(&raw_artifact)?.as_bytes() != artifact_bytes.as_slice() {
        failures.push(format!(
            "ZenUML executable evidence {field} attestation artifact is not canonical JSON"
        ));
    }
    let artifact: AttestationArtifact = serde_json::from_value(raw_artifact)?;
    if artifact.schema_version != ATTESTATION_ARTIFACT_SCHEMA_VERSION
        || artifact.package != reference.package
        || artifact.version != reference.version
        || artifact.attestations.len() != 2
    {
        failures.push(format!(
            "ZenUML executable evidence {field} attestation artifact identity is invalid"
        ));
        return Ok(());
    }

    let Some(tarball_hex) = tarball_hex else {
        return Ok(());
    };
    let mut derived_attestations = BTreeMap::new();
    for attestation in artifact.attestations {
        let Some(envelope_value) = attestation.bundle.get("dsseEnvelope") else {
            failures.push(format!(
                "ZenUML executable evidence {field} {} has no DSSE envelope",
                attestation.predicate_type
            ));
            continue;
        };
        let envelope: DsseEnvelope = serde_json::from_value(envelope_value.clone())?;
        if envelope.payload_type != "application/vnd.in-toto+json"
            || envelope.signatures.is_empty()
            || envelope.signatures.len() > 4
            || envelope.signatures.iter().any(|signature| {
                signature.keyid.len() > 1024
                    || decode_bounded_base64(&signature.sig, 4096).is_none()
            })
        {
            failures.push(format!(
                "ZenUML executable evidence {field} {} has an invalid DSSE envelope",
                attestation.predicate_type
            ));
            continue;
        }
        let Some(payload_bytes) =
            decode_bounded_base64(&envelope.payload, MAX_ATTESTATION_PAYLOAD_BYTES)
        else {
            failures.push(format!(
                "ZenUML executable evidence {field} {} has an invalid DSSE payload",
                attestation.predicate_type
            ));
            continue;
        };
        let statement: JsonValue = serde_json::from_slice(&payload_bytes)?;
        if statement.get("predicateType").and_then(JsonValue::as_str)
            != Some(attestation.predicate_type.as_str())
            || !statement_binds_subject(&statement, reference, tarball_hex)
        {
            failures.push(format!(
                "ZenUML executable evidence {field} {} does not bind its predicate and npm subject",
                attestation.predicate_type
            ));
            continue;
        }
        let derived = DerivedAttestation {
            envelope_sha256: crate::util::sha256_hex(&canonical_json(envelope_value)?),
            payload_sha256: crate::util::sha256_hex(&payload_bytes),
            statement,
        };
        if derived_attestations
            .insert(attestation.predicate_type, derived)
            .is_some()
        {
            failures.push(format!(
                "ZenUML executable evidence {field} has duplicate attestation predicates"
            ));
        }
    }
    let expected_predicates =
        BTreeSet::from([PUBLISH_ATTESTATION_PREDICATE, SLSA_ATTESTATION_PREDICATE]);
    let actual_artifact_predicates = derived_attestations
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual_artifact_predicates != expected_predicates {
        failures.push(format!(
            "ZenUML executable evidence {field} attestation artifact has an invalid predicate set"
        ));
        return Ok(());
    }

    let publish = &derived_attestations[PUBLISH_ATTESTATION_PREDICATE];
    let slsa = &derived_attestations[SLSA_ATTESTATION_PREDICATE];
    for (name, derived) in [("publish", publish), ("slsa", slsa)] {
        if supply
            .pointer(&format!("/{name}/envelopeSha256"))
            .and_then(JsonValue::as_str)
            != Some(derived.envelope_sha256.as_str())
            || supply
                .pointer(&format!("/{name}/payloadSha256"))
                .and_then(JsonValue::as_str)
                != Some(derived.payload_sha256.as_str())
        {
            failures.push(format!(
                "ZenUML executable evidence {field} {name} digest is not derived from its attestation artifact"
            ));
        }
    }
    let expected_subject = format!(
        "pkg:npm/%40{}@{}",
        reference.package.trim_start_matches('@'),
        reference.version
    );
    if supply.pointer("/subject/name").and_then(JsonValue::as_str)
        != Some(expected_subject.as_str())
        || supply
            .pointer("/subject/digest/sha512")
            .and_then(JsonValue::as_str)
            != Some(tarball_hex)
    {
        failures.push(format!(
            "ZenUML executable evidence {field} does not bind its npm subject to the tarball"
        ));
    }
    let recorded_predicate_values = supply
        .pointer("/npmAudit/predicateTypes")
        .and_then(JsonValue::as_array);
    let actual_predicates = recorded_predicate_values
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .collect::<BTreeSet<_>>();
    let npm_version = supply
        .pointer("/npmAudit/cliVersion")
        .and_then(JsonValue::as_str)
        .and_then(parse_numeric_semver);
    if supply
        .pointer("/npmAudit/verified")
        .and_then(JsonValue::as_bool)
        != Some(true)
        || supply
            .pointer("/npmAudit/registry")
            .and_then(JsonValue::as_str)
            != Some(OFFICIAL_NPM_REGISTRY_PREFIX)
        || actual_predicates != expected_predicates
        || recorded_predicate_values.map(Vec::len) != Some(2)
        || npm_version.is_none_or(|version| version < (11, 17, 0))
    {
        failures.push(format!(
            "ZenUML executable evidence {field} does not record the required online npm verification"
        ));
    }
    let expected_san = expected_workflow_san(&provenance.workflow);
    let parsed_workflow = slsa
        .statement
        .pointer("/predicate/buildDefinition/externalParameters/workflow")
        .cloned()
        .map(serde_json::from_value::<PublishWorkflow>)
        .transpose()?;
    let resolved_commit = slsa
        .statement
        .pointer("/predicate/buildDefinition/resolvedDependencies")
        .and_then(JsonValue::as_array)
        .is_some_and(|dependencies| {
            dependencies.iter().any(|dependency| {
                dependency
                    .pointer("/digest/gitCommit")
                    .and_then(JsonValue::as_str)
                    == Some(reference.source.commit.as_str())
            })
        });
    if supply
        .pointer("/publish/registry")
        .and_then(JsonValue::as_str)
        != Some("https://registry.npmjs.org")
        || supply
            .pointer("/publish/predicateType")
            .and_then(JsonValue::as_str)
            != Some(PUBLISH_ATTESTATION_PREDICATE)
        || publish
            .statement
            .pointer("/predicate/name")
            .and_then(JsonValue::as_str)
            != Some(reference.package.as_str())
        || publish
            .statement
            .pointer("/predicate/version")
            .and_then(JsonValue::as_str)
            != Some(reference.version.as_str())
        || publish
            .statement
            .pointer("/predicate/registry")
            .and_then(JsonValue::as_str)
            != Some("https://registry.npmjs.org")
        || supply
            .pointer("/slsa/resolvedGitCommit")
            .and_then(JsonValue::as_str)
            != Some(reference.source.commit.as_str())
        || supply
            .pointer("/slsa/predicateType")
            .and_then(JsonValue::as_str)
            != Some(SLSA_ATTESTATION_PREDICATE)
        || !resolved_commit
        || parsed_workflow.as_ref() != Some(&provenance.workflow)
        || supply
            .pointer("/slsa/workflow/repository")
            .and_then(JsonValue::as_str)
            != Some(provenance.workflow.repository.as_str())
        || supply
            .pointer("/slsa/workflow/path")
            .and_then(JsonValue::as_str)
            != Some(provenance.workflow.path.as_str())
        || supply
            .pointer("/slsa/workflow/ref")
            .and_then(JsonValue::as_str)
            != Some(provenance.workflow.r#ref.as_str())
        || supply
            .pointer("/slsa/certificateSubjectAltName")
            .and_then(JsonValue::as_str)
            != Some(expected_san.as_str())
        || supply
            .pointer("/slsa/certificateIssuer")
            .and_then(JsonValue::as_str)
            .is_none_or(|issuer| !issuer.contains("sigstore"))
        || supply
            .pointer("/slsa/certificateFingerprint256")
            .and_then(JsonValue::as_str)
            .is_none_or(|fingerprint| !is_sha256_fingerprint(fingerprint))
    {
        failures.push(format!(
            "ZenUML executable evidence {field} has incomplete SLSA workflow binding"
        ));
    }
    Ok(())
}

fn parse_numeric_semver(value: &str) -> Option<(u64, u64, u64)> {
    let mut components = value.split('.');
    let version = (
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
    );
    components.next().is_none().then_some(version)
}

#[derive(Debug)]
struct DerivedAttestation {
    envelope_sha256: String,
    payload_sha256: String,
    statement: JsonValue,
}

fn statement_binds_subject(
    statement: &JsonValue,
    reference: &PackageReference,
    tarball_sha512_hex: &str,
) -> bool {
    let expected_name = format!(
        "pkg:npm/%40{}@{}",
        reference.package.trim_start_matches('@'),
        reference.version
    );
    let Some(subjects) = statement.get("subject").and_then(JsonValue::as_array) else {
        return false;
    };
    subjects.len() == 1
        && subjects[0].get("name").and_then(JsonValue::as_str) == Some(expected_name.as_str())
        && subjects[0]
            .pointer("/digest/sha512")
            .and_then(JsonValue::as_str)
            == Some(tarball_sha512_hex)
}

fn decode_bounded_base64(value: &str, max_decoded_bytes: usize) -> Option<Vec<u8>> {
    if value.is_empty() || value.len() > max_decoded_bytes.saturating_mul(2) {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value.as_bytes())
        .ok()?;
    (!decoded.is_empty()
        && decoded.len() <= max_decoded_bytes
        && base64::engine::general_purpose::STANDARD.encode(&decoded) == value)
        .then_some(decoded)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn expected_workflow_san(workflow: &PublishWorkflow) -> String {
    format!(
        "URI:{}/{}@{}",
        workflow.repository, workflow.path, workflow.r#ref
    )
}

fn is_sha256_fingerprint(value: &str) -> bool {
    let segments = value.split(':').collect::<Vec<_>>();
    segments.len() == 32
        && segments.iter().all(|segment| {
            segment.len() == 2
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        })
}

fn canonical_json(value: &JsonValue) -> Result<Vec<u8>, XtaskError> {
    let mut canonical = value.clone();
    sort_json_keys(&mut canonical);
    serde_json::to_vec(&canonical).map_err(XtaskError::from)
}

fn canonical_pretty_json(value: &JsonValue) -> Result<String, XtaskError> {
    let mut canonical = value.clone();
    sort_json_keys(&mut canonical);
    let mut output = serde_json::to_string_pretty(&canonical)?;
    output.push('\n');
    Ok(output)
}

fn sort_json_keys(value: &mut JsonValue) {
    match value {
        JsonValue::Object(object) => {
            let mut entries = std::mem::take(object).into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, mut child) in entries {
                sort_json_keys(&mut child);
                object.insert(key, child);
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                sort_json_keys(value);
            }
        }
        _ => {}
    }
}

fn verify_candidate_topology_and_deltas(evidence: &JsonValue, failures: &mut Vec<String>) {
    let required_topology = BTreeSet::from([
        "emojiMessages",
        "numberUnitConditionFragments",
        "parallelFragments",
        "participantGroups",
        "tryCatchFinallyFragments",
    ]);
    let Some(topology) = evidence
        .pointer("/semantic/requiredTopology")
        .and_then(JsonValue::as_object)
    else {
        failures.push("ZenUML candidate evidence has no required topology inventory".to_string());
        return;
    };
    if topology.keys().map(String::as_str).collect::<BTreeSet<_>>() != required_topology
        || topology
            .values()
            .any(|value| value.as_u64().is_none_or(|count| count == 0))
    {
        failures.push(
            "ZenUML candidate evidence required topology must be exact and non-empty".to_string(),
        );
    }

    let Some(classified) = evidence
        .pointer("/semantic/classifiedBehavior")
        .and_then(JsonValue::as_array)
    else {
        failures.push("ZenUML candidate evidence has no classified behavior probes".to_string());
        return;
    };
    let mut ids = BTreeSet::new();
    if classified.len() < 8 {
        failures.push(
            "ZenUML candidate evidence must retain the complete named behavior matrix".to_string(),
        );
    }
    for behavior in classified {
        let id = behavior.get("id").and_then(JsonValue::as_str);
        if id.is_none_or(|id| id.is_empty() || !ids.insert(id))
            || behavior
                .get("classification")
                .and_then(JsonValue::as_str)
                .is_none_or(str::is_empty)
            || behavior
                .pointer("/sourceAttribution/paths")
                .and_then(JsonValue::as_array)
                .is_none_or(Vec::is_empty)
            || behavior
                .pointer("/sourceAttribution/rules")
                .and_then(JsonValue::as_array)
                .is_none_or(Vec::is_empty)
        {
            failures
                .push("ZenUML classified behavior has incomplete source attribution".to_string());
        }
        let assertions = behavior.get("assertions").and_then(JsonValue::as_array);
        if assertions.is_none_or(Vec::is_empty) {
            failures.push("ZenUML classified behavior has no named assertions".to_string());
            continue;
        }
        for assertion in assertions.into_iter().flatten() {
            if assertion.get("actual") != assertion.get("expected")
                || assertion
                    .get("engine")
                    .and_then(JsonValue::as_str)
                    .is_none_or(|engine| !matches!(engine, "oracle" | "candidate"))
                || assertion
                    .get("fact")
                    .and_then(JsonValue::as_str)
                    .is_none_or(str::is_empty)
            {
                failures
                    .push("ZenUML classified behavior assertion failed or is invalid".to_string());
            }
        }
    }
}

fn verify_candidate_strict_inline_artifact(
    root: &Path,
    evidence: &JsonValue,
    fixture_count: Option<u64>,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let strict = evidence.get("strictInlineSvg");
    let fixtures = strict
        .and_then(|value| value.get("fixtures"))
        .and_then(JsonValue::as_array);
    let corpus_sources = evidence
        .pointer("/corpus/sources")
        .and_then(JsonValue::as_array);
    let Some((fixtures, corpus_sources)) = fixtures.zip(corpus_sources) else {
        failures.push("ZenUML strict-inline artifact evidence has no fixture array".to_string());
        return Ok(());
    };
    let fixture_len = u64::try_from(fixtures.len()).ok();
    let strict_names = fixtures
        .iter()
        .filter_map(|fixture| fixture.get("name").and_then(JsonValue::as_str))
        .collect::<Vec<_>>();
    let corpus_names = corpus_sources
        .iter()
        .filter_map(|source| source.get("name").and_then(JsonValue::as_str))
        .collect::<Vec<_>>();
    let passed_count = fixtures
        .iter()
        .filter(|fixture| fixture.get("passed").and_then(JsonValue::as_bool) == Some(true))
        .count();
    let svg_bytes = fixtures
        .iter()
        .filter_map(|fixture| fixture.get("svgBytes").and_then(JsonValue::as_u64))
        .collect::<Vec<_>>();
    let total_svg_bytes = svg_bytes
        .iter()
        .map(|bytes| u128::from(*bytes))
        .sum::<u128>();
    let recorded_total = strict
        .and_then(|value| value.get("totalSvgBytes"))
        .and_then(JsonValue::as_u64)
        .map(u128::from);
    let recorded_max = strict
        .and_then(|value| value.get("maxSvgBytes"))
        .and_then(JsonValue::as_u64);
    if fixture_count != fixture_len
        || strict
            .and_then(|value| value.get("fixtureCount"))
            .and_then(JsonValue::as_u64)
            != fixture_len
        || strict
            .and_then(|value| value.get("passedCount"))
            .and_then(JsonValue::as_u64)
            != u64::try_from(passed_count).ok()
        || passed_count != fixtures.len()
        || strict_names.len() != fixtures.len()
        || strict_names != corpus_names
        || svg_bytes.len() != fixtures.len()
        || svg_bytes.contains(&0)
        || recorded_total != Some(total_svg_bytes)
        || recorded_max != svg_bytes.iter().copied().max()
        || fixtures.iter().any(|fixture| {
            fixture
                .get("foreignObjectFree")
                .and_then(JsonValue::as_bool)
                != Some(true)
        })
        || fixtures.iter().any(|fixture| {
            fixture
                .get("svgSha256")
                .and_then(JsonValue::as_str)
                .is_none_or(|digest| !crate::util::is_canonical_sha256(digest))
        })
    {
        failures.push(
            "ZenUML strict-inline artifact facts and counts must derive from the complete fixture array"
                .to_string(),
        );
    }
    let expected_validator_paths = BTreeSet::from([
        "platforms/web/src/svg-safety-policy.ts",
        "platforms/web/src/svg-safety.ts",
    ]);
    let validator_sources = strict
        .and_then(|value| value.get("validatorSources"))
        .and_then(JsonValue::as_array);
    let actual_validator_paths = validator_sources
        .into_iter()
        .flatten()
        .filter_map(|source| source.get("path").and_then(JsonValue::as_str))
        .collect::<BTreeSet<_>>();
    if validator_sources.map(Vec::len) != Some(expected_validator_paths.len())
        || actual_validator_paths != expected_validator_paths
    {
        failures.push(
            "ZenUML strict-inline artifact evidence must own the publication validator sources"
                .to_string(),
        );
        return Ok(());
    }
    for source in validator_sources.into_iter().flatten() {
        let path = source
            .get("path")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();
        let expected = source
            .get("sha256")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();
        if !crate::util::is_canonical_sha256(expected) || file_sha256(&root.join(path))? != expected
        {
            failures.push(format!(
                "ZenUML strict-inline artifact validator source drift at {path}"
            ));
        }
    }
    Ok(())
}

fn verify_candidate_browser_admission(
    root: &Path,
    admission: &JsonValue,
    candidate: &JsonValue,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let fields = [
        ("execution-isolation", "executionIsolation"),
        ("security", "security"),
    ];
    let artifact_path = candidate
        .pointer("/executionIsolation/artifact")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let contract_path = candidate
        .pointer("/executionIsolation/probeContract")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let artifact_sha256 = candidate
        .pointer("/executionIsolation/artifactSha256")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    if !is_owned_relative_path(artifact_path)
        || !is_owned_relative_path(contract_path)
        || !crate::util::is_canonical_sha256(artifact_sha256)
        || fields.iter().any(|(_, field)| {
            candidate
                .pointer(&format!("/{field}/artifact"))
                .and_then(JsonValue::as_str)
                != Some(artifact_path)
                || candidate
                    .pointer(&format!("/{field}/probeContract"))
                    .and_then(JsonValue::as_str)
                    != Some(contract_path)
                || candidate
                    .pointer(&format!("/{field}/artifactSha256"))
                    .and_then(JsonValue::as_str)
                    != Some(artifact_sha256)
        })
    {
        failures.push(
            "ZenUML browser admission summaries must share one owned artifact and probe contract"
                .to_string(),
        );
        return Ok(());
    }

    let artifact_file = root.join(artifact_path);
    let artifact_text =
        fs::read_to_string(&artifact_file).map_err(|source| XtaskError::ReadFile {
            path: artifact_file.display().to_string(),
            source,
        })?;
    let artifact: JsonValue = serde_json::from_str(&artifact_text)?;
    if crate::util::sha256_hex(artifact_text.as_bytes()) != artifact_sha256
        || artifact.get("schemaVersion").and_then(JsonValue::as_u64) != Some(1)
        || artifact.get("generatedBy").and_then(JsonValue::as_str)
            != Some("playground/scripts/zenuml-browser-admission.mjs")
        || artifact.get("command").and_then(JsonValue::as_str)
            != Some("npm run test:zenuml-browser-admission")
    {
        failures.push(
            "ZenUML browser admission artifact is non-canonical, stale, or has an invalid contract"
                .to_string(),
        );
    }

    let contract_file = root.join(contract_path);
    let contract_text =
        fs::read_to_string(&contract_file).map_err(|source| XtaskError::ReadFile {
            path: contract_file.display().to_string(),
            source,
        })?;
    let contract: JsonValue = serde_json::from_str(&contract_text)?;
    let contract_sha256 = crate::util::sha256_hex(contract_text.as_bytes());
    if contract.get("schemaVersion").and_then(JsonValue::as_u64) != Some(1)
        || artifact
            .pointer("/probeContract/path")
            .and_then(JsonValue::as_str)
            != Some(contract_path)
        || artifact
            .pointer("/probeContract/sha256")
            .and_then(JsonValue::as_str)
            != Some(contract_sha256.as_str())
    {
        failures.push(
            "ZenUML browser admission probe contract is non-canonical, stale, or unbound"
                .to_string(),
        );
    }

    verify_browser_admission_sources(root, &artifact, failures)?;
    let projects = contract.get("projects").and_then(JsonValue::as_array);
    if projects.is_none_or(Vec::is_empty) || artifact.get("projects") != contract.get("projects") {
        failures.push(
            "ZenUML browser admission projects must derive from the non-empty probe contract"
                .to_string(),
        );
    }
    for (gate, field) in fields {
        let required = contract
            .pointer(&format!("/categories/{gate}"))
            .and_then(JsonValue::as_array);
        let observed = artifact.get(field);
        verify_browser_admission_category(gate, projects, required, observed, failures);
        verify_browser_admission_summary(admission, candidate, gate, field, observed, failures);
    }
    Ok(())
}

fn verify_browser_admission_sources(
    root: &Path,
    artifact: &JsonValue,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let sources = artifact.get("sourceFiles").and_then(JsonValue::as_array);
    let required = BTreeSet::from([
        "playground/scripts/zenuml-admission-reporter.mjs",
        "playground/scripts/zenuml-browser-admission.mjs",
        "playground/src/benchmark/realm/controller.ts",
        "playground/src/runtime/realm/browser-realm-channel.ts",
        "playground/src/runtime/realm/opaque-realm-document.ts",
        "playground/tests/benchmark.realm.spec.ts",
    ]);
    let mut paths = Vec::new();
    for source in sources.into_iter().flatten() {
        let path = source
            .get("path")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();
        let expected = source
            .get("sha256")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();
        paths.push(path);
        if !is_owned_relative_path(path)
            || !crate::util::is_canonical_sha256(expected)
            || file_sha256(&root.join(path))? != expected
        {
            failures.push(format!(
                "ZenUML browser admission source digest drift at {path}"
            ));
        }
    }
    let actual = paths.iter().copied().collect::<BTreeSet<_>>();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    if paths.is_empty()
        || paths != sorted
        || actual.len() != paths.len()
        || !required.is_subset(&actual)
    {
        failures.push(
            "ZenUML browser admission must bind a sorted unique set of security-critical sources"
                .to_string(),
        );
    }
    Ok(())
}

fn verify_browser_admission_category(
    gate: &str,
    projects: Option<&Vec<JsonValue>>,
    required: Option<&Vec<JsonValue>>,
    observed: Option<&JsonValue>,
    failures: &mut Vec<String>,
) {
    let project_count = projects.and_then(|values| u64::try_from(values.len()).ok());
    let probe_count = required.and_then(|values| u64::try_from(values.len()).ok());
    let observations = observed
        .and_then(|value| value.get("probes"))
        .and_then(JsonValue::as_array);
    let expected_observation_count = project_count.zip(probe_count).map(|(a, b)| a * b);
    let mut actual_observation_count = 0u64;
    let valid_probes = required
        .zip(observations)
        .is_some_and(|(required, observations)| {
            required.len() == observations.len()
                && required
                    .iter()
                    .zip(observations)
                    .all(|(required, observed)| {
                        if observed.get("id") != required.get("id")
                            || observed.get("description") != required.get("description")
                        {
                            return false;
                        }
                        let probe_observations =
                            observed.get("observations").and_then(JsonValue::as_array);
                        actual_observation_count += probe_observations
                            .and_then(|values| u64::try_from(values.len()).ok())
                            .unwrap_or_default();
                        probe_observations.zip(projects).is_some_and(
                            |(probe_observations, projects)| {
                                probe_observations.len() == projects.len()
                                    && probe_observations.iter().zip(projects).all(
                                        |(observation, project)| {
                                            observation.get("project") == Some(project)
                                                && observation
                                                    .get("passed")
                                                    .and_then(JsonValue::as_bool)
                                                    == Some(true)
                                                && observation.get("observed")
                                                    == observation.get("expected")
                                                && observation
                                                    .get("testTitle")
                                                    .and_then(JsonValue::as_str)
                                                    .is_some_and(|title| {
                                                        !title.trim().is_empty()
                                                            && title.trim() == title
                                                    })
                                        },
                                    )
                            },
                        )
                    })
        });
    if project_count.is_none_or(|count| count == 0)
        || probe_count.is_none_or(|count| count == 0)
        || !valid_probes
        || Some(actual_observation_count) != expected_observation_count
        || observed
            .and_then(|value| value.get("projectCount"))
            .and_then(JsonValue::as_u64)
            != project_count
        || observed
            .and_then(|value| value.get("probeCount"))
            .and_then(JsonValue::as_u64)
            != probe_count
        || observed
            .and_then(|value| value.get("observationCount"))
            .and_then(JsonValue::as_u64)
            != expected_observation_count
        || observed
            .and_then(|value| value.get("passedObservationCount"))
            .and_then(JsonValue::as_u64)
            != expected_observation_count
    {
        failures.push(format!(
            "ZenUML {gate} evidence must contain every passing probe observation for every project"
        ));
    }
}

fn verify_browser_admission_summary(
    admission: &JsonValue,
    candidate: &JsonValue,
    gate: &str,
    field: &str,
    artifact_category: Option<&JsonValue>,
    failures: &mut Vec<String>,
) {
    let summary = candidate.get(field);
    let matrix_evidence = admission
        .get("matrix")
        .and_then(JsonValue::as_array)
        .and_then(|matrix| {
            matrix
                .iter()
                .find(|entry| entry.get("gate").and_then(JsonValue::as_str) == Some(gate))
        })
        .and_then(|entry| entry.get("evidence"));
    let expected_reference = format!("tools/upstreams/ZENUML_CORE_CANDIDATE_EVIDENCE.json#{field}");
    let count_fields = [
        "projectCount",
        "probeCount",
        "observationCount",
        "passedObservationCount",
    ];
    if summary.is_none()
        || matrix_evidence
            .and_then(|value| value.get("kind"))
            .and_then(JsonValue::as_str)
            != Some("artifact")
        || matrix_evidence
            .and_then(|value| value.get("reference"))
            .and_then(JsonValue::as_str)
            != Some(expected_reference.as_str())
        || count_fields.iter().any(|field| {
            summary.and_then(|value| value.get(field))
                != artifact_category.and_then(|value| value.get(field))
                || matrix_evidence.and_then(|value| value.get(field))
                    != summary.and_then(|value| value.get(field))
        })
    {
        failures.push(format!(
            "ZenUML {gate} admission summary is not derived from executable browser evidence"
        ));
    }
}

fn verify_candidate_resource_evidence(evidence: &JsonValue, failures: &mut Vec<String>) {
    let oracle_bytes = evidence
        .pointer("/oracle/runtimeEntryBytes")
        .and_then(JsonValue::as_u64);
    let candidate_bytes = evidence
        .pointer("/candidate/runtimeEntryBytes")
        .and_then(JsonValue::as_u64);
    let recorded_delta = evidence
        .pointer("/resource/runtimeEntryDeltaBytes")
        .and_then(JsonValue::as_i64);
    let recorded_basis_points = evidence
        .pointer("/resource/runtimeEntryDeltaBasisPoints")
        .and_then(JsonValue::as_i64);
    let scope = evidence
        .pointer("/resource/measurementScope")
        .and_then(JsonValue::as_str);

    let Some((oracle_bytes, candidate_bytes)) = oracle_bytes.zip(candidate_bytes) else {
        failures.push("ZenUML resource evidence must record both runtime entry sizes".to_string());
        return;
    };
    if oracle_bytes == 0 || candidate_bytes == 0 || scope != Some("runtime-entry") {
        failures.push(
            "ZenUML resource evidence must identify a non-empty runtime-entry measurement"
                .to_string(),
        );
        return;
    }
    let expected_delta = i128::from(candidate_bytes) - i128::from(oracle_bytes);
    let expected_basis_points =
        (expected_delta * 10_000 + i128::from(oracle_bytes) / 2) / i128::from(oracle_bytes);
    if recorded_delta.map(i128::from) != Some(expected_delta)
        || recorded_basis_points.map(i128::from) != Some(expected_basis_points)
    {
        failures.push(
            "ZenUML resource evidence must reproducibly derive its absolute and relative deltas"
                .to_string(),
        );
    }
}

fn verify_installed_content(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let node_modules = root.join("tools/mermaid-cli/node_modules");
    for reference in materialized_runtime_references(bundle)? {
        let expected = reference
            .installed_content_sha256
            .as_deref()
            .ok_or_else(|| {
                XtaskError::MermaidReference(format!(
                    "materialized runtime package {}@{} has no installedContentSha256",
                    reference.package, reference.version
                ))
            })?;
        let package_root = node_modules.join(&reference.package);
        verify_installed_package_content(
            &package_root,
            &reference.package,
            &reference.version,
            expected,
            failures,
        )?;
    }
    Ok(())
}

fn verify_installed_package_content(
    package_root: &Path,
    package: &str,
    version: &str,
    expected: &str,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let actual = crate::cmd::upstream_svg_package_tree_sha256(package_root)?;
    if actual != expected {
        failures.push(format!(
            "installed {package}@{version} content drift at {}: expected {expected}, found {actual}",
            package_root.display()
        ));
    }
    Ok(())
}

fn verify_repository_state(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    materialized: bool,
) -> Result<(), XtaskError> {
    let mut failures = Vec::new();
    verify_playground_files(root, bundle, &mut failures)?;
    verify_reference_cli_files(root, bundle, &mut failures)?;
    verify_upstream_provenance_sources(root, bundle, &mut failures)?;
    verify_source_checkouts(root, bundle, materialized, &mut failures)?;
    verify_builtin_registry_inventory(root, bundle, materialized, &mut failures);
    let zenuml = bundle
        .external_diagrams
        .iter()
        .find(|diagram| diagram.id == "zenuml")
        .ok_or_else(|| XtaskError::MermaidReference("missing ZenUML reference".to_string()))?;
    let selected = selected_behavior_source(&zenuml.behavior_source)
        .ok_or_else(|| XtaskError::MermaidReference("invalid ZenUML selection".to_string()))?;
    verify_admission(
        root,
        &bundle.release,
        &zenuml.plugin,
        &zenuml.behavior_source,
        &mut failures,
    )?;

    for expectation in workspace_graph_expectations(bundle, zenuml, selected) {
        verify_workspace_graph(root, &expectation, bundle, materialized, &mut failures)?;
    }
    if materialized {
        verify_installed_content(root, bundle, &mut failures)?;
    }

    for (relative, expected) in expected_projections(bundle)? {
        let path = root.join(relative);
        let actual = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        if actual != expected {
            failures.push(format!(
                "generated projection drift at {relative}; run `cargo run -p xtask -- gen-mermaid-reference`"
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::MermaidReference(failures.join("\n")))
    }
}

pub(crate) fn gen_mermaid_reference(args: Vec<String>) -> Result<(), XtaskError> {
    let refresh_provenance = match args.as_slice() {
        [] => false,
        [argument] if argument == "--refresh-provenance" => true,
        _ => return Err(XtaskError::Usage),
    };
    let path = crate::cmd::workspace_root().join(BUNDLE_RELATIVE_PATH);
    let bundle = load_bundle(&path)?;
    validate_bundle(&bundle)?;
    for (relative, contents) in expected_projections(&bundle)? {
        write_projection(&crate::cmd::workspace_root().join(relative), &contents)?;
    }
    if refresh_provenance {
        regenerate_upstream_provenance()?;
    }
    Ok(())
}

pub(crate) fn verify_mermaid_reference(args: Vec<String>) -> Result<(), XtaskError> {
    let materialized = match args.as_slice() {
        [] => false,
        [argument] if argument == "--materialized" => true,
        _ => return Err(XtaskError::Usage),
    };
    let path = crate::cmd::workspace_root().join(BUNDLE_RELATIVE_PATH);
    let bundle = load_bundle(&path)?;
    validate_bundle(&bundle)?;
    verify_repository_state(&crate::cmd::workspace_root(), &bundle, materialized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admission_with_statuses(statuses: &[(&str, &str)]) -> JsonValue {
        serde_json::json!({
            "matrix": statuses
                .iter()
                .map(|(gate, status)| {
                    serde_json::json!({
                        "gate": gate,
                        "status": status,
                        "evidence": {
                            "kind": "test",
                            "reference": "test://admission",
                            "summary": "Focused admission-matrix evidence."
                        }
                    })
                })
                .collect::<Vec<_>>()
        })
    }

    #[test]
    fn provenance_inventory_fails_closed_when_a_primary_family_manifest_is_missing() {
        let temporary = tempfile::tempdir().expect("temporary provenance root");
        let provenance_root = temporary.path().join(UPSTREAM_SVG_PROVENANCE_RELATIVE_PATH);
        let flowchart = provenance_root.join("flowchart");
        fs::create_dir_all(&flowchart).expect("create flowchart provenance directory");
        fs::write(flowchart.join("_baseline-manifest.json"), "{}\n")
            .expect("write flowchart manifest");

        let error = upstream_provenance_manifests_for(temporary.path(), &["flowchart", "state"])
            .expect_err("the absent state manifest must fail closed");

        assert!(error.to_string().contains("state"));
        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn provenance_refresh_delegates_to_a_complete_svg_regeneration() {
        let mut observed = None;

        regenerate_upstream_provenance_with(|args| {
            observed = Some(args);
            Ok(())
        })
        .expect("delegate provenance refresh");

        assert_eq!(
            observed,
            Some(vec!["--diagram".to_string(), "all".to_string()])
        );
    }

    #[test]
    fn bundle_requires_installed_content_hashes_for_dynamic_runtime_companions() {
        let root = crate::cmd::workspace_root();
        let mut bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let zenuml = bundle
            .external_diagrams
            .iter_mut()
            .find(|diagram| diagram.id == "zenuml")
            .expect("ZenUML reference");
        zenuml.plugin.installed_content_sha256 = None;

        let error = validate_bundle(&bundle).expect_err("missing companion digest must fail");

        assert!(error.to_string().contains("@mermaid-js/mermaid-zenuml"));
        assert!(error.to_string().contains("installedContentSha256"));
    }

    #[test]
    fn materialized_runtime_graph_covers_every_imported_companion() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let packages = materialized_runtime_references(&bundle)
            .expect("resolve materialized runtime graph")
            .into_iter()
            .map(|reference| reference.package.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            packages,
            BTreeSet::from([
                "@mermaid-js/layout-elk",
                "@mermaid-js/layout-tidy-tree",
                "@mermaid-js/mermaid-cli",
                "@mermaid-js/mermaid-zenuml",
                "@mermaid-js/parser",
                "@zenuml/core",
                "dompurify",
                "mermaid",
            ])
        );
    }

    #[test]
    fn installed_companion_content_drift_fails_materialized_verification() {
        let temporary = tempfile::tempdir().expect("temporary installed package");
        fs::write(
            temporary.path().join("index.js"),
            "export const value = 1;\n",
        )
        .expect("write companion package");
        let expected = crate::cmd::upstream_svg_package_tree_sha256(temporary.path())
            .expect("hash companion package");
        let mut failures = Vec::new();

        verify_installed_package_content(
            temporary.path(),
            "@mermaid-js/layout-test",
            "1.0.0",
            &expected,
            &mut failures,
        )
        .expect("verify matching companion content");
        assert!(failures.is_empty());

        fs::write(
            temporary.path().join("index.js"),
            "export const value = 2;\n",
        )
        .expect("mutate companion package");
        verify_installed_package_content(
            temporary.path(),
            "@mermaid-js/layout-test",
            "1.0.0",
            &expected,
            &mut failures,
        )
        .expect("verify changed companion content");

        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("@mermaid-js/layout-test@1.0.0"));
        assert!(failures[0].contains("content drift"));
    }

    fn retained_oracle_statuses() -> Vec<(&'static str, &'static str)> {
        ZENUML_ADMISSION_GATES
            .into_iter()
            .map(|gate| {
                if gate == "execution-isolation" {
                    (gate, "pending")
                } else {
                    (gate, "pass")
                }
            })
            .collect()
    }

    #[test]
    fn committed_bundle_has_a_valid_behavior_selection() {
        let path = crate::cmd::workspace_root().join(BUNDLE_RELATIVE_PATH);
        let bundle = load_bundle(&path).expect("load committed Mermaid reference bundle");
        validate_bundle(&bundle).expect("validate committed Mermaid reference bundle");
    }

    #[test]
    fn shared_sanitizer_is_bound_to_every_workspace_graph() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        validate_bundle(&bundle).expect("validate bundle");

        assert_eq!(bundle.sanitizer.package, "dompurify");
        assert_eq!(bundle.sanitizer.role, "sanitizer");
        assert!(
            package_references(&bundle)
                .iter()
                .any(|reference| reference.id == bundle.sanitizer.id)
        );

        let zenuml = bundle
            .external_diagrams
            .iter()
            .find(|diagram| diagram.id == "zenuml")
            .expect("ZenUML diagram reference");
        let selected = selected_behavior_source(&zenuml.behavior_source)
            .expect("selected ZenUML behavior reference");

        for expectation in workspace_graph_expectations(&bundle, zenuml, selected) {
            assert!(
                expectation
                    .required_overrides
                    .iter()
                    .any(|package| package.id == bundle.sanitizer.id)
            );
            assert!(
                expectation
                    .expected_packages
                    .iter()
                    .any(|package| package.id == bundle.sanitizer.id)
            );

            let mut failures = Vec::new();
            verify_workspace_graph(&root, &expectation, &bundle, false, &mut failures)
                .expect("read workspace graph");
            assert!(
                failures.is_empty(),
                "{} workspace graph drifted: {failures:?}",
                expectation.workspace
            );
        }
    }

    #[test]
    fn shared_sanitizer_workspace_contract_rejects_override_or_lock_drift() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let temporary = tempfile::tempdir().expect("temporary workspace root");
        let workspace = "sanitizer-contract";
        let workspace_root = temporary.path().join(workspace);
        fs::create_dir_all(&workspace_root).expect("create temporary workspace");
        fs::write(
            workspace_root.join(".npmrc"),
            "ignore-scripts=true\nregistry=https://registry.npmjs.org/\n",
        )
        .expect("write npm configuration");
        fs::write(
            workspace_root.join("package.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "overrides": { "dompurify": bundle.sanitizer.version.as_str() }
            }))
            .expect("serialize package manifest"),
        )
        .expect("write package manifest");

        let expectation = WorkspaceGraphExpectation {
            workspace,
            direct_dependencies: Vec::new(),
            expected_packages: vec![&bundle.sanitizer],
            required_overrides: vec![&bundle.sanitizer],
        };
        fs::write(
            workspace_root.join("package-lock.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "packages": {
                    "": {},
                    "node_modules/dompurify": {
                        "version": "3.4.11",
                        "integrity": bundle.sanitizer.integrity.as_str(),
                        "resolved": bundle.sanitizer.tarball_url.as_str()
                    }
                }
            }))
            .expect("serialize package lock"),
        )
        .expect("write package lock");

        let mut failures = Vec::new();
        verify_workspace_graph(
            temporary.path(),
            &expectation,
            &bundle,
            false,
            &mut failures,
        )
        .expect("read temporary workspace graph");
        assert!(failures.iter().any(|failure| {
            failure.contains("must resolve dompurify@") && failure.contains("recorded integrity")
        }));

        fs::write(
            workspace_root.join("package-lock.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "packages": {
                    "": {},
                    "node_modules/dompurify": {
                        "version": bundle.sanitizer.version.as_str(),
                        "integrity": bundle.sanitizer.integrity.as_str(),
                        "resolved": bundle.sanitizer.tarball_url.as_str()
                    }
                }
            }))
            .expect("serialize corrected package lock"),
        )
        .expect("write corrected package lock");
        fs::write(
            workspace_root.join("package.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "overrides": { "dompurify": "3.4.11" }
            }))
            .expect("serialize stale package manifest"),
        )
        .expect("write stale package manifest");

        failures.clear();
        verify_workspace_graph(
            temporary.path(),
            &expectation,
            &bundle,
            false,
            &mut failures,
        )
        .expect("read temporary workspace graph");
        assert!(failures.iter().any(|failure| {
            failure.contains("must override dompurify to") && failure.contains("3.4.11")
        }));
    }

    #[test]
    fn committed_projections_are_exact_generator_outputs() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");

        for (relative, expected) in expected_projections(&bundle).expect("render projections") {
            let actual = fs::read_to_string(root.join(relative)).expect("read projection");
            assert_eq!(
                actual, expected,
                "regenerate with `cargo run -p xtask -- gen-mermaid-reference`"
            );
        }
    }

    #[test]
    fn builtin_diagram_inventory_extractor_keeps_upstream_registration_order() {
        let orchestration = r#"
import alpha from '../diagrams/alpha/detector.js';
import { beta } from '../diagrams/beta/detector.js';

const addDiagrams = () => {
  registerDiagram(
    'error',
    {} as never,
  );
  registerLazyLoadedDiagrams(alpha, beta);
};
"#;
        let ids = extract_builtin_diagram_ids(
            orchestration,
            Path::new("packages/mermaid/src/diagram-api/diagram-orchestration.ts"),
            |path| match path.to_string_lossy().as_ref() {
                "packages/mermaid/src/diagrams/alpha/detector.ts" => {
                    Ok("const id = 'alpha';".to_string())
                }
                "packages/mermaid/src/diagrams/beta/detector.ts" => {
                    Ok("const id = 'beta';".to_string())
                }
                other => Err(format!("unexpected detector path {other}")),
            },
        )
        .expect("extract static registry entries");

        assert_eq!(ids, ["error", "alpha", "beta"]);
    }

    #[test]
    fn builtin_diagram_inventory_extractor_fails_closed_on_nonstatic_registration() {
        let source = r#"
const id = 'untracked';
registerDiagram(id, {} as never);
"#;

        let error = extract_builtin_diagram_ids(
            source,
            Path::new("packages/mermaid/src/diagram-api/diagram-orchestration.ts"),
            |_| unreachable!("no detector source should be read"),
        )
        .expect_err("non-static registration must fail");

        assert!(error.contains("registerDiagram call does not use a static string id"));
    }

    #[test]
    fn builtin_layout_inventory_extractor_includes_conditional_default_loaders() {
        let source = r#"
const registerDefaultLayoutLoaders = () => {
  registerLayoutLoaders([
    {
      name: 'dagre',
    },
    ...(injected.includeLargeFeatures
      ? [
          {
            name: 'cose-bilkent',
          },
        ]
      : []),
  ]);
};

registerDefaultLayoutLoaders();
"#;

        assert_eq!(
            extract_builtin_layout_ids(source).expect("extract default layouts"),
            ["dagre", "cose-bilkent"]
        );
    }

    #[test]
    fn builtin_registry_inventory_validation_rejects_duplicate_or_missing_ids() {
        let root = crate::cmd::workspace_root();
        let mut bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        bundle
            .builtin_registry
            .diagrams
            .ids
            .push("error".to_string());
        bundle.builtin_registry.layouts.ids.clear();

        let error = validate_bundle(&bundle).expect_err("invalid registry inventory must fail");
        assert!(
            error
                .to_string()
                .contains("built-in diagram registry inventory contains a duplicate")
        );
        assert!(
            error
                .to_string()
                .contains("built-in layout registry inventory has an invalid source or is empty")
        );
    }

    #[test]
    fn materialized_registry_gate_rejects_an_unrecorded_upstream_registration() {
        let repository_root = crate::cmd::workspace_root();
        let mut bundle =
            load_bundle(&repository_root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let temporary = tempfile::tempdir().expect("temporary checkout root");
        let checkout = temporary.path().join(
            bundle
                .release
                .source
                .checkout_path
                .as_deref()
                .expect("Mermaid checkout path"),
        );
        let diagrams = r#"
import alpha from '../diagrams/alpha/detector.js';
import beta from '../diagrams/beta/detector.js';

registerLazyLoadedDiagrams(alpha, beta);
"#;
        let layouts = r#"
const registerDefaultLayoutLoaders = () => {
  registerLayoutLoaders([
    {
      name: 'dagre',
    },
  ]);
};
registerDefaultLayoutLoaders();
"#;
        let diagram_source_path = checkout.join(&bundle.builtin_registry.diagrams.source_path);
        let layout_source_path = checkout.join(&bundle.builtin_registry.layouts.source_path);
        let alpha_path = checkout.join("packages/mermaid/src/diagrams/alpha/detector.ts");
        let beta_path = checkout.join("packages/mermaid/src/diagrams/beta/detector.ts");
        for path in [
            &diagram_source_path,
            &layout_source_path,
            &alpha_path,
            &beta_path,
        ] {
            fs::create_dir_all(path.parent().expect("source parent"))
                .expect("create source parent");
        }
        fs::write(&diagram_source_path, diagrams).expect("write diagram registry");
        fs::write(&layout_source_path, layouts).expect("write layout registry");
        fs::write(&alpha_path, "const id = 'alpha';").expect("write alpha detector");
        fs::write(&beta_path, "const id = 'beta';").expect("write beta detector");
        bundle.builtin_registry.diagrams.ids = vec!["alpha".to_string(), "beta".to_string()];
        bundle.builtin_registry.diagrams.source_sha256 =
            crate::util::sha256_hex(diagrams.as_bytes());
        bundle.builtin_registry.layouts.ids = vec!["dagre".to_string()];
        bundle.builtin_registry.layouts.source_sha256 = crate::util::sha256_hex(layouts.as_bytes());

        let mut failures = Vec::new();
        verify_builtin_registry_inventory(temporary.path(), &bundle, true, &mut failures);
        assert!(failures.is_empty(), "{failures:?}");

        bundle.builtin_registry.diagrams.ids.pop();
        verify_builtin_registry_inventory(temporary.path(), &bundle, true, &mut failures);
        assert!(failures.iter().any(|failure| {
            failure.contains("built-in diagram registry inventory drift")
                && failure.contains("beta")
        }));
    }

    #[test]
    fn nested_package_lock_entries_are_owned_by_package_identity() {
        let lock = serde_json::json!({
            "packages": {
                "": {},
                "node_modules/@zenuml/core": { "version": "3.50.1" },
                "node_modules/plugin/node_modules/@zenuml/core": { "version": "3.50.1" },
                "node_modules/@zenuml/core-extra": { "version": "3.50.1" }
            }
        });

        let paths = package_lock_entries(&lock, "@zenuml/core")
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            [
                "node_modules/@zenuml/core",
                "node_modules/plugin/node_modules/@zenuml/core"
            ]
        );
    }

    #[test]
    fn package_lock_registry_policy_rejects_mirrors_but_allows_non_registry_sources() {
        let lock = serde_json::json!({
            "packages": {
                "node_modules/official": {
                    "resolved": "https://registry.npmjs.org/official/-/official-1.0.0.tgz"
                },
                "node_modules/mirror": {
                    "resolved": "https://registry.npmmirror.com/mirror/-/mirror-1.0.0.tgz"
                },
                "node_modules/direct-tarball": {
                    "resolved": "https://example.com/direct-tarball-1.0.0.tgz"
                },
                "node_modules/source": {
                    "resolved": "git+https://github.com/example/source.git"
                }
            }
        });
        let mut failures = Vec::new();

        verify_lock_registry_sources(&lock, "test-workspace", &mut failures);

        assert_eq!(failures.len(), 2);
        assert!(failures[0].contains("registry.npmmirror.com"));
        assert!(failures[1].contains("example.com"));
    }

    #[test]
    fn resource_evidence_validates_derivation_without_an_arbitrary_threshold() {
        let evidence = serde_json::json!({
            "oracle": { "runtimeEntryBytes": 100 },
            "candidate": { "runtimeEntryBytes": 1000 },
            "resource": {
                "measurementScope": "runtime-entry",
                "runtimeEntryDeltaBytes": 900,
                "runtimeEntryDeltaBasisPoints": 90000
            }
        });
        let mut failures = Vec::new();

        verify_candidate_resource_evidence(&evidence, &mut failures);

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn resource_evidence_rejects_a_non_reproducible_delta() {
        let evidence = serde_json::json!({
            "oracle": { "runtimeEntryBytes": 1000 },
            "candidate": { "runtimeEntryBytes": 1010 },
            "resource": {
                "measurementScope": "runtime-entry",
                "runtimeEntryDeltaBytes": 9,
                "runtimeEntryDeltaBasisPoints": 100
            }
        });
        let mut failures = Vec::new();

        verify_candidate_resource_evidence(&evidence, &mut failures);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("reproducibly derive"))
        );
    }

    #[test]
    fn candidate_selection_requires_matching_admission_decision() {
        let root = crate::cmd::workspace_root();
        let mut bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let zenuml = bundle
            .external_diagrams
            .iter_mut()
            .find(|diagram| diagram.id == "zenuml")
            .expect("ZenUML reference");
        zenuml.behavior_source.decision = "oracle-retained".to_string();

        let error = validate_bundle(&bundle).expect_err("inconsistent decision must fail");
        assert!(error.to_string().contains("incomplete decision evidence"));
    }

    #[test]
    fn candidate_selection_fails_closed_on_a_pending_gate() {
        let admission = admission_with_statuses(&retained_oracle_statuses());
        let mut failures = Vec::new();

        validate_admission_matrix(&admission, "candidate-selected", &mut failures);

        assert!(failures.iter().any(|failure| {
            failure.contains("cannot be selected until every required gate passes")
        }));
    }

    #[test]
    fn candidate_status_must_match_the_selection_decision() {
        let admission = serde_json::json!({ "candidateStatus": "pending" });
        let mut failures = Vec::new();

        validate_candidate_status(&admission, "candidate-selected", &mut failures);

        assert_eq!(
            failures,
            [
                "ZenUML candidateStatus Some(\"pending\") is inconsistent with decision candidate-selected"
            ]
        );
    }

    #[test]
    fn admission_deltas_require_unique_complete_ownership() {
        let admission = serde_json::json!({
            "classifiedDeltas": [
                {
                    "area": "tooling",
                    "source": "package.json",
                    "classification": "residual",
                    "owner": "upstream",
                    "summary": "An owned residual."
                },
                {
                    "area": "tooling",
                    "source": "",
                    "classification": "residual",
                    "owner": "upstream",
                    "summary": "A duplicate residual."
                }
            ]
        });
        let mut failures = Vec::new();

        validate_admission_deltas(&admission, &mut failures);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("duplicated"))
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("missing source"))
        );
    }

    #[test]
    fn admission_rejects_a_missing_required_gate() {
        let statuses = retained_oracle_statuses()
            .into_iter()
            .filter(|(gate, _)| *gate != "strict-inline-artifact")
            .collect::<Vec<_>>();
        let admission = admission_with_statuses(&statuses);
        let mut failures = Vec::new();

        validate_admission_matrix(&admission, "oracle-retained", &mut failures);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("admission gates must be exactly"))
        );
    }

    #[test]
    fn admission_rejects_a_missing_security_gate() {
        let statuses = retained_oracle_statuses()
            .into_iter()
            .filter(|(gate, _)| *gate != "security")
            .collect::<Vec<_>>();
        let admission = admission_with_statuses(&statuses);
        let mut failures = Vec::new();

        validate_admission_matrix(&admission, "oracle-retained", &mut failures);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("admission gates must be exactly"))
        );
    }

    #[test]
    fn browser_admission_rejects_failed_or_missing_probe_observations() {
        let projects = serde_json::json!(["chromium-desktop", "chromium-mobile"]);
        let required = serde_json::json!([
            {
                "id": "opaque-origin",
                "description": "The realm has an opaque origin."
            }
        ]);
        let failed = serde_json::json!({
            "projectCount": 2,
            "probeCount": 1,
            "observationCount": 2,
            "passedObservationCount": 2,
            "probes": [
                {
                    "id": "opaque-origin",
                    "description": "The realm has an opaque origin.",
                    "observations": [
                        {
                            "project": "chromium-desktop",
                            "testTitle": "opaque realm",
                            "expected": true,
                            "observed": false,
                            "passed": false
                        },
                        {
                            "project": "chromium-mobile",
                            "testTitle": "opaque realm",
                            "expected": true,
                            "observed": true,
                            "passed": true
                        }
                    ]
                }
            ]
        });
        let missing = serde_json::json!({
            "projectCount": 2,
            "probeCount": 1,
            "observationCount": 2,
            "passedObservationCount": 2,
            "probes": [
                {
                    "id": "opaque-origin",
                    "description": "The realm has an opaque origin.",
                    "observations": [
                        {
                            "project": "chromium-desktop",
                            "testTitle": "opaque realm",
                            "expected": true,
                            "observed": true,
                            "passed": true
                        }
                    ]
                }
            ]
        });

        for observed in [&failed, &missing] {
            let mut failures = Vec::new();
            verify_browser_admission_category(
                "security",
                projects.as_array(),
                required.as_array(),
                Some(observed),
                &mut failures,
            );
            assert!(failures.iter().any(|failure| {
                failure.contains("every passing probe observation for every project")
            }));
        }
    }

    #[test]
    fn deferred_major_semantics_reject_provenance_range_status_and_inventory_drift() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let zenuml = bundle
            .external_diagrams
            .iter()
            .find(|diagram| diagram.id == "zenuml")
            .expect("ZenUML reference");
        let admission = read_json(&root.join(&zenuml.behavior_source.admission_evidence))
            .expect("load admission");
        let artifact_path = admission
            .pointer("/excludedLatestMajor/artifact")
            .and_then(JsonValue::as_str)
            .expect("deferred artifact path");
        let artifact = read_json(&root.join(artifact_path)).expect("load deferred artifact");
        let cases = [
            (
                "/latestStable/source/commit",
                serde_json::json!("0000000000000000000000000000000000000000"),
                "source.commit provenance drift",
            ),
            (
                "/hostGraph/pluginDeclaredRange",
                serde_json::json!("^4.0.0"),
                "host graph does not derive",
            ),
            (
                "/status",
                serde_json::json!("admitted"),
                "invalid schema, status, or decision",
            ),
            (
                "/behaviorDeltaInventory",
                serde_json::json!([]),
                "behavior areas must be exactly",
            ),
        ];

        for (pointer, replacement, expected_failure) in cases {
            let mut mutated = artifact.clone();
            *mutated.pointer_mut(pointer).expect("fixture pointer") = replacement;
            let mut failures = Vec::new();
            validate_deferred_major_admission(
                &bundle.release,
                &zenuml.plugin,
                &zenuml.behavior_source,
                &admission,
                &mutated,
                &mut failures,
            );
            assert!(
                failures
                    .iter()
                    .any(|failure| failure.contains(expected_failure)),
                "{pointer} produced {failures:?}"
            );
        }
    }

    #[test]
    fn deferred_major_artifact_fails_closed_when_missing_or_mutated() {
        let repository_root = crate::cmd::workspace_root();
        let bundle = load_bundle(&repository_root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let zenuml = bundle
            .external_diagrams
            .iter()
            .find(|diagram| diagram.id == "zenuml")
            .expect("ZenUML reference");
        let admission =
            read_json(&repository_root.join(&zenuml.behavior_source.admission_evidence))
                .expect("load admission");
        let temporary = tempfile::tempdir().expect("temporary root");
        let mut missing_admission = admission.clone();
        *missing_admission
            .pointer_mut("/excludedLatestMajor/artifact")
            .expect("artifact pointer") = serde_json::json!("missing.json");

        let error = verify_deferred_major_admission(
            temporary.path(),
            &bundle.release,
            &zenuml.plugin,
            &zenuml.behavior_source,
            &missing_admission,
            &mut Vec::new(),
        )
        .expect_err("missing deferred artifact must fail");
        assert!(error.to_string().contains("missing.json"));

        let artifact_path = admission
            .pointer("/excludedLatestMajor/artifact")
            .and_then(JsonValue::as_str)
            .expect("artifact path");
        let destination = temporary.path().join(artifact_path);
        fs::create_dir_all(destination.parent().expect("artifact parent"))
            .expect("create artifact parent");
        let mut artifact =
            read_json(&repository_root.join(artifact_path)).expect("load deferred artifact");
        *artifact.pointer_mut("/status").expect("status pointer") = serde_json::json!("admitted");
        fs::write(
            &destination,
            serde_json::to_vec_pretty(&artifact).expect("serialize artifact"),
        )
        .expect("write mutated artifact");
        let mut failures = Vec::new();
        verify_deferred_major_admission(
            temporary.path(),
            &bundle.release,
            &zenuml.plugin,
            &zenuml.behavior_source,
            &admission,
            &mut failures,
        )
        .expect("mutated JSON remains readable");
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("artifact digest drift"))
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("invalid schema, status, or decision"))
        );
    }

    #[test]
    fn admission_rejects_a_duplicate_gate() {
        let mut statuses = retained_oracle_statuses();
        statuses.push(("corpus", "pass"));
        let admission = admission_with_statuses(&statuses);
        let mut failures = Vec::new();

        validate_admission_matrix(&admission, "oracle-retained", &mut failures);

        assert!(
            failures
                .iter()
                .any(|failure| failure == "duplicate ZenUML admission gate corpus")
        );
    }
}
