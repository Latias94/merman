#![cfg(feature = "diagram-sequence")]

mod common;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{DiagramTheme, DiagramThemeCompiler};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

const DIAGRAM_ID: &str = "sequence-terminals";

fn theme(design: Value) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile_recipe(
            serde_json::from_value(json!({
                "schema_version": 1,
                "kind": "design",
                "design": design
            }))
            .unwrap(),
        )
        .unwrap()
}

fn render(
    source: &str,
    config: Value,
    theme: &DiagramTheme,
    environment: RenderEnvironment,
) -> String {
    let engine = common::install_parse_compatibility(theme, MermaidEngine::new())
        .with_site_config(SourcePresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = environment.begin_session_with_theme(theme).unwrap();
    family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(DIAGRAM_ID.to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_owned()
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

fn stylesheet(document: &roxmltree::Document<'_>) -> String {
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect()
}

fn css_property<'a>(css: &'a str, selector: &str, property: &str) -> Option<&'a str> {
    css.split('}')
        .filter_map(|rule| rule.split_once('{'))
        .filter(|(selectors, _)| selectors.split(',').any(|value| value.trim() == selector))
        .flat_map(|(_, declarations)| declarations.split(';'))
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim())
        .last()
}

#[test]
fn lifeline_shadow_contains_the_css_stroke_for_default_explicit_and_cleared_widths() {
    let width_rule = |width| json!({"target":"lifeline","style":{"stroke":{"width":width}}});
    for (rules, expected_override) in [
        (vec![], None),
        (vec![width_rule(json!(6))], Some("6px")),
        (vec![width_rule(json!(6)), width_rule(Value::Null)], None),
    ] {
        let theme = theme(json!({
            "styles": {"lifeline": {"effect": {
                "kind":"shadow-stack", "composition":"independent",
                "shadows":[{"offset_x":12,"offset_y":0,"std_deviation":0,"color":"#f97316","opacity":1}]
            }}},
            "rules":rules
        }));
        let svg = render(
            "sequenceDiagram\nA->>B: Ping\n",
            json!({"theme":"base","look":"classic"}),
            &theme,
            RenderEnvironment::deterministic(),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let css = stylesheet(&document);
        let baseline = css_property(&css, "#sequence-terminals line", "stroke-width").unwrap();
        assert_eq!(baseline, "2px");
        let override_width = css_property(&css, "#sequence-terminals .actor-line", "stroke-width");
        assert_eq!(override_width, expected_override);
        let effective_width: f64 = override_width
            .unwrap_or(baseline)
            .trim_end_matches("px")
            .parse()
            .unwrap();
        let lifelines: Vec<_> = document
            .descendants()
            .filter(|node| node.attribute("data-et") == Some("life-line"))
            .collect();
        assert_eq!(lifelines.len(), 2);
        for line in lifelines {
            assert_eq!(line.attribute("stroke-width"), Some("0.5px"));
            let reference = line.attribute("filter").unwrap();
            let id = reference
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let filter = document
                .descendants()
                .find(|node| node.has_tag_name("filter") && node.attribute("id") == Some(id))
                .unwrap();
            assert_eq!(filter.attribute("filterUnits"), Some("userSpaceOnUse"));
            let x = number(line, "x1");
            assert!(number(filter, "x") <= x - effective_width / 2.0);
            assert!(
                number(filter, "x") + number(filter, "width") >= x + effective_width / 2.0 + 12.0
            );
        }
    }
}

#[test]
fn neo_canvas_contains_retained_actor_shadows_and_respects_explicit_clear() {
    let source = "sequenceDiagram\nparticipant A\n";
    let config = json!({"theme":"redux","look":"neo","sequence":{"diagramMarginY":0}});
    let mut canvas_bottoms = Vec::new();
    for clear in [false, true] {
        let styles = if clear {
            json!({"actor":{"effect":null}})
        } else {
            json!({})
        };
        let baseline = render(
            source,
            config.clone(),
            &theme(json!({"styles":styles})),
            RenderEnvironment::deterministic(),
        );
        let svg = render(
            source,
            config.clone(),
            &theme(json!({"canvas":{"base":"#ffffff"},"styles":styles})),
            RenderEnvironment::deterministic(),
        );
        let baseline = roxmltree::Document::parse(&baseline).unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let viewport = view_box(&document);
        let canvas = document
            .descendants()
            .find(|node| node.attribute("data-merman-theme-canvas") == Some("base"))
            .unwrap();
        assert_eq!(
            ["x", "y", "width", "height"].map(|name| number(canvas, name)),
            viewport
        );
        let baseline_actors: Vec<_> = baseline
            .descendants()
            .filter(|node| node.has_tag_name("rect") && has_class(*node, "actor"))
            .collect();
        let actors: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("rect") && has_class(*node, "actor"))
            .collect();
        assert_eq!(actors.len(), 2);
        assert_eq!(baseline_actors.len(), actors.len());
        let css = stylesheet(&document);
        let half_stroke = css_property(&css, "#sequence-terminals .actor", "stroke-width")
            .unwrap()
            .parse::<f64>()
            .unwrap()
            / 2.0;
        let mut footer_bottom = f64::NEG_INFINITY;
        for (actor, baseline_actor) in actors.into_iter().zip(baseline_actors) {
            assert_eq!(
                ["x", "y", "width", "height"].map(|name| number(actor, name)),
                ["x", "y", "width", "height"].map(|name| number(baseline_actor, name))
            );
            let (dx, dy) = if clear {
                assert!(actor.attribute("filter").is_none());
                (0.0, 0.0)
            } else {
                assert_eq!(
                    actor.attribute("filter"),
                    Some("url(#sequence-terminals-drop-shadow)")
                );
                let shadow = document
                    .descendants()
                    .find(|node| node.has_tag_name("feDropShadow"))
                    .unwrap();
                assert_eq!(number(shadow, "stdDeviation"), 0.0);
                (number(shadow, "dx"), number(shadow, "dy"))
            };
            let left = number(actor, "x") - half_stroke;
            let top = number(actor, "y") - half_stroke;
            let right = number(actor, "x") + number(actor, "width") + half_stroke + dx;
            let bottom = number(actor, "y") + number(actor, "height") + half_stroke + dy;
            assert!(viewport[0] <= left && viewport[1] <= top);
            assert!(viewport[0] + viewport[2] >= right);
            assert!(viewport[1] + viewport[3] >= bottom);
            footer_bottom = footer_bottom.max(number(actor, "y") + number(actor, "height"));
        }
        let baseline_viewport = view_box(&baseline);
        assert_eq!(
            baseline_viewport[1] + baseline_viewport[3],
            footer_bottom + 1.0
        );
        canvas_bottoms.push(viewport[1] + viewport[3]);
    }
    assert!(canvas_bottoms[0] > canvas_bottoms[1]);
}

#[cfg(feature = "math")]
#[test]
fn primary_control_math_uses_the_frame_center_at_two_keyword_widths() {
    let theme = theme(json!({}));
    for keyword_width in [50, 100] {
        for keyword in ["loop", "alt", "critical"] {
            let source = format!("sequenceDiagram\n{keyword} $$x^2$$\nA->>B: Work\nend\n");
            let svg = render(
                &source,
                json!({"look":"classic","sequence":{"labelBoxWidth":keyword_width,"wrap":false}}),
                &theme,
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
