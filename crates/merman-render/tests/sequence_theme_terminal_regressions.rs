#![cfg(feature = "diagram-sequence")]

mod common;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{DiagramTheme, DiagramThemeCompiler};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::SequenceDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle};
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
    render_with_layout(source, config, theme, environment).0
}

fn render_with_layout(
    source: &str,
    config: Value,
    theme: &DiagramTheme,
    environment: RenderEnvironment,
) -> (String, SequenceDiagramLayout) {
    let engine = common::install_parse_compatibility(theme, MermaidEngine::new())
        .with_site_config(SourcePresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = environment.begin_session_with_theme(theme).unwrap();
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
        .next_back()
}

#[test]
fn title_root_size_override_keeps_typed_role_sizes_active() {
    let theme = theme(json!({
        "typography": {"families": {"sequence": {"font_size_px": 24}}}
    }));
    for (css_size, title_size_px) in [("52px", 52.0), ("24pt", 32.0)] {
        let svg = render(
            "sequenceDiagram\ntitle Authored Root Size\nparticipant A\nparticipant B\nloop Retry\nA->>B: Ping\nNote over A,B: Note\nend\n",
            json!({
                "theme": "base", "look": "classic",
                "themeVariables": {"fontSize": css_size},
                "sequence": {"mirrorActors": false}
            }),
            &theme,
            RenderEnvironment::deterministic(),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let css = stylesheet(&document);
        assert_eq!(
            css_property(&css, "#sequence-terminals", "font-size"),
            Some(css_size)
        );
        for class in ["actor", "messageText", "noteText", "labelText", "loopText"] {
            let labels = document
                .descendants()
                .filter(|node| node.has_tag_name("text") && has_class(*node, class))
                .collect::<Vec<_>>();
            assert!(!labels.is_empty(), "missing {class} terminal");
            for label in labels {
                assert!(
                    label
                        .attribute("style")
                        .unwrap_or_default()
                        .contains("font-size:24px"),
                    "{class} must retain the typed role default: {label:?}"
                );
            }
        }
        let title = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Authored Root Size"))
            .unwrap();
        assert!(title.attribute("style").is_none());
        assert!(
            view_box(&document)[1] <= number(title, "y") - title_size_px * 0.8,
            "title geometry must measure the configured root size"
        );
    }
}

#[test]
fn normal_and_base_actor_typography_reserve_the_complete_unwrapped_stack() {
    for design in large_actor_designs() {
        let theme = theme(design);
        for mirror in [false, true] {
            let svg = render(
                "sequenceDiagram\nparticipant A as WWWW\nparticipant B as WWWW\nA->>B: Ping\n",
                json!({"theme": "base", "look": "classic", "sequence": {"mirrorActors": mirror}}),
                &theme,
                RenderEnvironment::deterministic(),
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let actors = document
                .descendants()
                .filter(|node| node.has_tag_name("rect") && node.attribute("name").is_some())
                .collect::<Vec<_>>();
            assert_eq!(actors.len(), if mirror { 4 } else { 2 });
            for actor in actors {
                assert!(number(actor, "height") >= 112.0, "{actor:?}");
            }
            let top_actor = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("name") == Some("A")
                        && has_class(*node, "actor-top")
                })
                .unwrap();
            let top_bottom = number(top_actor, "y") + number(top_actor, "height");
            for lifeline in document
                .descendants()
                .filter(|node| node.attribute("data-et") == Some("life-line"))
            {
                assert!(number(lifeline, "y1") >= top_bottom);
            }
        }
    }
}

fn large_actor_designs() -> [Value; 2] {
    [
        json!({"rules": [{"family": "sequence", "target": "actor-label", "style": {
            "typography": {"font_stack": ["Arial", "sans-serif"], "font_size_px": 112, "font_style": "normal"}
        }}]}),
        json!({"typography": {"families": {"sequence": {
            "font_stack": ["Arial", "sans-serif"], "font_size_px": 112
        }}}}),
    ]
}

fn large_actor_label_height() -> f64 {
    DeterministicTextMeasurer::default().measure_svg_tspan_text_bbox_height_px(
        "WWWW",
        &TextStyle {
            font_family: Some("Arial, sans-serif".to_owned()),
            font_size: 112.0,
            font_weight: Some("400".to_owned()),
            font_style: Some("normal".to_owned()),
        },
    )
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
fn classic_large_normal_and_base_glyph_labels_fit_top_footer_and_lifeline_geometry() {
    let text_height = large_actor_label_height();
    for design in large_actor_designs() {
        let theme = theme(design);
        for actor_type in ["actor", "boundary", "control", "entity", "database"] {
            for mirror in [false, true] {
                let source = format!(
                    "sequenceDiagram\nparticipant A@{{\"type\":\"{actor_type}\"}} as WWWW\n"
                );
                let (svg, layout) = render_with_layout(
                    &source,
                    json!({"theme":"base","look":"classic","sequence":{"mirrorActors":mirror}}),
                    &theme,
                    RenderEnvironment::deterministic(),
                );
                let document = roxmltree::Document::parse(&svg).unwrap();
                let viewport = view_box(&document);
                assert!(!svg.contains("data-merman-prepared-math"));
                assert!(
                    !document
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject"))
                );
                for footer in [false, true]
                    .into_iter()
                    .filter(|footer| !*footer || mirror)
                {
                    let placement = if footer { "actor-bottom" } else { "actor-top" };
                    let actor = layout
                        .nodes
                        .iter()
                        .find(|node| node.id == format!("{placement}-A"))
                        .unwrap();
                    let top = actor.y - actor.height / 2.0;
                    let bottom = top + actor.height;
                    let text = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("text") && has_class(*node, "actor") && {
                                let center = number(*node, "y") + translated_y(*node);
                                center >= top && center <= bottom
                            }
                        })
                        .unwrap();
                    let center = number(text, "y") + translated_y(text);
                    let label_top = center - text_height / 2.0;
                    let label_bottom = center + text_height / 2.0;
                    let glyph_bottom = if actor_type == "database" {
                        let path = document
                            .descendants()
                            .find(|node| {
                                node.has_tag_name("path")
                                    && node
                                        .ancestors()
                                        .any(|ancestor| has_class(ancestor, placement))
                            })
                            .unwrap();
                        top + actor.width / 3.0 + translated_y(path)
                    } else {
                        let circle = text
                            .parent()
                            .unwrap()
                            .children()
                            .find(|node| node.has_tag_name("circle"))
                            .unwrap();
                        let circle_bottom =
                            number(circle, "cy") + number(circle, "r") + translated_y(circle);
                        if actor_type == "actor" {
                            circle_bottom.max(top + 60.0)
                        } else {
                            circle_bottom
                        }
                    };
                    assert!(label_top > glyph_bottom, "{actor_type}/{placement}: {svg}");
                    assert!(
                        label_top >= top && label_bottom <= bottom + 1e-6,
                        "{actor_type}/{placement}: {svg}"
                    );
                    assert!(
                        viewport[1] <= label_top && viewport[1] + viewport[3] >= label_bottom,
                        "{actor_type}/{placement}: {svg}"
                    );
                    if !footer {
                        let lifeline = layout
                            .edges
                            .iter()
                            .find(|edge| edge.id == "lifeline-A")
                            .unwrap();
                        assert!(
                            lifeline.points[0].y >= label_bottom,
                            "{actor_type}: lifeline crosses label"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn expanded_collection_and_queue_popups_use_prepared_row_origins() {
    let text_height = large_actor_label_height();
    for design in large_actor_designs() {
        let theme = theme(design);
        for look in ["classic", "neo"] {
            for actor_type in ["collections", "queue"] {
                for mirror in [false, true] {
                    for force in [false, true] {
                        let source = format!(
                            "sequenceDiagram\nparticipant A@{{\"type\":\"{actor_type}\"}} as WWWW\nlinks A: {{\"WWWW\":\"https://example.com\"}}\n"
                        );
                        let (svg, layout) = render_with_layout(
                            &source,
                            json!({"theme":"base","look":look,"sequence":{"mirrorActors":mirror,"forceMenus":force}}),
                            &theme,
                            RenderEnvironment::deterministic(),
                        );
                        let document = roxmltree::Document::parse(&svg).unwrap();
                        let popup = document
                            .descendants()
                            .find(|node| has_class(*node, "actorPopupMenu"))
                            .unwrap();
                        assert_eq!(
                            popup.attribute("display"),
                            Some(if force { "block !important" } else { "none" })
                        );
                        let panel = popup
                            .children()
                            .find(|node| node.has_tag_name("rect"))
                            .unwrap();
                        let actor = layout
                            .nodes
                            .iter()
                            .find(|node| {
                                node.id
                                    == if mirror {
                                        "actor-bottom-A"
                                    } else {
                                        "actor-top-A"
                                    }
                            })
                            .unwrap();
                        let offset = if actor_type == "collections" && look == "neo" {
                            6.0
                        } else {
                            0.0
                        };
                        assert!(actor.height > 112.0);
                        assert_eq!(
                            number(panel, "y"),
                            actor.height - offset,
                            "{look}/{actor_type}/mirror={mirror}"
                        );
                        let menu = popup
                            .descendants()
                            .find(|node| node.has_tag_name("text"))
                            .unwrap();
                        assert!(number(menu, "y") - text_height / 2.0 >= number(panel, "y"));
                        assert!(
                            number(menu, "y") + text_height / 2.0
                                <= number(panel, "y") + number(panel, "height")
                        );
                        let viewport = view_box(&document);
                        assert!(
                            viewport[1] + viewport[3]
                                >= number(panel, "y") + number(panel, "height")
                        );
                        if !mirror {
                            let participant = document
                                .descendants()
                                .find(|node| {
                                    node.has_tag_name("text") && has_class(*node, "actor-box")
                                })
                                .unwrap();
                            let participant_bottom = number(participant, "y") + text_height / 2.0;
                            assert!(
                                participant_bottom <= number(panel, "y"),
                                "{look}/{actor_type}: menu covers participant label"
                            );
                        }
                    }
                }
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
    for design in [
        json!({}),
        json!({"rules": [{"family": "sequence", "target": "actor-label", "style": {
            "typography": {"font_size_px": 112}
        }}]}),
    ] {
        let theme = theme(design);
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
                        &theme,
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

#[test]
fn classic_untyped_glyphs_keep_legacy_top_and_footer_positions() {
    let theme = theme(json!({}));
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
                &theme,
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
fn typed_actor_footprints_follow_visible_actor_order_after_hiding_unused() {
    let theme = theme(large_actor_designs()[0].clone());
    let text_height = large_actor_label_height();
    for (actor_type, unused_label) in ["boundary", "database"].into_iter().flat_map(|actor_type| {
        ["Hidden"]
            .into_iter()
            .chain(cfg!(feature = "math").then_some("$$x$$"))
            .map(move |unused_label| (actor_type, unused_label))
    }) {
        let environment = RenderEnvironment::deterministic();
        #[cfg(feature = "math")]
        let environment = environment.with_compiled_math_renderer();
        let (svg, layout) = render_with_layout(
            &format!(
                "sequenceDiagram\nparticipant Unused as {unused_label}\nparticipant A@{{\"type\":\"{actor_type}\"}} as WWWW\nparticipant B as Worker\nA->>B: Ping\n"
            ),
            json!({"theme":"base","look":"classic","sequence":{"hideUnusedParticipants":true,"mirrorActors":true}}),
            &theme,
            environment,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(!layout.nodes.iter().any(|node| node.id.contains("Unused")));
        let actor = layout
            .nodes
            .iter()
            .find(|node| node.id == "actor-top-A")
            .unwrap();
        let text = document
            .descendants()
            .find(|node| {
                node.has_tag_name("text")
                    && has_class(*node, "actor")
                    && node
                        .ancestors()
                        .any(|ancestor| ancestor.attribute("data-id") == Some("A"))
            })
            .unwrap();
        let center = number(text, "y") + translated_y(text);
        assert!(center - text_height / 2.0 >= actor.y - actor.height / 2.0);
        assert!(center + text_height / 2.0 <= actor.y + actor.height / 2.0);
        let lifeline = layout
            .edges
            .iter()
            .find(|edge| edge.id == "lifeline-A")
            .unwrap();
        assert!(lifeline.points[0].y >= center + text_height / 2.0);
    }
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
