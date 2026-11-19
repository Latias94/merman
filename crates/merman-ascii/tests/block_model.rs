#![cfg(feature = "diagram-block")]

use merman_ascii::{
    AsciiDiagram, AsciiError, AsciiOutput, AsciiOutputOutcome, AsciiProjection, AsciiRenderOptions,
    AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy, AsciiSourceContext,
    AsciiViewportPolicy, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::block::{BlockDiagramModel, BlockEdge, BlockNode};
use merman_model::{MermaidConfig, OperationControl, SemanticDiagramModel, runtime::RuntimePolicy};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/testdata/local-semantic/block")
            .join(format!("{name}.mmd")),
    )
    .unwrap()
}

fn parse(source: &str) -> BlockDiagramModel {
    match SemanticParser::new()
        .parse_diagram_sync(source)
        .unwrap()
        .model
    {
        SemanticDiagramModel::Block(model) => model,
        other => panic!("expected Block, got {other:?}"),
    }
}

fn render(
    model: &BlockDiagramModel,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
    policy: AsciiResourcePolicy,
    control: &OperationControl,
) -> merman_ascii::Result<AsciiOutput> {
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    let config = MermaidConfig::empty_object();
    AsciiRenderer::new(options)?.render_model_report(
        &AsciiDiagram::Block(model),
        AsciiSourceContext {
            diagram_type: "block",
            config: &config,
            semantic_config: &config,
            title: None,
        },
        viewport,
        control,
        &operation,
        policy,
    )
}

fn primary(model: &BlockDiagramModel) -> merman_ascii::Result<AsciiOutput> {
    render(
        model,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
        AsciiResourcePolicy::default(),
        &OperationControl::new(),
    )
}

fn width(line: &str, profile: TerminalWidthProfile) -> usize {
    match profile {
        TerminalWidthProfile::Cjk => UnicodeWidthStr::width_cjk(line),
        _ => UnicodeWidthStr::width(line),
    }
}

fn position(text: &str, label: &str) -> (usize, usize) {
    text.lines()
        .enumerate()
        .find_map(|(y, line)| line.find(label).map(|x| (x, y)))
        .unwrap_or_else(|| panic!("missing {label:?}:\n{text}"))
}

