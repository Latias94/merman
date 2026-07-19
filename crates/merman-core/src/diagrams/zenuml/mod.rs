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

use crate::{EditorSemanticFacts, Error, ParseMetadata, Result};
use serde_json::Value;

pub(crate) use model::render_model_to_compat_json;
pub use model::{
    ZenumlDiagramRenderModel, ZenumlFragmentKind, ZenumlFragmentSection, ZenumlGroup,
    ZenumlMessageStyle, ZenumlParticipant, ZenumlStatement, ZenumlStatementKind,
};

struct ZenumlSemanticSource {
    model: ZenumlDiagramRenderModel,
    editor_facts: EditorSemanticFacts,
    first_diagnostic: Option<ast::SyntaxDiagnostic>,
}

pub fn parse_zenuml(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let source = parse_semantic_source(code, meta)?;
    model::render_model_to_compat_json(&source.model, meta)
}

pub fn parse_zenuml_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<ZenumlDiagramRenderModel> {
    Ok(parse_semantic_source(code, meta)?.model)
}

pub(crate) fn parse_zenuml_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let source = parse_semantic_source(code, meta)?;
    let value = model::render_model_to_compat_json(&source.model, meta)?;
    Ok((value, source.editor_facts))
}

pub(crate) fn parse_zenuml_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    construct_semantic_source(code).editor_facts
}

fn parse_semantic_source(code: &str, meta: &ParseMetadata) -> Result<ZenumlSemanticSource> {
    let source = construct_semantic_source(code);
    if let Some(diagnostic) = &source.first_diagnostic {
        return Err(Error::diagram_parse_exact(
            meta.diagram_type.clone(),
            diagnostic.message.clone(),
            diagnostic.span,
        ));
    }
    Ok(source)
}

fn construct_semantic_source(code: &str) -> ZenumlSemanticSource {
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("zenuml");

    let tokens = lexer::lex(code);
    let parsed = parser::parse(code, &tokens);
    let lexemes = lexer::editor_lexemes(code, &tokens, !parsed.diagnostics.is_empty());
    let semantic::SemanticBuild {
        model,
        mut editor_facts,
        diagnostics,
    } = semantic::build(parsed);
    editor_facts.replace_family_lexemes(lexemes);
    ZenumlSemanticSource {
        model,
        editor_facts,
        first_diagnostic: diagnostics.into_iter().next(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EditorSemanticCompleteness, MermaidConfig, RenderSemanticModel};

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "zenuml".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
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
    fn shared_accessibility_terminals_feed_the_owned_model() {
        let model = parse_zenuml_model_for_render(
            "zenuml\naccTitle:  Order   service\naccDescr {\n  Creates orders\n  and invoices\n}\nA.call()\n",
            &meta(),
        )
        .unwrap();

        assert_eq!(model.acc_title.as_deref(), Some("Order service"));
        assert_eq!(
            model.acc_descr.as_deref(),
            Some("Creates orders\nand invoices")
        );
    }

    #[test]
    fn unterminated_accessibility_description_is_strictly_rejected() {
        let error = parse_zenuml_model_for_render("zenuml\naccDescr {\n  incomplete\n", &meta())
            .unwrap_err();
        assert!(error.to_string().contains("unterminated accDescr block"));

        let facts = parse_zenuml_editor_facts("zenuml\naccDescr {\n  incomplete\n", &meta());
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "accDescr")
        );
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
        let ZenumlStatementKind::Message { from, to, .. } = &model.statements[0].kind else {
            panic!("expected message");
        };
        assert_eq!((from.as_str(), to.as_str()), ("A", "B"));
        let ZenumlStatementKind::Message { assignment, to, .. } = &model.statements[1].kind else {
            panic!("expected message");
        };
        assert_eq!(assignment.as_deref(), Some("result"));
        assert_eq!(to, "Service");
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
            ZenumlStatementKind::Message { to, label, .. } if to == "A" && label == "B"
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
}
