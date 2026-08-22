//! Pinned, cross-transport contracts used only by ASCII integration tests.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsciiResourceBoundary {
    pub id: String,
    pub phase: String,
    pub source: String,
    pub exact: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsciiTransportRepresentativeBoundaries {
    pub cli_trusted_native: AsciiResourceBoundary,
    pub uniffi_interactive: AsciiResourceBoundary,
    pub wasm_interactive: AsciiResourceBoundary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsciiResourceBoundaryContract {
    schema_version: u32,
    purpose: String,
    update_policy: String,
    pub binding_core_interactive: Vec<AsciiResourceBoundary>,
    pub transport_representatives: AsciiTransportRepresentativeBoundaries,
}

fn validate_boundary(
    boundary: &AsciiResourceBoundary,
    expected_id: &str,
    expected_phase: &str,
    owner: &str,
) {
    assert_eq!(boundary.id, expected_id, "{owner} has the wrong limit id");
    assert_eq!(
        boundary.phase, expected_phase,
        "{owner} has the wrong phase"
    );
    assert!(boundary.exact > 0, "{owner} boundary must be positive");
    assert!(
        !boundary.source.is_empty(),
        "{owner} boundary must provide source"
    );
}

pub fn ascii_resource_boundary_contract() -> AsciiResourceBoundaryContract {
    let contract: AsciiResourceBoundaryContract =
        serde_json::from_str(include_str!("ascii_resource_boundaries.json"))
            .expect("ASCII resource boundary contract must be valid JSON");
    assert_eq!(
        contract.schema_version, 3,
        "unsupported ASCII resource boundary contract schema"
    );
    assert!(
        !contract.purpose.is_empty(),
        "ASCII resource boundary contract must state its purpose"
    );
    assert!(
        !contract.update_policy.is_empty(),
        "ASCII resource boundary contract must state its update policy"
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
        contract.binding_core_interactive.len(),
        expected_id_phases.len(),
        "Binding Core ASCII resource boundaries must cover every public limit exactly once"
    );
    for (expected_id, expected_phase) in expected_id_phases {
        let matches: Vec<_> = contract
            .binding_core_interactive
            .iter()
            .filter(|case| case.id == expected_id)
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "Binding Core ASCII resource boundary {expected_id} must appear exactly once"
        );
        validate_boundary(
            matches[0],
            expected_id,
            expected_phase,
            "Binding Core ASCII resource boundary",
        );
    }

    for (transport, representative) in [
        (
            "CLI trusted-native",
            &contract.transport_representatives.cli_trusted_native,
        ),
        (
            "UniFFI interactive",
            &contract.transport_representatives.uniffi_interactive,
        ),
        (
            "WASM interactive",
            &contract.transport_representatives.wasm_interactive,
        ),
    ] {
        validate_boundary(
            representative,
            "max_ascii_output_bytes",
            "ascii_output",
            transport,
        );
    }

    contract
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_separates_canonical_and_transport_boundaries() {
        let contract = ascii_resource_boundary_contract();
        assert_eq!(contract.binding_core_interactive.len(), 6);
        for boundary in [
            contract.transport_representatives.cli_trusted_native,
            contract.transport_representatives.uniffi_interactive,
            contract.transport_representatives.wasm_interactive,
        ] {
            assert_eq!(boundary.id, "max_ascii_output_bytes");
            assert_eq!(boundary.phase, "ascii_output");
        }
    }
}
