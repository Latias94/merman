use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use std::sync::Arc;

struct WideInheritedFontHost;

impl HostTextMeasurer for WideInheritedFontHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        if request.style.font_family.as_deref() != Some("WideInheritedFont") {
            return Ok(None);
        }
        Ok(match request.operation {
            TextMeasurementOperation::BBoxX => {
                let half_width = if request.text == "Syntax error in text" {
                    1_800.0
                } else if request.text.starts_with("mermaid version ") {
                    1_400.0
                } else if request.text.starts_with('v') {
                    300.0
                } else {
                    return Ok(None);
                };
                Some(HostTextMeasurement::HorizontalExtents {
                    left: half_width,
                    right: half_width,
                })
            }
            TextMeasurementOperation::RawBBoxHeight => {
                Some(HostTextMeasurement::Length(request.style.font_size))
            }
            _ => None,
        })
    }
}

fn wide_inherited_font_environment() -> RenderEnvironment {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.wide-inherited-font").expect("valid profile id"),
        "1",
    )
    .expect("valid profile identity");
    RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(
            identity,
            Arc::new(WideInheritedFontHost),
            [TextMeasurementPhase::SvgBBox],
        ),
    )
}

fn render_with_wide_font(source: &str, parse_options: ParseOptions) -> family::RenderedFamilySvg {
    let parsed = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {"fontFamily": "WideInheritedFont"}
        })))
        .parse_diagram_for_render_model_sync(source, parse_options)
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = wide_inherited_font_environment().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
}

fn root_max_width_px(root: roxmltree::Node<'_, '_>) -> f64 {
    root.attribute("style")
        .expect("root max-width style")
        .split(';')
        .map(str::trim)
        .find_map(|declaration| declaration.strip_prefix("max-width:"))
        .map(str::trim)
        .and_then(|value| value.strip_suffix("px"))
        .and_then(|value| value.parse().ok())
        .expect("numeric root max-width")
}

#[cfg(feature = "diagram-info")]
#[test]
fn info_authored_font_expands_the_viewport_and_moves_the_version_anchor() {
    let rendered = render_with_wide_font("info", ParseOptions::strict());
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let root = document.root_element();
    let version = root
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("version"))
        .unwrap();
    assert_eq!(root.attribute("aria-roledescription"), Some("info"));
    assert_eq!(root_max_width_px(root), 1_200.0);
    assert_eq!(version.attribute("x"), Some("300"));
    assert_eq!(version.attribute("y"), Some("40"));
    assert_eq!(version.attribute("font-size"), Some("32"));
    assert!(
        rendered
            .svg()
            .contains("#merman{font-family:WideInheritedFont;")
    );
    assert!(
        rendered
            .svg()
            .contains("--mermaid-font-family:WideInheritedFont;")
    );
}

#[test]
fn error_authored_font_expands_the_viewport_and_moves_the_message_anchor() {
    let rendered = render_with_wide_font("flowchart TD\nA -->", ParseOptions::lenient());
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let root = document.root_element();
    let view_box_width: f64 = root
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    let message = root
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("Syntax error in text"))
        .unwrap();
    let message_x: f64 = message.attribute("x").unwrap().parse().unwrap();
    assert_eq!(root.attribute("aria-roledescription"), Some("error"));
    assert!(view_box_width >= 4_400.0, "{view_box_width}");
    assert!(root_max_width_px(root) > 512.0);
    assert!(message_x > 1_440.0, "{message_x}");
    assert_eq!(message.attribute("font-size"), Some("150px"));
}
