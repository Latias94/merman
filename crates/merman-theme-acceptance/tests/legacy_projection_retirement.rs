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
        77, 125, 131, 219, 133, 170, 153, 182, 147, 219, 243, 180, 102, 7, 199, 106, 129, 162, 224,
        76, 60, 92, 170, 73, 39, 8, 140, 242, 79, 234, 143, 27,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        91, 27, 202, 7, 158, 53, 61, 66, 95, 36, 207, 106, 68, 220, 36, 142, 204, 152, 173, 16,
        228, 112, 136, 136, 99, 106, 241, 17, 148, 246, 28, 84,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        32, 61, 200, 12, 197, 179, 211, 112, 190, 184, 99, 134, 153, 120, 255, 231, 98, 137, 180,
        212, 24, 73, 196, 69, 241, 104, 127, 240, 83, 127, 59, 172,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 242);
    assert_eq!(status.matrix_family_count(), 12);
    assert_eq!(status.dispatched_family_count(), 12);
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
