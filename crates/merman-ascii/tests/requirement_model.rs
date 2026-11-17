#![cfg(feature = "diagram-requirement")]

mod support;

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiGraphPresentation, AsciiOutput,
    AsciiOutputOutcome, AsciiProjection, AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId,
    AsciiResourcePolicy, AsciiSourceContext, AsciiTrimPolicy, AsciiViewportPolicy, Lossiness,
    OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::requirement::RequirementDiagramModel;
use merman_model::{
    MermaidConfig, OperationControl, ParsedSemanticDiagram, SemanticDiagramModel,
    runtime::RuntimePolicy,
};

const SMALL: &str = "requirementDiagram\nrequirement reliable {\nid: R-1\ntext: Handle requests\nrisk: high\nverifymethod: test\n}\nelement suite {\ntype: test suite\ndocRef: tests/api\n}\nsuite - verifies -> reliable\n";
const DENSE: &str = include_str!(
    "../../../fixtures/requirement/upstream_docs_requirementdiagram_larger_example_010.mmd"
);
const TYPICAL: &str = "requirementDiagram\nrequirement service {\nid: R-1\ntext: Serve requests\n}\nfunctionalRequirement audit {\nid: R-2\ntext: Record results\n}\nelement suite {\ntype: test suite\n}\nservice - contains -> audit\nsuite - verifies -> audit\n";

fn parse(source: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("Requirement parses")
}

fn model(source: &str) -> RequirementDiagramModel {
    let SemanticDiagramModel::Requirement(model) = parse(source).model else {
        panic!("Requirement model");
    };
    model
}

fn report_with(
    parsed: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
    control: &OperationControl,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?
        .render_parsed_report(parsed, viewport, control, &context, resources)
}

