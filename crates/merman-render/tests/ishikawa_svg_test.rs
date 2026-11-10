mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, InsetsPx,
    OrdinalPalette, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
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
use merman_render::{DiagramFamilyId, LayoutOptions};
use std::sync::{Arc, Mutex};

const DEEP_ISHIKAWA_RENDER_DEPTH: usize = 1_200;

fn ishikawa_core_paint_theme(transparent: bool, explicit_default: bool) -> DiagramTheme {
    let paint = |color| {
        if transparent {
            CanvasPaint::Transparent
        } else {
            CanvasPaint::solid(color).unwrap()
        }
    };
    let rules = [
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(paint("#123456"))
                .with_stroke(paint("#345678")),
        ),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(paint("#56789a")),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(paint("#789abc")),
        ),
        ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_fill(paint("#9abcde")),
        ),
    ];
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(rules.into_iter().fold(
                ThemeRuleSet::default(),
                |rules, rule| {
                    rules.with_rule(if explicit_default {
                        rule.with_variant(merman_render::diagram_theme::ThemeVariant::Default)
                    } else {
                        rule
                    })
                },
            )),
        )
        .unwrap()
}

#[test]
fn ishikawa_core_paints_reach_real_bodies_skeleton_and_marker_with_evidence() {
    for transparent in [false, true] {
        for explicit_default in [false, true] {
            let theme = ishikawa_core_paint_theme(transparent, explicit_default);
            let rendered = render_ishikawa_with_theme(
                &ishikawa_theme_source("classic"),
                &theme,
                MermaidEngine::new(),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            for (class, property, solid) in [
                ("ishikawa-head", "fill", "#123456"),
                ("ishikawa-label-box", "stroke", "#345678"),
                ("ishikawa-arrow", "fill", "#9abcde"),
            ] {
                assert!(
                    document
                        .descendants()
                        .any(|node| node.attribute("class") == Some(class))
                );
                let body = css
                    .split_once(&format!(" .{class} {{"))
                    .unwrap()
                    .1
                    .split('}')
                    .next()
                    .unwrap();
                let value = if transparent { "transparent" } else { solid };
                assert!(body.contains(&format!("{property}: {value};")));
            }
            assert!(document.descendants().any(|node| node.has_tag_name("line")));
            let edge = if transparent {
                "transparent"
            } else {
                "#789abc"
            };
            assert!(css.contains(&format!(
                "{{ stroke: {edge}; stroke-width: 2; fill: none; }}"
            )));
            for text in ishikawa_visible_text_nodes(&document) {
                let value = if transparent {
                    "transparent"
                } else {
                    "#56789a"
                };
                assert_eq!(
                    text.attribute("style"),
                    Some(format!("fill:{value} !important;").as_str())
                );
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 4);
            assert_eq!(evidence.applied_count(), 4);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn ishikawa_config_fill_leaves_typed_border_and_skeleton_independent() {
    let theme = ishikawa_core_paint_theme(false, false);
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "mainBkg": "#de3163", "textColor": "#fedcba" }
        })),
    );
    let rendered = render_ishikawa_with_theme(&ishikawa_theme_source("classic"), &theme, engine);
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(".ishikawa-head { fill: #de3163; stroke: #345678;"));
    assert!(css.contains("{ stroke: #789abc; stroke-width: 2; fill: none; }"));
    assert!(css.contains(".ishikawa-arrow { fill: #9abcde; }"));
    assert!(
        ishikawa_visible_text_nodes(&document)
            .iter()
            .all(|node| node.attribute("style").is_none())
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 3);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn ishikawa_core_paints_reach_hand_drawn_body_and_skeleton_paths() {
    let theme = ishikawa_core_paint_theme(false, false);
    let rendered = render_ishikawa_with_theme(
        &ishikawa_theme_source("handDrawn"),
        &theme,
        MermaidEngine::new(),
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    for body in document.descendants().filter(|node| {
        matches!(
            node.attribute("class"),
            Some("ishikawa-head" | "ishikawa-label-box")
        )
    }) {
        assert!(
            body.children().any(
                |node| node.has_tag_name("path") && node.attribute("stroke") == Some("#123456")
            )
        );
        assert!(
            body.children().any(
                |node| node.has_tag_name("path") && node.attribute("stroke") == Some("#345678")
            )
        );
    }
    for spine in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("ishikawa-spine"))
    {
        assert!(
            spine
                .children()
                .filter(|node| node.has_tag_name("path"))
                .all(|node| node.attribute("stroke") == Some("#789abc"))
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#9abcde"))
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 4);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn ishikawa_hand_drawn_marker_stroke_remains_residual_with_consumed_fill() {
    for mixed in [false, true] {
        let mut style =
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#ff00ff").unwrap());
        if mixed {
            style = style.with_fill(CanvasPaint::solid("#123456").unwrap());
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Marker, style)),
            ))
            .unwrap();
        let source = ishikawa_theme_source("handDrawn");
        let rendered = try_render_ishikawa_with_theme_requirement(
            &source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        if mixed {
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("path")
                        && node.attribute("fill") == Some("#123456"))
            );
        }
        assert!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("path"))
                .all(|node| node.attribute("stroke") != Some("#ff00ff"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        let error = try_render_ishikawa_with_theme_requirement(
            &source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .err()
        .unwrap();
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::ISHIKAWA, 1))
        );
    }
}

