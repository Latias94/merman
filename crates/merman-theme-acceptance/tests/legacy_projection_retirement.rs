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
        0x7d, 0x65, 0x68, 0xb9, 0x03, 0xde, 0x92, 0xe0, 0xa2, 0x31, 0xf9, 0x42, 0x64, 0xdc, 0x05,
        0x1e, 0x8d, 0x5a, 0xe9, 0x1e, 0x81, 0xc9, 0xa5, 0x39, 0x1b, 0xc3, 0x3d, 0x64, 0x87, 0x3f,
        0x9e, 0xca,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0xe3, 0x09, 0xb6, 0xbf, 0x7f, 0xd4, 0xce, 0x65, 0x99, 0x40, 0x80, 0x13, 0x2a, 0xea, 0xe9,
        0x72, 0xb8, 0x8f, 0x63, 0x58, 0x70, 0x90, 0x40, 0x6d, 0xb6, 0xee, 0x40, 0xcb, 0xaf, 0x43,
        0x7f, 0x63,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0x80, 0xaf, 0x43, 0xcf, 0x1c, 0xc5, 0x17, 0x00, 0x98, 0x8a, 0xdf, 0x8f, 0x58, 0x3d, 0x04,
        0xef, 0xb2, 0x9e, 0x9b, 0xbd, 0x26, 0x62, 0xa3, 0x31, 0xd9, 0xc4, 0xa7, 0x64, 0xe3, 0x42,
        0x75, 0x8e,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 262);
    assert_eq!(status.matrix_family_count(), 15);
    assert_eq!(status.dispatched_family_count(), 15);
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
