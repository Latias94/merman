use crate::family::CombinedSemanticFailure;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, family::EditorFactsDemand,
};
use crate::{Error, OperationControl, OperationControlResult, SourceSpan};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static JOURNEY_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_journey_syntax_construction_count() {
    JOURNEY_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn journey_syntax_construction_count() -> usize {
    JOURNEY_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[cfg(feature = "editor-facts")]
use merman_core_structured::journey::{JourneyObservation, JourneyTextKind};
use merman_core_structured::journey::{JourneySourceIssue, parse_journey_source_controlled};
use merman_model::families::journey::JourneyDiagramModel;
#[cfg(feature = "presentation")]
use merman_model::families::journey::JourneyOutputData;
#[cfg(feature = "editor-facts")]
use merman_model::text::is_ecmascript_whitespace;

pub(crate) fn sanitize_common_db_fields(
    model: &mut JourneyDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

struct JourneySemanticSource {
    model: JourneyDiagramModel,
    #[cfg(feature = "presentation")]
    output_data: JourneyOutputData,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "presentation")]
impl JourneySemanticSource {
    fn into_render_parts(self) -> (JourneyDiagramModel, JourneyOutputData) {
        (self.model, self.output_data)
    }

    #[cfg(feature = "compat-json")]
    fn into_compat_json(self, meta: &SourcePresentationMetadata) -> Result<Value> {
        let (model, output_data) = self.into_render_parts();
        render_model_to_compat_json(&model, output_data, meta)
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_journey(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source = construct_journey_semantic_source(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
    )
    .map_err(CombinedSemanticFailure::into_error)?;
    source.into_compat_json(meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_journey_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_journey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    Ok(crate::family::CombinedSemanticParse::from_construction(
        construction,
        |mut source| {
            let editor_facts = source.editor_facts.take().unwrap_or_default();
            let model = source.into_compat_json(meta);
            (model, editor_facts)
        },
        CombinedSemanticFailure::into_parts,
    ))
}

pub(crate) fn parse_journey_semantic_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<JourneyDiagramModel>> {
    let construction = construct_journey_semantic_source_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts;
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, config);
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

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &JourneyDiagramModel,
    output_data: JourneyOutputData,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    Ok(merman_model::families::journey::compat::project(
        model,
        output_data,
        &meta.diagram_type,
    ))
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_journey_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<(JourneyDiagramModel, JourneyOutputData)> {
    construct_journey_semantic_source(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
    )
    .map(JourneySemanticSource::into_render_parts)
    .map_err(CombinedSemanticFailure::into_error)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_journey_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<(JourneyDiagramModel, JourneyOutputData)>> {
    let construction = construct_journey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(failure.into_error())),
    };
    control.checkpoint()?;
    Ok(Ok(source.into_render_parts()))
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_journey_semantic_source(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
) -> std::result::Result<JourneySemanticSource, CombinedSemanticFailure> {
    construct_journey_semantic_source_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_journey_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<JourneySemanticSource, CombinedSemanticFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    JOURNEY_SYNTAX_CONSTRUCTION_COUNT.set(JOURNEY_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand
        .needs_recovery()
        .then(EditorSemanticFacts::new);
    #[cfg(feature = "editor-facts")]
    let mut observe = editor_facts
        .as_mut()
        .map(|facts| move |observation| project_journey_observation(facts, observation));
    let outcome = parse_journey_source_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand.captures_rich(),
        #[cfg(feature = "editor-facts")]
        observe.as_mut().map(|observe| observe as &mut dyn FnMut(_)),
        control,
    )?;
    Ok(match outcome {
        Ok(source) => Ok(JourneySemanticSource {
            model: source.model,
            #[cfg(feature = "presentation")]
            output_data: if source.header_seen {
                JourneyOutputData::Model
            } else {
                JourneyOutputData::Empty
            },
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => Err(CombinedSemanticFailure::parser_recovery(
            #[cfg(feature = "editor-facts")]
            "journey",
            journey_source_error(diagram_type, issue),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )),
    })
}

fn journey_source_error(diagram_type: &str, issue: JourneySourceIssue) -> Error {
    let exact = |message: &'static str, span: SourceSpan| {
        Error::diagram_parse_exact(diagram_type, message, span)
    };
    let insertion = |message: &'static str, offset: usize| {
        Error::diagram_parse_insertion_point(diagram_type, message, offset)
    };
    match issue {
        JourneySourceIssue::UnexpectedHeader(span) => {
            exact("unexpected content after journey header", span)
        }
        JourneySourceIssue::ExpectedHeader(span) => exact("expected journey header", span),
        JourneySourceIssue::MissingAccTitle(offset) => {
            insertion("expected journey accessibility title value", offset)
        }
        JourneySourceIssue::MissingAccDescr(offset) => {
            insertion("expected journey accessibility description value", offset)
        }
        JourneySourceIssue::UnterminatedAccDescr(offset) => {
            insertion("unterminated accDescr block", offset)
        }
        JourneySourceIssue::TrailingSectionTaskData(span) => {
            exact("unexpected journey task data after section", span)
        }
        JourneySourceIssue::MissingTaskName(span) => exact("expected journey task name", span),
        JourneySourceIssue::MissingTaskData(offset) => {
            insertion("expected journey task data after ':'", offset)
        }
    }
}

#[cfg(feature = "editor-facts")]
fn project_journey_observation(
    facts: &mut EditorSemanticFacts,
    observation: JourneyObservation<'_>,
) {
    let (kind, mut text, statement, mut selection) = match observation {
        JourneyObservation::Text {
            kind,
            text,
            statement,
            selection,
        } => (kind, text, statement, selection),
        JourneyObservation::UnterminatedAccDescr(span) => {
            facts.mark_recovered_from_parse_error(
                "journey parser recovered from unterminated accDescr block",
                Some(span),
            );
            return;
        }
    };
    let (prefix, detail) = match kind {
        JourneyTextKind::Title => (Some("title"), "journey title"),
        JourneyTextKind::AccTitle => (Some("accTitle"), "journey accessibility title"),
        JourneyTextKind::AccDescr | JourneyTextKind::AccDescrBlock => {
            (Some("accDescr"), "journey accessibility description")
        }
        JourneyTextKind::Section => (None, "journey section"),
        JourneyTextKind::Task => (None, "journey task"),
        JourneyTextKind::Score => (None, "journey score"),
        JourneyTextKind::People => (None, "journey people"),
    };
    if let Some(prefix) = prefix {
        facts.push_directive_prefix(prefix);
        if kind != JourneyTextKind::AccDescrBlock {
            let trimmed = text.trim_matches(is_ecmascript_whitespace);
            if trimmed.is_empty() {
                return;
            }
            let start = selection.start + text.len()
                - text.trim_start_matches(is_ecmascript_whitespace).len();
            selection = SourceSpan::new(start, start + trimmed.len());
            text = trimmed;
        }
    }
    if kind != JourneyTextKind::Section {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            selection,
        ));
    }
    let name = text.to_string();
    let detail = Some(detail.to_string());
    let symbol = match kind {
        JourneyTextKind::Section => EditorSemanticSymbol::outline(
            name,
            detail,
            EditorSemanticKind::Namespace,
            statement,
            selection,
        ),
        JourneyTextKind::Task => EditorSemanticSymbol::outline(
            name,
            detail,
            EditorSemanticKind::Event,
            statement,
            selection,
        ),
        _ => EditorSemanticSymbol::payload(
            name,
            detail,
            EditorSemanticKind::String,
            statement,
            selection,
        ),
    };
    facts.push_symbol(symbol);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[cfg(feature = "editor-facts")]
    use crate::{
        EditorExpectedSyntaxKind, EditorSemanticCompleteness, EditorSemanticDiagnosticKind,
        EditorSemanticKind, EditorSemanticRole,
    };
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    use crate::{MermaidEngine, ParseDiagnosticSpanKind, ParseOptions, SourceSpan};
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    use futures::executor::block_on;
    use serde_json::json;

    #[test]
    fn journey_structural_whitespace_matches_ecmascript() {
        let parser = crate::SemanticParser::new();
        for whitespace in [' ', '\t', '\u{00a0}', '\u{feff}'] {
            for ending in ["\n", "\r\n"] {
                let source = format!(
                    concat!(
                        "journey{ending}",
                        "{whitespace}title{whitespace}Checkout{ending}",
                        "{whitespace}accTitle{whitespace}:{whitespace}Accessible{whitespace}{ending}",
                        "{whitespace}accDescr{whitespace}:{whitespace}Help{whitespace}{ending}",
                        "{whitespace}section{whitespace}Payment{ending}",
                        "{whitespace}Pay: 4: Actor{ending}",
                    ),
                    whitespace = whitespace,
                    ending = ending,
                );
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model
                else {
                    panic!("expected Journey model");
                };
                assert_eq!(model.title.as_deref(), Some("Checkout"));
                assert_eq!(model.acc_title.as_deref(), Some("Accessible"));
                assert_eq!(model.acc_descr.as_deref(), Some("Help"));
                assert_eq!(model.sections, ["Payment"]);
                assert_eq!(model.tasks[0].task, "Pay");
                assert_eq!(model.tasks[0].section_index, Some(0));
                assert_eq!(model.tasks[0].score, 4.0);
                assert_eq!(model.tasks[0].people, ["Actor"]);
            }
        }
    }

    #[test]
    fn journey_keyword_separators_preserve_remaining_label_whitespace() {
        let source = concat!(
            "journey\n",
            "title\u{feff} Title \u{feff} \n",
            "section\u{feff} \u{feff}Checkout \u{feff} \n",
            "\u{feff}\t Pay \u{feff} : 4: Actor\n",
        );
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model else {
            panic!("expected Journey model");
        };

        assert_eq!(model.title.as_deref(), Some(" Title \u{feff} "));
        assert_eq!(model.sections, [" \u{feff}Checkout \u{feff} "]);
        assert_eq!(model.tasks[0].section, " \u{feff}Checkout \u{feff} ");
        assert_eq!(model.tasks[0].task, "Pay \u{feff} ");
    }

    #[test]
    fn journey_nel_is_task_text_instead_of_structural_whitespace() {
        let parser = crate::SemanticParser::new();
        for body in [
            "\u{0085}section Checkout",
            "\u{0085}title Checkout",
            "\u{0085}",
        ] {
            let source = format!("journey\n{body}\nPay: 4: Actor\n");
            assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
        }
        for (body, expected_name, expected_nan) in [
            ("\u{0085}Pay: 4: Actor", "\u{0085}Pay", false),
            ("\u{0085}: 4: Actor", "\u{0085}", false),
            ("title\u{0085}Pay: 4: Actor", "title\u{0085}Pay", false),
            ("section\u{0085}Pay: 4: Actor", "section\u{0085}Pay", false),
            ("\u{0085}accTitle: Checkout", "\u{0085}accTitle", true),
            ("accTitle\u{0085}: 4: Actor", "accTitle\u{0085}", false),
            ("accDescr\u{0085}: 4: Actor", "accDescr\u{0085}", false),
        ] {
            let source = format!("journey\n{body}\n");
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model else {
                panic!("expected Journey model");
            };
            assert!(model.title.is_none());
            assert!(model.acc_title.is_none());
            assert!(model.acc_descr.is_none());
            assert!(model.sections.is_empty());
            assert_eq!(model.tasks[0].task, expected_name);
            assert_eq!(model.tasks[0].score_is_nan, expected_nan);
        }
    }

    #[test]
    fn journey_prefixed_comment_priority_preserves_literal_label_controls() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for line in [
                "\u{0085}%% Ghost: 4: Actor",
                "\u{0085}%% Ghost",
                "x%% Ghost: 4: Actor",
            ] {
                let source = format!("journey{ending}{line}{ending}Pay: 5: Actor{ending}");
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model
                else {
                    panic!("expected Journey model");
                };
                assert_eq!(model.tasks.len(), 1, "{source}");
                assert_eq!(model.tasks[0].task, "Pay");
                #[cfg(feature = "editor-facts")]
                {
                    let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                        parser.capture_diagram_snapshot_sync(&source)
                    else {
                        panic!("expected Journey snapshot");
                    };
                    let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                        panic!("expected editor facts");
                    };
                    assert!(facts.diagnostics.is_empty());
                    assert!(
                        !facts
                            .symbols
                            .iter()
                            .any(|symbol| symbol.name.contains("Ghost"))
                    );
                }
            }
            for name in ["}%% Visible", "Long%% Visible", "\u{1f680}%% Visible"] {
                let source =
                    format!("journey{ending}{name}: 4: Actor{ending}Pay: 5: Actor{ending}");
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model
                else {
                    panic!("expected Journey model");
                };
                assert_eq!(model.tasks.len(), 2);
                assert_eq!(model.tasks[0].task, name);
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn journey_unicode_payload_facts_keep_original_keyword_and_task_offsets() {
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "journey{ending}",
                    "\u{feff}title\u{feff}title\u{feff} {ending}",
                    "\u{feff}accTitle\u{feff}:\u{feff}\u{0085}Accessible\u{0085}\u{feff}{ending}",
                    "\u{feff}accDescr\u{feff}:\u{feff}\u{0085}Help\u{0085}\u{feff}{ending}",
                    " \u{feff}section\u{feff}section\u{feff} {ending}",
                    " \u{feff}\u{0085}Pay\u{0085}\u{feff}: 4: Actor{ending}",
                ),
                ending = ending,
            );
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                crate::SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected Journey snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            let crate::SemanticParseOutcome::Parsed(
                merman_model::semantic::SemanticDiagramModel::Journey(model),
            ) = outcome
            else {
                panic!("expected canonical Journey model");
            };
            assert_eq!(
                model.acc_title.as_deref(),
                Some("\u{0085}Accessible\u{0085}")
            );
            assert_eq!(model.acc_descr.as_deref(), Some("\u{0085}Help\u{0085}"));
            assert_eq!(model.tasks[0].task, "\u{0085}Pay\u{0085}\u{feff}");
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Journey editor facts");
            };
            assert!(facts.diagnostics.is_empty());
            for (detail, payload, start) in [
                (
                    "journey title",
                    "title",
                    source.find("title\u{feff}title").unwrap() + "title\u{feff}".len(),
                ),
                (
                    "journey accessibility title",
                    "\u{0085}Accessible\u{0085}",
                    source.find("\u{0085}Accessible\u{0085}").unwrap(),
                ),
                (
                    "journey accessibility description",
                    "\u{0085}Help\u{0085}",
                    source.find("\u{0085}Help\u{0085}").unwrap(),
                ),
                (
                    "journey section",
                    "section",
                    source.find("section\u{feff}section").unwrap() + "section\u{feff}".len(),
                ),
                (
                    "journey task",
                    "\u{0085}Pay\u{0085}",
                    source.find("\u{0085}Pay\u{0085}").unwrap(),
                ),
            ] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.detail.as_deref() == Some(detail))
                    .unwrap();
                assert_eq!(symbol.name, payload);
                assert_eq!(
                    symbol.selection,
                    SourceSpan::new(start, start + payload.len())
                );
                assert_eq!(
                    &source[symbol.selection.start..symbol.selection.end],
                    payload
                );
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn journey_accessibility_block_unicode_whitespace_preserves_span_and_continuation() {
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "journey{ending}",
                    "\u{feff}accDescr\u{feff}{{\u{feff}{ending}",
                    "\u{feff}\u{0085}Description\u{0085}\u{feff}{ending}",
                    "\u{feff}}}\u{feff} Pay: 4: Actor{ending}",
                ),
                ending = ending,
            );
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                crate::SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected Journey snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            let crate::SemanticParseOutcome::Parsed(
                merman_model::semantic::SemanticDiagramModel::Journey(model),
            ) = outcome
            else {
                panic!("expected canonical Journey model");
            };
            assert_eq!(
                model.acc_descr.as_deref(),
                Some("\u{0085}Description\u{0085}")
            );
            assert_eq!(model.tasks[0].task, "Pay");
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Journey editor facts");
            };
            assert!(facts.diagnostics.is_empty());
            let description = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.detail.as_deref() == Some("journey accessibility description")
                })
                .unwrap();
            assert_eq!(description.name, "\u{0085}Description\u{0085}");
            assert_eq!(
                description.selection,
                SourceSpan::new(source.find('{').unwrap() + 1, source.find('}').unwrap()),
            );
            let task = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some("journey task"))
                .unwrap();
            let task_start = source.find("Pay").unwrap();
            assert_eq!(
                task.selection,
                SourceSpan::new(task_start, task_start + "Pay".len())
            );
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn journey_recovered_task_range_keeps_nel_and_excludes_bom_indentation() {
        let source = "journey\r\n\u{feff}Broken\u{0085}\u{feff} \r\n";
        let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
            crate::SemanticParser::new().capture_diagram_snapshot_sync(source)
        else {
            panic!("expected recovered Journey snapshot");
        };
        let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("expected Journey editor facts");
        };
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        let task = facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some("journey task"))
            .unwrap();
        let task_start = source.find("Broken").unwrap();
        let task_end = task_start + "Broken\u{0085}".len();
        assert_eq!(task.name, "Broken\u{0085}");
        assert_eq!(task.selection, SourceSpan::new(task_start, task_end));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(task_end, task_end))
        }));
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn journey_editor_people_facts_keep_authored_unicode_order() {
        let source = "journey\nPay: 4: \u{e000}, 😀\n";
        let facts = crate::MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("journey", source)
            .unwrap()
            .unwrap();
        let people = facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some("journey people"))
            .unwrap();

        assert_eq!(people.name, "\u{e000}, 😀");
        assert_eq!(
            &source[people.selection.start..people.selection.end],
            "\u{e000}, 😀"
        );
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn journey_editor_payloads_follow_consumed_score_and_people_fields() {
        for ending in ["\n", "\r\n"] {
            let source = format!(
                "journey{ending}Pay: \u{feff}0x4\u{feff}: \u{feff}Alice\u{feff}: ignored{ending}"
            );
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                crate::SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected Journey snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            let crate::SemanticParseOutcome::Parsed(
                merman_model::semantic::SemanticDiagramModel::Journey(model),
            ) = outcome
            else {
                panic!("expected canonical Journey model");
            };
            assert_eq!(model.tasks[0].score, 4.0);
            assert!(!model.tasks[0].score_is_nan);
            assert_eq!(model.tasks[0].people, ["Alice"]);
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Journey editor facts");
            };
            assert!(facts.diagnostics.is_empty());
            for (detail, expected) in [("journey score", "0x4"), ("journey people", "Alice")] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.detail.as_deref() == Some(detail))
                    .unwrap();
                assert_eq!(symbol.name, expected);
                assert_eq!(
                    &source[symbol.selection.start..symbol.selection.end],
                    expected
                );
            }
        }
    }

    #[test]
    fn journey_section_tokens_reject_trailing_task_data_and_keep_task_name_controls() {
        let parser = crate::SemanticParser::new();
        for source in [
            "journey\nsection Checkout: 5: Actor\n",
            "journey\nsection \n",
            "journey\ntitle \n",
        ] {
            assert!(parser.parse_diagram_sync(source).is_err(), "{source:?}");
            #[cfg(feature = "editor-facts")]
            {
                let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                    parser.capture_diagram_snapshot_sync(source)
                else {
                    panic!("expected recovered Journey snapshot");
                };
                let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("expected Journey editor facts");
                };
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Recovered
                );
                assert!(!facts.diagnostics.is_empty());
            }
        }
        for (source, sections, task_name, score) in [
            ("journey\nsection : 4: Actor\n", vec![], "section ", 4.0),
            ("journey\nsection  \nPay: 3: Actor\n", vec![" "], "Pay", 3.0),
            (
                "journey\nPay: \u{feff}0x4\u{feff}: \u{feff}Actor\u{feff}\n",
                vec![],
                "Pay",
                4.0,
            ),
        ] {
            let parsed = parser.parse_diagram_sync(source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model else {
                panic!("expected Journey model");
            };
            assert_eq!(model.sections, sections);
            assert_eq!(model.tasks[0].task, task_name);
            assert_eq!(model.tasks[0].score, score);
            assert!(!model.tasks[0].score_is_nan);
            assert_eq!(model.tasks[0].people, ["Actor"]);
        }
    }

    #[test]
    fn journey_multiline_keywords_consume_one_separator_and_retain_payloads() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for payload in [
                "Value",
                "  Value ",
                "\u{feff}Value\u{feff} ",
                "\u{0085}Value",
            ] {
                let source = format!(
                    "journey{ending}title{ending}{payload}:4:Actor{ending}section{ending}{payload}{ending}Pay:4:Actor{ending}"
                );
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model
                else {
                    panic!("expected Journey model");
                };

                let expected_title = format!("{payload}:4:Actor");
                assert_eq!(model.title.as_deref(), Some(expected_title.as_str()));
                assert_eq!(model.sections, [payload]);
                assert_eq!(model.tasks.len(), 1);
                assert_eq!(model.tasks[0].task, "Pay");
                assert_eq!(model.tasks[0].section, payload);
            }
        }
    }

    #[test]
    fn journey_multiline_keyword_failures_do_not_join_extra_separators_or_task_data() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for keyword in ["title", "section"] {
                for separator in [
                    format!("{ending}{ending}"),
                    format!(" {ending}"),
                    format!("\t{ending}"),
                    format!("\u{feff}{ending}"),
                ] {
                    let source = format!(
                        "journey{ending}{keyword}{separator}Value{ending}Pay:4:Actor{ending}"
                    );
                    assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
                }
            }
            for payload in ["Value:4:Actor", ":4:Actor", " :4:Actor"] {
                let source =
                    format!("journey{ending}section{ending}{payload}{ending}Pay:4:Actor{ending}");
                assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
            }
        }
    }

    #[test]
    fn journey_multiline_accessibility_prefixes_own_their_scalar_lines() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "journey{ending}",
                    "accTitle{ending}\u{feff}\t:{ending}{ending}\u{feff}\u{0085}Title;#%%literal\u{0085}\u{feff}{ending}",
                    "accDescr\u{feff}{ending}{ending}:\u{feff}{ending}Consumed:4:Ghost{ending}",
                    "Pay:4:Actor{ending}",
                ),
                ending = ending,
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model else {
                panic!("expected Journey model");
            };

            assert_eq!(
                model.acc_title.as_deref(),
                Some("\u{0085}Title;#%%literal\u{0085}")
            );
            assert_eq!(model.acc_descr.as_deref(), Some("Consumed:4:Ghost"));
            assert_eq!(model.tasks.len(), 1);
            assert_eq!(model.tasks[0].task, "Pay");
            assert_eq!(model.actors, ["Actor"]);
        }
    }

    #[test]
    fn journey_empty_multiline_accessibility_blocks_retain_the_last_value() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "journey{ending}",
                    "accDescr: Old{ending}",
                    "accDescr{ending}{{}}{ending}",
                    "accDescr{ending}{{ \t{ending}\u{feff} }}title{ending}Plan{ending}",
                    "Pay:4:Actor{ending}",
                ),
                ending = ending,
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Journey(model) = parsed.model else {
                panic!("expected Journey model");
            };

            assert_eq!(model.acc_descr.as_deref(), Some("Old"));
            assert_eq!(model.title.as_deref(), Some("Plan"));
            assert_eq!(model.tasks.len(), 1);
            assert_eq!(model.tasks[0].task, "Pay");
            #[cfg(feature = "editor-facts")]
            {
                let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                    parser.capture_diagram_snapshot_sync(&source)
                else {
                    panic!("expected Journey snapshot");
                };
                let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("expected Journey editor facts");
                };
                let descriptions = facts
                    .symbols
                    .iter()
                    .filter(|symbol| {
                        symbol.detail.as_deref() == Some("journey accessibility description")
                    })
                    .collect::<Vec<_>>();
                assert_eq!(descriptions.len(), 1);
                assert_eq!(descriptions[0].name, "Old");
                assert!(facts.diagnostics.is_empty());
            }
        }
    }

    #[test]
    fn journey_accessibility_tokens_reject_missing_eof_values_and_unclosed_blocks() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for suffix in [
                "accTitle:".to_string(),
                format!("accTitle:{ending}"),
                format!("accTitle{ending}:\u{feff}{ending}"),
                "accDescr:".to_string(),
                format!("accDescr:{ending}"),
                "accDescr {".to_string(),
                format!("accDescr{ending}{{Help"),
                format!("accDescr{ending}{{ \u{feff}{ending}"),
            ] {
                let source = format!("journey{ending}title Plan{ending}{suffix}");
                assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
                #[cfg(feature = "editor-facts")]
                {
                    let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                        parser.capture_diagram_snapshot_sync(&source)
                    else {
                        panic!("expected recovered Journey snapshot");
                    };
                    let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                        panic!("expected Journey editor facts");
                    };
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Recovered
                    );
                    let title = facts
                        .symbols
                        .iter()
                        .find(|symbol| symbol.detail.as_deref() == Some("journey title"))
                        .unwrap();
                    assert_eq!(title.name, "Plan");
                    assert_eq!(&source[title.selection.start..title.selection.end], "Plan");
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
    fn journey_multiline_tokens_and_block_suffix_keep_original_editor_ranges() {
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "journey{ending}",
                    "\u{feff}accDescr{ending} \u{feff}{{{ending}Help{ending}Next{ending}}}title{ending} Plan {ending}",
                    "section{ending}\u{feff}Checkout\u{feff} {ending}",
                    "accTitle{ending}\u{feff}:{ending}\u{0085}Accessible\u{0085}{ending}",
                    "Pay:4:Actor{ending}",
                ),
                ending = ending,
            );
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                crate::SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected Journey snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            let crate::SemanticParseOutcome::Parsed(
                merman_model::semantic::SemanticDiagramModel::Journey(model),
            ) = outcome
            else {
                panic!("expected Journey model");
            };
            assert_eq!(model.acc_descr.as_deref(), Some("Help\nNext"));
            assert_eq!(model.title.as_deref(), Some(" Plan "));
            assert_eq!(model.sections, ["\u{feff}Checkout\u{feff} "]);
            assert_eq!(model.tasks.len(), 1);
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Journey editor facts");
            };
            assert!(facts.diagnostics.is_empty());
            for (detail, keyword, payload) in [
                ("journey title", "title", "Plan"),
                ("journey section", "section", "Checkout"),
                (
                    "journey accessibility title",
                    "accTitle",
                    "\u{0085}Accessible\u{0085}",
                ),
            ] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.detail.as_deref() == Some(detail))
                    .unwrap();
                let start = source.find(keyword).unwrap();
                let value_start = source.find(payload).unwrap();
                assert_eq!(symbol.name, payload);
                assert_eq!(symbol.span.start, start);
                assert_eq!(
                    &source[symbol.selection.start..symbol.selection.end],
                    payload
                );
                assert_eq!(
                    symbol.selection,
                    SourceSpan::new(value_start, value_start + payload.len())
                );
                assert!(source[symbol.span.start..symbol.span.end].contains(ending));
                assert!(symbol.span.end >= symbol.selection.end);
            }
            let block = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.detail.as_deref() == Some("journey accessibility description")
                })
                .unwrap();
            let open = source.find('{').unwrap();
            let close = source.find('}').unwrap();
            assert_eq!(block.name, "Help\nNext");
            assert_eq!(
                block.span,
                SourceSpan::new(source.find("accDescr").unwrap(), close + 1)
            );
            assert_eq!(block.selection, SourceSpan::new(open + 1, close));
            assert_eq!(
                &source[block.selection.start..block.selection.end],
                format!("{ending}Help{ending}Next{ending}")
            );
            let task = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some("journey task"))
                .unwrap();
            assert_eq!(&source[task.selection.start..task.selection.end], "Pay");
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_title_definition_parses() {
        let model = parse("journey\ntitle Adding journey diagram functionality to mermaid");
        assert_eq!(
            model["title"],
            json!("Adding journey diagram functionality to mermaid")
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_parses_acc_descr_block_and_title_and_acc_title() {
        let model = parse(
            "journey\n\
accDescr {\n\
  A user journey for\n\
  family shopping\n\
}\n\
title Adding journey diagram functionality to mermaid\n\
accTitle: Adding acc journey diagram functionality to mermaid\n\
section Order from website\n",
        );
        assert_eq!(
            model["accDescr"],
            json!("A user journey for\nfamily shopping")
        );
        assert_eq!(
            model["title"],
            json!("Adding journey diagram functionality to mermaid")
        );
        assert_eq!(
            model["accTitle"],
            json!("Adding acc journey diagram functionality to mermaid")
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_parses_acc_title_without_description() {
        let model = parse(
            "journey\n\
accTitle: The title\n\
section Order from website\n",
        );
        assert_eq!(model["accTitle"], json!("The title"));
        assert!(model["accDescr"].is_null());
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_parses_acc_descr_single_line() {
        let model = parse(
            "journey\n\
accDescr: A user journey for family shopping\n\
title Adding journey diagram functionality to mermaid\n\
section Order from website\n",
        );
        assert_eq!(
            model["accDescr"],
            json!("A user journey for family shopping")
        );
        assert_eq!(
            model["title"],
            json!("Adding journey diagram functionality to mermaid")
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_accessibility_directives_preserve_punctuation_and_exact_payload_facts() {
        let source = concat!(
            "journey\r\n",
            "# ignored whole-line comment\r\n",
            "%% ignored whole-line comment\r\n",
            "  accTitle: accTitle #1 支付; ready  \r\n",
            "  accDescr: Confirm; then pay #ready  \r\n",
            "section Checkout# ignored section comment\r\n",
            "Pay: 3.5: Customer # ignored task comment\r\n",
        );
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let merman_model::presentation::RenderSemanticModel::Journey(model) = parsed.model() else {
            panic!("expected Journey model");
        };
        assert_eq!(model.acc_title.as_deref(), Some("accTitle #1 支付; ready"));
        assert_eq!(model.acc_descr.as_deref(), Some("Confirm; then pay #ready"));
        assert_eq!(model.tasks.len(), 1);
        assert_eq!(model.tasks[0].score, 3.5);
        assert_eq!(model.tasks[0].people, ["Customer"]);
        assert_eq!(model.sections, ["Checkout"]);
        let compat = parsed.compatibility_json().unwrap();
        assert_eq!(compat["accTitle"], json!("accTitle #1 支付; ready"));
        assert_eq!(compat["accDescr"], json!("Confirm; then pay #ready"));
        assert_eq!(compat["tasks"][0]["score"], json!(3.5));

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("journey", source)
            .unwrap()
            .unwrap();
        assert!(facts.diagnostics.is_empty());
        for (detail, payload) in [
            ("journey accessibility title", "accTitle #1 支付; ready"),
            (
                "journey accessibility description",
                "Confirm; then pay #ready",
            ),
            ("journey score", "3.5"),
        ] {
            let start = source.find(payload).unwrap();
            let span = SourceSpan::new(start, start + payload.len());
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some(detail))
                .unwrap();
            assert_eq!(symbol.name, payload);
            assert_eq!(symbol.selection, span);
            assert_eq!(
                &source[symbol.selection.start..symbol.selection.end],
                payload
            );
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload && expected.span == span
            }));
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_inline_accessibility_block_preserves_punctuation_and_resumes_source() {
        let source = "journey\naccDescr { Confirm; then pay #ready }Pay: 4: Customer\n";
        let model = parse(source);
        assert_eq!(model["accDescr"], json!("Confirm; then pay #ready"));
        assert_eq!(model["tasks"][0]["task"], json!("Pay"));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = "journey\n\
title Adding journey diagram functionality to mermaid\n\
accTitle: Adding acc journey diagram functionality to mermaid\n\
accDescr: A user journey for family shopping\n\
section Order from website\n\
A task: 5: Alice, Bob\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("journey", text)
            .unwrap()
            .unwrap();

        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "accTitle")
        );
        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "accDescr")
        );
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Order from website"
                && symbol.kind == EditorSemanticKind::Namespace
                && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "A task"
                && symbol.kind == EditorSemanticKind::Event
                && symbol.role == EditorSemanticRole::Outline
        }));

        let task_start = text.find("A task").unwrap();
        let score_start = text.find("5: Alice, Bob").unwrap();
        let people_start = text.find("Alice, Bob").unwrap();

        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(task_start, task_start + "A task".len())
        }));
        assert!(!facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                && expected.span == SourceSpan::new(task_start, task_start + "A task".len())
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(score_start, score_start + 1)
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(people_start, people_start + "Alice, Bob".len())
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_entrypoints_and_combined_projection_construct_once() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "journey\n",
            "title Checkout journey\n",
            "accTitle: Checkout\n",
            "section Payment\n",
            "Confirm order: 5: Alice, Bob\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("standalone Journey JSON parse succeeds")
            .expect("standalone Journey JSON parse returns a diagram");
        reset_journey_syntax_construction_count();
        crate::editor::test_work::reset();
        parse_journey(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("Journey JSON projection succeeds");
        assert_eq!(journey_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        reset_journey_syntax_construction_count();
        let (typed, _output_data) = parse_journey_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("Journey typed projection succeeds");
        assert_eq!(journey_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        reset_journey_syntax_construction_count();
        let (combined_json, combined_editor) =
            crate::family::test_support::into_result(parse_journey_json_and_editor_facts(
                text,
                &crate::diagrams::source_metadata_for_test(&parsed.meta),
                &OperationControl::new(),
            ))
            .expect("Journey combined projection succeeds");
        assert_eq!(journey_syntax_construction_count(), 1);
        assert_eq!(combined_json, parsed.model);
        assert!(!combined_editor.symbols.is_empty());

        assert_eq!(
            merman_model::families::journey::compat::project(
                &typed,
                _output_data,
                &parsed.meta.diagram_type
            ),
            combined_json
        );
        assert_eq!(parsed.model["type"], "journey");
        assert!(parsed.model["accDescr"].is_null());
        crate::editor::test_work::reset();
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn journey_typed_projection_preserves_empty_and_header_only_output_states() {
        let meta = SourcePresentationMetadata {
            diagram_type: "journey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        for source in ["", "journey"] {
            let compat = parse_journey(source, &meta).unwrap();
            let (typed, _output_data) = parse_journey_model_for_render(source, &meta).unwrap();

            assert_eq!(
                render_model_to_compat_json(&typed, _output_data, &meta).unwrap(),
                compat,
                "projection drift for {source:?}"
            );
        }
    }

    #[test]
    fn journey_semantic_model_omits_editor_facts_and_compatibility_state() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_journey_syntax_construction_count();
        let source = concat!(
            "journey\n",
            "title Checkout\n",
            "section Repeat\n",
            "First task: 5: Alice\n",
            "section Repeat\n",
            "Second task: invalid: Bob\n",
        );
        let semantic = parse_journey_semantic_model_controlled(
            source,
            "journey",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(journey_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        assert_eq!(semantic.sections, ["Repeat", "Repeat"]);
        assert_eq!(
            semantic
                .tasks
                .iter()
                .map(|task| task.section_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1)]
        );
        assert!(semantic.tasks[1].score_is_nan);
        assert_eq!(semantic.actors, ["Alice", "Bob"]);

        #[cfg(feature = "presentation")]
        {
            let meta = crate::SourcePresentationMetadata {
                diagram_type: "journey".to_string(),
                config: crate::MermaidConfig::default(),
                effective_config: crate::presentation::SourcePresentationConfig::default(),
                title: None,
            };
            let (render, _output_data) = parse_journey_model_for_render(source, &meta).unwrap();
            assert_eq!(
                serde_json::to_value(&semantic).unwrap(),
                serde_json::to_value(render).unwrap()
            );
        }
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
    }

    #[test]
    fn journey_semantic_empty_input_has_no_compatibility_sentinel() {
        let semantic = parse_journey_semantic_model_controlled(
            "",
            "journey",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        #[cfg(feature = "compat-json")]
        let meta = SourcePresentationMetadata {
            diagram_type: "journey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };

        assert_eq!(
            serde_json::to_value(semantic).unwrap(),
            json!({
                "title": null,
                "accTitle": null,
                "accDescr": null,
                "sections": [],
                "tasks": [],
                "actors": [],
            })
        );
        #[cfg(feature = "compat-json")]
        assert_eq!(parse_journey("", &meta).unwrap(), json!({}));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_malformed_task_recovers_prior_statement_facts_once() {
        let engine = MermaidEngine::new();
        let text = "journey\nsection Checkout\nBroken task\n";
        let task_start = text.find("Broken task").expect("malformed task");
        let insertion = task_start + "Broken task".len();

        reset_journey_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("journey", text)
            .expect("Journey editor recovery succeeds")
            .expect("Journey editor facts are available");

        assert_eq!(journey_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Checkout" && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Broken task" && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(insertion, insertion))
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_task_without_data_reports_insertion_point() {
        let engine = MermaidEngine::new();
        let text = "journey\nTask:\n";
        let colon = text.find(':').expect("task colon");
        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("a bare task colon is not a Journey taskData token");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected Journey parse error");
        };

        assert_eq!(diagnostic.message(), "expected journey task data after ':'");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(colon + 1, colon + 1))
        );
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_multiline_acc_descr_fact_preserves_source_span() {
        let text = concat!(
            "journey\n",
            "accDescr {\n",
            "  First line\n",
            "  Second line\n",
            "}\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_journey_json_and_editor_facts,
            text,
            &SourcePresentationMetadata {
                diagram_type: "journey".to_string(),
                config: crate::MermaidConfig::empty_object(),
                effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                title: None,
            },
        );
        let description = facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some("journey accessibility description"))
            .expect("multiline accessibility description fact");
        let content_start = text.find('{').expect("opening brace") + 1;
        let content_end = text.rfind('}').expect("closing brace");

        assert_eq!(description.name, "First line\n  Second line");
        assert_eq!(
            description.selection,
            SourceSpan::new(content_start, content_end)
        );
        assert_eq!(description.role, EditorSemanticRole::Payload);
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_allows_section_titles_with_br_variants() {
        let model = parse(
            "journey\n\
title Adding gantt diagram functionality to mermaid\n\
section Line1<br>Line2<br/>Line3</br />Line4<br\t/>Line5\n",
        );
        let sections = model["sections"].as_array().unwrap();
        assert_eq!(sections.len(), 1);
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_parses_tasks_and_people_like_upstream() {
        let model = parse(
            "journey\n\
title Adding journey diagram functionality to mermaid\n\
section Documentation\n\
A task: 5: Alice, Bob, Charlie\n\
B task: 3:Bob, Charlie\n\
C task: 5\n\
D task: 5: Charlie, Alice\n\
E task: 5:\n\
section Another section\n\
P task: 5:\n\
Q task: 5:\n\
R task: 5:\n",
        );

        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 8);

        assert_eq!(
            tasks[0],
            json!({
                "score": 5,
                "people": ["Alice", "Bob", "Charlie"],
                "section": "Documentation",
                "task": "A task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[1],
            json!({
                "score": 3,
                "people": ["Bob", "Charlie"],
                "section": "Documentation",
                "task": "B task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[2],
            json!({
                "score": 5,
                "people": [],
                "section": "Documentation",
                "task": "C task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[3],
            json!({
                "score": 5,
                "people": ["Charlie", "Alice"],
                "section": "Documentation",
                "task": "D task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[4],
            json!({
                "score": 5,
                "people": [""],
                "section": "Documentation",
                "task": "E task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[5],
            json!({
                "score": 5,
                "people": [""],
                "section": "Another section",
                "task": "P task",
                "type": "Another section",
            })
        );
        assert_eq!(
            tasks[6],
            json!({
                "score": 5,
                "people": [""],
                "section": "Another section",
                "task": "Q task",
                "type": "Another section",
            })
        );
        assert_eq!(
            tasks[7],
            json!({
                "score": 5,
                "people": [""],
                "section": "Another section",
                "task": "R task",
                "type": "Another section",
            })
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_typed_tasks_retain_duplicate_section_occurrences() {
        let source = concat!(
            "journey\n",
            "section Repeated\n",
            "First: 5: Alice\n",
            "section Repeated\n",
            "Second: 3: Bob\n",
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("duplicate Journey sections should parse")
            .expect("Journey source should be detected");
        let merman_model::presentation::RenderSemanticModel::Journey(model) = parsed.model() else {
            panic!("expected a Journey render model");
        };

        assert_eq!(
            model.sections,
            vec!["Repeated".to_string(), "Repeated".to_string()]
        );
        assert_eq!(
            model
                .tasks
                .iter()
                .map(|task| task.section_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1)]
        );
        assert_eq!(
            serde_json::to_value(&model.tasks[0]).expect("typed task should serialize")["sectionIndex"],
            json!(0)
        );

        let compat = parsed
            .compatibility_json()
            .expect("Journey compatibility projection should serialize");
        assert!(compat["tasks"][0].get("sectionIndex").is_none());
    }
}

#[cfg(all(test, feature = "presentation"))]
mod output_data_tests {
    use super::*;

    #[test]
    fn full_producer_preserves_empty_output_independently_of_json_projection() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "journey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let parse = |source| {
            parse_journey_model_for_render_controlled(
                source,
                &meta,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap()
        };
        let (empty, empty_output) = parse("");
        let (header, header_output) = parse("journey");
        assert_eq!(empty_output, JourneyOutputData::Empty);
        assert_eq!(header_output, JourneyOutputData::Model);
        assert_eq!(JourneyOutputData::default(), JourneyOutputData::Model);
        // Identical canonical values must not erase the source producer's distinct output shape.
        assert_eq!(
            serde_json::to_value(&empty).unwrap(),
            serde_json::to_value(&header).unwrap()
        );
        #[cfg(feature = "compat-json")]
        {
            assert_eq!(
                render_model_to_compat_json(&empty, empty_output, &meta).unwrap(),
                serde_json::json!({})
            );
            assert_eq!(
                render_model_to_compat_json(
                    &JourneyDiagramModel::default(),
                    JourneyOutputData::default(),
                    &meta
                )
                .unwrap()["type"],
                "journey"
            );
        }
    }
}
