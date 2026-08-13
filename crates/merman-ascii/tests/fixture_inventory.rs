use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use merman_ascii::{AsciiRenderOptions, render_model};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::sequence::SequenceMessageKind;
use merman_core::{Engine, ParseOptions};
use sha2::{Digest, Sha256};

const EXPECTED_FIXTURE_COUNTS: &[(&str, usize)] = &[
    ("ascii", 54),
    ("extended-chars", 25),
    ("sequence", 12),
    ("sequence-ascii", 5),
];

const IMPORTED_FAMILY_FIXTURE_COUNTS: &[(&str, &str, usize)] = &[
    ("sequence", "sequence", 322),
    ("class", "class", 251),
    ("er", "er", 101),
];

const IMPORTED_INTENTIONAL_EMPTY_FIXTURES: &[(&str, &str)] = &[
    (
        "class/upstream_pkgtests_classdiagram_spec_004.mmd",
        "accessibility-only Class input has no terminal-visible diagram facts",
    ),
    (
        "class/upstream_pkgtests_classdiagram_spec_005.mmd",
        "multiline accessibility-only Class input has no terminal-visible diagram facts",
    ),
    (
        "er/upstream_pkgtests_diagram_orchestration_spec_030.mmd",
        "an empty ER diagram has no terminal-visible diagram facts",
    ),
];

#[test]
fn fixture_inventory_matches_tracked_upstream_snapshot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata/mermaid-ascii");

    for (directory, expected_count) in EXPECTED_FIXTURE_COUNTS {
        let dir = root.join(directory);
        let mut files = fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
            .map(|entry| entry.expect("fixture entry must be readable").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
            .collect::<Vec<_>>();
        files.sort();

        assert_eq!(
            files.len(),
            *expected_count,
            "unexpected fixture count in {}",
            dir.display()
        );

        for path in files {
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            assert!(
                content.contains("\n---\n") || content.contains("\r\n---\r\n"),
                "fixture must keep upstream input/output separator: {}",
                path.display()
            );
        }
    }
}

#[test]
fn fixture_inventory_records_source_provenance() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(manifest_dir.join("tests/testdata/mermaid-ascii/README.md"))
        .expect("fixture README must be readable");
    let provenance =
        fs::read_to_string(manifest_dir.join("tests/testdata/mermaid-ascii/SOURCE_PROVENANCE.tsv"))
            .expect("fixture provenance manifest must be readable");
    let license = fs::read_to_string(manifest_dir.join("LICENSES/mermaid-ascii-MIT.txt"))
        .expect("upstream MIT license copy must be readable");

    assert!(readme.contains("https://github.com/AlexanderGrooff/mermaid-ascii"));
    assert!(readme.contains("6fffb8e"));
    assert!(readme.contains("SOURCE_PROVENANCE.tsv"));
    assert!(readme.contains("MIT"));
    assert!(provenance.contains("d5430290e873b327ca1af07f753e28a25db76cc7"));
    assert!(license.contains("MIT License"));
    assert!(license.contains("Copyright (c) 2023 Alexander Grooff"));
}

