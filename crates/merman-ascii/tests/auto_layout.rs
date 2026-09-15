mod support;

use merman_ascii::{
    AsciiColorMode, AsciiError, AsciiLayoutProfile, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiViewportPolicy, OverflowPolicy, TerminalWidthProfile,
};
use support::{
    assert_rectangular_terminal_grid_with_profile, local_semantic_input, parse_model, render_model,
    render_model_report, terminal_extent_with_profile,
};

const LONG_NODE_CHAIN: &str = "flowchart LR\n\
    A[many words are too much sometime] --> B[many words are too much sometime]\n\
    B --> C[many words are too much sometime]\n\
    C --> D[many words are too much sometime]";

#[test]
fn auto_width_matrix_keeps_complete_nodes_and_arrows_when_compact_is_taller() {
    let model = parse_model(LONG_NODE_CHAIN);
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let canonical = render_model(&model, &options).expect("canonical chain should render");
        let compact = render_model(
            &model,
            &options.with_layout_profile(AsciiLayoutProfile::Compact),
        )
        .expect("compact chain should render");
        let canonical_extent =
            terminal_extent_with_profile(&canonical, options.terminal_width_profile);
        let compact_extent = terminal_extent_with_profile(&compact, options.terminal_width_profile);
        assert!(compact_extent.0 < canonical_extent.0);
        assert!(compact_extent.1 > canonical_extent.1);
        assert!(canonical_extent.0 > 120);
        assert!(compact_extent.0 <= 120 && compact_extent.0 > 100);

        for width in [60, 80, 100, 120] {
            let output = render_model_report(
                &model,
                &options.with_layout_profile(AsciiLayoutProfile::Auto),
                AsciiViewportPolicy::with_max_width(width),
            )
            .expect("Allow should emit the complete narrower candidate");
            assert_eq!(output.text, compact);
            assert_eq!(output.requested_layout_profile, AsciiLayoutProfile::Auto);
            assert_eq!(output.layout_profile, AsciiLayoutProfile::Compact);
            assert!(output.compact_attempted);
            assert!(!output.fallback.attempted);
            assert_eq!(output.primary_extent.width, compact_extent.0);
            assert_eq!(output.primary_extent.height, compact_extent.1);
            assert_eq!(output.emitted_extent, output.primary_extent);
            assert_eq!(output.overflowed, width < 120);
            assert_eq!(
                output.outcome,
                if width < 120 {
                    AsciiOutputOutcome::WideAllowed
                } else {
                    AsciiOutputOutcome::Primary
                }
            );
            for word in ["many", "words", "are", "too", "much", "sometime"] {
                assert_eq!(
                    output.text.matches(word).count(),
                    4,
                    "lost {word}: {}",
                    output.text
                );
            }
            let marker = if options == AsciiRenderOptions::ascii() {
                '>'
            } else {
                '►'
            };
            assert_eq!(output.text.matches(marker).count(), 3, "{}", output.text);
            assert_rectangular_terminal_grid_with_profile(
                &output.text,
                options.terminal_width_profile,
            );
        }
    }
}

#[test]
fn auto_keeps_canonical_without_retry_when_it_fits_exactly() {
    let model = parse_model("flowchart LR\nA -->|approved| B");
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        let options = AsciiRenderOptions::unicode().with_terminal_width_profile(profile);
        let canonical = render_model(&model, &options).expect("canonical graph should render");
        let width = terminal_extent_with_profile(&canonical, profile).0;
        for overflow in [
            OverflowPolicy::Allow,
            OverflowPolicy::Error,
            OverflowPolicy::Fallback,
        ] {
            let output = render_model_report(
                &model,
                &options.with_layout_profile(AsciiLayoutProfile::Auto),
                AsciiViewportPolicy::with_max_width(width).overflow(overflow),
            )
            .expect("an exact fit should emit canonical output under every overflow policy");
            assert_eq!(output.text, canonical);
            assert_eq!(output.layout_profile, AsciiLayoutProfile::Canonical);
            assert_eq!(output.requested_layout_profile, AsciiLayoutProfile::Auto);
            assert!(!output.compact_attempted);
            assert!(!output.fallback.attempted);
            assert!(!output.overflowed);
        }
    }
}

