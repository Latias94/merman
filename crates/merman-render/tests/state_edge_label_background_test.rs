#![cfg(all(
    feature = "diagram-state",
    any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )
))]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::environment::RenderEnvironment;
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
const SOURCE: &str = "stateDiagram-v2
state Outer {
  state Inner {
    A --> B: inner transition
  }
  C --> D: outer transition
}
X --> Y: canvas transition
classDef backdrop fill:#88CCCC80
class Inner backdrop
";
const CANVAS: &str = "#F1EEE3";
const CONTAINER: &str = "#244464";
const NODE: &str = "#EDF4FC";
const BORDER: &str = "#516D87";

fn render(source: &str, config: Value, mode: Option<&str>) -> String {
    let parsed = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    let session = if let Some(mode) = mode {
        let theme = json!({
            "schema_version": 2,
            "extends": "studio",
            "roles": {
                "surface.container": CONTAINER,
                "surface.node": NODE,
                "surface.emphasis": "#BA573A",
                "stroke.border": BORDER
            },
            "form": {"edge_label_bg": mode, "elevation": "flat"}
        });
        let theme = ThemeCompiler::new()
            .compile_json(&serde_json::to_vec(&theme).unwrap(), None)
            .unwrap();
        environment
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap()
            .with_canvas_background(Color::parse(CANVAS).unwrap())
    } else {
        environment.begin_session().unwrap()
    };
    family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(&Default::default(), &Default::default())
    .unwrap()
    .svg()
    .to_owned()
}

fn label<'a, 'input>(doc: &'a Document<'input>, text: &str) -> Node<'a, 'input> {
    doc.descendants()
        .find(|node| {
            node.attribute("class") == Some("label")
                && node.parent().and_then(|parent| parent.attribute("class")) == Some("edgeLabel")
                && node
                    .descendants()
                    .filter(Node::is_text)
                    .filter_map(|part| part.text())
                    .collect::<String>()
                    == text
        })
        .unwrap_or_else(|| panic!("missing State transition label {text}"))
}

fn backplate<'a, 'input>(label: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    label.parent()?.children().find(|node| {
        node.has_tag_name("rect") && node.attribute("class") == Some("merman-edge-label-background")
    })
}

fn extent(node: Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name).unwrap().parse().unwrap()
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[test]
fn state_auto_backplates_follow_nested_paint_and_canvas_in_both_label_modes() {
    let blended = Color::parse("#88CCCC80")
        .unwrap()
        .composite(Color::parse(CONTAINER).unwrap(), 1.0)
        .to_string();
    for layout in [
        #[cfg(feature = "layout-dagre")]
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
    ] {
        for html in [false, true] {
            let svg = render(
                SOURCE,
                json!({"layout": layout, "look": "classic", "htmlLabels": html}),
                Some("auto"),
            );
            let doc = Document::parse(&svg).unwrap();
            for (text, fill) in [
                ("inner transition", blended.as_str()),
                ("outer transition", CONTAINER),
                ("canvas transition", CANVAS),
            ] {
                let label = label(&doc, text);
                let plate = backplate(label).unwrap();
                assert_eq!(
                    plate.attribute("fill"),
                    Some(fill),
                    "{layout}, {html}, {text}"
                );
                assert_eq!(plate.attribute("rx"), Some("4"));
                assert_eq!(plate.attribute("ry"), Some("4"));
                let footprint = label
                    .descendants()
                    .find(|node| {
                        if html {
                            node.has_tag_name("foreignObject")
                        } else {
                            node.attribute("class") == Some("background")
                        }
                    })
                    .unwrap();
                assert!((extent(plate, "width") - extent(footprint, "width") - 8.0).abs() < 1e-6);
                assert!((extent(plate, "height") - extent(footprint, "height") - 4.0).abs() < 1e-6);
            }
            let gradient = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("linearGradient")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.contains("merman-state-composite-header"))
                })
                .unwrap();
            assert_eq!(
                gradient
                    .last_element_child()
                    .unwrap()
                    .attribute("stop-opacity"),
                Some("0")
            );
        }
    }
}

#[test]
fn state_edge_label_paint_keeps_native_and_mermaid_defaults_independent() {
    let layout = if cfg!(feature = "layout-dagre") {
        "dagre"
    } else if cfg!(feature = "layout-elk") {
        "elk"
    } else {
        "shoal"
    };
    let source = "stateDiagram-v2\nA --> B: move\n";
    for html in [false, true] {
        let config = json!({"layout": layout, "look": "classic", "htmlLabels": html});
        let native_svg = render(source, config.clone(), Some("pill"));
        let doc = Document::parse(&native_svg).unwrap();
        let plate = backplate(label(&doc, "move")).unwrap();
        assert_eq!(plate.attribute("fill"), Some(NODE));
        assert_eq!(plate.attribute("stroke"), Some(BORDER));
        assert_eq!(plate.attribute("stroke-width"), Some("0.75"));
        assert!((extent(plate, "rx") - extent(plate, "height") / 2.0).abs() < 1e-6);

        for mode in [Some("transparent"), None] {
            let svg = render(source, config.clone(), mode);
            let doc = Document::parse(&svg).unwrap();
            assert!(backplate(label(&doc, "move")).is_none());
            assert_eq!(svg.contains(".edgeLabel rect{opacity:0.5;"), mode.is_none());
        }

        for (edge_fill, svg_label_fill) in [
            (Some("#A18263"), None),
            (Some("#A18263"), Some("#B29374")),
            (None, Some("#B29374")),
        ] {
            let mut authored = config.clone();
            authored["themeVariables"] = json!({"transitionLabelColor": "#102938"});
            if let Some(fill) = edge_fill {
                authored["themeVariables"]["edgeLabelBackground"] = json!(fill);
            }
            if let Some(fill) = svg_label_fill {
                authored["themeVariables"]["labelBackgroundColor"] = json!(fill);
            }
            assert_eq!(render(source, authored.clone(), Some("pill")), native_svg);
            let svg = render(source, authored, None);
            let doc = Document::parse(&svg).unwrap();
            assert!(backplate(label(&doc, "move")).is_none());
            if let Some(fill) = edge_fill {
                assert!(svg.contains(&format!(".edgeLabel p{{background-color:{fill};")));
            }
            if let Some(fill) = svg_label_fill {
                assert!(svg.contains(&format!(".edgeLabel .label rect{{fill:{fill};")));
            }
            assert!(
                svg.contains(".edgeLabel .label text{fill:#102938;"),
                "{svg}"
            );
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn state_shoal_backplate_keeps_neo_transition_masks_and_marker_geometry() {
    let config = json!({"layout": "shoal", "look": "neo", "htmlLabels": false});
    let source = "stateDiagram-v2\nA --> B: move\n";
    let auto = render(source, config.clone(), Some("auto"));
    let transparent = render(source, config, Some("transparent"));
    let auto = Document::parse(&auto).unwrap();
    let transparent = Document::parse(&transparent).unwrap();
    let edge = |doc: &Document<'_>| {
        let path = doc
            .descendants()
            .find(|node| node.attribute("data-edge") == Some("true"))
            .unwrap();
        ["style", "d", "data-points", "marker-end"]
            .map(|attr| path.attribute(attr).unwrap().to_owned())
    };
    assert!(edge(&auto)[0].contains("stroke-dasharray"));
    assert_eq!(edge(&auto), edge(&transparent));
    assert_eq!(
        backplate(label(&auto, "move")).unwrap().attribute("fill"),
        Some(CANVAS)
    );
}
