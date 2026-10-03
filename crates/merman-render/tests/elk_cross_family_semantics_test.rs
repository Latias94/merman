#![cfg(feature = "layout-elk")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

#[derive(Debug, PartialEq)]
struct SvgGeometry {
    paths: Vec<(String, String)>,
    transforms: Vec<(String, String, String)>,
    markers: Vec<(String, String)>,
}

fn geometry(svg: &str) -> SvgGeometry {
    let document = roxmltree::Document::parse(svg).expect("valid SVG");
    let mut paths = Vec::new();
    let mut transforms = Vec::new();
    let mut markers = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        if node.has_tag_name("g")
            && let Some(transform) = node.attribute("transform")
        {
            transforms.push((
                node.attribute("id").unwrap_or_default().to_owned(),
                node.attribute("class").unwrap_or_default().to_owned(),
                transform.to_owned(),
            ));
        }
        if node.has_tag_name("path")
            && node.ancestors().any(|ancestor| {
                ancestor.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| matches!(class, "edgePaths" | "edges"))
                })
            })
        {
            paths.push((
                node.attribute("id")
                    .or_else(|| node.attribute("data-id"))
                    .unwrap_or_default()
                    .to_owned(),
                node.attribute("d").expect("edge path geometry").to_owned(),
            ));
            for attribute in ["marker-start", "marker-end"] {
                if let Some(reference) = node.attribute(attribute) {
                    markers.push((attribute.to_owned(), reference.to_owned()));
                }
            }
        }
    }
    SvgGeometry {
        paths,
        transforms,
        markers,
    }
}

fn render(source: &str, line_hops: Option<Value>) -> (Value, SvgGeometry) {
    let mut config = json!({"layout": "elk", "look": "classic", "elk": {}});
    if let Some(value) = line_hops {
        config["elk"]["lineHops"] = value;
    }
    let parsed = Engine::new()
        .with_site_config(MermaidConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse crossing fixture")
        .expect("detect diagram");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare crossing fixture");
    let layout = artifact.layout_json().expect("layout JSON")["layout"].clone();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("elk-crossings".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render crossing fixture");
    (layout, geometry(svg.svg()))
}

fn assert_line_hops_preserve_layout(source: &str) {
    let (plain_layout, plain) = render(source, Some(json!(false)));
    let (arc_layout, arc) = render(source, Some(json!(true)));
    let (gap_layout, gap) = render(source, Some(json!("gap")));
    let (default_layout, default) = render(source, None);
    assert_eq!(plain.paths.len(), 9, "K3,3 retains all nine relationships");
    assert!(
        !plain.transforms.is_empty(),
        "node and label transforms are observed"
    );
    assert!(
        !plain.markers.is_empty(),
        "relationship markers are observed"
    );
    for (layout, svg) in [
        (&arc_layout, &arc),
        (&gap_layout, &gap),
        (&default_layout, &default),
    ] {
        assert_eq!(
            layout, &plain_layout,
            "line hops must not affect provider layout"
        );
        assert_eq!(svg.paths.len(), plain.paths.len());
        assert_eq!(
            svg.transforms, plain.transforms,
            "node and label placement must remain fixed"
        );
        assert_eq!(
            svg.markers, plain.markers,
            "relationship markers must remain attached"
        );
    }
    assert_ne!(
        arc.paths, plain.paths,
        "enabled arcs must affect an actual crossing"
    );
    assert_ne!(
        gap.paths, plain.paths,
        "enabled gaps must affect an actual crossing"
    );
    assert_ne!(
        arc.paths, gap.paths,
        "arc and gap styles must remain distinct"
    );
    assert!(
        arc.paths.iter().any(|(_, path)| path.contains('A')),
        "arc mode emits an arc"
    );
    assert!(
        gap.paths
            .iter()
            .any(|(_, path)| path.matches('M').count() > 1),
        "gap mode splits a path"
    );
    assert_eq!(default, arc, "ELK line hops default to arcs");
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_elk_line_hops_preserve_layout_and_labels() {
    assert_line_hops_preserve_layout(include_str!("fixtures/elk/flowchart_crossings.mmd"));
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_elk_line_hops_preserve_layout_and_labels() {
    assert_line_hops_preserve_layout(include_str!("fixtures/elk/state_crossings.mmd"));
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_elk_line_hops_preserve_layout_and_labels() {
    assert_line_hops_preserve_layout(include_str!("fixtures/elk/class_crossings.mmd"));
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_elk_line_hops_preserve_layout_and_labels() {
    assert_line_hops_preserve_layout(include_str!("fixtures/elk/er_crossings.mmd"));
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_elk_line_hops_preserve_layout_and_labels() {
    assert_line_hops_preserve_layout(include_str!("fixtures/elk/requirement_crossings.mmd"));
}

#[cfg(feature = "diagram-usecase")]
#[test]
fn usecase_elk_line_hops_preserve_layout_and_labels() {
    assert_line_hops_preserve_layout(include_str!("fixtures/elk/usecase_crossings.mmd"));
}
