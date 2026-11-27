#![forbid(unsafe_code)]

// NOTE: This fallback module intentionally keeps parsing "cheap" and non-validating.
// It is a best-effort readability fallback for SVG consumers that do not fully
// support HTML inside `<foreignObject>` (e.g. many rasterizers).

mod attr;
mod cascade;
mod context;
mod css;
mod html;
mod transform;

use crate::svg::foreign_objects::ForeignObjectSwitches;
use crate::svg::pipeline::{
    SvgPostprocessExecution, SvgStructureMetrics, SvgTagScanner, checkpoint_loop, end_tag_name,
    escape_xml_attr_with_checkpoints, escape_xml_text_with_checkpoints,
    extract_exact_double_quoted_attr_with_checkpoints, find_with_checkpoints, start_tag_name,
};
use crate::text::TextMeasurer;
use std::collections::HashSet;
use std::convert::Infallible;
use std::fmt::{self, Write};

use attr::is_self_closing;
use cascade::{CascadeIndex, Namespace, SourceElement};
use context::{SourceFrame, attribute, source_class_attr_tokens};
pub(crate) use html::measure_styled_html_label;
use html::{
    foreign_object_html_wrapping, normalized_foreign_object_text, standalone_node_label_box,
};

/// A writer's glyph-only HTML filter request; the matching paragraph is checked before projection.
pub(crate) const FALLBACK_TEXT_FILTER_DATA_ATTR: &str = "data-merman-fallback-text-filter";

/// Writer-owned terminal fill for a single fallback background occurrence.
pub(crate) const FALLBACK_BACKGROUND_FILL_DATA_ATTR: &str = "data-merman-fallback-background-fill";
/// Stable occurrence identity paired with [`FALLBACK_BACKGROUND_FILL_DATA_ATTR`].
pub(crate) const FALLBACK_OCCURRENCE_DATA_ATTR: &str = "data-merman-fallback-occurrence";
/// Advisory compatibility evidence; this never grants native or portable admission.
pub(crate) const FALLBACK_CSS_RESIDUAL_DATA_ATTR: &str = "data-merman-fallback-css-residual";

/// Adds a best-effort `<text>/<tspan>` overlay extracted from Mermaid label `<foreignObject>`
/// content.
///
/// Many headless SVG renderers and rasterizers do not fully support HTML inside `<foreignObject>`.
/// The returned SVG aims to be *more readable* for raster outputs and UI previews.
///
/// Important:
/// - This does not aim for Mermaid DOM parity.
/// - For parity-focused SVG output, keep the original SVG unchanged.
pub fn foreign_object_label_fallback_svg_text(
    svg: &str,
    text_measurer: &dyn TextMeasurer,
) -> String {
    let mut checkpoint = || Ok::<(), Infallible>(());
    match foreign_object_label_fallback_svg_text_with_checkpoints(
        svg,
        text_measurer,
        &mut checkpoint,
        &mut |_, _| Ok::<(), Infallible>(()),
        &mut |_, _| Ok::<(), Infallible>(()),
        &mut |_| Ok::<(), Infallible>(()),
    ) {
        Ok(output) => output,
        Err(error) => match error {},
    }
}

/// Controlled variant used by the SVG postprocess pipeline.
///
/// The public helper above remains infallible for compatibility. Pipeline callers provide the
/// operation-owned Postprocess checkpoint so a host measurement that declines, fails, or returns
/// an invalid value cannot enter its fallback backend after cancellation.
pub(crate) fn foreign_object_label_fallback_svg_text_controlled(
    svg: &str,
    text_measurer: &dyn TextMeasurer,
    execution: SvgPostprocessExecution<'_>,
    structure: SvgStructureMetrics,
) -> crate::Result<String> {
    foreign_object_label_fallback_svg_text_with_checkpoints(
        svg,
        text_measurer,
        &mut || execution.checkpoint(),
        &mut |actual, maximum| Err(execution.selector_index_limit(actual, maximum)),
        &mut |projected_bytes, generated_elements| {
            execution.preflight_svg_byte_count(projected_bytes)?;
            execution
                .preflight_svg_element_count(structure.elements.saturating_add(generated_elements))
        },
        &mut |message| {
            Err(crate::Error::SvgPostprocess {
                pass: "foreign-object-fallback".to_string(),
                message: message.to_string(),
            })
        },
    )
}

