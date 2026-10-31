//! Semantic-only parsing without resolving Mermaid themes or creating render state.

use crate::preprocess::preprocess_builtin_semantic_controlled;
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result};

#[cfg(feature = "editor-facts")]
mod snapshot;
#[cfg(feature = "editor-facts")]
pub use snapshot::{SemanticDiagramSnapshot, SemanticParseOutcome, SemanticSnapshotCapture};

use merman_model::semantic::{ParsedSemanticDiagram, SemanticDiagramModel, SemanticMetadata};

fn remap_semantic_source_positions(
    model: &mut SemanticDiagramModel,
    source: &crate::PreprocessedSource,
    control: &OperationControl,
) -> OperationControlResult<()> {
    control.checkpoint()?;
    #[allow(unreachable_patterns)]
    let facts: &mut [crate::DiagramWarningFact] = match model {
        #[cfg(feature = "diagram-agentflow")]
        SemanticDiagramModel::Agentflow(model) => {
            let (line_offset, column_offset) = source.parser_position_offset();
            crate::diagrams::agentflow::offset_diagnostic_positions(
                model,
                line_offset,
                column_offset,
            );
            &mut model.warning_facts
        }
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        SemanticDiagramModel::Flowchart(model) => &mut model.warning_facts,
        #[cfg(feature = "diagram-block")]
        SemanticDiagramModel::Block(model) => &mut model.warning_facts,
        #[cfg(feature = "diagram-git-graph")]
        SemanticDiagramModel::GitGraph(model) => &mut model.warning_facts,
        _ => &mut [],
    };
    for (index, fact) in facts.iter_mut().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        source.remap_warning_fact_spans(fact);
    }
    control.checkpoint()
}
/// Parses diagram meaning without requiring theme resolution, layout, or SVG rendering.
///
/// Enabled families publish their canonical parser through the family catalog.
/// Recognized families omitted from the build return [`Error::UnsupportedDiagram`].
#[derive(Debug, Clone)]
pub struct SemanticParser {
    site_config: MermaidConfig,
    site_config_overrides: MermaidConfig,
    runtime_policy: merman_model::runtime::RuntimePolicy,
}

struct SemanticTiming {
    total: Option<merman_model::runtime::OperationTimer>,
    input_bytes: usize,
}

impl SemanticTiming {
    fn new(context: &merman_model::runtime::OperationContext, input: &str) -> Self {
        Self {
            total: context
                .timing()
                .map(merman_model::runtime::OperationTiming::start),
            input_bytes: input.len(),
        }
    }

    fn log(self) {
        if let Some(total) = self.total {
            eprintln!(
                "[parse-timing] contract=semantic total={:?} input_bytes={}",
                total.elapsed(),
                self.input_bytes,
            );
        }
    }
}

impl Default for SemanticParser {
    fn default() -> Self {
        Self {
            site_config: crate::generated::default_site_config(),
            site_config_overrides: MermaidConfig::empty_object(),
            runtime_policy: merman_model::runtime::RuntimePolicy::deterministic(),
        }
    }
}

impl SemanticParser {
    /// Creates a parser with Mermaid defaults and Merman's hardened source-config policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the runtime policy used to begin future parse operations.
    pub fn with_runtime_policy(mut self, policy: merman_model::runtime::RuntimePolicy) -> Self {
        self.runtime_policy = policy;
        self
    }

    /// Replays a frozen runtime context without consulting system state.
    pub fn with_operation_context(self, context: merman_model::runtime::OperationContext) -> Self {
        self.with_runtime_policy(
            merman_model::runtime::RuntimePolicy::from_operation_context(context),
        )
    }

    pub fn runtime_policy(&self) -> &merman_model::runtime::RuntimePolicy {
        &self.runtime_policy
    }

    /// Returns the explicit host overrides applied before source-config admission.
    pub fn site_config_overrides(&self) -> &MermaidConfig {
        &self.site_config_overrides
    }

    pub fn begin_operation(
        &self,
    ) -> std::result::Result<
        merman_model::runtime::OperationContext,
        merman_model::runtime::RuntimePolicyError,
    > {
        self.runtime_policy.begin_operation()
    }

    /// Captures canonical meaning and parser recovery facts without resolving presentation.
    #[cfg(feature = "editor-facts")]
    pub fn capture_diagram_snapshot_sync(&self, input: &str) -> SemanticSnapshotCapture {
        self.capture_diagram_snapshot_controlled_sync(input, &OperationControl::new())
            .expect("a private semantic snapshot control cannot be cancelled")
    }

    /// Captures a semantic snapshot; cooperative cancellation remains the outer error channel.
    #[cfg(feature = "editor-facts")]
    pub fn capture_diagram_snapshot_controlled_sync(
        &self,
        input: &str,
        control: &OperationControl,
    ) -> OperationControlResult<SemanticSnapshotCapture> {
        self.capture_snapshot_controlled(
            input,
            None,
            crate::family::EditorFactsDemand::Capture,
            control,
        )
    }

    /// Captures canonical meaning and every parser diagnostic without editor navigation facts.
    ///
    /// The family is constructed once, using the same recovery, source mapping, runtime, and
    /// cancellation contract as a rich snapshot. Symbols, directive prefixes, and expected syntax
    /// are omitted from the returned editor facts.
    #[cfg(feature = "editor-facts")]
    pub fn capture_diagram_recovery_snapshot_controlled_sync(
        &self,
        input: &str,
        control: &OperationControl,
    ) -> OperationControlResult<SemanticSnapshotCapture> {
        self.capture_snapshot_controlled(
            input,
            None,
            crate::family::EditorFactsDemand::Recovery,
            control,
        )
    }

    /// Captures a semantic snapshot for a known built-in diagram type.
    #[cfg(feature = "editor-facts")]
    pub fn capture_diagram_snapshot_with_type_sync(
        &self,
        diagram_type: &str,
        input: &str,
    ) -> SemanticSnapshotCapture {
        self.capture_diagram_snapshot_with_type_controlled_sync(
            diagram_type,
            input,
            &OperationControl::new(),
        )
        .expect("a private semantic snapshot control cannot be cancelled")
    }

    /// Known-type capture with the same cancellation and recovery contract as detection capture.
    #[cfg(feature = "editor-facts")]
    pub fn capture_diagram_snapshot_with_type_controlled_sync(
        &self,
        diagram_type: &str,
        input: &str,
        control: &OperationControl,
    ) -> OperationControlResult<SemanticSnapshotCapture> {
        self.capture_snapshot_controlled(
            input,
            Some(diagram_type),
            crate::family::EditorFactsDemand::Capture,
            control,
        )
    }

