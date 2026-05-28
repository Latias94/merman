use std::fs;
use std::path::Path;

const EXPECTED_FIXTURE_COUNTS: &[(&str, usize)] = &[
    ("ascii", 52),
    ("extended-chars", 23),
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
