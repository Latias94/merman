use crate::config::{
    ConfigOverlayContribution, ConfigOverlayError, ConfigOverlayField, PostDetectionConfigOverlay,
    PostDetectionConfigOverlayProvider, ThemeParseBinding,
};
use crate::*;
use futures::executor::block_on;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::{Arc, Mutex};

fn test_detector_always_matches(_text: &str, _config: &mut MermaidConfig) -> bool {
    true
}

fn test_detector_deep_merges_empty_patch(_text: &str, config: &mut MermaidConfig) -> bool {
    config.deep_merge(&json!({}));
    true
}

fn test_detector_deep_merges_unrelated_nested_patch(
    _text: &str,
    config: &mut MermaidConfig,
) -> bool {
    config.deep_merge(&json!({
        "detectorState": {
            "nested": {
                "selected": true
            }
        }
    }));
    true
}

fn test_detector_deep_merges_ignored_type_conflict(
    _text: &str,
    config: &mut MermaidConfig,
) -> bool {
    config.deep_merge(&json!({ "flowchart": "ignored" }));
    true
}

fn test_detector_deep_merges_depth_boundary_value(_text: &str, config: &mut MermaidConfig) -> bool {
    config.deep_merge(&json!({
        "flowchart": {
            "subGraphTitleMargin": {
                "top": 0
            }
        }
    }));
    true
}

#[test]
fn canonical_catalog_detects_mindmap() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("mindmap\n  root")).unwrap();
    assert_eq!(res.diagram_type, "mindmap");
}

#[test]
fn canonical_catalog_detects_flowchart_elk_and_sets_layout() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("flowchart-elk TD\nA-->B")).unwrap();
    assert_eq!(res.diagram_type, "flowchart-elk");
    assert_eq!(res.effective_config.get_str("layout"), Some("elk"));
}

fn flowchart_overlay(path: &str, value: Value) -> PostDetectionConfigOverlay {
    family_overlay(
        "flowchart",
        &format!("legacy.flowchart.{path}"),
        path,
        value,
    )
}

fn family_overlay(
    family: &str,
    contribution_id: &str,
    path: &str,
    value: Value,
) -> PostDetectionConfigOverlay {
    let mut patch = MermaidConfig::empty_object();
    patch.set_value(path, value);
    PostDetectionConfigOverlay::new()
        .with_family_contribution(
            family,
            ConfigOverlayContribution::new(contribution_id, patch).unwrap(),
        )
        .unwrap()
}

#[derive(Debug)]
struct RecordingOverlayProvider {
    calls: Arc<Mutex<Vec<String>>>,
    overlays: BTreeMap<String, Arc<PostDetectionConfigOverlay>>,
}

impl RecordingOverlayProvider {
    fn new(
        calls: Arc<Mutex<Vec<String>>>,
        overlays: impl IntoIterator<Item = (&'static str, PostDetectionConfigOverlay)>,
    ) -> Self {
        Self {
            calls,
            overlays: overlays
                .into_iter()
                .map(|(family, overlay)| (family.to_string(), Arc::new(overlay)))
                .collect(),
        }
    }
}

impl PostDetectionConfigOverlayProvider for RecordingOverlayProvider {
    fn overlay_for_family(
        &self,
        family: &str,
        control: &ParseControl,
    ) -> ParseControlResult<Option<Arc<PostDetectionConfigOverlay>>> {
        control.checkpoint()?;
        self.calls.lock().unwrap().push(family.to_string());
        Ok(self.overlays.get(family).cloned())
    }
}

#[test]
fn post_detection_overlay_is_family_local_for_detected_and_known_type_parses() {
    let overlay = flowchart_overlay("flowchart.nodeSpacing", json!(91));
    let engine = Engine::new().with_post_detection_config_overlay(overlay);

    let detected = engine
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("detect flowchart");
    let known = engine
        .parse_metadata_with_type_sync("flowchart-v2", "flowchart TD\nA-->B")
        .expect("parse known flowchart");
    let sequence = engine
        .parse_metadata_sync("sequenceDiagram\nAlice->>Bob: Hi")
        .expect("detect sequence");

    assert_eq!(
        detected.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(91)
    );
    assert_eq!(
        known.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(91)
    );
    assert!(
        detected
            .config_overlay_provenance()
            .contains("legacy.flowchart.flowchart.nodeSpacing")
    );
    assert_eq!(
        detected.config_overlay_provenance(),
        known.config_overlay_provenance()
    );
    assert_ne!(
        sequence.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(91)
    );
    assert!(sequence.config_overlay_provenance().is_empty());
}

fn engine_with_only_flowchart_detector(
    detector: fn(&str, &mut MermaidConfig) -> bool,
    overlay: PostDetectionConfigOverlay,
) -> Engine {
    let mut engine = Engine::new().with_post_detection_config_overlay(overlay);
    *engine.registry_mut() = DetectorRegistry::new();
    engine.registry_mut().add_fn("flowchart-v2", detector);
    engine
}

#[test]
fn empty_detector_deep_merge_does_not_block_family_overlay() {
    let metadata = engine_with_only_flowchart_detector(
        test_detector_deep_merges_empty_patch,
        flowchart_overlay("flowchart.nodeSpacing", json!(91)),
    )
    .parse_metadata_sync("custom diagram")
    .expect("parse through custom flowchart detector");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(91)
    );
}

