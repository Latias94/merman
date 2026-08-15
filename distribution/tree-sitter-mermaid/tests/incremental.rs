use std::{
    cell::Cell,
    fs,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use tree_sitter::{InputEdit, Language, ParseOptions, Parser, Point, Tree};

use tree_sitter_mermaid::LANGUAGE;

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    kind: String,
    named: bool,
    error: bool,
    missing: bool,
    start_byte: usize,
    end_byte: usize,
    start_position: Point,
    end_position: Point,
    field: Option<String>,
    children: Vec<Snapshot>,
}

fn new_parser() -> Parser {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
}

fn snapshot(node: tree_sitter::Node<'_>, field: Option<&str>) -> Snapshot {
    snapshot_with_recovery(node, field, false)
}

fn recovery_normalized_snapshot(node: tree_sitter::Node<'_>, field: Option<&str>) -> Snapshot {
    snapshot_with_recovery(node, field, true)
}

fn snapshot_with_recovery(
    node: tree_sitter::Node<'_>,
    field: Option<&str>,
    collapse_error_children: bool,
) -> Snapshot {
    let mut cursor = node.walk();
    let children = if collapse_error_children && node.is_error() {
        Vec::new()
    } else {
        node.children(&mut cursor)
            .enumerate()
            .filter(|(_, child)| child.is_named())
            .map(|(index, child)| {
                let field_name = node.field_name_for_child(index as u32);
                snapshot_with_recovery(child, field_name, collapse_error_children)
            })
            .collect()
    };
    Snapshot {
        kind: node.kind().to_owned(),
        named: node.is_named(),
        error: node.is_error(),
        missing: node.is_missing(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_position: node.start_position(),
        end_position: node.end_position(),
        field: field.map(str::to_owned),
        children,
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

fn find_bytes(source: &[u8], needle: &[u8]) -> usize {
    source
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap_or_else(|| panic!("source does not contain {needle:?}"))
}

fn replace_and_compare(source: &[u8], start: usize, end: usize, replacement: &[u8]) {
    let mut parser = new_parser();
    let mut old_tree = parser
        .parse(source, None)
        .expect("initial parse must succeed");
    let mut edited = source.to_vec();
    edited.splice(start..end, replacement.iter().copied());

    let edit = InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: start + replacement.len(),
        start_position: point_at(source, start),
        old_end_position: point_at(source, end),
        new_end_position: point_at(&edited, start + replacement.len()),
    };
    old_tree.edit(&edit);
    let incremental = parser
        .parse(&edited, Some(&old_tree))
        .expect("incremental parse must produce a tree");
    let mut fresh_parser = new_parser();
    let fresh = fresh_parser
        .parse(&edited, None)
        .expect("fresh parse must produce a tree");
    assert_eq!(
        snapshot(incremental.root_node(), None),
        snapshot(fresh.root_node(), None),
        "incremental and fresh trees diverged after edit at byte {start}"
    );
}

#[test]
fn committed_u2_edit_traces_match_fresh_parse() {
    let traces: serde_json::Value =
        serde_json::from_str(include_str!("../test/edits/u2-mechanics.json"))
            .expect("committed edit traces must be valid JSON");
    for trace in traces.as_array().expect("edit traces must be an array") {
        let name = trace["name"].as_str().expect("edit trace name");
        let source = trace["source"].as_str().expect("edit trace source");
        let old = trace["old"].as_str().expect("edit trace old text");
        let replacement = trace["replacement"]
            .as_str()
            .expect("edit trace replacement");
        let start = source
            .find(old)
            .unwrap_or_else(|| panic!("{name}: old text is absent"));
        replace_and_compare(
            source.as_bytes(),
            start,
            start + old.len(),
            replacement.as_bytes(),
        );
    }
}

fn family_edit_trace_paths() -> Vec<PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("test/edits/families");
    let mut paths = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| entry.expect("family edit trace directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[test]
fn committed_family_edit_traces_match_fresh_parse() {
    let paths = family_edit_trace_paths();
    assert!(!paths.is_empty(), "family edit traces must not be empty");
    for path in paths {
        let traces: serde_json::Value = serde_json::from_slice(
            &fs::read(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
        )
        .unwrap_or_else(|error| panic!("invalid family edit traces {}: {error}", path.display()));
        for trace in traces.as_array().unwrap_or_else(|| {
            panic!(
                "family edit trace file must be an array: {}",
                path.display()
            )
        }) {
            let name = trace["name"].as_str().expect("edit trace name");
            let source = trace["source"].as_str().expect("edit trace source");
            let old = trace["old"].as_str().expect("edit trace old text");
            let replacement = trace["replacement"]
                .as_str()
                .expect("edit trace replacement");
            assert!(!old.is_empty(), "{name}: old text must not be empty");
            let mut matches = source.match_indices(old);
            let (start, _) = matches
                .next()
                .unwrap_or_else(|| panic!("{name}: old text is absent"));
            assert!(
                matches.next().is_none(),
                "{name}: old text must occur exactly once"
            );
            replace_and_compare(
                source.as_bytes(),
                start,
                start + old.len(),
                replacement.as_bytes(),
            );
        }
    }
}

fn parse_with_read_count(
    parser: &mut Parser,
    source: &[u8],
    old_tree: Option<&Tree>,
) -> (Tree, usize, usize, usize) {
    const INPUT_CHUNK_BYTES: usize = 64;
    let mut supplied = 0;
    let mut covered = vec![false; source.len()];
    let mut progress_callbacks = 0;
    let mut read = |offset: usize, _position: Point| {
        let end = offset.saturating_add(INPUT_CHUNK_BYTES).min(source.len());
        let chunk = source.get(offset..end).unwrap_or_default();
        supplied += chunk.len();
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
        .expect("counted parse must produce a tree");
    let unique_coverage = covered.into_iter().filter(|covered| *covered).count();
    (tree, supplied, unique_coverage, progress_callbacks)
}

#[test]
fn header_switch_replaces_the_reused_family_subtree() {
    let source = b"flowchart TD\n  A --> B\n";
    replace_and_compare(source, 0, b"flowchart TD".len(), b"sequenceDiagram");

    let mut parser = new_parser();
    let tree = parser
        .parse(b"sequenceDiagram\n  A ->> B: hi\n", None)
        .unwrap();
    let sexp = tree.root_node().to_sexp();
    assert!(sexp.contains("sequence_diagram"));
    assert!(!sexp.contains("flowchart_diagram"));

    let scanner_source = b"mindmap\nRoot\n  Child\n";
    replace_and_compare(scanner_source, 0, b"mindmap".len(), b"treeView-beta");
}

#[test]
fn parse_options_cancel_then_resume_with_the_same_parser_and_options() {
    let mut source = b"mindmap\n  Root\n".to_vec();
    for index in 0..8_192 {
        source.extend_from_slice(format!("    Branch {index}\n      Leaf {index}\n").as_bytes());
    }
    let cancellation_offset = source.len() / 4;

    let mut parser = new_parser();
    let mut read = |offset: usize, _position: Point| source.get(offset..).unwrap_or_default();
    let cancel = Cell::new(true);
    let callbacks = Cell::new(0_usize);
    let cancelled_at = Cell::new(0_usize);
    let mut progress = |state: &tree_sitter::ParseState| {
        callbacks.set(callbacks.get() + 1);
        if cancel.get() && state.current_byte_offset() >= cancellation_offset {
            cancelled_at.set(state.current_byte_offset());
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let mut options = ParseOptions::new().progress_callback(&mut progress);

    let cancelled = parser.parse_with_options(&mut read, None, Some(options.reborrow()));
    assert!(
        cancelled.is_none(),
        "parsing must cancel after scanner state has been established"
    );
    assert!(callbacks.get() > 1);
    assert!(cancelled_at.get() >= cancellation_offset);

    cancel.set(false);
    callbacks.set(0);
    let resumed = parser
        .parse_with_options(&mut read, None, Some(options.reborrow()))
        .expect("the cancelled parser must remain reusable");
    assert!(
        callbacks.get() > 0,
        "the reused options must invoke the callback"
    );

    let mut fresh_parser = new_parser();
    let fresh = fresh_parser
        .parse(&source, None)
        .expect("fresh parse must produce a tree");
    assert_eq!(
        snapshot(resumed.root_node(), None),
        snapshot(fresh.root_node(), None)
    );
}

#[test]
fn preamble_edits_match_fresh_parse() {
    let source =
        b"---\ntitle: Alpha\n---\n%%{init: {\"theme\": \"default\"}}%%\nflowchart TD\nA --> B\n";
    let title = find_bytes(source, b"Alpha");
    replace_and_compare(source, title, title + b"Alpha".len(), "世界".as_bytes());

    let theme = find_bytes(source, b"default");
    replace_and_compare(source, theme, theme + b"default".len(), b"dark");

    let directive_end = find_bytes(source, b"}%%") + 3;
    replace_and_compare(
        source,
        directive_end,
        directive_end,
        b"\n%% edited preamble",
    );
}

#[test]
fn venn_and_event_modeling_structural_edits_match_fresh_parse() {
    let venn = b"venn-beta\nset A [Alpha]: 1\nset B [Beta]: 2\nunion A,B [Both]: 1\n";
    let first_set = find_bytes(venn, b"set A");
    replace_and_compare(venn, first_set, first_set + b"set A".len(), b"union A,B");

    let event_modeling =
        b"eventmodeling\ndata Payload `json` {\n  {\"id\": 1}\n  {\"nested\": {\"ok\": true}}\n}\n";
    let nested = find_bytes(event_modeling, b"{\"ok\": true}");
    replace_and_compare(
        event_modeling,
        nested,
        nested + b"{\"ok\": true}".len(),
        b"{\"ok\": false, \"label\": \"updated\"}",
    );
}

#[test]
fn treemap_and_tree_view_scanner_edits_match_fresh_parse() {
    let treemap = b"treemap-beta\n\"Root\"\n  \"Child\"\n    \"Leaf\": 1\n  \"Sibling\": 2\n";
    let treemap_leaf = find_bytes(treemap, b"    \"Leaf\"");
    replace_and_compare(treemap, treemap_leaf, treemap_leaf + 4, b" ");

    let tree_view =
        "treeView-beta\r\nRoot\r\n  子节点\r\n    Grandchild\r\n  Sibling\r\n".as_bytes();
    let grandchild = find_bytes(tree_view, b"    Grandchild");
    replace_and_compare(tree_view, grandchild, grandchild + 4, b"  ");

    let box_drawing = "treeView-beta\nRoot\n├── Child\n│   └── Grandchild\n".as_bytes();
    let child = find_bytes(box_drawing, b"Child");
    replace_and_compare(
        box_drawing,
        child,
        child + b"Child".len(),
        "子节点".as_bytes(),
    );
}

#[test]
fn scanner_indent_reindent_and_multiline_payload_edits_match_fresh_parse() {
    let mindmap = b"mindmap\nRoot\n    A\n      B\n    C\n";
    let start = mindmap
        .windows(4)
        .position(|window| window == b"    ")
        .unwrap();
    replace_and_compare(mindmap, start, start + 4, b"  ");

    let kanban = b"kanban\n  Todo\n    task1\n  Done\n    task2\n";
    let task = find_bytes(kanban, b"    task1");
    replace_and_compare(kanban, task, task + 4, b"      ");

    let sankey = b"sankey-beta\n\"Source\nline\",Target,1\n";
    let edit = sankey
        .windows(4)
        .position(|window| window == b"line")
        .unwrap();
    replace_and_compare(sankey, edit, edit + 4, b"edited");

    let zenuml = b"zenuml\nif(flag) {\n  A.method()\n}\n";
    let flag = zenuml
        .windows(4)
        .position(|window| window == b"flag")
        .unwrap();
    replace_and_compare(zenuml, flag, flag + 4, b"ready");
}

#[test]
fn flowchart_statement_insertions_and_deletions_match_fresh_parse() {
    let header = b"flowchart TD\n";
    let statement = b"  A --> B\n";
    let mut source = header.to_vec();
    for _ in 0..96 {
        source.extend_from_slice(statement);
    }

    for statement_index in [31, 32, 33, 63, 64, 65] {
        let edit_start = header.len() + statement_index * statement.len();
        replace_and_compare(&source, edit_start, edit_start, b"  X --> Y\n");
        replace_and_compare(&source, edit_start, edit_start + statement.len(), b"");
    }
}

#[test]
fn flowchart_recovery_is_line_local_and_preserves_structural_siblings() {
    let source = b"flowchart TD\nA -->\nB --> C\nD\n";
    let mut parser = new_parser();
    let tree = parser.parse(source, None).expect("parse must succeed");
    assert!(!tree.root_node().has_error());
    let sexp = tree.root_node().to_sexp();
    assert!(sexp.contains("flow_incomplete_edge_statement"));
    assert_eq!(sexp.matches("flow_edge_statement").count(), 1);
    assert_eq!(sexp.matches("flow_node_statement").count(), 1);

    let arrow = source
        .windows(3)
        .position(|window| window == b"-->")
        .expect("source has an incomplete arrow");
    replace_and_compare(source, arrow + 3, arrow + 3, b" X");

    let missing_end = b"flowchart TD\nsubgraph cluster\nA --> B\n";
    let tree = parser
        .parse(missing_end, None)
        .expect("missing terminator must recover");
    assert!(tree.root_node().has_error());
    let sexp = tree.root_node().to_sexp();
    assert!(sexp.contains("flow_subgraph"));
    assert!(sexp.contains("(flow_subgraph_end (MISSING \"end\"))"));
    assert!(sexp.contains("flow_edge_statement"));
}

#[test]
fn malformed_flowchart_tail_after_many_statements_has_bounded_work() {
    let mut source = b"flowchart TD\n".to_vec();
    for _ in 0..32 {
        source.extend_from_slice(b"  A --> B\n");
    }
    source.extend_from_slice(b"@@@ [unterminated\n");

    let mut parser = new_parser();
    let (tree, supplied_bytes, coverage_bytes, progress_callbacks) =
        parse_with_read_count(&mut parser, &source, None);
    assert!(tree.root_node().has_error());
    assert!(
        supplied_bytes <= source.len() * 2,
        "malformed tail supplied {supplied_bytes} bytes for a {}-byte source",
        source.len()
    );
    assert_eq!(coverage_bytes, source.len());
    assert!(progress_callbacks <= 64);
}

#[test]
fn crlf_and_bare_cr_point_edits_remain_fresh_equivalent() {
    let crlf = b"flowchart TD\r\n  A --> B\r\n";
    let label = crlf.iter().position(|&byte| byte == b'B').unwrap();
    replace_and_compare(crlf, label, label + 1, "终".as_bytes());

    let bare_cr = b"mindmap\rRoot\r  Child\r";
    let child = bare_cr
        .windows(5)
        .position(|window| window == b"Child")
        .unwrap();
    replace_and_compare(bare_cr, child, child + 5, b"Leaf");
}

#[test]
fn invalid_utf8_is_bounded_and_reparseable() {
    let source = b"mindmap\nRoot\n\xff\xff\xff\n";
    let mut parser = new_parser();
    let first = parser
        .parse(source, None)
        .expect("invalid bytes must not panic");
    let second = parser.parse(source, None).expect("parser remains reusable");
    assert_eq!(
        snapshot(first.root_node(), None),
        snapshot(second.root_node(), None)
    );
}

#[test]
fn arbitrary_byte_header_edits_remain_fresh_equivalent() {
    const OPERATIONS: [(usize, usize, &[u8]); 4] = [
        (0, 0, b" "),
        (5, 1, b""),
        (2, 0, &[0xe7, 0xbb, 0x88]),
        (255, 2, &[0xe2, 0x94, 0x82]),
    ];

    let mut source = b"m ishikawa".to_vec();
    let mut parser = new_parser();
    let mut old_tree = parser.parse(&source, None).expect("initial parse");

    for (index, (position_seed, delete, replacement)) in OPERATIONS.into_iter().enumerate() {
        let start = position_seed % (source.len() + 1);
        let end = start + delete.min(source.len() - start);
        let start_position = point_at(&source, start);
        let old_end_position = point_at(&source, end);
        source.splice(start..end, replacement.iter().copied());
        old_tree.edit(&InputEdit {
            start_byte: start,
            old_end_byte: end,
            new_end_byte: start + replacement.len(),
            start_position,
            old_end_position,
            new_end_position: point_at(&source, start + replacement.len()),
        });

        let incremental = parser
            .parse(&source, Some(&old_tree))
            .expect("incremental parse");
        let mut fresh_parser = new_parser();
        let fresh = fresh_parser.parse(&source, None).expect("fresh parse");
        assert_eq!(
            recovery_normalized_snapshot(incremental.root_node(), None),
            recovery_normalized_snapshot(fresh.root_node(), None),
            "operation {} diverged for source {:?}",
            index + 1,
            source
        );
        old_tree = incremental;
    }
}

#[test]
fn local_edit_work_stays_local_on_a_256_kib_document() {
    let metrics: serde_json::Value =
        serde_json::from_str(include_str!("../metadata/metrics/u2-mechanics.json"))
            .expect("U2 mechanics metrics must be valid JSON");
    let recorded = &metrics["observed"]["freshAndIncrementalWork"];
    let mut source = b"flowchart TD\n".to_vec();
    let statement = format!("  A[{}] --> B\n", "x".repeat(992));
    while source.len() < 256 * 1024 {
        source.extend_from_slice(statement.as_bytes());
    }
    let mut initial_parser = new_parser();
    let (mut old_tree, initial_fresh_bytes, initial_fresh_coverage, initial_fresh_work) =
        parse_with_read_count(&mut initial_parser, &source, None);

    let midpoint = source.len() / 2;
    let edit_start = source[midpoint..]
        .iter()
        .position(|byte| *byte == b'A')
        .map(|offset| midpoint + offset)
        .expect("large flowchart contains a node after its midpoint");
    let mut edited = source.clone();
    edited[edit_start] = b'C';
    old_tree.edit(&InputEdit {
        start_byte: edit_start,
        old_end_byte: edit_start + 1,
        new_end_byte: edit_start + 1,
        start_position: point_at(&source, edit_start),
        old_end_position: point_at(&source, edit_start + 1),
        new_end_position: point_at(&edited, edit_start + 1),
    });
    let mut pending = vec![old_tree.root_node()];
    let mut changed_nodes = 0;
    while let Some(node) = pending.pop() {
        if node.has_changes() {
            changed_nodes += 1;
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }

    let (incremental, incremental_bytes, incremental_coverage, incremental_work) =
        parse_with_read_count(&mut initial_parser, &edited, Some(&old_tree));
    let mut fresh_parser = new_parser();
    let (fresh, final_fresh_bytes, final_fresh_coverage, final_fresh_work) =
        parse_with_read_count(&mut fresh_parser, &edited, None);

    assert_eq!(
        snapshot(incremental.root_node(), None),
        snapshot(fresh.root_node(), None)
    );
    eprintln!(
        "long-label local edit: source={}, edit={}, fresh={}/{}/{}, incremental={}/{}/{}, changed_nodes={changed_nodes}",
        edited.len(),
        edit_start,
        final_fresh_bytes,
        final_fresh_coverage,
        final_fresh_work,
        incremental_bytes,
        incremental_coverage,
        incremental_work,
    );
    let recorded_u64 = |field: &str| {
        recorded[field]
            .as_u64()
            .unwrap_or_else(|| panic!("missing U2 local-edit metric {field}")) as usize
    };
    assert_eq!(recorded_u64("sourceBytes"), edited.len());
    assert_eq!(recorded_u64("inputChunkBytes"), 64);
    assert_eq!(recorded_u64("editByte"), edit_start);
    assert_eq!(recorded_u64("freshSuppliedBytes"), final_fresh_bytes);
    assert_eq!(recorded_u64("freshSuppliedBytes"), initial_fresh_bytes);
    assert_eq!(
        recorded_u64("freshUniqueCoverageBytes"),
        final_fresh_coverage
    );
    assert_eq!(
        recorded_u64("freshUniqueCoverageBytes"),
        initial_fresh_coverage
    );
    assert_eq!(recorded_u64("incrementalSuppliedBytes"), incremental_bytes);
    assert_eq!(
        recorded_u64("incrementalUniqueCoverageBytes"),
        incremental_coverage
    );
    assert_eq!(recorded_u64("freshProgressCallbacks"), final_fresh_work);
    assert_eq!(recorded_u64("freshProgressCallbacks"), initial_fresh_work);
    assert_eq!(
        recorded_u64("incrementalProgressCallbacks"),
        incremental_work
    );
    assert_eq!(recorded_u64("changedNamedNodes"), changed_nodes);
    let read_limit = recorded_u64("maxIncrementalSuppliedPermille");
    let work_limit = recorded_u64("maxIncrementalProgressPermille");
    assert!(
        incremental_bytes * 1000 <= edited.len() * read_limit,
        "local edit supplied {incremental_bytes} bytes for a {}-byte source",
        edited.len()
    );
    assert!(
        incremental_work * 1000 <= final_fresh_work * work_limit,
        "local edit used {incremental_work} progress callbacks versus {final_fresh_work} fresh"
    );
}

#[test]
fn local_edit_coverage_stays_local_for_common_short_statements() {
    let metrics: serde_json::Value =
        serde_json::from_str(include_str!("../metadata/metrics/u2-mechanics.json"))
            .expect("U2 mechanics metrics must be valid JSON");
    let recorded = &metrics["observed"]["commonShortStatementLocalEdits"];
    let recorded_u64 = |field: &str| {
        recorded[field]
            .as_u64()
            .unwrap_or_else(|| panic!("missing U2 short-local-edit metric {field}"))
            as usize
    };
    let recorded_operations = recorded["operations"]
        .as_array()
        .expect("U2 short-local-edit operations must be an array");
    let mut source = b"flowchart TD\n".to_vec();
    let statement = b"  A --> B\n";
    while source.len() < 256 * 1024 {
        source.extend_from_slice(statement);
    }

    assert_eq!(recorded_u64("sourceBytes"), source.len());
    assert_eq!(recorded_u64("inputChunkBytes"), 64);
    assert_eq!(recorded_operations.len(), 3);

    for (operation, expected_operation) in ["replace", "insert-statement", "delete-statement"]
        .into_iter()
        .zip(recorded_operations)
    {
        assert_eq!(expected_operation["operation"], operation);
        let recorded_positions = expected_operation["positions"]
            .as_array()
            .expect("operation positions must be an array");
        assert_eq!(recorded_positions.len(), 3);
        for ((position_name, numerator), expected) in
            [("quarter", 1), ("middle", 2), ("three-quarter", 3)]
                .into_iter()
                .zip(recorded_positions)
        {
            let mut parser = new_parser();
            let (mut old_tree, initial_bytes, initial_coverage, initial_work) =
                parse_with_read_count(&mut parser, &source, None);
            let requested_position = source.len() * numerator / 4;
            let node_id = source[requested_position..]
                .iter()
                .position(|byte| *byte == b'A')
                .map(|offset| requested_position + offset)
                .expect("large flowchart contains a node after every requested position");
            let statement_start = node_id - 2;
            let (edit_start, old_end, replacement): (usize, usize, &[u8]) = match operation {
                "replace" => (node_id, node_id + 1, b"C"),
                "insert-statement" => (statement_start, statement_start, b"  X --> Y\n"),
                "delete-statement" => (statement_start, statement_start + statement.len(), b""),
                _ => unreachable!(),
            };
            let mut edited = source.clone();
            edited.splice(edit_start..old_end, replacement.iter().copied());
            old_tree.edit(&InputEdit {
                start_byte: edit_start,
                old_end_byte: old_end,
                new_end_byte: edit_start + replacement.len(),
                start_position: point_at(&source, edit_start),
                old_end_position: point_at(&source, old_end),
                new_end_position: point_at(&edited, edit_start + replacement.len()),
            });

            let (incremental, incremental_bytes, incremental_coverage, incremental_work) =
                parse_with_read_count(&mut parser, &edited, Some(&old_tree));
            let mut fresh_parser = new_parser();
            let (fresh, fresh_bytes, fresh_coverage, fresh_work) =
                parse_with_read_count(&mut fresh_parser, &edited, None);
            assert_eq!(
                snapshot(incremental.root_node(), None),
                snapshot(fresh.root_node(), None),
                "{operation} {position_name} edit diverged from a fresh tree"
            );
            eprintln!(
                "short-statement {operation} {position_name}: edit={edit_start}, source={}, initial={}/{}/{}, fresh={}/{}/{}, incremental={}/{}/{}",
                edited.len(),
                initial_bytes,
                initial_coverage,
                initial_work,
                fresh_bytes,
                fresh_coverage,
                fresh_work,
                incremental_bytes,
                incremental_coverage,
                incremental_work,
            );
            assert_eq!(recorded_u64("freshSuppliedBytes"), initial_bytes);
            assert_eq!(recorded_u64("freshUniqueCoverageBytes"), initial_coverage);
            assert_eq!(recorded_u64("freshProgressCallbacks"), initial_work);
            assert_eq!(fresh_coverage, edited.len());
            assert_eq!(expected["position"], position_name);
            assert_eq!(expected["editByte"].as_u64(), Some(edit_start as u64));
            assert_eq!(
                expected["editedSourceBytes"].as_u64(),
                Some(edited.len() as u64)
            );
            assert_eq!(
                expected["freshSuppliedBytes"].as_u64(),
                Some(fresh_bytes as u64)
            );
            assert_eq!(
                expected["freshUniqueCoverageBytes"].as_u64(),
                Some(fresh_coverage as u64)
            );
            assert_eq!(
                expected["freshProgressCallbacks"].as_u64(),
                Some(fresh_work as u64)
            );
            assert_eq!(
                expected["incrementalSuppliedBytes"].as_u64(),
                Some(incremental_bytes as u64)
            );
            assert_eq!(
                expected["incrementalUniqueCoverageBytes"].as_u64(),
                Some(incremental_coverage as u64)
            );
            assert_eq!(
                expected["incrementalProgressCallbacks"].as_u64(),
                Some(incremental_work as u64)
            );
            assert!(
                incremental_bytes <= recorded_u64("maxIncrementalSuppliedBytes"),
                "{operation} {position_name} edit supplied {incremental_bytes} bytes"
            );
            assert!(
                incremental_coverage <= recorded_u64("maxIncrementalUniqueCoverageBytes"),
                "{operation} {position_name} edit uniquely covered {incremental_coverage} bytes"
            );
            assert!(
                incremental_work * 1000
                    <= fresh_work * recorded_u64("maxIncrementalProgressPermille"),
                "{operation} {position_name} edit used {incremental_work} callbacks versus {fresh_work} fresh"
            );
        }
    }
}
