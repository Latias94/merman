use super::super::*;
use crate::treemap::{
    TREEMAP_SECTION_HEADER_HEIGHT_PX, TREEMAP_SECTION_INNER_PADDING_PX, TREEMAP_TITLE_CLASS,
};

fn treemap_group_decimal(value: &str) -> String {
    let (sign, unsigned) = value
        .strip_prefix('-')
        .or_else(|| value.strip_prefix('−'))
        .map_or(("", value), |rest| ("−", rest));
    let (integer, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, ""), |(integer, fraction)| (integer, fraction));
    let mut grouped = String::with_capacity(value.len() + integer.len() / 3);
    for (index, digit) in integer.bytes().enumerate() {
        if index != 0 && (integer.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(char::from(digit));
    }
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{fraction}")
    }
}

fn treemap_js_number(value: f64) -> String {
    if !value.is_finite() {
        return "NaN".to_string();
    }
    let mut buffer = ryu_js::Buffer::new();
    let rendered = buffer
        .format_finite(if value == -0.0 { 0.0 } else { value })
        .to_string();
    if let Some(rest) = rendered.strip_prefix('-') {
        format!("−{rest}")
    } else {
        rendered
    }
}

fn treemap_exponential_parts(value: f64, significant_digits: usize) -> Option<(String, i32)> {
    if !value.is_finite() || value == 0.0 {
        return None;
    }
    let raw = format!("{:.*e}", significant_digits.saturating_sub(1), value.abs());
    let Some((mantissa, exponent)) = raw.split_once('e') else {
        return None;
    };
    let digits = mantissa.replace('.', "");
    let exponent = exponent.parse::<i32>().ok()?;
    Some((digits, exponent))
}

fn treemap_trim_decimal(mut value: String) -> String {
    if let Some(dot) = value.find('.') {
        while value.ends_with('0') {
            value.pop();
        }
        if value.len() == dot + 1 {
            value.pop();
        }
    }
    value
}

fn treemap_format_significant(value: f64, precision: usize) -> String {
    if !value.is_finite() {
        return "NaN".to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }

    let precision = precision.clamp(1, 21);
    let negative = value.is_sign_negative();
    let Some((mut digits, exponent)) = treemap_exponential_parts(value, precision) else {
        return treemap_js_number(value);
    };
    while digits.ends_with('0') {
        digits.pop();
    }

    let mut formatted = if exponent >= precision as i32 || exponent < -6 {
        let mantissa = if digits.len() == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let exponent = if exponent >= 0 {
            format!("+{exponent}")
        } else {
            exponent.to_string()
        };
        format!("{mantissa}e{exponent}")
    } else {
        let decimal_position = exponent + 1;
        if decimal_position <= 0 {
            format!("0.{}{}", "0".repeat((-decimal_position) as usize), digits)
        } else if decimal_position as usize >= digits.len() {
            format!(
                "{}{}",
                digits,
                "0".repeat(decimal_position as usize - digits.len())
            )
        } else {
            let split_at = decimal_position as usize;
            format!("{}.{}", &digits[..split_at], &digits[split_at..])
        }
    };
    formatted = treemap_trim_decimal(formatted);
    if negative {
        format!("−{formatted}")
    } else {
        formatted
    }
}

fn treemap_format_fixed(value: f64, precision: usize, grouped: bool) -> String {
    let formatted = format!("{:.*}", precision.min(20), value);
    if grouped {
        treemap_group_decimal(&formatted)
    } else if formatted.starts_with('-') {
        format!("−{}", &formatted[1..])
    } else {
        formatted
    }
}

fn treemap_format_scientific(value: f64, precision: usize) -> String {
    let raw = format!("{:.*e}", precision.min(20), value.abs());
    let Some((mantissa, exponent)) = raw.split_once('e') else {
        return raw;
    };
    let exponent = exponent
        .parse::<i32>()
        .map_or_else(|_| exponent.to_string(), |value| format!("{value:+}"));
    let formatted = format!("{mantissa}e{exponent}");
    if value.is_sign_negative() {
        format!("−{formatted}")
    } else {
        formatted
    }
}

fn treemap_format_precision(format_str: &str) -> Option<usize> {
    let dot = format_str.find('.')?;
    let digits = format_str[dot + 1..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().unwrap_or(6))
}

fn treemap_default_format(value: f64) -> String {
    let mut formatted = treemap_format_significant(value, 12);
    if !formatted.contains('e') {
        formatted = treemap_group_decimal(&formatted);
    }
    formatted
}

