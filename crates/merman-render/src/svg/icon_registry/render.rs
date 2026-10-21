use super::ingest::ResolvedIcon;
use super::xml::ValidatedIconBody;
use super::{IconCurrentColorUse, IconRenderRequest, RenderedIcon};
use crate::svg::pipeline::validate_well_formed_svg_with_controls;
use merman_core::OperationPhase;
use merman_core::sanitize::{SanitizeFailure, SanitizeOutputSink};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use std::cell::Cell;

const WORK_BYTES_PER_UNIT: usize = 256;
// XML attribute escaping expands one source scalar by at most `&quot;` (six bytes).
// This also covers spelling out the closing names of admitted self-closing SVG elements.
// Per-element additions cover the longest supported default namespace declaration, an XLink
// binding, and Mermaid's target hook. The outer SVG and optional transform add two elements.
const SANITIZER_MAX_BYTE_EXPANSION: usize = "&quot;".len();
const SANITIZER_MAX_ADDED_BYTES_PER_ELEMENT: usize =
    r#" xmlns="http://www.w3.org/1998/Math/MathML""#.len()
        + r#" xmlns:xlink="http://www.w3.org/1999/xlink""#.len()
        + r#" rel="noopener""#.len();

#[derive(Debug, Clone, PartialEq, Eq)]
enum BoundedSanitizeError {
    OutputLimitExceeded { max_bytes: usize },
    ParseInputLimitExceeded { max_bytes: usize },
    AllocationFailed,
    Control(crate::resources::OperationWorkError),
}

#[derive(Debug)]
struct BoundedSanitizeSink<'a> {
    max_output_bytes: usize,
    max_parse_input_bytes: usize,
    parse_work: Cell<usize>,
    work_meter: &'a crate::resources::OperationWorkMeter,
}

impl<'a> BoundedSanitizeSink<'a> {
    const fn new(
        max_output_bytes: usize,
        max_parse_input_bytes: usize,
        work_meter: &'a crate::resources::OperationWorkMeter,
    ) -> Self {
        Self {
            max_output_bytes,
            max_parse_input_bytes,
            parse_work: Cell::new(0),
            work_meter,
        }
    }

    const fn output_limit_error(&self) -> BoundedSanitizeError {
        BoundedSanitizeError::OutputLimitExceeded {
            max_bytes: self.max_output_bytes,
        }
    }
}

impl SanitizeOutputSink for BoundedSanitizeSink<'_> {
    type Error = BoundedSanitizeError;

    fn checkpoint(&self) -> Result<(), Self::Error> {
        self.work_meter
            .checkpoint(OperationPhase::Emit)
            .map_err(BoundedSanitizeError::Control)
    }

    fn charge_parse_work(&self, additional: usize) -> Result<(), Self::Error> {
        self.work_meter
            .charge_emit_work(additional)
            .map_err(BoundedSanitizeError::Control)
    }

    fn check_parse_storage(
        &self,
        input_bytes: usize,
        nodes: usize,
        retained_bytes: usize,
    ) -> Result<(), Self::Error> {
        self.checkpoint()?;
        if input_bytes > self.max_parse_input_bytes {
            return Err(BoundedSanitizeError::ParseInputLimitExceeded {
                max_bytes: self.max_parse_input_bytes,
            });
        }
        if nodes == 0 && retained_bytes == 0 {
            self.parse_work.set(0);
        }
        // Charge tokenizer input, tree construction/policy visits, and retained markup before
        // each parser allocation. A second Mermaid sanitization stage starts a new accounting
        // epoch; it still contributes to the same operation ledger.
        let work = nodes
            .checked_mul(2)
            .and_then(|work| work.checked_add(ceil_div(input_bytes, WORK_BYTES_PER_UNIT)))
            .and_then(|work| work.checked_add(ceil_div(retained_bytes, WORK_BYTES_PER_UNIT)))
            .ok_or_else(|| BoundedSanitizeError::Control(self.work_meter.arithmetic_overflow()))?;
        let additional = work.saturating_sub(self.parse_work.get());
        self.work_meter
            .charge_emit_work(additional)
            .map_err(BoundedSanitizeError::Control)?;
        self.parse_work.set(work);
        Ok(())
    }

    fn checked_output_len(&self, current: usize, additional: usize) -> Result<usize, Self::Error> {
        let next = current
            .checked_add(additional)
            .ok_or_else(|| self.output_limit_error())?;
        if next > self.max_output_bytes {
            return Err(self.output_limit_error());
        }
        Ok(next)
    }

    fn string_with_capacity(&self, capacity: usize) -> Result<String, Self::Error> {
        self.checked_output_len(0, capacity)?;
        let mut output = String::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| BoundedSanitizeError::AllocationFailed)?;
        Ok(output)
    }

    fn output_buffer(&self, input_len: usize) -> Result<Vec<u8>, Self::Error> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(input_len.min(self.max_output_bytes))
            .map_err(|_| BoundedSanitizeError::AllocationFailed)?;
        Ok(output)
    }

    fn push_output_chunk(&self, output: &mut Vec<u8>, chunk: &[u8]) -> Result<(), Self::Error> {
        self.checked_output_len(output.len(), chunk.len())?;
        output
            .try_reserve(chunk.len())
            .map_err(|_| BoundedSanitizeError::AllocationFailed)?;
        output.extend_from_slice(chunk);
        Ok(())
    }
}

