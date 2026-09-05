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
fn bridge_retirement_gate_reports_live_legacy_routes_without_visual_proof() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        0x2e, 0xb6, 0xb2, 0xfe, 0xf4, 0xc0, 0x21, 0xd6, 0xed, 0x6e, 0xa0, 0x71, 0x73, 0xbd, 0x16,
        0x47, 0x24, 0xa4, 0xa5, 0x4b, 0x50, 0xf1, 0x64, 0x6b, 0x22, 0x86, 0xed, 0x4c, 0x73, 0x6b,
        0x9d, 0x99,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0xbc, 0x40, 0x3a, 0x8f, 0xed, 0xa3, 0x07, 0x97, 0x11, 0x43, 0x8b, 0x5b, 0x3d, 0xb1, 0x7a,
        0xca, 0x27, 0x60, 0xc6, 0x7c, 0x35, 0x2a, 0x04, 0x08, 0xf1, 0x6b, 0x93, 0x7d, 0x49, 0xe3,
        0x36, 0xa2,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0xde, 0x3b, 0x64, 0xa7, 0xb4, 0x54, 0xbd, 0xf4, 0xb1, 0xe0, 0x18, 0x02, 0x85, 0xfa, 0x04,
        0x33, 0xf0, 0x24, 0xe4, 0xb9, 0x26, 0xdb, 0x36, 0xe0, 0x88, 0x54, 0xd2, 0x3e, 0xa5, 0x8a,
        0xa8, 0xf3,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_retirement_status();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 360);
    assert_eq!(status.matrix_family_count(), 22);
    assert_eq!(status.dispatched_family_count(), 22);
    assert_eq!(status.matrix_route_digest(), EXPECTED_MATRIX_ROUTE_DIGEST);
    assert_eq!(status.matrix_family_digest(), EXPECTED_MATRIX_FAMILY_DIGEST);
    assert_eq!(
        status.dispatched_family_digest(),
        EXPECTED_DISPATCH_FAMILY_DIGEST
    );
    assert_eq!(status.matrix_only_family_count(), 0);
    assert_eq!(status.dispatch_only_family_count(), 0);
    assert!(!status.can_delete_bridge(true));
    assert!(!status.can_delete_bridge(false));
}
