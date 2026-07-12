use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let sequence = TEMP_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "merman-upstream-provenance-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create provenance test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn test_source() -> UpstreamSvgSource {
    UpstreamSvgSource {
        mermaid_version: PINNED_MERMAID_VERSION.to_string(),
        mermaid_cli_version: PINNED_MERMAID_CLI_VERSION.to_string(),
        mermaid_source_tag: MERMAID_SOURCE_TAG.to_string(),
        mermaid_source_commit: MERMAID_SOURCE_COMMIT.to_string(),
        package_json_sha256: "package-json".to_string(),
        package_lock_sha256: "package-lock".to_string(),
        mermaid_config_sha256: "config".to_string(),
        renderer_revision: RENDERER_REVISION.to_string(),
    }
}

fn test_render_environment() -> UpstreamSvgRenderEnvironment {
    UpstreamSvgRenderEnvironment {
        browser: UpstreamSvgBrowserEnvironment {
            product: "Chrome".to_string(),
            version: "131.0.6778.204".to_string(),
            revision: "131.0.6778.204".to_string(),
        },
        puppeteer: UpstreamSvgPuppeteerEnvironment {
            version: "23.11.1".to_string(),
        },
        operating_system: UpstreamSvgOperatingSystemEnvironment {
            platform: "win32".to_string(),
            arch: "x64".to_string(),
            release: "10.0.26100".to_string(),
        },
        mermaid_runtime: UpstreamSvgRuntimeEnvironment {
            esm_version: PINNED_MERMAID_VERSION.to_string(),
            iife_version: PINNED_MERMAID_VERSION.to_string(),
            mermaid_package_sha256:
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            mermaid_cli_package_sha256:
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
        },
        font_probe: UpstreamSvgFontProbeEnvironment {
            revision: "font-probe-v1".to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        },
    }
}

fn test_attestation(mode: UpstreamSvgAttestationMode) -> UpstreamSvgAttestation {
    match mode {
        UpstreamSvgAttestationMode::Generated => {
            UpstreamSvgAttestation::generated(test_render_environment())
        }
        UpstreamSvgAttestationMode::AdoptedExisting => UpstreamSvgAttestation::adopted_existing(),
    }
}

fn write_fixture(dir: &Path, stem: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("create fixture directory");
    let path = dir.join(format!("{stem}.mmd"));
    fs::write(&path, "flowchart TD\n  A --> B\n").expect("write fixture");
    path
}

