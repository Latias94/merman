mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
    RenderEnvironment, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::IshikawaDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use std::sync::{Arc, Mutex};

const DEEP_ISHIKAWA_RENDER_DEPTH: usize = 1_200;

fn ishikawa_theme_source(look: &str) -> String {
    let frontmatter = if look == "handDrawn" {
        "---\nconfig:\n  look: handDrawn\n  handDrawnSeed: 7\n---\n"
    } else {
        ""
    };
    format!(
        "{frontmatter}ishikawa-beta\n Root cause\n  Process\n   Slow step\n   Missing spec\n  People\n   Missing owner\n"
    )
}

#[test]
fn native_theme_v2_ishikawa_separates_head_and_bones_in_both_looks() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.emphasis":"#305070","text.on_emphasis":"#FFFFFF",
                         "surface.node":"#E0F0D0","stroke.connector":"#506070",
                         "stroke.border":"#708090","text.primary":"#102030"},
                "typography":{"sans":["Fishbone Body"],"serif":["Head Serif"],
                              "title_family":"serif","base_size_px":21,"title_weight":700}
            }"##,
            None,
        )
        .unwrap();
    for look in ["classic", "handDrawn"] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                &ishikawa_theme_source(look),
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let prepared = family::prepare(
            parsed.presentation(),
            &LayoutOptions::default(),
            RenderEnvironment::deterministic()
                .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
                .unwrap(),
        )
        .unwrap();
        let projection = prepared.layout_json().unwrap();
        let layout: IshikawaDiagramLayout =
            serde_json::from_value(projection["layout"]["IshikawaDiagram"].clone()).unwrap();
        assert_eq!(layout.font_size, 21.0);
        let rendered = prepared
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let dom = roxmltree::Document::parse(rendered.svg()).unwrap();
        let css = dom
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(
            css.contains(
                "text { font-family: \"Fishbone Body\"; font-size: 21px; fill: #102030; }"
            )
        );
        assert!(css.contains(".ishikawa-head-label { font-weight: 700;"));
        assert!(css.contains("font-family: \"Head Serif\"; fill: #FFFFFF;"));
        assert!(css.contains(".ishikawa-head { fill: #305070; stroke: #708090;"));
        assert!(css.contains(".ishikawa-label-box { fill: #E0F0D0; stroke: #708090;"));
        assert!(css.contains(".ishikawa-sub-branch { stroke: #708090; }"));
        if look == "handDrawn" {
            let shape = |class| {
                dom.descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .unwrap()
            };
            assert!(shape("ishikawa-head").children().any(|node| {
                node.has_tag_name("path") && node.attribute("fill") == Some("#305070")
            }));
            assert!(shape("ishikawa-branch").children().any(|node| {
                node.has_tag_name("path") && node.attribute("stroke") == Some("#506070")
            }));
        }
    }
}

fn ishikawa_stylesheet(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .expect("valid Ishikawa SVG")
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Ishikawa stylesheet")
        .to_string()
}

fn try_render_ishikawa_svg_with_resource_policy(
    source: &str,
    policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Ishikawa resource-bound fixture")
        .expect("detect Ishikawa diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    Ok(artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("ishikawa-bounded".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )?
        .svg()
        .to_owned())
}

