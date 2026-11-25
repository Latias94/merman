use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

use merman_render::theme::{Color, NativeTheme, ThemeCompiler};

fn try_render_zenuml_with_resource_policy(
    source: &str,
    diagram_id: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ZenUML resource-bound fixture")
        .expect("detect ZenUML resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin ZenUML resource-bound session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn zenuml_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"zenuml
@Actor Client
@Boundary Service
@Starter(Client)
Client->Service: request
if(ok) {
  Service.call()
}
"#;
    let diagram_id = "zenuml-bounded";
    let baseline = try_render_zenuml_with_resource_policy(
        source,
        diagram_id,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded ZenUML baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "ZenUML fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact ZenUML SVG byte ceiling");
    let exact = try_render_zenuml_with_resource_policy(source, diagram_id, exact_policy)
        .expect("the exact ZenUML family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact ZenUML SVG byte ceiling");
    let error = try_render_zenuml_with_resource_policy(source, diagram_id, below_policy)
        .expect_err("one byte below the ZenUML family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ZenUML MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

const ZENUML_PAINT: &str = r#"zenuml
title Theme coverage
group Business { @Actor <<Client>> Alice @Database Bob }
@Starter(Alice)
// Important comment
Bob.call() {
  if(ready) {
    Bob.self()
  } else {
    Alice->Bob: Retry
  }
  return result
}
== Phase two ==
while(active) {
  Alice->Bob: Continue
}
new Service
Bob-->Alice: Reply
"#;

fn render_zenuml_source_config(source: &str, config: serde_json::Value) -> String {
    let parsed = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

#[test]
fn zenuml_configured_css_font_is_xml_encoded_once() {
    for (authored, expected) in [
        (r#""Inter", sans-serif"#, r#""Inter",sans-serif"#),
        (r#""A&B", monospace"#, r#""A&B",monospace"#),
        (r#""A&amp;B", serif"#, r#""A&amp;B",serif"#),
        (r#""A#quot;B", serif"#, r#""A#quot;B",serif"#),
        (r#""A#65;B", serif"#, r#""A#65;B",serif"#),
        (r#""A&#35;B", serif"#, r#""A&#35;B",serif"#),
        (r#""A<B", serif"#, r#""A<B",serif"#),
        (r#""A]]>B", serif"#, r#""A]]>B",serif"#),
        (r#""字體 Ω", sans-serif"#, r#""字體 Ω",sans-serif"#),
        (
            r#""</style><script>bad()</script>", serif"#,
            r#""</style><script>bad()</script>",serif"#,
        ),
        (
            r#""Comma, \"Quoted, Font\" Glyph", monospace"#,
            r#""Comma, \"Quoted, Font\" Glyph",monospace"#,
        ),
    ] {
        let svg = render_zenuml_source_config(
            "zenuml\nA->B: request\n",
            serde_json::json!({"themeVariables":{"zenuml":{"fontFamily":authored}}}),
        );
        let document = roxmltree::Document::parse(&svg).expect("valid configured ZenUML SVG");
        assert!(
            !document
                .descendants()
                .any(|node| node.has_tag_name("script"))
        );
        let styles = document
            .descendants()
            .filter(|node| node.has_tag_name("style"))
            .collect::<Vec<_>>();
        assert_eq!(styles.len(), 1);
        let css = styles[0].text().unwrap();
        assert_eq!(
            css.matches(&format!("font-family:{expected};")).count(),
            12,
            "each CSS font declaration must preserve the logical value: {authored}"
        );
    }
}

#[test]
fn zenuml_configured_css_colors_remain_xml_character_data() {
    for color in [
        r"var(--accent\&tone, red)",
        "var(--accent#quot;, red)",
        "var(--accent&amp;tone, red)",
        "red]]></style><script>bad()</script><style>",
    ] {
        let svg = render_zenuml_source_config(
            "zenuml\ntitle Color\nA->B: request\n",
            serde_json::json!({"themeVariables":{"zenuml":{"textColor":color}}}),
        );
        let document = roxmltree::Document::parse(&svg).expect("valid CSS-containing SVG");
        assert!(
            !document
                .descendants()
                .any(|node| node.has_tag_name("script"))
        );
        let styles = document
            .descendants()
            .filter(|node| node.has_tag_name("style"))
            .collect::<Vec<_>>();
        assert_eq!(styles.len(), 1);
        let css = styles[0].text().unwrap();
        assert!(css.contains(&format!("font-weight:600;fill:{color}}}")));
        assert!(css.contains(&format!(".fragment-icon{{color:{color}}}")));
    }
}

#[test]
fn zenuml_configured_css_removes_forbidden_scalars_at_the_xml_boundary() {
    let svg = render_zenuml_source_config(
        "zenuml\nA->B: request\n",
        serde_json::json!({
            "themeVariables":{"zenuml":{"noteTextColor":"A]]\0>\u{fffe}&<B"}}
        }),
    );
    let document =
        roxmltree::Document::parse(&svg).expect("normalized configured CSS is valid XML");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains("fill:A]]>&<B}"));
    assert!(svg.contains("fill:A]]&gt;&amp;&lt;B}"));
}

#[test]
fn zenuml_configured_css_obeys_the_actual_svg_byte_ceiling() {
    let render = |policy| {
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables":{"zenuml":{"fontFamily":r#""A&B", monospace"#}}
                    }),
                ),
            )
            .parse_diagram_for_render_model_sync("zenuml\nA->B: request\n", ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .unwrap();
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    };
    let baseline = render(RenderResourcePolicy::unbounded_for_trusted_input()).unwrap();
    let length = baseline.svg().len();
    let exact = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, length)
        .unwrap();
    assert_eq!(render(exact).unwrap().svg(), baseline.svg());
    let below = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, length - 1)
        .unwrap();
    let error = render(below)
        .err()
        .expect("one byte below the actual SVG length must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.max, length - 1);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == length - 1
    }));
}

fn source_css_rule<'a>(svg: &'a str, class: &str) -> &'a str {
    let prefix = format!("#zenuml .{class}");
    svg.split_once(&prefix)
        .unwrap_or_else(|| panic!("missing {class} rule"))
        .1
        .split_once('{')
        .unwrap()
        .1
        .split_once('}')
        .unwrap()
        .0
}

#[test]
fn zenuml_native_roles_and_typography_reach_layout_and_paint() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version": 2,
                "extends": "paper",
                "typography": {
                    "sans": ["Wide ZenUML Font", "sans-serif"],
                    "body_family": "sans",
                    "base_size_px": 24
                },
                "roles": {
                    "surface.node": "#F5E6CC",
                    "stroke.connector": "#132435",
                    "text.title": "#154567"
                }
            }"##,
            None,
        )
        .unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(ZENUML_PAINT, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let svg = rendered.svg();
    assert!(svg.contains("font-family:\"Wide ZenUML Font\", sans-serif"));
    assert!(source_css_rule(svg, "participant-label").contains("font-size:24px"));
    assert!(source_css_rule(svg, "message-label").contains("font-size:21px"));
    assert!(source_css_rule(svg, "participant-box").contains("fill:#F5E6CC"));
    assert!(source_css_rule(svg, "message-line").contains("stroke:#132435"));
    assert!(source_css_rule(svg, "seq-number-badge").contains("fill:#132435"));
    assert!(svg.contains("#zenuml .seq-number{font-size:18px;fill:#FFFFFF}"));
    assert!(source_css_rule(svg, "frame-title").contains("fill:#154567"));
    let document = roxmltree::Document::parse(svg).unwrap();
    let height = document
        .descendants()
        .find(|node| node.attribute("class") == Some("participant-box"))
        .and_then(|node| node.attribute("height"))
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!(height >= 48.0);
}

#[test]
fn zenuml_host_canvas_replaces_the_compatibility_frame_background() {
    let source = "---\nconfig:\n  themeVariables:\n    zenuml:\n      backgroundColor: '#E6EFDD'\n---\nzenuml\nA->B: request\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    for background in [None, Some(Color::rgb(49, 65, 89)), Some(Color::TRANSPARENT)] {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let session = match background {
            Some(color) => session.with_canvas_background(color),
            None => session,
        };
        let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let svg = rendered.svg();
        if background.is_some() {
            assert!(source_css_rule(svg, "frame-border-outer").contains("fill:none"));
            assert!(source_css_rule(svg, "frame-border-inner").contains("fill:none"));
        } else {
            assert!(source_css_rule(svg, "frame-border-inner").contains("fill:#E6EFDD"));
        }
        let document = roxmltree::Document::parse(svg).unwrap();
        let canvas_fill = document
            .descendants()
            .find(|node| node.attribute("class") == Some("merman-theme-canvas"))
            .and_then(|node| node.children().find(|child| child.has_tag_name("rect")))
            .and_then(|node| node.attribute("fill"));
        assert_eq!(
            canvas_fill,
            (background == Some(Color::rgb(49, 65, 89))).then_some("#314159")
        );
    }
}

