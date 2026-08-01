use merman_fixture_render_context::{
    DiagramDomEvidence, FixtureDomEvidence, FixtureRenderContext, MANIFEST_RELATIVE_PATH,
    Provenance, RenderContextCatalog, SecurityLevel, diagram_dom_evidence, fixture_dom_evidence,
    parser_only_fixture_reason,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "merman-render-context-contract-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create fixture test root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn fixture(&self, relative: &str, source: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
        fs::write(path, source).expect("write fixture");
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sha256(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

fn manifest_entry(
    fixture: &str,
    source: &str,
    provenance: serde_json::Value,
    security_level: &str,
) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
        "contexts": [{
            "fixture": fixture,
            "fixtureSha256": sha256(source),
            "provenance": provenance,
            "siteConfig": { "securityLevel": security_level }
        }]
    })
}

#[test]
fn derives_only_declared_non_default_host_security_inputs() {
    let frontmatter = "---\nconfig:\n  securityLevel: loose\n---\nflowchart TD\nA-->B\n";
    let context = FixtureRenderContext::derive("flowchart/frontmatter.mmd", frontmatter.as_bytes())
        .expect("derive frontmatter context")
        .expect("non-default host input");
    assert_eq!(context.security_level(), SecurityLevel::Loose);
    assert_eq!(
        context.provenance(),
        &Provenance::Frontmatter {
            config_path: vec!["config".to_string(), "securityLevel".to_string()]
        }
    );

    let directive = "%%{initialize: {'securityLevel':'sandbox'}}%%\nclassDiagram\nclass A\n";
    let context = FixtureRenderContext::derive("class/directive.mmd", directive.as_bytes())
        .expect("derive directive context")
        .expect("non-default host input");
    assert_eq!(context.security_level(), SecurityLevel::Sandbox);
    assert_eq!(
        context.provenance(),
        &Provenance::Directive {
            directive: "initialize".to_string(),
            occurrence: 0,
            config_path: vec!["securityLevel".to_string()]
        }
    );

    let strict = "---\nconfig:\n  securityLevel: strict\n---\nflowchart TD\nA-->B\n";
    assert!(
        FixtureRenderContext::derive("flowchart/strict.mmd", strict.as_bytes())
            .expect("strict is the default, not a host context")
            .is_none()
    );
    let diagram_config = "---\nsecurityLevel: loose\n---\nflowchart TD\nA-->B\n";
    assert!(
        FixtureRenderContext::derive("flowchart/diagram-config.mmd", diagram_config.as_bytes())
            .expect("root diagram config is not a corpus host-input projection")
            .is_none()
    );
}

#[test]
fn rejects_ambiguous_host_security_provenance() {
    let conflicting = "%%{init: {\"securityLevel\":\"loose\"}}%%\n\
flowchart TD\n\
A-->B\n\
%%{initialize: {\"securityLevel\":\"sandbox\"}}%%\n";
    let error = FixtureRenderContext::derive("flowchart/conflicting.mmd", conflicting.as_bytes())
        .expect_err("multiple host security declarations must be rejected");
    assert!(
        error
            .to_string()
            .contains("multiple securityLevel declarations")
    );

    let duplicated = "---\nconfig:\n  securityLevel: loose\n---\n\
%%{init: {\"securityLevel\":\"loose\"}}%%\nflowchart TD\nA-->B\n";
    FixtureRenderContext::derive("flowchart/duplicated.mmd", duplicated.as_bytes())
        .expect_err("even equal declarations have ambiguous provenance");
}

#[test]
fn loads_and_resolves_a_hash_bound_structured_context() {
    let root = TestRoot::new();
    let relative = "flowchart/example.mmd";
    let source = "---\nconfig:\n  securityLevel: loose\n---\nflowchart TD\nA-->B\n";
    root.fixture(relative, source);
    let manifest = manifest_entry(
        relative,
        source,
        serde_json::json!({
            "kind": "frontmatter",
            "configPath": ["config", "securityLevel"]
        }),
        "loose",
    );

    let catalog = RenderContextCatalog::from_json(
        root.path(),
        &serde_json::to_string(&manifest).expect("serialize manifest"),
    )
    .expect("load valid catalog");
    let context = catalog
        .context_for_fixture(root.path().join(relative))
        .expect("resolve fixture path")
        .expect("fixture context");
    assert_eq!(context.fixture(), relative);
    assert_eq!(context.site_config_value()["securityLevel"], "loose");
}

