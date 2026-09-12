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
        11, 244, 242, 41, 100, 137, 137, 233, 48, 98, 109, 92, 25, 133, 197, 28, 2, 69, 121, 99,
        223, 64, 37, 98, 112, 162, 176, 164, 45, 149, 48, 203,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        216, 10, 61, 148, 214, 118, 69, 222, 187, 193, 53, 212, 38, 141, 26, 60, 165, 151, 0, 89,
        143, 220, 36, 181, 120, 240, 80, 134, 6, 163, 175, 132,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        27, 163, 33, 220, 12, 245, 182, 66, 109, 36, 235, 31, 15, 93, 92, 31, 218, 255, 111, 198,
        93, 114, 159, 128, 63, 43, 120, 226, 66, 236, 91, 50,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 94);
    assert_eq!(status.matrix_family_count(), 4);
    assert_eq!(status.dispatched_family_count(), 4);
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