#[test]
fn unrelated_nested_detector_deep_merge_does_not_block_family_overlay() {
    let metadata = engine_with_only_flowchart_detector(
        test_detector_deep_merges_unrelated_nested_patch,
        flowchart_overlay("flowchart.nodeSpacing", json!(91)),
    )
    .parse_metadata_sync("custom diagram")
    .expect("parse through custom flowchart detector");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(91)
    );
    assert_eq!(
        metadata.effective_config.as_value()["detectorState"]["nested"]["selected"],
        json!(true)
    );
}

#[test]
fn ignored_detector_deep_merge_type_conflict_does_not_block_family_overlay() {
    let metadata = engine_with_only_flowchart_detector(
        test_detector_deep_merges_ignored_type_conflict,
        flowchart_overlay("flowchart.nodeSpacing", json!(91)),
    )
    .parse_metadata_sync("custom diagram")
    .expect("parse through custom flowchart detector");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(91)
    );
}

#[test]
fn depth_boundary_detector_deep_merge_only_owns_the_touched_path() {
    let overlay = PostDetectionConfigOverlay::new()
        .with_family_contribution(
            "flowchart",
            ConfigOverlayContribution::new(
                "host.flowchart.subgraph-title-top",
                MermaidConfig::from_value(json!({
                    "flowchart": {
                        "subGraphTitleMargin": {
                            "top": 42
                        }
                    }
                })),
            )
            .unwrap(),
        )
        .unwrap()
        .with_family_contribution(
            "flowchart",
            ConfigOverlayContribution::new(
                "host.flowchart.node-spacing",
                MermaidConfig::from_value(json!({
                    "flowchart": {
                        "nodeSpacing": 91
                    }
                })),
            )
            .unwrap(),
        )
        .unwrap();
    let metadata = engine_with_only_flowchart_detector(
        test_detector_deep_merges_depth_boundary_value,
        overlay,
    )
    .parse_metadata_sync("custom diagram")
    .expect("parse through custom flowchart detector");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["subGraphTitleMargin"]["top"],
        json!(0),
        "same-value detector ownership at the depth boundary must survive"
    );
    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(91),
        "the depth-boundary mutation must not claim an unrelated sibling"
    );
}

#[test]
fn explicit_site_and_source_config_own_overlay_paths() {
    let overlay = flowchart_overlay("flowchart.nodeSpacing", json!(91));
    let site_engine = Engine::new()
        .with_post_detection_config_overlay(overlay.clone())
        .with_site_config(MermaidConfig::from_value(json!({
            "flowchart": {"nodeSpacing": 72}
        })));
    let site = site_engine
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse site override");
    assert_eq!(
        site.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(72)
    );
    assert!(site.config_overlay_provenance().is_empty());

    let source_engine = Engine::new().with_post_detection_config_overlay(overlay);
    let source = source_engine
        .parse_metadata_sync(
            "%%{init: {\"flowchart\": {\"nodeSpacing\": 33}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse source override");
    assert_eq!(
        source.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(33)
    );
    assert!(source.config_overlay_provenance().is_empty());
}

fn theme_compatibility_config() -> MermaidConfig {
    MermaidConfig::from_value(json!({
        "theme": "dark",
        "darkMode": true,
        "themeVariables": {
            "darkMode": true,
            "useGradient": true
        }
    }))
}

fn theme_binding_for(config: MermaidConfig) -> ThemeParseBinding {
    ThemeParseBinding::try_new([0x5a; 32], config).expect("valid theme parse binding")
}

fn theme_binding() -> ThemeParseBinding {
    theme_binding_for(theme_compatibility_config())
}

#[test]
fn theme_compatibility_freezes_binding_and_conceptual_field_count() {
    let parsed = Engine::new()
        .with_theme_compatibility(theme_binding())
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse theme compatibility config");

    let binding = theme_binding();
    assert_eq!(parsed.theme_parse_binding(), Some(&binding));
    assert_eq!(parsed.mermaid_compatibility_residual_count(), 3);
}

#[test]
fn theme_parse_binding_rejects_paths_outside_the_restricted_compatibility_shape() {
    for config in [
        json!({"themeCSS": ".node { fill: red; }"}),
        json!({"flowchart": {"nodeSpacing": 12}}),
        json!({"themeVariables": {"nested": {"value": true}}}),
    ] {
        assert!(ThemeParseBinding::try_new([0x5a; 32], MermaidConfig::from_value(config)).is_err());
    }
}

