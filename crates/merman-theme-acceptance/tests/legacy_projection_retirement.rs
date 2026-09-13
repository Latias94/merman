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

    assert_eq!(authorization.manifest_version(), 8);
    assert_eq!(authorization.retirement_count(), 78);
    assert_eq!(authorization.value_probe_count(), 156);
    assert_ne!(authorization.manifest_digest(), &[0; 32]);
    assert_ne!(authorization.historical_witness_digest(), &[0; 32]);
    assert_ne!(authorization.production_inventory_digest(), &[0; 32]);
    assert_ne!(authorization.receipt_report_digest(), &[0; 32]);
}

#[test]
fn bridge_inventory_reports_live_legacy_routes_without_visual_proof() {
    const EXPECTED_MATRIX_ROUTE_DIGEST: [u8; 32] = [
        1, 89, 64, 47, 193, 211, 87, 242, 37, 10, 16, 167, 122, 214, 17, 42, 100, 239, 51, 251, 57,
        254, 46, 228, 133, 48, 93, 177, 212, 203, 187, 244,
    ];
    const EXPECTED_MATRIX_FAMILY_DIGEST: [u8; 32] = [
        202, 91, 224, 228, 19, 201, 254, 185, 200, 178, 75, 195, 88, 54, 84, 162, 33, 28, 83, 211,
        146, 12, 62, 28, 208, 197, 220, 148, 119, 9, 164, 46,
    ];
    const EXPECTED_DISPATCH_FAMILY_DIGEST: [u8; 32] = [
        81, 162, 184, 249, 77, 209, 178, 116, 222, 216, 250, 166, 117, 53, 21, 69, 23, 216, 137,
        255, 80, 120, 159, 44, 247, 104, 114, 115, 232, 43, 242, 88,
    ];

    let status = merman::__theme_acceptance::legacy_family_theme_bridge_inventory();

    assert_eq!(status.dispatch_error_count(), 0);
    assert_eq!(status.matrix_route_count(), 62);
    assert_eq!(status.matrix_family_count(), 2);
    assert_eq!(status.dispatched_family_count(), 2);
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
