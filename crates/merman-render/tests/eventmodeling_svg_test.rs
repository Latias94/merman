mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    OrdinalSelector, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::EventModelingDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};

const THEME_TEXT_SOURCE: &str = r#"eventmodeling
tf 01 ui View
tf 02 cmd Run ->> 01
tf 03 evt Done ->> 02
"#;

fn eventmodeling_text_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::EVENT_MODELING),
                ),
            ),
        )
        .expect("compile Event Modeling text fill theme")
}

fn eventmodeling_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default()
                    .with_family_style(DiagramFamilyId::EVENT_MODELING, typography),
            ),
        )
        .expect("compile Event Modeling typography theme")
}

fn eventmodeling_stylesheet(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .expect("valid Event Modeling SVG")
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Event Modeling stylesheet")
        .to_string()
}

fn render_eventmodeling_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> merman_render::Result<family::RenderedFamilySvg> {
    render_eventmodeling_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn render_eventmodeling_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    render_eventmodeling_with_theme_requirement_and_id(
        source,
        theme,
        engine,
        portability,
        "eventmodeling-theme",
    )
}

fn render_eventmodeling_with_theme_requirement_and_id(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Event Modeling")
        .expect("detect themed Event Modeling");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Event Modeling session");

    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn eventmodeling_layout_json_with_theme(theme: &DiagramTheme) -> serde_json::Value {
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(THEME_TEXT_SOURCE, ParseOptions::strict())
        .expect("parse themed Event Modeling layout")
        .expect("detect themed Event Modeling layout");
    let session = RenderEnvironment::deterministic()
        .begin_session_with_theme(theme)
        .expect("begin themed Event Modeling layout session");
    let projection = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Event Modeling layout")
        .layout_json()
        .expect("serialize themed Event Modeling layout");
    projection
        .pointer("/layout/EventModelingDiagram")
        .cloned()
        .expect("Event Modeling layout projection")
}

fn style_property<'a>(style: &'a str, expected_property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case(expected_property))
        .map(|(_, value)| value.trim())
        .next_back()
}

fn eventmodeling_terminal_text_values(svg: &str) -> (Vec<String>, Vec<String>) {
    let document = roxmltree::Document::parse(svg).expect("valid Event Modeling SVG");
    let swimlane_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node.parent().is_some_and(|parent| {
                    parent.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|c| c == "em-swimlane")
                    })
                })
                && node
                    .descendants()
                    .filter_map(|descendant| descendant.text())
                    .any(|text| !text.trim().is_empty())
        })
        .map(|node| {
            node.attribute("fill")
                .unwrap_or_else(|| panic!("missing Event Modeling swimlane fill: {svg}"))
                .to_string()
        })
        .collect();
    let box_colors = document
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "span"
                && node.ancestors().any(|ancestor| {
                    ancestor
                        .attribute("class")
                        .is_some_and(|class| class.split_ascii_whitespace().any(|c| c == "em-box"))
                })
                && node
                    .descendants()
                    .filter_map(|descendant| descendant.text())
                    .any(|text| !text.trim().is_empty())
        })
        .map(|node| {
            let style = node
                .attribute("style")
                .unwrap_or_else(|| panic!("missing Event Modeling box span style: {svg}"));
            style_property(style, "color")
                .unwrap_or_else(|| panic!("missing Event Modeling box span color: {svg}"))
                .to_string()
        })
        .collect();
    (swimlane_fills, box_colors)
}

fn try_render_eventmodeling_svg_with_resource_policy(
    source: &str,
    policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Event Modeling resource-bound fixture")
        .expect("detect Event Modeling diagram");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    Ok(artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("eventmodeling-bounded".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )?
        .svg()
        .to_owned())
}

#[test]
fn eventmodeling_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "eventmodeling\ntf 01 event Start\ntf 02 cmd Continue ->> 01 { id: 42 }\n";
    let baseline = try_render_eventmodeling_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render unbounded Event Modeling SVG");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1);

    let exact = try_render_eventmodeling_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .expect("valid exact Event Modeling SVG ceiling"),
    )
    .expect("exact Event Modeling SVG ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let error = try_render_eventmodeling_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .expect("valid below-exact Event Modeling SVG ceiling"),
    )
    .expect_err("one byte below the Event Modeling SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Event Modeling MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
}

