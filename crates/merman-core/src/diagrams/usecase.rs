//! Native Mermaid 12 Usecase grammar and transactional semantic construction.
//!
//! Grammar, declaration merging, and deferred resolution follow the pinned upstream
//! `usecase.parser.ts`, `usecase.visitor.ts`, and `usecaseModelBuilder.ts`.

#[cfg(feature = "editor-facts")]
use crate::EditorSemanticFacts;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::{Error, OperationControl, OperationControlResult, SourceSpan, family};
#[cfg(feature = "presentation")]
use crate::{PresentationMetadata, Result};
pub use merman_model::families::usecase::{
    UsecaseActorType, UsecaseAnimation, UsecaseArrowType, UsecaseBoundary, UsecaseClassDef,
    UsecaseDiagramModel, UsecaseJsonInfinity, UsecaseJsonNode, UsecaseJsonStringEncoding,
    UsecaseLabelType, UsecaseNode, UsecaseNodeKind, UsecaseNote, UsecaseRelationship,
    UsecaseRelationshipType,
};
use serde_json::Value;
#[cfg(feature = "compat-json")]
use serde_json::json;

mod json_string;
mod lexer;
mod ordered_json;
mod parse;
mod resolve;

/// Whether an unquoted spelling is one complete identifier in the pinned grammar.
#[cfg(feature = "editor-facts")]
pub(crate) fn is_valid_editor_identifier(candidate: &str) -> bool {
    if candidate.is_empty()
        || is_forbidden_statement_identifier(candidate)
        || !candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return false;
    }
    let Ok(Ok(tokens)) = lexer::lex(candidate, &OperationControl::new()) else {
        return false;
    };
    tokens.len() == 2 && tokens[0].kind == lexer::TokenKind::Identifier
}

fn is_forbidden_statement_identifier(candidate: &str) -> bool {
    [
        "allowmixing",
        "newpage",
        "package",
        "rectangle",
        "skinparam",
    ]
    .iter()
    .any(|keyword| candidate.eq_ignore_ascii_case(keyword))
}

pub(crate) fn sanitize_common_db_fields(
    model: &mut UsecaseDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_description, config);
}

#[derive(Debug)]
struct ParseIssue {
    message: String,
    span: SourceSpan,
}

