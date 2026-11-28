use super::*;
use crate::mindmap::{
    MindmapEdgeRouting, MindmapLayoutBackend, MindmapLayoutContext, MindmapPreparedArtifact,
    MindmapSvgContext, prepare_mindmap_diagram, render_mindmap_diagram_svg,
    tests::presentation_config,
};
use merman_model::{OperationControl, family::DiagramFamilyId, presentation::RenderSemanticModel};
use merman_render_core::{
    RenderCapability,
    resources::{RenderResourcePolicy, ResourceLimitId},
    root_svg::RootViewportContext,
    svg_css::SvgId,
    svg_output::timing::RenderTiming,
    text::{DeterministicTextMeasurer, TextMeasurer},
};
use std::sync::Arc;

fn model() -> MindmapModel {
    let parsed = merman_core::MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "mindmap\n  root((Root circle))\n    left[First]\n      child((Child))\n    hex{{Hexagon}}\n      nested[Second]\n    bang))Bang((\n    cloud)Cloud(\n",
            merman_core::ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Mindmap(model) = parsed.model() else {
        panic!("Mindmap fixture");
    };
    model.clone()
}

fn prepare(
    model: &MindmapModel,
    config: &PresentationConfig,
    backend: MindmapLayoutBackend<'_>,
    meter: &Arc<OperationWorkMeter>,
    measurer: &dyn TextMeasurer,
) -> Result<MindmapPreparedArtifact> {
    prepare_mindmap_diagram(
        model,
        config,
        None,
        MindmapLayoutContext {
            backend,
            text_measurer: measurer,
            math_renderer: None,
            measure_math_label: |_| panic!("fixture contains no math"),
            work_meter: meter,
        },
    )
}

fn render(
    model: &MindmapModel,
    config: &PresentationConfig,
    artifact: &MindmapPreparedArtifact,
    meter: &OperationWorkMeter,
) -> Result<String> {
    let diagram_id = SvgId::new("layeredMindmap").unwrap();
    render_mindmap_diagram_svg(
        model,
        config,
        artifact,
        MindmapSvgContext {
            diagram_id,
            viewport: RootViewportContext::new(DiagramFamilyId::MINDMAP, diagram_id),
            text_measurer: &DeterministicTextMeasurer::default(),
            math_renderer: None,
            work_meter: meter,
            timing: RenderTiming::disabled(),
        },
    )?
    .into_string_for(DiagramFamilyId::MINDMAP)
}

fn limits() -> RenderResourcePolicy {
    RenderResourcePolicy::unbounded_for_trusted_input()
}

fn metered(limit: ResourceLimitId, cap: usize) -> Arc<OperationWorkMeter> {
    Arc::new(OperationWorkMeter::new(
        limits().with_limit(limit, cap).unwrap(),
    ))
}

