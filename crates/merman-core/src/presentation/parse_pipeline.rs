use super::{PresentationMetadata, SourcePresentationMetadata};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::EditorSemanticFacts;
use crate::config::presentation::DetectorConfigCheckpoint;
use crate::config::presentation::SourcePresentationConfig;
use crate::config::{AppearanceDecision, MaterializationPlan};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::diagram::{
    CapturedPanic, DiagramParseOutcome, DiagramParseSnapshot, DiagramSnapshotCapture,
    ParsedEditorFacts, RegistryOwner,
};
#[cfg(feature = "compat-json")]
use crate::diagram::{ParsedDiagram, ResolvedSemanticParser};
use crate::operation::OperationPhase;
use crate::preprocess::{
    DirectiveRecoveryMode, PreprocessedSource,
    preprocess_diagram_with_known_type_and_directive_recovery_controlled,
};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::preprocess::{
    SourceConfigEvidence,
    preprocess_diagram_with_known_type_and_directive_recovery_evidence_controlled,
};
use crate::{
    Error, MermaidConfig, MermaidEngine, OperationControl, OperationControlResult, ParseOptions,
    Result, diagrams::error_diagram, family, runtime, sanitize, theme,
};
#[cfg(feature = "compat-json")]
use crate::{ManagedSemanticJson, common_db, diagram};
#[cfg(any(feature = "compat-json", test))]
use merman_model::presentation::CustomJsonRenderModel;
#[cfg(feature = "compat-json")]
use serde::Deserialize;
#[cfg(feature = "compat-json")]
use serde_json::Value;
use {
    crate::diagram::{ParsedDiagramRender, RenderSemanticParseOutput, ResolvedRenderParser},
    merman_model::presentation::RenderSemanticModel,
    merman_model::warnings::DiagramWarningFact,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ParseSource<'a> {
    Detect,
    KnownType(&'a str),
}

#[cfg(feature = "compat-json")]
struct CompatibilitySemanticParse {
    model: ManagedSemanticJson,
    warnings: CompatibilityWarnings,
}

#[cfg(feature = "compat-json")]
enum CompatibilityWarnings {
    Typed(Vec<DiagramWarningFact>),
    BuiltInWithoutWarnings,
    CustomJson,
}

#[cfg(feature = "compat-json")]
impl CompatibilitySemanticParse {
    fn built_in(model: ManagedSemanticJson, warning_facts: Vec<DiagramWarningFact>) -> Self {
        Self {
            model,
            warnings: CompatibilityWarnings::Typed(warning_facts),
        }
    }

    fn built_in_without_warnings(model: Value) -> Self {
        Self {
            model: model.into(),
            warnings: CompatibilityWarnings::BuiltInWithoutWarnings,
        }
    }

    fn custom(model: Value) -> Self {
        Self {
            model: model.into(),
            warnings: CompatibilityWarnings::CustomJson,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParseTiming {
    #[cfg(feature = "compat-json")]
    None,
    #[cfg(feature = "compat-json")]
    Json,
    Render,
}

pub(crate) struct ParsePipeline<'a> {
    engine: &'a MermaidEngine,
    text: &'a str,
    options: ParseOptions,
    source: ParseSource<'a>,
}

/// Owns the configuration through detection, appearance selection, and paint input freezing.
/// Snapshots remain local to the phases that need their values or mutation provenance.
struct OperationConfigBuilder {
    effective: SourcePresentationConfig,
    source: MermaidConfig,
    detector_checkpoint: Option<DetectorConfigCheckpoint>,
}

impl OperationConfigBuilder {
    fn new(
        engine: &MermaidEngine,
        overrides: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self>> {
        let mut effective = match engine.default_effective_config() {
            Ok(config) => config,
            Err(error) => return Ok(Err(error)),
        };
        let source = if overrides.is_empty_object() {
            overrides.clone()
        } else {
            let source = crate::config::source_filtered_overrides(
                effective.as_config(),
                overrides,
                control,
            )?;
            effective.deep_merge_explicit(source.as_value());
            source
        };
        Ok(Ok(Self {
            effective,
            source,
            detector_checkpoint: None,
        }))
    }

    fn finalize(
        self,
        engine: &MermaidEngine,
        diagram_type: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<SourcePresentationConfig>> {
        let Self {
            mut effective,
            source,
            detector_checkpoint,
        } = self;
        let detector_changes = match detector_checkpoint {
            Some(checkpoint) => checkpoint.finish(&mut effective, control)?,
            None => None,
        };
        control.checkpoint()?;
        let defaults = crate::generated::upstream_default_config();
        let decision = AppearanceDecision::resolve(
            diagram_type,
            &source,
            engine.site_config_overrides.as_config(),
            engine
                .theme_compatibility_config
                .as_ref()
                .map(SourcePresentationConfig::as_config),
            &defaults,
        );
        match decision.materialization_plan(
            effective.get_str("theme"),
            engine.site_config.get_str("theme"),
        ) {
            MaterializationPlan::InheritInitialized => {
                decision.apply_to_presentation(&mut effective)
            }
            MaterializationPlan::Materialize { selected } => {
                // Detection must observe initialized config. Keep that result only when its
                // assignments actually need replay onto a newly materialized theme.
                effective =
                    match Self::materialize_operation_theme(engine, &decision, &source, selected) {
                        Ok(config) => config,
                        Err(error) => return Ok(Err(error.into())),
                    };
                if let Some(changes) = &detector_changes {
                    changes.replay(&mut effective, control)?;
                }
                // Detector writes cannot replace authored appearance. Scoped-key presence is
                // checked again because replay can create or remove a scoped setting.
                decision.apply_to_presentation(&mut effective);
            }
        }
        // The explicit keyword is the only detector-level layout selection retained in Mermaid 12.
        if diagram_type == "flowchart-elk" {
            effective.set_value_explicit("layout", serde_json::Value::String("elk".to_owned()));
        }
        if let Some(family) = family::operation_family_id(diagram_type, effective.as_config()) {
            effective.freeze_family_paint_inputs(
                family.as_str(),
                engine.initialization_inputs.as_config(),
                &source,
                detector_changes.as_ref(),
                control,
            )?;
        }
        effective.freeze_theme_compatibility();
        effective.seal_detector_mutations();
        Ok(Ok(effective))
    }

    fn materialize_operation_theme(
        engine: &MermaidEngine,
        decision: &AppearanceDecision<'_>,
        source_config: &MermaidConfig,
        selected: crate::MermaidThemeId,
    ) -> std::result::Result<SourcePresentationConfig, theme::ThemeResolutionError> {
        let mut raw_config = engine.site_config.clone();
        // Every actual theme recalculation consumes raw initialization and allowed source inputs.
        // The inherited path instead keeps ordinary updates to initialized derivations.
        let initial_variables = engine
            .initialization_inputs
            .as_value()
            .get("themeVariables")
            .filter(|value| theme::is_js_truthy(value))
            .map(crate::json_value::clone_value_nonrecursive)
            .unwrap_or_else(|| serde_json::json!({}));
        raw_config.set_value_preserving_theme_compatibility("themeVariables", initial_variables);
        raw_config.deep_merge_explicit(source_config.as_value());
        decision.apply_to_presentation(&mut raw_config);
        theme::materialize_selected_theme(raw_config, selected)
    }
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
struct PreparedPreprocessCapture {
    outcome: PreparedPreprocessOutcome,
    source_config: SourceConfigEvidence,
}

// The ready variant is the parse hot path and is moved exactly once into snapshot construction.
// Boxing it would add one allocation to every parse only to shrink this short-lived control value.
#[allow(clippy::large_enum_variant)]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
enum PreparedPreprocessOutcome {
    Ready(PreprocessedSource, SourcePresentationMetadata),
    Failed(Error),
    Panicked(CapturedPanic),
}

struct ParserSourceMap<'a> {
    source: &'a PreprocessedSource,
}

impl<'a> ParserSourceMap<'a> {
    fn new(source: &'a PreprocessedSource) -> Self {
        Self { source }
    }

    fn parser_input(&self) -> &'a str {
        self.source.text()
    }

    fn remap_parse_error(&self, err: Error) -> Error {
        self.source.remap_parse_error(err)
    }
}

impl<'a> ParsePipeline<'a> {
    pub(crate) fn detect(engine: &'a MermaidEngine, text: &'a str, options: ParseOptions) -> Self {
        Self {
            engine,
            text,
            options,
            source: ParseSource::Detect,
        }
    }

    pub(crate) fn known_type(
        engine: &'a MermaidEngine,
        diagram_type: &'a str,
        text: &'a str,
        options: ParseOptions,
    ) -> Self {
        Self {
            engine,
            text,
            options,
            source: ParseSource::KnownType(diagram_type),
        }
    }

    pub(crate) fn metadata(&self) -> Result<PresentationMetadata> {
        self.with_operation_context(|_| self.metadata_in_context())
    }

    fn metadata_in_context(&self) -> Result<PresentationMetadata> {
        let (_, metadata) =
            self.preprocess_with_directive_recovery(DirectiveRecoveryMode::Strict)?;
        metadata
            .freeze_controlled(&OperationControl::new())
            .map_err(Error::from)?
    }

    #[cfg(feature = "compat-json")]
    pub(crate) fn parse_json(&self, timing: ParseTiming) -> Result<Option<ParsedDiagram>> {
        let operation = OperationControl::new();
        let operation_context = self.engine.begin_operation()?;
        self.parse_model_controlled_in_context(
            timing,
            &operation,
            &operation_context,
            Self::parse_compatibility_semantic_controlled,
            |parsed, config| {
                common_db::apply_common_db_sanitization(parsed.model.as_value_mut(), config);
            },
            |meta, error, control| {
                control.checkpoint()?;
                Ok(Ok(error_diagram::suppressed_error_diagram(meta, error)))
            },
            |meta, parsed, control| {
                control.checkpoint()?;
                Ok(Ok(ParsedDiagram {
                    meta,
                    model: parsed.model,
                }))
            },
            Self::remap_compatibility_semantic_warnings,
            |_| None,
        )
        .map_err(Error::from)?
    }

    #[cfg(feature = "compat-json")]
    fn parse_compatibility_semantic_controlled(
        &self,
        code: &str,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<CompatibilitySemanticParse>> {
        control.checkpoint()?;
        let Some(parser) = self.engine.diagram_registry.resolve(&meta.diagram_type) else {
            return Ok(Err(Error::UnsupportedDiagram {
                diagram_type: meta.diagram_type.clone(),
            }));
        };
        match parser {
            ResolvedSemanticParser::BuiltIn(parser) => Ok(
                if let Some(parser) = family::warning_semantic_parser(&meta.diagram_type) {
                    parser(code, meta).map(|parsed| {
                        let (model, warning_facts) = parsed.into_parts();
                        CompatibilitySemanticParse::built_in(model, warning_facts)
                    })
                } else {
                    parser(code, meta).map(CompatibilitySemanticParse::built_in_without_warnings)
                },
            ),
            ResolvedSemanticParser::Custom(parser) => {
                Ok(parser(code, meta, control)?.map(CompatibilitySemanticParse::custom))
            }
        }
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    pub(crate) fn parse_editor_snapshot(
        &self,
        timing: ParseTiming,
    ) -> Result<Option<DiagramParseSnapshot>> {
        let control = OperationControl::new();
        self.parse_editor_snapshot_controlled(timing, &control)
            .map_err(Error::from)?
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    pub(crate) fn parse_editor_snapshot_controlled(
        &self,
        timing: ParseTiming,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Option<DiagramParseSnapshot>>> {
        Ok(self
            .capture_editor_snapshot_controlled(timing, control)?
            .into_result())
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    pub(crate) fn capture_editor_snapshot_controlled(
        &self,
        timing: ParseTiming,
        control: &OperationControl,
    ) -> OperationControlResult<DiagramSnapshotCapture> {
        control.checkpoint()?;
        let operation_context = match self.engine.begin_operation() {
            Ok(context) => context,
            Err(error) => {
                return Ok(DiagramSnapshotCapture::Failed {
                    error: error.into(),
                    source_config: SourceConfigEvidence::default(),
                });
            }
        };
        runtime::with_operation_context(&operation_context, || {
            self.parse_editor_snapshot_in_context_controlled(timing, &operation_context, control)
        })
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn parse_editor_snapshot_in_context_controlled(
        &self,
        timing: ParseTiming,
        operation_context: &merman_model::runtime::OperationContext,
        control: &OperationControl,
    ) -> OperationControlResult<DiagramSnapshotCapture> {
        control.checkpoint()?;
        let operation_timing = timing.operation_timing(operation_context);
        let total_start = operation_timing.map(merman_model::runtime::OperationTiming::start);
        let preprocess_start = operation_timing.map(merman_model::runtime::OperationTiming::start);
        let preprocessed =
            self.preprocess_with_evidence_controlled(DirectiveRecoveryMode::RecoverLine, control)?;
        let PreparedPreprocessCapture {
            outcome,
            source_config,
        } = preprocessed;
        let (code, meta) = match outcome {
            PreparedPreprocessOutcome::Ready(code, meta) => (code, meta),
            PreparedPreprocessOutcome::Failed(Error::DetectType(_))
                if self.options.suppress_errors =>
            {
                return Ok(DiagramSnapshotCapture::Snapshot(None));
            }
            PreparedPreprocessOutcome::Failed(error) => {
                return Ok(DiagramSnapshotCapture::Failed {
                    error,
                    source_config,
                });
            }
            PreparedPreprocessOutcome::Panicked(panic) => {
                return Ok(DiagramSnapshotCapture::Panicked {
                    panic,
                    source_config,
                });
            }
        };
        control.checkpoint()?;
        let source_map = ParserSourceMap::new(&code);
        let recovered_incomplete_directive = code.recovered_incomplete_directive();
        let editor_input = source_map.parser_input();
        let preprocess = preprocess_start.map(merman_model::runtime::OperationTimer::elapsed);

        let resolved = self.engine.diagram_registry.resolve(&meta.diagram_type);
        let combined = matches!(resolved, Some(ResolvedSemanticParser::BuiltIn(_)))
            .then(|| family::combined_parser(&meta.diagram_type))
            .flatten();

        let parse_start = operation_timing.map(merman_model::runtime::OperationTiming::start);
        let parse_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> OperationControlResult<(
                Result<ManagedSemanticJson>,
                Option<EditorSemanticFacts>,
                Vec<DiagramWarningFact>,
            )> {
                let parsed = match resolved {
                    Some(ResolvedSemanticParser::BuiltIn(parser)) => {
                        if let Some(parser) = combined {
                            control.checkpoint()?;
                            let parsed = parser(editor_input, &meta, control)?;
                            control.checkpoint()?;
                            let (model, editor_facts, warning_facts) = parsed.into_parts();
                            (model, Some(editor_facts), warning_facts)
                        } else {
                            control.checkpoint()?;
                            let model = parser(editor_input, &meta).map(ManagedSemanticJson::from);
                            control.checkpoint()?;
                            (model, None, Vec::new())
                        }
                    }
                    Some(ResolvedSemanticParser::Custom(parser)) => {
                        control.checkpoint()?;
                        let model = parser(editor_input, &meta, control)?.map(ManagedSemanticJson::from);
                        control.checkpoint()?;
                        (model, None, Vec::new())
                    }
                    None => (
                        Err(Error::UnsupportedDiagram {
                            diagram_type: meta.diagram_type.clone(),
                        }),
                        None,
                        Vec::new(),
                    ),
                };
                Ok(parsed)
            },
        ));
        let (model_result, combined_facts, mut warning_facts) = match parse_result {
            Ok(parsed) => parsed?,
            Err(payload) => {
                return Self::finish_editor_snapshot_controlled(
                    meta,
                    DiagramParseOutcome::Panicked(
                        CapturedPanic::from_payload(payload).message().to_string(),
                    ),
                    ParsedEditorFacts::Unavailable,
                    source_config,
                    recovered_incomplete_directive,
                    control,
                );
            }
        };
        let parse = parse_start.map(merman_model::runtime::OperationTimer::elapsed);
        let owner = resolved.map(ResolvedSemanticParser::owner);
        let mut model = match model_result {
            Ok(model) => model,
            Err(err) => {
                let err = source_map.remap_parse_error(err);
                let editor_facts = self.finish_snapshot_editor_facts(
                    owner,
                    combined_facts,
                    &meta,
                    &source_map,
                    control,
                )?;
                return Self::finish_editor_snapshot_controlled(
                    meta,
                    DiagramParseOutcome::Failed(err),
                    editor_facts,
                    source_config,
                    recovered_incomplete_directive,
                    control,
                );
            }
        };

        control.checkpoint()?;
        let sanitize_start = operation_timing.map(merman_model::runtime::OperationTiming::start);
        common_db::apply_common_db_sanitization(
            model.as_value_mut(),
            meta.effective_config.as_config(),
        );
        control.checkpoint()?;
        let sanitize = sanitize_start.map(merman_model::runtime::OperationTimer::elapsed);
        let custom_warning_adapter_succeeded = if matches!(owner, Some(RegistryOwner::Custom)) {
            match Self::decode_custom_warning_facts_controlled(&model, control)? {
                Some(custom_warning_facts) => {
                    warning_facts = custom_warning_facts;
                    true
                }
                None => false,
            }
        } else {
            false
        };
        #[cfg(feature = "diagram-agentflow")]
        if matches!(owner, Some(RegistryOwner::BuiltIn)) {
            let (line_offset, column_offset) = code.parser_position_offset();
            crate::diagrams::agentflow::offset_compatibility_diagnostic_positions(
                model.as_value_mut(),
                line_offset,
                column_offset,
            );
        }
        Self::remap_warning_facts_controlled(&mut warning_facts, &source_map, control)?;
        if matches!(owner, Some(RegistryOwner::BuiltIn)) || custom_warning_adapter_succeeded {
            Self::sync_compatibility_warning_facts(model.as_value_mut(), &warning_facts);
        }
        control.checkpoint()?;
        timing.log_success(ParseTimingSuccess {
            total_start,
            diagram_type: &meta.diagram_type,
            model_kind: None,
            preprocess,
            parse,
            sanitize,
            input_bytes: self.text.len(),
        });

        let editor_facts =
            self.finish_snapshot_editor_facts(owner, combined_facts, &meta, &source_map, control)?;

        Self::finish_editor_snapshot_controlled(
            meta,
            DiagramParseOutcome::Parsed {
                model,
                warning_facts,
            },
            editor_facts,
            source_config,
            recovered_incomplete_directive,
            control,
        )
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn finish_editor_snapshot_controlled(
        meta: SourcePresentationMetadata,
        outcome: DiagramParseOutcome,
        editor_facts: ParsedEditorFacts,
        source_config: SourceConfigEvidence,
        recovered_incomplete_directive: bool,
        control: &OperationControl,
    ) -> OperationControlResult<DiagramSnapshotCapture> {
        let finalization = control.for_phase(OperationPhase::Semantic);
        let meta = match meta.freeze_controlled(&finalization)? {
            Ok(meta) => meta,
            Err(error) => {
                return Ok(DiagramSnapshotCapture::Failed {
                    error,
                    source_config,
                });
            }
        };
        finalization.checkpoint()?;
        Ok(DiagramSnapshotCapture::Snapshot(Some(
            DiagramParseSnapshot::new(
                meta,
                outcome,
                editor_facts,
                source_config,
                recovered_incomplete_directive,
            ),
        )))
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn finish_snapshot_editor_facts(
        &self,
        owner: Option<RegistryOwner>,
        facts: Option<EditorSemanticFacts>,
        meta: &SourcePresentationMetadata,
        source_map: &ParserSourceMap<'_>,
        control: &OperationControl,
    ) -> OperationControlResult<ParsedEditorFacts> {
        control.checkpoint()?;
        let facts = match (owner, facts) {
            (Some(RegistryOwner::Custom), _) | (None, _) => ParsedEditorFacts::Unavailable,
            (Some(RegistryOwner::BuiltIn), Some(facts)) => {
                ParsedEditorFacts::Available(source_map.source.finish_editor_semantic_facts(
                    facts,
                    &meta.diagram_type,
                    control,
                )?)
            }
            (Some(RegistryOwner::BuiltIn), None) => {
                debug_assert!(
                    family::combined_parser(&meta.diagram_type).is_none(),
                    "built-in families with editor capability must provide a combined semantic parser"
                );
                ParsedEditorFacts::Unavailable
            }
        };
        control.checkpoint()?;
        Ok(facts)
    }

    pub(crate) fn parse_render_model(&self) -> Result<Option<ParsedDiagramRender>> {
        self.parse_render_model_controlled(&OperationControl::new())
            .map_err(Error::from)?
    }

    /// Parses a typed render model while observing one caller-owned operation control.
    ///
    /// The existing render parser implementations remain family-owned. This seam checks the
    /// shared control before and after each parser-owned stage, then returns cancellation through
    /// the outer operation result instead of converting it into a Mermaid parse error.
    pub(crate) fn parse_render_model_controlled(
        &self,
        operation: &OperationControl,
    ) -> OperationControlResult<Result<Option<ParsedDiagramRender>>> {
        let operation_context = match self.engine.begin_operation() {
            Ok(context) => context,
            Err(error) => return Ok(Err(error.into())),
        };
        self.parse_render_model_controlled_in_context(operation, &operation_context)
    }

    /// Parses a render model inside a caller-owned runtime context and operation.
    ///
    /// This is the composition seam for higher-level render facades. It deliberately does not
    /// begin another runtime operation, so deterministic time, randomness, and timezone values
    /// remain shared across parsing and every downstream target adapter.
    pub(crate) fn parse_render_model_controlled_in_context(
        &self,
        operation: &OperationControl,
        operation_context: &merman_model::runtime::OperationContext,
    ) -> OperationControlResult<Result<Option<ParsedDiagramRender>>> {
        self.parse_model_controlled_in_context(
            ParseTiming::Render,
            operation,
            operation_context,
            Self::parse_render_semantic_model_controlled,
            |output, config| {
                crate::presentation::diagram::sanitize_common_db_fields(output.model_mut(), config)
            },
            |meta, error, control| {
                control.checkpoint()?;
                error_diagram::suppressed_error_render_diagram_controlled(meta, error, control)
            },
            |meta, output, control| {
                control.checkpoint()?;
                ParsedDiagramRender::from_parse_output_controlled(meta, output, control)
            },
            |output, source_map| {
                let (line_offset, column_offset) = source_map.source.parser_position_offset();
                crate::presentation::diagram::offset_parser_diagnostic_positions(
                    output.model_mut(),
                    line_offset,
                    column_offset,
                );
                crate::presentation::diagram::remap_warning_fact_spans(
                    output.model_mut(),
                    |fact| {
                        Self::remap_warning_fact_spans(fact, source_map);
                    },
                );
            },
            |output| Some(output.model().kind()),
        )
    }

    // These callbacks are the explicit stages of the parse pipeline; bundling them into an
    // untyped options object would obscure their distinct ownership and lifetimes.
    #[allow(clippy::too_many_arguments)]
    fn parse_model_controlled_in_context<T, O>(
        &self,
        timing: ParseTiming,
        operation: &OperationControl,
        operation_context: &merman_model::runtime::OperationContext,
        parse: impl FnOnce(
            &Self,
            &str,
            &SourcePresentationMetadata,
            &OperationControl,
        ) -> OperationControlResult<Result<T>>,
        sanitize: impl FnOnce(&mut T, &MermaidConfig),
        suppressed: impl FnOnce(
            &PresentationMetadata,
            &Error,
            &OperationControl,
        ) -> OperationControlResult<Result<O>>,
        finish: impl FnOnce(
            PresentationMetadata,
            T,
            &OperationControl,
        ) -> OperationControlResult<Result<O>>,
        postprocess: impl FnOnce(&mut T, &ParserSourceMap<'_>),
        model_kind: impl FnOnce(&T) -> Option<&'static str>,
    ) -> OperationControlResult<Result<Option<O>>> {
        operation.checkpoint_at(OperationPhase::Admission)?;
        let control = operation.for_phase(OperationPhase::Parse);
        control.checkpoint_at(OperationPhase::Parse)?;
        runtime::with_operation_context(operation_context, || {
            let operation_timing = timing.operation_timing(operation_context);
            let total_start = operation_timing.map(merman_model::runtime::OperationTiming::start);

            let preprocess_start =
                operation_timing.map(merman_model::runtime::OperationTiming::start);
            let directive_recovery = if self.options.suppress_errors {
                DirectiveRecoveryMode::RecoverLine
            } else {
                DirectiveRecoveryMode::Strict
            };
            let preprocessed = self
                .preprocess_for_with_directive_recovery_controlled(directive_recovery, &control)?;
            let Some((code, meta)) = (match preprocessed {
                Ok(preprocessed) => preprocessed,
                Err(error) => return Ok(Err(error)),
            }) else {
                return Ok(Ok(None));
            };
            let preprocess = preprocess_start.map(merman_model::runtime::OperationTimer::elapsed);
            operation.checkpoint_at(OperationPhase::Semantic)?;
            let source_map = ParserSourceMap::new(&code);

            let parse_start = operation_timing.map(merman_model::runtime::OperationTiming::start);
            let parse_res = parse(self, source_map.parser_input(), &meta, &control);
            let parse = parse_start.map(merman_model::runtime::OperationTimer::elapsed);
            control.checkpoint()?;
            operation.checkpoint_at(OperationPhase::Semantic)?;

            let mut model = match parse_res? {
                Ok(model) => model,
                Err(err) => {
                    if !self.options.suppress_errors {
                        return Ok(Err(source_map.remap_parse_error(err)));
                    }

                    let finalization = operation.for_phase(OperationPhase::Semantic);
                    let meta = match meta.freeze_controlled(&finalization)? {
                        Ok(meta) => meta,
                        Err(error) => return Ok(Err(error)),
                    };
                    let output =
                        match suppressed(&meta, &source_map.remap_parse_error(err), &finalization)?
                        {
                            Ok(output) => output,
                            Err(error) => return Ok(Err(error)),
                        };
                    finalization.checkpoint()?;
                    timing.log_suppressed_error(total_start, preprocess, parse, self.text.len());
                    return Ok(Ok(Some(output)));
                }
            };

            let sanitize_start =
                operation_timing.map(merman_model::runtime::OperationTiming::start);
            sanitize(&mut model, meta.effective_config.as_config());
            let sanitize = sanitize_start.map(merman_model::runtime::OperationTimer::elapsed);
            operation.checkpoint_at(OperationPhase::Semantic)?;
            postprocess(&mut model, &source_map);
            operation.checkpoint_at(OperationPhase::Semantic)?;

            let model_kind = model_kind(&model);
            let finalization = operation.for_phase(OperationPhase::Semantic);
            let meta = match meta.freeze_controlled(&finalization)? {
                Ok(meta) => meta,
                Err(error) => return Ok(Err(error)),
            };
            let timing_diagram_type = total_start.as_ref().map(|_| meta.diagram_type.clone());
            let output = match finish(meta, model, &finalization)? {
                Ok(output) => output,
                Err(error) => return Ok(Err(error)),
            };
            finalization.checkpoint()?;
            timing.log_success(ParseTimingSuccess {
                total_start,
                diagram_type: timing_diagram_type.as_deref().unwrap_or_default(),
                model_kind,
                preprocess,
                parse,
                sanitize,
                input_bytes: self.text.len(),
            });
            Ok(Ok(Some(output)))
        })
    }

    #[cfg(feature = "compat-json")]
    fn remap_compatibility_semantic_warnings(
        parsed: &mut CompatibilitySemanticParse,
        source_map: &ParserSourceMap<'_>,
    ) {
        match &mut parsed.warnings {
            CompatibilityWarnings::Typed(warning_facts) => {
                #[cfg(feature = "diagram-agentflow")]
                {
                    let (line_offset, column_offset) = source_map.source.parser_position_offset();
                    crate::diagrams::agentflow::offset_compatibility_diagnostic_positions(
                        parsed.model.as_value_mut(),
                        line_offset,
                        column_offset,
                    );
                }
                for fact in warning_facts.iter_mut() {
                    Self::remap_warning_fact_spans(fact, source_map);
                }
                Self::sync_compatibility_warning_facts(parsed.model.as_value_mut(), warning_facts);
            }
            CompatibilityWarnings::BuiltInWithoutWarnings => {}
            CompatibilityWarnings::CustomJson => {
                Self::remap_custom_compatibility_json_warning_facts(
                    parsed.model.as_value_mut(),
                    source_map,
                );
            }
        }
    }

    #[cfg(feature = "compat-json")]
    fn remap_custom_compatibility_json_warning_facts(
        model: &mut serde_json::Value,
        source_map: &ParserSourceMap<'_>,
    ) {
        let Some(warning_facts_value) = model.get_mut("warningFacts") else {
            return;
        };
        let Ok(mut warning_facts) = Vec::<DiagramWarningFact>::deserialize(&*warning_facts_value)
        else {
            return;
        };

        for fact in &mut warning_facts {
            Self::remap_warning_fact_spans(fact, source_map);
        }

        crate::config::replace_value_nonrecursive(
            warning_facts_value,
            serde_json::json!(warning_facts),
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn decode_custom_warning_facts_controlled(
        model: &Value,
        control: &OperationControl,
    ) -> OperationControlResult<Option<Vec<DiagramWarningFact>>> {
        let Some(values) = model.get("warningFacts").and_then(Value::as_array) else {
            return Ok(None);
        };

        let mut warning_facts = Vec::with_capacity(values.len());
        for (index, value) in values.iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            let Ok(fact) = DiagramWarningFact::deserialize(value) else {
                return Ok(None);
            };
            warning_facts.push(fact);
        }
        control.checkpoint()?;
        Ok(Some(warning_facts))
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn remap_warning_facts_controlled(
        warning_facts: &mut [DiagramWarningFact],
        source_map: &ParserSourceMap<'_>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        for (index, fact) in warning_facts.iter_mut().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            Self::remap_warning_fact_spans(fact, source_map);
        }
        control.checkpoint()?;
        Ok(())
    }

    #[cfg(feature = "compat-json")]
    fn sync_compatibility_warning_facts(model: &mut Value, warning_facts: &[DiagramWarningFact]) {
        let Some(value) = model.get_mut("warningFacts") else {
            return;
        };
        crate::config::replace_value_nonrecursive(
            value,
            serde_json::to_value(warning_facts)
                .expect("diagram warning facts must remain JSON-serializable"),
        );
    }

    fn remap_warning_fact_spans(fact: &mut DiagramWarningFact, source_map: &ParserSourceMap<'_>) {
        source_map.source.remap_warning_fact_spans(fact);
    }

    fn parse_render_semantic_model_controlled(
        &self,
        code: &str,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<RenderSemanticParseOutput>> {
        control.checkpoint()?;
        #[cfg(feature = "compat-json")]
        let semantic = self.engine.diagram_registry.resolve(&meta.diagram_type);
        let render = self
            .engine
            .render_diagram_registry
            .resolve(&meta.diagram_type);

        if let Some(ResolvedRenderParser::Custom(parser)) = render {
            let parsed = parser(code, meta, control);
            control.checkpoint()?;
            return Ok(parsed?
                .map(RenderSemanticModel::CustomJson)
                .map(RenderSemanticParseOutput::new));
        }

        #[cfg(feature = "compat-json")]
        if let Some(ResolvedSemanticParser::Custom(_)) = semantic {
            return Ok(diagram::parse_or_unsupported_controlled(
                &self.engine.diagram_registry,
                &meta.diagram_type,
                code,
                meta,
                control,
            )?
            .map(|value| {
                RenderSemanticModel::CustomJson(CustomJsonRenderModel::with_provenance(
                    meta.diagram_type.clone(),
                    value,
                    merman_model::presentation::CustomJsonProvenance::SemanticRegistryOverlay,
                ))
            })
            .map(RenderSemanticParseOutput::new));
        }

        if let Some(ResolvedRenderParser::BuiltIn(parser)) = render {
            let parsed = parser(code, meta, control);
            control.checkpoint()?;
            return parsed;
        }

        #[cfg(feature = "compat-json")]
        let builtin_without_render = matches!(semantic, Some(ResolvedSemanticParser::BuiltIn(_)));
        #[cfg(not(feature = "compat-json"))]
        let builtin_without_render = family::is_builtin_diagram_type(&meta.diagram_type);
        if builtin_without_render {
            return Ok(Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                format!(
                    "built-in diagram type `{}` is missing a typed render parser; the custom JSON boundary is reserved for custom registry adapters",
                    meta.diagram_type
                ),
            )));
        }

        Ok(Err(Error::UnsupportedDiagram {
            diagram_type: meta.diagram_type.clone(),
        }))
    }

    fn preprocess_for_with_directive_recovery_controlled(
        &self,
        directive_recovery: DirectiveRecoveryMode,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Option<(PreprocessedSource, SourcePresentationMetadata)>>>
    {
        let preprocessed =
            self.preprocess_with_directive_recovery_controlled(directive_recovery, control)?;
        Ok(match preprocessed {
            Err(Error::DetectType(_)) if self.options.suppress_errors => Ok(None),
            result => result.map(Some),
        })
    }

    fn preprocess_with_directive_recovery(
        &self,
        directive_recovery: DirectiveRecoveryMode,
    ) -> Result<(PreprocessedSource, SourcePresentationMetadata)> {
        let control = OperationControl::new();
        self.preprocess_with_directive_recovery_controlled(directive_recovery, &control)
            .map_err(Error::from)?
    }

    fn preprocess_with_directive_recovery_controlled(
        &self,
        directive_recovery: DirectiveRecoveryMode,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(PreprocessedSource, SourcePresentationMetadata)>> {
        control.checkpoint()?;
        let known_type = match self.source {
            ParseSource::Detect => None,
            ParseSource::KnownType(diagram_type) => Some(diagram_type),
        };
        let pre = match preprocess_diagram_with_known_type_and_directive_recovery_controlled(
            self.text,
            &self.engine.registry,
            known_type,
            directive_recovery,
            control,
        )? {
            Ok(pre) => pre,
            Err(error) => return Ok(Err(error)),
        };
        self.finish_preprocessed_controlled(pre, known_type, control)
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn preprocess_with_evidence_controlled(
        &self,
        directive_recovery: DirectiveRecoveryMode,
        control: &OperationControl,
    ) -> OperationControlResult<PreparedPreprocessCapture> {
        control.checkpoint()?;
        let known_type = match self.source {
            ParseSource::Detect => None,
            ParseSource::KnownType(diagram_type) => Some(diagram_type),
        };
        let captured =
            preprocess_diagram_with_known_type_and_directive_recovery_evidence_controlled(
                self.text,
                &self.engine.registry,
                known_type,
                directive_recovery,
                control,
            )?;
        let outcome = match captured.outcome {
            crate::preprocess::PreprocessCaptureResult::Ready(preprocessed) => {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.finish_preprocessed_controlled(*preprocessed, known_type, control)
                })) {
                    Ok(result) => match result? {
                        Ok((source, metadata)) => {
                            PreparedPreprocessOutcome::Ready(source, metadata)
                        }
                        Err(error) => PreparedPreprocessOutcome::Failed(error),
                    },
                    Err(payload) => {
                        PreparedPreprocessOutcome::Panicked(CapturedPanic::from_payload(payload))
                    }
                }
            }
            crate::preprocess::PreprocessCaptureResult::Failed(error) => {
                PreparedPreprocessOutcome::Failed(error)
            }
            crate::preprocess::PreprocessCaptureResult::Panicked(panic) => {
                PreparedPreprocessOutcome::Panicked(panic)
            }
        };
        Ok(PreparedPreprocessCapture {
            outcome,
            source_config: captured.source_config,
        })
    }

    fn finish_preprocessed_controlled(
        &self,
        pre: crate::PreprocessResult,
        known_type: Option<&str>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(PreprocessedSource, SourcePresentationMetadata)>> {
        control.checkpoint()?;
        if pre.code().trim_start().starts_with("---") {
            return Ok(Err(Error::MalformedFrontMatter));
        }

        let mut config = match OperationConfigBuilder::new(self.engine, &pre.config, control)? {
            Ok(config) => config,
            Err(error) => return Ok(Err(error)),
        };
        let diagram_type = match known_type {
            Some(diagram_type) => diagram_type.to_string(),
            None => {
                config.detector_checkpoint = Some(DetectorConfigCheckpoint::new(&config.effective));
                match self.engine.registry.detect_type_precleaned_controlled(
                    pre.code(),
                    &mut config.effective,
                    control,
                )? {
                    Ok(diagram_type) => diagram_type.to_owned(),
                    Err(error) => return Ok(Err(error)),
                }
            }
        };
        // Public parse metadata follows upstream source authority. Rendering separately retains
        // Merman's hardened secure keys and presentation-value admission.
        let upstream_defaults = crate::generated::upstream_default_config();
        let authored_authority = if self
            .engine
            .initialization_inputs
            .as_value()
            .get("secure")
            .is_some()
        {
            self.engine.initialization_inputs.as_config()
        } else {
            &upstream_defaults
        };
        let source_config =
            crate::config::authored_source_overrides(authored_authority, pre.config, control)?;
        let effective_config = match config.finalize(self.engine, &diagram_type, control)? {
            Ok(config) => config,
            Err(error) => return Ok(Err(error)),
        };

        control.checkpoint()?;
        let title = sanitized_title(pre.title.as_deref(), effective_config.as_config());
        control.checkpoint()?;

        let source = crate::preprocess::select_family_parser_source_controlled(
            pre.source,
            pre.with_comments,
            &diagram_type,
            effective_config.as_config(),
            control,
        )?;

        Ok(Ok((
            source,
            SourcePresentationMetadata {
                diagram_type,
                config: source_config,
                effective_config,
                title,
            },
        )))
    }

    fn with_operation_context<R>(
        &self,
        f: impl FnOnce(&merman_model::runtime::OperationContext) -> Result<R>,
    ) -> Result<R> {
        let context = self.engine.begin_operation()?;
        runtime::with_operation_context(&context, || f(&context))
    }
}

impl ParseTiming {
    fn operation_timing(
        self,
        context: &merman_model::runtime::OperationContext,
    ) -> Option<merman_model::runtime::OperationTiming> {
        match self {
            #[cfg(feature = "compat-json")]
            Self::None => None,
            _ => context.timing(),
        }
    }

    fn log_suppressed_error(
        self,
        total_start: Option<merman_model::runtime::OperationTimer>,
        preprocess: Option<std::time::Duration>,
        parse: Option<std::time::Duration>,
        input_bytes: usize,
    ) {
        let Some(start) = total_start else {
            return;
        };

        match self {
            #[cfg(feature = "compat-json")]
            Self::None => {}
            #[cfg(feature = "compat-json")]
            Self::Json => {
                eprintln!(
                    "[parse-timing] diagram=error total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    start.elapsed(),
                    preprocess.unwrap_or_default(),
                    parse.unwrap_or_default(),
                    std::time::Duration::ZERO,
                    input_bytes,
                );
            }
            Self::Render => {
                eprintln!(
                    "[parse-render-timing] diagram=error model=json total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    start.elapsed(),
                    preprocess.unwrap_or_default(),
                    parse.unwrap_or_default(),
                    std::time::Duration::ZERO,
                    input_bytes,
                );
            }
        }
    }

    fn log_success(self, success: ParseTimingSuccess<'_>) {
        let Some(start) = success.total_start else {
            return;
        };

        match self {
            #[cfg(feature = "compat-json")]
            Self::None => {}
            #[cfg(feature = "compat-json")]
            Self::Json => {
                eprintln!(
                    "[parse-timing] diagram={} total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    success.diagram_type,
                    start.elapsed(),
                    success.preprocess.unwrap_or_default(),
                    success.parse.unwrap_or_default(),
                    success.sanitize.unwrap_or_default(),
                    success.input_bytes,
                );
            }
            Self::Render => {
                eprintln!(
                    "[parse-render-timing] diagram={} model={} total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    success.diagram_type,
                    success.model_kind.unwrap_or("unknown"),
                    start.elapsed(),
                    success.preprocess.unwrap_or_default(),
                    success.parse.unwrap_or_default(),
                    success.sanitize.unwrap_or_default(),
                    success.input_bytes,
                );
            }
        }
    }
}

struct ParseTimingSuccess<'a> {
    total_start: Option<merman_model::runtime::OperationTimer>,
    diagram_type: &'a str,
    model_kind: Option<&'static str>,
    preprocess: Option<std::time::Duration>,
    parse: Option<std::time::Duration>,
    sanitize: Option<std::time::Duration>,
    input_bytes: usize,
}

fn sanitized_title(title: Option<&str>, effective_config: &MermaidConfig) -> Option<String> {
    title
        .map(|title| sanitize::sanitize_text(title, effective_config))
        .filter(|title| !title.is_empty())
}

#[cfg(test)]
mod frozen_finalization_tests {
    use std::cell::Cell;

    use super::*;

    #[cfg(feature = "diagram-info")]
    #[test]
    fn source_defaults_freeze_without_charging_low_model_admission_limits() {
        use merman_model::resources::{
            InputResourceLimitId, InputResourcePolicy, ModelComplexity, ResourceProfile,
        };

        let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelItems, 1)
            .unwrap()
            .with_limit(InputResourceLimitId::MaxModelTextBytes, 1)
            .unwrap()
            .with_limit(InputResourceLimitId::MaxModelNestingDepth, 1)
            .unwrap();
        let engine = MermaidEngine::new();
        let source_defaults = engine.default_effective_config().unwrap();
        let config_complexity = ModelComplexity::from_json(source_defaults.as_value());
        assert!(config_complexity.items > 1);
        assert!(config_complexity.text_bytes > 1);
        assert!(config_complexity.nesting_depth > 1);
        assert!(policy.check_model_complexity(config_complexity).is_err());

        let frozen = SourcePresentationMetadata {
            diagram_type: "info".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: source_defaults.clone(),
            title: None,
        }
        .freeze_controlled(&OperationControl::new())
        .unwrap()
        .unwrap();
        assert_eq!(
            frozen.effective_config.as_value(),
            source_defaults.as_value()
        );

        let full = engine
            .parse_diagram_for_render_model_with_type_sync("info", "info", ParseOptions::strict())
            .unwrap()
            .unwrap();
        let actual =
            merman_model::resources::model_complexity_from_presentation(full.presentation());
        assert_eq!(actual, ModelComplexity::new(1, 0, 0));
        policy.check_model_complexity(actual).unwrap();
        assert_eq!(full.metadata().effective_config, frozen.effective_config);
    }

    #[test]
    fn fallible_finish_is_not_suppressed_as_a_family_parse_error() {
        let engine = MermaidEngine::new();
        let context = engine.begin_operation().unwrap();
        let pipeline = ParsePipeline::known_type(&engine, "info", "info", ParseOptions::lenient());
        let sanitized = Cell::new(false);
        let remapped = Cell::new(false);
        let result = pipeline
            .parse_model_controlled_in_context(
                ParseTiming::Render,
                &OperationControl::new(),
                &context,
                |_, _, _, _| Ok(Ok(())),
                |_, _| sanitized.set(true),
                |_, _, _| -> OperationControlResult<Result<()>> {
                    panic!("finish failure must not become a suppressed diagram")
                },
                |metadata, _, _| {
                    assert!(sanitized.get());
                    assert!(remapped.get());
                    assert_eq!(metadata.diagram_type, "info");
                    assert!(metadata.effective_config.as_value().is_object());
                    Ok(Err(crate::InternalFailure::new(
                        "checked full-data construction failed",
                    )
                    .into()))
                },
                |_, _| remapped.set(true),
                |_| None,
            )
            .unwrap();
        assert!(
            matches!(result, Err(Error::Internal(error)) if error.message() == "checked full-data construction failed")
        );
    }

    #[test]
    fn cancellation_after_finish_stays_on_the_outer_semantic_channel() {
        let engine = MermaidEngine::new();
        let context = engine.begin_operation().unwrap();
        let pipeline = ParsePipeline::known_type(&engine, "info", "info", ParseOptions::lenient());
        let result = pipeline.parse_model_controlled_in_context(
            ParseTiming::Render,
            &OperationControl::new(),
            &context,
            |_, _, _, _| Ok(Ok(())),
            |_, _| {},
            |_, _, _| -> OperationControlResult<Result<()>> {
                panic!("cancellation must not become a suppressed diagram")
            },
            |_, _, control| {
                control.cancel();
                Ok(Ok(()))
            },
            |_, _| {},
            |_| None,
        );
        assert!(matches!(result, Err(cancelled) if cancelled.phase == OperationPhase::Semantic));
    }
}

#[cfg(test)]
mod editor_parse_source_map_tests {
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    use std::time::Duration;

    use super::{CustomJsonRenderModel, SourcePresentationConfig, SourcePresentationMetadata};
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    use super::{ParsePipeline, ParserSourceMap};
    use crate::{
        CancelReason, DetectorRegistry, Error, MermaidEngine, OperationCancelled, OperationControl,
        OperationControlResult, OperationPhase, ParseOptions, Result,
    };
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    use crate::{
        DiagramSnapshotCapture, EditorExpectedSyntax, EditorExpectedSyntaxKind,
        EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, SourceSpan,
    };

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn panicking_detector(_source: &str, _config: &mut SourcePresentationConfig) -> bool {
        panic!("detector fixture panic")
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    static FRONTMATTER_PROBE_CALLS: AtomicUsize = AtomicUsize::new(0);

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn frontmatter_probe_detector(_source: &str, _config: &mut SourcePresentationConfig) -> bool {
        FRONTMATTER_PROBE_CALLS.fetch_add(1, Ordering::Relaxed);
        panic!("frontmatter probe detector must not run")
    }

    fn cancelling_render_parser(
        _code: &str,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<CustomJsonRenderModel>> {
        control.cancel();
        Ok(Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "custom parser failed after requesting cancellation",
        )))
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn expiring_render_parser(
        _code: &str,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<CustomJsonRenderModel>> {
        assert!(
            control.set_deadline(Duration::ZERO),
            "deadline fixture must install a fresh deadline"
        );
        Ok(Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "custom parser failed after expiring its deadline",
        )))
    }

    #[test]
    fn operation_selects_appearance_once_with_or_without_rematerialization() {
        let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
            serde_json::json!({
                "theme": "dark", "themeVariables": { "primaryColor": "#123456" },
            }),
        ));
        for (source, materializations, expected_theme) in [
            ("flowchart TD\nA-->B", 0, "dark"),
            (
                "---\nconfig:\n  theme: base\n---\nflowchart TD\nA-->B",
                1,
                "base",
            ),
            (
                "---\nconfig:\n  theme: 'null'\n---\nflowchart TD\nA-->B",
                0,
                "null",
            ),
        ] {
            let (metadata, work) =
                crate::config::measure_config_work(|| engine.parse_metadata_sync(source).unwrap());
            assert_eq!(
                metadata.effective_config.get_str("theme"),
                Some(expected_theme)
            );
            assert_eq!(work.appearance_selections, 1, "{work:?}");
            assert_eq!(
                work.selected_theme_materializations, materializations,
                "{work:?}"
            );
        }
    }

    #[test]
    fn rematerialization_replays_detector_values_before_reapplying_authored_appearance() {
        fn detector(_source: &str, config: &mut SourcePresentationConfig) -> bool {
            config.set_value("flowchart.nodeSpacing", serde_json::json!(71));
            config.set_value("flowchart.theme", serde_json::json!("forest"));
            config.set_value("themeVariables.primaryColor", serde_json::json!("#abcdef"));
            true
        }
        let mut engine = MermaidEngine::new().with_site_config(
            SourcePresentationConfig::from_value(serde_json::json!({
                "theme": "dark",
            })),
        );
        *engine.registry_mut() = DetectorRegistry::new();
        engine.registry_mut().add_fn("flowchart-v2", detector);
        let (metadata, work) = crate::config::measure_config_work(|| {
            engine
                .parse_metadata_sync("---\nconfig:\n  theme: base\n---\nprobe")
                .unwrap()
        });
        let config = metadata.effective_config;
        assert_eq!(
            config.as_value()["flowchart"]["nodeSpacing"],
            serde_json::json!(71)
        );
        assert_eq!(
            config.get_str("themeVariables.primaryColor"),
            Some("#abcdef")
        );
        assert_eq!(config.get_str("theme"), Some("base"));
        assert_eq!(config.get_str("flowchart.theme"), Some("base"));
        assert!(config.explicit_config_owns_path("flowchart.nodeSpacing"));
        assert!(config.explicit_config_owns_path("themeVariables.primaryColor"));
        assert_eq!(work.appearance_selections, 1, "{work:?}");
        assert_eq!(work.selected_theme_materializations, 1, "{work:?}");
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn controlled_snapshot_stops_before_a_cancelled_operation() {
        let control = OperationControl::new();
        control.cancel();

        let result = MermaidEngine::new()
            .parse_diagram_snapshot_controlled_sync("flowchart TD\nA-->B\n", &control);

        assert!(matches!(result, Err(OperationCancelled { .. })));
    }

    #[test]
    fn custom_render_parse_error_cannot_mask_callback_cancellation() {
        let mut engine = MermaidEngine::new();
        engine
            .render_diagram_registry_mut()
            .insert("flowchart-v2", cancelling_render_parser);
        let control = OperationControl::new();

        let error = engine
            .parse_diagram_for_render_model_with_type_controlled_sync(
                "flowchart-v2",
                "flowchart TD\nA -->\n",
                ParseOptions::lenient(),
                &control,
            )
            .expect_err("callback cancellation must win over a suppressible parse error");

        assert_eq!(error.phase, OperationPhase::Parse);
        assert_eq!(error.reason, CancelReason::Requested);
    }

    #[test]
    fn ordinary_render_parsing_preserves_callback_cancellation() {
        fn cancelling_success(
            _code: &str,
            _meta: &SourcePresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<CustomJsonRenderModel>> {
            control.cancel();
            Ok(Ok(CustomJsonRenderModel::new(
                "cancelled",
                serde_json::json!({}),
            )))
        }

        for parser in [
            cancelling_render_parser as crate::CustomJsonRenderParser,
            cancelling_success,
        ] {
            let mut engine = MermaidEngine::new();
            engine
                .render_diagram_registry_mut()
                .insert("flowchart-v2", parser);
            for options in [ParseOptions::strict(), ParseOptions::lenient()] {
                for known_type in [false, true] {
                    let result = if known_type {
                        engine.parse_diagram_for_render_model_with_type_sync(
                            "flowchart-v2",
                            "flowchart TD\nA --> B",
                            options,
                        )
                    } else {
                        engine.parse_diagram_for_render_model_sync("flowchart TD\nA --> B", options)
                    };
                    let Error::OperationCancelled(error) = result
                        .expect_err("cancellation must never publish a model or error diagram")
                    else {
                        panic!("callback cancellation must remain an operation error");
                    };
                    assert_eq!(error.phase, OperationPhase::Parse);
                    assert_eq!(error.reason, CancelReason::Requested);
                }
            }
        }
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn ordinary_json_parsing_preserves_callback_cancellation() {
        fn cancelling_parser(
            _code: &str,
            meta: &SourcePresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<serde_json::Value>> {
            control.cancel();
            Ok(Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                "cancelled JSON parser",
            )))
        }
        fn cancelling_success(
            _code: &str,
            _meta: &SourcePresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<serde_json::Value>> {
            control.cancel();
            Ok(Ok(serde_json::json!({})))
        }

        for parser in [
            cancelling_parser as crate::DiagramSemanticParser,
            cancelling_success,
        ] {
            let mut engine = MermaidEngine::new();
            engine.diagram_registry_mut().insert("flowchart-v2", parser);
            for options in [ParseOptions::strict(), ParseOptions::lenient()] {
                for known_type in [false, true] {
                    let result = if known_type {
                        engine.parse_diagram_with_type_sync(
                            "flowchart-v2",
                            "flowchart TD\nA --> B",
                            options,
                        )
                    } else {
                        engine.parse_diagram_sync("flowchart TD\nA --> B", options)
                    };
                    let Error::OperationCancelled(error) = result
                        .expect_err("cancellation must never publish JSON or an error diagram")
                    else {
                        panic!("callback cancellation must remain an operation error");
                    };
                    assert_eq!(error.phase, OperationPhase::Parse);
                    assert_eq!(error.reason, CancelReason::Requested);
                }
            }
        }
    }

    #[test]
    fn render_entrypoints_preserve_direct_outer_cancellation() {
        fn cancelled<T>(
            _code: &str,
            _meta: &SourcePresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<T>> {
            assert!(!control.is_cancelled());
            Err(OperationCancelled {
                phase: OperationPhase::Parse,
                reason: CancelReason::Requested,
            })
        }

        let mut render_engine = MermaidEngine::new();
        render_engine
            .render_diagram_registry_mut()
            .insert("flowchart-v2", cancelled::<CustomJsonRenderModel>);
        let engines = [
            (false, render_engine),
            #[cfg(feature = "compat-json")]
            (true, {
                let mut engine = MermaidEngine::new();
                engine
                    .diagram_registry_mut()
                    .insert("flowchart-v2", cancelled::<serde_json::Value>);
                engine
            }),
        ];
        let expected = OperationCancelled {
            phase: OperationPhase::Parse,
            reason: CancelReason::Requested,
        };
        let source = "flowchart TD\nA --> B";
        for (semantic_fallback, engine) in engines {
            for options in [ParseOptions::strict(), ParseOptions::lenient()] {
                for known_type in [false, true] {
                    let ordinary = if known_type {
                        engine.parse_diagram_for_render_model_with_type_sync(
                            "flowchart-v2",
                            source,
                            options,
                        )
                    } else {
                        engine.parse_diagram_for_render_model_sync(source, options)
                    };
                    let Error::OperationCancelled(error) =
                        ordinary.expect_err("outer cancellation must not become an error diagram")
                    else {
                        panic!("outer cancellation must remain an operation error");
                    };
                    assert_eq!(error, expected, "semantic_fallback={semantic_fallback}");

                    let control = OperationControl::new();
                    let controlled = if known_type {
                        engine.parse_diagram_for_render_model_with_type_controlled_sync(
                            "flowchart-v2",
                            source,
                            options,
                            &control,
                        )
                    } else {
                        engine.parse_diagram_for_render_model_controlled_sync(
                            source, options, &control,
                        )
                    };
                    assert_eq!(
                        controlled.expect_err("cancellation must remain in the outer result"),
                        expected,
                        "semantic_fallback={semantic_fallback}"
                    );
                    assert!(!control.is_cancelled());

                    #[cfg(feature = "compat-json")]
                    if semantic_fallback {
                        let json = if known_type {
                            engine.parse_diagram_with_type_sync("flowchart-v2", source, options)
                        } else {
                            engine.parse_diagram_sync(source, options)
                        };
                        let Error::OperationCancelled(error) =
                            json.expect_err("outer cancellation must not publish error JSON")
                        else {
                            panic!("JSON must preserve the operation cancellation");
                        };
                        assert_eq!(error, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn render_entrypoints_use_the_caller_owned_runtime_context() {
        fn observe_context(
            _code: &str,
            _meta: &SourcePresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<CustomJsonRenderModel>> {
            control.checkpoint()?;
            Ok(Ok(CustomJsonRenderModel::new(
                "runtime-context",
                serde_json::json!({
                    "today": crate::runtime::today_local().to_string(),
                    "id": crate::runtime::generated_id_hex("parse-pipeline.context", 0, 16),
                }),
            )))
        }

        let context = merman_model::runtime::RuntimePolicy::deterministic()
            .with_fixed_today(crate::time::CivilDate::new(2030, 1, 2))
            .with_fixed_seed(73)
            .begin_operation()
            .unwrap();
        let mut engine = MermaidEngine::new().with_runtime_policy(
            merman_model::runtime::RuntimePolicy::deterministic()
                .with_fixed_today(crate::time::CivilDate::new(1999, 12, 31))
                .with_fixed_seed(11),
        );
        engine
            .render_diagram_registry_mut()
            .insert("flowchart-v2", observe_context);
        let expected = serde_json::json!({
            "today": context.today_local().to_string(),
            "id": context.derive_hex("parse-pipeline.context", 0, 16),
        });
        let source = "flowchart TD\nA --> B";
        for known_type in [false, true] {
            let control = OperationControl::new();
            let parsed = if known_type {
                engine.parse_diagram_for_render_model_with_type_controlled_in_context_sync(
                    "flowchart-v2",
                    source,
                    ParseOptions::strict(),
                    &control,
                    &context,
                )
            } else {
                engine.parse_diagram_for_render_model_controlled_in_context_sync(
                    source,
                    ParseOptions::strict(),
                    &control,
                    &context,
                )
            }
            .unwrap()
            .unwrap()
            .unwrap();
            let crate::RenderSemanticModel::CustomJson(model) = parsed.model() else {
                panic!("the runtime context observer must produce its custom model");
            };
            assert_eq!(model.value(), &expected);
        }
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[test]
    fn custom_render_parse_error_cannot_mask_callback_deadline() {
        let mut engine = MermaidEngine::new();
        engine
            .render_diagram_registry_mut()
            .insert("flowchart-v2", expiring_render_parser);
        let control = OperationControl::new();

        let error = engine
            .parse_diagram_for_render_model_with_type_controlled_sync(
                "flowchart-v2",
                "flowchart TD\nA -->\n",
                ParseOptions::lenient(),
                &control,
            )
            .expect_err("callback deadline must win over a suppressible parse error");

        assert_eq!(error.phase, OperationPhase::Parse);
        assert_eq!(error.reason, CancelReason::DeadlineExceeded);
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn controlled_capture_cancellation_never_returns_partial_evidence() {
        let control = OperationControl::new();
        control.cancel();

        let result = MermaidEngine::new().capture_diagram_snapshot_controlled_sync(
            "%%{init: {theme: 'dark'}}%%\nflowchart TD\nA-->B\n",
            &control,
        );

        assert!(matches!(result, Err(OperationCancelled { .. })));
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn editor_capture_retains_source_config_on_detection_failure() {
        let source = "%%{ initialize: { theme: 'dark' } }%%\nnot a diagram\n";
        let captured = MermaidEngine::new()
            .capture_diagram_snapshot_controlled_sync(source, &OperationControl::new())
            .expect("an active parse control must not cancel");

        let DiagramSnapshotCapture::Failed {
            error: Error::DetectType(_),
            source_config,
        } = captured
        else {
            panic!("strict detection failure must retain preprocessing evidence");
        };
        assert_eq!(source_config.directives().len(), 1);
        let keyword = source_config.directives()[0].keyword_span();
        assert_eq!(&source[keyword.start..keyword.end], "initialize");
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn editor_capture_retains_source_config_when_init_config_detection_panics() {
        let source = concat!(
            "%%{ initialize: { lazyLoadedDiagrams: true, config: { htmlLabels: false } } }%%\n",
            "detector-panic-fixture\n",
        );
        let mut engine = MermaidEngine::new();
        *engine.registry_mut() = DetectorRegistry::new();
        engine
            .registry_mut()
            .add_fn("detector-panic-fixture", panicking_detector);

        let captured = engine
            .capture_diagram_snapshot_controlled_sync(source, &OperationControl::new())
            .expect("an active parse control must not cancel");

        let DiagramSnapshotCapture::Panicked {
            panic,
            source_config,
        } = captured
        else {
            panic!("detector panic must remain a capture outcome");
        };
        assert!(panic.message().contains("detector fixture panic"));
        assert_eq!(source_config.directives().len(), 1);
        let directive = &source_config.directives()[0];
        assert_eq!(directive.keyword(), "initialize");
        assert!(
            source_config
                .keys()
                .iter()
                .any(|entry| entry.matches_path(&["lazyLoadedDiagrams"]))
        );
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn legacy_snapshot_facade_resumes_the_original_detector_panic_payload() {
        let source = "detector-panic-fixture\n";
        let mut engine = MermaidEngine::new();
        *engine.registry_mut() = DetectorRegistry::new();
        engine
            .registry_mut()
            .add_fn("detector-panic-fixture", panicking_detector);

        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = engine.parse_diagram_snapshot_sync(source);
        }))
        .expect_err("the legacy snapshot facade must resume detector panics");

        assert_eq!(
            payload.downcast_ref::<&'static str>(),
            Some(&"detector fixture panic")
        );
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn editor_capture_retains_later_source_config_on_preprocessing_failure() {
        let source = concat!(
            "---\n",
            "config: [\n",
            "---\n",
            "%%{ initialize: { theme: 'dark' } }%%\n",
            "flowchart TD\nA-->B\n",
        );
        let captured = MermaidEngine::new()
            .capture_diagram_snapshot_controlled_sync(source, &OperationControl::new())
            .expect("an active parse control must not cancel");

        let DiagramSnapshotCapture::Failed {
            error: Error::InvalidFrontMatterYaml { .. },
            source_config,
        } = captured
        else {
            panic!("preprocessing failure must be distinct from cancellation");
        };
        assert_eq!(source_config.directives().len(), 1);
        assert_eq!(source_config.directives()[0].keyword(), "initialize");
        assert!(!source_config.rewrite_safe());
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn editor_capture_preserves_frontmatter_error_before_later_detector_panic() {
        let source = concat!(
            "---\n",
            "config: [\n",
            "---\n",
            "%%{ initialize: { config: { htmlLabels: false } } }%%\n",
            "detector-panic-fixture\n",
        );
        let mut engine = MermaidEngine::new();
        *engine.registry_mut() = DetectorRegistry::new();
        FRONTMATTER_PROBE_CALLS.store(0, Ordering::Relaxed);
        engine
            .registry_mut()
            .add_fn("detector-panic-fixture", frontmatter_probe_detector);

        let captured = engine
            .capture_diagram_snapshot_controlled_sync(source, &OperationControl::new())
            .expect("an active parse control must not cancel");

        let DiagramSnapshotCapture::Failed {
            error: Error::InvalidFrontMatterYaml { .. },
            source_config,
        } = captured
        else {
            panic!("the first preprocessing error must win over later detector work");
        };
        assert_eq!(source_config.directives().len(), 1);
        assert_eq!(source_config.directives()[0].keyword(), "initialize");
        assert!(!source_config.rewrite_safe());
        assert_eq!(FRONTMATTER_PROBE_CALLS.load(Ordering::Relaxed), 0);
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn lenient_editor_capture_preserves_suppressed_detection_semantics() {
        let engine = MermaidEngine::new();
        let pipeline = ParsePipeline::detect(&engine, "not a diagram\n", ParseOptions::lenient());
        let captured = pipeline
            .capture_editor_snapshot_controlled(super::ParseTiming::Json, &OperationControl::new())
            .expect("an active parse control must not cancel");

        assert!(matches!(captured, DiagramSnapshotCapture::Snapshot(None)));
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "diagram-flowchart")]
    #[cfg(feature = "editor-facts")]
    fn controlled_snapshot_stops_during_family_parser_work() {
        let mut source = String::from("flowchart TD\n");
        for index in 0..4_096 {
            source.push_str(&format!("n{index}-->n{}\n", index + 1));
        }
        let control = OperationControl::new();
        control.cancel_after_checkpoints(128);
        crate::diagrams::flowchart::reset_flowchart_token_trace_construction_count();

        let result = MermaidEngine::new().parse_diagram_snapshot_controlled_sync(&source, &control);

        assert!(matches!(result, Err(OperationCancelled { .. })));
        assert_eq!(
            crate::diagrams::flowchart::flowchart_token_trace_construction_count(),
            0,
            "the cancellation schedule must stop in the family-owned accessibility scan"
        );
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "diagram-flowchart")]
    #[cfg(feature = "editor-facts")]
    fn active_control_preserves_the_snapshot_model() {
        let source = "flowchart TD\nA-->B\n";
        let engine = MermaidEngine::new();
        let regular = engine
            .parse_diagram_snapshot_sync(source)
            .expect("regular parse succeeds")
            .expect("regular snapshot");
        let controlled = engine
            .parse_diagram_snapshot_controlled_sync(source, &OperationControl::new())
            .expect("active control")
            .expect("controlled parse succeeds")
            .expect("controlled snapshot");

        assert_eq!(
            controlled.metadata().diagram_type,
            regular.metadata().diagram_type
        );
        assert_eq!(
            controlled.outcome().parsed_model(),
            regular.outcome().parsed_model()
        );
    }

    #[test]
    fn public_and_render_entrypoints_do_not_reprocess_late_frontmatter() {
        let input = concat!(
            "   ---\n",
            "title: only-visible-after-trimming\n",
            "---\n",
            "flowchart TD\n",
            "A-->B\n",
        );
        let engine = MermaidEngine::new();
        let preprocessed = crate::preprocess::preprocess_diagram(input, &engine.registry)
            .expect("single preprocess");
        assert!(preprocessed.code().starts_with("---"));
        assert!(preprocessed.title.is_none());
        assert!(matches!(
            engine.parse_metadata_sync(input),
            Err(Error::MalformedFrontMatter)
        ));
        for options in [ParseOptions::strict(), ParseOptions::lenient()] {
            assert!(matches!(
                engine.parse_diagram_for_render_model_sync(input, options),
                Err(Error::MalformedFrontMatter)
            ));
            assert!(matches!(
                engine.parse_diagram_for_render_model_with_type_sync(
                    "flowchart-v2",
                    input,
                    options
                ),
                Err(Error::MalformedFrontMatter)
            ));
        }
    }

    #[test]
    fn render_rejects_a_second_frontmatter_block() {
        let input = concat!(
            "---\n",
            "title: outer\n",
            "---\n",
            "---\n",
            "title: inner\n",
            "---\n",
            "flowchart TD\n",
            "A-->B\n",
        );
        let engine = MermaidEngine::new();

        let error = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .expect_err("render preprocessing must leave the second block visible");

        assert!(matches!(error, crate::Error::MalformedFrontMatter));
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn fact_remap_drops_only_the_fact_crossing_a_deleted_span() {
        let original = "flowchart TD\nA%%{wrap}%%B\nC\n";
        let engine = MermaidEngine::new();
        let preprocessed =
            crate::preprocess::preprocess_diagram_with_known_type(original, &engine.registry, None)
                .unwrap()
                .source;
        let map = ParserSourceMap::new(&preprocessed);
        let mut facts = EditorSemanticFacts::new();
        let joined_start = preprocessed.text().find("AB").unwrap();
        facts.push_symbol(EditorSemanticSymbol::new(
            "AB",
            None,
            EditorSemanticKind::Variable,
            SourceSpan::new(joined_start, joined_start + 2),
            SourceSpan::new(joined_start, joined_start + 2),
        ));
        let c = preprocessed.text().find('C').unwrap();
        facts.push_symbol(EditorSemanticSymbol::new(
            "C",
            None,
            EditorSemanticKind::Variable,
            SourceSpan::new(c, c + 1),
            SourceSpan::new(c, c + 1),
        ));
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            SourceSpan::new(c, c + 1),
        ));

        map.source
            .remap_editor_facts_controlled(&mut facts, &OperationControl::new())
            .unwrap();

        assert_eq!(facts.symbols.len(), 1);
        assert_eq!(facts.symbols[0].name, "C");
        assert_eq!(
            &original[facts.symbols[0].span.start..facts.symbols[0].span.end],
            "C"
        );
        assert_eq!(facts.expected_syntax.len(), 1);
        assert!(
            facts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("dropped 1 editor fact span"))
        );
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn fact_remap_observes_cancellation_during_large_fact_batches() {
        let original = "flowchart TD\nA\n";
        let engine = MermaidEngine::new();
        let preprocessed =
            crate::preprocess::preprocess_diagram_with_known_type(original, &engine.registry, None)
                .unwrap()
                .source;
        let map = ParserSourceMap::new(&preprocessed);
        let mut facts = EditorSemanticFacts::new();
        let offset = preprocessed.text().find('A').unwrap();
        for index in 0..256 {
            facts.push_symbol(EditorSemanticSymbol::new(
                format!("node-{index}"),
                None,
                EditorSemanticKind::Variable,
                SourceSpan::new(offset, offset + 1),
                SourceSpan::new(offset, offset + 1),
            ));
        }
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);

        assert!(matches!(
            map.source
                .remap_editor_facts_controlled(&mut facts, &control),
            Err(crate::OperationCancelled { .. })
        ));
        assert!(control.is_cancelled());
    }
}
