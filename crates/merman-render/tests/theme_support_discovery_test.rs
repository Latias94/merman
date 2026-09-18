use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{ThemeSupportStateV1, ThemeTarget, describe_theme_support};
use merman_theme_contract::{
    ThemeRuleFacetV1, ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1,
    ThemeSupportOutputV1, ThemeSupportQueryV1,
};

#[test]
fn unknown_catalog_ids_remain_visible_and_resolve_to_unverified() {
    let query: ThemeSupportQueryV1 = serde_json::from_str(
        r#"{
            "schema_version": 1,
            "family": "future-family",
            "output": "standalone-svg",
            "subject": {"kind": "rule", "target": "future-target", "facet": "future-facet"}
        }"#,
    )
    .expect("unknown catalog ids remain valid discovery input");

    let support = describe_theme_support(&query);

    assert_eq!(support.claim_revision(), 1);
    assert_eq!(support.query(), &query);
    assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
    assert_eq!(support.reason_ids(), ["theme-support.unknown-family"]);
}

#[test]
fn known_target_outside_the_family_semantics_is_not_applicable() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::ER.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Requirement.id(),
        ThemeRuleFacetV1::Fill,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::NotApplicable);
    assert_eq!(
        support.reason_ids(),
        ["theme-support.target-not-applicable-to-family"]
    );
}

#[test]
fn unknown_facet_stays_unverified_even_when_the_known_target_is_not_applicable() {
    let query: ThemeSupportQueryV1 = serde_json::from_str(
        r#"{
            "schema_version": 1,
            "family": "er",
            "output": "standalone-svg",
            "subject": {"kind": "rule", "target": "requirement", "facet": "future-facet"}
        }"#,
    )
    .expect("unknown facet remains valid discovery input");

    let support = describe_theme_support(&query);

    assert_eq!(support.query(), &query);
    assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
    assert_eq!(support.reason_ids(), ["theme-support.unknown-facet"]);
}

#[test]
fn family_owned_partial_route_is_reported_as_conditional() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeTarget::Node.id(),
            ThemeRuleFacetV1::Radius,
        );
        let support = describe_theme_support(&query);
        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ]
        );
    }
}

#[test]
fn direct_only_family_slices_are_reported_as_conditional() {
    for (family, target, facet) in [
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeTarget::Relation,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeTarget::Relation,
            ThemeRuleFacetV1::StrokePaint,
        ),
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::KANBAN,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::JOURNEY,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::TIMELINE,
            ThemeTarget::TimelineEvent,
            ThemeRuleFacetV1::Opacity,
        ),
        (
            DiagramFamilyId::TIMELINE,
            ThemeTarget::TimelineEvent,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::TREE_VIEW,
            ThemeTarget::Edge,
            ThemeRuleFacetV1::StrokeWidth,
        ),
        (
            DiagramFamilyId::JOURNEY,
            ThemeTarget::JourneyTask,
            ThemeRuleFacetV1::Radius,
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::Axis,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::Axis,
            ThemeRuleFacetV1::StrokePaint,
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::ChartSeries,
            ThemeRuleFacetV1::Radius,
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::ChartSeries,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::Title,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::C4,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::C4,
            ThemeTarget::Cluster,
            ThemeRuleFacetV1::Radius,
        ),
        (
            DiagramFamilyId::C4,
            ThemeTarget::Cluster,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::C4,
            ThemeTarget::Cluster,
            ThemeRuleFacetV1::StrokePaint,
        ),
        (
            DiagramFamilyId::GANTT,
            ThemeTarget::Task,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::GANTT,
            ThemeTarget::Title,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::GANTT,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::PIE,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::TREEMAP,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::NodeLabel,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeTarget::NodeLabel,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::XY_CHART,
            ThemeTarget::Title,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::XY_CHART,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::XY_CHART,
            ThemeTarget::Axis,
            ThemeRuleFacetV1::Fill,
        ),
        (
            DiagramFamilyId::XY_CHART,
            ThemeTarget::Axis,
            ThemeRuleFacetV1::StrokePaint,
        ),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
            "family={family} target={} facet={facet:?}",
            target.id()
        );
    }
}

