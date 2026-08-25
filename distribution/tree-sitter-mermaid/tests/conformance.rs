use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_fixture_render_context::RenderContextCatalog;
use serde::Deserialize;
use tree_sitter::{Language, Node, Parser};
use tree_sitter_mermaid::LANGUAGE;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilyFixture {
    public_id: String,
    root: String,
    source: String,
}

fn read_family_fixtures(package_root: &Path) -> Vec<FamilyFixture> {
    let path = package_root.join("test/fixtures/family-roots.json");
    let source = fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_slice(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn collect_mmd_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];

    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| {
                    panic!("failed to inspect {}: {error}", directory.display())
                })
                .path();
            if path.is_dir() {
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('_'))
                {
                    continue;
                }
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "mmd") {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

fn broad_recovery_node(kind: &str) -> bool {
    matches!(
        kind,
        "catch_all_body"
            | "raw_line"
            | "unknown_statement"
            | "unstructured_body"
            | "unstructured_statement"
    ) || [
        "_incomplete",
        "_invalid",
        "_malformed",
        "_recovered",
        "_recovery",
        "_unclosed",
    ]
    .iter()
    .any(|fragment| kind.contains(fragment))
}

#[test]
fn broad_recovery_names_fail_the_strict_fixture_boundary() {
    for kind in [
        "sequence_recovered_statement",
        "sequence_invalid_line",
        "packet_recovery_statement",
        "journey_malformed_statement",
        "flowchart_incomplete_edge_statement",
        "langium_unclosed_string",
    ] {
        assert!(broad_recovery_node(kind), "{kind} must fail the oracle");
    }
    assert!(!broad_recovery_node("sequence_message_statement"));
}

fn first_invalid_node(node: Node<'_>) -> Option<String> {
    if node.is_error() {
        return Some("ERROR".to_string());
    }
    if node.is_missing() {
        return Some(format!("missing {}", node.kind()));
    }
    if broad_recovery_node(node.kind()) {
        return Some(node.kind().to_string());
    }

    let mut cursor = node.walk();
    node.children(&mut cursor).find_map(first_invalid_node)
}

fn diagram_roots(root: Node<'_>) -> Vec<String> {
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .filter(|node| node.kind().ends_with("_diagram"))
        .map(|node| node.kind().to_string())
        .collect()
}

fn validate_tree(parser: &mut Parser, source: &str, expected_root: &str) -> Result<(), String> {
    let tree = parser
        .parse(source.as_bytes(), None)
        .ok_or_else(|| "parse was cancelled".to_string())?;
    let root = tree.root_node();
    if let Some(invalid) = first_invalid_node(root) {
        return Err(format!("entered {invalid}"));
    }
    let roots = diagram_roots(root);
    if roots != [expected_root] {
        return Err(format!(
            "selected diagram roots {roots:?}; expected {expected_root}"
        ));
    }
    Ok(())
}

#[test]
fn representative_sources_cover_the_public_family_catalog() {
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = read_family_fixtures(package_root);
    let expected = merman_core::supported_diagrams()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let actual = fixtures
        .iter()
        .map(|fixture| fixture.public_id.as_str())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        fixtures.len(),
        actual.len(),
        "duplicate public family fixture"
    );
    assert_eq!(actual, expected);

    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    for fixture in fixtures {
        validate_tree(&mut parser, &fixture.source, &fixture.root)
            .unwrap_or_else(|error| panic!("{} representative source: {error}", fixture.public_id));
    }
}

#[test]
fn every_strict_valid_merman_fixture_has_a_clean_tree_sitter_tree() {
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = package_root
        .parent()
        .and_then(Path::parent)
        .expect("package must live under distribution/");
    let fixtures_root = workspace_root.join("fixtures");
    let catalog = RenderContextCatalog::load(&fixtures_root)
        .expect("fixture render-context catalog must load");
    let families = read_family_fixtures(package_root);
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");

    let mut strict_valid_by_family = BTreeMap::<&str, usize>::new();
    let mut failure_counts = BTreeMap::<String, usize>::new();
    let mut failures = Vec::new();
    let default_engine = Engine::new();

    for family in &families {
        let directory = fixtures_root.join(&family.public_id);
        assert!(
            directory.is_dir(),
            "public family {} has no fixture directory at {}",
            family.public_id,
            directory.display()
        );

        for path in collect_mmd_files(&directory) {
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            let context = catalog
                .context_for_fixture(&path)
                .unwrap_or_else(|error| panic!("{} render context: {error}", path.display()));
            let contextual_engine = context.map(|context| {
                Engine::new()
                    .with_site_config(MermaidConfig::from_value(context.site_config_value()))
            });
            let engine = contextual_engine.as_ref().unwrap_or(&default_engine);

            let Ok(Some(diagram)) = engine.parse_diagram_sync(&source, ParseOptions::strict())
            else {
                continue;
            };

            let detected = merman_core::diagram_type_metadata_id(&diagram.meta.diagram_type)
                .unwrap_or(diagram.meta.diagram_type.as_str());
            if detected != family.public_id {
                *failure_counts
                    .entry(format!(
                        "{}: strict parser selected {detected}",
                        family.public_id
                    ))
                    .or_default() += 1;
                failures.push(format!(
                    "{}: strict Merman parse belongs to {detected}, not fixture family {}",
                    path.display(),
                    family.public_id
                ));
                continue;
            }
            *strict_valid_by_family
                .entry(family.public_id.as_str())
                .or_default() += 1;

            if let Err(error) = validate_tree(&mut parser, &source, &family.root) {
                *failure_counts
                    .entry(format!("{}: {error}", family.public_id))
                    .or_default() += 1;
                failures.push(format!("{}: {error}", path.display()));
            }
        }
    }

    for family in &families {
        assert!(
            strict_valid_by_family
                .get(family.public_id.as_str())
                .is_some_and(|count| *count > 0),
            "public family {} has no strict-valid fixture",
            family.public_id
        );
    }

    assert!(
        failures.is_empty(),
        "{} strict-valid Merman fixtures violate the Tree-sitter boundary:\n\ncounts:\n{}\n\nfixtures:\n{}",
        failures.len(),
        failure_counts
            .iter()
            .map(|(failure, count)| format!("{count:>4}  {failure}"))
            .collect::<Vec<_>>()
            .join("\n"),
        failures
            .iter()
            .take(200)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
