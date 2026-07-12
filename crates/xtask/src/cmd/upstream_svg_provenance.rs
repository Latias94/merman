//! Provenance manifests for pinned upstream Mermaid SVG baselines.

mod family_lock;
mod schema;
mod snapshot;
mod transaction;

#[allow(unused_imports)]
pub(crate) use family_lock::{UpstreamSvgFamilyLock, UpstreamSvgToolchainLock};
pub(crate) use family_lock::{
    acquire_upstream_svg_family_lock, acquire_upstream_svg_family_lock_with_timeout,
    acquire_upstream_svg_family_locks, acquire_upstream_svg_toolchain_lock,
};
pub(crate) use schema::UpstreamSvgRenderEnvironment;
#[cfg(test)]
pub(crate) use schema::{
    UpstreamSvgBrowserEnvironment, UpstreamSvgFontProbeEnvironment,
    UpstreamSvgOperatingSystemEnvironment, UpstreamSvgPuppeteerEnvironment,
    UpstreamSvgRuntimeEnvironment,
};
#[allow(unused_imports)]
pub(crate) use snapshot::UpstreamSvgFixtureSnapshots;
pub(crate) use snapshot::{
    CapturedUpstreamSvgExclusion, CapturedUpstreamSvgFixture,
    capture_upstream_svg_fixture_selection,
};

use crate::XtaskError;
use family_lock::acquire_upstream_svg_family_locks_with_timeout;
use regex::Regex;
use schema::*;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
#[cfg(test)]
use transaction::{write_manifest, write_manifest_batch_with_installer};
use transaction::{write_manifest_batch, write_manifest_with_post_install_validator};

const MANIFEST_FILE_NAME: &str = "_baseline-manifest.json";
const MANIFEST_SCHEMA_VERSION: u32 = 2;
const PINNED_MERMAID_VERSION: &str = "11.16.0";
const PINNED_MERMAID_CLI_VERSION: &str = "11.16.0";
const MERMAID_SOURCE_TAG: &str = "mermaid@11.16.0";
const MERMAID_SOURCE_COMMIT: &str = "7c0cafcf42e76bfaf79d0cbbd12edb986612f014";
const RENDERER_REVISION: &str = "xtask-upstream-svg-v2";
const PACKAGE_JSON_SHA256: &str =
    "b5a4bb3b8d8b8d6d535fe237c9ad8a0c7cb8a8e1d87a8a0710f3dbb8b05e85b7";
const PACKAGE_LOCK_SHA256: &str =
    "0303e5502127385caf6808e56be6390836e6c807119c8d1e95f7988ebd79f77e";
const MERMAID_CONFIG_SHA256: &str =
    "da34e9d1dae1882d3b32a479e6223bad495f31877e6d0a3f0a3e3a157832eacc";
const PARSER_ONLY_EXCLUSION_REASON: &str =
    "parser-only fixture is intentionally excluded from upstream SVG baselines";

#[derive(Debug)]
struct CompleteCorpus {
    fixtures: BTreeMap<String, PathBuf>,
    excluded: BTreeMap<String, (PathBuf, String)>,
    svgs: BTreeMap<String, PathBuf>,
}

fn register_case_insensitive_stem(
    seen: &mut BTreeMap<String, String>,
    stem: &str,
    kind: &str,
    diagram: &str,
) -> Result<(), XtaskError> {
    let folded = stem.to_ascii_lowercase();
    if let Some(previous) = seen.insert(folded, stem.to_string())
        && previous != stem
    {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "case-insensitive {kind} stem collision for {diagram}: {previous:?} and {stem:?}"
        )));
    }
    Ok(())
}

pub(crate) fn upstream_svg_fixture_exclusion_reason(
    diagram: &str,
    fixture_path: &Path,
) -> Result<Option<String>, XtaskError> {
    let stem = fixture_stem(fixture_path)?;
    if let Some(reason) = crate::cmd::upstream_svg_baseline_skip_reason(diagram, stem) {
        return Ok(Some(reason.to_string()));
    }
    if crate::cmd::is_parser_only_fixture(fixture_path) {
        return Ok(Some(PARSER_ONLY_EXCLUSION_REASON.to_string()));
    }
    Ok(None)
}

pub(crate) fn collect_upstream_svg_generation_deletions(
    out_dir: &Path,
    generated_fixtures: &[CapturedUpstreamSvgFixture],
    excluded_fixtures: &[CapturedUpstreamSvgExclusion],
    full_generation: bool,
) -> Result<Vec<PathBuf>, XtaskError> {
    if !full_generation {
        return Ok(excluded_fixtures
            .iter()
            .map(|exclusion| out_dir.join(format!("{}.svg", exclusion.fixture().stem())))
            .collect());
    }

    let renderable_stems = generated_fixtures
        .iter()
        .map(CapturedUpstreamSvgFixture::stem)
        .collect::<BTreeSet<_>>();
    let entries = fs::read_dir(out_dir).map_err(|source| XtaskError::ReadFile {
        path: out_dir.display().to_string(),
        source,
    })?;
    let mut deletions = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: out_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
        {
            continue;
        }
        let stem = fixture_stem(&path)?;
        #[cfg(windows)]
        let is_renderable = renderable_stems
            .iter()
            .any(|renderable| renderable.eq_ignore_ascii_case(stem));
        #[cfg(not(windows))]
        let is_renderable = renderable_stems.contains(stem);
        if !is_renderable {
            deletions.insert(path);
        }
    }
    Ok(deletions.into_iter().collect())
}

