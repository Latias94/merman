mod common;

use std::sync::Arc;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::CynefinDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};

fn parse_layout_and_render(
    input: &str,
    layout_options: &LayoutOptions,
) -> (CynefinDiagramLayout, String) {
    parse_layout_and_render_with_environment(
        input,
        layout_options,
        &RenderEnvironment::deterministic(),
    )
}

fn parse_layout_and_render_with_environment(
    input: &str,
    layout_options: &LayoutOptions,
    environment: &RenderEnvironment,
) -> (CynefinDiagramLayout, String) {
    let session = environment.begin_session().unwrap();
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse cynefin")
        .expect("detect cynefin");
    let artifact = family::prepare(parsed, layout_options, session).expect("prepare cynefin");
    let projection = artifact.layout_json().expect("serialize Cynefin layout");
    let layout: CynefinDiagramLayout =
        serde_json::from_value(projection["layout"]["CynefinDiagram"].clone())
            .expect("Cynefin layout projection");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("cynefin-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render cynefin")
        .svg()
        .to_owned();

    (layout, svg)
}

fn try_render_cynefin_svg_with_resource_policy(
    input: &str,
    policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse Cynefin resource-bound fixture")
        .expect("detect Cynefin resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("cynefin-bounded".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

fn cynefin_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::CYNEFIN, typography),
        ))
        .expect("compile Cynefin typography theme")
}

fn cynefin_text_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(fill),
    )
    .for_family(DiagramFamilyId::CYNEFIN);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .expect("compile Cynefin text fill theme")
}

fn cynefin_default_text_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(fill),
    )
    .for_family(DiagramFamilyId::CYNEFIN)
    .with_variant(ThemeVariant::Default);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .expect("compile explicit-Default Cynefin text fill theme")
}

fn cynefin_stylesheet(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .expect("valid Cynefin SVG")
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Cynefin stylesheet")
        .to_owned()
}

fn cynefin_rule_body<'a>(stylesheet: &'a str, diagram_id: &str, selector: &str) -> &'a str {
    let rule = format!("#{diagram_id} {selector}{{");
    let body = stylesheet
        .split_once(&rule)
        .unwrap_or_else(|| panic!("missing Cynefin rule {rule:?}"))
        .1;
    let end = body.find('}').expect("Cynefin CSS rule closes");
    &body[..end]
}

const CYNEFIN_TEXT_FILL_SOURCE: &str = r#"---
config:
  cynefin:
    showDomainDescriptions: true
---
cynefin-beta
clear
"Runbook"
complex
"Retrospective"
clear --> complex : "Probe"
"#;

fn try_render_cynefin_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    environment: &RenderEnvironment,
    diagram_id: &str,
) -> merman_render::Result<(CynefinDiagramLayout, family::RenderedFamilySvg)> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Cynefin")
        .expect("detect themed Cynefin");
    let session = environment.begin_session_with_theme(theme).unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let projection = artifact.layout_json()?;
    let layout = serde_json::from_value(projection["layout"]["CynefinDiagram"].clone())
        .expect("Cynefin layout projection");
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok((layout, rendered))
}

fn portable_cynefin_environment() -> RenderEnvironment {
    RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
}

#[test]
fn cynefin_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let input = r#"---
title: Bounded Cynefin
config:
  cynefin:
    seed: 1
---
cynefin-beta
accTitle: Bounded Cynefin accessibility title
accDescr: A bounded Cynefin diagram with items and a transition
clear
  "Runbook"
complex
  "Retrospective"
