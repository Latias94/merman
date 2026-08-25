use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const EXPECTED_FIXTURE_COUNTS: &[(&str, usize)] = &[
    ("ascii", 54),
    ("extended-chars", 25),
    ("sequence", 12),
    ("sequence-ascii", 5),
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
fn fixture_inventory_keeps_the_upstream_license_copy() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let license = fs::read_to_string(manifest_dir.join("LICENSES/mermaid-ascii-MIT.txt"))
        .expect("upstream MIT license copy must be readable");

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
