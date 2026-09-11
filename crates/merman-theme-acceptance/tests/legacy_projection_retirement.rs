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
        162, 145, 190, 186, 120, 122, 158, 175, 0, 120, 251, 117, 93, 156, 146, 219, 26, 172, 63,
        249, 11, 93, 171, 212, 40, 36, 203, 122, 213, 241, 62, 164,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        253, 171, 157, 197, 106, 146, 47, 23, 92, 17, 161, 30, 215, 126, 179, 54, 14, 202, 181, 37,
        176, 216, 186, 248, 226, 218, 159, 65, 73, 149, 144, 99,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        49, 18, 79, 90, 11, 217, 227, 21, 142, 135, 222, 178, 11, 115, 88, 150, 121, 143, 23, 112,
        215, 179, 217, 125, 204, 242, 45, 100, 73, 201, 236, 100,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 180);
    assert_eq!(status.matrix_family_count(), 8);
    assert_eq!(status.dispatched_family_count(), 8);
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