#[test]
fn manifest_validation_is_fail_closed() {
    let root = TestRoot::new();
    let relative = "flowchart/example.mmd";
    let source = "%%{init: {\"securityLevel\":\"loose\"}}%%\nflowchart TD\nA-->B\n";
    root.fixture(relative, source);
    let valid = manifest_entry(
        relative,
        source,
        serde_json::json!({
            "kind": "directive",
            "directive": "init",
            "occurrence": 0,
            "configPath": ["securityLevel"]
        }),
        "loose",
    );

    let mut cases = Vec::new();
    let mut unknown_schema = valid.clone();
    unknown_schema["schemaVersion"] = serde_json::json!(2);
    cases.push(("unknown schema", unknown_schema));

    let mut unknown_top_field = valid.clone();
    unknown_top_field["unexpected"] = serde_json::json!(true);
    cases.push(("unknown top-level field", unknown_top_field));

    let mut absolute_path = valid.clone();
    absolute_path["contexts"][0]["fixture"] = serde_json::json!("/tmp/example.mmd");
    cases.push(("absolute fixture path", absolute_path));

    let mut escaping_path = valid.clone();
    escaping_path["contexts"][0]["fixture"] = serde_json::json!("../example.mmd");
    cases.push(("escaping fixture path", escaping_path));

    let mut missing_fixture = valid.clone();
    missing_fixture["contexts"][0]["fixture"] = serde_json::json!("flowchart/missing.mmd");
    cases.push(("missing fixture", missing_fixture));

    let mut hash_drift = valid.clone();
    hash_drift["contexts"][0]["fixtureSha256"] = serde_json::json!("0".repeat(64));
    cases.push(("fixture hash drift", hash_drift));

    let mut source_mismatch = valid.clone();
    source_mismatch["contexts"][0]["siteConfig"]["securityLevel"] = serde_json::json!("sandbox");
    cases.push(("source value mismatch", source_mismatch));

    let mut unknown_host_field = valid.clone();
    unknown_host_field["contexts"][0]["siteConfig"]["theme"] = serde_json::json!("dark");
    cases.push(("unknown host field", unknown_host_field));

    let mut strict = valid.clone();
    strict["contexts"][0]["siteConfig"]["securityLevel"] = serde_json::json!("strict");
    cases.push(("strict default", strict));

    let mut arbitrary = valid.clone();
    arbitrary["contexts"][0]["siteConfig"]["securityLevel"] = serde_json::json!("trusted");
    cases.push(("arbitrary security level", arbitrary));

    for (name, manifest) in cases {
        let error = RenderContextCatalog::from_json(
            root.path(),
            &serde_json::to_string(&manifest).expect("serialize invalid manifest"),
        )
        .expect_err(name);
        assert!(
            !error.to_string().is_empty(),
            "{name} should explain rejection"
        );
    }
}

#[test]
fn update_round_trip_upserts_and_removes_contexts() {
    let root = TestRoot::new();
    let relative = "flowchart/example.mmd";
    let loose = "%%{init: {\"securityLevel\":\"loose\"}}%%\nflowchart TD\nA-->B\n";
    root.fixture(relative, loose);

    let mut catalog = RenderContextCatalog::load_for_update(root.path())
        .expect("missing manifest starts an empty update catalog");
    assert!(
        catalog
            .upsert_from_source(relative, loose.as_bytes())
            .expect("insert context")
    );
    assert!(
        !catalog
            .upsert_from_source(relative, loose.as_bytes())
            .expect("unchanged context")
    );
    let rendered = catalog.to_json().expect("render canonical manifest");
    assert!(rendered.contains("\"schemaVersion\": 1"));
    assert!(rendered.contains("\"siteConfig\""));
    assert!(!rendered.to_ascii_lowercase().contains("override"));

    let strict = "%%{init: {\"securityLevel\":\"strict\"}}%%\nflowchart TD\nA-->B\n";
    root.fixture(relative, strict);
    assert!(
        catalog
            .upsert_from_source(relative, strict.as_bytes())
            .expect("default host input removes context")
    );
    assert_eq!(catalog.contexts().count(), 0);
    assert!(!catalog.remove(relative).expect("already absent"));
    assert_eq!(
        catalog.fixtures_root(),
        fs::canonicalize(root.path())
            .expect("canonical fixture root")
            .as_path()
    );
    assert_eq!(MANIFEST_RELATIVE_PATH, "_config/render_contexts.json");
}