#[test]
fn theme_parse_binding_canonicalizes_the_two_dark_mode_paths() {
    let root = theme_binding_for(MermaidConfig::from_value(json!({"darkMode": true})));
    let variable = theme_binding_for(MermaidConfig::from_value(json!({
        "themeVariables": {"darkMode": true}
    })));

    assert_eq!(root, variable);
}

#[test]
fn frozen_binding_keeps_the_validated_input_when_theme_normalization_drops_a_field() {
    let binding = theme_binding_for(MermaidConfig::from_value(json!({
        "theme": "default",
        "themeVariables": {"cynefin": "ignored-by-assignWithDepth"}
    })));
    let parsed = Engine::new()
        .with_theme_compatibility(binding.clone())
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse compatibility config with a normalized-away field");

    assert!(parsed.effective_config.as_value()["themeVariables"]["cynefin"].is_object());
    assert_eq!(parsed.theme_parse_binding(), Some(&binding));
    assert_eq!(parsed.mermaid_compatibility_residual_count(), 1);
}

#[test]
fn base_engine_host_config_outranks_later_theme_installation() {
    for host_value in [true, false] {
        let parsed = Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {"useGradient": host_value}
            })))
            .with_theme_compatibility(theme_binding())
            .parse_metadata_sync("flowchart TD\nA-->B")
            .expect("parse base engine host override");

        assert_eq!(
            parsed.effective_config.as_value()["themeVariables"]["useGradient"],
            json!(host_value)
        );
        assert_eq!(parsed.mermaid_compatibility_residual_count(), 2);
    }
}

#[test]
fn later_site_and_source_assignments_shadow_theme_ownership_even_when_values_match() {
    let site = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": {"useGradient": true}
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse same-value site override");
    assert_eq!(site.mermaid_compatibility_residual_count(), 2);

    let source = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(MermaidConfig::from_value(json!({"secure": []})))
        .parse_metadata_sync(
            "%%{init: {\"themeVariables\": {\"useGradient\": true}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse same-value source override");
    assert_eq!(source.mermaid_compatibility_residual_count(), 2);
}

#[test]
fn dark_mode_counts_once_until_both_physical_paths_are_shadowed() {
    let only_root = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(MermaidConfig::from_value(json!({"darkMode": true})))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse root dark-mode override");
    assert_eq!(only_root.mermaid_compatibility_residual_count(), 3);

    let only_variable = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": {"darkMode": true}
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse theme-variable dark-mode override");
    assert_eq!(only_variable.mermaid_compatibility_residual_count(), 3);

    let both = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(MermaidConfig::from_value(json!({
            "darkMode": true,
            "themeVariables": {"darkMode": true}
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse both dark-mode overrides");
    assert_eq!(both.mermaid_compatibility_residual_count(), 2);
}

#[test]
fn explicit_mermaid_compatibility_outranks_legacy_fallback_overlay() {
    let parsed = Engine::new()
        .with_theme_compatibility(theme_binding_for(MermaidConfig::from_value(json!({
            "themeVariables": {"useGradient": true}
        }))))
        .with_fallback_post_detection_config_overlay(flowchart_overlay(
            "themeVariables.useGradient",
            json!(false),
        ))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse explicit Mermaid compatibility over fallback overlay");

    assert_eq!(
        parsed.effective_config.as_value()["themeVariables"]["useGradient"],
        json!(true)
    );
    assert_eq!(parsed.mermaid_compatibility_residual_count(), 1);
    assert!(parsed.config_overlay_provenance().is_empty());
}

#[test]
fn host_overlay_outranks_and_shadows_mermaid_compatibility() {
    let parsed = Engine::new()
        .with_theme_compatibility(theme_binding_for(MermaidConfig::from_value(json!({
            "themeVariables": {"useGradient": true}
        }))))
        .with_post_detection_config_overlay(flowchart_overlay(
            "themeVariables.useGradient",
            json!(false),
        ))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse host overlay over Mermaid compatibility");

    assert_eq!(
        parsed.effective_config.as_value()["themeVariables"]["useGradient"],
        json!(false)
    );
    assert_eq!(parsed.mermaid_compatibility_residual_count(), 0);
    assert!(
        parsed
            .config_overlay_provenance()
            .contains("legacy.flowchart.themeVariables.useGradient")
    );
}

#[test]
fn post_detection_overlay_rejects_render_family_selection_paths() {
    for path in ["layout", "flowchart.defaultRenderer"] {
        let mut patch = MermaidConfig::empty_object();
        patch.set_value(path, json!("elk"));
        assert_eq!(
            ConfigOverlayContribution::new(format!("invalid.{path}"), patch).unwrap_err(),
            ConfigOverlayError::InvalidValue {
                field: ConfigOverlayField::AssignmentPath
            }
        );
    }
}

#[test]
fn host_overlay_has_priority_without_replacing_theme_fallback() {
    let host = family_overlay(
        "flowchart",
        "host.flowchart.node-spacing",
        "flowchart.nodeSpacing",
        json!(50),
    );
    let fallback = family_overlay(
        "flowchart",
        "legacy.flowchart.node-spacing",
        "flowchart.nodeSpacing",
        json!(91),
    );
    let metadata = Engine::new()
        .with_post_detection_config_overlay(host)
        .with_fallback_post_detection_config_overlay(fallback)
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse flowchart with layered overlays");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(50)
    );
    assert!(
        metadata
            .config_overlay_provenance()
            .contains("host.flowchart.node-spacing")
    );
    assert!(
        !metadata
            .config_overlay_provenance()
            .contains("legacy.flowchart.node-spacing")
    );
}

#[test]
fn fallback_provider_is_called_once_only_for_the_final_render_family() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let provider = RecordingOverlayProvider::new(
        Arc::clone(&calls),
        [
            (
                "flowchart",
                family_overlay(
                    "flowchart",
                    "provider.flowchart.spacing",
                    "flowchart.nodeSpacing",
                    json!(41),
                ),
            ),
            (
                "swimlane",
                family_overlay(
                    "swimlane",
                    "provider.swimlane.spacing",
                    "flowchart.nodeSpacing",
                    json!(81),
                ),
            ),
        ],
    );
    let metadata = Engine::new()
        .with_fallback_post_detection_config_overlay_provider(provider)
        .parse_metadata_sync("%%{init: {\"layout\": \"swimlane\"}}%%\nflowchart TD\nA-->B")
        .expect("parse flowchart routed to the swimlane renderer");

    assert_eq!(calls.lock().unwrap().as_slice(), ["swimlane"]);
    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(81)
    );
    assert!(
        metadata
            .config_overlay_provenance()
            .fallback_contribution_ids()
            .eq(["provider.swimlane.spacing"])
    );
}

#[test]
fn host_overlay_keeps_priority_over_a_lazy_fallback_provider() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let provider = RecordingOverlayProvider::new(
        Arc::clone(&calls),
        [(
            "flowchart",
            family_overlay(
                "flowchart",
                "provider.flowchart.spacing",
                "flowchart.nodeSpacing",
                json!(91),
            ),
        )],
    );
    let metadata = Engine::new()
        .with_post_detection_config_overlay(family_overlay(
            "flowchart",
            "host.flowchart.spacing",
            "flowchart.nodeSpacing",
            json!(50),
        ))
        .with_fallback_post_detection_config_overlay_provider(provider)
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse flowchart with host and provider overlays");

    assert_eq!(calls.lock().unwrap().as_slice(), ["flowchart"]);
    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(50)
    );
    assert!(
        metadata
            .config_overlay_provenance()
            .contains("host.flowchart.spacing")
    );
    assert!(
        metadata
            .config_overlay_provenance()
            .fallback_contribution_ids()
            .next()
            .is_none()
    );
}

