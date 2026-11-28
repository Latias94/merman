#![cfg(all(
    feature = "svg",
    any(
        feature = "diagram-flowchart",
        feature = "diagram-class",
        feature = "diagram-er",
        feature = "diagram-requirement",
        feature = "diagram-state"
    )
))]

use merman::svg::{
    DiagramFamilyId, RenderCapability, RenderCapabilityPolicy, RenderError as SvgError,
    layout_layered_available,
};
use merman::{LayoutBackend, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer};

#[test]
#[cfg(feature = "diagram-flowchart")]
fn flowchart_layered_builder_matches_planning_and_host_admission() {
    assert_layered_admission(
        DiagramFamilyId::FLOWCHART,
        "flowchart LR\nA[Start] -->|Approved| B[Done]\n",
    );
}

#[test]
#[cfg(feature = "diagram-class")]
fn class_layered_builder_matches_planning_and_host_admission() {
    assert_layered_admission(DiagramFamilyId::CLASS, "classDiagram\n  A --> B\n");
}

#[test]
#[cfg(feature = "diagram-er")]
fn er_layered_builder_matches_planning_and_host_admission() {
    assert_layered_admission(
        DiagramFamilyId::ER,
        "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n",
    );
}

#[test]
#[cfg(feature = "diagram-requirement")]
fn requirement_layered_builder_matches_planning_and_host_admission() {
    assert_layered_admission(
        DiagramFamilyId::REQUIREMENT,
        "requirementDiagram\n  requirement checkout {\n    id: 1\n  }\n",
    );
}

#[test]
#[cfg(feature = "diagram-state")]
fn state_layered_builder_matches_planning_and_host_admission() {
    assert_layered_admission(
        DiagramFamilyId::STATE,
        "stateDiagram-v2\n  Idle --> Ready\n",
    );
}

fn assert_layered_admission(family: DiagramFamilyId, source: &str) {
    let renderer = Renderer::builder()
        .with_default_layout(LayoutBackend::Layered)
        .build()
        .unwrap();
    assert_eq!(layout_layered_available(), cfg!(feature = "layout-layered"));
    let plan = renderer.render(RenderRequest::svg_plan(
        source,
        OperationControl::new(),
        renderer.default_svg_request(),
    ));
    if !merman_render::layout_layered_available() {
        assert!(matches!(
            plan,
            Err(RenderError::Svg(SvgError::UnsupportedLayoutBackend {
                backend: "layered",
                ..
            }))
        ));
        return;
    }
    let RenderOutput::SvgPlan(Some(plan)) = plan.unwrap() else {
        panic!("expected an SVG plan");
    };
    assert_eq!(plan.family_id(), family);
    assert_eq!(
        plan.required_capabilities(),
        [RenderCapability::LayoutLayered]
    );
    assert_eq!(plan.is_ready(), layout_layered_available());
    if plan.is_ready() {
        let svg = renderer.render_svg(source).unwrap();
        assert_eq!(svg.evidence().family_id(), family);
        assert_eq!(
            svg.evidence().required_capabilities(),
            plan.required_capabilities()
        );
        let doc = roxmltree::Document::parse(svg.svg()).unwrap();
        assert_eq!(doc.root_element().attribute("data-layout-fallback"), None);
    }

    let mut request = renderer.default_svg_request();
    request.environment = request.environment.with_capability_policy(
        RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutLayered),
    );
    assert!(matches!(
        renderer.render_svg_with(source, request, OperationControl::new()),
        Err(RenderError::Svg(SvgError::MissingCapability {
            capability: RenderCapability::LayoutLayered,
            ..
        }))
    ));
}
