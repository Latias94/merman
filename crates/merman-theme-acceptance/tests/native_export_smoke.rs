#[test]
fn representative_native_exports_share_one_portable_document() {
    let summary = merman_theme_acceptance::run_representative_native_export_smoke()
        .expect("the representative PNG/JPEG/PDF native export smoke must pass");

    assert_eq!(summary.verified_projection_count(), 3);
}
