use std::sync::Arc;

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemePortabilityRequirement,
    ThemeTextStyle, TypographySpec,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::RailroadDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::{DiagramFamilyId, LayoutOptions};
use serde_json::{Value, json};

const RAILROAD_SOURCE: &str = r#"railroad-beta
expr = sequence(nonterminal("term"), terminal("+"), special("guard")) ;
"#;

fn render_railroad(site_config: Value) -> (String, Value) {
    render_railroad_with_id(site_config, "railroad-theme")
}

fn render_railroad_with_id(site_config: Value, diagram_id: &str) -> (String, Value) {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(RAILROAD_SOURCE, ParseOptions::strict())
        .expect("railroad parse succeeds")
        .expect("railroad diagram is detected");
    let effective_config = parsed.metadata().effective_config.as_value().clone();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("railroad layout succeeds");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("railroad SVG renders");

    (rendered.svg().to_owned(), effective_config)
}

fn try_render_railroad_with_policy(policy: RenderResourcePolicy) -> merman_render::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(RAILROAD_SOURCE, ParseOptions::strict())
        .expect("railroad resource-bound parse succeeds")
        .expect("railroad resource-bound diagram is detected");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("railroad resource-bound render session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("railroad-bounded".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

fn railroad_style(svg: &str) -> &str {
    let start = svg.find("<style>").expect("railroad style element");
    let end = svg[start..]
        .find("</style>")
        .map(|offset| start + offset)
        .expect("railroad style element closes");
    &svg[start..end]
}

fn railroad_rule_body<'a>(style: &'a str, diagram_id: &str, selector: &str) -> &'a str {
    let rule_start = if selector == ".railroad-diagram" {
        format!("#{diagram_id}{selector}{{")
    } else {
        format!("#{diagram_id} {selector}{{")
    };
    let body = style
        .split_once(&rule_start)
        .unwrap_or_else(|| panic!("missing Railroad rule {rule_start:?}"))
        .1;
    let body_end = body.find('}').expect("Railroad CSS rule closes");
    &body[..body_end]
}

fn assert_railroad_font_rule(
    style: &str,
    diagram_id: &str,
    selector: &str,
    expected_font_family: &str,
    expected_font_size: &str,
) {
    let body = railroad_rule_body(style, diagram_id, selector);
    assert!(
        body.contains(&format!("font-family:{expected_font_family};")),
        "Railroad {selector} rule omitted the resolved font stack: {body}"
    );
    assert!(
        body.contains(&format!("font-size:{expected_font_size}px;")),
        "Railroad {selector} rule omitted the resolved font size: {body}"
    );
}

fn railroad_typography_theme(font_stack: FontStack, font_size_px: f32) -> DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_stack(font_stack)
        .with_font_size_px(font_size_px)
        .expect("valid Railroad fixture font size");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::RAILROAD, typography),
        ))
        .expect("compile Railroad typography theme")
}

fn try_render_railroad_source_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    environment: &RenderEnvironment,
    diagram_id: &str,
) -> merman_render::Result<(RailroadDiagramLayout, family::RenderedFamilySvg)> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Railroad")
        .expect("detect themed Railroad");
    let session = environment.begin_session_with_theme(theme).unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let projection = artifact.layout_json()?;
    let layout = serde_json::from_value(projection["layout"]["RailroadDiagram"].clone())
        .expect("Railroad layout projection");
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok((layout, rendered))
}

fn portable_railroad_environment() -> RenderEnvironment {
    RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
}

#[derive(Debug)]
struct RailroadTypographyProbeMeasurer {
    expected_font_family: String,
    expected_font_size: f64,
}

impl RailroadTypographyProbeMeasurer {
    fn assert_style(&self, style: &TextStyle) {
        assert_eq!(
            style.font_family.as_deref(),
            Some(self.expected_font_family.as_str())
        );
        assert_eq!(style.font_size, self.expected_font_size);
    }

    fn width(text: &str) -> f64 {
        text.chars().count() as f64 * 10.0
    }
}

impl TextMeasurer for RailroadTypographyProbeMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.assert_style(style);
        TextMetrics {
            width: Self::width(text),
            height: self.expected_font_size * 2.0,
            line_count: 1,
        }
    }

    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.assert_style(style);
        Self::width(text)
    }

    fn measure_svg_simple_text_bbox_height_px(&self, _text: &str, style: &TextStyle) -> f64 {
        self.assert_style(style);
        self.expected_font_size * 2.0
    }
}

