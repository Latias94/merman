mod support;

use merman_ascii::{
    AsciiColorMode, AsciiError, AsciiLayoutProfile, AsciiOutputEncoding, AsciiOutputOutcome,
    AsciiProjection, AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy,
    AsciiViewportPolicy, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::resources::ResourceProfile;
use support::{
    assert_rectangular_terminal_grid, assert_rectangular_terminal_grid_with_profile,
    local_semantic_input, parse_model, render_model, render_model_report,
    render_model_with_resources, terminal_extent, terminal_extent_with_profile,
};

const WIDTH_MATRIX: [usize; 4] = [60, 80, 100, 120];
const ENCODING_MATRIX: [(AsciiColorMode, AsciiOutputEncoding); 5] = [
    (AsciiColorMode::Plain, AsciiOutputEncoding::Plain),
    (AsciiColorMode::Ansi16, AsciiOutputEncoding::Ansi16),
    (AsciiColorMode::Ansi256, AsciiOutputEncoding::Ansi256),
    (AsciiColorMode::TrueColor, AsciiOutputEncoding::TrueColor),
    (AsciiColorMode::Html, AsciiOutputEncoding::Html),
];

fn option_matrix() -> [AsciiRenderOptions; 4] {
    [
        AsciiRenderOptions::ascii(),
        AsciiRenderOptions::unicode(),
        AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk),
        AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
    ]
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for escaped in chars.by_ref() {
                if escaped == 'm' {
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn strip_html_spans(input: &str) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with("<span ") {
            index += rest.find('>').expect("span start tag should be closed") + 1;
            continue;
        }
        if rest.starts_with("</span>") {
            index += "</span>".len();
            continue;
        }
        let entities = [
            ("&gt;", '>'),
            ("&lt;", '<'),
            ("&amp;", '&'),
            ("&quot;", '"'),
            ("&#39;", '\''),
        ];
        if let Some((entity, decoded)) =
            entities.iter().find(|(entity, _)| rest.starts_with(entity))
        {
            output.push(*decoded);
            index += entity.len();
            continue;
        }
        let ch = rest
            .chars()
            .next()
            .expect("index should be on a char boundary");
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

fn plain_text_for_encoding(text: &str, encoding: AsciiOutputEncoding) -> String {
    match encoding {
        AsciiOutputEncoding::Plain => text.to_string(),
        AsciiOutputEncoding::Ansi16
        | AsciiOutputEncoding::Ansi256
        | AsciiOutputEncoding::TrueColor => strip_ansi(text),
        AsciiOutputEncoding::Html => strip_html_spans(text),
        _ => panic!("unexpected output encoding: {encoding:?}"),
    }
}

fn collapsed_authored_text(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '"')
        .collect()
}

fn blank_cell_metrics(rendered: &str) -> (usize, usize) {
    let blank_cells = rendered.bytes().filter(|byte| *byte == b' ').count();
    let longest_run = rendered
        .lines()
        .flat_map(|line| line.split(|ch| ch != ' '))
        .map(str::len)
        .max()
        .unwrap_or_default();
    (blank_cells, longest_run)
}

fn assert_issue_53_semantics(rendered: &str) {
    for expected in [
        "browser / agent",
        "*.docs.mysampleapp.net",
        "Route53 subzone",
        "delegated to",
        "mysampleapps account",
        "CloudFront distribution",
        "wildcard cert",
        "Host-to-prefix CF",
        "Function, WAF VPN",
        "allowlist, OAC to S3",
        "upload app",
        "Lambda,",
        "Python): GET / form,",
        "POST /upload, POST",
        "/api/deploy (IAM),",
        "IAM",
        "boto3",
        "S3 bucket mysampleapp",
        "sites/subdomain/",
        "DynamoDB reservations",
        "reads",
        "writes",
        "reserve / check owner",
        "CreateInvalidation",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?} in Issue #53 characterization output:\n{rendered}"
        );
    }
}

