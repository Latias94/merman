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
        0x52, 0xfa, 0x7a, 0x63, 0x07, 0x27, 0x1e, 0x90, 0x6e, 0x2e, 0xd1, 0xab, 0x21, 0x32, 0xf4,
        0x71, 0xb1, 0x5a, 0xea, 0x74, 0x07, 0xdb, 0xbd, 0x95, 0xb3, 0x53, 0xe0, 0x2e, 0xe3, 0xdc,
        0x24, 0xde,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0x13, 0x25, 0x2d, 0xa6, 0x7d, 0xae, 0x5e, 0x64, 0x45, 0x32, 0xa7, 0x79, 0x40, 0xa7, 0x29,
        0xa1, 0x07, 0xe5, 0x8b, 0x42, 0xa3, 0x4f, 0x81, 0x62, 0xd0, 0xb3, 0xfd, 0xe1, 0x68, 0xe0,
        0x94, 0x9c,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0x51, 0xef, 0xa1, 0x21, 0xcc, 0x99, 0x5b, 0x7a, 0x3f, 0x5c, 0xc0, 0x3a, 0x75, 0x6a, 0x92,
        0xb9, 0x3f, 0xaa, 0x24, 0x84, 0x6a, 0xb1, 0x95, 0xad, 0xf7, 0x22, 0xf6, 0x10, 0x25, 0xf6,
        0xd1, 0xa6,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_retirement_status();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 364);
    assert_eq!(status.matrix_family_count(), 23);
    assert_eq!(status.dispatched_family_count(), 23);
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