clear --> complex : "Probe"
"#;
    let baseline = try_render_cynefin_svg_with_resource_policy(
        input,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Cynefin baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Cynefin fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Cynefin SVG byte ceiling");
    let exact = try_render_cynefin_svg_with_resource_policy(input, exact_policy)
        .expect("the exact Cynefin family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Cynefin SVG byte ceiling");
    let error = try_render_cynefin_svg_with_resource_policy(input, below_policy)
        .expect_err("one byte below the Cynefin family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Cynefin MaxSvgBytes rejection, got {error}");
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
fn cynefin_svg_uses_frontmatter_title_unless_body_title_overrides_it() {
    let (_, frontmatter_svg) = parse_layout_and_render(
        r#"---
title: Frontmatter title
---
cynefin-beta
complex
"A"
"#,
        &LayoutOptions::default(),
    );
    assert!(
        frontmatter_svg.contains(r#"class="cynefinTitle""#)
            && frontmatter_svg.contains(">Frontmatter title</text>"),
        "frontmatter title should render when the body has no title: {frontmatter_svg}"
    );

    let (_, body_svg) = parse_layout_and_render(
        r#"---
title: Frontmatter title
---
cynefin-beta
title Body title
complex
"A"
"#,
        &LayoutOptions::default(),
    );
    assert!(body_svg.contains(">Body title</text>"), "{body_svg}");
    assert!(
        !body_svg.contains(">Frontmatter title</text>"),
        "body title should override frontmatter like Mermaid 11.16: {body_svg}"
    );
}

#[test]
fn cynefin_typed_text_fill_reaches_all_visible_text_selectors() {
    for (case, fill, expected_fill) in [
        (
            "solid",
            CanvasPaint::solid("#123456").expect("valid Cynefin text fill"),
            "#123456",
        ),
        ("transparent", CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = cynefin_text_fill_theme(fill);
        let diagram_id = format!("cynefin-text-fill-{case}");
        let (_, rendered) = try_render_cynefin_with_theme(
            CYNEFIN_TEXT_FILL_SOURCE,
            &theme,
            Engine::new(),
            &portable_cynefin_environment(),
            &diagram_id,
        )
        .expect("render strict portable Cynefin text fill");

        let stylesheet = cynefin_stylesheet(rendered.svg());
        for selector in [".cynefinSubtitle", ".cynefinItemText", ".cynefinArrowLabel"] {
            let body = cynefin_rule_body(&stylesheet, &diagram_id, selector);
            assert!(
                body.contains(&format!("fill:{expected_fill};")),
                "Cynefin {selector} did not receive {case} text.fill: {body}"
            );
        }

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "case={case}");
        assert_eq!(evidence.accounted_count(), 1, "case={case}");
        assert_eq!(evidence.applied_count(), 1, "case={case}");
        assert_eq!(evidence.not_applicable_count(), 0, "case={case}");
        assert_eq!(evidence.theme_residual_count(), 0, "case={case}");
        assert_eq!(evidence.compatibility_residual_count(), 0, "case={case}");
        assert_eq!(
            evidence.mermaid_compatibility_residual_count(),
            0,
            "case={case}"
        );
    }
}

#[test]
fn cynefin_text_palette_only_remains_residual_when_its_fallback_is_needed() {
    for case in ["static", "default", "config", "empty", "uncovered"] {
        let palette =
            OrdinalPalette::new([ThemeColorValue::parse("#abcdef").expect("valid palette color")])
                .expect("non-empty palette");
        let mut styles = ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Text, palette);
        if matches!(case, "static" | "default") {
            let mut rule = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#123456").expect("valid text fill")),
            )
            .for_family(DiagramFamilyId::CYNEFIN);
            if case == "default" {
                rule = rule.with_variant(ThemeVariant::Default);
            }
            styles = styles.with_rule(rule);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile text palette theme");
        let engine = if case == "config" {
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "cynefin": { "textColor": "#123456" } }
            })))
        } else {
            Engine::new()
        };
        let source = if case == "empty" {
            "---\nconfig:\n  cynefin:\n    showDomainDescriptions: false\n---\ncynefin-beta\n"
        } else {
            CYNEFIN_TEXT_FILL_SOURCE
        };
        let result = try_render_cynefin_with_theme(
            source,
            &theme,
            engine,
            &portable_cynefin_environment(),
            "cynefin-palette-fallback",
        );
        if case == "uncovered" {
            let error = result
                .err()
                .expect("unsupported palette must remain a strict residual");
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::CYNEFIN, 1))
            );
            assert_eq!(error.incomplete_family_theme(), None);
            continue;
        }
        let (_, rendered) = result.unwrap_or_else(|error| panic!("{case}: {error}"));
        if case != "empty" {
            let stylesheet = cynefin_stylesheet(rendered.svg());
            for selector in [".cynefinSubtitle", ".cynefinItemText", ".cynefinArrowLabel"] {
                assert!(
                    cynefin_rule_body(&stylesheet, "cynefin-palette-fallback", selector)
                        .contains("fill:#123456;"),
                    "{case}: {selector}"
                );
            }
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.not_applicable_count(), 1, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
        assert_eq!(
            evidence.required_count(),
            evidence.accounted_count(),
            "{case}"
        );
        assert_eq!(
            evidence.applied_count(),
            usize::from(matches!(case, "static" | "default")),
            "{case}"
        );
    }
}