#[test]
fn fixture_source_provenance_pins_bytes_and_historical_transforms() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.join("tests/testdata/mermaid-ascii");
    let provenance_path = root.join("SOURCE_PROVENANCE.tsv");
    let provenance = fs::read_to_string(&provenance_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", provenance_path.display()));

    let mut metadata = BTreeMap::new();
    let mut supplements = BTreeMap::new();
    let mut transforms = BTreeMap::new();
    for (line_index, line) in provenance.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        match fields.as_slice() {
            ["meta", key, value] => {
                assert!(
                    metadata.insert(*key, *value).is_none(),
                    "duplicate provenance metadata key `{key}`"
                );
            }
            ["meta", key, value, extra] => {
                assert_eq!(*key, "baseline_tree");
                assert!(
                    metadata.insert(*key, *extra).is_none(),
                    "duplicate provenance metadata key `{key}`"
                );
                assert_eq!(*value, "cmd/testdata");
            }
            ["supplement", relative, source_blob] => {
                assert!(is_lower_hex(source_blob, 40));
                assert!(
                    supplements.insert(*relative, *source_blob).is_none(),
                    "duplicate supplemental provenance for `{relative}`"
                );
            }
            [
                "transform",
                relative,
                source_commit,
                source_blob,
                source_sha256,
                tracked_sha256,
                transform,
            ] => {
                assert!(is_lower_hex(source_commit, 40));
                assert!(is_lower_hex(source_blob, 40));
                assert!(is_lower_hex(source_sha256, 64));
                assert!(is_lower_hex(tracked_sha256, 64));
                assert_ne!(source_sha256, tracked_sha256);
                assert!(
                    transform.starts_with("historical_output_refresh_")
                        || transform.starts_with("reference_option_preamble_moved_"),
                    "unknown historical fixture transform `{transform}`"
                );
                assert!(
                    transforms
                        .insert(*relative, (*tracked_sha256, *transform))
                        .is_none(),
                    "duplicate historical transform for `{relative}`"
                );
            }
            _ => panic!(
                "invalid provenance record at {}:{}",
                provenance_path.display(),
                line_index + 1
            ),
        }
    }

    assert_eq!(
        metadata.get("baseline_commit"),
        Some(&"6fffb8e2714acab2c4cb41c78894fabbc62cee56")
    );
    assert_eq!(
        metadata.get("baseline_tree"),
        Some(&"d5430290e873b327ca1af07f753e28a25db76cc7")
    );
    assert_eq!(
        metadata.get("supplement_commit"),
        Some(&"876b5b44fcebb746e7aee09d3d19d0c059452621")
    );
    assert_eq!(
        supplements,
        BTreeMap::from([
            (
                "ascii/tight_arrow.txt",
                "3d1b05b706897c1d9ee8674abdf0984755852b06",
            ),
            (
                "ascii/tight_arrow_mixed.txt",
                "2329cd3a4249a0be4a8e0c2198dae1a08df8c802",
            ),
            (
                "extended-chars/tight_arrow.txt",
                "71b37b2e4fa8e01d8162def0bbda08571380b03d",
            ),
            (
                "extended-chars/tight_arrow_mixed.txt",
                "18834e2fcbf4100659d9dd071c0bfe80e5eb66ff",
            ),
        ]),
        "the supplemental source identities must stay pinned"
    );
    assert_eq!(transforms.len(), 8);

    let mut fixture_paths = EXPECTED_FIXTURE_COUNTS
        .iter()
        .flat_map(|(directory, _)| fixture_cases_in(&root, directory))
        .collect::<Vec<_>>();
    fixture_paths.sort();
    assert_eq!(
        fixture_paths.len(),
        parse_metadata_usize(&metadata, "tracked_fixture_count")
    );
    assert_eq!(
        fixture_paths.len(),
        parse_metadata_usize(&metadata, "byte_identical_source_count")
            + parse_metadata_usize(&metadata, "historical_transform_count")
    );
    assert_eq!(
        transforms.len(),
        parse_metadata_usize(&metadata, "historical_transform_count")
    );

    let mut aggregate = Sha256::new();
    let mut transformed_paths = BTreeSet::new();
    for fixture_path in fixture_paths {
        let relative = fixture_path
            .strip_prefix(&root)
            .unwrap_or_else(|error| {
                panic!(
                    "failed to relativize {} against {}: {error}",
                    fixture_path.display(),
                    root.display()
                )
            })
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&fixture_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", fixture_path.display()));
        aggregate.update((relative.len() as u64).to_le_bytes());
        aggregate.update(relative.as_bytes());
        aggregate.update((bytes.len() as u64).to_le_bytes());
        aggregate.update(&bytes);

        if let Some((expected_sha256, _)) = transforms.get(relative.as_str()) {
            let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
            assert_eq!(
                &actual_sha256, expected_sha256,
                "historically transformed fixture drifted: {relative}"
            );
            transformed_paths.insert(relative);
        }
    }

    assert_eq!(
        transformed_paths,
        transforms.keys().map(|path| (*path).to_owned()).collect(),
        "every historical transform must name one tracked fixture"
    );
    assert_eq!(
        format!("{:x}", aggregate.finalize()),
        *metadata
            .get("tracked_aggregate_sha256")
            .expect("tracked aggregate digest must be recorded"),
        "the immutable tracked fixture bytes drifted"
    );
}

