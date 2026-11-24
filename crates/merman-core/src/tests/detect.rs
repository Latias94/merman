use crate::presentation::SourcePresentationConfig;
use crate::*;
use futures::executor::block_on;
use serde_json::{Value, json};
use std::fmt::Write;

fn test_detector_always_matches(_text: &str, _config: &mut SourcePresentationConfig) -> bool {
    true
}

fn test_detector_deep_merges_empty_patch(
    _text: &str,
    config: &mut SourcePresentationConfig,
) -> bool {
    config.deep_merge(&json!({}));
    true
}

fn test_detector_deep_merges_unrelated_nested_patch(
    _text: &str,
    config: &mut SourcePresentationConfig,
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
    config: &mut SourcePresentationConfig,
) -> bool {
    config.deep_merge(&json!({ "flowchart": "ignored" }));
    true
}

fn test_detector_deep_merges_depth_boundary_value(
    _text: &str,
    config: &mut SourcePresentationConfig,
) -> bool {
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
    let engine = MermaidEngine::new();
    let res = block_on(engine.parse_metadata("mindmap\n  root")).unwrap();
    assert_eq!(res.diagram_type, "mindmap");
}

#[test]
fn usecase_header_requires_whitespace_or_end() {
    let registry = crate::detect::DetectorRegistry::pinned_mermaid_baseline();
    for source in [
        "usecase-beta",
        "  usecase-beta\nactor A",
        "usecase-beta\tLR",
    ] {
        let detected = registry.detect_type(source, &mut SourcePresentationConfig::empty_object());
        assert_eq!(detected.expect("Usecase header"), "usecase");
    }
    for source in [
        "usecase-betaExtra",
        "usecase-beta;",
        "usecase-beta_LR",
        "Usecase-beta",
    ] {
        assert!(
            registry
                .detect_type(source, &mut SourcePresentationConfig::empty_object())
                .is_err()
        );
    }
}

#[test]
fn canonical_catalog_detects_flowchart_elk_and_sets_layout() {
    let engine = MermaidEngine::new();
    let res = block_on(engine.parse_metadata("flowchart-elk TD\nA-->B")).unwrap();
    assert_eq!(res.diagram_type, "flowchart-elk");
    assert_eq!(res.effective_config.get_str("layout"), Some("elk"));
    assert!(res.effective_config.explicit_config_owns_path("layout"));

    let known = engine
        .parse_metadata_with_type_sync("flowchart-elk", "flowchart-elk TD\nA-->B")
        .expect("parse known Flowchart ELK type");
    assert!(known.effective_config.explicit_config_owns_path("layout"));
}

#[test]
fn detector_replacement_preserves_values_without_claiming_unchanged_or_missing_paths() {
    fn replace_config(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        let mut replacement = crate::json_value::clone_value_nonrecursive(config.as_value());
        replacement["flowchart"]["rankSpacing"] = json!(73);
        replacement
            .as_object_mut()
            .unwrap()
            .remove("themeVariables");
        *config = SourcePresentationConfig::from_value(replacement);
        true
    }

    for source in ["probe", "---\nconfig:\n  theme: base\n---\nprobe"] {
        let mut engine = MermaidEngine::new();
        *engine.registry_mut() = DetectorRegistry::new();
        engine.registry_mut().add_fn("flowchart-v2", replace_config);
        let result = engine.parse_metadata_sync(source).unwrap().effective_config;
        assert_eq!(result.as_value()["flowchart"]["rankSpacing"], json!(73));
        assert_eq!(
            result.as_value()["flowchart"]["nodeSpacing"],
            crate::generated::upstream_default_config().as_value()["flowchart"]["nodeSpacing"]
        );
        assert!(!result.explicit_config_owns_path("flowchart.nodeSpacing"));
        assert!(
            result.as_value()["themeVariables"]
                .get("primaryColor")
                .is_none()
        );
        assert_eq!(result.get_str("themeVariables.previouslyMissing"), None);
        assert!(!result.explicit_config_owns_path("flowchart.rankSpacing"));
    }
}