#[test]
fn issue_53_width_matrix_preserves_semantics_and_terminal_extent() {
    let source = local_semantic_input("flowchart/issue_53_long_node_labels.mmd");
    let model = parse_model(&source);
    let layouts = [
        ("canonical", AsciiLayoutProfile::Canonical, (74, 57)),
        ("compact", AsciiLayoutProfile::Compact, (58, 67)),
    ];

    for (layout_name, layout_profile, expected_extent) in layouts {
        let profiles = [
            ("ascii-unicode", AsciiRenderOptions::ascii()),
            ("unicode-unicode", AsciiRenderOptions::unicode()),
            (
                "ascii-cjk",
                AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk),
            ),
            (
                "unicode-cjk",
                AsciiRenderOptions::unicode()
                    .with_terminal_width_profile(TerminalWidthProfile::Cjk),
            ),
        ];

        for (profile_name, options) in profiles {
            let options = options.with_layout_profile(layout_profile);
            let rendered = render_model(&model, &options).unwrap_or_else(|error| {
                panic!("{layout_name}/{profile_name} Issue #53 render failed: {error}")
            });
            let (width, height) =
                terminal_extent_with_profile(&rendered, options.terminal_width_profile);

            assert_issue_53_semantics(&rendered);
            assert_eq!(
                (width, height),
                expected_extent,
                "{layout_name}/{profile_name} Issue #53 extent changed"
            );
            assert_rectangular_terminal_grid_with_profile(
                &rendered,
                options.terminal_width_profile,
            );

            let fits = WIDTH_MATRIX.map(|max_width| width <= max_width);
            assert!(
                fits.windows(2).all(|window| !window[0] || window[1]),
                "width-fit classification must be monotonic for {layout_name}/{profile_name}: width={width}, fits={fits:?}"
            );
            assert!(
                fits[3],
                "the 120-cell characterization bound must contain {layout_name}/{profile_name} output"
            );
        }
    }
}

#[test]
fn issue_53_compact_candidate_reduces_width_and_total_blank_cells() {
    let source = local_semantic_input("flowchart/issue_53_long_node_labels.mmd");
    let model = parse_model(&source);
    let canonical = render_model(&model, &AsciiRenderOptions::ascii())
        .expect("canonical Issue #53 fixture should render");
    let compact = render_model(
        &model,
        &AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Compact),
    )
    .expect("compact Issue #53 fixture should preserve route clearance");

    let canonical_extent = terminal_extent(&canonical);
    let compact_extent = terminal_extent(&compact);
    let (canonical_blank_cells, canonical_longest_blank_run) = blank_cell_metrics(&canonical);
    let (compact_blank_cells, compact_longest_blank_run) = blank_cell_metrics(&compact);

    assert_eq!(canonical_extent, (74, 57));
    assert_eq!(compact_extent, (58, 67));
    assert_eq!(canonical_blank_cells, 3_220);
    assert_eq!(compact_blank_cells, 3_006);
    assert_eq!(canonical_longest_blank_run, 51);
    assert_eq!(compact_longest_blank_run, 43);
    assert!(compact_extent.0 < canonical_extent.0);
    assert!(compact_extent.1 > canonical_extent.1);
    assert!(compact_extent.0 * compact_extent.1 < canonical_extent.0 * canonical_extent.1);
    assert!(compact_blank_cells < canonical_blank_cells);
    assert!(compact_longest_blank_run < canonical_longest_blank_run);
}

