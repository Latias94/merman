#![cfg(feature = "diagram-pie")]

mod support;

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiGraphPresentation, AsciiOutput,
    AsciiOutputEncoding, AsciiOutputOutcome, AsciiProjection, AsciiRenderOptions, AsciiRenderer,
    AsciiResourceLimitId, AsciiResourcePolicy, AsciiSourceContext, AsciiTrimPolicy,
    AsciiViewportPolicy, Lossiness, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::pie::{PieDiagramModel, PieSection};
use merman_model::{
    MermaidConfig, OperationControl, ParsedSemanticDiagram, SemanticDiagramModel,
    runtime::RuntimePolicy,
};

const SMALL: &str = "pie\ntitle Pets\n\"Dogs\": 386\n\"Cats\": 85\n";
const TYPICAL: &str = "pie showData\ntitle Requests\n\"Read\": 7\n\"Write\": 2\n\"Delete\": 1\n";
const DENSE: &str = "pie\ntitle Categories\n\"Direct\": 40\n\"Organic\": 20\n\"Paid\": 15\n\"Referral\": 10\n\"Email\": 5\n\"Social\": 4\n\"App\": 3\n\"QR\": 2\n\"Tiny\": 0.1\n\"Other\": 0.9\n\"Zero\": 0\n";

fn parse(source: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("Pie parses")
}