#[test]
fn detector_same_value_setter_on_replacement_retains_explicit_ownership() {
    fn replace_then_assign(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        *config = SourcePresentationConfig::from_value(
            crate::json_value::clone_value_nonrecursive(config.as_value()),
        );
        let same = config.as_value()["flowchart"]["nodeSpacing"].clone();
        config.set_value("flowchart.nodeSpacing", same);
        true
    }
    for source in ["probe", "---\nconfig:\n  theme: base\n---\nprobe"] {
        let mut engine = MermaidEngine::new();
        *engine.registry_mut() = DetectorRegistry::new();
        engine
            .registry_mut()
            .add_fn("flowchart-v2", replace_then_assign);
        let result = engine.parse_metadata_sync(source).unwrap().effective_config;
        assert_eq!(
            result.as_value()["flowchart"]["nodeSpacing"],
            crate::generated::upstream_default_config().as_value()["flowchart"]["nodeSpacing"]
        );
        assert!(result.explicit_config_owns_path("flowchart.nodeSpacing"));
    }
}

#[test]
fn nested_parse_replacement_does_not_promote_framework_writes_to_detector_assignments() {
    fn replace_with_parsed_config(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        *config = SourcePresentationConfig::from_frozen_controlled(
            MermaidEngine::new()
                .with_site_config(SourcePresentationConfig::from_value(
                    json!({"theme": "base"}),
                ))
                .parse_metadata_with_type_sync("flowchart-v2", "graph TD\nA-->B")
                .unwrap()
                .effective_config,
            &OperationControl::new(),
        )
        .expect("a private detector fixture control cannot be cancelled");
        assert!(!config.explicit_config_owns_path("themeVariables.previouslyMissing"));
        true
    }
    for source in ["probe", "---\nconfig:\n  theme: dark\n---\nprobe"] {
        let mut engine = MermaidEngine::new().with_site_config(
            SourcePresentationConfig::from_value(json!({"theme": "base"})),
        );
        *engine.registry_mut() = DetectorRegistry::new();
        engine
            .registry_mut()
            .add_fn("flowchart-v2", replace_with_parsed_config);
        let result = engine.parse_metadata_sync(source).unwrap().effective_config;
        assert_eq!(result.get_str("themeVariables.previouslyMissing"), None);
        assert!(!result.explicit_config_owns_path("themeVariables.previouslyMissing"));
    }
}

#[test]
fn nested_detector_same_value_ownership_survives_outer_replacement() {
    fn assign_same_value(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        let value = config.as_value()["flowchart"]["nodeSpacing"].clone();
        config.set_value("flowchart.nodeSpacing", value);
        true
    }
    fn replace_with_nested_result(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        let mut inner = MermaidEngine::new();
        *inner.registry_mut() = DetectorRegistry::new();
        inner
            .registry_mut()
            .add_fn("flowchart-v2", assign_same_value);
        *config = SourcePresentationConfig::from_frozen_controlled(
            inner
                .parse_metadata_sync("inner-probe")
                .unwrap()
                .effective_config,
            &OperationControl::new(),
        )
        .expect("a private detector fixture control cannot be cancelled");
        assert!(config.explicit_config_owns_path("flowchart.nodeSpacing"));
        assert!(!config.explicit_config_owns_path("flowchart.rankSpacing"));
        true
    }

    let expected =
        crate::generated::upstream_default_config().as_value()["flowchart"]["nodeSpacing"].clone();
    for source in [
        "outer-probe",
        "---\nconfig:\n  theme: base\n---\nouter-probe",
    ] {
        let mut engine = MermaidEngine::new();
        *engine.registry_mut() = DetectorRegistry::new();
        engine
            .registry_mut()
            .add_fn("flowchart-v2", replace_with_nested_result);
        let result = engine.parse_metadata_sync(source).unwrap().effective_config;
        assert_eq!(result.as_value()["flowchart"]["nodeSpacing"], expected);
        assert!(result.explicit_config_owns_path("flowchart.nodeSpacing"));
        assert_eq!(
            result.as_value()["flowchart"]["rankSpacing"],
            crate::generated::upstream_default_config().as_value()["flowchart"]["rankSpacing"]
        );
        assert!(!result.explicit_config_owns_path("flowchart.rankSpacing"));
    }
}

