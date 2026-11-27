use super::*;
use merman_render_core::resources::{RenderResourcePolicy, ResourceLimitId};
use quick_xml::events::Event;
use serde_json::json;

fn write_stylesheet(
    out: &mut BoundedSvgOutput<'_>,
    meter: &OperationWorkMeter,
    diagram_id: &str,
    effective_config: &serde_json::Value,
    font_family: &str,
    class_defs: &IndexMap<String, Vec<String>>,
) -> Result<()> {
    let theme = FlowchartPreparedTheme::resolve(effective_config, None, None);
    let palette = ContainerPalette::new(effective_config);
    let _ = write_flowchart_stylesheet(
        out,
        meter,
        FlowchartStylesheetRequest {
            diagram_id: SvgId::new(diagram_id).unwrap(),
            diagram_type: "flowchart-v2",
            drop_shadow_id: format!("{diagram_id}-merman-flowchart-document-filter-drop-shadow"),
            drop_shadow_small_id: format!(
                "{diagram_id}-merman-flowchart-document-filter-drop-shadow-small"
            ),
            gradient_id: format!("{diagram_id}-merman-flowchart-document-gradient-root"),
            theme: &theme,
            palette: &palette,
            font_family,
            font_size: 16.0,
            class_defs,
            swimlane: false,
            allow_deferred_palette: false,
        },
    )?;
    Ok(())
}

fn flowchart_css(
    diagram_id: &str,
    effective_config: &serde_json::Value,
    font_family: &str,
    class_defs: &IndexMap<String, Vec<String>>,
) -> Result<String> {
    let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    let mut out = BoundedSvgOutput::new(&meter);
    write_stylesheet(
        &mut out,
        &meter,
        diagram_id,
        effective_config,
        font_family,
        class_defs,
    )?;
    out.finish()
}

#[test]
fn bounded_flowchart_css_stops_before_class_catalog_after_prefix_rejection() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, 32)
        .expect("valid Flowchart CSS byte ceiling");
    let meter = OperationWorkMeter::new(policy);
    let mut out = BoundedSvgOutput::new(&meter);
    let class_defs = IndexMap::from([("late-class".to_string(), vec!["fill:#123456".to_string()])]);
    let config = json!({});

    let error = write_stylesheet(
        &mut out,
        &meter,
        "bounded-css",
        &config,
        "sans-serif",
        &class_defs,
    )
    .expect_err("the fixed Flowchart CSS prefix must exceed the tiny ceiling");

    assert!(matches!(error, Error::ResourceLimitExceeded(_)));
    assert!(
        !out.as_str().contains("late-class"),
        "classDef emission must not continue after the fixed-prefix checkpoint"
    );
}

#[test]
fn flowchart_class_selectors_are_css_encoded_before_terminal_xml_encoding() {
    let class = "class&<:quoted\"Ω";
    let class_defs = IndexMap::from([(class.to_owned(), vec!["fill:#123456".to_owned()])]);
    let config = json!({"htmlLabels": true});
    let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    let mut out = BoundedSvgOutput::new(&meter);
    out.push_str("<svg>");
    write_stylesheet(
        &mut out,
        &meter,
        "selector-scope",
        &config,
        "sans-serif",
        &class_defs,
    )
    .unwrap();
    out.push_str("</svg>");
    let svg = out.finish().unwrap();
    let mut reader = quick_xml::Reader::from_str(&svg);
    assert!(
        matches!(reader.read_event().unwrap(), Event::Start(root) if root.name().as_ref() == "svg")
    );
    let Event::Start(style) = reader.read_event().unwrap() else {
        panic!("expected the stylesheet start tag");
    };
    assert_eq!(style.name().as_ref(), "style");
    let escaped_css = reader.read_text(style.name()).unwrap();
    let css = quick_xml::escape::unescape(&escaped_css).unwrap();
    assert!(
        matches!(reader.read_event().unwrap(), Event::End(root) if root.name().as_ref() == "svg")
    );
    let escaped = escape_css_identifier(class);
    assert!(
        css.contains(&format!("#selector-scope .{escaped}>*{{")),
        "{css}"
    );
    assert!(!css.contains("&amp;"));
    assert!(!css.contains("&gt;"));
}

