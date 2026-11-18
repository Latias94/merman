#![cfg(any(feature = "diagram-state", feature = "diagram-swimlane"))]

use merman_ascii::{
    AsciiFallbackReason, AsciiLayoutProfile, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourcePolicy, AsciiTrimPolicy, AsciiViewportPolicy,
    Lossiness, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::{OperationControl, resources::ResourceProfile, runtime::RuntimePolicy};
use unicode_width::UnicodeWidthStr;

fn assert_viewports(
    source: &str,
    family: &str,
    layout: AsciiLayoutProfile,
    primary_from: usize,
    complete_fields: &[&str],
) {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("the native family fixture should parse");
    assert_eq!(parsed.metadata.diagram_type, family);
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            for width in [60, 80, 100, 120] {
                let context = format!("{family} {:?}/{profile:?} at {width}", options.charset);
                let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
                let report = AsciiRenderer::new(
                    options
                        .with_layout_profile(layout)
                        .with_terminal_width_profile(profile),
                )
                .unwrap()
                .render_parsed_report(
                    &parsed,
                    AsciiViewportPolicy::with_max_width(width)
                        .overflow(OverflowPolicy::Fallback)
                        .trim(AsciiTrimPolicy::TrimTrailingSpaces),
                    &OperationControl::new(),
                    &operation,
                    AsciiResourcePolicy::for_profile(ResourceProfile::Interactive),
                )
                .unwrap_or_else(|error| panic!("{context} should render: {error}"));

                assert_eq!(report.family, parsed.metadata.diagram_type, "{context}");
                assert_eq!(report.width_profile, profile, "{context}");
                assert_eq!(report.requested_max_width, Some(width), "{context}");
                assert_eq!(report.requested_layout_profile, layout, "{context}");
                assert_eq!(report.layout_profile, Some(AsciiLayoutProfile::Canonical));
                let fallback = width < primary_from;
                let compact_attempted = layout == AsciiLayoutProfile::Auto && fallback;
                assert_eq!(report.compact_attempted, compact_attempted, "{context}");
                assert_eq!(report.fallback.attempted, fallback, "{context}");
                assert_eq!(report.primary_overflowed, Some(fallback), "{context}");
                assert!(!report.emitted_overflowed, "{context}");
                assert_eq!(
                    report.primary_extent.unwrap().width > width,
                    fallback,
                    "{context}"
                );
                if fallback {
                    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback, "{context}");
                    assert_eq!(report.projection, AsciiProjection::StructuredText);
                    assert_eq!(report.lossiness, Lossiness::PresentationOnly, "{context}");
                    assert_eq!(
                        report.fallback.reason,
                        Some(AsciiFallbackReason::PrimaryOverflow)
                    );
                } else {
                    assert_eq!(report.outcome, AsciiOutputOutcome::Primary, "{context}");
                    assert_eq!(report.projection, AsciiProjection::Diagrammatic);
                    assert_eq!(report.lossiness, Lossiness::None, "{context}");
                    assert_eq!(report.fallback.reason, None, "{context}");
                    assert_eq!(
                        report.primary_extent,
                        Some(report.emitted_extent),
                        "{context}"
                    );
                }

                let measured_width = report
                    .text
                    .lines()
                    .map(|line| match profile {
                        TerminalWidthProfile::Unicode => UnicodeWidthStr::width(line),
                        TerminalWidthProfile::Cjk => UnicodeWidthStr::width_cjk(line),
                        _ => unreachable!("the matrix selects known width profiles"),
                    })
                    .max()
                    .unwrap_or_default();
                assert!(
                    measured_width > 0 && measured_width <= width,
                    "{context}\n{}",
                    report.text
                );
                assert_eq!(report.emitted_extent.width, measured_width, "{context}");
                assert_eq!(report.emitted_extent.height, report.text.lines().count());

                // These fixtures have no meaningful whitespace distinctions. Keep every other
                // character, while accepting both word and word-internal fallback wraps.
                let collapse = |text: &str| text.split_whitespace().collect::<String>();
                let complete = collapse(&report.text);
                for field in complete_fields {
                    assert!(
                        complete.contains(&collapse(field)),
                        "{context}: missing complete {field:?}\n{}",
                        report.text
                    );
                }
            }
        }
    }
}

