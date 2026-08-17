use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    ops::Range,
    path::Path,
};

use serde::Deserialize;
use tree_sitter::{
    InputEdit, Language, Parser, Point, Query, QueryCursor, StreamingIterator, Tree,
};
use tree_sitter_mermaid::LANGUAGE;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilyFixture {
    public_id: String,
    #[serde(rename = "root")]
    _root: String,
    source: String,
}

#[derive(Debug, PartialEq, Eq)]
struct CaptureSnapshot {
    name: String,
    node_kind: String,
    range: Range<usize>,
    start_position: Point,
    end_position: Point,
    text: String,
}

const EXPECTED_NON_HEADER_CAPTURE_CLASSES: &[(&str, &str)] = &[
    (
        "architecture",
        "constant keyword namespace operator punctuation.delimiter string string.special variable",
    ),
    (
        "block",
        "constant keyword number operator property punctuation.bracket punctuation.delimiter string variable",
    ),
    (
        "c4",
        "keyword keyword.operator punctuation.bracket punctuation.delimiter string type variable",
    ),
    ("class", "keyword operator property type"),
    ("cynefin", "keyword operator string"),
    ("er", "operator string type"),
    (
        "eventmodeling",
        "keyword number operator string type type.builtin variable",
    ),
    (
        "flowchart",
        "constant operator punctuation.bracket punctuation.delimiter string variable",
    ),
    ("gantt", "keyword number punctuation.delimiter string"),
    (
        "gitgraph",
        "keyword property punctuation.delimiter string",
    ),
    // `info` has no post-header body. Its fixture intentionally proves bounded
    // frontmatter highlighting instead of inventing diagram-body captures.
    ("info", "attribute punctuation.special"),
    ("ishikawa", "string"),
    (
        "journey",
        "keyword namespace number punctuation.delimiter string variable",
    ),
    ("kanban", "string"),
    ("mindmap", "string"),
    (
        "packet",
        "keyword number operator punctuation.delimiter string",
    ),
    ("pie", "keyword number punctuation.delimiter string"),
    (
        "quadrantchart",
        "keyword number operator punctuation.bracket punctuation.delimiter string",
    ),
    (
        "radar",
        "attribute function keyword number property punctuation.special string variable",
    ),
    (
        "railroad",
        "function keyword operator string string.special variable",
    ),
    (
        "railroadAbnf",
        "comment function number operator string variable",
    ),
    (
        "railroadEbnf",
        "function operator string string.special variable",
    ),
    (
        "railroadPeg",
        "constant function operator string variable",
    ),
    (
        "requirement",
        "constant property punctuation.bracket punctuation.delimiter string type variable",
    ),
    ("sankey", "comment number punctuation.delimiter string"),
    ("sequence", "keyword operator string type variable"),
    (
        "state",
        "constant operator punctuation.delimiter string variable",
    ),
    (
        "swimlane",
        "constant keyword operator punctuation.bracket punctuation.delimiter string variable",
    ),
    ("timeline", "keyword punctuation.delimiter string"),
    ("treeView", "string"),
    ("treemap", "number punctuation.delimiter string"),
    (
        "venn",
        "keyword punctuation.bracket punctuation.delimiter string variable",
    ),
    ("wardley", "keyword number operator string variable"),
    (
        "xychart",
        "keyword number punctuation.bracket punctuation.delimiter",
    ),
    ("zenuml", "operator string variable"),
];

fn language() -> Language {
    LANGUAGE.into()
}

fn parser() -> Parser {
    let language = language();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
}

