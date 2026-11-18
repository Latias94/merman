#![cfg(feature = "diagram-c4")]

mod support;

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiFallbackReason, AsciiGraphPresentation,
    AsciiLayoutProfile, AsciiOutput, AsciiOutputEncoding, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
    AsciiSourceContext, AsciiViewportPolicy, Lossiness, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::c4::{C4DiagramModel, C4RelationOffset, C4Text};
use merman_model::{
    MermaidConfig, OperationControl, ParsedSemanticDiagram, SemanticDiagramModel,
    runtime::RuntimePolicy,
};
use serde_json::json;

const BASIC: &str = include_str!("../../../fixtures/c4/basic.mmd");
const MEDIUM: &str = include_str!("../../merman/benches/fixtures/c4_medium.mmd");
const NESTED: &str = "C4Container\nBoundary(outer, \"Outer\") {\nContainer_Boundary(inner, \"Inner\") {\nContainer(api, \"Endpoint\", \"Rust\", \"Serve requests\")\n}\n}\n";

fn parse(source: &str) -> ParsedSemanticDiagram {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("C4 source parses");
    assert_eq!(parsed.metadata.diagram_type, "c4");
    parsed
}

fn c4(parsed: &ParsedSemanticDiagram) -> &C4DiagramModel {
    let SemanticDiagramModel::C4(model) = &parsed.model else {
        panic!("expected a C4 semantic model");
    };
    model
}

fn model(source: &str) -> C4DiagramModel {
    c4(&parse(source)).clone()
}

fn report_with(
    parsed: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
    control: &OperationControl,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?
        .render_parsed_report(parsed, viewport, control, &context, resources)
}

