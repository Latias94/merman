#![cfg(feature = "svg")]

use merman::svg::layout_grid_available;

#[test]
fn grid_availability_requires_the_facade_feature_and_a_flow_family() {
    assert_eq!(
        layout_grid_available(),
        cfg!(all(
            feature = "layout-grid",
            any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            )
        ))
    );
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
mod flow_families {
    use super::layout_grid_available;
    #[cfg(feature = "layout-grid")]
    use merman::svg::RenderCapabilityPolicy;
    use merman::svg::{
        DiagramFamilyId, RenderCapability, RenderCapabilityPlan, RenderError as SvgError,
    };
    use merman::{
        LayoutBackend, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer,
        SvgRequest,
    };

    const HEADERS: &[(DiagramFamilyId, &str)] = &[
        #[cfg(feature = "diagram-flowchart")]
        (DiagramFamilyId::FLOWCHART, "flowchart LR"),
        #[cfg(feature = "diagram-swimlane")]
        (DiagramFamilyId::SWIMLANE, "swimlane-beta LR"),
        #[cfg(feature = "diagram-agentflow")]
        (DiagramFamilyId::AGENTFLOW, "agentflow-beta LR"),
    ];

    fn source(header: &str) -> String {
        format!("{header}\nA[Start] -->|Approved| B[Done]\n")
    }

    fn plan(
        renderer: &Renderer,
        source: &str,
        request: SvgRequest,
    ) -> Result<RenderCapabilityPlan, RenderError> {
        let output = renderer.render(RenderRequest::svg_plan(
            source,
            OperationControl::new(),
            request,
        ))?;
        let RenderOutput::SvgPlan(Some(plan)) = output else {
            panic!("expected an SVG plan");
        };
        Ok(plan)
    }

    #[test]
    fn grid_builder_selection_agrees_with_planning_and_rendering() {
        let renderer = Renderer::builder()
            .with_default_layout(LayoutBackend::Grid)
            .build()
            .unwrap();
        for &(family, header) in HEADERS {
            let source = source(header);
            let planned = plan(&renderer, &source, SvgRequest::default());
            let rendered = renderer.render_svg(&source);
            if layout_grid_available() {
                let plan = planned.unwrap();
                assert_eq!(plan.family_id(), family);
                assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
                assert!(plan.is_ready());
                let svg = rendered.unwrap();
                assert_eq!(svg.evidence().family_id(), family);
                assert_eq!(
                    svg.evidence().required_capabilities(),
                    plan.required_capabilities()
                );
                assert!(
                    roxmltree::Document::parse(svg.svg())
                        .unwrap()
                        .root_element()
                        .has_tag_name("svg")
                );
            } else if merman_render::layout_grid_available() {
                assert_eq!(
                    planned.unwrap().missing_capabilities(),
                    [RenderCapability::LayoutGrid],
                );
                assert!(matches!(
                    rendered,
                    Err(RenderError::Svg(SvgError::MissingCapability {
                        capability: RenderCapability::LayoutGrid,
                        ..
                    }))
                ));
            } else {
                assert!(matches!(
                    planned,
                    Err(RenderError::Svg(SvgError::UnsupportedLayoutBackend {
                        backend: "grid",
                        ..
                    }))
                ));
                assert!(matches!(
                    rendered,
                    Err(RenderError::Svg(SvgError::UnsupportedLayoutBackend {
                        backend: "grid",
                        ..
                    }))
                ));
            }
        }
    }

    #[cfg(feature = "layout-grid")]
    #[test]
    fn grid_source_selection_respects_host_policy_over_the_builder_default() {
        let renderer = Renderer::builder()
            .with_default_layout(LayoutBackend::Dagre)
            .build()
            .unwrap();
        let source = format!(
            "---\nconfig:\n  layout: grid\n---\n{}",
            source(HEADERS[0].1)
        );
        assert_eq!(
            plan(&renderer, &source, SvgRequest::default())
                .unwrap()
                .required_capabilities(),
            [RenderCapability::LayoutGrid],
        );

        let mut request = SvgRequest::default();
        request.environment = request.environment.with_capability_policy(
            RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutGrid),
        );
        let blocked = plan(&renderer, &source, request.clone()).unwrap();
        assert_eq!(
            blocked.missing_capabilities(),
            [RenderCapability::LayoutGrid]
        );
        assert!(matches!(
            renderer.render_svg_with(&source, request, OperationControl::new()),
            Err(RenderError::Svg(SvgError::MissingCapability {
                capability: RenderCapability::LayoutGrid,
                ..
            }))
        ));
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_rejects_grid_as_a_selectable_backend() {
    use merman::svg::RenderError as SvgError;
    use merman::{LayoutBackend, RenderError, Renderer};

    let renderer = Renderer::builder()
        .with_default_layout(LayoutBackend::Grid)
        .build()
        .unwrap();
    assert!(matches!(
        renderer.render_svg("classDiagram\nclass A\n"),
        Err(RenderError::Svg(SvgError::UnsupportedLayoutBackend {
            backend: "grid",
            ..
        }))
    ));
}
