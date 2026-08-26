mod support;

use merman_ascii::{
    AsciiError, AsciiLayoutProfile, AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy,
    TerminalWidthProfile,
};
use merman_core::resources::ResourceProfile;
use support::{
    assert_rectangular_terminal_grid, assert_rectangular_terminal_grid_with_profile,
    local_semantic_input, parse_model, render_model, render_model_with_resources, terminal_extent,
    terminal_extent_with_profile,
};

const WIDTH_MATRIX: [usize; 4] = [60, 80, 100, 120];

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
    let canonical_blank_cells = canonical.bytes().filter(|byte| *byte == b' ').count();
    let compact_blank_cells = compact.bytes().filter(|byte| *byte == b' ').count();

    assert_eq!(canonical_extent, (74, 57));
    assert_eq!(compact_extent, (58, 67));
    assert_eq!(canonical_blank_cells, 3_220);
    assert_eq!(compact_blank_cells, 3_006);
    assert!(compact_extent.0 < canonical_extent.0);
    assert!(compact_extent.1 > canonical_extent.1);
    assert!(compact_extent.0 * compact_extent.1 < canonical_extent.0 * canonical_extent.1);
    assert!(compact_blank_cells < canonical_blank_cells);
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
