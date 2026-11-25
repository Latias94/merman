#![cfg(feature = "diagram-class")]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};

use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

const SOURCE: &str = "classDiagram\nclass Job\nnote for Job \"Retry on failure\"\n";

const AUTHORED: &str = "#246801";

fn config(html: bool) -> Value {
    json!({"htmlLabels": html, "class": {"htmlLabels": html}})
}

fn render(source: &str, config: Value) -> merman_render::Result<family::RenderedFamilySvg> {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(
        &parsed,
        &merman_render::LayoutOptions::headless_svg_defaults(),
        session,
    )?
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some("class-note-theme".into()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn note_label<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| has_class(*node, "noteLabel"))
        .expect("actual note label")
}

fn assert_note_surface(svg: &str, fill: &str, stroke: &str) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let note = note_label(&document).parent().unwrap();
    let paths = note
        .children()
        .find(|node| has_class(*node, "outer-path"))
        .unwrap()
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2, "{svg}");
    assert_eq!(paths[0].attribute("fill"), Some(fill), "{svg}");
    assert_eq!(paths[1].attribute("stroke"), Some(stroke), "{svg}");
    for path in paths {
        let style = path.attribute("style").unwrap();
        assert!(style.contains(&format!("fill:{fill} !important")), "{svg}");
        assert!(
            style.contains(&format!("stroke:{stroke} !important")),
            "{svg}"
        );
    }
}

#[test]
fn classic_note_keeps_authored_inline_html_color() {
    let source = format!(
        "classDiagram\nclass Job\nnote for Job \"<span style='color:{AUTHORED}'>Retry on failure</span>\"\n"
    );
    let mut config = config(true);
    config["theme"] = json!("default");
    config["look"] = json!("classic");
    let rendered = render(&source, config).unwrap();
    assert_note_surface(rendered.svg(), "#fff5ad", "#aaaa33");
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(note_label(&document).descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains(&format!("color:{AUTHORED}")))
    }));
}

#[test]
fn classic_notes_keep_their_paints_and_markup_in_both_label_modes() {
    for html in [false, true] {
        let mut config = config(html);
        config["theme"] = json!("default");
        config["look"] = json!("classic");
        let rendered = render(SOURCE, config).unwrap();
        assert_note_surface(rendered.svg(), "#fff5ad", "#aaaa33");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label = note_label(&document);
        assert_eq!(
            label
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            html
        );
        let text: String = label
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(text, "Retry on failure");
    }
}
