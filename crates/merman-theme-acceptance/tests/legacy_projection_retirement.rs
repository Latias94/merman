#![cfg(merman_internal_theme_acceptance)]

use merman_theme_acceptance::{
    LegacyProjectionRetirementAuthorization, authorize_legacy_projection_retirements,
};

#[test]
fn ktd23_authorizes_the_independent_full_retirement_inventory() {
    let runner: fn() -> Result<LegacyProjectionRetirementAuthorization, _> =
        authorize_legacy_projection_retirements;
    let authorization = runner()
        .expect("KTD23 must compare current probes with the independent historical witness");

    assert_eq!(authorization.manifest_version(), 6);
    assert_eq!(authorization.retirement_count(), 74);
    assert_eq!(authorization.value_probe_count(), 148);
    assert_ne!(authorization.manifest_digest(), &[0; 32]);
    assert_ne!(authorization.historical_witness_digest(), &[0; 32]);
    assert_ne!(authorization.production_inventory_digest(), &[0; 32]);
    assert_ne!(authorization.receipt_report_digest(), &[0; 32]);
}

#[test]
fn bridge_inventory_reports_live_legacy_routes_without_visual_proof() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        56, 214, 245, 115, 66, 170, 185, 107, 154, 252, 41, 213, 51, 222, 252, 148, 59, 3, 41, 27,
        128, 188, 18, 213, 41, 203, 169, 205, 115, 138, 201, 65,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        146, 200, 181, 98, 112, 145, 75, 209, 100, 155, 39, 129, 5, 59, 17, 202, 11, 6, 158, 64,
        88, 122, 141, 237, 132, 255, 208, 139, 208, 85, 148, 9,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        62, 68, 98, 5, 18, 230, 53, 76, 136, 186, 133, 229, 87, 66, 10, 227, 102, 33, 158, 202,
        160, 206, 15, 146, 217, 179, 180, 252, 103, 210, 54, 110,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 86);
    assert_eq!(status.matrix_family_count(), 3);
    assert_eq!(status.dispatched_family_count(), 3);
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