pub(super) fn render_resolved_icon(
    icon: &ResolvedIcon,
    request: &IconRenderRequest<'_>,
) -> crate::Result<String> {
    render_resolved_icon_inner(icon, request, false).map(RenderedIcon::into_svg)
}

pub(super) fn render_resolved_icon_with_paint_fact(
    icon: &ResolvedIcon,
    request: &IconRenderRequest<'_>,
) -> crate::Result<RenderedIcon> {
    render_resolved_icon_inner(icon, request, true)
}

fn render_resolved_icon_inner(
    icon: &ResolvedIcon,
    request: &IconRenderRequest<'_>,
    observe_paint_fact: bool,
) -> crate::Result<RenderedIcon> {
    let width = render_dimension(request.width_px, "width")?;
    let height = render_dimension(request.height_px, "height")?;
    let transformed = transformed_icon(icon)?;
    let class_attr = request
        .extra_class
        .map(|class| format!(r#" class="{}""#, escape_xml_attr(class)))
        .unwrap_or_default();
    let xlink_attr = if icon.body.uses_xlink() {
        r#" xmlns:xlink="http://www.w3.org/1999/xlink""#
    } else {
        ""
    };
    let open = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"{xlink_attr}{class_attr} width="{width}" height="{height}" viewBox="{} {} {} {}">"#,
        js_number(transformed.left),
        js_number(transformed.top),
        js_number(transformed.width),
        js_number(transformed.height),
    );
    let projected_body_bytes = transformed.projected_body_bytes(&icon.body)?;
    let projected_svg_bytes = open
        .len()
        .checked_add(projected_body_bytes)
        .and_then(|bytes| bytes.checked_add("</svg>".len()))
        .ok_or_else(|| crate::Error::icon_processing("projected icon SVG size overflowed"))?;
    let sanitizer_ceiling =
        sanitizer_output_ceiling(projected_svg_bytes, icon.body.element_count())?;
    let work_units = icon_render_work_units(
        icon,
        projected_svg_bytes,
        sanitizer_ceiling,
        observe_paint_fact,
    )?;

    // Both ledgers are charged before cloning the retained body or allocating the assembled SVG.
    request.work_meter.charge_emit_work(work_units)?;
    request.work_meter.charge_svg_bytes(projected_svg_bytes)?;

    let body = transformed.apply(&icon.body, request.id_scope)?;
    let mut raw_svg = String::with_capacity(projected_svg_bytes);
    raw_svg.push_str(&open);
    raw_svg.push_str(&body);
    raw_svg.push_str("</svg>");
    if raw_svg.len() > projected_svg_bytes {
        return Err(crate::Error::icon_processing(
            "assembled icon SVG exceeded its precharged size",
        ));
    }

    // Mermaid performs sanitization after transforms, deterministic ID replacement, and outer SVG
    // assembly. The effective request config must therefore be applied on every render.
    let requested_growth = sanitizer_ceiling - projected_svg_bytes;
    let growth_reservation = request
        .work_meter
        .reserve_svg_bytes_up_to(requested_growth)?;
    let reserved_svg_bytes = projected_svg_bytes
        .checked_add(growth_reservation.additional_bytes)
        .ok_or_else(|| crate::Error::icon_processing("sanitizer reservation overflowed"))?;
    let sanitize_sink =
        BoundedSanitizeSink::new(reserved_svg_bytes, sanitizer_ceiling, request.work_meter);
    let sanitized = match merman_core::sanitize::sanitize_xhtml_with_sink(
        &raw_svg,
        request.effective_config.as_config(),
        &sanitize_sink,
    ) {
        Ok(sanitized) => sanitized,
        Err(SanitizeFailure::Output(BoundedSanitizeError::Control(error))) => {
            return Err(error.into());
        }
        Err(SanitizeFailure::Output(BoundedSanitizeError::OutputLimitExceeded { .. })) => {
            request
                .work_meter
                .reconcile_svg_bytes(reserved_svg_bytes, 0)?;
            if let Some(error) = growth_reservation.limit_error {
                return Err(request
                    .work_meter
                    .terminate_absolute_resource_error(error, OperationPhase::Emit)
                    .into());
            }
            return Err(crate::Error::icon_processing(
                "icon SVG sanitizer exceeded its fixed expansion ceiling",
            ));
        }
        Err(SanitizeFailure::RejectedInput) => {
            request
                .work_meter
                .reconcile_svg_bytes(reserved_svg_bytes, 0)?;
            return Err(crate::Error::invalid_icon_output(
                "icon SVG sanitizer rejected the output",
            ));
        }
        Err(SanitizeFailure::Output(BoundedSanitizeError::ParseInputLimitExceeded { .. })) => {
            request
                .work_meter
                .reconcile_svg_bytes(reserved_svg_bytes, 0)?;
            return Err(crate::Error::icon_processing(
                "icon sanitizer input exceeded its admitted serialization ceiling",
            ));
        }
        Err(
            SanitizeFailure::Output(BoundedSanitizeError::AllocationFailed)
            | SanitizeFailure::InvalidUtf8Output,
        ) => {
            request
                .work_meter
                .reconcile_svg_bytes(reserved_svg_bytes, 0)?;
            return Err(crate::Error::icon_processing(
                "icon SVG sanitizer failed internally",
            ));
        }
        Err(_) => {
            request
                .work_meter
                .reconcile_svg_bytes(reserved_svg_bytes, 0)?;
            return Err(crate::Error::icon_processing(
                "icon SVG sanitizer returned an unknown failure",
            ));
        }
    };
    if sanitized.is_empty() {
        request
            .work_meter
            .reconcile_svg_bytes(reserved_svg_bytes, 0)?;
        return Err(crate::Error::invalid_icon_output(
            "sanitization removed the complete icon SVG",
        ));
    }
    let validation = validate_well_formed_svg_with_controls(
        &sanitized,
        &mut || {
            request
                .work_meter
                .checkpoint(OperationPhase::Postprocess)
                .map_err(crate::Error::from)
        },
        &mut |elements, _tree_depth| {
            request
                .work_meter
                .preflight_svg_element_count(elements, OperationPhase::Postprocess)
                .map_err(crate::Error::from)
        },
    );
    if let Err(error) = validation {
        if matches!(
            &error,
            crate::Error::Cancelled(_)
                | crate::Error::ResourceLimitExceeded(_)
                | crate::Error::OperationResourceTerminal(_)
        ) {
            return Err(error);
        }
        request.work_meter.checkpoint(OperationPhase::Postprocess)?;
        request
            .work_meter
            .reconcile_svg_bytes(reserved_svg_bytes, 0)?;
        return Err(crate::Error::icon_processing(
            "sanitizer produced invalid SVG XML",
        ));
    }
    let current_color_use = if observe_paint_fact {
        observe_current_color_use(&sanitized, request.work_meter)?
    } else {
        IconCurrentColorUse::NotConsumed
    };
    request
        .work_meter
        .reconcile_svg_bytes(reserved_svg_bytes, sanitized.len())?;
    Ok(RenderedIcon {
        svg: sanitized,
        current_color_use,
    })
}

fn sanitizer_output_ceiling(
    projected_svg_bytes: usize,
    element_count: usize,
) -> crate::Result<usize> {
    projected_svg_bytes
        .checked_mul(SANITIZER_MAX_BYTE_EXPANSION)
        .and_then(|bytes| {
            element_count
                .checked_add(2)?
                .checked_mul(SANITIZER_MAX_ADDED_BYTES_PER_ELEMENT)
                .and_then(|overhead| bytes.checked_add(overhead))
        })
        .ok_or_else(|| crate::Error::icon_processing("sanitizer output limit overflowed"))
}

fn icon_render_work_units(
    icon: &ResolvedIcon,
    projected_svg_bytes: usize,
    sanitizer_ceiling: usize,
    observe_paint_fact: bool,
) -> crate::Result<usize> {
    let edit_work = icon
        .body
        .edit_count()
        .checked_mul(4)
        .ok_or_else(|| crate::Error::icon_processing("icon edit work estimate overflowed"))?;
    let projected_work = ceil_div(projected_svg_bytes, WORK_BYTES_PER_UNIT);
    let sanitizer_work = ceil_div(sanitizer_ceiling, WORK_BYTES_PER_UNIT);
    let paint_scan_work = if observe_paint_fact {
        sanitizer_work
    } else {
        0
    };
    ceil_div(icon.body.source_len(), WORK_BYTES_PER_UNIT)
        // Assembly is bounded by the transformed source. Tokenizer/tree work is charged by
        // check_parse_storage before allocation, independently of emitted output length.
        .checked_add(projected_work)
        // At most two Mermaid serialization stages plus final XML validation.
        .and_then(|work| work.checked_add(sanitizer_work.checked_mul(3)?))
        // The optional paint-fact scan is only charged for callers that consume its result.
        .and_then(|work| work.checked_add(paint_scan_work))
        .and_then(|work| work.checked_add(edit_work))
        .and_then(|work| work.checked_add(1))
        .ok_or_else(|| crate::Error::icon_processing("icon render work estimate overflowed"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IconPaintValue {
    CurrentColor,
    Fixed,
    NoPaint,
    Inherit,
    Unverified,
}

#[derive(Debug, Clone, Copy)]
struct InlinePaintWinner {
    value: IconPaintValue,
    important: bool,
}

#[derive(Debug, Default)]
struct IconCurrentColorObservation {
    consumed: bool,
    fixed: bool,
    unverified: bool,
}

impl IconCurrentColorObservation {
    const fn finish(self) -> IconCurrentColorUse {
        if self.consumed && !self.fixed && !self.unverified {
            IconCurrentColorUse::Consumed
        } else if self.consumed || self.unverified {
            IconCurrentColorUse::Unverified
        } else {
            IconCurrentColorUse::NotConsumed
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct IconPaintContext {
    non_rendering: bool,
    fill: IconPaintValue,
    stroke: IconPaintValue,
}

impl Default for IconPaintContext {
    fn default() -> Self {
        Self {
            non_rendering: false,
            fill: IconPaintValue::Fixed,
            stroke: IconPaintValue::NoPaint,
        }
    }
}

fn observe_current_color_use(
    svg: &str,
    work_meter: &crate::resources::OperationWorkMeter,
) -> crate::Result<IconCurrentColorUse> {
    let mut reader = quick_xml::Reader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut observation = IconCurrentColorObservation::default();
    let mut paint_stack = Vec::new();
    loop {
        work_meter.checkpoint(OperationPhase::Postprocess)?;
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let inherited = paint_stack.last().copied().unwrap_or_default();
                let context =
                    observe_icon_element(&element, inherited, work_meter, &mut observation)?;
                paint_stack.push(context);
            }
            Ok(Event::Empty(element)) => {
                let inherited = paint_stack.last().copied().unwrap_or_default();
                observe_icon_element(&element, inherited, work_meter, &mut observation)?;
            }
            Ok(Event::End(_)) => {
                paint_stack.pop().ok_or_else(|| {
                    crate::Error::icon_processing(
                        "validated icon SVG paint scan encountered an unmatched end tag",
                    )
                })?;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => {
                return Err(crate::Error::icon_processing(
                    "validated icon SVG paint scan failed",
                ));
            }
        }
    }
    work_meter.checkpoint(OperationPhase::Postprocess)?;
    Ok(observation.finish())
}

fn observe_icon_element(
    element: &BytesStart<'_>,
    inherited: IconPaintContext,
    work_meter: &crate::resources::OperationWorkMeter,
    observation: &mut IconCurrentColorObservation,
) -> crate::Result<IconPaintContext> {
    let tag = element.name().local_name();
    let tag = tag.as_ref();
    if tag.eq_ignore_ascii_case("style") || tag.eq_ignore_ascii_case("use") {
        observation.unverified = true;
    }

    let mut fill = None;
    let mut stroke = None;
    let mut inline_fill = None;
    let mut inline_stroke = None;
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| {
            crate::Error::icon_processing(
                "validated icon SVG paint scan encountered an invalid attribute",
            )
        })?;
        let name = attribute.key.local_name();
        let name = name.as_ref();
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| {
                crate::Error::icon_processing(
                    "validated icon SVG paint scan could not decode an attribute",
                )
            })?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("class") || name.eq_ignore_ascii_case("color") {
            if !value.is_empty() {
                observation.unverified = true;
            }
        } else if name.eq_ignore_ascii_case("fill") {
            fill = Some(classify_icon_paint_value(value));
        } else if name.eq_ignore_ascii_case("stroke") {
            stroke = Some(classify_icon_paint_value(value));
        } else if name.eq_ignore_ascii_case("style") {
            observe_inline_paint(
                value,
                work_meter,
                &mut inline_fill,
                &mut inline_stroke,
                observation,
            )?;
        }
    }

    let inherit = |paint: Option<IconPaintValue>, fallback| match paint {
        None | Some(IconPaintValue::Inherit) => fallback,
        Some(paint) => paint,
    };
    let context = IconPaintContext {
        non_rendering: inherited.non_rendering || is_non_rendering_container(tag),
        fill: inherit(
            inline_fill.map(|winner| winner.value).or(fill),
            inherited.fill,
        ),
        stroke: inherit(
            inline_stroke.map(|winner| winner.value).or(stroke),
            inherited.stroke,
        ),
    };
    if is_direct_paint_terminal(tag) && !context.non_rendering {
        // A line has no fill surface. Every other admitted direct terminal can paint
        // an inherited fill; a missing fill therefore retains SVG's fixed black default.
        for paint in [
            (!tag.eq_ignore_ascii_case("line")).then_some(context.fill),
            Some(context.stroke),
        ]
        .into_iter()
        .flatten()
        {
            match paint {
                IconPaintValue::CurrentColor => observation.consumed = true,
                IconPaintValue::Fixed => observation.fixed = true,
                IconPaintValue::NoPaint => {}
                IconPaintValue::Inherit | IconPaintValue::Unverified => {
                    observation.unverified = true
                }
            }
        }
    }
    Ok(context)
}

fn observe_inline_paint(
    style: &str,
    work_meter: &crate::resources::OperationWorkMeter,
    fill: &mut Option<InlinePaintWinner>,
    stroke: &mut Option<InlinePaintWinner>,
    observation: &mut IconCurrentColorObservation,
) -> crate::Result<()> {
    crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
        style,
        &mut || {
            work_meter
                .checkpoint(OperationPhase::Postprocess)
                .map_err(crate::Error::from)
        },
        |boundary| {
            let Some(declaration) = crate::mermaid_style::parse_style_declaration(boundary.raw())
            else {
                observation.unverified = true;
                return Ok(true);
            };
            let destination = match declaration.property() {
                "fill" => Some(&mut *fill),
                "stroke" => Some(&mut *stroke),
                "color" | "all" => {
                    observation.unverified = true;
                    None
                }
                _ => None,
            };
            if let Some(destination) = destination
                && destination.is_none_or(|current| declaration.important() || !current.important)
            {
                *destination = Some(InlinePaintWinner {
                    value: classify_icon_paint_value(declaration.value()),
                    important: declaration.important(),
                });
            }
            Ok(true)
        },
    )
}

fn classify_icon_paint_value(value: &str) -> IconPaintValue {
    let value = value.trim();
    if value.eq_ignore_ascii_case("currentcolor") {
        IconPaintValue::CurrentColor
    } else if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("transparent") {
        IconPaintValue::NoPaint
    } else if value.eq_ignore_ascii_case("inherit") {
        IconPaintValue::Inherit
    } else if let Ok(color) = merman_core::theme_color::ThemeColor::parse(value) {
        if color.rgba_channels().alpha == 0.0 {
            IconPaintValue::NoPaint
        } else {
            IconPaintValue::Fixed
        }
    } else {
        IconPaintValue::Unverified
    }
}

fn is_direct_paint_terminal(tag: &str) -> bool {
    [
        "circle", "ellipse", "line", "path", "polygon", "polyline", "rect", "text", "tspan",
    ]
    .into_iter()
    .any(|candidate| tag.eq_ignore_ascii_case(candidate))
}

fn is_non_rendering_container(tag: &str) -> bool {
    [
        "clipPath",
        "defs",
        "filter",
        "linearGradient",
        "marker",
        "mask",
        "pattern",
        "radialGradient",
        "symbol",
    ]
    .into_iter()
    .any(|candidate| tag.eq_ignore_ascii_case(candidate))
}

const fn ceil_div(value: usize, divisor: usize) -> usize {
    value / divisor + if value.is_multiple_of(divisor) { 0 } else { 1 }
}

fn render_dimension(value: f64, name: &'static str) -> crate::Result<String> {
    if !value.is_finite() || value <= 0.0 {
        return Err(crate::Error::invalid_icon_output(format!(
            "requested icon {name} is not finite and positive"
        )));
    }
    Ok(js_number(value))
}

struct TransformedIcon {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    wrapper: Option<(String, &'static str)>,
}

impl TransformedIcon {
    fn projected_body_bytes(&self, body: &ValidatedIconBody) -> crate::Result<usize> {
        let Some((start, end)) = &self.wrapper else {
            return Ok(body.scoped_len());
        };
        body.transformed_scoped_len(start.len(), end.len())
            .map_err(|_| crate::Error::icon_processing("transformed icon size overflowed"))
    }

    fn apply(&self, body: &ValidatedIconBody, scope: super::IconIdScope) -> crate::Result<String> {
        let Some((start, end)) = &self.wrapper else {
            return body
                .scope(scope)
                .map_err(|_| crate::Error::icon_processing("validated icon ID scoping failed"));
        };
        body.scope_transformed(scope, start, end)
            .map_err(|_| crate::Error::icon_processing("validated icon transform assembly failed"))
    }
}

fn transformed_icon(icon: &ResolvedIcon) -> crate::Result<TransformedIcon> {
    let mut left = icon.left;
    let mut top = icon.top;
    let mut width = icon.width;
    let mut height = icon.height;
    let mut transformations = Vec::with_capacity(3);
    let mut rotation = i32::from(icon.rotate);

    if icon.h_flip {
        if icon.v_flip {
            rotation += 2;
        } else {
            transformations.push(format!(
                "translate({} {})",
                js_number(width + left),
                js_number(-top)
            ));
            transformations.push("scale(-1 1)".to_string());
            left = 0.0;
            top = 0.0;
        }
    } else if icon.v_flip {
        transformations.push(format!(
            "translate({} {})",
            js_number(-left),
            js_number(height + top)
        ));
        transformations.push("scale(1 -1)".to_string());
        left = 0.0;
        top = 0.0;
    }

    rotation = rotation.rem_euclid(4);
    match rotation {
        1 => {
            let center = height / 2.0 + top;
            transformations.insert(
                0,
                format!("rotate(90 {} {})", js_number(center), js_number(center)),
            );
        }
        2 => transformations.insert(
            0,
            format!(
                "rotate(180 {} {})",
                js_number(width / 2.0 + left),
                js_number(height / 2.0 + top)
            ),
        ),
        3 => {
            let center = width / 2.0 + left;
            transformations.insert(
                0,
                format!("rotate(-90 {} {})", js_number(center), js_number(center)),
            );
        }
        _ => {}
    }
    if rotation % 2 == 1 {
        std::mem::swap(&mut left, &mut top);
        std::mem::swap(&mut width, &mut height);
    }

    for value in [left, top, width, height] {
        if !value.is_finite() {
            return Err(crate::Error::invalid_icon_output(
                "icon transformation produced non-finite geometry",
            ));
        }
    }
    let wrapper = (!transformations.is_empty()).then(|| {
        (
            format!(r#"<g transform="{}">"#, transformations.join(" ")),
            "</g>",
        )
    });
    Ok(TransformedIcon {
        left,
        top,
        width,
        height,
        wrapper,
    })
}

fn escape_xml_attr(value: &str) -> String {
    crate::xml::strip_forbidden_xml_1_0_chars(value)
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn js_number(mut value: f64) -> String {
    if value == -0.0 {
        value = 0.0;
    }
    let mut buffer = ryu_js::Buffer::new();
    buffer.format_finite(value).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use crate::svg::icon_registry::{icon_id_scope_for_test, limits::IconRegistryBuildLimits};
    use serde_json::json;
    use std::sync::Arc;

    #[derive(Debug)]
    struct InvalidUtf8Sink<'a>(BoundedSanitizeSink<'a>);

    impl SanitizeOutputSink for InvalidUtf8Sink<'_> {
        type Error = BoundedSanitizeError;

        fn checked_output_len(
            &self,
            current: usize,
            additional: usize,
        ) -> Result<usize, Self::Error> {
            self.0.checked_output_len(current, additional)
        }

        fn string_with_capacity(&self, capacity: usize) -> Result<String, Self::Error> {
            self.0.string_with_capacity(capacity)
        }

        fn output_buffer(&self, input_len: usize) -> Result<Vec<u8>, Self::Error> {
            self.0.output_buffer(input_len)
        }

        fn push_output_chunk(&self, output: &mut Vec<u8>, chunk: &[u8]) -> Result<(), Self::Error> {
            self.0.push_output_chunk(output, chunk)?;
            if let Some(first) = output.first_mut() {
                *first = u8::MAX;
            }
            Ok(())
        }
    }

    fn resolved_icon(body: &str) -> ResolvedIcon {
        ResolvedIcon {
            body: Arc::new(
                ValidatedIconBody::parse(body.to_owned(), 0, &IconRegistryBuildLimits::fixed())
                    .expect("test icon body must be admitted"),
            ),
            left: 0.0,
            top: 0.0,
            width: 16.0,
            height: 16.0,
            h_flip: false,
            v_flip: false,
            rotate: 0,
        }
    }

    fn render(
        body: &str,
        security_level: &str,
        policy: RenderResourcePolicy,
    ) -> crate::Result<String> {
        render_with_work_usage(body, security_level, policy).0
    }

    fn render_with_work_usage(
        body: &str,
        security_level: &str,
        policy: RenderResourcePolicy,
    ) -> (crate::Result<String>, usize) {
        let icon = resolved_icon(body);
        let config = merman_core::presentation::PresentationConfig::from_value(json!({
            "securityLevel": security_level,
            "htmlLabels": true
        }));
        let work_meter = OperationWorkMeter::new(policy);
        let result = render_resolved_icon(
            &icon,
            &IconRenderRequest {
                icon_name: "test:icon",
                width_px: 16.0,
                height_px: 16.0,
                fallback_prefix: None,
                extra_class: None,
                id_scope: icon_id_scope_for_test("icon-scope"),
                effective_config: &config,
                work_meter: &work_meter,
            },
        );
        (result, work_meter.used())
    }

    fn render_repeated_with_usage(
        body: &str,
        security_level: &str,
        repetitions: usize,
        policy: RenderResourcePolicy,
    ) -> (crate::Result<Vec<String>>, usize, usize) {
        let icon = resolved_icon(body);
        let config = merman_core::presentation::PresentationConfig::from_value(json!({
            "securityLevel": security_level,
            "htmlLabels": true
        }));
        let work_meter = OperationWorkMeter::new(policy);
        let mut rendered = Vec::with_capacity(repetitions);
        let result = (|| {
            for index in 0..repetitions {
                let scope = format!("icon-scope-{index}");
                rendered.push(render_resolved_icon(
                    &icon,
                    &IconRenderRequest {
                        icon_name: "test:icon",
                        width_px: 16.0,
                        height_px: 16.0,
                        fallback_prefix: None,
                        extra_class: None,
                        id_scope: icon_id_scope_for_test(&scope),
                        effective_config: &config,
                        work_meter: &work_meter,
                    },
                )?);
            }
            Ok(rendered)
        })();
        (result, work_meter.used(), work_meter.projected_svg_bytes())
    }

    #[test]
    fn recoverable_icon_markup_is_sanitized_in_strict_and_loose_modes() {
        let body = "<select><xmp><script>alert(1)</script></xmp></select>";
        for security_level in ["strict", "loose"] {
            let rendered = render(
                body,
                security_level,
                RenderResourcePolicy::unbounded_for_trusted_input(),
            )
            .expect("HTML5 recovery retains a safe, well-formed SVG");
            assert!(!rendered.contains("<script"), "{rendered}");
            assert!(!rendered.contains("alert(1)"), "{rendered}");
            assert!(rendered.starts_with("<svg"), "{rendered}");
            assert!(rendered.ends_with("</svg>"), "{rendered}");
        }
    }

    #[test]
    fn bounded_sanitizer_accepts_exact_output_limit_and_rejects_one_less() {
        let input = r#"<a href="https://example.com" target="_blank">example</a>"#;
        let config = merman_core::presentation::PresentationConfig::from_value(json!({
            "securityLevel": "strict",
            "htmlLabels": true
        }));
        let expected = merman_core::sanitize::sanitize_xhtml(input, config.as_config());
        assert!(expected.contains(r#"target="_blank""#));
        assert!(expected.contains(r#"rel="noopener""#));
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        assert_eq!(
            merman_core::sanitize::sanitize_xhtml_with_sink(
                input,
                config.as_config(),
                &BoundedSanitizeSink::new(expected.len(), 4096, &meter),
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            merman_core::sanitize::sanitize_xhtml_with_sink(
                input,
                config.as_config(),
                &BoundedSanitizeSink::new(expected.len() - 1, 4096, &meter),
            ),
            Err(SanitizeFailure::Output(
                BoundedSanitizeError::OutputLimitExceeded {
                    max_bytes: expected.len() - 1,
                },
            ))
        );
    }

    #[test]
    fn bounded_sanitizer_classifies_allocation_and_invalid_utf8_failures() {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        assert_eq!(
            BoundedSanitizeSink::new(usize::MAX, 4096, &meter).string_with_capacity(usize::MAX),
            Err(BoundedSanitizeError::AllocationFailed)
        );

        let config = merman_core::presentation::PresentationConfig::from_value(json!({
            "securityLevel": "strict",
            "htmlLabels": true
        }));
        assert_eq!(
            merman_core::sanitize::sanitize_xhtml_with_sink(
                "<b>ok</b>",
                config.as_config(),
                &InvalidUtf8Sink(BoundedSanitizeSink::new(4096, 4096, &meter)),
            ),
            Err(SanitizeFailure::InvalidUtf8Output)
        );
    }

    #[test]
    fn bounded_sanitizer_admits_parser_input_independently_of_output() {
        let input = "<script>removed</script><b>ok</b>";
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let config = merman_core::presentation::PresentationConfig::from_value(json!({
            "securityLevel": "strict",
            "htmlLabels": true
        }));
        assert_eq!(
            merman_core::sanitize::sanitize_xhtml_with_sink(
                input,
                config.as_config(),
                &BoundedSanitizeSink::new(4096, input.len() - 1, &meter),
            ),
            Err(SanitizeFailure::Output(
                BoundedSanitizeError::ParseInputLimitExceeded {
                    max_bytes: input.len() - 1,
                },
            )),
        );
    }

    #[test]
    fn bounded_sanitizer_charges_tree_storage_and_replays_cancellation() {
        let input = "<b>ok</b>";
        let config = merman_core::presentation::PresentationConfig::from_value(json!({
            "securityLevel": "strict",
            "htmlLabels": true
        }));
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let error = merman_core::sanitize::sanitize_xhtml_with_sink(
            input,
            config.as_config(),
            &BoundedSanitizeSink::new(4096, 4096, &meter),
        )
        .expect_err("tree growth must be charged before it is retained");
        assert!(matches!(
            error,
            SanitizeFailure::Output(BoundedSanitizeError::Control(
                crate::resources::OperationWorkError::ResourceLimitExceeded(_),
            )),
        ));

        let control = merman_core::OperationControl::new();
        control.cancel_after_checkpoints(8);
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        );
        let error = merman_core::sanitize::sanitize_xhtml_with_sink(
            input,
            config.as_config(),
            &BoundedSanitizeSink::new(4096, 4096, &meter),
        )
        .expect_err("cancellation is observed inside HTML5 parsing");
        let SanitizeFailure::Output(BoundedSanitizeError::Control(error)) = error else {
            panic!("sanitizer must retain its caller-owned terminal error");
        };
        assert!(matches!(
            error,
            crate::resources::OperationWorkError::Cancelled(_)
        ));
        assert_eq!(meter.checkpoint(OperationPhase::Postprocess), Err(error));
    }

    #[test]
    fn icon_xhtml_projection_preserves_svg_and_xlink_namespaces() {
        let body = r##"<defs><path id="p" d="M0 0h1v1z"/></defs><a xlink:href="#p"><path d="M0 0h1v1z"/></a>"##;
        let rendered = render(
            body,
            "strict",
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("safe XLink references remain well-formed SVG");
        let mut reader = quick_xml::NsReader::from_str(&rendered);
        reader.config_mut().enable_all_checks(true);
        let mut xlink_seen = false;
        loop {
            match reader.read_event().expect("sanitized SVG is XML") {
                Event::Start(element) | Event::Empty(element) => {
                    let (namespace, _) = reader.resolver().resolve_element(element.name());
                    assert!(
                        matches!(namespace, quick_xml::name::ResolveResult::Bound(ns)
                        if ns.as_ref() == "http://www.w3.org/2000/svg")
                    );
                    for attribute in element.attributes() {
                        let attribute = attribute.expect("namespace attributes are unique");
                        if attribute.key.as_ref() == "xlink:href" {
                            let (namespace, _) = reader.resolver().resolve_attribute(attribute.key);
                            assert!(
                                matches!(namespace, quick_xml::name::ResolveResult::Bound(ns)
                                if ns.as_ref() == "http://www.w3.org/1999/xlink")
                            );
                            xlink_seen = true;
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        assert!(xlink_seen, "{rendered}");
    }

    #[test]
    fn active_external_icon_content_is_sanitized_in_strict_and_loose_modes() {
        let cases = [
            ("script", "<script>alert(1)</script><path d=\"M0 0h1v1z\"/>"),
            ("event", "<path onload=\"alert(1)\" d=\"M0 0h1v1z\"/>"),
            (
                "style",
                "<style>.owned{fill:red}</style><path class=\"owned\" d=\"M0 0h1v1z\"/>",
            ),
            (
                "foreign-object",
                "<foreignObject><div onclick=\"alert(1)\">owned</div></foreignObject><path d=\"M0 0h1v1z\"/>",
            ),
            (
                "href",
                "<a href=\"javascript:alert(1)\"><path d=\"M0 0h1v1z\"/></a>",
            ),
            (
                "xlink-href",
                "<a xmlns:xlink=\"http://www.w3.org/1999/xlink\" xlink:href=\"javascript:alert(1)\"><path d=\"M0 0h1v1z\"/></a>",
            ),
        ];

        for security_level in ["strict", "loose"] {
            for (name, body) in cases {
                let rendered = render(
                    body,
                    security_level,
                    RenderResourcePolicy::unbounded_for_trusted_input(),
                )
                .unwrap_or_else(|error| {
                    panic!("{name} must sanitize safely in {security_level} mode: {error}")
                });
                let lower = rendered.to_ascii_lowercase();
                for forbidden in [
                    "<script",
                    "onload=",
                    "onclick=",
                    "<style",
                    "<foreignobject",
                    "javascript:",
                    "xlink:href=",
                ] {
                    assert!(
                        !lower.contains(forbidden),
                        "{name} retained {forbidden:?} in {security_level} mode: {rendered}"
                    );
                }
                assert!(lower.starts_with("<svg"), "{rendered}");
                assert!(lower.ends_with("</svg>"), "{rendered}");
            }
        }
    }

    #[test]
    fn safe_sanitizer_growth_is_accounted_at_the_svg_limit() {
        let body = r#"<a href="https://example.com" target="_blank"><path d="M0 0h16v16H0z"/></a>"#;
        let rendered = render(
            body,
            "strict",
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("safe sanitizer growth is accepted");
        assert!(rendered.contains(r#"rel="noopener""#));

        let exact = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, rendered.len())
            .unwrap();
        assert_eq!(render(body, "strict", exact).unwrap(), rendered);

        let one_less = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, rendered.len() - 1)
            .unwrap();
        let error = render(body, "strict", one_less)
            .expect_err("sanitized output one byte above the SVG budget must be rejected");
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
    }

    #[test]
    fn icon_xml_validation_preserves_resource_error_classification() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgElements, 1)
            .unwrap();

        let error = render("<path d=\"M0 0h16v16H0z\"/>", "strict", policy)
            .expect_err("the generated svg and path exceed the one-element validation limit");

        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
    }

    #[test]
    fn sanitizer_ceiling_covers_worst_case_attribute_quote_serialization() {
        let quote_count = 70 * 1024;
        let quotes = "\"".repeat(quote_count);
        let body = format!(r#"<a target='{quotes}'><path d="M0 0h16v16H0z"/></a>"#);

        let rendered = render(
            &body,
            "strict",
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("a valid admitted attribute must fit the source-backed sanitizer ceiling");

        assert_eq!(rendered.matches("&quot;").count(), quote_count);
    }

    #[test]
    fn icon_render_work_precharge_is_exact_and_includes_sanitizer_passes() {
        let body = r#"<a target="_blank"><path d="M0 0h16v16H0z"/></a>"#;
        let (_, used) = render_with_work_usage(
            body,
            "strict",
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        assert!(used > 0);

        let exact = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, used)
            .unwrap();
        assert!(render(body, "strict", exact).is_ok());

        let one_less = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, used - 1)
            .unwrap();
        let error = render(body, "strict", one_less)
            .expect_err("one work unit below the precharge must reject before rendering");
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
    }

    #[test]
    fn repeated_maximum_body_precharges_aggregate_svg_and_work_exactly() {
        let maximum =
            usize::try_from(crate::svg::IconRegistryResourceLimitId::MaxBodyBytes.fixed_value())
                .expect("maximum icon body bytes fit usize");
        let prefix = r#"<path data-padding=""#;
        let suffix = r#"" d="M0 0H16V16H0z"/>"#;
        let body = format!(
            "{prefix}{}{suffix}",
            "x".repeat(maximum - prefix.len() - suffix.len())
        );
        assert_eq!(body.len(), maximum);

        let repetitions = 4;
        let (baseline, work, svg_bytes) = render_repeated_with_usage(
            &body,
            "strict",
            repetitions,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        assert_eq!(
            baseline.expect("maximum icon body renders").len(),
            repetitions
        );
        assert!(work > 0);
        assert!(svg_bytes >= maximum.checked_mul(repetitions).unwrap());

        let exact = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work)
            .unwrap()
            .with_limit(ResourceLimitId::MaxSvgBytes, svg_bytes)
            .unwrap();
        assert!(
            render_repeated_with_usage(&body, "strict", repetitions, exact)
                .0
                .is_ok()
        );

        let svg_one_less = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, svg_bytes - 1)
            .unwrap();
        let error = render_repeated_with_usage(&body, "strict", repetitions, svg_one_less)
            .0
            .expect_err("aggregate SVG bytes plus one must fail before the final expansion");
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));

        let work_one_less = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work - 1)
            .unwrap();
        let error = render_repeated_with_usage(&body, "strict", repetitions, work_one_less)
            .0
            .expect_err("aggregate icon work plus one must fail before the final expansion");
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
    }
}