#[test]
fn small_and_typical_are_spatial_at_terminal_widths_in_both_text_profiles() {
    for name in ["small", "typical", "cjk"] {
        let model = parse(&fixture(name));
        for maximum in [80, 100, 120] {
            for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                    let report = render(
                        &model,
                        options.with_terminal_width_profile(profile),
                        AsciiViewportPolicy::with_max_width(maximum)
                            .overflow(OverflowPolicy::Fallback),
                        AsciiResourcePolicy::default(),
                        &OperationControl::new(),
                    )
                    .unwrap_or_else(|error| panic!("{name}/{maximum}/{profile:?}: {error}"));
                    assert_eq!(
                        report.projection,
                        AsciiProjection::Diagrammatic,
                        "{name}: {report:?}"
                    );
                    assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
                    assert!(
                        report
                            .text
                            .lines()
                            .all(|line| width(line, profile) <= maximum)
                    );
                    for node in &model.blocks_flat {
                        if !matches!(node.block_type.as_str(), "space" | "composite") {
                            assert!(
                                report.text.contains(&node.label),
                                "missing {}: {}",
                                node.label,
                                report.text
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn dense_grid_keeps_every_record_or_reports_a_bounded_route_failure() {
    let model = parse(&fixture("dense"));
    for maximum in [80, 100, 120] {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                match render(
                    &model,
                    options.with_terminal_width_profile(profile),
                    AsciiViewportPolicy::with_max_width(maximum).overflow(OverflowPolicy::Fallback),
                    AsciiResourcePolicy::default(),
                    &OperationControl::new(),
                ) {
                    Ok(report) => {
                        for node in model
                            .blocks_flat
                            .iter()
                            .filter(|node| node.block_type != "composite")
                        {
                            assert!(report.text.contains(&node.label), "{}", report.text);
                        }
                        if report.projection == AsciiProjection::Diagrammatic {
                            let a1 = position(&report.text, "A1");
                            let a4 = position(&report.text, "A4");
                            let d1 = position(&report.text, "D1");
                            assert!(a1.0 < a4.0 && a1.1 == a4.1 && a1.1 < d1.1);
                        } else {
                            assert_eq!(
                                report.fallback.reason,
                                Some(merman_ascii::AsciiFallbackReason::PrimaryOverflow)
                            );
                            assert!(report.primary_extent.unwrap().width > maximum);
                            for index in 0..model.edges.len() {
                                assert!(report.text.contains(&format!("E{index} ")));
                            }
                        }
                    }
                    Err(
                        AsciiError::RoutingSearchExhausted {
                            diagram_type: "block",
                            ..
                        }
                        | AsciiError::PortCapacityExceeded {
                            diagram_type: "block",
                            ..
                        },
                    ) => {}
                    Err(error) => panic!("dense/{maximum}/{profile:?}: {error}"),
                }
            }
        }
    }
}

#[test]
fn authored_columns_spans_and_spaces_determine_positions() {
    let report = primary(&parse("block-beta\ncolumns 3\nA space B\nC:2 D\nE F G\n")).unwrap();
    let a = position(&report.text, "A");
    let b = position(&report.text, "B");
    let c = position(&report.text, "C");
    let d = position(&report.text, "D");
    let e = position(&report.text, "E");
    let f = position(&report.text, "F");
    assert_eq!(a.1, b.1);
    assert_eq!(c.1, d.1);
    assert!(a.1 < c.1 && c.1 < e.1);
    assert_eq!(a.0, e.0);
    assert_eq!(b.0, d.0);
    assert!(
        b.0 - a.0 > f.0 - e.0,
        "spacer must reserve a column: {}",
        report.text
    );
}

#[test]
fn auto_columns_and_redeclarations_keep_first_authored_hierarchy_order() {
    let model = parse("block-beta\nA B C\nA[\"Final\"]\n");
    let report = primary(&model).unwrap();
    let a = position(&report.text, "Final");
    let b = position(&report.text, "B");
    let c = position(&report.text, "C");
    assert!(a.0 < b.0 && b.0 < c.0);
    assert_eq!(a.1, b.1);
    assert_eq!(b.1, c.1);
    assert_eq!(model.root().unwrap().children.len(), 3);
}

#[test]
fn upstream_overflowing_span_starts_the_following_block_on_the_next_row() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/block/upstream_cypress_block_spec_bl30_block_should_overflow_if_too_wide_for_columns_030.mmd"),
    ).unwrap();
    let report = primary(&parse(&source)).unwrap();
    let fit = position(&report.text, "fit");
    let overflow = position(&report.text, "overflow");
    let short = position(&report.text, "short");
    let also_overflow = position(&report.text, "also_overflow");
    assert!(fit.1 < overflow.1 && overflow.1 < short.1);
    assert_eq!(short.1, also_overflow.1);
    assert!(short.0 < also_overflow.0);
}

#[test]
fn nested_frames_and_spanning_headers_contain_their_children() {
    let report = primary(&parse(&fixture("typical"))).unwrap();
    let gate = position(&report.text, "Gate");
    let api = position(&report.text, "API");
    let work = position(&report.text, "Work");
    let cache = position(&report.text, "Cache");
    let store = position(&report.text, "Store");
    assert!(gate.1 < api.1 && api.1 < cache.1);
    assert_eq!(api.1, work.1);
    assert!(api.0 < work.0 && work.0 < store.0);
    // A nested frame must have a separate boundary outside both child boxes.
    let api_line = report.text.lines().nth(api.1).unwrap();
    assert!(
        api_line[..api.0].chars().filter(|&ch| ch == '|').count() >= 2,
        "{}",
        report.text
    );
}

#[test]
fn markers_strokes_labels_and_unmarked_edges_remain_distinct() {
    for (syntax, required, forbidden) in [
        ("-->", ">", None),
        ("<-->", "<", None),
        ("--o", "o", None),
        ("x--x", "x", None),
        ("==>", "=", None),
        ("-.->", ".", None),
        ("---", "-", Some('>')),
    ] {
        let source = format!("block-beta\nA space:2 B\nA {syntax} B\n");
        let report = primary(&parse(&source)).unwrap();
        assert!(report.text.contains(required), "{syntax}: {}", report.text);
        if let Some(forbidden) = forbidden {
            assert!(!report.text.contains(forbidden), "{}", report.text);
        }
    }
    let report = primary(&parse("block-beta\nA space:2 B\nA-- \"link text\" -->B\n")).unwrap();
    assert!(report.text.contains("link text"));
}

#[test]
fn block_arrow_approximation_discloses_all_authored_directions() {
    let report = primary(&parse("block-beta\nA<[\"Turn\"]>(right, down)\n")).unwrap();
    assert!(
        report.text.contains("[block-arrow:right+down] Turn"),
        "{}",
        report.text
    );
}

fn node(id: &str, block_type: &str, children: Vec<usize>) -> BlockNode {
    BlockNode {
        id: id.into(),
        label: if block_type == "composite" {
            String::new()
        } else {
            id.into()
        },
        block_type: block_type.into(),
        children,
        ..BlockNode::default()
    }
}

fn direct_model() -> BlockDiagramModel {
    BlockDiagramModel {
        root: Some(0),
        blocks_flat: vec![
            node("root", "composite", vec![1, 2]),
            node("A", "square", vec![]),
            node("B", "square", vec![]),
        ],
        ..BlockDiagramModel::default()
    }
}

fn assert_invalid(model: &BlockDiagramModel, feature: &'static str) {
    assert_eq!(
        primary(model).unwrap_err(),
        AsciiError::UnsupportedFeature {
            diagram_type: "block",
            feature
        }
    );
}

#[test]
fn direct_models_reject_invalid_root_children_cycles_and_multiple_parents() {
    let mut model = direct_model();
    model.root = None;
    assert_invalid(&model, "missing Block root index");
    model.root = Some(99);
    assert_invalid(&model, "invalid Block root index");
    model = direct_model();
    model.blocks_flat[0].children.push(99);
    assert_invalid(&model, "invalid Block child index");
    model = direct_model();
    model.blocks_flat[0].children.push(1);
    assert_invalid(
        &model,
        "Block child has multiple parents or repeated membership",
    );
    model = direct_model();
    model.blocks_flat[1] = node("frame", "composite", vec![0]);
    assert_invalid(&model, "cyclic Block hierarchy");
    model = direct_model();
    model.blocks_flat[1] = node("frame", "composite", vec![2]);
    assert_invalid(
        &model,
        "Block child has multiple parents or repeated membership",
    );
}

#[test]
fn direct_models_reject_unreachable_or_duplicate_bindings_and_missing_endpoints() {
    let mut model = direct_model();
    model.blocks_flat.push(node("unused", "square", vec![]));
    assert_invalid(&model, "unreachable or cyclic Block database records");
    model = direct_model();
    model.blocks_flat[2].id = "A".into();
    assert_invalid(&model, "ambiguous duplicate Block database bindings");
    model = direct_model();
    model.edges.push(BlockEdge {
        start: "A".into(),
        end: "missing".into(),
        ..BlockEdge::default()
    });
    assert_invalid(&model, "unknown Block edge endpoint");
    let model = parse("block\nA[\"Alpha\"] B[\"Beta\"]\nclass A, B hot\n");
    assert_invalid(&model, "ambiguous duplicate Block database bindings");
}

#[test]
fn invalid_and_enormous_numeric_geometry_never_enters_packing() {
    for invalid in [0.0, -2.0, 1.5, f64::NAN, f64::INFINITY] {
        let mut model = direct_model();
        model.blocks_flat[1].width_in_columns = Some(invalid);
        assert_invalid(&model, "invalid Block column span");
        model = direct_model();
        model.blocks_flat[0].columns = Some(invalid);
        assert_invalid(&model, "invalid Block column count");
    }
    for huge in [1e12, f64::MAX, usize::MAX as f64] {
        let mut model = direct_model();
        model.blocks_flat[1].width_in_columns = Some(huge);
        assert!(matches!(
            primary(&model),
            Err(AsciiError::ResourceLimitExceeded(_))
        ));
    }
}

#[test]
fn exact_output_byte_budget_is_accepted_and_one_less_is_rejected() {
    let model = direct_model();
    let report = primary(&model).unwrap();
    let exact = report.text.len();
    for (limit, succeeds) in [(exact, true), (exact - 1, false)] {
        let result = render(
            &model,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
            AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                .unwrap(),
            &OperationControl::new(),
        );
        if succeeds {
            assert_eq!(result.unwrap().text, report.text);
        } else {
            assert!(matches!(result, Err(AsciiError::ResourceLimitExceeded(_))));
        }
    }
}

#[test]
fn depth_work_and_cancellation_are_terminal_instead_of_fallback() {
    let model = parse(&fixture("typical"));
    for (id, limit) in [
        (AsciiResourceLimitId::MaxNestingDepth, 1),
        (AsciiResourceLimitId::MaxLayoutWorkUnits, 1),
        (AsciiResourceLimitId::MaxGridCells, 1),
    ] {
        assert!(matches!(
            render(
                &model,
                AsciiRenderOptions::ascii(),
                AsciiViewportPolicy::with_max_width(20).overflow(OverflowPolicy::Fallback),
                AsciiResourcePolicy::default()
                    .with_limit(id, limit)
                    .unwrap(),
                &OperationControl::new(),
            ),
            Err(AsciiError::ResourceLimitExceeded(_))
        ));
    }
    for checkpoints in [0, 12] {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(checkpoints);
        assert!(matches!(
            render(
                &model,
                AsciiRenderOptions::ascii(),
                AsciiViewportPolicy::unrestricted(),
                AsciiResourcePolicy::default(),
                &control
            ),
            Err(AsciiError::Cancelled(_))
        ));
    }
}

#[test]
fn narrow_fallback_keeps_hierarchy_spans_spaces_markers_and_exact_metadata() {
    let model = parse(
        "block-beta\ncolumns 3\nA[\"A long authored label with terminal text\"]:2 space\nblock:frame:3\ncolumns 2\nB C\nend\nA x--o C\nclassDef hot fill:#f00,color:#fff\nclass A hot\n",
    );
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        let report = render(
            &model,
            AsciiRenderOptions::ascii().with_terminal_width_profile(profile),
            AsciiViewportPolicy::with_max_width(32).overflow(OverflowPolicy::Fallback),
            AsciiResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(report.projection, AsciiProjection::StructuredText);
        assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
        assert!(report.text.lines().all(|line| width(line, profile) <= 32));
        let unwrapped = report.text.lines().map(str::trim_start).collect::<String>();
        for required in [
            "root: B0",
            "children=[",
            "span=2",
            "spacer-count=",
            "type=\"space\"",
            "columns=2",
            "arrow_cross",
            "arrow_circle",
            "fill:#f00",
            "color:#fff",
            "sourceConfig",
            "sourceTitle",
        ] {
            assert!(
                unwrapped.contains(required),
                "missing {required:?}: {}",
                report.text
            );
        }
        for index in 0..model.blocks_flat.len() {
            assert!(
                report
                    .text
                    .lines()
                    .any(|line| line.starts_with(&format!("B{index} ")))
            );
        }
        assert!(!report.text.contains("blocks_flat["));
        assert!(!report.text.contains("children["));
    }
}
