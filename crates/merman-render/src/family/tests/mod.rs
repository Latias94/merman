type FamilyFixture = (
    merman_model::presentation::PresentationDiagram,
    LayoutOptions,
    RenderSession,
);

use super::*;
use crate::RenderCapability;

use std::sync::{Arc, Mutex};

use crate::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    TextMeasurementOperation, TextMeasurementPhase, TextMeasurementResultKind,
};
use crate::environment::{
    MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use crate::svg::SvgPipelinePreset;
use crate::text::{TextMetrics, WrapMode};

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationConfig;
use merman_model::presentation::{CustomJsonProvenance, CustomJsonRenderModel};
use serde_json::{Value, json};

fn custom_semantic_parser(
    _code: &str,
    meta: &merman_core::SourcePresentationMetadata,
    control: &merman_model::OperationControl,
) -> merman_model::OperationControlResult<merman_core::Result<Value>> {
    control.checkpoint()?;
    Ok(Ok(
        json!({ "type": meta.diagram_type, "owner": "semantic" }),
    ))
}

fn custom_render_parser(
    _code: &str,
    _meta: &merman_core::SourcePresentationMetadata,
    control: &merman_model::OperationControl,
) -> merman_model::OperationControlResult<merman_core::Result<CustomJsonRenderModel>> {
    control.checkpoint()?;
    Ok(Ok(CustomJsonRenderModel::new(
        "custom-flowchart",
        json!({ "owner": "render" }),
    )))
}

fn session() -> RenderSession {
    crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap()
}

/// Decodes the `data-points` attribute the SVG emitter writes on every edge `<path>`.
///
/// Deliberately an independent reader: the equivalence assertion below is only meaningful
/// if it goes through the serialized form rather than the in-memory value it came from.
fn decode_data_points(svg: &str) -> Vec<(String, Vec<(f64, f64)>)> {
    let mut out = Vec::new();
    for chunk in svg.split("<path").skip(1) {
        let Some(tag) = chunk.split('>').next() else {
            continue;
        };
        if !tag.contains("data-points=") {
            continue;
        }
        let id = tag
            .split("data-id=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_default()
            .to_owned();
        let encoded = tag
            .split("data-points=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("edge path carries data-points");
        use base64::Engine as _;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("data-points is base64");
        let points: Vec<serde_json::Value> =
            serde_json::from_slice(&raw).expect("data-points is JSON");
        out.push((
            id,
            points
                .iter()
                .map(|p| {
                    (
                        p["x"].as_f64().expect("point x"),
                        p["y"].as_f64().expect("point y"),
                    )
                })
                .collect(),
        ));
    }
    out
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn decode_edge_label_positions(svg: &str) -> std::collections::HashMap<String, (f64, f64)> {
    let document = crate::xml_dom::Document::parse(svg).expect("valid flowchart SVG");
    let mut positions = std::collections::HashMap::new();
    for edge_label in document.descendants().filter(|node| {
        node.has_tag_name("g")
            && node.attribute("class").is_some_and(|class| {
                class
                    .split_ascii_whitespace()
                    .any(|part| part == "edgeLabel")
            })
    }) {
        let Some(transform) = edge_label.attribute("transform") else {
            continue;
        };
        let Some((x, y)) = transform
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| {
                let (x, y) = value.split_once(',')?;
                Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
            })
        else {
            continue;
        };
        let Some(label) = edge_label.descendants().find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "label"))
                && node.attribute("data-id").is_some()
        }) else {
            continue;
        };
        positions.insert(label.attribute("data-id").unwrap().to_owned(), (x, y));
    }
    positions
}

fn parse_flowchart(source: &str) -> PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("detect flowchart")
        .into_presentation()
}

#[cfg(feature = "layout-dagre")]
#[test]
fn independent_flowchart_without_context_renders_canonical_css() {
    use merman_model::families::flowchart::{
        FlowSubgraphVertexStyle, FlowchartModel, FlowchartRenderContext,
    };
    use merman_model::presentation::{AuthoredPresentationMetadata, PresentationContext};
    use merman_model::resources::{InputResourcePolicy, ResourceProfile};

    let mut model: FlowchartModel = serde_json::from_value(json!({
        "keyword": "flowchart", "direction": "TB", "nodes": [], "edges": [],
        "subgraphs": [{"id": "G", "title": "Group", "nodes": []}]
    }))
    .unwrap();
    model.subgraph_vertex_styles.insert(
        "G".to_string(),
        0,
        FlowSubgraphVertexStyle {
            classes: Vec::new(),
            styles: vec!["fill:#123456".to_string()],
        },
    );
    let context = FlowchartRenderContext::from_model(&model);
    let complexity =
        merman_model::resources::FlowchartComplexity::from_model(&model).as_model_complexity();
    let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
        .with_limit(
            merman_model::resources::InputResourceLimitId::MaxModelTextBytes,
            complexity.text_bytes,
        )
        .unwrap();
    let render = |context| {
        let parsed = PresentationDiagram::try_new(
            AuthoredPresentationMetadata {
                diagram_type: "flowchart-v2".to_string(),
                config: merman_model::MermaidConfig::empty_object(),
                effective_config: merman_model::MermaidConfig::empty_object(),
                title: None,
            },
            RenderSemanticModel::Flowchart(model.clone()),
            context,
            &policy,
            &merman_model::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        prepare(&parsed, &LayoutOptions::default(), session())
            .unwrap()
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("canonical-css".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .unwrap()
            .svg()
            .to_owned()
    };
    let ordinary = render(PresentationContext::None);
    let explicit = render(PresentationContext::Flowchart(context));
    assert_eq!(ordinary, explicit);
    assert!(ordinary.contains("fill:#123456"), "{ordinary}");
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn edge_geometry_json_points_match_the_emitted_data_points() {
    // Circle endpoints exercise the difference between `layoutJson.points` and the clipped
    // polyline reported by the renderer. The reverse labelled edge also keeps this aligned
    // with the minimal reproduction from issue #172.
    let source = r#"flowchart TD
S[Start] --> M((Circle))
M -->|back| S
"#;

    let canonical_1 = parse_flowchart(source);
    let geometry = prepare(&canonical_1, &LayoutOptions::default(), session())
        .expect("prepare flowchart")
        .edge_geometry_json()
        .expect("edge geometry json");
    let canonical_2 = parse_flowchart(source);
    let svg = prepare(&canonical_2, &LayoutOptions::default(), session())
        .expect("prepare flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render flowchart")
        .svg()
        .to_owned();

    let from_svg = decode_data_points(&svg);
    assert!(
        !from_svg.is_empty(),
        "the emitter must have written data-points for this diagram"
    );
    assert_eq!(
        geometry.len(),
        from_svg.len(),
        "every rendered edge must have a structured counterpart"
    );

    let mut compared = 0;
    for edge in &geometry {
        let Some((_, expected)) = from_svg.iter().find(|(id, _)| id == &edge.id) else {
            continue;
        };
        let actual = edge
            .points
            .as_ref()
            .unwrap_or_else(|| panic!("edge {} must have clipped points", edge.id));
        assert_eq!(actual.len(), expected.len(), "edge {} point count", edge.id);
        for (index, (point, (ex, ey))) in actual.iter().zip(expected).enumerate() {
            let (ax, ay) = (point.x, point.y);
            assert_eq!(ax, *ex, "edge {} point {index} x", edge.id);
            assert_eq!(ay, *ey, "edge {} point {index} y", edge.id);
        }
        compared += 1;
    }
    assert_eq!(compared, from_svg.len(), "every SVG edge was compared");

    // The clipped polyline must differ from the layout polyline here, otherwise this test
    // would be asserting equality of two things that were never different.
    let canonical_3 = parse_flowchart(source);
    let layout = prepare(&canonical_3, &LayoutOptions::default(), session())
        .expect("prepare flowchart")
        .layout_json()
        .expect("layout json");
    let layout_edges = layout["layout"]["FlowchartV2"]["edges"]
        .as_array()
        .expect("layout edges");
    let differed = geometry.iter().filter(|edge| {
        layout_edges
            .iter()
            .find(|candidate| candidate["id"] == edge.id.as_str())
            .and_then(|candidate| candidate["points"].as_array())
            .zip(edge.points.as_ref())
            .is_some_and(|(layout_points, clipped)| {
                layout_points.len() != clipped.len()
                    || layout_points
                        .iter()
                        .zip(clipped)
                        .any(|(l, c)| l["x"].as_f64() != Some(c.x) || l["y"].as_f64() != Some(c.y))
            })
    });
    assert!(
        differed.count() > 0,
        "this diagram must exercise clipping, otherwise it proves nothing"
    );
}

#[cfg(feature = "layout-dagre")]
#[test]
fn edge_geometry_json_keeps_duplicate_ids_occurrence_bound() {
    let source = r#"---
config:
  layout: dagre
---
flowchart LR
X L_A_B_0@-->|first owner| Y
A -->|second owner| B
"#;
    let canonical_4 = parse_flowchart(source);
    let geometry = prepare(&canonical_4, &LayoutOptions::default(), session())
        .expect("prepare duplicate edge geometry")
        .edge_geometry_json()
        .expect("duplicate edge geometry");
    let canonical_5 = parse_flowchart(source);
    let svg = prepare(&canonical_5, &LayoutOptions::default(), session())
        .expect("prepare duplicate edge SVG")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render duplicate edge SVG");
    let from_svg = decode_data_points(svg.svg());
    assert_eq!(geometry.len(), 2);
    assert_eq!(from_svg.len(), 2);
    let coordinates = |edge: &crate::model::EdgePaintGeometry| {
        edge.points
            .as_ref()
            .expect("clipped duplicate edge points")
            .iter()
            .map(|point| (point.x, point.y))
            .collect::<Vec<_>>()
    };
    assert_ne!(coordinates(&geometry[0]), coordinates(&geometry[1]));
    let first_label = geometry[0]
        .label_position
        .as_ref()
        .expect("first occurrence has a painted label");
    let second_label = geometry[1]
        .label_position
        .as_ref()
        .expect("second occurrence has a painted label");
    assert_ne!(
        (first_label.x, first_label.y),
        (second_label.x, second_label.y),
        "duplicate public IDs must not overwrite painted label positions"
    );
    for (edge, (id, points)) in geometry.iter().zip(from_svg) {
        assert_eq!(edge.id, id);
        let actual = edge.points.as_ref().expect("clipped duplicate edge points");
        assert_eq!(actual.len(), points.len());
        for (point, (x, y)) in actual.iter().zip(points) {
            assert_eq!((point.x, point.y), (x, y));
        }
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn flowchart_node_wrapper<'a, 'input>(
    document: &'a crate::xml_dom::Document<'input>,
    node_id: &str,
) -> crate::xml_dom::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-et") == Some("node")
                && node.attribute("data-id") == Some(node_id)
        })
        .unwrap_or_else(|| panic!("Flowchart node wrapper for {node_id}"))
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn flowchart_node_shape_attribute(svg: &str, node_id: &str, attribute: &str) -> Option<String> {
    let document = crate::xml_dom::Document::parse(svg).expect("valid Flowchart SVG");
    let wrapper = flowchart_node_wrapper(&document, node_id);
    wrapper
        .descendants()
        .find(|node| {
            node.is_element()
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "label-container")
                })
        })
        .and_then(|node| node.attribute(attribute))
        .map(str::to_string)
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn flowchart_node_label_style(svg: &str, node_id: &str) -> String {
    let document = crate::xml_dom::Document::parse(svg).expect("valid Flowchart SVG");
    let wrapper = flowchart_node_wrapper(&document, node_id);
    wrapper
        .descendants()
        .find(|node| {
            node.is_element()
                && node
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "label"))
        })
        .and_then(|node| node.attribute("style"))
        .unwrap_or_else(|| panic!("Flowchart node label style for {node_id}"))
        .to_string()
}

#[test]
fn capability_plan_reports_the_authoritative_render_family() {
    let configured_swimlane = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(
            json!({ "layout": "swimlane" }),
        ),
    );
    let cases = [
        (
            MermaidEngine::new(),
            "flowchart TD\nA --> B\n",
            DiagramFamilyId::FLOWCHART,
        ),
        (
            MermaidEngine::new(),
            "swimlane-beta LR\nA --> B\n",
            DiagramFamilyId::SWIMLANE,
        ),
        (
            configured_swimlane,
            "flowchart LR\nA --> B\n",
            DiagramFamilyId::SWIMLANE,
        ),
    ];

    for (engine, source, expected_family) in cases {
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("family fixture should produce a render model")
            .into_presentation();
        let plan = plan_render(&parsed, &session()).unwrap();

        assert_eq!(plan.family_id(), expected_family, "{source}");
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn classic_flowchart_without_diagram_theme_does_not_emit_radius_attributes() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "%%{init: {\"look\": \"classic\"}}%%\nflowchart LR\nA[Alpha]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model")
        .into_presentation();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session())
        .expect("prepare default Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render default Flowchart");

    assert_eq!(
        flowchart_node_shape_attribute(rendered.svg(), "A", "rx"),
        None
    );
    assert_eq!(
        flowchart_node_shape_attribute(rendered.svg(), "A", "ry"),
        None
    );
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn flowchart_generated_css_emits_important_once() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart LR\nclassDef default fill:#22c55e !important\nA@{ shape: start }\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model")
        .into_presentation();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session())
        .expect("prepare important fixture")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render important fixture");

    assert!(rendered.svg().contains("fill:rgb(34, 197, 94)!important"));
    assert!(!rendered.svg().contains("important!important"));
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn planned_family_drives_flowchart_router_before_layout() {
    let engine = MermaidEngine::new();
    let options = LayoutOptions::default();

    let flowchart = engine
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model")
        .into_presentation();
    let swimlane = super::preparation::prepare_non_class_render(
        &flowchart,
        &options,
        FamilyRenderContext::resolve(
            session(),
            DiagramFamilyId::SWIMLANE,
            GraphLayoutBackend::Swimlane,
        ),
        FamilyPreparation::Render,
    )
    .expect("planned Swimlane family should drive layout");
    assert_eq!(swimlane.family_id(), DiagramFamilyId::SWIMLANE);

    #[cfg(feature = "layout-dagre")]
    {
        let configured_swimlane = engine
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA --> B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("configured Swimlane source should produce a render model")
            .into_presentation();
        let flowchart = super::preparation::prepare_non_class_render(
            &configured_swimlane,
            &options,
            FamilyRenderContext::resolve(
                session(),
                DiagramFamilyId::FLOWCHART,
                GraphLayoutBackend::Dagre,
            ),
            FamilyPreparation::Render,
        )
        .expect("planned Flowchart family should drive layout");
        assert_eq!(flowchart.family_id(), DiagramFamilyId::FLOWCHART);
    }
}

