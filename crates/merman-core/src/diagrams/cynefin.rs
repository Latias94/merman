#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(all(test, feature = "compat-json", feature = "editor-facts"))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::{
    push_langium_common_editor_fact, push_langium_common_recovery,
};
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, Result};
use merman_core_structured::cynefin::{CynefinIssue, parse_cynefin_controlled};
#[cfg(feature = "editor-facts")]
use merman_core_structured::cynefin::{CynefinObservation, SpannedText};
pub(crate) use merman_model::families::cynefin::CynefinDiagramModel;
#[cfg(feature = "compat-json")]
use serde_json::Value;

struct CynefinSemanticSource {
    model: CynefinDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

impl CynefinSemanticSource {
    fn into_model(mut self, config: &crate::MermaidConfig) -> CynefinDiagramModel {
        crate::common_db::sanitize_optional_title(&mut self.model.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.model.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.model.acc_descr, config);
        self.model
    }
}

struct CynefinParseOutcome {
    source: CynefinSemanticSource,
    first_error: Option<Error>,
}

impl CynefinParseOutcome {
    #[cfg(any(test, feature = "presentation"))]
    fn into_strict_source(self) -> Result<CynefinSemanticSource> {
        match self.first_error {
            Some(error) => Err(error),
            None => Ok(self.source),
        }
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_cynefin(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model =
        parse_cynefin_semantic_source(code, meta)?.into_model(meta.effective_config.as_config());
    render_model_to_compat_json(&model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_cynefin_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    let CynefinParseOutcome {
        source,
        first_error,
    } = construct_cynefin_parse_outcome_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    let construction = match first_error {
        Some(error) => Err(crate::family::CombinedSemanticFailure::new(
            error,
            source.editor_facts,
        )),
        None => Ok(source),
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |mut source| {
            let editor_facts = source.editor_facts.take().unwrap_or_default();
            let model = source.into_model(meta.effective_config.as_config());
            (render_model_to_compat_json(&model, meta), editor_facts)
        },
        crate::family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &CynefinDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::cynefin::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(all(test, feature = "all-diagrams", feature = "presentation"))]
pub(crate) fn parse_cynefin_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<CynefinDiagramModel> {
    Ok(parse_cynefin_semantic_source(code, meta)?.into_model(meta.effective_config.as_config()))
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_cynefin_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<CynefinDiagramModel>> {
    parse_cynefin_model_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        control,
    )
}

pub(crate) fn parse_cynefin_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<CynefinDiagramModel>> {
    let CynefinParseOutcome {
        source,
        first_error,
    } = construct_cynefin_parse_outcome_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let mut source = source;
    #[cfg(feature = "editor-facts")]
    let editor_facts = source.editor_facts.take();
    let model = match first_error {
        Some(error) => Err(error),
        None => Ok(source.into_model(&meta.semantic_config)),
    };
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

#[cfg(feature = "presentation")]
fn parse_cynefin_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<CynefinDiagramModel>> {
    let outcome = construct_cynefin_parse_outcome_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match outcome.into_strict_source() {
        Ok(source) => source,
        Err(error) => return Ok(Err(error)),
    };
    Ok(Ok(source.into_model(config)))
}

#[cfg(any(
    feature = "compat-json",
    all(test, feature = "all-diagrams", feature = "presentation")
))]
fn parse_cynefin_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<CynefinSemanticSource> {
    construct_cynefin_parse_outcome(code, &meta.diagram_type).into_strict_source()
}

#[cfg(any(
    feature = "compat-json",
    all(test, feature = "all-diagrams", feature = "presentation")
))]
fn construct_cynefin_parse_outcome(code: &str, diagram_type: &str) -> CynefinParseOutcome {
    construct_cynefin_parse_outcome_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_cynefin_parse_outcome_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<CynefinParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("cynefin");

    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand
        .needs_recovery()
        .then(EditorSemanticFacts::new);
    #[cfg(feature = "editor-facts")]
    let mut observe = editor_facts.as_mut().map(|facts| {
        move |observation: CynefinObservation<'_>| project_cynefin_observation(facts, observation)
    });
    let (model, first_issue) = parse_cynefin_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand.captures_rich(),
        #[cfg(feature = "editor-facts")]
        observe
            .as_mut()
            .map(|observe| observe as &mut dyn FnMut(CynefinObservation<'_>)),
        control,
    )?;
    Ok(CynefinParseOutcome {
        source: CynefinSemanticSource {
            model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        },
        first_error: first_issue.map(|issue| cynefin_error(diagram_type, issue)),
    })
}

const EXPECTED_HEADER: &str = "expected cynefin-beta header";
const ORPHAN_ITEM: &str = "cynefin item must follow a domain";

fn cynefin_error(diagram_type: &str, issue: CynefinIssue) -> Error {
    match issue {
        CynefinIssue::ExpectedHeader(span) => {
            Error::diagram_parse_exact(diagram_type, EXPECTED_HEADER, span)
        }
        CynefinIssue::Common(diagnostic) => Error::diagram_parse_insertion_point(
            diagram_type,
            diagnostic.message,
            diagnostic.span.start,
        ),
        CynefinIssue::InvalidBody { message, span } => {
            Error::diagram_parse_exact(diagram_type, message, span)
        }
        CynefinIssue::OrphanItem(span) => {
            Error::diagram_parse_exact(diagram_type, ORPHAN_ITEM, span)
        }
    }
}

#[cfg(feature = "editor-facts")]
fn project_cynefin_observation(
    facts: &mut EditorSemanticFacts,
    observation: CynefinObservation<'_>,
) {
    match observation {
        CynefinObservation::Common(fact) => push_langium_common_editor_fact(facts, fact, "cynefin"),
        CynefinObservation::Domain(domain) => push_domain_fact(facts, domain, "cynefin domain"),
        CynefinObservation::Item(item) => push_payload_fact(facts, item, "cynefin domain item"),
        CynefinObservation::Transition { from, to, label } => {
            push_domain_fact(facts, from, "cynefin transition source");
            push_domain_fact(facts, to, "cynefin transition target");
            if let Some(label) = label {
                push_payload_fact(facts, label, "cynefin transition label");
            }
        }
        CynefinObservation::Issue(issue) => match issue {
            CynefinIssue::ExpectedHeader(span) => {
                facts.mark_recovered_from_parse_error(EXPECTED_HEADER, Some(*span))
            }
            CynefinIssue::Common(diagnostic) => push_langium_common_recovery(facts, diagnostic),
            CynefinIssue::InvalidBody { message, span } => facts.mark_recovered_from_parse_error(
                format!("cynefin parser recovered after parse error: {message}"),
                Some(*span),
            ),
            CynefinIssue::OrphanItem(span) => {
                facts.mark_recovered_from_parse_error(ORPHAN_ITEM, Some(*span))
            }
        },
        CynefinObservation::SelfLoop(domain) => facts.push_diagnostic(
            format!(
                "cynefin self-loop transition on domain \"{}\" is skipped",
                domain.text
            ),
            Some(domain.span),
        ),
    }
}

#[cfg(feature = "editor-facts")]
fn push_domain_fact(facts: &mut EditorSemanticFacts, domain: &SpannedText, detail: &'static str) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        domain.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::outline(
        domain.text.clone(),
        Some(detail.to_string()),
        EditorSemanticKind::Namespace,
        domain.span,
        domain.selection,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_payload_fact(facts: &mut EditorSemanticFacts, value: &SpannedText, detail: &'static str) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        value.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        value.text.clone(),
        Some(detail.to_string()),
        EditorSemanticKind::String,
        value.span,
        value.selection,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_keeps_parser_warnings_without_rich_observers() {
        let source = "cynefin-beta\ncomplex\ncomplicated --> complicated : \"Self-loop\"\n";
        let capture = |demand| {
            construct_cynefin_parse_outcome_controlled(
                source,
                "cynefin",
                demand,
                &crate::OperationControl::new(),
            )
            .unwrap()
        };
        let full = capture(EditorFactsDemand::Capture);
        let full_facts = full.source.editor_facts.unwrap();
        assert!(!full_facts.symbols.is_empty());
        crate::editor::test_work::reset();
        crate::diagrams::langium_common::reset_family_syntax_construction_count("cynefin");
        let recovery = capture(EditorFactsDemand::Recovery);
        assert!(recovery.first_error.is_none());
        assert_eq!(
            recovery.source.model.domains.len(),
            full.source.model.domains.len()
        );
        let facts = recovery.source.editor_facts.unwrap();
        assert_eq!(facts.diagnostics, full_facts.diagnostics);
        assert_eq!(facts.diagnostics.len(), 1);
        assert_eq!(
            facts.diagnostics[0].kind,
            crate::EditorSemanticDiagnosticKind::ParserWarning
        );
        assert!(facts.symbols.is_empty());
        assert!(facts.directive_prefixes.is_empty());
        assert!(facts.expected_syntax.is_empty());
        let work = crate::editor::test_work::read();
        assert_eq!(
            (work.symbols, work.directives, work.expected_syntax),
            (0, 0, 0)
        );
        assert_eq!(work.diagnostics, 1);
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("cynefin"),
            1
        );
    }

    #[test]
    fn controlled_parse_can_cancel_between_cynefin_lines() {
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            construct_cynefin_parse_outcome_controlled(
                "cynefin-beta\ncomplex\n\"Probe\"\n",
                "cynefin",
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn cynefin_recovery_uses_exact_trimmed_crlf_spans() {
        for (source, invalid) in [
            ("  invalid header %% hidden\r\n", "invalid header"),
            (
                "cynefin-beta\r\n  invalid body %% hidden\r\n",
                "invalid body",
            ),
            ("cynefin-beta\r\n  \"orphan\" %% hidden\r\n", "\"orphan\""),
        ] {
            let facts = crate::MermaidEngine::new()
                .parse_editor_semantic_facts_with_type_sync("cynefin", source)
                .unwrap()
                .unwrap();
            let start = source.find(invalid).unwrap();
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(SourceSpan::new(start, start + invalid.len()))
            }));
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn cynefin_recovery_preserves_cursor_prefix_for_crlf_input() {
        let source = concat!(
            "cynefin-beta\r\n",
            "  complex --> ??? %% malformed target\r\n",
            "title Later\r\n",
            "clear\r\n",
        );
        let invalid = "complex --> ???";
        let invalid_start = source.find(invalid).unwrap();

        let facts = crate::family::test_support::editor_facts(
            parse_cynefin_json_and_editor_facts,
            source,
            &crate::SourcePresentationMetadata {
                diagram_type: "cynefin".to_string(),
                config: crate::MermaidConfig::empty_object(),
                effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                title: None,
            },
        );

        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span
                    == Some(SourceSpan::new(
                        invalid_start,
                        invalid_start + invalid.len(),
                    ))
        }));
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn typed_render_model_projects_exact_compatibility_json() {
        let source = concat!(
            "cynefin-beta\n",
            "title Cynefin Map\n",
            "accTitle: Accessible Map\n",
            "complex\n",
            "  \"Probe\"\n",
            "complex --> complicated : \"Sense\"\n",
        );
        let engine = crate::MermaidEngine::new();
        let compat = engine
            .parse_diagram_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let typed = engine
            .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::Cynefin(model) = typed.model() else {
            panic!("expected Cynefin render model");
        };

        assert_eq!(
            merman_model::families::cynefin::compat::project(
                model,
                &merman_model::presentation::compat::CompatJsonMetadata::from(typed.metadata())
            )
            .unwrap(),
            compat.model
        );
    }

    #[test]
    fn semantic_sanitization_preserves_domain_payloads_and_accessibility_normalization() {
        let source = concat!(
            "cynefin-beta\n",
            "title <script>secret</script><b>Header &amp; &#x3C;</b>\n",
            "accTitle: <script>secret</script>   Accessible\n",
            "accDescr {First\n    <b>Second &amp; &#x3C;</b>}\n",
            "complex\n",
            "\"<script>raw</script>&amp;\"\n",
            "complex --> clear : \"<b>raw &amp;</b>\"\n",
        );
        for security_level in ["strict", "loose"] {
            let metadata = crate::SemanticMetadata {
                diagram_type: "cynefin".to_string(),
                config: crate::MermaidConfig::empty_object(),
                semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                    "securityLevel": security_level,
                    "htmlLabels": true
                })),
                title: None,
            };
            let model = parse_cynefin_semantic_model_controlled(
                source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.title.as_deref(), Some("<b>Header &amp; &lt;</b>"));
            assert_eq!(model.acc_title.as_deref(), Some("Accessible"));
            assert_eq!(
                model.acc_descr.as_deref(),
                Some("First\n<b>Second &amp; &lt;</b>")
            );
            assert_eq!(model.domains[0].items[0].label, "<script>raw</script>&amp;");
            assert_eq!(
                model.transitions[0].label.as_deref(),
                Some("<b>raw &amp;</b>")
            );
        }
    }

    #[test]
    fn semantic_model_constructs_without_editor_facts() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("cynefin");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let outcome = construct_cynefin_parse_outcome_controlled(
            "cynefin-beta\ncomplex\n\"Probe\"\ncomplex --> clear : \"Sense\"\n",
            "cynefin",
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap();
        let source = outcome.into_strict_source().unwrap();
        #[cfg(feature = "compat-json")]
        #[cfg(feature = "editor-facts")]
        assert!(source.editor_facts.is_none());
        assert_eq!(source.model.domains[0].items[0].label, "Probe");
        assert_eq!(source.model.transitions[0].from, "complex");
        assert_eq!(source.model.transitions[0].to, "clear");
        assert_eq!(source.model.transitions[0].label.as_deref(), Some("Sense"));
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("cynefin"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
}