#[test]
fn ishikawa_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source =
        "ishikawa-beta\n Root cause\n  Process\n   Slow step\n  People\n   Missing owner\n";
    let baseline = try_render_ishikawa_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render unbounded Ishikawa SVG");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1);

    let exact = try_render_ishikawa_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .expect("valid exact Ishikawa SVG ceiling"),
    )
    .expect("exact Ishikawa SVG ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let error = try_render_ishikawa_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .expect("valid below-exact Ishikawa SVG ceiling"),
    )
    .expect_err("one byte below the Ishikawa SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Ishikawa MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
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
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
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
        .unwrap()
        .into_presentation();
    assert_eq!(parsed.metadata().diagram_type, "ishikawa");

    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let ishikawa_layout: IshikawaDiagramLayout =
        serde_json::from_value(projection["layout"]["IshikawaDiagram"].clone()).unwrap();
    assert_eq!(ishikawa_layout.font_size, 18.0);
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
    // Mermaid keeps root `fontSize` for deterministic layout but uses the selected
    // theme variable (16px here) for the diagram stylesheet.
    assert!(svg.contains(r##".ishikawa text { font-family: "trebuchet ms",verdana,arial,sans-serif; font-size: 16px;"##));
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

    let head_layout = ishikawa_layout.head.as_ref().expect("Ishikawa head layout");
    let head_label = diagram_group
        .descendants()
        .find(|node| node.is_element() && node.attribute("class") == Some("ishikawa-head-label"))
        .expect("Ishikawa head label");
    assert_eq!(head_label.attribute("text-anchor"), Some("start"));
    assert_eq!(head_label.attribute("x"), Some("0"));
    let transform = head_label
        .attribute("transform")
        .and_then(|value| value.strip_prefix("translate("))
        .and_then(|value| value.strip_suffix(')'))
        .expect("head label translate transform")
        .split(',')
        .map(|value| value.parse::<f64>().expect("numeric translate component"))
        .collect::<Vec<_>>();
    assert_eq!(transform.len(), 2);
    let text_width = head_layout.label.bbox.max_x - head_layout.label.bbox.min_x;
    let text_height = head_layout.label.bbox.max_y - head_layout.label.bbox.min_y;
    let local_bbox_x = head_layout.label.bbox.min_x - head_layout.label.x;
    let local_bbox_y = head_layout.label.bbox.min_y - head_layout.label.y;
    let expected_x = (head_layout.width - text_width) / 2.0 - local_bbox_x + 3.0;
    let expected_y = -local_bbox_y - text_height / 2.0;
    assert!(
        (transform[0] - expected_x).abs() < 1e-9 && (transform[1] - expected_y).abs() < 1e-9,
        "head label must use Mermaid's local getBBox transform: {transform:?}"
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
fn ishikawa_hand_drawn_generation_consumes_the_operation_work_budget() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 20_000)
        .unwrap();
    // Large rendered letters produce a large hatched label box. Root fontSize alone only
    // changes spacing, so it must not be used as a proxy for expensive fill geometry.
    let source = "---\nconfig:\n  look: classic\n  handDrawnSeed: 1\n  fontSize: 512\n  themeVariables:\n    fontSize: 512px\n---\nishikawa-beta\n  Effect\n  Cause\n    Detail\n";
    try_render_ishikawa_svg_with_resource_policy(source, policy)
        .expect("the small model and classic geometry fit the work budget");
    let hand_drawn = source.replace("look: classic", "look: handDrawn");
    let unbounded = try_render_ishikawa_svg_with_resource_policy(
        &hand_drawn,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("the same hand-drawn fixture renders without the work ceiling");
    let document = roxmltree::Document::parse(&unbounded).unwrap();
    let label_box = document
        .descendants()
        .find(|node| node.attribute("class") == Some("ishikawa-label-box"))
        .unwrap();
    assert!(label_box.children().any(|node| node.has_tag_name("path")
        && node.attribute("stroke-width") == Some("2.5")
        && node.attribute("fill") == Some("none")
        && node.attribute("d").is_some_and(|path| !path.is_empty())));

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&hand_drawn, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("the hand-drawn model and label layout fit the same work ceiling");
    let projection = artifact.layout_json().unwrap();
    assert_eq!(
        projection["layout"]["IshikawaDiagram"]["pairs"][0]["upper"]["label_group"]["label"]["font_size"],
        512.0
    );
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("hachure generation must consume the remaining operation work budget"),
        Err(error) => error,
    };
    let merman_render::Error::ResourceLimitExceeded(details) = error else {
        panic!("expected generation work rejection, got {error}");
    };
    assert_eq!(details.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
    assert_eq!(details.cause, ResourceLimitCause::Ceiling);
    assert_eq!(details.phase, ResourceLimitPhase::LayoutModel);
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
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let parsed = legacy_init_theme_compat_engine()
            .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
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
fn ishikawa_root_spacing_uses_integer_prefix_independently_of_terminal_css_size() {
    let mut reference = None;
    for root_size in ["24px", "24pt"] {
        let source = "ishikawa-beta\n    Problem\n    Cause\n        first<br/>second\n";
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "fontSize": root_size,
                        "themeVariables": {"fontSize": "32px"}
                    }),
                ),
            )
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let layout = artifact.layout_json().unwrap()["layout"]["IshikawaDiagram"].clone();
        assert_eq!(layout["font_size"], 24.0, "{root_size}");
        let detail = &layout["pairs"][0]["upper"]["sub_groups"][0]["label"];
        assert_eq!(detail["lines"], serde_json::json!(["first", "second"]));
        assert_eq!(detail["font_size"], 32.0);
        assert_eq!(detail["line_height"], 24.0 * 1.05);
        if let Some(reference) = &reference {
            assert_eq!(&layout, reference, "{root_size}");
        } else {
            reference = Some(layout);
        }
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let text = document
            .descendants()
            .find(|node| {
                node.has_tag_name("text")
                    && node
                        .descendants()
                        .any(|child| child.text() == Some("first"))
            })
            .unwrap();
        let second = text
            .children()
            .filter(|node| node.has_tag_name("tspan"))
            .nth(1)
            .unwrap();
        let line_step = second.attribute("dy").unwrap().parse::<f64>().unwrap();
        assert!((line_step - 24.0 * 1.05).abs() < 1e-10);
    }
}

