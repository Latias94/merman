use crate::{
    EditorSemanticFacts, Engine, Error, MermaidConfig, ParseMetadata, ParseOptions, Result,
    SourceSpan, common_db, diagram, diagrams::error_diagram, family, preprocess_diagram,
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

struct EditorParseSourceMap<'a> {
    parser_input: &'a str,
    remap: EditorSourceRemap,
}

enum EditorSourceRemap {
    None,
    Offset(usize),
    Normalized {
        normalized_offset: usize,
        normalized_to_original: Vec<usize>,
    },
    Unmapped,
}

impl<'a> EditorParseSourceMap<'a> {
    fn new(original: &'a str, preprocessed: &'a str) -> Self {
        if preprocessed == original {
            return Self {
                parser_input: original,
                remap: EditorSourceRemap::None,
            };
        }

        if preprocessed.is_empty() {
            return Self {
                parser_input: preprocessed,
                remap: EditorSourceRemap::Offset(original.len()),
            };
        }

        if let Some(offset) = original.rfind(preprocessed) {
            return Self {
                parser_input: preprocessed,
                remap: EditorSourceRemap::Offset(offset),
            };
        }

        if original.contains('\r') {
            let (normalized, normalized_to_original) = normalize_original_with_offsets(original);
            if let Some(normalized_offset) = normalized.rfind(preprocessed) {
                return Self {
                    parser_input: preprocessed,
                    remap: EditorSourceRemap::Normalized {
                        normalized_offset,
                        normalized_to_original,
                    },
                };
            }
        }

        Self {
            parser_input: preprocessed,
            remap: EditorSourceRemap::Unmapped,
        }
    }

    fn parser_input(&self) -> &'a str {
        self.parser_input
    }

    fn can_remap_facts(&self) -> bool {
        !matches!(self.remap, EditorSourceRemap::Unmapped)
    }

    fn remap_facts(&self, facts: &mut EditorSemanticFacts) {
        if matches!(
            self.remap,
            EditorSourceRemap::None | EditorSourceRemap::Offset(0)
        ) {
            return;
        }

        for symbol in &mut facts.symbols {
            symbol.span = self.remap_source_span(symbol.span);
            symbol.selection = self.remap_source_span(symbol.selection);
        }
        for diagnostic in &mut facts.diagnostics {
            diagnostic.span = diagnostic.span.map(|span| self.remap_source_span(span));
        }
        for expected in &mut facts.expected_syntax {
            expected.span = self.remap_source_span(expected.span);
        }
    }

    fn remap_source_span(&self, span: SourceSpan) -> SourceSpan {
        SourceSpan::new(self.remap_offset(span.start), self.remap_offset(span.end))
    }

    fn remap_offset(&self, offset: usize) -> usize {
        match &self.remap {
            EditorSourceRemap::None => offset,
            EditorSourceRemap::Offset(base) => offset + base,
            EditorSourceRemap::Unmapped => offset,
            EditorSourceRemap::Normalized {
                normalized_offset,
                normalized_to_original,
            } => {
                let normalized_index = normalized_offset + offset;
                normalized_to_original
                    .get(normalized_index)
                    .copied()
                    .unwrap_or_else(|| normalized_to_original.last().copied().unwrap_or(offset))
            }
        }
    }
}

