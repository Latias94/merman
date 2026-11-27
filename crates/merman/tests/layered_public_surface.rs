#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman::svg::{
    RenderCapability, RenderCapabilityPolicy, RenderError as SvgError, layout_layered_available,
};
use merman::{LayoutBackend, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer};

#[test]
fn layered_builder_matches_planning_and_host_admission() {
    const SOURCE: &str = "flowchart LR\nA[Start] -->|Approved| B[Done]\n";
    let renderer = Renderer::builder()
        .with_default_layout(LayoutBackend::Layered)
        .build()
        .unwrap();
    assert_eq!(layout_layered_available(), cfg!(feature = "layout-layered"));
    let plan = renderer.render(RenderRequest::svg_plan(
        SOURCE,
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
    assert_eq!(
        plan.required_capabilities(),
        [RenderCapability::LayoutLayered]
    );
    assert_eq!(plan.is_ready(), layout_layered_available());
    if plan.is_ready() {
        let svg = renderer.render_svg(SOURCE).unwrap();
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
        renderer.render_svg_with(SOURCE, request, OperationControl::new()),
        Err(RenderError::Svg(SvgError::MissingCapability {
            capability: RenderCapability::LayoutLayered,
            ..
        }))
    ));
}
