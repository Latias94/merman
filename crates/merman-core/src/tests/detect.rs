use crate::config::presentation::ThemeParseBinding;
use crate::presentation::PresentationConfig;
use crate::*;
use futures::executor::block_on;
use serde_json::{Value, json};
use std::fmt::Write;

fn test_detector_always_matches(_text: &str, _config: &mut PresentationConfig) -> bool {
    true
}

fn test_detector_deep_merges_empty_patch(_text: &str, config: &mut PresentationConfig) -> bool {
    config.deep_merge(&json!({}));
    true
}

fn test_detector_deep_merges_unrelated_nested_patch(
    _text: &str,
    config: &mut PresentationConfig,
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
    config: &mut PresentationConfig,
) -> bool {
    config.deep_merge(&json!({ "flowchart": "ignored" }));
    true
}

fn test_detector_deep_merges_depth_boundary_value(
    _text: &str,
    config: &mut PresentationConfig,
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
    let engine = Engine::new();
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
        let detected = registry.detect_type(source, &mut PresentationConfig::empty_object());
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
                .detect_type(source, &mut PresentationConfig::empty_object())
                .is_err()
        );
    }
}

#[test]
fn canonical_catalog_detects_flowchart_elk_and_sets_layout() {
    let engine = Engine::new();
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
    fn replace_config(_text: &str, config: &mut PresentationConfig) -> bool {
        let mut replacement = crate::json_value::clone_value_nonrecursive(config.as_value());
        replacement["flowchart"]["rankSpacing"] = json!(73);
        replacement
            .as_object_mut()
            .unwrap()
            .remove("themeVariables");
        *config = PresentationConfig::from_value(replacement);
        true
    }

    for source in ["probe", "---\nconfig:\n  theme: base\n---\nprobe"] {
        let mut engine = Engine::new();
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
    fn replace_then_assign(_text: &str, config: &mut PresentationConfig) -> bool {
        *config = PresentationConfig::from_value(crate::json_value::clone_value_nonrecursive(
            config.as_value(),
        ));
        let same = config.as_value()["flowchart"]["nodeSpacing"].clone();
        config.set_value("flowchart.nodeSpacing", same);
        true
    }
    for source in ["probe", "---\nconfig:\n  theme: base\n---\nprobe"] {
        let mut engine = Engine::new();
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
    fn replace_with_parsed_config(_text: &str, config: &mut PresentationConfig) -> bool {
        *config = Engine::new()
            .with_site_config(PresentationConfig::from_value(json!({"theme": "base"})))
            .parse_metadata_with_type_sync("flowchart-v2", "graph TD\nA-->B")
            .unwrap()
            .effective_config;
        assert!(!config.explicit_config_owns_path("themeVariables.previouslyMissing"));
        true
    }
    for source in ["probe", "---\nconfig:\n  theme: dark\n---\nprobe"] {
        let mut engine = Engine::new()
            .with_site_config(PresentationConfig::from_value(json!({"theme": "base"})));
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
    fn assign_same_value(_text: &str, config: &mut PresentationConfig) -> bool {
        let value = config.as_value()["flowchart"]["nodeSpacing"].clone();
        config.set_value("flowchart.nodeSpacing", value);
        true
    }
    fn replace_with_nested_result(_text: &str, config: &mut PresentationConfig) -> bool {
        let mut inner = Engine::new();
        *inner.registry_mut() = DetectorRegistry::new();
        inner
            .registry_mut()
            .add_fn("flowchart-v2", assign_same_value);
        *config = inner
            .parse_metadata_sync("inner-probe")
            .unwrap()
            .effective_config;
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
        let mut engine = Engine::new();
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
    fn assign_initialized_background(_text: &str, config: &mut PresentationConfig) -> bool {
        let background = config.as_value()["themeVariables"]["mainBkg"].clone();
        config.set_value("themeVariables.mainBkg", background);
        true
    }
    fn replace_with_nested_result(_text: &str, config: &mut PresentationConfig) -> bool {
        let mut inner = Engine::new()
            .with_site_config(PresentationConfig::from_value(json!({"theme": "dark"})));
        *inner.registry_mut() = DetectorRegistry::new();
        inner
            .registry_mut()
            .add_fn("flowchart-v2", assign_initialized_background);
        *config = inner
            .parse_metadata_sync("inner-probe")
            .unwrap()
            .effective_config;
        true
    }

    let initialized = Engine::new()
        .with_site_config(PresentationConfig::from_value(json!({"theme": "dark"})))
        .parse_metadata_with_type_sync("flowchart-v2", "graph TD\nA-->B")
        .unwrap();
    let source = "%%{init: {\"flowchart\": {\"theme\": \"base\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nouter-probe";
    let rebuilt = Engine::new()
        .with_site_config(PresentationConfig::from_value(json!({"theme": "dark"})))
        .parse_metadata_with_type_sync("flowchart-v2", source)
        .unwrap();
    assert_ne!(
        rebuilt.effective_config.get_str("themeVariables.mainBkg"),
        initialized
            .effective_config
            .get_str("themeVariables.mainBkg")
    );
    let mut engine =
        Engine::new().with_site_config(PresentationConfig::from_value(json!({"theme": "dark"})));
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
    fn replace(text: &str, config: &mut PresentationConfig) -> bool {
        let mut value = crate::json_value::clone_value_nonrecursive(config.as_value());
        if text.trim() == "changed-probe" {
            value["flowchart"]["rankSpacing"] = json!(73);
        }
        *config = PresentationConfig::from_value(value);
        true
    }
    for source in [
        "unchanged-probe",
        "changed-probe",
        "---\nconfig:\n  theme: base\n---\nunchanged-probe",
        "---\nconfig:\n  theme: base\n---\nchanged-probe",
    ] {
        let mut engine = Engine::new().with_site_config(PresentationConfig::from_value(json!({
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
    detector: fn(&str, &mut PresentationConfig) -> bool,
) -> Engine {
    let mut engine = Engine::new();
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
    let site_engine = Engine::new().with_site_config(PresentationConfig::from_value(json!({
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

    let source_engine = Engine::new();
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

fn node_default_plan(config: Value) -> crate::__private::ThemeCompatibilityPlan {
    crate::__private::ThemeCompatibilityPlan::try_new([0x5a; 32], MermaidConfig::from_value(config))
        .unwrap()
}

const NODE_PAINT_INPUTS: &[(crate::__private::GitGraphPaintInput, &str)] = &[
    (
        crate::__private::GitGraphPaintInput::PrimaryColor,
        "themeVariables.primaryColor",
    ),
    (
        crate::__private::GitGraphPaintInput::MainBackground,
        "themeVariables.mainBkg",
    ),
    (
        crate::__private::GitGraphPaintInput::TagBackground,
        "themeVariables.tagLabelBackground",
    ),
    (
        crate::__private::GitGraphPaintInput::PrimaryBorder,
        "themeVariables.primaryBorderColor",
    ),
    (
        crate::__private::GitGraphPaintInput::NodeBorder,
        "themeVariables.nodeBorder",
    ),
    (
        crate::__private::GitGraphPaintInput::TagBorder,
        "themeVariables.tagLabelBorder",
    ),
];

fn blocked(
    config: &PresentationConfig,
    input: crate::__private::GitGraphPaintInput,
) -> Option<bool> {
    crate::__private::gitgraph_paint_inputs(config).map(|inputs| inputs.is_owned(input))
}

#[test]
fn family_paint_inputs_capture_six_raw_paths() {
    use crate::__private::install_theme_compatibility;
    for &(explicit_input, explicit_path) in NODE_PAINT_INPUTS {
        let mut compatibility = MermaidConfig::from_value(json!({"theme": "base"}));
        compatibility.set_value(explicit_path, json!("#123456"));
        let plan = node_default_plan(compatibility.as_value().clone());
        let parsed = install_theme_compatibility(Engine::new(), &plan)
            .parse_metadata_sync("gitGraph\ncommit")
            .unwrap();
        for &(input, path) in NODE_PAINT_INPUTS {
            assert_eq!(
                blocked(&parsed.effective_config, input),
                Some(input as u8 == explicit_input as u8),
                "{explicit_path} -> {path}"
            );
        }
        assert!(
            crate::__private::theme_parse_evidence(&parsed).matches_recipe(Some(plan.recipe()))
        );
    }
    let plan =
        node_default_plan(json!({"theme": "base", "themeVariables": {"primaryColor": "#fff4dd"}}));
    let parsed = install_theme_compatibility(Engine::new(), &plan)
        .parse_metadata_sync("gitGraph\ncommit")
        .unwrap();
    assert!(crate::__private::config_path_overrides_typed_default(
        &parsed.effective_config,
        "themeVariables.mainBkg"
    ));
    assert_eq!(
        blocked(
            &parsed.effective_config,
            crate::__private::GitGraphPaintInput::MainBackground
        ),
        Some(false)
    );
}

#[test]
fn family_paint_inputs_keep_authority_across_secure_source_theme_reselection() {
    use crate::__private::install_theme_compatibility;
    let plan = node_default_plan(json!({"theme": "default"}));
    let source = "%%{init: {\"theme\": \"base\", \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\ngitGraph\ncommit";
    for (site, expected) in [
        (json!({"secure": []}), true),
        (json!({"secure": ["theme", "themeVariables"]}), false),
    ] {
        let parsed = install_theme_compatibility(
            Engine::new().with_site_config(PresentationConfig::from_value(site)),
            &plan,
        )
        .parse_metadata_sync(source)
        .unwrap();
        assert_eq!(
            blocked(
                &parsed.effective_config,
                crate::__private::GitGraphPaintInput::PrimaryColor
            ),
            Some(expected)
        );
        assert_eq!(
            blocked(
                &parsed.effective_config,
                crate::__private::GitGraphPaintInput::MainBackground
            ),
            Some(false)
        );
        assert!(
            crate::__private::theme_parse_evidence(&parsed).matches_recipe(Some(plan.recipe()))
        );
    }
}

#[test]
fn family_paint_inputs_distinguish_missing_decisions_and_frozen_mutation() {
    use crate::__private::install_theme_compatibility;
    let input = crate::__private::GitGraphPaintInput::PrimaryColor;
    let plan = node_default_plan(json!({}));
    assert_eq!(blocked(&PresentationConfig::empty_object(), input), None);
    let other_family = install_theme_compatibility(Engine::new(), &plan)
        .parse_metadata_sync("flowchart TD\nA-->B")
        .unwrap();
    assert_eq!(blocked(&other_family.effective_config, input), None);
    let parsed = install_theme_compatibility(Engine::new(), &plan)
        .parse_metadata_sync("gitGraph\ncommit")
        .unwrap();
    assert_eq!(blocked(&parsed.effective_config, input), Some(false));
    assert!(crate::__private::er_paint_inputs(&parsed.effective_config).is_none());
    let mut changed = parsed.effective_config.clone();
    changed.set_value("unrelated", json!(true));
    assert_eq!(blocked(&changed, input), None);
    assert_eq!(blocked(&parsed.effective_config, input), Some(false));
}

#[test]
fn family_paint_inputs_capture_same_value_detector_claims() {
    use crate::__private::{GitGraphPaintInput, install_theme_compatibility};
    fn claim_background(_text: &str, config: &mut PresentationConfig) -> bool {
        let value = config.as_value()["themeVariables"]["mainBkg"].clone();
        config.set_value("themeVariables.mainBkg", value);
        true
    }
    let plan = node_default_plan(json!({"theme": "base"}));
    let mut engine = install_theme_compatibility(Engine::new(), &plan);
    *engine.registry_mut() = DetectorRegistry::new();
    engine.registry_mut().add_fn("gitGraph", claim_background);
    let parsed = engine.parse_metadata_sync("custom graph").unwrap();
    assert_eq!(
        blocked(&parsed.effective_config, GitGraphPaintInput::MainBackground),
        Some(true)
    );
    assert_eq!(
        blocked(&parsed.effective_config, GitGraphPaintInput::TagBackground),
        Some(false)
    );
}

#[test]
fn er_paint_inputs_preserve_raw_authority_and_frozen_invalidation() {
    use crate::__private::{ErPaintInput, er_paint_inputs, install_theme_compatibility};
    let slots = [
        (ErPaintInput::Text, "themeVariables.textColor"),
        (ErPaintInput::NodeText, "themeVariables.nodeTextColor"),
        (ErPaintInput::Line, "themeVariables.lineColor"),
        (ErPaintInput::OddRow, "themeVariables.rowOdd"),
        (ErPaintInput::EvenRow, "themeVariables.rowEven"),
    ];
    let plan = node_default_plan(json!({"theme": "base"}));
    for (selected, path) in slots {
        let mut site = PresentationConfig::empty_object();
        site.set_value(path, json!("#123456"));
        let parsed = install_theme_compatibility(Engine::new().with_site_config(site), &plan)
            .parse_metadata_sync("erDiagram\nA ||--|| B : owns")
            .unwrap();
        let inputs = er_paint_inputs(&parsed.effective_config).unwrap();
        for (input, _) in slots {
            assert_eq!(
                inputs.is_owned(input),
                input as u8 == selected as u8,
                "{path}"
            );
        }
        assert!(crate::__private::gitgraph_paint_inputs(&parsed.effective_config).is_none());
        let mut changed = parsed.effective_config.clone();
        changed.set_value("unrelated", json!(true));
        assert!(er_paint_inputs(&changed).is_none());
    }
    let normalized = install_theme_compatibility(
        Engine::new().with_site_config(PresentationConfig::from_value(json!({
            "themeVariables": {"lineColor": null, "primaryColor": "#123456"}
        }))),
        &plan,
    )
    .parse_metadata_sync("erDiagram\nA ||--|| B : owns")
    .unwrap();
    assert!(
        !er_paint_inputs(&normalized.effective_config)
            .unwrap()
            .is_owned(ErPaintInput::Line)
    );
    assert!(
        !normalized
            .effective_config
            .explicit_config_owns_path("themeVariables.lineColor")
    );
}

#[test]
fn effective_config_tracks_only_high_priority_explicit_owners() {
    let metadata = Engine::new()
        .with_theme_compatibility(theme_binding_for(MermaidConfig::from_value(json!({
            "themeVariables": {"useGradient": true}
        }))))
        .with_site_config(PresentationConfig::from_value(json!({
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
fn theme_variable_derived_ownership_follows_initialization_but_not_source_only_updates() {
    let site = Engine::new()
        .with_site_config(PresentationConfig::from_value(json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "#fff4dd" }
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse site-owned base primary color");
    assert!(crate::__private::config_path_overrides_typed_default(
        &site.effective_config,
        "themeVariables.mainBkg"
    ));

    let source = Engine::new()
        .with_site_config(PresentationConfig::from_value(json!({
            "theme": "base",
            "secure": []
        })))
        .parse_metadata_sync(
            "%%{init: {\"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse source-owned base primary color");
    assert_eq!(
        source
            .effective_config
            .get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    assert_eq!(
        source.effective_config.get_str("themeVariables.mainBkg"),
        Some("#fff4dd")
    );
    assert!(
        source
            .effective_config
            .explicit_config_owns_path("themeVariables.primaryColor")
    );
    assert!(!crate::__private::config_path_overrides_typed_default(
        &source.effective_config,
        "themeVariables.mainBkg"
    ));

    let compatibility = Engine::new()
        .with_theme_compatibility(theme_binding_for(MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "#fff4dd" }
        }))))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse compatibility-owned base primary color");
    assert!(crate::__private::config_path_overrides_typed_default(
        &compatibility.effective_config,
        "themeVariables.mainBkg"
    ));
    assert_eq!(compatibility.mermaid_compatibility_residual_count(), 2);
    assert!(!crate::__private::config_path_overrides_typed_default(
        &compatibility.effective_config,
        "themeVariables.radius"
    ));
}

#[test]
fn extended_dark_primary_color_only_recomputes_requirement_background_during_initialization() {
    for theme in ["neo-dark", "redux-dark"] {
        let site = Engine::new()
            .with_site_config(PresentationConfig::from_value(json!({
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
        assert!(
            crate::__private::config_path_overrides_typed_default(
                &site.effective_config,
                "themeVariables.requirementBackground"
            ),
            "{theme} must preserve site ownership through the derived Requirement background"
        );

        let engine = Engine::new().with_site_config(PresentationConfig::from_value(json!({
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
            !crate::__private::config_path_overrides_typed_default(
                &source.effective_config,
                "themeVariables.requirementBackground"
            ),
            "{theme} source-only primaryColor must not own the retained Requirement background"
        );
    }
}

#[test]
fn root_source_theme_truthiness_controls_rematerialization_after_appearance_fallback() {
    let engine =
        Engine::new().with_site_config(PresentationConfig::from_value(json!({"theme": "base"})));
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
    let engine = Engine::new().with_site_config(PresentationConfig::from_value(json!({
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
    let equivalent_same_theme_initialization = Engine::new()
        .with_site_config(PresentationConfig::from_value(json!({
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
    let equivalent_initialization = Engine::new()
        .with_site_config(PresentationConfig::from_value(json!({
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
    let engine = Engine::new().with_site_config(PresentationConfig::from_value(json!({
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
fn scoped_source_theme_rebuilds_raw_variables_without_losing_recipe_binding() {
    let recipe = MermaidConfig::from_value(json!({
        "theme": "base", "themeVariables": { "secondaryColor": "#345678" }
    }));
    let engine = Engine::new().with_theme_compatibility(theme_binding_for(recipe));
    let baseline = engine.parse_metadata_sync("flowchart TD\nA-->B").unwrap();
    assert_eq!(baseline.effective_config.get_str("theme"), Some("base"));
    let source = engine.parse_metadata_sync(
        "%%{init: {\"flowchart\": {\"theme\": \"dark\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B",
    ).unwrap();
    let equivalent = Engine::new()
        .with_site_config(PresentationConfig::from_value(json!({
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
    assert_eq!(source.theme_parse_binding(), baseline.theme_parse_binding());
    assert_eq!(
        engine
            .parse_metadata_sync("flowchart TD\nA-->B")
            .unwrap()
            .effective_config,
        baseline.effective_config
    );
}

#[test]
fn scoped_same_value_appearance_is_authored_but_secure_source_is_not() {
    let engine = Engine::new().with_theme_compatibility(theme_binding_for(
        MermaidConfig::from_value(json!({"theme": "base"})),
    ));
    let baseline = engine.parse_metadata_sync("flowchart TD\nA-->B").unwrap();
    assert_eq!(baseline.mermaid_compatibility_residual_count(), 1);
    let text = "%%{init: {\"flowchart\": {\"theme\": \"base\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\nflowchart TD\nA-->B";
    let authored = engine.parse_metadata_sync(text).unwrap();
    assert_eq!(authored.effective_config.get_str("theme"), Some("base"));
    assert!(authored.effective_config.explicit_config_owns_path("theme"));
    assert_eq!(authored.mermaid_compatibility_residual_count(), 0);
    assert_eq!(
        authored
            .effective_config
            .get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    assert_eq!(
        authored.effective_config.get_str("themeVariables.mainBkg"),
        Some("#fff4dd")
    );
    assert!(!crate::__private::config_path_overrides_typed_default(
        &authored.effective_config,
        "themeVariables.mainBkg"
    ));
    let secure = engine
        .with_site_config(PresentationConfig::from_value(json!({"secure": ["theme"]})))
        .parse_metadata_sync(text)
        .unwrap();
    assert_eq!(secure.effective_config.get_str("theme"), Some("base"));
    assert!(!secure.effective_config.explicit_config_owns_path("theme"));
    assert_eq!(secure.mermaid_compatibility_residual_count(), 1);
}

#[test]
fn scoped_theme_derivations_do_not_become_raw_paint_authority() {
    use crate::__private::{GitGraphPaintInput, install_theme_compatibility};
    let plan = node_default_plan(json!({"theme": "base"}));
    let engine = install_theme_compatibility(
        Engine::new().with_site_config(PresentationConfig::from_value(json!({"theme": "dark"}))),
        &plan,
    );
    let source = "%%{init: {\"gitGraph\": {\"theme\": \"base\"}, \"themeVariables\": {\"primaryColor\": \"#123456\"}}}%%\ngitGraph\ncommit";
    let parsed = engine.parse_metadata_sync(source).unwrap();
    assert_eq!(
        parsed
            .effective_config
            .get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    assert_eq!(
        parsed.effective_config.get_str("themeVariables.mainBkg"),
        Some("#123456")
    );
    assert_eq!(
        blocked(&parsed.effective_config, GitGraphPaintInput::PrimaryColor),
        Some(true)
    );
    assert_eq!(
        blocked(&parsed.effective_config, GitGraphPaintInput::MainBackground),
        Some(false)
    );
}

#[test]
fn scoped_theme_rebuild_preserves_same_value_detector_assignments() {
    fn claim_initialized_background(_text: &str, config: &mut PresentationConfig) -> bool {
        let background = config.as_value()["themeVariables"]["mainBkg"].clone();
        config.set_value("themeVariables.mainBkg", background);
        true
    }
    fn claim_initialized_root(_text: &str, config: &mut PresentationConfig) -> bool {
        let _ = config.as_value_mut();
        true
    }

    for detector in [claim_initialized_background, claim_initialized_root] {
        let engine = engine_with_only_flowchart_detector(detector)
            .with_site_config(PresentationConfig::from_value(json!({"theme": "dark"})));
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
    let engine =
        Engine::new().with_theme_compatibility(theme_binding_for(MermaidConfig::from_value(
            json!({"theme": "base", "themeVariables": {"primaryColor": "#345678"}}),
        )));
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
    assert_eq!(source.theme_parse_binding(), baseline.theme_parse_binding());
}

#[test]
fn fully_filtered_source_config_does_not_change_effective_config() {
    let engine = Engine::new().with_site_config(PresentationConfig::from_value(json!({
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
    let metadata = Engine::new()
        .with_site_config(PresentationConfig::from_value(json!({
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
    let metadata = Engine::new()
        .with_exact_site_config(Some(PresentationConfig::from_value(json!({"secure": []}))))
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
fn theme_compatibility_freezes_binding_and_conceptual_field_count() {
    let parsed = Engine::new()
        .with_theme_compatibility(theme_binding())
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse theme compatibility config");

    let binding = theme_binding();
    assert_eq!(parsed.theme_parse_binding(), Some(&binding));
    assert_eq!(parsed.mermaid_compatibility_residual_count(), 3);
    assert!(crate::__private::config_path_overrides_typed_default(
        &parsed.effective_config,
        "themeVariables.useGradient"
    ));
    assert!(crate::__private::config_path_overrides_typed_default(
        &parsed.effective_config,
        "darkMode"
    ));
}

#[test]
fn theme_parse_borrowed_queries_match_owned_evidence_for_all_binding_states() {
    use crate::__private::{
        ThemeCompatibilityPlan, theme_parse_evidence, theme_parse_matches_recipe,
        theme_parse_mermaid_residual_count,
    };

    let input = json!({
        "theme": "base", "darkMode": true, "themeVariables": {"useGradient": true},
    });
    let plan = node_default_plan(input.clone());
    let equivalent_plan = node_default_plan(input.clone());
    let different_identity =
        ThemeCompatibilityPlan::try_new([0x5b; 32], MermaidConfig::from_value(input.clone()))
            .unwrap();
    let different_config = node_default_plan(json!({
        "theme": "base", "darkMode": true, "themeVariables": {"useGradient": false},
    }));
    let empty_plan = node_default_plan(json!({}));
    assert_eq!(plan.recipe(), equivalent_plan.recipe());
    assert_ne!(plan.recipe(), different_identity.recipe());
    assert_ne!(plan.recipe(), different_config.recipe());

    let tracking = PresentationConfig::from_theme_parse_binding(theme_binding_for(
        MermaidConfig::from_value(input.clone()),
    ));
    let tracking_empty = PresentationConfig::from_theme_parse_binding(theme_binding_for(
        MermaidConfig::empty_object(),
    ));
    let mut frozen = tracking.clone();
    frozen.freeze_theme_compatibility();
    let mut frozen_empty = tracking_empty.clone();
    frozen_empty.freeze_theme_compatibility();
    let mut shadowed = tracking.clone();
    shadowed.set_value("themeVariables.useGradient", json!(true));
    shadowed.freeze_theme_compatibility();

    for (state, config, expected_recipe, expected_count) in [
        ("none-empty", PresentationConfig::empty_object(), None, 0),
        (
            "none-authored",
            PresentationConfig::from_value(input),
            None,
            0,
        ),
        ("tracking-empty", tracking_empty, None, 0),
        ("tracking-fields", tracking, None, 0),
        ("frozen-empty", frozen_empty, Some(empty_plan.recipe()), 0),
        ("frozen-fields", frozen, Some(plan.recipe()), 3),
        ("frozen-shadowed", shadowed, Some(plan.recipe()), 2),
    ] {
        let metadata = ParseMetadata {
            diagram_type: "flowchart-v2".into(),
            config: MermaidConfig::empty_object(),
            effective_config: config,
            title: None,
        };
        let owned = theme_parse_evidence(&metadata);
        assert!(owned.matches_recipe(expected_recipe), "{state}");
        assert_eq!(owned.mermaid_residual_count(), expected_count, "{state}");
        assert_eq!(
            theme_parse_mermaid_residual_count(&metadata),
            owned.mermaid_residual_count(),
            "{state}",
        );
        for recipe in [
            None,
            Some(plan.recipe()),
            Some(equivalent_plan.recipe()),
            Some(different_identity.recipe()),
            Some(different_config.recipe()),
            Some(empty_plan.recipe()),
        ] {
            assert_eq!(
                theme_parse_matches_recipe(&metadata, recipe),
                owned.matches_recipe(recipe),
                "{state}: {recipe:?}",
            );
        }
    }
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
fn theme_parse_evidence_reconciles_complete_conceptual_fields_and_deduplicates_paths() {
    use crate::__private::{
        ThemeCompatibilityConsumptionDisposition as Disposition, ThemeCompatibilityFieldKind,
    };

    let parsed = Engine::new()
        .with_theme_compatibility(theme_binding_for(MermaidConfig::from_value(json!({
            "theme": "base",
            "darkMode": true
        }))))
        .parse_metadata_sync("packet\n0-7: Header")
        .expect("parse conceptual compatibility evidence");
    let evidence = crate::__private::theme_parse_evidence(&parsed);
    assert_eq!(evidence.mermaid_residual_count(), 2);

    let theme = evidence
        .mermaid_fields()
        .find(|field| field.kind() == ThemeCompatibilityFieldKind::Theme)
        .expect("theme conceptual field");
    let dark_mode = evidence
        .mermaid_fields()
        .find(|field| field.kind() == ThemeCompatibilityFieldKind::DarkMode)
        .expect("dark-mode conceptual field");
    assert_eq!(theme.opaque_id(), "mermaid.theme");
    assert_eq!(dark_mode.opaque_id(), "mermaid.darkMode");
    let dark_mode_paths = dark_mode.surviving_assignment_paths().collect::<Vec<_>>();
    // Base now records derived assignments at their actual execution stage. The conceptual
    // dark-mode field owns both its explicit spellings and the color decisions it controls.
    for path in [
        "darkMode",
        "themeVariables.darkMode",
        "themeVariables.primaryTextColor",
        "themeVariables.primaryBorderColor",
    ] {
        assert!(
            dark_mode_paths.contains(&path),
            "missing dark-mode dependency: {path}"
        );
    }
    assert!(dark_mode_paths.windows(2).all(|pair| pair[0] < pair[1]));

    let mut partial = theme
        .consume_all(Disposition::ReplacedByTypedSurface)
        .collect::<Vec<_>>();
    partial.push(
        dark_mode
            .consume_assignment_path("darkMode", Disposition::NotReadByFamily)
            .expect("surviving root dark-mode path"),
    );
    let partial_result = evidence.reconcile_mermaid_consumptions(partial.iter());
    assert_eq!(partial_result.consumed_field_count(), 1);
    assert_eq!(partial_result.remaining_field_count(), 1);

    let mut complete = theme
        .consume_all(Disposition::ReplacedByTypedSurface)
        .collect::<Vec<_>>();
    complete.extend(theme.consume_all(Disposition::ReplacedByTypedSurface));
    complete.extend(dark_mode.consume_all(Disposition::NotReadByFamily));
    let complete_result = evidence.reconcile_mermaid_consumptions(complete.iter());
    assert_eq!(complete_result.consumed_field_count(), 2);
    assert_eq!(complete_result.remaining_field_count(), 0);

    let mut conflicting = theme
        .consume_all(Disposition::ReplacedByTypedSurface)
        .collect::<Vec<_>>();
    conflicting.extend(theme.consume_all(Disposition::NotReadByFamily));
    conflicting.extend(dark_mode.consume_all(Disposition::NotReadByFamily));
    let conflicting_result = evidence.reconcile_mermaid_consumptions(conflicting.iter());
    assert_eq!(conflicting_result.consumed_field_count(), 1);
    assert_eq!(conflicting_result.remaining_field_count(), 1);
}

#[test]
fn theme_parse_evidence_keeps_unconsumed_variables_after_theme_and_dark_mode_reconcile() {
    use crate::__private::{
        ThemeCompatibilityConsumptionDisposition as Disposition, ThemeCompatibilityFieldKind,
    };

    let parsed = Engine::new()
        .with_theme_compatibility(theme_binding_for(MermaidConfig::from_value(json!({
            "theme": "base",
            "darkMode": true,
            "themeVariables": { "primaryColor": "#123456" }
        }))))
        .parse_metadata_sync("packet\n0-7: Header")
        .expect("parse compatibility evidence with an unrelated variable");
    let evidence = crate::__private::theme_parse_evidence(&parsed);
    assert_eq!(evidence.mermaid_residual_count(), 3);

    let consumptions = evidence
        .mermaid_fields()
        .filter(|field| {
            matches!(
                field.kind(),
                ThemeCompatibilityFieldKind::Theme | ThemeCompatibilityFieldKind::DarkMode
            )
        })
        .flat_map(|field| field.consume_all(Disposition::ReplacedByTypedSurface))
        .collect::<Vec<_>>();
    let result = evidence.reconcile_mermaid_consumptions(consumptions.iter());

    assert_eq!(result.consumed_field_count(), 2);
    assert_eq!(result.remaining_field_count(), 1);
    assert!(
        evidence
            .mermaid_fields()
            .any(|field| field.kind() == ThemeCompatibilityFieldKind::Variable)
    );
}

#[test]
fn frozen_binding_keeps_the_validated_input_when_theme_replays_a_dissimilar_field_type() {
    let binding = theme_binding_for(MermaidConfig::from_value(json!({
        "theme": "default",
        "themeVariables": {"cynefin": "ignored-by-assignWithDepth"}
    })));
    let parsed = Engine::new()
        .with_theme_compatibility(binding.clone())
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse compatibility config with a replayed dissimilar field type");

    assert_eq!(
        parsed.effective_config.as_value()["themeVariables"]["cynefin"],
        json!("ignored-by-assignWithDepth")
    );
    assert_eq!(parsed.theme_parse_binding(), Some(&binding));
    assert_eq!(parsed.mermaid_compatibility_residual_count(), 2);
}

#[test]
fn base_engine_host_config_outranks_later_theme_installation() {
    for host_value in [true, false] {
        let parsed = Engine::new()
            .with_site_config(PresentationConfig::from_value(json!({
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
        .with_site_config(PresentationConfig::from_value(json!({
            "themeVariables": {"useGradient": true}
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse same-value site override");
    assert_eq!(site.mermaid_compatibility_residual_count(), 2);
    assert!(
        site.effective_config
            .explicit_config_owns_path("themeVariables.useGradient")
    );

    let source = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(PresentationConfig::from_value(json!({"secure": []})))
        .parse_metadata_sync(
            "%%{init: {\"themeVariables\": {\"useGradient\": true}}}%%\nflowchart TD\nA-->B",
        )
        .expect("parse same-value source override");
    assert_eq!(source.mermaid_compatibility_residual_count(), 2);
    assert!(
        source
            .effective_config
            .explicit_config_owns_path("themeVariables.useGradient")
    );
}

#[test]
fn dark_mode_counts_once_until_both_physical_paths_are_shadowed() {
    let only_root = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(PresentationConfig::from_value(json!({"darkMode": true})))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse root dark-mode override");
    assert_eq!(only_root.mermaid_compatibility_residual_count(), 3);

    let only_variable = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(PresentationConfig::from_value(json!({
            "themeVariables": {"darkMode": true}
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse theme-variable dark-mode override");
    assert_eq!(only_variable.mermaid_compatibility_residual_count(), 3);

    let both = Engine::new()
        .with_theme_compatibility(theme_binding())
        .with_site_config(PresentationConfig::from_value(json!({
            "darkMode": true,
            "themeVariables": {"darkMode": true}
        })))
        .parse_metadata_sync("flowchart TD\nA-->B")
        .expect("parse both dark-mode overrides");
    assert_eq!(both.mermaid_compatibility_residual_count(), 2);
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
    let mut config = PresentationConfig::empty_object();

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
    let mut config = PresentationConfig::empty_object();

    // Mermaid detectType iterates the supplied detector records and returns the first match.
    let detected = registry
        .detect_type("sequenceDiagram\nA ->> B: hello", &mut config)
        .unwrap();

    assert_eq!(detected, "host-first");
}

#[test]
fn empty_detector_registry_rejects_builtin_leading_keywords() {
    let registry = DetectorRegistry::new();
    let mut config = PresentationConfig::empty_object();

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
    let mut config = PresentationConfig::empty_object();

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
    let mut config = PresentationConfig::empty_object();

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
#[cfg(feature = "diagram-flowchart")]
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
    let mut config = PresentationConfig::empty_object();

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
#[cfg(feature = "diagram-flowchart")]
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
            let mut config = PresentationConfig::empty_object();
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

    let mut config = PresentationConfig::empty_object();
    let detected = registry
        .detect_type(
            "   ---\n   title: Flow\n   ---\n   sequenceDiagram\n   Alice->Bob: Hi\n",
            &mut config,
        )
        .expect("matching indented frontmatter should be stripped before detection");
    assert_eq!(detected, "sequence");

    let mut config = PresentationConfig::empty_object();
    let detected = registry
        .detect_type(
            "   ---\ntitle: Flow\n---\nsequenceDiagram\nAlice->Bob: Hi\n",
            &mut config,
        )
        .expect("mismatched frontmatter should remain visible to the pseudo detector");
    assert_eq!(detected, "---");

    let mut config = PresentationConfig::empty_object();
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
    let engine =
        Engine::new().with_site_config(PresentationConfig::from_value(Value::Object(root)));

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

mod compiled_theme_install_contract {
    use super::*;
    use crate::__private::{
        GitGraphPaintInput, ThemeCompatibilityPlan, config_path_overrides_typed_default,
        gitgraph_paint_inputs, install_theme_compatibility, theme_parse_evidence,
    };

    #[derive(Clone, Copy)]
    enum Installation {
        Legacy,
        Compiled,
    }

    struct Compatibility {
        binding: ThemeParseBinding,
        plan: ThemeCompatibilityPlan,
    }

    impl Compatibility {
        fn new(value: Value) -> Self {
            let config = MermaidConfig::from_value(value);
            Self {
                binding: theme_binding_for(config.clone()),
                plan: ThemeCompatibilityPlan::try_new([0x5a; 32], config).unwrap(),
            }
        }

        fn install(&self, engine: Engine, installation: Installation) -> Engine {
            match installation {
                Installation::Legacy => engine.with_theme_compatibility(self.binding.clone()),
                Installation::Compiled => install_theme_compatibility(engine, &self.plan),
            }
        }
    }

    fn assert_same_metadata(
        expected: &ParseMetadata,
        actual: &ParseMetadata,
        plan: &ThemeCompatibilityPlan,
    ) {
        assert_eq!(actual.diagram_type, expected.diagram_type);
        assert_eq!(actual.title, expected.title);
        for (expected, actual) in [
            (&expected.config, &actual.config),
            (
                expected.effective_config.as_config(),
                actual.effective_config.as_config(),
            ),
        ] {
            assert_eq!(
                serde_json::to_vec(actual.as_value()).unwrap(),
                serde_json::to_vec(expected.as_value()).unwrap(),
                "public ordered JSON must remain identical"
            );
        }
        for path in [
            "theme",
            "darkMode",
            "secure",
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
            "themeVariables.primaryColor",
            "themeVariables.mainBkg",
            "themeVariables.useGradient",
        ] {
            assert_eq!(
                actual.effective_config.explicit_config_owns_path(path),
                expected.effective_config.explicit_config_owns_path(path),
                "explicit ownership at {path}"
            );
            assert_eq!(
                config_path_overrides_typed_default(&actual.effective_config, path),
                config_path_overrides_typed_default(&expected.effective_config, path),
                "typed default authority at {path}"
            );
        }
        let expected_evidence = theme_parse_evidence(expected);
        let actual_evidence = theme_parse_evidence(actual);
        assert!(expected_evidence.matches_recipe(Some(plan.recipe())));
        assert!(actual_evidence.matches_recipe(Some(plan.recipe())));
        assert_eq!(
            actual_evidence.mermaid_fields().collect::<Vec<_>>(),
            expected_evidence.mermaid_fields().collect::<Vec<_>>()
        );
        for input in [
            GitGraphPaintInput::PrimaryColor,
            GitGraphPaintInput::MainBackground,
            GitGraphPaintInput::TagBackground,
            GitGraphPaintInput::PrimaryBorder,
            GitGraphPaintInput::NodeBorder,
            GitGraphPaintInput::TagBorder,
        ] {
            assert_eq!(
                gitgraph_paint_inputs(&actual.effective_config).map(|paint| paint.is_owned(input)),
                gitgraph_paint_inputs(&expected.effective_config)
                    .map(|paint| paint.is_owned(input))
            );
        }
    }

    #[test]
    fn compiled_install_matches_legacy_host_reset_unknown_and_font_authority() {
        let recipe = json!({
            "theme": "base",
            "themeVariables": {"primaryColor": "#123456", "useGradient": true}
        });
        let prior = Engine::new()
            .with_theme_compatibility(theme_binding_for(MermaidConfig::from_value(json!({
                "theme": "dark", "themeVariables": {"useGradient": false}
            }))))
            .with_site_config(PresentationConfig::from_value(
                json!({"fontFamily": "Prior Host"}),
            ))
            .with_exact_site_config(None);
        let cases = [
            ("pristine", Engine::new(), recipe.clone()),
            ("ordinary empty host", Engine::new().with_site_config(PresentationConfig::empty_object()), recipe.clone()),
            ("same-value authored host", Engine::new().with_site_config(PresentationConfig::from_value(json!({
                "themeVariables": {"primaryColor": "#123456"}
            }))), recipe.clone()),
            ("exact reset replaces previous recipe", prior, recipe.clone()),
            ("unknown host theme", Engine::new().with_site_config(PresentationConfig::from_value(json!({
                "theme": "unknown-host-theme", "secure": []
            }))), recipe.clone()),
            ("unknown compatibility theme", Engine::new(), json!({
                "theme": "unknown-compatibility-theme", "themeVariables": {"useGradient": true}
            })),
            ("host and compatibility fonts", Engine::new().with_site_config(PresentationConfig::from_value(json!({
                "secure": [], "fontFamily": "Host Root", "themeVariables": {"fontFamily": "Host Theme", "fontSize": "22px"}
            }))), json!({
                "theme": "base", "themeVariables": {"fontFamily": "Plan Theme", "fontSize": "18px"}
            })),
        ];
        for (name, base, recipe) in cases {
            let compatibility = Compatibility::new(recipe);
            let legacy = compatibility.install(base.clone(), Installation::Legacy);
            let compiled = compatibility.install(base, Installation::Compiled);
            for source in [
                "gitGraph\ncommit",
                "%%{init: {\"theme\": \"forest\", \"themeVariables\": {\"primaryColor\": \"#654321\", \"fontFamily\": \"Source Theme\"}}}%%\ngitGraph\ncommit",
                "gitGraph\ncommit",
            ] {
                let expected = legacy
                    .parse_metadata_sync(source)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                let actual = compiled
                    .parse_metadata_sync(source)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                assert_same_metadata(&expected, &actual, &compatibility.plan);
                if name == "same-value authored host" {
                    assert!(
                        actual
                            .effective_config
                            .explicit_config_owns_path("themeVariables.primaryColor")
                    );
                }
            }
        }
    }

    #[test]
    fn compiled_bad_compatibility_color_stays_deferred_behind_preprocess_and_cancellation() {
        let compatibility = Compatibility::new(json!({
            "theme": "base", "themeVariables": {"primaryColor": "not-a-color"}
        }));
        for installation in [
            Installation::Legacy,
            Installation::Compiled,
            Installation::Compiled,
        ] {
            let engine = compatibility.install(Engine::new(), installation);
            for source in [
                "gitGraph\ncommit",
                "not-a-known-diagram",
                "%%{init: {invalid}}%%\ngitGraph\ncommit",
            ] {
                let error = engine.parse_metadata_sync(source).unwrap_err();
                let Error::ThemeColor(crate::theme_color::ColorError::UnsupportedFormat { input }) =
                    error
                else {
                    panic!("theme calculation must precede detection: {error:?}");
                };
                assert_eq!(input, "not-a-color");
            }
            assert!(matches!(
                engine.parse_metadata_sync("---\nconfig:\n  theme: base\ngitGraph\ncommit"),
                Err(Error::MalformedFrontMatter)
            ));
            let operation = OperationControl::new();
            operation.cancel();
            let cancelled = engine
                .parse_diagram_for_render_model_controlled_sync(
                    "gitGraph\ncommit",
                    ParseOptions::strict(),
                    &operation,
                )
                .expect_err("admission cancellation must precede deferred theme failure");
            assert_eq!(cancelled.reason, CancelReason::Requested);
            assert_eq!(cancelled.phase, OperationPhase::Admission);
        }
    }

    #[test]
    fn compiled_install_preserves_unfinished_nested_detector_clone_claims() {
        use std::cell::RefCell;

        thread_local! {
            static INNER: RefCell<Option<(Installation, Compatibility)>> = const { RefCell::new(None) };
            static ESCAPED: RefCell<Option<PresentationConfig>> = const { RefCell::new(None) };
            static CALLS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
        }

        fn capture_unfinished(_text: &str, config: &mut PresentationConfig) -> bool {
            CALLS.with(|calls| calls.borrow_mut().push("inner capture"));
            ESCAPED.with(|escaped| *escaped.borrow_mut() = Some(config.clone()));
            true
        }

        fn replace_with_unfinished_clone(_text: &str, config: &mut PresentationConfig) -> bool {
            CALLS.with(|calls| calls.borrow_mut().push("outer enter"));
            let mut inner = INNER.with(|inner| {
                let inner = inner.borrow();
                let (installation, compatibility) = inner.as_ref().unwrap();
                compatibility.install(Engine::new(), *installation)
            });
            *inner.registry_mut() = DetectorRegistry::new();
            inner.registry_mut().add_fn("gitGraph", capture_unfinished);
            let completed = inner.parse_metadata_sync("inner probe").unwrap();
            assert!(
                !completed
                    .effective_config
                    .explicit_config_owns_path("themeVariables.primaryColor")
            );
            CALLS.with(|calls| calls.borrow_mut().push("inner complete"));
            *config = ESCAPED.with(|escaped| escaped.borrow_mut().take().unwrap());
            assert!(!config.explicit_config_owns_path("themeVariables.primaryColor"));
            CALLS.with(|calls| calls.borrow_mut().push("outer assign escaped clone"));
            true
        }

        for warm in [false, true] {
            let compatibility = Compatibility::new(json!({
                "theme": "base", "themeVariables": {"primaryColor": "#123456"}
            }));
            if warm {
                drop(compatibility.install(Engine::new(), Installation::Compiled));
            }
            let mut results = Vec::new();
            for installation in [Installation::Legacy, Installation::Compiled] {
                let mut outer =
                    Engine::new().with_theme_compatibility(compatibility.binding.clone());
                *outer.registry_mut() = DetectorRegistry::new();
                outer
                    .registry_mut()
                    .add_fn("gitGraph", replace_with_unfinished_clone);
                CALLS.with(|calls| calls.borrow_mut().clear());
                INNER.with(|inner| {
                    *inner.borrow_mut() = Some((
                        installation,
                        Compatibility {
                            binding: compatibility.binding.clone(),
                            plan: compatibility.plan.clone(),
                        },
                    ))
                });
                let result = outer.parse_metadata_sync("outer probe").unwrap();
                INNER.with(|inner| *inner.borrow_mut() = None);
                assert!(
                    result
                        .effective_config
                        .explicit_config_owns_path("themeVariables.primaryColor")
                );
                assert!(
                    !result
                        .effective_config
                        .explicit_config_owns_path("themeVariables.mainBkg")
                );
                CALLS.with(|calls| {
                    assert_eq!(
                        calls.borrow().as_slice(),
                        [
                            "outer enter",
                            "inner capture",
                            "inner complete",
                            "outer assign escaped clone",
                        ]
                    )
                });
                results.push(result);
            }
            assert_same_metadata(&results[0], &results[1], &compatibility.plan);
        }
    }
}