#[test]
fn eventmodeling_typed_text_fill_reaches_svg_and_xhtml_terminals() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Event Modeling text fill"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (fill, expected) in cases {
        let theme = eventmodeling_text_fill_theme(fill);
        let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, Engine::new())
            .expect("render strict portable Event Modeling text fill");
        let (swimlane_fills, box_colors) = eventmodeling_terminal_text_values(rendered.svg());

        assert_eq!(swimlane_fills, vec![expected; 3]);
        assert_eq!(box_colors, vec![expected; 3]);

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn eventmodeling_explicit_text_color_outranks_typed_text_fill() {
    let theme = eventmodeling_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Event Modeling text fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "textColor": "#fedcba" }
    })));
    let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, engine)
        .expect("render site-owned Event Modeling text fill");
    let (swimlane_fills, box_colors) = eventmodeling_terminal_text_values(rendered.svg());

    assert_eq!(swimlane_fills, vec!["#fedcba"; 3]);
    assert_eq!(box_colors, vec!["#fedcba"; 3]);

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn eventmodeling_typed_font_stack_reaches_both_text_selectors_and_strict_receipt() {
    let font_stack = FontStack::new(["EventModelingTyped", "monospace"])
        .expect("valid Event Modeling typed font stack");
    let expected_font = font_stack.as_css().to_string();
    let theme =
        eventmodeling_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, Engine::new())
        .expect("render typed Event Modeling font stack");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());

    assert!(stylesheet.contains(&format!(
        "#eventmodeling-theme .em-swimlane text,#eventmodeling-theme .em-box span {{ font-family: {expected_font};"
    )));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn eventmodeling_theme_font_family_keeps_precedence_over_root_font_family() {
    let theme = eventmodeling_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("EventModelingTyped").expect("valid Event Modeling typed font"),
    ));
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "fontFamily": "EventRootFont",
        "themeVariables": { "fontFamily": "EventThemeFont" }
    })));
    let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, engine)
        .expect("render config-owned Event Modeling font stack");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());

    assert!(stylesheet.contains("font-family: EventThemeFont;"));
    assert!(!stylesheet.contains("EventRootFont"));
    assert!(!stylesheet.contains("EventModelingTyped"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn eventmodeling_typed_font_size_reaches_css_and_strict_receipt() {
    let theme = eventmodeling_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Event Modeling font size"),
    );
    let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, Engine::new())
        .expect("render typed Event Modeling font size");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());
    assert!(stylesheet.contains("font-size: 24px;"), "{stylesheet}");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn eventmodeling_styles_are_scoped_per_inline_svg() {
    let source = "eventmodeling\ntf 01 ui View\n";
    let theme_a = eventmodeling_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(12.0)
            .expect("valid Event Modeling font size"),
    );
    let theme_b = eventmodeling_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Event Modeling font size"),
    );
    let rendered_a = render_eventmodeling_with_theme_requirement_and_id(
        source,
        &theme_a,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
        "event-a",
    )
    .expect("render first scoped Event Modeling SVG");
    let rendered_b = render_eventmodeling_with_theme_requirement_and_id(
        source,
        &theme_b,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
        "event-b",
    )
    .expect("render second scoped Event Modeling SVG");

    let stylesheet_a = eventmodeling_stylesheet(rendered_a.svg());
    let stylesheet_b = eventmodeling_stylesheet(rendered_b.svg());
    assert!(
        stylesheet_a.contains("#event-a .em-swimlane text,#event-a .em-box span { font-family:")
    );
    assert!(stylesheet_a.contains("font-size: 12px;"));
    assert!(stylesheet_a.contains("#event-a .em-relation { fill: none; }"));
    assert!(!stylesheet_a.contains("#event-b "));
    assert!(
        stylesheet_b.contains("#event-b .em-swimlane text,#event-b .em-box span { font-family:")
    );
    assert!(stylesheet_b.contains("font-size: 24px;"));
    assert!(stylesheet_b.contains("#event-b .em-relation { fill: none; }"));
    assert!(!stylesheet_b.contains("#event-a "));
}

#[test]
fn eventmodeling_css_font_size_uses_theme_variable_not_root_layout_size() {
    let theme = eventmodeling_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("EventModelingTyped").expect("valid Event Modeling typed font"),
    ));
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "fontSize": "31px",
        "themeVariables": { "fontSize": "22px" }
    })));
    let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, engine)
        .expect("render Event Modeling split font-size configuration");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());

    assert!(stylesheet.contains("font-size: 22px;"), "{stylesheet}");
    assert!(!stylesheet.contains("font-size: 31px;"), "{stylesheet}");
}

#[test]
fn eventmodeling_materialized_theme_font_size_reaches_css_without_explicit_ownership() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty Event Modeling theme");
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "theme": "redux"
    })));
    let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, engine)
        .expect("render Redux Event Modeling font size");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());

    assert!(stylesheet.contains("font-size: 14px;"), "{stylesheet}");
}

