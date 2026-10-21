//! Grammar-derived ZenUML parser and family-owned semantic model.
//!
//! The Mermaid 11.16 companion matrix currently evaluates ZenUML Core 3.50.1 behavior. Its
//! `sequenceLexer.g4` and `sequenceParser.g4` define the rules implemented by this module. The
//! lexer, recovering recursive parser, semantic builder, editor facts, and render model form one
//! owned pipeline; ZenUML is never translated through Mermaid Sequence JSON or actions.

mod ast;
mod lexer;
mod model;
mod parser;
mod semantic;

#[cfg(feature = "editor-facts")]
use crate::EditorSemanticFacts;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::{Error, OperationControl, OperationControlResult, SemanticMetadata};
#[cfg(feature = "presentation")]
use crate::{ParseMetadata, Result};
#[cfg(feature = "compat-json")]
use serde_json::Value;

#[cfg(feature = "presentation")]
pub use model::ZenumlDiagramRenderModel;
pub use model::{
    ZenumlDiagramModel, ZenumlFragmentKind, ZenumlFragmentSection, ZenumlGroup, ZenumlMessageStyle,
    ZenumlParticipant, ZenumlStatement, ZenumlStatementKind,
};
#[cfg(feature = "compat-json")]
pub(crate) use model::{render_model_to_compat_json, render_model_to_compat_json_controlled};