#[test]
fn ishikawa_empty_compiled_theme_preserves_unthemed_svg_bytes() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    for look in ["classic", "handDrawn"] {
        let render = |theme: Option<&DiagramTheme>| {
            let engine = match theme {
                Some(theme) => common::install_parse_compatibility(theme, MermaidEngine::new()),
                None => MermaidEngine::new(),
            };
            let parsed = engine
                .parse_diagram_for_render_model_sync(
                    &ishikawa_theme_source(look),
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = match theme {
                Some(theme) => environment.begin_session_with_theme(theme).unwrap(),
                None => environment.begin_session().unwrap(),
            };
            family::prepare(&parsed, &LayoutOptions::default(), session)
                .unwrap()
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some("ishikawa-theme".into()),
                        ..Default::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap()
                .svg()
                .to_owned()
        };
        assert_eq!(
            render(None).as_bytes(),
            render(Some(&theme)).as_bytes(),
            "look={look}"
        );
    }
}

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

fn ishikawa_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::ISHIKAWA, typography),
        ))
        .expect("compile Ishikawa typography theme")
}

fn render_ishikawa_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
) -> family::RenderedFamilySvg {
    try_render_ishikawa_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render themed Ishikawa")
}

fn try_render_ishikawa_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_ishikawa_with_theme_requirement_and_id(
        source,
        theme,
        engine,
        portability,
        "ishikawa-theme",
    )
}