#[test]
fn eventmodeling_mixed_typography_is_direct_and_portable() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::single("EventModelingMixed").expect("valid mixed Event Modeling font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid mixed Event Modeling font size");
    let theme = eventmodeling_typography_theme(typography);
    let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, Engine::new())
        .expect("render mixed Event Modeling typography");
    let metadata = rendered.metadata();
    assert!(!merman_core::__private::fallback_overlay_owns_path(
        &metadata.effective_config,
        "themeVariables.fontSize"
    ));
    assert!(!merman_core::__private::fallback_overlay_owns_path(
        &metadata.effective_config,
        "themeVariables.fontFamily"
    ));
    assert!(!merman_core::__private::fallback_overlay_owns_path(
        &metadata.effective_config,
        "fontFamily"
    ));
    let stylesheet = eventmodeling_stylesheet(rendered.svg());
    assert!(stylesheet.contains("EventModelingMixed"));
    assert!(stylesheet.contains("font-size: 24px;"), "{stylesheet}");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn eventmodeling_explicit_theme_font_size_outranks_typed_size_but_root_size_does_not() {
    let theme = eventmodeling_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Event Modeling font size"),
    );
    let config_owned = render_eventmodeling_with_theme(
        THEME_TEXT_SOURCE,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "themeVariables": { "fontSize": "22px" }
        }))),
    )
    .expect("render config-owned Event Modeling font size");
    let config_stylesheet = eventmodeling_stylesheet(config_owned.svg());
    assert!(
        config_stylesheet.contains("font-size: 22px;"),
        "{config_stylesheet}"
    );
    assert!(!config_stylesheet.contains("font-size: 24px;"));
    let evidence =
        merman_render::__private::family_evidence(config_owned.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);

    let root_owned = render_eventmodeling_with_theme(
        THEME_TEXT_SOURCE,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "fontSize": "31px"
        }))),
    )
    .expect("render Event Modeling typed size with root layout size");
    let root_stylesheet = eventmodeling_stylesheet(root_owned.svg());
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
fn eventmodeling_typed_font_size_does_not_change_headless_layout() {
    let small = eventmodeling_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(12.0)
            .expect("valid small Event Modeling font size"),
    );
    let large = eventmodeling_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(48.0)
            .expect("valid large Event Modeling font size"),
    );

    assert_eq!(
        eventmodeling_layout_json_with_theme(&small),
        eventmodeling_layout_json_with_theme(&large),
        "Event Modeling FontSize is a final CSS terminal and must not alter pinned layout metrics"
    );
}

#[test]
fn eventmodeling_font_size_is_not_applicable_without_visible_text_terminals() {
    let theme = eventmodeling_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Event Modeling font size"),
    );
    let rendered = render_eventmodeling_with_theme("eventmodeling\n", &theme, Engine::new())
        .expect("render empty Event Modeling typography domain");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());

    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn eventmodeling_unsupported_typography_sibling_keeps_direct_font_size() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(24.0)
        .expect("valid Event Modeling font size")
        .with_font_weight(700)
        .expect("valid unsupported Event Modeling font weight");
    let theme = eventmodeling_typography_theme(typography);
    let rendered = render_eventmodeling_with_theme_requirement(
        THEME_TEXT_SOURCE,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders unsupported Event Modeling typography");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());
    assert!(stylesheet.contains("font-size: 24px;"), "{stylesheet}");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());

    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn eventmodeling_ordinal_palette_is_not_applicable_when_typed_fill_wins() {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#abcdef").expect("valid Event Modeling palette color")
    ])
    .expect("non-empty Event Modeling text palette");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456")
                                    .expect("valid Event Modeling text fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::EVENT_MODELING),
                    )
                    .with_ordinal_palette(ThemeTarget::Text, palette),
            ),
        )
        .expect("compile Event Modeling text fill and ordinal palette theme");
    let rendered = render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, Engine::new())
        .expect("typed Event Modeling text fill must shadow the unsupported palette");
    let (swimlane_fills, box_colors) = eventmodeling_terminal_text_values(rendered.svg());

    assert_eq!(swimlane_fills, vec!["#123456"; 3]);
    assert_eq!(box_colors, vec!["#123456"; 3]);

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn eventmodeling_empty_terminal_domain_marks_text_fill_not_applicable() {
    let theme = eventmodeling_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Event Modeling text fill"),
    );
    let rendered = render_eventmodeling_with_theme("eventmodeling\n", &theme, Engine::new())
        .expect("render empty Event Modeling terminal domain");
    let (swimlane_fills, box_colors) = eventmodeling_terminal_text_values(rendered.svg());

    assert!(swimlane_fills.is_empty());
    assert!(box_colors.is_empty());

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn eventmodeling_ordinal_text_fill_fails_closed() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#123456")
                                .expect("valid Event Modeling ordinal text fill"),
                        ),
                    )
                    .with_ordinal(OrdinalSelector::exact(2).expect("valid text ordinal"))
                    .for_family(DiagramFamilyId::EVENT_MODELING),
                ),
            ),
        )
        .expect("compile ordinal Event Modeling text fill theme");
    let error = match render_eventmodeling_with_theme(THEME_TEXT_SOURCE, &theme, Engine::new()) {
        Ok(_) => panic!("unsupported Event Modeling ordinal text fill must fail closed"),
        Err(error) => error,
    };

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::EVENT_MODELING, 1))
    );
}