#[derive(Default)]
struct IshikawaFontProbe(Mutex<Vec<(String, merman_render::text::TextStyle)>>);

impl HostTextMeasurer for IshikawaFontProbe {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.0
            .lock()
            .unwrap()
            .push((request.text.to_owned(), request.style.clone()));
        Ok(None)
    }
}

#[test]
fn ishikawa_final_fonts_size_labels_boxes_spine_and_canvas_in_both_looks() {
    for look in ["classic", "handDrawn"] {
        let mut reference = None;
        for size in ["24pt", "32px", "2pc"] {
            let config = serde_json::json!({
                "look": look, "handDrawnSeed": 7, "fontSize": "16px",
                "themeVariables": {"fontSize": size, "fontFamily": "IshikawaFinal"}
            });
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            );
            let source = "ishikawa-beta\n    Problem\n    WWWWWWWWWWWW\n        WWWWWWWWWWWW\n        Another cause\n";
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let host = Arc::new(IshikawaFontProbe::default());
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                    TextMeasurementProfileIdentity::new(
                        MeasurementProfileId::new("test.ishikawa-final-fonts").unwrap(),
                        "1",
                    )
                    .unwrap(),
                    host.clone(),
                    TextMeasurementPhase::ALL,
                ))
                .begin_session()
                .unwrap();
            let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
            let projection = artifact.layout_json().unwrap()["layout"]["IshikawaDiagram"].clone();
            if let Some(reference) = &reference {
                assert_eq!(&projection, reference, "{look}/{size}");
            } else {
                reference = Some(projection.clone());
            }
            let layout: IshikawaDiagramLayout = serde_json::from_value(projection).unwrap();
            let bounds = layout.bounds.as_ref().unwrap();
            let branch = &layout.pairs[0].upper;
            let label = &branch.label_group.label;
            assert_eq!(label.font_size, 32.0);
            assert_eq!(layout.head.as_ref().unwrap().label.font_size, 14.0);
            assert_eq!(label.line_height, layout.font_size * 1.05);
            let label_box = &branch.label_group.label_box;
            assert!(label_box.x <= label.bbox.min_x && label_box.y <= label.bbox.min_y);
            assert!(label_box.x + label_box.width >= label.bbox.max_x);
            assert!(label_box.y + label_box.height >= label.bbox.max_y);
            let minimum_x = std::iter::once(label)
                .chain(branch.sub_groups.iter().map(|group| &group.label))
                .map(|label| label.bbox.min_x)
                .fold(f64::INFINITY, f64::min);
            assert_eq!(layout.spine.as_ref().unwrap().x1, minimum_x);
            for text in
                std::iter::once(label).chain(branch.sub_groups.iter().map(|group| &group.label))
            {
                assert!(text.bbox.min_x >= bounds.min_x && text.bbox.max_x <= bounds.max_x);
                assert!(text.bbox.min_y >= bounds.min_y && text.bbox.max_y <= bounds.max_y);
            }
            let requests = host.0.lock().unwrap();
            assert!(requests.iter().any(|(text, _)| text == "Another cause"));
            for (text, style) in requests.iter() {
                assert_eq!(style.font_family.as_deref(), Some("IshikawaFinal"));
                assert_eq!(
                    style.font_size,
                    if text == "Problem" { 14.0 } else { 32.0 },
                    "{look}/{size}/{text}"
                );
                if text == "Problem" {
                    assert_eq!(style.font_weight.as_deref(), Some("600"));
                }
            }
            drop(requests);
            let rendered = artifact
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some("ishikawa-final-fonts".to_string()),
                        ..Default::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap();
            let stylesheet = ishikawa_stylesheet(rendered.svg());
            assert!(stylesheet.contains(&format!("font-size: {size};")));
            assert!(stylesheet.contains("font-family: IshikawaFinal;"));
        }
    }
}

