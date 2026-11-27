#[cfg(any(test, feature = "editor-facts"))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, MermaidConfig, family::CombinedSemanticFailure};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_structured::xychart::{
    ParsedAxisStatement, ParsedPlotStatement, ParsedXyChartStatement, XyChartObservation,
    parse_xychart_source_with_observer_controlled,
};
use merman_core_structured::xychart::{XyChartIssue, parse_xychart_source_controlled};
#[cfg(test)]
use merman_core_structured::xychart::{
    reset_xychart_syntax_construction_count, xychart_syntax_construction_count,
};
#[cfg(feature = "compat-json")]
use serde_json::Value;

pub(crate) use merman_model::families::xy_chart::*;

pub(crate) fn sanitize_common_db_fields(model: &mut XyChartDiagramModel, config: &MermaidConfig) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

struct XyChartSemanticSource {
    model: XyChartDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct XyChartParseOutcome {
    source: XyChartSemanticSource,
    first_error: Option<Error>,
}

impl XyChartParseOutcome {
    fn into_strict_result(
        self,
    ) -> std::result::Result<XyChartSemanticSource, CombinedSemanticFailure> {
        match self.first_error {
            Some(error) => Err(CombinedSemanticFailure::parser_recovery(
                #[cfg(feature = "editor-facts")]
                "xychart",
                error,
                #[cfg(feature = "editor-facts")]
                self.source.editor_facts,
            )),
            None => Ok(self.source),
        }
    }
}

fn construct_xychart_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<XyChartParseOutcome> {
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand.captures_rich().then(EditorSemanticFacts::new);
    #[cfg(feature = "editor-facts")]
    let parsed = if let Some(facts) = &mut editor_facts {
        parse_xychart_source_with_observer_controlled(
            code,
            config,
            &mut |observation| project_xychart_observation(facts, observation),
            control,
        )
    } else {
        parse_xychart_source_controlled(code, config, control)
    }?;
    #[cfg(not(feature = "editor-facts"))]
    let parsed = parse_xychart_source_controlled(code, config, control)?;

    let first_error = parsed.first_issue.map(|issue| match issue {
        XyChartIssue::Exact { message, span } => {
            Error::diagram_parse_exact("xychart", message, span)
        }
        XyChartIssue::InsertionPoint { message, offset } => {
            Error::diagram_parse_insertion_point("xychart", message, offset)
        }
        XyChartIssue::MissingHeader(span) => {
            Error::diagram_parse_exact(diagram_type, "expected xychart", span)
        }
        XyChartIssue::UnexpectedStatement { message, span } => {
            Error::diagram_parse_exact(diagram_type, message, span)
        }
    });
    Ok(XyChartParseOutcome {
        source: XyChartSemanticSource {
            model: parsed.model,
            #[cfg(feature = "editor-facts")]
            editor_facts: editor_demand.finish_facts(editor_facts),
        },
        first_error,
    })
}

#[cfg(feature = "editor-facts")]
fn project_xychart_observation(
    facts: &mut EditorSemanticFacts,
    observation: XyChartObservation<'_>,
) {
    use ParsedXyChartStatement as Statement;

    let (prefix, value, detail) = match observation {
        XyChartObservation::Header(span) => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            if let Some(span) = span {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    span,
                ));
            }
            return;
        }
        XyChartObservation::Statement(statement) => match statement {
            Statement::Title(value) => ("title", value, "xychart title"),
            Statement::AccessibilityTitle(value) => {
                ("accTitle", value, "xychart accessibility title")
            }
            Statement::AccessibilityDescription(value) => {
                ("accDescr", value, "xychart accessibility description")
            }
            Statement::XAxis(axis) => {
                return push_xychart_axis_facts(facts, axis, "xychart x-axis");
            }
            Statement::YAxis(axis) => {
                return push_xychart_axis_facts(facts, axis, "xychart y-axis");
            }
            Statement::Line(plot) => {
                return push_xychart_plot_statement_facts(
                    facts,
                    plot,
                    "xychart line",
                    "xychart line data label",
                );
            }
            Statement::Bar(plot) => {
                return push_xychart_plot_statement_facts(
                    facts,
                    plot,
                    "xychart bar",
                    "xychart bar data label",
                );
            }
        },
    };
    facts.push_directive_prefix(prefix);
    if prefix == "title" || !value.as_str().is_empty() {
        push_xychart_payload_fact(facts, value.as_str(), value.span(), detail);
    }
}

#[cfg(feature = "editor-facts")]
fn push_xychart_axis_facts(
    facts: &mut EditorSemanticFacts,
    axis: &ParsedAxisStatement,
    detail: &'static str,
) {
    if let Some(title) = axis.title() {
        push_xychart_payload_fact(facts, title.as_str(), title.span(), detail);
    }
    if let Some(span) = axis.data_span() {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            span,
        ));
    }
}

#[cfg(feature = "editor-facts")]
fn push_xychart_plot_statement_facts(
    facts: &mut EditorSemanticFacts,
    plot: &ParsedPlotStatement,
    detail: &'static str,
    data_label_detail: &'static str,
) {
    if let Some(title) = plot.title() {
        push_xychart_payload_fact(facts, title.as_str(), title.span(), detail);
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        plot.data_span(),
    ));
    for (text, span) in plot.data_labels() {
        push_xychart_payload_fact(facts, text, span, data_label_detail);
    }
}

