use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};

#[derive(Debug, Clone, Copy)]
enum InheritedTextFamily {
    Info,
    Error,
}

impl InheritedTextFamily {
    const ALL: [Self; 2] = [Self::Info, Self::Error];

    const fn family_id(self) -> DiagramFamilyId {
        match self {
            Self::Info => DiagramFamilyId::INFO,
            Self::Error => DiagramFamilyId::ERROR,
        }
    }

    const fn base_source(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Error => "flowchart TD\nA -->",
        }
    }

    fn parse_options(self) -> ParseOptions {
        match self {
            Self::Info => ParseOptions::strict(),
            Self::Error => ParseOptions::lenient(),
        }
    }

    const fn aria_roledescription(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Error => "error",
        }
    }

    const fn terminal_class(self) -> &'static str {
        match self {
            Self::Info => "version",
            Self::Error => "error-text",
        }
    }

    const fn terminal_font_sizes(self) -> &'static [&'static str] {
        match self {
            Self::Info => &["32"],
            Self::Error => &["150px", "100px"],
        }
    }
}

fn inherited_font_stack_theme(family: InheritedTextFamily, font_stack: FontStack) -> DiagramTheme {
    compile_inherited_theme(
        family,
        Some(ThemeTextStyle::default().with_font_stack(font_stack)),
        &[],
    )
}

fn inherited_font_size_theme(family: InheritedTextFamily, font_size_px: f32) -> DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(font_size_px)
        .expect("valid inherited text-family font size");
    compile_inherited_theme(family, Some(typography), &[])
}

fn inherited_paint_theme(
    family: InheritedTextFamily,
    fill_targets: &[ThemeTarget],
) -> DiagramTheme {
    compile_inherited_theme(family, None, fill_targets)
}

fn compile_inherited_theme(
    family: InheritedTextFamily,
    typography: Option<ThemeTextStyle>,
    fill_targets: &[ThemeTarget],
) -> DiagramTheme {
    let mut spec = DiagramThemeSpec::new();
    if let Some(typography) = typography {
        spec = spec.with_typography(
            TypographySpec::default().with_family_style(family.family_id(), typography),
        );
    }
    if !fill_targets.is_empty() {
        let mut rules = ThemeRuleSet::default();
        for target in fill_targets {
            rules = rules.with_rule(
                ThemeRule::new(
                    *target,
                    ThemeStylePatch::default().with_fill(
                        CanvasPaint::solid("#123456").expect("valid inherited text-family paint"),
                    ),
                )
                .for_family(family.family_id()),
            );
        }
        spec = spec.with_styles(rules);
    }
    DiagramThemeCompiler::new()
        .compile(spec)
        .expect("compile inherited text-family theme")
}

fn compile_global_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_default(typography)),
        )
        .expect("compile global inherited text-family theme")
}

fn compile_flowchart_only_theme(
    typography: Option<ThemeTextStyle>,
    node_fill: Option<CanvasPaint>,
) -> DiagramTheme {
    let mut spec = DiagramThemeSpec::new();
    if let Some(typography) = typography {
        spec = spec.with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::FLOWCHART, typography),
        );
    }
    if let Some(node_fill) = node_fill {
        spec = spec.with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(node_fill),
                )
                .with_variant(ThemeVariant::Default)
                .for_family(DiagramFamilyId::FLOWCHART),
            ),
        );
    }
    DiagramThemeCompiler::new()
        .compile(spec)
        .expect("compile Flowchart-only compatibility theme")
}

