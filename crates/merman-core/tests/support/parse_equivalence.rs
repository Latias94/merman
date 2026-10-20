//! Shared assertions for stable parser contracts and the in-process parse fuzzer.

use super::parse_api::{
    Engine, Error, ManagedSemanticJson, MermaidConfig, OperationControl, ParseMetadata,
    ParseOptions, ParsedDiagram, ParsedDiagramRender, Result, diagram_family_capabilities,
    runtime::RuntimePolicy, time::CivilDate,
};

pub fn deterministic_engine() -> Engine {
    let policy = RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(0)
        .expect("valid UTC offset")
        .with_fixed_today(CivilDate::new(2025, 1, 1))
        .with_fixed_unix_millis(1_735_689_600_000);
    Engine::new().with_runtime_policy(policy)
}

fn managed_config(config: &MermaidConfig) -> ManagedSemanticJson {
    config
        .clone_value_bounded_controlled(usize::MAX, usize::MAX, &OperationControl::new())
        .expect("private control is not cancelled")
        .expect("unbounded config clone")
        .into()
}

fn assert_metadata(semantic: &ParseMetadata, typed: &ParseMetadata, source: &str) {
    assert_eq!(semantic.diagram_type, typed.diagram_type, "{source}");
    assert_eq!(semantic.title, typed.title, "{source}");
    assert_eq!(
        managed_config(&semantic.config),
        managed_config(&typed.config),
        "authored config: {source}"
    );
    assert_eq!(
        managed_config(&semantic.effective_config),
        managed_config(&typed.effective_config),
        "effective config: {source}"
    );
}

fn error_diagram_type(error: &Error) -> Option<&str> {
    match error {
        Error::DiagramParse { diagram_type, .. } | Error::UnsupportedDiagram { diagram_type } => {
            Some(diagram_type)
        }
        _ => None,
    }
}

fn assert_error_category(semantic: &Error, typed: &Error, source: &str) {
    assert_eq!(
        std::mem::discriminant(semantic),
        std::mem::discriminant(typed),
        "different error categories: {semantic} / {typed}\n{source}"
    );
    assert_eq!(
        error_diagram_type(semantic),
        error_diagram_type(typed),
        "different error diagram identities: {source}"
    );
}

fn outcome<T>(result: &Result<Option<T>>) -> &'static str {
    match result {
        Ok(Some(_)) => "accepted diagram",
        Ok(None) => "no diagram",
        Err(_) => "error",
    }
}

pub fn assert_pair(
    semantic: &Result<Option<ParsedDiagram>>,
    typed: &Result<Option<ParsedDiagramRender>>,
    source: &str,
) {
    match (semantic, typed) {
        (Ok(Some(semantic)), Ok(Some(typed))) => {
            assert_metadata(&semantic.meta, typed.metadata(), source);
            let capability = diagram_family_capabilities()
                .iter()
                .find(|fact| fact.diagram_type == typed.metadata().diagram_type)
                .expect("built-in parse identity belongs to the family catalog");
            assert!(
                capability.has_semantic_parser && capability.has_render_parser,
                "{source}"
            );
            assert_eq!(
                Some(typed.model().kind()),
                capability.render_model_kind,
                "catalog model kind: {source}"
            );
            let projected = typed
                .model()
                .compatibility_json(typed.metadata())
                .unwrap_or_else(|error| {
                    panic!("accepted typed model has no lossless projection: {error}\n{source}")
                });
            assert_eq!(
                semantic.model, projected,
                "complete semantic projection: {source}"
            );
        }
        (Ok(None), Ok(None)) => {}
        (Err(semantic), Err(typed)) => assert_error_category(semantic, typed, source),
        _ => panic!(
            "different parser acceptance: semantic={}, typed={}\n{source}",
            outcome(semantic),
            outcome(typed)
        ),
    }
}

pub fn assert_parse_paths(engine: &Engine, source: &str) {
    let strict = engine.parse_diagram_sync(source, ParseOptions::strict());
    let strict_typed = engine.parse_diagram_for_render_model_sync(source, ParseOptions::strict());
    assert_pair(&strict, &strict_typed, source);
    let lenient = engine.parse_diagram_sync(source, ParseOptions::lenient());
    let lenient_typed = engine.parse_diagram_for_render_model_sync(source, ParseOptions::lenient());
    assert_pair(&lenient, &lenient_typed, source);

    match (&strict, &lenient) {
        (Ok(Some(strict)), Ok(Some(lenient))) => {
            assert_metadata(&strict.meta, &lenient.meta, source);
            assert_eq!(
                strict.model, lenient.model,
                "lenient changed accepted semantics: {source}"
            );
        }
        (Ok(None), Ok(None)) => {}
        (Err(Error::DiagramParse { .. } | Error::UnsupportedDiagram { .. }), Ok(Some(lenient))) => {
            assert_eq!(lenient.meta.diagram_type, "error", "{source}");
        }
        // Lenient preprocessing intentionally recovers incomplete directive lines. Strict
        // removal can consume the remaining source and fail detection; recovery can expose
        // a diagram or a later error. Pure unknown input has its own stable contract test.
        (Err(Error::InvalidDirectiveJson { .. } | Error::DetectType(_)), _) => {}
        (Err(strict), Err(lenient)) => assert_error_category(strict, lenient, source),
        _ => panic!(
            "unexpected strict-to-lenient transition: strict={}, lenient={}\n{source}",
            outcome(&strict),
            outcome(&lenient)
        ),
    }
}
