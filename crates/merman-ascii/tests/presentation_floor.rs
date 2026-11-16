#![cfg(feature = "diagram-flowchart")]

mod support;

use merman_ascii::{
    AsciiLayoutProfile, AsciiOutputOutcome, AsciiProjection, AsciiRenderOptions,
    AsciiViewportPolicy, OverflowPolicy, TerminalWidthProfile,
};
use support::{local_semantic_input, parse_model, render_model_report};

struct Case {
    name: &'static str,
    // Complete emitted documents: unbounded, then 60/80/100/120 terminal cells.
    rows: [usize; 5],
    fallback: [bool; 4],
    facts: &'static [&'static str],
}

// Original inputs and reading questions are frozen independently of reference binaries.
// The comparison note records which reference presentations retain those facts. These
// ceilings include every legend and detail row; they are not pixel-equivalence claims.
const CASES: &[Case] = &[
    Case {
        name: "chain",
        rows: [3; 5],
        fallback: [false; 4],
        facts: &["Read", "Parse", "Validate", "Plan", "Render"],
    },
    Case {
        name: "five-edge",
        rows: [6; 5],
        fallback: [false; 4],
        facts: &[" A ", " B ", " C ", " D "],
    },
    Case {
        name: "fork-merge",
        rows: [15; 5],
        fallback: [false; 4],
        facts: &["Start", "Left", "Right", "Finish"],
    },
    Case {
        name: "fan-out-12-lr",
        rows: [47; 5],
        fallback: [false; 4],
        facts: &["Dispatcher", "Job 1", "Job 12"],
    },
    Case {
        name: "fan-in-12-lr",
        rows: [47; 5],
        fallback: [false; 4],
        facts: &["Sink", "Source 1", "Source 12"],
    },
    Case {
        name: "fan-out-16-td",
        rows: [9, 20, 19, 19, 19],
        fallback: [true; 4],
        facts: &["Dispatcher", "Job 1", "Job 16"],
    },
    Case {
        name: "labeled-branches",
        rows: [18; 5],
        fallback: [false; 4],
        facts: &[
            "Request status",
            "valid",
            "invalid",
            "retry",
            "Execute",
            "Reject",
            "Queue",
            "Report",
        ],
    },
    Case {
        name: "feedback",
        rows: [7, 9, 7, 7, 7],
        fallback: [true, false, false, false],
        facts: &[
            "Input", "Parse", "Validate", "Write", "Done", "retry", "failed",
        ],
    },
    Case {
        name: "group-member-connection",
        rows: [9; 5],
        fallback: [false; 4],
        facts: &[
            "East", "West", "Ingress", "Worker", "Broker", "Store", "jobs",
        ],
    },
    Case {
        name: "group-nonterminal-member",
        rows: [23; 5],
        fallback: [false; 4],
        facts: &[
            "East", "West", "Ingress", "Worker", "Broker", "Store", "jobs",
        ],
    },
    Case {
        name: "long-labels",
        rows: [9, 13, 11, 11, 9],
        fallback: [true; 4],
        facts: &[
            "Approval is required before production writes",
            "The operation is read only",
        ],
    },
    Case {
        name: "distinct-suffixes",
        rows: [15, 23, 15, 15, 15],
        fallback: [false; 4],
        facts: &["PRODUCTION", "STAGING"],
    },
    Case {
        name: "cjk",
        rows: [7; 5],
        fallback: [false; 4],
        facts: &["校验结果", "通过", "写入队列", "失败", "返回错误"],
    },
];

#[test]
fn frozen_presentation_corpus_keeps_complete_documents_and_width_outcomes() {
    let widths = [None, Some(60), Some(80), Some(100), Some(120)];
    for case in CASES {
        let source = local_semantic_input(&format!("flowchart/presentation/{}.mmd", case.name));
        let model = parse_model(&source);
        for base in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                for (index, width) in widths.into_iter().enumerate() {
                    let options = base
                        .with_terminal_width_profile(profile)
                        .with_layout_profile(if width.is_some() {
                            AsciiLayoutProfile::Auto
                        } else {
                            AsciiLayoutProfile::Canonical
                        });
                    let viewport = width.map_or(AsciiViewportPolicy::unrestricted(), |width| {
                        AsciiViewportPolicy::with_max_width(width)
                            .overflow(OverflowPolicy::Fallback)
                    });
                    let report =
                        render_model_report(&model, &options, viewport).unwrap_or_else(|error| {
                            panic!("{}/{width:?}/{profile:?}: {error}", case.name)
                        });
                    let fallback = index > 0 && case.fallback[index - 1];
                    assert_eq!(
                        report.outcome,
                        if fallback {
                            AsciiOutputOutcome::Fallback
                        } else {
                            AsciiOutputOutcome::Primary
                        },
                        "{}/{width:?}",
                        case.name
                    );
                    assert_eq!(
                        report.projection,
                        if fallback {
                            AsciiProjection::StructuredText
                        } else {
                            AsciiProjection::Diagrammatic
                        }
                    );
                    assert!(
                        report.emitted_extent.height <= case.rows[index],
                        "{}/{width:?} exceeds the complete reading floor:\n{}",
                        case.name,
                        report.text
                    );
                    assert_eq!(report.emitted_extent.height, report.text.lines().count());
                    if let Some(width) = width {
                        assert!(report.emitted_extent.width <= width);
                    }
                    // The graph-document writer preserves literal text across physical wraps.
                    // The diagram corpus keeps these particular distinguishing facts unwrapped.
                    let complete = if fallback {
                        report
                            .text
                            .lines()
                            .map(|line| line.strip_prefix("  ").unwrap_or(line))
                            .collect::<String>()
                    } else {
                        report.text.clone()
                    };
                    for fact in case.facts {
                        assert!(
                            complete.contains(fact),
                            "{}/{width:?} lost {fact:?}:\n{}",
                            case.name,
                            report.text
                        );
                    }
                    if case.name.starts_with("fan-out-") || case.name.starts_with("fan-in-") {
                        let count = if case.name == "fan-out-16-td" { 16 } else { 12 };
                        let noun = if case.name.starts_with("fan-in-") {
                            "Source"
                        } else {
                            "Job"
                        };
                        for number in 1..=count {
                            assert!(
                                complete.contains(&format!("{noun} {number}")),
                                "missing terminal {number}: {}",
                                report.text
                            );
                        }
                    }
                }
            }
        }
    }
}