#[test]
fn final_five_family_owned_paint_slices_are_reported_as_conditional() {
    for (family, target) in [
        (DiagramFamilyId::EVENT_MODELING, ThemeTarget::Text),
        (DiagramFamilyId::ISHIKAWA, ThemeTarget::Text),
        (DiagramFamilyId::VENN, ThemeTarget::Title),
        (DiagramFamilyId::VENN, ThemeTarget::Text),
        (DiagramFamilyId::ZENUML, ThemeTarget::Title),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            ThemeRuleFacetV1::Fill,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
            "family={family} target={}",
            target.id()
        );
    }
}

#[test]
fn sequence_unused_text_and_title_projection_claims_are_unsupported() {
    for target in [ThemeTarget::Text, ThemeTarget::Title] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::SEQUENCE.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            ThemeRuleFacetV1::Fill,
        );
        let support = describe_theme_support(&query);
        assert_eq!(support.state(), ThemeSupportStateV1::Unsupported);
        assert_eq!(support.claim_revision(), 1);
    }
}

#[test]
fn wardley_generic_text_paint_routes_are_unsupported() {
    for target in [ThemeTarget::Text, ThemeTarget::Title] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::WARDLEY.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            ThemeRuleFacetV1::Fill,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Unsupported);
        assert_eq!(support.reason_ids(), ["theme-support.no-supported-route"]);
    }
}

#[test]
fn generic_title_paint_routes_are_unsupported_without_a_title_color_consumer() {
    for family in [
        DiagramFamilyId::ARCHITECTURE,
        DiagramFamilyId::C4,
        DiagramFamilyId::CYNEFIN,
        DiagramFamilyId::ER,
        DiagramFamilyId::JOURNEY,
        DiagramFamilyId::KANBAN,
        DiagramFamilyId::SANKEY,
        DiagramFamilyId::TIMELINE,
    ] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeTarget::Title.id(),
            ThemeRuleFacetV1::Fill,
        );

        let support = describe_theme_support(&query);

        if family == DiagramFamilyId::ARCHITECTURE && !merman_render::layout_cytoscape_available() {
            assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
            assert_eq!(
                support.reason_ids(),
                ["theme-support.family-render-capability-not-built"]
            );
        } else {
            assert_eq!(support.state(), ThemeSupportStateV1::Unsupported);
            assert_eq!(support.reason_ids(), ["theme-support.no-supported-route"]);
        }
    }
}

#[test]
fn railroad_title_paint_is_a_conditional_typed_surface() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::RAILROAD.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Title.id(),
        ThemeRuleFacetV1::Fill,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial",
        ]
    );
}

#[test]
fn relation_and_branch_edge_stroke_slices_are_reported_as_conditional() {
    for (family, target) in [
        (DiagramFamilyId::CLASS, ThemeTarget::Edge),
        (DiagramFamilyId::ER, ThemeTarget::Relation),
        (DiagramFamilyId::MINDMAP, ThemeTarget::Edge),
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Edge),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            ThemeRuleFacetV1::StrokePaint,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
            "family={family} target={}",
            target.id()
        );
    }
}

#[test]
fn final_four_terminal_less_paint_claims_are_unsupported() {
    for (family, target) in [
        (DiagramFamilyId::EVENT_MODELING, ThemeTarget::Title),
        (DiagramFamilyId::ISHIKAWA, ThemeTarget::Title),
        (DiagramFamilyId::ZENUML, ThemeTarget::Text),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            ThemeRuleFacetV1::Fill,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Unsupported);
        assert_eq!(support.reason_ids(), ["theme-support.no-supported-route"]);
    }
}

#[test]
fn class_terminal_less_paint_claims_are_unsupported() {
    for (target, facet) in [
        (ThemeTarget::Marker, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Marker, ThemeRuleFacetV1::StrokePaint),
        (ThemeTarget::ClusterLabel, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Table, ThemeRuleFacetV1::Fill),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::CLASS.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(
            support.state(),
            ThemeSupportStateV1::Unsupported,
            "target={} facet={facet:?}",
            target.id()
        );
        assert_eq!(
            support.reason_ids(),
            ["theme-support.no-supported-route"],
            "target={} facet={facet:?}",
            target.id()
        );
    }
}

#[test]
fn class_namespace_title_reports_a_conditional_typed_surface() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::CLASS.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Title.id(),
        ThemeRuleFacetV1::Fill,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial",
        ]
    );
}

