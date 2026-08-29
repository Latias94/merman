mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
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
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::{DiagramFamilyId, LayoutOptions};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn requirement_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile Requirement fill theme")
}

fn requirement_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default().with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile Requirement stroke theme")
}

fn requirement_fill_and_stroke_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default()
                            .with_fill(fill)
                            .with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile Requirement fill and stroke theme")
}

fn requirement_fill_with_ordinal_palette_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Requirement,
                            ThemeStylePatch::default().with_fill(fill),
                        )
                        .for_family(DiagramFamilyId::REQUIREMENT),
                    )
                    .with_ordinal_palette(
                        ThemeTarget::Requirement,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#123456").expect("valid palette color")
                        ])
                        .expect("non-empty Requirement palette"),
                    ),
            ),
        )
        .expect("compile Requirement fill and ordinal palette theme")
}

fn requirement_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::REQUIREMENT, typography),
        ))
        .expect("compile Requirement typography theme")
}

fn requirement_ordinal_palette_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Requirement,
                    OrdinalPalette::new([
                        ThemeColorValue::parse("#123456").expect("valid palette color")
                    ])
                    .expect("non-empty Requirement palette"),
                ),
            ),
        )
        .expect("compile Requirement ordinal palette theme")
}

fn render_requirement_with_theme(source: &str, theme: &DiagramTheme) -> family::RenderedFamilySvg {
    render_requirement_with_theme_and_engine(source, theme, Engine::new())
}

fn render_requirement_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    render_requirement_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn render_requirement_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> family::RenderedFamilySvg {
    try_render_requirement_with_theme_requirement_and_environment(
        source,
        theme,
        engine,
        portability,
        RenderEnvironment::deterministic(),
    )
    .expect("render themed Requirement")
}

fn render_requirement_with_theme_requirement_and_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> family::RenderedFamilySvg {
    try_render_requirement_with_theme_requirement_and_environment(
        source,
        theme,
        engine,
        portability,
        environment,
    )
    .expect("render themed Requirement")
}

fn try_render_requirement_with_theme_requirement_and_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Requirement")
        .expect("detect themed Requirement");
    let session = environment
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Requirement session");

    family::prepare(parsed, &LayoutOptions::default(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("requirement-theme".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn render_requirement_without_theme(source: &str) -> family::RenderedFamilySvg {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Requirement")
        .expect("detect Requirement");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin Requirement session");

    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Requirement")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("requirement-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Requirement")
}

fn terminal_fill_for(document: &roxmltree::Document<'_>, node_id: &str) -> String {
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Requirement node group {node_id}"));
    requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("stroke") == Some("none")
                && node.attribute("stroke-width") == Some("0")
        })
        .and_then(|path| path.attribute("fill"))
        .unwrap_or_else(|| panic!("missing terminal fill path for {node_id}"))
        .to_string()
}

fn terminal_stroke_for(document: &roxmltree::Document<'_>, node_id: &str) -> String {
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Requirement node group {node_id}"));
    requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("fill") == Some("none")
                && node.attribute("stroke-width") == Some("1.3")
                && node.ancestors().any(|ancestor| {
                    ancestor.attribute("class") == Some("basic label-container outer-path")
                })
        })
        .and_then(|path| path.attribute("stroke"))
        .unwrap_or_else(|| panic!("missing terminal stroke path for {node_id}"))
        .to_string()
}

fn terminal_stroke_geometry_for(document: &roxmltree::Document<'_>, node_id: &str) -> String {
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Requirement node group {node_id}"));
    requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("fill") == Some("none")
                && node.attribute("stroke-width") == Some("1.3")
                && node.ancestors().any(|ancestor| {
                    ancestor.attribute("class") == Some("basic label-container outer-path")
                })
        })
        .and_then(|path| path.attribute("d"))
        .unwrap_or_else(|| panic!("missing terminal stroke geometry for {node_id}"))
        .to_string()
}

fn requirement_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Requirement SVG");
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap_or_else(|| panic!("missing Requirement stylesheet"))
        .to_string()
}

