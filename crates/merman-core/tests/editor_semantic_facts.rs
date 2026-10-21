#![cfg(all(feature = "all-diagrams", feature = "compat-json"))]

#[cfg(feature = "editor-facts")]
use merman_core::presentation::PresentationConfig;
#[cfg(feature = "editor-facts")]
use merman_core::time::CivilDate;
#[cfg(feature = "editor-facts")]
use merman_core::{EditorSemanticFacts, diagram_family_capabilities};
use merman_core::{Engine, Error, SourceSpan};
#[cfg(feature = "editor-facts")]
use merman_fixture_render_context::RenderContextCatalog;
#[cfg(feature = "editor-facts")]
use std::collections::BTreeMap;
#[cfg(feature = "editor-facts")]
use std::fs;
#[cfg(feature = "editor-facts")]
use std::path::{Path, PathBuf};
#[cfg(feature = "editor-facts")]
use std::sync::OnceLock;

#[cfg(feature = "editor-facts")]
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

#[cfg(feature = "editor-facts")]
fn fixtures_root() -> PathBuf {
    workspace_root().join("fixtures")
}

#[cfg(feature = "editor-facts")]
fn fixture_render_contexts() -> &'static RenderContextCatalog {
    static CATALOG: OnceLock<RenderContextCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        RenderContextCatalog::load(fixtures_root()).expect("valid fixture render context catalog")
    })
}

#[cfg(feature = "editor-facts")]
fn fixture_site_config_for_path(path: &Path) -> Option<PresentationConfig> {
    fixture_render_contexts()
        .context_for_fixture(path)
        .unwrap_or_else(|error| {
            panic!(
                "invalid fixture render context lookup for {}: {error}",
                path.display()
            )
        })
        .map(|context| PresentationConfig::from_value(context.site_config_value()))
}

#[cfg(feature = "editor-facts")]
fn engine_for_fixture(base: &Engine, path: &Path) -> Engine {
    match fixture_site_config_for_path(path) {
        Some(site_config) => base.clone().with_site_config(site_config),
        None => base.clone(),
    }
}

#[cfg(feature = "editor-facts")]
fn list_formal_fixture_mmd_files(root: &Path) -> Vec<PathBuf> {
    // Generated reference SVGs and temporary/private trees are not authored source fixtures.
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with('_') || name.starts_with('.') || name == "upstream-svgs"
            })
        {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with('_') || name.starts_with('.') || name == "upstream-svgs"
                    })
                {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|extension| extension == "mmd") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[cfg(feature = "editor-facts")]
#[derive(Default)]
struct FormalFixtureEditorFactsAudit {
    total: usize,
    malformed_metadata: usize,
    unsupported: usize,
    supported: usize,
    available: usize,
    malformed_editor: usize,
    localized_drop_documents: usize,
}