#[test]
fn state_support_discovery_uses_the_family_consumer_instead_of_route_ownership() {
    for (target, facet, expected_state, expected_reasons) in [
        (
            ThemeTarget::Canvas,
            ThemeRuleFacetV1::Fill,
            ThemeSupportStateV1::Unverified,
            &["theme-support.root-support-query-not-yet-modeled"][..],
        ),
        (
            ThemeTarget::Title,
            ThemeRuleFacetV1::Radius,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            ThemeTarget::Title,
            ThemeRuleFacetV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ][..],
        ),
        (
            ThemeTarget::State,
            ThemeRuleFacetV1::Radius,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ][..],
        ),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::STATE.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), expected_state, "target={}", target.id());
        assert_eq!(
            support.reason_ids(),
            expected_reasons,
            "target={}",
            target.id()
        );
    }
}

#[test]
fn family_owned_ordinal_palettes_are_reported_without_exposing_private_routes() {
    for (family, target) in [
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Node),
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Node),
        (DiagramFamilyId::PIE, ThemeTarget::PieSlice),
        (DiagramFamilyId::XY_CHART, ThemeTarget::ChartSeries),
        (DiagramFamilyId::RADAR, ThemeTarget::ChartSeries),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            ThemeSupportFacetV1::OrdinalPalette,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ]
        );
    }
}

#[test]
fn compatibility_only_route_is_conditional_without_claiming_direct_support() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::SEQUENCE.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Message.id(),
        ThemeRuleFacetV1::Fill,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial",
        ]
    );
}

#[test]
fn mindmap_terminal_less_paint_claims_are_unsupported() {
    for (target, facet) in [
        (ThemeTarget::NodeLabel, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Text, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Title, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Marker, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Marker, ThemeRuleFacetV1::StrokePaint),
        (ThemeTarget::EdgeLabelBackground, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Cluster, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Cluster, ThemeRuleFacetV1::StrokePaint),
        (ThemeTarget::ClusterLabel, ThemeRuleFacetV1::Fill),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::MINDMAP.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(
            support.state(),
            ThemeSupportStateV1::Unsupported,
            "target={} facet={facet:?}",
            target.id()
        );
        assert_eq!(
            support.reason_ids(),
            ["theme-support.no-supported-route"],
            "target={} facet={facet:?}",
            target.id()
        );
    }
}

#[test]
fn mindmap_node_paint_is_reported_as_a_family_owned_partial_surface() {
    for facet in [ThemeRuleFacetV1::Fill, ThemeRuleFacetV1::StrokePaint] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::MINDMAP.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeTarget::Node.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
            "facet={facet:?}"
        );
    }
}

#[test]
fn tree_view_marker_paint_is_reported_as_a_family_owned_partial_surface() {
    for facet in [ThemeRuleFacetV1::Fill, ThemeRuleFacetV1::StrokePaint] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::TREE_VIEW.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeTarget::Marker.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
            "facet={facet:?}"
        );
    }
}

#[test]
fn tree_view_terminal_less_paint_claims_are_unsupported() {
    for (target, facet) in [
        (ThemeTarget::Node, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Node, ThemeRuleFacetV1::StrokePaint),
        (ThemeTarget::Title, ThemeRuleFacetV1::Fill),
        (ThemeTarget::EdgeLabelBackground, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Cluster, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Cluster, ThemeRuleFacetV1::StrokePaint),
        (ThemeTarget::ClusterLabel, ThemeRuleFacetV1::Fill),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::TREE_VIEW.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(
            support.state(),
            ThemeSupportStateV1::Unsupported,
            "target={} facet={facet:?}",
            target.id()
        );
        assert_eq!(
            support.reason_ids(),
            ["theme-support.no-supported-route"],
            "target={} facet={facet:?}",
            target.id()
        );
    }
}

#[test]
fn gitgraph_node_paint_edge_and_label_fill_are_typed_partial_surfaces() {
    for (target, facet) in [
        (ThemeTarget::Node, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Node, ThemeRuleFacetV1::StrokePaint),
        (ThemeTarget::Text, ThemeRuleFacetV1::Fill),
        (ThemeTarget::NodeLabel, ThemeRuleFacetV1::Fill),
        (ThemeTarget::EdgeLabel, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Edge, ThemeRuleFacetV1::Fill),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::GIT_GRAPH.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ]
        );
    }
}