fn parse_attr_f64<E>(
    tag: &str,
    name: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<f64>, E> {
    let Some(value) = attribute(tag, name, checkpoint)? else {
        return Ok(None);
    };
    checkpoint()?;
    let value = value.parse::<f64>().ok();
    checkpoint()?;
    Ok(value)
}

fn foreign_object_label_fallback_svg_text_with_checkpoints<E>(
    svg: &str,
    text_measurer: &dyn TextMeasurer,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
    selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    preflight_generated: &mut impl FnMut(usize, usize) -> Result<(), E>,
    invalid_geometry: &mut impl FnMut(&str) -> Result<(), E>,
) -> Result<String, E> {
    checkpoint()?;
    if find_with_checkpoints(svg, "<foreignObject", checkpoint)?.is_none() {
        return Ok(svg.to_string());
    }

    preflight_generated(svg.len(), 0)?;
    let mut out = String::with_capacity(svg.len());
    let mut overlays = String::new();
    let (mut identifiers, namespace) = collect_svg_ids(svg, checkpoint)?;
    let mut fallback_index = 0usize;
    let mut instance_bytes = 0usize;
    let mut generated_elements = 0usize;
    let mut geometry_stack: Vec<SourceFrame> = Vec::new();
    let mut source_stack: Vec<SourceElement> = Vec::new();
    let mut source_stack_overflow_depth = 0usize;
    let mut cascade_index = None;
    let ancestry_limit = cascade::MAX_SELECTOR_ANCESTRY_DEPTH;
    let switches = ForeignObjectSwitches::scan(svg, checkpoint, |depth| {
        if depth > ancestry_limit {
            selector_limit(depth, ancestry_limit)?;
        }
        Ok(())
    })?;
    let mut scanner = SvgTagScanner::new(svg);
    let mut root_name = None;
    let mut root_close = None;
    let mut i = 0usize;
    let mut iteration = 0usize;
    while let Some(scanned) = scanner.next_with_checkpoints(checkpoint)? {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        let lt = scanned.start();
        let gt = scanner.cursor();
        let tag = scanned.raw();
        out.push_str(&svg[i..lt]);

        if let Some(name) = end_tag_name(tag) {
            if root_name == Some(name) && geometry_stack.len() == 1 {
                root_close = Some(out.len());
            }
            geometry_stack.pop();
            if source_stack_overflow_depth > 0 {
                source_stack_overflow_depth -= 1;
            } else if source_stack
                .last()
                .is_some_and(|element| element.local_name.eq_ignore_ascii_case(name))
            {
                source_stack.pop();
            }
            out.push_str(tag);
            i = gt;
            continue;
        }

        let start_name = start_tag_name(tag);
        root_name = root_name.or(start_name);
        if start_name == Some("foreignObject") {
            let Some(close) = scanner.matching_end_with_checkpoints(scanned, checkpoint)? else {
                i = lt;
                break;
            };
            let i_next = scanner.cursor();
            let inner = if scanned.is_self_closing() {
                ""
            } else {
                &svg[gt..close.start()]
            };
            out.push_str(&svg[lt..i_next]);
            let from_switch = switches.native_objects.contains(&lt);

            let width = parse_attr_f64(tag, "width", checkpoint)?.unwrap_or(0.0);
            let height = parse_attr_f64(tag, "height", checkpoint)?.unwrap_or(0.0);
            if !width.is_finite() || !height.is_finite() {
                invalid_geometry("foreignObject dimensions must be finite")?;
                i = i_next;
                continue;
            }
            if width > 0.0 && height > 0.0 {
                let x = parse_attr_f64(tag, "x", checkpoint)?.unwrap_or(0.0);
                let y = parse_attr_f64(tag, "y", checkpoint)?.unwrap_or(0.0);
                if !x.is_finite() || !y.is_finite() {
                    invalid_geometry("foreignObject coordinates must be finite")?;
                    i = i_next;
                    continue;
                }
                let label_box = standalone_node_label_box(inner, checkpoint)?;
                let block_width = label_box.and_then(|label_box| label_box.width);
                let text_width = block_width.unwrap_or(width);
                let (anchor, text_x) = match attribute(tag, "text-anchor", checkpoint)?.as_deref() {
                    Some("start") => ("start", x),
                    Some("end") => ("end", x + text_width),
                    _ => ("middle", x + text_width / 2.0),
                };

                checkpoint()?;
                let cascade = match cascade_index.as_mut() {
                    Some(cascade) => cascade,
                    None => {
                        let cascade = CascadeIndex::new(svg, checkpoint, selector_limit)?;
                        cascade_index = Some(cascade);
                        cascade_index
                            .as_mut()
                            .expect("cascade index was initialized")
                    }
                };
                let resolved = cascade.resolve_foreign_object(
                    &source_stack,
                    tag,
                    inner,
                    checkpoint,
                    selector_limit,
                )?;
                let css_transform = resolved
                    .paint
                    .iter()
                    .find(|(property, _)| property == "transform")
                    .map(|(_, value)| value);
                let effective_tag = if let Some(css_transform) =
                    css_transform.filter(|value| !value.contains("px") && !value.contains('%'))
                {
                    let escaped = escape_xml_attr_with_checkpoints(css_transform, checkpoint)?;
                    format!(r#"<g transform="{escaped}"/>"#)
                } else {
                    tag.to_string()
                };
                let matrix = match geometry_stack.last() {
                    Some(frame) => frame.foreign_object_transform(&effective_tag, checkpoint)?,
                    None => transform::Affine::from_tag(&effective_tag, checkpoint)?,
                };
                let Some(matrix) = matrix else {
                    invalid_geometry(
                        "foreignObject transform or viewport cannot be resolved to a finite SVG matrix",
                    )?;
                    i = i_next;
                    continue;
                };
                if !matrix.maps_finite_rect(x, y, width, height) {
                    invalid_geometry("foreignObject coordinates must be finite")?;
                    i = i_next;
                    continue;
                }
                let normalized = normalized_foreign_object_text(&resolved.fragments, checkpoint)?;
                if !normalized.is_empty() {
                    let typography = resolved.typography;
                    let writer_background = if inner.contains("labelBkg") {
                        writer_owned_fallback_background(tag, checkpoint)?
                    } else {
                        None
                    };
                    let text_filter = writer_owned_fallback_text_filter(tag, inner, checkpoint)?;
                    let occurrence_attr = writer_background
                        .map(|(occurrence, _)| {
                            escape_xml_attr_with_checkpoints(occurrence, checkpoint).map(|value| {
                                format!(r#" {FALLBACK_OCCURRENCE_DATA_ATTR}="{value}""#)
                            })
                        })
                        .transpose()?
                        .unwrap_or_default();
                    let source_attr = if from_switch {
                        r#" data-merman-foreignobject-source="switch-native-fallback""#
                    } else {
                        ""
                    };
                    let source_classes =
                        source_class_attr_tokens(&geometry_stack, tag, inner, checkpoint)?;
                    let source_classes_attr = source_classes
                        .as_deref()
                        .map(|classes| format!(r#" data-merman-source-classes="{classes}""#))
                        .unwrap_or_default();
                    let measure_style = typography.text_style();
                    let wrapping = match block_width {
                        Some(width) => (width > 0.0).then_some(width),
                        None => foreign_object_html_wrapping(tag, inner, checkpoint)?,
                    };
                    let lines =
                        normalized.wrap(wrapping, text_measurer, &measure_style, checkpoint)?;
                    let line_heights = lines
                        .iter()
                        .map(|line| normalized.line_box_height(line, typography.line_height))
                        .collect::<Vec<_>>();
                    let mut top = if label_box.is_some_and(|label_box| label_box.height.is_some()) {
                        y
                    } else {
                        y + (height - line_heights.iter().sum::<f64>()) / 2.0
                    };
                    let mut positions = Vec::with_capacity(lines.len());
                    let mut valid = true;
                    for (idx, line) in lines.iter().enumerate() {
                        checkpoint_loop(idx, checkpoint)?;
                        let anchor_factor = match anchor {
                            "middle" => 0.5,
                            "end" => 1.0,
                            _ => 0.0,
                        };
                        // HTML retains the trailing letter spacing in the line width. Native
                        // SVG text anchors against the advance without that final spacing.
                        let trailing_spacing = normalized.trailing_letter_spacing(line);
                        let line_x = text_x
                            + (line.width - line.alignment_width - trailing_spacing)
                                * anchor_factor;
                        let line_height = line_heights[idx];
                        let y_line = top + line_height / 2.0;
                        top += line_height;
                        checkpoint()?;
                        // Emitted rows are decoded SVG text, not a new HTML label.
                        let (bbox_width, bbox_height) = if normalized
                            .has_inline_styles(line, &typography)
                        {
                            let mut width = 0.0;
                            let mut height = 0.0_f64;
                            for run in normalized.line_runs(line) {
                                checkpoint()?;
                                let style = run.typography.text_style();
                                width += text_measurer
                                    .measure_svg_raw_text_bbox_width_px(run.text, &style);
                                height = height.max(
                                    text_measurer
                                        .measure_svg_raw_text_bbox_height_px(run.text, &style),
                                );
                            }
                            (width, height)
                        } else {
                            (
                                text_measurer
                                    .measure_svg_raw_text_bbox_width_px(&line.text, &measure_style),
                                text_measurer.measure_svg_raw_text_bbox_height_px(
                                    &line.text,
                                    &measure_style,
                                ),
                            )
                        };
                        checkpoint()?;
                        let measured_width = bbox_width.max(line.width);
                        if !line_x.is_finite()
                            || !y_line.is_finite()
                            || !bbox_width.is_finite()
                            || !bbox_height.is_finite()
                            || bbox_width < 0.0
                            || bbox_height < 0.0
                            || !matrix.maps_finite_rect(
                                line_x - measured_width * anchor_factor,
                                y_line - bbox_height / 2.0,
                                measured_width,
                                bbox_height,
                            )
                        {
                            invalid_geometry(
                                "foreignObject fallback text geometry must be finite",
                            )?;
                            valid = false;
                            break;
                        }
                        positions.push((line_x, y_line));
                    }
                    if !valid {
                        i = i_next;
                        continue;
                    }
                    // Keep the actual source ancestors for opacity, masks, clipping and viewports.
                    // Explicit styles isolate generated shapes from node/cluster CSS selectors.
                    let local = transform::Affine::from_tag(tag, checkpoint)?.unwrap_or_default();
                    let transform_attr = if local == transform::Affine::default() {
                        String::new()
                    } else {
                        format!(
                            r#" transform="matrix({} {} {} {} {} {})""#,
                            local.a, local.b, local.c, local.d, local.e, local.f
                        )
                    };
                    let mut paint_style = "opacity: 1; display: inline; visibility: inherit; clip-path: none; mask: none; filter: none;".to_string();
                    for (property, value) in resolved.paint.iter() {
                        if property != "overflow" {
                            write!(paint_style, " {property}: {value};")
                                .expect("writing into a string");
                        }
                    }
                    if !resolved
                        .paint
                        .iter()
                        .any(|(property, _)| property == "transform")
                    {
                        write!(
                            paint_style,
                            " transform: matrix({} {} {} {} {} {});",
                            local.a, local.b, local.c, local.d, local.e, local.f
                        )
                        .expect("writing into a string");
                    }
                    let escaped_paint = escape_xml_attr_with_checkpoints(&paint_style, checkpoint)?;
                    let mut metadata = String::new();
                    for name in [
                        "role",
                        "aria-label",
                        "aria-labelledby",
                        "aria-describedby",
                        "aria-hidden",
                    ] {
                        if let Some(value) = attribute(tag, name, checkpoint)? {
                            let value =
                                escape_xml_attr_with_checkpoints(value.as_ref(), checkpoint)?;
                            write!(metadata, r#" {name}="{value}""#)
                                .expect("writing into a string");
                        }
                    }
                    let overlay_start = overlays.len();
                    let identifier = loop {
                        let candidate = format!(
                            "{}-foreignobject-label-{fallback_index}",
                            namespace.as_deref().unwrap_or("merman")
                        );
                        fallback_index = fallback_index.saturating_add(1);
                        if identifiers.insert(candidate.clone()) {
                            break candidate;
                        }
                        checkpoint()?;
                    };
                    push_generated_fmt(
                        &mut overlays,
                        format_args!(
                            r#"<g data-merman-foreignobject="fallback"{source_attr} class="merman-foreignobject-fallback"{source_classes_attr}{transform_attr}{metadata}{occurrence_attr} style="{escaped_paint}">"#
                        ),
                        1,
                        svg.len().saturating_add(instance_bytes),
                        &mut generated_elements,
                        preflight_generated,
                    )?;
                    let inner_start = overlays.len();
                    let inline_projection = text_filter.is_some() || writer_background.is_some();
                    let viewport_x = if inline_projection { x } else { 0.0 };
                    let viewport_y = if inline_projection { y } else { 0.0 };
                    let overflow = resolved
                        .paint
                        .iter()
                        .find(|(property, _)| property == "overflow")
                        .map_or("hidden", |(_, value)| value.as_ref());
                    let escaped_overflow = escape_xml_attr_with_checkpoints(overflow, checkpoint)?;
                    let escaped_font_family =
                        escape_xml_attr_with_checkpoints(&typography.font_family, checkpoint)?;
                    push_generated_fmt(
                        &mut overlays,
                        format_args!(
                            r#"<svg xmlns="http://www.w3.org/2000/svg" x="{viewport_x}" y="{viewport_y}" width="{width}" height="{height}" viewBox="0 0 {width} {height}" preserveAspectRatio="none" style="overflow: {escaped_overflow}; opacity: 1; display: inline; visibility: inherit; clip-path: none; mask: none; filter: none; transform: none; font-family: {escaped_font_family}; font-size: {}px;">"#,
                            typography.font_size,
                        ),
                        1,
                        svg.len().saturating_add(instance_bytes),
                        &mut generated_elements,
                        preflight_generated,
                    )?;
                    if !inline_projection {
                        // Preserve the source viewport's minimum geometric extent. Native glyph
                        // bboxes can still enlarge objectBoundingBox paints (a browser text residual).
                        push_generated_fmt(
                            &mut overlays,
                            format_args!(
                                r#"<rect x="0" y="0" width="{width}" height="{height}" fill="none" style="fill: none; stroke: none; opacity: 0; display: inline; transform: none;"/>"#
                            ),
                            1,
                            svg.len().saturating_add(instance_bytes),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                    }
                    if let Some((occurrence, fill)) = writer_background {
                        let occurrence = escape_xml_attr_with_checkpoints(occurrence, checkpoint)?;
                        let fill = escape_xml_attr_with_checkpoints(fill, checkpoint)?;
                        push_generated_fmt(
                            &mut overlays,
                            format_args!(
                                r#"<rect x="0" y="0" width="{width}" height="{height}" {FALLBACK_OCCURRENCE_DATA_ATTR}="{occurrence}" fill="{fill}" style="fill:{fill} !important; fill-opacity:1; stroke:none; opacity:1; display:inline; visibility:inherit; clip-path:none; mask:none; filter:none; transform:none;"/>"#
                            ),
                            1,
                            svg.len().saturating_add(instance_bytes),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                    } else if let Some(label_bkg) = &typography.label_background {
                        let escaped_label_bkg =
                            escape_xml_attr_with_checkpoints(label_bkg, checkpoint)?;
                        push_generated_fmt(
                            &mut overlays,
                            format_args!(
                                r#"<rect x="0" y="0" width="{width}" height="{height}" fill="{escaped_label_bkg}" style="fill:{escaped_label_bkg}; fill-opacity:1; stroke:none; opacity:1; display:inline; visibility:inherit; clip-path:none; mask:none; filter:none; transform:none;"/>"#
                            ),
                            1,
                            svg.len().saturating_add(instance_bytes),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                    }
                    let text_filter_attr =
                        text_filter.map(|reference| format!(r#" filter="{reference}""#));
                    if let Some(filter_attr) = &text_filter_attr {
                        push_generated_fmt(
                            &mut overlays,
                            format_args!("<g{filter_attr}>"),
                            1,
                            svg.len().saturating_add(instance_bytes),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                    }
                    let mut text_style = format!(
                        "text-anchor: {anchor}; font-size: {}px; font-family: {}; line-height: {}px; fill: {}; fill-opacity: 1; stroke: none; opacity: 1; display: inline; visibility: inherit; clip-path: none; mask: none; filter: none; transform: none; white-space: pre; dominant-baseline: central; alignment-baseline: central;",
                        typography.font_size,
                        typography.font_family,
                        typography.line_height,
                        typography.fill,
                    );
                    write!(
                        text_style,
                        " font-weight: {}; font-style: {};",
                        typography.font_weight.as_deref().unwrap_or("normal"),
                        typography.font_style.as_deref().unwrap_or("normal"),
                    )
                    .expect("writing into a string");
                    let spacing = typography.text_geometry.spacing;
                    if !spacing.is_normal() {
                        write!(
                            text_style,
                            " letter-spacing: {}px; word-spacing: {}px;",
                            spacing.letters, spacing.words,
                        )
                        .expect("writing into a string");
                    }
                    let escaped_fill =
                        escape_xml_attr_with_checkpoints(&typography.fill, checkpoint)?;
                    let escaped_style = escape_xml_attr_with_checkpoints(&text_style, checkpoint)?;
                    let text_attributes = format!(
                        r#" class="merman-foreignobject-fallback-text"{source_classes_attr}"#
                    );
                    for (idx, (line, (line_x, y_line))) in lines.iter().zip(positions).enumerate() {
                        checkpoint_loop(idx, checkpoint)?;
                        let line_x = line_x - x;
                        let y_line = y_line - y;
                        if normalized.has_inline_styles(line, &typography) {
                            push_generated_fmt(
                                &mut overlays,
                                format_args!(
                                    r##"<text x="{line_x}" y="{y_line}" text-anchor="{anchor}" dominant-baseline="central" alignment-baseline="central" fill="{escaped_fill}"{text_attributes} xml:space="preserve" style="{escaped_style}">"##
                                ),
                                1,
                                svg.len().saturating_add(instance_bytes),
                                &mut generated_elements,
                                preflight_generated,
                            )?;
                            let mut has_positioned_text = false;
                            for (run_index, run) in normalized.line_runs(line).enumerate() {
                                checkpoint_loop(run_index, checkpoint)?;
                                let position =
                                    if run.typography.font_size > 0.0 && !has_positioned_text {
                                        has_positioned_text = true;
                                        if run_index > 0 {
                                            // usvg skips zero-sized spans together with their x/y.
                                            // Preserve the row origin on its first paintable span.
                                            format!(r#" x="{line_x}" y="{y_line}""#)
                                        } else {
                                            String::new()
                                        }
                                    } else {
                                        String::new()
                                    };
                                let text = escape_xml_text_with_checkpoints(run.text, checkpoint)?;
                                let mut style = format!(
                                    "font-family: {} !important; font-size: {}px !important; font-weight: {} !important; font-style: {} !important; fill: {} !important; fill-opacity: 1 !important; stroke: none !important; opacity: 1 !important; display: inline !important; visibility: inherit !important; clip-path: none !important; mask: none !important; filter: none !important; transform: none !important;",
                                    run.typography.font_family,
                                    run.typography.font_size,
                                    run.typography.font_weight.as_deref().unwrap_or("normal"),
                                    run.typography.font_style.as_deref().unwrap_or("normal"),
                                    run.typography.fill,
                                );
                                let run_spacing = run.typography.text_geometry.spacing;
                                if run_spacing != spacing {
                                    write!(
                                        style,
                                        " letter-spacing: {}px !important; word-spacing: {}px !important;",
                                        run_spacing.letters, run_spacing.words,
                                    )
                                    .expect("writing into a string");
                                }
                                let style = escape_xml_attr_with_checkpoints(&style, checkpoint)?;
                                push_generated_fmt(
                                    &mut overlays,
                                    format_args!(
                                        r#"<tspan{position} style="{style}">{text}</tspan>"#
                                    ),
                                    1,
                                    svg.len().saturating_add(instance_bytes),
                                    &mut generated_elements,
                                    preflight_generated,
                                )?;
                            }
                            push_generated_fmt(
                                &mut overlays,
                                format_args!("</text>"),
                                0,
                                svg.len().saturating_add(instance_bytes),
                                &mut generated_elements,
                                preflight_generated,
                            )?;
                            continue;
                        }
                        let text = escape_xml_text_with_checkpoints(&line.text, checkpoint)?;
                        push_generated_fmt(
                            &mut overlays,
                            format_args!(
                                r##"<text x="{line_x}" y="{y_line}" text-anchor="{anchor}" dominant-baseline="central" alignment-baseline="central" fill="{escaped_fill}"{text_attributes} xml:space="preserve" style="{escaped_style}">{text}</text>"##
                            ),
                            1,
                            svg.len().saturating_add(instance_bytes),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                    }
                    if text_filter.is_some() {
                        push_generated_fmt(
                            &mut overlays,
                            format_args!("</g>"),
                            0,
                            svg.len().saturating_add(instance_bytes),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                    }
                    push_generated_fmt(
                        &mut overlays,
                        format_args!("</svg>"),
                        0,
                        svg.len().saturating_add(instance_bytes),
                        &mut generated_elements,
                        preflight_generated,
                    )?;
                    if inline_projection
                        && lines
                            .iter()
                            .any(|line| normalized.has_inline_styles(line, &typography))
                        && cascade.inline_text_css_residual(
                            &source_stack,
                            &overlays[overlay_start..],
                            checkpoint,
                            selector_limit,
                        )?
                    {
                        let marker = format!(r#" {FALLBACK_CSS_RESIDUAL_DATA_ATTR}="true""#);
                        preflight_generated(
                            svg.len()
                                .saturating_add(instance_bytes)
                                .saturating_add(overlays.len())
                                .saturating_add(marker.len()),
                            generated_elements,
                        )?;
                        overlays.insert_str(overlay_start + "<g".len(), &marker);
                    }
                    if inline_projection {
                        // Writer-owned evidence requires direct, inspectable terminal geometry
                        // under the real ancestry. Ordinary labels use isolated definitions.
                        let permitted = source_stack_overflow_depth == 0
                            && (text_filter.is_none()
                                || cascade.permits_html_text_filter(
                                    &source_stack,
                                    tag,
                                    inner,
                                    checkpoint,
                                    selector_limit,
                                )?)
                            && cascade.permits_source_projection(
                                &source_stack,
                                tag,
                                inner,
                                checkpoint,
                                selector_limit,
                            )?
                            && cascade.permits_in_place_fallback(
                                &source_stack,
                                &overlays[overlay_start..],
                                checkpoint,
                                selector_limit,
                            )?;
                        if !permitted
                            && let Some(filter_attr) = &text_filter_attr
                            && let Some(relative) = overlays[overlay_start..].find(filter_attr)
                        {
                            let start = overlay_start + relative;
                            overlays.replace_range(start..start + filter_attr.len(), "");
                        }
                        push_generated_fmt(
                            &mut overlays,
                            format_args!("</g>"),
                            0,
                            svg.len().saturating_add(instance_bytes),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                        let projected_bytes = instance_bytes
                            .saturating_add(overlays.len().saturating_sub(overlay_start));
                        preflight_generated(
                            svg.len()
                                .saturating_add(overlay_start)
                                .saturating_add(projected_bytes),
                            generated_elements,
                        )?;
                        instance_bytes = projected_bytes;
                        out.push_str(&overlays[overlay_start..]);
                        overlays.truncate(overlay_start);
                        i = i_next;
                        continue;
                    }
                    let inner_end = overlays.len();
                    let mut wrapper = overlays[overlay_start..inner_start].to_string();
                    let inner = overlays[inner_start..inner_end].to_string();
                    overlays.truncate(overlay_start);
                    let isolated =
                        format!(r#"<defs><g id="{identifier}"{source_attr}>{inner}</g></defs>"#);
                    if lines
                        .iter()
                        .any(|line| normalized.has_inline_styles(line, &typography))
                        && cascade.inline_text_css_residual(
                            &source_stack[..source_stack.len().min(1)],
                            &isolated,
                            checkpoint,
                            selector_limit,
                        )?
                    {
                        wrapper.insert_str(
                            "<g".len(),
                            &format!(r#" {FALLBACK_CSS_RESIDUAL_DATA_ATTR}="true""#),
                        );
                    }
                    push_generated_fmt(
                        &mut overlays,
                        format_args!("{isolated}"),
                        2,
                        svg.len().saturating_add(instance_bytes),
                        &mut generated_elements,
                        preflight_generated,
                    )?;
                    let instance = format!(
                        r##"{wrapper}<use x="{x}" y="{y}" href="#{identifier}" style="opacity: 1; display: inline; visibility: inherit; clip-path: none; mask: none; filter: none; transform: none;"/></g>"##
                    );
                    let projected_instance_bytes = instance_bytes.saturating_add(instance.len());
                    preflight_generated(
                        svg.len()
                            .saturating_add(overlays.len())
                            .saturating_add(projected_instance_bytes),
                        generated_elements.saturating_add(1),
                    )?;
                    generated_elements = generated_elements.saturating_add(1);
                    instance_bytes = projected_instance_bytes;
                    out.push_str(&instance);
                }
            }
            i = i_next;
            continue;
        }

        // Source styles are consumed by `CascadeIndex` in its first pass.
        if start_name == Some("style") && !scanned.is_self_closing() {
            if scanner
                .matching_end_with_checkpoints(scanned, checkpoint)?
                .is_none()
            {
                i = lt;
                break;
            }
            i = scanner.cursor();
            out.push_str(&svg[lt..i]);
            continue;
        }

        if let Some(name) = start_name
            && !is_self_closing(tag)
        {
            if source_stack_overflow_depth > 0 {
                source_stack_overflow_depth = source_stack_overflow_depth.saturating_add(1);
            } else if source_stack.len() >= ancestry_limit {
                selector_limit(source_stack.len().saturating_add(1), ancestry_limit)?;
                source_stack_overflow_depth = 1;
            } else {
                source_stack.push(CascadeIndex::source_element(
                    tag,
                    Namespace::Svg,
                    checkpoint,
                )?);
            }
            geometry_stack.push(SourceFrame::from_tag(
                tag,
                name,
                geometry_stack.last(),
                source_stack_overflow_depth == 0,
                checkpoint,
            )?);
        }

        out.push_str(tag);
        i = gt;
    }

    if i < svg.len() {
        out.push_str(&svg[i..]);
    }

    if !overlays.is_empty()
        && let Some(close) = root_close
    {
        preflight_generated(out.len().saturating_add(overlays.len()), generated_elements)?;
        out.insert_str(close, &overlays);
    }
    checkpoint()?;
    Ok(out)
}

fn collect_svg_ids<E>(
    svg: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(HashSet<String>, Option<String>), E> {
    let mut result = HashSet::new();
    let mut namespace = None;
    let mut saw_root = false;
    let mut scanner = SvgTagScanner::new(svg);
    while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
        if let Some(name) = start_tag_name(tag.raw()) {
            let id = attribute(tag.raw(), "id", checkpoint)?;
            if !saw_root && name.rsplit(':').next() == Some("svg") {
                saw_root = true;
                // Renderer roots carry document-unique, fragment-safe diagram IDs.
                namespace = id
                    .as_deref()
                    .filter(|id| {
                        !id.is_empty()
                            && id
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
                    })
                    .map(str::to_owned);
            }
            if let Some(id) = id {
                result.insert(id.into_owned());
            }
        }
    }
    Ok((result, namespace))
}

#[derive(Default)]
struct FmtByteCounter {
    bytes: usize,
}

impl Write for FmtByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.saturating_add(value.len());
        Ok(())
    }
}

fn push_generated_fmt<E>(
    output: &mut String,
    arguments: fmt::Arguments<'_>,
    added_elements: usize,
    source_bytes: usize,
    generated_elements: &mut usize,
    preflight_generated: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    let mut counter = FmtByteCounter::default();
    counter
        .write_fmt(arguments)
        .expect("counting formatted SVG bytes cannot fail");
    let projected_bytes = source_bytes
        .saturating_add(output.len())
        .saturating_add(counter.bytes);
    let projected_elements = generated_elements.saturating_add(added_elements);
    preflight_generated(projected_bytes, projected_elements)?;
    output.reserve(counter.bytes);
    output
        .write_fmt(arguments)
        .expect("writing formatted SVG into a String cannot fail");
    *generated_elements = projected_elements;
    Ok(())
}

fn writer_owned_fallback_text_filter<'a, E>(
    tag: &'a str,
    inner: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<&'a str>, E> {
    let Some(reference) = extract_exact_double_quoted_attr_with_checkpoints(
        tag,
        FALLBACK_TEXT_FILTER_DATA_ATTR,
        checkpoint,
    )?
    else {
        return Ok(None);
    };
    let Some(id) = reference
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
    else {
        return Ok(None);
    };
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Ok(None);
    }
    // The annotation requests projection, not admission. Require its real HTML application;
    // the downstream exporter still verifies the actual SVG definition and filtered glyphs.
    checkpoint()?;
    let Ok(document) = crate::xml_dom::Document::parse(inner) else {
        return Ok(None);
    };
    checkpoint()?;
    let div = document.root_element();
    let mut children = div.children().filter(|node| node.is_element());
    let Some(span) = children.next() else {
        return Ok(None);
    };
    if children.next().is_some()
        || !div.has_tag_name("div")
        || div.attribute("class") != Some("labelBkg")
        || !span.has_tag_name("span")
        || span.attribute("class") != Some("edgeLabel")
    {
        return Ok(None);
    }
    let mut children = span.children().filter(|node| node.is_element());
    let Some(paragraph) = children.next() else {
        return Ok(None);
    };
    let expected_style = format!("background:transparent;filter:{reference};");
    if children.next().is_some()
        || !paragraph.has_tag_name("p")
        || paragraph.attributes().len() != 1
        || paragraph.attribute("style") != Some(expected_style.as_str())
    {
        return Ok(None);
    }
    for outer in [div, span] {
        if outer
            .attributes()
            .any(|attribute| !matches!(attribute.name(), "class" | "style"))
        {
            return Ok(None);
        }
        let mut bounded = true;
        crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
            outer.attribute("style").unwrap_or_default(),
            checkpoint,
            |boundary| {
                let raw = boundary.raw();
                if raw.trim().trim_end_matches(';').trim().is_empty() {
                    return Ok(true);
                }
                bounded &=
                    crate::mermaid_style::parse_style_declaration(raw).is_some_and(
                        |decl| match decl.property() {
                            "color" | "fill" | "font-family" | "font-size" | "font-weight"
                            | "font-style" => true,
                            "background" => {
                                crate::mermaid_style::is_supported_css_color_value(decl.value())
                            }
                            "display" => matches!(decl.value(), "table" | "table-cell"),
                            "white-space" => matches!(decl.value(), "nowrap" | "break-spaces"),
                            "line-height" => decl.value() == "1.5",
                            "width" | "max-width" => decl.value() == "200px",
                            "text-align" => decl.value() == "center",
                            _ => false,
                        },
                    );
                Ok(true)
            },
        )?;
        if !bounded {
            return Ok(None);
        }
    }
    for node in document.descendants() {
        checkpoint()?;
        if node.is_element() && node.tag_name().namespace() != Some("http://www.w3.org/1999/xhtml")
        {
            return Ok(None);
        }
        if node == div || node == span || node == paragraph || node.is_root() {
            continue;
        }
        if node.is_element()
            && (!matches!(node.tag_name().name(), "span" | "br") || node.attributes().len() != 0)
        {
            return Ok(None);
        }
        if node.is_text()
            && node.text().is_some_and(|text| !text.trim().is_empty())
            && node
                .parent()
                .is_some_and(|parent| parent == div || parent == span || parent.is_root())
        {
            return Ok(None);
        }
    }
    Ok(Some(reference))
}

fn writer_owned_fallback_background<'a, E>(
    tag: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<(&'a str, &'a str)>, E> {
    let Some(occurrence) = extract_exact_double_quoted_attr_with_checkpoints(
        tag,
        FALLBACK_OCCURRENCE_DATA_ATTR,
        checkpoint,
    )?
    else {
        return Ok(None);
    };
    let Some(fill) = extract_exact_double_quoted_attr_with_checkpoints(
        tag,
        FALLBACK_BACKGROUND_FILL_DATA_ATTR,
        checkpoint,
    )?
    else {
        return Ok(None);
    };
    if occurrence.is_empty() || fill.is_empty() {
        return Ok(None);
    }
    if merman_markup::theme_color::ThemeColor::parse(fill).is_err() {
        return Ok(None);
    }
    Ok(Some((occurrence, fill)))
}

#[cfg(test)]
mod tests;
