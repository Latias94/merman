use crate::Error;
#[cfg(any(test, feature = "presentation"))]
use crate::MermaidConfig;
#[cfg(any(test, feature = "editor-facts"))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "presentation")]
use merman_core_structured::kanban::project_kanban_render_model;
#[cfg(feature = "editor-facts")]
use merman_core_structured::kanban::{KanbanFieldKind, KanbanObservation};
use merman_core_structured::kanban::{KanbanSourceIssue, KanbanSourceOptions, ParsedKanban};
#[cfg(test)]
use merman_core_structured::kanban::{
    kanban_syntax_construction_count, reset_kanban_syntax_construction_count,
};
use merman_model::families::kanban::KanbanDiagramModel;
#[cfg(feature = "presentation")]
use merman_model::families::kanban::KanbanDiagramRenderModel;
#[cfg(test)]
pub(crate) use merman_model::families::kanban::KanbanNodeModel;
#[cfg(all(test, feature = "presentation"))]
pub(crate) use merman_model::families::kanban::KanbanRenderNode;
#[cfg(test)]
use merman_model::families::kanban::KanbanTerminalView;
#[cfg(feature = "editor-facts")]
use merman_model::families::kanban::{
    NODE_TYPE_BANG, NODE_TYPE_CIRCLE, NODE_TYPE_CLOUD, NODE_TYPE_HEXAGON,
};
#[cfg(feature = "compat-json")]
use serde_json::Value;

struct KanbanSemanticSource<'config> {
    parsed: ParsedKanban<'config>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct KanbanParseFailure {
    error: Box<Error>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

impl KanbanParseFailure {
    #[cfg(feature = "editor-facts")]
    fn into_error_and_editor_facts(self) -> (Error, EditorSemanticFacts) {
        let mut facts = self.editor_facts.unwrap_or_default();
        facts.mark_recovered_from_parse_error(
            format!("kanban parser recovered after parse error: {}", self.error),
            Some(self.span),
        );
        (*self.error, facts)
    }
}

fn construct_kanban_semantic_source_controlled<'config>(
    code: &str,
    diagram_type: &str,
    options: KanbanSourceOptions<'config>,
    #[cfg(feature = "editor-facts")] demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<KanbanSemanticSource<'config>, KanbanParseFailure>,
> {
    #[cfg(not(feature = "editor-facts"))]
    let parsed = options.parse_controlled(code, control)?;
    #[cfg(feature = "editor-facts")]
    let (parsed, facts) = if demand.captures_rich() {
        let mut facts = EditorSemanticFacts::new();
        let parsed = options.parse_with_observer_controlled(
            code,
            &mut |observation| push_kanban_observation(&mut facts, observation),
            control,
        )?;
        (parsed, Some(facts))
    } else {
        (options.parse_controlled(code, control)?, None)
    };
    Ok(match parsed {
        Ok(parsed) => Ok(KanbanSemanticSource {
            parsed,
            #[cfg(feature = "editor-facts")]
            editor_facts: demand.finish_facts(facts),
        }),
        Err(issue) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = match &issue {
                KanbanSourceIssue::HeaderExact { .. }
                | KanbanSourceIssue::HeaderInsertionPoint { .. } => facts,
                _ => demand.finish_facts(facts),
            };
            #[cfg(feature = "editor-facts")]
            let span = issue.span();
            Err(KanbanParseFailure {
                error: Box::new(kanban_source_error(diagram_type, issue)),
                #[cfg(feature = "editor-facts")]
                editor_facts,
                #[cfg(feature = "editor-facts")]
                span,
            })
        }
    })
}

fn kanban_source_error(diagram_type: &str, issue: KanbanSourceIssue) -> Error {
    match issue {
        KanbanSourceIssue::HeaderExact { span } => {
            Error::diagram_parse_exact(diagram_type, "expected kanban header", span)
        }
        KanbanSourceIssue::HeaderInsertionPoint { offset } => {
            Error::diagram_parse_insertion_point(diagram_type, "expected kanban header", offset)
        }
        KanbanSourceIssue::Exact { message, span } => {
            Error::diagram_parse_exact("kanban", message, span)
        }
        KanbanSourceIssue::InsertionPoint { message, offset } => {
            Error::diagram_parse_insertion_point("kanban", message, offset)
        }
    }
}

#[cfg(feature = "editor-facts")]
fn push_kanban_observation(facts: &mut EditorSemanticFacts, observation: KanbanObservation<'_>) {
    use EditorSemanticKind as Kind;
    use KanbanFieldKind as Field;

    let (kind, text, span) = match observation {
        KanbanObservation::CollectionStarted => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            return;
        }
        KanbanObservation::Directive(directive) => {
            facts.push_directive_prefix(directive);
            return;
        }
        KanbanObservation::Field { kind, text, span } => (kind, text, span),
    };
    let (detail, kind) = match kind {
        Field::Section => {
            facts.push_symbol(EditorSemanticSymbol::outline(
                text,
                Some("kanban section".into()),
                Kind::Namespace,
                span,
                span,
            ));
            return;
        }
        Field::Item { node_type } => {
            facts.push_symbol(EditorSemanticSymbol::new(
                text,
                Some("kanban item".into()),
                kanban_node_editor_kind(node_type),
                span,
                span,
            ));
            return;
        }
        Field::Label => ("kanban label".into(), Kind::String),
        Field::Icon => ("kanban icon".into(), Kind::String),
        Field::Class => ("kanban class".into(), Kind::Class),
        Field::MetadataKey => ("kanban metadata key".into(), Kind::Property),
        Field::MetadataValue { key } => (format!("kanban metadata {key}"), Kind::String),
    };
    facts.push_symbol(EditorSemanticSymbol::payload(
        text,
        Some(detail),
        kind,
        span,
        span,
    ));
}

