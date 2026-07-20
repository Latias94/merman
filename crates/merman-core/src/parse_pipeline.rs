use crate::preprocess::{PreprocessedSource, preprocess_mermaid_public_parse_pipeline};
use crate::{
    EditorSemanticFacts, Engine, Error, MermaidConfig, ParseMetadata, ParseOptions, Result,
    SourceSpan, common_db, diagram, diagrams::error_diagram, family, runtime, sanitize, theme,
};
use diagram::{
    CustomJsonRenderModel, DiagramParseOutcome, DiagramParseSnapshot, DiagramWarningFact,
    ParsedDiagram, ParsedDiagramRender, ParsedEditorFacts, RegistryOwner, RenderSemanticModel,
    ResolvedRenderParser,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ParseSource<'a> {
    Detect,
    KnownType(&'a str),
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

    fn remap_facts(&self, facts: &mut EditorSemanticFacts) {
        let original_symbol_count = facts.symbols.len();
        facts.symbols = facts
            .symbols
            .drain(..)
            .filter_map(|mut symbol| {
                symbol.span = self.try_remap_source_span(symbol.span)?;
                symbol.selection = self.try_remap_source_span(symbol.selection)?;
                Some(symbol)
            })
            .collect();
        let dropped_lexemes = facts.remap_lexemes(|span| self.try_remap_source_span(span));

        let mut dropped_diagnostic_spans = 0usize;
        for diagnostic in &mut facts.diagnostics {
            if let Some(span) = diagnostic.span {
                diagnostic.span = self.try_remap_source_span(span);
                dropped_diagnostic_spans += usize::from(diagnostic.span.is_none());
            }
        }
        let original_expected_count = facts.expected_syntax.len();
        facts.expected_syntax = facts
            .expected_syntax
            .drain(..)
            .filter_map(|mut expected| {
                expected.span = self.try_remap_source_span(expected.span)?;
                Some(expected)
            })
            .collect();

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
        let (_, metadata) = match self.source {
            ParseSource::Detect => self.preprocess_and_detect_strict()?,
            ParseSource::KnownType(diagram_type) => {
                self.preprocess_and_assume_type(diagram_type)?
            }
        };
        Ok(metadata)
    }

    pub(crate) fn parse_json(&self, timing: ParseTiming) -> Result<Option<ParsedDiagram>> {
        self.parse_model(
            timing,
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
        let timing_enabled = timing.is_enabled();
        let total_start = runtime::timing_start(timing_enabled);
        let preprocess_start = runtime::timing_start(timing_enabled);
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        let source_map = EditorParseSourceMap::new(&code);
        let editor_input = source_map.parser_input();
        let preprocess = preprocess_start.map(runtime::timing_elapsed);

        let resolved = self.engine.diagram_registry.resolve(&meta.diagram_type);
        let combined = resolved
            .filter(|resolved| resolved.owner == RegistryOwner::BuiltIn)
            .and_then(|_| {
                family::combined_parser(self.engine.diagram_registry.profile(), &meta.diagram_type)
            });

        let parse_start = runtime::timing_start(timing_enabled);
        let parse_res = self.with_local_time(|| {
            Ok(match resolved {
                Some(resolved) => {
                    if let Some(parser) = combined {
                        let parsed = parser(editor_input, &meta);
                        let (model, editor_facts) = parsed.into_parts();
                        (model, Some(editor_facts))
                    } else {
                        ((resolved.parser)(editor_input, &meta), None)
                    }
                }
                None => (
                    Err(Error::UnsupportedDiagram {
                        diagram_type: meta.diagram_type.clone(),
                    }),
                    None,
                ),
            })
        });
        let parse = parse_start.map(runtime::timing_elapsed);
        let (model_result, combined_facts) = parse_res?;
        let owner = resolved.map(|resolved| resolved.owner);
        let mut model = match model_result {
            Ok(model) => model,
            Err(err) => {
                let err = source_map.remap_parse_error(err);
                let editor_facts =
                    self.finish_snapshot_editor_facts(owner, combined_facts, &meta, &source_map);
                return Ok(Some(DiagramParseSnapshot::new(
                    meta,
                    DiagramParseOutcome::Failed(err),
                    editor_facts,
                )));
            }
        };

        let sanitize_start = runtime::timing_start(timing_enabled);
        common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
        let sanitize = sanitize_start.map(runtime::timing_elapsed);
        Self::remap_value_warning_facts(&mut model, &source_map);
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
            self.finish_snapshot_editor_facts(owner, combined_facts, &meta, &source_map);

        Ok(Some(DiagramParseSnapshot::new(
            meta,
            DiagramParseOutcome::Parsed(model),
            editor_facts,
        )))
    }

    fn finish_snapshot_editor_facts(
        &self,
        owner: Option<RegistryOwner>,
        facts: Option<EditorSemanticFacts>,
        meta: &ParseMetadata,
        source_map: &EditorParseSourceMap<'_>,
    ) -> ParsedEditorFacts {
        match (owner, facts) {
            (Some(RegistryOwner::Custom), _) | (None, _) => ParsedEditorFacts::Unavailable,
            (Some(RegistryOwner::BuiltIn), Some(facts)) => ParsedEditorFacts::Available(
                self.finish_editor_semantic_facts(facts, meta, source_map),
            ),
            (Some(RegistryOwner::BuiltIn), None) => {
                debug_assert!(
                    family::combined_parser(
                        self.engine.diagram_registry.profile(),
                        &meta.diagram_type,
                    )
                    .is_none(),
                    "built-in families with editor capability must provide a combined semantic parser"
                );
                ParsedEditorFacts::Unavailable
            }
        }
    }

    pub(crate) fn parse_render_model(&self) -> Result<Option<ParsedDiagramRender>> {
        self.parse_model(
            ParseTiming::Render,
            Self::parse_render_semantic_model,
            RenderSemanticModel::sanitize_common_db_fields,
            error_diagram::suppressed_error_render_diagram,
            ParsedDiagramRender::new,
            |model, source_map| {
                model.remap_warning_fact_spans(|fact| {
                    Self::remap_warning_fact_spans(fact, source_map);
                });
            },
            |model| Some(model.kind()),
        )
    }

    fn finish_editor_semantic_facts(
        &self,
        mut facts: EditorSemanticFacts,
        meta: &ParseMetadata,
        source_map: &EditorParseSourceMap<'_>,
    ) -> EditorSemanticFacts {
        let family_directive_prefixes = std::mem::take(&mut facts.directive_prefixes);
        source_map.remap_facts(&mut facts);
        let family = family::diagram_type_family_id(&meta.diagram_type)
            .expect("built-in combined semantic facts belong to a catalog family");
        facts.finalize_lexemes(family, source_map.source.global_lexemes());
        for prefix in source_map.source.global_directive_prefixes() {
            facts.push_directive_prefix(prefix.clone());
        }
        for prefix in family_directive_prefixes {
            facts.push_directive_prefix(prefix);
        }
        facts
    }

    // These callbacks are the explicit stages of the parse pipeline; bundling them into an
    // untyped options object would obscure their distinct ownership and lifetimes.
    #[allow(clippy::too_many_arguments)]
    fn parse_model<T, O>(
        &self,
        timing: ParseTiming,
        parse: impl FnOnce(&Self, &str, &ParseMetadata) -> Result<T>,
        sanitize: impl FnOnce(&mut T, &MermaidConfig),
        suppressed: impl FnOnce(&ParseMetadata) -> O,
        finish: impl FnOnce(ParseMetadata, T) -> O,
        postprocess: impl FnOnce(&mut T, &EditorParseSourceMap<'_>),
        model_kind: impl FnOnce(&T) -> Option<&'static str>,
    ) -> Result<Option<O>> {
        let timing_enabled = timing.is_enabled();
        let total_start = runtime::timing_start(timing_enabled);

        let preprocess_start = runtime::timing_start(timing_enabled);
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        let source_map = EditorParseSourceMap::new(&code);
        let preprocess = preprocess_start.map(runtime::timing_elapsed);

        let parse_start = runtime::timing_start(timing_enabled);
        let parse_res = self.with_local_time(|| parse(self, source_map.parser_input(), &meta));
        let parse = parse_start.map(runtime::timing_elapsed);

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

        let sanitize_start = runtime::timing_start(timing_enabled);
        sanitize(&mut model, &meta.effective_config);
        let sanitize = sanitize_start.map(runtime::timing_elapsed);
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
    ) -> Result<RenderSemanticModel> {
        debug_assert_eq!(
            self.engine.diagram_registry.profile(),
            self.engine.render_diagram_registry.profile()
        );
        let semantic = self.engine.diagram_registry.resolve(&meta.diagram_type);
        let render = self
            .engine
            .render_diagram_registry
            .resolve(&meta.diagram_type);

        if let Some(ResolvedRenderParser::Custom(parser)) = render {
            return parser(code, meta).map(RenderSemanticModel::CustomJson);
        }

        if let Some(semantic) = semantic
            && semantic.owner == RegistryOwner::Custom
        {
            return (semantic.parser)(code, meta).map(|value| {
                RenderSemanticModel::CustomJson(CustomJsonRenderModel::from_semantic_registry(
                    meta.diagram_type.clone(),
                    value,
                ))
            });
        }

        if let Some(ResolvedRenderParser::BuiltIn(parser)) = render {
            return parser(code, meta);
        }

        if let Some(semantic) = semantic {
            debug_assert_eq!(semantic.owner, RegistryOwner::BuiltIn);
            return Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                format!(
                    "built-in diagram type `{}` is missing a typed render parser; the custom JSON boundary is reserved for custom registry adapters",
                    meta.diagram_type
                ),
            ));
        }

        Err(Error::UnsupportedDiagram {
            diagram_type: meta.diagram_type.clone(),
        })
    }

    fn preprocess(&self) -> Result<Option<(PreprocessedSource, ParseMetadata)>> {
        match self.source {
            ParseSource::Detect => self.preprocess_and_detect(),
            ParseSource::KnownType(diagram_type) => {
                self.preprocess_and_assume_type(diagram_type).map(Some)
            }
        }
    }

    fn preprocess_and_detect(&self) -> Result<Option<(PreprocessedSource, ParseMetadata)>> {
        match self.preprocess_and_detect_strict() {
            Err(Error::DetectType(_)) if self.options.suppress_errors => Ok(None),
            result => result.map(Some),
        }
    }

    fn preprocess_and_detect_strict(&self) -> Result<(PreprocessedSource, ParseMetadata)> {
        let pre = preprocess_mermaid_public_parse_pipeline(self.text, &self.engine.registry, None)?;
        if pre.code().trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let has_config_overrides = !pre.config.is_empty_object();
        let mut effective_config = self.effective_config_before_detect(&pre.config);
        let cached_effective_config = (!has_config_overrides).then(|| effective_config.clone());

        let diagram_type = self
            .engine
            .registry
            .detect_type_precleaned(pre.code(), &mut effective_config)
            .map(str::to_owned)?;
        family::apply_diagram_type_config_effects(
            &diagram_type,
            &pre.config,
            &mut effective_config,
        );
        if has_config_overrides {
            theme::apply_theme_defaults(&mut effective_config)?;
        } else if cached_effective_config
            .as_ref()
            .is_some_and(|cached| effective_config.ptr_eq(cached))
        {
            effective_config = self.engine.default_effective_config()?;
        } else {
            theme::apply_theme_defaults(&mut effective_config)?;
        }

        let title = sanitized_title(pre.title.as_deref(), &effective_config);

        Ok((
            pre.source,
            ParseMetadata {
                diagram_type,
                config: pre.config,
                effective_config,
                title,
            },
        ))
    }

    fn preprocess_and_assume_type(
        &self,
        diagram_type: &str,
    ) -> Result<(PreprocessedSource, ParseMetadata)> {
        let pre = preprocess_mermaid_public_parse_pipeline(
            self.text,
            &self.engine.registry,
            Some(diagram_type),
        )?;
        if pre.code().trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let has_config_overrides = !pre.config.is_empty_object();
        let mut effective_config = self.effective_config_before_detect(&pre.config);
        let cached_effective_config = (!has_config_overrides).then(|| effective_config.clone());
        family::apply_diagram_type_config_effects(diagram_type, &pre.config, &mut effective_config);
        if has_config_overrides {
            theme::apply_theme_defaults(&mut effective_config)?;
        } else if cached_effective_config
            .as_ref()
            .is_some_and(|cached| effective_config.ptr_eq(cached))
        {
            effective_config = self.engine.default_effective_config()?;
        } else {
            theme::apply_theme_defaults(&mut effective_config)?;
        }

        let title = sanitized_title(pre.title.as_deref(), &effective_config);

        Ok((
            pre.source,
            ParseMetadata {
                diagram_type: diagram_type.to_string(),
                config: pre.config,
                effective_config,
                title,
            },
        ))
    }

    fn with_local_time<R>(&self, f: impl FnOnce() -> Result<R>) -> Result<R> {
        let time_zone = self.engine.local_time_zone.as_ref().map_err(Clone::clone)?;
        runtime::with_fixed_today_local(self.engine.fixed_today_local, || {
            runtime::with_local_time_zone(time_zone, f)
        })
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
    fn is_enabled(self) -> bool {
        self != Self::None && Engine::parse_timing_enabled()
    }

    fn log_suppressed_error(
        self,
        total_start: Option<runtime::TimingInstant>,
        preprocess: Option<runtime::TimingDuration>,
        parse: Option<runtime::TimingDuration>,
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
                    runtime::timing_elapsed(start),
                    preprocess.unwrap_or_default(),
                    parse.unwrap_or_default(),
                    runtime::timing_zero_duration(),
                    input_bytes,
                );
            }
            Self::Render => {
                eprintln!(
                    "[parse-render-timing] diagram=error model=json total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    runtime::timing_elapsed(start),
                    preprocess.unwrap_or_default(),
                    parse.unwrap_or_default(),
                    runtime::timing_zero_duration(),
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
                    runtime::timing_elapsed(start),
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
                    runtime::timing_elapsed(start),
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
    total_start: Option<runtime::TimingInstant>,
    meta: &'a ParseMetadata,
    model_kind: Option<&'static str>,
    preprocess: Option<runtime::TimingDuration>,
    parse: Option<runtime::TimingDuration>,
    sanitize: Option<runtime::TimingDuration>,
    input_bytes: usize,
}

fn sanitized_title(title: Option<&str>, effective_config: &MermaidConfig) -> Option<String> {
    title
        .map(|title| sanitize::sanitize_text(title, effective_config))
        .filter(|title| !title.is_empty())
}

#[cfg(test)]
mod editor_parse_source_map_tests {
    use super::EditorParseSourceMap;
    use crate::{
        EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
        EditorSemanticSymbol, Engine, SourceSpan,
    };

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

        map.remap_facts(&mut facts);

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
}
