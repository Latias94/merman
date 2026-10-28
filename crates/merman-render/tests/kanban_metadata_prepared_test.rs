#![cfg(feature = "diagram-kanban")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render(field: &str, value: &str) -> (serde_json::Value, String) {
    let source = format!(
        "---\nconfig:\n  securityLevel: loose\n  kanban:\n    ticketBaseUrl: https://example.invalid/#TICKET#\n---\nkanban\n root[Assignments]\n  task[Task]@{{ {field}: '{value}' }}\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let item = projection["layout"]["KanbanDiagram"]["items"][0].clone();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    (item, rendered.svg().to_owned())
}

fn metadata<'a>(document: &'a roxmltree::Document<'a>) -> roxmltree::Node<'a, 'a> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .find(|node| {
            node.descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect::<String>()
                .contains("WWWW")
        })
        .expect("prepared metadata foreignObject")
}

fn dimension(node: roxmltree::Node<'_, '_>, key: &str) -> f64 {
    node.attribute(key).unwrap().parse().unwrap()
}

#[test]
fn kanban_metadata_prepared_breaks_match_the_pinned_html_terminal() {
    for field in ["assigned", "ticket"] {
        for spelling in ["<br>", "<br/>", "<BR />"] {
            let raw = format!("WWWW{spelling}WWWW");
            let (item, svg) = render(field, &raw);
            assert_eq!(item[field], raw, "public metadata stays in its source form");
            let document = roxmltree::Document::parse(&svg).expect("valid metadata XHTML");
            let label = metadata(&document);
            assert_eq!(
                label
                    .descendants()
                    .filter(|node| node.has_tag_name("br"))
                    .count(),
                1,
                "{field}: {svg}"
            );
            let visible: String = label
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect();
            assert_eq!(visible, "WWWWWWWW");
            let (_, one_row_svg) = render(field, "WWWW");
            let one_row_document = roxmltree::Document::parse(&one_row_svg).unwrap();
            let one_row = metadata(&one_row_document);
            assert!((dimension(label, "width") - dimension(one_row, "width")).abs() < 0.02);
            assert!((dimension(label, "height") - 2.0 * dimension(one_row, "height")).abs() < 0.02);
            if field == "ticket" {
                assert!(label.ancestors().any(|node| node.has_tag_name("a")));
            }
        }
    }
}

#[test]
fn kanban_metadata_prepared_entities_stay_literal_after_html_consumption() {
    for field in ["assigned", "ticket"] {
        let (_, svg) = render(field, "WWWW&lt;br/&gt;WWWW &amp;");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = metadata(&document);
        assert!(!label.descendants().any(|node| node.has_tag_name("br")));
        let visible: String = label
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(visible, "WWWW<br/>WWWW &");
        let (_, control_svg) = render(field, "WWWW");
        let control = roxmltree::Document::parse(&control_svg).unwrap();
        assert!(dimension(label, "width") > 2.0 * dimension(metadata(&control), "width"));
        assert_eq!(
            dimension(label, "height"),
            dimension(metadata(&control), "height")
        );
    }
}

#[test]
fn kanban_metadata_prepared_sanitization_owns_well_formed_xhtml() {
    for field in ["assigned", "ticket"] {
        for raw in [
            "<b>WWWW</b><br><em>WWWW</em>",
            "<b>WWWW",
            "<span title=\"left > right\">WWWW</span><img src=x onerror=alert(1)>",
        ] {
            let (item, svg) = render(field, raw);
            assert_eq!(item[field], raw);
            let document =
                roxmltree::Document::parse(&svg).expect("balanced, quoted metadata XHTML");
            let label = metadata(&document);
            assert!(!label.descendants().any(|node| {
                node.has_tag_name("script")
                    || node.attributes().any(|attr| attr.name().starts_with("on"))
            }));
        }
    }
}

#[test]
fn kanban_metadata_prepared_wraps_are_used_by_both_card_layout_and_emission() {
    for field in ["assigned", "ticket"] {
        let raw = "WWWW WWWW WWWW WWWW WWWW WWWW WWWW WWWW";
        let (item, svg) = render(field, raw);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = metadata(&document);
        let (control_item, control_svg) = render(field, "WWWW");
        let control = roxmltree::Document::parse(&control_svg).unwrap();
        assert!(dimension(label, "height") > dimension(metadata(&control), "height"));
        assert!(dimension(label, "width") <= item["width"].as_f64().unwrap());
        assert!(item["height"].as_f64().unwrap() > control_item["height"].as_f64().unwrap());
        let div_style = label
            .descendants()
            .find(|node| node.has_tag_name("div"))
            .and_then(|node| node.attribute("style"))
            .unwrap();
        assert!(div_style.contains("white-space: break-spaces"));
    }
}

#[test]
fn kanban_metadata_prepared_html_obeys_the_retained_text_budget() {
    use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "kanban\n root[Todo]\n  task[Task]@{ assigned: 'WWWW<br/>WWWW' }\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    let error = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .err()
        .expect("prepared Kanban HTML must reserve retained bytes");
    let merman_render::Error::ResourceLimitExceeded(error) = error else {
        panic!("expected retained-text resource error: {error:?}");
    };
    assert_eq!(
        error.limit,
        ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
    );
}
