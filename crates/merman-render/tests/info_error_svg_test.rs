use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};
use std::sync::Arc;

struct WideInheritedFontHost;

impl HostTextMeasurer for WideInheritedFontHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        if request.style.font_family.as_deref() != Some("WideInheritedFont") {
            return Ok(None);
        }
        Ok(match request.operation {
            TextMeasurementOperation::BBoxX => {
                let half_width = if request.text == "Syntax error in text" {
                    1_800.0
                } else if request.text.starts_with("mermaid version ") {
                    1_400.0
                } else if request.text.starts_with('v') {
                    300.0
                } else {
                    return Ok(None);
                };
                Some(HostTextMeasurement::HorizontalExtents {
                    left: half_width,
                    right: half_width,
                })
            }
            TextMeasurementOperation::RawBBoxHeight => {
                Some(HostTextMeasurement::Length(request.style.font_size))
            }
            _ => None,
        })
    }
}

fn wide_inherited_font_environment() -> RenderEnvironment {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.wide-inherited-font").expect("valid profile id"),
        "1",
    )
    .expect("valid profile identity");
    RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(
            identity,
            Arc::new(WideInheritedFontHost),
            [TextMeasurementPhase::SvgBBox],
        ),
    )
}

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
            Self::Error => &["150px", "100px", "42px", "42px"],
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

fn compile_flowchart_node_fill_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#123456").expect("valid Flowchart node fill"),
                        ),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        )
        .expect("compile Flowchart-only typed theme")
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
    let engine = merman_render::__private::install_parse_compatibility(theme, engine);
    try_render_family_with_preinstalled_parse_compatibility(
        family,
        source,
        theme,
        engine,
        diagram_id,
        requirement,
    )
}

fn try_render_family_with_preinstalled_parse_compatibility(
    family: InheritedTextFamily,
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_family_with_environment(
        family,
        source,
        theme,
        engine,
        diagram_id,
        requirement,
        RenderEnvironment::deterministic(),
    )
}

fn try_render_family_with_environment(
    family: InheritedTextFamily,
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
    requirement: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, family.parse_options())
        .expect("parse inherited text-family source")
        .expect("detect inherited text-family source");
    let session = environment
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

fn root_max_width_px(svg: &str) -> f64 {
    let document = roxmltree::Document::parse(svg).expect("valid inherited-family SVG");
    document
        .root_element()
        .attribute("style")
        .expect("root max-width style")
        .split(';')
        .map(str::trim)
        .find_map(|declaration| declaration.strip_prefix("max-width:"))
        .map(str::trim)
        .and_then(|value| value.strip_suffix("px"))
        .and_then(|value| value.parse::<f64>().ok())
        .expect("numeric root max-width")
}

fn root_view_box(svg: &str) -> [f64; 4] {
    let document = roxmltree::Document::parse(svg).expect("valid inherited-family SVG");
    document
        .root_element()
        .attribute("viewBox")
        .expect("root viewBox")
        .split_whitespace()
        .map(|value| value.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>()
        .try_into()
        .expect("four viewBox components")
}

fn text_x(svg: &str, text: &str) -> f64 {
    let document = roxmltree::Document::parse(svg).expect("valid inherited-family SVG");
    document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some(text))
        .and_then(|node| node.attribute("x"))
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing numeric x for {text:?}"))
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

fn assert_single_info_text_fill(
    rendered: family::RenderedFamilySvg,
    expected_fill: Option<&str>,
    expected_applied: usize,
    expected_not_applicable: usize,
) {
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Info SVG");
    let version = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "version")
                })
                && node.text().is_some_and(|text| !text.trim().is_empty())
        })
        .expect("one visible Info version terminal");
    assert_eq!(version.attribute("fill"), expected_fill);

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

#[test]
fn info_transparent_text_fill_is_owned_by_the_same_terminal() {
    let family = InheritedTextFamily::Info;
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                    )
                    .for_family(family.family_id()),
                ),
            ),
        )
        .expect("compile transparent Info text theme");
    let rendered = try_render_family(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "info-transparent-text-fill",
    )
    .expect("transparent Info text must remain portable");

    assert_single_info_text_fill(rendered, Some("transparent"), 1, 0);
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
fn info_text_fill_is_written_and_proved_by_the_version_terminal() {
    let family = InheritedTextFamily::Info;
    let theme = inherited_paint_theme(family, &[ThemeTarget::Text]);
    let rendered = try_render_family(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "info-direct-text-fill",
    )
    .expect("Info must directly own its visible Text.fill terminal");

    assert_single_info_text_fill(rendered, Some("#123456"), 1, 0);
}

