//! Focused family-selection checks, also runnable with independently widened core features.

use merman_core::{MermaidEngine, ParseOptions};
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
fn parse(source: &str) -> merman_model::presentation::PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
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
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
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
        let Ok(Some(parsed)) = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        else {
            continue;
        };
        let parsed = parsed.into_presentation();
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
            family::prepare(&parsed, &LayoutOptions::default(), session),
            Err(Error::UnsupportedDiagram { .. })
        ));
    }
}

#[cfg(all(
    feature = "diagram-flowchart",
    feature = "diagram-gantt",
    feature = "layout-dagre"
))]
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
fn swimlane_header_can_use_graph_backends_or_its_own_layout() {
    use merman_render::RenderCapability;

    let (kind, svg) = render("swimlane-beta LR\nA --> B");
    assert_eq!(kind, RenderFamilyKind::Swimlane);
    assert!(svg.contains("<svg"));

    let elk_available = merman_render::layout_elk_available();
    let dagre_available = merman_render::layout_dagre_available();
    for (layout, capability, available) in [
        ("dagre", RenderCapability::LayoutDagre, dagre_available),
        (
            "elk",
            if dagre_available && !elk_available {
                RenderCapability::LayoutDagre
            } else {
                RenderCapability::LayoutElk
            },
            elk_available || dagre_available,
        ),
    ] {
        let parsed = parse(&format!(
            "---\nconfig:\n  layout: {layout}\n---\nswimlane-beta LR\nA --> B"
        ));
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[capability]);
        assert_eq!(plan.is_ready(), available);
        let result = family::prepare(&parsed, &LayoutOptions::default(), session);
        if available {
            assert_eq!(result.unwrap().family_kind(), RenderFamilyKind::Flowchart);
        } else {
            assert!(
                matches!(result, Err(Error::MissingCapability { capability: actual, .. }) if actual == capability)
            );
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_plan_and_both_graphical_outputs_share_backend_admission() {
    use merman_render::{RenderCapability, RenderCapabilityPolicy};

    let elk = merman_render::layout_elk_available();
    let dagre = merman_render::layout_dagre_available();
    let elk_or_fallback = if !elk && dagre {
        RenderCapability::LayoutDagre
    } else {
        RenderCapability::LayoutElk
    };
    for (layout, capability, compiled) in [
        (None, elk_or_fallback, elk || dagre),
        (Some("dagre"), RenderCapability::LayoutDagre, dagre),
        (Some("elk.mrtree"), elk_or_fallback, elk || dagre),
        (
            Some("shoal"),
            RenderCapability::LayoutShoal,
            merman_render::layout_shoal_available(),
        ),
    ] {
        let source = "flowchart TD\nA[Alpha] --> B[Beta]";
        let parsed = if let Some(layout) = layout {
            parse(&format!("---\nconfig:\n  layout: {layout}\n---\n{source}"))
        } else {
            parse(source)
        };
        for (policy, ready) in [
            (RenderCapabilityPolicy::unrestricted(), compiled),
            (RenderCapabilityPolicy::deny_all(), false),
        ] {
            let environment = RenderEnvironment::deterministic().with_capability_policy(policy);
            let session = environment.begin_session().unwrap();
            let plan = family::plan_render(&parsed, &session).unwrap();
            assert_eq!(plan.required_capabilities(), &[capability]);
            assert_eq!(
                plan.missing_capabilities(),
                if ready {
                    &[][..]
                } else {
                    std::slice::from_ref(&capability)
                }
            );
            let options = LayoutOptions::default();
            let render = family::prepare(&parsed, &options, session);
            let layout =
                family::prepare_layout(&parsed, &options, environment.begin_session().unwrap());
            if ready {
                let svg = render
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
                assert_eq!(svg.required_capabilities(), &[capability]);
                assert!(svg.svg().contains("Alpha"));
                assert!(
                    layout
                        .unwrap()
                        .layout_geometry_json()
                        .unwrap()
                        .to_string()
                        .contains("A")
                );
            } else {
                assert!(
                    matches!(render, Err(Error::MissingCapability { capability: actual, .. }) if actual == capability)
                );
                assert!(
                    matches!(layout, Err(Error::MissingCapability { capability: actual, .. }) if actual == capability)
                );
            }
        }
    }
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
        _: &merman_core::SourcePresentationMetadata,
        control: &merman_model::OperationControl,
    ) -> merman_model::OperationControlResult<
        merman_core::Result<merman_model::presentation::CustomJsonRenderModel>,
    > {
        control.checkpoint()?;
        Ok(Ok(merman_model::presentation::CustomJsonRenderModel::new(
            "custom",
            serde_json::json!({}),
        )))
    }
    let mut engine = MermaidEngine::new();
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
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(Error::NonRenderableCustomModel { .. })
    ));
    let control = merman_model::OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(Error::Cancelled(_))
    ));
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(Error::Cancelled(_))
    ));
}

#[cfg(all(feature = "diagram-agentflow", feature = "layout-dagre"))]
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

#[cfg(all(feature = "diagram-usecase", feature = "layout-dagre"))]
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
