use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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
fn fixture_inventory_records_source_provenance() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(manifest_dir.join("tests/testdata/mermaid-ascii/README.md"))
        .expect("fixture README must be readable");
    let license = fs::read_to_string(manifest_dir.join("LICENSES/mermaid-ascii-MIT.txt"))
        .expect("upstream MIT license copy must be readable");

    assert!(readme.contains("https://github.com/AlexanderGrooff/mermaid-ascii"));
    assert!(readme.contains("6fffb8e"));
    assert!(readme.contains("MIT"));
    assert!(license.contains("MIT License"));
    assert!(license.contains("Copyright (c) 2023 Alexander Grooff"));
}

#[test]
fn fixture_inventory_documents_v1_coverage_contract() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contract = fs::read_to_string(manifest_dir.join("V1_MERMAID_ASCII_COVERAGE.md"))
        .expect("v1 coverage contract must be readable");

    for expected in [
        "6fffb8e2714acab2c4cb41c78894fabbc62cee56",
        "54 / 54 exact output matches",
        "25 / 25 exact output matches",
        "12 / 12 normalized exact output matches",
        "5 / 5 normalized exact output matches",
        "Graph/flowchart copied fixture parity: 79 / 79.",
        "Sequence copied fixture parity: 17 / 17.",
        "Named copied fixture gaps: none.",
        "cargo nextest run -p merman-ascii fixture_inventory graph_fixture sequence_golden",
    ] {
        assert!(
            contract.contains(expected),
            "v1 coverage contract must mention `{expected}`"
        );
    }
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
