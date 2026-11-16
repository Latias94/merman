#![cfg(feature = "diagram-packet")]

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiOutput, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
    AsciiSourceContext, AsciiViewportPolicy, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::packet::{PacketBlock, PacketDiagramModel};
use merman_model::runtime::RuntimePolicy;
use merman_model::{MermaidConfig, OperationControl, ParsedSemanticDiagram};
use unicode_width::UnicodeWidthStr;

const HEADER: &str = "0-3: \"VER\"\n4-7: \"HLEN\"\n8-15: \"LEN\"";

fn source(config: &str, body: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .parse_diagram_sync(&format!(
            "---\nconfig:\n  packet:\n{config}\n---\npacket\n{body}\n"
        ))
        .unwrap()
}

fn report(
    parsed: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    width: usize,
    overflow: OverflowPolicy,
) -> AsciiOutput {
    AsciiRenderer::new(options)
        .unwrap()
        .render_parsed_report(
            parsed,
            AsciiViewportPolicy::with_max_width(width).overflow(overflow),
            &OperationControl::new(),
            &RuntimePolicy::deterministic().begin_operation().unwrap(),
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn bare(model: &PacketDiagramModel) -> String {
    AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_model(
            &AsciiDiagram::Packet(model),
            &OperationControl::new(),
            &RuntimePolicy::deterministic().begin_operation().unwrap(),
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn field(start: f64, end: f64, bits: f64, label: &str) -> PacketBlock {
    PacketBlock {
        start,
        end,
        bits,
        label: label.into(),
    }
}

#[test]
fn fixed_scale_fields_fit_without_label_driven_geometry() {
    let parsed = source("    bitsPerRow: 16", HEADER);
    let output = report(
        &parsed,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    );
    assert_eq!(output.projection, AsciiProjection::Diagrammatic);
    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
    assert!(
        output.text.contains(concat!(
            "+-------+-------+---------------+\n",
            "|  VER  | HLEN  |      LEN      |\n",
            "+-------+-------+---------------+"
        )),
        "{}",
        output.text
    );
    assert!(
        output
            .text
            .contains("range=[0..3] bits=4 label(bytes=3)=\"VER\"")
    );
    assert!(
        output
            .text
            .contains("range=[8..15] bits=8 label(bytes=3)=\"LEN\"")
    );

    let long = source(
        "    bitsPerRow: 16",
        &HEADER.replace("VER", &"Long".repeat(30)),
    );
    let long = report(
        &long,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    );
    assert!(
        long.text.contains("|   *   | HLEN  |      LEN      |"),
        "{}",
        long.text
    );
    assert!(long.text.contains("label(bytes=120)="));
    assert!(
        long.text
            .lines()
            .all(|line| UnicodeWidthStr::width(line) <= 80)
    );
}

#[test]
fn bit_order_mirrors_fields_and_annotations_even_with_hidden_numbers() {
    let ascending = source("    bitsPerRow: 16", HEADER);
    let descending = source("    bitsPerRow: 16\n    bitOrder: descending", HEADER);
    let ascending = report(
        &ascending,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    );
    let descending = report(
        &descending,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    );
    assert!(ascending.text.contains("0..3"));
    assert!(descending.text.contains("3..0"));
    assert!(descending.text.contains("15..8"));
    assert!(ascending.text.contains("|  VER  | HLEN  |      LEN      |"));
    assert!(
        descending
            .text
            .contains("|      LEN      | HLEN  |  VER  |")
    );
    let hidden = source(
        "    bitsPerRow: 16\n    bitOrder: descending\n    showBits: false",
        HEADER,
    );
    let hidden = report(
        &hidden,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    );
    let plot = hidden.text.split("Field records:").next().unwrap();
    assert!(!plot.contains("0..3"), "{plot}");
    assert!(plot.contains("|      LEN      | HLEN  |  VER  |"));
}

#[test]
fn direct_models_use_absolute_coordinates_and_expose_unassigned_bits() {
    let model = PacketDiagramModel {
        packet: vec![
            vec![field(2.0, 5.0, 4.0, "A")],
            vec![field(30.0, 34.0, 99.0, "B")],
        ],
        ..Default::default()
    };
    let text = bare(&model);
    let plot = text.split("Field records:").next().unwrap();
    assert_eq!(plot.matches("word ").count(), 2, "{text}");
    assert_eq!(plot.lines().filter(|line| line.starts_with('+')).count(), 4);
    assert!(plot.contains("|...|   A   |"), "{text}");
    assert!(plot.contains("| B |"), "{text}");
    assert!(plot.contains("|  B  |"), "{text}");
    for line in plot.lines().filter(|line| line.starts_with(['+', '|'])) {
        assert_eq!(line.len(), 65, "{line}");
    }
    assert!(
        text.contains("range=[30..34] bits=99"),
        "DB width must not be corrected: {text}"
    );
}

#[test]
fn raw_split_counts_and_cross_row_ownership_remain_complete() {
    let parsed = source("    bitsPerRow: 16", "0-3: \"A\"\n4-35: \"Payload\"");
    let output = report(
        &parsed,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    );
    assert_eq!(output.text.matches("word ").count(), 3);
    for record in [
        "range=[4..15] bits=11",
        "range=[16..31] bits=15",
        "range=[32..35] bits=3",
    ] {
        assert!(output.text.contains(record), "{}", output.text);
    }
}

#[test]
fn descending_mirrors_cross_row_fields_and_asymmetric_gaps() {
    let model = PacketDiagramModel {
        packet: vec![vec![field(2.0, 5.0, 4.0, "A"), field(14.0, 18.0, 5.0, "B")]],
        ..Default::default()
    };
    let mut config = MermaidConfig::empty_object();
    config.set_value("packet.bitsPerRow", serde_json::json!(16));
    config.set_value("packet.bitOrder", serde_json::json!("descending"));
    for show_bits in [true, false] {
        config.set_value("packet.showBits", serde_json::json!(show_bits));
        let output = AsciiRenderer::new(AsciiRenderOptions::ascii())
            .unwrap()
            .render_model_report(
                &AsciiDiagram::Packet(&model),
                AsciiSourceContext {
                    diagram_type: "packet",
                    config: &config,
                    semantic_config: &config,
                    title: None,
                },
                AsciiViewportPolicy::unrestricted(),
                &OperationControl::new(),
                &RuntimePolicy::deterministic().begin_operation().unwrap(),
                AsciiResourcePolicy::default(),
            )
            .unwrap();
        let content: Vec<_> = output
            .text
            .split("Field records:")
            .next()
            .unwrap()
            .lines()
            .filter(|line| line.starts_with('|'))
            .collect();
        assert_eq!(
            content,
            [
                "| B |...............|   A   |...|",
                "|.........................|  B  |"
            ]
        );
        if show_bits {
            assert!(output.text.contains("5..2"));
            assert!(output.text.contains("range=[14..18] bits=5"));
        }
    }
}

#[test]
fn narrow_nonfinite_record_fallback_retains_the_numbers_json_cannot_encode() {
    let model = PacketDiagramModel {
        packet: vec![vec![field(0.0, f64::INFINITY, f64::NAN, "nonfinite")]],
        ..Default::default()
    };
    let config = MermaidConfig::empty_object();
    let output = AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_model_report(
            &AsciiDiagram::Packet(&model),
            AsciiSourceContext {
                diagram_type: "packet",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            AsciiViewportPolicy::with_max_width(32).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            &RuntimePolicy::deterministic().begin_operation().unwrap(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
    assert_eq!(output.outcome, AsciiOutputOutcome::Fallback);
    let flattened: String = output.text.lines().collect();
    assert!(
        flattened.contains("range=[0..Infinity] bits=NaN"),
        "{}",
        output.text
    );
    assert!(flattened.contains("nonfinite"));
}

#[test]
fn cjk_and_emoji_labels_keep_borders_at_fixed_terminal_cells() {
    let parsed = source(
        "    bitsPerRow: 16",
        "0-3: \"请求\"\n4-7: \"👩‍💻\"\n8-15: \"·Ω\"",
    );
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        let options = AsciiRenderOptions::default().with_terminal_width_profile(profile);
        let output = report(&parsed, options, 80, OverflowPolicy::Error);
        let plot = output.text.split("Field records:").next().unwrap();
        assert!(plot.contains("请求"));
        assert!(plot.contains("👩‍💻"));
        let grid: Vec<_> = plot
            .lines()
            .filter(|line| line.starts_with(['+', '|', '┌', '│', '└']))
            .collect();
        assert_eq!(grid.len(), 3, "{plot}");
        for line in grid {
            let width = if profile == TerminalWidthProfile::Cjk {
                UnicodeWidthStr::width_cjk(line)
            } else {
                UnicodeWidthStr::width(line)
            };
            assert_eq!(width, 33, "{line}");
        }
        if profile == TerminalWidthProfile::Cjk {
            assert!(
                !plot.contains('│'),
                "ambiguous-width borders need the ASCII structural charset"
            );
        }
    }
}

#[test]
fn one_bit_unsafe_and_multiline_labels_are_disclosed_without_changing_the_grid() {
    let model = PacketDiagramModel {
        packet: vec![vec![
            field(0.0, 0.0, 1.0, "SYN"),
            field(1.0, 7.0, 7.0, "a\n\u{1b}[31m|bad"),
            field(8.0, 15.0, 8.0, "\u{301}name"),
        ]],
        ..Default::default()
    };
    let text = bare(&model);
    let plot = text.split("Field records:").next().unwrap();
    assert!(!plot.contains("bad"), "{text}");
    assert!(!plot.contains("name"), "{text}");
    assert!(plot.contains("|*|      *      |       *       |"), "{text}");
    assert!(text.contains("\"SYN\""));
    assert!(text.contains("\\u{1B}"));
    assert!(text.contains("\\n"));
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn nonspatial_models_report_the_actual_text_projection() {
    let config = MermaidConfig::empty_object();
    for blocks in [
        vec![field(0.5, 7.0, 7.5, "fractional")],
        vec![field(0.0, 7.0, -1.0, "negative")],
        vec![field(0.0, f64::INFINITY, f64::NAN, "nonfinite")],
        vec![
            field(0.0, 7.0, 8.0, "first"),
            field(7.0, 15.0, 9.0, "overlap"),
        ],
        vec![field(8.0, 15.0, 8.0, "late"), field(0.0, 7.0, 8.0, "early")],
        vec![],
    ] {
        let model = PacketDiagramModel {
            packet: vec![blocks],
            ..Default::default()
        };
        let output = AsciiRenderer::new(AsciiRenderOptions::ascii())
            .unwrap()
            .render_model_report(
                &AsciiDiagram::Packet(&model),
                AsciiSourceContext {
                    diagram_type: "packet",
                    config: &config,
                    semantic_config: &config,
                    title: None,
                },
                AsciiViewportPolicy::unrestricted(),
                &OperationControl::new(),
                &RuntimePolicy::deterministic().begin_operation().unwrap(),
                AsciiResourcePolicy::default(),
            )
            .unwrap();
        assert_eq!(
            output.projection,
            AsciiProjection::StructuredText,
            "{}",
            output.text
        );
        assert!(!output.text.contains("cells/bit"));
    }
    for width in ["2.5", "65", "9007199254740994"] {
        let parsed = source(&format!("    bitsPerRow: {width}"), "0: \"A\"");
        assert_eq!(
            report(
                &parsed,
                AsciiRenderOptions::ascii(),
                80,
                OverflowPolicy::Error
            )
            .projection,
            AsciiProjection::StructuredText
        );
    }
}

#[test]
fn viewport_fallback_is_complete_readable_text_with_honest_metadata() {
    let parsed = source(
        "    bitsPerRow: 32\n    bitOrder: descending",
        "0-31: \"完整 Payload\"",
    );
    let fallback = report(
        &parsed,
        AsciiRenderOptions::ascii(),
        32,
        OverflowPolicy::Fallback,
    );
    assert_eq!(fallback.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(fallback.projection, AsciiProjection::StructuredText);
    assert!(fallback.text.contains("row 1:"), "{}", fallback.text);
    assert!(fallback.text.contains("range=[0..31]"));
    assert!(fallback.text.contains("完整 Payload"));
    assert!(fallback.text.contains("sourceConfig.packet.bitOrder"));
    assert!(
        fallback
            .text
            .lines()
            .all(|line| UnicodeWidthStr::width(line) <= 32)
    );

    let error = AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_parsed_report(
            &parsed,
            AsciiViewportPolicy::with_max_width(32).overflow(OverflowPolicy::Error),
            &OperationControl::new(),
            &RuntimePolicy::deterministic().begin_operation().unwrap(),
            AsciiResourcePolicy::default(),
        )
        .unwrap_err();
    assert!(
        matches!(
            error,
            AsciiError::WidthOverflow {
                actual_width: 65,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn output_grid_and_cancellation_budgets_are_not_text_fallbacks() {
    let parsed = source("    bitsPerRow: 8", "0-7: \"Type\"");
    let renderer = AsciiRenderer::new(AsciiRenderOptions::ascii()).unwrap();
    let render = |control: &OperationControl, policy| {
        renderer.render_parsed(
            &parsed,
            control,
            &RuntimePolicy::deterministic().begin_operation().unwrap(),
            policy,
        )
    };
    let text = render(&OperationControl::new(), AsciiResourcePolicy::default()).unwrap();
    for (limit, exact) in [
        (AsciiResourceLimitId::MaxOutputBytes, text.len()),
        (AsciiResourceLimitId::MaxGridCells, 17 * 5),
    ] {
        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(limit, exact)
            .unwrap();
        assert!(render(&OperationControl::new(), exact_policy).is_ok());
        let short = AsciiResourcePolicy::default()
            .with_limit(limit, exact - 1)
            .unwrap();
        assert!(
            matches!(render(&OperationControl::new(), short), Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == limit)
        );
    }
    for after in [0, 40, 500] {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(after);
        assert!(matches!(
            render(&control, AsciiResourcePolicy::default()),
            Err(AsciiError::Cancelled(_))
        ));
    }
    let huge = PacketDiagramModel {
        packet: vec![vec![field(0.0, 9_007_199_254_740_990.0, 1.0, "huge")]],
        ..Default::default()
    };
    let error = renderer
        .render_model(
            &AsciiDiagram::Packet(&huge),
            &OperationControl::new(),
            &RuntimePolicy::deterministic().begin_operation().unwrap(),
            AsciiResourcePolicy::default(),
        )
        .unwrap_err();
    assert!(matches!(error, AsciiError::ResourceLimitExceeded(_)));
}

#[test]
fn all_primary_encodings_and_phase_gate_widths_accept_packet_samples() {
    let samples = [
        source("    bitsPerRow: 16", HEADER),
        source("    bitsPerRow: 32", "0-15: \"Source port\"\n16-31: \"Destination port\"\n32-63: \"Sequence number\""),
        SemanticParser::new().parse_diagram_sync(include_str!("../../../fixtures/packet/upstream_cypress_packet_spec_should_render_a_complex_packet_diagram_004.mmd")).unwrap(),
    ];
    for (index, parsed) in samples.iter().enumerate() {
        for width in [80, 100, 120] {
            let output = report(
                parsed,
                AsciiRenderOptions::ascii(),
                width,
                OverflowPolicy::Error,
            );
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            assert!(output.emitted_extent.width <= width);
            assert!(output.text.contains("Field records:"));
            eprintln!(
                "packet sample {index} @ {width}: {} x {}",
                output.emitted_extent.width, output.emitted_extent.height
            );
        }
    }
    for color in [
        AsciiColorMode::Plain,
        AsciiColorMode::Ansi16,
        AsciiColorMode::Ansi256,
        AsciiColorMode::TrueColor,
        AsciiColorMode::Html,
    ] {
        let output = report(
            &samples[0],
            AsciiRenderOptions::default().with_color_mode(color),
            80,
            OverflowPolicy::Error,
        );
        assert_eq!(output.projection, AsciiProjection::Diagrammatic);
        assert_eq!(
            output.encoding,
            merman_ascii::AsciiOutputEncoding::from_color_mode(color)
        );
    }
}

#[test]
fn generated_partitions_keep_every_boundary_at_its_bit_coordinate() {
    use std::collections::BTreeSet;

    for bits_per_row in 1usize..=64 {
        let mut cursor = 1;
        let mut blocks = Vec::new();
        for index in 0..12 {
            let length = (index * 7 + bits_per_row) % 9 + 1;
            blocks.push(field(
                cursor as f64,
                (cursor + length - 1) as f64,
                length as f64,
                &format!("F{index}"),
            ));
            cursor += length + index % 3;
        }
        let model = PacketDiagramModel {
            packet: vec![blocks],
            ..Default::default()
        };
        let mut config = MermaidConfig::empty_object();
        config.set_value("packet.bitsPerRow", serde_json::json!(bits_per_row));
        config.set_value("packet.showBits", serde_json::json!(false));
        for descending in [false, true] {
            config.set_value(
                "packet.bitOrder",
                serde_json::json!(if descending {
                    "descending"
                } else {
                    "ascending"
                }),
            );
            let output = AsciiRenderer::new(AsciiRenderOptions::ascii())
                .unwrap()
                .render_model_report(
                    &AsciiDiagram::Packet(&model),
                    AsciiSourceContext {
                        diagram_type: "packet",
                        config: &config,
                        semantic_config: &config,
                        title: None,
                    },
                    AsciiViewportPolicy::unrestricted(),
                    &OperationControl::new(),
                    &RuntimePolicy::deterministic().begin_operation().unwrap(),
                    AsciiResourcePolicy::default(),
                )
                .unwrap();
            let plot = output.text.split("Field records:").next().unwrap();
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            let expected_rows =
                (model.packet[0].last().unwrap().end as usize + 1).div_ceil(bits_per_row);
            assert_eq!(
                plot.lines().filter(|line| line.starts_with('|')).count(),
                expected_rows
            );
            for index in 0..12 {
                assert!(output.text.contains(&format!("\"F{index}\"")));
            }
            for (row, line) in plot
                .lines()
                .filter(|line| line.starts_with('|'))
                .enumerate()
            {
                let start = row * bits_per_row;
                let end = start + bits_per_row;
                let mut expected = BTreeSet::from([0, bits_per_row * 2]);
                for block in &model.packet[0] {
                    if block.start < end as f64 && block.end >= start as f64 {
                        expected.insert(((block.start as usize).max(start) - start) * 2);
                        expected.insert(((block.end as usize + 1).min(end) - start) * 2);
                    }
                }
                if descending {
                    expected = expected
                        .into_iter()
                        .map(|column| bits_per_row * 2 - column)
                        .collect();
                }
                let actual: BTreeSet<_> =
                    line.match_indices('|').map(|(column, _)| column).collect();
                assert_eq!(
                    actual, expected,
                    "bitsPerRow={bits_per_row}, row={row}: {line}"
                );
                assert_eq!(line.len(), bits_per_row * 2 + 1);
            }
        }
    }
}