#[test]
fn fixture_inventory_documents_v1_coverage_contract() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contract = fs::read_to_string(manifest_dir.join("V1_MERMAID_ASCII_COVERAGE.md"))
        .expect("v1 coverage contract must be readable");

    for expected in [
        "6fffb8e2714acab2c4cb41c78894fabbc62cee56",
        "27 / 54 exact output matches; 27 named deterministic differences",
        "13 / 25 exact output matches; 12 named deterministic differences",
        "12 / 12 normalized exact output matches",
        "5 / 5 normalized exact output matches",
        "Graph/flowchart copied fixture exact subset: 40 / 79.",
        "Graph/flowchart named deterministic differences: 39 / 79.",
        "Sequence copied fixture parity: 17 / 17.",
        "GRAPH_FIXTURE_GAPS.md",
        "cargo nextest run -p merman-ascii fixture_inventory graph_fixture sequence_golden",
    ] {
        assert!(
            contract.contains(expected),
            "v1 coverage contract must mention `{expected}`"
        );
    }
}

#[test]
fn fixture_inventory_documents_graph_exact_and_gap_disposition() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gap_inventory =
        fs::read_to_string(manifest_dir.join("tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md"))
            .expect("graph fixture gap inventory must be readable");

    for expected in [
        "Copied corpus: 79 fixtures",
        "Exact-output subset: 40 fixtures",
        "Named intentional differences: 39 fixtures",
        "Every exact fixture must still match byte-for-byte",
        "Every named gap must still render successfully",
    ] {
        assert!(
            gap_inventory.contains(expected),
            "graph fixture gap inventory must mention `{expected}`"
        );
    }
}

#[test]
fn phase_gate_report_matches_executable_corpus_counts() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("merman-ascii must live under the workspace crates directory");
    let report =
        std::fs::read_to_string(workspace_root.join("docs/rendering/ASCII_PHASE_GATE_REPORT.md"))
            .expect("phase gate report should be tracked");

    assert!(
        report.contains("40/79 exact plus 39 named renderable differences"),
        "phase gate report must match the executable 40 exact / 39 gap graph disposition"
    );
    assert!(
        report.contains("moving-reference lane contains 140 uniquely identified paths"),
        "phase gate report must match the executable 140-path moving inventory"
    );
}

#[test]
fn reference_comparison_matches_executable_fixture_evidence() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let comparison = fs::read_to_string(manifest_dir.join("ASCII_REFERENCE_COMPARISON.md"))
        .expect("reference comparison must be readable");

    for expected in [
        "40 fixtures as an exact-output subset",
        "39 deterministic Dagre/route/compound differences",
        "The 140-path moving fixture delta",
    ] {
        assert!(
            comparison.contains(expected),
            "reference comparison must mention `{expected}`"
        );
    }

    for stale in [
        "45 fixtures as an exact-output subset",
        "34 deterministic Dagre/route/compound differences",
        "The 137-path moving fixture delta",
    ] {
        assert!(
            !comparison.contains(stale),
            "reference comparison contains stale evidence `{stale}`"
        );
    }
}