#[test]
fn nested_detector_same_value_theme_owner_survives_outer_rematerialization() {
    fn assign_initialized_background(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        let background = config.as_value()["themeVariables"]["mainBkg"].clone();
        config.set_value("themeVariables.mainBkg", background);
        true
    }
    fn replace_with_nested_result(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        let mut inner = MermaidEngine::new().with_site_config(
            SourcePresentationConfig::from_value(json!({"theme": "dark"})),
        );
        *inner.registry_mut() = DetectorRegistry::new();
        inner
            .registry_mut()
            .add_fn("flowchart-v2", assign_initialized_background);
        *config = SourcePresentationConfig::from_frozen_controlled(
            inner
                .parse_metadata_sync("inner-probe")
                .unwrap()
                .effective_config,
            &OperationControl::new(),
        )
        .expect("a private detector fixture control cannot be cancelled");
        true
    }

    let initialized = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(
            json!({"theme": "dark"}),
        ))
        .parse_metadata_with_type_sync("flowchart-v2", "graph TD\nA-->B")
        .unwrap();
    let source = "%%{init: {\"flowchart\": {\"theme\": \"base\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nouter-probe";
    let rebuilt = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(
            json!({"theme": "dark"}),
        ))
        .parse_metadata_with_type_sync("flowchart-v2", source)
        .unwrap();
    assert_ne!(
        rebuilt.effective_config.get_str("themeVariables.mainBkg"),
        initialized
            .effective_config
            .get_str("themeVariables.mainBkg")
    );
    let mut engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        json!({"theme": "dark"}),
    ));
    *engine.registry_mut() = DetectorRegistry::new();
    engine
        .registry_mut()
        .add_fn("flowchart-v2", replace_with_nested_result);
    let result = engine.parse_metadata_sync(source).unwrap().effective_config;
    assert_eq!(result.get_str("theme"), Some("base"));
    assert_eq!(
        result.get_str("themeVariables.mainBkg"),
        initialized
            .effective_config
            .get_str("themeVariables.mainBkg")
    );
    assert!(result.explicit_config_owns_path("themeVariables.mainBkg"));
}

#[test]
fn replacement_transfers_explicit_metadata_without_resurrecting_site_owners() {
    fn replace(text: &str, config: &mut SourcePresentationConfig) -> bool {
        let mut value = crate::json_value::clone_value_nonrecursive(config.as_value());
        if text.trim() == "changed-probe" {
            value["flowchart"]["rankSpacing"] = json!(73);
        }
        *config = SourcePresentationConfig::from_value(value);
        true
    }
    for source in [
        "unchanged-probe",
        "changed-probe",
        "---\nconfig:\n  theme: base\n---\nunchanged-probe",
        "---\nconfig:\n  theme: base\n---\nchanged-probe",
    ] {
        let mut engine =
            MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
                "flowchart": {"rankSpacing": 50},
            })));
        *engine.registry_mut() = DetectorRegistry::new();
        engine.registry_mut().add_fn("flowchart-v2", replace);
        let result = engine.parse_metadata_sync(source).unwrap().effective_config;
        assert!(!result.explicit_config_owns_path("flowchart.rankSpacing"));
        if source.ends_with("\nchanged-probe") || source == "changed-probe" {
            assert_eq!(result.as_value()["flowchart"]["rankSpacing"], json!(73));
        } else {
            assert_eq!(result.as_value()["flowchart"]["rankSpacing"], json!(50));
        }
    }
}

fn engine_with_only_flowchart_detector(
    detector: fn(&str, &mut SourcePresentationConfig) -> bool,
) -> MermaidEngine {
    let mut engine = MermaidEngine::new();
    *engine.registry_mut() = DetectorRegistry::new();
    engine.registry_mut().add_fn("flowchart-v2", detector);
    engine
}

#[test]
fn empty_detector_deep_merge_does_not_claim_untouched_config() {
    let metadata = engine_with_only_flowchart_detector(test_detector_deep_merges_empty_patch)
        .parse_metadata_sync("custom diagram")
        .expect("parse through custom flowchart detector");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        crate::generated::upstream_default_config().as_value()["flowchart"]["nodeSpacing"]
    );
    assert!(
        !metadata
            .effective_config
            .explicit_config_owns_path("flowchart.nodeSpacing")
    );
}

#[test]
fn unrelated_nested_detector_deep_merge_claims_only_the_written_path() {
    let metadata =
        engine_with_only_flowchart_detector(test_detector_deep_merges_unrelated_nested_patch)
            .parse_metadata_sync("custom diagram")
            .expect("parse through custom flowchart detector");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        crate::generated::upstream_default_config().as_value()["flowchart"]["nodeSpacing"]
    );
    assert_eq!(
        metadata.effective_config.as_value()["detectorState"]["nested"]["selected"],
        json!(true)
    );
    assert!(
        !metadata
            .effective_config
            .explicit_config_owns_path("flowchart.nodeSpacing")
    );
    assert!(
        metadata
            .effective_config
            .explicit_config_owns_path("detectorState.nested.selected")
    );
}

