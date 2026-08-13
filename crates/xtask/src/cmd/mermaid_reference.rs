use crate::XtaskError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BUNDLE_RELATIVE_PATH: &str = "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json";
const REPOS_LOCK_RELATIVE_PATH: &str = "tools/upstreams/REPOS.lock.json";
const SELECTION_DECISION_RELATIVE_PATH: &str = "tools/upstreams/MERMAID_SELECTION_DECISION.json";
const EXPECTED_BUNDLE_SCHEMA_VERSION: u32 = 6;
const EXPECTED_PROJECTION_SCHEMA_VERSION: u32 = 3;
const EXPECTED_SELECTION_DECISION_SCHEMA_VERSION: u32 = 1;
const CORE_PROJECTION_RELATIVE_PATH: &str = "crates/merman-core/src/generated/mermaid_reference.rs";
const XTASK_PROJECTION_RELATIVE_PATH: &str = "crates/xtask/src/generated/mermaid_reference.rs";
const TYPESCRIPT_PROJECTION_RELATIVE_PATH: &str = "playground/src/generated/mermaid-reference.ts";
const UPSTREAM_SVG_PROVENANCE_RELATIVE_PATH: &str = "fixtures/upstream-svgs";
const OFFICIAL_NPM_REGISTRY_PREFIX: &str = "https://registry.npmjs.org/";
const OFFICIAL_NPM_SIGNATURE_COMMAND: &str =
    "npm audit signatures --json --include-attestations --registry=https://registry.npmjs.org/";
const BUILTIN_DIAGRAM_METADATA_SCRIPT: &str = r#"
import mermaid from 'mermaid';