#[test]
fn auto_applies_error_or_structured_fallback_to_the_selected_primary_extent() {
    let model = parse_model(LONG_NODE_CHAIN);
    let options = AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto);
    let selected = render_model_report(&model, &options, AsciiViewportPolicy::with_max_width(60))
        .expect("Allow should return the complete compact diagram");
    let error = render_model_report(
        &model,
        &options,
        AsciiViewportPolicy::with_max_width(60).overflow(OverflowPolicy::Error),
    )
    .expect_err("neither primary candidate fits sixty cells");
    assert!(matches!(
        error,
        AsciiError::WidthOverflow { max_width: 60, actual_width, .. }
            if actual_width == selected.primary_extent.width
    ));

    let fallback = render_model_report(
        &model,
        &options,
        AsciiViewportPolicy::with_max_width(60).overflow(OverflowPolicy::Fallback),
    )
    .expect("the complete structured graph should fit sixty cells");
    assert_eq!(fallback.primary_extent, selected.primary_extent);
    assert_eq!(fallback.layout_profile, AsciiLayoutProfile::Compact);
    assert_eq!(fallback.requested_layout_profile, AsciiLayoutProfile::Auto);
    assert!(fallback.compact_attempted);
    assert!(fallback.fallback.attempted);
    assert!(fallback.overflowed);
    assert_eq!(fallback.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(fallback.projection, AsciiProjection::StructuredText);
    assert!(fallback.emitted_extent.width <= 60);
    for word in ["many", "words", "sometime"] {
        assert_eq!(fallback.text.matches(word).count(), 4, "{}", fallback.text);
    }
}

#[test]
fn explicit_canonical_remains_fixed_even_when_compact_would_fit() {
    let model = parse_model(LONG_NODE_CHAIN);
    let options = AsciiRenderOptions::ascii();
    let canonical = render_model(&model, &options).expect("canonical graph should render");
    let output = render_model_report(&model, &options, AsciiViewportPolicy::with_max_width(120))
        .expect("fixed Canonical with Allow should keep its full width");
    assert_eq!(output.text, canonical);
    assert_eq!(output.layout_profile, AsciiLayoutProfile::Canonical);
    assert_eq!(
        output.requested_layout_profile,
        AsciiLayoutProfile::Canonical
    );
    assert!(!output.compact_attempted);
    assert_eq!(output.outcome, AsciiOutputOutcome::WideAllowed);
    assert!(matches!(
        render_model_report(
            &model,
            &options,
            AsciiViewportPolicy::with_max_width(120).overflow(OverflowPolicy::Error),
        ),
        Err(AsciiError::WidthOverflow { .. })
    ));
}

#[test]
fn explicit_family_overrides_survive_auto_and_equal_width_prefers_canonical() {
    let cases = [
        (
            LONG_NODE_CHAIN,
            AsciiRenderOptions::ascii()
                .with_graph_padding_x(5)
                .with_flowchart_node_label_wrap_width(40),
        ),
        (
            "sequenceDiagram\nAlice->>Bob: ping\nBob-->>Alice: pong",
            AsciiRenderOptions::ascii().with_sequence_participant_spacing(5),
        ),
    ];
    for (source, options) in cases {
        let model = parse_model(source);
        let canonical = render_model(&model, &options).expect("explicit overrides should render");
        let compact = render_model(
            &model,
            &options.with_layout_profile(AsciiLayoutProfile::Compact),
        )
        .expect("Compact should preserve explicit overrides, including old default values");
        assert_eq!(canonical, compact);
        let width = terminal_extent_with_profile(&canonical, options.terminal_width_profile).0;
        let output = render_model_report(
            &model,
            &options.with_layout_profile(AsciiLayoutProfile::Auto),
            AsciiViewportPolicy::with_max_width(width - 1),
        )
        .expect("Auto should preserve a complete canonical candidate on a width tie");
        assert_eq!(output.text, canonical);
        assert_eq!(output.layout_profile, AsciiLayoutProfile::Canonical);
        assert!(output.compact_attempted);
        assert!(output.overflowed);
    }
}

