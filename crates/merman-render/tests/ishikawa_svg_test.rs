mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::IshikawaDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};

const DEEP_ISHIKAWA_RENDER_DEPTH: usize = 1_200;

fn ishikawa_text_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::ISHIKAWA),
                ),
            ),
        )
        .expect("compile Ishikawa text fill theme")
}

fn render_ishikawa_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Ishikawa")
        .expect("detect themed Ishikawa");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Ishikawa session");

    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Ishikawa")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("ishikawa-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Ishikawa")
}

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

fn ishikawa_visible_text_nodes<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .filter(|node| {
            if !node.has_tag_name("text") {
                return false;
            }
            let class = node.attribute("class").unwrap_or_default();
            let is_ishikawa_text = class == "ishikawa-head-label"
                || class
                    .split_ascii_whitespace()
                    .any(|token| token == "ishikawa-label");
            is_ishikawa_text
                && node
                    .descendants()
                    .filter(roxmltree::Node::is_text)
                    .filter_map(|descendant| descendant.text())
                    .any(|text| !text.trim().is_empty())
        })
        .collect()
}

fn try_render_ishikawa_svg_with_resource_policy(
    source: &str,
    policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Ishikawa resource-bound fixture")
        .expect("detect Ishikawa diagram");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
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
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "ishikawa");

    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let ishikawa_layout: IshikawaDiagramLayout =
        serde_json::from_value(projection["layout"]["IshikawaDiagram"].clone()).unwrap();
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
fn ishikawa_text_fill_reaches_every_visible_terminal_in_classic_and_hand_drawn() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Ishikawa text fill"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for look in ["classic", "handDrawn"] {
        for (fill, expected_fill) in cases.clone() {
            let theme = ishikawa_text_fill_theme(fill);
            let rendered =
                render_ishikawa_with_theme(&ishikawa_theme_source(look), &theme, Engine::new());
            let document =
                roxmltree::Document::parse(rendered.svg()).expect("valid themed Ishikawa SVG");
            let text_nodes = ishikawa_visible_text_nodes(&document);
            assert_eq!(
                text_nodes.len(),
                6,
                "{look} visible Ishikawa text terminals"
            );

            let visible_text = text_nodes
                .iter()
                .map(|node| {
                    node.descendants()
                        .filter(roxmltree::Node::is_text)
                        .filter_map(|descendant| descendant.text())
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                visible_text,
                vec![
                    "Root cause",
                    "Process",
                    "Missing spec",
                    "Slow step",
                    "People",
                    "Missing owner",
                ],
                "{look} upper-branch descendants must follow Mermaid's reversed terminal order"
            );
            for node in text_nodes {
                let style = node.attribute("style").unwrap_or_default();
                assert!(
                    style.contains(&format!("fill:{expected_fill} !important;")),
                    "{look} typed fill must own every terminal Ishikawa text node: {style}"
                );
            }

            drop(document);
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{look}");
            assert_eq!(evidence.applied_count(), 1, "{look}");
            assert_eq!(evidence.not_applicable_count(), 0, "{look}");
            assert_eq!(evidence.theme_residual_count(), 0, "{look}");
        }
    }
}

#[test]
fn ishikawa_explicit_text_color_outranks_typed_text_fill() {
    let theme = ishikawa_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Ishikawa typed text fill"),
    );

    for look in ["classic", "handDrawn"] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(
            serde_json::json!({ "themeVariables": { "textColor": "#fedcba" } }),
        ));
        let rendered = render_ishikawa_with_theme(&ishikawa_theme_source(look), &theme, engine);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid owned Ishikawa SVG");
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Ishikawa stylesheet");
        assert!(
            stylesheet.contains("fill: #fedcba;"),
            "{look} explicit Mermaid textColor must remain the stylesheet owner: {stylesheet}"
        );
        for node in ishikawa_visible_text_nodes(&document) {
            assert!(
                !node
                    .attribute("style")
                    .unwrap_or_default()
                    .contains("#123456"),
                "{look} typed fill must yield to explicit themeVariables.textColor"
            );
        }

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{look}");
        assert_eq!(evidence.applied_count(), 0, "{look}");
        assert_eq!(evidence.not_applicable_count(), 1, "{look}");
        assert_eq!(evidence.theme_residual_count(), 0, "{look}");
    }
}

#[test]
fn ishikawa_text_fill_is_not_applicable_without_visible_text() {
    let theme = ishikawa_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid empty Ishikawa text fill"),
    );
    let rendered = render_ishikawa_with_theme("ishikawa-beta\n", &theme, Engine::new());
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid empty themed Ishikawa SVG");
    assert!(ishikawa_visible_text_nodes(&document).is_empty());

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn ishikawa_deep_hierarchy_is_rejected_before_recursive_projection() {
    let input = deep_ishikawa_source(DEEP_ISHIKAWA_RENDER_DEPTH);
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    let error = match family::prepare(parsed, &LayoutOptions::default(), session) {
        Ok(_) => panic!("deep recursive Ishikawa model must be rejected before projection"),
        Err(error) => error,
    };
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected typed resource-limit error, got {error}");
    };

    assert_eq!(limit.limit, "typed_model_tree_depth");
    assert_eq!(limit.actual, DEEP_ISHIKAWA_RENDER_DEPTH);
    assert!(limit.actual > limit.max);
}
