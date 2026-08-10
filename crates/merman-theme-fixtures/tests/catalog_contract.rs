use merman_theme_fixtures::{
    CatalogError, ExpectedOutputTarget, ExpectedThemeCapability, ReferenceMechanismDisposition,
    ReferenceThemeMechanism, ThemeFixtureCatalog,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn themes_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes")
}

fn committed_manifest() -> Value {
    let root = themes_root();
    let json = fs::read_to_string(root.join("manifest.json")).expect("read committed manifest");
    serde_json::from_str(&json).expect("parse committed manifest")
}

fn rejection(manifest: Value) -> CatalogError {
    rejection_at(&themes_root(), manifest)
}

fn rejection_at(root: &Path, manifest: Value) -> CatalogError {
    ThemeFixtureCatalog::from_json(
        root,
        &serde_json::to_string(&manifest).expect("serialize mutation"),
    )
    .expect_err("mutated catalog must fail closed")
}

fn copied_theme_root() -> TempDir {
    let temp = tempfile::tempdir().expect("create temporary theme root");
    copy_tree(&themes_root(), temp.path());
    temp
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create fixture directory");
    for entry in fs::read_dir(source).expect("read fixture directory") {
        let entry = entry.expect("read fixture entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("read fixture type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy fixture file");
        }
    }
}

fn write_json(root: &Path, relative: &str, value: &Value) -> String {
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize fixture JSON");
    bytes.push(b'\n');
    fs::write(root.join(relative), &bytes).expect("write fixture JSON");
    format!("{:x}", Sha256::digest(bytes))
}

fn write_text(root: &Path, relative: &str, value: &str) -> String {
    fs::write(root.join(relative), value).expect("write fixture text");
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn theme_mut<'a>(manifest: &'a mut Value, name: &str) -> &'a mut Value {
    manifest["themes"]
        .as_array_mut()
        .expect("theme array")
        .iter_mut()
        .find(|theme| theme["referenceName"] == name)
        .expect("named theme")
}

fn translation_mut<'a>(manifest: &'a mut Value, source: &str) -> &'a mut Value {
    manifest["mechanismTranslations"]
        .as_array_mut()
        .expect("translation array")
        .iter_mut()
        .find(|translation| translation["source"] == source)
        .expect("named translation")
}

fn source_mut<'a>(manifest: &'a mut Value, id: &str) -> &'a mut Value {
    manifest["sources"]
        .as_array_mut()
        .expect("source array")
        .iter_mut()
        .find(|source| source["id"] == id)
        .expect("named source")
}

fn fixture_mut<'a>(manifest: &'a mut Value, id: &str) -> &'a mut Value {
    manifest["fixtures"]
        .as_array_mut()
        .expect("fixture array")
        .iter_mut()
        .find(|fixture| fixture["id"] == id)
        .expect("named fixture")
}

#[test]
fn manifest_shape_paths_hashes_and_provenance_fail_closed() {
    let valid = committed_manifest();

    let mut mutation = valid.clone();
    mutation["schemaVersion"] = json!(3);
    assert!(matches!(
        rejection(mutation),
        CatalogError::UnsupportedSchemaVersion(3)
    ));

    let mut mutation = valid.clone();
    mutation["sources"][0]["unexpected"] = json!(true);
    assert!(matches!(rejection(mutation), CatalogError::InvalidJson(_)));

    let mut mutation = valid.clone();
    mutation["assets"][0]["path"] = json!("/tmp/font.woff2");
    assert!(matches!(
        rejection(mutation),
        CatalogError::InvalidPath {
            kind: "hashed fixture file",
            ..
        }
    ));

    let mut mutation = valid.clone();
    mutation["fixtures"][0]["sourcePath"] = json!("../outside.mmd");
    assert!(matches!(
        rejection(mutation),
        CatalogError::InvalidPath {
            kind: "hashed fixture file",
            ..
        }
    ));

    let mut mutation = valid.clone();
    mutation["fixtures"][0]["sourceSha256"] = json!("0".repeat(64));
    assert!(matches!(
        rejection(mutation),
        CatalogError::HashMismatch { .. }
    ));

    let mut mutation = valid.clone();
    mutation["sources"][0]["licensePath"] = json!("licenses/missing.txt");
    assert!(matches!(rejection(mutation), CatalogError::ReadFile { .. }));

    let mut mutation = valid.clone();
    mutation["assets"][0]["id"] = valid["sources"][0]["id"].clone();
    assert!(matches!(
        rejection(mutation),
        CatalogError::DuplicateId { .. }
    ));

    let mut mutation = valid.clone();
    mutation["assets"][0]["unicodeRange"] = json!("U+XYZ");
    assert!(matches!(
        rejection(mutation),
        CatalogError::InvalidUnicodeRange(_)
    ));

    let mut mutation = valid.clone();
    mutation["assets"][0]["sourceSha256"] = json!("0".repeat(64));
    assert!(matches!(
        rejection(mutation),
        CatalogError::AssetSourceHashMismatch(_)
    ));

    let mut mutation = valid.clone();
    mutation["sources"][0]["revision"] = json!("main");
    assert!(matches!(
        rejection(mutation),
        CatalogError::InvalidRevision { .. }
    ));

    let mut mutation = valid;
    mutation["sources"][0]["evidence"][0]["sha256"] = json!("0".repeat(64));
    assert!(matches!(
        rejection(mutation),
        CatalogError::SourceSnapshotMismatch(_)
    ));
}

