use super::super::*;
use super::geometry::{DEFAULT_PIE_DIAGRAM_ID, open_pie_svg, pie_viewport};
use merman_model::families::pie::PieDiagramModel;
use merman_render_charts::pie::{
    PieConfigView, PieNativeStyle, PieNativeSvgContext, write_native_svg_body,
};

pub(crate) fn render_native_pie_svg(
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    style: &PieNativeStyle,
    effective_config: &impl crate::config::ConfigLookup,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.project_svg_id(
        options
            .validated_diagram_id
            .unwrap_or(DEFAULT_PIE_DIAGRAM_ID),
    );
    let settings = PieConfigView::new(effective_config).render_settings();
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root = open_pie_svg(
        &mut out,
        model,
        diagram_id,
        pie_viewport(layout, model),
        settings.use_max_width,
        options,
    )?;
    let session = options.family.session();
    write_native_svg_body(
        &mut out,
        layout,
        model,
        style,
        settings,
        diagram_id,
        &PieNativeSvgContext {
            title_vertical_extents: crate::text::svg_title_bbox_vertical_extents_px,
            canvas_background: session.canvas_background(),
            appearance_and_backdrop: session
                .native_theme()
                .map(|theme| (theme.resolved().appearance(), theme.export_backdrop())),
        },
    )?;
    out.push_str("</g></svg>\n");
    root.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::theme::{BuiltinPreset, Color, NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_render_charts::pie::native_pie_slice_path;
    use svgtypes::{PathParser, PathSegment};

    #[test]
    fn native_pie_body_reports_a_rejected_last_legend_close() {
        let model = PieDiagramModel {
            sections: vec![merman_model::families::pie::PieSection {
                label: "A".into(),
                value: 1.0,
            }],
            ..Default::default()
        };
        let config = serde_json::json!({});
        let theme = ThemeCompiler::new()
            .compile_preset(BuiltinPreset::Paper, None)
            .unwrap();
        let (layout, style) = crate::pie::prepare_native_pie(
            theme.family("pie"),
            &model,
            None,
            &config,
            &crate::text::DeterministicTextMeasurer::default(),
        )
        .unwrap();
        let settings = PieConfigView::new(&config).render_settings();
        let context = PieNativeSvgContext {
            title_vertical_extents: crate::text::svg_title_bbox_vertical_extents_px,
            canvas_background: None,
            appearance_and_backdrop: None,
        };
        let mut expected = String::new();
        write_native_svg_body(
            &mut expected,
            &layout,
            &model,
            &style,
            settings,
            DEFAULT_PIE_DIAGRAM_ID,
            &context,
        )
        .unwrap();
        let prefix = expected.strip_suffix("</g>").unwrap();
        assert!(prefix.ends_with("</text>"));
        let meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, prefix.len())
                .unwrap(),
        );
        let mut bounded = BoundedSvgOutput::new(&meter);
        let error = write_native_svg_body(
            &mut bounded,
            &layout,
            &model,
            &style,
            settings,
            DEFAULT_PIE_DIAGRAM_ID,
            &context,
        )
        .unwrap_err();

        assert!(
            matches!(error, Error::ResourceLimitExceeded(details) if details.limit == "max_svg_bytes" && details.actual == expected.len())
        );
        assert_eq!(bounded.as_str(), prefix);
    }

    #[test]
    fn native_pie_retains_a_slice_narrower_than_the_gap() {
        let slice = crate::model::PieSliceLayout {
            label: String::new(),
            value: 1.0,
            start_angle: 0.0,
            end_angle: 0.02,
            is_full_circle: false,
            percent: 0,
            text_x: 0.0,
            text_y: 0.0,
            fill: String::new(),
        };
        let path = native_pie_slice_path(&slice, 185.0, 0.0, 6.0);
        let mut segments = PathParser::from(path.as_str()).map(|segment| segment.unwrap());
        assert!(matches!(segments.next(), Some(PathSegment::MoveTo { .. })));
        assert!(matches!(
            segments.next(),
            Some(PathSegment::EllipticalArc { .. })
        ));
    }

    #[test]
    fn native_pie_uses_categorical_paint_after_filtering_and_wraparound() {
        let theme = ThemeCompiler::new()
            .compile_json(
                br##"{
                    "schema_version": 2,
                    "appearances": {"light": {
                        "seed": {"canvas": "#FFFFFF", "ink": "#151515", "accent": "#224466"},
                        "canvas": {"padding_px": 23},
                        "data": {
                            "fill": ["#EEEEEE", "#DDDDDD", "#222222", "#BBBBBB", "#CCCCCC", "#444444"],
                            "ink": ["#222222", "#333333", "#111111", "#555555"],
                            "pattern_encoding": true
                        }
                    }},
                    "roles": {"canvas": {"kind": "linear_gradient", "angle_deg": 30, "stops": [
                        {"offset": 0, "color": "#FFFFFF"}, {"offset": 1, "color": "#EEEEEE"}
                    ]}},
                    "overrides": [{
                        "families": ["pie"],
                        "roles": {"text.title": "#16304D", "text.primary": "#161A1F"},
                        "typography": {
                            "serif": ["A&B Serif", "serif"],
                            "sans": ["Pie Body", "sans-serif"],
                            "title_family": "serif", "base_size_px": 20,
                            "title_weight": 700, "label_weight": 600
                        },
                        "form": {"chart": {"pie_gap_px": 2.25, "pie_opacity": 0.8}}
                    }]
                }"##,
                None,
            )
            .unwrap();
        let source = r#"---