#[test]
fn gitgraph_edge_label_background_fill_is_a_typed_partial_surface() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::GIT_GRAPH.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::EdgeLabelBackground.id(),
        ThemeRuleFacetV1::Fill,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial",
        ]
    );
}

#[test]
fn gitgraph_terminal_less_paint_claims_are_unsupported() {
    for (target, facet) in [
        (ThemeTarget::Title, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Marker, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Marker, ThemeRuleFacetV1::StrokePaint),
        (ThemeTarget::Cluster, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Cluster, ThemeRuleFacetV1::StrokePaint),
        (ThemeTarget::ClusterLabel, ThemeRuleFacetV1::Fill),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::GIT_GRAPH.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        assert_eq!(
            support.state(),
            ThemeSupportStateV1::Unsupported,
            "target={} facet={facet:?}",
            target.id()
        );
        assert_eq!(
            support.reason_ids(),
            ["theme-support.no-supported-route"],
            "target={} facet={facet:?}",
            target.id()
        );
    }
}

#[test]
fn fully_rejected_known_domain_is_unsupported() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::SEQUENCE.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Loop.id(),
        ThemeRuleFacetV1::Radius,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::Unsupported);
    assert_eq!(support.reason_ids(), ["theme-support.no-supported-route"]);
}

#[test]
fn browser_svg_stays_unverified_until_terminal_qualification_exists() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::FLOWCHART.as_str(),
        ThemeSupportOutputV1::BrowserSvg,
        ThemeTarget::Node.id(),
        ThemeRuleFacetV1::Fill,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
    assert_eq!(
        support.reason_ids(),
        ["theme-support.terminal-qualification-incomplete"]
    );
}

#[test]
fn native_exports_stay_unverified_until_their_target_owner_qualifies_them() {
    for output in [
        ThemeSupportOutputV1::Png,
        ThemeSupportOutputV1::Jpeg,
        ThemeSupportOutputV1::Pdf,
    ] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::FLOWCHART.as_str(),
            output,
            ThemeTarget::Node.id(),
            ThemeRuleFacetV1::Fill,
        );

        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
        assert_eq!(
            support.reason_ids(),
            ["theme-support.output-qualification-not-owned-by-renderer"]
        );
    }
}

#[test]
fn support_discovery_respects_required_family_render_capabilities() {
    for (target, facet) in [
        (ThemeTarget::Cluster, ThemeRuleFacetV1::Fill),
        (ThemeTarget::Edge, ThemeRuleFacetV1::StrokePaint),
    ] {
        let query = ThemeSupportQueryV1::for_target(
            DiagramFamilyId::ARCHITECTURE.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            target.id(),
            facet,
        );

        let support = describe_theme_support(&query);

        if merman_render::layout_cytoscape_available() {
            assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
            assert_eq!(
                support.reason_ids(),
                [
                    "theme-support.family-owned-consumer-present",
                    "theme-support.public-value-domain-partial",
                ]
            );
        } else {
            assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
            assert_eq!(
                support.reason_ids(),
                ["theme-support.family-render-capability-not-built"]
            );
        }
    }
}

#[test]
fn ascii_has_a_separate_non_visual_theme_contract() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::FLOWCHART.as_str(),
        ThemeSupportOutputV1::Ascii,
        ThemeTarget::Node.id(),
        ThemeRuleFacetV1::Fill,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::NotApplicable);
    assert_eq!(
        support.reason_ids(),
        ["theme-support.visual-theme-not-applicable-to-output"]
    );
}