#[test]
fn flowchart_router_rejects_an_incompatible_planned_family() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model")
        .into_presentation();

    let error = match super::preparation::prepare_non_class_render(
        &parsed,
        &LayoutOptions::default(),
        FamilyRenderContext::resolve(session(), DiagramFamilyId::STATE, GraphLayoutBackend::Dagre),
        FamilyPreparation::Render,
    ) {
        Ok(_) => panic!("State cannot consume a Flowchart semantic model"),
        Err(error) => error,
    };
    let Error::InvalidModel { message } = error else {
        panic!("expected invalid model error")
    };
    assert!(message.contains("state"));
    assert!(message.contains("Flowchart"));
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn family_artifact_rejects_planned_family_drift() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model")
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();
    let FamilyRenderArtifact { family, .. } = artifact;

    let error = match FamilyRenderArtifact::new(
        &parsed,
        family,
        FamilyRenderContext::resolve(session(), DiagramFamilyId::STATE, GraphLayoutBackend::Dagre),
    ) {
        Ok(_) => panic!("family drift must be rejected"),
        Err(error) => error,
    };
    let Error::InvalidModel { message } = error else {
        panic!("expected invalid model error")
    };
    assert!(message.contains("state"));
    assert!(message.contains("flowchart"));
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-cytoscape"
))]
fn text_measurement_call_count(session: &RenderSession) -> u64 {
    session
        .text_measurement_report()
        .entries()
        .iter()
        .map(crate::environment::TextMeasurementSummary::count)
        .sum()
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[derive(Debug, Clone, Copy)]
enum SidecarHostOutcome {
    Success,
    Missing,
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct SidecarHostRequest {
    ordinal: usize,
    phase: TextMeasurementPhase,
    operation: TextMeasurementOperation,
    result_kind: TextMeasurementResultKind,
    text: String,
    font_size_bits: u64,
    max_width_bits: Option<u64>,
    wrap_mode: WrapMode,
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
struct SidecarRecordingHost {
    outcome: SidecarHostOutcome,
    requests: Mutex<Vec<SidecarHostRequest>>,
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
impl SidecarRecordingHost {
    fn new(outcome: SidecarHostOutcome) -> Self {
        Self {
            outcome,
            requests: Mutex::new(Vec::new()),
        }
    }

    fn snapshot(&self) -> Vec<SidecarHostRequest> {
        self.requests.lock().expect("host request trace").clone()
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
impl HostTextMeasurer for SidecarRecordingHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let ordinal = {
            let mut requests = self.requests.lock().expect("host request trace");
            let ordinal = requests.len();
            requests.push(SidecarHostRequest {
                ordinal,
                phase: request.phase,
                operation: request.operation,
                result_kind: request.operation.required_result_kind(),
                text: request.text.to_string(),
                font_size_bits: request.style.font_size.to_bits(),
                max_width_bits: request.max_width.map(f64::to_bits),
                wrap_mode: request.wrap_mode,
            });
            ordinal
        };

        match self.outcome {
            SidecarHostOutcome::Success => Ok(Some(sidecar_host_measurement(request, ordinal))),
            SidecarHostOutcome::Missing => Ok(None),
        }
    }
}

fn sidecar_host_measurement(
    request: HostTextMeasurementRequest<'_>,
    ordinal: usize,
) -> HostTextMeasurement {
    let state_delta = (ordinal % 7) as f64 / 32.0;
    let raw_width = request
        .text
        .lines()
        .map(|line| line.chars().count() as f64 * 8.0)
        .fold(0.0_f64, f64::max)
        + state_delta;
    let max_width = request
        .max_width
        .filter(|width| width.is_finite() && *width > 0.0);
    let line_count = max_width
        .map(|width| (raw_width / width).ceil() as usize)
        .unwrap_or(1)
        .max(request.text.lines().count())
        .max(1)
        .min(request.text.len().saturating_add(1));
    let metrics = TextMetrics {
        width: max_width.map_or(raw_width, |width| raw_width.min(width)),
        height: line_count as f64 * 20.0 + state_delta,
        line_count,
    };

    match request.operation.required_result_kind() {
        TextMeasurementResultKind::Metrics => HostTextMeasurement::Metrics(metrics),
        TextMeasurementResultKind::Length => {
            let length = match request.operation {
                TextMeasurementOperation::RawBBoxHeight
                | TextMeasurementOperation::SimpleBBoxHeight
                | TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                TextMeasurementOperation::CreateTextBBoxYOffset
                | TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                _ => raw_width,
            };
            HostTextMeasurement::Length(length)
        }
        TextMeasurementResultKind::HorizontalExtents => HostTextMeasurement::HorizontalExtents {
            left: raw_width / 2.0,
            right: raw_width / 2.0,
        },
        TextMeasurementResultKind::WrappedWithRawWidth => {
            HostTextMeasurement::WrappedWithRawWidth {
                metrics,
                raw_width: Some(raw_width),
            }
        }
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn flowchart_measurement_and_terminal_style_share_the_important_typography_winner() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A[Important Winner]
classDef strong font-size:30px!important
class A strong
style A font-size:10px
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Flowchart important typography")
        .expect("detect Flowchart important typography")
        .into_presentation();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.flowchart-important-typography").expect("profile id"),
        "1",
    )
    .expect("profile identity");
    let host = Arc::new(SidecarRecordingHost::new(SidecarHostOutcome::Success));
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            host.clone(),
            TextMeasurementPhase::ALL,
        ))
        .begin_session()
        .expect("Flowchart important typography session");
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Flowchart important typography")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Flowchart important typography");

    let matching = host
        .snapshot()
        .into_iter()
        .filter(|request| request.text.contains("Important Winner"))
        .collect::<Vec<_>>();
    assert!(!matching.is_empty(), "expected label measurement requests");
    assert!(
        matching
            .iter()
            .all(|request| request.font_size_bits == 30.0_f64.to_bits()),
        "measurement must retain the important class winner: {matching:#?}"
    );

    let terminal = flowchart_node_label_style(rendered.svg(), "A");
    assert!(terminal.contains("font-size:30px !important"), "{terminal}");
    assert!(
        !terminal.contains("font-size:10px !important"),
        "{terminal}"
    );
}

#[cfg(feature = "layout-cytoscape")]
fn prepare_mindmap_with_host_limit(
    max_layout_work_units: Option<usize>,
) -> (FamilyFixture, Arc<SidecarRecordingHost>) {
    let source = "mindmap\n  Root\n    First child\n    Second child\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse mindmap")
        .expect("detect mindmap")
        .into_presentation();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.mindmap-cose-budget").expect("profile id"),
        "1",
    )
    .expect("profile identity");
    let host = Arc::new(SidecarRecordingHost::new(SidecarHostOutcome::Success));
    let mut environment =
        crate::environment::RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::host_display(identity, host.clone(), TextMeasurementPhase::ALL),
        );
    if let Some(limit) = max_layout_work_units {
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, limit)
            .expect("layout work limit");
        environment = environment.with_resource_policy(policy);
    }
    let session = environment.begin_session().expect("render session");
    ((parsed, LayoutOptions::default(), session), host)
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn public_flowchart_preparation_enables_prepared_svg_label_reuse() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A -->|control label| B
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("detect flowchart")
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session())
        .expect("prepare public flowchart artifact");
    let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
        panic!("expected Flowchart family artifact");
    };

    assert!(
        flowchart
            .svg_label_sidecar()
            .node_owner("A", false)
            .is_some(),
        "the public Flowchart preparation path must build the label sidecar"
    );
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn prepared_self_loop_edge_label_keeps_its_semantic_owner_through_family_dispatch() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A ordinary-edge@-->|ordinary owner sentinel| B
A self-loop-edge@-->|self loop semantic owner keeps wrapped label rows through the logical render id alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron| A
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("detect flowchart")
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session())
        .expect("prepare flowchart family artifact");

    let rendered_svg = {
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            panic!("expected Flowchart family artifact");
        };
        let model = crate::flowchart::FlowchartRenderModelRef::new(
            flowchart.pair().semantic(),
            flowchart.render_context(),
        );
        let edge = model.edges.get(1).expect("self-loop edge");
        assert_eq!(edge.id, "self-loop-edge");
        let label = model
            .edge_label_for_render(1, edge)
            .expect("self-loop edge label");
        let owner = flowchart
            .svg_label_sidecar()
            .edge_owner(edge.id.as_str(), false)
            .expect("semantic self-loop owner");
        assert_eq!(owner, crate::flowchart::FlowchartSvgLabelOwner::Edge(1));
        assert_eq!(
            flowchart
                .svg_label_sidecar()
                .edge_owner("A-cyclic-special-mid", false),
            None
        );
        assert!(
            flowchart
                .pair()
                .layout()
                .edges
                .iter()
                .any(|edge| edge.id == "self-loop-edge")
        );

        let config = crate::flowchart::FlowchartConfigView::new(
            artifact.metadata.effective_config.as_value(),
        );
        let font_family = config.font_family();
        let render_style = config.render_text_style(&font_family, config.render_font_size());
        let edge_width = config.layout_settings().edge_label_wrapping_width;
        let render_measurer = artifact
            .context
            .session
            .text_measurer(crate::environment::TextMeasurementPhase::SvgBBox);
        let calls_before = text_measurement_call_count(&artifact.context.session);
        let plan = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
            Some(flowchart.svg_label_sidecar()),
            Some(owner),
            label,
            &render_measurer,
            &render_style,
            Some(edge_width),
            true,
            crate::flowchart::FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(
            &plan,
            crate::flowchart::FlowchartSvgLabelRenderPlan::Prepared { .. }
        ));
        let wrapped = plan.wrapped_lines();
        assert!(matches!(&wrapped, std::borrow::Cow::Borrowed(_)));
        assert!(wrapped.len() >= 2, "{wrapped:?}");
        assert_eq!(
            text_measurement_call_count(&artifact.context.session),
            calls_before,
            "a prepared self-loop label must not invoke the SVG measurer again"
        );
        drop(wrapped);
        drop(plan);

        let hits_before_render = flowchart.svg_label_sidecar().prepared_hit_count(owner);
        let (svg, _) = render_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
            crate::svg::SvgGeneration::Full,
        )
        .expect("render self-loop SVG");
        assert!(
            flowchart.svg_label_sidecar().prepared_hit_count(owner) > hits_before_render,
            "the real Flowchart SVG renderer must consume the prepared self-loop label"
        );
        svg
    };

    assert!(!rendered_svg.contains("cyclic-special"), "{}", rendered_svg);
    let document = crate::xml_dom::Document::parse(&rendered_svg).expect("valid self-loop SVG");
    let logical_path = document.descendants().any(|node| {
        node.has_tag_name("path") && node.attribute("data-id") == Some("self-loop-edge")
    });
    assert!(logical_path, "{rendered_svg}");
    let label_group = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some("self-loop-edge"))
        .expect("logical self-loop label group");
    let text = label_group
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .collect::<String>();
    assert!(text.contains("self loop semantic owner"), "{text:?}");
    let row_count = label_group
        .descendants()
        .filter(|node| {
            node.has_tag_name("tspan")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "text-outer-tspan")
                })
        })
        .count();
    assert!(row_count >= 2, "rows={row_count}: {rendered_svg}");
}

#[test]
fn prepared_swimlane_edge_label_is_consumed_by_the_real_svg_renderer() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
swimlane-beta LR
A styled@-->|swimlane semantic owner keeps wrapped label rows through the generated labelRect| B
linkStyle default font-size:24px,font-weight:bold
linkStyle 0 font-size:12px,font-style:italic
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Swimlane")
        .expect("detect Swimlane")
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session())
        .expect("prepare Swimlane family artifact");

    let rendered_svg = {
        let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
            panic!("expected Swimlane family artifact");
        };
        let model = crate::flowchart::FlowchartRenderModelRef::new(
            swimlane.pair().semantic(),
            swimlane.render_context(),
        );
        let edge = model.edges.first().expect("styled Swimlane edge");
        assert_eq!(edge.id, "styled");
        let label = model
            .edge_label_for_render(0, edge)
            .expect("Swimlane edge label");
        let owner = swimlane
            .svg_label_sidecar()
            .edge_owner(edge.id.as_str(), true)
            .expect("semantic Swimlane edge owner");
        assert_eq!(
            owner,
            crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(0)
        );
        assert_eq!(
            swimlane.pair().layout().edges[0].label_node_id.as_deref(),
            Some("edge-label-A-B-styled")
        );

        let config = crate::flowchart::FlowchartConfigView::new(
            artifact.metadata.effective_config.as_value(),
        );
        let font_family = config.font_family();
        let base_style = config.render_text_style(&font_family, config.render_font_size());
        let default_edge_styles = model
            .edge_defaults
            .as_ref()
            .map_or(&[][..], |defaults| defaults.style.as_slice());
        let label_style = crate::flowchart::flowchart_swimlane_label_rect_text_style(
            &base_style,
            default_edge_styles,
            &edge.style,
        );
        let render_measurer = artifact
            .context
            .session
            .text_measurer(crate::environment::TextMeasurementPhase::SvgBBox);
        let plan = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
            Some(swimlane.svg_label_sidecar()),
            Some(owner),
            label,
            &render_measurer,
            label_style.as_ref(),
            Some(config.render_wrapping_width()),
            true,
            crate::flowchart::FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(
            &plan,
            crate::flowchart::FlowchartSvgLabelRenderPlan::Prepared { .. }
        ));
        let wrapped = plan.wrapped_lines();
        assert!(matches!(&wrapped, std::borrow::Cow::Borrowed(_)));
        assert!(wrapped.len() >= 2, "{wrapped:?}");
        drop(wrapped);
        drop(plan);

        let hits_before_render = swimlane.svg_label_sidecar().prepared_hit_count(owner);
        let (svg, _) = render_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
            crate::svg::SvgGeneration::Full,
        )
        .expect("render Swimlane SVG");
        assert!(
            swimlane.svg_label_sidecar().prepared_hit_count(owner) > hits_before_render,
            "the real Swimlane SVG renderer must consume the prepared labelRect"
        );
        svg
    };

    let document = crate::xml_dom::Document::parse(&rendered_svg).expect("valid Swimlane SVG");
    let label_group = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-et") == Some("edge-label")
                && node.attribute("data-id") == Some("styled")
        })
        .expect("generated labelRect group");
    let visible = label_group
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .flat_map(str::chars)
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    assert_eq!(
        visible,
        "swimlanesemanticownerkeepswrappedlabelrowsthroughthegeneratedlabelRect"
    );
}

