use crate::preprocess::{
    DirectiveRecoveryMode, PreprocessedSource,
    preprocess_diagram_with_known_type_and_directive_recovery_controlled,
    preprocess_mermaid_public_parse_pipeline_with_directive_recovery_controlled,
};
use crate::{
    EditorSemanticFacts, Engine, Error, MermaidConfig, ParseControl, ParseControlResult,
    ParseMetadata, ParseOptions, Result, SourceSpan, common_db, diagram, diagrams::error_diagram,
    family, runtime, sanitize, theme,
};
use crate::{OperationControl, OperationControlResult, OperationPhase};
use diagram::{
    CustomJsonRenderModel, DiagramParseOutcome, DiagramParseSnapshot, DiagramWarningFact,
    ParsedDiagram, ParsedDiagramRender, ParsedEditorFacts, RegistryOwner, RenderSemanticModel,
    RenderSemanticParseOutput, ResolvedRenderParser, ResolvedSemanticParser,
};
use serde_json::Value;

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("panic while analyzing Mermaid source")
        .to_string()
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ParseSource<'a> {
    Detect,
    KnownType(&'a str),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PreprocessPath {
    PublicParse,
    Render,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParseTiming {
    None,
    Json,
    Render,
}

pub(crate) struct ParsePipeline<'a> {
    engine: &'a Engine,
    text: &'a str,
    options: ParseOptions,
    source: ParseSource<'a>,
}

struct EditorParseSourceMap<'a> {
    source: &'a PreprocessedSource,
}

impl<'a> EditorParseSourceMap<'a> {
    fn new(source: &'a PreprocessedSource) -> Self {
        Self { source }
    }

    fn parser_input(&self) -> &'a str {
        self.source.text()
    }

    fn remap_facts(
        &self,
        facts: &mut EditorSemanticFacts,
        control: &ParseControl,
    ) -> ParseControlResult<()> {
        let original_symbol_count = facts.symbols.len();
        let mut remapped_symbols = Vec::with_capacity(original_symbol_count);
        for (index, mut symbol) in std::mem::take(&mut facts.symbols).into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let Some(span) = self.try_remap_source_span(symbol.span) else {
                continue;
            };
            let Some(selection) = self.try_remap_source_span(symbol.selection) else {
                continue;
            };
            symbol.span = span;
            symbol.selection = selection;
            remapped_symbols.push(symbol);
        }
        facts.symbols = remapped_symbols;
        let dropped_lexemes =
            facts.remap_lexemes_controlled(|span| self.try_remap_source_span(span), control)?;

        let mut dropped_diagnostic_spans = 0usize;
        for (index, diagnostic) in facts.diagnostics.iter_mut().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(span) = diagnostic.span {
                diagnostic.span = self.try_remap_source_span(span);
                dropped_diagnostic_spans += usize::from(diagnostic.span.is_none());
            }
        }
        let original_expected_count = facts.expected_syntax.len();
        let mut remapped_expected = Vec::with_capacity(original_expected_count);
        for (index, mut expected) in std::mem::take(&mut facts.expected_syntax)
            .into_iter()
            .enumerate()
        {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(span) = self.try_remap_source_span(expected.span) {
                expected.span = span;
                remapped_expected.push(expected);
            }
        }
        facts.expected_syntax = remapped_expected;

        let dropped_symbols = original_symbol_count - facts.symbols.len();
        let dropped_expected = original_expected_count - facts.expected_syntax.len();
        let dropped_spans =
            dropped_symbols + dropped_lexemes + dropped_expected + dropped_diagnostic_spans;
        if dropped_spans > 0 {
            facts.mark_recovered_with_diagnostic(
                format!(
                    "dropped {dropped_spans} editor fact span(s) that crossed a preprocessing edit"
                ),
                None,
            );
        }
        control.checkpoint()
    }

    fn remap_parse_error(&self, err: Error) -> Error {
        match err {
            Error::DiagramParse {
                diagram_type,
                diagnostic,
            } => Error::DiagramParse {
                diagram_type,
                diagnostic: self.remap_parse_diagnostic(diagnostic),
            },
            err => err,
        }
    }

    fn remap_parse_diagnostic(&self, diagnostic: crate::ParseDiagnostic) -> crate::ParseDiagnostic {
        let Some(span) = diagnostic.span() else {
            return diagnostic;
        };
        match self.try_remap_source_span(span) {
            Some(remapped) => diagnostic.map_span(|_| remapped),
            None => diagnostic.without_span(),
        }
    }

    fn try_remap_source_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        self.source.try_map_span(span)
    }

    fn try_remap_warning_source_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        self.try_remap_source_span(span)
    }
}

