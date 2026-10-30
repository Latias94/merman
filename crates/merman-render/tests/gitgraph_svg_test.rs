#![cfg(feature = "diagram-git-graph")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

#[test]
fn gitgraph_tag_shadow_references_resolve_and_preserve_custom_filters() {
    for (theme_name, custom_shadow) in [
        ("redux", None),
        ("redux-dark", None),
        ("redux-color", None),
        ("redux-dark-color", None),
        ("redux", Some("drop-shadow(1px 2px 3px rgba(0,0,0,.4))")),
        ("redux", Some("url(#drop-shadow-custom)")),
        (
            "redux",
            Some("url(https://example.com/filters.svg#drop-shadow)"),
        ),
    ] {
        let source = format!(
            "%%{{init: {}}}%%\ngitGraph\n  commit tag: \"v1.0\"\n",
            json!({ "theme": theme_name, "look": "neo" })
        );
        let mut site_config = json!({ "secure": [] });
        if let Some(shadow) = custom_shadow {
            site_config["themeVariables"] = json!({ "dropShadow": shadow });
        }
        for diagram_id in ["git-tag-shadow-a", "git-tag-shadow-b"] {
            let parsed = Engine::new()
                .with_site_config(MermaidConfig::from_value(site_config.clone()))
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .expect("parse GitGraph tag shadow source")
                .expect("detect GitGraph");
            let session = RenderEnvironment::deterministic()
                .begin_session()
                .expect("begin GitGraph tag shadow session");
            let rendered =
                family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .expect("prepare GitGraph tag shadow")
                    .render_svg(
                        &SvgRenderOptions {
                            diagram_id: Some(diagram_id.to_owned()),
                            ..SvgRenderOptions::default()
                        },
                        &SvgDebugOptions::default(),
                    )
                    .expect("render GitGraph tag shadow");
            let effective = &rendered.metadata().effective_config;
            assert_eq!(effective.get_str("theme"), Some(theme_name));
            assert_eq!(effective.get_str("look"), Some("neo"));
            assert_eq!(
                effective.get_str("themeVariables.dropShadow"),
                Some(custom_shadow.unwrap_or("url(#drop-shadow)"))
            );

            let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
            assert_eq!(document.root_element().attribute("id"), Some(diagram_id));
            assert!(
                document.descendants().any(|node| {
                    node.has_tag_name("polygon")
                        && node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_ascii_whitespace()
                                .any(|class| class == "tag-label-bkg")
                        })
                }),
                "{theme_name}/{diagram_id}: the tag CSS must have an actual polygon consumer"
            );
            let stylesheet: String = document
                .descendants()
                .filter(|node| node.has_tag_name("style"))
                .filter_map(|node| node.text())
                .collect();
            let selector = format!("#{diagram_id} .tag-label-bkg{{");
            let tag_rule = stylesheet
                .split_once(&selector)
                .expect("tag background CSS selector")
                .1
                .split_once('}')
                .expect("tag background CSS declarations")
                .0;
            let filter = tag_rule
                .split(';')
                .find_map(|declaration| declaration.strip_prefix("filter:"))
                .expect("tag background filter");
            let expected = custom_shadow
                .map(str::to_owned)
                .unwrap_or_else(|| format!("url(#{diagram_id}-drop-shadow)"));
            assert_eq!(filter, expected, "{theme_name}/{diagram_id}");

            if custom_shadow.is_none() {
                let filter_id = filter
                    .strip_prefix("url(#")
                    .and_then(|value| value.strip_suffix(')'))
                    .expect("local tag shadow reference");
                let definition = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("filter") && node.attribute("id") == Some(filter_id)
                    })
                    .expect("the tag shadow reference must resolve to an emitted filter");
                assert!(
                    definition
                        .parent()
                        .is_some_and(|node| node.has_tag_name("defs"))
                );
                assert!(
                    definition
                        .children()
                        .any(|node| node.has_tag_name("feDropShadow"))
                );
                assert!(!stylesheet.contains("url(#drop-shadow)"));
            }
        }
    }
}
