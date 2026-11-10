#![cfg(feature = "diagram-kanban")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::KanbanDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn render(word: &str, section_width: f64) -> (KanbanDiagramLayout, String) {
    let source = format!(
        "kanban\n col[{word}]\n  task[{word}]@{{ ticket: '{word}', assigned: '{word}' }}\n"
    );
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({
                "fontFamily": "Arial",
                "themeVariables": {"fontFamily": "Arial", "fontSize": "14px"},
                "kanban": {"sectionWidth": section_width}
            })),
        )
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let layout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["KanbanDiagram"].clone())
            .unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    (layout, rendered.svg().to_owned())
}

#[test]
fn kanban_long_words_keep_measured_containers_and_the_original_wrap_width() {
    let word = "W".repeat(60);
    let (layout, svg) = render(&word, 200.0);
    let (_, wide_svg) = render(&word, 2000.0);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let control = roxmltree::Document::parse(&wide_svg).unwrap();
    let labels = |document: &roxmltree::Document<'_>| {
        document
            .descendants()
            .filter(|node| node.has_tag_name("foreignObject"))
            .map(|node| {
                (
                    node.attribute("width").unwrap().parse::<f64>().unwrap(),
                    node.descendants()
                        .find(|child| child.has_tag_name("div"))
                        .unwrap()
                        .attribute("style")
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    let actual = labels(&document);
    let expected = labels(&control);
    assert_eq!(actual.len(), 4, "column, title, ticket and assignee");
    for (index, ((width, style), (unwrapped_width, _))) in actual.iter().zip(&expected).enumerate()
    {
        let cap = if index == 0 { 200 } else { 175 };
        assert!(*width > f64::from(cap));
        assert!((width - unwrapped_width).abs() < 0.02);
        assert!(style.contains("white-space: break-spaces"));
        assert!(
            style
                .split(';')
                .any(|entry| entry.trim() == format!("width: {cap}px"))
        );
    }
    assert_eq!(layout.sections[0].width, 200.0);
    assert_eq!(layout.items[0].width, 185.0);
    let bounds = layout.bounds.unwrap();
    assert!(bounds.min_x <= layout.sections[0].center_x - actual[0].0 / 2.0);
    let item = &layout.items[0];
    assert!(bounds.max_x >= item.center_x - item.width / 2.0 + 10.0 + actual[1].0);
    assert!(bounds.min_x <= item.center_x + item.width / 2.0 - 10.0 - actual[3].0);
}
