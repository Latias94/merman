#![cfg(feature = "diagram-sequence")]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::SequenceDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use serde_json::{Value, json};

const DIAGRAM_ID: &str = "sequence-terminals";

fn native_fixture() -> NativeTheme {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
        "schema_version": 2, "extends": "paper-cut",
        "typography": {"base_size_px": 23},
        "roles": {"surface.container": "#D9DBDD", "text.title": "#154567"},
        "form": {"radius": 11, "connector_width": 2.5, "marker_style": "open"}
    }"##,
            None,
        )
        .unwrap();
    NativeTheme::new(theme)
}

fn render(
    source: &str,
    config: Value,
    theme: Option<&NativeTheme>,
    environment: RenderEnvironment,
) -> String {
    render_with_layout(source, config, theme, environment).0
}

fn render_with_layout(
    source: &str,
    config: Value,
    theme: Option<&NativeTheme>,
    environment: RenderEnvironment,
) -> (String, SequenceDiagramLayout) {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = match theme {
        Some(theme) => environment.begin_session_with_native_theme(theme.clone()),
        None => environment.begin_session(),
    }
    .unwrap();
    let prepared = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = prepared.layout_json().unwrap();
    let layout = serde_json::from_value(projection["layout"]["SequenceDiagram"].clone()).unwrap();
    let svg = prepared
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(DIAGRAM_ID.to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_owned();
    (svg, layout)
}

fn number(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name)
        .unwrap()
        .trim_end_matches("px")
        .parse()
        .unwrap()
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|value| value.split_whitespace().any(|value| value == class))
}