#[test]
fn zenuml_source_font_sizes_reserve_participant_and_fragment_title_rows() {
    for size in [14.0, 18.0, 24.0] {
        let svg = render_zenuml_source_config(
            ZENUML_PAINT,
            serde_json::json!({
                "themeVariables":{"zenuml":{"fontFamily":"Inter, sans-serif", "fontSize":format!("{size}px")}}
            }),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            source_css_rule(&svg, "participant-label").contains(&format!("font-size:{size}px"))
        );
        let scale = size / 16.0;
        let participant = document
            .descendants()
            .find(|node| {
                node.attribute("data-participant") == Some("Alice") && node.has_tag_name("g")
            })
            .unwrap();
        let rect = participant
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        let height = rect.attribute("height").unwrap().parse::<f64>().unwrap();
        assert!(
            height >= 2.0 * size,
            "two participant rows must fit at {size}px"
        );
        let stereotype = participant
            .children()
            .find(|node| node.attribute("class") == Some("stereotype-label"))
            .unwrap();
        let label = participant
            .children()
            .find(|node| node.attribute("class") == Some("participant-label"))
            .unwrap();
        assert_eq!(stereotype.text(), Some("«Client»"));
        assert_eq!(label.text(), Some("Alice"));
        let separation = label.attribute("y").unwrap().parse::<f64>().unwrap()
            - stereotype.attribute("y").unwrap().parse::<f64>().unwrap();
        assert!((separation - size).abs() < 1e-9);
        let rect_top = rect.attribute("y").unwrap().parse::<f64>().unwrap();
        assert!(
            stereotype.attribute("y").unwrap().parse::<f64>().unwrap() - size / 2.0 >= rect_top
        );
        assert!(
            label.attribute("y").unwrap().parse::<f64>().unwrap() + size / 2.0 <= rect_top + height
        );
        for header in document
            .descendants()
            .filter(|node| node.attribute("class") == Some("fragment-header"))
        {
            assert_eq!(
                header.attribute("height").unwrap().parse::<f64>().unwrap(),
                25.0 * scale
            );
            assert!(25.0 * scale > 14.0 * scale);
        }
    }
}

