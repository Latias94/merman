use sha2::{Digest, Sha256};
use tree_sitter::{Language, Parser};

use tree_sitter_mermaid::{ARTIFACT_RECEIPT, LANGUAGE};

const FAMILY_FIXTURES: &str = include_str!("../metadata/fixtures/family-roots.json");
const HEADER_MANIFEST: &str = include_str!("../metadata/headers.json");
const HEADER_RECEIPT: &str = include_str!("../metadata/evidence/u2-header-dispatch.json");
const STRICT_HEADER_ORACLE: &str =
    include_str!("../metadata/evidence/u2-mermaid-header-oracle.json");

fn parser() -> Parser {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn parse_result(source: &str) -> (bool, Vec<String>) {
    let mut parser = parser();
    let tree = parser
        .parse(source, None)
        .expect("parser must return a tree");
    let root = tree.root_node();
    let roots = root
        .named_children(&mut root.walk())
        .filter(|node| node.kind().ends_with("_diagram"))
        .map(|node| node.kind().to_owned())
        .collect();
    (root.has_error(), roots)
}

fn positive_cases<'a>(
    fixtures: &'a [serde_json::Value],
    headers: &'a serde_json::Value,
) -> impl Iterator<Item = (&'static str, &'a serde_json::Value)> {
    fixtures.iter().map(|case| ("baseline", case)).chain(
        headers["cases"]
            .as_array()
            .expect("header cases must be an array")
            .iter()
            .map(|case| ("header", case)),
    )
}

fn eof_candidates(headers: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut ownership = std::collections::BTreeMap::<(String, String), (String, String)>::new();
    let mut candidates = Vec::new();
    for case in headers["cases"]
        .as_array()
        .expect("header cases must be an array")
    {
        let public_id = case["publicId"].as_str().expect("publicId string");
        let root = case["root"].as_str().expect("root string");
        let expected_diagram_type = case["expectedDiagramType"]
            .as_str()
            .expect("expectedDiagramType string");
        let source = case["source"]
            .as_str()
            .expect("source string")
            .split(['\r', '\n'])
            .next()
            .unwrap_or_default();
        let key = (public_id.to_string(), source.to_string());
        if let Some((existing_root, existing_diagram_type)) = ownership.get(&key) {
            assert_eq!(existing_root, root, "EOF candidate has conflicting roots");
            assert_eq!(
                existing_diagram_type, expected_diagram_type,
                "EOF candidate has conflicting strict diagram ownership"
            );
            continue;
        }
        ownership.insert(key, (root.to_string(), expected_diagram_type.to_string()));
        candidates.push(serde_json::json!({
            "publicId": public_id,
            "root": root,
            "expectedDiagramType": expected_diagram_type,
            "source": source,
        }));
    }
    candidates
}

fn strict_positive_result_matches(case: &serde_json::Value, result: &serde_json::Value) -> bool {
    let Some(expected_diagram_type) = case["expectedDiagramType"]
        .as_str()
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    result["publicId"] == case["publicId"]
        && result["inputSha256"] == sha256(case["source"].as_str().unwrap_or_default().as_bytes())
        && result["expectedDiagramType"] == expected_diagram_type
        && result["accepted"] == true
        && result["diagramType"] == expected_diagram_type
}

fn strict_eof_result_matches(case: &serde_json::Value, result: &serde_json::Value) -> bool {
    let Some(expected_diagram_type) = case["expectedDiagramType"]
        .as_str()
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    let Some(accepted) = result["accepted"].as_bool() else {
        return false;
    };
    result["publicId"] == case["publicId"]
        && result["inputSha256"] == sha256(case["source"].as_str().unwrap_or_default().as_bytes())
        && result["expectedDiagramType"] == expected_diagram_type
        && if accepted {
            result["diagramType"] == expected_diagram_type
        } else {
            result["diagramType"].is_null()
        }
}