fn prepare_with_model_item_limit(source: &str, max_model_items: usize) -> FamilyFixture {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxModelItems,
                    max_model_items,
                )
                .unwrap(),
        )
        .begin_session()
        .unwrap();
    (parsed, LayoutOptions::default(), session)
}

fn prepare_with_layout_work_limit(source: &str, max_layout_work_units: usize) -> FamilyFixture {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxLayoutWorkUnits,
                    max_layout_work_units,
                )
                .unwrap(),
        )
        .begin_session()
        .unwrap();
    (parsed, LayoutOptions::default(), session)
}

#[cfg(feature = "layout-dagre")]
fn prepare_with_unbounded_layout_work(source: &str) -> FamilyFixture {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(crate::resources::RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    (parsed, LayoutOptions::default(), session)
}

fn assert_model_item_limit(error: Error, actual: usize, max: usize) {
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_model_items resource limit error")
    };
    assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(limit.limit, "max_model_items");
    assert_eq!(limit.actual, actual);
    assert_eq!(limit.max, max);
}

#[test]
fn session_report_accounts_for_every_metered_layout_family() {
    let cases = [
        #[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
        (
            "classDiagram\nclass A\nclass B\nA --> B\n",
            DiagramFamilyId::CLASS,
        ),
        #[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
        (
            "stateDiagram-v2\n[*] --> Idle\nIdle --> Active\n",
            DiagramFamilyId::STATE,
        ),
        #[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
        (
            "erDiagram\nCUSTOMER ||--o{ ORDER : places\n",
            DiagramFamilyId::ER,
        ),
        #[cfg(feature = "layout-shoal")]
        (
            "---\nconfig:\n  layout: shoal\n---\nflowchart LR\nA --> B",
            DiagramFamilyId::FLOWCHART,
        ),
        (
            "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    First child\n    Second child\n",
            DiagramFamilyId::MINDMAP,
        ),
        (
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: hello\n",
            DiagramFamilyId::SEQUENCE,
        ),
        (
            "kanban\n  todo[Todo]\n    task[Task]\n",
            DiagramFamilyId::KANBAN,
        ),
        #[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
        (
            "requirementDiagram\nrequirement req1 {\n  id: 1\n  text: Login\n  risk: high\n}\n",
            DiagramFamilyId::REQUIREMENT,
        ),
        ("sankey-beta\nA,B,10\n", DiagramFamilyId::SANKEY),
        (
            "radar-beta\naxis A,B,C\ncurve score{1,2,3}\n",
            DiagramFamilyId::RADAR,
        ),
        (
            "venn-beta\nset A[\"Core\"]:20\nset B[\"Editor\"]:14\nunion A,B[\"Shared\"]:4\n",
            DiagramFamilyId::VENN,
        ),
        (
            "treemap\n\"Section\"\n  \"Leaf\": 4\n",
            DiagramFamilyId::TREEMAP,
        ),
        #[cfg(feature = "layout-cytoscape")]
        (
            "mindmap\n  Root\n    First child\n    Second child\n",
            DiagramFamilyId::MINDMAP,
        ),
    ];

    for (source, expected_family) in cases {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::default())
            .unwrap()
            .expect("the layout-work fixture should produce a render model")
            .into_presentation();
        let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();

        assert_eq!(artifact.family_id(), expected_family);
        assert!(
            artifact.context.session().report().layout_work_units() > 0,
            "{expected_family} must contribute layout work to the session report"
        );
    }
}

#[cfg(feature = "layout-dagre")]
#[test]
fn dagre_family_work_budgets_are_exact_and_preserve_layout_output() {
    let cases = [
        (
            "classDiagram\nnamespace Outer {\n  class A\n  class B\n}\nA --> B\n",
            DiagramFamilyId::CLASS,
        ),
        (
            "stateDiagram-v2\nstate Parent {\n  [*] --> Idle\n  Idle --> Active\n}\nParent --> Outside\n",
            DiagramFamilyId::STATE,
        ),
        (
            "erDiagram\nNODE {\n  string id\n}\nNODE ||--o{ NODE : leads\n",
            DiagramFamilyId::ER,
        ),
    ];

    for (body, expected_family) in cases {
        let source = &format!("---\nconfig:\n  layout: dagre\n---\n{body}");
        let (fixture_parsed_1, fixture_layout_1, fixture_session_1) =
            prepare_with_unbounded_layout_work(source);
        let unbounded = prepare(&fixture_parsed_1, &fixture_layout_1, fixture_session_1).unwrap();
        assert_eq!(unbounded.family_id(), expected_family);
        let exact = unbounded.context.session.report().layout_work_units();
        assert!(exact > 0, "{expected_family} must report layout work");
        let expected_layout = unbounded.layout_json().unwrap();

        let (fixture_parsed_2, fixture_layout_2, fixture_session_2) =
            prepare_with_layout_work_limit(source, exact);
        let bounded = prepare(&fixture_parsed_2, &fixture_layout_2, fixture_session_2).unwrap();
        assert_eq!(bounded.context.session.report().layout_work_units(), exact);
        assert_eq!(bounded.layout_json().unwrap(), expected_layout);

        let (fixture_parsed_3, fixture_layout_3, fixture_session_3) =
            prepare_with_layout_work_limit(source, exact - 1);
        let error = match prepare(&fixture_parsed_3, &fixture_layout_3, fixture_session_3) {
            Ok(_) => panic!("{expected_family} exact minus one unexpectedly succeeded"),
            Err(error) => error,
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected {expected_family} layout work rejection")
        };
        assert_eq!(limit.limit, "max_layout_work_units");
        assert_eq!(limit.max, exact - 1);
    }
}

#[cfg(feature = "layout-cytoscape")]
#[test]
fn default_mindmap_cose_reports_kernel_work_and_has_an_exact_resource_boundary() {
    let ((unbounded_parsed, unbounded_options, unbounded_session), unbounded_host) =
        prepare_mindmap_with_host_limit(None);
    let unbounded_result = prepare(&unbounded_parsed, &unbounded_options, unbounded_session);
    let unbounded = unbounded_result.expect("unbounded COSE mindmap");
    let exact = unbounded.context.session.report().layout_work_units();
    assert!(
        exact > 78,
        "kernel work must exceed the 3-node adapter estimate"
    );
    let unbounded_layout = unbounded.layout_json().expect("unbounded layout json");
    let unbounded_trace = unbounded_host.snapshot();
    assert!(!unbounded_trace.is_empty());

    let ((exact_parsed, exact_options, exact_session), exact_host) =
        prepare_mindmap_with_host_limit(Some(exact));
    let exact_result = prepare(&exact_parsed, &exact_options, exact_session);
    let exact_artifact = exact_result.expect("exact COSE budget");
    assert_eq!(
        exact_artifact.context.session.report().layout_work_units(),
        exact
    );
    assert_eq!(
        exact_artifact.layout_json().expect("exact layout json"),
        unbounded_layout
    );
    assert_eq!(exact_host.snapshot(), unbounded_trace);

    let ((short_parsed, short_options, short_session), _short_host) =
        prepare_mindmap_with_host_limit(Some(exact - 1));
    let short_result = prepare(&short_parsed, &short_options, short_session);
    let error = match short_result {
        Ok(_) => panic!("exact minus one must reject COSE work"),
        Err(error) => error,
    };
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_layout_work_units rejection")
    };
    assert_eq!(limit.limit, "max_layout_work_units");
    assert_eq!(limit.max, exact - 1);

    let ((early_parsed, early_options, early_session), early_host) =
        prepare_mindmap_with_host_limit(Some(1));
    let early_result = prepare(&early_parsed, &early_options, early_session);
    assert!(matches!(early_result, Err(Error::ResourceLimitExceeded(_))));
    assert!(
        early_host.snapshot().is_empty(),
        "adapter admission must reject before the first host measurement"
    );
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn requirement_layout_projection_excludes_operation_prepared_labels() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            r#"requirementDiagram
requirement req1 {
  id: 1
  text: User logs in
  risk: high
}
element system {
  type: service
}
system - satisfies -> req1
"#,
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Requirement source should produce a render model")
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout = &projection["layout"]["RequirementDiagram"];
    let fields = layout
        .as_object()
        .expect("Requirement layout projection should remain an object");

    assert_eq!(artifact.family_id(), DiagramFamilyId::REQUIREMENT);
    assert!(fields.contains_key("nodes"));
    assert!(fields.contains_key("edges"));
    assert!(fields.contains_key("bounds"));
    assert!(!fields.contains_key("labels"));
    assert!(!layout.to_string().contains("display_text"));
    let serialized_projection = projection.to_string();
    assert!(!serialized_projection.contains("max_width_px"));
    assert!(!serialized_projection.contains("keep_centered"));
    assert!(!serialized_projection.contains("divider_y_offset"));
    assert!(
        serde_json::from_value::<RequirementDiagramLayout>(layout.clone()).is_ok(),
        "prepared labels must not alter the public Requirement layout schema"
    );
}

#[cfg(feature = "layout-dagre")]
#[test]
fn dagre_flowchart_family_renderer_reuses_prepared_labels_by_semantic_owner() {
    assert_flowchart_prepared_labels_reused_by_semantic_owner(
        "dagre",
        crate::flowchart::FlowchartSvgLabelOwner::EmptySubgraphNode,
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn elk_flowchart_family_renderer_reuses_prepared_labels_by_semantic_owner() {
    assert_flowchart_prepared_labels_reused_by_semantic_owner(
        "elk",
        crate::flowchart::FlowchartSvgLabelOwner::SubgraphTitle,
    );
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn assert_flowchart_prepared_labels_reused_by_semantic_owner(
    layout: &str,
    empty_subgraph_owner: fn(usize) -> crate::flowchart::FlowchartSvgLabelOwner,
) {
    let source = format!(
        r#"---
config:
  layout: {layout}
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
subgraph S[Service title]
  A[Node label]
end
subgraph E[Empty title]
end
A labeled@-->|edge semantic owner wraps alpha beta gamma delta epsilon| B[Second node]
"#
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Flowchart")
        .expect("detect Flowchart")
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session())
        .expect("prepare Flowchart family artifact");

    let owners = {
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            panic!("expected Flowchart family artifact");
        };
        assert_eq!(
            flowchart.pair().layout().uses_elk_adapter_dom,
            layout == "elk"
        );
        let model = crate::flowchart::FlowchartRenderModelRef::new(
            flowchart.pair().semantic(),
            flowchart.render_context(),
        );
        let node_index = model
            .nodes
            .iter()
            .position(|node| node.id == "A")
            .expect("semantic node A");
        let empty_subgraph_index = model
            .subgraphs
            .iter()
            .position(|subgraph| subgraph.id == "E")
            .expect("semantic empty subgraph E");
        let cluster_index = model
            .subgraphs
            .iter()
            .position(|subgraph| subgraph.id == "S")
            .expect("semantic cluster S");
        let edge_index = model
            .edges
            .iter()
            .position(|edge| edge.id == "labeled")
            .expect("semantic labeled edge");
        let sidecar = flowchart.svg_label_sidecar();

        let expected_empty_subgraph_owner = empty_subgraph_owner(empty_subgraph_index);
        let actual_empty_subgraph_owner = match expected_empty_subgraph_owner {
            crate::flowchart::FlowchartSvgLabelOwner::EmptySubgraphNode(_) => sidecar
                .node_owner("E", false)
                .expect("Dagre empty subgraph node owner"),
            crate::flowchart::FlowchartSvgLabelOwner::SubgraphTitle(_) => sidecar
                .subgraph_title_owner("E")
                .expect("ELK empty subgraph title owner"),
            _ => panic!("unexpected empty subgraph owner in test scenario"),
        };
        let owners = [
            sidecar.node_owner("A", false).expect("node owner"),
            sidecar.edge_owner("labeled", false).expect("edge owner"),
            actual_empty_subgraph_owner,
            sidecar
                .subgraph_title_owner("S")
                .expect("cluster title owner"),
        ];
        assert_eq!(
            owners,
            [
                crate::flowchart::FlowchartSvgLabelOwner::Node(node_index),
                crate::flowchart::FlowchartSvgLabelOwner::Edge(edge_index),
                expected_empty_subgraph_owner,
                crate::flowchart::FlowchartSvgLabelOwner::SubgraphTitle(cluster_index),
            ]
        );
        owners
    };

    let (prepared_hits_before, source_plans_before) = {
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            unreachable!();
        };
        let sidecar = flowchart.svg_label_sidecar();
        (
            owners.map(|owner| sidecar.prepared_hit_count(owner)),
            owners.map(|owner| sidecar.source_plan_count(owner)),
        )
    };

    let (svg, _) = render_family_artifact_svg(
        &artifact,
        &SvgRenderOptions::default(),
        &SvgDebugOptions::default(),
        crate::svg::SvgGeneration::Full,
    )
    .expect("render Flowchart SVG");

    let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
        unreachable!();
    };
    let sidecar = flowchart.svg_label_sidecar();
    for (index, owner) in owners.into_iter().enumerate() {
        let prepared_hits = sidecar.prepared_hit_count(owner);
        let source_plans = sidecar.source_plan_count(owner);
        assert!(
            prepared_hits > prepared_hits_before[index],
            "real Flowchart SVG emission must consume {owner:?}; prepared_hits={prepared_hits}, source_plans={source_plans}"
        );
        assert_eq!(
            source_plans, source_plans_before[index],
            "eligible owner {owner:?} must not fall back to render-time preparation"
        );
    }
    let document = crate::xml_dom::Document::parse(&svg).expect("valid Flowchart SVG");
    let visible_text = document
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .flat_map(str::chars)
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(visible_text.contains("Servicetitle"), "{visible_text}");
    assert!(visible_text.contains("Emptytitle"), "{visible_text}");
    assert!(visible_text.contains("edgesemanticowner"), "{visible_text}");
}

