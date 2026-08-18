use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{ThemeSupportStateV1, ThemeTarget, describe_theme_support};
use merman_theme_contract::{
    ThemeRuleFacetV1, ThemeSupportFacetV1, ThemeSupportOutputV1, ThemeSupportQueryV1,
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
            DiagramFamilyId::C4,
            ThemeTarget::Cluster,
            ThemeRuleFacetV1::Radius,
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
fn mindmap_and_gitgraph_text_fill_remain_explicit_legacy_compatibility_routes() {
    for family in [DiagramFamilyId::MINDMAP, DiagramFamilyId::GIT_GRAPH] {
        let query = ThemeSupportQueryV1::known(
            family.as_str(),
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
            ],
            "family={family}"
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
    let query = ThemeSupportQueryV1::known(
        DiagramFamilyId::ARCHITECTURE.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Cluster.id(),
        ThemeRuleFacetV1::Fill,
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
