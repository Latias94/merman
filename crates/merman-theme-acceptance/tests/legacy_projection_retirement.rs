use merman_theme_acceptance::{
    LegacyProjectionRetirementAuthorization, authorize_legacy_projection_retirements,
};

#[test]
fn ktd23_authorizes_the_independent_full_retirement_inventory() {
    let runner: fn() -> Result<LegacyProjectionRetirementAuthorization, _> =
        authorize_legacy_projection_retirements;
    let authorization = runner()
        .expect("KTD23 must compare current probes with the independent historical witness");

    assert_eq!(authorization.manifest_version(), 2);
    assert_eq!(authorization.retirement_count(), 58);
    assert_eq!(authorization.value_probe_count(), 116);
    assert_ne!(authorization.manifest_digest(), &[0; 32]);
    assert_ne!(authorization.historical_witness_digest(), &[0; 32]);
    assert_ne!(authorization.production_inventory_digest(), &[0; 32]);
    assert_ne!(authorization.receipt_report_digest(), &[0; 32]);
}

#[test]
fn bridge_inventory_reports_live_legacy_routes_without_visual_proof() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        234, 248, 144, 208, 35, 120, 120, 26, 110, 140, 49, 15, 204, 101, 217, 213, 12, 182, 75,
        57, 114, 129, 30, 23, 24, 119, 200, 95, 205, 165, 44, 229,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0x95, 0xdd, 0x05, 0x43, 0x4f, 0x1d, 0x61, 0xb2, 0x62, 0x0c, 0xd6, 0x5f, 0x2d, 0x95, 0xf9,
        0xd9, 0x85, 0x02, 0xe4, 0xe6, 0x1f, 0xbd, 0x9e, 0xac, 0x56, 0x7c, 0xdd, 0xe4, 0x4e, 0x0b,
        0x03, 0x19,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0xe2, 0xae, 0x8e, 0x38, 0x49, 0x7e, 0x16, 0x6c, 0x36, 0xac, 0x16, 0xe3, 0xec, 0x0b, 0xab,
        0x82, 0xf4, 0x46, 0x34, 0x4b, 0x35, 0x0b, 0x7a, 0xe8, 0x9f, 0x07, 0x36, 0xa5, 0xe9, 0x4c,
        0x0d, 0x33,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 254);
    assert_eq!(status.matrix_family_count(), 14);
    assert_eq!(status.dispatched_family_count(), 14);
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