#[derive(Debug)]
struct SwimlanePreparedMathRenderer {
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl crate::math::MathRenderer for SwimlanePreparedMathRenderer {
    fn render_html_label(&self, _text: &str, _config: &PresentationConfig) -> Option<String> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(r#"<span class="external-math">rendered</span>"#.to_owned())
    }

    fn measure_html_label(
        &self,
        _text: &str,
        _config: &PresentationConfig,
        _style: &crate::text::TextStyle,
        _max_width_px: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> Option<TextMetrics> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(TextMetrics {
            width: 42.0,
            height: 18.0,
            line_count: 1,
        })
    }
}

#[test]
fn swimlane_group_title_reuses_prepared_label_for_terminal_emission() {
    let source = r#"swimlane-beta LR
subgraph Lane["$$x^2$$"]
  A[Node]
end
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Swimlane")
        .expect("detect Swimlane")
        .into_presentation();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(Arc::new(SwimlanePreparedMathRenderer {
            calls: Arc::clone(&calls),
        }))
        .begin_session()
        .expect("begin Swimlane math session");
    let artifact =
        prepare(&parsed, &LayoutOptions::default(), session).expect("prepare Swimlane math title");

    let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
        panic!("expected Swimlane family artifact");
    };
    let owner = swimlane
        .svg_label_sidecar()
        .swimlane_group_title_owner("Lane")
        .expect("Swimlane group-title owner");
    assert!(matches!(
        swimlane.svg_label_sidecar().prepared_math(owner, "$$x^2$$"),
        crate::flowchart::FlowchartPreparedLabelResolution::PreparedMath(_)
    ));
    let calls_after_preparation = calls.load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        calls_after_preparation > 0,
        "preparation must invoke the math backend"
    );

    let (svg, _) = render_family_artifact_svg(
        &artifact,
        &SvgRenderOptions::default(),
        &SvgDebugOptions::default(),
        crate::svg::SvgGeneration::Full,
    )
    .expect("render Swimlane math title");
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::Relaxed),
        calls_after_preparation,
        "terminal emission must reuse prepared math without calling its backend"
    );
    assert!(svg.contains(r#"class="merman-prepared-math""#), "{svg}");
    assert!(svg.contains(r#"class="external-math""#), "{svg}");
    assert!(!svg.contains(">$$x^2$$<"), "{svg}");
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn flowchart_special_shape_intersections_reuse_layout_label_metrics() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
P[plain]
S([stadium label])
H{{hexagon label}}
D@{ shape: doc, label: "document label", labelType: "string" }
P --> S
P --> H
P --> D
S --> P
H --> P
D --> P
"#,
            ParseOptions::strict(),
        )
        .expect("parse Flowchart")
        .expect("detect Flowchart")
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session())
        .expect("prepare Flowchart family artifact");
    let operation_count =
        |artifact: &FamilyRenderArtifact,
         operation: crate::environment::TextMeasurementOperation| {
            artifact
                .context
                .session()
                .text_measurement_report()
                .entries()
                .iter()
                .filter(|entry| entry.provenance().operation == operation)
                .map(|entry| entry.count())
                .sum::<u64>()
        };
    let operation_counts = |artifact: &FamilyRenderArtifact| {
        [
            operation_count(
                artifact,
                crate::environment::TextMeasurementOperation::Wrapped,
            ),
            operation_count(
                artifact,
                crate::environment::TextMeasurementOperation::ComputedLength,
            ),
        ]
    };
    let operations_before = operation_counts(&artifact);
    let (owners, source_plans_before) = {
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            panic!("expected Flowchart family artifact");
        };
        let node_ids = flowchart
            .pair()
            .semantic()
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        let sidecar = flowchart.svg_label_sidecar();
        let owners = ["S", "H", "D"].map(|id| {
            sidecar.node_owner(id, false).unwrap_or_else(|| {
                panic!("missing semantic label owner for {id}; nodes={node_ids:?}")
            })
        });
        (owners, owners.map(|owner| sidecar.source_plan_count(owner)))
    };

    let (svg, _) = render_family_artifact_svg(
        &artifact,
        &SvgRenderOptions::default(),
        &SvgDebugOptions::default(),
        crate::svg::SvgGeneration::Full,
    )
    .expect("render Flowchart SVG");

    assert_eq!(
        operation_counts(&artifact),
        operations_before,
        "special-shape edge intersections must consume layout label metrics"
    );
    let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
        unreachable!();
    };
    for (index, owner) in owners.into_iter().enumerate() {
        assert_eq!(
            flowchart.svg_label_sidecar().source_plan_count(owner),
            source_plans_before[index],
            "special-shape label {owner:?} must reuse its prepared SVG plan"
        );
    }
    let document = crate::xml_dom::Document::parse(&svg).expect("valid Flowchart SVG");
    let visible_text = document
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .flat_map(str::chars)
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(visible_text.contains("stadiumlabel"), "{visible_text}");
    assert!(visible_text.contains("hexagonlabel"), "{visible_text}");
    assert!(visible_text.contains("documentlabel"), "{visible_text}");
}

#[test]
fn swimlane_family_renderer_reuses_the_original_semantic_edge_owner() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
swimlane-beta LR
A --> C
A styled@-->|swimlane semantic owner wraps alpha beta gamma delta epsilon| B
"#,
            ParseOptions::strict(),
        )
        .expect("parse Swimlane")
        .expect("detect Swimlane")
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session())
        .expect("prepare Swimlane family artifact");

    let (owner, hits_before, source_plans_before) = {
        let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
            panic!("expected Swimlane family artifact");
        };
        let model = crate::flowchart::FlowchartRenderModelRef::new(
            swimlane.pair().semantic(),
            swimlane.render_context(),
        );
        let edge_index = model
            .edges
            .iter()
            .position(|edge| edge.id == "styled")
            .expect("semantic styled edge");
        let owner = swimlane
            .svg_label_sidecar()
            .edge_owner("styled", true)
            .expect("Swimlane edge-label owner");
        assert_eq!(
            owner,
            crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(edge_index)
        );
        assert_eq!(
            swimlane
                .pair()
                .layout()
                .edges
                .iter()
                .find(|edge| edge.id == "styled")
                .and_then(|edge| edge.label_node_id.as_deref()),
            Some("edge-label-A-B-styled")
        );
        (
            owner,
            swimlane.svg_label_sidecar().prepared_hit_count(owner),
            swimlane.svg_label_sidecar().source_plan_count(owner),
        )
    };

    let (svg, _) = render_family_artifact_svg(
        &artifact,
        &SvgRenderOptions::default(),
        &SvgDebugOptions::default(),
        crate::svg::SvgGeneration::Full,
    )
    .expect("render Swimlane SVG");

    let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
        unreachable!();
    };
    assert!(
        swimlane.svg_label_sidecar().prepared_hit_count(owner) > hits_before,
        "real Swimlane SVG emission must consume the prepared labelRect"
    );
    assert_eq!(
        swimlane.svg_label_sidecar().source_plan_count(owner),
        source_plans_before
    );
    let document = crate::xml_dom::Document::parse(&svg).expect("valid Swimlane SVG");
    let visible_text = document
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .flat_map(str::chars)
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(
        visible_text.contains("swimlanesemanticowner"),
        "{visible_text}"
    );
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn flowchart_family_layout_projections_exclude_operation_prepared_labels() {
    for (source, variant) in [
        ("flowchart LR\nA -->|Flowchart label| B\n", "FlowchartV2"),
        (
            "swimlane-beta LR\nA -->|Swimlane label| B\n",
            "SwimlaneDiagram",
        ),
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse Flowchart family")
            .expect("detect Flowchart family")
            .into_presentation();
        let artifact = prepare(&parsed, &LayoutOptions::default(), session())
            .expect("prepare Flowchart family artifact");
        let projection = artifact.layout_json().expect("project public layout JSON");
        let layout = &projection["layout"][variant];
        let serialized = layout.to_string();

        for internal_field in [
            "wrapped_lines",
            "plain_text",
            "binding",
            "render_ids",
            "svg_label_sidecar",
        ] {
            assert!(
                !serialized.contains(internal_field),
                "{variant} leaked operation-local field {internal_field}: {serialized}"
            );
        }

        match variant {
            "FlowchartV2" => assert!(
                serde_json::from_value::<FlowchartLayout>(layout.clone()).is_ok(),
                "prepared labels must not alter the public Flowchart layout schema"
            ),
            "SwimlaneDiagram" => assert!(
                serde_json::from_value::<SwimlaneLayout>(layout.clone()).is_ok(),
                "prepared labels must not alter the public Swimlane layout schema"
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn layout_json_projects_its_paired_semantics_and_layout() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "treemap\ntitle Product Map\n\"Root\"\n  \"Leaf\": 42\n",
            ParseOptions::strict(),
        )
        .expect("parse Treemap render model")
        .expect("detect Treemap family")
        .into_presentation();
    let compatibility = parsed.compatibility_json().unwrap();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();
    let projected = artifact.layout_json().unwrap();
    assert_eq!(projected["semantic"], compatibility);
    assert_eq!(projected["meta"]["diagram_type"], "treemap");
    assert!(projected["layout"]["TreemapDiagram"].is_object());
    assert_eq!(artifact.layout_json().unwrap(), projected);
}

#[test]
fn layout_json_keeps_empty_output_facts_after_preparing_canonical_payloads() {
    let engine = MermaidEngine::new();
    for (diagram_type, header) in [
        ("info", "info"),
        ("pie", "pie"),
        ("journey", "journey"),
        ("timeline", "timeline"),
        ("packet", "packet-beta"),
        ("radar", "radar-beta"),
    ] {
        for source in ["", header] {
            let parsed = engine
                .parse_diagram_for_render_model_with_type_sync(
                    diagram_type,
                    source,
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap()
                .into_presentation();
            let expected = parsed.compatibility_json().unwrap();
            let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();
            let projected = artifact.layout_json().unwrap();
            assert_eq!(
                projected["semantic"], expected,
                "{diagram_type}: {source:?}"
            );
            if source.is_empty() {
                assert_eq!(projected["semantic"], json!({}), "{diagram_type}");
            } else {
                assert_eq!(projected["semantic"]["type"], diagram_type);
            }
        }
    }
}

#[test]
fn layout_json_observes_cancellation_inside_the_owned_semantic_projection() {
    let control = merman_model::OperationControl::new();
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nA->>B: Message",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    control.cancel_after_checkpoints(1);
    let Error::Cancelled(cancelled) = artifact.layout_json().unwrap_err() else {
        panic!("projection cancellation must keep its category");
    };
    assert_eq!(cancelled.phase, OperationPhase::Semantic);
}

#[cfg(feature = "diagram-block")]
#[test]
fn layout_json_preserves_the_original_owned_projection_error() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("block-beta\nA\n", ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let (_, RenderSemanticModel::Block(mut invalid_model), _) = parsed.clone().into_parts() else {
        panic!("Block model");
    };
    invalid_model.blocks_flat[0].children.push(usize::MAX);
    let mut artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();
    let BuiltinFamilyArtifact::Block(block) = &mut artifact.family else {
        panic!("Block artifact");
    };
    block.pair.semantic = &invalid_model;
    let Error::SemanticProjection(error) = artifact.layout_json().unwrap_err() else {
        panic!("the family projection diagnostic must remain structured");
    };
    assert_eq!(error.diagram_type(), "block");
    assert!(error.message().contains("invalid Block child index"));
}

#[cfg(feature = "layout-dagre")]
#[test]
fn dagre_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
    let source = "---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA --> B";
    let (fixture_parsed_6, fixture_layout_6, fixture_session_6) =
        prepare_with_model_item_limit(source, 3);
    let artifact = prepare(&fixture_parsed_6, &fixture_layout_6, fixture_session_6).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::FLOWCHART);

    let (fixture_parsed_7, fixture_layout_7, fixture_session_7) =
        prepare_with_model_item_limit(source, 2);
    let error = match prepare(&fixture_parsed_7, &fixture_layout_7, fixture_session_7) {
        Err(error) => error,
        Ok(_) => panic!("flowchart above the node limit unexpectedly rendered"),
    };
    assert_model_item_limit(error, 3, 2);
}

#[test]
fn swimlane_node_limit_accepts_boundary_and_rejects_one_beyond() {
    let source = "swimlane-beta LR\nA --> B";
    let (fixture_parsed_8, fixture_layout_8, fixture_session_8) =
        prepare_with_model_item_limit(source, 3);
    let artifact = prepare(&fixture_parsed_8, &fixture_layout_8, fixture_session_8).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::SWIMLANE);

    let (fixture_parsed_9, fixture_layout_9, fixture_session_9) =
        prepare_with_model_item_limit(source, 2);
    let error = match prepare(&fixture_parsed_9, &fixture_layout_9, fixture_session_9) {
        Err(error) => error,
        Ok(_) => panic!("swimlane above the node limit unexpectedly rendered"),
    };
    assert_model_item_limit(error, 3, 2);
}

#[test]
fn swimlane_rejects_pairwise_routing_work_before_layout() {
    let source = "swimlane-beta LR\nA --> B\nB --> C";
    let (fixture_parsed_4, fixture_layout_4, fixture_session_4) =
        prepare_with_layout_work_limit(source, 1_000);
    let artifact = prepare(&fixture_parsed_4, &fixture_layout_4, fixture_session_4).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::SWIMLANE);

    let (fixture_parsed_5, fixture_layout_5, fixture_session_5) =
        prepare_with_layout_work_limit(source, 1);
    let error = match prepare(&fixture_parsed_5, &fixture_layout_5, fixture_session_5) {
        Err(error) => error,
        Ok(_) => panic!("swimlane above the layout work limit unexpectedly rendered"),
    };
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_layout_work_units resource limit error");
    };
    assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(limit.limit, "max_layout_work_units");
    assert!(limit.actual > limit.max);
    assert_eq!(limit.max, 1);
}

