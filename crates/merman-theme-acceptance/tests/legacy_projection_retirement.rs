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
        11, 115, 188, 152, 224, 135, 125, 1, 210, 172, 22, 58, 140, 252, 202, 199, 41, 176, 23,
        134, 16, 21, 23, 235, 75, 70, 49, 226, 29, 103, 32, 170,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        213, 12, 155, 245, 113, 226, 120, 62, 158, 243, 119, 137, 176, 82, 1, 32, 57, 165, 64, 215,
        223, 210, 253, 161, 33, 248, 31, 191, 194, 44, 128, 102,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        32, 241, 230, 160, 147, 122, 246, 130, 165, 99, 255, 195, 49, 194, 226, 48, 168, 78, 209,
        37, 51, 64, 144, 193, 156, 85, 82, 126, 148, 37, 219, 46,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 184);
    assert_eq!(status.matrix_family_count(), 9);
    assert_eq!(status.dispatched_family_count(), 9);
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
