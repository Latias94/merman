use crate::{
    EditorSemanticFacts, EditorSpanCoordinateSpace, Engine, Error, MermaidConfig, ParseMetadata,
    ParseOptions, Result, SourceSpan, common_db, diagram, diagrams::error_diagram, family,
    preprocess_diagram, preprocess_diagram_with_known_type, runtime, sanitize, theme,
};
use diagram::{
    DiagramWarningFact, ParsedDiagram, ParsedDiagramRender, ParsedDiagramWithEditorFacts,
    ParsedEditorFacts, RenderSemanticModel,
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
    original: &'a str,
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
    ParserInputCoordinates,
}

const WARNING_FACT_REMAP_CONTEXT_EXPANSIONS: [usize; 6] = [1, 4, 8, 16, 32, 64];

impl<'a> EditorParseSourceMap<'a> {
    fn new(original: &'a str, preprocessed: &'a str) -> Self {
        if preprocessed == original {
            return Self {
                original,
                parser_input: original,
                remap: EditorSourceRemap::None,
            };
        }

        if preprocessed.is_empty() {
            return Self {
                original,
                parser_input: preprocessed,
                remap: EditorSourceRemap::Offset(original.len()),
            };
        }

        if let Some(offset) = original.rfind(preprocessed) {
            return Self {
                original,
                parser_input: preprocessed,
                remap: EditorSourceRemap::Offset(offset),
            };
        }

        if original.contains('\r') {
            let (normalized, normalized_to_original) = normalize_original_with_offsets(original);
            if let Some(normalized_offset) = normalized.rfind(preprocessed) {
                return Self {
                    original,
                    parser_input: preprocessed,
                    remap: EditorSourceRemap::Normalized {
                        normalized_offset,
                        normalized_to_original,
                    },
                };
            }
        }

        Self {
            original,
            parser_input: preprocessed,
            remap: EditorSourceRemap::ParserInputCoordinates,
        }
    }