fn try_render_ishikawa_with_theme_requirement_and_id(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Ishikawa")
        .expect("detect themed Ishikawa")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Ishikawa session");

    family::prepare(&parsed, &LayoutOptions::default(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
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

fn ishikawa_stylesheet(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .expect("valid Ishikawa SVG")
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Ishikawa stylesheet")
        .to_string()
}

fn ishikawa_layout_json_with_theme(
    theme: &DiagramTheme,
    engine: MermaidEngine,
) -> serde_json::Value {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(
            &ishikawa_theme_source("classic"),
            ParseOptions::strict(),
        )
        .expect("parse themed Ishikawa layout")
        .expect("detect themed Ishikawa layout")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_theme(theme)
        .expect("begin themed Ishikawa layout session");
    let projection = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Ishikawa layout")
        .layout_json()
        .expect("serialize themed Ishikawa layout");
    projection
        .pointer("/layout/IshikawaDiagram")
        .cloned()
        .expect("Ishikawa layout projection")
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
            let rendered = render_ishikawa_with_theme(
                &ishikawa_theme_source(look),
                &theme,
                MermaidEngine::new(),
            );
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
fn ishikawa_typed_font_stack_reaches_classic_and_hand_drawn_text_receipts() {
    for look in ["classic", "handDrawn"] {
        let font_stack = FontStack::new(["IshikawaTyped", "monospace"])
            .expect("valid Ishikawa typed font stack");
        let expected_font = font_stack.as_css().to_string();
        let theme =
            ishikawa_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
        let rendered =
            render_ishikawa_with_theme(&ishikawa_theme_source(look), &theme, MermaidEngine::new());
        let stylesheet = ishikawa_stylesheet(rendered.svg());

        assert!(
            stylesheet.contains(&format!(".ishikawa text {{ font-family: {expected_font};")),
            "look={look} stylesheet={stylesheet}"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "look={look}");
        assert_eq!(evidence.applied_count(), 1, "look={look}");
        assert_eq!(evidence.not_applicable_count(), 0, "look={look}");
        assert_eq!(evidence.theme_residual_count(), 0, "look={look}");
    }
}

#[test]
fn ishikawa_theme_font_family_keeps_precedence_over_root_font_family() {
    let theme =
        ishikawa_typography_theme(ThemeTextStyle::default().with_font_stack(
            FontStack::single("IshikawaTyped").expect("valid Ishikawa typed font"),
        ));
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "fontFamily": "IshikawaRootFont",
            "themeVariables": { "fontFamily": "IshikawaThemeFont" }
        })),
    );
    let rendered = render_ishikawa_with_theme(&ishikawa_theme_source("classic"), &theme, engine);
    let stylesheet = ishikawa_stylesheet(rendered.svg());

    assert!(stylesheet.contains("font-family: IshikawaThemeFont;"));
    assert!(!stylesheet.contains("IshikawaRootFont"));
    assert!(!stylesheet.contains("IshikawaTyped"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn ishikawa_font_size_is_direct_and_portable_in_classic_and_hand_drawn() {
    let theme = ishikawa_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Ishikawa font size"),
    );

    for look in ["classic", "handDrawn"] {
        let rendered =
            render_ishikawa_with_theme(&ishikawa_theme_source(look), &theme, MermaidEngine::new());

        let stylesheet = ishikawa_stylesheet(rendered.svg());
        let base = stylesheet
            .find(".ishikawa text {")
            .expect("Ishikawa base typography selector");
        let head = stylesheet
            .find(".ishikawa .ishikawa-head-label {")
            .expect("Ishikawa head typography selector");
        assert!(stylesheet.contains("font-size: 24px;"), "{stylesheet}");
        assert!(
            stylesheet[head..].contains("font-size: 14px;"),
            "{stylesheet}"
        );
        assert!(base < head, "head override must follow the base rule");

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "look={look}");
        assert_eq!(evidence.applied_count(), 1, "look={look}");
        assert_eq!(evidence.not_applicable_count(), 0, "look={look}");
        assert_eq!(evidence.theme_residual_count(), 0, "look={look}");
    }
}

#[test]
fn ishikawa_styles_are_scoped_per_inline_svg() {
    let source = ishikawa_theme_source("classic");
    let theme_a = ishikawa_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(12.0)
            .expect("valid Ishikawa font size"),
    );
    let theme_b = ishikawa_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Ishikawa font size"),
    );
    let rendered_a = try_render_ishikawa_with_theme_requirement_and_id(
        &source,
        &theme_a,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
        "ishikawa-a",
    )
    .expect("render first scoped Ishikawa SVG");
    let rendered_b = try_render_ishikawa_with_theme_requirement_and_id(
        &source,
        &theme_b,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
        "ishikawa-b",
    )
    .expect("render second scoped Ishikawa SVG");

    let stylesheet_a = ishikawa_stylesheet(rendered_a.svg());
    let stylesheet_b = ishikawa_stylesheet(rendered_b.svg());
    for (diagram_id, stylesheet, font_size, other_id) in [
        ("ishikawa-a", &stylesheet_a, "12px", "ishikawa-b"),
        ("ishikawa-b", &stylesheet_b, "24px", "ishikawa-a"),
    ] {
        let scope = format!("#{diagram_id} ");
        assert!(stylesheet.contains(&format!("font-size: {font_size};")));
        assert!(!stylesheet.contains(&format!("#{other_id} ")));
        for rule in stylesheet.split_terminator('}') {
            let (selectors, _) = rule
                .split_once('{')
                .unwrap_or_else(|| panic!("malformed Ishikawa CSS rule: {rule:?}"));
            for selector in selectors.split(',') {
                assert!(
                    selector.trim().starts_with(&scope),
                    "unscoped Ishikawa selector {selector:?}: {stylesheet}"
                );
            }
        }
    }
}