fn collect_complete_corpus(
    diagram: &str,
    fixtures_dir: &Path,
    upstream_dir: &Path,
) -> Result<CompleteCorpus, XtaskError> {
    if !fixtures_dir.is_dir() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "missing upstream SVG fixture directory for {diagram}: {}",
            fixtures_dir.display()
        )));
    }
    if !upstream_dir.is_dir() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "missing upstream SVG baseline directory for {diagram}: {}",
            upstream_dir.display()
        )));
    }

    let fixture_paths = crate::cmd::list_mmd_fixtures_in_dir(fixtures_dir, None, false);
    if fixture_paths.is_empty() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "no .mmd fixtures found under {}",
            fixtures_dir.display()
        )));
    }

    let mut fixtures = BTreeMap::new();
    let mut excluded = BTreeMap::new();
    let mut fixture_stems = BTreeMap::new();
    for fixture_path in fixture_paths {
        let stem = fixture_stem(&fixture_path)?.to_string();
        register_case_insensitive_stem(&mut fixture_stems, &stem, "fixture", diagram)?;
        if let Some(reason) = upstream_svg_fixture_exclusion_reason(diagram, &fixture_path)? {
            if excluded
                .insert(stem.clone(), (fixture_path, reason))
                .is_some()
            {
                return Err(XtaskError::UpstreamSvgFailed(format!(
                    "duplicate excluded fixture stem for {diagram}: {stem}"
                )));
            }
        } else if fixtures.insert(stem.clone(), fixture_path).is_some() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "duplicate fixture stem for {diagram}: {stem}"
            )));
        }
    }

    let entries = fs::read_dir(upstream_dir).map_err(|source| XtaskError::ReadFile {
        path: upstream_dir.display().to_string(),
        source,
    })?;
    let mut svgs = BTreeMap::new();
    let mut svg_stems = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: upstream_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
        {
            continue;
        }
        let stem = fixture_stem(&path)?.to_string();
        register_case_insensitive_stem(&mut svg_stems, &stem, "SVG", diagram)?;
        if svgs.insert(stem.clone(), path).is_some() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "duplicate SVG stem for {diagram}: {stem}"
            )));
        }
    }

    let excluded_residue: Vec<_> = excluded
        .keys()
        .filter(|stem| svgs.contains_key(*stem))
        .cloned()
        .collect();
    if !excluded_residue.is_empty() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "excluded fixture SVG residue for {diagram}: {}",
            excluded_residue.join(", ")
        )));
    }

    let expected: BTreeSet<_> = fixtures.keys().cloned().collect();
    let actual: BTreeSet<_> = svgs.keys().cloned().collect();
    let missing: Vec<_> = expected.difference(&actual).cloned().collect();
    let extra: Vec<_> = actual.difference(&expected).cloned().collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG corpus set mismatch for {diagram}: missing=[{}], extra=[{}]",
            missing.join(", "),
            extra.join(", ")
        )));
    }

    Ok(CompleteCorpus {
        fixtures,
        excluded,
        svgs,
    })
}

pub(crate) fn upstream_svg_id(stem: &str) -> String {
    let mut id = String::with_capacity(stem.len());
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            id.push(ch);
        } else {
            id.push('_');
        }
    }
    if id.is_empty() {
        "diagram".to_string()
    } else {
        id
    }
}

fn validate_mermaid_1116_svg(stem: &str, svg_path: &Path) -> Result<(), XtaskError> {
    let svg = fs::read_to_string(svg_path).map_err(|source| XtaskError::ReadFile {
        path: svg_path.display().to_string(),
        source,
    })?;
    if svg.trim().is_empty() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "empty upstream SVG for {stem}: {}",
            svg_path.display()
        )));
    }

    let document = roxmltree::Document::parse(&svg).map_err(|err| {
        XtaskError::UpstreamSvgFailed(format!(
            "invalid upstream SVG XML for {stem} at {}: {err}",
            svg_path.display()
        ))
    })?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG root element for {stem} is not <svg>: {}",
            svg_path.display()
        )));
    }

    let expected_id = upstream_svg_id(stem);
    let actual_id = root.attribute("id").unwrap_or_default();
    if actual_id != expected_id {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG root id mismatch for {stem}: actual={actual_id:?}, expected={expected_id:?}"
        )));
    }

    let marker_pattern = format!(
        r#"(?s){}\s+\[data-look\s*=\s*["']neo["']\]\.swimlane\.cluster\s+rect\s*\{{\s*filter\s*:\s*none\s*;?\s*\}}"#,
        regex::escape(&format!("#{expected_id}"))
    );
    let marker = Regex::new(&marker_pattern).map_err(|err| {
        XtaskError::UpstreamSvgFailed(format!("failed to compile SVG marker validator: {err}"))
    })?;
    let has_marker = root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "style")
        .filter_map(|node| node.text())
        .any(|style| marker.is_match(style));
    if !has_marker {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG for {stem} is missing the Mermaid 11.16 global style marker: {}",
            svg_path.display()
        )));
    }

    Ok(())
}

