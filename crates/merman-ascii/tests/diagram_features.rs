//! Availability contracts for isolated ASCII consumers.
use merman_ascii::{AsciiError, AsciiRenderOptions, AsciiRenderer, AsciiResourcePolicy};
use merman_core::{Engine, ParseOptions};

mod support;

#[test]
fn capabilities_follow_compiled_logical_families() {
    let capabilities = merman_ascii::ascii_capabilities();
    let families = merman_core::diagram_family_capabilities();
    for capability in capabilities {
        assert!(
            families.iter().any(|family| {
                merman_core::diagram_type_metadata_id(family.logical_family_kind)
                    .unwrap_or(family.logical_family_kind)
                    == capability.diagram_type
                    && family.has_semantic_parser
            }),
            "unexpected capability row {}",
            capability.diagram_type
        );
    }
    for family in families
        .iter()
        .filter(|family| family.has_semantic_parser && family.logical_family_kind != "error")
    {
        let id = merman_core::diagram_type_metadata_id(family.logical_family_kind)
            .unwrap_or(family.logical_family_kind);
        assert_eq!(
            capabilities
                .iter()
                .filter(|capability| capability.diagram_type == id)
                .count(),
            1
        );
    }
    assert_eq!(
        merman_ascii::ascii_supported_diagram_types().contains(&"flowchart"),
        cfg!(feature = "diagram-flowchart")
    );
    assert_eq!(
        merman_ascii::ascii_supported_diagram_types().contains(&"swimlane"),
        cfg!(feature = "diagram-swimlane")
    );
    assert_eq!(
        merman_ascii::ascii_supported_diagram_types().contains(&"sequence"),
        cfg!(feature = "diagram-sequence")
    );
}

#[test]
fn custom_models_stay_non_renderable_and_cancellation_wins() {
    let mut engine = Engine::new();
    engine
        .diagram_registry_mut()
        .insert("sequence", |_, _, control| {
            control.checkpoint()?;
            Ok(Ok(serde_json::json!({"custom": true})))
        });
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nA->>B: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let renderer = AsciiRenderer::new(AsciiRenderOptions::default()).unwrap();
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .unwrap();
    let control = merman_core::OperationControl::new();
    assert!(matches!(
        renderer.render_model_report(
            &support::full_model::view(parsed.model()),
            support::full_model::source(parsed.metadata()),
            merman_ascii::AsciiViewportPolicy::unrestricted(),
            &control,
            &context,
            AsciiResourcePolicy::default(),
        ),
        Err(AsciiError::UnsupportedDiagram { .. })
    ));
    control.cancel();
    assert!(matches!(
        renderer.render_model_report(
            &support::full_model::view(parsed.model()),
            support::full_model::source(parsed.metadata()),
            merman_ascii::AsciiViewportPolicy::unrestricted(),
            &control,
            &context,
            AsciiResourcePolicy::default(),
        ),
        Err(AsciiError::Cancelled(_))
    ));
}

#[test]
fn selected_graph_languages_render_independently() {
    let parser = merman_core::SemanticParser::new();
    let renderer = AsciiRenderer::new(AsciiRenderOptions::default()).unwrap();
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .unwrap();
    for (enabled, source, family) in [
        (
            cfg!(feature = "diagram-flowchart"),
            "flowchart LR\nA[Start] --> B[Finish]\n",
            "flowchart",
        ),
        (
            cfg!(feature = "diagram-swimlane"),
            "swimlane-beta LR\nA[Start] --> B[Finish]\n",
            "swimlane",
        ),
        (
            cfg!(feature = "diagram-state"),
            "stateDiagram-v2\nStart --> Finish\n",
            "state",
        ),
    ] {
        if !enabled {
            continue;
        }
        let parsed = parser.parse_diagram_sync(source).unwrap();
        let rendered = renderer
            .render_parsed_report(
                &parsed,
                merman_ascii::AsciiViewportPolicy::unrestricted(),
                &merman_core::OperationControl::new(),
                &context,
                AsciiResourcePolicy::default(),
            )
            .unwrap();
        assert!(rendered.text.contains("Start"), "{family}");
        assert!(rendered.text.contains("Finish"), "{family}");
    }
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_report_keeps_logical_family_through_fallback() {
    let parsed = merman_core::SemanticParser::new()
        .parse_diagram_sync("swimlane-beta LR\nA[Start] --> B[Finish]\n")
        .unwrap();
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .unwrap();
    let report = AsciiRenderer::default()
        .render_parsed_report(
            &parsed,
            merman_ascii::AsciiViewportPolicy::with_max_width(4)
                .overflow(merman_ascii::OverflowPolicy::Fallback),
            &merman_core::OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap();
    assert_eq!(report.family, "swimlane");
}

#[cfg(feature = "diagram-git-graph")]
#[test]
fn git_graph_metadata_and_model_identities_share_one_capability() {
    assert!(merman_ascii::ascii_supported_diagram_types().contains(&"gitgraph"));
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "gitGraph\ncommit id:\"first\"\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let renderer = AsciiRenderer::default();
    let context = engine.begin_operation().unwrap();
    let control = merman_core::OperationControl::new();
    let resources = AsciiResourcePolicy::default();
    let canonical = merman_core::SemanticParser::new()
        .parse_diagram_sync("gitGraph\ncommit id:\"first\"\n")
        .unwrap();
    assert!(
        renderer
            .render_parsed(&canonical, &control, &context, resources)
            .unwrap()
            .contains("first")
    );
    assert!(
        renderer
            .render_model(
                &support::full_model::view(parsed.model()),
                &control,
                &context,
                resources
            )
            .unwrap()
            .contains("first")
    );
}
