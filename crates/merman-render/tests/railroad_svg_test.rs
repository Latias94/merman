use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
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

fn railroad_style(svg: &str) -> &str {
    let start = svg.find("<style>").expect("railroad style element");
    let end = svg[start..]
        .find("</style>")
        .map(|offset| start + offset)
        .expect("railroad style element closes");
    &svg[start..end]
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
fn railroad_svg_escapes_css_significant_characters_in_the_root_id_selector() {
    let (svg, _) = render_railroad_with_id(json!({}), "railroad.theme:one");
    let style = railroad_style(&svg);

    assert!(style.contains(r#"#railroad\.theme\:one.railroad-diagram{"#));
    assert!(style.contains(r#"#railroad\.theme\:one .railroad-terminal rect{"#));
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
