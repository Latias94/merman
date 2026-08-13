use std::path::Path;

use merman_theme_fixtures::{C6AcceptanceCatalog, ThemeFixtureCatalog};

#[test]
fn enforced_c6_cells_are_proved_by_runtime_artifacts() {
    let themes_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes");
    let fixtures = ThemeFixtureCatalog::load(themes_root).expect("load the theme fixture catalog");
    let acceptance = C6AcceptanceCatalog::load(&fixtures).expect("load the C6 acceptance catalog");
    let report = merman_theme_acceptance::run_enforced_c6_runtime()
        .expect("the enforced C6 tranche must have a target adapter");
    assert_eq!(
        report.verified_cell_count(),
        acceptance.enforced_tranche().cells().len()
    );
    assert!(report.render_group_count() > 0);
    assert_ne!(report.execution_digest(), &[0; 32]);
}