#[test]
fn mindmap_node_limit_is_checked_before_layout_allocation_or_backend_dispatch() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "mindmap\n  Root\n    First child\n    Second child\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("mindmap source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxModelItems, 4)
                .unwrap(),
        )
        .begin_session()
        .unwrap();

    let error = match prepare(&parsed, &LayoutOptions::default(), session) {
        Err(error) => error,
        Ok(_) => panic!("mindmap above the node limit unexpectedly reached layout"),
    };
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_model_items resource limit error");
    };
    assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(limit.limit, "max_model_items");
    assert_eq!(limit.actual, 5);
    assert_eq!(limit.max, 4);
}

#[test]
fn flowchart_math_capability_uses_parser_owned_render_spelling() {
    for source in [
        "flowchart TD\nA[\"#36;#36;node#36;#36;\"]\n",
        "flowchart TD\nA -->|#36;#36;edge#36;#36;| B\n",
        "flowchart TD\nsubgraph S[\"#36;#36;group#36;#36;\"]\nA\nend\n",
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert!(
            !plan
                .required_capabilities()
                .contains(&RenderCapability::Math),
            "encoded dollar entities remain ordinary createText input: {source}"
        );
        if plan.is_ready() {
            prepare(&parsed, &LayoutOptions::default(), session)
                .expect("encoded dollar entities must not require a Math renderer");
        }
    }

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA[\"$$x$$\"]\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .without_math_renderer()
        .begin_session()
        .unwrap();
    let plan = plan_render(&parsed, &session).unwrap();
    assert!(
        plan.required_capabilities()
            .contains(&RenderCapability::Math)
    );
    assert!(
        plan.missing_capabilities()
            .contains(&RenderCapability::Math)
    );
}

#[derive(Debug)]
struct CountingMathRenderer {
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl crate::math::MathRenderer for CountingMathRenderer {
    fn render_html_label(
        &self,
        text: &str,
        _config: &merman_model::presentation::PresentationConfig,
    ) -> Option<String> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        text.contains("$$")
            .then(|| "<span>rendered</span>".to_string())
    }
}

#[test]
fn constrained_profile_rejects_math_before_invoking_an_opaque_backend() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA[\"$$x$$\"]\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model")
        .into_presentation();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(CountingMathRenderer {
            calls: std::sync::Arc::clone(&calls),
        }))
        .with_resource_policy(crate::resources::RenderResourcePolicy::constrained())
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert!(
        plan.required_capabilities()
            .contains(&RenderCapability::Math)
    );
    assert!(
        plan.missing_capabilities()
            .contains(&RenderCapability::Math)
    );
    let error = match prepare(&parsed, &LayoutOptions::default(), session) {
        Err(error) => error,
        Ok(_) => panic!("constrained math unexpectedly reached opaque backend dispatch"),
    };
    let expected = if cfg!(any(feature = "layout-dagre", feature = "layout-elk")) {
        RenderCapability::Math
    } else {
        RenderCapability::LayoutElk
    };
    assert!(matches!(
        error,
        Error::MissingCapability {
            capability,
            ref diagram_type,
        } if capability == expected && diagram_type == "flowchart-v2"
    ));
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[cfg(feature = "math")]
fn prepared_math_native_glyph_geometry(svg: &str) -> Vec<bool> {
    let document = crate::xml_dom::Document::parse(svg).expect("well-formed native math SVG");
    document
        .descendants()
        .filter(|node| {
            (node.has_tag_name("g") || node.has_tag_name("use"))
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "merman-prepared-math-native")
                })
        })
        .map(|group| {
            let geometry = if group.has_tag_name("use") {
                let Some(target) = group
                    .attribute("href")
                    .and_then(|href| href.strip_prefix('#'))
                else {
                    return false;
                };
                let Some(definition) = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(target))
                else {
                    return false;
                };
                definition
            } else {
                group
            };
            geometry.descendants().any(|node| {
                node.has_tag_name("path")
                    && node
                        .attribute("d")
                        .is_some_and(|data| !data.trim().is_empty())
            })
        })
        .collect()
}

#[cfg(feature = "math")]
#[test]
fn native_math_geometry_observation_ignores_unrelated_paths() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
<path d="M0 0 L10 10"/>
<g class="merman-prepared-math-native"/>
<g class="merman-prepared-math-native"><path d=" "/></g>
</svg>"#;
    assert_eq!(prepared_math_native_glyph_geometry(svg), [false, false]);
}

#[cfg(feature = "math")]
#[test]
fn compiled_flowchart_math_projects_to_nonempty_resvg_geometry() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart TD\nA[\"$$x^2$$\"] --> B[Done]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare compiled Flowchart math")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render compiled Flowchart math");

    assert!(rendered.svg().contains("<foreignObject"));
    assert!(
        rendered
            .svg()
            .contains(r#"data-merman-prepared-math-native="v1""#)
    );
    assert!(!rendered.svg().contains("<template"));
    assert!(
        rendered
            .prepared_math_evidence
            .native_projection(0)
            .is_some()
    );
    assert!(!rendered.svg().contains("$$x^2$$"));

    let finalized = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("project compiled math before removing foreignObject");
    let svg = finalized.svg().as_str();

    assert!(!svg.contains("<foreignObject"), "{svg}");
    assert!(!svg.contains("<template"), "{svg}");
    assert!(!svg.contains("$$x^2$$"), "{svg}");
    assert!(
        svg.contains(r#"class="merman-prepared-math-native""#),
        "{svg}"
    );
    assert_eq!(prepared_math_native_glyph_geometry(svg), [true], "{svg}");
    assert_eq!(
        finalized
            .svg()
            .prepared_math_terminal_receipt()
            .expect("template-free math must retain its terminal receipt")
            .occurrence_count(),
        1
    );
}

#[cfg(feature = "math")]
#[test]
fn compiled_flowchart_math_survives_the_canonical_resvg_output_policy() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart TD\nA[\"$$x^2$$\"] --> B[Done]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare compiled Flowchart math")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render compiled Flowchart math");
    let pipeline = crate::svg::SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        ..crate::svg::SvgOutputPolicy::default()
    }
    .pipeline();

    let finalized = rendered
        .finalize_resvg(&pipeline)
        .expect("canonical ResvgSafe policy must retain renderer-owned math evidence");
    let svg = finalized.svg().as_str();

    assert!(!svg.contains("<foreignObject"), "{svg}");
    assert!(!svg.contains("$$x^2$$"), "{svg}");
    assert!(svg.contains(r#"class="merman-prepared-math-native""#));
    assert_eq!(
        finalized
            .svg()
            .prepared_math_terminal_receipt()
            .expect("canonical policy math receipt")
            .occurrence_count(),
        1
    );
}

#[cfg(feature = "math")]
#[test]
fn compiled_flowchart_math_survives_user_native_class_collisions() {
    for source in [
        "flowchart LR\nA[\"$$x$$\"]:::merman-prepared-math-native\n",
        "flowchart LR\nA[\"$$x$$\"] --> B[Done]:::merman-prepared-math-native\n",
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_compiled_math_renderer()
            .begin_session()
            .unwrap();
        let rendered = prepare(&parsed, &LayoutOptions::default(), session)
            .expect("prepare compiled Flowchart math with user classes")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render compiled Flowchart math with user classes");
        let pipeline = crate::svg::SvgOutputPolicy {
            preset: SvgPipelinePreset::ResvgSafe,
            ..crate::svg::SvgOutputPolicy::default()
        }
        .pipeline();

        let finalized = rendered
            .finalize_resvg(&pipeline)
            .expect("user classes must not interfere with canonical native math finalization");
        let svg = finalized.svg().as_str();
        let document = crate::xml_dom::Document::parse(svg).expect("well-formed native math SVG");
        let marked: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "merman-prepared-math-native")
                })
            })
            .collect();
        assert_eq!(marked.len(), 2, "the user class must survive: {source}");
        let instances: Vec<_> = marked
            .iter()
            .filter(|node| {
                node.attribute("data-merman-prepared-math-occurrence")
                    .is_some()
            })
            .collect();
        assert_eq!(instances.len(), 1, "only the formula is owned: {source}");
        let instance = instances[0];
        assert!(instance.has_tag_name("use"), "{svg}");
        assert_eq!(
            instance.attribute("data-merman-prepared-math-occurrence"),
            Some("flowchart/node-label/0"),
        );
        let target = instance
            .attribute("href")
            .and_then(|href| href.strip_prefix('#'))
            .expect("native formula must reference its definition");
        let definition = document
            .descendants()
            .find(|node| node.attribute("id") == Some(target))
            .expect("native formula definition must exist");
        assert!(definition.descendants().any(|node| {
            node.has_tag_name("path")
                && node
                    .attribute("d")
                    .is_some_and(|data| !data.trim().is_empty())
        }));
        assert_eq!(
            finalized
                .svg()
                .prepared_math_terminal_receipt()
                .expect("the formula must retain its terminal receipt")
                .occurrence_count(),
            1,
        );
        assert!(!svg.contains("<foreignObject"), "{svg}");
    }
}

#[cfg(feature = "math")]
#[test]
fn compiled_math_native_finalization_rejects_untrusted_evidence_mutation() {
    struct UnknownMathPostprocessor;

    impl crate::svg::SvgPostprocessor for UnknownMathPostprocessor {
        fn name(&self) -> &'static str {
            "unknown-math-postprocessor"
        }

        fn process<'a>(
            &self,
            svg: std::borrow::Cow<'a, str>,
            _ctx: &crate::svg::SvgPostprocessContext<'_>,
        ) -> crate::Result<std::borrow::Cow<'a, str>> {
            Ok(svg)
        }
    }

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart TD\nA[\"$$x^2$$\"] --> B[Done]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare compiled Flowchart math")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render compiled Flowchart math");
    let original = rendered.svg().to_owned();
    let work_meter = std::sync::Arc::clone(rendered.session.work_meter());
    assert!(work_meter.prepared_text_retained_bytes() > 0);
    assert!(rendered.prepared_math_evidence_valid);
    let rendered = rendered
        .apply_pipeline(&SvgPipeline::parity().with_postprocessor(UnknownMathPostprocessor))
        .expect("draft output may carry invalidated math evidence");
    assert_eq!(rendered.svg(), original);
    assert!(!rendered.prepared_math_evidence_valid);
    assert!(
        rendered
            .prepared_math_evidence
            .native_projection(0)
            .is_some()
    );

    let rendered = rendered
        .apply_pipeline(&SvgPipeline::parity())
        .expect("a later trusted identity pass may still publish draft SVG");
    assert!(!rendered.prepared_math_evidence_valid);
    assert!(
        rendered
            .prepared_math_evidence
            .native_projection(0)
            .is_some()
    );

    let error = match rendered.finalize_resvg(&SvgPipeline::resvg_safe()) {
        Ok(_) => panic!("native projection must not reuse invalidated prepared-math evidence"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("prepared-math evidence was invalidated"),
        "{error}"
    );
    assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn declining_math_backend_fails_closed_for_native_flowchart_and_sequence_outputs() {
    for (diagram_type, source) in [
        ("flowchart", "flowchart TD\nA[\"$$x$$\"] --> B[Done]\n"),
        (
            "sequence",
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: $$x$$\n",
        ),
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap_or_else(|| panic!("{diagram_type} source should produce a render model"))
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_math_renderer(std::sync::Arc::new(crate::math::NoopMathRenderer))
            .begin_session()
            .unwrap();
        let rendered = prepare(&parsed, &LayoutOptions::default(), session)
            .unwrap_or_else(|error| panic!("prepare {diagram_type} math: {error}"))
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render {diagram_type} math: {error}"));

        assert!(rendered.svg().contains("$$x$$"), "{}", rendered.svg());
        let error = match rendered.finalize_resvg(&SvgPipeline::resvg_safe()) {
            Ok(_) => panic!("{diagram_type} declined math unexpectedly finalized for native SVG"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("native-unavailable"),
            "{diagram_type}: {error}"
        );
    }
}

#[cfg(feature = "math")]
#[test]
fn compiled_flowchart_unmatched_math_delimiter_remains_plain_in_resvg() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart TD\nA[\"literal $$\"] --> B[Done]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Flowchart unmatched delimiter")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Flowchart unmatched delimiter");

    assert!(rendered.svg().contains("literal $$"));
    assert!(!rendered.svg().contains(r#"class="merman-prepared-math""#));

    let finalized = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("an unmatched delimiter is ordinary text, not missing native math");
    assert!(finalized.svg().as_str().contains("literal $$"));
    assert!(finalized.svg().prepared_math_terminal_receipt().is_none());
}

#[cfg(feature = "math")]
#[test]
fn compiled_sequence_math_projects_to_nonempty_resvg_geometry() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: $$x^2$$\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare compiled Sequence math")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render compiled Sequence math");

    assert_eq!(
        rendered
            .svg()
            .matches(r#"data-merman-prepared-math-native="v1""#)
            .count(),
        1
    );
    assert!(!rendered.svg().contains("$$x^2$$"));

    let finalized = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("project compiled Sequence math before removing foreignObject");
    let svg = finalized.svg().as_str();

    assert!(!svg.contains("<foreignObject"), "{svg}");
    assert!(!svg.contains("<template"), "{svg}");
    assert!(!svg.contains("$$x^2$$"), "{svg}");
    assert_eq!(
        svg.matches(r#"class="merman-prepared-math-native""#)
            .count(),
        1,
        "{svg}"
    );
    assert_eq!(prepared_math_native_glyph_geometry(svg), [true], "{svg}");
}

#[cfg(feature = "math")]
#[test]
fn compiled_sequence_unmatched_math_delimiter_remains_plain_in_resvg() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: literal $$\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence unmatched delimiter")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence unmatched delimiter");

    assert!(rendered.svg().contains("literal $$"));
    assert!(!rendered.svg().contains(r#"class="merman-prepared-math""#));

    let finalized = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("an unmatched delimiter is ordinary text, not missing native math");
    assert!(finalized.svg().as_str().contains("literal $$"));
    assert!(finalized.svg().prepared_math_terminal_receipt().is_none());
}

#[cfg(feature = "math")]
#[test]
fn compiled_sequence_mixed_math_with_unmatched_delimiter_stays_native_unavailable() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: valid $$x$$<br>literal $$\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence mixed math source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence mixed math")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence mixed math");

    assert!(
        rendered
            .svg()
            .contains(r#"data-merman-prepared-math-native="unavailable""#)
    );
    let error = match rendered.finalize_resvg(&SvgPipeline::resvg_safe()) {
        Ok(_) => panic!("mixed prose and math has no renderer-owned native projection yet"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("native projection is unavailable"),
        "{error}"
    );
}

#[cfg(feature = "math")]
#[test]
fn compiled_sequence_actor_man_math_seals_one_occurrence_across_mirrored_emissions() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nactor A as $$x$$\nparticipant B\nA->>B: hi\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence actor-man source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare compiled Sequence actor-man math")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render compiled Sequence actor-man math");

    assert_eq!(
        rendered
            .svg()
            .matches(r#"data-merman-prepared-math-native="v1""#)
            .count(),
        2,
        "the mirrored top and bottom actor-man labels must share one prepared occurrence"
    );
    let document =
        crate::xml_dom::Document::parse(rendered.svg()).expect("well-formed Sequence SVG");
    let fallback_labels = document
        .descendants()
        .filter(|node| node.is_text() && node.text() == Some("$$x$$"))
        .collect::<Vec<_>>();
    assert_eq!(fallback_labels.len(), 2);
    assert!(fallback_labels.iter().all(|label| {
        label
            .ancestors()
            .any(|ancestor| ancestor.has_tag_name("switch"))
    }));

    let finalized = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("project both actor-man math labels before removing foreignObject");
    let svg = finalized.svg().as_str();

    assert!(!svg.contains("<foreignObject"), "{svg}");
    assert!(!svg.contains("<template"), "{svg}");
    assert!(!svg.contains("$$x$$"), "{svg}");
    assert_eq!(
        svg.matches(r#"class="merman-prepared-math-native""#)
            .count(),
        2,
        "{svg}"
    );
    assert_eq!(
        finalized
            .svg()
            .prepared_math_terminal_receipt()
            .expect("actor-man terminal math receipt")
            .occurrence_count(),
        1,
        "two mirrored terminal emissions belong to one semantic actor occurrence"
    );
    assert_eq!(
        prepared_math_native_glyph_geometry(svg),
        [true, true],
        "{svg}"
    );
}

#[cfg(feature = "math")]
#[test]
fn compiled_sequence_multiline_participant_math_expands_each_terminal_actor_box() {
    let formula = "$$x$$<br>$$x$$<br>$$x$$<br>$$x$$<br>$$x$$<br>$$x$$";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            &format!("sequenceDiagram\nparticipant A as {formula}\nparticipant B\nA->>B: hi\n"),
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence multiline participant source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
        .begin_session()
        .unwrap();
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare multiline Sequence participant math")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render multiline Sequence participant math");
    let document =
        crate::xml_dom::Document::parse(rendered.svg()).expect("well-formed Sequence SVG");
    let parse_length = |node: crate::xml_dom::Node<'_, '_>, name: &str| {
        node.attribute(name)
            .expect("expected geometry attribute")
            .parse::<f64>()
            .expect("finite numeric geometry")
    };
    let actor_boxes = document
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "rect"
                && node.attribute("name") == Some("A")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "actor-top" || token == "actor-bottom")
                })
        })
        .map(|node| {
            let y = parse_length(node, "y");
            (y, y + parse_length(node, "height"))
        })
        .collect::<Vec<_>>();
    let math_boxes = document
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "foreignObject"
                && node.descendants().any(|descendant| {
                    descendant.attribute("data-merman-prepared-math-occurrence")
                        == Some("sequence/actor-label/0")
                })
        })
        .map(|node| {
            let y = parse_length(node, "y");
            (y, y + parse_length(node, "height"))
        })
        .collect::<Vec<_>>();

    assert_eq!(actor_boxes.len(), 2);
    assert_eq!(math_boxes.len(), 2);
    assert!(
        math_boxes.iter().all(|(math_top, math_bottom)| {
            actor_boxes.iter().any(|(actor_top, actor_bottom)| {
                *math_top >= *actor_top - 0.01 && *math_bottom <= *actor_bottom + 0.01
            })
        }),
        "prepared math boxes {math_boxes:?} must fit terminal actor boxes {actor_boxes:?}"
    );

    let finalized = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("project both expanded participant labels");
    assert!(!finalized.svg().as_str().contains("<switch"));
    assert!(!finalized.svg().as_str().contains("$$x$$"));
    assert_eq!(
        finalized
            .svg()
            .as_str()
            .matches(r#"class="merman-prepared-math-native""#)
            .count(),
        2
    );
    assert_eq!(
        finalized
            .svg()
            .prepared_math_terminal_receipt()
            .expect("multiline participant terminal receipt")
            .occurrence_count(),
        1
    );
}