#[test]
fn zenuml_small_source_font_keeps_icons_inside_their_reserved_rows() {
    let source = format!("{ZENUML_PAINT}\nBob->Bob: Async self\nAlice->Bob: Final\n");
    let svg = render_zenuml_source_config(
        &source,
        serde_json::json!({
            "themeVariables":{"zenuml":{"fontSize":"8px"}}
        }),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let coordinate = |node: roxmltree::Node<'_, '_>, attribute| {
        node.attribute(attribute).unwrap().parse::<f64>().unwrap()
    };
    for fragment in document.descendants().filter(|node| {
        node.attribute("class")
            .is_some_and(|classes| classes.starts_with("fragment fragment-"))
    }) {
        let header = fragment
            .children()
            .find(|node| node.attribute("class") == Some("fragment-header"))
            .unwrap();
        let icon = fragment
            .children()
            .find(|node| node.attribute("class") == Some("fragment-icon"))
            .unwrap();
        let label = fragment
            .children()
            .find(|node| node.attribute("class") == Some("fragment-label"))
            .unwrap();
        assert_eq!(coordinate(icon, "width"), 10.0);
        assert_eq!(coordinate(icon, "height"), 12.0);
        assert!(
            coordinate(icon, "y") + coordinate(icon, "height")
                <= coordinate(header, "y") + coordinate(header, "height")
        );
        assert!(coordinate(icon, "x") + coordinate(icon, "width") < coordinate(label, "x"));
    }
    let final_message = document
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("message")
                && node.children().any(|text| {
                    text.attribute("class") == Some("message-label") && text.text() == Some("Final")
                })
        })
        .unwrap();
    let next_line = final_message
        .children()
        .find(|node| node.attribute("class") == Some("message-line"))
        .unwrap();
    for call in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("message self-call"))
    {
        let icon = call
            .children()
            .find(|node| node.has_tag_name("svg"))
            .unwrap();
        assert_eq!(coordinate(icon, "width"), 15.0);
        assert_eq!(coordinate(icon, "height"), 12.0);
        assert_eq!(icon.attribute("viewBox"), Some("0 0 30 24"));
        assert!(coordinate(icon, "y") + coordinate(icon, "height") < coordinate(next_line, "y1"));
    }
    for participant in document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("participant"))
    {
        if let Some(icon) = participant
            .children()
            .find(|node| node.attribute("class") == Some("participant-icon"))
        {
            let rect = participant
                .children()
                .find(|node| node.attribute("class") == Some("participant-box"))
                .unwrap();
            let transform = icon.attribute("transform").unwrap();
            let translation = transform
                .split_once("translate(")
                .unwrap()
                .1
                .split_once(')')
                .unwrap()
                .0;
            let icon_y = translation
                .split(',')
                .nth(1)
                .unwrap()
                .trim()
                .parse::<f64>()
                .unwrap();
            assert!(icon_y >= coordinate(rect, "y"));
            assert!(icon_y + 12.0 <= coordinate(rect, "y") + coordinate(rect, "height"));
        }
    }
}
