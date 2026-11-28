#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::Duration;

use super::SourcePresentationMetadata;
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
use super::{ParsePipeline, ParserSourceMap};
use crate::config::presentation::SourcePresentationConfig;
use crate::{
    CancelReason, DetectorRegistry, Error, MermaidEngine, OperationCancelled, OperationControl,
    OperationControlResult, OperationPhase, ParseOptions, Result,
};
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
use crate::{
    DiagramSnapshotCapture, EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts,
    EditorSemanticKind, EditorSemanticSymbol, SourceSpan,
};
use merman_model::presentation::CustomJsonRenderModel;

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn panicking_detector(_source: &str, _config: &mut SourcePresentationConfig) -> bool {
    panic!("detector fixture panic")
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
static FRONTMATTER_PROBE_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn frontmatter_probe_detector(_source: &str, _config: &mut SourcePresentationConfig) -> bool {
    FRONTMATTER_PROBE_CALLS.fetch_add(1, Ordering::Relaxed);
    panic!("frontmatter probe detector must not run")
}

fn cancelling_render_parser(
    _code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<CustomJsonRenderModel>> {
    control.cancel();
    Ok(Err(Error::diagram_parse_fallback(
        meta.diagram_type.clone(),
        "custom parser failed after requesting cancellation",
    )))
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn expiring_render_parser(
    _code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<CustomJsonRenderModel>> {
    assert!(
        control.set_deadline(Duration::ZERO),
        "deadline fixture must install a fresh deadline"
    );
    Ok(Err(Error::diagram_parse_fallback(
        meta.diagram_type.clone(),
        "custom parser failed after expiring its deadline",
    )))
}

#[test]
fn operation_selects_appearance_once_with_or_without_rematerialization() {
    let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        serde_json::json!({
            "theme": "dark", "themeVariables": { "primaryColor": "#123456" },
        }),
    ));
    for (source, materializations, expected_theme) in [
        ("flowchart TD\nA-->B", 0, "dark"),
        (
            "---\nconfig:\n  theme: base\n---\nflowchart TD\nA-->B",
            1,
            "base",
        ),
        (
            "---\nconfig:\n  theme: 'null'\n---\nflowchart TD\nA-->B",
            0,
            "null",
        ),
    ] {
        let (metadata, work) =
            crate::config::measure_config_work(|| engine.parse_metadata_sync(source).unwrap());
        assert_eq!(
            metadata.effective_config.get_str("theme"),
            Some(expected_theme)
        );
        assert_eq!(work.appearance_selections, 1, "{work:?}");
        assert_eq!(
            work.selected_theme_materializations, materializations,
            "{work:?}"
        );
    }
}

#[test]
fn site_appearance_is_reused_by_engine_clones_and_ignores_source_overrides() {
    use serde_json::json;

    let host = SourcePresentationConfig::from_value(json!({
        "fontFamily": "Inter",
        "flowchart": { "nodeSpacing": 50 },
    }));
    let engine = MermaidEngine::new().with_site_config(host.clone());
    let clone = engine.clone();
    for (parser, source, materializations) in [
        (&engine, "flowchart TD\nA-->B", 1),
        (&clone, "flowchart TD\nA-->B", 0),
        (
            &clone,
            "---\nconfig:\n  look: classic\n---\nflowchart TD\nA-->B",
            1,
        ),
        (
            &clone,
            "---\nconfig:\n  theme: default\n---\nflowchart TD\nA-->B",
            1,
        ),
        (
            &clone,
            "---\nconfig:\n  theme: 'null'\n---\nflowchart TD\nA-->B",
            0,
        ),
        (&engine, "flowchart-elk TD\nA-->B", 0),
    ] {
        let (actual, work) = crate::config::measure_config_work(|| {
            parser.parse_metadata_sync(source).unwrap().effective_config
        });
        let reference = MermaidEngine::new()
            .with_site_config(host.clone())
            .parse_metadata_sync(source)
            .unwrap()
            .effective_config;
        assert_eq!(
            work.selected_theme_materializations, materializations,
            "{source}"
        );
        assert_eq!(actual, reference, "{source}");
        assert_eq!(
            serde_json::to_vec(actual.as_value()).unwrap(),
            serde_json::to_vec(reference.as_value()).unwrap(),
            "{source}"
        );
        assert!(actual.explicit_config_owns_path("flowchart.nodeSpacing"));
    }

    let changed = clone.with_exact_site_config(Some(SourcePresentationConfig::from_value(
        json!({ "flowchart": { "nodeSpacing": 61 } }),
    )));
    let (metadata, work) =
        crate::config::measure_config_work(|| changed.parse_metadata_sync("flowchart TD\nA-->B"));
    assert_eq!(work.selected_theme_materializations, 1);
    assert_eq!(
        metadata.unwrap().effective_config.as_value()["flowchart"]["nodeSpacing"],
        61
    );
}

#[test]
fn alternating_families_reuse_site_appearance_storage() {
    let site = SourcePresentationConfig::from_value(serde_json::json!({
        "flowchart": {"theme": "redux-color"},
        "sequence": {"theme": "dark"},
        "class": {"theme": "forest"},
    }));
    let sources = [
        ("flowchart TD\nA-->B", "redux-color"),
        ("sequenceDiagram\nAlice->>Bob: Hello", "dark"),
        ("classDiagram\nA <|-- B", "forest"),
    ];
    let engine = MermaidEngine::new().with_exact_site_config(Some(site));
    let (cold, cold_work) = crate::config::measure_config_work(|| {
        sources.map(|(source, theme)| {
            let config = engine.parse_metadata_sync(source).unwrap().effective_config;
            assert_eq!(config.get_str("theme"), Some(theme));
            config
        })
    });
    let (_, warm_work) = crate::config::measure_config_work(|| {
        for _ in 0..32 {
            for ((source, _), reference) in sources.iter().zip(&cold) {
                let config = engine.parse_metadata_sync(source).unwrap().effective_config;
                assert_eq!(&config, reference);
                assert!(
                    config
                        .as_config()
                        .shares_storage_with(reference.as_config())
                );
            }
        }
    });
    assert_eq!(cold_work.selected_theme_materializations, 3);
    assert_eq!(cold_work.json_cow_copies, 0);
    assert_eq!(warm_work.selected_theme_materializations, 0);
    assert_eq!(warm_work.json_cow_copies, 0);
    assert!(cold_work.authored_path_validations > 0);
    assert_eq!(warm_work.authored_path_validations, 0);
}

#[test]
fn flowchart_elk_layout_replay_keeps_the_cached_projection_shared() {
    let engine = MermaidEngine::new().with_exact_site_config(None);
    engine
        .parse_metadata_sync("classDiagram\nA <|-- B")
        .unwrap();
    let parse_elk = || {
        engine
            .parse_metadata_sync("flowchart-elk TD\nA-->B")
            .unwrap()
            .effective_config
    };

    let (cold, cold_work) = crate::config::measure_config_work(parse_elk);
    let (warm, warm_work) = crate::config::measure_config_work(parse_elk);
    assert_eq!(
        cold_work.selected_theme_materializations, 1,
        "{cold_work:?}"
    );
    assert_eq!(
        warm_work.selected_theme_materializations, 0,
        "{warm_work:?}"
    );
    assert_eq!(warm_work.json_cow_copies, 0, "{warm_work:?}");
    assert!(std::ptr::eq(cold.as_value(), warm.as_value()));
    assert_eq!(warm, cold);
    assert_eq!(warm.get_str("layout"), Some("elk"));
    assert!(warm.explicit_config_owns_path("layout"));
}

#[test]
fn disjoint_source_spacing_reuses_the_site_palette_and_keeps_its_assignment() {
    use super::{MermaidConfig, OperationConfigBuilder};
    use crate::config::AppearanceDecision;

    let engine = MermaidEngine::new().with_exact_site_config(None);
    let baseline = engine
        .parse_metadata_sync("flowchart TD\nA --> B\nA --> C")
        .unwrap()
        .effective_config;
    let defaults = crate::generated::upstream_default_config();
    for spacing in [72, 95] {
        let source = format!(
            "---\nconfig:\n  flowchart:\n    nodeSpacing: {spacing}\n---\nflowchart TD\nA --> B\nA --> C"
        );
        let (result, work) =
            crate::config::measure_config_work(|| engine.parse_metadata_sync(&source));
        let actual = result.unwrap().effective_config;
        assert_eq!(work.selected_theme_materializations, 0, "{work:?}");
        assert_eq!(
            actual
                .get_path(&["flowchart", "nodeSpacing"])
                .and_then(serde_json::Value::as_i64),
            Some(spacing),
        );
        assert!(actual.explicit_config_owns_path("flowchart.nodeSpacing"));
        assert!(!baseline.explicit_config_owns_path("flowchart.nodeSpacing"));

        let overrides = MermaidConfig::from_value(serde_json::json!({
            "flowchart": { "nodeSpacing": spacing },
        }));
        let decision = AppearanceDecision::resolve(
            "flowchart-v2",
            &overrides,
            engine.site_config_overrides.as_config(),
            &defaults,
        );
        let mut reference = OperationConfigBuilder::materialize_operation_theme(
            &engine,
            &decision,
            &overrides,
            crate::MermaidThemeId::ReduxColor,
        )
        .unwrap();
        decision.apply_to_presentation(&mut reference);
        assert_eq!(
            serde_json::to_vec(actual.as_value()).unwrap(),
            serde_json::to_vec(reference.as_value()).unwrap(),
        );
    }
}

#[test]
fn site_appearance_replays_detector_changes_even_on_a_cache_hit() {
    fn detector(source: &str, config: &mut SourcePresentationConfig) -> bool {
        config.set_value("flowchart.nodeSpacing", serde_json::json!(source.len()));
        config.set_value("themeVariables.primaryColor", serde_json::json!("#123456"));
        true
    }

    let host = SourcePresentationConfig::from_value(serde_json::json!({"fontFamily": "Inter"}));
    let make_engine = || {
        let mut engine = MermaidEngine::new().with_site_config(host.clone());
        *engine.registry_mut() = DetectorRegistry::new();
        engine.registry_mut().add_fn("flowchart-v2", detector);
        engine
    };
    let engine = make_engine();
    for (source, materializations) in [("first", 1), ("longer second", 0)] {
        let (actual, work) =
            crate::config::measure_config_work(|| engine.parse_metadata_sync(source));
        let actual = actual.unwrap().effective_config;
        let reference = make_engine()
            .parse_metadata_sync(source)
            .unwrap()
            .effective_config;
        assert_eq!(work.selected_theme_materializations, materializations);
        assert_eq!(actual, reference);
        assert_eq!(actual.as_value()["flowchart"]["nodeSpacing"], source.len());
        assert!(actual.explicit_config_owns_path("themeVariables.primaryColor"));
    }
}

#[test]
fn read_only_detection_borrows_the_source_section_without_cloning_the_full_config() {
    fn detector(_source: &str, config: &mut crate::DetectorConfig<'_>) -> bool {
        assert_eq!(
            config.get(&["theme"]).and_then(serde_json::Value::as_str),
            Some("default")
        );
        assert_eq!(
            config.get(&["layout"]).and_then(serde_json::Value::as_str),
            Some("dagre")
        );
        true
    }

    let source = "---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA-->B";
    let builtin = MermaidEngine::new();
    let mut custom = builtin.clone();
    *custom.registry_mut() = DetectorRegistry::new();
    custom
        .registry_mut()
        .add_config_fn("flowchart-v2", detector);
    let (builtin_metadata, builtin_work) =
        crate::config::measure_config_work(|| builtin.parse_metadata_sync(source).unwrap());
    let (custom_metadata, custom_work) =
        crate::config::measure_config_work(|| custom.parse_metadata_sync(source).unwrap());
    let builtin_config = builtin_metadata.effective_config;
    let custom_config = custom_metadata.effective_config;
    assert_eq!(builtin_config.get_str("theme"), Some("redux-color"));
    assert!(builtin_config.explicit_config_owns_path("layout"));
    assert!(custom_config.explicit_config_owns_path("layout"));
    assert_eq!(
        serde_json::to_vec(builtin_config.as_value()).unwrap(),
        serde_json::to_vec(custom_config.as_value()).unwrap()
    );
    assert_eq!(builtin_work.json_cow_copies, custom_work.json_cow_copies);
}

#[test]
fn source_and_detector_section_writes_keep_effective_ownership() {
    fn detect(text: &str, config: &mut crate::DetectorConfig<'_>) -> bool {
        assert!(text.starts_with("flowchart"));
        assert_eq!(
            config.get(&["flowchart", "nodeSpacing"]),
            Some(&serde_json::json!(72))
        );
        config.set("flowchart.rankSpacing", serde_json::json!(61));
        // A same-value assignment is observable as explicit ownership.
        config.set("flowchart.nodeSpacing", serde_json::json!(72));
        true
    }

    let mut engine = MermaidEngine::new();
    *engine.registry_mut() = DetectorRegistry::new();
    engine.registry_mut().add_config_fn("flowchart-v2", detect);
    let metadata = engine
        .parse_metadata_sync(
            "---\nconfig:\n  flowchart:\n    nodeSpacing: 72\n---\nflowchart TD\nA-->B",
        )
        .unwrap();
    let config = metadata.effective_config;
    assert_eq!(config.get_str("theme"), Some("redux-color"));
    for (field, value, path) in [
        ("nodeSpacing", 72, "flowchart.nodeSpacing"),
        ("rankSpacing", 61, "flowchart.rankSpacing"),
    ] {
        assert_eq!(
            config
                .get_path(&["flowchart", field])
                .and_then(serde_json::Value::as_u64),
            Some(value)
        );
        assert!(config.explicit_config_owns_path(path));
    }
}

#[test]
fn rematerialization_replays_detector_values_before_reapplying_authored_appearance() {
    fn detector(_source: &str, config: &mut SourcePresentationConfig) -> bool {
        config.set_value("flowchart.nodeSpacing", serde_json::json!(71));
        config.set_value("flowchart.theme", serde_json::json!("forest"));
        config.set_value("themeVariables.primaryColor", serde_json::json!("#abcdef"));
        true
    }
    let mut engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        serde_json::json!({
            "theme": "dark",
        }),
    ));
    *engine.registry_mut() = DetectorRegistry::new();
    engine.registry_mut().add_fn("flowchart-v2", detector);
    let (metadata, work) = crate::config::measure_config_work(|| {
        engine
            .parse_metadata_sync("---\nconfig:\n  theme: base\n---\nprobe")
            .unwrap()
    });
    let config = metadata.effective_config;
    assert_eq!(
        config.as_value()["flowchart"]["nodeSpacing"],
        serde_json::json!(71)
    );
    assert_eq!(
        config.get_str("themeVariables.primaryColor"),
        Some("#abcdef")
    );
    assert_eq!(config.get_str("theme"), Some("base"));
    assert_eq!(config.get_str("flowchart.theme"), Some("base"));
    assert!(config.explicit_config_owns_path("flowchart.nodeSpacing"));
    assert!(config.explicit_config_owns_path("themeVariables.primaryColor"));
    assert_eq!(work.appearance_selections, 1, "{work:?}");
    assert_eq!(work.selected_theme_materializations, 1, "{work:?}");
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn controlled_snapshot_stops_before_a_cancelled_operation() {
    let control = OperationControl::new();
    control.cancel();

    let result = MermaidEngine::new()
        .parse_diagram_snapshot_controlled_sync("flowchart TD\nA-->B\n", &control);

    assert!(matches!(result, Err(OperationCancelled { .. })));
}

#[test]
fn custom_render_parse_error_cannot_mask_callback_cancellation() {
    let mut engine = MermaidEngine::new();
    engine
        .render_diagram_registry_mut()
        .insert("flowchart-v2", cancelling_render_parser);
    let control = OperationControl::new();

    let error = engine
        .parse_diagram_for_render_model_with_type_controlled_sync(
            "flowchart-v2",
            "flowchart TD\nA -->\n",
            ParseOptions::lenient(),
            &control,
        )
        .expect_err("callback cancellation must win over a suppressible parse error");

    assert_eq!(error.phase, OperationPhase::Parse);
    assert_eq!(error.reason, CancelReason::Requested);
}

#[test]
fn ordinary_render_parsing_preserves_callback_cancellation() {
    fn cancelling_success(
        _code: &str,
        _meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<CustomJsonRenderModel>> {
        control.cancel();
        Ok(Ok(CustomJsonRenderModel::new(
            "cancelled",
            serde_json::json!({}),
        )))
    }

    for parser in [
        cancelling_render_parser as crate::CustomJsonRenderParser,
        cancelling_success,
    ] {
        let mut engine = MermaidEngine::new();
        engine
            .render_diagram_registry_mut()
            .insert("flowchart-v2", parser);
        for options in [ParseOptions::strict(), ParseOptions::lenient()] {
            for known_type in [false, true] {
                let result = if known_type {
                    engine.parse_diagram_for_render_model_with_type_sync(
                        "flowchart-v2",
                        "flowchart TD\nA --> B",
                        options,
                    )
                } else {
                    engine.parse_diagram_for_render_model_sync("flowchart TD\nA --> B", options)
                };
                let Error::OperationCancelled(error) =
                    result.expect_err("cancellation must never publish a model or error diagram")
                else {
                    panic!("callback cancellation must remain an operation error");
                };
                assert_eq!(error.phase, OperationPhase::Parse);
                assert_eq!(error.reason, CancelReason::Requested);
            }
        }
    }
}

#[cfg(feature = "compat-json")]
#[test]
fn ordinary_json_parsing_preserves_callback_cancellation() {
    fn cancelling_parser(
        _code: &str,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<serde_json::Value>> {
        control.cancel();
        Ok(Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "cancelled JSON parser",
        )))
    }
    fn cancelling_success(
        _code: &str,
        _meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<serde_json::Value>> {
        control.cancel();
        Ok(Ok(serde_json::json!({})))
    }

    for parser in [
        cancelling_parser as crate::DiagramSemanticParser,
        cancelling_success,
    ] {
        let mut engine = MermaidEngine::new();
        engine.diagram_registry_mut().insert("flowchart-v2", parser);
        for options in [ParseOptions::strict(), ParseOptions::lenient()] {
            for known_type in [false, true] {
                let result = if known_type {
                    engine.parse_diagram_with_type_sync(
                        "flowchart-v2",
                        "flowchart TD\nA --> B",
                        options,
                    )
                } else {
                    engine.parse_diagram_sync("flowchart TD\nA --> B", options)
                };
                let Error::OperationCancelled(error) =
                    result.expect_err("cancellation must never publish JSON or an error diagram")
                else {
                    panic!("callback cancellation must remain an operation error");
                };
                assert_eq!(error.phase, OperationPhase::Parse);
                assert_eq!(error.reason, CancelReason::Requested);
            }
        }
    }
}

#[test]
fn render_entrypoints_preserve_direct_outer_cancellation() {
    fn cancelled<T>(
        _code: &str,
        _meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<T>> {
        assert!(!control.is_cancelled());
        Err(OperationCancelled {
            phase: OperationPhase::Parse,
            reason: CancelReason::Requested,
        })
    }

    let mut render_engine = MermaidEngine::new();
    render_engine
        .render_diagram_registry_mut()
        .insert("flowchart-v2", cancelled::<CustomJsonRenderModel>);
    let engines = [
        (false, render_engine),
        #[cfg(feature = "compat-json")]
        (true, {
            let mut engine = MermaidEngine::new();
            engine
                .diagram_registry_mut()
                .insert("flowchart-v2", cancelled::<serde_json::Value>);
            engine
        }),
    ];
    let expected = OperationCancelled {
        phase: OperationPhase::Parse,
        reason: CancelReason::Requested,
    };
    let source = "flowchart TD\nA --> B";
    for (semantic_fallback, engine) in engines {
        for options in [ParseOptions::strict(), ParseOptions::lenient()] {
            for known_type in [false, true] {
                let ordinary = if known_type {
                    engine.parse_diagram_for_render_model_with_type_sync(
                        "flowchart-v2",
                        source,
                        options,
                    )
                } else {
                    engine.parse_diagram_for_render_model_sync(source, options)
                };
                let Error::OperationCancelled(error) =
                    ordinary.expect_err("outer cancellation must not become an error diagram")
                else {
                    panic!("outer cancellation must remain an operation error");
                };
                assert_eq!(error, expected, "semantic_fallback={semantic_fallback}");

                let control = OperationControl::new();
                let controlled = if known_type {
                    engine.parse_diagram_for_render_model_with_type_controlled_sync(
                        "flowchart-v2",
                        source,
                        options,
                        &control,
                    )
                } else {
                    engine.parse_diagram_for_render_model_controlled_sync(source, options, &control)
                };
                assert_eq!(
                    controlled.expect_err("cancellation must remain in the outer result"),
                    expected,
                    "semantic_fallback={semantic_fallback}"
                );
                assert!(!control.is_cancelled());

                #[cfg(feature = "compat-json")]
                if semantic_fallback {
                    let json = if known_type {
                        engine.parse_diagram_with_type_sync("flowchart-v2", source, options)
                    } else {
                        engine.parse_diagram_sync(source, options)
                    };
                    let Error::OperationCancelled(error) =
                        json.expect_err("outer cancellation must not publish error JSON")
                    else {
                        panic!("JSON must preserve the operation cancellation");
                    };
                    assert_eq!(error, expected);
                }
            }
        }
    }
}

#[test]
fn render_entrypoints_use_the_caller_owned_runtime_context() {
    fn observe_context(
        _code: &str,
        _meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<CustomJsonRenderModel>> {
        control.checkpoint()?;
        Ok(Ok(CustomJsonRenderModel::new(
            "runtime-context",
            serde_json::json!({
                "today": crate::runtime::today_local().to_string(),
                "id": crate::runtime::generated_id_hex("parse-pipeline.context", 0, 16),
            }),
        )))
    }

    let context = merman_model::runtime::RuntimePolicy::deterministic()
        .with_fixed_today(crate::time::CivilDate::new(2030, 1, 2))
        .with_fixed_seed(73)
        .begin_operation()
        .unwrap();
    let mut engine = MermaidEngine::new().with_runtime_policy(
        merman_model::runtime::RuntimePolicy::deterministic()
            .with_fixed_today(crate::time::CivilDate::new(1999, 12, 31))
            .with_fixed_seed(11),
    );
    engine
        .render_diagram_registry_mut()
        .insert("flowchart-v2", observe_context);
    let expected = serde_json::json!({
        "today": context.today_local().to_string(),
        "id": context.derive_hex("parse-pipeline.context", 0, 16),
    });
    let source = "flowchart TD\nA --> B";
    for known_type in [false, true] {
        let control = OperationControl::new();
        let parsed = if known_type {
            engine.parse_diagram_for_render_model_with_type_controlled_in_context_sync(
                "flowchart-v2",
                source,
                ParseOptions::strict(),
                &control,
                &context,
            )
        } else {
            engine.parse_diagram_for_render_model_controlled_in_context_sync(
                source,
                ParseOptions::strict(),
                &control,
                &context,
            )
        }
        .unwrap()
        .unwrap()
        .unwrap();
        let crate::RenderSemanticModel::CustomJson(model) = parsed.model() else {
            panic!("the runtime context observer must produce its custom model");
        };
        assert_eq!(model.value(), &expected);
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#[test]
fn custom_render_parse_error_cannot_mask_callback_deadline() {
    let mut engine = MermaidEngine::new();
    engine
        .render_diagram_registry_mut()
        .insert("flowchart-v2", expiring_render_parser);
    let control = OperationControl::new();

    let error = engine
        .parse_diagram_for_render_model_with_type_controlled_sync(
            "flowchart-v2",
            "flowchart TD\nA -->\n",
            ParseOptions::lenient(),
            &control,
        )
        .expect_err("callback deadline must win over a suppressible parse error");

    assert_eq!(error.phase, OperationPhase::Parse);
    assert_eq!(error.reason, CancelReason::DeadlineExceeded);
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn controlled_capture_cancellation_never_returns_partial_evidence() {
    let control = OperationControl::new();
    control.cancel();

    let result = MermaidEngine::new().capture_diagram_snapshot_controlled_sync(
        "%%{init: {theme: 'dark'}}%%\nflowchart TD\nA-->B\n",
        &control,
    );

    assert!(matches!(result, Err(OperationCancelled { .. })));
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn editor_capture_retains_source_config_on_detection_failure() {
    let source = "%%{ initialize: { theme: 'dark' } }%%\nnot a diagram\n";
    let captured = MermaidEngine::new()
        .capture_diagram_snapshot_controlled_sync(source, &OperationControl::new())
        .expect("an active parse control must not cancel");

    let DiagramSnapshotCapture::Failed {
        error: Error::DetectType(_),
        source_config,
    } = captured
    else {
        panic!("strict detection failure must retain preprocessing evidence");
    };
    assert_eq!(source_config.directives().len(), 1);
    let keyword = source_config.directives()[0].keyword_span();
    assert_eq!(&source[keyword.start..keyword.end], "initialize");
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn editor_capture_retains_source_config_when_init_config_detection_panics() {
    let source = concat!(
        "%%{ initialize: { lazyLoadedDiagrams: true, config: { htmlLabels: false } } }%%\n",
        "detector-panic-fixture\n",
    );
    let mut engine = MermaidEngine::new();
    *engine.registry_mut() = DetectorRegistry::new();
    engine
        .registry_mut()
        .add_fn("detector-panic-fixture", panicking_detector);

    let captured = engine
        .capture_diagram_snapshot_controlled_sync(source, &OperationControl::new())
        .expect("an active parse control must not cancel");

    let DiagramSnapshotCapture::Panicked {
        panic,
        source_config,
    } = captured
    else {
        panic!("detector panic must remain a capture outcome");
    };
    assert!(panic.message().contains("detector fixture panic"));
    assert_eq!(source_config.directives().len(), 1);
    let directive = &source_config.directives()[0];
    assert_eq!(directive.keyword(), "initialize");
    assert!(
        source_config
            .keys()
            .iter()
            .any(|entry| entry.matches_path(&["lazyLoadedDiagrams"]))
    );
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn legacy_snapshot_facade_resumes_the_original_detector_panic_payload() {
    let source = "detector-panic-fixture\n";
    let mut engine = MermaidEngine::new();
    *engine.registry_mut() = DetectorRegistry::new();
    engine
        .registry_mut()
        .add_fn("detector-panic-fixture", panicking_detector);

    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = engine.parse_diagram_snapshot_sync(source);
    }))
    .expect_err("the legacy snapshot facade must resume detector panics");

    assert_eq!(
        payload.downcast_ref::<&'static str>(),
        Some(&"detector fixture panic")
    );
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn editor_capture_retains_later_source_config_on_preprocessing_failure() {
    let source = concat!(
        "---\n",
        "config: [\n",
        "---\n",
        "%%{ initialize: { theme: 'dark' } }%%\n",
        "flowchart TD\nA-->B\n",
    );
    let captured = MermaidEngine::new()
        .capture_diagram_snapshot_controlled_sync(source, &OperationControl::new())
        .expect("an active parse control must not cancel");

    let DiagramSnapshotCapture::Failed {
        error: Error::InvalidFrontMatterYaml { .. },
        source_config,
    } = captured
    else {
        panic!("preprocessing failure must be distinct from cancellation");
    };
    assert_eq!(source_config.directives().len(), 1);
    assert_eq!(source_config.directives()[0].keyword(), "initialize");
    assert!(!source_config.rewrite_safe());
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn editor_capture_preserves_frontmatter_error_before_later_detector_panic() {
    let source = concat!(
        "---\n",
        "config: [\n",
        "---\n",
        "%%{ initialize: { config: { htmlLabels: false } } }%%\n",
        "detector-panic-fixture\n",
    );
    let mut engine = MermaidEngine::new();
    *engine.registry_mut() = DetectorRegistry::new();
    FRONTMATTER_PROBE_CALLS.store(0, Ordering::Relaxed);
    engine
        .registry_mut()
        .add_fn("detector-panic-fixture", frontmatter_probe_detector);

    let captured = engine
        .capture_diagram_snapshot_controlled_sync(source, &OperationControl::new())
        .expect("an active parse control must not cancel");

    let DiagramSnapshotCapture::Failed {
        error: Error::InvalidFrontMatterYaml { .. },
        source_config,
    } = captured
    else {
        panic!("the first preprocessing error must win over later detector work");
    };
    assert_eq!(source_config.directives().len(), 1);
    assert_eq!(source_config.directives()[0].keyword(), "initialize");
    assert!(!source_config.rewrite_safe());
    assert_eq!(FRONTMATTER_PROBE_CALLS.load(Ordering::Relaxed), 0);
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn lenient_editor_capture_preserves_suppressed_detection_semantics() {
    let engine = MermaidEngine::new();
    let pipeline = ParsePipeline::detect(&engine, "not a diagram\n", ParseOptions::lenient());
    let captured = pipeline
        .capture_editor_snapshot_controlled(super::ParseTiming::Json, &OperationControl::new())
        .expect("an active parse control must not cancel");

    assert!(matches!(captured, DiagramSnapshotCapture::Snapshot(None)));
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "diagram-flowchart")]
#[cfg(feature = "editor-facts")]
fn active_control_preserves_the_snapshot_model() {
    let source = "flowchart TD\nA-->B\n";
    let engine = MermaidEngine::new();
    let regular = engine
        .parse_diagram_snapshot_sync(source)
        .expect("regular parse succeeds")
        .expect("regular snapshot");
    let controlled = engine
        .parse_diagram_snapshot_controlled_sync(source, &OperationControl::new())
        .expect("active control")
        .expect("controlled parse succeeds")
        .expect("controlled snapshot");

    assert_eq!(
        controlled.metadata().diagram_type,
        regular.metadata().diagram_type
    );
    assert_eq!(
        controlled.outcome().parsed_model(),
        regular.outcome().parsed_model()
    );
}

#[test]
fn public_and_render_entrypoints_do_not_reprocess_late_frontmatter() {
    let input = concat!(
        "   ---\n",
        "title: only-visible-after-trimming\n",
        "---\n",
        "flowchart TD\n",
        "A-->B\n",
    );
    let engine = MermaidEngine::new();
    let preprocessed =
        crate::preprocess::preprocess_diagram(input, &engine.registry).expect("single preprocess");
    assert!(preprocessed.code().starts_with("---"));
    assert!(preprocessed.title.is_none());
    assert!(matches!(
        engine.parse_metadata_sync(input),
        Err(Error::MalformedFrontMatter)
    ));
    for options in [ParseOptions::strict(), ParseOptions::lenient()] {
        assert!(matches!(
            engine.parse_diagram_for_render_model_sync(input, options),
            Err(Error::MalformedFrontMatter)
        ));
        assert!(matches!(
            engine.parse_diagram_for_render_model_with_type_sync("flowchart-v2", input, options),
            Err(Error::MalformedFrontMatter)
        ));
    }
}

#[test]
fn render_rejects_a_second_frontmatter_block() {
    let input = concat!(
        "---\n",
        "title: outer\n",
        "---\n",
        "---\n",
        "title: inner\n",
        "---\n",
        "flowchart TD\n",
        "A-->B\n",
    );
    let engine = MermaidEngine::new();

    let error = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect_err("render preprocessing must leave the second block visible");

    assert!(matches!(error, crate::Error::MalformedFrontMatter));
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn fact_remap_drops_only_the_fact_crossing_a_deleted_span() {
    let original = "flowchart TD\nA%%{wrap}%%B\nC\n";
    let engine = MermaidEngine::new();
    let preprocessed =
        crate::preprocess::preprocess_diagram_with_known_type(original, &engine.registry, None)
            .unwrap()
            .source;
    let map = ParserSourceMap::new(&preprocessed);
    let mut facts = EditorSemanticFacts::new();
    let joined_start = preprocessed.text().find("AB").unwrap();
    facts.push_symbol(EditorSemanticSymbol::new(
        "AB",
        None,
        EditorSemanticKind::Variable,
        SourceSpan::new(joined_start, joined_start + 2),
        SourceSpan::new(joined_start, joined_start + 2),
    ));
    let c = preprocessed.text().find('C').unwrap();
    facts.push_symbol(EditorSemanticSymbol::new(
        "C",
        None,
        EditorSemanticKind::Variable,
        SourceSpan::new(c, c + 1),
        SourceSpan::new(c, c + 1),
    ));
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        SourceSpan::new(c, c + 1),
    ));

    map.source
        .remap_editor_facts_controlled(&mut facts, &OperationControl::new())
        .unwrap();

    assert_eq!(facts.symbols.len(), 1);
    assert_eq!(facts.symbols[0].name, "C");
    assert_eq!(
        &original[facts.symbols[0].span.start..facts.symbols[0].span.end],
        "C"
    );
    assert_eq!(facts.expected_syntax.len(), 1);
    assert!(
        facts
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("dropped 1 editor fact span"))
    );
}

#[test]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn fact_remap_observes_cancellation_during_large_fact_batches() {
    let original = "flowchart TD\nA\n";
    let engine = MermaidEngine::new();
    let preprocessed =
        crate::preprocess::preprocess_diagram_with_known_type(original, &engine.registry, None)
            .unwrap()
            .source;
    let map = ParserSourceMap::new(&preprocessed);
    let mut facts = EditorSemanticFacts::new();
    let offset = preprocessed.text().find('A').unwrap();
    for index in 0..256 {
        facts.push_symbol(EditorSemanticSymbol::new(
            format!("node-{index}"),
            None,
            EditorSemanticKind::Variable,
            SourceSpan::new(offset, offset + 1),
            SourceSpan::new(offset, offset + 1),
        ));
    }
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);

    assert!(matches!(
        map.source
            .remap_editor_facts_controlled(&mut facts, &control),
        Err(crate::OperationCancelled { .. })
    ));
    assert!(control.is_cancelled());
}