#[test]
fn subject_base_typography_reports_the_family_wide_matrix_instead_of_a_text_rule() {
    for (family, property, expected_state, expected_reasons) in [
        (
            DiagramFamilyId::FLOWCHART,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::PACKET,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::PACKET,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::PIE,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::PIE,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::RAILROAD,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::RAILROAD,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::INFO,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::INFO,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::ERROR,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ERROR,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::CYNEFIN,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::CYNEFIN,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::WARDLEY,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::WARDLEY,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::EVENT_MODELING,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::EVENT_MODELING,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ISHIKAWA,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ISHIKAWA,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::RADAR,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::RADAR,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::CLASS,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::CLASS,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ER,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ER,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::JOURNEY,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::JOURNEY,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::KANBAN,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::KANBAN,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::TIMELINE,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::TIMELINE,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::VENN,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::VENN,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::SANKEY,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SANKEY,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::XY_CHART,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::XY_CHART,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::TREEMAP,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::TREEMAP,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
    ] {
        let query = ThemeSupportQueryV1::base_typography(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            property,
        );
        let support = describe_theme_support(&query);

        assert_eq!(
            support.state(),
            expected_state,
            "family={family} property={property:?}"
        );
        assert_eq!(
            support.reason_ids(),
            expected_reasons,
            "family={family} property={property:?}"
        );
    }
}

#[cfg(feature = "layout-cytoscape")]
#[test]
fn subject_architecture_base_typography_reports_the_typed_surface_when_built() {
    for property in [
        ThemeSupportBaseTypographyPropertyV1::FontStack,
        ThemeSupportBaseTypographyPropertyV1::FontSize,
    ] {
        let support = describe_theme_support(&ThemeSupportQueryV1::base_typography(
            DiagramFamilyId::ARCHITECTURE.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            property,
        ));

        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ],
            "property={property:?}"
        );
    }
}

#[test]
fn subject_state_base_typography_reports_each_runtime_property_domain() {
    for property in ThemeSupportBaseTypographyPropertyV1::ALL {
        let support = describe_theme_support(&ThemeSupportQueryV1::base_typography(
            DiagramFamilyId::STATE.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            *property,
        ));
        let supported = matches!(
            property,
            ThemeSupportBaseTypographyPropertyV1::FontStack
                | ThemeSupportBaseTypographyPropertyV1::FontSize
                | ThemeSupportBaseTypographyPropertyV1::FontWeight
                | ThemeSupportBaseTypographyPropertyV1::FontStyle
                | ThemeSupportBaseTypographyPropertyV1::LetterSpacing
                | ThemeSupportBaseTypographyPropertyV1::WordSpacing
                | ThemeSupportBaseTypographyPropertyV1::Transform
        );

        if supported {
            assert_eq!(
                support.state(),
                ThemeSupportStateV1::Conditional,
                "property={property:?}"
            );
            assert_eq!(
                support.reason_ids(),
                [
                    "theme-support.family-owned-consumer-present",
                    "theme-support.document-surface-dependent",
                ],
                "property={property:?}"
            );
        } else {
            assert_eq!(
                support.state(),
                ThemeSupportStateV1::Unsupported,
                "property={property:?}"
            );
            assert_eq!(
                support.reason_ids(),
                ["theme-support.no-supported-route"],
                "property={property:?}"
            );
        }
    }
}

#[test]
fn base_typography_is_distinct_from_text_font_stack_rule() {
    let v1 = describe_theme_support(&ThemeSupportQueryV1::for_target(
        DiagramFamilyId::INFO.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Text.id(),
        ThemeRuleFacetV1::FontStack,
    ));
    let v2 = describe_theme_support(&ThemeSupportQueryV1::base_typography(
        DiagramFamilyId::INFO.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeSupportBaseTypographyPropertyV1::FontStack,
    ));

    assert_eq!(v1.state(), ThemeSupportStateV1::Unsupported);
    assert_eq!(v2.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        v2.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.document-surface-dependent",
        ]
    );
}