#[test]
fn static_and_provider_fallback_installers_replace_the_same_lane() {
    let provider_calls = Arc::new(Mutex::new(Vec::new()));
    let provider = RecordingOverlayProvider::new(
        Arc::clone(&provider_calls),
        [(
            "flowchart",
            family_overlay(
                "flowchart",
                "provider.flowchart.spacing",
                "flowchart.nodeSpacing",
                json!(81),
            ),
        )],
    );
    let provider_wins = Engine::new()
        .with_fallback_post_detection_config_overlay(family_overlay(
            "flowchart",
            "static.flowchart.spacing",
            "flowchart.nodeSpacing",
            json!(71),
        ))
        .with_fallback_post_detection_config_overlay_provider(provider)
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("provider replaces static fallback");
    assert_eq!(provider_calls.lock().unwrap().as_slice(), ["flowchart"]);
    assert_eq!(
        provider_wins.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(81)
    );

    let replaced_provider_calls = Arc::new(Mutex::new(Vec::new()));
    let replaced_provider = RecordingOverlayProvider::new(
        Arc::clone(&replaced_provider_calls),
        [(
            "flowchart",
            family_overlay(
                "flowchart",
                "provider.flowchart.spacing",
                "flowchart.nodeSpacing",
                json!(91),
            ),
        )],
    );
    let static_wins = Engine::new()
        .with_fallback_post_detection_config_overlay_provider(replaced_provider)
        .with_fallback_post_detection_config_overlay(family_overlay(
            "flowchart",
            "static.flowchart.spacing",
            "flowchart.nodeSpacing",
            json!(61),
        ))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("static fallback replaces provider");
    assert!(replaced_provider_calls.lock().unwrap().is_empty());
    assert_eq!(
        static_wins.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(61)
    );
}

#[test]
fn overlay_provenance_keeps_host_and_fallback_owners_distinct() {
    let shared_id = "merman.legacy-family-theme.v1.flowchart.node-spacing";
    let host = family_overlay("flowchart", shared_id, "flowchart.nodeSpacing", json!(50));
    let fallback = family_overlay("flowchart", shared_id, "flowchart.rankSpacing", json!(91));
    let metadata = Engine::new()
        .with_post_detection_config_overlay(host)
        .with_fallback_post_detection_config_overlay(fallback)
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse flowchart with colliding overlay ids");
    let provenance = metadata.config_overlay_provenance();

    assert_eq!(
        provenance.contribution_ids().collect::<Vec<_>>(),
        vec![shared_id, shared_id]
    );
    assert_eq!(
        provenance.fallback_contribution_ids().collect::<Vec<_>>(),
        vec![shared_id]
    );
}

