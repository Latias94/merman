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

    family::prepare(parsed, &LayoutOptions::default(), session)
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
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)?.render_svg(
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
        assert!(!stylesheet.contains(".frame-title{fill:#abcdef;}"));

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
    }
}
