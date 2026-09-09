use merman_theme_acceptance::{
    LegacyProjectionRetirementAuthorization, authorize_legacy_projection_retirements,
};

#[test]
fn ktd23_authorizes_the_independent_full_retirement_inventory() {
    let runner: fn() -> Result<LegacyProjectionRetirementAuthorization, _> =
        authorize_legacy_projection_retirements;
    let authorization = runner()
        .expect("KTD23 must compare current probes with the independent historical witness");

    assert_eq!(authorization.manifest_version(), 3);
    assert_eq!(authorization.retirement_count(), 60);
    assert_eq!(authorization.value_probe_count(), 120);
    assert_ne!(authorization.manifest_digest(), &[0; 32]);
    assert_ne!(authorization.historical_witness_digest(), &[0; 32]);
    assert_ne!(authorization.production_inventory_digest(), &[0; 32]);
    assert_ne!(authorization.receipt_report_digest(), &[0; 32]);
}

#[test]
fn bridge_inventory_reports_live_legacy_routes_without_visual_proof() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        24, 188, 239, 13, 126, 185, 204, 157, 12, 249, 110, 191, 121, 180, 113, 133, 22, 173, 20,
        146, 3, 115, 185, 173, 125, 55, 126, 58, 172, 181, 161, 147,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        95, 1, 209, 154, 147, 189, 235, 158, 138, 101, 210, 238, 227, 0, 170, 189, 150, 121, 25,
        116, 9, 7, 66, 54, 158, 166, 241, 23, 199, 118, 210, 184,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        60, 17, 50, 133, 113, 23, 86, 213, 61, 122, 154, 82, 57, 162, 209, 163, 171, 38, 78, 27,
        46, 134, 46, 168, 9, 89, 242, 86, 45, 6, 193, 142,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 250);
    assert_eq!(status.matrix_family_count(), 13);
    assert_eq!(status.dispatched_family_count(), 13);
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
