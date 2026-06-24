use crate::{
    EditorSemanticFacts, Engine, Error, MermaidConfig, ParseMetadata, ParseOptions, Result,
    common_db, diagram, diagrams::error_diagram, family, preprocess_diagram,
    preprocess_diagram_with_known_type, runtime, sanitize, theme,
};
use diagram::{ParsedDiagram, ParsedDiagramRender, RenderSemanticModel};

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

    pub(crate) fn metadata(&self) -> Result<Option<ParseMetadata>> {
        Ok(self.preprocess()?.map(|(_, meta)| meta))
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
            |_| None,
        )
    }

    pub(crate) fn parse_render_model(&self) -> Result<Option<ParsedDiagramRender>> {
        self.parse_model(
            ParseTiming::Render,
            Self::parse_render_semantic_model,
            RenderSemanticModel::sanitize_common_db_fields,
            error_diagram::suppressed_error_render_diagram,
            |meta, model| ParsedDiagramRender { meta, model },
            |model| Some(model.kind()),
        )
    }

    pub(crate) fn parse_editor_semantic_facts(&self) -> Result<Option<EditorSemanticFacts>> {
        let mut directive_prefixes = editor_directive_prefixes(self.text);
        let Some((_code, meta)) = self.preprocess()? else {
            return Ok(None);
        };

        let facts = match meta.diagram_type.as_str() {
            "flowchart-v2" | "flowchart-elk" => {
                crate::diagrams::flowchart::parse_flowchart_editor_facts(self.text, &meta)?
            }
            "sequence" => crate::diagrams::sequence::parse_sequence_editor_facts(self.text, &meta),
            _ => return Ok(None),
        };

        directive_prefixes.extend(facts.directive_prefixes);
        let mut facts = EditorSemanticFacts {
            completeness: facts.completeness,
            symbols: facts.symbols,
            directive_prefixes: Vec::new(),
        };
        for prefix in directive_prefixes {
            facts.push_directive_prefix(prefix);
        }
        Ok(Some(facts))
    }

    fn parse_model<T, O>(
        &self,
        timing: ParseTiming,
        parse: impl FnOnce(&Self, &str, &ParseMetadata) -> Result<T>,
        sanitize: impl FnOnce(&mut T, &MermaidConfig),
        suppressed: impl FnOnce(&ParseMetadata) -> O,
        finish: impl FnOnce(ParseMetadata, T) -> O,
        model_kind: impl FnOnce(&T) -> Option<&'static str>,
    ) -> Result<Option<O>> {
        let timing_enabled = timing.is_enabled();
        let total_start = runtime::timing_start(timing_enabled);

        let preprocess_start = runtime::timing_start(timing_enabled);
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        let preprocess = preprocess_start.map(runtime::timing_elapsed);

        let parse_start = runtime::timing_start(timing_enabled);
        let parse_res = self.with_fixed_time(|| parse(self, &code, &meta));
        let parse = parse_start.map(runtime::timing_elapsed);

        let mut model = match parse_res {
            Ok(model) => model,
            Err(err) => {
                if !self.options.suppress_errors {
                    return Err(err);
                }

                timing.log_suppressed_error(total_start, preprocess, parse, self.text.len());
                return Ok(Some(suppressed(&meta)));
            }
        };

        let sanitize_start = runtime::timing_start(timing_enabled);
        sanitize(&mut model, &meta.effective_config);
        let sanitize = sanitize_start.map(runtime::timing_elapsed);

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

    fn parse_render_semantic_model(
        &self,
        code: &str,
        meta: &ParseMetadata,
    ) -> Result<RenderSemanticModel> {
        if let Some(parser) = self.engine.render_diagram_registry.get(&meta.diagram_type) {
            return parser(code, meta);
        }

        let registry_profile = self.engine.render_diagram_registry.profile();
        debug_assert_eq!(self.engine.diagram_registry.profile(), registry_profile);
        if !family::permits_json_render_fallback(registry_profile, &meta.diagram_type) {
            return Err(Error::DiagramParse {
                diagram_type: meta.diagram_type.clone(),
                message: format!(
                    "built-in diagram type `{}` is missing a typed render parser; JSON render fallback is reserved for error and custom diagram adapters",
                    meta.diagram_type
                ),
            });
        }

        diagram::parse_or_unsupported(
            &self.engine.diagram_registry,
            &meta.diagram_type,
            code,
            meta,
        )
        .map(RenderSemanticModel::Json)
    }

    fn preprocess(&self) -> Result<Option<(String, ParseMetadata)>> {
        match self.source {
            ParseSource::Detect => self.preprocess_and_detect(),
            ParseSource::KnownType(diagram_type) => self.preprocess_and_assume_type(diagram_type),
        }
    }

    fn preprocess_and_detect(&self) -> Result<Option<(String, ParseMetadata)>> {
        let pre = preprocess_diagram(self.text, &self.engine.registry)?;
        if pre.code.trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let has_config_overrides = !pre.config.is_empty_object();
        let mut effective_config = self.effective_config_before_detect(&pre.config);
        let cached_effective_config = (!has_config_overrides).then(|| effective_config.clone());

        let diagram_type = match self
            .engine
            .registry
            .detect_type_precleaned(&pre.code, &mut effective_config)
        {
            Ok(diagram_type) => diagram_type.to_string(),
            Err(err) => {
                if self.options.suppress_errors {
                    return Ok(None);
                }
                return Err(err);
            }
        };
        if has_config_overrides {
            theme::apply_theme_defaults(&mut effective_config);
        } else if cached_effective_config
            .as_ref()
            .is_some_and(|cached| effective_config.ptr_eq(cached))
        {
            effective_config = self.engine.default_effective_config();
        } else {
            theme::apply_theme_defaults(&mut effective_config);
        }

        let title = sanitized_title(pre.title.as_deref(), &effective_config);

        Ok(Some((
            pre.code,
            ParseMetadata {
                diagram_type,
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }

    fn preprocess_and_assume_type(
        &self,
        diagram_type: &str,
    ) -> Result<Option<(String, ParseMetadata)>> {
        let pre = preprocess_diagram_with_known_type(
            self.text,
            &self.engine.registry,
            Some(diagram_type),
        )?;
        if pre.code.trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let has_config_overrides = !pre.config.is_empty_object();
        let mut effective_config = self.effective_config_before_detect(&pre.config);
        let cached_effective_config = (!has_config_overrides).then(|| effective_config.clone());
        family::apply_known_type_detector_side_effects(diagram_type, &mut effective_config);
        if has_config_overrides {
            theme::apply_theme_defaults(&mut effective_config);
        } else if cached_effective_config
            .as_ref()
            .is_some_and(|cached| effective_config.ptr_eq(cached))
        {
            effective_config = self.engine.default_effective_config();
        } else {
            theme::apply_theme_defaults(&mut effective_config);
        }

        let title = sanitized_title(pre.title.as_deref(), &effective_config);

        Ok(Some((
            pre.code,
            ParseMetadata {
                diagram_type: diagram_type.to_string(),
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }

    fn with_fixed_time<R>(&self, f: impl FnOnce() -> R) -> R {
        runtime::with_fixed_today_local(self.engine.fixed_today_local, || {
            runtime::with_fixed_local_offset_minutes(self.engine.fixed_local_offset_minutes, f)
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

fn editor_directive_prefixes(text: &str) -> Vec<String> {
    let mut prefixes = Vec::new();
    for line in text.lines() {
        if let Some(prefix) = editor_directive_prefix(line) {
            let prefix = prefix.to_string();
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
        }
    }
    prefixes
}

fn editor_directive_prefix(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();

    if let Some(rest) = trimmed.strip_prefix("%%{") {
        let name = rest
            .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '}'))
            .next()
            .filter(|name| !name.is_empty())?;

        return matches!(name, "init" | "initialize" | "wrap").then_some(match name {
            "init" => "init",
            "initialize" => "initialize",
            "wrap" => "wrap",
            _ => unreachable!(),
        });
    }

    if trimmed.starts_with(":::") {
        return Some(":::");
    }

    None
}

fn sanitized_title(title: Option<&str>, effective_config: &MermaidConfig) -> Option<String> {
    title
        .map(|title| sanitize::sanitize_text(title, effective_config))
        .filter(|title| !title.is_empty())
}