struct ZenumlSemanticSource {
    model: ZenumlDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
    first_diagnostic: Option<ast::SyntaxDiagnostic>,
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_zenuml(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let source = parse_semantic_source(code, meta)?;
    model::render_model_to_compat_json(&source.model, meta)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_zenuml_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<ZenumlDiagramRenderModel> {
    Ok(parse_semantic_source(code, meta)?.model)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_zenuml_model_for_render_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<ZenumlDiagramRenderModel>> {
    let source = construct_semantic_source(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    if let Some(diagnostic) = source.first_diagnostic {
        return Ok(Err(Error::diagram_parse_exact(
            meta.diagram_type.clone(),
            diagnostic.message,
            diagnostic.span,
        )));
    }
    Ok(Ok(source.model))
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_zenuml_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let source = construct_semantic_source(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )?;
    let construction = match &source.first_diagnostic {
        Some(diagnostic) => Err(crate::family::CombinedSemanticFailure::new(
            Error::diagram_parse_exact(
                meta.diagram_type.clone(),
                diagnostic.message.clone(),
                diagnostic.span,
            ),
            Some(source.editor_facts),
        )),
        None => Ok((
            model::render_model_to_compat_json_controlled(&source.model, meta, control)?,
            source.editor_facts,
        )),
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |projected| projected,
        crate::family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_semantic_source(code: &str, meta: &ParseMetadata) -> Result<ZenumlSemanticSource> {
    let source = construct_semantic_source(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled");
    if let Some(diagnostic) = &source.first_diagnostic {
        return Err(Error::diagram_parse_exact(
            meta.diagram_type.clone(),
            diagnostic.message.clone(),
            diagnostic.span,
        ));
    }
    Ok(source)
}

pub(crate) fn parse_zenuml_semantic_model_controlled(
    code: &str,
    metadata: &SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<ZenumlDiagramModel>> {
    let source = construct_semantic_source(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = editor_demand
        .needs_recovery()
        .then_some(source.editor_facts);
    let model = match source.first_diagnostic {
        Some(diagnostic) => Err(Error::diagram_parse_exact(
            metadata.diagram_type.clone(),
            diagnostic.message,
            diagnostic.span,
        )),
        None => {
            let mut model = source.model;
            model.sanitize_common_db_fields(&metadata.semantic_config);
            Ok(model)
        }
    };
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

fn construct_semantic_source(
    code: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<ZenumlSemanticSource> {
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("zenuml");

    control.checkpoint()?;
    let tokens = lexer::lex_controlled(code, control)?;
    let parsed = parser::parse_controlled(code, &tokens, control)?;
    control.checkpoint()?;
    let built = semantic::build_controlled(
        parsed,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    control.checkpoint()?;
    Ok(ZenumlSemanticSource {
        model: built.model,
        #[cfg(feature = "editor-facts")]
        editor_facts: built.editor_facts,
        first_diagnostic: built.diagnostics.into_iter().next(),
    })
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> SemanticMetadata {
        SemanticMetadata {
            diagram_type: "zenuml".into(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "themeVariables": {"primaryColor": "not-a-color"}
            })),
            title: None,
        }
    }

    #[test]
    fn canonical_entry_keeps_authored_records_and_ownership_without_editor_work() {
        let source = concat!(
            "zenuml\ntitle Authored title\n",
            "group Business { @Actor Client @Boundary Service 00042 }\n",
            "@Starter(Client)\nService.call(){new Model(x=1)}\n",
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("zenuml");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_zenuml_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.title.as_deref(), Some("Authored title"));
        assert_eq!(model.starter.as_deref(), Some("Client"));
        assert_eq!(model.groups[0].id.as_deref(), Some("Business"));
        assert_eq!(model.groups[0].participant_names, ["Client", "Service"]);
        assert_eq!(
            model
                .participant("Service")
                .unwrap()
                .width_source
                .as_deref(),
            Some("00042")
        );
        let ZenumlStatementKind::Message {
            resolved_from,
            resolved_to,
            body,
            ..
        } = &model.statements[0].kind
        else {
            panic!("expected message");
        };
        assert_eq!(resolved_from.as_deref(), Some("Client"));
        assert_eq!(resolved_to.as_deref(), Some("Service"));
        assert!(matches!(
            &body[0].kind,
            ZenumlStatementKind::Creation { constructor, parameters, .. }
                if constructor == "Model" && parameters == "x=1"
        ));
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("zenuml"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn observer_demand_preserves_models_recovery_and_exact_error_spans() {
        let source = "zenuml\n@Starter(A) A.call() ? B.call()";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let omitted = construct_semantic_source(
            source,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(omitted.editor_facts.symbols.is_empty());
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let captured = construct_semantic_source(
            source,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(omitted.model, captured.model);
        assert_eq!(omitted.first_diagnostic, captured.first_diagnostic);
        assert_eq!(omitted.model.statements.len(), 2);
        assert!(
            captured
                .editor_facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "B")
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read().collections, 1);
        let expected = captured.first_diagnostic.unwrap();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_zenuml_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected diagram parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(expected.span));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn semantic_construction_keeps_control_cancellation() {
        #[cfg(feature = "editor-facts")]
        for demand in [EditorFactsDemand::Omit, EditorFactsDemand::Capture] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(3);
            assert!(construct_semantic_source("zenuml\nA.call()", demand, &control).is_err());
        }
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        assert!(
            parse_zenuml_semantic_model_controlled(
                "zenuml\nA.call()",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn typed_and_json_only_entrypoints_do_not_collect_editor_facts() {
        let meta = ParseMetadata {
            diagram_type: "zenuml".into(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        };
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_zenuml_model_for_render_controlled(
            "zenuml\n@Starter(A) B.call()",
            &meta,
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "compat-json")]
        {
            parse_zenuml("zenuml\n@Starter(A) B.call()", &meta).unwrap();
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }
}

#[cfg(all(test, feature = "presentation"))]
mod tests {
    use super::*;
    use crate::{MermaidConfig, RenderSemanticModel};

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "zenuml".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        }
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn zenuml_boundary_construction_stages_on_host_stack() {
        use std::io::Write as _;

        fn phase(name: &str) {
            let mut output = std::io::stdout().lock();
            writeln!(output, "zenuml_boundary_phase={name}").unwrap();
            output.flush().unwrap();
        }

        std::thread::Builder::new()
            .name("zenuml-boundary-construction".into())
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let depth = crate::MAX_DIAGRAM_NESTING_DEPTH;
                let source = format!(
                    "zenuml\n{}A.call()\n{}",
                    "opt {\n".repeat(depth),
                    "}\n".repeat(depth)
                );
                phase("lex-begin");
                let tokens = lexer::lex_controlled(&source, &OperationControl::new()).unwrap();
                phase("syntax-begin");
                let parsed =
                    parser::parse_controlled(&source, &tokens, &OperationControl::new()).unwrap();
                assert!(parsed.diagnostics.is_empty());
                phase("semantic-begin");
                let built = semantic::build_controlled(
                    parsed,
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Capture,
                    &OperationControl::new(),
                )
                .unwrap();
                phase("clone-begin");
                let cloned = built.model.clone();
                drop(cloned);
                phase("serde-begin");
                let error = serde_json::to_value(&built.model).unwrap_err();
                assert!(error.to_string().contains("128-container"), "{error}");
                phase("projection-begin");
                let json = crate::ManagedSemanticJson::from_value(
                    model::render_model_to_compat_json(&built.model, &meta()).unwrap(),
                );
                phase("writer-begin");
                let mut bytes = Vec::new();
                json.write_json(&mut bytes).unwrap();
                assert!(!bytes.is_empty());
                phase("drop-begin");
                drop(json);
                drop(built);
                phase("done");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn typed_model_is_owned_by_zenuml() {
        let model =
            parse_zenuml_model_for_render("zenuml\n@Starter(A)\nB.call()\n", &meta()).unwrap();
        assert_eq!(model.starter.as_deref(), Some("A"));
        assert!(matches!(
            model.statements[0].kind,
            ZenumlStatementKind::Message { .. }
        ));

        let wrapped = RenderSemanticModel::Zenuml(model);
        assert_eq!(wrapped.kind(), "zenuml");
    }

    #[test]
    fn inline_emoji_and_hidden_modifiers_follow_the_oracle_channels() {
        let model = parse_zenuml_model_for_render(
            "zenuml\nA->[rocket]B.call()\nconst result = await Service.work()\n",
            &meta(),
        )
        .unwrap();

        assert_eq!(
            model
                .participant("B")
                .and_then(|participant| participant.emoji.as_deref()),
            Some("rocket")
        );
        let ZenumlStatementKind::Message {
            resolved_from,
            resolved_to,
            ..
        } = &model.statements[0].kind
        else {
            panic!("expected message");
        };
        assert_eq!(
            (resolved_from.as_deref(), resolved_to.as_deref()),
            (Some("A"), Some("B"))
        );
        let ZenumlStatementKind::Message {
            assignment,
            resolved_to,
            ..
        } = &model.statements[1].kind
        else {
            panic!("expected message");
        };
        assert_eq!(assignment.as_deref(), Some("result"));
        assert_eq!(resolved_to.as_deref(), Some("Service"));
    }

    #[test]
    fn companion_matrix_supports_digit_names_units_emoji_and_optional_if_blocks() {
        let source = concat!(
            "zenuml\n",
            "[rocket] 2FAService\n",
            "[rocket]2FAService->[lock]3DSecure.call(10ms)\n",
            "if(5xx_error)\n",
            "3DSecure.next()\n",
        );
        let model =
            parse_zenuml_model_for_render(source, &meta()).expect("matrix candidate syntax");

        let service = model
            .participant("2FAService")
            .expect("digit-leading participant");
        assert_eq!(service.emoji.as_deref(), Some("rocket"));
        let secure = model
            .participant("3DSecure")
            .expect("digit-leading endpoint");
        assert_eq!(secure.emoji.as_deref(), Some("lock"));
        assert!(matches!(
            &model.statements[0].kind,
            ZenumlStatementKind::Message { label, .. } if label == "call(10ms)"
        ));
        assert!(matches!(
            &model.statements[1].kind,
            ZenumlStatementKind::Fragment { sections, .. }
                if sections.len() == 1 && sections[0].statements.is_empty()
        ));
        assert!(matches!(
            &model.statements[2].kind,
            ZenumlStatementKind::Message { label, .. } if label == "next()"
        ));
    }

    #[test]
    fn comments_preserve_whitespace_and_typed_owner_boundaries() {
        let source = concat!(
            "zenuml\n",
            "// participant comment \n",
            "@Actor A\n",
            "// statement comment  \n",
            "A.m() {\n",
            "// block close comment \n",
            "}\n",
        );
        let model = parse_zenuml_model_for_render(source, &meta()).expect("comment ownership");
        assert_eq!(
            model
                .participant("A")
                .and_then(|participant| participant.comment.as_deref()),
            Some(" participant comment ")
        );
        assert_eq!(
            model.statements[0].comment.as_deref(),
            Some(" statement comment  ")
        );
        let ZenumlStatementKind::Message { body_comment, .. } = &model.statements[0].kind else {
            panic!("expected message");
        };
        assert_eq!(body_comment.as_deref(), Some(" block close comment "));
    }

    #[test]
    fn parameters_and_conditions_preserve_oracle_semantics_without_symbol_leaks() {
        let source = concat!(
            "zenuml\n",
            "A.m(x=1, Type value, B.call(), 10ms) ",
            "if(item in items){A.m()} ",
            "if(status pending 10ms){A.m()}",
        );
        let model = parse_zenuml_model_for_render(source, &meta()).expect("rule semantics");
        assert_eq!(
            model
                .participants
                .iter()
                .map(|participant| participant.name.as_str())
                .collect::<Vec<_>>(),
            vec!["_STARTER_", "A"]
        );
        let ZenumlStatementKind::Message { label, .. } = &model.statements[0].kind else {
            panic!("expected message");
        };
        assert_eq!(label, "m(x=1,Type value,B.call(),10ms)");
        for (statement, expected) in model.statements[1..]
            .iter()
            .zip(["item in items", "status pending 10ms"])
        {
            let ZenumlStatementKind::Fragment { label, .. } = &statement.kind else {
                panic!("expected conditional fragment");
            };
            assert_eq!(label.as_deref(), Some(expected));
        }
    }

    #[test]
    fn starter_and_group_prediction_match_the_companion_runtime() {
        let unnamed = parse_zenuml_model_for_render("zenuml\n@Starter() A.m()", &meta())
            .expect("optional starter name");
        assert!(unnamed.starter.is_none());

        let dotted = parse_zenuml_model_for_render("zenuml\n@Starter(S) A.B", &meta())
            .expect("dotted bare function");
        assert!(matches!(
            &dotted.statements[0].kind,
            ZenumlStatementKind::Message { resolved_to, label, .. }
                if resolved_to.as_deref() == Some("A") && label == "B"
        ));

        let grouped = parse_zenuml_model_for_render(
            "zenuml\ngroup Business { @Actor A @Boundary B } A->B.m()",
            &meta(),
        )
        .expect("group head");
        assert!(grouped.starter.is_none());
        assert!(grouped.participant("_STARTER_").is_none());
        assert_eq!(grouped.groups[0].participant_names, ["A", "B"]);
    }

    #[test]
    fn default_starter_materialization_matches_ordered_participants() {
        for source in ["zenuml\n", "zenuml\n@Starter()"] {
            let model =
                parse_zenuml_model_for_render(source, &meta()).expect("valid empty diagram");
            assert_eq!(
                model
                    .participants
                    .iter()
                    .map(|participant| participant.name.as_str())
                    .collect::<Vec<_>>(),
                ["_STARTER_"]
            );
            assert!(model.participants[0].is_starter);
        }

        let declarations = parse_zenuml_model_for_render("zenuml\n@Actor A", &meta())
            .expect("participant-only diagram");
        assert!(declarations.participant("_STARTER_").is_none());

        let explicit_from = parse_zenuml_model_for_render("zenuml\nA->B.m()", &meta())
            .expect("message with sender");
        assert!(explicit_from.participant("_STARTER_").is_none());

        let implicit_from = parse_zenuml_model_for_render("zenuml\nB.m()", &meta())
            .expect("message without sender");
        assert_eq!(implicit_from.participants[0].name, "_STARTER_");
        assert!(implicit_from.participants[0].is_starter);
    }

    #[test]
    fn head_items_preserve_source_order_and_unnamed_groups_remain_unrendered() {
        let named = parse_zenuml_model_for_render(
            "zenuml\ngroup G { @Actor B } @Boundary A A->B.m()",
            &meta(),
        )
        .expect("interleaved head");
        assert_eq!(
            named
                .participants
                .iter()
                .map(|participant| participant.name.as_str())
                .collect::<Vec<_>>(),
            ["B", "A"]
        );
        assert_eq!(named.groups[0].id.as_deref(), Some("G"));
        assert_eq!(
            named.participant("B").unwrap().group_id.as_deref(),
            Some("G")
        );

        let unnamed = parse_zenuml_model_for_render(
            "zenuml\ngroup { @Actor B } @Boundary A A->B.m()",
            &meta(),
        )
        .expect("unnamed group");
        assert!(unnamed.groups[0].id.is_none());
        assert!(unnamed.participant("B").unwrap().group_id.is_none());
    }

    #[test]
    fn creation_signature_keeps_constructor_and_parameters_as_distinct_semantics() {
        let model = parse_zenuml_model_for_render(
            "zenuml\nnew A\nnew B(x=1, Type value, C.call())\nnew",
            &meta(),
        )
        .expect("creation signatures");
        for (statement, expected_constructor, expected_parameters, expected_label) in [
            (&model.statements[0], "A", "", "«create»"),
            (
                &model.statements[1],
                "B",
                "x=1,Type value,C.call()",
                "«x=1,Type value,C.call()»",
            ),
            (&model.statements[2], "Missing Constructor", "", "«create»"),
        ] {
            let ZenumlStatementKind::Creation {
                constructor,
                parameters,
                label,
                ..
            } = &statement.kind
            else {
                panic!("expected creation");
            };
            assert_eq!(constructor, expected_constructor);
            assert_eq!(parameters, expected_parameters);
            assert_eq!(label, expected_label);
        }
    }

    #[test]
    fn unresolved_endpoints_survive_semantic_construction() {
        let receiverless = parse_zenuml_model_for_render("zenuml\n@Starter(A)\nmethod()", &meta())
            .expect("receiver-less root message");
        let ZenumlStatementKind::Message {
            explicit_from,
            resolved_from,
            resolved_to,
            ..
        } = &receiverless.statements[0].kind
        else {
            panic!("expected message");
        };
        assert!(explicit_from.is_none());
        assert_eq!(resolved_from.as_deref(), Some("A"));
        assert!(resolved_to.is_none());
        assert_eq!(
            receiverless
                .participants
                .iter()
                .map(|participant| participant.name.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );

        let incomplete =
            parse_zenuml_model_for_render("zenuml\nA ->", &meta()).expect("incomplete async");
        let ZenumlStatementKind::Message {
            explicit_from,
            resolved_from,
            resolved_to,
            ..
        } = &incomplete.statements[0].kind
        else {
            panic!("expected async message");
        };
        assert_eq!(explicit_from.as_deref(), Some("A"));
        assert_eq!(resolved_from.as_deref(), Some("A"));
        assert!(resolved_to.is_none());
        assert!(incomplete.participant("_STARTER_").is_none());
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn width_projection_matches_parse_int_without_saturating_to_u64() {
        for (source_width, expected) in [
            ("0", serde_json::Value::Null),
            ("00042", serde_json::json!(42.0)),
            (
                "18446744073709551616",
                serde_json::json!(18_446_744_073_709_552_000.0_f64),
            ),
            ("1000000000000000000000000", serde_json::json!(1e24_f64)),
        ] {
            let source = format!("zenuml\n@Actor A {source_width}");
            let model = parse_zenuml_model_for_render(&source, &meta()).expect("participant width");
            assert_eq!(
                model.participant("A").unwrap().width_source.as_deref(),
                Some(source_width)
            );
            let compat = parse_zenuml(&source, &meta()).expect("compat model");
            assert_eq!(compat["participants"][0]["width"], expected);
        }

        let huge = "9".repeat(400);
        let source = format!("zenuml\n@Actor A {huge}");
        let model = parse_zenuml_model_for_render(&source, &meta()).expect("huge width");
        assert_eq!(
            model.participant("A").unwrap().width_source.as_deref(),
            Some(huge.as_str())
        );
        let compat = parse_zenuml(&source, &meta()).expect("compat huge width");
        assert!(compat["participants"][0]["width"].is_null());
    }

    #[test]
    fn divider_keeps_its_source_lexeme_and_statements_have_no_semantic_number() {
        let model =
            parse_zenuml_model_for_render("zenuml\n== Wide label ==", &meta()).expect("divider");
        let ZenumlStatementKind::Divider { label } = &model.statements[0].kind else {
            panic!("expected divider");
        };
        assert_eq!(label, "== Wide label ==");

        #[cfg(feature = "compat-json")]
        {
            let compat = parse_zenuml("zenuml\nA.m()", &meta()).expect("compat model");
            assert!(compat["statements"][0].get("number").is_none());
        }
    }
}
