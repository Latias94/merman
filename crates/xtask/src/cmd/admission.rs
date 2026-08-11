//! Structured diagram admission checks for alignment and compare tooling.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct FamilyAuthorityFact {
    diagram: &'static str,
    has_semantic_parser: bool,
    has_render_parser: bool,
}

pub(crate) fn primary_svg_matrix_diagrams() -> impl Iterator<Item = &'static str> {
    crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS
        .iter()
        .map(|fact| fact.diagram)
}

pub(crate) fn structured_admission_alignment_failures(fixtures_root: &Path) -> Vec<String> {
    let (families, mut failures) = canonical_family_authorities();
    let verification_diagrams = crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS
        .iter()
        .map(|fact| fact.diagram)
        .collect::<Vec<_>>();

    failures.extend(family_authority_failures(&families, &verification_diagrams));
    failures.extend(fixture_golden_failures(&families, fixtures_root));

    for fact in crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS {
        if fact.command.trim().is_empty() {
            failures.push(format!(
                "admission authorities: primary SVG family `{}` has an empty verification command",
                fact.diagram
            ));
        }

        let fixtures_dir = fixtures_root.join(fact.diagram);
        let upstream_dir = fixtures_root.join("upstream-svgs").join(fact.diagram);
        if let Err(error) = crate::cmd::load_upstream_svg_provenance(
            fact.diagram,
            &fixtures_dir,
            &upstream_dir,
            true,
        ) {
            failures.push(format!(
                "admission authorities: upstream SVG manifest for `{}` is invalid: {error}",
                fact.diagram
            ));
        }
    }

    failures
}

