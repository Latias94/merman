use merman_core as parse_api;
use merman_core::{Error, ParseOptions, diagram_family_capabilities};
use std::path::PathBuf;

#[path = "support/parse_equivalence.rs"]
mod parse_equivalence;
use parse_equivalence::{
    assert_known_type_parse_paths, assert_pair, assert_parse_paths, deterministic_engine,
};

#[cfg(feature = "diagram-flowchart")]
#[test]
#[should_panic(expected = "different parser acceptance")]
fn shared_assertion_rejects_a_single_path_acceptance_change() {
    let engine = deterministic_engine();
    let source = "flowchart TD\nA --> B\n";
    let semantic = engine.parse_diagram_sync(source, ParseOptions::strict());
    let typed =
        engine.parse_diagram_for_render_model_sync("flowchart TD\nA -->", ParseOptions::strict());
    assert_pair(&semantic, &typed, source);
}

#[cfg(feature = "diagram-flowchart")]
#[test]
#[should_panic(expected = "complete semantic projection")]
fn shared_assertion_rejects_different_real_edge_endpoints_with_the_same_metadata() {
    let engine = deterministic_engine();
    let source = "flowchart TD\nA --> B\n";
    let semantic = engine.parse_diagram_sync(source, ParseOptions::strict());
    let typed = engine
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> C\n", ParseOptions::strict());
    assert_pair(&semantic, &typed, source);
}

fn fixture_for(diagram_type: &str, family: &str) -> &'static str {
    match diagram_type {
        "railroadEbnf" => return "railroadEbnf/choice_optional_repetition.mmd",
        "railroadAbnf" => return "railroadAbnf/local_numeric_groups_bounds.mmd",
        "railroadPeg" => return "railroadPeg/local_predicate_group_suffix.mmd",
        _ => {}
    }
    match family {
        "error" => "error/basic.mmd",
        "flowchart" => "flowchart/basic.mmd",
        "swimlane" => "swimlane/basic_flowchart_reuse.mmd",
        "mindmap" => "mindmap/basic.mmd",
        "architecture" => "architecture/upstream_cypress_architecture_spec_example_001.mmd",
        "zenuml" => "zenuml/basic.mmd",
        "sequence" => "sequence/activation_explicit.mmd",
        "c4" => "c4/basic.mmd",
        "kanban" => "kanban/basic.mmd",
        "class" => "class/basic.mmd",
        "er" => "er/basic.mmd",
        "gantt" => "gantt/basic.mmd",
        "info" => {
            "info/upstream_cypress_statediagram_v2_spec_v2_should_render_a_simple_info_001.mmd"
        }
        "pie" => "pie/upstream_pie_simple_spec.mmd",
        "requirement" => "requirement/basic.mmd",
        "timeline" => "timeline/upstream_cypress_timeline_spec_example_001.mmd",
        "gitGraph" => "gitgraph/upstream_examples_git_basic_git_flow_001.mmd",
        "state" => "state/basic.mmd",
        "journey" => "journey/upstream_cypress_journey_spec_example_001.mmd",
        "quadrantChart" => "quadrantchart/upstream_cypress_quadrantchart_spec_example_001.mmd",
        "sankey" => "sankey/upstream_cypress_sankey_spec_example_001.mmd",
        "packet" => "packet/upstream_cypress_packet_spec_example_001.mmd",
        "xychart" => "xychart/upstream_docs_xychart_example_001.mmd",
        "block" => "block/upstream_basic_nodes.mmd",
        "eventmodeling" => "eventmodeling/upstream_docs_eventmodeling_minimum.mmd",
        "treeView" => "treeView/upstream_docs_treeview_basic.mmd",
        "radar" => "radar/upstream_radar_simple_spec.mmd",
        "ishikawa" => "ishikawa/upstream_docs_ishikawa_basic.mmd",
        "treemap" => "treemap/upstream_treemap_docs_basic_spec.mmd",
        "railroad" => "railroad/basic_ir.mmd",
        "venn" => "venn/upstream_cypress_venn_handdrawn_custom_styles_018.mmd",
        "wardley" => "wardley/upstream_cypress_wardley_spec_1_should_render_tea_shop_001.mmd",
        "cynefin" => "cynefin/basic_domains_transitions.mmd",
        "usecase" => "usecase/basic.mmd",
        "agentflow" => "agentflow/basic.mmd",
        family => panic!("add a legal projection sample for catalog family {family}"),
    }
}

