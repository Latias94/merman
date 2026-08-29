use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{
    ThemeSupportStateV1, ThemeTarget, describe_theme_support, describe_theme_support_v2,
};
use merman_theme_contract::{
    ThemeRuleFacetV1, ThemeSupportBaseTypographyPropertyV2, ThemeSupportFacetV1,
    ThemeSupportOutputV1, ThemeSupportQueryV1, ThemeSupportQueryV2,
};

#[test]
fn unknown_catalog_ids_remain_visible_and_resolve_to_unverified() {
    let query: ThemeSupportQueryV1 = serde_json::from_str(
        r#"{
            "schema_version": 1,
            "family": "future-family",
            "output": "standalone-svg",
            "target": "future-target",
            "facet": "future-facet"
        }"#,
    )
    .expect("unknown catalog ids remain valid discovery input");

    let support = describe_theme_support(&query);

    assert_eq!(support.claim_revision(), 16);
    assert_eq!(support.query(), &query);
    assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
    assert_eq!(support.reason_ids(), ["theme-support.unknown-family"]);
}

#[test]
fn known_target_outside_the_family_semantics_is_not_applicable() {
    let query = ThemeSupportQueryV1::known(
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
            "target": "requirement",
            "facet": "future-facet"
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
    let query = ThemeSupportQueryV1::known(
        DiagramFamilyId::FLOWCHART.as_str(),
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

#[test]
fn direct_only_family_slices_are_reported_as_conditional() {
    for (family, target, facet) in [
        (
            DiagramFamilyId::TIMELINE,
            ThemeTarget::TimelineEvent,
            ThemeRuleFacetV1::Opacity,
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
            ThemeTarget::ChartSeries,
            ThemeRuleFacetV1::Radius,
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::ChartSeries,
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
            DiagramFamilyId::PIE,
            ThemeTarget::Text,
            ThemeRuleFacetV1::Fill,
        ),
    ] {
        let query = ThemeSupportQueryV1::known(
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
fn final_four_family_owned_paint_slices_are_reported_as_conditional() {
    for (family, target) in [
        (DiagramFamilyId::EVENT_MODELING, ThemeTarget::Text),
        (DiagramFamilyId::ISHIKAWA, ThemeTarget::Text),
        (DiagramFamilyId::VENN, ThemeTarget::Title),
        (DiagramFamilyId::ZENUML, ThemeTarget::Title),
    ] {
        let query = ThemeSupportQueryV1::known(
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
fn relation_and_branch_edge_stroke_slices_are_reported_as_conditional() {
    for (family, target) in [
        (DiagramFamilyId::CLASS, ThemeTarget::Edge),
        (DiagramFamilyId::ER, ThemeTarget::Relation),
        (DiagramFamilyId::MINDMAP, ThemeTarget::Edge),
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Edge),
    ] {
        let query = ThemeSupportQueryV1::known(
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
        let query = ThemeSupportQueryV1::known(
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
        let query = ThemeSupportQueryV1::known(
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
fn class_namespace_title_remains_a_conditional_compatibility_surface() {
    let query = ThemeSupportQueryV1::known(
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
            "theme-support.legacy-compatibility-only",
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
        let query = ThemeSupportQueryV1::known(
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
        let query = ThemeSupportQueryV1::known(
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
    let query = ThemeSupportQueryV1::known(
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
            "theme-support.legacy-compatibility-only",
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
        let query = ThemeSupportQueryV1::known(
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
        let query = ThemeSupportQueryV1::known(
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
fn gitgraph_text_fill_remains_an_explicit_legacy_compatibility_route() {
    let query = ThemeSupportQueryV1::known(
        DiagramFamilyId::GIT_GRAPH.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Text.id(),
        ThemeRuleFacetV1::Fill,
    );

    let support = describe_theme_support(&query);

    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(
        support.reason_ids(),
        [
            "theme-support.legacy-compatibility-only",
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
        let query = ThemeSupportQueryV1::known(
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
    let query = ThemeSupportQueryV1::known(
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
    let query = ThemeSupportQueryV1::known(
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
        let query = ThemeSupportQueryV1::known(
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
        let query = ThemeSupportQueryV1::known(
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
    let query = ThemeSupportQueryV1::known(
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
fn v2_base_typography_reports_the_family_wide_matrix_instead_of_a_text_rule() {
    for (family, property, expected_state, expected_reasons) in [
        (
            DiagramFamilyId::FLOWCHART,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::PACKET,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::PACKET,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::PIE,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::PIE,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.legacy-compatibility-only",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::RAILROAD,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::RAILROAD,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::INFO,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::INFO,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::ERROR,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ERROR,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::CYNEFIN,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::CYNEFIN,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::WARDLEY,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::WARDLEY,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::EVENT_MODELING,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::EVENT_MODELING,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ISHIKAWA,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ISHIKAWA,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::RADAR,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::RADAR,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::CLASS,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::CLASS,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.legacy-compatibility-only",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ER,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::ER,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.legacy-compatibility-only",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::KANBAN,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::KANBAN,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.legacy-compatibility-only",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::VENN,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::VENN,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
        (
            DiagramFamilyId::SANKEY,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
            ThemeSupportStateV1::Conditional,
            &[
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ][..],
        ),
        (
            DiagramFamilyId::SANKEY,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
            ThemeSupportStateV1::Unsupported,
            &["theme-support.no-supported-route"][..],
        ),
    ] {
        let query = ThemeSupportQueryV2::base_typography(
            family.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            property,
        );
        let support = describe_theme_support_v2(&query);

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

#[test]
fn v2_state_base_typography_reports_each_runtime_property_domain() {
    for property in ThemeSupportBaseTypographyPropertyV2::ALL {
        let support = describe_theme_support_v2(&ThemeSupportQueryV2::base_typography(
            DiagramFamilyId::STATE.as_str(),
            ThemeSupportOutputV1::StandaloneSvg,
            *property,
        ));
        let supported = matches!(
            property,
            ThemeSupportBaseTypographyPropertyV2::FontStack
                | ThemeSupportBaseTypographyPropertyV2::FontSize
                | ThemeSupportBaseTypographyPropertyV2::FontWeight
                | ThemeSupportBaseTypographyPropertyV2::FontStyle
                | ThemeSupportBaseTypographyPropertyV2::LetterSpacing
                | ThemeSupportBaseTypographyPropertyV2::WordSpacing
                | ThemeSupportBaseTypographyPropertyV2::Transform
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
fn v2_rule_and_ordinal_subjects_preserve_v1_support_decisions() {
    let cases = [
        (
            ThemeSupportQueryV1::known(
                DiagramFamilyId::FLOWCHART.as_str(),
                ThemeSupportOutputV1::StandaloneSvg,
                ThemeTarget::Node.id(),
                ThemeRuleFacetV1::Radius,
            ),
            ThemeSupportQueryV2::rule(
                DiagramFamilyId::FLOWCHART.as_str(),
                ThemeSupportOutputV1::StandaloneSvg,
                ThemeTarget::Node.id(),
                ThemeRuleFacetV1::Radius,
            ),
        ),
        (
            ThemeSupportQueryV1::known(
                DiagramFamilyId::MINDMAP.as_str(),
                ThemeSupportOutputV1::StandaloneSvg,
                ThemeTarget::Node.id(),
                ThemeSupportFacetV1::OrdinalPalette,
            ),
            ThemeSupportQueryV2::ordinal_palette(
                DiagramFamilyId::MINDMAP.as_str(),
                ThemeSupportOutputV1::StandaloneSvg,
                ThemeTarget::Node.id(),
            ),
        ),
    ];

    for (v1_query, v2_query) in cases {
        let v1 = describe_theme_support(&v1_query);
        let v2 = describe_theme_support_v2(&v2_query);

        assert_eq!(v2.state(), v1.state());
        assert_eq!(v2.reason_ids(), v1.reason_ids());
    }
}

#[test]
fn v2_base_typography_is_not_the_v1_text_font_stack_rule() {
    let v1 = describe_theme_support(&ThemeSupportQueryV1::known(
        DiagramFamilyId::INFO.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Text.id(),
        ThemeRuleFacetV1::FontStack,
    ));
    let v2 = describe_theme_support_v2(&ThemeSupportQueryV2::base_typography(
        DiagramFamilyId::INFO.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeSupportBaseTypographyPropertyV2::FontStack,
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
fn v2_unknown_subject_and_base_typography_property_are_unverified() {
    for (query, reason_id) in [
        (
            serde_json::json!({
                "schema_version": 2,
                "family": "railroad",
                "output": "standalone-svg",
                "subject": { "kind": "future-subject" }
            }),
            "theme-support.unknown-subject",
        ),
        (
            serde_json::json!({
                "schema_version": 2,
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
        let query: ThemeSupportQueryV2 =
            serde_json::from_value(query).expect("unknown V2 identifiers remain queryable");
        let support = describe_theme_support_v2(&query);

        assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
        assert_eq!(support.reason_ids(), [reason_id]);
    }
}

#[test]
fn v2_preserves_the_v1_output_qualification_gates() {
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
        let support = describe_theme_support_v2(&ThemeSupportQueryV2::base_typography(
            DiagramFamilyId::RAILROAD.as_str(),
            output,
            ThemeSupportBaseTypographyPropertyV2::FontStack,
        ));

        assert_eq!(support.state(), expected_state, "output={output:?}");
        assert_eq!(support.reason_ids(), [expected_reason], "output={output:?}");
    }
}

#[test]
fn v2_preserves_target_and_family_capability_gates() {
    let not_applicable = describe_theme_support_v2(&ThemeSupportQueryV2::rule(
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

    let root_query = ThemeSupportQueryV2::rule(
        DiagramFamilyId::FLOWCHART.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Canvas.id(),
        ThemeRuleFacetV1::Fill,
    );
    let root_support = describe_theme_support_v2(&root_query);
    assert_eq!(root_support.state(), ThemeSupportStateV1::Unverified);
    assert_eq!(
        root_support.reason_ids(),
        ["theme-support.root-support-query-not-yet-modeled"]
    );

    let architecture = describe_theme_support_v2(&ThemeSupportQueryV2::rule(
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