#[test]
fn fixture_update_validates_siblings_but_allows_the_candidate_to_change() {
    let root = TestRoot::new();
    let candidate = "flowchart/candidate.mmd";
    let sibling = "flowchart/sibling.mmd";
    let loose = "%%{init: {\"securityLevel\":\"loose\"}}%%\nflowchart TD\nA-->B\n";
    root.fixture(candidate, loose);
    root.fixture(sibling, loose);
    let mut catalog = RenderContextCatalog::rebuild(root.path()).expect("empty catalog");
    catalog
        .upsert_from_source(candidate, loose.as_bytes())
        .expect("insert candidate");
    catalog
        .upsert_from_source(sibling, loose.as_bytes())
        .expect("insert sibling");
    let manifest_path = root.path().join(MANIFEST_RELATIVE_PATH);
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create manifest parent");
    fs::write(&manifest_path, catalog.to_json().expect("render manifest")).expect("write manifest");

    let sandbox = "%%{init: {\"securityLevel\":\"sandbox\"}}%%\nflowchart TD\nA-->B\n";
    root.fixture(candidate, sandbox);
    RenderContextCatalog::load(root.path()).expect_err("strict load catches candidate hash drift");
    let mut update = RenderContextCatalog::load_for_fixture_update(root.path(), candidate)
        .expect("candidate update tolerates only candidate drift");
    update
        .upsert_from_source(candidate, sandbox.as_bytes())
        .expect("update candidate context");
    assert_eq!(
        update
            .context_for_fixture(root.path().join(candidate))
            .expect("candidate lookup")
            .expect("candidate context")
            .security_level(),
        SecurityLevel::Sandbox
    );

    root.fixture(sibling, sandbox);
    RenderContextCatalog::load_for_fixture_update(root.path(), candidate)
        .expect_err("an unrelated context must still fail closed");
}

#[test]
fn parser_only_capabilities_are_exact_family_scoped_facts() {
    for (diagram, fixture) in [
        (
            "flowchart",
            "upstream_flow_text_ellipse_vertex_parser_only_spec.mmd",
        ),
        ("sankey", "upstream_sankey_allows_proto_id_parser_only_spec"),
        (
            "sankey",
            "upstream_sankey_allows_proto_id_sankey_header_parser_only_spec",
        ),
        (
            "xychart",
            "upstream_xychart_header_only_jison_spec_parser_only_",
        ),
        (
            "xychart",
            "upstream_xychart_title_variants_jison_spec_parser_only_",
        ),
    ] {
        assert!(
            parser_only_fixture_reason(diagram, fixture).is_some(),
            "missing exact fact for {diagram}/{fixture}"
        );
    }

    assert_eq!(
        parser_only_fixture_reason("flowchart", "new_parser_only_spec.mmd"),
        None
    );
    assert_eq!(
        parser_only_fixture_reason(
            "sequence",
            "upstream_flow_text_ellipse_vertex_parser_only_spec.mmd"
        ),
        None
    );
}

#[test]
fn rough_dom_evidence_is_exact_family_scoped_policy() {
    for (diagram, fixture) in [
        (
            "ishikawa",
            "upstream_cypress_ishikawa_spec_6_should_render_with_handdrawn_look_006.mmd",
        ),
        ("venn", "upstream_cypress_venn_handdrawn_two_set_014"),
        (
            "venn",
            "upstream_cypress_venn_handdrawn_three_set_title_015",
        ),
        ("venn", "upstream_cypress_venn_handdrawn_custom_styles_018"),
    ] {
        let evidence = fixture_dom_evidence(diagram, fixture)
            .unwrap_or_else(|| panic!("missing exact DOM evidence policy for {diagram}/{fixture}"));
        assert_eq!(evidence, FixtureDomEvidence::StructureOnly);
        assert!(evidence.reason().contains("RoughJS"));
    }

    assert_eq!(
        fixture_dom_evidence("ishikawa", "new_handdrawn_fixture.mmd"),
        None
    );
    assert_eq!(
        fixture_dom_evidence(
            "venn",
            "upstream_cypress_ishikawa_spec_6_should_render_with_handdrawn_look_006"
        ),
        None
    );
}

#[test]
fn browser_text_wrapping_evidence_is_exact_family_scoped_policy() {
    let evidence = fixture_dom_evidence("class", "stress_class_svg_font_size_precedence_025.mmd")
        .expect("Class font-size boundary fixture should declare its browser text evidence");
    assert_eq!(evidence, FixtureDomEvidence::BrowserTextWrapping);
    assert!(evidence.reason().contains("font measurement"));

    assert_eq!(
        fixture_dom_evidence(
            "class",
            "stress_class_svg_font_size_px_string_precedence_026"
        ),
        None
    );
    assert_eq!(
        fixture_dom_evidence("flowchart", "stress_class_svg_font_size_precedence_025"),
        None
    );
}

#[test]
fn browser_measured_text_length_evidence_is_family_scoped_policy() {
    let evidence = diagram_dom_evidence("c4")
        .expect("C4 should declare its browser-measured textLength evidence");
    assert_eq!(evidence, DiagramDomEvidence::BrowserMeasuredTextLength);
    assert!(evidence.reason().contains("browser text measurement"));

    assert_eq!(diagram_dom_evidence("class"), None);
    assert_eq!(diagram_dom_evidence("flowchart"), None);
}