mermaid.initialize({ startOnLoad: false });
process.stdout.write(
  JSON.stringify(mermaid.getRegisteredDiagramsMetadata().map(({ id }) => id)),
);
"#;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MermaidReferenceBundle {
    schema_version: u32,
    projection_schema_version: u32,
    selection_decision: SelectionDecisionReference,
    release: PackageReference,
    parser: PackageReference,
    sanitizer: PackageReference,
    builtin_registry: BuiltinRegistryInventory,
    external_diagrams: Vec<ExternalDiagramReference>,
    external_layouts: Vec<PackageReference>,
    playground: WorkspaceLocation,
    reference_cli: ReferenceCli,
    renderer_tools: Vec<PackageReference>,
    install_policy: InstallPolicy,
    feature_decision: FeatureDecision,
    generated_outputs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionDecisionReference {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageReference {
    id: String,
    role: String,
    package: String,
    version: String,
    declared_range: Option<String>,
    integrity: String,
    tarball_url: String,
    source: SourceReference,
    required_surfaces: Vec<String>,
    installed_content_sha256: Option<String>,
    #[serde(default)]
    runtime_registration: Option<RuntimeRegistration>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeRegistration {
    module_id: String,
    diagram_aliases: Vec<String>,
    layout_ids: Vec<String>,
    source_path: String,
    source_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuiltinRegistryInventory {
    diagrams: RegistryInventory,
    layouts: RegistryInventory,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryInventory {
    source_path: String,
    source_sha256: String,
    ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceReference {
    repository: String,
    reference: String,
    commit: String,
    checkout_path: Option<String>,
    package_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExternalDiagramReference {
    id: String,
    plugin: PackageReference,
    behavior: PackageReference,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReferenceCli {
    package: PackageReference,
    workspace: String,
    package_json_sha256: String,
    package_lock_sha256: String,
    config_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceLocation {
    workspace: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallPolicy {
    registry: String,
    default_ignore_scripts: bool,
    allowed_actions: Vec<String>,
    known_install_scripts: Vec<InstallScript>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallScript {
    workspace: String,
    package: String,
    action: String,
    reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FeatureDecision {
    add_cargo_feature: bool,
    reason: String,
    evidence: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionDecisionReceipt {
    schema_version: u32,
    receipt_kind: String,
    decision: String,
    #[serde(default)]
    bootstrap: Option<BootstrapDecision>,
    previous: SelectionSnapshot,
    current: SelectionSnapshot,
    changes: Vec<SelectionChange>,
    verification: SelectionVerification,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapDecision {
    reason: String,
    source_evidence_git_object: String,
    source_evidence_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionSnapshot {
    identity_digest: String,
    identity: JsonValue,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionChange {
    path: String,
    previous: JsonValue,
    current: JsonValue,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionVerification {
    tool: OfficialVerificationTool,
    packages: Vec<VerifiedPackage>,
    behavior_result: String,
    raw_output: VerificationOutput,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OfficialVerificationTool {
    name: String,
    version: String,
    command: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerifiedPackage {
    package: String,
    version: String,
    integrity: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerificationOutput {
    kind: String,
    sha256: String,
    official_tool_stdout_archived: bool,
}

#[derive(Debug)]
struct VerifiedSelectionReceipt {
    receipt: SelectionDecisionReceipt,
    reference: SelectionDecisionReference,
    current_identity: JsonValue,
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
    references.extend(bundle.renderer_tools.iter());
    for diagram in &bundle.external_diagrams {
        references.extend([&diagram.plugin, &diagram.behavior]);
    }
    references
}

fn materialized_runtime_references(
    bundle: &MermaidReferenceBundle,
) -> Result<Vec<&PackageReference>, XtaskError> {
    let mut packages = BTreeMap::new();
    for reference in package_references(bundle) {
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
    if bundle.schema_version != EXPECTED_BUNDLE_SCHEMA_VERSION {
        failures.push(format!(
            "schemaVersion must be {EXPECTED_BUNDLE_SCHEMA_VERSION}, found {}",
            bundle.schema_version
        ));
    }
    if bundle.projection_schema_version != EXPECTED_PROJECTION_SCHEMA_VERSION {
        failures.push(format!(
            "projectionSchemaVersion must be {EXPECTED_PROJECTION_SCHEMA_VERSION}, found {}",
            bundle.projection_schema_version
        ));
    }
    if bundle.selection_decision.path != SELECTION_DECISION_RELATIVE_PATH
        || !crate::util::is_canonical_sha256(&bundle.selection_decision.sha256)
        || bundle
            .selection_decision
            .sha256
            .bytes()
            .all(|byte| byte == b'0')
    {
        failures.push(format!(
            "selectionDecision must bind {SELECTION_DECISION_RELATIVE_PATH} by SHA-256"
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
    if zenuml.behavior.package != "@zenuml/core"
        || zenuml.behavior.role != "external-diagram-behavior"
    {
        failures.push("ZenUML must name one selected behavior package".to_string());
    }
    if bundle.external_layouts.is_empty()
        || bundle
            .external_layouts
            .iter()
            .any(|reference| reference.role != "external-layout")
    {
        failures.push(
            "externalLayouts must contain only selected external-layout packages".to_string(),
        );
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
    let renderer_tools = bundle
        .renderer_tools
        .iter()
        .map(|reference| reference.package.as_str())
        .collect::<BTreeSet<_>>();
    if renderer_tools != BTreeSet::from(["@puppeteer/browsers", "puppeteer", "puppeteer-core"])
        || bundle
            .renderer_tools
            .iter()
            .any(|reference| reference.role != "renderer-tool")
    {
        failures.push(
            "rendererTools must bind the selected Puppeteer browser-driver closure".to_string(),
        );
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
    for reference in package_references(bundle) {
        if reference.id.trim().is_empty()
            || reference.role.trim().is_empty()
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
                "{}@{} has incomplete selected identity/source provenance",
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
        {
            failures.push(format!(
                "{}@{} must record its official npm tarball",
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
        ("ZENUML_CORE_VERSION", zenuml.behavior.version.as_str()),
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
    mermaid_runtime_verified: bool,
    failures: &mut Vec<String>,
) {
    let Some(checkout_path) = bundle.release.source.checkout_path.as_deref() else {
        failures.push(
            "Mermaid release must materialize a source checkout for registry inventory".to_string(),
        );
        return;
    };
    let checkout = root.join(checkout_path);
    if !checkout.is_dir() {
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
        let (actual_ids, inventory_path) = if kind == "built-in diagram" {
            if !mermaid_runtime_verified {
                continue;
            }
            (
                installed_builtin_diagram_ids(root, bundle),
                root.join(&bundle.reference_cli.workspace)
                    .join("node_modules")
                    .join(&bundle.release.package),
            )
        } else {
            (extract_builtin_layout_ids(&source), source_path.clone())
        };
        match actual_ids {
            Ok(actual_ids) if actual_ids != registry.ids => failures.push(format!(
                "{kind} registry inventory drift at {}: expected {:?}, found {actual_ids:?}",
                inventory_path.display(),
                registry.ids
            )),
            Ok(_) => {}
            Err(reason) => failures.push(format!(
                "cannot extract {kind} registry inventory from {}: {reason}",
                inventory_path.display()
            )),
        }
    }
}

fn installed_builtin_diagram_ids(
    root: &Path,
    bundle: &MermaidReferenceBundle,
) -> Result<Vec<String>, String> {
    let workspace = root.join(&bundle.reference_cli.workspace);
    let output = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(BUILTIN_DIAGRAM_METADATA_SCRIPT)
        .current_dir(&workspace)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute Mermaid {} diagram metadata API from {}: {error}",
                bundle.release.version,
                workspace.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "Mermaid {} diagram metadata API failed: {}",
            bundle.release.version,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let ids = serde_json::from_slice::<Vec<String>>(&output.stdout).map_err(|error| {
        format!(
            "Mermaid {} diagram metadata API returned invalid JSON: {error}",
            bundle.release.version
        )
    })?;
    ensure_unique_registry_ids(&ids, "diagram")?;
    if ids.is_empty() {
        return Err("Mermaid diagram metadata API returned no registered diagrams".to_string());
    }
    Ok(ids)
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
) -> Vec<WorkspaceGraphExpectation<'a>> {
    let layout_refs = bundle.external_layouts.iter().collect::<Vec<_>>();
    let mut playground_direct = vec![&bundle.release, &zenuml.plugin];
    playground_direct.extend(layout_refs.iter().copied());
    let mut playground_expected = vec![
        &bundle.release,
        &bundle.parser,
        &zenuml.plugin,
        &zenuml.behavior,
        &bundle.sanitizer,
    ];
    playground_expected.extend(layout_refs.iter().copied());
    let puppeteer = bundle
        .renderer_tools
        .iter()
        .find(|reference| reference.package == "puppeteer")
        .expect("validated rendererTools must contain puppeteer");
    let mut cli_direct = vec![&bundle.reference_cli.package, &zenuml.plugin, puppeteer];
    cli_direct.extend(layout_refs.iter().copied());
    let mut cli_expected = vec![
        &bundle.reference_cli.package,
        &bundle.release,
        &bundle.parser,
        &zenuml.plugin,
        &zenuml.behavior,
        &bundle.sanitizer,
    ];
    cli_expected.extend(layout_refs.iter().copied());
    cli_expected.extend(bundle.renderer_tools.iter());
    vec![
        WorkspaceGraphExpectation {
            workspace: "playground",
            direct_dependencies: playground_direct,
            expected_packages: playground_expected,
            required_overrides: vec![&zenuml.behavior, &bundle.sanitizer],
        },
        WorkspaceGraphExpectation {
            workspace: &bundle.reference_cli.workspace,
            direct_dependencies: cli_direct,
            expected_packages: cli_expected,
            required_overrides: vec![&zenuml.behavior, &bundle.sanitizer],
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

fn verify_installed_content(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    failures: &mut Vec<String>,
) -> Result<bool, XtaskError> {
    let node_modules = root.join("tools/mermaid-cli/node_modules");
    let mut mermaid_runtime_verified = false;
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
        let package_verified = verify_installed_package_content(
            &package_root,
            &reference.package,
            &reference.version,
            expected,
            failures,
        )?;
        if reference.package == bundle.release.package {
            mermaid_runtime_verified = package_verified;
        }
    }
    Ok(mermaid_runtime_verified)
}

fn verify_installed_package_content(
    package_root: &Path,
    package: &str,
    version: &str,
    expected: &str,
    failures: &mut Vec<String>,
) -> Result<bool, XtaskError> {
    let actual = crate::cmd::upstream_svg_package_tree_sha256(package_root)?;
    let verified = actual == expected;
    if !verified {
        failures.push(format!(
            "installed {package}@{version} content drift at {}: expected {expected}, found {actual}",
            package_root.display()
        ));
    }
    Ok(verified)
}

fn canonical_json(value: &JsonValue) -> Result<Vec<u8>, XtaskError> {
    let mut canonical = value.clone();
    canonical.sort_all_objects();
    serde_json::to_vec(&canonical).map_err(XtaskError::from)
}

fn package_selection_identity(value: &JsonValue) -> Result<JsonValue, XtaskError> {
    let field = |name: &str| {
        value.get(name).cloned().ok_or_else(|| {
            XtaskError::MermaidReference(format!("selected package identity is missing {name}"))
        })
    };
    let source = value
        .get("source")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| {
            XtaskError::MermaidReference("selected package identity is missing source".to_string())
        })?;
    Ok(serde_json::json!({
        "role": field("role")?,
        "package": field("package")?,
        "version": field("version")?,
        "declaredRange": value.get("declaredRange").cloned().unwrap_or(JsonValue::Null),
        "integrity": field("integrity")?,
        "tarballUrl": field("tarballUrl")?,
        "source": {
            "repository": source.get("repository").cloned().unwrap_or(JsonValue::Null),
            "reference": source.get("reference").cloned().unwrap_or(JsonValue::Null),
            "commit": source.get("commit").cloned().unwrap_or(JsonValue::Null),
            "packagePath": source.get("packagePath").cloned().unwrap_or(JsonValue::Null),
        },
        "requiredSurfaces": field("requiredSurfaces")?,
        "runtimeRegistration": value
            .get("runtimeRegistration")
            .cloned()
            .unwrap_or(JsonValue::Null),
    }))
}

fn selection_identity_from_bundle_value(bundle: &JsonValue) -> Result<JsonValue, XtaskError> {
    let package = |pointer: &str| {
        let value = bundle.pointer(pointer).ok_or_else(|| {
            XtaskError::MermaidReference(format!(
                "reference bundle is missing selected package at {pointer}"
            ))
        })?;
        package_selection_identity(value)
    };
    let diagrams = bundle
        .get("externalDiagrams")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            XtaskError::MermaidReference("reference bundle is missing externalDiagrams".to_string())
        })?;
    let mut selected_diagrams = Vec::new();
    for diagram in diagrams {
        let id = diagram.get("id").cloned().ok_or_else(|| {
            XtaskError::MermaidReference("external diagram is missing id".to_string())
        })?;
        let plugin = diagram.get("plugin").ok_or_else(|| {
            XtaskError::MermaidReference("external diagram is missing plugin".to_string())
        })?;
        let behavior = if let Some(behavior) = diagram.get("behavior") {
            behavior
        } else {
            let legacy = diagram.get("behaviorSource").ok_or_else(|| {
                XtaskError::MermaidReference(
                    "external diagram is missing selected behavior".to_string(),
                )
            })?;
            let selected_version = legacy
                .get("selectedVersion")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    XtaskError::MermaidReference(
                        "legacy behavior source is missing selectedVersion".to_string(),
                    )
                })?;
            ["oracle", "candidate", "selected"]
                .into_iter()
                .filter_map(|field| legacy.get(field))
                .find(|reference| {
                    reference.get("version").and_then(JsonValue::as_str) == Some(selected_version)
                })
                .ok_or_else(|| {
                    XtaskError::MermaidReference(
                        "legacy selectedVersion does not identify a package".to_string(),
                    )
                })?
        };
        selected_diagrams.push(serde_json::json!({
            "id": id,
            "plugin": package_selection_identity(plugin)?,
            "behavior": package_selection_identity(behavior)?,
        }));
    }
    selected_diagrams.sort_by(|left, right| {
        left.get("id")
            .and_then(JsonValue::as_str)
            .cmp(&right.get("id").and_then(JsonValue::as_str))
    });
    let layouts = bundle
        .get("externalLayouts")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            XtaskError::MermaidReference("reference bundle is missing externalLayouts".to_string())
        })?;
    let mut selected_layouts = layouts
        .iter()
        .map(package_selection_identity)
        .collect::<Result<Vec<_>, _>>()?;
    selected_layouts.sort_by(|left, right| {
        left.get("package")
            .and_then(JsonValue::as_str)
            .cmp(&right.get("package").and_then(JsonValue::as_str))
    });
    let mut identity = serde_json::json!({
        "release": package("/release")?,
        "parser": package("/parser")?,
        "sanitizer": package("/sanitizer")?,
        "externalDiagrams": selected_diagrams,
        "externalLayouts": selected_layouts,
        "referenceCli": package("/referenceCli/package")?,
    });
    if let Some(renderer_tools) = bundle.get("rendererTools").and_then(JsonValue::as_array) {
        let mut selected_renderer_tools = renderer_tools
            .iter()
            .map(package_selection_identity)
            .collect::<Result<Vec<_>, _>>()?;
        selected_renderer_tools.sort_by(|left, right| {
            left.get("package")
                .and_then(JsonValue::as_str)
                .cmp(&right.get("package").and_then(JsonValue::as_str))
        });
        identity
            .as_object_mut()
            .expect("selection identity is an object")
            .insert(
                "rendererTools".to_string(),
                JsonValue::Array(selected_renderer_tools),
            );
    }
    Ok(identity)
}

fn selection_identity(bundle: &MermaidReferenceBundle) -> Result<JsonValue, XtaskError> {
    selection_identity_from_bundle_value(&serde_json::to_value(bundle)?)
}

fn selection_identity_digest(identity: &JsonValue) -> Result<String, XtaskError> {
    Ok(crate::util::sha256_hex(&canonical_json(identity)?))
}

fn diff_selection_values(
    previous: &JsonValue,
    current: &JsonValue,
    path: &str,
    changes: &mut Vec<SelectionChange>,
) {
    match (previous, current) {
        (JsonValue::Object(previous), JsonValue::Object(current)) => {
            let keys = previous
                .keys()
                .chain(current.keys())
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            for key in keys {
                let next = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                diff_selection_values(
                    previous.get(key).unwrap_or(&JsonValue::Null),
                    current.get(key).unwrap_or(&JsonValue::Null),
                    &next,
                    changes,
                );
            }
        }
        (JsonValue::Array(previous), JsonValue::Array(current))
            if previous.len() == current.len() =>
        {
            for (index, (previous, current)) in previous.iter().zip(current).enumerate() {
                diff_selection_values(previous, current, &format!("{path}[{index}]"), changes);
            }
        }
        _ if previous != current => changes.push(SelectionChange {
            path: path.to_string(),
            previous: previous.clone(),
            current: current.clone(),
        }),
        _ => {}
    }
}

fn selection_changes(previous: &JsonValue, current: &JsonValue) -> Vec<SelectionChange> {
    let mut changes = Vec::new();
    diff_selection_values(previous, current, "", &mut changes);
    changes
}

type SelectionPackageIdentity = (String, String, String);

fn collect_selection_packages(
    value: &JsonValue,
    path: &str,
    packages: &mut BTreeMap<String, (JsonValue, SelectionPackageIdentity)>,
) {
    match value {
        JsonValue::Object(object) => {
            let identity = object
                .get("package")
                .and_then(JsonValue::as_str)
                .zip(object.get("version").and_then(JsonValue::as_str))
                .zip(object.get("integrity").and_then(JsonValue::as_str))
                .map(|((package, version), integrity)| {
                    (
                        package.to_string(),
                        version.to_string(),
                        integrity.to_string(),
                    )
                });
            if let Some(identity) = identity {
                packages.insert(path.to_string(), (value.clone(), identity));
                return;
            }
            for (key, child) in object {
                let next = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                collect_selection_packages(child, &next, packages);
            }
        }
        JsonValue::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                collect_selection_packages(child, &format!("{path}[{index}]"), packages);
            }
        }
        _ => {}
    }
}

fn changed_current_selection_packages(
    previous: &JsonValue,
    current: &JsonValue,
) -> BTreeSet<SelectionPackageIdentity> {
    let mut previous_packages = BTreeMap::new();
    let mut current_packages = BTreeMap::new();
    collect_selection_packages(previous, "", &mut previous_packages);
    collect_selection_packages(current, "", &mut current_packages);
    current_packages
        .into_iter()
        .filter_map(|(path, (value, identity))| {
            (previous_packages.get(&path).map(|(value, _)| value) != Some(&value))
                .then_some(identity)
        })
        .collect()
}

fn validate_selection_snapshot(
    name: &str,
    snapshot: &SelectionSnapshot,
    failures: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let digest = selection_identity_digest(&snapshot.identity)?;
    if snapshot.identity_digest != digest {
        failures.push(format!(
            "selection receipt {name}.identityDigest does not match its identity"
        ));
    }
    Ok(())
}

fn verify_selection_receipt(
    root: &Path,
    bundle: &MermaidReferenceBundle,
) -> Result<VerifiedSelectionReceipt, XtaskError> {
    let receipt_path = root.join(&bundle.selection_decision.path);
    let bytes = fs::read(&receipt_path).map_err(|source| XtaskError::ReadFile {
        path: receipt_path.display().to_string(),
        source,
    })?;
    let actual_sha256 = crate::util::sha256_hex(&bytes);
    let mut failures = Vec::new();
    if actual_sha256 != bundle.selection_decision.sha256 {
        failures.push(format!(
            "selection receipt digest drift: expected {}, found {actual_sha256}",
            bundle.selection_decision.sha256
        ));
    }
    let receipt: SelectionDecisionReceipt = serde_json::from_slice(&bytes)?;
    if receipt.schema_version != EXPECTED_SELECTION_DECISION_SCHEMA_VERSION
        || receipt.receipt_kind != "mermaid-selection-decision"
        || receipt.decision != "selected"
    {
        failures.push("selection receipt has an invalid schema, kind, or decision".to_string());
    }
    validate_selection_snapshot("previous", &receipt.previous, &mut failures)?;
    validate_selection_snapshot("current", &receipt.current, &mut failures)?;
    let current_identity = selection_identity(bundle)?;
    let current_digest = selection_identity_digest(&current_identity)?;
    if receipt.current.identity != current_identity
        || receipt.current.identity_digest != current_digest
    {
        failures.push(
            "selection receipt current identity does not bind the selected reference graph"
                .to_string(),
        );
    }
    let expected_changes = selection_changes(&receipt.previous.identity, &receipt.current.identity);
    if receipt.changes != expected_changes || receipt.changes.is_empty() {
        failures.push(
            "selection receipt changes must be the exact non-empty previous/current identity diff"
                .to_string(),
        );
    }
    if let Some(bootstrap) = &receipt.bootstrap {
        let valid_git_object = bootstrap
            .source_evidence_git_object
            .split_once(':')
            .is_some_and(|(commit, path)| commit.len() == 40 && is_owned_relative_path(path));
        if bootstrap.reason.trim().is_empty()
            || !valid_git_object
            || !crate::util::is_canonical_sha256(&bootstrap.source_evidence_sha256)
            || bootstrap
                .source_evidence_sha256
                .bytes()
                .all(|byte| byte == b'0')
        {
            failures.push("selection receipt bootstrap evidence is incomplete".to_string());
        }
        if receipt.verification.raw_output.sha256 != bootstrap.source_evidence_sha256 {
            failures.push(
                "bootstrap receipt raw output digest must bind its historical aggregate evidence"
                    .to_string(),
            );
        }
    }
    if receipt.verification.tool.name != "npm"
        || receipt.verification.tool.version.trim().is_empty()
        || receipt.verification.tool.command != OFFICIAL_NPM_SIGNATURE_COMMAND
    {
        failures.push(
            "selection receipt must name the exact official npm signature command and version"
                .to_string(),
        );
    }
    let expected_output_kind = if receipt.bootstrap.is_some() {
        "historical-admission-aggregate"
    } else {
        "official-tool-output"
    };
    if receipt.verification.behavior_result != "pass"
        || receipt.verification.raw_output.kind != expected_output_kind
        || receipt
            .verification
            .raw_output
            .official_tool_stdout_archived
            != receipt.bootstrap.is_none()
        || !crate::util::is_canonical_sha256(&receipt.verification.raw_output.sha256)
        || receipt
            .verification
            .raw_output
            .sha256
            .bytes()
            .all(|byte| byte == b'0')
        || receipt.verification.packages.is_empty()
    {
        failures.push(
            "selection receipt must bind passing behavior and the declared admission output"
                .to_string(),
        );
    }
    let selected_packages = package_references(bundle)
        .into_iter()
        .map(|reference| {
            (
                reference.package.as_str(),
                reference.version.as_str(),
                reference.integrity.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut verified_package_names = BTreeSet::new();
    let mut verified_package_identities = BTreeSet::new();
    for package in &receipt.verification.packages {
        if !selected_packages.contains(&(
            package.package.as_str(),
            package.version.as_str(),
            package.integrity.as_str(),
        )) || !verified_package_names.insert(package.package.as_str())
        {
            failures.push(format!(
                "selection receipt verification package {}@{} does not match the selected graph",
                package.package, package.version
            ));
        }
        verified_package_identities.insert((
            package.package.clone(),
            package.version.clone(),
            package.integrity.clone(),
        ));
    }
    for (package, version, _) in
        changed_current_selection_packages(&receipt.previous.identity, &receipt.current.identity)
            .difference(&verified_package_identities)
    {
        failures.push(format!(
            "selection receipt does not bind official verification for changed package {package}@{version}"
        ));
    }
    if failures.is_empty() {
        Ok(VerifiedSelectionReceipt {
            receipt,
            reference: bundle.selection_decision.clone(),
            current_identity,
        })
    } else {
        Err(XtaskError::MermaidReference(failures.join("\n")))
    }
}

fn receipt_reference_from_bundle_value(bundle: &JsonValue) -> Option<SelectionDecisionReference> {
    serde_json::from_value(bundle.get("selectionDecision")?.clone()).ok()
}

fn validate_selection_transition(
    base_identity: &JsonValue,
    base_reference: Option<&SelectionDecisionReference>,
    verified: &VerifiedSelectionReceipt,
) -> Result<(), XtaskError> {
    let base_digest = selection_identity_digest(base_identity)?;
    let current_digest = selection_identity_digest(&verified.current_identity)?;
    let mut failures = Vec::new();
    if base_digest == current_digest {
        if let Some(base_reference) = base_reference {
            if base_reference != &verified.reference {
                failures.push(
                    "selected identity is unchanged, so the selection receipt must not be replaced"
                        .to_string(),
                );
            }
        } else if verified.receipt.bootstrap.is_none() {
            failures.push(
                "selected identity is unchanged and the base has no receipt; only an explicit bootstrap receipt is allowed"
                    .to_string(),
            );
        }
    } else {
        if verified.receipt.bootstrap.is_some() {
            failures.push(
                "bootstrap receipts cannot authorize a selected identity change from the trusted base"
                    .to_string(),
            );
        }
        if verified.receipt.previous.identity != *base_identity
            || verified.receipt.previous.identity_digest != base_digest
            || verified.receipt.current.identity_digest != current_digest
            || verified.receipt.changes
                != selection_changes(base_identity, &verified.current_identity)
        {
            failures.push(
                "selection receipt previous/current identities and changes do not match the trusted base transition"
                    .to_string(),
            );
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::MermaidReference(failures.join("\n")))
    }
}

fn verify_bootstrap_evidence_from_git(
    root: &Path,
    base_commit: &str,
    bootstrap: &BootstrapDecision,
) -> Result<(), XtaskError> {
    let (evidence_commit, _) = bootstrap
        .source_evidence_git_object
        .split_once(':')
        .ok_or_else(|| {
            XtaskError::MermaidReference(
                "bootstrap receipt has an invalid historical evidence object".to_string(),
            )
        })?;
    let ancestry = Command::new("git")
        .args(["merge-base", "--is-ancestor", evidence_commit, base_commit])
        .current_dir(root)
        .output()
        .map_err(|source| XtaskError::ReadFile {
            path: format!("{evidence_commit}..{base_commit}"),
            source,
        })?;
    if !ancestry.status.success() {
        if ancestry.status.code() == Some(1) {
            return Err(XtaskError::MermaidReference(format!(
                "bootstrap evidence commit {evidence_commit} is not an ancestor of trusted base {base_commit}"
            )));
        }
        return Err(XtaskError::MermaidReference(format!(
            "cannot verify bootstrap evidence ancestry for trusted base {base_commit}: {}",
            String::from_utf8_lossy(&ancestry.stderr).trim()
        )));
    }
    let evidence_output = Command::new("git")
        .args(["show", &bootstrap.source_evidence_git_object])
        .current_dir(root)
        .output()
        .map_err(|source| XtaskError::ReadFile {
            path: bootstrap.source_evidence_git_object.clone(),
            source,
        })?;
    if !evidence_output.status.success() {
        return Err(XtaskError::MermaidReference(format!(
            "cannot read bootstrap evidence {}: {}",
            bootstrap.source_evidence_git_object,
            String::from_utf8_lossy(&evidence_output.stderr).trim()
        )));
    }
    let evidence_sha256 = crate::util::sha256_hex(&evidence_output.stdout);
    if evidence_sha256 != bootstrap.source_evidence_sha256 {
        return Err(XtaskError::MermaidReference(format!(
            "bootstrap evidence digest drift: expected {}, found {evidence_sha256}",
            bootstrap.source_evidence_sha256
        )));
    }
    Ok(())
}

fn verify_selection_transition_from_git(
    root: &Path,
    base: &str,
    verified: &VerifiedSelectionReceipt,
) -> Result<(), XtaskError> {
    let commit_expression = format!("{base}^{{commit}}");
    let commit_output = Command::new("git")
        .args(["rev-parse", "--verify", &commit_expression])
        .current_dir(root)
        .output()
        .map_err(|source| XtaskError::ReadFile {
            path: commit_expression.clone(),
            source,
        })?;
    if !commit_output.status.success() {
        return Err(XtaskError::MermaidReference(format!(
            "cannot resolve trusted base {base}: {}",
            String::from_utf8_lossy(&commit_output.stderr).trim()
        )));
    }
    let base_commit = String::from_utf8_lossy(&commit_output.stdout)
        .trim()
        .to_string();
    let object = format!("{base}:{BUNDLE_RELATIVE_PATH}");
    let output = Command::new("git")
        .args(["show", &object])
        .current_dir(root)
        .output()
        .map_err(|source| XtaskError::ReadFile {
            path: object.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(XtaskError::MermaidReference(format!(
            "cannot read trusted base bundle {object}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let base_bundle: JsonValue = serde_json::from_slice(&output.stdout)?;
    let base_identity = selection_identity_from_bundle_value(&base_bundle)?;
    let base_reference = receipt_reference_from_bundle_value(&base_bundle);
    validate_selection_transition(&base_identity, base_reference.as_ref(), verified)?;
    if let Some(bootstrap) = &verified.receipt.bootstrap {
        verify_bootstrap_evidence_from_git(root, &base_commit, bootstrap)?;
    }
    Ok(())
}

fn verify_repository_state(
    root: &Path,
    bundle: &MermaidReferenceBundle,
    materialized: bool,
) -> Result<(), XtaskError> {
    let mut failures = Vec::new();
    verify_reference_cli_files(root, bundle, &mut failures)?;
    verify_upstream_provenance_sources(root, bundle, &mut failures)?;
    verify_source_checkouts(root, bundle, materialized, &mut failures)?;
    let zenuml = bundle
        .external_diagrams
        .iter()
        .find(|diagram| diagram.id == "zenuml")
        .ok_or_else(|| XtaskError::MermaidReference("missing ZenUML reference".to_string()))?;
    for expectation in workspace_graph_expectations(bundle, zenuml) {
        verify_workspace_graph(root, &expectation, bundle, materialized, &mut failures)?;
    }
    if materialized {
        let mermaid_runtime_verified = verify_installed_content(root, bundle, &mut failures)?;
        verify_builtin_registry_inventory(root, bundle, mermaid_runtime_verified, &mut failures);
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
    verify_selection_receipt(&crate::cmd::workspace_root(), &bundle)?;
    for (relative, contents) in expected_projections(&bundle)? {
        write_projection(&crate::cmd::workspace_root().join(relative), &contents)?;
    }
    if refresh_provenance {
        regenerate_upstream_provenance()?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct VerifyOptions {
    materialized: bool,
    base: Option<String>,
}

fn parse_verify_options(args: Vec<String>) -> Result<VerifyOptions, XtaskError> {
    let mut options = VerifyOptions::default();
    let mut arguments = args.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--materialized" if !options.materialized => options.materialized = true,
            "--base" if options.base.is_none() => {
                let base = arguments.next().ok_or(XtaskError::Usage)?;
                if base.trim().is_empty() || base.starts_with('-') {
                    return Err(XtaskError::Usage);
                }
                options.base = Some(base);
            }
            _ => return Err(XtaskError::Usage),
        }
    }
    Ok(options)
}

pub(crate) fn verify_mermaid_reference(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_verify_options(args)?;
    let root = crate::cmd::workspace_root();
    let path = root.join(BUNDLE_RELATIVE_PATH);
    let bundle = load_bundle(&path)?;
    validate_bundle(&bundle)?;
    let verified_receipt = verify_selection_receipt(&root, &bundle)?;
    if let Some(base) = options.base {
        verify_selection_transition_from_git(&root, &base, &verified_receipt)?;
    }
    verify_repository_state(&root, &bundle, options.materialized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git fixture command");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn bootstrap_git_fixture() -> (tempfile::TempDir, BootstrapDecision, String, String) {
        let temporary = tempfile::tempdir().expect("temporary Git repository");
        run_git(temporary.path(), &["init", "--quiet"]);
        run_git(
            temporary.path(),
            &["config", "user.email", "merman-tests@example.invalid"],
        );
        run_git(temporary.path(), &["config", "user.name", "Merman Tests"]);
        let evidence = b"historical admission aggregate\n";
        fs::write(temporary.path().join("evidence.txt"), evidence).expect("write evidence fixture");
        run_git(temporary.path(), &["add", "evidence.txt"]);
        run_git(temporary.path(), &["commit", "--quiet", "-m", "evidence"]);
        let evidence_commit = run_git(temporary.path(), &["rev-parse", "HEAD"]);
        run_git(
            temporary.path(),
            &["commit", "--quiet", "--allow-empty", "-m", "trusted base"],
        );
        let descendant = run_git(temporary.path(), &["rev-parse", "HEAD"]);
        let tree = run_git(temporary.path(), &["rev-parse", "HEAD^{tree}"]);
        let unrelated = run_git(
            temporary.path(),
            &["commit-tree", &tree, "-m", "unrelated base"],
        );
        let bootstrap = BootstrapDecision {
            reason: "test bootstrap".to_string(),
            source_evidence_git_object: format!("{evidence_commit}:evidence.txt"),
            source_evidence_sha256: crate::util::sha256_hex(evidence),
        };
        (temporary, bootstrap, descendant, unrelated)
    }

    #[test]
    fn committed_bundle_contains_only_the_selected_behavior_package() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        validate_bundle(&bundle).expect("validate selected bundle");
        let packages = package_references(&bundle)
            .into_iter()
            .map(|reference| (reference.package.as_str(), reference.version.as_str()))
            .collect::<BTreeSet<_>>();
        assert!(packages.contains(&("@zenuml/core", "3.50.1")));
        assert!(!packages.contains(&("@zenuml/core", "3.47.8")));
    }

    #[test]
    fn committed_receipt_binds_the_selected_identity() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        verify_selection_receipt(&root, &bundle).expect("verify committed receipt");
    }

    #[test]
    fn standing_verification_does_not_require_candidate_artifacts() {
        let root = crate::cmd::workspace_root();
        for relative in [
            "tools/upstreams/ZENUML_CORE_ADMISSION.json",
            "tools/upstreams/ZENUML_CORE_CANDIDATE_EVIDENCE.json",
            "tools/upstreams/ZENUML_CORE_V4_DEFERRED_ADMISSION.json",
            "tools/upstreams/ZENUML_BROWSER_SECURITY_EVIDENCE.json",
        ] {
            assert!(
                !root.join(relative).exists(),
                "{relative} must stay non-standing"
            );
        }
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        validate_bundle(&bundle).expect("validate selected bundle");
        verify_selection_receipt(&root, &bundle).expect("verify selected receipt");
        verify_repository_state(&root, &bundle, false)
            .expect("verify standing selected graph without candidate artifacts");
    }

    #[test]
    fn selected_identity_drift_invalidates_the_receipt() {
        let root = crate::cmd::workspace_root();
        let mut bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        bundle.release.version = "11.16.2".to_string();
        let error = verify_selection_receipt(&root, &bundle)
            .expect_err("selected identity drift must invalidate the receipt");
        assert!(error.to_string().contains("current identity"));
    }

    #[test]
    fn missing_selection_receipt_fails_closed() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let temporary = tempfile::tempdir().expect("temporary receipt root");
        let error = verify_selection_receipt(temporary.path(), &bundle)
            .expect_err("missing receipt must fail");
        assert!(
            error
                .to_string()
                .contains("MERMAID_SELECTION_DECISION.json")
        );
    }

    #[test]
    fn tampered_selection_receipt_fails_closed() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let temporary = tempfile::tempdir().expect("temporary receipt root");
        let target = temporary.path().join(SELECTION_DECISION_RELATIVE_PATH);
        fs::create_dir_all(target.parent().expect("receipt parent"))
            .expect("create receipt parent");
        let mut receipt =
            fs::read(root.join(SELECTION_DECISION_RELATIVE_PATH)).expect("read committed receipt");
        receipt.push(b'\n');
        fs::write(target, receipt).expect("write tampered receipt");
        let error = verify_selection_receipt(temporary.path(), &bundle)
            .expect_err("tampered receipt must fail");
        assert!(error.to_string().contains("receipt digest drift"));
    }

    #[test]
    fn selection_diff_is_exact_and_sorted() {
        let previous = serde_json::json!({"package": {"version": "1.0.0", "integrity": "a"}});
        let current = serde_json::json!({"package": {"version": "1.1.0", "integrity": "b"}});
        let changes = selection_changes(&previous, &current);
        assert_eq!(
            changes
                .iter()
                .map(|change| change.path.as_str())
                .collect::<Vec<_>>(),
            ["package.integrity", "package.version"]
        );
    }

    #[test]
    fn transition_rejects_a_base_mismatch() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let verified = verify_selection_receipt(&root, &bundle).expect("verify receipt");
        let wrong_base = serde_json::json!({"unrelated": true});
        let error = validate_selection_transition(&wrong_base, None, &verified)
            .expect_err("wrong base must fail");
        assert!(error.to_string().contains("trusted base"));
    }

    #[test]
    fn bootstrap_evidence_accepts_an_ancestor_of_the_trusted_base() {
        let (temporary, bootstrap, descendant, _) = bootstrap_git_fixture();
        verify_bootstrap_evidence_from_git(temporary.path(), &descendant, &bootstrap)
            .expect("ancestor evidence must authorize the bootstrap base");
    }

    #[test]
    fn bootstrap_evidence_rejects_an_unrelated_trusted_base() {
        let (temporary, bootstrap, _, unrelated) = bootstrap_git_fixture();
        let error = verify_bootstrap_evidence_from_git(temporary.path(), &unrelated, &bootstrap)
            .expect_err("unrelated base must fail bootstrap ancestry");
        assert!(error.to_string().contains("is not an ancestor"));
    }

    #[test]
    fn unchanged_selection_rejects_receipt_replacement_after_bootstrap() {
        let root = crate::cmd::workspace_root();
        let bundle = load_bundle(&root.join(BUNDLE_RELATIVE_PATH)).expect("load bundle");
        let verified = verify_selection_receipt(&root, &bundle).expect("verify receipt");
        let replaced = SelectionDecisionReference {
            path: verified.reference.path.clone(),
            sha256: "a".repeat(64),
        };
        let error =
            validate_selection_transition(&verified.current_identity, Some(&replaced), &verified)
                .expect_err("unchanged selection must retain its receipt");
        assert!(error.to_string().contains("must not be replaced"));
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
    fn materialized_runtime_graph_covers_every_selected_companion() {
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
                "@puppeteer/browsers",
                "@zenuml/core",
                "dompurify",
                "mermaid",
                "puppeteer",
                "puppeteer-core",
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
        assert!(
            verify_installed_package_content(
                temporary.path(),
                "@mermaid-js/layout-test",
                "1.0.0",
                &expected,
                &mut failures,
            )
            .expect("verify matching companion content")
        );
        fs::write(
            temporary.path().join("index.js"),
            "export const value = 2;\n",
        )
        .expect("mutate companion package");
        assert!(
            !verify_installed_package_content(
                temporary.path(),
                "@mermaid-js/layout-test",
                "1.0.0",
                &expected,
                &mut failures,
            )
            .expect("verify changed companion content")
        );
        assert!(failures[0].contains("content drift"));
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
}