#[test]
fn info_text_palette_respects_static_fill_and_config_ownership() {
    for case in ["static", "default", "config", "uncovered"] {
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
            .for_family(DiagramFamilyId::INFO);
            if case == "default" {
                rule = rule.with_variant(ThemeVariant::Default);
            }
            styles = styles.with_rule(rule);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile Info palette theme");
        let engine = if case == "config" {
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "textColor": "#123456" }
            })))
        } else {
            Engine::new()
        };
        let family = InheritedTextFamily::Info;
        let result =
            try_render_family(family, family.base_source(), &theme, engine, "info-palette");
        if case == "uncovered" {
            let error = result
                .err()
                .expect("uncovered palette must remain unsupported");
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::INFO, 1))
            );
            assert_eq!(error.incomplete_family_theme(), None);
            continue;
        }
        let rendered = result.unwrap_or_else(|error| panic!("{case}: {error}"));
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Info SVG");
        let version = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("version"))
            .expect("visible version text");
        assert_eq!(
            version.attribute("fill"),
            (case != "config").then_some("#123456"),
            "{case}"
        );
        if case == "config" {
            assert!(
                rendered.svg().contains("#123456"),
                "config color must reach stylesheet"
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(
            evidence.required_count(),
            evidence.accounted_count(),
            "{case}"
        );
        assert_eq!(
            evidence.applied_count(),
            usize::from(case != "config"),
            "{case}"
        );
        assert_eq!(evidence.not_applicable_count(), 1, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn info_explicit_default_text_fill_is_typed_and_verified() {
    let family = InheritedTextFamily::Info;
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#2563eb")
                                .expect("valid explicit-Default Info text fill"),
                        ),
                    )
                    .for_family(family.family_id())
                    .with_variant(ThemeVariant::Default),
                ),
            ),
        )
        .expect("compile explicit-Default Info text theme");
    let rendered = try_render_family(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "info-explicit-default-text-fill",
    )
    .expect("explicit-Default Info text fill must remain portable");

    assert_single_info_text_fill(rendered, Some("#2563eb"), 1, 0);
}

#[test]
fn info_config_text_color_outranks_typed_text_fill_per_property() {
    let family = InheritedTextFamily::Info;
    let theme = inherited_paint_theme(family, &[ThemeTarget::Text]);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "textColor": "#abcdef" }
    })));
    let rendered = try_render_family(
        family,
        family.base_source(),
        &theme,
        engine,
        "info-config-owned-text-fill",
    )
    .expect("explicit Info textColor ownership must remain portable");

    assert_single_info_text_fill(rendered, None, 0, 1);
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
fn info_and_error_direct_font_stack_drives_terminal_geometry_and_canvas_bounds() {
    for family in InheritedTextFamily::ALL {
        let theme = inherited_font_stack_theme(
            family,
            FontStack::single("WideInheritedFont").expect("valid wide inherited font"),
        );
        let diagram_id = format!("{}-wide-inherited-font", family.aria_roledescription());
        let engine = merman_render::__private::install_parse_compatibility(&theme, Engine::new());
        let rendered = try_render_family_with_environment(
            family,
            family.base_source(),
            &theme,
            engine,
            &diagram_id,
            ThemePortabilityRequirement::RequirePortable,
            wide_inherited_font_environment(),
        )
        .expect("render inherited text family with host-measured typed font");

        match family {
            InheritedTextFamily::Info => {
                assert!(
                    root_max_width_px(rendered.svg()) >= 1_200.0,
                    "Info canvas must expand for the measured terminal font: {}",
                    rendered.svg()
                );
                assert!(
                    text_x(
                        rendered.svg(),
                        &format!(
                            "v{}",
                            merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION
                        )
                    ) >= 300.0,
                    "Info terminal anchor must follow the prepared canvas geometry"
                );
            }
            InheritedTextFamily::Error => {
                let view_box = root_view_box(rendered.svg());
                assert!(
                    view_box[2] >= 4_400.0,
                    "Error viewBox must expand for the measured terminal font: {view_box:?}"
                );
                assert!(
                    root_max_width_px(rendered.svg()) > 512.0,
                    "Error max-width must preserve the expanded viewBox scale"
                );
                assert!(
                    text_x(rendered.svg(), "Syntax error in text") > 1_440.0,
                    "Error message anchor must follow the prepared canvas geometry"
                );
            }
        }

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
    assert_residual_evidence(rendered, 2, 1, 1);

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
fn lenient_error_does_not_invent_compatibility_for_typed_flowchart_node_fill() {
    let family = InheritedTextFamily::Error;
    let theme = compile_flowchart_node_fill_theme();
    let rendered = try_render_family(
        family,
        family.base_source(),
        &theme,
        Engine::new(),
        "lenient-error-typed-flowchart-node-fill",
    )
    .expect("a retired projection must not create a phantom compatibility residual");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}
