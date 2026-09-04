use merman_theme_fixtures::{
    EXPECTED_OUTPUT_TARGETS, ExpectedOutputTarget, ExpectedPortabilityGrade,
    ExpectedThemeCapability, FixtureEvidenceKind, MERMAID_STYLE_PRECEDENCE_ENTRY_COUNT,
    MERMAID_STYLE_PRECEDENCE_ENTRY_IDS, MODERN_MERMAID_REFERENCE_THEME_COUNT,
    MODERN_MERMAID_REFERENCE_THEMES, MermaidStyleFamily, ReferenceBlendMode,
    ReferenceDiagramFamily, ReferenceGradientKind, ReferenceGradientRepetition,
    ReferenceThemeFacet, ReferenceThemeMechanism, ThemeFixtureCatalog,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn themes_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes")
}

#[test]
fn committed_catalog_is_hash_bound_licensed_and_complete() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");

    assert_eq!(catalog.sources().len(), 3);
    assert_eq!(catalog.assets().len(), 2);
    assert_eq!(catalog.fixtures().len(), 25);
    assert_eq!(catalog.themes().len(), MODERN_MERMAID_REFERENCE_THEME_COUNT);
    assert!(
        catalog
            .themes()
            .all(|theme| theme.source_id() == "source-modern-mermaid")
    );

    let actual_reference_names = catalog
        .themes()
        .map(|theme| theme.reference_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_reference_names,
        MODERN_MERMAID_REFERENCE_THEMES
            .into_iter()
            .collect::<BTreeSet<_>>()
    );

    for source in catalog.sources() {
        assert_eq!(source.revision().len(), 40);
        assert!(catalog.root().join(source.license_path()).is_file());
        assert!(!source.evidence().is_empty());
        assert!(
            !catalog
                .source_snapshot_text(source.id())
                .expect("read validated source snapshot")
                .is_empty()
        );
    }
    for asset in catalog.assets() {
        assert!(
            !catalog
                .asset_bytes(asset.id())
                .expect("read font bytes")
                .is_empty()
        );
        assert!(catalog.root().join(asset.license_path()).is_file());
        assert!(catalog.root().join(asset.notice_path()).is_file());
        assert_eq!(asset.sha256(), asset.source_sha256());
    }
    for fixture in catalog.fixtures() {
        assert!(
            !catalog
                .source_text(fixture.id())
                .expect("read validated fixture source")
                .is_empty()
        );
    }
}

#[test]
fn mermaid_style_precedence_tracks_selected_upstream_revision() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("upstreams")
        .join("REPOS.lock.json");
    let lock: Value = serde_json::from_str(
        &std::fs::read_to_string(lock_path).expect("read upstream lock"),
    )
    .expect("parse upstream lock");
    let selected_revision = lock
        .get("repos")
        .and_then(Value::as_object)
        .and_then(|repos| repos.get("mermaid"))
        .and_then(Value::as_object)
        .and_then(|repo| repo.get("commit"))
        .and_then(Value::as_str)
        .expect("selected Mermaid commit");

    assert_eq!(
        catalog
            .source("source-mermaid")
            .expect("Mermaid source")
            .revision(),
        selected_revision
    );
}

