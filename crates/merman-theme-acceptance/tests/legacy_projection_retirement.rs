use merman_theme_acceptance::{
    LegacyProjectionRetirementAuthorization, authorize_legacy_projection_retirements,
};

#[test]
fn ktd23_authorizes_the_independent_full_retirement_inventory() {
    let runner: fn() -> Result<LegacyProjectionRetirementAuthorization, _> =
        authorize_legacy_projection_retirements;
    let authorization = runner()
        .expect("KTD23 must compare current probes with the independent historical witness");

    assert_eq!(authorization.manifest_version(), 2);
    assert_eq!(authorization.retirement_count(), 58);
    assert_eq!(authorization.value_probe_count(), 116);
    assert_ne!(authorization.manifest_digest(), &[0; 32]);
    assert_ne!(authorization.historical_witness_digest(), &[0; 32]);
    assert_ne!(authorization.production_inventory_digest(), &[0; 32]);
    assert_ne!(authorization.receipt_report_digest(), &[0; 32]);
}

#[test]
fn bridge_retirement_gate_reports_live_legacy_routes_without_visual_proof() {
    let status = merman::__theme_acceptance::legacy_family_theme_bridge_retirement_status();

    assert_eq!(status.dispatch_error_count(), 0);
    assert!(status.matrix_route_count() > 0);
    assert!(status.matrix_family_count() > 0);
    assert!(status.dispatched_family_count() > 0);
    assert_eq!(status.matrix_only_family_count(), 0);
    assert_eq!(status.dispatch_only_family_count(), 0);
    assert!(!status.can_delete_bridge());
}
