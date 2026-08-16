use merman_theme_fixtures::{
    C6_ACCEPTANCE_CELL_COUNT, C6_ACCEPTANCE_RELATIVE_PATH, C6_ACCEPTANCE_SCHEMA_VERSION,
    C6_NATIVE_OUTPUT_TARGETS, C6_PROOF_FAMILIES, C6_PROOF_THEMES, C6AcceptanceCatalog, C6CellKey,
    C6ExpectedMechanismDisposition, C6ProofFamily, C6ProofTheme, C6ReadinessBlocker, CatalogError,
    ExpectedOutputTarget, ReferenceThemeMechanism, ThemeFixtureCatalog,
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

fn expected_mechanisms(
    theme: C6ProofTheme,
    family: C6ProofFamily,
) -> BTreeSet<ReferenceThemeMechanism> {
    use ReferenceThemeMechanism as Mechanism;

    let mechanisms: &[Mechanism] = match (theme, family) {
        (C6ProofTheme::Brutalist, C6ProofFamily::Flowchart) => &[
            Mechanism::NthChildSelector,
            Mechanism::RoundedCorners,
            Mechanism::StrokeStyling,
        ],
        (C6ProofTheme::Brutalist, C6ProofFamily::State) => &[
            Mechanism::CanvasSolid,
            Mechanism::CssFilter,
            Mechanism::FontStack,
            Mechanism::NthChildSelector,
            Mechanism::RoundedCorners,
            Mechanism::StrokeStyling,
            Mechanism::ThemeVariables,
        ],
        (C6ProofTheme::Brutalist, C6ProofFamily::Sequence) => &[Mechanism::ThemeVariables],
        (C6ProofTheme::Spotless, C6ProofFamily::Flowchart) => &[
            Mechanism::CanvasGradient,
            Mechanism::CanvasPattern,
            Mechanism::DashArray,
            Mechanism::FontStack,
            Mechanism::RoundedCorners,
            Mechanism::StrokeStyling,
        ],
        (C6ProofTheme::Spotless, C6ProofFamily::State) => &[
            Mechanism::CanvasGradient,
            Mechanism::CanvasLayering,
            Mechanism::CanvasPattern,
            Mechanism::CanvasSolid,
            Mechanism::CssLetterSpacing,
            Mechanism::CssTextTransform,
            Mechanism::FontStack,
            Mechanism::ThemeVariables,
        ],
        (C6ProofTheme::Spotless, C6ProofFamily::Sequence) => &[
            Mechanism::FontStack,
            Mechanism::StrokeStyling,
            Mechanism::ThemeVariables,
        ],
        (C6ProofTheme::Cyberpunk, C6ProofFamily::Flowchart) => &[
            Mechanism::CanvasBlend,
            Mechanism::CanvasGradient,
            Mechanism::CanvasLayering,
            Mechanism::CanvasPattern,
            Mechanism::StrokeStyling,
        ],
        (C6ProofTheme::Cyberpunk, C6ProofFamily::State) => &[
            Mechanism::CanvasSolid,
            Mechanism::CssFilter,
            Mechanism::FontStack,
            Mechanism::RoundedCorners,
            Mechanism::StrokeStyling,
            Mechanism::ThemeVariables,
        ],
        (C6ProofTheme::Cyberpunk, C6ProofFamily::Sequence) => &[
            Mechanism::CanvasSolid,
            Mechanism::FontStack,
            Mechanism::StrokeStyling,
            Mechanism::ThemeVariables,
        ],
    };
    mechanisms.iter().copied().collect()
}

#[test]
fn committed_catalog_is_the_exact_native_c6a_ledger() {
    let source_catalog = source_catalog();
    let catalog = C6AcceptanceCatalog::load(&source_catalog)
        .expect("load and validate C6 acceptance catalog");
    let expected_keys = C6_PROOF_THEMES
        .into_iter()
        .flat_map(|theme| {
            C6_PROOF_FAMILIES.into_iter().flat_map(move |family| {
                C6_NATIVE_OUTPUT_TARGETS
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

    assert_eq!(C6_ACCEPTANCE_RELATIVE_PATH, "acceptance/c6-v3.json");
    assert_eq!(catalog.schema_version(), C6_ACCEPTANCE_SCHEMA_VERSION);
    assert_eq!(C6_ACCEPTANCE_SCHEMA_VERSION, 3);
    assert_eq!(
        catalog.specification().cells().len(),
        C6_ACCEPTANCE_CELL_COUNT
    );
    assert_eq!(C6_ACCEPTANCE_CELL_COUNT, 18);
    assert_eq!(catalog.enforced_tranche().cells().len(), 12);
    assert_eq!(catalog.enforced_tranche().deferred_cells().count(), 6);
    assert_eq!(actual_keys, expected_keys);
    assert_ne!(catalog.manifest_digest(), &[0; 32]);

    let mut semantic_assertion_ids = BTreeSet::new();
    for cell in catalog.specification().cells() {
        let expectation = cell.expectation();
        assert_eq!(
            expectation
                .mechanism_requirements()
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            expected_mechanisms(cell.key().theme(), cell.key().family())
        );
        assert!(expectation.expected_residual_ids().is_empty());
        assert!(
            expectation
                .mechanism_requirements()
                .values()
                .all(|disposition| { *disposition == C6ExpectedMechanismDisposition::MustApply })
        );
        assert!(semantic_assertion_ids.insert(expectation.semantic_assertion_id()));
    }

    for target in C6_NATIVE_OUTPUT_TARGETS {
        let key = C6CellKey::new(C6ProofTheme::Brutalist, C6ProofFamily::Flowchart, target);
        assert_eq!(
            catalog
                .enforced_tranche()
                .cell(key)
                .map(|cell| cell.source_fixture_id()),
            Some("fixture-ordinal-palette")
        );

        let key = C6CellKey::new(C6ProofTheme::Brutalist, C6ProofFamily::State, target);
        assert_eq!(
            catalog
                .enforced_tranche()
                .cell(key)
                .map(|cell| cell.source_fixture_id()),
            Some("fixture-c6-brutalist-state")
        );

        let key = C6CellKey::new(C6ProofTheme::Brutalist, C6ProofFamily::Sequence, target);
        assert_eq!(
            catalog
                .enforced_tranche()
                .cell(key)
                .map(|cell| cell.source_fixture_id()),
            Some("fixture-sequence-proof")
        );

        for (theme, fixture_id) in [
            (C6ProofTheme::Spotless, "fixture-c6-spotless-state"),
            (C6ProofTheme::Cyberpunk, "fixture-c6-cyberpunk-state"),
        ] {
            let key = C6CellKey::new(theme, C6ProofFamily::State, target);
            assert_eq!(
                catalog
                    .enforced_tranche()
                    .cell(key)
                    .map(|cell| cell.source_fixture_id()),
                Some(fixture_id)
            );
        }

        let key = C6CellKey::new(C6ProofTheme::Spotless, C6ProofFamily::Flowchart, target);
        assert_eq!(
            catalog
                .enforced_tranche()
                .cell(key)
                .map(|cell| cell.source_fixture_id()),
            Some("fixture-c6-spotless-flowchart")
        );
    }
    for theme in C6_PROOF_THEMES {
        for family in C6_PROOF_FAMILIES {
            for target in C6_NATIVE_OUTPUT_TARGETS {
                let key = C6CellKey::new(theme, family, target);
                if catalog.enforced_tranche().cell(key).is_some() {
                    continue;
                }
                assert_eq!(
                    catalog.enforced_tranche().readiness_blocker(key),
                    Some(C6ReadinessBlocker::ThemeSliceIncomplete)
                );
            }
        }
    }
}

#[test]
fn schema_and_exact_cell_set_fail_closed() {
    let source_catalog = source_catalog();

    let mut old_schema = committed_acceptance_value();
    old_schema["schemaVersion"] = json!(2);
    assert!(matches!(
        parse_value(&old_schema, &source_catalog),
        Err(CatalogError::UnsupportedC6AcceptanceSchemaVersion(2))
    ));

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

    let mut browser = committed_acceptance_value();
    browser["cells"][0]["target"] = json!("browser-svg");
    assert!(matches!(
        parse_value(&browser, &source_catalog),
        Err(CatalogError::C6AcceptanceCellSetMismatch { .. })
    ));
}

#[test]
fn mechanism_subsets_are_allowed_but_empty_or_foreign_mechanisms_are_rejected() {
    let source_catalog = source_catalog();
    parse_value(&committed_acceptance_value(), &source_catalog)
        .expect("family-scoped mechanism subsets are valid");

    let mut empty = committed_acceptance_value();
    cell_mut(&mut empty, "brutalist", "flowchart", "standalone-svg")["expectation"]["mechanismRequirements"] =
        json!({});
    assert!(matches!(
        parse_value(&empty, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));

    let mut foreign = committed_acceptance_value();
    cell_mut(&mut foreign, "brutalist", "flowchart", "standalone-svg")["expectation"]["mechanismRequirements"]
        ["canvas-blend"] = json!("must-apply");
    match parse_value(&foreign, &source_catalog) {
        Err(CatalogError::InvalidC6AcceptanceCell { reason, .. }) => {
            assert!(reason.contains("absent from the reference theme"));
        }
        other => panic!("foreign mechanism was accepted: {other:?}"),
    }
}

#[test]
fn semantic_assertion_ids_are_closed_unique_manifest_identity() {
    let source_catalog = source_catalog();
    let original = parse_value(&committed_acceptance_value(), &source_catalog)
        .expect("parse committed catalog");

    let mut reordered = committed_acceptance_value();
    reordered["cells"]
        .as_array_mut()
        .expect("cells array")
        .reverse();
    let reordered = parse_value(&reordered, &source_catalog).expect("parse reordered catalog");
    assert_eq!(original.manifest_digest(), reordered.manifest_digest());

    let mut changed = committed_acceptance_value();
    cell_mut(&mut changed, "brutalist", "state", "png")["expectation"]["semanticAssertionId"] =
        json!("brutalist-state-png-v2");
    let changed = parse_value(&changed, &source_catalog).expect("parse changed semantic contract");
    assert_ne!(original.manifest_digest(), changed.manifest_digest());

    let mut duplicate = committed_acceptance_value();
    cell_mut(&mut duplicate, "brutalist", "state", "png")["expectation"]["semanticAssertionId"] =
        json!("brutalist-state-standalone-svg-v1");
    assert!(matches!(
        parse_value(&duplicate, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));

    let mut invalid = committed_acceptance_value();
    cell_mut(&mut invalid, "brutalist", "state", "png")["expectation"]["semanticAssertionId"] =
        json!("Brutalist State PNG");
    assert!(matches!(
        parse_value(&invalid, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceCell { .. })
    ));
}

#[test]
fn unknown_and_runtime_observation_fields_fail_during_wire_parsing() {
    let source_catalog = source_catalog();

    for (field, unknown) in [
        ("theme", "future-theme"),
        ("family", "future-family"),
        ("target", "future-target"),
    ] {
        let mut value = committed_acceptance_value();
        value["cells"][0][field] = json!(unknown);
        assert!(matches!(
            parse_value(&value, &source_catalog),
            Err(CatalogError::InvalidC6AcceptanceJson(_))
        ));
    }

    for field in ["status", "verified", "observed", "outcome"] {
        let mut value = committed_acceptance_value();
        value["cells"][0]["expectation"][field] = json!("must-not-be-committed");
        assert!(matches!(
            parse_value(&value, &source_catalog),
            Err(CatalogError::InvalidC6AcceptanceJson(_))
        ));

        let mut value = committed_acceptance_value();
        value["cells"][0]["enforcement"][field] = json!("must-not-be-committed");
        assert!(matches!(
            parse_value(&value, &source_catalog),
            Err(CatalogError::InvalidC6AcceptanceJson(_))
        ));

        let mut value = committed_acceptance_value();
        value[field] = json!("must-not-be-committed");
        assert!(matches!(
            parse_value(&value, &source_catalog),
            Err(CatalogError::InvalidC6AcceptanceJson(_))
        ));
    }

    for field in [
        "requiredAdmission",
        "requiredFontSource",
        "requiredArtifactAssertion",
    ] {
        let mut value = committed_acceptance_value();
        value["cells"][0]["expectation"][field] = json!("removed-v2-field");
        assert!(matches!(
            parse_value(&value, &source_catalog),
            Err(CatalogError::InvalidC6AcceptanceJson(_))
        ));
    }

    let mut removed_blocker = committed_acceptance_value();
    removed_blocker["cells"][0]["enforcement"]["blocker"] = json!("runtime-runner-missing");
    assert!(matches!(
        parse_value(&removed_blocker, &source_catalog),
        Err(CatalogError::InvalidC6AcceptanceJson(_))
    ));
}

#[test]
fn enforcement_and_portable_expectation_invariants_fail_closed() {
    let source_catalog = source_catalog();

    let mut residual = committed_acceptance_value();
    let expectation = &mut cell_mut(&mut residual, "brutalist", "state", "png")["expectation"];
    expectation["mechanismRequirements"]["css-filter"] = json!("must-remain-residual");
    expectation["expectedResidualIds"] = json!(["residual-filter"]);
    assert!(matches!(
        parse_value(&residual, &source_catalog),
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

    let mut wrong_family = committed_acceptance_value();
    cell_mut(&mut wrong_family, "brutalist", "sequence", "standalone-svg")["enforcement"] = json!({
        "kind": "enforced",
        "sourceFixtureId": "fixture-token-baseline"
    });
    match parse_value(&wrong_family, &source_catalog) {
        Err(CatalogError::InvalidC6AcceptanceCell { reason, .. }) => {
            assert!(reason.contains("does not match the proof family"));
        }
        other => panic!("wrong-family fixture was accepted: {other:?}"),
    }

    let mut incomplete_input = committed_acceptance_value();
    cell_mut(
        &mut incomplete_input,
        "brutalist",
        "flowchart",
        "standalone-svg",
    )["enforcement"] = json!({
        "kind": "enforced",
        "sourceFixtureId": "fixture-token-baseline"
    });
    match parse_value(&incomplete_input, &source_catalog) {
        Err(CatalogError::InvalidC6AcceptanceCell { reason, .. }) => {
            assert!(reason.contains("not covered by the typed theme input"));
        }
        other => panic!("incomplete theme input was accepted: {other:?}"),
    }
}

#[test]
fn mechanism_requirements_remain_target_specific_data() {
    let source_catalog = source_catalog();
    let mut value = committed_acceptance_value();
    cell_mut(&mut value, "brutalist", "state", "png")["expectation"]["mechanismRequirements"] = json!({
        "stroke-styling": "must-apply",
        "theme-variables": "must-apply"
    });

    let catalog = parse_value(&value, &source_catalog).expect("target-local subset remains valid");
    let svg = catalog
        .cell(C6CellKey::new(
            C6ProofTheme::Brutalist,
            C6ProofFamily::State,
            ExpectedOutputTarget::StandaloneSvg,
        ))
        .expect("SVG cell");
    let png = catalog
        .cell(C6CellKey::new(
            C6ProofTheme::Brutalist,
            C6ProofFamily::State,
            ExpectedOutputTarget::Png,
        ))
        .expect("PNG cell");
    assert_ne!(
        svg.expectation().mechanism_requirements(),
        png.expectation().mechanism_requirements()
    );
    assert_eq!(
        png.expectation().mechanism_requirements(),
        &BTreeMap::from([
            (
                ReferenceThemeMechanism::StrokeStyling,
                C6ExpectedMechanismDisposition::MustApply,
            ),
            (
                ReferenceThemeMechanism::ThemeVariables,
                C6ExpectedMechanismDisposition::MustApply,
            ),
        ])
    );
}