impl<'a> ParsePipeline<'a> {
    pub(crate) fn detect(engine: &'a Engine, text: &'a str, options: ParseOptions) -> Self {
        Self {
            engine,
            text,
            options,
            source: ParseSource::Detect,
        }
    }

    pub(crate) fn known_type(
        engine: &'a Engine,
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

    pub(crate) fn metadata(&self) -> Result<ParseMetadata> {
        self.with_operation_context(|_| self.metadata_in_context())
    }

    fn metadata_in_context(&self) -> Result<ParseMetadata> {
        let (_, metadata) = self.preprocess_strict(PreprocessPath::PublicParse)?;
        Ok(metadata)
    }

    pub(crate) fn parse_json(&self, timing: ParseTiming) -> Result<Option<ParsedDiagram>> {
        self.parse_model(
            timing,
            PreprocessPath::PublicParse,
            |pipeline, code, meta| {
                diagram::parse_or_unsupported(
                    &pipeline.engine.diagram_registry,
                    &meta.diagram_type,
                    code,
                    meta,
                )
            },
            common_db::apply_common_db_sanitization,
            error_diagram::suppressed_error_diagram,
            |meta, model| ParsedDiagram { meta, model },
            Self::remap_value_warning_facts,
            |_| None,
        )
    }

    pub(crate) fn parse_editor_snapshot(
        &self,
        timing: ParseTiming,
    ) -> Result<Option<DiagramParseSnapshot>> {
        let control = ParseControl::new();
        self.parse_editor_snapshot_controlled(timing, &control)
            .map_err(Error::from)?
    }

    pub(crate) fn parse_editor_snapshot_controlled(
        &self,
        timing: ParseTiming,
        control: &ParseControl,
    ) -> ParseControlResult<Result<Option<DiagramParseSnapshot>>> {
        control.checkpoint()?;
        let operation_context = match self.engine.begin_operation() {
            Ok(context) => context,
            Err(error) => return Ok(Err(error.into())),
        };
        runtime::with_operation_context(&operation_context, || {
            self.parse_editor_snapshot_in_context_controlled(timing, &operation_context, control)
        })
    }

    fn parse_editor_snapshot_in_context_controlled(
        &self,
        timing: ParseTiming,
        operation_context: &runtime::OperationContext,
        control: &ParseControl,
    ) -> ParseControlResult<Result<Option<DiagramParseSnapshot>>> {
        control.checkpoint()?;
        let operation_timing = timing.operation_timing(operation_context);
        let total_start = operation_timing.map(runtime::OperationTiming::start);
        let preprocess_start = operation_timing.map(runtime::OperationTiming::start);
        let preprocessed = self.preprocess_for_with_directive_recovery_controlled(
            PreprocessPath::PublicParse,
            DirectiveRecoveryMode::RecoverLine,
            control,
        )?;
        let Some((code, meta)) = (match preprocessed {
            Ok(preprocessed) => preprocessed,
            Err(error) => return Ok(Err(error)),
        }) else {
            return Ok(Ok(None));
        };
        control.checkpoint()?;
        let source_map = EditorParseSourceMap::new(&code);
        let recovered_incomplete_directive = code.recovered_incomplete_directive();
        let editor_input = source_map.parser_input();
        let preprocess = preprocess_start.map(runtime::OperationTimer::elapsed);

        let resolved = self.engine.diagram_registry.resolve(&meta.diagram_type);
        let combined = matches!(resolved, Some(ResolvedSemanticParser::BuiltIn(_)))
            .then(|| family::combined_parser(&meta.diagram_type))
            .flatten();

        let parse_start = operation_timing.map(runtime::OperationTiming::start);
        let parse_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> ParseControlResult<(Result<Value>, Option<EditorSemanticFacts>)> {
                let parsed = match resolved {
                    Some(ResolvedSemanticParser::BuiltIn(parser)) => {
                        if let Some(parser) = combined {
                            control.checkpoint()?;
                            let parsed = parser(editor_input, &meta, control)?;
                            control.checkpoint()?;
                            let (model, editor_facts) = parsed.into_parts();
                            (model, Some(editor_facts))
                        } else {
                            control.checkpoint()?;
                            let model = parser(editor_input, &meta);
                            control.checkpoint()?;
                            (model, None)
                        }
                    }
                    Some(ResolvedSemanticParser::Custom(parser)) => {
                        control.checkpoint()?;
                        let model = parser(editor_input, &meta, control)?;
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
                Ok(parsed)
            },
        ));
        let (model_result, combined_facts) = match parse_result {
            Ok(parsed) => parsed?,
            Err(payload) => {
                return Ok(Ok(Some(DiagramParseSnapshot::new(
                    meta,
                    DiagramParseOutcome::Panicked(panic_payload_message(payload.as_ref())),
                    ParsedEditorFacts::Unavailable,
                    recovered_incomplete_directive,
                ))));
            }
        };
        let parse = parse_start.map(runtime::OperationTimer::elapsed);
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
                return Ok(Ok(Some(DiagramParseSnapshot::new(
                    meta,
                    DiagramParseOutcome::Failed(err),
                    editor_facts,
                    recovered_incomplete_directive,
                ))));
            }
        };