#[test]
fn host_only_overlay_id_is_never_reported_as_fallback_provenance() {
    let shared_id = "merman.legacy-family-theme.v1.flowchart.node-spacing";
    let metadata = Engine::new()
        .with_post_detection_config_overlay(family_overlay(
            "flowchart",
            shared_id,
            "flowchart.nodeSpacing",
            json!(50),
        ))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse flowchart with host-owned compatibility-shaped id");
    let provenance = metadata.config_overlay_provenance();

    assert!(provenance.contains(shared_id));
    assert!(provenance.fallback_contribution_ids().next().is_none());
}

#[test]
fn post_detection_overlay_uses_the_configured_flowchart_render_family() {
    let overlay = PostDetectionConfigOverlay::new()
        .with_family_contribution(
            "flowchart",
            ConfigOverlayContribution::new(
                "legacy.flowchart.spacing",
                MermaidConfig::from_value(json!({"flowchart": {"nodeSpacing": 41}})),
            )
            .unwrap(),
        )
        .unwrap()
        .with_family_contribution(
            "swimlane",
            ConfigOverlayContribution::new(
                "legacy.swimlane.spacing",
                MermaidConfig::from_value(json!({"flowchart": {"nodeSpacing": 81}})),
            )
            .unwrap(),
        )
        .unwrap();
    let engine = Engine::new().with_fallback_post_detection_config_overlay(overlay);

    let configured_swimlane = engine
        .parse_metadata_sync("%%{init: {\"layout\": \"swimlane\"}}%%\nflowchart TD\nA-->B")
        .expect("parse flowchart configured for swimlane layout");
    assert_eq!(
        configured_swimlane.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(81)
    );
    assert!(
        configured_swimlane
            .config_overlay_provenance()
            .contains("legacy.swimlane.spacing")
    );

    let configured_flowchart = engine
        .parse_metadata_sync("%%{init: {\"layout\": \"elk\"}}%%\nswimlane-beta LR\nA-->B")
        .expect("parse swimlane configured for flowchart layout");
    assert_eq!(
        configured_flowchart.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(41)
    );
    assert!(
        configured_flowchart
            .config_overlay_provenance()
            .contains("legacy.flowchart.spacing")
    );
    assert!(
        !configured_flowchart
            .config_overlay_provenance()
            .contains("legacy.swimlane.spacing")
    );
}

#[test]
fn generated_defaults_preserve_mermaids_runtime_class_object_override() {
    let expected = json!({
        "hideEmptyMembersBox": false,
        "hierarchicalNamespaces": true
    });
    assert_eq!(
        crate::generated::default_site_config()
            .as_value()
            .get("class"),
        Some(&expected),
        "Mermaid 11.16 defaultConfig.ts replaces rather than spreads the schema Class object"
    );
}

#[test]
fn class_diagram_detection_keeps_runtime_default_absent_when_site_config_is_merged() {
    let engine = Engine::new().with_site_config({
        let mut cfg = MermaidConfig::empty_object();
        cfg.set_value("securityLevel", json!("sandbox"));
        cfg
    });

    let text = r#"classDiagram
class Class1
"#;
    let res = block_on(engine.parse_metadata(text)).unwrap();
    assert_eq!(res.diagram_type, "class");
}

#[test]
fn class_diagram_detection_respects_explicit_wrapper_renderer() {
    let engine = Engine::new().with_site_config({
        let mut cfg = MermaidConfig::empty_object();
        cfg.set_value("class.defaultRenderer", json!("dagre-wrapper"));
        cfg
    });

    let text = r#"classDiagram
class Class1
"#;
    let res = block_on(engine.parse_metadata(text)).unwrap();
    assert_eq!(res.diagram_type, "classDiagram");
}

#[test]
fn class_diagram_detection_respects_explicit_dagre_d3_renderer() {
    let engine = Engine::new().with_site_config({
        let mut cfg = MermaidConfig::empty_object();
        cfg.set_value("class.defaultRenderer", json!("dagre-d3"));
        cfg
    });

    let text = r#"classDiagram
class Class1
"#;
    let res = block_on(engine.parse_metadata(text)).unwrap();
    assert_eq!(res.diagram_type, "class");
}

#[test]
fn class_diagram_detection_does_not_treat_renderer_as_root_layout() {
    let engine = Engine::new().with_site_config({
        let mut cfg = MermaidConfig::empty_object();
        cfg.set_value("class.defaultRenderer", json!("elk"));
        cfg
    });

    let res = block_on(engine.parse_metadata("classDiagram\nclass Class1\n")).unwrap();

    assert_eq!(res.diagram_type, "class");
    assert_eq!(res.effective_config.get_str("layout"), Some("dagre"));
}