#[test]
fn moving_reference_manifest_records_each_discovery_fixture_once() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(manifest_dir.join("ASCII_MOVING_REFERENCE_MANIFEST.md"))
        .expect("moving reference manifest must be readable");

    for expected in [
        "b1b35f67d6a5dd0699ccfc968c00a763db573076",
        "6fffb8e2714acab2c4cb41c78894fabbc62cee56",
        "2ac8bbbb060ca0a65a6a21f3200bd99b1587b488",
        "Mermaid `11.16.1`",
        "Classification: `mermaid_valid`",
        "Classification: `mixed_valid_private_behavior`",
        "Classification: `reference_private`",
        "Admission: `semantic_probe`",
        "Admission: `discovery_only`",
        "Semantic feature:",
        "Raw reference delta: 144 paths",
        "leaving 140 moving-only paths",
    ] {
        assert!(
            manifest.contains(expected),
            "moving reference manifest must mention `{expected}`"
        );
    }

    let dispositions = parse_moving_fixture_dispositions(&manifest);
    let entries = dispositions
        .iter()
        .map(|disposition| disposition.path.as_str())
        .collect::<Vec<_>>();
    let unique_entries = entries.iter().copied().collect::<BTreeSet<_>>();

    assert_eq!(entries.len(), 140, "moving discovery fixture count drifted");
    assert_eq!(
        unique_entries.len(),
        entries.len(),
        "moving discovery fixture identities must be unique"
    );

    for (prefix, expected_count) in [
        ("ascii/", 3),
        ("extended-chars/", 2),
        ("multibyte/", 3),
        ("sequence/", 31),
        ("sequence-ascii/", 20),
        ("er/", 69),
        ("er-ascii/", 12),
    ] {
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.starts_with(prefix))
                .count(),
            expected_count,
            "moving discovery fixture count drifted for {prefix}"
        );
    }

    for disposition in &dispositions {
        match (
            disposition.classification.as_str(),
            disposition.admission.as_str(),
        ) {
            ("mermaid_valid", "semantic_probe")
            | ("mixed_valid_private_behavior", "discovery_only")
            | ("reference_private", "discovery_only") => {}
            pair => panic!(
                "invalid moving-fixture disposition {pair:?} for {} in section {}",
                disposition.path, disposition.section
            ),
        }
        assert!(
            !disposition.semantic_feature.is_empty(),
            "moving fixture {} must name its semantic feature",
            disposition.path
        );
        assert!(
            !disposition.evidence.is_empty(),
            "moving fixture {} must name its evidence",
            disposition.path
        );
    }

    let by_path = dispositions
        .iter()
        .map(|disposition| (disposition.path.as_str(), disposition))
        .collect::<BTreeMap<_, _>>();
    let quoted = by_path
        .get("sequence/quoted_name_with_arrow.txt")
        .expect("quoted actor discovery fixture must stay classified");
    assert_eq!(quoted.classification, "reference_private");
    assert_eq!(quoted.admission, "discovery_only");

    for path in [
        "multibyte/accented_latin_node_and_edge_labels.txt",
        "multibyte/cyrillic_node_and_edge_labels.txt",
        "multibyte/greek_node_and_edge_labels.txt",
    ] {
        let disposition = by_path
            .get(path)
            .unwrap_or_else(|| panic!("missing moving fixture disposition for {path}"));
        assert_eq!(disposition.classification, "mermaid_valid");
        assert_eq!(disposition.admission, "semantic_probe");
    }

    for (relative, test_name) in [
        (
            "tests/flowchart_model/direction_and_labels.rs",
            "flowchart_parser_multibyte_reference_labels_render_readably",
        ),
        (
            "tests/sequence_model/control_composition.rs",
            "sequence_control_frame_uses_the_local_participant_span",
        ),
        (
            "../merman-core/src/tests/sequence.rs",
            "parse_diagram_sequence_rejects_reference_private_quoted_actor_ids",
        ),
    ] {
        let path = manifest_dir.join(relative);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            source.contains(&format!("fn {test_name}")),
            "moving-reference evidence anchor `{test_name}` must exist in {}",
            path.display()
        );
        assert!(
            manifest.contains(test_name),
            "moving-reference manifest must cite executable evidence `{test_name}`"
        );
    }
}

#[test]
fn moving_reference_evidence_anchors_resolve_to_executable_tests() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../..");
    let manifest = fs::read_to_string(manifest_dir.join("ASCII_MOVING_REFERENCE_MANIFEST.md"))
        .expect("moving reference manifest must be readable");
    let dispositions = parse_moving_fixture_dispositions(&manifest);
    let evidence_sets = dispositions
        .iter()
        .map(|disposition| disposition.evidence.as_str())
        .collect::<BTreeSet<_>>();

    for evidence in evidence_sets {
        for encoded_anchor in evidence.split(", ") {
            let anchor = encoded_anchor
                .strip_prefix('`')
                .and_then(|value| value.strip_suffix('`'))
                .unwrap_or_else(|| {
                    panic!(
                        "moving-reference evidence must be a comma-separated list of backticked executable anchors: {evidence}"
                    )
                });
            let (relative_path, test_name) = anchor.rsplit_once("::").unwrap_or_else(|| {
                panic!("moving-reference evidence anchor must name a test: {anchor}")
            });
            assert!(
                relative_path.ends_with(".rs") && !test_name.is_empty(),
                "invalid moving-reference evidence anchor: {anchor}"
            );

            let source_path = workspace_root.join(relative_path);
            let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
                panic!("failed to read {}: {error}", source_path.display())
            });
            assert!(
                source.contains(&format!("fn {test_name}")),
                "moving-reference evidence anchor `{anchor}` does not resolve to an executable test"
            );
        }
    }
}