fn try_render_family(
    family: InheritedTextFamily,
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_family_with_requirement(
        family,
        source,
        theme,
        engine,
        diagram_id,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_render_family_with_requirement(
    family: InheritedTextFamily,
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, family.parse_options())
        .expect("parse inherited text-family source")
        .expect("detect inherited text-family source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(theme)
        .expect("begin strict inherited text-family session");
    family::prepare(parsed, &LayoutOptions::default(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn assert_inherited_font_stack_surface(
    family: InheritedTextFamily,
    rendered: &family::RenderedFamilySvg,
    diagram_id: &str,
    expected_font: &str,
) {
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid inherited-family SVG");
    let root = document.root_element();
    assert_eq!(root.attribute("id"), Some(diagram_id));
    assert_eq!(
        root.attribute("aria-roledescription"),
        Some(family.aria_roledescription())
    );

    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("inherited-family stylesheet");
    assert!(
        stylesheet.contains(&format!("#{diagram_id}{{font-family:{expected_font};")),
        "resolved font stack must reach root CSS: {stylesheet}"
    );
    assert!(
        stylesheet.contains(&format!("#{diagram_id} svg{{font-family:{expected_font};")),
        "resolved font stack must reach inherited SVG CSS: {stylesheet}"
    );
    assert!(
        stylesheet.contains(&format!(
            "#{diagram_id} :root{{--mermaid-font-family:{expected_font};}}"
        )),
        "resolved font stack must reach the Mermaid root variable: {stylesheet}"
    );

    let terminals = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == family.terminal_class())
                })
                && node.text().is_some_and(|text| !text.trim().is_empty())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        terminals.len(),
        family.terminal_font_sizes().len(),
        "every real inherited text terminal must be counted exactly once"
    );
    assert_eq!(
        terminals
            .iter()
            .map(|node| {
                node.attribute("font-size")
                    .expect("fixed terminal font size")
            })
            .collect::<Vec<_>>(),
        family.terminal_font_sizes(),
        "family-owned fixed terminal sizes must remain unchanged"
    );
    assert!(
        terminals
            .iter()
            .all(|node| node.attribute("font-family").is_none()),
        "terminal text must inherit the resolved root font stack"
    );
}

fn assert_font_stack_evidence(
    rendered: family::RenderedFamilySvg,
    expected_applied: usize,
    expected_not_applicable: usize,
) {
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), expected_applied);
    assert_eq!(evidence.not_applicable_count(), expected_not_applicable);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

fn assert_residual_evidence(
    rendered: family::RenderedFamilySvg,
    required_count: usize,
    applied_count: usize,
    residual_count: usize,
) {
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), required_count);
    assert_eq!(evidence.accounted_count(), required_count);
    assert_eq!(evidence.applied_count(), applied_count);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), residual_count);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

fn non_empty_text_count_with_class(svg: &str, expected_class: &str) -> usize {
    let document = roxmltree::Document::parse(svg).expect("valid inherited-family SVG");
    document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == expected_class)
                })
                && node.text().is_some_and(|text| !text.trim().is_empty())
        })
        .count()
}

#[test]
fn info_and_error_direct_font_stack_above_legacy_limit_reaches_css_dom_and_evidence() {
    for family in InheritedTextFamily::ALL {
        let names = (0..32)
            .map(|index| format!("InheritedFont{index:02}{}", "x".repeat(140)))
            .collect::<Vec<_>>();
        let font_stack = FontStack::new(names).expect("valid maximum-width inherited font stack");
        let expected_font = font_stack.as_css();
        assert!(expected_font.len() > 4 * 1024);
        let theme = inherited_font_stack_theme(family, font_stack);
        let diagram_id = format!("{}-typed-font", family.aria_roledescription());
        let rendered = try_render_family(
            family,
            family.base_source(),
            &theme,
            Engine::new(),
            &diagram_id,
        )
        .expect("render directly themed inherited text family");

        assert_inherited_font_stack_surface(family, &rendered, &diagram_id, &expected_font);
        assert_font_stack_evidence(rendered, 1, 0);
    }
}

