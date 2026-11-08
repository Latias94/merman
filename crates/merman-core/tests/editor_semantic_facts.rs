#![cfg(all(feature = "all-diagrams", feature = "compat-json"))]

#[cfg(feature = "editor-facts")]
use merman_core::presentation::SourcePresentationConfig;
#[cfg(feature = "editor-facts")]
use merman_core::{EditorSemanticFacts, diagram_family_capabilities};
#[cfg(feature = "editor-facts")]
use merman_fixture_render_context::RenderContextCatalog;
#[cfg(feature = "editor-facts")]
use merman_model::time::CivilDate;
#[cfg(feature = "editor-facts")]
use std::collections::BTreeMap;
#[cfg(feature = "editor-facts")]
use std::fs;
#[cfg(feature = "editor-facts")]
use std::path::{Path, PathBuf};
#[cfg(feature = "editor-facts")]
use std::sync::OnceLock;
use {
    merman_core::{Error, MermaidEngine},
    merman_model::source::SourceSpan,
};

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
fn fixture_site_config_for_path(path: &Path) -> Option<SourcePresentationConfig> {
    fixture_render_contexts()
        .context_for_fixture(path)
        .unwrap_or_else(|error| {
            panic!(
                "invalid fixture render context lookup for {}: {error}",
                path.display()
            )
        })
        .map(|context| SourcePresentationConfig::from_value(context.site_config_value()))
}