#[test]
fn fixed_reference_set_and_references_fail_closed() {
    let valid = committed_manifest();

    let mut mutation = valid.clone();
    mutation["themes"]
        .as_array_mut()
        .expect("theme array")
        .pop();
    assert!(matches!(
        rejection(mutation),
        CatalogError::ReferenceThemeSetMismatch { .. }
    ));

    let mut mutation = valid.clone();
    mutation["fixtures"][0]["assetIds"] = json!(["font-missing"]);
    assert!(matches!(
        rejection(mutation),
        CatalogError::UnknownReference {
            field: "assetIds",
            ..
        }
    ));

    let mut mutation = valid.clone();
    theme_mut(&mut mutation, "linearLight")["sourceMechanisms"] = json!(["theme-variables"]);
    assert!(matches!(rejection(mutation), CatalogError::InvalidJson(_)));

    let temp = copied_theme_root();
    let mut mutation = valid.clone();
    let relative = "evidence/modern-mermaid-theme-mechanisms.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read modern snapshot"),
    )
    .expect("parse modern snapshot");
    let linear_light = snapshot["themes"]
        .as_array_mut()
        .expect("snapshot themes")
        .iter_mut()
        .find(|theme| theme["referenceName"] == "linearLight")
        .expect("linearLight evidence");
    linear_light["mechanisms"]
        .as_array_mut()
        .expect("source mechanisms")
        .push(json!("canvas-blend"));
    linear_light["facets"]
        .as_array_mut()
        .expect("source facets")
        .push(json!({"kind": "canvas-blend", "mode": "multiply"}));
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut mutation, "source-modern-mermaid")["snapshotSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), mutation),
        CatalogError::ModernMermaidSnapshotMismatch
    ));

    let mut mutation = valid;
    theme_mut(&mut mutation, "linearLight")["fixtureIds"] = json!([]);
    assert!(matches!(
        rejection(mutation),
        CatalogError::IncompleteTheme(_)
    ));
}

#[test]
fn target_specific_translation_can_change_one_output_without_changing_the_source_matrix() {
    let mut manifest = committed_manifest();
    translation_mut(&mut manifest, "canvas-gradient")["targetOverrides"] = json!({
        "jpeg": {
            "disposition": "capability",
            "capabilities": ["solid-paint"]
        }
    });
    let catalog = ThemeFixtureCatalog::from_json(
        themes_root(),
        &serde_json::to_string(&manifest).expect("serialize target override"),
    )
    .expect("target-specific translation is a valid closed catalog");
    let translation = catalog.translation(ReferenceThemeMechanism::CanvasGradient);
    assert_eq!(
        translation.effective_for(ExpectedOutputTarget::BrowserSvg),
        (
            ReferenceMechanismDisposition::Capability,
            &std::collections::BTreeSet::from([ExpectedThemeCapability::GradientPaint])
        )
    );
    assert_eq!(
        translation.effective_for(ExpectedOutputTarget::Jpeg),
        (
            ReferenceMechanismDisposition::Capability,
            &std::collections::BTreeSet::from([ExpectedThemeCapability::SolidPaint])
        )
    );
    let cyberpunk = catalog.theme("cyberpunk").expect("cyberpunk theme");
    assert!(
        cyberpunk
            .target(ExpectedOutputTarget::Jpeg)
            .expect("JPEG target")
            .capabilities()
            .contains(&ExpectedThemeCapability::SolidPaint)
    );
    assert!(
        !cyberpunk
            .target(ExpectedOutputTarget::Jpeg)
            .expect("JPEG target")
            .capabilities()
            .contains(&ExpectedThemeCapability::GradientPaint)
    );
    let fixture = catalog
        .fixture("fixture-layered-canvas")
        .expect("layered canvas fixture");
    assert_eq!(
        fixture
            .expectation()
            .capabilities_for(ExpectedOutputTarget::Jpeg),
        Some(&std::collections::BTreeSet::from([
            ExpectedThemeCapability::BlendMode,
            ExpectedThemeCapability::LayeredCanvas,
            ExpectedThemeCapability::PatternPaint,
            ExpectedThemeCapability::SolidPaint,
        ]))
    );
}

#[test]
fn typed_fixture_capabilities_are_derived_and_cannot_be_self_declared() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "expectations/aurora-residual.json";
    let mut expectation: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read Aurora expectation"),
    )
    .expect("parse Aurora expectation");
    expectation["capabilities"] = json!(["blend-mode"]);
    let hash = write_json(temp.path(), relative, &expectation);
    manifest["fixtures"]
        .as_array_mut()
        .expect("fixture array")
        .iter_mut()
        .find(|fixture| fixture["id"] == "fixture-aurora-residual")
        .expect("Aurora fixture")["expectationSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::InvalidFixtureExpectation { fixture, .. }
            if fixture == "fixture-aurora-residual"
    ));
}