#[test]
fn imported_common_family_fixtures_preserve_primary_typed_facts() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let engine = Engine::new();
    let options = AsciiRenderOptions::ascii();
    let mut failures = Vec::new();
    let mut intentional_empty_seen = BTreeSet::new();
    assert!(
        IMPORTED_INTENTIONAL_EMPTY_FIXTURES
            .iter()
            .all(|(path, reason)| !path.is_empty() && !reason.is_empty()),
        "every intentional-empty imported fixture must name a path and reason"
    );

    for (directory, expected_kind, expected_count) in IMPORTED_FAMILY_FIXTURE_COUNTS {
        let root = workspace_root.join("fixtures").join(directory);
        let mut fixtures = fs::read_dir(&root)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
            .map(|entry| entry.expect("fixture entry must be readable").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "mmd"))
            .collect::<Vec<_>>();
        fixtures.sort();

        assert_eq!(
            fixtures.len(),
            *expected_count,
            "imported fixture count drifted for {}",
            root.display()
        );

        let mut rendered_count = 0usize;
        let mut rejected_invalid_count = 0usize;
        for path in fixtures {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("fixture name must be UTF-8");
            if *directory == "sequence" && file_name == "stress_end_keyword_016.mmd" {
                assert!(
                    engine
                        .parse_diagram_for_render_model_sync(
                            &fs::read_to_string(&path).expect("fixture must be readable"),
                            ParseOptions::strict(),
                        )
                        .is_err(),
                    "the documented upstream-invalid local stress fixture must stay excluded"
                );
                rejected_invalid_count += 1;
                continue;
            }
            let fixture_key = format!("{directory}/{file_name}");
            let expected_empty = IMPORTED_INTENTIONAL_EMPTY_FIXTURES
                .iter()
                .any(|(path, _)| *path == fixture_key);

            let result = (|| -> std::result::Result<(), String> {
                let source = fs::read_to_string(&path)
                    .map_err(|error| format!("failed to read fixture: {error}"))?;
                let parsed = engine
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .map_err(|error| format!("strict parse failed: {error}"))?
                    .ok_or_else(|| "diagram type was not detected".to_string())?;
                let model = parsed.model();
                if model.kind() != *expected_kind {
                    return Err(format!(
                        "expected typed family `{expected_kind}`, got `{}`",
                        model.kind()
                    ));
                }
                // This corpus gate owns primary authored-text visibility. Focused family tests own
                // topology, marker, control-frame, and field-role semantics.
                let witnesses = primary_typed_semantic_witnesses(model);
                let rendered = render_model(model, &options)
                    .map_err(|error| format!("ASCII render failed: {error}"))?;
                if expected_empty {
                    if !rendered.trim().is_empty() {
                        return Err(format!(
                            "document has terminal-visible output despite its intentional-empty disposition:\n{rendered}"
                        ));
                    }
                } else if rendered.trim().is_empty() {
                    return Err("ASCII renderer returned an empty document".to_string());
                } else {
                    assert_primary_typed_semantic_witnesses(&rendered, &witnesses)?;
                }
                Ok(())
            })();

            match result {
                Ok(()) => {
                    rendered_count += 1;
                    if expected_empty {
                        intentional_empty_seen.insert(fixture_key);
                    }
                }
                Err(error) => {
                    let relative = path.strip_prefix(&workspace_root).unwrap_or(&path);
                    failures.push(format!("{}: {error}", relative.display()));
                }
            }
        }

        let expected_invalid_count = usize::from(*directory == "sequence");
        assert_eq!(
            rejected_invalid_count,
            expected_invalid_count,
            "intentional-invalid fixture count drifted for {}",
            root.display()
        );
        assert_eq!(
            rendered_count,
            expected_count - expected_invalid_count,
            "rendered fixture count drifted for {}:\n{}",
            root.display(),
            failures.join("\n")
        );
    }

    assert!(
        failures.is_empty(),
        "imported common-family admission failures:\n{}",
        failures.join("\n")
    );
    assert_eq!(
        intentional_empty_seen,
        IMPORTED_INTENTIONAL_EMPTY_FIXTURES
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect(),
        "every empty imported render must have one explicit metadata-only disposition"
    );
}