#[test]
fn subject_unknown_subject_and_base_typography_property_are_unverified() {
    for (query, reason_id) in [
        (
            serde_json::json!({
                "schema_version": 1,
                "family": "railroad",
                "output": "standalone-svg",
                "subject": { "kind": "future-subject" }
            }),
            "theme-support.unknown-subject",
        ),
        (
            serde_json::json!({
                "schema_version": 1,
                "family": "railroad",
                "output": "standalone-svg",
                "subject": {
                    "kind": "base-typography",
                    "property": "future-property"
                }
            }),
            "theme-support.unknown-base-typography-property",
        ),
    ] {
        let query: ThemeSupportQueryV1 =
            serde_json::from_value(query).expect("unknown V2 identifiers remain queryable");
        let support = describe_theme_support(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
        assert_eq!(support.reason_ids(), [reason_id]);
    }
}

#[test]
fn subjects_preserve_output_qualification_gates() {
    for (output, expected_state, expected_reason) in [
        (
            ThemeSupportOutputV1::BrowserSvg,
            ThemeSupportStateV1::Unverified,
            "theme-support.terminal-qualification-incomplete",
        ),
        (
            ThemeSupportOutputV1::Png,
            ThemeSupportStateV1::Unverified,
            "theme-support.output-qualification-not-owned-by-renderer",
        ),
        (
            ThemeSupportOutputV1::Jpeg,
            ThemeSupportStateV1::Unverified,
            "theme-support.output-qualification-not-owned-by-renderer",
        ),
        (
            ThemeSupportOutputV1::Pdf,
            ThemeSupportStateV1::Unverified,
            "theme-support.output-qualification-not-owned-by-renderer",
        ),
        (
            ThemeSupportOutputV1::Ascii,
            ThemeSupportStateV1::NotApplicable,
            "theme-support.visual-theme-not-applicable-to-output",
        ),
    ] {
        let support = describe_theme_support(&ThemeSupportQueryV1::base_typography(
            DiagramFamilyId::RAILROAD.as_str(),
            output,
            ThemeSupportBaseTypographyPropertyV1::FontStack,
        ));

        assert_eq!(support.state(), expected_state, "output={output:?}");
        assert_eq!(support.reason_ids(), [expected_reason], "output={output:?}");
    }
}

#[test]
fn subject_preserves_target_and_family_capability_gates() {
    let not_applicable = describe_theme_support(&ThemeSupportQueryV1::rule(
        DiagramFamilyId::ER.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Requirement.id(),
        ThemeRuleFacetV1::Fill,
    ));
    assert_eq!(not_applicable.state(), ThemeSupportStateV1::NotApplicable);
    assert_eq!(
        not_applicable.reason_ids(),
        ["theme-support.target-not-applicable-to-family"]
    );

    let root_query = ThemeSupportQueryV1::rule(
        DiagramFamilyId::FLOWCHART.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Canvas.id(),
        ThemeRuleFacetV1::Fill,
    );
    let root_support = describe_theme_support(&root_query);
    assert_eq!(root_support.state(), ThemeSupportStateV1::Unverified);
    assert_eq!(
        root_support.reason_ids(),
        ["theme-support.root-support-query-not-yet-modeled"]
    );

    let architecture = describe_theme_support(&ThemeSupportQueryV1::rule(
        DiagramFamilyId::ARCHITECTURE.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Cluster.id(),
        ThemeRuleFacetV1::Fill,
    ));
    if merman_render::layout_cytoscape_available() {
        assert_eq!(architecture.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            architecture.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ]
        );
    } else {
        assert_eq!(architecture.state(), ThemeSupportStateV1::Unverified);
        assert_eq!(
            architecture.reason_ids(),
            ["theme-support.family-render-capability-not-built"]
        );
    }
}

#[test]
fn er_table_fill_reports_a_partial_typed_consumer() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::ER.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Table.id(),
        ThemeRuleFacetV1::Fill,
    );
    let support = describe_theme_support(&query);
    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial",
        ]
    );
}

#[test]
fn er_relation_fill_reports_the_direct_stroke_fallback_consumer() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::ER.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Relation.id(),
        ThemeRuleFacetV1::Fill,
    );
    let support = describe_theme_support(&query);
    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial",
        ]
    );
}

#[test]
fn flowchart_and_swimlane_background_fill_is_a_typed_partial_surface() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeTarget::EdgeLabelBackground.id(),
            ThemeRuleFacetV1::Fill,
        );
        let support = describe_theme_support(&query);
        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ]
        );
    }
}

#[test]
fn flowchart_and_swimlane_cluster_label_fill_is_a_typed_partial_surface() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeTarget::ClusterLabel.id(),
            ThemeRuleFacetV1::Fill,
        );
        let support = describe_theme_support(&query);
        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ]
        );
    }
}

#[test]
fn flowchart_and_swimlane_edge_fill_is_a_typed_partial_surface() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        let query = ThemeSupportQueryV1::for_target(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeTarget::Edge.id(),
            ThemeRuleFacetV1::Fill,
        );
        let support = describe_theme_support(&query);
        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ]
        );
    }
}

