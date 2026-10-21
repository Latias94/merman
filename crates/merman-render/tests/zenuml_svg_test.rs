use merman_core::{Engine, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, GradientStop,
    LinearGradient, OrdinalSelector, Specified, ThemeColorValue, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant,
    TypographySpec,
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
    try_render_zenuml_with_theme(source, theme, require_portable).expect("render themed ZenUML")
}

fn try_render_zenuml_with_theme(
    source: &str,
    theme: &DiagramTheme,
    require_portable: bool,
) -> merman_render::Result<family::RenderedFamilySvg> {
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

    family::prepare(&parsed, &LayoutOptions::default(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("zenuml-theme".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
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
fn zenuml_default_variant_title_fill_reaches_the_static_title() {
    let theme = zenuml_title_rule_theme(
        ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
        )
        .with_variant(ThemeVariant::Default),
    );
    let rendered =
        render_zenuml_with_theme("zenuml\ntitle Static title\nA->B: request\n", &theme, true);
    assert!(themed_zenuml_css_rule(rendered.svg(), "frame-title").contains("fill:#abcdef"));
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn zenuml_generic_text_and_typography_reach_body_and_shared_title_font() {
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
        ".frame-title{font-family:\"ZenUML Generic Text\";font-size:16px;font-weight:600;fill:#222}"
    ));
    assert!(stylesheet.contains(
        ".message-label{font-family:\"ZenUML Generic Text\";font-size:14px;fill:#654321}"
    ));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn zenuml_ordinal_title_fill_does_not_claim_a_static_frame_title() {
    let style = || {
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#abcdef").expect("valid non-static ZenUML title fill"))
    };
    let themes = [zenuml_title_rule_theme(
        ThemeRule::new(ThemeTarget::Title, style())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid ZenUML title ordinal")),
    )];

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
        .with_site_config(merman_core::MermaidConfig::from_value(config))
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
            .with_site_config(merman_core::MermaidConfig::from_value(serde_json::json!({
                "themeVariables":{"zenuml":{"fontFamily":r#""A&B", monospace"#}}
            })))
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

fn themed_zenuml_css_rule<'a>(svg: &'a str, class: &str) -> &'a str {
    let prefix = format!("#zenuml-theme .{class}");
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
fn zenuml_pure_dark_preset_styles_all_visible_family_surfaces() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(merman_render::diagram_theme::ThemePreset::EditorDark)
        .unwrap();
    let rendered = render_zenuml_with_theme(ZENUML_PAINT, &theme, false);
    let svg = rendered.svg();
    let document = roxmltree::Document::parse(svg).unwrap();
    for class in [
        "frame-border-inner",
        "frame-title",
        "participant-label",
        "message-label",
        "lifeline",
        "occurrence",
        "fragment-header",
        "fragment-condition",
        "divider-bg",
        "divider-label",
        "group-title-bg",
        "group-title-text",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("class") == Some(class)),
            "fixture must reach the actual {class} terminal"
        );
    }
    for (class, paint) in [
        ("frame-border-inner", "fill:transparent"),
        ("frame-header-line", "stroke:#475569"),
        ("frame-title", "fill:#e5e7eb"),
        ("participant-box", "fill:#1f2937"),
        ("participant-label", "fill:#e5e7eb"),
        ("message-label", "fill:#e5e7eb"),
        ("seq-number", "fill:#cbd5e1"),
        ("message-line", "stroke:#94a3b8"),
        ("lifeline", "stroke:#475569"),
        ("occurrence", "fill:#334155"),
        ("group-title-bg", "fill:#1e293b"),
        ("divider-bg", "fill:#422006"),
        ("divider-label", "fill:#fef3c7"),
    ] {
        assert!(
            themed_zenuml_css_rule(svg, class).contains(paint),
            "{class} must receive {paint}: {}",
            themed_zenuml_css_rule(svg, class)
        );
    }
}

#[test]
fn zenuml_pure_light_preset_preserves_family_surface_foregrounds() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(merman_render::diagram_theme::ThemePreset::EditorLight)
        .unwrap();
    let rendered = render_zenuml_with_theme(ZENUML_PAINT, &theme, false);
    for (class, paint) in [
        ("frame-border-inner", "fill:transparent"),
        ("frame-header-line", "stroke:#94a3b8"),
        ("participant-box", "fill:#f8fafc"),
        ("participant-label", "fill:#0f172a"),
        ("message-label", "fill:#0f172a"),
        ("seq-number", "fill:#475569"),
        ("occurrence", "fill:#e2e8f0"),
        ("fragment-header", "fill:#e2e8f0"),
        ("fragment-condition", "fill:#0f172a"),
        ("group-title-bg", "fill:#f1f5f9"),
        ("group-title-text", "fill:#0f172a"),
        ("divider-bg", "fill:#fff7ed"),
        ("divider-label", "fill:#7c2d12"),
    ] {
        assert!(
            themed_zenuml_css_rule(rendered.svg(), class).contains(paint),
            "{class}: {paint}"
        );
    }
}

#[test]
fn zenuml_non_paint_themes_preserve_source_actor_and_frame_border_owners() {
    let empty = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let typography = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::ZENUML,
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::single("ZenUML Border Test").unwrap()),
                ),
            ),
        )
        .unwrap();
    for (common_border, expected_frame) in
        [("", "#666"), ("      borderColor: '#008800'\n", "#008800")]
    {
        let source = format!(
            "---\nconfig:\n  themeVariables:\n    zenuml:\n      actorBorderColor: '#ff0000'\n{common_border}---\nzenuml\nA->B: request\n== Divider ==\n"
        );
        let baseline = render_zenuml_source_config(&source, serde_json::json!({}));
        assert!(
            source_css_rule(&baseline, "frame-header-line")
                .contains(&format!("stroke:{expected_frame}"))
        );
        for (theme, applied) in [(&empty, 0), (&typography, 1)] {
            let rendered = render_zenuml_with_theme(&source, theme, true);
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for class in ["frame-border-outer", "frame-header-line", "participant-box"] {
                assert!(
                    document
                        .descendants()
                        .any(|node| node.attribute("class") == Some(class))
                );
                assert_eq!(
                    themed_zenuml_css_rule(rendered.svg(), class),
                    source_css_rule(&baseline, class)
                );
            }
            assert!(
                themed_zenuml_css_rule(rendered.svg(), "participant-box")
                    .contains("stroke:#ff0000")
            );
            if !common_border.is_empty() {
                assert!(
                    themed_zenuml_css_rule(rendered.svg(), "divider-bg").contains("stroke:#008800")
                );
            }
            drop(document);
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn zenuml_inline_comment_fill_reports_only_actual_host_text_consumers() {
    let theme = zenuml_title_rule_theme(ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    ));
    for (comment_style, source_note, applied) in
        [("<red>", true, 0), ("", true, 1), ("<red>", false, 1)]
    {
        let config = if source_note {
            "---\nconfig:\n  themeVariables:\n    zenuml:\n      noteTextColor: '#111111'\n---\n"
        } else {
            ""
        };
        let source =
            format!("{config}zenuml\n// {comment_style} Authored comment\n== Divider ==\n");
        let rendered = render_zenuml_with_theme(&source, &theme, true);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let comment = document
            .descendants()
            .find(|node| node.attribute("class") == Some("comment-text"))
            .unwrap();
        assert_eq!(
            comment.attribute("style"),
            (!comment_style.is_empty()).then_some("fill:red")
        );
        assert!(themed_zenuml_css_rule(rendered.svg(), "comment-text").contains("fill:#123456"));
        assert!(
            themed_zenuml_css_rule(rendered.svg(), "divider-label").contains(if source_note {
                "fill:#111111"
            } else {
                "fill:#123456"
            })
        );
        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), 1 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn zenuml_inline_comment_fill_preserves_unsupported_text_terminal_domains() {
    let owned_divider = "---\nconfig:\n  themeVariables:\n    zenuml:\n      noteTextColor: '#111111'\n---\nzenuml\n// <red> Authored comment\n== Divider ==\n";
    let owned_message = "---\nconfig:\n  themeVariables:\n    zenuml:\n      actorTextColor: '#111111'\n      sequenceNumberTextColor: '#111111'\n---\nzenuml\n// [red] Authored comment\nA->B: request\n";
    let gradient = CanvasPaint::LinearGradient(
        LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
            ],
        )
        .unwrap(),
    );
    for (source, opacity, residual) in [
        (owned_divider, false, 0),
        (owned_message, false, 0),
        (owned_divider, true, 1),
        (owned_message, true, 1),
        (
            "zenuml\n// <red> Authored comment\n== Divider ==\n",
            false,
            1,
        ),
        ("zenuml\nA->B: request\n", false, 1),
        ("zenuml\nA->B: request\n", true, 1),
    ] {
        let mut patch = ThemeStylePatch::default().with_fill(gradient.clone());
        if opacity {
            patch.paint.opacity = Specified::Value(0.5);
        }
        let theme = zenuml_title_rule_theme(ThemeRule::new(ThemeTarget::Text, patch));
        let result = try_render_zenuml_with_theme(source, &theme, true);
        if residual == 0 {
            assert!(
                result.is_ok(),
                "source-owned text must suppress unsupported fill"
            );
        } else {
            assert_eq!(
                result.err().unwrap().unverified_family_theme(),
                Some((DiagramFamilyId::ZENUML, 1))
            );
        }
        let rendered = render_zenuml_with_theme(source, &theme, false);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        if let Some(comment) = document
            .descendants()
            .find(|node| node.attribute("class") == Some("comment-text"))
        {
            assert_eq!(comment.attribute("style"), Some("fill:red"));
        }
        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1 - residual);
        assert_eq!(evidence.theme_residual_count(), residual);
    }
}