#[test]
fn every_public_family_baseline_selects_its_root_without_errors() {
    let cases: serde_json::Value =
        serde_json::from_str(FAMILY_FIXTURES).expect("family fixture JSON must be valid");
    let cases = cases
        .as_array()
        .expect("family fixture JSON must be an array");
    assert_eq!(cases.len(), 35);

    for case in cases {
        let public_id = case["publicId"].as_str().expect("publicId string");
        let source = case["source"].as_str().expect("source string");
        let expected_root = case["root"].as_str().expect("root string");
        let (has_error, roots) = parse_result(source);
        assert!(
            !has_error,
            "{public_id} baseline unexpectedly contains a parse error"
        );
        assert_eq!(
            roots,
            [expected_root],
            "{public_id} must select exactly one expected public family root"
        );
    }
}

#[test]
fn versioned_header_manifest_drives_every_strict_alias_and_header_negative() {
    let manifest: serde_json::Value =
        serde_json::from_str(HEADER_MANIFEST).expect("header manifest must be valid JSON");
    assert_eq!(manifest["schemaVersion"], 3);
    assert_eq!(manifest["authorities"]["mermaid"]["version"], "11.16.1");
    assert_eq!(
        manifest["authorities"]["mermaid"]["commit"],
        "7ecca0cd7f1658ef74f4e7e91f925724ef403bbf"
    );
    assert_eq!(manifest["authorities"]["zenuml"]["version"], "3.50.1");
    assert_eq!(
        manifest["authorities"]["zenuml"]["commit"],
        "38404ccc14243ed54ab45b804b2eb6f2ca73af36"
    );

    let cases = manifest["cases"]
        .as_array()
        .expect("header cases must be an array");
    assert!(cases.len() > 35);
    for case in cases {
        let public_id = case["publicId"].as_str().expect("publicId string");
        let source = case["source"].as_str().expect("source string");
        let expected_root = case["root"].as_str().expect("root string");
        assert!(
            case["expectedDiagramType"]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "{source:?} must declare exact strict Mermaid diagram ownership"
        );
        let (has_error, roots) = parse_result(source);
        assert!(!has_error, "{source:?} unexpectedly contains a parse error");
        assert_eq!(
            roots,
            [expected_root],
            "wrong root for {public_id}: {source:?}"
        );
    }

    let strict_negatives = manifest["strictHeaderNegatives"]
        .as_array()
        .expect("strict header negatives must be an array");
    for expected in [
        "flowchartTD\n",
        "infoshowInfo\n",
        "pieshowData\n",
        "pietitle Foo\n",
        "gitGraphLR:\n",
        "swimlane-betaTD\n",
        "timelineLR\n",
        "xycharthorizontal\n",
    ] {
        assert!(
            strict_negatives.iter().any(|source| source == expected),
            "concatenated header regression {expected:?} must remain strict-negative"
        );
    }
    for source in strict_negatives {
        let source = source.as_str().expect("negative source string");
        let (has_error, roots) = parse_result(source);
        assert!(
            roots.is_empty() || has_error,
            "{source:?} was incorrectly admitted as an error-free strict diagram header"
        );
    }
}

