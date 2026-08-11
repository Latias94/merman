use merman_theme_fixtures::{
    C6_ACCEPTANCE_CELL_COUNT, C6_ACCEPTANCE_RELATIVE_PATH, C6_PROOF_FAMILIES, C6_PROOF_THEMES,
    C6AcceptanceCatalog, C6ArtifactAssertion, C6CellKey, C6ExpectedMechanismDisposition,
    C6ProofFamily, C6ProofTheme, C6RequiredAdmission, CatalogError, EXPECTED_OUTPUT_TARGETS,
    ThemeFixtureCatalog,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn themes_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes")
}

fn source_catalog() -> ThemeFixtureCatalog {
    ThemeFixtureCatalog::load(themes_root()).expect("load committed theme source catalog")
}

fn committed_acceptance_json() -> String {
    fs::read_to_string(themes_root().join(C6_ACCEPTANCE_RELATIVE_PATH))
        .expect("read committed C6 acceptance catalog")
}

fn committed_acceptance_value() -> Value {
    serde_json::from_str(&committed_acceptance_json()).expect("parse committed acceptance JSON")
}

fn parse_value(
    value: &Value,
    source_catalog: &ThemeFixtureCatalog,
) -> Result<C6AcceptanceCatalog, CatalogError> {
    C6AcceptanceCatalog::from_json(
        &serde_json::to_string(value).expect("serialize mutated acceptance JSON"),
        source_catalog,
    )
}

#[test]
fn committed_c6_catalog_is_a_complete_expected_only_first_tranche() {
    let source_catalog = source_catalog();
    let catalog = C6AcceptanceCatalog::load(&source_catalog)
        .expect("load and validate C6 acceptance catalog");
    let expected_keys = C6_PROOF_THEMES
        .into_iter()
        .flat_map(|theme| {
            C6_PROOF_FAMILIES.into_iter().flat_map(move |family| {
                EXPECTED_OUTPUT_TARGETS
                    .into_iter()
                    .map(move |target| C6CellKey::new(theme, family, target))
            })
        })
        .collect::<BTreeSet<_>>();
    let actual_keys = catalog
        .cells()
        .map(|cell| cell.key())
        .collect::<BTreeSet<_>>();

    assert_eq!(catalog.cells().len(), C6_ACCEPTANCE_CELL_COUNT);
    assert_eq!(actual_keys, expected_keys);

    let executable_cells = catalog
        .cells()
        .filter(|cell| cell.expectation().executable().is_some())
        .collect::<Vec<_>>();
    assert_eq!(executable_cells.len(), 5);
    assert!(executable_cells.iter().all(|cell| {
        cell.key().theme() == C6ProofTheme::Brutalist && cell.key().family() == C6ProofFamily::State
    }));

    for target in EXPECTED_OUTPUT_TARGETS {
        let cell = catalog
            .cell(C6CellKey::new(
                C6ProofTheme::Brutalist,
                C6ProofFamily::State,
                target,
            ))
            .expect("Brutalist State target cell");
        let expectation = cell.expectation().executable().expect("executable tranche");
        assert_eq!(
            expectation.source_fixture_id(),
            "fixture-semantic-style-capabilities"
        );
        assert_eq!(
            expectation.required_admission(),
            C6RequiredAdmission::Portable
        );
        assert!(expectation.expected_residual_ids().is_empty());
        assert!(
            expectation
                .mechanism_requirements()
                .values()
                .all(|disposition| { *disposition == C6ExpectedMechanismDisposition::MustApply })
        );
    }

    let browser = catalog
        .cell(C6CellKey::new(
            C6ProofTheme::Brutalist,
            C6ProofFamily::State,
            merman_theme_fixtures::ExpectedOutputTarget::BrowserSvg,
        ))
        .and_then(|cell| cell.expectation().executable())
        .expect("Browser SVG expectation");
    let standalone = catalog
        .cell(C6CellKey::new(
            C6ProofTheme::Brutalist,
            C6ProofFamily::State,
            merman_theme_fixtures::ExpectedOutputTarget::StandaloneSvg,
        ))
        .and_then(|cell| cell.expectation().executable())
        .expect("standalone SVG expectation");
    assert_eq!(
        browser.required_artifact_assertion(),
        C6ArtifactAssertion::BrowserSvgDom
    );
    assert_eq!(
        standalone.required_artifact_assertion(),
        C6ArtifactAssertion::StandaloneSvgDocument
    );

    assert_eq!(
        catalog
            .cells()
            .filter(|cell| cell.expectation().readiness_blocker().is_some())
            .count(),
        40
    );
}

#[test]
fn duplicate_and_missing_cells_fail_closed() {
    let source_catalog = source_catalog();

    let mut duplicate = committed_acceptance_value();
    let duplicate_cell = duplicate["cells"][0].clone();
    duplicate["cells"]
        .as_array_mut()
        .expect("cells array")
        .push(duplicate_cell);
    assert!(matches!(
        parse_value(&duplicate, &source_catalog),
        Err(CatalogError::DuplicateC6AcceptanceCell { .. })
    ));

    let mut missing = committed_acceptance_value();
    missing["cells"].as_array_mut().expect("cells array").pop();
    assert!(matches!(
        parse_value(&missing, &source_catalog),
        Err(CatalogError::C6AcceptanceCellSetMismatch { .. })
    ));
}

#[test]
fn unknown_closed_values_fail_during_wire_parsing() {
    let source_catalog = source_catalog();

    for (field, unknown) in [
        ("theme", "future-theme"),
        ("family", "future-family"),
        ("target", "future-target"),
    ] {
        let mut value = committed_acceptance_value();
        value["cells"][0][field] = json!(unknown);
        assert!(
            matches!(
                parse_value(&value, &source_catalog),
                Err(CatalogError::InvalidC6AcceptanceJson(_))
            ),
            "{field} accepted an unknown closed value"
        );
    }
}

#[test]
fn committed_expectations_reject_runtime_result_fields() {
    let source_catalog = source_catalog();

    for field in ["status", "verified", "observed", "outcome"] {
        let mut value = committed_acceptance_value();
        value["cells"][5]["expectation"][field] = json!("must-not-be-committed");
        assert!(
            matches!(
                parse_value(&value, &source_catalog),
                Err(CatalogError::InvalidC6AcceptanceJson(_))
            ),
            "runtime field `{field}` was accepted inside an expectation"
        );

        let mut root_value = committed_acceptance_value();
        root_value[field] = json!("must-not-be-committed");
        assert!(
            matches!(
                parse_value(&root_value, &source_catalog),
                Err(CatalogError::InvalidC6AcceptanceJson(_))
            ),
            "runtime field `{field}` was accepted at the catalog root"
        );
    }
}

#[test]
fn schema_v1_rejects_false_executable_cells_and_target_aliasing() {
    let source_catalog = source_catalog();

    let mut false_executable = committed_acceptance_value();
    false_executable["cells"][0]["expectation"] =
        false_executable["cells"][5]["expectation"].clone();
    assert!(matches!(
        parse_value(&false_executable, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));

    let mut aliased_svg_target = committed_acceptance_value();
    aliased_svg_target["cells"][5]["expectation"]["requiredArtifactAssertion"] =
        json!("standalone-svg-document");
    assert!(matches!(
        parse_value(&aliased_svg_target, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));
}