#[test]
fn sequence_proof_fixture_registers_the_closed_family_and_source_inventory() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let fixture = catalog
        .fixture("fixture-sequence-proof")
        .expect("Sequence proof fixture");

    assert_eq!(fixture.source_family(), ReferenceDiagramFamily::Sequence);
    assert_eq!(
        fixture.expectation().evidence_kind(),
        FixtureEvidenceKind::TypedCapability
    );
    assert!(fixture.source_reference_mechanisms().is_empty());
    let mechanisms = fixture
        .theme_input()
        .expect("Sequence typed theme input")
        .mechanisms();
    assert_eq!(mechanisms, *fixture.expectation().reference_mechanisms());
    assert_eq!(
        mechanisms,
        BTreeSet::from([
            ReferenceThemeMechanism::CanvasSolid,
            ReferenceThemeMechanism::FontStack,
            ReferenceThemeMechanism::ThemeVariables,
        ])
    );
    assert!(
        catalog
            .theme("brutalist")
            .expect("Brutalist reference theme")
            .fixture_ids()
            .contains(fixture.id())
    );

    let visible_text = fixture
        .expectation()
        .visible_text()
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        visible_text,
        BTreeSet::from([
            "Complete",
            "Poll status",
            "Processing",
            "Request accepted",
            "Service",
            "Submit request",
            "User",
        ])
    );

    let source = catalog
        .source_text(fixture.id())
        .expect("read validated Sequence source");
    for construct in [
        "actor User",
        "User->>Service: Submit request",
        "Note over User,Service: Request accepted",
        "activate Service",
        "loop Poll status",
        "deactivate Service",
    ] {
        assert!(
            source.contains(construct),
            "missing Sequence source construct `{construct}`"
        );
    }
}

#[test]
fn ordinal_palette_fixture_registers_the_brutalist_flowchart_contract() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let fixture = catalog
        .fixture("fixture-ordinal-palette")
        .expect("Flowchart ordinal palette fixture");

    assert_eq!(fixture.source_family(), ReferenceDiagramFamily::Flowchart);
    assert_eq!(
        fixture.expectation().evidence_kind(),
        FixtureEvidenceKind::TypedCapability
    );
    assert!(fixture.source_reference_mechanisms().is_empty());
    let mechanisms = fixture
        .theme_input()
        .expect("Flowchart typed theme input")
        .mechanisms();
    assert_eq!(mechanisms, *fixture.expectation().reference_mechanisms());
    assert_eq!(
        mechanisms,
        BTreeSet::from([
            ReferenceThemeMechanism::CanvasSolid,
            ReferenceThemeMechanism::FontStack,
            ReferenceThemeMechanism::NthChildSelector,
            ReferenceThemeMechanism::RoundedCorners,
            ReferenceThemeMechanism::StrokeStyling,
            ReferenceThemeMechanism::ThemeVariables,
        ])
    );
    assert_eq!(
        fixture.asset_ids(),
        &BTreeSet::from(["font-excalifont-latin".to_string()])
    );
    assert!(
        catalog
            .theme("brutalist")
            .expect("Brutalist reference theme")
            .fixture_ids()
            .contains(fixture.id())
    );
}

#[test]
fn every_theme_has_a_closed_five_target_contract() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let expected_targets = EXPECTED_OUTPUT_TARGETS.into_iter().collect::<BTreeSet<_>>();

    for theme in catalog.themes() {
        let actual_targets = theme
            .targets()
            .iter()
            .map(|target| target.target())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_targets,
            expected_targets,
            "{}",
            theme.reference_name()
        );
        assert!(
            theme
                .targets()
                .iter()
                .all(|target| !target.capabilities().is_empty())
        );
    }
}