#[test]
fn railroad_svg_scopes_every_family_selector_to_the_root_id() {
    let (svg, _) = render_railroad(json!({}));
    let style = railroad_style(&svg)
        .strip_prefix("<style>")
        .expect("style element prefix");

    for rule in style.split('}').filter(|rule| !rule.is_empty()) {
        let selector = rule
            .split_once('{')
            .map(|(selector, _)| selector)
            .expect("complete CSS rule");
        for selector in selector.split(',') {
            assert!(
                selector.starts_with("#railroad-theme"),
                "Railroad selector escaped the root SVG scope: {selector:?}"
            );
        }
    }
}

#[test]
fn railroad_svg_normalizes_css_significant_characters_in_the_root_id_selector() {
    let (svg, _) = render_railroad_with_id(json!({}), "railroad.theme:one");
    let style = railroad_style(&svg);

    assert!(style.contains(r#"#railroad-theme-one.railroad-diagram{"#));
    assert!(!style.contains(r#"#railroad-theme-one .railroad-diagram{"#));
    assert!(style.contains(r#"#railroad-theme-one .railroad-terminal rect{"#));

    let document = roxmltree::Document::parse(&svg).expect("valid Railroad SVG");
    let root = document.root_element();
    assert_eq!(root.attribute("id"), Some("railroad-theme-one"));
    assert_eq!(root.attribute("class"), Some("railroad-diagram"));
}

#[test]
fn railroad_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let baseline =
        try_render_railroad_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .expect("render the unbounded Railroad baseline");
    let exact_bytes = baseline.len();
    assert!(
        exact_bytes > 1,
        "Railroad fixture must emit a non-empty SVG"
    );

    let exact = try_render_railroad_with_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .expect("valid exact Railroad SVG byte ceiling"),
    )
    .expect("the exact Railroad family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let error = try_render_railroad_with_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .expect("valid below-exact Railroad SVG byte ceiling"),
    )
    .expect_err("one byte below the Railroad family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Railroad MaxSvgBytes rejection, got {error}");
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

fn theme_string<'a>(config: &'a Value, key: &str) -> &'a str {
    config["themeVariables"][key]
        .as_str()
        .unwrap_or_else(|| panic!("themeVariables.{key} should be a string"))
}

#[test]
fn railroad_svg_derives_default_styles_from_the_active_theme() {
    let (svg, effective_config) = render_railroad(json!({}));
    let style = railroad_style(&svg);

    assert!(style.contains(&format!(
        "font-family:{};",
        theme_string(&effective_config, "fontFamily")
    )));
    assert!(style.contains("font-size:16px;"));
    assert!(style.contains(&format!(
        "fill:{};",
        theme_string(&effective_config, "secondBkg")
    )));
    assert!(!style.contains("fill:#FFFFC0;"));
}

#[test]
fn railroad_svg_derives_dark_styles_from_the_active_theme() {
    let (svg, effective_config) = render_railroad(json!({ "theme": "dark" }));
    let style = railroad_style(&svg);

    assert!(style.contains(&format!(
        "fill:{};",
        theme_string(&effective_config, "secondBkg")
    )));
    assert!(style.contains(&format!(
        "stroke:{};",
        theme_string(&effective_config, "secondaryBorderColor")
    )));
    assert!(style.contains(&format!(
        ".railroad-line{{stroke:{};",
        theme_string(&effective_config, "lineColor")
    )));
    assert!(!style.contains("fill:#FFFFC0;"));
}

#[test]
fn railroad_svg_derives_styles_from_custom_theme_variables() {
    let (svg, effective_config) = render_railroad(json!({
        "theme": "base",
        "themeVariables": {
            "fontFamily": "\"Fira Code\", monospace",
            "fontSize": "18px",
            "secondBkg": "oklch(70% 0.1 200)",
            "secondaryBorderColor": "hsl(120, 40%, 50%)",
            "secondaryTextColor": "navy",
            "lineColor": "rebeccapurple"
        }
    }));
    let style = railroad_style(&svg);

    assert_eq!(
        theme_string(&effective_config, "secondBkg"),
        "oklch(70% 0.1 200)"
    );
    assert!(style.contains("font-family:\"Fira Code\", monospace;"));
    assert!(style.contains("font-size:18px;"));
    assert!(style.contains("fill:oklch(70% 0.1 200);"));
    assert!(style.contains("stroke:hsl(120, 40%, 50%);"));
    assert!(style.contains(".railroad-line{stroke:rebeccapurple;"));
}

#[test]
fn railroad_svg_rejects_unsafe_css_and_invalid_numbers() {
    let (svg, effective_config) = render_railroad(json!({
        "theme": "dark",
        "railroad": {
            "fontFamily": "safe\"} .railroad-terminal { display: none; } /*",
            "fontSize": -7,
            "terminalFill": "#fff; stroke: red;",
            "lineColor": "red}</style><script>alert(1)</script>",
            "markerFill": "url(javascript:alert(1))",
            "strokeWidth": "Infinity"
        }
    }));
    let style = railroad_style(&svg);

    for payload in [
        "safe\"} .railroad-terminal { display: none; } /*",
        "#fff; stroke: red;",
        "red}</style><script>alert(1)</script>",
        "url(javascript:alert(1))",
    ] {
        assert!(!svg.contains(payload), "unsafe value survived: {payload}");
    }
    for invalid_css in [
        "display: none",
        "stroke: red",
        "url(javascript:",
        "Infinitypx",
        "-7px",
    ] {
        assert!(
            !style.contains(invalid_css),
            "unsafe CSS survived: {invalid_css}"
        );
    }
    assert!(style.contains(&format!(
        "font-family:{};",
        theme_string(&effective_config, "fontFamily")
    )));
    assert!(style.contains("font-size:16px;"));
    assert!(style.contains(&format!(
        "fill:{};",
        theme_string(&effective_config, "secondBkg")
    )));
    assert!(style.contains(&format!(
        ".railroad-line{{stroke:{};stroke-width:2px;",
        theme_string(&effective_config, "lineColor")
    )));
}

#[test]
fn railroad_typed_font_stack_and_size_drive_layout_css_and_terminal_text_receipt() {
    let font_stack =
        FontStack::new(["Railroad Typed", "monospace"]).expect("valid Railroad fixture font stack");
    let expected_font_family = font_stack.as_css();
    let expected_font_size = 24.0;
    let theme = railroad_typography_theme(font_stack, expected_font_size as f32);
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.railroad-typography-probe").unwrap(),
        "test",
    )
    .unwrap();
    let environment = portable_railroad_environment().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(RailroadTypographyProbeMeasurer {
                expected_font_family: expected_font_family.clone(),
                expected_font_size,
            }),
        )),
    );
    let (layout, rendered) = try_render_railroad_source_with_theme(
        RAILROAD_SOURCE,
        &theme,
        Engine::new(),
        &environment,
        "railroad-typed-font",
    )
    .expect("render directly themed Railroad");

    let rule = &layout.rules[0];
    let terminal = rule
        .elements
        .iter()
        .find(|element| element.kind == "terminal" && element.label == "+")
        .expect("typed Railroad terminal layout");
    let nonterminal = rule
        .elements
        .iter()
        .find(|element| element.kind == "nonterminal" && element.label == "term")
        .expect("typed Railroad nonterminal layout");
    assert_eq!(terminal.width, 30.0);
    assert_eq!(nonterminal.width, 60.0);
    assert_eq!(terminal.height, 68.0);
    assert_eq!(nonterminal.height, 68.0);

    let style = railroad_style(rendered.svg());
    for selector in [
        ".railroad-diagram",
        ".railroad-terminal text",
        ".railroad-nonterminal text",
        ".railroad-comment text",
        ".railroad-special text",
        ".railroad-rule-name",
    ] {
        assert_railroad_font_rule(
            style,
            "railroad-typed-font",
            selector,
            &expected_font_family,
            "24",
        );
    }

    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Railroad SVG");
    for (role, label, expected_width) in [
        ("railroad-terminal", "+", "30"),
        ("railroad-nonterminal", "term", "60"),
    ] {
        let group = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some(role)
                    && node
                        .children()
                        .any(|child| child.has_tag_name("text") && child.text() == Some(label))
            })
            .unwrap_or_else(|| panic!("visible Railroad {role} occurrence"));
        let rect = group
            .children()
            .find(|child| child.has_tag_name("rect"))
            .expect("Railroad text role rectangle");
        assert_eq!(rect.attribute("width"), Some(expected_width));
    }

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn railroad_explicit_font_config_outranks_theme_variables_and_typed_typography() {
    let theme = railroad_typography_theme(
        FontStack::single("Typed Railroad").expect("valid typed Railroad font"),
        24.0,
    );
    let cases = [
        (
            "railroad-theme-variable-font",
            json!({
                "theme": "base",
                "themeVariables": {
                    "fontFamily": "ThemeVariableFont, serif",
                    "fontSize": "18px"
                }
            }),
            "ThemeVariableFont, serif",
            "18",
        ),
        (
            "railroad-explicit-font",
            json!({
                "theme": "base",
                "themeVariables": {
                    "fontFamily": "ThemeVariableFont, serif",
                    "fontSize": "18px"
                },
                "railroad": {
                    "fontFamily": "RailroadExplicit, monospace",
                    "fontSize": 22
                }
            }),
            "RailroadExplicit, monospace",
            "22",
        ),
    ];

    for (diagram_id, site_config, expected_font_family, expected_font_size) in cases {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
        let (_, rendered) = try_render_railroad_source_with_theme(
            RAILROAD_SOURCE,
            &theme,
            engine,
            &portable_railroad_environment(),
            diagram_id,
        )
        .expect("explicit Railroad font configuration remains portable");
        let style = railroad_style(rendered.svg());
        for selector in [
            ".railroad-diagram",
            ".railroad-terminal text",
            ".railroad-nonterminal text",
            ".railroad-comment text",
        ] {
            assert_railroad_font_rule(
                style,
                diagram_id,
                selector,
                expected_font_family,
                expected_font_size,
            );
        }
        assert!(!style.contains("Typed Railroad"));

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
        assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
    }
}

