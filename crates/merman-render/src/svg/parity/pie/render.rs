use super::super::*;
use super::geometry::{DEFAULT_PIE_DIAGRAM_ID, open_pie_svg, pie_viewport};
use merman_model::families::pie::PieDiagramModel;
use merman_render_charts::pie::{PieConfigView, write_mermaid_svg_body};

#[cfg(test)]
pub(crate) fn render_pie_diagram_svg_model(
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    effective_config: &impl crate::config::ConfigLookup,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let paint_plan = crate::pie::PieThemePlan::baseline(model, effective_config);
    render_pie_diagram_svg_model_with_paint_plan(
        layout,
        model,
        &paint_plan,
        effective_config,
        options,
    )
}

pub(crate) fn render_pie_diagram_svg_model_with_paint_plan(
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    paint_plan: &crate::pie::PieThemePlan,
    effective_config: &impl crate::config::ConfigLookup,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options
        .validated_diagram_id
        .unwrap_or(DEFAULT_PIE_DIAGRAM_ID);

    let render_settings = PieConfigView::new(effective_config).render_settings();
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document = open_pie_svg(
        &mut out,
        model,
        options.project_svg_id(diagram_id),
        pie_viewport(layout, model),
        render_settings.use_max_width,
        options,
    )?;

    out.push_str("<style>");
    out.checkpoint()?;
    // This stylesheet streams each ID occurrence directly through the bounded sink.
    write_pie_css(
        &mut super::super::output::XmlTextOutput::new(&mut out),
        diagram_id,
        paint_plan,
    )?;
    out.push_str(r#"</style><g/>"#);
    out.checkpoint()?;

    write_mermaid_svg_body(
        &mut out,
        layout,
        model,
        paint_plan.mermaid_style(),
        render_settings,
        || options.checkpoint_emit(),
    )?;
    out.push_str("</g></svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::super::geometry::{
        EMPTY_PIE_HEIGHT, EMPTY_PIE_WIDTH, empty_pie_root_viewport_fallback,
    };
    use super::*;
    use crate::DiagramFamilyId;

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
        let plan = crate::pie::PieThemePlan::baseline(&model, &effective_config);
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let mut layout = crate::pie::layout_pie_diagram_typed_with_paint_plan(
            &model,
            None,
            &effective_config,
            &plan,
            &measurer,
        )
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
            bounds: Some(crate::model::Bounds {
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
        let options = SvgRenderOptions {
            diagram_id: Some("pieFixed".to_string()),
            ..SvgRenderOptions::default()
        };
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("render session");
        let debug = SvgDebugOptions::default();
        let execution =
            SvgExecution::unthemed_for_test(&options, &debug, &session, DiagramFamilyId::PIE)
                .expect("SVG execution");

        let svg = render_pie_diagram_svg_model(
            &layout,
            &PieDiagramModel::default(),
            &serde_json::json!({"pie": {"useMaxWidth": false}}),
            &execution,
        )
        .unwrap();
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
}