fn canonical_family_authorities() -> (Vec<FamilyAuthorityFact>, Vec<String>) {
    let mut families = Vec::new();
    let mut failures = Vec::new();
    let capabilities = merman_core::diagram_family_capabilities();
    let mut family_ids = Vec::new();
    let mut seen_supported = BTreeSet::new();

    for &diagram in merman_core::supported_diagrams() {
        if !seen_supported.insert(diagram) {
            failures.push(format!(
                "admission authorities: duplicate canonical public family `{diagram}`"
            ));
            continue;
        }
        family_ids.push(diagram);
    }
    let mut seen_family_ids = seen_supported;
    for diagram in primary_svg_matrix_diagrams() {
        if seen_family_ids.insert(diagram) {
            family_ids.push(diagram);
        }
    }

    for diagram in family_ids {
        let matching = capabilities
            .iter()
            .filter(|capability| {
                capability.metadata_id == Some(diagram) || capability.diagram_type == diagram
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            failures.push(format!(
                "admission authorities: canonical family `{diagram}` has no family-registry capability"
            ));
        } else {
            families.push(FamilyAuthorityFact {
                diagram,
                has_semantic_parser: matching
                    .iter()
                    .all(|capability| capability.has_semantic_parser),
                has_render_parser: matching
                    .iter()
                    .all(|capability| capability.has_render_parser),
            });
        }
    }

    (families, failures)
}

fn family_authority_failures(
    families: &[FamilyAuthorityFact],
    verification_diagrams: &[&str],
) -> Vec<String> {
    let mut failures = Vec::new();
    let mut seen_families = BTreeSet::new();
    for family in families {
        if !seen_families.insert(family.diagram) {
            failures.push(format!(
                "admission authorities: duplicate canonical family `{}`",
                family.diagram
            ));
        }
        if !family.has_semantic_parser {
            failures.push(format!(
                "admission authorities: canonical family `{}` has no semantic parser",
                family.diagram
            ));
        }
        if !family.has_render_parser {
            failures.push(format!(
                "admission authorities: canonical family `{}` has no typed render parser",
                family.diagram
            ));
        }
    }

    let mut seen_verification = BTreeSet::new();
    for &diagram in verification_diagrams {
        if !seen_verification.insert(diagram) {
            failures.push(format!(
                "admission authorities: duplicate compare-registry family `{diagram}`"
            ));
        }
        if !seen_families.contains(diagram) {
            failures.push(format!(
                "admission authorities: verification family `{diagram}` is missing from the canonical family registry"
            ));
        }
    }

    failures
}

fn fixture_golden_failures(families: &[FamilyAuthorityFact], fixtures_root: &Path) -> Vec<String> {
    let mut failures = Vec::new();

    for family in families {
        let fixtures_dir = fixtures_root.join(family.diagram);
        if !fixtures_dir.is_dir() {
            failures.push(format!(
                "admission authorities: fixture directory for `{}` does not exist: {}",
                family.diagram,
                fixtures_dir.display()
            ));
            continue;
        }
        if count_fixture_files(&fixtures_dir, |name| name.ends_with(".mmd")) == 0 {
            failures.push(format!(
                "admission authorities: canonical family `{}` has no structured Mermaid fixtures under {}",
                family.diagram,
                fixtures_dir.display()
            ));
        }
        if count_fixture_files(&fixtures_dir, |name| {
            name.ends_with(".golden.json") && !name.ends_with(".layout.golden.json")
        }) == 0
        {
            failures.push(format!(
                "admission authorities: canonical family `{}` has no semantic golden under {}",
                family.diagram,
                fixtures_dir.display()
            ));
        }
        if count_fixture_files(&fixtures_dir, |name| name.ends_with(".layout.golden.json")) == 0 {
            failures.push(format!(
                "admission authorities: canonical family `{}` has no layout golden under {}",
                family.diagram,
                fixtures_dir.display()
            ));
        }
    }

    failures
}

fn count_fixture_files(dir: &Path, matches: impl Fn(&str) -> bool) -> usize {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_file())
                .filter(|entry| entry.file_name().to_str().is_some_and(&matches))
                .count()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn structured_family_authorities_reject_missing_and_duplicate_registry_entries() {
        let families = [
            FamilyAuthorityFact {
                diagram: "flowchart",
                has_semantic_parser: true,
                has_render_parser: true,
            },
            FamilyAuthorityFact {
                diagram: "flowchart",
                has_semantic_parser: true,
                has_render_parser: true,
            },
        ];

        let failures = family_authority_failures(&families, &["flowchart", "state"]);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("duplicate canonical family `flowchart`"))
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("verification family `state` is missing"))
        );
    }

    #[test]
    fn structured_fixture_authority_requires_fixture_and_both_golden_kinds() {
        let root = temp_root("admission-fixtures");
        let fixtures_dir = root.join("flowchart");
        fs::create_dir_all(&fixtures_dir).expect("fixtures dir");
        fs::write(fixtures_dir.join("basic.mmd"), "flowchart TD\nA-->B\n").expect("fixture");
        let families = [FamilyAuthorityFact {
            diagram: "flowchart",
            has_semantic_parser: true,
            has_render_parser: true,
        }];

        let missing_goldens = fixture_golden_failures(&families, &root);
        assert!(
            missing_goldens
                .iter()
                .any(|failure| failure.contains("no semantic golden"))
        );
        assert!(
            missing_goldens
                .iter()
                .any(|failure| failure.contains("no layout golden"))
        );

        fs::write(fixtures_dir.join("basic.golden.json"), "{}\n").expect("semantic golden");
        fs::write(fixtures_dir.join("basic.layout.golden.json"), "{}\n").expect("layout golden");
        assert!(fixture_golden_failures(&families, &root).is_empty());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn primary_svg_matrix_is_a_direct_compare_registry_projection() {
        let primary = primary_svg_matrix_diagrams().collect::<Vec<_>>();
        let compare = crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS
            .iter()
            .map(|fact| fact.diagram)
            .collect::<Vec<_>>();

        assert_eq!(primary, compare);
    }

    #[test]
    fn native_cross_family_corpus_matches_the_canonical_family_registry() {
        let corpus_path = crate::cmd::workspace_root().join("tools/bench/corpus.json");
        let corpus: serde_json::Value =
            serde_json::from_slice(&fs::read(&corpus_path).unwrap_or_else(|error| {
                panic!("failed to read {}: {error}", corpus_path.display())
            }))
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", corpus_path.display()));
        let fixtures = corpus["fixtures"]
            .as_array()
            .expect("performance corpus fixtures must be an array");
        let cross_family_fixtures = fixtures
            .iter()
            .filter(|fixture| {
                fixture["suites"]
                    .as_array()
                    .is_some_and(|suites| suites.iter().any(|suite| suite == "cross_family"))
            })
            .collect::<Vec<_>>();
        let cross_family = cross_family_fixtures
            .iter()
            .map(|fixture| {
                fixture["family"]
                    .as_str()
                    .expect("cross-family fixture must declare a family")
            })
            .collect::<BTreeSet<_>>();
        let canonical = merman_core::supported_diagrams()
            .iter()
            .copied()
            .chain(primary_svg_matrix_diagrams())
            .collect::<BTreeSet<_>>();

        assert_eq!(cross_family, canonical);
        assert_eq!(cross_family_fixtures.len(), canonical.len());

        let engine = merman_core::Engine::new();
        for fixture in cross_family_fixtures {
            let declared = fixture["family"]
                .as_str()
                .expect("cross-family fixture must declare a family");
            let source = fixture["source"]
                .as_str()
                .expect("cross-family fixture must declare a source");
            let source_path = crate::cmd::workspace_root().join(source);
            let input = fs::read_to_string(&source_path).unwrap_or_else(|error| {
                panic!("failed to read {}: {error}", source_path.display())
            });
            let metadata = engine.parse_metadata_sync(&input).unwrap_or_else(|error| {
                panic!(
                    "failed to detect {declared} fixture {}: {error}",
                    source_path.display()
                )
            });
            let detected = merman_core::diagram_type_metadata_id(&metadata.diagram_type)
                .unwrap_or(metadata.diagram_type.as_str());
            assert_eq!(detected, declared);
        }
    }

    #[test]
    fn current_family_and_compare_registries_are_consistent() {
        let (families, registry_failures) = canonical_family_authorities();
        let verification_diagrams = crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS
            .iter()
            .map(|fact| fact.diagram)
            .collect::<Vec<_>>();

        assert!(
            registry_failures.is_empty(),
            "canonical registry failures:\n{}",
            registry_failures.join("\n")
        );
        let failures = family_authority_failures(&families, &verification_diagrams);
        assert!(
            failures.is_empty(),
            "structured family authority failures:\n{}",
            failures.join("\n")
        );
        assert!(
            crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS
                .iter()
                .all(|fact| !fact.command.trim().is_empty())
        );
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("merman-{label}-{}-{nonce}", std::process::id()))
    }
}
