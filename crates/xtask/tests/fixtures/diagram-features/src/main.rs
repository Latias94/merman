//! Runs in an independent Cargo workspace to prevent xtask feature unification.
use std::collections::BTreeSet;

#[cfg(feature = "omitted-sequence")]
use merman_model::families::sequence::SequenceDiagramModel;

fn expected(name: &str) -> BTreeSet<String> {
    std::env::var(name)
        .unwrap()
        .split(',')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn selected_models() -> BTreeSet<String> {
    merman_model::family::model_family_capabilities()
        .iter()
        .filter(|family| family.has_model && family.logical_family_kind != "error")
        .map(|family| family.logical_family_kind.to_string())
        .collect()
}

fn main() {
    assert_eq!(
        selected_models(),
        expected("MERMAN_EXPECTED_MODEL_FAMILIES")
    );
    #[cfg(feature = "source")]
    verify_source();
    #[cfg(feature = "renderer")]
    verify_render_handlers();
    #[cfg(feature = "ascii")]
    verify_ascii_capabilities();
    #[cfg(feature = "source-output-facts")]
    verify_source_output_facts();
    #[cfg(feature = "data-flowchart")]
    {
        let diagram = flowchart_data();
        #[cfg(feature = "ascii")]
        verify_declared_ascii(&diagram, &["Start", "Finish"]);
        #[cfg(feature = "renderer")]
        verify_declared_svg(&diagram);
        let _ = diagram;
    }
    #[cfg(feature = "data-gantt")]
    {
        let diagram = gantt_data();
        #[cfg(feature = "ascii")]
        verify_declared_ascii(&diagram, &["Task"]);
        let _ = diagram;
    }
}

#[cfg(feature = "source-output-facts")]
fn verify_source_output_facts() {
    // Core's producer compiles presentation without its compatibility serializer. Render's
    // independently selected model projector must still observe each frozen empty-output fact.
    let engine = merman_core::MermaidEngine::new();
    for (diagram_type, header) in [
        ("info", "info"), ("pie", "pie"), ("journey", "journey"),
        ("timeline", "timeline"), ("packet", "packet-beta"), ("radar", "radar-beta"),
    ] {
        for source in ["", header] {
            let parsed = engine.parse_diagram_for_render_model_with_type_sync(
                diagram_type, source, merman_core::ParseOptions::strict(),
            ).unwrap().unwrap();
            let presentation = parsed.presentation();
            let expected = presentation.compatibility_json().unwrap();
            let session = merman_render::environment::RenderEnvironment::deterministic()
                .begin_session().unwrap();
            let artifact = merman_render::family::prepare(
                presentation, &merman_render::LayoutOptions::default(), session,
            ).unwrap();
            let projected = artifact.layout_json().unwrap();
            assert_eq!(projected["semantic"], expected);
            if source.is_empty() {
                assert!(projected["semantic"].as_object().unwrap().is_empty(), "{diagram_type}");
            } else {
                assert_eq!(projected["semantic"]["type"], diagram_type);
            }
        }
    }
}

#[cfg(all(feature = "renderer", feature = "data-flowchart"))]
fn verify_declared_svg(diagram: &merman_model::ParsedSemanticDiagram) {
    use merman_model::presentation::{
        AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
    };
    let merman_model::SemanticDiagramModel::Flowchart(model) = &diagram.model else {
        panic!("the independent Flowchart fixture supplies its canonical typed payload");
    };
    let presentation = PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: diagram.metadata.diagram_type.clone(),
            config: diagram.metadata.config.clone(),
            effective_config: diagram.metadata.semantic_config.clone(),
            title: diagram.metadata.title.clone(),
            theme_recipe: None,
        },
        RenderSemanticModel::Flowchart(model.clone()),
        PresentationContext::None,
        &merman_model::resources::InputResourcePolicy::default(),
        &merman_model::OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let prepared = merman_render::family::prepare(
        &presentation,
        &merman_render::LayoutOptions::default(),
        session,
    )
    .unwrap();
    let output = prepared
        .render_svg(
            &merman_render::svg::SvgRenderOptions::default(),
            &merman_render::svg::SvgDebugOptions::default(),
        )
        .unwrap();
    assert!(output.svg().contains("<svg"));
    for label in ["Start", "Finish"] {
        assert!(output.svg().contains(label), "{label}");
    }
}