#[test]
fn source_matrix_preserves_real_mechanisms_and_only_aurora_is_residual() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");

    let linear_light = catalog.theme("linearLight").expect("Linear Light theme");
    assert!(
        linear_light
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::DashArray)
    );
    assert!(
        !linear_light
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::RoundedCorners)
    );
    assert!(linear_light.targets().iter().all(|target| {
        target
            .capabilities()
            .contains(&merman_theme_fixtures::ExpectedThemeCapability::DashStyling)
            && !target
                .capabilities()
                .contains(&merman_theme_fixtures::ExpectedThemeCapability::RoundedGeometry)
    }));

    let cyberpunk = catalog.theme("cyberpunk").expect("Cyberpunk theme");
    assert!(
        cyberpunk
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::CanvasBlend)
    );
    assert!(
        cyberpunk
            .source_facets()
            .contains(&ReferenceThemeFacet::CanvasBlend {
                mode: ReferenceBlendMode::Screen,
            })
    );

    for reference_name in ["noir", "aurora"] {
        let theme = catalog
            .theme(reference_name)
            .expect("layered gradient theme");
        assert!(
            theme
                .source_mechanisms()
                .contains(&ReferenceThemeMechanism::CanvasLayering),
            "{reference_name}"
        );
        assert!(
            !theme
                .source_mechanisms()
                .contains(&ReferenceThemeMechanism::CanvasPattern),
            "{reference_name}"
        );
        assert!(theme.targets().iter().all(|target| {
            target
                .capabilities()
                .contains(&ExpectedThemeCapability::LayeredCanvas)
                && !target
                    .capabilities()
                    .contains(&ExpectedThemeCapability::PatternPaint)
        }));
    }
    assert!(
        cyberpunk
            .source_facets()
            .contains(&ReferenceThemeFacet::CanvasGradient {
                gradient: ReferenceGradientKind::Linear,
                repetition: ReferenceGradientRepetition::Tiled,
            })
    );
    assert!(
        cyberpunk
            .source_facets()
            .contains(&ReferenceThemeFacet::CanvasGradient {
                gradient: ReferenceGradientKind::Radial,
                repetition: ReferenceGradientRepetition::None,
            })
    );

    let glass = catalog.theme("glassmorphism").expect("Glassmorphism theme");
    assert!(
        !glass
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::BackdropFilter)
    );
    assert!(glass.targets().iter().all(|target| {
        target.grade() == ExpectedPortabilityGrade::Portable && target.residuals().is_empty()
    }));

    let win95 = catalog.theme("win95").expect("Win95 theme");
    assert!(
        win95
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::DashArray)
    );
    assert!(
        win95
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::HasSelector)
    );
    assert!(
        win95
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::NotSelector)
    );
    for family in [
        ReferenceDiagramFamily::StateDiagram,
        ReferenceDiagramFamily::ClassDiagram,
        ReferenceDiagramFamily::ErDiagram,
    ] {
        assert!(
            win95
                .source_facets()
                .contains(&ReferenceThemeFacet::SemanticSelector {
                    mechanism: ReferenceThemeMechanism::HasSelector,
                    family,
                })
        );
    }
    assert!(
        win95
            .targets()
            .iter()
            .all(|target| target.grade() == ExpectedPortabilityGrade::Portable)
    );

    let hand_drawn = catalog.theme("handDrawn").expect("Hand Drawn theme");
    assert!(
        hand_drawn
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::ExternalSvgFilterReference)
    );

    let residual_themes = catalog
        .themes()
        .filter(|theme| {
            theme
                .targets()
                .iter()
                .any(|target| !target.residuals().is_empty())
        })
        .map(|theme| theme.reference_name())
        .collect::<Vec<_>>();
    assert_eq!(residual_themes, vec!["aurora"]);

    let aurora = catalog.theme("aurora").expect("Aurora theme");
    for target in EXPECTED_OUTPUT_TARGETS {
        let expectation = aurora.target(target).expect("closed Aurora target");
        assert_eq!(expectation.grade(), ExpectedPortabilityGrade::Unverified);
        assert_eq!(expectation.residuals().len(), 1);
        assert_eq!(expectation.residuals()[0].id(), "browser-backdrop-blur");
        assert_eq!(
            expectation.residuals()[0].source_mechanism(),
            ReferenceThemeMechanism::BackdropFilter
        );
    }
}