#[test]
fn typed_fixtures_reject_source_owned_visual_evidence() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/token-baseline.mmd";
    let source = fs::read_to_string(temp.path().join(relative)).expect("read token source");
    let source = format!("%%{{init: {{\"themeCSS\":\".node {{ fill: #fff; }}\"}}}}%%\n{source}");
    let hash = write_text(temp.path(), relative, &source);
    fixture_mut(&mut manifest, "fixture-token-baseline")["sourceSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::TypedFixtureContainsSourceEvidence {
            fixture,
            mechanisms,
            style_evidence_ids,
        } if fixture == "fixture-token-baseline"
            && mechanisms.is_empty()
            && style_evidence_ids.contains("global-css-theme")
    ));
}

#[test]
fn typed_fixtures_reject_global_visual_config_across_diagram_families() {
    for (fixture_id, relative) in [
        (
            "fixture-class-semantic-capabilities",
            "sources/class-semantic-capabilities.mmd",
        ),
        (
            "fixture-er-semantic-capabilities",
            "sources/er-semantic-capabilities.mmd",
        ),
        (
            "fixture-semantic-style-capabilities",
            "sources/semantic-style-capabilities.mmd",
        ),
    ] {
        let temp = copied_theme_root();
        let mut manifest = committed_manifest();
        let source = fs::read_to_string(temp.path().join(relative)).expect("read fixture source");
        let source = format!("%%{{init: {{\"theme\":\"dark\"}}}}%%\n{source}");
        let hash = write_text(temp.path(), relative, &source);
        fixture_mut(&mut manifest, fixture_id)["sourceSha256"] = json!(hash);

        assert!(matches!(
            rejection_at(temp.path(), manifest),
            CatalogError::TypedFixtureContainsSourceEvidence {
                fixture,
                mechanisms,
                style_evidence_ids,
            } if fixture == fixture_id
                && mechanisms.is_empty()
                && style_evidence_ids.contains("global-config-init")
        ));
    }
}

#[test]
fn typed_fixtures_reject_single_quoted_init_visual_config() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/token-baseline.mmd";
    let source = fs::read_to_string(temp.path().join(relative)).expect("read token source");
    let source = format!("%%{{init: {{'theme':'dark',}}}}%%\n{source}");
    let hash = write_text(temp.path(), relative, &source);
    fixture_mut(&mut manifest, "fixture-token-baseline")["sourceSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::TypedFixtureContainsSourceEvidence {
            fixture,
            mechanisms,
            style_evidence_ids,
        } if fixture == "fixture-token-baseline"
            && mechanisms.is_empty()
            && style_evidence_ids.contains("global-config-init")
    ));
}

#[test]
fn frontmatter_visual_evidence_is_not_inferred_from_later_directives() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let source_relative = "sources/state-style-precedence.mmd";
    let original =
        fs::read_to_string(temp.path().join(source_relative)).expect("read state source");
    let diagram = original
        .split_once("stateDiagram-v2")
        .map(|(_, rest)| format!("stateDiagram-v2{rest}"))
        .expect("state diagram body");
    let source = format!(
        "---\nconfig:\n  securityLevel: strict\n---\n%%{{init: {{'themeVariables': {{'primaryColor': '#dbeafe', 'primaryTextColor': '#1e3a8a'}}}}}}%%\n{diagram}"
    );
    let source_hash = write_text(temp.path(), source_relative, &source);
    fixture_mut(&mut manifest, "fixture-state-style-precedence")["sourceSha256"] =
        json!(source_hash);

    let expectation_relative = "expectations/state-style-precedence.json";
    let mut expectation: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(expectation_relative))
            .expect("read state expectation"),
    )
    .expect("parse state expectation");
    expectation["styleEvidenceIds"]
        .as_array_mut()
        .expect("style evidence array")
        .push(json!("global-config-frontmatter"));
    let expectation_hash = write_json(temp.path(), expectation_relative, &expectation);
    fixture_mut(&mut manifest, "fixture-state-style-precedence")["expectationSha256"] =
        json!(expectation_hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceStyleEvidenceMismatch {
            fixture,
            expected,
            actual,
        } if fixture == "fixture-state-style-precedence"
            && expected.contains("global-config-frontmatter")
            && !actual.contains("global-config-frontmatter")
            && actual.contains("global-config-init")
    ));
}

#[test]
fn typed_theme_input_is_the_authority_for_fixture_mechanisms() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "inputs/token-baseline.json";
    let mut input: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read token input"),
    )
    .expect("parse token input");
    input["nodeStyle"]["cornerRadiusPx"] = Value::Null;
    let hash = write_json(temp.path(), relative, &input);
    manifest["fixtures"]
        .as_array_mut()
        .expect("fixture array")
        .iter_mut()
        .find(|fixture| fixture["id"] == "fixture-token-baseline")
        .expect("token fixture")["themeInputSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::UncoveredSourceMechanism {
            mechanism: ReferenceThemeMechanism::RoundedCorners,
            ..
        }
    ));
}

#[test]
fn typed_evidence_cannot_substitute_a_different_mechanism_with_the_same_capability() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();

    let input_relative = "inputs/semantic-style-capabilities.json";
    let mut input: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(input_relative)).expect("read semantic input"),
    )
    .expect("parse semantic input");
    input["semanticRules"]
        .as_array_mut()
        .expect("semantic rules")
        .retain(|rule| rule["kind"] != "not-class");
    input["nodeStyle"] = json!({
        "border": null,
        "dashPattern": [],
        "cornerRadiusPx": null,
        "shadow": {
            "offsetXPx": 0,
            "offsetYPx": 2,
            "blurPx": 6,
            "spreadPx": 0,
            "color": "#0f172a33"
        }
    });
    let input_hash = write_json(temp.path(), input_relative, &input);
    fixture_mut(&mut manifest, "fixture-semantic-style-capabilities")["themeInputSha256"] =
        json!(input_hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::UncoveredSourceMechanism {
            mechanism: ReferenceThemeMechanism::NotSelector,
            ..
        }
    ));
}

