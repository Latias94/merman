use merman_core::{Engine, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalSelector,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn zenuml_title_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::ZENUML),
                ),
            ),
        )
        .expect("compile ZenUML title fill theme")
}

fn zenuml_non_title_text_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::ZENUML,
                        ThemeTextStyle::default().with_font_stack(
                            FontStack::single("ZenUML Generic Text")
                                .expect("valid ZenUML generic text font"),
                        ),
                    ),
                )
                .with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#654321")
                                    .expect("valid ZenUML generic text fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::ZENUML),
                    ),
                ),
        )
        .expect("compile ZenUML non-title text theme")
}

fn zenuml_title_rule_theme(rule: ThemeRule) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(rule.for_family(DiagramFamilyId::ZENUML)),
        ))
        .expect("compile qualified ZenUML title theme")
}

fn render_zenuml_with_theme(
    source: &str,
    theme: &DiagramTheme,
    require_portable: bool,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed ZenUML")
        .expect("detect themed ZenUML");
    let environment = if require_portable {
        RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
    } else {
        RenderEnvironment::deterministic()
    };
    let session = environment
        .begin_session_with_theme(theme)
        .expect("begin themed ZenUML session");

    family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare themed ZenUML")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("zenuml-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed ZenUML")
}

fn try_render_zenuml_with_resource_policy(
    source: &str,
    diagram_id: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ZenUML resource-bound fixture")
        .expect("detect ZenUML resource-bound fixture");
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

#[test]
fn zenuml_title_fill_reaches_frame_title_and_family_evidence() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid ZenUML title fill"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (fill, expected_fill) in cases {
        let theme = zenuml_title_fill_theme(fill);
        let rendered = render_zenuml_with_theme(
            "zenuml\ntitle Themed ZenUML\nClient->Service: request\n",
            &theme,
            true,
        );
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed ZenUML SVG");
        let frame_titles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text") && node.attribute("class") == Some("frame-title")
            })
            .collect::<Vec<_>>();
        assert_eq!(frame_titles.len(), 1);
        assert_eq!(frame_titles[0].text(), Some("Themed ZenUML"));

        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("ZenUML stylesheet");
        assert!(
            stylesheet.contains(&format!(
                ".frame-title{{font-family:Helvetica,Verdana,serif;font-size:16px;font-weight:600;fill:{expected_fill}}}"
            )),
            "typed Title.fill must own text.frame-title: {stylesheet}"
        );
        assert_eq!(stylesheet.matches(".frame-title{").count(), 1);

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn zenuml_title_fill_is_not_applicable_without_frame_title_text() {
    let theme = zenuml_title_fill_theme(
        CanvasPaint::solid("#123456").expect("valid titleless ZenUML fill"),
    );
    let rendered = render_zenuml_with_theme("zenuml\nClient->Service: request\n", &theme, true);
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid titleless ZenUML SVG");
    assert!(!document.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("class") == Some("frame-title")
    }));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn zenuml_generic_text_and_typography_do_not_claim_frame_title() {
    let theme = zenuml_non_title_text_theme();
    let rendered = render_zenuml_with_theme(
        "zenuml\ntitle Fixed ZenUML title\nClient->Service: request\n",
        &theme,
        false,
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid residual ZenUML SVG");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("ZenUML stylesheet");
    assert!(!stylesheet.contains(".frame-title{fill:#654321;}"));
    assert!(stylesheet.contains(
        ".frame-title{font-family:Helvetica,Verdana,serif;font-size:16px;font-weight:600;fill:#222}"
    ));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 2);
}

#[test]
fn zenuml_qualified_and_ordinal_title_fill_do_not_claim_frame_title() {
    let style = || {
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#abcdef").expect("valid non-static ZenUML title fill"))
    };
    let themes = [
        zenuml_title_rule_theme(
            ThemeRule::new(ThemeTarget::Title, style()).with_variant(ThemeVariant::Default),
        ),
        zenuml_title_rule_theme(
            ThemeRule::new(ThemeTarget::Title, style())
                .with_ordinal(OrdinalSelector::exact(1).expect("valid ZenUML title ordinal")),
        ),
    ];

    for theme in themes {
        let rendered = render_zenuml_with_theme(
            "zenuml\ntitle Static title only\nClient->Service: request\n",
            &theme,
            false,
        );
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid non-static ZenUML SVG");
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("ZenUML stylesheet");
        assert!(!rendered.svg().contains("#abcdef"));
        let frame_fill = stylesheet
            .split('}')
            .filter_map(|rule| rule.split_once('{'))
            .find(|(selector, _)| selector.trim().ends_with(".frame-title"))
            .and_then(|(_, declarations)| {
                declarations
                    .split(';')
                    .rev()
                    .filter_map(|declaration| declaration.split_once(':'))
                    .find_map(|(property, value)| {
                        (property.trim() == "fill").then_some(value.trim())
                    })
            });
        assert_eq!(frame_fill, Some("#222"));

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
    }
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
    let parsed = Engine::new()
        .with_site_config(merman_core::presentation::PresentationConfig::from_value(
            config,
        ))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
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
        let parsed = Engine::new()
            .with_site_config(merman_core::presentation::PresentationConfig::from_value(
                serde_json::json!({
                    "themeVariables":{"zenuml":{"fontFamily":r#""A&B", monospace"#}}
                }),
            ))
            .parse_diagram_for_render_model_sync("zenuml\nA->B: request\n", ParseOptions::strict())
            .unwrap()
            .unwrap();
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