#[test]
fn ignored_detector_deep_merge_type_conflict_does_not_claim_config() {
    let metadata =
        engine_with_only_flowchart_detector(test_detector_deep_merges_ignored_type_conflict)
            .parse_metadata_sync("custom diagram")
            .expect("parse through custom flowchart detector");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        crate::generated::upstream_default_config().as_value()["flowchart"]["nodeSpacing"]
    );
    assert!(
        !metadata
            .effective_config
            .explicit_config_owns_path("flowchart.nodeSpacing")
    );
}

#[test]
fn depth_boundary_detector_deep_merge_only_owns_the_touched_path() {
    let metadata =
        engine_with_only_flowchart_detector(test_detector_deep_merges_depth_boundary_value)
            .parse_metadata_sync("custom diagram")
            .expect("parse through custom flowchart detector");

    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["subGraphTitleMargin"]["top"],
        json!(0),
        "same-value detector ownership at the depth boundary must survive"
    );
    assert_eq!(
        metadata.effective_config.as_value()["flowchart"]["nodeSpacing"],
        crate::generated::upstream_default_config().as_value()["flowchart"]["nodeSpacing"],
        "the depth-boundary mutation must not claim an unrelated sibling"
    );
    assert!(
        !metadata
            .effective_config
            .explicit_config_owns_path("flowchart.nodeSpacing")
    );
    assert!(
        metadata
            .effective_config
            .explicit_config_owns_path("flowchart.subGraphTitleMargin.top")
    );
}

#[test]
fn explicit_site_and_source_config_own_surviving_paths() {
    let site_engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
            "flowchart": {"nodeSpacing": 72}
        })));
    let site = site_engine
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse site override");
    assert!(
        site.effective_config
            .explicit_config_owns_path("flowchart.nodeSpacing")
    );
    assert_eq!(
        site.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(72)
    );

    let source_engine = MermaidEngine::new();
    let source = source_engine
        .parse_metadata_sync(
            "%%{init: {\"flowchart\": {\"nodeSpacing\": 33}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse source override");
    assert!(
        source
            .effective_config
            .explicit_config_owns_path("flowchart.nodeSpacing")
    );
    assert_eq!(
        source.effective_config.as_value()["flowchart"]["nodeSpacing"],
        json!(33)
    );
}

#[test]
fn effective_config_tracks_only_high_priority_explicit_owners() {
    let metadata = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "flowchart": {"nodeSpacing": 50}
        })))
        .parse_metadata_sync(
            "%%{init: {\"flowchart\": {\"rankSpacing\": 50}}}%%\nflowchart-elk TD\nA-->B",
        )
        .expect("parse layered explicit ownership config");

    let owns = |path| metadata.effective_config.explicit_config_owns_path(path);

    for path in [
        "flowchart.nodeSpacing",
        "flowchart.rankSpacing",
        "layout",
        "flowchart",
        "flowchart.nodeSpacing.unmaterializedDescendant",
    ] {
        assert!(owns(path), "expected explicit ownership for {path}");
    }
    for path in [
        "themeVariables.useGradient",
        "themeVariables.lineColor",
        "themeVariables.mainBkg",
        "themeVariables",
        "sequence",
    ] {
        assert!(!owns(path), "unexpected explicit ownership for {path}");
    }
}

#[test]
fn direct_theme_variable_ownership_excludes_derived_values() {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "base", "themeVariables": { "primaryColor": "#345678" }
        })));
    let result = engine.parse_metadata_sync("flowchart TD\nA-->B").unwrap();
    let config = result.effective_config;
    assert_eq!(config.get_str("themeVariables.mainBkg"), Some("#345678"));
    assert!(config.explicit_config_owns_path("themeVariables.primaryColor"));
    assert!(!config.explicit_config_owns_path("themeVariables.mainBkg"));
}

