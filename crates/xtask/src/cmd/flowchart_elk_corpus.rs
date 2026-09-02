use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::XtaskError;
use crate::cmd::{
    CypressCollectionEvidence, CypressSourceIdentity, CypressSourcePolicy, MERMAID_SOURCE_COMMIT,
    PINNED_MERMAID_VERSION, RawCypressCollection, ValidationArgument,
};
use crate::util::{is_canonical_sha256, sha256_hex};

pub(crate) const FLOWCHART_ELK_MANIFEST_RELATIVE_PATH: &str =
    "fixtures/_upstream/flowchart-elk-11.16.1/_manifest.json";
const SCHEMA_VERSION: u32 = 2;
const SCOPE_ID: &str = "flowchart-elk";
const SCOPE_DESCRIPTION: &str = "Mermaid 11.16 Flowchart ELK Cypress coverage";
const HISTORICAL_MERMAID_VERSION: &str = "11.16.1";
const HISTORICAL_MERMAID_TAG: &str = "mermaid@11.16.1";
const HISTORICAL_MERMAID_SOURCE_COMMIT: &str = "7ecca0cd7f1658ef74f4e7e91f925724ef403bbf";
const SOURCE_SPEC: &str = "cypress/integration/rendering/flowchart/flowchart-elk.spec.js";
const EXPECTED_ACTIVE_CALLS: usize = 64;
const EXPECTED_SKIPPED_REGISTRATIONS: usize = 1;
const EXPECTED_UNIQUE_LAYOUT_BODIES: usize = 60;
const SUPPLEMENTAL_FIXTURES: &[&str] = &[
    "fixtures/flowchart/upstream_html_demos_ashish2_example_009.mmd",
    "fixtures/flowchart/upstream_html_demos_flow_elk_example_001.mmd",
    "fixtures/flowchart/upstream_html_demos_flowchart_elk_flowchart_elk_001.mmd",
    "fixtures/flowchart/upstream_html_demos_knsv2_example_011.mmd",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FlowchartElkCollectionManifest {
    pub(crate) schema_version: u32,
    pub(crate) source_policy: CypressSourcePolicy,
    pub(crate) mermaid_version: String,
    pub(crate) mermaid_source_commit: String,
    pub(crate) collection: CypressCollectionEvidence,
    pub(crate) entries: Vec<FlowchartElkCollectionEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FlowchartElkCollectionEntry {
    pub(crate) source_ordinal: usize,
    pub(crate) registration: String,
    pub(crate) helper_ordinal: usize,
    pub(crate) call: String,
    pub(crate) validation: ValidationArgument,
    pub(crate) test_name: String,
    pub(crate) stem: String,
    pub(crate) mmd_sha256: String,
    pub(crate) layout_body_sha256: String,
    pub(crate) raw_sha256: String,
}

impl FlowchartElkCollectionEntry {
    pub(crate) fn snapshot(&self) -> bool {
        self.call == "imgSnapshotTest"
    }
}

fn valid_stem(stem: &str) -> bool {
    !stem.is_empty()
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn projected_flowchart_elk_manifest(
    collection: &RawCypressCollection,
) -> Result<FlowchartElkCollectionManifest, XtaskError> {
    if collection.scope.id != SCOPE_ID
        || collection.scope.description != SCOPE_DESCRIPTION
        || collection.scope.expected_active_calls != EXPECTED_ACTIVE_CALLS
        || collection.scope.expected_skipped_registrations != EXPECTED_SKIPPED_REGISTRATIONS
        || !collection.scope.reviewed_removals.is_empty()
    {
        return Err(XtaskError::AlignmentCheckFailed(
            "Flowchart ELK collection scope contract drift requires an explicit manifest review"
                .to_string(),
        ));
    }
    if collection.source.specs.len() != 1 || collection.source.specs[0].path != SOURCE_SPEC {
        return Err(XtaskError::AlignmentCheckFailed(format!(
            "Flowchart ELK collection must contain only {SOURCE_SPEC}"
        )));
    }
    let supplemental = collection
        .source
        .supplemental_fixtures
        .iter()
        .map(|fixture| fixture.path.as_str())
        .collect::<Vec<_>>();
    if supplemental != SUPPLEMENTAL_FIXTURES {
        return Err(XtaskError::AlignmentCheckFailed(format!(
            "Flowchart ELK supplemental fixture contract drift: expected {SUPPLEMENTAL_FIXTURES:?}, found {supplemental:?}"
        )));
    }

    let source_slug = crate::cmd::flowchart_elk_source_slug();
    let existing_identities = crate::cmd::flowchart_elk_source_identities()?;
    let mut entries = Vec::with_capacity(collection.calls.len());
    for call in &collection.calls {
        if call.source_spec != SOURCE_SPEC {
            return Err(XtaskError::AlignmentCheckFailed(format!(
                "Flowchart ELK call {} references unexpected source {}",
                call.ordinal, call.source_spec
            )));
        }
        if call.api {
            return Err(XtaskError::AlignmentCheckFailed(format!(
                "Flowchart ELK call {} uses the API/XSS path and cannot become a fixture",
                call.ordinal
            )));
        }
        if !crate::cmd::flowchart_elk_requested(&call.diagram, &call.options) {
            return Err(XtaskError::AlignmentCheckFailed(format!(
                "Flowchart ELK call {} no longer requests the ELK renderer",
                call.ordinal
            )));
        }
        let helper = crate::cmd::raw_collection_helper(call)?;
        let fixture =
            crate::cmd::materialize_cypress_fixture_source(&call.diagram, helper, &call.options)
                .map_err(|reason| {
                    XtaskError::AlignmentCheckFailed(format!(
                        "failed to materialize Flowchart ELK call {}: {reason}",
                        call.ordinal
                    ))
                })?;
        let test_name = crate::cmd::collected_registration_title(collection, call)?.to_string();
        let case = crate::cmd::flowchart_elk_fixture_identity(
            &test_name,
            &fixture,
            &source_slug,
            &existing_identities,
        );
        entries.push(FlowchartElkCollectionEntry {
            source_ordinal: call.source_ordinal,
            registration: call.registration.clone(),
            helper_ordinal: call.helper_ordinal,
            call: call.helper.clone(),
            validation: call.validation,
            test_name,
            stem: case.stem,
            mmd_sha256: case.mmd_sha256,
            layout_body_sha256: sha256_hex(case.layout_body_key.as_bytes()),
            raw_sha256: call.raw_sha256.clone(),
        });
    }

    Ok(FlowchartElkCollectionManifest {
        schema_version: SCHEMA_VERSION,
        source_policy: CypressSourcePolicy::Selected,
        mermaid_version: PINNED_MERMAID_VERSION.to_string(),
        mermaid_source_commit: MERMAID_SOURCE_COMMIT.to_string(),
        collection: crate::cmd::collection_evidence(collection),
        entries,
    })
}

pub(crate) fn load_committed_flowchart_elk_manifest(
    workspace_root: &Path,
) -> Result<FlowchartElkCollectionManifest, String> {
    let path = workspace_root.join(FLOWCHART_ELK_MANIFEST_RELATIVE_PATH);
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn validate_flowchart_elk_manifest_for_source(
    workspace_root: &Path,
    manifest: &FlowchartElkCollectionManifest,
    expected_source: CypressSourceIdentity<'_>,
    expected_policy: CypressSourcePolicy,
) -> Vec<String> {
    let mut failures = Vec::new();
    if manifest.schema_version != SCHEMA_VERSION {
        failures.push(format!(
            "Flowchart ELK manifest schema_version must be {SCHEMA_VERSION}, found {}",
            manifest.schema_version
        ));
    }
    if manifest.source_policy != expected_policy {
        failures.push(format!(
            "Flowchart ELK manifest source_policy must be {expected_policy:?}, found {:?}",
            manifest.source_policy
        ));
    }
    if manifest.mermaid_version != expected_source.version
        || manifest.mermaid_source_commit != expected_source.commit
    {
        failures.push(
            "Flowchart ELK manifest source identity disagrees with the expected Mermaid source"
                .to_string(),
        );
    }
    failures.extend(crate::cmd::committed_collection_evidence_failures(
        workspace_root,
        &manifest.collection,
        SCOPE_ID,
        SCOPE_DESCRIPTION,
        expected_source,
    ));
    if manifest.collection.expected_active_calls != EXPECTED_ACTIVE_CALLS
        || manifest.collection.expected_skipped_registrations != EXPECTED_SKIPPED_REGISTRATIONS
        || !manifest.collection.reviewed_removals.is_empty()
    {
        failures.push("Flowchart ELK collection count/removal contract drifted".to_string());
    }
    if manifest
        .collection
        .registrations
        .iter()
        .any(|registration| !registration.skipped)
    {
        failures.push(
            "Flowchart ELK manifest must retain only skipped registration evidence".to_string(),
        );
    }
    let source_specs = manifest
        .collection
        .source
        .specs
        .iter()
        .map(|spec| spec.path.as_str())
        .collect::<Vec<_>>();
    if source_specs != [SOURCE_SPEC] {
        failures.push(format!(
            "Flowchart ELK manifest must contain only {SOURCE_SPEC}; found {source_specs:?}"
        ));
    }
    let supplemental = manifest
        .collection
        .source
        .supplemental_fixtures
        .iter()
        .map(|fixture| fixture.path.as_str())
        .collect::<Vec<_>>();
    if supplemental != SUPPLEMENTAL_FIXTURES {
        failures.push(format!(
            "Flowchart ELK supplemental fixture contract drift: expected {SUPPLEMENTAL_FIXTURES:?}, found {supplemental:?}"
        ));
    }
    if manifest.entries.len() != EXPECTED_ACTIVE_CALLS {
        failures.push(format!(
            "Flowchart ELK manifest must contain {EXPECTED_ACTIVE_CALLS} calls, found {}",
            manifest.entries.len()
        ));
    }

    let mut active_registrations = BTreeMap::new();
    let mut identities = BTreeSet::new();
    let mut helper_ordinals = BTreeMap::<&str, usize>::new();
    let mut layout_bodies = BTreeSet::new();
    for (index, entry) in manifest.entries.iter().enumerate() {
        let expected_ordinal = index + 1;
        if entry.source_ordinal != expected_ordinal {
            failures.push(format!(
                "Flowchart ELK call order drift at manifest entry {expected_ordinal}: source_ordinal={}",
                entry.source_ordinal
            ));
        }
        if !identities.insert(entry.source_ordinal) {
            failures.push(format!(
                "Flowchart ELK manifest has duplicate call identity {SOURCE_SPEC}#{}",
                entry.source_ordinal
            ));
        }
        if let Some(previous_title) =
            active_registrations.insert(entry.registration.as_str(), entry.test_name.as_str())
            && previous_title != entry.test_name
        {
            failures.push(format!(
                "Flowchart ELK registration {} has inconsistent title evidence",
                entry.registration
            ));
        }
        let helper_ordinal = helper_ordinals.entry(&entry.registration).or_insert(1);
        if entry.helper_ordinal != *helper_ordinal {
            failures.push(format!(
                "Flowchart ELK helper ordinal drift for {}: expected {}, found {}",
                entry.registration, *helper_ordinal, entry.helper_ordinal
            ));
        }
        *helper_ordinal += 1;
        let valid_call = matches!(entry.call.as_str(), "imgSnapshotTest" | "renderGraph");
        if !valid_call {
            failures.push(format!(
                "Flowchart ELK call {} has invalid helper evidence",
                entry.source_ordinal
            ));
        }
        if !valid_stem(&entry.stem) {
            failures.push(format!(
                "Flowchart ELK call {} has invalid fixture stem {:?}",
                entry.source_ordinal, entry.stem
            ));
        }
        for (description, digest) in [
            ("MMD", &entry.mmd_sha256),
            ("layout body", &entry.layout_body_sha256),
            ("raw call", &entry.raw_sha256),
        ] {
            if !is_canonical_sha256(digest) {
                failures.push(format!(
                    "Flowchart ELK call {} has a non-canonical {description} SHA-256",
                    entry.source_ordinal
                ));
            }
        }
        layout_bodies.insert(entry.layout_body_sha256.as_str());

        let fixture_path = workspace_root
            .join("fixtures/flowchart")
            .join(format!("{}.mmd", entry.stem));
        if fixture_path.is_file() {
            match fs::read(&fixture_path) {
                Ok(bytes) if sha256_hex(&bytes) == entry.mmd_sha256 => {}
                Ok(bytes) => failures.push(format!(
                    "Flowchart ELK fixture SHA-256 drift for {}: expected {}, found {}",
                    fixture_path.display(),
                    entry.mmd_sha256,
                    sha256_hex(&bytes)
                )),
                Err(error) => failures.push(format!(
                    "failed to read Flowchart ELK fixture {}: {error}",
                    fixture_path.display()
                )),
            }
        }
    }
    for effect in &manifest.collection.runtime_effects {
        if !active_registrations.contains_key(effect.registration.as_str()) {
            failures.push(format!(
                "Flowchart ELK runtime effect {} references an unknown active call registration",
                effect.operation
            ));
        }
    }
    if layout_bodies.len() != EXPECTED_UNIQUE_LAYOUT_BODIES {
        failures.push(format!(
            "Flowchart ELK manifest must contain {EXPECTED_UNIQUE_LAYOUT_BODIES} unique layout bodies, found {}",
            layout_bodies.len()
        ));
    }
    failures
}

fn validate_historical_flowchart_elk_manifest(
    workspace_root: &Path,
    manifest: &FlowchartElkCollectionManifest,
) -> Vec<String> {
    validate_flowchart_elk_manifest_for_source(
        workspace_root,
        manifest,
        CypressSourceIdentity {
            package: "mermaid",
            version: HISTORICAL_MERMAID_VERSION,
            tag: HISTORICAL_MERMAID_TAG,
            commit: HISTORICAL_MERMAID_SOURCE_COMMIT,
        },
        CypressSourcePolicy::Historical,
    )
}

fn validate_selected_flowchart_elk_manifest(
    workspace_root: &Path,
    manifest: &FlowchartElkCollectionManifest,
) -> Vec<String> {
    validate_flowchart_elk_manifest_for_source(
        workspace_root,
        manifest,
        CypressSourceIdentity::selected(),
        CypressSourcePolicy::Selected,
    )
}

pub(crate) fn validate_flowchart_elk_manifest(
    workspace_root: &Path,
    manifest: &FlowchartElkCollectionManifest,
) -> Vec<String> {
    validate_historical_flowchart_elk_manifest(workspace_root, manifest)
}

pub(crate) fn committed_flowchart_elk_collection_failures(workspace_root: &Path) -> Vec<String> {
    match load_committed_flowchart_elk_manifest(workspace_root) {
        Ok(manifest) => validate_flowchart_elk_manifest(workspace_root, &manifest),
        Err(error) => vec![error],
    }
}

pub(crate) fn project_flowchart_elk_collection(
    collection: &RawCypressCollection,
    refresh: bool,
) -> Result<(), XtaskError> {
    let projected = projected_flowchart_elk_manifest(collection)?;
    let workspace_root = crate::cmd::workspace_root();
    let projected_failures = validate_selected_flowchart_elk_manifest(&workspace_root, &projected);
    if !projected_failures.is_empty() {
        return Err(XtaskError::AlignmentCheckFailed(
            projected_failures.join("\n"),
        ));
    }
    if !refresh {
        let committed = load_committed_flowchart_elk_manifest(&workspace_root)
            .map_err(XtaskError::AlignmentCheckFailed)?;
        if committed != projected {
            return Err(XtaskError::AlignmentCheckFailed(
                "committed Flowchart ELK collection differs from the pinned executable collection; rerun project-upstream-cypress-collection --scope flowchart-elk --input <collection.json> --refresh after review"
                    .to_string(),
            ));
        }
        let failures = validate_selected_flowchart_elk_manifest(&workspace_root, &committed);
        return if failures.is_empty() {
            Ok(())
        } else {
            Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
        };
    }

    let committed = load_committed_flowchart_elk_manifest(&workspace_root)
        .map_err(XtaskError::AlignmentCheckFailed)?;
    if committed.source_policy != CypressSourcePolicy::Selected {
        return Err(XtaskError::AlignmentCheckFailed(
            "historical Flowchart ELK evidence is immutable; refresh it from a separately reviewed historical checkout"
                .to_string(),
        ));
    }

    let path = workspace_root.join(FLOWCHART_ELK_MANIFEST_RELATIVE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let json = serde_json::to_string_pretty(&projected)?;
    fs::write(&path, format!("{json}\n")).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })?;
    let failures = validate_selected_flowchart_elk_manifest(&workspace_root, &projected);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_manifest_matches_the_offline_collected_scope() {
        let failures = committed_flowchart_elk_collection_failures(&crate::cmd::workspace_root());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn fixture_stems_are_portable_components() {
        assert!(valid_stem("upstream_cypress_flowchart_elk_spec_case_001"));
        for stem in ["", "../case", "case/name", "case.mmd", "case\\name"] {
            assert!(!valid_stem(stem), "{stem:?} must be rejected");
        }
    }
}