#[test]
fn ishikawa_ignored_or_inherited_sizes_do_not_reuse_the_bone_spacing_size() {
    for size in [
        "-1px",
        "potato",
        "24ptpx",
        "inherit",
        "inherit !important",
        "-1px !important",
    ] {
        for look in ["classic", "handDrawn"] {
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "fontSize": "24pt", "look": look, "handDrawnSeed": 7,
                        "themeVariables": {"fontSize": size}
                    }),
                ),
            );
            let source = "ishikawa-beta\n Problem\n Cause\n  Detail\n";
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
            let layout: IshikawaDiagramLayout = serde_json::from_value(
                artifact.layout_json().unwrap()["layout"]["IshikawaDiagram"].clone(),
            )
            .unwrap();
            assert_eq!(
                layout.font_size, 24.0,
                "{size}/{look}: spacing remains source-owned"
            );
            assert_eq!(layout.pairs[0].upper.label_group.label.font_size, 16.0);
            assert_eq!(layout.pairs[0].upper.sub_groups[0].label.font_size, 16.0);
            let head_size = if size == "inherit !important" {
                16.0
            } else {
                14.0
            };
            assert_eq!(layout.head.as_ref().unwrap().label.font_size, head_size);
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            assert!(ishikawa_stylesheet(rendered.svg()).contains(&format!("font-size: {size};")));
        }
    }
}

#[test]
fn ishikawa_painter_size_admission_preserves_css_and_the_independent_head() {
    for (size, important, resolved_px) in [
        ("0px", false, Some(0.0)),
        ("0px !important", true, Some(0.0)),
        ("24pt", false, Some(32.0)),
        ("24pt !important", true, Some(32.0)),
        ("1e-50px", false, None),
        ("1e-50px !important", true, None),
        // A subnormal coefficient can convert to normal pixels, or normal to subnormal.
        ("1e-39in !important", true, None),
        ("1.2e-38q !important", true, None),
        ("1e-400px", false, None),
        ("1e-400px !important", true, None),
    ] {
        for has_body in [false, true] {
            let source = if has_body {
                "ishikawa-beta\n Problem\n Cause\n  Detail\n"
            } else {
                "ishikawa-beta\n Problem\n"
            };
            let engine = || {
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({
                            "fontSize": "24pt", "themeVariables": {"fontSize": size}
                        }),
                    ),
                )
            };
            let parsed = engine()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
            let layout: IshikawaDiagramLayout = serde_json::from_value(
                artifact.layout_json().unwrap()["layout"]["IshikawaDiagram"].clone(),
            )
            .unwrap();
            assert_eq!(layout.font_size, 24.0);
            let expected_head = if important {
                resolved_px.unwrap_or(16.0)
            } else {
                14.0
            };
            assert_eq!(
                layout.head.as_ref().unwrap().label.font_size,
                expected_head,
                "{size}/{has_body}"
            );
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let stylesheet = ishikawa_stylesheet(rendered.svg());
            assert!(stylesheet.contains(&format!("font-size: {size};")));
            assert!(stylesheet.contains("font-weight: 600;"));
        }
    }
}

#[test]
fn ishikawa_deep_hierarchy_projects_and_renders_flat_model() {
    let input = deep_ishikawa_source(DEEP_ISHIKAWA_RENDER_DEPTH);
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();

    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("flat deep Ishikawa layout");
    let projection = artifact
        .layout_json()
        .expect("managed deep semantic projection");
    assert_eq!(
        projection["semantic"]["nodes"].as_array().unwrap().len(),
        DEEP_ISHIKAWA_RENDER_DEPTH + 1
    );
    let mut exported = Vec::new();
    projection.write_json(&mut exported).unwrap();
    assert!(serde_json::to_vec(&projection).is_err());
    drop(projection.clone());
    drop(projection);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("deep Ishikawa SVG");
    roxmltree::Document::parse(rendered.svg()).expect("well-formed deep Ishikawa SVG");
}