#[test]
fn extended_dark_primary_color_only_recomputes_requirement_background_during_initialization() {
    for theme in ["neo-dark", "redux-dark"] {
        let site = MermaidEngine::new()
            .with_site_config(SourcePresentationConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { "primaryColor": "#123456" }
            })))
            .parse_metadata_sync("flowchart TD\nA-->B")
            .expect("parse site-owned extended dark primary color");
        assert_eq!(
            site.effective_config.as_value()["themeVariables"]["requirementBackground"],
            json!("#123456"),
            "{theme} must derive Requirement background from the site primary color"
        );

        let engine =
            MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
                "theme": theme,
                "secure": []
            })));
        let baseline = engine
            .parse_metadata_sync("flowchart TD\nA-->B")
            .expect("parse extended dark baseline");
        let source = engine
            .parse_metadata_sync(
                "%%{init: {\"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
            )
            .expect("parse source-owned extended dark primary color");
        assert_eq!(
            source.effective_config.as_value()["themeVariables"]["requirementBackground"],
            baseline.effective_config.as_value()["themeVariables"]["requirementBackground"],
            "{theme} must retain the already materialized Requirement background"
        );
        assert!(
            !source
                .effective_config
                .explicit_config_owns_path("themeVariables.requirementBackground"),
            "{theme} source-only primaryColor must not own the retained Requirement background"
        );
    }
}

#[test]
fn root_source_theme_truthiness_controls_rematerialization_after_appearance_fallback() {
    let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        json!({"theme": "base"}),
    ));
    for (source_theme, selected_theme, expected_background) in [
        (json!("base"), "base", "#123456"),
        (json!("unknown"), "base", "#123456"),
        (json!(false), "base", "#fff4dd"),
        (json!("null"), "null", "#fff4dd"),
    ] {
        let source = format!(
            "%%{{init: {{\"theme\": {source_theme}, \"themeVariables\": {{\"primaryColor\": \"#123456\"}}}}}}%%\nflowchart TD\nA-->B"
        );
        let parsed = engine
            .parse_metadata_sync(&source)
            .expect("parse a root source theme");
        assert_eq!(
            parsed.effective_config.get_str("theme"),
            Some(selected_theme)
        );
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.mainBkg"),
            Some(expected_background),
            "root source theme {source_theme} must evaluate independently of the winning appearance origin"
        );
        assert!(
            parsed
                .effective_config
                .explicit_config_owns_path("themeVariables.primaryColor")
        );
    }
}

#[test]
fn source_theme_lifecycle_matches_mermaid_update_current_config() {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "dark",
            "secure": [],
            "themeVariables": {
                "secondaryColor": "#345678"
            }
        })));
    let baseline = engine
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse materialized dark site config");

    let source = engine
        .parse_metadata_sync(
            "%%{init: {\"theme\": \"null\", \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse the null source theme sentinel");
    assert_eq!(source.effective_config.get_str("theme"), Some("null"));
    assert_eq!(
        source
            .effective_config
            .get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    for key in ["mainBkg", "nodeBkg", "requirementBackground"] {
        let path = format!("themeVariables.{key}");
        assert_eq!(
            source.effective_config.get_str(&path),
            baseline.effective_config.get_str(&path),
            "the null sentinel must retain the materialized site value for {key}"
        );
    }

    let same_theme = engine
        .parse_metadata_sync(
            "%%{init: {\"theme\": \"dark\", \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
        )
        .expect("rematerialize the selected site theme");
    let equivalent_same_theme_initialization = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "dark",
            "secure": [],
            "themeVariables": {
                "primaryColor": "#123456",
                "secondaryColor": "#345678"
            }
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("materialize equivalent dark initialization");
    assert_eq!(
        same_theme.effective_config.as_value()["themeVariables"],
        equivalent_same_theme_initialization
            .effective_config
            .as_value()["themeVariables"],
        "an explicitly selected registered source theme must rematerialize even when unchanged"
    );

    let unknown_theme = engine
        .parse_metadata_sync(
            "%%{init: {\"theme\": \"unknown-theme\", \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
        )
        .expect("rematerialize the selected theme after rejecting an unknown root source theme");
    assert_eq!(
        unknown_theme.effective_config.get_str("theme"),
        Some("dark")
    );
    assert_eq!(
        unknown_theme.effective_config.as_value()["themeVariables"],
        equivalent_same_theme_initialization
            .effective_config
            .as_value()["themeVariables"],
        "a truthy root source directive still triggers recalculation after appearance fallback"
    );

    let switched = engine
        .parse_metadata_sync(
            "%%{init: {\"theme\": \"forest\", \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
        )
        .expect("switch source theme to forest");
    let equivalent_initialization = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "forest",
            "secure": [],
            "themeVariables": {
                "primaryColor": "#123456",
                "secondaryColor": "#345678"
            }
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("materialize equivalent forest initialization");

    assert_eq!(switched.effective_config.get_str("theme"), Some("forest"));
    assert_eq!(
        switched.effective_config.as_value()["themeVariables"],
        equivalent_initialization.effective_config.as_value()["themeVariables"],
        "a registered source theme must rematerialize from raw initialize and source variables"
    );
}