#[test]
fn every_catalog_family_and_alias_has_paired_lossless_projection_or_unavailable_identity() {
    let engine = deterministic_engine();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for fact in diagram_family_capabilities() {
        let relative = fixture_for(fact.diagram_type, fact.logical_family_kind);
        let source =
            std::fs::read_to_string(fixtures.join(relative)).expect("read legal family sample");
        for options in [ParseOptions::strict(), ParseOptions::lenient()] {
            let semantic = engine.parse_diagram_with_type_sync(fact.diagram_type, &source, options);
            let typed = engine.parse_diagram_for_render_model_with_type_sync(
                fact.diagram_type,
                &source,
                options,
            );
            assert_pair(&semantic, &typed, &source);
            assert_eq!(
                fact.has_semantic_parser, fact.has_render_parser,
                "{}",
                fact.diagram_type
            );
            if fact.has_semantic_parser {
                let typed = typed
                    .unwrap_or_else(|error| panic!("{relative}, {}: {error}", fact.diagram_type))
                    .expect("legal sample is detected");
                assert_eq!(typed.metadata().diagram_type, fact.diagram_type);
                assert_eq!(
                    Some(typed.model().kind()),
                    fact.render_model_kind,
                    "{relative}"
                );
            } else if !options.suppress_errors {
                assert!(
                    matches!(typed, Err(Error::UnsupportedDiagram { diagram_type }) if diagram_type == fact.diagram_type),
                    "{relative}"
                );
            } else {
                assert_eq!(typed.unwrap().unwrap().metadata().diagram_type, "error");
            }
        }
        assert_parse_paths(&engine, &source);
    }
}

#[test]
fn authored_configuration_labels_accessibility_links_styles_and_warnings_are_lossless() {
    let engine = deterministic_engine();
    let sources = [
        "---\ntitle: '标题 & <b>Title</b>'\nconfig:\n  htmlLabels: false\n  flowchart:\n    nodeSpacing: 73\n---\n%% comment removed before parser\nflowchart\naccTitle: Accessible & title\naccDescr {first line\nsecond line}\nA[\"Unicode 边 &amp; label\"] -->|first| B[End]\nA -->|second| B\nclassDef note fill:#ffeecc,stroke:#123456\nclass A note\nstyle B color:#334455\nlinkStyle 1 stroke:#ff0000\nclick A href \"https://example.com/path?q=1\" \"Help\" _blank\n",
        "%%{init: {\"flowchart\": {\"defaultRenderer\": \"dagre-d3\"}}}%%\ngraph RL\nA[One] --> B[Two]",
        "flowchart-elk LR\nA[First] --> B[Second]\nA --> B\n",
        "---\r\ntitle: CRLF\r\n---\r\n%% comment\r\nsequenceDiagram\r\naccTitle: Actors\r\nparticipant A as 用户\r\nparticipant B as Client\r\nA->>B: a &amp; b\r\nNote over A,B: retained\r\n",
        "---\ntitle: Warning positions\n---\n%% removed\nagentflow-beta\nA@{ shape: cylinder, label: \"Data\" }\nA --> B\n",
        "flowchart TD\nA[\"<b>bold &amp; 安全</b>\"] -->|\"<i>edge</i>\"| B\n",
    ];
    for source in sources {
        assert_parse_paths(&engine, source);
    }
    #[cfg(feature = "diagram-flowchart")]
    {
        let parsed = engine
            .parse_diagram_sync(sources[0], ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.model["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(parsed.model["edges"].as_array().unwrap().len(), 2);
        assert!(!parsed.model["warningFacts"].as_array().unwrap().is_empty());
        assert_eq!(
            parsed.meta.effective_config.as_value()["flowchart"]["nodeSpacing"],
            73
        );
    }
}

#[test]
fn strict_and_lenient_failures_retain_their_actual_preprocessing_boundaries() {
    let engine = deterministic_engine();
    for source in [
        "",
        "not-a-diagram\n",
        "---\ntitle: unfinished\nflowchart TD\nA-->B",
        "---\nconfig: [broken\n---\nflowchart TD\nA-->B\n",
        "%%{init: {\"flowchart\": \nflowchart TD\nA-->B\n",
        "%%{init: {broken}}%%\nflowchart TD\nA-->B\n",
        "flowchart TD\nA -->",
        "sequenceDiagram\nparticipant",
        "stateDiagram-v2\nstate {",
        "classDiagram\nclass A {\n",
        "info\ninvalid statement",
    ] {
        assert_parse_paths(&engine, source);
    }
    for source in ["", "not-a-diagram\n"] {
        assert!(matches!(
            engine.parse_diagram_sync(source, ParseOptions::strict()),
            Err(Error::DetectType(_))
        ));
        assert!(
            engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::lenient())
                .unwrap()
                .is_none()
        );
    }
    let malformed = "---\ntitle: unfinished\nflowchart TD\nA-->B";
    assert!(matches!(
        engine.parse_diagram_sync(malformed, ParseOptions::lenient()),
        Err(Error::MalformedFrontMatter)
    ));
    let recoverable = "%%{init: {\"flowchart\": \nflowchart TD\nA-->B\n";
    assert!(matches!(
        engine.parse_diagram_sync(recoverable, ParseOptions::strict()),
        Err(Error::DetectType(_))
    ));
    let recovered = engine
        .parse_diagram_sync(recoverable, ParseOptions::lenient())
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.meta.diagram_type,
        if cfg!(feature = "diagram-flowchart") {
            "flowchart-v2"
        } else {
            "error"
        }
    );
}

