use super::super::*;
use super::geometry::{
    open_pie_svg, pie_group_offset, pie_slice_class, pie_slice_path, pie_viewport,
    write_pie_group_open,
};
use crate::pie::{
    PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_SPACING_PX, PIE_TITLE_BASELINE_Y_PX, PIE_TITLE_CLASS,
    PieConfigView, PieNativeStyle, native_legend_baseline_y, pie_legend_text,
};
use crate::theme::{Color, canvas_svg::write_color, pattern_svg::data_pattern};
use merman_model::families::pie::PieDiagramModel;
use merman_theme_contract::v2::Paint;

pub(crate) fn render_native_pie_svg(
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    style: &PieNativeStyle,
    effective_config: &impl crate::config::ConfigLookup,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    let settings = PieConfigView::new(effective_config).render_settings();
    let offset = pie_group_offset(layout, settings.legend_position, style.content_width);
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root = open_pie_svg(
        &mut out,
        model,
        diagram_id,
        pie_viewport(layout, model),
        settings.use_max_width,
        options,
    )?;
    let highlight = crate::config::config_string(effective_config, &["pie", "highlightSlice"])
        .unwrap_or_default();
    if !highlight.is_empty() {
        let id = util::css_selector_diagram_id(diagram_id);
        let _ = write!(
            &mut out,
            "<style>#{id} .pieCircle.highlighted{{scale:1.05;}}#{id} .pieCircle.highlightedOnHover:hover{{transition-duration:250ms;scale:1.05;}}</style>"
        );
        out.checkpoint()?;
    }

    write_patterns(&mut out, diagram_id, style, layout.legend_items.len())?;
    let session = options.family.session();
    let background = session.canvas_background();
    let background_paint = background.map(Paint::Solid);
    let gap_paint = (style.slice_gap_px > 0.0 && !layout.slices.is_empty())
        .then_some(background_paint.as_ref().unwrap_or(&style.canvas));
    if matches!(gap_paint, Some(Paint::LinearGradient(_))) {
        let view_box = root.view_box().ok_or_else(|| Error::InvalidModel {
            message: "Pie gap gradient requires a root viewBox".to_owned(),
        })?;
        write_gap_gradient(
            &mut out,
            diagram_id,
            view_box,
            (layout.center_x + offset.0, layout.center_y + offset.1),
        )?;
    }

    write_pie_group_open(&mut out, layout, offset);
    let inner_radius = settings.donut_hole * layout.radius;
    for (slice, &ordinal) in layout.slices.iter().zip(&style.slice_ordinals) {
        let d = pie_slice_path(slice, layout.radius, inner_radius);
        let class = pie_slice_class(&highlight, &slice.label);
        let _ = write!(&mut out, r#"<path d="{d}" class="{class}" "#);
        write_data_fill(&mut out, diagram_id, style, ordinal);
        if let Some(paint) = gap_paint {
            let _ = write!(
                &mut out,
                r#" stroke-width="{}" stroke-linejoin="round" "#,
                fmt(style.slice_gap_px)
            );
            match paint {
                Paint::Solid(color) => {
                    let _ = write_color(&mut out, "stroke", *color, 1.0);
                }
                Paint::LinearGradient(_) => {
                    let _ = write!(&mut out, r#"stroke="url(#{diagram_id}-pie-gap)""#);
                }
            }
        }
        out.push_str("/>");
        out.checkpoint()?;
    }

    let host_canvas = background.zip(
        session
            .native_theme()
            .map(|theme| theme.resolved().appearance()),
    );
    for (slice, &ordinal) in layout.slices.iter().zip(&style.slice_ordinals) {
        let _ = write!(
            &mut out,
            r#"<text transform="translate({},{})" class="slice" text-anchor="middle""#,
            fmt(slice.text_x),
            fmt(slice.text_y)
        );
        let ink = match host_canvas {
            Some((canvas, appearance)) => {
                style
                    .data
                    .fill_ink_for_canvas(ordinal, canvas, style.slice_opacity, appearance)
            }
            None => style.data.fill_ink(ordinal),
        };
        write_text(&mut out, &style.label, ink, &format!("{}%", slice.percent))?;
    }
    out.push_str("</g>");

    if let Some(title) = layout.title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<text x="0" y="{}" class="{PIE_TITLE_CLASS}" text-anchor="middle""#,
            fmt(PIE_TITLE_BASELINE_Y_PX)
        );
        write_text(&mut out, &style.title, style.title_color, title)?;
    }

    let legend_size = fmt(PIE_LEGEND_RECT_SIZE_PX);
    let legend_text_x = fmt(PIE_LEGEND_RECT_SIZE_PX + PIE_LEGEND_SPACING_PX);
    let legend_text_y = fmt(native_legend_baseline_y(&style.label));
    for (ordinal, item) in layout.legend_items.iter().enumerate() {
        let _ = write!(
            &mut out,
            r#"<g class="legend" transform="translate({},{})"><rect width="{legend_size}" height="{legend_size}" "#,
            fmt(layout.legend_x),
            fmt(item.y)
        );
        write_data_fill(&mut out, diagram_id, style, ordinal);
        let _ = write!(
            &mut out,
            r#"/><text x="{legend_text_x}" y="{legend_text_y}""#
        );
        write_text(
            &mut out,
            &style.label,
            style.legend_color,
            &pie_legend_text(&item.label, item.value, model.show_data),
        )?;
        out.push_str("</g>");
    }
    out.push_str("</g></svg>\n");
    root.complete(out.finish()?)
}

