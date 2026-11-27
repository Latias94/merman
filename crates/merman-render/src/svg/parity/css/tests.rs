use super::*;
use crate::svg::PreparedCommonCss;
use std::collections::BTreeMap;

fn er_binding(config: &serde_json::Value) -> crate::er::ErCssBinding {
    crate::er::ErCssBinding::resolve(config)
}

fn info_css_with_config<I>(diagram_id: I, effective_config: &serde_json::Value) -> String
where
    I: SvgDiagramIdValue,
{
    let mut out = String::new();
    PreparedCommonCss::new(effective_config, None)
        .write(&mut out, diagram_id)
        .expect("String-backed Mermaid CSS emission cannot fail");
    out
}

fn assert_fragments_in_order(css: &str, fragments: &[&str]) {
    let mut cursor = 0;
    for fragment in fragments {
        let offset = css[cursor..]
            .find(fragment)
            .unwrap_or_else(|| panic!("missing CSS fragment: {fragment}"));
        cursor += offset + fragment.len();
    }
}

fn css_properties_for_selector(css: &str, selector: &str) -> BTreeMap<String, String> {
    css.split('}')
        .filter_map(|rule| rule.rsplit_once('{'))
        .find_map(|(selectors, declarations)| {
            selectors
                .split(',')
                .map(str::trim)
                .any(|candidate| candidate == selector)
                .then(|| {
                    declarations
                        .split(';')
                        .filter_map(|declaration| declaration.split_once(':'))
                        .map(|(property, value)| {
                            (property.trim().to_string(), value.trim().to_string())
                        })
                        .collect()
                })
        })
        .unwrap_or_else(|| panic!("missing CSS selector: {selector}"))
}

fn assert_css_properties(css: &str, selector: &str, expected: &[(&str, &str)]) {
    let actual = css_properties_for_selector(css, selector);
    for (property, value) in expected {
        assert_eq!(
            actual.get(*property).map(String::as_str),
            Some(*value),
            "{selector} {property}"
        );
    }
}

