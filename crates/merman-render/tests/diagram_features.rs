//! Focused family-selection checks, also runnable with independently widened core features.

use merman_core::{Engine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-usecase",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-mindmap"
))]
use merman_render::family::RenderFamilyKind;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-usecase",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-mindmap"
))]
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{Error, LayoutOptions};

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-usecase",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-mindmap"
))]
fn parse(source: &str) -> merman_core::ParsedDiagramRender {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
}

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-usecase",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-mindmap"
))]
fn render(source: &str) -> (RenderFamilyKind, String) {
    let parsed = parse(source);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(family::plan_render(&parsed, &session).unwrap().is_ready());
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let kind = artifact.family_kind();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    (kind, svg)
}

#[test]
fn local_handler_projection_preserves_logical_language_ownership() {
    for (id, enabled) in [
        ("agentflow", cfg!(feature = "diagram-agentflow")),
        ("usecase", cfg!(feature = "diagram-usecase")),
        ("flowchart-v2", cfg!(feature = "diagram-flowchart")),
        ("flowchart-elk", cfg!(feature = "diagram-flowchart")),
        ("swimlane", cfg!(feature = "diagram-swimlane")),
        ("mindmap", cfg!(feature = "diagram-mindmap")),
        ("sequence", cfg!(feature = "diagram-sequence")),
        ("gantt", cfg!(feature = "diagram-gantt")),
    ] {
        assert_eq!(family::supports_diagram_type(id), enabled, "{id}");
    }
    assert!(family::supports_diagram_type("error"));
    assert!(!family::supports_diagram_type("unknown"));
}

#[test]
fn widened_core_cannot_bypass_local_handlers_or_precedence() {
    for source in [
        "agentflow-beta TB\nA[Alpha] --> B[Beta]",
        "usecase-beta\nactor Customer(\"Customer\")\nCheckout(\"Place order\")\nCustomer --> Checkout",
        "flowchart TD\nA --> B",
        "swimlane-beta LR\nA --> B",
        "sequenceDiagram\nA->>B: message",
        "mindmap\n  root((Root))",
        "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2026-01-01, 1d",
    ] {
        let Ok(Some(parsed)) =
            Engine::new().parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        else {
            continue;
        };
        if family::supports_diagram_type(&parsed.metadata().diagram_type) {
            continue;
        }
        // The unavailable local handler precedes both model-resource and backend planning.
        let policy = merman_render::RenderResourcePolicy::default()
            .with_limit(merman_render::ResourceLimitId::MaxModelItems, 1)
            .unwrap();
        let environment = RenderEnvironment::deterministic().with_resource_policy(policy);
        let session = environment.begin_session().unwrap();
        assert!(matches!(
            family::plan_render(&parsed, &session),
            Err(Error::UnsupportedDiagram { .. })
        ));
        assert!(matches!(
            family::prepare(parsed, &LayoutOptions::default(), session),
            Err(Error::UnsupportedDiagram { .. })
        ));
    }
}

