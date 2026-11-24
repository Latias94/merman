use std::sync::Arc;

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
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
use merman_render::theme::{NativeTheme, ThemeCompiler};
use serde_json::{Value, json};

const RAILROAD_SOURCE: &str = r#"railroad-beta
expr = sequence(nonterminal("term"), terminal("+"), special("guard")) ;
"#;
fn railroad_core_dialect_sources() -> [&'static str; 4] {
    [
        RAILROAD_SOURCE,
        include_str!("../../../fixtures/railroadAbnf/local_numeric_groups_bounds.mmd"),
        include_str!("../../../fixtures/railroadEbnf/local_exception_group_postfix.mmd"),
        include_str!("../../../fixtures/railroadPeg/local_predicate_group_suffix.mmd"),
    ]
}

#[test]
fn native_theme_v2_railroad_measures_code_and_paints_all_dialects() {
    struct TerminalMeasurer;

    impl TextMeasurer for TerminalMeasurer {
        fn measure(&self, _: &str, style: &TextStyle) -> TextMetrics {
            let width = if style
                .font_family
                .as_ref()
                .is_some_and(|font| font.contains("Track Mono"))
            {
                101.0
            } else {
                21.0
            };
            TextMetrics {
                width,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.emphasis":"#305070","text.on_emphasis":"#FFFFFF",
                         "surface.node":"#E0F0D0","stroke.connector":"#506070"},
                "typography":{"mono":["Track Mono","monospace"],"base_size_px":18},
                "form":{"radius":5,"stroke_width":2,"connector_width":3}
            }"##,
            None,
        )
        .unwrap();
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("railroad-native-v2-code").unwrap(),
            "test",
        )
        .unwrap(),
        Arc::new(TerminalMeasurer),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    for body in railroad_core_dialect_sources() {
        let source =
            format!("---\nconfig:\n  railroad:\n    nonTerminalFill: '#AABBCC'\n---\n{body}");
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let prepared = family::prepare(
            parsed.presentation(),
            &LayoutOptions::default(),
            environment
                .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
                .unwrap(),
        )
        .unwrap();
        let projection = prepared.layout_json().unwrap();
        let layout: RailroadDiagramLayout =
            serde_json::from_value(projection["layout"]["RailroadDiagram"].clone()).unwrap();
        let terminal = layout
            .rules
            .iter()
            .flat_map(|rule| &rule.elements)
            .find(|element| element.kind == "terminal")
            .unwrap();
        assert_eq!(terminal.width, 121.0);

        let rendered = prepared
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let svg = rendered.svg();
        let css = railroad_style(svg);
        assert!(css.contains(".railroad-terminal rect{fill:#305070;"));
        assert!(css.contains(".railroad-terminal text{fill:#FFFFFF;font-family:\"Track Mono\", monospace;font-size:18px;"));
        assert!(css.contains(".railroad-nonterminal rect{fill:#AABBCC;"));
        assert!(css.contains(".railroad-line{stroke:#506070;stroke-width:3px;fill:none;}"));
        let dom = roxmltree::Document::parse(svg).unwrap();
        assert!(dom.descendants().any(|node| {
            node.has_tag_name("rect")
                && node.attribute("rx") == Some("5")
                && node
                    .parent_element()
                    .is_some_and(|parent| parent.attribute("class") == Some("railroad-terminal"))
        }));
    }
}

fn render_railroad(site_config: Value) -> (String, Value) {
    render_railroad_with_id(site_config, "railroad-theme")
}