#[test]
fn cynefin_explicit_text_color_outranks_typed_text_fill() {
    let theme = cynefin_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Cynefin typed text fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "cynefin": { "textColor": "#fedcba" }
        }
    })));
    let (_, rendered) = try_render_cynefin_with_theme(
        CYNEFIN_TEXT_FILL_SOURCE,
        &theme,
        engine,
        &portable_cynefin_environment(),
        "cynefin-site-text-fill",
    )
    .expect("render site-owned Cynefin text fill");

    let stylesheet = cynefin_stylesheet(rendered.svg());
    for selector in [".cynefinSubtitle", ".cynefinItemText", ".cynefinArrowLabel"] {
        let body = cynefin_rule_body(&stylesheet, "cynefin-site-text-fill", selector);
        assert!(body.contains("fill:#fedcba;"), "{selector}: {body}");
        assert!(
            !body.contains("#123456"),
            "typed fill leaked into {selector}"
        );
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn cynefin_explicit_default_text_fill_reaches_the_typed_terminal() {
    let theme = cynefin_default_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid explicit-Default Cynefin text fill"),
    );
    let (_, rendered) = try_render_cynefin_with_theme(
        CYNEFIN_TEXT_FILL_SOURCE,
        &theme,
        Engine::new(),
        &portable_cynefin_environment(),
        "cynefin-default-text-fill",
    )
    .expect("render explicit-Default Cynefin text fill");

    let stylesheet = cynefin_stylesheet(rendered.svg());
    for selector in [".cynefinSubtitle", ".cynefinItemText", ".cynefinArrowLabel"] {
        assert!(
            cynefin_rule_body(&stylesheet, "cynefin-default-text-fill", selector)
                .contains("fill:#123456;"),
            "{selector} did not receive explicit-Default text.fill"
        );
    }
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn cynefin_text_fill_is_not_applicable_without_visible_text_surfaces() {
    let theme = cynefin_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid empty Cynefin text fill"),
    );
    let source = r#"---
config:
  cynefin:
    showDomainDescriptions: false
---
cynefin-beta
"#;
    let (_, rendered) = try_render_cynefin_with_theme(
        source,
        &theme,
        Engine::new(),
        &portable_cynefin_environment(),
        "cynefin-empty-text-fill",
    )
    .expect("render empty Cynefin text surface");

    let document = roxmltree::Document::parse(rendered.svg()).expect("valid empty Cynefin SVG");
    assert!(!document.descendants().any(|node| {
        node.has_tag_name("text")
            && matches!(
                node.attribute("class"),
                Some("cynefinSubtitle" | "cynefinItemText" | "cynefinArrowLabel")
            )
    }));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[derive(Debug)]
struct FontAwareTextMeasurer;

impl TextMeasurer for FontAwareTextMeasurer {
    fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
        let width = if style.font_family.as_deref() == Some(r#""Fira Code",monospace"#) {
            100.0
        } else {
            10.0
        };
        TextMetrics {
            width,
            height: style.font_size,
            line_count: 1,
        }
    }
}

#[test]
fn cynefin_global_font_family_drives_css_and_item_measurement() {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.cynefin-font-aware").unwrap(),
        "test",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(FontAwareTextMeasurer),
        )),
    );
    let (layout, svg) = parse_layout_and_render_with_environment(
        r#"---
config:
  fontFamily: '"Fira Code", monospace'
---
cynefin-beta
complex
"A"
"#,
        &LayoutOptions::default(),
        &environment,
    );

    assert_eq!(layout.items[0].width, 120.0);
    assert!(
        svg.contains(r#"#cynefin-test{font-family:"Fira Code",monospace;"#),
        "global font family should be emitted by the common Mermaid CSS: {svg}"
    );
}

#[derive(Debug)]
struct CynefinTypedFontProbeMeasurer {
    expected_font_family: String,
}

impl TextMeasurer for CynefinTypedFontProbeMeasurer {
    fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
        assert_eq!(
            style.font_family.as_deref(),
            Some(self.expected_font_family.as_str()),
            "Cynefin measurement must use the same resolved font winner as terminal CSS"
        );
        TextMetrics {
            width: 100.0,
            height: style.font_size,
            line_count: 1,
        }
    }
}

