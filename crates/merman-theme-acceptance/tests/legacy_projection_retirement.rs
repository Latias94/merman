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
        0x86, 0xec, 0xfd, 0xfa, 0xf4, 0xd3, 0x66, 0xb5, 0x9b, 0x38, 0x36, 0x25, 0x0b, 0xd2, 0x6d,
        0x45, 0x2b, 0x3d, 0x2b, 0x13, 0xff, 0x58, 0x26, 0xca, 0x5e, 0x94, 0x41, 0xea, 0x1a, 0x6e,
        0x2d, 0xec,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0x53, 0x8b, 0xdb, 0xf5, 0x9e, 0x50, 0x01, 0xae, 0xc6, 0x9a, 0xd0, 0xef, 0x20, 0x17, 0x25,
        0x97, 0x7c, 0xf6, 0x91, 0xc8, 0xe4, 0xea, 0xff, 0xc8, 0xda, 0x56, 0xf1, 0x7e, 0xbc, 0x09,
        0x3d, 0xbc,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0x71, 0x4c, 0x29, 0xe9, 0xc5, 0x6c, 0x18, 0x3f, 0x3e, 0x7e, 0x34, 0xe0, 0x19, 0x8e, 0x32,
        0xb8, 0xd4, 0xf9, 0x7a, 0x68, 0x2c, 0x18, 0xf0, 0x00, 0xcb, 0x78, 0x87, 0x92, 0x01, 0x43,
        0xc1, 0x39,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_retirement_status();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 306);
    assert_eq!(status.matrix_family_count(), 20);
    assert_eq!(status.dispatched_family_count(), 20);
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