    fn parser_input(&self) -> &'a str {
        self.parser_input
    }

    fn remap_facts(&self, facts: &mut EditorSemanticFacts) {
        match self.remap {
            EditorSourceRemap::None | EditorSourceRemap::Offset(0) => {
                facts.span_coordinate_space = EditorSpanCoordinateSpace::OriginalSource;
                return;
            }
            EditorSourceRemap::ParserInputCoordinates => {
                facts.span_coordinate_space = EditorSpanCoordinateSpace::ParserInput;
                return;
            }
            EditorSourceRemap::Offset(_) | EditorSourceRemap::Normalized { .. } => {
                facts.span_coordinate_space = EditorSpanCoordinateSpace::OriginalSource;
            }
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
        let start = self.try_remap_offset(span.start)?;
        let end = self.try_remap_offset(span.end)?;
        (start <= end).then(|| SourceSpan::new(start, end))
    }

    fn try_remap_warning_source_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        self.try_remap_source_span(span)
            .or_else(|| self.try_remap_span_by_unique_fragment(span))
    }

    fn try_remap_span_by_unique_fragment(&self, span: SourceSpan) -> Option<SourceSpan> {
        if span.start >= span.end {
            return None;
        }
        if let Some(mapped) = self.try_remap_span_with_unique_context(span, span.start, span.end) {
            return Some(mapped);
        }

        // Some warning facts are produced after family-local masking, so the raw span text can
        // also appear in frontmatter or config. Grow bounded context until the source fragment is
        // unique, then translate only the original span within that fragment.
        for extra_after in WARNING_FACT_REMAP_CONTEXT_EXPANSIONS {
            let context_end = span
                .end
                .saturating_add(extra_after)
                .min(self.parser_input.len());
            if let Some(mapped) =
                self.try_remap_span_with_unique_context(span, span.start, context_end)
            {
                return Some(mapped);
            }
        }

        for extra_before in WARNING_FACT_REMAP_CONTEXT_EXPANSIONS {
            let context_start = span.start.saturating_sub(extra_before);
            if let Some(mapped) =
                self.try_remap_span_with_unique_context(span, context_start, span.end)
            {
                return Some(mapped);
            }
        }

        None
    }

    fn try_remap_span_with_unique_context(
        &self,
        span: SourceSpan,
        context_start: usize,
        context_end: usize,
    ) -> Option<SourceSpan> {
        let fragment = self.parser_input.get(context_start..context_end)?;
        if fragment.is_empty() {
            return None;
        }

        let mut matches = self.original.match_indices(fragment);
        let (match_start, _) = matches.next()?;
        if matches.next().is_some() {
            return None;
        }

        let remapped_start = match_start.checked_add(span.start.checked_sub(context_start)?)?;
        let remapped_end = remapped_start.checked_add(span.end.checked_sub(span.start)?)?;
        (remapped_end <= self.original.len()).then(|| SourceSpan::new(remapped_start, remapped_end))
    }

    fn remap_offset(&self, offset: usize) -> usize {
        if let Some(remapped) = self.try_remap_offset(offset) {
            return remapped;
        }
        match &self.remap {
            EditorSourceRemap::None => offset,
            EditorSourceRemap::Offset(base) => offset + base,
            EditorSourceRemap::ParserInputCoordinates => offset,
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

    fn try_remap_offset(&self, offset: usize) -> Option<usize> {
        if offset > self.parser_input.len() {
            return None;
        }
        match &self.remap {
            EditorSourceRemap::None => Some(offset),
            EditorSourceRemap::Offset(base) => base.checked_add(offset),
            EditorSourceRemap::ParserInputCoordinates => None,
            EditorSourceRemap::Normalized {
                normalized_offset,
                normalized_to_original,
            } => normalized_to_original
                .get(normalized_offset.checked_add(offset)?)
                .copied(),
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

#[cfg(test)]
mod editor_parse_source_map_tests {
    use super::EditorParseSourceMap;
    use crate::{Error, ParseDiagnosticSpanKind, SourceSpan};

    #[test]
    fn parser_input_coordinate_parse_error_drops_span() {
        let map = EditorParseSourceMap::new(
            "flowchart TD\nA%% removed comment %%-->B\n",
            "flowchart TD\nA-->B\n",
        );
        let error = map.remap_parse_error(Error::diagram_parse_exact(
            "flowchart-v2",
            "bad parser input span",
            SourceSpan::new(13, 14),
        ));

        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected parse diagnostic");
        };
        assert_eq!(diagnostic.span(), None);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Fallback);
    }

    #[test]
    fn normalized_parse_error_remaps_valid_crlf_span() {
        let original = "flowchart TD\r\nA-->B\r\n";
        let preprocessed = "flowchart TD\nA-->B\n";
        let target = preprocessed.find('B').expect("parser input target");
        let map = EditorParseSourceMap::new(original, preprocessed);
        let error = map.remap_parse_error(Error::diagram_parse_exact(
            "flowchart-v2",
            "bad parser input span",
            SourceSpan::new(target, target + 1),
        ));

        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected parse diagnostic");
        };
        let span = diagnostic.span().expect("remapped span");
        assert_eq!(&original[span.start..span.end], "B");
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn normalized_parse_error_drops_out_of_bounds_span() {
        let original = "flowchart TD\r\nA-->B\r\n";
        let preprocessed = "flowchart TD\nA-->B\n";
        let map = EditorParseSourceMap::new(original, preprocessed);
        let error = map.remap_parse_error(Error::diagram_parse_exact(
            "flowchart-v2",
            "bad parser input span",
            SourceSpan::new(preprocessed.len() + 1, preprocessed.len() + 2),
        ));

        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected parse diagnostic");
        };
        assert_eq!(diagnostic.span(), None);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Fallback);
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
            Self::remap_value_warning_facts,
            |_| None,
        )
    }

    pub(crate) fn parse_json_with_editor_facts(
        &self,
        timing: ParseTiming,
    ) -> Result<Option<ParsedDiagramWithEditorFacts>> {
        let timing_enabled = timing.is_enabled();
        let total_start = runtime::timing_start(timing_enabled);
        let preprocess_start = runtime::timing_start(timing_enabled);
        let directive_prefixes = editor_directive_prefixes(self.text);
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        let source_map = EditorParseSourceMap::new(self.text, &code);
        let editor_input = source_map.parser_input();
        let preprocess = preprocess_start.map(runtime::timing_elapsed);

        let parse_start = runtime::timing_start(timing_enabled);
        let parsed = match meta.diagram_type.as_str() {
            "flowchart-v2" | "flowchart" | "flowchart-elk" | "swimlane" => {
                let parse_res = self.with_fixed_time(|| {
                    crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts(
                        editor_input,
                        &meta,
                    )
                });
                let parse = parse_start.map(runtime::timing_elapsed);
                let (mut model, facts) = match parse_res {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        if !self.options.suppress_errors {
                            return Err(source_map.remap_parse_error(err));
                        }

                        timing.log_suppressed_error(
                            total_start,
                            preprocess,
                            parse,
                            self.text.len(),
                        );
                        return Ok(Some(ParsedDiagramWithEditorFacts {
                            diagram: error_diagram::suppressed_error_diagram(&meta),
                            editor_facts: ParsedEditorFacts::Unavailable,
                        }));
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
                let facts =
                    self.finish_editor_semantic_facts(facts, &source_map, directive_prefixes);
                return Ok(Some(ParsedDiagramWithEditorFacts {
                    diagram: ParsedDiagram { meta, model },
                    editor_facts: ParsedEditorFacts::Available(facts),
                }));
            }
            _ => {
                let parse_res = self.with_fixed_time(|| {
                    diagram::parse_or_unsupported(
                        &self.engine.diagram_registry,
                        &meta.diagram_type,
                        editor_input,
                        &meta,
                    )
                });
                let parse = parse_start.map(runtime::timing_elapsed);
                let mut model = match parse_res {
                    Ok(model) => model,
                    Err(err) => {
                        if !self.options.suppress_errors {
                            return Err(source_map.remap_parse_error(err));
                        }

                        timing.log_suppressed_error(
                            total_start,
                            preprocess,
                            parse,
                            self.text.len(),
                        );
                        return Ok(Some(ParsedDiagramWithEditorFacts {
                            diagram: error_diagram::suppressed_error_diagram(&meta),
                            editor_facts: ParsedEditorFacts::Unavailable,
                        }));
                    }
                };
                let sanitize_start = runtime::timing_start(timing_enabled);
                common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
                let sanitize = sanitize_start.map(runtime::timing_elapsed);
                timing.log_success(ParseTimingSuccess {
                    total_start,
                    meta: &meta,
                    model_kind: None,
                    preprocess,
                    parse,
                    sanitize,
                    input_bytes: self.text.len(),
                });
                ParsedDiagram { meta, model }
            }
        };

        let mut parsed = parsed;
        Self::remap_value_warning_facts(&mut parsed.model, &source_map);

        let editor_facts = match self.parse_editor_semantic_facts_from_preprocessed(
            editor_input,
            &parsed.meta,
            &source_map,
            directive_prefixes,
        ) {
            Ok(Some(facts)) => ParsedEditorFacts::Available(facts),
            Ok(None) => ParsedEditorFacts::Unavailable,
            Err(error) => ParsedEditorFacts::Error(error),
        };
        Ok(Some(ParsedDiagramWithEditorFacts {
            diagram: parsed,
            editor_facts,
        }))
    }

    pub(crate) fn parse_render_model(&self) -> Result<Option<ParsedDiagramRender>> {
        self.parse_model(
            ParseTiming::Render,
            Self::parse_render_semantic_model,
            RenderSemanticModel::sanitize_common_db_fields,
            error_diagram::suppressed_error_render_diagram,
            |meta, model| ParsedDiagramRender { meta, model },
            |model, source_map| {
                model.remap_warning_fact_spans(|fact| {
                    Self::remap_warning_fact_spans(fact, source_map);
                });
            },
            |model| Some(model.kind()),
        )
    }

    pub(crate) fn parse_editor_semantic_facts(&self) -> Result<Option<EditorSemanticFacts>> {
        let mut directive_prefixes = editor_directive_prefixes(self.text);
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        let source_map = EditorParseSourceMap::new(self.text, &code);
        self.parse_editor_semantic_facts_from_preprocessed(
            source_map.parser_input(),
            &meta,
            &source_map,
            std::mem::take(&mut directive_prefixes),
        )
    }

    fn parse_editor_semantic_facts_from_preprocessed(
        &self,
        editor_input: &str,
        meta: &ParseMetadata,
        source_map: &EditorParseSourceMap<'_>,
        directive_prefixes: Vec<String>,
    ) -> Result<Option<EditorSemanticFacts>> {
        let registry_profile = self.engine.diagram_registry.profile();
        if !family::diagram_type_supported_in_profile(registry_profile, meta.diagram_type.as_str())
        {
            return Err(Error::UnsupportedDiagram {
                diagram_type: meta.diagram_type.clone(),
            });
        }

        let facts = match meta.diagram_type.as_str() {
            "flowchart-v2" | "flowchart" | "flowchart-elk" | "swimlane" => {
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
            "cynefin" => crate::diagrams::cynefin::parse_cynefin_editor_facts(editor_input, &meta),
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
            "railroad" => {
                crate::diagrams::railroad::parse_railroad_editor_facts(editor_input, &meta)
            }
            "railroadEbnf" => {
                crate::diagrams::railroad::parse_railroad_ebnf_editor_facts(editor_input, &meta)
            }
            "railroadAbnf" => {
                crate::diagrams::railroad::parse_railroad_abnf_editor_facts(editor_input, &meta)
            }
            "railroadPeg" => {
                crate::diagrams::railroad::parse_railroad_peg_editor_facts(editor_input, &meta)
            }
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

        Ok(Some(self.finish_editor_semantic_facts(
            facts,
            source_map,
            directive_prefixes,
        )))
    }

    fn finish_editor_semantic_facts(
        &self,
        facts: EditorSemanticFacts,
        source_map: &EditorParseSourceMap<'_>,
        mut directive_prefixes: Vec<String>,
    ) -> EditorSemanticFacts {
        let EditorSemanticFacts {
            completeness,
            span_coordinate_space: _,
            symbols,
            directive_prefixes: family_directive_prefixes,
            diagnostics,
            expected_syntax,
        } = facts;
        directive_prefixes.extend(family_directive_prefixes);
        let mut facts = EditorSemanticFacts {
            completeness,
            span_coordinate_space: EditorSpanCoordinateSpace::OriginalSource,
            symbols,
            directive_prefixes: Vec::new(),
            diagnostics,
            expected_syntax,
        };
        source_map.remap_facts(&mut facts);
        for prefix in directive_prefixes {
            facts.push_directive_prefix(prefix);
        }
        facts
    }

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
        let source_map = EditorParseSourceMap::new(self.text, &code);
        let preprocess = preprocess_start.map(runtime::timing_elapsed);

        let parse_start = runtime::timing_start(timing_enabled);
        let parse_res = self.with_fixed_time(|| parse(self, source_map.parser_input(), &meta));
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
        family::apply_diagram_type_config_defaults(
            &diagram_type,
            &pre.config,
            &mut effective_config,
        );
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
        family::apply_diagram_type_config_defaults(
            diagram_type,
            &pre.config,
            &mut effective_config,
        );
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