#[cfg(feature = "renderer")]
fn verify_render_handlers() {
    let actual = merman_model::family::diagram_family_selectors()
        .iter()
        .filter(|family| merman_render::family::supports_diagram_type(family.logical_family_kind))
        .map(|family| family.logical_family_kind.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected("MERMAN_EXPECTED_RENDER_FAMILIES"));
}

#[cfg(feature = "ascii")]
fn verify_ascii_capabilities() {
    let actual = merman_ascii::ascii_capabilities()
        .iter()
        .filter(|family| family.support_level.is_supported())
        .map(|family| family.diagram_type.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected("MERMAN_EXPECTED_ASCII_FAMILIES"));
}

#[cfg(any(feature = "data-flowchart", feature = "data-gantt"))]
fn declared(
    model: merman_model::SemanticDiagramModel,
    diagram_type: &str,
) -> merman_model::ParsedSemanticDiagram {
    merman_model::ParsedSemanticDiagram {
        metadata: merman_model::SemanticMetadata {
            diagram_type: diagram_type.to_string(),
            config: merman_model::MermaidConfig::default(),
            semantic_config: merman_model::MermaidConfig::default(),
            title: None,
        },
        model,
    }
}

#[cfg(feature = "data-flowchart")]
fn flowchart_data() -> merman_model::ParsedSemanticDiagram {
    use merman_model::families::flowchart::{FlowEdge, FlowEdgeMarker, FlowNode, FlowchartModel};
    let nodes = [("A", "Start"), ("B", "Finish")]
        .into_iter()
        .map(|(id, label)| FlowNode {
            id: id.to_string(),
            provenance: Default::default(),
            label: Some(label.to_string()),
            label_type: None,
            layout_shape: Some("rect".to_string()),
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        })
        .collect();
    let edge = FlowEdge {
        id: "A_B_0".to_string(),
        from: "A".to_string(),
        to: "B".to_string(),
        label: None,
        label_type: None,
        edge_type: None,
        arrow: "-->".to_string(),
        start_marker: FlowEdgeMarker::None,
        end_marker: FlowEdgeMarker::Point,
        is_user_defined_id: false,
        stroke: None,
        stroke_kind: Default::default(),
        visibility: Default::default(),
        interpolate: None,
        classes: Vec::new(),
        style: Vec::new(),
        animate: None,
        animation: None,
        length: 1,
    };
    declared(
        merman_model::SemanticDiagramModel::Flowchart(FlowchartModel {
            keyword: "flowchart".to_string(),
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: Some("TB".to_string()),
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes,
            edges: vec![edge],
            subgraphs: Vec::new(),
            subgraph_vertex_styles: Default::default(),
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        }),
        "flowchart",
    )
}

#[cfg(feature = "data-gantt")]
fn gantt_data() -> merman_model::ParsedSemanticDiagram {
    use merman_model::families::gantt::{GanttDiagramModel, GanttTask};
    declared(
        merman_model::SemanticDiagramModel::Gantt(GanttDiagramModel {
            date_format: "YYYY-MM-DD".to_string(),
            axis_format: "%Y-%m-%d".to_string(),
            today_marker: "off".to_string(),
            weekday: "monday".to_string(),
            weekend: "saturday".to_string(),
            sections: vec!["Work".to_string()],
            tasks: vec![GanttTask {
                id: "a".to_string(),
                task: "Task".to_string(),
                section: "Work".to_string(),
                section_index: Some(0),
                processed: true,
                start_ms: 1_704_067_200_000,
                end_ms: 1_704_153_600_000,
                ..Default::default()
            }],
            ..Default::default()
        }),
        "gantt",
    )
}

