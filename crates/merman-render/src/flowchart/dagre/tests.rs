use super::*;
#[cfg(feature = "diagram-flowchart")]
use crate::text::TextStyle;
use crate::{Error, resources::RenderResourcePolicy};
#[cfg(feature = "diagram-flowchart")]
use merman_core::{MermaidEngine, ParseOptions};
#[cfg(feature = "diagram-flowchart")]
use merman_model::presentation::RenderSemanticModel;

#[cfg(feature = "diagram-flowchart")]
const NON_LATTICE_COMPUTED_LENGTH_PX: f64 = 73.123_456_789;

#[cfg(feature = "diagram-flowchart")]
struct NonLatticeComputedLengthMeasurer;

#[cfg(feature = "diagram-flowchart")]
impl TextMeasurer for NonLatticeComputedLengthMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> crate::text::TextMetrics {
        crate::text::TextMetrics {
            width: 40.0,
            height: 18.0,
            line_count: 1,
        }
    }

    fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        NON_LATTICE_COMPUTED_LENGTH_PX
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_wrapped_labels_fit_rectangle_and_diamond_nodes() {
    let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                r#"%%{init: {"flowchart": {"wrappingWidth": 120}}}%%
flowchart TD
    A["Receive a new deployment request from the release pipeline"] --> B{"Did every required validation and security check pass successfully?"}
    B -->|Yes| C["Publish the approved application version to the production environment"]
    B -->|No| D["Return the detailed validation errors to the requesting developer"]
"#,
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let measurer = crate::text::DeterministicTextMeasurer::default()
        .with_width_callback(|text, _| text.chars().count() as f64 * 10.0);
    let layout = layout_flowchart_typed(
        model,
        &parsed.metadata().effective_config,
        measurer.as_ref(),
        None,
    )
    .expect("layout ok");

    for id in ["A", "C", "D"] {
        let node = layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("node {id}"));
        let label_width = node.label_width.expect("rectangle label width");
        let label_height = node.label_height.expect("rectangle label height");
        assert!(
            label_height > 24.0,
            "node {id} label did not wrap: {node:?}"
        );
        assert!(label_width < node.width, "node {id}: {node:?}");
        assert!(label_height < node.height, "node {id}: {node:?}");
    }

    let diamond = layout
        .nodes
        .iter()
        .find(|node| node.id == "B")
        .expect("diamond node B");
    let label_width = diamond.label_width.expect("diamond label width");
    let label_height = diamond.label_height.expect("diamond label height");
    assert!(
        label_height > 24.0,
        "diamond label did not wrap: {diamond:?}"
    );
    assert_eq!(diamond.width, diamond.height);
    assert!(
        label_width + label_height < diamond.width,
        "diamond label does not fit inside its sloped sides: {diamond:?}"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_preserves_operation_computed_length_precision() {
    // Isolate host precision from Mermaid 12's minimum label width.
    let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"htmlLabels\": false, \"flowchart\": {\"htmlLabels\": false, \"minNodeWidth\": 0}}}%%\nflowchart TB\nA[alpha]\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let layout = layout_flowchart_typed(
        model,
        &parsed.metadata().effective_config,
        &NonLatticeComputedLengthMeasurer,
        None,
    )
    .expect("layout ok");
    let node = layout
        .nodes
        .iter()
        .find(|node| node.id == "A")
        .expect("node A");

    assert_eq!(node.label_width, Some(NON_LATTICE_COMPUTED_LENGTH_PX));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_nonempty_subgraph_title_reuses_its_prepared_layout_measurement() {
    let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nsubgraph S[service title]\n  A\nend\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let labels = FlowchartRenderContext::default();
    let builder = FlowchartSvgLabelSidecarBuilder::default();
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("render session");
    let measurer = session.text_measurer(crate::environment::TextMeasurementPhase::Layout);
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let edge_style_plan = crate::test_support::prepare_flowchart_edge_style_plan(
        model,
        &parsed.metadata().effective_config,
        false,
        &meter,
    )
    .expect("prepare edge styles");
    layout(
        model,
        &labels,
        &super::super::FlowchartClassDefinitions::new(&model.class_defs),
        &parsed.metadata().effective_config,
        &measurer,
        None,
        Some(&builder),
        &edge_style_plan,
        meter,
    )
    .expect("layout ok");

    assert!(builder.prepared_count() > 0);
    assert!(
        builder.prepared_hit_count() > 0,
        "the cluster rect/title stages must reuse the same built-in measurement"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_html_nbsp_only_edge_labels_respect_source_provenance() {
    let nbsp = '\u{00A0}';
    let source = format!("flowchart LR\nA -- \"&nbsp;\" --> B\nC -- \"{nbsp}\" --> D\n");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let layout = layout_flowchart_typed(
        model,
        &parsed.metadata().effective_config,
        &crate::text::DeterministicTextMeasurer::default(),
        None,
    )
    .expect("layout ok");

    assert_eq!(layout.edges.len(), 2);
    let entity_label = layout
        .edges
        .iter()
        .find(|edge| edge.from == "A")
        .and_then(|edge| edge.label.as_ref())
        .expect("entity-authored NBSP label must reach Dagre");
    assert!(entity_label.width > 0.0, "{entity_label:?}");
    assert!(entity_label.height > 0.0, "{entity_label:?}");

    let direct_label = layout
        .edges
        .iter()
        .find(|edge| edge.from == "C")
        .and_then(|edge| edge.label.as_ref());
    assert!(direct_label.is_none(), "{direct_label:?}");
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_operation_meter_honors_below_equal_and_above_boundaries() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart TB\nsubgraph Outer\nsubgraph Inner\nA-->B\nend\nB-->C\nend\nC-->C\n",
            ParseOptions::default(),
        )
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let config = &parsed.metadata().effective_config;
    let measurer = crate::text::DeterministicTextMeasurer::default();

    let unbounded_meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let expected = layout_flowchart_typed_with_work_meter(
        model,
        config,
        &measurer,
        None,
        Arc::clone(&unbounded_meter),
    )
    .expect("unbounded Dagre layout succeeds");
    let exact = unbounded_meter.used();
    assert!(exact > 1);

    let below_meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .unwrap(),
    ));
    let error = layout_flowchart_typed_with_work_meter(
        model,
        config,
        &measurer,
        None,
        Arc::clone(&below_meter),
    )
    .expect_err("one unit below the exact Dagre work must fail");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded");
    };
    assert_eq!(limit.actual, exact);
    assert_eq!(limit.max, exact - 1);
    assert!(below_meter.used() < exact);

    for limit in [exact, exact + 1] {
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        ));
        let actual = layout_flowchart_typed_with_work_meter(
            model,
            config,
            &measurer,
            None,
            Arc::clone(&meter),
        )
        .expect("equal and above Dagre work budgets succeed");
        assert_eq!(meter.used(), exact);
        assert_eq!(
            actual.dom_node_order_by_root,
            expected.dom_node_order_by_root
        );
        assert_eq!(actual.uses_elk_adapter_dom, expected.uses_elk_adapter_dom);
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_kernel_arithmetic_overflow_maps_to_the_resource_contract() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TB\nA-->B\n", ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let mut model = model.clone();
    model.edges[0].length = usize::MAX;

    let error = layout_flowchart_typed_with_work_meter(
        &model,
        &parsed.metadata().effective_config,
        &crate::text::DeterministicTextMeasurer::default(),
        None,
        Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )),
    )
    .expect_err("Dugong arithmetic overflow must fail closed");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded");
    };
    assert_eq!(limit.limit, "max_layout_work_units");
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn unknown_shape_is_rejected_instead_of_becoming_a_rectangle() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TB\nA[known]\n", ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let mut model = model.clone();
    model.nodes[0].layout_shape = Some("definitely-unknown".to_string());
    let error = layout_flowchart_typed(
        &model,
        &parsed.metadata().effective_config,
        &crate::text::DeterministicTextMeasurer::default(),
        None,
    )
    .expect_err("unknown Flowchart shapes must not silently become rectangles");
    let Error::InvalidModel { message } = error else {
        panic!("expected InvalidModel for unknown Flowchart shape");
    };
    assert_eq!(
        message,
        "No such shape: definitely-unknown. Please check your syntax."
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shape_validation_scan_is_admitted_before_invalid_model_reporting() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart TB\nA[known]\nB[also known]\n",
            ParseOptions::default(),
        )
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let mut model = model.clone();
    model.nodes[1].layout_shape = Some("definitely-unknown".to_string());

    let error = layout_flowchart_typed_with_work_meter(
        &model,
        &parsed.metadata().effective_config,
        &crate::text::DeterministicTextMeasurer::default(),
        None,
        Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, 1)
                .unwrap(),
        )),
    )
    .expect_err("the node scan must be rejected before inspecting an invalid shape");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded");
    };
    assert_eq!(limit.actual, 2);
    assert_eq!(limit.max, 1);
}
