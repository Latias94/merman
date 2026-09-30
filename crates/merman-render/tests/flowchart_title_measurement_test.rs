#![cfg(feature = "diagram-flowchart")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity, TextMeasurementReport,
    TextMeasurementSource,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::TextStyle;
use serde_json::json;
use std::sync::{Arc, Mutex};

struct TitleMeasurementHost {
    requests: Mutex<Vec<(String, TextStyle)>>,
    left: f64,
    right: f64,
}

impl HostTextMeasurer for TitleMeasurementHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        if request.operation != TextMeasurementOperation::TitleBBoxX {
            return Ok(None);
        }
        assert_eq!(request.phase, TextMeasurementPhase::SvgBBox);
        assert_eq!(request.max_width, None);
        self.requests
            .lock()
            .unwrap()
            .push((request.text.to_owned(), request.style.clone()));
        Ok(Some(HostTextMeasurement::HorizontalExtents {
            left: self.left,
            right: self.right,
        }))
    }
}

fn render(environment: RenderEnvironment, backend: &str) -> (String, TextMeasurementReport) {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "layout": backend,
        "look": "classic",
        "fontFamily": "Arial",
        "htmlLabels": false,
        "flowchart": { "diagramPadding": 0 }
    })));
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "---\ntitle: Host measured title\n---\nflowchart TD\nA[one] --> B[two]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let (svg, _, _, session) = rendered.into_parts();
    (svg, session.text_measurement_report())
}

fn title_anchor_and_viewbox(svg: &str) -> (f64, Vec<f64>) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("flowchartTitleText"))
        .unwrap();
    let anchor = title.attribute("x").unwrap().parse().unwrap();
    let viewbox = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect();
    (anchor, viewbox)
}

#[test]
fn flowchart_title_host_extents_expand_the_viewport_without_moving_the_title() {
    for backend in ["dagre", "elk"] {
        let (deterministic_svg, _) = render(RenderEnvironment::deterministic(), backend);
        let (original_anchor, original_viewbox) = title_anchor_and_viewbox(&deterministic_svg);
        // Deliberately asymmetric glyph extents exercise both sides independently.
        // These are test host responses, not padding added by the renderer.
        let host = Arc::new(TitleMeasurementHost {
            requests: Mutex::new(Vec::new()),
            left: 600.25,
            right: 430.5,
        });
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.actual-display-title").unwrap(),
            "v1",
        )
        .unwrap();
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::host_display(
                identity.clone(),
                host.clone(),
                [TextMeasurementPhase::SvgBBox],
            ),
        );
        let (svg, report) = render(environment, backend);
        let (anchor, viewbox) = title_anchor_and_viewbox(&svg);
        assert_eq!(anchor, original_anchor, "{backend}");
        assert!(
            (viewbox[0] - (anchor - host.left)).abs() < 1e-9,
            "{backend}"
        );
        assert!(
            (viewbox[2] - (host.left + host.right)).abs() < 1e-9,
            "{backend}"
        );
        assert!(viewbox[2] > original_viewbox[2], "{backend}");
        assert_eq!(viewbox[1], original_viewbox[1], "{backend}");
        assert_eq!(viewbox[3], original_viewbox[3], "{backend}");

        let requests = host.requests.lock().unwrap();
        assert!(!requests.is_empty(), "{backend}");
        for (text, style) in requests.iter() {
            assert_eq!(text, "Host measured title");
            assert_eq!(style.font_family.as_deref(), Some("Arial"));
            assert_eq!(style.font_size, 18.0);
            assert_eq!(style.font_weight, None);
            assert_eq!(style.font_style, None);
        }
        assert!(report.entries().iter().any(|entry| {
            let provenance = entry.provenance();
            provenance.operation == TextMeasurementOperation::TitleBBoxX
                && provenance.source == TextMeasurementSource::Host
                && provenance.identity == identity
                && provenance.fallback_reason.is_none()
        }));
        assert!(
            report.entries().iter().any(|entry| {
                entry.provenance().operation != TextMeasurementOperation::TitleBBoxX
            })
        );
        assert!(report.entries().iter().all(|entry| {
            let provenance = entry.provenance();
            provenance.operation == TextMeasurementOperation::TitleBBoxX
                || provenance.source == TextMeasurementSource::Profile
        }));
    }
}
