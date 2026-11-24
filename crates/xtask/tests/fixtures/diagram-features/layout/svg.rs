use RenderCapability::{LayoutDagre, LayoutElk, LayoutShoal};
use merman::svg::{
    RenderCapability, RenderCapabilityPolicy, RenderError as SvgError, layout_dagre_available,
    layout_elk_available, layout_shoal_available,
};
use merman::{
    MermaidEngine, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgEnvironment, SvgRequest,
};

const FLOWCHART: &str = "flowchart LR\nA[Start] --> B[Done]\n";

fn configured(layout: &str, source: &str) -> String {
    format!("---\nconfig:\n  layout: {layout}\n---\n{source}")
}

fn draw(renderer: &Renderer, source: &str, label: &str, capability: Option<RenderCapability>) {
    let output = renderer
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest::default(),
        ))
        .unwrap();
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("expected SVG output: {source}")
    };
    assert!(
        svg.svg().starts_with("<svg") && svg.svg().contains(label),
        "{source}"
    );
    assert_eq!(
        svg.evidence().required_capabilities(),
        capability.as_slice()
    );
    if capability.is_some() {
        let output = renderer
            .render(RenderRequest::layout_json(
                source,
                OperationControl::new(),
                SvgRequest::default(),
            ))
            .unwrap();
        let RenderOutput::LayoutJson(Some(layout)) = output else {
            panic!("expected layout JSON: {source}")
        };
        assert!(
            layout.layout()["layout"]
                .as_object()
                .is_some_and(|geometry| !geometry.is_empty())
        );
    }
}

fn missing(
    renderer: &Renderer,
    source: &str,
    capability: RenderCapability,
    policy: RenderCapabilityPolicy,
) {
    let request = || SvgRequest {
        environment: SvgEnvironment::deterministic().with_capability_policy(policy),
        ..SvgRequest::default()
    };
    let output = renderer
        .render(RenderRequest::svg_plan(
            source,
            OperationControl::new(),
            request(),
        ))
        .unwrap();
    let RenderOutput::SvgPlan(Some(plan)) = output else {
        panic!("expected SVG plan: {source}")
    };
    assert_eq!(plan.required_capabilities(), &[capability]);
    assert_eq!(plan.missing_capabilities(), &[capability]);
    for request in [
        RenderRequest::svg(source, OperationControl::new(), request()),
        RenderRequest::layout_json(source, OperationControl::new(), request()),
    ] {
        let result = renderer.render(request);
        assert!(
            matches!(result, Err(RenderError::Svg(SvgError::MissingCapability {
            capability: actual, ..
        })) if actual == capability),
            "{source}: {result:?}"
        );
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let layout = args.next().unwrap();
    let capability = match layout.as_str() {
        "dagre" => Some(LayoutDagre),
        "elk" => Some(LayoutElk),
        "shoal" => Some(LayoutShoal),
        "native" | "swimlane" | "missing" => None,
        _ => panic!("unknown probe layout: {layout}"),
    };
    assert_eq!(layout_dagre_available(), capability == Some(LayoutDagre));
    assert_eq!(layout_elk_available(), capability == Some(LayoutElk));
    assert_eq!(layout_shoal_available(), capability == Some(LayoutShoal));
    let renderer = Renderer::new();
    if let Some(family) = args.next() {
        let (source, label) = match family.as_str() {
            "class" => ("classDiagram\n Alpha --> Beta\n", "Alpha"),
            "er" => ("erDiagram\n CUSTOMER ||--o{ ORDER : places\n", "CUSTOMER"),
            "state" => ("stateDiagram-v2\n [*] --> Ready\n Ready --> [*]\n", "Ready"),
            "requirement" => (
                "requirementDiagram\nrequirement req1 {\n id: 1\n text: A requirement\n risk: high\n verifymethod: analysis\n}\nelement system1 {\n type: system\n}\nreq1 - satisfies -> system1\n",
                "satisfies",
            ),
            _ => panic!("unknown probe family: {family}"),
        };
        draw(&renderer, &configured(&layout, source), label, capability);
        return;
    }

    let explicit = configured(&layout, FLOWCHART);
    match layout.as_str() {
        "native" => {
            draw(
                &renderer,
                "sequenceDiagram\nAlice->>Bob: Hello\n",
                "Hello",
                None,
            );
            draw(&renderer, "pie\n \"Slice\" : 1\n", "Slice", None);
        }
        "swimlane" => draw(
            &renderer,
            &configured(
                "swimlane",
                "swimlane-beta LR\nsubgraph Lane\nA[Start] --> B[Done]\nend\n",
            ),
            "Start",
            None,
        ),
        "missing" => {
            assert!(
                renderer
                    .parse_semantic(FLOWCHART, OperationControl::new())
                    .unwrap()
                    .is_some()
            );
            let policy = RenderCapabilityPolicy::unrestricted();
            missing(&renderer, FLOWCHART, LayoutElk, policy);
            missing(&renderer, &configured("elk", FLOWCHART), LayoutElk, policy);
            missing(
                &renderer,
                &configured("dagre", FLOWCHART),
                LayoutDagre,
                policy,
            );
        }
        "dagre" => {
            draw(&renderer, &explicit, "Start", capability);
            draw(&renderer, FLOWCHART, "Start", capability);
            draw(
                &renderer,
                &configured("elk", FLOWCHART),
                "Start",
                capability,
            );
            missing(
                &renderer,
                FLOWCHART,
                LayoutDagre,
                RenderCapabilityPolicy::unrestricted().with_denied(LayoutDagre),
            );
        }
        "elk" => {
            draw(&renderer, FLOWCHART, "Start", capability);
            draw(
                &renderer,
                &configured("elk.mrtree", FLOWCHART),
                "Start",
                capability,
            );
        }
        "shoal" => {
            draw(&renderer, &explicit, "Start", capability);
            missing(
                &renderer,
                FLOWCHART,
                LayoutElk,
                RenderCapabilityPolicy::unrestricted(),
            );
            let site = SourcePresentationConfig::from_value(
                r#"{"flowchart":{"layout":"shoal"}}"#.parse().unwrap(),
            );
            let renderer =
                renderer.with_presentation_engine(MermaidEngine::new().with_site_config(site));
            draw(&renderer, FLOWCHART, "Start", capability);
        }
        _ => unreachable!(),
    }
}
