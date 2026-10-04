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
        ("agentflow", "agentflow-beta\nA --> B\n"),
        ("usecase", "usecase-beta\nactor User\nUser --> Login\n"),
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
                let environment = merman_render::environment::RenderEnvironment::deterministic();
                if enabled.contains(family) {
                    let session = environment.begin_session().unwrap();
                    let plan = merman_render::family::plan_render(&parsed, &session).unwrap();
                    assert!(plan.is_ready(), "{family}: {plan:?}");
                    let prepared = merman_render::family::prepare(
                        parsed.clone(),
                        &merman_render::LayoutOptions::default(),
                        session,
                    )
                    .unwrap();
                    let svg = prepared
                        .render_svg(
                            &merman_render::svg::SvgRenderOptions::default(),
                            &merman_render::svg::SvgDebugOptions::default(),
                        )
                        .unwrap();
                    assert!(svg.svg().contains("<svg"), "{family}");
                }
                if !enabled.contains(family) || family == "flowchart" {
                    // A widened core must not move resource checks ahead of local availability.
                    // Selected Flowchart is the positive control: two nodes exceed this limit.
                    let policy = merman_render::RenderResourcePolicy::default()
                        .with_limit(merman_render::ResourceLimitId::MaxModelItems, 1)
                        .unwrap();
                    let environment = environment.with_resource_policy(policy);
                    let plan = merman_render::family::plan_render(
                        &parsed,
                        &environment.begin_session().unwrap(),
                    );
                    let prepared = merman_render::family::prepare(
                        parsed.clone(),
                        &merman_render::LayoutOptions::default(),
                        environment.begin_session().unwrap(),
                    );
                    if enabled.contains(family) {
                        assert!(matches!(
                            plan,
                            Err(merman_render::Error::ResourceLimitExceeded(_))
                        ));
                        assert!(matches!(
                            prepared,
                            Err(merman_render::Error::ResourceLimitExceeded(_))
                        ));
                    } else {
                        assert!(
                            matches!(plan, Err(merman_render::Error::UnsupportedDiagram { .. })),
                            "{family}: {plan:?}"
                        );
                        assert!(
                            matches!(
                                prepared,
                                Err(merman_render::Error::UnsupportedDiagram { .. })
                            ),
                            "{family}: {:?}",
                            prepared.as_ref().err()
                        );
                    }
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
    #[cfg(feature = "facade")]
    verify_embedding(&selected);
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

#[cfg(feature = "facade")]
fn verify_embedding(selected: &BTreeSet<String>) {
    use merman::svg::{CssOverridePostprocessor, SvgPipeline};
    use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

    let renderer = Renderer::new();
    for (family, source) in [
        ("flowchart", "flowchart TD\nA[Start]-->B[Finish]\n"),
        ("sequence", "sequenceDiagram\nAlice->>Bob: Hello\n"),
        ("class", "classDiagram\nAnimal <|-- Duck\n"),
        ("state", "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n"),
        ("er", "erDiagram\nCUSTOMER ||--o{ ORDER : places\n"),
        (
            "gantt",
            "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2024-01-01, 1d\n",
        ),
        ("pie", "pie\n\"One\" : 1\n"),
        ("gitGraph", "gitGraph\ncommit\n"),
        ("mindmap", "mindmap\n  root((Root))\n    Child\n"),
        ("timeline", "timeline\n2026 : Work\n"),
        ("quadrantChart", "quadrantChart\nA: [0.3, 0.6]\n"),
        (
            "xychart",
            "xychart-beta\nx-axis [a, b]\ny-axis 0 --> 10\nbar [3, 7]\n",
        ),
        ("journey", "journey\nsection Work\nTask: 5: Alice\n"),
    ] {
        let mut request = SvgRequest::default();
        request.options.diagram_id = Some(format!("embedding-{family}"));
        request.pipeline = Some(
            SvgPipeline::resvg_safe()
                .with_postprocessor(CssOverridePostprocessor::strip_existing_important()),
        );
        let output = renderer.render(RenderRequest::svg(source, OperationControl::new(), request));
        if selected.contains(family) {
            let RenderOutput::Svg(Some(svg)) = output.unwrap() else {
                panic!("{family}: expected an SVG artifact");
            };
            assert!(svg.svg().contains("<svg"), "{family}");
            assert!(!svg.svg().contains("<foreignObject"), "{family}");
        } else {
            let merman::RenderError::Parse(error) = output.unwrap_err() else {
                panic!("{family}: expected an unsupported parser diagnostic");
            };
            let details = error.terminal_diagnostic_details();
            assert_eq!(details.code, "merman.parse.unsupported_diagram");
            let selector = merman_core::diagram_family_selectors()
                .iter()
                .find(|selector| selector.logical_family_kind == family)
                .unwrap();
            assert!(error.to_string().contains(selector.feature), "{error}");
            assert!(error.to_string().contains("all-diagrams"), "{error}");
        }
    }
}