fn report(source: &str, options: AsciiRenderOptions, width: usize) -> AsciiOutput {
    report_with(
        &parse(source),
        options,
        AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap()
}

fn borrowed(model: &RequirementDiagramModel, width: usize) -> AsciiOutput {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let config = MermaidConfig::empty_object();
    AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_model_report(
            &AsciiDiagram::Requirement(model),
            AsciiSourceContext {
                diagram_type: "requirement",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn assert_records(text: &str, expected: &[&str]) {
    let compact = text.split_whitespace().collect::<String>();
    for value in expected {
        assert!(
            compact.contains(&value.split_whitespace().collect::<String>()),
            "missing {value}:\n{text}"
        );
    }
}

fn assert_primary(output: &AsciiOutput, projection: AsciiProjection, width: usize) {
    assert_eq!(output.projection, projection, "{}", output.text);
    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
    assert_eq!(output.primary_extent, Some(output.emitted_extent));
    assert!(!output.fallback.attempted);
    assert_eq!(output.lossiness, Lossiness::None);
    assert!(output.emitted_extent.width <= width, "{}", output.text);
    assert_eq!(
        support::terminal_extent_with_profile(&output.text, output.width_profile),
        (output.emitted_extent.width, output.emitted_extent.height)
    );
}

#[test]
fn shared_endpoint_admission_at_80_100_120_preserves_all_typed_fields() {
    for source in [SMALL, TYPICAL, DENSE] {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for width in [80, 100, 120] {
                let output = report(source, options, width);
                assert_primary(&output, AsciiProjection::Diagrammatic, width);
                assert_records(&output.text, &["direction: \"TB\"", "Requirement records"]);
                if source == SMALL {
                    assert_records(
                        &output.text,
                        &[
                            "R1 [\"Requirement\"] name=\"reliable\" id=\"R-1\"",
                            "text=\"Handle requests\" risk=\"High\" verifyMethod=\"Test\"",
                            "N1 [element] name=\"suite\" type=\"test suite\" docRef=\"tests/api\"",
                            "E1 [\"verifies\"] source=[N1] name=\"suite\" target=[R1] name=\"reliable\"",
                        ],
                    );
                } else if source == DENSE {
                    let topology = output.text.split("Requirement records").next().unwrap();
                    for name in ["test_req2", "test_req5"] {
                        assert_eq!(
                            topology.matches(name).count(),
                            1,
                            "shared endpoint {name}:\n{topology}"
                        );
                    }
                    for kind in [
                        "contains",
                        "copies",
                        "derives",
                        "satisfies",
                        "verifies",
                        "refines",
                        "traces",
                    ] {
                        assert!(
                            topology.contains(kind),
                            "missing topology relation {kind}:\n{topology}"
                        );
                    }
                    assert_records(
                        &output.text,
                        &[
                            "E8 [\"copies\"] source=[N2] name=\"test_entity2\" target=[R1] name=\"test_req\"",
                            "id=\"1.2.2\"",
                            "docRef=\"github.com/all_the_tests\"",
                        ],
                    );
                    assert_records(
                        &output.text,
                        &[
                            "Functional Requirement",
                            "Performance Requirement",
                            "Interface Requirement",
                            "Physical Requirement",
                            "Design Constraint",
                            "Demonstration",
                            "Inspection",
                            "Medium",
                            "Low",
                        ],
                    );
                }
            }
        }
    }
}

#[test]
fn all_directions_keep_canonical_relations_and_one_placement_per_declaration() {
    for (direction, before) in [("TB", true), ("BT", false), ("LR", true), ("RL", false)] {
        let source = SMALL.replacen(
            "requirementDiagram",
            &format!("requirementDiagram\ndirection {direction}"),
            1,
        );
        let output = report(&source, AsciiRenderOptions::ascii(), 120);
        assert_primary(&output, AsciiProjection::Diagrammatic, 120);
        let topology = output.text.split("Requirement records").next().unwrap();
        let position = |value: &str| {
            topology
                .lines()
                .enumerate()
                .find_map(|(y, line)| line.find(value).map(|x| (x, y)))
                .unwrap()
        };
        let (source, target) = (position("suite"), position("reliable"));
        if matches!(direction, "LR" | "RL") {
            assert_eq!(source.0 < target.0, before, "{topology}");
        } else {
            assert_eq!(source.1 < target.1, before, "{topology}");
        }
        assert_records(
            &output.text,
            &["E1 [\"verifies\"] source=[N1] name=\"suite\" target=[R1] name=\"reliable\""],
        );
    }
}

#[test]
fn parallel_relations_self_loops_and_reverse_syntax_preserve_every_occurrence() {
    let source = format!(
        "{SMALL}suite - verifies -> reliable\nreliable <- satisfies - suite\nreliable - traces -> reliable\nreliable - derives -> suite\n"
    );
    let output = report(
        &source,
        AsciiRenderOptions::ascii().with_graph_presentation(AsciiGraphPresentation::Adaptive),
        80,
    );
    assert!(!output.emitted_overflowed);
    assert_records(
        &output.text,
        &[
            "E1 [\"verifies\"] source=[N1] name=\"suite\" target=[R1] name=\"reliable\"",
            "E2 [\"verifies\"] source=[N1] name=\"suite\" target=[R1] name=\"reliable\"",
            "E3 [\"satisfies\"] source=[N1] name=\"suite\" target=[R1] name=\"reliable\"",
            "E4 [\"traces\"] source=[R1] name=\"reliable\" target=[R1] name=\"reliable\"",
            "E5 [\"derives\"] source=[R1] name=\"reliable\" target=[N1] name=\"suite\"",
        ],
    );
}

#[test]
fn every_tracked_requirement_fixture_has_complete_bounded_adaptive_output() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/requirement");
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mmd"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty());
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let output = report_with(
            &parse(&source),
            AsciiRenderOptions::unicode().with_graph_presentation(AsciiGraphPresentation::Adaptive),
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(output.emitted_extent.width <= 80, "{}", path.display());
        let authored = model(&source);
        let compact = output.text.split_whitespace().collect::<String>();
        for (index, _) in authored.relationships.iter().enumerate() {
            assert!(
                compact.contains(&format!("E{}[", index + 1)),
                "{}: missing edge occurrence {}",
                path.display(),
                index + 1
            );
        }
    }
}

#[test]
fn unresolved_or_overlapping_names_have_complete_honest_text_primaries() {
    for source in [
        "requirementDiagram\nmissing - satisfies -> absent\n",
        "requirementDiagram\nrequirement shared {\ntext: Requirement text\n}\nelement shared {\ntype: Element type\n}\nshared - traces -> shared\n",
    ] {
        let output = report(source, AsciiRenderOptions::ascii(), 80);
        assert_primary(&output, AsciiProjection::StructuredText, 80);
        if source.contains("missing") {
            assert_records(
                &output.text,
                &[
                    "source=[] (unresolved) name=\"missing\"",
                    "target=[] (unresolved) name=\"absent\"",
                ],
            );
        } else {
            assert_records(
                &output.text,
                &[
                    "Requirement text",
                    "Element type",
                    "source=[R1,N1] (ambiguous)",
                    "target=[R1,N1] (ambiguous)",
                ],
            );
        }
    }
}

#[test]
fn unknown_borrowed_semantics_and_duplicates_remain_inspectable() {
    let mut data = model(SMALL);
    data.relationships[0].rel_type = "custom relation".to_string();
    let output = borrowed(&data, 80);
    assert_primary(&output, AsciiProjection::StructuredText, 80);
    assert_records(
        &output.text,
        &["custom relation", "source=[N1]", "target=[R1]"],
    );
    data.relationships[0].rel_type = "verifies".to_string();
    data.requirements.push(data.requirements[0].clone());
    let output = borrowed(&data, 80);
    assert_primary(&output, AsciiProjection::StructuredText, 80);
    assert_records(
        &output.text,
        &[
            "R2 [\"Requirement\"]",
            "Identity diagnostic:",
            "target=[R1,R2] (ambiguous)",
        ],
    );
    data.direction = "diagonal".to_string();
    assert_records(&borrowed(&data, 80).text, &["direction: \"diagonal\""]);
}

#[test]
fn width_recovery_keeps_complete_records_and_allow_error_are_measured() {
    let parsed = parse(SMALL);
    let primary = report_with(
        &parsed,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap();
    for width in [20, 40] {
        let recovered = report(SMALL, AsciiRenderOptions::ascii(), width);
        assert_eq!(recovered.projection, AsciiProjection::StructuredText);
        assert!(recovered.fallback.attempted);
        assert!(recovered.emitted_extent.width <= width);
        assert_records(
            &recovered.text,
            &["reliable", "Handle requests", "tests/api", "verifies"],
        );
        let allowed = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Allow),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
        assert_eq!(allowed.text, primary.text);
        let error = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Error),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, AsciiError::WidthOverflow { actual_width, .. } if actual_width == primary.emitted_extent.width)
        );
    }
}

