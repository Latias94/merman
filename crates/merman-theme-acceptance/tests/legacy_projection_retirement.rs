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
        117, 134, 23, 95, 146, 125, 61, 71, 160, 18, 18, 137, 235, 114, 48, 45, 196, 34, 88, 87,
        59, 37, 144, 5, 214, 137, 228, 185, 178, 160, 117, 99,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        155, 101, 159, 135, 15, 217, 60, 122, 93, 212, 77, 155, 157, 202, 95, 218, 183, 31, 207,
        230, 32, 53, 128, 48, 205, 123, 138, 115, 195, 2, 151, 32,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        119, 27, 234, 146, 148, 96, 38, 254, 94, 205, 65, 124, 152, 42, 232, 20, 84, 251, 179, 97,
        34, 113, 121, 124, 148, 89, 241, 20, 201, 122, 143, 238,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 214);
    assert_eq!(status.matrix_family_count(), 11);
    assert_eq!(status.dispatched_family_count(), 11);
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
