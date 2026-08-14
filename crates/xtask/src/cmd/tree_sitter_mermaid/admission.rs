use super::SupportMetadata;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_fixture_render_context::RenderContextCatalog;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path};
use tree_sitter::{Language, Parser};
use tree_sitter_mermaid::LANGUAGE;

const MANIFEST_PATH: &str = "distribution/tree-sitter-mermaid/test/conformance/admission.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AdmissionManifest {
    schema_version: u32,
    artifact_receipt_id: String,
    families: Vec<FamilyAdmission>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilyAdmission {
    public_id: String,
    expected_diagram_type: String,
    root_node: String,
    required_named_nodes: Vec<String>,
    fixtures: Vec<FixtureAdmission>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureAdmission {
    role: String,
    path: String,
    source_sha256: String,
    render_context_sha256: String,
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut rendered = String::with_capacity(64);
    for byte in digest {
        write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
    }
    rendered
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn structured_support(support: &SupportMetadata) -> BTreeMap<&str, &str> {
    support
        .families
        .iter()
        .filter(|family| {
            matches!(
                family.support_tier.as_deref(),
                Some("structured" | "query-complete" | "conformant")
            )
        })
        .map(|family| (family.public_id.as_str(), family.root_node.as_str()))
        .collect()
}

fn normalized_fixture_path(path: &str, public_id: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path.extension().is_some_and(|extension| extension == "mmd")
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && path.starts_with(Path::new("fixtures").join(public_id))
        && !path.components().any(|component| {
            matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with('_'))
        })
}

fn named_node_types(node: tree_sitter::Node<'_>, types: &mut BTreeSet<String>) {
    if node.is_named() {
        types.insert(node.kind().to_owned());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        named_node_types(child, types);
    }
}

fn is_recovery_node(kind: &str) -> bool {
    kind.contains("unstructured")
        || kind.contains("_recovery")
        || kind.contains("_malformed")
        || kind.contains("_unclosed")
        || kind.contains("_incomplete")
        || matches!(kind, "raw_line" | "unknown_statement" | "catch_all_body")
}

fn validate_fixture(
    root: &Path,
    catalog: &RenderContextCatalog,
    parser: &mut Parser,
    family: &FamilyAdmission,
    fixture: &FixtureAdmission,
) -> Result<BTreeSet<String>, String> {
    if !matches!(fixture.role.as_str(), "family-baseline" | "admitted-valid") {
        return Err(format!(
            "family {} fixture {} has unknown role {:?}",
            family.public_id, fixture.path, fixture.role
        ));
    }
    if !normalized_fixture_path(&fixture.path, &family.public_id) {
        return Err(format!(
            "family {} fixture path {:?} is not an admitted family-local .mmd path",
            family.public_id, fixture.path
        ));
    }
    if !valid_digest(&fixture.source_sha256) || !valid_digest(&fixture.render_context_sha256) {
        return Err(format!(
            "family {} fixture {} has an invalid digest",
            family.public_id, fixture.path
        ));
    }

    let fixtures_root = root.join("fixtures");
    let path = root.join(&fixture.path);
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("failed to resolve {}: {error}", path.display()))?;
    let canonical_fixtures = fixtures_root
        .canonicalize()
        .map_err(|error| format!("failed to resolve {}: {error}", fixtures_root.display()))?;
    if !resolved.starts_with(&canonical_fixtures) || !resolved.is_file() {
        return Err(format!(
            "family {} fixture {} escapes the fixtures root",
            family.public_id, fixture.path
        ));
    }

    let source = fs::read(&resolved)
        .map_err(|error| format!("failed to read {}: {error}", resolved.display()))?;
    if sha256(&source) != fixture.source_sha256 {
        return Err(format!(
            "family {} fixture {} source digest drifted",
            family.public_id, fixture.path
        ));
    }
    let source = std::str::from_utf8(&source)
        .map_err(|error| format!("fixture {} is not UTF-8: {error}", fixture.path))?;

    let context = catalog
        .context_for_fixture(&resolved)
        .map_err(|error| format!("fixture {} render context failed: {error}", fixture.path))?;
    let context_value = context
        .map(|context| context.site_config_value())
        .unwrap_or(Value::Null);
    let context_bytes = serde_json::to_vec(&context_value).map_err(|error| {
        format!(
            "fixture {} context serialization failed: {error}",
            fixture.path
        )
    })?;
    if sha256(&context_bytes) != fixture.render_context_sha256 {
        return Err(format!(
            "family {} fixture {} render-context digest drifted",
            family.public_id, fixture.path
        ));
    }

    let engine = match context {
        Some(context) => {
            Engine::new().with_site_config(MermaidConfig::from_value(context.site_config_value()))
        }
        None => Engine::new(),
    };
    let metadata = engine
        .parse_metadata_sync(source)
        .map_err(|error| format!("fixture {} detection failed: {error}", fixture.path))?;
    if metadata.diagram_type != family.expected_diagram_type {
        return Err(format!(
            "fixture {} detected {:?}; expected {:?}",
            fixture.path, metadata.diagram_type, family.expected_diagram_type
        ));
    }
    if engine
        .parse_diagram_sync(source, ParseOptions::strict())
        .map_err(|error| format!("fixture {} strict parse failed: {error}", fixture.path))?
        .is_none()
    {
        return Err(format!(
            "fixture {} strict parse returned no diagram",
            fixture.path
        ));
    }

    let tree = parser
        .parse(source.as_bytes(), None)
        .ok_or_else(|| format!("fixture {} Tree-sitter parse was cancelled", fixture.path))?;
    if tree.root_node().has_error() {
        return Err(format!(
            "fixture {} has an unexpected Tree-sitter error: {}",
            fixture.path,
            tree.root_node().to_sexp()
        ));
    }
    let mut cursor = tree.root_node().walk();
    let roots = tree
        .root_node()
        .named_children(&mut cursor)
        .filter(|node| node.kind().ends_with("_diagram"))
        .map(|node| node.kind().to_owned())
        .collect::<Vec<_>>();
    if roots != [family.root_node.as_str()] {
        return Err(format!(
            "fixture {} selected roots {roots:?}; expected {}",
            fixture.path, family.root_node
        ));
    }
    let mut nodes = BTreeSet::new();
    named_node_types(tree.root_node(), &mut nodes);
    if let Some(generic) = nodes.iter().find(|kind| is_recovery_node(kind)) {
        return Err(format!(
            "fixture {} contains recovery or generic node {generic}",
            fixture.path
        ));
    }
    Ok(nodes)
}