#[test]
fn source_required_canvas_variants_must_match_typed_evidence() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "inputs/layered-canvas.json";
    let mut input: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read layered canvas input"),
    )
    .expect("parse layered canvas input");
    input["blendMode"] = json!("multiply");
    let hash = write_json(temp.path(), relative, &input);
    fixture_mut(&mut manifest, "fixture-layered-canvas")["themeInputSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::UncoveredSourceFacet { .. }
    ));
}

#[test]
fn fixture_font_families_must_resolve_to_the_declared_assets() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "inputs/mixed-script-typography.json";
    let mut input: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read typography input"),
    )
    .expect("parse typography input");
    input["typography"]["fontStack"]["families"] = json!(["Unrelated Sans"]);
    let hash = write_json(temp.path(), relative, &input);
    fixture_mut(&mut manifest, "fixture-mixed-script-typography")["themeInputSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::UnresolvedFixtureFontFamily { fixture, .. }
            if fixture == "fixture-mixed-script-typography"
    ));
}

#[test]
fn font_coverage_is_derived_from_the_parsed_visible_source_text() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/mixed-script-typography.mmd";
    let mut source = fs::read_to_string(temp.path().join(relative)).expect("read fixture source");
    source.push_str("    Mixed --> Missing[主]\n");
    fs::write(temp.path().join(relative), source.as_bytes()).expect("write fixture source");
    fixture_mut(&mut manifest, "fixture-mixed-script-typography")["sourceSha256"] =
        json!(format!("{:x}", Sha256::digest(source.as_bytes())));

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::UncoveredFixtureCodepoint { fixture, .. }
            if fixture == "fixture-mixed-script-typography"
    ));
}

#[test]
fn typed_fixture_output_coverage_cannot_be_narrowed_by_self_declaration() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed catalog");
    let fixture = catalog
        .fixture("fixture-mixed-script-typography")
        .expect("typography fixture");
    assert_eq!(
        fixture.expectation().outputs(),
        &std::collections::BTreeSet::from([
            ExpectedOutputTarget::BrowserSvg,
            ExpectedOutputTarget::Jpeg,
            ExpectedOutputTarget::Pdf,
            ExpectedOutputTarget::Png,
            ExpectedOutputTarget::StandaloneSvg,
        ])
    );

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "expectations/mixed-script-typography.json";
    let mut expectation: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read typography expectation"),
    )
    .expect("parse typography expectation");
    expectation["outputs"] = json!(["browser-svg"]);
    let hash = write_json(temp.path(), relative, &expectation);
    fixture_mut(&mut manifest, "fixture-mixed-script-typography")["expectationSha256"] =
        json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::InvalidFixtureExpectation { fixture, .. }
            if fixture == "fixture-mixed-script-typography"
    ));
}

#[test]
fn source_compatibility_fixture_must_reference_known_style_evidence() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "expectations/style-precedence.json";
    let mut expectation: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read style expectation"),
    )
    .expect("parse style expectation");
    expectation["styleEvidenceIds"] = json!(["missing-style-contract"]);
    let hash = write_json(temp.path(), relative, &expectation);
    manifest["fixtures"]
        .as_array_mut()
        .expect("fixture array")
        .iter_mut()
        .find(|fixture| fixture["id"] == "fixture-style-precedence")
        .expect("style fixture")["expectationSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::UnknownStyleEvidence { fixture, id }
            if fixture == "fixture-style-precedence" && id == "missing-style-contract"
    ));
}

#[test]
fn source_compatibility_fixtures_must_parse_and_match_evidence_family() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/class-assignment-before-definition.mmd";
    let hash = write_text(temp.path(), relative, "this is not a Mermaid diagram\n");
    fixture_mut(&mut manifest, "fixture-class-assignment-before-definition")["sourceSha256"] =
        json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::InvalidFixtureSource { fixture, .. }
            if fixture == "fixture-class-assignment-before-definition"
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "expectations/style-precedence.json";
    let mut expectation: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read style expectation"),
    )
    .expect("parse style expectation");
    expectation["styleEvidenceIds"] = json!(["class-node-inline"]);
    let hash = write_json(temp.path(), relative, &expectation);
    fixture_mut(&mut manifest, "fixture-style-precedence")["expectationSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::StyleEvidenceFamilyMismatch { fixture, id, .. }
            if fixture == "fixture-style-precedence" && id == "class-node-inline"
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/class-assignment-before-definition.mmd";
    let hash = write_text(temp.path(), relative, "classDiagram\n    class User\n");
    fixture_mut(&mut manifest, "fixture-class-assignment-before-definition")["sourceSha256"] =
        json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceStyleEvidenceMismatch { fixture, actual, .. }
            if fixture == "fixture-class-assignment-before-definition" && actual.is_empty()
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/style-precedence.mmd";
    let source = concat!(
        "%%{init: {\"theme\":\"base\",\"themeVariables\":{\"primaryColor\":\"#dbeafe\"},",
        "\"themeCSS\":\".node:nth-child(2):not(.disabled) rect { stroke: #7c3aed; } ",
        "svg:has(.node) .label { text-transform: uppercase; letter-spacing: 0.04em; }\"}}%%\n",
        "flowchart LR\n    A[Default] --> B[Assigned]\n    B --> C[Inline]\n",
    );
    let hash = write_text(temp.path(), relative, source);
    fixture_mut(&mut manifest, "fixture-style-precedence")["sourceSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceStyleEvidenceMismatch { fixture, actual, .. }
            if fixture == "fixture-style-precedence"
                && !actual.contains("flow-node-inline")
                && !actual.contains("flow-node-assigned")
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "expectations/style-precedence.json";
    let mut expectation: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read style expectation"),
    )
    .expect("parse style expectation");
    expectation["referenceMechanisms"]
        .as_array_mut()
        .expect("compatibility mechanism array")
        .push(json!("canvas-gradient"));
    let hash = write_json(temp.path(), relative, &expectation);
    fixture_mut(&mut manifest, "fixture-style-precedence")["expectationSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceCompatibilityMechanismMismatch { fixture, .. }
            if fixture == "fixture-style-precedence"
    ));
}

