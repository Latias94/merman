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
        0xf1, 0x75, 0x69, 0x7f, 0x10, 0x98, 0x3b, 0x41, 0x7d, 0x80, 0x7d, 0x7e, 0x7b, 0xe5, 0xad,
        0xac, 0x58, 0xff, 0x0a, 0x6f, 0xaf, 0x34, 0x1e, 0x0b, 0xf6, 0x4b, 0x3a, 0xe4, 0xcc, 0x94,
        0x60, 0x7b,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0x17, 0x7e, 0x46, 0xa3, 0x87, 0x88, 0x4d, 0xc4, 0x52, 0x03, 0x4e, 0x26, 0xa2, 0x95, 0x16,
        0x62, 0xb0, 0xd1, 0xf9, 0x0a, 0x29, 0x11, 0x6f, 0xfa, 0x03, 0xf9, 0x21, 0xbf, 0x57, 0x56,
        0x88, 0x3a,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0x5c, 0x03, 0x09, 0x50, 0x99, 0xa2, 0x53, 0x7a, 0xc2, 0x89, 0x6d, 0x89, 0x0d, 0x40, 0x8f,
        0xb2, 0x38, 0x3e, 0x65, 0xd8, 0xcd, 0xc2, 0x75, 0x38, 0x04, 0xb0, 0x90, 0xc3, 0x3c, 0x9b,
        0xa9, 0xc7,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_retirement_status();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 298);
    assert_eq!(status.matrix_family_count(), 19);
    assert_eq!(status.dispatched_family_count(), 19);
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