config:
  pie:
    donutHole: 0.4
    legendPosition: top
---
pie showData title Capacity
  "A" : 40
  "Hidden" : 0.1
  "C" : 25
  "D" : 10
  "E" : 10
  "F" : 5
  "G" : 5
  "H" : 5
"#;
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let environment = RenderEnvironment::deterministic();
        let native = NativeTheme::new(theme.clone());
        let options = SvgRenderOptions {
            diagram_id: Some("capacity".to_owned()),
            ..Default::default()
        };
        let render = |session| {
            family::prepare(parsed.presentation(), &LayoutOptions::default(), session)
                .unwrap()
                .render_svg(&options, &SvgDebugOptions::default())
                .unwrap()
                .svg()
                .to_owned()
        };
        let svg = render(
            environment
                .begin_session_with_native_theme(native.clone())
                .unwrap()
                .with_canvas_background(Color::rgb(250, 250, 250)),
        );
        let parity = render(environment.begin_session().unwrap());
        let document = roxmltree::Document::parse(&svg).unwrap();
        let parity = roxmltree::Document::parse(&parity).unwrap();
        let slices = |node: roxmltree::Node| {
            node.has_tag_name("path") && node.attribute("class") == Some("pieCircle")
        };
        let paths: Vec<_> = document
            .descendants()
            .filter(|node| slices(*node))
            .collect();
        let parity_paths: Vec<_> = parity
            .descendants()
            .filter(|node| slices(*node))
            .map(|node| node.attribute("d").unwrap())
            .collect();
        // The first boundary is at twelve o'clock. The two adjacent outer arcs each
        // leave half the 2.25 px gap, independent of the canvas paint.
        let first_move_x = |path: &str| match PathParser::from(path).next().unwrap().unwrap() {
            PathSegment::MoveTo { x, .. } => x,
            segment => panic!("expected an initial move, got {segment:?}"),
        };
        assert_eq!(first_move_x(parity_paths[0]), 0.0);
        assert_eq!(first_move_x(paths[0].attribute("d").unwrap()), 1.125);
        assert_eq!(paths[0].attribute("stroke"), None);

        let swatches: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("rect")
                    && node
                        .parent_element()
                        .is_some_and(|parent| parent.attribute("class") == Some("legend"))
            })
            .collect();
        let percentages: Vec<_> = document
            .descendants()
            .filter(|node| node.attribute("class") == Some("slice"))
            .collect();
        assert_eq!(swatches.len(), 8);
        assert_eq!(paths.len(), 7);
        assert_eq!(percentages.len(), paths.len());
        let palette = &theme.family("pie").data;
        for ((ordinal, path), label) in [0, 2, 3, 4, 5, 6, 7]
            .into_iter()
            .zip(paths)
            .zip(percentages)
        {
            assert_eq!(path.attribute("fill-opacity"), Some("0.8"));
            assert_eq!(path.attribute("fill"), swatches[ordinal].attribute("fill"));
            assert_eq!(
                label.attribute("fill").unwrap(),
                palette.fill_ink(ordinal).to_string()
            );
            assert_eq!(
                label.attribute("font-family"),
                Some("\"Pie Body\", sans-serif")
            );
            assert_eq!(label.attribute("font-size"), Some("20"));
            assert_eq!(label.attribute("font-weight"), Some("600"));
            if ordinal.is_multiple_of(6) {
                assert_eq!(label.attribute("stroke"), None);
            } else {
                assert_eq!(label.attribute("paint-order"), Some("stroke"));
                let outline = Color::parse(label.attribute("stroke").unwrap()).unwrap();
                assert!(palette.fill_ink(ordinal).contrast(outline) >= 4.5);
            }
        }
        assert_eq!(swatches[0].attribute("fill"), Some("#EEEEEE"));
        assert_eq!(swatches[6].attribute("fill"), Some("#EEEEEE"));
        assert_eq!(
            swatches[2].attribute("fill"),
            Some("url(#capacity-pie-data-2)")
        );
        let pattern = document
            .descendants()
            .find(|node| node.attribute("id") == Some("capacity-pie-data-2"))
            .unwrap();
        assert_eq!(
            pattern.first_element_child().unwrap().attribute("fill"),
            Some("#222222")
        );
        assert_eq!(
            pattern.last_element_child().unwrap().attribute("stroke"),
            Some("#111111")
        );
        let legend = swatches[1].next_sibling_element().unwrap();
        assert_eq!(legend.text(), Some("Hidden [0.1]"));
        assert_eq!(legend.attribute("fill"), Some("#161A1F"));
        let title = document
            .descendants()
            .find(|node| node.attribute("class") == Some("pieTitleText"))
            .unwrap();
        assert_eq!(title.attribute("fill"), Some("#16304D"));
        assert_eq!(title.attribute("font-family"), Some("\"A&B Serif\", serif"));
        assert_eq!(title.attribute("font-size"), Some("30"));
        assert_eq!(title.attribute("font-weight"), Some("700"));

        let gradient = render(environment.begin_session_with_native_theme(native).unwrap());
        let gradient = roxmltree::Document::parse(&gradient).unwrap();
        assert!(gradient.descendants().any(|node| {
            node.has_tag_name("linearGradient")
                && node.attribute("id") == Some("capacity-merman-canvas-base")
        }));
        assert_eq!(
            gradient
                .descendants()
                .find(|node| slices(*node))
                .unwrap()
                .attribute("d"),
            document
                .descendants()
                .find(|node| slices(*node))
                .unwrap()
                .attribute("d")
        );
    }
}