#[test]
fn ishikawa_mixed_typography_is_direct_and_portable() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::single("IshikawaMixed").expect("valid mixed Ishikawa font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid mixed Ishikawa font size");
    let theme = ishikawa_typography_theme(typography);
    let rendered = render_ishikawa_with_theme(
        &ishikawa_theme_source("classic"),
        &theme,
        MermaidEngine::new(),
    );
    let stylesheet = ishikawa_stylesheet(rendered.svg());
    assert!(stylesheet.contains("font-family: IshikawaMixed;"));
    assert!(stylesheet.contains("font-size: 24px;"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn ishikawa_explicit_theme_font_size_outranks_typed_size_but_root_size_does_not() {
    let theme = ishikawa_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Ishikawa font size"),
    );
    let source = ishikawa_theme_source("classic");

    let config_owned = render_ishikawa_with_theme(
        &source,
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "fontSize": "22px !important" }
            })),
        ),
    );
    let config_stylesheet = ishikawa_stylesheet(config_owned.svg());
    assert!(
        config_stylesheet.contains("font-size: 22px !important;"),
        "{config_stylesheet}"
    );
    assert!(!config_stylesheet.contains("font-size: 24px;"));
    assert!(config_stylesheet.contains("font-size: 14px;"));
    let evidence =
        merman_render::__private::family_evidence(config_owned.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);

    let source_owned_source =
        format!("%%{{init: {{\"themeVariables\": {{\"fontSize\": \"23px\"}}}} }}%%\n{source}");
    let source_owned = render_ishikawa_with_theme(
        &source_owned_source,
        &theme,
        legacy_init_theme_compat_engine(),
    );
    let source_stylesheet = ishikawa_stylesheet(source_owned.svg());
    assert!(
        source_stylesheet.contains("font-size: 23px;"),
        "{source_stylesheet}"
    );
    assert!(!source_stylesheet.contains("font-size: 24px;"));
    let evidence =
        merman_render::__private::family_evidence(source_owned.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);

    let root_owned = render_ishikawa_with_theme(
        &source,
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "fontSize": "31px"
            })),
        ),
    );
    let root_stylesheet = ishikawa_stylesheet(root_owned.svg());
    assert!(
        root_stylesheet.contains("font-size: 24px;"),
        "{root_stylesheet}"
    );
    assert!(!root_stylesheet.contains("font-size: 31px;"));
    let evidence = merman_render::__private::family_evidence(root_owned.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn ishikawa_typed_font_size_changes_text_geometry_without_changing_root_spacing() {
    let small = ishikawa_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(12.0)
            .expect("valid small Ishikawa font size"),
    );
    let large = ishikawa_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(48.0)
            .expect("valid large Ishikawa font size"),
    );
    let small_layout = ishikawa_layout_json_with_theme(&small, MermaidEngine::new());
    let large_layout = ishikawa_layout_json_with_theme(&large, MermaidEngine::new());
    assert_eq!(small_layout["font_size"], large_layout["font_size"]);
    assert_eq!(small_layout["head"], large_layout["head"]);
    assert_eq!(
        small_layout["pairs"][0]["upper"]["line"],
        large_layout["pairs"][0]["upper"]["line"]
    );
    let small_label = &small_layout["pairs"][0]["upper"]["label_group"];
    let large_label = &large_layout["pairs"][0]["upper"]["label_group"];
    assert_eq!(
        small_label["label"]["line_height"],
        large_label["label"]["line_height"]
    );
    assert_eq!(small_label["label"]["font_size"], 12.0);
    assert_eq!(large_label["label"]["font_size"], 48.0);
    for dimension in ["width", "height"] {
        assert!(
            large_label["label_box"][dimension].as_f64().unwrap()
                > small_label["label_box"][dimension].as_f64().unwrap()
        );
    }
    assert_ne!(small_layout["bounds"], large_layout["bounds"]);
    assert_ne!(small_layout["spine"]["x1"], large_layout["spine"]["x1"]);

    let baseline = ishikawa_typography_theme(ThemeTextStyle::default());
    let root_small = ishikawa_layout_json_with_theme(
        &baseline,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "fontSize": "12px"
            })),
        ),
    );
    let root_large = ishikawa_layout_json_with_theme(
        &baseline,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "fontSize": "48px"
            })),
        ),
    );
    assert_ne!(
        root_small, root_large,
        "root fontSize remains the Ishikawa layout owner"
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
    let theme = ishikawa_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("IshikawaFinal").unwrap())
            .with_font_size_px(72.0)
            .unwrap(),
    );
    for look in ["classic", "handDrawn"] {
        let mut reference = None;
        for size in ["24pt", "32px", "2pc"] {
            let config = serde_json::json!({
                "look": look, "handDrawnSeed": 7, "fontSize": "16px",
                "themeVariables": {"fontSize": size}
            });
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            );
            let source = "ishikawa-beta\n    Problem\n    WWWWWWWWWWWW\n        WWWWWWWWWWWW\n        Another cause\n";
            let parsed = common::install_parse_compatibility(&theme, engine)
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
                .begin_session_with_theme(&theme)
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
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn ishikawa_important_base_size_reaches_the_actual_head_geometry() {
    let theme =
        ishikawa_typography_theme(ThemeTextStyle::default().with_font_size_px(48.0).unwrap());
    let engine = |size| {
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables":{"fontSize":size}
            })),
        )
    };
    let ordinary = ishikawa_layout_json_with_theme(&theme, engine("24pt"));
    let important = ishikawa_layout_json_with_theme(&theme, engine("24pt !important"));
    assert_eq!(ordinary["head"]["label"]["font_size"], 14.0);
    assert_eq!(important["head"]["label"]["font_size"], 32.0);
    assert!(
        important["head"]["width"].as_f64().unwrap() > ordinary["head"]["width"].as_f64().unwrap()
    );
    assert_eq!(ordinary["font_size"], important["font_size"]);
    assert_eq!(
        ordinary["pairs"][0]["upper"]["label_group"]["label"]["font_size"],
        32.0
    );
    assert_eq!(
        important["pairs"][0]["upper"]["label_group"]["label"]["font_size"],
        32.0
    );
    let rendered = render_ishikawa_with_theme(
        &ishikawa_theme_source("classic"),
        &theme,
        engine("24pt !important"),
    );
    assert!(ishikawa_stylesheet(rendered.svg()).contains("font-size: 24pt !important;"));
}