#[test]
fn info_and_error_explicit_site_and_source_font_paths_outrank_typed_font_stack() {
    for family in InheritedTextFamily::ALL {
        let theme = inherited_font_stack_theme(
            family,
            FontStack::single("TypedInheritedFont").expect("valid typed inherited font"),
        );
        let source_theme_variable = [
            "%%{init: {\"themeVariables\": {\"fontFamily\": \"SourceVariableFont,serif\"}}}%%\n",
            family.base_source(),
        ]
        .concat();
        let source_root = [
            "%%{init: {\"fontFamily\": \"SourceRootFont,monospace\"}}%%\n",
            family.base_source(),
        ]
        .concat();
        let cases = [
            (
                "site-theme-variable",
                Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                    "themeVariables": { "fontFamily": "SiteVariableFont,serif" }
                }))),
                family.base_source().to_string(),
                "SiteVariableFont,serif",
            ),
            (
                "site-root",
                Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                    "fontFamily": "SiteRootFont,monospace"
                }))),
                family.base_source().to_string(),
                "SiteRootFont,monospace",
            ),
            (
                "source-theme-variable",
                Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                    "secure": []
                }))),
                source_theme_variable,
                "SourceVariableFont,serif",
            ),
            (
                "source-root",
                Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                    "secure": []
                }))),
                source_root,
                "SourceRootFont,monospace",
            ),
        ];

        for (case, engine, source, expected_font) in cases {
            let diagram_id = format!("{}-{case}", family.aria_roledescription());
            let rendered = try_render_family(family, &source, &theme, engine, &diagram_id)
                .expect("explicit inherited font configuration remains portable");

            assert_inherited_font_stack_surface(family, &rendered, &diagram_id, expected_font);
            assert!(!rendered.svg().contains("TypedInheritedFont"));
            assert_font_stack_evidence(rendered, 0, 1);
        }
    }
}

