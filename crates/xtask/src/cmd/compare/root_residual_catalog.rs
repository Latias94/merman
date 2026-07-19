//! Version-pinned, verification-only root viewport residual catalog.

use crate::svgdom::{self, DomComparisonProfile, DomMode, ParityRootMismatch, SvgDomNode};
use crate::util::{is_canonical_sha256, sha256_hex};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;
const COMPARISON_REVISION: &str = "svgdom-root-v3";
const CATALOG_RELATIVE_PATH: &str = "_verification/root-parity-residuals.json";
const CANDIDATE_FILE_NAME: &str = "root-parity-residuals.candidate.json";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootResidualCatalog {
    schema_version: u32,
    contract: RootResidualContract,
    evidence: BTreeMap<String, RootResidualEvidence>,
    entries: Vec<RootResidualEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootResidualContract {
    mermaid_version: String,
    mermaid_source_commit: String,
    comparison_revision: String,
    root_attributes: Vec<String>,
    decimals: u32,
}

impl RootResidualContract {
    fn current(decimals: u32) -> Self {
        Self {
            mermaid_version: merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION.to_string(),
            mermaid_source_commit: crate::cmd::MERMAID_SOURCE_COMMIT.to_string(),
            comparison_revision: COMPARISON_REVISION.to_string(),
            root_attributes: ["style", "viewBox", "width", "height"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            decimals,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootResidualEvidence {
    kind: RootResidualEvidenceKind,
    reference: String,
    rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RootResidualEvidenceKind {
    BrowserMeasurement,
    SourceBackedLayoutApproximation,
    RoughJsImplementation,
    Unreviewed,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
enum DescendantProfile {
    Parity,
    Structure,
}

impl DescendantProfile {
    fn from_comparison(profile: DomComparisonProfile) -> Result<Self, String> {
        match profile.descendants() {
            DomMode::Parity => Ok(Self::Parity),
            DomMode::Structure => Ok(Self::Structure),
            mode => Err(format!(
                "root residuals require parity or structure descendants, found {mode:?}"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootViewportSignature {
    style: Option<String>,
    view_box: Option<String>,
    width: Option<String>,
    height: Option<String>,
}

impl RootViewportSignature {
    fn from_dom(dom: &SvgDomNode) -> Self {
        Self {
            style: dom.attrs.get("style").cloned(),
            view_box: dom.attrs.get("viewBox").cloned(),
            width: dom.attrs.get("width").cloned(),
            height: dom.attrs.get("height").cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootResidualEntry {
    diagram: String,
    fixture: String,
    descendants: DescendantProfile,
    input_sha256: String,
    upstream_svg_sha256: String,
    upstream: RootViewportSignature,
    local: RootViewportSignature,
    evidence_id: String,
}

impl RootResidualEntry {
    fn key(&self) -> (&str, &str) {
        (&self.diagram, &self.fixture)
    }

    fn matches(&self, observation: &RootResidualObservation) -> bool {
        self.diagram == observation.diagram
            && self.fixture == observation.fixture
            && self.descendants == observation.descendants
            && self.input_sha256 == observation.input_sha256
            && self.upstream_svg_sha256 == observation.upstream_svg_sha256
            && self.upstream == observation.upstream
            && self.local == observation.local
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RootResidualObservation {
    diagram: String,
    fixture: String,
    descendants: DescendantProfile,
    input_sha256: String,
    upstream_svg_sha256: String,
    upstream: RootViewportSignature,
    local: RootViewportSignature,
}

impl RootResidualObservation {
    fn into_candidate_entry(self) -> RootResidualEntry {
        RootResidualEntry {
            diagram: self.diagram,
            fixture: self.fixture,
            descendants: self.descendants,
            input_sha256: self.input_sha256,
            upstream_svg_sha256: self.upstream_svg_sha256,
            upstream: self.upstream,
            local: self.local,
            evidence_id: "unreviewed".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DomMismatchLocator {
    fixture: String,
    upstream_path: PathBuf,
    local_path: PathBuf,
}

fn parse_dom_mismatch_locator(line: &str) -> Option<DomMismatchLocator> {
    let line = line.trim().strip_prefix("- ").unwrap_or(line.trim());
    let body = line.strip_prefix("dom mismatch for ")?;
    let (fixture, body) = body.split_once(": upstream=")?;
    let (upstream_path, body) = body.split_once(" local=")?;
    let (local_path, _) = body.split_once(" (")?;
    if fixture.is_empty() || upstream_path.is_empty() || local_path.is_empty() {
        return None;
    }
    Some(DomMismatchLocator {
        fixture: fixture.to_string(),
        upstream_path: PathBuf::from(upstream_path),
        local_path: PathBuf::from(local_path),
    })
}

pub(crate) fn accept_root_residual_candidate(args: Vec<String>) -> Result<(), crate::XtaskError> {
    let expected_sha256 = parse_accept_candidate_args(args)?;
    let candidate_path = candidate_path();
    let bytes = fs::read(&candidate_path).map_err(|source| crate::XtaskError::ReadFile {
        path: candidate_path.display().to_string(),
        source,
    })?;
    let actual_sha256 = sha256_hex(&bytes);
    if actual_sha256 != expected_sha256 {
        return Err(crate::XtaskError::SvgCompareFailed(format!(
            "root residual candidate digest mismatch: expected {expected_sha256}, found {actual_sha256}"
        )));
    }
    let catalog: RootResidualCatalog = serde_json::from_slice(&bytes)?;
    validate_catalog(&catalog, 3, true).map_err(crate::XtaskError::SvgCompareFailed)?;
    if catalog.entries.is_empty() {
        return Err(crate::XtaskError::SvgCompareFailed(
            "refusing to accept an empty root residual candidate".to_string(),
        ));
    }

    let path = catalog_path();
    fs::write(&path, bytes).map_err(|source| crate::XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })?;
    println!(
        "accepted {} reviewed root residuals from candidate {} into {}",
        catalog.entries.len(),
        actual_sha256,
        path.display()
    );
    Ok(())
}

fn parse_accept_candidate_args(args: Vec<String>) -> Result<String, crate::XtaskError> {
    let [flag, digest] = args.as_slice() else {
        return Err(crate::XtaskError::Usage);
    };
    if flag != "--sha256" || !is_canonical_sha256(digest) {
        return Err(crate::XtaskError::Usage);
    }
    Ok(digest.clone())
}

fn read_hashed(path: &Path) -> Result<(Vec<u8>, String), String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let digest = sha256_hex(&bytes);
    Ok((bytes, digest))
}

fn ensure_path_under(path: &Path, root: &Path, role: &str) -> Result<(), String> {
    let path = fs::canonicalize(path)
        .map_err(|error| format!("canonicalize {role} {}: {error}", path.display()))?;
    let root = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize {role} root {}: {error}", root.display()))?;
    if path.starts_with(&root) {
        Ok(())
    } else {
        Err(format!(
            "{role} path {} escapes expected root {}",
            path.display(),
            root.display()
        ))
    }
}

fn observe_root_residual(
    diagram: &str,
    line: &str,
    decimals: u32,
) -> Result<Option<RootResidualObservation>, String> {
    let Some(locator) = parse_dom_mismatch_locator(line) else {
        return Ok(None);
    };

    let upstream_root = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join(diagram);
    let local_root = crate::cmd::target_root().join("compare").join(diagram);
    ensure_path_under(&locator.upstream_path, &upstream_root, "upstream SVG")?;
    ensure_path_under(&locator.local_path, &local_root, "local SVG")?;

    let (upstream_bytes, upstream_svg_sha256) = read_hashed(&locator.upstream_path)?;
    let (local_bytes, _) = read_hashed(&locator.local_path)?;
    let upstream_svg = std::str::from_utf8(&upstream_bytes)
        .map_err(|error| format!("upstream SVG is not UTF-8: {error}"))?;
    let local_svg = std::str::from_utf8(&local_bytes)
        .map_err(|error| format!("local SVG is not UTF-8: {error}"))?;

    let (profile, _) = super::fixture_dom_profile(diagram, &locator.fixture, DomMode::ParityRoot);
    let upstream = svgdom::dom_signature_for_comparison(upstream_svg, profile, decimals)?;
    let local = svgdom::dom_signature_for_comparison(local_svg, profile, decimals)?;
    let mismatch = svgdom::diagnose_root_viewport_mismatch(
        upstream_svg,
        local_svg,
        &upstream,
        &local,
        profile,
        decimals,
    )?
    .ok_or_else(|| {
        format!(
            "reported mismatch for {diagram}/{} recomputed as equal",
            locator.fixture
        )
    })?;
    if !matches!(
        mismatch,
        ParityRootMismatch::NormalizedDescendantsMatch { .. }
    ) {
        return Ok(None);
    }

    let fixture_path = crate::cmd::fixtures_root()
        .join(diagram)
        .join(format!("{}.mmd", locator.fixture));
    let (_, input_sha256) = read_hashed(&fixture_path)?;

    Ok(Some(RootResidualObservation {
        diagram: diagram.to_string(),
        fixture: locator.fixture,
        descendants: DescendantProfile::from_comparison(profile)?,
        input_sha256,
        upstream_svg_sha256,
        upstream: RootViewportSignature::from_dom(&upstream),
        local: RootViewportSignature::from_dom(&local),
    }))
}

fn catalog_path() -> PathBuf {
    crate::cmd::fixtures_root().join(CATALOG_RELATIVE_PATH)
}

fn candidate_path() -> PathBuf {
    crate::cmd::target_root().join(CANDIDATE_FILE_NAME)
}

fn load_catalog(decimals: u32) -> Result<RootResidualCatalog, String> {
    let path = catalog_path();
    let json = fs::read_to_string(&path)
        .map_err(|error| format!("read root residual catalog {}: {error}", path.display()))?;
    let catalog: RootResidualCatalog = serde_json::from_str(&json)
        .map_err(|error| format!("parse root residual catalog {}: {error}", path.display()))?;
    validate_catalog(&catalog, decimals, true)?;
    Ok(catalog)
}

fn validate_evidence_reference(
    evidence_id: &str,
    evidence: &RootResidualEvidence,
) -> Result<(), String> {
    if evidence.rationale.trim().is_empty() {
        return Err(format!(
            "root residual evidence `{evidence_id}` has an empty rationale"
        ));
    }

    let workspace = fs::canonicalize(crate::cmd::workspace_root())
        .map_err(|error| format!("canonicalize workspace root: {error}"))?;
    let requested = workspace.join(&evidence.reference);
    let reference = fs::canonicalize(&requested).map_err(|error| {
        format!(
            "canonicalize root residual evidence `{evidence_id}` {}: {error}",
            requested.display()
        )
    })?;
    if !reference.starts_with(&workspace) {
        return Err(format!(
            "root residual evidence `{evidence_id}` reference {} escapes workspace {}",
            reference.display(),
            workspace.display()
        ));
    }
    if !reference.is_file() {
        return Err(format!(
            "root residual evidence `{evidence_id}` points to non-file {}",
            reference.display()
        ));
    }
    Ok(())
}

fn validate_evidence_profile(
    entry: &RootResidualEntry,
    evidence: &RootResidualEvidence,
    require_reviewed: bool,
) -> Result<(), String> {
    let compatible = match evidence.kind {
        RootResidualEvidenceKind::BrowserMeasurement
        | RootResidualEvidenceKind::SourceBackedLayoutApproximation => {
            entry.descendants == DescendantProfile::Parity
        }
        RootResidualEvidenceKind::RoughJsImplementation => {
            entry.descendants == DescendantProfile::Structure
        }
        // Candidate generation needs a temporary marker. Both catalog loading and candidate
        // acceptance call this validator with `require_reviewed = true`, so unreviewed evidence
        // can never enter verification policy.
        RootResidualEvidenceKind::Unreviewed => !require_reviewed,
    };
    if compatible {
        Ok(())
    } else {
        Err(format!(
            "root residual {}/{} has incompatible evidence kind {:?} and descendant profile {:?}",
            entry.diagram, entry.fixture, evidence.kind, entry.descendants
        ))
    }
}

fn validate_entry_artifacts(entry: &RootResidualEntry) -> Result<(), String> {
    for (role, value) in [
        ("input", entry.input_sha256.as_str()),
        ("upstream SVG", entry.upstream_svg_sha256.as_str()),
    ] {
        if !is_canonical_sha256(value) {
            return Err(format!(
                "root residual {}/{} {role} SHA-256 must be 64 lowercase hexadecimal characters",
                entry.diagram, entry.fixture
            ));
        }
    }

    let fixture_root = crate::cmd::fixtures_root().join(&entry.diagram);
    let fixture_path = fixture_root.join(format!("{}.mmd", entry.fixture));
    ensure_path_under(&fixture_path, &fixture_root, "root residual input")?;
    if !fixture_path.is_file() {
        return Err(format!(
            "root residual {}/{} input is not a file: {}",
            entry.diagram,
            entry.fixture,
            fixture_path.display()
        ));
    }
    let (_, input_sha256) = read_hashed(&fixture_path)?;
    if input_sha256 != entry.input_sha256 {
        return Err(format!(
            "root residual {}/{} input SHA-256 drifted: expected {}, found {input_sha256}",
            entry.diagram, entry.fixture, entry.input_sha256
        ));
    }

    let upstream_root = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join(&entry.diagram);
    let upstream_path = upstream_root.join(format!("{}.svg", entry.fixture));
    ensure_path_under(&upstream_path, &upstream_root, "root residual upstream SVG")?;
    if !upstream_path.is_file() {
        return Err(format!(
            "root residual {}/{} upstream SVG is not a file: {}",
            entry.diagram,
            entry.fixture,
            upstream_path.display()
        ));
    }
    let (_, upstream_svg_sha256) = read_hashed(&upstream_path)?;
    if upstream_svg_sha256 != entry.upstream_svg_sha256 {
        return Err(format!(
            "root residual {}/{} upstream SVG SHA-256 drifted: expected {}, found {upstream_svg_sha256}",
            entry.diagram, entry.fixture, entry.upstream_svg_sha256
        ));
    }

    let (profile, _) =
        super::fixture_dom_profile(&entry.diagram, &entry.fixture, DomMode::ParityRoot);
    let expected_descendants = DescendantProfile::from_comparison(profile)?;
    if entry.descendants != expected_descendants {
        return Err(format!(
            "root residual {}/{} descendant profile {:?} disagrees with fixture profile {:?}",
            entry.diagram, entry.fixture, entry.descendants, expected_descendants
        ));
    }
    Ok(())
}

fn validate_catalog(
    catalog: &RootResidualCatalog,
    decimals: u32,
    require_reviewed: bool,
) -> Result<(), String> {
    if catalog.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "root residual catalog schema {} is unsupported; expected {SCHEMA_VERSION}",
            catalog.schema_version
        ));
    }
    let expected_contract = RootResidualContract::current(decimals);
    if catalog.contract != expected_contract {
        return Err(format!(
            "root residual catalog contract drifted; expected {expected_contract:?}, found {:?}",
            catalog.contract
        ));
    }

    let primary_diagrams = crate::cmd::primary_svg_matrix_diagrams().collect::<BTreeSet<_>>();
    let mut previous: Option<(&str, &str)> = None;
    let mut used_evidence = BTreeSet::new();
    for entry in &catalog.entries {
        let key = entry.key();
        if previous.is_some_and(|previous| previous >= key) {
            return Err(format!(
                "root residual entries must be unique and sorted, found {}/{} after {:?}",
                entry.diagram, entry.fixture, previous
            ));
        }
        previous = Some(key);
        if !primary_diagrams.contains(entry.diagram.as_str()) {
            return Err(format!(
                "root residual {}/{} names a non-primary diagram",
                entry.diagram, entry.fixture
            ));
        }
        if !catalog.evidence.contains_key(&entry.evidence_id) {
            return Err(format!(
                "root residual {}/{} references missing evidence `{}`",
                entry.diagram, entry.fixture, entry.evidence_id
            ));
        }
        used_evidence.insert(entry.evidence_id.as_str());
    }

    for (evidence_id, evidence) in &catalog.evidence {
        if !used_evidence.contains(evidence_id.as_str()) {
            return Err(format!(
                "root residual catalog contains unused evidence `{evidence_id}`"
            ));
        }
        validate_evidence_reference(evidence_id, evidence)?;
    }

    for entry in &catalog.entries {
        let evidence = catalog.evidence.get(&entry.evidence_id).ok_or_else(|| {
            format!(
                "root residual {}/{} references missing evidence `{}`",
                entry.diagram, entry.fixture, entry.evidence_id
            )
        })?;
        if require_reviewed && evidence.kind == RootResidualEvidenceKind::Unreviewed {
            return Err(format!(
                "root residual {}/{} is still unreviewed",
                entry.diagram, entry.fixture
            ));
        }
        validate_evidence_profile(entry, evidence, require_reviewed)?;
        validate_entry_artifacts(entry)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CatalogMode {
    Verify,
    Candidate,
}

#[derive(Debug)]
pub(crate) struct RootParityResidualPolicy {
    mode: CatalogMode,
    decimals: u32,
    expected: BTreeMap<(String, String), RootResidualEntry>,
    observed: BTreeMap<(String, String), RootResidualObservation>,
}

#[derive(Debug, Default)]
pub(crate) struct RootParityPolicyFinish {
    pub(crate) accepted_summaries: Vec<String>,
    pub(crate) failures: Vec<String>,
    pub(crate) candidate_path: Option<PathBuf>,
}

impl RootParityResidualPolicy {
    pub(crate) fn verify(diagrams: &[&str], decimals: u32) -> Result<Self, String> {
        let selected = diagrams.iter().copied().collect::<BTreeSet<_>>();
        let expected = load_catalog(decimals)?
            .entries
            .into_iter()
            .filter(|entry| selected.contains(entry.diagram.as_str()))
            .map(|entry| ((entry.diagram.clone(), entry.fixture.clone()), entry))
            .collect();
        Ok(Self {
            mode: CatalogMode::Verify,
            decimals,
            expected,
            observed: BTreeMap::new(),
        })
    }

    pub(crate) fn candidate(decimals: u32) -> Self {
        Self {
            mode: CatalogMode::Candidate,
            decimals,
            expected: BTreeMap::new(),
            observed: BTreeMap::new(),
        }
    }

    pub(crate) fn accept_or_summarize_failure(
        &mut self,
        diagram: &str,
        msg: &str,
        report_path: Option<&Path>,
    ) -> Option<String> {
        let mut remaining = Vec::new();
        for line in msg.lines().filter(|line| !line.trim().is_empty()) {
            match observe_root_residual(diagram, line, self.decimals) {
                Ok(Some(observation)) => {
                    if !self.accept(observation) {
                        remaining.push(line.to_string());
                    }
                }
                Ok(None) => remaining.push(line.to_string()),
                Err(error) => {
                    remaining.push(format!("{line} [catalog evaluation rejected: {error}]"))
                }
            }
        }

        (!remaining.is_empty())
            .then(|| summarize_root_parity_failure(diagram, &remaining.join("\n"), report_path))
    }

    fn accept(&mut self, observation: RootResidualObservation) -> bool {
        let key = (observation.diagram.clone(), observation.fixture.clone());
        if self.observed.contains_key(&key) {
            return false;
        }
        if self.mode == CatalogMode::Verify
            && !self
                .expected
                .get(&key)
                .is_some_and(|entry| entry.matches(&observation))
        {
            return false;
        }
        self.observed.insert(key, observation);
        true
    }

    pub(crate) fn finish(self) -> Result<RootParityPolicyFinish, String> {
        let accepted_summaries = summarize_observations(&self.observed);
        match self.mode {
            CatalogMode::Verify => {
                let mut missing_by_diagram: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
                for ((diagram, fixture), _) in self
                    .expected
                    .iter()
                    .filter(|(key, _)| !self.observed.contains_key(*key))
                {
                    missing_by_diagram
                        .entry(diagram.as_str())
                        .or_default()
                        .push(fixture.as_str());
                }
                let failures = missing_by_diagram
                    .into_iter()
                    .map(|(diagram, fixtures)| {
                        format!(
                            "root parity residual catalog expected {} observation(s) for {diagram} but they were not seen; first={}/{}; refresh only with fresh pinned-source evidence",
                            fixtures.len(),
                            diagram,
                            fixtures[0]
                        )
                    })
                    .collect();
                Ok(RootParityPolicyFinish {
                    accepted_summaries,
                    failures,
                    candidate_path: None,
                })
            }
            CatalogMode::Candidate => {
                let path = write_candidate_catalog(self.decimals, self.observed.into_values())?;
                Ok(RootParityPolicyFinish {
                    accepted_summaries,
                    failures: Vec::new(),
                    candidate_path: Some(path),
                })
            }
        }
    }
}

fn summarize_observations(
    observations: &BTreeMap<(String, String), RootResidualObservation>,
) -> Vec<String> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for observation in observations.values() {
        *counts.entry(&observation.diagram).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(diagram, count)| format!("- {diagram}: {count}"))
        .collect()
}

fn write_candidate_catalog(
    decimals: u32,
    observations: impl Iterator<Item = RootResidualObservation>,
) -> Result<PathBuf, String> {
    let entries = observations
        .map(RootResidualObservation::into_candidate_entry)
        .collect::<Vec<_>>();
    let mut evidence = BTreeMap::new();
    if !entries.is_empty() {
        evidence.insert(
            "unreviewed".to_string(),
            RootResidualEvidence {
                kind: RootResidualEvidenceKind::Unreviewed,
                reference: "docs/workstreams/PARITY_BOUNDARY.md".to_string(),
                rationale: "Candidate only; classify every entry against pinned source and family evidence before acceptance."
                    .to_string(),
            },
        );
    }
    let catalog = RootResidualCatalog {
        schema_version: SCHEMA_VERSION,
        contract: RootResidualContract::current(decimals),
        evidence,
        entries,
    };
    validate_catalog(&catalog, decimals, false)?;
    let mut json = serde_json::to_string_pretty(&catalog)
        .map_err(|error| format!("serialize root residual candidate: {error}"))?;
    json.push('\n');
    let path = candidate_path();
    fs::write(&path, json)
        .map_err(|error| format!("write root residual candidate {}: {error}", path.display()))?;
    Ok(path)
}

fn summarize_root_parity_failure(diagram: &str, msg: &str, report_path: Option<&Path>) -> String {
    let lines = msg
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let mismatch_count = lines
        .iter()
        .filter(|line| line.trim_start().starts_with("dom mismatch for "))
        .count();
    let count = if mismatch_count > 0 {
        mismatch_count
    } else {
        lines.len()
    };
    let first = lines.first().copied().unwrap_or("no mismatch details");
    let report = report_path
        .map(|path| format!("; report={}", path.display()))
        .unwrap_or_default();
    format!("{diagram}: {count} unaccepted parity-root DOM mismatch(es){report}; first: {first}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const REVIEWED_REFERENCE: &str = "docs/workstreams/PARITY_BOUNDARY.md";

    fn observation(fixture: &str, local_width: &str) -> RootResidualObservation {
        RootResidualObservation {
            diagram: "flowchart".to_string(),
            fixture: fixture.to_string(),
            descendants: DescendantProfile::Parity,
            input_sha256: "input".to_string(),
            upstream_svg_sha256: "upstream".to_string(),
            upstream: RootViewportSignature {
                style: Some("max-width: 100px".to_string()),
                view_box: Some("0 0 100 100".to_string()),
                width: Some("100%".to_string()),
                height: None,
            },
            local: RootViewportSignature {
                style: Some(format!("max-width: {local_width}px")),
                view_box: Some(format!("0 0 {local_width} 100")),
                width: Some("100%".to_string()),
                height: None,
            },
        }
    }

    fn entry_with_evidence(
        observation: RootResidualObservation,
        evidence_id: &str,
    ) -> RootResidualEntry {
        RootResidualEntry {
            diagram: observation.diagram,
            fixture: observation.fixture,
            descendants: observation.descendants,
            input_sha256: observation.input_sha256,
            upstream_svg_sha256: observation.upstream_svg_sha256,
            upstream: observation.upstream,
            local: observation.local,
            evidence_id: evidence_id.to_string(),
        }
    }

    fn entry(observation: RootResidualObservation) -> RootResidualEntry {
        entry_with_evidence(observation, "browser-measurement")
    }

    fn catalog_entry(
        diagram: &str,
        fixture: &str,
        descendants: DescendantProfile,
        evidence_id: &str,
    ) -> RootResidualEntry {
        let fixture_path = crate::cmd::fixtures_root()
            .join(diagram)
            .join(format!("{fixture}.mmd"));
        let upstream_path = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join(diagram)
            .join(format!("{fixture}.svg"));
        let (_, input_sha256) = read_hashed(&fixture_path).unwrap();
        let (_, upstream_svg_sha256) = read_hashed(&upstream_path).unwrap();
        RootResidualEntry {
            diagram: diagram.to_string(),
            fixture: fixture.to_string(),
            descendants,
            input_sha256,
            upstream_svg_sha256,
            upstream: RootViewportSignature {
                style: Some("max-width: 100px".to_string()),
                view_box: Some("0 0 100 100".to_string()),
                width: Some("100%".to_string()),
                height: None,
            },
            local: RootViewportSignature {
                style: Some("max-width: 101px".to_string()),
                view_box: Some("0 0 101 100".to_string()),
                width: Some("100%".to_string()),
                height: None,
            },
            evidence_id: evidence_id.to_string(),
        }
    }

    fn reviewed_evidence(kind: RootResidualEvidenceKind) -> RootResidualEvidence {
        RootResidualEvidence {
            kind,
            reference: REVIEWED_REFERENCE.to_string(),
            rationale: "Pinned source and family evidence isolate this root-only residual."
                .to_string(),
        }
    }

    fn reviewed_catalog(
        entry: RootResidualEntry,
        kind: RootResidualEvidenceKind,
    ) -> RootResidualCatalog {
        RootResidualCatalog {
            schema_version: SCHEMA_VERSION,
            contract: RootResidualContract::current(3),
            evidence: [(entry.evidence_id.clone(), reviewed_evidence(kind))].into(),
            entries: vec![entry],
        }
    }

    #[test]
    fn entry_matching_is_exact_for_root_values_and_hashes() {
        let expected = entry(observation("basic", "101"));
        assert!(expected.matches(&observation("basic", "101")));
        assert!(!expected.matches(&observation("basic", "102")));

        let mut changed_hash = observation("basic", "101");
        changed_hash.input_sha256 = "changed".to_string();
        assert!(!expected.matches(&changed_hash));
    }

    #[test]
    fn policy_rejects_new_changed_duplicate_and_missing_observations() {
        let expected_observation = observation("basic", "101");
        let key = ("flowchart".to_string(), "basic".to_string());
        let mut policy = RootParityResidualPolicy {
            mode: CatalogMode::Verify,
            decimals: 3,
            expected: [(key, entry(expected_observation.clone()))].into(),
            observed: BTreeMap::new(),
        };

        assert!(!policy.accept(observation("new", "101")));
        assert!(!policy.accept(observation("basic", "102")));
        assert!(policy.accept(expected_observation.clone()));
        assert!(!policy.accept(expected_observation));
        assert!(policy.finish().unwrap().failures.is_empty());

        let missing = RootParityResidualPolicy {
            mode: CatalogMode::Verify,
            decimals: 3,
            expected: [(
                ("flowchart".to_string(), "basic".to_string()),
                entry(observation("basic", "101")),
            )]
            .into(),
            observed: BTreeMap::new(),
        }
        .finish()
        .unwrap();
        assert_eq!(missing.failures.len(), 1);
        assert!(missing.failures[0].contains("flowchart/basic"));
    }

    #[test]
    fn catalog_validation_rejects_unreviewed_entries() {
        let entry = catalog_entry(
            "flowchart",
            "basic",
            DescendantProfile::Parity,
            "unreviewed",
        );
        let catalog = RootResidualCatalog {
            schema_version: SCHEMA_VERSION,
            contract: RootResidualContract::current(3),
            evidence: [(
                "unreviewed".to_string(),
                RootResidualEvidence {
                    kind: RootResidualEvidenceKind::Unreviewed,
                    reference: "docs/workstreams/PARITY_BOUNDARY.md".to_string(),
                    rationale: "candidate".to_string(),
                },
            )]
            .into(),
            entries: vec![entry],
        };

        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("unreviewed")
        );
    }

    #[test]
    fn catalog_validation_rejects_unsorted_entries() {
        let mut first = catalog_entry(
            "flowchart",
            "upstream_docs_flowchart_chaining_of_links_159",
            DescendantProfile::Parity,
            "browser-measurement",
        );
        let second = catalog_entry(
            "flowchart",
            "basic",
            DescendantProfile::Parity,
            "browser-measurement",
        );
        first.evidence_id = second.evidence_id.clone();
        let mut catalog = reviewed_catalog(first, RootResidualEvidenceKind::BrowserMeasurement);
        catalog.entries.push(second);
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("sorted")
        );
    }

    #[test]
    fn reviewed_catalog_accepts_real_fixture_hashes_and_profile() {
        let catalog = reviewed_catalog(
            catalog_entry(
                "flowchart",
                "basic",
                DescendantProfile::Parity,
                "browser-measurement",
            ),
            RootResidualEvidenceKind::BrowserMeasurement,
        );

        validate_catalog(&catalog, 3, true).unwrap();
    }

    #[test]
    fn catalog_validation_rejects_noncanonical_sha256_values() {
        let mut catalog = reviewed_catalog(
            catalog_entry(
                "flowchart",
                "basic",
                DescendantProfile::Parity,
                "browser-measurement",
            ),
            RootResidualEvidenceKind::BrowserMeasurement,
        );
        catalog.entries[0].input_sha256 = "A".repeat(64);
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("64 lowercase hexadecimal")
        );

        catalog.entries[0].input_sha256 = "a".repeat(64);
        catalog.entries[0].upstream_svg_sha256 = "b".repeat(63);
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("64 lowercase hexadecimal")
        );
    }

    #[test]
    fn catalog_validation_rejects_blank_and_unused_evidence() {
        let mut catalog = reviewed_catalog(
            catalog_entry(
                "flowchart",
                "basic",
                DescendantProfile::Parity,
                "browser-measurement",
            ),
            RootResidualEvidenceKind::BrowserMeasurement,
        );
        catalog
            .evidence
            .get_mut("browser-measurement")
            .unwrap()
            .rationale = " \n\t ".to_string();
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("empty rationale")
        );

        catalog
            .evidence
            .get_mut("browser-measurement")
            .unwrap()
            .rationale = "reviewed".to_string();
        catalog.evidence.insert(
            "unused".to_string(),
            reviewed_evidence(RootResidualEvidenceKind::BrowserMeasurement),
        );
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("unused")
        );
    }

    #[test]
    fn catalog_validation_enforces_evidence_kind_and_descendant_profile() {
        let structure_entry = catalog_entry(
            "ishikawa",
            "upstream_cypress_ishikawa_spec_6_should_render_with_handdrawn_look_006",
            DescendantProfile::Structure,
            "reviewed",
        );
        validate_catalog(
            &reviewed_catalog(
                structure_entry.clone(),
                RootResidualEvidenceKind::RoughJsImplementation,
            ),
            3,
            true,
        )
        .unwrap();

        for kind in [
            RootResidualEvidenceKind::BrowserMeasurement,
            RootResidualEvidenceKind::SourceBackedLayoutApproximation,
        ] {
            assert!(
                validate_catalog(&reviewed_catalog(structure_entry.clone(), kind), 3, true,)
                    .unwrap_err()
                    .contains("incompatible")
            );
        }

        let parity_entry =
            catalog_entry("flowchart", "basic", DescendantProfile::Parity, "reviewed");
        assert!(
            validate_catalog(
                &reviewed_catalog(
                    parity_entry,
                    RootResidualEvidenceKind::RoughJsImplementation,
                ),
                3,
                true,
            )
            .unwrap_err()
            .contains("incompatible")
        );
    }

    #[test]
    fn catalog_validation_rejects_reference_and_fixture_path_escape() {
        let mut catalog = reviewed_catalog(
            catalog_entry(
                "flowchart",
                "basic",
                DescendantProfile::Parity,
                "browser-measurement",
            ),
            RootResidualEvidenceKind::BrowserMeasurement,
        );
        catalog
            .evidence
            .get_mut("browser-measurement")
            .unwrap()
            .reference = "/etc/hosts".to_string();
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("escapes workspace")
        );

        catalog
            .evidence
            .get_mut("browser-measurement")
            .unwrap()
            .reference = REVIEWED_REFERENCE.to_string();
        catalog.entries[0].fixture = "../../Cargo".to_string();
        assert!(validate_catalog(&catalog, 3, true).is_err());
    }

    #[test]
    fn catalog_validation_rejects_fixture_profile_and_file_hash_drift() {
        let mut catalog = reviewed_catalog(
            catalog_entry(
                "flowchart",
                "basic",
                DescendantProfile::Parity,
                "browser-measurement",
            ),
            RootResidualEvidenceKind::BrowserMeasurement,
        );
        catalog.entries[0].descendants = DescendantProfile::Structure;
        catalog
            .evidence
            .get_mut("browser-measurement")
            .unwrap()
            .kind = RootResidualEvidenceKind::RoughJsImplementation;
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("fixture profile")
        );

        catalog.entries[0].descendants = DescendantProfile::Parity;
        catalog
            .evidence
            .get_mut("browser-measurement")
            .unwrap()
            .kind = RootResidualEvidenceKind::BrowserMeasurement;
        catalog.entries[0].input_sha256 = "a".repeat(64);
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("input SHA-256 drifted")
        );

        let (_, input_sha256) =
            read_hashed(&crate::cmd::fixtures_root().join("flowchart/basic.mmd")).unwrap();
        catalog.entries[0].input_sha256 = input_sha256;
        catalog.entries[0].upstream_svg_sha256 = "b".repeat(64);
        assert!(
            validate_catalog(&catalog, 3, true)
                .unwrap_err()
                .contains("upstream SVG SHA-256 drifted")
        );
    }

    #[test]
    fn mismatch_locator_ignores_rendered_detail_and_keeps_paths() {
        let locator = parse_dom_mismatch_locator(
            "dom mismatch for basic: upstream=/tmp/up.svg local=/tmp/local.svg (scope=parity-normalized-descendants-match; arbitrary detail)",
        )
        .unwrap();
        assert_eq!(locator.fixture, "basic");
        assert_eq!(locator.upstream_path, PathBuf::from("/tmp/up.svg"));
        assert_eq!(locator.local_path, PathBuf::from("/tmp/local.svg"));
    }

    #[test]
    fn failure_summary_is_bounded() {
        let summary = summarize_root_parity_failure(
            "flowchart",
            "dom mismatch for a: upstream=a local=b (first)\ndom mismatch for b: upstream=a local=b (second)",
            Some(Path::new("target/compare/flowchart_report_parity_root.md")),
        );
        assert!(summary.contains("2 unaccepted"));
        assert!(summary.contains("first: dom mismatch for a"));
        assert!(!summary.contains("dom mismatch for b"));
    }

    #[test]
    fn candidate_acceptance_requires_one_explicit_sha256() {
        let digest = "a".repeat(64);
        assert_eq!(
            parse_accept_candidate_args(vec!["--sha256".to_string(), digest.clone()]).unwrap(),
            digest
        );
        assert!(parse_accept_candidate_args(Vec::new()).is_err());
        assert!(
            parse_accept_candidate_args(vec!["--sha256".to_string(), "short".to_string()]).is_err()
        );
        assert!(parse_accept_candidate_args(vec!["--sha256".to_string(), "A".repeat(64)]).is_err());
    }
}