        control.checkpoint()?;
        let sanitize_start = operation_timing.map(runtime::OperationTiming::start);
        common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
        control.checkpoint()?;
        let sanitize = sanitize_start.map(runtime::OperationTimer::elapsed);
        Self::remap_value_warning_facts(&mut model, &source_map);
        control.checkpoint()?;
        timing.log_success(ParseTimingSuccess {
            total_start,
            meta: &meta,
            model_kind: None,
            preprocess,
            parse,
            sanitize,
            input_bytes: self.text.len(),
        });

        let editor_facts =
            self.finish_snapshot_editor_facts(owner, combined_facts, &meta, &source_map, control)?;

        Ok(Ok(Some(DiagramParseSnapshot::new(
            meta,
            DiagramParseOutcome::Parsed(model),
            editor_facts,
            recovered_incomplete_directive,
        ))))
    }

    fn finish_snapshot_editor_facts(
        &self,
        owner: Option<RegistryOwner>,
        facts: Option<EditorSemanticFacts>,
        meta: &ParseMetadata,
        source_map: &EditorParseSourceMap<'_>,
        control: &ParseControl,
    ) -> ParseControlResult<ParsedEditorFacts> {
        control.checkpoint()?;
        let facts = match (owner, facts) {
            (Some(RegistryOwner::Custom), _) | (None, _) => ParsedEditorFacts::Unavailable,
            (Some(RegistryOwner::BuiltIn), Some(facts)) => ParsedEditorFacts::Available(
                self.finish_editor_semantic_facts(facts, meta, source_map, control)?,
            ),
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
        self.parse_model(
            ParseTiming::Render,
            PreprocessPath::Render,
            Self::parse_render_semantic_model,
            |output, config| output.model_mut().sanitize_common_db_fields(config),
            error_diagram::suppressed_error_render_diagram,
            ParsedDiagramRender::from_parse_output,
            |output, source_map| {
                output.model_mut().remap_warning_fact_spans(|fact| {
                    Self::remap_warning_fact_spans(fact, source_map);
                });
            },
            |output| Some(output.model().kind()),
        )
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
        operation_context: &runtime::OperationContext,
    ) -> OperationControlResult<Result<Option<ParsedDiagramRender>>> {
        operation.checkpoint_at(OperationPhase::Admission)?;
        let control = ParseControl::from_operation_control(operation.clone());
        control.checkpoint_operation(OperationPhase::Parse)?;
        runtime::with_operation_context(operation_context, || {
            operation.checkpoint_at(OperationPhase::Parse)?;
            let directive_recovery = if self.options.suppress_errors {
                DirectiveRecoveryMode::RecoverLine
            } else {
                DirectiveRecoveryMode::Strict
            };
            let preprocessed = control.map_cancellation(
                self.preprocess_for_with_directive_recovery_controlled(
                    PreprocessPath::Render,
                    directive_recovery,
                    &control,
                ),
                OperationPhase::Parse,
            )?;
            let Some((code, meta)) = (match preprocessed {
                Ok(preprocessed) => preprocessed,
                Err(error) => return Ok(Err(error)),
            }) else {
                return Ok(Ok(None));
            };
            operation.checkpoint_at(OperationPhase::Semantic)?;
            let source_map = EditorParseSourceMap::new(&code);
            let parsed = control.map_cancellation(
                self.parse_render_semantic_model_controlled(
                    source_map.parser_input(),
                    &meta,
                    &control,
                ),
                OperationPhase::Semantic,
            )?;
            let mut output = match parsed {
                Ok(output) => output,
                Err(error) => {
                    if !self.options.suppress_errors {
                        return Ok(Err(source_map.remap_parse_error(error)));
                    }
                    return Ok(Ok(Some(error_diagram::suppressed_error_render_diagram(
                        &meta,
                    ))));
                }
            };
            operation.checkpoint_at(OperationPhase::Semantic)?;
            output
                .model_mut()
                .sanitize_common_db_fields(&meta.effective_config);
            operation.checkpoint_at(OperationPhase::Semantic)?;
            output.model_mut().remap_warning_fact_spans(|fact| {
                Self::remap_warning_fact_spans(fact, &source_map);
            });
            operation.checkpoint_at(OperationPhase::Semantic)?;
            Ok(Ok(Some(ParsedDiagramRender::from_parse_output(
                meta, output,
            ))))
        })
    }

    fn finish_editor_semantic_facts(
        &self,
        mut facts: EditorSemanticFacts,
        meta: &ParseMetadata,
        source_map: &EditorParseSourceMap<'_>,
        control: &ParseControl,
    ) -> ParseControlResult<EditorSemanticFacts> {
        let family_directive_prefixes = std::mem::take(&mut facts.directive_prefixes);
        source_map.remap_facts(&mut facts, control)?;
        let family = family::diagram_type_family_id(&meta.diagram_type)
            .expect("built-in combined semantic facts belong to a catalog family");
        facts.finalize_lexemes_controlled(family, source_map.source.global_lexemes(), control)?;
        for (index, prefix) in source_map
            .source
            .global_directive_prefixes()
            .iter()
            .enumerate()
        {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            facts.push_directive_prefix(prefix.clone());
        }
        for (index, prefix) in family_directive_prefixes.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            facts.push_directive_prefix(prefix);
        }
        control.checkpoint()?;
        Ok(facts)
    }

    // These callbacks are the explicit stages of the parse pipeline; bundling them into an
    // untyped options object would obscure their distinct ownership and lifetimes.
    #[allow(clippy::too_many_arguments)]
    fn parse_model<T, O>(
        &self,
        timing: ParseTiming,
        preprocess_path: PreprocessPath,
        parse: impl FnOnce(&Self, &str, &ParseMetadata) -> Result<T>,
        sanitize: impl FnOnce(&mut T, &MermaidConfig),
        suppressed: impl FnOnce(&ParseMetadata) -> O,
        finish: impl FnOnce(ParseMetadata, T) -> O,
        postprocess: impl FnOnce(&mut T, &EditorParseSourceMap<'_>),
        model_kind: impl FnOnce(&T) -> Option<&'static str>,
    ) -> Result<Option<O>> {
        self.with_operation_context(|operation_context| {
            self.parse_model_in_context(
                timing,
                preprocess_path,
                operation_context,
                parse,
                sanitize,
                suppressed,
                finish,
                postprocess,
                model_kind,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_model_in_context<T, O>(
        &self,
        timing: ParseTiming,
        preprocess_path: PreprocessPath,
        operation_context: &runtime::OperationContext,
        parse: impl FnOnce(&Self, &str, &ParseMetadata) -> Result<T>,
        sanitize: impl FnOnce(&mut T, &MermaidConfig),
        suppressed: impl FnOnce(&ParseMetadata) -> O,
        finish: impl FnOnce(ParseMetadata, T) -> O,
        postprocess: impl FnOnce(&mut T, &EditorParseSourceMap<'_>),
        model_kind: impl FnOnce(&T) -> Option<&'static str>,
    ) -> Result<Option<O>> {
        let operation_timing = timing.operation_timing(operation_context);
        let total_start = operation_timing.map(runtime::OperationTiming::start);

        let preprocess_start = operation_timing.map(runtime::OperationTiming::start);
        let Some((code, meta)) = self.preprocess_for(preprocess_path)? else {
            return Ok(None);
        };
        let source_map = EditorParseSourceMap::new(&code);
        let preprocess = preprocess_start.map(runtime::OperationTimer::elapsed);

        let parse_start = operation_timing.map(runtime::OperationTiming::start);
        let parse_res = parse(self, source_map.parser_input(), &meta);
        let parse = parse_start.map(runtime::OperationTimer::elapsed);

        let mut model = match parse_res {
            Ok(model) => model,
            Err(err) => {
                if !self.options.suppress_errors {
                    return Err(source_map.remap_parse_error(err));
                }

                timing.log_suppressed_error(total_start, preprocess, parse, self.text.len());
                return Ok(Some(suppressed(&meta)));
            }
        };

        let sanitize_start = operation_timing.map(runtime::OperationTiming::start);
        sanitize(&mut model, &meta.effective_config);
        let sanitize = sanitize_start.map(runtime::OperationTimer::elapsed);
        postprocess(&mut model, &source_map);

        timing.log_success(ParseTimingSuccess {
            total_start,
            meta: &meta,
            model_kind: model_kind(&model),
            preprocess,
            parse,
            sanitize,
            input_bytes: self.text.len(),
        });

        Ok(Some(finish(meta, model)))
    }

    fn remap_value_warning_facts(
        model: &mut serde_json::Value,
        source_map: &EditorParseSourceMap<'_>,
    ) {
        let Some(warning_facts_value) = model.get_mut("warningFacts") else {
            return;
        };
        let Ok(mut warning_facts) =
            serde_json::from_value::<Vec<DiagramWarningFact>>(warning_facts_value.clone())
        else {
            return;
        };

        for fact in &mut warning_facts {
            Self::remap_warning_fact_spans(fact, source_map);
        }

        *warning_facts_value = serde_json::json!(warning_facts);
    }

    fn remap_warning_fact_spans(
        fact: &mut DiagramWarningFact,
        source_map: &EditorParseSourceMap<'_>,
    ) {
        let source_span = fact.span;
        let remapped_span =
            source_span.and_then(|span| source_map.try_remap_warning_source_span(span));
        fact.span = remapped_span;
        fact.fix_span = match (fact.fix_span, source_span, remapped_span) {
            (Some(fix_span), Some(source_span), Some(remapped_span))
                if fix_span.start == fix_span.end && fix_span.start == source_span.end =>
            {
                Some(SourceSpan::new(remapped_span.end, remapped_span.end))
            }
            (Some(fix_span), _, _) => source_map.try_remap_warning_source_span(fix_span),
            (None, _, _) => None,
        };
    }

    fn parse_render_semantic_model(
        &self,
        code: &str,
        meta: &ParseMetadata,
    ) -> Result<RenderSemanticParseOutput> {
        let control = ParseControl::new();
        self.parse_render_semantic_model_controlled(code, meta, &control)
            .map_err(Error::from)?
    }

    fn parse_render_semantic_model_controlled(
        &self,
        code: &str,
        meta: &ParseMetadata,
        control: &ParseControl,
    ) -> ParseControlResult<Result<RenderSemanticParseOutput>> {
        control.checkpoint()?;
        let semantic = self.engine.diagram_registry.resolve(&meta.diagram_type);
        let render = self
            .engine
            .render_diagram_registry
            .resolve(&meta.diagram_type);

        if let Some(ResolvedRenderParser::Custom(parser)) = render {
            return Ok(parser(code, meta, control)?
                .map(RenderSemanticModel::CustomJson)
                .map(RenderSemanticParseOutput::new));
        }

        if let Some(ResolvedSemanticParser::Custom(_)) = semantic {
            return Ok(diagram::parse_or_unsupported_controlled(
                &self.engine.diagram_registry,
                &meta.diagram_type,
                code,
                meta,
                control,
            )?
            .map(|value| {
                RenderSemanticModel::CustomJson(CustomJsonRenderModel::from_semantic_registry(
                    meta.diagram_type.clone(),
                    value,
                ))
            })
            .map(RenderSemanticParseOutput::new));
        }

        if let Some(ResolvedRenderParser::BuiltIn(parser)) = render {
            return parser(code, meta, control);
        }

        if let Some(ResolvedSemanticParser::BuiltIn(_)) = semantic {
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

    fn preprocess_for(
        &self,
        path: PreprocessPath,
    ) -> Result<Option<(PreprocessedSource, ParseMetadata)>> {
        let directive_recovery = if self.options.suppress_errors {
            DirectiveRecoveryMode::RecoverLine
        } else {
            DirectiveRecoveryMode::Strict
        };
        self.preprocess_for_with_directive_recovery(path, directive_recovery)
    }

    fn preprocess_for_with_directive_recovery(
        &self,
        path: PreprocessPath,
        directive_recovery: DirectiveRecoveryMode,
    ) -> Result<Option<(PreprocessedSource, ParseMetadata)>> {
        let control = ParseControl::new();
        self.preprocess_for_with_directive_recovery_controlled(path, directive_recovery, &control)
            .expect("a private parse control cannot be cancelled")
    }

    fn preprocess_for_with_directive_recovery_controlled(
        &self,
        path: PreprocessPath,
        directive_recovery: DirectiveRecoveryMode,
        control: &ParseControl,
    ) -> ParseControlResult<Result<Option<(PreprocessedSource, ParseMetadata)>>> {
        let preprocessed =
            self.preprocess_with_directive_recovery_controlled(path, directive_recovery, control)?;
        Ok(match preprocessed {
            Err(Error::DetectType(_)) if self.options.suppress_errors => Ok(None),
            result => result.map(Some),
        })
    }

    fn preprocess_strict(
        &self,
        path: PreprocessPath,
    ) -> Result<(PreprocessedSource, ParseMetadata)> {
        self.preprocess_with_directive_recovery(path, DirectiveRecoveryMode::Strict)
    }

    fn preprocess_with_directive_recovery(
        &self,
        path: PreprocessPath,
        directive_recovery: DirectiveRecoveryMode,
    ) -> Result<(PreprocessedSource, ParseMetadata)> {
        let control = ParseControl::new();
        self.preprocess_with_directive_recovery_controlled(path, directive_recovery, &control)
            .expect("a private parse control cannot be cancelled")
    }

    fn preprocess_with_directive_recovery_controlled(
        &self,
        path: PreprocessPath,
        directive_recovery: DirectiveRecoveryMode,
        control: &ParseControl,
    ) -> ParseControlResult<Result<(PreprocessedSource, ParseMetadata)>> {
        control.checkpoint()?;
        match self.source {
            ParseSource::Detect => {
                self.preprocess_and_detect_strict_controlled(path, directive_recovery, control)
            }
            ParseSource::KnownType(diagram_type) => self.preprocess_and_assume_type_controlled(
                diagram_type,
                path,
                directive_recovery,
                control,
            ),
        }
    }

    fn preprocess_and_detect_strict_controlled(
        &self,
        path: PreprocessPath,
        directive_recovery: DirectiveRecoveryMode,
        control: &ParseControl,
    ) -> ParseControlResult<Result<(PreprocessedSource, ParseMetadata)>> {
        let pre = match self.preprocess_input_with_directive_recovery_controlled(
            path,
            None,
            directive_recovery,
            control,
        )? {
            Ok(pre) => pre,
            Err(error) => return Ok(Err(error)),
        };
        control.checkpoint()?;
        if pre.code().trim_start().starts_with("---") {
            return Ok(Err(Error::MalformedFrontMatter));
        }

        let has_config_overrides = !pre.config.is_empty_object();
        let mut effective_config = self.effective_config_before_detect(&pre.config);
        let cached_effective_config = (!has_config_overrides).then(|| effective_config.clone());

        let diagram_type = match self.engine.registry.detect_type_precleaned_controlled(
            pre.code(),
            &mut effective_config,
            control,
        )? {
            Ok(diagram_type) => diagram_type.to_owned(),
            Err(error) => return Ok(Err(error)),
        };
        control.checkpoint()?;
        family::apply_diagram_type_config_effects(
            &diagram_type,
            &pre.config,
            &mut effective_config,
        );
        if has_config_overrides {
            if let Err(error) = theme::apply_theme_defaults(&mut effective_config) {
                return Ok(Err(error.into()));
            }
        } else if cached_effective_config
            .as_ref()
            .is_some_and(|cached| effective_config.ptr_eq(cached))
        {
            effective_config = match self.engine.default_effective_config() {
                Ok(config) => config,
                Err(error) => return Ok(Err(error)),
            };
        } else {
            if let Err(error) = theme::apply_theme_defaults(&mut effective_config) {
                return Ok(Err(error.into()));
            }
        }

        control.checkpoint()?;
        let title = sanitized_title(pre.title.as_deref(), &effective_config);
        control.checkpoint()?;

        Ok(Ok((
            pre.source,
            ParseMetadata {
                diagram_type,
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }

    fn preprocess_and_assume_type_controlled(
        &self,
        diagram_type: &str,
        path: PreprocessPath,
        directive_recovery: DirectiveRecoveryMode,
        control: &ParseControl,
    ) -> ParseControlResult<Result<(PreprocessedSource, ParseMetadata)>> {
        let pre = match self.preprocess_input_with_directive_recovery_controlled(
            path,
            Some(diagram_type),
            directive_recovery,
            control,
        )? {
            Ok(pre) => pre,
            Err(error) => return Ok(Err(error)),
        };
        control.checkpoint()?;
        if pre.code().trim_start().starts_with("---") {
            return Ok(Err(Error::MalformedFrontMatter));
        }

        let has_config_overrides = !pre.config.is_empty_object();
        let mut effective_config = self.effective_config_before_detect(&pre.config);
        let cached_effective_config = (!has_config_overrides).then(|| effective_config.clone());
        family::apply_diagram_type_config_effects(diagram_type, &pre.config, &mut effective_config);
        if has_config_overrides {
            if let Err(error) = theme::apply_theme_defaults(&mut effective_config) {
                return Ok(Err(error.into()));
            }
        } else if cached_effective_config
            .as_ref()
            .is_some_and(|cached| effective_config.ptr_eq(cached))
        {
            effective_config = match self.engine.default_effective_config() {
                Ok(config) => config,
                Err(error) => return Ok(Err(error)),
            };
        } else {
            if let Err(error) = theme::apply_theme_defaults(&mut effective_config) {
                return Ok(Err(error.into()));
            }
        }

        control.checkpoint()?;
        let title = sanitized_title(pre.title.as_deref(), &effective_config);
        control.checkpoint()?;

        Ok(Ok((
            pre.source,
            ParseMetadata {
                diagram_type: diagram_type.to_string(),
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }

    #[cfg(test)]
    fn preprocess_input(
        &self,
        path: PreprocessPath,
        diagram_type: Option<&str>,
    ) -> Result<crate::PreprocessResult> {
        self.preprocess_input_with_directive_recovery(
            path,
            diagram_type,
            DirectiveRecoveryMode::Strict,
        )
    }

    #[cfg(test)]
    fn preprocess_input_with_directive_recovery(
        &self,
        path: PreprocessPath,
        diagram_type: Option<&str>,
        directive_recovery: DirectiveRecoveryMode,
    ) -> Result<crate::PreprocessResult> {
        let control = ParseControl::new();
        self.preprocess_input_with_directive_recovery_controlled(
            path,
            diagram_type,
            directive_recovery,
            &control,
        )
        .expect("a private parse control cannot be cancelled")
    }

    fn preprocess_input_with_directive_recovery_controlled(
        &self,
        path: PreprocessPath,
        diagram_type: Option<&str>,
        directive_recovery: DirectiveRecoveryMode,
        control: &ParseControl,
    ) -> ParseControlResult<Result<crate::PreprocessResult>> {
        control.checkpoint()?;
        match path {
            PreprocessPath::PublicParse => {
                preprocess_mermaid_public_parse_pipeline_with_directive_recovery_controlled(
                    self.text,
                    &self.engine.registry,
                    diagram_type,
                    directive_recovery,
                    control,
                )
            }
            PreprocessPath::Render => {
                preprocess_diagram_with_known_type_and_directive_recovery_controlled(
                    self.text,
                    &self.engine.registry,
                    diagram_type,
                    directive_recovery,
                    control,
                )
            }
        }
    }

    fn with_operation_context<R>(
        &self,
        f: impl FnOnce(&runtime::OperationContext) -> Result<R>,
    ) -> Result<R> {
        let context = self.engine.begin_operation()?;
        runtime::with_operation_context(&context, || f(&context))
    }

    fn effective_config_before_detect(&self, overrides: &MermaidConfig) -> MermaidConfig {
        if overrides.is_empty_object() {
            return self.engine.site_config.clone();
        }

        let mut effective_config = self.engine.site_config.clone();
        let effective_overrides = effective_config.secure_filtered_overrides(overrides);
        effective_config.deep_merge(effective_overrides.as_value());
        effective_config
    }
}

impl ParseTiming {
    fn operation_timing(
        self,
        context: &runtime::OperationContext,
    ) -> Option<runtime::OperationTiming> {
        (self != Self::None).then(|| context.timing()).flatten()
    }

    fn log_suppressed_error(
        self,
        total_start: Option<runtime::OperationTimer>,
        preprocess: Option<std::time::Duration>,
        parse: Option<std::time::Duration>,
        input_bytes: usize,
    ) {
        let Some(start) = total_start else {
            return;
        };

        match self {
            Self::None => {}
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
            Self::None => {}
            Self::Json => {
                eprintln!(
                    "[parse-timing] diagram={} total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    success.meta.diagram_type,
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
                    success.meta.diagram_type,
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
    total_start: Option<runtime::OperationTimer>,
    meta: &'a ParseMetadata,
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
mod editor_parse_source_map_tests {
    use super::{EditorParseSourceMap, ParsePipeline, PreprocessPath};
    use crate::{
        EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
        EditorSemanticSymbol, Engine, ParseCancelled, ParseControl, ParseOptions, SourceSpan,
    };

    #[test]
    fn controlled_snapshot_stops_before_a_cancelled_operation() {
        let control = ParseControl::new();
        control.cancel();

        let result =
            Engine::new().parse_diagram_snapshot_controlled_sync("flowchart TD\nA-->B\n", &control);

        assert!(matches!(result, Err(ParseCancelled)));
    }

    #[test]
    fn controlled_snapshot_stops_during_family_parser_work() {
        let mut source = String::from("flowchart TD\n");
        for index in 0..4_096 {
            source.push_str(&format!("n{index}-->n{}\n", index + 1));
        }
        let control = ParseControl::new();
        control.cancel_after_checkpoints(128);
        crate::diagrams::flowchart::reset_flowchart_token_trace_construction_count();

        let result = Engine::new().parse_diagram_snapshot_controlled_sync(&source, &control);

        assert!(matches!(result, Err(ParseCancelled)));
        assert_eq!(
            crate::diagrams::flowchart::flowchart_token_trace_construction_count(),
            0,
            "the cancellation schedule must stop in the family-owned accessibility scan"
        );
    }

    #[test]
    fn active_control_preserves_the_snapshot_model() {
        let source = "flowchart TD\nA-->B\n";
        let engine = Engine::new();
        let regular = engine
            .parse_diagram_snapshot_sync(source)
            .expect("regular parse succeeds")
            .expect("regular snapshot");
        let controlled = engine
            .parse_diagram_snapshot_controlled_sync(source, &ParseControl::new())
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
    fn render_preprocessing_does_not_repeat_frontmatter_extraction() {
        let input = concat!(
            "   ---\n",
            "title: only-visible-after-trimming\n",
            "---\n",
            "flowchart TD\n",
            "A-->B\n",
        );
        let engine = Engine::new();
        let pipeline = ParsePipeline::detect(&engine, input, ParseOptions::strict());

        let render = pipeline
            .preprocess_input(PreprocessPath::Render, None)
            .expect("single render preprocess");
        let public_parse = pipeline
            .preprocess_input(PreprocessPath::PublicParse, None)
            .expect("public parse preprocess");

        assert!(render.code().starts_with("---"));
        assert!(public_parse.code().starts_with("flowchart TD"));
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
        let engine = Engine::new();

        let error = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .expect_err("render preprocessing must leave the second block visible");

        assert!(matches!(error, crate::Error::MalformedFrontMatter));
    }

    #[test]
    fn fact_remap_drops_only_the_fact_crossing_a_deleted_span() {
        let original = "flowchart TD\nA%%{wrap}%%B\nC\n";
        let engine = Engine::new();
        let preprocessed = crate::preprocess::preprocess_mermaid_public_parse_pipeline(
            original,
            &engine.registry,
            None,
        )
        .unwrap()
        .source;
        let map = EditorParseSourceMap::new(&preprocessed);
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

        map.remap_facts(&mut facts, &ParseControl::new())
            .expect("a private parse control cannot be cancelled");

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
    fn fact_remap_observes_cancellation_during_large_fact_batches() {
        let original = "flowchart TD\nA\n";
        let engine = Engine::new();
        let preprocessed = crate::preprocess::preprocess_mermaid_public_parse_pipeline(
            original,
            &engine.registry,
            None,
        )
        .unwrap()
        .source;
        let map = EditorParseSourceMap::new(&preprocessed);
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
        let control = ParseControl::new();
        control.cancel_after_checkpoints(1);

        assert!(matches!(
            map.remap_facts(&mut facts, &control),
            Err(crate::ParseCancelled)
        ));
        assert!(control.is_cancelled());
    }
}
