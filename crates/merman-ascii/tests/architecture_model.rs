#![cfg(feature = "diagram-architecture")]

mod support;

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiGraphPresentation, AsciiLayoutProfile,
    AsciiOutput, AsciiOutputOutcome, AsciiProjection, AsciiRenderOptions, AsciiRenderer,
    AsciiResourceLimitId, AsciiResourcePolicy, AsciiSourceContext, AsciiViewportPolicy,
    OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::architecture::{
    ArchitectureDiagramModel, ArchitectureLayoutDirection, ArchitectureLayoutHintModel,
};
use merman_model::{
    MermaidConfig, OperationControl, ParsedSemanticDiagram, SemanticDiagramModel,
    runtime::RuntimePolicy,
};

const BASIC: &str =
    "architecture-beta\nservice a(server)[Gateway]\nservice b(database)[Storage]\na:R --> L:b\n";
const GROUPS: &str = include_str!(
    "../../../fixtures/architecture/upstream_html_demos_architecture_groups_within_groups_002.mmd"
);
const CONTACTS: &str = include_str!(
    "../../../fixtures/architecture/upstream_html_demos_architecture_group_edges_007.mmd"
);
const JUNCTIONS: &str =
    include_str!("../../../fixtures/architecture/upstream_docs_architecture_junctions_016.mmd");

fn parse(source: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("Architecture source parses")
}

fn model(source: &str) -> ArchitectureDiagramModel {
    let SemanticDiagramModel::Architecture(model) = parse(source).model else {
        panic!("Architecture model")
    };
    model
}

fn render(
    source: &str,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?.render_parsed_report(
        &parse(source),
        viewport,
        &OperationControl::new(),
        &context,
        AsciiResourcePolicy::default(),
    )
}

