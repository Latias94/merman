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
        170, 105, 212, 243, 250, 63, 178, 90, 71, 59, 70, 45, 176, 145, 174, 170, 164, 100, 86, 40,
        64, 58, 32, 65, 10, 119, 189, 8, 159, 236, 124, 249,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        220, 157, 145, 114, 47, 193, 89, 43, 240, 101, 250, 129, 67, 186, 100, 66, 82, 248, 81, 10,
        126, 186, 19, 173, 44, 254, 21, 139, 104, 15, 81, 183,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        235, 191, 4, 235, 121, 168, 8, 55, 101, 132, 44, 176, 65, 180, 3, 170, 199, 104, 120, 224,
        115, 163, 156, 114, 93, 123, 226, 190, 67, 122, 30, 118,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 142);
    assert_eq!(status.matrix_family_count(), 6);
    assert_eq!(status.dispatched_family_count(), 6);
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
