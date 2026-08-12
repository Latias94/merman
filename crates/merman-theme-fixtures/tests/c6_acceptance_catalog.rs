use merman_theme_fixtures::{
    C6_ACCEPTANCE_CELL_COUNT, C6_ACCEPTANCE_RELATIVE_PATH, C6_PROOF_FAMILIES, C6_PROOF_THEMES,
    C6AcceptanceCatalog, C6AcceptanceEvaluationError, C6ArtifactAssertion, C6CellKey,
    C6ExpectationField, C6ExpectedFontSource, C6ExpectedMechanismDisposition,
    C6ObservedArtifactStatus, C6ObservedCellReport, C6ObservedMechanismDisposition,
    C6ObservedReport, C6ObservedReportError, C6ProofFamily, C6ProofTheme, C6ReadinessBlocker,
    C6RequiredAdmission, CatalogError, EXPECTED_OUTPUT_TARGETS, ExpectedOutputTarget,
    ThemeFixtureCatalog,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
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

fn cell_mut<'a>(value: &'a mut Value, theme: &str, family: &str, target: &str) -> &'a mut Value {
    value["cells"]
        .as_array_mut()
        .expect("cells array")
        .iter_mut()
        .find(|cell| cell["theme"] == theme && cell["family"] == family && cell["target"] == target)
        .expect("requested acceptance cell")
}

fn defer_brutalist_state_native_tranche(value: &mut Value) {
    for target in ["standalone-svg", "png", "jpeg", "pdf"] {
        cell_mut(value, "brutalist", "state", target)["enforcement"] = json!({
            "kind": "deferred",
            "blocker": "runtime-runner-missing"
        });
    }
}

#[test]
fn committed_catalog_separates_complete_spec_from_current_tranche() {
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
        .specification()
        .cells()
        .map(|cell| cell.key())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        catalog.specification().cells().len(),
        C6_ACCEPTANCE_CELL_COUNT
    );
    assert_eq!(catalog.enforced_tranche().cells().len(), 4);
    assert_eq!(actual_keys, expected_keys);

    for cell in catalog.specification().cells() {
        let expectation = cell.expectation();
        assert!(!expectation.mechanism_requirements().is_empty());
        assert!(expectation.expected_residual_ids().is_empty());
        assert_eq!(
            expectation.required_font_source(),
            C6ExpectedFontSource::Embedded
        );
        assert_eq!(
            expectation.required_admission(),
            C6RequiredAdmission::Portable
        );
        assert!(
            expectation
                .mechanism_requirements()
                .values()
                .all(|disposition| { *disposition == C6ExpectedMechanismDisposition::MustApply })
        );
    }

    assert_eq!(catalog.enforced_tranche().deferred_cells().count(), 41);

    for theme in C6_PROOF_THEMES {
        for target in EXPECTED_OUTPUT_TARGETS {
            let sequence = catalog
                .specification()
                .cell(C6CellKey::new(theme, C6ProofFamily::Sequence, target))
                .expect("Sequence final expectation");
            assert!(!sequence.expectation().mechanism_requirements().is_empty());
            let expected_blocker = if theme == C6ProofTheme::Brutalist {
                C6ReadinessBlocker::FamilyAdapterIncomplete
            } else {
                C6ReadinessBlocker::ThemeSliceIncomplete
            };
            assert_eq!(
                catalog.enforced_tranche().readiness_blocker(sequence.key()),
                Some(expected_blocker)
            );
        }
    }

    for target in EXPECTED_OUTPUT_TARGETS {
        let key = C6CellKey::new(C6ProofTheme::Brutalist, C6ProofFamily::State, target);
        if target == ExpectedOutputTarget::BrowserSvg {
            assert_eq!(
                catalog.enforced_tranche().readiness_blocker(key),
                Some(C6ReadinessBlocker::RuntimeRunnerMissing)
            );
        } else {
            assert_eq!(
                catalog
                    .enforced_tranche()
                    .cell(key)
                    .map(|cell| cell.source_fixture_id()),
                Some("fixture-c6-brutalist-state")
            );
        }
    }

    let browser = catalog
        .specification()
        .cell(C6CellKey::new(
            C6ProofTheme::Brutalist,
            C6ProofFamily::State,
            ExpectedOutputTarget::BrowserSvg,
        ))
        .expect("Browser SVG expectation");
    let standalone = catalog
        .specification()
        .cell(C6CellKey::new(
            C6ProofTheme::Brutalist,
            C6ProofFamily::State,
            ExpectedOutputTarget::StandaloneSvg,
        ))
        .expect("standalone SVG expectation");
    assert_eq!(
        browser.expectation().required_artifact_assertion(),
        C6ArtifactAssertion::BrowserSvgDom
    );
    assert_eq!(
        standalone.expectation().required_artifact_assertion(),
        C6ArtifactAssertion::StandaloneSvgDocument
    );
}

