//! Composed language contract for the independently versioned Tree-sitter Mermaid package.

mod admission;

use crate::XtaskError;
use merman_core::{diagram_family_capabilities, diagram_header_facts, supported_diagrams};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use tree_sitter_mermaid::{
    ARTIFACT_RECEIPT, LANGUAGE_ABI, LANGUAGE_SYMBOL, NODE_SCHEMA_VERSION, PACKAGE_VERSION,
    QUERY_SCHEMA_VERSION, TREE_SITTER_RUST_RUNTIME_VERSION,
};

const PACKAGE_ROOT: &str = "distribution/tree-sitter-mermaid";
const SUPPORT_PATH: &str = "distribution/tree-sitter-mermaid/metadata/support.json";
const PROVENANCE_PATH: &str = "distribution/tree-sitter-mermaid/metadata/provenance.json";
const DERIVATIONS_PATH: &str = "distribution/tree-sitter-mermaid/metadata/derivations.json";
const ARTIFACT_RECEIPT_PATH: &str =
    "distribution/tree-sitter-mermaid/metadata/artifact-receipt.json";
const METRICS_PATH: &str = "distribution/tree-sitter-mermaid/metadata/metrics/u2-mechanics.json";
const SCHEMA_PATH: &str = "distribution/tree-sitter-mermaid/metadata/schema-version.json";
const HEADER_MANIFEST_PATH: &str = "distribution/tree-sitter-mermaid/metadata/headers.json";
const HEADER_RECEIPT_PATH: &str =
    "distribution/tree-sitter-mermaid/metadata/evidence/u2-header-dispatch.json";
const HEADER_RECEIPT_PACKAGE_PATH: &str = "metadata/evidence/u2-header-dispatch.json";
const STRICT_HEADER_ORACLE_PATH: &str =
    "distribution/tree-sitter-mermaid/metadata/evidence/u2-mermaid-header-oracle.json";
const STRICT_HEADER_ORACLE_PACKAGE_PATH: &str = "metadata/evidence/u2-mermaid-header-oracle.json";
const HEADER_ORACLE_SCRIPT_PATH: &str = "distribution/tree-sitter-mermaid/scripts/header_oracle.js";
const HEADER_ORACLE_RUNNER_LOCK_PATH: &str =
    "distribution/tree-sitter-mermaid/scripts/header-oracle/package-lock.json";
const CONCATENATED_HEADER_NEGATIVES: [&str; 8] = [
    "flowchartTD\n",
    "infoshowInfo\n",
    "pieshowData\n",
    "pietitle Foo\n",
    "gitGraphLR:\n",
    "swimlane-betaTD\n",
    "timelineLR\n",
    "xycharthorizontal\n",
];
const FAMILY_FIXTURES_PATH: &str =
    "distribution/tree-sitter-mermaid/metadata/fixtures/family-roots.json";