fn report(source: &str, options: AsciiRenderOptions, viewport: AsciiViewportPolicy) -> AsciiOutput {
    report_with(
        &parse(source),
        options,
        viewport,
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap()
}

fn borrowed(model: &C4DiagramModel, viewport: AsciiViewportPolicy) -> AsciiOutput {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let config = MermaidConfig::empty_object();
    AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_model_report(
            &AsciiDiagram::C4(model),
            AsciiSourceContext {
                diagram_type: "c4",
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

fn bounded(width: usize) -> AsciiViewportPolicy {
    AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback)
}

fn assert_measured(output: &AsciiOutput) {
    assert_eq!(
        support::terminal_extent_with_profile(&output.text, output.width_profile),
        (output.emitted_extent.width, output.emitted_extent.height),
        "{}",
        output.text
    );
    assert!(!output.emitted_overflowed, "{}", output.text);
}

fn assert_primary(output: &AsciiOutput, projection: AsciiProjection) {
    assert_eq!(output.projection, projection, "{}", output.text);
    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
    assert_eq!(output.primary_extent, Some(output.emitted_extent));
    assert_eq!(output.lossiness, Lossiness::None);
    assert!(!output.fallback.attempted);
    assert_eq!(
        output.requested_layout_profile,
        AsciiLayoutProfile::Canonical
    );
    assert_eq!(
        output.requested_graph_presentation,
        AsciiGraphPresentation::Strict
    );
    assert!(!output.compact_attempted);
    assert_measured(output);
}

fn topology(output: &AsciiOutput) -> &str {
    assert_eq!(output.projection, AsciiProjection::Diagrammatic);
    output
        .text
        .split_once("C4 records")
        .expect("diagrammatic C4 output appends complete reading records")
        .0
}

fn records(output: &AsciiOutput) -> &str {
    output.text.split_once("C4 records").map_or_else(
        || {
            output
                .text
                .strip_prefix("family: c4\n")
                .unwrap_or(&output.text)
        },
        |(_, records)| records,
    )
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}

fn assert_records(text: &str, expected: &[&str]) {
    let actual = compact(text);
    for expected in expected {
        assert!(
            actual.contains(&compact(expected)),
            "missing {expected:?}:\n{text}"
        );
    }
}

fn assert_keys(output: &AsciiOutput, model: &C4DiagramModel) {
    for (prefix, count) in [
        ('S', model.shapes.len()),
        ('B', model.boundaries.len()),
        ('E', model.rels.len()),
    ] {
        for index in 1..=count {
            assert!(
                records(output)
                    .lines()
                    .any(|line| line.starts_with(&format!("{prefix}{index} "))),
                "missing {prefix}{index}:\n{}",
                output.text
            );
        }
    }
}

fn position(text: &str, label: &str) -> (usize, usize) {
    text.lines()
        .enumerate()
        .find_map(|(y, line)| line.find(label).map(|x| (x, y)))
        .unwrap_or_else(|| panic!("missing {label:?}:\n{text}"))
}

#[test]
fn all_five_headers_keep_role_boxes_and_complete_records_at_terminal_widths() {
    for header in [
        "C4Context",
        "C4Container",
        "C4Component",
        "C4Dynamic",
        "C4Deployment",
    ] {
        let source = BASIC.replacen("C4Context", header, 1);
        let parsed = parse(&source);
        assert_eq!(c4(&parsed).c4_type, header);
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for width in [80, 100, 120] {
                let output = report(&source, options, bounded(width));
                assert_primary(&output, AsciiProjection::Diagrammatic);
                assert!(output.emitted_extent.width <= width);
                assert_records(
                    topology(&output),
                    &["Customer", "Banking System", "[Person]", "[System]", "E1"],
                );
                assert!(!topology(&output).contains("global"));
                assert_keys(&output, c4(&parsed));
                assert_records(
                    records(&output),
                    &[
                        &format!("C4 view=\"{}\"", header.strip_prefix("C4").unwrap()),
                        "customerA",
                        "sys",
                        "A customer",
                        "Does banking",
                        "Uses",
                        "global",
                    ],
                );
            }
        }
    }
}

#[test]
fn unknown_view_discriminators_remain_exact_records() {
    let mut data = model(BASIC);
    data.c4_type = "C4FutureView".into();
    let output = borrowed(&data, AsciiViewportPolicy::unrestricted());
    assert_primary(&output, AsciiProjection::StructuredText);
    assert_keys(&output, &data);
    assert_records(records(&output), &["c4Type:\"C4FutureView\""]);
}

#[test]
fn canonical_and_presentation_parser_entries_keep_the_same_c4_records() {
    let canonical = report(
        BASIC,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    let presentation = support::parse_model(BASIC);
    let output = support::render_model_report(
        &presentation,
        &AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_primary(&output, AsciiProjection::Diagrammatic);
    assert_keys(&output, &model(BASIC));
    assert_eq!(compact(records(&output)), compact(records(&canonical)));
}

#[test]
fn medium_and_upstream_documentation_fixtures_keep_every_declaration_and_relationship() {
    for (specimen, source) in [
        ("c4_medium.mmd", MEDIUM),
        (
            "upstream_docs_c4_c4_diagrams_001.mmd",
            include_str!("../../../fixtures/c4/upstream_docs_c4_c4_diagrams_001.mmd"),
        ),
        (
            "upstream_docs_c4_c4_container_diagram_c4container_006.mmd",
            include_str!(
                "../../../fixtures/c4/upstream_docs_c4_c4_container_diagram_c4container_006.mmd"
            ),
        ),
        (
            "upstream_docs_c4_c4_component_diagram_c4component_008.mmd",
            include_str!(
                "../../../fixtures/c4/upstream_docs_c4_c4_component_diagram_c4component_008.mmd"
            ),
        ),
        (
            "upstream_docs_c4_c4_dynamic_diagram_c4dynamic_010.mmd",
            include_str!(
                "../../../fixtures/c4/upstream_docs_c4_c4_dynamic_diagram_c4dynamic_010.mmd"
            ),
        ),
        (
            "upstream_docs_c4_c4_deployment_diagram_c4deployment_012.mmd",
            include_str!(
                "../../../fixtures/c4/upstream_docs_c4_c4_deployment_diagram_c4deployment_012.mmd"
            ),
        ),
    ] {
        eprintln!("C4 specimen: {specimen}");
        let parsed = parse(source);
        let natural = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap_or_else(|error| panic!("{specimen}, unrestricted viewport: {error:?}"));
        assert_primary(&natural, AsciiProjection::Diagrammatic);
        assert_keys(&natural, c4(&parsed));
        for shape in &c4(&parsed).shapes {
            assert_records(records(&natural), &[&shape.alias, shape.label.as_str()]);
        }
        for boundary in &c4(&parsed).boundaries {
            assert_records(
                records(&natural),
                &[&boundary.alias, boundary.label.as_str()],
            );
        }
        for relation in &c4(&parsed).rels {
            assert_records(
                records(&natural),
                &[
                    &relation.from_alias,
                    &relation.to_alias,
                    relation.label.as_str(),
                ],
            );
        }
        for width in [80, 100, 120] {
            let output = report_with(
                &parsed,
                AsciiRenderOptions::ascii(),
                bounded(width),
                &OperationControl::new(),
                AsciiResourcePolicy::default(),
            )
            .unwrap_or_else(|error| panic!("{specimen}, max_width={width}: {error:?}"));
            assert_measured(&output);
            assert!(output.emitted_extent.width <= width);
            assert_keys(&output, c4(&parsed));
            assert_eq!(compact(records(&output)), compact(records(&natural)));
            if natural.emitted_extent.width <= width {
                assert_primary(&output, AsciiProjection::Diagrammatic);
            } else {
                assert_eq!(output.projection, AsciiProjection::StructuredText);
                assert_eq!(output.outcome, AsciiOutputOutcome::Fallback);
                assert_eq!(
                    output.fallback.reason,
                    Some(AsciiFallbackReason::PrimaryOverflow)
                );
            }
        }
    }
}

#[test]
fn duplicate_display_labels_keep_distinct_aliases_and_endpoint_keys() {
    let source = "C4Context\nPerson(first, \"Same\")\nPerson(second, \"Same\")\nRel(first, second, \"Connects\")\n";
    let output = report(source, AsciiRenderOptions::ascii(), bounded(100));
    assert_primary(&output, AsciiProjection::Diagrammatic);
    assert_eq!(topology(&output).matches("Same").count(), 2);
    assert_records(
        records(&output),
        &["S1", "S2", "first", "second", "source=[S1]", "target=[S2]"],
    );
    assert!(!records(&output).contains("(ambiguous)"));
}

#[test]
fn row_packing_follows_declaration_order_instead_of_relationship_direction() {
    let source = "C4Context\nPerson(z, \"Zed\")\nPerson(a, \"Alpha\")\nPerson(m, \"Middle\")\nPerson(t, \"Tail\")\nUpdateLayoutConfig($c4ShapeInRow=\"2\", $c4BoundaryInRow=\"1\")\nRel(t, z, \"Returns\")\n";
    let output = report(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_primary(&output, AsciiProjection::Diagrammatic);
    let scene = topology(&output);
    let (zed, alpha, middle, tail) = (
        position(scene, "Zed"),
        position(scene, "Alpha"),
        position(scene, "Middle"),
        position(scene, "Tail"),
    );
    assert!(zed.0 < alpha.0 && middle.0 < tail.0, "{scene}");
    assert_eq!(zed.1, alpha.1, "{scene}");
    assert_eq!(middle.1, tail.1, "{scene}");
    assert!(zed.1 < middle.1, "{scene}");
    assert_records(
        records(&output),
        &[
            r#"layout:{"c4ShapeInRow":2,"c4BoundaryInRow":1}"#,
            "source=[S4]",
            "target=[S1]",
        ],
    );
}

#[test]
fn sibling_boundary_rows_and_nested_frames_keep_members_inside_their_containers() {
    let source = format!(
        "{NESTED}Boundary(side, \"Side\") {{\nPerson(peer, \"Peer\")\n}}\nUpdateLayoutConfig($c4ShapeInRow=\"2\", $c4BoundaryInRow=\"1\")\n"
    );
    let output = report(
        &source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_primary(&output, AsciiProjection::Diagrammatic);
    let scene = topology(&output);
    let outer = position(scene, "Outer");
    let inner = position(scene, "Inner");
    let child = position(scene, "Endpoint");
    let side = position(scene, "Side");
    assert!(
        outer.1 < inner.1 && inner.1 < child.1 && child.1 < side.1,
        "{scene}"
    );
    let row = scene.lines().nth(child.1).unwrap();
    assert!(
        row[..child.0].chars().filter(|&ch| ch == '|').count() >= 3,
        "{scene}"
    );
    assert!(
        row[child.0 + "Endpoint".len()..]
            .chars()
            .filter(|&ch| ch == '|')
            .count()
            >= 3,
        "{scene}"
    );
    assert_records(
        records(&output),
        &[
            "B1",
            "B2",
            "B3",
            "B4",
            "global",
            "outer",
            "inner",
            "side",
            "parentBoundary",
        ],
    );
}

#[test]
fn redeclaring_a_shape_updates_its_container_without_duplicating_or_reordering_it() {
    let source = "C4Container\nContainer(a, \"Before\", \"Old\")\nContainer(b, \"Second\")\nBoundary(zone, \"Zone\") {\nContainer(a, \"After\", \"Rust\")\n}\nRel(b, a, \"Calls\")\n";
    let parsed = parse(source);
    assert_eq!(c4(&parsed).shapes.len(), 2);
    assert_eq!(c4(&parsed).shapes[0].parent_boundary, "zone");
    let output = report(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_primary(&output, AsciiProjection::Diagrammatic);
    assert_eq!(topology(&output).matches("After").count(), 1);
    assert!(!output.text.contains("Before"));
    assert!(position(topology(&output), "Zone").1 < position(topology(&output), "After").1);
    assert_records(
        records(&output),
        &[
            "S1",
            "S2",
            "After",
            "Rust",
            "zone",
            "source=[S2]",
            "target=[S1]",
        ],
    );
}

#[test]
fn reverse_and_double_markers_have_independent_arrow_ink_at_the_correct_end() {
    for (relation, source_arrow, target_arrow) in [
        ("Rel", false, true),
        ("Rel_Back", true, false),
        ("BiRel", true, true),
        ("Rel_U", false, true),
        ("Rel_D", false, true),
        ("Rel_L", false, true),
        ("Rel_R", false, true),
    ] {
        let source = format!(
            "C4Context\nPerson(a, \"Left\")\nSystem(b, \"Right\")\n{relation}(a, b, \"Ping\")\n"
        );
        let output = report(
            &source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        );
        assert_primary(&output, AsciiProjection::Diagrammatic);
        let scene = topology(&output);
        let left = position(scene, "Left");
        let right = position(scene, "Right");
        assert!(left.0 < right.0, "{relation}:\n{scene}");
        assert_eq!(
            left.1, right.1,
            "directional macro must not repack declarations:\n{scene}"
        );
        // Only topology ink is inspected; record prose and relation names cannot satisfy this.
        let arrows: Vec<_> = scene
            .lines()
            .flat_map(|line| {
                line.char_indices()
                    .filter(|(_, ch)| matches!(ch, '<' | '>' | '^' | 'v'))
                    .map(|(x, _)| x)
            })
            .collect();
        let midpoint = (left.0 + right.0) / 2;
        assert_eq!(
            arrows.len(),
            usize::from(source_arrow) + usize::from(target_arrow),
            "{relation}:\n{scene}"
        );
        assert_eq!(
            arrows.iter().filter(|&&x| x < midpoint).count(),
            usize::from(source_arrow),
            "{relation}:\n{scene}"
        );
        assert_eq!(
            arrows.iter().filter(|&&x| x > midpoint).count(),
            usize::from(target_arrow),
            "{relation}:\n{scene}"
        );
        assert_records(records(&output), &["source=[S1]", "target=[S2]", "Ping"]);
    }
}

#[test]
fn a_boundary_is_a_real_relationship_endpoint_and_global_is_never_silently_dropped() {
    let source = "C4Context\nPerson(actor, \"Actor\")\nBoundary(zone, \"Zone\") {\nSystem(app, \"Service\")\n}\nRel(actor, zone, \"Enters\")\n";
    let output = report(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_primary(&output, AsciiProjection::Diagrammatic);
    assert_records(topology(&output), &["Zone", "Service", "E1"]);
    assert_records(
        records(&output),
        &["source=[S1]", "target=[B2]", "actor", "zone"],
    );

    let explicit = source.replace("Rel(actor, zone", "Rel(actor, global");
    let output = report(
        &explicit,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
    assert!(!output.fallback.attempted);
    assert_records(
        records(&output),
        &["source=[S1]", "target=[B1]", "global", "Enters"],
    );
    if output.projection == AsciiProjection::Diagrammatic {
        assert!(
            topology(&output).contains("global"),
            "explicit global endpoint needs a visible frame:\n{}",
            output.text
        );
    } else {
        assert_primary(&output, AsciiProjection::StructuredText);
    }
}

#[test]
fn dynamic_numbering_uses_final_ordered_pair_records_after_overwrite() {
    let source = "C4Dynamic\nPerson(a, \"Alpha\")\nSystem(b, \"Beta\")\nSystem(c, \"Gamma\")\nRelIndex(914, a, b, \"Discarded\")\nRelIndex(82, b, c, \"Follows\")\nRelIndex(713, a, b, \"Final\", \"HTTPS\")\nRel(c, a, \"Returns\")\n";
    let parsed = parse(source);
    let data = c4(&parsed);
    assert_eq!(data.rels.len(), 3);
    assert_eq!(
        data.rels
            .iter()
            .map(|rel| (rel.from_alias.as_str(), rel.to_alias.as_str()))
            .collect::<Vec<_>>(),
        [("a", "b"), ("b", "c"), ("c", "a")]
    );
    let output = report(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    assert_primary(&output, AsciiProjection::Diagrammatic);
    assert_records(topology(&output), &["1: E1", "2: E2", "3: E3"]);
    assert_records(
        records(&output),
        &[
            "E1 [relationship] type=\"rel\" source=[S1] alias=\"a\" target=[S2] alias=\"b\" heads=target render-number=1",
            "E2 [relationship] type=\"rel\" source=[S2] alias=\"b\" target=[S3] alias=\"c\" heads=target render-number=2",
            "E3 [relationship] type=\"rel\" source=[S3] alias=\"c\" target=[S1] alias=\"a\" heads=target render-number=3",
            "Final",
            "HTTPS",
            "Follows",
            "Returns",
        ],
    );
    for discarded in ["Discarded", "914", "82", "713", "E4 "] {
        assert!(
            !output.text.contains(discarded),
            "{discarded}:\n{}",
            output.text
        );
    }
}

#[test]
fn relationship_override_keeps_source_direction_and_all_style_fields() {
    let source = include_str!("../../../fixtures/c4/upstream_rel_override_and_style_spec.mmd");
    let parsed = parse(source);
    assert_eq!(c4(&parsed).rels.len(), 1);
    assert_eq!(c4(&parsed).rels[0].rel_type, "rel_b");
    let output = report(source, AsciiRenderOptions::ascii(), bounded(100));
    assert_primary(&output, AsciiProjection::Diagrammatic);
    assert_records(
        records(&output),
        &[
            "rel_b",
            "override",
            "tech2",
            "d2",
            "textColor:\"red\"",
            "lineColor:\"blue\"",
            "offsetX:10",
            "offsetY:20",
            "source=[S1]",
            "target=[S2]",
        ],
    );
    assert!(!output.text.contains("first"));
    assert!(!output.text.contains("tech1"));
    assert!(!output.text.lines().any(|line| line.starts_with("E2 ")));
}

#[test]
fn missing_aliases_are_structured_and_cross_kind_overlaps_keep_shape_lookup_priority() {
    for (source, projection, expected) in [
        (
            "C4Context\nPerson(known, \"Known\")\nRel(known, missing, \"Unresolved\")\n",
            AsciiProjection::StructuredText,
            vec!["target=[]", "unresolved", "missing", "Known", "Unresolved"],
        ),
        (
            "C4Context\nPerson(shared, \"Actor\")\nBoundary(shared, \"Group\") {\nSystem(app, \"Application\")\n}\nRel(shared, app, \"Ambiguous\")\n",
            AsciiProjection::Diagrammatic,
            vec![
                "source=[S1]",
                "shadowed-boundaries=[B2]",
                "identity diagnostic:",
                "shapes=[S1]",
                "boundaries=[B2]",
                "Actor",
                "Group",
                "Application",
            ],
        ),
    ] {
        let output = report(source, AsciiRenderOptions::ascii(), bounded(80));
        assert_primary(&output, projection);
        assert_records(records(&output), &expected);
        assert_keys(&output, &model(source));
    }
}

#[test]
fn invalid_borrowed_hierarchies_keep_all_records_instead_of_erasing_or_looping() {
    let baseline = model(&format!("{NESTED}Rel(outer, api, \"Exposes\")\n"));
    let mut cases = Vec::new();
    let mut missing_parent = baseline.clone();
    missing_parent.shapes[0].parent_boundary = "absent".into();
    cases.push((
        missing_parent,
        "parentBoundary=[] (unresolved) alias=\"absent\"",
    ));
    let mut cycle = baseline.clone();
    cycle.boundaries[1].parent_boundary = "inner".into();
    cases.push((cycle, "parentBoundary=[B3] alias=\"inner\""));
    let mut duplicate = baseline.clone();
    duplicate.shapes.push(duplicate.shapes[0].clone());
    cases.push((duplicate, "target=[S1,S2] (ambiguous)"));
    let mut duplicate_boundary = baseline;
    duplicate_boundary
        .boundaries
        .push(duplicate_boundary.boundaries[1].clone());
    cases.push((duplicate_boundary, "source=[B2,B4] (ambiguous)"));
    for (data, diagnostic) in cases {
        let output = borrowed(&data, AsciiViewportPolicy::unrestricted());
        assert_primary(&output, AsciiProjection::StructuredText);
        assert_keys(&output, &data);
        assert_records(
            records(&output),
            &[
                "Outer",
                "Inner",
                "Endpoint",
                "Rust",
                "Serve requests",
                "Exposes",
                diagnostic,
            ],
        );
    }
}

#[test]
fn a_hidden_global_nested_under_a_visible_frame_uses_complete_records() {
    let mut data = model(concat!(
        "C4Context\n",
        "Person(inside, \"Inside\")\n",
        "Person(outside, \"Outside\")\n",
        "Boundary(outer, \"Outer\") {\n",
        "Rel(inside, outside, \"Calls\")\n}\n",
    ));
    data.shapes[1].parent_boundary.clear();
    data.boundaries[0].parent_boundary = "outer".into();
    data.boundaries[1].parent_boundary.clear();
    let output = borrowed(&data, AsciiViewportPolicy::unrestricted());
    assert_primary(&output, AsciiProjection::StructuredText);
    assert_keys(&output, &data);
    assert_records(
        records(&output),
        &[
            "Inside",
            "Outside",
            "Outer",
            "global",
            "Calls",
            "B1 [boundary] alias=\"global\" parentBoundary=[B2] alias=\"outer\"",
            "E1 [relationship] type=\"rel\" source=[S1] alias=\"inside\" target=[S2] alias=\"outside\"",
        ],
    );
}

#[test]
fn invalid_row_counts_are_inspectable_structured_primaries() {
    for invalid in [0.0, -1.0, 1.5, f64::NAN, f64::INFINITY] {
        for shape_rows in [true, false] {
            let mut data = model(BASIC);
            if shape_rows {
                data.layout.c4_shape_in_row = invalid;
            } else {
                data.layout.c4_boundary_in_row = invalid;
            }
            let output = borrowed(&data, AsciiViewportPolicy::unrestricted());
            assert_primary(&output, AsciiProjection::StructuredText);
            assert_keys(&output, &data);
            assert_records(
                records(&output),
                &[
                    "c4ShapeInRow",
                    "c4BoundaryInRow",
                    "Customer",
                    "Banking System",
                    "Uses",
                ],
            );
        }
    }
}

#[test]
fn object_valued_c4_text_remains_raw_and_selects_an_honest_text_primary() {
    let source = include_str!("../../../fixtures/c4/nesting_updates.mmd");
    let output = report(source, AsciiRenderOptions::ascii(), bounded(80));
    assert_primary(&output, AsciiProjection::StructuredText);
    assert_keys(&output, &model(source));
    assert_records(
        records(&output),
        &[
            r#"{"sprite":"users"}"#,
            "Node 1",
            "Boundary 1",
            "P2",
            "override",
            "offsetX:10",
            "offsetY:20",
        ],
    );
    for value in [
        C4Text::Wrapped {
            text: json!({"opaque": ["kept", 7, false, null]}),
        },
        C4Text::Value(json!({"opaque": ["kept", 7, false, null]})),
    ] {
        let mut data = model(BASIC);
        data.rels[0].descr = Some(value);
        let output = borrowed(&data, AsciiViewportPolicy::unrestricted());
        assert_primary(&output, AsciiProjection::StructuredText);
        assert_records(
            records(&output),
            &[
                r#"{"opaque":["kept",7,false,null]}"#,
                "Customer",
                "Banking System",
                "Uses",
            ],
        );
    }
}

#[test]
fn cjk_emoji_and_multiline_labels_keep_terminal_cell_measurements_and_raw_text() {
    let source = "C4Context\nPerson(actor, \"客户👨‍👩‍👧‍👦<br/>第二行\", \"组合 é\")\nSystem(app, \"服务🙂\", \"查询<br/>记录\")\nRel(actor, app, \"提交<br/>确认\")\n";
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            let options = options.with_terminal_width_profile(profile);
            let output = report(source, options, AsciiViewportPolicy::unrestricted());
            assert_primary(&output, AsciiProjection::Diagrammatic);
            for label in ["客户", "👨‍👩‍👧‍👦", "第二行", "服务", "🙂", "E1"]
            {
                assert!(
                    topology(&output).contains(label),
                    "missing {label}:\n{}",
                    output.text
                );
            }
            assert!(!topology(&output).contains("<br"));
            assert_ne!(
                position(topology(&output), "客户").1,
                position(topology(&output), "第二行").1
            );
            assert_records(
                records(&output),
                &[
                    "客户👨‍👩‍👧‍👦<br/>第二行",
                    "组合 é",
                    "查询<br/>记录",
                    "提交<br/>确认",
                ],
            );
            let narrow = report(source, options, bounded(20));
            assert_eq!(narrow.projection, AsciiProjection::StructuredText);
            assert_eq!(narrow.outcome, AsciiOutputOutcome::Fallback);
            assert_measured(&narrow);
            assert!(narrow.emitted_extent.width <= 20);
            assert_eq!(compact(records(&narrow)), compact(records(&output)));
        }
    }
}

#[test]
fn complete_keyed_records_retain_every_shape_boundary_and_relation_metadata_field() {
    let source = "%%{wrap}%%\nC4Deployment\ntitle Runtime\naccDescr: Complete metadata\nNode(host, \"Host\", \"Linux\", \"Runtime node\") {\nContainer(api, \"API\", \"Rust\", \"Serves\", \"code\", \"api-tag\", \"https://example.com/api\")\n}\nPerson_Ext(user, \"Guest\", \"External user\")\nRel_Back(api, user, \"Replies\", \"HTTPS\", \"Encrypted\", \"wire\", \"rel-tag\", \"https://example.com/rel\")\nUpdateElementStyle(api, $bgColor=\"red\", $fontColor=\"white\", $borderColor=\"black\", $shadowing=\"true\", $shape=\"rounded\", $sprite=\"code\", $techn=\"Rust\", $legendText=\"ServiceLegend\", $legendSprite=\"book\")\nUpdateElementStyle(host, $bgColor=\"blue\", $fontColor=\"yellow\", $borderColor=\"gray\", $shadowing=\"false\", $shape=\"rect\", $sprite=\"package\", $techn=\"Tech\", $legendText=\"HostLegend\", $legendSprite=\"server\")\nUpdateRelStyle(api, user, $textColor=\"red\", $lineColor=\"blue\", $offsetX=\"10\", $offsetY=\"20\")\nUpdateLayoutConfig($c4ShapeInRow=\"1\", $c4BoundaryInRow=\"1\")\n";
    let mut data = model(source);
    data.acc_title = Some("Accessible runtime".into());
    data.shapes[0].ty = Some(C4Text::String("service-kind".into()));
    data.shapes[0].sprite = Some(json!({"raw": ["code", true, null]}));
    data.boundaries[1].tags = Some(json!(["host-tag", 3]));
    data.boundaries[1].link = Some(json!("https://example.com/host"));
    data.rels[0].offset_y = Some(C4RelationOffset::String("20px".into()));
    let natural = borrowed(&data, AsciiViewportPolicy::unrestricted());
    assert_primary(&natural, AsciiProjection::Diagrammatic);
    assert_keys(&natural, &data);
    assert_records(
        records(&natural),
        &[
            "S1 [shape] alias=\"api\" parentBoundary=[B2] alias=\"host\"",
            "S1 typeC4Shape={text:\"container\"}",
            "S1 type=\"service-kind\"",
            "S1 label={text:\"API\"}",
            "S1 techn=\"Rust\"",
            "S1 descr={text:\"Serves\"}",
            "B2 [boundary] alias=\"host\" parentBoundary=[B1] alias=\"global\"",
            "B2 label={text:\"Host\"}",
            "B2 type={text:\"Linux\"}",
            "B2 descr={text:\"Runtime node\"}",
            "E1 [relationship] type=\"rel_b\" source=[S1] alias=\"api\" target=[S2] alias=\"user\" heads=source",
            "E1 label={text:\"Replies\"}",
            "E1 techn={text:\"HTTPS\"}",
            "E1 descr={text:\"Encrypted\"}",
        ],
    );
    assert_records(
        records(&natural),
        &[
            "Runtime",
            "Accessible runtime",
            "Complete metadata",
            "wrap:true",
            r#"layout:{"c4ShapeInRow":1,"c4BoundaryInRow":1}"#,
            "typeC4Shape",
            "parentBoundary",
            "service-kind",
            "Rust",
            "Serves",
            r#"{"raw":["code",true,null]}"#,
            "api-tag",
            "https://example.com/api",
            "bgColor:\"red\"",
            "borderColor:\"black\"",
            "fontColor:\"white\"",
            "shadowing:\"true\"",
            "shape:\"rounded\"",
            "legendText:\"ServiceLegend\"",
            "legendSprite:\"book\"",
            "Linux",
            "Runtime node",
            "nodeType:\"node\"",
            "bgColor:\"blue\"",
            "borderColor:\"gray\"",
            "fontColor:\"yellow\"",
            "shadowing:\"false\"",
            "shape:\"rect\"",
            "sprite:\"package\"",
            "techn=\"Tech\"",
            "legendText:\"HostLegend\"",
            "legendSprite:\"server\"",
            r#"["host-tag",3]"#,
            "https://example.com/host",
            "rel_b",
            "Replies",
            "HTTPS",
            "Encrypted",
            "wire",
            "rel-tag",
            "https://example.com/rel",
            "offsetX:10",
            "offsetY:\"20px\"",
            "lineColor:\"blue\"",
            "textColor:\"red\"",
            "sourceTitle:null",
            "sourceConfig:{}",
        ],
    );
    for width in [12, 24, 40] {
        let output = borrowed(&data, bounded(width));
        if width == 12 {
            assert_eq!(output.projection, AsciiProjection::StructuredText);
            assert_eq!(output.outcome, AsciiOutputOutcome::Fallback);
            assert_eq!(output.lossiness, Lossiness::PresentationOnly);
            assert_eq!(
                output.fallback.reason,
                Some(AsciiFallbackReason::PrimaryOverflow)
            );
        } else {
            assert_primary(&output, AsciiProjection::Diagrammatic);
        }
        let allowed = borrowed(&data, AsciiViewportPolicy::with_max_width(width));
        assert_eq!(output.primary_extent, allowed.primary_extent);
        assert_eq!(topology(&allowed), topology(&natural));
        assert_measured(&output);
        assert!(output.emitted_extent.width <= width);
        assert_eq!(compact(records(&output)), compact(records(&natural)));
    }
}

#[test]
fn primary_uses_the_common_final_encoders_and_keeps_source_metadata() {
    let source = format!(
        "---\ntitle: Source title\nconfig:\n  c4:\n    wrap: true\n---\n{BASIC}accDescr: Accessible description\n"
    );
    for (mode, encoding) in [
        (AsciiColorMode::Plain, AsciiOutputEncoding::Plain),
        (AsciiColorMode::Ansi16, AsciiOutputEncoding::Ansi16),
        (AsciiColorMode::Ansi256, AsciiOutputEncoding::Ansi256),
        (AsciiColorMode::TrueColor, AsciiOutputEncoding::TrueColor),
        (AsciiColorMode::Html, AsciiOutputEncoding::Html),
    ] {
        let output = report(
            &source,
            AsciiRenderOptions::ascii().with_color_mode(mode),
            AsciiViewportPolicy::unrestricted(),
        );
        assert_eq!(output.encoding, encoding);
        assert_eq!(output.projection, AsciiProjection::Diagrammatic);
        assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
        assert!(!output.fallback.attempted);
        for value in [
            "Customer",
            "Banking System",
            "Source title",
            "Accessible description",
            "sourceConfig",
            "wrap",
        ] {
            assert!(
                output.text.contains(value),
                "missing {value}:\n{}",
                output.text
            );
        }
    }
}

#[test]
fn allow_and_error_report_the_same_natural_primary_extent_as_plain_fallback() {
    let parsed = parse(BASIC);
    let natural = report(
        BASIC,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    let width = 20;
    assert!(natural.emitted_extent.width > width);
    let fallback = report(BASIC, AsciiRenderOptions::ascii(), bounded(width));
    assert_eq!(fallback.projection, AsciiProjection::StructuredText);
    let allowed = report(
        BASIC,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Allow),
    );
    assert_eq!(fallback.primary_extent, allowed.primary_extent);
    assert_eq!(topology(&allowed), topology(&natural));
    assert_eq!(allowed.outcome, AsciiOutputOutcome::WideAllowed);
    assert!(allowed.emitted_overflowed);
    let error = report_with(
        &parsed,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Error),
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, AsciiError::WidthOverflow { actual_width, .. } if actual_width == allowed.emitted_extent.width)
    );
}

#[test]
fn resource_limits_and_cancellation_are_terminal_instead_of_selecting_records() {
    let parsed = parse(NESTED);
    for limit in [
        AsciiResourceLimitId::MaxGridCells,
        AsciiResourceLimitId::MaxOutputBytes,
        AsciiResourceLimitId::MaxLayoutWorkUnits,
        AsciiResourceLimitId::MaxNestingDepth,
    ] {
        let result = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            bounded(20),
            &OperationControl::new(),
            AsciiResourcePolicy::default().with_limit(limit, 1).unwrap(),
        );
        assert!(
            matches!(&result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == limit),
            "{limit:?}: {result:?}"
        );
    }
    for checkpoints in [0, 16] {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(checkpoints);
        let result = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            bounded(20),
            &control,
            AsciiResourcePolicy::default(),
        );
        assert!(
            matches!(result, Err(AsciiError::Cancelled(_))),
            "{result:?}"
        );
    }
}

#[test]
fn final_output_bytes_alone_do_not_admit_retained_overview_and_serialization() {
    let parsed = parse(BASIC);
    let natural = report(
        BASIC,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    for maximum in [natural.text.len(), natural.text.len() - 1] {
        let result = report_with(
            &parsed,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, maximum)
                .unwrap(),
        );
        assert!(
            matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxOutputBytes)
        );
    }
}