#[cfg(feature = "editor-facts")]
fn engine_for_fixture(base: &MermaidEngine, path: &Path) -> MermaidEngine {
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
    let base = MermaidEngine::new()
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
                    let merman_core::SemanticSnapshotCapture::Snapshot(recovery) = parser
                        .capture_diagram_recovery_snapshot_controlled_sync(
                            &source,
                            &merman_model::operation::OperationControl::new(),
                        )
                        .expect("fixture recovery capture must not be cancelled")
                    else {
                        panic!("{} lost its recovery snapshot", relative.display());
                    };
                    let merman_core::ParsedEditorFacts::Available(recovery_facts) =
                        recovery.editor_facts()
                    else {
                        panic!("{} lost recovery facts", relative.display());
                    };
                    assert_eq!(
                        recovery_facts.completeness,
                        canonical.completeness,
                        "{}",
                        relative.display()
                    );
                    assert_eq!(
                        recovery_facts.family_semantics,
                        canonical.family_semantics,
                        "{}",
                        relative.display()
                    );
                    assert_eq!(
                        recovery_facts.diagnostics,
                        canonical.diagnostics,
                        "{} recovery diagnostics diverged",
                        relative.display()
                    );
                    assert_eq!(
                        recovery.source_config(),
                        snapshot.source_config(),
                        "{}",
                        relative.display()
                    );
                    assert!(recovery_facts.symbols.is_empty(), "{}", relative.display());
                    assert!(
                        recovery_facts.directive_prefixes.is_empty(),
                        "{}",
                        relative.display()
                    );
                    assert!(
                        recovery_facts.expected_syntax.is_empty(),
                        "{}",
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
    let engine = MermaidEngine::new();
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
    let engine = MermaidEngine::new();
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

#[cfg(feature = "editor-facts")]
#[test]
fn packet_and_treemap_multiline_payloads_retain_original_source_selections() {
    use merman_core::{EditorSemanticCompleteness, ParseOptions, ParsedEditorFacts};

    let engine = MermaidEngine::new();
    for newline in ["\n", "\r\n"] {
        for quote in ['\'', '"'] {
            let raw_name = format!("雪{newline}😀");
            let prefix = format!(
                "---{newline}title: 中文😀{newline}---{newline}%%{{init: {{\"theme\": \"default\"}}}}%%{newline}%% removed 中文😀{newline}"
            );
            for (family, statement, tail) in [
                (
                    "packet",
                    format!("0: {quote}{raw_name}{quote}"),
                    "1: \"Tail\"",
                ),
                (
                    "treemap",
                    format!("{quote}{raw_name}{quote}: .00"),
                    "\"Tail\": 1",
                ),
            ] {
                let source =
                    format!("{prefix}{family}{newline}{statement}{newline}{tail}{newline}");
                for known_type in [false, true] {
                    let snapshot = if known_type {
                        engine.parse_diagram_snapshot_with_type_sync(family, &source)
                    } else {
                        engine.parse_diagram_snapshot_sync(&source)
                    }
                    .unwrap()
                    .expect("multiline source retains its family");
                    assert!(snapshot.outcome().parsed_model().is_some(), "{source}");
                    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                        panic!("multiline payload requires parser-derived facts");
                    };
                    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
                    assert!(facts.diagnostics.is_empty());
                    assert_editor_fact_spans_belong_to_source(Path::new(family), &source, facts);
                    for (semantic_name, raw_text) in
                        [("雪\n😀", raw_name.as_str()), ("Tail", "Tail")]
                    {
                        let symbol = facts
                            .symbols
                            .iter()
                            .find(|symbol| symbol.name == semantic_name)
                            .unwrap();
                        let start = source.rfind(raw_text).unwrap();
                        assert_eq!(
                            symbol.selection,
                            SourceSpan::new(start, start + raw_text.len())
                        );
                        assert_eq!(
                            &source[symbol.selection.start..symbol.selection.end],
                            raw_text
                        );
                    }
                    if family == "treemap" {
                        let value = facts
                            .symbols
                            .iter()
                            .find(|symbol| symbol.name == ".00")
                            .unwrap();
                        let start = source.find(".00").unwrap();
                        assert_eq!(value.selection, SourceSpan::new(start, start + 3));
                    }
                    let parsed = if known_type {
                        engine.parse_diagram_with_type_sync(family, &source, ParseOptions::strict())
                    } else {
                        engine.parse_diagram_sync(&source, ParseOptions::strict())
                    }
                    .unwrap()
                    .unwrap();
                    if family == "packet" {
                        assert_eq!(parsed.model["packet"][0][0]["label"], "雪\n😀");
                        assert_eq!(parsed.model["packet"][0][1]["label"], "Tail");
                    } else {
                        assert_eq!(parsed.model["nodes"][0]["name"], "雪\n😀");
                        assert_eq!(parsed.model["nodes"][0]["value"].as_f64(), Some(0.0));
                        assert_eq!(parsed.model["nodes"][1]["name"], "Tail");
                    }
                }
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn quadrant_multiline_text_and_literal_title_tail_keep_original_source_ranges() {
    use merman_core::{EditorSemanticCompleteness, ParseOptions, ParsedEditorFacts};

    let engine = MermaidEngine::new();
    for newline in ["\n", "\r\n"] {
        for markdown in [false, true] {
            let label = format!("雪{newline}😀");
            let point = format!("点{newline}名");
            let wrap = |value: &str| {
                if markdown {
                    format!("\"`{value}`\"")
                } else {
                    format!("\"{value}\"")
                }
            };
            let source = format!(
                "---{newline}title: 中文😀{newline}---{newline}%%{{init: {{\"theme\": \"default\"}}}}%%{newline}%% removed 中文😀{newline}quadrantChart{newline}title Plan; quadrant-1 Hidden{newline}quadrant-1 {}{newline}{}: [0.25, 0.75]{newline}",
                wrap(&label),
                wrap(&point),
            );
            for known_type in [false, true] {
                let snapshot = if known_type {
                    engine.parse_diagram_snapshot_with_type_sync("quadrantChart", &source)
                } else {
                    engine.parse_diagram_snapshot_sync(&source)
                }
                .unwrap()
                .unwrap();
                assert!(snapshot.outcome().parsed_model().is_some(), "{source}");
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("Quadrant Chart must expose parser-derived facts");
                };
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
                assert!(facts.diagnostics.is_empty());
                assert_editor_fact_spans_belong_to_source(
                    Path::new("quadrant-multiline"),
                    &source,
                    facts,
                );
                for (name, raw) in [
                    ("Plan; quadrant-1 Hidden", "Plan; quadrant-1 Hidden"),
                    ("雪\n😀", label.as_str()),
                    ("点\n名", point.as_str()),
                ] {
                    let symbol = facts
                        .symbols
                        .iter()
                        .find(|symbol| symbol.name == name)
                        .unwrap();
                    let start = source.find(raw).unwrap();
                    assert_eq!(symbol.selection, SourceSpan::new(start, start + raw.len()));
                    assert_eq!(&source[symbol.selection.start..symbol.selection.end], raw);
                }
                assert!(!facts.symbols.iter().any(|symbol| symbol.name == "Hidden"));
                let parsed = if known_type {
                    engine.parse_diagram_with_type_sync(
                        "quadrantChart",
                        &source,
                        ParseOptions::strict(),
                    )
                } else {
                    engine.parse_diagram_sync(&source, ParseOptions::strict())
                }
                .unwrap()
                .unwrap();
                assert_eq!(parsed.model["title"], "Plan; quadrant-1 Hidden");
                assert_eq!(parsed.model["quadrants"]["quadrant1Text"], "雪\n😀");
                assert_eq!(parsed.model["points"][0]["text"], "点\n名");
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn xychart_reserved_category_diagnostics_map_to_original_crlf_source() {
    use merman_core::{EditorSemanticCompleteness, ParseOptions, ParsedEditorFacts};

    let prefix = "---\r\ntitle: 中文😀\r\n---\r\n%% removed 中文😀\r\nxychart\r\n";
    let engine = MermaidEngine::new();
    for quoted in [false, true] {
        let category = if quoted { "\"bar\"" } else { "bar" };
        let source = format!("{prefix}x-axis [{category}, 雪]\r\nline [1, 2]\r\n");
        let start = source.find(category).unwrap() + usize::from(quoted);
        for known_type in [false, true] {
            let snapshot = if known_type {
                engine.parse_diagram_snapshot_with_type_sync("xychart", &source)
            } else {
                engine.parse_diagram_snapshot_sync(&source)
            }
            .unwrap()
            .unwrap();
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("XY Chart must expose parser-derived facts");
            };
            assert_editor_fact_spans_belong_to_source(
                Path::new("xychart-reserved-category"),
                &source,
                facts,
            );
            let parsed = if known_type {
                engine.parse_diagram_with_type_sync("xychart", &source, ParseOptions::strict())
            } else {
                engine.parse_diagram_sync(&source, ParseOptions::strict())
            };
            if quoted {
                assert!(snapshot.outcome().parsed_model().is_some());
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
                let payload = format!("{category}, 雪");
                let payload_start = source.find(&payload).unwrap();
                assert!(facts.expected_syntax.iter().any(|expected| {
                    expected.span == SourceSpan::new(payload_start, payload_start + payload.len())
                }));
                assert_eq!(
                    parsed.unwrap().unwrap().model["xAxis"]["categories"][0],
                    "bar"
                );
            } else {
                assert!(parsed.is_err());
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(
                    facts.diagnostics.iter().any(
                        |diagnostic| diagnostic.span == Some(SourceSpan::new(start, start + 3))
                    )
                );
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn quadrant_unclosed_multiline_text_preserves_later_original_source_facts() {
    use merman_core::{EditorSemanticCompleteness, ParseOptions, ParsedEditorFacts};

    let engine = MermaidEngine::new();
    for newline in ["\n", "\r\n"] {
        let source = format!(
            "---{newline}title: 中文😀{newline}---{newline}%% removed 中文😀{newline}quadrantChart{newline}quadrant-1 \"broken{newline}x-axis Low --> High{newline}雪: [0.5, 0.5]{newline}"
        );
        for known_type in [false, true] {
            let snapshot = if known_type {
                engine.parse_diagram_snapshot_with_type_sync("quadrantChart", &source)
            } else {
                engine.parse_diagram_snapshot_sync(&source)
            }
            .unwrap()
            .unwrap();
            let strict = if known_type {
                engine.parse_diagram_with_type_sync(
                    "quadrantChart",
                    &source,
                    ParseOptions::strict(),
                )
            } else {
                engine.parse_diagram_sync(&source, ParseOptions::strict())
            };
            assert!(strict.is_err());
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("malformed Quadrant source must retain recovery facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(!facts.diagnostics.is_empty());
            assert_editor_fact_spans_belong_to_source(
                Path::new("quadrant-unclosed-recovery"),
                &source,
                facts,
            );
            for name in ["Low", "High", "雪"] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name)
                    .unwrap();
                let start = source.rfind(name).unwrap();
                assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn sankey_csv_quote_errors_and_recovery_keep_original_source_ranges() {
    use merman_core::{EditorSemanticCompleteness, ParseOptions, ParsedEditorFacts};

    let engine = MermaidEngine::new();
    for newline in ["\n", "\r\n"] {
        for quoted in [false, true] {
            let field = if quoted { "\"A\"\"B\"" } else { "A\"B" };
            let source = format!(
                "---{newline}title: 中文😀{newline}---{newline}sankey{newline}Before,Start,1{newline}{field},C,1{newline}Next,End,2{newline}"
            );
            for known_type in [false, true] {
                let snapshot = if known_type {
                    engine.parse_diagram_snapshot_with_type_sync("sankey", &source)
                } else {
                    engine.parse_diagram_snapshot_sync(&source)
                }
                .unwrap()
                .unwrap();
                let strict = if known_type {
                    engine.parse_diagram_with_type_sync("sankey", &source, ParseOptions::strict())
                } else {
                    engine.parse_diagram_sync(&source, ParseOptions::strict())
                };
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("Sankey CSV must expose parser-derived recovery facts");
                };
                assert_editor_fact_spans_belong_to_source(
                    Path::new("sankey-csv-quote-recovery"),
                    &source,
                    facts,
                );
                for name in ["Before", "Start", "Next", "End"] {
                    let symbol = facts
                        .symbols
                        .iter()
                        .find(|symbol| symbol.name == name)
                        .unwrap();
                    let start = source.find(name).unwrap();
                    assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
                }
                let start = source.find(field).unwrap();
                if quoted {
                    let parsed = strict.unwrap().unwrap();
                    assert_eq!(parsed.model["graph"]["nodes"][2]["id"], "A\"B");
                    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
                    let symbol = facts
                        .symbols
                        .iter()
                        .find(|symbol| symbol.name == "A\"B")
                        .unwrap();
                    assert_eq!(symbol.selection, SourceSpan::new(start + 1, start + 5));
                    assert_eq!(
                        &source[symbol.selection.start..symbol.selection.end],
                        "A\"\"B"
                    );
                } else {
                    assert!(strict.is_err());
                    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                    assert!(facts.diagnostics.iter().any(|diagnostic| {
                        diagnostic.span == Some(SourceSpan::new(start + 1, start + 2))
                    }));
                    assert!(!facts.symbols.iter().any(|symbol| symbol.name == "A\"B"));
                }
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn flowchart_group_membership_keeps_authored_editor_source_ranges() {
    use merman_core::{EditorSemanticCompleteness, ParseOptions, ParsedEditorFacts};

    let engine = MermaidEngine::new();
    for newline in ["\n", "\r\n"] {
        let source = format!(
            "---{newline}title: 中文😀{newline}---{newline}%% removed 中文😀{newline}flowchart TD{newline}subgraph S{newline}雪 & B --> C & D{newline}end{newline}"
        );
        for known_type in [false, true] {
            let snapshot = if known_type {
                engine.parse_diagram_snapshot_with_type_sync("flowchart-v2", &source)
            } else {
                engine.parse_diagram_snapshot_sync(&source)
            }
            .unwrap()
            .unwrap();
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("Flowchart groups must retain parser-derived source facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert_editor_fact_spans_belong_to_source(
                Path::new("flowchart-group-membership"),
                &source,
                facts,
            );
            for name in ["雪", "B", "C", "D"] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name)
                    .unwrap();
                let start = source.rfind(name).unwrap();
                assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
            }
            let parsed = if known_type {
                engine.parse_diagram_with_type_sync("flowchart-v2", &source, ParseOptions::strict())
            } else {
                engine.parse_diagram_sync(&source, ParseOptions::strict())
            }
            .unwrap()
            .unwrap();
            assert_eq!(
                parsed.model["subgraphs"][0]["nodes"],
                serde_json::json!(["C", "D", "雪", "B"])
            );
        }
    }
}
