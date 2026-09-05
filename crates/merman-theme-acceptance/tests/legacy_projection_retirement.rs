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
        0x25, 0x71, 0x91, 0x2f, 0xc2, 0xe2, 0x0e, 0xd0, 0x40, 0x98, 0xc2, 0x11, 0x6f, 0xf4, 0x95,
        0xda, 0x6b, 0x36, 0x7d, 0x46, 0x42, 0x94, 0xab, 0x65, 0x95, 0xf2, 0x72, 0xb5, 0x43, 0xc1,
        0xb9, 0xbb,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        0xde, 0x1d, 0xed, 0x0e, 0x2f, 0x87, 0x54, 0xf3, 0xb9, 0xcc, 0x65, 0x1b, 0xbf, 0xe4, 0x78,
        0x56, 0xa1, 0x7f, 0xfe, 0x8b, 0x26, 0x0e, 0x7b, 0x35, 0xd9, 0x88, 0x74, 0xb2, 0x75, 0x1f,
        0xc4, 0xd7,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        0xd1, 0x10, 0xba, 0xbc, 0xc8, 0xb9, 0xb5, 0x89, 0x4f, 0x08, 0x7d, 0xad, 0x69, 0xcd, 0x04,
        0xf6, 0xca, 0x30, 0x8a, 0x8f, 0x7b, 0x55, 0xa1, 0x7d, 0xa7, 0xa0, 0xb7, 0x91, 0xfe, 0xca,
        0xbd, 0x52,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_retirement_status();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 370);
    assert_eq!(status.matrix_family_count(), 24);
    assert_eq!(status.dispatched_family_count(), 24);
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
