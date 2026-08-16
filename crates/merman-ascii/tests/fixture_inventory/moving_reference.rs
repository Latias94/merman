use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[test]
fn moving_reference_manifest_records_each_discovery_fixture_once() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let authority_path =
        manifest_dir.join("tests/testdata/mermaid-ascii/MOVING_REFERENCE_DISPOSITIONS.tsv");
    let authority = fs::read_to_string(&authority_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", authority_path.display()));
    let (metadata, dispositions) = parse_moving_fixture_dispositions(&authority);
    assert_eq!(
        metadata,
        BTreeMap::from([
            ("format", "merman-ascii-moving-reference-dispositions-v1"),
            (
                "moving_reference",
                "b1b35f67d6a5dd0699ccfc968c00a763db573076",
            ),
            (
                "immutable_baseline",
                "6fffb8e2714acab2c4cb41c78894fabbc62cee56",
            ),
            (
                "capability_prior_art",
                "2ac8bbbb060ca0a65a6a21f3200bd99b1587b488",
            ),
            ("mermaid_version", "11.16.1"),
        ])
    );
    let entries = dispositions
        .iter()
        .map(|disposition| disposition.path.as_str())
        .collect::<Vec<_>>();
    let unique_entries = entries.iter().copied().collect::<BTreeSet<_>>();

    assert_eq!(
        unique_entries.len(),
        entries.len(),
        "moving discovery fixture identities must be unique"
    );

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
}

#[derive(Debug)]
struct MovingFixtureDisposition {
    section: String,
    path: String,
    classification: String,
    admission: String,
    semantic_feature: String,
}

fn parse_moving_fixture_dispositions(
    authority: &str,
) -> (BTreeMap<&str, &str>, Vec<MovingFixtureDisposition>) {
    let mut metadata = BTreeMap::new();
    let mut dispositions = Vec::new();
    let mut rows_started = false;

    for (line_index, line) in authority.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        if !rows_started && let Some(record) = line.strip_prefix("# ") {
            let (key, value) = record.split_once('\t').unwrap_or_else(|| {
                panic!(
                    "invalid moving-reference metadata at line {}",
                    line_index + 1
                )
            });
            assert!(
                metadata.insert(key, value).is_none(),
                "duplicate moving-reference metadata key `{key}`"
            );
            continue;
        }
        if !rows_started {
            assert_eq!(
                line, "section\tclassification\tadmission\tsemantic_feature\tpath",
                "moving-reference authority header drifted"
            );
            rows_started = true;
            continue;
        }
        assert!(
            !line.starts_with("# "),
            "moving-reference metadata must precede the header at line {}",
            line_index + 1
        );
        let fields = line.split('\t').collect::<Vec<_>>();
        let [section, classification, admission, semantic_feature, path] = fields.as_slice() else {
            panic!(
                "invalid moving-reference disposition at line {}",
                line_index + 1
            );
        };
        assert!(
            !section.is_empty(),
            "moving fixture section must not be empty"
        );
        assert!(
            path.ends_with(".txt")
                && !path.starts_with('/')
                && !path.contains('\\')
                && path.split('/').all(|component| !component.is_empty()
                    && component != "."
                    && component != ".."),
            "invalid moving fixture path `{path}`"
        );
        dispositions.push(MovingFixtureDisposition {
            section: (*section).to_owned(),
            path: (*path).to_owned(),
            classification: (*classification).to_owned(),
            admission: (*admission).to_owned(),
            semantic_feature: (*semantic_feature).to_owned(),
        });
    }

    assert!(rows_started, "moving-reference authority header is missing");

    (metadata, dispositions)
}