#[test]
fn zenuml_empty_and_title_only_inputs_report_only_visible_typography() {
    let theme = zenuml_non_title_text_theme();
    for (source, applied, absent) in [("zenuml\n", 0, 2), ("zenuml\ntitle Only a title\n", 1, 1)] {
        let rendered = render_zenuml_with_theme(source, &theme, true);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), absent);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn zenuml_namespace_source_paint_preserves_property_local_ownership_under_preset() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(merman_render::diagram_theme::ThemePreset::EditorDark)
        .unwrap();
    let source = format!(
        "---\nconfig:\n  themeVariables:\n    zenuml:\n      textColor: '#123456'\n      actorBackgroundColor: '#abcdef'\n      backgroundColor: '#f0f1f2'\n      noteBackgroundColor: '#fefefe'\n      noteBorderColor: '#456789'\n      noteTextColor: '#234567'\n---\n{ZENUML_PAINT}"
    );
    let rendered = render_zenuml_with_theme(&source, &theme, false);
    assert!(themed_zenuml_css_rule(rendered.svg(), "frame-title").contains("fill:#123456"));
    assert!(themed_zenuml_css_rule(rendered.svg(), "participant-box").contains("fill:#abcdef"));
    assert!(themed_zenuml_css_rule(rendered.svg(), "participant-box").contains("stroke:#475569"));
    assert!(themed_zenuml_css_rule(rendered.svg(), "message-label").contains("fill:#123456"));
    assert!(themed_zenuml_css_rule(rendered.svg(), "frame-border-inner").contains("fill:#f0f1f2"));
    assert!(themed_zenuml_css_rule(rendered.svg(), "divider-bg").contains("fill:#fefefe"));
    assert!(themed_zenuml_css_rule(rendered.svg(), "divider-bg").contains("stroke:#456789"));
    assert!(themed_zenuml_css_rule(rendered.svg(), "divider-label").contains("fill:#234567"));
}