#[derive(Debug)]
struct SemanticWitness<'model> {
    role: &'static str,
    text: &'model str,
    tokens: Vec<String>,
}

fn primary_typed_semantic_witnesses(model: &RenderSemanticModel) -> Vec<SemanticWitness<'_>> {
    let mut witnesses = Vec::new();
    match model {
        RenderSemanticModel::Sequence(model) => {
            if let Some(title) = &model.title {
                push_semantic_witness(&mut witnesses, "sequence title", title);
            }
            for actor_id in &model.actor_order {
                let Some(actor) = model.actors.get(actor_id) else {
                    continue;
                };
                let visible = if actor.description.is_empty() {
                    actor_id
                } else {
                    actor.description.as_str()
                };
                push_semantic_witness(&mut witnesses, "sequence participant", visible);
            }
            for sequence_box in &model.boxes {
                if let Some(name) = &sequence_box.name {
                    push_semantic_witness(&mut witnesses, "sequence box", name);
                }
            }
            for note in &model.notes {
                push_semantic_witness(&mut witnesses, "sequence note", &note.message);
            }
            for message in &model.messages {
                if matches!(
                    message.semantic_kind(),
                    SequenceMessageKind::Signal | SequenceMessageKind::Note
                ) {
                    push_semantic_witness(
                        &mut witnesses,
                        "sequence message",
                        message.message_text(),
                    );
                }
            }
        }
        RenderSemanticModel::Class(model) => {
            for class in model.classes.values() {
                let visible = if class.label.is_empty() {
                    class.id.as_str()
                } else {
                    class.label.as_str()
                };
                push_semantic_witness(&mut witnesses, "class identity", visible);
                for annotation in &class.annotations {
                    push_semantic_witness(&mut witnesses, "class annotation", annotation);
                }
                for member in class.members.iter().chain(&class.methods) {
                    let visible = if member.display_text.is_empty() {
                        member.id.as_str()
                    } else {
                        member.display_text.as_str()
                    };
                    push_semantic_witness(&mut witnesses, "class member", visible);
                }
            }
            for namespace in model.namespaces.values() {
                let visible = if namespace.label.is_empty() {
                    namespace.id.rsplit('.').next().unwrap_or(&namespace.id)
                } else {
                    namespace.label.as_str()
                };
                push_semantic_witness(&mut witnesses, "class namespace", visible);
            }
            for note in &model.notes {
                push_semantic_witness(&mut witnesses, "class note", &note.text);
            }
            for relation in &model.relations {
                push_semantic_witness(&mut witnesses, "class relation", &relation.title);
                for (role, label) in [
                    ("class source endpoint label", &relation.relation_title_1),
                    ("class target endpoint label", &relation.relation_title_2),
                ] {
                    // Core projects an absent endpoint label as this sentinel.
                    if !label.eq_ignore_ascii_case("none") {
                        push_semantic_witness(&mut witnesses, role, label);
                    }
                }
            }
        }
        RenderSemanticModel::Er(model) => {
            for entity in model.entities.values() {
                let visible = if entity.alias.is_empty() {
                    entity.label.as_str()
                } else {
                    entity.alias.as_str()
                };
                push_semantic_witness(&mut witnesses, "ER entity", visible);
                for attribute in &entity.attributes {
                    push_semantic_witness(&mut witnesses, "ER attribute type", &attribute.ty);
                    push_semantic_witness(&mut witnesses, "ER attribute name", &attribute.name);
                    for key in &attribute.keys {
                        push_semantic_witness(&mut witnesses, "ER attribute key", key);
                    }
                    push_semantic_witness(
                        &mut witnesses,
                        "ER attribute comment",
                        &attribute.comment,
                    );
                }
            }
            for relationship in &model.relationships {
                push_semantic_witness(
                    &mut witnesses,
                    "ER relationship label",
                    &relationship.role_a,
                );
            }
        }
        other => panic!(
            "imported common-family witness extraction does not support `{}`",
            other.kind()
        ),
    }
    witnesses
}