#[test]
fn cynefin_typed_font_stack_drives_measurement_css_and_terminal_evidence() {
    let font_stack =
        FontStack::new(["Cynefin Typed", "monospace"]).expect("valid Cynefin font stack");
    let expected_font = font_stack.as_css();
    let theme = cynefin_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.cynefin-typed-font-probe").unwrap(),
        "test",
    )
    .unwrap();
    let environment = portable_cynefin_environment().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(CynefinTypedFontProbeMeasurer {
                expected_font_family: expected_font.clone(),
            }),
        )),
    );
    let (layout, rendered) = try_render_cynefin_with_theme(
        "cynefin-beta\ncomplex\n\"Measure me\"\n",
        &theme,
        Engine::new(),
        &environment,
        "cynefin-typed-font",
    )
    .expect("render directly themed Cynefin");

    assert_eq!(layout.items[0].width, 120.0);
    let svg = rendered.svg();
    for rule in [
        format!("#cynefin-typed-font{{font-family:{expected_font};"),
        format!("#cynefin-typed-font svg{{font-family:{expected_font};"),
        format!("#cynefin-typed-font :root{{--mermaid-font-family:{expected_font};}}"),
    ] {
        assert!(
            svg.contains(&rule),
            "missing resolved Cynefin font rule {rule:?}"
        );
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn cynefin_empty_item_does_not_invalidate_font_stack_terminal_evidence() {
    let font_stack =
        FontStack::new(["Cynefin Empty Item", "monospace"]).expect("valid Cynefin font stack");
    let expected_font = font_stack.as_css();
    let theme = cynefin_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));

    let (_, rendered) = try_render_cynefin_with_theme(
        "cynefin-beta\ncomplex\n\"\"\n",
        &theme,
        Engine::new(),
        &portable_cynefin_environment(),
        "cynefin-empty-item-font",
    )
    .expect("an empty Cynefin item must not invalidate inherited font evidence");

    assert!(rendered.svg().contains(&format!(
        "#cynefin-empty-item-font{{font-family:{expected_font};"
    )));
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn cynefin_direct_font_stack_exceeds_the_retired_legacy_bridge_limit() {
    let families = (0..32)
        .map(|index| format!("CynefinFont{index:02}{}", "x".repeat(140)))
        .collect::<Vec<_>>();
    let font_stack = FontStack::new(families).expect("valid large Cynefin font stack");
    let expected_font = font_stack.as_css();
    assert!(expected_font.len() > 4 * 1024);
    let theme = cynefin_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));

    let (_, rendered) = try_render_cynefin_with_theme(
        "cynefin-beta\ncomplex\n\"A\"\n",
        &theme,
        Engine::new(),
        &portable_cynefin_environment(),
        "cynefin-large-font",
    )
    .expect("render Cynefin beyond the legacy bridge string budget");
    assert!(rendered.svg().contains(&format!(
        "#cynefin-large-font{{font-family:{expected_font};"
    )));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn cynefin_explicit_site_and_source_font_paths_outrank_the_typed_stack() {
    let theme =
        cynefin_typography_theme(ThemeTextStyle::default().with_font_stack(
            FontStack::single("TypedCynefinFont").expect("valid typed Cynefin font"),
        ));
    let cases = [
        (
            "site-font",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "fontFamily": "SiteCynefinFont,serif" }
            }))),
            "cynefin-beta\ncomplex\n\"A\"\n".to_string(),
            "SiteCynefinFont,serif",
        ),
        (
            "source-font",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
            concat!(
                "%%{init: {\"fontFamily\": \"SourceCynefinFont,monospace\"}}%%\n",
                "cynefin-beta\ncomplex\n\"A\"\n"
            )
            .to_string(),
            "SourceCynefinFont,monospace",
        ),
    ];

    for (case, engine, source, expected_font) in cases {
        let (_, rendered) = try_render_cynefin_with_theme(
            &source,
            &theme,
            engine,
            &portable_cynefin_environment(),
            case,
        )
        .expect("explicit Cynefin font ownership remains portable");
        assert!(
            rendered
                .svg()
                .contains(&format!("#{case}{{font-family:{expected_font};"))
        );
        assert!(!rendered.svg().contains("TypedCynefinFont"));

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }
}

#[test]
fn cynefin_mixed_base_typography_fails_closed() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::single("MixedCynefinFont").expect("valid mixed Cynefin font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid unsupported Cynefin font size");
    let theme = cynefin_typography_theme(typography);
    let error = try_render_cynefin_with_theme(
        "cynefin-beta\ncomplex\n\"A\"\n",
        &theme,
        Engine::new(),
        &portable_cynefin_environment(),
        "cynefin-mixed-typography",
    )
    .err()
    .expect("RequirePortable must reject mixed Cynefin typography");

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::CYNEFIN, 1))
    );
    assert_eq!(error.incomplete_family_theme(), None);
}