#[test]
fn khroma_named_edge_label_background_preserves_channels() {
    let css = flowchart_css(
        "theme_named_color",
        &json!({
            "themeVariables": {
                "edgeLabelBackground": "rebeccapurple"
            }
        }),
        "\"trebuchet ms\",verdana,arial,sans-serif",
        &IndexMap::new(),
    )
    .expect("valid khroma color");

    assert!(
        css.contains("#theme_named_color .labelBkg{background-color:rgba(102, 51, 153, 0.5);}")
    );
}

#[test]
fn unsupported_edge_label_background_returns_color_error() {
    let error = flowchart_css(
        "theme_unknown_color",
        &json!({
            "themeVariables": {
                "edgeLabelBackground": "not-a-css-color"
            }
        }),
        "\"trebuchet ms\",verdana,arial,sans-serif",
        &IndexMap::new(),
    )
    .expect_err("unsupported khroma color must fail");

    assert!(error.to_string().contains("not-a-css-color"));
}

#[test]
fn ordinary_neo_without_profile_keeps_mermaid_common_css_only() {
    let css = flowchart_css(
        "ordinary_neo",
        &json!({"look": "neo"}),
        "\"trebuchet ms\",verdana,arial,sans-serif",
        &IndexMap::new(),
    )
    .expect("valid Mermaid CSS");

    assert!(!css.contains(
        r#".flowchart-link[data-look="neo"]{stroke-linecap:round;stroke-linejoin:round;}"#
    ));
}

#[test]
fn deferred_flowchart_stylesheet_preserves_bytes_and_the_svg_ceiling() {
    use merman_render_core::svg_css::SvgDiagramIdProjection;

    let config = json!({
        "theme": "redux-color", "look": "neo",
        "themeVariables": {"borderColorArray": ["#446688"], "bkgColorArray": ["#ccddee"]}
    });
    let theme = FlowchartPreparedTheme::resolve(&config, None, None);
    let palette = ContainerPalette::new(&config);
    let class_defs = IndexMap::new();
    let render = |defer, ceiling| -> Result<String> {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let policy = match ceiling {
            Some(value) => policy
                .with_limit(ResourceLimitId::MaxSvgBytes, value)
                .unwrap(),
            None => policy,
        };
        let meter = OperationWorkMeter::new(policy);
        let projection = SvgDiagramIdProjection::new(&meter);
        let diagram_id = SvgId::new("palette-scope")
            .unwrap()
            .with_projection(&projection);
        let mut out = BoundedSvgOutput::new(&meter);
        out.push_str("<svg>");
        let deferred = write_flowchart_stylesheet(
            &mut out,
            &meter,
            FlowchartStylesheetRequest {
                diagram_id,
                diagram_type: "flowchart-v2",
                drop_shadow_id: "shadow",
                drop_shadow_small_id: "shadow-small",
                gradient_id: "gradient",
                theme: &theme,
                palette: &palette,
                font_family: "sans-serif",
                font_size: 16.0,
                class_defs: &class_defs,
                swimlane: false,
                allow_deferred_palette: defer,
            },
        )?;
        assert_eq!(deferred.is_some(), defer);
        out.push_str(r#"<g data-color-id="color-0"/></svg>"#);
        out.checkpoint()?;
        if let Some(deferred) = deferred {
            deferred.insert(&mut out)?;
        }
        projection.finish()?;
        out.finish()
    };
    let eager = render(false, None).unwrap();
    assert_eq!(render(true, Some(eager.len())).unwrap(), eager);
    assert!(matches!(
        render(true, Some(eager.len() - 1)),
        Err(Error::ResourceLimitExceeded(_))
    ));
}
