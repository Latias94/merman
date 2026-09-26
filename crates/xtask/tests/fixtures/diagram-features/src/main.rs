//! Runs in an independent Cargo workspace to prevent xtask feature unification.
use std::collections::BTreeSet;

#[cfg(feature = "omitted-sequence")]
use merman_core::diagrams::sequence::SequenceDiagramRenderModel;

fn expected(name: &str) -> BTreeSet<String> {
    std::env::var(name)
        .unwrap()
        .split(',')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn main() {
    let selected = merman_core::diagram_family_capabilities()
        .iter()
        .filter(|family| family.has_semantic_parser && family.logical_family_kind != "error")
        .map(|family| family.logical_family_kind.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(selected, expected("MERMAN_EXPECTED_FAMILIES"));
    let engine = merman_core::Engine::new();
    for (family, source) in [
        ("flowchart", "flowchart TD\nA[Start]-->B[Finish]\n"),
        ("swimlane", "swimlane-beta LR\nA[Start]-->B[Finish]\n"),
        (
            "gantt",
            "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2024-01-01, 1d\n",
        ),
        ("sequence", "sequenceDiagram\nAlice->>Bob: Hello\n"),
    ] {
        let result =
            engine.parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict());
        if selected.contains(family) {
            let parsed = result.unwrap().unwrap();
            #[cfg(feature = "renderer")]
            {
                let enabled = expected("MERMAN_EXPECTED_RENDER_FAMILIES");
                let session = merman_render::environment::RenderEnvironment::default()
                    .begin_session()
                    .unwrap();
                let plan = merman_render::family::plan_render(&parsed, &session);
                if enabled.contains(family) {
                    assert!(plan.is_ok(), "{family}: {plan:?}");
                } else {
                    assert!(
                        matches!(plan, Err(merman_render::Error::UnsupportedDiagram { .. })),
                        "{family}: {plan:?}"
                    );
                }
            }
            #[cfg(feature = "ascii")]
            {
                let enabled = expected("MERMAN_EXPECTED_ASCII_FAMILIES");
                let renderer = merman_ascii::AsciiRenderer::default();
                let context = engine.begin_operation().unwrap();
                let control = merman_core::OperationControl::new();
                let resources = merman_ascii::AsciiResourcePolicy::default();
                let rendered = renderer.render_parsed(&parsed, &control, &context, resources);
                let report = renderer.render_parsed_report(
                    &parsed,
                    merman_ascii::AsciiViewportPolicy::unrestricted(),
                    &control,
                    &context,
                    resources,
                );
                if enabled.contains(family) {
                    let rendered = rendered.unwrap();
                    let report = report.unwrap();
                    for text in [&rendered, &report.text] {
                        assert!(!text.is_empty(), "{family}");
                        if matches!(family, "flowchart" | "swimlane") {
                            assert!(text.contains("Start"), "{family}: {text}");
                            assert!(text.contains("Finish"), "{family}: {text}");
                        }
                    }
                } else {
                    assert!(
                        matches!(
                            rendered,
                            Err(merman_ascii::AsciiError::UnsupportedDiagram { .. })
                        ),
                        "{family}: {rendered:?}"
                    );
                    assert!(
                        matches!(
                            report,
                            Err(merman_ascii::AsciiError::UnsupportedDiagram { .. })
                        ),
                        "{family}: {report:?}"
                    );
                }
            }
            let _ = parsed;
        } else {
            assert!(
                matches!(result, Err(merman_core::Error::UnsupportedDiagram { .. })),
                "{family}: {result:?}"
            );
            let lenient = engine
                .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::lenient())
                .unwrap()
                .unwrap();
            assert!(matches!(
                lenient.model(),
                merman_core::RenderSemanticModel::Error(_)
            ));
        }
    }
    #[cfg(feature = "editor")]
    verify_editor_shape_completion();
    #[cfg(feature = "ascii")]
    {
        let actual = merman_ascii::ascii_capabilities()
            .iter()
            .filter(|family| family.support_level.is_supported())
            .map(|family| family.diagram_type.to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected("MERMAN_EXPECTED_ASCII_FAMILIES"));
    }
}

#[cfg(feature = "editor")]
fn verify_editor_shape_completion() {
    use merman_editor_core::{
        DocumentKind, Position, analyze_document_snapshot_with_shared_text, completion_for_snapshot,
    };
    use std::sync::Arc;

    for (line, expected_start, replacement) in [
        ("A@{ shape: rou", "A@{ shape: ".len(), "circle }"),
        ("A((", 1, "@{ shape: circle }"),
    ] {
        let snapshot = analyze_document_snapshot_with_shared_text(
            &merman_analysis::Analyzer::new(),
            "file:///feature-selection.mmd",
            1,
            Arc::from(format!("flowchart TD\n{line}")),
            DocumentKind::Diagram,
        )
        .unwrap();
        let completion = completion_for_snapshot(&snapshot, Position::new(1, line.len()));
        assert_eq!(
            completion.fact_source,
            Some(merman_editor_core::FenceTextIndexSource::ParserRecovered)
        );
        let edit = completion
            .items
            .iter()
            .find(|item| item.label == "@{ shape: circle }")
            .and_then(|item| item.text_edit.as_ref())
            .expect("linked Flowchart parser must provide shape completion");
        assert_eq!(edit.range.start, Position::new(1, expected_start));
        assert_eq!(edit.range.end, Position::new(1, line.len()));
        assert_eq!(edit.new_text, replacement);
    }
}