#[test]
fn zenuml_fragment_only_sequence_numbers_apply_static_fill_and_reject_unsupported_rules() {
    let source = "zenuml\nwhile(active) {}\nif(ready) {}\n";
    let theme = zenuml_title_rule_theme(ThemeRule::new(
        ThemeTarget::SequenceNumberLabel,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    ));
    let rendered = render_zenuml_with_theme(source, &theme, true);
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let numbers = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("seq-number"))
        .collect::<Vec<_>>();
    assert_eq!(numbers.len(), 2);
    assert!(
        numbers
            .iter()
            .all(|node| node.text().is_some_and(|text| !text.is_empty()))
    );
    assert!(themed_zenuml_css_rule(rendered.svg(), "seq-number").contains("fill:#123456"));
    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);

    let mut opacity = ThemeStylePatch::default();
    opacity.paint.opacity = Specified::Value(0.5);
    for rule in [
        ThemeRule::new(ThemeTarget::SequenceNumberLabel, opacity),
        ThemeRule::new(
            ThemeTarget::SequenceNumberLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(1).unwrap()),
    ] {
        let theme = zenuml_title_rule_theme(rule);
        let result = try_render_zenuml_with_theme(source, &theme, true);
        let error = result
            .err()
            .expect("visible fragment numbers must reject unsupported rules");
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::ZENUML, 1))
        );
        let rendered = render_zenuml_with_theme(source, &theme, false);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.theme_residual_count(), 1);
    }
}

