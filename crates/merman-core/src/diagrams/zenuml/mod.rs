//! Grammar-derived ZenUML parser and family-owned semantic model.
//!
//! The selected Mermaid 11.16 companion oracle is ZenUML Core 3.47.8. Its
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

pub fn parse_zenuml_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
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
    let parsed = parser::parse(code, tokens);
    let semantic::SemanticBuild {
        model,
        editor_facts,
        diagnostics,
    } = semantic::build(parsed);
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
}