#[test]
fn state_diagram_detection_respects_non_default_renderer() {
    let engine = Engine::new().with_site_config({
        let mut cfg = MermaidConfig::empty_object();
        cfg.set_value("state.defaultRenderer", json!("dagre-d3"));
        cfg
    });

    let text = r#"stateDiagram
[*] --> Still
"#;
    let res = block_on(engine.parse_metadata(text)).unwrap();
    assert_eq!(res.diagram_type, "state");
}

#[test]
fn detects_tree_view_beta_as_tree_view() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("treeView-beta\n\"Root\"")).unwrap();
    assert_eq!(res.diagram_type, "treeView");
}

#[test]
fn detects_ishikawa_headers_as_ishikawa() {
    let engine = Engine::new();
    for header in ["ishikawa", "ishikawa-beta", "ISHIKAWA-BETA"] {
        let res = block_on(engine.parse_metadata(&format!("{header}\nProblem"))).unwrap();
        assert_eq!(res.diagram_type, "ishikawa");
    }
}

#[test]
fn detects_eventmodeling_as_eventmodeling() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("eventmodeling\ntf 01 evt Start")).unwrap();
    assert_eq!(res.diagram_type, "eventmodeling");
}

#[test]
fn detects_venn_beta_as_venn() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("venn-beta\nset A")).unwrap();
    assert_eq!(res.diagram_type, "venn");
}

#[test]
fn detects_11_16_new_family_headers_for_metadata() {
    let engine = Engine::new();

    for (source, expected_type) in [
        ("swimlane-beta\nA --> B", "swimlane"),
        ("cynefin-beta:\nDomain: clear", "cynefin"),
        ("railroad-beta\nA ::= B", "railroad"),
        ("RAILROAD-EBNF-BETA\nrule ::= term", "railroadEbnf"),
        ("railroad-abnf-beta\nrule = term", "railroadAbnf"),
        ("railroad-peg-beta\nrule <- term", "railroadPeg"),
        ("wardley-beta\ncomponent A", "wardley"),
    ] {
        let res = engine.parse_metadata_sync(source).unwrap();
        assert_eq!(res.diagram_type, expected_type, "source: {source:?}");
    }
}

#[test]
fn detects_11_16_new_family_headers_with_upstream_boundaries() {
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let mut config = MermaidConfig::empty_object();

    let cynefin = registry
        .detect_type_precleaned("cynefin-beta:\nClear", &mut config)
        .expect("cynefin colon boundary should match");
    assert_eq!(cynefin, "cynefin");

    let railroad_prefix = registry
        .detect_type_precleaned("railroad-betatron", &mut config)
        .expect("railroad upstream regex has no trailing boundary");
    assert_eq!(railroad_prefix, "railroad");

    let err = registry
        .detect_type_precleaned("swimlane-betatron", &mut config)
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("No diagram type detected matching given configuration"),
        "swimlane-beta uses JS word-boundary semantics: {err}"
    );
}

#[test]
fn supplied_detector_registry_preserves_upstream_first_match_order() {
    let mut registry = DetectorRegistry::new();
    registry.add_fn("host-first", test_detector_always_matches);
    registry.add_fn("host-second", test_detector_always_matches);
    let mut config = MermaidConfig::empty_object();

    // Mermaid detectType iterates the supplied detector records and returns the first match.
    let detected = registry
        .detect_type("sequenceDiagram\nA ->> B: hello", &mut config)
        .unwrap();

    assert_eq!(detected, "host-first");
}

#[test]
fn empty_detector_registry_rejects_builtin_leading_keywords() {
    let registry = DetectorRegistry::new();
    let mut config = MermaidConfig::empty_object();

    let error = registry
        .detect_type_precleaned("sequenceDiagram\nA ->> B: hello", &mut config)
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("No diagram type detected matching given configuration"),
        "unexpected error: {error}"
    );
}

#[test]
fn kanban_detector_and_known_type_parser_preserve_distinct_upstream_case_rules() {
    let engine = Engine::new();

    let detected = engine.parse_metadata_sync("kanban\nTodo[Todo]\n").unwrap();
    assert_eq!(detected.diagram_type, "kanban");

    let detection_error = engine
        .parse_metadata_sync("KaNbAn\nTodo[Todo]\n")
        .unwrap_err();
    assert!(
        detection_error
            .to_string()
            .contains("No diagram type detected matching given configuration")
    );

    let parsed = engine
        .parse_diagram_with_type_sync("kanban", "KaNbAn\nTodo[Todo]\n", ParseOptions::strict())
        .expect("known-type Kanban parser follows Jison's case-insensitive mode")
        .expect("known-type Kanban parse returns a model");
    assert_eq!(parsed.meta.diagram_type, "kanban");
}

