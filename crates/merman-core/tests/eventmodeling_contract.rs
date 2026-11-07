#![cfg(all(feature = "diagram-event-modeling", feature = "compat-json"))]

#[cfg(feature = "editor-facts")]
use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
use {
    merman_core::{Error, MermaidEngine, ParseOptions, SemanticParser},
    merman_model::{
        families::event_modeling::EventModelingDiagramModel, presentation::RenderSemanticModel,
        semantic::SemanticDiagramModel, source::SourceSpan,
    },
};

fn assert_eventmodeling_parse_routes(source: &str) -> EventModelingDiagramModel {
    let canonical = SemanticParser::new()
        .parse_diagram_sync(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"));
    let SemanticDiagramModel::EventModeling(model) = canonical.model else {
        panic!("expected canonical Event Modeling model");
    };
    let engine = MermaidEngine::new();
    let render = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap_or_else(|error| panic!("{source}: {error}"))
        .unwrap();
    let RenderSemanticModel::EventModeling(render_model) = render.model() else {
        panic!("expected Event Modeling render model");
    };
    let compatibility = engine
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap_or_else(|error| panic!("{source}: {error}"))
        .unwrap();
    let expected = serde_json::to_value(&model).unwrap();
    let rendered = serde_json::to_value(render_model).unwrap();
    for key in ["frames", "dataEntities"] {
        assert_eq!(rendered[key], expected[key], "{source}");
        assert_eq!(compatibility.model[key], expected[key], "{source}");
    }
    model
}

#[test]
fn adjacent_hidden_comments_separate_all_scalar_fields_across_parse_routes() {
    for separator in [
        " ",
        "/**/",
        "/* boundary */",
        "// boundary\r\n",
        "%% boundary\r\n",
        "%%{init: {}}%%\r\n",
    ] {
        let source = format!(
            "---\ntitle: Scalar boundaries\n---\neventmodeling\n\
             entity Changed\n\
             tf 01{separator}cmd{separator}Start\n\
             rf 02{separator}evt{separator}Done ->> 01{separator}[[Payload{separator}]] \
             `json{separator}`{{ value: 7 }}\n\
             data Payload{separator}`text{separator}`{{\nbody\n}}\n\
             note 02{separator}`md{separator}`{{\nnote body\n}}\n\
             gwt 02{separator}given evt{separator}Changed{separator}when \
             cmd{separator}Changed{separator}then evt{separator}Changed{separator}\n"
        );
        let model = assert_eventmodeling_parse_routes(&source);
        assert_eq!(model.frames.len(), 2);
        assert_eq!(model.frames[0].name, "01");
        assert_eq!(model.frames[0].model_entity_type, "cmd");
        assert_eq!(model.frames[0].entity_identifier, "Start");
        assert_eq!(model.frames[1].name, "02");
        assert_eq!(model.frames[1].frame_kind, "resetframe");
        assert_eq!(model.frames[1].model_entity_type, "evt");
        assert_eq!(model.frames[1].entity_identifier, "Done");
        assert_eq!(model.frames[1].source_frames, ["01"]);
        assert_eq!(model.frames[1].data_reference.as_deref(), Some("Payload"));
        assert_eq!(
            model.frames[1].data_inline_value.as_deref(),
            Some("{ value: 7 }")
        );
        assert_eq!(model.data_entities.len(), 1);
        assert_eq!(model.data_entities[0].name, "Payload");

        #[cfg(feature = "editor-facts")]
        {
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("valid Event Modeling input must produce a canonical snapshot");
            };
            assert!(snapshot.outcome().parsed_model().is_some());
            let ParsedEditorFacts::Available(canonical_facts) = snapshot.editor_facts() else {
                panic!("canonical snapshot must expose Event Modeling facts");
            };
            let engine_facts = MermaidEngine::new()
                .parse_editor_semantic_facts_with_type_sync("eventmodeling", &source)
                .unwrap()
                .unwrap();
            for facts in [canonical_facts, &engine_facts] {
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
                assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);
                for name in ["01", "02"] {
                    let start = source.find(name).unwrap();
                    let frame = facts
                        .symbols
                        .iter()
                        .find(|symbol| {
                            symbol.name == name
                                && symbol.detail.as_deref() == Some("eventmodeling frame")
                        })
                        .expect("frame declaration must survive adjacent comments");
                    assert_eq!(frame.span, SourceSpan::new(start, start + name.len()));
                    assert_eq!(frame.selection, frame.span);
                }
                for symbol in facts.symbols.iter().filter(|symbol| {
                    matches!(
                        symbol.detail.as_deref(),
                        Some(
                            "eventmodeling frame"
                                | "eventmodeling entity type"
                                | "eventmodeling source frame"
                                | "eventmodeling data reference"
                                | "eventmodeling data entity"
                                | "eventmodeling data type"
                                | "eventmodeling note source frame"
                                | "eventmodeling gwt source frame"
                                | "eventmodeling gwt entity reference"
                        )
                    )
                }) {
                    assert_eq!(&source[symbol.span.start..symbol.span.end], symbol.name);
                    assert_eq!(symbol.selection, symbol.span);
                }
            }
        }
    }
}