#[test]
fn parser_accepts_a_tranche_change_without_theme_or_family_whitelists() {
    let source_catalog = source_catalog();
    let mut value = committed_acceptance_value();
    cell_mut(&mut value, "spotless", "flowchart", "standalone-svg")["enforcement"] = json!({
        "kind": "enforced",
        "sourceFixtureId": "fixture-token-baseline"
    });

    let catalog = parse_value(&value, &source_catalog).expect("expanded tranche is data-driven");
    let key = C6CellKey::new(
        C6ProofTheme::Spotless,
        C6ProofFamily::Flowchart,
        ExpectedOutputTarget::StandaloneSvg,
    );
    assert_eq!(
        catalog
            .enforced_tranche()
            .cell(key)
            .map(|cell| cell.source_fixture_id()),
        Some("fixture-token-baseline")
    );
    assert_eq!(catalog.enforced_tranche().cells().len(), 5);
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
fn committed_catalog_rejects_runtime_observation_fields() {
    let source_catalog = source_catalog();

    for field in ["status", "verified", "observed", "outcome"] {
        let mut expectation = committed_acceptance_value();
        expectation["cells"][0]["expectation"][field] = json!("must-not-be-committed");
        assert!(matches!(
            parse_value(&expectation, &source_catalog),
            Err(CatalogError::InvalidC6AcceptanceJson(_))
        ));

        let mut enforcement = committed_acceptance_value();
        enforcement["cells"][0]["enforcement"][field] = json!("must-not-be-committed");
        assert!(matches!(
            parse_value(&enforcement, &source_catalog),
            Err(CatalogError::InvalidC6AcceptanceJson(_))
        ));

        let mut root = committed_acceptance_value();
        root[field] = json!("must-not-be-committed");
        assert!(matches!(
            parse_value(&root, &source_catalog),
            Err(CatalogError::InvalidC6AcceptanceJson(_))
        ));
    }
}

#[test]
fn expectation_and_enforcement_invariants_fail_closed() {
    let source_catalog = source_catalog();

    let mut aliased_svg_target = committed_acceptance_value();
    cell_mut(&mut aliased_svg_target, "brutalist", "state", "browser-svg")["expectation"]["requiredArtifactAssertion"] =
        json!("standalone-svg-document");
    assert!(matches!(
        parse_value(&aliased_svg_target, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));

    let mut system_font_portable = committed_acceptance_value();
    cell_mut(
        &mut system_font_portable,
        "brutalist",
        "state",
        "browser-svg",
    )["expectation"]["requiredFontSource"] = json!("system");
    assert!(matches!(
        parse_value(&system_font_portable, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));

    let mut incomplete_mechanisms = committed_acceptance_value();
    cell_mut(
        &mut incomplete_mechanisms,
        "cyberpunk",
        "sequence",
        "pdf",
    )["expectation"]["mechanismRequirements"]
        .as_object_mut()
        .expect("mechanism map")
        .remove("canvas-blend");
    assert!(matches!(
        parse_value(&incomplete_mechanisms, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));

    let mut unknown_source_fixture = committed_acceptance_value();
    cell_mut(&mut unknown_source_fixture, "brutalist", "state", "png")["enforcement"] = json!({
        "kind": "enforced",
        "sourceFixtureId": "fixture-does-not-exist"
    });
    assert!(matches!(
        parse_value(&unknown_source_fixture, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));

    let mut sequence_with_wrong_family = committed_acceptance_value();
    cell_mut(
        &mut sequence_with_wrong_family,
        "brutalist",
        "sequence",
        "standalone-svg",
    )["enforcement"] = json!({
        "kind": "enforced",
        "sourceFixtureId": "fixture-token-baseline"
    });
    match parse_value(&sequence_with_wrong_family, &source_catalog) {
        Err(CatalogError::InvalidC6AcceptanceCell { reason, .. }) => {
            assert!(reason.contains("does not match the proof family"));
        }
        other => panic!("Sequence enforcement accepted a Flowchart fixture: {other:?}"),
    }
}

#[test]
fn an_empty_tranche_cannot_vacuously_pass_acceptance() {
    let source_catalog = source_catalog();
    let mut value = committed_acceptance_value();
    defer_brutalist_state_native_tranche(&mut value);
    let catalog = parse_value(&value, &source_catalog).expect("all-deferred test catalog");
    let report = C6ObservedReport::from_enforced_cells(
        catalog.enforced_tranche(),
        std::iter::empty::<C6ObservedCellReport>(),
    )
    .expect("empty raw report matches the empty tranche");
    assert!(matches!(
        report.evaluate(catalog.specification(), catalog.enforced_tranche()),
        Err(C6AcceptanceEvaluationError::EmptyTranche)
    ));
}

#[test]
fn raw_observations_preserve_failures_but_acceptance_evaluation_is_exact() {
    let source_catalog = source_catalog();
    let mut value = committed_acceptance_value();
    defer_brutalist_state_native_tranche(&mut value);
    cell_mut(&mut value, "brutalist", "state", "browser-svg")["enforcement"] = json!({
        "kind": "enforced",
        "sourceFixtureId": "fixture-semantic-style-capabilities"
    });
    let catalog = parse_value(&value, &source_catalog).expect("one-cell staged tranche");
    let key = C6CellKey::new(
        C6ProofTheme::Brutalist,
        C6ProofFamily::State,
        ExpectedOutputTarget::BrowserSvg,
    );
    let expectation = catalog
        .specification()
        .cell(key)
        .expect("staged expectation")
        .expectation();
    let expected_mechanisms = expectation
        .mechanism_requirements()
        .iter()
        .map(|(mechanism, disposition)| {
            let observed = match disposition {
                C6ExpectedMechanismDisposition::MustApply => {
                    C6ObservedMechanismDisposition::Applied
                }
                C6ExpectedMechanismDisposition::MustRemainResidual => {
                    C6ObservedMechanismDisposition::Residual
                }
                C6ExpectedMechanismDisposition::MustReject => {
                    C6ObservedMechanismDisposition::Rejected
                }
                C6ExpectedMechanismDisposition::NotApplicable => {
                    C6ObservedMechanismDisposition::NotApplicable
                }
            };
            (*mechanism, observed)
        })
        .collect::<BTreeMap<_, _>>();
    let build_observation =
        |mechanisms, residual_ids, font_source, admission, artifact_assertion, artifact_status| {
            C6ObservedCellReport::new(
                key,
                mechanisms,
                residual_ids,
                font_source,
                admission,
                artifact_assertion,
                artifact_status,
            )
        };
    let evaluate = |observation| {
        let report =
            C6ObservedReport::from_enforced_cells(catalog.enforced_tranche(), [observation])
                .expect("raw observation is retained");
        report.evaluate(catalog.specification(), catalog.enforced_tranche())
    };

    let passing = build_observation(
        expected_mechanisms.clone(),
        BTreeSet::new(),
        C6ExpectedFontSource::Embedded,
        C6RequiredAdmission::Portable,
        C6ArtifactAssertion::BrowserSvgDom,
        C6ObservedArtifactStatus::Passed,
    );
    assert_eq!(
        evaluate(passing.clone())
            .expect("matching observation passes")
            .verified_cell_count(),
        1
    );

    let assert_field = |result, expected_field| {
        assert!(matches!(
            result,
            Err(C6AcceptanceEvaluationError::ExpectationMismatch { field, .. })
                if field == expected_field
        ));
    };

    let mut wrong_mechanisms = expected_mechanisms.clone();
    let first_mechanism = *wrong_mechanisms.keys().next().expect("proof mechanism");
    wrong_mechanisms.insert(first_mechanism, C6ObservedMechanismDisposition::Residual);
    assert_field(
        evaluate(build_observation(
            wrong_mechanisms,
            BTreeSet::new(),
            C6ExpectedFontSource::Embedded,
            C6RequiredAdmission::Portable,
            C6ArtifactAssertion::BrowserSvgDom,
            C6ObservedArtifactStatus::Passed,
        )),
        C6ExpectationField::MechanismDisposition,
    );

    assert_field(
        evaluate(build_observation(
            expected_mechanisms.clone(),
            ["unexpected-residual".to_string()].into_iter().collect(),
            C6ExpectedFontSource::Embedded,
            C6RequiredAdmission::Portable,
            C6ArtifactAssertion::BrowserSvgDom,
            C6ObservedArtifactStatus::Passed,
        )),
        C6ExpectationField::ResidualIds,
    );
    assert_field(
        evaluate(build_observation(
            expected_mechanisms.clone(),
            BTreeSet::new(),
            C6ExpectedFontSource::System,
            C6RequiredAdmission::Portable,
            C6ArtifactAssertion::BrowserSvgDom,
            C6ObservedArtifactStatus::Passed,
        )),
        C6ExpectationField::FontSource,
    );
    assert_field(
        evaluate(build_observation(
            expected_mechanisms.clone(),
            BTreeSet::new(),
            C6ExpectedFontSource::Embedded,
            C6RequiredAdmission::HostDependent,
            C6ArtifactAssertion::BrowserSvgDom,
            C6ObservedArtifactStatus::Passed,
        )),
        C6ExpectationField::Admission,
    );
    assert_field(
        evaluate(build_observation(
            expected_mechanisms.clone(),
            BTreeSet::new(),
            C6ExpectedFontSource::Embedded,
            C6RequiredAdmission::Portable,
            C6ArtifactAssertion::StandaloneSvgDocument,
            C6ObservedArtifactStatus::Passed,
        )),
        C6ExpectationField::ArtifactAssertion,
    );
    assert_field(
        evaluate(build_observation(
            expected_mechanisms,
            BTreeSet::new(),
            C6ExpectedFontSource::Embedded,
            C6RequiredAdmission::Portable,
            C6ArtifactAssertion::BrowserSvgDom,
            C6ObservedArtifactStatus::Failed,
        )),
        C6ExpectationField::ArtifactStatus,
    );

    assert!(matches!(
        C6ObservedReport::from_enforced_cells(
            catalog.enforced_tranche(),
            std::iter::empty::<C6ObservedCellReport>(),
        ),
        Err(C6ObservedReportError::CellSetMismatch { .. })
    ));
    assert!(matches!(
        C6ObservedReport::from_enforced_cells(
            catalog.enforced_tranche(),
            [passing.clone(), passing]
        ),
        Err(C6ObservedReportError::DuplicateCell { .. })
    ));
}