const CONTRACT_PATH: &str = "contracts/tree-sitter/mermaid-language-v1.json";
const UPSTREAM_LOCK_PATH: &str = "tools/upstreams/REPOS.lock.json";
const THIRD_PARTY_COMPONENTS_PATH: &str = "docs/release/THIRD_PARTY_COMPONENTS.json";
const PUBLIC_FAMILY_COUNT: usize = 35;
const TREE_SITTER_CLI_VERSION: &str = "0.26.12";
const TREE_SITTER_NODE_VERSION: &str = "0.25.1";
const TREE_SITTER_WEB_VERSION: &str = "0.26.12";
const TREE_SITTER_WASI_SDK_VERSION: &str = "29.0";
const TREE_SITTER_WASI_CLANG_VERSION: &str = "21.1.4-wasi-sdk";
const MERMAN_ORACLE_VERSION: &str = "0.8.0-alpha.5";
const MERMAN_ORACLE_COMMIT: &str = "e4d3169a614f4eca3e4897fe9ee1fd578136db92";
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
const EVIDENCE_KINDS: [&str; 10] = [
    "binding",
    "conformance",
    "corpus",
    "fuzz",
    "header",
    "incremental",
    "metrics",
    "node-schema",
    "query",
    "recovery",
];
const PACKAGE_LICENSE_COPIES: [(&str, &str, &str); 6] = [
    (
        "tree-sitter",
        "THIRD_PARTY_LICENSES/tree-sitter/LICENSE",
        "THIRD_PARTY_LICENSES/tree-sitter/LICENSE",
    ),
    (
        "mermaid",
        "THIRD_PARTY_LICENSES/mermaid/LICENSE",
        "THIRD_PARTY_LICENSES/mermaid/LICENSE",
    ),
    (
        "zenuml-core",
        "THIRD_PARTY_LICENSES/zenuml-core/LICENSE",
        "THIRD_PARTY_LICENSES/zenuml-core/LICENSE",
    ),
    (
        "pappasam-tree-sitter-mermaid",
        "THIRD_PARTY_LICENSES/tree-sitter-mermaid-pappasam/LICENSE",
        "THIRD_PARTY_LICENSES/pappasam-tree-sitter-mermaid/LICENSE",
    ),
    (
        "monaqa-tree-sitter-mermaid",
        "THIRD_PARTY_LICENSES/tree-sitter-mermaid-monaqa/LICENSE",
        "THIRD_PARTY_LICENSES/monaqa-tree-sitter-mermaid/LICENSE",
    ),
    (
        "singularity-tree-sitter-mermaid",
        "THIRD_PARTY_LICENSES/tree-sitter-mermaid-singularity/LICENSE",
        "THIRD_PARTY_LICENSES/singularity-tree-sitter-mermaid/LICENSE",
    ),
];
const GENERATED_ARTIFACTS: [&str; 7] = [
    "src/parser.c",
    "src/grammar.json",
    "src/node-types.json",
    "src/tree_sitter/alloc.h",
    "src/tree_sitter/array.h",
    "src/tree_sitter/parser.h",
    "wasm/tree-sitter-mermaid.wasm",
];
const ATTRIBUTED_PACKAGE_FILES: [&str; 58] = [
    "binding.gyp",
    "bindings/c/tree-sitter-mermaid.pc.in",
    "bindings/c/tree_sitter/tree-sitter-mermaid.h",
    "bindings/c/tree_sitter/tree-sitter-mermaid.h.in",
    "bindings/node/binding.cc",
    "bindings/node/binding_test.js",
    "bindings/node/index.d.ts",
    "bindings/node/index.js",
    "bindings/rust/build.rs",
    "bindings/rust/lib.rs",
    "grammar.js",
    "grammar/families/architecture.js",
    "grammar/families/block.js",
    "grammar/families/c4.js",
    "grammar/families/class.js",
    "grammar/families/cynefin.js",
    "grammar/families/entity-relationship.js",
    "grammar/families/event-modeling.js",
    "grammar/families/flowchart.js",
    "grammar/families/gantt.js",
    "grammar/families/git-graph.js",
    "grammar/families/info.js",
    "grammar/families/ishikawa.js",
    "grammar/families/journey.js",
    "grammar/families/kanban.js",
    "grammar/families/mindmap.js",
    "grammar/families/packet.js",
    "grammar/families/pie.js",
    "grammar/families/quadrant-chart.js",
    "grammar/families/radar.js",
    "grammar/families/railroad-abnf.js",
    "grammar/families/railroad-ebnf.js",
    "grammar/families/railroad-peg.js",
    "grammar/families/railroad-shared.js",
    "grammar/families/railroad.js",
    "grammar/families/requirement.js",
    "grammar/families/sankey.js",
    "grammar/families/sequence.js",
    "grammar/families/state.js",
    "grammar/families/swimlane.js",
    "grammar/families/timeline.js",
    "grammar/families/tree-view.js",
    "grammar/families/treemap.js",
    "grammar/families/venn.js",
    "grammar/families/wardley.js",
    "grammar/families/xy-chart.js",
    "grammar/families/zenuml.js",
    "grammar/shared/common.js",
    "grammar/shared/header.js",
    "grammar/shared/indentation.js",
    "grammar/shared/langium.js",
    "grammar/shared/preamble.js",
    "metadata/headers.json",
    "scripts/generate.py",
    "src/scanner.c",
    "src/tree_sitter/alloc.h",
    "src/tree_sitter/array.h",
    "src/tree_sitter/parser.h",
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
    wasi_sdk: String,
    wasi_clang: String,
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
    #[serde(default)]
    legal_component_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DerivationMetadata {
    schema_version: u32,
    package: String,
    derivations: Vec<Derivation>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Derivation {
    local_paths: Vec<String>,
    local_symbols: Vec<String>,
    sources: Vec<DerivationSource>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DerivationSource {
    source_id: String,
    relationship: String,
    source_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArtifactReceipt {
    receipt_id: String,
    schema_version: u32,
    package: PackageIdentity,
    language: LanguageIdentity,
    toolchain: ReceiptToolchain,
    baselines: BTreeMap<String, ReceiptBaseline>,
    generation: serde_json::Value,
    query_profiles: Vec<ReceiptQueryProfile>,
    inputs: Vec<ReceiptFile>,
    artifacts: Vec<ReceiptFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptQueryProfile {
    profile: String,
    surface: String,
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptToolchain {
    tree_sitter_cli: String,
    rust_runtime: String,
    node_runtime: String,
    web_runtime: String,
    wasi_sdk: String,
    wasi_clang: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptBaseline {
    version: String,
    commit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptFile {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderDispatchReceipt {
    schema_version: u32,
    producer: HeaderReceiptProducer,
    artifact_receipt_id: String,
    strict_oracle_receipt: StrictOracleReceiptReference,
    header_manifest: ReceiptManifest,
    fixture_manifest: ReceiptManifest,
    cases: Vec<HeaderReceiptCase>,
    negative_cases: Vec<HeaderNegativeCase>,
    eof_negative_cases: Vec<HeaderEofNegativeCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrictOracleReceiptReference {
    path: String,
    sha256: String,
    receipt_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderReceiptProducer {
    id: String,
    version: u32,
    command: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptManifest {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderReceiptCase {
    kind: String,
    public_id: String,
    input_sha256: String,
    expected_root: String,
    expected_diagram_type: Option<String>,
    actual_root: String,
    has_error: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderNegativeCase {
    input_sha256: String,
    actual_roots: Vec<String>,
    has_error: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderEofNegativeCase {
    public_id: String,
    input_sha256: String,
    actual_roots: Vec<String>,
    has_error: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderManifest {
    schema_version: u32,
    authorities: serde_json::Value,
    strict_oracle: StrictOracleManifest,
    cases: Vec<HeaderManifestCase>,
    strict_header_negatives: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrictOracleManifest {
    receipt_path: String,
    runner_lock_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderManifestCase {
    public_id: String,
    root: String,
    expected_diagram_type: String,
    source: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrictHeaderOracleReceipt {
    receipt_id: String,
    schema_version: u32,
    producer: StrictOracleProducer,
    authority: serde_json::Value,
    header_manifest: ReceiptManifest,
    runner_lock: ReceiptManifest,
    runtime_packages: Vec<StrictOracleRuntimePackage>,
    cases: Vec<StrictOracleCase>,
    eof_candidate_count: usize,
    eof_cases: Vec<StrictOracleEofCase>,
    negative_cases: Vec<StrictOracleNegativeCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrictOracleProducer {
    id: String,
    version: u32,
    command: String,
    script: ReceiptManifest,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrictOracleRuntimePackage {
    name: String,
    version: String,
    package_json_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrictOracleCase {
    public_id: String,
    input_sha256: String,
    expected_diagram_type: String,
    accepted: bool,
    diagram_type: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrictOracleEofCase {
    public_id: String,
    input_sha256: String,
    expected_diagram_type: String,
    accepted: bool,
    diagram_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrictOracleNegativeCase {
    input_sha256: String,
    accepted: bool,
    diagram_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilyFixture {
    public_id: String,
    root: String,
    source: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MechanicsMetrics {
    schema_version: u32,
    checkpoint: String,
    artifact_receipt_id: String,
    attribution: MetricsAttribution,
    r#static: StaticMetrics,
    ratchet: MetricsRatchet,
    observed: ObservedMetrics,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MetricsAttribution {
    previous_checkpoint: String,
    previous_artifact_receipt_id: String,
    structured_families_added: Vec<String>,
    previous_static: StaticMetrics,
    delta: StaticMetricsDelta,
    explanation: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StaticMetricsDelta {
    generated_c_bytes: i64,
    wasm_bytes: i64,
    parser_states: i64,
    large_states: i64,
    symbols: i64,
    fields: i64,
    external_tokens: i64,
    conflicts: i64,
    wasm_declared_minimum_memory_pages: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservedMetrics {
    environment: MetricsEnvironment,
    build: BuildMetrics,
    native_node_smoke_parse_milliseconds: f64,
    native_node_smoke_maximum_resident_set_bytes: u64,
    wasm_node_smoke_parse_milliseconds: f64,
    wasm_node_smoke_maximum_resident_set_bytes: u64,
    smoke_measurement: String,
    real_corpus: serde_json::Value,
    synthetic_doubling: serde_json::Value,
    fresh_and_incremental_work: serde_json::Value,
    common_short_statement_local_edits: serde_json::Value,
    query_time: QueryTimeMetrics,
    wasm_runtime_memory_pages: WasmRuntimeMemoryMetrics,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MetricsEnvironment {
    os: String,
    architecture: String,
    rust: String,
    node: String,
    tree_sitter_cli: String,
    wasi_sdk: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildMetrics {
    two_runtime_one_wasm_generation_wall_milliseconds: u64,
    canonical_wasm_build_wall_milliseconds: u64,
    rust_release_compile_wall_milliseconds: u64,
    node_binding_compile_wall_milliseconds: u64,
    measurement: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QueryTimeMetrics {
    status: String,
    native_compile_milliseconds: f64,
    native_execution_milliseconds: f64,
    wasm_compile_milliseconds: f64,
    wasm_execution_milliseconds: f64,
    measurement: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WasmRuntimeMemoryMetrics {
    status: String,
    declared_minimum_pages: u64,
    initial_pages: u64,
    observed_peak_pages: u64,
    max_peak_pages: u64,
    stress_source_bytes: u64,
    measurement: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StaticMetrics {
    generated_c_bytes: u64,
    wasm_bytes: u64,
    parser_states: u64,
    large_states: u64,
    symbols: u64,
    fields: u64,
    external_tokens: u64,
    conflicts: u64,
    wasm_declared_minimum_memory_pages: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MetricsRatchet {
    generated_c_hard_limit_bytes: u64,
    wasm_hard_limit_bytes: u64,
    parser_states_investigate_above: u64,
    large_states_investigate_above: u64,
    conflicts_allowed: u64,
    generation_hard_limit_milliseconds: u64,
    canonical_wasm_build_hard_limit_milliseconds: u64,
    independent_compile_hard_limit_milliseconds: u64,
    native_smoke_parse_hard_limit_milliseconds: u64,
    wasm_smoke_parse_hard_limit_milliseconds: u64,
    native_peak_rss_investigate_above_bytes: u64,
    wasm_peak_rss_investigate_above_bytes: u64,
    query_hard_limit_milliseconds: u64,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryLock {
    repos: BTreeMap<String, RepositoryLockEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryLockEntry {
    url: String,
    r#ref: String,
    commit: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ThirdPartyContract {
    components: Vec<ThirdPartyComponent>,
}

#[derive(Clone, Debug, Deserialize)]
struct ThirdPartyComponent {
    id: String,
    version: String,
    source: ThirdPartySource,
    local_paths: Vec<String>,
    license_expression: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ThirdPartySource {
    repository: String,
    r#ref: String,
    commit: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreFamilyProjection {
    public_id: String,
    logical_family_kind: String,
    internal_variants: Vec<String>,
    authoring_header_suggestions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LanguageContract {
    schema_version: u32,
    generated_by: &'static str,
    provenance: ProvenanceMetadata,
    schemas: SchemaMetadata,
    authorities: AuthorityReceipt,
    artifact_receipt_id: String,
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
    authoring_header_suggestions: Vec<String>,
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
                authoring_header_suggestions: Vec::new(),
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
            .authoring_header_suggestions
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
            } else if family.authoring_header_suggestions.is_empty() {
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
    root: &Path,
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
    for family in &support.families {
        if !actual.insert(family.public_id.as_str()) {
            return Err(format!("duplicate public family {}", family.public_id));
        }
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

    let mut roots = BTreeSet::new();
    for family in &support.families {
        if !roots.insert(family.root_node.as_str()) {
            return Err(format!("duplicate family root {}", family.root_node));
        }
        if !valid_root_node(&family.root_node) {
            return Err(format!(
                "family {} has invalid root node {:?}",
                family.public_id, family.root_node
            ));
        }
        validate_family_support(root, family)?;
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

fn validate_family_support(root: &Path, family: &FamilySupport) -> Result<(), String> {
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
    if tier_rank == 0 {
        if !family.evidence.is_empty() || !family.query_applicability.is_empty() {
            return Err(format!(
                "planned family {} must not publish evidence or query claims",
                family.public_id
            ));
        }
        return Ok(());
    }
    if tier_rank > 0 && family.evidence.is_empty() {
        return Err(format!(
            "family {} claims support without evidence",
            family.public_id
        ));
    }
    let evidence_kinds = validate_evidence(root, family)?;
    validate_required_evidence(family, tier_rank, &evidence_kinds)?;
    validate_query_applicability(family, tier_rank >= 3, &evidence_kinds)
}

fn package_file_path(root: &Path, relative: &str, purpose: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative.is_empty()
        || relative_path.is_absolute()
        || !relative_path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{purpose} path {relative:?} is not a normalized package path"
        ));
    }
    let package_root = root
        .join(PACKAGE_ROOT)
        .canonicalize()
        .map_err(|error| format!("failed to resolve package root: {error}"))?;
    let path = package_root.join(relative_path);
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("failed to resolve {purpose} path {relative:?}: {error}"))?;
    if !resolved.starts_with(&package_root) || !resolved.is_file() {
        return Err(format!(
            "{purpose} path {relative:?} must resolve to a file inside the language package"
        ));
    }
    Ok(resolved)
}

fn package_evidence_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    package_file_path(root, relative, "evidence")
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read evidence {}: {error}", path.display()))?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
    }
    rendered
}

fn strict_oracle_case_matches(case: &HeaderManifestCase, result: &StrictOracleCase) -> bool {
    !case.expected_diagram_type.is_empty()
        && result.public_id == case.public_id
        && result.input_sha256 == sha256_bytes(case.source.as_bytes())
        && result.expected_diagram_type == case.expected_diagram_type
        && result.accepted
        && result.diagram_type == case.expected_diagram_type
}

fn strict_oracle_eof_case_matches(case: &HeaderManifestCase, result: &StrictOracleEofCase) -> bool {
    !case.expected_diagram_type.is_empty()
        && result.public_id == case.public_id
        && result.input_sha256 == sha256_bytes(case.source.as_bytes())
        && result.expected_diagram_type == case.expected_diagram_type
        && match (&result.diagram_type, result.accepted) {
            (Some(diagram_type), true) => diagram_type == &case.expected_diagram_type,
            (None, false) => true,
            _ => false,
        }
}

fn validate_strict_header_oracle(
    root: &Path,
    manifest: &HeaderManifest,
    reference: &StrictOracleReceiptReference,
) -> Result<(Vec<HeaderManifestCase>, Vec<HeaderManifestCase>), String> {
    let oracle: StrictHeaderOracleReceipt = read_json(root, STRICT_HEADER_ORACLE_PATH)?;
    if reference.path != STRICT_HEADER_ORACLE_PACKAGE_PATH
        || reference.sha256 != sha256_file(&root.join(STRICT_HEADER_ORACLE_PATH))?
        || reference.receipt_id != oracle.receipt_id
        || !valid_sha256(&oracle.receipt_id)
        || oracle.schema_version != 3
        || oracle.producer.id != "tree-sitter-mermaid/mermaid-strict-header-oracle"
        || oracle.producer.version != 3
        || oracle.producer.command
            != concat!(
                "node scripts/header_oracle.js --node-modules ",
                "scripts/header-oracle/node_modules"
            )
        || oracle.producer.script.path != "scripts/header_oracle.js"
        || oracle.producer.script.sha256 != sha256_file(&root.join(HEADER_ORACLE_SCRIPT_PATH))?
        || oracle.header_manifest.path != "metadata/headers.json"
        || oracle.header_manifest.sha256 != sha256_file(&root.join(HEADER_MANIFEST_PATH))?
        || oracle.runner_lock.path != "scripts/header-oracle/package-lock.json"
        || oracle.runner_lock.sha256 != sha256_file(&root.join(HEADER_ORACLE_RUNNER_LOCK_PATH))?
        || manifest.strict_oracle.receipt_path != STRICT_HEADER_ORACLE_PACKAGE_PATH
        || manifest.strict_oracle.runner_lock_path != "scripts/header-oracle/package-lock.json"
    {
        return Err("strict-header oracle identity or input digest drifted".to_string());
    }

    if oracle
        .authority
        .pointer("/mermaid/version")
        .and_then(serde_json::Value::as_str)
        != Some("11.16.1")
        || oracle
            .authority
            .pointer("/mermaid/commit")
            .and_then(serde_json::Value::as_str)
            != Some("7ecca0cd7f1658ef74f4e7e91f925724ef403bbf")
        || oracle
            .authority
            .pointer("/zenuml/version")
            .and_then(serde_json::Value::as_str)
            != Some("3.50.1")
        || oracle
            .authority
            .pointer("/zenuml/commit")
            .and_then(serde_json::Value::as_str)
            != Some("38404ccc14243ed54ab45b804b2eb6f2ca73af36")
        || oracle
            .authority
            .pointer("/zenuml/relationship")
            .and_then(serde_json::Value::as_str)
            != Some("project-selected companion override")
    {
        return Err("strict-header oracle authority drifted".to_string());
    }

    let expected_packages = [
        ("mermaid", "11.16.1"),
        ("@mermaid-js/parser", "1.2.0"),
        ("@mermaid-js/mermaid-zenuml", "0.2.3"),
        ("jsdom", "26.1.0"),
        ("@zenuml/core", "3.50.1"),
    ]
    .into_iter()
    .map(|(name, version)| (name.to_string(), version.to_string()))
    .collect::<BTreeMap<_, _>>();
    let mut actual_packages = BTreeMap::new();
    for package in &oracle.runtime_packages {
        if !valid_sha256(&package.package_json_sha256)
            || actual_packages
                .insert(package.name.clone(), package.version.clone())
                .is_some()
        {
            return Err("strict-header oracle runtime package identity is invalid".to_string());
        }
    }
    if actual_packages != expected_packages {
        return Err("strict-header oracle runtime package set drifted".to_string());
    }

    if oracle.cases.len() != manifest.cases.len()
        || oracle
            .cases
            .iter()
            .zip(&manifest.cases)
            .any(|(result, case)| !strict_oracle_case_matches(case, result))
    {
        return Err("strict-header oracle positive results drifted".to_string());
    }
    let eof_candidates = eof_header_candidates(manifest)?;
    if oracle.eof_candidate_count != eof_candidates.len()
        || oracle.eof_cases.len() != eof_candidates.len()
        || oracle
            .eof_cases
            .iter()
            .zip(&eof_candidates)
            .any(|(result, case)| !strict_oracle_eof_case_matches(case, result))
    {
        return Err("strict-header oracle EOF results drifted".to_string());
    }
    if oracle.negative_cases.len() != manifest.strict_header_negatives.len()
        || oracle
            .negative_cases
            .iter()
            .zip(&manifest.strict_header_negatives)
            .any(|(result, source)| {
                result.input_sha256 != sha256_bytes(source.as_bytes())
                    || result.accepted
                    || result.diagram_type.is_some()
            })
    {
        return Err("strict-header oracle negative results drifted".to_string());
    }
    let (accepted, rejected) = eof_candidates
        .into_iter()
        .zip(&oracle.eof_cases)
        .partition::<Vec<_>, _>(|(_, result)| result.accepted);
    Ok((
        accepted.into_iter().map(|(case, _)| case).collect(),
        rejected.into_iter().map(|(case, _)| case).collect(),
    ))
}

fn eof_header_candidates(manifest: &HeaderManifest) -> Result<Vec<HeaderManifestCase>, String> {
    let mut ownership = BTreeMap::<(String, String), (String, String)>::new();
    let mut candidates = Vec::new();
    for case in &manifest.cases {
        let source = case
            .source
            .split(['\r', '\n'])
            .next()
            .unwrap_or_default()
            .to_string();
        let key = (case.public_id.clone(), source.clone());
        if let Some((root, expected_diagram_type)) = ownership.get(&key) {
            if root != &case.root || expected_diagram_type != &case.expected_diagram_type {
                return Err(format!(
                    "EOF header candidate {source:?} has conflicting ownership"
                ));
            }
            continue;
        }
        ownership.insert(key, (case.root.clone(), case.expected_diagram_type.clone()));
        candidates.push(HeaderManifestCase {
            public_id: case.public_id.clone(),
            root: case.root.clone(),
            expected_diagram_type: case.expected_diagram_type.clone(),
            source,
        });
    }
    Ok(candidates)
}

fn validate_header_receipt(root: &Path, family: &FamilySupport) -> Result<(), String> {
    let receipt: HeaderDispatchReceipt = read_json(root, HEADER_RECEIPT_PATH)?;
    let artifact: ArtifactReceipt = read_json(root, ARTIFACT_RECEIPT_PATH)?;
    let manifest: HeaderManifest = read_json(root, HEADER_MANIFEST_PATH)?;
    let fixtures: Vec<FamilyFixture> = read_json(root, FAMILY_FIXTURES_PATH)?;

    let (eof_cases, eof_negative_cases) =
        validate_strict_header_oracle(root, &manifest, &receipt.strict_oracle_receipt)?;

    if receipt.schema_version != 5
        || receipt.producer.id != "tree-sitter-mermaid/header-dispatch"
        || receipt.producer.version != 5
        || receipt.producer.command != "node scripts/header_receipt.js"
        || receipt.artifact_receipt_id != artifact.receipt_id
        || receipt.header_manifest.path != "metadata/headers.json"
        || receipt.fixture_manifest.path != "metadata/fixtures/family-roots.json"
        || receipt.header_manifest.sha256 != sha256_file(&root.join(HEADER_MANIFEST_PATH))?
        || receipt.fixture_manifest.sha256 != sha256_file(&root.join(FAMILY_FIXTURES_PATH))?
    {
        return Err(
            "typed header-dispatch receipt identity or manifest digest drifted".to_string(),
        );
    }
    if manifest.schema_version != 3
        || manifest
            .authorities
            .pointer("/mermaid/version")
            .and_then(serde_json::Value::as_str)
            != Some("11.16.1")
        || manifest
            .authorities
            .pointer("/mermaid/commit")
            .and_then(serde_json::Value::as_str)
            != Some("7ecca0cd7f1658ef74f4e7e91f925724ef403bbf")
        || manifest
            .authorities
            .pointer("/zenuml/version")
            .and_then(serde_json::Value::as_str)
            != Some("3.50.1")
        || manifest
            .authorities
            .pointer("/zenuml/commit")
            .and_then(serde_json::Value::as_str)
            != Some("38404ccc14243ed54ab45b804b2eb6f2ca73af36")
        || manifest
            .cases
            .iter()
            .any(|case| case.expected_diagram_type.is_empty())
        || CONCATENATED_HEADER_NEGATIVES.iter().any(|source| {
            !manifest
                .strict_header_negatives
                .iter()
                .any(|negative| negative == source)
        })
    {
        return Err("header manifest authority identity drifted".to_string());
    }

    let expected_cases = fixtures
        .iter()
        .map(|case| {
            (
                "baseline".to_string(),
                case.public_id.clone(),
                sha256_bytes(case.source.as_bytes()),
                case.root.clone(),
                None,
            )
        })
        .chain(manifest.cases.iter().map(|case| {
            (
                "header".to_string(),
                case.public_id.clone(),
                sha256_bytes(case.source.as_bytes()),
                case.root.clone(),
                Some(case.expected_diagram_type.clone()),
            )
        }))
        .chain(eof_cases.iter().map(|case| {
            (
                "header-eof".to_string(),
                case.public_id.clone(),
                sha256_bytes(case.source.as_bytes()),
                case.root.clone(),
                Some(case.expected_diagram_type.clone()),
            )
        }))
        .collect::<BTreeSet<_>>();
    if expected_cases.len() != fixtures.len() + manifest.cases.len() + eof_cases.len() {
        return Err("header or fixture manifest repeats a positive case".to_string());
    }
    let mut actual_cases = BTreeSet::new();
    for case in &receipt.cases {
        if !matches!(case.kind.as_str(), "baseline" | "header" | "header-eof")
            || !valid_sha256(&case.input_sha256)
            || case.has_error
            || case.actual_root != case.expected_root
            || !actual_cases.insert((
                case.kind.clone(),
                case.public_id.clone(),
                case.input_sha256.clone(),
                case.expected_root.clone(),
                case.expected_diagram_type.clone(),
            ))
        {
            return Err("typed header-dispatch receipt contains an invalid result".to_string());
        }
    }
    if actual_cases != expected_cases {
        return Err("typed header-dispatch receipt does not cover the exact manifests".to_string());
    }

    let expected_negatives = manifest
        .strict_header_negatives
        .iter()
        .map(|source| sha256_bytes(source.as_bytes()))
        .collect::<Vec<_>>();
    let actual_negatives = receipt
        .negative_cases
        .iter()
        .map(|case| {
            if !valid_sha256(&case.input_sha256)
                || (!case.has_error && !case.actual_roots.is_empty())
            {
                return Err("typed header-dispatch receipt admitted a detector-only input");
            }
            Ok(case.input_sha256.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if actual_negatives != expected_negatives {
        return Err("typed header-dispatch receipt detector negatives drifted".to_string());
    }
    let actual_eof_negatives = receipt
        .eof_negative_cases
        .iter()
        .map(|case| {
            if !valid_sha256(&case.input_sha256)
                || (!case.has_error && !case.actual_roots.is_empty())
            {
                return Err("typed header-dispatch receipt admitted a strict-rejected EOF header");
            }
            Ok((case.public_id.clone(), case.input_sha256.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_eof_negatives = eof_negative_cases
        .iter()
        .map(|case| (case.public_id.clone(), sha256_bytes(case.source.as_bytes())))
        .collect::<Vec<_>>();
    if actual_eof_negatives != expected_eof_negatives {
        return Err("typed header-dispatch receipt EOF negatives drifted".to_string());
    }
    for kind in ["baseline", "header"] {
        if !receipt.cases.iter().any(|case| {
            case.kind == kind
                && case.public_id == family.public_id
                && case.expected_root == family.root_node
        }) {
            return Err(format!(
                "family {} lacks typed {kind} dispatch evidence",
                family.public_id
            ));
        }
    }
    if eof_cases
        .iter()
        .any(|case| case.public_id == family.public_id)
        && !receipt.cases.iter().any(|case| {
            case.kind == "header-eof"
                && case.public_id == family.public_id
                && case.expected_root == family.root_node
        })
    {
        return Err(format!(
            "family {} lacks typed header-eof dispatch evidence",
            family.public_id
        ));
    }
    Ok(())
}

fn evidence_path_matches_kind(kind: &str, path: &str) -> bool {
    let under = |prefix: &str| path.starts_with(prefix) && path.len() > prefix.len();
    match kind {
        "binding" => under("test/bindings/") || path == "metadata/artifact-receipt.json",
        "conformance" => under("test/conformance/"),
        "corpus" => under("test/corpus/") && path.ends_with(".txt"),
        "header" => path == HEADER_RECEIPT_PACKAGE_PATH,
        "fuzz" => under("fuzz/corpus/"),
        "incremental" => under("test/edits/") && path.ends_with(".json"),
        "metrics" => under("metadata/metrics/") && path.ends_with(".json"),
        "node-schema" => {
            path == "src/node-types.json" || (under("test/schema/") && path.ends_with(".json"))
        }
        "query" => under("test/queries/"),
        "recovery" => {
            (under("test/corpus/") && path.ends_with(".txt")) || under("test/adversarial/")
        }
        _ => false,
    }
}

fn validate_evidence(
    root: &Path,
    family: &FamilySupport,
) -> Result<BTreeMap<String, String>, String> {
    let mut ids = BTreeSet::new();
    let mut kinds = BTreeMap::new();
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
        if !EVIDENCE_KINDS.contains(&evidence.kind.as_str()) {
            return Err(format!(
                "family {} evidence {} has unknown kind {:?}",
                family.public_id, evidence.id, evidence.kind
            ));
        }
        if !evidence_path_matches_kind(&evidence.kind, &evidence.path) {
            return Err(format!(
                "family {} evidence {} kind {:?} must use its runner-owned path",
                family.public_id, evidence.id, evidence.kind
            ));
        }
        if evidence.sha256.len() != 64
            || !evidence
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "family {} evidence {} has an invalid SHA-256",
                family.public_id, evidence.id
            ));
        }
        let path = package_evidence_path(root, &evidence.path)?;
        let actual = sha256_file(&path)?;
        if actual != evidence.sha256 {
            return Err(format!(
                "family {} evidence {} digest drifted: expected {}, actual {actual}",
                family.public_id, evidence.id, evidence.sha256
            ));
        }
        if evidence.kind == "header" {
            validate_header_receipt(root, family)?;
        }
        kinds.insert(evidence.id.clone(), evidence.kind.clone());
    }
    Ok(kinds)
}

fn validate_required_evidence(
    family: &FamilySupport,
    tier_rank: u8,
    evidence_kinds: &BTreeMap<String, String>,
) -> Result<(), String> {
    let required: &[&str] = match tier_rank {
        0 => &[],
        1 => &["header"],
        2 => &["header", "corpus", "recovery", "incremental", "node-schema"],
        3 => &[
            "header",
            "corpus",
            "recovery",
            "incremental",
            "node-schema",
            "query",
        ],
        4 => &[
            "header",
            "corpus",
            "recovery",
            "incremental",
            "node-schema",
            "query",
            "conformance",
            "binding",
            "fuzz",
            "metrics",
        ],
        _ => unreachable!("support tier rank is validated above"),
    };
    for kind in required {
        if !evidence_kinds.values().any(|actual| actual == kind) {
            return Err(format!(
                "family {} support tier lacks {kind} evidence",
                family.public_id
            ));
        }
    }
    Ok(())
}

fn validate_query_applicability(
    family: &FamilySupport,
    complete: bool,
    evidence_kinds: &BTreeMap<String, String>,
) -> Result<(), String> {
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
                "asserted" => {
                    for evidence_id in &applicability.evidence {
                        if evidence_kinds.get(evidence_id).map(String::as_str) != Some("query") {
                            return Err(format!(
                                "family {} {profile}/{surface} references unverified query evidence {evidence_id}",
                                family.public_id
                            ));
                        }
                    }
                }
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

fn validate_query_profile_coverage(
    support: &SupportMetadata,
    receipt: &ArtifactReceipt,
) -> Result<(), String> {
    let packaged = receipt
        .query_profiles
        .iter()
        .map(|query| (query.profile.as_str(), query.surface.as_str()))
        .collect::<BTreeSet<_>>();
    let asserted = support
        .families
        .iter()
        .flat_map(|family| {
            family
                .query_applicability
                .iter()
                .flat_map(|(profile, surfaces)| {
                    surfaces.iter().filter_map(move |(surface, applicability)| {
                        (applicability.status == "asserted")
                            .then_some((profile.as_str(), surface.as_str()))
                    })
                })
        })
        .collect::<BTreeSet<_>>();

    let missing = asserted.difference(&packaged).copied().collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "asserted query applicability lacks packaged profiles: {missing:?}"
        ));
    }
    let unused = packaged.difference(&asserted).copied().collect::<Vec<_>>();
    if !unused.is_empty() {
        return Err(format!(
            "packaged query profiles lack asserted family coverage: {unused:?}"
        ));
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
        || provenance.language.symbol != LANGUAGE_SYMBOL
        || provenance.language.abi != LANGUAGE_ABI
        || provenance.language.cst_schema_version != NODE_SCHEMA_VERSION
        || provenance.language.query_schema_version != QUERY_SCHEMA_VERSION
    {
        return Err(
            "package or language identity does not match the admitted bootstrap".to_string(),
        );
    }
    if provenance.toolchain.tree_sitter_cli != TREE_SITTER_CLI_VERSION
        || provenance.toolchain.rust_runtime != TREE_SITTER_RUST_RUNTIME_VERSION
        || provenance.toolchain.node_runtime != TREE_SITTER_NODE_VERSION
        || provenance.toolchain.web_runtime != TREE_SITTER_WEB_VERSION
        || provenance.toolchain.wasi_sdk != TREE_SITTER_WASI_SDK_VERSION
        || provenance.toolchain.wasi_clang != TREE_SITTER_WASI_CLANG_VERSION
    {
        return Err("Tree-sitter runtime/toolchain identities drifted".to_string());
    }
    let required = [
        "merman-oracle",
        "tree-sitter",
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
            || !source
                .commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    let merman = provenance
        .sources
        .iter()
        .find(|source| source.id == "merman-oracle")
        .expect("the exact provenance source set was validated above");
    if merman.kind != "one-way-conformance-oracle"
        || merman.version != MERMAN_ORACLE_VERSION
        || merman.repository != "https://github.com/Latias94/merman.git"
        || merman.r#ref != MERMAN_ORACLE_COMMIT
        || merman.commit != MERMAN_ORACLE_COMMIT
        || merman.license != "MIT OR Apache-2.0"
        || merman.legal_component_id.is_some()
    {
        return Err("Merman oracle provenance identity drifted".to_string());
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_derivations(
    root: &Path,
    derivations: &DerivationMetadata,
    provenance: &ProvenanceMetadata,
) -> Result<(), String> {
    if derivations.schema_version != 1
        || derivations.package != "tree-sitter-mermaid"
        || derivations.derivations.is_empty()
    {
        return Err("derivation metadata identity is invalid".to_string());
    }
    let source_ids = provenance
        .sources
        .iter()
        .map(|source| source.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_paths = ATTRIBUTED_PACKAGE_FILES
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut actual_paths = BTreeSet::new();
    let mut pappasam_modified = false;

    for derivation in &derivations.derivations {
        if derivation.local_paths.is_empty()
            || derivation.local_symbols.is_empty()
            || derivation.sources.is_empty()
            || derivation
                .local_symbols
                .iter()
                .any(|symbol| symbol.trim().is_empty())
        {
            return Err("derivation entry is incomplete".to_string());
        }
        for local_path in &derivation.local_paths {
            package_file_path(root, local_path, "derivation")?;
            if !actual_paths.insert(local_path.as_str()) {
                return Err(format!("derivation path {local_path:?} is repeated"));
            }
        }
        for source in &derivation.sources {
            if !source_ids.contains(source.source_id.as_str())
                || !matches!(
                    source.relationship.as_str(),
                    "behavior-reference" | "copied" | "modified" | "translated"
                )
                || source.source_paths.is_empty()
                || source
                    .source_paths
                    .iter()
                    .any(|path| path.trim().is_empty())
            {
                return Err(format!(
                    "derivation source {} is incomplete or unknown",
                    source.source_id
                ));
            }
            if source.source_id == "pappasam-tree-sitter-mermaid"
                && source.relationship == "modified"
            {
                pappasam_modified = true;
            }
        }
    }
    if actual_paths != expected_paths {
        return Err(format!(
            "derivation paths differ from attributed package files; expected={expected_paths:?}, actual={actual_paths:?}"
        ));
    }
    if !pappasam_modified {
        return Err("pappasam-derived paths are not marked modified".to_string());
    }
    Ok(())
}

fn validate_receipt_files(
    root: &Path,
    files: &[ReceiptFile],
    purpose: &str,
) -> Result<BTreeMap<String, u64>, String> {
    let mut paths = BTreeMap::new();
    for file in files {
        if !valid_sha256(&file.sha256) || paths.insert(file.path.clone(), file.bytes).is_some() {
            return Err(format!(
                "{purpose} receipt entry {:?} is invalid",
                file.path
            ));
        }
        let path = package_file_path(root, &file.path, purpose)?;
        let actual_bytes = path
            .metadata()
            .map_err(|error| format!("failed to stat {}: {error}", path.display()))?
            .len();
        if actual_bytes != file.bytes || sha256_file(&path)? != file.sha256 {
            return Err(format!(
                "{purpose} receipt entry {} differs from package bytes",
                file.path
            ));
        }
    }
    Ok(paths)
}

fn package_query_profiles(root: &Path) -> Result<BTreeMap<(String, String), String>, String> {
    let query_root = root.join(PACKAGE_ROOT).join("queries");
    let known_profiles = QUERY_PROFILES.into_iter().collect::<BTreeSet<_>>();
    let known_surfaces = QUERY_SURFACES.into_iter().collect::<BTreeSet<_>>();
    let mut profiles = BTreeMap::new();

    for profile_entry in fs::read_dir(&query_root)
        .map_err(|error| format!("failed to read {}: {error}", query_root.display()))?
    {
        let profile_entry = profile_entry
            .map_err(|error| format!("failed to inspect {}: {error}", query_root.display()))?;
        let profile_path = profile_entry.path();
        if !profile_path.is_dir() {
            continue;
        }
        let profile = profile_entry.file_name().to_string_lossy().into_owned();
        if !known_profiles.contains(profile.as_str()) {
            return Err(format!("package names unknown query profile {profile}"));
        }

        for surface_entry in fs::read_dir(&profile_path)
            .map_err(|error| format!("failed to read {}: {error}", profile_path.display()))?
        {
            let surface_entry = surface_entry.map_err(|error| {
                format!("failed to inspect {}: {error}", profile_path.display())
            })?;
            let surface_path = surface_entry.path();
            if surface_path.is_dir() {
                return Err(format!(
                    "query profile files must not be nested: {}",
                    surface_path.display()
                ));
            }
            if surface_path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("scm")
            {
                continue;
            }
            let surface = surface_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    format!(
                        "query surface path is not UTF-8: {}",
                        surface_path.display()
                    )
                })?
                .to_string();
            if !known_surfaces.contains(surface.as_str()) {
                return Err(format!(
                    "package names unknown query surface {profile}/{surface}"
                ));
            }
            let relative = format!("queries/{profile}/{surface}.scm");
            if profiles
                .insert((profile.clone(), surface.clone()), relative)
                .is_some()
            {
                return Err(format!(
                    "package duplicates query profile {profile}/{surface}"
                ));
            }
        }
    }

    if profiles.is_empty() {
        return Err("package contains no query profiles".to_string());
    }
    Ok(profiles)
}

fn validate_artifact_receipt(
    root: &Path,
    receipt: &ArtifactReceipt,
    provenance: &ProvenanceMetadata,
) -> Result<(), String> {
    let mut canonical_body =
        serde_json::to_value(receipt).map_err(|error| format!("invalid receipt body: {error}"))?;
    canonical_body
        .as_object_mut()
        .expect("serialized artifact receipt must be an object")
        .remove("receiptId");
    canonical_body.sort_all_objects();
    let canonical_bytes = serde_json::to_vec(&canonical_body)
        .map_err(|error| format!("failed to serialize receipt body: {error}"))?;
    let mut expected_receipt_id = String::with_capacity(64);
    for byte in Sha256::digest(canonical_bytes) {
        write!(&mut expected_receipt_id, "{byte:02x}").expect("writing to String cannot fail");
    }

    if receipt.schema_version != 1
        || !valid_sha256(&receipt.receipt_id)
        || receipt.package.name != "tree-sitter-mermaid"
        || receipt.package.version != PACKAGE_VERSION
        || receipt.package.release_state != "dry-run-only"
        || receipt.language.symbol != LANGUAGE_SYMBOL
        || receipt.language.abi != LANGUAGE_ABI
        || receipt.language.cst_schema_version != NODE_SCHEMA_VERSION
        || receipt.language.query_schema_version != QUERY_SCHEMA_VERSION
    {
        return Err("artifact receipt digest, package, or language identity drifted".to_string());
    }
    if receipt.toolchain.tree_sitter_cli != TREE_SITTER_CLI_VERSION
        || receipt.toolchain.rust_runtime != TREE_SITTER_RUST_RUNTIME_VERSION
        || receipt.toolchain.node_runtime != TREE_SITTER_NODE_VERSION
        || receipt.toolchain.web_runtime != TREE_SITTER_WEB_VERSION
        || receipt.toolchain.wasi_sdk != TREE_SITTER_WASI_SDK_VERSION
        || receipt.toolchain.wasi_clang != TREE_SITTER_WASI_CLANG_VERSION
    {
        return Err("artifact receipt toolchain identity drifted".to_string());
    }
    if !receipt.generation.is_object() {
        return Err("artifact receipt lacks generation commands".to_string());
    }

    let source_by_id = provenance
        .sources
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    for (receipt_id, source_id) in [
        ("merman-oracle", "merman-oracle"),
        ("mermaid", "mermaid"),
        ("zenuml-core", "zenuml-core"),
    ] {
        let baseline = receipt
            .baselines
            .get(receipt_id)
            .ok_or_else(|| format!("artifact receipt lacks {receipt_id} baseline"))?;
        let source = source_by_id
            .get(source_id)
            .expect("provenance source set was validated");
        if baseline.version != source.version || baseline.commit != source.commit {
            return Err(format!("artifact receipt {receipt_id} baseline drifted"));
        }
    }
    if receipt.baselines.len() != 3 {
        return Err("artifact receipt has unexpected baselines".to_string());
    }

    let artifacts = validate_receipt_files(root, &receipt.artifacts, "artifact")?;
    let expected_artifacts = GENERATED_ARTIFACTS
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if artifacts.keys().cloned().collect::<BTreeSet<_>>() != expected_artifacts {
        return Err("artifact receipt file set drifted".to_string());
    }
    let inputs = validate_receipt_files(root, &receipt.inputs, "input")?;
    for required in [
        "grammar.js",
        "src/scanner.c",
        "package-lock.json",
        "metadata/provenance.json",
        "metadata/derivations.json",
        "metadata/headers.json",
        "metadata/evidence/u2-mermaid-header-oracle.json",
        "bindings/rust/lib.rs",
        "bindings/query-profiles.js",
        "bindings/node/index.js",
        "bindings/wasm/index.js",
        "bindings/c/tree_sitter/tree-sitter-mermaid.h.in",
        "scripts/header_receipt.js",
        "scripts/header_oracle.js",
        "scripts/header-oracle/package.json",
        "scripts/header-oracle/package-lock.json",
    ] {
        if !inputs.contains_key(required) {
            return Err(format!("artifact receipt lacks required input {required}"));
        }
    }
    let expected_query_profiles = package_query_profiles(root)?;
    let mut receipt_query_profiles = BTreeMap::new();
    for query_profile in &receipt.query_profiles {
        let key = (query_profile.profile.clone(), query_profile.surface.clone());
        let Some(expected_path) = expected_query_profiles.get(&key) else {
            return Err(format!(
                "artifact receipt names unknown query profile {}/{}",
                query_profile.profile, query_profile.surface
            ));
        };
        if query_profile.path != *expected_path
            || !valid_sha256(&query_profile.sha256)
            || inputs.get(&query_profile.path) != Some(&query_profile.bytes)
        {
            return Err(format!(
                "artifact receipt query profile identity drifted for {}/{}",
                query_profile.profile, query_profile.surface
            ));
        }
        let query_path = package_file_path(root, &query_profile.path, "query profile")?;
        if query_path
            .metadata()
            .map_err(|error| format!("failed to stat {}: {error}", query_path.display()))?
            .len()
            != query_profile.bytes
            || sha256_file(&query_path)? != query_profile.sha256
        {
            return Err(format!(
                "artifact receipt query profile differs from package bytes for {}/{}",
                query_profile.profile, query_profile.surface
            ));
        }
        if receipt_query_profiles
            .insert(key, query_profile.path.clone())
            .is_some()
        {
            return Err("artifact receipt duplicates a query profile".to_string());
        }
    }
    if receipt_query_profiles != expected_query_profiles {
        return Err("artifact receipt query profile set drifted".to_string());
    }
    if receipt.receipt_id != expected_receipt_id {
        return Err("artifact receipt digest does not match its canonical body".to_string());
    }
    let c_header = fs::read_to_string(
        root.join(PACKAGE_ROOT)
            .join("bindings/c/tree_sitter/tree-sitter-mermaid.h"),
    )
    .map_err(|error| format!("failed to read generated C receipt carrier: {error}"))?;
    let expected_c_carrier = format!(
        "#define TREE_SITTER_MERMAID_ARTIFACT_RECEIPT_ID \"{}\"",
        receipt.receipt_id
    );
    if !c_header.lines().any(|line| line == expected_c_carrier) {
        return Err("generated C binding carries a different artifact receipt".to_string());
    }

    let compiled_receipt: ArtifactReceipt = serde_json::from_str(ARTIFACT_RECEIPT)
        .map_err(|error| format!("compiled artifact receipt is invalid: {error}"))?;
    if compiled_receipt.receipt_id != receipt.receipt_id {
        return Err("compiled Rust binding carries a different artifact receipt".to_string());
    }
    Ok(())
}

fn parser_define(source: &str, name: &str) -> Result<u64, String> {
    let prefix = format!("#define {name} ");
    source
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .ok_or_else(|| format!("generated parser lacks {name}"))
        .and_then(|value| {
            value
                .parse::<u64>()
                .map_err(|error| format!("generated parser {name} is invalid: {error}"))
        })
}

fn validate_metrics(
    root: &Path,
    metrics: &MechanicsMetrics,
    receipt: &ArtifactReceipt,
) -> Result<(), String> {
    if metrics.schema_version != 1
        || metrics.checkpoint != "u9-conformant"
        || metrics.artifact_receipt_id != receipt.receipt_id
    {
        return Err("mechanics metrics identity drifted".to_string());
    }
    validate_metrics_attribution(metrics)?;
    let incremental = &metrics.observed.fresh_and_incremental_work;
    let metric = |name: &str| {
        incremental
            .get(name)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("U2 metrics lack numeric {name}"))
    };
    let source_bytes = metric("sourceBytes")?;
    let edit_byte = metric("editByte")?;
    let fresh_bytes = metric("freshSuppliedBytes")?;
    let fresh_coverage = metric("freshUniqueCoverageBytes")?;
    let incremental_bytes = metric("incrementalSuppliedBytes")?;
    let incremental_coverage = metric("incrementalUniqueCoverageBytes")?;
    let fresh_work = metric("freshProgressCallbacks")?;
    let incremental_work = metric("incrementalProgressCallbacks")?;
    let read_limit = metric("maxIncrementalSuppliedPermille")?;
    let work_limit = metric("maxIncrementalProgressPermille")?;
    if source_bytes < 256 * 1024
        || metric("inputChunkBytes")? != 64
        || edit_byte < source_bytes / 3
        || edit_byte > source_bytes * 2 / 3
        || fresh_bytes < source_bytes
        || fresh_coverage != source_bytes
        || incremental_bytes * 1000 > source_bytes * read_limit
        || incremental_coverage * 1000 > source_bytes * read_limit
        || incremental_work * 1000 > fresh_work * work_limit
        || read_limit != 10
        || work_limit != 250
        || metric("changedNamedNodes")? > 16
        || incremental
            .get("measurement")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err("U2 fresh/incremental work ratchet drifted".to_string());
    }

    let short = &metrics.observed.common_short_statement_local_edits;
    let short_metric = |value: &serde_json::Value, name: &str| {
        value
            .get(name)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("U2 short-edit metrics lack numeric {name}"))
    };
    let short_source = short_metric(short, "sourceBytes")?;
    let short_fresh_work = short_metric(short, "freshProgressCallbacks")?;
    let short_supplied_limit = short_metric(short, "maxIncrementalSuppliedBytes")?;
    let short_coverage_limit = short_metric(short, "maxIncrementalUniqueCoverageBytes")?;
    let short_work_limit = short_metric(short, "maxIncrementalProgressPermille")?;
    let operations = short
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "U2 short-edit metrics lack operations".to_string())?;
    if short_source < 256 * 1024
        || short_metric(short, "inputChunkBytes")? != 64
        || short_metric(short, "freshSuppliedBytes")? < short_source
        || short_metric(short, "freshUniqueCoverageBytes")? != short_source
        || short_supplied_limit != 4096
        || short_coverage_limit != 4096
        || short_work_limit != 250
        || operations.len() != 3
        || short
            .get("measurement")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err("U2 common short-statement edit ratchet drifted".to_string());
    }
    for (operation, (expected_operation, source_delta)) in operations.iter().zip([
        ("replace", 0_i64),
        ("insert-statement", 10),
        ("delete-statement", -10),
    ]) {
        if operation
            .get("operation")
            .and_then(serde_json::Value::as_str)
            != Some(expected_operation)
        {
            return Err("U2 common short-statement operation identity drifted".to_string());
        }
        let positions = operation
            .get("positions")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "U2 short-edit operation lacks positions".to_string())?;
        if positions.len() != 3 {
            return Err("U2 short-edit operation position count drifted".to_string());
        }
        for (index, (position, expected_name)) in positions
            .iter()
            .zip(["quarter", "middle", "three-quarter"])
            .enumerate()
        {
            let edit_byte = short_metric(position, "editByte")?;
            let requested = short_source * (index as u64 + 1) / 4;
            let edited_source = short_metric(position, "editedSourceBytes")?;
            let supplied = short_metric(position, "incrementalSuppliedBytes")?;
            let coverage = short_metric(position, "incrementalUniqueCoverageBytes")?;
            let work = short_metric(position, "incrementalProgressCallbacks")?;
            let fresh_position_work = short_metric(position, "freshProgressCallbacks")?;
            if position.get("position").and_then(serde_json::Value::as_str) != Some(expected_name)
                || edit_byte + 64 < requested
                || edit_byte > requested + 64
                || edited_source as i64 != short_source as i64 + source_delta
                || short_metric(position, "freshSuppliedBytes")? < edited_source
                || short_metric(position, "freshUniqueCoverageBytes")? != edited_source
                || fresh_position_work.abs_diff(short_fresh_work) > 1
                || supplied > short_supplied_limit
                || coverage > short_coverage_limit
                || work * 1000 > short_fresh_work * short_work_limit
            {
                return Err(format!(
                    "U2 common short-statement {expected_operation} {expected_name} metrics drifted"
                ));
            }
        }
    }

    let environment = &metrics.observed.environment;
    if environment.os != "darwin"
        || environment.architecture != "arm64"
        || environment.rust != "1.95.0"
        || environment.node != "26.7.0"
        || environment.tree_sitter_cli != receipt.toolchain.tree_sitter_cli
        || environment.wasi_sdk != receipt.toolchain.wasi_sdk
    {
        return Err("U2 metrics measurement environment drifted".to_string());
    }

    let build = &metrics.observed.build;
    let compile_limit = metrics.ratchet.independent_compile_hard_limit_milliseconds;
    let generation_limit = metrics.ratchet.generation_hard_limit_milliseconds;
    let wasm_build_limit = metrics.ratchet.canonical_wasm_build_hard_limit_milliseconds;
    if generation_limit != 120_000
        || wasm_build_limit != 120_000
        || compile_limit != 120_000
        || build.two_runtime_one_wasm_generation_wall_milliseconds == 0
        || build.two_runtime_one_wasm_generation_wall_milliseconds > generation_limit
        || build.canonical_wasm_build_wall_milliseconds == 0
        || build.canonical_wasm_build_wall_milliseconds > wasm_build_limit
        || build.rust_release_compile_wall_milliseconds == 0
        || build.rust_release_compile_wall_milliseconds > compile_limit
        || build.node_binding_compile_wall_milliseconds == 0
        || build.node_binding_compile_wall_milliseconds > compile_limit
        || build.measurement.is_empty()
    {
        return Err("U2 generation or independent compile metrics drifted".to_string());
    }

    let native_parse = metrics.observed.native_node_smoke_parse_milliseconds;
    let wasm_parse = metrics.observed.wasm_node_smoke_parse_milliseconds;
    if !native_parse.is_finite()
        || native_parse <= 0.0
        || native_parse > metrics.ratchet.native_smoke_parse_hard_limit_milliseconds as f64
        || !wasm_parse.is_finite()
        || wasm_parse <= 0.0
        || wasm_parse > metrics.ratchet.wasm_smoke_parse_hard_limit_milliseconds as f64
        || metrics
            .observed
            .native_node_smoke_maximum_resident_set_bytes
            == 0
        || metrics
            .observed
            .native_node_smoke_maximum_resident_set_bytes
            > metrics.ratchet.native_peak_rss_investigate_above_bytes
        || metrics.observed.wasm_node_smoke_maximum_resident_set_bytes == 0
        || metrics.observed.wasm_node_smoke_maximum_resident_set_bytes
            > metrics.ratchet.wasm_peak_rss_investigate_above_bytes
        || metrics.observed.smoke_measurement.is_empty()
    {
        return Err("U2 native or WASM smoke performance metrics drifted".to_string());
    }

    let real = &metrics.observed.real_corpus;
    let real_metric = |name: &str| {
        real.get(name)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("U2 real-corpus metrics lack numeric {name}"))
    };
    let real_source = real_metric("sourceBytes")?;
    let real_observed_wall = real_metric("observedFreshWallMilliseconds")?;
    let real_wall_limit = real_metric("maxFreshWallMilliseconds")?;
    if real_metric("fixtureCount")? != PUBLIC_FAMILY_COUNT as u64
        || real
            .get("fixtureManifestSha256")
            .and_then(serde_json::Value::as_str)
            .is_none_or(|digest| !valid_sha256(digest))
        || real_source == 0
        || real_metric("freshSuppliedBytes")? < real_source
        || real_metric("freshUniqueCoverageBytes")? != real_source
        || real_metric("freshProgressCallbacks")? == 0
        || real_observed_wall == 0
        || real_observed_wall > real_wall_limit
        || real_wall_limit > 2_000
        || real
            .get("measurement")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err("U2 real-corpus metrics drifted".to_string());
    }

    let doubling = &metrics.observed.synthetic_doubling;
    if doubling.get("fixture").and_then(serde_json::Value::as_str)
        != Some("synthetic-flowchart-1k-label-statements")
        || doubling
            .get("inputChunkBytes")
            .and_then(serde_json::Value::as_u64)
            != Some(64)
        || doubling
            .get("maxConsecutiveGrowthPermille")
            .and_then(serde_json::Value::as_u64)
            != Some(3_000)
        || doubling
            .get("runtimeTrials")
            .and_then(serde_json::Value::as_u64)
            != Some(3)
        || doubling
            .get("measurement")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err("U2 synthetic-doubling identity drifted".to_string());
    }
    let doubling_lanes = doubling
        .get("lanes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "U2 metrics lack synthetic-doubling lanes".to_string())?;
    if doubling_lanes.len() != 5 {
        return Err("U2 synthetic-doubling lane count drifted".to_string());
    }
    let mut prior_fresh_work = None;
    let mut prior_growth_was_threefold = false;
    let mut runtime_series = BTreeMap::<&str, Vec<f64>>::from([
        ("native parse", Vec::new()),
        ("native query", Vec::new()),
        ("native RSS", Vec::new()),
        ("WASM parse", Vec::new()),
        ("WASM query", Vec::new()),
        ("WASM RSS", Vec::new()),
        ("WASM pages", Vec::new()),
    ]);
    for (lane, target_kib) in doubling_lanes.iter().zip([64_u64, 128, 256, 512, 1024]) {
        let lane_metric = |name: &str| {
            lane.get(name)
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| format!("U2 doubling metrics lack numeric {name}"))
        };
        let source = lane_metric("sourceBytes")?;
        let fresh_work = lane_metric("freshProgressCallbacks")?;
        let incremental_work = lane_metric("incrementalProgressCallbacks")?;
        let observed_wall = lane_metric("observedFreshWallMilliseconds")?;
        let wall_limit = lane_metric("maxFreshWallMilliseconds")?;
        if lane_metric("targetKiB")? != target_kib
            || source < target_kib * 1024
            || source > target_kib * 1024 + 16
            || lane_metric("editByte")? < source / 3
            || lane_metric("editByte")? > source * 2 / 3
            || lane_metric("freshSuppliedBytes")? < source
            || lane_metric("freshUniqueCoverageBytes")? != source
            || fresh_work == 0
            || lane_metric("incrementalSuppliedBytes")? > 4_096
            || lane_metric("incrementalUniqueCoverageBytes")? > 4_096
            || incremental_work == 0
            || incremental_work * 1_000 > fresh_work * 250
            || observed_wall == 0
            || observed_wall > wall_limit
            || wall_limit > 2_000
        {
            return Err(format!("U2 {target_kib} KiB doubling lane drifted"));
        }
        if let Some(previous) = prior_fresh_work {
            let threefold = fresh_work >= previous * 3;
            if threefold && prior_growth_was_threefold {
                return Err(
                    "U2 fresh work has two consecutive at-least-threefold increases".to_string(),
                );
            }
            prior_growth_was_threefold = threefold;
        }
        prior_fresh_work = Some(fresh_work);

        for (runtime, label, parse_limit, rss_limit) in [
            (
                "nativeRuntime",
                "native",
                metrics.ratchet.native_smoke_parse_hard_limit_milliseconds,
                metrics.ratchet.native_peak_rss_investigate_above_bytes,
            ),
            (
                "wasmRuntime",
                "WASM",
                metrics.ratchet.wasm_smoke_parse_hard_limit_milliseconds,
                metrics.ratchet.wasm_peak_rss_investigate_above_bytes,
            ),
        ] {
            let snapshot = lane
                .get(runtime)
                .and_then(serde_json::Value::as_object)
                .ok_or_else(|| format!("U2 {target_kib} KiB {label} runtime metrics missing"))?;
            let timing = |name: &str| {
                snapshot
                    .get(name)
                    .and_then(serde_json::Value::as_f64)
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .ok_or_else(|| {
                        format!("U2 {target_kib} KiB {label} runtime lacks valid {name}")
                    })
            };
            let integer = |name: &str| {
                snapshot
                    .get(name)
                    .and_then(serde_json::Value::as_u64)
                    .filter(|value| *value > 0)
                    .ok_or_else(|| {
                        format!("U2 {target_kib} KiB {label} runtime lacks valid {name}")
                    })
            };
            let parse = timing("observedParseMilliseconds")?;
            let query_compile = timing("observedQueryCompileMilliseconds")?;
            let query = timing("observedQueryMilliseconds")?;
            let rss = integer("observedMaximumResidentSetBytes")?;
            if parse > parse_limit as f64
                || query_compile > metrics.ratchet.query_hard_limit_milliseconds as f64
                || query > metrics.ratchet.query_hard_limit_milliseconds as f64
                || rss > rss_limit
            {
                return Err(format!(
                    "U2 {target_kib} KiB {label} runtime metric exceeded its hard limit"
                ));
            }
            runtime_series
                .get_mut(if runtime == "nativeRuntime" {
                    "native parse"
                } else {
                    "WASM parse"
                })
                .expect("runtime series exists")
                .push(parse);
            runtime_series
                .get_mut(if runtime == "nativeRuntime" {
                    "native query"
                } else {
                    "WASM query"
                })
                .expect("runtime series exists")
                .push(query);
            runtime_series
                .get_mut(if runtime == "nativeRuntime" {
                    "native RSS"
                } else {
                    "WASM RSS"
                })
                .expect("runtime series exists")
                .push(rss as f64);
            if runtime == "wasmRuntime" {
                let pages = integer("observedMemoryPages")?;
                if integer("maxMemoryPages")? != 2_048 || pages > 2_048 {
                    return Err(format!(
                        "U2 {target_kib} KiB WASM runtime memory metric drifted"
                    ));
                }
                runtime_series
                    .get_mut("WASM pages")
                    .expect("runtime series exists")
                    .push(pages as f64);
            }
        }
    }

    for (name, values) in runtime_series {
        if values
            .windows(3)
            .any(|window| window[1] >= window[0] * 3.0 && window[2] >= window[1] * 3.0)
        {
            return Err(format!(
                "U2 {name} has two consecutive at-least-threefold increases"
            ));
        }
    }

    let query = &metrics.observed.query_time;
    let query_limit = metrics.ratchet.query_hard_limit_milliseconds as f64;
    if query.status != "measured"
        || query.measurement.is_empty()
        || [
            query.native_compile_milliseconds,
            query.native_execution_milliseconds,
            query.wasm_compile_milliseconds,
            query.wasm_execution_milliseconds,
        ]
        .into_iter()
        .any(|milliseconds| {
            !milliseconds.is_finite() || milliseconds <= 0.0 || milliseconds > query_limit
        })
    {
        return Err("U2 portable query timing metrics drifted".to_string());
    }

    let wasm_memory = &metrics.observed.wasm_runtime_memory_pages;
    if wasm_memory.status != "measured"
        || wasm_memory.declared_minimum_pages != metrics.r#static.wasm_declared_minimum_memory_pages
        || wasm_memory.initial_pages != 512
        || wasm_memory.observed_peak_pages < wasm_memory.initial_pages
        || wasm_memory.observed_peak_pages > wasm_memory.max_peak_pages
        || wasm_memory.max_peak_pages != 2_048
        || wasm_memory.stress_source_bytes < 1024 * 1024
        || wasm_memory.measurement.is_empty()
    {
        return Err("U2 WASM runtime memory metrics drifted".to_string());
    }

    let artifact_sizes = receipt
        .artifacts
        .iter()
        .map(|artifact| (artifact.path.as_str(), artifact.bytes))
        .collect::<BTreeMap<_, _>>();
    let parser_source = fs::read_to_string(root.join(PACKAGE_ROOT).join("src/parser.c"))
        .map_err(|error| format!("failed to read generated parser metrics: {error}"))?;
    let grammar: serde_json::Value =
        read_json(root, "distribution/tree-sitter-mermaid/src/grammar.json")?;
    let conflict_count = grammar
        .get("conflicts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "generated grammar lacks conflicts".to_string())?
        .len() as u64;
    let expected = [
        ("STATE_COUNT", metrics.r#static.parser_states),
        ("LARGE_STATE_COUNT", metrics.r#static.large_states),
        ("SYMBOL_COUNT", metrics.r#static.symbols),
        ("FIELD_COUNT", metrics.r#static.fields),
        ("EXTERNAL_TOKEN_COUNT", metrics.r#static.external_tokens),
    ];
    for (name, recorded) in expected {
        if parser_define(&parser_source, name)? != recorded {
            return Err(format!("U2 metrics {name} drifted"));
        }
    }
    if artifact_sizes.get("src/parser.c") != Some(&metrics.r#static.generated_c_bytes)
        || artifact_sizes.get("wasm/tree-sitter-mermaid.wasm") != Some(&metrics.r#static.wasm_bytes)
        || metrics.r#static.conflicts != conflict_count
        || metrics.r#static.conflicts > metrics.ratchet.conflicts_allowed
        || metrics.ratchet.conflicts_allowed != conflict_count
        || metrics.r#static.wasm_declared_minimum_memory_pages != 2
        || metrics.ratchet.generated_c_hard_limit_bytes != 10 * 1024 * 1024
        || metrics.ratchet.wasm_hard_limit_bytes != 5 * 1024 * 1024
        || metrics.ratchet.native_smoke_parse_hard_limit_milliseconds != 2_000
        || metrics.ratchet.wasm_smoke_parse_hard_limit_milliseconds != 2_000
        || metrics.ratchet.native_peak_rss_investigate_above_bytes != 256 * 1024 * 1024
        || metrics.ratchet.wasm_peak_rss_investigate_above_bytes != 768 * 1024 * 1024
        || metrics.ratchet.query_hard_limit_milliseconds != 2_000
        || metrics.r#static.generated_c_bytes > metrics.ratchet.generated_c_hard_limit_bytes
        || metrics.r#static.wasm_bytes > metrics.ratchet.wasm_hard_limit_bytes
        || metrics.r#static.parser_states > metrics.ratchet.parser_states_investigate_above
        || metrics.r#static.large_states > metrics.ratchet.large_states_investigate_above
    {
        return Err("mechanics metrics or ratchet limits drifted".to_string());
    }
    Ok(())
}

fn validate_metrics_attribution(metrics: &MechanicsMetrics) -> Result<(), String> {
    const U3_RECEIPT: &str = "33ad48cbc9d2dd2f0dbe390c3010cc073513313b1a3dd47c0ba37b2f77d5384f";
    const U8_FAMILIES: [&str; 27] = [
        "block",
        "c4",
        "class",
        "er",
        "eventmodeling",
        "flowchart",
        "gantt",
        "ishikawa",
        "journey",
        "kanban",
        "mindmap",
        "quadrantchart",
        "railroad",
        "railroadAbnf",
        "railroadEbnf",
        "railroadPeg",
        "requirement",
        "sankey",
        "sequence",
        "state",
        "swimlane",
        "timeline",
        "treeView",
        "treemap",
        "venn",
        "xychart",
        "zenuml",
    ];

    let attribution = &metrics.attribution;
    if attribution.previous_checkpoint != "u3-low-state-complexity"
        || attribution.previous_artifact_receipt_id != U3_RECEIPT
        || attribution.previous_artifact_receipt_id == metrics.artifact_receipt_id
        || attribution.structured_families_added != U8_FAMILIES.map(str::to_owned)
        || attribution.explanation.trim().is_empty()
    {
        return Err("U8 metrics attribution identity drifted".to_string());
    }

    let previous = &attribution.previous_static;
    if previous.generated_c_bytes != 2_282_299
        || previous.wasm_bytes != 822_514
        || previous.parser_states != 1_967
        || previous.large_states != 5
        || previous.symbols != 721
        || previous.fields != 103
        || previous.external_tokens != 15
        || previous.conflicts != 3
        || previous.wasm_declared_minimum_memory_pages != 2
    {
        return Err("U8 metrics attribution baseline drifted".to_string());
    }

    let delta = |current: u64, prior: u64| -> Result<i64, String> {
        let current = i64::try_from(current)
            .map_err(|_| "current static metric exceeds signed delta range".to_string())?;
        let prior = i64::try_from(prior)
            .map_err(|_| "previous static metric exceeds signed delta range".to_string())?;
        Ok(current - prior)
    };
    let current = &metrics.r#static;
    let recorded = &attribution.delta;
    let actual = StaticMetricsDelta {
        generated_c_bytes: delta(current.generated_c_bytes, previous.generated_c_bytes)?,
        wasm_bytes: delta(current.wasm_bytes, previous.wasm_bytes)?,
        parser_states: delta(current.parser_states, previous.parser_states)?,
        large_states: delta(current.large_states, previous.large_states)?,
        symbols: delta(current.symbols, previous.symbols)?,
        fields: delta(current.fields, previous.fields)?,
        external_tokens: delta(current.external_tokens, previous.external_tokens)?,
        conflicts: delta(current.conflicts, previous.conflicts)?,
        wasm_declared_minimum_memory_pages: delta(
            current.wasm_declared_minimum_memory_pages,
            previous.wasm_declared_minimum_memory_pages,
        )?,
    };
    if recorded.generated_c_bytes != actual.generated_c_bytes
        || recorded.wasm_bytes != actual.wasm_bytes
        || recorded.parser_states != actual.parser_states
        || recorded.large_states != actual.large_states
        || recorded.symbols != actual.symbols
        || recorded.fields != actual.fields
        || recorded.external_tokens != actual.external_tokens
        || recorded.conflicts != actual.conflicts
        || recorded.wasm_declared_minimum_memory_pages != actual.wasm_declared_minimum_memory_pages
    {
        return Err("U8 static metrics delta drifted".to_string());
    }
    Ok(())
}

fn validate_external_sources(
    provenance: &ProvenanceMetadata,
    lock: &RepositoryLock,
    legal: &ThirdPartyContract,
) -> Result<(), String> {
    for (source_id, expected_kind, expected_license, revision_may_drift) in [
        ("tree-sitter", "generator-and-template-source", "MIT", false),
        ("mermaid", "syntax-authority", "MIT", true),
        ("zenuml-core", "companion-syntax-authority", "MIT", true),
        (
            "pappasam-tree-sitter-mermaid",
            "implementation-seed",
            "MIT",
            false,
        ),
        (
            "monaqa-tree-sitter-mermaid",
            "downstream-compatibility-reference",
            "MIT",
            false,
        ),
        (
            "singularity-tree-sitter-mermaid",
            "behavior-reference",
            "MIT",
            false,
        ),
    ] {
        let source = provenance
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .expect("the exact provenance source set was validated above");
        let locked = lock
            .repos
            .get(source_id)
            .ok_or_else(|| format!("repository lock lacks {source_id}"))?;
        if source.kind != expected_kind
            || source.license != expected_license
            || source.repository != locked.url
        {
            return Err(format!(
                "provenance source {source_id} identity differs from its repository lock"
            ));
        }
        if !revision_may_drift && (source.r#ref != locked.r#ref || source.commit != locked.commit) {
            return Err(format!(
                "provenance source {source_id} revision differs from its repository lock"
            ));
        }

        let legal_id = source
            .legal_component_id
            .as_deref()
            .ok_or_else(|| format!("provenance source {source_id} lacks a legal component"))?;
        let component = legal
            .components
            .iter()
            .find(|component| component.id == legal_id)
            .ok_or_else(|| format!("third-party contract lacks component {legal_id}"))?;
        if component.version != source.version
            || component.source.repository != source.repository
            || component.source.r#ref != source.r#ref
            || component.source.commit != source.commit
            || component.license_expression != source.license
            || !component
                .local_paths
                .iter()
                .any(|path| path == PACKAGE_ROOT)
        {
            return Err(format!(
                "provenance source {source_id} differs from legal component {legal_id}"
            ));
        }
    }
    Ok(())
}

fn validate_package_legal_bundle(
    root: &Path,
    provenance: &ProvenanceMetadata,
) -> Result<(), String> {
    let notice_path = root.join(PACKAGE_ROOT).join("THIRD_PARTY_NOTICES.md");
    let notice = fs::read_to_string(&notice_path)
        .map_err(|error| format!("failed to read {}: {error}", notice_path.display()))?;
    for (source_id, repository_path, package_path) in PACKAGE_LICENSE_COPIES {
        let source = provenance
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .expect("the exact provenance source set was validated above");
        if !notice.contains(&source.repository)
            || !notice.contains(&source.commit)
            || !notice.contains(package_path)
        {
            return Err(format!(
                "package third-party notice lacks exact {source_id} provenance"
            ));
        }
        let repository_license = root.join(repository_path);
        let package_license = root.join(PACKAGE_ROOT).join(package_path);
        let repository_bytes = fs::read(&repository_license).map_err(|error| {
            format!(
                "failed to read repository license {}: {error}",
                repository_license.display()
            )
        })?;
        let package_bytes = fs::read(&package_license).map_err(|error| {
            format!(
                "failed to read package license {}: {error}",
                package_license.display()
            )
        })?;
        if repository_bytes != package_bytes {
            return Err(format!(
                "package license copy for {source_id} differs from legal authority"
            ));
        }
    }
    Ok(())
}

fn validate_baselines(
    support: &SupportMetadata,
    provenance: &ProvenanceMetadata,
    lock: &RepositoryLock,
) -> Result<(), String> {
    for (label, selected, lock_id, source_id, alignment) in [
        (
            "Mermaid",
            &support.selected_baselines.mermaid,
            "mermaid",
            "mermaid",
            support.repository_alignment.mermaid.as_str(),
        ),
        (
            "ZenUML",
            &support.selected_baselines.zenuml,
            "zenuml-core",
            "zenuml-core",
            support.repository_alignment.zenuml.as_str(),
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
        if selected.r#ref != source.r#ref
            || selected.commit != source.commit
            || selected.version != source.version
        {
            return Err(format!(
                "{label} identity drifted across support/provenance"
            ));
        }
        let expected_alignment =
            if selected.r#ref == locked.r#ref && selected.commit == locked.commit {
                "aligned"
            } else {
                "drifted"
            };
        if alignment != expected_alignment {
            return Err(format!(
                "{label} repository alignment must be {expected_alignment:?}, got {alignment:?}"
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

fn toml_integer(value: &toml::Value, path: &[&str]) -> Option<i64> {
    let mut current = value;
    for component in path {
        current = current.get(*component)?;
    }
    current.as_integer()
}

fn toml_bool(value: &toml::Value, path: &[&str]) -> Option<bool> {
    let mut current = value;
    for component in path {
        current = current.get(*component)?;
    }
    current.as_bool()
}

fn json_string_array(value: Option<&serde_json::Value>) -> Option<Vec<&str>> {
    value?
        .as_array()?
        .iter()
        .map(serde_json::Value::as_str)
        .collect()
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
        || toml_string(&package_manifest, &["package", "license"]) != Some("MIT")
        || package_manifest
            .get("package")
            .and_then(|package| package.get("publish"))
            .and_then(toml::Value::as_bool)
            != Some(false)
        || toml_string(&package_manifest, &["dependencies", "tree-sitter-language"])
            != Some("=0.1.7")
        || toml_string(&package_manifest, &["dev-dependencies", "tree-sitter"]) != Some("=0.26.12")
        || toml_string(&package_manifest, &["build-dependencies", "cc"]) != Some("1.2")
        || toml_integer(
            &package_manifest,
            &["package", "metadata", "tree-sitter-mermaid", "language-abi"],
        ) != Some(i64::from(LANGUAGE_ABI))
        || toml_integer(
            &package_manifest,
            &[
                "package",
                "metadata",
                "tree-sitter-mermaid",
                "node-schema-version",
            ],
        ) != Some(i64::from(NODE_SCHEMA_VERSION))
        || toml_integer(
            &package_manifest,
            &[
                "package",
                "metadata",
                "tree-sitter-mermaid",
                "query-schema-version",
            ],
        ) != Some(i64::from(QUERY_SCHEMA_VERSION))
        || toml_string(
            &package_manifest,
            &[
                "package",
                "metadata",
                "tree-sitter-mermaid",
                "release-state",
            ],
        ) != Some("dry-run-only")
        || toml_bool(
            &package_manifest,
            &["package", "metadata", "merman-legal", "third-party-bundle"],
        ) != Some(true)
    {
        return Err(
            "Cargo package identity, binding dependencies, legal bundle, or dry-run boundary drifted"
                .to_string(),
        );
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
    let dependencies = package_json
        .get("dependencies")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "npm dependencies are missing".to_string())?;
    for (name, version) in [("node-addon-api", "8.9.2"), ("node-gyp-build", "4.8.4")] {
        if dependencies.get(name).and_then(serde_json::Value::as_str) != Some(version) {
            return Err(format!("npm package does not pin {name} {version}"));
        }
    }
    let npm_language = package_json
        .get("tree-sitter")
        .and_then(serde_json::Value::as_array)
        .filter(|languages| languages.len() == 1)
        .and_then(|languages| languages.first())
        .ok_or_else(|| "npm package must register exactly one Tree-sitter language".to_string())?;
    if package_json.get("name").and_then(serde_json::Value::as_str) != Some("tree-sitter-mermaid")
        || package_json
            .get("version")
            .and_then(serde_json::Value::as_str)
            != Some(PACKAGE_VERSION)
        || package_json
            .get("license")
            .and_then(serde_json::Value::as_str)
            != Some("MIT")
        || package_json
            .get("private")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        || npm_language
            .get("scope")
            .and_then(serde_json::Value::as_str)
            != Some("source.mermaid")
        || json_string_array(npm_language.get("file-types")) != Some(vec!["mmd", "mermaid"])
        || npm_language
            .get("injection-regex")
            .and_then(serde_json::Value::as_str)
            != Some("^(mermaid|mmd)$")
    {
        return Err("npm package identity or dry-run boundary drifted".to_string());
    }

    let package_lock: serde_json::Value =
        read_json(root, &format!("{PACKAGE_ROOT}/package-lock.json"))?;
    if package_lock
        .pointer("/packages//name")
        .and_then(serde_json::Value::as_str)
        != Some("tree-sitter-mermaid")
        || package_lock
            .pointer("/packages//version")
            .and_then(serde_json::Value::as_str)
            != Some(PACKAGE_VERSION)
        || package_lock
            .pointer("/packages//license")
            .and_then(serde_json::Value::as_str)
            != Some("MIT")
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
    let grammar = tree_sitter_json
        .get("grammars")
        .and_then(serde_json::Value::as_array)
        .filter(|grammars| grammars.len() == 1)
        .and_then(|grammars| grammars.first())
        .ok_or_else(|| "tree-sitter.json must declare exactly one grammar".to_string())?;
    if grammar.get("name").and_then(serde_json::Value::as_str) != Some(LANGUAGE_SYMBOL)
        || grammar.get("camelcase").and_then(serde_json::Value::as_str) != Some("Mermaid")
        || grammar.get("scope").and_then(serde_json::Value::as_str) != Some("source.mermaid")
        || grammar.get("path").and_then(serde_json::Value::as_str) != Some(".")
        || grammar
            .get("highlights")
            .and_then(serde_json::Value::as_str)
            != Some("queries/portable/highlights.scm")
        || grammar
            .get("injections")
            .and_then(serde_json::Value::as_str)
            != Some("queries/portable/injections.scm")
        || grammar.get("locals").and_then(serde_json::Value::as_str)
            != Some("queries/portable/locals.scm")
        || grammar.get("tags").and_then(serde_json::Value::as_str)
            != Some("queries/portable/tags.scm")
        || json_string_array(grammar.get("file-types")) != Some(vec!["mmd", "mermaid"])
        || grammar
            .get("injection-regex")
            .and_then(serde_json::Value::as_str)
            != Some("^(mermaid|mmd)$")
        || tree_sitter_json
            .pointer("/metadata/version")
            .and_then(serde_json::Value::as_str)
            != Some(PACKAGE_VERSION)
        || tree_sitter_json
            .pointer("/metadata/license")
            .and_then(serde_json::Value::as_str)
            != Some("MIT")
    {
        return Err("tree-sitter.json language or package identity drifted".to_string());
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
    let derivations: DerivationMetadata = read_json(root, DERIVATIONS_PATH)?;
    let receipt: ArtifactReceipt = read_json(root, ARTIFACT_RECEIPT_PATH)?;
    let metrics: MechanicsMetrics = read_json(root, METRICS_PATH)?;
    let schemas: SchemaMetadata = read_json(root, SCHEMA_PATH)?;
    let upstream_lock: RepositoryLock = read_json(root, UPSTREAM_LOCK_PATH)?;
    let legal: ThirdPartyContract = read_json(root, THIRD_PARTY_COMPONENTS_PATH)?;
    let core = core_family_projection()?;
    validate_support(root, &support, &core)?;
    validate_provenance(&provenance)?;
    validate_derivations(root, &derivations, &provenance)?;
    validate_artifact_receipt(root, &receipt, &provenance)?;
    validate_query_profile_coverage(&support, &receipt)?;
    admission::validate(root, &receipt.receipt_id, &support)?;
    validate_metrics(root, &metrics, &receipt)?;
    validate_schemas(&schemas, &provenance)?;
    validate_external_sources(&provenance, &upstream_lock, &legal)?;
    validate_package_legal_bundle(root, &provenance)?;
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
                authoring_header_suggestions: family.authoring_header_suggestions,
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
        artifact_receipt_id: receipt.receipt_id,
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
        // The default verifier already executes the complete admitted fixture oracle in
        // `build_contract`. Keep this explicit release-gate spelling so callers cannot mistake a
        // metadata-only check for full conformance validation.
        [arg] if arg == "--all-fixtures" => false,
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

    fn validate_repository_support(
        support: &SupportMetadata,
        core: &[CoreFamilyProjection],
    ) -> Result<(), String> {
        validate_support(&crate::cmd::workspace_root(), support, core)
    }

    #[test]
    fn strict_header_oracle_rejects_wrong_diagram_type_ownership() {
        let case = HeaderManifestCase {
            public_id: "flowchart".to_string(),
            root: "flowchart_diagram".to_string(),
            expected_diagram_type: "flowchart-v2".to_string(),
            source: "flowchart TD\n".to_string(),
        };
        let mut result = StrictOracleCase {
            public_id: case.public_id.clone(),
            input_sha256: sha256_bytes(case.source.as_bytes()),
            expected_diagram_type: case.expected_diagram_type.clone(),
            accepted: true,
            diagram_type: case.expected_diagram_type.clone(),
        };
        assert!(strict_oracle_case_matches(&case, &result));

        result.diagram_type = "flowchart-elk".to_string();
        assert!(!strict_oracle_case_matches(&case, &result));

        let eof_case = HeaderManifestCase {
            source: "flowchart".to_string(),
            ..case
        };
        let eof_result = StrictOracleEofCase {
            public_id: eof_case.public_id.clone(),
            input_sha256: sha256_bytes(eof_case.source.as_bytes()),
            expected_diagram_type: eof_case.expected_diagram_type.clone(),
            accepted: true,
            diagram_type: Some("flowchart-elk".to_string()),
        };
        assert!(!strict_oracle_eof_case_matches(&eof_case, &eof_result));
    }

    #[test]
    fn support_metadata_matches_the_u9_all_conformant_tier() {
        let (support, core) = repository_inputs();
        let expected_conformant = core
            .iter()
            .map(|family| family.public_id.as_str())
            .collect::<BTreeSet<_>>();
        let expected_conformant_evidence = [
            "binding",
            "conformance",
            "corpus",
            "fuzz",
            "header",
            "incremental",
            "metrics",
            "node-schema",
            "query",
            "recovery",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();

        validate_repository_support(&support, &core).expect("valid support metadata");
        assert_eq!(support.families.len(), PUBLIC_FAMILY_COUNT);
        let actual_conformant = support
            .families
            .iter()
            .filter(|family| family.support_tier.as_deref() == Some("conformant"))
            .map(|family| family.public_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_conformant, expected_conformant);

        for family in &support.families {
            assert_eq!(family.lifecycle, "active");
            let evidence = family
                .evidence
                .iter()
                .map(|item| item.kind.as_str())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                evidence, expected_conformant_evidence,
                "{}",
                family.public_id
            );
            assert_eq!(
                family
                    .query_applicability
                    .get("portable")
                    .and_then(|surfaces| surfaces.get("highlights"))
                    .map(|query| query.status.as_str()),
                Some("asserted"),
                "{}",
                family.public_id
            );
        }
    }

    #[test]
    fn duplicate_missing_and_internal_variant_rows_are_rejected() {
        let (support, core) = repository_inputs();

        let mut duplicate = support.clone();
        duplicate.families[1].public_id = duplicate.families[0].public_id.clone();
        assert!(
            validate_repository_support(&duplicate, &core)
                .unwrap_err()
                .contains("duplicate")
        );

        let mut missing = support.clone();
        missing.families.pop();
        assert!(
            validate_repository_support(&missing, &core)
                .unwrap_err()
                .contains("missing=")
        );

        let mut internal = support;
        internal.families[0].public_id = "flowchart-v2".to_string();
        let error = validate_repository_support(&internal, &core).unwrap_err();
        assert!(error.contains("unexpected="));
        assert!(error.contains("flowchart-v2"));
    }

    #[test]
    fn unknown_tier_and_planned_query_claim_are_rejected() {
        let (support, core) = repository_inputs();

        let mut tier = support.clone();
        tier.families[0].lifecycle = "active".to_string();
        tier.families[0].support_tier = Some("complete-ish".to_string());
        assert!(
            validate_repository_support(&tier, &core)
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
        query.families[0].lifecycle = "planned".to_string();
        query.families[0].support_tier = None;
        query.families[0].evidence.clear();
        assert!(
            validate_repository_support(&query, &core)
                .unwrap_err()
                .contains("planned family")
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
    fn repository_lock_drift_requires_alignment_without_rewriting_support_tiers() {
        let root = crate::cmd::workspace_root();
        let (mut support, _core) = repository_inputs();
        let provenance: ProvenanceMetadata = read_json(&root, PROVENANCE_PATH).expect("provenance");
        let mut lock: RepositoryLock = read_json(&root, UPSTREAM_LOCK_PATH).expect("upstream lock");
        let legal: ThirdPartyContract =
            read_json(&root, THIRD_PARTY_COMPONENTS_PATH).expect("third-party contract");
        let tiers = support
            .families
            .iter()
            .map(|family| family.support_tier.clone())
            .collect::<Vec<_>>();
        lock.repos
            .get_mut("mermaid")
            .expect("Mermaid repository lock")
            .commit = "1".repeat(40);

        validate_external_sources(&provenance, &lock, &legal)
            .expect("selected baseline may trail the repository lock");
        assert!(
            validate_baselines(&support, &provenance, &lock)
                .unwrap_err()
                .contains("alignment must be \"drifted\"")
        );
        support.repository_alignment.mermaid = "drifted".to_string();
        validate_baselines(&support, &provenance, &lock).expect("explicit drift alignment");
        assert_eq!(
            support
                .families
                .iter()
                .map(|family| family.support_tier.clone())
                .collect::<Vec<_>>(),
            tiers
        );
    }

    #[test]
    fn external_source_revisions_and_legal_components_are_verified() {
        let root = crate::cmd::workspace_root();
        let provenance: ProvenanceMetadata = read_json(&root, PROVENANCE_PATH).expect("provenance");
        let lock: RepositoryLock = read_json(&root, UPSTREAM_LOCK_PATH).expect("upstream lock");
        let legal: ThirdPartyContract =
            read_json(&root, THIRD_PARTY_COMPONENTS_PATH).expect("third-party contract");

        let mut wrong_revision = provenance.clone();
        wrong_revision
            .sources
            .iter_mut()
            .find(|source| source.id == "pappasam-tree-sitter-mermaid")
            .expect("pappasam provenance")
            .commit = "2".repeat(40);
        assert!(
            validate_external_sources(&wrong_revision, &lock, &legal)
                .unwrap_err()
                .contains("revision differs")
        );

        let mut wrong_legal = legal;
        wrong_legal
            .components
            .iter_mut()
            .find(|component| component.id == "pappasam-tree-sitter-mermaid")
            .expect("pappasam legal component")
            .license_expression = "Apache-2.0".to_string();
        assert!(
            validate_external_sources(&provenance, &lock, &wrong_legal)
                .unwrap_err()
                .contains("differs from legal component")
        );
    }

    #[test]
    fn merman_oracle_identity_is_pinned_to_the_selected_catalog() {
        let root = crate::cmd::workspace_root();
        let mut provenance: ProvenanceMetadata =
            read_json(&root, PROVENANCE_PATH).expect("provenance");
        let oracle = provenance
            .sources
            .iter_mut()
            .find(|source| source.id == "merman-oracle")
            .expect("Merman oracle");
        oracle.r#ref = "3".repeat(40);
        oracle.commit = oracle.r#ref.clone();

        assert!(
            validate_provenance(&provenance)
                .unwrap_err()
                .contains("Merman oracle provenance identity drifted")
        );
    }

    #[test]
    fn package_legal_bundle_matches_provenance_and_repository_licenses() {
        let root = crate::cmd::workspace_root();
        let provenance: ProvenanceMetadata = read_json(&root, PROVENANCE_PATH).expect("provenance");
        let temporary = tempfile::tempdir().expect("temporary repository");
        let mut notice = String::new();
        for (source_id, repository_path, package_path) in PACKAGE_LICENSE_COPIES {
            let source = provenance
                .sources
                .iter()
                .find(|source| source.id == source_id)
                .expect("source identity");
            writeln!(
                notice,
                "{} {} {package_path}",
                source.repository, source.commit
            )
            .expect("notice buffer");
            for path in [
                temporary.path().join(repository_path),
                temporary.path().join(PACKAGE_ROOT).join(package_path),
            ] {
                fs::create_dir_all(path.parent().expect("license parent"))
                    .expect("license directory");
                fs::write(path, format!("{source_id} license\n")).expect("license copy");
            }
        }
        let notice_path = temporary
            .path()
            .join(PACKAGE_ROOT)
            .join("THIRD_PARTY_NOTICES.md");
        fs::write(&notice_path, notice).expect("package notice");

        validate_package_legal_bundle(temporary.path(), &provenance).expect("valid legal bundle");
        let (_, _, package_path) = PACKAGE_LICENSE_COPIES[0];
        fs::write(
            temporary.path().join(PACKAGE_ROOT).join(package_path),
            "drifted license\n",
        )
        .expect("drifted package license");
        assert!(
            validate_package_legal_bundle(temporary.path(), &provenance)
                .unwrap_err()
                .contains("differs from legal authority")
        );
    }

    #[test]
    fn support_evidence_must_resolve_match_bytes_and_back_query_claims() {
        let temporary = tempfile::tempdir().expect("temporary repository");
        let evidence_dir = temporary.path().join(PACKAGE_ROOT).join("test/corpus");
        fs::create_dir_all(&evidence_dir).expect("evidence directory");
        let evidence_path = evidence_dir.join("header.txt");
        fs::write(&evidence_path, "flowchart TD\n").expect("evidence fixture");

        let (support, _core) = repository_inputs();
        let mut family = support.families[0].clone();
        family.lifecycle = "planned".to_string();
        family.support_tier = None;
        family.evidence.clear();
        family.evidence.push(FamilyEvidence {
            id: "corpus-proof".to_string(),
            kind: "corpus".to_string(),
            path: "test/corpus/header.txt".to_string(),
            sha256: sha256_file(&evidence_path).expect("fixture digest"),
        });
        let error = validate_family_support(temporary.path(), &family).unwrap_err();
        assert!(error.contains("planned family"));

        family.lifecycle = "active".to_string();
        family.support_tier = Some("recognized".to_string());
        let error = validate_family_support(temporary.path(), &family).unwrap_err();
        assert!(error.contains("lacks header evidence"));

        validate_evidence(temporary.path(), &family).expect("well-formed corpus evidence");

        family.evidence[0].sha256 = "0".repeat(64);
        assert!(
            validate_evidence(temporary.path(), &family)
                .unwrap_err()
                .contains("digest drifted")
        );
        family.evidence[0].sha256 = sha256_file(&evidence_path).expect("fixture digest");
        family.evidence[0].kind = "query".to_string();
        assert!(
            validate_evidence(temporary.path(), &family)
                .unwrap_err()
                .contains("runner-owned path")
        );
        family.evidence[0].kind = "corpus".to_string();
        family.query_applicability.insert(
            "portable".to_string(),
            BTreeMap::from([(
                "highlights".to_string(),
                QueryApplicability {
                    status: "asserted".to_string(),
                    evidence: vec!["corpus-proof".to_string()],
                    rationale: None,
                },
            )]),
        );
        assert!(
            validate_query_applicability(
                &family,
                false,
                &BTreeMap::from([("corpus-proof".to_string(), "corpus".to_string())]),
            )
            .unwrap_err()
            .contains("unverified query evidence")
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
    fn derivation_coverage_and_source_relationships_are_enforced() {
        let root = crate::cmd::workspace_root();
        let provenance: ProvenanceMetadata = read_json(&root, PROVENANCE_PATH).expect("provenance");
        let mut derivations: DerivationMetadata =
            read_json(&root, DERIVATIONS_PATH).expect("derivations");

        derivations.derivations[0].local_paths.remove(0);
        assert!(
            validate_derivations(&root, &derivations, &provenance)
                .unwrap_err()
                .contains("differ from attributed package files")
        );

        let mut derivations: DerivationMetadata =
            read_json(&root, DERIVATIONS_PATH).expect("derivations");
        derivations.derivations[0].sources[0].source_id = "unknown-source".to_string();
        assert!(
            validate_derivations(&root, &derivations, &provenance)
                .unwrap_err()
                .contains("incomplete or unknown")
        );
    }

    #[test]
    fn artifact_receipt_binds_real_files_and_compiled_binding() {
        let root = crate::cmd::workspace_root();
        let provenance: ProvenanceMetadata = read_json(&root, PROVENANCE_PATH).expect("provenance");
        let mut receipt: ArtifactReceipt =
            read_json(&root, ARTIFACT_RECEIPT_PATH).expect("artifact receipt");

        receipt.artifacts[0].sha256 = "0".repeat(64);
        assert!(
            validate_artifact_receipt(&root, &receipt, &provenance)
                .unwrap_err()
                .contains("differs from package bytes")
        );

        let mut receipt: ArtifactReceipt =
            read_json(&root, ARTIFACT_RECEIPT_PATH).expect("artifact receipt");
        receipt.receipt_id = "0".repeat(64);
        assert!(
            validate_artifact_receipt(&root, &receipt, &provenance)
                .unwrap_err()
                .contains("receipt digest")
        );

        let mut receipt: ArtifactReceipt =
            read_json(&root, ARTIFACT_RECEIPT_PATH).expect("artifact receipt");
        receipt.language.abi += 1;
        assert!(
            validate_artifact_receipt(&root, &receipt, &provenance)
                .unwrap_err()
                .contains("language identity drifted")
        );

        let mut receipt: ArtifactReceipt =
            read_json(&root, ARTIFACT_RECEIPT_PATH).expect("artifact receipt");
        receipt.query_profiles[0].sha256 = "0".repeat(64);
        assert!(
            validate_artifact_receipt(&root, &receipt, &provenance)
                .unwrap_err()
                .contains("query profile differs")
        );

        let mut receipt: ArtifactReceipt =
            read_json(&root, ARTIFACT_RECEIPT_PATH).expect("artifact receipt");
        let duplicate = receipt.query_profiles[0].clone();
        receipt.query_profiles.push(duplicate);
        assert!(
            validate_artifact_receipt(&root, &receipt, &provenance)
                .unwrap_err()
                .contains("duplicates a query profile")
        );

        let mut receipt: ArtifactReceipt =
            read_json(&root, ARTIFACT_RECEIPT_PATH).expect("artifact receipt");
        receipt.query_profiles.pop();
        assert!(
            validate_artifact_receipt(&root, &receipt, &provenance)
                .unwrap_err()
                .contains("query profile set drifted")
        );
    }

    #[test]
    fn mechanics_metrics_match_generated_macros_and_hard_budgets() {
        let root = crate::cmd::workspace_root();
        let receipt: ArtifactReceipt =
            read_json(&root, ARTIFACT_RECEIPT_PATH).expect("artifact receipt");
        let mut metrics: MechanicsMetrics = read_json(&root, METRICS_PATH).expect("metrics");

        validate_metrics(&root, &metrics, &receipt).expect("current metrics");
        metrics.ratchet.parser_states_investigate_above = 1;
        assert!(
            validate_metrics(&root, &metrics, &receipt)
                .unwrap_err()
                .contains("ratchet limits")
        );

        let mut metrics: MechanicsMetrics = read_json(&root, METRICS_PATH).expect("metrics");
        metrics.observed.native_node_smoke_parse_milliseconds = 0.0;
        assert!(
            validate_metrics(&root, &metrics, &receipt)
                .unwrap_err()
                .contains("smoke performance")
        );

        let mut metrics: MechanicsMetrics = read_json(&root, METRICS_PATH).expect("metrics");
        metrics.observed.synthetic_doubling["lanes"][0]["nativeRuntime"]["observedParseMilliseconds"] =
            serde_json::json!(0.0);
        assert!(
            validate_metrics(&root, &metrics, &receipt)
                .unwrap_err()
                .contains("runtime lacks valid observedParseMilliseconds")
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
        assert_eq!(contract.artifact_receipt_id.len(), 64);
    }
}