fn write_svg(dir: &Path, stem: &str, root_id: &str, include_marker: bool) -> PathBuf {
    fs::create_dir_all(dir).expect("create SVG directory");
    let marker = if include_marker {
        format!(r#"#{root_id}   [data-look = "neo"].swimlane.cluster   rect {{ filter : none; }}"#)
    } else {
        format!("#{root_id} .node {{ fill: red; }}")
    };
    let path = dir.join(format!("{stem}.svg"));
    fs::write(
        &path,
        format!(r#"<svg id="{root_id}"><style>{marker}</style><g/></svg>"#),
    )
    .expect("write SVG");
    path
}

#[test]
fn pinned_source_metadata_is_mermaid_11_16() {
    let source = current_source().expect("read pinned Mermaid source metadata");
    assert_eq!(source.mermaid_version, "11.16.0");
    assert_eq!(source.mermaid_cli_version, "11.16.0");
    assert_eq!(source.mermaid_source_tag, "mermaid@11.16.0");
    assert_eq!(
        source.mermaid_source_commit,
        "7c0cafcf42e76bfaf79d0cbbd12edb986612f014"
    );
    assert_eq!(source.package_json_sha256, PACKAGE_JSON_SHA256);
    assert_eq!(source.package_lock_sha256, PACKAGE_LOCK_SHA256);
    assert_eq!(source.mermaid_config_sha256, MERMAID_CONFIG_SHA256);
}

#[test]
fn renderer_profiles_capture_seed_and_width_variants() {
    assert_eq!(renderer_profile("architecture"), "seeded-puppeteer-seed-1");
    assert_eq!(renderer_profile("gitgraph"), "seeded-puppeteer-seed-1");
    assert_eq!(renderer_profile("gantt"), "mmdc-default-width-1200");
    assert_eq!(renderer_profile("flowchart"), "mmdc-default");
}

#[test]
fn sha256_is_stable() {
    assert_eq!(
        hash_bytes(b"mermaid@11.16.0"),
        "7df890c8bf83c444c6ed6a3d72b6542be9d574866539c558f3147e4e895beaa2"
    );
}

#[test]
fn legacy_manifest_without_attestation_is_rejected_instead_of_upgraded() {
    let err = serde_json::from_value::<UpstreamSvgManifest>(serde_json::json!({
        "schema_version": 1,
        "source": test_source(),
        "complete": false,
        "fixtures": {},
        "excluded": {}
    }))
    .expect_err("legacy provenance without proof must be rejected");

    assert!(err.to_string().contains("attestation"), "{err}");
}

#[test]
fn generated_attestation_without_render_environment_is_rejected() {
    let err = serde_json::from_value::<UpstreamSvgManifest>(serde_json::json!({
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "source": test_source(),
        "attestation": { "mode": "generated" },
        "complete": false,
        "fixtures": {},
        "excluded": {}
    }))
    .expect_err("generated provenance requires a render environment");

    assert!(err.to_string().contains("render_environment"), "{err}");
}

#[test]
fn adopted_existing_attestation_cannot_carry_a_render_environment() {
    let err = serde_json::from_value::<UpstreamSvgManifest>(serde_json::json!({
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "source": test_source(),
        "attestation": {
            "mode": "adopted-existing",
            "render_environment": test_render_environment()
        },
        "complete": false,
        "fixtures": {},
        "excluded": {}
    }))
    .expect_err("adopted provenance must not claim a render environment");

    assert!(err.to_string().contains("render_environment"), "{err}");
}

#[test]
fn render_environment_equality_covers_every_recorded_field() {
    let environment = test_render_environment();
    let variants = [
        {
            let mut changed = environment.clone();
            changed.browser.product.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.browser.version.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.browser.revision.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.puppeteer.version.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.operating_system.platform.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.operating_system.arch.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.operating_system.release.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.mermaid_runtime.esm_version.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.mermaid_runtime.iife_version.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.mermaid_runtime.mermaid_package_sha256 =
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string();
            changed
        },
        {
            let mut changed = environment.clone();
            changed.mermaid_runtime.mermaid_cli_package_sha256 =
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
            changed
        },
        {
            let mut changed = environment.clone();
            changed.font_probe.revision.push_str("-changed");
            changed
        },
        {
            let mut changed = environment.clone();
            changed.font_probe.sha256 =
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string();
            changed
        },
    ];

    for changed in variants {
        assert_ne!(&environment, &changed);
    }
}

#[test]
fn render_environment_rejects_empty_fields_and_invalid_digests() {
    let mut empty = test_render_environment();
    empty.browser.product.clear();
    let empty_error = empty.validate().expect_err("empty fields must fail");
    assert!(empty_error.to_string().contains("browser.product"));

    let mut invalid = test_render_environment();
    invalid.mermaid_runtime.mermaid_package_sha256 = "not-a-digest".to_string();
    let digest_error = invalid.validate().expect_err("invalid digest must fail");
    assert!(
        digest_error
            .to_string()
            .contains("mermaid_runtime.mermaid_package_sha256")
    );
}

#[test]
fn fresh_checks_require_matching_generated_environments() {
    let environment = test_render_environment();
    let generated_validator =
        |diagram: &str, environment: UpstreamSvgRenderEnvironment| UpstreamSvgProvenanceValidator {
            diagram: diagram.to_string(),
            manifest: UpstreamSvgManifest::empty(
                test_source(),
                UpstreamSvgAttestation::generated(environment),
            ),
        };

    let baseline = generated_validator("sequence", environment.clone());
    let matching = generated_validator("sequence", environment.clone());
    baseline
        .require_same_generated_environment(&matching)
        .expect("identical generated environments are comparable");

    let mut changed = environment.clone();
    changed.font_probe.sha256 =
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string();
    let different = generated_validator("sequence", changed);
    let mismatch = baseline
        .require_same_generated_environment(&different)
        .expect_err("different font environments must fail before SVG comparison");
    assert!(mismatch.to_string().contains("render environment differs"));

    let adopted = UpstreamSvgProvenanceValidator {
        diagram: "sequence".to_string(),
        manifest: UpstreamSvgManifest::empty(
            test_source(),
            UpstreamSvgAttestation::adopted_existing(),
        ),
    };
    let missing = adopted
        .require_same_generated_environment(&matching)
        .expect_err("adopted baselines cannot prove a fresh environment match");
    assert!(missing.to_string().contains("adopted-existing"));
}

#[test]
fn partial_generation_requires_matching_generated_attestation() {
    let source = test_source();
    let environment = test_render_environment();

    let fresh = prepare_generated_manifest(None, &source, &environment, true)
        .expect("a fresh temporary output may record a partial generated corpus");
    assert!(!fresh.complete);
    assert_eq!(fresh.attestation.render_environment(), Some(&environment));
    let fresh_existing =
        prepare_generated_manifest(Some(fresh.clone()), &source, &environment, true)
            .expect_err("fresh output must never merge an existing manifest");
    assert!(
        fresh_existing
            .to_string()
            .contains("already has a manifest")
    );

    let missing = prepare_partial_generated_manifest(None, &source, &environment)
        .expect_err("partial generation needs an existing proof");
    assert!(missing.to_string().contains("complete generation"));

    let adopted =
        UpstreamSvgManifest::empty(source.clone(), UpstreamSvgAttestation::adopted_existing());
    let adopted = prepare_partial_generated_manifest(Some(adopted), &source, &environment)
        .expect_err("adopted corpora cannot be partially merged");
    assert!(adopted.to_string().contains("adopted-existing"));

    let mut different_environment = environment.clone();
    different_environment.browser.version.push_str("-different");
    let generated = UpstreamSvgManifest::empty(
        source.clone(),
        UpstreamSvgAttestation::generated(environment.clone()),
    );
    let different = prepare_partial_generated_manifest(
        Some(generated.clone()),
        &source,
        &different_environment,
    )
    .expect_err("different render environments cannot be mixed");
    assert!(different.to_string().contains("render environment differs"));

    let mut legacy = generated.clone();
    legacy.schema_version = 1;
    let legacy = prepare_partial_generated_manifest(Some(legacy), &source, &environment)
        .expect_err("legacy provenance cannot be partially merged");
    assert!(legacy.to_string().contains("schema 1"));

    let accepted = prepare_partial_generated_manifest(Some(generated), &source, &environment)
        .expect("the exact generated environment can extend its own corpus");
    assert!(!accepted.complete);
    assert_eq!(
        accepted.attestation.render_environment(),
        Some(&environment)
    );
}

#[test]
fn filtered_generation_merges_manifest_state_without_losing_other_fixtures() {
    let root = TestDir::new("filtered-merge");
    let fixtures_dir = root.path().join("fixtures");
    let out_dir = root.path().join("upstream");
    let kept_fixture = write_fixture(&fixtures_dir, "kept");
    let generated_fixture = write_fixture(&fixtures_dir, "newly-generated");
    let excluded_fixture = write_fixture(&fixtures_dir, "newly-excluded_parser_only_spec");
    let kept_svg = write_svg(&out_dir, "kept", &upstream_svg_id("kept"), true);
    let generated_svg = write_svg(
        &out_dir,
        "newly-generated",
        &upstream_svg_id("newly-generated"),
        true,
    );
    let excluded_svg = write_svg(
        &out_dir,
        "newly-excluded_parser_only_spec",
        &upstream_svg_id("newly-excluded_parser_only_spec"),
        true,
    );
    let source = current_source().expect("read pinned source");
    let environment = test_render_environment();
    let profile = renderer_profile("sequence").to_string();
    let kept_entry = UpstreamSvgFixtureProvenance {
        input_sha256: hash_file(&kept_fixture).expect("hash kept fixture"),
        svg_sha256: hash_file(&kept_svg).expect("hash kept SVG"),
        renderer_profile: profile.clone(),
    };
    let mut existing = UpstreamSvgManifest::empty(
        source.clone(),
        UpstreamSvgAttestation::generated(environment.clone()),
    );
    existing.complete = true;
    existing
        .fixtures
        .insert("kept".to_string(), kept_entry.clone());
    existing.fixtures.insert(
        "newly-excluded_parser_only_spec".to_string(),
        UpstreamSvgFixtureProvenance {
            input_sha256: hash_file(&excluded_fixture).expect("hash excluded fixture"),
            svg_sha256: hash_file(&excluded_svg).expect("hash excluded SVG"),
            renderer_profile: profile,
        },
    );
    existing.excluded.insert(
        "newly-generated".to_string(),
        UpstreamSvgExcludedFixture {
            input_sha256: hash_file(&generated_fixture).expect("hash generated fixture"),
            reason: "old exclusion".to_string(),
        },
    );
    write_manifest(&out_dir, &existing).expect("write existing manifest");
    fs::remove_file(&excluded_svg).expect("stage excluded SVG deletion");
    let mut snapshots = capture_upstream_svg_fixture_selection(
        &root.path().join("staging"),
        "sequence",
        &fixtures_dir,
        Some("newly-"),
    )
    .expect("capture filtered fixture evidence");

    write_upstream_svg_provenance(
        UpstreamSvgProvenanceWriteRequest {
            diagram: "sequence",
            fixtures_dir: &fixtures_dir,
            out_dir: &out_dir,
            generated_fixtures: snapshots.renderable(),
            excluded_fixtures: snapshots.excluded(),
            full_generation: false,
            fresh_output: false,
            render_environment: environment.clone(),
        },
        || snapshots.validate_live_selection_and_hashes(),
    )
    .expect("merge filtered provenance");

    let merged = read_manifest(&out_dir.join(MANIFEST_FILE_NAME))
        .expect("read merged manifest")
        .expect("merged manifest exists");
    assert!(merged.complete);
    assert_eq!(merged.source, source);
    assert_eq!(merged.attestation.render_environment(), Some(&environment));
    assert_eq!(merged.fixtures.get("kept"), Some(&kept_entry));
    let generated_svg_sha256 = hash_file(&generated_svg).expect("hash generated SVG");
    assert_eq!(
        merged
            .fixtures
            .get("newly-generated")
            .map(|entry| entry.svg_sha256.as_str()),
        Some(generated_svg_sha256.as_str())
    );
    assert!(!merged.excluded.contains_key("newly-generated"));
    assert!(
        !merged
            .fixtures
            .contains_key("newly-excluded_parser_only_spec")
    );
    assert_eq!(
        merged
            .excluded
            .get("newly-excluded_parser_only_spec")
            .map(|entry| entry.reason.as_str()),
        Some(PARSER_ONLY_EXCLUSION_REASON)
    );
    assert!(!excluded_svg.exists());
    snapshots.cleanup().expect("clean fixture snapshots");
}

#[test]
fn fresh_filtered_generation_writes_a_new_generated_manifest() {
    let root = TestDir::new("fresh-filtered");
    let fixtures_dir = root.path().join("fixtures");
    let out_dir = root.path().join("upstream");
    write_fixture(&fixtures_dir, "only");
    write_svg(&out_dir, "only", &upstream_svg_id("only"), true);
    let environment = test_render_environment();
    let mut snapshots = capture_upstream_svg_fixture_selection(
        &root.path().join("staging"),
        "sequence",
        &fixtures_dir,
        Some("only"),
    )
    .expect("capture filtered fixture evidence");

    write_upstream_svg_provenance(
        UpstreamSvgProvenanceWriteRequest {
            diagram: "sequence",
            fixtures_dir: &fixtures_dir,
            out_dir: &out_dir,
            generated_fixtures: snapshots.renderable(),
            excluded_fixtures: snapshots.excluded(),
            full_generation: false,
            fresh_output: true,
            render_environment: environment.clone(),
        },
        || snapshots.validate_live_selection_and_hashes(),
    )
    .expect("write fresh filtered provenance");

    let manifest = read_manifest(&out_dir.join(MANIFEST_FILE_NAME))
        .expect("read fresh manifest")
        .expect("fresh manifest exists");
    assert!(!manifest.complete);
    assert_eq!(manifest.fixtures.len(), 1);
    assert_eq!(
        manifest.attestation.render_environment(),
        Some(&environment)
    );
    snapshots.cleanup().expect("clean fixture snapshots");
}

#[test]
fn fixture_snapshot_keeps_rendered_bytes_and_rejects_live_drift() {
    let root = TestDir::new("fixture-snapshot-drift");
    let fixtures_dir = root.path().join("fixtures");
    let fixture_path = write_fixture(&fixtures_dir, "only");
    let original = fs::read(&fixture_path).expect("read original fixture");
    let original_sha256 = hash_bytes(&original);
    let mut snapshots = capture_upstream_svg_fixture_selection(
        &root.path().join("staging"),
        "sequence",
        &fixtures_dir,
        Some("only"),
    )
    .expect("capture fixture snapshot");
    let captured = &snapshots.renderable()[0];

    fs::write(&fixture_path, "sequenceDiagram\n  A->>B: changed\n").expect("mutate live fixture");

    assert_eq!(
        fs::read(captured.snapshot_path()).expect("read immutable snapshot"),
        original
    );
    assert_eq!(captured.input_sha256(), original_sha256);
    let error = snapshots
        .validate_live_selection_and_hashes()
        .expect_err("live fixture drift must reject the captured selection");
    assert!(error.to_string().contains("changed after snapshot capture"));
    snapshots.cleanup().expect("clean fixture snapshots");
}

#[test]
fn tampered_fixture_snapshot_is_rejected_before_provenance_install() {
    let root = TestDir::new("fixture-snapshot-tamper");
    let fixtures_dir = root.path().join("fixtures");
    let out_dir = root.path().join("upstream");
    write_fixture(&fixtures_dir, "only");
    write_svg(&out_dir, "only", &upstream_svg_id("only"), true);
    let mut snapshots = capture_upstream_svg_fixture_selection(
        &root.path().join("staging"),
        "sequence",
        &fixtures_dir,
        Some("only"),
    )
    .expect("capture fixture snapshot");
    fs::write(
        snapshots.renderable()[0].snapshot_path(),
        "sequenceDiagram\n  A->>B: tampered snapshot\n",
    )
    .expect("tamper fixture snapshot");
    let snapshot_error = snapshots
        .validate_live_selection_and_hashes()
        .expect_err("snapshot validation must reject tampered bytes");
    assert!(
        snapshot_error
            .to_string()
            .contains("fixture snapshot changed")
    );

    let error = write_upstream_svg_provenance(
        UpstreamSvgProvenanceWriteRequest {
            diagram: "sequence",
            fixtures_dir: &fixtures_dir,
            out_dir: &out_dir,
            generated_fixtures: snapshots.renderable(),
            excluded_fixtures: snapshots.excluded(),
            full_generation: false,
            fresh_output: true,
            render_environment: test_render_environment(),
        },
        || snapshots.validate_live_selection_and_hashes(),
    )
    .expect_err("tampered snapshot must not receive provenance");

    assert!(error.to_string().contains("fixture snapshot changed"));
    assert!(!out_dir.join(MANIFEST_FILE_NAME).exists());
    snapshots.cleanup().expect("clean fixture snapshots");
}

#[test]
fn generated_manifest_rejects_fixture_drift_instead_of_signing_new_input() {
    let root = TestDir::new("fixture-drift-provenance");
    let fixtures_dir = root.path().join("fixtures");
    let out_dir = root.path().join("upstream");
    let fixture_path = write_fixture(&fixtures_dir, "only");
    write_svg(&out_dir, "only", &upstream_svg_id("only"), true);
    let mut snapshots = capture_upstream_svg_fixture_selection(
        &root.path().join("staging"),
        "sequence",
        &fixtures_dir,
        Some("only"),
    )
    .expect("capture fixture snapshot");
    fs::write(&fixture_path, "sequenceDiagram\n  A->>B: changed\n").expect("mutate live fixture");

    let error = write_upstream_svg_provenance(
        UpstreamSvgProvenanceWriteRequest {
            diagram: "sequence",
            fixtures_dir: &fixtures_dir,
            out_dir: &out_dir,
            generated_fixtures: snapshots.renderable(),
            excluded_fixtures: snapshots.excluded(),
            full_generation: false,
            fresh_output: true,
            render_environment: test_render_environment(),
        },
        || snapshots.validate_live_selection_and_hashes(),
    )
    .expect_err("drifted fixture must not receive provenance");

    assert!(error.to_string().contains("changed after snapshot capture"));
    assert!(!out_dir.join(MANIFEST_FILE_NAME).exists());
    snapshots.cleanup().expect("clean fixture snapshots");
}

#[test]
fn post_install_live_drift_restores_the_previous_manifest() {
    let root = TestDir::new("post-install-live-drift");
    let fixtures_dir = root.path().join("fixtures");
    let out_dir = root.path().join("upstream");
    let fixture_path = write_fixture(&fixtures_dir, "only");
    write_svg(&out_dir, "only", &upstream_svg_id("only"), true);
    let environment = test_render_environment();
    let existing = UpstreamSvgManifest::empty(
        current_source().expect("read pinned source"),
        UpstreamSvgAttestation::generated(environment.clone()),
    );
    write_manifest(&out_dir, &existing).expect("write existing manifest");
    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    let previous_manifest = fs::read(&manifest_path).expect("read existing manifest bytes");
    let mut snapshots = capture_upstream_svg_fixture_selection(
        &root.path().join("staging"),
        "sequence",
        &fixtures_dir,
        Some("only"),
    )
    .expect("capture fixture snapshot");

    let error = write_upstream_svg_provenance(
        UpstreamSvgProvenanceWriteRequest {
            diagram: "sequence",
            fixtures_dir: &fixtures_dir,
            out_dir: &out_dir,
            generated_fixtures: snapshots.renderable(),
            excluded_fixtures: snapshots.excluded(),
            full_generation: false,
            fresh_output: false,
            render_environment: environment,
        },
        || {
            let installed = read_manifest(&manifest_path)
                .expect("read installed manifest")
                .expect("installed manifest exists");
            assert!(installed.fixtures.contains_key("only"));
            fs::write(
                &fixture_path,
                "sequenceDiagram\n  A->>B: changed after install\n",
            )
            .expect("mutate live fixture after manifest install");
            snapshots.validate_live_selection_and_hashes()
        },
    )
    .expect_err("post-install fixture drift must roll back provenance");

    assert!(error.to_string().contains("post-install validation failed"));
    assert!(error.to_string().contains("changed after snapshot capture"));
    assert_eq!(
        fs::read(&manifest_path).expect("read restored manifest"),
        previous_manifest
    );
    let transaction_residue = fs::read_dir(&out_dir)
        .expect("read output directory")
        .map(|entry| entry.expect("read output entry").file_name())
        .filter_map(|name| name.into_string().ok())
        .any(|name| name.ends_with(".tmp") || name.ends_with(".backup"));
    assert!(!transaction_residue);
    snapshots.cleanup().expect("clean fixture snapshots");
}

#[test]
fn excluded_fixture_validation_checks_hash_reason_and_svg_absence() {
    let root = TestDir::new("excluded-validation");
    let fixture_path = write_fixture(root.path(), "syntax_parser_only_spec");
    let svg_path = root.path().join("syntax_parser_only_spec.svg");
    let expected_reason = upstream_svg_fixture_exclusion_reason("sequence", &fixture_path)
        .expect("evaluate exclusion")
        .expect("parser-only fixture is excluded");
    let mut manifest = UpstreamSvgManifest::empty(
        test_source(),
        UpstreamSvgAttestation::generated(test_render_environment()),
    );
    manifest.excluded.insert(
        "syntax_parser_only_spec".to_string(),
        UpstreamSvgExcludedFixture {
            input_sha256: hash_file(&fixture_path).expect("hash excluded fixture"),
            reason: expected_reason.clone(),
        },
    );
    let mut validator = UpstreamSvgProvenanceValidator {
        diagram: "sequence".to_string(),
        manifest,
    };
    validator
        .validate_excluded_fixture(&fixture_path, &expected_reason, &svg_path)
        .expect("matching exclusion is valid");

    let original = fs::read(&fixture_path).expect("read excluded fixture");
    fs::write(&fixture_path, b"sequenceDiagram\n  A->>B: changed\n")
        .expect("mutate excluded fixture");
    let hash_error = validator
        .validate_excluded_fixture(&fixture_path, &expected_reason, &svg_path)
        .expect_err("stale excluded input hash must fail");
    assert!(hash_error.contains("hash mismatch"));
    fs::write(&fixture_path, original).expect("restore excluded fixture");

    validator
        .manifest
        .excluded
        .get_mut("syntax_parser_only_spec")
        .expect("excluded entry")
        .reason = "stale reason".to_string();
    let reason_error = validator
        .validate_excluded_fixture(&fixture_path, &expected_reason, &svg_path)
        .expect_err("stale exclusion reason must fail");
    assert!(reason_error.contains("reason mismatch"));
    validator
        .manifest
        .excluded
        .get_mut("syntax_parser_only_spec")
        .expect("excluded entry")
        .reason = expected_reason.clone();

    fs::write(&svg_path, b"<svg/>").expect("write excluded SVG residue");
    let residue_error = validator
        .validate_excluded_fixture(&fixture_path, &expected_reason, &svg_path)
        .expect_err("excluded SVG residue must fail");
    assert!(residue_error.contains("SVG residue"));
}

#[test]
fn locked_preflight_does_not_observe_the_manifest_install_window() {
    let root = TestDir::new("locked-preflight");
    let out_dir = root.path().join("upstream");
    fs::create_dir_all(&out_dir).expect("create upstream directory");
    let environment = test_render_environment();
    let manifest = UpstreamSvgManifest::empty(
        current_source().expect("read pinned source"),
        UpstreamSvgAttestation::generated(environment.clone()),
    );
    write_manifest(&out_dir, &manifest).expect("write generated manifest");
    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    let backup_path = out_dir.join("manifest-install-window.backup");
    let writer_lock = acquire_upstream_svg_family_lock(&out_dir).expect("hold writer lock");
    fs::rename(&manifest_path, &backup_path).expect("open manifest install window");

    let blocked = preflight_upstream_svg_provenance_write_with_lock_timeout(
        &out_dir,
        false,
        false,
        &environment,
        Duration::from_millis(50),
    )
    .expect_err("preflight must wait for the writer lock");

    assert!(
        blocked.to_string().contains("timed out waiting"),
        "{blocked}"
    );
    assert!(
        !blocked
            .to_string()
            .contains("without an existing generated manifest"),
        "{blocked}"
    );
    fs::rename(&backup_path, &manifest_path).expect("close manifest install window");
    drop(writer_lock);
    assert_eq!(
        preflight_upstream_svg_provenance_write(&out_dir, false, false, &environment,)
            .expect("preflight succeeds after writer commit"),
        UpstreamSvgProvenanceWriteScope::Requested
    );
}

#[test]
fn adopted_partial_preflight_requires_a_complete_generation() {
    let root = TestDir::new("adopted-preflight-upgrade");
    let out_dir = root.path().join("upstream");
    fs::create_dir_all(&out_dir).expect("create upstream directory");
    let manifest = UpstreamSvgManifest::empty(
        current_source().expect("read pinned source"),
        UpstreamSvgAttestation::adopted_existing(),
    );
    write_manifest(&out_dir, &manifest).expect("write adopted manifest");

    assert_eq!(
        preflight_upstream_svg_provenance_write(
            &out_dir,
            false,
            false,
            &test_render_environment(),
        )
        .expect("adopted provenance requests a full generation"),
        UpstreamSvgProvenanceWriteScope::CompleteGenerationRequired
    );
}

#[test]
fn legacy_schema_cannot_be_rewritten_as_schema_v2() {
    let root = TestDir::new("legacy-schema-write");
    let mut manifest =
        UpstreamSvgManifest::empty(test_source(), UpstreamSvgAttestation::adopted_existing());
    manifest.schema_version = 1;

    let err = write_manifest(root.path(), &manifest)
        .expect_err("legacy provenance must be explicitly rebuilt or adopted");
    assert!(err.to_string().contains("schema 1"));
    assert!(!root.path().join(MANIFEST_FILE_NAME).exists());
}

#[test]
fn complete_adoption_requires_the_1116_global_style_marker() {
    let root = TestDir::new("missing-marker");
    let fixtures_dir = root.path().join("fixtures");
    let upstream_dir = root.path().join("upstream");
    write_fixture(&fixtures_dir, "basic");
    write_svg(&upstream_dir, "basic", "basic", false);

    let err = build_complete_manifest_with_source(
        "probe",
        &fixtures_dir,
        &upstream_dir,
        test_source(),
        test_attestation(UpstreamSvgAttestationMode::AdoptedExisting),
    )
    .expect_err("11.15-style SVG must not be adopted");

    assert!(
        err.to_string()
            .contains("Mermaid 11.16 global style marker")
    );
}

#[test]
fn complete_adoption_requires_the_expected_svg_root_id() {
    let root = TestDir::new("wrong-root-id");
    let fixtures_dir = root.path().join("fixtures");
    let upstream_dir = root.path().join("upstream");
    write_fixture(&fixtures_dir, "basic");
    write_svg(&upstream_dir, "basic", "wrong", true);

    let err = build_complete_manifest_with_source(
        "probe",
        &fixtures_dir,
        &upstream_dir,
        test_source(),
        test_attestation(UpstreamSvgAttestationMode::AdoptedExisting),
    )
    .expect_err("wrong SVG root id must not be adopted");

    assert!(err.to_string().contains("root id"));
}

#[test]
fn complete_adoption_records_parser_only_fixtures_as_explicit_exclusions() {
    let root = TestDir::new("parser-only-exclusion");
    let fixtures_dir = root.path().join("fixtures");
    let upstream_dir = root.path().join("upstream");
    write_fixture(&fixtures_dir, "basic");
    write_fixture(&fixtures_dir, "syntax_parser_only_spec");
    write_svg(&upstream_dir, "basic", "basic", true);

    let manifest = build_complete_manifest_with_source(
        "probe",
        &fixtures_dir,
        &upstream_dir,
        test_source(),
        test_attestation(UpstreamSvgAttestationMode::AdoptedExisting),
    )
    .expect("complete corpus is adoptable");

    assert!(manifest.complete);
    assert_eq!(
        manifest.attestation.mode(),
        UpstreamSvgAttestationMode::AdoptedExisting
    );
    assert_eq!(manifest.fixtures.len(), 1);
    assert!(manifest.fixtures.contains_key("basic"));
    assert!(manifest.excluded.contains_key("syntax_parser_only_spec"));
}

#[test]
fn complete_adoption_rejects_an_svg_for_an_excluded_fixture() {
    let root = TestDir::new("excluded-svg");
    let fixtures_dir = root.path().join("fixtures");
    let upstream_dir = root.path().join("upstream");
    write_fixture(&fixtures_dir, "basic");
    write_fixture(&fixtures_dir, "syntax_parser_only_spec");
    write_svg(&upstream_dir, "basic", "basic", true);
    write_svg(
        &upstream_dir,
        "syntax_parser_only_spec",
        "syntax_parser_only_spec",
        true,
    );

    let err = build_complete_manifest_with_source(
        "probe",
        &fixtures_dir,
        &upstream_dir,
        test_source(),
        test_attestation(UpstreamSvgAttestationMode::AdoptedExisting),
    )
    .expect_err("excluded SVG residue must be rejected");

    assert!(err.to_string().contains("excluded fixture SVG"));
}

#[test]
fn parser_only_fixture_uses_the_shared_generation_exclusion_policy() {
    let root = TestDir::new("parser-only-policy");
    let fixture = write_fixture(root.path(), "syntax_parser_only_spec");

    let reason = upstream_svg_fixture_exclusion_reason("probe", &fixture)
        .expect("evaluate fixture exclusion")
        .expect("parser-only fixture must be excluded");

    assert_eq!(reason, PARSER_ONLY_EXCLUSION_REASON);
}

#[test]
fn complete_corpus_ignores_svg_files_in_the_out_root_staging_directory() {
    let root = TestDir::new("out-root-staging");
    let fixtures_dir = root.path().join("fixtures/probe");
    let upstream_root = root.path().join("upstream");
    let upstream_dir = upstream_root.join("probe");
    write_fixture(&fixtures_dir, "basic");
    write_svg(&upstream_dir, "basic", "basic", true);
    let staging_dir = upstream_root.join(".xtask-upstream-svg-staging/probe");
    write_svg(&staging_dir, "crashed-render.tmp", "temporary", true);

    let manifest = build_complete_manifest_with_source(
        "probe",
        &fixtures_dir,
        &upstream_dir,
        test_source(),
        test_attestation(UpstreamSvgAttestationMode::AdoptedExisting),
    )
    .expect("out-root staging artifacts must not enter the family corpus");

    assert_eq!(manifest.fixtures.len(), 1);
    assert!(manifest.fixtures.contains_key("basic"));
}

#[test]
fn complete_adoption_rejects_missing_and_extra_svg_stems() {
    let root = TestDir::new("set-mismatch");
    let fixtures_dir = root.path().join("fixtures");
    let upstream_dir = root.path().join("upstream");
    write_fixture(&fixtures_dir, "missing");
    write_svg(&upstream_dir, "extra", "extra", true);

    let err = build_complete_manifest_with_source(
        "probe",
        &fixtures_dir,
        &upstream_dir,
        test_source(),
        test_attestation(UpstreamSvgAttestationMode::AdoptedExisting),
    )
    .expect_err("missing and extra SVG stems must be rejected");

    let message = err.to_string();
    assert!(message.contains("missing=[missing]"), "{message}");
    assert!(message.contains("extra=[extra]"), "{message}");
}

#[test]
fn case_insensitive_stem_collisions_are_rejected() {
    let mut seen = BTreeMap::new();
    register_case_insensitive_stem(&mut seen, "Example", "fixture", "probe")
        .expect("first stem is accepted");

    let err = register_case_insensitive_stem(&mut seen, "example", "fixture", "probe")
        .expect_err("case-only collision must be rejected");

    assert!(
        err.to_string()
            .contains("case-insensitive fixture stem collision")
    );
}

#[test]
fn all_adoption_validates_every_family_before_writing_any_manifest() {
    let root = TestDir::new("all-atomic-validation");
    let fixtures_root = root.path().join("fixtures");
    let upstream_root = root.path().join("upstream");

    for family in ["good", "bad"] {
        write_fixture(&fixtures_root.join(family), "basic");
    }
    write_svg(&upstream_root.join("good"), "basic", "basic", true);
    write_svg(&upstream_root.join("bad"), "basic", "basic", false);

    let err = adopt_existing_manifests_with_source(
        &["good", "bad"],
        &fixtures_root,
        &upstream_root,
        test_source(),
        false,
    )
    .expect_err("one invalid family rejects the whole adoption batch");

    assert!(
        err.to_string()
            .contains("Mermaid 11.16 global style marker")
    );
    assert!(!upstream_root.join("good").join(MANIFEST_FILE_NAME).exists());
    assert!(!upstream_root.join("bad").join(MANIFEST_FILE_NAME).exists());
}

#[test]
fn all_adoption_rolls_back_every_manifest_when_a_later_install_fails() {
    let root = TestDir::new("all-atomic-install");
    let first_dir = root.path().join("first");
    let second_dir = root.path().join("second");
    fs::create_dir_all(&first_dir).expect("create first family");
    fs::create_dir_all(&second_dir).expect("create second family");
    let first_path = first_dir.join(MANIFEST_FILE_NAME);
    let second_path = second_dir.join(MANIFEST_FILE_NAME);
    fs::write(&first_path, b"first-old\n").expect("write first old manifest");
    fs::write(&second_path, b"second-old\n").expect("write second old manifest");

    let first_manifest =
        UpstreamSvgManifest::empty(test_source(), UpstreamSvgAttestation::adopted_existing());
    let second_manifest =
        UpstreamSvgManifest::empty(test_source(), UpstreamSvgAttestation::adopted_existing());
    let writes = [
        (first_dir.as_path(), &first_manifest),
        (second_dir.as_path(), &second_manifest),
    ];
    let mut install_count = 0usize;
    let err = write_manifest_batch_with_installer(&writes, |from, to| {
        install_count += 1;
        if install_count == 2 {
            return Err(std::io::Error::other("injected second install failure"));
        }
        fs::rename(from, to)
    })
    .expect_err("a later install failure must reject the whole adoption batch");

    assert!(
        err.to_string().contains("injected second install failure"),
        "{err}"
    );
    assert_eq!(
        fs::read(&first_path).expect("read restored first"),
        b"first-old\n"
    );
    assert_eq!(
        fs::read(&second_path).expect("read restored second"),
        b"second-old\n"
    );
    for dir in [&first_dir, &second_dir] {
        let entries: Vec<_> = fs::read_dir(dir)
            .expect("read family directory")
            .map(|entry| entry.expect("read family entry").file_name())
            .collect();
        assert_eq!(entries, vec![MANIFEST_FILE_NAME]);
    }
}

#[test]
fn adoption_uses_the_same_family_lock_as_generation() {
    let root = TestDir::new("adoption-family-lock");
    let fixtures_root = root.path().join("fixtures");
    let upstream_root = root.path().join("upstream");
    let fixtures_dir = fixtures_root.join("probe");
    let upstream_dir = upstream_root.join("probe");
    write_fixture(&fixtures_dir, "basic");
    write_svg(&upstream_dir, "basic", "basic", true);
    let generation_lock =
        acquire_upstream_svg_family_lock(&upstream_dir).expect("hold generation family lock");

    let blocked = adopt_existing_manifests_with_source_and_lock_timeout(
        &["probe"],
        &fixtures_root,
        &upstream_root,
        test_source(),
        false,
        Duration::from_millis(50),
    )
    .expect_err("adoption must wait for the generation transaction");

    assert!(
        blocked.to_string().contains("timed out waiting"),
        "{blocked}"
    );
    assert!(!upstream_dir.join(MANIFEST_FILE_NAME).exists());
    drop(generation_lock);
    adopt_existing_manifests_with_source(
        &["probe"],
        &fixtures_root,
        &upstream_root,
        test_source(),
        false,
    )
    .expect("adoption should proceed after generation releases the lock");
}

#[test]
fn batch_family_lock_paths_are_canonicalized_sorted_and_deduplicated() {
    let root = TestDir::new("ordered-family-locks");
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(&first).expect("create first family");
    fs::create_dir_all(&second).expect("create second family");
    let first_alias = first.join(".");

    let locks = acquire_upstream_svg_family_locks_with_timeout(
        &[second, first_alias, first],
        Duration::from_secs(1),
    )
    .expect("canonical duplicate paths must not self-deadlock");

    assert_eq!(locks.len(), 2);
}

#[test]
fn existing_manifest_validation_reads_and_checks_the_manifest() {
    let root = TestDir::new("existing-validation");
    let fixtures_root = root.path().join("fixtures");
    let upstream_root = root.path().join("upstream");
    let fixtures_dir = fixtures_root.join("probe");
    let upstream_dir = upstream_root.join("probe");
    write_fixture(&fixtures_dir, "basic");
    let svg_path = write_svg(&upstream_dir, "basic", "basic", true);
    let render_environment = test_render_environment();
    let manifest = build_complete_manifest_with_source(
        "probe",
        &fixtures_dir,
        &upstream_dir,
        test_source(),
        UpstreamSvgAttestation::generated(render_environment.clone()),
    )
    .expect("generated manifest is valid");
    assert_eq!(
        manifest.attestation.render_environment(),
        Some(&render_environment)
    );
    write_manifest(&upstream_dir, &manifest).expect("write manifest");

    let round_tripped = read_manifest(&upstream_dir.join(MANIFEST_FILE_NAME))
        .expect("read manifest")
        .expect("manifest exists");
    assert_eq!(round_tripped.attestation, manifest.attestation);

    validate_existing_manifests_with_source(
        &["probe"],
        &fixtures_root,
        &upstream_root,
        test_source(),
    )
    .expect("existing manifest validates");

    let mutated = fs::read_to_string(&svg_path)
        .expect("read SVG")
        .replace("<g/>", "<g><path/></g>");
    fs::write(&svg_path, mutated).expect("mutate SVG");
    let err = validate_existing_manifests_with_source(
        &["probe"],
        &fixtures_root,
        &upstream_root,
        test_source(),
    )
    .expect_err("manifest drift must fail validation");
    assert!(err.to_string().contains("manifest drifted"));
}

#[test]
fn adoption_requires_explicit_generated_attestation_downgrade() {
    let root = TestDir::new("generated-downgrade");
    let fixtures_root = root.path().join("fixtures");
    let upstream_root = root.path().join("upstream");
    let fixtures_dir = fixtures_root.join("probe");
    let upstream_dir = upstream_root.join("probe");
    write_fixture(&fixtures_dir, "basic");
    write_svg(&upstream_dir, "basic", "basic", true);
    let generated = build_complete_manifest_with_source(
        "probe",
        &fixtures_dir,
        &upstream_dir,
        test_source(),
        test_attestation(UpstreamSvgAttestationMode::Generated),
    )
    .expect("generated manifest is valid");
    write_manifest(&upstream_dir, &generated).expect("write generated manifest");

    let err = adopt_existing_manifests_with_source(
        &["probe"],
        &fixtures_root,
        &upstream_root,
        test_source(),
        false,
    );
    let err = err.expect_err("implicit downgrade must fail").to_string();
    assert!(err.contains("--allow-downgrade"), "{err}");
    assert_eq!(
        read_manifest(&upstream_dir.join(MANIFEST_FILE_NAME))
            .expect("read manifest")
            .expect("manifest exists")
            .attestation
            .mode(),
        UpstreamSvgAttestationMode::Generated
    );

    adopt_existing_manifests_with_source(
        &["probe"],
        &fixtures_root,
        &upstream_root,
        test_source(),
        true,
    )
    .expect("explicit downgrade is allowed");
    assert_eq!(
        read_manifest(&upstream_dir.join(MANIFEST_FILE_NAME))
            .expect("read manifest")
            .expect("manifest exists")
            .attestation
            .mode(),
        UpstreamSvgAttestationMode::AdoptedExisting
    );
}

#[test]
fn explicit_adoption_migrates_a_legacy_manifest_without_attestation() {
    let root = TestDir::new("legacy-migration");
    let fixtures_root = root.path().join("fixtures");
    let upstream_root = root.path().join("upstream");
    let fixtures_dir = fixtures_root.join("probe");
    let upstream_dir = upstream_root.join("probe");
    write_fixture(&fixtures_dir, "basic");
    write_svg(&upstream_dir, "basic", "basic", true);
    fs::write(
        upstream_dir.join(MANIFEST_FILE_NAME),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "source": test_source(),
            "complete": true,
            "fixtures": {},
            "excluded": {}
        }))
        .expect("serialize legacy manifest"),
    )
    .expect("write legacy manifest");

    adopt_existing_manifests_with_source(
        &["probe"],
        &fixtures_root,
        &upstream_root,
        test_source(),
        true,
    )
    .expect("explicit adoption may replace unprovable legacy metadata");

    let migrated = read_manifest(&upstream_dir.join(MANIFEST_FILE_NAME))
        .expect("read migrated manifest")
        .expect("migrated manifest exists");
    assert_eq!(migrated.schema_version, MANIFEST_SCHEMA_VERSION);
    assert!(migrated.complete);
    assert_eq!(
        migrated.attestation.mode(),
        UpstreamSvgAttestationMode::AdoptedExisting
    );
    assert!(migrated.fixtures.contains_key("basic"));
}