fn push_semantic_witness<'model>(
    witnesses: &mut Vec<SemanticWitness<'model>>,
    role: &'static str,
    text: &'model str,
) {
    let tokens = semantic_witness_tokens(text);
    if !tokens.is_empty() {
        witnesses.push(SemanticWitness { role, text, tokens });
    }
}

fn assert_primary_typed_semantic_witnesses(
    rendered: &str,
    witnesses: &[SemanticWitness<'_>],
) -> std::result::Result<(), String> {
    let rendered_tokens = semantic_tokens(rendered);
    for witness in witnesses {
        for token in &witness.tokens {
            if visible_token_occurrences(&rendered_tokens, token) == 0 {
                return Err(format!(
                    "{} fact {:?} lost visible token {:?}:\n{rendered}",
                    witness.role, witness.text, token
                ));
            }
        }
    }
    Ok(())
}

fn semantic_witness_tokens(text: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(text.len());
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some(after_break) = remaining.strip_prefix("\\n") {
            normalized.push(' ');
            remaining = after_break;
            continue;
        }
        if let Some(after_open) = remaining.strip_prefix("#lt;")
            && let Some(end) = after_open.find("#gt;")
        {
            normalized.push(' ');
            remaining = &after_open[end + "#gt;".len()..];
            continue;
        }
        if let Some(after_open) = remaining.strip_prefix('<')
            && let Some(end) = after_open.find('>')
        {
            let tag = after_open[..end].trim();
            if tag
                .as_bytes()
                .get(..2)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"br"))
                && tag[2..]
                    .chars()
                    .all(|character| character.is_ascii_whitespace() || character == '/')
            {
                normalized.push(' ');
                remaining = &after_open[end + 1..];
                continue;
            }
        }

        let character = remaining
            .chars()
            .next()
            .expect("non-empty semantic text must contain a character");
        normalized.push(character);
        remaining = &remaining[character.len_utf8()..];
    }

    semantic_tokens(&normalized)
}

fn semantic_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let flush = |token: &mut String, tokens: &mut Vec<String>| {
        if !token.is_empty() {
            tokens.push(std::mem::take(token));
        }
    };

    for character in text.chars() {
        if character.is_alphanumeric() || character == '_' {
            token.push(character);
        } else {
            flush(&mut token, &mut tokens);
        }
    }
    flush(&mut token, &mut tokens);
    tokens
}

fn visible_token_occurrences(tokens: &[String], expected: &str) -> usize {
    let mut occurrences = 0usize;
    for start in 0..tokens.len() {
        let mut joined = String::new();
        for token in &tokens[start..] {
            joined.push_str(token);
            if joined == expected {
                occurrences += 1;
                break;
            }
            if joined.len() >= expected.len() || !expected.starts_with(&joined) {
                break;
            }
        }
    }
    occurrences
}

