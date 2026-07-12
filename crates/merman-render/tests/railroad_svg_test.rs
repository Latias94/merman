use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::svg::{SvgRenderOptions, render_layout_svg_parts_for_render_model_with_config};
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};
use serde_json::{Value, json};

const RAILROAD_SOURCE: &str = r#"railroad-beta
expr = sequence(nonterminal("term"), terminal("+"), special("guard")) ;
"#;

fn render_railroad(site_config: Value) -> (String, Value) {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(RAILROAD_SOURCE, ParseOptions::strict())
        .expect("railroad parse succeeds")
        .expect("railroad diagram is detected");
    let layout_options = LayoutOptions::headless_svg_defaults();
    let layout = layout_parsed_render_layout_only(&parsed, &layout_options)
        .expect("railroad layout succeeds");
    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("railroad-theme".to_string()),
            ..Default::default()
        },
    )
    .expect("railroad SVG renders");

    (svg, parsed.meta.effective_config.as_value().clone())
}

fn railroad_style(svg: &str) -> &str {
    let start = svg
        .find("<style>.railroad-diagram")
        .expect("railroad style element");
    let end = svg[start..]
        .find("</style>")
        .map(|offset| start + offset)
        .expect("railroad style element closes");
    &svg[start..end]
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
