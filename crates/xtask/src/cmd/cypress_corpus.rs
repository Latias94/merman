use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::cmd::{MERMAID_SOURCE_COMMIT, MERMAID_SOURCE_TAG, PINNED_MERMAID_VERSION};
use crate::util::{is_canonical_sha256, sha256_hex};

pub(crate) const MANIFEST_RELATIVE_PATH: &str = "fixtures/_upstream/cypress-11.16.0/_manifest.json";
const SCHEMA_VERSION: u32 = 1;
const SCOPE_DESCRIPTION: &str = "Mermaid 11.16 new-family Cypress render calls";
const PINNED_SOURCE_SPECS: &[(&str, usize)] = &[
    (
        "cypress/integration/rendering/treeView/treeView.spec.ts",
        15,
    ),
    ("cypress/integration/rendering/cynefin/cynefin.spec.js", 12),
    ("cypress/integration/rendering/railroad/railroad.spec.ts", 9),
];
const MANAGED_FIXTURE_PREFIXES: &[&str] = &[
    "upstream_cypress_treeview_spec_",
    "upstream_cypress_cynefin_spec_",
    "upstream_cypress_railroad_spec_",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct SafeRelativePath(String);

impl SafeRelativePath {
    fn parse(value: String) -> Result<Self, String> {
        if value.is_empty() {
            return Err("relative path must not be empty".to_string());
        }
        if value.contains('\\') {
            return Err(format!("relative path `{value}` must use forward slashes"));
        }
        if value.contains(':') {
            return Err(format!(
                "relative path `{value}` must not contain a platform prefix or alternate stream"
            ));
        }
        if value
            .chars()
            .any(|character| character.is_control() || character == '\0')
        {
            return Err(format!(
                "relative path `{value}` must not contain control characters"
            ));
        }
        if value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(format!(
                "relative path `{value}` must contain only non-empty normal components"
            ));
        }

        let path = Path::new(&value);
        if path.is_absolute()
            || !path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(format!(
                "relative path `{value}` must not be absolute or escape its root"
            ));
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl<'de> Deserialize<'de> for SafeRelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for SafeRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct SafePathComponent(String);

impl SafePathComponent {
    fn parse(value: String) -> Result<Self, String> {
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(format!(
                "path component `{value}` must contain only ASCII letters, digits, `_`, or `-`"
            ));
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SafePathComponent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for SafePathComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCorpusManifest {
    pub(crate) schema_version: u32,
    pub(crate) mermaid_version: String,
    pub(crate) mermaid_source_commit: String,
    pub(crate) scope: CypressCorpusScope,
    pub(crate) entries: Vec<CypressCorpusEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCorpusScope {
    pub(crate) description: String,
    pub(crate) source_specs: Vec<CypressCorpusSourceSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCorpusSourceSpec {
    pub(crate) path: SafeRelativePath,
    pub(crate) expected_calls: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCorpusEntry {
    pub(crate) source_spec: SafeRelativePath,
    pub(crate) call_ordinal: usize,
    pub(crate) call: String,
    pub(crate) test_name: String,
    pub(crate) family: SafePathComponent,
    pub(crate) route: CypressCorpusRoute,
    pub(crate) fixture: SafeRelativePath,
    pub(crate) mmd_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CypressCorpusRoute {
    Active,
    Deferred,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CypressSourceObservation {
    pub(crate) source_spec: String,
    pub(crate) call_ordinal: usize,
    pub(crate) call: String,
    pub(crate) test_name: String,
    pub(crate) family: String,
    pub(crate) mmd_sha256: String,
}

struct CorpusArtifactPaths {
    active_fixture: PathBuf,
    deferred_fixture: PathBuf,
    active_semantic: PathBuf,
    active_layout: PathBuf,
    deferred_semantic: PathBuf,
    deferred_layout: PathBuf,
    active_svg: PathBuf,
    deferred_svg: PathBuf,
}

pub(crate) fn cypress_corpus_mmd_sha256(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

fn artifact_paths(
    workspace_root: &Path,
    entry: &CypressCorpusEntry,
) -> Option<CorpusArtifactPaths> {
    let stem = entry.fixture.as_path().file_stem()?.to_str()?;
    let active_fixture = workspace_root
        .join("fixtures")
        .join(entry.family.as_str())
        .join(format!("{stem}.mmd"));
    let deferred_fixture = workspace_root
        .join("fixtures")
        .join("_deferred")
        .join(entry.family.as_str())
        .join(format!("{stem}.mmd"));
    Some(CorpusArtifactPaths {
        active_semantic: active_fixture.with_extension("golden.json"),
        active_layout: active_fixture.with_extension("layout.golden.json"),
        deferred_semantic: deferred_fixture.with_extension("golden.json"),
        deferred_layout: deferred_fixture.with_extension("layout.golden.json"),
        active_svg: workspace_root
            .join("fixtures/upstream-svgs")
            .join(entry.family.as_str())
            .join(format!("{stem}.svg")),
        deferred_svg: workspace_root
            .join("fixtures/_deferred/upstream-svgs")
            .join(entry.family.as_str())
            .join(format!("{stem}.svg")),
        active_fixture,
        deferred_fixture,
    })
}

fn expected_fixture_relative_path(entry: &CypressCorpusEntry) -> Option<String> {
    let stem = entry.fixture.as_path().file_stem()?.to_str()?;
    Some(match entry.route {
        CypressCorpusRoute::Active => format!("fixtures/{}/{stem}.mmd", entry.family),
        CypressCorpusRoute::Deferred => {
            format!("fixtures/_deferred/{}/{stem}.mmd", entry.family)
        }
    })
}

fn require_file_under_workspace(
    failures: &mut Vec<String>,
    workspace_root: &Path,
    canonical_workspace_root: &Path,
    physical_files: &mut BTreeMap<PathBuf, PathBuf>,
    path: &Path,
    description: &str,
) -> Option<PathBuf> {
    if !path.is_file() {
        failures.push(format!(
            "Cypress corpus is missing {description} {}",
            path.display()
        ));
        return None;
    }

    let canonical = match fs::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(error) => {
            failures.push(format!(
                "failed to canonicalize Cypress corpus {description} {}: {error}",
                path.display()
            ));
            return None;
        }
    };
    if !canonical.starts_with(canonical_workspace_root) {
        failures.push(format!(
            "Cypress corpus {description} {} escapes workspace {} via {}",
            path.display(),
            workspace_root.display(),
            canonical.display()
        ));
        return None;
    }
    let Some(expected_root) = path.parent() else {
        failures.push(format!(
            "Cypress corpus {description} {} has no expected parent directory",
            path.display()
        ));
        return None;
    };
    let canonical_expected_root = match fs::canonicalize(expected_root) {
        Ok(root) => root,
        Err(error) => {
            failures.push(format!(
                "failed to canonicalize Cypress corpus {description} root {}: {error}",
                expected_root.display()
            ));
            return None;
        }
    };
    if !canonical_expected_root.starts_with(canonical_workspace_root) {
        failures.push(format!(
            "Cypress corpus {description} root {} escapes workspace {} via {}",
            expected_root.display(),
            workspace_root.display(),
            canonical_expected_root.display()
        ));
        return None;
    }
    if !canonical.starts_with(&canonical_expected_root) {
        failures.push(format!(
            "Cypress corpus {description} {} escapes expected root {} via {}",
            path.display(),
            expected_root.display(),
            canonical.display()
        ));
        return None;
    }
    if let Some(previous) = physical_files.insert(canonical.clone(), path.to_path_buf())
        && previous != path
    {
        failures.push(format!(
            "Cypress corpus paths {} and {} resolve to the same physical file {}",
            previous.display(),
            path.display(),
            canonical.display()
        ));
    }
    Some(canonical)
}

fn reject_file(failures: &mut Vec<String>, path: &Path, description: &str) {
    if path.exists() {
        failures.push(format!(
            "Cypress corpus has {description} on the opposite route: {}",
            path.display()
        ));
    }
}

fn managed_artifact_name(name: &str) -> bool {
    let stem = [".layout.golden.json", ".golden.json", ".mmd", ".svg"]
        .into_iter()
        .find_map(|suffix| name.strip_suffix(suffix));
    stem.is_some_and(|stem| {
        MANAGED_FIXTURE_PREFIXES
            .iter()
            .any(|prefix| stem.starts_with(prefix))
    })
}

fn collect_managed_artifacts(
    workspace_root: &Path,
    directory: &Path,
    artifacts: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory).map_err(|error| {
        format!(
            "read managed Cypress corpus directory {}: {error}",
            directory.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "read managed Cypress corpus entry under {}: {error}",
                directory.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "read managed Cypress corpus file type {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_managed_artifacts(workspace_root, &path, artifacts)?;
        } else if (file_type.is_file() || file_type.is_symlink())
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(managed_artifact_name)
        {
            let relative = path.strip_prefix(workspace_root).map_err(|_| {
                format!(
                    "managed Cypress corpus artifact {} is outside workspace {}",
                    path.display(),
                    workspace_root.display()
                )
            })?;
            artifacts.insert(relative.to_path_buf());
        }
    }
    Ok(())
}

fn expected_managed_path(workspace_root: &Path, path: &Path) -> Option<PathBuf> {
    path.strip_prefix(workspace_root)
        .ok()
        .map(Path::to_path_buf)
}

pub(crate) fn resolve_cypress_source_spec_path(
    mermaid_root: &Path,
    source_spec: &SafeRelativePath,
) -> Result<PathBuf, String> {
    let canonical_root = fs::canonicalize(mermaid_root).map_err(|error| {
        format!(
            "canonicalize pinned Mermaid checkout {}: {error}",
            mermaid_root.display()
        )
    })?;
    let candidate = mermaid_root.join(source_spec.as_path());
    if !candidate.is_file() {
        return Err(format!(
            "pinned Cypress source spec is not a file: {}",
            candidate.display()
        ));
    }
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        format!(
            "canonicalize pinned Cypress source spec {}: {error}",
            candidate.display()
        )
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(format!(
            "pinned Cypress source spec {} escapes Mermaid checkout {} via {}",
            candidate.display(),
            mermaid_root.display(),
            canonical.display()
        ));
    }
    Ok(candidate)
}

pub(crate) fn load_committed_cypress_corpus_manifest(
    workspace_root: &Path,
) -> Result<CypressCorpusManifest, String> {
    let path = workspace_root.join(MANIFEST_RELATIVE_PATH);
    let bytes = fs::read(&path).map_err(|error| {
        format!(
            "failed to read Cypress corpus manifest {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "failed to parse Cypress corpus manifest {}: {error}",
            path.display()
        )
    })
}

pub(crate) fn committed_cypress_corpus_alignment_failures(workspace_root: &Path) -> Vec<String> {
    let manifest = match load_committed_cypress_corpus_manifest(workspace_root) {
        Ok(manifest) => manifest,
        Err(error) => return vec![error],
    };
    let mut failures = validate_pinned_manifest_contract(workspace_root, &manifest);
    failures.extend(validate_cypress_corpus_manifest(workspace_root, &manifest));
    failures
}

fn validate_pinned_manifest_contract(
    workspace_root: &Path,
    manifest: &CypressCorpusManifest,
) -> Vec<String> {
    let mut failures = Vec::new();
    if manifest.schema_version != SCHEMA_VERSION {
        failures.push(format!(
            "Cypress corpus manifest schema_version must be {SCHEMA_VERSION}, found {}",
            manifest.schema_version
        ));
    }
    if manifest.mermaid_version != PINNED_MERMAID_VERSION {
        failures.push(format!(
            "Cypress corpus manifest Mermaid version must be {PINNED_MERMAID_VERSION}, found {}",
            manifest.mermaid_version
        ));
    }
    if manifest.mermaid_source_commit != MERMAID_SOURCE_COMMIT {
        failures.push(format!(
            "Cypress corpus manifest Mermaid commit must be {MERMAID_SOURCE_COMMIT}, found {}",
            manifest.mermaid_source_commit
        ));
    }
    if manifest.scope.description != SCOPE_DESCRIPTION {
        failures.push(format!(
            "Cypress corpus manifest scope must be `{SCOPE_DESCRIPTION}`"
        ));
    }
    let actual_specs: Vec<(&str, usize)> = manifest
        .scope
        .source_specs
        .iter()
        .map(|spec| (spec.path.as_str(), spec.expected_calls))
        .collect();
    if actual_specs != PINNED_SOURCE_SPECS {
        failures.push(format!(
            "Cypress corpus manifest must cover exactly the three pinned 11.16 source specs and 36 calls; found {actual_specs:?}"
        ));
    }

    let lock_path = workspace_root.join("tools/upstreams/REPOS.lock.json");
    match fs::read(&lock_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
    {
        Some(lock) => {
            let pinned = lock.get("repos").and_then(|repos| repos.get("mermaid"));
            let lock_ref = pinned
                .and_then(|repo| repo.get("ref"))
                .and_then(serde_json::Value::as_str);
            let lock_commit = pinned
                .and_then(|repo| repo.get("commit"))
                .and_then(serde_json::Value::as_str);
            if lock_ref != Some(MERMAID_SOURCE_TAG) || lock_commit != Some(MERMAID_SOURCE_COMMIT) {
                failures.push(format!(
                    "Cypress corpus manifest pin disagrees with {}",
                    lock_path.display()
                ));
            }
        }
        None => failures.push(format!(
            "failed to read pinned Mermaid metadata from {}",
            lock_path.display()
        )),
    }
    failures
}

pub(crate) fn validate_cypress_corpus_manifest(
    workspace_root: &Path,
    manifest: &CypressCorpusManifest,
) -> Vec<String> {
    let mut failures = Vec::new();
    let canonical_workspace_root = match fs::canonicalize(workspace_root) {
        Ok(root) => root,
        Err(error) => {
            return vec![format!(
                "failed to canonicalize Cypress corpus workspace {}: {error}",
                workspace_root.display()
            )];
        }
    };
    let mut identities = BTreeSet::new();
    let mut fixture_paths = BTreeSet::new();
    let mut physical_files = BTreeMap::new();
    let mut expected_managed_artifacts = BTreeSet::new();
    for entry in &manifest.entries {
        let identity = (entry.source_spec.as_str(), entry.call_ordinal);
        if !identities.insert(identity) {
            failures.push(format!(
                "Cypress corpus manifest has duplicate call identity {}#{}",
                entry.source_spec, entry.call_ordinal
            ));
        }
        if !fixture_paths.insert(entry.fixture.as_str()) {
            failures.push(format!(
                "Cypress corpus manifest has duplicate fixture path {}",
                entry.fixture
            ));
        }

        let Some(expected_fixture) = expected_fixture_relative_path(entry) else {
            failures.push(format!(
                "Cypress corpus manifest entry has an invalid fixture path {}",
                entry.fixture
            ));
            continue;
        };
        if entry.fixture.as_str() != expected_fixture {
            failures.push(format!(
                "Cypress corpus manifest route/path mismatch: expected {expected_fixture}, found {}",
                entry.fixture
            ));
            continue;
        }
        if !is_canonical_sha256(&entry.mmd_sha256) {
            failures.push(format!(
                "Cypress corpus manifest entry {}#{} has a non-canonical MMD SHA-256",
                entry.source_spec, entry.call_ordinal
            ));
        }

        let Some(paths) = artifact_paths(workspace_root, entry) else {
            failures.push(format!(
                "Cypress corpus manifest entry has an invalid fixture path {}",
                entry.fixture
            ));
            continue;
        };

        let (declared_fixture, fixture_description) = match entry.route {
            CypressCorpusRoute::Active => (&paths.active_fixture, "active fixture"),
            CypressCorpusRoute::Deferred => (&paths.deferred_fixture, "deferred fixture"),
        };
        if let Some(canonical_fixture) = require_file_under_workspace(
            &mut failures,
            workspace_root,
            &canonical_workspace_root,
            &mut physical_files,
            declared_fixture,
            fixture_description,
        ) {
            match fs::read(&canonical_fixture) {
                Ok(bytes) => {
                    let actual = cypress_corpus_mmd_sha256(&bytes);
                    if actual != entry.mmd_sha256 {
                        failures.push(format!(
                            "Cypress corpus MMD SHA-256 drift for {}: expected {}, found {actual}",
                            entry.fixture, entry.mmd_sha256
                        ));
                    }
                }
                Err(error) => failures.push(format!(
                    "failed to read Cypress corpus fixture {}: {error}",
                    canonical_fixture.display()
                )),
            }
        }

        match entry.route {
            CypressCorpusRoute::Active => {
                for path in [
                    &paths.active_fixture,
                    &paths.active_semantic,
                    &paths.active_layout,
                    &paths.active_svg,
                ] {
                    if let Some(relative) = expected_managed_path(workspace_root, path) {
                        expected_managed_artifacts.insert(relative);
                    }
                }
                require_file_under_workspace(
                    &mut failures,
                    workspace_root,
                    &canonical_workspace_root,
                    &mut physical_files,
                    &paths.active_semantic,
                    "active semantic golden",
                );
                require_file_under_workspace(
                    &mut failures,
                    workspace_root,
                    &canonical_workspace_root,
                    &mut physical_files,
                    &paths.active_layout,
                    "active layout golden",
                );
                require_file_under_workspace(
                    &mut failures,
                    workspace_root,
                    &canonical_workspace_root,
                    &mut physical_files,
                    &paths.active_svg,
                    "active upstream SVG baseline",
                );
                reject_file(&mut failures, &paths.deferred_fixture, "fixture");
                reject_file(&mut failures, &paths.deferred_semantic, "semantic golden");
                reject_file(&mut failures, &paths.deferred_layout, "layout golden");
                if paths.deferred_svg.exists() {
                    failures.push(format!(
                        "Cypress corpus baseline side mismatch for {}: deferred baseline exists for an active fixture",
                        entry.fixture
                    ));
                }
            }
            CypressCorpusRoute::Deferred => {
                for path in [&paths.deferred_fixture, &paths.deferred_svg] {
                    if let Some(relative) = expected_managed_path(workspace_root, path) {
                        expected_managed_artifacts.insert(relative);
                    }
                }
                require_file_under_workspace(
                    &mut failures,
                    workspace_root,
                    &canonical_workspace_root,
                    &mut physical_files,
                    &paths.deferred_svg,
                    "deferred upstream SVG baseline",
                );
                reject_file(&mut failures, &paths.active_fixture, "fixture");
                reject_file(&mut failures, &paths.active_semantic, "semantic golden");
                reject_file(&mut failures, &paths.active_layout, "layout golden");
                reject_file(
                    &mut failures,
                    &paths.deferred_semantic,
                    "deferred semantic golden",
                );
                reject_file(
                    &mut failures,
                    &paths.deferred_layout,
                    "deferred layout golden",
                );
                if paths.active_svg.exists() {
                    failures.push(format!(
                        "Cypress corpus baseline side mismatch for {}: active baseline exists for a deferred fixture",
                        entry.fixture
                    ));
                }
            }
        }
    }

    let mut actual_managed_artifacts = BTreeSet::new();
    let fixtures_root = workspace_root.join("fixtures");
    match collect_managed_artifacts(
        workspace_root,
        &fixtures_root,
        &mut actual_managed_artifacts,
    ) {
        Ok(()) => {
            for orphan in actual_managed_artifacts.difference(&expected_managed_artifacts) {
                failures.push(format!(
                    "Cypress corpus has orphan managed artifact {}",
                    workspace_root.join(orphan).display()
                ));
            }
        }
        Err(error) => failures.push(error),
    }

    let expected_order: Vec<(&str, usize)> = manifest
        .scope
        .source_specs
        .iter()
        .flat_map(|spec| {
            (1..=spec.expected_calls).map(move |ordinal| (spec.path.as_str(), ordinal))
        })
        .collect();
    let actual_order: Vec<(&str, usize)> = manifest
        .entries
        .iter()
        .map(|entry| (entry.source_spec.as_str(), entry.call_ordinal))
        .collect();
    if actual_order != expected_order {
        failures.push(
            "Cypress corpus manifest order must follow scope source_specs and contiguous call ordinals"
                .to_string(),
        );
    }
    failures
}

pub(crate) fn validate_cypress_source_observations(
    manifest: &CypressCorpusManifest,
    observations: &[CypressSourceObservation],
) -> Vec<String> {
    let mut failures = Vec::new();
    let expected_identities: Vec<(&str, usize)> = manifest
        .entries
        .iter()
        .map(|entry| (entry.source_spec.as_str(), entry.call_ordinal))
        .collect();
    let actual_identities: Vec<(&str, usize)> = observations
        .iter()
        .map(|entry| (entry.source_spec.as_str(), entry.call_ordinal))
        .collect();
    if expected_identities != actual_identities {
        failures.push("Cypress source call order drifted from the committed manifest".to_string());
    }

    let mut observed_identities = BTreeSet::new();
    for observation in observations {
        let identity = (observation.source_spec.as_str(), observation.call_ordinal);
        if !observed_identities.insert(identity) {
            failures.push(format!(
                "Cypress source scanner produced duplicate call identity {}#{}",
                observation.source_spec, observation.call_ordinal
            ));
        }
    }

    for expected in &manifest.entries {
        let observation = observations.iter().find(|observation| {
            observation.source_spec == expected.source_spec.as_str()
                && observation.call_ordinal == expected.call_ordinal
        });
        let Some(observation) = observation else {
            failures.push(format!(
                "Cypress manifest entry is missing source call {}#{}",
                expected.source_spec, expected.call_ordinal
            ));
            continue;
        };
        for (field, expected_value, actual_value) in [
            ("call", expected.call.as_str(), observation.call.as_str()),
            (
                "test_name",
                expected.test_name.as_str(),
                observation.test_name.as_str(),
            ),
            (
                "family",
                expected.family.as_str(),
                observation.family.as_str(),
            ),
        ] {
            if actual_value != expected_value {
                failures.push(format!(
                    "Cypress source {field} drift for {}#{}: expected {expected_value:?}, found {actual_value:?}",
                    expected.source_spec, expected.call_ordinal
                ));
            }
        }
        if observation.mmd_sha256 != expected.mmd_sha256 {
            failures.push(format!(
                "Cypress source content drift for {}#{}: expected {}, found {}",
                expected.source_spec,
                expected.call_ordinal,
                expected.mmd_sha256,
                observation.mmd_sha256
            ));
        }
    }
    for observation in observations {
        if !manifest.entries.iter().any(|expected| {
            expected.source_spec.as_str() == observation.source_spec
                && expected.call_ordinal == observation.call_ordinal
        }) {
            failures.push(format!(
                "Cypress source has an unexpected call {}#{}",
                observation.source_spec, observation.call_ordinal
            ));
        }
    }
    failures
}

pub(crate) fn refreshed_cypress_corpus_manifest(
    manifest: &CypressCorpusManifest,
    observations: &[CypressSourceObservation],
) -> Result<CypressCorpusManifest, Vec<String>> {
    let mut refreshed = manifest.clone();
    for entry in &mut refreshed.entries {
        if let Some(observation) = observations.iter().find(|observation| {
            observation.source_spec == entry.source_spec.as_str()
                && observation.call_ordinal == entry.call_ordinal
        }) {
            entry.mmd_sha256.clone_from(&observation.mmd_sha256);
        }
    }

    let failures = validate_cypress_source_observations(&refreshed, observations);
    if failures.is_empty() {
        Ok(refreshed)
    } else {
        Err(failures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TempWorkspace(PathBuf);

    impl TempWorkspace {
        fn new() -> Self {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "merman-cypress-corpus-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("create temp workspace");
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture_bytes(ordinal: usize) -> Vec<u8> {
        format!("treeView-beta\n  root {ordinal}\n").into_bytes()
    }

    fn safe_relative(value: impl Into<String>) -> SafeRelativePath {
        SafeRelativePath::parse(value.into()).expect("safe relative test path")
    }

    fn safe_component(value: impl Into<String>) -> SafePathComponent {
        SafePathComponent::parse(value.into()).expect("safe test component")
    }

    fn entry(ordinal: usize) -> CypressCorpusEntry {
        let fixture =
            format!("fixtures/treeView/upstream_cypress_treeview_spec_case_{ordinal:03}.mmd");
        CypressCorpusEntry {
            source_spec: safe_relative("cypress/integration/rendering/treeView/treeView.spec.ts"),
            call_ordinal: ordinal,
            call: "imgSnapshotTest".to_string(),
            test_name: format!("case {ordinal}"),
            family: safe_component("treeView"),
            route: CypressCorpusRoute::Active,
            fixture: safe_relative(fixture),
            mmd_sha256: sha256_hex(&fixture_bytes(ordinal)),
        }
    }

    fn manifest(entries: Vec<CypressCorpusEntry>) -> CypressCorpusManifest {
        CypressCorpusManifest {
            schema_version: 1,
            mermaid_version: "11.16.0".to_string(),
            mermaid_source_commit: "7c0cafcf42e76bfaf79d0cbbd12edb986612f014".to_string(),
            scope: CypressCorpusScope {
                description: "test corpus".to_string(),
                source_specs: vec![CypressCorpusSourceSpec {
                    path: safe_relative("cypress/integration/rendering/treeView/treeView.spec.ts"),
                    expected_calls: entries.len(),
                }],
            },
            entries,
        }
    }

    fn materialize_active_entry(root: &Path, entry: &CypressCorpusEntry) {
        let fixture = root.join(entry.fixture.as_path());
        fs::create_dir_all(fixture.parent().expect("fixture parent")).expect("fixture dir");
        fs::write(&fixture, fixture_bytes(entry.call_ordinal)).expect("fixture");
        fs::write(fixture.with_extension("golden.json"), b"{}\n").expect("semantic golden");
        fs::write(fixture.with_extension("layout.golden.json"), b"{}\n").expect("layout golden");

        let svg = root
            .join("fixtures/upstream-svgs")
            .join(entry.family.as_str())
            .join(format!(
                "{}.svg",
                fixture.file_stem().unwrap().to_string_lossy()
            ));
        fs::create_dir_all(svg.parent().expect("svg parent")).expect("svg dir");
        fs::write(svg, b"<svg/>\n").expect("svg");
    }

    fn observation(entry: &CypressCorpusEntry) -> CypressSourceObservation {
        CypressSourceObservation {
            source_spec: entry.source_spec.to_string(),
            call_ordinal: entry.call_ordinal,
            call: entry.call.clone(),
            test_name: entry.test_name.clone(),
            family: entry.family.to_string(),
            mmd_sha256: entry.mmd_sha256.clone(),
        }
    }

    #[test]
    fn committed_manifest_accepts_one_fixture_and_baseline_on_the_declared_side() {
        let root = TempWorkspace::new();
        let entry = entry(1);
        materialize_active_entry(root.path(), &entry);

        assert!(validate_cypress_corpus_manifest(root.path(), &manifest(vec![entry])).is_empty());
    }

    #[test]
    fn committed_manifest_rejects_duplicate_call_identity() {
        let root = TempWorkspace::new();
        let first = entry(1);
        let mut duplicate = first.clone();
        duplicate.fixture = safe_relative("fixtures/treeView/duplicate.mmd");
        materialize_active_entry(root.path(), &first);
        materialize_active_entry(root.path(), &duplicate);

        let failures =
            validate_cypress_corpus_manifest(root.path(), &manifest(vec![first, duplicate]));

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("duplicate call identity")),
            "{failures:#?}"
        );
    }

    #[test]
    fn committed_manifest_rejects_reordered_calls() {
        let root = TempWorkspace::new();
        let first = entry(1);
        let second = entry(2);
        materialize_active_entry(root.path(), &first);
        materialize_active_entry(root.path(), &second);

        let failures =
            validate_cypress_corpus_manifest(root.path(), &manifest(vec![second, first]));

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("manifest order")),
            "{failures:#?}"
        );
    }

    #[test]
    fn committed_manifest_rejects_missing_fixture() {
        let root = TempWorkspace::new();
        let entry = entry(1);

        let failures = validate_cypress_corpus_manifest(root.path(), &manifest(vec![entry]));

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("missing active fixture")),
            "{failures:#?}"
        );
    }

    #[test]
    fn committed_manifest_rejects_fixture_content_drift() {
        let root = TempWorkspace::new();
        let entry = entry(1);
        materialize_active_entry(root.path(), &entry);
        fs::write(
            root.path().join(entry.fixture.as_path()),
            b"treeView-beta\n  changed\n",
        )
        .expect("drift fixture");

        let failures = validate_cypress_corpus_manifest(root.path(), &manifest(vec![entry]));

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("MMD SHA-256 drift")),
            "{failures:#?}"
        );
    }

    #[test]
    fn committed_manifest_rejects_baseline_on_the_opposite_side() {
        let root = TempWorkspace::new();
        let entry = entry(1);
        materialize_active_entry(root.path(), &entry);
        let active_svg = root
            .path()
            .join("fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_case_001.svg");
        let deferred_svg = root.path().join(
            "fixtures/_deferred/upstream-svgs/treeView/upstream_cypress_treeview_spec_case_001.svg",
        );
        fs::create_dir_all(deferred_svg.parent().unwrap()).expect("deferred SVG dir");
        fs::rename(active_svg, deferred_svg).expect("move SVG to wrong side");

        let failures = validate_cypress_corpus_manifest(root.path(), &manifest(vec![entry]));

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("baseline side mismatch")),
            "{failures:#?}"
        );
    }

    #[test]
    fn committed_manifest_rejects_missing_active_goldens() {
        let root = TempWorkspace::new();
        let entry = entry(1);
        materialize_active_entry(root.path(), &entry);
        let fixture = root.path().join(entry.fixture.as_path());
        fs::remove_file(fixture.with_extension("golden.json")).expect("remove semantic golden");
        fs::remove_file(fixture.with_extension("layout.golden.json"))
            .expect("remove layout golden");

        let failures = validate_cypress_corpus_manifest(root.path(), &manifest(vec![entry]));

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("active semantic golden")),
            "{failures:#?}"
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("active layout golden")),
            "{failures:#?}"
        );
    }

    #[test]
    fn source_check_rejects_missing_and_drifted_calls() {
        let first = entry(1);
        let second = entry(2);
        let manifest = manifest(vec![first.clone(), second]);
        let mut drifted = observation(&first);
        drifted.mmd_sha256 = "f".repeat(64);

        let failures = validate_cypress_source_observations(&manifest, &[drifted]);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("source content drift")),
            "{failures:#?}"
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("missing source call")),
            "{failures:#?}"
        );
    }

    #[test]
    fn source_refresh_updates_only_content_hashes_after_contract_validation() {
        let first = entry(1);
        let second = entry(2);
        let manifest = manifest(vec![first.clone(), second.clone()]);
        let mut first_observation = observation(&first);
        first_observation.mmd_sha256 = "a".repeat(64);
        let mut second_observation = observation(&second);
        second_observation.mmd_sha256 = "b".repeat(64);

        let refreshed =
            refreshed_cypress_corpus_manifest(&manifest, &[first_observation, second_observation])
                .expect("matching source metadata should permit a content refresh");

        assert_eq!(refreshed.entries[0].mmd_sha256, "a".repeat(64));
        assert_eq!(refreshed.entries[1].mmd_sha256, "b".repeat(64));
        assert_eq!(refreshed.entries[0].fixture, first.fixture);
        assert_eq!(refreshed.entries[1].fixture, second.fixture);
    }

    #[test]
    fn source_refresh_rejects_metadata_drift() {
        let expected = entry(1);
        let mut drifted = observation(&expected);
        drifted.call = "renderSnapshot".to_string();

        let failures = refreshed_cypress_corpus_manifest(&manifest(vec![expected]), &[drifted])
            .expect_err("metadata drift must not be absorbed into a content refresh");

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("source call drift")),
            "{failures:#?}"
        );
    }

    #[test]
    fn manifest_schema_rejects_unknown_fields_at_every_object_layer() {
        for pointer in ["", "/scope", "/scope/source_specs/0", "/entries/0"] {
            let mut value = serde_json::to_value(manifest(vec![entry(1)])).unwrap();
            value
                .pointer_mut(pointer)
                .and_then(serde_json::Value::as_object_mut)
                .expect("object pointer")
                .insert("unexpected_field".to_string(), serde_json::json!(true));

            let parsed = serde_json::from_value::<CypressCorpusManifest>(value);
            assert!(
                parsed.is_err(),
                "unknown field at `{pointer}` must be rejected"
            );
        }
    }

    #[test]
    fn manifest_schema_rejects_non_portable_or_escaping_paths() {
        let cases = [
            ("/scope/source_specs/0/path", "../treeView.spec.ts"),
            (
                "/entries/0/source_spec",
                "cypress\\integration\\rendering\\treeView.spec.ts",
            ),
            ("/entries/0/fixture", "/tmp/escaped.mmd"),
            ("/entries/0/fixture", "fixtures/../escaped.mmd"),
            ("/entries/0/fixture", "fixtures/./treeView/case.mmd"),
            ("/entries/0/fixture", "fixtures//treeView/case.mmd"),
            ("/entries/0/fixture", "C:/escaped.mmd"),
            ("/entries/0/family", "../../escaped"),
            ("/entries/0/family", "treeView/child"),
        ];

        for (pointer, unsafe_value) in cases {
            let mut value = serde_json::to_value(manifest(vec![entry(1)])).unwrap();
            *value.pointer_mut(pointer).expect("string pointer") =
                serde_json::Value::String(unsafe_value.to_string());

            let parsed = serde_json::from_value::<CypressCorpusManifest>(value);
            assert!(
                parsed.is_err(),
                "unsafe value `{unsafe_value}` at `{pointer}` must be rejected"
            );
        }
    }

    #[test]
    fn committed_manifest_rejects_orphan_managed_artifacts() {
        let root = TempWorkspace::new();
        let entry = entry(1);
        materialize_active_entry(root.path(), &entry);

        let orphan = root
            .path()
            .join("fixtures/treeView/upstream_cypress_treeview_spec_orphan_999.mmd");
        fs::write(&orphan, b"treeView-beta\n  orphan\n").expect("orphan fixture");
        fs::write(orphan.with_extension("golden.json"), b"{}\n").expect("orphan semantic golden");
        fs::write(orphan.with_extension("layout.golden.json"), b"{}\n")
            .expect("orphan layout golden");
        let orphan_svg = root
            .path()
            .join("fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_orphan_999.svg");
        fs::write(orphan_svg, b"<svg/>\n").expect("orphan SVG");

        let failures = validate_cypress_corpus_manifest(root.path(), &manifest(vec![entry]));

        for suffix in [".mmd", ".golden.json", ".layout.golden.json", ".svg"] {
            assert!(
                failures.iter().any(|failure| {
                    failure.contains("orphan managed artifact") && failure.contains(suffix)
                }),
                "missing orphan diagnostic for {suffix}: {failures:#?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn committed_manifest_rejects_fixture_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = TempWorkspace::new();
        let outside = TempWorkspace::new();
        let entry = entry(1);
        materialize_active_entry(root.path(), &entry);
        let fixture = root.path().join(entry.fixture.as_path());
        let outside_fixture = outside.path().join("fixture.mmd");
        fs::write(&outside_fixture, fixture_bytes(entry.call_ordinal)).expect("outside fixture");
        fs::remove_file(&fixture).expect("remove fixture");
        symlink(&outside_fixture, &fixture).expect("fixture symlink");

        let failures = validate_cypress_corpus_manifest(root.path(), &manifest(vec![entry]));

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("escapes workspace")),
            "{failures:#?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn committed_manifest_rejects_parent_directory_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = TempWorkspace::new();
        let outside = TempWorkspace::new();
        let entry = entry(1);
        materialize_active_entry(root.path(), &entry);
        let family_dir = root.path().join("fixtures/treeView");
        let outside_family = outside.path().join("treeView");
        fs::rename(&family_dir, &outside_family).expect("move family outside workspace");
        symlink(&outside_family, &family_dir).expect("family directory symlink");

        let failures = validate_cypress_corpus_manifest(root.path(), &manifest(vec![entry]));

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("escapes workspace")),
            "{failures:#?}"
        );
    }

    #[test]
    fn source_check_reports_call_test_name_and_family_drift_separately() {
        let expected = entry(1);
        let manifest = manifest(vec![expected.clone()]);
        let mut drifted = observation(&expected);
        drifted.call = "renderSnapshot".to_string();
        drifted.test_name = "renamed test".to_string();
        drifted.family = "cynefin".to_string();

        let failures = validate_cypress_source_observations(&manifest, &[drifted]);

        for field in ["call", "test_name", "family"] {
            assert!(
                failures.iter().any(|failure| {
                    failure.contains(field)
                        && failure.contains("expected")
                        && failure.contains("found")
                }),
                "missing {field} diagnostic: {failures:#?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn source_spec_resolution_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let mermaid = TempWorkspace::new();
        let outside = TempWorkspace::new();
        let source = safe_relative("cypress/integration/rendering/treeView/treeView.spec.ts");
        let source_path = mermaid.path().join(source.as_path());
        fs::create_dir_all(source_path.parent().unwrap()).expect("source parent");
        let outside_source = outside.path().join("treeView.spec.ts");
        fs::write(&outside_source, b"describe('outside', () => {});\n").expect("outside source");
        symlink(&outside_source, &source_path).expect("source symlink");

        let error = resolve_cypress_source_spec_path(mermaid.path(), &source).unwrap_err();

        assert!(error.contains("escapes Mermaid checkout"), "{error}");
    }

    #[test]
    fn repository_manifest_matches_the_offline_committed_corpus() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");

        let failures = committed_cypress_corpus_alignment_failures(workspace_root);

        assert!(failures.is_empty(), "{failures:#?}");
    }
}