#[test]
fn local_semantic_fixture_inventory_matches_readme() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.join("tests/testdata/local-semantic");
    let readme_path = root.join("README.md");
    let readme = fs::read_to_string(&readme_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", readme_path.display()));

    let documented = readme
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- `"))
        .filter_map(|line| line.strip_suffix('`'))
        .filter(|path| path.ends_with(".mmd"))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    let mut actual_paths = Vec::new();
    collect_local_semantic_fixtures(&root, &root, &mut actual_paths);
    let actual = actual_paths
        .into_iter()
        .map(|path| {
            path.strip_prefix(&root)
                .unwrap_or_else(|err| panic!("failed to relativize {}: {err}", path.display()))
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        documented, actual,
        "local semantic README must list every .mmd fixture and only existing fixtures"
    );
}

#[test]
fn local_semantic_fixture_readme_documents_admission_policy() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme_path = manifest_dir.join("tests/testdata/local-semantic/README.md");
    let readme = fs::read_to_string(&readme_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", readme_path.display()));

    for expected in [
        "Copied fixtures",
        "`mermaid-ascii` graph and sequence fixtures",
        "`beautiful-mermaid` is capability evidence",
        "Class and ER relation fixtures are split by topology readability",
        "routed-grid fixtures",
        "structured relation-summary fixtures",
        "Resource limits are",
        "`AsciiResourcePolicy` grid budget",
    ] {
        assert!(
            readme.contains(expected),
            "local semantic fixture policy must mention `{expected}`"
        );
    }
}

fn collect_local_semantic_fixtures(root: &Path, dir: &Path, fixtures: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
        .map(|entry| {
            entry
                .expect("local semantic fixture entry must be readable")
                .path()
        })
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_local_semantic_fixtures(root, &path, fixtures);
        } else if path.extension().is_some_and(|ext| ext == "mmd") {
            assert!(
                path.starts_with(root),
                "local semantic fixture must stay under {}: {}",
                root.display(),
                path.display()
            );
            fixtures.push(path);
        }
    }
}

fn fixture_cases_in(root: &Path, directory: &str) -> Vec<PathBuf> {
    let dir = root.join(directory);
    let mut fixtures = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
        .map(|entry| entry.expect("fixture entry must be readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .collect::<Vec<_>>();
    fixtures.sort();
    fixtures
}

fn parse_metadata_usize(metadata: &BTreeMap<&str, &str>, key: &str) -> usize {
    metadata
        .get(key)
        .unwrap_or_else(|| panic!("missing fixture provenance metadata `{key}`"))
        .parse()
        .unwrap_or_else(|error| panic!("invalid fixture provenance metadata `{key}`: {error}"))
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug)]
struct MovingFixtureDisposition {
    section: String,
    path: String,
    classification: String,
    admission: String,
    semantic_feature: String,
    evidence: String,
}

fn parse_moving_fixture_dispositions(manifest: &str) -> Vec<MovingFixtureDisposition> {
    let mut section = None;
    let mut classification = None;
    let mut admission = None;
    let mut semantic_feature = None;
    let mut evidence = None;
    let mut dispositions = Vec::new();

    for (line_index, line) in manifest.lines().enumerate() {
        if let Some(title) = line.strip_prefix("## ") {
            section = Some(title.to_owned());
            classification = None;
            admission = None;
            semantic_feature = None;
            evidence = None;
            continue;
        }
        if let Some(value) = line
            .strip_prefix("- Classification: `")
            .and_then(|value| value.strip_suffix('`'))
        {
            classification = Some(value.to_owned());
            continue;
        }
        if let Some(value) = line
            .strip_prefix("- Admission: `")
            .and_then(|value| value.strip_suffix('`'))
        {
            admission = Some(value.to_owned());
            continue;
        }
        if let Some(value) = line.strip_prefix("- Semantic feature: ") {
            semantic_feature = Some(value.to_owned());
            continue;
        }
        if let Some(value) = line.strip_prefix("- Evidence: ") {
            evidence = Some(value.to_owned());
            continue;
        }
        let Some(path) = line
            .strip_prefix("- `")
            .and_then(|value| value.strip_suffix('`'))
            .filter(|value| value.ends_with(".txt"))
        else {
            continue;
        };

        let context = || {
            format!(
                "moving fixture `{path}` at manifest line {}",
                line_index + 1
            )
        };
        dispositions.push(MovingFixtureDisposition {
            section: section
                .clone()
                .unwrap_or_else(|| panic!("{} has no section", context())),
            path: path.to_owned(),
            classification: classification
                .clone()
                .unwrap_or_else(|| panic!("{} has no classification", context())),
            admission: admission
                .clone()
                .unwrap_or_else(|| panic!("{} has no admission", context())),
            semantic_feature: semantic_feature
                .clone()
                .unwrap_or_else(|| panic!("{} has no semantic feature", context())),
            evidence: evidence
                .clone()
                .unwrap_or_else(|| panic!("{} has no evidence", context())),
        });
    }

    dispositions
}