#[test]
fn unicode_cjk_and_unsafe_fields_share_safe_terminal_measurements() {
    let mut data = model(SMALL);
    data.requirements[0].name = "校验请求".to_string();
    data.requirements[0].text = "  请求 👨‍👩‍👧‍👦 e\u{301}\n\u{1b}[31m \"raw\"  ".to_string();
    data.relationships[0].dst = data.requirements[0].name.clone();
    let config = MermaidConfig::empty_object();
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        let output =
            AsciiRenderer::new(AsciiRenderOptions::unicode().with_terminal_width_profile(profile))
                .unwrap()
                .render_model_report(
                    &AsciiDiagram::Requirement(&data),
                    AsciiSourceContext {
                        diagram_type: "requirement",
                        config: &config,
                        semantic_config: &config,
                        title: None,
                    },
                    AsciiViewportPolicy::with_max_width(80)
                        .trim(AsciiTrimPolicy::TrimTrailingSpaces),
                    &OperationControl::new(),
                    &context,
                    AsciiResourcePolicy::default(),
                )
                .unwrap();
        assert_primary(&output, AsciiProjection::Diagrammatic, 80);
        assert!(!output.text.contains('\u{1b}'));
        assert!(output.text.contains("校验请求"));
        assert_records(&output.text, &[r#"\"raw\""#, r#"\u{1B}[31m"#]);
        assert!(!output.trimmed);
    }
}

#[test]
fn complete_primaries_use_all_encodings_and_rich_style_records() {
    let source = format!(
        "---\ntitle: Traceability\nconfig:\n  theme: forest\n---\n{SMALL}accTitle: Accessible\naccDescr: Trace relations\nclassDef important fill:red,color:white\nclass reliable important\nstyle suite stroke:blue\n"
    );
    for color in [
        AsciiColorMode::Plain,
        AsciiColorMode::Ansi16,
        AsciiColorMode::Ansi256,
        AsciiColorMode::TrueColor,
        AsciiColorMode::Html,
    ] {
        let output = report_with(
            &parse(&source),
            AsciiRenderOptions::ascii().with_color_mode(color),
            AsciiViewportPolicy::with_max_width(120).overflow(OverflowPolicy::Allow),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
        assert_eq!(output.projection, AsciiProjection::Diagrammatic);
        assert!(!output.fallback.attempted);
        assert!(output.text.contains("Traceability"));
        assert!(output.text.contains("Accessible"));
        assert!(output.text.contains("important"));
        assert!(output.text.contains("stroke:blue"));
    }
}

#[test]
fn resource_failures_and_cancellation_never_become_reading_fallbacks() {
    let parsed = parse(DENSE);
    for limit in [
        AsciiResourceLimitId::MaxGridCells,
        AsciiResourceLimitId::MaxOutputBytes,
        AsciiResourceLimitId::MaxLayoutWorkUnits,
    ] {
        let policy = AsciiResourcePolicy::default().with_limit(limit, 1).unwrap();
        let error = report_with(
            &parsed,
            AsciiRenderOptions::ascii().with_graph_presentation(AsciiGraphPresentation::Adaptive),
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            policy,
        )
        .unwrap_err();
        assert!(
            matches!(error, AsciiError::ResourceLimitExceeded(_)),
            "{error}"
        );
    }
    let control = OperationControl::new();
    control.cancel();
    let error = report_with(
        &parsed,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
        &control,
        AsciiResourcePolicy::default(),
    )
    .unwrap_err();
    assert!(matches!(error, AsciiError::Cancelled(_)), "{error}");
}

#[test]
fn combined_primary_and_record_reflow_admit_retained_cells_and_bytes_once() {
    for source in [SMALL, "requirementDiagram\nmissing - verifies -> absent\n"] {
        let parsed = parse(source);
        let primary = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            AsciiResourcePolicy::unbounded(),
        )
        .unwrap();
        let primary_cells = primary.text.lines().map(str::len).sum::<usize>();
        for viewport in [
            AsciiViewportPolicy::unrestricted(),
            AsciiViewportPolicy::with_max_width(24).overflow(OverflowPolicy::Fallback),
        ] {
            let options = AsciiRenderOptions::ascii();
            let natural = report_with(
                &parsed,
                options,
                viewport,
                &OperationControl::new(),
                AsciiResourcePolicy::unbounded(),
            )
            .unwrap();
            let cells = primary_cells.max(natural.text.lines().map(str::len).sum::<usize>());
            // Each attempted document must fit independently. Selecting the replacement must
            // not add the discarded primary to the final retained-cell admission.
            for limit in [cells, cells - 1] {
                let policy = AsciiResourcePolicy::unbounded()
                    .with_limit(AsciiResourceLimitId::MaxDocumentCells, limit)
                    .unwrap();
                let result =
                    report_with(&parsed, options, viewport, &OperationControl::new(), policy);
                if limit == cells {
                    assert_eq!(result.unwrap().text, natural.text);
                } else {
                    assert!(
                        matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxDocumentCells)
                    );
                }
            }
            if viewport == AsciiViewportPolicy::unrestricted() {
                for limit in [natural.text.len(), natural.text.len() - 1] {
                    let policy = AsciiResourcePolicy::unbounded()
                        .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                        .unwrap();
                    let result =
                        report_with(&parsed, options, viewport, &OperationControl::new(), policy);
                    if limit == natural.text.len() {
                        assert_eq!(result.unwrap().text, natural.text);
                    } else {
                        assert!(
                            matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxOutputBytes)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn record_whitespace_is_not_trimmed_or_lost_on_narrower_reflow() {
    let text = format!("{} trailing  ", "x".repeat(61));
    let source = format!(
        "requirementDiagram\nrequirement r {{\ntext: \"{text}\"\n}}\nmissing - traces -> r\n"
    );
    let parsed = parse(&source);
    let baseline = report_with(
        &parsed,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap();
    assert!(
        baseline.text.lines().any(|line| line.ends_with(' ')),
        "{}",
        baseline.text
    );
    for viewport in [
        AsciiViewportPolicy::unrestricted(),
        AsciiViewportPolicy::with_max_width(24).overflow(OverflowPolicy::Fallback),
    ] {
        let output = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            viewport.trim(AsciiTrimPolicy::TrimTrailingSpaces),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
        assert_eq!(
            output.text.replace('\n', ""),
            baseline.text.replace('\n', "")
        );
        assert_eq!(output.lossiness, Lossiness::None);
        assert!(!output.trimmed);
    }
}

#[test]
fn large_canvas_padding_is_not_charged_as_retained_document_text() {
    let parsed = parse(TYPICAL);
    let options = AsciiRenderOptions::ascii().with_graph_padding_y(1024);
    let natural = report_with(
        &parsed,
        options,
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        AsciiResourcePolicy::unbounded(),
    )
    .unwrap();
    let cells = natural.text.lines().map(str::len).sum::<usize>();
    assert!(natural.emitted_extent.width * natural.emitted_extent.height > cells * 2);
    for (id, limit) in [
        (AsciiResourceLimitId::MaxDocumentCells, cells),
        (AsciiResourceLimitId::MaxOutputBytes, natural.text.len()),
    ] {
        let exact = AsciiResourcePolicy::unbounded()
            .with_limit(id, limit)
            .unwrap();
        let output = report_with(
            &parsed,
            options,
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            exact,
        )
        .unwrap();
        assert_eq!(output.text, natural.text);
        let below = AsciiResourcePolicy::unbounded()
            .with_limit(id, limit - 1)
            .unwrap();
        assert!(
            matches!(report_with(&parsed, options, AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(), below), Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == id)
        );
    }
}
