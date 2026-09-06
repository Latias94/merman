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
        0xb2, 0x49, 0xae, 0x2f, 0xfa, 0xa9, 0xbc, 0xaa, 0xf0, 0xdb, 0x40, 0xb9, 0xc4, 0xc7, 0xad,
        0x6c, 0xc5, 0x5f, 0x3d, 0xf1, 0x3a, 0x2f, 0x63, 0xc9, 0x43, 0x2c, 0x95, 0x16, 0xc3, 0xd0,
        0xe2, 0x7d,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0x13, 0x5b, 0xf6, 0x83, 0xa6, 0x12, 0xc3, 0xce, 0x52, 0xb5, 0xe1, 0xc2, 0xb7, 0xcc, 0x3a,
        0x8c, 0xb1, 0xd2, 0x4c, 0x4c, 0x24, 0x0b, 0x9f, 0x3f, 0x90, 0xee, 0x03, 0xc1, 0xa7, 0x0f,
        0x18, 0xb3,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0x88, 0x04, 0xd4, 0xec, 0x9b, 0x15, 0xf6, 0xe2, 0x0f, 0xd4, 0xf1, 0x06, 0x0d, 0xa0, 0xe7,
        0xa6, 0xdf, 0x65, 0xda, 0x80, 0x2e, 0x67, 0x5e, 0x4e, 0xa2, 0x8b, 0xd6, 0xba, 0x81, 0xa5,
        0xbd, 0x57,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_retirement_status();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 282);
    assert_eq!(status.matrix_family_count(), 17);
    assert_eq!(status.dispatched_family_count(), 17);
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
