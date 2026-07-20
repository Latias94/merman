mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::environment::{RenderEnvironment, TextMeasurementPhase};
use merman_render::family;
use merman_render::ishikawa::layout_ishikawa_diagram_typed;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};

const DEEP_ISHIKAWA_RENDER_DEPTH: usize = 1_200;

struct PreciseIshikawaMeasurer;

impl TextMeasurer for PreciseIshikawaMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: 10.0,
            height: 16.0,
            line_count: 1,
        }
    }

    fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        10.008
    }

    fn measure_svg_text_bbox_x_with_ascii_overhang(
        &self,
        _text: &str,
        _style: &TextStyle,
    ) -> (f64, f64) {
        (5.004, 5.004)
    }
}

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
    let session = RenderEnvironment::parity().begin_session().unwrap();
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
    assert_eq!(parsed.metadata().diagram_type, "ishikawa");

    let ishikawa_layout = {
        let RenderSemanticModel::Ishikawa(model) = parsed.model() else {
            panic!("expected Ishikawa render model");
        };
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        layout_ishikawa_diagram_typed(
            model,
            parsed.metadata().effective_config.as_value(),
            &measurer,
        )
        .unwrap()
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

    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("ishikawa-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_owned();

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
fn ishikawa_hand_drawn_renders_rough_primitives_in_mermaid_dom_order() {
    let render = |seed| {
        let input = format!(
            r##"---
config:
  look: handDrawn
  handDrawnSeed: {seed}
  themeVariables:
    lineColor: '#123456'
    mainBkg: '#abcdef'
---
ishikawa-beta
    Root cause
    Process
        First detail
"##
        );
        let session = RenderEnvironment::parity().begin_session().unwrap();
        let parsed = legacy_init_theme_compat_engine()
            .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
        artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("ishikawa-hand-drawn".to_string()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .unwrap()
            .svg()
            .to_owned()
    };

    let svg = render(7);
    assert_eq!(
        svg,
        render(7),
        "a fixed handDrawnSeed must be deterministic"
    );
    assert_ne!(
        svg,
        render(8),
        "a different handDrawnSeed must change visible rough paths"
    );
    assert!(!svg.contains("<defs>"));
    assert!(!svg.contains("<marker"));
    assert!(!svg.contains("<line"));
    assert!(!svg.contains("<rect"));
    assert!(!svg.contains("marker-start"));

    let document = roxmltree::Document::parse(&svg).expect("valid hand-drawn Ishikawa SVG");
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
            ("g", Some("ishikawa-head-group")),
            ("g", Some("ishikawa-pair")),
            ("g", Some("ishikawa-spine")),
        ]
    );

    let head = diagram_group
        .descendants()
        .find(|node| node.is_element() && node.attribute("class") == Some("ishikawa-head"))
        .expect("rough Ishikawa head");
    assert_eq!(head.tag_name().name(), "g");
    let head_paths = head
        .children()
        .filter(roxmltree::Node::is_element)
        .filter(|node| node.tag_name().name() == "path")
        .collect::<Vec<_>>();
    assert_eq!(head_paths.len(), 2);
    assert_eq!(
        (
            head_paths[0].attribute("stroke"),
            head_paths[0].attribute("stroke-width"),
            head_paths[0].attribute("fill"),
        ),
        (Some("#abcdef"), Some("2.5"), Some("none"))
    );
    assert_eq!(
        (
            head_paths[1].attribute("stroke"),
            head_paths[1].attribute("stroke-width"),
            head_paths[1].attribute("fill"),
        ),
        (Some("#123456"), Some("2"), Some("none"))
    );

    let pair = diagram_group
        .children()
        .find(|node| node.is_element() && node.attribute("class") == Some("ishikawa-pair"))
        .expect("Ishikawa pair group");
    let pair_children = pair
        .children()
        .filter(roxmltree::Node::is_element)
        .map(|node| (node.tag_name().name(), node.attribute("class")))
        .collect::<Vec<_>>();
    assert_eq!(
        pair_children,
        vec![
            ("g", Some("ishikawa-branch")),
            ("g", None),
            ("g", Some("ishikawa-label-group")),
            ("g", Some("ishikawa-sub-group")),
        ]
    );
    let branch = pair
        .children()
        .find(|node| node.is_element() && node.attribute("class") == Some("ishikawa-branch"))
        .expect("rough branch");
    let branch_path = branch
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "path")
        .expect("rough branch path");
    assert_eq!(
        (
            branch_path.attribute("stroke"),
            branch_path.attribute("stroke-width"),
            branch_path.attribute("fill"),
        ),
        (Some("#123456"), Some("2"), Some("none"))
    );
    let branch_arrow = pair
        .children()
        .find(|node| node.is_element() && node.attribute("class").is_none())
        .expect("rough branch arrow");
    let branch_arrow_paths = branch_arrow
        .children()
        .filter(roxmltree::Node::is_element)
        .filter(|node| node.tag_name().name() == "path")
        .collect::<Vec<_>>();
    assert_eq!(branch_arrow_paths.len(), 2);
    assert_eq!(
        (
            branch_arrow_paths[0].attribute("stroke"),
            branch_arrow_paths[0].attribute("stroke-width"),
            branch_arrow_paths[0].attribute("fill"),
        ),
        (Some("none"), Some("0"), Some("#123456"))
    );
    assert_eq!(
        (
            branch_arrow_paths[1].attribute("stroke"),
            branch_arrow_paths[1].attribute("stroke-width"),
            branch_arrow_paths[1].attribute("fill"),
        ),
        (Some("#123456"), Some("1"), Some("none"))
    );

    let label_box = pair
        .descendants()
        .find(|node| node.is_element() && node.attribute("class") == Some("ishikawa-label-box"))
        .expect("rough cause label box");
    assert_eq!(label_box.tag_name().name(), "g");
    let label_box_paths = label_box
        .children()
        .filter(roxmltree::Node::is_element)
        .filter(|node| node.tag_name().name() == "path")
        .collect::<Vec<_>>();
    assert_eq!(label_box_paths.len(), 2);
    assert_eq!(
        (
            label_box_paths[0].attribute("stroke"),
            label_box_paths[0].attribute("stroke-width"),
            label_box_paths[0].attribute("fill"),
        ),
        (Some("#abcdef"), Some("2.5"), Some("none"))
    );
    assert_eq!(
        (
            label_box_paths[1].attribute("stroke"),
            label_box_paths[1].attribute("stroke-width"),
            label_box_paths[1].attribute("fill"),
        ),
        (Some("#123456"), Some("2"), Some("none"))
    );

    let sub_group = pair
        .descendants()
        .find(|node| node.is_element() && node.attribute("class") == Some("ishikawa-sub-group"))
        .expect("Ishikawa sub group");
    let sub_children = sub_group
        .children()
        .filter(roxmltree::Node::is_element)
        .map(|node| (node.tag_name().name(), node.attribute("class")))
        .collect::<Vec<_>>();
    assert_eq!(
        sub_children,
        vec![
            ("g", Some("ishikawa-sub-branch")),
            ("g", None),
            ("text", Some("ishikawa-label align")),
        ]
    );
}

#[test]
fn ishikawa_deep_hierarchy_layout_uses_heap_traversal() {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let input = deep_ishikawa_source(DEEP_ISHIKAWA_RENDER_DEPTH);
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    let RenderSemanticModel::Ishikawa(model) = parsed.model() else {
        panic!("expected Ishikawa render model");
    };
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    let layout = layout_ishikawa_diagram_typed(
        model,
        parsed.metadata().effective_config.as_value(),
        &measurer,
    )
    .unwrap();

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

#[test]
fn ishikawa_end_anchored_labels_preserve_computed_length_precision() {
    let input = "ishikawa-beta\n    Root\n    Cause\n        Detail\n";
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let RenderSemanticModel::Ishikawa(model) = parsed.model() else {
        panic!("expected Ishikawa render model");
    };

    let layout = layout_ishikawa_diagram_typed(
        model,
        parsed.metadata().effective_config.as_value(),
        &PreciseIshikawaMeasurer,
    )
    .unwrap();
    let label = &layout.pairs[0].upper.sub_groups[0].label;
    let width = label.bbox.max_x - label.bbox.min_x;

    assert!((width - 10.008).abs() < 1e-12, "label width = {width}");
}