#[test]
fn railroad_font_config_precedence_is_property_local() {
    let theme = railroad_typography_theme(
        FontStack::single("TypedRailroad").expect("valid typed Railroad font"),
        24.0,
    );
    let cases = [
        (
            "railroad-explicit-family-only",
            json!({ "railroad": { "fontFamily": "RailroadExplicit, monospace" } }),
            "RailroadExplicit, monospace",
            "24",
        ),
        (
            "railroad-explicit-size-only",
            json!({ "railroad": { "fontSize": 22 } }),
            "TypedRailroad",
            "22",
        ),
    ];

    for (diagram_id, site_config, expected_font_family, expected_font_size) in cases {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
        let (_, rendered) = try_render_railroad_source_with_theme(
            RAILROAD_SOURCE,
            &theme,
            engine,
            &portable_railroad_environment(),
            diagram_id,
        )
        .expect("the unconfigured Railroad font property remains directly themed");
        assert_railroad_font_rule(
            railroad_style(rendered.svg()),
            diagram_id,
            ".railroad-terminal text",
            expected_font_family,
            expected_font_size,
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
        assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
    }
}

#[test]
fn railroad_direct_font_stack_is_not_limited_by_the_legacy_bridge_budget() {
    let families = (0..32)
        .map(|index| format!("RailroadFont{index:02}{}", "x".repeat(140)))
        .collect::<Vec<_>>();
    let font_stack = FontStack::new(families).expect("valid maximum-width Railroad font stack");
    let expected_css = font_stack.as_css();
    assert!(expected_css.len() > 4 * 1024);
    let theme = railroad_typography_theme(font_stack, 19.0);

    let (_, rendered) = try_render_railroad_source_with_theme(
        RAILROAD_SOURCE,
        &theme,
        Engine::new(),
        &portable_railroad_environment(),
        "railroad-large-font-stack",
    )
    .expect("render Railroad beyond the legacy bridge string budget");
    assert_railroad_font_rule(
        railroad_style(rendered.svg()),
        "railroad-large-font-stack",
        ".railroad-terminal text",
        &expected_css,
        "19",
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn railroad_typed_typography_is_not_applicable_for_an_empty_diagram() {
    let theme = railroad_typography_theme(
        FontStack::single("Railroad Empty").expect("valid empty Railroad font"),
        21.0,
    );
    let (layout, rendered) = try_render_railroad_source_with_theme(
        "railroad-beta\n",
        &theme,
        Engine::new(),
        &portable_railroad_environment(),
        "railroad-empty-font",
    )
    .expect("an empty Railroad typography target remains portable");
    assert!(layout.rules.is_empty());
    assert!(rendered.svg().contains("font-family:\"Railroad Empty\";"));
    assert!(!rendered.svg().contains("<text "));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}
