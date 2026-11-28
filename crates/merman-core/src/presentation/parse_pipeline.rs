#[path = "parse_pipeline/operation_config.rs"]
mod operation_config;

use self::operation_config::OperationConfigBuilder;
use super::{PresentationMetadata, SourcePresentationMetadata};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::EditorSemanticFacts;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::diagram::{
    CapturedPanic, DiagramParseOutcome, DiagramParseSnapshot, DiagramSnapshotCapture,
    ParsedEditorFacts, RegistryOwner,
};
#[cfg(feature = "compat-json")]
use crate::diagram::{ParsedDiagram, ResolvedSemanticParser};
use crate::diagram::{ParsedDiagramRender, RenderSemanticParseOutput, ResolvedRenderParser};
use crate::operation::OperationPhase;
#[cfg(feature = "editor-facts")]
use crate::preprocess::SourceConfigEvidence;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::preprocess::preprocess_diagram_with_known_type_and_directive_recovery_evidence_controlled;
use crate::preprocess::{
    DirectiveRecoveryMode, PreprocessedSource,
    preprocess_diagram_with_known_type_and_directive_recovery_controlled, validate_grid_for_parser,
};
use crate::{
    Error, MermaidConfig, MermaidEngine, OperationControl, OperationControlResult, ParseOptions,
    Result, diagrams::error_diagram, family, runtime, sanitize,
};
#[cfg(feature = "compat-json")]
use crate::{ManagedSemanticJson, common_db, diagram};
#[cfg(feature = "compat-json")]
use merman_model::presentation::CustomJsonRenderModel;
use merman_model::presentation::RenderSemanticModel;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use merman_model::warnings::DiagramWarningFact;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use serde::Deserialize;
#[cfg(feature = "compat-json")]
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ParseSource<'a> {
    Detect,
    KnownType(&'a str),
}

#[cfg(feature = "compat-json")]
enum CompatibilitySemanticParse {
    BuiltIn(family::FamilySemanticModel),
    Custom(ManagedSemanticJson),
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
    allow_missing_diagram: bool,
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
}

impl<'a> ParsePipeline<'a> {
    pub(crate) fn detect(engine: &'a MermaidEngine, text: &'a str, options: ParseOptions) -> Self {
        Self {
            engine,
            text,
            options,
            source: ParseSource::Detect,
            allow_missing_diagram: false,
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
            allow_missing_diagram: false,
        }
    }

