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

    assert_eq!(authorization.manifest_version(), 5);
    assert_eq!(authorization.retirement_count(), 66);
    assert_eq!(authorization.value_probe_count(), 132);
    assert_ne!(authorization.manifest_digest(), &[0; 32]);
    assert_ne!(authorization.historical_witness_digest(), &[0; 32]);
    assert_ne!(authorization.production_inventory_digest(), &[0; 32]);
    assert_ne!(authorization.receipt_report_digest(), &[0; 32]);
}

#[test]
fn bridge_inventory_reports_live_legacy_routes_without_visual_proof() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        228, 206, 102, 249, 236, 14, 50, 23, 22, 205, 116, 66, 16, 224, 67, 168, 111, 65, 76, 7,
        222, 217, 87, 69, 33, 178, 83, 54, 145, 105, 221, 71,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        118, 180, 155, 132, 95, 252, 137, 117, 138, 183, 64, 127, 248, 160, 172, 77, 18, 205, 185,
        186, 22, 107, 192, 172, 101, 0, 90, 178, 21, 83, 124, 220,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        177, 166, 70, 135, 80, 102, 102, 183, 131, 212, 17, 161, 129, 7, 172, 10, 157, 136, 16,
        253, 34, 217, 50, 229, 150, 190, 91, 113, 248, 163, 212, 177,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 162);
    assert_eq!(status.matrix_family_count(), 7);
    assert_eq!(status.dispatched_family_count(), 7);
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