fn build_complete_manifest_with_source(
    diagram: &str,
    fixtures_dir: &Path,
    upstream_dir: &Path,
    source: UpstreamSvgSource,
    attestation: UpstreamSvgAttestation,
) -> Result<UpstreamSvgManifest, XtaskError> {
    attestation.validate()?;
    let CompleteCorpus {
        fixtures,
        excluded,
        svgs,
    } = collect_complete_corpus(diagram, fixtures_dir, upstream_dir)?;
    let profile = renderer_profile(diagram).to_string();
    let mut manifest = UpstreamSvgManifest::empty(source, attestation);
    manifest.complete = true;

    for (stem, fixture_path) in fixtures {
        let svg_path = svgs
            .get(&stem)
            .expect("complete corpus contains every renderable SVG");
        validate_mermaid_1116_svg(&stem, svg_path)?;
        manifest.fixtures.insert(
            stem,
            UpstreamSvgFixtureProvenance {
                input_sha256: hash_file(&fixture_path)?,
                svg_sha256: hash_file(svg_path)?,
                renderer_profile: profile.clone(),
            },
        );
    }
    for (stem, (fixture_path, reason)) in excluded {
        manifest.excluded.insert(
            stem,
            UpstreamSvgExcludedFixture {
                input_sha256: hash_file(&fixture_path)?,
                reason,
            },
        );
    }

    Ok(manifest)
}

fn captured_renderable_by_stem<'a>(
    diagram: &str,
    fixtures: &'a [CapturedUpstreamSvgFixture],
) -> Result<BTreeMap<&'a str, &'a CapturedUpstreamSvgFixture>, XtaskError> {
    let mut captured = BTreeMap::new();
    for fixture in fixtures {
        if captured.insert(fixture.stem(), fixture).is_some() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "duplicate captured upstream SVG fixture for {diagram}: {}",
                fixture.stem()
            )));
        }
    }
    Ok(captured)
}

fn captured_excluded_by_stem<'a>(
    diagram: &str,
    fixtures: &'a [CapturedUpstreamSvgExclusion],
) -> Result<BTreeMap<&'a str, &'a CapturedUpstreamSvgExclusion>, XtaskError> {
    let mut captured = BTreeMap::new();
    for exclusion in fixtures {
        if captured
            .insert(exclusion.fixture().stem(), exclusion)
            .is_some()
        {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "duplicate captured upstream SVG exclusion for {diagram}: {}",
                exclusion.fixture().stem()
            )));
        }
    }
    Ok(captured)
}

fn build_complete_generated_manifest_with_source(
    diagram: &str,
    fixtures_dir: &Path,
    upstream_dir: &Path,
    generated_fixtures: &[CapturedUpstreamSvgFixture],
    excluded_fixtures: &[CapturedUpstreamSvgExclusion],
    source: UpstreamSvgSource,
    attestation: UpstreamSvgAttestation,
) -> Result<UpstreamSvgManifest, XtaskError> {
    attestation.validate()?;
    let CompleteCorpus {
        fixtures,
        excluded,
        svgs,
    } = collect_complete_corpus(diagram, fixtures_dir, upstream_dir)?;
    let generated = captured_renderable_by_stem(diagram, generated_fixtures)?;
    let captured_excluded = captured_excluded_by_stem(diagram, excluded_fixtures)?;
    let live_renderable: BTreeSet<_> = fixtures.keys().map(String::as_str).collect();
    let captured_renderable: BTreeSet<_> = generated.keys().copied().collect();
    let live_excluded: BTreeSet<_> = excluded.keys().map(String::as_str).collect();
    let captured_excluded_stems: BTreeSet<_> = captured_excluded.keys().copied().collect();
    if live_renderable != captured_renderable || live_excluded != captured_excluded_stems {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG fixture selection changed after snapshot capture for {diagram}; rerun generation"
        )));
    }

    let profile = renderer_profile(diagram).to_string();
    let mut manifest = UpstreamSvgManifest::empty(source, attestation);
    manifest.complete = true;
    for (stem, fixture_path) in fixtures {
        let evidence = generated
            .get(stem.as_str())
            .expect("captured renderable set matches the complete corpus");
        if evidence.live_path() != fixture_path {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "upstream SVG fixture path changed after snapshot capture for {diagram}/{stem}"
            )));
        }
        evidence.validate_captured_hashes()?;
        let svg_path = svgs
            .get(&stem)
            .expect("complete corpus contains every renderable SVG");
        validate_mermaid_1116_svg(&stem, svg_path)?;
        manifest.fixtures.insert(
            stem,
            UpstreamSvgFixtureProvenance {
                input_sha256: evidence.input_sha256().to_string(),
                svg_sha256: hash_file(svg_path)?,
                renderer_profile: profile.clone(),
            },
        );
    }
    for (stem, (fixture_path, reason)) in excluded {
        let evidence = captured_excluded
            .get(stem.as_str())
            .expect("captured exclusion set matches the complete corpus");
        if evidence.fixture().live_path() != fixture_path || evidence.reason() != reason {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "upstream SVG exclusion changed after snapshot capture for {diagram}/{stem}"
            )));
        }
        evidence.fixture().validate_captured_hashes()?;
        manifest.excluded.insert(
            stem,
            UpstreamSvgExcludedFixture {
                input_sha256: evidence.fixture().input_sha256().to_string(),
                reason,
            },
        );
    }
    Ok(manifest)
}