fn detached(model: &ArchitectureDiagramModel) -> AsciiOutput {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let config = MermaidConfig::empty_object();
    AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_model_report(
            &AsciiDiagram::Architecture(model),
            AsciiSourceContext {
                diagram_type: "architecture",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn scene(output: &AsciiOutput) -> &str {
    assert_eq!(
        output.projection,
        AsciiProjection::Diagrammatic,
        "{}",
        output.text
    );
    output
        .text
        .split_once("Architecture records")
        .expect("complete records")
        .0
}

fn compact(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn records(output: &AsciiOutput) -> &str {
    output.text.split_once("Architecture records").map_or_else(
        || {
            output
                .text
                .strip_prefix("family: architecture\n")
                .unwrap_or(&output.text)
        },
        |(_, records)| records,
    )
}

fn measured(output: &AsciiOutput) {
    assert_eq!(
        support::terminal_extent_with_profile(&output.text, output.width_profile),
        (output.emitted_extent.width, output.emitted_extent.height)
    );
}

fn position(text: &str, needle: &str) -> (usize, usize) {
    text.lines()
        .enumerate()
        .find_map(|(y, row)| row.find(needle).map(|x| (x, y)))
        .unwrap_or_else(|| panic!("missing {needle}:\n{text}"))
}

#[test]
fn services_use_declared_horizontal_geometry_and_complete_edge_records() {
    let output = render(
        BASIC,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let scene = scene(&output);
    let (a, b) = (position(scene, "Gateway"), position(scene, "Storage"));
    assert!(a.0 < b.0, "{scene}");
    assert!(scene.contains('>'), "{scene}");
    let records = compact(output.text.split_once("Architecture records").unwrap().1);
    for value in ["a", "b", "Gateway", "Storage", "E1", "server", "database"] {
        assert!(records.contains(value), "{}", output.text);
    }
    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
    measured(&output);
}

#[test]
fn every_cardinal_port_pair_and_independent_heads_has_a_spatial_primary() {
    for left in ['L', 'R', 'T', 'B'] {
        for right in ['L', 'R', 'T', 'B'] {
            for arrow in ["--", "<--", "-->", "<-->", ">--<"] {
                let source = format!(
                    "architecture-beta\nservice a(server)[Alpha]\nservice b(server)[Beta]\na:{left} {arrow} {right}:b\n"
                );
                let output = render(
                    &source,
                    AsciiRenderOptions::ascii(),
                    AsciiViewportPolicy::unrestricted(),
                )
                .unwrap_or_else(|error| panic!("{left} {arrow} {right}: {error:?}"));
                let scene = scene(&output);
                assert!(scene.contains("Alpha") && scene.contains("Beta"), "{scene}");
                assert_eq!(
                    scene
                        .chars()
                        .filter(|ch| matches!(ch, '<' | '>' | '^' | 'v'))
                        .count(),
                    arrow.chars().filter(|ch| matches!(ch, '<' | '>')).count(),
                    "{scene}"
                );
            }
        }
    }
}

#[test]
fn representative_groups_contacts_and_junctions_render_without_dropping_entities() {
    for source in [GROUPS, CONTACTS, JUNCTIONS] {
        let output = render(
            source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        let scene = scene(&output);
        let model = model(source);
        for index in 1..=model.nodes.len() {
            assert!(scene.contains(&format!("N{index}")), "{scene}");
        }
        for index in 1..=model.groups.len() {
            assert!(scene.contains(&format!("G{index}")), "{scene}");
        }
        for index in 1..=model.edges.len() {
            assert!(scene.contains(&format!("E{index}")), "{scene}");
        }
        measured(&output);
    }
}

#[test]
fn aligned_rows_and_columns_preserve_order_and_center_across_groups() {
    let source = "architecture-beta\ngroup top(cloud)[Top]\ngroup bottom(cloud)[Bottom]\nservice a(server)[Alpha] in top\nservice b(server)[Bravo] in top\nservice c(server)[Charlie] in bottom\nservice d(server)[Delta] in bottom\na:B --> T:c\nb:B --> T:d\nalign row a b\nalign row c d\nalign column a c\nalign column b d\n";
    let output = render(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let scene = scene(&output);
    let (a, b, c, d) = (
        position(scene, "Alpha"),
        position(scene, "Bravo"),
        position(scene, "Charlie"),
        position(scene, "Delta"),
    );
    assert_eq!(a.1, b.1, "{scene}");
    assert_eq!(c.1, d.1, "{scene}");
    assert!(a.0 < b.0 && c.0 < d.0 && a.1 < c.1, "{scene}");
    assert_eq!(
        a.0 + "Alpha".len() / 2,
        c.0 + "Charlie".len() / 2,
        "{scene}"
    );
    assert_eq!(b.0 + "Bravo".len() / 2, d.0 + "Delta".len() / 2, "{scene}");
}

#[test]
fn exclusive_opposite_ports_keep_a_straight_spine_and_perpendicular_junction_branches() {
    let source = "architecture-beta\nservice user(internet)[User]\nservice gateway(server)[Gateway]\nservice a(server)[Worker A]\nservice b(server)[Worker B]\njunction hub\nuser:R -- L:gateway\ngateway:R -- L:hub\na:B -- T:hub\nb:T -- B:hub\n";
    let output = render(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let scene = scene(&output);
    let center = |text: &str| {
        let (x, y) = position(scene, text);
        (x + text.len() / 2, y)
    };
    let (user, gateway, a, b, hub) = (
        center("N1 User"),
        center("N2 Gateway"),
        center("N3 Worker A"),
        center("N4 Worker B"),
        center("N5 [+]"),
    );
    assert_eq!(user.1, gateway.1, "{scene}");
    assert_eq!(gateway.1, hub.1, "{scene}");
    assert!(user.0 < gateway.0 && gateway.0 < hub.0, "{scene}");
    assert_eq!(a.0, b.0, "{scene}");
    // An even-width diamond caption rounds one cell beside its exact contour center.
    // Geometry-level tests assert exact port and center coordinates independently.
    assert!(a.0.abs_diff(hub.0) <= 1, "{scene}");
    assert!(a.1 < hub.1 && hub.1 < b.1, "{scene}");
}

#[test]
fn ambiguous_or_nonspatial_detached_models_are_honest_complete_records() {
    let base = model(BASIC);
    let mut cases = Vec::new();
    let mut empty_id = base.clone();
    empty_id.nodes[0].id.clear();
    cases.push(empty_id);
    let mut duplicate = base.clone();
    duplicate.nodes.push(duplicate.nodes[0].clone());
    cases.push(duplicate);
    let mut missing = base.clone();
    missing.edges[0].rhs_id = "Missing endpoint".into();
    cases.push(missing);
    let mut side = base.clone();
    side.edges[0].lhs_dir = 'X';
    cases.push(side);
    let mut group = base.clone();
    group.edges[0].lhs_group = Some(true);
    cases.push(group);
    let mut owner = base.clone();
    owner.nodes[0].in_group = Some("Unknown group".into());
    cases.push(owner);
    let mut align = base.clone();
    for members in [["a", "b"], ["b", "a"]] {
        align.layout_hints.push(ArchitectureLayoutHintModel {
            direction: ArchitectureLayoutDirection::Row,
            members: members.into_iter().map(String::from).collect(),
        });
    }
    cases.push(align);
    let mut repeated = base.clone();
    repeated.layout_hints.push(ArchitectureLayoutHintModel {
        direction: ArchitectureLayoutDirection::Row,
        members: vec!["a".into(), "a".into()],
    });
    cases.push(repeated);
    for model in cases {
        let output = detached(&model);
        assert_eq!(
            output.projection,
            AsciiProjection::StructuredText,
            "{}",
            output.text
        );
        assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
        assert!(!output.fallback.attempted);
        assert!(output.text.contains("Gateway") && output.text.contains("Storage"));
    }
}

#[test]
fn equivalent_centers_and_three_member_order_cycles_keep_complete_records() {
    for hints in [
        "align row a b\nalign row b c\nalign row c a\n",
        "align row a b\nalign column a b\n",
    ] {
        let source = format!(
            "architecture-beta\nservice a(server)[Alpha]\nservice b(server)[Beta]\nservice c(server)[Gamma]\n{hints}"
        );
        let output = render(
            &source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        assert_eq!(
            output.projection,
            AsciiProjection::StructuredText,
            "{}",
            output.text
        );
        for value in ["Alpha", "Beta", "Gamma", "A1", "A2"] {
            assert!(output.text.contains(value));
        }
        assert!(!output.fallback.attempted);
    }
}

#[test]
fn narrow_viewports_reflow_the_complete_records() {
    for width in [60, 80, 100, 120] {
        let natural = render(
            CONTACTS,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        let output = render(
            CONTACTS,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
        )
        .unwrap();
        assert!(output.emitted_extent.width <= width);
        assert_eq!(compact(records(&output)), compact(records(&natural)));
        measured(&output);
    }
}

#[test]
fn cjk_width_and_all_primary_encodings_keep_authored_text() {
    let source = BASIC
        .replace("Gateway", "接收请求")
        .replace("Storage", "写入队列");
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        for color in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let options = AsciiRenderOptions::unicode()
                .with_color_mode(color)
                .with_terminal_width_profile(profile);
            let output = render(&source, options, AsciiViewportPolicy::unrestricted()).unwrap();
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            assert!(output.text.contains("接收请求") && output.text.contains("写入队列"));
        }
    }
}

#[test]
fn only_advertised_layout_and_presentation_profiles_are_admitted() {
    for layout in [AsciiLayoutProfile::Auto, AsciiLayoutProfile::Compact] {
        let mut options = AsciiRenderOptions::ascii();
        options.layout_profile = layout;
        assert!(render(BASIC, options, AsciiViewportPolicy::unrestricted()).is_err());
    }
    let mut options = AsciiRenderOptions::ascii();
    options.graph_presentation = AsciiGraphPresentation::Adaptive;
    assert!(render(BASIC, options, AsciiViewportPolicy::unrestricted()).is_err());
}

#[test]
fn resource_and_cancellation_failures_never_turn_into_record_success() {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let renderer = AsciiRenderer::new(AsciiRenderOptions::ascii()).unwrap();
    for limit in [
        AsciiResourceLimitId::MaxGridCells,
        AsciiResourceLimitId::MaxOutputBytes,
        AsciiResourceLimitId::MaxLayoutWorkUnits,
        AsciiResourceLimitId::MaxNestingDepth,
    ] {
        let result = renderer.render_parsed_report(
            &parse(GROUPS),
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default().with_limit(limit, 1).unwrap(),
        );
        assert!(
            matches!(&result,Err(AsciiError::ResourceLimitExceeded(details)) if details.limit==limit),
            "{limit:?}: {result:?}"
        );
    }
    for checkpoints in [0, 16] {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(checkpoints);
        let result = renderer.render_parsed_report(
            &parse(GROUPS),
            AsciiViewportPolicy::unrestricted(),
            &control,
            &context,
            AsciiResourcePolicy::default(),
        );
        assert!(
            matches!(result, Err(AsciiError::Cancelled(_))),
            "{result:?}"
        );
    }
}