#[test]
fn source_compatibility_rejects_security_only_init_as_visual_evidence() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/state-style-precedence.mmd";
    let source = fs::read_to_string(temp.path().join(relative)).expect("read state source");
    let source = source.replacen(
        "%%{init: {\"themeVariables\":{\"primaryColor\":\"#dbeafe\",\"primaryTextColor\":\"#1e3a8a\"}}}%%",
        "%%{init: {\"securityLevel\":\"strict\",\"themeVariables\":{},\"flowchart\":{}}}%%",
        1,
    );
    assert_ne!(
        source,
        fs::read_to_string(temp.path().join(relative)).expect("read state source")
    );
    let hash = write_text(temp.path(), relative, &source);
    fixture_mut(&mut manifest, "fixture-state-style-precedence")["sourceSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceStyleEvidenceMismatch {
            fixture,
            expected,
            actual,
        } if fixture == "fixture-state-style-precedence"
            && expected.contains("global-config-init")
            && !actual.contains("global-config-init")
    ));
}

#[test]
fn source_compatibility_rejects_unmodeled_theme_css() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/style-precedence.mmd";
    let source = fs::read_to_string(temp.path().join(relative)).expect("read style source");
    let source = source.replacen(
        "themeCSS\":\"",
        "themeCSS\":\"@media screen { .node { stroke: #7c3aed; } } ",
        1,
    );
    assert_ne!(
        source,
        fs::read_to_string(temp.path().join(relative)).expect("read style source")
    );
    let hash = write_text(temp.path(), relative, &source);
    fixture_mut(&mut manifest, "fixture-style-precedence")["sourceSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::UnmodeledThemeCss { fixture, .. }
            if fixture == "fixture-style-precedence"
    ));
}

#[test]
fn source_compatibility_rejects_css_and_theme_variable_placeholders() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/style-precedence.mmd";
    let source = concat!(
        "%%{init: {\"theme\":\"base\",\"themeVariables\":{\"actorBkg\":\"#dbeafe\"},",
        "\"themeCSS\":\"/* :nth-child( :not( :has( text-transform: letter-spacing: */ ",
        ".node::before { content: ':has('; }\"}}%%\n",
        "flowchart LR\n",
        "    A[Default] --> B[Assigned]\n",
        "    B --> C[Inline]\n",
        "    classDef default bogus\n",
        "    classDef assigned bogus\n",
        "    class B assigned\n",
        "    style C bogus\n",
    );
    let hash = write_text(temp.path(), relative, source);
    fixture_mut(&mut manifest, "fixture-style-precedence")["sourceSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceCompatibilityMechanismMismatch { fixture, actual, .. }
            if fixture == "fixture-style-precedence"
                && !actual.contains(&ReferenceThemeMechanism::ThemeVariables)
                && !actual.contains(&ReferenceThemeMechanism::NthChildSelector)
                && !actual.contains(&ReferenceThemeMechanism::HasSelector)
                && !actual.contains(&ReferenceThemeMechanism::NotSelector)
                && !actual.contains(&ReferenceThemeMechanism::CssTextTransform)
                && !actual.contains(&ReferenceThemeMechanism::CssLetterSpacing)
    ));
}

#[test]
fn source_compatibility_rejects_non_paint_class_and_inline_placeholders() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/style-precedence.mmd";
    let source = concat!(
        "%%{init: {\"theme\":\"base\",\"themeVariables\":{\"primaryColor\":\"#dbeafe\"},",
        "\"themeCSS\":\".node:nth-child(2):not(.disabled) rect { stroke: #7c3aed; } ",
        "svg:has(.node) .label { text-transform: uppercase; letter-spacing: 0.04em; }\"}}%%\n",
        "flowchart LR\n",
        "    A[Default] --> B[Assigned]\n",
        "    B --> C[Inline]\n",
        "    classDef default bogus\n",
        "    classDef assigned bogus\n",
        "    class B assigned\n",
        "    style C bogus\n",
    );
    let hash = write_text(temp.path(), relative, source);
    fixture_mut(&mut manifest, "fixture-style-precedence")["sourceSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceStyleEvidenceMismatch { fixture, actual, .. }
            if fixture == "fixture-style-precedence"
                && !actual.contains("global-css-classdef")
                && !actual.contains("flow-node-default")
                && !actual.contains("flow-node-named")
                && !actual.contains("flow-node-assigned")
                && !actual.contains("flow-node-inline")
    ));
}