pub(crate) struct UpstreamSvgProvenanceValidator {
    diagram: String,
    manifest: UpstreamSvgManifest,
}

impl UpstreamSvgProvenanceValidator {
    pub(crate) fn require_same_generated_environment(
        &self,
        generated: &Self,
    ) -> Result<(), XtaskError> {
        if self.diagram != generated.diagram {
            return Err(XtaskError::SvgCompareFailed(format!(
                "cannot compare upstream SVG render environments for different diagrams: {} and {}",
                self.diagram, generated.diagram
            )));
        }

        match (&self.manifest.attestation, &generated.manifest.attestation) {
            (
                UpstreamSvgAttestation::Generated {
                    render_environment: baseline,
                },
                UpstreamSvgAttestation::Generated {
                    render_environment: fresh,
                },
            ) if baseline == fresh => Ok(()),
            (
                UpstreamSvgAttestation::Generated { .. },
                UpstreamSvgAttestation::Generated { .. },
            ) => Err(XtaskError::SvgCompareFailed(format!(
                "fresh upstream SVG render environment differs from the generated baseline for {}; regenerate the complete family in one environment before comparing SVG output",
                self.diagram
            ))),
            (UpstreamSvgAttestation::AdoptedExisting, _) => {
                Err(XtaskError::SvgCompareFailed(format!(
                    "upstream SVG baseline for {} is adopted-existing and has no measured render environment; regenerate the complete family before running a fresh upstream check",
                    self.diagram
                )))
            }
            (_, UpstreamSvgAttestation::AdoptedExisting) => {
                Err(XtaskError::SvgCompareFailed(format!(
                    "fresh upstream SVG output for {} unexpectedly lacks a generated render-environment attestation",
                    self.diagram
                )))
            }
        }
    }

    pub(crate) fn validate_fixture(
        &self,
        fixture_path: &Path,
        svg_path: &Path,
    ) -> Result<(), String> {
        let stem = fixture_stem(fixture_path).map_err(|err| err.to_string())?;
        let entry = self.manifest.fixtures.get(stem).ok_or_else(|| {
            format!(
                "upstream SVG provenance for {}/{} is missing; regenerate the pinned Mermaid baseline",
                self.diagram, stem
            )
        })?;
        let expected_profile = renderer_profile(&self.diagram);
        if entry.renderer_profile != expected_profile {
            return Err(format!(
                "upstream SVG provenance profile mismatch for {}/{}: manifest={}, expected={expected_profile}",
                self.diagram, stem, entry.renderer_profile
            ));
        }
        validate_mermaid_1116_svg(stem, svg_path).map_err(|err| err.to_string())?;
        validate_hash(
            fixture_path,
            &entry.input_sha256,
            "input",
            &self.diagram,
            stem,
        )?;
        validate_hash(svg_path, &entry.svg_sha256, "SVG", &self.diagram, stem)
    }

    pub(crate) fn validate_excluded_fixture(
        &self,
        fixture_path: &Path,
        expected_reason: &str,
        svg_path: &Path,
    ) -> Result<(), String> {
        let stem = fixture_stem(fixture_path).map_err(|err| err.to_string())?;
        let entry = self.manifest.excluded.get(stem).ok_or_else(|| {
            format!(
                "upstream SVG exclusion provenance for {}/{} is missing; regenerate the pinned Mermaid baseline",
                self.diagram, stem
            )
        })?;
        if self.manifest.fixtures.contains_key(stem) {
            return Err(format!(
                "upstream SVG provenance for {}/{} records the fixture as both renderable and excluded",
                self.diagram, stem
            ));
        }
        if entry.reason != expected_reason {
            return Err(format!(
                "upstream SVG exclusion reason mismatch for {}/{}: manifest={:?}, expected={expected_reason:?}",
                self.diagram, stem, entry.reason
            ));
        }
        validate_hash(
            fixture_path,
            &entry.input_sha256,
            "excluded input",
            &self.diagram,
            stem,
        )?;
        match fs::symlink_metadata(svg_path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Ok(_) => Err(format!(
                "excluded fixture SVG residue for {}/{}: {}",
                self.diagram,
                stem,
                svg_path.display()
            )),
            Err(err) => Err(format!(
                "failed to verify excluded fixture SVG absence for {}/{} at {}: {err}",
                self.diagram,
                stem,
                svg_path.display()
            )),
        }
    }
}

