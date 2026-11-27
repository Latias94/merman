//! Mindmap diagnostics, editor facts, and compatibility projections.

use super::MindmapDiagramModel;
#[cfg(feature = "presentation")]
use super::MindmapDiagramRenderModel;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, family::CombinedSemanticFailure};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_graph::mindmap::MindmapObservation;
use merman_core_graph::mindmap::{
    MindmapSource, MindmapSourceIssue, parse_mindmap_syntax_controlled,
};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! { static MINDMAP_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn reset_mindmap_syntax_construction_count() {
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.set(0);
}
#[cfg(test)]
pub(crate) fn mindmap_syntax_construction_count() -> usize {
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.get()
}

struct MindmapSemanticSource {
    parsed: MindmapSource,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

pub(crate) fn parse_mindmap_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<MindmapDiagramModel>> {
    let construction = construct_mindmap_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => crate::family::SemanticConstruction {
            model: Ok(source.parsed.into_model()),
            #[cfg(feature = "editor-facts")]
            editor_facts: source.editor_facts,
        },
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            if editor_demand.needs_recovery() {
                let (error, facts) = failure.into_parts();
                return Ok(crate::family::SemanticConstruction {
                    model: Err(error),
                    editor_facts: Some(facts),
                });
            }
            crate::family::SemanticConstruction {
                model: Err(failure.into_error()),
                #[cfg(feature = "editor-facts")]
                editor_facts: None,
            }
        }
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_mindmap(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model =
        parse_mindmap_model_for_render_controlled(code, meta, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled")?;
    render_model_to_compat_json_controlled(&model, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_mindmap_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    control.checkpoint()?;
    let construction = match construct_mindmap_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )? {
        Ok(source) => {
            let model = source
                .parsed
                .into_render_model_controlled(meta.effective_config.as_config(), control)?;
            let projected = render_model_to_compat_json_controlled(&model, meta, control)?;
            Ok((projected, source.editor_facts.unwrap_or_default()))
        }
        Err(error) => Err(error),
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |parts| parts,
        CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_mindmap_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<MindmapDiagramRenderModel> {
    parse_mindmap_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_mindmap_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<MindmapDiagramRenderModel>> {
    match construct_mindmap_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )? {
        Ok(source) => source
            .parsed
            .into_render_model_controlled(meta.effective_config.as_config(), control)
            .map(Ok),
        Err(error) => Ok(Err(error.into_error())),
    }
}

#[cfg(feature = "compat-json")]
fn render_model_to_compat_json_controlled(
    model: &MindmapDiagramRenderModel,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Value>> {
    let projection = meta.compat_json_metadata();
    merman_model::families::mindmap::compat::project_controlled(model, &projection, control)
        .map(|result| result.map_err(Error::from))
}

fn mindmap_failure(
    error: Error,
    #[cfg(feature = "editor-facts")] facts: Option<EditorSemanticFacts>,
) -> CombinedSemanticFailure {
    CombinedSemanticFailure::parser_recovery(
        #[cfg(feature = "editor-facts")]
        "mindmap",
        error,
        #[cfg(feature = "editor-facts")]
        facts,
    )
}

fn construct_mindmap_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<MindmapSemanticSource, CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.set(MINDMAP_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    let syntax = parse_mindmap_syntax_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand.captures_rich(),
        control,
    )?;
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Recovery => Some(EditorSemanticFacts::new()),
        EditorFactsDemand::Capture => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            let mut facts = EditorSemanticFacts::new();
            syntax.observe_editor_facts(
                &mut |event| project_mindmap_observation(&mut facts, event),
                control,
            )?;
            Some(facts)
        }
    };
    Ok(match syntax.into_source(config, control)? {
        Ok(parsed) => Ok(MindmapSemanticSource {
            parsed,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => Err(mindmap_failure(
            mindmap_error(diagram_type, issue),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )),
    })
}

fn mindmap_error(diagram_type: &str, issue: MindmapSourceIssue) -> Error {
    match issue {
        MindmapSourceIssue::Exact { message, span } => {
            Error::diagram_parse_exact(diagram_type, message, span)
        }
        MindmapSourceIssue::InsertionPoint { message, offset } => {
            Error::diagram_parse_insertion_point(diagram_type, message, offset)
        }
    }
}

#[cfg(feature = "editor-facts")]
fn project_mindmap_observation(facts: &mut EditorSemanticFacts, event: MindmapObservation<'_>) {
    match event {
        MindmapObservation::DirectivePrefix(prefix) => facts.push_directive_prefix(prefix),
        MindmapObservation::Node {
            id,
            span,
            selection,
            label,
        } => {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::NodeIdentifier,
                selection,
            ));
            if let Some((text, label_span)) = label {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    label_span,
                ));
                facts.push_symbol(EditorSemanticSymbol::payload(
                    text,
                    Some("mindmap node label".to_string()),
                    EditorSemanticKind::String,
                    label_span,
                    label_span,
                ));
            }
            facts.push_symbol(EditorSemanticSymbol::new(
                id,
                Some("mindmap node".to_string()),
                EditorSemanticKind::Namespace,
                span,
                selection,
            ));
        }
        MindmapObservation::Class {
            text,
            span,
            selection,
        } => facts.push_symbol(EditorSemanticSymbol::payload(
            text,
            Some("mindmap class".to_string()),
            EditorSemanticKind::Property,
            span,
            selection,
        )),
        MindmapObservation::Icon {
            text,
            span,
            selection,
        } => facts.push_symbol(EditorSemanticSymbol::payload(
            text,
            Some("mindmap icon".to_string()),
            EditorSemanticKind::String,
            span,
            selection,
        )),
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    use crate::SourceSpan;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "mindmap".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn semantic_bare_percent_labels_preserve_literal_text() {
        reset_mindmap_syntax_construction_count();
        let construction = parse_mindmap_semantic_model_controlled(
            "mindmap\n  Root %% literal\n    %% comment line\n    child[Label %% literal] %% comment\n",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap();
        let model = construction.model.unwrap();
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(model.nodes[0].node_id, "Root %% literal");
        assert_eq!(model.nodes[0].descr, "Root %% literal");
        assert_eq!(model.nodes[1].descr, "Label %% literal");
        assert_eq!(model.nodes[0].children, vec![1]);
        assert_eq!(mindmap_syntax_construction_count(), 1);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn semantic_recovery_preserves_same_indent_anonymous_sibling_spans() {
        for sibling in ["(Recovered)", "((Recovered))"] {
            let input = format!("mindmap\n  Root\n    broken[\n    {sibling}\n    after[After]\n");
            let construction = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                EditorFactsDemand::Capture,
                &crate::OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = construction.model.unwrap_err() else {
                panic!("expected an incomplete node diagnostic");
            };
            let broken_start = input.find("broken[").unwrap();
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(
                    broken_start,
                    broken_start + "broken[".len()
                ))
            );
            assert!(diagnostic.message().contains("unterminated node delimiter"));

            let facts = construction.editor_facts.unwrap();
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            let recovered = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.name == "Recovered" && symbol.detail.as_deref() == Some("mindmap node")
                })
                .expect("complete anonymous sibling must survive recovery");
            let selection_start = input.find("Recovered").unwrap();
            assert_eq!(
                recovered.selection,
                SourceSpan::new(selection_start, selection_start + "Recovered".len())
            );
            let statement_start = input.find(sibling).unwrap();
            assert_eq!(
                recovered.span,
                SourceSpan::new(statement_start, statement_start + sibling.len())
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "after"));
            assert!(!facts.symbols.iter().any(|symbol| symbol.name == "broken"));
        }
    }

    #[test]
    fn semantic_standalone_generic_closers_continue_unquoted_nodes() {
        for closing in ["(", "((", "]"] {
            for suffix in ["", " %% comment"] {
                let input = format!(
                    "mindmap\n  Root\n    child[\n      Label\n    {closing}{suffix}\n    after[After]\n"
                );
                let model = parse_mindmap_semantic_model_controlled(
                    &input,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .model
                .unwrap();
                assert_eq!(model.nodes.len(), 3);
                assert_eq!(model.nodes[0].children, vec![1, 2]);
                assert_eq!(model.nodes[1].node_id, "child");
                assert_eq!(model.nodes[1].descr, "\n      Label\n    ");
                assert_eq!(model.nodes[1].node_type, super::super::NODE_TYPE_RECT);
                assert_eq!(model.nodes[2].node_id, "after");
            }
        }
    }

    #[test]
    fn semantic_multiline_quotes_preserve_same_indent_node_shaped_text() {
        for (opening, closing, markdown) in [("\"", "\"", false), ("\"`", "`\"", true)] {
            let input = format!(
                "mindmap\n  Root\n    child[{opening}Start\n    (Literal)\n    End{closing}]\n    after[After]\n"
            );
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.nodes.len(), 3);
            assert_eq!(model.nodes[0].children, vec![1, 2]);
            assert_eq!(model.nodes[1].descr, "Start\n    (Literal)\n    End");
            assert_eq!(model.nodes[1].descr_is_markdown, markdown);
            assert_eq!(model.nodes[2].node_id, "after");
        }
    }

    #[test]
    fn semantic_icon_state_preserves_raw_payload_across_physical_lines() {
        for payload in [
            "fa\n  fa-star",
            "  fa\n\n  fa-star  ",
            "\n ",
            "  ",
            "fa\nliteral[Node]\n:::literal\n  fa-star",
        ] {
            let input = format!("mindmap\n  Root\n  ::icon({payload})\n    child[Child]\n");
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            reset_mindmap_syntax_construction_count();
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.nodes.len(), 2, "{payload:?}");
            assert_eq!(model.nodes[0].icon.as_deref(), Some(payload));
            assert_eq!(model.nodes[0].children, vec![1]);
            assert_eq!(model.nodes[1].node_id, "child");
            assert_eq!(mindmap_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn semantic_empty_icon_leaves_the_previous_decoration_intact() {
        for previous in [None, Some("star")] {
            let prior = previous
                .map(|icon| format!("  ::icon({icon})\n"))
                .unwrap_or_default();
            let input = format!("mindmap\n  Root\n{prior}  ::icon()\n    child[Child]\n");
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.nodes.len(), 2);
            assert_eq!(model.nodes[0].icon.as_deref(), previous);
            assert_eq!(model.nodes[0].children, vec![1]);
        }
    }

    #[test]
    fn semantic_empty_icon_prefixes_preserve_the_following_statement() {
        for (tail, expected_icon, expected_class) in [
            (":::after", None, Some("after")),
            ("::icon(other)", Some("other"), None),
        ] {
            let input = format!("mindmap\n  Root\n  ::icon()::ICON(){tail}\n");
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.nodes[0].icon.as_deref(), expected_icon);
            assert_eq!(model.nodes[0].class.as_deref(), expected_class);
        }
    }

    #[test]
    fn semantic_icon_requires_a_statement_terminator() {
        for payload in ["star", "fa\n  fa-star"] {
            for suffix in [" trailing", ":::class", "::icon(other)"] {
                let input = format!("mindmap\n  Root\n  ::icon({payload}){suffix}\n");
                let error = parse_mindmap_semantic_model_controlled(
                    &input,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .model
                .unwrap_err();
                let Error::DiagramParse { diagnostic, .. } = error else {
                    panic!("expected trailing icon input diagnostic");
                };
                let start = input.find(suffix).unwrap() + suffix.len() - suffix.trim_start().len();
                assert_eq!(
                    diagnostic.span(),
                    Some(SourceSpan::new(start, start + suffix.trim_start().len()))
                );
            }
            for suffix in ["", " %% comment"] {
                let input = format!("mindmap\n  Root\n  ::icon({payload}){suffix}\n");
                let model = parse_mindmap_semantic_model_controlled(
                    &input,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .model
                .unwrap();
                assert_eq!(
                    model.nodes[0].icon.as_deref(),
                    (!payload.is_empty()).then_some(payload)
                );
            }
        }
    }

    #[test]
    fn semantic_repaired_syntax_preserves_in_progress_cancellation() {
        let input = format!(
            "mindmap\n  Root %% literal\n{}",
            "    child(Label]\n".repeat(256)
        );
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);
        assert!(
            parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
        assert!(control.checkpoint().is_err());
    }

    #[test]
    fn semantic_tree_constructs_once_without_render_fields_or_editor_work() {
        reset_mindmap_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_mindmap_semantic_model_controlled(
            "mindmap\n  root[Root]\n    child((Child))\n      leaf[\"`Markdown`\"]\n      ::icon(star)\n      :::highlight\n",
            &metadata(), #[cfg(feature = "editor-facts")] EditorFactsDemand::Omit, &crate::OperationControl::new(),
        ).unwrap().model.unwrap();
        assert_eq!(model.nodes.len(), 3);
        assert_eq!(model.nodes[0].node_id, "root");
        assert_eq!(model.nodes[0].children, vec![1]);
        assert_eq!(model.nodes[1].children, vec![2]);
        assert_eq!(model.nodes[1].node_type, super::super::NODE_TYPE_CIRCLE);
        assert_eq!(model.nodes[2].descr, "Markdown");
        assert!(model.nodes[2].descr_is_markdown);
        assert_eq!(model.nodes[2].icon.as_deref(), Some("star"));
        assert_eq!(model.nodes[2].class.as_deref(), Some("highlight"));
        assert_eq!(mindmap_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let value = serde_json::to_value(model).unwrap();
        for key in ["width", "height", "padding", "section", "shape", "look"] {
            assert!(
                value["nodes"][0].get(key).is_none(),
                "unexpected visual field {key}"
            );
        }
    }

    #[test]
    fn semantic_config_sanitizes_plain_descriptions_and_preserves_markdown_source() {
        let mut metadata = metadata();
        metadata.semantic_config = crate::MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "strict", "themeVariables": {"primaryColor": "not-a-color"},
            "mindmap": {"padding": 40, "colorBy": "depth"}
        }));
        let model = parse_mindmap_semantic_model_controlled(
            "mindmap\n  root[\"<script>bad()</script>Safe\"]\n    leaf[\"`**Markdown**`\"]",
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.nodes[0].descr, "Safe");
        assert_eq!(model.nodes[1].descr, "**Markdown**");
    }

    #[test]
    fn semantic_database_error_preserves_exact_selection_without_editor_work() {
        let input = "mindmap\n  root\n  second[Other]\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_mindmap_semantic_model_controlled(
            input,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected parser diagnostic")
        };
        let offset = input.find("second").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "second".len()))
        );
        assert!(diagnostic.message().contains("There can be only one root"));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full = SourcePresentationMetadata {
                diagram_type: "mindmap".to_string(),
                config: crate::MermaidConfig::empty_object(),
                effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                title: None,
            };
            let full_error = parse_mindmap_model_for_render_controlled(
                input,
                &full,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap_err();
            assert_eq!(error.to_string(), full_error.to_string());
        }
    }

    #[test]
    fn semantic_cancellation_stops_before_construction() {
        reset_mindmap_syntax_construction_count();
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_mindmap_semantic_model_controlled(
                "mindmap\n  root",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
        assert_eq!(mindmap_syntax_construction_count(), 0);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn typed_and_json_paths_skip_editor_projection() {
        let full = SourcePresentationMetadata {
            diagram_type: "mindmap".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_mindmap_syntax_construction_count();
        parse_mindmap_model_for_render_controlled(
            "mindmap\n  root\n    child",
            &full,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(mindmap_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        reset_mindmap_syntax_construction_count();
        parse_mindmap("mindmap\n  root\n    child", &full).unwrap();
        assert_eq!(mindmap_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
}
