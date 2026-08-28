//! Exact verification receipts for browser-text-layout residuals.
//!
//! The catalog is verification-only. It never changes production rendering and never accepts a
//! new DOM shape merely because it belongs to a measurement-sensitive diagram family. A residual
//! is diagnostic only when the fixture input, pinned upstream SVG, comparison mode, and complete
//! deterministic local SVG digest still match an explicitly reviewed receipt.

use crate::util::{is_canonical_sha256, sha256_hex};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

const SCHEMA_VERSION: u32 = 2;
const COMPARISON_REVISION: &str = "browser-text-layout-residual-v2";
const MEASUREMENT_PROVIDER: &str = "deterministic";
const CATALOG_RELATIVE_PATH: &str = "_verification/browser-text-layout-residuals.json";
const DIAGRAMS: [&str; 8] = [
    "architecture",
    "class",
    "flowchart",
    "gantt",
    "journey",
    "sequence",
    "timeline",
    "treemap",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserTextLayoutResidualCatalog {
    schema_version: u32,
    mermaid_version: String,
    mermaid_source_commit: String,
    measurement_provider: String,
    comparison_revision: String,
    entries: Vec<BrowserTextLayoutResidual>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BrowserTextLayoutResidual {
    diagram: String,
    fixture: String,
    modes: Vec<String>,
    input_sha256: String,
    upstream_svg_sha256: String,
    local_svg_sha256: String,
}

impl BrowserTextLayoutResidual {
    pub(crate) fn admits_mode(&self, mode: crate::svgdom::DomMode) -> bool {
        self.modes
            .iter()
            .any(|candidate| candidate == mode.as_str())
    }

    pub(crate) fn validate_local_svg(&self, local_svg: &str) -> Result<(), String> {
        self.validate_digest("local SVG", &self.local_svg_sha256, local_svg.as_bytes())
    }

    fn validate_source_artifacts(&self, input: &[u8], upstream_svg: &[u8]) -> Result<(), String> {
        self.validate_digest("input", &self.input_sha256, input)?;
        self.validate_digest("upstream SVG", &self.upstream_svg_sha256, upstream_svg)
    }

    fn validate_digest(&self, role: &str, expected: &str, bytes: &[u8]) -> Result<(), String> {
        let actual = sha256_hex(bytes);
        if actual == expected {
            return Ok(());
        }
        Err(format!(
            "browser text layout receipt {role} drifted for {}/{}: expected sha256:{expected}, found sha256:{actual}",
            self.diagram, self.fixture
        ))
    }

    #[cfg(test)]
    pub(crate) fn test_only(
        diagram: &str,
        fixture: &str,
        modes: &[crate::svgdom::DomMode],
        input: &str,
        upstream_svg: &str,
        local_svg: &str,
    ) -> Self {
        Self {
            diagram: diagram.to_string(),
            fixture: fixture.to_string(),
            modes: modes.iter().map(|mode| mode.as_str().to_string()).collect(),
            input_sha256: sha256_hex(input.as_bytes()),
            upstream_svg_sha256: sha256_hex(upstream_svg.as_bytes()),
            local_svg_sha256: sha256_hex(local_svg.as_bytes()),
        }
    }
}

static CATALOG: OnceLock<Result<BrowserTextLayoutResidualCatalog, String>> = OnceLock::new();

pub(crate) fn diagram_has_browser_text_layout_residuals(diagram: &str) -> bool {
    DIAGRAMS.contains(&diagram)
}

pub(crate) fn browser_text_layout_residual(
    diagram: &str,
    fixture: &str,
) -> Result<Option<&'static BrowserTextLayoutResidual>, String> {
    let catalog = match CATALOG.get_or_init(load_catalog) {
        Ok(catalog) => catalog,
        Err(error) => return Err(error.clone()),
    };
    Ok(catalog
        .entries
        .iter()
        .find(|entry| entry.diagram == diagram && entry.fixture == fixture))
}

fn catalog_path() -> PathBuf {
    crate::cmd::fixtures_root().join(CATALOG_RELATIVE_PATH)
}

fn load_catalog() -> Result<BrowserTextLayoutResidualCatalog, String> {
    let path = catalog_path();
    let json = fs::read_to_string(&path).map_err(|error| {
        format!(
            "read browser text layout residual catalog {}: {error}",
            path.display()
        )
    })?;
    let catalog: BrowserTextLayoutResidualCatalog =
        serde_json::from_str(&json).map_err(|error| {
            format!(
                "parse browser text layout residual catalog {}: {error}",
                path.display()
            )
        })?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

fn validate_catalog(catalog: &BrowserTextLayoutResidualCatalog) -> Result<(), String> {
    if catalog.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "browser text layout residual schema {} is unsupported; expected {SCHEMA_VERSION}",
            catalog.schema_version
        ));
    }
    if catalog.mermaid_version != merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION
        || catalog.mermaid_source_commit != crate::cmd::MERMAID_SOURCE_COMMIT
        || catalog.measurement_provider != MEASUREMENT_PROVIDER
        || catalog.comparison_revision != COMPARISON_REVISION
    {
        return Err("browser text layout residual reference contract drifted".to_string());
    }
    if catalog.entries.is_empty() {
        return Err("browser text layout residual catalog must not be empty".to_string());
    }

    let mut previous: Option<(&str, &str)> = None;
    for entry in &catalog.entries {
        let key = (entry.diagram.as_str(), entry.fixture.as_str());
        if previous.is_some_and(|previous| previous >= key) {
            return Err(format!(
                "browser text layout residual entries must be unique and sorted, found {}/{} after {:?}",
                entry.diagram, entry.fixture, previous
            ));
        }
        previous = Some(key);
        if !diagram_has_browser_text_layout_residuals(&entry.diagram) {
            return Err(format!(
                "browser text layout residual {}/{} names a diagram without a source-backed measurement boundary",
                entry.diagram, entry.fixture
            ));
        }
        for (role, digest) in [
            ("input", entry.input_sha256.as_str()),
            ("upstream SVG", entry.upstream_svg_sha256.as_str()),
            ("local SVG", entry.local_svg_sha256.as_str()),
        ] {
            if !is_canonical_sha256(digest) {
                return Err(format!(
                    "browser text layout residual {}/{} has an invalid {role} SHA-256",
                    entry.diagram, entry.fixture
                ));
            }
        }

        let mut previous_mode_rank = None;
        for mode in &entry.modes {
            let parsed = mode.parse::<crate::svgdom::DomMode>().map_err(|_| {
                format!(
                    "browser text layout residual {}/{} has invalid mode {mode:?}",
                    entry.diagram, entry.fixture
                )
            })?;
            let rank = match parsed {
                crate::svgdom::DomMode::Structure => 0,
                crate::svgdom::DomMode::Parity => 1,
                crate::svgdom::DomMode::ParityRoot => 2,
                crate::svgdom::DomMode::Strict => {
                    return Err(format!(
                        "browser text layout residual {}/{} cannot admit strict mode",
                        entry.diagram, entry.fixture
                    ));
                }
            };
            if previous_mode_rank.is_some_and(|previous| previous >= rank) {
                return Err(format!(
                    "browser text layout residual {}/{} modes must be unique and canonically ordered",
                    entry.diagram, entry.fixture
                ));
            }
            previous_mode_rank = Some(rank);
        }
        if entry.modes.is_empty() {
            return Err(format!(
                "browser text layout residual {}/{} must admit at least one mode",
                entry.diagram, entry.fixture
            ));
        }

        let input_path = crate::cmd::fixtures_root()
            .join(&entry.diagram)
            .join(format!("{}.mmd", entry.fixture));
        let upstream_path = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join(&entry.diagram)
            .join(format!("{}.svg", entry.fixture));
        let input = fs::read(&input_path).map_err(|error| {
            format!(
                "read browser text layout input {}: {error}",
                input_path.display()
            )
        })?;
        let upstream_svg = fs::read(&upstream_path).map_err(|error| {
            format!(
                "read browser text layout upstream SVG {}: {error}",
                upstream_path.display()
            )
        })?;
        entry.validate_source_artifacts(&input, &upstream_svg)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_catalog_is_sorted_valid_and_source_backed() {
        let catalog = load_catalog().expect("browser text layout residual catalog");
        assert_eq!(catalog.entries.len(), 92);
        for diagram in DIAGRAMS {
            assert!(
                catalog.entries.iter().any(|entry| entry.diagram == diagram),
                "diagram={diagram}"
            );
        }
    }

    #[test]
    fn receipt_binds_the_complete_local_svg_and_exact_modes() {
        let input = "sequenceDiagram\nA->>B: probe";
        let upstream = "<svg><text>browser</text></svg>";
        let accepted = "<svg><text>wrapped</text></svg>";
        let receipt = BrowserTextLayoutResidual::test_only(
            "sequence",
            "probe",
            &[crate::svgdom::DomMode::Structure],
            input,
            upstream,
            accepted,
        );

        assert!(receipt.admits_mode(crate::svgdom::DomMode::Structure));
        assert!(!receipt.admits_mode(crate::svgdom::DomMode::Parity));
        receipt
            .validate_source_artifacts(input.as_bytes(), upstream.as_bytes())
            .unwrap();
        receipt.validate_local_svg(accepted).unwrap();
        assert!(
            receipt
                .validate_source_artifacts(b"changed input", upstream.as_bytes())
                .is_err()
        );
        assert!(
            receipt
                .validate_source_artifacts(input.as_bytes(), b"<svg><circle/></svg>")
                .is_err()
        );
        assert!(receipt.validate_local_svg("<svg><circle/></svg>").is_err());
    }
}
