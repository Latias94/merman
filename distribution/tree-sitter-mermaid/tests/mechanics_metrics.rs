use std::ops::ControlFlow;
use std::time::Instant;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tree_sitter::{InputEdit, Language, ParseOptions, Parser, Point, Tree};

use tree_sitter_mermaid::LANGUAGE;

const INPUT_CHUNK_BYTES: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FamilyFixture {
    public_id: String,
    root: String,
    source: String,
}

#[derive(Debug)]
struct ParseWork {
    tree: Tree,
    supplied_bytes: usize,
    unique_coverage_bytes: usize,
    progress_callbacks: usize,
}

fn parser() -> Parser {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
}

fn parse_with_work(parser: &mut Parser, source: &[u8], old_tree: Option<&Tree>) -> ParseWork {
    let mut supplied_bytes = 0;
    let mut covered = vec![false; source.len()];
    let mut progress_callbacks = 0;
    let mut read = |offset: usize, _position: Point| {
        let end = offset.saturating_add(INPUT_CHUNK_BYTES).min(source.len());
        let chunk = source.get(offset..end).unwrap_or_default();
        supplied_bytes += chunk.len();
        covered[offset.min(source.len())..end].fill(true);
        chunk
    };
    let mut progress = |_state: &tree_sitter::ParseState| {
        progress_callbacks += 1;
        ControlFlow::Continue(())
    };
    let tree = parser
        .parse_with_options(
            &mut read,
            old_tree,
            Some(ParseOptions::new().progress_callback(&mut progress)),
        )
        .expect("instrumented parse must produce a tree");
    ParseWork {
        tree,
        supplied_bytes,
        unique_coverage_bytes: covered.into_iter().filter(|covered| *covered).count(),
        progress_callbacks,
    }
}