#[test]
fn sequence_compact_candidate_reduces_width_area_and_blank_cells() {
    let source = local_semantic_input("sequence/self_messages_with_notes.mmd");
    let model = parse_model(&source);
    let canonical = render_model(&model, &AsciiRenderOptions::ascii())
        .expect("canonical Sequence evidence fixture should render");
    let compact = render_model(
        &model,
        &AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Compact),
    )
    .expect("compact Sequence evidence fixture should render");

    let canonical_extent = terminal_extent(&canonical);
    let compact_extent = terminal_extent(&compact);
    let (canonical_blank_cells, canonical_longest_blank_run) = blank_cell_metrics(&canonical);
    let (compact_blank_cells, compact_longest_blank_run) = blank_cell_metrics(&compact);

    assert_eq!(canonical_extent, (82, 58));
    assert_eq!(compact_extent, (78, 58));
    assert_eq!(canonical_blank_cells, 3_227);
    assert_eq!(compact_blank_cells, 2_982);
    assert_eq!(canonical_longest_blank_run, 20);
    assert_eq!(compact_longest_blank_run, 21);
    assert!(compact_extent.0 < canonical_extent.0);
    assert_eq!(compact_extent.1, canonical_extent.1);
    assert!(compact_extent.0 * compact_extent.1 < canonical_extent.0 * canonical_extent.1);
    assert!(compact_blank_cells < canonical_blank_cells);
}

#[test]
fn flowchart_and_sequence_encoding_matrix_preserves_plain_geometry() {
    let cases = [
        (
            "flowchart/issue_53_long_node_labels.mmd",
            &[
                "browser / agent",
                "CreateInvalidation",
                "DynamoDB reservations",
            ][..],
        ),
        (
            "sequence/self_messages_with_notes.mmd",
            &[
                "Main Process",
                "event.preventDefault()",
                "WINDOW_CLOSE_REQUESTED",
                "window.destroy()",
            ][..],
        ),
    ];

    for (fixture, required_fields) in cases {
        let model = parse_model(&local_semantic_input(fixture));
        for layout_profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
            for base_options in option_matrix() {
                let plain_options = base_options
                    .with_layout_profile(layout_profile)
                    .with_color_mode(AsciiColorMode::Plain);
                let plain =
                    render_model_report(&model, &plain_options, AsciiViewportPolicy::default())
                        .unwrap_or_else(|error| {
                            panic!("plain {fixture} render failed for {plain_options:?}: {error}")
                        });

                for field in required_fields {
                    assert!(
                        plain.text.contains(field),
                        "plain {fixture} lost authored field {field:?}:\n{}",
                        plain.text
                    );
                }

                for (color_mode, encoding) in ENCODING_MATRIX {
                    let options = plain_options.with_color_mode(color_mode);
                    let report =
                        render_model_report(&model, &options, AsciiViewportPolicy::default())
                            .unwrap_or_else(|error| {
                                panic!(
                                    "{encoding:?} {fixture} render failed for {options:?}: {error}"
                                )
                            });

                    assert_eq!(report.encoding, encoding, "{fixture}/{options:?}");
                    assert_eq!(
                        report.primary_extent, plain.primary_extent,
                        "{fixture}/{options:?} primary extent drifted"
                    );
                    assert_eq!(
                        report.emitted_extent, plain.emitted_extent,
                        "{fixture}/{options:?} emitted extent drifted"
                    );
                    assert_eq!(
                        plain_text_for_encoding(&report.text, encoding),
                        plain.text,
                        "{fixture}/{options:?} styled bytes changed semantic text"
                    );
                }
            }
        }
    }
}

