use merman_theme_contract::{
    ThemeCapabilityDescriptorV1, ThemeCapabilityDescriptorV2, ThemeRuleFacetV1,
    ThemeSupportBaseTypographyPropertyV2, ThemeSupportOutputV1, ThemeSupportQueryV1,
    ThemeSupportQueryV2, ThemeSupportStateV1, ThemeSupportSubjectV2,
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

#[test]
fn support_v2_subjects_serialize_as_one_stable_tagged_union() {
    let cases = [
        (
            ThemeSupportQueryV2::rule(
                "flowchart",
                ThemeSupportOutputV1::StandaloneSvg,
                "node",
                ThemeRuleFacetV1::Radius,
            ),
            serde_json::json!({
                "schema_version": 2,
                "family": "flowchart",
                "output": "standalone-svg",
                "subject": {
                    "kind": "rule",
                    "target": "node",
                    "facet": "radius"
                }
            }),
        ),
        (
            ThemeSupportQueryV2::ordinal_palette(
                "mindmap",
                ThemeSupportOutputV1::StandaloneSvg,
                "node",
            ),
            serde_json::json!({
                "schema_version": 2,
                "family": "mindmap",
                "output": "standalone-svg",
                "subject": {
                    "kind": "ordinal-palette",
                    "target": "node"
                }
            }),
        ),
        (
            ThemeSupportQueryV2::base_typography(
                "railroad",
                ThemeSupportOutputV1::StandaloneSvg,
                ThemeSupportBaseTypographyPropertyV2::FontSize,
            ),
            serde_json::json!({
                "schema_version": 2,
                "family": "railroad",
                "output": "standalone-svg",
                "subject": {
                    "kind": "base-typography",
                    "property": "font-size"
                }
            }),
        ),
    ];

    for (query, expected) in cases {
        assert_eq!(serde_json::to_value(query).unwrap(), expected);
    }
}

#[test]
fn support_v2_preserves_forward_compatible_subject_and_property_ids() {
    let unknown_subject: ThemeSupportQueryV2 = serde_json::from_value(serde_json::json!({
        "schema_version": 2,
        "family": "flowchart",
        "output": "standalone-svg",
        "subject": {
            "kind": "future-subject"
        }
    }))
    .expect("unknown subject tags remain valid discovery input");
    assert_eq!(
        unknown_subject.subject(),
        &ThemeSupportSubjectV2::Unknown {
            kind: "future-subject".to_owned()
        }
    );

    let unknown_property: ThemeSupportQueryV2 = serde_json::from_value(serde_json::json!({
        "schema_version": 2,
        "family": "railroad",
        "output": "standalone-svg",
        "subject": {
            "kind": "base-typography",
            "property": "future-property"
        }
    }))
    .expect("unknown base typography properties remain valid discovery input");
    assert_eq!(
        unknown_property.subject(),
        &ThemeSupportSubjectV2::BaseTypography {
            property: "future-property".to_owned()
        }
    );
}

#[test]
fn support_v2_base_typography_property_catalog_round_trips() {
    for property in ThemeSupportBaseTypographyPropertyV2::ALL {
        assert_eq!(
            ThemeSupportBaseTypographyPropertyV2::from_id(property.id()),
            Some(*property)
        );
    }
}

#[test]
fn support_v2_descriptor_serializes_the_subject_based_query() {
    let query = ThemeSupportQueryV2::base_typography(
        "railroad",
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeSupportBaseTypographyPropertyV2::FontSize,
    );
    let descriptor = ThemeCapabilityDescriptorV2::from_renderer_claim(
        1,
        query,
        ThemeSupportStateV1::Conditional,
        ["theme-support.family-owned-consumer-present"],
    );

    assert_eq!(
        serde_json::to_value(descriptor).expect("V2 support descriptor serializes"),
        serde_json::json!({
            "schema_version": 2,
            "claim_revision": 1,
            "query": {
                "schema_version": 2,
                "family": "railroad",
                "output": "standalone-svg",
                "subject": {
                    "kind": "base-typography",
                    "property": "font-size"
                }
            },
            "state": "conditional",
            "reason_ids": ["theme-support.family-owned-consumer-present"]
        })
    );
}
