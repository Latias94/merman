mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_layout_svg_parts_for_render_model_with_config};
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};

const DEEP_ISHIKAWA_RENDER_DEPTH: usize = 1_200;

fn deep_ishikawa_source(depth: usize) -> String {
    let mut source = String::from("ishikawa-beta\n Root\n");
    for i in 0..depth {
        source.push_str(&" ".repeat(i + 2));
        source.push_str(&format!("Node {i}\n"));
    }
    source
}

#[test]
fn ishikawa_typed_render_model_outputs_svg() {
    let input = r##"---
config:
  ishikawa:
    diagramPadding: 24
    useMaxWidth: true
  fontSize: '18px'
  themeVariables:
    lineColor: '#008800'
    mainBkg: '#FFFFFF'
    textColor: '#111111'
---
ishikawa-beta
    Blurry Photo
    Process
        Out of focus
        Shutter speed too slow
    User
        Shaky hands
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.meta.diagram_type, "ishikawa");

    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let LayoutDiagram::IshikawaDiagram(ishikawa_layout) = &layout else {
        panic!("expected Ishikawa layout");
    };
    assert!(ishikawa_layout.spine.is_some());
    assert_eq!(ishikawa_layout.pairs.len(), 1);
    assert_eq!(ishikawa_layout.pairs[0].upper.sub_groups.len(), 2);
    assert_eq!(
        ishikawa_layout.pairs[0]
            .lower
            .as_ref()
            .expect("lower branch")
            .sub_groups
            .len(),
        1
    );

    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("ishikawa-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(svg.contains(r#"aria-roledescription="ishikawa""#));
    assert!(svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"max-width:"#));
    assert!(svg.contains(r#"<g/><g class="ishikawa">"#));
    assert!(svg.contains(r#"<g class="ishikawa">"#));
    assert!(svg.contains(r#"class="ishikawa-spine""#));
    assert!(svg.contains(r#"class="ishikawa-branch""#));
    assert!(svg.contains(r#"class="ishikawa-sub-branch""#));
    assert!(svg.contains(r#"class="ishikawa-pair""#));
    assert!(svg.contains(r#"class="ishikawa-label-group""#));
    assert!(svg.contains(r#"class="ishikawa-sub-group""#));
    assert!(svg.contains(r#"class="ishikawa-head""#));
    assert!(svg.contains(r#"class="ishikawa-label-box""#));
    assert!(svg.contains(r#"id="ishikawa-arrow-ishikawa-test""#));
    assert!(svg.contains(r#"font-size: 18px"#));
    assert!(svg.contains(r#"stroke: #008800"#));

    let document = roxmltree::Document::parse(&svg).expect("valid Ishikawa SVG");
    let diagram_group = document
        .descendants()
        .find(|node| node.is_element() && node.attribute("class") == Some("ishikawa"))
        .expect("Ishikawa diagram group");
    let root_children = diagram_group
        .children()
        .filter(roxmltree::Node::is_element)
        .map(|node| (node.tag_name().name(), node.attribute("class")))
        .collect::<Vec<_>>();
    assert_eq!(
        root_children,
        vec![
            ("defs", None),
            ("line", Some("ishikawa-spine")),
            ("g", Some("ishikawa-head-group")),
            ("g", Some("ishikawa-pair")),
        ]
    );

    let pair_group = diagram_group
        .children()
        .find(|node| node.is_element() && node.attribute("class") == Some("ishikawa-pair"))
        .expect("Ishikawa pair group");
    let pair_children = pair_group
        .children()
        .filter(roxmltree::Node::is_element)
        .map(|node| (node.tag_name().name(), node.attribute("class")))
        .collect::<Vec<_>>();
    assert_eq!(
        pair_children,
        vec![
            ("line", Some("ishikawa-branch")),
            ("g", Some("ishikawa-label-group")),
            ("g", Some("ishikawa-sub-group")),
            ("g", Some("ishikawa-sub-group")),
            ("line", Some("ishikawa-branch")),
            ("g", Some("ishikawa-label-group")),
            ("g", Some("ishikawa-sub-group")),
        ]
    );

    for label_group in pair_group
        .children()
        .filter(|node| node.is_element() && node.attribute("class") == Some("ishikawa-label-group"))
    {
        let children = label_group
            .children()
            .filter(roxmltree::Node::is_element)
            .map(|node| (node.tag_name().name(), node.attribute("class")))
            .collect::<Vec<_>>();
        assert_eq!(
            children,
            vec![
                ("rect", Some("ishikawa-label-box")),
                ("text", Some("ishikawa-label cause")),
            ]
        );
    }

    for sub_group in pair_group
        .children()
        .filter(|node| node.is_element() && node.attribute("class") == Some("ishikawa-sub-group"))
    {
        let children = sub_group
            .children()
            .filter(roxmltree::Node::is_element)
            .map(|node| (node.tag_name().name(), node.attribute("class")))
            .collect::<Vec<_>>();
        assert_eq!(
            children,
            vec![
                ("line", Some("ishikawa-sub-branch")),
                ("text", Some("ishikawa-label align")),
            ]
        );
    }
}

#[test]
fn ishikawa_deep_hierarchy_layout_uses_heap_traversal() {
    let input = deep_ishikawa_source(DEEP_ISHIKAWA_RENDER_DEPTH);
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let LayoutDiagram::IshikawaDiagram(layout) = layout else {
        panic!("expected Ishikawa layout");
    };

    assert!(layout.total_width.is_finite());
    assert!(layout.total_height.is_finite());
    assert!(layout.head.is_some());
    let label_count = layout
        .pairs
        .iter()
        .map(|pair| {
            1 + pair.upper.sub_groups.len()
                + pair
                    .lower
                    .as_ref()
                    .map(|branch| 1 + branch.sub_groups.len())
                    .unwrap_or(0)
        })
        .sum::<usize>();
    assert!(label_count >= DEEP_ISHIKAWA_RENDER_DEPTH);
}