    pub(crate) fn allow_missing_diagram(mut self) -> Self {
        self.allow_missing_diagram = true;
        self
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
            |parsed, meta| match parsed {
                CompatibilitySemanticParse::BuiltIn(model) => {
                    if let Some(model) = model.json_mut() {
                        Self::sanitize_compatibility_common_fields(
                            model.as_value_mut(),
                            meta,
                            true,
                        );
                    } else {
                        model.sanitize_typed_common_fields(meta.effective_config.as_config());
                    }
                }
                CompatibilitySemanticParse::Custom(model) => {
                    Self::sanitize_compatibility_common_fields(model.as_value_mut(), meta, false);
                }
            },
            |meta, error, control| {
                control.checkpoint()?;
                Ok(Ok(error_diagram::suppressed_error_diagram(meta, error)))
            },
            |meta, model, control| {
                control.checkpoint()?;
                Ok(Ok(ParsedDiagram { meta, model }))
            },
            Self::project_compatibility_semantic_controlled,
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
        let parsed = match parser {
            ResolvedSemanticParser::BuiltIn(parser) => parser
                .parse_controlled(code, meta, control)?
                .map(CompatibilitySemanticParse::BuiltIn),
            ResolvedSemanticParser::Custom(parser) => parser(code, meta, control)?
                .map(ManagedSemanticJson::from)
                .map(CompatibilitySemanticParse::Custom),
        };
        validate_grid_for_parser(
            parsed,
            meta.effective_config.as_config(),
            &meta.diagram_type,
            matches!(parser, ResolvedSemanticParser::BuiltIn(_)),
            control,
        )
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
                let (model, editor_facts) = match resolved {
                    Some(ResolvedSemanticParser::BuiltIn(parser)) => {
                        if let Some(parser) = combined {
                            control.checkpoint()?;
                            let parsed = parser(editor_input, &meta, control)?;
                            control.checkpoint()?;
                            let (model, editor_facts) = parsed.into_parts();
                            (model, Some(editor_facts))
                        } else {
                            control.checkpoint()?;
                            let model = parser.parse_controlled(editor_input, &meta, control)?;
                            control.checkpoint()?;
                            (model, None)
                        }
                    }
                    Some(ResolvedSemanticParser::Custom(parser)) => {
                        control.checkpoint()?;
                        let model = parser(editor_input, &meta, control)?
                            .map(ManagedSemanticJson::from)
                            .map(family::FamilySemanticModel::Json);
                        control.checkpoint()?;
                        (model, None)
                    }
                    None => (
                        Err(Error::UnsupportedDiagram {
                            diagram_type: meta.diagram_type.clone(),
                        }),
                        None,
                    ),
                };
                let builtin = matches!(resolved, Some(ResolvedSemanticParser::BuiltIn(_)));
                let mut model = match validate_grid_for_parser(model, meta.effective_config.as_config(),
                    &meta.diagram_type, builtin, control)? {
                    Ok(model) => model,
                    Err(error) => return Ok((Err(error), editor_facts, Vec::new())),
                };
                if builtin {
                    let (line_offset, column_offset) = code.parser_position_offset();
                    model.offset_diagnostic_positions(line_offset, column_offset);
                    code.remap_warning_facts_controlled(model.warning_facts_mut(), control)?;
                }
                match model.into_compatibility_json_controlled(&meta.compat_json_metadata(), control)? {
                    Ok((model, warning_facts)) => Ok((Ok(model), editor_facts, warning_facts)),
                    Err(error) => Ok((Err(error), editor_facts, Vec::new())),
                }
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
                let err = source_map
                    .source
                    .remap_parse_error_controlled(err, control)?;
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
        Self::sanitize_compatibility_common_fields(
            model.as_value_mut(),
            &meta,
            matches!(owner, Some(RegistryOwner::BuiltIn)),
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
        if custom_warning_adapter_succeeded {
            code.remap_warning_facts_controlled(&mut warning_facts, control)?;
            Self::write_custom_warning_facts(model.as_value_mut(), &warning_facts);
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

        let mut editor_facts =
            self.finish_snapshot_editor_facts(owner, combined_facts, &meta, &source_map, control)?;
        if let ParsedEditorFacts::Available(facts) = &mut editor_facts {
            warning_facts.append(&mut facts.warning_facts);
        }

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
            |output, meta| {
                crate::presentation::diagram::sanitize_common_db_fields(
                    output.model_mut(),
                    meta.effective_config.as_config(),
                )
            },
            |meta, error, control| {
                control.checkpoint()?;
                error_diagram::suppressed_error_render_diagram_controlled(meta, error, control)
            },
            |meta, output, control| {
                control.checkpoint()?;
                ParsedDiagramRender::from_parse_output_controlled(meta, output, control)
            },
            |mut output, source_map, _meta, control| {
                let (line_offset, column_offset) = source_map.source.parser_position_offset();
                crate::presentation::diagram::offset_parser_diagnostic_positions(
                    output.model_mut(),
                    line_offset,
                    column_offset,
                );
                crate::presentation::diagram::remap_warning_fact_spans_controlled(
                    output.model_mut(),
                    source_map.source,
                    control,
                )?;
                control.checkpoint()?;
                Ok(Ok(output))
            },
            |output| Some(output.model().kind()),
        )
    }

    // These callbacks are the explicit stages of the parse pipeline; bundling them into an
    // untyped options object would obscure their distinct ownership and lifetimes.
    #[allow(clippy::too_many_arguments)]
    fn parse_model_controlled_in_context<T, P, O>(
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
        sanitize: impl FnOnce(&mut T, &SourcePresentationMetadata),
        suppressed: impl FnOnce(
            &PresentationMetadata,
            &Error,
            &OperationControl,
        ) -> OperationControlResult<Result<O>>,
        finish: impl FnOnce(
            PresentationMetadata,
            P,
            &OperationControl,
        ) -> OperationControlResult<Result<O>>,
        postprocess: impl FnOnce(
            T,
            &ParserSourceMap<'_>,
            &SourcePresentationMetadata,
            &OperationControl,
        ) -> OperationControlResult<Result<P>>,
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
            let preprocessed = {
                #[cfg(feature = "benchmark-stages")]
                let _stage = merman_model::benchmark_stages::enter(
                    merman_model::benchmark_stages::Stage::Preprocess,
                );
                self.preprocess_for_with_directive_recovery_controlled(
                    directive_recovery,
                    &control,
                )?
            };
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
            let parse_res = {
                #[cfg(feature = "benchmark-stages")]
                let _stage = merman_model::benchmark_stages::enter(
                    merman_model::benchmark_stages::Stage::Parse,
                );
                parse(self, source_map.parser_input(), &meta, &control)
            };
            let parse = parse_start.map(merman_model::runtime::OperationTimer::elapsed);
            control.checkpoint()?;
            operation.checkpoint_at(OperationPhase::Semantic)?;

            let mut sanitize_duration = None;
            let mut parsed_model_kind = None;
            let model_result = match parse_res? {
                Ok(mut model) => {
                    let sanitize_start =
                        operation_timing.map(merman_model::runtime::OperationTiming::start);
                    sanitize(&mut model, &meta);
                    sanitize_duration =
                        sanitize_start.map(merman_model::runtime::OperationTimer::elapsed);
                    operation.checkpoint_at(OperationPhase::Semantic)?;
                    parsed_model_kind = model_kind(&model);
                    postprocess(model, &source_map, &meta, &control)?
                }
                Err(error) => Err(error),
            };
            operation.checkpoint_at(OperationPhase::Semantic)?;
            let model = match model_result {
                Ok(model) => model,
                Err(err) => {
                    if !self.options.suppress_errors {
                        return Ok(Err(source_map
                            .source
                            .remap_parse_error_controlled(err, &control)?));
                    }

                    let finalization = operation.for_phase(OperationPhase::Semantic);
                    let meta = match meta.freeze_controlled(&finalization)? {
                        Ok(meta) => meta,
                        Err(error) => return Ok(Err(error)),
                    };
                    let err = source_map
                        .source
                        .remap_parse_error_controlled(err, &finalization)?;
                    let output = match suppressed(&meta, &err, &finalization)? {
                        Ok(output) => output,
                        Err(error) => return Ok(Err(error)),
                    };
                    finalization.checkpoint()?;
                    timing.log_suppressed_error(total_start, preprocess, parse, self.text.len());
                    return Ok(Ok(Some(output)));
                }
            };

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
                model_kind: parsed_model_kind,
                preprocess,
                parse,
                sanitize: sanitize_duration,
                input_bytes: self.text.len(),
            });
            Ok(Ok(Some(output)))
        })
    }

    #[cfg(feature = "compat-json")]
    fn sanitize_compatibility_common_fields(
        model: &mut Value,
        meta: &SourcePresentationMetadata,
        builtin: bool,
    ) {
        if builtin
            && matches!(
                meta.diagram_type.as_str(),
                "railroad"
                    | "railroadEbnf"
                    | "railroadAbnf"
                    | "railroadPeg"
                    | "cynefin"
                    | "wardley"
            )
        {
            return;
        }
        common_db::apply_common_db_sanitization(model, meta.effective_config.as_config());
    }

    #[cfg(feature = "compat-json")]
    fn project_compatibility_semantic_controlled(
        parsed: CompatibilitySemanticParse,
        source_map: &ParserSourceMap<'_>,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedSemanticJson>> {
        match parsed {
            CompatibilitySemanticParse::BuiltIn(mut model) => {
                let (line_offset, column_offset) = source_map.source.parser_position_offset();
                model.offset_diagnostic_positions(line_offset, column_offset);
                source_map
                    .source
                    .remap_warning_facts_controlled(model.warning_facts_mut(), control)?;
                Ok(model
                    .into_compatibility_json_controlled(&meta.compat_json_metadata(), control)?
                    .map(|(model, _)| model))
            }
            CompatibilitySemanticParse::Custom(mut model) => {
                crate::presentation::diagram::remap_json_warning_fact_spans_controlled(
                    model.as_value_mut(),
                    source_map.source,
                    control,
                )?;
                control.checkpoint()?;
                Ok(Ok(model))
            }
        }
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
    fn write_custom_warning_facts(model: &mut Value, warning_facts: &[DiagramWarningFact]) {
        let Some(value) = model.get_mut("warningFacts") else {
            return;
        };
        crate::config::replace_value_nonrecursive(
            value,
            serde_json::to_value(warning_facts)
                .expect("diagram warning facts must remain JSON-serializable"),
        );
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
        let config = meta.effective_config.as_config();

        if let Some(ResolvedRenderParser::Custom(parser)) = render {
            let parsed = parser(code, meta, control);
            control.checkpoint()?;
            let parsed = parsed?
                .map(RenderSemanticModel::CustomJson)
                .map(RenderSemanticParseOutput::new);
            return validate_grid_for_parser(parsed, config, &meta.diagram_type, false, control);
        }

        #[cfg(feature = "compat-json")]
        if let Some(ResolvedSemanticParser::Custom(_)) = semantic {
            let parsed = diagram::parse_or_unsupported_controlled(
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
            .map(RenderSemanticParseOutput::new);
            return validate_grid_for_parser(parsed, config, &meta.diagram_type, false, control);
        }

        if let Some(ResolvedRenderParser::BuiltIn(parser)) = render {
            let parsed = parser(code, meta, control);
            control.checkpoint()?;
            return validate_grid_for_parser(parsed?, config, &meta.diagram_type, true, control);
        }

        #[cfg(feature = "compat-json")]
        let builtin_without_render = matches!(semantic, Some(ResolvedSemanticParser::BuiltIn(_)));
        #[cfg(not(feature = "compat-json"))]
        let builtin_without_render = family::canonical_parser(&meta.diagram_type).is_some();
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
            Err(Error::DetectType(_))
                if self.allow_missing_diagram || self.options.suppress_errors =>
            {
                Ok(None)
            }
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
        self.finish_preprocessed_controlled(
            pre,
            known_type,
            #[cfg(feature = "editor-facts")]
            None,
            control,
        )
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
        let mut captured =
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
                    self.finish_preprocessed_controlled(
                        *preprocessed,
                        known_type,
                        Some(&mut captured.source_config),
                        control,
                    )
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
        #[cfg(feature = "editor-facts")] source_evidence: Option<&mut SourceConfigEvidence>,
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
                match self
                    .engine
                    .registry
                    .detect_type_precleaned_with_config_controlled(
                        pre.code(),
                        &mut config,
                        OperationConfigBuilder::effective_for_detection,
                        control,
                    )? {
                    Ok(diagram_type) => diagram_type.to_owned(),
                    Err(error) => return Ok(Err(error)),
                }
            }
        };
        #[cfg(feature = "editor-facts")]
        if let Some(evidence) = source_evidence {
            evidence.freeze_appearance(&diagram_type, config.filtered_source());
        }
        // Public parse metadata follows upstream source authority. Rendering separately retains
        // Merman's hardened secure keys and presentation-value admission.
        let upstream_defaults = crate::generated::upstream_default_config();
        let authored_authority = if self.engine.site_config_overrides.get("secure").is_some() {
            self.engine.site_config_overrides.as_config()
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
            self.text,
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
#[path = "parse_pipeline/editor_parse_source_map_tests.rs"]
mod editor_parse_source_map_tests;
#[cfg(test)]
#[path = "parse_pipeline/frozen_finalization_tests.rs"]
mod frozen_finalization_tests;