fn report_with(
    parsed: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
    control: &OperationControl,
    policy: AsciiResourcePolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?.render_parsed_report(parsed, viewport, control, &context, policy)
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

fn borrowed(
    model: &PieDiagramModel,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
) -> AsciiOutput {
    let config = MermaidConfig::empty_object();
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)
        .unwrap()
        .render_model_report(
            &AsciiDiagram::Pie(model),
            AsciiSourceContext {
                diagram_type: "pie",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            viewport,
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn model(values: &[f64]) -> PieDiagramModel {
    PieDiagramModel {
        sections: values
            .iter()
            .enumerate()
            .map(|(index, &value)| PieSection {
                label: format!("Section {}", index + 1),
                value,
            })
            .collect(),
        ..Default::default()
    }
}

fn assert_primary(output: &AsciiOutput, projection: AsciiProjection, width: usize) {
    assert_eq!(output.family, "pie");
    assert_eq!(output.projection, projection, "{}", output.text);
    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
    assert_eq!(output.primary_extent, Some(output.emitted_extent));
    assert!(!output.fallback.attempted);
    assert!(!output.compact_attempted);
    assert!(!output.emitted_overflowed);
    assert_eq!(output.lossiness, Lossiness::None);
    assert!(output.emitted_extent.width <= width, "{}", output.text);
    assert_eq!(
        support::terminal_extent_with_profile(&output.text, output.width_profile),
        (output.emitted_extent.width, output.emitted_extent.height)
    );
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}

fn bars(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| {
            let (key, rest) = line.split_once(" |")?;
            key.trim().strip_prefix('P')?.parse::<usize>().ok()?;
            let (bar, _) = rest.split_once('|')?;
            Some(bar)
        })
        .collect()
}

#[test]
fn small_typical_dense_remain_spatial_at_80_100_120_in_both_charsets() {
    for source in [SMALL, TYPICAL, DENSE] {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for width in [80, 100, 120] {
                let output = report(source, options, width);
                assert_primary(&output, AsciiProjection::Diagrammatic, width);
                assert!(output.text.contains("Pie proportions"));
                assert!(output.text.contains("0%"));
                assert!(output.text.contains("100%"));
                let SemanticDiagramModel::Pie(authored) = parse(source).model else {
                    panic!("Pie model")
                };
                let chart = bars(&output.text);
                assert_eq!(chart.len(), authored.sections.len());
                for bar in chart {
                    assert_eq!(
                        support::terminal_extent_with_profile(bar, output.width_profile).0,
                        50
                    );
                }
                for (index, section) in authored.sections.iter().enumerate() {
                    assert!(compact(&output.text).contains(&compact(&format!(
                        "P{} label=\"{}\" value={}",
                        index + 1,
                        section.label,
                        section.value
                    ))));
                }
            }
        }
    }
}

#[test]
fn proportions_use_a_shared_hundred_percent_denominator_with_an_independent_error_bound() {
    for count in [1, 2, 3, 10, 25, 100] {
        let values: Vec<_> = (0..count)
            .map(|index| ((index * 17 + 3) % 101) as f64)
            .collect();
        let total: f64 = values.iter().sum();
        let output = borrowed(
            &model(&values),
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        );
        let rows = bars(&output.text);
        assert_eq!(rows.len(), count);
        for (bar, value) in rows.iter().zip(&values) {
            let filled = bar.chars().filter(|&ch| ch == '#').count();
            assert!(
                (filled as f64 / 50.0 - value / total).abs() <= 0.0100000001,
                "value {value}/{total}: {bar}"
            );
            assert_eq!(bar.contains('*'), *value > 0.0 && filled == 0);
        }
        let starts: Vec<_> = output
            .text
            .lines()
            .filter(|line| line.trim_start().starts_with('P') && line.contains(" |"))
            .map(|line| line.find('|').unwrap())
            .collect();
        assert!(
            starts.iter().all(|start| *start == starts[0]),
            "all bars need the same origin"
        );
    }
    let output = borrowed(
        &model(&[1.0, 2.0, 7.0]),
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_eq!(
        bars(&output.text)
            .iter()
            .map(|bar| bar.matches('#').count())
            .collect::<Vec<_>>(),
        [5, 10, 35]
    );
}

#[test]
fn tiny_positive_sections_keep_markers_values_and_the_original_denominator() {
    let output = borrowed(
        &model(&[9999.0, 1.0, 0.0]),
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    let chart = bars(&output.text);
    assert_eq!(chart[0].matches('#').count(), 50);
    assert_eq!(chart[1].matches('*').count(), 1);
    assert_eq!(chart[1].matches('#').count(), 0);
    assert!(chart[2].chars().all(|ch| ch == ' '));
    assert!(output.text.contains("<0.1%"));
    assert!(output.text.contains("not a 2% bar"));
    assert!(output.text.contains("P2 label=\"Section 2\" value=1"));
    assert!(output.text.contains("P3 label=\"Section 3\" value=0"));
}

#[test]
fn half_cell_rounding_distinguishes_adjacent_canonical_values() {
    for (middle, expected) in [(3.0f64.next_down(), 1), (3.0, 1), (3.0f64.next_up(), 0)] {
        let output = borrowed(
            &model(&[1.0, middle, 96.0]),
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        );
        assert_eq!(bars(&output.text)[0].matches('#').count(), expected);
        assert_eq!(bars(&output.text)[0].contains('*'), expected == 0);
    }
    let parsed = report(
        "pie\n\"A\": 1\n\"B\": 2.9999999999999996\n\"C\": 96\n",
        AsciiRenderOptions::ascii(),
        80,
    );
    assert_eq!(bars(&parsed.text)[0].matches('#').count(), 1);
    // The same exact tie at widely separated exponents must not overflow or underflow.
    for unit in [f64::from_bits(1), f64::MIN_POSITIVE, 2.0f64.powi(1000)] {
        let output = borrowed(
            &model(&[unit, 99.0 * unit]),
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        );
        assert_eq!(bars(&output.text)[0].matches('#').count(), 1);
    }
    for (second, percentage) in [(1999.0, "0.1%"), (1999.0f64.next_up(), "<0.1%")] {
        let output = borrowed(
            &model(&[1.0, second]),
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        );
        let row = output
            .text
            .lines()
            .find(|line| line.starts_with("P1 |"))
            .unwrap();
        assert!(row.contains(percentage), "{row}");
        assert_eq!(row.contains('<'), percentage.starts_with('<'));
    }
}

#[test]
fn overflow_and_subnormal_values_keep_correct_spatial_proportions() {
    for values in [[f64::MAX, f64::MAX], [f64::from_bits(1), f64::from_bits(1)]] {
        let output = borrowed(
            &model(&values),
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        );
        assert_primary(&output, AsciiProjection::Diagrammatic, 120);
        assert_eq!(
            bars(&output.text)
                .iter()
                .map(|bar| bar.matches('#').count())
                .collect::<Vec<_>>(),
            [25, 25]
        );
    }
    let output = borrowed(
        &model(&[f64::MAX, f64::from_bits(1)]),
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_eq!(bars(&output.text)[0].matches('#').count(), 50);
    assert!(bars(&output.text)[1].contains('*'));
    assert!(output.text.contains("5e-324"));

    // In units of the least subnormal, these add to 2^64. The carry must not vanish
    // at the accumulator word boundary or make the tiny second section a full bar.
    let carry = borrowed(
        &model(&[
            f64::from_bits((12u64 << 52) | ((1u64 << 52) - 1)),
            f64::from_bits(1 << 11),
        ]),
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_eq!(bars(&carry.text)[0].matches('#').count(), 50);
    assert_eq!(bars(&carry.text)[1].matches('#').count(), 0);
    assert!(bars(&carry.text)[1].contains('*'));
}

#[test]
fn empty_zero_negative_and_nonfinite_models_are_complete_text_primaries() {
    for values in [
        &[][..],
        &[0.0, -0.0],
        &[-1.0, 2.0],
        &[f64::INFINITY, 1.0],
        &[f64::NEG_INFINITY, 2.0],
        &[f64::from_bits(0x7ff8000000000042), 1.0],
    ] {
        let output = borrowed(
            &model(values),
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        );
        assert_primary(&output, AsciiProjection::StructuredText, 120);
        assert!(bars(&output.text).is_empty());
        assert!(output.text.contains("Proportions unavailable:"));
        assert!(output.text.contains("showData: false"));
        if values.iter().any(|value| !value.is_finite()) {
            assert!(output.text.contains("bits=0x"));
        }
        if values.iter().any(|value| value.is_nan()) {
            assert!(output.text.contains("bits=0x7ff8000000000042"));
        }
        if values
            .iter()
            .any(|value| *value == 0.0 && value.is_sign_negative())
        {
            assert!(output.text.contains("value=-0"));
        }
    }
    assert_primary(
        &report("pie\n\"Zero\": 0\n", AsciiRenderOptions::ascii(), 80),
        AsciiProjection::StructuredText,
        80,
    );
}

#[test]
fn parser_first_label_wins_but_direct_duplicate_occurrences_remain_separate() {
    let output = report(
        "pie\n\"Same\": 1\n\"Same\": 9\n\"Other\": 3\n",
        AsciiRenderOptions::ascii(),
        80,
    );
    assert_eq!(bars(&output.text).len(), 2);
    assert!(output.text.contains("P1 label=\"Same\" value=1"));
    assert!(output.text.contains("P2 label=\"Other\" value=3"));
    let mut direct = model(&[1.0, 3.0]);
    direct.sections[1].label = direct.sections[0].label.clone();
    let output = borrowed(
        &direct,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert!(output.text.contains("P1 label=\"Section 1\" value=1"));
    assert!(output.text.contains("P2 label=\"Section 1\" value=3"));
    assert_eq!(bars(&output.text).len(), 2);
}

#[test]
fn long_cjk_emoji_markdown_and_control_labels_remain_safe_complete_records() {
    let mut data = model(&[1.0, 2.0, 7.0]);
    data.sections[0].label = "客户 👨‍👩‍👧‍👦".to_string();
    data.sections[1].label =
        "**Long** label with e\u{301} and <&> a complete meaningful suffix".to_string();
    data.sections[2].label = "  quoted \"value\"\n\u{1b}[31m  ".to_string();
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        let output = borrowed(
            &data,
            AsciiRenderOptions::unicode().with_terminal_width_profile(profile),
            AsciiViewportPolicy::with_max_width(80),
        );
        assert_primary(&output, AsciiProjection::Diagrammatic, 80);
        assert_eq!(bars(&output.text).len(), 3);
        assert!(!output.text.contains('\u{1b}'));
        assert!(output.text.contains("[P2]"));
        assert!(output.text.contains("complete meaningful suffix"));
        assert!(output.text.contains("**Long**"));
        assert!(output.text.contains("客户"));
        assert!(output.text.contains("\\u{1B}"));
        if profile == TerminalWidthProfile::Cjk {
            assert!(!output.text.contains('█'));
        }
    }
}

#[test]
fn width_recovery_is_complete_and_reports_primary_and_fallback_truthfully() {
    let parsed = parse(TYPICAL);
    let natural = report_with(
        &parsed,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap();
    for width in [10, 20, 40] {
        let output = report(TYPICAL, AsciiRenderOptions::ascii(), width);
        assert_eq!(output.projection, AsciiProjection::StructuredText);
        assert_eq!(output.outcome, AsciiOutputOutcome::Fallback);
        assert_eq!(output.lossiness, Lossiness::PresentationOnly);
        assert!(output.fallback.attempted);
        assert!(output.emitted_extent.width <= width);
        assert_eq!(output.primary_extent, natural.primary_extent);
        let text = compact(&output.text);
        for (key, label, value) in [(1, "Read", 7), (2, "Write", 2), (3, "Delete", 1)] {
            assert!(
                text.contains(&format!("P{key}label=\"{label}\"value={value}")),
                "{}",
                output.text
            );
        }
        let allowed = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::with_max_width(width),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
        assert_eq!(allowed.text, natural.text);
        assert!(
            matches!(report_with(&parsed, AsciiRenderOptions::ascii(), AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Error), &OperationControl::new(), AsciiResourcePolicy::default()), Err(AsciiError::WidthOverflow{actual_width,..}) if actual_width == natural.emitted_extent.width)
        );
    }
}

#[test]
fn complete_metadata_show_data_and_all_primary_encodings_are_retained() {
    let source = format!(
        "---\ntitle: Source title\nconfig:\n  pie:\n    donutHole: 0.4\n---\n{SMALL}accTitle: Accessible\naccDescr: Complete proportions\n"
    );
    for (mode, encoding) in [
        (AsciiColorMode::Plain, AsciiOutputEncoding::Plain),
        (AsciiColorMode::Ansi16, AsciiOutputEncoding::Ansi16),
        (AsciiColorMode::Ansi256, AsciiOutputEncoding::Ansi256),
        (AsciiColorMode::TrueColor, AsciiOutputEncoding::TrueColor),
        (AsciiColorMode::Html, AsciiOutputEncoding::Html),
    ] {
        let output = report_with(
            &parse(&source),
            AsciiRenderOptions::ascii().with_color_mode(mode),
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
        assert_eq!(output.encoding, encoding);
        assert_eq!(output.projection, AsciiProjection::Diagrammatic);
        for label in [
            "Pets",
            "Accessible",
            "Complete proportions",
            "donutHole",
            "Source title",
        ] {
            assert!(
                output.text.contains(label),
                "missing {label}: {}",
                output.text
            );
        }
    }
    let hidden = report(SMALL, AsciiRenderOptions::ascii(), 80);
    let shown = report(
        &SMALL.replacen("pie", "pie showData", 1),
        AsciiRenderOptions::ascii(),
        80,
    );
    assert!(
        !hidden
            .text
            .lines()
            .any(|line| line.contains(" |") && line.contains("[386]"))
    );
    assert!(
        shown
            .text
            .lines()
            .any(|line| line.contains(" |") && line.contains("[386]"))
    );
    for output in [hidden, shown] {
        assert!(output.text.contains("value=386"));
    }
    let defaults = report("pie\n\"A\": 1\n", AsciiRenderOptions::ascii(), 80);
    let complete = compact(&defaults.text);
    for field in [
        "title:null",
        "accTitle:null",
        "accDescr:null",
        "sourceTitle:null",
        "sourceConfig:{}",
    ] {
        assert!(
            complete.contains(field),
            "missing {field}: {}",
            defaults.text
        );
    }
}

#[test]
fn resource_failures_and_cancellation_are_not_converted_to_records() {
    let parsed = parse(DENSE);
    for limit in [
        AsciiResourceLimitId::MaxGridCells,
        AsciiResourceLimitId::MaxOutputBytes,
        AsciiResourceLimitId::MaxLayoutWorkUnits,
    ] {
        let policy = AsciiResourcePolicy::default().with_limit(limit, 1).unwrap();
        assert!(matches!(
            report_with(
                &parsed,
                AsciiRenderOptions::ascii(),
                AsciiViewportPolicy::with_max_width(20).overflow(OverflowPolicy::Fallback),
                &OperationControl::new(),
                policy
            ),
            Err(AsciiError::ResourceLimitExceeded(_))
        ));
    }
    for checkpoints in [0, 12, 80] {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(checkpoints);
        assert!(matches!(
            report_with(
                &parsed,
                AsciiRenderOptions::ascii(),
                AsciiViewportPolicy::unrestricted(),
                &control,
                AsciiResourcePolicy::default()
            ),
            Err(AsciiError::Cancelled(_))
        ));
    }
    let error = report_with(
        &parsed,
        AsciiRenderOptions::ascii().with_graph_presentation(AsciiGraphPresentation::Adaptive),
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AsciiError::InvalidOption {
            field: "graph_presentation",
            ..
        }
    ));
}

#[test]
fn every_tracked_pie_fixture_renders_complete_bounded_output() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pie");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mmd"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for path in files {
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = parse(&source);
        let output = report_with(
            &parsed,
            AsciiRenderOptions::unicode(),
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(output.emitted_extent.width <= 80);
        let SemanticDiagramModel::Pie(model) = &parsed.model else {
            panic!("Pie")
        };
        let data = compact(&output.text);
        for index in 0..model.sections.len() {
            assert!(
                data.contains(&format!("P{}label=", index + 1)),
                "{}: missing section",
                path.display()
            );
        }
    }
}

#[test]
fn retained_cells_and_bytes_are_admitted_once_for_bars_and_record_reflow() {
    for source in [SMALL, "pie\n\"Zero\": 0\n"] {
        let parsed = parse(source);
        let options = AsciiRenderOptions::ascii();
        let primary = report_with(
            &parsed,
            options,
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            AsciiResourcePolicy::unbounded(),
        )
        .unwrap();
        let primary_cells = primary.text.lines().map(str::len).sum::<usize>();
        for viewport in [
            AsciiViewportPolicy::unrestricted(),
            AsciiViewportPolicy::with_max_width(20).overflow(OverflowPolicy::Fallback),
        ] {
            let output = report_with(
                &parsed,
                options,
                viewport,
                &OperationControl::new(),
                AsciiResourcePolicy::unbounded(),
            )
            .unwrap();
            let cells = primary_cells.max(output.text.lines().map(str::len).sum::<usize>());
            let bytes = primary.text.len().max(output.text.len());
            for (id, exact) in [
                (AsciiResourceLimitId::MaxDocumentCells, cells),
                (AsciiResourceLimitId::MaxOutputBytes, bytes),
            ] {
                // The byte ceiling also bounds semantic-envelope materialization. The bars
                // case has a larger final document than that envelope; the zero-only record
                // case can be shorter and therefore owns the cell boundary here instead.
                if id == AsciiResourceLimitId::MaxOutputBytes && source != SMALL {
                    continue;
                }
                for limit in [exact, exact - 1] {
                    let policy = AsciiResourcePolicy::unbounded()
                        .with_limit(id, limit)
                        .unwrap();
                    let result =
                        report_with(&parsed, options, viewport, &OperationControl::new(), policy);
                    if limit == exact {
                        assert_eq!(result.unwrap().text, output.text);
                    } else {
                        assert!(
                            matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == id)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn authored_label_spaces_survive_trim_requests_and_narrow_record_reflow() {
    let label = format!("{} tail  ", "x".repeat(61));
    let source = format!("pie\n\"{label}\": 0\n");
    let parsed = parse(&source);
    let options = AsciiRenderOptions::ascii();
    let baseline = report_with(
        &parsed,
        options,
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
            options,
            viewport.trim(AsciiTrimPolicy::TrimTrailingSpaces),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
        assert_eq!(
            output.text.replace('\n', ""),
            baseline.text.replace('\n', "")
        );
        assert!(!output.trimmed);
        assert_eq!(output.lossiness, Lossiness::None);
    }
}