#[test]
fn secure_filtered_source_theme_does_not_trigger_rematerialization() {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "dark",
            "secure": ["theme"],
            "themeVariables": {
                "secondaryColor": "#345678"
            }
        })));
    let baseline = engine
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse materialized secure site theme");
    let source = engine
        .parse_metadata_sync(
            "%%{init: {\"theme\": \"forest\", \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse source theme filtered by the secure policy");

    assert_eq!(source.effective_config.get_str("theme"), Some("dark"));
    assert_eq!(
        source
            .effective_config
            .get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    for key in ["mainBkg", "nodeBkg", "requirementBackground"] {
        let path = format!("themeVariables.{key}");
        assert_eq!(
            source.effective_config.get_str(&path),
            baseline.effective_config.get_str(&path),
            "a filtered source theme must not rematerialize {path}"
        );
    }
}

#[test]
fn scoped_source_theme_rebuilds_raw_initialization_variables() {
    let overrides = SourcePresentationConfig::from_value(json!({
        "theme": "base", "themeVariables": { "secondaryColor": "#345678" }
    }));
    let engine = MermaidEngine::new().with_site_config(overrides);
    let baseline = engine.parse_metadata_sync("flowchart TD\nA-->B").unwrap();
    assert_eq!(baseline.effective_config.get_str("theme"), Some("base"));
    let source = engine.parse_metadata_sync(
        "%%{init: {\"flowchart\": {\"theme\": \"dark\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
    ).unwrap();
    let equivalent = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "dark",
            "themeVariables": { "secondaryColor": "#345678", "primaryColor": "#123456" }
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .unwrap();
    assert_eq!(source.effective_config.get_str("theme"), Some("dark"));
    assert_eq!(
        source.effective_config.as_value()["themeVariables"],
        equivalent.effective_config.as_value()["themeVariables"]
    );
    assert!(source.effective_config.explicit_config_owns_path("theme"));
    assert_eq!(
        engine
            .parse_metadata_sync("flowchart TD\nA-->B")
            .unwrap()
            .effective_config,
        baseline.effective_config
    );
}

#[test]
fn scoped_same_value_source_theme_is_authored_unless_locked_by_site_policy() {
    let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        json!({"theme": "base"}),
    ));
    let source = "%%{init: {\"flowchart\": {\"theme\": \"base\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B";
    let authored = engine.parse_metadata_sync(source).unwrap();
    assert!(source_declares_theme(
        &authored.config,
        &authored.diagram_type
    ));
    assert_eq!(
        authored.effective_config.get_str("themeVariables.mainBkg"),
        Some("#fff4dd")
    );
    let secure = engine
        .with_site_config(SourcePresentationConfig::from_value(
            json!({"secure": ["theme"]}),
        ))
        .parse_metadata_sync(source)
        .unwrap();
    assert!(!source_declares_theme(&secure.config, &secure.diagram_type));
    assert_eq!(secure.effective_config.get_str("theme"), Some("base"));
}