#[test]
fn raw_css_evidence_is_separate_from_typed_capability_evidence() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let raw = catalog
        .fixture("fixture-style-precedence")
        .expect("raw source compatibility fixture");
    assert_eq!(
        raw.expectation().evidence_kind(),
        FixtureEvidenceKind::SourceCompatibility
    );
    assert!(raw.expectation().capabilities().is_empty());
    assert_eq!(
        raw.source_reference_mechanisms(),
        raw.expectation().reference_mechanisms()
    );
    assert_eq!(
        raw.source_style_evidence_ids(),
        raw.expectation().style_evidence_ids()
    );

    let typed = catalog
        .fixture("fixture-semantic-style-capabilities")
        .expect("typed semantic fixture");
    assert_eq!(
        typed.expectation().evidence_kind(),
        FixtureEvidenceKind::TypedCapability
    );
    assert!(
        typed
            .expectation()
            .outputs()
            .contains(&ExpectedOutputTarget::Jpeg)
    );
    assert_eq!(
        typed
            .theme_input()
            .expect("typed fixture input")
            .mechanisms(),
        *typed.expectation().reference_mechanisms()
    );
    assert!(raw.theme_input().is_none());
}

#[test]
fn class_diagram_encounter_order_has_both_source_backed_fixtures() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let assignment_first = catalog
        .fixture("fixture-class-assignment-before-definition")
        .expect("assignment-first fixture");
    assert_eq!(
        assignment_first.expectation().style_evidence_ids(),
        &BTreeSet::from(["class-assignment-before-definition-copy".to_string()])
    );
    assert_eq!(
        assignment_first.source_style_evidence_ids(),
        assignment_first.expectation().style_evidence_ids()
    );
    assert_eq!(
        assignment_first.source_family(),
        ReferenceDiagramFamily::ClassDiagram
    );
    assert!(
        assignment_first
            .expectation()
            .visible_text()
            .iter()
            .any(|text| text == "User")
    );
    let definition_first = catalog
        .fixture("fixture-class-definition-before-assignment")
        .expect("definition-first fixture");
    assert_eq!(
        definition_first.expectation().style_evidence_ids(),
        &BTreeSet::from([
            "class-definition-before-assignment-no-backfill".to_string(),
            "class-node-inline".to_string(),
            "class-typography-classdef-residual".to_string(),
        ])
    );
    assert_eq!(
        definition_first.source_style_evidence_ids(),
        definition_first.expectation().style_evidence_ids()
    );
    assert_eq!(
        definition_first.source_family(),
        ReferenceDiagramFamily::ClassDiagram
    );
    assert!(
        definition_first
            .expectation()
            .visible_text()
            .iter()
            .any(|text| text == "User")
    );
}

#[test]
fn typed_class_and_er_fixtures_preserve_visible_model_text() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");

    let class = catalog
        .fixture("fixture-class-semantic-capabilities")
        .expect("Class fixture");
    assert_eq!(class.source_family(), ReferenceDiagramFamily::ClassDiagram);
    let class_text = class
        .expectation()
        .visible_text()
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert!(class_text.contains("Account"));
    assert!(class_text.contains("+String name"));
    assert!(class_text.contains("+activate()"));
    assert!(class_text.contains("GenericAccount<T>"));
    assert!(class_text.contains("«service»"));
    assert!(class_text.contains("+T value"));

    let er = catalog
        .fixture("fixture-er-semantic-capabilities")
        .expect("ER fixture");
    assert_eq!(er.source_family(), ReferenceDiagramFamily::ErDiagram);
    let er_text = er
        .expectation()
        .visible_text()
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for expected in ["客户账户", "string", "name", "boolean", "active"] {
        assert!(er_text.contains(expected), "missing ER text `{expected}`");
    }
    assert!(!er_text.contains("ACCOUNT"));
}