fn requirement_source() -> &'static str {
    r#"requirementDiagram
  requirement req1 {
    id: 1
    text: Themed requirement
    risk: high
    verifymethod: analysis
  }
"#
}

fn same_named_requirement_and_element_source() -> &'static str {
    r#"requirementDiagram
  requirement shared {
    id: R-1
    text: Requirement owner
    risk: low
    verifymethod: analysis
  }
  element shared {
    type: simulation
    docref: Element owner
  }
"#
}

#[derive(Debug)]
struct RequirementTypedFontProbeMeasurer {
    expected_font_family: String,
    typed_calls: Arc<AtomicUsize>,
    sans_serif_fallback_calls: Arc<AtomicUsize>,
}

impl TextMeasurer for RequirementTypedFontProbeMeasurer {
    fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
        match style.font_family.as_deref() {
            Some(actual) if actual == self.expected_font_family => {
                self.typed_calls.fetch_add(1, Ordering::Relaxed);
            }
            Some("sans-serif") => {
                self.sans_serif_fallback_calls
                    .fetch_add(1, Ordering::Relaxed);
            }
            actual => panic!(
                "Requirement layout measurement must use the resolved font winner or Mermaid's explicit sans-serif fallback probe: {actual:?}"
            ),
        }
        TextMetrics {
            width: 100.0,
            height: style.font_size,
            line_count: 1,
        }
    }
}

#[test]
fn requirement_typed_font_stack_reaches_measurement_stylesheet_and_evidence() {
    let font_stack = FontStack::new(["Requirement Typed", "sans-serif"])
        .expect("valid Requirement typed font stack");
    let expected_font = font_stack.as_css();
    let theme = requirement_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.requirement-typed-font-probe").unwrap(),
        "test",
    )
    .unwrap();
    let typed_calls = Arc::new(AtomicUsize::new(0));
    let sans_serif_fallback_calls = Arc::new(AtomicUsize::new(0));
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(RequirementTypedFontProbeMeasurer {
                expected_font_family: expected_font.clone(),
                typed_calls: Arc::clone(&typed_calls),
                sans_serif_fallback_calls: Arc::clone(&sans_serif_fallback_calls),
            }),
        )),
    );
    let rendered = render_requirement_with_theme_requirement_and_environment(
        requirement_source(),
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
        environment,
    );
    assert!(
        typed_calls.load(Ordering::Relaxed) > 0,
        "Requirement layout must measure at least one label with the typed font stack"
    );
    assert!(
        sans_serif_fallback_calls.load(Ordering::Relaxed) > 0,
        "the Mermaid-compatible sans-serif comparison probe should remain observable"
    );
    let css = requirement_stylesheet(rendered.svg());
    for selector in [
        "#requirement-theme",
        "#requirement-theme svg",
        "#requirement-theme .label",
    ] {
        assert!(
            css.contains(&format!("{selector}{{font-family:{expected_font};")),
            "missing resolved Requirement font rule for {selector}: {css}"
        );
    }

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn requirement_typed_font_stack_respects_source_font_family_ownership() {
    let theme = requirement_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("Requirement Typed").expect("valid Requirement font stack"),
    ));
    let rendered = render_requirement_with_theme_and_engine(
        r##"%%{init: {"themeVariables": {"fontFamily": "Source Requirement"}}}%%
requirementDiagram
  requirement req1 {
    id: 1
    text: Source-owned family
    risk: low
    verifymethod: analysis
  }
"##,
        &theme,
        legacy_init_theme_compat_engine(),
    );
    let css = requirement_stylesheet(rendered.svg());
    assert!(
        css.contains("#requirement-theme{font-family:Source Requirement;")
            || css.contains("#requirement-theme svg{font-family:Source Requirement;"),
        "source-owned Requirement font must win in final CSS: {css}"
    );
    assert!(!css.contains("font-family:Requirement Typed"), "{css}");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn requirement_font_size_only_remains_a_legacy_compatibility_route() {
    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Requirement legacy font size"),
    );
    let rendered = render_requirement_with_theme_requirement(
        requirement_source(),
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let css = requirement_stylesheet(rendered.svg());
    assert!(css.contains("#requirement-theme svg{font-family:"));
    assert!(css.contains("font-size:24px"), "{css}");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);

    let error = match try_render_requirement_with_theme_requirement_and_environment(
        requirement_source(),
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
        RenderEnvironment::deterministic(),
    ) {
        Ok(_) => panic!("strict Requirement FontSize must retain compatibility residual"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        merman_render::Error::LegacyFamilyThemeCompatibility {
            family_id: DiagramFamilyId::REQUIREMENT,
            residual_count: 1,
        }
    ));
}