    #[cfg(feature = "editor-facts")]
    fn capture_snapshot_controlled(
        &self,
        input: &str,
        diagram_type: Option<&str>,
        editor_demand: crate::family::EditorFactsDemand,
        control: &OperationControl,
    ) -> OperationControlResult<SemanticSnapshotCapture> {
        use crate::preprocess::{PreprocessCaptureResult, SourceConfigEvidence};

        control.checkpoint()?;
        let context = match self.begin_operation() {
            Ok(context) => context,
            Err(error) => {
                return Ok(SemanticSnapshotCapture::Failed {
                    error: error.into(),
                    source_config: SourceConfigEvidence::default(),
                });
            }
        };
        let timing = SemanticTiming::new(&context, input);
        let result = crate::runtime::with_operation_context(&context, || {
            let captured = crate::preprocess::preprocess_builtin_semantic_with_evidence_controlled(
                input,
                diagram_type,
                editor_demand,
                control,
            )?;
            let source_config = captured.source_config;
            let pre = match captured.outcome {
                PreprocessCaptureResult::Ready(pre) => *pre,
                PreprocessCaptureResult::Failed(error) => {
                    return Ok(SemanticSnapshotCapture::Failed {
                        error,
                        source_config,
                    });
                }
                PreprocessCaptureResult::Panicked(panic) => {
                    return Ok(SemanticSnapshotCapture::Panicked {
                        panic,
                        source_config,
                    });
                }
            };
            let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let diagram_type = match diagram_type {
                    Some(diagram_type) => diagram_type,
                    None => match crate::detect::detect_builtin_precleaned_controlled(
                        pre.code(),
                        control,
                    )? {
                        Ok(fact) => fact.id,
                        Err(error) => return Ok(Err(error)),
                    },
                };
                self.prepare_semantic_source_controlled(diagram_type, pre, control)
            }));
            let (metadata, source) = match prepared {
                Ok(ready) => match ready? {
                    Ok(ready) => ready,
                    Err(error) => {
                        return Ok(SemanticSnapshotCapture::Failed {
                            error,
                            source_config,
                        });
                    }
                },
                Err(payload) => {
                    return Ok(SemanticSnapshotCapture::Panicked {
                        panic: crate::CapturedPanic::from_payload(payload),
                        source_config,
                    });
                }
            };
            let recovered_incomplete_directive = source.recovered_incomplete_directive();
            let family_demand = if editor_demand == crate::family::EditorFactsDemand::Recovery
                && source.may_drop_editor_spans_controlled(control)?
            {
                // Preserve the existing dropped-span diagnostics without reparsing. The final
                // recovery projection discards navigation facts after their spans are remapped.
                crate::family::EditorFactsDemand::Capture
            } else {
                editor_demand
            };
            let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                control.checkpoint()?;
                match crate::family::canonical_parser(&metadata.diagram_type) {
                    Some(parser) => parser(source.text(), &metadata, family_demand, control),
                    None => Ok(crate::family::SemanticConstruction {
                        model: Err(Error::UnsupportedDiagram {
                            diagram_type: metadata.diagram_type.clone(),
                        }),
                        editor_facts: None,
                    }),
                }
            }));
            control.checkpoint()?;
            let (outcome, editor_facts) = match parsed {
                Err(payload) => (
                    SemanticParseOutcome::Panicked(
                        crate::CapturedPanic::from_payload(payload)
                            .message()
                            .to_string(),
                    ),
                    crate::ParsedEditorFacts::Unavailable,
                ),
                Ok(construction) => {
                    let construction = construction?;
                    let editor_facts = match construction.editor_facts {
                        Some(facts) => crate::ParsedEditorFacts::Available(
                            source.finish_editor_semantic_facts_with_demand(
                                facts,
                                &metadata.diagram_type,
                                editor_demand,
                                control,
                            )?,
                        ),
                        None => crate::ParsedEditorFacts::Unavailable,
                    };
                    let outcome = match construction.model {
                        Ok(mut model) => {
                            remap_semantic_source_positions(&mut model, &source, control)?;
                            SemanticParseOutcome::Parsed(model)
                        }
                        Err(error) => SemanticParseOutcome::Failed(source.remap_parse_error(error)),
                    };
                    (outcome, editor_facts)
                }
            };
            control.checkpoint()?;
            Ok(SemanticSnapshotCapture::Snapshot(
                SemanticDiagramSnapshot::new(
                    metadata,
                    outcome,
                    editor_facts,
                    source_config,
                    recovered_incomplete_directive,
                ),
            ))
        });
        timing.log();
        result
    }

    /// Applies host-level config before parsing. An explicit `secure` array replaces the
    /// hardened defaults, matching the full parser's site-policy behavior.
    pub fn with_site_config(mut self, mut overrides: MermaidConfig) -> Self {
        if overrides.is_empty_object() {
            return self;
        }
        crate::config::mirror_legacy_font_family_into_theme_variables(&mut overrides);
        normalize_global_theme(&mut overrides);
        let explicit_secure = overrides
            .as_value()
            .get("secure")
            .filter(|value| value.is_array())
            .map(crate::json_value::clone_value_nonrecursive);

        self.site_config_overrides.deep_merge(overrides.as_value());
        self.site_config.deep_merge(overrides.as_value());
        if let Some(secure) = explicit_secure {
            self.site_config_overrides
                .set_value("secure", secure.clone());
            self.site_config.set_value("secure", secure);
        }
        self
    }

    /// Detects and parses a built-in family supported by the semantic facade.
    pub fn parse_diagram_sync(&self, input: &str) -> Result<ParsedSemanticDiagram> {
        let control = OperationControl::new();
        self.parse_diagram_controlled_sync(input, &control)
            .expect("a private semantic parse control cannot be cancelled")
    }

    /// Controlled variant for callers that need to cancel detection or semantic parsing.
    pub fn parse_diagram_controlled_sync(
        &self,
        input: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        self.parse_input_controlled(input, None, control)
    }

    /// Parses canonical meaning with the shared strict or lenient model contract.
    ///
    /// The caller supplies the runtime context shared by parsing and downstream targets. This
    /// method never begins a replacement operation or consults the parser's runtime policy.
    /// Detection failures produce no diagram in lenient mode; preprocessing failures remain
    /// errors, and detected family failures become canonical `error` diagram models.
    pub fn parse_diagram_with_options_controlled_in_context_sync(
        &self,
        input: &str,
        options: crate::ParseOptions,
        control: &OperationControl,
        context: &merman_model::runtime::OperationContext,
    ) -> OperationControlResult<Result<Option<ParsedSemanticDiagram>>> {
        use crate::preprocess::{
            DirectiveRecoveryMode, preprocess_builtin_semantic_with_directive_recovery_controlled,
        };

        control.checkpoint_at(crate::OperationPhase::Admission)?;
        let control = control.for_phase(crate::OperationPhase::Parse);
        control.checkpoint()?;
        let timing = SemanticTiming::new(context, input);
        let result = crate::runtime::with_operation_context(context, || {
            let recovery = if options.suppress_errors {
                DirectiveRecoveryMode::RecoverLine
            } else {
                DirectiveRecoveryMode::Strict
            };
            let pre = match preprocess_builtin_semantic_with_directive_recovery_controlled(
                input, None, recovery, &control,
            )? {
                Ok(pre) => pre,
                Err(Error::DetectType(_)) if options.suppress_errors => return Ok(Ok(None)),
                Err(error) => return Ok(Err(error)),
            };
            if pre.code().trim_start().starts_with("---") {
                return Ok(Err(Error::MalformedFrontMatter));
            }
            let diagram_type =
                match crate::detect::detect_builtin_precleaned_controlled(pre.code(), &control)? {
                    Ok(fact) => fact.id,
                    Err(Error::DetectType(_)) if options.suppress_errors => return Ok(Ok(None)),
                    Err(error) => return Ok(Err(error)),
                };
            Ok(self
                .parse_preprocessed_controlled(diagram_type, pre, options, &control)?
                .map(Some))
        });
        timing.log();
        result
    }

    /// Parses a diagram when its Mermaid type is already known and supported by this facade.
    pub fn parse_diagram_with_type_sync(
        &self,
        diagram_type: &str,
        input: &str,
    ) -> Result<ParsedSemanticDiagram> {
        let control = OperationControl::new();
        self.parse_diagram_with_type_controlled_sync(diagram_type, input, &control)
            .expect("a private semantic parse control cannot be cancelled")
    }

    /// Controlled variant for callers that need to cancel semantic parsing.
    pub fn parse_diagram_with_type_controlled_sync(
        &self,
        diagram_type: &str,
        input: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        self.parse_input_controlled(input, Some(diagram_type), control)
    }

    fn parse_input_controlled(
        &self,
        input: &str,
        diagram_type: Option<&str>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        control.checkpoint()?;
        let context = match self.begin_operation() {
            Ok(context) => context,
            Err(error) => return Ok(Err(error.into())),
        };
        let timing = SemanticTiming::new(&context, input);
        let result = crate::runtime::with_operation_context(&context, || {
            let pre = match preprocess_builtin_semantic_controlled(input, diagram_type, control)? {
                Ok(pre) => pre,
                Err(error) => return Ok(Err(error)),
            };
            let diagram_type = match diagram_type {
                Some(diagram_type) => diagram_type,
                None => {
                    match crate::detect::detect_builtin_precleaned_controlled(pre.code(), control)?
                    {
                        Ok(fact) => fact.id,
                        Err(error) => return Ok(Err(error)),
                    }
                }
            };
            self.parse_preprocessed_controlled(
                diagram_type,
                pre,
                crate::ParseOptions::strict(),
                control,
            )
        });
        timing.log();
        result
    }

    fn parse_preprocessed_controlled(
        &self,
        diagram_type: &str,
        pre: crate::PreprocessResult,
        options: crate::ParseOptions,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        let (mut metadata, source) =
            match self.prepare_semantic_source_controlled(diagram_type, pre, control)? {
                Ok(prepared) => prepared,
                Err(error) => return Ok(Err(error)),
            };
        control.checkpoint_at(crate::OperationPhase::Semantic)?;
        let parsed = match crate::family::canonical_parser(diagram_type) {
            Some(parser) => {
                parser(
                    source.text(),
                    &metadata,
                    #[cfg(feature = "editor-facts")]
                    crate::family::EditorFactsDemand::Omit,
                    control,
                )?
                .model
            }
            None => Err(Error::UnsupportedDiagram {
                diagram_type: diagram_type.to_string(),
            }),
        };
        control.checkpoint_at(crate::OperationPhase::Semantic)?;
        let mut model = match parsed {
            Ok(model) => model,
            Err(error) => {
                let error = source.remap_parse_error(error);
                if !options.suppress_errors {
                    return Ok(Err(error));
                }
                metadata.diagram_type = "error".to_string();
                return Ok(Ok(ParsedSemanticDiagram {
                    metadata,
                    model: SemanticDiagramModel::Error(
                        crate::diagrams::error_diagram::suppressed_error_model(&error),
                    ),
                }));
            }
        };
        remap_semantic_source_positions(&mut model, &source, control)?;
        control.checkpoint()?;
        Ok(Ok(ParsedSemanticDiagram { metadata, model }))
    }

    fn prepare_semantic_source_controlled(
        &self,
        diagram_type: &str,
        pre: crate::PreprocessResult,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(SemanticMetadata, crate::PreprocessedSource)>> {
        control.checkpoint()?;
        if pre.code().trim_start().starts_with("---") {
            return Ok(Err(Error::MalformedFrontMatter));
        }

        let upstream_defaults = crate::generated::upstream_default_config();
        let admitted_source = if pre.config.is_empty_object() {
            pre.config.clone()
        } else {
            crate::config::source_filtered_overrides(&self.site_config, &pre.config, control)?
        };
        let mut semantic_config = self.site_config.clone();
        if !admitted_source.is_empty_object() {
            semantic_config.deep_merge(admitted_source.as_value());
        }
        crate::config::resolve_semantic_appearance(
            diagram_type,
            &admitted_source,
            &self.site_config_overrides,
            &upstream_defaults,
            &mut semantic_config,
            control,
        )?;
        crate::detect::builtin_config_effect(diagram_type).apply(&mut semantic_config);
        let source_authority = if self
            .site_config_overrides
            .as_value()
            .get("secure")
            .is_some()
        {
            &self.site_config_overrides
        } else {
            &upstream_defaults
        };
        let config =
            crate::config::authored_source_overrides(source_authority, pre.config, control)?;
        let title = pre
            .title
            .as_deref()
            .map(|title| crate::sanitize::sanitize_text(title, &semantic_config));
        let metadata = SemanticMetadata {
            diagram_type: diagram_type.to_string(),
            config,
            semantic_config,
            title,
        };
        let source = crate::preprocess::select_family_parser_source_controlled(
            pre.source,
            pre.with_comments,
            diagram_type,
            &metadata.semantic_config,
            control,
        )?;
        Ok(Ok((metadata, source)))
    }
}