#[cfg(feature = "editor-facts")]
impl FormalFixtureEditorFactsAudit {
    fn accounted(&self) -> usize {
        self.malformed_metadata + self.unsupported + self.available + self.malformed_editor
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn formal_fixture_corpus_keeps_supported_editor_facts_available() {
    let root = fixtures_root();
    let fixtures = list_formal_fixture_mmd_files(&root);
    assert!(
        !fixtures.is_empty(),
        "formal fixture corpus must not be empty"
    );

    let editor_capabilities = diagram_family_capabilities()
        .iter()
        .map(|capability| (capability.diagram_type, capability.has_editor_parser))
        .collect::<BTreeMap<_, _>>();
    let base = Engine::new()
        .with_fixed_today(Some(
            CivilDate::new(2026, 2, 15).expect("valid fixed fixture date"),
        ))
        .try_with_fixed_local_offset_minutes(0)
        .expect("UTC is a valid fixed offset");
    let mut audit = FormalFixtureEditorFactsAudit {
        total: fixtures.len(),
        ..Default::default()
    };
    #[cfg(feature = "editor-facts")]
    let semantic_base =
        merman_core::SemanticParser::new().with_runtime_policy(base.runtime_policy().clone());
    let mut unavailable = Vec::new();
    let mut malformed_samples = Vec::new();
    let mut localized_drop_fixtures = Vec::new();
    let mut unsupported_types = BTreeMap::<String, usize>::new();

    for path in fixtures {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let engine = engine_for_fixture(&base, &path);
        let metadata = match engine.parse_metadata_sync(&source) {
            Ok(metadata) => metadata,
            Err(error) => {
                assert!(
                    is_malformed_fixture_error(&error),
                    "{} failed metadata extraction for a non-syntax reason: {error}",
                    relative.display()
                );
                audit.malformed_metadata += 1;
                record_sample(
                    &mut malformed_samples,
                    format!("{} (metadata): {error}", relative.display()),
                );
                continue;
            }
        };

        let has_editor_parser = editor_capabilities
            .get(metadata.diagram_type.as_str())
            .unwrap_or_else(|| {
                panic!(
                    "{} detected uncataloged diagram type {}",
                    relative.display(),
                    metadata.diagram_type
                )
            });
        if !*has_editor_parser {
            audit.unsupported += 1;
            *unsupported_types
                .entry(metadata.diagram_type.clone())
                .or_default() += 1;
            continue;
        }
        audit.supported += 1;

        match engine.parse_editor_semantic_facts_with_type_sync(&metadata.diagram_type, &source) {
            Ok(Some(facts)) => {
                #[cfg(feature = "editor-facts")]
                {
                    let parser = match fixture_site_config_for_path(&path) {
                        Some(config) => semantic_base
                            .clone()
                            .with_site_config(config.as_config().clone()),
                        None => semantic_base.clone(),
                    };
                    let capture = parser
                        .capture_diagram_snapshot_with_type_sync(&metadata.diagram_type, &source);
                    let merman_core::SemanticSnapshotCapture::Snapshot(snapshot) = capture else {
                        panic!("{} lost canonical editor snapshot", relative.display());
                    };
                    let merman_core::ParsedEditorFacts::Available(canonical) =
                        snapshot.editor_facts()
                    else {
                        panic!("{} lost canonical editor facts", relative.display());
                    };
                    assert_eq!(
                        canonical,
                        &facts,
                        "{} canonical/full editor facts diverged",
                        relative.display()
                    );
                }
                audit.available += 1;
                let localized_drop_diagnostics = facts
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic
                            .message
                            .contains("editor fact span(s) that crossed a preprocessing edit")
                    })
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>();
                if !localized_drop_diagnostics.is_empty() {
                    audit.localized_drop_documents += 1;
                    localized_drop_fixtures.push(format!(
                        "{} ({}): {}",
                        relative.display(),
                        metadata.diagram_type,
                        localized_drop_diagnostics.join("; ")
                    ));
                }
                assert_editor_fact_spans_belong_to_source(relative, &source, &facts);
            }
            Ok(None) => unavailable.push(format!(
                "{} ({})",
                relative.display(),
                metadata.diagram_type
            )),
            Err(error) => {
                assert!(
                    is_malformed_fixture_error(&error),
                    "{} ({}) failed editor parsing for a non-syntax reason: {error}",
                    relative.display(),
                    metadata.diagram_type
                );
                audit.malformed_editor += 1;
                record_sample(
                    &mut malformed_samples,
                    format!(
                        "{} ({} editor): {error}",
                        relative.display(),
                        metadata.diagram_type
                    ),
                );
            }
        }
    }

    assert_eq!(
        audit.accounted() + unavailable.len(),
        audit.total,
        "every formal fixture must receive exactly one honest outcome"
    );
    assert_eq!(
        audit.supported,
        audit.available + audit.malformed_editor + unavailable.len(),
        "every supported fixture must receive exactly one editor-parser outcome"
    );
    assert!(audit.available > 0, "the corpus must exercise editor facts");
    assert!(
        unavailable.is_empty(),
        "supported fixtures must not lose the whole editor-fact document because one span is unmappable:\n{}",
        unavailable.join("\n")
    );

    eprintln!(
        "formal fixture editor-fact audit: total={}, supported={}, available={}, localized_drops={}, malformed_editor={}, unsupported={} {:?}, malformed_metadata={}{}{}",
        audit.total,
        audit.supported,
        audit.available,
        audit.localized_drop_documents,
        audit.malformed_editor,
        audit.unsupported,
        unsupported_types,
        audit.malformed_metadata,
        if localized_drop_fixtures.is_empty() {
            String::new()
        } else {
            format!(
                "\nlocalized preprocessing drops retained document facts:\n{}",
                localized_drop_fixtures.join("\n")
            )
        },
        if malformed_samples.is_empty() {
            String::new()
        } else {
            format!("\nmalformed samples:\n{}", malformed_samples.join("\n"))
        }
    );
}