#[test]
fn same_named_requirement_and_element_render_one_default_fill_terminal() {
    let rendered = render_requirement_without_theme(same_named_requirement_and_element_source());
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid default Requirement SVG");
    let node_id = "requirement-theme-shared";

    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("id") == Some(node_id))
            .count(),
        1
    );
    assert_eq!(terminal_fill_for(&document, node_id), "#ECECFF");
}

#[test]
fn same_named_requirement_and_element_share_one_typed_fill_receipt() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid same-name Requirement fill"),
    );
    let rendered =
        render_requirement_with_theme(same_named_requirement_and_element_source(), &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid themed Requirement SVG");
    let node_id = "requirement-theme-shared";

    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("id") == Some(node_id))
            .count(),
        1
    );
    assert_eq!(terminal_fill_for(&document, node_id), "#123456");

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_fill_reaches_terminal_path_and_family_evidence() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Requirement fill"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (fill, expected_fill) in cases {
        let theme = requirement_fill_theme(fill);
        let rendered = render_requirement_with_theme(requirement_source(), &theme);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid themed Requirement SVG");
        assert_eq!(
            terminal_fill_for(&document, "requirement-theme-req1"),
            expected_fill
        );

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
fn requirement_stroke_reaches_terminal_path_and_family_evidence() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Requirement stroke"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    let mut expected_geometry = None;
    for (stroke, expected_stroke) in cases {
        let theme = requirement_stroke_theme(stroke);
        let rendered = render_requirement_with_theme(requirement_source(), &theme);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid themed Requirement SVG");
        assert_eq!(
            terminal_stroke_for(&document, "requirement-theme-req1"),
            expected_stroke
        );
        let geometry = terminal_stroke_geometry_for(&document, "requirement-theme-req1");
        if let Some(expected_geometry) = &expected_geometry {
            assert_eq!(
                &geometry, expected_geometry,
                "Requirement stroke paint must not change terminal path geometry"
            );
        } else {
            expected_geometry = Some(geometry);
        }

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
fn requirement_typed_stroke_remains_the_neo_terminal_winner() {
    let theme = requirement_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid Neo Requirement stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "neo"
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Neo themed Requirement SVG");
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-req1"))
        .expect("Neo Requirement node group");
    let border = requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("fill") == Some("none")
                && node.ancestors().any(|ancestor| {
                    ancestor.attribute("class") == Some("basic label-container outer-path")
                })
        })
        .expect("Neo Requirement border path");
    assert_eq!(border.attribute("stroke"), Some("#123456"));
    assert_eq!(border.attribute("style"), Some("stroke:#123456 !important"));
    let divider = requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.ancestors().any(|ancestor| {
                    ancestor.has_tag_name("g") && ancestor.attribute("class") == Some("divider")
                })
        })
        .expect("Neo Requirement divider path");
    assert_eq!(divider.attribute("stroke"), Some("#123456"));
    assert_eq!(
        divider.attribute("style"),
        Some("stroke:#123456 !important")
    );
    assert!(
        rendered
            .svg()
            .contains(r#"[data-look="neo"].node path{stroke:"#),
        "the regression requires a competing Neo path rule"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_source_class_and_inline_strokes_outrank_typed_stroke_per_node() {
    let source = r#"requirementDiagram
  requirement bare {
    id: 1
    text: Typed owner
    risk: low
    verifymethod: analysis
  }
  requirement assigned {
    id: 2
    text: Class owner
    risk: medium
    verifymethod: test
  }
  element inline {
    type: simulation
    docref: inline-owner
  }
  classDef accent stroke:#00aa00
  class assigned,inline accent
  style inline stroke:#0000aa
"#;
    let theme = requirement_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid mixed Requirement stroke"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid mixed-owner Requirement SVG");

    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-bare"),
        "#123456"
    );
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-assigned"),
        "#00aa00"
    );
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-inline"),
        "#0000aa"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_source_owned_fill_does_not_hide_direct_stroke_evidence() {
    let source = r#"requirementDiagram
  requirement req1 {
    id: 1
    text: Property-local owner
    risk: low
    verifymethod: analysis
  }
  style req1 fill:#00aa00
"#;
    let theme = requirement_fill_and_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid suppressed Requirement fill"),
        CanvasPaint::solid("#123456").expect("valid direct Requirement stroke"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid property-local Requirement SVG");

    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#00aa00"
    );
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-req1"),
        "#123456"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_explicit_default_paint_reaches_terminal_receipt() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default()
                            .with_fill(
                                CanvasPaint::solid("#654321")
                                    .expect("valid explicit-Default Requirement fill"),
                            )
                            .with_stroke(
                                CanvasPaint::solid("#123456")
                                    .expect("valid explicit-Default Requirement stroke"),
                            ),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile explicit Default Requirement paint");
    let rendered = render_requirement_with_theme(requirement_source(), &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid explicit-Default Requirement SVG");
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#654321"
    );
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-req1"),
        "#123456"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_explicit_node_border_outranks_typed_stroke() {
    let theme = requirement_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid config-owned Requirement stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "nodeBorder": "#fedcba" }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid config-owned Requirement SVG");
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-req1"),
        "#fedcba"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_source_class_and_inline_fills_outrank_typed_fill_per_node() {
    let source = r#"requirementDiagram
  requirement bare {
    id: 1
    text: Typed owner
    risk: low
    verifymethod: analysis
  }
  requirement assigned {
    id: 2
    text: Class owner
    risk: medium
    verifymethod: test
  }
  element inline {
    type: simulation
    docref: inline-owner
  }
  classDef accent fill:#00aa00
  class assigned,inline accent
  style inline fill:#0000aa
"#;
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid mixed Requirement fill"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid mixed-owner Requirement SVG");

    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-bare"),
        "#123456"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-assigned"),
        "#00aa00"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-inline"),
        "#0000aa"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_source_styles_reject_css_injection_tokens() {
    let source = r#"requirementDiagram
  requirement unsafe {
    id: 1
    text: Unsafe source style
    risk: low
    verifymethod: analysis
  }
  classDef unsafe fill:#123456;stroke:url(javascript:alert(1))
  class unsafe unsafe
  style unsafe stroke:#654321;fill:red
"#;
    let theme = requirement_fill_and_stroke_theme(
        CanvasPaint::solid("#abcdef").expect("valid Requirement fill"),
        CanvasPaint::solid("#fedcba").expect("valid Requirement stroke"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Requirement SVG");
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-unsafe"))
        .expect("unsafe Requirement node");
    let emitted_styles = requirement
        .descendants()
        .filter_map(|node| node.attribute("style"))
        .collect::<Vec<_>>()
        .join(";");

    assert!(
        !emitted_styles.contains("url("),
        "unsafe URL reached SVG: {emitted_styles}"
    );
    assert!(!emitted_styles.contains("javascript:"));
    assert!(!emitted_styles.contains(";fill:red"));
}

#[test]
fn requirement_fill_is_not_applicable_when_every_node_has_a_source_owner() {
    let source = r#"requirementDiagram
  requirement assigned {
    id: 1
    text: Class owner
    risk: medium
    verifymethod: test
  }
  element inline {
    type: simulation
    docref: inline-owner
  }
  classDef accent fill:#00aa00
  class assigned accent
  style inline fill:#0000aa
"#;
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid suppressed Requirement fill"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid source-owned Requirement SVG");
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-assigned"),
        "#00aa00"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-inline"),
        "#0000aa"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_ordinal_palette_is_not_applicable_when_typed_fill_wins() {
    let theme = requirement_fill_with_ordinal_palette_theme(
        CanvasPaint::solid("#123456").expect("valid typed Requirement fill"),
    );
    let rendered = render_requirement_with_theme(requirement_source(), &theme);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn requirement_ordinal_palette_is_not_applicable_when_source_fill_wins() {
    let source = r#"requirementDiagram
  requirement assigned {
    id: 1
    text: Class owner
    risk: medium
    verifymethod: test
  }
  classDef accent fill:#00aa00
  class assigned accent
"#;
    let rendered = render_requirement_with_theme(source, &requirement_ordinal_palette_theme());
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn requirement_explicit_background_outranks_typed_fill() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid config-owned Requirement fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "requirementBackground": "#fedcba" }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid config-owned Requirement SVG");
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#fedcba"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn extended_dark_primary_color_derived_background_outranks_typed_fill() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid config-derived Requirement fill"),
    );
    let source_owned = format!(
        "%%{{init: {{\"themeVariables\": {{\"primaryColor\": \"#fedcba\"}}}}}}%%\n{}",
        requirement_source()
    );
    let cases = [
        (
            "site-owned neo-dark",
            requirement_source().to_string(),
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "theme": "neo-dark",
                "themeVariables": { "primaryColor": "#fedcba" }
            }))),
            "#fedcba",
            0,
            1,
        ),
        (
            "source-owned redux-dark",
            source_owned,
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "theme": "redux-dark",
                "secure": []
            }))),
            "#123456",
            1,
            0,
        ),
    ];

    for (owner, source, engine, expected_fill, applied_count, not_applicable_count) in cases {
        let rendered = render_requirement_with_theme_and_engine(&source, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("valid {owner} Requirement SVG: {error}"));
        assert_eq!(
            terminal_fill_for(&document, "requirement-theme-req1"),
            expected_fill,
            "{owner} primaryColor must retain the terminal fill"
        );

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{owner}");
        assert_eq!(evidence.applied_count(), applied_count, "{owner}");
        assert_eq!(
            evidence.not_applicable_count(),
            not_applicable_count,
            "{owner}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    }
}

#[test]
fn requirement_border_color_array_owns_stroke_without_hiding_typed_fill() {
    let theme = requirement_fill_and_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid typed Requirement fill"),
        CanvasPaint::solid("#654321").expect("valid typed Requirement stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "borderColorArray": ["#fedcba"]
        }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid borderColorArray Requirement SVG");

    // borderColorArray only owns the stroke surface; the sibling fill remains typed-owned.
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#123456"
    );
    let css = requirement_stylesheet(rendered.svg());
    assert!(
        css.contains(
            r#"#requirement-theme [data-look="classic"][data-color-id="color-0"].node path{stroke:#fedcba;"#
        ),
        "borderColorArray must own the terminal stroke selector"
    );
    assert!(
        !css.contains(
            r#"#requirement-theme [data-look="classic"][data-color-id="color-0"].node path{stroke:#654321;"#
        ),
        "typed stroke must not overwrite the borderColorArray owner"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_color_arrays_own_fill_and_stroke_without_false_applied_evidence() {
    let theme = requirement_fill_and_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid typed Requirement fill"),
        CanvasPaint::solid("#654321").expect("valid typed Requirement stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "borderColorArray": ["#fedcba"],
            "bkgColorArray": ["#abcdef"]
        }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Requirement color-array SVG");

    // The paired Mermaid arrays own both terminal paints. The typed values must not be emitted
    // as inline winners, otherwise the CSS palette would be visually and evidentially bypassed.
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#ECECFF"
    );
    let css = requirement_stylesheet(rendered.svg());
    let selector =
        r#"#requirement-theme [data-look="classic"][data-color-id="color-0"].node path{"#;
    assert!(
        css.contains(&format!("{selector}stroke:#fedcba;fill:#abcdef;")),
        "paired arrays must own the Requirement stroke and fill selectors"
    );
    assert!(
        !css.contains("#123456") && !css.contains("#654321"),
        "typed paint must not be re-emitted as a competing stylesheet winner"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}