#[cfg(all(feature = "diagram-flowchart", feature = "diagram-gantt"))]
#[test]
fn selected_embedding_renders_flowchart_and_gantt() {
    let (kind, svg) =
        render("---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA[Alpha] --> B[Beta]");
    assert_eq!(kind, RenderFamilyKind::Flowchart);
    assert!(svg.contains("Alpha"));
    assert!(svg.contains("Beta"));
    let (kind, svg) = render("gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2026-01-01, 1d");
    assert_eq!(kind, RenderFamilyKind::Gantt);
    assert!(svg.contains("Task"));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_preserves_swimlane_layout_without_admitting_swimlane_language() {
    let (kind, svg) = render("---\nconfig:\n  layout: swimlane\n---\nflowchart LR\nA --> B");
    assert_eq!(kind, RenderFamilyKind::Swimlane);
    assert!(svg.contains("<svg"));
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_preserves_dagre_and_elk_routes() {
    let (kind, svg) = render("---\nconfig:\n  layout: dagre\n---\nswimlane-beta LR\nA --> B");
    assert_eq!(kind, RenderFamilyKind::Flowchart);
    assert!(svg.contains("<svg"));
    let parsed = parse("---\nconfig:\n  layout: elk\n---\nswimlane-beta LR\nA --> B");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let plan = family::plan_render(&parsed, &session).unwrap();
    let expected: &[merman_render::RenderCapability] = if cfg!(feature = "layout-elk") {
        &[merman_render::RenderCapability::LayoutElk]
    } else {
        &[]
    };
    assert_eq!(plan.required_capabilities(), expected);
    assert!(plan.is_ready());
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    assert_eq!(artifact.family_kind(), RenderFamilyKind::Flowchart);
}

#[cfg(feature = "diagram-mindmap")]
#[test]
fn mindmap_uses_shared_label_measurement_without_flowchart_implementation() {
    let (kind, svg) =
        render("---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  root[Root]\n    child[Child]");
    assert_eq!(kind, RenderFamilyKind::Mindmap);
    assert!(svg.contains("Root"));
    assert!(svg.contains("Child"));
}

#[test]
fn custom_provenance_and_cancellation_precede_family_availability() {
    fn custom(
        _: &str,
        _: &merman_core::ParseMetadata,
        control: &merman_core::OperationControl,
    ) -> merman_core::OperationControlResult<merman_core::Result<merman_core::CustomJsonRenderModel>>
    {
        control.checkpoint()?;
        Ok(Ok(merman_core::CustomJsonRenderModel::new(
            "custom",
            serde_json::json!({}),
        )))
    }
    let mut engine = Engine::new();
    engine
        .render_diagram_registry_mut()
        .insert("flowchart-v2", custom);
    let parsed = engine
        .parse_diagram_for_render_model_with_type_sync(
            "flowchart-v2",
            "flowchart TD\nA --> B",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(Error::NonRenderableCustomModel { .. })
    ));
    let control = merman_core::OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(Error::Cancelled(_))
    ));
    assert!(matches!(
        family::prepare(parsed, &LayoutOptions::default(), session),
        Err(Error::Cancelled(_))
    ));
}

#[cfg(feature = "diagram-mindmap")]
#[test]
fn mindmap_declined_math_measurement_keeps_markdown_fallback() {
    let parsed = parse(
        "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  root[Root]\n    child[\"**Bold** $$x$$\"]",
    );
    let session = RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(merman_render::math::NoopMathRenderer))
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let layout = artifact.layout_json().unwrap();
    let nodes = layout["layout"]["MindmapDiagram"]["nodes"]
        .as_array()
        .unwrap();
    assert_eq!(nodes.len(), 2);
    assert!(
        nodes
            .iter()
            .all(|node| node["width"].as_f64().unwrap() > 0.0)
    );
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn agentflow_renders_with_independent_family_selection() {
    let (kind, svg) =
        render("---\nconfig:\n  layout: dagre\n---\nagentflow-beta TB\nA[Alpha] --> B[Beta]");
    assert_eq!(kind, RenderFamilyKind::Agentflow);
    assert!(svg.contains("Alpha"));
    assert!(svg.contains("Beta"));
    assert_eq!(
        family::supports_diagram_type("flowchart-v2"),
        cfg!(feature = "diagram-flowchart")
    );
    assert_eq!(
        family::supports_diagram_type("swimlane"),
        cfg!(feature = "diagram-swimlane")
    );
}

#[cfg(feature = "diagram-usecase")]
#[test]
fn usecase_renders_with_independent_family_selection() {
    let (kind, svg) = render(
        "---\nconfig:\n  layout: dagre\n---\nusecase-beta\nactor Customer(\"Customer\")\nCheckout(\"Place order\")\nCustomer --> Checkout",
    );
    assert_eq!(kind, RenderFamilyKind::Usecase);
    assert!(svg.contains("Customer"));
    assert!(svg.contains("Place order"));
    assert_eq!(
        family::supports_diagram_type("flowchart-v2"),
        cfg!(feature = "diagram-flowchart")
    );
}
