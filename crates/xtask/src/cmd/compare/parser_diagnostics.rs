//! Exact, reviewed differences in implementation-specific parser diagnostic messages.
//!
//! Mermaid 12.1 renders parser errors in its error diagram. The Rust parser does not implement
//! Jison/Chevrotain error prose. These receipts acknowledge ten reviewed error-rendering
//! residual fixtures across four fixture families, not a rendering equivalence: every byte of each
//! SVG is bound, including messages, geometry and escaping. No diagnostic text or DOM is normalized and
//! strict comparison remains blocking.

use crate::util::{is_canonical_sha256, sha256_hex};
use std::sync::OnceLock;

const RECEIPT_KEYS: [(&str, &str); 10] = [
    ("error", "upstream_pkgtests_statediagram_spec_024"),
    ("error", "upstream_pkgtests_statediagram_v2_spec_024"),
    ("packet", "upstream_docs_packet_bits_syntax_v11_7_0_002"),
    ("packet", "upstream_docs_packet_syntax_001"),
    ("radar", "upstream_docs_radar_axis_007"),
    ("radar", "upstream_docs_radar_curve_008"),
    ("radar", "upstream_docs_radar_examples_005"),
    ("radar", "upstream_docs_radar_options_009"),
    ("radar", "upstream_docs_radar_title_006"),
    (
        "treemap",
        "upstream_treemap_classdef_and_css_compiled_styles_db",
    ),
];

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    mermaid_version: String,
    mermaid_source_commit: String,
    entries: Vec<Receipt>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    diagram: String,
    fixture: String,
    input_sha256: String,
    upstream_svg_sha256: String,
    local_svg_sha256: String,
}

static CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();

fn load_catalog() -> Result<Catalog, String> {
    let path = crate::cmd::fixtures_root().join("_verification/parser-diagnostic-residuals.json");
    let contents = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "read parser diagnostic receipts {}: {error}",
            path.display()
        )
    })?;
    let catalog: Catalog = serde_json::from_str(&contents)
        .map_err(|error| format!("parse parser diagnostic receipts: {error}"))?;
    if catalog.schema_version != 2
        || catalog.mermaid_version != merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION
        || catalog.mermaid_source_commit != crate::cmd::MERMAID_SOURCE_COMMIT
        || !catalog
            .entries
            .iter()
            .map(|entry| (entry.diagram.as_str(), entry.fixture.as_str()))
            .eq(RECEIPT_KEYS)
    {
        return Err("parser diagnostic residual reference contract drifted".to_string());
    }
    for entry in &catalog.entries {
        for digest in [
            &entry.input_sha256,
            &entry.upstream_svg_sha256,
            &entry.local_svg_sha256,
        ] {
            if !is_canonical_sha256(digest) {
                return Err(format!(
                    "invalid parser diagnostic receipt digest for {}",
                    entry.fixture
                ));
            }
        }
    }
    Ok(catalog)
}