#[test]
fn class_encounter_order_evidence_rejects_missing_targets_and_inline_mimics() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/class-assignment-before-definition.mmd";
    let source = concat!(
        "classDiagram\n",
        "    class User\n",
        "    cssClass \"Missing\" accent\n",
        "    classDef accent fill:red\n",
        "    cssClass \"User\" accent\n",
        "    style User fill:red\n",
    );
    let hash = write_text(temp.path(), relative, source);
    fixture_mut(&mut manifest, "fixture-class-assignment-before-definition")["sourceSha256"] =
        json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceStyleEvidenceMismatch {
            fixture,
            expected,
            actual,
        } if fixture == "fixture-class-assignment-before-definition"
            && expected.contains("class-assignment-before-definition-copy")
            && !actual.contains("class-assignment-before-definition-copy")
            && actual.contains("class-definition-before-assignment-no-backfill")
    ));
}

#[test]
fn er_precedence_evidence_requires_one_entity_to_observe_the_complete_origin_chain() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "sources/er-style-precedence.mmd";
    let source = fs::read_to_string(temp.path().join(relative)).expect("read ER source");
    let source = source.replacen("class ACCOUNT assigned", "class CUSTOMER assigned", 1);
    assert_ne!(
        source,
        fs::read_to_string(temp.path().join(relative)).expect("read ER source")
    );
    let hash = write_text(temp.path(), relative, &source);
    fixture_mut(&mut manifest, "fixture-er-style-precedence")["sourceSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::SourceStyleEvidenceMismatch {
            fixture,
            expected,
            actual,
        } if fixture == "fixture-er-style-precedence"
            && expected.contains("er-node-default")
            && expected.contains("er-typography-inline-residual")
            && !actual.contains("er-node-default")
            && !actual.contains("er-typography-inline-residual")
    ));
}

#[test]
fn controlled_font_families_cannot_follow_a_generic_fallback() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "inputs/mixed-script-typography.json";
    let mut input: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read typography input"),
    )
    .expect("parse typography input");
    input["typography"]["fontStack"]["families"] = json!(["sans-serif", "Excalifont", "Xiaolai"]);
    let hash = write_json(temp.path(), relative, &input);
    fixture_mut(&mut manifest, "fixture-mixed-script-typography")["themeInputSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::FixtureFontAfterGenericFallback {
            fixture,
            family,
            generic,
        } if fixture == "fixture-mixed-script-typography"
            && family == "Excalifont"
            && generic == "sans-serif"
    ));
}

#[test]
fn reference_themes_are_bound_to_the_modern_mermaid_source() {
    let mut manifest = committed_manifest();
    theme_mut(&mut manifest, "linearLight")["sourceId"] = json!("source-mermaid");
    assert!(matches!(
        rejection(manifest),
        CatalogError::IncompleteTheme(theme) if theme == "theme-linear-light"
    ));
}

#[test]
fn semantic_snapshot_content_is_validated_after_its_hash() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/excalidraw-font-provenance.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read Excalidraw snapshot"),
    )
    .expect("parse Excalidraw snapshot");
    snapshot["fonts"][0]["runtimeFamily"] = json!("Drifted Family");
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-excalidraw")["snapshotSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::ExcalidrawFontSnapshotMismatch
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/excalidraw-font-provenance.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read Excalidraw snapshot"),
    )
    .expect("parse Excalidraw snapshot");
    let duplicate = snapshot["fonts"][0].clone();
    snapshot["fonts"]
        .as_array_mut()
        .expect("font records")
        .push(duplicate);
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-excalidraw")["snapshotSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::ExcalidrawFontSnapshotMismatch
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/mermaid-style-precedence.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read Mermaid snapshot"),
    )
    .expect("parse Mermaid snapshot");
    snapshot["entries"]
        .as_array_mut()
        .expect("precedence entries")
        .pop();
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-mermaid")["snapshotSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::MermaidStylePrecedenceSnapshotMismatch
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/mermaid-style-precedence.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read Mermaid snapshot"),
    )
    .expect("parse Mermaid snapshot");
    snapshot["entries"]
        .as_array_mut()
        .expect("precedence entries")
        .iter_mut()
        .find(|entry| entry["id"] == "global-config-init")
        .expect("ranked config entry")["rank"] = json!(0);
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-mermaid")["snapshotSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::MermaidStylePrecedenceSnapshotMismatch
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/mermaid-style-precedence.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read Mermaid snapshot"),
    )
    .expect("parse Mermaid snapshot");
    snapshot["entries"]
        .as_array_mut()
        .expect("precedence entries")
        .iter_mut()
        .find(|entry| entry["id"] == "state-node-assigned")
        .expect("state assigned entry")["origin"] = json!("inline-style");
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-mermaid")["snapshotSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::MermaidStylePrecedenceSnapshotMismatch
    ));

    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/mermaid-style-precedence.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read Mermaid snapshot"),
    )
    .expect("parse Mermaid snapshot");
    snapshot["entries"]
        .as_array_mut()
        .expect("precedence entries")
        .iter_mut()
        .find(|entry| entry["id"] == "class-typography-classdef-residual")
        .expect("class typography entry")["evidence"]
        .as_array_mut()
        .expect("class typography evidence")
        .retain(|reference| {
            reference.as_str().is_none_or(|reference| {
                !reference.starts_with("packages/mermaid/src/diagrams/class/shapeUtil.ts:")
            })
        });
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-mermaid")["snapshotSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::MermaidStylePrecedenceSnapshotMismatch
    ));
}