#[test]
fn zenuml_source_owned_paint_suppresses_only_the_overridden_unsupported_properties() {
    let gradient = CanvasPaint::LinearGradient(
        LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
            ],
        )
        .unwrap(),
    );
    for (key, target, class, stroke) in [
        (
            "actorTextColor",
            ThemeTarget::ActorLabel,
            "participant-label",
            false,
        ),
        (
            "textColor",
            ThemeTarget::ActorLabel,
            "participant-label",
            false,
        ),
        (
            "actorBorderColor",
            ThemeTarget::Actor,
            "participant-box",
            true,
        ),
    ] {
        let source = format!(
            "---\nconfig:\n  themeVariables:\n    zenuml:\n      {key}: '#654321'\n---\nzenuml\nA->B: request\n"
        );
        let patch = if stroke {
            ThemeStylePatch::default().with_stroke(gradient.clone())
        } else {
            ThemeStylePatch::default().with_fill(gradient.clone())
        };
        let theme = zenuml_title_rule_theme(ThemeRule::new(target, patch.clone()));
        let rendered = render_zenuml_with_theme(&source, &theme, true);
        assert!(
            themed_zenuml_css_rule(rendered.svg(), class).contains(&format!(
                "{}:#654321",
                if stroke { "stroke" } else { "fill" }
            ))
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);

        let mut mixed = patch;
        mixed.paint.opacity = Specified::Value(0.5);
        let mixed_theme = zenuml_title_rule_theme(ThemeRule::new(target, mixed));
        let result = try_render_zenuml_with_theme(&source, &mixed_theme, true);
        let error = result
            .err()
            .expect("source paint must not hide unsupported opacity");
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::ZENUML, 1))
        );
        let rendered = render_zenuml_with_theme(&source, &mixed_theme, false);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.theme_residual_count(), 1);
    }
}

#[test]
fn zenuml_typed_font_size_reaches_layout_and_paint_together() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::ZENUML,
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::single("Wide ZenUML Font").unwrap())
                        .with_font_size_px(24.0)
                        .unwrap(),
                ),
            ),
        )
        .unwrap();
    let rendered = render_zenuml_with_theme(ZENUML_PAINT, &theme, false);
    assert!(themed_zenuml_css_rule(rendered.svg(), "participant-label").contains("font-size:24px"));
    assert!(themed_zenuml_css_rule(rendered.svg(), "message-label").contains("font-size:21px"));
    assert!(
        themed_zenuml_css_rule(rendered.svg(), "participant-label")
            .contains("font-family:\"Wide ZenUML Font\";")
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let actor = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("data-participant") == Some("Alice"))
        .unwrap();
    let rect = actor
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    assert!(rect.attribute("height").unwrap().parse::<f64>().unwrap() >= 48.0);
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
