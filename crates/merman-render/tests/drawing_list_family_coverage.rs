use merman_render::family::RenderFamilyKind;
use serde::Deserialize;
use std::collections::BTreeSet;

const COVERAGE_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/drawing-list/v1/family-coverage.json"
));

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    schema_version: u32,
    families: Vec<FamilyCoverageRow>,
}

#[derive(Debug, Deserialize)]
struct FamilyCoverageRow {
    id: String,
    document_adapter: String,
    svg_serializer: String,
}

#[test]
fn family_coverage_matrix_matches_the_renderer_catalog() {
    let matrix: CoverageMatrix =
        serde_json::from_str(COVERAGE_JSON).expect("family coverage fixture must be valid JSON");
    assert_eq!(matrix.schema_version, 1);
    assert_eq!(matrix.families.len(), RenderFamilyKind::ALL.len());

    let expected = RenderFamilyKind::ALL
        .into_iter()
        .map(RenderFamilyKind::as_str)
        .collect::<Vec<_>>();
    let actual = matrix
        .families
        .iter()
        .map(|row| row.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "coverage rows must follow enum order");

    let unique_ids = actual.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_ids.len(), actual.len(), "family ids must be unique");
    for row in &matrix.families {
        assert_eq!(
            row.document_adapter, "direct",
            "{} must have a direct adapter",
            row.id
        );
        assert!(
            matches!(row.svg_serializer.as_str(), "canonical" | "legacy-bridge"),
            "{} has an unknown SVG serializer status",
            row.id
        );
    }
}
