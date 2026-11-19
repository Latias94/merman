#![cfg(feature = "diagram-usecase")]

mod support;

use merman_ascii::{
    AsciiDiagram, AsciiError, AsciiFallbackReason, AsciiGraphPresentation, AsciiLayoutProfile,
    AsciiOutput, AsciiOutputOutcome, AsciiProjection, AsciiRenderOptions, AsciiRenderer,
    AsciiResourceLimitId, AsciiResourcePolicy, AsciiSourceContext, AsciiViewportPolicy, Lossiness,
    OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::usecase::{
    UsecaseActorType, UsecaseAnimation, UsecaseArrowType, UsecaseBoundary, UsecaseDiagramModel,
    UsecaseJsonInfinity, UsecaseJsonStringEncoding, UsecaseLabelType, UsecaseRelationshipType,
};
use merman_model::{
    MermaidConfig, OperationControl, SemanticDiagramModel, resources::ResourceProfile,
    runtime::RuntimePolicy,
};

fn parsed(source: &str) -> merman_model::ParsedSemanticDiagram {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("Usecase should parse");
    assert_eq!(parsed.metadata.diagram_type, "usecase");
    parsed
}

fn model(source: &str) -> UsecaseDiagramModel {
    let SemanticDiagramModel::Usecase(model) = parsed(source).model else {
        panic!("expected the canonical Usecase family");
    };
    model
}

fn report(
    source: &str,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?.render_parsed_report(
        &parsed(source),
        viewport,
        &OperationControl::new(),
        &operation,
        AsciiResourcePolicy::for_profile(ResourceProfile::Interactive),
    )
}

fn direct_report(
    model: &UsecaseDiagramModel,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
    control: &OperationControl,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    let config = MermaidConfig::empty_object();
    AsciiRenderer::new(options)?.render_model_report(
        &AsciiDiagram::Usecase(model),
        AsciiSourceContext {
            diagram_type: "usecase",
            config: &config,
            semantic_config: &config,
            title: None,
        },
        viewport,
        control,
        &operation,
        resources,
    )
}

fn render_direct(model: &UsecaseDiagramModel) -> merman_ascii::Result<AsciiOutput> {
    direct_report(
        model,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
}

fn assert_report_measurement(report: &AsciiOutput, profile: TerminalWidthProfile) {
    let (width, height) = support::terminal_extent_with_profile(&report.text, profile);
    assert_eq!(report.family, "usecase");
    assert_eq!(
        (report.emitted_extent.width, report.emitted_extent.height),
        (width, height)
    );
    assert_eq!(report.width_profile, profile);
    assert_eq!(
        report.requested_graph_presentation,
        AsciiGraphPresentation::Strict
    );
    assert_eq!(
        report.requested_layout_profile,
        AsciiLayoutProfile::Canonical
    );
    assert!(!report.compact_attempted);
}

fn assert_json_table_rows(report: &AsciiOutput, expected: &[(&str, &str)]) {
    assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
    assert_eq!(report.projection, AsciiProjection::Diagrammatic);
    assert_eq!(report.lossiness, Lossiness::None);
    assert_report_measurement(report, report.width_profile);
    support::assert_rectangular_terminal_grid_with_profile(&report.text, report.width_profile);
    let top = report
        .text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap()
        .trim();
    assert!(
        (top.starts_with('+') && top.ends_with('+'))
            || (top.starts_with('┌') && top.ends_with('┐')),
        "a JSON table must have a rectangular frame:\n{}",
        report.text
    );
    assert_json_table_body_rows(report, expected);
}

fn assert_json_table_body_rows(report: &AsciiOutput, expected: &[(&str, &str)]) {
    let (header_y, header) = report
        .text
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.split_once(" | Value").map(|(left, _)| (y, left)))
        .expect("a table must have a Path | Value header");
    assert!(header.trim_end().ends_with("Path"), "{}", report.text);
    let column = support::terminal_extent_with_profile(header, report.width_profile).0 + 1;
    let rows: Vec<_> = report
        .text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains('$'))
        .collect();
    assert_eq!(rows.len(), expected.len(), "{}", report.text);
    for (index, ((y, row), &(path, value))) in rows.into_iter().zip(expected).enumerate() {
        assert_eq!(
            y,
            header_y + index + 1,
            "table body rows must be adjacent in source order:\n{}",
            report.text
        );
        let start = row.find('$').unwrap();
        let suffix = row[start..]
            .strip_prefix(path)
            .unwrap_or_else(|| panic!("expected {path}:\n{}", report.text));
        let (padding, actual) = suffix.split_once(" | ").expect("two table columns");
        assert!(padding.chars().all(|ch| ch == ' '), "{row}");
        let left = &row[..start + path.len() + padding.len() + 1];
        assert_eq!(
            support::terminal_extent_with_profile(left, report.width_profile).0,
            column,
            "table separators must share one terminal column:\n{}",
            report.text
        );
        let trailing = actual
            .strip_prefix(value)
            .unwrap_or_else(|| panic!("expected {path} | {value}:\n{}", report.text))
            .trim_start_matches(' ');
        assert!(
            matches!(
                trailing.chars().next(),
                Some('|' | '│' | '+' | '├' | '┤' | '┼')
            ),
            "a table value must end at its rectangular frame: {path}\n{}",
            report.text
        );
    }
}