#[test]
fn ishikawa_invalid_config_sizes_do_not_shadow_typed_size() {
    let theme =
        ishikawa_typography_theme(ThemeTextStyle::default().with_font_size_px(48.0).unwrap());
    for invalid in [
        serde_json::json!("bogus"),
        serde_json::json!("-2px"),
        serde_json::json!("20"),
        serde_json::json!(20),
        serde_json::json!("!important"),
        serde_json::json!("24px !important !important"),
    ] {
        for from_source in [false, true] {
            let config = serde_json::json!({"themeVariables":{"fontSize":invalid}});
            let body = "ishikawa-beta\n Problem\n  Cause\n";
            let (source, engine) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    MermaidEngine::new(),
                )
            } else {
                (
                    body.to_owned(),
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                )
            };
            let rendered = render_ishikawa_with_theme(&source, &theme, engine);
            assert!(ishikawa_stylesheet(rendered.svg()).contains("font-size: 48px;"));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1, "{invalid}/{from_source}");
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn ishikawa_ignored_or_inherited_sizes_do_not_reuse_the_bone_spacing_size() {
    let theme = ishikawa_typography_theme(
        ThemeTextStyle::default().with_font_stack(FontStack::single("monospace").unwrap()),
    );
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
            let parsed = common::install_parse_compatibility(&theme, engine)
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap();
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
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), 1, "{size}/{look}");
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn ishikawa_painter_size_admission_preserves_css_and_the_independent_head() {
    let theme = ishikawa_typography_theme(
        ThemeTextStyle::default().with_font_stack(FontStack::single("monospace").unwrap()),
    );
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
            let parsed = common::install_parse_compatibility(&theme, engine())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .unwrap();
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
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let unresolved = resolved_px.is_none() && (has_body || important);
            let visible = !important || resolved_px != Some(0.0);
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(
                evidence.applied_count(),
                usize::from(visible && !unresolved),
                "{size}/{has_body}"
            );
            assert_eq!(evidence.not_applicable_count(), usize::from(!visible));
            assert_eq!(evidence.theme_residual_count(), usize::from(unresolved));
            assert_eq!(
                try_render_ishikawa_with_theme_requirement(
                    source,
                    &theme,
                    engine(),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .is_ok(),
                !unresolved,
                "{size}/{has_body}",
            );
        }
    }
}