pub(crate) fn load_upstream_svg_provenance(
    diagram: &str,
    fixtures_dir: &Path,
    upstream_dir: &Path,
    require_complete: bool,
) -> Result<UpstreamSvgProvenanceValidator, XtaskError> {
    load_upstream_svg_provenance_with_source(
        diagram,
        fixtures_dir,
        upstream_dir,
        require_complete,
        current_source()?,
    )
}

fn load_upstream_svg_provenance_with_source(
    diagram: &str,
    fixtures_dir: &Path,
    upstream_dir: &Path,
    require_complete: bool,
    current_source: UpstreamSvgSource,
) -> Result<UpstreamSvgProvenanceValidator, XtaskError> {
    let manifest_path = upstream_dir.join(MANIFEST_FILE_NAME);
    let manifest = read_manifest(&manifest_path)?.ok_or_else(|| {
        XtaskError::SvgCompareFailed(format!(
            "missing upstream SVG provenance manifest {}; regenerate the {diagram} baseline with pinned Mermaid 11.16.0",
            manifest_path.display()
        ))
    })?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(XtaskError::SvgCompareFailed(format!(
            "unsupported upstream SVG provenance schema {} in {} (expected {})",
            manifest.schema_version,
            manifest_path.display(),
            MANIFEST_SCHEMA_VERSION
        )));
    }
    if manifest.source != current_source {
        return Err(XtaskError::SvgCompareFailed(format!(
            "upstream SVG provenance source in {} does not match the pinned Mermaid toolchain; regenerate the baseline",
            manifest_path.display()
        )));
    }
    manifest.attestation.validate().map_err(|err| {
        XtaskError::SvgCompareFailed(format!(
            "invalid upstream SVG provenance attestation in {}: {err}",
            manifest_path.display()
        ))
    })?;
    if require_complete && !manifest.complete {
        return Err(XtaskError::SvgCompareFailed(format!(
            "upstream SVG provenance manifest {} was produced by a filtered generation and does not cover the complete {diagram} fixture family",
            manifest_path.display()
        )));
    }

    let validator = UpstreamSvgProvenanceValidator {
        diagram: diagram.to_string(),
        manifest,
    };
    if require_complete {
        validator.validate_complete_coverage(fixtures_dir, upstream_dir)?;
    }
    Ok(validator)
}

impl UpstreamSvgProvenanceValidator {
    fn validate_complete_coverage(
        &self,
        fixtures_dir: &Path,
        upstream_dir: &Path,
    ) -> Result<(), XtaskError> {
        let expected = build_complete_manifest_with_source(
            &self.diagram,
            fixtures_dir,
            upstream_dir,
            self.manifest.source.clone(),
            self.manifest.attestation.clone(),
        )
        .map_err(|err| XtaskError::SvgCompareFailed(err.to_string()))?;
        if expected == self.manifest {
            Ok(())
        } else {
            Err(XtaskError::SvgCompareFailed(format!(
                "complete upstream SVG provenance manifest drifted for {}; rebuild or re-adopt the full family",
                self.diagram
            )))
        }
    }
}

#[derive(Debug)]
pub(crate) struct UpstreamSvgProvenanceWriteRequest<'a> {
    pub(crate) diagram: &'a str,
    pub(crate) fixtures_dir: &'a Path,
    pub(crate) out_dir: &'a Path,
    pub(crate) generated_fixtures: &'a [CapturedUpstreamSvgFixture],
    pub(crate) excluded_fixtures: &'a [CapturedUpstreamSvgExclusion],
    pub(crate) full_generation: bool,
    pub(crate) fresh_output: bool,
    pub(crate) render_environment: UpstreamSvgRenderEnvironment,
}