fn treemap_format_standard(value: f64, format_str: &str) -> Option<String> {
    let format_str = format_str.trim();
    if format_str.is_empty() {
        return Some(treemap_js_number(value));
    }

    let (grouped, body) = format_str
        .strip_prefix(',')
        .map_or((false, format_str), |rest| (true, rest));
    if body.contains(',') {
        return None;
    }

    let (precision, type_char) = if let Some(rest) = body.strip_prefix('.') {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let precision = rest[..digits].parse().unwrap_or(6);
        let suffix = &rest[digits..];
        let type_char = match suffix {
            "" => None,
            "%" | "e" | "f" | "g" => suffix.chars().next(),
            _ => return None,
        };
        (Some(precision), type_char)
    } else {
        let type_char = match body {
            "" => None,
            "%" | "e" | "f" | "g" => body.chars().next(),
            _ => return None,
        };
        (None, type_char)
    };

    if type_char.is_none() && precision.is_none() && !grouped {
        return None;
    }

    match type_char {
        Some('%') => Some(format!(
            "{}%",
            treemap_format_fixed(value * 100.0, precision.unwrap_or(6), false)
        )),
        Some('e') => Some(treemap_format_scientific(value, precision.unwrap_or(6))),
        Some('f') => Some(treemap_format_fixed(value, precision.unwrap_or(6), grouped)),
        Some('g') => Some(treemap_format_significant(value, precision.unwrap_or(6))),
        None => {
            let mut formatted = treemap_format_significant(value, precision.unwrap_or(12));
            if grouped && !formatted.contains('e') {
                formatted = treemap_group_decimal(&formatted);
            }
            Some(formatted)
        }
        Some(_) => None,
    }
}

fn treemap_format_value(value: f64, format_str: &str) -> String {
    let format_str = format_str.trim();
    let format_str = if format_str.is_empty() {
        ","
    } else {
        format_str
    };
    if format_str == "$0,0" {
        return format!(
            "${}",
            treemap_format_standard(value, ",").unwrap_or_else(|| treemap_default_format(value))
        );
    }
    if format_str.starts_with('$') && format_str.contains(',') {
        let precision = treemap_format_precision(format_str)
            .map_or_else(String::new, |precision| format!(".{precision}"));
        return format!(
            "${}",
            treemap_format_standard(value, &format!(",{precision}"))
                .unwrap_or_else(|| treemap_default_format(value))
        );
    }
    if let Some(rest) = format_str.strip_prefix('$') {
        return treemap_format_standard(value, rest)
            .map(|formatted| format!("${formatted}"))
            .unwrap_or_else(|| treemap_default_format(value));
    }
    treemap_format_standard(value, format_str).unwrap_or_else(|| treemap_default_format(value))
}

// Treemap diagram SVG renderer implementation (split from parity.rs).