fn render_railroad_with_id(site_config: Value, diagram_id: &str) -> (String, Value) {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(site_config),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(RAILROAD_SOURCE, ParseOptions::strict())
        .expect("railroad parse succeeds")
        .expect("railroad diagram is detected")
        .into_presentation();
    let effective_config = parsed.metadata().effective_config.as_value().clone();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
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
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(RAILROAD_SOURCE, ParseOptions::strict())
        .expect("railroad resource-bound parse succeeds")
        .expect("railroad resource-bound diagram is detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("railroad resource-bound render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("railroad-bounded".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

fn railroad_style(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Railroad SVG XML");
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Railroad stylesheet text")
        .to_owned()
}

fn try_render_railroad_source(
    source: &str,
    engine: MermaidEngine,
    environment: &RenderEnvironment,
    diagram_id: &str,
) -> merman_render::Result<(RailroadDiagramLayout, family::RenderedFamilySvg)> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Railroad")
        .expect("detect Railroad")
        .into_presentation();
    let session = environment.begin_session().expect("begin Railroad session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
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

#[test]
fn railroad_svg_scopes_every_family_selector_to_the_root_id() {
    let (svg, _) = render_railroad(json!({}));
    let style = railroad_style(&svg);

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
fn railroad_blank_and_sanitized_labels_keep_zero_text_height_in_every_dialect() {
    for source in [
        r#"railroad-beta
empty = terminal("") ;
spaces = terminal("   ") ;
stripped = terminal("<script>discarded</script>") ;
"#,
        r#"railroad-ebnf-beta
empty = "" ;
spaces = "   " ;
stripped = "<script>discarded</script>" ;
"#,
        r#"railroad-abnf-beta
empty = "" ;
spaces = "   " ;
stripped = "<script>discarded</script>" ;
"#,
        r#"railroad-peg-beta
empty <- "" ;
spaces <- "   " ;
stripped <- "<script>discarded</script>" ;
"#,
    ] {
        for html_labels in [false, true] {
            let (layout, rendered) = try_render_railroad_source(
                source,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(json!({
                        "htmlLabels": html_labels, "railroad":{"fontFamily":"Cousine", "fontSize": 24}
                    })),
                ),
                &RenderEnvironment::deterministic(),
                "railroad-blank-text",
            )
            .expect("render blank Railroad labels with configured typography");

            assert_eq!(layout.rules.len(), 3);
            for rule in &layout.rules {
                assert_eq!(rule.elements.len(), 1);
                let terminal = &rule.elements[0];
                assert_eq!(terminal.kind, "terminal");
                assert!(terminal.label.trim().is_empty());
                assert_eq!(terminal.width, 20.0);
                assert_eq!(terminal.height, 20.0);
                assert_eq!(rule.baseline_y, 20.0);
                assert_eq!(rule.height, 50.0);
            }
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let boxes = document
                .descendants()
                .filter(|node| node.has_tag_name("rect"))
                .collect::<Vec<_>>();
            assert_eq!(boxes.len(), 3);
            for rect in boxes {
                assert_eq!(rect.attribute("width"), Some("20"));
                assert_eq!(rect.attribute("height"), Some("20"));
            }
            assert!(!rendered.svg().contains("discarded"));
            assert!(!rendered.svg().contains("<foreignObject"));
        }
    }
}

#[test]
fn railroad_dialect_combinations_preserve_source_branches_in_actual_reference_svg() {
    let cases = [
        (
            "railroadAbnf",
            include_str!("../../../fixtures/railroadAbnf/local_numeric_groups_bounds.mmd"),
            include_str!(
                "../../../fixtures/upstream-svgs/railroadAbnf/local_numeric_groups_bounds.svg"
            ),
            vec![
                "%x41.42",
                "%d67-90",
                "tail",
                "octets =",
                "%b01000001",
                "%X30-39",
                "tail =",
            ],
            vec![
                ("railroad-optional", 2),
                ("railroad-choice", 2),
                ("railroad-repetition", 1),
            ],
        ),
        (
            "railroadEbnf",
            include_str!("../../../fixtures/railroadEbnf/local_exception_group_postfix.mmd"),
            include_str!(
                "../../../fixtures/upstream-svgs/railroadEbnf/local_exception_group_postfix.svg"
            ),
            vec![
                "letter",
                "-",
                "x",
                "a",
                "b",
                "c",
                "d",
                "token =",
                "? alphabetic ?",
                "z",
                "letter =",
            ],
            vec![
                ("railroad-sequence", 2),
                ("railroad-choice", 3),
                ("railroad-optional", 1),
                ("railroad-repetition", 1),
            ],
        ),
        (
            "railroadPeg",
            include_str!("../../../fixtures/railroadPeg/local_predicate_group_suffix.mmd"),
            include_str!(
                "../../../fixtures/upstream-svgs/railroadPeg/local_predicate_group_suffix.svg"
            ),
            vec![
                "? &(...) ?",
                "? !(...) ?",
                "x",
                "y",
                "? . ?",
                "Token =",
                "!",
                "End =",
            ],
            vec![
                ("railroad-special", 3),
                ("railroad-choice", 1),
                ("railroad-repetition", 1),
            ],
        ),
    ];
    for (diagram_type, source, upstream, labels, groups) in cases {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        assert_eq!(parsed.metadata().diagram_type, diagram_type);
        let artifact = family::prepare(
            &parsed,
            &LayoutOptions::headless_svg_defaults(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap();
        let projection = artifact.layout_json().unwrap();
        let first = &projection["semantic"]["rules"][0]["definition"];
        match diagram_type {
            "railroadAbnf" => {
                assert_eq!(first["elements"][0]["min"], 2);
                assert_eq!(first["elements"][0]["max"], 2);
                assert_eq!(first["elements"][1]["element"]["type"], "optional");
            }
            "railroadEbnf" => {
                assert_eq!(first["elements"][0]["elements"][1]["value"], "-");
                assert_eq!(first["elements"][2]["min"], 1);
                assert!(first["elements"][2]["max"].is_null());
            }
            "railroadPeg" => {
                assert_eq!(first["elements"][0]["text"], "&(...)");
                assert_eq!(first["elements"][1]["text"], "!(...)");
                assert_eq!(first["elements"][2]["min"], 0);
            }
            _ => unreachable!(),
        }
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        for svg in [rendered.svg(), upstream] {
            let document = roxmltree::Document::parse(svg).unwrap();
            let texts: Vec<_> = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| {
                    node.descendants()
                        .filter(roxmltree::Node::is_text)
                        .filter_map(|node| node.text())
                        .collect::<String>()
                })
                .collect();
            assert_eq!(texts, labels, "{diagram_type}: complete source text order");
            for (class, count) in groups.iter().copied().chain([
                ("railroad-rule", 2),
                ("railroad-start", 2),
                ("railroad-end", 2),
            ]) {
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| node.attribute("class") == Some(class))
                        .count(),
                    count,
                    "{diagram_type}: {class}",
                );
            }
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("title"))
            );
            assert!(document.descendants().any(|node| node.has_tag_name("desc")));
        }
    }
}

#[allow(dead_code)]
mod common;

#[test]
fn railroad_svg_preserves_db_accessibility_text() {
    for (title, description, expected_title, expected_description) in [
        (
            "Accessible <!--a11y-->",
            "First\n<!--a11y-->",
            Some("Accessible "),
            Some("First\n"),
        ),
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            Some("\u{0085}"),
            Some("\u{0085}"),
        ),
        ("<!--a11y-->", "<!--a11y-->", None, None),
    ] {
        let source = format!(
            "railroad-beta\naccTitle: {title}\naccDescr {{\n{description}\n}}\nentry = terminal(\"x\") ;\n"
        );
        let (_, rendered) = try_render_railroad_source(
            &source,
            MermaidEngine::new(),
            &RenderEnvironment::deterministic(),
            "railroad-a11y",
        )
        .unwrap();
        let svg = rendered.svg();
        common::assert_svg_accessibility(svg, expected_title, expected_description);
    }
}