pub(super) fn validate(
    root: &Path,
    artifact_receipt_id: &str,
    support: &SupportMetadata,
) -> Result<(), String> {
    let expected = structured_support(support);
    let manifest_path = root.join(MANIFEST_PATH);
    if expected.is_empty() {
        if manifest_path.exists() {
            return Err(format!(
                "{MANIFEST_PATH} exists before any family reaches structured"
            ));
        }
        return Ok(());
    }
    let bytes = fs::read(&manifest_path)
        .map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?;
    let manifest: AdmissionManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {MANIFEST_PATH}: {error}"))?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported family admission schema {}",
            manifest.schema_version
        ));
    }
    if manifest.artifact_receipt_id != artifact_receipt_id {
        return Err(
            "family admission manifest is stale relative to generated artifacts".to_string(),
        );
    }

    let actual = manifest
        .families
        .iter()
        .map(|family| (family.public_id.as_str(), family.root_node.as_str()))
        .collect::<BTreeMap<_, _>>();
    if actual != expected {
        return Err(format!(
            "family admission rows differ from structured support; expected={expected:?}, actual={actual:?}"
        ));
    }

    let catalog = RenderContextCatalog::load(root.join("fixtures"))
        .map_err(|error| format!("failed to load fixture render contexts: {error}"))?;
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|error| format!("failed to load Tree-sitter Mermaid: {error}"))?;
    let mut seen_paths = BTreeSet::new();
    for family in &manifest.families {
        if family.expected_diagram_type.trim().is_empty()
            || family.required_named_nodes.is_empty()
            || family
                .required_named_nodes
                .iter()
                .any(|node| node.trim().is_empty())
        {
            return Err(format!(
                "family {} admission identity is incomplete",
                family.public_id
            ));
        }
        let roles = family
            .fixtures
            .iter()
            .map(|fixture| fixture.role.as_str())
            .collect::<BTreeSet<_>>();
        if !roles.contains("family-baseline") || !roles.contains("admitted-valid") {
            return Err(format!(
                "family {} requires baseline and admitted-valid fixtures",
                family.public_id
            ));
        }
        let mut observed_nodes = BTreeSet::new();
        for fixture in &family.fixtures {
            if !seen_paths.insert(fixture.path.as_str()) {
                return Err(format!("duplicate admitted fixture {}", fixture.path));
            }
            observed_nodes.extend(validate_fixture(
                root,
                &catalog,
                &mut parser,
                family,
                fixture,
            )?);
        }
        for required in &family.required_named_nodes {
            if !observed_nodes.contains(required) {
                return Err(format!(
                    "family {} admitted slice lacks required named node {required}",
                    family.public_id
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_recovery_node;

    #[test]
    fn strict_valid_fixtures_reject_recovery_node_conventions() {
        for kind in [
            "unstructured_body",
            "packet_recovery_statement",
            "radar_malformed_statement",
            "git_graph_incomplete_merge_statement",
            "architecture_unclosed_quoted_string",
            "raw_line",
            "unknown_statement",
            "catch_all_body",
        ] {
            assert!(is_recovery_node(kind), "{kind}");
        }
        assert!(!is_recovery_node("architecture_service_statement"));
    }
}