/// Returns `true` only for an exact acknowledged diagnostic difference. This does not turn the
/// underlying DOM mismatch into parity evidence; the caller must report the residual explicitly.
pub(crate) fn accepts_parser_diagnostic_residual(
    diagram: &str,
    fixture: &str,
    mode: crate::svgdom::DomMode,
    decimals: u32,
    input: &str,
    upstream_svg: &str,
    local_svg: &str,
) -> Result<bool, String> {
    if matches!(mode, crate::svgdom::DomMode::Strict) {
        return Ok(false);
    }
    if !RECEIPT_KEYS.contains(&(diagram, fixture)) {
        return Ok(false);
    }
    if decimals != 3 {
        return Err(format!(
            "parser diagnostic receipt precision drifted for {diagram}/{fixture}"
        ));
    }
    let catalog = CATALOG
        .get_or_init(load_catalog)
        .as_ref()
        .map_err(Clone::clone)?;
    let receipt = catalog
        .entries
        .iter()
        .find(|entry| entry.diagram == diagram && entry.fixture == fixture)
        .ok_or_else(|| format!("missing parser diagnostic receipt for {diagram}/{fixture}"))?;
    for (role, expected, actual) in [
        ("input", &receipt.input_sha256, input),
        ("upstream SVG", &receipt.upstream_svg_sha256, upstream_svg),
        ("local SVG", &receipt.local_svg_sha256, local_svg),
    ] {
        if sha256_hex(actual.as_bytes()) != *expected {
            return Err(format!(
                "parser diagnostic receipt {role} drifted for {diagram}/{fixture}; the mismatch remains blocking"
            ));
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svgdom::DomMode;

    #[test]
    fn parser_diagnostic_receipts_cover_all_reviewed_diagrams() {
        let catalog = load_catalog().expect("current pinned diagnostic receipts");
        assert_eq!(
            catalog
                .entries
                .iter()
                .map(|entry| (entry.diagram.as_str(), entry.fixture.as_str()))
                .collect::<Vec<_>>(),
            RECEIPT_KEYS.as_slice()
        );
        for receipt in &catalog.entries {
            let root = crate::cmd::fixtures_root();
            let input = std::fs::read_to_string(
                root.join(&receipt.diagram)
                    .join(format!("{}.mmd", receipt.fixture)),
            )
            .unwrap();
            let upstream = std::fs::read_to_string(
                root.join("upstream-svgs")
                    .join(&receipt.diagram)
                    .join(format!("{}.svg", receipt.fixture)),
            )
            .unwrap();
            assert_eq!(sha256_hex(input.as_bytes()), receipt.input_sha256);
            assert_eq!(sha256_hex(upstream.as_bytes()), receipt.upstream_svg_sha256);
            assert!(
                accepts_parser_diagnostic_residual(
                    &receipt.diagram,
                    &receipt.fixture,
                    DomMode::Structure,
                    3,
                    &input,
                    &upstream,
                    &receipt.local_svg_sha256,
                )
                .is_err(),
                "a digest string is not a local SVG"
            );
            assert!(
                accepts_parser_diagnostic_residual(
                    &receipt.diagram,
                    &receipt.fixture,
                    DomMode::Structure,
                    2,
                    &input,
                    &upstream,
                    "",
                )
                .is_err()
            );
            assert!(
                !accepts_parser_diagnostic_residual(
                    &receipt.diagram,
                    &receipt.fixture,
                    DomMode::Strict,
                    3,
                    &input,
                    &upstream,
                    "",
                )
                .unwrap()
            );
            for (source, oracle, local, role) in [
                (
                    format!("{input}\n"),
                    upstream.clone(),
                    "anything".to_string(),
                    "input",
                ),
                (
                    input.clone(),
                    format!("{upstream} "),
                    "anything".to_string(),
                    "upstream SVG",
                ),
                (
                    input.clone(),
                    upstream.clone(),
                    "anything".to_string(),
                    "local SVG",
                ),
            ] {
                let error = accepts_parser_diagnostic_residual(
                    &receipt.diagram,
                    &receipt.fixture,
                    DomMode::Structure,
                    3,
                    &source,
                    &oracle,
                    &local,
                )
                .unwrap_err();
                assert!(error.contains(role), "{error}");
            }
        }
    }

    #[test]
    fn parser_diagnostic_receipts_require_the_matching_diagram() {
        let catalog = load_catalog().expect("current pinned diagnostic receipts");
        let receipt = catalog
            .entries
            .iter()
            .find(|entry| entry.diagram == "treemap")
            .unwrap();
        let root = crate::cmd::fixtures_root();
        let input = std::fs::read_to_string(
            root.join(&receipt.diagram)
                .join(format!("{}.mmd", receipt.fixture)),
        )
        .unwrap();
        let upstream = std::fs::read_to_string(
            root.join("upstream-svgs")
                .join(&receipt.diagram)
                .join(format!("{}.svg", receipt.fixture)),
        )
        .unwrap();
        assert!(
            !accepts_parser_diagnostic_residual(
                "error",
                &receipt.fixture,
                DomMode::Structure,
                3,
                &input,
                &upstream,
                "",
            )
            .unwrap()
        );
        assert!(
            !accepts_parser_diagnostic_residual(
                &receipt.diagram,
                "new-unreviewed-fixture",
                DomMode::Structure,
                3,
                &input,
                &upstream,
                "",
            )
            .unwrap()
        );
    }
}