#[test]
fn mermaid_base_css_fragments_keep_parity_order() {
    let cfg = serde_json::json!({});
    let base_fragments = [
        r#"#diag{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:16px;fill:#333;}"#,
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
        r#"#diag .edge-animation-slow{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}#diag .edge-animation-fast{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}"#,
        r#"#diag .error-icon{fill:#552222;}#diag .error-text{fill:#552222;stroke:#552222;}"#,
        r#"#diag .edge-thickness-normal{stroke-width:1px;}#diag .edge-thickness-thick{stroke-width:3.5px;}#diag .edge-pattern-solid{stroke-dasharray:0;}#diag .edge-thickness-invisible{stroke-width:0;fill:none;}#diag .edge-pattern-dashed{stroke-dasharray:3;}#diag .edge-pattern-dotted{stroke-dasharray:2;}"#,
        r#"#diag .marker{fill:#333333;stroke:#333333;}#diag .marker.cross{stroke:#333333;}"#,
        r#"#diag svg{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:16px;}#diag p{margin:0;}"#,
    ];

    let info = info_css_with_config("diag", &cfg);
    assert_fragments_in_order(&info, &base_fragments);
    assert!(info.ends_with(
        r#"#diag :root{--mermaid-font-family:"trebuchet ms",verdana,arial,sans-serif;}"#
    ));

    let er =
        er_css_with_resolved_typography("diag", &er_binding(&cfg)).expect("valid ER theme colors");
    assert_fragments_in_order(
        &er,
        &[
            &base_fragments[..],
            &[r#"#diag .entityBox{fill:#ECECFF;stroke:#9370DB;}"#],
        ]
        .concat(),
    );
    assert!(er.ends_with(
        r#"#diag :root{--mermaid-font-family:"trebuchet ms",verdana,arial,sans-serif;}"#
    ));
}

#[test]
fn common_css_preserves_nonnegative_configured_font_sizes() {
    for (size, expected) in [("0px", "0px"), ("0.5px", "0.5px"), ("24pt", "32px")] {
        let css = info_css_with_config(
            "diag",
            &serde_json::json!({"themeVariables": {"fontSize": size}}),
        );
        for selector in ["#diag", "#diag svg"] {
            assert_css_properties(&css, selector, &[("font-size", expected)]);
        }
    }
}

#[test]
fn shared_css_keeps_selectors_and_url_fragments_in_their_own_escaping_domains() {
    let css = info_css_with_config(
        "seq:prod.v1",
        &serde_json::json!({
            "look": "neo",
            "themeVariables": {
                "useGradient": true,
                "dropShadow": "url(#drop-shadow)"
            }
        }),
    );

    assert!(css.contains(r#"#seq\:prod\.v1 [data-look="neo"].node rect"#));
    assert!(css.contains("url(#seq:prod.v1-gradient)"));
    assert!(css.contains("url(#seq:prod.v1-drop-shadow)"));
    assert!(!css.contains(r"url(#seq\:prod\.v1-"));
}

#[test]
fn logical_css_fragment_ids_survive_one_terminal_xml_encoding() {
    use ::cssparser::ToCss;

    let diagram_id = "scope&<quoted \"Ω'()\\";
    let fragment = format!("url(#{}-gradient)", FragmentDiagramId(diagram_id));
    let mut parser = ::cssparser::Parser::new(&fragment);
    let ::cssparser::Token::UnquotedUrl(value) = parser.next().unwrap() else {
        panic!("expected one escaped CSS URL token: {fragment}");
    };
    assert_eq!(value.as_ref(), format!("#{diagram_id}-gradient"));
    let canonical = ::cssparser::Token::UnquotedUrl(value.clone()).to_css_string();
    assert_eq!(canonical, fragment);

    let css = info_css_with_config(
        diagram_id,
        &serde_json::json!({"look": "neo", "themeVariables": {"useGradient": true}}),
    );
    assert!(css.contains(crate::svg::escape_css_identifier(diagram_id).as_ref()));
    let mut serialized = String::from("<svg><style>");
    super::super::output::XmlTextOutput::new(&mut serialized).push_str(&css);
    serialized.push_str("</style></svg>");
    let document = crate::xml_dom::Document::parse(&serialized).unwrap();
    let decoded = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert_eq!(decoded, css);
    assert!(!decoded.contains("&amp;"));
}

#[test]
fn neo_css_accepts_renderer_owned_resource_ids() {
    let mut css = String::new();
    PreparedCommonNeoCss::new(&serde_json::json!({ "themeVariables": { "useGradient": true } }))
        .write_with_ids(
            &mut css,
            "diag",
            "document-gradient-7",
            "url(#document-shadow-9)",
        )
        .unwrap();
    assert!(css.contains("stroke:url(#document-gradient-7)"));
    assert!(css.contains("filter:url(#document-shadow-9)"));
    assert!(!css.contains("url(#diag-gradient)"));
    assert!(!css.contains("url(#diag-drop-shadow)"));
}

#[test]
fn resolved_typography_owns_the_root_variable_and_nested_svg_font() {
    let config = serde_json::json!({
        "fontFamily": "Courier, monospace",
        "themeVariables": { "fontFamily": "Arial, sans-serif" }
    });
    let mut css = String::new();
    let values = PreparedCommonCss::new(&config, Some("serif"));
    values.write(&mut css, "diag").unwrap();
    assert!(css.ends_with("#diag :root{--mermaid-font-family:serif;}"));
    assert!(css.contains("#diag svg{font-family:serif;"));
}

#[test]
fn explicit_empty_root_font_omits_the_rule_without_changing_theme_font() {
    for root_font in ["", "   "] {
        let config = serde_json::json!({
            "fontFamily": root_font,
            "themeVariables": { "fontFamily": "Arial, sans-serif" }
        });
        for css in [
            info_css_with_config("diag", &config),
            er_css_with_resolved_typography("diag", &er_binding(&config)).unwrap(),
        ] {
            assert!(css.starts_with("#diag{font-family:Arial,sans-serif;"));
            assert!(!css.contains("--mermaid-font-family"));
        }
    }
}

#[test]
fn shared_and_er_css_keep_theme_font_separate_from_root_custom_property() {
    let config = serde_json::json!({
        "fontFamily": "Courier, monospace",
        "themeVariables": { "fontFamily": "Arial, sans-serif" }
    });
    for css in [
        info_css_with_config("diag", &config),
        er_css_with_resolved_typography("diag", &er_binding(&config)).unwrap(),
    ] {
        assert!(css.starts_with("#diag{font-family:Arial,sans-serif;"));
        assert!(css.ends_with("#diag :root{--mermaid-font-family:Courier,monospace;}"));
    }
}

#[test]
fn mermaid_base_css_exposes_the_complete_common_neo_contract() {
    let css = info_css_with_config(
        "diag",
        &serde_json::json!({
            "themeVariables": {
                "nodeBorder": "#123456",
                "strokeWidth": 3,
                "useGradient": true,
                "dropShadow": "url(#drop-shadow)"
            }
        }),
    );
    let gradient = "url(#diag-gradient)";
    let shadow = "url(#diag-drop-shadow)";

    assert_css_properties(
        &css,
        "#diag .edge-thickness-normal",
        &[("stroke-width", "3px")],
    );
    assert_css_properties(&css, "#diag .node .neo-node", &[("stroke", "#123456")]);
    for selector in [
        "#diag [data-look=\"neo\"].node rect",
        "#diag [data-look=\"neo\"].cluster rect",
        "#diag [data-look=\"neo\"].node polygon",
    ] {
        assert_css_properties(&css, selector, &[("stroke", gradient), ("filter", shadow)]);
    }
    assert_css_properties(
        &css,
        "#diag [data-look=\"neo\"].swimlane.cluster rect",
        &[("filter", "none")],
    );
    assert_css_properties(
        &css,
        "#diag [data-look=\"neo\"].node path",
        &[("stroke", gradient), ("stroke-width", "3px")],
    );
    assert_css_properties(
        &css,
        "#diag [data-look=\"neo\"].node .outer-path",
        &[("filter", shadow)],
    );
    assert_css_properties(
        &css,
        "#diag [data-look=\"neo\"].node .neo-line path",
        &[("stroke", "#123456"), ("filter", "none")],
    );
    assert_css_properties(
        &css,
        "#diag [data-look=\"neo\"].node circle",
        &[("stroke", gradient), ("filter", shadow)],
    );
    assert_css_properties(
        &css,
        "#diag [data-look=\"neo\"].node circle .state-start",
        &[("fill", "#000000")],
    );
    assert_css_properties(
        &css,
        "#diag [data-look=\"neo\"].icon-shape .icon",
        &[("fill", gradient), ("filter", shadow)],
    );
    assert_css_properties(
        &css,
        "#diag [data-look=\"neo\"].icon-shape .icon-neo path",
        &[("stroke", gradient), ("filter", shadow)],
    );
}

#[test]
fn er_css_honors_mermaid_11_15_theme_options() {
    let cfg = serde_json::json!({
        "look": "neo",
        "themeVariables": {
            "fontFamily": "\"ibm plex sans\", arial, sans-serif",
            "fontSize": "18px",
            "textColor": "#101010",
            "lineColor": "#202020",
            "errorBkgColor": "#303030",
            "errorTextColor": "#404040",
            "mainBkg": "#505050",
            "nodeBorder": "#606060",
            "nodeTextColor": "#707070",
            "tertiaryColor": "hsl(80, 100%, 96%)",
            "edgeLabelBackground": "#b0c0d0",
            "strokeWidth": 3
        }
    });

    let css =
        er_css_with_resolved_typography("er", &er_binding(&cfg)).expect("valid ER theme colors");

    assert!(css.contains(
        r#"#er{font-family:"ibm plex sans",arial,sans-serif;font-size:18px;fill:#101010;}"#
    ));
    assert!(css.contains(
        r#"#er .error-icon{fill:#303030;}#er .error-text{fill:#404040;stroke:#404040;}"#
    ));
    assert!(css.contains(r#"#er .marker{fill:#202020;stroke:#202020;}"#));
    assert!(css.contains(r#"#er .entityBox{fill:#505050;stroke:#606060;}"#));
    assert!(css.contains(
        r#"#er .relationshipLabelBox{fill:hsl(80, 100%, 96%);opacity:0.7;background-color:hsl(80, 100%, 96%);}"#
    ));
    assert!(css.contains(r#"#er .labelBkg{background-color:rgba(248.2, 255, 234.6, 0.5);}"#));
    assert!(css.contains(r#"#er .edgeLabel{background-color:#b0c0d0;}#er .edgeLabel .label rect{fill:#b0c0d0;}#er .edgeLabel .label text{fill:#101010;}#er .edgeLabel .label{fill:#606060;font-size:14px;}"#));
    assert!(
        css.contains(r#"#er .label{font-family:"ibm plex sans",arial,sans-serif;color:#707070;}"#)
    );
    assert!(css.contains(r#"#er .node rect,#er .node circle,#er .node ellipse,#er .node polygon{fill:#505050;stroke:#606060;stroke-width:3;}"#));
    assert!(css.contains(r#"#er .relationshipLine{stroke:#202020;stroke-width:3;fill:none;}"#));
    assert!(
        css.contains(
            r#"#er .marker{fill:none!important;stroke:#202020!important;stroke-width:1;}"#
        )
    );
}

#[test]
fn er_css_rejects_unsupported_tertiary_color() {
    let error = er_css_with_resolved_typography(
        "er",
        &er_binding(&serde_json::json!({
            "themeVariables": {
                "tertiaryColor": "not-a-css-color"
            }
        })),
    )
    .expect_err("unsupported khroma color must fail stylesheet generation");

    assert!(error.to_string().contains("not-a-css-color"));
}

#[cfg(feature = "diagram-xychart")]
#[test]
fn xychart_common_css_keeps_source_colors_with_its_resolved_font_stack() {
    let config = crate::test_support::presentation_config(serde_json::json!({
        "fontFamily": "Inter, sans-serif",
        "themeVariables": { "textColor": "#123456", "lineColor": "#abcdef" }
    }));
    let typography = crate::xychart::XyChartTypographyThemePlan::resolve(&config);
    let mut css = String::new();
    typography.common_css().write(&mut css, "chart").unwrap();
    assert_css_properties(
        &css,
        "#chart",
        &[("font-family", "Inter,sans-serif"), ("fill", "#123456")],
    );
    assert_css_properties(
        &css,
        "#chart .marker",
        &[("fill", "#abcdef"), ("stroke", "#abcdef")],
    );
}