#[test]
fn ishikawa_unknown_config_sizes_keep_source_ownership_without_portable_certification() {
    let theme =
        ishikawa_typography_theme(ThemeTextStyle::default().with_font_size_px(48.0).unwrap());
    for size in [
        "125%",
        "1.5em",
        "var(--ishikawa-size)",
        "calc(1em + 2px)",
        "/* comment */24pt!important",
    ] {
        let engine = || {
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables":{"fontSize":size}
                    }),
                ),
            )
        };
        let source = "ishikawa-beta\n Problem\n  Cause\n";
        let rendered = try_render_ishikawa_with_theme_requirement(
            source,
            &theme,
            engine(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert!(ishikawa_stylesheet(rendered.svg()).contains(&format!("font-size: {size};")));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        assert!(
            try_render_ishikawa_with_theme_requirement(
                source,
                &theme,
                engine(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .is_err()
        );
    }
}

#[test]
fn ishikawa_font_size_is_not_applicable_without_inherited_text() {
    let theme = ishikawa_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Ishikawa font size"),
    );

    for source in ["ishikawa-beta\n", "ishikawa-beta\n Root cause\n"] {
        for config in [
            serde_json::json!({}),
            serde_json::json!({"themeVariables":{"fontSize":"var(--unused-size)"}}),
        ] {
            let rendered = render_ishikawa_with_theme(
                source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                ),
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1, "source={source:?}");
            assert_eq!(evidence.accounted_count(), 1, "source={source:?}");
            assert_eq!(evidence.applied_count(), 0, "source={source:?}");
            assert_eq!(evidence.not_applicable_count(), 1, "source={source:?}");
            assert_eq!(evidence.theme_residual_count(), 0, "source={source:?}");
        }
    }
}

