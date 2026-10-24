#![cfg(all(feature = "math", feature = "diagram-flowchart"))]

use kurbo::{BezPath, Shape};
use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};

fn node_label<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    node_id: &str,
) -> roxmltree::Node<'a, 'input> {
    let id_fragment = format!("-flowchart-{node_id}-");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains(&id_fragment))
        })
        .unwrap_or_else(|| panic!("rendered flowchart node {node_id}"));
    node.children()
        .find(|child| {
            child.has_tag_name("g")
                && child
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "label"))
        })
        .unwrap_or_else(|| panic!("label group for flowchart node {node_id}"))
}

fn assert_math_label_visible(label: &str, html_labels: bool, resvg_safe: bool) -> String {
    let context = format!("label={label}, htmlLabels={html_labels}, resvg_safe={resvg_safe}");
    let source = format!(
        r#"---
config:
  htmlLabels: {html_labels}
  flowchart:
    htmlLabels: {html_labels}
---
flowchart LR
A["{label}"] --> B[Done]
"#
    );
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse math flowchart")
        .expect("detect math flowchart");
    let environment = RenderEnvironment::deterministic().with_compiled_math_renderer();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        environment
            .begin_session()
            .expect("begin preparation session"),
    )
    .expect("prepare math flowchart");
    let parity = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("math-native".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render math flowchart")
        .svg()
        .to_string();
    let pipeline = if resvg_safe {
        SvgPipeline::resvg_safe()
    } else {
        SvgPipeline::parity()
    };
    let svg = pipeline
        .process_to_string(
            &parity,
            &environment.begin_session().expect("begin pipeline session"),
        )
        .expect("process math flowchart SVG");
    let document = roxmltree::Document::parse(&svg).expect("valid processed math SVG");
    let reference = roxmltree::Document::parse(&parity).expect("valid parity math SVG");
    assert!(reference.root_element().attribute("viewBox").is_some());
    for attribute in ["viewBox", "width", "height"] {
        assert_eq!(
            document.root_element().attribute(attribute),
            reference.root_element().attribute(attribute),
            "{context}: SVG processing must preserve layout {attribute}"
        );
    }
    if html_labels {
        assert!(!svg.contains("$$"), "{context}: raw math delimiters remain");
    }
    if resvg_safe {
        assert!(
            !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            "{context}: resvg-safe output must not depend on HTML labels"
        );
    }

    let ordinary_label = node_label(&document, "B");
    let ordinary_content = ordinary_label
        .descendants()
        .find(|node| node.has_tag_name("use"))
        .map_or(ordinary_label, |instance| {
            let id = instance
                .attribute("href")
                .and_then(|href| href.strip_prefix('#'))
                .expect("same-document normal label fallback");
            document
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .expect("referenced normal label fallback content")
        });
    let ordinary_text: String = ordinary_content
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    assert_eq!(ordinary_text.trim(), "Done", "{context}: normal label text");

    let math_label = node_label(&document, "A");
    if !html_labels {
        // Mermaid 12.1 createText's explicit SVG-text branch does not invoke KaTeX.
        let text: String = math_label
            .descendants()
            .filter_map(|node| node.text())
            .collect();
        assert!(text.contains(label), "{context}: preserve literal SVG text");
        return svg;
    }
    let math_content = math_label
        .descendants()
        .find(|node| node.has_tag_name("use"))
        .map_or(math_label, |instance| {
            let id = instance
                .attribute("href")
                .and_then(|href| href.strip_prefix('#'))
                .unwrap();
            document
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .unwrap()
        });
    let has_formula_geometry = math_content
        .descendants()
        .filter(|node| node.has_tag_name("path"))
        .filter(|node| {
            !node
                .ancestors()
                .take_while(|ancestor| *ancestor != math_content)
                .any(|ancestor| {
                    ["defs", "marker", "clipPath", "mask", "symbol"]
                        .iter()
                        .any(|tag| ancestor.has_tag_name(*tag))
                })
        })
        .any(|path| {
            let path = BezPath::from_svg(path.attribute("d").expect("formula path data"))
                .expect("valid formula path geometry");
            let bounds = path.bounding_box();
            bounds.width().is_finite()
                && bounds.height().is_finite()
                && bounds.width() > 0.0
                && bounds.height() > 0.0
        });
    assert!(
        has_formula_geometry,
        "{context}: formula needs nonempty path geometry inside its label group, not arrow or node-outline paths: {svg}"
    );
    let original_label = node_label(&reference, "A");
    let formula_paths = |root: roxmltree::Node<'_, '_>| {
        root.descendants()
            .filter(|node| node.has_tag_name("path"))
            .map(|node| node.attribute("d").expect("formula path").to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        formula_paths(math_content),
        formula_paths(original_label),
        "{context}: every formula outline must survive SVG processing in order"
    );
    svg
}

#[test]
fn explicit_svg_label_mode_preserves_mermaid_literal_math_in_parity() {
    assert_math_label_visible("$$x^2$$", false, false);
}

#[test]
fn explicit_svg_label_mode_preserves_mermaid_literal_math_in_resvg_safe() {
    assert_math_label_visible("$$x^2$$", false, true);
}

#[test]
fn compiled_math_html_label_is_visible_in_parity() {
    assert_math_label_visible("$$x^2$$", true, false);
}

#[test]
fn compiled_math_html_label_is_visible_in_resvg_safe() {
    assert_math_label_visible("$$x^2$$", true, true);
}

#[test]
fn compiled_math_fraction_root_and_multiline_preserve_all_formula_outlines() {
    for label in [
        r"$$\frac{1}{x+1}$$",
        r"before $$\sqrt{x^2+1}$$ after<br>$$\frac{a}{b}$$",
        r"$$\char32 x$$",
    ] {
        assert_math_label_visible(label, true, true);
    }
}

#[test]
fn empty_html_rows_keep_formula_position_and_viewport_unchanged() {
    let base = assert_math_label_visible("$$x^2$$", true, true);
    let padded = assert_math_label_visible("<br><br><br><br>$$x^2$$", true, true);
    let projection_geometry = |svg: &str| {
        let document = roxmltree::Document::parse(svg).unwrap();
        let formula = document
            .descendants()
            .find(|node| {
                node.has_tag_name("svg")
                    && node.attribute("class") == Some("merman-native-math-svg")
            })
            .expect("projected formula viewport");
        (
            document
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .to_owned(),
            ["x", "y", "width", "height"].map(|key| formula.attribute(key).unwrap().to_owned()),
        )
    };
    assert_eq!(
        projection_geometry(&base),
        projection_geometry(&padded),
        "zero-height HTML blocks must not push visible formula glyphs out of the unchanged viewport"
    );
}