fn uses_pattern(style: &PieNativeStyle, ordinal: usize) -> bool {
    style.data.pattern_encoding && !ordinal.is_multiple_of(6)
}

fn write_patterns(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    style: &PieNativeStyle,
    category_count: usize,
) -> Result<()> {
    if !style.data.pattern_encoding || category_count < 2 {
        return Ok(());
    }

    out.push_str("<defs>");
    let mut pattern = String::new();
    for ordinal in (1..category_count).filter(|ordinal| uses_pattern(style, *ordinal)) {
        let id = format!("{diagram_id}-pie-data-{ordinal}");
        out.checkpoint()?;
        pattern.clear();
        data_pattern(
            &mut pattern,
            &id,
            ordinal,
            style.data.fill(ordinal),
            style.data.ink(ordinal),
        );
        out.push_str(&pattern);
        out.checkpoint()?;
    }
    out.push_str("</defs>");
    out.checkpoint()
}

fn write_data_fill(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    style: &PieNativeStyle,
    ordinal: usize,
) {
    if uses_pattern(style, ordinal) {
        let _ = write!(out, r#"fill="url(#{diagram_id}-pie-data-{ordinal})""#);
        if style.slice_opacity != 1.0 {
            let _ = write!(out, r#" fill-opacity="{}""#, fmt(style.slice_opacity));
        }
    } else {
        let _ = write_color(out, "fill", style.data.fill(ordinal), style.slice_opacity);
    }
}

fn write_gap_gradient(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    view_box: root_svg::ViewBox,
    center: (f64, f64),
) -> Result<()> {
    // The canvas gradient uses normalized viewport coordinates. Project those same coordinates
    // into the translated pie group so every sector shares the canvas's gradient.
    let (x, y) = center;
    let _ = write!(
        out,
        r##"<defs><linearGradient id="{diagram_id}-pie-gap" xlink:href="#{diagram_id}-merman-canvas-base" gradientUnits="userSpaceOnUse" gradientTransform="translate({},{}) scale({},{})"/></defs>"##,
        fmt(view_box.min_x - x),
        fmt(view_box.min_y - y),
        fmt(view_box.width),
        fmt(view_box.height)
    );
    out.checkpoint()
}

fn write_text(out: &mut impl SvgOutput, style: &TextStyle, fill: Color, text: &str) -> Result<()> {
    let _ = write!(
        out,
        r#" font-family="{}" font-size="{}" font-weight="{}" "#,
        escape_attr(style.font_family.as_deref().unwrap_or_default()),
        fmt(style.font_size),
        style.font_weight.as_deref().unwrap_or("normal")
    );
    let _ = write_color(out, "fill", fill, 1.0);
    let _ = write!(out, ">{}</text>", escape_xml(text));
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use crate::environment::RenderEnvironment;
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::theme::{Color, NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};
    use merman_core::{MermaidEngine, ParseOptions};

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
        let parity_ds: Vec<_> = parity
            .descendants()
            .filter(|node| slices(*node))
            .map(|node| node.attribute("d"))
            .collect();
        assert_eq!(
            paths
                .iter()
                .map(|node| node.attribute("d"))
                .collect::<Vec<_>>(),
            parity_ds
        );

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
            assert_eq!(path.attribute("stroke"), Some("#FAFAFA"));
            assert_eq!(path.attribute("stroke-width"), Some("2.25"));
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
        let gap = gradient
            .descendants()
            .find(|node| node.attribute("id") == Some("capacity-pie-gap"))
            .unwrap();
        assert_eq!(gap.attribute("gradientUnits"), Some("userSpaceOnUse"));
        assert_eq!(
            gap.attribute(("http://www.w3.org/1999/xlink", "href")),
            Some("#capacity-merman-canvas-base")
        );
        assert!(gradient.descendants().any(|node| {
            node.has_tag_name("linearGradient")
                && node.attribute("id") == Some("capacity-merman-canvas-base")
        }));
        let slice = gradient.descendants().find(|node| slices(*node)).unwrap();
        assert_eq!(slice.attribute("stroke"), Some("url(#capacity-pie-gap)"));

        let view_box = gradient
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .parse::<svgtypes::ViewBox>()
            .unwrap();
        let gap_transform = gap
            .attribute("gradientTransform")
            .unwrap()
            .parse::<svgtypes::Transform>()
            .unwrap();
        let (pie_x, pie_y) = slice
            .ancestors()
            .filter(|node| node.has_tag_name("g"))
            .filter_map(|node| node.attribute("transform"))
            .map(|transform| transform.parse::<svgtypes::Transform>().unwrap())
            .fold((0.0, 0.0), |(x, y), transform| {
                (x + transform.e, y + transform.f)
            });
        assert_eq!((gap_transform.a, gap_transform.d), (view_box.w, view_box.h));
        assert!((gap_transform.e + pie_x - view_box.x).abs() < 1e-9);
        assert!((gap_transform.f + pie_y - view_box.y).abs() < 1e-9);
    }
}
