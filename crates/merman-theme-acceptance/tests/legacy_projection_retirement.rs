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
        0x8f, 0xd4, 0xcd, 0x68, 0x8d, 0x87, 0xc3, 0xeb, 0xa8, 0x6a, 0xc2, 0x91, 0xa8, 0x9e, 0x35,
        0xd5, 0x21, 0xea, 0x3e, 0x93, 0xee, 0xa3, 0xa9, 0x97, 0xf5, 0xc0, 0x08, 0xfc, 0x0b, 0x6c,
        0x0c, 0x67,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0x4a, 0xc2, 0x67, 0x2f, 0x90, 0x43, 0xa3, 0xe4, 0xad, 0x5a, 0x39, 0x9e, 0x53, 0x01, 0xa4,
        0xd7, 0x09, 0x2c, 0x30, 0xec, 0x8a, 0xe8, 0xe5, 0x60, 0x7e, 0x7b, 0xb7, 0x16, 0xf3, 0xaf,
        0xf8, 0x22,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0xcc, 0x78, 0xeb, 0xdd, 0x8d, 0x32, 0xee, 0xb6, 0x96, 0x5b, 0x1d, 0x94, 0x59, 0xb0, 0xa2,
        0xa1, 0xec, 0x28, 0x43, 0x86, 0x24, 0x88, 0x7d, 0x9c, 0x8f, 0x03, 0x3c, 0xdd, 0xb9, 0x67,
        0x5a, 0x61,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 266);
    assert_eq!(status.matrix_family_count(), 16);
    assert_eq!(status.dispatched_family_count(), 16);
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