#[test]
fn c4_detector_preserves_upstream_ungrouped_regex_shape() {
    let engine = Engine::new();

    let anchored = engine
        .parse_metadata_sync("  C4Context\nPerson(a, \"A\")")
        .unwrap();
    assert_eq!(anchored.diagram_type, "c4");

    let ungrouped_anywhere = engine.parse_metadata_sync("kanban\nC4Container").unwrap();
    assert_eq!(ungrouped_anywhere.diagram_type, "c4");

    let err = engine
        .parse_metadata_sync("not a diagram C4Context")
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("No diagram type detected matching given configuration"),
        "unexpected error: {err}"
    );
}

#[test]
fn detector_registry_strips_mermaid_comment_lines_without_regex() {
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let mut config = MermaidConfig::empty_object();

    for source in [
        "\n\n%% This is a comment\nflowchart TD\nA-->B\n",
        "    %% This is a comment\nflowchart TD\nA-->B\n",
        "flowchart TD\nA-->B\n%% This is a comment",
        "%%{init: {'theme': 'forest'}}%%\nflowchart TD\nA-->B\n",
    ] {
        let detected = registry
            .detect_type(source, &mut config)
            .expect("detect type");
        assert_eq!(detected, "flowchart-v2", "source: {source:?}");
    }
}

#[test]
fn leading_utf8_bom_is_handled_consistently_across_public_entrypoints() {
    let source = "\u{feff}flowchart TD\nA-->B\n";
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let mut config = MermaidConfig::empty_object();

    let detected = registry
        .detect_type(source, &mut config)
        .expect("detector accepts a leading UTF-8 BOM");
    assert_eq!(detected, "flowchart-v2");

    let preprocessed =
        preprocess_diagram(source, &registry).expect("preprocessor accepts a leading UTF-8 BOM");
    assert_eq!(preprocessed.code(), "flowchart TD\nA-->B\n");
    let mapped = preprocessed
        .source
        .try_map_span(SourceSpan::new(0, "flowchart".len()))
        .expect("diagram header remains exactly mapped");
    assert_eq!(&source[mapped.start..mapped.end], "flowchart");

    let parsed = Engine::new()
        .parse_diagram_sync(source, ParseOptions::strict())
        .expect("public parse accepts a leading UTF-8 BOM")
        .expect("flowchart model");
    assert_eq!(parsed.meta.diagram_type, "flowchart-v2");

    let metadata = Engine::new()
        .parse_metadata_sync("\u{feff}---\ntitle: BOM title\n---\nflowchart TD\nA-->B\n")
        .expect("BOM is removed before frontmatter extraction");
    assert_eq!(metadata.title.as_deref(), Some("BOM title"));
}

#[test]
fn malformed_directive_json_is_removed_without_rejecting_the_diagram() {
    let source = "%%{init: {\"theme\": }}%%\nflowchart TD\nA-->B\n";
    let registry = DetectorRegistry::pinned_mermaid_baseline();

    let preprocessed =
        preprocess_diagram(source, &registry).expect("invalid directive config is fail-soft");
    assert_eq!(preprocessed.code(), "flowchart TD\nA-->B\n");
    assert!(preprocessed.config.is_empty_object());

    let parsed = Engine::new()
        .parse_diagram_sync(source, ParseOptions::strict())
        .expect("invalid directive config does not reject a valid diagram")
        .expect("flowchart model");
    assert_eq!(parsed.meta.diagram_type, "flowchart-v2");
}

#[test]
fn strict_unterminated_directive_marker_truncates_like_mermaid() {
    let source = "%%{\nflowchart TD\nA-->B\n";
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let mut config = MermaidConfig::empty_object();

    assert!(registry.detect_type(source, &mut config).is_err());

    let preprocessed = preprocess_diagram(source, &registry)
        .expect("strict preprocessing truncates an unterminated directive");
    assert_eq!(preprocessed.code(), "");

    let error = Engine::new()
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap_err();
    assert!(matches!(error, crate::Error::DetectType(_)));
}

#[test]
fn lenient_unterminated_directive_marker_recovers_the_following_diagram() {
    let source = concat!(
        "%%{init: {\"config\": {\"curve\": \"linear\"}}}%%\n",
        "%%{\n",
        "flowchart TD\nA-->B\n",
    );

    let parsed = Engine::new()
        .parse_diagram_sync(source, ParseOptions::lenient())
        .expect("lenient preprocessing recovers an unterminated directive")
        .expect("flowchart model");
    assert_eq!(parsed.meta.diagram_type, "flowchart-v2");
    assert_eq!(
        parsed.meta.config.get_str("flowchart.curve"),
        Some("linear")
    );
}

#[test]
fn editor_unterminated_directive_marker_recovers_the_following_diagram() {
    let source = "%%{\nflowchart TD\nA-->B\n";

    let snapshot = Engine::new()
        .parse_diagram_snapshot_sync(source)
        .expect("editor preprocessing recovers an unterminated directive")
        .expect("flowchart snapshot");

    assert_eq!(snapshot.metadata().diagram_type, "flowchart-v2");
}