#[test]
fn checked_in_parse_fuzz_seeds_use_the_same_complete_assertions() {
    let engine = deterministic_engine();
    let seeds = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds/mermaid");
    let mut entries = std::fs::read_dir(seeds)
        .expect("parse seed corpus")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    entries.sort();
    assert!(!entries.is_empty());
    for path in entries {
        let bytes = std::fs::read(&path).expect("read parse seed");
        if let Ok(source) = std::str::from_utf8(&bytes) {
            assert_parse_paths(&engine, source);
        }
    }
}

#[test]
fn known_type_invalid_sources_share_model_and_snapshot_acceptance() {
    let engine = deterministic_engine();
    for family in diagram_family_capabilities() {
        for source in ["", "not-a-diagram", "piep", "packet", "\"雪\": -1\n"] {
            assert_known_type_parse_paths(&engine, family.diagram_type, source);
        }
    }
}

#[test]
fn malformed_family_headers_are_failures_instead_of_successful_error_json() {
    let engine = deterministic_engine();
    for (diagram_type, invalid) in [("pie", "piep"), ("info", "infox")] {
        let capability = diagram_family_capabilities()
            .iter()
            .find(|capability| capability.diagram_type == diagram_type)
            .unwrap();
        if !capability.has_semantic_parser {
            continue;
        }
        for source in [invalid, "packet"] {
            assert!(
                matches!(
                    engine.parse_diagram_with_type_sync(
                        diagram_type,
                        source,
                        ParseOptions::strict(),
                    ),
                    Err(Error::DiagramParse { .. })
                ),
                "{diagram_type}: {source}"
            );
            let lenient = engine
                .parse_diagram_with_type_sync(diagram_type, source, ParseOptions::lenient())
                .unwrap()
                .unwrap();
            assert_eq!(lenient.meta.diagram_type, "error");
            let snapshot = engine
                .parse_diagram_snapshot_with_type_sync(diagram_type, source)
                .unwrap()
                .unwrap();
            assert!(matches!(
                snapshot.outcome(),
                merman_core::DiagramParseOutcome::Failed(_)
            ));
            let merman_core::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("failed {diagram_type} parse lost editor facts");
            };
            assert_eq!(
                facts.completeness,
                merman_core::EditorSemanticCompleteness::Recovered
            );
            assert!(!facts.diagnostics.is_empty());
        }
    }
}

#[cfg(feature = "diagram-packet")]
#[test]
fn failed_snapshot_retains_parser_symbols_and_original_source_spans() {
    let source = "packet\r\n0-7: \"valid\"\r\n  9-15: \"not contiguous\"  \r\n";
    let snapshot = deterministic_engine()
        .parse_diagram_snapshot_sync(source)
        .unwrap()
        .unwrap();
    assert!(matches!(
        snapshot.outcome(),
        merman_core::DiagramParseOutcome::Failed(_)
    ));
    let merman_core::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("failed packet parse lost editor facts");
    };
    assert_eq!(
        facts.completeness,
        merman_core::EditorSemanticCompleteness::Recovered
    );
    for name in ["valid", "not contiguous"] {
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap();
        assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
    }
    assert!(
        facts
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.span.is_some())
    );
}