fn query_source(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn canonical_highlights_query() -> Query {
    let query_path = "queries/portable/highlights.scm";
    Query::new(&language(), &query_source(query_path))
        .unwrap_or_else(|error| panic!("{query_path} does not compile: {error}"))
}

fn family_fixtures() -> Vec<FamilyFixture> {
    serde_json::from_str(include_str!("../test/fixtures/family-roots.json"))
        .expect("family root fixtures must be valid JSON")
}

fn capture_snapshots(query: &Query, tree: &Tree, source: &str) -> Vec<CaptureSnapshot> {
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(query, tree.root_node(), source.as_bytes());
    let mut snapshots = Vec::new();
    while let Some((query_match, capture_index)) = captures.next() {
        let capture = query_match.captures[*capture_index];
        let capture_index = usize::try_from(capture.index).expect("capture index fits usize");
        let range = capture.node.byte_range();
        let text = source.get(range.clone()).unwrap_or_else(|| {
            panic!(
                "capture {} has invalid UTF-8 bounds {range:?}",
                capture_names[capture_index]
            )
        });
        snapshots.push(CaptureSnapshot {
            name: capture_names[capture_index].to_owned(),
            node_kind: capture.node.kind().to_owned(),
            range,
            start_position: capture.node.start_position(),
            end_position: capture.node.end_position(),
            text: text.to_owned(),
        });
    }
    snapshots
}

fn parse_capture_snapshots(
    parser: &mut Parser,
    query: &Query,
    query_path: &str,
    source: &str,
) -> Vec<CaptureSnapshot> {
    let tree = parser
        .parse(source, None)
        .expect("query fixture must parse");
    assert!(!tree.root_node().has_error(), "{query_path}: {source}");
    capture_snapshots(query, &tree, source)
}

fn capture_names(
    parser: &mut Parser,
    query: &Query,
    query_path: &str,
    source: &str,
) -> Vec<String> {
    parse_capture_snapshots(parser, query, query_path, source)
        .into_iter()
        .map(|capture| capture.name)
        .collect()
}

fn assert_exact_capture(
    captures: &[CaptureSnapshot],
    source: &str,
    name: &str,
    expected_text: &str,
) {
    let start = source
        .find(expected_text)
        .unwrap_or_else(|| panic!("fixture must contain {expected_text:?}"));
    let expected_range = start..start + expected_text.len();
    assert!(
        captures.iter().any(|capture| {
            capture.name == name && capture.range == expected_range && capture.text == expected_text
        }),
        "missing exact {name} capture for {expected_text:?} at {expected_range:?}: {captures:#?}"
    );
}

fn point_at(source: &[u8], byte: usize) -> Point {
    let mut point = Point::new(0, 0);
    for value in &source[..byte] {
        if *value == b'\n' {
            point.row += 1;
            point.column = 0;
        } else {
            point.column += 1;
        }
    }
    point
}

fn replace_and_compare_highlights(source: &str, needle: &str, replacement: &str) {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("fixture must contain {needle:?}"));
    let end = start + needle.len();
    let mut edited = source.as_bytes().to_vec();
    edited.splice(start..end, replacement.bytes());
    let edited = String::from_utf8(edited).expect("edited fixture remains UTF-8");

    let query = canonical_highlights_query();
    let mut incremental_parser = parser();
    let mut old_tree = incremental_parser
        .parse(source, None)
        .expect("initial parse must produce a tree");
    old_tree.edit(&InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: start + replacement.len(),
        start_position: point_at(source.as_bytes(), start),
        old_end_position: point_at(source.as_bytes(), end),
        new_end_position: point_at(edited.as_bytes(), start + replacement.len()),
    });

    let incremental = incremental_parser
        .parse(&edited, Some(&old_tree))
        .expect("incremental parse must produce a tree");
    let fresh = parser()
        .parse(&edited, None)
        .expect("fresh parse must produce a tree");
    assert_eq!(
        capture_snapshots(&query, &incremental, &edited),
        capture_snapshots(&query, &fresh, &edited),
        "incremental and fresh highlights diverged after replacing {needle:?}"
    );
}

fn scm_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        {
            let path = entry.expect("query directory entry").path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "scm") {
                files.push(
                    path.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
                        .expect("query path belongs to the package")
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    files.sort();
    files
}

#[test]
fn every_packaged_query_compiles() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = scm_files(&package.join("queries"));
    assert!(!files.is_empty());

    let language = language();
    for path in files {
        Query::new(&language, &query_source(&path))
            .unwrap_or_else(|error| panic!("{path} does not compile: {error}"));
    }
}

#[test]
fn canonical_highlights_define_non_header_capture_classes_for_every_family() {
    let fixtures = family_fixtures();
    assert_eq!(fixtures.len(), 35);
    assert_eq!(EXPECTED_NON_HEADER_CAPTURE_CLASSES.len(), 35);

    let expected_by_family: BTreeMap<_, _> = EXPECTED_NON_HEADER_CAPTURE_CLASSES
        .iter()
        .copied()
        .collect();
    assert_eq!(
        expected_by_family.len(),
        EXPECTED_NON_HEADER_CAPTURE_CLASSES.len()
    );

    let query_path = "queries/portable/highlights.scm";
    let query = canonical_highlights_query();
    let mut parser = parser();

    for fixture in fixtures {
        let captures = parse_capture_snapshots(&mut parser, &query, query_path, &fixture.source);
        let actual: BTreeSet<_> = captures
            .iter()
            .filter(|capture| capture.node_kind != "diagram_keyword")
            .map(|capture| capture.name.as_str())
            .collect();
        let expected: BTreeSet<_> = expected_by_family
            .get(fixture.public_id.as_str())
            .unwrap_or_else(|| panic!("{} has no capture-class contract", fixture.public_id))
            .split_ascii_whitespace()
            .collect();
        assert_eq!(
            actual, expected,
            "{} non-header capture classes changed",
            fixture.public_id
        );
    }
}

