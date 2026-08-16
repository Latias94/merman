use std::path::Path;

use merman_theme_fixtures::{C6AcceptanceCatalog, ThemeFixtureCatalog};

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

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
    assert_eq!(report.verified_cell_count(), 18);
    assert_eq!(report.render_group_count(), 9);
    assert_eq!(
        report.verified_cell_count(),
        acceptance.enforced_tranche().cells().len()
    );
    assert_eq!(report.manifest_digest(), acceptance.manifest_digest());
    assert_ne!(report.execution_digest(), &[0; 32]);
}

#[test]
fn exact_native_ledger_is_the_only_c6a_eligibility_issuer_input() {
    let themes_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes");
    let fixtures = ThemeFixtureCatalog::load(themes_root).expect("load the theme fixture catalog");
    let acceptance = C6AcceptanceCatalog::load(&fixtures).expect("load the C6 acceptance catalog");

    let receipt = merman_theme_acceptance::run_c6a_eligibility()
        .expect("the exact 18-cell native ledger must qualify");

    assert_eq!(receipt.verified_cell_count(), 18);
    assert_eq!(receipt.render_group_count(), 9);
    assert_eq!(acceptance.schema_version(), 4);
    assert_eq!(receipt.manifest_revision(), acceptance.manifest_revision());
    assert_eq!(receipt.manifest_digest(), acceptance.manifest_digest());
    assert_eq!(
        receipt.previous_manifest_digest(),
        acceptance.previous_manifest_digest()
    );
    assert_eq!(
        encode_hex(receipt.previous_manifest_digest()),
        "2b9a4b96853c8728c4d11435becc094394e5430e3af2a60ef720181eb076efd2"
    );
    assert_ne!(receipt.execution_digest(), &[0; 32]);
    assert_ne!(receipt.proof_recipe_revisions_digest(), &[0; 32]);
    assert_ne!(receipt.critical_mechanisms_digest(), &[0; 32]);
    assert_ne!(receipt.receipt_digest(), &[0; 32]);
}