#[cfg(feature = "editor-facts")]
fn kanban_node_editor_kind(ty: i32) -> EditorSemanticKind {
    if matches!(
        ty,
        NODE_TYPE_CIRCLE | NODE_TYPE_CLOUD | NODE_TYPE_BANG | NODE_TYPE_HEXAGON
    ) {
        EditorSemanticKind::Object
    } else {
        EditorSemanticKind::Variable
    }
}

#[cfg(feature = "presentation")]
fn into_render_model(
    model: KanbanDiagramModel,
    config: &MermaidConfig,
) -> Result<KanbanDiagramRenderModel> {
    project_kanban_render_model(model, config).map_err(|issue| kanban_source_error("kanban", issue))
}

pub(crate) fn parse_kanban_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<KanbanDiagramModel>> {
    let construction = construct_kanban_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        KanbanSourceOptions::new(&metadata.semantic_config),
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => crate::family::SemanticConstruction {
            model: Ok(source.parsed.into_semantic_model()),
            #[cfg(feature = "editor-facts")]
            editor_facts: source.editor_facts,
        },
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            if editor_demand.needs_recovery() {
                let (error, facts) = failure.into_error_and_editor_facts();
                return Ok(crate::family::SemanticConstruction {
                    model: Err(error),
                    editor_facts: Some(facts),
                });
            }
            crate::family::SemanticConstruction {
                model: Err(*failure.error),
                #[cfg(feature = "editor-facts")]
                editor_facts: None,
            }
        }
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_kanban(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model =
        parse_kanban_model_for_render_controlled(code, meta, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled")?;
    render_model_to_compat_json(&model, meta)
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_kanban_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    control.checkpoint()?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construct_kanban_semantic_source_controlled(
            code,
            &meta.diagram_type,
            KanbanSourceOptions::new(meta.effective_config.as_config()).with_presentation(),
            EditorFactsDemand::Capture,
            control,
        )?,
        |source| {
            let model = into_render_model(
                source.parsed.into_semantic_model(),
                meta.effective_config.as_config(),
            )
            .and_then(|model| render_model_to_compat_json(&model, meta));
            (model, source.editor_facts.unwrap_or_default())
        },
        KanbanParseFailure::into_error_and_editor_facts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &KanbanDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::kanban::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_kanban_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<KanbanDiagramRenderModel> {
    parse_kanban_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_kanban_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<KanbanDiagramRenderModel>> {
    let construction = construct_kanban_semantic_source_controlled(
        code,
        &meta.diagram_type,
        KanbanSourceOptions::new(meta.effective_config.as_config()).with_presentation(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    control.checkpoint()?;
    Ok(construction
        .map_err(|failure| *failure.error)
        .and_then(|source| {
            into_render_model(
                source.parsed.into_semantic_model(),
                meta.effective_config.as_config(),
            )
        }))
}

#[cfg(all(test, feature = "presentation"))]
mod presentation_data_tests {
    use super::*;

    #[test]
    fn full_construction_keeps_resolved_fields_without_json_selection() {
        let meta = SourcePresentationMetadata {
            diagram_type: "kanban".into(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"mindmap": {"maxNodeWidth": 333, "padding": 12}}),
            ),
            title: None,
        };
        let model = parse_kanban_model_for_render_controlled(
            "kanban\n  backlog[Backlog]\n    work[Work]\n",
            &meta,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.nodes.len(), 2);
        for node in &model.nodes {
            assert_eq!(node.compatibility().width, 333);
            assert_eq!(node.compatibility().padding, 24);
        }
        assert!(model.nodes[1].compatibility().level > model.nodes[0].compatibility().level);
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::EditorSemanticCompleteness;
    use crate::{
        MermaidConfig, MermaidEngine, ParseDiagnosticSpanKind, ParseOptions,
        SourcePresentationMetadata,
    };
    use futures::executor::block_on;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> crate::ParseDiagnostic {
        let engine = MermaidEngine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic,
            other => panic!("expected kanban parse error, got {other:?}"),
        }
    }

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "kanban".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn combined_parse_constructs_once_and_preserves_all_projections() {
        let text = concat!(
            "kanban\r\n",
            "  backlog\r\n",
            "    task1@{ ticket: MC-1 }\r\n",
            "    ::icon(star)\r\n",
            "    :::highlight\r\n",
        );
        let mut meta = meta();
        meta.effective_config =
            crate::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "look": "handDrawn",
                "theme": "forest"
            }));
        let expected_json = parse_kanban(text, &meta).unwrap();
        let expected_model = parse_kanban_model_for_render(text, &meta).unwrap();

        reset_kanban_syntax_construction_count();
        let (json, facts) = crate::family::test_support::into_result(
            parse_kanban_json_and_editor_facts(text, &meta, &crate::OperationControl::new()),
        )
        .unwrap();

        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(json, expected_json);
        assert!(!facts.symbols.is_empty());
        assert_eq!(
            render_model_to_compat_json(&expected_model, &meta).unwrap(),
            json
        );
        assert_eq!(json["type"], "kanban");
        assert_eq!(json["config"], meta.effective_config.as_value().clone());
        assert_eq!(json["nodes"][0]["look"], "handDrawn");
        assert!(json["nodes"][0]["ticket"].is_null());

        for name in ["backlog", "task1", "star", "highlight"] {
            let start = text.find(name).unwrap();
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.selection == SourceSpan::new(start, start + name.len())
                }),
                "missing exact Kanban fact for {name:?}"
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn malformed_editor_input_recovers_from_one_construction() {
        let text = "kanban\n  backlog[Backlog]\n    broken[Open\n";
        reset_kanban_syntax_construction_count();
        let facts = crate::family::test_support::editor_facts(
            parse_kanban_json_and_editor_facts,
            text,
            &meta(),
        );

        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "backlog"));
        assert_eq!(facts.diagnostics.len(), 1);
    }

    #[test]
    fn kanban_recovery_keeps_first_error_prefix_and_later_safe_statements() {
        let text = concat!(
            "kanban\r\n",
            "  todo[\"之前\"]\r\n",
            "    broken[\"Open 🤓\r\n",
            "    invalid[Valid] trailing\r\n",
            "    later[\"后来\"]@{ ticket: 42, active: false }\r\n",
        );
        let strict_diagnostic = parse_err(text);
        let broken_end = text.find("\r\n    invalid").unwrap();
        assert_eq!(
            strict_diagnostic.span(),
            Some(SourceSpan::new(broken_end, broken_end))
        );

        #[cfg(feature = "editor-facts")]
        {
            reset_kanban_syntax_construction_count();
            let facts = crate::family::test_support::editor_facts(
                parse_kanban_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(kanban_syntax_construction_count(), 1);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(facts.diagnostics[0].span, strict_diagnostic.span());
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "todo"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "later"));
            assert!(!facts.symbols.iter().any(|symbol| symbol.name == "invalid"));
        }
    }

    fn sections(model: &Value) -> Vec<Value> {
        model["sections"].as_array().cloned().unwrap_or_default()
    }

    fn data_nodes(model: &Value) -> Vec<Value> {
        model["nodes"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn parse_kanban_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = "kanban\n    root\n      child1\n    :::highlight\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("kanban", text)
            .unwrap()
            .unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "root"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "child1"));
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "highlight")
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn kanban_recovered_editor_fact_diagnostics_are_english() {
        let engine = MermaidEngine::new();
        let text = "kanban\n  broken[Open\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("kanban", text)
            .unwrap()
            .unwrap();

        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .starts_with("kanban parser recovered after parse error:")
        }));
        assert!(facts.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .chars()
                .any(|ch| ('\u{4E00}'..='\u{9FFF}').contains(&ch))
        }));
    }

    #[test]
    fn kanban_unterminated_node_delimiter_reports_insertion_point() {
        let text = "kanban\n  root[Open\n";
        let diagnostic = parse_err(text);
        let offset = text.trim_end().len();

        assert_eq!(diagnostic.message(), "unterminated node delimiter");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(offset, offset)));
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[test]
    fn kanban_trailing_node_input_reports_exact_span() {
        let text = "kanban\n  root[Root] extra\n";
        let diagnostic = parse_err(text);
        let offset = text.find("extra").unwrap();

        assert_eq!(diagnostic.message(), "unexpected trailing input");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "extra".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn kanban_percent_labels_still_reject_missing_delimiters_and_trailing_input() {
        let text = "kanban\n  todo\n    card[Progress %% complete\n    next[Next]\n";
        let diagnostic = parse_err(text);
        let end = text.find("\n    next").unwrap();
        assert_eq!(diagnostic.message(), "unterminated node delimiter");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(end, end)));

        let text = "kanban\n  todo\n    card[Progress %% complete] extra %% hidden\n";
        let diagnostic = parse_err(text);
        let start = text.find("extra").unwrap();
        assert_eq!(diagnostic.message(), "unexpected trailing input");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start + 5)));
    }

    #[test]
    fn kanban_multiline_label_trailing_error_keeps_its_range_and_later_recovery() {
        let text = "kanban\n  todo\n    card[\"First\nSecond\"] extra\n    next[Next]\n";
        let diagnostic = parse_err(text);
        let start = text.find("extra").unwrap();
        assert_eq!(diagnostic.message(), "unexpected trailing input");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start + 5)));
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_kanban_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(facts.diagnostics[0].span, diagnostic.span());
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "next"));
            assert!(!facts.symbols.iter().any(|symbol| symbol.name == "card"));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn kanban_multiline_and_percent_label_facts_keep_original_byte_ranges() {
        let text = concat!(
            "kanban\n",
            "  todo[Todo]\n",
            "    card[\"First 🤓  \n",
            "Second %% @{literal}\"]@{\n",
            "      ticket: '0'\n",
            "      assigned: \"Line 1\n",
            "         Line 2\"\n",
            "    } %% hidden\n",
            "    literal[Progress %% @{literal complete] %% ignored\n",
            "    next[Next]\n",
        );
        let (model, facts) = crate::family::test_support::into_result(
            parse_kanban_json_and_editor_facts(text, &meta(), &crate::OperationControl::new()),
        )
        .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        assert_eq!(
            model["nodes"][1]["label"],
            "First 🤓  \nSecond %% @{literal}"
        );
        assert_eq!(model["nodes"][1]["assigned"], "Line 1<br/>Line 2");
        for (name, detail) in [
            ("card", "kanban item"),
            ("First 🤓  \nSecond %% @{literal}", "kanban label"),
            ("ticket", "kanban metadata key"),
            ("0", "kanban metadata ticket"),
            ("Line 1\n         Line 2", "kanban metadata assigned"),
            ("Progress %% @{literal complete", "kanban label"),
            ("next", "kanban item"),
        ] {
            let start = text.find(name).unwrap();
            let span = SourceSpan::new(start, start + name.len());
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.detail.as_deref() == Some(detail)
                        && symbol.span == span
                        && symbol.selection == span
                }),
                "missing exact {detail} fact for {name:?}"
            );
        }
        assert!(!facts.symbols.iter().any(|symbol| symbol.name == "hidden"));
    }

    #[test]
    fn kanban_unterminated_metadata_reports_eof_insertion_point() {
        let text = "kanban\n  root@{ icon: star\n";
        let diagnostic = parse_err(text);

        assert_eq!(diagnostic.message(), "unterminated @{ ... } metadata block");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(text.len(), text.len()))
        );
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[test]
    fn kanban_invalid_shape_metadata_reports_exact_metadata_span() {
        let text = "kanban\n  root@{ shape: bad_shape }\n";
        let diagnostic = parse_err(text);
        let offset = text.find("@{").unwrap();

        assert!(diagnostic.message().contains("No such shape"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(
                offset,
                offset + "@{ shape: bad_shape }".len()
            ))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn knbn_1_simple_root() {
        let model = parse("kanban\n    root");
        let sections = sections(&model);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
    }

    #[test]
    fn knbn_2_hierarchy_two_children() {
        let model = parse("kanban\n    root\n      child1\n      child2\n");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["label"].as_str().unwrap(), "child1");
        assert_eq!(children[1]["label"].as_str().unwrap(), "child2");
    }

    #[test]
    fn knbn_3_shape_without_id() {
        let model = parse("kanban\n    (root)");
        let sections = sections(&model);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
    }

    #[test]
    fn knbn_4_does_not_distinguish_deeper_levels() {
        let model = parse("kanban\n    root\n      child1\n        leaf1\n      child2");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections.len(), 1);
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn knbn_5_multiple_sections() {
        let model = parse("kanban\n    section1\n    section2");
        let sections = sections(&model);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "section1");
        assert_eq!(sections[1]["label"].as_str().unwrap(), "section2");
    }

    #[test]
    fn knbn_6_real_root_in_wrong_place_is_error() {
        let engine = MermaidEngine::new();
        let text = "kanban\n          root\n        fakeRoot\n    realRootWrongPlace";
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        assert!(
            err.to_string()
                .contains("Items without section detected, found section (\"fakeRoot\")")
        );
    }

    #[test]
    fn knbn_7_id_and_label_rect() {
        let model = parse("kanban\n    root[The root]\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "The root");
    }

    #[test]
    fn knbn_8_child_id_and_label() {
        let model = parse("kanban\n    root\n      theId(child1)");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["label"].as_str().unwrap(), "child1");
        assert_eq!(children[0]["id"].as_str().unwrap(), "theId");
    }

    #[test]
    fn knbn_9_child_id_and_label_without_indent_on_root() {
        let model = parse("kanban\nroot\n      theId(child1)");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["label"].as_str().unwrap(), "child1");
        assert_eq!(children[0]["id"].as_str().unwrap(), "theId");
    }

    #[test]
    fn knbn_13_set_icon_for_node() {
        let model = parse("kanban\n    root[The root]\n    ::icon(bomb)\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "The root");
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "bomb");
    }

    #[test]
    fn empty_icon_decoration_is_an_upstream_compatible_noop() {
        let text = "kanban\n    root[The root]\n    ::icon()\n";
        let model = parse(text);
        let sections = sections(&model);
        assert!(sections[0].get("icon").is_none());

        #[cfg(feature = "editor-facts")]
        {
            let (_, facts) = crate::family::test_support::into_result(
                parse_kanban_json_and_editor_facts(text, &meta(), &crate::OperationControl::new()),
            )
            .unwrap();
            assert!(facts.directive_prefixes.iter().any(|value| value == "icon"));
            assert!(facts.symbols.iter().all(|symbol| symbol.name != "icon"));
        }
    }

    #[test]
    fn knbn_14_set_classes_for_node() {
        let model = parse("kanban\n    root[The root]\n    :::m-4 p-8\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "The root");
        assert_eq!(sections[0]["cssClasses"].as_str().unwrap(), "m-4 p-8");
    }

    #[test]
    fn knbn_15_set_classes_and_icon_classes_first() {
        let model = parse("kanban\n    root[The root]\n    :::m-4 p-8\n    ::icon(bomb)\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["cssClasses"].as_str().unwrap(), "m-4 p-8");
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "bomb");
    }

    #[test]
    fn knbn_16_set_classes_and_icon_icon_first() {
        let model = parse("kanban\n    root[The root]\n    ::icon(bomb)\n    :::m-4 p-8\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["cssClasses"].as_str().unwrap(), "m-4 p-8");
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "bomb");
    }

    #[test]
    fn knbn_17_node_syntax_in_description() {
        let model = parse("kanban\n    root[\"String containing []\"]\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(
            sections[0]["label"].as_str().unwrap(),
            "String containing []"
        );
    }

    #[test]
    fn knbn_18_node_syntax_in_child_description() {
        let model = parse(
            "kanban\n    root[\"String containing []\"]\n      child1[\"String containing ()\"]\n",
        );
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(
            sections[0]["label"].as_str().unwrap(),
            "String containing []"
        );
        assert_eq!(children.len(), 1);
        assert_eq!(
            children[0]["label"].as_str().unwrap(),
            "String containing ()"
        );
    }

    #[test]
    fn knbn_19_child_after_class_assignment() {
        let model = parse(
            "kanban\n  root(Root)\n    Child(Child)\n    :::hot\n      a(a)\n      b[New Stuff]",
        );
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "Root");
        assert_eq!(children.len(), 3);
        assert_eq!(children[0]["id"].as_str().unwrap(), "Child");
        assert_eq!(children[1]["id"].as_str().unwrap(), "a");
        assert_eq!(children[2]["id"].as_str().unwrap(), "b");
    }

    #[test]
    fn knbn_20_empty_rows() {
        let model =
            parse("kanban\n  root(Root)\n    Child(Child)\n      a(a)\n\n      b[New Stuff]");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "Root");
        assert_eq!(children.len(), 3);
        assert_eq!(children[0]["id"].as_str().unwrap(), "Child");
        assert_eq!(children[1]["id"].as_str().unwrap(), "a");
        assert_eq!(children[2]["id"].as_str().unwrap(), "b");
    }

    #[test]
    fn knbn_22_inline_comment_at_end_of_line() {
        let model = parse(
            "kanban\n  root(Root)\n    Child(Child)\n      a(a) %% This is a comment\n      b[New Stuff]",
        );
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(children.len(), 3);
        assert_eq!(children[0]["id"].as_str().unwrap(), "Child");
        assert_eq!(children[1]["id"].as_str().unwrap(), "a");
        assert_eq!(children[2]["id"].as_str().unwrap(), "b");
    }

    #[test]
    fn knbn_23_rows_with_only_spaces_should_not_interfere() {
        let model = parse("kanban\nroot\n A\n \n\n B");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["id"].as_str().unwrap(), "A");
        assert_eq!(children[1]["id"].as_str().unwrap(), "B");
    }

    #[test]
    fn knbn_24_rows_above_header() {
        let model = parse("\n \nkanban\nroot\n A\n \n\n B");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["id"].as_str().unwrap(), "A");
        assert_eq!(children[1]["id"].as_str().unwrap(), "B");
    }

    #[test]
    fn knbn_30_priority_metadata() {
        let model = parse("kanban\n        root@{ priority: high }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["priority"].as_str().unwrap(), "high");
    }

    #[test]
    fn knbn_31_assigned_metadata() {
        let model = parse("kanban\n        root@{ assigned: knsv }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["assigned"].as_str().unwrap(), "knsv");
    }

    #[test]
    fn knbn_32_icon_metadata() {
        let model = parse("kanban\n        root@{ icon: star }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "star");
    }

    #[test]
    fn knbn_34_multiline_metadata() {
        let model = parse(
            "kanban\n        root@{\n          icon: star\n          assigned: knsv\n        }\n",
        );
        let sections = sections(&model);
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "star");
        assert_eq!(sections[0]["assigned"].as_str().unwrap(), "knsv");
    }

    #[test]
    fn knbn_35_inline_metadata_multiple_pairs() {
        let model = parse("kanban\n        root@{ icon: star, assigned: knsv }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "star");
        assert_eq!(sections[0]["assigned"].as_str().unwrap(), "knsv");
    }

    #[test]
    fn knbn_36_label_override_metadata() {
        let model = parse("kanban\n        root@{ icon: star, label: 'fix things' }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "fix things");
    }

    #[test]
    fn knbn_37_ticket_metadata() {
        let model = parse("kanban\n        root@{ ticket: MC-1234 }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["ticket"].as_str().unwrap(), "MC-1234");
    }

    #[test]
    fn kanban_get_data_sanitizes_labels_again() {
        let model = parse("kanban\n    root[<b>x</b>]");
        let nodes = data_nodes(&model);
        assert_eq!(nodes[0]["label"].as_str().unwrap(), "<b>x</b>");
    }

    #[test]
    fn kanban_shape_data_rewrites_newline_whitespace_in_double_quotes() {
        let model = parse("kanban\n  root@{ label: \"line1\n      line2\" }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "line1<br>line2");
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "kanban".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    fn parse_semantic(source: &str) -> KanbanDiagramModel {
        parse_kanban_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap()
    }

    #[test]
    fn semantic_ticket_truthiness_precedes_string_conversion_and_keeps_raw_metadata() {
        for (raw, authored, expected) in [
            ("0", serde_json::json!(0), None),
            ("-0.0", serde_json::json!(-0.0), None),
            ("false", serde_json::json!(false), None),
            ("null", serde_json::Value::Null, None),
            ("''", serde_json::json!(""), None),
            ("'0'", serde_json::json!("0"), Some("0")),
            ("1", serde_json::json!(1), Some("1")),
            ("1.0", serde_json::json!(1.0), Some("1")),
            ("'1.0'", serde_json::json!("1.0"), Some("1.0")),
            (
                "9007199254740993",
                serde_json::json!(9_007_199_254_740_993_u64),
                Some("9007199254740992"),
            ),
            ("1e-7", serde_json::json!(1e-7), Some("1e-7")),
            ("1e21", serde_json::json!(1e21), Some("1e+21")),
        ] {
            let source = format!("kanban\n  todo\n    card[Task]@{{ ticket: {raw} }}\n");
            let model = parse_semantic(&source);
            let card = &model.nodes[1];
            assert_eq!(card.ticket.as_deref(), expected, "ticket: {raw}");
            assert_eq!(card.metadata.as_ref().unwrap()["ticket"], authored);
            #[cfg(feature = "presentation")]
            {
                let rendered = into_render_model(model, &MermaidConfig::empty_object()).unwrap();
                assert_eq!(rendered.nodes[1].ticket.as_deref(), expected);
                assert_eq!(
                    rendered.nodes[1].compatibility().ticket.as_deref(),
                    expected
                );
            }
        }
    }

    #[test]
    fn semantic_numeric_icon_and_assignee_share_number_text_without_changing_metadata() {
        let model = parse_semantic(concat!(
            "kanban\n  todo\n",
            "    card[Task]@{ icon: 1.0, assigned: 9007199254740993 }\n",
        ));
        let card = &model.nodes[1];
        assert_eq!(card.icon.as_deref(), Some("1"));
        assert_eq!(card.assigned.as_deref(), Some("9007199254740992"));
        let authored = card.metadata.as_ref().unwrap();
        assert_eq!(authored["icon"], serde_json::json!(1.0));
        assert_eq!(
            authored["assigned"],
            serde_json::json!(9_007_199_254_740_993_u64)
        );
    }

    #[test]
    fn semantic_collection_metadata_preserves_nested_authored_order() {
        let model = parse_semantic(concat!(
            "kanban\n",
            "  todo[Todo]\n",
            "    card[Task]@{\n",
            "      details: &details\n",
            "        z:\n",
            "          - second: 9007199254740993\n",
            "            first: true\n",
            "        a: []\n",
            "      assigned: *details\n",
            "      ticket:\n",
            "        - *details\n",
            "        - 0\n",
            "      icon: *details\n",
            "      priority: *details\n",
            "    }\n",
        ));
        let card = &model.nodes[1];
        let details_text = r#"{"z":[{"second":9007199254740993,"first":true}],"a":[]}"#;
        assert_eq!(card.assigned.as_deref(), Some(details_text));
        assert_eq!(card.icon.as_deref(), Some(details_text));
        assert_eq!(card.priority.as_deref(), Some(details_text));
        assert_eq!(card.ticket, Some(format!("[{details_text},0]")));
        let metadata = card.metadata.as_ref().unwrap();
        assert_eq!(
            metadata["details"],
            serde_json::json!({"z": [{"second": 9007199254740993_u64, "first": true}], "a": []})
        );
        assert_eq!(metadata["assigned"], metadata["details"]);
        assert_eq!(metadata["ticket"][0], metadata["details"]);
    }

    #[test]
    fn semantic_header_and_body_use_the_same_reserved_keyword_boundary() {
        for (source, id) in [
            ("kanban[x]\n", "x"),
            ("KANBAN-X\n", "-X"),
            ("kanbané\n", "é"),
        ] {
            let model = parse_semantic(source);
            assert_eq!(model.nodes.len(), 1);
            assert_eq!(model.nodes[0].id, id);
            assert!(model.nodes[0].is_group);
        }
        let model = parse_semantic("kanban\n  kanbanX[kanban-X]\n  kanban_id\n");
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(model.nodes[0].id, "kanbanX");
        assert_eq!(model.nodes[0].label, "kanban-X");
        assert_eq!(model.nodes[1].id, "kanban_id");
        for source in ["kanbanX\n", "kanban_id\n"] {
            assert!(
                parse_kanban_semantic_model_controlled(
                    source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .model
                .is_err(),
                "{source}"
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn semantic_reserved_keywords_recover_at_the_token_without_publishing_a_node() {
        for newline in ["\n", "\r\n"] {
            for reserved in ["kanban", "kanban-X", "KANBAN[x]", "kanbané"] {
                let source =
                    format!("kanban{newline}  {reserved}{newline}  safe[Recovered 中文]{newline}");
                let parsed = parse_kanban_semantic_model_controlled(
                    &source,
                    &metadata(),
                    EditorFactsDemand::Capture,
                    &crate::OperationControl::new(),
                )
                .unwrap();
                let Error::DiagramParse { diagnostic, .. } = parsed.model.unwrap_err() else {
                    panic!("expected a reserved keyword diagnostic");
                };
                let start = "kanban".len() + newline.len() + 2;
                let span = SourceSpan::new(start, start + "kanban".len());
                assert_eq!(diagnostic.span(), Some(span));
                let facts = parsed.editor_facts.unwrap();
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Recovered
                );
                assert_eq!(facts.diagnostics.len(), 1);
                assert_eq!(facts.diagnostics[0].span, Some(span));
                assert!(!facts.symbols.iter().any(|symbol| symbol.name == reserved));
                let safe_start = source.find("safe").unwrap();
                assert!(facts.symbols.iter().any(|symbol| symbol.name == "safe"
                    && symbol.selection == SourceSpan::new(safe_start, safe_start + 4)));
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn semantic_header_suffix_keeps_original_editor_ranges() {
        use crate::{ParsedEditorFacts, SemanticParser, SemanticSnapshotCapture};

        for (header, id) in [("kanban[x]", "x"), ("kanban-X", "-X"), ("kanbané", "é")] {
            for newline in ["\n", "\r\n"] {
                let source = format!(
                    "\u{feff}---{newline}title: 中文😀{newline}---{newline}{header}{newline}"
                );
                let SemanticSnapshotCapture::Snapshot(snapshot) =
                    SemanticParser::new().capture_diagram_snapshot_sync(&source)
                else {
                    panic!("Kanban retains a snapshot");
                };
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("Kanban retains editor facts");
                };
                assert!(facts.diagnostics.is_empty());
                let start = source.rfind(id).unwrap();
                assert!(facts.symbols.iter().any(|symbol| symbol.name == id
                    && symbol.selection == SourceSpan::new(start, start + id.len())));
            }
        }
    }

    #[test]
    fn semantic_quoted_label_lines_keep_indentation_nesting_and_multiline_metadata() {
        let source = concat!(
            "kanban\n",
            "  todo[Todo]\n",
            "    card[\"First  \n",
            "Second\"]@{\n",
            "      ticket: 'KBN-2'\n",
            "      assigned: \"Alice\n",
            "        Smith\"\n",
            "    } %% hidden\n",
            "      child[Child]\n",
            "    sibling[Sibling]\n",
            "  done[Done]\n",
            "    final[Final]\n",
        );
        let model = parse_semantic(source);
        let card = &model.nodes[1];
        assert_eq!(card.label, "First  \nSecond");
        assert_eq!(card.ticket.as_deref(), Some("KBN-2"));
        assert_eq!(card.assigned.as_deref(), Some("Alice<br/>Smith"));
        let start = source.find("card").unwrap();
        let end = source.find("\"]").unwrap() + 2;
        assert_eq!(card.span, SourceSpan::new(start, end));
        let membership: Vec<_> = model
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.level, node.parent_id.as_deref()))
            .collect();
        assert_eq!(
            membership,
            vec![
                ("todo", 2, None),
                ("card", 4, Some("todo")),
                ("child", 6, Some("todo")),
                ("sibling", 4, Some("todo")),
                ("done", 2, None),
                ("final", 4, Some("done")),
            ]
        );
    }

    #[test]
    fn semantic_quoted_labels_continue_for_markdown_and_other_shapes() {
        for (open, close, id) in [
            ("card[\"", "\"]", "card"),
            ("card[\"`", "`\"]", "card"),
            ("card((\"", "\"))", "card"),
            ("[\"", "\"]", "First\nSecond"),
        ] {
            let source =
                format!("kanban\n  todo\n    {open}First\nSecond{close}\n    next[Next]\n");
            let model = parse_semantic(&source);
            assert_eq!(model.nodes.len(), 3, "{source}");
            assert_eq!(model.nodes[1].id, id);
            assert_eq!(model.nodes[1].label, "First\nSecond");
            assert_eq!(model.nodes[2].id, "next");
            assert_eq!(model.nodes[2].parent_id.as_deref(), Some("todo"));
        }

        let model = parse_semantic("kanban  todo[\"First  \nSecond\"]\n    card[Next]\n");
        assert_eq!(model.nodes[0].label, "First  \nSecond");
        assert_eq!(model.nodes[1].parent_id.as_deref(), Some("todo"));
    }

    #[test]
    fn semantic_multiline_label_spans_and_text_budgets_use_the_original_source() {
        use merman_model::resources::{InputResourceLimitId, InputResourcePolicy, ResourceProfile};

        for newline in ["\n", "\r\n"] {
            let label = format!("First 🤓{newline}Second");
            let source = format!("kanban{newline}  todo{newline}    card[\"{label}\"]{newline}");
            let model = parse_semantic(&source);
            assert_eq!(model.nodes[1].label, label);
            let start = source.find("card").unwrap();
            let end = source.rfind(']').unwrap() + 1;
            assert_eq!(model.nodes[1].span, SourceSpan::new(start, end));
            let complexity = merman_model::resources::model_complexity_from_kanban_semantic(&model);
            let policy =
                InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
            policy
                .with_limit(
                    InputResourceLimitId::MaxModelTextBytes,
                    complexity.text_bytes,
                )
                .unwrap()
                .check_model_complexity(complexity)
                .unwrap();
            let error = policy
                .with_limit(
                    InputResourceLimitId::MaxModelTextBytes,
                    complexity.text_bytes - 1,
                )
                .unwrap()
                .check_model_complexity(complexity)
                .unwrap_err();
            assert_eq!(error.actual, complexity.text_bytes);
            assert_eq!(error.limit, "max_model_text_bytes");
        }
    }

    #[test]
    fn semantic_shape_labels_keep_literal_percent_and_metadata_openers() {
        for (open, close) in [
            ("[", "]"),
            ("(", ")"),
            ("((", "))"),
            ("{{", "}}"),
            (")", "("),
        ] {
            let source = format!(
                "kanban\n  todo\n    card{open}Progress %% @{{literal complete{close} %% hidden\n    next[Next]\n"
            );
            let model = parse_semantic(&source);
            assert_eq!(model.nodes.len(), 3, "{source}");
            assert_eq!(model.nodes[1].label, "Progress %% @{literal complete");
            assert!(model.nodes[1].metadata.is_none());
            assert_eq!(model.nodes[2].id, "next");
        }
        let model = parse_semantic("kanban\n  todo\n    card[\"Progress %% complete\"]\n");
        assert_eq!(model.nodes[1].label, "Progress %% complete");
        let model =
            parse_semantic("kanban%% header comment\n  todo %% section comment\n    card\n");
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(model.nodes[0].id, "todo");
        assert_eq!(model.nodes[1].parent_id.as_deref(), Some("todo"));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn semantic_unclosed_markdown_keeps_later_quoted_node_and_metadata_facts() {
        let source = concat!(
            "kanban\r\n",
            "  todo[Todo]\r\n",
            "    first[\"`Unclosed\r\n",
            "    second[\"`Still open\r\n",
            "    safe[\"Recovered 中文\"]@{ ticket: '0' }\r\n",
        );
        let parsed = parse_kanban_semantic_model_controlled(
            source,
            &metadata(),
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap();
        let Error::DiagramParse { diagnostic, .. } = parsed.model.unwrap_err() else {
            panic!("expected an unterminated node diagnostic");
        };
        let end = source.find("\r\n    second").unwrap();
        assert_eq!(diagnostic.message(), "unterminated node delimiter");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(end, end)));
        let facts = parsed.editor_facts.unwrap();
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert_eq!(facts.diagnostics.len(), 1);
        assert_eq!(facts.diagnostics[0].span, diagnostic.span());
        for (name, detail) in [
            ("safe", "kanban item"),
            ("Recovered 中文", "kanban label"),
            ("ticket", "kanban metadata key"),
            ("0", "kanban metadata ticket"),
        ] {
            let start = source.find(name).unwrap();
            let span = SourceSpan::new(start, start + name.len());
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.span == span
                    && symbol.selection == span
            }));
        }
        assert!(
            !facts
                .symbols
                .iter()
                .any(|symbol| { symbol.name == "first" || symbol.name == "second" })
        );
    }

    #[test]
    fn semantic_markdown_lookahead_keeps_valid_multiline_labels_and_facts() {
        for newline in ["\n", "\r\n"] {
            let label = format!("First 中文{newline}Second %% @{{literal}}");
            let source = format!(
                "kanban{newline}  todo{newline}    card[\"`{label}`\"]@{{ ticket: '0' }}{newline}    next[Next]{newline}"
            );
            let parsed = parse_kanban_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &crate::OperationControl::new(),
            )
            .unwrap();
            let model = parsed.model.unwrap();
            assert_eq!(model.nodes.len(), 3);
            assert_eq!(model.nodes[1].id, "card");
            assert_eq!(model.nodes[1].label, label);
            assert_eq!(model.nodes[1].ticket.as_deref(), Some("0"));
            assert_eq!(model.nodes[2].id, "next");
            assert_eq!(model.nodes[2].parent_id.as_deref(), Some("todo"));
            #[cfg(feature = "editor-facts")]
            {
                let facts = parsed.editor_facts.unwrap();
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Complete
                );
                assert!(facts.diagnostics.is_empty());
                for (name, detail) in [
                    ("card", "kanban item"),
                    (label.as_str(), "kanban label"),
                    ("next", "kanban item"),
                ] {
                    let start = source.find(name).unwrap();
                    let span = SourceSpan::new(start, start + name.len());
                    assert!(facts.symbols.iter().any(|symbol| {
                        symbol.name == name
                            && symbol.detail.as_deref() == Some(detail)
                            && symbol.span == span
                            && symbol.selection == span
                    }));
                }
            }
        }
    }

    #[test]
    fn semantic_cards_preserve_authored_metadata_without_editor_work() {
        reset_kanban_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_kanban_semantic_model_controlled(
            "kanban\n  todo[Todo]\n    card[Caption]@{ ticket: MC-1, priority: high, custom: [1, 2] }\n    ::icon(star)\n    :::highlight\n",
            &metadata(), #[cfg(feature = "editor-facts")] EditorFactsDemand::Omit, &crate::OperationControl::new(),
        ).unwrap().model.unwrap();
        assert_eq!(model.nodes.len(), 2);
        assert!(model.nodes[0].is_group);
        let card = &model.nodes[1];
        assert_eq!(
            (
                card.id.as_str(),
                card.label.as_str(),
                card.parent_id.as_deref()
            ),
            ("card", "Caption", Some("todo"))
        );
        assert_eq!(card.ticket.as_deref(), Some("MC-1"));
        assert_eq!(card.icon.as_deref(), Some("star"));
        assert_eq!(card.css_classes.as_deref(), Some("highlight"));
        assert_eq!(
            card.metadata.as_ref().unwrap()["custom"],
            serde_json::json!([1, 2])
        );
        let terminal = KanbanTerminalView::from_model(&model);
        assert_eq!(
            serde_json::to_value(terminal).unwrap(),
            serde_json::to_value(&model).unwrap()
        );
        assert_eq!(
            terminal.complexity(),
            merman_model::resources::model_complexity_from_kanban_semantic(&model)
        );
        assert!(std::ptr::eq(
            terminal.node(1).unwrap().authored().unwrap(),
            card
        ));
        assert_eq!(kanban_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn semantic_config_and_exact_errors_share_the_parser() {
        let mut metadata = metadata();
        metadata.semantic_config =
            MermaidConfig::from_value(serde_json::json!({"securityLevel": "strict"}));
        let model = parse_kanban_semantic_model_controlled(
            "kanban\n  a[<script>bad()</script>Safe]\n",
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.nodes[0].label, "Safe");
        let input = "kanban\n  broken[Open";
        let error = parse_kanban_semantic_model_controlled(
            input,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected exact parser diagnostic")
        };
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(input.len(), input.len()))
        );
        #[cfg(feature = "presentation")]
        {
            let full = SourcePresentationMetadata {
                diagram_type: metadata.diagram_type,
                config: metadata.config,
                effective_config: crate::presentation::SourcePresentationConfig::from_value(
                    crate::json_value::clone_value_nonrecursive(
                        metadata.semantic_config.as_value(),
                    ),
                ),
                title: None,
            };
            let full_error = parse_kanban_model_for_render_controlled(
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
        reset_kanban_syntax_construction_count();
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_kanban_semantic_model_controlled(
                "kanban\n  root",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
        assert_eq!(kanban_syntax_construction_count(), 0);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn typed_and_json_paths_skip_editor_projection() {
        let full = SourcePresentationMetadata {
            diagram_type: "kanban".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_kanban_syntax_construction_count();
        parse_kanban_model_for_render_controlled(
            "kanban\n  section\n    card",
            &full,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(kanban_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        reset_kanban_syntax_construction_count();
        parse_kanban("kanban\n  section\n    card", &full).unwrap();
        assert_eq!(kanban_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
    #[cfg(feature = "presentation")]
    #[test]
    fn canonical_cards_keep_source_identity_while_full_repeats_duplicate_section_groups() {
        let source = "kanban\n  same[First]\n    one[One]\n  same[Second]\n    two[Two]\n";
        let canonical = parse_kanban_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(canonical.nodes.len(), 4);
        let projected = into_render_model(canonical, &MermaidConfig::empty_object()).unwrap();
        let terminal = KanbanTerminalView::from_render(&projected);
        let mut fallback = serde_json::to_value(terminal).unwrap();
        for (wire, node) in fallback["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(&projected.nodes)
        {
            let fields = wire.as_object_mut().unwrap();
            assert_eq!(
                fields.remove("level").unwrap(),
                serde_json::json!(node.compatibility().level)
            );
            assert_eq!(
                fields.remove("cssClasses"),
                node.compatibility()
                    .css_classes
                    .as_ref()
                    .map(|value| serde_json::json!(value))
            );
            assert_eq!(
                fields.remove("shape"),
                node.compatibility()
                    .shape
                    .as_ref()
                    .map(|value| serde_json::json!(value))
            );
        }
        assert_eq!(fallback, serde_json::to_value(&projected).unwrap());
        assert_eq!(
            terminal.complexity(),
            merman_model::resources::model_complexity_from_kanban(&projected)
        );
        let ids: Vec<_> = projected
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect();
        assert_eq!(ids, vec!["same", "one", "two", "same", "one", "two"]);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn full_visual_validation_keeps_first_error_and_recovery_span() {
        let source = "kanban\n  section[Section]\n  broken[";
        let full = SourcePresentationMetadata {
            diagram_type: "kanban".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"mindmap": {"padding": i64::MAX}}),
            ),
            title: None,
        };
        let error = parse_kanban_model_for_render_controlled(
            source,
            &full,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected parser diagnostic")
        };
        assert_eq!(
            diagnostic.message(),
            "node padding exceeds the supported integer range"
        );
        let start = source.find("section").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "section[Section]".len()))
        );
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_kanban_json_and_editor_facts,
                source,
                &full,
            );
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert_eq!(facts.diagnostics[0].span, diagnostic.span());
        }
    }
}