fn view_box(document: &roxmltree::Document<'_>) -> [f64; 4] {
    document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn translated_y(node: roxmltree::Node<'_, '_>) -> f64 {
    node.ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .map(|transform| {
            let components = transform
                .strip_prefix("translate(")
                .unwrap_or_else(|| panic!("unexpected transform {transform}"))
                .strip_suffix(')')
                .unwrap()
                .split([',', ' '])
                .filter(|value| !value.is_empty())
                .map(|value| value.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            components.get(1).copied().unwrap_or(0.0)
        })
        .sum()
}

#[test]
fn classic_untyped_glyphs_keep_legacy_top_and_footer_positions() {
    for (actor_type, height, top_center, footer_center) in [
        ("actor", 65.0, 67.5, 67.5),
        ("boundary", 64.0, 68.5, 68.0),
        ("control", 84.0, 66.5, 69.0),
        ("entity", 64.0, 68.5, 69.0),
        ("database", 70.0, 67.5, 70.0),
    ] {
        for mirror in [false, true] {
            let (svg, layout) = render_with_layout(
                &format!("sequenceDiagram\nparticipant A@{{\"type\":\"{actor_type}\"}} as A\n"),
                json!({"theme":"base","look":"classic","sequence":{"mirrorActors":mirror}}),
                None,
                RenderEnvironment::deterministic(),
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let top = layout
                .nodes
                .iter()
                .find(|node| node.id == "actor-top-A")
                .unwrap();
            let footer = layout
                .nodes
                .iter()
                .find(|node| node.id == "actor-bottom-A")
                .unwrap();
            let footer_y = footer.y - footer.height / 2.0;
            assert_eq!(top.height, height, "{actor_type}");
            assert_eq!(
                footer.height,
                if mirror { height } else { 0.0 },
                "{actor_type}"
            );
            let labels = document
                .descendants()
                .filter(|node| node.has_tag_name("text") && has_class(*node, "actor"))
                .collect::<Vec<_>>();
            assert_eq!(labels.len(), if mirror { 2 } else { 1 });
            for label in labels {
                let center = number(label, "y") + translated_y(label);
                let expected = if center >= footer_y {
                    footer_y + footer_center
                } else {
                    top_center
                };
                assert_eq!(center, expected, "{actor_type}: {svg}");
            }
        }
    }
}

#[test]
fn popup_root_margins_follow_independent_sequence_configuration() {
    let links: serde_json::Map<String, Value> = (0..8)
        .map(|index| {
            (
                format!("Item {index}"),
                Value::String(format!("https://example.com/{index}")),
            )
        })
        .collect();
    for theme in [None, Some(native_fixture())] {
        for (margin_y, box_margin) in [(10.0, 10.0), (20.0, 7.0), (5.0, 12.0)] {
            for mirror in [false, true] {
                for boxed in [false, true] {
                    let actors = if boxed {
                        "box Group\nparticipant A as Actor\nend\n"
                    } else {
                        "participant A as Actor\n"
                    };
                    let source = format!("sequenceDiagram\n{actors}links A: {}\n", json!(links));
                    let svg = render(
                        &source,
                        json!({"theme": "base", "look": "classic", "sequence": {
                            "mirrorActors": mirror, "forceMenus": true,
                            "diagramMarginY": margin_y, "boxMargin": box_margin,
                            "bottomMarginAdj": 3
                        }}),
                        theme.as_ref(),
                        RenderEnvironment::deterministic(),
                    );
                    let document = roxmltree::Document::parse(&svg).unwrap();
                    let panel = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("rect") && has_class(*node, "actorPopupMenuPanel")
                        })
                        .unwrap();
                    let panel_bottom = number(panel, "y") + number(panel, "height");
                    let viewport = view_box(&document);
                    // The upstream popup height already includes its entire panel. Only mirrored
                    // actors subtract boxMargin; a participant group adds no second popup inset.
                    let expected_padding = margin_y + if mirror { 3.0 - box_margin } else { 0.0 };
                    assert!(
                        (viewport[1] + viewport[3] - panel_bottom - expected_padding).abs() < 1e-8,
                        "margin={margin_y}, boxMargin={box_margin}, mirror={mirror}, boxed={boxed}: viewport={viewport:?}, panel_bottom={panel_bottom}, expected_padding={expected_padding}"
                    );
                }
            }
        }
    }
}

#[cfg(feature = "math")]
#[test]
fn primary_control_math_uses_the_frame_center_at_two_keyword_widths() {
    for keyword_width in [50, 100] {
        for keyword in ["loop", "alt", "critical"] {
            let source = format!("sequenceDiagram\n{keyword} $$x^2$$\nA->>B: Work\nend\n");
            let svg = render(
                &source,
                json!({"look":"classic","sequence":{"labelBoxWidth":keyword_width,"wrap":false}}),
                None,
                RenderEnvironment::deterministic().with_compiled_math_renderer(),
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let control = document
                .descendants()
                .find(|node| node.attribute("data-et") == Some("control-structure"))
                .unwrap();
            let frame_top = control
                .children()
                .find(|node| has_class(*node, "loopLine"))
                .unwrap();
            let math = control
                .children()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            let center = (number(frame_top, "x1") + number(frame_top, "x2")) / 2.0;
            assert_eq!(
                number(math, "x"),
                (center - number(math, "width") / 2.0).round(),
                "{keyword}, labelBoxWidth={keyword_width}"
            );
            assert_eq!(number(math, "y"), number(frame_top, "y1").round());
        }
    }
}

#[test]
fn native_sequence_paints_control_frames_and_measures_actor_labels_with_the_theme_font() {
    let svg = render(
        "sequenceDiagram\ntitle Native sequence\nparticipant A as Reader\nparticipant B as Service\nloop Retry\nA->>+B: Request\nB-->>-A: Reply\nend\n",
        json!({}),
        Some(&native_fixture()),
        RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let frame = document
        .descendants()
        .find(|node| has_class(*node, "merman-sequence-frame-surface"))
        .unwrap();
    assert_eq!(frame.attribute("fill"), Some("#D9DBDD"));
    assert!(
        document
            .descendants()
            .any(|node| has_class(node, "merman-sequence-open-arrow"))
    );
    let actor = document
        .descendants()
        .find(|node| node.has_tag_name("rect") && has_class(*node, "actor-top"))
        .unwrap();
    assert_eq!(actor.attribute("rx"), Some("11"));
    let label = document
        .descendants()
        .find(|node| node.tag_name().name() == "tspan" && node.text() == Some("Reader"))
        .unwrap();
    assert!(
        label
            .parent()
            .unwrap()
            .attribute("style")
            .unwrap()
            .contains("font-size:23px")
    );
    let title = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("Native sequence"))
        .unwrap();
    assert_eq!(title.attribute("fill"), Some("#154567"));
}
