use merman_model::family::DiagramFamilyId;
use merman_render_core::{
    Bounds, resources::RenderResourcePolicy, root_svg, svg_css::SvgDiagramIdProjection,
    text::DeterministicTextMeasurer,
};

use super::root::{EMPTY_PIE_HEIGHT, EMPTY_PIE_WIDTH, empty_pie_root_viewport_fallback};
use super::*;
use crate::pie::preparation::layout_pie_diagram_typed_with_paint_plan;

#[test]
fn pie_slice_fill_preserves_a_quoted_paint_reference() {
    let fill = r##"url("#swatch-#333;") red"##;
    let config = serde_json::json!({"themeVariables": {"pie1": fill}});
    let model = PieDiagramModel {
        sections: vec![merman_model::families::pie::PieSection {
            label: "Section".to_owned(),
            value: 1.0,
        }],
        ..Default::default()
    };
    let plan = PieThemePlan::baseline(&model, &config);
    let layout = layout_pie_diagram_typed_with_paint_plan(
        &model,
        None,
        &config,
        &plan,
        &DeterministicTextMeasurer::default(),
    )
    .unwrap();
    let mut svg = String::from("<svg>");
    write_mermaid_svg_body(
        &mut svg,
        &layout,
        &model,
        plan.mermaid_style(),
        PieConfigView::new(&config).render_settings(),
        || Ok(()),
    )
    .unwrap();
    svg.push_str("</g></svg>");

    let doc = roxmltree::Document::parse(&svg).unwrap();
    let slice = doc.descendants().find(|node| node.has_tag_name("path"));
    assert_eq!(slice.and_then(|node| node.attribute("fill")), Some(fill));
}

#[test]
fn pie_legend_rect_style_serializes_default_palette_colors_as_rgb() {
    let effective_config = serde_json::json!({});
    let model = PieDiagramModel {
        sections: vec![merman_model::families::pie::PieSection {
            label: "Section".to_owned(),
            value: 1.0,
        }],
        ..Default::default()
    };
    let plan = PieThemePlan::baseline(&model, &effective_config);
    let measurer = DeterministicTextMeasurer::default();
    let mut layout =
        layout_pie_diagram_typed_with_paint_plan(&model, None, &effective_config, &plan, &measurer)
            .unwrap();
    let mut legend_swatch_style = |fill: &str| {
        layout.legend_items[0].fill = fill.to_owned();
        let mut svg = String::from("<svg>");
        write_mermaid_svg_body(
            &mut svg,
            &layout,
            &model,
            plan.mermaid_style(),
            PieConfigView::new(&effective_config).render_settings(),
            || Ok(()),
        )
        .unwrap();
        svg.push_str("</g></svg>");
        let doc = roxmltree::Document::parse(&svg).unwrap();
        doc.descendants()
            .find(|node| node.has_tag_name("rect"))
            .and_then(|rect| rect.attribute("style"))
            .expect("legend swatch style")
            .to_owned()
    };
    assert_eq!(
        legend_swatch_style("hsl(60, 100%, 63.5294117647%)"),
        "fill: rgb(255, 255, 69); stroke: rgb(255, 255, 69);"
    );
    assert_eq!(
        legend_swatch_style("#ECECFF"),
        "fill: rgb(236, 236, 255); stroke: rgb(236, 236, 255);"
    );
    assert_eq!(
        legend_swatch_style("hsla(210 65.3846153846% 20.3921568627% / .5)"),
        "fill: rgba(18, 52, 86, 0.5); stroke: rgba(18, 52, 86, 0.5);"
    );
    assert_eq!(
        legend_swatch_style("ReBeccAPurple"),
        "fill: rebeccapurple; stroke: rebeccapurple;"
    );
}

#[test]
fn empty_pie_root_viewport_fallback_only_repairs_non_finite_roots() {
    let model = PieDiagramModel::default();
    let (bounds, max_width) =
        empty_pie_root_viewport_fallback(&model, 0.0, 0.0, f64::INFINITY, 450.0)
            .expect("non-finite empty pie should use fallback");

    assert_eq!(
        bounds,
        root_svg::DiagramBounds::from_view_box(0.0, 0.0, EMPTY_PIE_WIDTH, EMPTY_PIE_HEIGHT,)
    );
    assert_eq!(
        max_width,
        root_svg::RootMaxWidth::SvgNumber(EMPTY_PIE_WIDTH)
    );
}