#[test]
fn scoped_theme_rebuild_preserves_same_value_detector_assignments() {
    fn claim_initialized_background(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        let background = config.as_value()["themeVariables"]["mainBkg"].clone();
        config.set_value("themeVariables.mainBkg", background);
        true
    }
    fn claim_initialized_root(_text: &str, config: &mut SourcePresentationConfig) -> bool {
        let _ = config.as_value_mut();
        true
    }

    for detector in [claim_initialized_background, claim_initialized_root] {
        let engine = engine_with_only_flowchart_detector(detector).with_site_config(
            SourcePresentationConfig::from_value(json!({"theme": "dark"})),
        );
        let initialized = engine.parse_metadata_sync("custom diagram").unwrap();
        let source = engine.parse_metadata_sync(
            "%%{init: {\"flowchart\": {\"theme\": \"base\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\ncustom diagram",
        ).unwrap();
        assert_eq!(source.effective_config.get_str("theme"), Some("base"));
        assert_eq!(
            source
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
        assert_eq!(
            source.effective_config.get_str("themeVariables.mainBkg"),
            initialized
                .effective_config
                .get_str("themeVariables.mainBkg")
        );
        assert!(
            source
                .effective_config
                .explicit_config_owns_path("themeVariables.mainBkg")
        );
    }
}

#[test]
fn scoped_null_keeps_initialized_variables_and_raw_source_ownership() {
    let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        json!({"theme": "base", "themeVariables": {"primaryColor": "#345678"}}),
    ));
    let baseline = engine.parse_metadata_sync("flowchart TD\nA-->B").unwrap();
    let source = engine.parse_metadata_sync(
        "%%{init: {\"flowchart\": {\"theme\": \"null\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
    ).unwrap();
    assert_eq!(source.effective_config.get_str("theme"), Some("null"));
    assert_eq!(
        source
            .effective_config
            .get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    assert_eq!(
        source.effective_config.get_str("themeVariables.actorBkg"),
        baseline.effective_config.get_str("themeVariables.actorBkg")
    );
    assert!(
        source
            .effective_config
            .explicit_config_owns_path("themeVariables.primaryColor")
    );
}

#[test]
fn fully_filtered_source_config_does_not_change_effective_config() {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "base",
            "secure": ["secure", "securityLevel", "themeVariables"]
        })));
    let without_source = engine
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse without source config");
    let filtered_source = engine
        .parse_metadata_sync(
            "%%{init: {\"themeVariables\": {\"textColor\": \"#22c55e\"}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse fully filtered source config");

    assert_eq!(
        filtered_source.effective_config.as_value(),
        without_source.effective_config.as_value(),
        "a source config removed by the secure policy must be semantically invisible"
    );
}

#[test]
fn registered_source_theme_uses_raw_truthy_initial_theme_variables() {
    let metadata = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "secure": [],
            "themeVariables": true
        })))
        .parse_metadata_sync(
            "%%{init: {\"theme\": \"base\", \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse a registered source theme over truthy scalar initialization input");

    assert_eq!(metadata.effective_config.get_str("theme"), Some("base"));
    assert_eq!(
        metadata
            .effective_config
            .get_str("themeVariables.primaryColor"),
        Some("#fff4dd"),
        "truthy scalar initialize input must make assignWithDepth ignore source themeVariables"
    );
}

#[test]
fn empty_explicit_site_secure_policy_owns_the_replacement_path() {
    let metadata = MermaidEngine::new()
        .with_exact_site_config(Some(SourcePresentationConfig::from_value(
            json!({"secure": []}),
        )))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse with an explicit empty secure policy");

    assert_eq!(metadata.effective_config.as_value()["secure"], json!([]));
    assert!(
        metadata
            .effective_config
            .explicit_config_owns_path("secure")
    );
}

#[test]
fn generated_defaults_preserve_mermaids_runtime_class_object_override() {
    let expected = json!({
        "theme": "redux-color",
        "look": "neo",
        "hideEmptyMembersBox": false,
        "hierarchicalNamespaces": true
    });
    assert_eq!(
        crate::generated::default_site_config()
            .as_value()
            .get("class"),
        Some(&expected),
        "Mermaid 12 defaultConfig.ts carries appearance defaults into the replacement Class object"
    );
}

#[test]
fn detects_tree_view_beta_as_tree_view() {
    let engine = MermaidEngine::new();
    let res = block_on(engine.parse_metadata("treeView-beta\n\"Root\"")).unwrap();
    assert_eq!(res.diagram_type, "treeView");
}

#[test]
fn detects_ishikawa_headers_as_ishikawa() {
    let engine = MermaidEngine::new();
    for header in ["ishikawa", "ishikawa-beta", "ISHIKAWA-BETA"] {
        let res = block_on(engine.parse_metadata(&format!("{header}\nProblem"))).unwrap();
        assert_eq!(res.diagram_type, "ishikawa");
    }
}

#[test]
fn detects_eventmodeling_as_eventmodeling() {
    let engine = MermaidEngine::new();
    let res = block_on(engine.parse_metadata("eventmodeling\ntf 01 evt Start")).unwrap();
    assert_eq!(res.diagram_type, "eventmodeling");
}

#[test]
fn detects_venn_beta_as_venn() {
    let engine = MermaidEngine::new();
    let res = block_on(engine.parse_metadata("venn-beta\nset A")).unwrap();
    assert_eq!(res.diagram_type, "venn");
}