#[test]
fn flowchart_and_sequence_width_policy_matrix_is_typed_and_complete() {
    let cases = [
        (
            "flowchart/issue_53_long_node_labels.mmd",
            &[
                "browser / agent",
                "CreateInvalidation",
                "reserve / check owner",
            ][..],
        ),
        (
            "sequence/self_messages_with_notes.mmd",
            &[
                "Main Process",
                "event.preventDefault()",
                "WINDOW_CLOSE_REQUESTED",
                "window.destroy()",
            ][..],
        ),
    ];

    for (fixture, required_fields) in cases {
        let model = parse_model(&local_semantic_input(fixture));
        for layout_profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
            for base_options in option_matrix() {
                let plain_options = base_options
                    .with_layout_profile(layout_profile)
                    .with_color_mode(AsciiColorMode::Plain);
                let baseline =
                    render_model_report(&model, &plain_options, AsciiViewportPolicy::default())
                        .unwrap_or_else(|error| {
                            panic!(
                                "baseline {fixture} render failed for {plain_options:?}: {error}"
                            )
                        });

                for max_width in WIDTH_MATRIX {
                    let overflowed = baseline.primary_extent.width > max_width;
                    for (color_mode, encoding) in [
                        (AsciiColorMode::Plain, AsciiOutputEncoding::Plain),
                        (AsciiColorMode::Ansi16, AsciiOutputEncoding::Ansi16),
                    ] {
                        let options = plain_options.with_color_mode(color_mode);
                        let allow = render_model_report(
                            &model,
                            &options,
                            AsciiViewportPolicy::with_max_width(max_width)
                                .overflow(OverflowPolicy::Allow),
                        )
                        .unwrap_or_else(|error| {
                            panic!(
                                "Allow {fixture} render failed for {options:?}, width={max_width}: {error}"
                            )
                        });
                        assert_eq!(allow.encoding, encoding);
                        assert_eq!(allow.primary_extent, baseline.primary_extent);
                        assert_eq!(allow.emitted_extent, baseline.emitted_extent);
                        assert_eq!(allow.overflowed, overflowed);
                        assert_eq!(
                            allow.outcome,
                            if overflowed {
                                AsciiOutputOutcome::WideAllowed
                            } else {
                                AsciiOutputOutcome::Primary
                            }
                        );
                        assert_eq!(
                            plain_text_for_encoding(&allow.text, encoding),
                            baseline.text
                        );

                        let error_result = render_model_report(
                            &model,
                            &options,
                            AsciiViewportPolicy::with_max_width(max_width)
                                .overflow(OverflowPolicy::Error),
                        );
                        if overflowed {
                            assert!(
                                matches!(
                                    error_result,
                                    Err(AsciiError::WidthOverflow {
                                        max_width: actual_max,
                                        actual_width,
                                        ..
                                    }) if actual_max == max_width
                                        && actual_width == baseline.primary_extent.width
                                ),
                                "Error {fixture}/{options:?}/width={max_width} produced {error_result:?}"
                            );
                        } else {
                            let report = error_result.unwrap_or_else(|error| {
                                panic!(
                                    "fitting Error {fixture}/{options:?}/width={max_width} failed: {error}"
                                )
                            });
                            assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
                            assert_eq!(report.encoding, encoding);
                            assert_eq!(report.emitted_extent, baseline.emitted_extent);
                        }
                    }

                    let fallback = render_model_report(
                        &model,
                        &plain_options,
                        AsciiViewportPolicy::with_max_width(max_width)
                            .overflow(OverflowPolicy::Fallback),
                    )
                    .unwrap_or_else(|error| {
                        panic!(
                            "Fallback {fixture} render failed for {plain_options:?}, width={max_width}: {error}"
                        )
                    });
                    assert_eq!(fallback.encoding, AsciiOutputEncoding::Plain);
                    assert_eq!(fallback.primary_extent, baseline.primary_extent);
                    if overflowed {
                        assert_eq!(fallback.outcome, AsciiOutputOutcome::Fallback);
                        assert_eq!(fallback.projection, AsciiProjection::StructuredText);
                        assert!(fallback.fallback.attempted);
                        assert!(fallback.emitted_extent.width <= max_width);
                        let collapsed = collapsed_authored_text(&fallback.text);
                        for field in required_fields {
                            assert!(
                                collapsed.contains(&collapsed_authored_text(field)),
                                "fallback {fixture}/width={max_width} lost {field:?}:\n{}",
                                fallback.text
                            );
                        }
                    } else {
                        assert_eq!(fallback.outcome, AsciiOutputOutcome::Primary);
                        assert!(!fallback.fallback.attempted);
                        assert_eq!(fallback.text, baseline.text);
                    }
                }

                for (color_mode, _) in ENCODING_MATRIX.into_iter().skip(1) {
                    let styled = plain_options.with_color_mode(color_mode);
                    let error = render_model_report(
                        &model,
                        &styled,
                        AsciiViewportPolicy::with_max_width(WIDTH_MATRIX[0])
                            .overflow(OverflowPolicy::Fallback),
                    )
                    .expect_err("styled fallback must fail capability preflight");
                    assert_eq!(
                        error,
                        AsciiError::InvalidOption {
                            field: "ascii_viewport.overflow",
                            message: "fallback is not admitted for the selected output encoding",
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn compact_flowchart_route_and_group_corpus_preserves_authored_fields() {
    let cases: [(&str, &[&str]); 9] = [
        (
            "flowchart/ampersand_fanin_fanout.mmd",
            &[
                "SourceA", "SourceB", "Merge", "Fanout", "TargetA", "TargetB",
            ],
        ),
        (
            "flowchart/back_edge_labels.mmd",
            &["back to top", "back to middle"],
        ),
        (
            "flowchart/boundary_label_lane.mmd",
            &["Pipeline", "boundaryLabelWithEnoughWidth", "Success"],
        ),
        (
            "flowchart/cjk_boundary_routes.mmd",
            &["入口", "流程中枢", "校验层", "验证", "发布", "完成"],
        ),
        (
            "flowchart/disconnected_subgraphs.mmd",
            &["Today", "Today Markdown", "Next Wave", "Next Widget"],
        ),
        (
            "flowchart/multi_boundary_routes.mmd",
            &["load", "check", "Pipeline", "ok", "fail", "Retry"],
        ),
        ("flowchart/multiline_edge_label.mmd", &["north", "south"]),
        (
            "flowchart/nested_direction_boundary.mmd",
            &[
                "Outer Pipeline",
                "Inner Steps",
                "Validate",
                "Persist",
                "Done",
            ],
        ),
        (
            "flowchart/sibling_boundary_routes.mmd",
            &["Left Group", "Right Group", "Alpha", "Delta", "handoff"],
        ),
    ];
    let profiles = [
        AsciiRenderOptions::ascii(),
        AsciiRenderOptions::unicode(),
        AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk),
        AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
    ];

    for (fixture, expected_fields) in cases {
        let model = parse_model(&local_semantic_input(fixture));
        for options in profiles {
            let options = options.with_layout_profile(AsciiLayoutProfile::Compact);
            let rendered = render_model(&model, &options).unwrap_or_else(|error| {
                panic!("compact {fixture} render failed for {options:?}: {error}")
            });
            assert_rectangular_terminal_grid_with_profile(
                &rendered,
                options.terminal_width_profile,
            );
            for field in expected_fields {
                assert!(
                    rendered.contains(field),
                    "compact {fixture} lost authored field {field:?}:\n{rendered}"
                );
            }
        }
    }
}

#[test]
fn compact_sequence_corpus_preserves_lifecycle_controls_and_terminal_extents() {
    struct Case {
        fixture: &'static str,
        required_fields: &'static [&'static str],
        canonical_extent: (usize, usize),
        compact_extent: (usize, usize),
    }

    let cases = [
        Case {
            fixture: "sequence/dense_control_rows.mmd",
            required_fields: &[
                "Start",
                "Coordinate",
                "Parallel Branches",
                "Fallback",
                "Ship",
                "Return",
                "Retry",
                "Stop",
            ],
            canonical_extent: (39, 30),
            compact_extent: (37, 30),
        },
        Case {
            fixture: "sequence/multiple_messages.mmd",
            required_fields: &[
                "Alice", "Bob", "Charlie", "Hello", "Forward", "Reply", "Done",
            ],
            canonical_extent: (37, 16),
            compact_extent: (33, 16),
        },
        Case {
            fixture: "sequence/self_messages_with_notes.mmd",
            required_fields: &[
                "User",
                "Main Process",
                "Renderer",
                "3s Fallback Timer",
                "event.preventDefault()",
                "WINDOW_CLOSE_REQUESTED",
                "Multiple panels",
                "Single panel",
                "Panel removed",
                "window.destroy()",
                "Panel reopens",
            ],
            canonical_extent: (82, 58),
            compact_extent: (78, 58),
        },
    ];
    let profiles = [
        AsciiRenderOptions::ascii(),
        AsciiRenderOptions::unicode(),
        AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk),
        AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
    ];

    for case in cases {
        let model = parse_model(&local_semantic_input(case.fixture));
        for options in profiles {
            let canonical = render_model(&model, &options).unwrap_or_else(|error| {
                panic!(
                    "canonical {} render failed for {options:?}: {error}",
                    case.fixture
                )
            });
            let compact_options = options.with_layout_profile(AsciiLayoutProfile::Compact);
            let compact = render_model(&model, &compact_options).unwrap_or_else(|error| {
                panic!(
                    "compact {} render failed for {compact_options:?}: {error}",
                    case.fixture
                )
            });

            let canonical_extent =
                terminal_extent_with_profile(&canonical, options.terminal_width_profile);
            let compact_extent =
                terminal_extent_with_profile(&compact, compact_options.terminal_width_profile);
            assert_eq!(
                canonical_extent, case.canonical_extent,
                "canonical {} extent changed for {options:?}",
                case.fixture
            );
            assert_eq!(
                compact_extent, case.compact_extent,
                "compact {} extent changed for {compact_options:?}",
                case.fixture
            );
            assert!(compact_extent.0 < canonical_extent.0, "{}", case.fixture);
            assert_eq!(compact_extent.1, canonical_extent.1, "{}", case.fixture);
            assert!(
                compact_extent.0 * compact_extent.1 < canonical_extent.0 * canonical_extent.1,
                "{}",
                case.fixture
            );

            for field in case.required_fields {
                assert!(
                    canonical.contains(field),
                    "canonical {} lost authored field {field:?}:\n{canonical}",
                    case.fixture
                );
                assert!(
                    compact.contains(field),
                    "compact {} lost authored field {field:?}:\n{compact}",
                    case.fixture
                );
            }
        }
    }
}

#[test]
fn cjk_width_profile_changes_only_the_measured_extent() {
    let source = "flowchart TD\nA[\"A·B C\"]";
    let model = parse_model(source);
    let unicode = render_model(&model, &AsciiRenderOptions::unicode())
        .expect("Unicode profile should render the ambiguous-width fixture");
    let cjk = render_model(
        &model,
        &AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
    )
    .expect("CJK profile should render the ambiguous-width fixture");

    assert!(unicode.contains("A·B"), "label prefix was lost:\n{unicode}");
    assert!(unicode.contains('C'), "label suffix was lost:\n{unicode}");
    assert!(cjk.contains("A·B"), "label prefix was lost:\n{cjk}");
    assert!(cjk.contains('C'), "label suffix was lost:\n{cjk}");
    assert_rectangular_terminal_grid(&unicode);
    assert_rectangular_terminal_grid_with_profile(&cjk, TerminalWidthProfile::Cjk);

    let (unicode_width, unicode_height) =
        support::terminal_extent_with_profile(&unicode, TerminalWidthProfile::Unicode);
    let (cjk_width, cjk_height) =
        support::terminal_extent_with_profile(&cjk, TerminalWidthProfile::Cjk);
    assert!(
        cjk_width >= unicode_width && cjk_height >= unicode_height,
        "CJK width changes wrapping/extent only in the selected profile:\nunicode:\n{unicode}\ncjk:\n{cjk}"
    );
}

#[test]
fn representative_finalizers_expose_stable_extents_and_semantic_fields() {
    struct Case {
        name: &'static str,
        source: String,
        required_fields: &'static [&'static str],
        rectangular: bool,
        expected_extent: (usize, usize),
    }

    let cases = [
        Case {
            name: "sequence",
            source: local_semantic_input("sequence/dense_control_rows.mmd"),
            required_fields: &[
                "Start",
                "Coordinate",
                "Parallel Branches",
                "Fallback",
                "Retry",
                "Stop",
            ],
            rectangular: false,
            expected_extent: (39, 30),
        },
        Case {
            name: "class",
            source: local_semantic_input("class/dense_multiline_relations.mmd"),
            required_fields: &[
                "Gateway",
                "Service",
                "receives",
                "request",
                "invalidates",
                "entry",
            ],
            rectangular: false,
            expected_extent: (91, 41),
        },
        Case {
            name: "er",
            source: local_semantic_input("er/dense_multiline_relations.mmd"),
            required_fields: &["CUSTOMER", "ORDER", "places", "orders", "settles", "order"],
            rectangular: false,
            expected_extent: (97, 41),
        },
        Case {
            name: "xychart",
            source: local_semantic_input("xychart/horizontal_mixed_cjk.mmd"),
            required_fields: &["营收", "北区", "南区", "分数", "4"],
            rectangular: false,
            expected_extent: (270, 10),
        },
        Case {
            name: "structured-text",
            source: "timeline\ntitle Basic\n2024 : Event".to_string(),
            required_fields: &["Basic", "Event"],
            rectangular: false,
            expected_extent: (28, 4),
        },
    ];

    for case in cases {
        let model = parse_model(&case.source);
        for (charset_name, options) in [
            ("ascii", AsciiRenderOptions::ascii()),
            ("unicode", AsciiRenderOptions::unicode()),
        ] {
            let rendered = render_model(&model, &options).unwrap_or_else(|error| {
                panic!(
                    "{} ({charset_name}) characterization render failed: {error}",
                    case.name
                )
            });
            let (width, height) = terminal_extent(&rendered);
            assert_eq!(
                (width, height),
                case.expected_extent,
                "{} ({charset_name}) extent changed",
                case.name
            );

            assert!(
                width > 0,
                "{} ({charset_name}) width must be positive",
                case.name
            );
            assert!(
                height > 0,
                "{} ({charset_name}) height must be positive",
                case.name
            );
            for field in case.required_fields {
                assert!(
                    rendered.contains(field),
                    "{} ({charset_name}) lost authored field {field:?}:\n{rendered}",
                    case.name
                );
            }
            if case.rectangular {
                assert_rectangular_terminal_grid(&rendered);
            }

            let fits = WIDTH_MATRIX.map(|max_width| width <= max_width);
            assert!(
                fits.windows(2).all(|window| !window[0] || window[1]),
                "{} extent-fit classification must be monotonic: width={width}, fits={fits:?}",
                case.name
            );
        }
    }
}

#[test]
fn issue_53_exact_and_minus_one_grid_boundaries_remain_resource_outcomes() {
    let source = local_semantic_input("flowchart/issue_53_long_node_labels.mmd");
    let model = parse_model(&source);
    let options = AsciiRenderOptions::ascii();
    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let rendered = render_model_with_resources(&model, &options, unbounded)
        .expect("unbounded Issue #53 fixture should render");
    let (width, height) = terminal_extent(&rendered);
    let exact_cells = width * height;
    assert!(
        exact_cells > 1,
        "fixture should consume more than one grid cell"
    );

    let exact_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells)
        .expect("exact grid limit should be valid");
    render_model_with_resources(&model, &options, exact_policy)
        .expect("exact Issue #53 grid boundary should remain accepted");

    let below_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells - 1)
        .expect("max-minus-one grid limit should be valid");
    let error = render_model_with_resources(&model, &options, below_policy)
        .expect_err("max-minus-one Issue #53 grid boundary should fail");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected a grid resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
    assert_eq!(details.actual, exact_cells);
    assert_eq!(details.max, exact_cells - 1);
}
