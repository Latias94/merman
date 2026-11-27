//! Requirement source diagnostics and semantic projections.

#[cfg(any(test, feature = "editor-facts"))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol,
};
use crate::{Error, OperationControl, OperationControlResult, family::CombinedSemanticFailure};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_graph::requirement::{
    RequirementEntityKind, RequirementFieldKind, RequirementObservation,
};
use merman_core_graph::requirement::{
    RequirementIssueLocation, parse_requirement_source_controlled,
};
use merman_model::families::requirement::RequirementDiagramModel;
#[cfg(all(test, feature = "presentation"))]
pub(crate) use merman_model::families::requirement::RequirementNode;
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_requirement_syntax_construction_count() {
    REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn requirement_syntax_construction_count() -> usize {
    REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.get()
}

pub(crate) fn sanitize_common_db_fields(
    model: &mut RequirementDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_requirement(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model = parse_requirement_semantic_source(code, meta)?.model;
    render_model_to_compat_json(&model, meta)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_requirement_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<RequirementDiagramModel> {
    Ok(parse_requirement_semantic_source(code, meta)?.model)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_requirement_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<RequirementDiagramModel>> {
    let construction = construct_requirement_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => Ok(Ok(source.model)),
        Err(failure) => Ok(Err(failure.into_error())),
    }
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_requirement_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_requirement_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    Ok(crate::family::CombinedSemanticParse::from_construction(
        construction,
        |RequirementSemanticSource {
             model,
             editor_facts,
         }| (render_model_to_compat_json(&model, meta), editor_facts),
        CombinedSemanticFailure::into_parts,
    ))
}

pub(crate) fn parse_requirement_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<RequirementDiagramModel>> {
    let construction = construct_requirement_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = editor_demand
                .needs_recovery()
                .then_some(source.editor_facts);
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            crate::family::SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            {
                match editor_demand {
                    EditorFactsDemand::Capture | EditorFactsDemand::Recovery => {
                        let (error, facts) = failure.into_parts();
                        crate::family::SemanticConstruction {
                            model: Err(error),
                            editor_facts: Some(facts),
                        }
                    }
                    EditorFactsDemand::Omit => crate::family::SemanticConstruction {
                        model: Err(failure.into_error()),
                        editor_facts: None,
                    },
                }
            }
            #[cfg(not(feature = "editor-facts"))]
            {
                crate::family::SemanticConstruction {
                    model: Err(failure.into_error()),
                }
            }
        }
    })
}