#[cfg(feature = "editor-facts")]
fn is_malformed_fixture_error(error: &Error) -> bool {
    matches!(
        error,
        Error::DiagramParse { .. }
            | Error::MalformedFrontMatter
            | Error::InvalidDirectiveJson { .. }
            | Error::InvalidFrontMatterYaml { .. }
    )
}

#[cfg(feature = "editor-facts")]
fn record_sample(samples: &mut Vec<String>, sample: String) {
    const SAMPLE_LIMIT: usize = 8;
    if samples.len() < SAMPLE_LIMIT {
        samples.push(sample);
    }
}

#[cfg(feature = "editor-facts")]
fn assert_editor_fact_spans_belong_to_source(
    fixture: &Path,
    source: &str,
    facts: &EditorSemanticFacts,
) {
    let fixture = fixture.display().to_string();
    for symbol in &facts.symbols {
        assert_source_span(&fixture, source, "symbol", symbol.span);
        assert_source_span(&fixture, source, "symbol selection", symbol.selection);
    }
    for diagnostic in &facts.diagnostics {
        if let Some(span) = diagnostic.span {
            assert_source_span(&fixture, source, "diagnostic", span);
        }
    }
    for expected in &facts.expected_syntax {
        assert_source_span(&fixture, source, "expected syntax", expected.span);
    }
}

#[cfg(feature = "editor-facts")]
fn assert_source_span(fixture: &str, source: &str, owner: &str, span: SourceSpan) {
    assert!(
        span.start <= span.end,
        "{fixture}: {owner} span is reversed: {span:?}"
    );
    assert!(
        span.end <= source.len(),
        "{fixture}: {owner} span is out of bounds: {span:?}"
    );
    assert!(
        source.is_char_boundary(span.start) && source.is_char_boundary(span.end),
        "{fixture}: {owner} span splits a UTF-8 code point: {span:?}"
    );
}