#[cfg(feature = "ascii")]
fn verify_declared_ascii(diagram: &merman_model::ParsedSemanticDiagram, labels: &[&str]) {
    let renderer = merman_ascii::AsciiRenderer::default();
    let control = merman_model::OperationControl::new();
    let context = merman_model::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .unwrap();
    let resources = merman_ascii::AsciiResourcePolicy::default();
    let text = renderer.render_parsed(diagram, &control, &context, resources);
    let report = renderer.render_parsed_report(
        diagram,
        merman_ascii::AsciiViewportPolicy::unrestricted(),
        &control,
        &context,
        resources,
    );
    if expected("MERMAN_EXPECTED_ASCII_FAMILIES").contains(&diagram.metadata.diagram_type) {
        let text = text.unwrap();
        let report = report.unwrap();
        for output in [&text, &report.text] {
            for label in labels {
                assert!(output.contains(label), "{output}");
            }
        }
    } else {
        assert!(matches!(
            text,
            Err(merman_ascii::AsciiError::UnsupportedDiagram { .. })
        ));
        assert!(matches!(
            report,
            Err(merman_ascii::AsciiError::UnsupportedDiagram { .. })
        ));
    }
}

#[cfg(all(
    test,
    feature = "source",
    feature = "ascii",
    feature = "data-flowchart",
    feature = "data-gantt"
))]
mod tests {
    #[test]
    fn dev_dependency_model_union_preserves_narrow_parser_and_terminal_owners() {
        let all = merman_model::family::diagram_family_selectors()
            .iter()
            .map(|family| family.logical_family_kind.to_string())
            .collect();
        assert_eq!(super::selected_models(), all);
        super::verify_source();
        super::verify_ascii_capabilities();
        super::verify_declared_ascii(&super::flowchart_data(), &["Start", "Finish"]);
        super::verify_declared_ascii(&super::gantt_data(), &["Task"]);
    }
}

#[cfg(feature = "source")]
fn verify_source() {
    let selected = merman_core::diagram_family_capabilities()
        .iter()
        .filter(|family| family.has_semantic_parser && family.logical_family_kind != "error")
        .map(|family| family.logical_family_kind.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(selected, expected("MERMAN_EXPECTED_FAMILIES"));
    let engine = merman_core::MermaidEngine::new();
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
                    let plan = merman_render::family::plan_render(parsed.presentation(), &session).unwrap();
                    assert!(plan.is_ready(), "{family}: {plan:?}");
                    let prepared = merman_render::family::prepare(
                        parsed.presentation(),
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
                        parsed.presentation(),
                        &environment.begin_session().unwrap(),
                    );
                    let prepared = merman_render::family::prepare(
                        parsed.presentation(),
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
                let control = merman_model::OperationControl::new();
                let resources = merman_ascii::AsciiResourcePolicy::default();
                if enabled.contains(family) {
                    let parsed = merman_core::SemanticParser::new()
                        .parse_diagram_sync(source)
                        .unwrap();
                    let rendered = renderer
                        .render_parsed(&parsed, &control, &context, resources)
                        .unwrap();
                    let report = renderer
                        .render_parsed_report(
                            &parsed,
                            merman_ascii::AsciiViewportPolicy::unrestricted(),
                            &control,
                            &context,
                            resources,
                        )
                        .unwrap();
                    for text in [&rendered, &report.text] {
                        assert!(!text.is_empty(), "{family}");
                        if matches!(family, "flowchart" | "swimlane") {
                            assert!(text.contains("Start"), "{family}: {text}");
                            assert!(text.contains("Finish"), "{family}: {text}");
                        }
                    }
                } else {
                    let semantic = merman_core::SemanticParser::new().parse_diagram_sync(source);
                    match semantic {
                        Ok(parsed) => {
                            let rendered =
                                renderer.render_parsed(&parsed, &control, &context, resources);
                            let report = renderer.render_parsed_report(
                                &parsed,
                                merman_ascii::AsciiViewportPolicy::unrestricted(),
                                &control,
                                &context,
                                resources,
                            );
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
                        Err(error) => assert!(
                            matches!(error, merman_core::Error::UnsupportedDiagram { .. }),
                            "{family}: {error:?}"
                        ),
                    }
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
                merman_model::presentation::RenderSemanticModel::Error(_)
            ));
        }
    }
    #[cfg(feature = "facade")]
    verify_embedding(&selected);
    #[cfg(feature = "editor")]
    verify_editor_shape_completion();
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
            let selector = merman_model::family::diagram_family_selectors()
                .iter()
                .find(|selector| selector.logical_family_kind == family)
                .unwrap();
            assert!(error.to_string().contains(selector.feature), "{error}");
            assert!(error.to_string().contains("all-diagrams"), "{error}");
        }
    }
}