struct RequirementSemanticSource {
    model: RequirementDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &RequirementDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::requirement::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_requirement_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<RequirementSemanticSource> {
    construct_requirement_semantic_source(code, meta).map_err(CombinedSemanticFailure::into_error)
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_requirement_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> std::result::Result<RequirementSemanticSource, CombinedSemanticFailure> {
    construct_requirement_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_requirement_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<RequirementSemanticSource, CombinedSemanticFailure>>
{
    control.checkpoint()?;
    #[cfg(test)]
    REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.set(REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    #[cfg(feature = "editor-facts")]
    let mut editor_facts = EditorSemanticFacts::new();
    #[cfg(feature = "editor-facts")]
    let mut observe = |event: RequirementObservation<'_>| {
        project_requirement_observation(&mut editor_facts, event);
    };
    let model = parse_requirement_source_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand.captures_rich(),
        #[cfg(feature = "editor-facts")]
        editor_demand
            .needs_recovery()
            .then_some(&mut observe as &mut dyn for<'a> FnMut(RequirementObservation<'a>)),
        control,
    )?;
    Ok(match model {
        Ok(model) => Ok(RequirementSemanticSource {
            model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => {
            let error = match issue.location {
                RequirementIssueLocation::Exact(span) => {
                    Error::diagram_parse_exact(diagram_type, issue.message, span)
                }
                RequirementIssueLocation::InsertionPoint(offset) => {
                    Error::diagram_parse_insertion_point(diagram_type, issue.message, offset)
                }
            };
            Err(CombinedSemanticFailure::parser_recovery(
                #[cfg(feature = "editor-facts")]
                "requirement",
                error,
                #[cfg(feature = "editor-facts")]
                editor_demand.needs_recovery().then_some(editor_facts),
            ))
        }
    })
}

#[cfg(feature = "editor-facts")]
fn project_requirement_observation(
    facts: &mut EditorSemanticFacts,
    event: RequirementObservation<'_>,
) {
    use RequirementObservation::*;

    match event {
        CaptureStarted => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
        }
        Direction {
            value,
            span,
            selection,
        } => {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::CardinalDirectionValue,
                selection,
            ));
            if let Some(value) = value {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    value,
                    Some("requirement direction".to_string()),
                    EditorSemanticKind::String,
                    span,
                    selection,
                ));
            }
        }
        Accessibility {
            directive,
            value,
            span,
            complete,
        } => {
            facts.push_directive_prefix(directive);
            if let Some((text, selection)) = value {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    selection,
                ));
                let detail = if directive == "accTitle" {
                    "requirement accessibility title"
                } else {
                    "requirement accessibility description"
                };
                facts.push_symbol(EditorSemanticSymbol::payload(
                    text,
                    Some(detail.to_string()),
                    EditorSemanticKind::String,
                    span,
                    selection,
                ));
            } else if !complete {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    SourceSpan::new(span.end, span.end),
                ));
            }
        }
        Field {
            kind,
            text,
            span,
            selection,
        } => {
            project_requirement_field(facts, kind, text, span, selection);
        }
        IncompleteAccessibility(span) => facts.mark_recovered_from_parse_error(
            "requirement parser recovered from incomplete accessibility directive",
            Some(span),
        ),
        ResolveReferences(entities) => {
            for symbol in &mut facts.symbols {
                if symbol.role == EditorSemanticRole::Reference
                    && let Some(kind) = entities.get(&symbol.name)
                {
                    symbol.kind = match kind {
                        RequirementEntityKind::Requirement => EditorSemanticKind::Struct,
                        RequirementEntityKind::Element => EditorSemanticKind::Object,
                    };
                }
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
fn project_requirement_field(
    facts: &mut EditorSemanticFacts,
    field: RequirementFieldKind<'_>,
    text: &str,
    span: SourceSpan,
    selection: SourceSpan,
) {
    use EditorSemanticKind::{Object, Property, String, Struct};
    use EditorSemanticRole::{ClassDefinition, Entity, Payload, Reference};
    use RequirementFieldKind::*;

    let (detail, kind, role) = match field {
        Requirement(ty) => (ty.to_lowercase(), Struct, Entity),
        Element => ("requirement element".to_string(), Object, Entity),
        Class => ("requirement class".to_string(), Property, Payload),
        ClassTarget => ("requirement class target".to_string(), Struct, Reference),
        RequirementFieldKind::ClassDefinition => (
            "requirement class definition".to_string(),
            Property,
            ClassDefinition,
        ),
        StyleTarget => ("requirement style target".to_string(), Struct, Reference),
        Style => ("requirement style".to_string(), Property, Payload),
        ClassStyle => ("requirement class style".to_string(), Property, Payload),
        Relationship => ("requirement relationship".to_string(), String, Payload),
        RelationshipSource => (
            "requirement relationship source".to_string(),
            Struct,
            Reference,
        ),
        RelationshipTarget => (
            "requirement relationship target".to_string(),
            Struct,
            Reference,
        ),
        Id => ("requirement id".to_string(), String, Payload),
        Text => ("requirement text".to_string(), String, Payload),
        Risk => ("requirement risk".to_string(), String, Payload),
        VerifyMethod => ("requirement verify method".to_string(), String, Payload),
        ElementType => ("requirement element type".to_string(), String, Payload),
        DocRef => ("requirement doc ref".to_string(), String, Payload),
    };
    let expected = match field {
        Requirement(_) | Element => Some(EditorExpectedSyntaxKind::NodeIdentifier),
        Id | Text | Risk | VerifyMethod | ElementType | DocRef => {
            Some(EditorExpectedSyntaxKind::Payload)
        }
        _ => None,
    };
    if let Some(expected) = expected {
        facts.push_expected_syntax(EditorExpectedSyntax::new(expected, selection));
    }
    facts.push_symbol(EditorSemanticSymbol::with_role(
        text,
        Some(detail),
        kind,
        role,
        span,
        selection,
    ));
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::EditorSemanticCompleteness;
    use crate::{MermaidConfig, MermaidEngine, ParseDiagnosticSpanKind, ParseOptions};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap_err()
            .to_string()
    }

    fn test_meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "requirement".to_string(),
            config: MermaidConfig::default(),
            effective_config: crate::presentation::SourcePresentationConfig::default(),
            title: None,
        }
    }

    #[test]
    fn requirement_quoted_names_keep_relationship_and_class_delimiters_literal() {
        let source = concat!(
            "requirementDiagram\n",
            "element \"a-b:::c<-d->e\" {\n}\n",
            "requirement req {\n}\n",
            "\"a-b:::c<-d->e\" - traces -> req\n",
            "req <- traces - \"a-b:::c<-d->e\"\n",
        );
        let model = parse(source);
        assert_eq!(model["elements"][0]["name"], "a-b:::c<-d->e");
        for relation in model["relationships"].as_array().unwrap() {
            assert_eq!(relation["src"], "a-b:::c<-d->e");
            assert_eq!(relation["dst"], "req");
        }
    }

    #[test]
    fn requirement_unquoted_tokens_keep_embedded_comment_markers() {
        let source = concat!(
            "requirementDiagram\r\n",
            "requirement R#tag {\r\n",
            "  id: REQ#7%%rev\r\n",
            "  text: Guide 雪#章 %%literal\r\n",
            "}\r\n",
            "element Doc%%rev {\r\n",
            "  type: 5\" 雪#screen%%rev\r\n",
            "  docRef: guide#section%%tail\r\n",
            "}\r\n",
            "R#tag - traces -> Doc%%rev\r\n",
        );
        let parser = crate::SemanticParser::new();
        let parsed = parser.parse_diagram_sync(source).unwrap();
        let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model else {
            panic!("expected Requirement model");
        };
        assert_eq!(model.requirements[0].name, "R#tag");
        assert_eq!(model.requirements[0].requirement_id, "REQ#7%%rev");
        assert_eq!(model.requirements[0].text, "Guide 雪#章 %%literal");
        assert_eq!(model.elements[0].name, "Doc%%rev");
        assert_eq!(model.elements[0].element_type, "5\" 雪#screen%%rev");
        assert_eq!(model.elements[0].doc_ref, "guide#section%%tail");
        assert_eq!(model.relationships[0].src, "R#tag");
        assert_eq!(model.relationships[0].dst, "Doc%%rev");

        #[cfg(feature = "editor-facts")]
        {
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(source)
            else {
                panic!("expected Requirement snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            assert!(matches!(outcome, crate::SemanticParseOutcome::Parsed(_)));
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Requirement editor facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            for (detail, value) in [
                ("requirement id", "REQ#7%%rev"),
                ("requirement text", "Guide 雪#章 %%literal"),
                ("requirement element type", "5\" 雪#screen%%rev"),
                ("requirement doc ref", "guide#section%%tail"),
            ] {
                let selection = payload_selection(&facts, detail, value);
                let start = source.find(value).unwrap();
                assert_eq!(selection, SourceSpan::new(start, start + value.len()));
                assert_eq!(&source[selection.start..selection.end], value);
                assert!(facts.expected_syntax.iter().any(|syntax| {
                    syntax.kind == EditorExpectedSyntaxKind::Payload && syntax.span == selection
                }));
            }
            for name in ["R#tag", "Doc%%rev"] {
                let symbols: Vec<_> = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.name == name)
                    .collect();
                assert_eq!(symbols.len(), 2);
                assert_eq!(symbols[0].role, EditorSemanticRole::Entity);
                assert_eq!(symbols[1].role, EditorSemanticRole::Reference);
                for symbol in symbols {
                    assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
                }
            }
        }
    }

    #[test]
    fn requirement_quoted_body_values_preserve_physical_line_endings_and_comments() {
        let source = concat!(
            "requirementDiagram # header comment\r\n",
            "# boundary comment: \"\r\n",
            "requirement req {\r\n",
            " id: \"REQ#%%\" # trailing comment: \"\r\n",
            " text: \"First\r\nSecond %% # café\" %% trailing comment: \"\r\n",
            " risk: high# boundary comment\r\n",
            " verifyMethod: test%% boundary comment\r\n",
            "} # closing comment\r\n",
            "element doc {\r\n",
            " type: \"One\r\nTwo\" # trailing comment: \"\r\n",
            " docRef: \"guide#section%%rev\" %% trailing comment\r\n",
            "}\r\n",
            "style doc fill:#fff\r\n",
        );
        let model = parse(source);
        assert_eq!(model["requirements"][0]["requirementId"], "REQ#%%");
        assert_eq!(model["requirements"][0]["text"], "First\nSecond %% # café");
        assert_eq!(model["requirements"][0]["risk"], "High");
        assert_eq!(model["requirements"][0]["verifyMethod"], "Test");
        assert_eq!(model["elements"][0]["type"], "One\nTwo");
        assert_eq!(model["elements"][0]["docRef"], "guide#section%%rev");
        assert_eq!(model["elements"][0]["cssStyles"], json!(["fill:#fff"]));
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_requirement_json_and_editor_facts,
                source,
                &test_meta(),
            );
            let literal = "\"First\r\nSecond %% # café\"";
            let selection = payload_selection(&facts, "requirement text", literal);
            assert_eq!(&source[selection.start..selection.end], literal);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            assert!(
                !facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name.contains("comment"))
            );
        }
        assert!(
            parse_err("requirementDiagram\nrequirement req {\ntext: \"First\nSecond\n}\n")
                .contains("unterminated")
        );
    }

    #[test]
    fn requirement_unquoted_inches_do_not_start_a_multiline_string() {
        let model = parse(
            "requirementDiagram\nrequirement r {\n text: 5\" display\n}\nelement next {\n}\n",
        );
        assert_eq!(model["requirements"][0]["text"], "5\" display");
        assert_eq!(model["elements"][0]["name"], "next");

        let model = parse(
            "requirementDiagram\nelement a\"b:::hot {\n}\nrequirement r {\n}\na\"b - traces -> r\n",
        );
        assert_eq!(model["elements"][0]["name"], "a\"b");
        assert_eq!(model["elements"][0]["classes"], json!(["default", "hot"]));
        assert_eq!(model["relationships"][0]["src"], "a\"b");

        for comment in ["# note: \"", "%% note: \""] {
            let model = parse(&format!(
                "requirementDiagram\nrequirement r {{\n{comment}\ntext: retained\n}}\nelement next {{\n}}\n"
            ));
            assert_eq!(model["requirements"][0]["text"], "retained");
            assert_eq!(model["elements"][0]["name"], "next");
        }
    }

    #[test]
    fn requirement_keyword_prefixes_keep_unquoted_token_boundaries() {
        let source = concat!(
            "requirementDiagram\n",
            "requirement textid\":::critical {\n",
            "id: DocRefId#7\n",
            "text: testid#section\n",
            "}\n",
            "element testid\" {\n",
            "docRef: textid%%anchor\n",
            "}\n",
            "testid\" - traces -> textid\"\n",
        );
        let model = parse(source);
        assert_eq!(model["requirements"][0]["name"], "textid\"");
        assert_eq!(
            model["requirements"][0]["classes"],
            json!(["default", "critical"])
        );
        assert_eq!(model["requirements"][0]["requirementId"], "DocRefId#7");
        assert_eq!(model["requirements"][0]["text"], "testid#section");
        assert_eq!(model["elements"][0]["name"], "testid\"");
        assert_eq!(model["elements"][0]["docRef"], "textid%%anchor");
        assert_eq!(model["relationships"][0]["src"], "testid\"");
        assert_eq!(model["relationships"][0]["dst"], "textid\"");

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_requirement_json_and_editor_facts,
                source,
                &test_meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            for (detail, value) in [
                ("requirement id", "DocRefId#7"),
                ("requirement text", "testid#section"),
                ("requirement doc ref", "textid%%anchor"),
            ] {
                let selection = payload_selection(&facts, detail, value);
                assert_eq!(&source[selection.start..selection.end], value);
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    fn payload_selection(facts: &EditorSemanticFacts, detail: &str, name: &str) -> SourceSpan {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some(detail) && symbol.name == name)
            .unwrap_or_else(|| panic!("missing payload symbol {detail:?} {name:?}"))
            .selection
    }

    fn statement_occurrences(source: &str, statement: &str, needle: &str) -> Vec<SourceSpan> {
        let statement_start = source
            .find(statement)
            .unwrap_or_else(|| panic!("missing statement {statement:?}"));
        statement
            .match_indices(needle)
            .map(|(start, value)| {
                SourceSpan::new(
                    statement_start + start,
                    statement_start + start + value.len(),
                )
            })
            .collect()
    }

    #[cfg(feature = "editor-facts")]
    fn statement_symbol_selections(
        facts: &EditorSemanticFacts,
        source: &str,
        statement: &str,
        detail: &str,
    ) -> Vec<SourceSpan> {
        let statement_start = source
            .find(statement)
            .unwrap_or_else(|| panic!("missing statement {statement:?}"));
        let statement_span = SourceSpan::new(statement_start, statement_start + statement.len());
        let mut selections = facts
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.detail.as_deref() == Some(detail)
                    && symbol.selection.start >= statement_span.start
                    && symbol.selection.end <= statement_span.end
            })
            .map(|symbol| symbol.selection)
            .collect::<Vec<_>>();
        selections.sort_by_key(|span| (span.start, span.end));
        selections
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_semantic_symbols_use_parser_spans_when_values_repeat() {
        let source = concat!(
            "requirementDiagram\n",
            "requirement same:::same,same {\n",
            "}\n",
            "short:::short,short\n",
            "style paint,paint paint:paint,paint:paint\n",
            "classDef defc,defc defc:defc,defc:defc\n",
            "class cls,cls cls,cls\n",
            "verifies - verifies -> verifies\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_requirement_json_and_editor_facts,
            source,
            &test_meta(),
        );

        let definition = "requirement same:::same,same {";
        let definition_same = statement_occurrences(source, definition, "same");
        assert_eq!(
            statement_symbol_selections(&facts, source, definition, "requirement"),
            vec![definition_same[0]],
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, definition, "requirement class"),
            definition_same[1..].to_vec(),
        );

        let shorthand = "short:::short,short";
        let shorthand_same = statement_occurrences(source, shorthand, "short");
        assert_eq!(
            statement_symbol_selections(&facts, source, shorthand, "requirement class target"),
            vec![shorthand_same[0]],
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, shorthand, "requirement class"),
            shorthand_same[1..].to_vec(),
        );

        let style = "style paint,paint paint:paint,paint:paint";
        let style_same = statement_occurrences(source, style, "paint");
        assert_eq!(
            statement_symbol_selections(&facts, source, style, "requirement style target"),
            style_same[..2].to_vec(),
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, style, "requirement style"),
            statement_occurrences(source, style, "paint:paint"),
        );

        let class_def = "classDef defc,defc defc:defc,defc:defc";
        let class_def_same = statement_occurrences(source, class_def, "defc");
        assert_eq!(
            statement_symbol_selections(&facts, source, class_def, "requirement class definition",),
            class_def_same[..2].to_vec(),
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, class_def, "requirement class style",),
            statement_occurrences(source, class_def, "defc:defc"),
        );

        let class = "class cls,cls cls,cls";
        let class_same = statement_occurrences(source, class, "cls");
        assert_eq!(
            statement_symbol_selections(&facts, source, class, "requirement class target"),
            class_same[..2].to_vec(),
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, class, "requirement class"),
            class_same[2..].to_vec(),
        );

        let relationship = "verifies - verifies -> verifies";
        let verifies = statement_occurrences(source, relationship, "verifies");
        assert_eq!(
            statement_symbol_selections(
                &facts,
                source,
                relationship,
                "requirement relationship source",
            ),
            vec![verifies[0]],
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, relationship, "requirement relationship",),
            vec![verifies[1]],
        );
        assert_eq!(
            statement_symbol_selections(
                &facts,
                source,
                relationship,
                "requirement relationship target",
            ),
            vec![verifies[2]],
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_targets_use_reference_roles_and_resolve_forward_definition_kinds() {
        let source = concat!(
            "requirementDiagram\n",
            "future - verifies -> later\n",
            "style future,later fill:#fff\n",
            "class future,later important\n",
            "future:::important\n",
            "element future {\n",
            "}\n",
            "requirement later {\n",
            "}\n",
            "classDef important fill:#fff\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_requirement_json_and_editor_facts,
            source,
            &test_meta(),
        );
        let symbols_for = |name: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .collect::<Vec<_>>()
        };

        let future = symbols_for("future");
        assert_eq!(future.len(), 5);
        assert!(
            future[..4]
                .iter()
                .all(|symbol| symbol.role == EditorSemanticRole::Reference)
        );
        assert!(
            future
                .iter()
                .all(|symbol| symbol.kind == EditorSemanticKind::Object)
        );
        assert_eq!(future[4].role, EditorSemanticRole::Entity);

        let later = symbols_for("later");
        assert_eq!(later.len(), 4);
        assert!(
            later[..3]
                .iter()
                .all(|symbol| symbol.role == EditorSemanticRole::Reference)
        );
        assert!(
            later
                .iter()
                .all(|symbol| symbol.kind == EditorSemanticKind::Struct)
        );
        assert_eq!(later[3].role, EditorSemanticRole::Entity);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_reference_kind_resolution_does_not_inspect_display_detail() {
        let span = SourceSpan::new(0, "node".len());
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::reference(
            "node",
            Some("definition-looking display text".to_string()),
            EditorSemanticKind::Struct,
            span,
            span,
        ));

        let mut observe = |event: RequirementObservation<'_>| {
            if let RequirementObservation::ResolveReferences(_) = event {
                project_requirement_observation(&mut facts, event);
            }
        };
        parse_requirement_source_controlled(
            "requirementDiagram\nelement node {\n}\n",
            true,
            Some(&mut observe),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(facts.symbols[0].role, EditorSemanticRole::Reference);
        assert_eq!(facts.symbols[0].kind, EditorSemanticKind::Object);
    }

    #[test]
    fn requirement_entrypoints_construct_one_semantic_source() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "requirementDiagram\n",
            "requirement login {\n",
            "  id: REQ-1\n",
            "  risk: high\n",
            "  verifymethod: test\n",
            "}\n",
        );

        reset_requirement_syntax_construction_count();
        engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(requirement_syntax_construction_count(), 1);

        reset_requirement_syntax_construction_count();
        engine
            .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(requirement_syntax_construction_count(), 1);

        #[cfg(feature = "editor-facts")]
        {
            reset_requirement_syntax_construction_count();
            engine
                .parse_editor_semantic_facts_with_type_sync("requirement", text)
                .unwrap()
                .unwrap();
            assert_eq!(requirement_syntax_construction_count(), 1);
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_combined_projection_constructs_once_and_matches_standalone_entrypoints() {
        let text = concat!(
            "requirementDiagram\n",
            "accTitle: Login requirements\n",
            "direction LR\n",
            "requirement login:::critical {\n",
            "  id: REQ-1\n",
            "  text: Login must work\n",
            "  risk: high\n",
            "  verifymethod: test\n",
            "}\n",
            "element api {\n",
            "  type: service\n",
            "}\n",
            "classDef critical fill:#f9f\n",
            "login - verifies -> api\n",
        );
        let mut meta = test_meta();
        meta.effective_config = crate::presentation::SourcePresentationConfig::from_value(json!({
            "theme": "forest",
            "securityLevel": "strict"
        }));
        let standalone_json = parse_requirement(text, &meta).unwrap();
        reset_requirement_syntax_construction_count();
        let (combined_json, combined_editor) = crate::family::test_support::into_result(
            parse_requirement_json_and_editor_facts(text, &meta, &OperationControl::new()),
        )
        .unwrap();

        assert_eq!(requirement_syntax_construction_count(), 1);
        assert_eq!(combined_json, standalone_json);
        assert!(!combined_editor.symbols.is_empty());
    }

    #[test]
    fn requirement_typed_projection_matches_compatibility_json() {
        let text = concat!(
            "requirementDiagram\n",
            "accTitle: Login requirements\n",
            "direction LR\n",
            "requirement login {\n",
            "  id: REQ-1\n",
            "  text: Login must work\n",
            "  risk: high\n",
            "  verifymethod: test\n",
            "}\n",
            "element api {\n",
            "  type: service\n",
            "}\n",
            "login - verifies -> api\n",
        );
        let meta = test_meta();
        let compat = parse_requirement(text, &meta).unwrap();
        let typed = parse_requirement_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["type"], "requirement");
        assert_eq!(compat["config"], meta.effective_config.as_value().clone());
        assert!(compat["accDescr"].is_null());
    }

    #[test]
    fn requirement_malformed_recovery_reuses_partial_statement_facts_and_exact_span() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "requirementDiagram\n",
            "requirement login {\n",
            "  id: REQ-1\n",
            "  risk: impossible\n",
            "}\n",
        );
        let bad_value_start = text.find("impossible").unwrap();

        reset_requirement_syntax_construction_count();
        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("invalid risk must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("invalid risk returned a non-parse error");
        };
        assert_eq!(requirement_syntax_construction_count(), 1);
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(
                bad_value_start,
                bad_value_start + "impossible".len(),
            ))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

        #[cfg(feature = "editor-facts")]
        {
            reset_requirement_syntax_construction_count();
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("requirement", text)
                .unwrap()
                .unwrap();
            assert_eq!(requirement_syntax_construction_count(), 1);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "login"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "REQ-1"));
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.span
                    == Some(SourceSpan::new(
                        bad_value_start,
                        bad_value_start + "impossible".len(),
                    ))
            }));
        }
    }

    #[test]
    fn requirement_full_requirement_definition_is_parsed() {
        let model = parse(
            r#"requirementDiagram

requirement test_req {
  id: test_id
  text: the test text.
  risk: high
  verifymethod: analysis
}
"#,
        );

        assert_eq!(model["requirements"].as_array().unwrap().len(), 1);
        assert_eq!(model["requirements"][0]["name"], json!("test_req"));
        assert_eq!(model["requirements"][0]["type"], json!("Requirement"));
        assert_eq!(model["requirements"][0]["requirementId"], json!("test_id"));
        assert_eq!(model["requirements"][0]["text"], json!("the test text."));
        assert_eq!(model["requirements"][0]["risk"], json!("High"));
        assert_eq!(model["requirements"][0]["verifyMethod"], json!("Analysis"));
        assert_eq!(model["elements"].as_array().unwrap().len(), 0);
        assert_eq!(model["relationships"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn requirement_full_element_definition_is_parsed() {
        let model = parse(
            r#"requirementDiagram

element test_el {
  type: test_type
  docref: test_ref
}
"#,
        );

        assert_eq!(model["requirements"].as_array().unwrap().len(), 0);
        assert_eq!(model["elements"].as_array().unwrap().len(), 1);
        assert_eq!(model["elements"][0]["name"], json!("test_el"));
        assert_eq!(model["elements"][0]["type"], json!("test_type"));
        assert_eq!(model["elements"][0]["docRef"], json!("test_ref"));
        assert_eq!(model["relationships"].as_array().unwrap().len(), 0);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_editor_payload_spans_point_to_values_when_values_match_keys() {
        let text = r#"requirementDiagram
accTitle: accTitle
accDescr: accDescr
requirement req {
  id: id
  text: text
  risk: risk
  verifymethod: verifymethod
}
element el {
  type: type
  docref: docref
}
"#;
        let facts = crate::family::test_support::editor_facts(
            parse_requirement_json_and_editor_facts,
            text,
            &test_meta(),
        );

        for (detail, name, needle) in [
            (
                "requirement accessibility title",
                "accTitle",
                "accTitle: accTitle",
            ),
            (
                "requirement accessibility description",
                "accDescr",
                "accDescr: accDescr",
            ),
            ("requirement id", "id", "id: id"),
            ("requirement text", "text", "text: text"),
            ("requirement risk", "risk", "risk: risk"),
            (
                "requirement verify method",
                "verifymethod",
                "verifymethod: verifymethod",
            ),
            ("requirement element type", "type", "type: type"),
            ("requirement doc ref", "docref", "docref: docref"),
        ] {
            let value_start = text.find(needle).unwrap() + needle.rfind(name).unwrap();
            assert_eq!(
                payload_selection(&facts, detail, name),
                SourceSpan::new(value_start, value_start + name.len()),
                "wrong span for {detail}"
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_definition_spans_point_to_name_tokens_when_names_prefix_keywords() {
        let text = concat!(
            "requirementDiagram\n",
            "  requirement require {\n",
            "  }\n",
            "  element elem {\n",
            "  }\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_requirement_json_and_editor_facts,
            text,
            &test_meta(),
        );

        for (detail, name, declaration) in [
            ("requirement", "require", "  requirement require {"),
            ("requirement element", "elem", "  element elem {"),
        ] {
            let declaration_start = text.find(declaration).unwrap();
            let name_start = declaration_start + declaration.rfind(name).unwrap();
            let expected = SourceSpan::new(name_start, name_start + name.len());
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some(detail) && symbol.name == name)
                .unwrap_or_else(|| panic!("missing definition symbol {detail:?} {name:?}"));

            assert_eq!(symbol.role, EditorSemanticRole::Entity);
            assert_eq!(symbol.rename_policy, crate::EditorRenamePolicy::Identifier);
            assert_eq!(symbol.selection, expected);
            assert_eq!(&text[symbol.selection.start..symbol.selection.end], name);
            assert!(facts.expected_syntax.iter().any(|syntax| {
                syntax.kind == EditorExpectedSyntaxKind::NodeIdentifier && syntax.span == expected
            }));
        }
    }

    #[test]
    fn requirement_acc_title_and_acc_descr_are_parsed() {
        let model = parse(
            r#"requirementDiagram
accTitle: test title
accDescr: my chart description
element test_name {
  type: test_type
  docref: test_ref
}
"#,
        );

        assert_eq!(model["accTitle"], json!("test title"));
        assert_eq!(model["accDescr"], json!("my chart description"));
    }

    #[test]
    fn requirement_multiline_acc_descr_is_parsed() {
        let model = parse(
            r#"requirementDiagram
accTitle: test title
accDescr {
  my chart description
line 2
}
element test_name {
  type: test_type
  docref: test_ref
}
"#,
        );

        assert_eq!(model["accTitle"], json!("test title"));
        assert_eq!(model["accDescr"], json!("my chart description\nline 2"));
    }

    #[test]
    fn requirement_relationship_is_parsed() {
        let model = parse(
            r#"requirementDiagram

a - contains -> b
"#,
        );
        assert_eq!(model["relationships"].as_array().unwrap().len(), 1);
        assert_eq!(model["relationships"][0]["type"], json!("contains"));
        assert_eq!(model["relationships"][0]["src"], json!("a"));
        assert_eq!(model["relationships"][0]["dst"], json!("b"));
    }

    #[test]
    fn requirement_relationship_left_arrow_is_parsed() {
        let model = parse(
            r#"requirementDiagram

a <- contains - b
"#,
        );
        assert_eq!(model["relationships"].as_array().unwrap().len(), 1);
        assert_eq!(model["relationships"][0]["type"], json!("contains"));
        assert_eq!(model["relationships"][0]["src"], json!("b"));
        assert_eq!(model["relationships"][0]["dst"], json!("a"));
    }

    #[test]
    fn requirement_proto_and_constructor_ids_are_accepted() {
        for id in ["__proto__", "constructor"] {
            let model = parse(&format!(
                r#"requirementDiagram
requirement {id} {{
  id: 1
  text: the test text.
  risk: high
  verifymethod: test
}}
"#
            ));
            assert_eq!(model["requirements"].as_array().unwrap().len(), 1);
        }

        for id in ["__proto__", "constructor"] {
            let model = parse(&format!(
                r#"requirementDiagram
element {id} {{
  type: simulation
}}
"#
            ));
            assert_eq!(model["elements"].as_array().unwrap().len(), 1);
        }
    }

    #[test]
    fn requirement_style_statement_applies_to_requirement() {
        let model = parse(
            r#"requirementDiagram

requirement test_req {
}
style test_req fill:#f9f,stroke:#333,stroke-width:4px
"#,
        );

        assert_eq!(
            model["requirements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
    }

    #[test]
    fn requirement_style_statement_applies_to_element() {
        let model = parse(
            r#"requirementDiagram

element test_element {
}
style test_element fill:#f9f,stroke:#333,stroke-width:4px
"#,
        );

        assert_eq!(
            model["elements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
    }

    #[test]
    fn requirement_style_statement_applies_to_multiple_things() {
        let model = parse(
            r#"requirementDiagram

requirement test_requirement {
}
element test_element {
}
style test_requirement,test_element fill:#f9f,stroke:#333,stroke-width:4px
"#,
        );

        assert_eq!(
            model["requirements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
        assert_eq!(
            model["elements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
    }

    #[test]
    fn requirement_classdef_and_class_statement_are_parsed() {
        let model = parse(
            r#"requirementDiagram

requirement myReq {
}
classDef myClass fill:#f9f,stroke:#333,stroke-width:4px
class myReq myClass
"#,
        );

        assert_eq!(
            model["requirements"][0]["classes"],
            json!(["default", "myClass"])
        );
        assert_eq!(
            model["requirements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
        assert_eq!(model["classes"]["myClass"]["id"], json!("myClass"));
        assert_eq!(
            model["classes"]["myClass"]["styles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
        assert_eq!(model["classes"]["myClass"]["textStyles"], json!([]));
    }

    #[test]
    fn requirement_shorthand_class_statement_is_supported() {
        let model = parse(
            r#"requirementDiagram

requirement myReq {
}
classDef myClass fill:#f9f,stroke:#333,stroke-width:4px
myReq:::myClass
"#,
        );
        assert_eq!(
            model["requirements"][0]["classes"],
            json!(["default", "myClass"])
        );
    }

    #[test]
    fn requirement_shorthand_is_supported_in_definition() {
        let model = parse(
            r#"requirementDiagram

requirement myReq:::class1 {
}
element myElem:::class1,class2 {
}

classDef class1 fill:#f9f,stroke:#333,stroke-width:4px
classDef class2 color:blue
"#,
        );
        assert_eq!(
            model["requirements"][0]["classes"],
            json!(["default", "class1"])
        );
        assert_eq!(
            model["elements"][0]["classes"],
            json!(["default", "class1", "class2"])
        );
    }

    #[test]
    fn requirement_direction_is_parsed() {
        for dir in ["TB", "BT", "LR", "RL"] {
            let model = parse(&format!("requirementDiagram\n\ndirection {dir}\n"));
            assert_eq!(model["direction"], json!(dir));
        }
    }

    #[test]
    fn requirement_style_and_class_directives_require_exact_first_word() {
        for statement in ["styleNode fill:#f00", "classify myReq myClass"] {
            let message = parse_err(&format!("requirementDiagram\n{statement}\n"));
            assert!(
                message.contains("unexpected requirement statement"),
                "unexpected error for {statement:?}: {message}"
            );
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn requirement_default_whitespace_keeps_its_greedy_crossline_endpoint() {
        for ending in ["\n", "\r\n"] {
            for (blank, direction_wins) in
                [("", true), (" ", false), ("\t", false), ("\u{feff}", false)]
            {
                for statement in [
                    "accTitle: direction LR",
                    "accDescr: direction LR",
                    "accDescr{direction LR}",
                ] {
                    let source =
                        format!("requirementDiagram{ending}{blank}{ending} {statement}{ending}");
                    let parsed = crate::SemanticParser::new()
                        .parse_diagram_sync(&source)
                        .unwrap();
                    let merman_model::semantic::SemanticDiagramModel::Requirement(model) =
                        parsed.model
                    else {
                        panic!("expected Requirement model");
                    };
                    assert_eq!(model.direction, if direction_wins { "LR" } else { "TB" });
                    let payload = if statement.starts_with("accTitle") {
                        model.acc_title
                    } else {
                        model.acc_descr
                    };
                    assert_eq!(
                        payload.as_deref(),
                        if direction_wins {
                            None
                        } else {
                            Some("direction LR")
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn requirement_repeated_same_line_blocks_keep_total_checkpoint_work_linear() {
        for count in [128, 2048] {
            for block in [" accDescr{}", " accDescr{界}"] {
                for capture in [false, true] {
                    let source = format!("requirementDiagram\n{}\n", block.repeat(count));
                    let control = OperationControl::new();
                    control.cancel_after_checkpoints(count * 10 + 64);
                    let result = parse_requirement_semantic_model_controlled(
                        &source,
                        &metadata(),
                        #[cfg(feature = "editor-facts")]
                        if capture {
                            EditorFactsDemand::Capture
                        } else {
                            EditorFactsDemand::Omit
                        },
                        &control,
                    )
                    .expect("same-line token consumption must stay within linear checkpoint work");
                    let model = result.model.unwrap();
                    assert_eq!(
                        model.acc_descr.as_deref(),
                        if block.contains('界') {
                            Some("界")
                        } else {
                            None
                        }
                    );
                    #[cfg(feature = "editor-facts")]
                    if capture && block.contains('界') {
                        assert_eq!(result.editor_facts.unwrap().symbols.len(), count);
                    }
                    #[cfg(not(feature = "editor-facts"))]
                    let _ = capture;
                }
            }
        }
    }

    #[test]
    fn requirement_default_direction_tokens_keep_ordered_lexer_priority() {
        for ending in ["\n", "\r\n"] {
            for prefix in ["", " ", "\t", "\u{feff}", "\u{85}"] {
                for (statement, title, description) in [
                    ("accTitle: direction LR", Some("direction LR"), None),
                    ("accDescr: direction LR", None, Some("direction LR")),
                    ("accDescr{direction LR}", None, Some("direction LR")),
                ] {
                    let source = format!("requirementDiagram{ending}{prefix}{statement}{ending}");
                    let parsed = crate::SemanticParser::new()
                        .parse_diagram_sync(&source)
                        .unwrap();
                    let merman_model::semantic::SemanticDiagramModel::Requirement(model) =
                        parsed.model
                    else {
                        panic!("expected Requirement model");
                    };
                    assert_eq!(model.direction, if prefix.is_empty() { "TB" } else { "LR" });
                    assert_eq!(
                        model.acc_title.as_deref(),
                        if prefix.is_empty() { title } else { None }
                    );
                    assert_eq!(
                        model.acc_descr.as_deref(),
                        if prefix.is_empty() { description } else { None }
                    );
                }
            }
            for (statement, expected) in [
                ("prefix direction LR suffix", "LR"),
                ("direction\n\u{feff}LR", "LR"),
                ("prefix direction BT direction LR", "BT"),
                ("prefix direction LR direction TB", "TB"),
                ("prefix direction LRX", "LR"),
            ] {
                let source = format!("requirementDiagram\n{statement}\n").replace('\n', ending);
                let parsed = crate::SemanticParser::new()
                    .parse_diagram_sync(&source)
                    .unwrap();
                let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
                else {
                    panic!("expected Requirement model");
                };
                assert_eq!(model.direction, expected);
            }
            for statement in ["accTitle\n:direction LR", "accDescr\n{direction LR}"] {
                let source = format!("requirementDiagram\n{statement}\n").replace('\n', ending);
                let parsed = crate::SemanticParser::new()
                    .parse_diagram_sync(&source)
                    .unwrap();
                let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
                else {
                    panic!("expected Requirement model");
                };
                assert_eq!(model.direction, "TB");
            }
            for statement in [
                "title direction LR",
                "title\ndirection LR",
                "prefix\u{2028}direction LR",
            ] {
                let source = format!("requirementDiagram\n{statement}\n").replace('\n', ending);
                assert!(
                    crate::SemanticParser::new()
                        .parse_diagram_sync(&source)
                        .is_err()
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_literal_nel_root_prefixes_never_publish_shifted_identity_ranges() {
        for ending in ["\n", "\r\n"] {
            for statement in [
                "element Ghost {\n}\n",
                "requirement Ghost {\n}\n",
                "style Ghost fill:red\n",
                "class Ghost hot\n",
                "classDef Ghost fill:red\n",
                "Ghost - satisfies -> Target\n",
            ] {
                let source = format!(
                    "requirementDiagram\n\u{85}{statement}element Target {{\n}}\nelement Source {{\n}}\nSource - satisfies -> Target\n"
                ).replace('\n', ending);
                let parser = crate::SemanticParser::new();
                assert!(parser.parse_diagram_sync(&source).is_err());
                let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                    parser.capture_diagram_snapshot_sync(&source)
                else {
                    panic!("expected Requirement snapshot");
                };
                let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("expected recovered Requirement facts");
                };
                assert!(!facts.symbols.iter().any(|symbol| symbol.name == "Ghost"));
                for symbol in facts
                    .symbols
                    .iter()
                    .filter(|symbol| matches!(symbol.name.as_str(), "Source" | "Target"))
                {
                    assert_eq!(
                        &source[symbol.selection.start..symbol.selection.end],
                        symbol.name
                    );
                }
                let nel = source.find('\u{85}').unwrap();
                assert!(facts.diagnostics.iter().any(|diagnostic| diagnostic.span
                    == Some(SourceSpan::new(nel, nel + '\u{85}'.len_utf8()))));
            }
        }
    }

    #[test]
    fn canonical_capture_resolves_forward_reference_kinds() {
        let source = concat!(
            "requirementDiagram\n",
            "future - verifies -> later\n",
            "style future,later fill:#fff\n",
            "class future,later important\n",
            "future:::important\n",
            "element future {\n",
            "}\n",
            "requirement later {\n",
            "}\n",
            "classDef important fill:#fff\n",
        );
        reset_requirement_syntax_construction_count();
        let parsed = parse_requirement_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(parsed.model.is_ok());
        assert_eq!(requirement_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed.editor_facts.expect("requested facts");
            for (name, kind, count) in [
                ("future", EditorSemanticKind::Object, 5),
                ("later", EditorSemanticKind::Struct, 4),
            ] {
                let symbols = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.name == name)
                    .collect::<Vec<_>>();
                assert_eq!(symbols.len(), count);
                assert!(symbols.iter().all(|symbol| symbol.kind == kind));
                assert!(
                    symbols[..count - 1]
                        .iter()
                        .all(|symbol| symbol.role == EditorSemanticRole::Reference)
                );
                assert_eq!(symbols[count - 1].role, EditorSemanticRole::Entity);
            }
        }
    }

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "requirement".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict"
            })),
            title: None,
        }
    }

    #[test]
    fn requirement_multiline_accessibility_scalars_keep_punctuation_and_js_whitespace() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "requirementDiagram{ending}",
                    "\u{feff}accTitle{ending}\u{00a0}\u{2028}:{ending}{ending}",
                    "\u{feff}\u{0085}Title;#%%literal\u{0085}\u{feff}{ending}",
                    "accDescr{ending}%% Removed{ending}\u{feff}:{ending}",
                    "\u{00a0}\u{0085}Help;#%%literal\u{0085}\u{feff}{ending}",
                    "requirement R {{{ending} id: 1{ending} text: \"Body\"{ending}}}{ending}",
                ),
                ending = ending,
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
            else {
                panic!("expected Requirement model");
            };
            assert_eq!(
                model.acc_title.as_deref(),
                Some("\u{0085}Title;#%%literal\u{0085}")
            );
            assert_eq!(
                model.acc_descr.as_deref(),
                Some("\u{0085}Help;#%%literal\u{0085}")
            );
            assert_eq!(model.requirements.len(), 1);
            assert_eq!(model.requirements[0].name, "R");
            assert_eq!(model.requirements[0].text, "Body");
        }
    }

    #[test]
    fn requirement_root_accessibility_structural_whitespace_matches_ecmascript() {
        let parser = crate::SemanticParser::new();
        for whitespace in ["\u{feff}", "\u{00a0}", "\u{2028}"] {
            let source = format!(
                "{whitespace}requirementDiagram\n{whitespace}accTitle{whitespace}:{whitespace}Value\n"
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
            else {
                panic!("expected Requirement model");
            };
            assert_eq!(model.acc_title.as_deref(), Some("Value"));
        }
        for statement in [
            "\u{0085}accTitle: Value",
            "accTitle\u{0085}: Value",
            "\u{0085}accDescr: Value",
            "accDescr\u{0085}{Help}",
        ] {
            let source = format!("requirementDiagram\n{statement}\n");
            assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
        }
    }

    #[test]
    fn requirement_empty_blocks_preserve_description_and_replay_closed_suffixes() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "requirementDiagram{ending}",
                    "accDescr: Old{ending}",
                    "accDescr{{}}{ending}",
                    "accDescr{ending}\u{feff}{{ {ending}\u{00a0}\u{2028}}}",
                    "accTitle{ending}:Value{ending}",
                    "element E {{{ending}}}{ending}",
                ),
                ending = ending,
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
            else {
                panic!("expected Requirement model");
            };
            assert_eq!(model.acc_title.as_deref(), Some("Value"));
            assert_eq!(model.acc_descr.as_deref(), Some("Old"));
            assert_eq!(model.elements.len(), 1);
            assert_eq!(model.elements[0].name, "E");
        }
    }

    #[test]
    fn requirement_accessibility_states_own_directive_text_and_same_line_replacements() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for (directives, title, description) in [
                ("accTitle:\ndirection LR\n", Some("direction LR"), None),
                (
                    "accDescr:\naccTitle: Literal\n",
                    None,
                    Some("accTitle: Literal"),
                ),
                (
                    "accDescr\n{\naccTitle: Keep\ndirection LR\n}\n",
                    None,
                    Some("accTitle: Keep\ndirection LR"),
                ),
                (
                    "accDescr: Old\naccDescr{\u{0085}}\n",
                    None,
                    Some("\u{0085}"),
                ),
                ("accDescr{First}accDescr{Second}\n", None, Some("Second")),
                (
                    "accDescr{Help}\u{feff}accTitle\n:Value\n",
                    Some("Value"),
                    Some("Help"),
                ),
            ] {
                let source = format!(
                    "requirementDiagram{ending}{}element E {{{ending}}}{ending}",
                    directives.replace('\n', ending),
                );
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
                else {
                    panic!("expected Requirement model");
                };
                assert_eq!(model.acc_title.as_deref(), title, "{source:?}");
                assert_eq!(model.acc_descr.as_deref(), description, "{source:?}");
                assert_eq!(model.direction, "TB");
                assert_eq!(model.elements.len(), 1);
            }
        }
    }

    #[test]
    fn requirement_accessibility_prefix_grammar_stays_family_local_and_title_is_unadmitted() {
        let source = concat!(
            "accTitle\n:\nAccessible\n",
            "accDescr: Old\naccDescr { }accDescr\n{Help}\n",
            "requirementDiagram\nrequirement R {\n id: 1\n}\n",
        );
        let construction = parse_requirement_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap();
        let model = construction.model.unwrap();
        assert_eq!(model.acc_title.as_deref(), Some("Accessible"));
        assert_eq!(model.acc_descr.as_deref(), Some("Help"));
        assert_eq!(model.requirements.len(), 1);

        let parser = crate::SemanticParser::new();
        assert!(parser.parse_diagram_sync(source).is_err());
        assert!(
            parser
                .parse_diagram_sync("requirementDiagram\ntitle Unsupported\n")
                .is_err()
        );
    }

    #[test]
    fn requirement_missing_accessibility_values_reject_at_eof_and_keep_prior_facts() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for suffix in [
                "accTitle:".to_string(),
                format!("accTitle{ending}:\u{feff}{ending}"),
                "accDescr:".to_string(),
                format!("accDescr:{ending}{ending}"),
                "accDescr{".to_string(),
                format!("accDescr{ending}{{\u{feff}{ending}"),
                format!("accDescr{ending}{{Help{ending}unfinished"),
            ] {
                let source =
                    format!("requirementDiagram{ending}element E {{{ending}}}{ending}{suffix}");
                assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
                #[cfg(feature = "editor-facts")]
                {
                    let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                        parser.capture_diagram_snapshot_sync(&source)
                    else {
                        panic!("expected recovered Requirement snapshot");
                    };
                    let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                        panic!("expected Requirement editor facts");
                    };
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Recovered
                    );
                    let entity = facts
                        .symbols
                        .iter()
                        .find(|symbol| symbol.name == "E")
                        .unwrap();
                    assert_eq!(&source[entity.selection.start..entity.selection.end], "E");
                    assert!(
                        facts.diagnostics.iter().any(|diagnostic| {
                            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                                && diagnostic.span
                                    == Some(SourceSpan::new(source.len(), source.len()))
                        }),
                        "{source:?}"
                    );
                }
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn requirement_multiline_accessibility_facts_keep_exact_original_ranges() {
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "requirementDiagram{ending}",
                    "\u{feff}accDescr{ending} \u{feff}{{{ending}",
                    "Help  {ending}  Next #%%;{ending}}}",
                    "accTitle{ending}%% Removed{ending}:{ending}\u{0085}Accessible\u{0085}\u{feff}{ending}",
                    "element E {{{ending}}}{ending}",
                ),
                ending = ending,
            );
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                crate::SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected Requirement snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            let crate::SemanticParseOutcome::Parsed(
                merman_model::semantic::SemanticDiagramModel::Requirement(model),
            ) = outcome
            else {
                panic!("expected Requirement model");
            };
            assert_eq!(
                model.acc_title.as_deref(),
                Some("\u{0085}Accessible\u{0085}")
            );
            assert_eq!(model.acc_descr.as_deref(), Some("Help  \nNext #%%;"));
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Requirement editor facts");
            };
            assert!(facts.diagnostics.is_empty());
            let block = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.detail.as_deref() == Some("requirement accessibility description")
                })
                .unwrap();
            let content = format!("Help  {ending}  Next #%%;");
            let content_start = source.find("Help").unwrap();
            assert_eq!(block.name, "Help  \n  Next #%%;");
            assert_eq!(
                block.selection,
                SourceSpan::new(content_start, content_start + content.len())
            );
            assert_eq!(&source[block.selection.start..block.selection.end], content);
            assert_eq!(
                block.span,
                SourceSpan::new(
                    source.find("accDescr").unwrap(),
                    source.find('}').unwrap() + 1
                )
            );

            let title = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some("requirement accessibility title"))
                .unwrap();
            let value_start = source.find('\u{0085}').unwrap();
            assert_eq!(title.name, "\u{0085}Accessible\u{0085}");
            assert_eq!(
                title.selection,
                SourceSpan::new(value_start, value_start + title.name.len())
            );
            assert_eq!(
                &source[title.selection.start..title.selection.end],
                title.name
            );
            assert_eq!(title.span.start, source.find("accTitle").unwrap());
            assert!(source[title.span.start..title.span.end].contains("%% Removed"));
            assert!(facts.expected_syntax.iter().any(|syntax| {
                syntax.kind == EditorExpectedSyntaxKind::Payload && syntax.span == title.selection
            }));
            let entity = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == "E")
                .unwrap();
            assert_eq!(&source[entity.selection.start..entity.selection.end], "E");
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn requirement_closed_block_suffix_errors_recover_at_the_original_range() {
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "requirementDiagram{ending}",
                    "accDescr{ending}{{Help}}unexpected{ending}",
                    "element E {{{ending}}}{ending}",
                ),
                ending = ending,
            );
            let parser = crate::SemanticParser::new();
            assert!(parser.parse_diagram_sync(&source).is_err());
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected recovered Requirement snapshot");
            };
            let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("expected Requirement editor facts");
            };
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            let bad_start = source.find("unexpected").unwrap();
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.span == Some(SourceSpan::new(bad_start, bad_start + "unexpected".len()))
            }));
            for name in ["Help", "E"] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name)
                    .unwrap();
                assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
            }
        }
    }

    #[test]
    fn semantic_model_preserves_domain_records_without_editor_work() {
        let metadata = metadata();
        let source = concat!(
            "requirementDiagram\n",
            "accTitle: <script>secret</script>Public\n",
            "direction LR\n",
            "requirement login:::critical {\n",
            "  id: REQ-1\n",
            "  text: Login must work\n",
            "  risk: high\n",
            "  verifymethod: test\n",
            "}\n",
            "element api {\n",
            "  type: service\n",
            "}\n",
            "classDef critical fill:#f9f\n",
            "login - verifies -> api\n",
        );
        reset_requirement_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_requirement_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.acc_title.as_deref(), Some("Public"));
        assert_eq!(model.direction, "LR");
        assert_eq!(model.requirements[0].requirement_id, "REQ-1");
        assert_eq!(model.requirements[0].risk, "High");
        assert_eq!(model.requirements[0].verify_method, "Test");
        assert!(
            model.requirements[0]
                .classes
                .iter()
                .any(|name| name == "critical")
        );
        assert_eq!(model.elements[0].element_type, "service");
        assert_eq!(model.relationships[0].src, "login");
        assert_eq!(model.relationships[0].dst, "api");
        assert_eq!(requirement_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full_metadata = crate::SourcePresentationMetadata {
                diagram_type: metadata.diagram_type.clone(),
                config: metadata.config.clone(),
                effective_config: crate::presentation::SourcePresentationConfig::from_config(
                    metadata.semantic_config.clone(),
                ),
                title: metadata.title.clone(),
            };
            parse_requirement_model_for_render_controlled(
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
                parse_requirement(source, &full_metadata).unwrap();
                #[cfg(feature = "editor-facts")]
                assert_eq!(crate::editor::test_work::read(), Default::default());
            }
        }
    }

    #[test]
    fn semantic_errors_keep_exact_spans_and_observe_cancellation() {
        let source = "requirementDiagram\nrequirement login {\n  risk: impossible\n}\n";
        let metadata = metadata();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_requirement_semantic_model_controlled(
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
        let start = source.find("impossible").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "impossible".len())),
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_requirement_semantic_model_controlled(
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
