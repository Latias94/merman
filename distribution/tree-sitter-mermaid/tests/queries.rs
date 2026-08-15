use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;
use tree_sitter_mermaid::{LANGUAGE, QUERY_PROFILES};

const PROFILES: [&str; 4] = ["portable", "neovim", "helix", "zed"];
const SURFACES: [&str; 9] = [
    "highlights",
    "folds",
    "indents",
    "injections",
    "locals",
    "tags",
    "brackets",
    "outline",
    "textobjects",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Support {
    families: Vec<SupportFamily>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupportFamily {
    public_id: String,
    query_applicability: BTreeMap<String, BTreeMap<String, SupportCell>>,
}

#[derive(Debug, Deserialize)]
struct SupportCell {
    status: String,
    #[serde(default)]
    evidence: Vec<String>,
    rationale: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Matrix {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    profile: String,
    surfaces: Option<Vec<String>>,
    families: Vec<MatrixFamily>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatrixFamily {
    public_id: String,
    surfaces: BTreeMap<String, MatrixCell>,
}

#[derive(Debug, Deserialize)]
struct MatrixCell {
    status: String,
    query: Option<String>,
    rationale: Option<String>,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

#[test]
fn query_profiles_compile_and_match_the_complete_applicability_contract() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let language: tree_sitter::Language = LANGUAGE.into();
    let packaged = QUERY_PROFILES
        .iter()
        .map(|profile| {
            tree_sitter::Query::new(&language, profile.source).unwrap_or_else(|error| {
                panic!(
                    "{}/{} query does not compile: {error}",
                    profile.profile, profile.surface
                )
            });
            assert_eq!(
                profile.path,
                format!("queries/{}/{}.scm", profile.profile, profile.surface)
            );
            (profile.profile.to_string(), profile.surface.to_string())
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(packaged.len(), QUERY_PROFILES.len());

    let support: Support = read_json(&package.join("metadata/support.json"));
    assert_eq!(support.families.len(), 35);
    let support_by_id = support
        .families
        .into_iter()
        .map(|family| (family.public_id.clone(), family))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(support_by_id.len(), 35);

    let expected_surfaces = SURFACES
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let mut asserted_profiles = BTreeSet::new();
    for profile in PROFILES {
        let matrix_path = package
            .join("test/queries")
            .join(profile)
            .join("applicability.json");
        let matrix: Matrix = read_json(&matrix_path);
        assert_eq!(matrix.schema_version, 1, "{profile}");
        assert_eq!(matrix.profile, profile);
        if let Some(surfaces) = matrix.surfaces {
            assert_eq!(
                surfaces.into_iter().collect::<BTreeSet<_>>(),
                expected_surfaces
            );
        }
        assert_eq!(matrix.families.len(), 35, "{profile}");
        let mut family_ids = BTreeSet::new();

        for family in matrix.families {
            assert!(family_ids.insert(family.public_id.clone()), "{profile}");
            assert_eq!(
                family.surfaces.keys().cloned().collect::<BTreeSet<_>>(),
                expected_surfaces,
                "{profile}/{}",
                family.public_id
            );
            let support = support_by_id
                .get(&family.public_id)
                .unwrap_or_else(|| panic!("{profile}: unknown family {}", family.public_id));
            let support_surfaces = support
                .query_applicability
                .get(profile)
                .unwrap_or_else(|| panic!("{} lacks {profile}", family.public_id));

            for (surface, cell) in family.surfaces {
                let support_cell = support_surfaces
                    .get(&surface)
                    .unwrap_or_else(|| panic!("{} lacks {profile}/{surface}", family.public_id));
                match cell.status.as_str() {
                    "applicable" => {
                        assert_eq!(support_cell.status, "asserted");
                        assert!(!support_cell.evidence.is_empty());
                        assert!(support_cell.rationale.is_none());
                        assert_eq!(
                            cell.query.as_deref(),
                            Some(format!("queries/{profile}/{surface}.scm").as_str())
                        );
                        assert!(
                            packaged.contains(&(profile.to_string(), surface.clone())),
                            "{profile}/{surface} is applicable but not packaged"
                        );
                        asserted_profiles.insert((profile.to_string(), surface));
                    }
                    "not_applicable" => {
                        assert_eq!(support_cell.status, "not_applicable");
                        assert!(support_cell.evidence.is_empty());
                        assert_eq!(support_cell.rationale, cell.rationale);
                        assert!(
                            cell.rationale
                                .as_deref()
                                .is_some_and(|rationale| !rationale.trim().is_empty())
                        );
                        assert!(cell.query.is_none());
                    }
                    status => panic!("{profile}/{}/{surface}: {status}", family.public_id),
                }
            }
        }
        assert_eq!(family_ids, support_by_id.keys().cloned().collect());
    }

    assert_eq!(asserted_profiles, packaged);
}