#[test]
fn ishikawa_typography_config_ownership_is_property_local() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::single("IshikawaTyped").expect("valid Ishikawa typed font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid Ishikawa typed font size");
    let theme = ishikawa_typography_theme(typography);

    let config_size = render_ishikawa_with_theme(
        &ishikawa_theme_source("classic"),
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "fontSize": "22px" }
            })),
        ),
    );
    let stylesheet = ishikawa_stylesheet(config_size.svg());
    assert!(stylesheet.contains("font-family: IshikawaTyped;"));
    assert!(stylesheet.contains("font-size: 22px;"));
    let evidence =
        merman_render::__private::family_evidence(config_size.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);

    let config_stack = render_ishikawa_with_theme(
        &ishikawa_theme_source("classic"),
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "fontFamily": "IshikawaConfigured" }
            })),
        ),
    );
    let stylesheet = ishikawa_stylesheet(config_stack.svg());
    assert!(stylesheet.contains("font-family: IshikawaConfigured;"));
    assert!(stylesheet.contains("font-size: 24px;"));
    let evidence =
        merman_render::__private::family_evidence(config_stack.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn ishikawa_unsupported_typography_sibling_keeps_direct_font_size() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(24.0)
        .expect("valid Ishikawa font size")
        .with_font_weight(700)
        .expect("valid unsupported Ishikawa font weight");
    let theme = ishikawa_typography_theme(typography);
    let rendered = try_render_ishikawa_with_theme_requirement(
        &ishikawa_theme_source("classic"),
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders unsupported Ishikawa typography");
    let stylesheet = ishikawa_stylesheet(rendered.svg());
    assert!(stylesheet.contains("font-size: 24px;"), "{stylesheet}");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());

    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);

    assert!(
        try_render_ishikawa_with_theme_requirement(
            &ishikawa_theme_source("classic"),
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .is_err(),
        "RequirePortable must reject the unsupported typography sibling"
    );
}

#[test]
fn ishikawa_explicit_text_color_outranks_typed_text_fill() {
    let theme = ishikawa_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Ishikawa typed text fill"),
    );

    for look in ["classic", "handDrawn"] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({ "themeVariables": { "textColor": "#fedcba" } }),
            ),
        );
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
fn ishikawa_typed_text_fill_shadows_unsupported_ordinal_palette() {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#abcdef").expect("valid Ishikawa ordinal palette color")
    ])
    .expect("non-empty Ishikawa ordinal palette");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456")
                                    .expect("valid Ishikawa typed text fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::ISHIKAWA),
                    )
                    .with_ordinal_palette(ThemeTarget::Text, palette),
            ),
        )
        .expect("compile Ishikawa typed fill and ordinal palette theme");
    let rendered = render_ishikawa_with_theme(
        &ishikawa_theme_source("classic"),
        &theme,
        MermaidEngine::new(),
    );
    assert!(
        rendered.svg().contains("fill:#123456 !important;"),
        "{}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn ishikawa_text_fill_is_not_applicable_without_visible_text() {
    let theme = ishikawa_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid empty Ishikawa text fill"),
    );
    let rendered = render_ishikawa_with_theme("ishikawa-beta\n", &theme, MermaidEngine::new());
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
fn ishikawa_fill_only_themes_keep_unknown_sizes_role_local() {
    for target in [ThemeTarget::Text, ThemeTarget::NodeLabel] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        target,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap();
        // No FontStack or FontSize request may supply the missing paint residual.
        for (size, has_body, applied, not_applicable, residual, painted) in [
            ("1e-50px !important", true, 0, 0, 1, 0),
            ("1e-50px", true, 0, 0, 1, 1),
            ("1e-50px", false, 1, 0, 0, 1),
            ("1e-400px !important", true, 0, 0, 1, 0),
            ("0px !important", true, 0, 1, 0, 0),
            ("0px", true, 1, 0, 0, 1),
            ("24pt", true, 1, 0, 0, 3),
        ] {
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
            let rendered = try_render_ishikawa_with_theme_requirement(
                source,
                &theme,
                engine(),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert!(ishikawa_stylesheet(rendered.svg()).contains(&format!("font-size: {size};")));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let text = ishikawa_visible_text_nodes(&document);
            let painted_text = text
                .iter()
                .filter(|node| node.attribute("style") == Some("fill:#123456 !important;"))
                .collect::<Vec<_>>();
            assert_eq!(painted_text.len(), painted, "{target:?}/{size}/{has_body}");
            if painted == 1 {
                assert_eq!(
                    painted_text[0].attribute("class"),
                    Some("ishikawa-head-label")
                );
            }
            drop(painted_text);
            drop(text);
            drop(document);
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(
                evidence.applied_count(),
                applied,
                "{target:?}/{size}/{has_body}"
            );
            assert_eq!(evidence.not_applicable_count(), not_applicable);
            assert_eq!(evidence.theme_residual_count(), residual);
            assert_eq!(
                try_render_ishikawa_with_theme_requirement(
                    source,
                    &theme,
                    engine(),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .is_ok(),
                residual == 0,
                "{target:?}/{size}/{has_body}"
            );
        }
    }
}

#[test]
fn ishikawa_unknown_text_size_does_not_suppress_independent_shape_paint() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#abcdef").unwrap()),
                    ))
                    .with_rule(ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
            ),
        )
        .unwrap();
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "fontSize": "24pt", "themeVariables": {"fontSize": "1e-50px !important"}
        })),
    );
    let rendered = try_render_ishikawa_with_theme_requirement(
        "ishikawa-beta\n Problem\n Cause\n  Detail\n",
        &theme,
        engine,
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    assert!(ishikawa_stylesheet(rendered.svg()).contains(".ishikawa-head { fill: #abcdef;"));
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        ishikawa_visible_text_nodes(&document)
            .iter()
            .all(|node| node.attribute("style").is_none())
    );
    drop(document);
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn ishikawa_zero_sized_text_retires_font_and_fill_but_keeps_geometry_residuals() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::ISHIKAWA,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("monospace").unwrap()),
                    ),
                )
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch::default().with_padding(InsetsPx::all(2.0)),
                        )),
                ),
        )
        .unwrap();
    for (size, head_visible) in [("0px !important", false), ("0px", true)] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": {"fontSize": size}
            })),
        );
        let rendered = try_render_ishikawa_with_theme_requirement(
            "ishikawa-beta\n    Problem\n    Cause\n",
            &theme,
            engine,
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for text in ishikawa_visible_text_nodes(&document) {
            let consumes_fill =
                head_visible && text.attribute("class") == Some("ishikawa-head-label");
            assert_eq!(
                text.attribute("style"),
                consumes_fill.then_some("fill:#123456 !important;"),
                "{size} / {:?}",
                text.attribute("class")
            );
        }
        drop(document);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 3, "{size}");
        assert_eq!(evidence.accounted_count(), 3, "{size}");
        assert_eq!(
            evidence.applied_count(),
            if head_visible { 2 } else { 0 },
            "{size}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            if head_visible { 0 } else { 2 },
            "{size}"
        );
        assert_eq!(evidence.theme_residual_count(), 1, "{size}");
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