fn point_at(source: &[u8], byte: usize) -> Point {
    let mut row = 0;
    let mut column = 0;
    for &value in &source[..byte] {
        if value == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    Point { row, column }
}

fn metric_u64(value: &serde_json::Value, field: &str) -> usize {
    value[field]
        .as_u64()
        .unwrap_or_else(|| panic!("mechanics metrics lack numeric {field}")) as usize
}

fn short_flowchart(target_kib: usize) -> Vec<u8> {
    let mut source = b"flowchart TD\n".to_vec();
    while source.len() < target_kib * 1024 {
        source.extend_from_slice(b"  A --> B\n");
    }
    source
}

#[test]
fn real_corpus_work_matches_the_receipt_bound_snapshot() {
    let fixture_bytes = include_bytes!("../metadata/fixtures/family-roots.json");
    let fixtures: Vec<FamilyFixture> =
        serde_json::from_slice(fixture_bytes).expect("family fixtures must be valid JSON");
    let metrics: serde_json::Value =
        serde_json::from_str(include_str!("../metadata/metrics/u2-mechanics.json"))
            .expect("U2 mechanics metrics must be valid JSON");
    let recorded = &metrics["observed"]["realCorpus"];

    let started = Instant::now();
    let mut source_bytes = 0;
    let mut supplied_bytes = 0;
    let mut unique_coverage_bytes = 0;
    let mut progress_callbacks = 0;
    for fixture in &fixtures {
        let mut parser = parser();
        let work = parse_with_work(&mut parser, fixture.source.as_bytes(), None);
        assert!(!work.tree.root_node().has_error(), "{}", fixture.public_id);
        let roots = work
            .tree
            .root_node()
            .named_children(&mut work.tree.root_node().walk())
            .filter(|node| node.kind().ends_with("_diagram"))
            .map(|node| node.kind().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(roots, [fixture.root.as_str()], "{}", fixture.public_id);
        source_bytes += fixture.source.len();
        supplied_bytes += work.supplied_bytes;
        unique_coverage_bytes += work.unique_coverage_bytes;
        progress_callbacks += work.progress_callbacks;
    }
    let wall_milliseconds = started.elapsed().as_millis() as usize;
    eprintln!(
        "real corpus: sources={source_bytes}, supplied={supplied_bytes}, coverage={unique_coverage_bytes}, callbacks={progress_callbacks}, wall_ms={wall_milliseconds}"
    );

    assert_eq!(metric_u64(recorded, "fixtureCount"), fixtures.len());
    assert_eq!(metric_u64(recorded, "sourceBytes"), source_bytes);
    assert_eq!(metric_u64(recorded, "freshSuppliedBytes"), supplied_bytes);
    assert_eq!(
        metric_u64(recorded, "freshUniqueCoverageBytes"),
        unique_coverage_bytes
    );
    assert_eq!(
        metric_u64(recorded, "freshProgressCallbacks"),
        progress_callbacks
    );
    let digest = format!("{:x}", Sha256::digest(fixture_bytes));
    assert_eq!(recorded["fixtureManifestSha256"], digest);
    assert!(
        wall_milliseconds <= metric_u64(recorded, "maxFreshWallMilliseconds"),
        "real-corpus parse took {wall_milliseconds} ms"
    );
}

#[test]
fn synthetic_doubling_work_matches_the_complexity_ratchet() {
    let metrics: serde_json::Value =
        serde_json::from_str(include_str!("../metadata/metrics/u2-mechanics.json"))
            .expect("U2 mechanics metrics must be valid JSON");
    let recorded = &metrics["observed"]["syntheticDoubling"];
    let lanes = recorded["lanes"]
        .as_array()
        .expect("synthetic doubling lanes must be an array");
    assert_eq!(lanes.len(), 5);

    let mut fresh_callbacks = Vec::with_capacity(lanes.len());
    let mut snapshot_mismatches = Vec::new();
    for (lane, target_kib) in lanes.iter().zip([64, 128, 256, 512, 1024]) {
        assert_eq!(metric_u64(lane, "targetKiB"), target_kib);
        let source = short_flowchart(target_kib);
        let started = Instant::now();
        let mut incremental_parser = parser();
        let mut initial = parse_with_work(&mut incremental_parser, &source, None);
        let fresh_wall_milliseconds = started.elapsed().as_millis() as usize;

        let requested = source.len() / 2;
        let edit_byte = source[requested..]
            .iter()
            .position(|byte| *byte == b'A')
            .map(|offset| requested + offset)
            .expect("synthetic flowchart has a node after its midpoint");
        let mut edited = source.clone();
        edited[edit_byte] = b'C';
        initial.tree.edit(&InputEdit {
            start_byte: edit_byte,
            old_end_byte: edit_byte + 1,
            new_end_byte: edit_byte + 1,
            start_position: point_at(&source, edit_byte),
            old_end_position: point_at(&source, edit_byte + 1),
            new_end_position: point_at(&edited, edit_byte + 1),
        });
        let incremental = parse_with_work(&mut incremental_parser, &edited, Some(&initial.tree));
        let mut fresh_parser = parser();
        let final_fresh = parse_with_work(&mut fresh_parser, &edited, None);
        assert_eq!(
            incremental.tree.root_node().to_sexp(),
            final_fresh.tree.root_node().to_sexp(),
            "{target_kib} KiB incremental tree differs from fresh"
        );
        eprintln!(
            "synthetic {target_kib} KiB: bytes={}, fresh={}/{}/{}, incremental={}/{}/{}, wall_ms={fresh_wall_milliseconds}",
            source.len(),
            initial.supplied_bytes,
            initial.unique_coverage_bytes,
            initial.progress_callbacks,
            incremental.supplied_bytes,
            incremental.unique_coverage_bytes,
            incremental.progress_callbacks,
        );

        for (field, actual) in [
            ("sourceBytes", source.len()),
            ("editByte", edit_byte),
            ("freshSuppliedBytes", initial.supplied_bytes),
            ("freshUniqueCoverageBytes", initial.unique_coverage_bytes),
            ("freshProgressCallbacks", initial.progress_callbacks),
            ("incrementalSuppliedBytes", incremental.supplied_bytes),
            (
                "incrementalUniqueCoverageBytes",
                incremental.unique_coverage_bytes,
            ),
            (
                "incrementalProgressCallbacks",
                incremental.progress_callbacks,
            ),
        ] {
            let expected = metric_u64(lane, field);
            if expected != actual {
                snapshot_mismatches.push(format!(
                    "{target_kib} KiB {field}: recorded {expected}, observed {actual}"
                ));
            }
        }
        assert!(
            fresh_wall_milliseconds <= metric_u64(lane, "maxFreshWallMilliseconds"),
            "{target_kib} KiB fresh parse took {fresh_wall_milliseconds} ms"
        );
        fresh_callbacks.push(initial.progress_callbacks);
    }

    assert!(
        snapshot_mismatches.is_empty(),
        "synthetic-doubling snapshot drift:\n{}",
        snapshot_mismatches.join("\n")
    );

    let growth_limit = metric_u64(recorded, "maxConsecutiveGrowthPermille");
    assert_eq!(growth_limit, 3000);
    let threefold = fresh_callbacks
        .windows(2)
        .map(|pair| pair[1] * 1000 >= pair[0] * growth_limit)
        .collect::<Vec<_>>();
    assert!(
        !threefold.windows(2).any(|pair| pair == [true, true]),
        "fresh parse work has two consecutive at-least-threefold increases: {fresh_callbacks:?}"
    );
}