fn measure_treemap_computed_length(
    measurer: &dyn crate::text::TextMeasurer,
    text: &str,
    style: &crate::text::TextStyle,
    work_meter: &crate::resources::OperationWorkMeter,
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
    work_meter: &crate::resources::OperationWorkMeter,
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

fn write_treemap_leaf_group_open(
    out: &mut impl SvgOutput,
    group_class: &str,
    x: f64,
    y: f64,
) -> Result<()> {
    let _ = write!(
        out,
        r#"<g class="{class}" transform="translate({x},{y})">"#,
        class = escape_attr(group_class),
        x = fmt(x),
        y = fmt(y)
    );
    out.checkpoint()
}

pub(crate) fn render_treemap_diagram_svg(
    layout: &crate::model::TreemapDiagramLayout,
    effective_config: &serde_json::Value,
    title_theme: &crate::treemap::TreemapTitleThemePlan,
    typography_theme: &crate::treemap::TreemapTypographyThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    #[derive(Default)]
    struct OrdinalScale {
        range: Vec<String>,
        domain: std::collections::HashMap<String, usize>,
    }

    impl OrdinalScale {
        fn get(&mut self, key: &str) -> String {
            let idx = if let Some(idx) = self.domain.get(key).copied() {
                idx
            } else {
                let idx = self.domain.len();
                self.domain.insert(key.to_string(), idx);
                idx
            };
            if self.range.is_empty() {
                return String::new();
            }
            self.range[idx % self.range.len()].clone()
        }
    }

    fn replace_first(haystack: &str, needle: &str, replacement: &str) -> String {
        if needle.is_empty() {
            return haystack.to_string();
        }
        let Some(idx) = haystack.find(needle) else {
            return haystack.to_string();
        };
        let mut out = String::with_capacity(haystack.len() - needle.len() + replacement.len());
        out.push_str(&haystack[..idx]);
        out.push_str(replacement);
        out.push_str(&haystack[idx + needle.len()..]);
        out
    }

    #[derive(Default)]
    struct OrderedMap {
        order: Vec<(String, String)>,
        idx: std::collections::HashMap<String, usize>,
    }

    impl OrderedMap {
        fn set(&mut self, k: &str, v: &str) {
            if k.is_empty() {
                return;
            }
            if let Some(&i) = self.idx.get(k) {
                self.order[i].1 = v.to_string();
                return;
            }
            self.idx.insert(k.to_string(), self.order.len());
            self.order.push((k.to_string(), v.to_string()));
        }
    }

    fn treemap_is_label_style(key: &str) -> bool {
        matches!(
            key.trim(),
            "color"
                | "font-size"
                | "font-family"
                | "font-weight"
                | "font-style"
                | "text-decoration"
                | "text-align"
                | "text-transform"
                | "line-height"
                | "letter-spacing"
                | "word-spacing"
                | "text-shadow"
                | "text-overflow"
                | "white-space"
                | "word-wrap"
                | "word-break"
                | "overflow-wrap"
                | "hyphens"
        )
    }

    #[derive(Default)]
    struct TreemapCompiledStyles {
        label_styles: String,
        label_styles_without_font_size: String,
        text_fill_ownership: crate::treemap::TreemapTextFillOwnership,
        text_fill_without_font_size_ownership: crate::treemap::TreemapTextFillOwnership,
        node_styles: String,
        border_styles: Vec<String>,
    }

    fn treemap_styles2_string(
        css_compiled_styles: &[String],
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<TreemapCompiledStyles> {
        // Ported from Mermaid `handDrawnShapeStyles.compileStyles()` / `styles2String()`:
        // - preserve insertion order of the first occurrence of a key
        // - later occurrences override values, without changing order
        // - tolerate tokens without `:` (JS `split(':')` yields `value = undefined`)
        let mut m = OrderedMap::default();

        for entry in css_compiled_styles {
            work_meter.charge(1usize.saturating_add(entry.len().div_ceil(64)))?;
            let mut checkpoint = || work_meter.checkpoint(OperationPhase::Emit);
            crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
                entry,
                &mut checkpoint,
                |boundary| {
                    work_meter.charge(1)?;
                    let raw = boundary.raw();
                    let s = raw
                        .trim()
                        .strip_suffix(';')
                        .unwrap_or_else(|| raw.trim())
                        .trim();
                    if s.is_empty() {
                        return Ok(true);
                    }
                    let (k, v) = if let Some(declaration) =
                        crate::mermaid_style::parse_style_declaration(raw)
                    {
                        (declaration.property_source(), declaration.source_value())
                    } else if let Some((k, v)) = s.split_once(':') {
                        (k.trim(), v.trim())
                    } else {
                        (s.trim(), "")
                    };
                    m.set(k, v);
                    Ok(true)
                },
            )?;
        }

        let mut label_styles: Vec<String> = Vec::new();
        let mut label_styles_without_font_size: Vec<String> = Vec::new();
        let mut node_styles: Vec<String> = Vec::new();
        let mut border_styles: Vec<String> = Vec::new();
        let mut text_fill_ownership = crate::treemap::TreemapTextFillOwnership::Generated;
        let mut text_fill_without_font_size_ownership = text_fill_ownership;
        let mut label_styles_contain_color = false;
        let mut label_styles_without_font_size_contain_color = false;

        for (k, v) in &m.order {
            if v.is_empty() {
                continue;
            }
            work_meter.charge(
                1usize
                    .saturating_add(k.len().div_ceil(64))
                    .saturating_add(v.len().div_ceil(64)),
            )?;
            let decl = format!("{k}:{v}");
            let decl_imp = format!("{decl} !important");
            if treemap_is_label_style(k) {
                if k == "color" {
                    // The emitted suffix converts this declaration to fill and appends !important.
                    // Dynamic or invalid values cannot prove that the generated fill is shadowed.
                    text_fill_ownership = if crate::mermaid_style::is_supported_css_color_value(v)
                        || v.eq_ignore_ascii_case("none")
                    {
                        crate::treemap::TreemapTextFillOwnership::SourceOwned
                    } else {
                        crate::treemap::TreemapTextFillOwnership::Unverified
                    };
                    text_fill_without_font_size_ownership = text_fill_ownership;
                    // Preserve the writer's first-substring replacement, including values that
                    // contain `color:` before the real declaration. Such output is not proof of
                    // a source-owned fill; the two suffixes can have different first matches.
                    if label_styles_contain_color {
                        text_fill_ownership = crate::treemap::TreemapTextFillOwnership::Unverified;
                    }
                    if label_styles_without_font_size_contain_color {
                        text_fill_without_font_size_ownership =
                            crate::treemap::TreemapTextFillOwnership::Unverified;
                    }
                }
                let contains_color = decl_imp.contains("color:");
                label_styles_contain_color |= contains_color;
                label_styles.push(decl_imp.clone());
                if k.trim() != "font-size" {
                    label_styles_without_font_size_contain_color |= contains_color;
                    label_styles_without_font_size.push(decl_imp);
                }
            } else {
                node_styles.push(decl_imp.clone());
                if k.contains("stroke") {
                    border_styles.push(decl_imp);
                }
            }
        }

        Ok(TreemapCompiledStyles {
            label_styles: label_styles.join(";"),
            label_styles_without_font_size: label_styles_without_font_size.join(";"),
            text_fill_ownership,
            text_fill_without_font_size_ownership,
            node_styles: node_styles.join(";"),
            border_styles,
        })
    }

    fn normalize_dom_style_color(color: &str) -> String {
        // Upstream mutates this style through D3 after setting the attribute, so preserve the
        // browser CSSOM serialization boundary while sharing the color parser.
        super::super::util::cssom_color_value(color)
    }

    let diagram_id = options.diagram_id_or("treemap");

    let theme = MermaidThemeAdapter::new(effective_config).treemap()?;
    let typed_label_text_fill = typography_theme.label_text_fill_css();
    let typed_value_text_fill = typography_theme.value_text_fill_css();

    let mut color_scale = OrdinalScale::default();
    color_scale.range.push("transparent".to_string());
    color_scale.range.extend(theme.color_scale.iter().cloned());

    let mut color_scale_peer = OrdinalScale::default();
    color_scale_peer.range.push("transparent".to_string());
    color_scale_peer
        .range
        .extend(theme.color_scale_peer.iter().cloned());

    let mut color_scale_label = OrdinalScale::default();
    color_scale_label
        .range
        .extend(theme.color_scale_label.iter().cloned());

    let has_acc_title = layout
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = layout
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());

    let measurer = options.text_measurer();
    let title = layout.title.as_deref().filter(|t| !t.trim().is_empty());
    let mut typography_theme_receipt = typography_theme.begin_terminal_receipt(layout);
    let title_shift_y = layout.title_height;
    let (title_text_style, title_bbox) = if let Some(title) = title {
        options
            .work_meter()
            .charge(1usize.saturating_add(theme.title_font_size.len().div_ceil(64)))?;
        let title_text_style = typography_theme.title_text_style(&theme.title_font_size);
        let measurement_work = 1usize.saturating_add(title.len().div_ceil(64));
        options
            .work_meter()
            .charge(measurement_work.saturating_mul(2))?;
        let style = title_text_style.style();
        let w = measurer
            .measure_svg_simple_text_bbox_width_px(title, style)
            .max(0.0);
        let h = measurer
            .measure_svg_simple_text_bbox_height_px(title, style)
            .max(0.0);
        (Some(title_text_style), Some((w, h)))
    } else {
        (None, None)
    };

    #[derive(Debug, Clone, Copy)]
    struct TreemapRect {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    }

    #[derive(Debug, Clone, Copy)]
    struct TreemapViewBoxBounds {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    impl TreemapViewBoxBounds {
        const fn empty() -> Self {
            Self {
                min_x: f64::INFINITY,
                min_y: f64::INFINITY,
                max_x: f64::NEG_INFINITY,
                max_y: f64::NEG_INFINITY,
            }
        }

        fn include_rect(&mut self, rect: TreemapRect) {
            let TreemapRect { x0, y0, x1, y1 } = rect;
            let w = x1 - x0;
            let h = y1 - y0;
            if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
                return;
            }
            self.min_x = self.min_x.min(x0);
            self.min_y = self.min_y.min(y0);
            self.max_x = self.max_x.max(x1);
            self.max_y = self.max_y.max(y1);
        }

        fn has_rects(self) -> bool {
            self.min_x.is_finite()
                && self.min_y.is_finite()
                && self.max_x.is_finite()
                && self.max_y.is_finite()
        }
    }

    let mut viewbox_bounds = TreemapViewBoxBounds::empty();

    for s in &layout.sections {
        if s.depth == 0 {
            continue;
        }
        viewbox_bounds.include_rect(TreemapRect {
            x0: s.x0,
            y0: s.y0,
            x1: s.x1,
            y1: s.y1,
        });
    }
    for l in &layout.leaves {
        viewbox_bounds.include_rect(TreemapRect {
            x0: l.x0,
            y0: l.y0,
            x1: l.x1,
            y1: l.y1,
        });
    }

    // Treemap sections/leaves are rendered under `<g class="treemapContainer" transform="translate(0, title_height)">`.
    // Include that translation when computing the root viewport. Also include the title text's
    // bbox (dominant-baseline="middle") so `parity-root` matches the upstream getBBox-derived
    // viewBox w/h.
    if title_shift_y > 0.0 && viewbox_bounds.min_y.is_finite() && viewbox_bounds.max_y.is_finite() {
        viewbox_bounds.min_y += title_shift_y;
        viewbox_bounds.max_y += title_shift_y;
    }
    if let (Some(title), Some(&(w, h))) = (title, title_bbox.as_ref()) {
        let cx = layout.width / 2.0;
        let cy = layout.title_height / 2.0;
        if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
            if !title.trim().is_empty() {
                // If measurement is unexpectedly degenerate, still ensure we don't ignore the title
                // region entirely.
                viewbox_bounds.min_y = viewbox_bounds.min_y.min(0.0);
                viewbox_bounds.max_y = viewbox_bounds.max_y.max(layout.title_height);
            }
        } else {
            viewbox_bounds.include_rect(TreemapRect {
                x0: cx - (w / 2.0),
                y0: cy - (h / 2.0),
                x1: cx + (w / 2.0),
                y1: cy + (h / 2.0),
            });
        }
    }

    let vb_x;
    let vb_y;
    let vb_w;
    let vb_h;
    if viewbox_bounds.has_rects() {
        vb_x = viewbox_bounds.min_x - layout.diagram_padding;
        vb_y = viewbox_bounds.min_y - layout.diagram_padding;
        vb_w = (viewbox_bounds.max_x - viewbox_bounds.min_x) + layout.diagram_padding * 2.0;
        vb_h = (viewbox_bounds.max_y - viewbox_bounds.min_y) + layout.diagram_padding * 2.0;
    } else {
        vb_x = -layout.diagram_padding;
        vb_y = -layout.diagram_padding;
        vb_w = layout.diagram_padding * 2.0;
        vb_h = layout.diagram_padding * 2.0;
    }

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let extra_attrs: [(&str, &str); 1] = [("class", "flowchart")];
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "treemap");
    root_chrome.extra_attrs = &extra_attrs;
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = root_svg::RootDomProfile {
        style_viewbox_order: root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::TREEMAP, diagram_id)
            .write_open(
                &mut out,
                root_svg::RootViewportSpec::responsive(root_svg::DiagramBounds::from_view_box(
                    vb_x, vb_y, vb_w, vb_h,
                )),
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
            escape_xml(descr.trim_end_matches('\n'))
        );
        out.checkpoint()?;
    }

    let mut title_theme_receipt = title_theme.begin_terminal_receipt();
    let (css, title_css_emission, typography_css_emission) =
        super::super::css::treemap_css_with_title_fill_and_font_family(
            diagram_id,
            effective_config,
            Some(typography_theme.font_family_css()),
            title_theme.fill_css(),
            typography_theme.label_text_fill_css(),
            typography_theme.value_text_fill_css(),
        )?;
    if let Some(receipt) = title_theme_receipt.as_mut() {
        receipt.record_stylesheet(title_css_emission.class(), title_css_emission.fill());
    }
    if let Some(receipt) = typography_theme_receipt.as_mut() {
        receipt.record_css_emission(typography_css_emission);
    }
    let _ = write!(&mut out, "<style>{}</style>", css);
    drop(css);
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
        if let Some(receipt) = title_theme_receipt.as_mut() {
            receipt.record_title_text(TREEMAP_TITLE_CLASS);
        }
        if let Some(receipt) = typography_theme_receipt.as_mut() {
            receipt.record_text(
                crate::treemap::TreemapTextRole::Title,
                true,
                crate::treemap::TreemapTextFillOwnership::Generated,
                title_text_style
                    .as_ref()
                    .expect("a visible Treemap title has a resolved text style"),
                true,
            );
        }
    }

    let _ = write!(
        &mut out,
        r#"<g transform="translate(0, {ty})" class="treemapContainer">"#,
        ty = fmt(layout.title_height)
    );
    out.checkpoint()?;

    let computed_length_measurer = options.text_measurer_for(TextMeasurementPhase::ComputedLength);
    let section_header_height = TREEMAP_SECTION_HEADER_HEIGHT_PX;
    let section_header_center_y = section_header_height / 2.0;
    let section_label_inset_x: f64 = 6.0;
    let section_label_font_size: f64 = 12.0;
    let section_value_font_size: f64 = 10.0;
    let section_inner_padding = TREEMAP_SECTION_INNER_PADDING_PX;
    let section_label_reserved_value_width: f64 = 30.0;
    let section_label_min_visible_width: f64 = 15.0;

    for (i, section) in layout.sections.iter().enumerate() {
        let section_clip_id = format!("clip-section-{diagram_id}-{i}");
        options.checkpoint_emit()?;
        options.work_meter().charge(1)?;
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

        let fill = color_scale.get(&section.name);
        let stroke = color_scale_peer.get(&section.name);
        let section_css: &[String] = section.css_compiled_styles.as_deref().unwrap_or(&[]);
        let compiled = treemap_styles2_string(section_css, options.work_meter())?;
        let section_label_text_style =
            typography_theme.section_text_style(i, section_label_font_size, Some("bold"), None);
        let section_style = if section.depth == 0 {
            "display: none;".to_string()
        } else {
            format!(
                "{};{}",
                compiled.node_styles,
                compiled.border_styles.join(";")
            )
        };
        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{h}" class="treemapSection section{i}" fill="{fill}" fill-opacity="0.6" stroke="{stroke}" stroke-width="2" stroke-opacity="0.4" style="{style}"/>"#,
            w = fmt(w),
            h = fmt(h),
            i = i,
            fill = escape_attr(&fill),
            stroke = escape_attr(&stroke),
            style = escape_attr(&section_style)
        );
        out.checkpoint()?;

        let mut label_text = if section.depth == 0 {
            String::new()
        } else {
            options
                .work_meter()
                .charge(1usize.saturating_add(section.name.len().div_ceil(64)))?;
            section.name.clone()
        };

        let default_label_fill = if section.depth == 0 {
            String::new()
        } else {
            color_scale_label.get(&section.name)
        };
        let label_fill =
            typed_label_text_fill.map_or_else(|| default_label_fill.clone(), str::to_owned);
        let value_fill = if section.depth == 0 {
            String::new()
        } else if let Some(fill) = typed_value_text_fill {
            fill.to_owned()
        } else {
            default_label_fill
        };
        let label_styles_suffix = replace_first(&compiled.label_styles, "color:", "fill:");

        if label_text.is_empty() {
            let _ = write!(
                &mut out,
                r#"<text class="treemapSectionLabel" x="{x}" y="{y}" dominant-baseline="middle" font-weight="bold" clip-path="url(#{id})" style="display: none;"/>"#,
                x = fmt(section_label_inset_x),
                y = fmt(section_header_center_y),
                id = section_clip_id.as_str(),
            );
            out.checkpoint()?;
            if let Some(receipt) = typography_theme_receipt.as_mut() {
                receipt.record_text(
                    crate::treemap::TreemapTextRole::SectionLabel,
                    false,
                    compiled.text_fill_ownership,
                    &section_label_text_style,
                    true,
                );
            }
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
                &computed_length_measurer,
                &label_text,
                style,
                options.work_meter(),
            )? > actual_available_width
            {
                let ellipsis = "...";
                options
                    .work_meter()
                    .charge(1usize.saturating_add(label_text.len().div_ceil(64)))?;
                let candidate_capacity = label_text
                    .len()
                    .checked_add(ellipsis.len())
                    .ok_or_else(|| options.work_meter().arithmetic_overflow())?;
                let mut candidate = String::with_capacity(candidate_capacity);
                candidate.push_str(&label_text);
                while !candidate.is_empty() {
                    candidate.pop();
                    if candidate.is_empty() {
                        if measure_treemap_computed_length(
                            &computed_length_measurer,
                            ellipsis,
                            style,
                            options.work_meter(),
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
                        &computed_length_measurer,
                        &candidate,
                        style,
                        options.work_meter(),
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
                fill = escape_attr(&label_fill),
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
            if let Some(receipt) = typography_theme_receipt.as_mut() {
                receipt.record_text(
                    crate::treemap::TreemapTextRole::SectionLabel,
                    !label_text.trim().is_empty(),
                    compiled.text_fill_ownership,
                    &section_label_text_style,
                    true,
                );
            }
        }

        if layout.show_values {
            let section_value_text_style = typography_theme.section_text_style(
                i,
                section_value_font_size,
                None,
                Some("italic"),
            );
            let value_text = if section.value != 0.0 {
                treemap_format_value(section.value, &layout.value_format)
            } else {
                String::new()
            };
            let section_value_style = if section.depth == 0 {
                "display: none;".to_string()
            } else {
                format!(
                    "text-anchor: end; dominant-baseline: middle; font-size: {}px; fill:{fill}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;{suffix}",
                    fmt(section_value_font_size),
                    fill = escape_attr(&value_fill),
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
            if let Some(receipt) = typography_theme_receipt.as_mut() {
                receipt.record_text(
                    crate::treemap::TreemapTextRole::SectionValue,
                    section.depth != 0 && !value_text.is_empty(),
                    compiled.text_fill_ownership,
                    &section_value_text_style,
                    true,
                );
            }
        }

        out.push_str("</g>");
        out.checkpoint()?;
    }

    let is_complex_treemap = layout.leaves.len() > 20;
    let base_label_font_size = if is_complex_treemap { 16.0 } else { 38.0 };
    let base_value_font_size = if is_complex_treemap { 14.0 } else { 28.0 };
    let min_label_font_size = if is_complex_treemap { 4.0 } else { 8.0 };
    let min_value_font_size = if is_complex_treemap { 4.0 } else { 6.0 };
    let label_padding = if is_complex_treemap { 2.0 } else { 4.0 };
    let min_display_threshold = if is_complex_treemap { 8.0 } else { 10.0 };
    let spacing_between_label_and_value = if is_complex_treemap { 1.0 } else { 2.0 };

    for (i, leaf) in layout.leaves.iter().enumerate() {
        let leaf_clip_id = format!("clip-{diagram_id}-{i}");
        options.checkpoint_emit()?;
        options.work_meter().charge(1)?;
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

        let fill_key = leaf.parent_name.as_deref().unwrap_or(leaf.name.as_str());
        let fill = color_scale.get(fill_key);

        let leaf_css: &[String] = leaf.css_compiled_styles.as_deref().unwrap_or(&[]);
        let TreemapCompiledStyles {
            label_styles,
            label_styles_without_font_size,
            text_fill_ownership,
            text_fill_without_font_size_ownership,
            node_styles: leaf_rect_style,
            border_styles: _,
        } = treemap_styles2_string(leaf_css, options.work_meter())?;
        let label_styles_suffix = replace_first(&label_styles, "color:", "fill:");
        let label_styles_without_font_size_suffix =
            replace_first(&label_styles_without_font_size, "color:", "fill:");
        let default_leaf_label_fill = theme.readable_leaf_label_fill(
            &fill,
            &leaf_rect_style,
            color_scale_label.get(&leaf.name),
        );
        let leaf_label_fill =
            typed_label_text_fill.map_or_else(|| default_leaf_label_fill.clone(), str::to_owned);

        write_treemap_leaf_group_open(&mut out, &group_class, leaf.x0, leaf.y0)?;

        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{h}" class="treemapLeaf" fill="{fill}" style="{style}" fill-opacity="0.3" stroke="{fill}" stroke-width="3"/>"#,
            w = fmt(w),
            h = fmt(h),
            fill = escape_attr(&fill),
            style = escape_attr(&leaf_rect_style)
        );
        out.checkpoint()?;

        let _ = write!(
            &mut out,
            r#"<clipPath id="{id}"><rect width="{w}" height="{h}"/></clipPath>"#,
            id = leaf_clip_id.as_str(),
            w = fmt((w - 4.0).max(0.0)),
            h = fmt((h - 4.0).max(0.0))
        );
        out.checkpoint()?;

        let available_w = w - 2.0 * label_padding;
        let available_h = h - 2.0 * label_padding;

        let leaf_label_initial_style = typography_theme.leaf_text_style(i, base_label_font_size);
        let mut label_font_size = leaf_label_initial_style.style().font_size.trunc();
        let value_scale_factor = 0.6;

        let mut label_hidden = false;
        let mut label_font_size_mutated = false;
        let mut label_measurement_matches = true;
        if available_w < min_display_threshold || available_h < min_display_threshold {
            label_hidden = true;
        } else {
            let mut style = leaf_label_initial_style.style().clone();
            let mut expected_style = leaf_label_initial_style.clone();

            loop {
                label_measurement_matches &= expected_style.matches_measurement(&style);
                if measure_treemap_computed_length(
                    &computed_length_measurer,
                    &leaf.name,
                    &style,
                    options.work_meter(),
                )? <= available_w
                    || label_font_size <= min_label_font_size
                {
                    break;
                }
                label_font_size -= 1.0;
                style.font_size = label_font_size;
                expected_style = leaf_label_initial_style.with_font_size_px(label_font_size);
            }

            label_font_size = fit_treemap_label_height(
                label_font_size,
                available_h,
                min_label_font_size,
                base_value_font_size,
                min_value_font_size,
                spacing_between_label_and_value,
                options.work_meter(),
            )?;
            expected_style = leaf_label_initial_style.with_font_size_px(label_font_size);

            style.font_size = label_font_size;
            if is_complex_treemap {
                if label_font_size < min_label_font_size || available_h < min_label_font_size {
                    label_hidden = true;
                }
            } else {
                label_measurement_matches &= expected_style.matches_measurement(&style);
                if measure_treemap_computed_length(
                    &computed_length_measurer,
                    &leaf.name,
                    &style,
                    options.work_meter(),
                )? > available_w
                    || label_font_size < min_label_font_size
                    || available_h < label_font_size
                {
                    label_hidden = true;
                }
            }
            // Mermaid always calls `selection.style("font-size", ...)` after the fitting pass.
            // That mutation replaces the authored `font-size !important` declaration.
            label_font_size_mutated = true;
        }

        let final_label_text_style = if label_font_size_mutated {
            leaf_label_initial_style.with_font_size_px(label_font_size)
        } else {
            leaf_label_initial_style.clone()
        };
        let label_style = if !label_font_size_mutated {
            let mut style = format!(
                "text-anchor: middle; dominant-baseline: middle; font-size: {font_size}px;fill:{fill};{suffix}",
                font_size = fmt(base_label_font_size),
                fill = escape_attr(&leaf_label_fill),
                suffix = label_styles_suffix
            );
            if label_hidden {
                style.push_str(" display: none;");
            }
            style
        } else {
            let fill = normalize_dom_style_color(&leaf_label_fill);
            let mut s = format!(
                "text-anchor: middle; dominant-baseline: middle; font-size: {fs}px; fill: {fill};",
                fs = fmt(label_font_size),
                fill = escape_attr(&fill),
            );
            if label_hidden {
                s.push_str(" display: none;");
            }
            if !label_styles_without_font_size_suffix.is_empty() {
                s.push_str(&label_styles_without_font_size_suffix);
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
        if let Some(receipt) = typography_theme_receipt.as_mut() {
            receipt.record_text(
                crate::treemap::TreemapTextRole::LeafLabel,
                !label_hidden && !leaf.name.trim().is_empty(),
                if label_font_size_mutated {
                    text_fill_without_font_size_ownership
                } else {
                    text_fill_ownership
                },
                &final_label_text_style,
                label_measurement_matches,
            );
        }

        if layout.show_values {
            let value_text = if leaf.value != 0.0 {
                treemap_format_value(leaf.value, &layout.value_format)
            } else {
                String::new()
            };
            let leaf_value_initial_style =
                typography_theme.leaf_text_style(i, base_value_font_size);
            let mut final_value_text_style = leaf_value_initial_style.clone();
            let mut value_font_size = base_value_font_size;
            let mut value_y = h / 2.0; // placeholder (overwritten when label is visible)
            let mut value_hidden = true;

            if !label_hidden {
                let actual_value_font_size = (label_font_size * value_scale_factor)
                    .round()
                    .min(base_value_font_size)
                    .max(min_value_font_size);
                value_font_size = actual_value_font_size;
                final_value_text_style =
                    leaf_value_initial_style.with_font_size_px(value_font_size);

                let label_center_y = h / 2.0;
                value_y =
                    label_center_y + (label_font_size / 2.0) + spacing_between_label_and_value;

                let cell_bottom_padding = 4.0;
                let max_value_bottom_y = h - cell_bottom_padding;
                let available_w_for_value = w - 2.0 * label_padding;

                let value_w_px = measure_treemap_computed_length(
                    &computed_length_measurer,
                    &value_text,
                    final_value_text_style.style(),
                    options.work_meter(),
                )?;
                if value_w_px <= available_w_for_value
                    && value_y + value_font_size <= max_value_bottom_y
                    && value_font_size >= min_value_font_size
                {
                    value_hidden = false;
                }
            }

            let value_fill = typed_value_text_fill
                .map_or_else(|| default_leaf_label_fill.clone(), str::to_owned);
            let mut value_style = if !label_hidden {
                let fill = normalize_dom_style_color(&value_fill);
                format!(
                    "text-anchor: middle; dominant-baseline: hanging; font-size: {fs}px; fill: {fill};",
                    fs = fmt(value_font_size),
                    fill = escape_attr(&fill)
                )
            } else {
                format!(
                    "text-anchor: middle; dominant-baseline: hanging; font-size: {fs}px;fill:{fill};{suffix}",
                    fs = fmt(base_value_font_size),
                    fill = escape_attr(&value_fill),
                    suffix = label_styles_suffix,
                )
            };
            if value_hidden {
                value_style.push_str(" display: none;");
            }
            if !label_hidden && !label_styles_without_font_size_suffix.is_empty() {
                value_style.push_str(&label_styles_without_font_size_suffix);
            }

            if value_text.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapValue" x="{x}" y="{y}" style="{style}" clip-path="url(#{id})"/>"#,
                    x = fmt(w / 2.0),
                    y = fmt(value_y),
                    style = escape_attr(&value_style),
                    id = leaf_clip_id.as_str(),
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapValue" x="{x}" y="{y}" style="{style}" clip-path="url(#{id})">{text}</text>"#,
                    x = fmt(w / 2.0),
                    y = fmt(value_y),
                    style = escape_attr(&value_style),
                    id = leaf_clip_id.as_str(),
                    text = escape_xml(&value_text)
                );
            }
            out.checkpoint()?;
            if let Some(receipt) = typography_theme_receipt.as_mut() {
                receipt.record_text(
                    crate::treemap::TreemapTextRole::LeafValue,
                    !value_hidden && !value_text.is_empty(),
                    if label_hidden {
                        text_fill_ownership
                    } else {
                        text_fill_without_font_size_ownership
                    },
                    &final_value_text_style,
                    true,
                );
            }
        }

        out.push_str("</g>");
        out.checkpoint()?;
    }

    out.push_str("</g></svg>\n");
    let rooted = root_document.complete(out.finish()?)?;
    if title_theme_receipt.is_some_and(|receipt| !title_theme.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "Treemap title theme receipt did not match the terminal SVG".to_string(),
        });
    }
    if let Some(receipt) = typography_theme_receipt
        && !typography_theme.record_terminal(
            receipt,
            options.resolved_theme(),
            options.work_meter(),
        )?
    {
        return Err(crate::Error::InvalidModel {
            message: "Treemap typography receipt did not match the terminal SVG".to_string(),
        });
    }
    Ok(rooted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};
    use std::fmt;
    use std::ops::Range;

    #[test]
    fn treemap_height_search_matches_decrement_and_bounds_extreme_work() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
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
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
        let control = merman_core::OperationControl::new();
        control.cancel();
        let meter = OperationWorkMeter::new_with_control(policy, control);
        let Err(crate::Error::Cancelled(error)) =
            fit_treemap_label_height(1e20, 100.0, 8.0, 28.0, 6.0, 2.0, &meter)
        else {
            panic!("height fitting must observe cancellation before searching");
        };
        assert_eq!(error.phase, merman_core::OperationPhase::Emit);
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn treemap_value_formats_match_mermaid_special_cases() {
        assert_eq!(treemap_format_value(1_234_567.0, ","), "1,234,567");
        assert_eq!(treemap_format_value(1_234.5, ""), "1,234.5");
        assert_eq!(treemap_format_value(-1_234.5, ","), "−1,234.5");
        assert_eq!(treemap_format_value(0.0, ","), "0");
        assert_eq!(treemap_format_value(0.35, ".1%"), "35.0%");
        assert_eq!(treemap_format_value(0.6723, ".2f"), "0.67");
        assert_eq!(treemap_format_value(1_234_567.0, ".2e"), "1.23e+6");
        assert_eq!(treemap_format_value(700_000.0, "$0,0"), "$700,000");
        assert_eq!(treemap_format_value(0.35, "$.1%"), "$35.0%");
        assert_eq!(treemap_format_value(1_234.5, "$,.2f"), "$1.2e+3");
        assert_eq!(treemap_format_value(1_234.5, "$,.0f"), "$1e+3");
        assert_eq!(treemap_format_value(1_234.5, "invalid"), "1,234.5");
        assert_eq!(treemap_format_value(1_234.5, "$invalid"), "1,234.5");
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

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> crate::Result<()> {
            if self.rejected {
                Err(crate::Error::InvalidModel {
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

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
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

        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution =
            SvgExecution::unthemed_for_test(&request, &debug, &session, DiagramFamilyId::TREEMAP)
                .expect("SVG execution");
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let title_theme = crate::treemap::TreemapTitleThemePlan::resolve(
            None,
            &config,
            None,
            session.work_meter().as_ref(),
        )
        .unwrap();
        let typography_theme = crate::treemap::TreemapTypographyThemePlan::resolve(
            None,
            &config,
            &layout,
            std::sync::Arc::clone(session.work_meter()),
        )
        .unwrap();
        let svg = render_treemap_diagram_svg(
            &layout,
            config.as_value(),
            &title_theme,
            &typography_theme,
            &execution,
        )
        .unwrap();

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
