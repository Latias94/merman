#![cfg(feature = "diagram-git-graph")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn render_gitgraph(
    source: &str,
    engine: MermaidEngine,
    diagram_id: &str,
) -> family::RenderedFamilySvg {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse GitGraph")
        .expect("detect GitGraph")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare GitGraph")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render GitGraph SVG")
}

fn gitgraph_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid GitGraph SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect()
}

fn gitgraph_css_rule(stylesheet: &str, selector: &str) -> String {
    let marker = format!("{selector}{{");
    let start = stylesheet
        .find(&marker)
        .unwrap_or_else(|| panic!("missing GitGraph CSS selector `{selector}`: {stylesheet}"))
        + marker.len();
    let end = stylesheet[start..]
        .find('}')
        .unwrap_or_else(|| panic!("unterminated GitGraph CSS selector `{selector}`: {stylesheet}"));
    stylesheet[start..start + end].to_string()
}

fn css_property<'a>(declarations: &'a str, property: &str) -> Option<&'a str> {
    declarations.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        (name == property).then_some(value)
    })
}

fn has_class(node: &roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

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
            let parsed = MermaidEngine::new()
                .with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        site_config.clone(),
                    ),
                )
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .expect("parse GitGraph tag shadow source")
                .expect("detect GitGraph")
                .into_presentation();
            let session = RenderEnvironment::deterministic()
                .begin_session()
                .expect("begin GitGraph tag shadow session");
            let rendered =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
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
                document
                    .descendants()
                    .any(|node| node.has_tag_name("polygon") && has_class(&node, "tag-label-bkg")),
                "{theme_name}/{diagram_id}: the tag CSS must have an actual polygon consumer"
            );
            let stylesheet = gitgraph_stylesheet(rendered.svg());
            let tag_rule = gitgraph_css_rule(&stylesheet, &format!("#{diagram_id} .tag-label-bkg"));
            let filter = css_property(&tag_rule, "filter").expect("tag background filter");
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

#[test]
fn gitgraph_absolute_role_sizes_reserve_equivalent_svg_geometry() {
    let geometry = |svg: &str| {
        roxmltree::Document::parse(svg)
            .unwrap()
            .descendants()
            .filter(|node| node.is_element())
            .map(|node| {
                (
                    node.tag_name().name().to_owned(),
                    node.attributes()
                        .map(|attribute| {
                            (attribute.name().to_owned(), attribute.value().to_owned())
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    for property in ["commitLabelFontSize", "tagLabelFontSize"] {
        let render = |size: &str| {
            render_gitgraph(
                "gitGraph\ncommit id:\"MMMMMMMM\" tag:\"MMMMMMMM\"\n",
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(json!({
                        "gitGraph": {"showBranches": false, "rotateCommitLabel": false},
                        "themeVariables": {property: size}
                    })),
                ),
                "git-absolute-role",
            )
        };
        let expected = render("32px");
        let expected = geometry(expected.svg());
        assert_ne!(
            geometry(render("10px").svg()),
            expected,
            "{property} fixture must distinguish sizes"
        );
        for size in [
            "24pt",
            "24PT",
            r"24p\74",
            "24pt !IMPORTANT",
            r"24p\74 !IMPORTANT",
        ] {
            assert_eq!(geometry(render(size).svg()), expected, "{property}/{size}");
        }
    }
}

#[test]
fn gitgraph_zero_size_labels_preserve_terminal_order_backgrounds_and_tag_holes() {
    for (commit_size, tag_size) in [("0em", "0%"), ("0pt", "0rem"), ("0ex", "0cap")] {
        let rendered = render_gitgraph(
        "gitGraph\n  commit id: \"A\" tag: \"first\" tag: \"second\"\n  commit id: \"B\"\n",
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({
                "gitGraph": {"showBranches": false, "rotateCommitLabel": false},
                "themeVariables": {"commitLabelFontSize": commit_size, "tagLabelFontSize": tag_size, "textColor": "#123456", "commitLabelBackground": "#fedcba"}
            })),
        ),
        "git-zero-formatting",
    );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| (node.attribute("class").unwrap(), node.text().unwrap()))
            .collect();
        assert_eq!(
            labels,
            [
                ("commit-label", "A"),
                ("tag-label", "second"),
                ("tag-label", "first"),
                ("commit-label", "B"),
            ]
        );
        let backgrounds: Vec<_> = document
            .descendants()
            .filter(|node| has_class(node, "commit-label-bkg"))
            .collect();
        assert_eq!(backgrounds.len(), 2);
        for background in backgrounds {
            assert_eq!(background.attribute("width"), Some("4"));
            assert_eq!(background.attribute("height"), Some("4"));
        }
        let holes: Vec<_> = document
            .descendants()
            .filter(|node| has_class(node, "tag-hole"))
            .collect();
        assert_eq!(holes.len(), 2);
        assert!(holes.iter().all(|hole| hole.attribute("r") == Some("1.5")));
        assert_eq!(
            document
                .descendants()
                .filter(|node| has_class(node, "tag-label-bkg"))
                .count(),
            2
        );
        let stylesheet = gitgraph_stylesheet(rendered.svg());
        assert_eq!(
            css_property(
                &gitgraph_css_rule(&stylesheet, "#git-zero-formatting .tag-hole"),
                "fill"
            ),
            Some("#123456")
        );
        assert_eq!(
            css_property(
                &gitgraph_css_rule(&stylesheet, "#git-zero-formatting .commit-label-bkg"),
                "fill"
            ),
            Some("#fedcba")
        );
    }
}

#[allow(dead_code)]
mod common;

#[test]
fn gitgraph_svg_preserves_db_accessibility_text() {
    for (title, description, expected_title, expected_description) in [
        (
            "Accessible <!--a11y-->",
            "First\n<!--a11y-->",
            Some("Accessible "),
            Some("First\n"),
        ),
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            Some("\u{0085}"),
            Some("\u{0085}"),
        ),
        ("<!--a11y-->", "<!--a11y-->", None, None),
    ] {
        let source =
            format!("gitGraph\naccTitle: {title}\naccDescr {{\n{description}\n}}\ncommit\n");
        let rendered = render_gitgraph(&source, MermaidEngine::new(), "gitgraph-a11y");
        let svg = rendered.svg();
        common::assert_svg_accessibility(svg, expected_title, expected_description);
    }
}