#[test]
fn canonical_highlights_keep_exact_archetype_spans() {
    let query_path = "queries/portable/highlights.scm";
    let query = canonical_highlights_query();
    let mut parser = parser();

    let flowchart = concat!(
        "---\r\n",
        "title: \"Demo\"\r\n",
        "---\r\n",
        "%%{init: {\"theme\":\"neutral\"}}%%\r\n",
        "%% comment\r\n",
        "flowchart TD\r\n",
        "  Alpha[\"Hello 🌍\"] --> Beta\r\n",
        "  style Alpha fill:#969,stroke-width:4px\r\n",
    );
    let captures = parse_capture_snapshots(&mut parser, &query, query_path, flowchart);
    for (name, text) in [
        ("attribute", "title: \"Demo\""),
        ("attribute", "%%{init: {\"theme\":\"neutral\"}}%%"),
        ("comment", "%% comment"),
        ("variable", "Beta"),
        ("punctuation.bracket", "["),
        ("string", "\"Hello 🌍\""),
        ("operator", "-->"),
        ("property", "fill"),
        ("string", "#969"),
        ("property", "stroke-width"),
        ("string", "4px"),
    ] {
        assert_exact_capture(&captures, flowchart, name, text);
    }

    let gantt = "gantt\r\n  Task one :a1, 2024-01-01, 1d\r\n";
    let captures = parse_capture_snapshots(&mut parser, &query, query_path, gantt);
    assert_exact_capture(&captures, gantt, "number", "2024-01-01");
    assert_exact_capture(&captures, gantt, "number", "1d");

    let radar = "radar-beta\naccDescr {\n第一行\nsecond line\n}\naxis A, B\n";
    let captures = parse_capture_snapshots(&mut parser, &query, query_path, radar);
    assert_exact_capture(&captures, radar, "string", "{\n第一行\nsecond line\n}");
}

#[test]
fn malformed_statement_keeps_later_sibling_highlights() {
    let source = "flowchart TD\n  A[broken label\n  C[Later] --> Target[Done]\n";
    let query = canonical_highlights_query();
    let tree = parser()
        .parse(source, None)
        .expect("recovery parse must produce a tree");
    assert!(tree.root_node().has_error());

    let captures = capture_snapshots(&query, &tree, source);
    for (name, text) in [
        ("variable", "C"),
        ("string", "Later"),
        ("operator", "-->"),
        ("variable", "Target"),
        ("string", "Done"),
    ] {
        assert_exact_capture(&captures, source, name, text);
    }
    assert!(captures.iter().all(|capture| {
        capture.range.start <= capture.range.end && capture.range.end <= source.len()
    }));
}

#[test]
fn incremental_highlights_match_fresh_queries() {
    replace_and_compare_highlights(
        "flowchart TD\nA[Start] --> B[Finish]\n",
        "Finish",
        "完成 🌍",
    );
    replace_and_compare_highlights(
        "mindmap\n  id1[\"`Root\na second line\nUnicode 🤓`\"]\n",
        "second",
        "next",
    );
    replace_and_compare_highlights(
        "mindmap\nRoot\n    Branch\n      Leaf\n",
        "    Branch",
        "  Branch",
    );
}

#[test]
fn portable_non_highlight_queries_execute_on_representative_sources() {
    let fixtures = family_fixtures();
    let architecture = fixtures
        .iter()
        .find(|fixture| fixture.public_id == "architecture")
        .expect("architecture fixture");

    let language = language();
    let mut parser = parser();
    for query_path in ["queries/portable/locals.scm", "queries/portable/tags.scm"] {
        let query = Query::new(&language, &query_source(query_path))
            .unwrap_or_else(|error| panic!("{query_path} does not compile: {error}"));
        assert!(
            !capture_names(&mut parser, &query, query_path, &architecture.source).is_empty(),
            "{query_path} produced no captures"
        );
    }

    let event_modeling = "eventmodeling\ndata Payload `json` {\n  {\"id\": 1}\n}\n";
    let query_path = "queries/portable/injections.scm";
    let query = Query::new(&language, &query_source(query_path))
        .unwrap_or_else(|error| panic!("{query_path} does not compile: {error}"));
    let captures = capture_names(&mut parser, &query, query_path, event_modeling);
    assert!(captures
        .iter()
        .any(|capture| capture == "injection.content"));
}
