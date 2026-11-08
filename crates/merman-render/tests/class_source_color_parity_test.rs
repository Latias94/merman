use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemePortabilityRequirement, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};

const LAYOUTS: &[&str] = if cfg!(feature = "layout-elk") {
    &["dagre", "elk"]
} else {
    &["dagre"]
};

fn render_class(
    source_styles: &str,
    layout: &str,
    html_labels: bool,
    spec: DiagramThemeSpec,
) -> String {
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .expect("compile Class source-color theme");
    let style_line = if source_styles.is_empty() {
        String::new()
    } else {
        format!("style A {source_styles}\n")
    };
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: classic\n  htmlLabels: {html_labels}\n---\nclassDiagram\nclass A\nclass B\n{style_line}"
    );
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Class source-color control")
        .expect("detect Class diagram")
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let node = model.class("A").expect("source-colored Class node");
    assert_eq!(
        node.styles.iter().map(String::as_str).collect::<Vec<_>>(),
        source_styles
            .split(',')
            .filter(|style| !style.is_empty())
            .collect::<Vec<_>>(),
        "SVG color selection must preserve the ordered raw style declarations"
    );
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin Class source-color session");
    let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Class source-color control")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Class source-color control");
    let svg = rendered.svg().to_owned();
    assert_eq!(
        merman_render::__private::family_evidence(rendered.into_completion().report())
            .theme_residual_count(),
        0
    );
    svg
}

fn title_terminal<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    id: &str,
    html_labels: bool,
) -> roxmltree::Node<'a, 'input> {
    let prefix = format!("merman-classId-{id}-");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with(&prefix))
        })
        .expect("named Class node");
    let title = node
        .descendants()
        .find(|node| node.attribute("class") == Some("label-group text"))
        .expect("Class title group");
    assert_eq!(
        title
            .descendants()
            .any(|node| node.has_tag_name("foreignObject")),
        html_labels
    );
    title
        .descendants()
        .find(|node| {
            if html_labels {
                node.has_tag_name("span")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "nodeLabel")
                    })
            } else {
                node.has_tag_name("text")
            }
        })
        .expect("visible Class title terminal")
}

#[test]
fn class_source_color_uses_first_svg_match_and_preserves_html_cascade_and_node_isolation() {
    // classBox.ts uses the first raw color for SVG tspans; HTML retains the whole cascade.
    let cases = [
        (
            "color:#ff0000,color:#0000ff",
            "color:#ff0000;fill:#ff0000",
            "color:#ff0000;color:#0000ff",
        ),
        (
            "color:#0000ff,color:#ff0000",
            "color:#0000ff;fill:#0000ff",
            "color:#0000ff;color:#ff0000",
        ),
        (
            "color:#0000ff",
            "color:#0000ff;fill:#0000ff",
            "color:#0000ff",
        ),
        ("", "", ""),
    ];
    for layout in LAYOUTS {
        for html_labels in [false, true] {
            for (source_styles, svg_style, html_style) in cases {
                let svg = render_class(source_styles, layout, html_labels, DiagramThemeSpec::new());
                let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
                let expected = if html_labels { html_style } else { svg_style };
                for (id, expected_style) in [("A", expected), ("B", "")] {
                    let terminal = title_terminal(&document, id, html_labels);
                    assert_eq!(
                        terminal.attribute("style").unwrap_or(""),
                        expected_style,
                        "{layout}, htmlLabels={html_labels}, {source_styles:?}, node={id}"
                    );
                    let text = terminal
                        .descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect::<String>();
                    assert_eq!(text, id);
                }
            }
        }
    }
}

#[test]
fn class_first_svg_source_color_keeps_typed_fill_on_unowned_labels() {
    for layout in LAYOUTS {
        for html_labels in [false, true] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#123abc").expect("typed label color")),
                )),
            );
            let svg = render_class("color:#ff0000,color:#0000ff", layout, html_labels, spec);
            let document = roxmltree::Document::parse(&svg).expect("valid themed Class SVG");
            let source_style = if html_labels {
                "color:#ff0000;color:#0000ff"
            } else {
                "color:#ff0000;fill:#ff0000"
            };
            assert_eq!(
                title_terminal(&document, "A", html_labels).attribute("style"),
                Some(source_style),
                "{layout}, htmlLabels={html_labels}: source-owned label"
            );
            assert_eq!(
                title_terminal(&document, "B", html_labels).attribute("style"),
                Some("color:#123abc !important;fill:#123abc !important"),
                "{layout}, htmlLabels={html_labels}: typed label"
            );
        }
    }
}

#[allow(dead_code)]
mod common;
