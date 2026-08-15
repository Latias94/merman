use tree_sitter::{Language, Node, Parser, Tree};

use tree_sitter_mermaid::LANGUAGE;

fn parse(source: &str) -> Tree {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
        .parse(source, None)
        .expect("parse must return a tree")
}

fn count_kind(node: Node<'_>, expected: &str) -> usize {
    let mut count = usize::from(node.kind() == expected);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_kind(child, expected);
    }
    count
}

fn assert_structured(source: &str, root: &str, required: &[&str]) -> Tree {
    let tree = parse(source);
    assert!(
        !tree.root_node().has_error(),
        "unexpected error for {source:?}: {}",
        tree.root_node().to_sexp()
    );
    assert_eq!(count_kind(tree.root_node(), root), 1, "{source:?}");
    for kind in required {
        assert!(
            count_kind(tree.root_node(), kind) > 0,
            "missing {kind} for {source:?}: {}",
            tree.root_node().to_sexp()
        );
    }
    tree
}

#[test]
fn declarative_families_expose_family_owned_structures() {
    assert_structured(
        "gantt\nsection Build\nShip parser:done, release, 2026-08-15, 1d\n",
        "gantt_diagram",
        &["gantt_section_statement", "gantt_task_statement"],
    );
    assert_structured(
        "ishikawa-beta\n    Quality\n        Process\n            Review\n",
        "ishikawa_diagram",
        &["ishikawa_effect_statement", "ishikawa_cause_statement"],
    );
    assert_structured(
        "journey\nsection Build\nImplement parser: 5: Alice\n",
        "journey_diagram",
        &["journey_section_statement", "journey_task_statement"],
    );
    assert_structured(
        "quadrantChart\nquadrant-1 Invest\nParser: [0.3, 0.7]\n",
        "quadrant_chart_diagram",
        &[
            "quadrant_chart_quadrant_statement",
            "quadrant_chart_point_statement",
        ],
    );
    assert_structured(
        "requirementDiagram\nrequirement Parser {\n  id: REQ-1\n}\nelement Runtime {\n  type: service\n}\nRuntime - satisfies -> Parser\n",
        "requirement_diagram",
        &[
            "requirement_declaration",
            "requirement_element_declaration",
            "requirement_relationship_statement",
        ],
    );
    assert_structured(
        "timeline TD\n    title Delivery plan\n    section Delivery\n        2026 : Parser release\n",
        "timeline_diagram",
        &[
            "timeline_title_statement",
            "timeline_section_statement",
            "timeline_period_statement",
        ],
    );
    assert_structured(
        "xychart-beta horizontal\nx-axis [Jan, Feb]\ny-axis 0 --> 10\nline [1, 2]\n",
        "xy_chart_diagram",
        &[
            "xy_chart_x_axis_statement",
            "xy_chart_y_axis_statement",
            "xy_chart_line_statement",
        ],
    );
}

#[test]
fn line_local_recovery_preserves_following_declarative_siblings() {
    let gantt = assert_structured(
        "gantt\nBroken task:\nunknown setting payload\nsection After\nHealthy task:done, healthy, 1d\n",
        "gantt_diagram",
        &[
            "gantt_incomplete_task_statement",
            "gantt_malformed_statement",
            "gantt_task_statement",
        ],
    );
    assert_eq!(count_kind(gantt.root_node(), "gantt_task_statement"), 1);

    assert_structured(
        "journey\nBroken task:\nunknown row payload\nsection After\nHealthy task: 5: Alice\n",
        "journey_diagram",
        &[
            "journey_incomplete_task_statement",
            "journey_malformed_statement",
            "journey_task_statement",
        ],
    );
    assert_structured(
        "quadrantChart\nBroken: [1.2, 0.4]\nunknown row payload\nquadrant-2 After\nHealthy: [0.2, 0.4]\n",
        "quadrant_chart_diagram",
        &[
            "quadrant_chart_malformed_point_statement",
            "quadrant_chart_malformed_statement",
            "quadrant_chart_point_statement",
        ],
    );
    assert_structured(
        "requirementDiagram\nfunctionalRequirement MissingOpen\nBefore - traces ->\nelement After {\n  type: service\n}\nBefore - satisfies -> After\n",
        "requirement_diagram",
        &[
            "requirement_incomplete_declaration_statement",
            "requirement_incomplete_relationship_statement",
            "requirement_relationship_statement",
        ],
    );
    assert_structured(
        "timeline\n2025 :\n:broken event\nsection After\n2026 : Healthy event\n",
        "timeline_diagram",
        &[
            "timeline_incomplete_event",
            "timeline_malformed_event_statement",
            "timeline_period_statement",
        ],
    );
    assert_structured(
        "xychart\nline \"unterminated [1, 2\nbar [3, 4]\nunknown ???\ntitle \"after\"\n",
        "xy_chart_diagram",
        &[
            "xy_chart_malformed_series_statement",
            "xy_chart_malformed_statement",
            "xy_chart_title_statement",
        ],
    );
}

#[test]
fn invalid_xy_chart_header_suffix_is_not_a_clean_family_root() {
    let tree = parse("xychart-1\n");
    assert!(
        tree.root_node().has_error(),
        "{}",
        tree.root_node().to_sexp()
    );
}

#[test]
fn timeline_ignores_indented_blank_lines() {
    assert_structured(
        "timeline\n  2000 : Start\n    \n  2001 : End\n",
        "timeline_diagram",
        &["timeline_period_statement"],
    );
}