#[test]
fn flowchart_and_swimlane_unused_marker_projection_claims_are_unsupported() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for facet in [ThemeRuleFacetV1::Fill, ThemeRuleFacetV1::StrokePaint] {
            let query = ThemeSupportQueryV1::for_target(
                family.as_str(),
                ThemeSupportOutputV1::StandaloneSvg,
                ThemeTarget::Marker.id(),
                facet,
            );
            let support = describe_theme_support(&query);
            assert_eq!(support.state(), ThemeSupportStateV1::Unsupported);
            assert_eq!(support.reason_ids(), ["theme-support.no-supported-route"]);
        }
    }
}

#[test]
fn block_title_discovery_reports_unsupported_after_projection_retirement() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::BLOCK.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Title.id(),
        ThemeRuleFacetV1::Fill,
    );
    let support = describe_theme_support(&query);
    assert_eq!(support.state(), ThemeSupportStateV1::Unsupported);
    assert_eq!(support.reason_ids(), ["theme-support.no-supported-route"]);
}

#[test]
fn block_cluster_label_discovery_reports_unsupported_after_projection_retirement() {
    let query = ThemeSupportQueryV1::for_target(
        DiagramFamilyId::BLOCK.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::ClusterLabel.id(),
        ThemeRuleFacetV1::Fill,
    );
    let support = describe_theme_support(&query);
    assert_eq!(support.state(), ThemeSupportStateV1::Unsupported);
    assert_eq!(support.reason_ids(), ["theme-support.no-supported-route"]);
}

#[test]
fn block_background_discovery_reports_a_partial_typed_consumer() {
    let query = ThemeSupportQueryV1::for_target(
        "block",
        ThemeSupportOutputV1::StandaloneSvg,
        "edge-label-background",
        ThemeRuleFacetV1::Fill,
    );
    let support = describe_theme_support(&query);
    assert_eq!(support.claim_revision(), 1);
    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial"
        ]
    );
}

#[test]
fn block_edge_discovery_reports_partial_typed_fill_and_stroke() {
    for facet in [ThemeRuleFacetV1::Fill, ThemeRuleFacetV1::StrokePaint] {
        let query = ThemeSupportQueryV1::for_target(
            "block",
            ThemeSupportOutputV1::StandaloneSvg,
            "edge",
            facet,
        );
        let support = describe_theme_support(&query);
        assert_eq!(support.claim_revision(), 1);
        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial"
            ]
        );
    }
}

#[test]
fn block_cluster_discovery_reports_partial_typed_fill_and_stroke() {
    for facet in [ThemeRuleFacetV1::Fill, ThemeRuleFacetV1::StrokePaint] {
        let query = ThemeSupportQueryV1::for_target(
            "block",
            ThemeSupportOutputV1::StandaloneSvg,
            "cluster",
            facet,
        );
        let support = describe_theme_support(&query);
        assert_eq!(support.claim_revision(), 1);
        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial"
            ]
        );
    }
}

#[test]
fn block_marker_discovery_reports_partial_typed_fill_and_stroke() {
    for facet in [ThemeRuleFacetV1::Fill, ThemeRuleFacetV1::StrokePaint] {
        let query = ThemeSupportQueryV1::for_target(
            "block",
            ThemeSupportOutputV1::StandaloneSvg,
            "marker",
            facet,
        );
        let support = describe_theme_support(&query);
        assert_eq!(support.claim_revision(), 1);
        assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
        assert_eq!(
            support.reason_ids(),
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial"
            ]
        );
    }
}

#[test]
fn block_generic_text_discovery_reports_partial_typed_fill() {
    let support = describe_theme_support(&ThemeSupportQueryV1::for_target(
        "block",
        ThemeSupportOutputV1::StandaloneSvg,
        "text",
        ThemeRuleFacetV1::Fill,
    ));
    assert_eq!(support.claim_revision(), 1);
    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial",
        ]
    );
}

#[test]
fn flowchart_label_weight_discovery_is_conditional() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for target in [ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel] {
            let query = ThemeSupportQueryV1::for_target(
                family.as_str(),
                ThemeSupportOutputV1::StandaloneSvg,
                target.id(),
                ThemeRuleFacetV1::FontWeight,
            );
            assert_eq!(
                describe_theme_support(&query).state(),
                ThemeSupportStateV1::Conditional
            );
        }
    }
}
