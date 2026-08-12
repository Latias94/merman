#[test]
fn enforced_c6_cells_are_proved_by_runtime_artifacts() {
    let report = merman_theme_acceptance::run_enforced_c6_runtime();
    assert_eq!(report.verified_cell_count(), 4);
    assert_eq!(report.enforced_cell_count(), 4);
    assert_ne!(report.execution_digest(), &[0; 32]);
}