#[test]
fn er_diagram_node_style_precedence_has_a_closed_source_witness() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let fixture = catalog
        .fixture("fixture-er-style-precedence")
        .expect("ER style precedence fixture");
    let expected = BTreeSet::from([
        "er-node-assigned".to_string(),
        "er-node-default".to_string(),
        "er-node-inline".to_string(),
        "er-typography-assigned-residual".to_string(),
        "er-typography-default-residual".to_string(),
        "er-typography-inline-residual".to_string(),
    ]);
    assert_eq!(fixture.source_family(), ReferenceDiagramFamily::ErDiagram);
    assert_eq!(fixture.source_style_evidence_ids(), &expected);
    assert_eq!(fixture.expectation().style_evidence_ids(), &expected);
    assert_eq!(
        fixture.source_style_evidence_ids(),
        fixture.expectation().style_evidence_ids()
    );
    assert!(
        fixture
            .expectation()
            .visible_text()
            .iter()
            .any(|text| text == "ACCOUNT")
    );
    assert!(
        fixture
            .expectation()
            .visible_text()
            .iter()
            .any(|text| text == "CUSTOMER")
    );
}

#[test]
fn layered_canvas_and_pattern_capabilities_are_derived_independently() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");

    let layered = catalog
        .fixture("fixture-layered-canvas")
        .and_then(|fixture| fixture.theme_input())
        .expect("layered canvas input");
    let layered_mechanisms = layered.mechanisms();
    assert!(
        layered_mechanisms.contains(&ReferenceThemeMechanism::CanvasLayering),
        "multiple paint layers require composition"
    );
    assert!(
        layered_mechanisms.contains(&ReferenceThemeMechanism::CanvasPattern),
        "the tiled layer independently requires pattern support"
    );

    let aurora = catalog
        .fixture("fixture-aurora-residual")
        .and_then(|fixture| fixture.theme_input())
        .expect("Aurora input");
    let aurora_mechanisms = aurora.mechanisms();
    assert!(aurora_mechanisms.contains(&ReferenceThemeMechanism::CanvasLayering));
    assert!(!aurora_mechanisms.contains(&ReferenceThemeMechanism::CanvasPattern));
}

#[test]
fn modern_theme_records_retain_source_positions_and_canvas_layer_counts() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let linear = catalog.theme("linearLight").expect("linearLight theme");
    assert_eq!(linear.source_line(), 18);
    assert_eq!(linear.canvas_layer_count(), 1);
    assert!(
        !linear
            .source_mechanisms()
            .contains(&ReferenceThemeMechanism::CanvasLayering)
    );

    let noir = catalog.theme("noir").expect("noir theme");
    assert_eq!(noir.source_line(), 3263);
    assert_eq!(noir.canvas_layer_count(), 3);
    assert!(
        noir.source_mechanisms()
            .contains(&ReferenceThemeMechanism::CanvasLayering)
    );
}

#[test]
fn mixed_script_fixture_is_fully_covered_by_the_two_pinned_font_slices() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let fixture = catalog
        .fixture("fixture-mixed-script-typography")
        .expect("mixed-script fixture");
    assert_eq!(
        fixture.asset_ids(),
        &BTreeSet::from([
            "font-excalifont-latin".to_string(),
            "font-xiaolai-cjk-test".to_string(),
        ])
    );

    let source = catalog
        .source_text(fixture.id())
        .expect("mixed-script source");
    assert!(source.contains("测试"));
    assert!(
        fixture
            .expectation()
            .visible_text()
            .iter()
            .any(|text| text == "测试")
    );
    assert!(
        fixture
            .expectation()
            .visible_text()
            .iter()
            .any(|text| text == "Hand drawn")
    );
    assert!(!source.contains('主'));

    let families = catalog
        .assets()
        .map(|asset| asset.family())
        .collect::<BTreeSet<_>>();
    assert_eq!(families, BTreeSet::from(["Excalifont", "Xiaolai"]));

    let xiaolai = catalog
        .font_asset("font-xiaolai-cjk-test")
        .expect("Xiaolai source slice");
    assert!(xiaolai.unicode_range().split(',').count() > 200);
    let excalidraw = catalog
        .source("source-excalidraw")
        .expect("Excalidraw provenance");
    assert!(
        excalidraw
            .evidence()
            .iter()
            .any(|entry| entry.source_path() == "packages/excalidraw/fonts/Fonts.ts")
    );
    let snapshot: Value = serde_json::from_str(
        &catalog
            .source_snapshot_text(excalidraw.id())
            .expect("Excalidraw font snapshot"),
    )
    .expect("parse Excalidraw font snapshot");
    let snapshot_font = snapshot["fonts"]
        .as_array()
        .expect("font provenance array")
        .iter()
        .find(|font| font["assetId"] == xiaolai.id())
        .expect("Xiaolai provenance record");
    assert_eq!(snapshot_font["sha256"], xiaolai.sha256());
    assert_eq!(snapshot_font["unicodeRange"], xiaolai.unicode_range());
    assert_eq!(
        snapshot_font["fixtureCodepoints"],
        serde_json::json!(["U+6D4B", "U+8BD5"])
    );
}