pub(crate) fn write_upstream_svg_provenance<V>(
    request: UpstreamSvgProvenanceWriteRequest<'_>,
    validate_after_install: V,
) -> Result<(), XtaskError>
where
    V: FnOnce() -> Result<(), XtaskError>,
{
    let UpstreamSvgProvenanceWriteRequest {
        diagram,
        fixtures_dir,
        out_dir,
        generated_fixtures,
        excluded_fixtures,
        full_generation,
        fresh_output,
        render_environment,
    } = request;
    render_environment.validate()?;
    let source = current_source()?;
    if full_generation {
        let manifest = build_complete_generated_manifest_with_source(
            diagram,
            fixtures_dir,
            out_dir,
            generated_fixtures,
            excluded_fixtures,
            source,
            UpstreamSvgAttestation::generated(render_environment),
        )?;
        return write_manifest_with_post_install_validator(
            out_dir,
            &manifest,
            validate_after_install,
        );
    }

    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    let existing = read_manifest(&manifest_path)?;
    let mut manifest =
        prepare_generated_manifest(existing, &source, &render_environment, fresh_output)?;

    let profile = renderer_profile(diagram).to_string();
    for fixture in generated_fixtures {
        fixture.validate_captured_hashes()?;
        let stem = fixture.stem().to_string();
        let svg_path = out_dir.join(format!("{stem}.svg"));
        validate_mermaid_1116_svg(&stem, &svg_path)?;
        manifest.fixtures.insert(
            stem.clone(),
            UpstreamSvgFixtureProvenance {
                input_sha256: fixture.input_sha256().to_string(),
                svg_sha256: hash_file(&svg_path)?,
                renderer_profile: profile.clone(),
            },
        );
        manifest.excluded.remove(&stem);
    }
    for exclusion in excluded_fixtures {
        exclusion.fixture().validate_captured_hashes()?;
        let stem = exclusion.fixture().stem().to_string();
        manifest.excluded.insert(
            stem.clone(),
            UpstreamSvgExcludedFixture {
                input_sha256: exclusion.fixture().input_sha256().to_string(),
                reason: exclusion.reason().to_string(),
            },
        );
        manifest.fixtures.remove(&stem);
    }

    if manifest.complete {
        UpstreamSvgProvenanceValidator {
            diagram: diagram.to_string(),
            manifest: manifest.clone(),
        }
        .validate_complete_coverage(fixtures_dir, out_dir)?;
    }

    write_manifest_with_post_install_validator(out_dir, &manifest, validate_after_install)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpstreamSvgProvenanceWriteScope {
    Requested,
    CompleteGenerationRequired,
}

pub(crate) fn preflight_upstream_svg_provenance_write(
    out_dir: &Path,
    full_generation: bool,
    fresh_output: bool,
    render_environment: &UpstreamSvgRenderEnvironment,
) -> Result<UpstreamSvgProvenanceWriteScope, XtaskError> {
    preflight_upstream_svg_provenance_write_with_lock_timeout(
        out_dir,
        full_generation,
        fresh_output,
        render_environment,
        Duration::from_secs(30),
    )
}

fn preflight_upstream_svg_provenance_write_with_lock_timeout(
    out_dir: &Path,
    full_generation: bool,
    fresh_output: bool,
    render_environment: &UpstreamSvgRenderEnvironment,
    timeout: Duration,
) -> Result<UpstreamSvgProvenanceWriteScope, XtaskError> {
    let _family_lock = acquire_upstream_svg_family_lock_with_timeout(out_dir, timeout)?;
    preflight_upstream_svg_provenance_write_under_family_lock(
        out_dir,
        full_generation,
        fresh_output,
        render_environment,
    )
}

pub(crate) fn preflight_upstream_svg_provenance_write_under_family_lock(
    out_dir: &Path,
    full_generation: bool,
    fresh_output: bool,
    render_environment: &UpstreamSvgRenderEnvironment,
) -> Result<UpstreamSvgProvenanceWriteScope, XtaskError> {
    render_environment.validate()?;
    let source = current_source()?;
    if full_generation {
        return Ok(UpstreamSvgProvenanceWriteScope::Requested);
    }

    let existing = read_manifest(&out_dir.join(MANIFEST_FILE_NAME))?;
    if !fresh_output
        && existing.as_ref().is_some_and(|manifest| {
            manifest.schema_version == MANIFEST_SCHEMA_VERSION
                && manifest.source == source
                && matches!(
                    &manifest.attestation,
                    UpstreamSvgAttestation::AdoptedExisting
                )
        })
    {
        return Ok(UpstreamSvgProvenanceWriteScope::CompleteGenerationRequired);
    }
    prepare_generated_manifest(existing, &source, render_environment, fresh_output)
        .map(|_| UpstreamSvgProvenanceWriteScope::Requested)
}

fn prepare_generated_manifest(
    existing: Option<UpstreamSvgManifest>,
    source: &UpstreamSvgSource,
    render_environment: &UpstreamSvgRenderEnvironment,
    fresh_output: bool,
) -> Result<UpstreamSvgManifest, XtaskError> {
    if fresh_output {
        if existing.is_some() {
            return Err(XtaskError::UpstreamSvgFailed(
                "refusing fresh partial upstream SVG provenance because the output already has a manifest"
                    .to_string(),
            ));
        }
        render_environment.validate()?;
        return Ok(UpstreamSvgManifest::empty(
            source.clone(),
            UpstreamSvgAttestation::generated(render_environment.clone()),
        ));
    }

    prepare_partial_generated_manifest(existing, source, render_environment)
}

fn prepare_partial_generated_manifest(
    existing: Option<UpstreamSvgManifest>,
    source: &UpstreamSvgSource,
    render_environment: &UpstreamSvgRenderEnvironment,
) -> Result<UpstreamSvgManifest, XtaskError> {
    render_environment.validate()?;
    let manifest = existing.ok_or_else(|| {
        XtaskError::UpstreamSvgFailed(
            "refusing partial upstream SVG generation without an existing generated manifest; run a complete generation first"
                .to_string(),
        )
    })?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "refusing partial upstream SVG generation against schema {} provenance; schema {MANIFEST_SCHEMA_VERSION} with render-environment attestation is required",
            manifest.schema_version
        )));
    }
    if &manifest.source != source {
        return Err(XtaskError::UpstreamSvgFailed(
            "refusing partial upstream SVG generation against a different pinned Mermaid source"
                .to_string(),
        ));
    }
    manifest.attestation.validate()?;
    match &manifest.attestation {
        UpstreamSvgAttestation::Generated {
            render_environment: existing_environment,
        } if existing_environment.as_ref() == render_environment => {}
        UpstreamSvgAttestation::Generated { .. } => {
            return Err(XtaskError::UpstreamSvgFailed(
                "refusing partial upstream SVG generation because the render environment differs from the existing generated corpus; regenerate the complete family"
                    .to_string(),
            ));
        }
        UpstreamSvgAttestation::AdoptedExisting => {
            return Err(XtaskError::UpstreamSvgFailed(
                "refusing partial upstream SVG generation against adopted-existing provenance; regenerate the complete family to establish one render environment"
                    .to_string(),
            ));
        }
    }
    Ok(manifest)
}

