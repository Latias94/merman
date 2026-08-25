use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use merman_ascii::AsciiRenderOptions;
use merman_core::{Engine, ParseOptions};

use super::support::render_model;

const IMPORTED_FAMILY_FIXTURE_COUNTS: &[(&str, &str, usize)] = &[
    ("sequence", "sequence", 322),
    ("class", "class", 251),
    ("er", "er", 101),
];

const IMPORTED_INTENTIONAL_EMPTY_FIXTURES: &[(&str, &str)] = &[
    (
        "class/upstream_pkgtests_classdiagram_spec_004.mmd",
        "accessibility-only Class input has no terminal-visible diagram facts",
    ),
    (
        "class/upstream_pkgtests_classdiagram_spec_005.mmd",
        "multiline accessibility-only Class input has no terminal-visible diagram facts",
    ),
    (
        "er/upstream_pkgtests_diagram_orchestration_spec_030.mmd",
        "an empty ER diagram has no terminal-visible diagram facts",
    ),
];

#[test]
fn imported_common_family_fixtures_parse_and_render() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let engine = Engine::new();
    let options = AsciiRenderOptions::ascii();
    let mut failures = Vec::new();
    let mut intentional_empty_seen = BTreeSet::new();
    assert!(
        IMPORTED_INTENTIONAL_EMPTY_FIXTURES
            .iter()
            .all(|(path, reason)| !path.is_empty() && !reason.is_empty()),
        "every intentional-empty imported fixture must name a path and reason"
    );

    for (directory, expected_kind, expected_count) in IMPORTED_FAMILY_FIXTURE_COUNTS {
        let root = workspace_root.join("fixtures").join(directory);
        let mut fixtures = fs::read_dir(&root)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
            .map(|entry| entry.expect("fixture entry must be readable").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "mmd"))
            .collect::<Vec<_>>();
        fixtures.sort();

        assert_eq!(
            fixtures.len(),
            *expected_count,
            "imported fixture count drifted for {}",
            root.display()
        );

        let mut rendered_count = 0usize;
        let mut rejected_invalid_count = 0usize;
        for path in fixtures {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("fixture name must be UTF-8");
            if *directory == "sequence" && file_name == "stress_end_keyword_016.mmd" {
                assert!(
                    engine
                        .parse_diagram_for_render_model_sync(
                            &fs::read_to_string(&path).expect("fixture must be readable"),
                            ParseOptions::strict(),
                        )
                        .is_err(),
                    "the documented upstream-invalid local stress fixture must stay excluded"
                );
                rejected_invalid_count += 1;
                continue;
            }
            let fixture_key = format!("{directory}/{file_name}");
            let expected_empty = IMPORTED_INTENTIONAL_EMPTY_FIXTURES
                .iter()
                .any(|(path, _)| *path == fixture_key);

            let result = (|| -> std::result::Result<(), String> {
                let source = fs::read_to_string(&path)
                    .map_err(|error| format!("failed to read fixture: {error}"))?;
                let parsed = engine
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .map_err(|error| format!("strict parse failed: {error}"))?
                    .ok_or_else(|| "diagram type was not detected".to_string())?;
                let model = parsed.model();
                if model.kind() != *expected_kind {
                    return Err(format!(
                        "expected typed family `{expected_kind}`, got `{}`",
                        model.kind()
                    ));
                }
                let rendered = render_model(model, &options)
                    .map_err(|error| format!("ASCII render failed: {error}"))?;
                if expected_empty {
                    if !rendered.trim().is_empty() {
                        return Err(format!(
                            "document has terminal-visible output despite its intentional-empty disposition:\n{rendered}"
                        ));
                    }
                } else if rendered.trim().is_empty() {
                    return Err("ASCII renderer returned an empty document".to_string());
                }
                Ok(())
            })();

            match result {
                Ok(()) => {
                    rendered_count += 1;
                    if expected_empty {
                        intentional_empty_seen.insert(fixture_key);
                    }
                }
                Err(error) => {
                    let relative = path.strip_prefix(&workspace_root).unwrap_or(&path);
                    failures.push(format!("{}: {error}", relative.display()));
                }
            }
        }

        let expected_invalid_count = usize::from(*directory == "sequence");
        assert_eq!(
            rejected_invalid_count,
            expected_invalid_count,
            "intentional-invalid fixture count drifted for {}",
            root.display()
        );
        assert_eq!(
            rendered_count,
            expected_count - expected_invalid_count,
            "rendered fixture count drifted for {}:\n{}",
            root.display(),
            failures.join("\n")
        );
    }

    assert!(
        failures.is_empty(),
        "imported common-family admission failures:\n{}",
        failures.join("\n")
    );
    assert_eq!(
        intentional_empty_seen,
        IMPORTED_INTENTIONAL_EMPTY_FIXTURES
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect(),
        "every empty imported render must have one explicit metadata-only disposition"
    );
}
