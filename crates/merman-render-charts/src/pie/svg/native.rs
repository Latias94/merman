use merman_model::families::pie::PieDiagramModel;
use merman_render_core::{
    Result,
    number_format::canonical_number as fmt,
    svg_css::SvgId,
    svg_output::SvgOutput,
    text::TextStyle,
    theme_svg::{data_pattern, write_color},
    xml::{escape_attr, escape_attr_display, escape_xml},
};
use merman_theme_contract::v2::{Appearance, Color};

use super::write_pie_group_open;
use crate::pie::{
    PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_SPACING_PX, PIE_TITLE_BASELINE_Y_PX, PIE_TITLE_CLASS,
    PieDiagramLayout, PieNativeStyle, PieRenderSettings, native_legend_baseline_y,
    native_pie_slice_path, pie_group_offset, pie_legend_text, pie_slice_class,
};

/// Resolved canvas and text inputs supplied by the native-rendering host.
pub struct PieNativeSvgContext {
    pub title_vertical_extents: fn(&TextStyle) -> (f64, f64),
    pub canvas_background: Option<Color>,
    pub appearance_and_backdrop: Option<(Appearance, Option<Color>)>,
}

/// Writes the native definitions and chart groups. The caller closes the outer group.
/// Formats the host-normalized `diagram_id` for selectors and fragment references.
pub fn write_native_svg_body(
    out: &mut impl SvgOutput,
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    style: &PieNativeStyle,
    settings: PieRenderSettings<'_>,
    diagram_id: SvgId<'_>,
    context: &PieNativeSvgContext,
) -> Result<()> {
    let highlight = settings.highlight_slice;
    if !highlight.is_empty() {
        let _ = write!(
            out,
            "<style>#{diagram_id} .pieCircle.highlighted{{scale:1.05;}}#{diagram_id} .pieCircle.highlightedOnHover:hover{{transition-duration:250ms;scale:1.05;}}</style>"
        );
        out.checkpoint()?;
    }

    write_patterns(out, diagram_id, style, layout.legend_items.len())?;

    let offset = pie_group_offset(layout, settings.legend_position, style.content_width);

    write_pie_group_open(out, layout, offset);
    let inner_radius = settings.donut_hole * layout.radius;
    for (slice, &ordinal) in layout.slices.iter().zip(&style.slice_ordinals) {
        let d = native_pie_slice_path(slice, layout.radius, inner_radius, style.slice_gap_px);
        let class = pie_slice_class(highlight, &slice.label);
        let _ = write!(out, r#"<path d="{d}" class="{class}" "#);
        write_data_fill(out, diagram_id, style, ordinal);
        out.push_str("/>");
        out.checkpoint()?;
    }

    let percentage_ink =
        style.percentage_ink(context.canvas_background, context.appearance_and_backdrop);
    for (slice, &ordinal) in layout.slices.iter().zip(&style.slice_ordinals) {
        let _ = write!(
            out,
            r#"<text transform="translate({},{})" class="slice" text-anchor="middle""#,
            fmt(slice.text_x),
            fmt(slice.text_y)
        );
        let ink = percentage_ink(ordinal);
        if style.uses_pattern(ordinal) {
            // Keep a contrasting edge around the glyphs where they cross pattern marks.
            let black = Color::rgb(0, 0, 0);
            let white = Color::rgb(255, 255, 255);
            let outline = if ink.contrast(black) >= ink.contrast(white) {
                black
            } else {
                white
            };
            out.push_str(r#" paint-order="stroke" stroke-width="2" stroke-linejoin="round" "#);
            let _ = write_color(out, "stroke", outline, 1.0);
        }
        write_native_text(out, &style.label, ink, &format!("{}%", slice.percent))?;
    }
    out.push_str("</g>");

    if let Some(title) = layout.title.as_deref() {
        let _ = write!(
            out,
            r#"<text x="0" y="{}" class="{PIE_TITLE_CLASS}" text-anchor="middle""#,
            fmt(PIE_TITLE_BASELINE_Y_PX)
        );
        write_native_text(out, &style.title, style.title_color, title)?;
    }

    let legend_size = fmt(PIE_LEGEND_RECT_SIZE_PX);
    let legend_text_x = fmt(PIE_LEGEND_RECT_SIZE_PX + PIE_LEGEND_SPACING_PX);
    let legend_text_y = fmt(native_legend_baseline_y((context.title_vertical_extents)(
        &style.label,
    )));
    for (ordinal, item) in layout.legend_items.iter().enumerate() {
        let _ = write!(
            out,
            r#"<g class="legend" transform="translate({},{})"><rect width="{legend_size}" height="{legend_size}" "#,
            fmt(layout.legend_x),
            fmt(item.y)
        );
        write_data_fill(out, diagram_id, style, ordinal);
        let _ = write!(out, r#"/><text x="{legend_text_x}" y="{legend_text_y}""#);
        write_native_text(
            out,
            &style.label,
            style.legend_color,
            &pie_legend_text(&item.label, item.value, model.show_data),
        )?;
        out.push_str("</g>");
    }

    out.checkpoint()
}

fn write_patterns(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    style: &PieNativeStyle,
    category_count: usize,
) -> Result<()> {
    if !style.data.pattern_encoding || category_count < 2 {
        return Ok(());
    }

    out.push_str("<defs>");
    let mut pattern = String::new();
    for ordinal in (1..category_count).filter(|ordinal| style.uses_pattern(*ordinal)) {
        let id = format!("{diagram_id}-pie-data-{ordinal}");
        out.checkpoint()?;
        pattern.clear();
        data_pattern(
            &mut pattern,
            &id,
            ordinal,
            style.data.fill(ordinal),
            style.data.ink(ordinal),
        )
        .expect("writing a pie pattern to String cannot fail");
        out.push_str(&pattern);
        out.checkpoint()?;
    }
    out.push_str("</defs>");
    out.checkpoint()
}

fn write_data_fill(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    style: &PieNativeStyle,
    ordinal: usize,
) {
    if style.uses_pattern(ordinal) {
        let _ = write!(out, r#"fill="url(#{diagram_id}-pie-data-{ordinal})""#);
        if style.slice_opacity != 1.0 {
            let _ = write!(out, r#" fill-opacity="{}""#, fmt(style.slice_opacity));
        }
    } else {
        let _ = write_color(out, "fill", style.data.fill(ordinal), style.slice_opacity);
    }
}

fn write_native_text(
    out: &mut impl SvgOutput,
    style: &TextStyle,
    fill: Color,
    text: &str,
) -> Result<()> {
    let _ = write!(
        out,
        r#" font-family="{}" font-size="{}" font-weight="{}" "#,
        escape_attr(style.font_family.as_deref().unwrap_or_default()),
        fmt(style.font_size),
        escape_attr_display(style.font_weight.as_deref().unwrap_or("normal"))
    );
    let _ = write_color(out, "fill", fill, 1.0);
    let _ = write!(out, ">{}</text>", escape_xml(text));
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use merman_model::families::pie::PieSection;
    use merman_render_core::{
        Error,
        resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId},
        svg_output::BoundedSvgOutput,
        text::{DeterministicTextMeasurer, svg_title_bbox_vertical_extents_px},
    };
    use merman_theme_contract::v2::{BuiltinPreset, ThemeCompiler};
    use svgtypes::{PathParser, PathSegment};

    use super::*;
    use crate::pie::{PieAppearance, PieConfigView, PieSliceLayout, prepare_pie_diagram};

    #[test]
    fn native_pie_body_reports_a_rejected_last_legend_close() {
        let model = PieDiagramModel {
            sections: vec![PieSection {
                label: "A".into(),
                value: 1.0,
            }],
            ..Default::default()
        };
        let config = serde_json::json!({});
        let theme = ThemeCompiler::new()
            .compile_preset(BuiltinPreset::Paper, None)
            .unwrap();
        let (layout, appearance) = prepare_pie_diagram(
            &model,
            None,
            &config,
            Some(theme.family("pie")),
            &DeterministicTextMeasurer::default(),
        )
        .unwrap();
        let PieAppearance::Native(style) = appearance else {
            panic!("expected the native Pie style");
        };
        let settings = PieConfigView::new(&config).render_settings();
        let context = PieNativeSvgContext {
            title_vertical_extents: svg_title_bbox_vertical_extents_px,
            canvas_background: None,
            appearance_and_backdrop: None,
        };
        let diagram_id = SvgId::new("merman").unwrap();
        let mut expected = String::new();
        write_native_svg_body(
            &mut expected,
            &layout,
            &model,
            &style,
            settings,
            diagram_id,
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
            diagram_id,
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
        let slice = PieSliceLayout {
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
}
