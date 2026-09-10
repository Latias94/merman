use merman_theme_acceptance::{
    LegacyProjectionRetirementAuthorization, authorize_legacy_projection_retirements,
};

#[test]
fn ktd23_authorizes_the_independent_full_retirement_inventory() {
    let runner: fn() -> Result<LegacyProjectionRetirementAuthorization, _> =
        authorize_legacy_projection_retirements;
    let authorization = runner()
        .expect("KTD23 must compare current probes with the independent historical witness");

    assert_eq!(authorization.manifest_version(), 4);
    assert_eq!(authorization.retirement_count(), 62);
    assert_eq!(authorization.value_probe_count(), 124);
    assert_ne!(authorization.manifest_digest(), &[0; 32]);
    assert_ne!(authorization.historical_witness_digest(), &[0; 32]);
    assert_ne!(authorization.production_inventory_digest(), &[0; 32]);
    assert_ne!(authorization.receipt_report_digest(), &[0; 32]);
}

#[test]
fn bridge_inventory_reports_live_legacy_routes_without_visual_proof() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        171, 21, 233, 61, 224, 197, 215, 106, 94, 156, 71, 114, 15, 21, 10, 139, 8, 104, 215, 133,
        158, 232, 170, 118, 36, 175, 168, 88, 111, 108, 165, 28,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        40, 57, 158, 73, 73, 152, 15, 78, 240, 220, 196, 42, 209, 254, 126, 197, 89, 18, 48, 252,
        1, 105, 0, 238, 214, 132, 137, 144, 29, 231, 12, 43,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        100, 240, 238, 42, 118, 143, 18, 137, 24, 67, 197, 66, 211, 226, 3, 115, 33, 204, 203, 28,
        42, 250, 164, 153, 176, 12, 239, 98, 39, 175, 203, 145,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 192);
    assert_eq!(status.matrix_family_count(), 10);
    assert_eq!(status.dispatched_family_count(), 10);
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
