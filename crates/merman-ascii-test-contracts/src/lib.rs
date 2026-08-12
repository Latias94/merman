//! Pinned, cross-transport contracts used only by ASCII integration tests.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AsciiResourceBoundary {
    pub id: String,
    pub phase: String,
    pub source: String,
    pub expected: AsciiResourceBoundaryProfiles,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AsciiResourceBoundaryProfiles {
    pub headless_ascii: u64,
    pub binding_interactive: u64,
    pub wasm_interactive: u64,
    pub uniffi_interactive: u64,
    pub cli_trusted_native: u64,
}

#[derive(Debug, Deserialize)]
struct AsciiResourceBoundaryContract {
    schema_version: u32,
    cases: Vec<AsciiResourceBoundary>,
}

pub fn ascii_resource_boundaries() -> Vec<AsciiResourceBoundary> {
    let contract: AsciiResourceBoundaryContract =
        serde_json::from_str(include_str!("ascii_resource_boundaries.json"))
            .expect("ASCII resource boundary contract must be valid JSON");
    assert_eq!(
        contract.schema_version, 1,
        "unsupported ASCII resource boundary contract schema"
    );

    let expected_id_phases = [
        ("max_ascii_grid_cells", "ascii_layout"),
        ("max_ascii_layout_work_units", "ascii_layout_work"),
        ("max_ascii_document_cells", "ascii_document"),
        ("max_ascii_output_bytes", "ascii_output"),
        ("max_ascii_grapheme_bytes", "ascii_grapheme"),
        ("max_ascii_nesting_depth", "ascii_nesting"),
    ];
    assert_eq!(
        contract.cases.len(),
        expected_id_phases.len(),
        "ASCII resource boundary contract must cover every public limit exactly once"
    );
    for (expected_id, expected_phase) in expected_id_phases {
        let matches: Vec<_> = contract
            .cases
            .iter()
            .filter(|case| case.id == expected_id)
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "ASCII resource boundary {expected_id} must appear exactly once"
        );
        assert_eq!(
            matches[0].phase, expected_phase,
            "ASCII resource boundary {expected_id} has the wrong phase"
        );
    }
    contract.cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_boundaries_cover_each_public_limit_once() {
        assert_eq!(ascii_resource_boundaries().len(), 6);
    }
}