#[test]
fn modern_mermaid_snapshot_is_machine_readable_and_includes_filter_definitions() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    let json = catalog
        .source_snapshot_text("source-modern-mermaid")
        .expect("modern_mermaid snapshot");
    let snapshot: Value = serde_json::from_str(&json).expect("parse snapshot");
    assert_eq!(snapshot["snapshotVersion"], 3);
    assert_eq!(
        snapshot["themes"].as_array().expect("theme matrix").len(),
        MODERN_MERMAID_REFERENCE_THEME_COUNT
    );
    assert!(
        snapshot["evidenceFiles"]
            .as_array()
            .expect("evidence files")
            .iter()
            .any(|entry| entry["path"] == "src/components/Preview.tsx")
    );
}

#[test]
fn mermaid_style_precedence_is_a_closed_typed_matrix() {
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    assert_eq!(
        catalog.style_precedence().len(),
        MERMAID_STYLE_PRECEDENCE_ENTRY_COUNT
    );
    assert_eq!(
        catalog
            .style_precedence()
            .iter()
            .map(|entry| entry.family())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            MermaidStyleFamily::ClassDiagram,
            MermaidStyleFamily::ErDiagram,
            MermaidStyleFamily::Flowchart,
            MermaidStyleFamily::Global,
            MermaidStyleFamily::StateDiagram,
        ])
    );
    assert!(catalog.style_precedence().iter().all(|entry| {
        entry.rank().is_some() != entry.is_encounter_ordered()
            && !entry.evidence().is_empty()
            && !entry.note().is_empty()
    }));
    let consumed = catalog
        .fixtures()
        .filter(|fixture| {
            fixture.expectation().evidence_kind() == FixtureEvidenceKind::SourceCompatibility
        })
        .flat_map(|fixture| fixture.expectation().style_evidence_ids().iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        consumed,
        MERMAID_STYLE_PRECEDENCE_ENTRY_IDS
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
    );
}

#[test]
#[ignore = "requires MERMAN_REPO_REF_ROOT with the three pinned source checkouts"]
fn pinned_source_checkouts_match_every_manifest_hash() {
    let repo_ref = std::env::var_os("MERMAN_REPO_REF_ROOT")
        .map(PathBuf::from)
        .expect("set MERMAN_REPO_REF_ROOT");
    let catalog = ThemeFixtureCatalog::load(themes_root()).expect("load committed theme corpus");
    for (source_id, directory) in [
        ("source-mermaid", "mermaid"),
        ("source-modern-mermaid", "modern_mermaid"),
        ("source-excalidraw", "excalidraw"),
    ] {
        let checkout = repo_ref.join(directory);
        let output = Command::new("git")
            .args([
                "-C",
                checkout.to_str().expect("UTF-8 checkout"),
                "rev-parse",
                "HEAD",
            ])
            .output()
            .expect("run git rev-parse");
        assert!(output.status.success(), "{directory}");
        let revision = String::from_utf8(output.stdout)
            .expect("UTF-8 revision")
            .trim()
            .to_string();
        catalog
            .verify_source_checkout(source_id, &revision, checkout)
            .unwrap_or_else(|error| panic!("{directory}: {error}"));
    }
}