#[test]
fn preprocess_strips_mermaid_comment_at_eof_without_regex() {
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let result = preprocess_diagram("flowchart TD\nA-->B\n%% This is a comment", &registry)
        .expect("preprocess succeeds");

    assert_eq!(result.code(), "flowchart TD\nA-->B\n");
}

#[test]
fn preprocess_normalizes_crlf_without_regex() {
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let result = preprocess_diagram("flowchart TD\r\nA-->B\r%% This is a comment", &registry)
        .expect("preprocess succeeds");

    assert_eq!(result.code(), "flowchart TD\nA-->B\n");
}

#[test]
fn preprocess_encodes_entities_without_entity_regex() {
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let result = preprocess_diagram("flowchart TD\nA[#there;]\nB[#77653;]", &registry)
        .expect("preprocess succeeds");

    assert!(
        result.code().contains("A[ﬂ°there¶ß]"),
        "{:?}",
        result.code()
    );
    assert!(
        result.code().contains("B[ﬂ°°77653¶ß]"),
        "{:?}",
        result.code()
    );
}

#[test]
fn preprocess_rewrites_html_attributes_without_regex() {
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let result = preprocess_diagram(
        r#"flowchart TD
A["<span title="alpha" data-empty="">Label</span>"]
B["<é title="unchanged">Local</é>"]"#,
        &registry,
    )
    .expect("preprocess succeeds");

    assert!(
        result
            .code()
            .contains(r#"A["<span title='alpha' data-empty=''>Label</span>"]"#),
        "{:?}",
        result.code()
    );
    assert!(
        result
            .code()
            .contains(r#"B["<é title="unchanged">Local</é>"]"#),
        "{:?}",
        result.code()
    );
}

#[test]
fn detector_registry_strips_deep_frontmatter_with_small_stack() {
    const DEPTH: usize = 512;
    let mut text = String::from("---\nconfig: {\"sequence\": ");
    for idx in 0..DEPTH {
        write!(&mut text, r#"{{"k{idx}":"#).expect("write frontmatter config");
    }
    text.push_str("\"leaf\"");
    for _ in 0..DEPTH {
        text.push('}');
    }
    text.push_str("}\n---\nsequenceDiagram\nAlice->Bob: Hi\n");
    let registry = DetectorRegistry::pinned_mermaid_baseline();

    let handle = std::thread::Builder::new()
        .name("detector-deep-frontmatter-strip".to_string())
        .stack_size(64 * 1024)
        .spawn(move || {
            let mut config = MermaidConfig::empty_object();
            let detected = registry
                .detect_type(&text, &mut config)
                .expect("detect type");
            assert_eq!(detected, "sequence");
        })
        .expect("spawn detector deep frontmatter test");
    handle
        .join()
        .expect("detector frontmatter stripping should finish without stack overflow");
}

#[test]
fn detector_registry_requires_matching_frontmatter_indentation() {
    let registry = DetectorRegistry::pinned_mermaid_baseline();

    let mut config = MermaidConfig::empty_object();
    let detected = registry
        .detect_type(
            "   ---\n   title: Flow\n   ---\n   sequenceDiagram\n   Alice->Bob: Hi\n",
            &mut config,
        )
        .expect("matching indented frontmatter should be stripped before detection");
    assert_eq!(detected, "sequence");

    let mut config = MermaidConfig::empty_object();
    let detected = registry
        .detect_type(
            "   ---\ntitle: Flow\n---\nsequenceDiagram\nAlice->Bob: Hi\n",
            &mut config,
        )
        .expect("mismatched frontmatter should remain visible to the pseudo detector");
    assert_eq!(detected, "---");

    let mut config = MermaidConfig::empty_object();
    let detected = registry
        .detect_type(
            "---\ntitle: Flow\n   ---\nsequenceDiagram\nAlice->Bob: Hi\n",
            &mut config,
        )
        .expect("indented closing delimiter must not close column-zero frontmatter");
    assert_eq!(detected, "---");
}

#[test]
fn auto_detect_common_headers_with_deep_config_small_stack() {
    const DEPTH: usize = 1_024;
    let mut value = Value::String("#778899".to_string());
    for idx in (0..DEPTH).rev() {
        let mut map = serde_json::Map::new();
        map.insert(format!("k{idx}"), value);
        value = Value::Object(map);
    }
    let mut root = serde_json::Map::new();
    root.insert("retainedConfig".to_string(), value);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(Value::Object(root)));

    let handle = std::thread::Builder::new()
        .name("detect-common-headers-deep-config".to_string())
        .stack_size(128 * 1024)
        .spawn(move || {
            for (source, expected_type) in [
                ("block\n  A\n", "block"),
                ("sankey\nA,B,1\n", "sankey"),
                ("treemap\n\"A\": 1\n", "treemap"),
                ("C4Context\nPerson(a, \"A\")\n", "c4"),
            ] {
                let meta = engine.parse_metadata_sync(source).expect("parse succeeds");
                assert_eq!(meta.diagram_type, expected_type);
            }
        })
        .expect("spawn common header detect test");
    handle
        .join()
        .expect("common header detection should finish without stack overflow");
}