#[cfg(feature = "math")]
#[test]
fn compiled_class_and_mindmap_math_fail_closed_until_their_native_projection_is_wired() {
    for (diagram_type, source) in [
        ("class", "classDiagram\nclass Formula[\"$$x^2$$\"]\n"),
        (
            "mindmap",
            "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  root[Root]\n    formula[\"$$x^2$$\"]\n",
        ),
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap_or_else(|| panic!("{diagram_type} source should produce a render model"))
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_compiled_math_renderer()
            .begin_session()
            .unwrap();
        let rendered = prepare(&parsed, &LayoutOptions::default(), session)
            .unwrap_or_else(|error| panic!("prepare {diagram_type} math: {error}"))
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render {diagram_type} math: {error}"));

        assert!(rendered.svg().contains("<path"), "{}", rendered.svg());
        assert!(
            rendered
                .svg()
                .contains(crate::math::BROWSER_ONLY_MATH_NATIVE_UNAVAILABLE_ATTRIBUTE),
            "{}",
            rendered.svg()
        );
        let error = match rendered.finalize_resvg(&SvgPipeline::resvg_safe()) {
            Err(error) => error,
            Ok(_) => panic!("{diagram_type} browser-only math unexpectedly finalized for resvg"),
        };
        assert!(
            error
                .to_string()
                .contains("browser-only math has no renderer-owned native projection"),
            "{diagram_type}: {error}"
        );
    }
}