#[test]
fn auto_sequence_with_notes_and_self_messages_fits_eighty_columns() {
    let model = parse_model(&local_semantic_input(
        "sequence/self_messages_with_notes.mmd",
    ));
    let options = AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto);
    let output = render_model_report(
        &model,
        &options,
        AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Error),
    )
    .expect("Compact should fit the note/self-message diagram");
    assert_eq!(output.layout_profile, AsciiLayoutProfile::Compact);
    assert!(output.compact_attempted);
    assert_eq!(
        (output.emitted_extent.width, output.emitted_extent.height),
        (78, 58)
    );
    for label in [
        "event.preventDefault()",
        "WINDOW_CLOSE_REQUESTED",
        "Panel removed",
        "Panel reopens",
    ] {
        assert!(output.text.contains(label), "lost {label}: {}", output.text);
    }
    assert!(!output.fallback.attempted);
}

#[test]
fn auto_requires_a_positive_viewport_and_string_helpers_cannot_supply_one() {
    let model = parse_model("flowchart LR\nA --> B");
    let options = AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto);
    for viewport in [
        AsciiViewportPolicy::unrestricted(),
        AsciiViewportPolicy::with_max_width(0),
    ] {
        assert!(matches!(
            render_model_report(&model, &options, viewport),
            Err(AsciiError::InvalidOption {
                field: "ascii_viewport.max_width",
                ..
            })
        ));
    }
    assert!(matches!(
        render_model(&model, &options),
        Err(AsciiError::InvalidOption {
            field: "ascii_viewport.max_width",
            ..
        })
    ));
}

#[test]
fn auto_unsupported_family_and_canonical_route_errors_remain_errors() {
    let options = AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto);
    let state = parse_model("stateDiagram-v2\n[*] --> Ready\nReady --> [*]");
    assert!(matches!(
        render_model_report(&state, &options, AsciiViewportPolicy::with_max_width(80)),
        Err(AsciiError::InvalidOption {
            field: "layout_profile",
            ..
        })
    ));

    let fanout = parse_model(
        "flowchart LR\nA -->|description 0| N0\nA -->|description 1| N1\nA -->|description 2| N2\nA -->|description 3| N3\nA -->|description 4| N4\nA -->|description 5| N5",
    );
    assert!(matches!(
        render_model_report(
            &fanout,
            &options,
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Allow),
        ),
        Err(AsciiError::UnsupportedFeature { .. })
    ));
}

#[test]
fn auto_preserves_styled_encoding_and_rejects_styled_fallback_preflight() {
    let model = parse_model(LONG_NODE_CHAIN);
    for (color, encoded_marker) in [
        (AsciiColorMode::Ansi16, "\u{1b}["),
        (AsciiColorMode::Html, "<span"),
    ] {
        let options = AsciiRenderOptions::ascii()
            .with_layout_profile(AsciiLayoutProfile::Auto)
            .with_color_mode(color);
        let output = render_model_report(
            &model,
            &options,
            AsciiViewportPolicy::with_max_width(120).overflow(OverflowPolicy::Error),
        )
        .expect("automatic fitting supports styled primary output");
        assert_eq!(output.layout_profile, AsciiLayoutProfile::Compact);
        assert!(output.compact_attempted);
        assert!(output.text.contains(encoded_marker));
        assert!(output.emitted_extent.width <= 120);
        assert!(matches!(
            render_model_report(
                &model,
                &options,
                AsciiViewportPolicy::with_max_width(200).overflow(OverflowPolicy::Fallback),
            ),
            Err(AsciiError::InvalidOption {
                field: "ascii_viewport.overflow",
                ..
            })
        ));
    }
}

