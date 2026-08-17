use merman_theme_contract::{
    ThemeCapabilityDescriptorV1, ThemeRuleFacetV1, ThemeSupportOutputV1, ThemeSupportQueryV1,
    ThemeSupportStateV1,
};

#[test]
fn support_query_preserves_unknown_ids_for_forward_compatible_discovery() {
    let query: ThemeSupportQueryV1 = serde_json::from_str(
        r#"{
            "schema_version": 1,
            "family": "future-family",
            "output": "browser-svg",
            "target": "future-target",
            "facet": "future-facet"
        }"#,
    )
    .expect("unknown catalog ids remain valid discovery input");

    assert_eq!(query.schema_version(), 1);
    assert_eq!(query.family_id(), "future-family");
    assert_eq!(query.output_id(), "browser-svg");
    assert_eq!(query.target_id(), "future-target");
    assert_eq!(query.facet_id(), "future-facet");

    let known = ThemeSupportQueryV1::known(
        "flowchart",
        ThemeSupportOutputV1::Png,
        "node",
        ThemeRuleFacetV1::Radius,
    );
    assert_eq!(known.output_id(), "png");
    assert_eq!(known.facet_id(), "radius");
}

#[test]
fn known_support_ids_round_trip_through_their_single_catalog_authority() {
    for facet in ThemeRuleFacetV1::ALL {
        assert_eq!(ThemeRuleFacetV1::from_id(facet.id()), Some(*facet));
    }
    for output in ThemeSupportOutputV1::ALL {
        assert_eq!(ThemeSupportOutputV1::from_id(output.id()), Some(*output));
    }
}

#[test]
fn support_descriptor_serializes_a_versioned_bounded_result_envelope() {
    let query = ThemeSupportQueryV1::known(
        "flowchart",
        ThemeSupportOutputV1::StandaloneSvg,
        "node",
        ThemeRuleFacetV1::Radius,
    );
    let descriptor = ThemeCapabilityDescriptorV1::from_renderer_claim(
        1,
        query,
        ThemeSupportStateV1::Conditional,
        [
            "theme-support.family-owned-consumer-present",
            "theme-support.public-value-domain-partial",
        ],
    );

    assert_eq!(
        serde_json::to_value(descriptor).expect("support descriptor serializes"),
        serde_json::json!({
            "schema_version": 1,
            "claim_revision": 1,
            "query": {
                "schema_version": 1,
                "family": "flowchart",
                "output": "standalone-svg",
                "target": "node",
                "facet": "radius"
            },
            "state": "conditional",
            "reason_ids": [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial"
            ]
        })
    );
}