impl ParseIssue {
    fn new(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

pub(crate) fn parse_usecase_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<family::SemanticConstruction<UsecaseDiagramModel>> {
    let construction = construct_usecase_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        metadata.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = None;
    let model = match construction {
        Ok(source) => {
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            #[cfg(feature = "editor-facts")]
            {
                editor_facts = editor_demand
                    .needs_recovery()
                    .then_some(source.editor_facts);
            }
            Ok(model)
        }
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            if editor_demand.needs_recovery() {
                let (error, facts) = failure.into_parts();
                editor_facts = Some(facts);
                return Ok(family::SemanticConstruction {
                    model: Err(error),
                    editor_facts,
                });
            }
            Err(failure.into_error())
        }
    };
    Ok(family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

#[cfg(all(test, feature = "compat-json"))]
fn construct(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    construct_usecase_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )
}

struct Construction {
    model: UsecaseDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_usecase(code: &str, meta: &PresentationMetadata) -> Result<Value> {
    let control = OperationControl::new();
    let construction = construct_usecase_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &control,
    )
    .expect("a private operation control cannot be cancelled")
    .map_err(family::CombinedSemanticFailure::into_error)?;
    render_model_to_compat_json(&construction.model, meta)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_usecase_model_for_render_controlled(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<UsecaseDiagramModel>> {
    Ok(construct_usecase_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?
    .map(|value| value.model)
    .map_err(family::CombinedSemanticFailure::into_error))
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_usecase_json_and_editor_facts(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_usecase_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )?;
    Ok(family::CombinedSemanticParse::from_construction(
        construction,
        |value| {
            (
                render_model_to_compat_json(&value.model, meta),
                value.editor_facts,
            )
        },
        family::CombinedSemanticFailure::into_parts,
    ))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &UsecaseDiagramModel,
    meta: &PresentationMetadata,
) -> Result<Value> {
    let json_nodes: Vec<_> = model
        .json_nodes
        .iter()
        .map(|node| {
            let mut value = json!({
                "id": node.id, "value": node.value, "propertyOrder": node.property_order,
                "classes": node.classes, "styles": node.styles,
            });
            if let Some(encoding) = node.string_encoding {
                value["stringEncoding"] = json!(encoding);
            }
            value
        })
        .collect();
    Ok(
        json!({ "type": meta.diagram_type, "direction": model.direction,
        "nodes": model.nodes, "boundaries": model.boundaries, "relationships": model.relationships,
        "notes": model.notes, "jsonNodes": json_nodes, "classDefs": model.class_defs,
        "title": model.title, "accTitle": model.acc_title, "accDescr": model.acc_description }),
    )
}

fn construct_usecase_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    title: Option<&str>,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("usecase");
    let tokens = match lexer::lex(code, control)? {
        Ok(tokens) => tokens,
        Err(issue) => {
            return Ok(Err(failure(
                diagram_type,
                issue,
                #[cfg(feature = "editor-facts")]
                EditorSemanticFacts::new(),
                #[cfg(feature = "editor-facts")]
                editor_demand,
            )));
        }
    };
    let mut parser = parse::Parser::new(
        code,
        tokens,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    );
    let parsed = parser.parse()?;
    #[cfg(feature = "editor-facts")]
    parser.finalize_editor_facts()?;
    if let Err(issue) = parsed {
        return Ok(Err(failure(
            diagram_type,
            issue,
            #[cfg(feature = "editor-facts")]
            parser.facts,
            #[cfg(feature = "editor-facts")]
            editor_demand,
        )));
    }
    let model = resolve::resolve(&parser.draft, title, control)?;
    match model {
        Ok(model) => Ok(Ok(Construction {
            model,
            #[cfg(feature = "editor-facts")]
            editor_facts: parser.facts,
        })),
        Err(issue) => Ok(Err(failure(
            diagram_type,
            issue,
            #[cfg(feature = "editor-facts")]
            parser.facts,
            #[cfg(feature = "editor-facts")]
            editor_demand,
        ))),
    }
}

fn failure(
    diagram_type: &str,
    issue: ParseIssue,
    #[cfg(feature = "editor-facts")] mut facts: EditorSemanticFacts,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
) -> family::CombinedSemanticFailure {
    #[cfg(feature = "editor-facts")]
    if editor_demand.needs_recovery() {
        facts.mark_recovered_from_parse_error(issue.message.clone(), Some(issue.span));
    }
    family::CombinedSemanticFailure::new(
        Error::diagram_parse_exact(diagram_type.to_string(), issue.message, issue.span),
        #[cfg(feature = "editor-facts")]
        editor_demand.needs_recovery().then_some(facts),
    )
}

#[cfg(all(test, feature = "compat-json"))]
mod tests;

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "usecase".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict"
            })),
            title: None,
        }
    }

    #[test]
    fn semantic_model_preserves_domain_records_without_editor_work() {
        let mut metadata = metadata();
        metadata.title = Some("<script>secret</script>Authentication".to_string());
        let source = concat!(
            "usecase-beta\n",
            "actor User\n",
            "Login[Sign in]\n",
            "User --> Login\n",
            "json Data@{\"enabled\":true,\"items\":[2,1]}\n",
            "note for Login \"Try again\"\n",
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("usecase");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_usecase_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.title.as_deref(), Some("Authentication"));
        assert_eq!(model.nodes[0].kind, UsecaseNodeKind::Actor);
        assert_eq!(model.nodes[1].label, "Sign in");
        assert_eq!(model.relationships[0].source, "User");
        assert_eq!(model.relationships[0].target, "Login");
        assert_eq!(model.notes[0].target, "Login");
        assert_eq!(model.json_nodes[0].value["enabled"], true);
        assert_eq!(model.json_nodes[0].value["items"][0], 2);
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("usecase"),
            1,
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full_metadata = crate::PresentationMetadata {
                diagram_type: metadata.diagram_type.clone(),
                config: metadata.config.clone(),
                effective_config: crate::presentation::PresentationConfig::from_config(
                    metadata.semantic_config.clone(),
                ),
                title: metadata.title.clone(),
            };
            parse_usecase_model_for_render_controlled(
                source,
                &full_metadata,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());

            #[cfg(feature = "compat-json")]
            {
                parse_usecase(source, &full_metadata).unwrap();
                #[cfg(feature = "editor-facts")]
                assert_eq!(crate::editor::test_work::read(), Default::default());
            }
        }
    }

    #[test]
    fn semantic_errors_keep_exact_spans_and_observe_cancellation() {
        let source = "usecase-beta\nactor User\nA -->";
        let metadata = metadata();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_usecase_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured parse diagnostic");
        };
        let start = source.len();
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start)),);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_usecase_semantic_model_controlled(
                source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
    }
}