#[test]
fn schema_three_serializes_requested_and_selected_layout_without_conflating_fallback() {
    let model = parse_model(LONG_NODE_CHAIN);
    let output = render_model_report(
        &model,
        &AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto),
        AsciiViewportPolicy::with_max_width(120),
    )
    .expect("automatic fitting should produce a report");
    let metadata = serde_json::to_value(output.metadata()).expect("metadata should serialize");
    assert_eq!(metadata["schema_version"], 3);
    assert_eq!(metadata["requested_layout_profile"], "auto");
    assert_eq!(metadata["layout_profile"], "compact");
    assert_eq!(metadata["compact_attempted"], true);
    assert_eq!(metadata["fallback_attempted"], false);
    let report = serde_json::to_value(output.report()).expect("report should serialize");
    for (field, value) in metadata.as_object().expect("metadata object") {
        assert_eq!(
            &report[field], value,
            "report and metadata differ at {field}"
        );
    }
    assert_eq!(report["text"], output.text);
}

#[test]
fn auto_sequence_retains_canonical_document_under_the_same_exact_cell_budget() {
    use merman_ascii::{AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::{OperationControl, runtime::RuntimePolicy};
    use unicode_width::UnicodeWidthStr;

    let model = parse_model(&local_semantic_input(
        "sequence/self_messages_with_notes.mmd",
    ));
    let options = AsciiRenderOptions::ascii();
    let canonical = render_model(&model, &options).expect("canonical sequence should render");
    let compact = render_model(
        &model,
        &options.with_layout_profile(AsciiLayoutProfile::Compact),
    )
    .expect("compact sequence should render");
    let document_cells = |text: &str| text.lines().map(UnicodeWidthStr::width).sum::<usize>();
    let canonical_cells = document_cells(&canonical);
    let compact_cells = document_cells(&compact);
    assert!(canonical_cells > compact_cells);

    // Auto with Allow retains the complete Canonical document while admitting Compact.
    // The fixed-profile case distinguishes candidate accounting from retained storage.
    for (profile, exact_cells) in [
        (AsciiLayoutProfile::Compact, compact_cells),
        (AsciiLayoutProfile::Auto, canonical_cells + compact_cells),
    ] {
        let renderer =
            AsciiRenderer::new(options.with_layout_profile(profile)).expect("valid layout profile");
        let render_with_budget = |limit| {
            let context = RuntimePolicy::deterministic()
                .begin_operation()
                .expect("deterministic test operation context");
            renderer.render_model_report(
                &model,
                AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Allow),
                &OperationControl::new(),
                &context,
                AsciiResourcePolicy::default()
                    .with_limit(AsciiResourceLimitId::MaxDocumentCells, limit)
                    .expect("positive document cell budget"),
            )
        };
        let output = render_with_budget(exact_cells).unwrap_or_else(|error| {
            panic!("{profile:?} must fit exactly {exact_cells} cells: {error}")
        });
        assert_eq!(output.text, compact);
        assert_eq!(output.layout_profile, AsciiLayoutProfile::Compact);
        assert_eq!(output.requested_layout_profile, profile);
        assert_eq!(
            output.compact_attempted,
            profile == AsciiLayoutProfile::Auto
        );
        assert_eq!(
            (output.primary_extent.width, output.primary_extent.height),
            (78, 58)
        );
        assert_eq!(output.emitted_extent, output.primary_extent);
        assert!(!output.overflowed);
        assert!(!output.fallback.attempted);

        let error = render_with_budget(exact_cells - 1)
            .expect_err("one cell below retained plus selected documents must exhaust the budget");
        assert!(
            matches!(
                error,
                AsciiError::ResourceLimitExceeded(ref exceeded)
                    if exceeded.limit == AsciiResourceLimitId::MaxDocumentCells
                        && exceeded.max == exact_cells - 1
                        && exceeded.actual > exceeded.max
            ),
            "{profile:?} returned an unexpected error with {} cells: {error:?}",
            exact_cells - 1
        );
    }
}