fn normalize_global_theme(config: &mut MermaidConfig) {
    let invalid_theme = config.as_value().get("theme").is_some_and(|value| {
        !value.is_null()
            && !value
                .as_str()
                .is_some_and(|name| name == "null" || crate::MermaidThemeId::NAMES.contains(&name))
    });
    if invalid_theme
        && let Some(theme) = crate::generated::upstream_default_config()
            .as_value()
            .get("theme")
    {
        config.set_value("theme", crate::json_value::clone_value_nonrecursive(theme));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "editor-facts")]
    fn assert_recovery_snapshot_matches_rich(
        source: &str,
        omit_rich_work: bool,
    ) -> SemanticDiagramSnapshot {
        let parser = SemanticParser::new();
        let SemanticSnapshotCapture::Snapshot(rich) = parser.capture_diagram_snapshot_sync(source)
        else {
            panic!("fixture must reach its family parser: {source}");
        };
        crate::editor::test_work::reset();
        let SemanticSnapshotCapture::Snapshot(recovery) = parser
            .capture_diagram_recovery_snapshot_controlled_sync(source, &OperationControl::new())
            .unwrap()
        else {
            panic!("recovery fixture must reach its family parser: {source}");
        };
        let work = crate::editor::test_work::read();
        if omit_rich_work {
            assert_eq!(work.symbols, 0, "{source}");
            assert_eq!(work.directives, 0, "{source}");
            assert_eq!(work.expected_syntax, 0, "{source}");
        }
        assert_eq!(
            recovery.metadata().diagram_type,
            rich.metadata().diagram_type
        );
        assert_eq!(
            recovery.metadata().config.as_value(),
            rich.metadata().config.as_value()
        );
        assert_eq!(
            recovery.metadata().semantic_config.as_value(),
            rich.metadata().semantic_config.as_value()
        );
        assert_eq!(recovery.source_config(), rich.source_config());
        assert_eq!(
            recovery.recovered_incomplete_directive(),
            rich.recovered_incomplete_directive()
        );
        let outcome_fields = |outcome: &SemanticParseOutcome| match outcome {
            #[cfg(feature = "diagram-state")]
            SemanticParseOutcome::Parsed(SemanticDiagramModel::State(model)) => {
                let crate::diagrams::state::StateDiagramModel {
                    document,
                    direction,
                    acc_title,
                    acc_descr,
                    nodes,
                    edges,
                    relations,
                    links,
                    states,
                    style_classes,
                } = model;
                // HashMap iteration is not part of State meaning. Preserve every field and
                // every ordered sequence while comparing these two maps by key.
                let links = links.iter().collect::<std::collections::BTreeMap<_, _>>();
                let states = states.iter().collect::<std::collections::BTreeMap<_, _>>();
                format!(
                    "State{:?}",
                    (
                        document,
                        direction,
                        acc_title,
                        acc_descr,
                        nodes,
                        edges,
                        relations,
                        links,
                        states,
                        style_classes,
                    )
                )
            }
            _ => format!("{outcome:?}"),
        };
        assert_eq!(
            outcome_fields(recovery.outcome()),
            outcome_fields(rich.outcome()),
            "{source}"
        );
        let crate::ParsedEditorFacts::Available(rich_facts) = rich.editor_facts() else {
            panic!("rich family facts must be available");
        };
        let crate::ParsedEditorFacts::Available(recovery_facts) = recovery.editor_facts() else {
            panic!("recovery family facts must be available");
        };
        assert_eq!(
            recovery_facts.completeness, rich_facts.completeness,
            "{source}"
        );
        assert_eq!(
            recovery_facts.family_semantics, rich_facts.family_semantics,
            "{source}"
        );
        assert_eq!(
            recovery_facts.diagnostics, rich_facts.diagnostics,
            "{source}"
        );
        assert!(recovery_facts.symbols.is_empty(), "{source}");
        assert!(recovery_facts.directive_prefixes.is_empty(), "{source}");
        assert!(recovery_facts.expected_syntax.is_empty(), "{source}");
        recovery
    }

    #[cfg(all(feature = "editor-facts", feature = "diagram-flowchart"))]
    #[test]
    fn recovery_snapshot_retains_flowchart_eof_spans_without_rich_fact_work() {
        for (source, omit_rich_work) in [
            ("flowchart TD\nA[終] --> B\n", true),
            ("flowchart TD\nA[終] -->\n", true),
            (
                "---\r\nconfig:\r\n  flowchart:\r\n    htmlLabels: false\r\n---\r\n%%{init: {theme: 'dark'}}%%\r\nflowchart TD\r\nA[終] -->\r\n",
                false,
            ),
            ("%%{init: {theme: 'dark'}\nflowchart TD\nA-->B\n", false),
        ] {
            let snapshot = assert_recovery_snapshot_matches_rich(source, omit_rich_work);
            if let SemanticParseOutcome::Failed(Error::DiagramParse { diagnostic, .. }) =
                snapshot.outcome()
            {
                let span = diagnostic
                    .span()
                    .expect("Flowchart failure retains a precise span");
                assert!(source.is_char_boundary(span.start));
                assert!(source.is_char_boundary(span.end));
            }
        }
        crate::diagrams::flowchart::reset_flowchart_token_trace_construction_count();
        SemanticParser::new()
            .capture_diagram_recovery_snapshot_controlled_sync(
                "flowchart TD\nA-->B\n",
                &OperationControl::new(),
            )
            .unwrap();
        assert_eq!(
            crate::diagrams::flowchart::flowchart_token_trace_construction_count(),
            1
        );
    }

    #[cfg(all(
        feature = "editor-facts",
        feature = "diagram-sequence",
        feature = "diagram-state",
        feature = "diagram-class"
    ))]
    #[test]
    fn recovery_snapshots_skip_lalrpop_editor_collectors_and_construct_once() {
        for source in [
            "sequenceDiagram\nAlice->>Bob: 終\n",
            "sequenceDiagram\nAlice->>Bob: 終\ninvalid ???\n",
            "stateDiagram-v2\nIdle --> Running: 終\n",
            "stateDiagram-v2\nIdle --> Running: 終\nstate [\n",
            "classDiagram\nclass Widget\nWidget : +run()\n",
            "classDiagram\nclass Widget\n???\n",
        ] {
            assert_recovery_snapshot_matches_rich(source, true);
            assert_eq!(crate::editor::test_work::read().collections, 0, "{source}");
        }
        crate::diagrams::sequence::reset_sequence_syntax_construction_count();
        SemanticParser::new()
            .capture_diagram_recovery_snapshot_controlled_sync(
                "sequenceDiagram\nA->>B: Hi\n",
                &OperationControl::new(),
            )
            .unwrap();
        assert_eq!(
            crate::diagrams::sequence::sequence_syntax_construction_count(),
            1
        );
    }

    #[cfg(all(
        feature = "editor-facts",
        feature = "diagram-architecture",
        feature = "diagram-requirement",
        feature = "diagram-zenuml",
        feature = "diagram-treemap"
    ))]
    #[test]
    fn recovery_snapshots_preserve_partial_parse_and_semantic_validation_diagnostics() {
        for source in [
            "architecture-beta\nservice api\nbroken ?\nservice later\n",
            "architecture-beta\nservice api\nservice api\n",
            "requirementDiagram\naccDescr { unterminated\n",
            "zenuml\n@Starter(A) A.call() ? B.call()",
            "treemap-beta\n\"valid\": 2\ninvalid ?\n\"later\": 3\n",
        ] {
            let snapshot = assert_recovery_snapshot_matches_rich(source, true);
            let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                unreachable!();
            };
            assert!(!facts.diagnostics.is_empty(), "{source}");
            assert!(
                facts.diagnostics.iter().any(|diagnostic| diagnostic.kind
                    == crate::EditorSemanticDiagnosticKind::ParserRecovery),
                "{source}"
            );
        }
    }

    #[cfg(all(feature = "editor-facts", feature = "diagram-flowchart"))]
    #[test]
    fn recovery_snapshot_preserves_preprocessing_evidence_and_cancellation() {
        let parser = SemanticParser::new();
        let source =
            "---\nconfig: [\n---\n%%{initialize: {theme: 'dark'}}%%\nflowchart TD\nA-->B\n";
        let SemanticSnapshotCapture::Failed {
            error: rich_error,
            source_config: rich_config,
        } = parser.capture_diagram_snapshot_sync(source)
        else {
            panic!("invalid frontmatter must fail before the family parse");
        };
        let SemanticSnapshotCapture::Failed {
            error,
            source_config,
        } = parser
            .capture_diagram_recovery_snapshot_controlled_sync(source, &OperationControl::new())
            .unwrap()
        else {
            panic!("recovery capture must retain the preprocessing failure");
        };
        assert_eq!(error.to_string(), rich_error.to_string());
        assert_eq!(source_config, rich_config);

        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let cancelled = parser
            .capture_diagram_recovery_snapshot_controlled_sync("flowchart TD\nA-->B\n", &control)
            .unwrap_err();
        assert_eq!(
            parser
                .capture_diagram_recovery_snapshot_controlled_sync(
                    "flowchart TD\nA-->B\n",
                    &control
                )
                .unwrap_err(),
            cancelled
        );
        assert_recovery_snapshot_matches_rich("flowchart TD\nA-->B\n", true);
    }

    #[cfg(all(feature = "editor-facts", feature = "diagram-flowchart"))]
    #[test]
    fn recovery_snapshot_keeps_real_dropped_span_diagnostics_in_one_construction() {
        let source = "flowchart TD\nA%%{wrap}%%B\nC\n";
        let snapshot = assert_recovery_snapshot_matches_rich(source, false);
        let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            unreachable!();
        };
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("editor fact span(s) that crossed a preprocessing edit")
        }));
        assert!(
            crate::editor::test_work::read().symbols > 0,
            "the edit-spanning input must retain its diagnostic observer"
        );
        crate::diagrams::flowchart::reset_flowchart_token_trace_construction_count();
        SemanticParser::new()
            .capture_diagram_recovery_snapshot_controlled_sync(source, &OperationControl::new())
            .unwrap();
        assert_eq!(
            crate::diagrams::flowchart::flowchart_token_trace_construction_count(),
            1
        );
    }

    #[cfg(feature = "diagram-git-graph")]
    fn semantic_gitgraph_ids(parser: &SemanticParser, known_type: bool) -> Vec<String> {
        let source = "gitGraph\ncommit\ncommit\n";
        let parsed = if known_type {
            parser.parse_diagram_with_type_sync("gitGraph", source)
        } else {
            parser.parse_diagram_sync(source)
        }
        .expect("GitGraph semantic parse");
        let SemanticDiagramModel::GitGraph(model) = parsed.model else {
            panic!("expected GitGraph semantics");
        };
        model.commits.into_iter().map(|commit| commit.id).collect()
    }

    #[cfg(feature = "diagram-git-graph")]
    #[test]
    fn semantic_parser_owns_its_runtime_inside_an_ambient_operation() {
        let parser = SemanticParser::new();
        let expected = semantic_gitgraph_ids(&parser, false);
        let outer = merman_model::runtime::RuntimePolicy::deterministic()
            .with_fixed_seed(1)
            .with_fixed_unix_millis(1_771_113_600_000)
            .begin_operation()
            .unwrap();
        crate::runtime::with_operation_context(&outer, || {
            assert_eq!(semantic_gitgraph_ids(&parser, false), expected);
            assert_eq!(semantic_gitgraph_ids(&parser, true), expected);
            assert_eq!(crate::runtime::today_local(), outer.today_local());
        });
    }

    #[cfg(feature = "diagram-git-graph")]
    #[test]
    fn semantic_runtime_seed_and_captured_context_control_commit_ids() {
        let policy = merman_model::runtime::RuntimePolicy::deterministic().with_fixed_seed(1);
        let first = SemanticParser::new().with_runtime_policy(policy.clone());
        let second = SemanticParser::new().with_runtime_policy(
            merman_model::runtime::RuntimePolicy::deterministic().with_fixed_seed(2),
        );
        let replay = SemanticParser::new().with_operation_context(first.begin_operation().unwrap());
        let expected = semantic_gitgraph_ids(&first, false);
        assert_eq!(first.runtime_policy(), &policy);
        assert_ne!(semantic_gitgraph_ids(&second, false), expected);
        for known_type in [false, true] {
            assert_eq!(semantic_gitgraph_ids(&first, known_type), expected);
            assert_eq!(semantic_gitgraph_ids(&replay, known_type), expected);
        }
        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .with_runtime_policy(policy)
                .parse_diagram_for_render_model_sync(
                    "gitGraph\ncommit\ncommit\n",
                    crate::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::GitGraph(model) = full.model() else {
                panic!("expected full GitGraph model");
            };
            assert_eq!(
                model.commits.iter().map(|c| &c.id).collect::<Vec<_>>(),
                expected.iter().collect::<Vec<_>>()
            );
        }
    }

    #[cfg(feature = "diagram-gantt")]
    #[test]
    fn semantic_runtime_clock_and_timezone_resolve_gantt_dates() {
        let source = "gantt\ndateFormat MM-DD\nMissing year: id1,03-01,1d\nMissing ref: id2,after missing,1d\n";
        for (offset, adjustment) in [(0, 0), (480, -28_800_000)] {
            let policy = merman_model::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_771_113_600_000)
                .try_with_fixed_local_offset_minutes(offset)
                .unwrap();
            let parser = SemanticParser::new().with_runtime_policy(policy);
            for known_type in [false, true] {
                crate::diagrams::gantt::reset_gantt_syntax_construction_count();
                let parsed = if known_type {
                    parser.parse_diagram_with_type_sync("gantt", source)
                } else {
                    parser.parse_diagram_sync(source)
                }
                .unwrap();
                let SemanticDiagramModel::Gantt(model) = parsed.model else {
                    panic!("expected Gantt semantics");
                };
                assert_eq!(model.tasks[0].start_ms, 1_772_323_200_000 + adjustment);
                assert_eq!(model.tasks[1].start_ms, 1_771_113_600_000 + adjustment);
                assert_eq!(crate::diagrams::gantt::gantt_syntax_construction_count(), 1);
            }
        }
    }

    #[cfg(feature = "diagram-gantt")]
    #[test]
    fn semantic_runtime_restores_ambient_context_after_errors_and_cancellation() {
        let policy = merman_model::runtime::RuntimePolicy::deterministic()
            .with_fixed_unix_millis(1_771_113_600_000);
        let parser = SemanticParser::new().with_runtime_policy(policy);
        let outer = merman_model::runtime::RuntimePolicy::deterministic()
            .with_fixed_today(Some(crate::time::CivilDate::new(2030, 1, 2).unwrap()))
            .begin_operation()
            .unwrap();
        let valid = "gantt\ndateFormat MM-DD\nMissing year: id1,03-01,1d\n";
        let invalid = "gantt\nweekday someday\n";
        crate::runtime::with_operation_context(&outer, || {
            for known_type in [false, true] {
                let parse = |input: &str, control: &OperationControl| {
                    if known_type {
                        parser.parse_diagram_with_type_controlled_sync("gantt", input, control)
                    } else {
                        parser.parse_diagram_controlled_sync(input, control)
                    }
                };
                for _ in 0..2 {
                    let parsed = parse(valid, &OperationControl::new()).unwrap().unwrap();
                    let SemanticDiagramModel::Gantt(model) = parsed.model else {
                        panic!("expected Gantt semantics");
                    };
                    assert_eq!(model.tasks[0].start_ms, 1_772_323_200_000);
                    assert_eq!(crate::runtime::today_local(), outer.today_local());
                    let error = parse(invalid, &OperationControl::new())
                        .unwrap()
                        .unwrap_err();
                    let Error::DiagramParse { diagnostic, .. } = error else {
                        panic!("expected Gantt parse diagnostic");
                    };
                    let start = invalid.find("someday").unwrap();
                    assert_eq!(
                        diagnostic.span(),
                        Some(crate::SourceSpan::new(start, start + "someday".len()))
                    );
                    assert_eq!(crate::runtime::today_local(), outer.today_local());
                    let control = OperationControl::new();
                    control.cancel_after_checkpoints(2);
                    assert!(parse(valid, &control).is_err());
                    assert_eq!(crate::runtime::today_local(), outer.today_local());
                }
            }
        });
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn semantic_agentflow_preserves_quoted_comment_lines() {
        let source = "agentflow-beta\nA[\"One\n%% Two\nThree\"]\n";
        let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
        let model = match parsed.model {
            SemanticDiagramModel::Agentflow(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected Agentflow semantics"),
        };
        assert_eq!(
            model.vertices[0].label.as_deref(),
            Some("One\n%% Two\nThree")
        );
        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::Agentflow(full) = full.model() else {
                panic!("expected full Agentflow model");
            };
            assert_eq!(model.vertices[0].label, full.vertices[0].label);
        }
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn semantic_agentflow_preserves_comment_aware_diagnostic_positions() {
        let source = "---\r\ntitle: \"\u{4e2d}\u{6587}\u{1f600}\"\r\n---\r\nagentflow-beta\r\n%% comment\r\na[\"\u{4e2d}\u{6587}\u{1f600}\"]; b@{ shape: cloud }\r\n";
        let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
        let model = match parsed.model {
            SemanticDiagramModel::Agentflow(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected Agentflow semantics"),
        };
        let position = model.diagnostics[0].position.as_ref().unwrap();
        assert_eq!((position.start_line, position.start_column), (6, 11));
        assert_eq!((position.end_line, position.end_column), (6, 28));
        let span = model.warning_facts[0].span.unwrap();
        assert_eq!(&source[span.start..span.end], "b@{ shape: cloud }");
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn semantic_source_positions_map_parse_errors_to_original_input() {
        let source = "---\ntitle: Source\n---\nagentflow-beta\nA ==> B\n";
        let error = SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a structured Agentflow parse error");
        };
        let span = diagnostic.span().expect("exact error span");
        assert_eq!(&source[span.start..span.end], "A ==> B");
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn semantic_source_positions_map_warnings_to_original_input() {
        let source = "\u{feff}---\r\ntitle: Source\r\n---\r\nagentflow-beta\r\n A((Circle))\r\n";
        let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
        let SemanticDiagramModel::Agentflow(model) = parsed.model else {
            panic!("expected Agentflow semantics");
        };
        let warning = &model.warning_facts[0];
        let span = warning.span.expect("warning span");
        assert_eq!(&source[span.start..span.end], "A((Circle))");
        let position = model.diagnostics[0].position.as_ref().unwrap();
        assert_eq!((position.start_line, position.start_column), (5, 1));
    }

    #[cfg(feature = "diagram-timeline")]
    #[test]
    fn semantic_timeline_is_available_without_presentation_or_editor_work() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let parsed = SemanticParser::new()
            .parse_diagram_sync("timeline\nsection Phase\n2026 : Launch\n")
            .expect("Timeline semantics");
        assert_eq!(parsed.metadata.diagram_type, "timeline");
        assert!(matches!(parsed.model, SemanticDiagramModel::Timeline(_)));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
    #[cfg(feature = "diagram-state")]
    #[test]
    fn semantic_state_parses_without_presentation_or_editor_work() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync(
                "stateDiagram",
                "stateDiagram-v2\nstate Outer {\n[*] --> A: ready\n}\nOuter --> Done\n",
            )
            .expect("State semantics are available without presentation");
        assert_eq!(parsed.metadata.diagram_type, "stateDiagram");
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[cfg(any(
        feature = "diagram-cynefin",
        feature = "diagram-flowchart",
        feature = "diagram-info",
        feature = "diagram-pie"
    ))]
    use serde_json::json;
    #[cfg(all(feature = "diagram-flowchart", feature = "presentation"))]
    use serde_json::to_value;

    #[cfg(all(feature = "diagram-cynefin", feature = "diagram-flowchart"))]
    #[test]
    fn semantic_cynefin_parse_matches_full_typed_model() {
        let source =
            "cynefin-beta\ncomplex\n\"Probe\"\nclear\n\"Classify\"\ncomplex --> clear: \"learn\"";
        let parsed = SemanticParser::new()
            .parse_diagram_sync(source)
            .expect("automatic semantic parse");
        assert_eq!(parsed.metadata.diagram_type, "cynefin");
        let SemanticDiagramModel::Cynefin(semantic_model) = parsed.model else {
            panic!("expected Cynefin model");
        };
        assert_eq!(semantic_model.domains.len(), 2);
        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "cynefin",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("Cynefin result");
            let merman_model::presentation::RenderSemanticModel::Cynefin(full_model) = full.model()
            else {
                panic!("expected full Cynefin model");
            };
            assert_eq!(
                serde_json::to_value(semantic_model).unwrap(),
                serde_json::to_value(full_model).unwrap()
            );
        }
    }

    #[cfg(all(
        feature = "diagram-cynefin",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane"),
        not(feature = "diagram-sequence")
    ))]
    #[test]
    fn semantic_cynefin_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("cynefin-beta\ncomplex\n\"Probe\"")
            .expect("Cynefin-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Cynefin(_)));
    }

    #[cfg(feature = "diagram-cynefin")]
    #[test]
    fn semantic_cynefin_parse_does_not_resolve_invalid_visual_theme_values() {
        let input = "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\ncynefin-beta\ncomplex\n\"Probe\"";
        let parsed = SemanticParser::new()
            .parse_diagram_sync(input)
            .expect("semantic parsing does not evaluate theme colors");
        assert!(matches!(parsed.model, SemanticDiagramModel::Cynefin(_)));
        assert_eq!(
            parsed.metadata.semantic_config.as_value()["themeVariables"]["primaryColor"],
            json!("not-a-color")
        );
        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new().parse_diagram_for_render_model_with_type_sync(
                "cynefin",
                input,
                crate::ParseOptions::strict(),
            );
            assert!(
                full.is_ok(),
                "Cynefin typed-model construction should not resolve theme variables: {full:?}"
            );
        }
    }

    #[cfg(feature = "diagram-info")]
    #[test]
    fn semantic_info_matches_full_typed_model_without_editor_or_compat_state() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let source = "info\nshowInfo\ntitle Version\n";
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("info", source)
            .expect("semantic Info parse");
        assert_eq!(parsed.metadata.diagram_type, "info");
        let semantic_model = match parsed.model {
            SemanticDiagramModel::Info(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Info model"),
        };
        assert!(semantic_model.show_info);
        assert_eq!(
            serde_json::to_value(&semantic_model).unwrap(),
            json!({"showInfo": true})
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "info",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("Info result");
            let merman_model::presentation::RenderSemanticModel::Info(full_model) = full.model()
            else {
                panic!("expected full Info model");
            };
            assert_eq!(semantic_model.show_info, full_model.show_info);
        }
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
    }

    #[cfg(all(
        feature = "diagram-info",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane"),
        not(feature = "diagram-sequence"),
        not(feature = "diagram-cynefin")
    ))]
    #[test]
    fn semantic_info_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("info\nshowInfo")
            .expect("Info-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Info(_)));
    }

    #[cfg(feature = "diagram-pie")]
    #[test]
    fn semantic_pie_matches_full_typed_model_without_editor_or_compat_state() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let source = "pie showData\ntitle Revenue\n\"A\": 1\n\"B\": 2\n";
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("pie", source)
            .expect("semantic Pie parse");
        let semantic_model = match parsed.model {
            SemanticDiagramModel::Pie(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Pie model"),
        };
        assert_eq!(
            serde_json::to_value(&semantic_model).unwrap(),
            json!({
                "showData": true,
                "title": "Revenue",
                "accTitle": null,
                "accDescr": null,
                "sections": [
                    {"label": "A", "value": 1.0},
                    {"label": "B", "value": 2.0}
                ]
            })
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "pie",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("Pie result");
            let merman_model::presentation::RenderSemanticModel::Pie(full_model) = full.model()
            else {
                panic!("expected full Pie model");
            };
            assert_eq!(
                serde_json::to_value(&semantic_model).unwrap(),
                json!({
                    "showData": full_model.show_data,
                    "title": full_model.title,
                    "accTitle": full_model.acc_title,
                    "accDescr": full_model.acc_descr,
                    "sections": full_model.sections,
                })
            );
        }
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
    }

    #[cfg(all(
        feature = "diagram-pie",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane"),
        not(feature = "diagram-sequence"),
        not(feature = "diagram-cynefin"),
        not(feature = "diagram-info")
    ))]
    #[test]
    fn semantic_pie_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("pie\n\"A\": 1")
            .expect("Pie-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Pie(_)));
    }

    #[cfg(feature = "diagram-journey")]
    #[test]
    fn semantic_journey_matches_full_typed_model_without_editor_or_compat_state() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let source = concat!(
            "journey\n",
            "title Checkout\n",
            "section Payment\n",
            "Confirm order: 5: Alice, Bob\n",
        );
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("journey", source)
            .expect("semantic Journey parse");
        let semantic_model = match parsed.model {
            SemanticDiagramModel::Journey(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Journey model"),
        };
        assert_eq!(semantic_model.tasks.len(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "journey",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("Journey result");
            let merman_model::presentation::RenderSemanticModel::Journey(full_model) = full.model()
            else {
                panic!("expected full Journey model");
            };
            assert_eq!(
                serde_json::to_value(&semantic_model).unwrap(),
                serde_json::to_value(full_model).unwrap()
            );
        }
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
    }

    #[cfg(all(
        feature = "diagram-journey",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane"),
        not(feature = "diagram-sequence"),
        not(feature = "diagram-cynefin"),
        not(feature = "diagram-info"),
        not(feature = "diagram-pie")
    ))]
    #[test]
    fn semantic_journey_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("journey\nTask: 5")
            .expect("Journey-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Journey(_)));
    }

    #[cfg(all(feature = "diagram-sequence", feature = "diagram-flowchart"))]
    #[test]
    fn semantic_sequence_parse_matches_full_typed_model() {
        let source = "%%{init: {\"wrap\": false, \"sequence\": {\"wrap\": true}}}%%\nsequenceDiagram\naccTitle: Sequence title\nparticipant Alice\nparticipant Bob\nAlice->>Bob: wrap: Ready?\nBob-->>Alice: Yes";
        let semantic = SemanticParser::new()
            .parse_diagram_sync(source)
            .expect("automatic semantic parse");
        assert_eq!(semantic.metadata.diagram_type, "sequence");
        let SemanticDiagramModel::Sequence(semantic_model) = semantic.model else {
            panic!("expected Sequence model");
        };
        let known_type = SemanticParser::new()
            .parse_diagram_with_type_sync("sequence", source)
            .expect("known-type semantic parse");
        let SemanticDiagramModel::Sequence(known_type_model) = known_type.model else {
            panic!("expected known-type Sequence model");
        };
        assert_eq!(
            serde_json::to_value(&semantic_model).unwrap(),
            serde_json::to_value(&known_type_model).unwrap()
        );

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "sequence",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("sequence result");
            let merman_model::presentation::RenderSemanticModel::Sequence(full_model) =
                full.model()
            else {
                panic!("expected full Sequence model");
            };
            assert_eq!(
                serde_json::to_value(&semantic_model).unwrap(),
                serde_json::to_value(full_model).unwrap()
            );
        }
        assert!(!semantic_model.actors["Alice"].wrap);
        assert!(semantic_model.messages[0].wrap);
    }

    #[cfg(all(
        feature = "diagram-sequence",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane")
    ))]
    #[test]
    fn semantic_sequence_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("sequenceDiagram\nAlice->>Bob: Hi")
            .expect("Sequence-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Sequence(_)));
        assert!(matches!(
            SemanticParser::new()
                .parse_diagram_with_type_sync("flowchart-v2", "flowchart TD\nA-->B"),
            Err(Error::UnsupportedDiagram { .. })
        ));
    }

    #[cfg(feature = "diagram-sequence")]
    #[test]
    fn semantic_sequence_parse_skips_visual_theme_evaluation() {
        let source = "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\nsequenceDiagram\nAlice->>Bob: Hi";
        assert!(SemanticParser::new().parse_diagram_sync(source).is_ok());
        #[cfg(feature = "presentation")]
        assert!(matches!(
            crate::MermaidEngine::new().parse_diagram_for_render_model_with_type_sync(
                "sequence",
                source,
                crate::ParseOptions::strict(),
            ),
            Err(crate::Error::ThemeColor(_))
        ));
    }

    #[cfg(feature = "diagram-sequence")]
    #[test]
    fn semantic_sequence_uses_nested_wrap_when_global_wrap_is_not_boolean() {
        let source = "%%{init: {\"wrap\": null, \"sequence\": {\"wrap\": true}}}%%\nsequenceDiagram\nAlice->>Bob: Hi";
        let parsed = SemanticParser::new()
            .parse_diagram_sync(source)
            .expect("semantic parse");
        match parsed.model {
            SemanticDiagramModel::Sequence(model) => assert!(model.actors["Alice"].wrap),
            #[allow(unreachable_patterns)]
            _ => panic!("expected Sequence model"),
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_flowchart_parse_does_not_resolve_invalid_visual_theme_values() {
        let input = "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\nflowchart TD\nA-->B";
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("flowchart-v2", input)
            .expect("semantic parsing does not evaluate theme colors");
        assert!(matches!(parsed.model, SemanticDiagramModel::Flowchart(_)));
        assert_eq!(
            parsed.metadata.config.as_value()["themeVariables"]["primaryColor"],
            json!("not-a-color")
        );
        assert_eq!(
            parsed.metadata.semantic_config.as_value()["themeVariables"]["primaryColor"],
            json!("not-a-color")
        );
        let host_config = MermaidConfig::from_value(json!({
            "theme": "default",
            "themeVariables": {"primaryColor": "not-a-color"}
        }));
        assert!(
            SemanticParser::new()
                .with_site_config(host_config)
                .parse_diagram_with_type_sync("flowchart-v2", "flowchart TD\nA-->B")
                .is_ok()
        );
        #[cfg(feature = "presentation")]
        assert!(matches!(
            crate::MermaidEngine::new().parse_diagram_for_render_model_with_type_sync(
                "flowchart-v2",
                input,
                crate::ParseOptions::strict(),
            ),
            Err(crate::Error::ThemeColor(_))
        ));
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_metadata_separates_upstream_source_projection_from_hardened_config() {
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync(
                "flowchart-v2",
                "%%{init: {themeCSS: '.node { color: red; }'}}%%\nflowchart TD\nA-->B",
            )
            .expect("semantic parse");
        assert_eq!(
            parsed.metadata.config.get_str("themeCSS"),
            Some(".node { color: red; }")
        );
        assert_eq!(
            parsed.metadata.semantic_config.get_str("themeCSS"),
            None,
            "Merman hardening rejects themeCSS from source semantic config"
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_flowchart_uses_secure_site_defaults_and_honors_host_policy() {
        let source =
            "%%{init: {securityLevel: 'loose', maxEdges: 1}}%%\nflowchart TD\nA-->B\nB-->C";
        let hardened = SemanticParser::new()
            .parse_diagram_with_type_sync("flowchart-v2", source)
            .expect("source cannot lower hardened security or edge limits");
        assert_eq!(
            hardened.metadata.semantic_config.get_str("securityLevel"),
            Some("strict")
        );
        assert_ne!(
            hardened.metadata.semantic_config.as_value()["maxEdges"],
            json!(1)
        );

        let host_policy =
            SemanticParser::new().with_site_config(MermaidConfig::from_value(json!({
                "secure": [],
                "maxEdges": 1
            })));
        let error = host_policy
            .parse_diagram_with_type_sync("flowchart-v2", source)
            .expect_err("explicit host policy allows the source maxEdges override");
        assert!(error.to_string().contains("maxEdges"));
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_model_matches_full_parser_flowchart_model() {
        let source = "flowchart LR\nA[Alpha] -->|next| B{Ready?}\nB -->|yes| C[Done]";
        let semantic = SemanticParser::new()
            .parse_diagram_with_type_sync("flowchart-v2", source)
            .expect("semantic parse");
        let semantic_model = match semantic.model {
            SemanticDiagramModel::Flowchart(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Flowchart model"),
        };
        assert_eq!(semantic_model.nodes.len(), 3);
        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "flowchart-v2",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("flowchart result");
            let merman_model::presentation::RenderSemanticModel::Flowchart(full_model) =
                full.model()
            else {
                panic!("expected full Flowchart model");
            };
            assert_eq!(
                to_value(semantic_model).unwrap(),
                to_value(full_model).unwrap()
            );
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_flowchart_applies_source_inherit_direction() {
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync(
                "flowchart-v2",
                "%%{init: {flowchart: {inheritDir: true}}}%%\nflowchart LR\nsubgraph Outer\nsubgraph Inner\nA\nend\nend",
            )
            .expect("semantic parse");
        let model = match parsed.model {
            SemanticDiagramModel::Flowchart(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Flowchart model"),
        };
        let inner = model
            .subgraphs
            .iter()
            .find(|subgraph| subgraph.id == "Inner")
            .expect("nested subgraph");
        assert_eq!(inner.dir.as_deref(), Some("LR"));
        assert!(!inner.has_explicit_dir);
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_flowchart_elk_retains_detector_layout_selection() {
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("flowchart-elk", "flowchart-elk TD\nA-->B")
            .expect("semantic parse");
        assert_eq!(
            parsed.metadata.semantic_config.get_str("layout"),
            Some("elk")
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_parser_detects_flowchart_without_constructing_engine() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("flowchart LR\nA[Start]-->B[Finish]")
            .expect("automatic semantic parse");
        assert_eq!(parsed.metadata.diagram_type, "flowchart-v2");
        assert!(matches!(parsed.model, SemanticDiagramModel::Flowchart(_)));
    }

    #[cfg(feature = "diagram-swimlane")]
    #[test]
    fn semantic_parser_detects_canonical_swimlane_type() {
        for (site, source, layout) in [
            (json!({}), "swimlane-beta LR\nA-->B", json!("swimlane")),
            (
                json!({"layout": "elk"}),
                "swimlane-beta LR\nA-->B",
                json!("elk"),
            ),
            (
                json!({"layout": "elk"}),
                // Frontmatter merging ignores null before appearance selection.
                "---\nconfig:\n  layout: null\n---\nswimlane-beta LR\nA-->B",
                json!("elk"),
            ),
            (
                json!({"layout": "elk"}),
                "---\nconfig:\n  swimlane:\n    layout: dagre\n    theme: forest\n    look: neo\n---\nswimlane-beta LR\nA-->B",
                json!("dagre"),
            ),
        ] {
            let parser =
                SemanticParser::new().with_site_config(MermaidConfig::from_value(site.clone()));
            for known_type in [false, true] {
                let parsed = if known_type {
                    parser.parse_diagram_with_type_sync("swimlane", source)
                } else {
                    parser.parse_diagram_sync(source)
                }
                .expect("semantic Swimlane parse");
                assert_eq!(parsed.metadata.diagram_type, "swimlane");
                assert!(matches!(parsed.model, SemanticDiagramModel::Flowchart(_)));
                #[cfg(feature = "presentation")]
                {
                    let full = crate::MermaidEngine::new()
                        .with_site_config(
                            crate::presentation::SourcePresentationConfig::from_value(site.clone()),
                        )
                        .parse_diagram_for_render_model_with_type_sync(
                            "swimlane",
                            source,
                            crate::ParseOptions::strict(),
                        )
                        .expect("full Swimlane parse")
                        .expect("detected Swimlane");
                    for key in ["layout", "theme", "look"] {
                        assert_eq!(
                            parsed.metadata.semantic_config.as_value().get(key),
                            full.metadata().effective_config.as_value().get(key),
                            "{key} for {source}"
                        );
                    }
                }
                assert_eq!(parsed.metadata.semantic_config.as_value()["layout"], layout);
            }
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_auto_detection_retains_flowchart_elk_layout_mutation() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("flowchart-elk TD\nA-->B")
            .expect("automatic semantic parse");
        assert_eq!(parsed.metadata.diagram_type, "flowchart-elk");
        assert_eq!(
            parsed.metadata.semantic_config.get_str("layout"),
            Some("elk")
        );
    }

    #[cfg(all(feature = "diagram-swimlane", not(feature = "diagram-flowchart")))]
    #[test]
    fn swimlane_feature_enables_the_shared_semantic_flowchart_parser() {
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("swimlane", "swimlane-beta\nA-->B")
            .expect("shared Flowchart parser");
        assert!(matches!(parsed.model, SemanticDiagramModel::Flowchart(_)));
    }

    #[cfg(all(feature = "diagram-swimlane", not(feature = "diagram-flowchart")))]
    #[test]
    fn swimlane_only_feature_does_not_claim_flowchart_types() {
        assert!(matches!(
            SemanticParser::new()
                .parse_diagram_with_type_sync("flowchart-v2", "flowchart TD\nA-->B"),
            Err(Error::UnsupportedDiagram { .. })
        ));
    }

    #[cfg(all(feature = "diagram-flowchart", not(feature = "diagram-swimlane")))]
    #[test]
    fn flowchart_only_feature_does_not_claim_swimlane_type() {
        assert!(matches!(
            SemanticParser::new().parse_diagram_with_type_sync("swimlane", "swimlane-beta\nA-->B"),
            Err(Error::UnsupportedDiagram { .. })
        ));
    }
}
