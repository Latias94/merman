use merman_model::OperationPhase;
use merman_render_core::{
    Error, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::{
        DiagramBounds, RootChrome, RootDomProfile, RootViewportContext, RootViewportSpec,
        RootedSvg, SvgRootStyleViewBoxOrder,
    },
    svg_css::SvgId,
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    text::{TextMeasurer, TextStyle},
    theme_svg::data_pattern,
    xml::{escape_attr, escape_xml},
};
use std::fmt::Write as _;

use super::super::{
    TREEMAP_SECTION_HEADER_HEIGHT_PX, TREEMAP_SECTION_INNER_PADDING_PX, TREEMAP_TITLE_CLASS,
    TreemapDiagramLayout, TreemapStyle, css::treemap_css, format_treemap_value, treemap_view_box,
};
use super::{
    TreemapLeafRectPaint, write_treemap_leaf_group_open, write_treemap_leaf_rect_and_clip,
};

fn measure_treemap_computed_length(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    work_meter: &OperationWorkMeter,
) -> Result<f64> {
    work_meter.charge(1usize.saturating_add(text.len().div_ceil(64)))?;
    Ok(measurer.measure_svg_text_computed_length_px(text, style))
}

fn fit_treemap_label_height(
    font_size: f64,
    available_height: f64,
    min_label_size: f64,
    base_value_size: f64,
    min_value_size: f64,
    spacing: f64,
    work_meter: &OperationWorkMeter,
) -> Result<f64> {
    let combined_height = |size: f64| {
        size + spacing
            + (size * 0.6)
                .round()
                .min(base_value_size)
                .max(min_value_size)
    };
    work_meter.charge_emit_work(1)?;
    if font_size <= min_label_size || combined_height(font_size) <= available_height {
        return Ok(font_size);
    }
    if combined_height(min_label_size) > available_height {
        return Ok(min_label_size);
    }

    // Font sizes are truncated before fitting. The original decrement loop selects the
    // greatest fitting integer, or the minimum even when it does not fit. Search the same
    // monotone predicate without assuming that subtracting 1 changes a large f64.
    let mut fitting = min_label_size;
    let mut too_large = font_size;
    loop {
        work_meter.charge_emit_work(1)?;
        let candidate = (fitting + (too_large - fitting) / 2.0).floor();
        if candidate <= fitting || candidate >= too_large {
            return Ok(fitting);
        }
        if combined_height(candidate) <= available_height {
            fitting = candidate;
        } else {
            too_large = candidate;
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Text fitting needs the label, its style and the available geometry."
)]
fn fit_treemap_label_width(
    font_size: f64,
    available_width: f64,
    min_label_size: f64,
    text: &str,
    style: &mut TextStyle,
    measurer: &dyn TextMeasurer,
    work_meter: &OperationWorkMeter,
) -> Result<f64> {
    const MAX_WIDTH_SEARCH_STEPS: usize = 2_048;

    if !font_size.is_finite() || !min_label_size.is_finite() {
        return Err(Error::InvalidModel {
            message: "Treemap label font size must be finite during width fitting".to_owned(),
        });
    }

    if measure_treemap_computed_length(measurer, text, style, work_meter)? <= available_width
        || font_size <= min_label_size
    {
        return Ok(font_size);
    }

    // The Mermaid loop walks down by one CSS pixel. Preserve its result for the monotone text
    // measurement used by the renderer, but search the integer range in logarithmic work so a
    // huge authored font size cannot force millions of host measurements.
    let mut fitting = min_label_size;
    let mut too_large = font_size;
    for _ in 0..MAX_WIDTH_SEARCH_STEPS {
        work_meter.charge_emit_work(1)?;
        let candidate = (fitting + (too_large - fitting) / 2.0).floor();
        if candidate <= fitting || candidate >= too_large {
            break;
        }
        style.font_size = candidate;
        if measure_treemap_computed_length(measurer, text, style, work_meter)? <= available_width {
            fitting = candidate;
        } else {
            too_large = candidate;
        }
    }
    style.font_size = fitting;
    Ok(fitting)
}

/// Emits the complete Treemap SVG before host canvas and terminal admission.
pub fn render_treemap_diagram_svg(
    layout: &TreemapDiagramLayout,
    style: &TreemapStyle,
    diagram_id: SvgId<'_>,
    root_viewport: &RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
    bbox_measurer: &dyn TextMeasurer,
    computed_length_measurer: &dyn TextMeasurer,
) -> Result<RootedSvg> {
    let has_acc_title = layout.acc_title.as_deref().is_some_and(|s| !s.is_empty());
    let has_acc_descr = layout.acc_descr.as_deref().is_some_and(|s| !s.is_empty());

    let title = layout.title.as_deref().filter(|t| !t.trim().is_empty());
    let title_bbox = if let Some(title) = title {
        let title_text_style = style.title_text_style();
        let measurement_work = 1usize.saturating_add(title.len().div_ceil(64));
        work_meter.charge(measurement_work.saturating_mul(2))?;
        let w = bbox_measurer
            .measure_svg_raw_text_bbox_width_px(title, &title_text_style)
            .max(0.0);
        let h = bbox_measurer
            .measure_svg_raw_text_bbox_height_px(title, &title_text_style)
            .max(0.0);
        Some((w, h))
    } else {
        None
    };

    let (vb_x, vb_y, vb_w, vb_h) = treemap_view_box(layout, title_bbox);

    let mut out = BoundedSvgOutput::new(work_meter);
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let extra_attrs: [(&str, &str); 1] = [("class", "flowchart")];
    let mut root_chrome = RootChrome::new(diagram_id, "treemap");
    root_chrome.extra_attrs = &extra_attrs;
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = RootDomProfile {
        style_viewbox_order: SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
        trailing_newline: false,
        ..RootDomProfile::default()
    };
    let root_document = root_viewport.write_open(
        &mut out,
        RootViewportSpec::mermaid(
            DiagramBounds::from_view_box(vb_x, vb_y, vb_w, vb_h),
            layout.use_max_width,
        ),
        root_chrome,
    )?;
    out.checkpoint()?;

    if let (Some(title), true) = (layout.acc_title.as_deref(), has_acc_title) {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
        out.checkpoint()?;
    }
    if let (Some(descr), true) = (layout.acc_descr.as_deref(), has_acc_descr) {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(descr)
        );
        out.checkpoint()?;
    }

    let css = treemap_css(diagram_id, style)?;
    out.push_str("<style>");
    XmlTextOutput::new(&mut out).push_str(&css);
    out.push_str("</style>");
    drop(css);
    if let Some(palette) = &style.native_patterns {
        out.push_str("<defs>");
        for slot in 1..palette.fill.len() {
            if slot % 6 != 0 {
                let id = format!("{diagram_id}-treemap-pattern-{slot}");
                let _ = data_pattern(&mut out, &id, slot, palette.fill(slot), palette.ink(slot));
                out.checkpoint()?;
            }
        }
        out.push_str("</defs>");
    }
    out.push_str("<g/>");
    out.checkpoint()?;

    if let Some(title) = layout.title.as_deref().filter(|t| !t.trim().is_empty()) {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}" class="{class}" text-anchor="middle" dominant-baseline="middle">{text}</text>"#,
            x = fmt(layout.width / 2.0),
            y = fmt(layout.title_height / 2.0),
            class = TREEMAP_TITLE_CLASS,
            text = escape_xml(title)
        );
        out.checkpoint()?;
    }

    let _ = write!(
        &mut out,
        r#"<g transform="translate(0, {ty})" class="treemapContainer">"#,
        ty = fmt(layout.title_height)
    );
    out.checkpoint()?;

    let section_header_height = TREEMAP_SECTION_HEADER_HEIGHT_PX;
    let section_header_center_y = section_header_height / 2.0;
    let section_label_inset_x: f64 = 6.0;
    let section_label_font_size = style.native_base_size.unwrap_or(12.0);
    let section_value_font_size = style.native_base_size.map_or(10.0, |size| size * 0.8);
    let section_inner_padding = TREEMAP_SECTION_INNER_PADDING_PX;
    let section_label_reserved_value_width: f64 = 30.0;
    let section_label_min_visible_width: f64 = 15.0;

    for (i, section) in layout.sections.iter().enumerate() {
        let section_clip_id = format!("clip-section-{diagram_id}-{i}");
        work_meter.checkpoint(OperationPhase::Emit)?;
        work_meter.charge(1)?;
        let w = section.x1 - section.x0;
        let h = section.y1 - section.y0;
        let _ = write!(
            &mut out,
            r#"<g class="treemapSection" transform="translate({x},{y})">"#,
            x = fmt(section.x0),
            y = fmt(section.y0)
        );
        out.checkpoint()?;

        let header_style = if section.depth == 0 {
            "display: none;"
        } else {
            ""
        };
        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{hh}" class="treemapSectionHeader" fill="none" fill-opacity="0.6" stroke-width="0.6" style="{style}"/>"#,
            w = fmt(w),
            hh = fmt(section_header_height),
            style = header_style
        );
        out.checkpoint()?;

        let _ = write!(
            &mut out,
            r#"<clipPath id="{id}"><rect width="{w}" height="{h}"/></clipPath>"#,
            id = section_clip_id.as_str(),
            w = fmt((w - 2.0 * section_label_inset_x).max(0.0)),
            h = fmt(section_header_height)
        );
        out.checkpoint()?;

        let visual = style.section_visual(i).ok_or_else(|| Error::InvalidModel {
            message: format!("Treemap prepared section visual missing index {i}"),
        })?;
        let fill = visual.fill.as_str();
        let stroke = visual.stroke.as_str();
        let section_label_text_style =
            style.section_text_style(i, section_label_font_size, Some("bold"), None);
        let section_style = visual.rect_style.as_str();
        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{h}" class="treemapSection section{i}" fill="{fill}" fill-opacity="0.6" stroke="{stroke}" stroke-width="2" stroke-opacity="0.4" style="{style}"/>"#,
            w = fmt(w),
            h = fmt(h),
            i = i,
            fill = escape_attr(fill),
            stroke = escape_attr(stroke),
            style = escape_attr(section_style)
        );
        out.checkpoint()?;

        let mut label_text = if section.depth == 0 {
            String::new()
        } else {
            work_meter.charge(1usize.saturating_add(section.name.len().div_ceil(64)))?;
            section.name.clone()
        };

        let label_fill = visual.label_fill.as_str();
        let value_fill = visual.value_fill.as_str();
        let label_styles_suffix = visual.label_styles_suffix.as_str();

        if label_text.is_empty() {
            let _ = write!(
                &mut out,
                r#"<text class="treemapSectionLabel" x="{x}" y="{y}" dominant-baseline="middle" font-weight="bold" clip-path="url(#{id})" style="display: none;"/>"#,
                x = fmt(section_label_inset_x),
                y = fmt(section_header_center_y),
                id = section_clip_id.as_str(),
            );
            out.checkpoint()?;
        } else {
            // Mirror Mermaid's truncation loop in `renderer.ts` (uses `getComputedTextLength()`).
            let total_header_width = w;
            let label_x_position = section_label_inset_x;
            let mut space_for_text_content =
                total_header_width - label_x_position - section_label_inset_x;
            if layout.show_values && section.value != 0.0 {
                let value_ends_at_x_relative = total_header_width - section_inner_padding;
                let estimated_value_text_actual_width = section_label_reserved_value_width;
                let gap_between_label_and_value = section_inner_padding;
                let label_must_end_before_x = value_ends_at_x_relative
                    - estimated_value_text_actual_width
                    - gap_between_label_and_value;
                space_for_text_content = label_must_end_before_x - label_x_position;
            }
            let actual_available_width =
                section_label_min_visible_width.max(space_for_text_content);

            let style = section_label_text_style.style();
            if measure_treemap_computed_length(
                computed_length_measurer,
                &label_text,
                style,
                work_meter,
            )? > actual_available_width
            {
                let ellipsis = "...";
                work_meter.charge(1usize.saturating_add(label_text.len().div_ceil(64)))?;
                let candidate_capacity = label_text
                    .len()
                    .checked_add(ellipsis.len())
                    .ok_or_else(|| work_meter.arithmetic_overflow())?;
                let mut candidate = String::with_capacity(candidate_capacity);
                candidate.push_str(&label_text);
                while !candidate.is_empty() {
                    candidate.pop();
                    if candidate.is_empty() {
                        if measure_treemap_computed_length(
                            computed_length_measurer,
                            ellipsis,
                            style,
                            work_meter,
                        )? > actual_available_width
                        {
                            label_text.clear();
                        } else {
                            label_text = ellipsis.to_string();
                        }
                        break;
                    }
                    candidate.push_str(ellipsis);
                    if measure_treemap_computed_length(
                        computed_length_measurer,
                        &candidate,
                        style,
                        work_meter,
                    )? <= actual_available_width
                    {
                        label_text = candidate;
                        break;
                    }
                    candidate.truncate(candidate.len() - ellipsis.len());
                }
            }

            let section_label_style = format!(
                "dominant-baseline: middle; font-size: {}px; fill:{fill}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;{suffix}",
                fmt(section_label_font_size),
                fill = escape_attr(label_fill),
                suffix = label_styles_suffix
            );
            let _ = write!(
                &mut out,
                r#"<text class="treemapSectionLabel" x="{x}" y="{y}" dominant-baseline="middle" font-weight="bold" clip-path="url(#{id})" style="{style}">{text}</text>"#,
                x = fmt(section_label_inset_x),
                y = fmt(section_header_center_y),
                id = section_clip_id.as_str(),
                style = escape_attr(&section_label_style),
                text = escape_xml(&label_text)
            );
            out.checkpoint()?;
        }

        if layout.show_values {
            let value_text = if section.value != 0.0 {
                format_treemap_value(section.value, &layout.value_format)?
            } else {
                String::new()
            };
            let section_value_style = if section.depth == 0 {
                "display: none;".to_string()
            } else {
                format!(
                    "text-anchor: end; dominant-baseline: middle; font-size: {}px; fill:{fill}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;{suffix}",
                    fmt(section_value_font_size),
                    fill = escape_attr(value_fill),
                    suffix = label_styles_suffix
                )
            };
            if value_text.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapSectionValue" x="{x}" y="{y}" text-anchor="end" dominant-baseline="middle" font-style="italic" style="{style}"/>"#,
                    x = fmt(w - section_inner_padding),
                    y = fmt(section_header_center_y),
                    style = escape_attr(&section_value_style)
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapSectionValue" x="{x}" y="{y}" text-anchor="end" dominant-baseline="middle" font-style="italic" style="{style}">{text}</text>"#,
                    x = fmt(w - section_inner_padding),
                    y = fmt(section_header_center_y),
                    style = escape_attr(&section_value_style),
                    text = escape_xml(&value_text)
                );
            }
            out.checkpoint()?;
        }

        out.push_str("</g>");
        out.checkpoint()?;
    }

    let is_complex_treemap = layout.leaves.len() > 20;
    let base_label_font_size =
        style
            .native_base_size
            .unwrap_or(if is_complex_treemap { 16.0 } else { 38.0 });
    let base_value_font_size = style
        .native_base_size
        .map_or(if is_complex_treemap { 14.0 } else { 28.0 }, |size| {
            size * 0.8
        });
    let min_label_font_size = if is_complex_treemap { 4.0 } else { 8.0 };
    let min_value_font_size = if is_complex_treemap { 4.0 } else { 6.0 };
    let label_padding = if is_complex_treemap { 2.0 } else { 4.0 };
    let min_display_threshold = if is_complex_treemap { 8.0 } else { 10.0 };
    let spacing_between_label_and_value = if is_complex_treemap { 1.0 } else { 2.0 };

    for (i, leaf) in layout.leaves.iter().enumerate() {
        let leaf_clip_id = format!("clip-{diagram_id}-{i}");
        work_meter.checkpoint(OperationPhase::Emit)?;
        work_meter.charge(1)?;
        let w = leaf.x1 - leaf.x0;
        let h = leaf.y1 - leaf.y0;

        let group_class = if let Some(cls) = leaf
            .class_selector
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            format!("treemapNode treemapLeafGroup leaf{i} {cls}x")
        } else {
            format!("treemapNode treemapLeafGroup leaf{i}x")
        };

        let visual = style.leaf_visual(i).ok_or_else(|| Error::InvalidModel {
            message: format!("Treemap prepared leaf visual missing index {i}"),
        })?;
        let fill = visual.fill.as_str();
        let leaf_rect_style = visual.rect_style.as_str();
        let label_styles_suffix = visual.label_styles_suffix.as_str();
        let label_styles_without_font_size_suffix =
            visual.label_styles_without_font_size_suffix.as_str();
        let leaf_label_fill = visual.label_fill.as_str();

        write_treemap_leaf_group_open(&mut out, &group_class, leaf.x0, leaf.y0)?;

        let native_fill = style
            .native_patterns
            .as_ref()
            .zip(visual.native_category)
            .and_then(|(palette, ordinal)| {
                let slot = ordinal % palette.fill.len();
                (slot % 6 != 0).then(|| format!("url(#{diagram_id}-treemap-pattern-{slot})"))
            });
        let fill = native_fill.as_deref().unwrap_or(fill);
        write_treemap_leaf_rect_and_clip(
            &mut out,
            w,
            h,
            &leaf_clip_id,
            TreemapLeafRectPaint {
                fill,
                stroke: &visual.stroke,
                style: leaf_rect_style,
                native_radius: style.native_radius,
            },
        )?;

        let available_w = w - 2.0 * label_padding;
        let available_h = h - 2.0 * label_padding;

        let leaf_label_initial_style = style.leaf_text_style(i, base_label_font_size);
        let mut final_label_text_style = leaf_label_initial_style.clone();
        let mut label_font_size = leaf_label_initial_style.fitting_font_size();
        let value_scale_factor = 0.6;

        let mut label_hidden = false;
        let mut label_font_size_mutated = false;
        if available_w < min_display_threshold || available_h < min_display_threshold {
            label_hidden = true;
        } else {
            let mut style = leaf_label_initial_style.style().clone();
            if let Some(size) = label_font_size {
                let size = fit_treemap_label_width(
                    size,
                    available_w,
                    min_label_font_size,
                    &leaf.name,
                    &mut style,
                    computed_length_measurer,
                    work_meter,
                )?;
                let size = fit_treemap_label_height(
                    size,
                    available_h,
                    min_label_font_size,
                    base_value_font_size,
                    min_value_font_size,
                    spacing_between_label_and_value,
                    work_meter,
                )?;
                label_font_size = Some(size);
                style.font_size = size;
                final_label_text_style = leaf_label_initial_style.with_font_size_px(size);
                label_font_size_mutated =
                    leaf_label_initial_style.accepts_font_size_assignment(size);
            } else {
                // D3 reads the inline CSSOM keyword, so parseInt produces NaN. The width
                // condition still measures the used style, but neither decrement loop runs.
                measure_treemap_computed_length(
                    computed_length_measurer,
                    &leaf.name,
                    &style,
                    work_meter,
                )?;
            }

            if is_complex_treemap {
                if label_font_size.is_some_and(|size| size < min_label_font_size)
                    || available_h < min_label_font_size
                {
                    label_hidden = true;
                }
            } else {
                if measure_treemap_computed_length(
                    computed_length_measurer,
                    &leaf.name,
                    &style,
                    work_meter,
                )? > available_w
                    || label_font_size
                        .is_some_and(|size| size < min_label_font_size || available_h < size)
                {
                    label_hidden = true;
                }
            }
            // The final setProperty rejects NaNpx. Valid pixel assignments retain the writer's
            // canonical serialization; rejected assignments preserve the authored spelling.
        }

        let label_style = if !label_font_size_mutated {
            let mut style = format!(
                "text-anchor: middle; dominant-baseline: middle; font-size: {font_size}px;fill:{fill};{suffix}",
                font_size = fmt(base_label_font_size),
                fill = escape_attr(leaf_label_fill),
                suffix = label_styles_suffix
            );
            if label_hidden {
                if !style.trim_end().ends_with(';') {
                    style.push(';');
                }
                style.push_str(" display: none;");
            }
            style
        } else {
            let fill = visual.fitted_label_fill.as_str();
            let mut s = format!(
                "text-anchor: middle; dominant-baseline: middle; font-size: {fs}px; fill: {fill};",
                fs = fmt(final_label_text_style.style().font_size),
                fill = escape_attr(fill),
            );
            if label_hidden {
                s.push_str(" display: none;");
            }
            if !label_styles_without_font_size_suffix.is_empty() {
                s.push_str(label_styles_without_font_size_suffix);
            }
            s
        };

        let _ = write!(
            &mut out,
            r#"<text class="treemapLabel" x="{x}" y="{y}" style="{style}" clip-path="url(#{id})">{text}</text>"#,
            x = fmt(w / 2.0),
            y = fmt(h / 2.0),
            style = escape_attr(&label_style),
            id = leaf_clip_id.as_str(),
            text = escape_xml(&leaf.name)
        );
        out.checkpoint()?;

        if layout.show_values {
            let value_text = if leaf.value != 0.0 {
                format_treemap_value(leaf.value, &layout.value_format)?
            } else {
                String::new()
            };
            let leaf_value_initial_style = style.leaf_text_style(i, base_value_font_size);
            let mut final_value_text_style = leaf_value_initial_style.clone();
            let mut value_font_size_mutated = false;
            let mut value_y = Some(h / 2.0); // Source placeholder when the label is hidden.
            let mut value_hidden = true;

            if !label_hidden {
                // Unlike the fitting loops, this pass uses parseFloat on the final inline
                // CSSOM value. A retained keyword rejects NaNpx and keeps the value style.
                let final_label_font_size = final_label_text_style.inline_font_size_number();
                let actual_value_font_size = final_label_font_size.map(|size| {
                    (size * value_scale_factor)
                        .round()
                        .min(base_value_font_size)
                        .max(min_value_font_size)
                });
                if let Some(size) = actual_value_font_size {
                    value_font_size_mutated =
                        leaf_value_initial_style.accepts_font_size_assignment(size);
                    final_value_text_style = leaf_value_initial_style.with_font_size_px(size);
                }

                let label_center_y = h / 2.0;
                value_y = final_label_font_size
                    .map(|size| label_center_y + size / 2.0 + spacing_between_label_and_value)
                    .filter(|y| y.is_finite());

                let cell_bottom_padding = 4.0;
                let max_value_bottom_y = h - cell_bottom_padding;
                let available_w_for_value = w - 2.0 * label_padding;

                let value_w_px = measure_treemap_computed_length(
                    computed_length_measurer,
                    &value_text,
                    final_value_text_style.style(),
                    work_meter,
                )?;
                value_hidden = value_w_px > available_w_for_value
                    || actual_value_font_size.is_some_and(|size| {
                        value_y.is_some_and(|y| y + size > max_value_bottom_y)
                            || size < min_value_font_size
                    });
            }

            let value_fill = visual.value_fill.as_str();
            let mut value_style = if value_font_size_mutated {
                let fill = visual.fitted_value_fill.as_str();
                format!(
                    "text-anchor: middle; dominant-baseline: hanging; font-size: {fs}px; fill: {fill};",
                    fs = fmt(final_value_text_style.style().font_size),
                    fill = escape_attr(fill)
                )
            } else {
                format!(
                    "text-anchor: middle; dominant-baseline: hanging; font-size: {fs}px;fill:{fill};{suffix}",
                    fs = fmt(base_value_font_size),
                    fill = escape_attr(value_fill),
                    suffix = label_styles_suffix,
                )
            };
            if value_hidden {
                if !value_style.trim_end().ends_with(';') {
                    value_style.push(';');
                }
                value_style.push_str(" display: none;");
            }
            if value_font_size_mutated && !label_styles_without_font_size_suffix.is_empty() {
                value_style.push_str(label_styles_without_font_size_suffix);
            }

            let value_y_attribute = value_y
                .map(|y| format!(r#" y="{}""#, fmt(y)))
                .unwrap_or_default();
            if value_text.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapValue" x="{x}"{y_attribute} style="{style}" clip-path="url(#{id})"/>"#,
                    x = fmt(w / 2.0),
                    y_attribute = value_y_attribute,
                    style = escape_attr(&value_style),
                    id = leaf_clip_id.as_str(),
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapValue" x="{x}"{y_attribute} style="{style}" clip-path="url(#{id})">{text}</text>"#,
                    x = fmt(w / 2.0),
                    y_attribute = value_y_attribute,
                    style = escape_attr(&value_style),
                    id = leaf_clip_id.as_str(),
                    text = escape_xml(&value_text)
                );
            }
            out.checkpoint()?;
        }

        out.push_str("</g>");
        out.checkpoint()?;
    }

    out.push_str("</g></svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treemap::{
        TreemapLeafLayout, TreemapSectionLayout,
        test_support::{FALLBACK_C_SCALE, presentation_config},
    };
    use merman_model::family::DiagramFamilyId;
    use merman_render_core::{
        resources::{RenderResourcePolicy, ResourceLimitId},
        svg_css::SvgDiagramIdProjection,
        text::{
            TextMetrics,
            measurement::{TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementSession},
        },
    };
    use std::{fmt, ops::Range, sync::Arc};

    #[test]
    fn treemap_height_search_matches_decrement_and_bounds_extreme_work() {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        for (minimum, base_value, min_value, spacing) in
            [(8.0, 28.0, 6.0, 2.0), (4.0, 14.0, 4.0, 1.0)]
        {
            for initial in 1..=128 {
                for height in 0..=256 {
                    for fraction in [0.0, 0.25, 0.999999] {
                        let height = height as f64 + fraction;
                        let mut expected = initial as f64;
                        while expected > minimum
                            && expected
                                + spacing
                                + (expected * 0.6).round().min(base_value).max(min_value)
                                > height
                        {
                            expected -= 1.0;
                        }
                        let actual = fit_treemap_label_height(
                            initial as f64,
                            height,
                            minimum,
                            base_value,
                            min_value,
                            spacing,
                            &meter,
                        )
                        .unwrap();
                        assert_eq!(
                            actual, expected,
                            "initial={initial}, height={height}, minimum={minimum}"
                        );
                    }
                }
            }
        }
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1100)
            .unwrap();
        for initial in [1e20, 1e100, f64::MAX] {
            let meter = OperationWorkMeter::new(policy);
            assert_eq!(
                fit_treemap_label_height(initial, 100.0, 8.0, 28.0, 6.0, 2.0, &meter).unwrap(),
                70.0
            );
            assert!(meter.used() <= 1100);
        }
        let meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
                .unwrap(),
        );
        assert!(matches!(
            fit_treemap_label_height(1e20, 100.0, 8.0, 28.0, 6.0, 2.0, &meter),
            Err(Error::ResourceLimitExceeded(_))
        ));
        let control = merman_model::OperationControl::new();
        control.cancel();
        let meter = OperationWorkMeter::new_with_control(policy, control);
        let Err(Error::Cancelled(error)) =
            fit_treemap_label_height(1e20, 100.0, 8.0, 28.0, 6.0, 2.0, &meter)
        else {
            panic!("height fitting must observe cancellation before searching");
        };
        assert_eq!(error.phase, merman_model::OperationPhase::Emit);
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn treemap_width_search_strictly_progresses_for_extreme_overwide_labels() {
        struct FontSizeWidthMeasurer;

        impl TextMeasurer for FontSizeWidthMeasurer {
            fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: style.font_size,
                    height: style.font_size,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, _text: &str, style: &TextStyle) -> f64 {
                style.font_size
            }
        }

        let measurer = FontSizeWidthMeasurer;
        let layout = TreemapDiagramLayout {
            title_height: 0.0,
            width: 100.0,
            height: 100.0,
            use_max_width: true,
            diagram_padding: 8.0,
            show_values: true,
            value_format: ",".to_string(),
            acc_title: None,
            acc_descr: None,
            title: None,
            sections: Vec::new(),
            leaves: vec![leaf("over-wide", 1.0, 0.0, 100.0, 100.0)],
        };
        let config = presentation_config(serde_json::json!({}));
        let typography_theme = TreemapStyle::resolve(
            &config,
            &layout,
            None,
            None,
            &FALLBACK_C_SCALE,
            &Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            )),
        )
        .unwrap();
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 4_096)
            .unwrap();

        for initial_font_size in [1e20, f64::MAX] {
            let initial_style = typography_theme.leaf_text_style(0, initial_font_size);
            let mut style = initial_style.style().clone();
            let meter = OperationWorkMeter::new(policy);
            let font_size = fit_treemap_label_width(
                initial_font_size,
                16.0,
                8.0,
                "over-wide",
                &mut style,
                &measurer,
                &meter,
            )
            .unwrap();

            assert_eq!(font_size, 16.0);
            assert_eq!(style.font_size, 16.0);
            assert!(meter.used() <= 4_096);
        }
    }

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<()> {
            if self.rejected {
                Err(Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn treemap_leaf_group_stops_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterFirstWrite::default();

        let error = write_treemap_leaf_group_open(&mut out, "treemapNode leaf0x", 12.0, 24.0)
            .expect_err("the rejecting sink must stop Treemap leaf-group emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Treemap leaf-group emission must stop at the first failed sink checkpoint"
        );
    }

    fn leaf(name: impl Into<String>, value: f64, x0: f64, x1: f64, y1: f64) -> TreemapLeafLayout {
        TreemapLeafLayout {
            name: name.into(),
            value,
            parent_name: None,
            x0,
            y0: 0.0,
            x1,
            y1,
            class_selector: None,
            css_compiled_styles: None,
        }
    }

    fn leaf_group(svg: &str, index: usize) -> &str {
        let class = format!(r#"class="treemapNode treemapLeafGroup leaf{index}x""#);
        let class_start = svg.find(&class).expect("leaf group class");
        let start = svg[..class_start].rfind("<g").expect("leaf group start");
        let end = start + svg[start..].find("</g>").expect("leaf group end") + 4;
        &svg[start..end]
    }

    fn opening_tag_by_class<'a>(fragment: &'a str, class_name: &str) -> &'a str {
        let needle = format!(r#"<text class="{class_name}""#);
        let start = fragment.find(&needle).expect("text tag by class");
        let end = start + fragment[start..].find('>').expect("text tag end") + 1;
        &fragment[start..end]
    }

    fn attr_f64(tag: &str, name: &str) -> f64 {
        let prefix = format!(r#"{name}=""#);
        let start = tag.find(&prefix).expect("attribute") + prefix.len();
        let end = start + tag[start..].find('"').expect("attribute end");
        tag[start..end].parse().expect("numeric attribute")
    }

    fn font_size_px(tag: &str) -> f64 {
        let (_, suffix) = tag.split_once("font-size:").expect("font-size style");
        let value = suffix.trim_start();
        let end = value.find("px").expect("font-size px suffix");
        value[..end].trim().parse().expect("font-size number")
    }

    #[test]
    fn treemap_complex_leaf_text_and_section_clipping_match_mermaid_11_16() {
        let mut leaves = vec![
            leaf("Wide", 100.0, 0.0, 200.0, 100.0),
            leaf("A label much wider than its cell", 1.0, 210.0, 222.0, 40.0),
            leaf("Tiny", 1.0, 230.0, 236.0, 40.0),
        ];
        for index in 3..21 {
            leaves.push(leaf(format!("Leaf {index}"), 1.0, 0.0, 100.0, 60.0));
        }
        let layout = TreemapDiagramLayout {
            title_height: 0.0,
            width: 500.0,
            height: 200.0,
            use_max_width: true,
            diagram_padding: 8.0,
            show_values: true,
            value_format: ",".to_string(),
            acc_title: None,
            acc_descr: None,
            title: None,
            sections: vec![TreemapSectionLayout {
                name: "Section label wider than its header".to_string(),
                depth: 1,
                value: 102.0,
                x0: 0.0,
                y0: 0.0,
                x1: 40.0,
                y1: 100.0,
                class_selector: None,
                css_compiled_styles: None,
            }],
            leaves,
        };

        let work = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::interactive()));
        let measurement = TextMeasurementSession::new(TextMeasurementPolicy::deterministic());
        let bbox = measurement.routed(
            &work,
            TextMeasurementPhase::SvgBBox,
            Some(OperationPhase::Emit),
        );
        let computed_length = measurement.routed(
            &work,
            TextMeasurementPhase::ComputedLength,
            Some(OperationPhase::Emit),
        );
        let projection = SvgDiagramIdProjection::new(&work);
        let diagram_id = SvgId::new("treemap").unwrap().with_projection(&projection);
        let root = RootViewportContext::new(DiagramFamilyId::TREEMAP, diagram_id);
        let config = presentation_config(serde_json::json!({}));
        let style =
            TreemapStyle::resolve(&config, &layout, None, None, &FALLBACK_C_SCALE, &work).unwrap();
        let svg = render_treemap_diagram_svg(
            &layout,
            &style,
            diagram_id,
            &root,
            &work,
            &bbox,
            &computed_length,
        )
        .unwrap();
        projection.finish().unwrap();

        let section_label = opening_tag_by_class(&svg, "treemapSectionLabel");
        assert!(
            section_label.contains(r#"clip-path="url(#clip-section-treemap-0)""#),
            "section label must use its emitted clipping path: {section_label}"
        );

        let wide = leaf_group(&svg, 0);
        let wide_label = opening_tag_by_class(wide, "treemapLabel");
        let wide_value = opening_tag_by_class(wide, "treemapValue");
        assert!(!wide_label.contains("display: none"), "{wide_label}");
        assert!(!wide_value.contains("display: none"), "{wide_value}");
        assert!(font_size_px(wide_label) > font_size_px(wide_value));
        assert!(attr_f64(wide_value, "y") > attr_f64(wide_label, "y"));

        let narrow = leaf_group(&svg, 1);
        let narrow_label = opening_tag_by_class(narrow, "treemapLabel");
        let narrow_value = opening_tag_by_class(narrow, "treemapValue");
        assert!(
            narrow.contains(">A label much wider than its cell</text>"),
            "{narrow}"
        );
        assert!(font_size_px(narrow_label) <= font_size_px(wide_label));
        assert!(font_size_px(narrow_label) > 0.0);
        assert!(!narrow_label.contains("display: none"), "{narrow_label}");
        assert!(
            narrow_label.contains(r#"clip-path="url(#clip-treemap-1)""#),
            "narrow complex labels remain present and rely on clipping: {narrow_label}"
        );
        assert!(font_size_px(narrow_value) <= font_size_px(narrow_label));
        assert!(attr_f64(narrow_value, "y") > attr_f64(narrow_label, "y"));
        assert!(!narrow_value.contains("display: none"), "{narrow_value}");

        let tiny = leaf_group(&svg, 2);
        let tiny_label = opening_tag_by_class(tiny, "treemapLabel");
        let tiny_value = opening_tag_by_class(tiny, "treemapValue");
        assert!(tiny_label.contains("display: none"), "{tiny_label}");
        assert!(tiny_value.contains("display: none"), "{tiny_value}");
    }
}
