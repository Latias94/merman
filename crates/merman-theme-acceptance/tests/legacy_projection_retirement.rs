#![cfg(merman_internal_theme_acceptance)]

use merman_theme_acceptance::{
    LegacyProjectionRetirementAuthorization, authorize_legacy_projection_retirements,
};

#[test]
fn ktd23_authorizes_the_independent_full_retirement_inventory() {
    let runner: fn() -> Result<LegacyProjectionRetirementAuthorization, _> =
        authorize_legacy_projection_retirements;
    let authorization = runner()
        .expect("KTD23 must compare current probes with the independent historical witness");

    assert_eq!(authorization.manifest_version(), 9);
    assert_eq!(authorization.retirement_count(), 80);
    assert_eq!(authorization.value_probe_count(), 160);
    assert_ne!(authorization.manifest_digest(), &[0; 32]);
    assert_ne!(authorization.historical_witness_digest(), &[0; 32]);
    assert_ne!(authorization.production_inventory_digest(), &[0; 32]);
    assert_ne!(authorization.receipt_report_digest(), &[0; 32]);
}

#[test]
fn bridge_inventory_confirms_complete_legacy_route_retirement() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        6, 238, 134, 105, 100, 36, 13, 21, 48, 12, 40, 205, 143, 82, 119, 45, 68, 74, 230, 58, 73,
        77, 134, 99, 208, 217, 210, 251, 212, 211, 230, 146,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        181, 11, 131, 200, 17, 132, 74, 192, 155, 90, 117, 53, 155, 15, 218, 23, 127, 151, 57, 57,
        120, 230, 55, 1, 228, 29, 81, 10, 52, 34, 128, 232,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        194, 114, 37, 66, 6, 54, 59, 42, 195, 176, 83, 45, 106, 187, 195, 12, 10, 221, 85, 93, 129,
        85, 94, 33, 167, 210, 136, 147, 252, 54, 93, 75,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 0);
    assert_eq!(status.matrix_family_count(), 0);
    assert_eq!(status.dispatched_family_count(), 0);
    assert_eq!(status.matrix_route_digest(), EXPECTED_MATRIX_ROUTE_DIGEST);
    assert_eq!(status.matrix_family_digest(), EXPECTED_MATRIX_FAMILY_DIGEST);
    assert_eq!(
        status.dispatched_family_digest(),
        EXPECTED_DISPATCH_FAMILY_DIGEST
    );
    assert_eq!(status.matrix_only_family_count(), 0);
    assert_eq!(status.dispatch_only_family_count(), 0);
    assert!(status.route_dispatch_is_empty());
}

#[test]
fn class_edge_label_background_selector_has_no_historical_group_consumer() {
    // Independent upstream SVG witnesses cover classic/Dagre, handDrawn/Dagre, and ELK.
    // The fixture set does not expose a separate HTML/SVG switch for every look; this test
    // records only dimensions represented by checked-in historical artifacts.
    let fixtures = [
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_a_custom_theme_056.svg"
        ),
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_a_class_with_text_label_033.svg"
        ),
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_simple_class_diagram_with_a_custom_theme_055.svg"
        ),
    ];

    for svg in fixtures {
        assert!(svg.contains(".edgeLabel[data-look=\"neo\"]"));
        let mut cursor = 0usize;
        while let Some(relative) = svg[cursor..].find("<g class=\"edgeLabel\"") {
            let start = cursor + relative;
            let end = svg[start..]
                .find("</g>")
                .map(|offset| start + offset)
                .unwrap_or(svg.len());
            let group = &svg[start..end];
            assert!(
                !group.contains("data-look="),
                "historical Class edgeLabel group unexpectedly carries data-look: {group}"
            );
            cursor = end.saturating_add(4);
            if cursor >= svg.len() {
                break;
            }
        }
        assert!(svg.contains(".edgeLabel .label rect{fill:"));
        assert!(svg.contains(".labelBkg{background:"));
        assert!(svg.contains(".edgeLabel .label span{background:"));
    }
}

#[test]
fn class_edge_label_background_historical_projection_baseline_digest() {
    use sha2::{Digest as _, Sha256};
    let fixtures = [
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_a_custom_theme_056.svg"
        ),
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_a_class_with_text_label_033.svg"
        ),
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_simple_class_diagram_with_a_custom_theme_055.svg"
        ),
    ];
    let mut digest = Sha256::new();
    for svg in fixtures {
        let style = svg
            .split_once("<style>")
            .and_then(|(_, rest)| rest.split_once("</style>"))
            .map(|(style, _)| style)
            .expect("historical Class fixture style block");
        for marker in [
            ".edgeLabel[data-look=\"neo\"]",
            ".edgeLabel .label rect",
            ".labelBkg",
            ".edgeLabel .label span",
        ] {
            let start = style.find(marker).expect("historical Class CSS marker");
            let end = style[start..]
                .find('}')
                .map(|offset| start + offset + 1)
                .expect("historical Class CSS declaration");
            digest.update(&style[start..end]);
        }
    }
    let digest: [u8; 32] = digest.finalize().into();
    assert_eq!(
        digest,
        [
            0x04, 0x44, 0x4e, 0xec, 0xf9, 0x1d, 0x09, 0xda, 0x2f, 0x89, 0x2e, 0x04, 0x58, 0x33,
            0xc0, 0x97, 0xc2, 0x49, 0x69, 0x57, 0x79, 0x9e, 0x23, 0x2a, 0x6d, 0x81, 0x75, 0x6d,
            0xb3, 0x25, 0x98, 0x4b
        ],
        "Class edge-label background CSS projection baseline changed",
    );
}