#[test]
fn info_and_error_unsupported_paint_is_explicit_best_effort_evidence() {
    for (case, family, targets) in [
        (
            "info-title",
            InheritedTextFamily::Info,
            &[ThemeTarget::Title][..],
        ),
        (
            "error-text",
            InheritedTextFamily::Error,
            &[ThemeTarget::Text][..],
        ),
        (
            "error-title",
            InheritedTextFamily::Error,
            &[ThemeTarget::Title][..],
        ),
    ] {
        let theme = inherited_paint_theme(family, targets);
        let rendered = try_render_family_with_requirement(
            family,
            family.base_source(),
            &theme,
            Engine::new(),
            case,
            ThemePortabilityRequirement::BestEffort,
        )
        .expect("render unsupported paint with BestEffort evidence");

        assert_eq!(
            non_empty_text_count_with_class(rendered.svg(), family.terminal_class()),
            family.terminal_font_sizes().len(),
            "unsupported paint must be reconciled against the real text occurrences"
        );
        if targets.contains(&ThemeTarget::Title) {
            assert_eq!(
                non_empty_text_count_with_class(rendered.svg(), "title"),
                0,
                "Info/Error must not invent a Title terminal to claim unsupported paint"
            );
        }
        assert_residual_evidence(rendered, 1, 0, 1);
    }

    let family = InheritedTextFamily::Error;
    let expected_font = "TypedMixedFont";
    let theme = compile_inherited_theme(
        family,
        Some(ThemeTextStyle::default().with_font_stack(
            FontStack::single(expected_font).expect("valid mixed Error font stack"),
        )),
        &[ThemeTarget::Text, ThemeTarget::Title],
    );
    let rendered = try_render_family_with_requirement(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "error-mixed-unsupported-paint",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render mixed Error typography and unsupported paint evidence");

    assert_inherited_font_stack_surface(
        family,
        &rendered,
        "error-mixed-unsupported-paint",
        expected_font,
    );
    assert_residual_evidence(rendered, 3, 1, 2);
}

#[test]
fn info_and_error_unsupported_paint_fails_require_portable_as_residual_not_incomplete() {
    for (case, family, targets, residual_count) in [
        (
            "info-title-strict",
            InheritedTextFamily::Info,
            &[ThemeTarget::Title][..],
            1,
        ),
        (
            "error-text-strict",
            InheritedTextFamily::Error,
            &[ThemeTarget::Text][..],
            1,
        ),
        (
            "error-title-strict",
            InheritedTextFamily::Error,
            &[ThemeTarget::Title][..],
            1,
        ),
        (
            "error-text-title-strict",
            InheritedTextFamily::Error,
            &[ThemeTarget::Text, ThemeTarget::Title][..],
            2,
        ),
    ] {
        let theme = inherited_paint_theme(family, targets);
        let error = try_render_family(family, family.base_source(), &theme, Engine::new(), case)
            .err()
            .expect("strict portability must reject unsupported inherited-family paint");

        assert_eq!(
            error.unverified_family_theme(),
            Some((family.family_id(), residual_count))
        );
        assert_eq!(error.incomplete_family_theme(), None);
    }
}

#[test]
fn info_and_error_fixed_terminals_reject_unsupported_base_font_size() {
    for family in InheritedTextFamily::ALL {
        let theme = inherited_font_size_theme(family, 24.0);
        let error = try_render_family(
            family,
            family.base_source(),
            &theme,
            Engine::new(),
            &format!("{}-unsupported-font-size", family.aria_roledescription()),
        )
        .err()
        .expect("strict portability must reject inherited-family base font size");

        assert_eq!(
            error.unverified_family_theme(),
            Some((family.family_id(), 1))
        );
        assert_eq!(error.incomplete_family_theme(), None);
    }
}

#[test]
fn lenient_error_reconciles_the_detected_flowchart_typography_for_a_global_font_stack() {
    let family = InheritedTextFamily::Error;
    let font_stack =
        FontStack::new(["PortableErrorFont", "sans-serif"]).expect("valid global Error font stack");
    let expected_font = font_stack.as_css();
    let theme =
        compile_global_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let rendered = try_render_family(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "lenient-error-global-font-stack",
    )
    .expect("a typed Error font stack must retire the parse-family compatibility evidence");

    assert_inherited_font_stack_surface(
        family,
        &rendered,
        "lenient-error-global-font-stack",
        &expected_font,
    );
    assert_font_stack_evidence(rendered, 1, 0);
}

#[test]
fn lenient_error_retires_font_size_compatibility_but_keeps_the_typed_theme_residual() {
    let family = InheritedTextFamily::Error;
    let font_stack =
        FontStack::new(["ResidualErrorFont", "sans-serif"]).expect("valid global Error font stack");
    let expected_font = font_stack.as_css();
    let typography = ThemeTextStyle::default()
        .with_font_stack(font_stack)
        .with_font_size_px(24.0)
        .expect("valid unsupported Error font size");
    let theme = compile_global_typography_theme(typography);
    let rendered = try_render_family_with_requirement(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "lenient-error-font-stack-size-best-effort",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort must preserve the structured Error font-size residual");

    assert_inherited_font_stack_surface(
        family,
        &rendered,
        "lenient-error-font-stack-size-best-effort",
        &expected_font,
    );
    assert_residual_evidence(rendered, 1, 0, 1);

    let error = try_render_family(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "lenient-error-font-stack-size-strict",
    )
    .err()
    .expect("RequirePortable must reject the structured Error font-size residual");
    assert!(matches!(
        error,
        merman_render::Error::UnverifiedFamilyTheme {
            family_id: DiagramFamilyId::ERROR,
            residual_count: 1
        }
    ));
}

#[test]
fn lenient_error_keeps_non_typography_flowchart_compatibility_residuals() {
    let family = InheritedTextFamily::Error;
    let theme = compile_flowchart_only_theme(
        None,
        Some(CanvasPaint::solid("#123456").expect("valid Flowchart node fill")),
    );
    let rendered = try_render_family_with_requirement(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "lenient-error-flowchart-node-fill",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort must retain unrelated Flowchart compatibility evidence");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.compatibility_residual_count(), 1);

    let error = try_render_family(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "lenient-error-flowchart-node-fill-strict",
    )
    .err()
    .expect("RequirePortable must retain the unrelated Flowchart contribution");
    assert!(matches!(
        error,
        merman_render::Error::LegacyFamilyThemeCompatibility {
            family_id: DiagramFamilyId::ERROR,
            residual_count: 1
        }
    ));
}

#[test]
fn lenient_error_does_not_reconcile_flowchart_only_typography() {
    let family = InheritedTextFamily::Error;
    let theme = compile_flowchart_only_theme(
        Some(
            ThemeTextStyle::default().with_font_stack(
                FontStack::single("FlowchartOnlyFont")
                    .expect("valid Flowchart-only compatibility font"),
            ),
        ),
        None,
    );
    let rendered = try_render_family_with_requirement(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "lenient-error-cross-family-typography",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort must retain cross-family typography compatibility evidence");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.required_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 1);
}