#[test]
fn flowchart_percent_spans_survive_unicode_frontmatter_directives_comments_and_crlf() {
    use merman_core::ParseOptions;
    #[cfg(feature = "editor-facts")]
    use merman_core::{DiagramParseOutcome, EditorSemanticCompleteness, ParsedEditorFacts};

    let prefix = concat!(
        "---\r\n",
        "title: 中文😀\r\n",
        "---\r\n",
        "%%{init: {\"flowchart\":{\"htmlLabels\":false}}}%%\r\n",
        "%% deleted 中文😀\r\n",
        "flowchart TD\r\n",
    );
    let engine = Engine::new();
    // Both complete sources were observed through the pinned Mermaid public oracle. The same
    // original-byte boundaries apply to successful symbols and to the retained failure facts.
    for (body, should_parse, names) in [
        ("雪%-->B%%tail", true, ["雪%", "B%%tail"]),
        ("雪%-->B %% comment", false, ["雪%", "B"]),
    ] {
        let source = format!("{prefix}{body}\r\n");
        for known_type in [false, true] {
            #[cfg(feature = "editor-facts")]
            let snapshot = if known_type {
                engine.parse_diagram_snapshot_with_type_sync("flowchart-v2", &source)
            } else {
                engine.parse_diagram_snapshot_sync(&source)
            }
            .unwrap()
            .expect("Flowchart snapshot");
            #[cfg(feature = "editor-facts")]
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("percent sources must retain their editor facts");
            };
            #[cfg(feature = "editor-facts")]
            assert_editor_fact_spans_belong_to_source(
                Path::new("flowchart-percent-original-source.mmd"),
                &source,
                facts,
            );
            #[cfg(feature = "editor-facts")]
            assert!(
                facts
                    .directive_prefixes
                    .iter()
                    .any(|prefix| prefix == "init")
            );
            #[cfg(feature = "editor-facts")]
            for name in names {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name)
                    .unwrap_or_else(|| panic!("missing percent source symbol {name}"));
                let start = source.rfind(name).unwrap();
                let expected = SourceSpan::new(start, start + name.len());
                assert_eq!(symbol.selection, expected);
                assert_eq!(symbol.span, expected);
                assert_eq!(&source[expected.start..expected.end], name);
            }

            let parsed = if known_type {
                engine.parse_diagram_with_type_sync("flowchart-v2", &source, ParseOptions::strict())
            } else {
                engine.parse_diagram_sync(&source, ParseOptions::strict())
            };
            if should_parse {
                #[cfg(feature = "editor-facts")]
                assert!(snapshot.outcome().parsed_model().is_some());
                #[cfg(feature = "editor-facts")]
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
                let parsed = parsed.unwrap().unwrap();
                assert_eq!(parsed.model["nodes"][0]["id"], names[0]);
                assert_eq!(parsed.model["edges"][0]["to"], "B%%tail");
            } else {
                let marker_start = source.rfind("%% comment").unwrap();
                let expected = SourceSpan::new(marker_start, marker_start + "%%".len());
                #[cfg(feature = "editor-facts")]
                let DiagramParseOutcome::Failed(Error::DiagramParse { diagnostic, .. }) =
                    snapshot.outcome()
                else {
                    panic!("inline percent source must retain its structured parse failure");
                };
                #[cfg(feature = "editor-facts")]
                assert_eq!(diagnostic.span(), Some(expected));
                let Error::DiagramParse { diagnostic, .. } = parsed.unwrap_err() else {
                    panic!("strict JSON parsing must retain its original diagnostic");
                };
                assert_eq!(diagnostic.span(), Some(expected));
                #[cfg(feature = "editor-facts")]
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                #[cfg(feature = "editor-facts")]
                assert!(
                    facts
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.span == Some(expected))
                );
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn flowchart_invalid_header_direction_recovers_complete_value_and_following_nodes() {
    use merman_core::{
        DiagramParseOutcome, EditorExpectedSyntaxKind, EditorSemanticCompleteness,
        ParsedEditorFacts,
    };

    let prefix = concat!(
        "---\r\n",
        "title: 中文😀\r\n",
        "---\r\n",
        "%% deleted 中文😀\r\n",
    );
    let engine = Engine::new();
    for (header, direction_value, value_start) in [
        ("flowchart LR%", "LR%", 10),
        ("flowchart 错误%", "错误%", 10),
        ("flowchart LR% extra", "LR%", 10),
        ("flowchart", "", 9),
        ("flowchart ", "", 10),
    ] {
        for separator in ["\r\n", ";", ""] {
            if direction_value.is_empty() && separator != ";" {
                continue;
            }
            let body = if separator.is_empty() {
                ""
            } else {
                "雪%-->B\r\n"
            };
            let source = format!("{prefix}{header}{separator}{body}");
            for known_type in [false, true] {
                let snapshot = if known_type {
                    engine.parse_diagram_snapshot_with_type_sync("flowchart-v2", &source)
                } else {
                    engine.parse_diagram_snapshot_sync(&source)
                }
                .unwrap()
                .expect("invalid header retains a Flowchart snapshot");
                assert!(matches!(snapshot.outcome(), DiagramParseOutcome::Failed(_)));
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("invalid header must retain editor recovery facts");
                };
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert_editor_fact_spans_belong_to_source(
                    Path::new("flowchart-invalid-header-original-source.mmd"),
                    &source,
                    facts,
                );

                let start = prefix.len() + value_start;
                let expected = SourceSpan::new(start, start + direction_value.len());
                let directions = facts
                    .expected_syntax
                    .iter()
                    .filter(|expected| {
                        expected.kind == EditorExpectedSyntaxKind::FlowchartDirectionValue
                    })
                    .map(|expected| expected.span)
                    .collect::<Vec<_>>();
                assert_eq!(directions, [expected], "{source}");
                assert_eq!(&source[expected.start..expected.end], direction_value);

                let names = facts
                    .symbols
                    .iter()
                    .map(|symbol| symbol.name.as_str())
                    .collect::<Vec<_>>();
                let expected_names: &[&str] = if body.is_empty() { &[] } else { &["雪%", "B"] };
                assert_eq!(names, expected_names, "header tail is not a node: {source}");
                for symbol in &facts.symbols {
                    let start = source.rfind(&symbol.name).unwrap();
                    let expected = SourceSpan::new(start, start + symbol.name.len());
                    assert_eq!(symbol.selection, expected);
                    assert_eq!(symbol.span, expected);
                }
            }
        }
    }
}