#[cfg(feature = "editor-facts")]
fn push_xychart_payload_fact(
    facts: &mut EditorSemanticFacts,
    text: &str,
    span: SourceSpan,
    detail: &'static str,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.to_string(),
        Some(detail.to_string()),
        EditorSemanticKind::String,
        span,
        span,
    ));
}

pub(crate) fn parse_xychart_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<XyChartDiagramModel>> {
    let construction = construct_xychart_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .into_strict_result();
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts;
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
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
pub(crate) fn parse_xychart(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model =
        parse_xychart_model_for_render_controlled(code, meta, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled")?;
    render_model_to_compat_json(&model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_xychart_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construct_xychart_semantic_source_controlled(
            code,
            &meta.diagram_type,
            meta.effective_config.as_config(),
            EditorFactsDemand::Capture,
            control,
        )?
        .into_strict_result(),
        |source| {
            let model = source
                .model
                .into_render_model(meta.effective_config.as_config());
            (
                render_model_to_compat_json(&model, meta),
                source.editor_facts.unwrap_or_default(),
            )
        },
        CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_xychart_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<XyChartDiagramRenderModel> {
    parse_xychart_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_xychart_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<XyChartDiagramRenderModel>> {
    let outcome = construct_xychart_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    control.checkpoint()?;
    Ok(outcome
        .into_strict_result()
        .map(|source| {
            source
                .model
                .into_render_model(meta.effective_config.as_config())
        })
        .map_err(CombinedSemanticFailure::into_error))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &XyChartDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::xy_chart::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticDiagnosticKind, EditorSemanticRole};
    use crate::{MermaidEngine, ParseDiagnosticSpanKind, ParseOptions};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = MermaidEngine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
            other => other.to_string(),
        }
    }

    #[test]
    fn xychart_header_only_is_accepted() {
        let model = parse("xychart");
        assert_eq!(model["plots"], json!([]));
    }

    #[test]
    fn xychart_empty_sources_share_the_configured_default_model_and_editor_facts() {
        let engine = MermaidEngine::new();
        let configured = concat!(
            "%%{init: {\"xyChart\": {\"chartOrientation\": \"horizontal\", ",
            "\"showTitle\": false, \"xAxis\": {\"showLabel\": false}}}}%%\nxychart",
        );
        let parsed = engine
            .parse_diagram_sync(configured, ParseOptions::strict())
            .unwrap()
            .unwrap();
        for source in ["", " \t\r\n", "%% no statements\n", ";\n"] {
            let typed = parse_xychart_model_for_render(
                source,
                &crate::diagrams::source_metadata_for_test(&parsed.meta),
            )
            .unwrap();
            assert_eq!(typed.orientation, "horizontal");
            assert!(!typed.display.show_title);
            assert!(!typed.display.x_axis.show_label);
            assert!(typed.plots.is_empty());
            let projection = merman_model::families::xy_chart::compat::project(
                &typed,
                &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta),
            )
            .unwrap();
            assert_eq!(projection, parsed.model);
            assert_eq!(
                parse_xychart(
                    source,
                    &crate::diagrams::source_metadata_for_test(&parsed.meta)
                )
                .unwrap(),
                projection
            );
            #[cfg(feature = "editor-facts")]
            {
                let (combined, facts) =
                    crate::family::test_support::into_result(parse_xychart_json_and_editor_facts(
                        source,
                        &crate::diagrams::source_metadata_for_test(&parsed.meta),
                        &crate::OperationControl::new(),
                    ))
                    .unwrap();
                assert_eq!(combined, projection);
                assert!(facts.symbols.is_empty());
                assert!(facts.diagnostics.is_empty());
            }
        }
    }

    #[test]
    fn xychart_invalid_header_throws() {
        let err = parse_err("xychart-1");
        assert!(err.contains("unexpected"));
    }

    #[test]
    fn xychart_orientation_is_parsed() {
        let model = parse("xychart horizontal");
        assert_eq!(model["orientation"], json!("horizontal"));
    }

    #[test]
    fn xychart_same_line_commands_preserve_text_and_bar_annotation_controls() {
        let model = parse(concat!(
            "xychart title \"Revenue\nbar [literal]\" ",
            "x-axis \"Fiscal quarter\" [First quarter, \"bar [Q2]\"] ",
            "y-axis Revenue 0 --> 10 ",
            "bar barTitle [1 \"ignored\", 2] line lineTitle [2 \"Low\", 4 \"High\"]",
        ));
        assert_eq!(model["title"], "Revenue\nbar [literal]");
        assert_eq!(model["xAxis"]["title"], "Fiscal quarter");
        assert_eq!(
            model["xAxis"]["categories"],
            json!(["Firstquarter", "bar [Q2]"])
        );
        assert_eq!(model["yAxis"]["max"], 10.0);
        assert_eq!(model["plots"].as_array().unwrap().len(), 2);
        assert_eq!(model["plots"][0]["title"], "barTitle");
        assert_eq!(
            model["plots"][0]["data"],
            json!([["Firstquarter", 1.0], ["bar [Q2]", 2.0]])
        );
        assert!(model["plots"][0].get("pointLabels").is_none());
        assert_eq!(model["plots"][1]["pointLabels"], json!(["Low", "High"]));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_same_line_editor_facts_keep_multiline_and_structured_payload_ranges() {
        let text = concat!(
            "xychart title \"Revenue\nbar [literal]\" ",
            "x-axis \"Fiscal quarter\" [First quarter, \"bar [Q2]\"] ",
            "y-axis Revenue 0 --> 10 ",
            "bar Sales [1 \"ignored\", 2] ",
            "line Forecast [2 \"Low\nline [literal]\", 4 \"High\"] %% bar [99]\n",
            "accTitle: Revenue chart; bar [98]\n",
            "accDescr {A literal \" and bar [97]\nMore %% literal description} ",
            "title \"After block\" line [3, 4]\n",
        );
        let engine = MermaidEngine::new();
        let canonical = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(canonical.model["title"], "After block");
        assert_eq!(canonical.model["plots"].as_array().unwrap().len(), 3);
        assert_eq!(canonical.model["accTitle"], "Revenue chart; bar [98]");
        assert_eq!(
            canonical.model["accDescr"],
            "A literal \" and bar [97]\nMore %% literal description"
        );
        reset_xychart_syntax_construction_count();
        let snapshot = engine
            .parse_diagram_snapshot_with_type_sync("xychart", text)
            .unwrap()
            .unwrap();
        assert_eq!(xychart_syntax_construction_count(), 1);
        assert_eq!(snapshot.outcome().parsed_model().unwrap(), &canonical.model);
        let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("XYChart snapshot must retain the same parser's editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for payload in [
            "Revenue\nbar [literal]",
            "Fiscal quarter",
            "Sales",
            "Forecast",
            "Low\nline [literal]",
            "High",
            "Revenue chart; bar [98]",
            "A literal \" and bar [97]\nMore %% literal description",
            "After block",
        ] {
            let start = text.find(payload).unwrap();
            let selection = SourceSpan::new(start, start + payload.len());
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| { symbol.name == payload && symbol.selection == selection }),
                "missing source-backed payload {payload:?}"
            );
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload && expected.span == selection
            }));
        }
        for structured in [
            "First quarter, \"bar [Q2]\"",
            "0 --> 10",
            "1 \"ignored\", 2",
            "2 \"Low\nline [literal]\", 4 \"High\"",
            "3, 4",
        ] {
            let start = text.find(structured).unwrap();
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span == SourceSpan::new(start, start + structured.len())
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_same_line_recovery_keeps_exact_error_and_later_facts() {
        let text = "xychart x-axis [A, B] bar [1., 2] line After [3, 4]";
        let engine = MermaidEngine::new();
        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a numeric diagnostic");
        };
        let start = text.find("1.").unwrap();
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start + 2)));
        assert_eq!(diagnostic.message(), "invalid number: 1.");
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("xychart", text)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        let later_start = text.find("After").unwrap();
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "After"
                && symbol.selection == SourceSpan::new(later_start, later_start + 5)
        }));
    }

    #[test]
    fn xychart_same_line_support_preserves_upstream_rejection_boundaries() {
        for text in [
            "xychart x-axis [A, B] bar [1, 2] stray",
            "xychart x-axis [A, B bar [1, 2]",
            "xychart title \"unterminated x-axis [A, B] bar [1, 2]",
            "xychart title \"A\" \"B\" bar [1, 2]",
            // XY STR ends at every quote; it has no Langium backslash escape rule.
            "xychart title \"A\\\"B\" bar [1, 2]",
        ] {
            assert!(
                MermaidEngine::new()
                    .parse_diagram_sync(text, ParseOptions::strict())
                    .is_err(),
                "unexpectedly accepted {text:?}"
            );
        }
    }

    #[test]
    fn xychart_orientation_invalid_throws() {
        let err = parse_err("xychart abc");
        assert!(err.contains("invalid chart orientation"));
    }

    #[test]
    fn xychart_title_parses_quoted_and_unquoted() {
        let model = parse("xychart\ntitle \"This is a title\"");
        assert_eq!(model["title"], json!("This is a title"));

        let model = parse("xychart\ntitle oneLinertitle");
        assert_eq!(model["title"], json!("oneLinertitle"));
    }

    #[test]
    fn xychart_band_text_rejects_reserved_tokens_at_jison_token_boundaries() {
        for (category, token) in [
            ("bar", "bar"),
            ("LINE", "LINE"),
            ("title", "title"),
            ("horizontal", "horizontal"),
            ("horizontalFoo", "horizontal"),
            ("verticalFoo", "vertical"),
            ("vertical42", "vertical"),
            ("HORIZONTAL_name", "HORIZONTAL"),
            ("xychart-beta", "xychart-beta"),
            ("1bar", "bar"),
            ("_line", "line"),
            ("foo bar", "bar"),
            ("A-horizontal", "horizontal"),
        ] {
            let source = format!("xychart\nx-axis [{category}, B]\nline [1, 2]\n");
            let error = MermaidEngine::new()
                .parse_diagram_sync(&source, ParseOptions::strict())
                .expect_err("inclusive band state must retain reserved lexer rules");
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("expected exact XY Chart parser diagnostic");
            };
            let start = "xychart\nx-axis [".len() + category.rfind(token).unwrap();
            assert_eq!(
                diagnostic.message(),
                format!("unexpected reserved token in text: {token}")
            );
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + token.len()))
            );
        }
    }

    #[test]
    fn xychart_band_text_keeps_alpha_numeric_and_quoted_controls() {
        let model = parse(concat!(
            "xychart\n",
            "x-axis [barTitle, bar_1, bar1, foobar, b ar, 1foo, accTitle, accDescr, 01]\n",
            "line [1, 2, 3, 4, 5, 6, 7, 8, 9]\n",
        ));
        assert_eq!(
            model["xAxis"]["categories"],
            json!([
                "barTitle", "bar_1", "bar1", "foobar", "bar", "1foo", "accTitle", "accDescr", "01"
            ])
        );
        for (quoted, expected) in [
            ("\"bar\"", "bar"),
            ("\"`bar`\"", "bar"),
            ("\"horizontalFoo\"", "horizontalFoo"),
        ] {
            let source = format!("xychart\nx-axis [{quoted}, B]\nline [1, 2]\n");
            assert_eq!(
                parse(&source)["xAxis"]["categories"],
                json!([expected, "B"])
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_reserved_band_recovery_preserves_repeated_raw_label_occurrences() {
        let source = concat!(
            "xychart\n",
            "x-axis [bar, B, bar]\n",
            "line [1 \" bar \", 1 \"bar\", 1 \"bar\"]\n",
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("xychart", source)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        let reserved = source.find("bar").unwrap();
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(reserved, reserved + 3))
        }));
        let occurrences = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.detail.as_deref() == Some("xychart line data label"))
            .map(|symbol| (symbol.name.as_str(), symbol.selection))
            .collect::<Vec<_>>();
        let first = source.find("\" bar \"").unwrap() + 1;
        let second = source.find("\"bar\"").unwrap() + 1;
        let third = source.rfind("\"bar\"").unwrap() + 1;
        assert_eq!(
            occurrences,
            vec![
                (" bar ", SourceSpan::new(first, first + 5)),
                ("bar", SourceSpan::new(second, second + 3)),
                ("bar", SourceSpan::new(third, third + 3)),
            ]
        );
        let valid = source.replace("x-axis [bar, B, bar]", "x-axis [\"bar\", B, \"bar\"]");
        let model = parse(&valid);
        assert_eq!(model["xAxis"]["categories"], json!(["bar", "B", "bar"]));
        assert_eq!(model["plots"][0]["values"], json!([1.0, 1.0, 1.0]));
        assert_eq!(
            model["plots"][0]["pointLabels"],
            json!(["bar", "bar", "bar"])
        );
    }

    #[test]
    fn xychart_parses_axis_band_and_range_and_plots() {
        let model = parse(
            r#"xychart horizontal
title "Basic xychart"
x-axis "this is x axis" [category1, "category 2", category3]
y-axis yaxisText 10 --> 150
bar barTitle1 [23, 45, 56.6]
line lineTitle1 [11, 45.5, 67, 23]
"#,
        );
        assert_eq!(model["orientation"], json!("horizontal"));
        assert_eq!(model["xAxis"]["type"], json!("band"));
        assert_eq!(
            model["xAxis"]["categories"],
            json!(["category1", "category 2", "category3"])
        );
        assert_eq!(model["yAxis"]["min"], json!(10.0));
        assert_eq!(model["yAxis"]["max"], json!(150.0));
        assert_eq!(model["plots"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn xychart_line_points_accept_optional_labels() {
        let model = parse(
            r#"xychart
x-axis [A, B, C]
y-axis 0 --> 10
line [1 "low", 5, 9 "high"]
bar [2 "ignored", 4, 6]
"#,
        );

        assert_eq!(model["plots"][0]["type"], json!("line"));
        assert_eq!(model["plots"][0]["values"], json!([1.0, 5.0, 9.0]));
        assert_eq!(model["plots"][0]["pointLabels"], json!(["low", "", "high"]));
        assert!(model["plots"][1].get("pointLabels").is_none());
    }

    #[test]
    fn xychart_equal_linear_axis_bounds_map_every_point_by_data_index() {
        let model = parse(
            r#"xychart
x-axis 5 --> 5
line [10, 20, 30]
"#,
        );

        assert_eq!(
            model["plots"][0]["data"],
            json!([["5", 10.0], ["5", 20.0], ["5", 30.0]])
        );
    }

    #[test]
    fn xychart_single_point_on_linear_axis_remains_mapped_to_minimum() {
        let model = parse(
            r#"xychart
x-axis 5 --> 9
line [10]
"#,
        );

        assert_eq!(model["plots"][0]["data"], json!([["5", 10.0]]));
    }

    #[test]
    fn xychart_band_axis_truncates_points_but_preserves_labels_before_auto_y_range() {
        let model = parse(
            r#"xychart
x-axis [Q1, Q2]
line [10 "first", 50 "second", 999 "orphan", 800 "ignored"]
"#,
        );

        assert_eq!(model["yAxis"]["min"], json!(10.0));
        assert_eq!(model["yAxis"]["max"], json!(50.0));
        assert_eq!(model["plots"][0]["values"], json!([10.0, 50.0]));
        assert_eq!(
            model["plots"][0]["data"],
            json!([["Q1", 10.0], ["Q2", 50.0]])
        );
        assert_eq!(
            model["plots"][0]["pointLabels"],
            json!(["first", "second", "orphan", "ignored"])
        );
    }

    #[test]
    fn xychart_unquoted_multibyte_categories_do_not_panic() {
        let model = parse(
            r#"xychart
x-axis [東京, 大阪]
y-axis "値" 0 --> 10
bar [1, 2]
"#,
        );

        assert_eq!(model["xAxis"]["categories"], json!(["東京", "大阪"]));
    }

    #[test]
    fn xychart_plot_requires_nonempty_data() {
        let err = parse_err("xychart\nline \"t\" [ ]");
        assert!(err.contains("empty"));
        let err = parse_err("xychart\nline \"t\"");
        assert!(err.contains("missing") || err.contains("requires"));
    }

    #[test]
    fn xychart_accepts_line_without_whitespace_after_keyword() {
        let model = parse("xychart\nline[1,2,3]");
        assert_eq!(model["plots"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn xychart_comment_after_plot_does_not_merge_next_statement() {
        let model = parse("xychart\nbar [1] %% keep next line separate\nline [2]\n");
        assert_eq!(model["plots"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn xychart_acc_title_requires_colon() {
        let err = parse_err("xychart\naccTitle hello");
        assert!(err.contains("accTitle"));
    }

    #[test]
    fn xychart_rejects_invalid_x_axis_range_like_upstream() {
        let err = parse_err("xychart\nx-axis xAxisName aaa --> 33\n");
        assert!(err.contains("invalid"));
    }

    #[test]
    fn xychart_rejects_unbalanced_x_axis_brackets_like_upstream() {
        let err = parse_err("xychart\nx-axis xAxisName [ \"cat1\" [ cat2a ]\n");
        assert!(err.contains("unbalanced"));
        let err = parse_err("xychart\nx-axis xAxisName [ \"cat1\" , cat2a ] ]\n");
        assert!(err.contains("unexpected") || err.contains("unbalanced"));
    }

    #[test]
    fn xychart_rejects_invalid_y_axis_range_like_upstream() {
        let err = parse_err("xychart\ny-axis yAxisName 45.5 --> abc\n");
        assert!(err.contains("expected number") || err.contains("invalid"));
    }

    #[test]
    fn xychart_rejects_y_axis_band_data_like_upstream() {
        let err = parse_err("xychart\ny-axis yAxisName [ 45.3, 33 ]\n");
        assert!(err.contains("does not support") || err.contains("band"));
    }

    #[test]
    fn xychart_rejects_unbalanced_plot_brackets_like_upstream() {
        let err = parse_err("xychart\nline \"t\" [  +23 [ -45  , 56.6 ]\n");
        assert!(err.contains("unbalanced") || err.contains("expected"));
        let err = parse_err("xychart\nbar \"t\" [  +23 , -45  ] 56.6 ]\n");
        assert!(err.contains("unexpected") || err.contains("unbalanced"));
    }

    #[test]
    fn xychart_rejects_invalid_plot_commas_and_numbers_like_upstream() {
        let err = parse_err("xychart\nline \"t\" [  +23 ,  , -45  , 56.6 ]\n");
        assert!(err.contains("empty") || err.contains("invalid"));
        let err = parse_err("xychart\nbar \"t\" [  +23 , -4aa5  , 56.6 ]\n");
        assert!(err.contains("invalid number"));
    }

    #[test]
    fn xychart_invalid_plot_number_reports_exact_token_span() {
        let text = "xychart\nbar \"t\" [  +23 , -4aa5  , 56.6 ]\n";
        let engine = MermaidEngine::new();
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = err else {
            panic!("expected xychart parse error");
        };

        let token_start = text.find("-4aa5").unwrap();
        assert_eq!(diagnostic.message(), "invalid number: -4aa5");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(token_start, token_start + "-4aa5".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn xychart_rejects_trailing_decimal_point_like_pinned_jison() {
        let text = "xychart\nbar [1.]\n";
        let engine = MermaidEngine::new();
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = err else {
            panic!("expected xychart parse error");
        };
        let token_start = text.find("1.").expect("invalid number token");

        assert_eq!(diagnostic.message(), "invalid number: 1.");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(token_start, token_start + "1.".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn xychart_entrypoints_and_combined_projection_construct_once() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "xychart horizontal\n",
            "title \"Revenue; Growth\"; accTitle: Revenue chart\n",
            "accDescr {\n",
            "  Quarterly %% literal\n",
            "  projection\n",
            "}\n",
            "x-axis \"Fiscal quarter\" [Q1, \"Q 2\"]\n",
            "y-axis Revenue 0 --> 100\n",
            "line \"Forecast line\" [23 \"Low\", 45 \"High\"]\n",
            "bar \"Actual bars\" [20, 40]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("standalone XYChart JSON parse succeeds")
            .expect("standalone XYChart JSON parse returns a diagram");
        reset_xychart_syntax_construction_count();
        let standalone_json = parse_xychart(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("XYChart JSON projection succeeds");
        assert_eq!(xychart_syntax_construction_count(), 1);

        reset_xychart_syntax_construction_count();
        let typed = parse_xychart_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("XYChart typed projection succeeds");
        assert_eq!(xychart_syntax_construction_count(), 1);

        reset_xychart_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let (combined_json, combined_editor) =
            crate::family::test_support::into_result(parse_xychart_json_and_editor_facts(
                text,
                &crate::diagrams::source_metadata_for_test(&parsed.meta),
                &crate::OperationControl::new(),
            ))
            .expect("XYChart combined projection succeeds");
        #[cfg(not(feature = "editor-facts"))]
        let combined_json = parse_xychart(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .unwrap();
        assert_eq!(xychart_syntax_construction_count(), 1);
        assert_eq!(
            combined_json, standalone_json,
            "XYChart combined projection drift"
        );
        #[cfg(feature = "editor-facts")]
        assert!(!combined_editor.symbols.is_empty());

        let typed_projection = merman_model::families::xy_chart::compat::project(
            &typed,
            &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta),
        )
        .expect("XYChart typed compatibility projection succeeds");
        assert_eq!(
            typed_projection, combined_json,
            "XYChart typed projection drift"
        );
    }

    #[test]
    fn xychart_typed_projection_matches_complete_compat_json() {
        let text = "xychart\nx-axis [Q1, Q2]\nline [1, 2]\n";
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("XYChart compatibility parse succeeds")
            .expect("XYChart compatibility parse returns a diagram");
        let typed = parse_xychart_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("XYChart typed parse succeeds");

        let projection = merman_model::families::xy_chart::compat::project(
            &typed,
            &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta),
        )
        .expect("XYChart compatibility projection succeeds");

        assert_eq!(projection, parsed.model);
        assert_eq!(projection["type"], json!("xychart"));
        assert!(projection["config"].is_object());
        assert_eq!(projection["title"], Value::Null);
        assert_eq!(projection["accTitle"], Value::Null);
        assert_eq!(projection["accDescr"], Value::Null);
        assert_eq!(projection["plots"][0]["title"], json!(""));
    }

    #[test]
    fn xychart_plot_titles_are_sanitized_and_projected_for_legends() {
        let text = "xychart\nx-axis [Q1, Q2]\nline \" avg \" [1, 2]\nbar \" p95 \" [2, 3]\n";
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("XYChart compatibility parse succeeds")
            .expect("XYChart compatibility parse returns a diagram");
        let typed = parse_xychart_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("XYChart typed parse succeeds");

        assert_eq!(typed.plots[0].title.as_deref(), Some("avg"));
        assert_eq!(typed.plots[1].title.as_deref(), Some("p95"));
        assert_eq!(parsed.model["plots"][0]["title"], json!("avg"));
        assert_eq!(parsed.model["plots"][1]["title"], json!("p95"));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_editor_projection_preserves_multiline_and_nested_payload_spans() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "xychart\n",
            "title \"Revenue plan\"\n",
            "accTitle: Revenue chart\n",
            "accDescr {\n",
            "  Quarterly %% literal\n",
            "  projection\n",
            "}\n",
            "x-axis \"Fiscal quarter\" [Q1, \"Q 2\"]\n",
            "y-axis Revenue 0 --> 100\n",
            "line \"Forecast line\" [23 \"Low\", 45 \"High\"]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("XYChart parse succeeds")
            .expect("XYChart model");
        let facts = crate::family::test_support::editor_facts_from_frozen(
            parse_xychart_json_and_editor_facts,
            text,
            &parsed.meta,
        );

        let assert_payload = |name: &str, detail: &str, start: usize| {
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.name == name
                        && symbol.detail.as_deref() == Some(detail)
                        && symbol.selection.start == start
                })
                .unwrap_or_else(|| panic!("missing {detail} payload {name:?} at {start}"));
            assert_eq!(symbol.role, EditorSemanticRole::Payload);
            assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span == symbol.selection
            }));
        };

        for (name, detail, start) in [
            (
                "Revenue plan",
                "xychart title",
                text.find("Revenue plan").expect("title source"),
            ),
            (
                "Revenue chart",
                "xychart accessibility title",
                text.find("Revenue chart")
                    .expect("accessibility title source"),
            ),
            (
                "Quarterly %% literal\n  projection",
                "xychart accessibility description",
                text.find("Quarterly %% literal")
                    .expect("accessibility description source"),
            ),
            (
                "Fiscal quarter",
                "xychart x-axis",
                text.find("Fiscal quarter").expect("x-axis title source"),
            ),
            (
                "Revenue",
                "xychart y-axis",
                text.find("y-axis Revenue").expect("y-axis source") + "y-axis ".len(),
            ),
            (
                "Forecast line",
                "xychart line",
                text.find("Forecast line").expect("plot title source"),
            ),
            (
                "Low",
                "xychart line data label",
                text.find("Low").expect("low label source"),
            ),
            (
                "High",
                "xychart line data label",
                text.find("High").expect("high label source"),
            ),
        ] {
            assert_payload(name, detail, start);
        }

        for structured in ["Q1, \"Q 2\"", "0 --> 100", "23 \"Low\", 45 \"High\""] {
            let start = text.find(structured).expect("structured payload source");
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span == SourceSpan::new(start, start + structured.len())
            }));
            assert!(!facts.symbols.iter().any(|symbol| {
                symbol.name == structured && symbol.role == EditorSemanticRole::Payload
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_malformed_plot_recovers_prior_parser_facts() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "xychart\n",
            "title Revenue\n",
            "x-axis [Q1, Q2]\n",
            "bar [23, -4aa5]\n",
        );
        let invalid_start = text.find("-4aa5").expect("invalid number token");
        reset_xychart_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("xychart", text)
            .expect("XYChart editor recovery succeeds")
            .expect("XYChart editor facts are available");

        assert_eq!(xychart_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Revenue" && symbol.role == EditorSemanticRole::Payload
        }));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span
                    == Some(SourceSpan::new(
                        invalid_start,
                        invalid_start + "-4aa5".len(),
                    ))
        }));
    }

    #[test]
    fn xychart_recovery_keeps_confirmed_prefix_and_safe_later_statements() {
        let text = concat!(
            "xychart\n",
            "title Before\n",
            "bar [1, -4aa5]\n",
            "y-axis [0, 10]\n",
            "line \"After 后来\" [2 \"后来\"]\n",
        );
        let invalid_start = text.find("-4aa5").expect("invalid token");

        let strict = MermaidEngine::new()
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("strict XYChart parse must return its first error");
        let Error::DiagramParse { diagnostic, .. } = strict else {
            panic!("expected diagram parse error");
        };
        assert_eq!(diagnostic.message(), "invalid number: -4aa5");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(
                invalid_start,
                invalid_start + "-4aa5".len()
            ))
        );

        #[cfg(feature = "editor-facts")]
        {
            reset_xychart_syntax_construction_count();
            let facts = MermaidEngine::new()
                .parse_editor_semantic_facts_with_type_sync("xychart", text)
                .expect("XYChart editor recovery")
                .expect("XYChart recovered facts");
            assert_eq!(xychart_syntax_construction_count(), 1);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "After 后来"
                    && symbol.detail.as_deref() == Some("xychart line")
                    && symbol.role == EditorSemanticRole::Payload
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_unterminated_eof_string_reports_insertion_point() {
        let text = "xychart\ntitle \"未完成 🤓";
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("xychart", text)
            .expect("XYChart editor recovery")
            .expect("XYChart recovered facts");

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(text.len(), text.len()))
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_missing_bracket_recovers_at_the_next_statement_boundary() {
        let text = "xychart\nbar [1, 2\nline [3]\n";
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("xychart", text)
            .expect("XYChart editor recovery")
            .expect("XYChart recovered facts");

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        let later_value_start = text.rfind('3').expect("later line value");
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(later_value_start, later_value_start + 1)
        }));
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn canonical_empty_capture_keeps_an_available_empty_journal() {
        reset_xychart_syntax_construction_count();
        let parsed = parse_xychart_semantic_model_controlled(
            "",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap();
        assert!(parsed.model.is_ok());
        assert_eq!(xychart_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed
                .editor_facts
                .expect("empty input still provides requested facts");
            assert!(facts.symbols.is_empty());
            assert!(facts.diagnostics.is_empty());
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Complete
            );
        }
    }

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "xychart".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn semantic_orientation_preserves_upstream_literal_setter_behavior() {
        let mut metadata = metadata();
        metadata.semantic_config = MermaidConfig::from_value(serde_json::json!({
            "xyChart": {"chartOrientation": "horizontal"}
        }));
        for (orientation, expected) in [
            ("horizontal", "horizontal"),
            ("HORIZONTAL", "vertical"),
            ("Horizontal", "vertical"),
            ("vertical", "vertical"),
            ("VERTICAL", "vertical"),
        ] {
            let source = format!("xychart {orientation}\nx-axis [A, B]\nbar [1, 2]");
            let model = parse_xychart_semantic_model_controlled(
                &source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(
                model.orientation, expected,
                "orientation token {orientation}"
            );
        }
    }

    #[test]
    fn semantic_same_line_header_axis_and_plot_share_one_construction() {
        for header in ["xychart", "xychart-beta"] {
            reset_xychart_syntax_construction_count();
            let source = format!("{header} x-axis [A, B] bar [1, 2]");
            let model = parse_xychart_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(xychart_syntax_construction_count(), 1);
            assert_eq!(model.orientation, "vertical");
            assert_eq!(
                model.x_axis,
                XyChartAxisModel::Band {
                    title: String::new(),
                    categories: vec!["A".to_string(), "B".to_string()],
                }
            );
            assert_eq!(model.plots.len(), 1);
            assert_eq!(model.plots[0].plot_type, XyChartPlotType::Bar);
            assert_eq!(model.plots[0].values, vec![1.0, 2.0]);
            assert_eq!(
                model.plots[0].data,
                vec![("A".to_string(), Some(1.0)), ("B".to_string(), Some(2.0)),]
            );
        }
    }

    #[test]
    fn semantic_same_line_lexing_can_cancel_inside_long_strings_and_comments() {
        for source in [
            format!("xychart title \"{}\" bar [1]", "a".repeat(32768)),
            format!("xychart x-axis [A] %%{}\nbar [1]", "a".repeat(32768)),
            format!("xychart x-axis [{}bar, B] line [1, 2]", "A ".repeat(32768)),
        ] {
            let control = crate::OperationControl::new();
            control.cancel_after_checkpoints(5);
            assert!(matches!(
                parse_xychart_semantic_model_controlled(
                    &source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &control,
                ),
                Err(crate::OperationCancelled { .. })
            ));
        }
    }

    #[test]
    fn canonical_terminal_display_policy_reads_all_admitted_visibility_flags() {
        let policy = XyChartDisplayPolicy::from_config(&MermaidConfig::from_value(
            serde_json::json!({"xyChart": {
                "showTitle": false,
                "showDataLabel": true,
                "showDataLabelOutsideBar": true,
                "xAxis": {
                    "showLabel": false, "showTitle": false,
                    "showTick": false, "showAxisLine": false
                },
                "yAxis": {
                    "showLabel": false, "showTitle": false,
                    "showTick": false, "showAxisLine": false
                }
            }}),
        ));
        assert_eq!(
            policy,
            XyChartDisplayPolicy {
                show_title: false,
                show_data_label: true,
                show_data_label_outside_bar: true,
                x_axis: XyChartAxisDisplayPolicy {
                    show_label: false,
                    show_title: false,
                    show_tick: false,
                    show_axis_line: false,
                },
                y_axis: XyChartAxisDisplayPolicy {
                    show_label: false,
                    show_title: false,
                    show_tick: false,
                    show_axis_line: false,
                },
            }
        );
        assert_eq!(
            XyChartDisplayPolicy::from_config(&MermaidConfig::empty_object()),
            XyChartDisplayPolicy::default()
        );
    }

    #[test]
    fn semantic_axes_and_series_construct_once_without_display_or_editor_work() {
        reset_xychart_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_xychart_semantic_model_controlled(
            "xychart horizontal\ntitle Revenue\nx-axis Quarter [Q1, Q2]\ny-axis Revenue 0 --> 100\nline Forecast [23 \"Low\", 45 \"High\"]\n",
            &metadata(), #[cfg(feature = "editor-facts")]
EditorFactsDemand::Omit, &crate::OperationControl::new(),
        ).unwrap().model.unwrap();
        assert_eq!(model.orientation, "horizontal");
        assert_eq!(model.title.as_deref(), Some("Revenue"));
        assert_eq!(
            model.x_axis,
            XyChartAxisModel::Band {
                title: "Quarter".to_string(),
                categories: vec!["Q1".to_string(), "Q2".to_string()]
            }
        );
        assert_eq!(
            model.y_axis,
            XyChartAxisModel::Linear {
                title: "Revenue".to_string(),
                min: Some(0.0),
                max: Some(100.0)
            }
        );
        assert_eq!(model.plots[0].plot_type, XyChartPlotType::Line);
        assert_eq!(model.plots[0].values, vec![23.0, 45.0]);
        assert_eq!(
            model.plots[0].data,
            vec![
                ("Q1".to_string(), Some(23.0)),
                ("Q2".to_string(), Some(45.0))
            ]
        );
        assert_eq!(model.plots[0].point_labels, vec!["Low", "High"]);
        assert_eq!(xychart_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        assert!(
            serde_json::to_value(model)
                .unwrap()
                .get("display")
                .is_none()
        );
    }

    #[test]
    fn semantic_config_sanitizes_text_without_materializing_display_policy() {
        let mut metadata = metadata();
        metadata.semantic_config = MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "strict", "xyChart": {"chartOrientation": "horizontal", "showTitle": false},
            "themeVariables": {"primaryColor": "not-a-color"}
        }));
        let model = parse_xychart_semantic_model_controlled(
            "xychart\ntitle \"<script>bad()</script>Revenue\"\nbar [1, 2]",
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.orientation, "horizontal");
        assert_eq!(model.title.as_deref(), Some("Revenue"));
        assert_eq!(model.plots[0].values, vec![1.0, 2.0]);
    }

    #[test]
    fn semantic_invalid_number_retains_exact_error_and_omits_editor_recovery() {
        let input = "xychart\nbar [1.]\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_xychart_semantic_model_controlled(
            input,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected exact parser diagnostic")
        };
        let offset = input.find("1.").unwrap();
        assert_eq!(diagnostic.message(), "invalid number: 1.");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(offset, offset + 2)));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full = SourcePresentationMetadata {
                diagram_type: "xychart".to_string(),
                config: MermaidConfig::empty_object(),
                effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                title: None,
            };
            let full_error = parse_xychart_model_for_render_controlled(
                input,
                &full,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap_err();
            assert_eq!(error.to_string(), full_error.to_string());
        }
    }

    #[test]
    fn semantic_cancellation_stops_before_construction() {
        reset_xychart_syntax_construction_count();
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_xychart_semantic_model_controlled(
                "xychart\nbar [1]",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
        assert_eq!(xychart_syntax_construction_count(), 0);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn typed_and_json_paths_skip_editor_projection() {
        let full = SourcePresentationMetadata {
            diagram_type: "xychart".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_xychart_syntax_construction_count();
        parse_xychart_model_for_render_controlled(
            "xychart\nbar [1, 2]",
            &full,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(xychart_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        reset_xychart_syntax_construction_count();
        parse_xychart("xychart\nbar [1, 2]", &full).unwrap();
        assert_eq!(xychart_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
}
