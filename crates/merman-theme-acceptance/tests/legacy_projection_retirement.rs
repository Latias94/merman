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
fn bridge_inventory_reports_live_legacy_routes_without_visual_proof() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        82, 158, 56, 87, 221, 84, 135, 159, 221, 169, 207, 110, 18, 115, 239, 172, 222, 25, 127,
        237, 243, 180, 242, 98, 82, 194, 254, 73, 155, 6, 183, 61,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        113, 216, 240, 96, 78, 109, 128, 32, 228, 87, 90, 101, 180, 72, 232, 194, 206, 89, 54, 101,
        81, 116, 213, 41, 189, 95, 142, 168, 49, 149, 248, 185,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        49, 107, 86, 220, 165, 170, 20, 220, 180, 75, 4, 252, 118, 119, 247, 37, 124, 117, 252,
        104, 109, 215, 13, 136, 211, 108, 6, 10, 224, 81, 172, 202,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 12);
    assert_eq!(status.matrix_family_count(), 1);
    assert_eq!(status.dispatched_family_count(), 1);
    assert_eq!(status.matrix_route_digest(), EXPECTED_MATRIX_ROUTE_DIGEST);
    assert_eq!(status.matrix_family_digest(), EXPECTED_MATRIX_FAMILY_DIGEST);
    assert_eq!(
        status.dispatched_family_digest(),
        EXPECTED_DISPATCH_FAMILY_DIGEST
    );
    assert_eq!(status.matrix_only_family_count(), 0);
    assert_eq!(status.dispatch_only_family_count(), 0);
    assert!(!status.route_dispatch_is_empty());
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