#[test]
#[ignore = "requires MERMAN_REPO_REF_ROOT with the pinned Mermaid checkout"]
fn mermaid_style_evidence_ranges_must_fit_the_pinned_source_files() {
    let repo_ref = std::env::var_os("MERMAN_REPO_REF_ROOT")
        .map(PathBuf::from)
        .expect("set MERMAN_REPO_REF_ROOT");
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/mermaid-style-precedence.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read Mermaid snapshot"),
    )
    .expect("parse Mermaid snapshot");
    let reference = snapshot["entries"][0]["evidence"][0]
        .as_str()
        .expect("source reference");
    let path = reference.split_once(':').expect("path and ranges").0;
    snapshot["entries"][0]["evidence"][0] = json!(format!("{path}:1-999999"));
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-mermaid")["snapshotSha256"] = json!(hash);

    let catalog = ThemeFixtureCatalog::from_json(
        temp.path(),
        &serde_json::to_string(&manifest).expect("serialize mutation"),
    )
    .expect("syntactically valid source evidence");
    let revision = catalog
        .source("source-mermaid")
        .expect("Mermaid source")
        .revision()
        .to_string();
    assert!(matches!(
        catalog.verify_source_checkout("source-mermaid", &revision, repo_ref.join("mermaid")),
        Err(CatalogError::MermaidStyleEvidenceRangeOutOfBounds { .. })
    ));
}

#[test]
fn semantic_snapshot_requires_value_facets_for_selector_mechanisms() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/modern-mermaid-theme-mechanisms.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read modern snapshot"),
    )
    .expect("parse modern snapshot");
    let win95 = snapshot["themes"]
        .as_array_mut()
        .expect("snapshot themes")
        .iter_mut()
        .find(|theme| theme["referenceName"] == "win95")
        .expect("Win95 evidence");
    win95["facets"]
        .as_array_mut()
        .expect("source facets")
        .retain(|facet| facet["kind"] != "semantic-selector");
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-modern-mermaid")["snapshotSha256"] = json!(hash);
    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::ModernMermaidSnapshotMismatch
    ));
}

#[test]
fn modern_semantic_matrix_rejects_joint_mechanism_and_facet_rewrites() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/modern-mermaid-theme-mechanisms.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read modern snapshot"),
    )
    .expect("parse modern snapshot");
    let linear = snapshot["themes"]
        .as_array_mut()
        .expect("snapshot themes")
        .iter_mut()
        .find(|theme| theme["referenceName"] == "linearLight")
        .expect("linearLight evidence");
    linear["mechanisms"]
        .as_array_mut()
        .expect("mechanism array")
        .retain(|mechanism| mechanism != "canvas-pattern");
    linear["facets"][0]["repetition"] = json!("none");
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-modern-mermaid")["snapshotSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::ModernMermaidSnapshotMismatch
    ));
}

#[test]
fn modern_canvas_layering_is_derived_from_the_declared_layer_count() {
    for (theme_name, mutate) in [
        ("linearLight", (Some("canvas-layering"), None, None)),
        ("noir", (None, Some("canvas-layering"), None)),
        ("linearLight", (None, None, Some(2_u64))),
        ("noir", (None, None, Some(2_u64))),
    ] {
        let temp = copied_theme_root();
        let mut manifest = committed_manifest();
        let relative = "evidence/modern-mermaid-theme-mechanisms.json";
        let mut snapshot: Value = serde_json::from_str(
            &fs::read_to_string(temp.path().join(relative)).expect("read modern snapshot"),
        )
        .expect("parse modern snapshot");
        let theme = snapshot["themes"]
            .as_array_mut()
            .expect("snapshot themes")
            .iter_mut()
            .find(|theme| theme["referenceName"] == theme_name)
            .expect("named theme evidence");
        let (add, remove, layer_count) = mutate;
        if let Some(mechanism) = add {
            theme["mechanisms"]
                .as_array_mut()
                .expect("mechanism array")
                .push(json!(mechanism));
        }
        if let Some(mechanism) = remove {
            theme["mechanisms"]
                .as_array_mut()
                .expect("mechanism array")
                .retain(|value| value != mechanism);
        }
        if let Some(layer_count) = layer_count {
            theme["canvasLayerCount"] = json!(layer_count);
        }
        let hash = write_json(temp.path(), relative, &snapshot);
        source_mut(&mut manifest, "source-modern-mermaid")["snapshotSha256"] = json!(hash);
        assert!(matches!(
            rejection_at(temp.path(), manifest),
            CatalogError::ModernMermaidSnapshotMismatch
        ));
    }
}