#[test]
fn empty_pie_root_viewport_fallback_preserves_finite_title_bounds() {
    let model = PieDiagramModel::default();
    assert_eq!(
        empty_pie_root_viewport_fallback(&model, 0.0, 0.0, 292.400390625, 450.0),
        None
    );
}

#[test]
fn pie_root_honors_disabled_max_width() {
    let layout = PieDiagramLayout {
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 490.0,
            max_y: 450.0,
        }),
        title: None,
        center_x: 225.0,
        center_y: 225.0,
        radius: 185.0,
        outer_radius: 186.0,
        legend_x: 216.0,
        legend_start_y: 0.0,
        legend_step_y: 22.0,
        slices: Vec::new(),
        legend_items: Vec::new(),
    };
    let meter = OperationWorkMeter::new(RenderResourcePolicy::interactive());
    let projection = SvgDiagramIdProjection::new(&meter);
    let stylesheet_id = SvgId::new("pieFixed").unwrap();
    let diagram_id = stylesheet_id.with_projection(&projection);
    let model = PieDiagramModel::default();
    let config = serde_json::json!({"pie": {"useMaxWidth": false}});
    let appearance = PieAppearance::Mermaid(PieThemePlan::baseline(&model, &config));
    let svg = render_pie_svg(
        &layout,
        &model,
        &appearance,
        &config,
        &PieSvgContext {
            stylesheet_id,
            diagram_id,
            viewport: RootViewportContext::new(DiagramFamilyId::PIE, diagram_id),
            work_meter: &meter,
            canvas_background: None,
            appearance_and_backdrop: None,
        },
    )
    .unwrap();
    projection.finish().unwrap();
    let root_open = svg.split_once('>').expect("root svg open tag").0;

    assert!(root_open.contains(r#"width="490""#), "{root_open}");
    assert!(root_open.contains(r#"height="450""#), "{root_open}");
    assert!(
        root_open.contains(r#"viewBox="0 0 490 450""#),
        "{root_open}"
    );
    assert!(!root_open.contains("style="), "{root_open}");
    assert!(!root_open.contains("max-width"), "{root_open}");
}

#[test]
fn pie_css_honors_mermaid_11_16_theme_options() {
    let cfg = serde_json::json!({
        "themeVariables": {
            "fontFamily": "\"ibm plex sans\", arial, sans-serif",
            "textColor": "#111111",
            "taskTextDarkColor": "#222222",
            "pieStrokeColor": "#333333",
            "pieStrokeWidth": "4px",
            "pieOpacity": "0.9",
            "pieOuterStrokeColor": "#444444",
            "pieOuterStrokeWidth": "5px",
            "pieTitleTextSize": "26px",
            "pieTitleTextColor": "#555555",
            "pieSectionTextSize": "18px",
            "pieSectionTextColor": "#666666",
            "pieLegendTextSize": "19px",
            "pieLegendTextColor": "#777777"
        }
    });

    let mut css = String::new();
    let paint_plan = PieThemePlan::baseline(&PieDiagramModel::default(), &cfg);
    write_pie_css(
        &mut XmlTextOutput::new(&mut css),
        SvgId::new("pie").unwrap(),
        &paint_plan,
    )
    .unwrap();

    assert!(css.contains(r#"#pie .pieCircle{stroke:#333333;stroke-width:4px;opacity:0.9;}"#));
    assert!(css.contains(r#"#pie .pieCircle.highlighted{scale:1.05;opacity:1;}"#));
    assert!(css.contains(
        r#"#pie .pieCircle.highlightedOnHover:hover{transition-duration:250ms;scale:1.05;opacity:1;}"#
    ));
    assert!(css.contains(r#"#pie .pieOuterCircle{stroke:#444444;stroke-width:5px;fill:none;}"#));
    assert!(css.contains(r#"#pie .pieTitleText{text-anchor:middle;font-size:26px;fill:#555555;font-family:"ibm plex sans",arial,sans-serif;}"#));
    assert!(css.contains(
        r#"#pie .slice{font-family:"ibm plex sans",arial,sans-serif;fill:#666666;font-size:18px;}"#
    ));
    assert!(css.contains(r#"#pie .legend text{fill:#777777;font-family:"ibm plex sans",arial,sans-serif;font-size:19px;}"#));
}
