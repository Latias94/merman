use std::{fs, path::Path};

use serde::Deserialize;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};
use tree_sitter_mermaid::LANGUAGE;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilyFixture {
    public_id: String,
    #[serde(rename = "root")]
    _root: String,
    source: String,
}

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

fn family_fixtures() -> Vec<FamilyFixture> {
    serde_json::from_str(include_str!("../test/fixtures/family-roots.json"))
        .expect("family root fixtures must be valid JSON")
}

fn capture_names(
    parser: &mut Parser,
    query: &Query,
    query_path: &str,
    source: &str,
) -> Vec<String> {
    let tree = parser
        .parse(source, None)
        .expect("query fixture must parse");
    assert!(!tree.root_node().has_error(), "{query_path}: {source}");

    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(query, tree.root_node(), source.as_bytes());
    let mut names = Vec::new();
    while let Some((query_match, capture_index)) = captures.next() {
        let capture = query_match.captures[*capture_index];
        let capture_index = usize::try_from(capture.index).expect("capture index fits usize");
        names.push(capture_names[capture_index].to_owned());
    }
    names
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
fn canonical_highlights_cover_every_public_family() {
    let fixtures = family_fixtures();
    assert_eq!(fixtures.len(), 35);
    let language = language();
    let query_path = "queries/portable/highlights.scm";
    let query = Query::new(&language, &query_source(query_path))
        .unwrap_or_else(|error| panic!("{query_path} does not compile: {error}"));
    let mut parser = parser();

    for fixture in fixtures {
        let captures = capture_names(&mut parser, &query, query_path, &fixture.source);
        assert!(
            captures.iter().any(|capture| capture == "keyword"),
            "{} has no canonical keyword highlight",
            fixture.public_id
        );
    }
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
    assert!(
        captures
            .iter()
            .any(|capture| capture == "injection.content")
    );
}