#[test]
fn committed_header_receipt_replays_exact_parser_results() {
    let fixtures: serde_json::Value =
        serde_json::from_str(FAMILY_FIXTURES).expect("family fixture JSON must be valid");
    let fixtures = fixtures.as_array().expect("fixtures must be an array");
    let headers: serde_json::Value =
        serde_json::from_str(HEADER_MANIFEST).expect("header manifest must be valid");
    let receipt: serde_json::Value =
        serde_json::from_str(HEADER_RECEIPT).expect("header receipt must be valid");
    let artifact: serde_json::Value =
        serde_json::from_str(ARTIFACT_RECEIPT).expect("artifact receipt must be valid");

    assert_eq!(receipt["schemaVersion"], 5);
    assert_eq!(
        receipt["producer"]["id"],
        "tree-sitter-mermaid/header-dispatch"
    );
    assert_eq!(receipt["producer"]["version"], 5);
    assert_eq!(
        receipt["artifactReceiptId"], artifact["receiptId"],
        "header evidence must bind the loaded grammar artifact"
    );
    let oracle: serde_json::Value =
        serde_json::from_str(STRICT_HEADER_ORACLE).expect("strict oracle receipt must be valid");
    assert_eq!(
        receipt["strictOracleReceipt"]["path"],
        "metadata/evidence/u2-mermaid-header-oracle.json"
    );
    assert_eq!(
        receipt["strictOracleReceipt"]["sha256"],
        sha256(STRICT_HEADER_ORACLE.as_bytes())
    );
    assert_eq!(
        receipt["strictOracleReceipt"]["receiptId"],
        oracle["receiptId"]
    );
    assert_eq!(oracle["schemaVersion"], 3);
    assert_eq!(
        oracle["producer"]["id"],
        "tree-sitter-mermaid/mermaid-strict-header-oracle"
    );
    assert_eq!(
        oracle["headerManifest"]["sha256"],
        sha256(HEADER_MANIFEST.as_bytes())
    );
    let oracle_cases = oracle["cases"]
        .as_array()
        .expect("strict oracle cases must be an array");
    let header_cases = headers["cases"]
        .as_array()
        .expect("header cases must be an array");
    assert_eq!(oracle_cases.len(), header_cases.len());
    for (case, result) in header_cases.iter().zip(oracle_cases) {
        assert!(
            strict_positive_result_matches(case, result),
            "strict oracle must retain exact publicId/diagramType ownership"
        );
    }
    let eof_candidates = eof_candidates(&headers);
    let oracle_eof_cases = oracle["eofCases"]
        .as_array()
        .expect("strict oracle EOF cases must be an array");
    assert_eq!(oracle["eofCandidateCount"], eof_candidates.len());
    assert_eq!(oracle_eof_cases.len(), eof_candidates.len());
    let mut accepted_eof_cases = Vec::new();
    let mut rejected_eof_cases = Vec::new();
    for (case, result) in eof_candidates.iter().zip(oracle_eof_cases) {
        assert!(
            strict_eof_result_matches(case, result),
            "strict EOF oracle must retain exact publicId/diagramType ownership"
        );
        let accepted = result["accepted"].as_bool().expect("accepted boolean");
        let source = case["source"].as_str().expect("EOF source string");
        let expected_root = case["root"].as_str().expect("EOF root string");
        let (has_error, roots) = parse_result(source);
        if accepted {
            assert!(!has_error, "accepted EOF header {source:?} has an error");
            assert_eq!(roots, [expected_root], "accepted EOF header has wrong root");
            accepted_eof_cases.push(case.clone());
        } else {
            assert!(
                roots.is_empty() || has_error,
                "strict-rejected EOF header {source:?} was admitted as {roots:?}"
            );
            rejected_eof_cases.push((case.clone(), has_error, roots));
        }
    }
    assert_eq!(
        receipt["headerManifest"]["sha256"],
        sha256(HEADER_MANIFEST.as_bytes())
    );
    assert_eq!(
        receipt["fixtureManifest"]["sha256"],
        sha256(FAMILY_FIXTURES.as_bytes())
    );

    let recorded = receipt["cases"]
        .as_array()
        .expect("receipt cases must be an array");
    let mut expected = positive_cases(fixtures, &headers)
        .map(|(kind, case)| (kind, case.clone()))
        .collect::<Vec<_>>();
    expected.extend(
        accepted_eof_cases
            .into_iter()
            .map(|case| ("header-eof", case)),
    );
    assert_eq!(recorded.len(), expected.len());
    for ((kind, case), result) in expected.into_iter().zip(recorded) {
        let public_id = case["publicId"].as_str().expect("publicId string");
        let source = case["source"].as_str().expect("source string");
        let expected_root = case["root"].as_str().expect("root string");
        let (has_error, actual_roots) = parse_result(source);

        assert_eq!(result["kind"], kind);
        assert_eq!(result["publicId"], public_id);
        assert_eq!(result["inputSha256"], sha256(source.as_bytes()));
        assert_eq!(result["expectedRoot"], expected_root);
        assert_eq!(
            result["expectedDiagramType"],
            case.get("expectedDiagramType")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        );
        assert_eq!(result["actualRoot"], actual_roots[0]);
        assert_eq!(result["hasError"], has_error);
        assert!(!has_error);
        assert_eq!(actual_roots, [expected_root]);
    }

    let negative_sources = headers["strictHeaderNegatives"]
        .as_array()
        .expect("negative cases must be an array");
    let negative_results = receipt["negativeCases"]
        .as_array()
        .expect("negative receipt cases must be an array");
    let oracle_negative_results = oracle["negativeCases"]
        .as_array()
        .expect("strict oracle negative cases must be an array");
    assert_eq!(negative_results.len(), negative_sources.len());
    assert_eq!(oracle_negative_results.len(), negative_sources.len());
    for ((source, result), oracle_result) in negative_sources
        .iter()
        .zip(negative_results)
        .zip(oracle_negative_results)
    {
        let source = source.as_str().expect("negative source string");
        let (has_error, roots) = parse_result(source);
        assert_eq!(oracle_result["inputSha256"], sha256(source.as_bytes()));
        assert_eq!(oracle_result["accepted"], false);
        assert_eq!(oracle_result["diagramType"], serde_json::Value::Null);
        assert_eq!(result["inputSha256"], sha256(source.as_bytes()));
        assert_eq!(result["hasError"], has_error);
        assert_eq!(
            result["actualRoots"],
            serde_json::to_value(&roots).expect("roots serialize")
        );
        assert!(roots.is_empty() || has_error);
    }

    let eof_negative_results = receipt["eofNegativeCases"]
        .as_array()
        .expect("EOF-negative receipt cases must be an array");
    assert_eq!(eof_negative_results.len(), rejected_eof_cases.len());
    for ((case, has_error, roots), result) in
        rejected_eof_cases.into_iter().zip(eof_negative_results)
    {
        let public_id = case["publicId"].as_str().expect("publicId string");
        let source = case["source"].as_str().expect("EOF source string");
        assert_eq!(result["publicId"], public_id);
        assert_eq!(result["inputSha256"], sha256(source.as_bytes()));
        let recorded_roots = result["actualRoots"].as_array().expect("actualRoots array");
        assert!(
            result["hasError"] == true || recorded_roots.is_empty(),
            "Node receipt admitted strict-rejected EOF header {source:?}"
        );
        assert!(
            has_error || roots.is_empty(),
            "Rust runtime admitted strict-rejected EOF header {source:?}"
        );
    }
}

#[test]
fn strict_diagram_type_misattribution_is_rejected_by_the_test_contract() {
    let case = serde_json::json!({
        "publicId": "flowchart",
        "root": "flowchart_diagram",
        "expectedDiagramType": "flowchart-v2",
        "source": "flowchart TD\n",
    });
    let mut result = serde_json::json!({
        "publicId": "flowchart",
        "inputSha256": sha256(b"flowchart TD\n"),
        "expectedDiagramType": "flowchart-v2",
        "accepted": true,
        "diagramType": "flowchart-v2",
    });
    assert!(strict_positive_result_matches(&case, &result));

    result["diagramType"] = serde_json::json!("flowchart-elk");
    assert!(!strict_positive_result_matches(&case, &result));

    let eof_case = serde_json::json!({
        "publicId": "flowchart",
        "root": "flowchart_diagram",
        "expectedDiagramType": "flowchart-v2",
        "source": "flowchart",
    });
    result["inputSha256"] = serde_json::json!(sha256(b"flowchart"));
    assert!(!strict_eof_result_matches(&eof_case, &result));
}
