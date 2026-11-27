use merman_core::{MermaidEngine, ParseOptions};

#[cfg(feature = "layout-dagre")]
#[test]
fn class_html_family_releases_plans_after_svg_failure_or_cancellation() {
    use crate::environment::RenderEnvironment;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use std::sync::Arc;

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  htmlLabels: true\n---\nclassDiagram\nclass A[\"MMMM\"]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    for cancel in [false, true] {
        let control = merman_model::OperationControl::new();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxSvgBytes, 1)
                    .unwrap(),
            )
            .begin_session_with_control(control.clone())
            .unwrap();
        let meter = Arc::clone(session.work_meter());
        let artifact = crate::family::prepare(
            &parsed,
            &crate::LayoutOptions::headless_svg_defaults(),
            session,
        )
        .unwrap();
        assert!(meter.prepared_text_retained_bytes() > 0);
        if cancel {
            control.cancel();
        }
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Err(error) => error,
                Ok(_) => panic!("expected SVG failure or cancellation"),
            };
        if cancel {
            assert!(matches!(error, crate::Error::Cancelled(_)));
        } else {
            let crate::Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected the SVG output ceiling");
            };
            assert_eq!(limit.limit, "max_svg_bytes");
        }
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}
