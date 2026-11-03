#![cfg(feature = "compat-json")]

use merman_core as parse_api;
use merman_core::{Error, ParseOptions, diagram_family_capabilities};
use std::path::PathBuf;

#[path = "support/parse_equivalence.rs"]
mod parse_equivalence;
use parse_equivalence::{
    assert_known_type_parse_paths, assert_pair, assert_parse_paths, deterministic_engine,
};

#[cfg(any(
    feature = "diagram-railroad",
    feature = "diagram-cynefin",
    feature = "diagram-wardley"
))]
#[test]
fn family_sanitization_has_independent_single_pass_results_across_projections() {
    use merman_core::SemanticParser;
    use merman_model::config::MermaidConfig;
    use merman_model::operation::OperationControl;
    use merman_model::semantic::SemanticDiagramModel;
    use serde_json::{Value, json};

    let families = [
        #[cfg(feature = "diagram-railroad")]
        ("railroad", "railroad-beta"),
        #[cfg(feature = "diagram-railroad")]
        ("railroadEbnf", "railroad-ebnf-beta"),
        #[cfg(feature = "diagram-railroad")]
        ("railroadAbnf", "railroad-abnf-beta"),
        #[cfg(feature = "diagram-railroad")]
        ("railroadPeg", "railroad-peg-beta"),
        #[cfg(feature = "diagram-cynefin")]
        ("cynefin", "cynefin-beta"),
        #[cfg(feature = "diagram-wardley")]
        ("wardley", "wardley-beta"),
    ];
    let cases = [
        (
            "<script>secret</script><b>Safe &amp; &lt;</b>",
            "<b>Safe &amp; &lt;</b>",
            "<b>Safe &amp; &lt;</b>",
            json!({}),
        ),
        (
            "<a href=javascript&colon;alert(1) onclick=alert(1)>A &amp; B</a>",
            "<a>A &amp; B</a>",
            "<a>A &amp; B</a>",
            json!({}),
        ),
        (
            "<b><i>one</b>two</i>",
            "<b><i>one</i></b><i>two</i>",
            "<b><i>one</i></b><i>two</i>",
            json!({}),
        ),
        (
            "Text &amp; &amp;lt; &lt; &bogus;",
            "Text &amp; &amp;lt; &lt; &bogus;",
            "Text &amp; &amp;lt; &lt; &amp;bogus;",
            json!({}),
        ),
        (
            "<p><table><tr><td>value &amp; &lt;</td></tr></table>tail</p>",
            "<p><tbody><tr><td>value &amp; &lt;</td></tr></tbody>tail</p>",
            "<p><tbody><tr><td>value &amp; &lt;</td></tr></tbody>tail</p>",
            json!({"dompurifyConfig": {"FORBID_TAGS": ["table"]}}),
        ),
    ];
    for security_level in ["strict", "loose"] {
        for (payload, expected, expected_acc_title, overrides) in &cases {
            let mut config = overrides.clone();
            config["securityLevel"] = json!(security_level);
            config["htmlLabels"] = json!(true);
            let engine = deterministic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config.clone()),
            );
            let parser = SemanticParser::new().with_site_config(MermaidConfig::from_value(config));
            let context = parser.begin_operation().unwrap();
            for (diagram_type, header) in families {
                let body = match diagram_type {
                    "railroad" => format!(
                        "entry = sequence(optional(terminal(\"{payload}\")), choice(nonterminal(\"{payload}\"), zeroOrMore(special(\"{payload}\")))) ;\r\n"
                    ),
                    "railroadEbnf" | "railroadAbnf" => {
                        format!("entry = [ \"{payload}\" ] ;\r\n")
                    }
                    "railroadPeg" => format!("entry <- \"{payload}\" ;\r\n"),
                    "cynefin" => {
                        format!("complex\r\n\"{payload}\"\r\ncomplex --> clear : \"{payload}\"\r\n")
                    }
                    "wardley" => format!(
                        "component \"{payload}\" [0.5, 0.25]\r\nnote \"{payload}\" [0.4, 0.3]\r\n"
                    ),
                    _ => unreachable!(),
                };
                let source = format!(
                    "\u{feff}---\r\ntitle: '<i>Front &amp; &lt;</i>'\r\n---\r\n%% removed comment\r\n{header}\r\ntitle {payload}\r\naccTitle: <script>secret</script>   {payload}\r\naccDescr {{First\r\n    {payload}\r\n}}\r\n{body}"
                );
                let assert_model = |model: &Value| {
                    assert_eq!(model["title"], *expected, "{security_level}: {source}");
                    assert_eq!(
                        model["accTitle"], *expected_acc_title,
                        "{security_level}: {source}"
                    );
                    assert_eq!(
                        model["accDescr"],
                        format!("First\n{expected}"),
                        "{security_level}: {source}"
                    );
                    match diagram_type {
                        "railroad" => assert_eq!(
                            model["rules"],
                            json!([{"name": "entry", "definition": {
                                "type": "sequence", "elements": [
                                    {"type": "optional", "element": {"type": "terminal", "value": expected}},
                                    {"type": "choice", "alternatives": [
                                        {"type": "nonterminal", "name": expected},
                                        {"type": "repetition", "min": 0, "max": null,
                                         "element": {"type": "special", "text": expected}}
                                    ]}
                                ]
                            }}]),
                            "{security_level}: {source}"
                        ),
                        "railroadEbnf" | "railroadAbnf" => assert_eq!(
                            model["rules"],
                            json!([{"name": "entry", "definition": {
                                "type": "optional", "element": {"type": "terminal", "value": expected}
                            }}]),
                            "{security_level}: {source}"
                        ),
                        "railroadPeg" => assert_eq!(
                            model["rules"],
                            json!([{"name": "entry", "definition": {"type": "terminal", "value": expected}}]),
                            "{security_level}: {source}"
                        ),
                        "cynefin" => {
                            assert_eq!(model["domains"][0]["items"][0]["label"], *payload);
                            assert_eq!(model["transitions"][0]["label"], *payload);
                        }
                        "wardley" => {
                            assert_eq!(model["nodes"][0]["id"], *payload);
                            assert_eq!(model["nodes"][0]["label"], *payload);
                            assert_eq!(model["notes"][0]["text"], *payload);
                        }
                        _ => unreachable!(),
                    }
                };
                let assert_semantic = |model: &SemanticDiagramModel| {
                    let model = match model {
                        #[cfg(feature = "diagram-railroad")]
                        SemanticDiagramModel::Railroad(model) => serde_json::to_value(model),
                        #[cfg(feature = "diagram-cynefin")]
                        SemanticDiagramModel::Cynefin(model) => serde_json::to_value(model),
                        #[cfg(feature = "diagram-wardley")]
                        SemanticDiagramModel::Wardley(model) => serde_json::to_value(model),
                        _ => panic!("expected {diagram_type} model"),
                    }
                    .unwrap();
                    assert_model(&model);
                };
                for options in [ParseOptions::strict(), ParseOptions::lenient()] {
                    let parsed = engine
                        .parse_diagram_sync(&source, options)
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        parsed.meta.title.as_deref(),
                        Some("<i>Front &amp; &lt;</i>")
                    );
                    assert_model(&parsed.model);
                    let typed = engine
                        .parse_diagram_for_render_model_sync(&source, options)
                        .unwrap()
                        .unwrap();
                    assert_model(&typed.compatibility_json().unwrap());
                    let semantic = parser
                        .parse_diagram_with_options_controlled_in_context_sync(
                            &source,
                            options,
                            &OperationControl::new(),
                            &context,
                        )
                        .unwrap()
                        .unwrap()
                        .unwrap();
                    assert_semantic(&semantic.model);
                }
                #[cfg(feature = "editor-facts")]
                {
                    let snapshot = engine
                        .parse_diagram_snapshot_sync(&source)
                        .unwrap()
                        .unwrap();
                    let merman_core::DiagramParseOutcome::Parsed { model, .. } = snapshot.outcome()
                    else {
                        panic!("expected {diagram_type} editor model");
                    };
                    assert_model(model);
                    let merman_core::ParsedEditorFacts::Available(facts) = snapshot.editor_facts()
                    else {
                        panic!("expected {diagram_type} editor facts");
                    };
                    let payload_symbol = facts
                        .symbols
                        .iter()
                        .find(|symbol| {
                            symbol.name == *payload
                                && &source[symbol.selection.start..symbol.selection.end] == *payload
                        })
                        .expect("editor payload and selection retain authored HTML");
                    assert_eq!(payload_symbol.name, *payload);
                    let merman_core::SemanticSnapshotCapture::Snapshot(snapshot) =
                        parser.capture_diagram_snapshot_sync(&source)
                    else {
                        panic!("expected {diagram_type} semantic snapshot");
                    };
                    let merman_core::SemanticParseOutcome::Parsed(model) = snapshot.outcome()
                    else {
                        panic!("expected {diagram_type} semantic model");
                    };
                    assert_semantic(model);
                }
                assert_parse_paths(&engine, &source);
                assert_known_type_parse_paths(&engine, diagram_type, &source);
            }
        }
    }
}

