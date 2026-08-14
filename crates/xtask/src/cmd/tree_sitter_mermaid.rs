//! Composed language contract for the independently versioned Tree-sitter Mermaid package.

use crate::XtaskError;
use merman_core::{diagram_family_capabilities, diagram_header_facts, supported_diagrams};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const PACKAGE_ROOT: &str = "distribution/tree-sitter-mermaid";
const SUPPORT_PATH: &str = "distribution/tree-sitter-mermaid/metadata/support.json";
const PROVENANCE_PATH: &str = "distribution/tree-sitter-mermaid/metadata/provenance.json";
const SCHEMA_PATH: &str = "distribution/tree-sitter-mermaid/metadata/schema-version.json";
const CONTRACT_PATH: &str = "contracts/tree-sitter/mermaid-language-v1.json";
const UPSTREAM_LOCK_PATH: &str = "tools/upstreams/REPOS.lock.json";
const PUBLIC_FAMILY_COUNT: usize = 35;
const PACKAGE_VERSION: &str = "0.1.0";
const TREE_SITTER_CLI_VERSION: &str = "0.26.12";
const TREE_SITTER_RUST_VERSION: &str = "0.26.12";
const TREE_SITTER_NODE_VERSION: &str = "0.25.1";
const TREE_SITTER_WEB_VERSION: &str = "0.26.12";
const LANGUAGE_ABI: u32 = 14;
const QUERY_PROFILES: [&str; 4] = ["portable", "neovim", "helix", "zed"];
const QUERY_SURFACES: [&str; 9] = [
    "highlights",
    "folds",
    "indents",
    "injections",
    "locals",
    "tags",
    "brackets",
    "outline",
    "textobjects",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SupportMetadata {
    schema_version: u32,
    selected_baselines: SelectedBaselines,
    repository_alignment: RepositoryAlignment,
    families: Vec<FamilySupport>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectedBaselines {
    mermaid: Baseline,
    zenuml: Baseline,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Baseline {
    version: String,
    r#ref: String,
    commit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RepositoryAlignment {
    mermaid: String,
    zenuml: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilySupport {
    public_id: String,
    root_node: String,
    lifecycle: String,
    support_tier: Option<String>,
    evidence: Vec<FamilyEvidence>,
    query_applicability: BTreeMap<String, BTreeMap<String, QueryApplicability>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilyEvidence {
    id: String,
    kind: String,
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QueryApplicability {
    status: String,
    #[serde(default)]
    evidence: Vec<String>,
    #[serde(default)]
    rationale: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProvenanceMetadata {
    schema_version: u32,
    package: PackageIdentity,
    language: LanguageIdentity,
    toolchain: ToolchainIdentity,
    sources: Vec<SourceIdentity>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageIdentity {
    name: String,
    version: String,
    release_state: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LanguageIdentity {
    symbol: String,
    abi: u32,
    cst_schema_version: u32,
    query_schema_version: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToolchainIdentity {
    tree_sitter_cli: String,
    rust_runtime: String,
    node_runtime: String,
    web_runtime: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceIdentity {
    id: String,
    kind: String,
    version: String,
    repository: String,
    r#ref: String,
    commit: String,
    usage: String,
    license: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SchemaMetadata {
    schema_version: u32,
    cst: InterfaceSchema,
    queries: InterfaceSchema,
    compatible_pairs: Vec<CompatiblePair>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InterfaceSchema {
    id: String,
    version: u32,
    stability: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompatiblePair {
    cst: u32,
    queries: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryLock {
    repos: BTreeMap<String, RepositoryLockEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryLockEntry {
    r#ref: String,
    commit: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreFamilyProjection {
    public_id: String,
    logical_family_kind: String,
    internal_variants: Vec<String>,
    authoring_headers: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LanguageContract {
    schema_version: u32,
    generated_by: &'static str,
    provenance: ProvenanceMetadata,
    schemas: SchemaMetadata,
    authorities: AuthorityReceipt,
    selected_baselines: SelectedBaselines,
    repository_alignment: RepositoryAlignment,
    families: Vec<ContractFamily>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityReceipt {
    merman_family_catalog_sha256: String,
    grammar_support_sha256: String,
    public_family_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContractFamily {
    public_id: String,
    logical_family_kind: String,
    internal_variants: Vec<String>,
    authoring_headers: Vec<String>,
    root_node: String,
    lifecycle: String,
    support_tier: Option<String>,
    evidence: Vec<FamilyEvidence>,
    query_applicability: BTreeMap<String, BTreeMap<String, QueryApplicability>>,
}

fn contract_error(message: impl Into<String>) -> XtaskError {
    XtaskError::VerifyFailed(format!(
        "Tree-sitter Mermaid contract is invalid: {}",
        message.into()
    ))
}

fn read_json<T: for<'de> Deserialize<'de>>(root: &Path, relative: &str) -> Result<T, String> {
    let path = root.join(relative);
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut rendered = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(rendered)
}

fn core_family_projection() -> Result<Vec<CoreFamilyProjection>, String> {
    let public_ids = supported_diagrams();
    if public_ids.len() != PUBLIC_FAMILY_COUNT {
        return Err(format!(
            "Merman public family catalog has {} rows; expected {PUBLIC_FAMILY_COUNT}",
            public_ids.len()
        ));
    }
    let public_set = public_ids.iter().copied().collect::<BTreeSet<_>>();
    if public_set.len() != public_ids.len() {
        return Err("Merman public family catalog contains duplicate IDs".to_string());
    }

    for capability in diagram_family_capabilities() {
        let Some(public_id) = capability.metadata_id else {
            continue;
        };
        if !public_set.contains(public_id) {
            return Err(format!(
                "catalog variant {} names unknown public family {public_id}",
                capability.diagram_type
            ));
        }
    }

    let mut families = BTreeMap::<&str, CoreFamilyProjection>::new();
    let mut variant_owner = BTreeMap::<&str, &str>::new();
    for public_id in public_ids {
        families.insert(
            public_id,
            CoreFamilyProjection {
                public_id: (*public_id).to_string(),
                logical_family_kind: String::new(),
                internal_variants: Vec::new(),
                authoring_headers: Vec::new(),
            },
        );
    }
    for capability in diagram_family_capabilities() {
        let Some(public_id) = capability.metadata_id else {
            continue;
        };
        let family = families
            .get_mut(public_id)
            .expect("logical owner was derived from a public catalog row");
        if family.logical_family_kind.is_empty() {
            family.logical_family_kind = capability.logical_family_kind.to_string();
        } else if family.logical_family_kind != capability.logical_family_kind {
            return Err(format!(
                "public family {public_id} spans logical families {} and {}",
                family.logical_family_kind, capability.logical_family_kind
            ));
        }
        family
            .internal_variants
            .push(capability.diagram_type.to_string());
        variant_owner.insert(capability.diagram_type, public_id);
    }
    for header in diagram_header_facts() {
        let public_id = variant_owner.get(header.diagram_type).ok_or_else(|| {
            format!(
                "header {} belongs to an internal variant without a public family",
                header.label
            )
        })?;
        families
            .get_mut(public_id)
            .expect("header owner is a public family")
            .authoring_headers
            .push(header.label.to_string());
    }

    public_ids
        .iter()
        .map(|public_id| {
            let family = families
                .remove(public_id)
                .expect("every public family was initialized");
            if family.logical_family_kind.is_empty() || family.internal_variants.is_empty() {
                Err(format!(
                    "public family {public_id} has no catalog-owned variant"
                ))
            } else if family.authoring_headers.is_empty() {
                Err(format!(
                    "public family {public_id} has no catalog-owned authoring header"
                ))
            } else {
                Ok(family)
            }
        })
        .collect()
}

fn validate_support(
    support: &SupportMetadata,
    core: &[CoreFamilyProjection],
) -> Result<(), String> {
    if support.schema_version != 1 {
        return Err(format!(
            "support schema version {} is unsupported",
            support.schema_version
        ));
    }
    for (name, alignment) in [
        ("Mermaid", support.repository_alignment.mermaid.as_str()),
        ("ZenUML", support.repository_alignment.zenuml.as_str()),
    ] {
        if !matches!(alignment, "aligned" | "drifted") {
            return Err(format!(
                "{name} repository alignment {alignment:?} is unknown"
            ));
        }
    }

    let expected = core
        .iter()
        .map(|family| family.public_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    let mut roots = BTreeSet::new();
    for family in &support.families {
        if !actual.insert(family.public_id.as_str()) {
            return Err(format!("duplicate public family {}", family.public_id));
        }
        if !roots.insert(family.root_node.as_str()) {
            return Err(format!("duplicate family root {}", family.root_node));
        }
        if !valid_root_node(&family.root_node) {
            return Err(format!(
                "family {} has invalid root node {:?}",
                family.public_id, family.root_node
            ));
        }
        validate_family_support(family)?;
    }
    let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
    let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
    if !missing.is_empty() || !unexpected.is_empty() {
        return Err(format!(
            "support families differ from Merman catalog; missing={missing:?}, unexpected={unexpected:?}"
        ));
    }
    if support.families.len() != PUBLIC_FAMILY_COUNT {
        return Err(format!(
            "support metadata has {} rows; expected {PUBLIC_FAMILY_COUNT}",
            support.families.len()
        ));
    }
    Ok(())
}

fn valid_root_node(value: &str) -> bool {
    !value.is_empty()
        && value.ends_with("_diagram")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !value.starts_with('_')
        && !value.contains("__")
}

fn validate_family_support(family: &FamilySupport) -> Result<(), String> {
    let tier_rank = match family.support_tier.as_deref() {
        None => 0,
        Some("recognized") => 1,
        Some("structured") => 2,
        Some("query-complete") => 3,
        Some("conformant") => 4,
        Some(tier) => {
            return Err(format!(
                "family {} has unknown support tier {tier:?}",
                family.public_id
            ));
        }
    };
    match (family.lifecycle.as_str(), tier_rank) {
        ("planned", 0) | ("active", 1..=4) => {}
        (lifecycle, _) if lifecycle != "planned" && lifecycle != "active" => {
            return Err(format!(
                "family {} has unknown lifecycle {lifecycle:?}",
                family.public_id
            ));
        }
        _ => {
            return Err(format!(
                "family {} lifecycle and support tier disagree",
                family.public_id
            ));
        }
    }
    if tier_rank > 0 && family.evidence.is_empty() {
        return Err(format!(
            "family {} claims support without evidence",
            family.public_id
        ));
    }
    validate_evidence(family)?;
    validate_query_applicability(family, tier_rank >= 3)
}

fn validate_evidence(family: &FamilySupport) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for evidence in &family.evidence {
        if evidence.id.trim().is_empty()
            || evidence.kind.trim().is_empty()
            || evidence.path.trim().is_empty()
        {
            return Err(format!(
                "family {} has incomplete evidence",
                family.public_id
            ));
        }
        if !ids.insert(evidence.id.as_str()) {
            return Err(format!(
                "family {} repeats evidence ID {}",
                family.public_id, evidence.id
            ));
        }
        if evidence.sha256.len() != 64
            || !evidence.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(format!(
                "family {} evidence {} has an invalid SHA-256",
                family.public_id, evidence.id
            ));
        }
    }
    Ok(())
}

fn validate_query_applicability(family: &FamilySupport, complete: bool) -> Result<(), String> {
    let known_profiles = QUERY_PROFILES.into_iter().collect::<BTreeSet<_>>();
    let known_surfaces = QUERY_SURFACES.into_iter().collect::<BTreeSet<_>>();
    for (profile, surfaces) in &family.query_applicability {
        if !known_profiles.contains(profile.as_str()) {
            return Err(format!(
                "family {} names unknown query profile {profile}",
                family.public_id
            ));
        }
        for (surface, applicability) in surfaces {
            if !known_surfaces.contains(surface.as_str()) {
                return Err(format!(
                    "family {} names unknown query surface {surface}",
                    family.public_id
                ));
            }
            match applicability.status.as_str() {
                "asserted" if applicability.evidence.is_empty() => {
                    return Err(format!(
                        "family {} claims {profile}/{surface} without applicability evidence",
                        family.public_id
                    ));
                }
                "asserted" if applicability.rationale.is_some() => {
                    return Err(format!(
                        "family {} asserted {profile}/{surface} must not use an N/A rationale",
                        family.public_id
                    ));
                }
                "asserted" => {}
                "not_applicable"
                    if applicability
                        .rationale
                        .as_deref()
                        .is_none_or(|rationale| rationale.trim().is_empty()) =>
                {
                    return Err(format!(
                        "family {} marks {profile}/{surface} N/A without a rationale",
                        family.public_id
                    ));
                }
                "not_applicable" if !applicability.evidence.is_empty() => {
                    return Err(format!(
                        "family {} N/A {profile}/{surface} must not claim capture evidence",
                        family.public_id
                    ));
                }
                "not_applicable" => {}
                status => {
                    return Err(format!(
                        "family {} has unknown {profile}/{surface} status {status:?}",
                        family.public_id
                    ));
                }
            }
        }
    }
    if complete {
        for profile in QUERY_PROFILES {
            let surfaces = family.query_applicability.get(profile).ok_or_else(|| {
                format!(
                    "family {} is query-complete but lacks profile {profile}",
                    family.public_id
                )
            })?;
            for surface in QUERY_SURFACES {
                if !surfaces.contains_key(surface) {
                    return Err(format!(
                        "family {} is query-complete but lacks {profile}/{surface}",
                        family.public_id
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_schemas(
    schemas: &SchemaMetadata,
    provenance: &ProvenanceMetadata,
) -> Result<(), String> {
    if schemas.schema_version != 1
        || schemas.cst.id != "mermaid-cst"
        || schemas.queries.id != "mermaid-queries"
        || schemas.cst.stability != "experimental"
        || schemas.queries.stability != "experimental"
    {
        return Err("schema identity is not the admitted experimental v1 shape".to_string());
    }
    if schemas.cst.version != provenance.language.cst_schema_version
        || schemas.queries.version != provenance.language.query_schema_version
    {
        return Err("provenance and schema versions disagree".to_string());
    }
    let compatible = schemas
        .compatible_pairs
        .iter()
        .any(|pair| pair.cst == schemas.cst.version && pair.queries == schemas.queries.version);
    if !compatible {
        return Err(format!(
            "CST/query schema pair {}/{} is not declared compatible",
            schemas.cst.version, schemas.queries.version
        ));
    }
    Ok(())
}

fn validate_provenance(provenance: &ProvenanceMetadata) -> Result<(), String> {
    if provenance.schema_version != 1
        || provenance.package.name != "tree-sitter-mermaid"
        || provenance.package.version != PACKAGE_VERSION
        || provenance.package.release_state != "dry-run-only"
        || provenance.language.symbol != "mermaid"
        || provenance.language.abi != LANGUAGE_ABI
    {
        return Err(
            "package or language identity does not match the admitted bootstrap".to_string(),
        );
    }
    if provenance.toolchain.tree_sitter_cli != TREE_SITTER_CLI_VERSION
        || provenance.toolchain.rust_runtime != TREE_SITTER_RUST_VERSION
        || provenance.toolchain.node_runtime != TREE_SITTER_NODE_VERSION
        || provenance.toolchain.web_runtime != TREE_SITTER_WEB_VERSION
    {
        return Err("Tree-sitter runtime/toolchain identities drifted".to_string());
    }
    let required = [
        "merman-oracle",
        "mermaid",
        "zenuml-core",
        "pappasam-tree-sitter-mermaid",
        "monaqa-tree-sitter-mermaid",
        "singularity-tree-sitter-mermaid",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let actual = provenance
        .sources
        .iter()
        .map(|source| source.id.as_str())
        .collect::<BTreeSet<_>>();
    if actual.len() != provenance.sources.len() || actual != required {
        return Err(format!(
            "provenance sources differ from the admitted set: {actual:?}"
        ));
    }
    for source in &provenance.sources {
        if source.commit.len() != 40
            || !source.commit.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !source.repository.starts_with("https://")
            || source.r#ref.trim().is_empty()
            || source.kind.trim().is_empty()
            || source.version.trim().is_empty()
            || source.usage.trim().is_empty()
            || source.license.trim().is_empty()
        {
            return Err(format!("provenance source {} is incomplete", source.id));
        }
    }
    Ok(())
}

fn validate_baselines(
    support: &SupportMetadata,
    provenance: &ProvenanceMetadata,
    lock: &RepositoryLock,
) -> Result<(), String> {
    for (label, selected, lock_id, source_id) in [
        (
            "Mermaid",
            &support.selected_baselines.mermaid,
            "mermaid",
            "mermaid",
        ),
        (
            "ZenUML",
            &support.selected_baselines.zenuml,
            "zenuml-core",
            "zenuml-core",
        ),
    ] {
        let locked = lock
            .repos
            .get(lock_id)
            .ok_or_else(|| format!("repository lock lacks {lock_id}"))?;
        let source = provenance
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .ok_or_else(|| format!("provenance lacks {source_id}"))?;
        if selected.r#ref != locked.r#ref
            || selected.commit != locked.commit
            || selected.r#ref != source.r#ref
            || selected.commit != source.commit
            || selected.version != source.version
        {
            return Err(format!(
                "{label} identity drifted across support/provenance/lock"
            ));
        }
    }
    Ok(())
}

fn toml_string<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for component in path {
        current = current.get(*component)?;
    }
    current.as_str()
}

fn validate_package_manifests(root: &Path) -> Result<(), String> {
    let package_manifest_path = root.join(PACKAGE_ROOT).join("Cargo.toml");
    let package_manifest_source = fs::read_to_string(&package_manifest_path).map_err(|error| {
        format!(
            "failed to read {}: {error}",
            package_manifest_path.display()
        )
    })?;
    let package_manifest =
        toml::from_str::<toml::Value>(&package_manifest_source).map_err(|error| {
            format!(
                "failed to parse {}: {error}",
                package_manifest_path.display()
            )
        })?;
    if toml_string(&package_manifest, &["package", "name"]) != Some("tree-sitter-mermaid")
        || toml_string(&package_manifest, &["package", "version"]) != Some(PACKAGE_VERSION)
        || package_manifest
            .get("package")
            .and_then(|package| package.get("publish"))
            .and_then(toml::Value::as_bool)
            != Some(false)
        || toml_string(
            &package_manifest,
            &["dependencies", "tree-sitter", "version"],
        ) != Some("=0.26.12")
    {
        return Err("Cargo package identity, runtime pin, or dry-run boundary drifted".to_string());
    }

    let root_manifest_path = root.join("Cargo.toml");
    let root_manifest_source = fs::read_to_string(&root_manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", root_manifest_path.display()))?;
    let root_manifest = toml::from_str::<toml::Value>(&root_manifest_source)
        .map_err(|error| format!("failed to parse {}: {error}", root_manifest_path.display()))?;
    let members = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "root workspace members are missing".to_string())?;
    if !members
        .iter()
        .any(|member| member.as_str() == Some(PACKAGE_ROOT))
    {
        return Err("Tree-sitter package is not an explicit workspace member".to_string());
    }
    let independent = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("metadata"))
        .and_then(|metadata| metadata.get("merman-release"))
        .and_then(|release| release.get("independent-packages"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "independent package metadata is missing".to_string())?;
    if !independent
        .iter()
        .any(|package| package.as_str() == Some("tree-sitter-mermaid"))
    {
        return Err("Tree-sitter package is not independently versioned".to_string());
    }

    let package_json: serde_json::Value = read_json(root, &format!("{PACKAGE_ROOT}/package.json"))?;
    let dev_dependencies = package_json
        .get("devDependencies")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "npm devDependencies are missing".to_string())?;
    for (name, version) in [
        ("tree-sitter", TREE_SITTER_NODE_VERSION),
        ("tree-sitter-cli", TREE_SITTER_CLI_VERSION),
        ("web-tree-sitter", TREE_SITTER_WEB_VERSION),
    ] {
        if dev_dependencies
            .get(name)
            .and_then(serde_json::Value::as_str)
            != Some(version)
        {
            return Err(format!("npm package does not pin {name} {version}"));
        }
    }
    if package_json
        .get("version")
        .and_then(serde_json::Value::as_str)
        != Some(PACKAGE_VERSION)
        || package_json
            .get("private")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
    {
        return Err("npm package identity or dry-run boundary drifted".to_string());
    }

    let package_lock: serde_json::Value =
        read_json(root, &format!("{PACKAGE_ROOT}/package-lock.json"))?;
    if package_lock
        .pointer("/packages//version")
        .and_then(serde_json::Value::as_str)
        != Some(PACKAGE_VERSION)
    {
        return Err("npm package lock root version drifted".to_string());
    }
    for (name, version) in [
        ("tree-sitter", TREE_SITTER_NODE_VERSION),
        ("tree-sitter-cli", TREE_SITTER_CLI_VERSION),
        ("web-tree-sitter", TREE_SITTER_WEB_VERSION),
    ] {
        let pointer = format!("/packages/node_modules~1{name}/version");
        if package_lock
            .pointer(&pointer)
            .and_then(serde_json::Value::as_str)
            != Some(version)
        {
            return Err(format!("npm package lock does not pin {name} {version}"));
        }
    }

    let tree_sitter_json: serde_json::Value =
        read_json(root, &format!("{PACKAGE_ROOT}/tree-sitter.json"))?;
    if tree_sitter_json
        .pointer("/metadata/version")
        .and_then(serde_json::Value::as_str)
        != Some(PACKAGE_VERSION)
    {
        return Err("tree-sitter.json package version drifted".to_string());
    }
    let license = fs::read_to_string(root.join(PACKAGE_ROOT).join("LICENSE"))
        .map_err(|error| format!("failed to read package LICENSE: {error}"))?;
    if !license.contains("MIT License")
        || !license.contains("Copyright (c) 2026 Samuel Roeca")
        || !license.contains("Copyright (c) 2026 Merman contributors")
    {
        return Err("package MIT license does not preserve package attribution".to_string());
    }
    Ok(())
}

fn build_contract(root: &Path) -> Result<LanguageContract, String> {
    let support: SupportMetadata = read_json(root, SUPPORT_PATH)?;
    let provenance: ProvenanceMetadata = read_json(root, PROVENANCE_PATH)?;
    let schemas: SchemaMetadata = read_json(root, SCHEMA_PATH)?;
    let upstream_lock: RepositoryLock = read_json(root, UPSTREAM_LOCK_PATH)?;
    let core = core_family_projection()?;
    validate_support(&support, &core)?;
    validate_provenance(&provenance)?;
    validate_schemas(&schemas, &provenance)?;
    validate_baselines(&support, &provenance, &upstream_lock)?;
    validate_package_manifests(root)?;

    let grammar_support_sha256 = sha256_json(&support)?;
    let merman_family_catalog_sha256 = sha256_json(&core)?;
    let mut support_by_id = support
        .families
        .into_iter()
        .map(|family| (family.public_id.clone(), family))
        .collect::<BTreeMap<_, _>>();
    let families = core
        .into_iter()
        .map(|family| {
            let support = support_by_id
                .remove(&family.public_id)
                .expect("support was validated against the core catalog");
            ContractFamily {
                public_id: family.public_id,
                logical_family_kind: family.logical_family_kind,
                internal_variants: family.internal_variants,
                authoring_headers: family.authoring_headers,
                root_node: support.root_node,
                lifecycle: support.lifecycle,
                support_tier: support.support_tier,
                evidence: support.evidence,
                query_applicability: support.query_applicability,
            }
        })
        .collect();

    Ok(LanguageContract {
        schema_version: 1,
        generated_by: "cargo run --locked -p xtask -- verify-tree-sitter-mermaid --write",
        provenance,
        schemas,
        authorities: AuthorityReceipt {
            merman_family_catalog_sha256,
            grammar_support_sha256,
            public_family_count: PUBLIC_FAMILY_COUNT,
        },
        selected_baselines: support.selected_baselines,
        repository_alignment: support.repository_alignment,
        families,
    })
}

fn render_contract(root: &Path) -> Result<String, String> {
    let mut rendered =
        serde_json::to_string_pretty(&build_contract(root)?).map_err(|error| error.to_string())?;
    rendered.push('\n');
    Ok(rendered)
}

pub(crate) fn verify_tree_sitter_mermaid(args: Vec<String>) -> Result<(), XtaskError> {
    let write = match args.as_slice() {
        [] => false,
        [arg] if arg == "--write" => true,
        _ => return Err(XtaskError::Usage),
    };
    let root = crate::cmd::workspace_root();
    let expected = render_contract(&root).map_err(contract_error)?;
    let path = root.join(CONTRACT_PATH);
    if write {
        let parent = path.parent().ok_or_else(|| {
            contract_error(format!("contract path has no parent: {}", path.display()))
        })?;
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
        fs::write(&path, expected).map_err(|source| XtaskError::WriteFile {
            path: path.display().to_string(),
            source,
        })?;
        return Ok(());
    }
    let actual = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    if actual.replace("\r\n", "\n") != expected {
        return Err(contract_error(format!(
            "{CONTRACT_PATH} drifted; regenerate with `cargo run --locked -p xtask -- verify-tree-sitter-mermaid --write`"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_inputs() -> (SupportMetadata, Vec<CoreFamilyProjection>) {
        let root = crate::cmd::workspace_root();
        (
            read_json(&root, SUPPORT_PATH).expect("support metadata"),
            core_family_projection().expect("core projection"),
        )
    }

    #[test]
    fn support_metadata_matches_exact_public_catalog_without_claiming_support() {
        let (support, core) = repository_inputs();

        validate_support(&support, &core).expect("valid support metadata");
        assert_eq!(support.families.len(), PUBLIC_FAMILY_COUNT);
        assert!(support.families.iter().all(|family| {
            family.lifecycle == "planned"
                && family.support_tier.is_none()
                && family.evidence.is_empty()
                && family.query_applicability.is_empty()
        }));
    }

    #[test]
    fn duplicate_missing_and_internal_variant_rows_are_rejected() {
        let (support, core) = repository_inputs();

        let mut duplicate = support.clone();
        duplicate.families[1].public_id = duplicate.families[0].public_id.clone();
        assert!(
            validate_support(&duplicate, &core)
                .unwrap_err()
                .contains("duplicate")
        );

        let mut missing = support.clone();
        missing.families.pop();
        assert!(
            validate_support(&missing, &core)
                .unwrap_err()
                .contains("missing=")
        );

        let mut internal = support;
        internal.families[0].public_id = "flowchart-v2".to_string();
        let error = validate_support(&internal, &core).unwrap_err();
        assert!(error.contains("unexpected="));
        assert!(error.contains("flowchart-v2"));
    }

    #[test]
    fn unknown_tier_and_unproved_query_claim_are_rejected() {
        let (support, core) = repository_inputs();

        let mut tier = support.clone();
        tier.families[0].lifecycle = "active".to_string();
        tier.families[0].support_tier = Some("complete-ish".to_string());
        assert!(
            validate_support(&tier, &core)
                .unwrap_err()
                .contains("unknown support tier")
        );

        let mut query = support;
        query.families[0].query_applicability.insert(
            "portable".to_string(),
            BTreeMap::from([(
                "highlights".to_string(),
                QueryApplicability {
                    status: "asserted".to_string(),
                    evidence: Vec::new(),
                    rationale: None,
                },
            )]),
        );
        assert!(
            validate_support(&query, &core)
                .unwrap_err()
                .contains("without applicability evidence")
        );
    }

    #[test]
    fn baseline_drift_is_rejected_without_rewriting_support_tiers() {
        let root = crate::cmd::workspace_root();
        let (mut support, _core) = repository_inputs();
        let provenance: ProvenanceMetadata = read_json(&root, PROVENANCE_PATH).expect("provenance");
        let lock: RepositoryLock = read_json(&root, UPSTREAM_LOCK_PATH).expect("upstream lock");
        support.selected_baselines.mermaid.commit = "0".repeat(40);

        assert!(
            validate_baselines(&support, &provenance, &lock)
                .unwrap_err()
                .contains("Mermaid identity drifted")
        );
    }

    #[test]
    fn schema_pair_must_be_explicitly_compatible() {
        let root = crate::cmd::workspace_root();
        let provenance: ProvenanceMetadata = read_json(&root, PROVENANCE_PATH).expect("provenance");
        let mut schemas: SchemaMetadata = read_json(&root, SCHEMA_PATH).expect("schemas");
        schemas.compatible_pairs.clear();

        assert!(
            validate_schemas(&schemas, &provenance)
                .unwrap_err()
                .contains("not declared compatible")
        );
    }

    #[test]
    fn rendered_contract_contains_35_unique_roots_and_two_authority_digests() {
        let contract = build_contract(&crate::cmd::workspace_root()).expect("language contract");
        let roots = contract
            .families
            .iter()
            .map(|family| family.root_node.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(contract.families.len(), PUBLIC_FAMILY_COUNT);
        assert_eq!(roots.len(), PUBLIC_FAMILY_COUNT);
        assert_eq!(contract.authorities.merman_family_catalog_sha256.len(), 64);
        assert_eq!(contract.authorities.grammar_support_sha256.len(), 64);
    }
}