#[test]
fn eventmodeling_typed_render_model_outputs_svg() {
    let input = r##"---
config:
  eventmodeling:
    padding: 24
    useMaxWidth: true
  themeVariables:
    emRelationStroke: '#135790'
    emCommandFill: '#DDEEFF'
    emCommandStroke: '#336699'
    textColor: '#111111'
---
eventmodeling
tf 01 ui Web.ShopCart
tf 02 cmd Cart.AddItem ->> 01 { sku: "SKU-1" }
tf 03 evt Cart.ItemAdded ->> 02 [[ItemAddedData]]
rf 04 rmo Cart.Summary
tf 05 evt Cart.CheckedOut

data ItemAddedData {
  sku: "SKU-1"
  quantity: 1
}
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "eventmodeling");

    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("eventmodeling-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    let svg = rendered.svg();

    assert!(svg.contains(r#"aria-roledescription="eventmodeling""#));
    assert!(svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"max-width:"#));
    assert!(svg.contains(r#"<g/><g class="em-swimlane">"#));
    assert!(svg.contains(r#"class="em-relation""#));
    assert!(svg.contains(r#"class="em-box""#));
    assert!(svg.contains(r#"id="em-arrowhead-eventmodeling-test""#));
    assert!(svg.contains(r#"font-family: "trebuchet ms",verdana,arial,sans-serif;"#));
    assert!(svg.contains(r#"color: #111111;"#));
    assert!(svg.contains(r##"stroke="#135790""##));
    assert!(svg.contains(r##"fill="#DDEEFF""##));
}

#[test]
fn eventmodeling_svg_wires_accessibility_metadata_to_the_root() {
    let input = r#"eventmodeling
accTitle: Accessible event model
accDescr {
  Event model description
}
tf 01 event Start
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse eventmodeling")
        .expect("detect eventmodeling");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("eventmodeling-a11y".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render eventmodeling")
        .svg()
        .to_owned();

    assert!(svg.contains(r#"aria-labelledby="chart-title-eventmodeling-a11y""#));
    assert!(svg.contains(r#"aria-describedby="chart-desc-eventmodeling-a11y""#));
    assert!(
        svg.contains(
            r#"<title id="chart-title-eventmodeling-a11y">Accessible event model</title>"#
        )
    );
    assert!(
        svg.contains(r#"<desc id="chart-desc-eventmodeling-a11y">Event model description</desc>"#)
    );
}

#[test]
fn eventmodeling_docs_minimum_layout_uses_bounded_deterministic_label_metrics() {
    let input =
        include_str!("../../../fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact
        .layout_json()
        .expect("serialize EventModeling layout");
    let layout: EventModelingDiagramLayout =
        serde_json::from_value(projection["layout"]["EventModelingDiagram"].clone())
            .expect("EventModeling layout projection");

    assert_eq!(layout.boxes.len(), 5);
    assert!(!layout.swimlanes.is_empty());
    assert!(!layout.relations.is_empty());
    assert!(
        layout
            .boxes
            .iter()
            .all(|box_layout| box_layout.width.is_finite()
                && box_layout.height.is_finite()
                && (100.0..=470.0).contains(&box_layout.width)
                && (100.0..=770.0).contains(&box_layout.height)),
        "EventModeling boxes must stay inside the source-defined min/max geometry: {:?}",
        layout.boxes
    );
    assert!(
        layout.boxes[2].width > layout.boxes[0].width,
        "the data-rich event must remain wider than the short UI frame: {:?}",
        layout.boxes
    );
    assert!(
        layout.boxes[2].height >= layout.boxes[0].height,
        "the data-rich event must not become shorter than the short UI frame: {:?}",
        layout.boxes
    );
    let rightmost_box = layout
        .boxes
        .iter()
        .map(|box_layout| box_layout.x + box_layout.width)
        .fold(0.0_f64, f64::max);
    assert!(layout.total_width > rightmost_box);
    assert!(layout.total_height > 0.0);
}