#[cfg(any(
    feature = "diagram-railroad",
    feature = "diagram-cynefin",
    feature = "diagram-wardley"
))]
#[test]
fn custom_overlays_with_family_names_keep_json_sanitization() {
    use merman_core::SourcePresentationMetadata;
    use merman_model::operation::{OperationControl, OperationControlResult};
    use merman_model::presentation::CustomJsonRenderModel;
    use serde_json::{Value, json};

    fn custom_json(
        _code: &str,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<merman_core::Result<Value>> {
        control.checkpoint()?;
        Ok(Ok(json!({
            "type": meta.diagram_type,
            "title": "<script>secret</script><b>Custom &amp; &lt;</b>",
            "accTitle": "<script>secret</script>   Accessible",
            "accDescr": "First\n   <b>Second</b>"
        })))
    }

    fn custom_render(
        code: &str,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<merman_core::Result<CustomJsonRenderModel>> {
        Ok(custom_json(code, meta, control)?
            .map(|model| CustomJsonRenderModel::new("custom-family", model)))
    }

    let assert_model = |model: &Value| {
        assert_eq!(model["title"], "<b>Custom &amp; &lt;</b>");
        assert_eq!(model["accTitle"], "Accessible");
        assert_eq!(model["accDescr"], "First\n<b>Second</b>");
    };
    for security_level in ["strict", "loose"] {
        for diagram_type in [
            "railroad",
            "railroadEbnf",
            "railroadAbnf",
            "railroadPeg",
            "cynefin",
            "wardley",
        ] {
            let mut engine = deterministic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(json!({
                    "securityLevel": security_level,
                    "htmlLabels": true
                })),
            );
            engine
                .diagram_registry_mut()
                .insert(diagram_type, custom_json);
            let parsed = engine
                .parse_diagram_with_type_sync(diagram_type, "custom input", ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_model(&parsed.model);
            #[cfg(feature = "editor-facts")]
            {
                let snapshot = engine
                    .parse_diagram_snapshot_with_type_sync(diagram_type, "custom input")
                    .unwrap()
                    .unwrap();
                assert_model(snapshot.outcome().parsed_model().unwrap());
                assert!(matches!(
                    snapshot.editor_facts(),
                    merman_core::ParsedEditorFacts::Unavailable
                ));
            }
            for explicit_render_overlay in [false, true] {
                if explicit_render_overlay {
                    engine
                        .render_diagram_registry_mut()
                        .insert(diagram_type, custom_render);
                }
                let typed = engine
                    .parse_diagram_for_render_model_with_type_sync(
                        diagram_type,
                        "custom input",
                        ParseOptions::strict(),
                    )
                    .unwrap()
                    .unwrap();
                assert!(matches!(
                    typed.model(),
                    merman_model::presentation::RenderSemanticModel::CustomJson(_)
                ));
                assert_model(&typed.compatibility_json().unwrap());
            }
        }
    }
}

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

#[cfg(feature = "diagram-git-graph")]
#[test]
fn incomplete_directive_recovery_preserves_each_parse_path_contract() {
    let engine = deterministic_engine();
    for (source, strict_source, recovered_source) in [
        (
            "gitGraph\n%%{\nmerge x\n",
            "gitGraph\n",
            "gitGraph\nmerge x\n",
        ),
        (
            "gitGraph LR:\n  commit\n  branch develoInfinity%%{\n  checkout main\n  merge develop\n",
            "gitGraph LR:\n  commit\n  branch develoInfinity",
            "gitGraph LR:\n  commit\n  branch develoInfinity\n  checkout main\n  merge develop\n",
        ),
    ] {
        let strict = engine.parse_diagram_sync(source, ParseOptions::strict());
        let lenient = engine.parse_diagram_sync(source, ParseOptions::lenient());
        assert_eq!(
            strict.as_ref().unwrap().as_ref().unwrap().meta.diagram_type,
            "gitGraph"
        );
        assert_eq!(
            lenient
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .meta
                .diagram_type,
            "error"
        );
        assert_pair(
            &strict,
            &engine.parse_diagram_for_render_model_sync(strict_source, ParseOptions::strict()),
            source,
        );
        assert_pair(
            &lenient,
            &engine.parse_diagram_for_render_model_sync(recovered_source, ParseOptions::lenient()),
            source,
        );
        assert_parse_paths(&engine, source);
        assert_known_type_parse_paths(&engine, "gitGraph", source);
        #[cfg(feature = "editor-facts")]
        {
            let snapshot = engine.parse_diagram_snapshot_sync(source).unwrap().unwrap();
            assert!(snapshot.recovered_incomplete_directive());
            assert!(matches!(
                snapshot.outcome(),
                merman_core::DiagramParseOutcome::Failed(Error::DiagramParse { diagram_type, .. })
                    if diagram_type == "gitGraph"
            ));
        }
    }
}

#[cfg(feature = "diagram-git-graph")]
#[test]
fn recovery_can_change_successful_models_without_changing_the_diagram_type() {
    let engine = deterministic_engine();
    let source = "gitGraph\n%%{\ncommit\n";
    let strict = engine
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let lenient = engine
        .parse_diagram_sync(source, ParseOptions::lenient())
        .unwrap()
        .unwrap();
    assert_eq!(strict.meta.diagram_type, "gitGraph");
    assert_eq!(lenient.meta.diagram_type, "gitGraph");
    assert_ne!(strict.model, lenient.model);
    assert_pair(
        &Ok(Some(strict)),
        &engine.parse_diagram_for_render_model_sync("gitGraph\n", ParseOptions::strict()),
        source,
    );
    assert_pair(
        &Ok(Some(lenient)),
        &engine.parse_diagram_for_render_model_sync("gitGraph\ncommit\n", ParseOptions::lenient()),
        source,
    );
    assert_parse_paths(&engine, source);
    assert_known_type_parse_paths(&engine, "gitGraph", source);

    // Complete directives and ordinary input retain the same policy result in both modes.
    for source in ["gitGraph\ncommit\n", "gitGraph\n%%{init: {}}%%\ncommit\n"] {
        assert_pair(
            &engine.parse_diagram_sync(source, ParseOptions::strict()),
            &engine.parse_diagram_for_render_model_sync(source, ParseOptions::lenient()),
            source,
        );
        assert_parse_paths(&engine, source);
        #[cfg(feature = "editor-facts")]
        assert!(
            !engine
                .parse_diagram_snapshot_sync(source)
                .unwrap()
                .unwrap()
                .recovered_incomplete_directive()
        );
    }
}

#[cfg(feature = "diagram-git-graph")]
#[test]
#[should_panic(expected = "different parser acceptance")]
fn recovery_does_not_hide_a_strict_typed_path_acceptance_change() {
    let engine = deterministic_engine();
    let source = "gitGraph\n%%{\nmerge x\n";
    let semantic = engine.parse_diagram_with_type_sync("gitGraph", source, ParseOptions::strict());
    assert!(matches!(&semantic, Ok(Some(parsed)) if parsed.meta.diagram_type == "gitGraph"));
    let typed = engine.parse_diagram_for_render_model_with_type_sync(
        "gitGraph",
        "gitGraph\nmerge x\n",
        ParseOptions::strict(),
    );
    assert_pair(&semantic, &typed, source);
}

#[cfg(feature = "diagram-git-graph")]
#[test]
#[should_panic(expected = "complete semantic projection")]
fn recovery_does_not_hide_a_lenient_known_type_model_change() {
    let engine = deterministic_engine();
    let source = "gitGraph\n%%{\ncommit id: \"expected\"\n";
    let semantic = engine.parse_diagram_with_type_sync("gitGraph", source, ParseOptions::lenient());
    assert!(matches!(&semantic, Ok(Some(parsed)) if parsed.meta.diagram_type == "gitGraph"));
    let typed = engine.parse_diagram_for_render_model_with_type_sync(
        "gitGraph",
        "gitGraph\ncommit id: \"changed\"\n",
        ParseOptions::lenient(),
    );
    assert_pair(&semantic, &typed, source);
}

#[cfg(all(feature = "diagram-git-graph", feature = "editor-facts"))]
#[test]
#[should_panic(expected = "snapshot recovery projection")]
fn recovery_receipt_does_not_hide_a_snapshot_model_change() {
    let engine = deterministic_engine();
    let source = "gitGraph\n%%{\ncommit id: \"wanted\"\n";
    let strict = engine.parse_diagram_sync(source, ParseOptions::strict());
    let lenient = engine.parse_diagram_sync(source, ParseOptions::lenient());
    let snapshot = engine
        .parse_diagram_snapshot_sync("gitGraph\n%%{\ncommit id: \"actual\"\n")
        .unwrap()
        .unwrap();
    assert!(snapshot.recovered_incomplete_directive());
    parse_equivalence::assert_snapshot(&strict, &lenient, Ok(Some(snapshot)), source);
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
fn every_catalog_family_and_alias_admits_configuration_with_the_same_precedence() {
    use merman_core::presentation::SourcePresentationConfig;
    use serde_json::json;
    use {
        merman_model::operation::OperationControl, merman_model::presentation::PresentationMetadata,
    };

    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for fact in diagram_family_capabilities() {
        let fixture = std::fs::read_to_string(
            fixtures.join(fixture_for(fact.diagram_type, fact.logical_family_kind)),
        )
        .expect("read legal family sample");
        let body = merman_core::preprocess::split_frontmatter_block(&fixture)
            .map_or(fixture.as_str(), |block| block.stripped);
        let mut site = json!({
            "theme": "dark", "fontFamily": "SiteFamily", "securityLevel": "strict",
            "secure": ["securityLevel", "maxTextSize"]
        });
        let mut init = json!({
            "theme": "neutral", "fontFamily": "InitFamily", "securityLevel": "loose",
            "secure": [], "maxTextSize": 1
        });
        let mut frontmatter = String::from(
            "---\nconfig:\n  theme: forest\n  fontFamily: FrontFamily\n  securityLevel: loose\n  secure: []\n  maxTextSize: 1\n",
        );
        if let Some(namespace) = fact.config_namespace {
            site[namespace] = json!({"useMaxWidth": true, "padding": 11});
            init[namespace] = json!({"useMaxWidth": false});
            frontmatter.push_str(&format!(
                "  {namespace}:\n    useMaxWidth: true\n    padding: 23\n"
            ));
        }
        let source = format!("{frontmatter}---\n%%{{init: {init}}}%%\n{body}");
        let engine =
            deterministic_engine().with_site_config(SourcePresentationConfig::from_value(site));
        let assert_config = |meta: &PresentationMetadata| {
            let authored = meta.config.as_value();
            let effective = meta.effective_config.as_value();
            assert_eq!(authored["theme"], "neutral", "{}", fact.diagram_type);
            assert_eq!(
                authored["fontFamily"], "InitFamily",
                "{}",
                fact.diagram_type
            );
            assert!(authored.get("secure").is_none(), "{}", fact.diagram_type);
            assert!(
                authored.get("securityLevel").is_none(),
                "{}",
                fact.diagram_type
            );
            assert!(
                authored.get("maxTextSize").is_none(),
                "{}",
                fact.diagram_type
            );
            assert_eq!(effective["theme"], "neutral", "{}", fact.diagram_type);
            assert_eq!(
                effective["fontFamily"], "InitFamily",
                "{}",
                fact.diagram_type
            );
            assert_eq!(
                effective["securityLevel"], "strict",
                "{}",
                fact.diagram_type
            );
            if let Some(namespace) = fact.config_namespace {
                // Selected Mermaid filters these init namespaces; their frontmatter survives.
                let use_max_width = matches!(namespace, "zenuml" | "wardley-beta");
                assert_eq!(
                    authored[namespace]["useMaxWidth"], use_max_width,
                    "{}",
                    fact.diagram_type
                );
                assert_eq!(
                    effective[namespace]["useMaxWidth"], use_max_width,
                    "{}",
                    fact.diagram_type
                );
                assert_eq!(effective[namespace]["padding"], 23, "{}", fact.diagram_type);
            }
        };
        assert_config(
            &engine
                .parse_metadata_sync(&source)
                .expect("detected metadata"),
        );
        assert_config(
            &engine
                .parse_metadata_with_type_sync(fact.diagram_type, &source)
                .expect("known metadata"),
        );
        if fact.has_semantic_parser {
            let semantic = engine
                .parse_diagram_with_type_sync(fact.diagram_type, &source, ParseOptions::strict())
                .unwrap_or_else(|error| panic!("{}: {error}", fact.diagram_type))
                .expect("legal sample");
            assert_config(&semantic.meta);
            let render = engine
                .parse_diagram_for_render_model_with_type_controlled_sync(
                    fact.diagram_type,
                    &source,
                    ParseOptions::strict(),
                    &OperationControl::new(),
                )
                .expect("active operation")
                .unwrap_or_else(|error| panic!("{}: {error}", fact.diagram_type))
                .expect("legal sample");
            assert_config(render.metadata());
        }
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

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-git-graph",
    feature = "diagram-block"
))]
#[test]
fn built_in_warnings_keep_original_coordinates_field_presence_and_order() {
    use serde_json::{Value, json};

    let engine = deterministic_engine();
    let cases = [
        #[cfg(feature = "diagram-flowchart")]
        (
            "\u{feff}---\r\ntitle: Warn\r\n---\r\n%% removed\r\nflowchart\r\nA --> B\r\n%% middle\r\nstyle Missing fill:red\r\nstyle Other fill:blue\r\n",
            json!([
                {
                    "ruleId": "merman.semantic.flowchart.unknown_style_target",
                    "message": "Style applied to unknown node \"Missing\". This may indicate a typo. The node will be created automatically.",
                    "span": {"start": 75, "end": 82}
                },
                {
                    "ruleId": "merman.semantic.flowchart.unknown_style_target",
                    "message": "Style applied to unknown node \"Other\". This may indicate a typo. The node will be created automatically.",
                    "span": {"start": 99, "end": 104}
                },
                {
                    "ruleId": "merman.authoring.flowchart.explicit_direction",
                    "message": "flowchart headers should declare an explicit direction such as `TB`, `TD`, `BT`, `LR`, or `RL`",
                    "span": {"start": 38, "end": 47},
                    "fixSpan": {"start": 47, "end": 47}
                }
            ]),
        ),
        #[cfg(feature = "diagram-git-graph")]
        (
            "\u{feff}---\r\ntitle: Warn\r\n---\r\n%% removed\r\ngitGraph\r\ncommit id:\"dup\"\r\n%% middle\r\ncommit id:\"dup\"\r\ncommit id:\"dup\"\r\n",
            json!([
                {
                    "ruleId": "merman.git_graph.duplicate_commit_id",
                    "message": "Commit ID dup already exists",
                    "span": {"start": 76, "end": 91}
                },
                {
                    "ruleId": "merman.git_graph.duplicate_commit_id",
                    "message": "Commit ID dup already exists",
                    "span": {"start": 93, "end": 108}
                }
            ]),
        ),
        #[cfg(feature = "diagram-block")]
        (
            "\u{feff}---\r\ntitle: Warn\r\n---\r\n%% removed\r\nblock-beta\r\ncolumns 1\r\nA:1\r\n%% middle\r\nB:2\r\nC:3\r\n",
            json!([
                {
                    "ruleId": "merman.block.width_exceeds_columns",
                    "message": "Block B width 2 exceeds configured column width 1"
                },
                {
                    "ruleId": "merman.block.width_exceeds_columns",
                    "message": "Block C width 3 exceeds configured column width 1"
                }
            ]),
        ),
    ];
    for (source, expected) in cases {
        let assert_model = |model: &Value| {
            assert_eq!(model["warningFacts"], expected, "{source}");
            let messages: Vec<_> = expected
                .as_array()
                .unwrap()
                .iter()
                .map(|fact| fact["message"].clone())
                .collect();
            assert_eq!(model["warnings"], json!(messages), "{source}");
        };
        for options in [ParseOptions::strict(), ParseOptions::lenient()] {
            let parsed = engine.parse_diagram_sync(source, options).unwrap().unwrap();
            assert_model(&parsed.model);
            let typed = engine
                .parse_diagram_for_render_model_sync(source, options)
                .unwrap()
                .unwrap();
            assert_model(&typed.compatibility_json().unwrap());
        }
        #[cfg(feature = "editor-facts")]
        {
            let snapshot = engine.parse_diagram_snapshot_sync(source).unwrap().unwrap();
            let merman_core::DiagramParseOutcome::Parsed {
                model,
                warning_facts,
            } = snapshot.outcome()
            else {
                panic!("warning-bearing snapshot must parse: {source}");
            };
            assert_model(model);
            assert_eq!(json!(warning_facts), expected, "{source}");
        }
    }
    for (source, has_empty_field) in [
        #[cfg(feature = "diagram-flowchart")]
        ("flowchart TD\nA --> B\n", false),
        #[cfg(feature = "diagram-git-graph")]
        ("gitGraph\ncommit id:\"one\"\n", true),
        #[cfg(feature = "diagram-block")]
        ("block\ncolumns 1\nA:1\n", true),
    ] {
        let parsed = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let expected = has_empty_field.then(|| json!([]));
        assert_eq!(parsed.model.get("warningFacts"), expected.as_ref());
        assert_eq!(parsed.model.get("warnings"), expected.as_ref());
    }
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn agentflow_omits_warning_json_but_retains_original_diagnostic_positions() {
    use serde_json::{Value, json};

    let source = "\u{feff}---\r\ntitle: Warn\r\n---\r\n%% removed\r\nagentflow-beta\r\nA@{ shape: cylinder, label: \"Data\" }\r\nA --> B\r\n";
    let engine = deterministic_engine();
    let assert_model = |model: &Value| {
        assert!(model.get("warningFacts").is_none());
        assert!(model.get("warnings").is_none());
        assert_eq!(model["diagnostics"].as_array().unwrap().len(), 1);
        assert_eq!(model["diagnostics"][0]["id"], "SHAPE_REMOVED");
        assert_eq!(
            model["diagnostics"][0]["position"],
            json!({
                "startLine": 6, "startColumn": 0,
                "endLine": 6, "endColumn": 35,
                "startIndex": 0, "endIndex": 0
            })
        );
    };
    let parsed = engine
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_model(&parsed.model);
    let typed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_model(&typed.compatibility_json().unwrap());
    #[cfg(feature = "editor-facts")]
    {
        let snapshot = engine.parse_diagram_snapshot_sync(source).unwrap().unwrap();
        let merman_core::DiagramParseOutcome::Parsed {
            model,
            warning_facts,
        } = snapshot.outcome()
        else {
            panic!("Agentflow snapshot must parse");
        };
        assert_model(model);
        assert_eq!(warning_facts.len(), 1);
        assert_eq!(
            warning_facts[0].span,
            Some(merman_model::source::SourceSpan::new(54, 90))
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
            #[cfg(feature = "editor-facts")]
            {
                let snapshot = engine
                    .parse_diagram_snapshot_with_type_sync(diagram_type, source)
                    .unwrap()
                    .unwrap();
                assert!(matches!(
                    snapshot.outcome(),
                    merman_core::DiagramParseOutcome::Failed(_)
                ));
                let merman_core::ParsedEditorFacts::Available(facts) = snapshot.editor_facts()
                else {
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
}

#[cfg(all(feature = "diagram-packet", feature = "editor-facts"))]
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