#[test]
fn comment_markers_inside_inline_and_block_payloads_remain_literal() {
    let block = "{\n/*keep*/\n//keep\n%%keep\n}";
    for inline in [
        "{ /*keep*/ //keep %%keep }",
        "\"/*keep*/ //keep %%keep\"",
        "'/*keep*/ //keep %%keep'",
    ] {
        let source = format!(
            "eventmodeling\ntf 01 evt Started {inline}\ndata Payload {block}\nnote 01 {block}\n"
        );
        let model = assert_eventmodeling_parse_routes(&source);
        assert_eq!(model.frames[0].data_inline_value.as_deref(), Some(inline));
        assert_eq!(model.data_entities[0].data_block_value, block);
        #[cfg(feature = "editor-facts")]
        {
            let facts = MermaidEngine::new()
                .parse_editor_semantic_facts_with_type_sync("eventmodeling", &source)
                .unwrap()
                .unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            for (detail, expected) in [
                ("eventmodeling inline data", inline),
                ("eventmodeling data block", block),
                ("eventmodeling note block", block),
            ] {
                let payload = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.detail.as_deref() == Some(detail))
                    .expect("payload fact must retain comment-looking literals");
                assert_eq!(payload.name, expected);
                assert_eq!(&source[payload.span.start..payload.span.end], expected);
                assert_eq!(payload.selection, payload.span);
            }
        }
    }
}

#[test]
fn partial_comment_markers_and_split_scalars_remain_invalid() {
    for (statement, invalid) in [
        ("tf 1000/* boundary */evt Started", "1000"),
        ("tf 01/ evt Started", "01/"),
        ("tf 01% evt Started", "01%"),
        ("tf 01*/ evt Started", "01*/"),
        ("tf 01/% evt Started", "01/%"),
        ("tf 01 evt/ Started", "evt/"),
        ("tf 01 evt% Started", "evt%"),
        ("tf 01 evt*/ Started", "evt*/"),
        ("tf 0/* boundary */1 evt Started", "1"),
        ("tf 01 ev/* boundary */t Started", "ev"),
    ] {
        let prefix = "eventmodeling\n";
        let source = format!("{prefix}{statement}\ntf 03 evt Later\n");
        let start = prefix.len() + statement.find(invalid).unwrap();
        let expected = SourceSpan::new(start, start + invalid.len());
        let engine = MermaidEngine::new();
        for error in [
            SemanticParser::new()
                .parse_diagram_sync(&source)
                .unwrap_err(),
            engine
                .parse_diagram_sync(&source, ParseOptions::strict())
                .unwrap_err(),
            engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_err(),
        ] {
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("invalid scalar must remain a family parse error: {source}");
            };
            assert_eq!(diagnostic.span(), Some(expected), "{source}");
        }
        #[cfg(feature = "editor-facts")]
        {
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("eventmodeling", &source)
                .unwrap()
                .unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(
                facts
                    .diagnostics
                    .iter()
                    .any(|item| item.span == Some(expected))
            );
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "03" && symbol.detail.as_deref() == Some("eventmodeling frame")
            }));
        }
    }
}

#[test]
fn unterminated_adjacent_block_comments_report_eof_and_keep_prefix_facts() {
    for statement in ["tf 02/* unterminated", "tf 02 evt/* unterminated"] {
        let source = format!("eventmodeling\ntf 01 evt Before\n{statement}");
        let expected = SourceSpan::new(source.len(), source.len());
        let engine = MermaidEngine::new();
        for error in [
            SemanticParser::new()
                .parse_diagram_sync(&source)
                .unwrap_err(),
            engine
                .parse_diagram_sync(&source, ParseOptions::strict())
                .unwrap_err(),
            engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_err(),
        ] {
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("unterminated comment must remain a family parse error");
            };
            assert_eq!(
                diagnostic.message(),
                "expected closing eventmodeling block comment"
            );
            assert_eq!(diagnostic.span(), Some(expected));
        }
        #[cfg(feature = "editor-facts")]
        {
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("eventmodeling", &source)
                .unwrap()
                .unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(
                facts
                    .diagnostics
                    .iter()
                    .any(|item| item.span == Some(expected))
            );
            let frame = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.name == "01" && symbol.detail.as_deref() == Some("eventmodeling frame")
                })
                .expect("the frame before the unclosed comment must retain its facts");
            let start = source.find("01").unwrap();
            assert_eq!(frame.span, SourceSpan::new(start, start + 2));
        }
    }
}

#[test]
fn duplicate_eventmodeling_frame_ids_fail_all_parse_routes_with_source_spans() {
    let engine = MermaidEngine::new();
    for first in ["tf", "rf"] {
        for second in ["tf", "rf"] {
            let source = format!(
                "---\ntitle: Frame IDs\n---\neventmodeling\n{first} 01 ui First\n%% comment preserves the original source line\n{second} 01 evt Second\ntf 02 cmd Later\n"
            );
            let start = source.rfind("01").unwrap();
            let expected = SourceSpan::new(start, start + 2);
            for error in [
                engine
                    .parse_diagram_sync(&source, ParseOptions::strict())
                    .unwrap_err(),
                engine
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap_err(),
            ] {
                let Error::DiagramParse { diagnostic, .. } = error else {
                    panic!("expected duplicate-frame parse error");
                };
                assert!(
                    diagnostic
                        .message()
                        .contains("Duplicate event modeling frame ID")
                );
                assert_eq!(diagnostic.span(), Some(expected), "{first}/{second}");
            }
            #[cfg(feature = "editor-facts")]
            {
                let facts = engine
                    .parse_editor_semantic_facts_with_type_sync("eventmodeling", &source)
                    .unwrap()
                    .unwrap();
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(
                    facts
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.span == Some(expected)
                            && diagnostic
                                .message
                                .contains("Duplicate event modeling frame ID")),
                    "{:?}",
                    facts.diagnostics
                );
                assert!(
                    facts.symbols.iter().any(|symbol| symbol.name == "02"),
                    "later valid frame facts must survive recovery"
                );
            }
        }
    }
    let valid = "eventmodeling\ntf 01 ui Fresh\n";
    assert!(
        engine
            .parse_diagram_for_render_model_sync(valid, ParseOptions::strict())
            .unwrap()
            .is_some()
    );
}