#[test]
fn ishikawa_text_palette_is_shadowed_by_the_config_owned_fill_domain() {
    let palette = OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap();
    let theme =
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Text, palette),
            ))
            .unwrap();
    for look in ["classic", "handDrawn"] {
        for source_owner in [false, true] {
            let source = if source_owner {
                format!(
                    "---\nconfig:\n  look: {look}\n  handDrawnSeed: 7\n  themeVariables:\n    textColor: '#abcdef'\n---\n{}",
                    ishikawa_theme_source("classic")
                )
            } else {
                ishikawa_theme_source(look)
            };
            let engine = if source_owner {
                MermaidEngine::new()
            } else {
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"themeVariables":{"textColor":"#abcdef"}}),
                    ),
                )
            };
            let rendered = render_ishikawa_with_theme(&source, &theme, engine);
            let css = ishikawa_stylesheet(rendered.svg());
            let text_rule = css
                .split_once("#ishikawa-theme .ishikawa text {")
                .expect("actual inherited text rule")
                .1
                .split_once('}')
                .unwrap()
                .0;
            assert!(
                text_rule
                    .split(';')
                    .any(|declaration| declaration.trim() == "fill: #abcdef")
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let texts = ishikawa_visible_text_nodes(&document);
            assert!(!texts.is_empty());
            assert!(
                texts
                    .iter()
                    .all(|text| text.attribute("style").is_none()
                        && text.attribute("fill").is_none())
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}
