use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::XtaskError;
use crate::cmd::{
    CypressCollectionEvidence, MERMAID_SOURCE_COMMIT, MERMAID_SOURCE_TAG, PINNED_MERMAID_VERSION,
    RawCypressCollection, RawRenderCall, ValidationArgument,
};
use crate::util::{is_canonical_sha256, sha256_hex};

pub(crate) const MANIFEST_RELATIVE_PATH: &str = "fixtures/_upstream/cypress-11.16.1/_manifest.json";
const SCHEMA_VERSION: u32 = 2;
const SCOPE_ID: &str = "new-family";
const SCOPE_DESCRIPTION: &str = "Mermaid 11.16 new-family Cypress render calls";
const MANAGED_FIXTURE_PREFIXES: &[&str] = &[
    "upstream_cypress_treeview_spec_",
    "upstream_cypress_cynefin_spec_",
    "upstream_cypress_railroad_spec_",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct SafeRelativePath(String);

impl SafeRelativePath {
    pub(crate) fn parse(value: String) -> Result<Self, String> {
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCorpusManifest {
    pub(crate) schema_version: u32,
    pub(crate) mermaid_version: String,
    pub(crate) mermaid_source_commit: String,
    pub(crate) collection: CypressCollectionEvidence,
    pub(crate) scope: CypressCorpusScope,
    pub(crate) entries: Vec<CypressCorpusEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCorpusScope {
    pub(crate) description: String,
    pub(crate) source_specs: Vec<CypressCorpusSourceSpec>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCorpusSourceSpec {
    pub(crate) path: SafeRelativePath,
    pub(crate) expected_calls: usize,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCorpusEntry {
    pub(crate) source_spec: SafeRelativePath,
    pub(crate) call_ordinal: usize,
    pub(crate) registration: String,
    pub(crate) helper_ordinal: usize,
    pub(crate) call: String,
    pub(crate) validation: ValidationArgument,
    pub(crate) test_name: String,
    pub(crate) family: SafePathComponent,
    pub(crate) route: CypressCorpusRoute,
    pub(crate) fixture: SafeRelativePath,
    pub(crate) mmd_sha256: String,
    pub(crate) raw_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CypressCorpusRoute {
    Active,
    Deferred,
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
    let collection = &manifest.collection;
    failures.extend(crate::cmd::committed_collection_evidence_failures(
        workspace_root,
        collection,
        SCOPE_ID,
        SCOPE_DESCRIPTION,
    ));
    if collection.expected_active_calls != manifest.entries.len() {
        failures.push(format!(
            "Cypress corpus collection expected {} active calls, found {} entries",
            collection.expected_active_calls,
            manifest.entries.len()
        ));
    }
    if !collection.source.supplemental_fixtures.is_empty() {
        failures
            .push("new-family Cypress corpus must not declare supplemental fixtures".to_string());
    }
    if collection
        .registrations
        .iter()
        .any(|registration| !registration.skipped)
    {
        failures.push(
            "new-family Cypress corpus must retain only skipped registration evidence".to_string(),
        );
    }
    let actual_specs: Vec<(&str, &str)> = manifest
        .scope
        .source_specs
        .iter()
        .map(|spec| (spec.path.as_str(), spec.sha256.as_str()))
        .collect();
    let collected_specs = collection
        .source
        .specs
        .iter()
        .map(|spec| (spec.path.as_str(), spec.sha256.as_str()))
        .collect::<Vec<_>>();
    if actual_specs != collected_specs {
        failures.push(format!(
            "Cypress corpus source_specs disagree with collector evidence: expected {collected_specs:?}, found {actual_specs:?}"
        ));
    }
    for spec in &manifest.scope.source_specs {
        if !is_canonical_sha256(&spec.sha256) {
            failures.push(format!(
                "Cypress corpus source spec {} has a non-canonical SHA-256",
                spec.path
            ));
        }
        let actual_calls = manifest
            .entries
            .iter()
            .filter(|entry| entry.source_spec == spec.path)
            .count();
        if actual_calls != spec.expected_calls {
            failures.push(format!(
                "Cypress corpus source spec {} expected {} calls, found {actual_calls}",
                spec.path, spec.expected_calls
            ));
        }
    }

    let mut registrations = BTreeMap::new();
    let mut helper_ordinals = BTreeMap::<&str, usize>::new();
    for entry in &manifest.entries {
        if let Some(previous) = registrations.insert(
            entry.registration.as_str(),
            (entry.source_spec.as_str(), entry.test_name.as_str()),
        ) && previous != (entry.source_spec.as_str(), entry.test_name.as_str())
        {
            failures.push(format!(
                "Cypress corpus registration {} has inconsistent source or title evidence",
                entry.registration
            ));
        }
        let expected_helper_ordinal = helper_ordinals.entry(&entry.registration).or_insert(1);
        if entry.helper_ordinal != *expected_helper_ordinal {
            failures.push(format!(
                "Cypress corpus helper ordinal drift for {}: expected {}, found {}",
                entry.registration, *expected_helper_ordinal, entry.helper_ordinal
            ));
        }
        *expected_helper_ordinal += 1;
        if entry.call != "imgSnapshotTest" {
            failures.push(format!(
                "new-family Cypress corpus entry {}#{} uses unsupported helper {:?}",
                entry.source_spec, entry.call_ordinal, entry.call
            ));
        }
    }
    for effect in &collection.runtime_effects {
        if !registrations
            .get(effect.registration.as_str())
            .is_some_and(|(source_spec, _)| *source_spec == effect.source_spec)
        {
            failures.push(format!(
                "Cypress corpus runtime effect {} references an unknown active call registration",
                effect.operation
            ));
        }
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
        if entry.registration.is_empty() || entry.helper_ordinal == 0 {
            failures.push(format!(
                "Cypress corpus manifest entry {}#{} has invalid registration evidence",
                entry.source_spec, entry.call_ordinal
            ));
        }
        if !is_canonical_sha256(&entry.raw_sha256) {
            failures.push(format!(
                "Cypress corpus manifest entry {}#{} has a non-canonical raw SHA-256",
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

fn detector_input(source: &str) -> &str {
    let source = source.trim_start_matches(char::is_whitespace);
    let mut pieces = source.split_inclusive('\n');
    let Some(first) = pieces.next() else {
        return source;
    };
    if first.trim_end_matches(['\n', '\r']).trim_end() != "---" {
        return source;
    }
    let mut consumed = first.len();
    for piece in pieces {
        consumed += piece.len();
        if piece.trim_end_matches(['\n', '\r']).trim_end() == "---" {
            return source.get(consumed..).unwrap_or("");
        }
    }
    source
}

fn collected_new_family_fixture(
    collection: &RawCypressCollection,
    call: &RawRenderCall,
) -> Result<(String, String, String), XtaskError> {
    if call.api {
        return Err(XtaskError::AlignmentCheckFailed(format!(
            "collected Cypress call {} uses the API/XSS path and cannot become a fixture",
            call.ordinal
        )));
    }
    let helper = crate::cmd::raw_collection_helper(call)?;
    let fixture =
        crate::cmd::materialize_cypress_fixture_source(&call.diagram, helper, &call.options)
            .map_err(|reason| {
                XtaskError::AlignmentCheckFailed(format!(
                    "failed to materialize collected Cypress call {}: {reason}",
                    call.ordinal
                ))
            })?;
    let mut config = merman::MermaidConfig::default();
    let detected = merman::detect::DetectorRegistry::pinned_mermaid_baseline()
        .detect_type(detector_input(&fixture), &mut config)
        .map_err(|error| {
            XtaskError::AlignmentCheckFailed(format!(
                "failed to detect collected Cypress call {}: {error}",
                call.ordinal
            ))
        })?;
    let family = merman_core::diagram_type_metadata_id(detected)
        .ok_or_else(|| {
            XtaskError::AlignmentCheckFailed(format!(
                "collected Cypress call {} detected unsupported family {detected:?}",
                call.ordinal
            ))
        })?
        .to_string();
    let title = crate::cmd::collected_registration_title(collection, call)?.to_string();
    Ok((fixture, family, title))
}

fn write_collection_replacements(replacements: &[(PathBuf, Vec<u8>)]) -> Result<(), XtaskError> {
    let originals = replacements
        .iter()
        .map(|(path, _)| {
            fs::read(path)
                .map(|bytes| (path.clone(), bytes))
                .map_err(|source| XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (path, bytes) in replacements {
        if let Err(source) = fs::write(path, bytes) {
            let mut rollback_failures = Vec::new();
            for (original_path, original_bytes) in &originals {
                if let Err(error) = fs::write(original_path, original_bytes) {
                    rollback_failures.push(format!("{}: {error}", original_path.display()));
                }
            }
            return Err(XtaskError::WriteFile {
                path: if rollback_failures.is_empty() {
                    path.display().to_string()
                } else {
                    format!(
                        "{} (rollback failures: {})",
                        path.display(),
                        rollback_failures.join(", ")
                    )
                },
                source,
            });
        }
    }
    Ok(())
}

pub(crate) fn project_new_family_cypress_collection(
    collection: &RawCypressCollection,
    refresh: bool,
) -> Result<(), XtaskError> {
    let workspace_root = crate::cmd::workspace_root();
    let evidence = crate::cmd::collection_evidence(collection);
    let evidence_failures = crate::cmd::committed_collection_evidence_failures(
        &workspace_root,
        &evidence,
        SCOPE_ID,
        SCOPE_DESCRIPTION,
    );
    if !evidence_failures.is_empty() {
        return Err(XtaskError::AlignmentCheckFailed(
            evidence_failures.join("\n"),
        ));
    }
    let committed = load_committed_cypress_corpus_manifest(&workspace_root)
        .map_err(XtaskError::AlignmentCheckFailed)?;
    let collected_source_paths = collection
        .source
        .specs
        .iter()
        .map(|spec| spec.path.as_str())
        .collect::<Vec<_>>();
    let committed_source_paths = committed
        .scope
        .source_specs
        .iter()
        .map(|spec| spec.path.as_str())
        .collect::<Vec<_>>();
    if collection.scope.expected_active_calls != committed.entries.len()
        || collection.scope.expected_skipped_registrations != 0
        || !collection.scope.reviewed_skipped_registrations.is_empty()
        || !collection.scope.reviewed_removals.is_empty()
        || !collection.source.supplemental_fixtures.is_empty()
        || collected_source_paths != committed_source_paths
    {
        return Err(XtaskError::AlignmentCheckFailed(
            "new-family Cypress collection scope drift requires an explicit corpus review"
                .to_string(),
        ));
    }
    let mut entries = Vec::with_capacity(collection.calls.len());
    let mut replacements = Vec::with_capacity(collection.calls.len() + 1);
    for call in &collection.calls {
        let existing = committed
            .entries
            .iter()
            .find(|entry| {
                entry.source_spec.as_str() == call.source_spec
                    && entry.call_ordinal == call.source_ordinal
            })
            .ok_or_else(|| {
                XtaskError::AlignmentCheckFailed(format!(
                    "collected Cypress call {}#{} has no reviewed corpus route",
                    call.source_spec, call.source_ordinal
                ))
            })?;
        let (fixture, family, title) = collected_new_family_fixture(collection, call)?;
        if existing.call != call.helper
            || existing.test_name != title
            || existing.family.as_str() != family
        {
            return Err(XtaskError::AlignmentCheckFailed(format!(
                "collected Cypress identity drift for {}#{} requires an explicit corpus review",
                call.source_spec, call.source_ordinal
            )));
        }
        let mut entry = existing.clone();
        entry.registration.clone_from(&call.registration);
        entry.helper_ordinal = call.helper_ordinal;
        entry.validation = call.validation;
        entry.mmd_sha256 = sha256_hex(fixture.as_bytes());
        entry.raw_sha256.clone_from(&call.raw_sha256);
        replacements.push((
            workspace_root.join(entry.fixture.as_path()),
            fixture.into_bytes(),
        ));
        entries.push(entry);
    }
    if entries.len() != committed.entries.len() {
        return Err(XtaskError::AlignmentCheckFailed(format!(
            "collected Cypress corpus has {} calls but the reviewed manifest has {} entries; declare reviewed removals or routes before refresh",
            entries.len(),
            committed.entries.len()
        )));
    }
    let source_specs = collection
        .source
        .specs
        .iter()
        .map(|spec| {
            let expected_calls = collection
                .calls
                .iter()
                .filter(|call| call.source_spec == spec.path)
                .count();
            Ok(CypressCorpusSourceSpec {
                path: SafeRelativePath::parse(spec.path.clone())
                    .map_err(XtaskError::AlignmentCheckFailed)?,
                expected_calls,
                sha256: spec.sha256.clone(),
            })
        })
        .collect::<Result<Vec<_>, XtaskError>>()?;
    let projected = CypressCorpusManifest {
        schema_version: SCHEMA_VERSION,
        mermaid_version: PINNED_MERMAID_VERSION.to_string(),
        mermaid_source_commit: MERMAID_SOURCE_COMMIT.to_string(),
        collection: evidence,
        scope: CypressCorpusScope {
            description: SCOPE_DESCRIPTION.to_string(),
            source_specs,
        },
        entries,
    };

    if !refresh {
        if projected != committed {
            return Err(XtaskError::AlignmentCheckFailed(format!(
                "committed Cypress corpus manifest differs from the pinned executable collection; rerun project-upstream-cypress-collection --scope new-family --input <collection.json> --refresh after review"
            )));
        }
        let failures = validate_pinned_manifest_contract(&workspace_root, &projected)
            .into_iter()
            .chain(validate_cypress_corpus_manifest(
                &workspace_root,
                &projected,
            ))
            .collect::<Vec<_>>();
        return if failures.is_empty() {
            Ok(())
        } else {
            Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
        };
    }

    let manifest_json = serde_json::to_string_pretty(&projected)?;
    replacements.push((
        workspace_root.join(MANIFEST_RELATIVE_PATH),
        format!("{manifest_json}\n").into_bytes(),
    ));
    write_collection_replacements(&replacements)?;
    let failures = committed_cypress_corpus_alignment_failures(&workspace_root);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
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
            registration: format!("TreeView Diagram > case {ordinal}"),
            helper_ordinal: 1,
            call: "imgSnapshotTest".to_string(),
            validation: ValidationArgument::Absent,
            test_name: format!("case {ordinal}"),
            family: safe_component("treeView"),
            route: CypressCorpusRoute::Active,
            fixture: safe_relative(fixture),
            mmd_sha256: sha256_hex(&fixture_bytes(ordinal)),
            raw_sha256: "a".repeat(64),
        }
    }

    fn collection_evidence(expected_active_calls: usize) -> CypressCollectionEvidence {
        let source_spec = "cypress/integration/rendering/treeView/treeView.spec.ts";
        CypressCollectionEvidence {
            scope_id: "test-corpus".to_string(),
            description: "test corpus".to_string(),
            expected_active_calls,
            expected_skipped_registrations: 0,
            reviewed_skipped_registrations: Vec::new(),
            reviewed_removals: Vec::new(),
            source: crate::cmd::CypressCollectionSourceEvidence {
                package: "mermaid".to_string(),
                version: PINNED_MERMAID_VERSION.to_string(),
                tag: MERMAID_SOURCE_TAG.to_string(),
                commit: MERMAID_SOURCE_COMMIT.to_string(),
                test_config: crate::cmd::TestConfigEvidence {
                    path: "cypress.config.ts".to_string(),
                    sha256: "a".repeat(64),
                    spec_pattern: "cypress/integration/**/*.{js,ts}".to_string(),
                },
                render_helper: crate::cmd::DigestPath {
                    path: "cypress/helpers/util.ts".to_string(),
                    sha256: "b".repeat(64),
                },
                specs: vec![crate::cmd::DigestPath {
                    path: source_spec.to_string(),
                    sha256: "c".repeat(64),
                }],
                supplemental_fixtures: Vec::new(),
            },
            collector: crate::cmd::CypressCollectorEvidence {
                files: Vec::new(),
                scope_catalog_sha256: "d".repeat(64),
                node_version: "22.14.0".to_string(),
                pnpm_version: "10.30.3".to_string(),
                esbuild_version: "0.25.12".to_string(),
                upstream_lock: crate::cmd::DigestPath {
                    path: "pnpm-lock.yaml".to_string(),
                    sha256: "e".repeat(64),
                },
            },
            registrations: Vec::new(),
            runtime_effects: Vec::new(),
        }
    }

    fn manifest(entries: Vec<CypressCorpusEntry>) -> CypressCorpusManifest {
        let expected_active_calls = entries.len();
        CypressCorpusManifest {
            schema_version: SCHEMA_VERSION,
            mermaid_version: PINNED_MERMAID_VERSION.to_string(),
            mermaid_source_commit: MERMAID_SOURCE_COMMIT.to_string(),
            collection: collection_evidence(expected_active_calls),
            scope: CypressCorpusScope {
                description: "test corpus".to_string(),
                source_specs: vec![CypressCorpusSourceSpec {
                    path: safe_relative("cypress/integration/rendering/treeView/treeView.spec.ts"),
                    expected_calls: expected_active_calls,
                    sha256: "b".repeat(64),
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
    fn repository_manifest_matches_the_offline_committed_corpus() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");

        let failures = committed_cypress_corpus_alignment_failures(workspace_root);

        assert!(failures.is_empty(), "{failures:#?}");
    }
}