#[test]
fn modern_theme_source_lines_are_part_of_the_closed_semantic_matrix() {
    let temp = copied_theme_root();
    let mut manifest = committed_manifest();
    let relative = "evidence/modern-mermaid-theme-mechanisms.json";
    let mut snapshot: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(relative)).expect("read modern snapshot"),
    )
    .expect("parse modern snapshot");
    snapshot["themes"]
        .as_array_mut()
        .expect("snapshot themes")
        .iter_mut()
        .find(|theme| theme["referenceName"] == "linearLight")
        .expect("linearLight evidence")["sourceLine"] = json!(19);
    let hash = write_json(temp.path(), relative, &snapshot);
    source_mut(&mut manifest, "source-modern-mermaid")["snapshotSha256"] = json!(hash);

    assert!(matches!(
        rejection_at(temp.path(), manifest),
        CatalogError::ModernMermaidSnapshotMismatch
    ));
}

#[test]
fn read_apis_revalidate_the_exact_bytes_they_return() {
    let temp = copied_theme_root();
    let catalog = ThemeFixtureCatalog::load(temp.path()).expect("load copied catalog");

    fs::write(
        temp.path().join("sources/token-baseline.mmd"),
        "flowchart LR\n    Changed[Changed]\n",
    )
    .expect("replace fixture source");
    assert!(matches!(
        catalog.source_text("fixture-token-baseline"),
        Err(CatalogError::HashMismatch { .. })
    ));

    fs::write(
        temp.path()
            .join("assets/fonts/Excalifont-Regular-Latin.woff2"),
        b"changed font bytes",
    )
    .expect("replace font asset");
    assert!(matches!(
        catalog.asset_bytes("font-excalifont-latin"),
        Err(CatalogError::HashMismatch { .. })
    ));

    fs::write(
        temp.path()
            .join("evidence/modern-mermaid-theme-mechanisms.json"),
        b"{}\n",
    )
    .expect("replace source snapshot");
    assert!(matches!(
        catalog.source_snapshot_text("source-modern-mermaid"),
        Err(CatalogError::HashMismatch { .. })
    ));
}

#[test]
fn sparse_target_policy_expands_and_derived_targets_fail_closed() {
    let valid = committed_manifest();
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed catalog");
    let expected_targets = std::collections::BTreeSet::from([
        ExpectedOutputTarget::BrowserSvg,
        ExpectedOutputTarget::Jpeg,
        ExpectedOutputTarget::Pdf,
        ExpectedOutputTarget::Png,
        ExpectedOutputTarget::StandaloneSvg,
    ]);
    for theme in catalog.themes() {
        assert_eq!(
            theme
                .targets()
                .iter()
                .map(|target| target.target())
                .collect::<std::collections::BTreeSet<_>>(),
            expected_targets
        );
    }

    let mut mutation = valid.clone();
    theme_mut(&mut mutation, "linearLight")["targetPolicy"]["default"]["capabilities"] = json!([]);
    assert!(matches!(rejection(mutation), CatalogError::InvalidJson(_)));

    let mut mutation = valid.clone();
    let noir = theme_mut(&mut mutation, "noir");
    noir["fixtureIds"]
        .as_array_mut()
        .expect("fixture IDs")
        .retain(|id| id != "fixture-semantic-style-capabilities");
    assert!(matches!(
        rejection(mutation),
        CatalogError::UncoveredSourceMechanism {
            mechanism: ReferenceThemeMechanism::NotSelector,
            ..
        }
    ));

    let mut mutation = valid.clone();
    theme_mut(&mut mutation, "linearLight")["targetPolicy"]["overrides"]["webp"] = json!({
        "grade": "portable",
        "residuals": []
    });
    assert!(matches!(rejection(mutation), CatalogError::InvalidJson(_)));

    let mut mutation = valid.clone();
    theme_mut(&mut mutation, "linearLight")["targetPolicy"]["overrides"]["browser-svg"] = json!({
        "grade": "unverified",
        "residuals": []
    });
    assert!(matches!(
        rejection(mutation),
        CatalogError::UnverifiedTargetMissingResidual {
            target: ExpectedOutputTarget::BrowserSvg,
            ..
        }
    ));

    let mut mutation = valid.clone();
    let aurora = theme_mut(&mut mutation, "aurora");
    aurora["targetPolicy"]["default"]["residuals"] = json!([]);
    assert!(matches!(
        rejection(mutation),
        CatalogError::TargetResidualSetMismatch {
            target: ExpectedOutputTarget::BrowserSvg,
            ..
        }
    ));

    let mut mutation = valid.clone();
    let aurora = theme_mut(&mut mutation, "aurora");
    aurora["targetPolicy"]["default"]["grade"] = json!("portable");
    assert!(matches!(
        rejection(mutation),
        CatalogError::PortableTargetHasResidual {
            target: ExpectedOutputTarget::BrowserSvg,
            ..
        }
    ));

    let mut mutation = valid;
    theme_mut(&mut mutation, "aurora")["targetPolicy"]["overrides"]["browser-svg"] = json!({
        "grade": "unverified",
        "residuals": []
    });
    assert!(matches!(
        rejection(mutation),
        CatalogError::TargetResidualSetMismatch {
            target: ExpectedOutputTarget::BrowserSvg,
            ..
        }
    ));
}
