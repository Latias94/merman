use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;
use tree_sitter::{Language, Node, Parser};
use tree_sitter_mermaid::{ARTIFACT_RECEIPT, LANGUAGE, NODE_TYPES};

const PUBLIC_FAMILY_COUNT: usize = 35;
const REQUIRED_EVIDENCE_KINDS: [&str; 10] = [
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupportMetadata {
    schema_version: u32,
    families: Vec<SupportFamily>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupportFamily {
    public_id: String,
    root_node: String,
    lifecycle: String,
    support_tier: String,
    evidence: Vec<SupportEvidence>,
}

#[derive(Debug, Deserialize)]
struct SupportEvidence {
    kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdmissionManifest {
    schema_version: u32,
    artifact_receipt_id: String,
    families: Vec<AdmissionFamily>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdmissionFamily {
    public_id: String,
    root_node: String,
    required_named_nodes: Vec<String>,
    fixtures: Vec<AdmissionFixture>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdmissionFixture {
    role: String,
    source_sha256: String,
    render_context_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryApplicability {
    schema_version: u32,
    profile: String,
    families: Vec<QueryFamily>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryFamily {
    public_id: String,
    source: String,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn insert_unique<'a, T>(
    values: &'a [T],
    key: impl Fn(&'a T) -> &'a str,
    label: &str,
) -> BTreeMap<&'a str, &'a T> {
    let mut result = BTreeMap::new();
    for value in values {
        let key = key(value);
        assert!(
            result.insert(key, value).is_none(),
            "duplicate {label} row {key:?}"
        );
    }
    result
}

fn count_kind(node: Node<'_>, kind: &str) -> usize {
    let mut count = usize::from(node.kind() == kind);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_kind(child, kind);
    }
    count
}

fn assert_no_unexpected_recovery(node: Node<'_>, public_id: &str) {
    const FORBIDDEN_EXACT: [&str; 5] = [
        "catch_all_body",
        "raw_line",
        "unknown_statement",
        "unstructured_body",
        "unstructured_statement",
    ];
    const FORBIDDEN_FRAGMENTS: [&str; 5] =
        ["incomplete", "invalid", "malformed", "recovery", "unclosed"];

    let kind = node.kind();
    assert!(
        !node.is_error(),
        "{public_id} produced ERROR: {}",
        node.to_sexp()
    );
    assert!(
        !node.is_missing(),
        "{public_id} produced a missing node: {}",
        node.to_sexp()
    );
    assert!(
        !FORBIDDEN_EXACT.contains(&kind)
            && !FORBIDDEN_FRAGMENTS
                .iter()
                .any(|fragment| kind.contains(fragment)),
        "{public_id} representative source entered recovery node {kind}: {}",
        node.to_sexp()
    );

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        assert_no_unexpected_recovery(child, public_id);
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[test]
fn every_public_family_has_a_clean_package_owned_conformance_source() {
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let support: SupportMetadata = read_json(&package_root.join("metadata/support.json"));
    let admission: AdmissionManifest =
        read_json(&package_root.join("test/conformance/admission.json"));
    let queries: QueryApplicability =
        read_json(&package_root.join("test/queries/portable/applicability.json"));
    let artifact_receipt: serde_json::Value =
        serde_json::from_str(ARTIFACT_RECEIPT).expect("artifact receipt must be valid JSON");

    assert_eq!(support.schema_version, 1);
    assert_eq!(admission.schema_version, 1);
    assert_eq!(queries.schema_version, 1);
    assert_eq!(queries.profile, "portable");
    assert_eq!(
        artifact_receipt["receiptId"], admission.artifact_receipt_id,
        "conformance admission must bind the generated artifact receipt"
    );

    let support_by_id = insert_unique(&support.families, |family| &family.public_id, "support");
    let admission_by_id =
        insert_unique(&admission.families, |family| &family.public_id, "admission");
    let query_by_id = insert_unique(&queries.families, |family| &family.public_id, "query");
    assert_eq!(support_by_id.len(), PUBLIC_FAMILY_COUNT);
    assert_eq!(
        support_by_id.keys().collect::<BTreeSet<_>>(),
        admission_by_id.keys().collect()
    );
    assert_eq!(
        support_by_id.keys().collect::<BTreeSet<_>>(),
        query_by_id.keys().collect()
    );

    let node_types: Vec<serde_json::Value> =
        serde_json::from_str(NODE_TYPES).expect("generated node types must be valid JSON");
    let named_node_types = node_types
        .iter()
        .filter(|node| node["named"] == true)
        .filter_map(|node| node["type"].as_str())
        .collect::<BTreeSet<_>>();
    let required_evidence = REQUIRED_EVIDENCE_KINDS.into_iter().collect::<BTreeSet<_>>();

    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");

    for (public_id, support_family) in support_by_id {
        let admission_family = admission_by_id[public_id];
        let query_family = query_by_id[public_id];

        assert_eq!(support_family.lifecycle, "active", "{public_id}");
        assert_eq!(support_family.support_tier, "conformant", "{public_id}");
        assert_eq!(
            support_family.root_node, admission_family.root_node,
            "{public_id}"
        );

        let evidence_kinds = support_family
            .evidence
            .iter()
            .map(|evidence| evidence.kind.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(evidence_kinds, required_evidence, "{public_id}");

        assert!(
            admission_family
                .required_named_nodes
                .iter()
                .any(|kind| kind == &admission_family.root_node),
            "{public_id} required-node contract must include its root"
        );
        for kind in &admission_family.required_named_nodes {
            assert!(
                named_node_types.contains(kind.as_str()),
                "{public_id} requires unknown named node {kind}"
            );
        }

        let fixture_roles = admission_family
            .fixtures
            .iter()
            .map(|fixture| fixture.role.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            fixture_roles,
            ["admitted-valid", "family-baseline"].into_iter().collect(),
            "{public_id}"
        );
        for fixture in &admission_family.fixtures {
            assert!(valid_sha256(&fixture.source_sha256), "{public_id}");
            assert!(valid_sha256(&fixture.render_context_sha256), "{public_id}");
        }

        let source_path = package_root.join(&query_family.source);
        let source = fs::read(&source_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", source_path.display()));
        let tree = parser
            .parse(&source, None)
            .unwrap_or_else(|| panic!("{public_id} parse was cancelled"));
        let root = tree.root_node();
        assert!(
            !root.has_error(),
            "{public_id} representative source has errors: {}",
            root.to_sexp()
        );
        assert_eq!(
            count_kind(root, &support_family.root_node),
            1,
            "{public_id} representative source selected the wrong family: {}",
            root.to_sexp()
        );
        assert_no_unexpected_recovery(root, public_id);
    }
}