#[cfg(feature = "diagram-state")]
#[test]
fn single_region_state_keeps_its_compact_canonical_scene() {
    let source = concat!(
        "stateDiagram-v2\n[*] --> Active\nstate Active {\n[*] --> NumLockOff\n",
        "NumLockOff --> NumLockOn: EvNumLockPressed\n",
        "NumLockOn --> NumLockOff: EvNumLockPressed\n}\n",
    );
    let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            for width in [20, 60, 80, 100, 120] {
                let report = AsciiRenderer::new(options.with_terminal_width_profile(profile))
                    .unwrap()
                    .render_parsed_report(
                        &parsed,
                        AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Error),
                        &OperationControl::new(),
                        &operation,
                        AsciiResourcePolicy::for_profile(ResourceProfile::Interactive),
                    )
                    .expect("the validated original scene must not be expanded");
                assert_eq!(report.projection, AsciiProjection::Diagrammatic);
                assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
                assert_eq!(report.layout_profile, Some(AsciiLayoutProfile::Canonical));
                assert!(!report.compact_attempted && !report.fallback.attempted);
                assert_eq!(report.lossiness, Lossiness::None);
                assert_eq!(report.primary_extent, Some(report.emitted_extent));
                assert!(report.emitted_extent.width <= 20, "{}", report.text);
                assert!(report.emitted_extent.height <= 32, "{}", report.text);
                assert_eq!(report.emitted_extent.height, report.text.lines().count());
                let measured = report
                    .text
                    .lines()
                    .map(|line| match profile {
                        TerminalWidthProfile::Unicode => UnicodeWidthStr::width(line),
                        TerminalWidthProfile::Cjk => UnicodeWidthStr::width_cjk(line),
                        _ => unreachable!(),
                    })
                    .max()
                    .unwrap();
                assert_eq!(report.emitted_extent.width, measured);
                let mut wires = report.text.clone();
                for (field, count) in [
                    ("Active", 1),
                    ("NumLockOff", 1),
                    ("NumLockOn", 1),
                    ("EvNumLockPressed", 2),
                ] {
                    assert_eq!(wires.matches(field).count(), count, "{}", report.text);
                    wires = wires.replace(field, "");
                }
                assert_eq!(
                    wires
                        .chars()
                        .filter(|ch| matches!(ch, '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼'))
                        .count(),
                    4,
                    "{}",
                    report.text,
                );
                assert_eq!(
                    wires.chars().filter(|ch| matches!(ch, '*' | '●')).count(),
                    2
                );
                assert!(
                    !report.text.contains("root_start") && !report.text.contains("Active_start")
                );
            }
        }
    }
}

#[cfg(feature = "diagram-state")]
#[test]
fn nested_state_selects_canonical_or_complete_fallback_without_compact() {
    let source = concat!(
        "stateDiagram-v2\ndirection LR\nstate Session {\nstate Worker {\n",
        "Queued --> Running: dispatch\n}\nstate Monitor {\n",
        "Watching --> Alerted: alarm\n}\n}\nRunning --> Offline: close\n",
    );
    assert_viewports(
        source,
        "stateDiagram",
        AsciiLayoutProfile::Canonical,
        80,
        &[
            "Session", "Worker", "Queued", "Running", "Monitor", "Watching", "Alerted", "Offline",
            "dispatch", "alarm", "close",
        ],
    );
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_notes_and_full_conditions_fit_interactive_limits_without_compact() {
    let source = concat!(
        "stateDiagram-v2\ndirection LR\nstate Session {\ndirection TB\n[*] --> Ready\n",
        "Ready --> Running: permission granted for the complete production operation\n",
        "Running --> [*]: finished\n",
        "note right of Ready: Only authorized operators may start the operation\n",
        "}\nSession --> Closed: close the complete session\n",
    );
    assert_viewports(
        source,
        "stateDiagram",
        AsciiLayoutProfile::Canonical,
        120,
        &[
            "Session",
            "Ready",
            "Running",
            "Closed",
            "finished",
            "permission granted for the complete production operation",
            "Only authorized operators may start the operation",
            "close the complete session",
        ],
    );
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_auto_keeps_canonical_fallback_when_compact_handoff_ties() {
    let source = concat!(
        "swimlane-beta LR\nsubgraph Client[Client team]\nA[Request]\nend\n",
        "subgraph Operator[Operator team]\nB[Inspect]\nC[Resolve]\n",
        "B -->|diagnose| C\nend\nA -->|handoff| B\n",
    );
    // Shared Flowchart Compact ties Canonical in width; Auto keeps Canonical and falls back at 60.
    assert_viewports(
        source,
        "swimlane",
        AsciiLayoutProfile::Auto,
        80,
        &[
            "Client team",
            "Operator team",
            "Request",
            "Inspect",
            "Resolve",
            "diagnose",
            "handoff",
        ],
    );
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_group_and_member_targets_both_keep_primary_viewports() {
    for target in ["Lane", "C"] {
        let source = format!(
            "swimlane-beta LR\nsubgraph Lane[Review team]\nC[Check]\nend\nS[Submit]\nS -->|ready| {target}\n"
        );
        assert_viewports(
            &source,
            "swimlane",
            AsciiLayoutProfile::Auto,
            60,
            &["Review team", "Check", "Submit", "ready"],
        );
    }
}