#[test]
fn mindmap_math_label_requires_the_math_capability() {
    let source = r#"---
config:
  layout: tidy-tree
---
mindmap
  root[Root]
    formula["$$x^2$$"]
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("mindmap source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .without_math_renderer()
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert!(
        plan.required_capabilities()
            .contains(&RenderCapability::Math)
    );
    assert!(
        plan.missing_capabilities()
            .contains(&RenderCapability::Math)
    );
    assert!(!plan.is_ready());

    let error = match prepare(&parsed, &LayoutOptions::default(), session) {
        Err(error) => error,
        Ok(_) => panic!("mindmap math label unexpectedly rendered without a math backend"),
    };
    assert!(matches!(
        error,
        Error::MissingCapability {
            capability: RenderCapability::Math,
            ref diagram_type,
        } if diagram_type == "mindmap"
    ));
}

#[cfg(feature = "diagram-mindmap")]
#[test]
fn mindmap_svg_text_does_not_require_a_math_renderer() {
    let source = "---\nconfig:\n  layout: tidy-tree\n  htmlLabels: false\n---\nmindmap\nRoot\n  formula[\"$$x^2$$\"]\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .without_math_renderer()
        .begin_session()
        .unwrap();
    let plan = plan_render(&parsed, &session).unwrap();
    assert!(plan.is_ready());
    assert!(
        !plan
            .required_capabilities()
            .contains(&RenderCapability::Math)
    );
    let artifact = prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(svg.svg().contains("$$x^2$$"));
    assert!(!svg.svg().contains("foreignObject"));
}

#[derive(Debug)]
struct MindmapMathRenderer;

impl crate::math::MathRenderer for MindmapMathRenderer {
    fn render_html_label(
        &self,
        text: &str,
        _config: &merman_model::presentation::PresentationConfig,
    ) -> Option<String> {
        text.contains("$$")
            .then(|| "<strong>rendered-mindmap-math</strong>".to_string())
    }

    fn measure_html_label(
        &self,
        text: &str,
        _config: &merman_model::presentation::PresentationConfig,
        _style: &crate::text::TextStyle,
        _max_width_px: Option<f64>,
        _wrap_mode: crate::text::WrapMode,
    ) -> Option<crate::text::TextMetrics> {
        text.contains("$$").then_some(crate::text::TextMetrics {
            width: 96.0,
            height: 24.0,
            line_count: 1,
        })
    }
}

#[test]
fn mindmap_math_label_is_consumed_by_the_math_renderer() {
    let source = r#"---
config:
  layout: tidy-tree
---
mindmap
  root[Root]
    formula["$$x^2$$"]
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("mindmap source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(MindmapMathRenderer))
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert!(
        plan.required_capabilities()
            .contains(&RenderCapability::Math)
    );
    assert!(plan.missing_capabilities().is_empty());
    assert!(plan.is_ready());

    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(rendered.svg().contains("rendered-mindmap-math"));
    assert!(!rendered.svg().contains("$$x^2$$"));
}

#[test]
fn class_math_label_requires_the_math_capability() {
    let source = r#"classDiagram
class Formula["$$x^2$$"]
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Class source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .without_math_renderer()
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert!(
        plan.required_capabilities()
            .contains(&RenderCapability::Math)
    );
    assert!(
        plan.missing_capabilities()
            .contains(&RenderCapability::Math)
    );
    assert!(!plan.is_ready());

    let error = match prepare(&parsed, &LayoutOptions::default(), session) {
        Err(error) => error,
        Ok(_) => panic!("Class math label unexpectedly rendered without a math backend"),
    };
    let expected = if cfg!(any(feature = "layout-dagre", feature = "layout-elk")) {
        RenderCapability::Math
    } else {
        RenderCapability::LayoutElk
    };
    assert!(matches!(
        error,
        Error::MissingCapability {
            capability,
            ref diagram_type,
        } if capability == expected && diagram_type == "classDiagram"
    ));
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[derive(Debug)]
struct ClassMathRenderer;

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
impl crate::math::MathRenderer for ClassMathRenderer {
    fn render_html_label(
        &self,
        text: &str,
        _config: &merman_model::presentation::PresentationConfig,
    ) -> Option<String> {
        text.contains("$$")
            .then(|| "<div>rendered-class-math</div>".to_string())
    }

    fn measure_html_label(
        &self,
        text: &str,
        _config: &merman_model::presentation::PresentationConfig,
        _style: &crate::text::TextStyle,
        _max_width_px: Option<f64>,
        _wrap_mode: crate::text::WrapMode,
    ) -> Option<crate::text::TextMetrics> {
        text.contains("$$").then_some(crate::text::TextMetrics {
            width: 96.0,
            height: 24.0,
            line_count: 1,
        })
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn class_math_label_is_consumed_by_the_math_renderer() {
    let source = r#"classDiagram
class Formula["$$x^2$$"]
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Class source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(ClassMathRenderer))
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert!(
        plan.required_capabilities()
            .contains(&RenderCapability::Math)
    );
    assert!(plan.missing_capabilities().is_empty());
    assert!(plan.is_ready());

    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(rendered.svg().contains("rendered-class-math"));
    assert!(!rendered.svg().contains("$$x^2$$"));
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn render_class_math(source: &str) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Class source should produce a render model")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(ClassMathRenderer))
        .begin_session()
        .unwrap();
    prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_string()
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn class_math_label_forces_html_rendering_when_html_labels_are_disabled() {
    let svg = render_class_math(
        r#"---
config:
  htmlLabels: false
---
classDiagram
class Formula["$$x^2$$"]
"#,
    );

    assert!(svg.contains("rendered-class-math"));
    assert!(!svg.contains("$$x^2$$"));
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn class_relation_terminal_and_note_math_labels_use_the_math_renderer() {
    let svg = render_class_math(
        r#"classDiagram
class Formula
class Result
Formula "$$one$$" --> "$$many$$" Result : $$edge$$
note for Formula "$$note$$"
"#,
    );

    assert_eq!(svg.matches("rendered-class-math").count(), 4);
    assert!(!svg.contains("$$"));
    assert!(!svg.contains("<p><div>"));
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn class_annotation_and_interface_math_labels_require_and_use_math() {
    for source in [
        r#"classDiagram
class Formula <<$$annotation$$>>
"#,
        r#"classDiagram
class Formula
$$interface$$ ()-- Formula
"#,
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model")
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert!(
            plan.required_capabilities()
                .contains(&RenderCapability::Math)
        );
        assert!(
            plan.missing_capabilities()
                .contains(&RenderCapability::Math)
        );

        let svg = render_class_math(source);
        assert!(svg.contains("rendered-class-math"));
        assert!(!svg.contains("$$"));
        assert!(!svg.contains("<p><div>"));
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn elk_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
    let source = "flowchart-elk TD\nA --> B";
    let (fixture_parsed_10, fixture_layout_10, fixture_session_10) =
        prepare_with_model_item_limit(source, 3);
    let artifact = prepare(&fixture_parsed_10, &fixture_layout_10, fixture_session_10).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::FLOWCHART);

    let (fixture_parsed_11, fixture_layout_11, fixture_session_11) =
        prepare_with_model_item_limit(source, 2);
    let error = match prepare(&fixture_parsed_11, &fixture_layout_11, fixture_session_11) {
        Err(error) => error,
        Ok(_) => panic!("ELK flowchart above the node limit unexpectedly rendered"),
    };
    assert_model_item_limit(error, 3, 2);
}

#[test]
fn custom_semantic_json_is_explicitly_non_renderable() {
    let mut engine = MermaidEngine::new();
    engine
        .diagram_registry_mut()
        .insert("customDiagram", custom_semantic_parser);
    let parsed = engine
        .parse_diagram_for_render_model_with_type_sync(
            "customDiagram",
            "customDiagram\npayload",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();

    let error = match prepare(&parsed, &LayoutOptions::default(), session()) {
        Err(error) => error,
        Ok(_) => panic!("custom JSON unexpectedly produced a built-in artifact"),
    };
    let Error::NonRenderableCustomModel {
        diagram_type,
        model_name,
        provenance,
    } = error
    else {
        panic!("expected explicit custom-model capability error")
    };
    assert_eq!(diagram_type, "customDiagram");
    assert_eq!(model_name, "customDiagram");
    assert_eq!(provenance, CustomJsonProvenance::SemanticRegistryOverlay);
}

#[test]
fn custom_render_overlay_cannot_masquerade_as_a_builtin_family() {
    let mut engine = MermaidEngine::new();
    engine
        .render_diagram_registry_mut()
        .insert("flowchart-v2", custom_render_parser);
    let parsed = engine
        .parse_diagram_for_render_model_with_type_sync(
            "flowchart-v2",
            "flowchart TD\nA --> B",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();

    let error = match prepare(&parsed, &LayoutOptions::default(), session()) {
        Err(error) => error,
        Ok(_) => panic!("custom JSON unexpectedly produced a built-in artifact"),
    };
    let Error::NonRenderableCustomModel {
        diagram_type,
        model_name,
        provenance,
    } = error
    else {
        panic!("expected explicit custom-model capability error")
    };
    assert_eq!(diagram_type, "flowchart-v2");
    assert_eq!(model_name, "custom-flowchart");
    assert_eq!(provenance, CustomJsonProvenance::RenderRegistryOverlay);
}

#[test]
fn gantt_time_axis_diagnostics_invert_rendered_x_without_exposing_layout() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  gantt:
    useWidth: 130
    leftPadding: 10
    rightPadding: 20
---
gantt
dateFormat x
section Delivery
First: first,-1,1ms
Second: second,after first,2ms
"#,
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();

    assert_eq!(artifact.family_id(), DiagramFamilyId::GANTT);
    let diagnostics = artifact
        .gantt_time_axis_diagnostics()
        .expect("Gantt tasks should expose time-axis diagnostics");
    assert_eq!(diagnostics.unix_millis_at_rendered_x(10.0), Some(-1));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(43.0), Some(0));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(110.0), Some(2));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(44.0), None);
    assert_eq!(diagnostics.unix_millis_at_rendered_x(f64::NAN), None);

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
}

#[test]
fn suppressed_parse_failure_uses_the_typed_error_artifact_and_renderer() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA -->", ParseOptions::lenient())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();

    assert_eq!(artifact.family_id(), DiagramFamilyId::ERROR);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(rendered.svg().contains("Syntax error in text"));
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn render_with_sidecar_host_control(
    preparation: FamilyPreparation,
    outcome: SidecarHostOutcome,
) -> (
    bool,
    Vec<SidecarHostRequest>,
    crate::environment::TextMeasurementReport,
    String,
) {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
subgraph S["service words"]
  A["alpha beta<br/>gamma"]
end
A traced-edge@-->|edge label words| B["delta epsilon"]
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("detect flowchart")
        .into_presentation();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.flowchart-sidecar-control").expect("profile id"),
        "1",
    )
    .expect("profile identity");
    let host = Arc::new(SidecarRecordingHost::new(outcome));
    let environment =
        crate::environment::RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::host_display(identity, host.clone(), TextMeasurementPhase::ALL),
        );
    let session = environment.begin_session().expect("render session");
    let artifact = prepare_for(&parsed, &LayoutOptions::default(), session, preparation)
        .expect("prepare flowchart artifact");
    let indexed_sidecar = match &artifact.family {
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        BuiltinFamilyArtifact::Flowchart(flowchart) => {
            flowchart
                .svg_label_sidecar()
                .node_owner("A", false)
                .is_some()
                && flowchart
                    .svg_label_sidecar()
                    .edge_owner("traced-edge", false)
                    .is_some()
        }
        _ => panic!("expected Flowchart family artifact"),
    };
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Flowchart SVG");
    let trace = host.snapshot();
    let report = rendered.session.text_measurement_report();
    let svg = rendered.svg().to_owned();
    (indexed_sidecar, trace, report, svg)
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn routed_host_and_fallback_traces_match_a_no_sidecar_family_control() {
    for outcome in [SidecarHostOutcome::Success, SidecarHostOutcome::Missing] {
        let (control_indexed, control_trace, control_report, control_svg) =
            render_with_sidecar_host_control(
                FamilyPreparation::RenderWithoutFlowchartSvgLabels,
                outcome,
            );
        let (sidecar_indexed, sidecar_trace, sidecar_report, sidecar_svg) =
            render_with_sidecar_host_control(FamilyPreparation::Render, outcome);

        assert!(
            !control_indexed,
            "the control must bypass sidecar preparation"
        );
        assert!(
            sidecar_indexed,
            "the candidate must prepare semantic owners"
        );
        assert!(!control_trace.is_empty());
        assert_eq!(sidecar_trace, control_trace);
        assert_eq!(sidecar_report, control_report);
        assert_eq!(sidecar_svg, control_svg);
    }
}

fn project_agentflow_for_test(
    config: &merman_model::presentation::PresentationConfig,
    control: &merman_model::OperationControl,
) -> Result<merman_model::presentation::PresentationConfig> {
    let backend = crate::layout_backend::resolve(DiagramFamilyId::AGENTFLOW, "agentflow", config)?;
    project_agentflow_flowchart_config(config, backend, control)
}

#[test]
fn agentflow_layout_config_prioritizes_family_measurement_keys() {
    let config = crate::test_support::presentation_config(json!({
        "agentflow": { "minNodeWidth": 180, "wrappingWidth": 240 },
        "flowchart": { "minNodeWidth": 90 }
    }));
    let projected =
        project_agentflow_for_test(&config, &merman_model::OperationControl::new()).unwrap();
    let number = |config: &merman_model::presentation::PresentationConfig, path: &str| {
        config
            .as_value()
            .pointer(path)
            .and_then(serde_json::Value::as_f64)
    };
    assert_eq!(number(&projected, "/flowchart/minNodeWidth"), Some(180.0));
    assert_eq!(number(&projected, "/flowchart/wrappingWidth"), Some(240.0));
    assert_eq!(number(&config, "/flowchart/wrappingWidth"), None);
    assert_eq!(number(&config, "/agentflow/minNodeWidth"), Some(180.0));
}

#[test]
fn agentflow_projection_clones_deep_routing_values_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            use merman_model::presentation::FrozenPresentationConfigParts;
            use merman_model::resources::{InputResourcePolicy, ResourceProfile};
            use merman_model::{MermaidConfig, OperationControl};

            let mut value = Value::String("leaf".to_string());
            for _ in 0..2048 {
                value = Value::Array(vec![value]);
            }
            let spacing = merman_model::json::clone_value_nonrecursive(&value);
            let mut values = MermaidConfig::empty_object();
            values.set_value("agentflow.diagramPadding", value);
            values.set_value("nodeSpacing", spacing);
            let control = OperationControl::new();
            let config = PresentationConfig::try_from_frozen_parts_controlled(
                FrozenPresentationConfigParts::new(values),
                &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
                &control,
            )
            .unwrap()
            .unwrap();
            let projected = project_agentflow_for_test(&config, &control).unwrap();
            for key in ["diagramPadding", "nodeSpacing"] {
                let mut value = &projected.as_value()["flowchart"][key];
                for _ in 0..2048 {
                    value = &value.as_array().expect("routing value retains its depth")[0];
                }
                assert_eq!(value.as_str(), Some("leaf"));
            }
        })
        .expect("spawn the small-stack projection fixture")
        .join()
        .expect("deep routing values clone and drop iteratively");
}

#[test]
fn agentflow_layout_config_keeps_flowchart_wrapping_fallback() {
    for wrapping in [json!(0), json!(null), json!(false), json!("")] {
        let config = crate::test_support::presentation_config(json!({
            "agentflow": { "minNodeWidth": 0, "wrappingWidth": wrapping },
            "flowchart": { "minNodeWidth": 90, "wrappingWidth": 240 }
        }));
        let projected =
            project_agentflow_for_test(&config, &merman_model::OperationControl::new()).unwrap();
        assert_eq!(projected.as_value()["flowchart"]["minNodeWidth"], 0);
        assert_eq!(projected.as_value()["flowchart"]["wrappingWidth"], 240);
    }
}

#[test]
fn agentflow_projection_separates_renderer_ownership_from_shared_shape_settings() {
    for family in [json!({}), json!(null)] {
        let config = crate::test_support::presentation_config(json!({
            "agentflow": family,
            "flowchart": {
                "diagramPadding": 99, "titleTopMargin": 99, "useMaxWidth": false,
                "nodeSpacing": 99, "rankSpacing": 99,
                "padding": 17, "curve": "linear", "htmlLabels": false,
                "inheritDir": true, "subGraphTitleMargin": {"top": 7}
            }
        }));
        let projected =
            project_agentflow_for_test(&config, &merman_model::OperationControl::new()).unwrap();
        let flow = &projected.as_value()["flowchart"];
        assert_eq!(flow["diagramPadding"], 8);
        assert_eq!(flow["titleTopMargin"], 0);
        assert_eq!(flow["useMaxWidth"], true);
        assert_eq!(flow["nodeSpacing"], 50);
        assert_eq!(flow["rankSpacing"], 50);
        for key in [
            "padding",
            "curve",
            "htmlLabels",
            "inheritDir",
            "subGraphTitleMargin",
        ] {
            assert_eq!(flow[key], config.as_value()["flowchart"][key]);
        }
    }
    for value in [json!(null), json!(0)] {
        let config = crate::test_support::presentation_config(json!({
            "nodeSpacing": 90, "rankSpacing": 0,
            "agentflow": {
                "diagramPadding": value, "titleTopMargin": value, "useMaxWidth": false,
                "nodeSpacing": 200, "rankSpacing": value
            }
        }));
        let projected =
            project_agentflow_for_test(&config, &merman_model::OperationControl::new()).unwrap();
        let flow = &projected.as_value()["flowchart"];
        assert_eq!(flow["diagramPadding"], if value.is_null() { 8 } else { 0 });
        assert_eq!(flow["titleTopMargin"], 0);
        assert_eq!(flow["useMaxWidth"], false);
        assert_eq!(flow["nodeSpacing"], 90);
        assert_eq!(flow["rankSpacing"], 50);
    }
}

#[test]
fn agentflow_spacing_projection_follows_registered_backend_selection() {
    for layout in [
        "dagre",
        "unknown",
        "elk.layered",
        "elk",
        "elk.stress",
        "elk.force",
        "elk.mrtree",
        "elk.sporeOverlap",
        "elk.box",
        "elk.rectpacking",
    ] {
        let config = crate::test_support::presentation_config(json!({
            "layout": layout,
            "nodeSpacing": 90, "rankSpacing": 110,
            "agentflow": {"nodeSpacing": 200, "rankSpacing": 210},
            "flowchart": {"nodeSpacing": 300, "rankSpacing": 310}
        }));
        let selected =
            crate::layout_backend::resolve(DiagramFamilyId::AGENTFLOW, "agentflow", &config)
                .unwrap();
        let projected = project_agentflow_flowchart_config(
            &config,
            selected,
            &merman_model::OperationControl::new(),
        )
        .unwrap();
        let flow = &projected.as_value()["flowchart"];
        let elk_selected = (crate::layout_elk_available() || !crate::layout_dagre_available())
            && crate::layout_backend::ElkRootAlgorithm::from_name(layout).is_some();
        assert_eq!(
            matches!(selected, GraphLayoutBackend::Elk(_)),
            elk_selected,
            "{layout}"
        );
        assert_eq!(
            flow["nodeSpacing"],
            if elk_selected { 50 } else { 90 },
            "{layout}"
        );
        assert_eq!(
            flow["rankSpacing"],
            if elk_selected { 50 } else { 110 },
            "{layout}"
        );
    }
}

struct CancellingSuccessfulHost {
    calls: std::sync::atomic::AtomicUsize,
    control: merman_model::OperationControl,
}

impl HostTextMeasurer for CancellingSuccessfulHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.control.cancel();
        Ok(Some(sidecar_host_measurement(request, 0)))
    }
}

#[test]
fn family_layout_stops_host_measurement_after_callback_cancellation() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    First child\n    Second child\n",
            ParseOptions::strict(),
        )
        .expect("parse mindmap")
        .expect("detect mindmap").into_presentation();
    let control = merman_model::OperationControl::new();
    let host = Arc::new(CancellingSuccessfulHost {
        calls: std::sync::atomic::AtomicUsize::new(0),
        control: control.clone(),
    });
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.family-cancelling-host").expect("profile id"),
        "1",
    )
    .expect("profile identity");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            host.clone(),
            TextMeasurementPhase::ALL,
        ))
        .begin_session_with_control(control)
        .expect("begin render session");

    let result = prepare(&parsed, &LayoutOptions::default(), session);
    let Err(Error::Cancelled(cancelled)) = result else {
        panic!("family preparation must surface callback cancellation");
    };
    assert_eq!(cancelled.phase, OperationPhase::Layout);
    assert_eq!(host.calls.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
fn final_svg_admission_prefers_emit_cancellation_to_byte_limit() {
    let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 1)
        .unwrap();
    let control = merman_model::OperationControl::new();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session_with_control(control.clone())
        .unwrap();
    let svg = "<svg/>";

    let limit = session
        .resource_policy()
        .check_svg_bytes(svg, ResourceLimitPhase::SvgOutput)
        .unwrap_err();
    assert_eq!(limit.limit, "max_svg_bytes");

    control.cancel();
    let error = admit_rendered_svg_output(&session, svg).unwrap_err();
    let Error::Cancelled(error) = error else {
        panic!("expected final SVG admission cancellation");
    };
    assert_eq!(error.phase, OperationPhase::Emit);
}

#[test]
fn final_svg_resource_terminal_replays_before_later_cancellation() {
    let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 1)
        .unwrap();
    let control = merman_model::OperationControl::new();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session_with_control(control.clone())
        .unwrap();
    let svg = "<svg/>";

    let first = admit_rendered_svg_output(&session, svg)
        .expect_err("the formal SVG output admission must reject");
    let Error::ResourceLimitExceeded(first_limit) = first else {
        panic!("expected a resource rejection");
    };
    assert_eq!(first_limit.limit, "max_svg_bytes");
    assert_eq!(first_limit.actual, svg.len());
    assert_eq!(first_limit.max, 1);

    control.cancel();
    let replayed = admit_rendered_svg_output(&session, svg)
        .expect_err("the first SVG output terminal must remain sticky");
    let Error::ResourceLimitExceeded(replayed_limit) = replayed else {
        panic!("expected the resource terminal to replay");
    };
    assert_eq!(replayed_limit, first_limit);
}

