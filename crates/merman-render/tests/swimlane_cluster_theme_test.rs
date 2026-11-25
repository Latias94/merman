#![cfg(feature = "diagram-swimlane")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

const SOURCE: &str = "swimlane-beta TD\nsubgraph Lane[Lane]\nA\nend\n";

fn render(source: &str, config: Value, theme: Option<NativeTheme>) -> String {
    let parsed = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic();
    let session = match theme {
        Some(theme) => environment.begin_session_with_native_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn lane<'a, 'input>(document: &'a Document<'input>) -> Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.attribute("data-id") == Some("Lane")
                && node.attribute("data-et") == Some("cluster")
        })
        .expect("Lane terminal group")
}

fn lane_rect<'a, 'input>(lane: Node<'a, 'input>, class: &str) -> Node<'a, 'input> {
    lane.children()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some(class))
        .expect("Lane terminal rectangle")
}

fn stylesheet<'a>(document: &'a Document<'_>) -> &'a str {
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Swimlane stylesheet")
}

#[test]
fn swimlane_neo_non_palette_lane_stroke_has_the_matching_cluster_css_owner() {
    for (variables, expected) in [
        (json!({}), "#aaaa33"),
        (
            json!({"clusterBorder": "#112233", "nodeBorder": "#445566"}),
            "#112233",
        ),
    ] {
        let svg = render(
            SOURCE,
            json!({"theme":"default", "look":"neo", "themeVariables":variables}),
            None,
        );
        let document = Document::parse(&svg).unwrap();
        let lane = lane(&document);
        assert_eq!(lane.attribute("data-look"), Some("neo"));
        assert_eq!(lane.attribute("data-color-id"), None);
        assert!(stylesheet(&document).contains(&format!(
            ".swimlane.cluster:not([data-color-id]) rect{{stroke:{expected}!important;}}"
        )));
        for class in ["swimlane-body", "swimlane-title"] {
            assert_eq!(lane_rect(lane, class).attribute("stroke"), Some(expected));
        }
    }
}

#[test]
fn swimlane_neo_palette_rules_and_authored_strokes_target_both_lane_rectangles() {
    for authored in [
        "style Lane stroke:#22c55e\n",
        "classDef source stroke:#22c55e\nclass Lane source\n",
    ] {
        for selected in ["default", "redux-color"] {
            let svg = render(
                &format!("{SOURCE}{authored}"),
                json!({"theme":selected, "look":"neo"}),
                None,
            );
            let document = Document::parse(&svg).unwrap();
            let lane = lane(&document);
            let color_slot = (selected == "redux-color").then_some("color-0");
            assert_eq!(lane.attribute("data-color-id"), color_slot);
            for class in ["swimlane-body", "swimlane-title"] {
                if let Some(slot) = color_slot {
                    assert!(stylesheet(&document).contains(&format!(
                        r#"[data-look="neo"][data-color-id="{slot}"].swimlane.cluster rect.{class}"#
                    )));
                }
                let style = lane_rect(lane, class).attribute("style").unwrap();
                assert!(
                    style.contains("stroke:#22c55e !important"),
                    "{selected}/{class}: {style}"
                );
            }
        }
    }
}

#[test]
fn native_theme_v2_swimlane_layers_container_and_title_paints_beneath_authored_stroke() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.container":"#ABCDEF","surface.emphasis":"#234567",
                         "stroke.guide":"#56789A","stroke.border":"#102030"}
            }"##,
            None,
        )
        .unwrap();
    let native = NativeTheme::new(theme);
    for (authored, has_override) in [
        ("", false),
        ("classDef source stroke:#22c55e\nclass Lane source\n", true),
    ] {
        let svg = render(
            &format!("{SOURCE}{authored}"),
            json!({"theme":"default", "look":"neo"}),
            Some(native.clone()),
        );
        let document = Document::parse(&svg).unwrap();
        let lane = lane(&document);
        let css = stylesheet(&document);
        assert!(!css.contains(".swimlane.cluster:not([data-color-id]) rect"));
        for (class, fill) in [("swimlane-body", "#ABCDEF"), ("swimlane-title", "#234567")] {
            assert!(css.contains(&format!(
                ".swimlane.cluster rect.{class}{{fill:{fill};stroke:#56789A}}"
            )));
            let style = lane_rect(lane, class).attribute("style").unwrap();
            assert_eq!(
                style.contains("stroke:#22c55e !important"),
                has_override,
                "{class}: {style}",
            );
        }
    }
}