#[test]
fn detects_11_16_new_family_headers_for_metadata() {
    let engine = MermaidEngine::new();

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
    let mut config = SourcePresentationConfig::empty_object();

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
    let mut config = SourcePresentationConfig::empty_object();

    // Mermaid detectType iterates the supplied detector records and returns the first match.
    let detected = registry
        .detect_type("sequenceDiagram\nA ->> B: hello", &mut config)
        .unwrap();

    assert_eq!(detected, "host-first");
}

#[test]
fn empty_detector_registry_rejects_builtin_leading_keywords() {
    let registry = DetectorRegistry::new();
    let mut config = SourcePresentationConfig::empty_object();

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
#[cfg(feature = "diagram-kanban")]
fn kanban_detector_and_known_type_parser_preserve_distinct_upstream_case_rules() {
    let engine = MermaidEngine::new();

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
    let engine = MermaidEngine::new();

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
    let mut config = SourcePresentationConfig::empty_object();

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
#[cfg(feature = "diagram-flowchart")]
fn leading_utf8_bom_is_handled_consistently_across_public_entrypoints() {
    let source = "\u{feff}flowchart TD\nA-->B\n";
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let mut config = SourcePresentationConfig::empty_object();

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

    let parsed = MermaidEngine::new()
        .parse_diagram_sync(source, ParseOptions::strict())
        .expect("public parse accepts a leading UTF-8 BOM")
        .expect("flowchart model");
    assert_eq!(parsed.meta.diagram_type, "flowchart-v2");

    let metadata = MermaidEngine::new()
        .parse_metadata_sync("\u{feff}---\ntitle: BOM title\n---\nflowchart TD\nA-->B\n")
        .expect("BOM is removed before frontmatter extraction");
    assert_eq!(metadata.title.as_deref(), Some("BOM title"));
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn malformed_directive_json_is_removed_without_rejecting_the_diagram() {
    let source = "%%{init: {\"theme\": }}%%\nflowchart TD\nA-->B\n";
    let registry = DetectorRegistry::pinned_mermaid_baseline();

    let preprocessed =
        preprocess_diagram(source, &registry).expect("invalid directive config is fail-soft");
    assert_eq!(preprocessed.code(), "flowchart TD\nA-->B\n");
    assert!(preprocessed.config.is_empty_object());

    let parsed = MermaidEngine::new()
        .parse_diagram_sync(source, ParseOptions::strict())
        .expect("invalid directive config does not reject a valid diagram")
        .expect("flowchart model");
    assert_eq!(parsed.meta.diagram_type, "flowchart-v2");
}

#[test]
fn strict_unterminated_directive_marker_truncates_like_mermaid() {
    let source = "%%{\nflowchart TD\nA-->B\n";
    let registry = DetectorRegistry::pinned_mermaid_baseline();
    let mut config = SourcePresentationConfig::empty_object();

    assert!(registry.detect_type(source, &mut config).is_err());

    let preprocessed = preprocess_diagram(source, &registry)
        .expect("strict preprocessing truncates an unterminated directive");
    assert_eq!(preprocessed.code(), "");

    let error = MermaidEngine::new()
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap_err();
    assert!(matches!(error, crate::Error::DetectType(_)));
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn lenient_unterminated_directive_marker_recovers_the_following_diagram() {
    let source = concat!(
        "%%{init: {\"config\": {\"curve\": \"linear\"}}}%%\n",
        "%%{\n",
        "flowchart TD\nA-->B\n",
    );

    let parsed = MermaidEngine::new()
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

    let snapshot = MermaidEngine::new()
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
            let mut config = SourcePresentationConfig::empty_object();
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

    let mut config = SourcePresentationConfig::empty_object();
    let detected = registry
        .detect_type(
            "   ---\n   title: Flow\n   ---\n   sequenceDiagram\n   Alice->Bob: Hi\n",
            &mut config,
        )
        .expect("matching indented frontmatter should be stripped before detection");
    assert_eq!(detected, "sequence");

    let mut config = SourcePresentationConfig::empty_object();
    let detected = registry
        .detect_type(
            "   ---\ntitle: Flow\n---\nsequenceDiagram\nAlice->Bob: Hi\n",
            &mut config,
        )
        .expect("mismatched frontmatter should remain visible to the pseudo detector");
    assert_eq!(detected, "---");

    let mut config = SourcePresentationConfig::empty_object();
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
    let engine = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(Value::Object(root)));

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