fn adopt_existing_manifests_with_source<S: AsRef<str>>(
    diagrams: &[S],
    fixtures_root: &Path,
    upstream_root: &Path,
    source: UpstreamSvgSource,
    allow_generated_downgrade: bool,
) -> Result<(), XtaskError> {
    adopt_existing_manifests_with_source_and_lock_timeout(
        diagrams,
        fixtures_root,
        upstream_root,
        source,
        allow_generated_downgrade,
        Duration::from_secs(30),
    )
}

fn adopt_existing_manifests_with_source_and_lock_timeout<S: AsRef<str>>(
    diagrams: &[S],
    fixtures_root: &Path,
    upstream_root: &Path,
    source: UpstreamSvgSource,
    allow_generated_downgrade: bool,
    lock_timeout: Duration,
) -> Result<(), XtaskError> {
    let families: Vec<_> = diagrams
        .iter()
        .map(|diagram| {
            let diagram = diagram.as_ref().to_string();
            let upstream_dir = upstream_root.join(&diagram);
            (diagram, upstream_dir)
        })
        .collect();
    let lock_dirs: Vec<_> = families
        .iter()
        .map(|(_, upstream_dir)| upstream_dir.clone())
        .collect();
    let _family_locks = acquire_upstream_svg_family_locks_with_timeout(&lock_dirs, lock_timeout)?;

    let mut validated = Vec::with_capacity(families.len());
    for (diagram, upstream_dir) in families {
        let fixtures_dir = fixtures_root.join(&diagram);
        let manifest_path = upstream_dir.join(MANIFEST_FILE_NAME);
        if !allow_generated_downgrade
            && read_manifest(&manifest_path)?.is_some_and(|manifest| {
                manifest.attestation.mode() == UpstreamSvgAttestationMode::Generated
            })
        {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "refusing to replace generated upstream SVG provenance for {diagram} with adopted-existing; rerun with --allow-downgrade only when the loss of render attestation is intentional"
            )));
        }
        let manifest = build_complete_manifest_with_source(
            &diagram,
            &fixtures_dir,
            &upstream_dir,
            source.clone(),
            UpstreamSvgAttestation::adopted_existing(),
        )?;
        validated.push((upstream_dir, manifest));
    }

    let writes: Vec<_> = validated
        .iter()
        .map(|(upstream_dir, manifest)| (upstream_dir.as_path(), manifest))
        .collect();
    write_manifest_batch(&writes)
}

fn validate_existing_manifests_with_source<S: AsRef<str>>(
    diagrams: &[S],
    fixtures_root: &Path,
    upstream_root: &Path,
    source: UpstreamSvgSource,
) -> Result<(), XtaskError> {
    let families: Vec<_> = diagrams
        .iter()
        .map(|diagram| {
            let diagram = diagram.as_ref().to_string();
            let upstream_dir = upstream_root.join(&diagram);
            (diagram, upstream_dir)
        })
        .collect();
    let lock_dirs: Vec<_> = families
        .iter()
        .map(|(_, upstream_dir)| upstream_dir.clone())
        .collect();
    let _family_locks = acquire_upstream_svg_family_locks(&lock_dirs)?;

    for (diagram, upstream_dir) in families {
        load_upstream_svg_provenance_with_source(
            &diagram,
            &fixtures_root.join(&diagram),
            &upstream_dir,
            true,
            source.clone(),
        )?;
    }
    Ok(())
}

pub(crate) fn adopt_upstream_svg_provenance(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: Option<String> = None;
    let mut check_only = false;
    let mut allow_downgrade = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--diagram" => {
                index += 1;
                let value = args.get(index).ok_or(XtaskError::Usage)?.trim();
                if value.is_empty() || diagram.replace(value.to_string()).is_some() {
                    return Err(XtaskError::Usage);
                }
            }
            "--check-only" => check_only = true,
            "--allow-downgrade" => allow_downgrade = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }

    let diagram = diagram.ok_or(XtaskError::Usage)?;
    let primary: Vec<_> = crate::cmd::primary_svg_matrix_diagrams().collect();
    let selected: Vec<&str> = if diagram == "all" {
        primary.clone()
    } else if primary.contains(&diagram.as_str()) {
        vec![diagram.as_str()]
    } else {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "unsupported upstream SVG provenance family {diagram:?}; expected a primary SVG family or all"
        )));
    };

    let fixtures_root = crate::cmd::fixtures_root();
    let upstream_root = fixtures_root.join("upstream-svgs");
    if check_only {
        if allow_downgrade {
            return Err(XtaskError::Usage);
        }
        validate_existing_manifests_with_source(
            &selected,
            &fixtures_root,
            &upstream_root,
            current_source()?,
        )?;
        println!(
            "validated {} existing upstream SVG manifest/corpus family/families without writing manifests",
            selected.len()
        );
    } else {
        adopt_existing_manifests_with_source(
            &selected,
            &fixtures_root,
            &upstream_root,
            current_source()?,
            allow_downgrade,
        )?;
        println!(
            "adopted {} complete upstream SVG provenance family/families",
            selected.len()
        );
    }
    Ok(())
}