#[test]
fn source_slots_own_the_layout_and_axis_tagged_svg_edges() {
    let model = model();
    for routing in ["rounded", "curved", "stepped", "straight"] {
        let config = presentation_config(serde_json::json!({
            "htmlLabels": false,
            "layered": {"edgeRouting": routing, "lineHops": false},
        }));
        let meter = Arc::new(OperationWorkMeter::new(limits()));
        let artifact = prepare(
            &model,
            &config,
            MindmapLayoutBackend::Layered {
                allow_dagre_retry: false,
            },
            &meter,
            &DeterministicTextMeasurer::default(),
        )
        .unwrap();
        let MindmapEdgeRouting::Layered(Paint { routes, .. }) = &artifact.routing else {
            panic!("expected Layered route ownership");
        };
        assert!(
            model
                .nodes
                .iter()
                .zip(&artifact.layout.nodes)
                .all(|(source, placed)| source.id == placed.id)
        );
        assert_eq!(routes.len(), model.edges.len());

        let svg = render(&model, &config, &artifact, &meter).unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            document
                .root_element()
                .attribute("data-layout-fallback")
                .is_none()
        );
        let groups: Vec<_> = document
            .descendants()
            .find(|node| node.attribute("class") == Some("root"))
            .unwrap()
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();
        assert_eq!(
            groups
                .iter()
                .map(|group| group.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            ["clusters", "edges edgePaths", "edgeLabels", "nodes"]
        );
        assert_eq!(groups[2].children().count(), 0);
        let mut moved = false;
        for (((source, placed), route), path) in model
            .edges
            .iter()
            .zip(&artifact.layout.edges)
            .zip(routes)
            .zip(groups[1].children().filter(roxmltree::Node::is_element))
        {
            assert_eq!(path.attribute("data-id"), Some(source.id.as_str()));
            assert_eq!(
                (&source.id, &source.start, &source.end),
                (&placed.id, &placed.from, &placed.to)
            );
            let points: Vec<Value> = serde_json::from_slice(
                &STANDARD
                    .decode(path.attribute("data-points").unwrap())
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(points.len(), route.points.len());
            for (point, raw) in points.iter().zip(&route.points) {
                assert_eq!(
                    point["axis"],
                    match raw.axis {
                        merman_render_core::route_types::Axis::Vertical => "v",
                        merman_render_core::route_types::Axis::Horizontal => "h",
                    }
                );
            }
            if let Endpoint::Node(0) = route.source {
                let node = &artifact.layout.nodes[0];
                let x = points[0]["x"].as_f64().unwrap();
                let y = points[0]["y"].as_f64().unwrap();
                assert!(((x - node.x).hypot(y - node.y) - node.width / 2.0).abs() < 1e-7);
                moved |= (route.points[0].y - y).abs() > 1e-7;
            }
        }
        assert!(moved, "offset ports meet the circle rather than its box");
    }
}

#[test]
fn exact_work_and_svg_caps_keep_the_original_typed_terminal() {
    let model = model();
    let config = presentation_config(serde_json::json!({"htmlLabels": false}));
    let prepare = |meter: &Arc<_>| {
        prepare(
            &model,
            &config,
            MindmapLayoutBackend::Layered {
                allow_dagre_retry: true,
            },
            meter,
            &DeterministicTextMeasurer::default(),
        )
    };
    let meter = Arc::new(OperationWorkMeter::new(limits()));
    let artifact = prepare(&meter).unwrap();
    let layout_work = meter.used();
    let svg = render(&model, &config, &artifact, &meter).unwrap();
    let full_work = meter.used();
    let work = ResourceLimitId::MaxLayoutWorkUnits;
    assert!(prepare(&metered(work, layout_work)).is_ok());
    let limit_error = |error| matches!(error, Error::ResourceLimitExceeded(error) if error.limit == "max_layout_work_units");
    assert!(limit_error(
        prepare(&metered(work, layout_work - 1)).unwrap_err()
    ));
    let render_with = |limit, cap| {
        let meter = metered(limit, cap);
        let artifact = prepare(&meter)?;
        render(&model, &config, &artifact, &meter)
    };
    assert_eq!(render_with(work, full_work).unwrap(), svg);
    assert!(limit_error(render_with(work, full_work - 1).unwrap_err()));
    let bytes = ResourceLimitId::MaxSvgBytes;
    assert_eq!(render_with(bytes, svg.len()).unwrap(), svg);
    assert!(
        matches!(render_with(bytes, svg.len() - 1), Err(Error::ResourceLimitExceeded(error)) if error.limit == "max_svg_bytes")
    );

    let control = OperationControl::new();
    let timer = Arc::new(OperationWorkMeter::new_with_control(
        limits(),
        control.clone(),
    ));
    let artifact = prepare(&timer).unwrap();
    control.cancel();
    assert!(
        matches!(render(&model, &config, &artifact, &timer), Err(Error::Cancelled(error)) if error.phase == OperationPhase::Emit)
    );
}

#[test]
fn recoverable_failure_requires_dagre_and_host_refusal_wins() {
    let mut model = model();
    model.edges[1].id = model.edges[0].id.clone();
    let config =
        presentation_config(serde_json::json!({"htmlLabels": true, "securityLevel": "strict"}));
    let measurer = DeterministicTextMeasurer::default();
    let run = |allow_dagre_retry, meter: &Arc<_>| {
        prepare(
            &model,
            &config,
            MindmapLayoutBackend::Layered { allow_dagre_retry },
            meter,
            &measurer,
        )
    };
    let meter = Arc::new(OperationWorkMeter::new(limits()));
    assert!(matches!(
        run(false, &meter),
        Err(Error::MissingCapability {
            capability: RenderCapability::LayoutDagre,
            ..
        })
    ));
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
    #[cfg(not(feature = "layout-dagre"))]
    assert!(matches!(
        run(true, &meter),
        Err(Error::MissingCapability {
            capability: RenderCapability::LayoutDagre,
            ..
        })
    ));

    let meter = metered(ResourceLimitId::MaxPreparedTextRetainedBytes, 1);
    assert!(
        matches!(run(true, &meter), Err(Error::ResourceLimitExceeded(error)) if error.limit == "max_prepared_text_retained_bytes")
    );
    let control = OperationControl::new();
    control.cancel();
    let meter = Arc::new(OperationWorkMeter::new_with_control(limits(), control));
    assert!(
        matches!(run(true, &meter), Err(Error::Cancelled(error)) if error.phase == OperationPhase::Layout)
    );
}

#[cfg(feature = "layout-dagre")]
#[test]
fn admitted_retry_repeats_layout_measurement_with_fresh_labels_and_same_meter() {
    use merman_render_core::text::{TextMetrics, TextStyle};
    use std::cell::RefCell;

    #[derive(Default)]
    struct RecordingMeasurer(RefCell<Vec<String>>);
    impl TextMeasurer for RecordingMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.borrow_mut().push(text.to_owned());
            DeterministicTextMeasurer::default().measure(text, style)
        }
    }

    let mut model = model();
    model.edges[1].id = model.edges[0].id.clone();
    let config =
        presentation_config(serde_json::json!({"htmlLabels": true, "securityLevel": "strict"}));
    let direct_meter = Arc::new(OperationWorkMeter::new(limits()));
    let direct_measure = RecordingMeasurer::default();
    let direct = prepare(
        &model,
        &config,
        MindmapLayoutBackend::Dagre,
        &direct_meter,
        &direct_measure,
    )
    .unwrap();
    let retry_meter = Arc::new(OperationWorkMeter::new(limits()));
    let retry_measure = RecordingMeasurer::default();
    let backend = || MindmapLayoutBackend::Layered {
        allow_dagre_retry: true,
    };
    let retry = prepare(&model, &config, backend(), &retry_meter, &retry_measure).unwrap();
    let calls = direct_measure.0.into_inner();
    assert!(!calls.is_empty());
    assert_eq!(
        retry_measure.0.into_inner(),
        [calls.as_slice(), calls.as_slice()].concat()
    );
    let retained = retry_meter.prepared_text_retained_bytes();
    assert!(retained > 0);
    assert_eq!(retained, direct_meter.prepared_text_retained_bytes());
    let cumulative = retry_meter.used();
    assert!(cumulative > direct_meter.used());
    let svg = render(&model, &config, &retry, &retry_meter).unwrap();
    assert!(svg.contains(r#"data-layout-fallback="dagre""#));
    assert_eq!(
        svg.replacen(r#" data-layout-fallback="dagre""#, "", 1),
        render(&model, &config, &direct, &direct_meter).unwrap()
    );
    drop(retry);
    assert_eq!(retry_meter.prepared_text_retained_bytes(), 0);

    let exact = metered(ResourceLimitId::MaxLayoutWorkUnits, cumulative);
    assert!(
        prepare(
            &model,
            &config,
            backend(),
            &exact,
            &RecordingMeasurer::default()
        )
        .is_ok()
    );
    let short = metered(ResourceLimitId::MaxLayoutWorkUnits, cumulative - 1);
    assert!(
        matches!(prepare(&model, &config, backend(), &short, &RecordingMeasurer::default()),
        Err(Error::ResourceLimitExceeded(error)) if error.limit == "max_layout_work_units")
    );
    assert_eq!(short.prepared_text_retained_bytes(), 0);
    assert_eq!(short.projected_svg_bytes(), 0);
}
