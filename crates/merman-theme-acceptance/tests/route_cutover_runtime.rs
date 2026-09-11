#![cfg(merman_internal_theme_acceptance)]

use merman_theme_acceptance::{
    RouteCutoverAuthorizationReport, RouteCutoverRuntimeError, run_route_cutover_authorization,
};

#[test]
fn route_cutover_authorization_binds_manifest_and_authorization_digests() {
    let runner: fn() -> Result<RouteCutoverAuthorizationReport, RouteCutoverRuntimeError> =
        run_route_cutover_authorization;
    let report = runner()
        .expect("the typed bridge cutover inventory must have terminal authorization evidence");

    assert_ne!(report.manifest_digest(), &[0; 32]);
    assert_ne!(report.authorization_digest(), &[0; 32]);
}
