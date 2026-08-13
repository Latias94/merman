use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use crate::XtaskError;
use crate::cmd::{MERMAID_SOURCE_COMMIT, MERMAID_SOURCE_TAG, PINNED_MERMAID_VERSION};
use crate::util::{is_canonical_sha256, sha256_hex};

const COLLECTION_SCHEMA_VERSION: u32 = 1;
const COLLECTION_KIND: &str = "merman-upstream-cypress-collection";
const NODE_VERSION: &str = "22.14.0";
const PNPM_VERSION: &str = "10.30.3";
const ESBUILD_VERSION: &str = "0.25.12";
const TEST_CONFIG_PATH: &str = "cypress.config.ts";
const TEST_SPEC_PATTERN: &str = "cypress/integration/**/*.{js,ts}";
const RENDER_HELPER_PATH: &str = "cypress/helpers/util.ts";
const UPSTREAM_LOCK_PATH: &str = "pnpm-lock.yaml";
const COLLECTOR_FILE_PATHS: [&str; 3] = [
    "tools/upstreams/cypress-collector/collect.mjs",
    "tools/upstreams/cypress-collector/scopes.json",
    "tools/upstreams/cypress-collector/worker.mjs",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawCypressCollection {
    pub(crate) schema_version: u32,
    pub(crate) kind: String,
    pub(crate) scope: RawCollectionScope,
    pub(crate) source: RawCollectionSource,
    pub(crate) collector: RawCollectorIdentity,
    pub(crate) registrations: Vec<RawRegistration>,
    pub(crate) calls: Vec<RawRenderCall>,
    pub(crate) runtime_effects: Vec<RawRuntimeEffect>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawCollectionScope {
    pub(crate) id: String,
    pub(crate) description: String,
    pub(crate) expected_active_calls: usize,
    pub(crate) expected_skipped_registrations: usize,
    pub(crate) reviewed_skipped_registrations: Vec<ReviewedRegistration>,
    pub(crate) reviewed_removals: Vec<ReviewedRemoval>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawCollectionSource {
    pub(crate) package: String,
    pub(crate) version: String,
    pub(crate) tag: String,
    pub(crate) commit: String,
    pub(crate) test_config: RawTestConfigIdentity,
    pub(crate) render_helper: DigestPath,
    pub(crate) specs: Vec<DigestPath>,
    pub(crate) supplemental_fixtures: Vec<DigestPath>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawTestConfigIdentity {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) spec_pattern: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawCollectorIdentity {
    pub(crate) files: Vec<DigestPath>,
    pub(crate) scope_catalog_sha256: String,
    pub(crate) node_version: String,
    pub(crate) pnpm_version: String,
    pub(crate) esbuild_version: String,
    pub(crate) upstream_lock: DigestPath,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawRegistration {
    pub(crate) source_spec: String,
    pub(crate) ordinal: usize,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) skipped: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawRenderCall {
    pub(crate) source_spec: String,
    pub(crate) source_ordinal: usize,
    pub(crate) ordinal: usize,
    pub(crate) registration: String,
    pub(crate) helper_ordinal: usize,
    pub(crate) helper: String,
    pub(crate) diagram: String,
    pub(crate) options: serde_json::Value,
    pub(crate) api: bool,
    pub(crate) validation: ValidationArgument,
    pub(crate) raw_sha256: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ValidationArgument {
    #[default]
    Absent,
    Present,
}

impl ValidationArgument {
    fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Present => "present",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawRuntimeEffect {
    pub(crate) source_spec: String,
    pub(crate) registration: String,
    pub(crate) operation: String,
    pub(crate) selector: String,
    pub(crate) argument_kinds: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DigestPath {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewedRegistration {
    pub(crate) registration: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewedRemoval {
    pub(crate) source_spec: String,
    pub(crate) registration: String,
    pub(crate) helper_ordinal: usize,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCollectionEvidence {
    pub(crate) scope_id: String,
    pub(crate) description: String,
    pub(crate) expected_active_calls: usize,
    pub(crate) expected_skipped_registrations: usize,
    pub(crate) reviewed_skipped_registrations: Vec<ReviewedRegistration>,
    pub(crate) reviewed_removals: Vec<ReviewedRemoval>,
    pub(crate) source: CypressCollectionSourceEvidence,
    pub(crate) collector: CypressCollectorEvidence,
    pub(crate) registrations: Vec<RegistrationEvidence>,
    pub(crate) runtime_effects: Vec<RuntimeEffectEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCollectionSourceEvidence {
    pub(crate) package: String,
    pub(crate) version: String,
    pub(crate) tag: String,
    pub(crate) commit: String,
    pub(crate) test_config: TestConfigEvidence,
    pub(crate) render_helper: DigestPath,
    pub(crate) specs: Vec<DigestPath>,
    pub(crate) supplemental_fixtures: Vec<DigestPath>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TestConfigEvidence {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) spec_pattern: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CypressCollectorEvidence {
    pub(crate) files: Vec<DigestPath>,
    pub(crate) scope_catalog_sha256: String,
    pub(crate) node_version: String,
    pub(crate) pnpm_version: String,
    pub(crate) esbuild_version: String,
    pub(crate) upstream_lock: DigestPath,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegistrationEvidence {
    pub(crate) source_spec: String,
    pub(crate) ordinal: usize,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) skipped: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeEffectEvidence {
    pub(crate) source_spec: String,
    pub(crate) registration: String,
    pub(crate) operation: String,
    pub(crate) selector: String,
    pub(crate) argument_kinds: Vec<String>,
}

fn portable_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && !value.contains(':')
        && !value.chars().any(char::is_control)
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn validate_digest_path(value: &DigestPath, description: &str, failures: &mut Vec<String>) {
    if !portable_relative_path(&value.path) {
        failures.push(format!(
            "{description} path must be a portable repository-relative path: {:?}",
            value.path
        ));
    }
    if !is_canonical_sha256(&value.sha256) {
        failures.push(format!(
            "{description} {} has a non-canonical SHA-256",
            value.path
        ));
    }
}

fn has_exact_collector_file_paths(files: &[DigestPath]) -> bool {
    files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>()
        == COLLECTOR_FILE_PATHS.into_iter().collect::<BTreeSet<_>>()
}

fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => {
            serde_json::to_string(value).expect("serializing a JSON string cannot fail")
        }
        serde_json::Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        serde_json::Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(key, _)| *key);
            let fields = entries
                .into_iter()
                .map(|(key, value)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key)
                            .expect("serializing a JSON object key cannot fail"),
                        canonical_json(value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{fields}}}")
        }
    }
}

fn raw_call_sha256(call: &RawRenderCall) -> String {
    let identity = format!(
        "{{\"api\":{},\"diagram\":{},\"helper\":{},\"options\":{},\"registration\":{},\"validation\":{}}}",
        call.api,
        serde_json::to_string(&call.diagram).expect("serializing a diagram string cannot fail"),
        serde_json::to_string(&call.helper).expect("serializing a helper string cannot fail"),
        canonical_json(&call.options),
        serde_json::to_string(&call.registration)
            .expect("serializing a registration string cannot fail"),
        serde_json::to_string(call.validation.as_str())
            .expect("serializing a validation string cannot fail"),
    );
    sha256_hex(identity.as_bytes())
}

fn raw_collection_failures(collection: &RawCypressCollection) -> Vec<String> {
    let mut failures = Vec::new();
    if collection.schema_version != COLLECTION_SCHEMA_VERSION {
        failures.push(format!(
            "Cypress collection schemaVersion must be {COLLECTION_SCHEMA_VERSION}, found {}",
            collection.schema_version
        ));
    }
    if collection.kind != COLLECTION_KIND {
        failures.push(format!(
            "Cypress collection kind must be `{COLLECTION_KIND}`, found {:?}",
            collection.kind
        ));
    }
    if collection.source.package != "mermaid"
        || collection.source.version != PINNED_MERMAID_VERSION
        || collection.source.tag != MERMAID_SOURCE_TAG
        || collection.source.commit != MERMAID_SOURCE_COMMIT
    {
        failures.push(
            "Cypress collection source identity disagrees with the selected Mermaid graph"
                .to_string(),
        );
    }
    if collection.scope.id.is_empty()
        || collection.scope.description.is_empty()
        || collection.scope.expected_active_calls == 0
    {
        failures.push(
            "Cypress collection scope identity and active-call count must be non-empty".to_string(),
        );
    }
    validate_digest_path(
        &DigestPath {
            path: collection.source.test_config.path.clone(),
            sha256: collection.source.test_config.sha256.clone(),
        },
        "Cypress test config",
        &mut failures,
    );
    if collection.source.test_config.path != TEST_CONFIG_PATH
        || collection.source.test_config.spec_pattern != TEST_SPEC_PATTERN
    {
        failures.push(format!(
            "Cypress collection has unsupported test config identity {:?} / {:?}",
            collection.source.test_config.path, collection.source.test_config.spec_pattern
        ));
    }
    validate_digest_path(
        &collection.source.render_helper,
        "Cypress render helper",
        &mut failures,
    );
    if collection.source.render_helper.path != RENDER_HELPER_PATH {
        failures.push(format!(
            "Cypress collection has unsupported render helper path {:?}",
            collection.source.render_helper.path
        ));
    }
    if collection.source.specs.is_empty() {
        failures.push("Cypress collection must contain at least one source spec".to_string());
    }
    for spec in &collection.source.specs {
        validate_digest_path(spec, "Cypress source spec", &mut failures);
    }
    for fixture in &collection.source.supplemental_fixtures {
        validate_digest_path(fixture, "Cypress supplemental fixture", &mut failures);
    }
    let mut collector_files = BTreeSet::new();
    for file in &collection.collector.files {
        validate_digest_path(file, "Cypress collector file", &mut failures);
        if !collector_files.insert(file.path.as_str()) {
            failures.push(format!(
                "Cypress collection contains duplicate collector file {}",
                file.path
            ));
        }
    }
    if !has_exact_collector_file_paths(&collection.collector.files) {
        failures.push(format!(
            "Cypress collector file identity set drift: expected {COLLECTOR_FILE_PATHS:?}, found {collector_files:?}"
        ));
    }
    validate_digest_path(
        &collection.collector.upstream_lock,
        "Cypress collector upstream lock",
        &mut failures,
    );
    if collection.collector.upstream_lock.path != UPSTREAM_LOCK_PATH {
        failures.push(format!(
            "Cypress collection has unsupported upstream lock path {:?}",
            collection.collector.upstream_lock.path
        ));
    }
    if !is_canonical_sha256(&collection.collector.scope_catalog_sha256) {
        failures.push("Cypress collector scope catalog has a non-canonical SHA-256".to_string());
    }
    if collection
        .collector
        .files
        .iter()
        .find(|file| file.path == "tools/upstreams/cypress-collector/scopes.json")
        .map(|file| file.sha256.as_str())
        != Some(collection.collector.scope_catalog_sha256.as_str())
    {
        failures.push(
            "Cypress collector scope catalog digest disagrees with its collector file identity"
                .to_string(),
        );
    }
    if collection.collector.node_version != NODE_VERSION
        || collection.collector.pnpm_version != PNPM_VERSION
        || collection.collector.esbuild_version != ESBUILD_VERSION
    {
        failures.push(format!(
            "Cypress collector toolchain drift: expected Node {NODE_VERSION}, pnpm {PNPM_VERSION}, esbuild {ESBUILD_VERSION}; found Node {}, pnpm {}, esbuild {}",
            collection.collector.node_version,
            collection.collector.pnpm_version,
            collection.collector.esbuild_version
        ));
    }

    let source_specs = collection
        .source
        .specs
        .iter()
        .map(|spec| spec.path.as_str())
        .collect::<BTreeSet<_>>();
    if source_specs.len() != collection.source.specs.len() {
        failures.push("Cypress collection contains duplicate source specs".to_string());
    }
    let supplemental_fixtures = collection
        .source
        .supplemental_fixtures
        .iter()
        .map(|fixture| fixture.path.as_str())
        .collect::<BTreeSet<_>>();
    if supplemental_fixtures.len() != collection.source.supplemental_fixtures.len() {
        failures.push("Cypress collection contains duplicate supplemental fixtures".to_string());
    }
    let mut registration_ids = BTreeSet::new();
    let mut expected_registration_ordinal = BTreeMap::<&str, usize>::new();
    for registration in &collection.registrations {
        if !source_specs.contains(registration.source_spec.as_str()) {
            failures.push(format!(
                "Cypress registration {} references unknown source spec {}",
                registration.id, registration.source_spec
            ));
        }
        let expected = expected_registration_ordinal
            .entry(&registration.source_spec)
            .or_insert(1);
        if registration.ordinal != *expected {
            failures.push(format!(
                "Cypress registration ordinal drift for {}: expected {}, found {}",
                registration.source_spec, *expected, registration.ordinal
            ));
        }
        *expected += 1;
        if registration.id.is_empty()
            || registration.title.is_empty()
            || !registration_ids.insert(registration.id.as_str())
        {
            failures.push(format!(
                "Cypress collection has an empty or duplicate registration identity {:?}",
                registration.id
            ));
        }
    }

    let skipped = collection
        .registrations
        .iter()
        .filter(|registration| registration.skipped)
        .map(|registration| registration.id.as_str())
        .collect::<BTreeSet<_>>();
    if skipped.len() != collection.scope.expected_skipped_registrations {
        failures.push(format!(
            "Cypress collection scope {} expected {} skipped registrations, found {}",
            collection.scope.id,
            collection.scope.expected_skipped_registrations,
            skipped.len()
        ));
    }
    let reviewed_skipped = collection
        .scope
        .reviewed_skipped_registrations
        .iter()
        .map(|entry| entry.registration.as_str())
        .collect::<BTreeSet<_>>();
    if reviewed_skipped.len() != collection.scope.reviewed_skipped_registrations.len()
        || collection
            .scope
            .reviewed_skipped_registrations
            .iter()
            .any(|entry| entry.registration.is_empty() || entry.reason.is_empty())
    {
        failures.push(format!(
            "Cypress collection scope {} has invalid reviewed skip evidence",
            collection.scope.id
        ));
    }
    if skipped != reviewed_skipped {
        failures.push(format!(
            "Cypress collection scope {} skipped registrations disagree with reviewed evidence",
            collection.scope.id
        ));
    }
    let mut reviewed_removals = BTreeSet::new();
    for removal in &collection.scope.reviewed_removals {
        if !portable_relative_path(&removal.source_spec)
            || !source_specs.contains(removal.source_spec.as_str())
            || removal.registration.is_empty()
            || removal.helper_ordinal == 0
            || removal.reason.is_empty()
            || !reviewed_removals.insert((
                removal.source_spec.as_str(),
                removal.registration.as_str(),
                removal.helper_ordinal,
            ))
        {
            failures.push(format!(
                "Cypress collection scope {} has invalid or duplicate reviewed removal evidence",
                collection.scope.id
            ));
        }
    }

    let active_registration_ids = collection
        .registrations
        .iter()
        .filter(|registration| !registration.skipped)
        .map(|registration| registration.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut expected_source_ordinal = BTreeMap::<&str, usize>::new();
    let mut expected_helper_ordinal = BTreeMap::<&str, usize>::new();
    for (expected_global_ordinal, call) in (1usize..).zip(collection.calls.iter()) {
        if call.ordinal != expected_global_ordinal {
            failures.push(format!(
                "Cypress call global ordinal drift: expected {expected_global_ordinal}, found {}",
                call.ordinal
            ));
        }
        let source_ordinal = expected_source_ordinal
            .entry(&call.source_spec)
            .or_insert(1);
        if call.source_ordinal != *source_ordinal {
            failures.push(format!(
                "Cypress call source ordinal drift for {}: expected {}, found {}",
                call.source_spec, *source_ordinal, call.source_ordinal
            ));
        }
        *source_ordinal += 1;
        let helper_ordinal = expected_helper_ordinal
            .entry(&call.registration)
            .or_insert(1);
        if call.helper_ordinal != *helper_ordinal {
            failures.push(format!(
                "Cypress helper ordinal drift for {}: expected {}, found {}",
                call.registration, *helper_ordinal, call.helper_ordinal
            ));
        }
        *helper_ordinal += 1;
        if !source_specs.contains(call.source_spec.as_str()) {
            failures.push(format!(
                "Cypress call {} references unknown source spec {}",
                call.ordinal, call.source_spec
            ));
        }
        if !active_registration_ids.contains(call.registration.as_str()) {
            failures.push(format!(
                "Cypress call {} references missing or skipped registration {}",
                call.ordinal, call.registration
            ));
        }
        if call.diagram.is_empty() {
            failures.push(format!(
                "Cypress call {} has an empty diagram",
                call.ordinal
            ));
        }
        if !call.options.is_object() {
            failures.push(format!(
                "Cypress call {} options must be an object",
                call.ordinal
            ));
        }
        if !is_canonical_sha256(&call.raw_sha256) {
            failures.push(format!(
                "Cypress call {} has a non-canonical raw SHA-256",
                call.ordinal
            ));
        } else {
            let expected = raw_call_sha256(call);
            if call.raw_sha256 != expected {
                failures.push(format!(
                    "Cypress call {} raw identity SHA-256 drift: expected {expected}, found {}",
                    call.ordinal, call.raw_sha256
                ));
            }
        }
    }
    if collection.calls.len() != collection.scope.expected_active_calls {
        failures.push(format!(
            "Cypress collection scope {} expected {} active calls, found {}",
            collection.scope.id,
            collection.scope.expected_active_calls,
            collection.calls.len()
        ));
    }

    for effect in &collection.runtime_effects {
        if !source_specs.contains(effect.source_spec.as_str())
            || !active_registration_ids.contains(effect.registration.as_str())
        {
            failures.push(format!(
                "Cypress runtime effect {} references an unknown source or registration",
                effect.operation
            ));
        }
        if effect.operation.is_empty()
            || effect.selector.is_empty()
            || effect.argument_kinds.is_empty()
            || effect.argument_kinds.iter().any(String::is_empty)
        {
            failures.push("Cypress runtime effect fields must not be empty".to_string());
        }
    }
    failures
}

pub(crate) fn load_raw_cypress_collection(path: &Path) -> Result<RawCypressCollection, XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let collection = serde_json::from_slice::<RawCypressCollection>(&bytes)?;
    let failures = raw_collection_failures(&collection);
    if failures.is_empty() {
        Ok(collection)
    } else {
        Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
    }
}

pub(crate) fn raw_collection_helper(
    call: &RawRenderCall,
) -> Result<crate::cmd::CypressRenderHelper, XtaskError> {
    match call.helper.as_str() {
        "imgSnapshotTest" => Ok(crate::cmd::CypressRenderHelper::ImgSnapshotTest),
        "renderGraph" => Ok(crate::cmd::CypressRenderHelper::RenderGraph),
        helper => Err(XtaskError::AlignmentCheckFailed(format!(
            "unsupported collected Cypress render helper {helper:?} at call {}",
            call.ordinal
        ))),
    }
}

pub(crate) fn collected_registration_title<'a>(
    collection: &'a RawCypressCollection,
    call: &RawRenderCall,
) -> Result<&'a str, XtaskError> {
    collection
        .registrations
        .iter()
        .find(|registration| {
            registration.source_spec == call.source_spec
                && registration.id == call.registration
                && !registration.skipped
        })
        .map(|registration| registration.title.as_str())
        .ok_or_else(|| {
            XtaskError::AlignmentCheckFailed(format!(
                "collected Cypress call {} references missing or skipped registration {:?}",
                call.ordinal, call.registration
            ))
        })
}

pub(crate) fn collection_evidence(collection: &RawCypressCollection) -> CypressCollectionEvidence {
    CypressCollectionEvidence {
        scope_id: collection.scope.id.clone(),
        description: collection.scope.description.clone(),
        expected_active_calls: collection.scope.expected_active_calls,
        expected_skipped_registrations: collection.scope.expected_skipped_registrations,
        reviewed_skipped_registrations: collection.scope.reviewed_skipped_registrations.clone(),
        reviewed_removals: collection.scope.reviewed_removals.clone(),
        source: CypressCollectionSourceEvidence {
            package: collection.source.package.clone(),
            version: collection.source.version.clone(),
            tag: collection.source.tag.clone(),
            commit: collection.source.commit.clone(),
            test_config: TestConfigEvidence {
                path: collection.source.test_config.path.clone(),
                sha256: collection.source.test_config.sha256.clone(),
                spec_pattern: collection.source.test_config.spec_pattern.clone(),
            },
            render_helper: collection.source.render_helper.clone(),
            specs: collection.source.specs.clone(),
            supplemental_fixtures: collection.source.supplemental_fixtures.clone(),
        },
        collector: CypressCollectorEvidence {
            files: collection.collector.files.clone(),
            scope_catalog_sha256: collection.collector.scope_catalog_sha256.clone(),
            node_version: collection.collector.node_version.clone(),
            pnpm_version: collection.collector.pnpm_version.clone(),
            esbuild_version: collection.collector.esbuild_version.clone(),
            upstream_lock: collection.collector.upstream_lock.clone(),
        },
        registrations: collection
            .registrations
            .iter()
            .filter(|registration| registration.skipped)
            .map(|registration| RegistrationEvidence {
                source_spec: registration.source_spec.clone(),
                ordinal: registration.ordinal,
                id: registration.id.clone(),
                title: registration.title.clone(),
                skipped: true,
            })
            .collect(),
        runtime_effects: collection
            .runtime_effects
            .iter()
            .map(|effect| RuntimeEffectEvidence {
                source_spec: effect.source_spec.clone(),
                registration: effect.registration.clone(),
                operation: effect.operation.clone(),
                selector: effect.selector.clone(),
                argument_kinds: effect.argument_kinds.clone(),
            })
            .collect(),
    }
}

pub(crate) fn committed_collection_evidence_failures(
    workspace_root: &Path,
    evidence: &CypressCollectionEvidence,
    expected_scope: &str,
    expected_description: &str,
) -> Vec<String> {
    let mut failures = Vec::new();
    if evidence.scope_id != expected_scope || evidence.description != expected_description {
        failures.push(format!(
            "Cypress collection manifest scope drift: expected {expected_scope:?} / {expected_description:?}, found {:?} / {:?}",
            evidence.scope_id, evidence.description
        ));
    }
    if evidence.source.package != "mermaid"
        || evidence.source.version != PINNED_MERMAID_VERSION
        || evidence.source.tag != MERMAID_SOURCE_TAG
        || evidence.source.commit != MERMAID_SOURCE_COMMIT
    {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} disagrees with the selected Mermaid graph"
        ));
    }
    if evidence.expected_active_calls == 0 {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} must expect at least one active call"
        ));
    }
    validate_digest_path(
        &DigestPath {
            path: evidence.source.test_config.path.clone(),
            sha256: evidence.source.test_config.sha256.clone(),
        },
        "Cypress test config",
        &mut failures,
    );
    if evidence.source.test_config.path != TEST_CONFIG_PATH
        || evidence.source.test_config.spec_pattern != TEST_SPEC_PATTERN
    {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} has unsupported test config identity"
        ));
    }
    validate_digest_path(
        &evidence.source.render_helper,
        "Cypress render helper",
        &mut failures,
    );
    if evidence.source.render_helper.path != RENDER_HELPER_PATH {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} has unsupported render helper path"
        ));
    }
    if evidence.source.specs.is_empty() {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} has no source specs"
        ));
    }
    let mut source_specs = BTreeSet::new();
    for spec in &evidence.source.specs {
        validate_digest_path(spec, "Cypress source spec", &mut failures);
        if !source_specs.insert(spec.path.as_str()) {
            failures.push(format!(
                "Cypress collection manifest {expected_scope} has duplicate source spec {}",
                spec.path
            ));
        }
    }
    let mut supplemental_fixtures = BTreeSet::new();
    for fixture in &evidence.source.supplemental_fixtures {
        validate_digest_path(fixture, "Cypress supplemental fixture", &mut failures);
        if !supplemental_fixtures.insert(fixture.path.as_str()) {
            failures.push(format!(
                "Cypress collection manifest {expected_scope} has duplicate supplemental fixture {}",
                fixture.path
            ));
        }
        let path = workspace_root.join(&fixture.path);
        match fs::read(&path) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if actual != fixture.sha256 {
                    failures.push(format!(
                        "Cypress supplemental fixture SHA-256 drift for {}: expected {}, found {actual}",
                        fixture.path, fixture.sha256
                    ));
                }
            }
            Err(error) => failures.push(format!(
                "failed to read Cypress supplemental fixture {}: {error}",
                path.display()
            )),
        }
    }
    let mut collector_files = BTreeSet::new();
    for file in &evidence.collector.files {
        validate_digest_path(file, "Cypress collector file", &mut failures);
        if !collector_files.insert(file.path.as_str()) {
            failures.push(format!(
                "Cypress collection manifest {expected_scope} has duplicate collector file {}",
                file.path
            ));
        }
        let path = workspace_root.join(&file.path);
        match fs::read(&path) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if actual != file.sha256 {
                    failures.push(format!(
                        "Cypress collector SHA-256 drift for {}: expected {}, found {actual}",
                        file.path, file.sha256
                    ));
                }
            }
            Err(error) => failures.push(format!(
                "failed to read Cypress collector file {}: {error}",
                path.display()
            )),
        }
    }
    if !has_exact_collector_file_paths(&evidence.collector.files) {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} has an incomplete collector file identity set"
        ));
    }
    if !is_canonical_sha256(&evidence.collector.scope_catalog_sha256) {
        failures.push("Cypress collector scope catalog has a non-canonical SHA-256".to_string());
    }
    if evidence
        .collector
        .files
        .iter()
        .find(|file| file.path == "tools/upstreams/cypress-collector/scopes.json")
        .map(|file| file.sha256.as_str())
        != Some(evidence.collector.scope_catalog_sha256.as_str())
    {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} has inconsistent scope-catalog evidence"
        ));
    }
    if evidence.collector.node_version != NODE_VERSION
        || evidence.collector.pnpm_version != PNPM_VERSION
        || evidence.collector.esbuild_version != ESBUILD_VERSION
    {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} has unsupported collector toolchain evidence"
        ));
    }
    validate_digest_path(
        &evidence.collector.upstream_lock,
        "Cypress collector upstream lock",
        &mut failures,
    );
    if evidence.collector.upstream_lock.path != UPSTREAM_LOCK_PATH {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} has unsupported upstream lock path"
        ));
    }

    let mut registration_ids = BTreeSet::new();
    let mut skipped_registrations = BTreeSet::new();
    for registration in &evidence.registrations {
        if !source_specs.contains(registration.source_spec.as_str()) {
            failures.push(format!(
                "Cypress collection manifest {expected_scope} skipped registration {} references unknown source {}",
                registration.id, registration.source_spec
            ));
        }
        if registration.id.is_empty()
            || registration.title.is_empty()
            || registration.ordinal == 0
            || !registration_ids.insert(registration.id.as_str())
        {
            failures.push(format!(
                "Cypress collection manifest has an invalid registration identity {:?}",
                registration.id
            ));
        }
        if registration.skipped && !skipped_registrations.insert(registration.id.as_str()) {
            failures.push(format!(
                "Cypress collection manifest has a duplicate skipped registration identity {:?}",
                registration.id
            ));
        }
    }
    let reviewed = evidence
        .reviewed_skipped_registrations
        .iter()
        .map(|entry| entry.registration.as_str())
        .collect::<BTreeSet<_>>();
    if reviewed.len() != evidence.reviewed_skipped_registrations.len()
        || evidence
            .reviewed_skipped_registrations
            .iter()
            .any(|entry| entry.registration.is_empty() || entry.reason.is_empty())
    {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} has invalid reviewed skip evidence"
        ));
    }
    if skipped_registrations.len() != evidence.expected_skipped_registrations
        || skipped_registrations != reviewed
    {
        failures.push(format!(
            "Cypress collection manifest {expected_scope} skipped registrations disagree with reviewed evidence"
        ));
    }
    let mut reviewed_removals = BTreeSet::new();
    for removal in &evidence.reviewed_removals {
        if !portable_relative_path(&removal.source_spec)
            || !source_specs.contains(removal.source_spec.as_str())
            || removal.registration.is_empty()
            || removal.helper_ordinal == 0
            || removal.reason.is_empty()
            || !reviewed_removals.insert((
                removal.source_spec.as_str(),
                removal.registration.as_str(),
                removal.helper_ordinal,
            ))
        {
            failures.push(format!(
                "Cypress collection manifest {expected_scope} has invalid or duplicate reviewed removal evidence"
            ));
        }
    }
    for effect in &evidence.runtime_effects {
        if !source_specs.contains(effect.source_spec.as_str())
            || effect.registration.is_empty()
            || effect.operation.is_empty()
            || effect.selector.is_empty()
            || effect.argument_kinds.is_empty()
            || effect.argument_kinds.iter().any(String::is_empty)
        {
            failures.push(format!(
                "Cypress collection manifest {expected_scope} has invalid runtime-effect evidence"
            ));
        }
    }
    failures
}