#[test]
fn requested_diagram_id_fanout_is_admitted_per_output_occurrence() {
    let diagram_id = "d".repeat(256);
    let maximum = diagram_id.len() * 4;
    let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, maximum)
        .unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
        .expect("parse info diagram")
        .expect("detect info diagram")
        .into_presentation();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("begin render session");
    let artifact =
        prepare(&parsed, &LayoutOptions::default(), session).expect("prepare info diagram");
    let error = match artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.clone()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    ) {
        Ok(_) => panic!("the diagram ID fanout must exceed the SVG byte ceiling"),
        Err(error) => error,
    };

    let Error::ResourceLimitExceeded(details) = error else {
        panic!("expected the family fanout preflight to reject");
    };
    assert_eq!(details.limit, "max_svg_bytes");
    assert_eq!(details.max, maximum);
    assert!(details.actual > maximum);
    assert_eq!(
        details.phase,
        crate::resources::ResourceLimitPhase::SvgOutput
    );
}

#[test]
fn renderer_owned_metadata_scan_observes_the_artifact_session_control() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
        .expect("parse info diagram")
        .expect("detect info diagram")
        .into_presentation();
    let control = merman_model::OperationControl::new();
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .expect("begin render session");
    let rendered = prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare info diagram")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render info diagram");

    control.cancel();
    let error = rendered
        .output_metadata()
        .expect_err("metadata extraction must observe the artifact session control");

    let Error::Cancelled(cancelled) = error else {
        panic!("expected structured postprocess cancellation");
    };
    assert_eq!(cancelled.phase, OperationPhase::Postprocess);
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn edge_geometry_json_label_positions_match_emitted_positions() {
    let cases = [
        (
            "issue_176_three_parallel_unequal_labels",
            "flowchart TD\n    A -->|one| B\n    A -->|two| B\n    A -->|three| B\n",
            3,
        ),
        (
            "two_parallel_unequal_labels",
            "flowchart TD\n    A -->|short| B\n    A -->|a substantially wider edge label| B\n",
            2,
        ),
        (
            "three_parallel_equal_labels",
            "flowchart TD\n    A -->|same| B\n    A -->|same| B\n    A -->|same| B\n",
            3,
        ),
    ];

    for (case, source, expected_edges) in cases {
        let canonical_77 = parse_flowchart(source);
        let geometry = prepare(&canonical_77, &LayoutOptions::default(), session())
            .expect("prepare flowchart")
            .edge_geometry_json()
            .expect("edge geometry json");
        let canonical_78 = parse_flowchart(source);
        let svg = prepare(&canonical_78, &LayoutOptions::default(), session())
            .expect("prepare flowchart")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render flowchart")
            .svg()
            .to_owned();
        let rendered = decode_edge_label_positions(&svg);
        let positions: Vec<_> = geometry
            .iter()
            .map(|edge| {
                let actual = edge.label_position.as_ref().unwrap_or_else(|| {
                    panic!("{case}: edge {} must have a label position", edge.id)
                });
                let (rendered_x, rendered_y) = rendered
                    .get(&edge.id)
                    .unwrap_or_else(|| panic!("{case}: SVG must contain label {}", edge.id));
                (
                    edge.id.as_str(),
                    actual.x,
                    actual.y,
                    *rendered_x,
                    *rendered_y,
                )
            })
            .collect();

        assert_eq!(positions.len(), expected_edges, "{case}: edge count");
        assert_eq!(
            rendered.len(),
            expected_edges,
            "{case}: rendered label count"
        );

        // SVG labels are emitted in the root's translated coordinate space, while this API
        // intentionally reports layout coordinates. Their pairwise deltas must therefore
        // match exactly even when the whole diagram has a non-zero translation.
        for left in 0..positions.len() {
            for right in left + 1..positions.len() {
                let (left_id, lx, ly, lrx, lry) = positions[left];
                let (right_id, rx, ry, rrx, rry) = positions[right];
                assert!(
                    ((rx - lx) - (rrx - lrx)).abs() < 1e-5,
                    "{case}: x delta for {left_id} -> {right_id} differs between geometry and SVG"
                );
                assert!(
                    ((ry - ly) - (rry - lry)).abs() < 1e-5,
                    "{case}: y delta for {left_id} -> {right_id} differs between geometry and SVG"
                );
            }
        }
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn preparation_borrows_the_canonical_models_metadata_and_flowchart_context() {
    for source in [
        "flowchart LR\nA --> B",
        "stateDiagram-v2\n[*] --> Active",
        "block-beta\nA",
        "treemap-beta\n\"Root\"\n    \"Leaf\": 1",
        "ishikawa-beta\nEffect\n  Cause",
        "agentflow-beta\nA --> B",
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        for _ in 0..2 {
            let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();
            assert!(std::ptr::eq(artifact.metadata(), parsed.metadata()));
            match (&artifact.family, parsed.model()) {
                (BuiltinFamilyArtifact::Flowchart(flow), RenderSemanticModel::Flowchart(model)) => {
                    assert!(std::ptr::eq(flow.pair().semantic(), model));
                    assert!(std::ptr::eq(
                        flow.render_context(),
                        parsed.render_context().flowchart_render_context().unwrap()
                    ));
                }
                (BuiltinFamilyArtifact::State(value), RenderSemanticModel::State(model)) => {
                    assert!(std::ptr::eq(value.pair().semantic(), model))
                }
                (BuiltinFamilyArtifact::Block(value), RenderSemanticModel::Block(model)) => {
                    assert!(std::ptr::eq(value.pair().semantic(), model))
                }
                (BuiltinFamilyArtifact::Treemap(value), RenderSemanticModel::Treemap(model)) => {
                    assert!(std::ptr::eq(value.pair().semantic(), model))
                }
                (BuiltinFamilyArtifact::Ishikawa(value), RenderSemanticModel::Ishikawa(model)) => {
                    assert!(std::ptr::eq(value.pair().semantic(), model))
                }
                (
                    BuiltinFamilyArtifact::Agentflow { semantic, flow, .. },
                    RenderSemanticModel::Agentflow(model),
                ) => {
                    assert!(std::ptr::eq(*semantic, model));
                    assert!(matches!(flow.pair.semantic, std::borrow::Cow::Owned(_)));
                }
                _ => panic!("unexpected family for {source}"),
            }
        }
    }
}

#[test]
fn edge_geometry_json_family_cases_match_actual_svg_coordinates() {
    let cases = [
        #[cfg(feature = "layout-dagre")]
        (
            "dagre_nested",
            "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n---\nflowchart LR\nsubgraph Outer\n direction TB\n A((Circle)) -->|inner| B{Diamond}\nend\nB -->|outer| C[End]",
        ),
        #[cfg(any(feature = "layout-elk", feature = "layout-dagre"))]
        (
            "elk_parallel",
            "---\nconfig:\n  layout: elk\n  htmlLabels: false\n---\nflowchart LR\nA((Circle)) -->|one| B{Diamond}\nA -->|two| B\nB --> C[End]",
        ),
        #[cfg(feature = "layout-dagre")]
        (
            "hand_drawn",
            "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n  look: handDrawn\n  handDrawnSeed: 7\n---\nflowchart LR\nA((Circle)) -->|one| B[End]\nB -->|back| A",
        ),
        #[cfg(feature = "layout-dagre")]
        (
            "fallback_rough_nested",
            "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n  look: handDrawn\n  handDrawnSeed: -1\n---\nflowchart LR\nsubgraph Group\n direction TB\n A((Circle)) -->|inner| B[End]\nend\nX -->|outer| Y\nY --> Group",
        ),
        #[cfg(feature = "layout-dagre")]
        (
            "fallback_uint32_zero_nested",
            "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n  look: handDrawn\n  handDrawnSeed: 4294967296\n---\nflowchart LR\nsubgraph Group\n direction TB\n A((Circle)) -->|inner| B[End]\nend\nX -->|outer| Y\nY --> Group",
        ),
        #[cfg(feature = "layout-shoal")]
        (
            "shoal_nested",
            "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n---\nflowchart LR\nsubgraph Outer\n A((Circle)) -->|inner| B{Diamond}\nend\nB -->|outer| C[End]",
        ),
        (
            "swimlane_nested",
            "---\nconfig:\n  htmlLabels: false\n---\nswimlane-beta LR\nsubgraph Outer\n subgraph Inner\n A((Circle)) -->|inner| B{Diamond}\n end\nend\nB -->|outer| C[End]",
        ),
        #[cfg(any(feature = "layout-elk", feature = "layout-dagre"))]
        (
            "empty_labels",
            "---\nconfig:\n  htmlLabels: false\n  layout: elk\n---\nflowchart LR\nA -->| | B\nB --> C",
        ),
        #[cfg(any(feature = "layout-elk", feature = "layout-dagre"))]
        ("agentflow", "agentflow-beta\nA -->|label| B"),
    ];
    fn transform(node: crate::xml_dom::Node<'_, '_>) -> (f64, f64) {
        node.ancestors()
            .filter_map(|ancestor| {
                ancestor
                    .attribute("transform")
                    .and_then(|value| value.strip_prefix("translate("))
                    .and_then(|value| value.strip_suffix(')'))
                    .and_then(|value| value.split_once(','))
                    .and_then(|(x, y)| {
                        Some((x.trim().parse::<f64>().ok()?, y.trim().parse::<f64>().ok()?))
                    })
            })
            .fold((0.0, 0.0), |sum, point| (sum.0 + point.0, sum.1 + point.1))
    }
    fn root<'a, 'input: 'a>(
        node: crate::xml_dom::Node<'a, 'input>,
    ) -> Option<crate::xml_dom::Node<'a, 'input>> {
        node.ancestors()
            .find(|ancestor| ancestor.attribute("class") == Some("root"))
    }
    fn layout_offset(node: crate::xml_dom::Node<'_, '_>, nodes: &[Value]) -> (f64, f64) {
        let Some(root_node) = root(node) else {
            return (0.0, 0.0);
        };
        // Recursive roots include a Mermaid wrapper translation beyond layout coordinates.
        // Recover that root-local translation from an actual emitted node and public layout.
        for candidate in root_node.descendants().filter(|candidate| {
            candidate.attribute("data-et") == Some("node") && root(*candidate) == Some(root_node)
        }) {
            let Some(layout) = nodes
                .iter()
                .find(|layout| layout["id"].as_str() == candidate.attribute("data-id"))
            else {
                continue;
            };
            let actual = transform(candidate);
            return (
                layout["x"].as_f64().unwrap() - actual.0,
                layout["y"].as_f64().unwrap() - actual.1,
            );
        }
        (0.0, 0.0)
    }
    for (case, source) in cases {
        let parsed = parse_flowchart(source);
        let artifact = prepare(&parsed, &LayoutOptions::default(), session()).unwrap();
        let layout = artifact.layout_json().unwrap();
        let nodes = layout["layout"]
            .as_object()
            .unwrap()
            .values()
            .find_map(|family| family.get("nodes").and_then(Value::as_array))
            .expect("flowchart layout nodes");
        let geometry = artifact.edge_geometry_json().unwrap();
        let svg = prepare(&parsed, &LayoutOptions::default(), session())
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = crate::xml_dom::Document::parse(svg.svg()).unwrap();
        let paths: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-points").is_some())
            .collect();
        let points = decode_data_points(svg.svg());
        for edge in geometry.iter().filter(|edge| edge.points.is_some()) {
            let (index, (_, points)) = points
                .iter()
                .enumerate()
                .find(|(_, (id, _))| id == &edge.id)
                .expect("actual SVG path");
            let offset = layout_offset(paths[index], nodes);
            let origin = transform(paths[index]);
            let actual = edge.points.as_ref().unwrap();
            assert_eq!(actual.len(), points.len(), "{case}");
            for (actual, expected) in actual.iter().zip(points.iter()) {
                assert!(
                    (actual.x - expected.0 - origin.0 - offset.0).abs() < 1e-5
                        && (actual.y - expected.1 - origin.1 - offset.1).abs() < 1e-5,
                    "{case}: {} geometry({}, {}) SVG({}, {})",
                    edge.id,
                    actual.x,
                    actual.y,
                    expected.0 + origin.0 + offset.0,
                    expected.1 + origin.1 + offset.1
                );
            }
        }
        let mut labels = std::collections::HashMap::new();
        for node in document.descendants().filter(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "edgeLabel")
                })
        }) {
            let Some(label) = node
                .descendants()
                .find(|child| child.attribute("data-id").is_some())
            else {
                continue;
            };
            let mut position = transform(node);
            let offset = layout_offset(node, nodes);
            position.0 += offset.0;
            position.1 += offset.1;
            let has_position = node.attribute("transform").is_some();
            if has_position {
                labels.insert(label.attribute("data-id").unwrap().to_owned(), position);
            }
        }
        for edge in geometry {
            if let Some((x, y)) = labels.get(&edge.id) {
                let actual = edge
                    .label_position
                    .unwrap_or_else(|| panic!("{case}: missing {}", edge.id));
                assert!(
                    (actual.x - x).abs() < 1e-5 && (actual.y - y).abs() < 1e-5,
                    "{case}: {} actual({}, {}) SVG({x}, {y})",
                    edge.id,
                    actual.x,
                    actual.y
                );
            } else {
                assert!(
                    edge.label_position.is_none(),
                    "{case}: {} has no positioned SVG label",
                    edge.id
                );
            }
        }
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn edge_geometry_rejects_portability_after_observing_the_session_terminal() {
    let parsed = parse_flowchart("flowchart LR\nA --> B");
    for cancelled in [false, true] {
        let control = merman_model::OperationControl::new();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_control(control.clone())
            .unwrap();
        let artifact = prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        if cancelled {
            control.cancel();
        }
        let result = artifact.edge_geometry_json();
        if cancelled {
            assert!(matches!(result, Err(Error::Cancelled(_))), "{result:?}");
        } else {
            assert!(
                matches!(
                    result,
                    Err(Error::PortabilityUnavailableForTarget {
                        target: "edge-geometry-json"
                    })
                ),
                "{result:?}"
            );
        }
    }
}