fn current_source() -> Result<UpstreamSvgSource, XtaskError> {
    let tools_root = crate::cmd::mermaid_cli_root();
    let package_json_path = tools_root.join("package.json");
    let package_lock_path = tools_root.join("package-lock.json");
    let config_path = tools_root.join("mermaid-config.json");
    let package_json = read_json(&package_json_path)?;
    let mermaid_version =
        required_json_string(&package_json, &package_json_path, &["overrides", "mermaid"])?;
    let mermaid_cli_version = required_json_string(
        &package_json,
        &package_json_path,
        &["devDependencies", "@mermaid-js/mermaid-cli"],
    )?;
    if mermaid_version != PINNED_MERMAID_VERSION
        || mermaid_cli_version != PINNED_MERMAID_CLI_VERSION
    {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "pinned Mermaid package metadata must use Mermaid {PINNED_MERMAID_VERSION} and Mermaid CLI {PINNED_MERMAID_CLI_VERSION}, found Mermaid {mermaid_version} and CLI {mermaid_cli_version}"
        )));
    }

    let package_json_sha256 = hash_file(&package_json_path)?;
    let package_lock_sha256 = hash_file(&package_lock_path)?;
    let mermaid_config_sha256 = hash_file(&config_path)?;
    for (kind, actual, expected) in [
        (
            "package.json",
            package_json_sha256.as_str(),
            PACKAGE_JSON_SHA256,
        ),
        (
            "package-lock.json",
            package_lock_sha256.as_str(),
            PACKAGE_LOCK_SHA256,
        ),
        (
            "mermaid-config.json",
            mermaid_config_sha256.as_str(),
            MERMAID_CONFIG_SHA256,
        ),
    ] {
        if actual != expected {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "pinned Mermaid {kind} SHA-256 drifted: actual={actual}, expected={expected}"
            )));
        }
    }

    Ok(UpstreamSvgSource {
        mermaid_version,
        mermaid_cli_version,
        mermaid_source_tag: MERMAID_SOURCE_TAG.to_string(),
        mermaid_source_commit: MERMAID_SOURCE_COMMIT.to_string(),
        package_json_sha256,
        package_lock_sha256,
        mermaid_config_sha256,
        renderer_revision: RENDERER_REVISION.to_string(),
    })
}

fn renderer_profile(diagram: &str) -> &'static str {
    if matches!(diagram, "architecture" | "gitgraph") {
        "seeded-puppeteer-seed-1"
    } else if diagram == "gantt" {
        "mmdc-default-width-1200"
    } else {
        "mmdc-default"
    }
}

fn fixture_stem(path: &Path) -> Result<&str, XtaskError> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            XtaskError::UpstreamSvgFailed(format!(
                "invalid upstream SVG fixture filename {}",
                path.display()
            ))
        })
}

fn read_manifest(path: &Path) -> Result<Option<UpstreamSvgManifest>, XtaskError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(XtaskError::ReadFile {
                path: path.display().to_string(),
                source,
            });
        }
    };
    serde_json::from_str(&text).map(Some).map_err(|err| {
        XtaskError::UpstreamSvgFailed(format!(
            "failed to parse upstream SVG provenance {}: {err}",
            path.display()
        ))
    })
}

fn read_json(path: &Path) -> Result<serde_json::Value, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|err| {
        XtaskError::UpstreamSvgFailed(format!(
            "failed to parse pinned Mermaid metadata {}: {err}",
            path.display()
        ))
    })
}

fn required_json_string(
    value: &serde_json::Value,
    path: &Path,
    fields: &[&str],
) -> Result<String, XtaskError> {
    fields
        .iter()
        .try_fold(value, |current, field| current.get(field))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            XtaskError::UpstreamSvgFailed(format!(
                "pinned Mermaid metadata {} is missing {}",
                path.display(),
                fields.join(".")
            ))
        })
}

fn hash_file(path: &Path) -> Result<String, XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    Ok(hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_hash(
    path: &Path,
    expected: &str,
    kind: &str,
    diagram: &str,
    stem: &str,
) -> Result<(), String> {
    let actual = hash_file(path).map_err(|err| err.to_string())?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "upstream SVG provenance {kind} hash mismatch for {diagram}/{stem}: {}; regenerate the pinned baseline",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests;