pub(crate) fn project_upstream_cypress_collection(args: Vec<String>) -> Result<(), XtaskError> {
    let mut scope = None;
    let mut input = None;
    let mut refresh = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--scope" => {
                index += 1;
                scope = args.get(index).cloned();
            }
            "--input" => {
                index += 1;
                input = args.get(index).map(std::path::PathBuf::from);
            }
            "--refresh" => refresh = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }
    let scope = scope.ok_or(XtaskError::Usage)?;
    let input = input.ok_or(XtaskError::Usage)?;
    let collection = load_raw_cypress_collection(&input)?;
    if collection.scope.id != scope {
        return Err(XtaskError::AlignmentCheckFailed(format!(
            "collection scope mismatch: --scope requested {scope:?}, input contains {:?}",
            collection.scope.id
        )));
    }
    match scope.as_str() {
        "new-family" => crate::cmd::project_new_family_cypress_collection(&collection, refresh),
        "flowchart-elk" => crate::cmd::project_flowchart_elk_collection(&collection, refresh),
        _ => Err(XtaskError::Usage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_paths_reject_absolute_parent_and_platform_forms() {
        for path in [
            "",
            "../escape",
            "/tmp/escape",
            "C:/escape",
            "a\\b",
            "a/../b",
        ] {
            assert!(!portable_relative_path(path), "{path:?} must be rejected");
        }
        assert!(portable_relative_path(
            "cypress/integration/example.spec.ts"
        ));
    }

    fn render_call(options: serde_json::Value) -> RawRenderCall {
        RawRenderCall {
            source_spec: "cypress/integration/example.spec.ts".to_string(),
            source_ordinal: 1,
            ordinal: 1,
            registration: "suite > case".to_string(),
            helper_ordinal: 1,
            helper: "imgSnapshotTest".to_string(),
            diagram: "flowchart LR\nA-->B".to_string(),
            options,
            api: false,
            validation: ValidationArgument::Absent,
            raw_sha256: String::new(),
        }
    }

    #[test]
    fn raw_call_digest_is_order_stable_and_identity_sensitive() {
        let left_options = serde_json::from_str(r#"{"z":1,"a":{"y":2,"b":3}}"#).unwrap();
        let right_options = serde_json::from_str(r#"{"a":{"b":3,"y":2},"z":1}"#).unwrap();
        let left = render_call(left_options);
        let mut right = render_call(right_options);

        assert_eq!(raw_call_sha256(&left), raw_call_sha256(&right));
        right.validation = ValidationArgument::Present;
        assert_ne!(raw_call_sha256(&left), raw_call_sha256(&right));
    }

    #[test]
    fn collector_identity_requires_every_executable_boundary_file() {
        let digest = |path: &str| DigestPath {
            path: path.to_string(),
            sha256: "a".repeat(64),
        };
        let exact = COLLECTOR_FILE_PATHS.map(digest);
        assert!(has_exact_collector_file_paths(&exact));

        let missing_worker = &exact[..2];
        assert!(!has_exact_collector_file_paths(missing_worker));

        let mut extra = exact.to_vec();
        extra.push(digest("tools/upstreams/cypress-collector/unreviewed.mjs"));
        assert!(!has_exact_collector_file_paths(&extra));
    }
}
