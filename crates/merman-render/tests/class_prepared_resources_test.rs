#![cfg(feature = "diagram-class")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{RenderResourcePolicy, ResourceLimitId, ResourceLimitPhase};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

#[test]
fn class_prepared_html_and_svg_reject_a_one_byte_retained_limit() {
    for layout in [
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
    ] {
        for html_labels in [true, false] {
            let source = format!(
                "---\nconfig:\n  layout: {layout}\n  htmlLabels: {html_labels}\n---\nclassDiagram\nclass A[\"MMMM\"]\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = RenderEnvironment::deterministic()
                .with_resource_policy(
                    RenderResourcePolicy::unbounded_for_trusted_input()
                        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                        .unwrap(),
                )
                .begin_session()
                .unwrap();
            let error = match family::prepare(
                &parsed,
                &LayoutOptions::headless_svg_defaults(),
                session,
            ) {
                Err(error) => error,
                Ok(_) => panic!(
                    "Class {layout}, htmlLabels={html_labels} must reject a one-byte retained limit"
                ),
            };
            let merman_render::Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected a prepared-text resource rejection, got {error}");
            };
            assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
            assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
            assert_eq!(limit.max, 1);
            assert!(limit.actual > 1);
        }
    }
}

#[test]
fn class_notes_do_not_retain_temporary_html_or_svg_preparations() {
    for html_labels in [true, false] {
        let source = format!(
            "---\nconfig:\n  htmlLabels: {html_labels}\n---\nclassDiagram\nnote \"temporary **note** &amp; text<br/>second line\"\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .expect("notes keep no prepared text between layout and emission")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(rendered.svg().contains("second"));
        assert_eq!(
            rendered
                .into_completion()
                .report()
                .session_report()
                .prepared_text_retained_bytes_peak(),
            0
        );
    }
}