#[test]
fn small_and_typical_are_spatial_at_80_100_120_for_both_charsets_and_width_profiles() {
    for (fixture, labels) in [
        (
            "small.mmd",
            &["Customer", "Place order", "[actor]", "[use case]"][..],
        ),
        (
            "typical.mmd",
            &[
                "Customer",
                "Sign in",
                "Check",
                "Remember",
                "Authentication",
                "[package]",
                "<<include>>",
                "[business]",
                "Human",
            ][..],
        ),
        (
            "cjk.mmd",
            &["客户", "订单系统", "提交订单", "确认付款", "核对金额👩‍💻"][..],
        ),
    ] {
        let source = support::local_semantic_input(&format!("usecase/{fixture}"));
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                for width in [80, 100, 120] {
                    let report = report(
                        &source,
                        options.with_terminal_width_profile(profile),
                        AsciiViewportPolicy::with_max_width(width)
                            .overflow(OverflowPolicy::Fallback),
                    )
                    .unwrap_or_else(|error| panic!("{fixture} {profile:?}/{width}: {error}"));
                    assert_report_measurement(&report, profile);
                    assert_eq!(
                        report.outcome,
                        AsciiOutputOutcome::Primary,
                        "{fixture}\n{}",
                        report.text
                    );
                    assert_eq!(report.projection, AsciiProjection::Diagrammatic);
                    assert_eq!(report.primary_extent, Some(report.emitted_extent));
                    assert_eq!(report.primary_overflowed, Some(false));
                    assert!(!report.emitted_overflowed);
                    assert!(!report.fallback.attempted);
                    assert_eq!(report.lossiness, Lossiness::None);
                    assert!(report.emitted_extent.width <= width);
                    support::assert_rectangular_terminal_grid_with_profile(&report.text, profile);
                    for &label in labels {
                        assert!(
                            report.text.contains(label),
                            "missing {label}:\n{}",
                            report.text
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn full_and_canonical_model_entries_share_the_family_projection() {
    let source = support::local_semantic_input("usecase/typical.mmd");
    let full = support::parse_model(&source);
    let options = AsciiRenderOptions::ascii();
    let rendered = support::render_model(&full, &options)
        .expect("the full model variant should route to Usecase");
    assert_eq!(
        rendered,
        report(&source, options, AsciiViewportPolicy::unrestricted())
            .unwrap()
            .text
    );
}

#[test]
fn direction4_uses_graph_transforms_without_reversing_labels_or_endpoint_markers() {
    for (direction, horizontal, reversed, marker) in [
        ("LR", true, false, '>'),
        ("RL", true, true, '<'),
        ("TB", false, false, 'v'),
        ("BT", false, true, '^'),
    ] {
        let source =
            format!("usecase-beta\ndirection {direction}\nactor A(Alpha)\nB(Beta)\nA --> B");
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                let report = report(
                    &source,
                    options.with_terminal_width_profile(profile),
                    AsciiViewportPolicy::unrestricted(),
                )
                .unwrap();
                let position = |label: &str| {
                    report
                        .text
                        .lines()
                        .enumerate()
                        .find_map(|(y, row)| row.find(label).map(|x| (x, y)))
                        .unwrap()
                };
                let a = position("Alpha");
                let b = position("Beta");
                let increasing = if horizontal { a.0 < b.0 } else { a.1 < b.1 };
                assert_eq!(increasing, !reversed, "{direction}\n{}", report.text);
                let unicode_marker = match marker {
                    '>' => '►',
                    '<' => '◄',
                    'v' => '▼',
                    _ => '▲',
                };
                assert!(
                    report.text.contains(marker) || report.text.contains(unicode_marker),
                    "{direction}\n{}",
                    report.text
                );
                support::assert_rectangular_terminal_grid_with_profile(&report.text, profile);
            }
        }
    }
}

#[test]
fn dense_strict_reports_a_valid_spatial_scene_or_an_accurate_routing_failure() {
    let source = support::local_semantic_input("usecase/dense.mmd");
    let typed = model(&source);
    let edge_count = typed.relationships.len() + typed.notes.len();
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            for width in [80, 100, 120] {
                match report(
                    &source,
                    options.with_terminal_width_profile(profile),
                    AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
                ) {
                    Ok(report) => {
                        assert_report_measurement(&report, profile);
                        assert!(!report.emitted_overflowed);
                        assert!(report.emitted_extent.width <= width);
                        for label in [
                            "Customer",
                            "Auditor",
                            "Browse",
                            "Checkout",
                            "Payment",
                            "Receipt",
                            "Review payment",
                        ] {
                            assert!(
                                report.text.contains(label),
                                "missing {label}:\n{}",
                                report.text
                            );
                        }
                        if report.outcome == AsciiOutputOutcome::Fallback {
                            assert_eq!(report.projection, AsciiProjection::StructuredText);
                            assert_eq!(
                                report.fallback.reason,
                                Some(AsciiFallbackReason::PrimaryOverflow)
                            );
                            assert_eq!(report.primary_overflowed, Some(true));
                            assert_eq!(report.lossiness, Lossiness::PresentationOnly);
                            assert!(report.text.contains("E9:"), "{}", report.text);
                        } else {
                            assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
                            assert_eq!(report.projection, AsciiProjection::Diagrammatic);
                            assert!(!report.fallback.attempted);
                        }
                    }
                    Err(AsciiError::RoutingSearchExhausted {
                        diagram_type,
                        edge_index,
                    }) => {
                        assert_eq!(diagram_type, "usecase");
                        assert!(edge_index < edge_count);
                    }
                    Err(error) => {
                        panic!("dense strict must not hide unrelated failures: {error:?}")
                    }
                }
            }
        }
    }
}

#[test]
fn all_arrow_markers_and_relationship_roles_are_visible() {
    for (arrow, marker) in [
        ("-->", ">"),
        ("<--", "<"),
        ("---", ""),
        ("--o", "o"),
        ("--x", "x"),
        ("o--", "o"),
        ("x--", "x"),
    ] {
        let source = format!("usecase-beta\nA(Alpha)\nB(Beta)\nA {arrow} B");
        let report = report(
            &source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        assert!(report.text.contains("Alpha") && report.text.contains("Beta"));
        if !marker.is_empty() {
            assert!(report.text.contains(marker), "{arrow}\n{}", report.text);
            let label_x =
                |label: &str| report.text.lines().find_map(|row| row.find(label)).unwrap();
            let marker_x = label_x(marker);
            let midpoint = (label_x("Alpha") + label_x("Beta")) / 2;
            assert_eq!(
                marker_x <= midpoint,
                matches!(arrow, "<--" | "o--" | "x--"),
                "the marker must stay at its written endpoint for {arrow}:\n{}",
                report.text
            );
        }
        if arrow == "---" {
            assert!(
                !report.text.contains(['>', '<', 'x', 'o']),
                "{}",
                report.text
            );
        }
    }
    for (arrow, label, dotted) in [
        ("..>:include", "<<include>>", true),
        ("..>:extend", "<<extend>>", true),
        ("--|>", "<<generalization>>", false),
    ] {
        let source = format!("usecase-beta\nA(Alpha)\nB(Beta)\nA {arrow} B");
        let report = report(
            &source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        assert!(report.text.contains(label), "{}", report.text);
        if dotted {
            assert!(report.text.contains('.'), "{}", report.text);
        }
    }
}

#[test]
fn node_metadata_notes_and_boundary_containment_survive_small_spatial_scenes() {
    let source = "usecase-beta\ndirection TB\nactor Owner@{icon:\"fa:user\"} <<Human>>\nsystemBoundary S(System)\nAction(Work)\nend\nOwner --> Action\nnote for Action \"Note text\"";
    let report = report(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    for label in [
        "[actor: icon]",
        "fa:user",
        "[stereotype]",
        "Human",
        "[boundary]",
        "[note]",
        "Note text",
    ] {
        assert!(
            report.text.contains(label),
            "missing {label}:\n{}",
            report.text
        );
    }
    let system_row = report
        .text
        .lines()
        .position(|row| row.contains("System"))
        .unwrap();
    let work_row = report
        .text
        .lines()
        .position(|row| row.contains("Work"))
        .unwrap();
    let note_row = report
        .text
        .lines()
        .position(|row| row.contains("Note text"))
        .unwrap();
    assert!(
        system_row < work_row && system_row < note_row,
        "{}",
        report.text
    );
    for row in [work_row, note_row] {
        assert!(
            report
                .text
                .lines()
                .nth(row)
                .unwrap()
                .chars()
                .filter(|&ch| ch == '|')
                .count()
                >= 4,
            "the node and note must both stay inside the system frame:\n{}",
            report.text
        );
    }
    assert!(
        report.text.contains(['.', ':']),
        "the note attachment must be dotted:\n{}",
        report.text
    );
}

#[test]
fn empty_and_disconnected_boundaries_keep_their_own_frames() {
    let source = "usecase-beta\nsystemBoundary Empty(Empty scope)\nend\nsystemBoundary Full(Full scope)@{type:package}\nA(Alpha)\nend";
    let report = report(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
    for label in [
        "Empty scope",
        "Full scope",
        "Alpha",
        "[boundary]",
        "[package]",
    ] {
        assert!(report.text.contains(label), "{label}:\n{}", report.text);
    }
    support::assert_rectangular_terminal_grid(&report.text);
}

#[test]
fn width_fallback_is_complete_keyed_and_accurately_reported() {
    let mut model = model(&support::local_semantic_input("usecase/typical.mmd"));
    model.nodes[0].label =
        "Customer with a deliberately long authored label that exceeds the fallback viewport"
            .into();
    model.title = Some("Auth \"title\"".into());
    model.acc_title = Some("Accessible title".into());
    model.acc_description = Some("A description\nwith a second line".into());
    model.nodes[0].classes.push("human".into());
    model.nodes[0].styles.push("fill:red".into());
    model.relationships[0].animate = true;
    model.relationships[0].animation = Some(UsecaseAnimation::Fast);
    model.relationships[0].label = Some("Request \"quoted\"\nnext".into());
    model.relationships[0].label_type = Some(UsecaseLabelType::Markdown);
    model
        .class_defs
        .push(merman_model::families::usecase::UsecaseClassDef {
            id: "human".into(),
            styles: vec!["color:blue".into()],
        });
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            let report = direct_report(
                &model,
                options.with_terminal_width_profile(profile),
                AsciiViewportPolicy::with_max_width(24).overflow(OverflowPolicy::Fallback),
                &OperationControl::new(),
                AsciiResourcePolicy::default(),
            )
            .unwrap();
            assert_report_measurement(&report, profile);
            assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
            assert_eq!(report.projection, AsciiProjection::StructuredText);
            assert_eq!(
                report.fallback.reason,
                Some(AsciiFallbackReason::PrimaryOverflow)
            );
            assert_eq!(report.primary_overflowed, Some(true));
            assert!(report.primary_extent.unwrap().width > 24);
            assert_eq!(report.lossiness, Lossiness::PresentationOnly);
            assert!(!report.emitted_overflowed);
            let collapsed = report.text.split_whitespace().collect::<String>();
            for expected in [
                "N1[actor]id=\"User\"",
                "N2parent:G1",
                "G1members:[N2,N3]",
                "E1:N1[open]--solidassociation--N2[point]",
                "T1noteforN3",
                "labelType:\"text\"",
                "actorType:\"hollow\"",
                "business:true",
                "stereotype:\"Human\"",
                "parentId:\"Auth\"",
                "package:true",
                "styles:[\"fill:red\"]",
                "classes:[\"human\"]",
                "explicitId:true",
                "minlen:1",
                "animate:true",
                "animation:\"fast\"",
                "labelType:\"markdown\"",
                "title:\"Auth\\\"title\\\"\"",
                "acc_title:\"Accessibletitle\"",
                "acc_description:",
                "class_defs:[{\"id\":\"human\",\"styles\":[\"color:blue\"]}]",
                "sourceConfig:{}",
                "json_nodes:[]",
                "Request\\\"quoted\\\"\\nnext",
            ] {
                assert!(
                    collapsed.contains(expected),
                    "missing {expected}:\n{}",
                    report.text
                );
            }
        }
    }
}

#[test]
fn overflow_allow_error_and_fallback_have_distinct_truthful_outcomes() {
    let source = "usecase-beta\nA(A very long use case label for overflow)\nB(Beta)\nA --> B";
    let options = AsciiRenderOptions::ascii();
    let allow = report(source, options, AsciiViewportPolicy::with_max_width(20)).unwrap();
    assert_eq!(allow.outcome, AsciiOutputOutcome::WideAllowed);
    assert!(allow.emitted_overflowed);
    assert!(matches!(
        report(
            source,
            options,
            AsciiViewportPolicy::with_max_width(20).overflow(OverflowPolicy::Error)
        ),
        Err(AsciiError::WidthOverflow { max_width: 20, .. })
    ));
    let fallback = report(
        source,
        options,
        AsciiViewportPolicy::with_max_width(20).overflow(OverflowPolicy::Fallback),
    )
    .unwrap();
    assert_eq!(fallback.outcome, AsciiOutputOutcome::Fallback);
    assert!(!fallback.emitted_overflowed);
}

fn invalid(model: &UsecaseDiagramModel, feature: &str) {
    let error =
        render_direct(model).expect_err("malformed direct models must fail before graph routing");
    assert!(
        matches!(&error, AsciiError::UnsupportedFeature { diagram_type: "usecase", feature: actual } if *actual == feature),
        "{error:?}"
    );
}

#[test]
fn malformed_direct_ids_endpoints_roles_and_metadata_are_rejected() {
    let base = model(
        "usecase-beta\nactor User\nA(Alpha)\nB[Beta]\nUser --> A\nA ..>:include B\nnote for A \"Note\"",
    );
    let mut bad = base.clone();
    bad.nodes.push(bad.nodes[0].clone());
    invalid(&bad, "duplicate node ids");
    let mut bad = base.clone();
    bad.nodes[0].id.clear();
    invalid(&bad, "empty node ids");
    let mut bad = base.clone();
    bad.relationships[0].target = "Missing".into();
    invalid(&bad, "relationships with missing endpoint nodes");
    let mut bad = base.clone();
    bad.relationships.push(bad.relationships[0].clone());
    invalid(&bad, "empty or duplicate relationship ids");
    let mut bad = base.clone();
    bad.relationships[0].id = "User".into();
    bad.relationships[0].explicit_id = true;
    invalid(&bad, "explicit relationship ids collide with entities");
    let mut bad = base.clone();
    bad.relationships[0].minlen = 0;
    invalid(&bad, "zero relationship minlen");
    let mut bad = base.clone();
    bad.relationships[0].relationship_type = UsecaseRelationshipType::Include;
    invalid(&bad, "include and extend require use-case endpoints");
    let mut bad = base.clone();
    bad.relationships[0].relationship_type = UsecaseRelationshipType::Generalization;
    invalid(&bad, "generalization requires endpoints of the same role");
    let mut bad = base.clone();
    bad.notes[0].target = "Missing".into();
    invalid(&bad, "notes with missing target nodes");
    let mut bad = base.clone();
    bad.notes.push(bad.notes[0].clone());
    invalid(&bad, "empty or duplicate note ids");
    let mut bad = base.clone();
    bad.nodes[0].actor_type = UsecaseActorType::Icon;
    invalid(&bad, "inconsistent actor icon metadata");
    let mut bad = base.clone();
    bad.nodes[2].business = true;
    invalid(&bad, "business rectangular use cases");
    let mut bad = base.clone();
    bad.nodes[1].actor_type = UsecaseActorType::Hollow;
    invalid(&bad, "actor metadata on use-case nodes");
    let mut bad = base;
    bad.direction = "XX".into();
    invalid(&bad, "direction");
}

#[test]
fn malformed_membership_and_cycles_are_distinguished_from_unsupported_nesting() {
    let base = model("usecase-beta\nsystemBoundary S(System)\nA(Alpha)\nend");
    let mut bad = base.clone();
    bad.nodes[0].parent_id = Some("Missing".into());
    invalid(&bad, "unknown node parent ids");
    let mut bad = base.clone();
    bad.nodes[0].parent_id = Some("A".into());
    invalid(&bad, "node parents that are not boundaries");
    let mut bad = base.clone();
    bad.nodes[0].parent_id = None;
    invalid(&bad, "boundary membership disagrees with node parent");
    let mut bad = base.clone();
    bad.boundaries[0].members.clear();
    invalid(&bad, "parented nodes missing from boundary membership");
    let mut bad = base.clone();
    bad.boundaries[0].members.push("Missing".into());
    invalid(&bad, "boundary members with missing targets");
    let mut bad = base.clone();
    bad.boundaries[0].members.push("A".into());
    invalid(&bad, "duplicate boundary members");
    let mut bad = base.clone();
    bad.boundaries.push(bad.boundaries[0].clone());
    invalid(&bad, "duplicate or colliding boundary ids");
    let mut bad = base.clone();
    bad.boundaries[0].id = "A".into();
    invalid(&bad, "duplicate or colliding boundary ids");
    let boundary = |id: &str, members: &[&str]| UsecaseBoundary {
        id: id.into(),
        label: id.into(),
        label_type: UsecaseLabelType::Text,
        members: members.iter().map(|id| (*id).into()).collect(),
        package: false,
        classes: vec![],
        styles: vec![],
    };
    let mut bad = base.clone();
    bad.boundaries[0].members.push("S".into());
    invalid(&bad, "cyclic boundary membership");
    let mut bad = base.clone();
    bad.boundaries.push(boundary("T", &["S"]));
    bad.boundaries[0].members.push("T".into());
    invalid(&bad, "cyclic boundary membership");
    let mut bad = base;
    bad.boundaries.push(boundary("T", &["S"]));
    invalid(&bad, "nested system boundaries");
}

#[test]
fn json_table_associations_keep_titles_labels_and_all_directions() {
    let expected = [
        (r#"$["id"]"#, "1"),
        (r#"$["客户·"]"#, r#""👩‍💻""#),
        (r#"$["items"][0]["x"]"#, r#""a longer value""#),
        (r#"$["items"][1]"#, r#""短""#),
    ];
    for (direction, arrow, horizontal, reversed) in [
        ("LR", "-- sends -->", true, false),
        ("RL", "<-- sends --", true, true),
        ("TB", "-- sends ---", false, false),
        ("BT", "-- sends -->", false, true),
    ] {
        let source = format!(
            r#"usecase-beta
direction {direction}
json Data@{{"id":1,"客户·":"👩‍💻","items":[{{"x":"a longer value"}},"短"]}}
Consumer(Receiver)
Data {arrow} Consumer"#
        );
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                let report = report(
                    &source,
                    options.with_terminal_width_profile(profile),
                    AsciiViewportPolicy::unrestricted(),
                )
                .unwrap();
                assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
                assert_eq!(report.projection, AsciiProjection::Diagrammatic);
                assert_report_measurement(&report, profile);
                support::assert_rectangular_terminal_grid_with_profile(&report.text, profile);
                assert_json_table_body_rows(&report, &expected);
                for label in [r#""Data" [JSON]"#, "sends"] {
                    assert!(
                        report.text.contains(label),
                        "missing {label}:\n{}",
                        report.text
                    );
                }
                let position = |label: &str| {
                    report
                        .text
                        .lines()
                        .enumerate()
                        .find_map(|(y, row)| row.find(label).map(|x| (x, y)))
                        .unwrap()
                };
                let data = position(r#""Data" [JSON]"#);
                let receiver = position("Receiver");
                let increasing = if horizontal {
                    data.0 < receiver.0
                } else {
                    data.1 < receiver.1
                };
                assert_eq!(increasing, !reversed, "{direction}\n{}", report.text);
            }
        }
    }
}

#[test]
fn json_table_title_dividers_stay_uninterrupted_after_direction_transforms() {
    for json in [
        r#"{"a":1}"#,
        r#"{"a":1,"items":[{"x":"longer value"},"短"]}"#,
    ] {
        for direction in ["LR", "RL", "TB", "BT"] {
            let source = format!("usecase-beta\ndirection {direction}\njson D@{json}");
            for (options, left, horizontal, right) in [
                (AsciiRenderOptions::ascii(), '+', '-', '+'),
                (AsciiRenderOptions::unicode(), '├', '─', '┤'),
            ] {
                let report = report(&source, options, AsciiViewportPolicy::unrestricted()).unwrap();
                assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
                let lines: Vec<_> = report.text.lines().map(str::trim).collect();
                let title = lines
                    .iter()
                    .position(|line| line.contains(r#""D" [JSON]"#))
                    .unwrap();
                let header = lines
                    .iter()
                    .position(|line| line.contains(" | Value"))
                    .unwrap();
                assert_eq!(header, title + 2, "{direction} {json}:\n{}", report.text);
                let width = lines[title].chars().count();
                let divider = format!("{left}{}{right}", horizontal.to_string().repeat(width - 2));
                assert_eq!(
                    lines[title + 1],
                    divider,
                    "the full title divider must survive {direction} for {json}:\n{}",
                    report.text
                );
            }
        }
    }
}

#[test]
fn frozen_trident_json_fixtures_remain_diagrammatic_with_measured_cjk_output() {
    for (fixture, labels, rows) in [
        (
            "json-payload",
            &["Inspect payload", r#""Payload" [JSON]"#][..],
            &[
                (r#"$["2"]"#, r#""second in source""#),
                (r#"$["1"]"#, r#""first after 2""#),
                (r#"$["enabled"]"#, "true"),
                (r#"$["count"]"#, "3"),
                (r#"$["missing"]"#, "null"),
                (r#"$["colors"][0]"#, r#""Red""#),
                (r#"$["colors"][1]"#, r#""Green""#),
                (r#"$["address"]["city"]"#, r#""Oslo""#),
                (r#"$["items"][0]["name"]"#, r#""Book""#),
                (r#"$["emptyObject"]"#, "{}"),
                (r#"$["emptyArray"]"#, "[]"),
            ][..],
        ),
        (
            "json-styled-entities",
            &[
                "Customer",
                "Checkout",
                "Account",
                "Edit profile",
                r#""Session" [JSON]"#,
            ][..],
            &[(r#"$["active"]"#, "true")][..],
        ),
        (
            "json-ordering-system",
            &[
                "Ordering System",
                "Order staff",
                "places order",
                "<<include>>",
                r#""OrderData" [JSON]"#,
            ][..],
            &[
                (r#"$["status"]"#, r#""pending""#),
                (r#"$["items"][0]["name"]"#, r#""Book""#),
                (r#"$["items"][0]["quantity"]"#, "1"),
                (r#"$["total"]"#, "29.95"),
            ][..],
        ),
    ] {
        let source = support::local_semantic_input(&format!("usecase/{fixture}.mmd"));
        let profile = TerminalWidthProfile::Cjk;
        let report = report(
            &source,
            AsciiRenderOptions::unicode().with_terminal_width_profile(profile),
            AsciiViewportPolicy::with_max_width(120).overflow(OverflowPolicy::Allow),
        )
        .unwrap_or_else(|error| panic!("{fixture}: {error}"));
        assert_eq!(
            report.projection,
            AsciiProjection::Diagrammatic,
            "{fixture}"
        );
        assert_eq!(report.lossiness, Lossiness::None);
        assert_report_measurement(&report, profile);
        support::assert_rectangular_terminal_grid_with_profile(&report.text, profile);
        let overflowed = report.emitted_extent.width > 120;
        assert_eq!(report.emitted_overflowed, overflowed);
        assert_eq!(report.primary_overflowed, Some(overflowed));
        assert_eq!(report.primary_extent, Some(report.emitted_extent));
        assert_eq!(
            report.outcome,
            if overflowed {
                AsciiOutputOutcome::WideAllowed
            } else {
                AsciiOutputOutcome::Primary
            }
        );
        assert!(!report.fallback.attempted);
        assert_json_table_body_rows(&report, rows);
        for &label in labels {
            assert!(
                report.text.contains(label),
                "{fixture} missing {label}:\n{}",
                report.text
            );
        }
    }
}

#[test]
fn json_tables_use_source_order_typed_paths_and_index_every_array_item() {
    let source = r#"usecase-beta
json Data@{"10":"ten","2":2,"items":[{"z":true,"a":null},[1,{"0":"zero"}],[],{}],"a/b~c":{"q\"[x]":"<br>\\n"},"":{},"客户·":"👩‍💻"}
"#;
    let expected = [
        (r#"$["10"]"#, r#""ten""#),
        (r#"$["2"]"#, "2"),
        (r#"$["items"][0]["z"]"#, "true"),
        (r#"$["items"][0]["a"]"#, "null"),
        (r#"$["items"][1][0]"#, "1"),
        (r#"$["items"][1][1]["0"]"#, r#""zero""#),
        (r#"$["items"][2]"#, "[]"),
        (r#"$["items"][3]"#, "{}"),
        (r#"$["a/b~c"]["q\"[x]"]"#, r#""<br>\\n""#),
        (r#"$[""]"#, "{}"),
        (r#"$["客户·"]"#, r#""👩‍💻""#),
    ];
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            let report = report(
                source,
                options.with_terminal_width_profile(profile),
                AsciiViewportPolicy::unrestricted(),
            )
            .unwrap();
            assert_json_table_rows(&report, &expected);
        }
    }
}

#[test]
fn json_tables_keep_empty_roots_and_choose_explicit_or_typed_map_order() {
    let empty = report(
        "usecase-beta\njson Empty@{}",
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_json_table_rows(&empty, &[("$", "{}")]);

    let mut model = model(
        r#"usecase-beta
json Data@{"z":3,"a":1,"m":2}
"#,
    );
    let rows: Vec<_> = model.json_nodes[0]
        .value
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| {
            (
                format!("$[{}]", serde_json::to_string(key).unwrap()),
                value.to_string(),
            )
        })
        .collect();
    model.json_nodes[0]
        .property_order
        .insert("".into(), vec!["m".into(), "z".into(), "a".into()]);
    assert_json_table_rows(
        &render_direct(&model).unwrap(),
        &[(r#"$["m"]"#, "2"), (r#"$["z"]"#, "3"), (r#"$["a"]"#, "1")],
    );
    model.json_nodes[0].property_order.clear();
    let expected: Vec<_> = rows
        .iter()
        .map(|(path, value)| (path.as_str(), value.as_str()))
        .collect();
    assert_json_table_rows(&render_direct(&model).unwrap(), &expected);
}

#[test]
fn json_table_quoted_text_keeps_controls_html_breaks_and_backslashes_literal() {
    let source = r#"usecase-beta
json Data@{"break":"a<br>b","escaped":"a\\nb","newline":"a\nb","tab":"a\tb","quote":"a\"b","control":"\u001b[31m\u202e"}
"#;
    let mut model = model(source);
    model.json_nodes[0].id = "Data \"quoted\"<br>\\n".into();
    let report = render_direct(&model).unwrap();
    assert!(
        report.text.contains(r#""Data \"quoted\"<br>\\n" [JSON]"#),
        "{}",
        report.text
    );
    assert!(!report.text.contains(['\u{1b}', '\u{202e}']));
    assert_json_table_rows(
        &report,
        &[
            (r#"$["break"]"#, r#""a<br>b""#),
            (r#"$["escaped"]"#, r#""a\\nb""#),
            (r#"$["newline"]"#, r#""a\nb""#),
            (r#"$["tab"]"#, r#""a\tb""#),
            (r#"$["quote"]"#, r#""a\"b""#),
            (r#"$["control"]"#, r#""\u{1B}[31m\u{202E}""#),
        ],
    );
}

#[test]
fn json_tables_distinguish_unpaired_surrogates_from_replacement_and_literal_escapes() {
    let source = r#"usecase-beta
json Data@{"\ud800":"\udc00","\ud801":"\ud800","�":"replacement","\\ud800":"literal","\ud83d\ude00":"pair key","pair":"\ud83d\ude00","normal":"hello"}
"#;
    let model: UsecaseDiagramModel =
        serde_json::from_slice(&serde_json::to_vec(&model(source)).unwrap()).unwrap();
    assert_eq!(
        model.json_nodes[0].string_encoding,
        Some(UsecaseJsonStringEncoding::JsonUtf16)
    );
    let expected = [
        (r#"$[utf16:"\ud800"]"#, r#"utf16:"\udc00""#),
        (r#"$[utf16:"\ud801"]"#, r#"utf16:"\ud800""#),
        (r#"$["�"]"#, r#""replacement""#),
        (r#"$["\\ud800"]"#, r#""literal""#),
        (r#"$["😀"]"#, r#""pair key""#),
        (r#"$["pair"]"#, r#""😀""#),
        (r#"$["normal"]"#, r#""hello""#),
    ];
    assert_json_table_rows(&render_direct(&model).unwrap(), &expected);
    let parsed = report(
        source,
        AsciiRenderOptions::unicode(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_json_table_rows(&parsed, &expected);
}

#[test]
fn json_tables_format_js_numbers_and_distinguish_infinity_null_and_strings() {
    let source = r#"usecase-beta
json Data@{"rounded":9007199254740993,"negativeZero":-0,"large":1e20,"exponent":1e21,"small":1e-6,"tiny":1e-7,"positive":1e400,"negative":-1e400,"nothing":null,"text":"Infinity"}
"#;
    let expected = [
        (r#"$["rounded"]"#, "9007199254740992"),
        (r#"$["negativeZero"]"#, "0"),
        (r#"$["large"]"#, "100000000000000000000"),
        (r#"$["exponent"]"#, "1e+21"),
        (r#"$["small"]"#, "0.000001"),
        (r#"$["tiny"]"#, "1e-7"),
        (r#"$["positive"]"#, "Infinity"),
        (r#"$["negative"]"#, "-Infinity"),
        (r#"$["nothing"]"#, "null"),
        (r#"$["text"]"#, r#""Infinity""#),
    ];
    let parsed = report(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_json_table_rows(&parsed, &expected);
    let mut model = model(source);
    model.json_nodes[0].value["rounded"] = serde_json::json!(9_007_199_254_740_993_u64);
    assert_json_table_rows(&render_direct(&model).unwrap(), &expected);
}

#[test]
fn json_table_width_fallback_appends_keys_and_preserves_raw_sidecars() {
    let mut model = model(
        r#"usecase-beta
json Data@{"z":"\ud800","payloadValue":1e400}
actor User
json Empty@{}
Action(Act)
User -- request --> Data
Data --> Action
Empty --- Data
note for Action "Regular note"
"#,
    );
    model.json_nodes[0].classes.push("payload".into());
    model.json_nodes[0].styles.push("fill:red".into());
    let serialized = serde_json::to_value(&model.json_nodes[0]).unwrap();
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        let report = direct_report(
            &model,
            AsciiRenderOptions::ascii().with_terminal_width_profile(profile),
            AsciiViewportPolicy::with_max_width(24).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
        assert_report_measurement(&report, profile);
        assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
        assert_eq!(report.projection, AsciiProjection::StructuredText);
        assert_eq!(report.lossiness, Lossiness::PresentationOnly);
        assert_eq!(
            report.fallback.reason,
            Some(AsciiFallbackReason::PrimaryOverflow)
        );
        assert_eq!(report.primary_overflowed, Some(true));
        assert!(report.primary_extent.unwrap().width > 24);
        assert!(!report.emitted_overflowed);
        let collapsed = report.text.split_whitespace().collect::<String>();
        for expected in [
            r#"N1[actor]id="User""#,
            r#"N2[use-caseellipse]id="Action""#,
            r#"N3[JSONtable]id="Data""#,
            r#"N4[JSONtable]id="Empty""#,
            "E1:N1[open]--solidassociation--N3[point]",
            "E2:N3[open]--solidassociation--N2[point]",
            "E3:N4[open]--solidassociation--N3[open]",
            r#"label:"request""#,
            "T1noteforN2",
        ] {
            assert!(
                collapsed.contains(expected),
                "missing {expected}:\n{}",
                report.text
            );
        }
        for field in [
            "value",
            "stringEncoding",
            "propertyOrder",
            "nonFiniteNumbers",
            "classes",
            "styles",
        ] {
            let expected = format!("{field}:{}", serialized[field]);
            assert!(
                collapsed.contains(&expected),
                "missing {expected}:\n{}",
                report.text
            );
        }
    }
}

#[test]
fn malformed_direct_json_roots_order_and_infinity_metadata_are_rejected() {
    let base = model(
        r#"usecase-beta
json Data@{"a":1,"b":null,"nested":{"x":true}}
"#,
    );
    let mut bad = base.clone();
    bad.json_nodes[0].value = serde_json::json!([1]);
    invalid(&bad, "JSON table roots must be objects");
    for order in [vec!["a", "a", "nested"], vec!["a", "b"]] {
        let mut bad = base.clone();
        bad.json_nodes[0]
            .property_order
            .insert("".into(), order.into_iter().map(str::to_string).collect());
        invalid(
            &bad,
            "JSON property order must contain every key exactly once",
        );
    }
    let mut bad = base.clone();
    bad.json_nodes[0]
        .property_order
        .insert("/missing".into(), vec![]);
    invalid(&bad, "unreachable JSON table metadata");
    for (pointer, feature) in [
        ("/missing", "unreachable JSON table metadata"),
        ("/a", "JSON infinity metadata must identify null values"),
    ] {
        let mut bad = base.clone();
        bad.json_nodes[0]
            .non_finite_numbers
            .insert(pointer.into(), UsecaseJsonInfinity::Positive);
        invalid(&bad, feature);
    }

    let mut bad = base;
    bad.json_nodes[0].string_encoding = Some(UsecaseJsonStringEncoding::JsonUtf16);
    invalid(&bad, "invalid JSON UTF-16 strings");
    let mut bad = model(
        r#"usecase-beta
json Data@{"text":"\ud800"}
"#,
    );
    let key = bad.json_nodes[0].property_order[""][0].clone();
    bad.json_nodes[0].value[&key] = serde_json::json!("not a JSON string literal");
    invalid(&bad, "invalid JSON UTF-16 strings");
}

#[test]
fn direct_json_entities_reject_colliding_ids_special_links_notes_and_membership() {
    let base = model(
        r#"usecase-beta
actor User
Action(Act)
json Data@{}
User --> Data
note for Action "Ordinary note"
systemBoundary Scope(Scope)
end
"#,
    );
    let mut bad = base.clone();
    bad.json_nodes[0].id.clear();
    invalid(&bad, "empty JSON table ids");
    let mut bad = base.clone();
    bad.json_nodes[0].id = "User".into();
    invalid(&bad, "duplicate or colliding JSON table ids");
    for (relationship_type, arrow_type) in [
        (UsecaseRelationshipType::Include, UsecaseArrowType::Point),
        (
            UsecaseRelationshipType::Association,
            UsecaseArrowType::Circle,
        ),
    ] {
        let mut bad = base.clone();
        bad.relationships[0].relationship_type = relationship_type;
        bad.relationships[0].arrow_type = arrow_type;
        invalid(&bad, "JSON tables require plain association relationships");
    }
    let mut bad = base.clone();
    bad.notes[0].target = "Data".into();
    invalid(&bad, "notes targeting JSON tables");
    let mut bad = base;
    bad.boundaries[0].members.push("Data".into());
    invalid(&bad, "JSON tables inside boundaries");
}

#[test]
fn note_ids_do_not_shadow_authored_nodes_or_relationship_ids() {
    let mut model = model("usecase-beta\nactor User\nA(Alpha)\nUser --> A\nnote for A \"Note\"");
    // The generated note spelling is not a source identifier. Borrowed models can still
    // contain this collision; it must not shadow either an entity or an anonymous edge.
    let note_id = model.notes[0].id.clone();
    model.nodes[0].id.clone_from(&note_id);
    model.nodes[0].label.clone_from(&note_id);
    model.relationships[0].source.clone_from(&note_id);
    model.relationships[0].id.clone_from(&note_id);
    let report = render_direct(&model).unwrap();
    assert_eq!(report.text.matches("note-0").count(), 1);
    assert!(report.text.contains("Alpha") && report.text.contains("Note"));
}

#[test]
fn terminal_text_escaping_preserves_cjk_emoji_and_hides_control_sequences() {
    let mut model = model("usecase-beta\nA(Alpha)");
    model.nodes[0].label = "客户👩‍💻\u{1b}[31m\u{202e}a\u{301}".into();
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            let report = direct_report(
                &model,
                options.with_terminal_width_profile(profile),
                AsciiViewportPolicy::unrestricted(),
                &OperationControl::new(),
                AsciiResourcePolicy::default(),
            )
            .unwrap();
            assert!(!report.text.contains(['\u{1b}', '\u{202e}']));
            assert!(report.text.contains("客户👩‍💻"));
            assert!(report.text.contains("a\u{301}"));
            support::assert_rectangular_terminal_grid_with_profile(&report.text, profile);
        }
    }
}

#[test]
fn public_grid_quota_accepts_exact_n_and_rejects_n_minus_one() {
    let model = model("usecase-beta\nactor A(Alpha)\nB(Beta)\nA --> B");
    let baseline = render_direct(&model).unwrap();
    let cells = baseline.emitted_extent.width * baseline.emitted_extent.height;
    for max in [cells, cells - 1] {
        let resources = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxGridCells, max)
            .unwrap();
        let result = direct_report(
            &model,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            resources,
        );
        if max == cells {
            assert_eq!(result.unwrap().text, baseline.text);
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxGridCells && details.actual == cells && details.max == max)
            );
        }
    }
}

#[test]
fn resource_and_cancel_failures_never_become_semantic_fallback() {
    let model = model("usecase-beta\nactor A(Alpha)\nB(Beta)\nA --> B");
    let viewport = AsciiViewportPolicy::with_max_width(1).overflow(OverflowPolicy::Fallback);
    let resources = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap();
    assert!(matches!(
        direct_report(
            &model,
            AsciiRenderOptions::ascii(),
            viewport,
            &OperationControl::new(),
            resources
        ),
        Err(AsciiError::ResourceLimitExceeded(_))
    ));
    let control = OperationControl::new();
    control.cancel();
    assert!(matches!(
        direct_report(
            &model,
            AsciiRenderOptions::ascii(),
            viewport,
            &control,
            resources
        ),
        Err(AsciiError::Cancelled(_))
    ));
}

#[test]
fn usecase_rejects_flowchart_only_profiles_and_unadmitted_adaptive_presentation() {
    let source = "usecase-beta\nA(Alpha)\nB(Beta)\nA --> B";
    for layout in [AsciiLayoutProfile::Compact, AsciiLayoutProfile::Auto] {
        assert!(matches!(
            report(
                source,
                AsciiRenderOptions::ascii().with_layout_profile(layout),
                AsciiViewportPolicy::with_max_width(80)
            ),
            Err(AsciiError::InvalidOption {
                field: "layout_profile",
                ..
            })
        ));
    }
    assert!(matches!(
        report(
            source,
            AsciiRenderOptions::ascii().with_graph_presentation(AsciiGraphPresentation::Adaptive),
            AsciiViewportPolicy::unrestricted()
        ),
        Err(AsciiError::InvalidOption {
            field: "graph_presentation",
            ..
        })
    ));
}