fn normalize_original_with_offsets(original: &str) -> (String, Vec<usize>) {
    let mut normalized = String::with_capacity(original.len());
    let mut normalized_to_original = Vec::with_capacity(original.len() + 1);
    let bytes = original.as_bytes();
    let mut offset = 0;

    while offset < bytes.len() {
        normalized_to_original.push(offset);
        if bytes[offset] == b'\r' {
            normalized.push('\n');
            offset += if bytes.get(offset + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
            continue;
        }

        let ch = original[offset..]
            .chars()
            .next()
            .expect("offset should be at a UTF-8 character boundary");
        normalized.push(ch);
        for byte_offset in 1..ch.len_utf8() {
            normalized_to_original.push(offset + byte_offset);
        }
        offset += ch.len_utf8();
    }

    normalized_to_original.push(original.len());
    (normalized, normalized_to_original)
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
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        let source_map = EditorParseSourceMap::new(self.text, &code);
        if !source_map.can_remap_facts() {
            return Ok(None);
        }
        let editor_input = source_map.parser_input();

        let facts = match meta.diagram_type.as_str() {
            "flowchart-v2" | "flowchart" | "flowchart-elk" => {
                crate::diagrams::flowchart::parse_flowchart_editor_facts(editor_input, &meta)?
            }
            "sequence" => {
                crate::diagrams::sequence::parse_sequence_editor_facts(editor_input, &meta)
            }
            "state" | "stateDiagram" => {
                crate::diagrams::state::parse_state_editor_facts(editor_input, &meta)
            }
            "class" | "classDiagram" => {
                crate::diagrams::class::parse_class_editor_facts(editor_input, &meta)
            }
            "er" | "erDiagram" => crate::diagrams::er::parse_er_editor_facts(editor_input, &meta),
            "mindmap" => crate::diagrams::mindmap::parse_mindmap_editor_facts(editor_input, &meta),
            "gantt" => crate::diagrams::gantt::parse_gantt_editor_facts(editor_input, &meta),
            "architecture" => {
                crate::diagrams::architecture::parse_architecture_editor_facts(editor_input, &meta)
            }
            "block" => crate::diagrams::block::parse_block_editor_facts(editor_input, &meta),
            "c4" => crate::diagrams::c4::parse_c4_editor_facts(editor_input, &meta),
            "gitGraph" => {
                crate::diagrams::git_graph::parse_git_graph_editor_facts(editor_input, &meta)
            }
            "kanban" => crate::diagrams::kanban::parse_kanban_editor_facts(editor_input, &meta),
            "ishikawa" => {
                crate::diagrams::ishikawa::parse_ishikawa_editor_facts(editor_input, &meta)
            }
            "journey" => crate::diagrams::journey::parse_journey_editor_facts(editor_input, &meta),
            "info" => crate::diagrams::info::parse_info_editor_facts(editor_input, &meta),
            "timeline" => {
                crate::diagrams::timeline::parse_timeline_editor_facts(editor_input, &meta)
            }
            "pie" => crate::diagrams::pie::parse_pie_editor_facts(editor_input, &meta),
            "packet" => crate::diagrams::packet::parse_packet_editor_facts(editor_input, &meta),
            "sankey" => crate::diagrams::sankey::parse_sankey_editor_facts(editor_input, &meta),
            "treeView" => {
                crate::diagrams::tree_view::parse_tree_view_editor_facts(editor_input, &meta)
            }
            "eventmodeling" => crate::diagrams::eventmodeling::parse_eventmodeling_editor_facts(
                editor_input,
                &meta,
            ),
            "quadrantChart" => crate::diagrams::quadrant_chart::parse_quadrant_chart_editor_facts(
                editor_input,
                &meta,
            ),
            "radar" => crate::diagrams::radar::parse_radar_editor_facts(editor_input, &meta),
            "treemap" => crate::diagrams::treemap::parse_treemap_editor_facts(editor_input, &meta),
            "requirement" => {
                crate::diagrams::requirement::parse_requirement_editor_facts(editor_input, &meta)
            }
            "venn" => crate::diagrams::venn::parse_venn_editor_facts(editor_input, &meta),
            "xychart" => crate::diagrams::xychart::parse_xychart_editor_facts(editor_input, &meta),
            "zenuml" => crate::diagrams::zenuml::parse_zenuml_editor_facts(editor_input, &meta),
            _ => return Ok(None),
        };

        let EditorSemanticFacts {
            completeness,
            symbols,
            directive_prefixes: family_directive_prefixes,
            diagnostics,
            expected_syntax,
        } = facts;
        directive_prefixes.extend(family_directive_prefixes);
        let mut facts = EditorSemanticFacts {
            completeness,
            symbols,
            directive_prefixes: Vec::new(),
            diagnostics,
            expected_syntax,
        };
        source_map.remap_facts(&mut facts);
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
            return Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                format!(
                    "built-in diagram type `{}` is missing a typed render parser; JSON render fallback is reserved for error and custom diagram adapters",
                    meta.diagram_type
                ),
            ));
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
